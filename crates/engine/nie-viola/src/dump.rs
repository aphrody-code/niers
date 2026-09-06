//! **Dump** — extraction massive de toutes les archives CPK vers une arborescence claire.
//!
//! # Ce qui change par rapport aux implémentations amont
//!
//! Trois implémentations existent ailleurs : `Viola.Core/Dump` (C#), son port `src/viola/dump.cpp`
//! (C++), et `Telmo26/ievr_toolbox` (Rust). Les deux premières attribuent **un thread par CPK** en
//! piochant dans une liste non triée ; la troisième trie déjà les packs par taille décroissante,
//! **le point 1 ci-dessous ne la vise donc pas**. En revanche elle déchiffre chaque pack vers un
//! dossier temporaire **sur disque** avant d'extraire, et borne la mémoire par un pool à
//! `Condvar` : ~57 Gio écrits puis relus, là où le mappage mémoire du point 2 rend les deux
//! inutiles.
//!
//! 1. **Ordonnancement.** Les packs d'IEVR sont très inégaux (de quelques Kio à plusieurs Gio).
//!    Avec une file non triée, le temps total est dicté par le dernier gros pack tiré : si un
//!    pack de 6 Gio part en dernier, tous les cœurs l'attendent. Ici les CPK sont triés par
//!    **volume décroissant** avant distribution — c'est l'ordonnancement LPT (*longest
//!    processing time first*), dont la borne de Graham garantit un temps total d'au plus
//!    `4/3 − 1/(3m)` fois l'optimal, là où une file arbitraire n'a aucune borne. Le volume de
//!    chaque pack est **connu d'avance** : il est déjà dans l'index du VFS ([`VfsEntry::file_size`]),
//!    donc l'ordonnancement ne coûte rien de plus qu'un tri.
//!
//! 2. **Empreinte mémoire.** Viola et son port C++ lisent le CPK entier en mémoire (`read_to_end`
//!    / `std::vector`), et `nie_formats::vfs::Vfs::read` fait pire : il **conserve** chaque CPK
//!    déchiffré dans un cache jamais purgé — dumper les ~57 Gio de packs par ce chemin ferait
//!    tenir tout le jeu en RAM. Ici chaque pack est **mappé en mémoire** (`memmap2`) : les pages
//!    sont chargées à la demande et rendues par l'OS, l'occupation reste bornée quel que soit le
//!    nombre de travailleurs.
//!
//! 3. **Recherche d'entrée.** `Vfs::read` retrouve l'entrée d'un fichier par **balayage linéaire**
//!    du sommaire du pack, jusqu'à quatre passes, et recommence pour chaque fichier — soit
//!    `O(N·M)` sur un pack de `N` entrées dont on extrait `M`. Ici le sommaire est parcouru une
//!    fois par pack, et l'extraction se fait directement sur ses entrées.
//!
//! S'y ajoutent deux propriétés qu'**aucune** des trois implémentations amont n'offre : la
//! **reprise** d'un dump interrompu au pack près, et le **saut des fichiers déjà à la bonne
//! taille**, qui rend un second dump quasi instantané.
//!
//! # Ce qui fait la *qualité* d'un dump, et non sa vitesse
//!
//! Un dump rapide qui rend 250 000 fichiers dont on ignore lesquels manquent ou sont tronqués
//! n'a rien prouvé. Quatre propriétés le rendent vérifiable ; les quatre manquaient.
//!
//! 4. **Couverture.** `Vfs::iter` n'expose que l'index issu de `cpk_list.cfg.bin`. Les packs
//!    **absents du `cpk_list`** (films, `sound_asset`, packs de mise à jour) vivent dans un
//!    second index ([`Vfs::iter_extra`]) — ils étaient donc absents de la sortie *et* du total
//!    affiché, ce qui rendait le manque invisible. Les deux index sont maintenant parcourus, et
//!    [`DumpReport::depuis_extra`] chiffre ce que le premier ratait.
//!
//! 5. **Correction.** Le sommaire du CPK annonce la taille attendue après décompression
//!    (`extract_size`). Une décompression CRILAYLA silencieusement tronquée produisait un
//!    fichier plus court, écrit sans un mot. Elle est désormais rejetée
//!    ([`Raison::TailleInattendue`]). De même, le repli « par nom de base » — utile quand le
//!    chemin du `cpk_list` et celui du sommaire divergent — **n'accepte plus un nom ambigu** :
//!    associer un chemin au mauvais homonyme écrivait un contenu faux sous un nom juste, la
//!    pire des erreurs possibles ici puisqu'elle survit à toute vérification par taille.
//!
//! 6. **Diagnostic.** Un compteur d'échecs unique ne dit pas quoi réparer. Chaque échec porte
//!    maintenant une [`Raison`], ventilée dans le rapport et détaillée fichier par fichier dans
//!    un journal JSON déposé à la racine de la sortie. « 4 812 échecs » devient « 4 812 entrées
//!    introuvables dans trois packs », qui se corrige.
//!
//! 7. **Coût du second passage.** Le saut d'un fichier déjà présent se décidait *après* l'avoir
//!    déchiffré et décompressé — tout le travail était fait pour être jeté. La taille attendue
//!    étant dans le sommaire, la décision se prend maintenant **avant** l'extraction.
//!
//! Reste un piège propre à Windows, que ce module signale sans le corriger : deux chemins VFS
//! qui ne diffèrent que par la casse désignent le **même** fichier sur NTFS, et le second
//! écrase le premier. Le rapport les compte ([`DumpReport::collisions_casse`]) et les nomme
//! dans le journal ; les renommer trahirait l'arborescence du jeu, ce qui n'est pas au dump
//! d'en décider.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use nie_formats::cpk::{CpkEntry, CpkReader};
use nie_formats::vfs::Vfs;
use rayon::prelude::*;

