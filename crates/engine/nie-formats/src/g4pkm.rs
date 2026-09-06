//! Parseur de layout 2D G4PKM (menu-objet Level-5).
//!
//! Port Rust fidèle de `IECODE.Core/Formats/Menu/G4pkmLayout.cs` (588 lignes).
//! Extrait les transforms 2D (position, échelle, rotation) de chaque bone d'un
//! squelette G4SK encapsulé dans un container G4PK de type menu.
//!
//! ## Espace de référence
//!
//! Origine : centre de l'écran (0, 0). X positif → droite, Y positif → haut
//! (convention OpenGL). Unité : pixels 1920×1080 (1 unité = 1 pixel à résolution
//! native). Conversion CSS 1280×720 :
//!   `px = 640 + x * (1280 / 1920)`, `py = 360 - y * (720 / 1080)`.
//!
//! ## Calibration (rétro-ingénierie sur 3 fichiers réels)
//!
//! - `option02_02` / `_license_nvidia_dmy01` : tx=0 ty=0 sx=1920 sy=1080 → plein-écran.
//! - `win00_04` / `_cursor01`               : tx=-40 ty=40 sx=80 sy=80 → centre-gauche.
//! - `title00_09` / `_pos_scl_base01`       : tx=1873 ty=-39 → hors-écran (bind = caché).
//!
//! ## Structure G4PKM
//!
//! Container G4PK (header 0x40, magic `G4PK`) contenant plusieurs sous-fichiers :
//! G4SK (squelette), G4MD (géométrie), G4MA (animation), G4MT (matériau).
//! Ce parseur extrait le G4SK et en décode la hiérarchie de bones avec leurs
//! matrices bind-pose locales (3×4 row-major f32 LE) puis compose les poses monde.
//!
//! Compatible `no_std + alloc`.

extern crate alloc;
use alloc::{collections::BTreeMap, format, string::String, vec, vec::Vec};

use crate::FormatError;

// ── Magics ────────────────────────────────────────────────────────────────────

/// Magic « G4PK » little-endian (`0x4B503447`).
const MAGIC_G4PK: u32 = 0x4B50_3447;
/// Magic « G4SK » little-endian (`0x4B533447`).
const MAGIC_G4SK: u32 = 0x4B53_3447;
/// Magic « G4MD » little-endian (`0x444D3447`) — sous-fichier géométrie + matériaux.
const MAGIC_G4MD: u32 = 0x444D_3447;

/// Taille fixe de l'en-tête G4PK (octets).
const G4PK_HEADER_SIZE: usize = 0x40;
/// Stride d'une matrice 3×4 flottants (3 rangées × 4 floats × 4 octets).
const MATRIX34_STRIDE: usize = 48;

// ── Structures de données ─────────────────────────────────────────────────────

/// Transform 2D d'un bone de menu, exprimé dans l'espace-écran du jeu.
///
/// Espace de référence (confirmé par rétro-ingénierie sur 3 fichiers réels) :
/// - Origine : CENTRE de l'écran (0, 0).
/// - X positif → droite, X négatif → gauche.
/// - Y positif → HAUT (convention OpenGL), Y négatif → bas.
/// - Unité : pixels 1920×1080 (1 unité = 1 pixel à résolution native).
///
/// ## Exemple
///
/// ```
/// # use nie_formats::g4pkm::Transform2D;
/// let (px, py) = Transform2D::ZERO.to_css_1280x720();
/// assert_eq!(px, 640.0);
/// assert_eq!(py, 360.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transform2D {
    /// Position X en unités-écran (centre=0, droite+). Non nul = décalage du pivot.
    pub x: f32,
    /// Position Y en unités-écran (centre=0, haut+).
    pub y: f32,
    /// Largeur du sprite en pixels 1920 (= sx de la matrice locale).
    pub scale_x: f32,
    /// Hauteur du sprite en pixels 1080 (= sy de la matrice locale).
    pub scale_y: f32,
    /// Rotation en radians (sens trigonométrique). 0 pour la quasi-totalité des menus 2D.
    pub rot: f32,
    /// Pivot X normalisé dans le sprite (0=gauche, 0.5=centre, 1=droite). Toujours 0.5.
    pub anchor_x: f32,
    /// Pivot Y normalisé dans le sprite (0=bas, 0.5=centre, 1=haut). Toujours 0.5.
    pub anchor_y: f32,
}

