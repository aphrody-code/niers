#![allow(clippy::pedantic)]
#![allow(clippy::doc_lazy_continuation)]
//! Tests golden `weather` — valeurs réelles tirées de :
//! - `weather/weather_convert_0.00.00.cfg.bin.json`
//! - `weather/weather_schedule_0.00.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ
//!
//! ### weather_convert — m_weatherConvertList (8 entrées)
//!
//! - `[0]` : detailWeather=0, gameWeather=0, mapWeather=0  (ligne 9-11 du JSON)
//! - `[1]` : detailWeather=1, gameWeather=1, mapWeather=1  (ligne 13-15)
//! - `[2]` : detailWeather=2, gameWeather=1, mapWeather=3  (ligne 17-19)
//! - `[7]` : detailWeather=7, gameWeather=0, mapWeather=4  (ligne 45-47)
//!
//! ### weather_schedule — WEATHER_SCHEDULE_INFO_DATA_0 (ligne ~32-141)
//!
//! - `data_id` = HashId(0x8F2BD1BA) = `from_i64(-1892953670)`, var[0]
//! - `weather_type` = 1, var[1]
//! - `detail_weights` : 23 zéros, var[2..=24]
//! - `base_probability` = 30, var[25]
//! - `reserved` = 0, var[26]
//!
//! ### weather_schedule — WEATHER_SCHEDULE_INFO_DATA_1 (ligne ~144-255)
//!
//! - `data_id` = HashId(0x16228000) = `from_i64(371359744)`, var[0]
//! - `weather_type` = 2, var[1]
//! - `detail_weights[4]` = 1 (= var[6]), `detail_weights[20]` = 2 (= var[22])
//! - `base_probability` = 20, var[25]
//!
//! ### weather_schedule — WEATHER_SCHEDULE_INFO_0
//!
//! - `schedule_id` = HashId(0xB45F5437) = `from_i64(-1268820937)`, var[0]
//!
//! ### weather_schedule — WEATHER_REGION_INFO_0
//!
//! - `region_type` = 1, var[0]
//! - `schedule_ids[0]` = HashId(0xB45F5437), var[1]
//! - `schedule_ids[1]` = HashId(0xB7528AAD), var[2]

mod common;

use nie_data::hash::HashId;
use nie_data::weather::{
    WeatherConvert, WeatherRegionInfo, WeatherScheduleConfig, WeatherScheduleData,
    WeatherScheduleInfo, parse_weather_convert, parse_weather_schedule,
};
use serde_json::json;

// ─── Fixtures ─────────────────────────────────────────────────────────────────

/// Fixture pour `weather_convert_0.00.00.cfg.bin.json` (valeurs réelles, extrait).
fn fixture_convert() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_weatherConvertList",
                "typeName": "WEATHER_CONVERT",
                "values": [
                    // entrée 0 — dump réel lignes 9-11
                    { "detailWeather": 0, "gameWeather": 0, "mapWeather": 0 },
                    // entrée 1 — dump réel lignes 13-15
                    { "detailWeather": 1, "gameWeather": 1, "mapWeather": 1 },
                    // entrée 2 — dump réel lignes 17-19
                    { "detailWeather": 2, "gameWeather": 1, "mapWeather": 3 },
                    // entrée 3 — dump réel lignes 21-23
                    { "detailWeather": 3, "gameWeather": 1, "mapWeather": 1 },
                    // entrée 4 — dump réel lignes 25-27
                    { "detailWeather": 4, "gameWeather": 2, "mapWeather": 2 },
                    // entrée 5 — dump réel lignes 29-31
                    { "detailWeather": 5, "gameWeather": 2, "mapWeather": 2 },
                    // entrée 6 — dump réel lignes 33-35
                    { "detailWeather": 6, "gameWeather": 2, "mapWeather": 2 },
                    // entrée 7 — dump réel lignes 37-39
                    { "detailWeather": 7, "gameWeather": 0, "mapWeather": 4 }
                ]
            }
        ]
    })
}

