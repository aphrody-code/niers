//! Index des chemins du VFS, et les **filtres enregistrés** qui s'y appliquent.
//!
//! Amendement A3 : l'URL d'une ressource est son chemin de jeu verbatim. Les vues nommées
//! (`textures`, `modeles`, `sons`, `videos`) ne sont pas des dossiers ni des tables : ce sont
//! des **filtres enregistrés** sur l'espace `/f` et `/b`, définis par un jeu d'extensions.
//! Elles n'inventent aucun identifiant et ne renomment rien.
//!
//! L'index est une liste de chemins **triée**, ce qui rend le parcours d'un dossier
//! (`/b/<préfixe>`) équivalent à une recherche dichotomique suivie d'un balayage de plage — au
//! lieu d'un parcours des 255 000 entrées à chaque requête. Les vues sont pré-calculées à la
//! construction : servir `/api/v1/textures?page=1` ne coûte alors qu'une tranche de `Vec`.
//!
//! # Les listes secondaires — une seule passe, pas une seconde
//!
//! Les quatre vues ne couvrent que 143 246 des 255 308 entrées : 112 062 fichiers (`.bin`,
//! `.p3lip`, `.objbin`…) n'étaient atteignables que par le parcours, sans aucun filtre. La même
//! boucle qui range les vues range désormais aussi, **sans passe supplémentaire** :
//!
//! - une liste d'indices par **extension** (`ext_listes`) ;
//! - une liste d'indices par **CPK d'origine** (`cpk_listes`), le nom de pack étant interné en
//!   `u16` — 936 valeurs distinctes, donc quelques kilooctets plutôt que 255 308 `String` ;
//! - l'ordre **par taille** (`par_taille`), une permutation d'indices calculée une fois.
//!
//! Le surcoût est en mémoire (~1 Mio par `Vec<u32>`), pas en temps : toutes les listes sont
//! croissantes, donc testables par dichotomie, et un tri par taille se sert de la permutation
//! déjà calculée au lieu de retrier à chaque requête.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

/// Une vue nommée : un filtre enregistré sur l'espace VFS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Vue {
    /// Textures du jeu (`.g4tx`, `.dds`, `.png`).
    Textures,
    /// Modèles et leurs pièces (`.g4md`, `.g4mg`, `.g4sk`, `.g4mt`, `.g4pk`).
    Modeles,
    /// Sons et conteneurs audio CRI (`.acb`, `.awb`, `.hca`, `.adx`, `.wav`).
    Sons,
    /// Vidéos (`.usm`, `.mp4`, `.webm`).
    Videos,
}

/// Les quatre vues, dans l'ordre où elles sont exposées sous `/api/v1/`.
pub const VUES: [Vue; 4] = [Vue::Textures, Vue::Modeles, Vue::Sons, Vue::Videos];

/// Segment réservé qui désigne **tout** l'espace VFS plutôt qu'une vue.
///
/// Ce n'est pas une vue de plus : c'est le seul moyen d'atteindre les 112 062 fichiers que les
/// quatre filtres enregistrés ne retiennent pas, en le combinant à `?ext=` ou `?q=`.
pub const SEGMENT_TOUT: &str = "tout";

impl Vue {
    /// Segment d'URL de la vue (`textures`, `modeles`, `sons`, `videos`).
    #[must_use]
    pub fn segment(self) -> &'static str {
        match self {
            Self::Textures => "textures",
            Self::Modeles => "modeles",
            Self::Sons => "sons",
            Self::Videos => "videos",
        }
    }

    /// Extensions retenues par le filtre, en minuscules et sans point.
    #[must_use]
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Textures => &["g4tx", "dds", "png"],
            Self::Modeles => &["g4md", "g4mg", "g4sk", "g4mt", "g4pk", "g4pkm"],
            Self::Sons => &["acb", "awb", "hca", "adx", "wav"],
            Self::Videos => &["usm", "mp4", "webm"],
        }
    }

    /// Reconnaît une vue depuis son segment d'URL.
    #[must_use]
    pub fn depuis_segment(s: &str) -> Option<Self> {
        VUES.into_iter().find(|v| v.segment() == s)
    }

    /// Dit si un chemin VFS entre dans le filtre.
    #[must_use]
    pub fn retient(self, chemin: &str) -> bool {
        let Some(ext) = extension(chemin) else {
            return false;
        };
        self.extensions().iter().any(|e| ext.eq_ignore_ascii_case(e))
    }
}

/// Extension d'un chemin VFS, sans point, telle qu'elle est écrite dans le jeu.
///
/// Elle se lit sur la **feuille** : un point dans un nom de dossier ne fait pas une extension,
/// et `data/x.y/z` n'a pas d'extension.
#[must_use]
pub fn extension(chemin: &str) -> Option<&str> {
    let feuille = chemin.rsplit('/').next().unwrap_or(chemin);
    let (base, ext) = feuille.rsplit_once('.')?;
    if base.is_empty() || ext.is_empty() {
        return None;
    }
    Some(ext)
}

/// Critère de tri d'une liste de fichiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tri {
    /// Ordre lexicographique du chemin VFS — c'est l'ordre naturel de l'index.
    #[default]
    Nom,
    /// Ordre des tailles déclarées. Les ex æquo restent en ordre de chemin.
    Taille,
}

impl Tri {
    /// Reconnaît un critère. Une valeur inconnue **n'est pas une erreur** : elle retombe sur
    /// [`Tri::Nom`], et la réponse annonce ce qui a réellement été appliqué.
    #[must_use]
    pub fn depuis(s: Option<&str>) -> Self {
        match s.map(str::trim).unwrap_or_default() {
            "taille" | "size" => Self::Taille,
            _ => Self::Nom,
        }
    }

    /// Nom public du critère. Un champ JSON se `match` vers une chaîne choisie — jamais un
    /// `format!("{:?}")`, qui publierait le nom Rust de la variante.
    #[must_use]
    pub fn nom(self) -> &'static str {
        match self {
            Self::Nom => "nom",
            Self::Taille => "taille",
        }
    }
}

/// Sens de tri.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ordre {
    /// Croissant.
    #[default]
    Asc,
    /// Décroissant.
    Desc,
}

impl Ordre {
    /// Reconnaît un sens. Inconnu ou absent : [`Ordre::Asc`].
    #[must_use]
    pub fn depuis(s: Option<&str>) -> Self {
        match s.map(str::trim).unwrap_or_default() {
            "desc" | "decroissant" | "descendant" => Self::Desc,
            _ => Self::Asc,
        }
    }

