#![allow(clippy::pedantic)]
//! Tests golden `ai` — valeurs réelles tirées de :
//! - `data/common/gamedata/ai/soccer_ai_cmd_config_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/ai/soccer_ai_cmd_config_0.05.91.cfg.bin.json`
//! - `data/common/gamedata/ai/soccer_user_ai_config_1.01.50.cfg.bin.json`
//! - `data/common/gamedata/ai/strategy_ai_config_1.01.50.cfg.bin.json`
//! - `data/common/gamedata/ai/tactics_ai_config_0.06.44.cfg.bin.json`
//!
//! ## Vérifications champ par champ — valeurs réelles
//!
//! ### strategy_ai_config : m_StrategyAITacticsInfoList
//! - `values[0].tacticsId` = "0x906D3411" (ligne 9)
//! - `values[0].tacticsParam` = 0 (ligne 10)
//! - `values[6].tacticsId` = "0x6F57E0D8" (ligne 35), `tacticsParam` = 100 (ligne 36)
//!
//! ### strategy_ai_config : m_StrategyAIInfoList
//! - `values[0].strategyId` = "0x7C24BB03" (ligne 279), `isConfirm` = false
//! - `values[0].tacticsInfo` = [0, 2]
//! - `values[1].strategyId` = "0xFAB0C9AD" (ligne 291), `isConfirm` = true, `evaluationAdjustId` = "0xAD1047E1"
//!
//! ### strategy_ai_config : m_PhaseCheckInfoList
//! - `values[0].phaseId` = "0x3E7D8B80", `phaseStateId` = 0, `condition` = ""
//! - `values[2].phaseStateId` = 1, `cond_id_list_count` = 1
//!
//! ### strategy_ai_config : m_TeamStrategyAIInfoList
//! - `values[0].groupId` = "0x9714544E" (ligne 3311), `strategyInfo` = [0, 3]
//! - `values[4].groupId` = "0x38CB4D40", `strategyInfo` = [43, 13]
//!
//! ### tactics_ai_config : m_TacticsAITargetInfoList
//! - `values[0].targetId` = "0x1C44B74C" (ligne 9), `funcName` = "CalcTargetChara_None"
//! - `values[1].targetId` = "0x6B4387DA" (ligne 13), `funcName` = "CalcTargetChara_Shoot"
//! - `values[11].targetId` = "0xA4F8D029", `funcName` = "CalcTargetPos_None"
//!
//! ### tactics_ai_config : m_TacticsAIActionInfoList
//! - `values[0]` = {actionId: 1, targetCharaType: 0, targetPosType: 10, targetId: "0xBDE3E168"}
//!
//! ### tactics_ai_config : m_TacticsAIInfoList
//! - `values[0].tacticsId` = "0xD23CF887", `actionInfo` = [0, 1]
//!
//! ### tactics_ai_config : m_CharaTacticsAIInfoList
//! - `values[0].groupId` = "0x35664547" (ligne 1261), `tacticsInfo` = [0, 2]
//! - `values[4].groupId` = "0xDA9C897B", `tacticsInfo` = [28, 2]
//!
//! ### soccer_ai_cmd_config : m_updateTimingInfoList
//! - `values[0].updateTimingId` = 2022 (ligne 9)
//! - `values[6].updateTimingId` = 2021
//!
//! ### soccer_ai_cmd_config : m_conditionInfoList
//! - `values[0]` = {conditionId: 1001, updateTimingInfo: [0, 1]} (ligne 234)
//! - `values[6]` = {conditionId: 1007, updateTimingInfo: [6, 2]}
//!
//! ### soccer_user_ai_config : m_userParam
//! - `values[0]` = {id: 1, param: 0.3}
//!
//! ### soccer_user_ai_config : m_coachInfo
//! - `values[0]` = {id: "0x50317469", textId: "0x809D8D96", choise: [0, 3]}

mod common;

extern crate std;

use nie_data::ai::{
    parse_soccer_ai_cmd_config, parse_soccer_user_ai_config, parse_strategy_ai_config,
    parse_tactics_ai_config,
};
use nie_data::hash::HashId;
use serde_json::json;

// ─── Constantes vérifiées ─────────────────────────────────────────────────────

/// "0x906D3411" (strategy_ai_config m_StrategyAITacticsInfoList[0].tacticsId, ligne 9)
const TACTICS_ID_0: HashId = HashId(0x906D3411);
/// "0x6F57E0D8" (m_StrategyAITacticsInfoList[6].tacticsId, ligne 35)
const TACTICS_ID_6: HashId = HashId(0x6F57E0D8);
/// "0x7C24BB03" (m_StrategyAIInfoList[0].strategyId, ligne 279)
const STRATEGY_ID_0: HashId = HashId(0x7C24BB03);
/// "0xFAB0C9AD" (m_StrategyAIInfoList[1].strategyId, ligne 291)
const STRATEGY_ID_1: HashId = HashId(0xFAB0C9AD);
/// "0xAD1047E1" (m_StrategyAIInfoList[1].evaluationAdjustId)
const EVAL_ADJ_ID: HashId = HashId(0xAD1047E1);
/// "0x3E7D8B80" (m_PhaseCheckInfoList[0].phaseId)
const PHASE_ID_0: HashId = HashId(0x3E7D8B80);
/// "0x9714544E" (m_TeamStrategyAIInfoList[0].groupId, ligne 3311)
const TEAM_GROUP_ID_0: HashId = HashId(0x9714544E);
/// "0x38CB4D40" (m_TeamStrategyAIInfoList[4].groupId)
const TEAM_GROUP_ID_4: HashId = HashId(0x38CB4D40);
/// "0x1C44B74C" (tactics_ai_config m_TacticsAITargetInfoList[0].targetId, ligne 9)
const TARGET_ID_0: HashId = HashId(0x1C44B74C);
/// "0x6B4387DA" (m_TacticsAITargetInfoList[1].targetId, ligne 13)
const TARGET_ID_1: HashId = HashId(0x6B4387DA);
/// "0xA4F8D029" (m_TacticsAITargetInfoList[11].targetId)
const TARGET_ID_11: HashId = HashId(0xA4F8D029);
/// "0xBDE3E168" (m_TacticsAIActionInfoList[0].targetId)
const ACTION_TARGET_ID_0: HashId = HashId(0xBDE3E168);
/// "0xD23CF887" (m_TacticsAIInfoList[0].tacticsId)
const TACTICS_INFO_ID_0: HashId = HashId(0xD23CF887);
/// "0x906D3411" (m_CharaTacticsAIDataInfoList[0].tacticsId)
const CHARA_TACTICS_ID_0: HashId = HashId(0x906D3411);
/// "0x35664547" (m_CharaTacticsAIInfoList[0].groupId, ligne 1261)
const CHARA_GROUP_ID_0: HashId = HashId(0x35664547);
/// "0xDA9C897B" (m_CharaTacticsAIInfoList[4].groupId)
const CHARA_GROUP_ID_4: HashId = HashId(0xDA9C897B);
/// "0x50317469" (soccer_user_ai_config m_coachInfo[0].id)
const COACH_ID_0: HashId = HashId(0x50317469);

// ═══════════════════════════════════════════════════════════════════════════════
// Fixtures inline
// ═══════════════════════════════════════════════════════════════════════════════

// ─── Fixture soccer_ai_cmd_config ─────────────────────────────────────────────

