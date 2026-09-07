//! La couche Lua du dépôt, servie par Aphrody — `/api/v1/lua/*`.
//!
//! ## Ce que le jeu met dans un `.lua.bin`, et ce qu'on en sert
//!
//! La logique de menus, de scènes et de règles d'`nie.exe` n'est pas dans le binaire : elle
//! est compilée en **bytecode Lua 5.2 PUC-Rio** et rangée dans le VFS
//! (1 199 fichiers mesurés sur ce montage, sous `data/common/script/lua/` et
//! `data/common/gamedata/`). `nie-lua` sait deux choses très différentes de ces octets :
//!
//! | Étage | Ce qu'il fait | Exposé ici |
//! |---|---|---|
//! | `nie_lua::bytecode` | **décode** le chunk : en-tête, prototypes, constantes, instructions | oui |
//! | `nie_lua::runtime` / `session` / `menu_host` | **exécute** le chunk dans une vraie VM | **non** |
//!
//! ## Ce que ce module refuse d'exposer, et pourquoi
//!
//! `execute_with_include`, `run_menu`, `drive_menu`, `install_menu_host`, `LuaSession::eval`,
//! `set_global` et `discover_host_calls` **exécutent du code**. `discover_host_calls` en
//! particulier ressemble à de l'analyse, mais son procédé est de poser une métatable sur `_G`
//! puis d'appeler la fonction principale du script : c'est un interpréteur, avec un
//! `Lua::unsafe_new` (requis pour charger un chunk binaire) sous le capot. Rien de tout cela
//! n'a sa place derrière une URL publique, quand bien même l'entrée serait bornée au VFS —
//! un chargeur de bytecode arbitraire est une primitive d'exécution, pas un décodeur.
//!
//! Ce refus n'est pas seulement une politique : il est **structurel**. `nie-site` déclare
//! `nie-lua` avec `default-features = false`, ce qui coupe `vm` (mlua, Lua 5.2 en C) et
//! `analysis` (tree-sitter). Aucun interpréteur n'est lié dans ce processus ; il ne s'agit
//! donc pas d'une route qu'on aurait « oublié » d'écrire, mais d'une capacité absente du
//! binaire. C'est ce que [`capacites`] rapporte, mesuré, plutôt qu'affirmé.
//!
//! Conséquence assumée : les onglets d'en-tête (`enumerate_header_tabs`) et la surface d'API
//! hôte réelle ne sont pas servis, parce qu'ils ne s'obtiennent qu'en faisant tourner le
//! script. Ce que ce module rend à la place est **statique** et vérifiable : les globaux lus
//! et écrits sont extraits des instructions `GETTABUP`/`SETTABUP` sur l'upvalue `_ENV`, ce qui
//! est la définition même d'un accès à une variable globale en Lua 5.2.
//!
//! ## Deux espaces, et pourquoi le désassemblage n'est pas un suffixe
//!
//! Le chemin VFS est un **joker terminal** (`{*chemin}`) : axum ne peut rien router après lui,
//! et `/scripts/{*chemin}/desassemblage` ne compile pas comme une route. Le désassemblage a
//! donc son propre préfixe, `/api/v1/lua/desassemblage/{*chemin}`. Il rend du texte, pas du
//! JSON — c'est un listing de la forme de `luac -l -l`, fait pour être lu.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, RwLock};

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use nie_lua::bytecode::{Chunk, Constant, Prototype};

use crate::error::ErreurSite;
use crate::routes::{DemandePage, Page};
use crate::state::EtatSite;
use crate::vfs_index::IndexVfs;

/// Suffixe qui identifie un script du jeu. Les fichiers portent un numéro de version
/// (`ability_list_menu_5.00.29.00.lua.bin`) : c'est le suffixe qui est stable, jamais le nom.
pub const SUFFIXE: &str = ".lua.bin";

/// Taille au-delà de laquelle un script n'est pas analysé.
///
/// Le plus gros mesuré sur ce montage fait 88 644 octets ; la borne est donc large d'un facteur
/// cent. Elle n'existe que pour qu'un montage inattendu ne fasse pas parser un fichier de
/// plusieurs mégaoctets sur un jeton de calcul.
pub const TAILLE_MAX: usize = 8 * 1024 * 1024;

/// `Cache-Control` des analyses : le contenu d'un chemin ne change qu'avec une mise à jour du
/// jeu, mais il change — une journée, jamais `immutable`.
pub const CONTROLE: &str = "public, max-age=86400, stale-while-revalidate=604800";

/// Vrai si un interpréteur Lua est lié dans ce processus.
///
/// Il ne l'est pas, et ce n'est pas une opinion : `nie-site/Cargo.toml` déclare `nie-lua` avec
/// `default-features = false`, ce qui coupe la feature `vm` (mlua, Lua 5.2 en C). Seul
/// `nie_lua::bytecode` est lié. La constante existe pour que `/api/v1/lua` puisse le **dire**
/// au client plutôt que de le laisser deviner de l'absence de route.
pub const VM_LIEE: bool = false;

