#![allow(clippy::pedantic)]
#![recursion_limit = "512"]
//! Tests golden `soccer` — valeurs réelles tirées des fichiers :
//! - `data/common/gamedata/soccer/soccer_focus_battle_effect_config.cfg.bin.json`
//! - `data/common/gamedata/soccer/soccer_technic_config.cfg.bin.json`
//! - `data/common/gamedata/soccer/soccer_basic_effect_config_1.01.79.00.cfg.bin.json`
//! - `data/common/gamedata/soccer/soccer_game_additional_config_1.04.14.00.cfg.bin.json`
//! - `data/common/gamedata/soccer/soccer_game_config_plus_1.04.08.00.cfg.bin.json`
//! - `data/common/gamedata/soccer/soccer_game_config_1.04.08.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ
//!
//! ### soccer_focus_battle_effect_config — m_soccerFocusBattleEffectRangeList[1]
//! - `rangeType` = 1 (actif)
//! - `rangeParam1` = 30 (dimension de zone de FocusBattle)
//! - `rangeParam2` = 10
//! - (lignes 15-20 du fichier JSON)
//!
//! ### soccer_focus_battle_effect_config — m_soccerFocusBattleDemoTriggerList[0]
//! - `demoTriggerType` = 3
//! - `demoTriggerTextId` = `"0x8901F53D"` → `HashId(0x8901F53D)`
//! - (lignes 190-198 du fichier JSON)
//!
//! ### soccer_focus_battle_effect_config — m_soccerFocusBattleEffectInfoList[0]
//! - `id` = `"0x64659E21"` → `HashId(0x64659E21)`
//! - `range` = `[0, 1]`, `activatedEffect` = `[0, 1]`, `demoTrigger` = `[0, 1]`
//! - (lignes 264-280 du fichier JSON)
//!
//! ### soccer_technic_config — m_soccerTechnicInfoList[0]
//! - `id` = `"0x07857A3F"` → `HashId(0x07857A3F)`
//! - `recastTime` = 5
//! - `focusBattleEffectId` = `"0x8A6BFF0D"` → `HashId(0x8A6BFF0D)`
//! - (lignes 104-114 du fichier JSON)
//!
//! ### soccer_basic_effect_config — m_soccerBasicEffectInfoList[0]
//! - `id` = `"0x83DB0758"` → `HashId(0x83DB0758)`
//! - `funcName` = `"0xC9D604D4"` → `HashId(0xC9D604D4)`
//! - `buildIconType` = 6
//! - (lignes 8-12 du fichier JSON)
//!
//! ### soccer_game_additional_config — m_SoccerGetExpDataTable[0]
//! - `id` = 0, `ratio` = 7.8
//! - (lignes 67-70 du fichier JSON)
//!
//! ### soccer_game_additional_config — m_SoccerGetExpDataTable[16]
//! - `id` = 16, `ratio` = 4.0 (dernière entrée)
//!
//! ### soccer_game_config_plus — SOCCER_GAME_DIFFICULTY_0
//! - var\[0\] = 1 → `difficulty_id` = 1
//! - var\[2\] = 40 → `raw[1]` = 40
//! - (lignes 13-17 du fichier JSON)
//!
//! ### soccer_game_config_plus — SOCCER_GAME_INFO_0
//! - var\[0\] = -1344098363 → `match_id` = 0xAFE2AFC5
//! - var\[1\] = `"fbtl_st_0904"` → `id_str` = `"fbtl_st_0904"`
//! - `SOCCER_GAME_INFO_REF_DIFFICULTY_0.var[0]` = 0 → `difficulty_index` = 0
//! - (lignes 216-260 du fichier JSON)
//!
//! ### soccer_game_config (full) — SOCCER_GAME_INFO_0
//! - var\[0\] = -778368270 → `match_id` = 0xD19B0AF2
//! - var\[1\] = `"fbtl_st_0101"` → `id_str` = `"fbtl_st_0101"`
//! - `difficulty_index` = 0

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::soccer::{
    parse_basic_effect_config, parse_focus_battle_effect_config,
    parse_soccer_game_additional_config, parse_soccer_game_config, parse_technic_config,
};
use serde_json::json;

// ─── Fixtures inline ──────────────────────────────────────────────────────────