fn fixture_soccer_ai_cmd() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_updateTimingInfoList",
                "typeName": "UPDATE_TIMING_INFO",
                "values": [
                    { "updateTimingId": 2022 },
                    { "updateTimingId": 2022 },
                    { "updateTimingId": 1010 },
                    { "updateTimingId": 1010 }
                ]
            },
            {
                "name": "m_conditionInfoList",
                "typeName": "AI_CMD_INFO",
                "values": [
                    // soccer_ai_cmd_config_0.00.00 ligne 234
                    { "conditionId": 1001, "updateTimingInfo": [0, 1] },
                    // ligne 241
                    { "conditionId": 1002, "updateTimingInfo": [1, 1] },
                    // conditionId avec timing_count=0 (soccer ligne 475)
                    { "conditionId": 1210, "updateTimingInfo": [0, 0] }
                ]
            }
        ]
    })
}

// ─── Fixture strategy_ai_config ───────────────────────────────────────────────

fn fixture_strategy_ai() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_StrategyAITacticsInfoList",
                "typeName": "STRATEGY_AI_TACTICS_INFO",
                "values": [
                    // ligne 9 : values[0]
                    { "tacticsId": "0x906D3411", "tacticsParam": 0 },
                    // ligne 13 : values[1]
                    { "tacticsId": "0x16F946BF", "tacticsParam": 0 }
                ]
            },
            {
                "name": "m_StrategyAIInfoList",
                "typeName": "STRATEGY_AI_INFO",
                "values": [
                    // ligne 279 : values[0]
                    {
                        "strategyId": "0x7C24BB03",
                        "nameId": "0x7F0AF7DF",
                        "descId": "0x0C099EA1",
                        "phaseId": "0xD854BC64",
                        "evaluationAdjustId": "0x00000000",
                        "isConfirm": false,
                        "tacticsInfo": [0, 2]
                    },
                    // ligne 291 : values[1]
                    {
                        "strategyId": "0xFAB0C9AD",
                        "nameId": "0xF99E8571",
                        "descId": "0x8A9DEC0F",
                        "phaseId": "0x5EC0CECA",
                        "evaluationAdjustId": "0xAD1047E1",
                        "isConfirm": true,
                        "tacticsInfo": [2, 2]
                    }
                ]
            },
            {
                "name": "m_ConditionIdList",
                "typeName": "CONDITION_ID_INFO",
                "values": [
                    { "conditionId": 1102, "condition": "AAAAAAkCNeHe3UoAAQA=" }
                ]
            },
            {
                "name": "m_ConditionIdList2",
                "typeName": "CONDITION_ID_INFO",
                "values": [
                    { "conditionId": 1301, "condition": "AAAAAAkCNbGmMAoAAQA=" }
                ]
            },
            {
                "name": "m_PhaseCheckInfoList",
                "typeName": "PHASE_CHECK_INFO",
                "values": [
                    // values[0] : phaseStateId=0, conditions vides
                    {
                        "phaseId": "0x3E7D8B80",
                        "phaseStateId": 0,
                        "condition": "",
                        "condition2": "",
                        "conditionIdList": [0, 0],
                        "conditionIdList2": [0, 0]
                    },
                    // values[1]
                    {
                        "phaseId": "0x497ABB16",
                        "phaseStateId": 1,
                        "condition": "",
                        "condition2": "",
                        "conditionIdList": [0, 0],
                        "conditionIdList2": [0, 0]
                    }
                ]
            },
            {
                "name": "m_EvaluationAdjustParamInfoList",
                "typeName": "EVALUATION_ADJUST_PARAM_INFO",
                "values": [
                    { "adjustType": 1, "param": 0 },
                    { "adjustType": 2, "param": 0 }
                ]
            },
            {
                "name": "m_EvaluationAdjustInfoList",
                "typeName": "EVALUATION_ADJUST_INFO",
                "values": [
                    { "evaluationAdustId": "0xAD1047E1", "adjustInfo": [0, 2] }
                ]
            },
            {
                "name": "m_TeamStrategyAIDataInfoList",
                "typeName": "TEAM_STRATEGY_AI_DATA_INFO",
                "values": [
                    { "strategyId": "0x7C24BB03", "enableLv": 1 },
                    { "strategyId": "0xFAB0C9AD", "enableLv": 1 },
                    { "strategyId": "0x31EC1A08", "enableLv": 1 }
                ]
            },
            {
                "name": "m_TeamStrategyAIInfoList",
                "typeName": "TEAM_STRATEGY_AI_INFO",
                "values": [
                    // ligne 3311 : values[0]
                    { "groupId": "0x9714544E", "strategyInfo": [0, 3] }
                ]
            }
        ]
    })
}

// ─── Fixture tactics_ai_config ────────────────────────────────────────────────

fn fixture_tactics_ai() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_TacticsAITargetInfoList",
                "typeName": "TACTICS_AI_TARGET_INFO",
                "values": [
                    // ligne 9 : values[0]
                    { "targetId": "0x1C44B74C", "funcName": "CalcTargetChara_None" },
                    // ligne 13 : values[1]
                    { "targetId": "0x6B4387DA", "funcName": "CalcTargetChara_Shoot" }
                ]
            },
            {
                "name": "m_TacticsAIActionInfoList",
                "typeName": "TACTICS_AI_ACTION_INFO",
                "values": [
                    // values[0] réel
                    { "actionId": 1, "targetCharaType": 0, "targetPosType": 10, "targetId": "0xBDE3E168" },
                    { "actionId": 1, "targetCharaType": 0, "targetPosType": 11, "targetId": "0xCAE4D1FE" }
                ]
            },
            {
                "name": "m_TacticsAIInfoList",
                "typeName": "TACTICS_AI_INFO",
                "values": [
                    // values[0] réel
                    { "tacticsId": "0xD23CF887", "nameId": "0xD4446BCA", "descId": "0x9D6D0350", "actionInfo": [0, 1] },
                    { "tacticsId": "0xA53BC811", "nameId": "0xA3435B5C", "descId": "0xEA6A33C6", "actionInfo": [1, 1] }
                ]
            },
            {
                "name": "m_CharaTacticsAIDataInfoList",
                "typeName": "CHARA_TACTICS_AI_DATA_INFO",
                "values": [
                    { "tacticsId": "0x906D3411", "enableLv": 1 },
                    { "tacticsId": "0x16F946BF", "enableLv": 1 }
                ]
            },
            {
                "name": "m_CharaTacticsAIInfoList",
                "typeName": "CHARA_TACTICS_AI_INFO",
                "values": [
                    // ligne 1261 : values[0]
                    { "groupId": "0x35664547", "tacticsInfo": [0, 2] }
                ]
            }
        ]
    })
}

// ─── Fixture soccer_user_ai_config ────────────────────────────────────────────

fn fixture_soccer_user_ai() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_userParam",
                "typeName": "SoccerUserAIParam",
                "values": [
                    { "id": 1, "param": 0.3 },
                    { "id": 2, "param": 0.3 }
                ]
            },
            {
                "name": "m_coachEffect",
                "typeName": "SoccerCoachAIEffect",
                "values": [
                    { "type": 26, "calc": 1, "value": 0 },
                    { "type": 1, "calc": 0, "value": 1.0 }
                ]
            },
            {
                "name": "m_coachChoise",
                "typeName": "SoccerCoachAIChoise",
                "values": [
                    {
                        "idx": 0,
                        "textId": "0xD3725F39",
                        "timing": 0,
                        "geoId": "0x00000000",
                        "cameraId": "0x00000000",
                        "time": 0,
                        "offsetX": 0,
                        "offsetZ": 0,
                        "speachId": "0xC2A4169A",
                        "effect": [0, 1]
                    }
                ]
            },
            {
                "name": "m_coachInfo",
                "typeName": "SoccerCoachAIInfo",
                "values": [
                    { "id": "0x50317469", "textId": "0x809D8D96", "choise": [0, 1] }
                ]
            },
            {
                "name": "m_titleEffect",
                "typeName": "SoccerTitleAIEffect",
                "values": [
                    { "type": 17, "calc": 0, "value": 100.0 }
                ]
            },
            {
                "name": "m_titleConditionId",
                "typeName": "SoccerTitleAICondId",
                "values": [
                    { "conditionId": 1113, "condition": "AAAAAA8FNWokrVsAAQAyAAAAAXg=" }
                ]
            },
            {
                "name": "m_titleInfo",
                "typeName": "SoccerTitleAIInfo",
                "values": [
                    {
                        "id": "0x0312BC99",
                        "textId": "0x7C138CF6",
                        "type": 0,
                        "limitType": 0,
                        "limitParam": 1,
                        "condition": "AAAAAB4LNWokrVsAAQAyAAAAAXg1bH8t4wABADM+zMzNcY8=",
                        "conditionId": [0, 1],
                        "effect": [0, 1]
                    }
                ]
            }
        ]
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests sur fixtures : soccer_ai_cmd_config
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn soccer_ai_cmd_comptes_fixture() {
    let cfg = parse_soccer_ai_cmd_config(&fixture_soccer_ai_cmd());
    assert_eq!(
        cfg.update_timings.len(),
        4,
        "4 UPDATE_TIMING dans la fixture"
    );
    assert_eq!(cfg.conditions.len(), 3, "3 AI_CMD dans la fixture");
}

