//! Aphrody — tout ce que `nie-aphrody` sait, servi nativement.
//!
//! ## Ce que cette crate porte, et pourquoi le site l'expose en entier
//!
//! `nie-aphrody` n'est pas un dossier d'images : c'est le runtime typé du personnage. Elle
//! embarque au build (`include_str!`/`include_bytes!`) le package Codex Pet v2 — atlas
//! 1536×2288 en 8×11 cellules de 192×208, onze animations, 74 frames, seize poses de regard —
//! et le dossier documentaire du personnage : identité trilingue, trois séries du jeu,
//! statistiques, techniques, auras, variantes, assets VFS. Elle porte en plus une chaîne
//! d'image complète (`pixel`) : mesure de palette en Oklab, comparaison, rastérisation et
//! **vectorisation** — le SVG pixel-perfect —, et un contrôle de conformité au format Codex
//! (`codex`).
//!
//! Le site republie tout cela sans en garder de copie. Recopier des frames dans
//! `apps/nie-web/public/` aurait créé un second jeu d'octets qui se périme en silence : le
//! manifeste porte un condensé par frame, la copie ne le porterait plus, et un réexport ne
//! ferait diverger que l'un des deux.
//!
//! ## Les sept routes, et ce que chacune tire de la crate
//!
//! | Route | Source |
//! |---|---|
//! | `/pet/aphrody.json` | `AnimationsManifest` — réduit aux rectangles et aux durées |
//! | `/pet/atlas.webp` | `BUNDLED_ATLAS_WEBP`, sans perte, RGBA identique au PNG |
//! | `/pet/frame/{animation}/{n}.png` | `Pet::extract` + `assets::encoder_png` — les 74 frames |
//! | `/pet/aphrody.svg` | `pixel::vectoriser` — le décalque vectoriel |
//! | `/api/v1/aphrody` | `BUNDLED_DOSSIER_JSON` — le dossier du personnage |
//! | `/api/v1/aphrody/diagnostic` | `Pet::diagnose` + `codex::conformite` |
//! | `/api/v1/aphrody/palette` | `pixel::mesurer` + `pixel::tokens_css` |
//!
//! ## Le coût, mesuré et payé une seule fois
//!
//! `Pet::bundled()` décode l'atlas en RGBA : 1536 × 2288 × 4 = 14 Mio résidents. Les routes qui
//! en ont besoin (frames, SVG, palette, diagnostic) le partagent par un `OnceLock` ; celles qui
//! n'en ont pas besoin (manifeste, atlas, dossier) ne le déclenchent jamais. Un service qui ne
//! sert que la page d'accueil ne paie donc rien de cette chaîne.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use nie_aphrody::{
    AnimationsManifest, BUNDLED_ANIMATIONS_JSON, BUNDLED_ATLAS_WEBP, BUNDLED_DOSSIER_JSON, Pet,
    assets, codex, pixel,
};

/// L'URL de l'atlas, telle que le manifeste l'annonce au navigateur.
pub const URL_ATLAS: &str = "/pet/atlas.webp";

/// L'animation dont la première frame sert de portrait — vignette, SVG, palette.
///
/// `look-neutral` plutôt qu'`idle` : c'est la pose de repos frontale du package, celle qui ne
/// dépend d'aucune phase d'animation. Prendre `idle[0]` donnerait un portrait correct un cycle
/// sur six et une paupière à demi close le reste du temps.
const POSE_PORTRAIT: &str = "look-neutral";

/// L'identité du pet, réduite à ce qu'une page affiche.
#[derive(Debug, Serialize)]
pub struct IdentitePet {
    /// Identifiant du package (`aphrody`).
    pub id: String,
    /// Nom affichable.
    pub nom: String,
    /// Version du jeu de sprites, pour qu'un client sache s'il a changé.
    pub version: u32,
}

/// Les dimensions de l'atlas et de ses cellules.
#[derive(Debug, Serialize)]
pub struct AtlasPet {
    /// Où chercher l'image.
    pub url: &'static str,
    /// Largeur de l'atlas, en pixels.
    pub largeur: u32,
    /// Hauteur de l'atlas, en pixels.
    pub hauteur: u32,
    /// Largeur d'une cellule.
    pub cellule_l: u32,
    /// Hauteur d'une cellule.
    pub cellule_h: u32,
}

