//! `WeatherConfig` — port des familles `weather_convert` et `weather_schedule` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! Deux fichiers dans `/home/ubuntu/niers/data/common/gamedata/weather/` :
//!
//! | Fichier | Format | Taille |
//! |---------|--------|--------|
//! | `weather_convert_0.00.00.cfg.bin.json` | `lists` | 450 o |
//! | `weather_schedule_0.00.00.cfg.bin.json` | `entries` | 64 262 o |
//!
//! ## `weather_convert` (format `lists`)
//!
//! Une seule liste `m_weatherConvertList` (type `WEATHER_CONVERT`), 8 entrées :
//! chaque entrée associe un `detailWeather` (météo fine, 0–7) à un `gameWeather`
//! (météo gameplay, 0–2) et un `mapWeather` (météo de la carte, 0–4).
//!
//! | `detailWeather` | `gameWeather` | `mapWeather` |
//! |-----------------|---------------|--------------|
//! | 0 | 0 | 0 |
//! | 1 | 1 | 1 |
//! | 2 | 1 | 3 |
//! | … | … | … |
//! | 7 | 0 | 4 |
//!
//! ## `weather_schedule` (format `entries`)
//!
//! Hiérarchie :
//! - `WEATHER_SCHEDULE_INFO_LIST_BEG_0` — conteneur (2 schedules)
//!   - `WEATHER_SCHEDULE_INFO_{n}` — identifiant du schedule (1 var = HashId)
//!     - `WEATHER_SCHEDULE_INFO_DATA_LIST_BEG_{n}` — 9 entrées de données
//!       - `WEATHER_SCHEDULE_INFO_DATA_{i}` — **27 vars** chacun
//! - `WEATHER_REGION_INFO_LIST_BEG_0` — 1 région
//!   - `WEATHER_REGION_INFO_0` — 3 vars : type + 2 hashes de schedule
//!
//! ### Layout `WEATHER_SCHEDULE_INFO_DATA_*` (27 variables positionnelles)
//!
//! | var | type | sémantique observée |
//! |-----|------|---------------------|
//! | 0 | HashId | identifiant de la donnée |
//! | 1 | i64 | type de météo (correspond à `gameWeather` dans `WeatherConvert`) |
//! | 2..=24 | i64 | poids détaillés par sous-slot (23 valeurs, sémantique interne) |
//! | 25 | i64 | probabilité de base (% sur 100 ; les 9 entrées d'un schedule somment à 100) |
//! | 26 | i64 | réservé (toujours 0 dans le dump réel) |
//!
//! Comptes réels : 2 schedules × 9 données = **18 `WeatherScheduleData`** ;
//! 2 `WeatherScheduleInfo` ; 1 `WeatherRegionInfo`.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, field_i64, list_values, walk_named};
use crate::hash::HashId;

// ─── WeatherConvert ───────────────────────────────────────────────────────────

/// Correspondance météo fine → météo gameplay + météo carte (`WEATHER_CONVERT`).
///
/// Vérité terrain : `weather_convert_0.00.00.cfg.bin.json`, liste `m_weatherConvertList`.
/// 8 entrées (detailWeather 0–7).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeatherConvert {
    /// `detailWeather` — indice météo détaillée (0–7 dans le dump réel).
    pub detail_weather: i64,
    /// `gameWeather` — catégorie météo gameplay (0=clair, 1=pluie, 2=neige dans le dump).
    pub game_weather: i64,
    /// `mapWeather` — catégorie météo carte (0–4 dans le dump réel).
    pub map_weather: i64,
}

impl WeatherConvert {
    /// Parse une entrée de `m_weatherConvertList`. `None` si les champs requis sont absents.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            detail_weather: field_i64(v, "detailWeather")?,
            game_weather: field_i64(v, "gameWeather")?,
            map_weather: field_i64(v, "mapWeather")?,
        })
    }
}

// ─── WeatherScheduleData ──────────────────────────────────────────────────────

