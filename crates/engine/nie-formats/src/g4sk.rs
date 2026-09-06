//! Lecteur de squelettes G4SK (Level-5 « Graphics 4 Skeleton »).
//!
//! Port Rust de `IECODE.Core/Formats/Level5/G4skParser.cs`, **complété** par le vrai layout
//! recoupé sur un `.g4sk` réel extrait du jeu (`s28g001b.g4sk`, 19 os).
//!
//! ## En-tête : DÉTERMINISTE et vérifiable
//!
//! `ParseHeader` (G4skParser.cs L51) lit des champs à offsets fixes : magic `0x4B533447`
//! "G4SK", header_size u16 @0x04, type_id u16 @0x06, file_size u32 @0x0C, bone_count u16 @0x20.
//! Porté tel quel.
//!
//! ## Hiérarchie d'os : RÉSOLUE au réel (table d'offsets de l'en-tête)
//!
//! Le parser C# `ParseBones` (L69) était **heuristique** : `FindParentIndicesOffset` scanne à
//! partir de `0x1000`. Or les `.g4sk` réels font **moins de 0x1000 octets** (s28g001b.g4sk =
//! 3344 octets) → cette heuristique **ne se déclenche jamais** et la hiérarchie restait vide.
//!
//! Le vrai layout, recoupé octet par octet sur `s28g001b.g4sk`, est une **table d'offsets**
//! immédiatement après `bone_count` :
//!
//! ```text
//! @0x20  u16  bone_count                 (= N, ici 19)
//! @0x22  u16  slot[0]  matrices bind-pose (dwords depuis 0x40)
//! @0x24  u16  slot[1]  matrices (autre jeu)
//! @0x26  u16  slot[2]  matrices (autre jeu)
//! @0x28  u16  slot[3]  données transform
//! @0x2A  u16  slot[4]  ► PARENT INDICES : N × i16 (sentinelle = bone_count → racine)
//! @0x2C  u16  slot[5]  permutation/remap d'os (N × i16, permutation de 0..N)
//! @0x2E  u16  slot[6]  …
//! @0x30  u16  slot[7]  …
//! @0x32  u16  slot[8]  ► NAME OFFSET TABLE : N × u16 (offsets relatifs à la base de cette table)
//! ```
//!
//! Chaque `slot[i]` est un **compte de dwords** : adresse fichier = `0x40 + slot[i] * 4`.
//! (header_size @0x04 = 0x40 = fin d'en-tête.)
//!
//! - **Parents** (`slot[4]`) : `N` `i16`. La sentinelle racine est `bone_count` (et non `-1`).
//!   Validation : chaque parent ∈ `[0, N]` (où `N` = racine) ou `-1`, et l'arbre est acyclique.
//! - **Noms** (`slot[8]`) : `N` `u16` formant une table d'offsets ; `name[i]` est la chaîne
//!   nulle-terminée à `base_table + offset[i]`, où `base_table` est l'adresse fichier du slot 8.
//!
//! Recoupement réel (`s28g001b.g4sk`) :
//! `parents = [19,0,1,2,1,1,5,5,5,5,5,5,5,5,5,19,15,16,16]` → arbre acyclique cohérent ;
//! `names   = [s28g001b_map, bk00_s28g001b, spatial_b_00, range_00_1, model_a_00, model_r_00,
//!             ao200, ao241, s28g001g01..07, instance, bk00, ao241_bk00_01, ao241_bk00_02]`.
//! L'os 0 (`s28g001b_map`) a pour parent `19` = sentinelle racine.
//!
//! ## Garde-fous anti-hallucination
//!
//! [`parse_hierarchy`] **valide** la table avant de l'accepter (parents bornés + arbre
//! acyclique + noms imprimables résolvables). Si la validation échoue, on retombe sur
//! [`parse_parents_heuristic`] (le scan C# d'origine, marqué `heuristic = true`) plutôt que de
//! fabriquer une hiérarchie. Les deux chemins renvoient le même type ; `heuristic` indique la
//! provenance.
//!
//! Compatible `no_std + alloc`.

extern crate alloc;
use alloc::{string::String, vec::Vec};

use crate::FormatError;

/// Magic « G4SK » little-endian (`0x4B533447`).
pub const MAGIC: u32 = 0x4B53_3447;
/// Magic « G4SK » en octets.
pub const MAGIC_BYTES: [u8; 4] = *b"G4SK";