fn fixture_focus_battle() -> serde_json::Value {
    // Extrait de soccer_focus_battle_effect_config.cfg.bin.json
    // ligne 15-20 : m_soccerFocusBattleEffectRangeList[1] (rangeType=1, rangeParam1=30)
    // ligne 190-198 : m_soccerFocusBattleDemoTriggerList[0] (demoTriggerType=3)
    // ligne 264-280 : m_soccerFocusBattleEffectInfoList[0]
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_soccerFocusBattleEffectRangeList",
                "typeName": "ScFbtlEffectRange",
                "values": [
                    { "rangeType": 0, "rangeParam1": 0, "rangeParam2": 0, "rangeParam3": 0 },
                    { "rangeType": 1, "rangeParam1": 30, "rangeParam2": 10, "rangeParam3": 0 }
                ]
            },
            {
                "name": "m_soccerFocusBattleActivatedEffectList",
                "typeName": "ScFbtlActivatedEffect",
                "values": [
                    {
                        "effectId": "0xBEBB2EE8",
                        "effectParam1": 0, "effectParam2": 0, "effectParam3": 0, "effectParam4": 0,
                        "effectParam5": 0, "effectParam6": 0, "effectParam7": 0, "effectParam8": 0
                    }
                ]
            },
            {
                "name": "m_soccerFocusBattleDemoTriggerList",
                "typeName": "ScFbtlDemoTrigger",
                "values": [
                    {
                        "demoTriggerType": 3,
                        "demoTriggerParam1": 0, "demoTriggerParam2": 0, "demoTriggerParam3": 0,
                        "demoTriggerTextId": "0x8901F53D"
                    }
                ]
            },
            {
                "name": "m_soccerFocusBattleEffectInfoList",
                "typeName": "ScFbtlEffectInfo",
                "values": [
                    {
                        "id": "0x64659E21",
                        "range": [0, 1],
                        "activatedEffect": [0, 1],
                        "demoTrigger": [0, 1]
                    }
                ]
            }
        ]
    })
}

fn fixture_technic() -> serde_json::Value {
    // Extrait de soccer_technic_config.cfg.bin.json
    // lignes 6-14 : m_soccerTechnicUsableConditionList[0] (type=1, param1=3, param2=30)
    // lignes 104-114 : m_soccerTechnicInfoList[0]
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_soccerTechnicUsableConditionList",
                "typeName": "ScTechnicUsableCondition",
                "values": [
                    { "type": 1, "param1": 3, "param2": 30, "param3": 0 },
                    { "type": 2, "param1": 1, "param2": 0, "param3": 0 }
                ]
            },
            {
                "name": "m_soccerTechnicInfoList",
                "typeName": "ScTechnicInfo",
                "values": [
                    {
                        "id": "0x07857A3F",
                        "nameTextId": "0x7B5D17D1",
                        "descTextId": "0x1702D47E",
                        "recastTime": 5,
                        "usableCondition": [0, 2],
                        "focusBattleEffectId": "0x8A6BFF0D"
                    }
                ]
            }
        ]
    })
}

fn fixture_basic_effect() -> serde_json::Value {
    // Extrait de soccer_basic_effect_config_1.01.79.00.cfg.bin.json
    // lignes 8-12 : m_soccerBasicEffectInfoList[0]
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_soccerBasicEffectInfoList",
                "typeName": "ScBasicEffectInfo",
                "values": [
                    { "id": "0x83DB0758", "funcName": "0xC9D604D4", "buildIconType": 6 },
                    { "id": "0x1AD256E2", "funcName": "0x7E90C35D", "buildIconType": 6 }
                ]
            }
        ]
    })
}

fn fixture_additional() -> serde_json::Value {
    // Extrait de soccer_game_additional_config_1.04.14.00.cfg.bin.json
    // lignes 7-12 : m_SoccerTeamAIDataList[0]
    // lignes 67-70 : m_SoccerGetExpDataTable[0]
    // lignes 140-148 : m_SoccerNicePlayExpDataList[0]
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_SoccerTeamAIDataList",
                "typeName": "SOCCER_TEAM_AI_DATA",
                "values": [
                    {
                        "id": "0x6FF00159",
                        "quoteId": "0x00000000",
                        "paramData": "01010101",
                        "strategyId": "0x1CE5F4E2"
                    }
                ]
            },
            {
                "name": "m_SoccerGetExpDataTable",
                "typeName": "SOCCER_GET_EXP_DATA",
                "values": [
                    { "id": 0, "ratio": 7.8 },
                    { "id": 1, "ratio": 7.3 }
                ]
            },
            {
                "name": "m_SoccerNicePlayExpDataList",
                "typeName": "SOCCER_NICE_PLAY_EXP_DATA",
                "values": [
                    {
                        "id": "0x439CE11B",
                        "shootExp": 10, "skillExp": 10, "focusExp": 10, "scrambleExp": 10
                    }
                ]
            }
        ]
    })
}

