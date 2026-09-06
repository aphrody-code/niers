//! Décodage des chunks de texture Switch **NXTCH** + déswizzle GOB Tegra X1.
//!
//! Port Rust de :
//! - `IECODE.Core/Formats/Level5/NxtchParser.cs` (`NxtchTextureFormat` L18,
//!   `CalculateTextureDataSize` L86, `NxtchHeader.IsValid` magic `0x484354584E`).
//! - `IECODE.Core/Formats/Level5/TextureSwizzler.cs` (déswizzle Block-Linear Tegra X1).
//!
//! ## Limite de validation (honnête)
//!
//! Aucun fichier NXTCH réel n'existe sur ce VPS : les deux seuls `.g4tx` disponibles
//! (`font_def.g4tx`, `gaiji_game.g4tx`) portent un payload **DDS**, pas NXTCH (vérifié :
//! le magic `4E 58 54 43 48` n'apparaît dans aucun des deux). Ce module porte donc la
//! LOGIQUE (header, mapping format→BCn, taille de mip, addressage GOB) mais ses sorties
//! ne sont validées que contre les FORMULES C#/Tegra (Ryujinx/yuzu), pas contre des
//! pixels réels IEVR. Les golden values des tests sont les tailles déterministes des
//! formules, pas un dump de jeu.
//!
//! Compatible `no_std + alloc`.

extern crate alloc;
use alloc::vec::Vec;

use crate::FormatError;

/// Magic NXTCH sur 40 bits (`"NXTCH"` = `0x484354584E`).
pub const NXTCH_MAGIC40: u64 = 0x48_4354_584E;
/// Magic NXTCH en octets.
pub const NXTCH_MAGIC_BYTES: [u8; 5] = *b"NXTCH";
/// Taille de l'en-tête NXTCH (44 octets).
pub const NXTCH_HEADER_SIZE: usize = 48;

/// Taille d'un GOB Tegra X1 (octets).
const GOB_SIZE: usize = 512;

/// Format de texture NXTCH → famille BCn / RGBA8.
///
/// Mapping exact de `NxtchTextureFormat` (NxtchParser.cs L18) et `GetTextureFormat` L67.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NxtchFormat {
    /// Inconnu / non géré.
    Unknown,
    /// `0x01` — BC1 (DXT1).
    Bc1,
    /// `0x02` — BC2 (DXT3).
    Bc2,
    /// `0x03` — BC3 (DXT5).
    Bc3,
    /// `0x04` — BC4 (ATI1).
    Bc4,
    /// `0x05` — BC5 (ATI2).
    Bc5,
    /// `0x06` — BC6H.
    Bc6,
    /// `0x07` — BC7.
    Bc7,
    /// `0x1F` — RGBA8888.
    Rgba8,
}

impl NxtchFormat {
    /// Convertit le code de format brut de l'en-tête.
    #[must_use]
    pub fn from_code(code: i32) -> Self {
        match code {
            0x01 => Self::Bc1,
            0x02 => Self::Bc2,
            0x03 => Self::Bc3,
            0x04 => Self::Bc4,
            0x05 => Self::Bc5,
            0x06 => Self::Bc6,
            0x07 => Self::Bc7,
            0x1F => Self::Rgba8,
            _ => Self::Unknown,
        }
    }

    /// Taille d'un bloc 4×4 en octets (BC1/BC4 = 8 ; BC2/3/5/6/7 = 16). 0 si non bloc.
    #[must_use]
    pub fn block_byte_size(self) -> usize {
        match self {
            Self::Bc1 | Self::Bc4 => 8,
            Self::Bc2 | Self::Bc3 | Self::Bc5 | Self::Bc6 | Self::Bc7 => 16,
            _ => 0,
        }
    }

    /// Vrai pour les formats compressés par blocs 4×4 (BCn).
    #[must_use]
    pub fn is_block_compressed(self) -> bool {
        self.block_byte_size() != 0
    }
}