impl Transform2D {
    /// Transform identité (0, 0, 1, 1, 0, 0.5, 0.5).
    pub const ZERO: Transform2D = Transform2D {
        x: 0.0,
        y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        rot: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
    };

    /// Transforme le pivot du bone en coordonnées pixel CSS sur un canvas 1280×720.
    ///
    /// ```
    /// # use nie_formats::g4pkm::Transform2D;
    /// let (px, py) = Transform2D { x: 960.0, ..Transform2D::ZERO }.to_css_1280x720();
    /// assert_eq!(px, 1280.0);
    /// ```
    #[must_use]
    pub fn to_css_1280x720(&self) -> (f32, f32) {
        let px = 640.0_f32 + self.x * (1280.0_f32 / 1920.0_f32);
        let py = 360.0_f32 - self.y * (720.0_f32 / 1080.0_f32);
        (px, py)
    }

    /// Retourne `true` si la position est visiblement hors du canvas 1920×1080
    /// (`|x| > 960` ou `|y| > 540`).
    ///
    /// ```
    /// # use nie_formats::g4pkm::Transform2D;
    /// assert!(Transform2D { x: 1873.0, ..Transform2D::ZERO }.is_off_screen_1920());
    /// assert!(!Transform2D { x: -40.0, y: 40.0, ..Transform2D::ZERO }.is_off_screen_1920());
    /// ```
    #[must_use]
    pub fn is_off_screen_1920(&self) -> bool {
        self.x.abs() > 960.0_f32 || self.y.abs() > 540.0_f32
    }
}

/// Bone parsé depuis un squelette G4SK (issu d'un G4PKM de menu).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4pkmBone {
    /// Index du bone dans le squelette (0-based, ordre du fichier).
    pub index: usize,
    /// Nom du bone (ex. `_pos_scl_base01`, `_cursor01`, `_license_nvidia_dmy01`).
    pub name: String,
    /// Index du parent (-1 = racine).
    pub parent_index: i32,
    /// Transform LOCAL en bind pose (relatif au parent direct).
    pub local_bind_pose: Transform2D,
    /// Transform MONDE calculé en composant la chaîne parentale (bind pose).
    /// Représente la position/taille réelle dans l'espace-écran.
    pub world_bind_pose: Transform2D,
}

/// Layout 2D complet d'un squelette G4PKM de menu.
///
/// Contient la liste des bones avec leurs transforms bind-pose (local + monde).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4pkmLayout {
    /// Bones dans l'ordre du fichier.
    pub bones: Vec<G4pkmBone>,
    /// Map nom → transform monde (bind pose) ; premier bone conservé en cas de doublon.
    pub world_pose_by_name: BTreeMap<String, Transform2D>,
}

impl G4pkmLayout {
    /// Nombre de bones dans le squelette.
    #[must_use]
    pub fn bone_count(&self) -> usize {
        self.bones.len()
    }

    /// Retourne le transform monde d'un bone par son nom, ou `None` si non trouvé.
    #[must_use]
    pub fn world_pose(&self, name: &str) -> Option<Transform2D> {
        self.world_pose_by_name.get(name).copied()
    }
}

// ── API publique ──────────────────────────────────────────────────────────────

/// Parse un fichier `.g4pkm` (container G4PK de type menu) et retourne le layout
/// 2D du squelette inclus.
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si les données sont trop courtes.
/// - [`FormatError::BadMagic`] si le magic G4PK est absent.
/// - [`FormatError::Corrupt`] si la structure interne est invalide ou si aucun G4SK
///   n'est trouvé dans le container.
pub fn parse(g4pkm_data: &[u8]) -> Result<G4pkmLayout, FormatError> {
    validate_g4pk_magic(g4pkm_data)?;
    let g4sk = extract_sub_file(g4pkm_data, MAGIC_G4SK).ok_or(FormatError::Corrupt(
        "g4pkm : aucun sous-fichier G4SK dans le container",
    ))?;
    parse_g4sk(g4sk)
}

