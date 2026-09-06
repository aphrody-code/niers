//! Parseur conteneur **DXBC** (DirectX Bytecode) — shaders compilés du jeu : `.vfxo` (vertex
//! shaders), `.pfxo` (pixel shaders), `.cfxo`/`.gfxo` (compute/geometry). **Format public Microsoft**
//! (pas du RE Level-5), donc parsé d'après sa spec et **validé byte sur 2448 `.vfxo`/`.pfxo` réels**
//! du VFS : magic « DXBC » + invariant `total_size == file_size` + chunks bornés.
//!
//! ```text
//!   0x00  char[4]  magic       "DXBC"
//!   0x04  u8[16]   digest      (somme de contrôle type MD5 du conteneur)
//!   0x14  u32      version     (1)
//!   0x18  u32      total_size  (== taille du fichier)
//!   0x1C  u32      chunk_count
//!   0x20  u32[n]   chunk_offsets
//!   …     par chunk : char[4] fourcc + u32 size + données  (RDEF/ISGN/OSGN/SHEX/STAT/…)
//! ```
//!
//! On parse le **conteneur** (chunks : fourcc/offset/taille) ; le désassemblage du bytecode shader
//! SM5 (chunk SHEX) n'est pas requis pour la lecture d'asset et n'est pas fait ici.

extern crate alloc;
use alloc::vec::Vec;

use crate::FormatError;

/// Magic « DXBC » en little-endian.
const MAGIC: u32 = 0x4342_5844;
/// Octets minimaux : en-tête (0x20) sans aucun chunk.
const MIN_LEN: usize = 0x20;

/// Un chunk du conteneur DXBC (fourcc + emplacement + taille des données).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DxbcChunk {
    /// Identifiant 4 octets (`RDEF`, `ISGN`, `OSGN`, `SHEX`, `STAT`, …).
    pub fourcc: [u8; 4],
    /// Offset des données du chunk (après son en-tête de 8 octets) dans le fichier.
    pub data_offset: usize,
    /// Taille des données du chunk en octets.
    pub size: usize,
}

/// Conteneur DXBC parsé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dxbc {
    pub digest: [u8; 16],
    pub version: u32,
    pub total_size: u32,
    pub chunks: Vec<DxbcChunk>,
    pub file_size: usize,
}

impl Dxbc {
    /// Invariant du format : `total_size == file_size`.
    #[must_use]
    pub fn is_size_consistent(&self) -> bool {
        self.total_size as usize == self.file_size
    }

    /// Cherche un chunk par son fourcc (ex. `b"SHEX"` pour le bytecode shader).
    #[must_use]
    pub fn chunk(&self, fourcc: &[u8; 4]) -> Option<&DxbcChunk> {
        self.chunks.iter().find(|c| &c.fourcc == fourcc)
    }
}

/// `true` si les 4 premiers octets sont le magic « DXBC ».
#[must_use]
pub fn is_dxbc(data: &[u8]) -> bool {
    read_u32(data, 0).is_ok_and(|m| m == MAGIC)
}

/// Parse le conteneur DXBC : en-tête + table de chunks (fourcc/offset/taille), tout borné.
///
/// # Errors
/// [`FormatError::TooShort`] si < 0x20 octets ; [`FormatError::BadMagic`] si ≠ « DXBC » ;
/// [`FormatError::Corrupt`] si la table de chunks ou un chunk sort des limites du fichier.
pub fn parse(data: &[u8]) -> Result<Dxbc, FormatError> {
    if data.len() < MIN_LEN {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: MIN_LEN,
        });
    }
    if !is_dxbc(data) {
        return Err(FormatError::BadMagic { format: "DXBC" });
    }
    let mut digest = [0u8; 16];
    digest.copy_from_slice(&data[4..20]);
    let version = read_u32(data, 0x14)?;
    let total_size = read_u32(data, 0x18)?;
    let chunk_count = read_u32(data, 0x1C)? as usize;

    // La table d'offsets (chunk_count × u32) doit tenir juste après l'en-tête.
    let table_end = 0x20usize
        .checked_add(
            chunk_count
                .checked_mul(4)
                .ok_or(FormatError::Corrupt("dxbc chunk_count overflow"))?,
        )
        .ok_or(FormatError::Corrupt("dxbc table overflow"))?;
    if table_end > data.len() {
        return Err(FormatError::Corrupt("dxbc: table de chunks hors limites"));
    }

    let mut chunks = Vec::with_capacity(chunk_count.min(64));
    for i in 0..chunk_count {
        let off = read_u32(data, 0x20 + i * 4)? as usize;
        // en-tête de chunk : fourcc(4) + size(4)
        if off.checked_add(8).is_none_or(|e| e > data.len()) {
            return Err(FormatError::Corrupt("dxbc: en-tête de chunk hors limites"));
        }
        let mut fourcc = [0u8; 4];
        fourcc.copy_from_slice(&data[off..off + 4]);
        let size = read_u32(data, off + 4)? as usize;
        let data_offset = off + 8;
        if data_offset.checked_add(size).is_none_or(|e| e > data.len()) {
            return Err(FormatError::Corrupt("dxbc: données de chunk hors limites"));
        }
        chunks.push(DxbcChunk {
            fourcc,
            data_offset,
            size,
        });
    }

    Ok(Dxbc {
        digest,
        version,
        total_size,
        chunks,
        file_size: data.len(),
    })
}