/// En-tête NXTCH (48 octets). Offsets alignés 1:1 sur la struct C# `NxtchHeader`
/// (`IECODE.Core/Formats/Level5/G4txParser.cs` l.77, layout séquentiel : `Magic` u64
/// @0x00 puis 10 `int`). Les champs `UnknownN` ne sont pas portés.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NxtchHeader {
    /// `TextureDataSize` (@0x08).
    pub texture_data_size: i32,
    /// Largeur en pixels (`Width` @0x14).
    pub width: i32,
    /// Hauteur en pixels (`Height` @0x18).
    pub height: i32,
    /// Code de format (`Format` @0x24).
    pub format: i32,
    /// Nombre de mips (`MipMapCount` @0x28).
    pub mipmap_count: i32,
    /// Seconde taille des données texture (`TextureDataSize2` @0x2C).
    pub texture_data_size2: i32,
}

impl NxtchHeader {
    /// Famille de format décodée.
    #[must_use]
    pub fn texture_format(&self) -> NxtchFormat {
        NxtchFormat::from_code(self.format)
    }
}

/// Vrai si `data` commence par le magic NXTCH (5 octets).
#[must_use]
pub fn is_nxtch(data: &[u8]) -> bool {
    data.starts_with(&NXTCH_MAGIC_BYTES)
}

/// Parse un en-tête NXTCH.
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si `data` < 48 octets.
/// - [`FormatError::BadMagic`] si le magic 40 bits n'est pas NXTCH.
pub fn parse_header(data: &[u8]) -> Result<NxtchHeader, FormatError> {
    if data.len() < NXTCH_HEADER_SIZE {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: NXTCH_HEADER_SIZE,
        });
    }
    if !is_nxtch(data) {
        return Err(FormatError::BadMagic { format: "NXTCH" });
    }

    // Offsets 1:1 de la struct C# `NxtchHeader` (G4txParser.cs l.77, layout séquentiel) :
    //   Magic u64 @0x00 ; TextureDataSize @0x08 ; Unknown1 @0x0C ; Unknown2 @0x10 ;
    //   Width @0x14 ; Height @0x18 ; Unknown3 @0x1C ; Unknown4 @0x20 ; Format @0x24 ;
    //   MipMapCount @0x28 ; TextureDataSize2 @0x2C. (HEADER_SIZE = 0x30 = 48.)
    let texture_data_size = read_i32_le(data, 0x08)?;
    let width = read_i32_le(data, 0x14)?;
    let height = read_i32_le(data, 0x18)?;
    let format = read_i32_le(data, 0x24)?;
    let mipmap_count = read_i32_le(data, 0x28)?;
    let texture_data_size2 = read_i32_le(data, 0x2C)?;

    Ok(NxtchHeader {
        texture_data_size,
        width,
        height,
        format,
        mipmap_count,
        texture_data_size2,
    })
}

/// Calcule la taille totale (tous mips) du buffer texture linéaire attendu.
///
/// Port exact de `CalculateTextureDataSize` (NxtchParser.cs L86) :
///
/// - BC1/BC4 : `ceil(w/4) * ceil(h/4) * 8`.
/// - BC2/3/5/7 : `ceil(w/4) * ceil(h/4) * 16`.
/// - RGBA8 : `w * h * 4`.
///
/// BC6 n'est pas listé dans le `switch` C# → 0, comportement conservé tel quel.
#[must_use]
pub fn calculate_texture_data_size(
    format: NxtchFormat,
    width: i32,
    height: i32,
    mipmap_count: i32,
) -> usize {
    let mips = mipmap_count.max(1);
    let mut total = 0usize;
    for mip in 0..mips {
        let mw = (width >> mip).max(1) as usize;
        let mh = (height >> mip).max(1) as usize;
        total += match format {
            NxtchFormat::Bc1 | NxtchFormat::Bc4 => mw.div_ceil(4) * mh.div_ceil(4) * 8,
            NxtchFormat::Bc2 | NxtchFormat::Bc3 | NxtchFormat::Bc5 | NxtchFormat::Bc7 => {
                mw.div_ceil(4) * mh.div_ceil(4) * 16
            }
            NxtchFormat::Rgba8 => mw * mh * 4,
            // BC6 et Unknown : non couverts par le switch C# → 0.
            _ => 0,
        };
    }
    total
}

