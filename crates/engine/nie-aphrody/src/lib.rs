//! Aphrody — la source de vérité du dépôt sur ce personnage, et le runtime 2D de son pet.
//!
//! Cette crate porte deux choses, et c'est délibéré : le **paquet du pet** (atlas RGBA,
//! animations, directions) et le **dossier documentaire** qui rassemble tout ce que le dépôt
//! sait d'Aphrody. Quiconque a besoin d'une information sur Aphrody la lit ici, plutôt que de
//! rejouer un export ou d'interroger un gisement — les deux jeux d'octets sont embarqués par
//! `include_str!`, donc disponibles sans fichier, sans base et sans réseau.
//!
//! Le pet est une feuille RGBA 8×11 (cellules 192×208) décrite par JSON. Il ne s'agit pas d'un
//! modèle Level-5 : G4MD décrit un modèle 3D, G4MG sa géométrie et `assemble` produit un GLB ;
//! cette crate reste volontairement sur le contrat atlas/raster, tout en exposant des rectangles
//! utilisables par les surfaces 2D de `nie-formats`.

pub mod assets;
pub mod codex;
/// Le design system du site : les couleurs de l'interface **dérivées** de la palette mesurée
/// d'Aphrody, et la feuille `game-tokens.css` qu'elles produisent.
pub mod design;
pub mod gisement;
/// Le contrat « pet » de Codex : manifeste, pistes minutées, états. Dérivé d'`openai/codex`
/// (Apache-2.0) — voir `NOTICE`.
pub mod pets;
/// Mesure, comparaison et rastérisation — le socle de la skill `pixel-perfect`.
pub mod pixel;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

/// `pet.json` du package Aphrody validé et embarqué.
pub const BUNDLED_PET_JSON: &str = include_str!("../assets/aphrody/pet.json");
/// `animations.json` exhaustif du package Aphrody validé.
pub const BUNDLED_ANIMATIONS_JSON: &str = include_str!("../assets/aphrody/animations.json");
/// Atlas PNG canonique, lossless, du package Aphrody validé.
pub const BUNDLED_ATLAS_PNG: &[u8] = include_bytes!("../assets/aphrody/sprites/spritesheet.png");
/// Atlas WebP VP8L utilisé par le runtime Codex.
pub const BUNDLED_ATLAS_WEBP: &[u8] = include_bytes!("../assets/aphrody/sprites/spritesheet.webp");

/// Dossier complet d'Aphrody, en JSON.
///
/// Produit par `scripts/aphrody/dossier.ts`, qui croise le dossier Rust
/// (`export_aphrody`, données du jeu), le zukan officiel de LEVEL-5, le VFS, la couverture des
/// wikis et le paquet du pet. Chaque bloc y porte sa source et sa confiance.
pub const BUNDLED_DOSSIER_JSON: &str = include_str!("../assets/dossier/aphrody.json");
/// Le même dossier, en Markdown lisible — la forme qu'on relit et qu'on cite.
pub const BUNDLED_DOSSIER_MD: &str = include_str!("../assets/dossier/aphrody.md");

/// Taille d'une cellule de l'atlas v2.
pub const CELL_WIDTH: u32 = 192;
/// Hauteur d'une cellule de l'atlas v2.
pub const CELL_HEIGHT: u32 = 208;
/// Nombre de colonnes de l'atlas v2.
pub const ATLAS_COLUMNS: u32 = 8;
/// Nombre de lignes de l'atlas v2.
pub const ATLAS_ROWS: u32 = 11;

/// Rectangle entier, avec bord droit et bas exclusifs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Rectangle d'une cellule de la grille.
    #[must_use]
    pub const fn cell(row: u32, column: u32) -> Self {
        Self {
            x: column * CELL_WIDTH,
            y: row * CELL_HEIGHT,
            width: CELL_WIDTH,
            height: CELL_HEIGHT,
        }
    }
    /// Vérifie que le rectangle tient dans les dimensions indiquées.
    #[must_use]
    pub const fn fits(self, width: u32, height: u32) -> bool {
        self.x <= width
            && self.y <= height
            && self.width <= width - self.x
            && self.height <= height - self.y
    }
}