/// Nombre maximal d'analyses simultanées, faute de connaître le parallélisme de la machine.
const ANALYSES_SIMULTANEES_DEFAUT: usize = 4;

/// Le sémaphore des analyses. Comme celui du rendu 3D, il borne **le CPU de la machine** et non
/// une ressource du service : il vit donc dans le module et non dans [`EtatSite`], que les
/// tests construisent à volonté.
fn jetons_analyse() -> &'static tokio::sync::Semaphore {
    static JETONS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    JETONS.get_or_init(|| {
        let n = std::thread::available_parallelism()
            .map_or(ANALYSES_SIMULTANEES_DEFAUT, std::num::NonZeroUsize::get);
        tokio::sync::Semaphore::new(n.max(1))
    })
}

/// Prend un jeton d'analyse, ou dit pourquoi il n'y en a plus.
async fn jeton_analyse() -> Result<tokio::sync::SemaphorePermit<'static>, ErreurSite> {
    jetons_analyse()
        .acquire()
        .await
        .map_err(|_| ErreurSite::Interne("limiteur d'analyse ferme".to_owned()))
}

// ─── Le catalogue des scripts ───────────────────────────────────────────────────────────────

/// Un script du VFS, tel que le catalogue le rend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Script {
    /// Chemin VFS verbatim — c'est aussi le segment des routes d'analyse.
    pub chemin: String,
    /// Nom de la feuille, suffixe conservé.
    pub nom: String,
    /// Taille déclarée par le VFS, en octets.
    pub octets: u32,
}

/// Le catalogue mémorisé, avec la taille d'index qui l'a produit.
///
/// L'index du VFS ne se parcourt pas : il expose des dossiers et des vues, et aucune des quatre
/// vues ne retient le Lua. On le balaie donc une fois, récursivement, et on garde le résultat.
/// La clé est [`IndexVfs::len`] : un montage qui se termine change ce nombre, ce qui invalide
/// naturellement un catalogue construit sur l'index vide du démarrage.
type CatalogueMemorise = Option<(usize, Arc<Vec<Script>>)>;
static CATALOGUE: RwLock<CatalogueMemorise> = RwLock::new(None);

/// Balaye l'index et rend tous les scripts, triés par chemin.
///
/// Le parcours descend dossier par dossier depuis la racine. [`IndexVfs::dossier`] s'arrête dès
/// que le préfixe ne correspond plus (il est trié), donc le coût total est celui d'un parcours
/// par niveau, pas d'un produit.
#[must_use]
pub fn balayer(index: &IndexVfs) -> Vec<Script> {
    let mut trouves = Vec::new();
    let mut a_visiter = vec![String::new()];
    while let Some(prefixe) = a_visiter.pop() {
        let d = index.dossier(&prefixe, 0, usize::MAX);
        for f in d.fichiers {
            if f.chemin.ends_with(SUFFIXE) {
                trouves.push(Script {
                    nom: f.nom,
                    octets: f.taille,
                    chemin: f.chemin,
                });
            }
        }
        a_visiter.extend(d.dossiers);
    }
    trouves.sort_by(|a, b| a.chemin.cmp(&b.chemin));
    trouves
}

/// Le catalogue courant, balayé une fois par état d'index.
fn catalogue(index: &IndexVfs) -> Arc<Vec<Script>> {
    let cle = index.len();
    if let Ok(garde) = CATALOGUE.read()
        && let Some((k, v)) = garde.as_ref()
        && *k == cle
    {
        return Arc::clone(v);
    }
    let liste = Arc::new(balayer(index));
    if let Ok(mut garde) = CATALOGUE.write() {
        *garde = Some((cle, Arc::clone(&liste)));
    }
    liste
}

// ─── Les capacités ──────────────────────────────────────────────────────────────────────────

/// Une capacité de la couche Lua, servie ou refusée, avec sa raison.
#[derive(Debug, Clone, Serialize)]
pub struct Capacite {
    /// Nom court, stable.
    pub nom: &'static str,
    /// `servi` ou `refuse` — un jeton choisi, jamais un `Debug`.
    pub etat: &'static str,
    /// Route qui la rend, quand elle est servie.
    pub route: Option<&'static str>,
    /// Pourquoi elle est refusée, quand elle l'est.
    pub raison: Option<&'static str>,
}

/// Corps de `/api/v1/lua`.
#[derive(Debug, Clone, Serialize)]
pub struct CapacitesLua {
    /// Toujours `nie-site`.
    pub service: &'static str,
    /// Version de la crate.
    pub version: &'static str,
    /// Vrai quand l'index du VFS est prêt : sans lui, le catalogue est vide.
    pub vfs_pret: bool,
    /// Nombre de `.lua.bin` indexés, **compté** sur le VFS.
    pub scripts: usize,
    /// Somme des tailles déclarées, en octets.
    pub octets: u64,
    /// Suffixe reconnu.
    pub suffixe: &'static str,
    /// Dialecte attendu par le décodeur.
    pub dialecte: &'static str,
    /// Vrai si un interpréteur Lua est lié dans ce processus — cf. [`VM_LIEE`].
    pub vm_liee: bool,
    /// Ce que la couche sait faire ici, et ce qu'elle refuse.
    pub capacites: Vec<Capacite>,
}

