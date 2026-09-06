//! `soccer_suggest` — port de `soccer/soccer_suggest_config_*.cfg.bin` (Level-5 IEVR).
//!
//! Système de **suggestions de match** (commandes assistées proposées au joueur pendant un match :
//! passes, tirs, prédictions de mouvement, caméras, zones d'extension de passe). Famille game-data
//! **non couverte** ; port direct (pas de réf inagle). 8 listes, certaines à tranches `[offset,count]`.
//!
//! ## Vérité terrain
//!
//! Dump réel : `data/common/gamedata/soccer/soccer_suggest_config_0.01.92.cfg.bin.json` (format `lists`).
//! - `m_soccerSuggestInfoList` (5) — les suggestions (cmd/text/icônes/coût/connexions).
//! - `m_soccerSuggestCameraInfoList` (1) — caméra de suggestion.
//! - `m_soccerSuggestPredictMot/Phase/Object/InfoList` (5 chacune) — arbre de prédiction de mouvement.
//! - `m_soccerSuggestPassExtensionData/InfoList` (132 / 12) — extension de passe par zone.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_pair, list_values};
use crate::hash::HashId;

/// `SOCCER_SUGGEST_INFO` — une commande de suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuggestInfo {
    /// `id` — hash de la suggestion.
    pub id: HashId,
    /// `cmdId` — hash de la commande déclenchée.
    pub cmd_id: HashId,
    /// `textId` — hash du libellé.
    pub text_id: HashId,
    /// `requestTextId` — hash du libellé de requête.
    pub request_text_id: HashId,
    /// `category` / `subCategory` / `type` — classification.
    pub category: i64,
    /// `subCategory`.
    pub sub_category: i64,
    /// `type`.
    pub kind: i64,
    /// `iconId` / `icon_on_Id` / `icon_off_Id` — icônes.
    pub icon_id: HashId,
    /// `icon_on_Id`.
    pub icon_on_id: HashId,
    /// `icon_off_Id`.
    pub icon_off_id: HashId,
    /// `isDefElected` — sélectionnée par défaut.
    pub is_def_elected: bool,
    /// `cost` — coût (jauge).
    pub cost: i64,
    /// `connectType`.
    pub connect_type: i64,
    /// `isSpProd` — production spéciale.
    pub is_sp_prod: bool,
    /// `spProdCamera` — caméra de production spéciale.
    pub sp_prod_camera: HashId,
    /// `counterSuggest` — suggestion de contre.
    pub counter_suggest: HashId,
    /// `connectSuggest` — suggestion enchaînée.
    pub connect_suggest: HashId,
}

impl SuggestInfo {
    /// Parse une entrée. `None` si `id` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            cmd_id: field_hash(v, "cmdId"),
            text_id: field_hash(v, "textId"),
            request_text_id: field_hash(v, "requestTextId"),
            category: field_i64(v, "category").unwrap_or(0),
            sub_category: field_i64(v, "subCategory").unwrap_or(0),
            kind: field_i64(v, "type").unwrap_or(0),
            icon_id: field_hash(v, "iconId"),
            icon_on_id: field_hash(v, "icon_on_Id"),
            icon_off_id: field_hash(v, "icon_off_Id"),
            is_def_elected: field_bool(v, "isDefElected").unwrap_or(false),
            cost: field_i64(v, "cost").unwrap_or(0),
            connect_type: field_i64(v, "connectType").unwrap_or(0),
            is_sp_prod: field_bool(v, "isSpProd").unwrap_or(false),
            sp_prod_camera: field_hash(v, "spProdCamera"),
            counter_suggest: field_hash(v, "counterSuggest"),
            connect_suggest: field_hash(v, "connectSuggest"),
        })
    }
}

/// `SOCCER_SUGGEST_CAMERA_INFO` — réglage de caméra de suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuggestCameraInfo {
    /// `id` — hash de la caméra.
    pub id: HashId,
    /// `refAttach`.
    pub ref_attach: i64,
    /// `ref_pos_distance` / `ref_pos_altitude` / `ref_pos_azimuth` — position relative.
    pub distance: i64,
    /// `ref_pos_altitude`.
    pub altitude: i64,
    /// `ref_pos_azimuth`.
    pub azimuth: i64,
    /// `scriptFunc` — hash de fonction script (`0` = aucune).
    pub script_func: HashId,
}

