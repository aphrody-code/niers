//! `/api/v1/text` — le texte localisé du jeu, adressé par **langue** et par **famille**.
//!
//! Depuis le 2026-09-06, `nie_data::typed::decode_by_key` rend les 980 fichiers
//! `common/text/**` du jeu sous la famille `text` : une liste de `{hash, text}`. Cette donnée
//! était atteignable par `/api/v1/donnees/{chemin}` — **à condition de connaître le chemin VFS
//! exact, numéro de version compris**. Or les fichiers du jeu portent ce numéro
//! (`chara_base_1.03.98.00.cfg.bin`), personne ne le devine, et un consommateur qui veut « le
//! menu en français » n'a pas à savoir où vit le fichier.
//!
//! C'est le même manque que `/api/v1/donnees/famille/{cle}` a fermé pour les données de jeu,
//! transposé au texte — avec une dimension de plus, la **langue**, qui n'est pas un paramètre
//! du fichier mais un segment de son chemin.
//!
//! # Nommage
//!
//! Ce module est le premier écrit sous la règle du **2026-09-06** : `niers` est un projet
//! mondial, donc **tout identifiant est en anglais** — nom de fichier, types, champs, et
//! surtout les **URLs et les clés JSON**, qui sont ce qu'un consommateur étranger lit.
//! La prose des commentaires reste en français, comme dans les vingt modules voisins. Cf.
//! `CLAUDE.md` § *Langue*.
//!
//! # Ce qui est mesuré, et ce qui ne l'est pas
//!
//! Rien ici n'est écrit à la main. Le catalogue est **construit à la première demande** en
//! décodant réellement chaque fichier candidat, exactement comme `routes::donnees` dérive ses
//! 18 326 clés : une liste de langues versionnée dans le code se périmerait à la première
//! mise à jour du jeu, et personne ne le verrait.
//!
//! Deux conséquences assumées :
//!
//! - le premier appel paie le décodage du corpus entier (52,9 Mio de sources), parallélisé sur
//!   [`SURVEY_THREADS`] fils — **6,3 s mesurées** ; les suivants lisent un
//!   [`std::sync::OnceLock`] en 10 ms ;
//! - le catalogue ne garde **aucun texte**, seulement des comptes et des chemins. C'est ce qui
//!   le rend borné : **152 672 octets mesurés**, et cette taille est publiée
//!   (`catalog_bytes`) plutôt qu'affirmée.
//!
//! # La famille `chara_description_text` n'est pas servie ici, et c'est voulu
//!
//! Elle a son **parseur propre** dans `nie-data` et sort de la façade sous la famille
//! `chara_description`, avec une structure plus riche que `{hash, text}`. La servir ici
//! forcerait soit à l'aplatir (perte), soit à faire rendre deux formes différentes à la même
//! route (pire). Elle reste sur `/api/v1/donnees/{chemin}`, et le catalogue la cite dans
//! `skipped` avec sa raison — un fichier écarté en silence est un fichier qu'on croit absent.
//!
//! # Les routes
//!
//! | Route | Ce qu'elle rend |
//! |---|---|
//! | `GET /api/v1/text` | le catalogue mesuré : langues, familles, comptes de lignes |
//! | `GET /api/v1/text/{language}/{family}` | les lignes, paginées et bornées |
//! | `GET /api/v1/text/{language}/{family}/{hash}` | ce qu'un hash désigne — une **liste** |
//! | `GET /api/v1/text/search?q=&language=` | une sous-chaîne dans le texte d'une langue |

use std::collections::BTreeMap;
use std::sync::OnceLock;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use crate::error::ErreurSite;
use crate::routes::{DemandePage, Page};
use crate::state::EtatSite;
use crate::vfs_index::{IndexVfs, Requete};

/// Préfixe VFS sous lequel vit **tout** le texte localisé du jeu.
///
/// Mesuré, pas supposé : `niers vfs find 'common/text/' -n 300000` rend 44 242 entrées, dont
/// 980 seulement sont des tables de texte ; les autres sont des événements, des cartes de
/// correspondance (`system_text_map`) et des scripts.
pub const ROOT: &str = "data/common/text/";

/// Suffixe des fichiers que cette route lit.
pub const SUFFIX: &str = ".cfg.bin";

/// Nom de la famille que `nie_data::typed::decode_by_key` rend pour une table de texte.
///
/// C'est **lui** qui décide ce qui entre dans le catalogue : le filtre sur le nom de fichier
/// n'est qu'une présélection destinée à éviter de décoder 44 242 fichiers pour en garder 980.
pub const RENDERED_FAMILY: &str = "text";

/// Nombre de fils de décodage utilisés pour construire le catalogue.
///
/// Le corpus fait 52,9 Mio et se décode fichier par fichier, sans état partagé : c'est le cas
/// idéal du découpage en tranches. Une valeur fixe et modeste plutôt qu'un `num_cpus` : le
/// service partage la machine avec 18 autres.
pub const SURVEY_THREADS: usize = 8;

/// Nombre maximal de résultats qu'une recherche accumule avant de se déclarer tronquée.
///
/// Sans cette borne, `?q=e` sur `ja` rendrait des dizaines de milliers de lignes en mémoire
/// pour n'en servir que 50. La réponse porte `truncated: true` — un total silencieusement faux
/// serait pire qu'un total absent.
pub const MAX_RESULTS: usize = 5_000;

/// Longueur minimale d'un motif de recherche.
///
/// Un motif d'un caractère ne cherche rien : il balaie le corpus entier pour rendre la borne
/// ci-dessus. La demande est refusée en `400` plutôt que servie à moitié.
pub const MIN_PATTERN: usize = 2;

/// Nombre maximal d'écarts détaillés conservés dans le catalogue.
pub const MAX_SKIPPED: usize = 64;

// ── Dérivations de chemin ────────────────────────────────────────────────────

/// Code de langue d'un chemin de texte, ou `None` si le chemin n'est pas sous [`ROOT`].
///
/// La langue est le **premier segment** après la racine (`data/common/text/fr/…`). Elle n'est
/// jamais devinée ni traduite : `zh_hant` reste `zh_hant`.
#[must_use]
pub fn language_of(path: &str) -> Option<&str> {
    let rest = path.strip_prefix(ROOT)?;
    let (language, tail) = rest.split_once('/')?;
    if language.is_empty() || tail.is_empty() {
        return None;
    }
    Some(language)
}

/// Présélection par le nom : ce fichier a-t-il la forme d'une table de texte ?
///
/// C'est **exactement** la condition de dispatch de la façade (`*_text`, plus le catalogue
/// `nie_data::text::TEXT_FILES` qui rattrape les deux `*_text_roma`) — reproduite ici pour
/// éviter un décodage inutile, jamais pour trancher : le verdict reste la famille rendue par
/// la façade, comparée à [`RENDERED_FAMILY`].
#[must_use]
pub fn looks_like_text(key: &str) -> bool {
    key.ends_with("_text") || nie_data::text::TEXT_FILES.iter().any(|(_, f)| *f == key)
}