/// La liste des capacités, servie et refusée. Figée : c'est un contrat, pas une mesure.
#[must_use]
pub fn capacites_liste() -> Vec<Capacite> {
    vec![
        Capacite {
            nom: "catalogue",
            etat: "servi",
            route: Some("/api/v1/lua/scripts"),
            raison: None,
        },
        Capacite {
            nom: "analyse_statique",
            etat: "servi",
            route: Some("/api/v1/lua/scripts/{chemin}"),
            raison: None,
        },
        Capacite {
            nom: "chunk_integral",
            etat: "servi",
            route: Some("/api/v1/lua/scripts/{chemin}?forme=chunk"),
            raison: None,
        },
        Capacite {
            nom: "desassemblage",
            etat: "servi",
            route: Some("/api/v1/lua/desassemblage/{chemin}"),
            raison: None,
        },
        Capacite {
            nom: "execution",
            etat: "refuse",
            route: None,
            raison: Some(
                "charger un chunk de bytecode est une primitive d'execution, pas un decodage",
            ),
        },
        Capacite {
            nom: "pilotage_de_menu",
            etat: "refuse",
            route: None,
            raison: Some("drive_menu/run_menu font tourner le script dans une VM"),
        },
        Capacite {
            nom: "onglets_d_entete",
            etat: "refuse",
            route: None,
            raison: Some("enumerate_header_tabs appelle les fonctions du script"),
        },
        Capacite {
            nom: "surface_d_api_hote",
            etat: "refuse",
            route: None,
            raison: Some("discover_host_calls execute la fonction principale du script"),
        },
    ]
}

/// `GET /api/v1/lua` — ce que cette machine sait faire du Lua du jeu, **mesuré**.
pub async fn capacites(State(etat): State<EtatSite>) -> Json<CapacitesLua> {
    let index = etat.index().ok();
    let (scripts, octets) = match index.as_ref() {
        Some(i) => {
            let liste = catalogue(i);
            (
                liste.len(),
                liste.iter().map(|s| u64::from(s.octets)).sum::<u64>(),
            )
        }
        None => (0, 0),
    };
    Json(CapacitesLua {
        service: crate::SERVICE,
        version: crate::VERSION,
        vfs_pret: index.is_some(),
        scripts,
        octets,
        suffixe: SUFFIXE,
        dialecte: "Lua 5.2 PUC-Rio (bytecode, petit-boutiste)",
        vm_liee: VM_LIEE,
        capacites: capacites_liste(),
    })
}

/// `GET /api/v1/lua/scripts` — une page du catalogue des scripts.
///
/// # Errors
///
/// `Indisponible` tant que l'index du VFS n'est pas monté.
pub async fn scripts(
    State(etat): State<EtatSite>,
    Query(demande): Query<DemandePage>,
) -> Result<Json<Page<Script>>, ErreurSite> {
    let index = etat.index()?;
    let p = demande.bornee();
    let motif = demande
        .q
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_lowercase);

    let liste = tokio::task::spawn_blocking(move || catalogue(&index)).await?;
    let filtres: Vec<&Script> = match motif.as_deref() {
        None => liste.iter().collect(),
        Some(m) => liste
            .iter()
            .filter(|s| s.chemin.to_lowercase().contains(m))
            .collect(),
    };
    let total = filtres.len();
    let elements = filtres
        .into_iter()
        .skip(p.offset())
        .take(p.per_page as usize)
        .cloned()
        .collect();
    Ok(Json(Page::nouvelle(elements, p, total)))
}

// ─── L'analyse statique ─────────────────────────────────────────────────────────────────────

/// En-tête d'un chunk, tel qu'il sort du décodeur.
#[derive(Debug, Clone, Serialize)]
pub struct Entete {
    /// Version encodée (`82` = `0x52` = Lua 5.2).
    pub version: u8,
    /// Format officiel (0).
    pub format: u8,
    /// `petit` ou `gros`.
    pub boutisme: &'static str,
    /// Taille d'un `int` C, en octets.
    pub taille_int: u8,
    /// Taille d'un `size_t` C, en octets.
    pub taille_size_t: u8,
    /// Taille d'une instruction, en octets.
    pub taille_instruction: u8,
    /// Taille d'un `lua_Number`, en octets.
    pub taille_nombre: u8,
    /// Vrai si les nombres sont entiers (build exotique).
    pub nombres_entiers: bool,
}