#[test]
fn soccer_ai_cmd_timing_0() {
    let cfg = parse_soccer_ai_cmd_config(&fixture_soccer_ai_cmd());
    // soccer_ai_cmd_config_0.00.00.cfg.bin.json ligne 9 : values[0].updateTimingId = 2022
    assert_eq!(
        cfg.update_timings[0].update_timing_id, 2022,
        "timing[0] = 2022"
    );
}

#[test]
fn soccer_ai_cmd_condition_0() {
    let cfg = parse_soccer_ai_cmd_config(&fixture_soccer_ai_cmd());
    // soccer_ai_cmd_config_0.00.00.cfg.bin.json ligne 234 :
    // values[0] = {conditionId: 1001, updateTimingInfo: [0, 1]}
    let c = &cfg.conditions[0];
    assert_eq!(c.condition_id, 1001, "condition[0].condition_id = 1001");
    assert_eq!(c.timing_offset, 0, "condition[0].timing_offset = 0");
    assert_eq!(c.timing_count, 1, "condition[0].timing_count = 1");
}

#[test]
fn soccer_ai_cmd_condition_2_timing_nul() {
    let cfg = parse_soccer_ai_cmd_config(&fixture_soccer_ai_cmd());
    // soccer_ai_cmd_config_0.00.00.cfg.bin.json ligne 475 :
    // conditionId=1210, updateTimingInfo=[0, 0]
    let c = &cfg.conditions[2];
    assert_eq!(c.condition_id, 1210, "condition[2].condition_id = 1210");
    assert_eq!(c.timing_count, 0, "condition[2].timing_count = 0");
    // timings_of doit renvoyer une tranche vide
    let timings = cfg.timings_of(c);
    assert_eq!(timings.len(), 0, "timings_of(1210) = vide");
}

#[test]
fn soccer_ai_cmd_timings_of() {
    let cfg = parse_soccer_ai_cmd_config(&fixture_soccer_ai_cmd());
    let c0 = &cfg.conditions[0]; // offset=0, count=1
    let timings = cfg.timings_of(c0);
    assert_eq!(timings.len(), 1, "timings_of(1001) = 1 timing");
    assert_eq!(timings[0].update_timing_id, 2022, "timing réel = 2022");
}

#[test]
fn soccer_ai_cmd_find_condition() {
    let cfg = parse_soccer_ai_cmd_config(&fixture_soccer_ai_cmd());
    assert!(
        cfg.find_condition(1001).is_some(),
        "find_condition(1001) trouvé"
    );
    assert!(
        cfg.find_condition(9999).is_none(),
        "find_condition(9999) absent"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests sur fixtures : strategy_ai_config
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn strategy_ai_comptes_fixture() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    assert_eq!(
        cfg.tactics_infos.len(),
        2,
        "2 STRATEGY_AI_TACTICS_INFO dans la fixture"
    );
    assert_eq!(cfg.strategy_infos.len(), 2, "2 STRATEGY_AI_INFO");
    assert_eq!(cfg.condition_ids.len(), 1, "1 CONDITION_ID_INFO liste1");
    assert_eq!(cfg.condition_ids2.len(), 1, "1 CONDITION_ID_INFO liste2");
    assert_eq!(cfg.phase_checks.len(), 2, "2 PHASE_CHECK_INFO");
    assert_eq!(
        cfg.eval_adjust_params.len(),
        2,
        "2 EVALUATION_ADJUST_PARAM_INFO"
    );
    assert_eq!(cfg.eval_adjusts.len(), 1, "1 EVALUATION_ADJUST_INFO");
    assert_eq!(
        cfg.team_strategy_data.len(),
        3,
        "3 TEAM_STRATEGY_AI_DATA_INFO"
    );
    assert_eq!(cfg.team_strategy_infos.len(), 1, "1 TEAM_STRATEGY_AI_INFO");
}

#[test]
fn strategy_ai_tactics_info_0() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    // strategy_ai_config_1.01.50.cfg.bin.json ligne 9
    let t = &cfg.tactics_infos[0];
    assert_eq!(
        t.tactics_id, TACTICS_ID_0,
        "tactics_infos[0].tactics_id = 0x906D3411"
    );
    assert_eq!(t.tactics_param, 0, "tactics_infos[0].tactics_param = 0");
}

#[test]
fn strategy_ai_info_0_champs() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    // strategy_ai_config_1.01.50.cfg.bin.json ligne 279
    let s = &cfg.strategy_infos[0];
    assert_eq!(s.strategy_id, STRATEGY_ID_0, "strategy_id[0] = 0x7C24BB03");
    assert!(!s.is_confirm, "is_confirm[0] = false");
    assert_eq!(s.tactics_offset, 0, "tactics_offset[0] = 0");
    assert_eq!(s.tactics_count, 2, "tactics_count[0] = 2");
    assert!(
        s.evaluation_adjust_id.is_zero(),
        "evaluationAdjustId[0] = 0x0 (aucun)"
    );
}

#[test]
fn strategy_ai_info_1_confirm_et_adj() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    // strategy_ai_config_1.01.50.cfg.bin.json ligne 291
    let s = &cfg.strategy_infos[1];
    assert_eq!(s.strategy_id, STRATEGY_ID_1, "strategy_id[1] = 0xFAB0C9AD");
    assert!(s.is_confirm, "is_confirm[1] = true");
    assert_eq!(
        s.evaluation_adjust_id, EVAL_ADJ_ID,
        "evaluationAdjustId[1] = 0xAD1047E1"
    );
    assert_eq!(s.tactics_offset, 2, "tactics_offset[1] = 2");
    assert_eq!(s.tactics_count, 2, "tactics_count[1] = 2");
}

#[test]
fn strategy_ai_tactics_of() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    let s0 = &cfg.strategy_infos[0]; // offset=0, count=2
    let tactics = cfg.tactics_of(s0);
    assert_eq!(tactics.len(), 2, "tactics_of(strategy[0]) = 2 tactiques");
    assert_eq!(
        tactics[0].tactics_id, TACTICS_ID_0,
        "tactics[0] = 0x906D3411"
    );
}

#[test]
fn strategy_ai_find_strategy() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    let found = cfg.find_strategy(STRATEGY_ID_0);
    assert!(found.is_some(), "find_strategy(0x7C24BB03) trouvé");
    assert!(!found.unwrap().is_confirm, "is_confirm = false");
    assert!(cfg.find_strategy(HashId(0xDEADBEEF)).is_none());
}

