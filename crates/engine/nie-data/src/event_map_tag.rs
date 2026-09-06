//! `event_map_tag` — port de `event/event_map_tag_config_*.cfg.bin` (Level-5 IEVR).
//!
//! Associe les **événements overworld** à leurs **tags de map** (marqueurs/positions). Famille non
//! couverte ; port direct. Deux chaînes de tranches `[offset,count]` : un *setting* d'event →
//! liste d'ids de tag ; une *info* de tag → données de tag.
//!
//! Vérité terrain : `event_map_tag_config_0.00.00.cfg.bin.json` —
//! `m_mapTagDataList` (98) / `m_mapTagInfoList` (43) / `m_eventMapTagIdList` (200) /
//! `m_eventMapTagSettingList` (147).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_pair, list_values};
use crate::hash::HashId;

/// `m_mapTagDataList` — une donnée de tag de map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapTagData {
    /// `mapTagName` — hash du nom du tag.
    pub map_tag_name: HashId,
    /// `invisible` — tag invisible.
    pub invisible: bool,
}

/// `m_mapTagInfoList` — une info de tag (id + tranche de données).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapTagInfo {
    /// `id` — hash de l'info de tag.
    pub id: HashId,
    /// `data` — `[offset, count]` indexant `m_mapTagDataList`.
    pub data: [i64; 2],
}

/// `m_eventMapTagSettingList` — un réglage par event (id + tranche d'ids de tag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventMapTagSetting {
    /// `eventId` — hash de l'événement.
    pub event_id: HashId,
    /// `tagIdListRef` — `[offset, count]` indexant `m_eventMapTagIdList`.
    pub tag_id_list_ref: [i64; 2],
}

/// Contenu d'un `event_map_tag_config_*.cfg.bin`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventMapTagConfig {
    /// `m_mapTagDataList` — données de tag (indexées par `MapTagInfo::data`).
    pub tag_data: Vec<MapTagData>,
    /// `m_mapTagInfoList` — infos de tag.
    pub tag_infos: Vec<MapTagInfo>,
    /// `m_eventMapTagIdList` — ids de tag (indexés par `EventMapTagSetting::tag_id_list_ref`).
    pub tag_ids: Vec<HashId>,
    /// `m_eventMapTagSettingList` — réglages par event.
    pub event_settings: Vec<EventMapTagSetting>,
}

impl EventMapTagConfig {
    /// Ids de tag d'un event.
    #[must_use]
    pub fn tags_of_event(&self, s: &EventMapTagSetting) -> &[HashId] {
        slice(&self.tag_ids, s.tag_id_list_ref)
    }
    /// Données de tag d'une info.
    #[must_use]
    pub fn data_of_info(&self, i: &MapTagInfo) -> &[MapTagData] {
        slice(&self.tag_data, i.data)
    }
}

fn slice<T>(list: &[T], range: [i64; 2]) -> &[T] {
    let n = list.len();
    let start = (range[0].max(0) as usize).min(n);
    let end = start.saturating_add(range[1].max(0) as usize).min(n);
    list.get(start..end).unwrap_or(&[])
}

/// Parse un `event_map_tag_config_*.cfg.bin.json`.
#[must_use]
pub fn parse_event_map_tag_config(root: &Value) -> EventMapTagConfig {
    let mut tag_data = Vec::new();
    if let Some(values) = list_values(root, "m_mapTagDataList") {
        for v in values {
            tag_data.push(MapTagData {
                map_tag_name: field_hash(v, "mapTagName"),
                invisible: field_bool(v, "invisible").unwrap_or(false),
            });
        }
    }
    let mut tag_infos = Vec::new();
    if let Some(values) = list_values(root, "m_mapTagInfoList") {
        for v in values {
            if let Some(data) = field_pair(v, "data") {
                tag_infos.push(MapTagInfo {
                    id: field_hash(v, "id"),
                    data,
                });
            }
        }
    }
    let mut tag_ids = Vec::new();
    if let Some(values) = list_values(root, "m_eventMapTagIdList") {
        for v in values {
            tag_ids.push(field_hash(v, "tagId"));
        }
    }
    let mut event_settings = Vec::new();
    if let Some(values) = list_values(root, "m_eventMapTagSettingList") {
        for v in values {
            if let Some(tag_id_list_ref) = field_pair(v, "tagIdListRef") {
                event_settings.push(EventMapTagSetting {
                    event_id: field_hash(v, "eventId"),
                    tag_id_list_ref,
                });
            }
        }
    }
    EventMapTagConfig {
        tag_data,
        tag_infos,
        tag_ids,
        event_settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_et_resolution() {
        let root = json!({
            "lists": [
                { "name": "m_mapTagDataList", "values": [
                    { "mapTagName": "0x705841BC", "invisible": true }
                ]},
                { "name": "m_mapTagInfoList", "values": [{ "id": "0x86B77A27", "data": [0, 1] }] },
                { "name": "m_eventMapTagIdList", "values": [{ "tagId": "0x4E5690B9" }] },
                { "name": "m_eventMapTagSettingList", "values": [
                    { "eventId": "0x7D1E95AC", "tagIdListRef": [0, 1] }
                ]}
            ]
        });
        let cfg = parse_event_map_tag_config(&root);
        assert_eq!(cfg.tag_data.len(), 1);
        assert!(cfg.tag_data[0].invisible);
        assert_eq!(cfg.tag_data[0].map_tag_name, HashId(0x7058_41BC));
        assert_eq!(cfg.tag_ids[0], HashId(0x4E56_90B9));
        let s = &cfg.event_settings[0];
        assert_eq!(s.event_id, HashId(0x7D1E_95AC));
        assert_eq!(cfg.tags_of_event(s), &[HashId(0x4E56_90B9)]);
        assert_eq!(cfg.data_of_info(&cfg.tag_infos[0]).len(), 1);
    }
}