/// Fin de l'en-tête fixe (= `header_size` réel observé). Base des slots de la table d'offsets.
const HEADER_END: usize = 0x40;

/// Offset du `bone_count` (u16).
const BONE_COUNT_OFF: usize = 0x20;

/// Offset du premier slot de la table d'offsets (juste après `bone_count`).
const SLOT_TABLE_OFF: usize = 0x22;

/// Index du slot pointant la table des parents (recoupé sur le réel).
const SLOT_PARENTS: usize = 4;

/// Index du slot pointant la table d'offsets de noms (recoupé sur le réel).
const SLOT_NAMES: usize = 8;

/// En-tête G4SK (déterministe, vérifiable).
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4skHeader {
    /// Taille de l'en-tête (@0x04).
    pub header_size: u16,
    /// Identifiant de type (@0x06).
    pub type_id: u16,
    /// Taille totale du fichier (@0x0C). Note : le pool de chaînes peut s'étendre au-delà.
    pub file_size: u32,
    /// Nombre d'os (@0x20).
    pub bone_count: u16,
}

/// Os décodé.
///
/// `parent_index`/`name` sont fiables si `heuristic == false` (résolus via la table d'offsets
/// réelle). Si `heuristic == true`, ils proviennent du scan C# non recoupé (indicatif).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bone {
    /// Index de l'os.
    pub index: usize,
    /// Index du parent. `-1` = racine. Note : dans le format réel la sentinelle racine est
    /// `bone_count` ; [`parse_hierarchy`] la **normalise en `-1`** ici pour une sémantique
    /// uniforme.
    pub parent_index: i16,
    /// Nom (peut être `Bone_<i>` si non résolu).
    pub name: String,
}

/// Hiérarchie d'os.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4skBones {
    /// Os résolus.
    pub bones: Vec<Bone>,
    /// `false` = résolu via la table d'offsets réelle (fiable) ; `true` = heuristique C#
    /// (indicatif, non recoupé).
    pub heuristic: bool,
}

/// Vrai si `data` commence par le magic G4SK.
#[must_use]
pub fn is_g4sk(data: &[u8]) -> bool {
    data.starts_with(&MAGIC_BYTES)
}

/// Parse l'en-tête G4SK (garanti).
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si `data` < 0x22 octets.
/// - [`FormatError::BadMagic`] si le magic n'est pas G4SK.
pub fn parse_header(data: &[u8]) -> Result<G4skHeader, FormatError> {
    if data.len() < 0x22 {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: 0x22,
        });
    }
    if !is_g4sk(data) {
        return Err(FormatError::BadMagic { format: "G4SK" });
    }
    Ok(G4skHeader {
        header_size: read_u16(data, 0x04)?,
        type_id: read_u16(data, 0x06)?,
        file_size: read_u32(data, 0x0C)?,
        bone_count: read_u16(data, BONE_COUNT_OFF)?,
    })
}

/// Résout l'adresse fichier d'un slot de la table d'offsets de l'en-tête.
///
/// `slot[i]` (u16 @ `0x22 + i*2`) est un compte de dwords depuis [`HEADER_END`] (0x40).
/// Renvoie `None` si l'index ou l'adresse calculée sort des limites.
#[must_use]
pub fn slot_offset(data: &[u8], slot: usize) -> Option<usize> {
    let raw = read_u16(data, SLOT_TABLE_OFF + slot * 2).ok()? as usize;
    let addr = HEADER_END.checked_add(raw.checked_mul(4)?)?;
    if addr <= data.len() { Some(addr) } else { None }
}

/// Parse la hiérarchie d'os via la **table d'offsets réelle** de l'en-tête.
///
/// Lit `slot[4]` → parents (`N` × i16), `slot[8]` → table d'offsets de noms, puis valide
/// (parents bornés, arbre acyclique, noms résolvables). En cas de succès, renvoie une
/// [`G4skBones`] avec `heuristic = false`. En cas d'échec de validation, **retombe** sur
/// [`parse_parents_heuristic`] (sans fabrication).
///
/// La sentinelle racine du format (`parent == bone_count`) est normalisée en `-1`.
#[must_use]
pub fn parse_hierarchy(data: &[u8], header: &G4skHeader) -> G4skBones {
    match try_parse_real_hierarchy(data, header) {
        Some(bones) => bones,
        None => parse_parents_heuristic(data, header),
    }
}

