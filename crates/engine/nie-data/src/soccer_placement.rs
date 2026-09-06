//! `soccer_placement` — port de `soccer/soccer_chara_placement_*.cfg.bin` (Level-5 IEVR).
//!
//! **Placement des joueurs sur le terrain** (positions de kickoff, coups de pied arrêtés, scènes).
//! Famille non couverte ; port direct. 3 niveaux : un *placement* référence des *catégories*, qui
//! référencent des *placements de perso* (position/rotation). Tranches `[offset,count]` imbriquées.
//!
//! Vérité terrain : `soccer_chara_placement_1.01.97.00.cfg.bin.json` —
//! `m_charaPlacementData` (2269) / `m_placementCategory` (798) / `m_placementData` (398).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_pair, list_values};
use crate::hash::HashId;

/// `m_charaPlacementData` — placement d'un joueur (position/rotation sur le terrain).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaPlacement {
    /// `charaParameterId` — hash du paramètre de perso placé.
    pub chara_parameter_id: HashId,
    /// `posX` / `posZ` — position sur le terrain (unités entières).
    pub pos_x: i64,
    /// `posZ`.
    pub pos_z: i64,
    /// `rotY` — rotation (degrés).
    pub rot_y: i64,
    /// `isSetCtrlChara` — perso contrôlé par défaut.
    pub is_set_ctrl_chara: bool,
    /// `catchBallType` — type de réception du ballon.
    pub catch_ball_type: i64,
    /// `actionId` — hash d'action initiale (`0` = aucune).
    pub action_id: HashId,
}

impl CharaPlacement {
    /// Parse une entrée (infaillible : indexée par tranche, position significative).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            chara_parameter_id: field_hash(v, "charaParameterId"),
            pos_x: field_i64(v, "posX").unwrap_or(0),
            pos_z: field_i64(v, "posZ").unwrap_or(0),
            rot_y: field_i64(v, "rotY").unwrap_or(0),
            is_set_ctrl_chara: field_bool(v, "isSetCtrlChara").unwrap_or(false),
            catch_ball_type: field_i64(v, "catchBallType").unwrap_or(0),
            action_id: field_hash(v, "actionId"),
        }
    }
}

/// `m_placementCategory` — une catégorie (référence une tranche de `chara_placement_data`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlacementCategory {
    /// `placementCategory` — code de catégorie.
    pub category: i64,
    /// `placementData` — `[offset, count]` indexant `m_charaPlacementData`.
    pub data: [i64; 2],
}

/// `m_placementData` — un placement nommé (référence une tranche de catégories).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlacementInfo {
    /// `placementId` — hash du placement.
    pub placement_id: HashId,
    /// `categoryData` — `[offset, count]` indexant `m_placementCategory`.
    pub category_data: [i64; 2],
}

/// Contenu d'un `soccer_chara_placement_*.cfg.bin`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerPlacementConfig {
    /// `m_charaPlacementData` — placements de perso (positions).
    pub chara_placements: Vec<CharaPlacement>,
    /// `m_placementCategory` — catégories.
    pub categories: Vec<PlacementCategory>,
    /// `m_placementData` — placements nommés.
    pub placements: Vec<PlacementInfo>,
}

impl SoccerPlacementConfig {
    /// Catégories d'un placement nommé.
    #[must_use]
    pub fn categories_of(&self, p: &PlacementInfo) -> &[PlacementCategory] {
        slice(&self.categories, p.category_data)
    }
    /// Placements de perso d'une catégorie.
    #[must_use]
    pub fn charas_of(&self, c: &PlacementCategory) -> &[CharaPlacement] {
        slice(&self.chara_placements, c.data)
    }
    /// Recherche un placement par son `placementId`.
    #[must_use]
    pub fn find(&self, placement_id: HashId) -> Option<&PlacementInfo> {
        self.placements
            .iter()
            .find(|p| p.placement_id == placement_id)
    }
}

fn slice<T>(list: &[T], range: [i64; 2]) -> &[T] {
    let n = list.len();
    let start = (range[0].max(0) as usize).min(n);
    let end = start.saturating_add(range[1].max(0) as usize).min(n);
    list.get(start..end).unwrap_or(&[])
}

/// Parse un `soccer_chara_placement_*.cfg.bin.json`.
#[must_use]
pub fn parse_soccer_placement_config(root: &Value) -> SoccerPlacementConfig {
    let mut chara_placements = Vec::new();
    if let Some(values) = list_values(root, "m_charaPlacementData") {
        for v in values {
            chara_placements.push(CharaPlacement::from_value(v));
        }
    }
    let mut categories = Vec::new();
    if let Some(values) = list_values(root, "m_placementCategory") {
        for v in values {
            if let Some(data) = field_pair(v, "placementData") {
                categories.push(PlacementCategory {
                    category: field_i64(v, "placementCategory").unwrap_or(0),
                    data,
                });
            }
        }
    }
    let mut placements = Vec::new();
    if let Some(values) = list_values(root, "m_placementData") {
        for v in values {
            if let Some(category_data) = field_pair(v, "categoryData") {
                let placement_id = field_hash(v, "placementId");
                if !placement_id.is_zero() {
                    placements.push(PlacementInfo {
                        placement_id,
                        category_data,
                    });
                }
            }
        }
    }
    SoccerPlacementConfig {
        chara_placements,
        categories,
        placements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_et_resolution_3_niveaux() {
        let root = json!({
            "lists": [
                { "name": "m_charaPlacementData", "values": [
                    { "charaParameterId": "0xA5F32308", "posX": -10, "posZ": 13, "rotY": 180,
                      "isSetCtrlChara": false, "catchBallType": 0, "actionId": "0x00000000" },
                    { "charaParameterId": "0x11111111", "posX": 5, "posZ": -7, "rotY": 0,
                      "isSetCtrlChara": true, "catchBallType": 1, "actionId": "0x22222222" }
                ]},
                { "name": "m_placementCategory", "values": [
                    { "placementCategory": 0, "placementData": [0, 2] }
                ]},
                { "name": "m_placementData", "values": [
                    { "placementId": "0xA2500FF1", "categoryData": [0, 1] }
                ]}
            ]
        });
        let cfg = parse_soccer_placement_config(&root);
        assert_eq!(cfg.chara_placements.len(), 2);
        assert_eq!(
            cfg.chara_placements[0].chara_parameter_id,
            HashId(0xA5F3_2308)
        );
        assert_eq!(cfg.chara_placements[0].pos_x, -10);
        assert_eq!(cfg.chara_placements[0].rot_y, 180);
        assert!(cfg.chara_placements[1].is_set_ctrl_chara);
        // résolution 3 niveaux : placement → catégories → persos.
        let p = cfg.find(HashId(0xA2500FF1)).expect("placement présent");
        let cats = cfg.categories_of(p);
        assert_eq!(cats.len(), 1);
        let charas = cfg.charas_of(&cats[0]);
        assert_eq!(charas.len(), 2);
        assert_eq!(charas[1].pos_x, 5);
    }
}