/// Parse directement un sous-fichier G4SK (squelette de menu).
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si les données sont trop courtes (< 0x40 octets).
/// - [`FormatError::BadMagic`] si le magic G4SK est absent.
/// - [`FormatError::Corrupt`] si la structure interne est invalide.
pub fn parse_g4sk(g4sk_data: &[u8]) -> Result<G4pkmLayout, FormatError> {
    if g4sk_data.len() < 0x40 {
        return Err(FormatError::TooShort {
            got: g4sk_data.len(),
            need: 0x40,
        });
    }
    if read_u32(g4sk_data, 0)? != MAGIC_G4SK {
        return Err(FormatError::BadMagic { format: "G4SK" });
    }

    let bone_count = read_u16(g4sk_data, 0x20)? as usize;
    if bone_count == 0 {
        return Ok(G4pkmLayout {
            bones: Vec::new(),
            world_pose_by_name: BTreeMap::new(),
        });
    }

    // Offsets des sections : champ u16 @ field_pos → absolu = 0x40 + raw * 4.
    let name_table_abs = section_abs_offset(g4sk_data, 0x32)?;
    let parent_abs = section_abs_offset(g4sk_data, 0x2A)?;

    let names = read_bone_names(g4sk_data, name_table_abs, bone_count);
    let parents = read_parent_indices(g4sk_data, parent_abs, bone_count);
    let local_poses = read_local_bind_poses(g4sk_data, bone_count)?;
    let world_poses = compute_world_poses(&local_poses, &parents);

    let mut bones = Vec::with_capacity(bone_count);
    let mut world_pose_by_name = BTreeMap::new();
    for i in 0..bone_count {
        // Premier bone en cas de doublon de nom (rare).
        world_pose_by_name
            .entry(names[i].clone())
            .or_insert(world_poses[i]);
        bones.push(G4pkmBone {
            index: i,
            name: names[i].clone(),
            parent_index: parents[i],
            local_bind_pose: local_poses[i],
            world_bind_pose: world_poses[i],
        });
    }

    Ok(G4pkmLayout {
        bones,
        world_pose_by_name,
    })
}

/// Extrait le sous-fichier **G4MD** (géométrie + matériaux) d'un container G4PKM, s'il existe.
///
/// Sert à résoudre la texture des objets-menu dont l'objbin ne déclare **pas** de paramètre
/// `Texture` : la texture base-color est alors nommée par le matériau g4md (cf.
/// [`crate::g4md::G4md::material_base_names`]) plutôt que par l'objbin. Retourne `None` si le
/// container est invalide ou n'a pas de bloc G4MD.
#[must_use]
pub fn extract_g4md(g4pkm_data: &[u8]) -> Option<&[u8]> {
    if validate_g4pk_magic(g4pkm_data).is_err() {
        return None;
    }
    extract_sub_file(g4pkm_data, MAGIC_G4MD)
}

// ── Validation et extraction du sous-fichier G4SK ────────────────────────────

fn validate_g4pk_magic(data: &[u8]) -> Result<(), FormatError> {
    if data.len() < G4PK_HEADER_SIZE {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: G4PK_HEADER_SIZE,
        });
    }
    if read_u32(data, 0)? != MAGIC_G4PK {
        return Err(FormatError::BadMagic { format: "G4PK" });
    }
    Ok(())
}

/// Cherche et retourne le slice d'un sous-fichier par magic depuis un container G4PK.
/// Retourne `None` si non trouvé ou si la structure est invalide.
///
/// Table des offsets : `headerSize × i32` (décalés `<< 2` depuis `headerSize`).
/// Table des tailles : `fileCount × i32` immédiatement après.
fn extract_sub_file(pk_data: &[u8], target_magic: u32) -> Option<&[u8]> {
    if pk_data.len() < G4PK_HEADER_SIZE {
        return None;
    }

    let header_size = u16::from_le_bytes(pk_data.get(4..6)?.try_into().ok()?) as usize;
    let file_count_raw = i32::from_le_bytes(pk_data.get(0x20..0x24)?.try_into().ok()?);

    if file_count_raw <= 0 || file_count_raw > 64 {
        return None;
    }
    let fc = file_count_raw as usize;

    let offset_table = header_size;
    let size_table = offset_table.checked_add(fc.checked_mul(4)?)?;

    for i in 0..fc {
        let raw_off = i32::from_le_bytes(
            pk_data
                .get(offset_table + i * 4..offset_table + i * 4 + 4)?
                .try_into()
                .ok()?,
        );
        let sz = i32::from_le_bytes(
            pk_data
                .get(size_table + i * 4..size_table + i * 4 + 4)?
                .try_into()
                .ok()?,
        );

        if raw_off < 0 || sz <= 0 {
            continue;
        }

        // fileOffset = headerSize + (rawOffset << 2)
        let file_offset = header_size.checked_add((raw_off as usize).checked_mul(4)?)?;
        let file_size = sz as usize;

        if file_offset.checked_add(file_size)? > pk_data.len() {
            continue;
        }

        let slice = &pk_data[file_offset..file_offset + file_size];
        if slice.len() >= 4 {
            let magic = u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]);
            if magic == target_magic {
                return Some(slice);
            }
        }
    }

    None
}

