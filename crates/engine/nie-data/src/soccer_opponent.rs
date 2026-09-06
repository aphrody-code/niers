//! `soccer_opponent` — port de `soccer/soccer_opponent_info_*.cfg.bin` (Level-5 IEVR).
//!
//! Liste des **adversaires de match** (catégorie, identifiant de combat, ordre de tri) et leurs
//! **conditions de déblocage** (`cond1`..`cond4`, blobs `cond` décodés via [`crate::unlock_condition`]).
//! Famille non couverte ; port direct.
//!
//! Vérité terrain : `data/common/gamedata/soccer/soccer_opponent_info_0.00.00.cfg.bin.json`
//! (`m_soccerOpponentInfoList`, 154 entrées : `category`, `battleId`, `sortOrder`, `cond1..4`).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, field_str, list_values};
use crate::hash::HashId;
use crate::unlock_condition::{UnlockCondition, decode_unlock_condition};

/// Une entrée d'adversaire de match.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpponentInfo {
    /// `category` — catégorie d'adversaire.
    pub category: i64,
    /// `battleId` — hash du combat/équipe adverse.
    pub battle_id: HashId,
    /// `sortOrder` — ordre d'affichage.
    pub sort_order: i64,
    /// `cond1`..`cond4` décodées (seules les conditions non vides sont conservées).
    pub conditions: Vec<UnlockCondition>,
}

impl OpponentInfo {
    /// Parse une entrée. `None` si `battleId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let battle_id = field_hash(v, "battleId");
        if battle_id.is_zero() {
            return None;
        }
        let mut conditions = Vec::new();
        for key in ["cond1", "cond2", "cond3", "cond4"] {
            if let Some(s) = field_str(v, key)
                && s.len() >= 12
                && s.starts_with("AAAA")
            {
                conditions.push(decode_unlock_condition(s));
            }
        }
        Some(Self {
            category: field_i64(v, "category").unwrap_or(0),
            battle_id,
            sort_order: field_i64(v, "sortOrder").unwrap_or(0),
            conditions,
        })
    }
}

/// Contenu d'un `soccer_opponent_info_*.cfg.bin`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerOpponentConfig {
    /// `m_soccerOpponentInfoList` — les adversaires.
    pub opponents: Vec<OpponentInfo>,
}

impl SoccerOpponentConfig {
    /// Recherche un adversaire par son `battleId`.
    #[must_use]
    pub fn find_by_battle_id(&self, battle_id: HashId) -> Option<&OpponentInfo> {
        self.opponents.iter().find(|o| o.battle_id == battle_id)
    }
}

/// Parse un `soccer_opponent_info_*.cfg.bin.json`.
#[must_use]
pub fn parse_soccer_opponent_config(root: &Value) -> SoccerOpponentConfig {
    let mut opponents = Vec::new();
    if let Some(values) = list_values(root, "m_soccerOpponentInfoList") {
        for v in values {
            if let Some(o) = OpponentInfo::from_value(v) {
                opponents.push(o);
            }
        }
    }
    SoccerOpponentConfig { opponents }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_opponent_et_conditions() {
        let root = json!({
            "lists": [{ "name": "m_soccerOpponentInfoList", "values": [
                { "category": 0, "battleId": "0xA57EED32", "sortOrder": 10,
                  "cond1": "AAAAACEFNbgXycUAEwIoAAYCNKV+7TIoAAYCMgAAAAEyAAAAAXg=",
                  "cond2": "0", "cond3": "", "cond4": "" }
            ]}]
        });
        let cfg = parse_soccer_opponent_config(&root);
        assert_eq!(cfg.opponents.len(), 1);
        let o = &cfg.opponents[0];
        assert_eq!(o.battle_id, HashId(0xA57E_ED32));
        assert_eq!(o.sort_order, 10);
        // cond1 = blob valide → décodé ; cond2="0"/cond3,4 vides → ignorés.
        assert_eq!(o.conditions.len(), 1);
        assert!(cfg.find_by_battle_id(HashId(0xA57E_ED32)).is_some());
    }
}
