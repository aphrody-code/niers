//! `/api/v1/inspect` — six décodeurs de `nie-formats` qu'aucune route n'appelait.
//!
//! ## Pourquoi ce module existe
//!
//! `routes::formats`, `routes::geometrie` et `routes::level5` servent les familles qui se
//! reconnaissent à un **suffixe** : un chemin entre, une structure sort. Six modules de
//! `nie-formats` restaient hors de cette grille parce qu'ils ne décodent pas un format — ils
//! **interprètent** une structure déjà décodée (`sprite_sheet` lit un `G4tx`, `font` lit un
//! `CfgBinFile`, `menu` combine un `objbin` et un `g4pkm`) ou ils **mesurent** des pixels
//! qu'aucun chemin ne porte (`planche`, `imgmetric`). Un septième, `nxtch`, décrit un
//! conteneur que ce build du jeu n'utilise pas.
//!
//! Écrits, documentés, testés dans `nie-formats` — et invisibles depuis le web. C'est
//! exactement ce que la matrice de couverture appelle `manquant` : le code est là, la route
//! manque.
//!
//! | Module | Route | Ce qu'il lit | Ce qu'il rend |
//! |---|---|---|---|
//! | `sprite_sheet` | `/api/v1/inspect/spritesheet/{*path}` | un `.g4tx` | manifeste JSON, feuille CSS, feuille SVG |
//! | `font` | `/api/v1/inspect/font/{*path}` | un `font.cfg.bin` (T2B) | métriques de glyphes, paginées |
//! | `menu` | `/api/v1/inspect/menu/{*path}` | un `.objbin` **et** son `.g4pkm` | géométrie écran, priorités, points d'attache |
//! | `nxtch` | `/api/v1/inspect/texture-chunk/{*path}` | un `.g4tx` ou un bloc NXTCH | en-tête, tailles, tampon délinéarisé |
//! | `imgmetric` | `/api/v1/inspect/color`, `/api/v1/inspect/compare` | deux couleurs, deux tampons RGBA | ΔE2000, rapport T0/T1/T2 |
//! | `planche` | `/api/v1/inspect/plate` | un tampon RGBA et son masque | zones, rôle, convention de composition |
//!
//! ## Ce que ce module NE promet pas
//!
//! **Il ne décode aucun pixel.** `nie-site` compile `nie-formats` avec `std`, `lua` et `serde`
//! seulement ; `textures`, `images` et `audio-decode` restent éteintes (un test de
//! `routes::formats` le verrouille). Trois conséquences, dites plutôt que subies :
//!
//! 1. la feuille **SVG** référence son atlas par **URL**, pas par une `data:` — la variante
//!    autonome de `nie_formats::sprite_sheet::data_uri` exige les pixels encodés, que ce
//!    service n'a pas. L'URL par défaut pointe vers l'amont, qui, lui, sait décoder ;
//! 2. `planche::analyser` (qui prend un `.g4tx` entier) est derrière `textures` : **elle n'est
//!    pas appelée ici**. Seul `planche::mesurer` l'est, sur un tampon RGBA que l'appelant
//!    fournit — d'où un `POST`, et non un chemin VFS qui ferait croire à une lecture du jeu ;
//! 3. `imgmetric` compare deux tampons **fournis**. Ce module ne rend jamais un score contre
//!    une capture du jeu : il n'en produit aucune.
//!
//! ## Le corpus NXTCH est vide, et la route le dit
//!
//! Mesuré le 2026-09-06 sur ce montage : `niers vfs find 'nxtch'` rend **0 résultat**, et le
//! payload d'un `.g4tx` de ce build est du **DDS** — vérifié sur
//! `data/dx11/chr/_animal/an000100/an000100.g4tx` (3 498 240 o) : `grep -c NXTCH` = 0,
//! `grep -ac 'DDS '` = 6. NXTCH est le conteneur de texture **Switch** (tuiles GOB Tegra X1) ;
//! la version PC n'en embarque pas.
//!
//! La route existe quand même, et c'est délibéré : le parseur est écrit et validé, un `.g4tx`
//! d'un autre build en porterait, et **une capacité qu'on ne peut pas appeler ne se distingue
//! pas d'une capacité absente**. Elle annonce donc son corpus (`corpus: 0`) au lieu de laisser
//! croire à une panne au premier `400`.
//!
//! ## Nommage
//!
//! Écrit sous la règle du **2026-09-06** : identifiants, URLs et clés JSON en **anglais**,
//! prose en français (cf. `CLAUDE.md` § *Langue*, et `routes::text`, le module de référence).
//! Une seule exception, documentée à son point d'emploi : `?form=json` republie **verbatim**
//! le manifeste de `nie_formats::sprite_sheet::vers_json`, dont les clés sont françaises —
//! c'est une API **déjà servie** (`nie_wasm::g4tx_sprite_sheet_json`, la CLI `niers`,
//! `apps/inacord`), et une API déjà servie ne se renomme pas au passage.
//!
//! Aucun `format!("{:?}")` n'entre dans une réponse : chaque énumération de `nie-formats` est
//! traduite par un `match` vers un jeton choisi, figé par un test.

use std::sync::{Arc, RwLock};

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use nie_formats::{
    cfgbin, font, g4pkm, g4tx, imgmetric, menu as menu_layout, nxtch, objbin, planche, sprite_sheet,
};

use crate::error::ErreurSite;
use crate::routes::{DemandePage, Page};
use crate::state::EtatSite;
use crate::vfs_index::{DemandeFiltre, IndexVfs};

// ─── Bornes, toutes mesurées ────────────────────────────────────────────────────────────────

/// Taille au-delà de laquelle une source du VFS n'est pas lue pour être inspectée.
///
/// Mesuré le 2026-09-06 (`niers vfs find '.g4tx' -n 100000`, agrégé en `awk`) : le VFS porte
/// **54 203** `.g4tx`, dont le plus gros pèse **347 230 704 octets** — un atlas de carte
/// (`data/dx11/map/s/s38g001/s38g001g.g4tx`). Lire cela pour en extraire une liste de
/// rectangles serait payer 331 Mio de RAM pour quelques kilo-octets de sortie. La borne de
/// 16 Mio couvre **53 820 fichiers, soit 99,29 %** du corpus, et les 383 autres reçoivent un
/// `400` qui **dit leur taille et la borne** au lieu d'un délai inexpliqué.
///
/// Les deux autres corpus tiennent très en deçà : `.objbin` plafonne à 15 024 octets (12 190
/// fichiers) et le plus gros `font.cfg.bin` à 941 968 octets (10 fichiers).
pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// Nombre maximal de pixels d'un tampon reçu en `POST`.
///
/// Ce n'est **pas** la borne qui se déclenche en premier : le routeur d'axum applique une
/// limite de corps de 2 Mio, et deux tampons base64 la dépassent bien avant (2 × p × 4 × 4/3
/// octets ⇒ p ≈ 196 608 pixels). La borne est ici pour le cas que la limite de corps ne voit
/// pas — un corps minuscule qui **déclare** `width: 100000, height: 100000`. Sans elle,
/// `imgmetric::comparer` allouerait deux `Vec<[f64; 3]>` de 10^10 éléments avant de
/// s'apercevoir que les tampons ne suivent pas. On refuse d'abord, on alloue ensuite.
pub const MAX_BUFFER_PIXELS: usize = 196_608;

/// Nombre maximal de régions d'intérêt acceptées par une comparaison.
pub const MAX_REGIONS: usize = 64;

/// Longueur maximale du nom d'une région — il est repris tel quel dans la réponse.
pub const MAX_REGION_NAME: usize = 64;

/// Nombre maximal de réductions 2× successives avant comparaison.
pub const MAX_DOWNSCALE: u32 = 4;

/// Nombre maximal de candidats examinés quand un compagnon se résout par nom de fichier.
pub const MAX_COMPANION_CANDIDATES: usize = 256;

/// Suffixe des atlas de textures du jeu.
pub const ATLAS_SUFFIX: &str = ".g4tx";

/// Suffixe des objets de menu.
pub const MENU_SUFFIX: &str = ".objbin";

/// Suffixe des tables de métriques de police.
///
/// Le suffixe porte le `/` : `font_color.cfg.bin` n'est pas une table de glyphes, et le
/// compter en ferait un onzième fichier qui rendrait un `FontMetrics` vide en annonçant un
/// succès. Mesuré le 2026-09-06 : **10** chemins finissent par `/font.cfg.bin`, tous sous
/// `data/common/font/font/<variante>/`.
pub const FONT_SUFFIX: &str = "/font.cfg.bin";

/// Suffixe d'un bloc NXTCH autonome. **Aucun fichier du VFS n'en porte** — cf. la doc de
/// module.
pub const CHUNK_SUFFIX: &str = ".nxtch";

/// Jeton de locale que les chemins logiques du jeu portent (`.../<LG>/x.g4tx`).
pub const LOCALE_PLACEHOLDER: &str = "<LG>";

/// Locale employée par défaut pour résoudre un compagnon localisé.
///
/// C'est celle de `nie-game` (`MENU_LOCALE`), pour que deux services qui lisent le même objet
/// de menu ne choisissent pas deux fichiers différents.
pub const DEFAULT_LOCALE: &str = "fr";

/// Les segments de chemin qui sont des tags de locale, et non des dossiers de contenu.
///
/// Repris de `nie-game::is_locale_tag` : un compagnon dont le dossier parent n'est **pas** un
/// tag de locale est un compagnon non localisé, et il prime sur toute variante traduite.
pub const LOCALE_TAGS: [&str; 11] = [
    "de", "en", "es", "fr", "it", "pt", "ja", "ko", "zh_hans", "zh_hant", "common",
];

/// Vrai si `segment` est un tag de locale.
#[must_use]
pub fn is_locale_tag(segment: &str) -> bool {
    LOCALE_TAGS.contains(&segment)
}

// ─── Lecture, réponses, tampons ─────────────────────────────────────────────────────────────

/// Lit une source du VFS, bornée par [`MAX_SOURCE_BYTES`].
///
/// L'ordre des vérifications est celui des coûts : l'index d'abord (binaire, gratuit), la
/// taille déclarée ensuite (gratuite aussi), la lecture enfin. Refuser une source de 331 Mio
/// **après** l'avoir lue serait la lire pour rien.
///
/// # Errors
///
/// `Introuvable` quand le chemin n'est pas indexé ou n'est pas lisible sur ce montage ;
/// `Demande` quand la source dépasse la borne ; `Indisponible` tant que le VFS n'est pas monté.
pub async fn read_source(state: &EtatSite, path: &str) -> Result<Vec<u8>, ErreurSite> {
    let index = state.index()?;
    if !index.contient(path) {
        return Err(ErreurSite::Introuvable(format!(
            "chemin absent du VFS: {path}"
        )));
    }
    if let Some(size) = index.taille(path) {
        let size = usize::try_from(size).unwrap_or(usize::MAX);
        if size > MAX_SOURCE_BYTES {
            return Err(ErreurSite::Demande(format!(
                "source trop volumineuse pour une inspection ({size} octets, borne {MAX_SOURCE_BYTES})"
            )));
        }
    }
    let vfs = state.vfs()?;
    let to_read = path.to_owned();
    tokio::task::spawn_blocking(move || vfs.read(&to_read))
        .await?
        .map_err(|e| {
            tracing::debug!(erreur = %e, "lecture VFS impossible");
            ErreurSite::Introuvable("fichier indexe mais illisible sur ce montage".to_owned())
        })
}

/// Construit une réponse textuelle en posant son type de contenu.
fn text_response(body: String, content_type: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type)], body).into_response()
}

/// Construit une réponse binaire nommée.
///
/// Le nom porte la **sous-entité** (l'index de texture et le mip), jamais le seul nom du
/// fichier source : deux extractions du même conteneur se recouvriraient sur le disque de
/// celui qui les télécharge.
fn bytes_response(body: Vec<u8>, filename: &str) -> Response {
    let mut response = ([(header::CONTENT_TYPE, "application/octet-stream")], body).into_response();
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, v);
    }
    response
}

/// Un tampon d'octets reçu en `POST`, sous l'une des deux formes que du JSON sait porter.
///
/// La forme base64 pèse 4/3 des octets, le tableau d'entiers en pèse 3 à 4 fois plus : c'est
/// elle qu'il faut employer dès qu'une image dépasse quelques milliers de pixels. Les deux
/// sont acceptées parce qu'un client qui construit sa requête à la main n'a pas toujours un
/// encodeur sous la main.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Buffer {
    /// Base64, alphabet standard ou URL-safe, bourrage optionnel.
    Base64(String),
    /// Les octets, un par élément.
    Bytes(Vec<u8>),
}

impl Buffer {
    /// Rend les octets du tampon.
    ///
    /// # Errors
    ///
    /// `Demande` quand la chaîne base64 est invalide.
    pub fn into_bytes(self) -> Result<Vec<u8>, ErreurSite> {
        match self {
            Self::Base64(s) => decode_base64(&s),
            Self::Bytes(b) => Ok(b),
        }
    }
}

/// Valeur d'un caractère base64, ou `None`.
///
/// Les deux alphabets sont acceptés : `+/` (standard) et `-_` (URL-safe). Refuser le second
/// obligerait tout client qui passe le tampon dans une URL à le réencoder.
fn base64_value(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

/// Décode une chaîne base64.
///
/// Les blancs sont ignorés (un tampon collé depuis un fichier arrive replié), le bourrage est
/// optionnel, et un caractère étranger est **refusé en nommant sa position** — un décodeur qui
/// saute ce qu'il ne comprend pas rend un tampon plus court que prévu, que la vérification de
/// taille refuserait ensuite en accusant les dimensions.
///
/// # Errors
///
/// `Demande` sur un caractère invalide, un caractère après le bourrage, ou une longueur
/// impossible (un reste de six bits ne peut pas former un octet).
pub fn decode_base64(s: &str) -> Result<Vec<u8>, ErreurSite> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut padded = false;
    for (i, c) in s.bytes().enumerate() {
        if c.is_ascii_whitespace() {
            continue;
        }
        if c == b'=' {
            padded = true;
            continue;
        }
        if padded {
            return Err(ErreurSite::Demande(format!(
                "base64: caractere apres le bourrage en position {i}"
            )));
        }
        let Some(v) = base64_value(c) else {
            return Err(ErreurSite::Demande(format!(
                "base64: caractere invalide en position {i}"
            )));
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            let byte = u8::try_from((acc >> bits) & 0xFF).unwrap_or(0);
            out.push(byte);
        }
    }
    if bits >= 6 {
        return Err(ErreurSite::Demande(
            "base64: longueur invalide (six bits orphelins)".to_owned(),
        ));
    }
    Ok(out)
}

/// Ne publie un flottant que s'il est fini.
///
/// `imgmetric` rend `f64::NAN` pour un bloc entièrement exclu de la mesure. `serde_json` le
/// traduirait en `null` **par accident** ; le rendre `Option` fait de ce `null` une décision,
/// et un client qui voit `null` sait qu'il n'y a pas de score, pas qu'il vaut zéro.
fn finite(x: f64) -> Option<f64> {
    x.is_finite().then_some(x)
}