/// Une frame : son rectangle dans l'atlas, sa durée, sa direction.
#[derive(Debug, Serialize)]
pub struct FramePet {
    /// Abscisse dans l'atlas.
    pub x: u32,
    /// Ordonnée dans l'atlas.
    pub y: u32,
    /// Durée d'affichage, en millisecondes. Absente pour une pose, qui ne s'enchaîne pas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u32>,
    /// Le nom de la direction, pour les poses de regard.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
}

/// Une animation séquencée.
#[derive(Debug, Serialize)]
pub struct AnimationPet {
    /// Durée totale d'un cycle, en millisecondes.
    pub duree_ms: u32,
    /// Ce à quoi l'animation sert, tel que le package le déclare.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Les frames, dans l'ordre.
    pub frames: Vec<FramePet>,
}

/// Le jeu de poses directionnelles.
#[derive(Debug, Serialize)]
pub struct RegardPet {
    /// Pas angulaire entre deux poses, en degrés (22,5° pour seize poses).
    pub pas_degres: f64,
    /// Les poses, dans l'ordre du package : la première regarde vers le haut, puis sens horaire.
    pub poses: Vec<FramePet>,
}

/// Le manifeste réduit publié à `/pet/aphrody.json`.
#[derive(Debug, Serialize)]
pub struct ManifestePet {
    /// Qui est ce pet.
    pub pet: IdentitePet,
    /// Son atlas.
    pub atlas: AtlasPet,
    /// Les animations séquencées, par nom.
    pub animations: BTreeMap<String, AnimationPet>,
    /// Les poses fixes, par nom — elles n'ont ni durée ni suite.
    pub poses: BTreeMap<String, FramePet>,
    /// Les seize poses de regard, à part parce qu'un angle les choisit, pas une horloge.
    pub regard: Option<RegardPet>,
}

/// Le manifeste, construit une seule fois.
static MANIFESTE: OnceLock<Result<String, String>> = OnceLock::new();

/// L'atlas décodé, partagé par les routes qui travaillent les pixels.
static PET: OnceLock<Result<Pet, String>> = OnceLock::new();

/// Le SVG vectorisé, calculé une seule fois.
static SVG: OnceLock<Result<String, String>> = OnceLock::new();

/// Le rapport de diagnostic sérialisé.
static DIAGNOSTIC: OnceLock<Result<String, String>> = OnceLock::new();

/// La palette mesurée, sérialisée.
static PALETTE: OnceLock<Result<String, String>> = OnceLock::new();

/// L'atlas décodé, ou l'erreur de package.
fn pet() -> Result<&'static Pet, &'static str> {
    match PET.get_or_init(|| Pet::bundled().map_err(|e| e.to_string())) {
        Ok(p) => Ok(p),
        Err(e) => Err(e.as_str()),
    }
}