    /// Nom public du sens.
    #[must_use]
    pub fn nom(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Paramètres de filtrage et de tri acceptés en query par `/b` et par `/api/v1/{vue}`.
///
/// Aucun champ n'est obligatoire et aucune valeur n'est refusée : ce qui n'est pas reconnu est
/// ignoré ou borné, et la réponse porte un [`FiltresAppliques`] qui dit ce qui a compté.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DemandeFiltre {
    /// Motif glob du jeu (`glob=data/dx11/**,!**/movie/**`).
    ///
    /// La syntaxe est celle de [`nie_viola::Filtre`], c'est-à-dire celle des presets de dump,
    /// c'est-à-dire celle de `DumpService.GlobToRegex` côté IECODE : listes séparées par des
    /// virgules, `!` pour exclure (et l'exclusion prime), `**` traverse les `/`, `*` et `?` ne
    /// les traversent pas, le tout ancré et insensible à la casse. En écrire une seconde ici
    /// aurait donné deux syntaxes divergentes pour la même question.
    pub glob: Option<String>,
    /// Sous-arbre auquel restreindre la recherche (`prefixe=data/dx11/menu`).
    ///
    /// Sans lui, `?q=` cherche dans les 255 308 entrées et `/b?q=` ne regarde qu'un dossier :
    /// la question qu'on pose réellement — « ce motif, mais sous CE dossier » — n'avait aucune
    /// forme. Le préfixe est résolu par dichotomie sur l'index déjà trié, donc il **restreint**
    /// le travail au lieu de l'augmenter.
    pub prefixe: Option<String>,
    /// Extension exacte, sans point, insensible à la casse (`ext=bin`).
    pub ext: Option<String>,
    /// Nom du CPK d'origine (`cpk=common.cpk`). Sans effet sur un montage dump.
    pub cpk: Option<String>,
    /// Critère de tri : `nom` (défaut) ou `taille`.
    pub tri: Option<String>,
    /// Sens de tri : `asc` (défaut) ou `desc`.
    pub ordre: Option<String>,
    /// Taille minimale en octets, incluse.
    pub taille_min: Option<u32>,
    /// Taille maximale en octets, incluse.
    pub taille_max: Option<u32>,
}

/// Ce que le serveur a **réellement** appliqué à la demande.
///
/// Rendu dans chaque réponse : un filtre silencieusement ignoré est le pire des défauts, parce
/// que le client croit filtrer. Quand une valeur ne désigne rien (`ext_inconnue`, `cpk_inconnu`),
/// le résultat est vide **et le dit**, au lieu de rendre la liste entière.
#[derive(Debug, Clone, Serialize)]
pub struct FiltresAppliques {
    /// Motif appliqué au chemin entier, en minuscules.
    pub q: Option<String>,
    /// Extension appliquée, en minuscules.
    pub ext: Option<String>,
    /// CPK appliqué, tel qu'il est écrit dans l'index.
    pub cpk: Option<String>,
    /// Borne basse de taille appliquée.
    pub taille_min: Option<u32>,
    /// Borne haute de taille appliquée.
    pub taille_max: Option<u32>,
    /// Critère de tri appliqué (`nom` ou `taille`).
    pub tri: &'static str,
    /// Sens appliqué (`asc` ou `desc`).
    pub ordre: &'static str,
    /// Sous-arbre appliqué, normalisé avec sa barre finale.
    pub prefixe: Option<String>,
    /// Motif glob appliqué, tel qu'il a été compilé.
    pub glob: Option<String>,
    /// `true` quand `?glob=` a été demandé mais ne compile qu'en filtre vide (donc n'exclut
    /// rien) — dit plutôt que laissé croire.
    pub glob_vide: bool,
    /// `true` quand `?ext=` a été demandé mais n'existe nulle part dans l'index.
    pub ext_inconnue: bool,
    /// `true` quand `?cpk=` a été demandé mais ne désigne aucun pack indexé.
    pub cpk_inconnu: bool,
}

impl Default for FiltresAppliques {
    /// Le défaut n'est pas « rien » : c'est ce qui s'applique quand le client ne demande rien —
    /// tri par nom, ordre croissant. Un champ `tri: ""` mentirait sur ce qui a été fait.
    fn default() -> Self {
        Self {
            q: None,
            prefixe: None,
            glob: None,
            ext: None,
            cpk: None,
            taille_min: None,
            taille_max: None,
            tri: Tri::Nom.nom(),
            ordre: Ordre::Asc.nom(),
            glob_vide: false,
            ext_inconnue: false,
            cpk_inconnu: false,
        }
    }
}

/// Filtre résolu contre un index donné : les noms sont déjà normalisés et le CPK interné.
#[derive(Debug, Clone, Default)]
pub struct Filtre {
    q: Option<String>,
    /// Sous-arbre, normalisé (barre finale, jamais de barre initiale).
    prefixe: Option<String>,
    /// Sélecteur glob compilé **une fois** — le réinterpréter par chemin coûterait 255 308
    /// compilations, ce que le module de `nie-viola` documente comme son propre défaut d'avant.
    glob: Option<nie_viola::Filtre>,
    ext: Option<String>,
    cpk: Option<u16>,
    /// `true` quand un `?cpk=`/`?ext=` a été demandé sans correspondance : rien ne peut passer.
    impossible: bool,
    taille_min: Option<u32>,
    taille_max: Option<u32>,
}

impl Filtre {
    /// Dit si le filtre ne restreint rien — le cas courant, qu'on ne veut pas faire payer.
    #[must_use]
    pub fn est_vide(&self) -> bool {
        !self.impossible
            && self.q.is_none()
            && self.prefixe.is_none()
            && self.glob.is_none()
            && self.ext.is_none()
            && self.cpk.is_none()
            && self.taille_min.is_none()
            && self.taille_max.is_none()
    }
}

/// Tout ce qui règle une lecture de l'index : quoi retenir, dans quel ordre, quelle tranche.
///
/// C'est un **struct d'options** et non une liste de paramètres : trois énumérations et deux
/// entiers passés positionnellement donnent un appel illisible, et le filtre suivant y
/// ajouterait un argument de plus. Le chemin normal est [`IndexVfs::resoudre`], qui la
/// construit depuis la query ; [`Requete::paginer`] pose ensuite la tranche.
#[derive(Debug, Clone)]
pub struct Requete {
    /// Ce qui est retenu.
    pub filtre: Filtre,
    /// Ce qui sera annoncé au client — construit en même temps que `filtre`.
    pub applique: FiltresAppliques,
    /// Critère de tri.
    pub tri: Tri,
    /// Sens de tri.
    pub ordre: Ordre,
    /// Éléments sautés avant la page.
    pub offset: usize,
    /// Taille maximale de la page. `usize::MAX` rend tout ce qui reste.
    pub limite: usize,
}

impl Default for Requete {
    /// Aucun filtre, ordre naturel, et **tout** ce qui reste — jamais `limite = 0`, qui
    /// rendrait une page vide en annonçant un total non nul.
    fn default() -> Self {
        Self {
            filtre: Filtre::default(),
            applique: FiltresAppliques::default(),
            tri: Tri::Nom,
            ordre: Ordre::Asc,
            offset: 0,
            limite: usize::MAX,
        }
    }
}

impl Requete {
    /// Pose la tranche à rendre.
    #[must_use]
    pub fn paginer(mut self, offset: usize, limite: usize) -> Self {
        self.offset = offset;
        self.limite = limite;
        self
    }
}

/// Une entrée du VFS telle que l'index la retient : chemin, taille, et pack d'origine.
#[derive(Debug, Clone)]
pub struct Entree {
    /// Chemin VFS verbatim.
    pub chemin: String,
    /// Taille déclarée par l'index du jeu.
    pub taille: u32,
    /// Nom du CPK d'origine, vide sur un montage dump.
    pub cpk: String,
}

/// Une entrée de fichier telle qu'elle est rendue par `/b` et par les vues.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fichier {
    /// Chemin VFS verbatim — c'est aussi l'URL, sous `/f/`.
    pub chemin: String,
    /// Nom de la feuille, extension du jeu conservée.
    pub nom: String,
    /// Taille en octets telle que l'index du jeu la déclare.
    pub taille: u32,
    /// Pack d'origine, quand le montage en a un (`None` sur un dump extrait).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpk: Option<String>,
}

