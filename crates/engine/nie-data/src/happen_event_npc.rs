//! `HappenEventNpcCommon` — port Rust de `system/happen_event_npc_common.cfg.bin` (Level-5 IEVR).
//!
//! Système d'**événements NPC du monde** (overworld) : combats de rue / rencontres déclenchées,
//! avec récompenses « nice point », armes tirées au sort, motions et conditions. Famille game-data
//! **non couverte** par nie-data ; **pas de parseur inagle** → port direct depuis la structure du
//! dump (champs nommés verbatim, aucune sémantique inventée — anti-hallucination).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/system/happen_event_npc_common.cfg.bin.json`
//!   (format `lists`, file `version = 100`).
//! - 4 listes :
//!   - `m_EventWeaponInfo` / `m_EventWeaponAnotherInfo` — **24** chacune
//!     (`weapon_id`, `weight`) — pools d'armes tirées au sort (indexées par tranche).
//!   - `m_EventAimedInfo` — **14** (`npc_type_id`, motions, `weapon_info` `[offset,count]`…).
//!   - `m_EventCommonInfo` — **15** (un événement : type, récompenses, slices `aimed_info`/`weapon_info`…).
//!
//! **Parse infaillible** (toutes les entrées conservées) : les 3 premières listes sont indexées par
//! tranches `[offset, count]` → la **position est significative**, aucune entrée n'est filtrée.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_pair, field_str, list_values};
use crate::hash::HashId;

/// Entrée `NPC_EVENT_WEAPON_INFO` — une arme tirable avec son poids (pools weapon/another).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventWeaponInfo {
    /// `weapon_id` — hash de l'arme.
    pub weapon_id: HashId,
    /// `weight` — poids de tirage.
    pub weight: i64,
}

impl EventWeaponInfo {
    /// Parse une entrée (infaillible : position significative).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            weapon_id: field_hash(v, "weapon_id"),
            weight: field_i64(v, "weight").unwrap_or(0),
        }
    }
}

/// Entrée `NPC_EVENT_AIMDE_INFO` — un NPC ciblé (motions + pool d'armes via tranche).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventAimedInfo {
    /// `npc_type_id` — hash du type de NPC.
    pub npc_type_id: HashId,
    /// `on_loop_mot_id` — hash de la motion en boucle (`0` = aucune).
    pub on_loop_mot_id: HashId,
    /// `face_id` — hash du visage (`0` = aucun).
    pub face_id: HashId,
    /// `ow_charaparam_id` — hash des paramètres overworld du perso.
    pub ow_charaparam_id: HashId,
    /// `weapon_info` — `[offset, count]` indexant `m_EventWeaponInfo`.
    pub weapon_info: [i64; 2],
    /// `on_loop_npc_act_type` — type d'action NPC en boucle (`-1` = aucun).
    pub on_loop_npc_act_type: i64,
    /// `is_not_battle_in` — `true` si l'événement n'entre pas en combat.
    pub is_not_battle_in: bool,
}

impl EventAimedInfo {
    /// Parse une entrée (infaillible : position significative).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            npc_type_id: field_hash(v, "npc_type_id"),
            on_loop_mot_id: field_hash(v, "on_loop_mot_id"),
            face_id: field_hash(v, "face_id"),
            ow_charaparam_id: field_hash(v, "ow_charaparam_id"),
            weapon_info: field_pair(v, "weapon_info").unwrap_or([0, 0]),
            on_loop_npc_act_type: field_i64(v, "on_loop_npc_act_type").unwrap_or(0),
            is_not_battle_in: field_bool(v, "is_not_battle_in").unwrap_or(false),
        }
    }
}