/// Analyse un hash de ligne écrit en décimal ou en hexadécimal préfixé `0x`.
///
/// Le dépôt cite ces hash sous les deux formes (`3528663132` dans un JSON, `0xd25e0b9c` dans
/// un désassemblage) : la route accepte les deux et refuse tout le reste, plutôt que de
/// tenter une devinette qui ferait passer `1e5` pour un nombre.
///
/// # Errors
///
/// `Demande` quand la chaîne n'est ni un `u32` décimal ni un `0x` hexadécimal tenant sur 32
/// bits.
pub fn parse_hash(raw: &str) -> Result<u32, ErreurSite> {
    let clean = raw.trim();
    let parsed = if let Some(hex) = clean.strip_prefix("0x").or_else(|| clean.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        clean.parse::<u32>().ok()
    };
    parsed.ok_or_else(|| {
        ErreurSite::Demande(format!(
            "hash invalide `{raw}` : attendu un entier 32 bits, en decimal (3528663132) \
             ou en hexadecimal prefixe (0xd25e0b9c)"
        ))
    })
}

// ── Décodage d'un fichier ────────────────────────────────────────────────────

/// Une ligne de texte du jeu : son hash et son contenu déjà nettoyé par `nie-data`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    /// Hash de la ligne, tel que le jeu l'adresse.
    pub hash: u32,
    /// Le même hash en hexadécimal — c'est sous cette forme que le RE le cite.
    pub hash_hex: String,
    /// Le texte, nettoyé (furigana, balises, échappements) par `nie_data::text`.
    pub text: String,
}

/// Décode un `.cfg.bin` de texte en lignes.
///
/// Passe par la **même** façade que `/api/v1/donnees` — `to_iecode_json` puis
/// `decode_by_key` — et n'ouvre aucun second chemin de décodage.
///
/// # Errors
///
/// `Demande` si le conteneur n'est pas lisible, `Introuvable` si la façade ne rend pas la
/// famille [`RENDERED_FAMILY`] pour ce fichier.
pub fn decode(path: &str, bytes: &[u8]) -> Result<Vec<Line>, ErreurSite> {
    let root = nie_formats::cfgbin::to_iecode_json(bytes).ok_or_else(|| {
        ErreurSite::Demande(format!(
            "conteneur illisible : ni RDBN ni T2B — /api/v1/formats/decode/{path} dit ce que c'est"
        ))
    })?;
    let key = nie_data::typed::family_key(path);
    let (family, data) = nie_data::typed::decode_by_key(&key, &root).ok_or_else(|| {
        ErreurSite::Introuvable(format!(
            "aucune famille nommee pour la cle `{key}` ; \
             la structure generique est sur /api/v1/formats/decode/{path}"
        ))
    })?;
    if family != RENDERED_FAMILY {
        return Err(ErreurSite::Introuvable(format!(
            "`{key}` est servie sous la famille `{family}`, pas `{RENDERED_FAMILY}` : \
             sa structure est plus riche que {{hash, text}} et vit sur /api/v1/donnees/{path}"
        )));
    }
    let serde_json::Value::Array(array) = data else {
        return Err(ErreurSite::Interne(
            "la facade a rendu une famille `text` qui n'est pas une liste".to_owned(),
        ));
    };
    Ok(array.iter().filter_map(line_from).collect())
}

/// Convertit un élément `{hash, texte}` de la façade en [`Line`].
///
/// La façade nomme son champ `texte` — c'est **son** contrat, antérieur à la règle de nommage,
/// et le renommer ici casserait `/api/v1/donnees`. La traduction se fait donc à la frontière,
/// une fois, et tout ce qui sort de ce module est en anglais.
fn line_from(value: &serde_json::Value) -> Option<Line> {
    let hash = u32::try_from(value.get("hash")?.as_u64()?).ok()?;
    let text = value.get("texte")?.as_str()?.to_owned();
    Some(Line {
        hash,
        hash_hex: format!("0x{hash:08x}"),
        text,
    })
}

// ── Le catalogue mesuré ──────────────────────────────────────────────────────

/// Une famille de texte d'une langue, telle que la mesure la retient.
///
/// Ne porte **aucun texte** : c'est ce qui borne le catalogue en mémoire.
#[derive(Debug, Default)]
struct FamilySurvey {
    /// Chemins VFS qui portent cette famille pour cette langue. Presque toujours un seul —
    /// `zh_hant` en porte deux pour `mission_text` (racine + sous-dossier `mission/`, 2 lignes
    /// et 285 lignes mesurées), et choisir arbitrairement l'un des deux perdrait des lignes en
    /// silence.
    paths: Vec<String>,
    /// Nombre de lignes, tel que la façade les rend — **sans dédoublonnage** (cf. [`load`]).
    lines: usize,
    /// Taille cumulée des sources, en octets.
    bytes: u64,
}

/// Un fichier candidat qui n'entre pas dans le catalogue, et pourquoi.
#[derive(Debug, Clone, Serialize)]
pub struct Skipped {
    /// Chemin VFS du fichier écarté.
    pub path: String,
    /// La raison, telle que le décodage l'a rendue.
    pub reason: String,
}

/// Le catalogue interne : langue → famille → mesure.
#[derive(Debug, Default)]
pub struct Survey {
    languages: BTreeMap<String, BTreeMap<String, FamilySurvey>>,
    files: usize,
    lines: usize,
    bytes: u64,
    skipped: Vec<Skipped>,
    skipped_total: usize,
    catalog_bytes: usize,
    elapsed_ms: u128,
}

impl Survey {
    /// Les codes de langue mesurés, triés.
    #[must_use]
    pub fn languages(&self) -> Vec<&str> {
        self.languages.keys().map(String::as_str).collect()
    }

    /// Les chemins d'un couple (langue, famille), ou `None` si le couple n'existe pas.
    #[must_use]
    pub fn paths(&self, language: &str, family: &str) -> Option<&[String]> {
        Some(self.languages.get(language)?.get(family)?.paths.as_slice())
    }

    /// Tous les chemins d'une langue, dans l'ordre des familles.
    #[must_use]
    pub fn paths_of_language(&self, language: &str) -> Vec<String> {
        self.languages
            .get(language)
            .map_or_else(Vec::new, |families| {
                families.values().flat_map(|f| f.paths.iter().cloned()).collect()
            })
    }
}

/// Le catalogue, calculé une fois puis gardé pour la vie du processus.
static CATALOG: OnceLock<Survey> = OnceLock::new();