/// Contenu direct d'un préfixe : ses sous-dossiers et ses fichiers.
#[derive(Debug, Clone, Serialize)]
pub struct Dossier {
    /// Préfixe demandé, normalisé (sans slash de fin).
    pub prefixe: String,
    /// Sous-dossiers directs, chemins complets, triés.
    pub dossiers: Vec<String>,
    /// Fichiers directs de la page demandée.
    pub fichiers: Vec<Fichier>,
    /// Nombre total de fichiers directs **retenus par le filtre**, toutes pages confondues.
    pub total_fichiers: usize,
    /// Nombre de fichiers directs avant filtrage — ce que le dossier contient vraiment.
    pub total_fichiers_sans_filtre: usize,
    /// Ce qui a été appliqué à cette réponse.
    pub filtres: FiltresAppliques,
}

/// Un compte par facette : une valeur et le nombre de fichiers qui la portent.
#[derive(Debug, Clone, Serialize)]
pub struct Compte {
    /// Valeur de la facette (extension sans point, ou nom de CPK).
    pub valeur: String,
    /// Nombre de fichiers indexés qui la portent.
    pub total: usize,
}

/// Index trié des chemins du VFS, avec les vues, extensions, packs et tailles pré-calculés.
#[derive(Debug, Default)]
pub struct IndexVfs {
    chemins: Vec<String>,
    tailles: Vec<u32>,
    /// Rang du CPK d'origine dans `cpk_noms`, ou [`CPK_INCONNU`].
    cpk_de: Vec<u16>,
    /// Noms de CPK internés, triés.
    cpk_noms: Vec<String>,
    /// Indices de `chemins` par rang de CPK, croissants.
    cpk_listes: Vec<Vec<u32>>,
    /// Extensions distinctes, en minuscules, triées.
    ext_noms: Vec<String>,
    /// Indices de `chemins` par rang d'extension, croissants.
    ext_listes: Vec<Vec<u32>>,
    /// Indices dans `chemins`, une liste par vue, dans l'ordre de [`VUES`].
    vues: [Vec<u32>; 4],
    /// Tous les indices, croissants — la base des sélections non restreintes.
    tous: Vec<u32>,
    /// Permutation des indices par taille croissante, ex æquo en ordre de chemin.
    par_taille: Vec<u32>,
}

/// Rang réservé aux entrées sans pack d'origine (montage dump).
const CPK_INCONNU: u16 = u16::MAX;

impl IndexVfs {
    /// Construit l'index depuis des couples `(chemin, taille)`, sans pack d'origine.
    ///
    /// Conservé pour les appelants qui n'ont pas la provenance — les tests, et tout montage
    /// dump où `cpk_filename` est vide.
    #[must_use]
    pub fn depuis(entrees: Vec<(String, u32)>) -> Self {
        Self::depuis_entrees(
            entrees
                .into_iter()
                .map(|(chemin, taille)| Entree {
                    chemin,
                    taille,
                    cpk: String::new(),
                })
                .collect(),
        )
    }

    /// Construit l'index depuis des entrées complètes. Les doublons de chemin sont écartés.
    ///
    /// Le tri est fait ici une fois pour toutes ; toutes les lectures suivantes sont
    /// dichotomiques. Les listes secondaires (extension, CPK) sont remplies **dans la même
    /// boucle** que les vues : aucune seconde passe sur les 255 308 entrées.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn depuis_entrees(mut entrees: Vec<Entree>) -> Self {
        entrees.sort_unstable_by(|a, b| a.chemin.cmp(&b.chemin));
        entrees.dedup_by(|a, b| a.chemin == b.chemin);
        let n = entrees.len();

        let mut chemins = Vec::with_capacity(n);
        let mut tailles = Vec::with_capacity(n);
        let mut cpk_bruts = Vec::with_capacity(n);
        for e in entrees {
            chemins.push(e.chemin);
            tailles.push(e.taille);
            cpk_bruts.push(e.cpk);
        }

        // Les noms de CPK sont internés AVANT la boucle principale : 936 valeurs distinctes
        // pour 255 308 entrées, donc un `u16` par entrée au lieu d'une `String`.
        let mut cpk_noms: Vec<String> = {
            let mut vus: BTreeSet<&str> = BTreeSet::new();
            for c in &cpk_bruts {
                if !c.is_empty() {
                    vus.insert(c.as_str());
                }
            }
            vus.into_iter().map(str::to_owned).collect()
        };
        if cpk_noms.len() >= usize::from(CPK_INCONNU) {
            // Garde-fou : au-delà de 65 534 packs l'internement en `u16` ne tient plus. Aucun
            // montage connu n'en a plus de 936 ; on préfère perdre la facette que mentir.
            cpk_noms.clear();
        }
        let rang_cpk: HashMap<&str, u16> = cpk_noms
            .iter()
            .enumerate()
            .map(|(r, nom)| (nom.as_str(), u16::try_from(r).unwrap_or(CPK_INCONNU)))
            .collect();

        let mut cpk_de = Vec::with_capacity(n);
        let mut cpk_listes: Vec<Vec<u32>> = vec![Vec::new(); cpk_noms.len()];
        let mut vues: [Vec<u32>; 4] = [const { Vec::new() }; 4];
        let mut ext_map: HashMap<String, Vec<u32>> = HashMap::new();

