//! Parseur **G4LA** — conteneur Level-5 d'**animation de lumière** d'événement (`.g4la`,
//! `common/event/**/*_light/eventmap_fix_*.catocfg.bin.g4la`).
//!
//! En-tête commun Level-5 (cf. [`crate::level5`]), **validé byte sur les 4 `.g4la` réels** du VFS :
//! magic `G4LA` 4/4 + invariant `header_size + data_size == file_size` 4/4 (`header_size`=0x40,
//! `type_id`=0x68, `align`=16). Le corps (pistes d'animation de lumière) n'est pas décodé faute de
//! vérité terrain (pas de parseur iecode) — en-tête byte-exact seulement, comme [`crate::g4ma`].

use crate::FormatError;
use crate::level5::{self, Level5Header};

/// Magic « G4LA » en little-endian.
const MAGIC: u32 = 0x414C_3447;
/// Taille de l'en-tête fixe Level-5 pour ce format.
const HEADER_LEN: usize = 0x40;

/// Fichier G4LA parsé : en-tête commun + taille fichier.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4la {
    pub header: Level5Header,
    pub file_size: usize,
}

impl G4la {
    /// Invariant structurel : `header_size + data_size == file_size`.
    #[must_use]
    pub fn is_size_consistent(&self) -> bool {
        self.header.is_size_consistent(self.file_size)
    }
}

/// `true` si les 4 premiers octets sont le magic « G4LA ».
#[must_use]
pub fn is_g4la(data: &[u8]) -> bool {
    level5::read_u32_le(data, 0).is_ok_and(|m| m == MAGIC)
}

/// Parse l'en-tête d'un `.g4la` (corps non interprété).
///
/// # Errors
/// [`FormatError::TooShort`] si < 0x40 octets, [`FormatError::BadMagic`] si le magic ≠ « G4LA ».
pub fn parse(data: &[u8]) -> Result<G4la, FormatError> {
    if data.len() < HEADER_LEN {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: HEADER_LEN,
        });
    }
    let header = level5::parse_header(data, MAGIC, "G4LA")?;
    Ok(G4la {
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
        buf[0..4].copy_from_slice(b"G4LA");
        buf[4..6].copy_from_slice(&0x0040u16.to_le_bytes());
        buf[6..8].copy_from_slice(&0x0068u16.to_le_bytes());
        buf[10..12].copy_from_slice(&0x0010u16.to_le_bytes());
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
        assert!(matches!(parse(b"G4LA"), Err(FormatError::TooShort { .. })));
        assert!(is_g4la(b"G4LA____"));
        assert!(!is_g4la(b"G4VS"));
    }

    /// Golden sur de VRAIS `.g4la` du VFS (animation de lumière d'événement).
    #[cfg(feature = "real-fixtures")]
    #[test]
    fn golden_g4la_reels() {
        for (bytes, size, data_size) in [
            (
                include_bytes!("../tests/fixtures/g4la/small.g4la").as_slice(),
                896usize,
                0x340u32,
            ),
            (
                include_bytes!("../tests/fixtures/g4la/med.g4la").as_slice(),
                2112usize,
                0x800u32,
            ),
        ] {
            let g = parse(bytes).expect("g4la réel");
            assert_eq!(&g.header.magic.to_le_bytes(), b"G4LA");
            assert_eq!(g.header.header_size, 64);
            assert_eq!(g.header.type_id, 0x68);
            assert_eq!(g.header.data_size, data_size);
            assert_eq!(g.file_size, size);
            assert!(g.is_size_consistent());
        }
    }
}