/// Vérifie qu'une URL fournie par le client peut être insérée dans du CSS ou du SVG.
///
/// **C'est une garde d'injection, pas une validation d'URL.** `SpriteSheet::vers_svg` écrit son
/// argument tel quel dans `href="…"` ; une URL portant un guillemet en sortirait et pourrait
/// ajouter un attribut à l'élément — dans un document servi en `image/svg+xml`, c'est-à-dire un
/// document que le navigateur exécute. La liste blanche est donc posée sur les caractères, pas
/// sur la forme : un chemin relatif, une URL absolue et une `data:` passent, tout ce qui porte
/// un guillemet, un chevron, une esperluette, une barre oblique inverse, un blanc ou un
/// caractère de contrôle est refusé.
///
/// # Errors
///
/// `Demande` quand l'URL est vide, trop longue, ou porte un caractère hors de la liste.
pub fn check_url(url: &str) -> Result<(), ErreurSite> {
    if url.is_empty() {
        return Err(ErreurSite::Demande("url d'atlas vide".to_owned()));
    }
    if url.len() > 512 {
        return Err(ErreurSite::Demande(
            "url d'atlas trop longue (borne 512)".to_owned(),
        ));
    }
    for c in url.chars() {
        let ok = c.is_ascii_alphanumeric() || "._-/:;,+=?%#()~".contains(c);
        if !ok {
            return Err(ErreurSite::Demande(format!(
                "url d'atlas: caractere refuse {c:?} (liste blanche stricte, cf. inspect::check_url)"
            )));
        }
    }
    Ok(())
}

// ─── Le catalogue des inspecteurs ───────────────────────────────────────────────────────────

/// Les corpus que ce module sait viser, comptés sur l'index.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Corpus {
    /// Fichiers `.g4tx` — la source de `sprite_sheet` et de `nxtch`.
    pub atlases: usize,
    /// Tables `font.cfg.bin` — la source de `font`.
    pub fonts: usize,
    /// Fichiers `.objbin` — la source de `menu`.
    pub menu_objects: usize,
    /// Blocs `.nxtch` autonomes. **Zéro sur ce montage**, cf. la doc de module.
    pub texture_chunks: usize,
}

/// Compte les corpus en descendant l'index une fois.
///
/// L'index n'est pas itérable : on descend dossier par dossier, comme
/// `super::formats::compter`. Un seul parcours pour les quatre comptes — quatre parcours
/// coûteraient quatre fois le prix pour la même information.
#[must_use]
pub fn count(index: &IndexVfs) -> Corpus {
    let mut c = Corpus::default();
    let mut to_visit = vec![String::new()];
    while let Some(prefix) = to_visit.pop() {
        let folder = index.dossier(&prefix, 0, usize::MAX);
        for f in &folder.fichiers {
            let p = f.chemin.as_str();
            if p.ends_with(ATLAS_SUFFIX) {
                c.atlases += 1;
            }
            if p.ends_with(FONT_SUFFIX) {
                c.fonts += 1;
            }
            if p.ends_with(MENU_SUFFIX) {
                c.menu_objects += 1;
            }
            if p.ends_with(CHUNK_SUFFIX) {
                c.texture_chunks += 1;
            }
        }
        to_visit.extend(folder.dossiers);
    }
    c
}

/// Les comptes mémorisés, avec la taille d'index qui les a produits.
static CORPUS: RwLock<Option<(usize, Corpus)>> = RwLock::new(None);

/// Les comptes courants, balayés une seule fois par état d'index.
fn corpus(index: &IndexVfs) -> Corpus {
    let key = index.len();
    if let Ok(guard) = CORPUS.read()
        && let Some((k, v)) = guard.as_ref()
        && *k == key
    {
        return *v;
    }
    let counted = count(index);
    if let Ok(mut guard) = CORPUS.write() {
        *guard = Some((key, counted));
    }
    counted
}

/// Un inspecteur, tel que la route de découverte le publie.
#[derive(Debug, Clone, Serialize)]
pub struct Inspector {
    /// Jeton stable de l'inspecteur.
    pub name: &'static str,
    /// Chemin réellement monté.
    pub route: &'static str,
    /// Méthodes acceptées par ce chemin.
    pub methods: &'static [&'static str],
    /// Module de `nie-formats` que la route appelle — le lien entre l'URL et le code.
    pub module: &'static str,
    /// Ce que la route lit.
    pub reads: &'static str,
    /// Ce qu'elle rend.
    pub produces: &'static str,
    /// Nombre de fichiers du VFS que la route peut viser, `None` tant que l'index n'est pas
    /// prêt. Un inspecteur qui travaille sur un corps `POST` n'en a pas.
    pub corpus: Option<usize>,
    /// Ce que la route ne promet pas, quand il y a quelque chose à ne pas promettre.
    pub caveat: Option<&'static str>,
}

/// Corps de `GET /api/v1/inspect`.
#[derive(Debug, Clone, Serialize)]
pub struct Catalog {
    /// Toujours `nie-site`.
    pub service: &'static str,
    /// Version de la crate.
    pub version: &'static str,
    /// Vrai quand l'index du VFS est prêt : sans lui, aucun corpus n'est mesurable.
    pub vfs_ready: bool,
    /// Features de `nie-formats` réellement compilées — source unique,
    /// [`super::formats::features`].
    pub formats_features: Vec<&'static str>,
    /// Les corpus comptés.
    pub corpus: Option<Corpus>,
    /// Les inspecteurs.
    pub inspectors: Vec<Inspector>,
}

/// Les inspecteurs et ce que chacun lit, avec leur corpus quand il y en a un.
fn inspectors(c: Option<Corpus>) -> Vec<Inspector> {
    vec![
        Inspector {
            name: "spritesheet",
            route: "/api/v1/inspect/spritesheet/{path}",
            methods: &["GET"],
            module: "nie_formats::sprite_sheet",
            reads: "un .g4tx du VFS — ses regions d'atlas, jamais ses pixels",
            produces: "?form=json (manifeste), ?form=css (une classe par region), ?form=svg",
            corpus: c.map(|c| c.atlases),
            caveat: Some(
                "le SVG reference l'atlas par URL: la variante autonome (data:) exigerait \
                 d'encoder les pixels, ce que la feature `textures` eteinte interdit",
            ),
        },
        Inspector {
            name: "font",
            route: "/api/v1/inspect/font/{path}",
            methods: &["GET"],
            module: "nie_formats::font",
            reads: "un font.cfg.bin (T2B) du VFS",
            produces: "?form=summary (dimensions et comptes), ?form=glyphs (table paginee)",
            corpus: c.map(|c| c.fonts),
            caveat: Some("le kerning (entrees KERN/KERNINF) n'est pas interprete par le parseur"),
        },
        Inspector {
            name: "menu",
            route: "/api/v1/inspect/menu/{path}",
            methods: &["GET"],
            module: "nie_formats::menu",
            reads: "un .objbin, son squelette .g4pkm et son atlas .g4tx, resolus sur l'index",
            produces: "transform ecran 1280x720, priorite de dessin, points d'attache",
            corpus: c.map(|c| c.menu_objects),
            caveat: Some(
                "seuls les elements STATIQUES sont places: la pose d'un element anime depend \
                 de keyframes runtime absentes des fichiers",
            ),
        },
        Inspector {
            name: "texture-chunk",
            route: "/api/v1/inspect/texture-chunk/{path}",
            methods: &["GET"],
            module: "nie_formats::nxtch",
            reads: "un bloc NXTCH, ou le payload d'un .g4tx qui en porte un",
            produces: "en-tete, format, tailles attendues, ?form=linear rend le tampon delinearise",
            corpus: c.map(|c| c.texture_chunks),
            caveat: Some(
                "corpus vide sur ce montage: NXTCH est le conteneur Switch, et les payloads \
                 .g4tx de ce build sont du DDS (mesure du 2026-09-06)",
            ),
        },
        Inspector {
            name: "color",
            route: "/api/v1/inspect/color",
            methods: &["GET"],
            module: "nie_formats::imgmetric",
            reads: "deux couleurs sRGB en query (?a=&b=)",
            produces: "ΔE2000, lumiere lineaire de chaque canal, seuil de perceptibilite",
            corpus: None,
            caveat: None,
        },
        Inspector {
            name: "compare",
            route: "/api/v1/inspect/compare",
            methods: &["GET", "POST"],
            module: "nie_formats::imgmetric",
            reads: "deux tampons RGBA8 fournis dans le corps, et des regions optionnelles",
            produces: "T0 identite, T1 imperceptibilite, T2 SSIM, par region, plus la carte par bloc",
            corpus: None,
            caveat: Some(
                "aucune capture du jeu n'est produite par ce service: la reference est celle \
                 que l'appelant fournit",
            ),
        },
        Inspector {
            name: "plate",
            route: "/api/v1/inspect/plate",
            methods: &["GET", "POST"],
            module: "nie_formats::planche",
            reads: "un tampon RGBA8 de planche et, en option, celui de son masque",
            produces: "zones, emprises, role de la planche et convention de composition",
            corpus: None,
            caveat: Some(
                "planche::analyser, qui prendrait un .g4tx entier, est derriere la feature \
                 `textures` et n'est PAS appelee ici",
            ),
        },
    ]
}

/// `GET /api/v1/inspect` — ce que ce module sait inspecter, et sur quel corpus.
pub async fn catalog(State(state): State<EtatSite>) -> Json<Catalog> {
    let index = state.index().ok();
    let counted = match index.as_ref() {
        Some(i) => {
            let i = Arc::clone(i);
            tokio::task::spawn_blocking(move || corpus(&i)).await.ok()
        }
        None => None,
    };
    Json(Catalog {
        service: crate::SERVICE,
        version: crate::VERSION,
        vfs_ready: index.is_some(),
        formats_features: super::formats::features(),
        corpus: counted,
        inspectors: inspectors(counted),
    })
}

// ─── `sprite_sheet` — les régions d'un atlas, en JSON, en CSS et en SVG ─────────────────────

/// Query de `GET /api/v1/inspect/spritesheet/{*path}`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SheetQuery {
    /// `json` (défaut), `css` ou `svg`.
    pub form: Option<String>,
    /// Index de la texture porteuse dans le conteneur, `0` par défaut.
    pub texture: Option<usize>,
    /// `image` (défaut) ou `mask`, pour la feuille CSS.
    pub mode: Option<String>,
    /// URL de l'atlas. Par défaut, celle de l'amont pour cette texture.
    pub image: Option<String>,
}

/// La forme demandée pour une feuille de sprites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetForm {
    /// Le manifeste JSON du module, verbatim.
    Json,
    /// Une feuille CSS.
    Css,
    /// Une feuille SVG.
    Svg,
}

impl SheetForm {
    /// Reconnaît une forme, ou dit lesquelles existent.
    ///
    /// # Errors
    ///
    /// `Demande` sur une forme inconnue.
    pub fn parse(s: Option<&str>) -> Result<Self, ErreurSite> {
        match s.map(str::trim).filter(|f| !f.is_empty()) {
            None | Some("json") => Ok(Self::Json),
            Some("css") => Ok(Self::Css),
            Some("svg") => Ok(Self::Svg),
            Some(other) => Err(ErreurSite::Demande(format!(
                "forme inconnue: {other} (connues: json, css, svg)"
            ))),
        }
    }

    /// Type de contenu de cette forme.
    #[must_use]
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Css => "text/css; charset=utf-8",
            Self::Svg => "image/svg+xml; charset=utf-8",
        }
    }
}

/// Reconnaît le mode de pose CSS.
///
/// # Errors
///
/// `Demande` sur un mode inconnu.
pub fn css_mode(s: Option<&str>) -> Result<sprite_sheet::ModeCss, ErreurSite> {
    match s.map(str::trim).filter(|m| !m.is_empty()) {
        None | Some("image") => Ok(sprite_sheet::ModeCss::Image),
        Some("mask") => Ok(sprite_sheet::ModeCss::Masque),
        Some(other) => Err(ErreurSite::Demande(format!(
            "mode inconnu: {other} (connus: image, mask)"
        ))),
    }
}

/// URL par défaut de l'atlas d'une texture, telle que l'amont l'adresse.
///
/// Deux formes, et ce sont celles de `nie-model-serve` (`src/main.rs`, route `/tex/`), pas des
/// conventions inventées ici :
///
/// - texture principale : `/assets/tex/<chemin sans .g4tx>.png` ;
/// - texture nommée : `/assets/tex/<chemin>/<nom>.png` — la seule façon d'adresser un
///   conteneur multi-textures, où les icônes vivent à quatre-vingts par fichier.
///
/// Le préfixe `data/` est **conservé** : l'amont accepte les deux formes
/// (`rel.starts_with("data/")`), et garder le chemin VFS verbatim évite d'avoir deux
/// écritures du même fichier selon la route qui l'a produit.
#[must_use]
pub fn default_atlas_url(path: &str, texture_name: Option<&str>) -> String {
    match texture_name {
        Some(name) => format!("/assets/tex/{path}/{name}.png"),
        None => {
            let stem = path.strip_suffix(ATLAS_SUFFIX).unwrap_or(path);
            format!("/assets/tex/{stem}.png")
        }
    }
}

/// Rend la feuille de sprites d'un atlas.
///
/// Fonction **pure** : ni HTTP ni VFS, comme celles de `super::geometrie` — c'est elle que les
/// tests falsifient.
///
/// # Errors
///
/// `Demande` quand les octets ne sont pas un `.g4tx`, quand l'index de texture n'existe pas,
/// ou quand la forme, le mode ou l'URL sont refusés.
pub fn render_sheet(
    path: &str,
    bytes: &[u8],
    query: &SheetQuery,
) -> Result<(String, &'static str), ErreurSite> {
    let form = SheetForm::parse(query.form.as_deref())?;
    let atlas =
        g4tx::parse(bytes).map_err(|e| ErreurSite::Demande(format!("G4TX illisible: {e}")))?;
    let index = query.texture.unwrap_or(0);
    let sheet = sprite_sheet::depuis_g4tx(&atlas, index).ok_or_else(|| {
        ErreurSite::Demande(format!(
            "index de texture {index} absent du conteneur ({} texture(s))",
            atlas.textures.len()
        ))
    })?;

    // Le nom sert à construire l'URL d'une texture NOMMÉE : au-delà de la principale, l'amont
    // n'adresse plus par le seul chemin du conteneur.
    let named = (index > 0).then(|| sheet.nom.clone());
    let url = match query
        .image
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        Some(u) => {
            check_url(u)?;
            u.to_owned()
        }
        None => default_atlas_url(path, named.as_deref()),
    };
    // L'URL par défaut vient d'un chemin VFS déjà normalisé, mais elle traverse les mêmes
    // documents : on la contrôle aussi, faute de quoi la garde ne couvrirait qu'une moitié des
    // chemins d'appel.
    check_url(&url)?;

    let body = match form {
        SheetForm::Json => sheet.vers_json(),
        SheetForm::Css => sheet.vers_css_mode(&url, css_mode(query.mode.as_deref())?),
        SheetForm::Svg => sheet.vers_svg(&url),
    };
    Ok((body, form.content_type()))
}