/// Tentative stricte de lecture via la table d'offsets. `None` si quoi que ce soit ne valide pas.
fn try_parse_real_hierarchy(data: &[u8], header: &G4skHeader) -> Option<G4skBones> {
    let n = header.bone_count as usize;
    if n == 0 {
        return Some(G4skBones {
            bones: Vec::new(),
            heuristic: false,
        });
    }

    let parents_off = slot_offset(data, SLOT_PARENTS)?;
    let names_table_off = slot_offset(data, SLOT_NAMES)?;

    // --- 1) Lire et valider les parents ---
    // Sentinelle racine = bone_count (format réel). Parents valides ∈ [-1, bone_count].
    let mut parents: Vec<i16> = Vec::with_capacity(n);
    parents_off.checked_add(n * 2)?;
    if parents_off + n * 2 > data.len() {
        return None;
    }
    for i in 0..n {
        let val = read_u16(data, parents_off + i * 2).ok()? as i16;
        // -1 (déjà racine) OU 0..=bone_count (bone_count = sentinelle racine).
        if val < -1 || (val as i64) > n as i64 {
            return None;
        }
        parents.push(val);
    }
    // Normaliser la sentinelle bone_count en -1 et vérifier l'acyclicité.
    let normalized: Vec<i16> = parents
        .iter()
        .map(|&p| if p as i64 == n as i64 { -1 } else { p })
        .collect();
    if !is_acyclic_forest(&normalized) {
        return None;
    }

    // --- 2) Lire et valider la table d'offsets de noms ---
    if names_table_off + n * 2 > data.len() {
        return None;
    }
    let mut names: Vec<String> = Vec::with_capacity(n);
    for i in 0..n {
        let name_off = read_u16(data, names_table_off + i * 2).ok()? as usize;
        let abs = names_table_off.checked_add(name_off)?;
        let name = read_printable_cstr(data, abs)?;
        // Un nom d'os IEVR n'est jamais vide.
        if name.is_empty() {
            return None;
        }
        names.push(name);
    }

    let bones = (0..n)
        .map(|i| Bone {
            index: i,
            parent_index: normalized[i],
            name: names[i].clone(),
        })
        .collect();

    Some(G4skBones {
        bones,
        heuristic: false,
    })
}

/// Vrai si `parents` (où `-1` = racine) forme une forêt acyclique d'indices valides.
fn is_acyclic_forest(parents: &[i16]) -> bool {
    let n = parents.len();
    for start in 0..n {
        let mut cur = start as i64;
        let mut steps = 0usize;
        loop {
            let p = parents[cur as usize];
            if p == -1 {
                break;
            }
            if p < 0 || (p as usize) >= n {
                return false;
            }
            // Auto-parent ⇒ cycle.
            if p as usize == cur as usize {
                return false;
            }
            cur = p as i64;
            steps += 1;
            if steps > n {
                return false; // cycle détecté (chemin plus long que le nombre d'os).
            }
        }
    }
    true
}

/// Heuristique de parent-indices (port de `FindParentIndicesOffset`, G4skParser.cs L147).
///
/// Cherche, à partir de 0x1000, une suite de `bone_count` i16 ∈ [-1, bone_count) contenant au
/// moins un 0 ou un -1. **Fallback uniquement** (ne se déclenche que sur des fichiers ≥ 0x1000,
/// ce qui est rare). Conservé pour fidélité au C#. Renvoie l'offset trouvé ou `None`.
#[must_use]
pub fn find_parent_indices_offset(data: &[u8], bone_count: usize) -> Option<usize> {
    if bone_count == 0 {
        return None;
    }
    let array_size = bone_count * 2;
    let start = 0x1000usize;
    if data.len() < array_size {
        return None;
    }
    let mut i = start;
    while i + array_size <= data.len() {
        let mut valid = true;
        let mut zero_or_neg = false;
        for j in 0..bone_count {
            let val = read_u16(data, i + j * 2).unwrap_or(0) as i16;
            if val < -1 || (val as i64) >= bone_count as i64 {
                valid = false;
                break;
            }
            if val == 0 || val == -1 {
                zero_or_neg = true;
            }
        }
        if valid && zero_or_neg {
            return Some(i);
        }
        i += 2;
    }
    None
}