/// Construit la mesure en décodant réellement chaque candidat.
fn build_survey(index: &IndexVfs, vfs: &nie_formats::vfs::Vfs) -> Survey {
    let start = std::time::Instant::now();
    let (files, _) = index.page_filtree(None, &Requete::default());
    let candidates: Vec<(String, String, u32)> = files
        .into_iter()
        .filter_map(|f| {
            if !f.chemin.ends_with(SUFFIX) {
                return None;
            }
            let language = language_of(&f.chemin)?.to_owned();
            if !looks_like_text(&nie_data::typed::family_key(&f.chemin)) {
                return None;
            }
            Some((f.chemin, language, f.taille))
        })
        .collect();

    // Décodage parallèle : chaque fichier est indépendant, et le corpus fait 52,9 Mio.
    let chunks: Vec<&[(String, String, u32)]> = candidates
        .chunks(candidates.len().div_ceil(SURVEY_THREADS).max(1))
        .collect();
    let mut batches: Vec<Vec<Decoded>> = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in chunks {
            handles.push(scope.spawn(move || {
                chunk
                    .iter()
                    .map(|(path, language, size)| survey_one(vfs, path, language, *size))
                    .collect::<Vec<_>>()
            }));
        }
        for handle in handles {
            if let Ok(batch) = handle.join() {
                batches.push(batch);
            }
        }
    });

    let mut s = Survey::default();
    let mut flat: Vec<Decoded> = batches.into_iter().flatten().collect();
    flat.sort_by(|a, b| a.path.cmp(&b.path));
    for d in flat {
        match d.lines {
            Ok(lines) => {
                let family = nie_data::typed::family_key(&d.path);
                let entry = s
                    .languages
                    .entry(d.language)
                    .or_default()
                    .entry(family)
                    .or_default();
                entry.paths.push(d.path);
                entry.bytes += u64::from(d.size);
                entry.lines += lines;
                s.files += 1;
                s.lines += lines;
                s.bytes += u64::from(d.size);
            }
            Err(reason) => {
                s.skipped_total += 1;
                if s.skipped.len() < MAX_SKIPPED {
                    s.skipped.push(Skipped {
                        path: d.path,
                        reason,
                    });
                }
            }
        }
    }
    s.catalog_bytes = weigh(&s);
    s.elapsed_ms = start.elapsed().as_millis();
    tracing::info!(
        languages = s.languages.len(),
        files = s.files,
        lines = s.lines,
        catalog_bytes = s.catalog_bytes,
        ms = s.elapsed_ms,
        "catalogue de texte mesure"
    );
    s
}

/// Résultat du décodage d'un candidat, avant agrégation.
struct Decoded {
    path: String,
    language: String,
    size: u32,
    /// Nombre de lignes, ou la raison de l'écart.
    lines: Result<usize, String>,
}

/// Lit et décode un candidat, en ne gardant que son **compte** de lignes.
fn survey_one(
    vfs: &nie_formats::vfs::Vfs,
    path: &str,
    language: &str,
    size: u32,
) -> Decoded {
    let lines = match vfs.read(path) {
        Ok(bytes) => decode(path, &bytes).map(|l| l.len()).map_err(|e| e.to_string()),
        Err(e) => Err(format!("lecture VFS impossible: {e}")),
    };
    Decoded {
        path: path.to_owned(),
        language: language.to_owned(),
        size,
        lines,
    }
}

/// Poids mémoire du catalogue, en octets — les chaînes qu'il retient, plus ses structures.
///
/// Approximation **haute** et calculée, pas une estimation de couloir : chaque chemin, chaque
/// clé et chaque enregistrement sont comptés. C'est ce nombre qui est publié (152 672 octets
/// sur ce jeu, mesuré le 2026-09-06).
fn weigh(s: &Survey) -> usize {
    let mut total = std::mem::size_of::<Survey>();
    for (language, families) in &s.languages {
        total += language.len() + std::mem::size_of::<String>();
        for (family, f) in families {
            total += family.len() + std::mem::size_of::<String>();
            total += std::mem::size_of::<FamilySurvey>();
            for p in &f.paths {
                total += p.len() + std::mem::size_of::<String>();
            }
        }
    }
    for e in &s.skipped {
        total += e.path.len() + e.reason.len() + 2 * std::mem::size_of::<String>();
    }
    total
}

/// Rend le catalogue, en le construisant à la première demande.
///
/// # Errors
///
/// `Indisponible` (503) tant que le VFS n'est pas monté, ou quand le montage n'a pas de
/// contenu — c'est la capacité, pas la route, qui manque.
async fn survey(state: &EtatSite) -> Result<&'static Survey, ErreurSite> {
    if let Some(s) = CATALOG.get() {
        return Ok(s);
    }
    let index = state.index()?;
    let vfs = state.vfs()?;
    let s = tokio::task::spawn_blocking(move || CATALOG.get_or_init(|| build_survey(&index, &vfs)))
        .await?;
    Ok(s)
}

// ── DTO publics du catalogue ─────────────────────────────────────────────────

/// Une langue réellement présente dans le VFS.
#[derive(Debug, Clone, Serialize)]
pub struct TextLanguage {
    /// Code de langue, verbatim (`fr`, `ja`, `zh_hant`).
    pub language: String,
    /// Nombre de familles de texte que cette langue porte.
    pub families: usize,
    /// Nombre de fichiers.
    pub files: usize,
    /// Nombre de lignes.
    pub lines: usize,
    /// Taille cumulée des sources, en octets.
    pub bytes: u64,
}

/// Une famille de texte, vue toutes langues confondues.
#[derive(Debug, Clone, Serialize)]
pub struct TextFamily {
    /// Nom de la famille, dérivé par `nie_data::typed::family_key` (`menu_text`,
    /// `w50_npc_text`, `chara_text_roma`).
    pub family: String,
    /// Les langues qui la portent.
    pub languages: Vec<String>,
    /// Nombre de fichiers, toutes langues confondues.
    pub files: usize,
    /// Nombre de lignes, toutes langues confondues.
    pub lines: usize,
}

/// Le catalogue tel qu'il est servi.
#[derive(Debug, Clone, Serialize)]
pub struct Catalog {
    /// Les langues mesurées, par ordre de code.
    pub languages: Vec<TextLanguage>,
    /// Les familles mesurées, de la plus fournie à la moins fournie.
    pub families: Vec<TextFamily>,
    /// Nombre de fichiers retenus.
    pub files: usize,
    /// Nombre de lignes, tout le corpus.
    pub lines: usize,
    /// Taille cumulée des sources décodées, en octets.
    pub source_bytes: u64,
    /// Poids du catalogue en mémoire, en octets — **calculé**, cf. [`weigh`].
    pub catalog_bytes: usize,
    /// Durée de la mesure initiale, en millisecondes.
    pub survey_ms: u128,
    /// Nombre de candidats écartés.
    pub skipped: usize,
    /// Le détail des premiers écarts, avec leur raison.
    pub skipped_detail: Vec<Skipped>,
    /// La route qui rend les lignes d'une famille.
    pub family_route: &'static str,
    /// La route qui rend ce qu'un hash désigne.
    pub line_route: &'static str,
    /// La route de recherche.
    pub search_route: &'static str,
    /// D'où vient le décodage — nommer la source unique évite qu'on en écrive une seconde.
    pub facade: &'static str,
}