/// `GET /api/v1/inspect/spritesheet/{*path}` — les régions d'un atlas du jeu.
///
/// # Errors
///
/// `Demande` sur chemin sortant, forme, mode ou URL refusés, source trop volumineuse ou octets
/// illisibles ; `Introuvable` sur chemin absent ; `Indisponible` tant que le VFS n'est pas
/// monté.
pub async fn spritesheet(
    State(state): State<EtatSite>,
    Path(raw): Path<String>,
    Query(query): Query<SheetQuery>,
) -> Result<Response, ErreurSite> {
    let path = super::vfs::normaliser(&raw)?;
    // La forme est validée AVANT la lecture : refuser `?form=xml` après avoir lu 16 Mio ferait
    // payer au client une erreur qui tient dans la query string.
    SheetForm::parse(query.form.as_deref())?;
    let bytes = read_source(&state, &path).await?;
    let (body, content_type) =
        tokio::task::spawn_blocking(move || render_sheet(&path, &bytes, &query)).await??;
    Ok(text_response(body, content_type))
}

// ─── `font` — les métriques de glyphes d'une police du jeu ──────────────────────────────────

/// Query de `GET /api/v1/inspect/font/{*path}`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FontQuery {
    /// `summary` (défaut) ou `glyphs`.
    pub form: Option<String>,
    /// Police visée : `0` (principale, défaut) ou `1` (petite).
    pub font: Option<u8>,
    /// Point de code à résoudre, en `?form=summary`.
    pub codepoint: Option<u32>,
}

/// Dimensions d'une police, republiées.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct FontDims {
    /// Pixels au-dessus de la ligne de base.
    pub ascent: u16,
    /// Hauteur totale d'une cellule.
    pub cell_height: u16,
    /// Pixels sous la ligne de base.
    pub descent: u16,
}

impl From<font::FontDimensions> for FontDims {
    fn from(d: font::FontDimensions) -> Self {
        Self {
            ascent: d.ascent,
            cell_height: d.cell_height,
            descent: d.descent,
        }
    }
}

/// La métrique d'un glyphe.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Glyph {
    /// Index de police (0 principale, 1 petite).
    pub font: u8,
    /// Identifiant de groupe (`CHR` col[1]).
    pub base: u32,
    /// Point de code (`CHR` col[2] — la clé unique).
    pub codepoint: u32,
    /// Position X dans l'atlas, en pixels.
    pub x: u16,
    /// Position Y dans l'atlas, en pixels.
    pub y: u16,
    /// Largeur du glyphe dans l'atlas.
    pub width: u16,
    /// Décalage horizontal, en pixels (peut être négatif).
    pub bearing_x: i16,
    /// Avance du curseur après ce glyphe.
    pub advance: u16,
    /// Page d'atlas.
    pub page: u8,
}

impl From<&font::GlyphMetric> for Glyph {
    fn from(g: &font::GlyphMetric) -> Self {
        Self {
            font: g.font,
            base: g.base,
            codepoint: g.codepoint,
            x: g.x,
            y: g.y,
            width: g.width,
            bearing_x: g.bearing_x,
            advance: g.advance,
            page: g.page,
        }
    }
}

/// Corps de `?form=summary`.
#[derive(Debug, Clone, Serialize)]
pub struct FontSummary {
    /// Chemin VFS lu.
    pub path: String,
    /// Taille de la source, en octets.
    pub bytes: usize,
    /// Largeur de l'atlas de police déclarée par `INF`.
    pub atlas_width: u32,
    /// Hauteur de l'atlas de police.
    pub atlas_height: u32,
    /// Nombre de lignes `INF` (une par police déclarée).
    pub inf_rows: usize,
    /// Nombre de glyphes de la police principale.
    pub glyphs: usize,
    /// Nombre de glyphes de la petite police.
    pub glyphs_small: usize,
    /// Dimensions de la police principale.
    pub dims: FontDims,
    /// Dimensions de la petite police.
    pub dims_small: FontDims,
    /// Le glyphe demandé par `?codepoint=`, quand il a été demandé.
    ///
    /// Trois états, et ils sont distincts : absent du JSON (rien n'a été demandé), `null` (le
    /// point de code n'est pas dans cette police), un objet (il y est). Confondre les deux
    /// derniers ferait passer « absent » pour « pas demandé ».
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glyph: Option<Option<Glyph>>,
}

/// Reconnaît l'index de police demandé.
///
/// # Errors
///
/// `Demande` au-delà de 1 : `font.cfg.bin` ne déclare que deux polices (grande et petite), et
/// une troisième rendrait une table vide en annonçant un succès.
pub fn font_index(v: Option<u8>) -> Result<u8, ErreurSite> {
    match v.unwrap_or(0) {
        n @ (0 | 1) => Ok(n),
        other => Err(ErreurSite::Demande(format!(
            "police inconnue: {other} (connues: 0 = principale, 1 = petite)"
        ))),
    }
}

/// Interprète un `font.cfg.bin` et rend la forme demandée.
///
/// Fonction **pure**, testable sans HTTP ni VFS.
///
/// # Errors
///
/// `Demande` quand les octets ne sont pas un `cfg.bin`, quand la forme ou la police sont
/// inconnues, ou quand la sérialisation échoue.
pub fn font_report(
    path: &str,
    bytes: &[u8],
    query: &FontQuery,
    page: &DemandePage,
) -> Result<serde_json::Value, ErreurSite> {
    let index = font_index(query.font)?;
    let cfg = cfgbin::cfgbin_parse(bytes)
        .map_err(|e| ErreurSite::Demande(format!("cfg.bin illisible: {e}")))?;
    let metrics = font::parse_metrics(&cfg);

    match query
        .form
        .as_deref()
        .map(str::trim)
        .filter(|f| !f.is_empty())
    {
        None | Some("summary") => {
            let glyph = query
                .codepoint
                .map(|cp| metrics.glyph_in_font(index, cp).map(Glyph::from));
            let summary = FontSummary {
                path: path.to_owned(),
                bytes: bytes.len(),
                atlas_width: metrics.atlas_width,
                atlas_height: metrics.atlas_height,
                inf_rows: metrics.inf_raw.len(),
                glyphs: metrics.glyphs.len(),
                glyphs_small: metrics.glyphs_small.len(),
                dims: metrics.dims.into(),
                dims_small: metrics.dims_small.into(),
                glyph,
            };
            serde_json::to_value(summary)
                .map_err(|e| ErreurSite::Interne(format!("reponse non serialisable: {e}")))
        }
        Some("glyphs") => {
            // La table est PAGINÉE, jamais rendue entière : `font_zh_hans` pèse 939 328 octets
            // de source et sa table de glyphes dépasserait de loin la borne au-delà de laquelle
            // l'ETag cesse de condenser. Une collection se sert par pages, comme partout
            // ailleurs dans cette API.
            let table = if index == 0 {
                &metrics.glyphs
            } else {
                &metrics.glyphs_small
            };
            let p = page.bornee();
            let total = table.len();
            let elements: Vec<Glyph> = table
                .values()
                .skip(p.offset())
                .take(p.per_page as usize)
                .map(Glyph::from)
                .collect();
            serde_json::to_value(Page::nouvelle(elements, p, total))
                .map_err(|e| ErreurSite::Interne(format!("reponse non serialisable: {e}")))
        }
        Some(other) => Err(ErreurSite::Demande(format!(
            "forme inconnue: {other} (connues: summary, glyphs)"
        ))),
    }
}

/// `GET /api/v1/inspect/font/{*path}` — les métriques de glyphes d'une police du jeu.
///
/// # Errors
///
/// `Demande` sur chemin sortant, forme ou police inconnues, source trop volumineuse ou octets
/// illisibles ; `Introuvable` sur chemin absent ; `Indisponible` tant que le VFS n'est pas
/// monté.
pub async fn font_metrics(
    State(state): State<EtatSite>,
    Path(raw): Path<String>,
    Query(query): Query<FontQuery>,
    Query(page): Query<DemandePage>,
) -> Result<Json<serde_json::Value>, ErreurSite> {
    let path = super::vfs::normaliser(&raw)?;
    font_index(query.font)?;
    let bytes = read_source(&state, &path).await?;
    let body =
        tokio::task::spawn_blocking(move || font_report(&path, &bytes, &query, &page)).await??;
    Ok(Json(body))
}

// ─── `menu` — la géométrie écran d'un objet de menu ─────────────────────────────────────────

/// Le jeton public d'un composant d'objet de menu.
///
/// Écrit ici plutôt que partagé avec `super::geometrie` : son équivalent y est **privé**, et
/// son dernier jeton (`inconnu`) date d'avant la règle du 2026-09-06. Un jeton public est un
/// contrat, il se choisit une fois.
#[must_use]
pub fn component_token(c: &objbin::MenuComponent) -> &'static str {
    use objbin::MenuComponent as C;
    match c {
        C::Render(_) => "render",
        C::Animation(_) => "animation",
        C::Text(_) => "text",
        C::Primitive(_) => "primitive",
        C::AttachLocator(_) => "attach_locator",
        C::Collision(_) => "collision",
        C::SoundCmd(_) => "sound_cmd",
        C::MeshVisible(_) => "mesh_visible",
        C::Unknown(_) => "unknown",
    }
}

/// Résout un chemin logique de compagnon (`common/menu/…/x.g4pkm`) en chemin VFS.
///
/// Trois tentatives, dans l'ordre du moins cher au plus cher, et **aucune ne devine** :
///
/// 1. `data/` suivi du chemin logique, tel quel — c'est la forme la plus fréquente, vérifiée
///    sur `data/common/menu/100_mainmenu/mainmenu01/mainmenu01_06/mainmenu01_06.g4pkm` ;
/// 2. la même, `<LG>` remplacé par la locale demandée, pour les assets localisés ;
/// 3. la recherche par **nom de fichier**, avec l'ordre de priorité de
///    `nie-game::resolve_vfs_basename` : locale demandée, puis dossier non localisé, puis
///    `common`, puis `en`, puis le premier par ordre lexicographique. L'index étant trié, le
///    résultat est reproductible — un `find` direct choisirait une locale au hasard.
///
/// Rend `None` quand rien ne correspond. Mesuré le 2026-09-06 : cela **arrive**.
/// `mainmenu01_04_menu_list.objbin` référence `mainmenu01_04.g4pkm`, qui n'existe nulle part
/// dans le VFS de ce build (les dossiers présents sont `mainmenu01_06`, `_07`, `_07b`…). Le
/// dire vaut mieux que rendre une position par défaut.
///
/// Les deux dernières étapes ne sont pas décoratives, elles sont mesurées :
/// `btl01_02_action_stone_base.objbin` déclare sa texture sous
/// `dx11/menu/02_btl/btl01/btl01_02/<LG>/btl01_02.g4tx`, chemin qui n'existe **sous aucune
/// forme littérale** — les neuf fichiers réels sont `.../btl01_02.g4tx` et huit variantes de
/// locale. Sans la recherche par nom, cet objet n'aurait jamais de sprite, donc jamais
/// d'échelle, et le canvas paraîtrait vide sans qu'aucune valeur ne soit fausse.
///
/// **Aucun chemin ne sort de l'index.** Les trois issues sont soit `index.contient(…)`, soit un
/// chemin lu dans l'index : un chemin logique portant `..` ne peut pas produire une lecture
/// hors du VFS, il ne correspond simplement à rien.
#[must_use]
pub fn resolve_companion(index: &IndexVfs, logical: &str, locale: &str) -> Option<String> {
    let logical = logical.trim_start_matches('/');
    if logical.is_empty() {
        return None;
    }
    let direct = format!("data/{logical}");
    if index.contient(&direct) {
        return Some(direct);
    }
    if direct.contains(LOCALE_PLACEHOLDER) {
        let replaced = direct.replace(LOCALE_PLACEHOLDER, locale);
        if index.contient(&replaced) {
            return Some(replaced);
        }
    }

    let basename = logical.rsplit('/').next().filter(|s| !s.is_empty())?;
    if basename.contains(LOCALE_PLACEHOLDER) {
        return None;
    }
    let requete = index
        .resoudre(Some(basename), &DemandeFiltre::default())
        .paginer(0, MAX_COMPANION_CANDIDATES);
    let (files, _) = index.page_filtree(None, &requete);
    let candidates: Vec<&str> = files
        .iter()
        .filter(|f| f.nom.eq_ignore_ascii_case(basename))
        .map(|f| f.chemin.as_str())
        .collect();
    if candidates.is_empty() {
        return None;
    }

    let parent_of = |p: &str| -> String {
        p.strip_suffix(basename)
            .map(|d| d.trim_end_matches('/'))
            .and_then(|d| d.rsplit('/').next())
            .unwrap_or("")
            .to_owned()
    };
    if let Some(p) = candidates.iter().find(|p| parent_of(p) == locale) {
        return Some((*p).to_owned());
    }
    if let Some(p) = candidates.iter().find(|p| !is_locale_tag(&parent_of(p))) {
        return Some((*p).to_owned());
    }
    for fallback in ["common", "en"] {
        if let Some(p) = candidates.iter().find(|p| parent_of(p) == fallback) {
            return Some((*p).to_owned());
        }
    }
    candidates.first().map(|p| (*p).to_owned())
}

/// Un compagnon d'objet de menu, tel que la réponse le rapporte.
#[derive(Debug, Clone, Serialize)]
pub struct Companion {
    /// Chemin logique déclaré par l'`.objbin`, tel quel.
    pub logical: Option<String>,
    /// Chemin VFS résolu, `null` quand rien ne correspond.
    pub resolved: Option<String>,
    /// Pourquoi il manque, quand il manque.
    pub missing_reason: Option<&'static str>,
}

/// Ce que l'appelant a résolu et lu pour un objet de menu.
///
/// La résolution vit dans le handler, là où l'index et le VFS existent ; le calcul reste une
/// fonction pure. C'est le même partage que `super::formats::resoudre_compagnon` pour `.g4mg`.
#[derive(Debug, Default)]
pub struct MenuSources {
    /// Le squelette `.g4pkm` : son chemin VFS et ses octets.
    pub skeleton: Option<(String, Vec<u8>)>,
    /// L'atlas `.g4tx` : son chemin VFS et ses octets.
    pub texture: Option<(String, Vec<u8>)>,
}

/// Le canvas de référence des menus du jeu.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Canvas {
    /// Largeur, en pixels CSS.
    pub width: u32,
    /// Hauteur, en pixels CSS.
    pub height: u32,
}

/// Un transform écran.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Placement {
    /// Position X du pivot, en pixels du canvas.
    pub x: f32,
    /// Position Y du pivot.
    pub y: f32,
    /// Facteur d'échelle horizontal appliqué au sprite.
    pub scale_x: f32,
    /// Facteur d'échelle vertical.
    pub scale_y: f32,
    /// Rotation, en radians.
    pub rotation: f32,
}

impl From<menu_layout::ScreenTransform> for Placement {
    fn from(t: menu_layout::ScreenTransform) -> Self {
        Self {
            x: t.x_px,
            y: t.y_px,
            scale_x: t.scale_x,
            scale_y: t.scale_y,
            rotation: t.rot,
        }
    }
}

/// Les dimensions natives du sprite porté par l'objet.
#[derive(Debug, Clone, Serialize)]
pub struct SpriteSize {
    /// Nom de la texture principale du conteneur.
    pub name: String,
    /// Largeur native, en pixels.
    pub width: u32,
    /// Hauteur native.
    pub height: u32,
    /// Nombre de régions déclarées par cette texture.
    pub regions: usize,
}