/// Bornes alpha d'une frame, dans la cellule ou l'atlas selon le champ JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct AlphaBounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Une frame telle que décrite dans `animations.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Frame {
    pub index: usize,
    pub image: String,
    pub row: u32,
    pub column: u32,
    #[serde(rename = "atlasRect")]
    pub atlas_rect: Rect,
    #[serde(rename = "alphaBoundsInCell")]
    pub alpha_bounds_in_cell: AlphaBounds,
    #[serde(rename = "alphaBoundsInAtlas")]
    pub alpha_bounds_in_atlas: AlphaBounds,
    #[serde(rename = "baselineYInCell")]
    pub baseline_y_in_cell: u32,
    #[serde(rename = "nonTransparentPixels")]
    pub non_transparent_pixels: u32,
    #[serde(rename = "opaquePixels")]
    pub opaque_pixels: u32,
    #[serde(rename = "translucentPixels")]
    pub translucent_pixels: u32,
    #[serde(rename = "transparentPixels")]
    pub transparent_pixels: u32,
    #[serde(rename = "rgbaSha256")]
    pub rgba_sha256: String,
    #[serde(rename = "pngSha256")]
    pub png_sha256: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<u32>,
    #[serde(rename = "directionDegrees")]
    pub direction_degrees: Option<String>,
    pub direction: Option<String>,
}

/// Animation séquencée ou ligne directionnelle.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Animation {
    pub kind: String,
    pub row: Option<u32>,
    #[serde(rename = "frameCount")]
    pub frame_count: usize,
    #[serde(rename = "totalDurationMs")]
    pub total_duration_ms: Option<u32>,
    pub purpose: Option<String>,
    pub frames: Vec<Frame>,
}

/// Manifest JSON de l'atlas et de ses animations.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AnimationsManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub pet: PetInfo,
    pub atlas: AtlasInfo,
    #[serde(rename = "animationCount")]
    pub animation_count: usize,
    #[serde(rename = "exportedFrameCount")]
    pub exported_frame_count: usize,
    pub animations: BTreeMap<String, Animation>,
}

/// Manifest d'identité optionnel (`pet.json`) du package.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PetManifest {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "spriteVersionNumber")]
    pub sprite_version_number: u32,
    pub description: String,
    #[serde(rename = "spritesheetPath")]
    pub spritesheet_path: String,
}

/// Identité publique du pet.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PetInfo {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "spriteVersionNumber")]
    pub sprite_version_number: u32,
    pub description: String,
}

/// Métadonnées physiques de l'atlas.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AtlasInfo {
    pub image: String,
    #[serde(rename = "runtimeImage")]
    pub runtime_image: String,
    pub format: String,
    #[serde(rename = "sourceMode")]
    pub source_mode: String,
    #[serde(rename = "decodedMode")]
    pub decoded_mode: String,
    pub width: u32,
    pub height: u32,
    pub columns: u32,
    pub rows: u32,
    #[serde(rename = "cellWidth")]
    pub cell_width: u32,
    #[serde(rename = "cellHeight")]
    pub cell_height: u32,
    #[serde(rename = "pngSha256")]
    pub png_sha256: String,
    #[serde(rename = "rgbaSha256")]
    pub rgba_sha256: String,
    #[serde(rename = "webpSha256")]
    pub webp_sha256: String,
    #[serde(rename = "webpRgbaSha256")]
    pub webp_rgba_sha256: String,
}

/// Erreurs de chargement, validation et décodage.
#[derive(Debug)]
pub enum Error {
    Json(serde_json::Error),
    Io(std::io::Error),
    Png(png::DecodingError),
    Invalid(String),
    MissingAsset(PathBuf),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => write!(f, "JSON Aphrody invalide: {e}"),
            Self::Io(e) => write!(f, "lecture Aphrody: {e}"),
            Self::Png(e) => write!(f, "PNG Aphrody invalide: {e}"),
            Self::Invalid(e) => f.write_str(e),
            Self::MissingAsset(p) => write!(f, "asset Aphrody absent: {}", p.display()),
        }
    }
}
impl std::error::Error for Error {}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<png::DecodingError> for Error {
    fn from(e: png::DecodingError) -> Self {
        Self::Png(e)
    }
}

/// Atlas décodé en RGBA8 et manifest associé.
#[derive(Debug, Clone)]
pub struct Pet {
    /// Identité issue de `pet.json` (recopiée dans le manifest d'animations).
    pub pet: PetManifest,
    pub manifest: AnimationsManifest,
    pub rgba: Vec<u8>,
}

/// Résultat vérifiable d'un diagnostic raster du package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityReport {
    /// Nombre de frames comparées au manifest.
    pub checked_frames: usize,
    /// Erreurs détaillées, vides quand `ok` est vrai.
    pub errors: Vec<String>,
}