#[test]
fn strategy_ai_phase_check_0() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    // values[0] : phaseId = "0x3E7D8B80", phaseStateId = 0, condition = ""
    let p = &cfg.phase_checks[0];
    assert_eq!(p.phase_id, PHASE_ID_0, "phase_id[0] = 0x3E7D8B80");
    assert_eq!(p.phase_state_id, 0, "phase_state_id[0] = 0");
    assert!(p.condition.is_empty(), "condition[0] = \"\"");
    assert_eq!(p.cond_id_list_count, 0, "cond_id_list_count[0] = 0");
}

#[test]
fn strategy_ai_eval_adjust_info_0() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    // m_EvaluationAdjustInfoList[0] : evaluationAdustId = "0xAD1047E1", adjustInfo = [0, 2]
    let a = &cfg.eval_adjusts[0];
    assert_eq!(
        a.evaluation_adjust_id, EVAL_ADJ_ID,
        "eval_adj_id[0] = 0xAD1047E1"
    );
    assert_eq!(a.adjust_offset, 0, "adjust_offset[0] = 0");
    assert_eq!(a.adjust_count, 2, "adjust_count[0] = 2");
}

#[test]
fn strategy_ai_adjust_params_of() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    let adj = &cfg.eval_adjusts[0]; // offset=0, count=2
    let params = cfg.adjust_params_of(adj);
    assert_eq!(params.len(), 2, "adjust_params_of(adj[0]) = 2 params");
    // m_EvaluationAdjustParamInfoList[0] = {adjustType: 1, param: 0}
    assert_eq!(params[0].adjust_type, 1, "param[0].adjust_type = 1");
    assert_eq!(params[0].param, 0, "param[0].param = 0");
}

#[test]
fn strategy_ai_team_group_0() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    // ligne 3311 : values[0] = {groupId: "0x9714544E", strategyInfo: [0, 3]}
    let g = &cfg.team_strategy_infos[0];
    assert_eq!(g.group_id, TEAM_GROUP_ID_0, "group_id[0] = 0x9714544E");
    assert_eq!(g.strategy_offset, 0, "strategy_offset[0] = 0");
    assert_eq!(g.strategy_count, 3, "strategy_count[0] = 3");
}

#[test]
fn strategy_ai_strategy_data_of() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    let g = &cfg.team_strategy_infos[0]; // offset=0, count=3
    let data = cfg.strategy_data_of(g);
    assert_eq!(data.len(), 3, "strategy_data_of(group[0]) = 3 entrées");
    assert_eq!(
        data[0].strategy_id, STRATEGY_ID_0,
        "data[0].strategy_id = 0x7C24BB03"
    );
    assert_eq!(data[0].enable_lv, 1, "data[0].enable_lv = 1");
}