impl SuggestCameraInfo {
    /// Parse une entrée. `None` si `id` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            ref_attach: field_i64(v, "refAttach").unwrap_or(0),
            distance: field_i64(v, "ref_pos_distance").unwrap_or(0),
            altitude: field_i64(v, "ref_pos_altitude").unwrap_or(0),
            azimuth: field_i64(v, "ref_pos_azimuth").unwrap_or(0),
            script_func: field_hash(v, "scriptFunc"),
        })
    }
}

/// `SOCCER_SUGGEST_PASS_EXTENSION_DATA` — une cible d'extension de passe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PassExtensionData {
    /// `dstAreaIdx` — index de zone de destination.
    pub dst_area_idx: i64,
    /// `cmdId`.
    pub cmd_id: HashId,
    /// `textId`.
    pub text_id: HashId,
    /// `requestTextId`.
    pub request_text_id: HashId,
    /// `iconId` / `icon_on_Id` / `icon_off_Id`.
    pub icon_id: HashId,
    /// `icon_on_Id`.
    pub icon_on_id: HashId,
    /// `icon_off_Id`.
    pub icon_off_id: HashId,
    /// `spProdCamera`.
    pub sp_prod_camera: HashId,
    /// `isSpProd`.
    pub is_sp_prod: bool,
    /// `cost`.
    pub cost: i64,
}

impl PassExtensionData {
    /// Parse une entrée (infaillible : indexée par tranche, position significative).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            dst_area_idx: field_i64(v, "dstAreaIdx").unwrap_or(0),
            cmd_id: field_hash(v, "cmdId"),
            text_id: field_hash(v, "textId"),
            request_text_id: field_hash(v, "requestTextId"),
            icon_id: field_hash(v, "iconId"),
            icon_on_id: field_hash(v, "icon_on_Id"),
            icon_off_id: field_hash(v, "icon_off_Id"),
            sp_prod_camera: field_hash(v, "spProdCamera"),
            is_sp_prod: field_bool(v, "isSpProd").unwrap_or(false),
            cost: field_i64(v, "cost").unwrap_or(0),
        }
    }
}

/// Une table à id + tranche `[offset,count]` (prédiction/extension).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuggestSlice {
    /// `id`/`srcAreaIdx`/`type`/`phase` — clé (selon la liste).
    pub key: i64,
    /// hash optionnel (pour les listes à `id` hash).
    pub id: HashId,
    /// `[offset, count]` indexant la liste fille.
    pub slice: [i64; 2],
}

/// Contenu complet d'un `soccer_suggest_config_*.cfg.bin`.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerSuggestConfig {
    /// `m_soccerSuggestInfoList` — suggestions.
    pub suggests: Vec<SuggestInfo>,
    /// `m_soccerSuggestCameraInfoList` — caméras.
    pub cameras: Vec<SuggestCameraInfo>,
    /// `m_soccerSuggestPredictMotList` — motions de prédiction (`mot` hash).
    pub predict_mots: Vec<HashId>,
    /// `m_soccerSuggestPredictPhaseList` — phases (`phase` + tranche `motList`).
    pub predict_phases: Vec<SuggestSlice>,
    /// `m_soccerSuggestPredictObjectList` — objets (`type` + tranche `phaseList`).
    pub predict_objects: Vec<SuggestSlice>,
    /// `m_soccerSuggestPredictInfoList` — infos (`id` + tranche `objList`).
    pub predict_infos: Vec<SuggestSlice>,
    /// `m_soccerSuggestPassExtensionDataList` — cibles d'extension de passe.
    pub pass_extension_data: Vec<PassExtensionData>,
    /// `m_soccerSuggestPassExtensionInfoList` — zones source (`srcAreaIdx` + tranche `refData`).
    pub pass_extension_infos: Vec<SuggestSlice>,
}

fn parse_list<T>(root: &Value, name: &str, f: impl Fn(&Value) -> Option<T>) -> Vec<T> {
    let mut out = Vec::new();
    if let Some(values) = list_values(root, name) {
        for v in values {
            if let Some(item) = f(v) {
                out.push(item);
            }
        }
    }
    out
}