/// Extrait les parents d'os par heuristique bornée (port de `ParseBones`). **Fallback** : marqué
/// non fiable (`heuristic = true`). Les noms ne sont pas devinés ici : `Bone_<i>`.
#[must_use]
pub fn parse_parents_heuristic(data: &[u8], header: &G4skHeader) -> G4skBones {
    let bone_count = header.bone_count as usize;
    let parents_off = find_parent_indices_offset(data, bone_count);

    let mut bones = Vec::with_capacity(bone_count);
    for i in 0..bone_count {
        let parent_index = match parents_off {
            Some(off) => read_u16(data, off + i * 2).unwrap_or(0) as i16,
            None => -1,
        };
        bones.push(Bone {
            index: i,
            parent_index,
            name: alloc::format!("Bone_{i}"),
        });
    }

    G4skBones {
        bones,
        heuristic: true,
    }
}

/// Lit une chaîne nulle-terminée **imprimable** (ASCII 0x20..=0x7E) à `off`. `None` si le
/// premier octet n'est pas imprimable, si pas de terminateur, ou hors limites.
fn read_printable_cstr(data: &[u8], off: usize) -> Option<String> {
    if off >= data.len() {
        return None;
    }
    let mut end = off;
    while end < data.len() && data[end] != 0 {
        let b = data[end];
        if !(0x20..=0x7E).contains(&b) {
            return None;
        }
        end += 1;
    }
    if end >= data.len() {
        return None; // pas de terminateur ⇒ chaîne tronquée, on refuse.
    }
    // Tous les octets sont ASCII imprimable ⇒ UTF-8 valide.
    String::from_utf8(data[off..end].to_vec()).ok()
}

fn read_u16(data: &[u8], off: usize) -> Result<u16, FormatError> {
    let b: [u8; 2] = data
        .get(off..off + 2)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("G4SK : lecture u16 hors limites"))?;
    Ok(u16::from_le_bytes(b))
}

fn read_u32(data: &[u8], off: usize) -> Result<u32, FormatError> {
    let b: [u8; 4] = data
        .get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("G4SK : lecture u32 hors limites"))?;
    Ok(u32::from_le_bytes(b))
}

fn read_f32(data: &[u8], off: usize) -> Option<f32> {
    let b: [u8; 4] = data.get(off..off + 4)?.try_into().ok()?;
    Some(f32::from_le_bytes(b))
}

// ============================================================================
// Poses de bind (animation/skinning) — sections décodées byte-à-byte (workflow anim-re-crack,
// validé : `max|A·B−I|<1.1e-7`, FK(C) reproduit la bind-pose monde à `<1e-7`).
// Le répertoire d'offsets de l'en-tête (slots, [`slot_offset`]) pointe les sections en index de
// float depuis 0x40 : slot[1]=B (inverse-bind), slot[2]=C (pose locale TRS).
// ============================================================================

/// Pose locale d'un os (relative au parent) : scale, quaternion `(x,y,z,w)`, translation.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LocalTrs {
    pub scale: [f32; 3],
    pub quat: [f32; 4],
    pub translation: [f32; 3],
}

/// Pose de repos complète d'un os : TRS local (FK) + matrice inverse-bind (skinning).
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BonePose {
    /// Pose locale de repos — base de la cinématique directe animée.
    pub local: LocalTrs,
    /// Matrice inverse-bind 4×4 (col-major `m[col][row]`), pour transformer les sommets en espace os.
    pub inverse_bind: [[f32; 4]; 4],
}

/// Lit une matrice 3×4 row-major (ligne `[m0,m1,m2,t]`) à l'octet `base + idx*48`, en 4×4 col-major
/// avec dernière ligne `[0,0,0,1]`.
#[allow(clippy::needless_range_loop)] // transposition row-major → col-major : indices explicites.
fn read_mat3x4(data: &[u8], base: usize, idx: usize) -> Option<[[f32; 4]; 4]> {
    let o = base + idx * 12 * 4;
    let mut m = [[0.0f32; 4]; 4];
    for r in 0..3 {
        for c in 0..4 {
            // élément ligne r, colonne c → m[col=c][row=r].
            m[c][r] = read_f32(data, o + (r * 4 + c) * 4)?;
        }
    }
    m[3][3] = 1.0;
    Some(m)
}