/// `GET /api/v1/text` — le catalogue mesuré.
///
/// # Errors
///
/// `503` tant que le VFS n'est pas monté.
pub async fn catalog(State(state): State<EtatSite>) -> Result<Json<Catalog>, ErreurSite> {
    let s = survey(&state).await?;
    let languages: Vec<TextLanguage> = s
        .languages
        .iter()
        .map(|(language, families)| TextLanguage {
            language: language.clone(),
            families: families.len(),
            files: families.values().map(|f| f.paths.len()).sum(),
            lines: families.values().map(|f| f.lines).sum(),
            bytes: families.values().map(|f| f.bytes).sum(),
        })
        .collect();

    let mut by_family: BTreeMap<&str, (Vec<String>, usize, usize)> = BTreeMap::new();
    for (language, families) in &s.languages {
        for (family, f) in families {
            let e = by_family
                .entry(family.as_str())
                .or_insert_with(|| (Vec::new(), 0, 0));
            e.0.push(language.clone());
            e.1 += f.paths.len();
            e.2 += f.lines;
        }
    }
    let mut families: Vec<TextFamily> = by_family
        .into_iter()
        .map(|(family, (languages, files, lines))| TextFamily {
            family: family.to_owned(),
            languages,
            files,
            lines,
        })
        .collect();
    families.sort_unstable_by(|a, b| b.lines.cmp(&a.lines).then_with(|| a.family.cmp(&b.family)));

    Ok(Json(Catalog {
        languages,
        families,
        files: s.files,
        lines: s.lines,
        source_bytes: s.bytes,
        catalog_bytes: s.catalog_bytes,
        survey_ms: s.elapsed_ms,
        skipped: s.skipped_total,
        skipped_detail: s.skipped.clone(),
        family_route: "/api/v1/text/{language}/{family}",
        line_route: "/api/v1/text/{language}/{family}/{hash}",
        search_route: "/api/v1/text/search?q={pattern}&language={language}",
        facade: "nie_data::typed::decode_by_key",
    }))
}

// ── Lignes d'une famille ─────────────────────────────────────────────────────

/// Résout les chemins d'un couple (langue, famille), ou dit précisément lequel des deux manque.
///
/// # Errors
///
/// `Introuvable` (404) — un segment d'URL désigne une ressource, pas un paramètre : c'est un
/// `404`, et le message cite ce qui existe pour que l'appelant n'ait pas à deviner.
fn resolve(s: &'static Survey, language: &str, family: &str) -> Result<Vec<String>, ErreurSite> {
    let Some(families) = s.languages.get(language) else {
        return Err(ErreurSite::Introuvable(format!(
            "langue inconnue `{language}` ; les langues mesurees dans ce jeu sont : {}",
            s.languages().join(", ")
        )));
    };
    families.get(family).map(|f| f.paths.clone()).ok_or_else(|| {
        ErreurSite::Introuvable(format!(
            "la langue `{language}` ne porte aucune famille `{family}` ; \
             ses {} familles sont sur /api/v1/text",
            families.len()
        ))
    })
}

/// Charge les lignes d'un jeu de chemins, **sans rien dédoublonner**.
///
/// La première version de cette route dédoublonnait par hash. C'était une perte silencieuse, et
/// la mesure l'a montrée avant qu'elle ne soit servie : **un hash n'identifie pas une ligne**.
/// Mesuré le 2026-09-06 sur trois fichiers, via `/api/v1/donnees` (chemin indépendant) :
///
/// | Fichier | Lignes | Hash distincts | Couples (hash, texte) distincts |
/// |---|---|---|---|
/// | `fr/menu_text` | 2 755 | 2 675 | 2 747 |
/// | `de/map/w50_npc_text` | 155 | **83** | 155 |
/// | `fr/skill_text` | 2 976 | 2 784 | 2 974 |
///
/// Sur `w50_npc_text`, dédoublonner par hash effaçait 72 chaînes **toutes différentes** — un
/// dialogue sur deux — et le compte rendu au client avait l'air parfaitement plausible. La
/// route rend donc exactement ce que la façade décode, dans l'ordre des fichiers puis des
/// nœuds : c'est aussi ce qui rend ses comptes comparables, ligne pour ligne, à ceux de
/// `/api/v1/donnees/{chemin}`.
async fn load(state: &EtatSite, paths: Vec<String>) -> Result<Vec<Line>, ErreurSite> {
    let vfs = state.vfs()?;
    tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        for path in &paths {
            let bytes = vfs.read(path).map_err(|e| {
                tracing::debug!(erreur = %e, path, "lecture VFS impossible");
                ErreurSite::Introuvable("fichier indexe mais illisible sur ce montage".to_owned())
            })?;
            out.extend(decode(path, &bytes)?);
        }
        Ok(out)
    })
    .await?
}

/// Une page de lignes, avec ce que la demande a réellement appliqué.
#[derive(Debug, Clone, Serialize)]
pub struct TextPage {
    /// Code de langue servi.
    pub language: String,
    /// Famille servie.
    pub family: String,
    /// Les fichiers VFS agrégés pour la rendre — la provenance est publiée, pas supposée.
    pub files: Vec<String>,
    /// Le motif appliqué, s'il y en avait un.
    pub q: Option<String>,
    /// Nombre de lignes de la famille **avant** filtrage.
    pub total_unfiltered: usize,
    /// La page.
    pub results: Page<Line>,
}

/// `GET /api/v1/text/{language}/{family}` — les lignes, paginées et bornées.
///
/// `?q=` filtre sans casse sur le texte, et le filtre appliqué est **republié** : un paramètre
/// accepté qui n'agit pas est le pire des défauts, parce que le client croit filtrer.
///
/// # Errors
///
/// `503` sans VFS, `404` si la langue ou la famille n'existe pas.
pub async fn family(
    State(state): State<EtatSite>,
    Path((language, family)): Path<(String, String)>,
    Query(query): Query<DemandePage>,
) -> Result<Json<TextPage>, ErreurSite> {
    let s = survey(&state).await?;
    let paths = resolve(s, &language, &family)?;
    let lines = load(&state, paths.clone()).await?;
    let total_unfiltered = lines.len();

    let pattern = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_lowercase);
    let kept: Vec<&Line> = lines
        .iter()
        .filter(|l| {
            pattern
                .as_ref()
                .is_none_or(|q| l.text.to_lowercase().contains(q))
        })
        .collect();

    let bounds = query.bornee();
    let page: Vec<Line> = kept
        .iter()
        .skip(bounds.offset())
        .take(bounds.per_page as usize)
        .map(|l| (*l).clone())
        .collect();
    let total = kept.len();
    Ok(Json(TextPage {
        language,
        family,
        files: paths,
        q: pattern,
        total_unfiltered,
        results: Page::nouvelle(page, bounds, total),
    }))
}

/// Une occurrence d'un hash : son fichier et son texte.
#[derive(Debug, Clone, Serialize)]
pub struct Occurrence {
    /// Le fichier VFS qui la porte.
    pub file: String,
    /// Le texte.
    pub text: String,
}

