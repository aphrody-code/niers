//! `playstyle` — style de jeu d'un personnage (`playStyle`, **6ᵉ variable `Int`** d'un noeud
//! `CHARA_PARAM_INFO`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/playstyle.ts`
//!   (`parsePlaystyleNode`, `parseAllPlaystyles`, tables `PLAYSTYLE_EN`/`PLAYSTYLE_FR`,
//!   `playstyleIdToEn`/`playstyleIdToFr`) — **port 1:1**.
//! - Données : `data/common/gamedata/character/chara_param_1.03.66.00.cfg.bin` (VFS IEVR).
//!   **6166** noeuds `CHARA_PARAM_INFO` valides (≥ 6 entiers), distribution `playStyle` mesurée
//!   en live `{0:1055, 1:1034, 2:1059, 3:1003, 4:989, 5:1026}` — couvre **exactement** `0..=5`
//!   (aucune valeur hors plage), ce qui valide les 6 libellés.
//!
//! ## Champ `playStyle`
//!
//! Le champ vit à l'**index 5 des variables de type `Int`** du noeud (juste après
//! `mainPosition` = index 3 et `subPosition` = index 4 ; cf. enregistrement RDBN `nie.c`,
//! `playStyle` = `T2B[3 + 2]`). On collecte **uniquement** les variables `Int` (comme inagle),
//! puis on lit l'index 5 ; un noeud avec moins de 6 entiers est rejeté (`None`).
//!
//! Libellés (source : `menu_text.cfg.bin` `TEXT_INFO_1812-1817`, repris tels quels d'inagle) :
//!
//! | id | EN          | FR           |
//! |----|-------------|--------------|
//! | 0  | Counter     | Contre       |
//! | 1  | Bond        | Lien         |
//! | 2  | Tension     | Tension      |
//! | 3  | Rough Play  | Jeu violent  |
//! | 4  | Justice     | Justice      |
//! | 5  | Freedom     | Liberté      |

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

/// Libellé **anglais** d'un identifiant de style de jeu, ou `None` hors `0..=5`.
///
/// Port de `PLAYSTYLE_EN` ; inagle retombe sur `"PlayStyle{id}"` pour l'inconnu — on renvoie
/// `None` (les données réelles n'observent que `0..=5`, cf. distribution mesurée).
#[must_use]
pub fn playstyle_id_to_en(id: i64) -> Option<&'static str> {
    Some(match id {
        0 => "Counter",
        1 => "Bond",
        2 => "Tension",
        3 => "Rough Play",
        4 => "Justice",
        5 => "Freedom",
        _ => return None,
    })
}

/// Libellé **français** d'un identifiant de style de jeu, ou `None` hors `0..=5`.
///
/// Port de `PLAYSTYLE_FR`.
#[must_use]
pub fn playstyle_id_to_fr(id: i64) -> Option<&'static str> {
    Some(match id {
        0 => "Contre",
        1 => "Lien",
        2 => "Tension",
        3 => "Jeu violent",
        4 => "Justice",
        5 => "Liberté",
        _ => return None,
    })
}

/// Style de jeu d'un personnage, porté d'un noeud `CHARA_PARAM_INFO`.
///
/// Pendant Rust de l'interface `PlaystyleEntry` d'inagle. Les libellés `playStyleEn`/
/// `playStyleFr` d'inagle sont dérivables à la demande via [`PlaystyleEntry::label_en`] /
/// [`PlaystyleEntry::label_fr`] (on ne stocke que l'identifiant brut).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaystyleEntry {
    /// `values[0]` — identifiant du `chara_param` (Int signé → hash non signé, `toHex`).
    pub chara_param_id: HashId,
    /// `values[5]` — identifiant du style de jeu (`0..=5` sur les données réelles).
    pub play_style: i64,
}

impl PlaystyleEntry {
    /// Libellé anglais résolu (`None` si `play_style` hors `0..=5`).
    #[must_use]
    pub fn label_en(&self) -> Option<&'static str> {
        playstyle_id_to_en(self.play_style)
    }

    /// Libellé français résolu (`None` si `play_style` hors `0..=5`).
    #[must_use]
    pub fn label_fr(&self) -> Option<&'static str> {
        playstyle_id_to_fr(self.play_style)
    }
}

/// Parse un noeud `CHARA_PARAM_INFO` en [`PlaystyleEntry`].
///
/// Réplique exacte de `parsePlaystyleNode` : on collecte dans l'ordre **les seules variables de
/// type `Int`** (les éventuelles `String`/`Float` décalent donc l'indexation, comme côté TS),
/// on rejette (`None`) si moins de 6, puis `play_style = values[5]` et
/// `chara_param_id = toHex(values[0])`.
#[must_use]
pub fn parse_playstyle_node(node: &Node<'_>) -> Option<PlaystyleEntry> {
    let mut values: Vec<i64> = Vec::new();
    for i in 0..node.var_count() {
        if let Some(v) = node.var(i)
            && v.ty == "Int"
        {
            values.push(v.as_i64());
        }
    }
    if values.len() < 6 {
        return None;
    }
    Some(PlaystyleEntry {
        chara_param_id: HashId::from_i64(values[0]),
        play_style: values[5],
    })
}

/// Parse tous les styles de jeu d'un `chara_param_*.cfg.bin.json` désérialisé.
///
/// Port de `parseAllPlaystyles` : retient les noeuds nommés `CHARA_PARAM_INFO_*` (forme iecode
/// suffixée `_<i>`), en excluant les noeuds de liste (`*LIST*` / `*BEG*`).
#[must_use]
pub fn parse_all_playstyles(root: &Value) -> Vec<PlaystyleEntry> {
    let mut out = Vec::new();
    walk_named(root, "CHARA_PARAM_INFO_", |node| {
        let name = node.name();
        if name.contains("LIST") || name.contains("BEG") {
            return;
        }
        if let Some(e) = parse_playstyle_node(&node) {
            out.push(e);
        }
    });
    out
}