// ── Lecture des sections G4SK ─────────────────────────────────────────────────

/// Calcule l'offset absolu d'une section G4SK.
///
/// Le champ u16 à `field_pos` est un compte de dwords depuis 0x40 (fin de l'en-tête).
/// Adresse absolue = `0x40 + raw * 4`.
fn section_abs_offset(sk: &[u8], field_pos: usize) -> Result<usize, FormatError> {
    let raw = read_u16(sk, field_pos)? as usize;
    Ok(0x40_usize + raw * 4)
}

/// Lit les noms des bones depuis la table d'offsets de noms.
///
/// La table contient `bone_count` × u16. Chaque entrée est un offset **relatif
/// au début de la table elle-même** vers la chaîne null-terminée UTF-8.
/// Fallback `Bone_{i}` si l'offset est hors limites.
fn read_bone_names(sk: &[u8], table_offset: usize, bone_count: usize) -> Vec<String> {
    let mut names = Vec::with_capacity(bone_count);
    for i in 0..bone_count {
        let entry_pos = table_offset + i * 2;
        let name = (|| -> Option<String> {
            let rel_off =
                u16::from_le_bytes(sk.get(entry_pos..entry_pos + 2)?.try_into().ok()?) as usize;
            let abs_pos = table_offset.checked_add(rel_off)?;
            if abs_pos >= sk.len() {
                return None;
            }
            Some(read_null_terminated_utf8(sk, abs_pos))
        })()
        .unwrap_or_else(|| format!("Bone_{i}"));
        names.push(name);
    }
    names
}

/// Lit les indices parents.
///
/// Chaque entrée est un i16. Une valeur hors `[0, bone_count)` signifie racine → -1.
fn read_parent_indices(sk: &[u8], table_offset: usize, bone_count: usize) -> Vec<i32> {
    let mut parents = Vec::with_capacity(bone_count);
    for i in 0..bone_count {
        let pos = table_offset + i * 2;
        let raw: i16 = match sk.get(pos..pos + 2).and_then(|s| s.try_into().ok()) {
            Some(b) => i16::from_le_bytes(b),
            None => {
                parents.push(-1);
                continue;
            }
        };
        let p = if raw >= 0 && (raw as usize) < bone_count {
            raw as i32
        } else {
            -1
        };
        parents.push(p);
    }
    parents
}

/// Lit les matrices bind-pose locales depuis le bloc à l'offset 0x40 du G4SK.
///
/// Chaque matrice est 3 rangées × 4 floats = 48 octets :
/// - ligne 0 = (r00, r01, r02, tx)  — pour 2D pur : (sx, 0, 0, tx)
/// - ligne 1 = (r10, r11, r12, ty)  — pour 2D pur : (0, sy, 0, ty)
/// - ligne 2 = (r20, r21, sz,  0)
///
/// Offsets dans la matrice : r00@+0, r01@+4, tx@+12, r10@+16, r11@+20, ty@+28.
///
/// Décomposition 2D : `sx = √(r00²+r10²)`, `sy = √(r01²+r11²)`, `rot = atan2(r10, r00)`.
fn read_local_bind_poses(sk: &[u8], bone_count: usize) -> Result<Vec<Transform2D>, FormatError> {
    const BASE: usize = 0x40;
    let mut poses = Vec::with_capacity(bone_count);

    for i in 0..bone_count {
        let off = BASE + i * MATRIX34_STRIDE;

        if off + MATRIX34_STRIDE > sk.len() {
            poses.push(Transform2D::ZERO);
            continue;
        }

        // Ligne 0 : (r00, r01, _, tx)
        let r00 = read_f32(sk, off)?;
        let r01 = read_f32(sk, off + 4)?;
        let tx = read_f32(sk, off + 12)?;
        // Ligne 1 : (r10, r11, _, ty)
        let r10 = read_f32(sk, off + 16)?;
        let r11 = read_f32(sk, off + 20)?;
        let ty = read_f32(sk, off + 28)?;

        // Décomposer scale et rotation depuis la matrice 2D.
        // sx = longueur de la première colonne (r00, r10).
        // sy = longueur de la deuxième colonne (r01, r11).
        // rot = atan2(r10, r00).
        let mut sx = (r00 * r00 + r10 * r10).sqrt();
        let mut sy = (r01 * r01 + r11 * r11).sqrt();
        let rot = r10.atan2(r00);

        // Si sx/sy proches de zéro, utiliser la diagonale directement.
        if sx < 1e-6_f32 {
            sx = r00.abs();
        }
        if sy < 1e-6_f32 {
            sy = r11.abs();
        }

        poses.push(Transform2D {
            x: tx,
            y: ty,
            scale_x: sx,
            scale_y: sy,
            rot,
            anchor_x: 0.5,
            anchor_y: 0.5,
        });
    }

    Ok(poses)
}

