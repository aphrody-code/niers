//! La couche « formats Level-5 » du dépôt, servie par Aphrody — `/api/v1/formats/*`.
//!
//! ## Ce que cette crate décode elle-même, et ce qu'elle délègue
//!
//! `nie-formats` sait lire une trentaine de formats du jeu, et la frontière ne passe pas où on
//! l'attend : ce sont les **features** qui décident, pas la difficulté du format. Ses décodeurs
//! de textures, d'images et d'audio sont derrière `textures`, `images` et `audio-decode`, que
//! `nie-site` n'active pas — les tirer amènerait `image_dds`, `image` et `cridecoder` dans un
//! service web. Ses parseurs géométriques, eux, sont derrière `std`, une feature **par défaut**
//! que le site liait déjà : ils étaient présents dans le binaire, personne ne les appelait.
//!
//! Il en résulte une frontière nette, et [`capacites`] la **mesure** au lieu de la promettre :
//!
//! | Famille | Qui décode | Où |
//! |---|---|---|
//! | `cfg.bin` (RDBN et T2B) | cette crate, en process | `/api/v1/formats/decode/{chemin}` |
//! | `lua.bin` (bytecode Lua 5.2) | cette crate, en process | `/api/v1/lua` (cf. [`super::lua`]) |
//! | les 9 familles géométriques, 83 753 fichiers | cette crate, en process | `/api/v1/formats/decode/{chemin}` (cf. [`super::geometrie`]) |
//! | textures, modèles, audio, vidéo | `nie-model-serve`, en amont | `/assets/…` (cf. [`super::assets`]) |
//! | octets bruts, sans décodage | le VFS | `/f/{chemin}` (cf. [`super::vfs`]) |
//!
//! Annoncer ici un décodage de texture aurait été une capacité inventée : elle aurait répondu
//! `500` sur chaque appel, et un `500` n'apprend rien à l'appelant. Une capacité absente se
//! **dit**, avec la route qui la porte réellement.
//!
//! ## `cfg.bin` : deux formats derrière une seule extension
//!
//! Un `.cfg.bin` est soit du **RDBN** (des listes de lignes typées), soit du **T2B** (un arbre
//! d'entrées) — c'est le magic qui tranche, jamais le chemin. Le décodage passe par
//! `nie_formats::cfgbin::to_iecode_json`, qui aiguille sur `is_rdbn` et rend la forme
//! canonique `{lists}` ou `{entries}`. C'est la forme que lisent tous les parseurs de
//! `nie-data` — à ne pas confondre avec `niers decode`, qui rend la structure BRUTE
//! (`header`/`types`/`fields`) et dont un consommateur typé lit zéro élément en annonçant un
//! succès.
//!
//! Le format effectivement reconnu est rendu dans la réponse, en jeton choisi (`rdbn` / `t2b`),
//! jamais par un `format!("{:?}")` sur un type interne.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, RwLock};

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use crate::error::ErreurSite;
use crate::state::EtatSite;
use crate::vfs_index::IndexVfs;

/// Suffixe des fichiers de configuration du jeu.
pub const SUFFIXE_CFG: &str = ".cfg.bin";

/// Taille au-delà de laquelle un fichier n'est pas décodé en JSON.
///
/// Un `cfg.bin` décodé pèse plusieurs fois ses octets d'origine, et la couche d'ETag cesse de
/// condenser au-delà de 8 Mio ([`crate::etag::TAILLE_MAX`]) : passer cette borne servirait un
/// corps énorme sans validateur. Le plus gros `cfg.bin` du jeu tient très en deçà.
pub const TAILLE_MAX: usize = 4 * 1024 * 1024;

/// Nombre maximal de décodages simultanés, faute de connaître le parallélisme de la machine.
const DECODAGES_SIMULTANES_DEFAUT: usize = 4;

/// Le sémaphore des décodages : il borne le CPU de la machine, pas une ressource du service.
fn jetons_decodage() -> &'static tokio::sync::Semaphore {
    static JETONS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    JETONS.get_or_init(|| {
        let n = std::thread::available_parallelism()
            .map_or(DECODAGES_SIMULTANES_DEFAUT, std::num::NonZeroUsize::get);
        tokio::sync::Semaphore::new(n.max(1))
    })
}

/// Prend un jeton de décodage, ou dit pourquoi il n'y en a plus.
async fn jeton_decodage() -> Result<tokio::sync::SemaphorePermit<'static>, ErreurSite> {
    jetons_decodage()
        .acquire()
        .await
        .map_err(|_| ErreurSite::Interne("limiteur de decodage ferme".to_owned()))
}