/// Entrée `WEATHER_SCHEDULE_INFO_DATA_*` — une condition météo dans un schedule.
///
/// Chacune des 27 variables est positionnelle (format `entries` cfgbin).
/// La somme des `base_probability` des 9 entrées d'un même schedule vaut 100
/// (probabilité en points de pourcentage).
///
/// Vérité terrain : `weather_schedule_0.00.00.cfg.bin.json`, entrée DATA_0 :
/// `[-1892953670, 1, 0×23, 30, 0]` → `data_id=0x8F2BD1BA`, `weather_type=1`,
/// `base_probability=30`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeatherScheduleData {
    /// var\[0\] — identifiant de cette donnée météo.
    ///
    /// Vérité terrain : DATA_0 → 0x8F2BD1BA ; DATA_1 → 0x16228000.
    pub data_id: HashId,
    /// var\[1\] — type de météo (référence `gameWeather` dans [`WeatherConvert`]).
    ///
    /// Vérité terrain : schedule 0 : types observés = 1 (×8), 2 (×1) ;
    /// schedule 1 : type = 5 pour toutes les 9 entrées.
    pub weather_type: i64,
    /// var\[2\]..=var\[24\] — poids détaillés par sous-slot (23 valeurs, sémantique interne
    /// non documentée dans les sources disponibles).
    ///
    /// Vérité terrain : DATA_1 : `detail_weights[4]=1, detail_weights[20]=2`, reste=0.
    pub detail_weights: Vec<i64>,
    /// var\[25\] — probabilité de base en points de pourcentage.
    ///
    /// Les 9 entrées d'un même [`WeatherScheduleInfo`] somment à 100.
    /// Vérité terrain : schedule 0 : 30+20+6+14+7+6+6+6+5 = 100.
    pub base_probability: i64,
    /// var\[26\] — réservé, toujours 0 dans le dump réel.
    pub reserved: i64,
}

impl WeatherScheduleData {
    /// Parse un noeud `WEATHER_SCHEDULE_INFO_DATA_*`. `None` si `data_id` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let data_id = node.hash(0);
        if data_id.is_zero() {
            return None;
        }
        let mut detail_weights = Vec::with_capacity(23);
        for i in 2..=24_usize {
            detail_weights.push(node.int(i));
        }
        Some(Self {
            data_id,
            weather_type: node.int(1),
            detail_weights,
            base_probability: node.int(25),
            reserved: node.int(26),
        })
    }
}

// ─── WeatherScheduleInfo ──────────────────────────────────────────────────────

/// Entrée `WEATHER_SCHEDULE_INFO_*` — identifiant d'un schedule météo.
///
/// Contient uniquement un hash d'identification ; les données détaillées sont dans
/// [`WeatherScheduleData`]. Deux schedules dans le dump réel :
/// - `WEATHER_SCHEDULE_INFO_0` → `schedule_id = 0xB45F5437`
/// - `WEATHER_SCHEDULE_INFO_1` → `schedule_id = 0xB7528AAD`
///
/// Vérité terrain : `weather_schedule_0.00.00.cfg.bin.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeatherScheduleInfo {
    /// var\[0\] — hash identifiant ce schedule (même valeur référencée dans
    /// [`WeatherRegionInfo::schedule_ids`]).
    pub schedule_id: HashId,
}

impl WeatherScheduleInfo {
    /// Parse un noeud `WEATHER_SCHEDULE_INFO_*`. `None` si `schedule_id` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let schedule_id = node.hash(0);
        if schedule_id.is_zero() {
            return None;
        }
        Some(Self { schedule_id })
    }
}

// ─── WeatherRegionInfo ────────────────────────────────────────────────────────

/// Entrée `WEATHER_REGION_INFO_*` — association région ↔ schedules météo.
///
/// Dans le dump réel, une seule région (`WEATHER_REGION_INFO_0`) référençant
/// deux schedules (`0xB45F5437` et `0xB7528AAD`).
///
/// Vérité terrain : `weather_schedule_0.00.00.cfg.bin.json`, WEATHER_REGION_INFO_0 :
/// vars = `[1, -1268820937, -1219327315]` → `region_type=1`,
/// `schedule_ids=[0xB45F5437, 0xB7528AAD]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeatherRegionInfo {
    /// var\[0\] — type ou indice de la région (vaut 1 dans le dump réel).
    pub region_type: i64,
    /// var\[1\], var\[2\], … — hashes des [`WeatherScheduleInfo`] assignés.
    ///
    /// Vérité terrain : `[HashId(0xB45F5437), HashId(0xB7528AAD)]`.
    pub schedule_ids: Vec<HashId>,
}

