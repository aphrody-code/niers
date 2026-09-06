//! Lecteurs d'archives Level-5 G4PK / G4RA.
//!
//! Port Rust de :
//! - `IECODE.Core/Formats/Level5/G4pkParser.cs` (`G4pkHeader` L18-40, `ParseFiles` L81 :
//!   séquence offsetTable → sizeTable → hashTable → stringTable).
//! - `IECODE.Core/Formats/Level5/G4raParser.cs` (magic G4RA `0x41523447`).
//!
//! Magics confirmés dans `GameFileType.cs` : `Magic_G4PK=0x4B503447`, `Magic_G4RA=0x41523447`.
//! Discriminant G4PK@/G4PKM (`GameFileType.cs` Detect L185-191) : 5e octet `'M'` (0x4D) ⇒ paquet
//! menu (`G4PKM`), sinon archive standard.
//!
//! ## En-tête G4PK (0x40 octets)
//!
//! ```text
//! 0x00 u32  Magic "G4PK" (LE 0x4B503447)
//! 0x04 i16  HeaderSize (0x40)
//! 0x06 i16  FileType   (0x64)
//! 0x08 i32  Version
//! 0x0C i32  ContentSize
//! 0x20 i32  FileCount
//! 0x24 i16  Table2EntryCount (hashes)
//! 0x26 i16  Table3EntryCount (strings)
//! ```
//!
//! Tables séquentielles depuis `header_size` : offsets (file_count × i32, décalés `<< 2`),
//! tailles (file_count × i32), hashes (table2_entry_count × u32), puis table de chaînes.
//!
//! ## Validation
//!
//! Aucun `.g4pk`/`.g4ra` isolé n'est présent sur le VPS (ils vivent dans les `.cpk`). La logique
//! de table est validée par un **fixture synthétique** respectant l'en-tête 0x40 + offsetTable +
//! sizeTable + hashTable + stringTable, conformément à la consigne du livrable.
//!
//! Compatible `no_std + alloc`.

extern crate alloc;
use alloc::{string::String, vec::Vec};

use crate::FormatError;

/// Magic « G4PK » little-endian (`0x4B503447`).
pub const G4PK_MAGIC: u32 = 0x4B50_3447;
/// Magic « G4PK » en octets.
pub const G4PK_MAGIC_BYTES: [u8; 4] = *b"G4PK";
/// Magic « G4RA » little-endian (`0x41523447`).
pub const G4RA_MAGIC: u32 = 0x4152_3447;
/// Magic « G4RA » en octets.
pub const G4RA_MAGIC_BYTES: [u8; 4] = *b"G4RA";

const HEADER_SIZE: usize = 0x40;

/// Variante de paquet G4PK selon le 5e octet (`GameFileType.cs` Detect L185-191).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum G4pkKind {
    /// Archive standard (5e octet ≠ 'M').
    Archive,
    /// Paquet menu (5e octet = 'M' / 0x4D).
    Menu,
}

/// En-tête G4PK.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4pkHeader {
    /// Taille de l'en-tête (0x40).
    pub header_size: i16,
    /// Type de fichier (0x64).
    pub file_type: i16,
    /// Version.
    pub version: i32,
    /// Taille du contenu après l'en-tête.
    pub content_size: i32,
    /// Nombre de sous-fichiers.
    pub file_count: i32,
    /// Nombre d'entrées de hash (table 2).
    pub table2_entry_count: i16,
    /// Nombre d'entrées de chaînes (table 3).
    pub table3_entry_count: i16,
    /// Variante (archive / menu).
    pub kind: G4pkKind,
}

/// Sous-fichier d'une archive G4PK.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4pkFile {
    /// Nom du sous-fichier (résolu depuis la table de chaînes).
    pub name: String,
    /// Offset absolu dans le tampon (`header_size + (raw_offset << 2)`).
    pub offset: usize,
    /// Taille en octets.
    pub size: usize,
    /// Hash CRC32 (0 si hors table).
    pub hash: u32,
}

/// Archive G4PK parsée.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4pk {
    /// En-tête.
    pub header: G4pkHeader,
    /// Sous-fichiers.
    pub files: Vec<G4pkFile>,
}

/// Vrai si `data` commence par le magic G4PK.
#[must_use]
pub fn is_g4pk(data: &[u8]) -> bool {
    data.starts_with(&G4PK_MAGIC_BYTES)
}

/// Vrai si `data` commence par le magic G4RA.
#[must_use]
pub fn is_g4ra(data: &[u8]) -> bool {
    data.starts_with(&G4RA_MAGIC_BYTES)
}