use crate::filtre::Filtre;

/// Cause nommée d'un échec d'extraction.
///
/// Le seul intérêt de nommer les causes est de rendre un dump réparable : un total d'échecs ne
/// distingue pas un pack absent du disque (à réinstaller) d'une entrée introuvable dans son
/// sommaire (index à corriger) ou d'une décompression tronquée (bug de format).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Raison {
    /// Le fichier `.cpk` n'a pas pu être ouvert (absent, verrouillé, droits).
    PackAbsent,
    /// Le pack existe mais n'a pas pu être mappé en mémoire.
    PackNonMappable,
    /// Le sommaire du pack (table `@UTF`) est illisible ou non déchiffrable.
    SommaireIllisible,
    /// Le chemin annoncé par l'index n'existe pas dans le sommaire du pack.
    EntreeIntrouvable,
    /// Le repli par nom de base a trouvé **plusieurs** candidats : refusé plutôt que deviné.
    NomAmbigu,
    /// Le déchiffrement ou la décompression CRILAYLA a échoué.
    Extraction,
    /// L'extraction a réussi mais rend une taille différente de celle annoncée par le sommaire.
    TailleInattendue,
    /// L'écriture sur le disque de sortie a échoué.
    Ecriture,
    /// Fichier hors pack (« loose ») illisible à la source.
    SourceIllisible,
}

impl Raison {
    /// Toutes les causes, dans l'ordre du rapport.
    pub const TOUTES: [Self; 9] = [
        Self::PackAbsent,
        Self::PackNonMappable,
        Self::SommaireIllisible,
        Self::EntreeIntrouvable,
        Self::NomAmbigu,
        Self::Extraction,
        Self::TailleInattendue,
        Self::Ecriture,
        Self::SourceIllisible,
    ];

    /// Nombre de causes distinctes — dimension des ventilations du rapport.
    pub const N: usize = Self::TOUTES.len();

    /// Identifiant stable, utilisé dans le journal JSON et à l'affichage.
    #[must_use]
    pub const fn nom(self) -> &'static str {
        match self {
            Self::PackAbsent => "pack_absent",
            Self::PackNonMappable => "pack_non_mappable",
            Self::SommaireIllisible => "sommaire_illisible",
            Self::EntreeIntrouvable => "entree_introuvable",
            Self::NomAmbigu => "nom_ambigu",
            Self::Extraction => "extraction",
            Self::TailleInattendue => "taille_inattendue",
            Self::Ecriture => "ecriture",
            Self::SourceIllisible => "source_illisible",
        }
    }

    /// Position dans les tableaux ventilés par cause.
    #[must_use]
    pub const fn indice(self) -> usize {
        self as usize
    }
}

/// Un échec, tel qu'il est journalisé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Echec {
    /// Chemin VFS concerné.
    pub chemin: String,
    /// Pack d'origine, vide pour un fichier hors pack.
    pub pack: String,
    /// Cause.
    pub raison: Raison,
    /// Précision libre (tailles attendue/obtenue, message d'E/S).
    pub detail: String,
}

/// Réglages d'un dump. Les valeurs par défaut sont celles qu'on veut dans une interface :
/// reprise active, réécriture évitée, tous les cœurs, et toutes les vérifications actives —
/// un dump muet sur ses propres manques ne servirait à rien.
#[derive(Debug, Clone)]
pub struct DumpOptions {
    /// Ne garder que les chemins retenus par ce filtre (cf. [`Filtre`] : listes, `**`, `!`).
    ///
    /// Un nom de preset se résout d'abord par [`crate::presets::resoudre`].
    pub filtre: Option<String>,
    /// Écrire (et relire) un manifeste de reprise dans le dossier de sortie.
    pub reprise: bool,
    /// Ne pas réécrire un fichier déjà présent à la taille attendue.
    pub sauter_identiques: bool,
    /// Nombre de travailleurs ; `None` = tous les cœurs disponibles.
    pub threads: Option<usize>,
    /// Inclure les packs absents de `cpk_list.cfg.bin` (films, `sound_asset`, mises à jour).
    ///
    /// Les exclure ne se justifie que pour reproduire à l'identique le périmètre d'un outil
    /// amont ; c'est plusieurs milliers de fichiers réels en moins.
    pub inclure_extra: bool,
    /// Rejeter une extraction dont la taille ne correspond pas au sommaire du pack.
    pub verifier_taille: bool,
    /// Déposer le journal des échecs à la racine de la sortie.
    pub journal: bool,
    /// Déposer l'index de contenu (chemin, taille, pack) à la racine de la sortie.
    pub index_contenu: bool,
    /// Détecter les chemins qui ne diffèrent que par la casse (destructeur sur NTFS).
    pub controler_casse: bool,
}