/// Entrée `HAPPEN_EVENT_NPC_COMMON_INFO` — un événement NPC complet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventCommonInfo {
    /// `event_id` — hash de l'événement.
    pub event_id: HashId,
    /// `event_type` — type d'événement.
    pub event_type: i64,
    /// `limit_num` — nombre limite de déclenchements.
    pub limit_num: i64,
    /// `recast_max` — recast maximum.
    pub recast_max: i64,
    /// `recast_min` — recast minimum.
    pub recast_min: i64,
    /// `nice_point_max` — récompense « nice point » maximale.
    pub nice_point_max: i64,
    /// `nice_point_min` — récompense « nice point » minimale.
    pub nice_point_min: i64,
    /// `event_name_text_id` — hash du texte de nom (`0` = aucun).
    pub event_name_text_id: HashId,
    /// `on_loop_mot_id` — hash de la motion en boucle (`0` = aucune).
    pub on_loop_mot_id: HashId,
    /// `face_id` — hash du visage (`0` = aucun).
    pub face_id: HashId,
    /// `ow_charaparam_id` — hash des paramètres overworld (`0` = aucun).
    pub ow_charaparam_id: HashId,
    /// `aimed_info` — `[offset, count]` indexant `m_EventAimedInfo`.
    pub aimed_info: [i64; 2],
    /// `weapon_info` — `[offset, count]` indexant `m_EventWeaponInfo`.
    pub weapon_info: [i64; 2],
    /// `on_loop_npc_act_type` — type d'action NPC en boucle (`-1` = aucun).
    pub on_loop_npc_act_type: i64,
    /// `cond` — condition de déclenchement (base64 opaque).
    pub cond: String,
    /// `y_times_text_id` — hash du texte « N fois » (`0` = aucun).
    pub y_times_text_id: HashId,
}

impl EventCommonInfo {
    /// Parse une entrée (infaillible : position significative dans la liste d'événements).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            event_id: field_hash(v, "event_id"),
            event_type: field_i64(v, "event_type").unwrap_or(0),
            limit_num: field_i64(v, "limit_num").unwrap_or(0),
            recast_max: field_i64(v, "recast_max").unwrap_or(0),
            recast_min: field_i64(v, "recast_min").unwrap_or(0),
            nice_point_max: field_i64(v, "nice_point_max").unwrap_or(0),
            nice_point_min: field_i64(v, "nice_point_min").unwrap_or(0),
            event_name_text_id: field_hash(v, "event_name_text_id"),
            on_loop_mot_id: field_hash(v, "on_loop_mot_id"),
            face_id: field_hash(v, "face_id"),
            ow_charaparam_id: field_hash(v, "ow_charaparam_id"),
            aimed_info: field_pair(v, "aimed_info").unwrap_or([0, 0]),
            weapon_info: field_pair(v, "weapon_info").unwrap_or([0, 0]),
            on_loop_npc_act_type: field_i64(v, "on_loop_npc_act_type").unwrap_or(0),
            cond: field_str(v, "cond").unwrap_or("").into(),
            y_times_text_id: field_hash(v, "y_times_text_id"),
        }
    }

    /// Décode la condition de déclenchement `cond` (blob base64) en [`UnlockCondition`] structurée
    /// via [`crate::unlock_condition`] (réf inagle, validée corpus-wide — cf. [`crate::cond`]).
    #[must_use]
    pub fn decode_cond(&self) -> crate::unlock_condition::UnlockCondition {
        crate::unlock_condition::decode_unlock_condition(&self.cond)
    }
}

/// Contenu complet d'un `happen_event_npc_common.cfg.bin` (les 4 listes du système d'événements NPC).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HappenEventNpcCommon {
    /// `m_EventWeaponInfo` — pool d'armes principal (indexé par tranche).
    pub weapon_info: Vec<EventWeaponInfo>,
    /// `m_EventWeaponAnotherInfo` — pool d'armes alternatif.
    pub weapon_another_info: Vec<EventWeaponInfo>,
    /// `m_EventAimedInfo` — NPC ciblés (indexés par tranche).
    pub aimed_info: Vec<EventAimedInfo>,
    /// `m_EventCommonInfo` — événements NPC.
    pub common_info: Vec<EventCommonInfo>,
}

impl HappenEventNpcCommon {
    /// Résout la tranche `weapon_info` d'un [`EventAimedInfo`] dans `m_EventWeaponInfo`.
    #[must_use]
    pub fn weapons_of_aimed(&self, aimed: &EventAimedInfo) -> &[EventWeaponInfo] {
        slice(&self.weapon_info, aimed.weapon_info)
    }

    /// Résout la tranche `aimed_info` d'un [`EventCommonInfo`] dans `m_EventAimedInfo`.
    #[must_use]
    pub fn aimed_of_event(&self, event: &EventCommonInfo) -> &[EventAimedInfo] {
        slice(&self.aimed_info, event.aimed_info)
    }
}

/// Extrait `list[offset .. offset+count]`, bornes plafonnées à `list.len()`.
fn slice<T>(list: &[T], range: [i64; 2]) -> &[T] {
    let n = list.len();
    let start = (range[0].max(0) as usize).min(n);
    let end = start.saturating_add(range[1].max(0) as usize).min(n);
    list.get(start..end).unwrap_or(&[])
}