/// Ce qu'un hash désigne dans un couple (langue, famille).
#[derive(Debug, Clone, Serialize)]
pub struct Matches {
    /// Code de langue.
    pub language: String,
    /// Famille.
    pub family: String,
    /// Le hash demandé, normalisé en décimal.
    pub hash: u32,
    /// Le même hash en hexadécimal.
    pub hash_hex: String,
    /// Nombre d'occurrences — **il n'est pas toujours 1**, cf. la documentation de la route.
    pub total: usize,
    /// Les occurrences, dans l'ordre des fichiers puis des nœuds.
    pub occurrences: Vec<Occurrence>,
}

/// `GET /api/v1/text/{language}/{family}/{hash}` — ce qu'un hash désigne.
///
/// Le hash s'écrit en décimal ou en hexadécimal préfixé (`0xd25e0b9c`) : le dépôt cite les deux
/// formes selon qu'on lit un JSON ou un désassemblage, et exiger l'une des deux ferait porter
/// la conversion à chaque appelant.
///
/// **La route rend une liste, pas une ligne**, et ce n'est pas une précaution de principe : sur
/// `data/common/text/de/map/w50_npc_text.cfg.bin`, 155 lignes ne portent que **83 hash
/// distincts** (mesuré, cf. [`load`]), et `0x2d909dd6` désigne **70** textes dans
/// `fr/menu_text`. Rendre « la » ligne aurait servi la première venue avec l'assurance d'un
/// accès par clé.
///
/// # Errors
///
/// `400` sur un hash mal formé, `404` sur une langue, une famille ou un hash absents.
pub async fn line(
    State(state): State<EtatSite>,
    Path((language, family, hash)): Path<(String, String, String)>,
) -> Result<Json<Matches>, ErreurSite> {
    let target = parse_hash(&hash)?;
    let s = survey(&state).await?;
    let paths = resolve(s, &language, &family)?;
    let vfs = state.vfs()?;
    let occurrences = tokio::task::spawn_blocking(move || {
        let mut found = Vec::new();
        for path in &paths {
            let Ok(bytes) = vfs.read(path) else {
                continue;
            };
            let Ok(lines) = decode(path, &bytes) else {
                continue;
            };
            found.extend(
                lines
                    .into_iter()
                    .filter(|l| l.hash == target)
                    .map(|l| Occurrence {
                        file: path.clone(),
                        text: l.text,
                    }),
            );
        }
        found
    })
    .await?;

    if occurrences.is_empty() {
        return Err(ErreurSite::Introuvable(format!(
            "aucune ligne de hash {target} (0x{target:08x}) dans `{family}` en `{language}`"
        )));
    }
    Ok(Json(Matches {
        language,
        family,
        hash: target,
        hash_hex: format!("0x{target:08x}"),
        total: occurrences.len(),
        occurrences,
    }))
}

// ── Recherche ────────────────────────────────────────────────────────────────

/// Ce que la recherche accepte.
///
/// **Champs à plat, jamais `#[serde(flatten)]`** : avec lui, la désérialisation d'une query
/// string passe par un tampon où toute valeur est une chaîne, et `?per_page=2` échoue en
/// « invalid type: string "2", expected u32 » — un `400` sur une requête valide, invisible à la
/// lecture des deux structures. Piège déjà payé sur `routes::recherche`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchQuery {
    /// Motif cherché, sans casse, dans le texte. Obligatoire, au moins [`MIN_PATTERN`]
    /// caractères.
    pub q: Option<String>,
    /// Langue à balayer. **Obligatoire** : sans elle, une recherche décoderait les 52,9 Mio des
    /// neuf langues à chaque appel, pour rendre neuf fois la même ligne traduite.
    pub language: Option<String>,
    /// Numéro de page, à partir de 1.
    pub page: Option<u32>,
    /// Nombre d'éléments par page, plafonné à [`crate::config::PER_PAGE_MAX`].
    pub per_page: Option<u32>,
}

/// Une ligne trouvée : elle porte sa famille, sinon elle n'est pas adressable.
#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    /// Famille où la ligne a été trouvée.
    pub family: String,
    /// Fichier VFS d'origine.
    pub file: String,
    /// Hash de la ligne.
    pub hash: u32,
    /// Le même hash en hexadécimal.
    pub hash_hex: String,
    /// Le texte.
    pub text: String,
}

/// Le résultat d'une recherche.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResults {
    /// Le motif réellement appliqué (rogné, en minuscules).
    pub q: String,
    /// La langue balayée.
    pub language: String,
    /// Nombre de fichiers décodés pour répondre.
    pub files_read: usize,
    /// `true` quand le balayage s'est arrêté à [`MAX_RESULTS`] : le total est alors un
    /// plancher, pas un total. Le taire donnerait un compte faux qui a l'air juste.
    pub truncated: bool,
    /// La page de résultats.
    pub results: Page<Hit>,
}

/// `GET /api/v1/text/search` — une sous-chaîne dans le texte d'une langue.
///
/// C'est ce qui remplace les `getText*` du wiki : chercher un libellé sans savoir dans quelle
/// famille il vit, et récupérer de quoi l'adresser (famille + hash).
///
/// # Errors
///
/// `400` si `q` manque, est trop court, si `language` manque ou est inconnue ; `503` sans VFS.
pub async fn search(
    State(state): State<EtatSite>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResults>, ErreurSite> {
    let s = survey(&state).await?;

    let pattern = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .ok_or_else(|| {
            ErreurSite::Demande(
                "parametre `q` obligatoire : la sous-chaine a chercher dans le texte".to_owned(),
            )
        })?
        .to_lowercase();
    if pattern.chars().count() < MIN_PATTERN {
        return Err(ErreurSite::Demande(format!(
            "motif trop court : au moins {MIN_PATTERN} caracteres (un motif d'un caractere \
             balaie tout le corpus pour ne rendre que la borne de {MAX_RESULTS} resultats)"
        )));
    }

    let language = query
        .language
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .ok_or_else(|| {
            ErreurSite::Demande(format!(
                "parametre `language` obligatoire ; les langues mesurees sont : {}",
                s.languages().join(", ")
            ))
        })?
        .to_owned();
    if !s.languages.contains_key(&language) {
        return Err(ErreurSite::Demande(format!(
            "langue inconnue `{language}` ; les langues mesurees sont : {}",
            s.languages().join(", ")
        )));
    }

    let paths = s.paths_of_language(&language);
    let vfs = state.vfs()?;
    let needle = pattern.clone();
    let (hits, read, truncated) = tokio::task::spawn_blocking(move || {
        let mut out: Vec<Hit> = Vec::new();
        let mut read = 0usize;
        let mut truncated = false;
        for path in &paths {
            if out.len() >= MAX_RESULTS {
                truncated = true;
                break;
            }
            let Ok(bytes) = vfs.read(path) else {
                continue;
            };
            let Ok(lines) = decode(path, &bytes) else {
                continue;
            };
            read += 1;
            let family = nie_data::typed::family_key(path);
            for l in lines {
                if l.text.to_lowercase().contains(&needle) {
                    out.push(Hit {
                        family: family.clone(),
                        file: path.clone(),
                        hash: l.hash,
                        hash_hex: l.hash_hex,
                        text: l.text,
                    });
                    if out.len() >= MAX_RESULTS {
                        truncated = true;
                        break;
                    }
                }
            }
        }
        (out, read, truncated)
    })
    .await?;

    let bounds = DemandePage {
        page: query.page,
        per_page: query.per_page,
        q: None,
    }
    .bornee();
    let total = hits.len();
    let page: Vec<Hit> = hits
        .into_iter()
        .skip(bounds.offset())
        .take(bounds.per_page as usize)
        .collect();
    Ok(Json(SearchResults {
        q: pattern,
        language,
        files_read: read,
        truncated,
        results: Page::nouvelle(page, bounds, total),
    }))
}

// ── Traduction ───────────────────────────────────────────────────────────────

/// Nombre maximal de langues cibles par appel.
///
/// Le jeu en porte dix ; en demander plus que quatre à la fois fait décoder tout le corpus de
/// chacune pour un résultat qu'aucune interface n'affiche.
pub const TARGETS_MAX: usize = 4;

/// Nombre maximal de correspondances traduites, avant pagination.
///
/// Plus bas que [`MAX_RESULTS`] : chaque correspondance coûte un balayage par langue cible.
pub const TRANSLATE_MAX: usize = 200;

/// Ce que la traduction accepte. Champs à plat, jamais `flatten` (cf. [`SearchQuery`]).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TranslateQuery {
    /// Le terme cherché, dans la langue `from`. Au moins [`MIN_PATTERN`] caractères.
    pub q: Option<String>,
    /// La langue de départ. Obligatoire.
    pub from: Option<String>,
    /// Les langues d'arrivée, séparées par des virgules. Par défaut : **toutes** les autres,
    /// plafonnées à [`TARGETS_MAX`].
    pub to: Option<String>,
    /// Numéro de page.
    pub page: Option<u32>,
    /// Taille de page.
    pub per_page: Option<u32>,
}

