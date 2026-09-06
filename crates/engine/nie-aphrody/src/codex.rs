//! Conformité au format **Codex Pet**, et production des artefacts d'installation.
//!
//! Le paquet d'Aphrody n'est pas un format maison : il suit la spécification publique des
//! Codex pets, telle que publiée par la galerie communautaire `legeling/awesome-codex-pet`
//! (code MIT). Vérifié le 2026-09-05 — notre paquet la respecte déjà à la lettre :
//!
//! | Version | Atlas | Colonnes × lignes | `spriteVersionNumber` |
//! |---|---|---|---|
//! | v1 | 1536 × 1872 | 8 × 9  | absent ou `1` |
//! | v2 | 1536 × 2288 | 8 × 11 | `2` — plus 16 directions de regard |
//!
//! Aphrody est un v2 : atlas 1536 × 2288, 8 × 11 cellules de 192 × 208, et son animation
//! `look-directions` porte exactement 16 poses.
//!
//! Ce module n'emprunte aucun code à la galerie : il implémente le format, ce qui est
//! justement ce qu'un format sert à permettre. Les assets de la galerie sont sous
//! CC BY-NC 4.0 et ne sont ni copiés ni redistribués ici.

use crate::{AnimationsManifest, Error, PetManifest, sha256_hex};
use serde::Serialize;

/// Dimensions attendues d'un atlas, par version de la spécification.
const V1: (u32, u32, u32, u32) = (1536, 1872, 8, 9);
/// Idem pour la v2, qui ajoute les seize directions de regard.
const V2: (u32, u32, u32, u32) = (1536, 2288, 8, 11);
/// Nombre de poses attendues dans `look-directions` pour un pet v2.
const DIRECTIONS_V2: usize = 16;

/// Version de la spécification à laquelle un paquet se conforme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Version {
    /// 8 × 9, sans directions de regard.
    V1,
    /// 8 × 11, avec seize directions de regard.
    V2,
}

/// Verdict de conformité, avec le détail de ce qui cloche le cas échéant.
#[derive(Debug, Clone, Serialize)]
pub struct Conformite {
    /// Version déduite des dimensions de l'atlas.
    pub version: Option<Version>,
    /// Écarts constatés. Vide ⇒ le paquet est installable tel quel.
    pub ecarts: Vec<String>,
}

impl Conformite {
    /// Le paquet est-il conforme ?
    #[must_use]
    pub fn ok(&self) -> bool {
        self.version.is_some() && self.ecarts.is_empty()
    }
}

/// Entrée d'`install-manifest.json` décrivant un pet installable.
///
/// C'est ce que la galerie publie pour permettre l'installation sans cloner : chaque fichier
/// y est décrit par sa taille **et** son empreinte, de sorte qu'un téléchargement tronqué ou
/// substitué se voie avant d'être écrit sur le disque de quelqu'un.
#[derive(Debug, Clone, Serialize)]
pub struct EntreeInstallation {
    /// Nom affiché du pet.
    pub name: String,
    /// `2` pour la v2 ; omis ou `1` pour la v1.
    #[serde(rename = "spriteVersionNumber")]
    pub sprite_version_number: u32,
    /// SHA-256 de `pet.json`.
    #[serde(rename = "petJsonSha256")]
    pub pet_json_sha256: String,
    /// Taille de `pet.json`, en octets.
    #[serde(rename = "petJsonBytes")]
    pub pet_json_bytes: usize,
    /// SHA-256 de la feuille de sprites servie au runtime (le WebP).
    #[serde(rename = "spritesheetSha256")]
    pub spritesheet_sha256: String,
    /// Taille de cette feuille, en octets.
    #[serde(rename = "spritesheetBytes")]
    pub spritesheet_bytes: usize,
    /// Largeur de l'atlas.
    #[serde(rename = "spritesheetWidth")]
    pub spritesheet_width: u32,
    /// Hauteur de l'atlas.
    #[serde(rename = "spritesheetHeight")]
    pub spritesheet_height: u32,
}

/// Vérifie qu'un paquet respecte la spécification Codex Pet.
#[must_use]
pub fn conformite(pet: &PetManifest, manifest: &AnimationsManifest) -> Conformite {
    let (l, h, c, r) = (
        manifest.atlas.width,
        manifest.atlas.height,
        manifest.atlas.columns,
        manifest.atlas.rows,
    );
    let mut ecarts = Vec::new();

    let version = if (l, h, c, r) == V2 {
        Some(Version::V2)
    } else if (l, h, c, r) == V1 {
        Some(Version::V1)
    } else {
        ecarts.push(format!(
            "atlas {l}×{h} en {c}×{r} : ni v1 (1536×1872, 8×9) ni v2 (1536×2288, 8×11)"
        ));
        None
    };

    // La correspondance version ↔ `spriteVersionNumber` est la seule chose qu'un installeur
    // lit avant de décoder l'atlas : si elle ment, il découpe la grille au mauvais pas.
    match (version, pet.sprite_version_number) {
        (Some(Version::V2), n) if n != 2 => {
            ecarts.push(format!(
                "atlas v2 mais spriteVersionNumber = {n} (attendu 2)"
            ));
        }
        (Some(Version::V1), n) if n != 1 => {
            ecarts.push(format!(
                "atlas v1 mais spriteVersionNumber = {n} (attendu 1)"
            ));
        }
        _ => {}
    }

    if version == Some(Version::V2) {
        match manifest.animations.get("look-directions") {
            Some(a) if a.frames.len() != DIRECTIONS_V2 => ecarts.push(format!(
                "look-directions porte {} poses (attendu {DIRECTIONS_V2})",
                a.frames.len()
            )),
            None => ecarts.push("un pet v2 doit porter l'animation look-directions".into()),
            Some(_) => {}
        }
    }

    if pet.spritesheet_path != "spritesheet.webp" {
        ecarts.push(format!(
            "spritesheetPath = {} (l'installeur attend spritesheet.webp)",
            pet.spritesheet_path
        ));
    }

    Conformite { version, ecarts }
}

/// Produit l'entrée d'installation du paquet embarqué.
///
/// # Errors
/// Rend [`Error::Invalid`] si le paquet n'est pas conforme — publier une entrée pour un
/// paquet qu'un installeur refusera n'aide personne.
pub fn entree_installation(
    pet: &PetManifest,
    manifest: &AnimationsManifest,
    pet_json: &[u8],
    spritesheet_webp: &[u8],
) -> Result<EntreeInstallation, Error> {
    let c = conformite(pet, manifest);
    if !c.ok() {
        return Err(Error::Invalid(format!(
            "paquet non conforme au format Codex Pet : {}",
            c.ecarts.join(" ; ")
        )));
    }
    Ok(EntreeInstallation {
        name: pet.display_name.clone(),
        sprite_version_number: pet.sprite_version_number,
        pet_json_sha256: sha256_hex(pet_json),
        pet_json_bytes: pet_json.len(),
        spritesheet_sha256: sha256_hex(spritesheet_webp),
        spritesheet_bytes: spritesheet_webp.len(),
        spritesheet_width: manifest.atlas.width,
        spritesheet_height: manifest.atlas.height,
    })
}