/// Fixture pour `weather_schedule_0.00.00.cfg.bin.json` (extrait minimal).
///
/// Contient un seul schedule avec deux DATA pour tester le parsing de base.
fn fixture_schedule() -> serde_json::Value {
    // Format entries : noeuds hiérarchiques avec variables positionnelles.
    // Toutes les valeurs sont issues du dump réel (DATA_0 et DATA_1).
    json!({
        "entries": [
            {
                "name": "WEATHER_SCHEDULE_INFO_LIST_BEG_0",
                "variables": [{ "type": "Int", "value": "1" }],
                "children": [
                    {
                        // WEATHER_SCHEDULE_INFO_0 — dump réel : var[0]=-1268820937
                        "name": "WEATHER_SCHEDULE_INFO_0",
                        "variables": [{ "type": "Int", "value": "-1268820937" }],
                        "children": [
                            {
                                "name": "WEATHER_SCHEDULE_INFO_DATA_LIST_BEG_0",
                                "variables": [{ "type": "Int", "value": "2" }],
                                "children": [
                                    {
                                        // DATA_0 — dump réel lignes 32-141
                                        // var[0]=-1892953670, var[1]=1, var[2..24]=0x23, var[25]=30, var[26]=0
                                        "name": "WEATHER_SCHEDULE_INFO_DATA_0",
                                        "variables": [
                                            { "type": "Int", "value": "-1892953670" },
                                            { "type": "Int", "value": "1" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "30" },
                                            { "type": "Int", "value": "0" }
                                        ],
                                        "children": []
                                    },
                                    {
                                        // DATA_1 — dump réel lignes 144-255
                                        // var[0]=371359744, var[1]=2, var[6]=1, var[22]=2, var[25]=20, var[26]=0
                                        "name": "WEATHER_SCHEDULE_INFO_DATA_1",
                                        "variables": [
                                            { "type": "Int", "value": "371359744" },
                                            { "type": "Int", "value": "2" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "1" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "2" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "0" },
                                            { "type": "Int", "value": "20" },
                                            { "type": "Int", "value": "0" }
                                        ],
                                        "children": []
                                    }
                                ]
                            }
                        ]
                    }
                ]
            },
            {
                // WEATHER_REGION_INFO_LIST_BEG_0
                "name": "WEATHER_REGION_INFO_LIST_BEG_0",
                "variables": [{ "type": "Int", "value": "1" }],
                "children": [
                    {
                        // WEATHER_REGION_INFO_0 — dump réel :
                        // var[0]=1, var[1]=-1268820937 (0xB45F5437), var[2]=-1219327315 (0xB7528AAD)
                        "name": "WEATHER_REGION_INFO_0",
                        "variables": [
                            { "type": "Int", "value": "1" },
                            { "type": "Int", "value": "-1268820937" },
                            { "type": "Int", "value": "-1219327315" }
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

// ─── Tests weather_convert (fixture) ─────────────────────────────────────────

#[test]
fn convert_compte_entrees() {
    let cfg = parse_weather_convert(&fixture_convert());
    // 8 entrées dans m_weatherConvertList — weather_convert_0.00.00 lignes 7-48
    assert_eq!(cfg.len(), 8, "8 entrées WeatherConvert");
}

#[test]
fn convert_entree_0_clair() {
    let cfg = parse_weather_convert(&fixture_convert());
    let e: &WeatherConvert = &cfg[0];
    // weather_convert_0.00.00 lignes 9-11
    assert_eq!(e.detail_weather, 0, "detailWeather[0]=0");
    assert_eq!(e.game_weather, 0, "gameWeather[0]=0 (clair)");
    assert_eq!(e.map_weather, 0, "mapWeather[0]=0");
}

#[test]
fn convert_entree_1_pluie() {
    let cfg = parse_weather_convert(&fixture_convert());
    let e: &WeatherConvert = &cfg[1];
    // weather_convert_0.00.00 lignes 13-15
    assert_eq!(e.detail_weather, 1);
    assert_eq!(e.game_weather, 1, "gameWeather=1 (pluie)");
    assert_eq!(e.map_weather, 1);
}

#[test]
fn convert_entree_2_detail_vers_map3() {
    let cfg = parse_weather_convert(&fixture_convert());
    let e: &WeatherConvert = &cfg[2];
    // weather_convert_0.00.00 lignes 17-19 : detailWeather=2 -> mapWeather=3
    assert_eq!(e.detail_weather, 2);
    assert_eq!(e.game_weather, 1);
    assert_eq!(e.map_weather, 3, "mapWeather=3 (distinct du gameWeather)");
}

#[test]
fn convert_entree_7_derniere() {
    let cfg = parse_weather_convert(&fixture_convert());
    let e: &WeatherConvert = &cfg[7];
    // weather_convert_0.00.00 lignes 37-39 (index 7)
    assert_eq!(e.detail_weather, 7);
    assert_eq!(
        e.game_weather, 0,
        "gameWeather=0 (clair) pour detailWeather=7"
    );
    assert_eq!(e.map_weather, 4);
}

// ─── Tests weather_schedule (fixture) ────────────────────────────────────────

#[test]
fn schedule_compte_total() {
    let cfg: WeatherScheduleConfig = parse_weather_schedule(&fixture_schedule());
    // Fixture : 1 schedule, 2 data entries, 1 région
    assert_eq!(
        cfg.schedules.len(),
        1,
        "1 WeatherScheduleInfo dans la fixture"
    );
    assert_eq!(
        cfg.data_entries.len(),
        2,
        "2 WeatherScheduleData dans la fixture"
    );
    assert!(cfg.region_info.is_some(), "region_info présent");
}

#[test]
fn schedule_info_0_hash() {
    let cfg = parse_weather_schedule(&fixture_schedule());
    let s: &WeatherScheduleInfo = &cfg.schedules[0];
    // WEATHER_SCHEDULE_INFO_0.var[0] = -1268820937 -> HashId(0xB45F5437)
    assert_eq!(
        s.schedule_id,
        HashId(0xB45F_5437),
        "schedule_id = 0xB45F5437"
    );
}

#[test]
fn data_0_champs_base() {
    let cfg = parse_weather_schedule(&fixture_schedule());
    let d: &WeatherScheduleData = &cfg.data_entries[0];
    // DATA_0.var[0]=-1892953670 -> HashId(0x8F2BD1BA)
    assert_eq!(
        d.data_id,
        HashId(0x8F2B_D1BA),
        "DATA_0 data_id = 0x8F2BD1BA"
    );
    // DATA_0.var[1]=1
    assert_eq!(d.weather_type, 1, "DATA_0 weather_type=1");
    // DATA_0.var[25]=30 (base_probability)
    assert_eq!(d.base_probability, 30, "DATA_0 base_probability=30");
    // DATA_0.var[26]=0 (reserved)
    assert_eq!(d.reserved, 0, "DATA_0 reserved=0");
    // detail_weights : 23 valeurs, toutes zéro pour DATA_0
    assert_eq!(d.detail_weights.len(), 23, "23 detail_weights");
    assert!(
        d.detail_weights.iter().all(|&w| w == 0),
        "DATA_0 : tous les detail_weights sont nuls"
    );
}

#[test]
fn data_1_slots_non_nuls() {
    let cfg = parse_weather_schedule(&fixture_schedule());
    let d: &WeatherScheduleData = &cfg.data_entries[1];
    // DATA_1.var[0]=371359744 -> HashId(0x16228000)
    assert_eq!(
        d.data_id,
        HashId(0x1622_8000),
        "DATA_1 data_id = 0x16228000"
    );
    // DATA_1.var[1]=2
    assert_eq!(d.weather_type, 2, "DATA_1 weather_type=2");
    // DATA_1.var[6]=1 -> detail_weights[4]=1 (var[6] = index 6 - 2 = slot 4)
    assert_eq!(
        d.detail_weights[4], 1,
        "DATA_1 detail_weights[4]=1 (var[6])"
    );
    // DATA_1.var[22]=2 -> detail_weights[20]=2 (var[22] = index 22 - 2 = slot 20)
    assert_eq!(
        d.detail_weights[20], 2,
        "DATA_1 detail_weights[20]=2 (var[22])"
    );
    // DATA_1.var[25]=20
    assert_eq!(d.base_probability, 20, "DATA_1 base_probability=20");
}

#[test]
fn region_info_champs() {
    let cfg = parse_weather_schedule(&fixture_schedule());
    let r: &WeatherRegionInfo = cfg.region_info.as_ref().unwrap();
    // WEATHER_REGION_INFO_0.var[0]=1
    assert_eq!(r.region_type, 1, "region_type=1");
    // var[1]=-1268820937 -> 0xB45F5437
    assert_eq!(
        r.schedule_ids[0],
        HashId(0xB45F_5437),
        "schedule_ids[0]=0xB45F5437"
    );
    // var[2]=-1219327315 -> 0xB7528AAD
    assert_eq!(
        r.schedule_ids[1],
        HashId(0xB752_8AAD),
        "schedule_ids[1]=0xB7528AAD"
    );
    assert_eq!(r.schedule_ids.len(), 2, "2 schedule_ids");
}

#[test]
fn find_schedule_existant() {
    let cfg = parse_weather_schedule(&fixture_schedule());
    assert!(
        cfg.find_schedule(HashId(0xB45F_5437)).is_some(),
        "find_schedule(0xB45F5437) doit réussir"
    );
}

#[test]
fn find_schedule_absent() {
    let cfg = parse_weather_schedule(&fixture_schedule());
    assert!(
        cfg.find_schedule(HashId(0xDEAD_BEEF)).is_none(),
        "find_schedule(0xDEADBEEF) doit échouer"
    );
}

// ─── Tests sur les vrais fichiers (skip silencieux si absents du VPS) ─────────

const REAL_CONVERT_PATH: &str = "weather/weather_convert_0.00.00.cfg.bin.json";
const REAL_SCHEDULE_PATH: &str = "weather/weather_schedule_0.00.00.cfg.bin.json";

fn load_real_convert() -> Option<Vec<nie_data::weather::WeatherConvert>> {
    let chemin_abs = common::chemin(REAL_CONVERT_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_weather_convert(&root))
}

fn load_real_schedule() -> Option<WeatherScheduleConfig> {
    let chemin_abs = common::chemin(REAL_SCHEDULE_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_weather_schedule(&root))
}

#[test]
fn real_file_convert_comptes() {
    let Some(cfg) = load_real_convert() else {
        return;
    };
    // weather_convert_0.00.00 : exactement 8 entrées (detailWeather 0..7)
    assert_eq!(
        cfg.len(),
        8,
        "8 entrées WeatherConvert dans le vrai fichier"
    );
}

#[test]
fn real_file_convert_entree_0() {
    let Some(cfg) = load_real_convert() else {
        return;
    };
    // weather_convert_0.00.00.cfg.bin.json ligne 9-11
    let e = &cfg[0];
    assert_eq!(e.detail_weather, 0);
    assert_eq!(e.game_weather, 0);
    assert_eq!(e.map_weather, 0);
}

#[test]
fn real_file_convert_entree_2_map3() {
    let Some(cfg) = load_real_convert() else {
        return;
    };
    // weather_convert_0.00.00.cfg.bin.json ligne 17-19
    let e = &cfg[2];
    assert_eq!(e.detail_weather, 2);
    assert_eq!(e.game_weather, 1);
    assert_eq!(e.map_weather, 3, "detailWeather=2 -> mapWeather=3");
}

#[test]
fn real_file_convert_entree_7() {
    let Some(cfg) = load_real_convert() else {
        return;
    };
    // weather_convert_0.00.00.cfg.bin.json ligne 37-39
    let e = &cfg[7];
    assert_eq!(e.detail_weather, 7);
    assert_eq!(e.game_weather, 0);
    assert_eq!(e.map_weather, 4);
}

#[test]
fn real_file_schedule_comptes() {
    let Some(cfg) = load_real_schedule() else {
        return;
    };
    // weather_schedule_0.00.00 : 2 schedules, 18 données, 1 région
    assert_eq!(cfg.schedules.len(), 2, "2 WeatherScheduleInfo");
    assert_eq!(
        cfg.data_entries.len(),
        18,
        "18 WeatherScheduleData (9 par schedule)"
    );
    assert!(cfg.region_info.is_some(), "1 WeatherRegionInfo");
}

#[test]
fn real_file_schedule_info_0_hash() {
    let Some(cfg) = load_real_schedule() else {
        return;
    };
    // WEATHER_SCHEDULE_INFO_0.var[0] = -1268820937 -> HashId(0xB45F5437)
    assert_eq!(
        cfg.schedules[0].schedule_id,
        HashId(0xB45F_5437),
        "schedule 0 id = 0xB45F5437"
    );
}

#[test]
fn real_file_schedule_info_1_hash() {
    let Some(cfg) = load_real_schedule() else {
        return;
    };
    // WEATHER_SCHEDULE_INFO_1.var[0] = -1219327315 -> HashId(0xB7528AAD)
    assert_eq!(
        cfg.schedules[1].schedule_id,
        HashId(0xB752_8AAD),
        "schedule 1 id = 0xB7528AAD"
    );
}

#[test]
fn real_file_data_0_hash_et_probabilite() {
    let Some(cfg) = load_real_schedule() else {
        return;
    };
    // WEATHER_SCHEDULE_INFO_DATA_0.var[0]=-1892953670 -> 0x8F2BD1BA, var[1]=1, var[25]=30
    let d = &cfg.data_entries[0];
    assert_eq!(d.data_id, HashId(0x8F2B_D1BA), "DATA_0 data_id");
    assert_eq!(d.weather_type, 1, "DATA_0 weather_type=1");
    assert_eq!(d.base_probability, 30, "DATA_0 base_probability=30");
    assert_eq!(d.detail_weights.len(), 23, "DATA_0 : 23 detail_weights");
}

#[test]
fn real_file_data_1_detail_weights() {
    let Some(cfg) = load_real_schedule() else {
        return;
    };
    // DATA_1.var[6]=1 -> detail_weights[4]=1 ; var[22]=2 -> detail_weights[20]=2
    let d = &cfg.data_entries[1];
    assert_eq!(
        d.data_id,
        HashId(0x1622_8000),
        "DATA_1 data_id = 0x16228000"
    );
    assert_eq!(d.weather_type, 2, "DATA_1 weather_type=2");
    assert_eq!(
        d.detail_weights[4], 1,
        "DATA_1 detail_weights[4]=1 (var[6])"
    );
    assert_eq!(
        d.detail_weights[20], 2,
        "DATA_1 detail_weights[20]=2 (var[22])"
    );
    assert_eq!(d.base_probability, 20, "DATA_1 base_probability=20");
    assert_eq!(d.reserved, 0, "DATA_1 reserved=0");
}

#[test]
fn real_file_schedule_somme_probabilites() {
    let Some(cfg) = load_real_schedule() else {
        return;
    };
    // Chaque groupe de 9 données doit sommer à 100 en base_probability
    let schedule_0_probs: i64 = cfg.data_entries[0..9]
        .iter()
        .map(|d| d.base_probability)
        .sum();
    let schedule_1_probs: i64 = cfg.data_entries[9..18]
        .iter()
        .map(|d| d.base_probability)
        .sum();
    assert_eq!(
        schedule_0_probs, 100,
        "schedule 0 : somme base_probability = 100"
    );
    assert_eq!(
        schedule_1_probs, 100,
        "schedule 1 : somme base_probability = 100"
    );
}

#[test]
fn real_file_region_info() {
    let Some(cfg) = load_real_schedule() else {
        return;
    };
    // WEATHER_REGION_INFO_0 : region_type=1, schedule_ids=[0xB45F5437, 0xB7528AAD]
    let r = cfg.region_info.as_ref().unwrap();
    assert_eq!(r.region_type, 1, "region_type=1");
    assert_eq!(r.schedule_ids.len(), 2, "2 schedule_ids");
    assert_eq!(
        r.schedule_ids[0],
        HashId(0xB45F_5437),
        "schedule_ids[0]=0xB45F5437"
    );
    assert_eq!(
        r.schedule_ids[1],
        HashId(0xB752_8AAD),
        "schedule_ids[1]=0xB7528AAD"
    );
}

#[test]
fn real_file_data_9_schedule1_debut() {
    let Some(cfg) = load_real_schedule() else {
        return;
    };
    // DATA_9.var[0]=933371288 -> HashId(0x37A21D98), var[1]=5, var[25]=30
    let d = &cfg.data_entries[9];
    assert_eq!(
        d.data_id,
        HashId(0x37A2_1D98),
        "DATA_9 data_id = 0x37A21D98"
    );
    assert_eq!(d.weather_type, 5, "DATA_9 weather_type=5 (schedule 1)");
    assert_eq!(d.base_probability, 30, "DATA_9 base_probability=30");
}

#[test]
fn real_file_tous_detail_weights_23_valeurs() {
    let Some(cfg) = load_real_schedule() else {
        return;
    };
    // Chaque WeatherScheduleData doit avoir exactement 23 detail_weights
    for (i, d) in cfg.data_entries.iter().enumerate() {
        assert_eq!(
            d.detail_weights.len(),
            23,
            "DATA_{i} doit avoir 23 detail_weights"
        );
    }
}