/// Construit le manifeste réduit à partir du package embarqué.
///
/// Rend `Err` quand le package est incohérent — un rectangle qui déborde de l'atlas, une
/// animation vide. C'est la seule occasion de le voir : côté navigateur, une cellule hors
/// bornes s'affiche en transparent et le pet paraît simplement absent.
fn construire() -> Result<ManifestePet, String> {
    let manifeste: AnimationsManifest =
        serde_json::from_str(BUNDLED_ANIMATIONS_JSON).map_err(|e| e.to_string())?;
    let atlas = &manifeste.atlas;

    let mut animations = BTreeMap::new();
    let mut poses = BTreeMap::new();
    let mut regard = None;
    for (nom, animation) in &manifeste.animations {
        if animation.frames.is_empty() {
            return Err(format!("animation « {nom} » sans frame"));
        }
        let mut frames = Vec::with_capacity(animation.frames.len());
        for frame in &animation.frames {
            if !frame.atlas_rect.fits(atlas.width, atlas.height) {
                return Err(format!(
                    "frame {} de « {nom} » hors de l'atlas {}×{}",
                    frame.index, atlas.width, atlas.height
                ));
            }
            frames.push(FramePet {
                x: frame.atlas_rect.x,
                y: frame.atlas_rect.y,
                ms: frame.duration_ms,
                direction: frame.direction.clone(),
            });
        }
        // Le package distingue trois natures, et les confondre casse le client : une pose fixe
        // n'a pas de durée, une pose directionnelle n'a pas de suite. Les mettre toutes dans
        // `animations` obligeait le navigateur à deviner, sur un champ absent, s'il devait
        // avancer une horloge ou attendre un angle.
        match animation.kind.as_str() {
            "directional-pose-set" => {
                // Le pas se déduit du nombre de poses plutôt que d'être recopié : seize poses
                // couvrent 360°, et une pose retirée du package changerait le pas sans que la
                // valeur écrite ici s'en aperçoive.
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "seize poses : la conversion est exacte"
                )]
                let pas_degres = 360.0 / frames.len() as f64;
                regard = Some(RegardPet {
                    pas_degres,
                    poses: frames,
                });
            }
            "still" => {
                let Some(frame) = frames.into_iter().next() else {
                    return Err(format!("pose « {nom} » sans frame"));
                };
                poses.insert(nom.clone(), frame);
            }
            _ => {
                let duree = animation
                    .total_duration_ms
                    .unwrap_or_else(|| frames.iter().filter_map(|f| f.ms).sum::<u32>());
                if duree == 0 {
                    return Err(format!("animation « {nom} » de durée nulle"));
                }
                animations.insert(
                    nom.clone(),
                    AnimationPet {
                        duree_ms: duree,
                        role: animation.purpose.clone(),
                        frames,
                    },
                );
            }
        }
    }
    if animations.is_empty() {
        return Err("aucune animation séquencée dans le package".to_owned());
    }

    Ok(ManifestePet {
        pet: IdentitePet {
            id: manifeste.pet.id.clone(),
            nom: manifeste.pet.display_name.clone(),
            version: manifeste.pet.sprite_version_number,
        },
        atlas: AtlasPet {
            url: URL_ATLAS,
            largeur: atlas.width,
            hauteur: atlas.height,
            cellule_l: atlas.cell_width,
            cellule_h: atlas.cell_height,
        },
        animations,
        poses,
        regard,
    })
}

/// Réponse JSON avec sa durée de cache, ou l'erreur de package en clair.
///
/// Un package incohérent est un défaut de build, pas une panne passagère : la réponse le dit au
/// lieu de servir un corps tronqué que le client afficherait à moitié.
fn json(prepare: &'static Result<String, String>, max_age: u32) -> Response {
    match prepare {
        Ok(corps) => (
            [
                (header::CONTENT_TYPE, "application/json; charset=utf-8"),
                (
                    header::CACHE_CONTROL,
                    Box::leak(format!("public, max-age={max_age}").into_boxed_str()) as &str,
                ),
            ],
            corps.clone(),
        )
            .into_response(),
        Err(e) => erreur_package(e),
    }
}

/// La réponse d'un package invalide : `500`, et la raison.
fn erreur_package(raison: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!("package Aphrody invalide : {raison}\n"),
    )
        .into_response()
}

/// `GET /pet/aphrody.json` — le manifeste réduit : animations, poses, regard.
pub async fn manifeste() -> Response {
    json(
        MANIFESTE.get_or_init(|| {
            construire().and_then(|m| serde_json::to_string(&m).map_err(|e| e.to_string()))
        }),
        300,
    )
}

/// `GET /pet/atlas.webp` — l'atlas, tel qu'il est embarqué.
///
/// Le WebP du package est **sans perte** (VP8L) et son RGBA décodé porte le même condensé que
/// le PNG : servir cette variante économise 480 Ko sans changer un pixel.
pub async fn atlas() -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/webp"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        BUNDLED_ATLAS_WEBP,
    )
        .into_response()
}