impl Default for DumpOptions {
    fn default() -> Self {
        Self {
            filtre: None,
            reprise: true,
            sauter_identiques: true,
            threads: None,
            inclure_extra: true,
            verifier_taille: true,
            journal: true,
            index_contenu: false,
            controler_casse: true,
        }
    }
}

/// Avancement d'un dump, poussé au rythme des fichiers traités.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DumpProgress {
    /// Fichiers traités (extraits, sautés ou en échec).
    pub faits: usize,
    /// Fichiers à traiter au total.
    pub total: usize,
    /// Octets réellement écrits sur le disque.
    pub octets: u64,
}

/// Bilan final d'un dump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DumpReport {
    /// Fichiers écrits.
    pub extraits: usize,
    /// Fichiers ignorés parce que déjà présents à la bonne taille.
    pub sautes: usize,
    /// Fichiers dont l'extraction ou l'écriture a échoué — jamais fatal.
    pub echecs: usize,
    /// Octets écrits.
    pub octets: u64,
    /// Packs entièrement sautés grâce au manifeste de reprise.
    pub packs_repris: usize,
    /// `true` si l'appelant a demandé l'arrêt avant la fin.
    pub annule: bool,
    /// Fichiers planifiés, filtre appliqué — le dénominateur de la couverture.
    pub total: usize,
    /// Part du total venue de l'index supplémentaire (packs hors `cpk_list`).
    pub depuis_extra: usize,
    /// Paires de chemins ne différant que par la casse : sur NTFS, le second écrase le premier.
    pub collisions_casse: usize,
    /// Échecs ventilés par [`Raison`], indexés par [`Raison::indice`].
    pub par_raison: [usize; Raison::N],
}

impl DumpReport {
    /// Ventilation des échecs, causes vides omises.
    pub fn echecs_par_raison(&self) -> impl Iterator<Item = (Raison, usize)> + '_ {
        Raison::TOUTES.into_iter().filter_map(|r| {
            let n = self.par_raison[r.indice()];
            (n > 0).then_some((r, n))
        })
    }
}

/// Nom du manifeste de reprise, déposé à la racine de la sortie.
const MANIFESTE: &str = ".nie-dump-manifest.json";

/// Nom du journal des échecs, déposé à la racine de la sortie.
const JOURNAL: &str = ".nie-dump-echecs.json";

/// Nom de l'index de contenu, déposé à la racine de la sortie.
const INDEX_CONTENU: &str = ".nie-dump-index.tsv";

/// Au-delà, le journal ne retient plus le détail : les compteurs, eux, restent exacts. Un pack
/// entièrement absent produirait sinon un journal de plusieurs dizaines de milliers d'entrées
/// toutes identiques, illisible et coûteux à écrire.
const MAX_ECHECS_JOURNALISES: usize = 20_000;

/// Packs déjà terminés lors d'un dump précédent, lus depuis le manifeste.
fn lire_manifeste(sortie: &Path) -> Vec<String> {
    std::fs::read_to_string(sortie.join(MANIFESTE))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
}

/// Enregistre les packs terminés. Écriture par fichier temporaire puis renommage : une coupure
/// pendant l'écriture laisserait sinon un manifeste tronqué, donc un dump « repris » incomplet
/// et silencieusement faux.
fn ecrire_manifeste(sortie: &Path, faits: &[String]) {
    let Ok(json) = serde_json::to_string(faits) else {
        return;
    };
    let tmp = sortie.join(format!("{MANIFESTE}.tmp"));
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, sortie.join(MANIFESTE));
    }
}

/// Chemin VFS d'une entrée de sommaire CPK.
///
/// La règle doit être **exactement** celle qu'applique `Vfs::discover_extra_cpks` pour peupler
/// l'index supplémentaire, sans quoi chaque chemin de cet index manquerait sa propre entrée :
/// un répertoire vide ne produit pas de `/` de tête.
fn chemin_entree(e: &CpkEntry) -> String {
    if e.directory.is_empty() {
        e.filename.clone()
    } else {
        format!("{}/{}", e.directory, e.filename)
    }
}

/// Hachage FNV-1a insensible à la casse — sert à repérer les collisions NTFS sans conserver
/// 255 000 chemins minusculisés en mémoire.
fn hash_insensible(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b.to_ascii_lowercase());
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Écrit un fichier extrait, en sautant l'écriture si la taille sur disque correspond déjà.
///
/// Renvoie `Ok(true)` si le fichier a été écrit, `Ok(false)` s'il a été sauté.
fn ecrire(dest: &Path, octets: &[u8], sauter_identiques: bool) -> std::io::Result<bool> {
    if sauter_identiques && deja_a_la_taille(dest, octets.len() as u64) {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, octets)?;
    Ok(true)
}

/// Le fichier de destination existe-t-il déjà à la taille attendue ?
///
/// Interrogé **avant** l'extraction quand le sommaire annonce la taille : c'est ce qui rend un
/// second passage quasi gratuit, là où décider après extraction faisait déchiffrer et
/// décompresser ~57 Gio pour les jeter.
fn deja_a_la_taille(dest: &Path, taille: u64) -> bool {
    std::fs::metadata(dest).is_ok_and(|m| m.len() == taille)
}

