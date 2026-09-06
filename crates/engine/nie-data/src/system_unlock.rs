//! `SystemUnlockWindowConfig` — port Rust de `system/system_unlock_window_config_*.cfg.bin`.
//!
//! **Gating de progression** : décrit les fenêtres « système débloqué » d'IEVR (le bandeau qui
//! apparaît quand une fonctionnalité s'ouvre). Chaque *set* de déblocage référence une tranche
//! d'entrées d'info (texte + icône + tag). Famille game-data **non couverte** par nie-data ;
//! **pas de parseur inagle** → port direct depuis la structure du dump (champs nommés verbatim,
//! aucune sémantique inventée — anti-hallucination).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/system/system_unlock_window_config_0.00.00.00.cfg.bin.json`
//!   (format `lists`, file `version = 100`).
//! - 2 listes :
//!   - `m_systemUnlockWindowInfoList` — **98** `SYSTEM_UNLOCK_WINDOW_INFO`
//!     (`textId`, `tagTypeId`, `replaceId`, `gaijiIconCrc`).
//!   - `m_systemUnlockWindowSetDataList` — **26** `SYSTEM_UNLOCK_WINDOW_SET_DATA`
//!     (`systemUnlockWindowIdCrc` + `systemUnlockWindowInfoList` = `[offset, count]`).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, field_pair, list_values};
use crate::hash::HashId;

/// Entrée `SYSTEM_UNLOCK_WINDOW_INFO` — une ligne d'info de fenêtre de déblocage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemUnlockWindowInfo {
    /// `textId` — hash du texte affiché.
    pub text_id: HashId,
    /// `tagTypeId` — type de tag (catégorie de la fonctionnalité débloquée).
    pub tag_type_id: i64,
    /// `replaceId` — hash de remplacement (variante textuelle).
    pub replace_id: HashId,
    /// `gaijiIconCrc` — crc de l'icône gaiji associée.
    pub gaiji_icon_crc: HashId,
}

impl SystemUnlockWindowInfo {
    /// Parse une entrée. `None` si `textId` est nul/absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let text_id = field_hash(v, "textId");
        if text_id.is_zero() {
            return None;
        }
        Some(Self {
            text_id,
            tag_type_id: field_i64(v, "tagTypeId").unwrap_or(0),
            replace_id: field_hash(v, "replaceId"),
            gaiji_icon_crc: field_hash(v, "gaijiIconCrc"),
        })
    }
}

/// Entrée `SYSTEM_UNLOCK_WINDOW_SET_DATA` — un set de déblocage (id + tranche d'infos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemUnlockWindowSet {
    /// `systemUnlockWindowIdCrc` — crc identifiant le set de déblocage.
    pub id: HashId,
    /// `systemUnlockWindowInfoList` — `[offset, count]` indexant `m_systemUnlockWindowInfoList`.
    pub info_slice: [i64; 2],
}

impl SystemUnlockWindowSet {
    /// Parse une entrée. `None` si `systemUnlockWindowInfoList` n'est pas un tableau.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let info_slice = field_pair(v, "systemUnlockWindowInfoList")?;
        Some(Self {
            id: field_hash(v, "systemUnlockWindowIdCrc"),
            info_slice,
        })
    }
}

/// Contenu complet d'un `system_unlock_window_config_*.cfg.bin`.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemUnlockWindowConfig {
    /// `m_systemUnlockWindowInfoList` — lignes d'info (indexées par les sets).
    pub infos: Vec<SystemUnlockWindowInfo>,
    /// `m_systemUnlockWindowSetDataList` — sets de déblocage.
    pub sets: Vec<SystemUnlockWindowSet>,
}

impl SystemUnlockWindowConfig {
    /// Résout la tranche d'infos d'un set par son `id`. `None` si le set est absent.
    #[must_use]
    pub fn resolve_set(&self, id: HashId) -> Option<&[SystemUnlockWindowInfo]> {
        let set = self.sets.iter().find(|s| s.id == id)?;
        let n = self.infos.len();
        let start = (set.info_slice[0].max(0) as usize).min(n);
        let end = start
            .saturating_add(set.info_slice[1].max(0) as usize)
            .min(n);
        self.infos.get(start..end)
    }
}

/// Parse un `system_unlock_window_config_*.cfg.bin.json`.
#[must_use]
pub fn parse_system_unlock_window_config(root: &Value) -> SystemUnlockWindowConfig {
    let mut infos = Vec::new();
    if let Some(values) = list_values(root, "m_systemUnlockWindowInfoList") {
        for v in values {
            if let Some(info) = SystemUnlockWindowInfo::from_value(v) {
                infos.push(info);
            }
        }
    }
    let mut sets = Vec::new();
    if let Some(values) = list_values(root, "m_systemUnlockWindowSetDataList") {
        for v in values {
            if let Some(set) = SystemUnlockWindowSet::from_value(v) {
                sets.push(set);
            }
        }
    }
    SystemUnlockWindowConfig { infos, sets }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({
            "version": 100,
            "lists": [
                { "name": "m_systemUnlockWindowInfoList", "values": [
                    { "textId": "0x395AEAB5", "tagTypeId": 1, "replaceId": "0xEBBEE959", "gaijiIconCrc": "0xA6F614F6" },
                    { "textId": "0xB722B2A8", "tagTypeId": 4, "replaceId": "0xA075F1F5", "gaijiIconCrc": "0xA82D9CC4" }
                ]},
                { "name": "m_systemUnlockWindowSetDataList", "values": [
                    { "systemUnlockWindowIdCrc": "0x23CA11FE", "systemUnlockWindowInfoList": [0, 1] },
                    { "systemUnlockWindowIdCrc": "0x91EACDEE", "systemUnlockWindowInfoList": [1, 1] }
                ]}
            ]
        })
    }

    #[test]
    fn parse_comptes_et_valeurs() {
        let cfg = parse_system_unlock_window_config(&fixture());
        assert_eq!(cfg.infos.len(), 2);
        assert_eq!(cfg.sets.len(), 2);
        assert_eq!(cfg.infos[0].text_id, HashId(0x395A_EAB5));
        assert_eq!(cfg.infos[0].tag_type_id, 1);
        assert_eq!(cfg.infos[0].replace_id, HashId(0xEBBE_E959));
        assert_eq!(cfg.infos[0].gaiji_icon_crc, HashId(0xA6F6_14F6));
        assert_eq!(cfg.sets[0].id, HashId(0x23CA_11FE));
        assert_eq!(cfg.sets[0].info_slice, [0, 1]);
    }

    #[test]
    fn resolve_set_tranche() {
        let cfg = parse_system_unlock_window_config(&fixture());
        let slice = cfg.resolve_set(HashId(0x91EA_CDEE)).expect("set présent");
        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].text_id, HashId(0xB722_B2A8));
        assert!(cfg.resolve_set(HashId(0xDEAD_BEEF)).is_none());
    }
}