// ─── Les capacités, mesurées ────────────────────────────────────────────────────────────────

/// Les suffixes que ce module compte sur le VFS, et ce qu'il en dit.
///
/// `(suffixe, décodé en process, route qui le sert, ce que ça rend)`. Le tableau est court à
/// dessein : chaque ligne doit être vraie, et une ligne dont la route répondrait `500` n'a rien
/// à y faire.
///
/// Il ne porte **pas** les neuf familles géométriques : leur source unique est
/// [`super::geometrie::FAMILLES`], et [`familles`] fusionne les deux. Recopier une liste, c'est
/// s'engager à la mettre à jour deux fois.
const FAMILLES_PROPRES: [(&str, bool, &str, &str); 7] = [
    (
        ".cfg.bin",
        true,
        "/api/v1/formats/decode/{chemin}",
        "application/json",
    ),
    (
        ".lua.bin",
        true,
        "/api/v1/lua/scripts/{chemin}",
        "application/json",
    ),
    (".g4tx", false, "/assets/tex/{chemin}.png", "image/png"),
    (".g4md", false, "/api/v1/3d", "model/gltf-binary"),
    // `.g4mg` n'est PAS ici : il est passé au décodage en process (cf. `super::geometrie`), et
    // une extension ne peut avoir qu'une ligne — deux donneraient deux comptes du même corpus.
    // Le maillage **assemblé** reste servi en GLB par `/api/v1/3d` et `/model/…` : décoder la
    // géométrie d'un fichier et assembler un modèle jouable sont deux services distincts.
    (".acb", false, "/assets/audio-info/{chemin}", "application/json"),
    (".awb", false, "/assets/audio-info/{chemin}", "application/json"),
    (".usm", false, "/f/{chemin}", "application/octet-stream"),
];

/// Toutes les familles annoncées : celles de ce module, plus les neuf géométriques.
///
/// Une seule liste est publiée, mais elle a deux sources et aucune n'est recopiée : c'est ce
/// qui garantit qu'une famille ajoutée à [`super::geometrie::FAMILLES`] apparaît ici, dans les
/// comptes de `/api/v1/formats` et dans l'aiguillage de `/decode`, sans autre geste.
#[must_use]
pub fn familles() -> Vec<(&'static str, bool, &'static str, &'static str)> {
    FAMILLES_PROPRES
        .into_iter()
        .chain(super::geometrie::FAMILLES.into_iter().map(|(s, ..)| {
            (
                s,
                true,
                "/api/v1/formats/decode/{chemin}",
                "application/json",
            )
        }))
        .collect()
}

/// Ce que le service sait faire d'une famille de fichiers, avec son compte.
#[derive(Debug, Clone, Serialize)]
pub struct Famille {
    /// Suffixe reconnu, point compris.
    pub suffixe: &'static str,
    /// `en_process` quand cette crate décode elle-même, `delegue` sinon.
    pub decodage: &'static str,
    /// Route qui rend réellement cette famille.
    pub route: &'static str,
    /// Type de contenu produit par cette route.
    pub sortie: &'static str,
    /// Nombre de fichiers indexés portant ce suffixe, `None` tant que le VFS n'est pas monté.
    pub fichiers: Option<usize>,
    /// Somme de leurs tailles déclarées, en octets.
    pub octets: Option<u64>,
}

/// Corps de `/api/v1/formats`.
#[derive(Debug, Clone, Serialize)]
pub struct CapacitesFormats {
    /// Toujours `nie-site`.
    pub service: &'static str,
    /// Version de la crate.
    pub version: &'static str,
    /// Vrai quand l'index du VFS est prêt : sans lui, aucun compte n'est mesurable.
    pub vfs_pret: bool,
    /// Nombre total de chemins indexés.
    pub vfs_entrees: usize,
    /// Features de `nie-formats` réellement compilées dans ce binaire.
    pub features: Vec<&'static str>,
    /// Les suffixes décodés **en process**, la seule liste que `/decode` accepte.
    pub decodables: Vec<&'static str>,
    /// Le détail par famille, compté.
    pub familles: Vec<Famille>,
}

/// Les comptes par suffixe, mémorisés avec la taille d'index qui les a produits.
type TableComptes = BTreeMap<&'static str, (usize, u64)>;
type ComptesMemorises = Option<(usize, Arc<TableComptes>)>;
static COMPTES: RwLock<ComptesMemorises> = RwLock::new(None);