/// `GET /pet/frame/{animation}/{fichier}` — une frame, extraite de l'atlas et encodée en PNG.
///
/// Les 74 frames sont adressables une par une. Elles ne sont pas stockées : `Pet::extract`
/// découpe la cellule dans le RGBA déjà décodé, sans rééchantillonnage, et `encoder_png` la
/// réencode — ce qui garantit que ce qui sort d'ici est ce que le manifeste décrit, et non une
/// copie qu'on aurait oublié de regénérer.
pub async fn frame(Path((animation, fichier)): Path<(String, String)>) -> Response {
    let Some(index) = fichier.strip_suffix(".png").and_then(|n| n.parse::<usize>().ok()) else {
        return (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "frame introuvable : la forme attendue est `<n>.png`\n",
        )
            .into_response();
    };
    let pet = match pet() {
        Ok(p) => p,
        Err(e) => return erreur_package(e),
    };
    let Some(frame) = pet
        .animation(&animation)
        .and_then(|a| a.frames.iter().find(|f| f.index == index))
    else {
        return (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("frame {index} de « {animation} » introuvable\n"),
        )
            .into_response();
    };
    let rgba = match pet.extract(frame) {
        Ok(r) => r,
        Err(e) => return erreur_package(&e.to_string()),
    };
    match assets::encoder_png(
        &rgba,
        frame.atlas_rect.width,
        frame.atlas_rect.height,
    ) {
        Ok(png) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "public, max-age=86400"),
            ],
            png,
        )
            .into_response(),
        Err(e) => erreur_package(&e.to_string()),
    }
}

/// `GET /pet/aphrody.svg` — le décalque vectoriel de la pose de repos.
///
/// C'est un **décalque**, et le module le dit : suivre le bord d'un masque puis simplifier ne
/// produit pas un dessin conçu comme vectoriel. Il tient pour une silhouette, une favicon ou un
/// filigrane — pas pour remplacer le sprite, qui reste la vérité du package.
pub async fn svg() -> Response {
    let prepare = SVG.get_or_init(|| {
        let pet = pet()?;
        let frame = pet
            .animation(POSE_PORTRAIT)
            .and_then(|a| a.frames.first())
            .ok_or("pose de repos absente du package")?;
        let rgba = pet.extract(frame).map_err(|e| e.to_string())?;
        let image = pixel::Image::nouvelle(
            frame.atlas_rect.width,
            frame.atlas_rect.height,
            rgba,
        )
        .map_err(|e| e.to_string())?;
        pixel::vectoriser(&image, pixel::ReglagesVecteur::default()).map_err(|e| e.to_string())
    });
    match prepare {
        Ok(corps) => (
            [
                (header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"),
                (header::CACHE_CONTROL, "public, max-age=86400"),
            ],
            corps.clone(),
        )
            .into_response(),
        Err(e) => erreur_package(e),
    }
}

/// `GET /api/v1/aphrody` — le dossier du personnage, tel que la crate l'embarque.
///
/// Identité trilingue, trois séries du jeu, statistiques, techniques, auras, variantes, assets
/// VFS et sources. Il est servi verbatim : le réduire ici en ferait une seconde vérité, et
/// c'est précisément ce que le dossier existe pour éviter.
pub async fn dossier() -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        BUNDLED_DOSSIER_JSON,
    )
        .into_response()
}

/// Ce que le serveur peut affirmer du package, mesuré et non déclaré.
#[derive(Debug, Serialize)]
pub struct Diagnostic {
    /// Frames comparées à leur condensé.
    pub frames_verifiees: usize,
    /// Vrai quand chaque frame vérifiée correspond à son condensé.
    pub integrite: bool,
    /// Les écarts, vides quand `integrite` vaut vrai.
    pub erreurs: Vec<String>,
    /// La version du format Codex Pet à laquelle le package se conforme, `null` si aucune.
    pub codex_version: Option<&'static str>,
    /// Vrai quand la conformité au format ne relève aucun écart.
    pub codex_ok: bool,
    /// Les écarts de conformité relevés.
    pub codex_ecarts: Vec<String>,
}

/// `GET /api/v1/aphrody/diagnostic` — intégrité raster et conformité au format Codex.
///
/// `Pet::diagnose` recalcule le condensé RGBA de chaque frame et le compare au manifeste :
/// c'est ce qui distingue « le package est là » de « le package est celui qu'on croit ».
pub async fn diagnostic() -> Response {
    json(
        DIAGNOSTIC.get_or_init(|| {
            let pet = pet()?;
            let rapport = pet.diagnose();
            let conformite = codex::conformite(&pet.pet, &pet.manifest);
            serde_json::to_string(&Diagnostic {
                frames_verifiees: rapport.checked_frames,
                integrite: rapport.ok(),
                erreurs: rapport.errors.clone(),
                // `format!("{:?}")` sur l'`Option` publiait « Some(V2) » : le nom Rust de la
                // variante, entouré de son conteneur, dans un JSON destiné à être lu.
                codex_version: match conformite.version {
                    Some(codex::Version::V1) => Some("v1"),
                    Some(codex::Version::V2) => Some("v2"),
                    None => None,
                },
                codex_ok: conformite.ok(),
                codex_ecarts: conformite.ecarts.clone(),
            })
            .map_err(|e| e.to_string())
        }),
        300,
    )
}