impl From<&nie_lua::bytecode::Header> for Entete {
    fn from(h: &nie_lua::bytecode::Header) -> Self {
        Self {
            version: h.version,
            format: h.format,
            // `format!("{:?}")` n'est pas une serialisation : le boutisme sort en jeton choisi.
            boutisme: if h.little_endian { "petit" } else { "gros" },
            taille_int: h.size_int,
            taille_size_t: h.size_size_t,
            taille_instruction: h.size_instruction,
            taille_nombre: h.size_number,
            nombres_entiers: h.number_is_integral,
        }
    }
}

/// Un nom global et le nombre d'accès statiques qui le visent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Global {
    /// Le nom, tel qu'il figure au pool de constantes.
    pub nom: String,
    /// Nombre d'instructions qui l'atteignent.
    pub occurrences: usize,
}

/// Corps de `/api/v1/lua/scripts/{*chemin}`.
#[derive(Debug, Clone, Serialize)]
pub struct Analyse {
    /// Chemin VFS analysé.
    pub chemin: String,
    /// Taille du fichier, en octets.
    pub octets: usize,
    /// En-tête décodé.
    pub entete: Entete,
    /// Nom de source du chunk principal, vide quand le chunk est dépouillé.
    pub source: String,
    /// Vrai quand la table de débogage est présente (lignes, locales, upvalues nommées).
    pub debogage: bool,
    /// Nombre de prototypes, chunk principal compris.
    pub prototypes: usize,
    /// Nombre total d'instructions.
    pub instructions: usize,
    /// Nombre total de constantes, tous prototypes confondus.
    pub constantes: usize,
    /// Globaux **lus** (`GETTABUP` sur `_ENV`), par occurrences décroissantes.
    pub globaux_lus: Vec<Global>,
    /// Globaux **écrits** (`SETTABUP` sur `_ENV`), par occurrences décroissantes.
    pub globaux_ecrits: Vec<Global>,
    /// Histogramme des opcodes, par occurrences décroissantes.
    pub opcodes: Vec<Global>,
    /// L'arbre des prototypes, aplati, chacun repéré par son chemin d'indices (`main`,
    /// `main/0`, `main/0/2`…). C'est la structure du script : une fonction Lua par ligne.
    pub arbre: Vec<ProtoResume>,
    /// Les chaînes du pool de constantes, dédoublonnées et triées — noms de fonctions hôtes,
    /// clés de table, libellés. Bornées par [`CHAINES_MAX`].
    pub chaines: Vec<String>,
    /// Nombre de chaînes distinctes avant bornage : dire qu'on a coupé, plutôt que de laisser
    /// croire que la liste est complète.
    pub chaines_total: usize,
}

/// Nombre maximal de chaînes rendues par [`Analyse`].
pub const CHAINES_MAX: usize = 512;

/// Un prototype de fonction, résumé. Chaque champ vient directement du décodeur — rien n'est
/// déduit ni approché.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProtoResume {
    /// Chemin d'indices dans l'arbre (`main`, `main/0`, `main/0/2`…).
    pub chemin: String,
    /// Ligne de début dans la source d'origine (0 quand le chunk est dépouillé).
    pub ligne_debut: u32,
    /// Ligne de fin.
    pub ligne_fin: u32,
    /// Nombre de paramètres fixes.
    pub parametres: u8,
    /// Vrai si la fonction est variadique.
    pub variadique: bool,
    /// Taille de pile requise.
    pub pile: u8,
    /// Nombre d'instructions de ce prototype seul.
    pub instructions: usize,
    /// Nombre de constantes de son pool.
    pub constantes: usize,
    /// Nombre d'upvalues.
    pub upvalues: usize,
    /// Noms des upvalues, vides quand le chunk est dépouillé.
    pub upvalues_nommees: Vec<String>,
    /// Nombre de variables locales déclarées (table de débogage).
    pub locales: usize,
    /// Nombre de fonctions imbriquées directes.
    pub enfants: usize,
}

/// Le compteur qui traverse les prototypes.
#[derive(Default)]
struct Compteur {
    prototypes: usize,
    instructions: usize,
    constantes: usize,
    lus: BTreeMap<String, usize>,
    ecrits: BTreeMap<String, usize>,
    opcodes: BTreeMap<String, usize>,
    debogage: bool,
    arbre: Vec<ProtoResume>,
    chaines: std::collections::BTreeSet<String>,
}

/// Dit si l'upvalue `b` d'un prototype est `_ENV`, c'est-à-dire la table des globaux.
///
/// Quand la table de débogage est présente, le nom tranche. Quand le chunk est dépouillé —
/// c'est le cas des scripts du jeu — l'upvalue 0 du chunk principal est `_ENV` par
/// construction (`lua_load` l'y installe), et les fonctions imbriquées héritent de la même
/// position dans l'immense majorité des cas. On le dit plutôt que de le taire.
fn est_env(p: &Prototype, b: u32) -> bool {
    match p.upvalue_names.get(b as usize) {
        Some(nom) => nom == "_ENV",
        None => p.upvalue_names.is_empty() && b == 0,
    }
}

