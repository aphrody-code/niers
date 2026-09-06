//! `AddModelConfig` — port de `character/add_model_config.cfg.bin.json` (Level-5 IEVR).
//!
//! Les **modèles 3D additionnels** attachés à un personnage de base (ex. accessoires/variantes
//! greffés sur un perso). Format **`lists`**, 2 listes :
//! - `m_addModelDataList` — `{addModelType, addModelId}`, les modèles ajoutables.
//! - `m_addModelInfoList` — `{baseCharaId, addModel:[offset,count]}`, par perso de base, la plage
//!   de modèles ajoutés (offset+count dans la data list).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, list_values};
use crate::hash::HashId;

/// Un modèle ajoutable (`m_addModelDataList`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddModelData {
    /// `addModelType` — type de modèle ajouté (entier).
    pub add_model_type: i64,
    /// `addModelId` — hash de l'identifiant du modèle.
    pub add_model_id: HashId,
}

impl AddModelData {
    fn from_value(v: &Value) -> Self {
        Self {
            add_model_type: field_i64(v, "addModelType").unwrap_or(0),
            add_model_id: field_hash(v, "addModelId"),
        }
    }
}

/// Liaison perso de base → plage de modèles ajoutés (`m_addModelInfoList`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddModelInfo {
    /// `baseCharaId` — hash du perso de base.
    pub base_chara_id: HashId,
    /// `addModel[0]` — offset dans `m_addModelDataList`.
    pub data_offset: i64,
    /// `addModel[1]` — nombre de modèles ajoutés.
    pub data_count: i64,
}

impl AddModelInfo {
    fn from_value(v: &Value) -> Self {
        let arr = v.get("addModel").and_then(Value::as_array);
        let at = |i: usize| {
            arr.and_then(|a| a.get(i))
                .and_then(Value::as_i64)
                .unwrap_or(0)
        };
        Self {
            base_chara_id: field_hash(v, "baseCharaId"),
            data_offset: at(0),
            data_count: at(1),
        }
    }
}

/// Config des modèles additionnels (2 listes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddModelConfig {
    pub data: Vec<AddModelData>,
    pub info: Vec<AddModelInfo>,
}

/// Parse `add_model_config.cfg.bin.json`.
#[must_use]
pub fn parse_add_model_config(root: &Value) -> AddModelConfig {
    AddModelConfig {
        data: list_values(root, "m_addModelDataList").map_or_else(Vec::new, |vs| {
            vs.iter().map(AddModelData::from_value).collect()
        }),
        info: list_values(root, "m_addModelInfoList").map_or_else(Vec::new, |vs| {
            vs.iter().map(AddModelInfo::from_value).collect()
        }),
    }
}