/// La palette mesurée et ses jetons CSS.
#[derive(Debug, Serialize)]
pub struct Palette {
    /// La mesure brute : couleurs dominantes, bornes, statistiques.
    pub mesure: pixel::Mesure,
    /// Les mêmes couleurs, prêtes à être posées dans une feuille de style.
    pub css: String,
}

/// `GET /api/v1/aphrody/palette` — les couleurs du personnage, mesurées en Oklab.
///
/// Le k-means travaille en Oklab et non en sRGB : deux couleurs à distance égale en sRGB ne
/// sont pas également différentes à l'œil, et une palette calculée en sRGB fusionne les tons
/// sombres tout en éclatant les clairs. C'est la source légitime d'une teinte écrite dans du
/// code — le reste vient du souvenir.
pub async fn palette() -> Response {
    json(
        PALETTE.get_or_init(|| {
            let pet = pet()?;
            let frame = pet
                .animation(POSE_PORTRAIT)
                .and_then(|a| a.frames.first())
                .ok_or("pose de repos absente du package")?;
            let rgba = pet.extract(frame).map_err(|e| e.to_string())?;
            let image =
                pixel::Image::nouvelle(frame.atlas_rect.width, frame.atlas_rect.height, rgba)
                    .map_err(|e| e.to_string())?;
            let mesure =
                pixel::mesurer(&image, pixel::Reglages::default()).map_err(|e| e.to_string())?;
            let css = pixel::tokens_css(&mesure, "aphrody");
            serde_json::to_string(&Palette { mesure, css }).map_err(|e| e.to_string())
        }),
        300,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_package_embarque_est_coherent() {
        let m = construire().expect("package Aphrody valide");
        // Neuf animations séquencées, une pose fixe, seize directions : onze entrées en tout.
        assert_eq!(m.animations.len(), 9, "neuf animations séquencées");
        assert_eq!(m.poses.len(), 1, "une pose fixe");
        assert!(m.animations.contains_key("idle"));
        assert!(m.animations.contains_key("waving"));
        assert!(m.animations.contains_key("failed"));
        assert!(m.poses.contains_key(POSE_PORTRAIT));
        let regard = m.regard.as_ref().expect("poses de regard");
        assert_eq!(regard.poses.len(), 16);
        assert!((regard.pas_degres - 22.5).abs() < 1e-9);
        // Chaque pose porte son nom de direction : c'est ce qui permet de vérifier que l'ordre
        // n'a pas été perdu au passage.
        assert_eq!(regard.poses[0].direction.as_deref(), Some("up"));
        assert_eq!(m.atlas.largeur, 1536);
        assert_eq!(m.atlas.cellule_l, 192);
    }

    #[test]
    fn chaque_frame_sequencee_porte_une_duree() {
        let m = construire().expect("package Aphrody valide");
        for (nom, animation) in &m.animations {
            assert!(animation.duree_ms > 0, "« {nom} » sans durée");
            for frame in &animation.frames {
                assert!(frame.ms.is_some(), "frame sans durée dans « {nom} »");
            }
        }
    }

    #[test]
    fn l_atlas_embarque_est_un_webp() {
        // `RIFF....WEBP` : sans cet en-tête, le navigateur refuse l'image sans rien dire de
        // plus qu'une case vide.
        assert_eq!(&BUNDLED_ATLAS_WEBP[0..4], b"RIFF");
        assert_eq!(&BUNDLED_ATLAS_WEBP[8..12], b"WEBP");
    }

    #[test]
    fn le_dossier_embarque_porte_l_identite() {
        let v: serde_json::Value =
            serde_json::from_str(BUNDLED_DOSSIER_JSON).expect("dossier Aphrody valide");
        assert!(v.get("identite").is_some(), "le dossier porte une identité");
        assert!(v.get("techniques").is_some());
        assert!(v.get("variantes").is_some());
    }
}