/// Décode les poses de bind par os (sections B + C). Renvoie `None` si les offsets/longueurs
/// sortent des limites. `header.bone_count` os attendus.
#[must_use]
pub fn parse_poses(data: &[u8], header: &G4skHeader) -> Option<Vec<BonePose>> {
    let n = header.bone_count as usize;
    let inv_base = slot_offset(data, 1)?; // section B (inverse-bind)
    let trs_base = slot_offset(data, 2)?; // section C (pose locale TRS)
    let mut poses = Vec::with_capacity(n);
    for i in 0..n {
        let inverse_bind = read_mat3x4(data, inv_base, i)?;
        // C : 12 floats/os = [sx,sy,sz,_, qx,qy,qz,qw, tx,ty,tz,_].
        let cf = |k: usize| read_f32(data, trs_base + (i * 12 + k) * 4);
        let local = LocalTrs {
            scale: [cf(0)?, cf(1)?, cf(2)?],
            quat: [cf(4)?, cf(5)?, cf(6)?, cf(7)?],
            translation: [cf(8)?, cf(9)?, cf(10)?],
        };
        poses.push(BonePose {
            local,
            inverse_bind,
        });
    }
    Some(poses)
}

/// Matrice 4×4 col-major d'une pose locale TRS : `T · R(quat) · S`.
#[must_use]
pub fn local_matrix(trs: &LocalTrs) -> [[f32; 4]; 4] {
    let [x, y, z, w] = trs.quat;
    let (xx, yy, zz) = (x * x, y * y, z * z);
    let (xy, xz, yz) = (x * y, x * z, y * z);
    let (wx, wy, wz) = (w * x, w * y, w * z);
    // Lignes de la matrice de rotation (convention standard quaternion → matrice).
    let r = [
        [1.0 - 2.0 * (yy + zz), 2.0 * (xy - wz), 2.0 * (xz + wy)],
        [2.0 * (xy + wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - wx)],
        [2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (xx + yy)],
    ];
    let s = trs.scale;
    let t = trs.translation;
    // Col-major m[col][row]. Colonne c = R[.][c]*scale[c] ; colonne 3 = translation.
    [
        [r[0][0] * s[0], r[1][0] * s[0], r[2][0] * s[0], 0.0],
        [r[0][1] * s[1], r[1][1] * s[1], r[2][1] * s[1], 0.0],
        [r[0][2] * s[2], r[1][2] * s[2], r[2][2] * s[2], 0.0],
        [t[0], t[1], t[2], 1.0],
    ]
}