/// Un emplacement déclaré par un `CMenuAttachLocator`.
#[derive(Debug, Clone, Serialize)]
pub struct AttachSlot {
    /// Nom de l'os d'attache, résolu dans le squelette du locator.
    pub bone: String,
    /// CRC-32 du nom de l'objet à placer ici.
    pub target_hash: u32,
    /// Rang de l'emplacement dans sa série.
    pub index: u32,
    /// Position X sur le canvas.
    pub x: f32,
    /// Position Y sur le canvas.
    pub y: f32,
}

/// Corps de `GET /api/v1/inspect/menu/{*path}`.
#[derive(Debug, Clone, Serialize)]
pub struct MenuGeometry {
    /// Chemin VFS de l'objet.
    pub path: String,
    /// Taille de la source, en octets.
    pub bytes: usize,
    /// Nom de l'objet (`OBJ_BGN`).
    pub name: String,
    /// Type moteur déclaré.
    pub engine_type: String,
    /// Priorité de dessin (z-order croissant = au-dessus).
    pub draw_priority: i32,
    /// Les composants attachés, en jetons.
    pub components: Vec<&'static str>,
    /// Le canvas de référence.
    pub canvas: Canvas,
    /// Le squelette.
    pub skeleton: Companion,
    /// L'atlas.
    pub texture: Companion,
    /// Les dimensions natives du sprite, `null` quand l'atlas n'a pas pu être lu.
    pub sprite: Option<SpriteSize>,
    /// Le transform final, sprite pris en compte. `null` sans squelette.
    pub placement: Option<Placement>,
    /// Le transform **avant** l'appariement de l'os à la taille du sprite.
    ///
    /// Publié à côté du précédent parce que c'est la seule façon de voir ce que
    /// `pick_best_pose` a changé : identiques, la pose de placement portait déjà sa géométrie ;
    /// différents, c'est un os feuille qui a été retenu pour ce sprite.
    pub placement_unmatched: Option<Placement>,
    /// La taille que l'os de placement **désigne**, en pixels de l'espace de référence.
    pub designated_size: Option<[f32; 2]>,
    /// Les emplacements d'attache déclarés par cet objet.
    pub attach_slots: Vec<AttachSlot>,
}

/// Assemble la géométrie d'un objet de menu.
///
/// Fonction **pure** : les compagnons arrivent déjà lus.
///
/// # Errors
///
/// `Demande` quand les octets ne sont pas un `.objbin` lisible.
pub fn menu_geometry(
    path: &str,
    bytes: &[u8],
    sources: MenuSources,
) -> Result<MenuGeometry, ErreurSite> {
    let obj =
        objbin::parse(bytes).map_err(|e| ErreurSite::Demande(format!("OBJBIN illisible: {e}")))?;

    let draw_priority = obj
        .components
        .iter()
        .find_map(|c| match c {
            objbin::MenuComponent::Render(r) => Some(r.draw_priority),
            _ => None,
        })
        .unwrap_or(0);

    // L'atlas : ses dimensions natives conditionnent l'appariement d'os. Illisible ou absent,
    // on le DIT et on place avec un sprite de 0×0 — ce qui rend la pose de placement brute,
    // sans appariement. Inventer une taille produirait une échelle fausse d'apparence saine.
    let mut sprite = None;
    if let Some((_, ref data)) = sources.texture
        && let Ok(atlas) = g4tx::parse(data)
        && let Some(t) = atlas.textures.first()
    {
        sprite = Some(SpriteSize {
            name: t.name.clone(),
            width: u32::try_from(t.width).unwrap_or(0),
            height: u32::try_from(t.height).unwrap_or(0),
            regions: t.sub_textures.len(),
        });
    }
    let (sprite_w, sprite_h) = sprite.as_ref().map_or((0, 0), |s| (s.width, s.height));

    let layout = sources
        .skeleton
        .as_ref()
        .and_then(|(_, data)| g4pkm::parse(data).ok());

    let (placement, placement_unmatched, designated_size, attach_slots) = match layout.as_ref() {
        Some(l) => {
            let positioned = menu_layout::assemble_object(&obj, l, sprite_w, sprite_h);
            let raw = menu_layout::place_on_canvas(l, 0, 0);
            let designated =
                menu_layout::taille_designee(l, sprite_w, sprite_h).map(|(w, h)| [w, h]);
            let slots = menu_layout::attach_slots(&obj, l)
                .into_iter()
                .map(|s| {
                    let (x, y) = s.to_css();
                    AttachSlot {
                        bone: s.bone,
                        target_hash: s.target_hash,
                        index: s.index,
                        x,
                        y,
                    }
                })
                .collect();
            (
                Some(Placement::from(positioned.transform)),
                Some(Placement::from(raw)),
                designated,
                slots,
            )
        }
        None => (None, None, None, Vec::new()),
    };

    let skeleton_missing = match (&obj.g4pkm_path, &sources.skeleton, &layout) {
        (None, ..) => Some("l'objet ne declare aucun SkeletonAnime"),
        (Some(_), None, _) => Some("chemin declare mais absent de ce montage du VFS"),
        (Some(_), Some(_), None) => Some("squelette lu mais illisible par g4pkm::parse"),
        _ => None,
    };
    let texture_missing = match (&obj.g4tx_path, &sources.texture, &sprite) {
        (None, ..) => Some("l'objet ne declare aucune texture"),
        (Some(_), None, _) => Some("chemin declare mais absent de ce montage du VFS"),
        (Some(_), Some(_), None) => Some("atlas lu mais illisible par g4tx::parse"),
        _ => None,
    };

    Ok(MenuGeometry {
        path: path.to_owned(),
        bytes: bytes.len(),
        name: obj.name.clone(),
        engine_type: obj.engine_type.clone(),
        draw_priority,
        components: obj.components.iter().map(component_token).collect(),
        canvas: Canvas {
            width: 1280,
            height: 720,
        },
        skeleton: Companion {
            logical: obj.g4pkm_path.clone(),
            resolved: sources.skeleton.as_ref().map(|(p, _)| p.clone()),
            missing_reason: skeleton_missing,
        },
        texture: Companion {
            logical: obj.g4tx_path.clone(),
            resolved: sources.texture.as_ref().map(|(p, _)| p.clone()),
            missing_reason: texture_missing,
        },
        sprite,
        placement,
        placement_unmatched,
        designated_size,
        attach_slots,
    })
}

/// Query de `GET /api/v1/inspect/menu/{*path}`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MenuQuery {
    /// Locale employée pour résoudre un compagnon localisé. `fr` par défaut.
    pub locale: Option<String>,
}

/// `GET /api/v1/inspect/menu/{*path}` — la géométrie écran d'un objet de menu.
///
/// # Errors
///
/// `Demande` sur chemin sortant, source trop volumineuse ou octets illisibles ; `Introuvable`
/// sur chemin absent ; `Indisponible` tant que le VFS n'est pas monté.
pub async fn menu(
    State(state): State<EtatSite>,
    Path(raw): Path<String>,
    Query(query): Query<MenuQuery>,
) -> Result<Json<MenuGeometry>, ErreurSite> {
    let path = super::vfs::normaliser(&raw)?;
    let locale = query
        .locale
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .unwrap_or(DEFAULT_LOCALE)
        .to_owned();
    let bytes = read_source(&state, &path).await?;

    // Le parseur tourne deux fois : une première ici, pour connaître les compagnons à lire, et
    // une seconde dans la fonction pure. Un `.objbin` plafonne à 15 024 octets (mesuré sur les
    // 12 190 du VFS) : le second parcours coûte moins que le partage d'état qu'il éviterait.
    let obj =
        objbin::parse(&bytes).map_err(|e| ErreurSite::Demande(format!("OBJBIN illisible: {e}")))?;
    let index = state.index()?;
    let logical_skeleton = obj.g4pkm_path.clone();
    let logical_texture = obj.g4tx_path.clone();
    let index_for_lookup = Arc::clone(&index);
    let locale_for_lookup = locale.clone();
    let (skeleton_path, texture_path) = tokio::task::spawn_blocking(move || {
        (
            logical_skeleton
                .as_deref()
                .and_then(|l| resolve_companion(&index_for_lookup, l, &locale_for_lookup)),
            logical_texture
                .as_deref()
                .and_then(|l| resolve_companion(&index_for_lookup, l, &locale_for_lookup)),
        )
    })
    .await?;

    let mut sources = MenuSources::default();
    if let Some(p) = skeleton_path
        && let Ok(data) = read_source(&state, &p).await
    {
        sources.skeleton = Some((p, data));
    }
    if let Some(p) = texture_path
        && let Ok(data) = read_source(&state, &p).await
    {
        sources.texture = Some((p, data));
    }

    let geometry =
        tokio::task::spawn_blocking(move || menu_geometry(&path, &bytes, sources)).await??;
    Ok(Json(geometry))
}

// ─── `nxtch` — l'en-tête d'un bloc de texture Switch ────────────────────────────────────────

/// Le jeton public d'un format NXTCH.
#[must_use]
pub fn nxtch_format_token(f: nxtch::NxtchFormat) -> &'static str {
    use nxtch::NxtchFormat as F;
    match f {
        F::Unknown => "unknown",
        F::Bc1 => "bc1",
        F::Bc2 => "bc2",
        F::Bc3 => "bc3",
        F::Bc4 => "bc4",
        F::Bc5 => "bc5",
        F::Bc6 => "bc6",
        F::Bc7 => "bc7",
        F::Rgba8 => "rgba8",
    }
}

/// Query de `GET /api/v1/inspect/texture-chunk/{*path}`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChunkQuery {
    /// `header` (défaut) ou `linear`.
    pub form: Option<String>,
    /// Index de la texture du conteneur `.g4tx` à examiner. `0` par défaut.
    pub texture: Option<usize>,
    /// Hauteur de bloc en GOBs, exprimée en log2 (`0..=5`), pour la délinéarisation.
    pub block_height_log2: Option<u32>,
}

/// L'en-tête NXTCH, republié.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ChunkHeader {
    /// Taille des données texture déclarée.
    pub texture_data_size: i32,
    /// Seconde taille déclarée.
    pub texture_data_size2: i32,
    /// Largeur en pixels.
    pub width: i32,
    /// Hauteur en pixels.
    pub height: i32,
    /// Code de format brut.
    pub format_code: i32,
    /// Jeton du format.
    pub format: &'static str,
    /// Nombre de mips.
    pub mipmap_count: i32,
}

/// Corps de `?form=header`.
#[derive(Debug, Clone, Serialize)]
pub struct ChunkReport {
    /// Chemin VFS lu.
    pub path: String,
    /// Taille de la source, en octets.
    pub bytes: usize,
    /// `raw` quand le fichier EST un bloc NXTCH, `g4tx` quand il en porte un.
    pub source: &'static str,
    /// Index de la texture examinée dans le conteneur.
    pub texture_index: usize,
    /// Nom de cette texture, quand le conteneur le donne.
    pub texture_name: Option<String>,
    /// Décalage du bloc dans le fichier.
    pub offset: usize,
    /// L'en-tête.
    pub header: ChunkHeader,
    /// Vrai pour les formats compressés par blocs 4×4.
    pub block_compressed: bool,
    /// Taille d'un bloc 4×4, en octets. `0` hors BCn.
    pub block_byte_size: usize,
    /// Taille linéaire attendue, tous mips confondus.
    pub expected_data_bytes: usize,
    /// Octets réellement disponibles après l'en-tête.
    pub available_bytes: usize,
    /// Vrai quand le fichier porte au moins ce que l'en-tête annonce.
    ///
    /// Publié plutôt que vérifié en silence : un bloc tronqué n'est pas une erreur du service,
    /// c'est un fait sur le fichier.
    pub complete: bool,
}

/// Localise le bloc NXTCH d'un fichier, dans le fichier lui-même ou dans un `.g4tx`.
///
/// # Errors
///
/// `Demande` quand ni le fichier ni la texture demandée ne portent le magic NXTCH — avec, dans
/// le message, ce qui a réellement été vu.
fn locate_chunk(
    bytes: &[u8],
    texture: usize,
) -> Result<(&'static str, usize, usize, Option<String>), ErreurSite> {
    if nxtch::is_nxtch(bytes) {
        return Ok(("raw", 0, 0, None));
    }
    if let Ok(atlas) = g4tx::parse(bytes) {
        let t = atlas.textures.get(texture).ok_or_else(|| {
            ErreurSite::Demande(format!(
                "index de texture {texture} absent du conteneur ({} texture(s))",
                atlas.textures.len()
            ))
        })?;
        let payload = bytes.get(t.data_offset..).unwrap_or_default();
        if nxtch::is_nxtch(payload) {
            return Ok(("g4tx", texture, t.data_offset, Some(t.name.clone())));
        }
        return Err(ErreurSite::Demande(format!(
            "la texture {texture} ({}) de ce conteneur ne porte pas de bloc NXTCH \
             (payload DDS: {}). Aucun fichier du VFS de ce build n'en porte — cf. \
             /api/v1/inspect, champ corpus",
            t.name, t.is_dds
        )));
    }
    let head: String = bytes
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    Err(ErreurSite::Demande(format!(
        "ni bloc NXTCH ni conteneur G4TX (premiers octets: {head})"
    )))
}

/// Décrit le bloc NXTCH d'un fichier.
///
/// Fonction **pure**, testable sans HTTP ni VFS.
///
/// # Errors
///
/// `Demande` quand le fichier ne porte pas de bloc NXTCH, ou que l'en-tête est illisible.
pub fn chunk_report(
    path: &str,
    bytes: &[u8],
    query: &ChunkQuery,
) -> Result<ChunkReport, ErreurSite> {
    let texture = query.texture.unwrap_or(0);
    let (source, texture_index, offset, texture_name) = locate_chunk(bytes, texture)?;
    let payload = bytes.get(offset..).unwrap_or_default();
    let header = nxtch::parse_header(payload)
        .map_err(|e| ErreurSite::Demande(format!("en-tete NXTCH illisible: {e}")))?;
    let format = header.texture_format();
    let expected = nxtch::calculate_texture_data_size(
        format,
        header.width,
        header.height,
        header.mipmap_count,
    );
    let available = payload.len().saturating_sub(nxtch::NXTCH_HEADER_SIZE);
    Ok(ChunkReport {
        path: path.to_owned(),
        bytes: bytes.len(),
        source,
        texture_index,
        texture_name,
        offset,
        header: ChunkHeader {
            texture_data_size: header.texture_data_size,
            texture_data_size2: header.texture_data_size2,
            width: header.width,
            height: header.height,
            format_code: header.format,
            format: nxtch_format_token(format),
            mipmap_count: header.mipmap_count,
        },
        block_compressed: format.is_block_compressed(),
        block_byte_size: format.block_byte_size(),
        expected_data_bytes: expected,
        available_bytes: available,
        complete: available >= expected,
    })
}