/// Rend la constante visée par un opérande RK, ou `None` si l'opérande est un registre.
///
/// `BITRK` vaut `1 << 8` en Lua 5.2 : au-delà, l'opérande indexe le pool de constantes.
fn rk(p: &Prototype, operande: u32) -> Option<&Constant> {
    const BITRK: u32 = 1 << 8;
    if operande >= BITRK {
        p.constants.get((operande - BITRK) as usize)
    } else {
        None
    }
}

/// Rend le nom de constante chaîne visé par un opérande RK.
fn nom_rk(p: &Prototype, operande: u32) -> Option<String> {
    match rk(p, operande)? {
        // Les scripts portent des libellés non-UTF-8 (japonais hérité) : on convertit en
        // remplaçant, jamais en rejetant — un nom de global, lui, est toujours ASCII.
        Constant::String(octets) => Some(String::from_utf8_lossy(octets).into_owned()),
        _ => None,
    }
}

impl Compteur {
    fn visiter(&mut self, p: &Prototype, chemin: &str) {
        self.prototypes += 1;
        self.arbre.push(ProtoResume {
            chemin: chemin.to_owned(),
            ligne_debut: p.line_defined,
            ligne_fin: p.last_line_defined,
            parametres: p.num_params,
            variadique: p.is_vararg != 0,
            pile: p.max_stack_size,
            instructions: p.code.len(),
            constantes: p.constants.len(),
            upvalues: p.upvalues.len(),
            upvalues_nommees: p.upvalue_names.clone(),
            locales: p.loc_vars.len(),
            enfants: p.protos.len(),
        });
        for k in &p.constants {
            if let Constant::String(octets) = k {
                let s = String::from_utf8_lossy(octets);
                let s = s.trim_end_matches('\0');
                if !s.is_empty() {
                    self.chaines.insert(s.to_owned());
                }
            }
        }
        self.instructions += p.code.len();
        self.constantes += p.constants.len();
        if !p.line_info.is_empty() || !p.loc_vars.is_empty() || !p.upvalue_names.is_empty() {
            self.debogage = true;
        }
        for raw in &p.code {
            let ins = nie_lua::bytecode::decode_instruction(*raw);
            let nom = ins.name();
            *self.opcodes.entry(nom.clone()).or_insert(0) += 1;
            match nom.as_str() {
                // GETTABUP A B C : R(A) := UpValue[B][RK(C)] — une lecture de global quand
                // l'upvalue est `_ENV`.
                "GETTABUP" if est_env(p, ins.b) => {
                    if let Some(n) = nom_rk(p, ins.c) {
                        *self.lus.entry(n).or_insert(0) += 1;
                    }
                }
                // SETTABUP A B C : UpValue[A][RK(B)] := RK(C) — l'upvalue est en A, pas en B.
                "SETTABUP" if est_env(p, ins.a) => {
                    if let Some(n) = nom_rk(p, ins.b) {
                        *self.ecrits.entry(n).or_insert(0) += 1;
                    }
                }
                _ => {}
            }
        }
        for (i, enfant) in p.protos.iter().enumerate() {
            self.visiter(enfant, &format!("{chemin}/{i}"));
        }
    }
}

/// Trie une table de comptes par occurrences décroissantes, puis par nom.
fn classer(table: BTreeMap<String, usize>) -> Vec<Global> {
    let mut v: Vec<Global> = table
        .into_iter()
        .map(|(nom, occurrences)| Global { nom, occurrences })
        .collect();
    v.sort_by(|a, b| {
        b.occurrences
            .cmp(&a.occurrences)
            .then_with(|| a.nom.cmp(&b.nom))
    });
    v
}

/// Analyse un chunk déjà décodé. Séparée du handler pour être testable sans HTTP ni VFS.
#[must_use]
pub fn analyser(chemin: &str, octets: usize, chunk: &Chunk) -> Analyse {
    let mut c = Compteur::default();
    c.visiter(&chunk.main, "main");
    let chaines_total = c.chaines.len();
    let chaines: Vec<String> = c.chaines.into_iter().take(CHAINES_MAX).collect();
    Analyse {
        chemin: chemin.to_owned(),
        octets,
        entete: Entete::from(&chunk.header),
        source: chunk.main.source.clone(),
        debogage: c.debogage,
        prototypes: c.prototypes,
        instructions: c.instructions,
        constantes: c.constantes,
        globaux_lus: classer(c.lus),
        globaux_ecrits: classer(c.ecrits),
        opcodes: classer(c.opcodes),
        arbre: c.arbre,
        chaines,
        chaines_total,
    }
}