fn parse_slices(
    root: &Value,
    name: &str,
    key: &str,
    id_key: &str,
    slice_key: &str,
) -> Vec<SuggestSlice> {
    let mut out = Vec::new();
    if let Some(values) = list_values(root, name) {
        for v in values {
            if let Some(slice) = field_pair(v, slice_key) {
                out.push(SuggestSlice {
                    key: field_i64(v, key).unwrap_or(0),
                    id: field_hash(v, id_key),
                    slice,
                });
            }
        }
    }
    out
}

/// Parse un `soccer_suggest_config_*.cfg.bin.json` complet.
#[must_use]
pub fn parse_soccer_suggest_config(root: &Value) -> SoccerSuggestConfig {
    let mut predict_mots = Vec::new();
    if let Some(values) = list_values(root, "m_soccerSuggestPredictMotList") {
        for v in values {
            predict_mots.push(field_hash(v, "mot"));
        }
    }
    let mut pass_extension_data = Vec::new();
    if let Some(values) = list_values(root, "m_soccerSuggestPassExtensionDataList") {
        for v in values {
            pass_extension_data.push(PassExtensionData::from_value(v));
        }
    }
    SoccerSuggestConfig {
        suggests: parse_list(root, "m_soccerSuggestInfoList", SuggestInfo::from_value),
        cameras: parse_list(
            root,
            "m_soccerSuggestCameraInfoList",
            SuggestCameraInfo::from_value,
        ),
        predict_mots,
        predict_phases: parse_slices(
            root,
            "m_soccerSuggestPredictPhaseList",
            "phase",
            "",
            "motList",
        ),
        predict_objects: parse_slices(
            root,
            "m_soccerSuggestPredictObjectList",
            "type",
            "",
            "phaseList",
        ),
        predict_infos: parse_slices(root, "m_soccerSuggestPredictInfoList", "", "id", "objList"),
        pass_extension_data,
        pass_extension_infos: parse_slices(
            root,
            "m_soccerSuggestPassExtensionInfoList",
            "srcAreaIdx",
            "",
            "refData",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({
            "lists": [
                { "name": "m_soccerSuggestInfoList", "values": [
                    { "id": "0xEC13A97C", "cmdId": "0x9178C3FE", "textId": "0xF321EEC8",
                      "requestTextId": "0xE4B5DA57", "category": 1, "subCategory": 1, "type": 0,
                      "iconId": "0xD33400DB", "icon_on_Id": "0x70A6A51F", "icon_off_Id": "0x33720AB8",
                      "isDefElected": true, "cost": 1, "connectType": 0, "isSpProd": false,
                      "spProdCamera": "0x00000000", "counterSuggest": "0xABB3D3AC", "connectSuggest": "0x00000000" }
                ]},
                { "name": "m_soccerSuggestPredictPhaseList", "values": [
                    { "phase": 0, "motList": [0, 1] }, { "phase": 0, "motList": [1, 1] }
                ]},
                { "name": "m_soccerSuggestPassExtensionInfoList", "values": [
                    { "srcAreaIdx": 0, "refData": [0, 11] }, { "srcAreaIdx": 1, "refData": [11, 11] }
                ]}
            ]
        })
    }

    #[test]
    fn parse_suggest_et_slices() {
        let cfg = parse_soccer_suggest_config(&fixture());
        assert_eq!(cfg.suggests.len(), 1);
        let s = &cfg.suggests[0];
        assert_eq!(s.id, HashId(0xEC13_A97C));
        assert_eq!(s.cmd_id, HashId(0x9178_C3FE));
        assert_eq!(s.category, 1);
        assert!(s.is_def_elected);
        assert_eq!(s.cost, 1);
        assert_eq!(s.counter_suggest, HashId(0xABB3_D3AC));
        // slices.
        assert_eq!(cfg.predict_phases.len(), 2);
        assert_eq!(cfg.predict_phases[0].slice, [0, 1]);
        assert_eq!(cfg.pass_extension_infos[1].key, 1);
        assert_eq!(cfg.pass_extension_infos[1].slice, [11, 11]);
    }
}
