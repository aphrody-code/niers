//! `AddContentEquipConfig` — port de `system/add_content_equip_config.cfg.bin.json` (Level-5 IEVR).
//!
//! Les **équipements de contenu additionnel (AOC = Add-On Content / DLC)** : une liste
//! `m_aocEquipConfigInfo` associant un `equipID` à une `aocCondition` (blob base64 = condition de
//! déblocage DLC). Format **`lists`**. 22 entrées dans le dump réel.

use alloc::{string::String, vec::Vec};
use serde_json::Value;

use crate::cfgbin::{field_hash, field_str, list_values};
use crate::hash::HashId;

/// Une entrée `m_aocEquipConfigInfo` — un équipement débloqué par contenu additionnel.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AocEquipConfigInfo {
    /// `aocCondition` — condition de déblocage encodée base64 (blob binaire, sémantique non décodée).
    pub aoc_condition: String,
    /// `equipID` — hash de l'identifiant d'équipement débloqué.
    pub equip_id: HashId,
}

impl AocEquipConfigInfo {
    /// Parse une entrée. `None` si `equipID` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let equip_id = field_hash(v, "equipID");
        if equip_id.is_zero() {
            return None;
        }
        Some(Self {
            aoc_condition: String::from(field_str(v, "aocCondition").unwrap_or("")),
            equip_id,
        })
    }
}

/// Parse `add_content_equip_config.cfg.bin.json` → la liste des équipements DLC.
#[must_use]
pub fn parse_add_content_equip_config(root: &Value) -> Vec<AocEquipConfigInfo> {
    list_values(root, "m_aocEquipConfigInfo").map_or_else(Vec::new, |vs| {
        vs.iter()
            .filter_map(AocEquipConfigInfo::from_value)
            .collect()
    })
}