/// Résout un chemin de script : garde de traversée, présence dans l'index, suffixe attendu.
///
/// La garde de traversée est celle de [`super::vfs::normaliser`] — il n'y en a qu'une dans la
/// crate, et c'est ce qui fait qu'un `..` refusé sur `/f` l'est aussi ici.
///
/// # Errors
///
/// `Demande` sur chemin sortant ou suffixe inattendu, `Introuvable` sur chemin absent de
/// l'index, `Indisponible` tant que le VFS n'est pas monté.
fn resoudre(etat: &EtatSite, brut: &str) -> Result<String, ErreurSite> {
    let chemin = super::vfs::normaliser(brut)?;
    if !chemin.ends_with(SUFFIXE) {
        return Err(ErreurSite::Demande(format!(
            "ce n'est pas un script du jeu (suffixe attendu: {SUFFIXE})"
        )));
    }
    let index = etat.index()?;
    if !index.contient(&chemin) {
        return Err(ErreurSite::Introuvable(format!(
            "chemin absent du VFS: {chemin}"
        )));
    }
    Ok(chemin)
}

/// Lit les octets d'un script, bornés par [`TAILLE_MAX`].
async fn octets_script(etat: &EtatSite, chemin: &str) -> Result<Vec<u8>, ErreurSite> {
    let vfs = etat.vfs()?;
    let a_lire = chemin.to_owned();
    let octets = tokio::task::spawn_blocking(move || vfs.read(&a_lire))
        .await?
        .map_err(|e| {
            tracing::debug!(erreur = %e, "lecture VFS impossible");
            ErreurSite::Introuvable("script indexe mais illisible sur ce montage".to_owned())
        })?;
    if octets.len() > TAILLE_MAX {
        return Err(ErreurSite::Demande(format!(
            "script trop volumineux pour l'analyse ({} octets, borne {TAILLE_MAX})",
            octets.len()
        )));
    }
    Ok(octets)
}

/// Décode un chunk, ou dit pourquoi ces octets n'en sont pas un.
fn decoder(octets: &[u8]) -> Result<Chunk, ErreurSite> {
    if !nie_lua::is_lua52_bytecode(octets) {
        return Err(ErreurSite::Demande(
            "ces octets ne portent pas la signature d'un bytecode Lua 5.2".to_owned(),
        ));
    }
    nie_lua::bytecode::parse(octets)
        .map_err(|e| ErreurSite::Demande(format!("bytecode illisible: {e}")))
}

/// Ce que `/api/v1/lua/scripts/{*chemin}` doit rendre.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DemandeScript {
    /// `analyse` (défaut) ou `chunk`.
    pub forme: Option<String>,
}

/// Les deux formes servies, et ce qui les distingue.
///
/// `analyse` est un résumé : il répond « que fait ce script, et de quoi a-t-il besoin ».
/// `chunk` est le **décodage intégral** rendu par `nie_lua::bytecode::parse` — en-tête,
/// prototypes imbriqués, pool de constantes, mots d'instruction bruts, table de débogage.
/// C'est tout ce que la crate sait tirer de ces octets, sans rien retrancher ; les types de
/// `nie-lua` dérivent `Serialize` sans condition, donc rien n'est reconstruit ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forme {
    /// Le résumé statique ([`Analyse`]).
    Analyse,
    /// Le chunk entier, tel que le décodeur le rend.
    Chunk,
}

impl Forme {
    /// Reconnaît une forme, ou dit lesquelles existent.
    ///
    /// # Errors
    ///
    /// `Demande` sur une forme inconnue.
    pub fn depuis(s: Option<&str>) -> Result<Self, ErreurSite> {
        match s.map(str::trim).filter(|f| !f.is_empty()) {
            None | Some("analyse") => Ok(Self::Analyse),
            Some("chunk") => Ok(Self::Chunk),
            Some(autre) => Err(ErreurSite::Demande(format!(
                "forme inconnue: {autre} (connues: analyse, chunk)"
            ))),
        }
    }
}

/// `GET /api/v1/lua/scripts/{*chemin}` — un script décodé.
///
/// `?forme=analyse` (défaut) rend le résumé statique ; `?forme=chunk` rend le décodage
/// intégral. Les deux passent par le même `parse` : la seconde ne fait que ne rien jeter.
///
/// # Errors
///
/// `Demande` sur chemin sortant, forme inconnue, suffixe inattendu ou bytecode illisible ;
/// `Introuvable` sur chemin absent ; `Indisponible` tant que le VFS n'est pas monté.
pub async fn script(
    State(etat): State<EtatSite>,
    Path(brut): Path<String>,
    Query(demande): Query<DemandeScript>,
) -> Result<Json<serde_json::Value>, ErreurSite> {
    let forme = Forme::depuis(demande.forme.as_deref())?;
    let chemin = resoudre(&etat, &brut)?;
    let octets = octets_script(&etat, &chemin).await?;

    let _jeton = jeton_analyse().await?;
    let pour_tache = chemin.clone();
    let corps = tokio::task::spawn_blocking(move || {
        let chunk = decoder(&octets)?;
        let v = match forme {
            Forme::Analyse => serde_json::to_value(analyser(&pour_tache, octets.len(), &chunk)),
            Forme::Chunk => serde_json::to_value(&chunk).map(
                |c| serde_json::json!({ "chemin": pour_tache, "octets": octets.len(), "chunk": c }),
            ),
        }
        .map_err(|e| ErreurSite::Interne(format!("reponse non serialisable: {e}")))?;
        Ok::<_, ErreurSite>(v)
    })
    .await??;
    Ok(Json(corps))
}