        for (i, chemin) in chemins.iter().enumerate() {
            let idx = u32::try_from(i).unwrap_or(u32::MAX);
            for (rang, vue) in VUES.into_iter().enumerate() {
                if vue.retient(chemin) {
                    vues[rang].push(idx);
                }
            }
            if let Some(ext) = extension(chemin) {
                ext_map
                    .entry(ext.to_ascii_lowercase())
                    .or_default()
                    .push(idx);
            }
            let rang = rang_cpk
                .get(cpk_bruts[i].as_str())
                .copied()
                .unwrap_or(CPK_INCONNU);
            if rang != CPK_INCONNU {
                cpk_listes[usize::from(rang)].push(idx);
            }
            cpk_de.push(rang);
        }

        let mut ext_paires: Vec<(String, Vec<u32>)> = ext_map.into_iter().collect();
        ext_paires.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let mut ext_noms = Vec::with_capacity(ext_paires.len());
        let mut ext_listes = Vec::with_capacity(ext_paires.len());
        for (nom, liste) in ext_paires {
            ext_noms.push(nom);
            ext_listes.push(liste);
        }

        let tous: Vec<u32> = (0..u32::try_from(n).unwrap_or(u32::MAX)).collect();
        let mut par_taille = tous.clone();
        par_taille.sort_unstable_by_key(|i| (tailles[*i as usize], *i));