#[test]
fn strategy_ai_condition_id_list() {
    let cfg = parse_strategy_ai_config(&fixture_strategy_ai());
    // m_ConditionIdList[0] = {conditionId: 1102, condition: "AAAAAAkCNeHe3UoAAQA="}
    assert_eq!(cfg.condition_ids[0].condition_id, 1102, "cond_id[0] = 1102");
    assert_eq!(
        cfg.condition_ids[0].condition, "AAAAAAkCNeHe3UoAAQA=",
        "cond[0] = base64 attendu"
    );
    // m_ConditionIdList2[0] = {conditionId: 1301}
    assert_eq!(
        cfg.condition_ids2[0].condition_id, 1301,
        "cond_id2[0] = 1301"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests sur fixtures : tactics_ai_config
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn tactics_ai_comptes_fixture() {
    let cfg = parse_tactics_ai_config(&fixture_tactics_ai());
    assert_eq!(
        cfg.target_infos.len(),
        2,
        "2 TACTICS_AI_TARGET_INFO dans la fixture"
    );
    assert_eq!(cfg.action_infos.len(), 2, "2 TACTICS_AI_ACTION_INFO");
    assert_eq!(cfg.tactics_infos.len(), 2, "2 TACTICS_AI_INFO");
    assert_eq!(
        cfg.chara_tactics_data.len(),
        2,
        "2 CHARA_TACTICS_AI_DATA_INFO"
    );
    assert_eq!(cfg.chara_tactics_infos.len(), 1, "1 CHARA_TACTICS_AI_INFO");
}

#[test]
fn tactics_ai_target_0() {
    let cfg = parse_tactics_ai_config(&fixture_tactics_ai());
    // tactics_ai_config_0.06.44.cfg.bin.json ligne 9
    let t = &cfg.target_infos[0];
    assert_eq!(t.target_id, TARGET_ID_0, "target_id[0] = 0x1C44B74C");
    assert_eq!(
        t.func_name, "CalcTargetChara_None",
        "func_name[0] = CalcTargetChara_None"
    );
}

#[test]
fn tactics_ai_target_1() {
    let cfg = parse_tactics_ai_config(&fixture_tactics_ai());
    // ligne 13
    let t = &cfg.target_infos[1];
    assert_eq!(t.target_id, TARGET_ID_1, "target_id[1] = 0x6B4387DA");
    assert_eq!(
        t.func_name, "CalcTargetChara_Shoot",
        "func_name[1] = CalcTargetChara_Shoot"
    );
}

#[test]
fn tactics_ai_action_0() {
    let cfg = parse_tactics_ai_config(&fixture_tactics_ai());
    // m_TacticsAIActionInfoList[0] réel
    let a = &cfg.action_infos[0];
    assert_eq!(a.action_id, 1, "action_id[0] = 1");
    assert_eq!(a.target_chara_type, 0, "target_chara_type[0] = 0");
    assert_eq!(a.target_pos_type, 10, "target_pos_type[0] = 10");
    assert_eq!(a.target_id, ACTION_TARGET_ID_0, "target_id[0] = 0xBDE3E168");
}

#[test]
fn tactics_ai_info_0() {
    let cfg = parse_tactics_ai_config(&fixture_tactics_ai());
    // m_TacticsAIInfoList[0] réel
    let t = &cfg.tactics_infos[0];
    assert_eq!(
        t.tactics_id, TACTICS_INFO_ID_0,
        "tactics_id[0] = 0xD23CF887"
    );
    assert_eq!(t.action_offset, 0, "action_offset[0] = 0");
    assert_eq!(t.action_count, 1, "action_count[0] = 1");
}

#[test]
fn tactics_ai_actions_of() {
    let cfg = parse_tactics_ai_config(&fixture_tactics_ai());
    let t0 = &cfg.tactics_infos[0]; // offset=0, count=1
    let actions = cfg.actions_of(t0);
    assert_eq!(actions.len(), 1, "actions_of(tactics[0]) = 1 action");
    assert_eq!(actions[0].action_id, 1, "action[0].action_id = 1");
    assert_eq!(
        actions[0].target_id, ACTION_TARGET_ID_0,
        "action[0].target_id = 0xBDE3E168"
    );
}

#[test]
fn tactics_ai_find_target() {
    let cfg = parse_tactics_ai_config(&fixture_tactics_ai());
    let found = cfg.find_target(TARGET_ID_0);
    assert!(found.is_some(), "find_target(0x1C44B74C) trouvé");
    assert_eq!(found.unwrap().func_name, "CalcTargetChara_None");
    assert!(cfg.find_target(HashId(0xDEADBEEF)).is_none());
}

#[test]
fn tactics_ai_find_tactics() {
    let cfg = parse_tactics_ai_config(&fixture_tactics_ai());
    let found = cfg.find_tactics(TACTICS_INFO_ID_0);
    assert!(found.is_some(), "find_tactics(0xD23CF887) trouvé");
    assert!(cfg.find_tactics(HashId(0xDEADBEEF)).is_none());
}

#[test]
fn tactics_ai_chara_group_0() {
    let cfg = parse_tactics_ai_config(&fixture_tactics_ai());
    // ligne 1261 : values[0] = {groupId: "0x35664547", tacticsInfo: [0, 2]}
    let g = &cfg.chara_tactics_infos[0];
    assert_eq!(g.group_id, CHARA_GROUP_ID_0, "group_id[0] = 0x35664547");
    assert_eq!(g.tactics_offset, 0, "tactics_offset[0] = 0");
    assert_eq!(g.tactics_count, 2, "tactics_count[0] = 2");
}

#[test]
fn tactics_ai_tactics_data_of() {
    let cfg = parse_tactics_ai_config(&fixture_tactics_ai());
    let g = &cfg.chara_tactics_infos[0]; // offset=0, count=2
    let data = cfg.tactics_data_of(g);
    assert_eq!(data.len(), 2, "tactics_data_of(group[0]) = 2 entrées");
    assert_eq!(
        data[0].tactics_id, CHARA_TACTICS_ID_0,
        "data[0].tactics_id = 0x906D3411"
    );
    assert_eq!(data[0].enable_lv, 1, "data[0].enable_lv = 1");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests sur fixtures : soccer_user_ai_config
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn soccer_user_ai_comptes_fixture() {
    let cfg = parse_soccer_user_ai_config(&fixture_soccer_user_ai());
    assert_eq!(
        cfg.user_params.len(),
        2,
        "2 SoccerUserAIParam dans la fixture"
    );
    assert_eq!(cfg.coach_effects.len(), 2, "2 SoccerCoachAIEffect");
    assert_eq!(cfg.coach_choises.len(), 1, "1 SoccerCoachAIChoise");
    assert_eq!(cfg.coach_infos.len(), 1, "1 SoccerCoachAIInfo");
    assert_eq!(cfg.title_effects.len(), 1, "1 SoccerTitleAIEffect");
    assert_eq!(cfg.title_cond_ids.len(), 1, "1 SoccerTitleAICondId");
    assert_eq!(cfg.title_infos.len(), 1, "1 SoccerTitleAIInfo");
}

#[test]
fn soccer_user_ai_param_0() {
    let cfg = parse_soccer_user_ai_config(&fixture_soccer_user_ai());
    // m_userParam[0] = {id: 1, param: 0.3}
    let p = &cfg.user_params[0];
    assert_eq!(p.id, 1, "user_params[0].id = 1");
    assert!(
        (p.param - 0.3_f32).abs() < 1e-5,
        "user_params[0].param ≈ 0.3"
    );
}

#[test]
fn soccer_user_ai_find_user_param() {
    let cfg = parse_soccer_user_ai_config(&fixture_soccer_user_ai());
    assert!(
        cfg.find_user_param(1).is_some(),
        "find_user_param(1) trouvé"
    );
    assert!(
        cfg.find_user_param(99).is_none(),
        "find_user_param(99) absent"
    );
}

#[test]
fn soccer_user_ai_coach_effect_0() {
    let cfg = parse_soccer_user_ai_config(&fixture_soccer_user_ai());
    // m_coachEffect[0] = {type: 26, calc: 1, value: 0}
    let e = &cfg.coach_effects[0];
    assert_eq!(e.effect_type, 26, "coach_effect[0].type = 26");
    assert_eq!(e.calc, 1, "coach_effect[0].calc = 1");
    assert_eq!(e.value, 0.0_f32, "coach_effect[0].value = 0");
}

#[test]
fn soccer_user_ai_coach_choise_0() {
    let cfg = parse_soccer_user_ai_config(&fixture_soccer_user_ai());
    // m_coachChoise[0] : textId = "0xD3725F39", speachId = "0xC2A4169A", effect = [0, 1]
    let c = &cfg.coach_choises[0];
    assert_eq!(
        c.text_id,
        HashId(0xD3725F39),
        "choise[0].text_id = 0xD3725F39"
    );
    assert_eq!(
        c.speach_id,
        HashId(0xC2A4169A),
        "choise[0].speach_id = 0xC2A4169A"
    );
    assert_eq!(c.effect_offset, 0, "choise[0].effect_offset = 0");
    assert_eq!(c.effect_count, 1, "choise[0].effect_count = 1");
    assert!(c.geo_id.is_zero(), "choise[0].geo_id = 0 (aucune géo)");
}

#[test]
fn soccer_user_ai_coach_info_0() {
    let cfg = parse_soccer_user_ai_config(&fixture_soccer_user_ai());
    // m_coachInfo[0] = {id: "0x50317469", textId: "0x809D8D96", choise: [0, 1]}
    let c = &cfg.coach_infos[0];
    assert_eq!(c.id, COACH_ID_0, "coach_info[0].id = 0x50317469");
    assert_eq!(
        c.text_id,
        HashId(0x809D8D96),
        "coach_info[0].text_id = 0x809D8D96"
    );
    assert_eq!(c.choise_offset, 0, "coach_info[0].choise_offset = 0");
    assert_eq!(c.choise_count, 1, "coach_info[0].choise_count = 1");
}

#[test]
fn soccer_user_ai_choises_of() {
    let cfg = parse_soccer_user_ai_config(&fixture_soccer_user_ai());
    let coach = &cfg.coach_infos[0]; // offset=0, count=1
    let choises = cfg.choises_of(coach);
    assert_eq!(choises.len(), 1, "choises_of(coach[0]) = 1 choix");
    assert_eq!(
        choises[0].text_id,
        HashId(0xD3725F39),
        "choix[0].text_id = 0xD3725F39"
    );
}

#[test]
fn soccer_user_ai_title_cond_id_0() {
    let cfg = parse_soccer_user_ai_config(&fixture_soccer_user_ai());
    // m_titleConditionId[0] = {conditionId: 1113, condition: "AAAAAA8FNWokrVsAAQAyAAAAAXg="}
    let c = &cfg.title_cond_ids[0];
    assert_eq!(
        c.condition_id, 1113,
        "title_cond_ids[0].condition_id = 1113"
    );
    assert_eq!(
        c.condition, "AAAAAA8FNWokrVsAAQAyAAAAAXg=",
        "title_cond_ids[0].condition = base64 attendu"
    );
}

#[test]
fn soccer_user_ai_title_info_0() {
    let cfg = parse_soccer_user_ai_config(&fixture_soccer_user_ai());
    // m_titleInfo[0] = {id: "0x0312BC99", type: 0, limitParam: 1, conditionId: [0, 1], effect: [0, 1]}
    let t = &cfg.title_infos[0];
    assert_eq!(t.id, HashId(0x0312BC99), "title_info[0].id = 0x0312BC99");
    assert_eq!(t.info_type, 0, "title_info[0].type = 0");
    assert_eq!(t.limit_param, 1, "title_info[0].limit_param = 1");
    assert_eq!(
        t.condition_id_offset, 0,
        "title_info[0].condition_id_offset = 0"
    );
    assert_eq!(
        t.condition_id_count, 1,
        "title_info[0].condition_id_count = 1"
    );
    assert_eq!(t.effect_offset, 0, "title_info[0].effect_offset = 0");
    assert_eq!(t.effect_count, 1, "title_info[0].effect_count = 1");
    assert!(!t.condition.is_empty(), "title_info[0].condition non vide");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests sur fichiers réels (skip silencieux si absents)
// ═══════════════════════════════════════════════════════════════════════════════

const DIR: &str = "ai";
const REAL_SOCCER_CMD_V0: &str = "ai/soccer_ai_cmd_config_0.00.00.cfg.bin.json";
const REAL_SOCCER_CMD_V5: &str = "ai/soccer_ai_cmd_config_0.05.91.cfg.bin.json";
const REAL_SOCCER_USER: &str = "ai/soccer_user_ai_config_1.01.50.cfg.bin.json";
const REAL_STRATEGY: &str = "ai/strategy_ai_config_1.01.50.cfg.bin.json";
const REAL_TACTICS: &str = "ai/tactics_ai_config_0.06.44.cfg.bin.json";

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

// ─── soccer_ai_cmd_config v0.00.00 ────────────────────────────────────────────

#[test]
fn real_file_soccer_cmd_v0_comptes() {
    let Some(root) = load_json(REAL_SOCCER_CMD_V0) else {
        return;
    };
    let cfg = parse_soccer_ai_cmd_config(&root);
    // soccer_ai_cmd_config_0.00.00 : 73 UPDATE_TIMING + 71 AI_CMD
    assert_eq!(
        cfg.update_timings.len(),
        73,
        "soccer_ai_cmd v0 : 73 timings"
    );
    assert_eq!(cfg.conditions.len(), 71, "soccer_ai_cmd v0 : 71 conditions");
}

#[test]
fn real_file_soccer_cmd_v0_timing_0() {
    let Some(root) = load_json(REAL_SOCCER_CMD_V0) else {
        return;
    };
    let cfg = parse_soccer_ai_cmd_config(&root);
    // soccer_ai_cmd_config_0.00.00.cfg.bin.json ligne 9 : values[0].updateTimingId = 2022
    assert_eq!(
        cfg.update_timings[0].update_timing_id, 2022,
        "timing[0] = 2022 (ligne 9)"
    );
}

#[test]
fn real_file_soccer_cmd_v0_timing_6() {
    let Some(root) = load_json(REAL_SOCCER_CMD_V0) else {
        return;
    };
    let cfg = parse_soccer_ai_cmd_config(&root);
    // values[6].updateTimingId = 2021
    assert_eq!(
        cfg.update_timings[6].update_timing_id, 2021,
        "timing[6] = 2021"
    );
}

#[test]
fn real_file_soccer_cmd_v0_condition_0() {
    let Some(root) = load_json(REAL_SOCCER_CMD_V0) else {
        return;
    };
    let cfg = parse_soccer_ai_cmd_config(&root);
    // soccer_ai_cmd_config_0.00.00.cfg.bin.json ligne 234 :
    // values[0] = {conditionId: 1001, updateTimingInfo: [0, 1]}
    let c = &cfg.conditions[0];
    assert_eq!(
        c.condition_id, 1001,
        "condition[0].condition_id = 1001 (ligne 234)"
    );
    assert_eq!(c.timing_offset, 0, "condition[0].timing_offset = 0");
    assert_eq!(c.timing_count, 1, "condition[0].timing_count = 1");
}

#[test]
fn real_file_soccer_cmd_v0_condition_6() {
    let Some(root) = load_json(REAL_SOCCER_CMD_V0) else {
        return;
    };
    let cfg = parse_soccer_ai_cmd_config(&root);
    // values[6] = {conditionId: 1007, updateTimingInfo: [6, 2]}
    let c = &cfg.conditions[6];
    assert_eq!(c.condition_id, 1007, "condition[6].condition_id = 1007");
    assert_eq!(c.timing_offset, 6, "condition[6].timing_offset = 6");
    assert_eq!(c.timing_count, 2, "condition[6].timing_count = 2");
}

// ─── soccer_ai_cmd_config v0.05.91 ────────────────────────────────────────────

#[test]
fn real_file_soccer_cmd_v5_comptes() {
    let Some(root) = load_json(REAL_SOCCER_CMD_V5) else {
        return;
    };
    let cfg = parse_soccer_ai_cmd_config(&root);
    // soccer_ai_cmd_config_0.05.91 : 80 UPDATE_TIMING + 77 AI_CMD
    assert_eq!(
        cfg.update_timings.len(),
        80,
        "soccer_ai_cmd v5 : 80 timings"
    );
    assert_eq!(cfg.conditions.len(), 77, "soccer_ai_cmd v5 : 77 conditions");
}

#[test]
fn real_file_soccer_cmd_v5_condition_0() {
    let Some(root) = load_json(REAL_SOCCER_CMD_V5) else {
        return;
    };
    let cfg = parse_soccer_ai_cmd_config(&root);
    // soccer_ai_cmd_config_0.05.91 : values[0] = {conditionId: 1, updateTimingInfo: [0, 1]}
    let c = &cfg.conditions[0];
    assert_eq!(
        c.condition_id, 1,
        "soccer_cmd v5 condition[0].condition_id = 1"
    );
    assert_eq!(
        c.timing_count, 1,
        "soccer_cmd v5 condition[0].timing_count = 1"
    );
}

// ─── soccer_user_ai_config ────────────────────────────────────────────────────

#[test]
fn real_file_soccer_user_comptes() {
    let Some(root) = load_json(REAL_SOCCER_USER) else {
        return;
    };
    let cfg = parse_soccer_user_ai_config(&root);
    assert_eq!(cfg.user_params.len(), 26, "soccer_user_ai : 26 user_params");
    assert_eq!(
        cfg.coach_effects.len(),
        20,
        "soccer_user_ai : 20 coach_effects"
    );
    assert_eq!(
        cfg.coach_choises.len(),
        11,
        "soccer_user_ai : 11 coach_choises"
    );
    assert_eq!(cfg.coach_infos.len(), 4, "soccer_user_ai : 4 coach_infos");
    assert_eq!(
        cfg.title_effects.len(),
        13,
        "soccer_user_ai : 13 title_effects"
    );
    assert_eq!(
        cfg.title_cond_ids.len(),
        6,
        "soccer_user_ai : 6 title_cond_ids"
    );
    assert_eq!(cfg.title_infos.len(), 4, "soccer_user_ai : 4 title_infos");
}

#[test]
fn real_file_soccer_user_param_0() {
    let Some(root) = load_json(REAL_SOCCER_USER) else {
        return;
    };
    let cfg = parse_soccer_user_ai_config(&root);
    // m_userParam[0] = {id: 1, param: 0.3}
    let p = &cfg.user_params[0];
    assert_eq!(p.id, 1, "user_params[0].id = 1");
    assert!(
        (p.param - 0.3_f32).abs() < 1e-5,
        "user_params[0].param ≈ 0.3"
    );
}

#[test]
fn real_file_soccer_user_param_tous_non_nuls() {
    let Some(root) = load_json(REAL_SOCCER_USER) else {
        return;
    };
    let cfg = parse_soccer_user_ai_config(&root);
    // Tous les id 1..26 doivent être présents
    let ids: std::vec::Vec<i64> = cfg.user_params.iter().map(|p| p.id).collect();
    assert!(ids.contains(&1), "id=1 présent");
    assert!(ids.contains(&26), "id=26 présent");
}

#[test]
fn real_file_soccer_user_coach_effect_0() {
    let Some(root) = load_json(REAL_SOCCER_USER) else {
        return;
    };
    let cfg = parse_soccer_user_ai_config(&root);
    // m_coachEffect[0] = {type: 26, calc: 1, value: 0}
    assert_eq!(
        cfg.coach_effects[0].effect_type, 26,
        "coach_effect[0].type = 26"
    );
    assert_eq!(cfg.coach_effects[0].calc, 1, "coach_effect[0].calc = 1");
}

#[test]
fn real_file_soccer_user_coach_info_0() {
    let Some(root) = load_json(REAL_SOCCER_USER) else {
        return;
    };
    let cfg = parse_soccer_user_ai_config(&root);
    // m_coachInfo[0] = {id: "0x50317469", textId: "0x809D8D96", choise: [0, 3]}
    let c = &cfg.coach_infos[0];
    assert_eq!(c.id, COACH_ID_0, "coach_info[0].id = 0x50317469");
    assert_eq!(c.choise_count, 3, "coach_info[0].choise_count = 3");
}

#[test]
fn real_file_soccer_user_title_cond_id_0() {
    let Some(root) = load_json(REAL_SOCCER_USER) else {
        return;
    };
    let cfg = parse_soccer_user_ai_config(&root);
    // m_titleConditionId[0] = {conditionId: 1113, ...}
    assert_eq!(
        cfg.title_cond_ids[0].condition_id, 1113,
        "title_cond_ids[0].condition_id = 1113"
    );
}

// ─── strategy_ai_config ───────────────────────────────────────────────────────

#[test]
fn real_file_strategy_comptes() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    assert_eq!(cfg.tactics_infos.len(), 66, "strategy : 66 tactics_infos");
    assert_eq!(cfg.strategy_infos.len(), 62, "strategy : 62 strategy_infos");
    assert_eq!(cfg.condition_ids.len(), 182, "strategy : 182 condition_ids");
    assert_eq!(cfg.condition_ids2.len(), 74, "strategy : 74 condition_ids2");
    assert_eq!(cfg.phase_checks.len(), 60, "strategy : 60 phase_checks");
    assert_eq!(
        cfg.eval_adjust_params.len(),
        15,
        "strategy : 15 eval_adjust_params"
    );
    assert_eq!(cfg.eval_adjusts.len(), 14, "strategy : 14 eval_adjusts");
    assert_eq!(
        cfg.team_strategy_data.len(),
        56,
        "strategy : 56 team_strategy_data"
    );
    assert_eq!(
        cfg.team_strategy_infos.len(),
        5,
        "strategy : 5 team_strategy_infos"
    );
}

#[test]
fn real_file_strategy_tactics_info_0() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // ligne 9 : values[0] = {tacticsId: "0x906D3411", tacticsParam: 0}
    let t = &cfg.tactics_infos[0];
    assert_eq!(
        t.tactics_id, TACTICS_ID_0,
        "tactics_infos[0].tactics_id = 0x906D3411 (ligne 9)"
    );
    assert_eq!(
        t.tactics_param, 0,
        "tactics_infos[0].tactics_param = 0 (ligne 10)"
    );
}

#[test]
fn real_file_strategy_tactics_info_6() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // ligne 35 : values[6] = {tacticsId: "0x6F57E0D8", tacticsParam: 100}
    let t = &cfg.tactics_infos[6];
    assert_eq!(
        t.tactics_id, TACTICS_ID_6,
        "tactics_infos[6].tactics_id = 0x6F57E0D8 (ligne 35)"
    );
    assert_eq!(
        t.tactics_param, 100,
        "tactics_infos[6].tactics_param = 100 (ligne 36)"
    );
}

#[test]
fn real_file_strategy_info_0() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // ligne 279 : values[0].strategyId = "0x7C24BB03", isConfirm = false, tacticsInfo = [0, 2]
    let s = &cfg.strategy_infos[0];
    assert_eq!(
        s.strategy_id, STRATEGY_ID_0,
        "strategy_id[0] = 0x7C24BB03 (ligne 279)"
    );
    assert!(!s.is_confirm, "is_confirm[0] = false");
    assert_eq!(s.tactics_offset, 0, "tactics_offset[0] = 0");
    assert_eq!(s.tactics_count, 2, "tactics_count[0] = 2");
    assert!(s.evaluation_adjust_id.is_zero(), "eval_adj_id[0] = 0x0");
}