/// Discrimine la variante d'un paquet G4PK par son 5e octet (`'M'` ⇒ menu).
#[must_use]
pub fn g4pk_kind(data: &[u8]) -> G4pkKind {
    if data.get(4) == Some(&b'M') {
        G4pkKind::Menu
    } else {
        G4pkKind::Archive
    }
}

/// Parse une archive G4PK.
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si `data` < 0x40 octets.
/// - [`FormatError::BadMagic`] si le magic n'est pas G4PK.
/// - [`FormatError::Corrupt`] si une table déborde du tampon.
pub fn parse(data: &[u8]) -> Result<G4pk, FormatError> {
    if data.len() < HEADER_SIZE {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: HEADER_SIZE,
        });
    }
    if !is_g4pk(data) {
        return Err(FormatError::BadMagic { format: "G4PK" });
    }

    let header = G4pkHeader {
        header_size: read_i16(data, 0x04)?,
        file_type: read_i16(data, 0x06)?,
        version: read_i32(data, 0x08)?,
        content_size: read_i32(data, 0x0C)?,
        file_count: read_i32(data, 0x20)?,
        table2_entry_count: read_i16(data, 0x24)?,
        table3_entry_count: read_i16(data, 0x26)?,
        kind: g4pk_kind(data),
    };

    let file_count = header.file_count.max(0) as usize;
    let table2 = header.table2_entry_count.max(0) as usize;
    let table3 = header.table3_entry_count.max(0) as usize;
    let hs = header.header_size.max(0) as usize;

    // 1) offsetTable (file_count × i32).
    let offset_table = hs;
    // 2) sizeTable (file_count × i32).
    let size_table = offset_table + file_count * 4;
    // 3) hashTable (table2 × u32).
    let hash_table = size_table + file_count * 4;
    // 4) table d'ids inconnus (table3 octets), puis offsets de chaînes alignés sur 4.
    let unk_table = hash_table + table2 * 4;
    let string_offset_pos = (unk_table + table3 + 3) & !3;
    let string_data_pos = string_offset_pos;

    let raw_offsets = read_i32_array(data, offset_table, file_count)?;
    let sizes = read_i32_array(data, size_table, file_count)?;
    let hashes = read_u32_array(data, hash_table, table2)?;

    // string offsets : i16, table3/2 entrées (port C# : stringOffsets.Length = table3/2).
    let string_offsets = read_i16_array(data, string_offset_pos, table3 / 2)?;

    let mut files = Vec::with_capacity(file_count);
    for i in 0..file_count {
        let name = match string_offsets.get(i) {
            Some(&so) => {
                let name_pos = string_data_pos
                    .checked_add(so as usize)
                    .ok_or(FormatError::Corrupt("G4PK : overflow offset de nom"))?;
                read_cstr(data, name_pos)
            }
            None => String::new(),
        };
        let offset = hs + ((raw_offsets[i] as usize) << 2);
        files.push(G4pkFile {
            name,
            offset,
            size: sizes[i].max(0) as usize,
            hash: hashes.get(i).copied().unwrap_or(0),
        });
    }

    Ok(G4pk { header, files })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_i32_array(data: &[u8], off: usize, count: usize) -> Result<Vec<i32>, FormatError> {
    let mut v = Vec::with_capacity(count);
    for i in 0..count {
        v.push(read_i32(data, off + i * 4)?);
    }
    Ok(v)
}

fn read_u32_array(data: &[u8], off: usize, count: usize) -> Result<Vec<u32>, FormatError> {
    let mut v = Vec::with_capacity(count);
    for i in 0..count {
        v.push(read_u32(data, off + i * 4)?);
    }
    Ok(v)
}

fn read_i16_array(data: &[u8], off: usize, count: usize) -> Result<Vec<i16>, FormatError> {
    let mut v = Vec::with_capacity(count);
    for i in 0..count {
        v.push(read_i16(data, off + i * 2)?);
    }
    Ok(v)
}

fn read_cstr(data: &[u8], abs: usize) -> String {
    let slice = data.get(abs..).unwrap_or(&[]);
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).into_owned()
}

fn read_i16(data: &[u8], off: usize) -> Result<i16, FormatError> {
    let b: [u8; 2] = data
        .get(off..off + 2)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("G4PK : lecture i16 hors limites"))?;
    Ok(i16::from_le_bytes(b))
}

fn read_i32(data: &[u8], off: usize) -> Result<i32, FormatError> {
    let b: [u8; 4] = data
        .get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("G4PK : lecture i32 hors limites"))?;
    Ok(i32::from_le_bytes(b))
}