/// Balaye l'index une fois et compte les fichiers de chaque famille.
///
/// Comme pour le catalogue Lua, l'index n'est pas itérable : on descend dossier par dossier, et
/// [`IndexVfs::dossier`] s'arrête dès que le préfixe ne correspond plus.
#[must_use]
pub fn compter(index: &IndexVfs) -> TableComptes {
    let table = familles();
    let mut comptes: TableComptes = table.iter().map(|(s, ..)| (*s, (0, 0))).collect();
    let mut a_visiter = vec![String::new()];
    while let Some(prefixe) = a_visiter.pop() {
        let d = index.dossier(&prefixe, 0, usize::MAX);
        for f in &d.fichiers {
            for (suffixe, ..) in &table {
                if f.chemin.ends_with(suffixe)
                    && let Some(e) = comptes.get_mut(suffixe)
                {
                    e.0 += 1;
                    e.1 += u64::from(f.taille);
                }
            }
        }
        a_visiter.extend(d.dossiers);
    }
    comptes
}

/// Les comptes courants, balayés une fois par état d'index.
fn comptes(index: &IndexVfs) -> Arc<TableComptes> {
    let cle = index.len();
    if let Ok(garde) = COMPTES.read()
        && let Some((k, v)) = garde.as_ref()
        && *k == cle
    {
        return Arc::clone(v);
    }
    let table = Arc::new(compter(index));
    if let Ok(mut garde) = COMPTES.write() {
        *garde = Some((cle, Arc::clone(&table)));
    }
    table
}

/// Les features de `nie-formats` compilées dans ce binaire, telles que `cfg!` les voit.
///
/// Elles sont rapportées parce qu'une feature éteinte ne se manifeste jamais à l'exécution :
/// elle fait juste disparaître une capacité, en silence. Les nommer ici, c'est transformer une
/// absence invisible en fait vérifiable par une requête.
#[must_use]
pub fn features() -> Vec<&'static str> {
    FEATURES_FORMATS.to_vec()
}

/// Les features de `nie-formats` que `nie-site/Cargo.toml` demande, c'est-à-dire ses features
/// par défaut. `textures`, `images`, `textures-encode` et `audio-decode` en sont absentes, et
/// leur absence est ce qui décide de la colonne « qui décode » de [`familles`].
pub const FEATURES_FORMATS: [&str; 2] = ["std", "lua"];

/// `GET /api/v1/formats` — les formats du jeu, ce qui les décode, et combien il y en a.
pub async fn capacites(State(etat): State<EtatSite>) -> Json<CapacitesFormats> {
    let index = etat.index().ok();
    let comptes_par_suffixe = match index.as_ref() {
        Some(i) => {
            let i = Arc::clone(i);
            tokio::task::spawn_blocking(move || comptes(&i)).await.ok()
        }
        None => None,
    };

    let table = familles();
    let detail = table
        .iter()
        .map(|&(suffixe, en_process, route, sortie)| {
            let compte = comptes_par_suffixe
                .as_ref()
                .and_then(|t| t.get(suffixe))
                .copied();
            Famille {
                suffixe,
                decodage: if en_process { "en_process" } else { "delegue" },
                route,
                sortie,
                fichiers: compte.map(|(n, _)| n),
                octets: compte.map(|(_, o)| o),
            }
        })
        .collect();

    Json(CapacitesFormats {
        service: crate::SERVICE,
        version: crate::VERSION,
        vfs_pret: index.is_some(),
        vfs_entrees: index.as_ref().map_or(0, |i| i.len()),
        features: features(),
        decodables: table
            .iter()
            .filter_map(|&(s, en_process, ..)| en_process.then_some(s))
            .collect(),
        familles: detail,
    })
}

// ─── Le décodage ────────────────────────────────────────────────────────────────────────────

/// Corps de `/api/v1/formats/decode/{*chemin}`.
#[derive(Debug, Clone, Serialize)]
pub struct Decodage {
    /// Chemin VFS décodé.
    pub chemin: String,
    /// Taille du fichier source, en octets.
    pub octets: usize,
    /// Format reconnu au magic : `rdbn` ou `t2b`. Un jeton choisi, pas un nom de type Rust.
    pub format: &'static str,
    /// Nombre de listes (RDBN) ou d'entrées racines (T2B) — de quoi vérifier d'un coup d'œil
    /// qu'un décodage « réussi » n'est pas vide.
    pub racines: usize,
    /// La forme canonique iecode : `{"lists": …}` en RDBN, `{"entries": …}` en T2B.
    pub donnees: serde_json::Value,
}