/// Déswizzle un buffer Block-Linear (tuiles GOB Tegra X1) vers linéaire.
///
/// Port de `TextureSwizzler.Unswizzle` (TextureSwizzler.cs L22). `block_size` = octets par
/// bloc 4×4 (8 ou 16 pour BCn) ou par pixel (4 pour RGBA8). `block_height_log2` = log2 de la
/// hauteur de bloc en GOBs (0-5). Les coordonnées sont en blocs pour les formats BCn, en
/// pixels pour RGBA8.
///
/// Renvoie un buffer linéaire `w_blocks * h_blocks * block_size`. Les zones source absentes
/// (buffer tronqué) sont laissées à zéro — on ne fabrique rien.
#[must_use]
pub fn unswizzle(
    data: &[u8],
    width: i32,
    height: i32,
    block_size: usize,
    block_height_log2: u32,
) -> Vec<u8> {
    let block_width = ((width as i64 + 3) / 4).max(0) as usize;
    let block_height = ((height as i64 + 3) / 4).max(0) as usize;

    let compressed = block_size >= 8;
    let w = if compressed {
        block_width
    } else {
        width.max(0) as usize
    };
    let h = if compressed {
        block_height
    } else {
        height.max(0) as usize
    };

    let mut output = alloc::vec![0u8; w.saturating_mul(h).saturating_mul(block_size)];

    for y in 0..h {
        for x in 0..w {
            let swizzled = swizzled_offset(x, y, w, block_size, block_height_log2);
            let linear = (y * w + x) * block_size;
            if swizzled + block_size <= data.len() && linear + block_size <= output.len() {
                output[linear..linear + block_size]
                    .copy_from_slice(&data[swizzled..swizzled + block_size]);
            }
        }
    }

    output
}

/// Addressage Block-Linear Tegra X1 (port de `GetSwizzledOffset`, TextureSwizzler.cs L67).
fn swizzled_offset(
    x: usize,
    y: usize,
    width: usize,
    bytes_per_unit: usize,
    block_height_log2: u32,
) -> usize {
    let image_width_in_gobs = (width * bytes_per_unit).div_ceil(64);

    let gob_x = (x * bytes_per_unit) / 64;
    let gob_y = y / 8;
    let x_in_gob = (x * bytes_per_unit) % 64;
    let y_in_gob = y % 8;

    let block_height_in_gobs = 1usize << block_height_log2;
    let gob_y_in_block = gob_y % block_height_in_gobs;

    let mut base =
        (gob_y / block_height_in_gobs) * image_width_in_gobs * block_height_in_gobs * GOB_SIZE;
    base += gob_x * block_height_in_gobs * GOB_SIZE;
    base += gob_y_in_block * GOB_SIZE;

    let gob_offset = y_in_gob * 64 + x_in_gob;
    base + gob_offset
}