impl IntegrityReport {
    /// Vrai si toutes les frames comparées sont cohérentes.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

impl AnimationsManifest {
    /// Parse et valide `animations.json`.
    pub fn from_json(json: &str) -> Result<Self, Error> {
        let value: Self = serde_json::from_str(json)?;
        value.validate()?;
        Ok(value)
    }
    /// Vérifie les invariants de grille, ordres, durées et comptes.
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema_version != 1 || self.pet.sprite_version_number != 2 {
            return Err(Error::Invalid(
                "version de manifest Aphrody non supportée".into(),
            ));
        }
        if (
            self.atlas.width,
            self.atlas.height,
            self.atlas.columns,
            self.atlas.rows,
            self.atlas.cell_width,
            self.atlas.cell_height,
        ) != (1536, 2288, 8, 11, 192, 208)
        {
            return Err(Error::Invalid(
                "géométrie d'atlas Aphrody inattendue".into(),
            ));
        }
        let mut frames = 0;
        for (name, animation) in &self.animations {
            if animation.frame_count != animation.frames.len() {
                return Err(Error::Invalid(format!("{name}: frameCount incohérent")));
            }
            let mut duration = 0;
            for (i, frame) in animation.frames.iter().enumerate() {
                if frame.index != i
                    || frame.atlas_rect != Rect::cell(frame.row, frame.column)
                    || !frame.atlas_rect.fits(self.atlas.width, self.atlas.height)
                {
                    return Err(Error::Invalid(format!(
                        "{name}: ordre ou rectangle incohérent"
                    )));
                }
                if let Some(d) = frame.duration_ms {
                    if d == 0 {
                        return Err(Error::Invalid(format!("{name}: durée nulle")));
                    }
                    duration += d;
                }
            }
            if let Some(total) = animation.total_duration_ms
                && duration != total
            {
                return Err(Error::Invalid(format!(
                    "{name}: totalDurationMs incohérent"
                )));
            }
            frames += animation.frames.len();
        }
        if frames != self.exported_frame_count || self.animations.len() != self.animation_count {
            return Err(Error::Invalid(
                "comptes d'animations ou de frames incohérents".into(),
            ));
        }
        Ok(())
    }
}

impl Pet {
    /// Charge le package validé embarqué dans la crate.
    pub fn bundled() -> Result<Self, Error> {
        Self::from_package_bytes(
            BUNDLED_PET_JSON,
            BUNDLED_ANIMATIONS_JSON,
            BUNDLED_ATLAS_PNG,
            BUNDLED_ATLAS_WEBP,
        )
    }

    /// Charge un package complet depuis ses octets canoniques.
    pub fn from_package_bytes(
        pet_json: &str,
        animations_json: &str,
        png_bytes: &[u8],
        webp_bytes: &[u8],
    ) -> Result<Self, Error> {
        let pet: PetManifest = serde_json::from_str(pet_json)?;
        let manifest = AnimationsManifest::from_json(animations_json)?;
        validate_identity(&pet, &manifest)?;
        if sha256_hex(webp_bytes) != manifest.atlas.webp_sha256 {
            return Err(Error::Invalid(
                "hash WebP runtime différent du manifest".into(),
            ));
        }
        let mut loaded = Self::from_png(manifest, png_bytes)?;
        loaded.pet = pet;
        Ok(loaded)
    }

