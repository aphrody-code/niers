#![allow(clippy::pedantic)]
//! Tests golden `rpg_battle` — valeurs réelles tirées des dumps :
//!
//! | Fichier | Lignes-clés vérifiées |
//! |---------|----------------------|
//! | `rpg_battle_rule_config_1.02.84.00.cfg.bin.json` | l.28 ruleId[0], l.9 type[0] |
//! | `rpg_battle_status_pattern_config_1.03.17.00.cfg.bin.json` | l.9 id[0], l.10 hpAddRate[0], l.11 attackAdd[0] |
//! | `rpg_battle_chara_swap_motion_config_1.04.06.00.cfg.bin.json` | l.10 bodyType[0], l.11 motionId[0], l.55 modelAttribute[0] |
//! | `rpg_battle_party_config_0.00.00.cfg.bin.json` | l.17 action_id[0], l.84 party_id[0], l.97-101 REF[0] |
//! | `rpg_battle_cmd_event_config_0.00.00.cfg.bin.json` | l.16 var[0] |
//! | `rpg_battle_cmd_obj_config_0.00.00.cfg.bin.json` | l.13 var[0] |
//! | `rpg_battle_cmd_set_config_1.01.31.00.cfg.bin.json` | l.17 CMD_DATA[0], l.11313 CMD_SET_INFO[0], l.11320-11328 REF[0] |
//! | `rpg_battle_add_status_config_0.00.00.cfg.bin.json` | l.22 TYPE_INFO[0], l.354 ADD_STATUS_INFO[0], l.372 PARAM[0], l.598-604 ATTR paire[0] |

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::rpg_battle::{
    parse_add_status_config, parse_chara_swap_motion_config, parse_cmd_event_config,
    parse_cmd_obj_config, parse_cmd_set_config, parse_party_config, parse_rule_config,
    parse_status_pattern_config,
};
use serde_json::json;

// ─── Constantes golden ────────────────────────────────────────────────────────

/// `m_BattleRuleList[0].ruleId = "0x42D90CBC"` (ligne 28)
const RULE_ID_0: HashId = HashId(0x42D90CBC);

/// `m_statusPatternInfoList[0].id = "0xD1002353"` (ligne 9)
const STATUS_PAT_ID_0: HashId = HashId(0xD1002353);

/// `m_SwapMotionDataList[0].motionId = "0x2C0F2E35"` (ligne 11)
const MOTION_ID_0: HashId = HashId(0x2C0F2E35);

/// `RPG_BTL_PARTY_DISABLE_ACTION_UNIQUE_INFO_0.var[0] = -1046485492` (ligne 17)
/// → -1046485492 & 0xFFFFFFFF = 0xC19FE60C
const DISABLE_ACTION_ID_0: HashId = HashId(0xC19F_E60C);

/// `RPG_BTL_PARTY_INFO_0.var[0] = -1758152796` (ligne 84)
/// → -1758152796 & 0xFFFFFFFF = 0x9734B7A4
const PARTY_ID_0: HashId = HashId(0x9734_B7A4);

/// `RPG_BTL_CMD_EVENT_INFO_0.var[0] = 20635986` (ligne 16)
/// → 20635986 & 0xFFFFFFFF = 0x013AE152
const CMD_EVENT_ID_0: HashId = HashId(0x013A_E152);

/// `RPG_BTL_CMD_OBJ_INFO_0.var[0] = 894153704` (ligne 13)
/// → 894153704 & 0xFFFFFFFF = 0x354BB3E8
const CMD_OBJ_ID_0: HashId = HashId(0x354B_B3E8);

/// `RPG_BTL_CMD_SET_CMD_DATA_0.var[0] = -1467292399` (ligne 17)
/// → -1467292399 & 0xFFFFFFFF = 0xA88AE511
const CMD_DATA_ID_0: HashId = HashId(0xA88A_E511);

/// `RPG_BTL_CMD_SET_INFO_0.var[0] = 1268152025` (ligne 11313)
/// → 1268152025 & 0xFFFFFFFF = 0x4B9676D9
const CMD_SET_ID_0: HashId = HashId(0x4B96_76D9);

/// `RPG_BTL_ADD_STATUS_INFO_0.var[0] = -408109186` (ligne 354)
/// → -408109186 & 0xFFFFFFFF = 0xE7ACBF7E
const ADD_STATUS_ID_0: HashId = HashId(0xE7AC_BF7E);

/// `RPG_BTL_ADD_STATUS_INFO_ATTR_0.var[1] = -1356316547` (ligne 604)
/// → -1356316547 & 0xFFFFFFFF = 0xAF28407D
const ATTR_ID_0: HashId = HashId(0xAF28_407D);

// ─── Fixtures inline ──────────────────────────────────────────────────────────

fn fixture_rule_config() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_StartRuleList",
                "typeName": "RpgBattleStartRuleInfo",
                "values": [
                    // ligne 9 : type=1, param=0, trg=0, paramId=0x00000000
                    { "type": 1, "param": 0, "trg": 0, "paramId": "0x00000000" }
                ]
            },
            {
                "name": "m_BattleRuleList",
                "typeName": "RpgBattleRuleInfo",
                "values": [
                    // ligne 28 : ruleId="0x42D90CBC", start=[0,0], end=[0,0], result=[0,0]
                    { "ruleId": "0x42D90CBC", "start": [0, 0], "end": [0, 0], "result": [0, 0] },
                    // ligne 43 : ruleId="0x73601B43"
                    { "ruleId": "0x73601B43", "start": [0, 1], "end": [0, 0], "result": [0, 0] }
                ]
            }
        ]
    })
}

fn fixture_status_pattern() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_statusPatternInfoList",
                "typeName": "RPG_BTL_STATUS_PATTERN_INFO",
                "values": [
                    // entrée 0 — ligne 9-14 : id="0xD1002353", hpAddRate=0.2, attackAdd=-2
                    { "id": "0xD1002353", "hpAddRate": 0.2, "attackAdd": -2, "defenseAdd": 0 },
                    // entrée 10 — id="0x6C8F6D1C", hpAddRate=1.5, attackAdd=2
                    { "id": "0x6C8F6D1C", "hpAddRate": 1.5, "attackAdd": 2, "defenseAdd": 0 }
                ]
            }
        ]
    })
}

