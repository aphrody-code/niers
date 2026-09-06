//! `FontColorData` — port Rust de `common/font/font_color.cfg.bin` (Level-5 IEVR).
//!
//! ## Quoi
//!
//! La **palette de texte du jeu** : chaque couleur utilisable par le moteur de texte, désignée
//! par un `fontColorId` (CRC-32 d'un nom), avec deux triplets RVB —
//!
//! - `red` / `green` / `blue` : la couleur du texte principal ;
//! - `rubiRed` / `rubiGreen` / `rubiBlue` : celle des *rubis* (les furigana affichés au-dessus
//!   des kanji). Les deux diffèrent souvent : un texte blanc peut porter des rubis jaunes.
//!
//! ## Vérité terrain
//!
//! Dump réel : `data/common/font/font_color.cfg.bin.json`, format **`lists`**, une seule liste
//! `m_FontColorDataList` (`typeName` = `FONT_COLOR`) de 70 entrées.
//!
//! Les composantes sont des entiers 0..255 dans les fichiers observés ; elles sont lues telles
//! quelles et bornées à cet intervalle, sans conversion d'espace colorimétrique — le moteur les
//! consomme en RVB8.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_i64, list_values};
use crate::hash::HashId;

/// Une couleur de la palette de texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FontColor {
    /// `fontColorId` — hash du nom de la couleur.
    pub id: HashId,
    /// Couleur du texte, `(r, g, b)` en 0..255.
    pub rgb: (u8, u8, u8),
    /// Couleur des rubis (furigana), `(r, g, b)` en 0..255.
    pub rubi_rgb: (u8, u8, u8),
}

impl FontColor {
    /// Rend la couleur du texte en hexadécimal CSS (`#RRGGBB`).
    #[must_use]
    pub fn hex(self) -> alloc::string::String {
        let (r, g, b) = self.rgb;
        alloc::format!("#{r:02X}{g:02X}{b:02X}")
    }

    /// Rend la couleur des rubis en hexadécimal CSS.
    #[must_use]
    pub fn rubi_hex(self) -> alloc::string::String {
        let (r, g, b) = self.rubi_rgb;
        alloc::format!("#{r:02X}{g:02X}{b:02X}")
    }

    fn from_value(v: &Value) -> Self {
        // Une composante absente vaut 0 ; une valeur hors 0..255 est bornée plutôt que tronquée
        // par transtypage, qui replierait 256 en 0.
        let comp = |cle: &str| -> u8 {
            let n = field_i64(v, cle).unwrap_or(0);
            u8::try_from(n.clamp(0, 255)).unwrap_or(0)
        };
        Self {
            id: crate::cfgbin::field_hash(v, "fontColorId"),
            rgb: (comp("red"), comp("green"), comp("blue")),
            rubi_rgb: (comp("rubiRed"), comp("rubiGreen"), comp("rubiBlue")),
        }
    }
}

/// Parse `font_color.cfg.bin.json` → la palette de texte, dans l'ordre du fichier.
#[must_use]
pub fn parse_font_colors(root: &Value) -> Vec<FontColor> {
    list_values(root, "m_FontColorDataList").map_or_else(Vec::new, |vs| {
        vs.iter().map(FontColor::from_value).collect()
    })
}

/// Retrouve une couleur par son identifiant.
#[must_use]
pub fn find_color(palette: &[FontColor], id: HashId) -> Option<&FontColor> {
    palette.iter().find(|c| c.id == id)
}
