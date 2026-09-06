//! `soccer_map_env` — port de `soccer/soccer_game_map_enviroment_config_*.cfg.bin` (Level-5 IEVR).
//!
//! **Environnement de match** : heure, météo, attribut de sol par configuration de terrain, +
//! tags de bit-flag global activables. Famille non couverte ; port direct (typo Level-5
//! « enviroment » préservée dans le nom de fichier).
//!
//! Vérité terrain : `m_soccerGameMapTagDataList` (28) / `m_soccerGameMapEnvList` (29).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_pair, list_values};
use crate::hash::HashId;

/// `m_soccerGameMapTagDataList` — un tag de bit-flag global activable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapTagData {
    /// `globalBitFlagCrc` — crc du bit-flag global.
    pub global_bit_flag_crc: HashId,
    /// `isEnable` — activé.
    pub is_enable: bool,
}

/// `m_soccerGameMapEnvList` — l'environnement d'une configuration de terrain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapEnv {
    /// `configId` — hash de la config de terrain.
    pub config_id: HashId,
    /// `hour` — heure (0-23).
    pub hour: i64,
    /// `weather` — code météo.
    pub weather: i64,
    /// `groundAttr` — hash d'attribut de sol (`0` = défaut).
    pub ground_attr: HashId,
    /// `refMapTag` — `[offset, count]` indexant `m_soccerGameMapTagDataList`.
    pub ref_map_tag: [i64; 2],
}

/// Contenu d'un `soccer_game_map_enviroment_config_*.cfg.bin`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerMapEnvConfig {
    /// `m_soccerGameMapTagDataList` — tags de bit-flag (indexés par `MapEnv::ref_map_tag`).
    pub tag_data: Vec<MapTagData>,
    /// `m_soccerGameMapEnvList` — environnements par config de terrain.
    pub envs: Vec<MapEnv>,
}

impl SoccerMapEnvConfig {
    /// Tags d'un environnement.
    #[must_use]
    pub fn tags_of(&self, e: &MapEnv) -> &[MapTagData] {
        let n = self.tag_data.len();
        let start = (e.ref_map_tag[0].max(0) as usize).min(n);
        let end = start
            .saturating_add(e.ref_map_tag[1].max(0) as usize)
            .min(n);
        self.tag_data.get(start..end).unwrap_or(&[])
    }
}

/// Parse un `soccer_game_map_enviroment_config_*.cfg.bin.json`.
#[must_use]
pub fn parse_soccer_map_env_config(root: &Value) -> SoccerMapEnvConfig {
    let mut tag_data = Vec::new();
    if let Some(values) = list_values(root, "m_soccerGameMapTagDataList") {
        for v in values {
            tag_data.push(MapTagData {
                global_bit_flag_crc: field_hash(v, "globalBitFlagCrc"),
                is_enable: field_bool(v, "isEnable").unwrap_or(false),
            });
        }
    }
    let mut envs = Vec::new();
    if let Some(values) = list_values(root, "m_soccerGameMapEnvList") {
        for v in values {
            let config_id = field_hash(v, "configId");
            if !config_id.is_zero() {
                envs.push(MapEnv {
                    config_id,
                    hour: field_i64(v, "hour").unwrap_or(0),
                    weather: field_i64(v, "weather").unwrap_or(0),
                    ground_attr: field_hash(v, "groundAttr"),
                    ref_map_tag: field_pair(v, "refMapTag").unwrap_or([0, 0]),
                });
            }
        }
    }
    SoccerMapEnvConfig { tag_data, envs }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_env() {
        let root = json!({
            "lists": [
                { "name": "m_soccerGameMapTagDataList", "values": [
                    { "globalBitFlagCrc": "0xDA4D4E24", "isEnable": true }
                ]},
                { "name": "m_soccerGameMapEnvList", "values": [
                    { "configId": "0xE35E00DF", "hour": 15, "weather": 1, "groundAttr": "0x00000000", "refMapTag": [0, 1] }
                ]}
            ]
        });
        let cfg = parse_soccer_map_env_config(&root);
        assert_eq!(cfg.tag_data.len(), 1);
        assert!(cfg.tag_data[0].is_enable);
        assert_eq!(cfg.envs.len(), 1);
        let e = &cfg.envs[0];
        assert_eq!(e.config_id, HashId(0xE35E_00DF));
        assert_eq!(e.hour, 15);
        assert_eq!(e.weather, 1);
        assert_eq!(cfg.tags_of(e).len(), 1);
    }
}