#[test]
fn real_file_strategy_info_1() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // ligne 291 : values[1].strategyId = "0xFAB0C9AD", isConfirm = true, evaluationAdjustId = "0xAD1047E1"
    let s = &cfg.strategy_infos[1];
    assert_eq!(
        s.strategy_id, STRATEGY_ID_1,
        "strategy_id[1] = 0xFAB0C9AD (ligne 291)"
    );
    assert!(s.is_confirm, "is_confirm[1] = true");
    assert_eq!(
        s.evaluation_adjust_id, EVAL_ADJ_ID,
        "eval_adj_id[1] = 0xAD1047E1"
    );
    assert_eq!(s.tactics_offset, 2, "tactics_offset[1] = 2");
}

#[test]
fn real_file_strategy_phase_check_0() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // values[0] = {phaseId: "0x3E7D8B80", phaseStateId: 0, condition: ""}
    let p = &cfg.phase_checks[0];
    assert_eq!(p.phase_id, PHASE_ID_0, "phase_id[0] = 0x3E7D8B80");
    assert_eq!(p.phase_state_id, 0, "phase_state_id[0] = 0");
    assert!(p.condition.is_empty(), "condition[0] = vide");
    assert_eq!(p.cond_id_list_count, 0, "cond_id_list_count[0] = 0");
}

