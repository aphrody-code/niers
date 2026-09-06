//! Parser **p3lip** — pistes de synchronisation labiale (lip-sync) des voix IEVR.
//!
//! Les `.p3lip` vivent sous `data/common/sound/<lang>/<event>_<n>.p3lip` (20 357 fichiers,
//! le plus gros format encore non exploité du VFS — cf. `docs/FORMATS.md`). Chaque
//! fichier décrit, pour une réplique vocale, la séquence de **visèmes** (formes de bouche)
//! horodatée, à jouer en synchro avec l'`.acb`/`.awb` correspondant.
//!
//! # Format (validé byte-à-byte sur échantillons réels)
//!
//! ```text
//! 0x00  4   magic "lip\0"
//! 0x08  u32 taille du fichier (== longueur réelle)
//! 0x14  f32 durée totale en secondes
//! 0x18  u32 offset table 1 (= 112, constant ; table de paramètres, souvent nulle)
//! 0x1C  u32 (= 64, constant)
//! 0x20  u32 offset des images-clés (= 176, constant)
//! 0x24  u16 / 0x26 u16 : compteur indicatif + taille d'enregistrement (= 8)
//! 0xB0  N×8 enregistrements : [f32 temps][u8 visème][u8 canal][u16 paramètre]
//! ```
//!
//! `N = (taille - 176) / 8`. Le visème `254` marque le **début**, `255` la **fin** ; les
//! valeurs `0..=8` sont les neuf formes de bouche. Le champ `temps` croît de `0.0` à la durée.

extern crate alloc;
use alloc::vec::Vec;

use crate::FormatError;

/// Offset constant de la table d'images-clés (validé sur tous les échantillons).
const FRAMES_OFFSET: usize = 0xB0;
/// Taille d'un enregistrement d'image-clé.
const FRAME_SIZE: usize = 8;
/// Visème sentinelle de début de piste.
pub const VISEME_START: u8 = 254;
/// Visème sentinelle de fin de piste.
pub const VISEME_END: u8 = 255;

/// Une image-clé de lip-sync : un visème daté.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct LipFrame {
    /// Horodatage en secondes depuis le début de la réplique.
    pub time_s: f32,
    /// Indice de visème (forme de bouche) ; [`VISEME_START`]/[`VISEME_END`] = sentinelles.
    pub viseme: u8,
    /// Canal (constant par fichier ; rôle exact non confirmé — exposé brut).
    pub channel: u8,
    /// Paramètre 16 bits (intensité/transition présumée — exposé brut, non normalisé).
    pub param: u16,
}

impl LipFrame {
    /// `true` si l'image-clé est une sentinelle de début/fin (pas un visème jouable).
    #[must_use]
    pub fn is_sentinel(self) -> bool {
        self.viseme == VISEME_START || self.viseme == VISEME_END
    }
}

/// Une piste de lip-sync complète décodée d'un `.p3lip`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct LipSync {
    /// Durée totale de la réplique (secondes).
    pub duration_s: f32,
    /// Images-clés dans l'ordre chronologique (sentinelles incluses).
    pub frames: Vec<LipFrame>,
}

impl LipSync {
    /// Nombre de visèmes jouables (hors sentinelles début/fin).
    #[must_use]
    pub fn playable_count(&self) -> usize {
        self.frames.iter().filter(|f| !f.is_sentinel()).count()
    }
}

#[inline]
fn u32_le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
#[inline]
fn u16_le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
#[inline]
fn f32_le(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Décode un `.p3lip` en [`LipSync`].
///
/// # Errors
/// - [`FormatError::TooShort`] si le tampon est plus court que l'en-tête.
/// - [`FormatError::BadMagic`] si le magic n'est pas `lip\0`.
/// - [`FormatError::Corrupt`] si la taille déclarée est incohérente avec le contenu.
pub fn parse(data: &[u8]) -> Result<LipSync, FormatError> {
    if data.len() < FRAMES_OFFSET {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: FRAMES_OFFSET,
        });
    }
    if &data[0..4] != b"lip\0" {
        return Err(FormatError::BadMagic { format: "p3lip" });
    }
    let declared = u32_le(data, 0x08) as usize;
    if declared != data.len() {
        return Err(FormatError::Corrupt(
            "p3lip : taille déclarée != taille réelle",
        ));
    }
    let duration_s = f32_le(data, 0x14);

    let count = (data.len() - FRAMES_OFFSET) / FRAME_SIZE;
    let mut frames = Vec::with_capacity(count);
    for k in 0..count {
        let off = FRAMES_OFFSET + k * FRAME_SIZE;
        frames.push(LipFrame {
            time_s: f32_le(data, off),
            viseme: data[off + 4],
            channel: data[off + 5],
            param: u16_le(data, off + 6),
        });
    }
    Ok(LipSync { duration_s, frames })
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;

    /// Construit un p3lip synthétique minimal : en-tête + 3 images-clés (start, visème, end).
    fn synth() -> std::vec::Vec<u8> {
        let total = FRAMES_OFFSET + 3 * FRAME_SIZE; // 0xB0 + 24 = 200
        let mut b = std::vec![0u8; total];
        b[0..4].copy_from_slice(b"lip\0");
        b[0x08..0x0C].copy_from_slice(&(total as u32).to_le_bytes());
        b[0x14..0x18].copy_from_slice(&1.5f32.to_le_bytes()); // durée
        // frame 0 : start @ t=0
        let f0 = FRAMES_OFFSET;
        b[f0..f0 + 4].copy_from_slice(&0.0f32.to_le_bytes());
        b[f0 + 4] = VISEME_START;
        // frame 1 : visème 5 @ t=0.5, param 1234
        let f1 = FRAMES_OFFSET + FRAME_SIZE;
        b[f1..f1 + 4].copy_from_slice(&0.5f32.to_le_bytes());
        b[f1 + 4] = 5;
        b[f1 + 5] = 109;
        b[f1 + 6..f1 + 8].copy_from_slice(&1234u16.to_le_bytes());
        // frame 2 : end @ t=1.5
        let f2 = FRAMES_OFFSET + 2 * FRAME_SIZE;
        b[f2..f2 + 4].copy_from_slice(&1.5f32.to_le_bytes());
        b[f2 + 4] = VISEME_END;
        b
    }

    #[test]
    fn parse_synth() {
        let lip = parse(&synth()).expect("parse");
        assert!((lip.duration_s - 1.5).abs() < 1e-6);
        assert_eq!(lip.frames.len(), 3);
        assert_eq!(lip.playable_count(), 1);
        let f = lip.frames[1];
        assert_eq!(f.viseme, 5);
        assert_eq!(f.channel, 109);
        assert_eq!(f.param, 1234);
        assert!((f.time_s - 0.5).abs() < 1e-6);
        assert!(lip.frames[0].is_sentinel() && lip.frames[2].is_sentinel());
    }

    #[test]
    fn rejette_magic_invalide() {
        let mut b = synth();
        b[0] = b'X';
        assert!(matches!(parse(&b), Err(FormatError::BadMagic { .. })));
    }

    #[test]
    fn rejette_taille_incoherente() {
        let mut b = synth();
        b[0x08] = 0xFF; // taille déclarée fausse
        assert!(matches!(parse(&b), Err(FormatError::Corrupt(_))));
    }

    #[test]
    fn rejette_trop_court() {
        assert!(matches!(
            parse(&[b'l', b'i', b'p', 0]),
            Err(FormatError::TooShort { .. })
        ));
    }
}