/// Délinéarise le premier mip d'un bloc NXTCH.
///
/// C'est ce que `?form=linear` sert : le tampon Block-Linear (tuiles GOB Tegra X1) ramené en
/// ordre linéaire, prêt à entrer dans un décodeur BCn — que ce service n'a pas, et c'est
/// précisément pourquoi il rend les octets plutôt qu'une image.
///
/// # Errors
///
/// `Demande` quand le fichier ne porte pas de bloc, ou quand `block_height_log2` sort de
/// `0..=5` (au-delà, la formule GOB ne décrit plus rien).
pub fn chunk_linear(bytes: &[u8], query: &ChunkQuery) -> Result<Vec<u8>, ErreurSite> {
    let block_height_log2 = query.block_height_log2.unwrap_or(4);
    if block_height_log2 > 5 {
        return Err(ErreurSite::Demande(format!(
            "block_height_log2 hors bornes: {block_height_log2} (0..=5)"
        )));
    }
    let texture = query.texture.unwrap_or(0);
    let (_, _, offset, _) = locate_chunk(bytes, texture)?;
    let payload = bytes.get(offset..).unwrap_or_default();
    let header = nxtch::parse_header(payload)
        .map_err(|e| ErreurSite::Demande(format!("en-tete NXTCH illisible: {e}")))?;
    let format = header.texture_format();
    let block_size = if format.is_block_compressed() {
        format.block_byte_size()
    } else if format == nxtch::NxtchFormat::Rgba8 {
        4
    } else {
        return Err(ErreurSite::Demande(format!(
            "format {} non delinearisable (code {})",
            nxtch_format_token(format),
            header.format
        )));
    };
    let data = payload.get(nxtch::NXTCH_HEADER_SIZE..).unwrap_or_default();
    Ok(nxtch::unswizzle(
        data,
        header.width,
        header.height,
        block_size,
        block_height_log2,
    ))
}

/// `GET /api/v1/inspect/texture-chunk/{*path}` — l'en-tête d'un bloc de texture Switch.
///
/// # Errors
///
/// `Demande` sur chemin sortant, forme inconnue, source trop volumineuse, ou fichier qui ne
/// porte pas de bloc NXTCH ; `Introuvable` sur chemin absent ; `Indisponible` tant que le VFS
/// n'est pas monté.
pub async fn texture_chunk(
    State(state): State<EtatSite>,
    Path(raw): Path<String>,
    Query(query): Query<ChunkQuery>,
) -> Result<Response, ErreurSite> {
    let path = super::vfs::normaliser(&raw)?;
    let linear = match query
        .form
        .as_deref()
        .map(str::trim)
        .filter(|f| !f.is_empty())
    {
        None | Some("header") => false,
        Some("linear") => true,
        Some(other) => {
            return Err(ErreurSite::Demande(format!(
                "forme inconnue: {other} (connues: header, linear)"
            )));
        }
    };
    let bytes = read_source(&state, &path).await?;
    if linear {
        let name = path.rsplit('/').next().unwrap_or(&path);
        let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
        let index = query.texture.unwrap_or(0);
        let filename = format!("{stem}_tex{index}_mip0.bin");
        let data = tokio::task::spawn_blocking(move || chunk_linear(&bytes, &query)).await??;
        return Ok(bytes_response(data, &filename));
    }
    let report = tokio::task::spawn_blocking(move || chunk_report(&path, &bytes, &query)).await??;
    Ok(Json(report).into_response())
}

// ─── `imgmetric` — la couleur, puis la comparaison ──────────────────────────────────────────

/// Query de `GET /api/v1/inspect/color`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ColorQuery {
    /// Première couleur, en hexadécimal (`RRGGBB` ou `#RRGGBB`).
    pub a: Option<String>,
    /// Seconde couleur.
    pub b: Option<String>,
}

/// Une couleur sRGB et sa lumière linéaire.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Color {
    /// Canal rouge.
    pub r: u8,
    /// Canal vert.
    pub g: u8,
    /// Canal bleu.
    pub b: u8,
    /// Les trois canaux en lumière linéaire `0..=1`, courbe sRGB officielle.
    pub linear: [f64; 3],
}

impl Color {
    /// Construit la couleur et calcule sa lumière linéaire.
    #[must_use]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
            linear: [
                imgmetric::srgb_vers_lineaire(r),
                imgmetric::srgb_vers_lineaire(g),
                imgmetric::srgb_vers_lineaire(b),
            ],
        }
    }
}

/// Corps de `GET /api/v1/inspect/color`.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ColorDistance {
    /// Première couleur.
    pub a: Color,
    /// Seconde couleur.
    pub b: Color,
    /// ΔE CIEDE2000 entre les deux.
    pub delta_e2000: Option<f64>,
    /// Vrai au-dessus de 1,0 — le seuil sous lequel l'œil ne sépare plus deux couleurs.
    pub perceptible: bool,
}

/// Lit une couleur hexadécimale.
///
/// # Errors
///
/// `Demande` quand la chaîne n'est pas six chiffres hexadécimaux, `#` optionnel.
pub fn parse_hex_color(s: &str) -> Result<(u8, u8, u8), ErreurSite> {
    let t = s.trim().trim_start_matches('#');
    if t.len() != 6 || !t.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(ErreurSite::Demande(format!(
            "couleur invalide: {s} (attendu RRGGBB en hexadecimal)"
        )));
    }
    let byte = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).unwrap_or(0);
    Ok((byte(0), byte(2), byte(4)))
}

/// `GET /api/v1/inspect/color` — la distance perceptuelle entre deux couleurs.
///
/// # Errors
///
/// `Demande` quand une couleur manque ou n'est pas lisible.
pub async fn color(Query(query): Query<ColorQuery>) -> Result<Json<ColorDistance>, ErreurSite> {
    let a = query
        .a
        .as_deref()
        .ok_or_else(|| ErreurSite::Demande("parametre `a` manquant".to_owned()))?;
    let b = query
        .b
        .as_deref()
        .ok_or_else(|| ErreurSite::Demande("parametre `b` manquant".to_owned()))?;
    let a = parse_hex_color(a)?;
    let b = parse_hex_color(b)?;
    let delta = imgmetric::delta_e2000(a, b);
    Ok(Json(ColorDistance {
        a: Color::new(a.0, a.1, a.2),
        b: Color::new(b.0, b.1, b.2),
        delta_e2000: finite(delta),
        perceptible: delta > 1.0,
    }))
}

/// Une région d'intérêt, telle que la demande la porte.
#[derive(Debug, Clone, Deserialize)]
pub struct RegionRequest {
    /// Nom lisible, repris tel quel dans le rapport.
    pub name: String,
    /// `[x, y, width, height]`, en pixels.
    pub rect: [u32; 4],
    /// `named` (défaut) ou `dynamic` — une région dynamique est **exclue** de tous les scores.
    pub kind: Option<String>,
}

/// Corps de `POST /api/v1/inspect/compare`.
#[derive(Debug, Clone, Deserialize)]
pub struct CompareRequest {
    /// Largeur commune aux deux tampons.
    pub width: u32,
    /// Hauteur commune.
    pub height: u32,
    /// Le tampon rendu, RGBA8.
    pub rendered: Buffer,
    /// Le tampon de référence, RGBA8.
    pub reference: Buffer,
    /// Régions d'intérêt, au plus [`MAX_REGIONS`].
    #[serde(default)]
    pub regions: Vec<RegionRequest>,
    /// Nombre de réductions 2× appliquées **aux deux** tampons avant comparaison.
    #[serde(default)]
    pub downscale: u32,
}

/// Les scores d'une zone.
#[derive(Debug, Clone, Serialize)]
pub struct RegionScore {
    /// Nom de la zone (`global` pour l'image entière).
    pub name: String,
    /// Pixels effectivement comparés.
    pub pixels: u64,
    /// T0 — part de pixels dont les trois canaux sont égaux.
    pub exact_pct: Option<f64>,
    /// T1 — part de pixels à ΔE2000 ≤ 1.
    pub delta_e1_pct: Option<f64>,
    /// Part de pixels dont chaque canal s'écarte d'au plus 2 niveaux.
    pub channel2_pct: Option<f64>,
    /// ΔE2000 moyen.
    pub delta_e_mean: Option<f64>,
    /// ΔE2000 au 99ᵉ centile.
    pub delta_e_p99: Option<f64>,
    /// ΔE2000 maximal.
    pub delta_e_max: Option<f64>,
    /// T2 — SSIM, le pire des trois canaux, en lumière linéaire.
    pub ssim: Option<f64>,
}

impl From<&imgmetric::ScoreRegion> for RegionScore {
    fn from(s: &imgmetric::ScoreRegion) -> Self {
        Self {
            name: s.nom.clone(),
            pixels: s.px,
            exact_pct: finite(s.exact_pct),
            delta_e1_pct: finite(s.de1_pct),
            channel2_pct: finite(s.canal2_pct),
            delta_e_mean: finite(s.de_moyen),
            delta_e_p99: finite(s.de_p99),
            delta_e_max: finite(s.de_max),
            ssim: finite(s.ssim),
        }
    }
}

/// La carte SSIM par bloc.
#[derive(Debug, Clone, Serialize)]
pub struct BlockMap {
    /// Nombre de blocs par ligne.
    pub width: u32,
    /// Nombre de lignes de blocs.
    pub height: u32,
    /// Les valeurs, ligne par ligne. `null` pour un bloc entièrement exclu.
    pub values: Vec<Option<f64>>,
}

/// Corps de la réponse de `POST /api/v1/inspect/compare`.
#[derive(Debug, Clone, Serialize)]
pub struct CompareReport {
    /// Largeur réellement comparée (après réduction).
    pub width: u32,
    /// Hauteur réellement comparée.
    pub height: u32,
    /// Nombre de réductions 2× appliquées.
    pub downscale: u32,
    /// Scores sur toute l'image, régions dynamiques exclues.
    pub global: RegionScore,
    /// Scores des régions nommées.
    pub regions: Vec<RegionScore>,
    /// Part de la surface retirée de la mesure par les régions dynamiques.
    pub excluded_area_pct: Option<f64>,
    /// Part des pixels mesurés dont le rendu est opaque.
    pub opaque_coverage_pct: Option<f64>,
    /// La carte SSIM par bloc.
    pub block_ssim: BlockMap,
}

/// Traduit les régions demandées en régions d'`imgmetric`.
///
/// # Errors
///
/// `Demande` quand il y en a trop, qu'un nom est trop long, ou qu'un genre est inconnu.
fn build_rois(regions: &[RegionRequest], divisor: u32) -> Result<Vec<imgmetric::Roi>, ErreurSite> {
    if regions.len() > MAX_REGIONS {
        return Err(ErreurSite::Demande(format!(
            "{} regions demandees, borne {MAX_REGIONS}",
            regions.len()
        )));
    }
    let mut out = Vec::with_capacity(regions.len());
    for r in regions {
        if r.name.chars().count() > MAX_REGION_NAME {
            return Err(ErreurSite::Demande(format!(
                "nom de region trop long (borne {MAX_REGION_NAME})"
            )));
        }
        let kind = match r.kind.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
            None | Some("named") => imgmetric::RoiKind::Nommee,
            Some("dynamic") => imgmetric::RoiKind::Dynamique,
            Some(other) => {
                return Err(ErreurSite::Demande(format!(
                    "genre de region inconnu: {other} (connus: named, dynamic)"
                )));
            }
        };
        // Le rectangle suit la réduction : une région exprimée dans l'image d'origine ne
        // désignerait plus rien après un downscale, et une région fausse est pire qu'absente.
        let d = divisor.max(1);
        out.push(imgmetric::Roi {
            nom: r.name.clone(),
            rect: (r.rect[0] / d, r.rect[1] / d, r.rect[2] / d, r.rect[3] / d),
            kind,
        });
    }
    Ok(out)
}

/// Compare deux tampons RGBA8.
///
/// Fonction **pure**, et c'est elle qui porte la garde qui compte : `imgmetric::comparer`
/// **panique** (`assert_eq!`) quand un tampon ne fait pas `w × h × 4` octets. Une panique dans
/// un handler devient un `500` sans message ; ici, la taille est vérifiée avant, et le refus
/// dit les deux nombres.
///
/// # Errors
///
/// `Demande` sur dimensions nulles ou hors borne, tampon de mauvaise taille, base64 invalide,
/// région refusée, ou réduction impossible.
pub fn compare_report(request: CompareRequest) -> Result<CompareReport, ErreurSite> {
    let (mut w, mut h) = (request.width, request.height);
    let pixels = (w as usize)
        .checked_mul(h as usize)
        .ok_or_else(|| ErreurSite::Demande("dimensions absurdes".to_owned()))?;
    if pixels == 0 {
        return Err(ErreurSite::Demande(
            "largeur et hauteur doivent etre non nulles".to_owned(),
        ));
    }
    if pixels > MAX_BUFFER_PIXELS {
        return Err(ErreurSite::Demande(format!(
            "{pixels} pixels demandes, borne {MAX_BUFFER_PIXELS}"
        )));
    }
    if request.downscale > MAX_DOWNSCALE {
        return Err(ErreurSite::Demande(format!(
            "downscale {} hors bornes (0..={MAX_DOWNSCALE})",
            request.downscale
        )));
    }
    let expected = pixels * 4;
    let mut rendered = request.rendered.into_bytes()?;
    let mut reference = request.reference.into_bytes()?;
    if rendered.len() != expected {
        return Err(ErreurSite::Demande(format!(
            "tampon `rendered` de {} octets, {expected} attendus ({w}x{h}x4)",
            rendered.len()
        )));
    }
    if reference.len() != expected {
        return Err(ErreurSite::Demande(format!(
            "tampon `reference` de {} octets, {expected} attendus ({w}x{h}x4)",
            reference.len()
        )));
    }

    for _ in 0..request.downscale {
        if w < 2 || h < 2 {
            return Err(ErreurSite::Demande(format!(
                "reduction impossible en dessous de 2x2 (arret a {w}x{h})"
            )));
        }
        let (nw, nh, a) = imgmetric::downscale_lineaire_2x(w, h, &rendered);
        let (_, _, b) = imgmetric::downscale_lineaire_2x(w, h, &reference);
        w = nw;
        h = nh;
        rendered = a;
        reference = b;
    }

    let rois = build_rois(&request.regions, 1u32 << request.downscale)?;
    let report = imgmetric::comparer(w, h, &rendered, &reference, &rois);
    Ok(CompareReport {
        width: w,
        height: h,
        downscale: request.downscale,
        global: RegionScore::from(&report.global),
        regions: report.regions.iter().map(RegionScore::from).collect(),
        excluded_area_pct: finite(report.surface_exclue_pct),
        opaque_coverage_pct: finite(report.couverture_opaque_pct),
        block_ssim: BlockMap {
            width: report.bloc_w,
            height: report.bloc_h,
            values: report.bloc_ssim.iter().copied().map(finite).collect(),
        },
    })
}

/// Ce qu'un contrat de route publie : ce qu'il faut envoyer, et ce qui est refusé.
#[derive(Debug, Clone, Serialize)]
pub struct Contract {
    /// Le chemin concerné.
    pub route: &'static str,
    /// Les méthodes acceptées.
    pub methods: &'static [&'static str],
    /// Le module de `nie-formats` appelé.
    pub module: &'static str,
    /// À quoi sert la route, en une phrase.
    pub purpose: &'static str,
    /// Les champs attendus dans le corps.
    pub body: Vec<&'static str>,
    /// Les bornes appliquées.
    pub limits: Vec<String>,
    /// Ce que la route ne promet pas.
    pub caveat: &'static str,
}