fn read_u32(data: &[u8], off: usize) -> Result<u32, FormatError> {
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
    fn parse_synthetique() {
        // 1 chunk "TEST" de 4 octets de données.
        let mut buf = alloc::vec![0u8; 0x20 + 4 + 8 + 4];
        buf[0..4].copy_from_slice(b"DXBC");
        let total = buf.len() as u32;
        buf[0x18..0x1C].copy_from_slice(&total.to_le_bytes());
        buf[0x1C..0x20].copy_from_slice(&1u32.to_le_bytes()); // 1 chunk
        let chunk_off = 0x24u32; // juste après la table d'offsets (1 × u32)
        buf[0x20..0x24].copy_from_slice(&chunk_off.to_le_bytes());
        buf[0x24..0x28].copy_from_slice(b"TEST");
        buf[0x28..0x2C].copy_from_slice(&4u32.to_le_bytes());
        let d = parse(&buf).expect("parse");
        assert!(d.is_size_consistent());
        assert_eq!(d.chunks.len(), 1);
        assert_eq!(&d.chunks[0].fourcc, b"TEST");
        assert_eq!(d.chunks[0].size, 4);
        assert!(d.chunk(b"TEST").is_some());
        assert!(d.chunk(b"SHEX").is_none());
    }

    #[test]
    fn rejette_magic_court_et_corrompu() {
        assert!(matches!(
            parse(&[0u8; 0x20]),
            Err(FormatError::BadMagic { .. })
        ));
        assert!(matches!(parse(b"DXBC"), Err(FormatError::TooShort { .. })));
        assert!(is_dxbc(b"DXBC____________________________"));
        // chunk_count énorme → table hors limites = Corrupt (pas de panic).
        let mut buf = [0u8; 0x20];
        buf[0..4].copy_from_slice(b"DXBC");
        buf[0x1C..0x20].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert!(matches!(parse(&buf), Err(FormatError::Corrupt(_))));
    }

    /// Golden sur de VRAIS shaders du VFS (vertex `.vfxo` + pixel `.pfxo`). Vérifie magic, invariant
    /// de taille, et les chunks attendus d'un shader SM5 (RDEF/ISGN/OSGN/SHEX/STAT).
    #[cfg(feature = "real-fixtures")]
    #[test]
    fn golden_dxbc_reels() {
        for (bytes, size) in [
            (
                include_bytes!("../tests/fixtures/dxbc/sample.vfxo").as_slice(),
                21656usize,
            ),
            (
                include_bytes!("../tests/fixtures/dxbc/sample.pfxo").as_slice(),
                20180usize,
            ),
            // .cfxo (compute/effect shader) = aussi du DXBC : même conteneur, mêmes chunks.
            (
                include_bytes!("../tests/fixtures/dxbc/sample.cfxo").as_slice(),
                8052usize,
            ),
        ] {
            let d = parse(bytes).expect("dxbc réel");
            assert_eq!(d.file_size, size);
            assert!(d.is_size_consistent(), "total_size == file_size");
            assert_eq!(d.chunks.len(), 5);
            assert!(d.chunk(b"SHEX").is_some(), "bytecode shader SM5");
            assert!(d.chunk(b"RDEF").is_some(), "définitions de ressources");
            assert!(d.chunk(b"ISGN").is_some(), "signature d'entrée");
        }
    }
}