fn fixture_game_config_plus() -> serde_json::Value {
    // soccer_game_config_plus_1.04.08.00.cfg.bin.json — structure complète
    // lignes 13-199 : SOCCER_GAME_DIFFICULTY_0 (var[0]=1, var[2]=40)
    // lignes 216-288 : SOCCER_GAME_INFO_0 + SOCCER_GAME_INFO_REF_DIFFICULTY_0
    json!({
        "entries": [
            {
                "name": "SOCCER_GAME_DIFFICULTY_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    {
                        "name": "SOCCER_GAME_DIFFICULTY_0",
                        "variables": [
                            {"type": "Int", "value": "1"},
                            {"type": "Int", "value": "-587180464"},
                            {"type": "Int", "value": "40"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "896841888"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "1"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "106489223"},
                            {"type": "Int", "value": "2208"},
                            {"type": "Int", "value": "1134354715"},
                            {"type": "Int", "value": "-527581349"},
                            {"type": "Int", "value": "1712967157"},
                            {"type": "Int", "value": "-527581349"},
                            {"type": "Int", "value": "1712967157"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "-1"},
                            {"type": "Int", "value": "2102410747"},
                            {"type": "Int", "value": "588111552"},
                            {"type": "Int", "value": "30"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "1"},
                            {"type": "Int", "value": "1587478599"},
                            {"type": "Int", "value": "2108103800"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "-352149556"},
                            {"type": "Int", "value": "-450674566"},
                            {"type": "Int", "value": "-1287497974"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "13"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "-1287497974"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "-450674566"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "SOCCER_GAME_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}, {"type": "Int", "value": "1"}],
                "children": [
                    {
                        "name": "SOCCER_GAME_INFO_0",
                        "variables": [
                            {"type": "Int",    "value": "-1344098363"},
                            {"type": "String", "value": "fbtl_st_0904"},
                            {"type": "Int",    "value": "1"},
                            {"type": "Int",    "value": "-724197456"},
                            {"type": "Int",    "value": "0"},
                            {"type": "Int",    "value": "292017266"},
                            {"type": "Int",    "value": "0"},
                            {"type": "Int",    "value": "1"},
                            {"type": "Int",    "value": "0"},
                            {"type": "Int",    "value": "0"}
                        ],
                        "children": []
                    },
                    {
                        "name": "SOCCER_GAME_INFO_REF_DIFFICULTY_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "1"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur les fixtures ───────────────────────────────────────────────────

#[test]
fn focus_battle_range_comptes_et_valeurs() {
    let cfg = parse_focus_battle_effect_config(&fixture_focus_battle());
    // fixture : 2 ranges, 1 activated, 1 trigger, 1 effect info
    assert_eq!(cfg.ranges.len(), 2, "2 ranges dans la fixture");
    // ranges[0] inactif
    assert!(!cfg.ranges[0].is_active(), "ranges[0] inactif");
    // ranges[1] actif — soccer_focus_battle_effect_config lignes 15-20
    assert!(cfg.ranges[1].is_active(), "ranges[1] actif");
    assert_eq!(cfg.ranges[1].range_param1, 30, "rangeParam1=30");
    assert_eq!(cfg.ranges[1].range_param2, 10, "rangeParam2=10");
    assert_eq!(cfg.ranges[1].range_param3, 0);
}

#[test]
fn focus_battle_activated_effect_id() {
    let cfg = parse_focus_battle_effect_config(&fixture_focus_battle());
    // m_soccerFocusBattleActivatedEffectList[0].effectId = "0xBEBB2EE8"
    assert_eq!(cfg.activated_effects.len(), 1);
    assert_eq!(cfg.activated_effects[0].effect_id, HashId(0xBEBB_2EE8));
    // tous les params à 0 dans le dump réel
    assert_eq!(cfg.activated_effects[0].effect_params, [0i64; 8]);
}

#[test]
fn focus_battle_demo_trigger_type3() {
    let cfg = parse_focus_battle_effect_config(&fixture_focus_battle());
    // m_soccerFocusBattleDemoTriggerList[0] — soccer_focus_battle_effect_config lignes 190-198
    assert_eq!(cfg.demo_triggers.len(), 1);
    let t = &cfg.demo_triggers[0];
    assert_eq!(t.demo_trigger_type, 3);
    assert_eq!(t.demo_trigger_text_id, HashId(0x8901_F53D));
}

#[test]
fn focus_battle_effect_info_0() {
    let cfg = parse_focus_battle_effect_config(&fixture_focus_battle());
    // m_soccerFocusBattleEffectInfoList[0] — soccer_focus_battle_effect_config lignes 264-280
    assert_eq!(cfg.effect_infos.len(), 1);
    let e = &cfg.effect_infos[0];
    assert_eq!(e.id, HashId(0x6465_9E21));
    assert_eq!(e.range, [0, 1]);
    assert_eq!(e.activated_effect, [0, 1]);
    assert_eq!(e.demo_trigger, [0, 1]);
}

#[test]
fn focus_battle_find_effect() {
    let cfg = parse_focus_battle_effect_config(&fixture_focus_battle());
    assert!(cfg.find_effect(HashId(0x6465_9E21)).is_some());
    assert!(cfg.find_effect(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn focus_battle_ranges_of() {
    let cfg = parse_focus_battle_effect_config(&fixture_focus_battle());
    let info = &cfg.effect_infos[0]; // range=[0,1] → ranges[0]
    let r = cfg.ranges_of(info);
    assert_eq!(r.len(), 1);
    assert!(!r[0].is_active()); // ranges[0] est inactif dans la fixture
}

#[test]
fn technic_conditions_et_technics() {
    let cfg = parse_technic_config(&fixture_technic());
    assert_eq!(cfg.conditions.len(), 2, "2 conditions dans la fixture");
    assert_eq!(cfg.technics.len(), 1, "1 technique dans la fixture");

    // m_soccerTechnicUsableConditionList[0] — soccer_technic_config lignes 6-14
    let c0 = &cfg.conditions[0];
    assert_eq!(c0.cond_type, 1, "type=1 (stat requirement)");
    assert_eq!(c0.param1, 3);
    assert_eq!(c0.param2, 30, "seuil de stat = 30");
    assert_eq!(c0.param3, 0);

    // m_soccerTechnicInfoList[0] — soccer_technic_config lignes 104-114
    let t0 = &cfg.technics[0];
    assert_eq!(t0.id, HashId(0x0785_7A3F));
    assert_eq!(t0.recast_time, 5, "recastTime = 5 s");
    assert_eq!(t0.usable_condition, [0, 2]);
    assert_eq!(t0.focus_battle_effect_id, HashId(0x8A6B_FF0D));
}

#[test]
fn technic_conditions_of_slice() {
    let cfg = parse_technic_config(&fixture_technic());
    let t0 = &cfg.technics[0]; // usableCondition=[0,2] → conditions[0..2]
    let conds = cfg.conditions_of(t0);
    assert_eq!(conds.len(), 2);
    assert_eq!(conds[0].cond_type, 1);
    assert_eq!(conds[1].cond_type, 2);
}

#[test]
fn technic_find_technic() {
    let cfg = parse_technic_config(&fixture_technic());
    assert!(cfg.find_technic(HashId(0x0785_7A3F)).is_some());
    assert!(cfg.find_technic(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn basic_effect_comptes_et_valeurs() {
    let effects = parse_basic_effect_config(&fixture_basic_effect());
    assert_eq!(effects.len(), 2, "2 effets dans la fixture");
    // m_soccerBasicEffectInfoList[0] — soccer_basic_effect_config lignes 8-12
    assert_eq!(effects[0].id, HashId(0x83DB_0758));
    assert_eq!(effects[0].func_name, HashId(0xC9D6_04D4));
    assert_eq!(effects[0].build_icon_type, 6);
    // entrée [1]
    assert_eq!(effects[1].id, HashId(0x1AD2_56E2));
}

#[test]
fn additional_config_exp_table() {
    let cfg = parse_soccer_game_additional_config(&fixture_additional());
    // m_SoccerGetExpDataTable[0] — soccer_game_additional_config lignes 67-70
    assert_eq!(cfg.get_exp_table.len(), 2);
    assert_eq!(cfg.get_exp_table[0].id, 0);
    assert!(
        (cfg.get_exp_table[0].ratio - 7.8_f64).abs() < 1e-9,
        "ratio[0] = 7.8"
    );
    assert!(
        (cfg.get_exp_table[1].ratio - 7.3_f64).abs() < 1e-9,
        "ratio[1] = 7.3"
    );
}

#[test]
fn additional_config_team_ai() {
    let cfg = parse_soccer_game_additional_config(&fixture_additional());
    // m_SoccerTeamAIDataList[0] — soccer_game_additional_config lignes 7-12
    assert_eq!(cfg.team_ai_data.len(), 1);
    assert_eq!(cfg.team_ai_data[0].id, HashId(0x6FF0_0159));
    assert_eq!(cfg.team_ai_data[0].param_data, "01010101");
    assert_eq!(cfg.team_ai_data[0].strategy_id, HashId(0x1CE5_F4E2));
}

#[test]
fn additional_config_exp_ratio_helper() {
    let cfg = parse_soccer_game_additional_config(&fixture_additional());
    assert!((cfg.exp_ratio(0).unwrap() - 7.8_f64).abs() < 1e-9);
    assert!(cfg.exp_ratio(99).is_none());
}

#[test]
fn additional_config_nice_play_exp() {
    let cfg = parse_soccer_game_additional_config(&fixture_additional());
    // m_SoccerNicePlayExpDataList[0] — soccer_game_additional_config lignes 140-148
    assert_eq!(cfg.nice_play_exp.len(), 1);
    assert_eq!(cfg.nice_play_exp[0].id, HashId(0x439C_E11B));
    assert_eq!(cfg.nice_play_exp[0].shoot_exp, 10);
    assert_eq!(cfg.nice_play_exp[0].focus_exp, 10);
}

#[test]
fn game_config_plus_difficulty_0() {
    let cfg = parse_soccer_game_config(&fixture_game_config_plus());
    // SOCCER_GAME_DIFFICULTY_0 — soccer_game_config_plus lignes 13-17
    assert_eq!(cfg.difficulties.len(), 1);
    let d = &cfg.difficulties[0];
    assert_eq!(d.difficulty_id, 1, "difficulty_id = 1");
    // raw[0] = var[1] = -587180464 → 0xDD005650
    assert_eq!(d.raw[0], -587180464_i64, "raw[0] = var[1]");
    // raw[1] = var[2] = 40
    assert_eq!(d.raw[1], 40_i64, "raw[1] = var[2] = 40");
    // raw[19] = var[20] = 30 (seuil commun aux deux fichiers)
    assert_eq!(d.raw[19], 30_i64, "raw[19] = var[20] = 30");
}

#[test]
fn game_config_plus_game_info_0() {
    let cfg = parse_soccer_game_config(&fixture_game_config_plus());
    // SOCCER_GAME_INFO_0 — soccer_game_config_plus lignes 216-260
    // -1344098363 as u32 = 0xAFE2AFC5
    assert_eq!(cfg.game_infos.len(), 1);
    let g = &cfg.game_infos[0];
    assert_eq!(g.match_id, HashId(0xAFE2_AFC5), "match_id = 0xAFE2AFC5");
    assert_eq!(g.id_str, "fbtl_st_0904", "id_str = fbtl_st_0904");
    assert_eq!(
        g.difficulty_index, 0,
        "difficulty_index = 0 depuis REF_DIFFICULTY"
    );
    // raw[0] = var[2] = 1
    assert_eq!(g.raw[0], 1_i64, "raw[0] = var[2] = 1");
    // raw[1] = var[3] = -724197456 → 0xD4D59FB0
    assert_eq!(g.raw[1], -724197456_i64, "raw[1] = var[3]");
}

#[test]
fn game_config_plus_difficulty_of() {
    let cfg = parse_soccer_game_config(&fixture_game_config_plus());
    let g = &cfg.game_infos[0];
    let d = cfg.difficulty_of(g).expect("difficulty_of doit résoudre");
    assert_eq!(d.difficulty_id, 1);
}

#[test]
fn game_config_plus_find_game() {
    let cfg = parse_soccer_game_config(&fixture_game_config_plus());
    assert!(cfg.find_game(HashId(0xAFE2_AFC5)).is_some());
    assert!(cfg.find_game(HashId(0xDEAD_BEEF)).is_none());
}

// ─── Tests sur les vrais fichiers (skip silencieux si absents du VPS) ─────────

const BASE: &str = "soccer";

fn load_json(filename: &str) -> Option<serde_json::Value> {
    let path = std::format!("{BASE}/{filename}");
    let path = common::chemin(&path)?;
    if !path.is_file() {
        eprintln!("skip : {} absent du corpus", path.display());
        return None;
    }
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", path.display()));
    Some(
        serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("JSON invalide {}: {e}", path.display())),
    )
}

#[test]
fn real_file_focus_battle_comptes() {
    let Some(root) = load_json("soccer_focus_battle_effect_config.cfg.bin.json") else {
        return;
    };
    let cfg = parse_focus_battle_effect_config(&root);
    // 10 entrées dans chaque liste — soccer_focus_battle_effect_config.cfg.bin.json
    assert_eq!(cfg.ranges.len(), 10, "10 ranges");
    assert_eq!(cfg.activated_effects.len(), 10, "10 activated effects");
    assert_eq!(cfg.demo_triggers.len(), 10, "10 demo triggers");
    assert_eq!(cfg.effect_infos.len(), 10, "10 effect infos");
}

#[test]
fn real_file_focus_battle_range1_actif() {
    let Some(root) = load_json("soccer_focus_battle_effect_config.cfg.bin.json") else {
        return;
    };
    let cfg = parse_focus_battle_effect_config(&root);
    // m_soccerFocusBattleEffectRangeList[1] — lignes 15-20 du fichier
    let r1 = &cfg.ranges[1];
    assert_eq!(r1.range_type, 1, "ranges[1].rangeType = 1");
    assert_eq!(r1.range_param1, 30, "ranges[1].rangeParam1 = 30");
    assert_eq!(r1.range_param2, 10, "ranges[1].rangeParam2 = 10");
    assert_eq!(r1.range_param3, 0);
}

#[test]
fn real_file_focus_battle_activated_effect0() {
    let Some(root) = load_json("soccer_focus_battle_effect_config.cfg.bin.json") else {
        return;
    };
    let cfg = parse_focus_battle_effect_config(&root);
    // m_soccerFocusBattleActivatedEffectList[0].effectId = "0xBEBB2EE8"
    assert_eq!(cfg.activated_effects[0].effect_id, HashId(0xBEBB_2EE8));
    assert_eq!(cfg.activated_effects[0].effect_params, [0i64; 8]);
}

#[test]
fn real_file_focus_battle_demo_trigger0() {
    let Some(root) = load_json("soccer_focus_battle_effect_config.cfg.bin.json") else {
        return;
    };
    let cfg = parse_focus_battle_effect_config(&root);
    // m_soccerFocusBattleDemoTriggerList[0] — lignes 190-198 du fichier
    assert_eq!(cfg.demo_triggers[0].demo_trigger_type, 3);
    assert_eq!(
        cfg.demo_triggers[0].demo_trigger_text_id,
        HashId(0x8901_F53D)
    );
}

#[test]
fn real_file_focus_battle_effect_info0() {
    let Some(root) = load_json("soccer_focus_battle_effect_config.cfg.bin.json") else {
        return;
    };
    let cfg = parse_focus_battle_effect_config(&root);
    // m_soccerFocusBattleEffectInfoList[0] — lignes 264-280 du fichier
    assert_eq!(cfg.effect_infos[0].id, HashId(0x6465_9E21));
    assert_eq!(cfg.effect_infos[0].range, [0, 1]);
}

#[test]
fn real_file_technic_comptes() {
    let Some(root) = load_json("soccer_technic_config.cfg.bin.json") else {
        return;
    };
    let cfg = parse_technic_config(&root);
    assert_eq!(cfg.conditions.len(), 15, "15 conditions");
    assert_eq!(cfg.technics.len(), 8, "8 techniques");
}

#[test]
fn real_file_technic_info0() {
    let Some(root) = load_json("soccer_technic_config.cfg.bin.json") else {
        return;
    };
    let cfg = parse_technic_config(&root);
    // m_soccerTechnicInfoList[0] — soccer_technic_config lignes 104-114
    let t0 = &cfg.technics[0];
    assert_eq!(t0.id, HashId(0x0785_7A3F), "technic[0].id = 0x07857A3F");
    assert_eq!(t0.recast_time, 5, "technic[0].recastTime = 5");
    assert_eq!(t0.usable_condition, [0, 2]);
    assert_eq!(t0.focus_battle_effect_id, HashId(0x8A6B_FF0D));
}

#[test]
fn real_file_technic_recast_times() {
    let Some(root) = load_json("soccer_technic_config.cfg.bin.json") else {
        return;
    };
    let cfg = parse_technic_config(&root);
    // recastTime des 8 techniques (lignes 104-193) : [5, 5, 5, 8, 3, 4, 3, 4]
    let expected = [5i64, 5, 5, 8, 3, 4, 3, 4];
    for (i, &expected_rt) in expected.iter().enumerate() {
        assert_eq!(
            cfg.technics[i].recast_time, expected_rt,
            "technic[{i}].recastTime = {expected_rt}"
        );
    }
}

#[test]
fn real_file_basic_effect_comptes() {
    let Some(root) = load_json("soccer_basic_effect_config_1.01.79.00.cfg.bin.json") else {
        return;
    };
    let effects = parse_basic_effect_config(&root);
    // 102 entrées — soccer_basic_effect_config_1.01.79.00.cfg.bin.json
    assert_eq!(effects.len(), 102, "102 effets de base");
}

#[test]
fn real_file_basic_effect_0() {
    let Some(root) = load_json("soccer_basic_effect_config_1.01.79.00.cfg.bin.json") else {
        return;
    };
    let effects = parse_basic_effect_config(&root);
    // m_soccerBasicEffectInfoList[0] — lignes 8-12 du fichier
    assert_eq!(effects[0].id, HashId(0x83DB_0758));
    assert_eq!(effects[0].func_name, HashId(0xC9D6_04D4));
    assert_eq!(effects[0].build_icon_type, 6);
    // tous ont buildIconType=6 dans le dump réel
    assert!(
        effects.iter().all(|e| e.build_icon_type == 6),
        "tous buildIconType=6"
    );
}

#[test]
fn real_file_additional_comptes() {
    let Some(root) = load_json("soccer_game_additional_config_1.04.14.00.cfg.bin.json") else {
        return;
    };
    let cfg = parse_soccer_game_additional_config(&root);
    assert_eq!(cfg.team_ai_data.len(), 9, "9 équipes IA");
    assert_eq!(cfg.get_exp_table.len(), 17, "17 niveaux XP");
    assert_eq!(cfg.nice_play_exp.len(), 7, "7 niveaux belle action");
}

#[test]
fn real_file_additional_exp_table_bornes() {
    let Some(root) = load_json("soccer_game_additional_config_1.04.14.00.cfg.bin.json") else {
        return;
    };
    let cfg = parse_soccer_game_additional_config(&root);
    // m_SoccerGetExpDataTable[0] — lignes 67-70 du fichier
    assert_eq!(cfg.get_exp_table[0].id, 0);
    assert!(
        (cfg.get_exp_table[0].ratio - 7.8_f64).abs() < 1e-9,
        "ratio[0] = 7.8"
    );
    // m_SoccerGetExpDataTable[16] — dernière entrée
    assert_eq!(cfg.get_exp_table[16].id, 16);
    assert!(
        (cfg.get_exp_table[16].ratio - 4.0_f64).abs() < 1e-9,
        "ratio[16] = 4.0"
    );
}

#[test]
fn real_file_additional_team_ai0() {
    let Some(root) = load_json("soccer_game_additional_config_1.04.14.00.cfg.bin.json") else {
        return;
    };
    let cfg = parse_soccer_game_additional_config(&root);
    // m_SoccerTeamAIDataList[0] — lignes 7-12 du fichier
    assert_eq!(cfg.team_ai_data[0].id, HashId(0x6FF0_0159));
    assert_eq!(cfg.team_ai_data[0].param_data, "01010101");
    assert_eq!(cfg.team_ai_data[0].strategy_id, HashId(0x1CE5_F4E2));
}

#[test]
fn real_file_game_config_plus_difficulte() {
    let Some(root) = load_json("soccer_game_config_plus_1.04.08.00.cfg.bin.json") else {
        return;
    };
    let cfg = parse_soccer_game_config(&root);
    // soccer_game_config_plus_1.04.08.00.cfg.bin.json : 1 difficulté
    assert_eq!(
        cfg.difficulties.len(),
        1,
        "1 difficulté dans le fichier plus"
    );
    assert_eq!(cfg.difficulties[0].difficulty_id, 1, "difficulty_id = 1");
    // var[2]=40 → raw[1]=40 (ligne 25 du fichier)
    assert_eq!(cfg.difficulties[0].raw[1], 40, "raw[1] = var[2] = 40");
    // var[20]=30 → raw[19]=30 (seuil commun)
    assert_eq!(cfg.difficulties[0].raw[19], 30, "raw[19] = var[20] = 30");
    // var[35]=13 (plus) → raw[34]=13
    assert_eq!(cfg.difficulties[0].raw[34], 13, "raw[34] = var[35] = 13");
}

#[test]
fn real_file_game_config_plus_match_info() {
    let Some(root) = load_json("soccer_game_config_plus_1.04.08.00.cfg.bin.json") else {
        return;
    };
    let cfg = parse_soccer_game_config(&root);
    // SOCCER_GAME_INFO_0 — soccer_game_config_plus lignes 216-260
    assert_eq!(cfg.game_infos.len(), 1, "1 match dans le fichier plus");
    let g = &cfg.game_infos[0];
    // -1344098363 as u32 = 0xAFE2AFC5
    assert_eq!(g.match_id, HashId(0xAFE2_AFC5), "match_id = 0xAFE2AFC5");
    assert_eq!(g.id_str, "fbtl_st_0904", "id_str = fbtl_st_0904");
    assert_eq!(
        g.difficulty_index, 0,
        "difficulty_index depuis REF_DIFFICULTY_0.var[0]=0"
    );
    // raw[1] = var[3] = -724197456 → 0xD4D59FB0
    assert_eq!(g.raw[1], -724197456_i64, "raw[1] = var[3]");
}

#[test]
fn real_file_game_config_full_comptes() {
    let Some(root) = load_json("soccer_game_config_1.04.08.00.cfg.bin.json") else {
        return;
    };
    let cfg = parse_soccer_game_config(&root);
    // soccer_game_config_1.04.08.00.cfg.bin.json
    assert_eq!(cfg.cpu_beans_rates.len(), 7, "7 CPU beans rates");
    assert_eq!(cfg.difficulties.len(), 1772, "1772 difficultés");
    assert_eq!(cfg.game_infos.len(), 325, "325 matches");
}

#[test]
fn real_file_game_config_full_cpu_beans() {
    let Some(root) = load_json("soccer_game_config_1.04.08.00.cfg.bin.json") else {
        return;
    };
    let cfg = parse_soccer_game_config(&root);
    // CPU_BEANS_RATE_INFO_0 = [1, 1] → kind=1, rate=1.0
    assert_eq!(cfg.cpu_beans_rates[0].kind, 1);
    assert!(
        (cfg.cpu_beans_rates[0].rate - 1.0_f64).abs() < 1e-9,
        "rate[0] = 1.0"
    );
    // CPU_BEANS_RATE_INFO_3 = [4, 0] → kind=4, rate=0.0
    assert_eq!(cfg.cpu_beans_rates[3].kind, 4);
    assert!(
        (cfg.cpu_beans_rates[3].rate - 0.0_f64).abs() < 1e-9,
        "rate[3] = 0.0"
    );
    // CPU_BEANS_RATE_INFO_6 = [7, 1] → kind=7, rate=1.0
    assert_eq!(cfg.cpu_beans_rates[6].kind, 7);
    assert!(
        (cfg.cpu_beans_rates[6].rate - 1.0_f64).abs() < 1e-9,
        "rate[6] = 1.0"
    );
}

#[test]
fn real_file_game_config_full_difficulty0() {
    let Some(root) = load_json("soccer_game_config_1.04.08.00.cfg.bin.json") else {
        return;
    };
    let cfg = parse_soccer_game_config(&root);
    // SOCCER_GAME_DIFFICULTY_0 — soccer_game_config_1.04.08.00 ligne ~125
    let d = &cfg.difficulties[0];
    assert_eq!(d.difficulty_id, 1, "difficulty_id = 1");
    // var[2] = 23 → raw[1] = 23
    assert_eq!(d.raw[1], 23, "raw[1] = var[2] = 23");
    // var[9] = 1027 → raw[8] = 1027
    assert_eq!(d.raw[8], 1027, "raw[8] = var[9] = 1027");
    // var[20] = 30 → raw[19] = 30 (commun avec plus)
    assert_eq!(d.raw[19], 30, "raw[19] = var[20] = 30");
    // var[35] = 1 → raw[34] = 1
    assert_eq!(d.raw[34], 1, "raw[34] = var[35] = 1");
}

#[test]
fn real_file_game_config_full_game_info0() {
    let Some(root) = load_json("soccer_game_config_1.04.08.00.cfg.bin.json") else {
        return;
    };
    let cfg = parse_soccer_game_config(&root);
    // SOCCER_GAME_INFO_0 — soccer_game_config_1.04.08.00 ligne ~215 (children[0])
    // -778368270 as u32 = 0xD19B0AF2
    let g = &cfg.game_infos[0];
    assert_eq!(g.match_id, HashId(0xD19B_0AF2), "match_id = 0xD19B0AF2");
    assert_eq!(g.id_str, "fbtl_st_0101", "id_str = fbtl_st_0101");
    assert_eq!(g.difficulty_index, 0, "REF_DIFFICULTY_0.var[0] = 0");
    // raw[1] = var[3] = -724197456
    assert_eq!(g.raw[1], -724197456_i64, "raw[1] = var[3]");
}