/// Sérialise le journal des échecs. JSON plutôt que texte libre : ce fichier est fait pour être
/// relu par l'explorateur et par les tests, pas seulement par un humain.
fn ecrire_journal(sortie: &Path, echecs: &[Echec], total: usize, collisions: &[(String, String)]) {
    let liste: Vec<serde_json::Value> = echecs
        .iter()
        .map(|e| {
            serde_json::json!({
                "chemin": e.chemin,
                "pack": e.pack,
                "raison": e.raison.nom(),
                "detail": e.detail,
            })
        })
        .collect();
    let casse: Vec<serde_json::Value> = collisions
        .iter()
        .map(|(a, b)| serde_json::json!({ "premier": a, "second": b }))
        .collect();
    let doc = serde_json::json!({
        "echecs_total": total,
        "echecs_journalises": liste.len(),
        "tronque": total > liste.len(),
        "echecs": liste,
        "collisions_casse": casse,
    });
    let Ok(json) = serde_json::to_string_pretty(&doc) else {
        return;
    };
    let _ = std::fs::write(sortie.join(JOURNAL), json);
}

/// Trie et dédoublonne l'index de contenu écrit en flux par les travailleurs.
///
/// Sans cette passe l'index sortirait dans l'ordre d'ordonnancement — donc différent à chaque
/// exécution — et une reprise y empilerait les lignes du passage précédent. Trié et dédoublonné,
/// il devient comparable d'un dump à l'autre, ce qui est tout son intérêt.
fn normaliser_index(sortie: &Path) {
    let chemin = sortie.join(INDEX_CONTENU);
    let Ok(contenu) = std::fs::read_to_string(&chemin) else {
        return;
    };
    let mut lignes: Vec<&str> = contenu.lines().filter(|l| !l.is_empty()).collect();
    lignes.sort_unstable();
    lignes.dedup();
    let mut sortie_texte = String::with_capacity(contenu.len());
    for l in lignes {
        sortie_texte.push_str(l);
        sortie_texte.push('\n');
    }
    let _ = std::fs::write(&chemin, sortie_texte);
}

/// Ce qu'un pack doit rendre : ses chemins, son volume, et d'où vient sa liste.
struct PlanPack<'a> {
    fichiers: Vec<&'a str>,
    octets: u64,
    extra: bool,
}

