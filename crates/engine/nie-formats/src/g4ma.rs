//! Parseur **G4MA** — **animation de matériau** Level-5 (`.g4ma`), 35 fichiers au VFS + sous-table
//! des archives `.g4pk` (avec G4MT, cf. les anims de matériau de menu reversées en D1.a).
//!
//! En-tête commun Level-5 (cf. [`crate::level5`]), **validé byte sur les 35 `.g4ma` réels** du VFS :
//! magic `G4MA` + invariant `header_size + data_size == file_size` (`header_size`=0x40,
//! `type_id`=0x68 — même famille que [`crate::g4cm`]/[`crate::g4mt`]). Corps (clés d'animation de
//! matériau) non décodé faute de vérité terrain — en-tête byte-exact seulement.

use crate::FormatError;
use crate::level5::{self, Level5Header};

/// Magic « G4MA » en little-endian.
const MAGIC: u32 = 0x414D_3447;
/// Taille de l'en-tête fixe Level-5 pour ce format.
const HEADER_LEN: usize = 0x40;

/// Fichier G4MA parsé : en-tête commun + taille fichier.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4ma {
    pub header: Level5Header,
    pub file_size: usize,
}

impl G4ma {
    /// Invariant structurel : `header_size + data_size == file_size`.
    #[must_use]
    pub fn is_size_consistent(&self) -> bool {
        self.header.is_size_consistent(self.file_size)
    }
}

/// `true` si les 4 premiers octets sont le magic « G4MA ».
#[must_use]
pub fn is_g4ma(data: &[u8]) -> bool {
    level5::read_u32_le(data, 0).is_ok_and(|m| m == MAGIC)
}

/// Parse l'en-tête d'un `.g4ma` (corps non interprété).
///
/// # Errors
/// [`FormatError::TooShort`] si < 0x40 octets, [`FormatError::BadMagic`] si le magic ≠ « G4MA ».
pub fn parse(data: &[u8]) -> Result<G4ma, FormatError> {
    if data.len() < HEADER_LEN {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: HEADER_LEN,
        });
    }
    let header = level5::parse_header(data, MAGIC, "G4MA")?;
    Ok(G4ma {
        header,
        file_size: data.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_synthetique() {
        let mut buf = [0u8; HEADER_LEN];
        buf[0..4].copy_from_slice(b"G4MA");
        buf[4..6].copy_from_slice(&0x0040u16.to_le_bytes());
        buf[6..8].copy_from_slice(&0x0068u16.to_le_bytes());
        buf[12..16].copy_from_slice(&0u32.to_le_bytes());
        let g = parse(&buf).expect("parse");
        assert_eq!(g.header.magic, MAGIC);
        assert_eq!(g.header.header_size, 0x40);
        assert_eq!(g.header.type_id, 0x68);
        assert!(g.is_size_consistent());
    }

    #[test]
    fn rejette_magic_et_court() {
        assert!(matches!(
            parse(&[0u8; HEADER_LEN]),
            Err(FormatError::BadMagic { .. })
        ));
        assert!(matches!(parse(b"G4MA"), Err(FormatError::TooShort { .. })));
        assert!(is_g4ma(b"G4MA____"));
        assert!(!is_g4ma(b"G4MT"));
    }

    /// Golden sur de VRAIS `.g4ma` du VFS (anims de matériau).
    #[cfg(feature = "real-fixtures")]
    #[test]
    fn golden_g4ma_reels() {
        for bytes in [
            include_bytes!("../tests/fixtures/g4ma/f0.g4ma").as_slice(),
            include_bytes!("../tests/fixtures/g4ma/f1.g4ma").as_slice(),
        ] {
            let g = parse(bytes).expect("g4ma réel");
            assert_eq!(&g.header.magic.to_le_bytes(), b"G4MA");
            assert_eq!(g.header.header_size, 64);
            assert_eq!(g.header.type_id, 0x68);
            assert!(g.is_size_consistent());
        }
    }
}
