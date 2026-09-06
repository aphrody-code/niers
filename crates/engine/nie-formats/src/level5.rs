//! En-tête commun des conteneurs « métadonnées » Level-5 G4 (G4CM caméra, G4MT matériaux/motion,
//! NAVM navmesh, …) : 16 octets fixes + l'invariant `header_size + data_size == file_size`.
//!
//! Layout (validé byte sur des milliers de fichiers réels du VFS, cf. [`crate::navm`], [`crate::g4cm`],
//! [`crate::g4mt`]) :
//!
//! ```text
//!   0x00  char[4]  magic
//!   0x04  u16      header_size   (taille de l'en-tête : 0x40 pour G4CM/G4MT, 0x60 pour NAVM)
//!   0x06  u16      type_id
//!   0x08  u16      reserved0
//!   0x0A  u16      align
//!   0x0C  u32      data_size     (header_size + data_size == file_size)
//! ```
//!
//! Les formats à corps spécifique (g4tx, g4md, g4sk…) gardent leur propre parseur ; ce helper sert
//! les conteneurs dont seul l'en-tête est reversé (sémantique des données non confirmée).

use crate::FormatError;

/// Nombre d'octets de l'en-tête commun lus par [`parse_header`].
pub const COMMON_HEADER_LEN: usize = 0x10;

/// En-tête commun Level-5 (champs aux offsets fixes 0x00-0x10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Level5Header {
    /// Magic en little-endian (ex. `0x4D43_3447` = « G4CM »).
    pub magic: u32,
    pub header_size: u16,
    pub type_id: u16,
    pub reserved0: u16,
    pub align: u16,
    pub data_size: u32,
}

impl Level5Header {
    /// Invariant structurel : `header_size + data_size == file_size`.
    #[must_use]
    pub fn is_size_consistent(&self, file_size: usize) -> bool {
        self.header_size as usize + self.data_size as usize == file_size
    }
}

/// Parse l'en-tête commun (16 octets) en vérifiant le `magic_le` attendu.
///
/// # Errors
/// [`FormatError::TooShort`] si < 0x10 octets ; [`FormatError::BadMagic`] (avec `format`) si le
/// magic diffère.
pub fn parse_header(
    data: &[u8],
    magic_le: u32,
    format: &'static str,
) -> Result<Level5Header, FormatError> {
    if data.len() < COMMON_HEADER_LEN {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: COMMON_HEADER_LEN,
        });
    }
    let magic = read_u32_le(data, 0)?;
    if magic != magic_le {
        return Err(FormatError::BadMagic { format });
    }
    Ok(Level5Header {
        magic,
        header_size: read_u16_le(data, 0x04)?,
        type_id: read_u16_le(data, 0x06)?,
        reserved0: read_u16_le(data, 0x08)?,
        align: read_u16_le(data, 0x0A)?,
        data_size: read_u32_le(data, 0x0C)?,
    })
}

/// Lit un u16 little-endian borné.
///
/// # Errors
/// [`FormatError::TooShort`] si `off + 2 > data.len()`.
pub fn read_u16_le(data: &[u8], off: usize) -> Result<u16, FormatError> {
    data.get(off..off + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(FormatError::TooShort {
            got: data.len(),
            need: off + 2,
        })
}

/// Lit un u32 little-endian borné.
///
/// # Errors
/// [`FormatError::TooShort`] si `off + 4 > data.len()`.
pub fn read_u32_le(data: &[u8], off: usize) -> Result<u32, FormatError> {
    data.get(off..off + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(FormatError::TooShort {
            got: data.len(),
            need: off + 4,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_header_et_invariant() {
        let mut buf = [0u8; 0x20];
        buf[0..4].copy_from_slice(b"G4CM");
        buf[4..6].copy_from_slice(&0x40u16.to_le_bytes());
        buf[12..16].copy_from_slice(&0x20u32.to_le_bytes());
        let h = parse_header(&buf, 0x4D43_3447, "G4CM").expect("hdr");
        assert_eq!(h.header_size, 0x40);
        assert!(h.is_size_consistent(0x60)); // 0x40 + 0x20
        assert!(!h.is_size_consistent(0x40));
        assert!(matches!(
            parse_header(&buf, 0x4D56_414E, "NAVM"),
            Err(FormatError::BadMagic { .. })
        ));
        assert!(matches!(
            parse_header(b"G4", 0, "X"),
            Err(FormatError::TooShort { .. })
        ));
    }
}