/// Extrait tout le VFS (ou la part retenue par le filtre) vers `sortie`.
///
/// `progres` est appelé depuis plusieurs threads : il doit être bon marché et ne rien supposer
/// de l'ordre. `annuler` est consulté entre deux fichiers ; un dump annulé laisse le manifeste
/// cohérent (seuls les packs entièrement terminés y figurent), donc reprenable.
///
/// # Errors
/// Si le dossier de sortie ne peut pas être créé. Les échecs par fichier sont comptés et
/// journalisés, jamais remontés : un pack corrompu ne doit pas emporter les 935 autres.
#[allow(clippy::too_many_lines)]
pub fn dump_all(
    vfs: &Vfs,
    sortie: &Path,
    options: &DumpOptions,
    annuler: &AtomicBool,
    progres: &(dyn Fn(DumpProgress) + Send + Sync),
) -> Result<DumpReport, String> {
    std::fs::create_dir_all(sortie).map_err(|e| format!("{} : {e}", sortie.display()))?;

    // Compilé une fois : l'ancien chemin réinterprétait le motif pour chacun des 255 308 chemins.
    let filtre = options
        .filtre
        .as_deref()
        .map_or_else(Filtre::default, Filtre::parse);

    // ── Regroupement par pack ────────────────────────────────────────────────────────────────
    // Le coût de chaque pack est connu ici même (tailles déjà indexées), ce qui permet de trier
    // avant de distribuer — c'est tout l'intérêt de passer par l'index plutôt que par `Vfs::read`.
    let mut par_cpk: HashMap<&str, PlanPack> = HashMap::new();
    let mut loose: Vec<&str> = Vec::new();
    for (chemin, entree) in vfs.iter() {
        if !filtre.accepte(chemin) {
            continue;
        }
        if entree.cpk_filename.is_empty() {
            loose.push(chemin);
        } else {
            let e = par_cpk
                .entry(entree.cpk_filename.as_str())
                .or_insert(PlanPack {
                    fichiers: Vec::new(),
                    octets: 0,
                    extra: false,
                });
            e.fichiers.push(chemin);
            e.octets += u64::from(entree.file_size);
        }
    }

    // Index supplémentaire : les packs qui ne figurent pas dans `cpk_list.cfg.bin`. `Vfs::iter`
    // ne les voit pas, ce qui les rendait absents de la sortie ET du total — un manque
    // indétectable depuis le rapport.
    let mut depuis_extra = 0usize;
    if options.inclure_extra {
        for (chemin, cpk) in vfs.iter_extra() {
            if !filtre.accepte(chemin) {
                continue;
            }
            let e = par_cpk.entry(cpk).or_insert(PlanPack {
                fichiers: Vec::new(),
                octets: 0,
                extra: true,
            });
            e.fichiers.push(chemin);
            depuis_extra += 1;
        }
    }

    // Ces packs-là n'ont pas de tailles indexées : sans volume, le tri LPT les enverrait en
    // dernier alors que les films sont parmi les plus gros. La taille du `.cpk` sur disque est
    // le bon substitut, et coûte un `stat` par pack.
    let dossier_packs = vfs.game_data_dir().join("packs");
    for (cpk, plan) in &mut par_cpk {
        if plan.extra && plan.octets == 0 {
            plan.octets = std::fs::metadata(dossier_packs.join(*cpk)).map_or(0, |m| m.len());
        }
    }

    // ── Collisions de casse ──────────────────────────────────────────────────────────────────
    // Sur NTFS `Data/X.bin` et `data/x.bin` sont le même fichier : le second écrase le premier,
    // sans erreur. Les compter n'est pas cosmétique — c'est la seule façon de savoir qu'une
    // sortie de 255 000 fichiers en contient 254 990.
    let mut collisions: Vec<(String, String)> = Vec::new();
    if options.controler_casse {
        let attendus = par_cpk.values().map(|p| p.fichiers.len()).sum::<usize>() + loose.len();
        let mut vus: HashMap<u64, &str> = HashMap::with_capacity(attendus);
        let tous = par_cpk
            .values()
            .flat_map(|p| p.fichiers.iter().copied())
            .chain(loose.iter().copied());
        for chemin in tous {
            match vus.entry(hash_insensible(chemin)) {
                Entry::Occupied(o) => {
                    // Le hachage peut collisionner sans que les chemins le fassent : confirmer.
                    if o.get().eq_ignore_ascii_case(chemin) {
                        collisions.push(((*o.get()).to_string(), chemin.to_string()));
                    }
                }
                Entry::Vacant(v) => {
                    v.insert(chemin);
                }
            }
        }
    }

    let deja: Vec<String> = if options.reprise {
        lire_manifeste(sortie)
    } else {
        Vec::new()
    };
    let packs_repris = par_cpk
        .keys()
        .filter(|c| deja.iter().any(|d| d == *c))
        .count();

    // Tri par volume décroissant : ordonnancement LPT (cf. doc du module).
    let mut packs: Vec<(&str, PlanPack)> = par_cpk
        .into_iter()
        .filter(|(cpk, _)| !deja.iter().any(|d| d == cpk))
        .collect();
    packs.sort_unstable_by(|a, b| b.1.octets.cmp(&a.1.octets).then_with(|| a.0.cmp(b.0)));

    let total: usize = packs.iter().map(|p| p.1.fichiers.len()).sum::<usize>() + loose.len();

    let faits = AtomicUsize::new(0);
    let extraits = AtomicUsize::new(0);
    let sautes = AtomicUsize::new(0);
    let echecs = AtomicUsize::new(0);
    let octets = AtomicU64::new(0);
    let par_raison: [AtomicUsize; Raison::N] = std::array::from_fn(|_| AtomicUsize::new(0));
    let journal: std::sync::Mutex<Vec<Echec>> = std::sync::Mutex::new(Vec::new());
    let termines: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(deja.clone());

    // L'index de contenu est écrit en flux : le retenir en mémoire coûterait ~30 Mio de chaînes
    // pour un fichier qu'on ne relit qu'à la fin.
    let index_fichier = options.index_contenu.then(|| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(sortie.join(INDEX_CONTENU))
            .ok()
            .map(std::sync::Mutex::new)
    });
    let index_fichier = index_fichier.flatten();

    // Un échec sans cause ne se répare pas ; un journal sans borne ne s'écrit pas. Les compteurs
    // restent exacts au-delà de la borne, seul le détail s'arrête.
    let noter = |chemin: &str, pack: &str, raison: Raison, detail: String| {
        echecs.fetch_add(1, Ordering::Relaxed);
        par_raison[raison.indice()].fetch_add(1, Ordering::Relaxed);
        if options.journal
            && let Ok(mut j) = journal.lock()
            && j.len() < MAX_ECHECS_JOURNALISES
        {
            j.push(Echec {
                chemin: chemin.to_string(),
                pack: pack.to_string(),
                raison,
                detail,
            });
        }
    };

    // Un compte rendu par fichier saturerait le canal d'événements sur 255 000 entrées : on
    // n'en pousse qu'un tous les 256 fichiers, plus un à la toute fin.
    let signaler = |force: bool| {
        let n = faits.load(Ordering::Relaxed);
        if force || n.is_multiple_of(256) {
            progres(DumpProgress {
                faits: n,
                total,
                octets: octets.load(Ordering::Relaxed),
            });
        }
    };

    let traiter_pack = |(cpk, plan): &(&str, PlanPack)| {
        if annuler.load(Ordering::Relaxed) {
            return;
        }
        let cpk = *cpk;
        let fichiers = &plan.fichiers;
        let tout_en_echec = |raison: Raison, detail: &str| {
            for chemin in fichiers {
                noter(chemin, cpk, raison, detail.to_string());
                faits.fetch_add(1, Ordering::Relaxed);
            }
        };

        let chemin_pack = dossier_packs.join(cpk);
        // Mappage mémoire : les pages sont paginées à la demande, jamais tout le pack d'un coup.
        let fichier = match std::fs::File::open(&chemin_pack) {
            Ok(f) => f,
            Err(e) => return tout_en_echec(Raison::PackAbsent, &e.to_string()),
        };
        // SAFETY : le mappage n'est valide que tant que le fichier n'est pas modifié sous nos
        // pieds. Les packs du jeu sont en lecture seule pendant un dump ; c'est l'hypothèse que
        // fait déjà tout lecteur de CPK du dépôt.
        let mmap = match unsafe { memmap2::Mmap::map(&fichier) } {
            Ok(m) => m,
            Err(e) => return tout_en_echec(Raison::PackNonMappable, &e.to_string()),
        };
        let lecteur = match CpkReader::new(&mmap, cpk) {
            Ok(l) => l,
            Err(e) => return tout_en_echec(Raison::SommaireIllisible, &format!("{e:?}")),
        };

        // Sommaire indexé UNE fois par pack : la recherche d'un fichier devient une consultation
        // de table, au lieu du balayage linéaire refait à chaque fichier par `Vfs::read`.
        let mut index: HashMap<String, usize> = HashMap::with_capacity(lecteur.entries.len());
        // Repli par nom de base, lui aussi indexé une fois. `None` marque un nom **ambigu** :
        // l'ancien repli prenait le premier venu par un `find` linéaire, donc écrivait parfois
        // le contenu d'un homonyme sous un nom juste — une faute qu'aucune vérification par
        // taille ne rattrape ensuite.
        let mut par_base: HashMap<&str, Option<usize>> =
            HashMap::with_capacity(lecteur.entries.len());
        for (i, e) in lecteur.entries.iter().enumerate() {
            index.insert(chemin_entree(e), i);
            par_base
                .entry(e.filename.as_str())
                .and_modify(|v| *v = None)
                .or_insert(Some(i));
        }

        // Second niveau de parallélisme : rayon vole le travail entre les deux niveaux, donc un
        // pack unique et énorme occupe quand même tous les cœurs — ce qu'un thread par pack
        // (les deux amonts) ne peut pas faire.
        fichiers.par_iter().for_each(|chemin| {
            if annuler.load(Ordering::Relaxed) {
                return;
            }
            let dest = sortie.join(chemin.trim_start_matches('/'));

            // Localisation : chemin exact, puis repli par nom de base s'il est sans ambiguïté.
            let indice = match index.get(*chemin) {
                Some(&i) => Some(i),
                None => {
                    let base = chemin.rsplit('/').next().unwrap_or(chemin);
                    match par_base.get(base) {
                        Some(Some(i)) => Some(*i),
                        Some(None) => {
                            noter(
                                chemin,
                                cpk,
                                Raison::NomAmbigu,
                                format!("« {base} » désigne plusieurs entrées du pack"),
                            );
                            None
                        }
                        None => {
                            noter(chemin, cpk, Raison::EntreeIntrouvable, String::new());
                            None
                        }
                    }
                }
            };

            if let Some(i) = indice {
                let entree = &lecteur.entries[i];
                // Décision de saut AVANT extraction : le sommaire annonce déjà la taille finale.
                if options.sauter_identiques
                    && entree.extract_size > 0
                    && deja_a_la_taille(&dest, entree.extract_size)
                {
                    sautes.fetch_add(1, Ordering::Relaxed);
                } else {
                    traiter_fichier(
                        &lecteur,
                        &mmap,
                        entree,
                        chemin,
                        cpk,
                        &dest,
                        options,
                        &noter,
                        &extraits,
                        &sautes,
                        &octets,
                        index_fichier.as_ref(),
                    );
                }
            }
            faits.fetch_add(1, Ordering::Relaxed);
            signaler(false);
        });

        // Le pack n'entre au manifeste que s'il a été traité en entier : un pack interrompu doit
        // être refait, sans quoi la reprise produirait un dump incomplet réputé complet.
        if !annuler.load(Ordering::Relaxed)
            && let Ok(mut t) = termines.lock()
        {
            t.push(cpk.to_string());
            if options.reprise {
                ecrire_manifeste(sortie, &t);
            }
        }
    };

    // Le nombre de travailleurs est imposé par un pool local : toucher au pool global de rayon
    // depuis une bibliothèque affecterait tout le processus hôte (l'application Tauri).
    let executer = || match options.threads {
        Some(n) if n > 0 => rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build()
            .map_err(|e| e.to_string())
            .map(|pool| pool.install(|| packs.par_iter().for_each(traiter_pack))),
        _ => {
            packs.par_iter().for_each(traiter_pack);
            Ok(())
        }
    };
    executer()?;

    // Fichiers loose déclarés dans cpk_list (vidéos d'intro, configuration système) : ils sont
    // déjà sur le disque, une simple copie suffit.
    loose.par_iter().for_each(|chemin| {
        if annuler.load(Ordering::Relaxed) {
            return;
        }
        let dest = sortie.join(chemin.trim_start_matches('/'));
        // Le chemin déclaré par le `cpk_list` n'est pas toujours le chemin réel : les vidéos
        // d'introduction sont annoncées sous `common/` et rangées sous le répertoire de
        // plateforme. Le VFS sait résoudre les deux ; recalculer le chemin ici ne le saurait pas.
        let Some(source) = vfs.resolve_loose_path(chemin) else {
            noter(
                chemin,
                "",
                Raison::SourceIllisible,
                "introuvable sous data/".to_string(),
            );
            faits.fetch_add(1, Ordering::Relaxed);
            signaler(false);
            return;
        };
        // Même économie que pour les packs : la taille de la source suffit à décider.
        if options.sauter_identiques
            && let Ok(m) = std::fs::metadata(&source)
            && deja_a_la_taille(&dest, m.len())
        {
            sautes.fetch_add(1, Ordering::Relaxed);
        } else {
            match std::fs::read(&source) {
                Err(e) => noter(chemin, "", Raison::SourceIllisible, e.to_string()),
                Ok(donnees) => match ecrire(&dest, &donnees, options.sauter_identiques) {
                    Ok(true) => {
                        extraits.fetch_add(1, Ordering::Relaxed);
                        octets.fetch_add(donnees.len() as u64, Ordering::Relaxed);
                        noter_index(index_fichier.as_ref(), chemin, donnees.len() as u64, "");
                    }
                    Ok(false) => {
                        sautes.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => noter(chemin, "", Raison::Ecriture, e.to_string()),
                },
            }
        }
        faits.fetch_add(1, Ordering::Relaxed);
        signaler(false);
    });

    signaler(true);

    let echecs_total = echecs.load(Ordering::Relaxed);
    if options.journal && (echecs_total > 0 || !collisions.is_empty()) {
        let liste = journal.lock().map(|j| j.clone()).unwrap_or_default();
        ecrire_journal(sortie, &liste, echecs_total, &collisions);
    }
    if options.index_contenu {
        drop(index_fichier);
        normaliser_index(sortie);
    }

    Ok(DumpReport {
        extraits: extraits.into_inner(),
        sautes: sautes.into_inner(),
        echecs: echecs_total,
        octets: octets.into_inner(),
        packs_repris,
        annule: annuler.load(Ordering::Relaxed),
        total,
        depuis_extra,
        collisions_casse: collisions.len(),
        par_raison: std::array::from_fn(|i| par_raison[i].load(Ordering::Relaxed)),
    })
}