/// `GET /api/v1/inspect/compare` — le contrat de la comparaison.
///
/// Un `GET` qui publie ce que le `POST` attend, plutôt qu'un `405` muet au premier client qui
/// explore : c'est le même parti que `/api/v1/regles/comparaison`.
pub async fn compare_contract() -> Json<Contract> {
    Json(Contract {
        route: "/api/v1/inspect/compare",
        methods: &["GET", "POST"],
        module: "nie_formats::imgmetric",
        purpose: "compare deux tampons RGBA8 fournis: T0 identite, T1 imperceptibilite, T2 SSIM",
        body: vec![
            "width: entier",
            "height: entier",
            "rendered: base64 | tableau d'octets (RGBA8, width*height*4)",
            "reference: base64 | tableau d'octets (meme taille)",
            "regions: [{name, rect: [x, y, w, h], kind: named | dynamic}]",
            "downscale: entier, nombre de reductions 2x avant comparaison",
        ],
        limits: vec![
            format!("pixels <= {MAX_BUFFER_PIXELS}"),
            format!("regions <= {MAX_REGIONS}, nom <= {MAX_REGION_NAME} caracteres"),
            format!("downscale <= {MAX_DOWNSCALE}"),
            "corps <= 2 Mio (limite du routeur, pas de ce module)".to_owned(),
        ],
        caveat: "ce service ne produit aucune capture du jeu: la reference est celle que \
                 l'appelant fournit, et un score ne mesure donc que ce qu'on lui a donne",
    })
}

/// `POST /api/v1/inspect/compare` — le rapport chiffré.
///
/// # Errors
///
/// `Demande` sur dimensions, tampons, régions ou réduction refusés.
pub async fn compare(
    Json(request): Json<CompareRequest>,
) -> Result<Json<CompareReport>, ErreurSite> {
    let report = tokio::task::spawn_blocking(move || compare_report(request)).await??;
    Ok(Json(report))
}

// ─── `planche` — ce qu'une planche contient, et comment la composer ─────────────────────────

/// Le jeton public d'une zone de couleur.
#[must_use]
pub fn zone_token(z: planche::Zone) -> &'static str {
    use planche::Zone as Z;
    match z {
        Z::Noir => "black",
        Z::Blanc => "white",
        Z::Rouge => "red",
        Z::Vert => "green",
        Z::Bleu => "blue",
        Z::Autre => "other",
    }
}

/// Le jeton public d'un rôle de planche.
#[must_use]
pub fn role_token(r: planche::Role) -> &'static str {
    use planche::Role as R;
    match r {
        R::Aplat => "flat",
        R::Zones => "zones",
        R::Trace => "stroke",
        R::Nuance => "shade",
    }
}

/// Le jeton public d'une convention de composition.
#[must_use]
pub fn convention_token(c: planche::Convention) -> &'static str {
    use planche::Convention as C;
    match c {
        C::SansMasque => "no-mask",
        C::Decoupe => "cutout",
        C::FondRouge => "red-background",
        C::TraceVert => "green-stroke",
        C::ZoneBleue => "blue-zone",
        C::Aplat => "flat",
        C::Indeterminee => "undetermined",
    }
}

/// La part d'une zone et son emprise.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ZoneShare {
    /// Jeton de la zone.
    pub zone: &'static str,
    /// Part des pixels, dans `[0, 1]`.
    pub share: f32,
    /// Emprise normalisée `[u_min, v_min, u_max, v_max]`, `null` si la zone est vide.
    pub extent: Option<[f32; 4]>,
}

/// Ce qu'une planche contient, en chiffres.
#[derive(Debug, Clone, Serialize)]
pub struct Measures {
    /// Largeur mesurée.
    pub width: u32,
    /// Hauteur mesurée.
    pub height: u32,
    /// Nombre de pixels mesurés.
    pub pixels: usize,
    /// Les six zones, dans l'ordre de leur indice.
    pub zones: Vec<ZoneShare>,
    /// Part de pixels d'encre — sombres ET opaques.
    pub ink_share: f32,
    /// Emprise de l'encre.
    pub ink_extent: Option<[f32; 4]>,
    /// Alpha moyen, dans `[0, 255]`.
    pub alpha_mean: f32,
    /// Alpha minimal rencontré.
    pub alpha_min: u8,
    /// Alpha maximal rencontré.
    pub alpha_max: u8,
    /// Pour chaque canal RGBA, vrai si toutes ses valeurs sont égales.
    pub constant_channels: [bool; 4],
    /// Nombre de couleurs distinctes, plafonné.
    pub colors: usize,
    /// Vrai si le plafond a été atteint : le compte est alors une borne inférieure.
    pub colors_capped: bool,
    /// Couleur moyenne, canal par canal.
    pub mean_color: [u8; 3],
    /// Une seule couleur sur toute la surface.
    pub flat: bool,
    /// La planche porte un trait dessiné.
    pub has_stroke: bool,
    /// Fond rouge franc et régions d'une autre couleur.
    pub zone_mask: bool,
    /// Le canal rouge ne porte aucune information spatiale.
    pub uniform_channel: bool,
}

impl From<&planche::Mesures> for Measures {
    fn from(m: &planche::Mesures) -> Self {
        Self {
            width: m.largeur,
            height: m.hauteur,
            pixels: m.pixels,
            zones: planche::Zone::toutes()
                .into_iter()
                .map(|z| ZoneShare {
                    zone: zone_token(z),
                    share: m.part(z),
                    extent: m.emprise(z),
                })
                .collect(),
            ink_share: m.part_encre,
            ink_extent: m.emprise_encre,
            alpha_mean: m.alpha_moyen,
            alpha_min: m.alpha_min,
            alpha_max: m.alpha_max,
            constant_channels: m.canaux_constants,
            colors: m.couleurs,
            colors_capped: m.couleurs_plafonnees,
            mean_color: m.couleur_moyenne,
            flat: m.est_aplat(),
            has_stroke: m.porte_un_trait(),
            zone_mask: m.est_masque_de_zones(),
            uniform_channel: m.canal_uniforme(),
        }
    }
}

/// Corps de `POST /api/v1/inspect/plate`.
#[derive(Debug, Clone, Deserialize)]
pub struct PlateRequest {
    /// Largeur de la planche.
    pub width: u32,
    /// Hauteur de la planche.
    pub height: u32,
    /// Le tampon de couleur, RGBA8.
    pub color: Buffer,
    /// Le tampon du masque compagnon, RGBA8 et de mêmes dimensions. Facultatif.
    pub mask: Option<Buffer>,
}

/// Corps de la réponse de `POST /api/v1/inspect/plate`.
#[derive(Debug, Clone, Serialize)]
pub struct PlateReport {
    /// Largeur mesurée.
    pub width: u32,
    /// Hauteur mesurée.
    pub height: u32,
    /// Mesures de la planche de couleur.
    pub color: Measures,
    /// Rôle de la planche de couleur.
    pub color_role: &'static str,
    /// Mesures du masque, quand il y en a un.
    pub mask: Option<Measures>,
    /// Rôle du masque.
    pub mask_role: Option<&'static str>,
    /// Convention de composition déduite des deux.
    pub convention: &'static str,
}

/// Mesure une planche et déduit sa convention de composition.
///
/// Fonction **pure**. Le masque est mesuré aux **mêmes** dimensions que la couleur : un masque
/// d'une autre taille n'est pas redimensionné, il est refusé — c'est la règle de
/// `planche::analyser`, et l'assouplir ici ferait diverger deux mesures du même objet.
///
/// # Errors
///
/// `Demande` sur dimensions nulles ou hors borne, base64 invalide, ou tampon trop court.
pub fn plate_report(request: PlateRequest) -> Result<PlateReport, ErreurSite> {
    let (w, h) = (request.width, request.height);
    let pixels = (w as usize)
        .checked_mul(h as usize)
        .ok_or_else(|| ErreurSite::Demande("dimensions absurdes".to_owned()))?;
    if pixels == 0 {
        return Err(ErreurSite::Demande(
            "largeur et hauteur doivent etre non nulles".to_owned(),
        ));
    }
    if pixels > MAX_BUFFER_PIXELS {
        return Err(ErreurSite::Demande(format!(
            "{pixels} pixels demandes, borne {MAX_BUFFER_PIXELS}"
        )));
    }
    let color_bytes = request.color.into_bytes()?;
    let color = planche::mesurer(w, h, &color_bytes).ok_or_else(|| {
        ErreurSite::Demande(format!(
            "tampon `color` de {} octets, {} attendus au minimum ({w}x{h}x4)",
            color_bytes.len(),
            pixels * 4
        ))
    })?;
    let mask = match request.mask {
        Some(b) => {
            let mask_bytes = b.into_bytes()?;
            Some(planche::mesurer(w, h, &mask_bytes).ok_or_else(|| {
                ErreurSite::Demande(format!(
                    "tampon `mask` de {} octets, {} attendus au minimum ({w}x{h}x4)",
                    mask_bytes.len(),
                    pixels * 4
                ))
            })?)
        }
        None => None,
    };

    Ok(PlateReport {
        width: w,
        height: h,
        color: Measures::from(&color),
        color_role: role_token(planche::Role::deriver(&color)),
        mask: mask.as_ref().map(Measures::from),
        mask_role: mask.as_ref().map(|m| role_token(planche::Role::deriver(m))),
        convention: convention_token(planche::Convention::deriver(&color, mask.as_ref())),
    })
}

/// `GET /api/v1/inspect/plate` — le contrat de la mesure de planche, seuils compris.
///
/// Les seuils sont publiés parce que ce sont **eux** qui décident du rôle et de la convention :
/// un client qui lit `role: "stroke"` sans savoir qu'il suffit de 0,5 % d'encre lirait un
/// verdict là où il n'y a qu'un seuil.
pub async fn plate_contract() -> Json<Contract> {
    Json(Contract {
        route: "/api/v1/inspect/plate",
        methods: &["GET", "POST"],
        module: "nie_formats::planche",
        purpose: "mesure une planche RGBA8 et son masque, et en deduit la convention de composition",
        body: vec![
            "width: entier",
            "height: entier",
            "color: base64 | tableau d'octets (RGBA8, width*height*4)",
            "mask: base64 | tableau d'octets, memes dimensions (facultatif)",
        ],
        limits: vec![
            format!("pixels <= {MAX_BUFFER_PIXELS}"),
            format!(
                "roles: {} | {} | {} | {}",
                role_token(planche::Role::Aplat),
                role_token(planche::Role::Zones),
                role_token(planche::Role::Trace),
                role_token(planche::Role::Nuance)
            ),
            format!(
                "seuils: encre > {}, fond de zones > {}, zone utile > {}, plafond de couleurs {}",
                planche::PART_ENCRE_TRACE,
                planche::PART_FOND_ZONES,
                planche::PART_ZONE_UTILE,
                planche::PLAFOND_COULEURS
            ),
            format!(
                "classification d'un pixel: noir < {}, blanc > {}, rouge > {} avec les autres < {}, vif > {}",
                planche::NOIR_MAX,
                planche::BLANC_MIN,
                planche::FOND_ROUGE_MIN,
                planche::FOND_ROUGE_AUTRES_MAX,
                planche::ZONE_VIVE_MIN
            ),
        ],
        caveat: "planche::analyser, qui prendrait un .g4tx entier et l'apparierait a son masque, \
                 est derriere la feature `textures`: elle n'est PAS appelee ici, et ce service ne \
                 lit donc aucune planche du jeu par elle-meme",
    })
}