fn read_i32_le(data: &[u8], off: usize) -> Result<i32, FormatError> {
    let b: [u8; 4] = data
        .get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("NXTCH : lecture i32 hors limites"))?;
    Ok(i32::from_le_bytes(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_mapping_golden() {
        assert_eq!(NxtchFormat::from_code(0x01), NxtchFormat::Bc1);
        assert_eq!(NxtchFormat::from_code(0x03), NxtchFormat::Bc3);
        assert_eq!(NxtchFormat::from_code(0x07), NxtchFormat::Bc7);
        assert_eq!(NxtchFormat::from_code(0x1F), NxtchFormat::Rgba8);
        assert_eq!(NxtchFormat::from_code(0x99), NxtchFormat::Unknown);
        assert_eq!(NxtchFormat::Bc1.block_byte_size(), 8);
        assert_eq!(NxtchFormat::Bc3.block_byte_size(), 16);
        assert!(NxtchFormat::Bc7.is_block_compressed());
        assert!(!NxtchFormat::Rgba8.is_block_compressed());
    }

    #[test]
    fn size_calc_matches_csharp_formulas() {
        // BC1 256×256, 1 mip = ceil(256/4)*ceil(256/4)*8 = 64*64*8 = 32768.
        assert_eq!(
            calculate_texture_data_size(NxtchFormat::Bc1, 256, 256, 1),
            32_768
        );
        // BC3 256×256, 1 mip = 64*64*16 = 65536.
        assert_eq!(
            calculate_texture_data_size(NxtchFormat::Bc3, 256, 256, 1),
            65_536
        );
        // RGBA8 16×16, 1 mip = 16*16*4 = 1024.
        assert_eq!(
            calculate_texture_data_size(NxtchFormat::Rgba8, 16, 16, 1),
            1024
        );
        // BC1 8×8 avec 2 mips : 8×8 (2*2*8=32) + 4×4 (1*1*8=8) = 40.
        assert_eq!(calculate_texture_data_size(NxtchFormat::Bc1, 8, 8, 2), 40);
        // Dimension non multiple de 4 : BC1 5×5 → ceil(5/4)=2 → 2*2*8 = 32.
        assert_eq!(calculate_texture_data_size(NxtchFormat::Bc1, 5, 5, 1), 32);
    }

    #[test]
    fn unswizzle_taille_coherente() {
        // BC1 64×64 → 16×16 blocs × 8 octets = 2048 octets.
        let src = alloc::vec![0u8; 4096];
        let out = unswizzle(&src, 64, 64, 8, 0);
        assert_eq!(out.len(), 16 * 16 * 8);
    }

    #[test]
    fn unswizzle_identite_simple() {
        // 1 GOB (largeur 64 octets, hauteur 8) en RGBA8 16px×8px (16*4=64 octets/ligne) :
        // avec block_height_log2=0, l'offset swizzled doit couvrir tout le GOB sans perte.
        let mut src = alloc::vec![0u8; 64 * 8];
        for (i, b) in src.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        let out = unswizzle(&src, 16, 8, 4, 0);
        assert_eq!(out.len(), 16 * 8 * 4);
        // Le pixel (0,0) provient de l'offset swizzled 0 → octets 0..4 identiques.
        assert_eq!(&out[0..4], &src[0..4]);
    }

    #[test]
    fn magic_detection() {
        assert!(is_nxtch(b"NXTCH...."));
        assert!(!is_nxtch(b"NXTC"));
        assert!(!is_nxtch(b"DDS "));
    }

    #[test]
    fn header_trop_court_rejete() {
        assert!(matches!(
            parse_header(&[0u8; 10]),
            Err(FormatError::TooShort { .. })
        ));
    }

    #[test]
    fn parse_header_offsets_alignes_sur_struct_csharp() {
        // En-tête synthétique aux offsets EXACTS de la struct C# `NxtchHeader`
        // (Magic@0x00, TextureDataSize@0x08, Width@0x14, Height@0x18, Format@0x24,
        //  MipMapCount@0x28, TextureDataSize2@0x2C ; HEADER_SIZE=48). Place des valeurs
        // distinctes aux trous `UnknownN` pour attraper tout décalage (l'ancien bug
        // off-by-4 lisait Width@0x10 = Unknown2).
        let mut h = alloc::vec![0u8; NXTCH_HEADER_SIZE];
        h[0..5].copy_from_slice(b"NXTCH");
        let put = |h: &mut alloc::vec::Vec<u8>, off: usize, v: i32| {
            h[off..off + 4].copy_from_slice(&v.to_le_bytes());
        };
        put(&mut h, 0x08, 0x4000); // TextureDataSize
        put(&mut h, 0x0C, 111); // Unknown1 (piège)
        put(&mut h, 0x10, 222); // Unknown2 (piège : l'ancien code lisait width ici)
        put(&mut h, 0x14, 256); // Width
        put(&mut h, 0x18, 128); // Height
        put(&mut h, 0x1C, 333); // Unknown3 (piège)
        put(&mut h, 0x20, 444); // Unknown4 (piège : l'ancien code lisait format ici)
        put(&mut h, 0x24, 0x03); // Format = BC3
        put(&mut h, 0x28, 5); // MipMapCount
        put(&mut h, 0x2C, 0x8000); // TextureDataSize2

        let hdr = parse_header(&h).expect("en-tête valide");
        assert_eq!(hdr.texture_data_size, 0x4000);
        assert_eq!(hdr.width, 256, "Width @0x14 (et non Unknown2=222 @0x10)");
        assert_eq!(hdr.height, 128, "Height @0x18");
        assert_eq!(hdr.format, 0x03, "Format @0x24 (et non Unknown4=444 @0x20)");
        assert_eq!(hdr.texture_format(), NxtchFormat::Bc3);
        assert_eq!(hdr.mipmap_count, 5, "MipMapCount @0x28");
        assert_eq!(hdr.texture_data_size2, 0x8000, "TextureDataSize2 @0x2C");
    }
}