/// Parse un `happen_event_npc_common.cfg.bin.json` complet.
#[must_use]
pub fn parse_happen_event_npc_common(root: &Value) -> HappenEventNpcCommon {
    HappenEventNpcCommon {
        weapon_info: parse_all(root, "m_EventWeaponInfo", EventWeaponInfo::from_value),
        weapon_another_info: parse_all(
            root,
            "m_EventWeaponAnotherInfo",
            EventWeaponInfo::from_value,
        ),
        aimed_info: parse_all(root, "m_EventAimedInfo", EventAimedInfo::from_value),
        common_info: parse_all(root, "m_EventCommonInfo", EventCommonInfo::from_value),
    }
}

/// Parse toutes les entrées d'une liste (infaillible, positions préservées).
fn parse_all<T>(root: &Value, name: &str, f: impl Fn(&Value) -> T) -> Vec<T> {
    let mut out = Vec::new();
    if let Some(values) = list_values(root, name) {
        for v in values {
            out.push(f(v));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({
            "version": 100,
            "lists": [
                { "name": "m_EventWeaponInfo", "values": [
                    { "weapon_id": "0xC4C2C944", "weight": 3 },
                    { "weapon_id": "0xC500A373", "weight": 1 }
                ]},
                { "name": "m_EventAimedInfo", "values": [
                    { "npc_type_id": "0x11F1F540", "on_loop_mot_id": "0x00000000", "face_id": "0x00000000",
                      "ow_charaparam_id": "0xEF12775F", "weapon_info": [0, 2], "on_loop_npc_act_type": -1,
                      "is_not_battle_in": false }
                ]},
                { "name": "m_EventCommonInfo", "values": [
                    { "event_id": "0xF03D7EB5", "event_type": 7, "limit_num": 0, "recast_max": 0,
                      "recast_min": 0, "nice_point_max": 1000, "nice_point_min": 1000,
                      "event_name_text_id": "0xCCC2E53C", "on_loop_mot_id": "0x00000000", "face_id": "0x00000000",
                      "ow_charaparam_id": "0xEF12775F", "aimed_info": [0, 1], "weapon_info": [0, 0],
                      "on_loop_npc_act_type": -1, "cond": "AAAA", "y_times_text_id": "0xDEEE0BCC" }
                ]}
            ]
        })
    }

    #[test]
    fn parse_quatre_listes() {
        let cfg = parse_happen_event_npc_common(&fixture());
        assert_eq!(cfg.weapon_info.len(), 2);
        assert_eq!(cfg.weapon_another_info.len(), 0); // absente de la fixture
        assert_eq!(cfg.aimed_info.len(), 1);
        assert_eq!(cfg.common_info.len(), 1);
    }

    #[test]
    fn weapon_et_aimed_byte_exact() {
        let cfg = parse_happen_event_npc_common(&fixture());
        assert_eq!(
            cfg.weapon_info[0],
            EventWeaponInfo {
                weapon_id: HashId(0xC4C2_C944),
                weight: 3
            }
        );
        let a = &cfg.aimed_info[0];
        assert_eq!(a.npc_type_id, HashId(0x11F1_F540));
        assert_eq!(a.on_loop_mot_id, HashId::ZERO); // 0x00000000 = aucune motion
        assert_eq!(a.ow_charaparam_id, HashId(0xEF12_775F));
        assert_eq!(a.weapon_info, [0, 2]);
        assert_eq!(a.on_loop_npc_act_type, -1);
        assert!(!a.is_not_battle_in);
    }

    #[test]
    fn common_info_et_resolution_imbriquee() {
        let cfg = parse_happen_event_npc_common(&fixture());
        let e = &cfg.common_info[0];
        assert_eq!(e.event_id, HashId(0xF03D_7EB5));
        assert_eq!(e.event_type, 7);
        assert_eq!(e.nice_point_max, 1000);
        assert_eq!(e.y_times_text_id, HashId(0xDEEE_0BCC));
        assert_eq!(e.cond, "AAAA");
        // event[0].aimed_info [0,1] → aimed[0] ; aimed[0].weapon_info [0,2] → weapon[0],weapon[1].
        let aimed = cfg.aimed_of_event(e);
        assert_eq!(aimed.len(), 1);
        let weapons = cfg.weapons_of_aimed(&aimed[0]);
        assert_eq!(weapons.len(), 2);
        assert_eq!(weapons[1].weapon_id, HashId(0xC500_A373));
    }
}