    /// Charge un pet depuis un dossier de package contenant `animations.json` et `sprites/`.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, Error> {
        let root = root.as_ref();
        let pet_path = root.join("pet.json");
        let pet: PetManifest = serde_json::from_str(&std::fs::read_to_string(&pet_path)?)?;
        let manifest =
            AnimationsManifest::from_json(&std::fs::read_to_string(root.join("animations.json"))?)?;
        let path = root.join(&manifest.atlas.image);
        let bytes = std::fs::read(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::MissingAsset(path.clone())
            } else {
                Error::Io(e)
            }
        })?;
        let webp_path = root.join(&manifest.atlas.runtime_image);
        let webp = std::fs::read(&webp_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::MissingAsset(webp_path.clone())
            } else {
                Error::Io(e)
            }
        })?;
        validate_identity(&pet, &manifest)?;
        if sha256_hex(&webp) != manifest.atlas.webp_sha256 {
            return Err(Error::Invalid(
                "hash WebP runtime différent du manifest".into(),
            ));
        }
        let mut loaded = Self::from_png(manifest, &bytes)?;
        loaded.pet = pet;
        Ok(loaded)
    }
    /// Construit le runtime à partir du manifest et de PNG RGBA décodé lossless.
    pub fn from_png(manifest: AnimationsManifest, png_bytes: &[u8]) -> Result<Self, Error> {
        if sha256_hex(png_bytes) != manifest.atlas.png_sha256 {
            return Err(Error::Invalid(
                "hash PNG atlas différent du manifest".into(),
            ));
        }
        let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
        let mut reader = decoder.read_info()?;
        let mut buf = vec![
            0;
            reader.output_buffer_size().ok_or_else(|| {
                Error::Invalid("taille de sortie PNG indisponible".into())
            })?
        ];
        let info = reader.next_frame(&mut buf)?;
        if info.width != manifest.atlas.width || info.height != manifest.atlas.height {
            return Err(Error::Invalid(
                "dimensions PNG différentes du manifest".into(),
            ));
        }
        let rgba = match info.color_type {
            png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
            png::ColorType::Rgb => buf[..info.buffer_size()]
                .chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
            _ => return Err(Error::Invalid("l'atlas doit être RGB ou RGBA".into())),
        };
        if sha256_hex(&rgba) != manifest.atlas.rgba_sha256 {
            return Err(Error::Invalid(
                "hash RGBA atlas différent du manifest".into(),
            ));
        }
        let pet = PetManifest {
            id: manifest.pet.id.clone(),
            display_name: manifest.pet.display_name.clone(),
            sprite_version_number: manifest.pet.sprite_version_number,
            description: manifest.pet.description.clone(),
            spritesheet_path: manifest.atlas.runtime_image.clone(),
        };
        Ok(Self {
            pet,
            manifest,
            rgba,
        })
    }
    /// Extrait une cellule sans rééchantillonnage ni conversion de couleur.
    pub fn extract(&self, frame: &Frame) -> Result<Vec<u8>, Error> {
        if frame.atlas_rect != Rect::cell(frame.row, frame.column) {
            return Err(Error::Invalid("rectangle de frame non canonique".into()));
        }
        crop_rgba(
            &self.rgba,
            self.manifest.atlas.width,
            self.manifest.atlas.height,
            frame.atlas_rect,
        )
        .ok_or_else(|| Error::Invalid("rectangle hors atlas".into()))
    }
    /// Compare chaque cellule au hash, aux comptes alpha et aux bornes déclarés dans JSON.
    #[must_use]
    pub fn diagnose(&self) -> IntegrityReport {
        let mut report = IntegrityReport {
            checked_frames: 0,
            errors: Vec::new(),
        };
        for animation in self.manifest.animations.values() {
            for frame in &animation.frames {
                report.checked_frames += 1;
                let Ok(cell) = self.extract(frame) else {
                    report
                        .errors
                        .push(format!("frame {}: rectangle invalide", frame.image));
                    continue;
                };
                if sha256_hex(&cell) != frame.rgba_sha256 {
                    report
                        .errors
                        .push(format!("frame {}: hash RGBA", frame.image));
                }
                let mut non_transparent = 0u32;
                let mut opaque = 0u32;
                let mut min_x = CELL_WIDTH;
                let mut min_y = CELL_HEIGHT;
                let mut max_x = 0;
                let mut max_y = 0;
                for (i, px) in cell.chunks_exact(4).enumerate() {
                    if px[3] != 0 {
                        non_transparent += 1;
                        let x = i as u32 % CELL_WIDTH;
                        let y = i as u32 / CELL_WIDTH;
                        min_x = min_x.min(x);
                        min_y = min_y.min(y);
                        max_x = max_x.max(x + 1);
                        max_y = max_y.max(y + 1);
                        if px[3] == 255 {
                            opaque += 1;
                        }
                    }
                }
                let translucent = non_transparent - opaque;
                if non_transparent != frame.non_transparent_pixels
                    || opaque != frame.opaque_pixels
                    || translucent != frame.translucent_pixels
                    || (CELL_WIDTH * CELL_HEIGHT - non_transparent) != frame.transparent_pixels
                {
                    report
                        .errors
                        .push(format!("frame {}: comptes alpha", frame.image));
                }
                let actual = AlphaBounds {
                    x: if non_transparent == 0 { 0 } else { min_x },
                    y: if non_transparent == 0 { 0 } else { min_y },
                    width: max_x.saturating_sub(min_x),
                    height: max_y.saturating_sub(min_y),
                };
                if actual != frame.alpha_bounds_in_cell {
                    report
                        .errors
                        .push(format!("frame {}: bornes alpha", frame.image));
                }
            }
        }
        report
    }
    /// Cherche une animation par nom.
    #[must_use]
    pub fn animation(&self, name: &str) -> Option<&Animation> {
        self.manifest.animations.get(name)
    }
    /// Sélectionne la direction nominale la plus proche parmi les frames directionnelles.
    #[must_use]
    pub fn direction(&self, degrees: f32) -> Option<&Frame> {
        self.animation("look-directions")?
            .frames
            .iter()
            .min_by_key(|f| {
                angular_distance(
                    degrees,
                    f.direction_degrees
                        .as_deref()
                        .and_then(|x| x.parse::<f32>().ok())
                        .unwrap_or(0.0),
                ) as u32
            })
    }
}