fn read_u32(data: &[u8], off: usize) -> Result<u32, FormatError> {
    let b: [u8; 4] = data
        .get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("G4PK : lecture u32 hors limites"))?;
    Ok(u32::from_le_bytes(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfgbin::crc32;

    /// Construit une archive G4PK synthétique à 2 fichiers ("a.g4md", "b.g4tx"), respectant
    /// l'en-tête 0x40 + offsetTable + sizeTable + hashTable + stringTable.
    fn build_archive() -> Vec<u8> {
        let file_count = 2usize;
        let table2 = 2usize; // hashes
        let table3 = 4usize; // 2 string offsets (i16) ⇒ table3/2 = 2

        let offset_table = HEADER_SIZE;
        let size_table = offset_table + file_count * 4;
        let hash_table = size_table + file_count * 4;
        let unk_table = hash_table + table2 * 4;
        let string_offset_pos = (unk_table + table3 + 3) & !3;
        // string data = string_offset_pos ; place 2 offsets i16 puis les noms.
        let names_pos = string_offset_pos + 2 * 2;

        let name_a = b"a.g4md\0";
        let name_b = b"b.g4tx\0";
        // Les offsets de chaînes sont relatifs à string_data_pos (= string_offset_pos). Les noms
        // sont placés APRÈS les 2 offsets i16 (donc à +4) → off_a = 4, off_b = 4 + len(name_a).
        let off_a = (2 * 2) as i16;
        let off_b = (2 * 2 + name_a.len()) as i16;

        // Données fichiers placées après les chaînes. Les offsets G4PK sont codés `>> 2`, donc
        // chaque payload doit commencer à une frontière de 4 octets relative à header_size.
        let data_a = b"DATA-A";
        let data_b = b"DATA-BB";
        let payload_start = (names_pos + name_a.len() + name_b.len() + 0xF) & !0xF;
        // Offset 4-aligné pour le second fichier (relatif à header_size).
        let data_b_start = (payload_start + data_a.len() + 3) & !3;

        let total = data_b_start + data_b.len();
        let mut buf = alloc::vec![0u8; total];

        // En-tête.
        buf[0..4].copy_from_slice(&G4PK_MAGIC_BYTES);
        buf[0x04..0x06].copy_from_slice(&(HEADER_SIZE as i16).to_le_bytes());
        buf[0x06..0x08].copy_from_slice(&0x64i16.to_le_bytes());
        buf[0x20..0x24].copy_from_slice(&(file_count as i32).to_le_bytes());
        buf[0x24..0x26].copy_from_slice(&(table2 as i16).to_le_bytes());
        buf[0x26..0x28].copy_from_slice(&(table3 as i16).to_le_bytes());

        // offsetTable : offsets relatifs >> 2 (donc (abs - header_size) >> 2), 4-alignés.
        let raw_a = ((payload_start - HEADER_SIZE) >> 2) as i32;
        let raw_b = ((data_b_start - HEADER_SIZE) >> 2) as i32;
        buf[offset_table..offset_table + 4].copy_from_slice(&raw_a.to_le_bytes());
        buf[offset_table + 4..offset_table + 8].copy_from_slice(&raw_b.to_le_bytes());

        // sizeTable.
        buf[size_table..size_table + 4].copy_from_slice(&(data_a.len() as i32).to_le_bytes());
        buf[size_table + 4..size_table + 8].copy_from_slice(&(data_b.len() as i32).to_le_bytes());

        // hashTable.
        buf[hash_table..hash_table + 4].copy_from_slice(&crc32(b"a.g4md").to_le_bytes());
        buf[hash_table + 4..hash_table + 8].copy_from_slice(&crc32(b"b.g4tx").to_le_bytes());

        // stringOffsets.
        buf[string_offset_pos..string_offset_pos + 2].copy_from_slice(&off_a.to_le_bytes());
        buf[string_offset_pos + 2..string_offset_pos + 4].copy_from_slice(&off_b.to_le_bytes());

        // Noms.
        buf[names_pos..names_pos + name_a.len()].copy_from_slice(name_a);
        buf[names_pos + name_a.len()..names_pos + name_a.len() + name_b.len()]
            .copy_from_slice(name_b);

        // Payloads (4-alignés).
        buf[payload_start..payload_start + data_a.len()].copy_from_slice(data_a);
        buf[data_b_start..data_b_start + data_b.len()].copy_from_slice(data_b);

        buf
    }

    #[test]
    fn magics_detection() {
        assert!(is_g4pk(b"G4PK...."));
        assert!(is_g4ra(b"G4RA...."));
        assert!(!is_g4pk(b"G4RA"));
        assert_eq!(g4pk_kind(b"G4PKM..."), G4pkKind::Menu);
        assert_eq!(g4pk_kind(b"G4PK@..."), G4pkKind::Archive);
    }

    #[test]
    fn parse_archive_golden() {
        let buf = build_archive();
        let pk = parse(&buf).expect("parse G4PK synthétique");
        assert_eq!(pk.header.file_count, 2);
        assert_eq!(pk.header.header_size, 0x40);
        assert_eq!(pk.files.len(), 2);

        // Noms résolus + tailles + hashes.
        assert_eq!(pk.files[0].name, "a.g4md");
        assert_eq!(pk.files[0].size, 6); // "DATA-A"
        assert_eq!(pk.files[0].hash, crc32(b"a.g4md"));
        assert_eq!(pk.files[1].name, "b.g4tx");
        assert_eq!(pk.files[1].size, 7); // "DATA-BB"
        assert_eq!(pk.files[1].hash, crc32(b"b.g4tx"));

        // Le payload extrait au {offset, size} doit correspondre.
        let f0 = &pk.files[0];
        assert_eq!(&buf[f0.offset..f0.offset + f0.size], b"DATA-A");
        let f1 = &pk.files[1];
        assert_eq!(&buf[f1.offset..f1.offset + f1.size], b"DATA-BB");
    }

    #[test]
    fn rejette_petit_et_mauvais_magic() {
        assert!(matches!(
            parse(&[0u8; 8]),
            Err(FormatError::TooShort { .. })
        ));
        let mut buf = alloc::vec![0u8; HEADER_SIZE];
        buf[..4].copy_from_slice(b"XXXX");
        assert!(matches!(parse(&buf), Err(FormatError::BadMagic { .. })));
    }

    /// Golden VFS : `s28g001b.g4pk` contient un unique `s28g001b.g4sk` de 3 344 octets.
    ///
    /// Valeurs reprises de `G4pkParserTests.cs`, qui les lisait dans un dossier de fixtures
    /// codé en dur hors dépôt (`/home/ubuntu/…/_g4pk_fixtures`) et sautait donc partout
    /// ailleurs. Ici la source est le VFS du jeu : le test s'exécute pour de bon.
    #[test]
    fn golden_g4pk_s28g001b_un_seul_g4sk() {
        let Some((chemin, data)) = super::tests_vfs::lire_par_suffixe("s28g001b.g4pk") else {
            return;
        };

        let pk = parse(&data).expect("parse s28g001b.g4pk");
        assert_eq!(pk.files.len(), 1, "{chemin} : un seul sous-fichier attendu");

        let f = &pk.files[0];
        assert_eq!(f.name, "s28g001b.g4sk");
        assert_eq!(f.size, 3344, "taille du sous-fichier");
        assert_eq!(f.hash, 0x940E_596D, "hash du sous-fichier");
        // Le hash de la table EST le hash Level-5 du nom.
        assert_eq!(f.hash, crc32(f.name.as_bytes()));
        // Parité avec `DetectSubFormat` du C# : le sous-format se lit au magic de la tranche.
        assert_eq!(&data[f.offset..f.offset + 4], b"G4SK");
    }
}

/// Accès au VFS pour les goldens de ce crate — factorisé ici parce que `g4pk`, `g4sk` et `g4mg`
/// en ont tous besoin, et qu'un golden qui saute doit le **dire**.
#[cfg(test)]
pub(crate) mod tests_vfs {
    use std::string::String;
    use std::vec::Vec;

    /// Monte le VFS et renvoie `(chemin, contenu)` du premier fichier dont le chemin se termine
    /// par `suffixe`. Annonce le saut sur stderr et renvoie `None` si le jeu ou le fichier
    /// manque — jamais d'échec silencieux.
    pub(crate) fn lire_par_suffixe(suffixe: &str) -> Option<(String, Vec<u8>)> {
        use crate::vfs::Vfs;
        use std::path::Path;

        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = Path::new(&dir).join("data");

        let mut vfs = Vfs::new();
        if vfs.init(&data_dir).is_err() {
            std::eprintln!("skip {suffixe} : jeu absent à {}", data_dir.display());
            return None;
        }
        let chemin = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .find(|p| p.ends_with(suffixe));
        let Some(chemin) = chemin else {
            std::eprintln!("skip {suffixe} : non trouvé dans le VFS");
            return None;
        };
        let data = vfs.read(&chemin).ok()?;
        Some((chemin, data))
    }
}