/// `POST /api/v1/inspect/plate` — les mesures d'une planche fournie.
///
/// # Errors
///
/// `Demande` sur dimensions ou tampons refusés.
pub async fn plate(Json(request): Json<PlateRequest>) -> Result<Json<PlateReport>, ErreurSite> {
    let report = tokio::task::spawn_blocking(move || plate_report(request)).await??;
    Ok(Json(report))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::routing::{get, post};
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    use super::*;
    use crate::config::Config;
    use crate::vfs_index::IndexVfs;

    /// Monte ces routes sur un routeur nu — **pour les tests seuls**.
    ///
    /// Le câblage réel vit dans `crate::app`, et il n'est pas dupliqué ici : ce routeur ne sert
    /// qu'à prouver ce qu'aucune autre vérification ne prouve. `cargo clippy` compile un
    /// handler sans jamais vérifier qu'il **est** un handler : les bornes du trait `Handler`
    /// (ordre des extracteurs, `State` avant le corps, `Json` en dernier) ne sont contrôlées
    /// qu'au `route()`. Et la syntaxe de route d'axum 0.8 (`{*path}`) ne dégrade pas, elle
    /// **panique** au montage. Les deux défauts n'apparaîtraient donc qu'au démarrage du
    /// service ; ils apparaissent ici.
    fn routeur_de_test(etat: EtatSite) -> axum::Router {
        axum::Router::new()
            .route("/api/v1/inspect", get(catalog))
            .route("/api/v1/inspect/spritesheet/{*path}", get(spritesheet))
            .route("/api/v1/inspect/font/{*path}", get(font_metrics))
            .route("/api/v1/inspect/menu/{*path}", get(menu))
            .route("/api/v1/inspect/texture-chunk/{*path}", get(texture_chunk))
            .route("/api/v1/inspect/color", get(color))
            .route("/api/v1/inspect/compare", get(compare_contract))
            .route("/api/v1/inspect/compare", post(compare))
            .route("/api/v1/inspect/plate", get(plate_contract))
            .route("/api/v1/inspect/plate", post(plate))
            .with_state(etat)
    }

    /// État de test : index injecté, aucun contenu VFS, miroir absent, amont clos.
    fn etat() -> EtatSite {
        let config = Config {
            db: "/nonexistent/mirror.sqlite".into(),
            statique: "/nonexistent/dist".into(),
            amont: "http://127.0.0.1:1".to_owned(),
            ..Config::default()
        };
        let index = IndexVfs::depuis(vec![
            ("data/dx11/menu/title/a.g4tx".to_owned(), 100),
            (
                "data/common/font/font/font_def/font.cfg.bin".to_owned(),
                200,
            ),
            ("data/common/gamedata/menu/obj/x.objbin".to_owned(), 300),
        ]);
        EtatSite::pour_tests(config, index)
    }

    async fn appel(requete: axum::http::request::Builder, corps: Body) -> (StatusCode, Vec<u8>) {
        let r = routeur_de_test(etat())
            .oneshot(requete.body(corps).unwrap())
            .await
            .unwrap();
        let statut = r.status();
        let corps = r.into_body().collect().await.unwrap().to_bytes().to_vec();
        (statut, corps)
    }

    async fn get_json(uri: &str) -> (StatusCode, serde_json::Value) {
        let (statut, corps) = appel(Request::builder().uri(uri), Body::empty()).await;
        let valeur = serde_json::from_slice(&corps).unwrap_or(serde_json::Value::Null);
        (statut, valeur)
    }

    #[tokio::test]
    async fn les_routes_se_montent_et_repondent() {
        // Le catalogue : sept inspecteurs, et les comptes de l'index injecté.
        let (statut, v) = get_json("/api/v1/inspect").await;
        assert_eq!(statut, StatusCode::OK);
        assert_eq!(v["inspectors"].as_array().unwrap().len(), 7);
        assert_eq!(v["vfs_ready"], true);
        assert_eq!(v["corpus"]["atlases"], 1);
        assert_eq!(v["corpus"]["texture_chunks"], 0);
        // Les features de `nie-formats` sont celles de `super::formats` — source unique.
        assert_eq!(v["formats_features"], serde_json::json!(["std", "lua"]));

        // Les deux contrats publiés en `GET`, plutôt qu'un `405` muet.
        for uri in ["/api/v1/inspect/compare", "/api/v1/inspect/plate"] {
            let (statut, v) = get_json(uri).await;
            assert_eq!(statut, StatusCode::OK, "{uri}");
            assert_eq!(v["route"], uri);
            assert!(!v["body"].as_array().unwrap().is_empty(), "{uri}");
        }
    }

    #[tokio::test]
    async fn la_couleur_repond_et_refuse_une_demande_incomplete() {
        let (statut, v) = get_json("/api/v1/inspect/color?a=000000&b=ffffff").await;
        assert_eq!(statut, StatusCode::OK);
        assert!(v["delta_e2000"].as_f64().unwrap() > 50.0);
        assert_eq!(v["perceptible"], true);
        assert_eq!(v["a"]["linear"][0], 0.0);

        // Falsification : sans les deux paramètres, la route refuse au lieu de supposer.
        let (statut, _) = get_json("/api/v1/inspect/color?a=000000").await;
        assert_eq!(statut, StatusCode::BAD_REQUEST);
        let (statut, _) = get_json("/api/v1/inspect/color?a=zzz&b=000000").await;
        assert_eq!(statut, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn la_comparaison_passe_par_le_routeur_en_post() {
        let pixels = vec![7u8; 4 * 4 * 4];
        let corps = serde_json::json!({
            "width": 4,
            "height": 4,
            "rendered": pixels,
            "reference": pixels,
        })
        .to_string();
        let (statut, corps) = appel(
            Request::builder()
                .method("POST")
                .uri("/api/v1/inspect/compare")
                .header(header::CONTENT_TYPE, "application/json"),
            Body::from(corps),
        )
        .await;
        assert_eq!(statut, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(&corps).unwrap();
        assert_eq!(v["global"]["exact_pct"], 100.0);
        assert_eq!(v["width"], 4);

        // Falsification : un tampon mal dimensionné rend un 400, jamais une panique en 500.
        let corps = serde_json::json!({
            "width": 4,
            "height": 4,
            "rendered": vec![7u8; 4 * 4 * 4],
            "reference": vec![7u8; 3],
        })
        .to_string();
        let (statut, _) = appel(
            Request::builder()
                .method("POST")
                .uri("/api/v1/inspect/compare")
                .header(header::CONTENT_TYPE, "application/json"),
            Body::from(corps),
        )
        .await;
        assert_eq!(statut, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn les_routes_de_chemin_distinguent_absent_indisponible_et_sortant() {
        // Chemin indexé, contenu absent (l'état de test n'a pas de VFS) : 503, pas 404 — la
        // route existe, la capacité manque, et l'appelant peut réessayer.
        let (statut, _) = get_json("/api/v1/inspect/spritesheet/data/dx11/menu/title/a.g4tx").await;
        assert_eq!(statut, StatusCode::SERVICE_UNAVAILABLE);

        // Chemin absent de l'index : 404.
        let (statut, _) =
            get_json("/api/v1/inspect/font/data/dx11/menu/title/inconnu.cfg.bin").await;
        assert_eq!(statut, StatusCode::NOT_FOUND);

        // Traversée : 400, par la garde partagée avec `/f` et `/api/v1/lua`.
        let (statut, _) = get_json("/api/v1/inspect/menu/data/../../etc/passwd").await;
        assert_eq!(statut, StatusCode::BAD_REQUEST);

        // Forme inconnue : refusée AVANT toute lecture, donc 400 et non 503.
        let (statut, _) =
            get_json("/api/v1/inspect/spritesheet/data/dx11/menu/title/a.g4tx?form=xml").await;
        assert_eq!(statut, StatusCode::BAD_REQUEST);
        let (statut, _) =
            get_json("/api/v1/inspect/texture-chunk/data/dx11/menu/title/a.g4tx?form=raw").await;
        assert_eq!(statut, StatusCode::BAD_REQUEST);
        // Police hors bornes : idem.
        let (statut, _) =
            get_json("/api/v1/inspect/font/data/common/font/font/font_def/font.cfg.bin?font=9")
                .await;
        assert_eq!(statut, StatusCode::BAD_REQUEST);
    }

    /// Un tampon RGBA uni.
    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        rgba.iter()
            .copied()
            .cycle()
            .take((w as usize) * (h as usize) * 4)
            .collect()
    }

    #[test]
    fn base64_fait_l_aller_retour_et_refuse_le_reste() {
        // Vecteurs standard : la valeur attendue est connue, pas recalculee par le meme code.
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(decode_base64("aGVsbG8").unwrap(), b"hello");
        assert_eq!(decode_base64("aGVs\nbG8=").unwrap(), b"hello");
        assert_eq!(decode_base64("").unwrap(), Vec::<u8>::new());
        // Alphabet URL-safe : `-` et `_` valent `+` et `/`.
        assert_eq!(
            decode_base64("--__").unwrap(),
            decode_base64("++//").unwrap()
        );
        // Falsification : chacune de ces entrees DOIT rougir.
        for mauvais in ["aGVsbG8*", "a", "aGVsbG8=x", "!!!!"] {
            let e = decode_base64(mauvais)
                .unwrap_err_or_panic(&format!("{mauvais} aurait du etre refuse"));
            assert_eq!(e.statut().as_u16(), 400, "{mauvais}");
        }
    }

    /// Petite aide de falsification : un `Result` qui aurait dû être une erreur.
    trait UnwrapErrOrPanic<T> {
        fn unwrap_err_or_panic(self, message: &str) -> ErreurSite;
    }

    impl<T> UnwrapErrOrPanic<T> for Result<T, ErreurSite> {
        fn unwrap_err_or_panic(self, message: &str) -> ErreurSite {
            match self {
                Ok(_) => panic!("{message}"),
                Err(e) => e,
            }
        }
    }

    #[test]
    fn l_url_d_atlas_refuse_ce_qui_sort_de_l_attribut() {
        assert!(check_url("/assets/tex/data/dx11/menu/x.png").is_ok());
        assert!(check_url("https://cdn.example.com/a.png?v=2").is_ok());
        // Falsification : l'injection SVG qui a motive la garde.
        for mauvais in [
            "",
            "a\"/><script>alert(1)</script>",
            "a'onload='x",
            "a b.png",
            "a\\b.png",
            "a<b>.png",
            "a&b.png",
            "a\u{0}.png",
        ] {
            let e = check_url(mauvais).unwrap_err_or_panic(&format!("{mauvais} accepte"));
            assert_eq!(e.statut().as_u16(), 400, "{mauvais}");
        }
    }

    #[test]
    fn l_url_par_defaut_suit_les_deux_formes_de_l_amont() {
        assert_eq!(
            default_atlas_url("data/dx11/menu/x.g4tx", None),
            "/assets/tex/data/dx11/menu/x.png"
        );
        assert_eq!(
            default_atlas_url("data/dx11/menu/x.g4tx", Some("eq_ac0100101")),
            "/assets/tex/data/dx11/menu/x.g4tx/eq_ac0100101.png"
        );
        // Un chemin sans suffixe n'est pas tronque au hasard.
        assert_eq!(default_atlas_url("data/x", None), "/assets/tex/data/x.png");
    }

    #[test]
    fn les_formes_et_modes_connus_passent_les_autres_non() {
        assert_eq!(SheetForm::parse(None).unwrap(), SheetForm::Json);
        assert_eq!(SheetForm::parse(Some(" ")).unwrap(), SheetForm::Json);
        assert_eq!(SheetForm::parse(Some("css")).unwrap(), SheetForm::Css);
        assert_eq!(SheetForm::parse(Some("svg")).unwrap(), SheetForm::Svg);
        assert_eq!(
            SheetForm::parse(Some("xml"))
                .unwrap_err_or_panic("xml accepte")
                .statut()
                .as_u16(),
            400
        );
        assert_eq!(
            css_mode(Some("mask")).unwrap(),
            sprite_sheet::ModeCss::Masque
        );
        assert_eq!(
            css_mode(Some("masque"))
                .unwrap_err_or_panic("masque accepte")
                .statut()
                .as_u16(),
            400,
            "le jeton public est anglais: `mask`, pas `masque`"
        );
        // Les trois types de contenu sont distincts : servir du CSS en `application/json` le
        // ferait telecharger au lieu de s'appliquer.
        let mut tries = vec![
            SheetForm::Json.content_type(),
            SheetForm::Css.content_type(),
            SheetForm::Svg.content_type(),
        ];
        tries.sort_unstable();
        tries.dedup();
        assert_eq!(tries.len(), 3);
    }

    #[test]
    fn une_feuille_de_sprites_refuse_ce_qui_n_est_pas_un_g4tx() {
        let q = SheetQuery::default();
        let e = render_sheet("data/x.g4tx", b"pas un atlas", &q)
            .unwrap_err_or_panic("des octets quelconques ont ete acceptes");
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("G4TX"), "{e}");
    }

    #[test]
    fn la_police_demandee_est_bornee_a_deux() {
        assert_eq!(font_index(None).unwrap(), 0);
        assert_eq!(font_index(Some(1)).unwrap(), 1);
        assert_eq!(
            font_index(Some(2))
                .unwrap_err_or_panic("police 2 acceptee")
                .statut()
                .as_u16(),
            400
        );
    }

    #[test]
    fn les_metriques_de_police_refusent_ce_qui_n_est_pas_un_cfg_bin() {
        let e = font_report(
            "data/x/font.cfg.bin",
            b"trop court",
            &FontQuery::default(),
            &DemandePage::default(),
        )
        .unwrap_err_or_panic("des octets quelconques ont ete acceptes");
        assert_eq!(e.statut().as_u16(), 400);
    }

    #[test]
    fn la_geometrie_de_menu_refuse_ce_qui_n_est_pas_un_objbin() {
        let e = menu_geometry("data/x.objbin", b"pas un objbin", MenuSources::default())
            .unwrap_err_or_panic("des octets quelconques ont ete acceptes");
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("OBJBIN"), "{e}");
    }

    #[test]
    fn le_compagnon_se_resout_directement_puis_par_nom_puis_pas_du_tout() {
        let index = IndexVfs::depuis(vec![
            (
                "data/common/menu/100_mainmenu/mainmenu01/mainmenu01_06/mainmenu01_06.g4pkm"
                    .to_owned(),
                6912,
            ),
            ("data/dx11/menu/ailleurs/fr/win01.g4tx".to_owned(), 100),
            ("data/dx11/menu/ailleurs/en/win01.g4tx".to_owned(), 100),
            ("data/dx11/menu/ailleurs/atlas/win02.g4tx".to_owned(), 100),
        ]);

        // 1. le chemin logique tel quel, prefixe de `data/`.
        assert_eq!(
            resolve_companion(
                &index,
                "common/menu/100_mainmenu/mainmenu01/mainmenu01_06/mainmenu01_06.g4pkm",
                "fr"
            )
            .as_deref(),
            Some("data/common/menu/100_mainmenu/mainmenu01/mainmenu01_06/mainmenu01_06.g4pkm")
        );

        // 2. par nom de fichier, la locale demandee gagne.
        assert_eq!(
            resolve_companion(&index, "menu/autre/chemin/win01.g4tx", "fr").as_deref(),
            Some("data/dx11/menu/ailleurs/fr/win01.g4tx")
        );
        assert_eq!(
            resolve_companion(&index, "menu/autre/chemin/win01.g4tx", "en").as_deref(),
            Some("data/dx11/menu/ailleurs/en/win01.g4tx")
        );
        // 3. un dossier qui n'est PAS un tag de locale prime sur toute variante traduite.
        assert_eq!(
            resolve_companion(&index, "menu/autre/win02.g4tx", "fr").as_deref(),
            Some("data/dx11/menu/ailleurs/atlas/win02.g4tx")
        );

        // 4. le cas REEL du `<LG>` : le chemin declare n'existe sous aucune forme litterale, et
        // seule la recherche par nom le resout. Chemin releve le 2026-09-06 dans
        // `btl01_02_action_stone_base.objbin`.
        assert_eq!(
            resolve_companion(&index, "#/menu/x/<LG>/win01.g4tx", "en").as_deref(),
            Some("data/dx11/menu/ailleurs/en/win01.g4tx")
        );

        // 5. absent : `None`, jamais un repli. Cas REEL, mesure le 2026-09-06 —
        // `mainmenu01_04_menu_list.objbin` reference un g4pkm qui n'existe pas dans ce build.
        assert_eq!(
            resolve_companion(&index, "common/menu/x/mainmenu01_04.g4pkm", "fr"),
            None
        );
        assert_eq!(resolve_companion(&index, "", "fr"), None);
    }

    #[test]
    fn les_jetons_publics_sont_distincts_et_anglais() {
        // Composants : neuf variantes, neuf jetons.
        let composants = [
            "render",
            "animation",
            "text",
            "primitive",
            "attach_locator",
            "collision",
            "sound_cmd",
            "mesh_visible",
            "unknown",
        ];
        let mut tries = composants.to_vec();
        tries.sort_unstable();
        tries.dedup();
        assert_eq!(tries.len(), composants.len());

        // NXTCH : neuf formats, neuf jetons, aucun nom de variante Rust.
        use nxtch::NxtchFormat as F;
        let formats = [
            F::Unknown,
            F::Bc1,
            F::Bc2,
            F::Bc3,
            F::Bc4,
            F::Bc5,
            F::Bc6,
            F::Bc7,
            F::Rgba8,
        ];
        let mut jetons: Vec<&str> = formats.iter().map(|f| nxtch_format_token(*f)).collect();
        assert_eq!(nxtch_format_token(F::Bc7), "bc7");
        assert_eq!(nxtch_format_token(F::Rgba8), "rgba8");
        jetons.sort_unstable();
        jetons.dedup();
        assert_eq!(jetons.len(), formats.len());

        // Zones, roles, conventions : les jetons sont ANGLAIS, la ou `nom()` rend le libelle
        // d'affichage francais du module. Le test fige le choix.
        assert_eq!(zone_token(planche::Zone::Rouge), "red");
        assert_eq!(planche::Zone::Rouge.nom(), "rouge");
        assert_eq!(role_token(planche::Role::Trace), "stroke");
        assert_eq!(
            convention_token(planche::Convention::TraceVert),
            "green-stroke"
        );
        let mut conventions: Vec<&str> = [
            planche::Convention::SansMasque,
            planche::Convention::Decoupe,
            planche::Convention::FondRouge,
            planche::Convention::TraceVert,
            planche::Convention::ZoneBleue,
            planche::Convention::Aplat,
            planche::Convention::Indeterminee,
        ]
        .iter()
        .map(|c| convention_token(*c))
        .collect();
        let avant = conventions.len();
        conventions.sort_unstable();
        conventions.dedup();
        assert_eq!(conventions.len(), avant);
    }

    #[test]
    fn le_bloc_de_texture_dit_ce_qu_il_a_vu() {
        let e = chunk_report("data/x.g4tx", b"pas un conteneur", &ChunkQuery::default())
            .unwrap_err_or_panic("des octets quelconques ont ete acceptes");
        assert_eq!(e.statut().as_u16(), 400);
        // L'erreur PUBLIE les premiers octets : sans cela, le prochain refait le xxd.
        assert!(format!("{e}").contains("premiers octets"), "{e}");

        // Falsification de la borne de `block_height_log2` : 5 passe la borne, 6 non.
        let q = ChunkQuery {
            block_height_log2: Some(6),
            ..ChunkQuery::default()
        };
        let e = chunk_linear(b"pas un conteneur", &q)
            .unwrap_err_or_panic("block_height_log2=6 accepte");
        assert!(format!("{e}").contains("hors bornes"), "{e}");
    }

    #[test]
    fn un_en_tete_nxtch_fabrique_se_lit_et_se_chiffre() {
        // 48 octets d'en-tete : magic, puis les entiers aux decalages du portage C#.
        let mut chunk = vec![0u8; nxtch::NXTCH_HEADER_SIZE + 8 * 8 * 16];
        chunk[0..5].copy_from_slice(&nxtch::NXTCH_MAGIC_BYTES);
        chunk[0x14..0x18].copy_from_slice(&32i32.to_le_bytes()); // width
        chunk[0x18..0x1c].copy_from_slice(&32i32.to_le_bytes()); // height
        chunk[0x24..0x28].copy_from_slice(&0x07i32.to_le_bytes()); // BC7
        chunk[0x28..0x2c].copy_from_slice(&1i32.to_le_bytes()); // 1 mip

        let r = chunk_report("data/x.nxtch", &chunk, &ChunkQuery::default())
            .expect("un en-tete NXTCH valide doit se lire");
        assert_eq!(r.source, "raw");
        assert_eq!(r.header.format, "bc7");
        assert_eq!(r.header.width, 32);
        assert!(r.block_compressed);
        assert_eq!(r.block_byte_size, 16);
        // 8x8 blocs de 16 octets = 1 024 : le chiffre vient de la formule, pas d'un compte.
        assert_eq!(r.expected_data_bytes, 1024);
        assert!(r.complete, "le tampon fabrique porte bien ses 1 024 octets");

        // Delinearisation : la sortie fait exactement la taille lineaire du mip 0.
        let linear = chunk_linear(&chunk, &ChunkQuery::default()).expect("delinearisation");
        assert_eq!(linear.len(), 1024);
    }

    #[test]
    fn le_corpus_se_compte_et_le_corpus_nxtch_est_vide() {
        let index = IndexVfs::depuis(vec![
            ("data/dx11/menu/a.g4tx".to_owned(), 10),
            ("data/dx11/menu/b.g4tx".to_owned(), 20),
            ("data/common/font/font/font_def/font.cfg.bin".to_owned(), 30),
            ("data/common/font/font_color.cfg.bin".to_owned(), 40),
            ("data/common/gamedata/menu/obj/x.objbin".to_owned(), 50),
        ]);
        let c = count(&index);
        assert_eq!(c.atlases, 2);
        // `font_color.cfg.bin` n'est PAS une table de glyphes : le suffixe porte le `/`.
        assert_eq!(c.fonts, 1);
        assert_eq!(c.menu_objects, 1);
        assert_eq!(
            c.texture_chunks, 0,
            "aucun .nxtch: c'est la reserve que la route doit annoncer"
        );
        // Le catalogue rend un inspecteur par module servi, avec son corpus.
        let l = inspectors(Some(c));
        assert_eq!(l.len(), 7);
        let sheet = l.iter().find(|i| i.name == "spritesheet").unwrap();
        assert_eq!(sheet.corpus, Some(2));
        let chunk = l.iter().find(|i| i.name == "texture-chunk").unwrap();
        assert_eq!(chunk.corpus, Some(0));
        assert!(chunk.caveat.is_some(), "un corpus vide se DIT");
        // Chaque inspecteur nomme le module de nie-formats qu'il appelle, et les six modules
        // du lot y sont tous.
        let modules: Vec<&str> = l.iter().map(|i| i.module).collect();
        for m in [
            "nie_formats::sprite_sheet",
            "nie_formats::font",
            "nie_formats::menu",
            "nie_formats::nxtch",
            "nie_formats::imgmetric",
            "nie_formats::planche",
        ] {
            assert!(modules.contains(&m), "{m} absent du catalogue");
        }
    }

    #[test]
    fn la_couleur_se_lit_en_hexadecimal_et_pas_autrement() {
        assert_eq!(parse_hex_color("#FF8000").unwrap(), (255, 128, 0));
        assert_eq!(parse_hex_color("ff8000").unwrap(), (255, 128, 0));
        for mauvais in ["", "#FFF", "GGGGGG", "ff80000", "ff 800"] {
            assert_eq!(
                parse_hex_color(mauvais)
                    .unwrap_err_or_panic(&format!("{mauvais} accepte"))
                    .statut()
                    .as_u16(),
                400,
                "{mauvais}"
            );
        }
        // ΔE2000 d'une couleur avec elle-meme vaut 0 ; le noir et le blanc en sont loin.
        assert!(imgmetric::delta_e2000((10, 20, 30), (10, 20, 30)).abs() < 1e-9);
        assert!(imgmetric::delta_e2000((0, 0, 0), (255, 255, 255)) > 50.0);
        // La lumiere lineaire n'est PAS le canal divise par 255 : 128 donne ~0,216.
        let c = Color::new(128, 128, 128);
        assert!((c.linear[0] - 0.2158).abs() < 1e-3, "{:?}", c.linear);
    }

    #[test]
    fn la_comparaison_refuse_un_tampon_de_mauvaise_taille() {
        // C'est LA garde : `imgmetric::comparer` panique sur un tampon mal dimensionne, et une
        // panique dans un handler devient un 500 muet.
        let e = compare_report(CompareRequest {
            width: 4,
            height: 4,
            rendered: Buffer::Bytes(vec![0; 4 * 4 * 4]),
            reference: Buffer::Bytes(vec![0; 4 * 4 * 4 - 1]),
            regions: Vec::new(),
            downscale: 0,
        })
        .unwrap_err_or_panic("un tampon trop court a ete accepte");
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("reference"), "{e}");

        // Dimensions absurdes : refusees AVANT toute allocation.
        let e = compare_report(CompareRequest {
            width: 100_000,
            height: 100_000,
            rendered: Buffer::Bytes(Vec::new()),
            reference: Buffer::Bytes(Vec::new()),
            regions: Vec::new(),
            downscale: 0,
        })
        .unwrap_err_or_panic("10^10 pixels acceptes");
        assert!(format!("{e}").contains("borne"), "{e}");

        // Dimensions nulles.
        assert!(
            compare_report(CompareRequest {
                width: 0,
                height: 4,
                rendered: Buffer::Bytes(Vec::new()),
                reference: Buffer::Bytes(Vec::new()),
                regions: Vec::new(),
                downscale: 0,
            })
            .is_err()
        );
    }

    #[test]
    fn deux_tampons_identiques_donnent_une_identite_parfaite() {
        let buf = solid(8, 8, [12, 34, 56, 255]);
        let r = compare_report(CompareRequest {
            width: 8,
            height: 8,
            rendered: Buffer::Bytes(buf.clone()),
            reference: Buffer::Bytes(buf.clone()),
            regions: vec![RegionRequest {
                name: "coin".to_owned(),
                rect: [0, 0, 4, 4],
                kind: None,
            }],
            downscale: 0,
        })
        .expect("deux tampons identiques doivent se comparer");
        assert_eq!(r.width, 8);
        assert_eq!(r.global.exact_pct, Some(100.0));
        assert_eq!(r.opaque_coverage_pct, Some(100.0));
        assert_eq!(r.regions.len(), 1);
        assert_eq!(r.regions[0].name, "coin");
        assert_eq!(
            r.block_ssim.values.len(),
            r.block_ssim.width as usize * r.block_ssim.height as usize
        );

        // Falsification : deux tampons DIFFERENTS ne doivent pas rendre 100 %.
        let autre = solid(8, 8, [200, 34, 56, 255]);
        let r = compare_report(CompareRequest {
            width: 8,
            height: 8,
            rendered: Buffer::Bytes(buf),
            reference: Buffer::Bytes(autre),
            regions: Vec::new(),
            downscale: 0,
        })
        .expect("comparaison");
        assert_eq!(r.global.exact_pct, Some(0.0));
        assert!(r.global.delta_e_max.unwrap() > 1.0);
    }

    #[test]
    fn la_reduction_divise_les_dimensions_et_reste_bornee() {
        let buf = solid(8, 8, [10, 20, 30, 255]);
        let r = compare_report(CompareRequest {
            width: 8,
            height: 8,
            rendered: Buffer::Bytes(buf.clone()),
            reference: Buffer::Bytes(buf.clone()),
            regions: vec![RegionRequest {
                name: "moitie".to_owned(),
                rect: [4, 4, 4, 4],
                kind: Some("dynamic".to_owned()),
            }],
            downscale: 2,
        })
        .expect("reduction");
        assert_eq!((r.width, r.height), (2, 2));
        assert_eq!(r.downscale, 2);
        // La region dynamique a suivi la reduction : 4x4 sur 8x8 devient 1x1 sur 2x2, soit un
        // quart de la surface exclue. Une region non divisee aurait exclu 0 pixel.
        assert_eq!(r.excluded_area_pct, Some(25.0));

        // Falsification des deux bornes.
        for (downscale, motif) in [(5u32, "hors bornes"), (4, "reduction impossible")] {
            let e = compare_report(CompareRequest {
                width: 8,
                height: 8,
                rendered: Buffer::Bytes(buf.clone()),
                reference: Buffer::Bytes(buf.clone()),
                regions: Vec::new(),
                downscale,
            })
            .unwrap_err_or_panic(&format!("downscale={downscale} accepte"));
            assert!(format!("{e}").contains(motif), "{downscale}: {e}");
        }
    }

    #[test]
    fn une_region_de_genre_inconnu_est_refusee() {
        let buf = solid(4, 4, [0, 0, 0, 255]);
        let e = compare_report(CompareRequest {
            width: 4,
            height: 4,
            rendered: Buffer::Bytes(buf.clone()),
            reference: Buffer::Bytes(buf),
            regions: vec![RegionRequest {
                name: "x".to_owned(),
                rect: [0, 0, 1, 1],
                kind: Some("dynamique".to_owned()),
            }],
            downscale: 0,
        })
        .unwrap_err_or_panic("un genre francais a ete accepte");
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("named"), "{e}");
    }

    #[test]
    fn une_planche_unie_est_un_aplat_et_un_tampon_court_est_refuse() {
        let plate = plate_report(PlateRequest {
            width: 4,
            height: 4,
            color: Buffer::Bytes(solid(4, 4, [200, 30, 30, 255])),
            mask: None,
        })
        .expect("une planche unie doit se mesurer");
        assert_eq!(plate.color.pixels, 16);
        assert_eq!(plate.color.colors, 1);
        assert!(plate.color.flat);
        assert_eq!(plate.color_role, "flat");
        assert_eq!(plate.convention, "flat");
        // Six zones, dans l'ordre de leur indice, et le rouge franc est celle qui porte tout.
        assert_eq!(plate.color.zones.len(), 6);
        let rouge = plate.color.zones.iter().find(|z| z.zone == "red").unwrap();
        assert!((rouge.share - 1.0).abs() < 1e-6);
        assert!(rouge.extent.is_some());

        // Falsification : un tampon plus court que `width * height * 4` est refuse — sans
        // cela, `mesurer` rendrait `None` et le handler un succes vide.
        let e = plate_report(PlateRequest {
            width: 4,
            height: 4,
            color: Buffer::Bytes(vec![0; 4 * 4 * 4 - 1]),
            mask: None,
        })
        .unwrap_err_or_panic("un tampon trop court a ete accepte");
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("color"), "{e}");
    }

    #[test]
    fn un_masque_uniforme_ne_decoupe_rien_un_masque_de_zones_si() {
        // La planche porte DEUX couleurs, et aucune n'est de l'encre (somme des canaux au-delà
        // de `ENCRE_SOMME_MAX`). Les deux conditions comptent : unie, elle donnerait
        // `Convention::Aplat` quoi que dise le masque ; encrée, elle donnerait `FondRouge`.
        // C'est exactement ce que le premier jet de ce test a produit — la règle de `deriver`
        // est ordonnée, et un témoin mal choisi teste une autre branche que celle qu'on croit.
        let mut color = solid(4, 4, [220, 220, 40, 255]);
        color[0..4].copy_from_slice(&[200, 200, 60, 255]);

        // Masque uniforme : `Convention::deriver` le rejette (canal rouge constant), et la
        // planche se compose seule.
        let uniforme = plate_report(PlateRequest {
            width: 4,
            height: 4,
            color: Buffer::Bytes(color.clone()),
            mask: Some(Buffer::Bytes(solid(4, 4, [128, 128, 128, 255]))),
        })
        .expect("mesure");
        assert_eq!(uniforme.convention, "no-mask");
        assert_eq!(uniforme.mask_role.unwrap(), "flat");

        // Masque de zones : fond rouge franc sur trois quarts, vert vif sur le dernier.
        let mut masque = solid(4, 4, [200, 30, 30, 255]);
        for i in 0..4 {
            masque[i * 4..i * 4 + 4].copy_from_slice(&[10, 200, 10, 255]);
        }
        let zones = plate_report(PlateRequest {
            width: 4,
            height: 4,
            color: Buffer::Bytes(color),
            mask: Some(Buffer::Bytes(masque)),
        })
        .expect("mesure");
        assert!(zones.mask.as_ref().unwrap().zone_mask);
        assert_eq!(zones.mask_role.unwrap(), "zones");
        assert_eq!(zones.convention, "green-stroke");
    }

    #[test]
    fn un_flottant_non_fini_devient_null_par_decision() {
        assert_eq!(finite(1.5), Some(1.5));
        assert_eq!(finite(f64::NAN), None);
        assert_eq!(finite(f64::INFINITY), None);
        // Et il sort bien en `null`, pas en `0`.
        let v = serde_json::to_value(finite(f64::NAN)).unwrap();
        assert!(v.is_null());
    }

    #[test]
    fn le_tampon_accepte_les_deux_formes_du_json() {
        let par_tableau: Buffer = serde_json::from_str("[1,2,3]").unwrap();
        assert_eq!(par_tableau.into_bytes().unwrap(), vec![1, 2, 3]);
        let par_base64: Buffer = serde_json::from_str("\"aGVsbG8=\"").unwrap();
        assert_eq!(par_base64.into_bytes().unwrap(), b"hello");
        // Falsification : une chaine qui n'est pas du base64 doit rougir a la conversion, pas
        // passer pour un tampon vide.
        let mauvais: Buffer = serde_json::from_str("\"pas du base64 !\"").unwrap();
        assert!(mauvais.into_bytes().is_err());
    }
}