impl WeatherRegionInfo {
    /// Parse un noeud `WEATHER_REGION_INFO_*`. `None` si `var_count < 2`.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        if node.var_count() < 2 {
            return None;
        }
        let region_type = node.int(0);
        let mut schedule_ids = Vec::new();
        let mut i = 1;
        loop {
            let h = node.hash(i);
            if h.is_zero() {
                break;
            }
            schedule_ids.push(h);
            i += 1;
        }
        if schedule_ids.is_empty() {
            return None;
        }
        Some(Self {
            region_type,
            schedule_ids,
        })
    }
}

// ─── WeatherScheduleConfig ────────────────────────────────────────────────────

/// Contenu complet d'un `weather_schedule_*.cfg.bin.json`.
///
/// Utiliser [`parse_weather_schedule`] pour construire depuis un JSON désérialisé.
///
/// Comptes réels (dump `weather_schedule_0.00.00.cfg.bin.json`) :
/// - 2 [`WeatherScheduleInfo`]
/// - 18 [`WeatherScheduleData`] (9 par schedule)
/// - 1 [`WeatherRegionInfo`]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeatherScheduleConfig {
    /// Identifiants des schedules (`WEATHER_SCHEDULE_INFO_*`).
    pub schedules: Vec<WeatherScheduleInfo>,
    /// Données météo détaillées (`WEATHER_SCHEDULE_INFO_DATA_*`), toutes schedules confondus.
    pub data_entries: Vec<WeatherScheduleData>,
    /// Association région → schedules (`WEATHER_REGION_INFO_*`).
    pub region_info: Option<WeatherRegionInfo>,
}

impl WeatherScheduleConfig {
    /// Recherche un schedule par son hash. `None` si absent.
    #[must_use]
    pub fn find_schedule(&self, schedule_id: HashId) -> Option<&WeatherScheduleInfo> {
        self.schedules.iter().find(|s| s.schedule_id == schedule_id)
    }
}

// ─── Parseurs principaux ──────────────────────────────────────────────────────

/// Parse un `weather_convert_*.cfg.bin.json` — liste `m_weatherConvertList`.
///
/// Renvoie les 8 [`WeatherConvert`] du dump réel. Les entrées invalides sont ignorées.
///
/// Vérité terrain : `weather_convert_0.00.00.cfg.bin.json` → 8 entrées.
#[must_use]
pub fn parse_weather_convert(root: &Value) -> Vec<WeatherConvert> {
    let mut out = Vec::new();
    if let Some(values) = list_values(root, "m_weatherConvertList") {
        for v in values {
            if let Some(item) = WeatherConvert::from_value(v) {
                out.push(item);
            }
        }
    }
    out
}

/// Parse un `weather_schedule_*.cfg.bin.json` complet.
///
/// Renvoie un [`WeatherScheduleConfig`] avec les trois collections remplies.
/// Les noeuds conteneurs (`*_LIST_BEG_*`) sont ignorés.
///
/// Comptes réels : 2 schedules, 18 données, 1 région.
#[must_use]
pub fn parse_weather_schedule(root: &Value) -> WeatherScheduleConfig {
    let mut schedules = Vec::new();
    walk_named(root, "WEATHER_SCHEDULE_INFO_", |node| {
        let name = node.name();
        if name.contains("_DATA_") || name.contains("_LIST_BEG_") {
            return;
        }
        if let Some(info) = WeatherScheduleInfo::from_node(node) {
            schedules.push(info);
        }
    });

    let mut data_entries = Vec::new();
    walk_named(root, "WEATHER_SCHEDULE_INFO_DATA_", |node| {
        if node.name().contains("_LIST_BEG_") {
            return;
        }
        if let Some(data) = WeatherScheduleData::from_node(node) {
            data_entries.push(data);
        }
    });

    let mut region_info = None;
    walk_named(root, "WEATHER_REGION_INFO_", |node| {
        if node.name().contains("_LIST_BEG_") {
            return;
        }
        if region_info.is_none() {
            region_info = WeatherRegionInfo::from_node(node);
        }
    });

    WeatherScheduleConfig {
        schedules,
        data_entries,
        region_info,
    }
}