/// Extrait, vérifie et écrit une entrée. Séparé de la boucle pour garder celle-ci lisible.
#[allow(clippy::too_many_arguments)]
fn traiter_fichier(
    lecteur: &CpkReader,
    mmap: &[u8],
    entree: &CpkEntry,
    chemin: &str,
    cpk: &str,
    dest: &Path,
    options: &DumpOptions,
    noter: &dyn Fn(&str, &str, Raison, String),
    extraits: &AtomicUsize,
    sautes: &AtomicUsize,
    octets: &AtomicU64,
    index: Option<&std::sync::Mutex<std::fs::File>>,
) {
    let donnees = match lecteur.extract(mmap, entree) {
        Ok(d) => d,
        Err(e) => return noter(chemin, cpk, Raison::Extraction, format!("{e:?}")),
    };

    // Le sommaire annonce la taille après décompression : une CRILAYLA tronquée se voit ici, et
    // nulle part ailleurs. Sans ce test, elle produisait un fichier court écrit sans un mot.
    if options.verifier_taille
        && entree.extract_size > 0
        && donnees.len() as u64 != entree.extract_size
    {
        return noter(
            chemin,
            cpk,
            Raison::TailleInattendue,
            format!(
                "sommaire {} octets, extraction {}",
                entree.extract_size,
                donnees.len()
            ),
        );
    }

    match ecrire(dest, &donnees, options.sauter_identiques) {
        Ok(true) => {
            extraits.fetch_add(1, Ordering::Relaxed);
            octets.fetch_add(donnees.len() as u64, Ordering::Relaxed);
            noter_index(index, chemin, donnees.len() as u64, cpk);
        }
        Ok(false) => {
            sautes.fetch_add(1, Ordering::Relaxed);
        }
        Err(e) => noter(chemin, cpk, Raison::Ecriture, e.to_string()),
    }
}