/// Le schéma d'un `cfg.bin`, tel que le parseur le voit avant toute mise en forme.
///
/// C'est l'autre moitié de ce que `nie_formats::cfgbin` sait faire : `to_iecode_json` rend les
/// **valeurs**, `parse`/`cfgbin_parse` rendent la **forme** — la table de types, la table de
/// champs, les racines, la table de hachage CRC32 qui résout les noms. Sans elle, un client ne
/// peut pas savoir quels champs existent avant d'en lire une ligne, ni pourquoi un nom sort en
/// `Unknown_0x…` (son hash n'est pas dans la table du fichier).
#[derive(Debug, Clone, Serialize)]
pub struct Structure {
    /// Version du format déclarée par l'en-tête (RDBN seulement).
    pub version: Option<i32>,
    /// Nombre de types, champs, racines et chaînes déclarés par l'en-tête (RDBN seulement).
    pub entete: Option<EnteteRdbn>,
    /// Les listes racines, avec leur nom et leur type résolus.
    pub racines: Vec<Racine>,
    /// Les types et leurs champs, dans l'ordre de la table.
    pub types: Vec<TypeRdbn>,
    /// Nombre de couples (hash CRC32, nom) portés par le fichier.
    pub chaines: usize,
}

/// L'en-tête RDBN, champ par champ.
#[derive(Debug, Clone, Serialize)]
pub struct EnteteRdbn {
    /// Nombre de types.
    pub types: u16,
    /// Nombre de champs.
    pub champs: u16,
    /// Nombre de racines.
    pub racines: u16,
    /// Nombre d'entrées de la table de hachage.
    pub chaines: u16,
    /// Taille de la section de données, en octets.
    pub donnees_octets: i32,
}

/// Une liste racine.
#[derive(Debug, Clone, Serialize)]
pub struct Racine {
    /// Nom résolu depuis la table de hachage, ou `Unknown_0x…` quand le hash n'y est pas.
    pub nom: String,
    /// Nom du type de ses lignes.
    pub type_nom: String,
    /// Nombre de lignes.
    pub lignes: i32,
    /// Taille d'une ligne, en octets.
    pub ligne_octets: i32,
}

/// Un type et ses champs.
#[derive(Debug, Clone, Serialize)]
pub struct TypeRdbn {
    /// Nom résolu du type.
    pub nom: String,
    /// Ses champs, dans l'ordre de la table.
    pub champs: Vec<Champ>,
}

/// Un champ d'un type RDBN.
#[derive(Debug, Clone, Serialize)]
pub struct Champ {
    /// Nom résolu du champ.
    pub nom: String,
    /// Type du champ, en jeton choisi (`bool`, `float`, `position`…), jamais un `Debug`.
    pub type_champ: &'static str,
    /// Taille de la valeur, en octets.
    pub octets: i32,
    /// Nombre de valeurs.
    pub valeurs: i32,
    /// Décalage de la valeur dans la ligne.
    pub decalage: i32,
}

/// Traduit un type de champ RDBN en jeton stable.
///
/// `format!("{:?}")` publierait le nom de la variante Rust — un détail d'implémentation qui
/// changerait avec un renommage. Le jeton, lui, est un contrat.
#[must_use]
pub fn jeton_type(t: nie_formats::cfgbin::RdbnFieldType) -> &'static str {
    use nie_formats::cfgbin::RdbnFieldType as T;
    match t {
        T::AbilityData => "ability_data",
        T::EnhanceData => "enhance_data",
        T::StatusRate => "status_rate",
        T::Bool => "bool",
        T::Byte => "byte",
        T::Short => "short",
        T::Int => "int",
        T::ActType => "act_type",
        T::Flag => "flag",
        T::Float => "float",
        T::Hash => "hash",
        T::Rates => "rates",
        T::Position => "position",
        T::Condition => "condition",
        T::ShortTuple => "short_tuple",
        T::Unknown(_) => "inconnu",
    }
}