#[test]
fn real_file_strategy_eval_adj_info_0() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // m_EvaluationAdjustInfoList[0] = {evaluationAdustId: "0xAD1047E1", adjustInfo: [0, 2]}
    let a = &cfg.eval_adjusts[0];
    assert_eq!(
        a.evaluation_adjust_id, EVAL_ADJ_ID,
        "eval_adj[0].id = 0xAD1047E1"
    );
    assert_eq!(a.adjust_offset, 0, "eval_adj[0].adjust_offset = 0");
    assert_eq!(a.adjust_count, 2, "eval_adj[0].adjust_count = 2");
}

#[test]
fn real_file_strategy_eval_adj_param_0() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // m_EvaluationAdjustParamInfoList[0] = {adjustType: 1, param: 0}
    // m_EvaluationAdjustParamInfoList[2] = {adjustType: 0, param: 5}
    assert_eq!(
        cfg.eval_adjust_params[0].adjust_type, 1,
        "eval_param[0].adjust_type = 1"
    );
    assert_eq!(
        cfg.eval_adjust_params[0].param, 0,
        "eval_param[0].param = 0"
    );
    assert_eq!(
        cfg.eval_adjust_params[2].adjust_type, 0,
        "eval_param[2].adjust_type = 0"
    );
    assert_eq!(
        cfg.eval_adjust_params[2].param, 5,
        "eval_param[2].param = 5"
    );
}

#[test]
fn real_file_strategy_team_group_0() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // ligne 3311 : values[0] = {groupId: "0x9714544E", strategyInfo: [0, 3]}
    let g = &cfg.team_strategy_infos[0];
    assert_eq!(
        g.group_id, TEAM_GROUP_ID_0,
        "team_group[0].group_id = 0x9714544E (ligne 3311)"
    );
    assert_eq!(g.strategy_offset, 0, "team_group[0].strategy_offset = 0");
    assert_eq!(g.strategy_count, 3, "team_group[0].strategy_count = 3");
}

#[test]
fn real_file_strategy_team_group_4() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // values[4] = {groupId: "0x38CB4D40", strategyInfo: [43, 13]}
    let g = &cfg.team_strategy_infos[4];
    assert_eq!(
        g.group_id, TEAM_GROUP_ID_4,
        "team_group[4].group_id = 0x38CB4D40"
    );
    assert_eq!(g.strategy_offset, 43, "team_group[4].strategy_offset = 43");
    assert_eq!(g.strategy_count, 13, "team_group[4].strategy_count = 13");
}

#[test]
fn real_file_strategy_team_data_0() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // m_TeamStrategyAIDataInfoList[0] = {strategyId: "0x7C24BB03", enableLv: 1}
    let d = &cfg.team_strategy_data[0];
    assert_eq!(
        d.strategy_id, STRATEGY_ID_0,
        "team_data[0].strategy_id = 0x7C24BB03"
    );
    assert_eq!(d.enable_lv, 1, "team_data[0].enable_lv = 1");
}

#[test]
fn real_file_strategy_condition_ids_comptes() {
    let Some(root) = load_json(REAL_STRATEGY) else {
        return;
    };
    let cfg = parse_strategy_ai_config(&root);
    // m_ConditionIdList : 182 ; m_ConditionIdList2 : 74
    assert_eq!(
        cfg.condition_ids[0].condition_id, 1102,
        "cond_ids[0].condition_id = 1102"
    );
    assert_eq!(
        cfg.condition_ids2[0].condition_id, 1301,
        "cond_ids2[0].condition_id = 1301"
    );
}