/// Ajoute une ligne à l'index de contenu. Silencieux en cas d'échec : perdre une ligne d'index
/// ne doit pas faire échouer l'extraction du fichier qu'elle décrit.
fn noter_index(
    index: Option<&std::sync::Mutex<std::fs::File>>,
    chemin: &str,
    taille: u64,
    cpk: &str,
) {
    if let Some(m) = index
        && let Ok(mut f) = m.lock()
    {
        let _ = writeln!(f, "{chemin}\t{taille}\t{cpk}");
    }
}

/// Efface le manifeste de reprise — un dump complet reparti de zéro ne doit pas hériter de
/// l'état d'un dump précédent portant un autre filtre.
///
/// # Errors
/// Si le manifeste existe mais ne peut pas être supprimé.
pub fn oublier_reprise(sortie: &Path) -> std::io::Result<()> {
    let m = sortie.join(MANIFESTE);
    if m.exists() {
        std::fs::remove_file(m)
    } else {
        Ok(())
    }
}

/// Chemin du manifeste, pour l'afficher dans une interface.
#[must_use]
pub fn chemin_manifeste(sortie: &Path) -> PathBuf {
    sortie.join(MANIFESTE)
}

/// Chemin du journal des échecs, pour l'afficher ou le relire.
#[must_use]
pub fn chemin_journal(sortie: &Path) -> PathBuf {
    sortie.join(JOURNAL)
}

/// Chemin de l'index de contenu.
#[must_use]
pub fn chemin_index(sortie: &Path) -> PathBuf {
    sortie.join(INDEX_CONTENU)
}