/// Extrait la structure d'un `cfg.bin`, RDBN ou T2B.
///
/// # Errors
///
/// `Demande` quand les octets ne sont lisibles par aucun des deux parseurs.
pub fn structurer(octets: &[u8]) -> Result<Structure, ErreurSite> {
    use nie_formats::cfgbin;
    if cfgbin::is_rdbn(octets) {
        let d = cfgbin::parse(octets)
            .map_err(|e| ErreurSite::Demande(format!("RDBN illisible: {e}")))?;
        let racines = d
            .roots
            .iter()
            .map(|r| Racine {
                nom: d
                    .root_name(r)
                    .map_or_else(|| format!("Unknown_0x{:08X}", r.name_hash), str::to_owned),
                type_nom: d
                    .types
                    .get(usize::try_from(r.type_index).unwrap_or(usize::MAX))
                    .and_then(|t| d.type_name(t))
                    .map_or_else(|| format!("Type_{}", r.type_index), str::to_owned),
                lignes: r.value_count,
                ligne_octets: r.value_size,
            })
            .collect();
        let types = d
            .types
            .iter()
            .map(|t| {
                let debut = usize::try_from(t.field_index).unwrap_or(0);
                let fin = debut.saturating_add(usize::try_from(t.field_count).unwrap_or(0));
                let champs = d
                    .fields
                    .get(debut..fin.min(d.fields.len()))
                    .unwrap_or(&[])
                    .iter()
                    .map(|f| Champ {
                        nom: d.field_name(f).map_or_else(
                            || format!("Unknown_0x{:08X}", f.name_hash),
                            str::to_owned,
                        ),
                        type_champ: jeton_type(f.field_type),
                        octets: f.value_size,
                        valeurs: f.value_count,
                        decalage: f.value_offset,
                    })
                    .collect();
                TypeRdbn {
                    nom: d
                        .type_name(t)
                        .map_or_else(|| format!("Type_0x{:08X}", t.name_hash), str::to_owned),
                    champs,
                }
            })
            .collect();
        return Ok(Structure {
            version: Some(d.header.version),
            entete: Some(EnteteRdbn {
                types: d.header.type_count,
                champs: d.header.field_count,
                racines: d.header.root_count,
                chaines: d.header.hash_count,
                donnees_octets: d.header.data_size,
            }),
            racines,
            types,
            chaines: d.strings.entries.len(),
        });
    }

    // T2B : un arbre, sans table de types. Les racines sont les entrées de premier niveau, et
    // leur « type » est le nombre de variables qu'elles portent — c'est la seule forme que le
    // format déclare.
    let f = cfgbin::cfgbin_parse(octets)
        .map_err(|e| ErreurSite::Demande(format!("T2B illisible: {e}")))?;
    let racines = f
        .entries
        .iter()
        .map(|e| Racine {
            nom: e.name.clone(),
            type_nom: "t2b_entry".to_owned(),
            lignes: i32::try_from(e.children.len()).unwrap_or(i32::MAX),
            ligne_octets: i32::try_from(e.variables.len()).unwrap_or(i32::MAX),
        })
        .collect();
    Ok(Structure {
        version: None,
        entete: None,
        racines,
        types: Vec::new(),
        chaines: 0,
    })
}

/// Décode des octets de `cfg.bin`. Séparée du handler pour être testable sans HTTP ni VFS.
///
/// # Errors
///
/// `Demande` quand les octets ne sont ni du RDBN ni du T2B lisible.
pub fn decoder(chemin: &str, octets: &[u8]) -> Result<Decodage, ErreurSite> {
    let rdbn = nie_formats::cfgbin::is_rdbn(octets);
    let donnees = nie_formats::cfgbin::to_iecode_json(octets).ok_or_else(|| {
        ErreurSite::Demande(
            "ces octets ne sont ni du RDBN ni du T2B lisible par le decodeur du depot".to_owned(),
        )
    })?;
    let (format, cle) = if rdbn {
        ("rdbn", "lists")
    } else {
        ("t2b", "entries")
    };
    let racines = donnees
        .get(cle)
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    Ok(Decodage {
        chemin: chemin.to_owned(),
        octets: octets.len(),
        format,
        racines,
        donnees,
    })
}

/// Ce que `/api/v1/formats/decode/{*chemin}` doit rendre.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DemandeDecode {
    /// `valeurs` (défaut) ou `structure`.
    pub forme: Option<String>,
}

/// Les deux moitiés de ce que `cfgbin` sait faire d'un fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forme {
    /// Les données, en forme canonique iecode (`to_iecode_json`).
    Valeurs,
    /// Le schéma : types, champs, racines, table de hachage (`parse` / `cfgbin_parse`).
    Structure,
}

impl Forme {
    /// Reconnaît une forme, ou dit lesquelles existent.
    ///
    /// # Errors
    ///
    /// `Demande` sur une forme inconnue.
    pub fn depuis(s: Option<&str>) -> Result<Self, ErreurSite> {
        match s.map(str::trim).filter(|f| !f.is_empty()) {
            None | Some("valeurs") => Ok(Self::Valeurs),
            Some("structure") => Ok(Self::Structure),
            Some(autre) => Err(ErreurSite::Demande(format!(
                "forme inconnue: {autre} (connues: valeurs, structure)"
            ))),
        }
    }
}