// ── Composition de la hiérarchie ──────────────────────────────────────────────

/// Compose les matrices 2D à travers la hiérarchie pour obtenir les poses monde.
///
/// Pour chaque bone, la pose monde = compose(world[parent], local[i]).
/// Les racines (parent < 0) ont world = local.
fn compute_world_poses(local: &[Transform2D], parents: &[i32]) -> Vec<Transform2D> {
    let n = local.len();
    let mut world = vec![Transform2D::ZERO; n];
    let mut computed = vec![false; n];
    for i in 0..n {
        compute_world_recursive(i, local, parents, &mut world, &mut computed);
    }
    world
}

fn compute_world_recursive(
    i: usize,
    local: &[Transform2D],
    parents: &[i32],
    world: &mut Vec<Transform2D>,
    computed: &mut Vec<bool>,
) {
    if computed[i] {
        return;
    }
    let p = parents[i];
    if p < 0 || p as usize >= local.len() {
        // Racine : pose monde = pose locale.
        world[i] = local[i];
    } else {
        let pu = p as usize;
        compute_world_recursive(pu, local, parents, world, computed);
        let parent_world = world[pu];
        world[i] = compose_transforms(parent_world, local[i]);
    }
    computed[i] = true;
}

/// Compose deux transforms 2D (parent × child) en tenant compte de la rotation.
///
/// Utilise la multiplication matricielle 3×3 (espace 2D homogène) :
///
/// ```text
/// parent = [sx·cos(r)  -sy·sin(r)  tx]    child = [sx·cos(r)  -sy·sin(r)  tx]
///          [sx·sin(r)   sy·cos(r)  ty]             [sx·sin(r)   sy·cos(r)  ty]
///          [0           0           1]             [0           0           1]
/// ```
fn compose_transforms(parent: Transform2D, child: Transform2D) -> Transform2D {
    let pc = parent.rot.cos();
    let ps = parent.rot.sin();
    let cc = child.rot.cos();
    let cs = child.rot.sin();

    // Colonnes de la matrice parent (partie 2×2 rotation×échelle).
    let p00 = parent.scale_x * pc;
    let p01 = -parent.scale_y * ps;
    let p10 = parent.scale_x * ps;
    let p11 = parent.scale_y * pc;

    // Colonnes de la matrice child.
    let c00 = child.scale_x * cc;
    let c01 = -child.scale_y * cs;
    let c10 = child.scale_x * cs;
    let c11 = child.scale_y * cc;

    // Produit matriciel 2×2 : résultat = parent × child.
    let r00 = p00 * c00 + p01 * c10;
    let r01 = p00 * c01 + p01 * c11;
    let r10 = p10 * c00 + p11 * c10;
    let r11 = p10 * c01 + p11 * c11;

    // Translation monde : parent_mat × child.translation + parent.translation.
    let world_tx = p00 * child.x + p01 * child.y + parent.x;
    let world_ty = p10 * child.x + p11 * child.y + parent.y;

    // Décomposer le résultat.
    let mut world_sx = (r00 * r00 + r10 * r10).sqrt();
    let mut world_sy = (r01 * r01 + r11 * r11).sqrt();
    let world_rot = r10.atan2(r00);

    if world_sx < 1e-6_f32 {
        world_sx = 1.0;
    }
    if world_sy < 1e-6_f32 {
        world_sy = 1.0;
    }

    Transform2D {
        x: world_tx,
        y: world_ty,
        scale_x: world_sx,
        scale_y: world_sy,
        rot: world_rot,
        anchor_x: 0.5,
        anchor_y: 0.5,
    }
}

// ── Utilitaires ───────────────────────────────────────────────────────────────

fn read_null_terminated_utf8(data: &[u8], offset: usize) -> String {
    let slice = data.get(offset..).unwrap_or(&[]);
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).into_owned()
}