/// Le dossier documentaire d'Aphrody.
///
/// Les champs stables sont typés ; le reste est laissé en [`serde_json::Value`] à dessein. Le
/// dossier gagne des blocs au fil des sources qu'on lui branche, et figer sa forme complète
/// obligerait à modifier cette crate à chaque ajout — ce qui la rendrait plus fragile que la
/// donnée qu'elle décrit.
#[derive(Debug, Clone, Deserialize)]
pub struct Dossier {
    /// Identifiant stable du concept, tel que publié (`byron-love-aphrody`).
    pub slug: String,
    /// Horodatage ISO-8601 de la génération.
    pub genere_le: String,
    /// Noms, surnoms, lecture kana, romaji, élément, poste, équipes…
    pub identite: serde_json::Value,
    /// Codes internes du jeu couverts par ce dossier (un par ère du personnage).
    pub codes_internes: Vec<String>,
    /// Tous les blocs, y compris ceux qui ne sont pas typés ci-dessus.
    #[serde(flatten)]
    pub reste: BTreeMap<String, serde_json::Value>,
}

impl Dossier {
    /// Charge le dossier embarqué.
    ///
    /// # Errors
    /// Rend [`Error::Json`] si le JSON embarqué n'est pas conforme — ce qui ne peut arriver
    /// qu'après une modification manuelle du fichier, le test d'intégrité le vérifiant.
    pub fn bundled() -> Result<Self, Error> {
        Ok(serde_json::from_str(BUNDLED_DOSSIER_JSON)?)
    }

    /// Un bloc du dossier par son nom (`statistiques`, `techniques`, `jeu`, `pet`…).
    #[must_use]
    pub fn bloc(&self, nom: &str) -> Option<&serde_json::Value> {
        match nom {
            "identite" => Some(&self.identite),
            _ => self.reste.get(nom),
        }
    }

    /// Les noms de tous les blocs présents, triés.
    #[must_use]
    pub fn blocs(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.reste.keys().map(String::as_str).collect();
        v.push("identite");
        v.sort_unstable();
        v
    }

    /// Une valeur d'identité par son nom (`nom_fr`, `romaji`, `furigana`, `element`…).
    #[must_use]
    pub fn identite_str(&self, champ: &str) -> Option<&str> {
        self.identite.get(champ).and_then(serde_json::Value::as_str)
    }
}

/// Copie RGBA d'un rectangle entier.
pub fn crop_rgba(src: &[u8], width: u32, height: u32, rect: Rect) -> Option<Vec<u8>> {
    if rect.fits(width, height) && src.len() == width as usize * height as usize * 4 {
        let stride = width as usize * 4;
        let row = rect.width as usize * 4;
        let mut out = Vec::with_capacity(rect.width as usize * rect.height as usize * 4);
        for y in rect.y..rect.y + rect.height {
            let start = y as usize * stride + rect.x as usize * 4;
            out.extend_from_slice(&src[start..start + row]);
        }
        Some(out)
    } else {
        None
    }
}
/// Calcule un SHA-256 hexadécimal minuscule.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}
fn validate_identity(pet: &PetManifest, manifest: &AnimationsManifest) -> Result<(), Error> {
    if pet.id != manifest.pet.id
        || pet.display_name != manifest.pet.display_name
        || pet.sprite_version_number != manifest.pet.sprite_version_number
        || pet.spritesheet_path != "spritesheet.webp"
    {
        return Err(Error::Invalid(
            "pet.json et animations.json ne décrivent pas le même pet".into(),
        ));
    }
    Ok(())
}
fn angular_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grille_et_crop_sont_lossless() {
        let mut pixels = vec![0; 4 * 4 * 4];
        pixels[0..4].copy_from_slice(&[1, 2, 3, 4]);
        let r = Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        assert_eq!(crop_rgba(&pixels, 4, 4, r), Some(vec![1, 2, 3, 4]));
        assert_eq!(
            Rect::cell(10, 7),
            Rect {
                x: 1344,
                y: 2080,
                width: 192,
                height: 208
            }
        );
    }
    #[test]
    fn direction_circulaire() {
        assert!((angular_distance(359.0, 0.0) - 1.0).abs() < f32::EPSILON);
        assert!((angular_distance(-1.0, 0.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn hash_sha256_est_stable() {
        assert_eq!(
            sha256_hex(b"Aphrody"),
            "acc244d702c23f26841eb398a3906f1ed16130ad9104fa25fe7416ffa88ddf5a"
        );
    }
}
