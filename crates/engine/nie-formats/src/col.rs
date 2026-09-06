//! Parseur **PXCL** — collision Level-5 (`.col`, `common/map/**/*.col`, `dx11/map/**/*.col`).
//!
//! C'est un **conteneur Level-5** (même en-tête commun que [`crate::g4cm`], cf. [`crate::level5`])
//! enveloppant des données de collision **PhysX 3.x cooked** (le footer porte une version `s03_1`/
//! `s05_2`…). **Validé byte sur 1143 `.col` réels** du VFS : magic `PXCL` + invariant
//! `header_size + data_size == file_size` (`header_size`=0x30=48, `type_id`=0x65).
//!
//! Le **conteneur** est parsé byte-exact ; l'intérieur PhysX-cooked (maillage de collision sérialisé
//! par le SDK PhysX) est **opaque** — relève du SDK PhysX, pas du RE Level-5 — donc non décodé.

use crate::FormatError;
use crate::level5::{self, Level5Header};

/// Magic « PXCL » en little-endian.
const MAGIC: u32 = 0x4C43_5850;

/// Fichier PXCL parsé : en-tête commun + taille fichier (intérieur PhysX non décodé).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pxcl {
    pub header: Level5Header,
    pub file_size: usize,
}

impl Pxcl {
    /// Invariant structurel : `header_size + data_size == file_size`.
    #[must_use]
    pub fn is_size_consistent(&self) -> bool {
        self.header.is_size_consistent(self.file_size)
    }

    /// Offset où commencent les données PhysX-cooked (après l'en-tête conteneur).
    #[must_use]
    pub fn data_offset(&self) -> usize {
        self.header.header_size as usize
    }
}

/// `true` si les 4 premiers octets sont le magic « PXCL ».
#[must_use]
pub fn is_pxcl(data: &[u8]) -> bool {
    level5::read_u32_le(data, 0).is_ok_and(|m| m == MAGIC)
}

/// Parse l'en-tête conteneur d'un `.col` (intérieur PhysX non interprété).
///
/// # Errors
/// [`FormatError::TooShort`] si < 0x10 octets, [`FormatError::BadMagic`] si le magic ≠ « PXCL ».
pub fn parse(data: &[u8]) -> Result<Pxcl, FormatError> {
    let header = level5::parse_header(data, MAGIC, "PXCL")?;
    Ok(Pxcl {
        header,
        file_size: data.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_synthetique() {
        let mut buf = [0u8; 0x30];
        buf[0..4].copy_from_slice(b"PXCL");
        buf[4..6].copy_from_slice(&0x0030u16.to_le_bytes());
        buf[6..8].copy_from_slice(&0x0065u16.to_le_bytes());
        buf[12..16].copy_from_slice(&0u32.to_le_bytes()); // 0x30 + 0 == 0x30 = file
        let c = parse(&buf).expect("parse");
        assert_eq!(c.header.magic, MAGIC);
        assert_eq!(c.header.header_size, 0x30);
        assert_eq!(c.header.type_id, 0x65);
        assert_eq!(c.data_offset(), 0x30);
        assert!(c.is_size_consistent());
    }

    #[test]
    fn rejette_magic_et_court() {
        assert!(matches!(
            parse(&[0u8; 0x30]),
            Err(FormatError::BadMagic { .. })
        ));
        assert!(matches!(parse(b"PX"), Err(FormatError::TooShort { .. })));
        assert!(is_pxcl(b"PXCL____________"));
        assert!(!is_pxcl(b"G4CM"));
    }

    /// Golden sur de VRAIS `.col` du VFS (collisions de map).
    #[cfg(feature = "real-fixtures")]
    #[test]
    fn golden_col_reels() {
        for (bytes, size) in [
            (
                include_bytes!("../tests/fixtures/col/ao111.col").as_slice(),
                1020usize,
            ),
            (
                include_bytes!("../tests/fixtures/col/k01h512.col").as_slice(),
                1604usize,
            ),
        ] {
            let c = parse(bytes).expect("col réel");
            assert_eq!(&c.header.magic.to_le_bytes(), b"PXCL");
            assert_eq!(c.header.header_size, 48);
            assert_eq!(c.header.type_id, 0x65);
            assert_eq!(c.file_size, size);
            assert!(c.is_size_consistent());
            assert_eq!(c.data_offset(), 48);
        }
    }
}