fn read_u16(data: &[u8], off: usize) -> Result<u16, FormatError> {
    let b: [u8; 2] = data
        .get(off..off + 2)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("g4pkm : lecture u16 hors limites"))?;
    Ok(u16::from_le_bytes(b))
}

fn read_u32(data: &[u8], off: usize) -> Result<u32, FormatError> {
    let b: [u8; 4] = data
        .get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("g4pkm : lecture u32 hors limites"))?;
    Ok(u32::from_le_bytes(b))
}

fn read_f32(data: &[u8], off: usize) -> Result<f32, FormatError> {
    let b: [u8; 4] = data
        .get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("g4pkm : lecture f32 hors limites"))?;
    Ok(f32::from_le_bytes(b))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tests unitaires (pas de fixtures réelles) ─────────────────────────────

    #[test]
    fn magic_g4pkm_invalide_retourne_err() {
        let mut data = std::vec![0u8; 64];
        data[0] = b'X';
        data[1] = b'X';
        data[2] = b'X';
        data[3] = b'X';
        assert!(parse(&data).is_err(), "magic invalide doit retourner Err");
    }

    #[test]
    fn trop_court_g4pkm_retourne_err() {
        assert!(
            parse(&[0u8; 4]).is_err(),
            "tampon < 0x40 doit retourner Err"
        );
    }

    #[test]
    fn magic_g4sk_invalide_retourne_err() {
        let mut data = std::vec![0u8; 64];
        data[0] = b'X';
        data[1] = b'X';
        data[2] = b'X';
        data[3] = b'X';
        assert!(
            parse_g4sk(&data).is_err(),
            "magic G4SK invalide doit retourner Err"
        );
    }

    #[test]
    fn trop_court_g4sk_retourne_err() {
        assert!(
            parse_g4sk(&[0u8; 16]).is_err(),
            "tampon < 0x40 doit retourner Err"
        );
    }

    #[test]
    fn zero_to_css_donne_centre_canvas() {
        let (px, py) = Transform2D::ZERO.to_css_1280x720();
        assert!((px - 640.0).abs() < 0.01, "px attendu 640, obtenu {px}");
        assert!((py - 360.0).abs() < 0.01, "py attendu 360, obtenu {py}");
    }

    #[test]
    fn bord_droit_x960_mappe_en_px1280() {
        // x=960 = bord droit 1920 (depuis centre).
        let t = Transform2D {
            x: 960.0,
            ..Transform2D::ZERO
        };
        let (px, _) = t.to_css_1280x720();
        assert!((px - 1280.0).abs() < 0.01, "px attendu 1280, obtenu {px}");
    }

    #[test]
    fn bord_haut_y540_mappe_en_py0() {
        // y=540 = bord haut 1080 (depuis centre, Y haut positif).
        let t = Transform2D {
            y: 540.0,
            ..Transform2D::ZERO
        };
        let (_, py) = t.to_css_1280x720();
        assert!((py - 0.0).abs() < 0.01, "py attendu 0, obtenu {py}");
    }

    #[test]
    fn bord_bas_y_neg540_mappe_en_py720() {
        // y=-540 = bord bas 1080 (depuis centre, Y bas négatif).
        let t = Transform2D {
            y: -540.0,
            ..Transform2D::ZERO
        };
        let (_, py) = t.to_css_1280x720();
        assert!((py - 720.0).abs() < 0.01, "py attendu 720, obtenu {py}");
    }

    #[test]
    fn hors_ecran_true_quand_x_depasse_960() {
        let t = Transform2D {
            x: 1873.0,
            y: -39.0,
            ..Transform2D::ZERO
        };
        assert!(t.is_off_screen_1920(), "1873 > 960, doit être hors-écran");
    }

    #[test]
    fn hors_ecran_false_quand_dans_zone_visible() {
        let t = Transform2D {
            x: -40.0,
            y: 40.0,
            ..Transform2D::ZERO
        };
        assert!(
            !t.is_off_screen_1920(),
            "(-40, 40) est dans le canvas 1920×1080"
        );
    }

    // ── Tests golden sur fichiers réels (VFS) ─────────────────────────────────

    /// Résout le premier chemin VFS dont le basename se termine par `suffix`.
    fn find_vfs_path(vfs: &crate::vfs::Vfs, suffix: &str) -> Option<std::string::String> {
        vfs.iter()
            .find(|(p, _)| p.ends_with(suffix))
            .map(|(p, _)| p.to_string())
    }

    #[test]
    fn golden_g4pkm_option02_02_win00_04_title00_09() {
        use crate::vfs::Vfs;
        use std::path::Path;

        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = Path::new(&dir).join("data");

        let mut vfs = Vfs::new();
        if vfs.init(&data_dir).is_err() {
            eprintln!("skip golden_g4pkm : jeu absent à {}", data_dir.display());
            return;
        }

        // ── option02_02.g4pkm : 4 bones, plein-écran (0, 0, 1920, 1080) ──────
        match find_vfs_path(&vfs, "option02_02.g4pkm") {
            None => eprintln!("skip option02_02.g4pkm : non trouvé dans le VFS"),
            Some(path) => {
                let data = vfs.read(&path).expect("lecture option02_02.g4pkm");
                let layout = parse(&data).expect("parse option02_02.g4pkm");

                assert_eq!(layout.bone_count(), 4, "option02_02 : bone_count attendu 4");

                let nvidia = layout
                    .bones
                    .iter()
                    .find(|b| b.name.contains("nvidia") || b.name.contains("license"))
                    .expect("option02_02 : bone nvidia/license absent");
                let t = nvidia.world_bind_pose;
                eprintln!(
                    "option02_02 {} : X={} Y={} ScaleX={} ScaleY={}",
                    nvidia.name, t.x, t.y, t.scale_x, t.scale_y
                );
                assert!(t.x.abs() < 1.0, "option02_02 nvidia X≈0, obtenu {}", t.x);
                assert!(t.y.abs() < 1.0, "option02_02 nvidia Y≈0, obtenu {}", t.y);
                assert!(
                    (t.scale_x - 1920.0).abs() < 1.0,
                    "option02_02 nvidia ScaleX≈1920, obtenu {}",
                    t.scale_x
                );
                assert!(
                    (t.scale_y - 1080.0).abs() < 1.0,
                    "option02_02 nvidia ScaleY≈1080, obtenu {}",
                    t.scale_y
                );

                assert!(
                    layout.bones.iter().any(|b| b.name.starts_with("option02")),
                    "option02_02 : aucun bone ne commence par 'option02'"
                );
                assert!(
                    layout.bones.iter().any(|b| b.name.contains("pos_base")),
                    "option02_02 : aucun bone ne contient 'pos_base'"
                );
            }
        }

        // ── win00_04.g4pkm : 5 bones, curseur à (-40, 40) de 80×80 ────────────
        match find_vfs_path(&vfs, "win00_04.g4pkm") {
            None => eprintln!("skip win00_04.g4pkm : non trouvé dans le VFS"),
            Some(path) => {
                let data = vfs.read(&path).expect("lecture win00_04.g4pkm");
                let layout = parse(&data).expect("parse win00_04.g4pkm");
                eprintln!("win00_04 : {} bones", layout.bone_count());
                assert_eq!(layout.bone_count(), 5, "win00_04 : bone_count attendu 5");

                let c = layout
                    .world_pose("_cursor01")
                    .expect("win00_04 : _cursor01 absent");
                eprintln!(
                    "win00_04 _cursor01 : X={} Y={} ScaleX={} ScaleY={}",
                    c.x, c.y, c.scale_x, c.scale_y
                );
                assert!((c.x - -40.0).abs() < 1.0, "_cursor01 X≈-40, obtenu {}", c.x);
                assert!((c.y - 40.0).abs() < 1.0, "_cursor01 Y≈40, obtenu {}", c.y);
                assert!(
                    (c.scale_x - 80.0).abs() < 1.0,
                    "_cursor01 ScaleX≈80, obtenu {}",
                    c.scale_x
                );
                assert!(
                    (c.scale_y - 80.0).abs() < 1.0,
                    "_cursor01 ScaleY≈80, obtenu {}",
                    c.scale_y
                );
                assert!(!c.is_off_screen_1920(), "_cursor01 est dans le canvas");

                // Projection CSS : 640 − 40·(2/3) ≈ 613 ; 360 − 40·(2/3) ≈ 333.
                let (px, py) = c.to_css_1280x720();
                assert!(
                    (600.0..=625.0).contains(&px),
                    "_cursor01 px hors [600,625] : {px}"
                );
                assert!(
                    (320.0..=345.0).contains(&py),
                    "_cursor01 py hors [320,345] : {py}"
                );

                // Hiérarchie : racine `win00`, puis `win00_04_output` accroché dessus.
                assert_eq!(layout.bones[0].parent_index, -1, "bone 0 = racine");
                assert_eq!(
                    layout.bones[1].parent_index, 0,
                    "bone 1 accroché à la racine"
                );
            }
        }

        // ── title00_09.g4pkm : 20 bones nommés, `_pos_scl_base01` hors écran ──
        match find_vfs_path(&vfs, "title00_09.g4pkm") {
            None => eprintln!("skip title00_09.g4pkm : non trouvé dans le VFS"),
            Some(path) => {
                let data = vfs.read(&path).expect("lecture title00_09.g4pkm");
                let layout = parse(&data).expect("parse title00_09.g4pkm");
                eprintln!("title00_09 : {} bones", layout.bone_count());
                assert_eq!(
                    layout.bone_count(),
                    20,
                    "title00_09 : bone_count attendu 20"
                );

                // Les 20 noms, dans l'ordre du fichier. `G4pkmLayoutTests.cs` n'en assérait que
                // neuf ; la table de noms en contient vingt, et c'est elle la vérité terrain.
                const NOMS: [&str; 20] = [
                    "title00",
                    "title00_09_output",
                    "_pos_base01",
                    "_pos_scl_base01",
                    "_gtxt_ver01",
                    "_gtxt_dot01",
                    "_gtxt_dot02",
                    "_gtxt_dot03",
                    "_num_ver01",
                    "_num_ver_dmy01",
                    "_num_ver02",
                    "_num_ver_dmy02",
                    "_num_ver03",
                    "_num_ver_dmy03",
                    "_num_save01",
                    "_num_save_dmy01",
                    "_num_save02",
                    "_num_save_dmy02",
                    "_num_net01",
                    "_num_net_dmy01",
                ];
                let obtenus: Vec<&str> = layout.bones.iter().map(|b| b.name.as_str()).collect();
                assert_eq!(obtenus, NOMS, "title00_09 : table de noms");

                // Aucun nom fabriqué : `Bone_<n>` est ce que produit le fallback heuristique de
                // `g4sk`. Sa présence signifierait que la table de noms n'a pas été lue.
                for b in &layout.bones {
                    let fabrique = b
                        .name
                        .strip_prefix("Bone_")
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()));
                    assert!(!fabrique, "nom fabriqué par l'heuristique : {}", b.name);
                }

                // Racine à l'origine.
                let racine = &layout.bones[0];
                assert_eq!(racine.parent_index, -1, "title00 est la racine");
                assert!(racine.world_bind_pose.x.abs() < 1.0, "title00 X≈0");
                assert!(racine.world_bind_pose.y.abs() < 1.0, "title00 Y≈0");

                // `_pos_scl_base01` : hors écran à droite, mis à l'échelle 0.65 × 0.9.
                let p = layout
                    .world_pose("_pos_scl_base01")
                    .expect("_pos_scl_base01 absent");
                eprintln!(
                    "title00_09 _pos_scl_base01 : X={} Y={} ScaleX={} ScaleY={}",
                    p.x, p.y, p.scale_x, p.scale_y
                );
                assert!(
                    (p.x - 1873.0).abs() < 1.0,
                    "_pos_scl_base01 X≈1873, obtenu {}",
                    p.x
                );
                assert!(
                    (p.y - -39.0).abs() < 1.0,
                    "_pos_scl_base01 Y≈-39, obtenu {}",
                    p.y
                );
                assert!(
                    p.is_off_screen_1920(),
                    "_pos_scl_base01 est hors du canvas 1920×1080"
                );
                assert!(
                    (0.6..=0.7).contains(&p.scale_x),
                    "ScaleX≈0.65, obtenu {}",
                    p.scale_x
                );
                assert!(
                    (0.85..=0.95).contains(&p.scale_y),
                    "ScaleY≈0.9, obtenu {}",
                    p.scale_y
                );

                // `_gtxt_ver01` hérite de ces échelles : 0.65×104 ≈ 67.6 et 0.9×32.4 ≈ 29.2.
                let g = layout
                    .world_pose("_gtxt_ver01")
                    .expect("_gtxt_ver01 absent");
                assert!(
                    (50.0..=90.0).contains(&g.scale_x),
                    "_gtxt_ver01 ScaleX {}",
                    g.scale_x
                );
                assert!(
                    (20.0..=45.0).contains(&g.scale_y),
                    "_gtxt_ver01 ScaleY {}",
                    g.scale_y
                );

                // Un nom absent ne doit pas être inventé.
                assert!(layout.world_pose("_does_not_exist_bone_xyz").is_none());
            }
        }
    }
}