        Self {
            chemins,
            tailles,
            cpk_de,
            cpk_noms,
            cpk_listes,
            ext_noms,
            ext_listes,
            vues,
            tous,
            par_taille,
        }
    }

    /// Nombre de chemins indexés.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chemins.len()
    }

    /// Dit si l'index est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chemins.is_empty()
    }

    /// Nombre de chemins retenus par une vue.
    #[must_use]
    pub fn compte_vue(&self, vue: Vue) -> usize {
        self.vues[rang(vue)].len()
    }

    /// Histogramme des extensions, par nombre décroissant puis par nom.
    ///
    /// C'est la facette qui rend atteignables les 112 062 fichiers hors des quatre vues : sans
    /// elle, un client ne peut pas deviner que `.p3lip` existe.
    #[must_use]
    pub fn extensions(&self) -> Vec<Compte> {
        let mut v: Vec<Compte> = self
            .ext_noms
            .iter()
            .zip(&self.ext_listes)
            .map(|(nom, liste)| Compte {
                valeur: nom.clone(),
                total: liste.len(),
            })
            .collect();
        v.sort_unstable_by(|a, b| b.total.cmp(&a.total).then_with(|| a.valeur.cmp(&b.valeur)));
        v
    }

    /// Histogramme des CPK d'origine, par nombre décroissant puis par nom. Vide sur un dump.
    #[must_use]
    pub fn cpks(&self) -> Vec<Compte> {
        let mut v: Vec<Compte> = self
            .cpk_noms
            .iter()
            .zip(&self.cpk_listes)
            .map(|(nom, liste)| Compte {
                valeur: nom.clone(),
                total: liste.len(),
            })
            .collect();
        v.sort_unstable_by(|a, b| b.total.cmp(&a.total).then_with(|| a.valeur.cmp(&b.valeur)));
        v
    }

    /// Nombre d'extensions distinctes.
    #[must_use]
    pub fn nb_extensions(&self) -> usize {
        self.ext_noms.len()
    }

    /// Nombre de CPK distincts indexés.
    #[must_use]
    pub fn nb_cpks(&self) -> usize {
        self.cpk_noms.len()
    }

    /// Résout une demande de filtrage contre cet index.
    ///
    /// Rend le filtre exécutable **et** ce qui sera annoncé au client. Une extension ou un CPK
    /// qui ne désigne rien n'est pas une erreur : le filtre devient impossible, la réponse est
    /// vide, et `ext_inconnue`/`cpk_inconnu` le disent.
    #[must_use]
    pub fn resoudre(&self, q: Option<&str>, dem: &DemandeFiltre) -> Requete {
        let tri = Tri::depuis(dem.tri.as_deref());
        let ordre = Ordre::depuis(dem.ordre.as_deref());
        let mut f = Filtre::default();
        let mut a = FiltresAppliques {
            tri: tri.nom(),
            ordre: ordre.nom(),
            ..FiltresAppliques::default()
        };

        if let Some(m) = q.map(str::trim).filter(|m| !m.is_empty()) {
            let m = m.to_lowercase();
            a.q = Some(m.clone());
            f.q = Some(m);
        }

        if let Some(p) = dem
            .prefixe
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            // Normalisé une fois ici, jamais à chaque comparaison : `data/dx11` et
            // `data/dx11/` désignent le même sous-arbre, et sans la barre finale le préfixe
            // `data/dx1` attraperait `data/dx11` — un sous-arbre voisin, pas un descendant.
            let p = format!("{}/", p.trim_matches('/'));
            a.prefixe = Some(p.clone());
            f.prefixe = Some(p);
        }

        if let Some(g) = dem.glob.as_deref().map(str::trim).filter(|g| !g.is_empty()) {
            let compile = nie_viola::Filtre::parse(g);
            // Un motif fait uniquement de separateurs compile en filtre vide, qui accepte
            // tout : le republier comme applique laisserait croire a un filtre actif.
            if compile.est_vide() {
                a.glob_vide = true;
            } else {
                a.glob = Some(g.to_owned());
                f.glob = Some(compile);
            }
        }

        if let Some(e) = dem.ext.as_deref().map(str::trim).filter(|e| !e.is_empty()) {
            let e = e.trim_start_matches('.').to_ascii_lowercase();
            if self.rang_ext(&e).is_some() {
                a.ext = Some(e.clone());
                f.ext = Some(e);
            } else {
                a.ext = Some(e);
                a.ext_inconnue = true;
                f.impossible = true;
            }
        }

        if let Some(c) = dem.cpk.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
            match self.rang_cpk(c) {
                Some(r) => {
                    a.cpk = Some(self.cpk_noms[usize::from(r)].clone());
                    f.cpk = Some(r);
                }
                None => {
                    a.cpk = Some(c.to_owned());
                    a.cpk_inconnu = true;
                    f.impossible = true;
                }
            }
        }

        let (mut min, mut max) = (dem.taille_min, dem.taille_max);
        // Des bornes croisées ne sont pas une erreur : on les remet dans l'ordre et on le dit.
        if let (Some(a0), Some(b0)) = (min, max)
            && a0 > b0
        {
            min = Some(b0);
            max = Some(a0);
        }
        f.taille_min = min;
        f.taille_max = max;
        a.taille_min = min;
        a.taille_max = max;

        Requete {
            filtre: f,
            applique: a,
            tri,
            ordre,
            ..Requete::default()
        }
    }

    fn rang_ext(&self, ext: &str) -> Option<u16> {
        self.ext_noms
            .binary_search_by(|n| n.as_str().cmp(ext))
            .ok()
            .and_then(|r| u16::try_from(r).ok())
    }

    fn rang_cpk(&self, nom: &str) -> Option<u16> {
        // Le nom de pack est comparé sans casse : `Common.cpk` et `common.cpk` désignent le
        // même fichier sur les deux montages.
        self.cpk_noms
            .iter()
            .position(|n| n.eq_ignore_ascii_case(nom))
            .and_then(|r| u16::try_from(r).ok())
    }

    /// La liste d'indices la plus étroite qui contienne à coup sûr tous les résultats.
    ///
    /// Choisir la bonne base est ce qui rend le filtrage gratuit : `?ext=p3lip` balaie 21 047
    /// indices au lieu de 255 308, et les prédicats redondants restent vrais.
    fn base<'a>(&'a self, vue: Option<Vue>, f: &Filtre) -> &'a [u32] {
        if f.impossible {
            return &[];
        }
        if let Some(e) = f.ext.as_deref()
            && let Some(r) = self.rang_ext(e)
        {
            return &self.ext_listes[usize::from(r)];
        }
        if let Some(r) = f.cpk {
            return &self.cpk_listes[usize::from(r)];
        }
        // Le préfixe se résout par dichotomie sur l'index trié : `tous` est `0..n` dans l'ordre
        // de `chemins`, donc une tranche contigüe suffit. Elle n'est prise que faute de liste
        // pré-calculée plus étroite — celles d'extension et de pack ne sont pas triées par
        // chemin, et c'est `retenu` qui finit alors le travail.
        if vue.is_none()
            && let Some(p) = f.prefixe.as_deref()
        {
            let debut = self.chemins.partition_point(|c| c.as_str() < p);
            let fin = self.chemins[debut..].partition_point(|c| c.starts_with(p)) + debut;
            return &self.tous[debut..fin];
        }
        match vue {
            Some(v) => &self.vues[rang(v)],
            None => &self.tous,
        }
    }

    fn retenu(&self, i: u32, f: &Filtre) -> bool {
        // Un critère qui ne désigne rien ne laisse rien passer. La garde vit ICI et pas
        // seulement dans `base` : le parcours d'un dossier n'utilise pas les listes
        // pré-calculées, et sans elle `/b?ext=inexistante` rendait le dossier ENTIER en
        // annonçant `ext_inconnue` — le client croyait filtrer. Mesuré par curl le 2026-09-06.
        if f.impossible {
            return false;
        }
        let i = i as usize;
        let Some(chemin) = self.chemins.get(i) else {
            return false;
        };
        if let Some(m) = &f.q
            && !chemin.to_lowercase().contains(m.as_str())
        {
            return false;
        }
        // La garde vit ICI aussi, et pas seulement dans `base` : avec un `?ext=` ou un `?cpk=`,
        // le point de départ est une liste pré-calculée qui n'est pas triée par chemin, et la
        // tranche dichotomique ne s'applique pas. C'est le même défaut que `/b?ext=inexistante`
        // rendant le dossier entier — une garde écrite dans UN chemin de code n'en couvre pas
        // deux.
        if let Some(p) = &f.prefixe
            && !chemin.starts_with(p.as_str())
        {
            return false;
        }
        if let Some(g) = &f.glob
            && !g.accepte(chemin)
        {
            return false;
        }
        if let Some(e) = &f.ext
            && !extension(chemin).is_some_and(|x| x.eq_ignore_ascii_case(e))
        {
            return false;
        }
        if let Some(r) = f.cpk
            && self.cpk_de.get(i).copied().unwrap_or(CPK_INCONNU) != r
        {
            return false;
        }
        let taille = self.tailles.get(i).copied().unwrap_or(0);
        if f.taille_min.is_some_and(|m| taille < m) || f.taille_max.is_some_and(|m| taille > m) {
            return false;
        }
        true
    }

    /// Applique filtre, tri et pagination à une base d'indices **croissante**.
    ///
    /// Le tri par taille ne retrie rien : il balaie la permutation `par_taille` calculée à la
    /// construction et teste l'appartenance à la base par dichotomie — la base étant croissante
    /// par construction.
    fn trancher(&self, base: &[u32], r: &Requete) -> (Vec<Fichier>, usize) {
        let (f, tri, ordre, offset, limite) = (&r.filtre, r.tri, r.ordre, r.offset, r.limite);
        let vide = f.est_vide();
        let garde = |i: &u32| vide || self.retenu(*i, f);
        let total = if vide {
            base.len()
        } else {
            base.iter().filter(|i| garde(i)).count()
        };

        let indices: Vec<u32> = match tri {
            Tri::Nom => match ordre {
                Ordre::Asc => base
                    .iter()
                    .filter(|i| garde(i))
                    .skip(offset)
                    .take(limite)
                    .copied()
                    .collect(),
                Ordre::Desc => base
                    .iter()
                    .rev()
                    .filter(|i| garde(i))
                    .skip(offset)
                    .take(limite)
                    .copied()
                    .collect(),
            },
            Tri::Taille => {
                let dans_base = |i: &u32| base.binary_search(i).is_ok();
                match ordre {
                    Ordre::Asc => self
                        .par_taille
                        .iter()
                        .filter(|i| dans_base(i) && garde(i))
                        .skip(offset)
                        .take(limite)
                        .copied()
                        .collect(),
                    Ordre::Desc => self
                        .par_taille
                        .iter()
                        .rev()
                        .filter(|i| dans_base(i) && garde(i))
                        .skip(offset)
                        .take(limite)
                        .copied()
                        .collect(),
                }
            }
        };

        let fichiers = indices
            .into_iter()
            .filter_map(|i| self.fichier(i as usize))
            .collect();
        (fichiers, total)
    }

    /// Une page d'une vue : les fichiers retenus, dans l'ordre du VFS.
    #[must_use]
    pub fn page_vue(&self, vue: Vue, offset: usize, limite: usize) -> Vec<Fichier> {
        self.vues[rang(vue)]
            .iter()
            .skip(offset)
            .take(limite)
            .filter_map(|i| self.fichier(*i as usize))
            .collect()
    }

    /// Une page filtrée et triée, sur une vue ou sur l'espace VFS entier.
    ///
    /// `vue = None` désigne les 255 308 entrées — le seul moyen d'atteindre les 112 062
    /// fichiers que les quatre filtres enregistrés ne retiennent pas.
    ///
    /// Rend la page et le **total filtré** : déduire le total du nombre d'éléments rendus
    /// donnerait une dernière page qui ne finit jamais.
    #[must_use]
    pub fn page_filtree(&self, vue: Option<Vue>, r: &Requete) -> (Vec<Fichier>, usize) {
        let base = self.base(vue, &r.filtre);
        self.trancher(base, r)
    }

    /// Une page d'une vue restreinte à un motif, dans l'ordre du VFS.
    ///
    /// Forme courte conservée pour les appelants qui n'ont qu'un motif à appliquer.
    #[must_use]
    pub fn page_vue_filtree(
        &self,
        vue: Vue,
        motif: &str,
        offset: usize,
        limite: usize,
    ) -> Vec<Fichier> {
        let r = self
            .resoudre(Some(motif), &DemandeFiltre::default())
            .paginer(offset, limite);
        self.page_filtree(Some(vue), &r).0
    }

    /// Nombre de chemins d'une vue qui contiennent `motif`.
    ///
    /// Compté séparément de la page : le total conditionne la pagination, et le déduire du
    /// nombre d'éléments rendus donnerait une dernière page qui ne finit jamais.
    #[must_use]
    pub fn compte_vue_filtree(&self, vue: Vue, motif: &str) -> usize {
        let r = self.resoudre(Some(motif), &DemandeFiltre::default());
        self.page_filtree(Some(vue), &r.paginer(0, 0)).1
    }

    /// Contenu direct d'un préfixe. Le préfixe vide décrit la racine du VFS.
    ///
    /// Les sous-dossiers sont rendus en entier (ils sont peu nombreux à chaque niveau) ; seuls
    /// les fichiers sont paginés — c'est là que se trouvent les dossiers à 40 000 entrées.
    ///
    /// Le filtre porte aussi sur les sous-dossiers quand c'est un motif `?q=` : un dossier est
    /// un résultat au même titre qu'un fichier. Les autres critères (extension, taille, pack)
    /// n'ont pas de sens pour un dossier et ne le masquent donc pas.
    #[must_use]
    pub fn dossier(&self, prefixe: &str, offset: usize, limite: usize) -> Dossier {
        self.dossier_filtre(prefixe, &Requete::default().paginer(offset, limite))
    }

    /// Contenu direct d'un préfixe, filtré et trié.
    ///
    /// Le filtre porte aussi sur les sous-dossiers quand c'est un motif `?q=` : un dossier est
    /// un résultat au même titre qu'un fichier. Les autres critères (extension, taille, pack)
    /// n'ont pas de sens pour un dossier et ne le masquent donc pas.
    #[must_use]
    pub fn dossier_filtre(&self, prefixe: &str, r: &Requete) -> Dossier {
        let f = &r.filtre;
        let prefixe = prefixe.trim_matches('/').to_owned();
        let base_chemin = if prefixe.is_empty() {
            String::new()
        } else {
            format!("{prefixe}/")
        };
        let debut = self
            .chemins
            .partition_point(|c| c.as_str() < base_chemin.as_str());
        let mut dossiers = BTreeSet::new();
        let mut directs: Vec<u32> = Vec::new();
        for i in debut..self.chemins.len() {
            let chemin = &self.chemins[i];
            let Some(reste) = chemin.strip_prefix(base_chemin.as_str()) else {
                break;
            };
            match reste.split_once('/') {
                Some((segment, _)) => {
                    if !segment.is_empty() {
                        dossiers.insert(format!("{base_chemin}{segment}"));
                    }
                }
                None => directs.push(u32::try_from(i).unwrap_or(u32::MAX)),
            }
        }
        let total_sans_filtre = directs.len();
        let (fichiers, total_fichiers) = self.trancher(&directs, r);

        let mut dossiers: Vec<String> = dossiers.into_iter().collect();
        if let Some(m) = &f.q {
            dossiers.retain(|d| d.to_lowercase().contains(m.as_str()));
        }
        if f.impossible {
            dossiers.clear();
        }
        if r.ordre == Ordre::Desc {
            dossiers.reverse();
        }

        Dossier {
            prefixe,
            dossiers,
            fichiers,
            total_fichiers,
            total_fichiers_sans_filtre: total_sans_filtre,
            filtres: r.applique.clone(),
        }
    }

    /// Dit si un chemin exact est indexé.
    #[must_use]
    pub fn contient(&self, chemin: &str) -> bool {
        self.chemins
            .binary_search_by(|c| c.as_str().cmp(chemin))
            .is_ok()
    }

    /// Taille déclarée d'un chemin exact.
    #[must_use]
    pub fn taille(&self, chemin: &str) -> Option<u32> {
        let i = self
            .chemins
            .binary_search_by(|c| c.as_str().cmp(chemin))
            .ok()?;
        self.tailles.get(i).copied()
    }

    /// CPK d'origine d'un chemin exact, quand le montage en a un.
    #[must_use]
    pub fn cpk_de(&self, chemin: &str) -> Option<&str> {
        let i = self
            .chemins
            .binary_search_by(|c| c.as_str().cmp(chemin))
            .ok()?;
        self.nom_cpk(i)
    }

    fn nom_cpk(&self, i: usize) -> Option<&str> {
        let r = self.cpk_de.get(i).copied()?;
        if r == CPK_INCONNU {
            return None;
        }
        self.cpk_noms.get(usize::from(r)).map(String::as_str)
    }

    fn fichier(&self, i: usize) -> Option<Fichier> {
        let chemin = self.chemins.get(i)?;
        let nom = chemin.rsplit('/').next().unwrap_or(chemin).to_owned();
        Some(Fichier {
            chemin: chemin.clone(),
            nom,
            taille: self.tailles.get(i).copied().unwrap_or(0),
            cpk: self.nom_cpk(i).map(str::to_owned),
        })
    }
}