/// Le même terme dans une autre langue.
#[derive(Debug, Clone, Serialize)]
pub struct Rendering {
    /// Le code de langue.
    pub language: String,
    /// Les textes trouvés pour ce hash dans cette langue. **Une liste**, parce qu'un hash ne
    /// désigne pas toujours une ligne unique.
    pub texts: Vec<String>,
}

/// Une correspondance alignée entre langues.
#[derive(Debug, Clone, Serialize)]
pub struct Translation {
    /// La famille où le terme a été trouvé — c'est elle qui rend la ligne adressable.
    pub family: String,
    /// Le hash de la ligne.
    pub hash: u32,
    /// Le même hash en hexadécimal.
    pub hash_hex: String,
    /// Le texte source.
    pub source: String,
    /// Les rendus dans les langues demandées.
    pub renderings: Vec<Rendering>,
    /// `true` quand le hash désigne **plusieurs** lignes d'un côté ou de l'autre : l'alignement
    /// n'est alors pas certain, et le taire donnerait une traduction qui a l'air sûre.
    pub ambiguous: bool,
}

/// Le résultat d'une traduction.
#[derive(Debug, Clone, Serialize)]
pub struct TranslateResults {
    /// Le motif réellement appliqué.
    pub q: String,
    /// La langue de départ.
    pub from: String,
    /// Les langues d'arrivée réellement balayées.
    pub to: Vec<String>,
    /// `true` quand le balayage s'est arrêté à [`TRANSLATE_MAX`].
    pub truncated: bool,
    /// La page de correspondances.
    pub results: Page<Translation>,
}