// ─── tactics_ai_config ────────────────────────────────────────────────────────

#[test]
fn real_file_tactics_comptes() {
    let Some(root) = load_json(REAL_TACTICS) else {
        return;
    };
    let cfg = parse_tactics_ai_config(&root);
    assert_eq!(cfg.target_infos.len(), 52, "tactics : 52 target_infos");
    assert_eq!(cfg.action_infos.len(), 60, "tactics : 60 action_infos");
    assert_eq!(cfg.tactics_infos.len(), 60, "tactics : 60 tactics_infos");
    assert_eq!(
        cfg.chara_tactics_data.len(),
        30,
        "tactics : 30 chara_tactics_data"
    );
    assert_eq!(
        cfg.chara_tactics_infos.len(),
        5,
        "tactics : 5 chara_tactics_infos"
    );
}

#[test]
fn real_file_tactics_target_0() {
    let Some(root) = load_json(REAL_TACTICS) else {
        return;
    };
    let cfg = parse_tactics_ai_config(&root);
    // ligne 9 : values[0] = {targetId: "0x1C44B74C", funcName: "CalcTargetChara_None"}
    let t = &cfg.target_infos[0];
    assert_eq!(
        t.target_id, TARGET_ID_0,
        "target_id[0] = 0x1C44B74C (ligne 9)"
    );
    assert_eq!(
        t.func_name, "CalcTargetChara_None",
        "func_name[0] = CalcTargetChara_None"
    );
}

#[test]
fn real_file_tactics_target_1() {
    let Some(root) = load_json(REAL_TACTICS) else {
        return;
    };
    let cfg = parse_tactics_ai_config(&root);
    // ligne 13 : values[1] = {targetId: "0x6B4387DA", funcName: "CalcTargetChara_Shoot"}
    let t = &cfg.target_infos[1];
    assert_eq!(
        t.target_id, TARGET_ID_1,
        "target_id[1] = 0x6B4387DA (ligne 13)"
    );
    assert_eq!(
        t.func_name, "CalcTargetChara_Shoot",
        "func_name[1] = CalcTargetChara_Shoot"
    );
}

#[test]
fn real_file_tactics_target_11() {
    let Some(root) = load_json(REAL_TACTICS) else {
        return;
    };
    let cfg = parse_tactics_ai_config(&root);
    // values[11] = {targetId: "0xA4F8D029", funcName: "CalcTargetPos_None"}
    let t = &cfg.target_infos[11];
    assert_eq!(t.target_id, TARGET_ID_11, "target_id[11] = 0xA4F8D029");
    assert_eq!(
        t.func_name, "CalcTargetPos_None",
        "func_name[11] = CalcTargetPos_None"
    );
}

#[test]
fn real_file_tactics_action_0() {
    let Some(root) = load_json(REAL_TACTICS) else {
        return;
    };
    let cfg = parse_tactics_ai_config(&root);
    // m_TacticsAIActionInfoList[0] = {actionId: 1, targetCharaType: 0, targetPosType: 10, targetId: "0xBDE3E168"}
    let a = &cfg.action_infos[0];
    assert_eq!(a.action_id, 1, "action[0].action_id = 1");
    assert_eq!(a.target_pos_type, 10, "action[0].target_pos_type = 10");
    assert_eq!(
        a.target_id, ACTION_TARGET_ID_0,
        "action[0].target_id = 0xBDE3E168"
    );
}

#[test]
fn real_file_tactics_info_0() {
    let Some(root) = load_json(REAL_TACTICS) else {
        return;
    };
    let cfg = parse_tactics_ai_config(&root);
    // m_TacticsAIInfoList[0] = {tacticsId: "0xD23CF887", actionInfo: [0, 1]}
    let t = &cfg.tactics_infos[0];
    assert_eq!(
        t.tactics_id, TACTICS_INFO_ID_0,
        "tactics_info[0].tactics_id = 0xD23CF887"
    );
    assert_eq!(t.action_offset, 0, "tactics_info[0].action_offset = 0");
    assert_eq!(t.action_count, 1, "tactics_info[0].action_count = 1");
}

#[test]
fn real_file_tactics_chara_data_0() {
    let Some(root) = load_json(REAL_TACTICS) else {
        return;
    };
    let cfg = parse_tactics_ai_config(&root);
    // m_CharaTacticsAIDataInfoList[0] = {tacticsId: "0x906D3411", enableLv: 1}
    let d = &cfg.chara_tactics_data[0];
    assert_eq!(
        d.tactics_id, CHARA_TACTICS_ID_0,
        "chara_data[0].tactics_id = 0x906D3411"
    );
    assert_eq!(d.enable_lv, 1, "chara_data[0].enable_lv = 1");
}

#[test]
fn real_file_tactics_chara_group_0() {
    let Some(root) = load_json(REAL_TACTICS) else {
        return;
    };
    let cfg = parse_tactics_ai_config(&root);
    // ligne 1261 : values[0] = {groupId: "0x35664547", tacticsInfo: [0, 2]}
    let g = &cfg.chara_tactics_infos[0];
    assert_eq!(
        g.group_id, CHARA_GROUP_ID_0,
        "chara_group[0].group_id = 0x35664547 (ligne 1261)"
    );
    assert_eq!(g.tactics_offset, 0, "chara_group[0].tactics_offset = 0");
    assert_eq!(g.tactics_count, 2, "chara_group[0].tactics_count = 2");
}

#[test]
fn real_file_tactics_chara_group_4() {
    let Some(root) = load_json(REAL_TACTICS) else {
        return;
    };
    let cfg = parse_tactics_ai_config(&root);
    // values[4] = {groupId: "0xDA9C897B", tacticsInfo: [28, 2]}
    let g = &cfg.chara_tactics_infos[4];
    assert_eq!(
        g.group_id, CHARA_GROUP_ID_4,
        "chara_group[4].group_id = 0xDA9C897B"
    );
    assert_eq!(g.tactics_offset, 28, "chara_group[4].tactics_offset = 28");
    assert_eq!(g.tactics_count, 2, "chara_group[4].tactics_count = 2");
}

#[test]
fn real_file_tactics_func_noms_tous_non_vides() {
    let Some(root) = load_json(REAL_TACTICS) else {
        return;
    };
    let cfg = parse_tactics_ai_config(&root);
    // Tous les 52 targets doivent avoir un funcName non vide
    let vides = cfg
        .target_infos
        .iter()
        .filter(|t| t.func_name.is_empty())
        .count();
    assert_eq!(vides, 0, "aucun funcName vide parmi les 52 cibles réelles");
}

/// Vérifie que les fichiers réels sont bien présents pour valider les tests real_file_*.
/// Si ce test est vert, les autres real_file_* se sont bien exécutés (pas filtrés).
#[test]
fn real_files_existence() {
    // Ce test doit résoudre le corpus comme les `real_file_*` qu'il garde — par `common::chemin`,
    // et non par un chemin relatif au répertoire courant : sinon le gardien dort pendant que les
    // gardés s'exécutent, et un dump partiel passe inaperçu.
    let Some(dir) = common::chemin(DIR) else {
        return;
    };
    if !dir.exists() {
        eprintln!(
            "skip real_files_existence : dumps absents ({})",
            dir.display()
        );
        return;
    }
    let paths = [
        REAL_SOCCER_CMD_V0,
        REAL_SOCCER_CMD_V5,
        REAL_SOCCER_USER,
        REAL_STRATEGY,
        REAL_TACTICS,
    ];
    for p in paths {
        let chemin = common::chemin(p).expect("racine du corpus déjà résolue");
        assert!(
            chemin.exists(),
            "Fichier réel manquant : {}",
            chemin.display()
        );
    }
}
