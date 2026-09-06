//! `TrialTakeOverConfig` — port Rust de `system/trial_take_over_config.cfg.bin` (Level-5 IEVR).
//!
//! **Report de progression d'essai** (« trial take-over ») : ce qui est conservé/transféré lors
//! d'un essai (flags, items, conditions) + report partiel. Famille game-data **non couverte** ;
//! **pas de parseur inagle** → port direct (champs nommés verbatim).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/system/trial_take_over_config.cfg.bin.json` (format `lists`).
//! - 2 listes :
//!   - `m_trialTakeOverInfoList` — **5** (`id`, `flagNo`, `itemId`, `condition`).
//!   - `m_trialPartTakeOverInfoList` — **9** (`id`, `flagNo`).

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, field_str, list_values};
use crate::hash::HashId;

/// Entrée `m_trialTakeOverInfoList` — un élément de report d'essai complet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrialTakeOverInfo {
    /// `id` — hash identifiant l'élément de report.
    pub id: HashId,
    /// `flagNo` — numéro de flag associé.
    pub flag_no: i64,
    /// `itemId` — hash de l'item reporté (`0` = aucun).
    pub item_id: HashId,
    /// `condition` — condition (base64 opaque, souvent vide).
    pub condition: String,
}

impl TrialTakeOverInfo {
    /// Parse une entrée. `None` si `id` est nul/absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            flag_no: field_i64(v, "flagNo").unwrap_or(0),
            item_id: field_hash(v, "itemId"),
            condition: field_str(v, "condition").unwrap_or("").into(),
        })
    }

    /// Décode la `condition` (blob base64) en [`UnlockCondition`] structurée via
    /// [`crate::unlock_condition`] (réf inagle, validée corpus-wide). Chaîne vide ⇒ `Always`.
    #[must_use]
    pub fn decode_condition(&self) -> crate::unlock_condition::UnlockCondition {
        crate::unlock_condition::decode_unlock_condition(&self.condition)
    }
}

/// Entrée `m_trialPartTakeOverInfoList` — un report partiel (id + flag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrialPartTakeOverInfo {
    /// `id` — hash identifiant le report partiel.
    pub id: HashId,
    /// `flagNo` — numéro de flag associé.
    pub flag_no: i64,
}

impl TrialPartTakeOverInfo {
    /// Parse une entrée. `None` si `id` est nul/absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            flag_no: field_i64(v, "flagNo").unwrap_or(0),
        })
    }
}

/// Contenu complet d'un `trial_take_over_config.cfg.bin`.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrialTakeOverConfig {
    /// `m_trialTakeOverInfoList` — reports d'essai complets.
    pub take_over: Vec<TrialTakeOverInfo>,
    /// `m_trialPartTakeOverInfoList` — reports partiels.
    pub part_take_over: Vec<TrialPartTakeOverInfo>,
}

/// Parse un `trial_take_over_config.cfg.bin.json`.
#[must_use]
pub fn parse_trial_take_over_config(root: &Value) -> TrialTakeOverConfig {
    let mut take_over = Vec::new();
    if let Some(values) = list_values(root, "m_trialTakeOverInfoList") {
        for v in values {
            if let Some(e) = TrialTakeOverInfo::from_value(v) {
                take_over.push(e);
            }
        }
    }
    let mut part_take_over = Vec::new();
    if let Some(values) = list_values(root, "m_trialPartTakeOverInfoList") {
        for v in values {
            if let Some(e) = TrialPartTakeOverInfo::from_value(v) {
                part_take_over.push(e);
            }
        }
    }
    TrialTakeOverConfig {
        take_over,
        part_take_over,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({
            "lists": [
                { "name": "m_trialTakeOverInfoList", "values": [
                    { "id": "0x46896BAB", "flagNo": 0, "itemId": "0x00000000", "condition": "" },
                    { "id": "0x41E4AFB2", "flagNo": 4, "itemId": "0x00000000", "condition": "AAAA" }
                ]},
                { "name": "m_trialPartTakeOverInfoList", "values": [
                    { "id": "0x26EA0F62", "flagNo": 0 }
                ]}
            ]
        })
    }

    #[test]
    fn parse_deux_listes() {
        let cfg = parse_trial_take_over_config(&fixture());
        assert_eq!(cfg.take_over.len(), 2);
        assert_eq!(cfg.part_take_over.len(), 1);
        assert_eq!(cfg.take_over[0].id, HashId(0x4689_6BAB));
        assert_eq!(cfg.take_over[0].item_id, HashId::ZERO);
        assert_eq!(cfg.take_over[0].condition, "");
        assert_eq!(cfg.take_over[1].flag_no, 4);
        assert_eq!(cfg.take_over[1].condition, "AAAA");
        assert_eq!(cfg.part_take_over[0].id, HashId(0x26EA_0F62));
    }
}