/// Produit de deux matrices 4×4 col-major (`a·b`).
#[must_use]
pub fn mat_mul(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut m = [[0.0f32; 4]; 4];
    for (c, col) in m.iter_mut().enumerate() {
        for (r, cell) in col.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    m
}

/// Matrices MONDE de repos par cinématique directe (composition des poses locales le long des
/// parents). `parents[i] < 0` ou `>= i` ⇒ racine. Ordre des os = profondeur croissante en g4sk.
#[must_use]
pub fn rest_world_matrices(poses: &[BonePose], parents: &[i16]) -> Vec<[[f32; 4]; 4]> {
    let mut world: Vec<[[f32; 4]; 4]> = Vec::with_capacity(poses.len());
    for (i, p) in poses.iter().enumerate() {
        let local = local_matrix(&p.local);
        let par = parents.get(i).copied().unwrap_or(-1);
        let m = if par < 0 || (par as usize) >= i {
            local
        } else {
            mat_mul(&world[par as usize], &local)
        };
        world.push(m);
    }
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    /// G4SK réel du jeu (`s28g001b.g4pk` → `s28g001b.g4sk`, 3 344 octets, 19 os), **tranché à
    /// l'exécution** depuis l'archive du VFS.
    ///
    /// Ces tests s'adossaient auparavant à `include_bytes!("../tests/fixtures/s28g001b.g4sk")`
    /// sous la feature `real-fixtures` : un fichier gitignoré et absent, donc des assertions qui
    /// ne compilaient nulle part. L'archive, elle, est toujours là — et le G4PK donne offset et
    /// taille du sous-fichier, sans rien écrire sur disque.
    fn g4sk_reel() -> Option<Vec<u8>> {
        let (chemin, archive) = crate::g4pk::tests_vfs::lire_par_suffixe("s28g001b.g4pk")?;
        let pk = crate::g4pk::parse(&archive).expect("parse s28g001b.g4pk");
        let f = pk.files.iter().find(|f| f.name.ends_with(".g4sk"))?;
        std::eprintln!("{chemin} → {} ({} octets)", f.name, f.size);
        Some(archive[f.offset..f.offset + f.size].to_vec())
    }

    /// Construit un en-tête G4SK synthétique (header DÉTERMINISTE : ce qu'on garantit).
    fn build_header(bone_count: u16, file_size: u32) -> Vec<u8> {
        let mut buf = alloc::vec![0u8; 0x22];
        buf[0..4].copy_from_slice(&MAGIC_BYTES);
        buf[0x04..0x06].copy_from_slice(&0x20u16.to_le_bytes()); // header_size
        buf[0x06..0x08].copy_from_slice(&0x64u16.to_le_bytes()); // type_id
        buf[0x0C..0x10].copy_from_slice(&file_size.to_le_bytes());
        buf[0x20..0x22].copy_from_slice(&bone_count.to_le_bytes());
        buf
    }

    #[test]
    fn is_g4sk_detection() {
        assert!(is_g4sk(b"G4SK...."));
        assert!(!is_g4sk(b"G4MD"));
    }

    #[test]
    fn header_deterministe_golden() {
        let buf = build_header(42, 0x1234);
        let h = parse_header(&buf).expect("header G4SK");
        assert_eq!(h.header_size, 0x20);
        assert_eq!(h.type_id, 0x64);
        assert_eq!(h.file_size, 0x1234);
        assert_eq!(h.bone_count, 42);
    }

    #[test]
    fn rejette_petit_et_mauvais_magic() {
        assert!(matches!(
            parse_header(&[0u8; 8]),
            Err(FormatError::TooShort { .. })
        ));
        let mut buf = build_header(1, 1);
        buf[..4].copy_from_slice(b"XXXX");
        assert!(matches!(
            parse_header(&buf),
            Err(FormatError::BadMagic { .. })
        ));
    }

    // ------------------------------------------------------------------
    // Valeurs RÉELLES recoupées sur s28g001b.g4sk
    // ------------------------------------------------------------------

    #[test]
    fn header_reel_s28g001b() {
        let Some(g4sk) = g4sk_reel() else { return };
        let h = parse_header(&g4sk).expect("header G4SK réel");
        assert_eq!(&g4sk[..4], b"G4SK");
        assert_eq!(g4sk.len(), 3344, "taille du G4SK tranché");
        assert_eq!(h.header_size, 0x40);
        assert_eq!(h.type_id, 0x68);
        assert_eq!(h.bone_count, 19);
    }

    #[test]
    fn slots_offsets_reels() {
        let Some(g4sk) = g4sk_reel() else { return };
        // Adresses fichier recoupées : slot[4]=parents @0xB6C, slot[8]=noms @0xC1C.
        assert_eq!(slot_offset(&g4sk, SLOT_PARENTS), Some(0xB6C));
        assert_eq!(slot_offset(&g4sk, SLOT_NAMES), Some(0xC1C));
        assert_eq!(slot_offset(&g4sk, 0), Some(0x40)); // matrices bind-pose @0x40
    }

    #[test]
    fn hierarchie_reelle_resolue_non_heuristique() {
        let Some(g4sk) = g4sk_reel() else { return };
        let h = parse_header(&g4sk).unwrap();
        let res = parse_hierarchy(&g4sk, &h);

        // C'est la résolution réelle, PAS l'heuristique.
        assert!(
            !res.heuristic,
            "la hiérarchie réelle doit être résolue (heuristic=false)"
        );
        assert_eq!(res.bones.len(), 19);

        // Parents recoupés (sentinelle bone_count=19 normalisée en -1 pour les racines 0 et 15).
        let expected_parents: [i16; 19] =
            [-1, 0, 1, 2, 1, 1, 5, 5, 5, 5, 5, 5, 5, 5, 5, -1, 15, 16, 16];
        let got: Vec<i16> = res.bones.iter().map(|b| b.parent_index).collect();
        assert_eq!(got, expected_parents);

        // Noms recoupés.
        let expected_names = [
            "s28g001b_map",
            "bk00_s28g001b",
            "spatial_b_00",
            "range_00_1",
            "model_a_00",
            "model_r_00",
            "ao200",
            "ao241",
            "s28g001g01",
            "s28g001g02",
            "s28g001g03",
            "s28g001g04",
            "s28g001g05",
            "s28g001g06",
            "s28g001g07",
            "instance",
            "bk00",
            "ao241_bk00_01",
            "ao241_bk00_02",
        ];
        let got_names: Vec<&str> = res.bones.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(got_names, expected_names);
    }

    #[test]
    fn arbre_reel_acyclique_et_racines() {
        let Some(g4sk) = g4sk_reel() else { return };
        let h = parse_header(&g4sk).unwrap();
        let res = parse_hierarchy(&g4sk, &h);
        let parents: Vec<i16> = res.bones.iter().map(|b| b.parent_index).collect();
        assert!(
            is_acyclic_forest(&parents),
            "l'arbre d'os réel doit être acyclique"
        );
        // Deux racines réelles : os 0 (s28g001b_map) et os 15 (instance).
        let roots: Vec<usize> = res
            .bones
            .iter()
            .filter(|b| b.parent_index == -1)
            .map(|b| b.index)
            .collect();
        assert_eq!(roots, alloc::vec![0, 15]);
    }

    #[test]
    fn is_acyclic_detecte_cycle() {
        // 0->1->0 : cycle.
        assert!(!is_acyclic_forest(&[1, 0]));
        // auto-parent.
        assert!(!is_acyclic_forest(&[0]));
        // index hors limites.
        assert!(!is_acyclic_forest(&[5, -1]));
        // forêt valide.
        assert!(is_acyclic_forest(&[-1, 0, 1, -1]));
    }

    #[test]
    fn fallback_heuristique_si_table_invalide() {
        // Un buffer où les slots pointent sur du bruit non validable → fallback heuristique
        // (jamais de fabrication silencieuse d'une fausse hiérarchie « fiable »).
        let mut buf = alloc::vec![0xABu8; 0x200];
        buf[0..4].copy_from_slice(&MAGIC_BYTES);
        buf[0x04..0x06].copy_from_slice(&0x40u16.to_le_bytes());
        buf[0x20..0x22].copy_from_slice(&8u16.to_le_bytes()); // 8 os
        // slot[4] et slot[8] pointent vers des zones de 0xAB → parents hors plage → invalide.
        let h = parse_header(&buf).unwrap();
        let res = parse_hierarchy(&buf, &h);
        assert!(
            res.heuristic,
            "table invalide ⇒ retombe en heuristique, pas de fabrication"
        );
        assert_eq!(res.bones.len(), 8);
    }

    #[test]
    fn heuristique_parents_marquee_non_fiable() {
        // Place 3 parents [-1, 0, 1] à 0x1000 et vérifie que l'heuristique (fallback) les retrouve.
        let mut buf = alloc::vec![0u8; 0x1000 + 6];
        buf[0..4].copy_from_slice(&MAGIC_BYTES);
        buf[0x20..0x22].copy_from_slice(&3u16.to_le_bytes());
        buf[0x1000..0x1002].copy_from_slice(&(-1i16).to_le_bytes());
        buf[0x1002..0x1004].copy_from_slice(&0i16.to_le_bytes());
        buf[0x1004..0x1006].copy_from_slice(&1i16.to_le_bytes());

        let h = G4skHeader {
            header_size: 0x20,
            type_id: 0,
            file_size: 0,
            bone_count: 3,
        };
        let res = parse_parents_heuristic(&buf, &h);
        assert!(
            res.heuristic,
            "la hiérarchie heuristique DOIT être marquée non fiable"
        );
        assert_eq!(res.bones.len(), 3);
        assert_eq!(res.bones[0].parent_index, -1);
        assert_eq!(res.bones[1].parent_index, 0);
        assert_eq!(res.bones[2].parent_index, 1);
        assert_eq!(res.bones[0].name, "Bone_0");
    }

    #[test]
    fn find_parents_absent_renvoie_none() {
        let buf = alloc::vec![0xFFu8; 0x1000 + 8];
        let mut b2 = buf.clone();
        for x in b2.iter_mut().skip(0x1000) {
            *x = 0x7F; // 0x7F7F = 32639 ≥ bone_count(2) → invalide
        }
        assert_eq!(find_parent_indices_offset(&b2, 2), None);
    }
}