/// Cherche la description d'un `.g4mg` — son `.g4md` frère, sinon le `.g4pkm` qui l'empaquette.
///
/// Rend `None` sans erreur quand aucun des deux n'est lisible : c'est au décodeur de dire ce
/// qui manque, en une phrase qui nomme les deux endroits cherchés. Un `None` silencieux ici
/// deviendrait un « décodage vide » là-bas, et personne ne saurait pourquoi.
async fn resoudre_compagnon(
    etat: &EtatSite,
    index: &IndexVfs,
    chemin_g4mg: &str,
) -> Option<super::geometrie::Compagnon> {
    let vfs = etat.vfs().ok()?;
    for candidat in super::geometrie::Compagnon::candidats(chemin_g4mg) {
        if !index.contient(&candidat) {
            continue;
        }
        let vfs = Arc::clone(&vfs);
        let a_lire = candidat.clone();
        let lu = tokio::task::spawn_blocking(move || vfs.read(&a_lire)).await;
        let Ok(Ok(octets)) = lu else { continue };
        if let Some(c) = super::geometrie::Compagnon::depuis(&candidat, octets) {
            return Some(c);
        }
    }
    None
}

/// `GET /api/v1/formats/decode/{*chemin}` — un fichier du VFS décodé.
///
/// Deux espaces de formes derrière une seule route, parce qu'il n'y a qu'une intention —
/// « décode-moi ce chemin » — et qu'un client n'a pas à deviner le préfixe de sa famille :
///
/// - `.cfg.bin` : `?forme=valeurs` (défaut) rend les données en forme canonique iecode,
///   `?forme=structure` rend le schéma que le fichier porte lui-même ;
/// - les huit familles géométriques ([`super::geometrie::FAMILLES`]) : `?forme=resume`
///   (défaut) rend des comptes, `?forme=complet` rend la structure entière.
///
/// # Errors
///
/// `Demande` sur chemin sortant, forme inconnue, suffixe non décodable en process, fichier trop
/// volumineux ou octets illisibles ; `Introuvable` sur chemin absent ; `Indisponible` tant que
/// le VFS n'est pas monté.
pub async fn decode(
    State(etat): State<EtatSite>,
    Path(brut): Path<String>,
    Query(demande): Query<DemandeDecode>,
) -> Result<Json<serde_json::Value>, ErreurSite> {
    let chemin = super::vfs::normaliser(&brut)?;
    if chemin.ends_with(super::lua::SUFFIXE) {
        return Err(ErreurSite::Demande(
            "un script se lit sur /api/v1/lua/scripts, qui en rend l'analyse statique".to_owned(),
        ));
    }

    // La famille décide de la forme acceptée, de la borne de taille et du décodeur appelé. On
    // la résout d'abord au **suffixe**, parce que c'est gratuit ; mais un suffixe inconnu ne
    // fait plus refuser la requête : le magic tranchera après lecture (cf. `identifier`).
    // Quatorze fichiers du VFS portent le magic `G4PK` sous un suffixe de révision
    // (`.g4pk.r41152`…) — les refuser sur leur nom, c'est croire le nom plutôt que le contenu.
    let geom = super::geometrie::Famille::depuis_chemin(&chemin);
    let cfg = chemin.ends_with(SUFFIXE_CFG);
    let forme_geom = geom
        .map(|_| super::geometrie::Forme::depuis(demande.forme.as_deref()))
        .transpose()?;
    let forme_cfg = if cfg {
        Some(Forme::depuis(demande.forme.as_deref())?)
    } else {
        None
    };

    let index = etat.index()?;
    if !index.contient(&chemin) {
        return Err(ErreurSite::Introuvable(format!(
            "chemin absent du VFS: {chemin}"
        )));
    }

    let vfs = etat.vfs()?;
    let a_lire = chemin.clone();
    let octets = tokio::task::spawn_blocking(move || vfs.read(&a_lire))
        .await?
        .map_err(|e| {
            tracing::debug!(erreur = %e, "lecture VFS impossible");
            ErreurSite::Introuvable("fichier indexe mais illisible sur ce montage".to_owned())
        })?;
    // La famille définitive : le suffixe s'il disait quelque chose, le **magic** sinon.
    let famille = geom.or_else(|| {
        if cfg {
            None
        } else {
            super::geometrie::famille_au_magic(&octets)
        }
    });
    let forme_geom = match (famille, forme_geom) {
        (Some(_), None) => Some(super::geometrie::Forme::depuis(demande.forme.as_deref())?),
        (_, f) => f,
    };

    // Deux bornes, parce qu'elles ne bornent pas la même chose : la sortie grossit avec la
    // source (un `cfg.bin` décodé, une structure complète) ou non (un résumé, un en-tête
    // identifié). La seconde borne vaut 16 Mio et couvre le plus gros `.g4pk` du jeu.
    //
    // Mesuré : avec la borne de 4 Mio par défaut, `ev60007900.g4tg` (4 587 520 o) était refusé
    // « trop volumineux » alors que l'identification n'aurait rendu que seize octets d'en-tête.
    // Une borne qui protège d'un JSON énorme n'a rien à faire sur un chemin qui n'en produit
    // aucun.
    let borne = match (forme_geom, forme_cfg) {
        (Some(super::geometrie::Forme::Complet), _) | (_, Some(_)) => TAILLE_MAX,
        _ => super::geometrie::TAILLE_MAX_RESUME,
    };
    if octets.len() > borne {
        return Err(ErreurSite::Demande(format!(
            "fichier trop volumineux pour un decodage en JSON ({} octets, borne {borne})",
            octets.len()
        )));
    }

    // Un `.g4mg` ne se lit pas seul : sa description vit dans le `.g4md` frère ou, pour 6 920
    // fichiers sur 15 875, empaquetée dans le `.g4pkm` voisin. On la résout ICI, où l'index et
    // le VFS sont disponibles, plutôt que dans le décodeur — qui reste une fonction pure,
    // testable sans HTTP.
    let compagnon = if famille == Some(super::geometrie::Famille::G4mg) {
        resoudre_compagnon(&etat, &index, &chemin).await
    } else {
        None
    };

    let _jeton = jeton_decodage().await?;
    let corps = tokio::task::spawn_blocking(move || {
        let v = match (famille, forme_geom, forme_cfg) {
            (Some(f), Some(forme), _) => serde_json::to_value(super::geometrie::decoder(
                &chemin,
                &octets,
                f,
                forme,
                compagnon.as_ref(),
            )?),
            (_, _, Some(Forme::Structure)) => {
                serde_json::to_value(structurer(&octets)?).map(|st| {
                    serde_json::json!({ "chemin": chemin, "octets": octets.len(), "structure": st })
                })
            }
            (_, _, Some(Forme::Valeurs)) => serde_json::to_value(decoder(&chemin, &octets)?),
            // Ni suffixe connu, ni magic connu : le dernier recours, qui identifie au lieu de
            // refuser. Il rend un `cfg.bin` déguisé, un conteneur Level-5 nommé, ou une erreur
            // qui DIT ce qu'elle a vu.
            _ => serde_json::to_value(identifier(&chemin, &octets)?),
        }
        .map_err(|e| ErreurSite::Interne(format!("reponse non serialisable: {e}")))?;
        Ok::<_, ErreurSite>(v)
    })
    .await??;
    Ok(Json(corps))
}