/// `GET /api/v1/text/translate` — le même terme d'une langue à l'autre, par le texte du jeu.
///
/// # Ce que ça remplace, et pourquoi c'est mieux fondé
///
/// L'outil « Traducteur » d'Azalée interroge **sept tables** `inagle_*` avec une normalisation
/// kana↔romaji et un score flou. Il traduit des noms de fiches de wiki. Cette route-ci traduit
/// le **texte du jeu**, en s'appuyant sur ce qui aligne réellement deux langues dans les
/// fichiers : le **hash**. Elle ne devine rien — deux textes ne se correspondent que s'ils
/// portent le même hash dans la même famille.
///
/// # Ce qu'elle ne promet pas
///
/// Un hash n'identifie pas une ligne : sur `de/map/w50_npc_text`, 155 lignes ne portent que 83
/// hash distincts (mesuré, cf. [`load`]). Quand un hash désigne plusieurs lignes, la route rend
/// **toutes** les occurrences et lève `ambiguous` plutôt que d'en choisir une par son rang.
///
/// # Errors
///
/// `400` si `q` manque ou est trop court, si `from` manque ou est inconnue, si une langue de
/// `to` est inconnue ; `503` sans VFS.
pub async fn translate(
    State(state): State<EtatSite>,
    Query(query): Query<TranslateQuery>,
) -> Result<Json<TranslateResults>, ErreurSite> {
    let s = survey(&state).await?;

    let pattern = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .ok_or_else(|| {
            ErreurSite::Demande("parametre `q` obligatoire : le terme a traduire".to_owned())
        })?
        .to_lowercase();
    if pattern.chars().count() < MIN_PATTERN {
        return Err(ErreurSite::Demande(format!(
            "motif trop court : au moins {MIN_PATTERN} caracteres"
        )));
    }

    let from = query
        .from
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .ok_or_else(|| {
            ErreurSite::Demande(format!(
                "parametre `from` obligatoire ; les langues mesurees sont : {}",
                s.languages().join(", ")
            ))
        })?
        .to_owned();
    if !s.languages.contains_key(&from) {
        return Err(ErreurSite::Demande(format!(
            "langue inconnue `{from}` ; les langues mesurees sont : {}",
            s.languages().join(", ")
        )));
    }

    let to: Vec<String> = match query.to.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(liste) => {
            let demandees: Vec<String> = liste
                .split(',')
                .map(|l| l.trim().to_owned())
                .filter(|l| !l.is_empty())
                .collect();
            for l in &demandees {
                if !s.languages.contains_key(l) {
                    return Err(ErreurSite::Demande(format!(
                        "langue d'arrivee inconnue `{l}` ; les langues mesurees sont : {}",
                        s.languages().join(", ")
                    )));
                }
            }
            if demandees.len() > TARGETS_MAX {
                return Err(ErreurSite::Demande(format!(
                    "trop de langues d'arrivee : {} (borne {TARGETS_MAX})",
                    demandees.len()
                )));
            }
            demandees
        }
        None => s
            .languages()
            .into_iter()
            .filter(|l| *l != from)
            .take(TARGETS_MAX)
            .map(str::to_owned)
            .collect(),
    };

    let paths = s.paths_of_language(&from);
    let cibles = to.clone();
    let cibles_paths: Vec<(String, Vec<String>)> = cibles
        .iter()
        .map(|l| (l.clone(), s.paths_of_language(l)))
        .collect();
    let vfs = state.vfs()?;
    let needle = pattern.clone();

    let (translations, truncated) = tokio::task::spawn_blocking(move || {
        // 1. Trouver les (famille, hash) qui portent le terme dans la langue de depart.
        let mut trouves: Vec<(String, u32, String)> = Vec::new();
        let mut truncated = false;
        for path in &paths {
            if trouves.len() >= TRANSLATE_MAX {
                truncated = true;
                break;
            }
            let Ok(bytes) = vfs.read(path) else { continue };
            let Ok(lines) = decode(path, &bytes) else {
                continue;
            };
            let family = nie_data::typed::family_key(path);
            for l in lines {
                if l.text.to_lowercase().contains(&needle) {
                    trouves.push((family.clone(), l.hash, l.text));
                    if trouves.len() >= TRANSLATE_MAX {
                        truncated = true;
                        break;
                    }
                }
            }
        }

        // 2. Pour chaque langue cible, relever les textes des memes (famille, hash).
        //    Un seul balayage par langue, pas un par correspondance.
        let voulus: std::collections::HashSet<(String, u32)> = trouves
            .iter()
            .map(|(f, h, _)| (f.clone(), *h))
            .collect();
        let mut par_langue: BTreeMap<String, BTreeMap<(String, u32), Vec<String>>> =
            BTreeMap::new();
        for (langue, chemins) in &cibles_paths {
            let table = par_langue.entry(langue.clone()).or_default();
            for path in chemins {
                let family = nie_data::typed::family_key(path);
                if !voulus.iter().any(|(f, _)| *f == family) {
                    continue;
                }
                let Ok(bytes) = vfs.read(path) else { continue };
                let Ok(lines) = decode(path, &bytes) else {
                    continue;
                };
                for l in lines {
                    let cle = (family.clone(), l.hash);
                    if voulus.contains(&cle) {
                        table.entry(cle).or_default().push(l.text);
                    }
                }
            }
        }

        // 3. Compter les occurrences du hash dans la langue de depart, pour `ambiguous`.
        let mut occurrences: BTreeMap<(String, u32), usize> = BTreeMap::new();
        for (f, h, _) in &trouves {
            *occurrences.entry((f.clone(), *h)).or_insert(0) += 1;
        }

        let out: Vec<Translation> = trouves
            .into_iter()
            .map(|(family, hash, source)| {
                let cle = (family.clone(), hash);
                let renderings: Vec<Rendering> = cibles
                    .iter()
                    .map(|langue| Rendering {
                        language: langue.clone(),
                        texts: par_langue
                            .get(langue)
                            .and_then(|t| t.get(&cle))
                            .cloned()
                            .unwrap_or_default(),
                    })
                    .collect();
                let ambiguous = occurrences.get(&cle).copied().unwrap_or(0) > 1
                    || renderings.iter().any(|r| r.texts.len() > 1);
                Translation {
                    family,
                    hash,
                    hash_hex: format!("0x{hash:08x}"),
                    source,
                    renderings,
                    ambiguous,
                }
            })
            .collect();
        (out, truncated)
    })
    .await?;

    let bounds = DemandePage {
        page: query.page,
        per_page: query.per_page,
        q: None,
    }
    .bornee();
    let total = translations.len();
    let page: Vec<Translation> = translations
        .into_iter()
        .skip(bounds.offset())
        .take(bounds.per_page as usize)
        .collect();
    Ok(Json(TranslateResults {
        q: pattern,
        from,
        to,
        truncated,
        results: Page::nouvelle(page, bounds, total),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn language_is_the_first_segment_after_the_root() {
        assert_eq!(language_of("data/common/text/fr/menu_text.cfg.bin"), Some("fr"));
        assert_eq!(
            language_of("data/common/text/zh_hant/mission/mission_text.cfg.bin"),
            Some("zh_hant")
        );
        // Hors racine, ou sans fichier derriere la langue : rien. Un chemin qui ne PORTE pas de
        // langue ne doit pas en inventer une.
        assert_eq!(language_of("data/common/chara/chara_text.cfg.bin"), None);
        assert_eq!(language_of("data/common/text/fr"), None);
        assert_eq!(language_of("data/common/text/fr/"), None);
    }

    #[test]
    fn the_preselection_mirrors_the_facade_dispatch() {
        // Preuve par falsification : la moitie positive seule passerait aussi sur un filtre qui
        // dit oui a tout.
        let k = nie_data::typed::family_key;
        assert!(looks_like_text(&k("data/common/text/fr/menu_text.cfg.bin")));
        assert!(looks_like_text(&k(
            "data/common/text/fr/map/w50_npc_text.cfg.bin"
        )));
        // Les deux `*_text_roma` ne finissent PAS par `_text` : c'est le catalogue TEXT_FILES
        // qui les rattrape, et sans lui 18 fichiers du corpus disparaissaient en silence.
        assert!(looks_like_text("chara_text_roma"));
        assert!(looks_like_text("map_text_roma"));
        assert!(!"chara_text_roma".ends_with("_text"));
        // Ce qui vit sous la racine sans etre une table de texte reste dehors.
        assert!(!looks_like_text("system_text_map"));
        assert!(!looks_like_text("common_talk_text_map"));
        assert!(!looks_like_text("chara_base"));
    }

    #[test]
    fn a_hash_reads_in_decimal_and_in_hexadecimal() {
        assert_eq!(parse_hash("3528663132").unwrap(), 3_528_663_132);
        assert_eq!(parse_hash("0xd25e0b9c").unwrap(), 0xd25e_0b9c);
        assert_eq!(parse_hash("0XD25E0B9C").unwrap(), 0xd25e_0b9c);
        assert_eq!(parse_hash("  42 ").unwrap(), 42);
    }

    #[test]
    fn a_malformed_hash_is_a_400_naming_both_forms() {
        for raw in ["", "1e5", "0x", "d25e0b9c", "-1", "4294967296", "0xffffffff0"] {
            let e = parse_hash(raw).unwrap_err();
            assert_eq!(e.statut().as_u16(), 400, "`{raw}` doit etre refuse");
            assert!(format!("{e}").contains("hexadecimal"), "{e}");
        }
    }

    #[test]
    fn the_hexadecimal_hash_is_serialised_on_eight_digits() {
        // `format!("{:?}")` n'est pas une serialisation, et un hex de longueur variable force
        // chaque client a normaliser : la forme est choisie ici, une fois.
        let l = line_from(&serde_json::json!({ "hash": 42, "texte": "x" })).unwrap();
        assert_eq!(l.hash_hex, "0x0000002a");
        assert_eq!(l.text, "x");
        // Un element mal forme est ecarte, pas transforme en ligne vide.
        assert!(line_from(&serde_json::json!({ "hash": 42 })).is_none());
        assert!(line_from(&serde_json::json!({ "texte": "x" })).is_none());
        assert!(line_from(&serde_json::json!(3)).is_none());
    }

    #[test]
    fn a_repeated_hash_with_different_texts_keeps_every_line() {
        // Le defaut que cette route a failli servir : dedoublonner par hash. Mesure du
        // 2026-09-06, par `/api/v1/donnees` (chemin independant) —
        // `data/common/text/de/map/w50_npc_text.cfg.bin` porte 155 lignes pour 83 hash
        // distincts, et ses 155 couples (hash, texte) sont TOUS differents : dedoublonner y
        // effacait 72 dialogues, avec un compte parfaitement plausible en sortie.
        let array = serde_json::json!([
            { "hash": 7, "texte": "un" },
            { "hash": 7, "texte": "deux" },
            { "hash": 7, "texte": "trois" },
        ]);
        let lines: Vec<Line> = array
            .as_array()
            .expect("tableau")
            .iter()
            .filter_map(line_from)
            .collect();
        assert_eq!(lines.len(), 3, "aucune ligne ne doit disparaitre");
        assert_eq!(
            lines.iter().map(|l| l.hash).collect::<Vec<_>>(),
            vec![7, 7, 7]
        );
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, vec!["un", "deux", "trois"], "ordre preserve");
    }

    #[test]
    fn an_unreadable_container_points_back_to_the_generic_decoder() {
        let e = decode("data/common/text/fr/menu_text.cfg.bin", b"pas un cfg.bin").unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("/api/v1/formats/decode"), "{e}");
    }

    #[test]
    fn the_facade_does_render_the_text_family_for_the_families_we_advertise() {
        // Ce que la route sert repose entierement sur ce bras de dispatch : s'il disparait, la
        // route rend 404 partout. On le verifie ici, et on verifie aussi qu'il ne dit pas oui
        // a tout.
        let empty = serde_json::json!({ "entries": [], "lists": [] });
        for key in ["menu_text", "item_text", "skill_text", "w50_npc_text"] {
            let (family, _) = nie_data::typed::decode_by_key(key, &empty)
                .unwrap_or_else(|| panic!("`{key}` doit etre couverte par la facade"));
            assert_eq!(family, RENDERED_FAMILY);
        }
        // `chara_description_text` part a dessein sur son parseur propre : elle sort sous une
        // AUTRE famille, et c'est pour cela qu'elle n'est pas servie ici.
        let (other, _) = nie_data::typed::decode_by_key("chara_description_text", &empty)
            .expect("famille dediee presente");
        assert_ne!(other, RENDERED_FAMILY);
        assert!(nie_data::typed::decode_by_key("famille_inventee_2026", &empty).is_none());
    }

    /// Un état sans VFS : le montage est `EnCours`, comme au démarrage du service.
    fn state_without_vfs() -> EtatSite {
        EtatSite::nouveau(Config::default())
    }

    #[tokio::test]
    async fn without_a_vfs_the_four_routes_answer_503_not_an_empty_catalog() {
        // Capacite absente = 503 explicite. Un catalogue vide servi en 200 ferait croire que
        // ce jeu n'a pas de texte.
        let state = state_without_vfs();
        let e = catalog(State(state.clone())).await.expect_err("503");
        assert_eq!(e.statut().as_u16(), 503);

        let e = family(
            State(state.clone()),
            Path(("fr".to_owned(), "menu_text".to_owned())),
            Query(DemandePage::default()),
        )
        .await
        .expect_err("503");
        assert_eq!(e.statut().as_u16(), 503);

        let e = line(
            State(state.clone()),
            Path(("fr".to_owned(), "menu_text".to_owned(), "1".to_owned())),
        )
        .await
        .expect_err("503");
        assert_eq!(e.statut().as_u16(), 503);

        let e = search(
            State(state),
            Query(SearchQuery {
                q: Some("test".to_owned()),
                language: Some("fr".to_owned()),
                ..SearchQuery::default()
            }),
        )
        .await
        .expect_err("503");
        assert_eq!(e.statut().as_u16(), 503);
    }

    #[tokio::test]
    async fn an_invalid_hash_is_refused_before_touching_the_vfs() {
        // L'ordre compte : sur un service sans VFS, un hash invalide doit rendre 400 (la
        // demande est fautive) et non 503 (la capacite manque). Sinon le client corrige la
        // mauvaise chose.
        let e = line(
            State(state_without_vfs()),
            Path((
                "fr".to_owned(),
                "menu_text".to_owned(),
                "pas_un_hash".to_owned(),
            )),
        )
        .await
        .expect_err("400");
        assert_eq!(e.statut().as_u16(), 400);
    }

    #[test]
    fn a_search_query_with_numbers_deserialises() {
        // Non-regression du piege `#[serde(flatten)]` : avec lui, `per_page=25` echouait en
        // « invalid type: string "25", expected u32 » sur une requete parfaitement valide.
        let d: SearchQuery = query_witness("q=coup&language=fr&page=3&per_page=25");
        assert_eq!(d.q.as_deref(), Some("coup"));
        assert_eq!(d.language.as_deref(), Some("fr"));
        assert_eq!(d.page, Some(3));
        assert_eq!(d.per_page, Some(25));
    }

    #[test]
    fn the_search_pagination_is_bounded_by_the_config() {
        let bounds = DemandePage {
            page: Some(1),
            per_page: Some(100_000),
            q: None,
        }
        .bornee();
        assert_eq!(bounds.per_page, crate::config::PER_PAGE_MAX);
    }

    /// Désérialise comme axum le fait pour `Query<T>`, depuis une query string.
    fn query_witness(qs: &str) -> SearchQuery {
        let map: serde_json::Map<String, serde_json::Value> = qs
            .split('&')
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| {
                let val = v.parse::<u64>().map_or_else(
                    |_| serde_json::Value::String(v.to_owned()),
                    |n| serde_json::Value::Number(n.into()),
                );
                (k.to_owned(), val)
            })
            .collect();
        serde_json::from_value(serde_json::Value::Object(map)).expect("demande valide")
    }

    #[test]
    fn the_catalog_weight_is_computed_and_grows_with_its_content() {
        // Le poids publie doit etre une mesure, pas une constante : on le prouve en le voyant
        // bouger quand on ajoute une entree.
        let mut s = Survey::default();
        let empty = weigh(&s);
        s.languages.entry("fr".to_owned()).or_default().insert(
            "menu_text".to_owned(),
            FamilySurvey {
                paths: vec!["data/common/text/fr/menu_text.cfg.bin".to_owned()],
                lines: 2755,
                bytes: 262_144,
            },
        );
        assert!(weigh(&s) > empty, "le poids doit croitre avec le contenu");
        assert_eq!(s.languages(), vec!["fr"]);
        assert_eq!(
            s.paths("fr", "menu_text").map(<[String]>::len),
            Some(1),
            "le couple existe"
        );
        assert!(s.paths("de", "menu_text").is_none(), "langue absente");
        assert!(s.paths("fr", "absente").is_none(), "famille absente");
        assert_eq!(s.paths_of_language("fr").len(), 1);
        assert!(s.paths_of_language("xx").is_empty());
    }
}