fn fixture_chara_swap_motion() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_SwapMotionDataList",
                "typeName": "SWAP_MOTION_DATA",
                "values": [
                    // entrée 0 — ligne 10-11 : bodyType=1, motionId="0x2C0F2E35"
                    { "bodyType": 1, "motionId": "0x2C0F2E35" },
                    { "bodyType": 1, "motionId": "0xC96C3264" }
                ]
            },
            {
                "name": "m_RpgBattleCharaSwapMotionDataList",
                "typeName": "RPG_BATTLE_CHARA_SWAP_MOTION_DATA",
                "values": [
                    // entrée 0 — ligne 55 : modelAttribute=3, swapMotion=[0,1]
                    {
                        "modelAttribute": 3,
                        "openCond": "AAAAAEgXMg==",
                        "swapMotion": [0, 1]
                    }
                ]
            }
        ]
    })
}

fn fixture_party_config() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "RPG_BTL_PARTY_DISABLE_ACTION_UNIQUE_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    // entrée 0 — ligne 17 : var[0]=-1046485492 → 0xC19FE60C
                    {
                        "name": "RPG_BTL_PARTY_DISABLE_ACTION_UNIQUE_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "-1046485492"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    // entrée 1 — ligne 35 : var[0]=-1510792440 → 0xA5F32308
                    {
                        "name": "RPG_BTL_PARTY_DISABLE_ACTION_UNIQUE_INFO_1",
                        "variables": [
                            {"type": "Int", "value": "-1510792440"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "RPG_BTL_PARTY_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    // party 0 — ligne 84 : var[0]=-1758152796 → 0x9734B7A4
                    {
                        "name": "RPG_BTL_PARTY_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "-1758152796"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    // REF 0 — ligne 97-101 : offset=0, count=1
                    {
                        "name": "RPG_BTL_PARTY_INFO_REF_DISABLE_ACTION_UNIQUE_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "1"}
                        ],
                        "children": []
                    },
                    // party 1 — var[0]=-1762682639 → 0x96EF98F1
                    {
                        "name": "RPG_BTL_PARTY_INFO_1",
                        "variables": [
                            {"type": "Int", "value": "-1762682639"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    // REF 1 : offset=1, count=2
                    {
                        "name": "RPG_BTL_PARTY_INFO_REF_DISABLE_ACTION_UNIQUE_INFO_1",
                        "variables": [
                            {"type": "Int", "value": "1"},
                            {"type": "Int", "value": "2"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

fn fixture_cmd_event() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "RPG_BTL_CMD_EVENT_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    // ligne 16 : var[0]=20635986 → 0x013AE152
                    {
                        "name": "RPG_BTL_CMD_EVENT_INFO_0",
                        "variables": [{"type": "Int", "value": "20635986"}],
                        "children": []
                    }
                ]
            }
        ]
    })
}

fn fixture_cmd_obj() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "RPG_BTL_CMD_OBJ_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    // ligne 13 : var[0]=894153704 → 0x354BB3E8
                    {
                        "name": "RPG_BTL_CMD_OBJ_INFO_0",
                        "variables": [{"type": "Int", "value": "894153704"}],
                        "children": []
                    }
                ]
            }
        ]
    })
}