fn rang(vue: Vue) -> usize {
    match vue {
        Vue::Textures => 0,
        Vue::Modeles => 1,
        Vue::Sons => 2,
        Vue::Videos => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(chemin: &str, taille: u32, cpk: &str) -> Entree {
        Entree {
            chemin: chemin.to_owned(),
            taille,
            cpk: cpk.to_owned(),
        }
    }

    fn index_exemple() -> IndexVfs {
        IndexVfs::depuis_entrees(vec![
            e("data/dx11/tex/a.g4tx", 10, "dx11.cpk"),
            e("data/dx11/tex/b.g4tx", 20, "dx11.cpk"),
            e("data/dx11/tex/sub/c.g4tx", 30, "dx11.cpk"),
            e("data/common/chr/c01000010.g4md", 40, "common.cpk"),
            e("data/common/sound/bgm.acb", 50, "sound.cpk"),
            e("data/common/movie/op.usm", 60, "movie.cpk"),
            e("data/common/movie/op.usm", 60, "movie.cpk"),
            e("data/common/param/game_param.bin", 70, "common.cpk"),
            e("data/common/param/chara.p3lip", 80, "common.cpk"),
        ])
    }

    fn dem() -> DemandeFiltre {
        DemandeFiltre::default()
    }


    #[test]
    fn vues_comptent_ce_qu_elles_filtrent() {
        let idx = index_exemple();
        assert_eq!(idx.len(), 8, "le doublon est ecarte");
        assert_eq!(idx.compte_vue(Vue::Textures), 3);
        assert_eq!(idx.compte_vue(Vue::Modeles), 1);
        assert_eq!(idx.compte_vue(Vue::Sons), 1);
        assert_eq!(idx.compte_vue(Vue::Videos), 1);
        assert_eq!(VUES.len(), 4);
    }

    #[test]
    fn dossier_separe_fichiers_et_sous_dossiers() {
        let idx = index_exemple();
        let r = idx.resoudre(None, &dem()).paginer(0, 50);
        let d = idx.dossier_filtre("data/dx11/tex", &r);
        assert_eq!(d.total_fichiers, 2);
        assert_eq!(d.dossiers, vec!["data/dx11/tex/sub".to_owned()]);
        assert_eq!(d.fichiers[0].nom, "a.g4tx");
        assert_eq!(d.fichiers[0].taille, 10);
        assert_eq!(d.fichiers[0].cpk.as_deref(), Some("dx11.cpk"));

        let r = idx.resoudre(None, &dem()).paginer(0, 50);
        let racine = idx.dossier_filtre("", &r);
        assert_eq!(racine.dossiers, vec!["data".to_owned()]);
        assert_eq!(racine.total_fichiers, 0);
    }

    #[test]
    fn pagination_de_vue() {
        let idx = index_exemple();
        assert_eq!(idx.page_vue(Vue::Textures, 0, 2).len(), 2);
        assert_eq!(idx.page_vue(Vue::Textures, 2, 2).len(), 1);
        assert_eq!(idx.page_vue(Vue::Textures, 9, 2).len(), 0);
    }

    #[test]
    fn extension_insensible_a_la_casse() {
        assert!(Vue::Textures.retient("data/A.G4TX"));
        assert!(!Vue::Textures.retient("data/sans_extension"));
        assert_eq!(Vue::depuis_segment("modeles"), Some(Vue::Modeles));
        assert_eq!(Vue::depuis_segment("inexistante"), None);
        assert_eq!(extension("data/x.y/z"), None, "un point de dossier ne compte pas");
        assert_eq!(extension("data/a.g4tx"), Some("g4tx"));
    }

    /// Lot 1 — le test qui échoue si `?q=` cesse d'être appliqué au parcours.
    ///
    /// Prouvé par falsification : en retirant la prise en compte de `f.q` dans `retenu`, ce
    /// test rend 2 au lieu de 1 et rougit.
    #[test]
    fn q_filtre_reellement_le_parcours() {
        let idx = index_exemple();
        let r = idx.resoudre(Some("a.g4tx"), &dem()).paginer(0, 50);
        let d = idx.dossier_filtre("data/dx11/tex", &r);
        assert_eq!(d.total_fichiers, 1, "q doit restreindre le parcours");
        assert_eq!(d.total_fichiers_sans_filtre, 2, "le dossier en contient 2");
        assert_eq!(d.fichiers.len(), 1);
        assert_eq!(d.fichiers[0].nom, "a.g4tx");
        assert_eq!(d.filtres.q.as_deref(), Some("a.g4tx"));

        // Le motif porte sur le chemin ENTIER, pas sur le seul nom.
        let r = idx.resoudre(Some("DX11"), &dem()).paginer(0, 50);
        let d = idx.dossier_filtre("data/dx11/tex", &r);
        assert_eq!(d.total_fichiers, 2, "sans casse, sur le chemin entier");

        // Un motif qui ne matche rien ne rend rien — surtout pas la liste entière.
        let r = idx.resoudre(Some("zzz"), &dem()).paginer(0, 50);
        let d = idx.dossier_filtre("data/dx11/tex", &r);
        assert_eq!(d.total_fichiers, 0);
        assert!(d.dossiers.is_empty(), "les dossiers aussi sont filtres");
    }

    /// Lot 2 — le CPK d'origine est conservé et filtrable.
    #[test]
    fn filtre_par_cpk() {
        let idx = index_exemple();
        assert_eq!(idx.nb_cpks(), 4);
        assert_eq!(idx.cpk_de("data/common/param/game_param.bin"), Some("common.cpk"));

        let d = DemandeFiltre {
            cpk: Some("COMMON.CPK".to_owned()),
            ..dem()
        };
        let r = idx.resoudre(None, &d);
        let a = &r.applique;
        assert_eq!(a.cpk.as_deref(), Some("common.cpk"), "renvoye tel qu'indexe");
        assert!(!a.cpk_inconnu);
        let (_, total) = idx.page_filtree(None, &r.clone().paginer(0, 50));
        assert_eq!(total, 3, "chr + game_param + chara.p3lip");

        let comptes = idx.cpks();
        assert_eq!(comptes[0].valeur, "common.cpk");
        assert_eq!(comptes[0].total, 3);

        // Un CPK inconnu ne rend pas tout : il rend rien, et le dit.
        let d = DemandeFiltre {
            cpk: Some("absent.cpk".to_owned()),
            ..dem()
        };
        let r = idx.resoudre(None, &d);
        let a = &r.applique;
        assert!(a.cpk_inconnu);
        assert_eq!(idx.page_filtree(None, &r.clone().paginer(0, 50)).1, 0);
    }

    /// Lot 3 — l'extension atteint ce qu'aucune vue ne couvre.
    #[test]
    fn filtre_par_extension() {
        let idx = index_exemple();
        let exts = idx.extensions();
        assert_eq!(idx.nb_extensions(), 6);
        assert_eq!(exts[0].valeur, "g4tx", "trie par nombre decroissant");
        assert_eq!(exts[0].total, 3);

        for (ext, attendu) in [("bin", 1), ("p3lip", 1), ("g4tx", 3)] {
            let d = DemandeFiltre {
                ext: Some(ext.to_owned()),
                ..dem()
            };
            let r = idx.resoudre(None, &d);
        let a = &r.applique;
            assert!(!a.ext_inconnue, "{ext}");
            assert_eq!(idx.page_filtree(None, &r.clone().paginer(0, 50)).1, attendu, "{ext}");
        }

        // `.bin` n'est retenu par AUCUNE vue : sans ce filtre il est inatteignable.
        assert!(!VUES.into_iter().any(|v| v.retient("a.bin")));

        // Le point initial est toléré, la casse aussi.
        let d = DemandeFiltre {
            ext: Some(".BIN".to_owned()),
            ..dem()
        };
        let r = idx.resoudre(None, &d);
        let a = &r.applique;
        assert_eq!(a.ext.as_deref(), Some("bin"));
        assert_eq!(idx.page_filtree(None, &r.clone().paginer(0, 50)).1, 1);

        // Une extension inconnue rend zero, pas tout.
        let d = DemandeFiltre {
            ext: Some("inexistante".to_owned()),
            ..dem()
        };
        let r = idx.resoudre(None, &d);
        let a = &r.applique;
        assert!(a.ext_inconnue);
        assert_eq!(idx.page_filtree(None, &r.clone().paginer(0, 50)).1, 0);
    }

    /// Lot 4 — tri par nom et par taille, dans les deux sens.
    #[test]
    fn tri_par_nom_et_par_taille() {
        let idx = index_exemple();

        let d = DemandeFiltre {
            tri: Some("taille".to_owned()),
            ..dem()
        };
        let r = idx.resoudre(None, &d);
        let a = &r.applique;
        assert_eq!(a.tri, "taille");
        assert_eq!(a.ordre, "asc");
        let (page, total) = idx.page_filtree(None, &r.clone().paginer(0, 50));
        assert_eq!(total, 8);
        let tailles: Vec<u32> = page.iter().map(|f| f.taille).collect();
        assert_eq!(tailles, vec![10, 20, 30, 40, 50, 60, 70, 80]);

        let d = DemandeFiltre {
            tri: Some("taille".to_owned()),
            ordre: Some("desc".to_owned()),
            ..dem()
        };
        let r = idx.resoudre(None, &d);
        let a = &r.applique;
        assert_eq!(a.ordre, "desc");
        let (page, _) = idx.page_filtree(None, &r.clone().paginer(0, 3));
        assert_eq!(
            page.iter().map(|f| f.taille).collect::<Vec<_>>(),
            vec![80, 70, 60]
        );

        // Tri par nom descendant sur une vue.
        let d = DemandeFiltre {
            ordre: Some("desc".to_owned()),
            ..dem()
        };
        let r = idx.resoudre(None, &d);
        let (page, total) = idx.page_filtree(Some(Vue::Textures), &r.clone().paginer(0, 50));
        assert_eq!(total, 3);
        assert_eq!(page[0].chemin, "data/dx11/tex/sub/c.g4tx");

        // Tri par taille COMBINE a un filtre d'extension.
        let d = DemandeFiltre {
            ext: Some("g4tx".to_owned()),
            tri: Some("taille".to_owned()),
            ordre: Some("desc".to_owned()),
            ..dem()
        };
        let r = idx.resoudre(None, &d);
        let (page, total) = idx.page_filtree(None, &r.clone().paginer(0, 50));
        assert_eq!(total, 3);
        assert_eq!(
            page.iter().map(|f| f.taille).collect::<Vec<_>>(),
            vec![30, 20, 10]
        );
    }

    /// Régression : un critère impossible doit vider AUSSI le parcours d'un dossier.
    ///
    /// Le parcours n'emprunte pas les listes pré-calculées, donc la garde de `base` ne le
    /// couvrait pas : `/b?ext=inexistante` rendait le dossier entier tout en annonçant
    /// `ext_inconnue: true`. Trouvé par mesure `curl`, pas par le compilateur.
    #[test]
    fn critere_impossible_vide_aussi_le_parcours() {
        let idx = index_exemple();
        for d in [
            DemandeFiltre {
                ext: Some("inexistante".to_owned()),
                ..dem()
            },
            DemandeFiltre {
                cpk: Some("absent.cpk".to_owned()),
                ..dem()
            },
        ] {
            let r = idx.resoudre(None, &d).paginer(0, 50);
            let doss = idx.dossier_filtre("data/dx11/tex", &r);
            assert_eq!(doss.total_fichiers, 0, "{:?}", r.applique);
            assert!(doss.fichiers.is_empty());
            assert_eq!(doss.total_fichiers_sans_filtre, 2, "le dossier en a bien 2");
        }
    }

    #[test]
    fn tri_inconnu_retombe_sur_le_defaut_sans_erreur() {
        let idx = index_exemple();
        let d = DemandeFiltre {
            tri: Some("n_importe_quoi".to_owned()),
            ordre: Some("!!".to_owned()),
            ..dem()
        };
        let r = idx.resoudre(None, &d);
        let a = &r.applique;
        assert_eq!(r.tri, Tri::Nom);
        assert_eq!(r.ordre, Ordre::Asc);
        assert_eq!(a.tri, "nom");
        assert_eq!(a.ordre, "asc");
    }

    #[test]
    fn bornes_de_taille_et_bornes_croisees() {
        let idx = index_exemple();
        let d = DemandeFiltre {
            taille_min: Some(80),
            taille_max: Some(30),
            ..dem()
        };
        let r = idx.resoudre(None, &d);
        let a = &r.applique;
        assert_eq!(a.taille_min, Some(30), "bornes croisees remises en ordre");
        assert_eq!(a.taille_max, Some(80));
        // Les tailles vont de 10 à 80 par pas de 10 : `30..=80` en retient six.
        let (page, total) = idx.page_filtree(None, &r.clone().paginer(0, 50));
        assert_eq!(total, 6);
        assert!(page.iter().all(|f| (30..=80).contains(&f.taille)));
    }

    #[test]
    fn pagination_sur_selection_filtree() {
        let idx = index_exemple();
        let r = idx.resoudre(Some("data/"), &dem());
        let (p1, total) = idx.page_filtree(None, &r.clone().paginer(0, 3));
        assert_eq!(total, 8);
        assert_eq!(p1.len(), 3);
        let (p2, _) = idx.page_filtree(None, &r.clone().paginer(6, 3));
        assert_eq!(p2.len(), 2, "la derniere page finit");
        let (p3, _) = idx.page_filtree(None, &r.clone().paginer(100, 3));
        assert!(p3.is_empty());
    }

    #[test]
    fn depuis_sans_cpk_reste_utilisable() {
        let idx = IndexVfs::depuis(vec![("data/a.g4tx".to_owned(), 1)]);
        assert_eq!(idx.len(), 1);
        assert_eq!(idx.nb_cpks(), 0);
        assert!(idx.cpks().is_empty());
        assert_eq!(idx.cpk_de("data/a.g4tx"), None);
        let r = idx.resoudre(None, &dem());
        let (page, _) = idx.page_filtree(None, &r.clone().paginer(0, 10));
        assert_eq!(page[0].cpk, None, "aucun cpk invente sur un dump");
    }
}