/// `GET /api/v1/lua/desassemblage/{*chemin}` — le listing du chunk, en texte.
///
/// # Errors
///
/// Les mêmes que [`script`].
pub async fn desassemblage(
    State(etat): State<EtatSite>,
    Path(brut): Path<String>,
) -> Result<Response, ErreurSite> {
    let chemin = resoudre(&etat, &brut)?;
    let octets = octets_script(&etat, &chemin).await?;

    let _jeton = jeton_analyse().await?;
    let listing = tokio::task::spawn_blocking(move || {
        let chunk = decoder(&octets)?;
        Ok::<_, ErreurSite>(nie_lua::bytecode::disassemble(&chunk))
    })
    .await??;

    let mut reponse = listing.into_response();
    let entetes = reponse.headers_mut();
    entetes.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    entetes.insert(header::CACHE_CONTROL, HeaderValue::from_static(CONTROLE));
    Ok(reponse)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un chunk Lua 5.2 minimal, fabriqué octet par octet : `return`.
    ///
    /// Aucun fichier du jeu n'entre dans les tests — les `.lua.bin` sont © LEVEL-5 et le VFS
    /// n'est pas monté sous `cargo test`. Le chunk est donc construit ici, ce qui a l'avantage
    /// de rendre les attentes exactes plutôt qu'approximatives.
    fn chunk_minimal() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"\x1bLua"); // signature
        v.push(0x52); // version 5.2
        v.push(0); // format officiel
        v.push(1); // petit-boutiste
        v.push(4); // sizeof(int)
        v.push(8); // sizeof(size_t)
        v.push(4); // sizeof(Instruction)
        v.push(8); // sizeof(lua_Number)
        v.push(0); // nombres flottants
        v.extend_from_slice(&[0x19, 0x93, b'\r', b'\n', 0x1a, b'\n']); // LUAC_TAIL
        // Prototype principal, dépouillé.
        v.extend_from_slice(&0u32.to_le_bytes()); // line_defined
        v.extend_from_slice(&0u32.to_le_bytes()); // last_line_defined
        v.push(0); // num_params
        v.push(1); // is_vararg
        v.push(2); // max_stack_size
        // code : un GETTABUP (lecture du global constante 0), puis RETURN.
        v.extend_from_slice(&2u32.to_le_bytes());
        // GETTABUP A=0 B=0 C=256 → opcode 6 | A<<6 | C<<14 | B<<23
        let gettabup: u32 = 6 | (256 << 14);
        v.extend_from_slice(&gettabup.to_le_bytes());
        // RETURN A=0 B=1 → opcode 31
        let retour: u32 = 31 | (1 << 23);
        v.extend_from_slice(&retour.to_le_bytes());
        // constantes : une chaîne "print"
        v.extend_from_slice(&1u32.to_le_bytes());
        v.push(4); // LUA_TSTRING
        v.extend_from_slice(&6u64.to_le_bytes()); // longueur, terminateur compris
        v.extend_from_slice(b"print\0");
        v.extend_from_slice(&0u32.to_le_bytes()); // protos imbriqués
        v.extend_from_slice(&1u32.to_le_bytes()); // upvalues : 1
        v.push(1); // in_stack
        v.push(0); // index
        v.extend_from_slice(&0u64.to_le_bytes()); // source (vide)
        v.extend_from_slice(&0u32.to_le_bytes()); // line_info
        v.extend_from_slice(&0u32.to_le_bytes()); // loc_vars
        v.extend_from_slice(&0u32.to_le_bytes()); // upvalue_names
        v
    }

    #[test]
    fn la_signature_est_verifiee_avant_le_parse() {
        let e = decoder(b"pas du lua").unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
    }

    #[test]
    fn l_analyse_extrait_le_global_lu() {
        let octets = chunk_minimal();
        let chunk = decoder(&octets).expect("chunk minimal decodable");
        let a = analyser("data/x.lua.bin", octets.len(), &chunk);
        assert_eq!(a.entete.version, 0x52);
        assert_eq!(a.entete.boutisme, "petit");
        assert_eq!(a.prototypes, 1);
        assert_eq!(a.instructions, 2);
        assert_eq!(
            a.globaux_lus,
            vec![Global {
                nom: "print".to_owned(),
                occurrences: 1
            }]
        );
        assert!(a.globaux_ecrits.is_empty());
        assert!(a.opcodes.iter().any(|o| o.nom == "RETURN"));
    }

    #[test]
    fn les_deux_formes_sont_reconnues_et_les_autres_refusees() {
        assert_eq!(Forme::depuis(None).unwrap(), Forme::Analyse);
        assert_eq!(Forme::depuis(Some("")).unwrap(), Forme::Analyse);
        assert_eq!(Forme::depuis(Some("chunk")).unwrap(), Forme::Chunk);
        assert_eq!(
            Forme::depuis(Some("brut")).unwrap_err().statut().as_u16(),
            400
        );
    }

    /// La forme `chunk` ne retranche rien : tout ce que le décodeur rend est sérialisable, et
    /// on le vérifie plutôt que de le supposer (un champ non sérialisable donnerait un 500).
    #[test]
    fn la_forme_chunk_serialise_tout_le_decodage() {
        let octets = chunk_minimal();
        let chunk = decoder(&octets).expect("chunk minimal decodable");
        let v = serde_json::to_value(&chunk).expect("chunk serialisable");
        assert_eq!(v["header"]["version"], 0x52);
        assert_eq!(v["main"]["code"].as_array().map(Vec::len), Some(2));
        assert_eq!(v["main"]["constants"].as_array().map(Vec::len), Some(1));
        assert!(v["main"]["upvalues"].is_array());
    }

    /// L'arbre des prototypes et le pool de chaînes sont l'autre moitié de ce que le décodeur
    /// sait dire : un résumé qui les tairait laisserait le client redemander la forme `chunk`.
    #[test]
    fn l_analyse_rend_l_arbre_et_les_chaines() {
        let octets = chunk_minimal();
        let chunk = decoder(&octets).expect("chunk minimal decodable");
        let a = analyser("data/x.lua.bin", octets.len(), &chunk);
        assert_eq!(a.arbre.len(), 1);
        assert_eq!(a.arbre[0].chemin, "main");
        assert!(a.arbre[0].variadique);
        assert_eq!(a.arbre[0].upvalues, 1);
        assert_eq!(a.arbre[0].instructions, 2);
        assert_eq!(a.chaines, vec!["print".to_owned()]);
        assert_eq!(a.chaines_total, 1);
    }

    #[test]
    fn le_desassemblage_cite_l_opcode() {
        let octets = chunk_minimal();
        let chunk = decoder(&octets).expect("chunk minimal decodable");
        let texte = nie_lua::bytecode::disassemble(&chunk);
        assert!(texte.contains("GETTABUP"), "{texte}");
    }

    /// La garde de traversée est celle de `routes::vfs` : on vérifie qu'elle est bien branchée
    /// ici, pas qu'elle fonctionne (elle a ses propres tests).
    #[test]
    fn le_suffixe_et_la_traversee_sont_refuses_avant_toute_lecture() {
        let etat = crate::state::EtatSite::pour_tests(
            crate::config::Config::default(),
            IndexVfs::depuis(vec![("data/common/script/lua/a.lua.bin".to_owned(), 10)]),
        );
        assert_eq!(
            resoudre(&etat, "../etc/passwd.lua.bin")
                .unwrap_err()
                .statut()
                .as_u16(),
            400
        );
        assert_eq!(
            resoudre(&etat, "data/common/script/lua/a.cfg.bin")
                .unwrap_err()
                .statut()
                .as_u16(),
            400
        );
        assert_eq!(
            resoudre(&etat, "data/common/script/lua/absent.lua.bin")
                .unwrap_err()
                .statut()
                .as_u16(),
            404
        );
        assert_eq!(
            resoudre(&etat, "data/common/script/lua/a.lua.bin").unwrap(),
            "data/common/script/lua/a.lua.bin"
        );
    }

    #[test]
    fn le_balayage_ne_retient_que_les_scripts() {
        let index = IndexVfs::depuis(vec![
            ("data/common/script/lua/menu/a.lua.bin".to_owned(), 12),
            ("data/common/script/lua/menu/b.cfg.bin".to_owned(), 34),
            ("data/common/gamedata/soccer/c.lua.bin".to_owned(), 56),
        ]);
        let s = balayer(&index);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].chemin, "data/common/gamedata/soccer/c.lua.bin");
        assert_eq!(s[0].nom, "c.lua.bin");
        assert_eq!(s[1].octets, 12);
    }

    /// Le contrat qui compte le plus de ce module : aucune capacité d'exécution n'est servie.
    #[test]
    fn aucune_execution_n_est_exposee() {
        let liste = capacites_liste();
        for interdite in [
            "execution",
            "pilotage_de_menu",
            "onglets_d_entete",
            "surface_d_api_hote",
        ] {
            let c = liste
                .iter()
                .find(|c| c.nom == interdite)
                .expect("capacite declaree");
            assert_eq!(c.etat, "refuse", "{interdite}");
            assert!(c.route.is_none(), "{interdite} ne doit porter aucune route");
            assert!(c.raison.is_some(), "{interdite} doit dire pourquoi");
        }
        // Vérifié à la COMPILATION : si `nie-lua` reprenait un jour ses features par défaut et
        // que quelqu'un basculait `VM_LIEE`, le crate ne construirait plus.
        const { assert!(!VM_LIEE) };
    }
}