fn fixture_cmd_set() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "RPG_BTL_CMD_SET_CMD_DATA_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    // ligne 17 : var[0]=-1467292399 → 0xA88AE511
                    {
                        "name": "RPG_BTL_CMD_SET_CMD_DATA_0",
                        "variables": [{"type": "Int", "value": "-1467292399"}],
                        "children": []
                    },
                    // ligne 24 : var[0]=-544352889 → 0xDFACF2C7
                    {
                        "name": "RPG_BTL_CMD_SET_CMD_DATA_1",
                        "variables": [{"type": "Int", "value": "-544352889"}],
                        "children": []
                    }
                ]
            },
            {
                "name": "RPG_BTL_CMD_SET_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}, {"type": "Int", "value": "1"}],
                "children": [
                    // ligne 11313 : var[0]=1268152025 → 0x4B9676D9
                    {
                        "name": "RPG_BTL_CMD_SET_INFO_0",
                        "variables": [{"type": "Int", "value": "1268152025"}],
                        "children": []
                    },
                    // ligne 11320-11328 : offset=0, count=2 (fixture: 2 cmd_data)
                    {
                        "name": "RPG_BTL_CMD_SET_INFO_REF_CMD_DATA_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "2"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

fn fixture_add_status() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "RPG_BTL_ADD_STATUS_TYPE_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    // entrée 0 — ligne ~22 : var[0]=-1 (toutes catégories)
                    {
                        "name": "RPG_BTL_ADD_STATUS_TYPE_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "-1"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    // entrée 2 — ligne ~73 : var[0]=29
                    {
                        "name": "RPG_BTL_ADD_STATUS_TYPE_INFO_1",
                        "variables": [
                            {"type": "Int", "value": "29"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "RPG_BTL_ADD_STATUS_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    // entrée 0 — ligne 354 : var[0]=-408109186 → 0xE7ACBF7E
                    {
                        "name": "RPG_BTL_ADD_STATUS_INFO_0",
                        "variables": [{"type": "Int", "value": "-408109186"}],
                        "children": [
                            {
                                "name": "RPG_BTL_ADD_STATUS_INFO_PARAM_LIST_BEG_0",
                                "variables": [{"type": "Int", "value": "1"}],
                                "children": [
                                    // PARAM_0 — ligne 372 : var[0]=32, var[5]="0,2"
                                    {
                                        "name": "RPG_BTL_ADD_STATUS_INFO_PARAM_0",
                                        "variables": [
                                            {"type": "Int", "value": "32"},
                                            {"type": "Int", "value": "-1"},
                                            {"type": "Int", "value": "2"},
                                            {"type": "Int", "value": "2"},
                                            {"type": "Int", "value": "2"},
                                            {"type": "Float", "value": "0,2"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "-1"}
                                        ],
                                        "children": []
                                    }
                                ]
                            }
                        ]
                    },
                    // ATTR_0 — ligne 596 : var[0]=1, var[1]=-1356316547 → 0xAF28407D
                    {
                        "name": "RPG_BTL_ADD_STATUS_INFO_ATTR_0",
                        "variables": [
                            {"type": "Int", "value": "1"},
                            {"type": "Int", "value": "-1356316547"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "-1957340403"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

// ─── Tests fixture : parse_rule_config ───────────────────────────────────────

#[test]
fn rule_config_start_rule_compte() {
    let cfg = parse_rule_config(&fixture_rule_config());
    assert_eq!(cfg.start_rules.len(), 1, "fixture: 1 StartRuleInfo");
}

#[test]
fn rule_config_start_rule_type_0() {
    let cfg = parse_rule_config(&fixture_rule_config());
    // m_StartRuleList[0] (ligne 9) : type=1, param=0, trg=0
    assert_eq!(
        cfg.start_rules[0].rule_type, 1,
        "start_rule[0].rule_type = 1"
    );
    assert_eq!(cfg.start_rules[0].param, 0, "start_rule[0].param = 0");
    assert_eq!(cfg.start_rules[0].trg, 0, "start_rule[0].trg = 0");
    assert!(
        cfg.start_rules[0].param_id.is_zero(),
        "start_rule[0].param_id = 0"
    );
}

#[test]
fn rule_config_battle_rule_compte() {
    let cfg = parse_rule_config(&fixture_rule_config());
    assert_eq!(cfg.battle_rules.len(), 2, "fixture: 2 BattleRuleInfo");
}

#[test]
fn rule_config_battle_rule_id_0() {
    let cfg = parse_rule_config(&fixture_rule_config());
    // m_BattleRuleList[0] (ligne 28) : ruleId="0x42D90CBC"
    assert_eq!(
        cfg.battle_rules[0].rule_id, RULE_ID_0,
        "rule_id[0] = 0x42D90CBC"
    );
}

#[test]
fn rule_config_battle_rule_start_1() {
    let cfg = parse_rule_config(&fixture_rule_config());
    // m_BattleRuleList[1] : start=[0, 1]
    assert_eq!(
        cfg.battle_rules[1].start,
        [0_i64, 1_i64],
        "start[1] = [0, 1]"
    );
}

#[test]
fn rule_config_find_battle_rule() {
    let cfg = parse_rule_config(&fixture_rule_config());
    assert!(
        cfg.find_battle_rule(RULE_ID_0).is_some(),
        "find_battle_rule(0x42D90CBC)"
    );
    assert!(cfg.find_battle_rule(HashId(0xDEADBEEF)).is_none());
}

// ─── Tests fixture : parse_status_pattern_config ─────────────────────────────

#[test]
fn status_pattern_compte() {
    let data = parse_status_pattern_config(&fixture_status_pattern());
    assert_eq!(data.len(), 2, "fixture: 2 StatusPatternInfo");
}

#[test]
fn status_pattern_id_0() {
    let data = parse_status_pattern_config(&fixture_status_pattern());
    // entrée 0 (ligne 9) : id="0xD1002353"
    assert_eq!(data[0].id, STATUS_PAT_ID_0, "id[0] = 0xD1002353");
}

#[test]
fn status_pattern_hp_add_rate_0() {
    let data = parse_status_pattern_config(&fixture_status_pattern());
    // entrée 0 (ligne 10) : hpAddRate=0.2
    assert!(
        (data[0].hp_add_rate - 0.2_f32).abs() < 1e-6,
        "hp_add_rate[0] ≈ 0.2"
    );
}

#[test]
fn status_pattern_attack_add_0() {
    let data = parse_status_pattern_config(&fixture_status_pattern());
    // entrée 0 (ligne 11) : attackAdd=-2
    assert_eq!(data[0].attack_add, -2, "attack_add[0] = -2");
    assert_eq!(data[0].defense_add, 0, "defense_add[0] = 0");
}

#[test]
fn status_pattern_entree_1_hp() {
    let data = parse_status_pattern_config(&fixture_status_pattern());
    // entrée 1 : hpAddRate=1.5, attackAdd=2
    assert!(
        (data[1].hp_add_rate - 1.5_f32).abs() < 1e-6,
        "hp_add_rate[1] ≈ 1.5"
    );
    assert_eq!(data[1].attack_add, 2, "attack_add[1] = 2");
}

// ─── Tests fixture : parse_chara_swap_motion_config ──────────────────────────

#[test]
fn chara_swap_comptes() {
    let cfg = parse_chara_swap_motion_config(&fixture_chara_swap_motion());
    assert_eq!(cfg.swap_motions.len(), 2, "fixture: 2 SwapMotionData");
    assert_eq!(
        cfg.chara_swap_motions.len(),
        1,
        "fixture: 1 RpgBattleCharaSwapMotionData"
    );
}

#[test]
fn chara_swap_motion_data_0() {
    let cfg = parse_chara_swap_motion_config(&fixture_chara_swap_motion());
    // m_SwapMotionDataList[0] (ligne 10-11) : bodyType=1, motionId=0x2C0F2E35
    assert_eq!(cfg.swap_motions[0].body_type, 1, "body_type[0] = 1");
    assert_eq!(
        cfg.swap_motions[0].motion_id, MOTION_ID_0,
        "motion_id[0] = 0x2C0F2E35"
    );
}

#[test]
fn chara_swap_rpg_data_0() {
    let cfg = parse_chara_swap_motion_config(&fixture_chara_swap_motion());
    // m_RpgBattleCharaSwapMotionDataList[0] (ligne 55) : modelAttribute=3, swapMotion=[0,1]
    assert_eq!(
        cfg.chara_swap_motions[0].model_attribute, 3,
        "model_attribute[0] = 3"
    );
    assert_eq!(
        cfg.chara_swap_motions[0].swap_motion,
        [0_i64, 1_i64],
        "swap_motion[0] = [0,1]"
    );
}

#[test]
fn chara_swap_motions_of() {
    let cfg = parse_chara_swap_motion_config(&fixture_chara_swap_motion());
    let csm = &cfg.chara_swap_motions[0];
    // swap_motion=[0,1] → slice of 1 element starting at index 0
    let slice = cfg.swap_motions_of(csm);
    assert_eq!(slice.len(), 1, "swap_motions_of([0,1]) → 1 élément");
    assert_eq!(slice[0].motion_id, MOTION_ID_0);
}

// ─── Tests fixture : parse_party_config ──────────────────────────────────────

#[test]
fn party_config_comptes() {
    let cfg = parse_party_config(&fixture_party_config());
    assert_eq!(cfg.disable_actions.len(), 2, "fixture: 2 disable_actions");
    assert_eq!(cfg.parties.len(), 2, "fixture: 2 parties");
}

#[test]
fn party_disable_action_id_0() {
    let cfg = parse_party_config(&fixture_party_config());
    // DISABLE_ACTION_0 (ligne 17) : var[0]=-1046485492 → 0xC19FE60C
    assert_eq!(
        cfg.disable_actions[0].action_id, DISABLE_ACTION_ID_0,
        "disable_actions[0].action_id = 0xC19FE60C"
    );
    assert_eq!(
        cfg.disable_actions[0].var1, 0,
        "disable_actions[0].var1 = 0"
    );
}

#[test]
fn party_info_id_0() {
    let cfg = parse_party_config(&fixture_party_config());
    // PARTY_INFO_0 (ligne 84) : var[0]=-1758152796 → 0x9734B7A4
    assert_eq!(
        cfg.parties[0].party_id, PARTY_ID_0,
        "party_id[0] = 0x9734B7A4"
    );
}

#[test]
fn party_info_ref_0() {
    let cfg = parse_party_config(&fixture_party_config());
    // REF_DISABLE_ACTION_UNIQUE_INFO_0 (lignes 97-101) : offset=0, count=1
    assert_eq!(cfg.parties[0].disable_offset, 0, "disable_offset[0] = 0");
    assert_eq!(cfg.parties[0].disable_count, 1, "disable_count[0] = 1");
}

#[test]
fn party_info_ref_1() {
    let cfg = parse_party_config(&fixture_party_config());
    // REF_DISABLE_ACTION_UNIQUE_INFO_1 : offset=1, count=2
    assert_eq!(cfg.parties[1].disable_offset, 1, "disable_offset[1] = 1");
    assert_eq!(cfg.parties[1].disable_count, 2, "disable_count[1] = 2");
}

#[test]
fn party_disable_actions_of() {
    let cfg = parse_party_config(&fixture_party_config());
    // party[0] → disable_offset=0, disable_count=1 → 1 action désactivée
    let actions = cfg.disable_actions_of(&cfg.parties[0]);
    assert_eq!(actions.len(), 1, "disable_actions_of(party[0]) → 1 action");
    assert_eq!(actions[0].action_id, DISABLE_ACTION_ID_0);
}

#[test]
fn party_find_party() {
    let cfg = parse_party_config(&fixture_party_config());
    assert!(
        cfg.find_party(PARTY_ID_0).is_some(),
        "find_party(0x9734B7A4)"
    );
    assert!(cfg.find_party(HashId(0xDEADBEEF)).is_none());
}

// ─── Tests fixture : parse_cmd_event_config ──────────────────────────────────

#[test]
fn cmd_event_compte() {
    let data = parse_cmd_event_config(&fixture_cmd_event());
    assert_eq!(data.len(), 1, "fixture: 1 CMD_EVENT_INFO");
}

#[test]
fn cmd_event_id_0() {
    let data = parse_cmd_event_config(&fixture_cmd_event());
    // CMD_EVENT_INFO_0 (ligne 16) : 20635986 → 0x013AE152
    assert_eq!(data[0], CMD_EVENT_ID_0, "cmd_event[0] = 0x013AE152");
}

// ─── Tests fixture : parse_cmd_obj_config ────────────────────────────────────

#[test]
fn cmd_obj_compte() {
    let data = parse_cmd_obj_config(&fixture_cmd_obj());
    assert_eq!(data.len(), 1, "fixture: 1 CMD_OBJ_INFO");
}

#[test]
fn cmd_obj_id_0() {
    let data = parse_cmd_obj_config(&fixture_cmd_obj());
    // CMD_OBJ_INFO_0 (ligne 13) : 894153704 → 0x354BB3E8
    assert_eq!(data[0], CMD_OBJ_ID_0, "cmd_obj[0] = 0x354BB3E8");
}

// ─── Tests fixture : parse_cmd_set_config ────────────────────────────────────

#[test]
fn cmd_set_comptes() {
    let cfg = parse_cmd_set_config(&fixture_cmd_set());
    assert_eq!(cfg.cmd_data.len(), 2, "fixture: 2 cmd_data");
    assert_eq!(cfg.cmd_sets.len(), 1, "fixture: 1 cmd_set");
}

#[test]
fn cmd_set_cmd_data_0() {
    let cfg = parse_cmd_set_config(&fixture_cmd_set());
    // CMD_SET_CMD_DATA_0 (ligne 17) : -1467292399 → 0xA88AE511
    assert_eq!(cfg.cmd_data[0], CMD_DATA_ID_0, "cmd_data[0] = 0xA88AE511");
}

#[test]
fn cmd_set_info_0() {
    let cfg = parse_cmd_set_config(&fixture_cmd_set());
    // CMD_SET_INFO_0 (ligne 11313) : 1268152025 → 0x4B9676D9
    assert_eq!(
        cfg.cmd_sets[0].set_id, CMD_SET_ID_0,
        "cmd_set[0].set_id = 0x4B9676D9"
    );
}

#[test]
fn cmd_set_ref_0() {
    let cfg = parse_cmd_set_config(&fixture_cmd_set());
    // REF_CMD_DATA_0 (fixture) : offset=0, count=2
    assert_eq!(cfg.cmd_sets[0].cmd_offset, 0, "cmd_set[0].cmd_offset = 0");
    assert_eq!(cfg.cmd_sets[0].cmd_count, 2, "cmd_set[0].cmd_count = 2");
}

#[test]
fn cmd_set_cmds_of() {
    let cfg = parse_cmd_set_config(&fixture_cmd_set());
    let set = &cfg.cmd_sets[0];
    let cmds = cfg.cmds_of(set);
    assert_eq!(cmds.len(), 2, "cmds_of(set[0]) → 2 commandes");
    assert_eq!(cmds[0], CMD_DATA_ID_0);
}

#[test]
fn cmd_set_find_set() {
    let cfg = parse_cmd_set_config(&fixture_cmd_set());
    assert!(cfg.find_set(CMD_SET_ID_0).is_some(), "find_set(0x4B9676D9)");
    assert!(cfg.find_set(HashId(0xDEADBEEF)).is_none());
}

// ─── Tests fixture : parse_add_status_config ─────────────────────────────────

#[test]
fn add_status_comptes() {
    let cfg = parse_add_status_config(&fixture_add_status());
    assert_eq!(cfg.type_infos.len(), 2, "fixture: 2 type_infos");
    assert_eq!(cfg.status_infos.len(), 1, "fixture: 1 status_info");
    assert!(cfg.attr.is_some(), "fixture: 1 attr");
}

#[test]
fn add_status_type_info_0() {
    let cfg = parse_add_status_config(&fixture_add_status());
    // TYPE_INFO_0 (ligne ~22) : var[0]=-1 (toutes catégories)
    assert_eq!(cfg.type_infos[0].type_id, -1, "type_id[0] = -1");
    assert_eq!(cfg.type_infos[0].params, [0_i64; 5], "params[0] = [0;5]");
}

#[test]
fn add_status_type_info_1() {
    let cfg = parse_add_status_config(&fixture_add_status());
    // TYPE_INFO_1 (ligne ~73 dans le vrai fichier) : var[0]=29
    assert_eq!(cfg.type_infos[1].type_id, 29, "type_id[1] = 29");
}

#[test]
fn add_status_info_0_status_id() {
    let cfg = parse_add_status_config(&fixture_add_status());
    // ADD_STATUS_INFO_0 (ligne 354) : -408109186 → 0xE7ACBF7E
    assert_eq!(
        cfg.status_infos[0].status_id, ADD_STATUS_ID_0,
        "status_id[0] = 0xE7ACBF7E"
    );
}

#[test]
fn add_status_info_0_params_count() {
    let cfg = parse_add_status_config(&fixture_add_status());
    assert_eq!(
        cfg.status_infos[0].params.len(),
        1,
        "status_info[0]: 1 param dans la fixture"
    );
}

#[test]
fn add_status_param_0_type_et_rate() {
    let cfg = parse_add_status_config(&fixture_add_status());
    let p = &cfg.status_infos[0].params[0];
    // PARAM_0 (ligne 372) : var[0]=32, var[1]=-1, var[5]="0,2" → rate≈0.2
    assert_eq!(p.param_type, 32, "param_type = 32");
    assert_eq!(p.target, -1, "target = -1");
    assert!((p.rate - 0.2_f32).abs() < 1e-6, "rate ≈ 0.2");
    assert_eq!(p.flag, -1, "flag = -1");
}

#[test]
fn add_status_attr_paires() {
    let cfg = parse_add_status_config(&fixture_add_status());
    let attr = cfg.attr.as_ref().unwrap();
    // fixture : 4 vars → 2 paires
    assert_eq!(attr.entries.len(), 2, "attr: 2 paires dans la fixture");
}

#[test]
fn add_status_attr_paire_0() {
    let cfg = parse_add_status_config(&fixture_add_status());
    let attr = cfg.attr.as_ref().unwrap();
    // paire 0 (lignes 598-604) : var[0]=1 (enabled=true), var[1]=-1356316547 → 0xAF28407D
    assert!(attr.entries[0].enabled, "attr.entries[0].enabled = true");
    assert_eq!(
        attr.entries[0].id, ATTR_ID_0,
        "attr.entries[0].id = 0xAF28407D"
    );
}

#[test]
fn add_status_attr_paire_1() {
    let cfg = parse_add_status_config(&fixture_add_status());
    let attr = cfg.attr.as_ref().unwrap();
    // paire 1 : var[2]=0 (enabled=false), var[3]=-1957340403 → 0x8B555B0D
    assert!(!attr.entries[1].enabled, "attr.entries[1].enabled = false");
    assert_eq!(
        attr.entries[1].id,
        HashId(0x8B55_5B0D),
        "attr.entries[1].id = 0x8B555B0D"
    );
}

// ─── Utilitaire pour tests réels ──────────────────────────────────────────────

fn load_json(path: &str) -> Option<serde_json::Value> {
    let path = common::chemin(path)?;
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

const REAL_RULE: &str = "rpg_battle/rpg_battle_rule_config_1.02.84.00.cfg.bin.json";
const REAL_STATUS_PAT: &str = "rpg_battle/rpg_battle_status_pattern_config_1.03.17.00.cfg.bin.json";
const REAL_SWAP_MOTION: &str =
    "rpg_battle/rpg_battle_chara_swap_motion_config_1.04.06.00.cfg.bin.json";
const REAL_PARTY: &str = "rpg_battle/rpg_battle_party_config_0.00.00.cfg.bin.json";
const REAL_CMD_EVENT: &str = "rpg_battle/rpg_battle_cmd_event_config_0.00.00.cfg.bin.json";
const REAL_CMD_OBJ: &str = "rpg_battle/rpg_battle_cmd_obj_config_0.00.00.cfg.bin.json";
const REAL_CMD_SET: &str = "rpg_battle/rpg_battle_cmd_set_config_1.01.31.00.cfg.bin.json";
const REAL_ADD_STATUS: &str = "rpg_battle/rpg_battle_add_status_config_0.00.00.cfg.bin.json";

// ─── Tests réels : rpg_battle_rule_config ─────────────────────────────────────

#[test]
fn real_file_rule_config_comptes() {
    let Some(root) = load_json(REAL_RULE) else {
        return;
    };
    let cfg = parse_rule_config(&root);
    // m_StartRuleList (ligne 6) : 2 entrées
    assert_eq!(
        cfg.start_rules.len(),
        2,
        "rpg_battle_rule_config: 2 start_rules"
    );
    // m_BattleRuleList (ligne 25) : 13 entrées
    assert_eq!(
        cfg.battle_rules.len(),
        13,
        "rpg_battle_rule_config: 13 battle_rules"
    );
}

#[test]
fn real_file_rule_config_start_rule_0() {
    let Some(root) = load_json(REAL_RULE) else {
        return;
    };
    let cfg = parse_rule_config(&root);
    // m_StartRuleList[0] (ligne 9) : type=1
    assert_eq!(
        cfg.start_rules[0].rule_type, 1,
        "start_rules[0].rule_type = 1"
    );
    assert!(
        cfg.start_rules[0].param_id.is_zero(),
        "start_rules[0].param_id = 0x00000000"
    );
}

#[test]
fn real_file_rule_config_battle_rule_0() {
    let Some(root) = load_json(REAL_RULE) else {
        return;
    };
    let cfg = parse_rule_config(&root);
    // m_BattleRuleList[0] (ligne 28) : ruleId="0x42D90CBC"
    assert_eq!(
        cfg.battle_rules[0].rule_id, RULE_ID_0,
        "battle_rules[0].rule_id = 0x42D90CBC"
    );
    assert_eq!(
        cfg.battle_rules[0].start,
        [0_i64, 0_i64],
        "battle_rules[0].start = [0,0]"
    );
}

#[test]
fn real_file_rule_config_tous_rule_id_non_nuls() {
    let Some(root) = load_json(REAL_RULE) else {
        return;
    };
    let cfg = parse_rule_config(&root);
    let nuls = cfg
        .battle_rules
        .iter()
        .filter(|r| r.rule_id.is_zero())
        .count();
    assert_eq!(nuls, 0, "aucun rule_id nul");
}

// ─── Tests réels : rpg_battle_status_pattern_config ──────────────────────────

#[test]
fn real_file_status_pattern_compte() {
    let Some(root) = load_json(REAL_STATUS_PAT) else {
        return;
    };
    let data = parse_status_pattern_config(&root);
    // m_statusPatternInfoList : 28 entrées (vérifié par grep -c '"id":')
    assert_eq!(
        data.len(),
        28,
        "rpg_battle_status_pattern_config: 28 entrées"
    );
}

#[test]
fn real_file_status_pattern_id_0() {
    let Some(root) = load_json(REAL_STATUS_PAT) else {
        return;
    };
    let data = parse_status_pattern_config(&root);
    // entrée 0 (ligne 9) : id="0xD1002353"
    assert_eq!(
        data[0].id, STATUS_PAT_ID_0,
        "status_pattern[0].id = 0xD1002353"
    );
}

#[test]
fn real_file_status_pattern_hp_rate_0() {
    let Some(root) = load_json(REAL_STATUS_PAT) else {
        return;
    };
    let data = parse_status_pattern_config(&root);
    // entrée 0 (ligne 10) : hpAddRate=0.2
    assert!(
        (data[0].hp_add_rate - 0.2_f32).abs() < 1e-6,
        "hp_add_rate[0] ≈ 0.2"
    );
    assert_eq!(data[0].attack_add, -2, "attack_add[0] = -2");
}

#[test]
fn real_file_status_pattern_entry_10_hp() {
    let Some(root) = load_json(REAL_STATUS_PAT) else {
        return;
    };
    let data = parse_status_pattern_config(&root);
    // entrée 10 : id="0x6C8F6D1C", hpAddRate=1.5, attackAdd=2
    assert_eq!(
        data[10].id,
        HashId(0x6C8F_6D1C),
        "status_pattern[10].id = 0x6C8F6D1C"
    );
    assert!(
        (data[10].hp_add_rate - 1.5_f32).abs() < 1e-6,
        "hp_add_rate[10] ≈ 1.5"
    );
    assert_eq!(data[10].attack_add, 2, "attack_add[10] = 2");
}

#[test]
fn real_file_status_pattern_tous_id_non_nuls() {
    let Some(root) = load_json(REAL_STATUS_PAT) else {
        return;
    };
    let data = parse_status_pattern_config(&root);
    let nuls = data.iter().filter(|p| p.id.is_zero()).count();
    assert_eq!(nuls, 0, "aucun id nul dans status_pattern_config");
}

// ─── Tests réels : rpg_battle_chara_swap_motion_config ───────────────────────

#[test]
fn real_file_swap_motion_comptes() {
    let Some(root) = load_json(REAL_SWAP_MOTION) else {
        return;
    };
    let cfg = parse_chara_swap_motion_config(&root);
    // m_SwapMotionDataList : 10 entrées (lignes 9-44)
    assert_eq!(cfg.swap_motions.len(), 10, "swap_motions: 10 entrées");
    // m_RpgBattleCharaSwapMotionDataList : 2 entrées (lignes 52-70)
    assert_eq!(
        cfg.chara_swap_motions.len(),
        2,
        "chara_swap_motions: 2 entrées"
    );
}

#[test]
fn real_file_swap_motion_data_0() {
    let Some(root) = load_json(REAL_SWAP_MOTION) else {
        return;
    };
    let cfg = parse_chara_swap_motion_config(&root);
    // m_SwapMotionDataList[0] (ligne 10) : bodyType=1, motionId="0x2C0F2E35"
    assert_eq!(
        cfg.swap_motions[0].body_type, 1,
        "swap_motions[0].body_type = 1"
    );
    assert_eq!(
        cfg.swap_motions[0].motion_id, MOTION_ID_0,
        "swap_motions[0].motion_id = 0x2C0F2E35"
    );
}

#[test]
fn real_file_swap_motion_chara_0() {
    let Some(root) = load_json(REAL_SWAP_MOTION) else {
        return;
    };
    let cfg = parse_chara_swap_motion_config(&root);
    // m_RpgBattleCharaSwapMotionDataList[0] (ligne 55) : modelAttribute=3, swapMotion=[0,1]
    assert_eq!(
        cfg.chara_swap_motions[0].model_attribute, 3,
        "chara_swap[0].model_attribute = 3"
    );
    assert_eq!(
        cfg.chara_swap_motions[0].swap_motion,
        [0_i64, 1_i64],
        "chara_swap[0].swap_motion = [0,1]"
    );
}

#[test]
fn real_file_swap_motion_chara_1() {
    let Some(root) = load_json(REAL_SWAP_MOTION) else {
        return;
    };
    let cfg = parse_chara_swap_motion_config(&root);
    // m_RpgBattleCharaSwapMotionDataList[1] (ligne 64) : modelAttribute=2, swapMotion=[1,9]
    assert_eq!(
        cfg.chara_swap_motions[1].model_attribute, 2,
        "chara_swap[1].model_attribute = 2"
    );
    assert_eq!(
        cfg.chara_swap_motions[1].swap_motion,
        [1_i64, 9_i64],
        "chara_swap[1].swap_motion = [1,9]"
    );
}

// ─── Tests réels : rpg_battle_party_config ────────────────────────────────────

#[test]
fn real_file_party_comptes() {
    let Some(root) = load_json(REAL_PARTY) else {
        return;
    };
    let cfg = parse_party_config(&root);
    // RPG_BTL_PARTY_DISABLE_ACTION_UNIQUE_INFO_LIST_BEG_0.var[0] = 4
    assert_eq!(
        cfg.disable_actions.len(),
        4,
        "party_config: 4 disable_actions"
    );
    // RPG_BTL_PARTY_INFO_LIST_BEG_0 : 10 parties
    assert_eq!(cfg.parties.len(), 10, "party_config: 10 parties");
}

#[test]
fn real_file_party_disable_action_0() {
    let Some(root) = load_json(REAL_PARTY) else {
        return;
    };
    let cfg = parse_party_config(&root);
    // DISABLE_ACTION_0 (ligne 17) : -1046485492 → 0xC19FE60C
    assert_eq!(
        cfg.disable_actions[0].action_id, DISABLE_ACTION_ID_0,
        "disable_actions[0].action_id = 0xC19FE60C"
    );
}

#[test]
fn real_file_party_info_0() {
    let Some(root) = load_json(REAL_PARTY) else {
        return;
    };
    let cfg = parse_party_config(&root);
    // PARTY_INFO_0 (ligne 84) : -1758152796 → 0x9734B7A4
    assert_eq!(
        cfg.parties[0].party_id, PARTY_ID_0,
        "parties[0].party_id = 0x9734B7A4"
    );
    // REF_DISABLE_ACTION_UNIQUE_INFO_0 (lignes 97-101) : offset=0, count=1
    assert_eq!(
        cfg.parties[0].disable_offset, 0,
        "parties[0].disable_offset = 0"
    );
    assert_eq!(
        cfg.parties[0].disable_count, 1,
        "parties[0].disable_count = 1"
    );
}

#[test]
fn real_file_party_tous_id_non_nuls() {
    let Some(root) = load_json(REAL_PARTY) else {
        return;
    };
    let cfg = parse_party_config(&root);
    let nuls = cfg.parties.iter().filter(|p| p.party_id.is_zero()).count();
    assert_eq!(nuls, 0, "aucun party_id nul");
}

// ─── Tests réels : rpg_battle_cmd_event_config ───────────────────────────────

#[test]
fn real_file_cmd_event_compte() {
    let Some(root) = load_json(REAL_CMD_EVENT) else {
        return;
    };
    let data = parse_cmd_event_config(&root);
    // RPG_BTL_CMD_EVENT_INFO_LIST_BEG_0.var[0] = 1
    assert_eq!(data.len(), 1, "cmd_event_config: 1 entrée");
}

#[test]
fn real_file_cmd_event_id_0() {
    let Some(root) = load_json(REAL_CMD_EVENT) else {
        return;
    };
    let data = parse_cmd_event_config(&root);
    // RPG_BTL_CMD_EVENT_INFO_0.var[0] = 20635986 → 0x013AE152 (ligne 16)
    assert_eq!(data[0], CMD_EVENT_ID_0, "cmd_event[0] = 0x013AE152");
}

// ─── Tests réels : rpg_battle_cmd_obj_config ─────────────────────────────────

#[test]
fn real_file_cmd_obj_compte() {
    let Some(root) = load_json(REAL_CMD_OBJ) else {
        return;
    };
    let data = parse_cmd_obj_config(&root);
    // RPG_BTL_CMD_OBJ_INFO_LIST_BEG_0.var[0] = 1
    assert_eq!(data.len(), 1, "cmd_obj_config: 1 entrée");
}

#[test]
fn real_file_cmd_obj_id_0() {
    let Some(root) = load_json(REAL_CMD_OBJ) else {
        return;
    };
    let data = parse_cmd_obj_config(&root);
    // RPG_BTL_CMD_OBJ_INFO_0.var[0] = 894153704 → 0x354BB3E8 (ligne 13)
    assert_eq!(data[0], CMD_OBJ_ID_0, "cmd_obj[0] = 0x354BB3E8");
}

// ─── Tests réels : rpg_battle_cmd_set_config ─────────────────────────────────

#[test]
fn real_file_cmd_set_comptes() {
    let Some(root) = load_json(REAL_CMD_SET) else {
        return;
    };
    let cfg = parse_cmd_set_config(&root);
    // RPG_BTL_CMD_SET_CMD_DATA_LIST_BEG_0.var[0] = 1128
    assert_eq!(cfg.cmd_data.len(), 1128, "cmd_set_config: 1128 cmd_data");
    // RPG_BTL_CMD_SET_INFO_LIST_BEG_0.var[0] = 186
    assert_eq!(cfg.cmd_sets.len(), 186, "cmd_set_config: 186 cmd_sets");
}

#[test]
fn real_file_cmd_set_cmd_data_0() {
    let Some(root) = load_json(REAL_CMD_SET) else {
        return;
    };
    let cfg = parse_cmd_set_config(&root);
    // CMD_SET_CMD_DATA_0.var[0] = -1467292399 → 0xA88AE511 (ligne 17)
    assert_eq!(cfg.cmd_data[0], CMD_DATA_ID_0, "cmd_data[0] = 0xA88AE511");
}

#[test]
fn real_file_cmd_set_info_0() {
    let Some(root) = load_json(REAL_CMD_SET) else {
        return;
    };
    let cfg = parse_cmd_set_config(&root);
    // CMD_SET_INFO_0.var[0] = 1268152025 → 0x4B9676D9 (ligne 11313)
    assert_eq!(
        cfg.cmd_sets[0].set_id, CMD_SET_ID_0,
        "cmd_sets[0].set_id = 0x4B9676D9"
    );
    // REF_CMD_DATA_0 (lignes 11320-11328) : offset=0, count=4
    assert_eq!(cfg.cmd_sets[0].cmd_offset, 0, "cmd_sets[0].cmd_offset = 0");
    assert_eq!(cfg.cmd_sets[0].cmd_count, 4, "cmd_sets[0].cmd_count = 4");
}

#[test]
fn real_file_cmd_set_cmds_of_0() {
    let Some(root) = load_json(REAL_CMD_SET) else {
        return;
    };
    let cfg = parse_cmd_set_config(&root);
    let set = &cfg.cmd_sets[0];
    let cmds = cfg.cmds_of(set);
    assert_eq!(cmds.len(), 4, "cmds_of(set[0]) → 4 commandes");
    assert_eq!(cmds[0], CMD_DATA_ID_0, "cmds_of[0][0] = 0xA88AE511");
}

#[test]
fn real_file_cmd_set_tous_non_nuls() {
    let Some(root) = load_json(REAL_CMD_SET) else {
        return;
    };
    let cfg = parse_cmd_set_config(&root);
    let nuls_data = cfg.cmd_data.iter().filter(|h| h.is_zero()).count();
    let nuls_sets = cfg.cmd_sets.iter().filter(|s| s.set_id.is_zero()).count();
    assert_eq!(nuls_data, 0, "aucun cmd_data nul");
    assert_eq!(nuls_sets, 0, "aucun set_id nul");
}

// ─── Tests réels : rpg_battle_add_status_config ──────────────────────────────

#[test]
fn real_file_add_status_comptes() {
    let Some(root) = load_json(REAL_ADD_STATUS) else {
        return;
    };
    let cfg = parse_add_status_config(&root);
    // RPG_BTL_ADD_STATUS_TYPE_INFO_LIST_BEG_0.var[0] = 11
    assert_eq!(cfg.type_infos.len(), 11, "add_status_config: 11 type_infos");
    // RPG_BTL_ADD_STATUS_INFO_LIST_BEG_0 : 6 ADD_STATUS_INFO
    assert_eq!(
        cfg.status_infos.len(),
        6,
        "add_status_config: 6 status_infos"
    );
    // 1 noeud ATTR
    assert!(cfg.attr.is_some(), "add_status_config: 1 attr");
}

#[test]
fn real_file_add_status_type_info_0() {
    let Some(root) = load_json(REAL_ADD_STATUS) else {
        return;
    };
    let cfg = parse_add_status_config(&root);
    // TYPE_INFO_0 (ligne ~22) : var[0]=-1 (toutes catégories)
    assert_eq!(cfg.type_infos[0].type_id, -1, "type_infos[0].type_id = -1");
}

#[test]
fn real_file_add_status_info_0() {
    let Some(root) = load_json(REAL_ADD_STATUS) else {
        return;
    };
    let cfg = parse_add_status_config(&root);
    // ADD_STATUS_INFO_0 (ligne 354) : -408109186 → 0xE7ACBF7E
    assert_eq!(
        cfg.status_infos[0].status_id, ADD_STATUS_ID_0,
        "status_infos[0].status_id = 0xE7ACBF7E"
    );
    // 2 paramètres (PARAM_0 + PARAM_1)
    assert_eq!(
        cfg.status_infos[0].params.len(),
        2,
        "status_infos[0]: 2 params"
    );
}

#[test]
fn real_file_add_status_param_0() {
    let Some(root) = load_json(REAL_ADD_STATUS) else {
        return;
    };
    let cfg = parse_add_status_config(&root);
    let p = &cfg.status_infos[0].params[0];
    // PARAM_0 (ligne 372) : var[0]=32, var[1]=-1, var[5]="0,2", var[14]=-1
    assert_eq!(p.param_type, 32, "params[0].param_type = 32");
    assert_eq!(p.target, -1, "params[0].target = -1");
    assert!((p.rate - 0.2_f32).abs() < 1e-6, "params[0].rate ≈ 0.2");
    assert_eq!(p.flag, -1, "params[0].flag = -1");
}

#[test]
fn real_file_add_status_attr_count() {
    let Some(root) = load_json(REAL_ADD_STATUS) else {
        return;
    };
    let cfg = parse_add_status_config(&root);
    let attr = cfg.attr.as_ref().unwrap();
    // ATTR_0 : 36 vars → 18 paires
    assert_eq!(
        attr.entries.len(),
        18,
        "attr.entries: 18 paires (36 vars / 2)"
    );
}

#[test]
fn real_file_add_status_attr_paire_0() {
    let Some(root) = load_json(REAL_ADD_STATUS) else {
        return;
    };
    let cfg = parse_add_status_config(&root);
    let attr = cfg.attr.as_ref().unwrap();
    // paire 0 (lignes 598-604) : enabled=true, id=-1356316547 → 0xAF28407D
    assert!(attr.entries[0].enabled, "attr.entries[0].enabled = true");
    assert_eq!(
        attr.entries[0].id, ATTR_ID_0,
        "attr.entries[0].id = 0xAF28407D"
    );
}

#[test]
fn real_file_add_status_tous_status_id_non_nuls() {
    let Some(root) = load_json(REAL_ADD_STATUS) else {
        return;
    };
    let cfg = parse_add_status_config(&root);
    let nuls = cfg
        .status_infos
        .iter()
        .filter(|s| s.status_id.is_zero())
        .count();
    assert_eq!(nuls, 0, "aucun status_id nul");
}