/// Identifie un fichier qu'aucun suffixe ni aucun magic de parseur ne réclame.
///
/// Trois issues, dans cet ordre, et **aucune n'invente** :
///
/// 1. un `cfg.bin` déguisé — le T2B n'a pas de magic ASCII, donc un `.cfg.bin.r65902` ne se
///    reconnaît qu'en essayant de le lire ;
/// 2. un **conteneur Level-5** dont le corps n'est pas interprété : `.g4vs` et `.g4la` portent
///    le même en-tête de 16 octets que les formats connus, et le dire vaut mieux que se taire ;
/// 3. un refus qui **publie les premiers octets**. Une erreur qui n'apprend rien oblige le
///    prochain à refaire le `xxd` ; celle-ci le lui épargne.
///
/// # Errors
///
/// `Demande` quand rien n'identifie le fichier.
pub fn identifier(chemin: &str, octets: &[u8]) -> Result<serde_json::Value, ErreurSite> {
    if let Ok(d) = decoder(chemin, octets) {
        return serde_json::to_value(d)
            .map_err(|e| ErreurSite::Interne(format!("reponse non serialisable: {e}")));
    }
    if let Some(c) = super::geometrie::conteneur_level5(octets) {
        return Ok(serde_json::json!({
            "chemin": chemin,
            "octets": octets.len(),
            "format": "conteneur_level5",
            "produit": "en-tete commun Level-5, corps non interprete",
            "conteneur": c,
        }));
    }
    // CriWare : `@UTF` est la table de métadonnées de toute la pile audio (ACB, AWB, ACF). Le
    // site ne décode pas l'audio — ces features sont éteintes, cf. la doc de module — mais
    // **nommer** un format qu'on ne décode pas est une information, et « inconnu » n'en est
    // pas une. Le seul `.acf` du VFS (`sound.acf`, la configuration du moteur audio) tombe ici.
    if octets.starts_with(b"@UTF") {
        return Ok(serde_json::json!({
            "chemin": chemin,
            "octets": octets.len(),
            "format": "criware_utf",
            "produit": "table @UTF CriWare, corps non interprete par ce service",
            "decodage": "delegue",
            "route": "/assets/audio-info/{chemin}",
        }));
    }
    let tete: String = octets
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    Err(ErreurSite::Demande(format!(
        "format non identifie: ni cfg.bin, ni conteneur Level-5, ni magic connu \
         (premiers octets: {tete})"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_familles_declarent_une_route_et_un_type() {
        let table = familles();
        assert_eq!(table.len(), 16, "7 familles propres + 9 geometriques");
        for (suffixe, _, route, sortie) in &table {
            assert!(suffixe.starts_with('.'), "{suffixe}");
            assert!(route.starts_with('/'), "{route}");
            assert!(sortie.contains('/'), "{sortie}");
        }
        // Onze familles sont decodees ici : les deux de ce module (`cfg.bin` via `std`,
        // `lua.bin` via `lua`) et les neuf geometriques, toutes derriere `std`. Les cinq autres
        // exigeraient `textures` / `images` / `audio-decode`, qui restent eteintes.
        let en_process: Vec<&str> = table
            .iter()
            .filter_map(|&(s, p, ..)| p.then_some(s))
            .collect();
        assert_eq!(
            en_process,
            vec![
                ".cfg.bin", ".lua.bin", ".g4pk", ".g4mg", ".objbin", ".g4pkm", ".g4cm", ".col",
                ".g4sk", ".mevbin", ".g4mt",
            ]
        );
        assert_eq!(features(), vec!["std", "lua"]);
        // Aucun suffixe en double : deux lignes pour un meme suffixe donneraient deux comptes
        // du meme corpus, et l'un des deux serait faux.
        let mut suffixes: Vec<&str> = table.iter().map(|&(s, ..)| s).collect();
        suffixes.sort_unstable();
        suffixes.dedup();
        assert_eq!(suffixes.len(), table.len());
    }

    #[test]
    fn le_decodage_refuse_ce_qui_n_est_pas_un_cfg_bin() {
        let e = decoder("data/x.cfg.bin", b"pas un cfg.bin du tout").unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
    }

    #[test]
    fn les_deux_formes_sont_reconnues_et_les_autres_refusees() {
        assert_eq!(Forme::depuis(None).unwrap(), Forme::Valeurs);
        assert_eq!(Forme::depuis(Some("structure")).unwrap(), Forme::Structure);
        assert_eq!(
            Forme::depuis(Some("brut")).unwrap_err().statut().as_u16(),
            400
        );
    }

    /// Les jetons de type sont un contrat : ils ne doivent jamais être le nom de la variante
    /// Rust, et deux types différents ne doivent pas se confondre.
    #[test]
    fn les_jetons_de_type_sont_choisis_et_distincts() {
        use nie_formats::cfgbin::RdbnFieldType as T;
        let jetons = [
            jeton_type(T::Bool),
            jeton_type(T::Float),
            jeton_type(T::Position),
            jeton_type(T::ShortTuple),
        ];
        assert_eq!(jetons, ["bool", "float", "position", "short_tuple"]);
        assert_eq!(jeton_type(T::Unknown(77)), "inconnu");
    }

    #[test]
    fn la_structure_refuse_ce_qui_n_est_ni_rdbn_ni_t2b() {
        assert_eq!(
            structurer(b"trop court").unwrap_err().statut().as_u16(),
            400
        );
    }

    #[test]
    fn le_comptage_ventile_par_suffixe() {
        let index = IndexVfs::depuis(vec![
            ("data/common/a.cfg.bin".to_owned(), 10),
            ("data/common/b/c.cfg.bin".to_owned(), 20),
            ("data/common/script/lua/d.lua.bin".to_owned(), 30),
            ("data/common/e.g4tx".to_owned(), 40),
            ("data/common/f.inconnu".to_owned(), 50),
        ]);
        let t = compter(&index);
        assert_eq!(t[".cfg.bin"], (2, 30));
        assert_eq!(t[".lua.bin"], (1, 30));
        assert_eq!(t[".g4tx"], (1, 40));
        assert_eq!(t[".usm"], (0, 0));
    }

    /// La garde de traversée est celle de `routes::vfs`, partagée avec `/f` et `/api/v1/lua`.
    #[test]
    fn la_traversee_est_refusee() {
        for mauvais in ["..", "data/../../etc/passwd", "data\\a.cfg.bin"] {
            assert_eq!(
                super::super::vfs::normaliser(mauvais)
                    .unwrap_err()
                    .statut()
                    .as_u16(),
                400,
                "{mauvais}"
            );
        }
    }
}