/// Rend `Arc`-partageable un rapporteur de progression, utilisé par les appelants qui doivent
/// relayer vers un canal d'événements.
#[must_use]
pub fn rapporteur<F>(f: F) -> Arc<dyn Fn(DumpProgress) + Send + Sync>
where
    F: Fn(DumpProgress) + Send + Sync + 'static,
{
    Arc::new(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_manifeste_survit_a_un_aller_retour() {
        let dir = std::env::temp_dir().join(format!("nie-viola-manif-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dossier de test");
        assert!(
            lire_manifeste(&dir).is_empty(),
            "pas de manifeste = rien de repris"
        );
        ecrire_manifeste(&dir, &["a.cpk".to_string(), "b.cpk".to_string()]);
        assert_eq!(
            lire_manifeste(&dir),
            vec!["a.cpk".to_string(), "b.cpk".to_string()]
        );
        oublier_reprise(&dir).expect("effacement");
        assert!(lire_manifeste(&dir).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn l_ecriture_saute_un_fichier_deja_a_la_bonne_taille() {
        let dir = std::env::temp_dir().join(format!("nie-viola-ecrit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dossier de test");
        let f = dir.join("x.bin");
        assert!(
            ecrire(&f, b"1234", true).expect("première écriture"),
            "fichier absent : écrit"
        );
        assert!(
            !ecrire(&f, b"5678", true).expect("seconde"),
            "même taille : sauté"
        );
        assert_eq!(
            std::fs::read(&f).expect("relecture"),
            b"1234",
            "le contenu n'a pas bougé"
        );
        assert!(
            ecrire(&f, b"123", true).expect("taille différente"),
            "taille différente : réécrit"
        );
        assert!(
            ecrire(&f, b"123", false).expect("sans saut"),
            "saut désactivé : toujours écrit"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn le_saut_se_decide_sans_lire_le_fichier() {
        let dir = std::env::temp_dir().join(format!("nie-viola-taille-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dossier de test");
        let f = dir.join("y.bin");
        assert!(!deja_a_la_taille(&f, 4), "fichier absent");
        std::fs::write(&f, b"1234").expect("écriture");
        assert!(deja_a_la_taille(&f, 4), "taille attendue");
        assert!(!deja_a_la_taille(&f, 5), "taille différente");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn le_chemin_d_entree_suit_la_regle_de_l_index_supplementaire() {
        let avec = CpkEntry {
            filename: "a.bin".into(),
            directory: "data/chr".into(),
            offset: 0,
            size: 0,
            extract_size: 0,
            is_compressed: false,
        };
        let sans = CpkEntry {
            directory: String::new(),
            ..avec.clone()
        };
        assert_eq!(chemin_entree(&avec), "data/chr/a.bin");
        // Un `/` de tête ici ferait manquer toutes les entrées de l'index supplémentaire dont
        // le répertoire est vide : leur chemin y est stocké sans préfixe.
        assert_eq!(chemin_entree(&sans), "a.bin");
    }

    #[test]
    fn le_hachage_ignore_la_casse_et_distingue_le_reste() {
        assert_eq!(
            hash_insensible("data/Chr/A.bin"),
            hash_insensible("DATA/chr/a.BIN")
        );
        assert_ne!(
            hash_insensible("data/chr/a.bin"),
            hash_insensible("data/chr/b.bin")
        );
    }

    #[test]
    fn la_ventilation_n_affiche_que_les_causes_rencontrees() {
        let mut r = DumpReport {
            echecs: 3,
            ..DumpReport::default()
        };
        r.par_raison[Raison::EntreeIntrouvable.indice()] = 2;
        r.par_raison[Raison::TailleInattendue.indice()] = 1;
        let v: Vec<(Raison, usize)> = r.echecs_par_raison().collect();
        assert_eq!(
            v,
            vec![
                (Raison::EntreeIntrouvable, 2),
                (Raison::TailleInattendue, 1)
            ]
        );
    }

    #[test]
    fn l_index_de_contenu_est_trie_et_dedoublonne() {
        let dir = std::env::temp_dir().join(format!("nie-viola-index-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dossier de test");
        // Ordre d'extraction quelconque, et une ligne répétée par une reprise.
        std::fs::write(dir.join(INDEX_CONTENU), "z\t1\tp\na\t2\tp\nz\t1\tp\n").expect("écriture");
        normaliser_index(&dir);
        let lu = std::fs::read_to_string(dir.join(INDEX_CONTENU)).expect("relecture");
        assert_eq!(lu, "a\t2\tp\nz\t1\tp\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn le_journal_dit_qu_il_est_tronque() {
        let dir = std::env::temp_dir().join(format!("nie-viola-journal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dossier de test");
        let e = Echec {
            chemin: "data/x.bin".into(),
            pack: "p.cpk".into(),
            raison: Raison::EntreeIntrouvable,
            detail: String::new(),
        };
        ecrire_journal(
            &dir,
            std::slice::from_ref(&e),
            5000,
            &[("A".into(), "a".into())],
        );
        let lu = std::fs::read_to_string(chemin_journal(&dir)).expect("relecture");
        let v: serde_json::Value = serde_json::from_str(&lu).expect("json");
        assert_eq!(v["echecs_total"], 5000);
        assert_eq!(v["echecs_journalises"], 1);
        assert_eq!(v["tronque"], true, "1 détail pour 5000 échecs : le dire");
        assert_eq!(v["echecs"][0]["raison"], "entree_introuvable");
        assert_eq!(v["collisions_casse"][0]["second"], "a");
        std::fs::remove_dir_all(&dir).ok();
    }
}
