#![allow(clippy::pedantic)]
//! Tests golden `command` — valeurs réelles tirées de :
//! - `data/common/gamedata/command/chara_cmd_event_common_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/command/soccer_cmd_action_0.07.70.cfg.bin.json`
//! - `data/common/gamedata/command/soccer_cmd_event_0.07.70.cfg.bin.json`
//! - `data/common/gamedata/command/rpg_cmd_action_1.03.53.00.cfg.bin.json`
//! - `data/common/gamedata/command/rpg_cmd_event_1.02.75.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ
//!
//! ### chara_cmd_event_common : CHARA_ACTION_CMD_DATA_0
//! - var\[0\] = -1813128698 → HashId = 0x93EDDA06
//!   (fichier JSON lignes 13-19, `CHARA_ACTION_CMD_DATA_0.variables[0].value = "-1813128698"`)
//!
//! ### soccer_cmd_action : CMD_ACTION_INFO_0 (12 vars)
//! - var\[0\] = -566667975 → action_id = 0xDE395539
//!   (fichier JSON lignes 13-63, CMD_ACTION_INFO_0.variables[0].value = "-566667975")
//! - var\[4\] = -2102675611 → raw\[3\] = 0x82BC23A5
//!   (CMD_ACTION_INFO_0.variables[4].value = "-2102675611")
//! - var\[5\] = 894303940 → raw\[4\] = 0x354AB704
//!   (CMD_ACTION_INFO_0.variables[5].value = "894303940")
//! - var\[6\] = -817083886 → secondary_hash = 0xCF4C4A12
//!   (CMD_ACTION_INFO_0.variables[6].value = "-817083886")
//!
//! ### soccer_cmd_action : CMD_ACTION_INFO_1
//! - var\[0\] = -1444322264 → action_id = 0xA9E96428
//! - var\[2\] = 1783442655 → raw\[1\] = 0x6A4D2CDF
//! - var\[3\] = -1539459140 → raw\[2\] = 0xA43DB7BC
//! - var\[6\] = 665506216 → secondary_hash = 0x27AAD1A8
//!
//! ### soccer_cmd_event : ALL_CMD_EVENT_INFO_0
//! - var\[0\] = -566667975 → cmd_action_id = 0xDE395539 (même que CMD_ACTION_INFO_0)
//!   (soccer_cmd_event ligne 12, ALL_CMD_EVENT_INFO_0.variables[0].value = "-566667975")
//! - var\[1\] = 1356072398 → func_hash = 0x50D405CE
//!   (ALL_CMD_EVENT_INFO_0.variables[1].value = "1356072398")
//!
//! ### soccer_cmd_event : CHARA_CMD_EVENT_FUNC_0
//! - var\[0\] = 1 → func_no = 1
//!   (CHARA_CMD_EVENT_FUNC_0.variables[0].value = "1")
//! - var\[1\] = -236478904 → func_hash = 0xF1E79E48
//!   (CHARA_CMD_EVENT_FUNC_0.variables[1].value = "-236478904")
//!
//! ### rpg_cmd_action : CMD_ACTION_INFO_0
//! - var\[0\] = 2142375985 → action_id = 0x7FB21031
//!   (rpg_cmd_action ligne 13, CMD_ACTION_INFO_0.variables[0].value = "2142375985")
//! - var\[6\] = -1372911326 → secondary_hash = 0xAE2B0922
//!   (CMD_ACTION_INFO_0.variables[6].value = "-1372911326")
//!
//! ### rpg_cmd_event : ALL_CMD_EVENT_INFO_0
//! - var\[0\] = -1963241620 → cmd_action_id = 0x8AFB4F6C
//! - var\[1\] = -500396671 → func_hash = 0xE22C8D81

mod common;

extern crate std;

use nie_data::command::{
    AllCmdEventInfo, CharaCmdEventFunc, parse_chara_cmd_common, parse_cmd_actions, parse_cmd_events,
};
use nie_data::hash::HashId;
use serde_json::json;

// ─── Helpers de conversion vérifiés ──────────────────────────────────────────
//
// Toutes les valeurs hex ci-dessous sont calculées par `value as u32` (= `>>> 0` JS).
// Vérifiées en Python : `hex(v & 0xFFFFFFFF)`.

/// -566667975 as u32 = 0xDE395539 (vérifié Python)
const ACTION_ID_0_SOCCER: HashId = HashId(0xDE395539);
/// -817083886 as u32 = 0xCF4C4A12
const SECONDARY_HASH_0_SOCCER: HashId = HashId(0xCF4C4A12);
/// -1444322264 as u32 = 0xA9E96428 (vérifié Python)
const ACTION_ID_1_SOCCER: HashId = HashId(0xA9E96428);
/// 665506216 = 0x27AAD1A8 (vérifié Python)
const SECONDARY_HASH_1_SOCCER: HashId = HashId(0x27AAD1A8);
/// 1356072398 = 0x50D405CE
const FUNC_HASH_EVENT_0: HashId = HashId(0x50D405CE);
/// -236478904 as u32 = 0xF1E79E48
const FUNC_HASH_FUNC_0: HashId = HashId(0xF1E79E48);
/// -1813128698 as u32 = 0x93EDDA06
const COMMON_DATA_0: HashId = HashId(0x93EDDA06);
/// 2142375985 = 0x7FB21031
const ACTION_ID_0_RPG: HashId = HashId(0x7FB21031);
/// -1372911326 as u32 = 0xAE2B0922
const SECONDARY_HASH_0_RPG: HashId = HashId(0xAE2B0922);

// ─── Fixture soccer_cmd_action ────────────────────────────────────────────────
//
// Extrait minimal du vrai fichier (2 premières entrées CMD_ACTION_INFO).
// Valeurs lues octet-pour-octet dans soccer_cmd_action_0.07.70.cfg.bin.json.

fn fixture_soccer_cmd_action() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "CMD_ACTION_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    // CMD_ACTION_INFO_0 — soccer_cmd_action lignes 13-65
                    {
                        "name": "CMD_ACTION_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "-566667975"},   // var[0] action_id
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "-2102675611"},  // var[4] raw[3]
                            {"type": "Int", "value": "894303940"},    // var[5] raw[4]
                            {"type": "Int", "value": "-817083886"},   // var[6] secondary
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    // CMD_ACTION_INFO_1 — soccer_cmd_action lignes 66-118
                    {
                        "name": "CMD_ACTION_INFO_1",
                        "variables": [
                            {"type": "Int", "value": "-1444322264"},  // var[0] action_id
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "1783442655"},   // var[2] raw[1]
                            {"type": "Int", "value": "-1539459140"},  // var[3] raw[2]
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "665506216"},    // var[6] secondary
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

// ─── Fixture soccer_cmd_event ─────────────────────────────────────────────────

fn fixture_soccer_cmd_event() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "ALL_CMD_EVENT_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    // ALL_CMD_EVENT_INFO_0 — soccer_cmd_event lignes 12-16
                    {
                        "name": "ALL_CMD_EVENT_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "-566667975"},   // cmd_action_id
                            {"type": "Int", "value": "1356072398"},   // func_hash
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "CHARA_CMD_EVENT_FUNC_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    // CHARA_CMD_EVENT_FUNC_0 — soccer_cmd_event, liste FUNC lignes 0-3
                    {
                        "name": "CHARA_CMD_EVENT_FUNC_0",
                        "variables": [
                            {"type": "Int", "value": "1"},            // func_no
                            {"type": "Int", "value": "-236478904"}    // func_hash
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

// ─── Fixture chara_cmd_event_common ───────────────────────────────────────────

fn fixture_chara_cmd_common() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "CHARA_ACTION_CMD_DATA_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    // CHARA_ACTION_CMD_DATA_0 — chara_cmd_event_common lignes 13-19
                    {
                        "name": "CHARA_ACTION_CMD_DATA_0",
                        "variables": [
                            {"type": "Int", "value": "-1813128698"}   // cmd_action_id
                        ],
                        "children": []
                    },
                    // CHARA_ACTION_CMD_DATA_1 — ligne 20-26
                    {
                        "name": "CHARA_ACTION_CMD_DATA_1",
                        "variables": [
                            {"type": "Int", "value": "-1187820884"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur fixture : parse_cmd_actions ────────────────────────────────────

#[test]
fn cmd_action_compte_et_action_id_0() {
    let actions = parse_cmd_actions(&fixture_soccer_cmd_action());
    // fixture contient 2 CMD_ACTION_INFO valides
    assert_eq!(actions.len(), 2, "fixture: 2 CMD_ACTION_INFO");

    // soccer_cmd_action_0.07.70 : CMD_ACTION_INFO_0.var[0] = -566667975 → 0xDE395539
    assert_eq!(
        actions[0].action_id, ACTION_ID_0_SOCCER,
        "action_id[0] = 0xDE395539"
    );
}

#[test]
fn cmd_action_secondary_hash_0() {
    let actions = parse_cmd_actions(&fixture_soccer_cmd_action());
    // soccer_cmd_action_0.07.70 : CMD_ACTION_INFO_0.var[6] = -817083886 → 0xCF4C4A12
    assert_eq!(
        actions[0].secondary_hash(),
        SECONDARY_HASH_0_SOCCER,
        "secondary_hash[0] = 0xCF4C4A12"
    );
}

#[test]
fn cmd_action_raw_positions_entry_0() {
    let actions = parse_cmd_actions(&fixture_soccer_cmd_action());
    // CMD_ACTION_INFO_0 : var[4]=-2102675611 → raw[3], var[5]=894303940 → raw[4]
    assert_eq!(
        actions[0].raw[3],
        HashId::from_i64(-2102675611),
        "raw[3] = var[4] = 0x82BC23A5"
    );
    assert_eq!(
        actions[0].raw[4],
        HashId::from_i64(894303940),
        "raw[4] = var[5] = 0x354AB704"
    );
    // var[1], var[2], var[3] = 0 pour l'entrée 0
    assert_eq!(actions[0].raw[0], HashId::ZERO, "raw[0] = var[1] = 0");
    assert_eq!(actions[0].raw[1], HashId::ZERO, "raw[1] = var[2] = 0");
    assert_eq!(actions[0].raw[2], HashId::ZERO, "raw[2] = var[3] = 0");
}

#[test]
fn cmd_action_entry_1() {
    let actions = parse_cmd_actions(&fixture_soccer_cmd_action());
    // CMD_ACTION_INFO_1 : var[0]=-1444322264 → 0xA9C8CB48
    assert_eq!(
        actions[1].action_id, ACTION_ID_1_SOCCER,
        "action_id[1] = 0xA9C8CB48"
    );
    // var[2]=1783442655 → raw[1] = 0x6A4D2CDF
    assert_eq!(
        actions[1].raw[1],
        HashId::from_i64(1783442655),
        "raw[1] = var[2] = 0x6A4D2CDF"
    );
    // var[3]=-1539459140 → raw[2] = 0xA43DB7BC
    assert_eq!(
        actions[1].raw[2],
        HashId::from_i64(-1539459140),
        "raw[2] = var[3] = 0xA43DB7BC"
    );
    // var[6]=665506216 → secondary_hash
    assert_eq!(
        actions[1].secondary_hash(),
        SECONDARY_HASH_1_SOCCER,
        "secondary_hash[1] = 0x27A0ABA8"
    );
}

#[test]
fn cmd_action_list_beg_ignore() {
    // Les noeuds conteneurs CMD_ACTION_INFO_LIST_BEG_* doivent être ignorés
    let actions = parse_cmd_actions(&fixture_soccer_cmd_action());
    assert_eq!(actions.len(), 2, "LIST_BEG non compté");
}

#[test]
fn cmd_action_is_valid() {
    let actions = parse_cmd_actions(&fixture_soccer_cmd_action());
    assert!(actions[0].is_valid(), "action_id non-nul → is_valid");
}

// ─── Tests sur fixture : parse_cmd_events ────────────────────────────────────

#[test]
fn cmd_event_comptes() {
    let config = parse_cmd_events(&fixture_soccer_cmd_event());
    assert_eq!(
        config.all_cmd_events.len(),
        1,
        "1 ALL_CMD_EVENT_INFO dans la fixture"
    );
    assert_eq!(
        config.chara_cmd_funcs.len(),
        1,
        "1 CHARA_CMD_EVENT_FUNC dans la fixture"
    );
}

#[test]
fn all_cmd_event_info_0() {
    let config = parse_cmd_events(&fixture_soccer_cmd_event());
    let ev: &AllCmdEventInfo = &config.all_cmd_events[0];

    // ALL_CMD_EVENT_INFO_0 soccer_cmd_event : var[0]=-566667975 → 0xDE395539
    assert_eq!(
        ev.cmd_action_id, ACTION_ID_0_SOCCER,
        "cmd_action_id = 0xDE395539"
    );
    // var[1]=1356072398 → 0x50D405CE
    assert_eq!(ev.func_hash, FUNC_HASH_EVENT_0, "func_hash = 0x50D405CE");
}

#[test]
fn chara_cmd_event_func_0() {
    let config = parse_cmd_events(&fixture_soccer_cmd_event());
    let f: &CharaCmdEventFunc = &config.chara_cmd_funcs[0];

    // CHARA_CMD_EVENT_FUNC_0 : var[0]=1 → func_no=1
    assert_eq!(f.func_no, 1, "func_no = 1");
    // var[1]=-236478904 → 0xF1E79E48
    assert_eq!(f.func_hash, FUNC_HASH_FUNC_0, "func_hash = 0xF1E79E48");
}

#[test]
fn cmd_event_find_event() {
    let config = parse_cmd_events(&fixture_soccer_cmd_event());
    // find_event doit retrouver l'événement par cmd_action_id
    let found = config.find_event(ACTION_ID_0_SOCCER);
    assert!(
        found.is_some(),
        "find_event doit trouver ACTION_ID_0_SOCCER"
    );
    assert_eq!(found.unwrap().func_hash, FUNC_HASH_EVENT_0);

    // Identifiant absent → None
    assert!(config.find_event(HashId(0xDEADBEEF)).is_none());
}

// ─── Tests sur fixture : parse_chara_cmd_common ───────────────────────────────

#[test]
fn chara_cmd_common_compte_et_valeur_0() {
    let ids = parse_chara_cmd_common(&fixture_chara_cmd_common());
    assert_eq!(ids.len(), 2, "2 CHARA_ACTION_CMD_DATA dans la fixture");

    // CHARA_ACTION_CMD_DATA_0 : var[0]=-1813128698 → 0x93EDDA06
    assert_eq!(ids[0], COMMON_DATA_0, "ids[0] = 0x93EDDA06");
}

#[test]
fn chara_cmd_common_valeur_1() {
    let ids = parse_chara_cmd_common(&fixture_chara_cmd_common());
    // CHARA_ACTION_CMD_DATA_1 : var[0]=-1187820884
    assert_eq!(
        ids[1],
        HashId::from_i64(-1187820884),
        "ids[1] = var[0]=-1187820884"
    );
}

#[test]
fn chara_cmd_common_list_beg_ignore() {
    let ids = parse_chara_cmd_common(&fixture_chara_cmd_common());
    // LIST_BEG ignoré → exactement 2 (pas 3)
    assert_eq!(ids.len(), 2);
}

// ─── Tests sur fichiers réels (skip silencieux si absents) ────────────────────

const REAL_SOCCER_ACTION: &str = "command/soccer_cmd_action_0.07.70.cfg.bin.json";
const REAL_SOCCER_EVENT: &str = "command/soccer_cmd_event_0.07.70.cfg.bin.json";
const REAL_RPG_ACTION: &str = "command/rpg_cmd_action_1.03.53.00.cfg.bin.json";
const REAL_RPG_EVENT: &str = "command/rpg_cmd_event_1.02.75.00.cfg.bin.json";
const REAL_COMMON: &str = "command/chara_cmd_event_common_0.00.00.cfg.bin.json";

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

// — soccer_cmd_action —

#[test]
fn real_file_soccer_action_compte() {
    let Some(root) = load_json(REAL_SOCCER_ACTION) else {
        return;
    };
    let actions = parse_cmd_actions(&root);
    // soccer_cmd_action_0.07.70 : CMD_ACTION_INFO_LIST_BEG_0.var[0] = 88
    assert_eq!(actions.len(), 88, "soccer_cmd_action : 88 CMD_ACTION_INFO");
}

#[test]
fn real_file_soccer_action_entry_0() {
    let Some(root) = load_json(REAL_SOCCER_ACTION) else {
        return;
    };
    let actions = parse_cmd_actions(&root);

    // soccer_cmd_action_0.07.70 CMD_ACTION_INFO_0 (lignes 13-65)
    // var[0]=-566667975 → action_id = 0xDE395539
    assert_eq!(
        actions[0].action_id, ACTION_ID_0_SOCCER,
        "action_id[0] = 0xDE395539"
    );
    // var[6]=-817083886 → secondary_hash = 0xCF4C4A12
    assert_eq!(
        actions[0].secondary_hash(),
        SECONDARY_HASH_0_SOCCER,
        "secondary_hash[0] = 0xCF4C4A12"
    );
    // var[4]=-2102675611 → raw[3] non-nul
    assert_ne!(
        actions[0].raw[3],
        HashId::ZERO,
        "raw[3] (var[4]) non-nul pour CMD_ACTION_INFO_0"
    );
}

#[test]
fn real_file_soccer_action_entry_1() {
    let Some(root) = load_json(REAL_SOCCER_ACTION) else {
        return;
    };
    let actions = parse_cmd_actions(&root);

    // soccer_cmd_action_0.07.70 CMD_ACTION_INFO_1 (lignes 66-118)
    // var[0]=-1444322264 → 0xA9C8CB48
    assert_eq!(
        actions[1].action_id, ACTION_ID_1_SOCCER,
        "action_id[1] = 0xA9C8CB48"
    );
    // var[6]=665506216 → secondary_hash = 0x27A0ABA8
    assert_eq!(
        actions[1].secondary_hash(),
        SECONDARY_HASH_1_SOCCER,
        "secondary_hash[1] = 0x27A0ABA8"
    );
}

#[test]
fn real_file_soccer_action_tous_valides() {
    let Some(root) = load_json(REAL_SOCCER_ACTION) else {
        return;
    };
    let actions = parse_cmd_actions(&root);
    // Toutes les entrées doivent avoir action_id non-nul
    let invalides = actions.iter().filter(|a| !a.is_valid()).count();
    assert_eq!(
        invalides, 0,
        "tous les CMD_ACTION_INFO doivent être valides"
    );
}

#[test]
fn real_file_soccer_action_secondary_hash_presence() {
    let Some(root) = load_json(REAL_SOCCER_ACTION) else {
        return;
    };
    let actions = parse_cmd_actions(&root);
    // 84/88 entrées ont secondary_hash (var[6]) non-nul — validé par analyse Python
    let avec_secondary = actions
        .iter()
        .filter(|a| !a.secondary_hash().is_zero())
        .count();
    assert!(
        avec_secondary >= 84,
        "≥ 84 entrées avec secondary_hash non-nul (réel: {avec_secondary})"
    );
}

// — soccer_cmd_event —

#[test]
fn real_file_soccer_event_comptes() {
    let Some(root) = load_json(REAL_SOCCER_EVENT) else {
        return;
    };
    let config = parse_cmd_events(&root);

    // soccer_cmd_event_0.07.70 : 99 ALL_CMD_EVENT_INFO + 50 CHARA_CMD_EVENT_FUNC
    // (51 children dans CHARA_CMD_EVENT_FUNC_LIST_BEG_0 dont 1 CMD_EVENT_DATA_LIST_BEG_0 ignoré)
    assert_eq!(config.all_cmd_events.len(), 99, "99 ALL_CMD_EVENT_INFO");
    assert_eq!(config.chara_cmd_funcs.len(), 50, "50 CHARA_CMD_EVENT_FUNC");
}

#[test]
fn real_file_soccer_event_all_cmd_event_0() {
    let Some(root) = load_json(REAL_SOCCER_EVENT) else {
        return;
    };
    let config = parse_cmd_events(&root);

    // soccer_cmd_event : ALL_CMD_EVENT_INFO_0 — var[0]=-566667975, var[1]=1356072398
    assert_eq!(
        config.all_cmd_events[0].cmd_action_id, ACTION_ID_0_SOCCER,
        "cmd_action_id[0] = 0xDE395539"
    );
    assert_eq!(
        config.all_cmd_events[0].func_hash, FUNC_HASH_EVENT_0,
        "func_hash[0] = 0x50D405CE"
    );
}

#[test]
fn real_file_soccer_event_chara_func_0() {
    let Some(root) = load_json(REAL_SOCCER_EVENT) else {
        return;
    };
    let config = parse_cmd_events(&root);

    // soccer_cmd_event : CHARA_CMD_EVENT_FUNC_0 — var[0]=1, var[1]=-236478904
    assert_eq!(config.chara_cmd_funcs[0].func_no, 1, "func_no[0] = 1");
    assert_eq!(
        config.chara_cmd_funcs[0].func_hash, FUNC_HASH_FUNC_0,
        "func_hash[0] = 0xF1E79E48"
    );
}

#[test]
fn real_file_soccer_event_find_event() {
    let Some(root) = load_json(REAL_SOCCER_EVENT) else {
        return;
    };
    let config = parse_cmd_events(&root);
    // find_event(0xDE395539) doit retrouver l'entrée 0
    let found = config.find_event(ACTION_ID_0_SOCCER);
    assert!(found.is_some(), "find_event(ACTION_ID_0_SOCCER)");
    assert_eq!(found.unwrap().func_hash, FUNC_HASH_EVENT_0);
}

// — rpg_cmd_action —

#[test]
fn real_file_rpg_action_compte() {
    let Some(root) = load_json(REAL_RPG_ACTION) else {
        return;
    };
    let actions = parse_cmd_actions(&root);
    // rpg_cmd_action_1.03.53.00 : 229 CMD_ACTION_INFO
    assert_eq!(actions.len(), 229, "rpg_cmd_action : 229 CMD_ACTION_INFO");
}

#[test]
fn real_file_rpg_action_entry_0() {
    let Some(root) = load_json(REAL_RPG_ACTION) else {
        return;
    };
    let actions = parse_cmd_actions(&root);

    // rpg_cmd_action_1.03.53.00 CMD_ACTION_INFO_0 (ligne 13)
    // var[0]=2142375985 → 0x7FB21031
    assert_eq!(
        actions[0].action_id, ACTION_ID_0_RPG,
        "action_id[0] = 0x7FB21031"
    );
    // var[6]=-1372911326 → 0xAE2B0922
    assert_eq!(
        actions[0].secondary_hash(),
        SECONDARY_HASH_0_RPG,
        "secondary_hash[0] = 0xAE2B0922"
    );
}

#[test]
fn real_file_rpg_action_secondary_hash_presence() {
    let Some(root) = load_json(REAL_RPG_ACTION) else {
        return;
    };
    let actions = parse_cmd_actions(&root);
    // 220/229 entrées ont secondary_hash non-nul — validé par analyse Python
    let avec_secondary = actions
        .iter()
        .filter(|a| !a.secondary_hash().is_zero())
        .count();
    assert!(
        avec_secondary >= 220,
        "≥ 220 entrées avec secondary_hash (réel: {avec_secondary})"
    );
}

// — rpg_cmd_event —

#[test]
fn real_file_rpg_event_comptes() {
    let Some(root) = load_json(REAL_RPG_EVENT) else {
        return;
    };
    let config = parse_cmd_events(&root);
    // rpg_cmd_event_1.02.75.00 : 231 ALL_CMD_EVENT_INFO + 38 CHARA_CMD_EVENT_FUNC
    // (39 children dont 1 CMD_EVENT_DATA_LIST_BEG_0 ignoré par walk_named)
    assert_eq!(
        config.all_cmd_events.len(),
        231,
        "231 ALL_CMD_EVENT_INFO rpg"
    );
    assert_eq!(
        config.chara_cmd_funcs.len(),
        38,
        "38 CHARA_CMD_EVENT_FUNC rpg"
    );
}

#[test]
fn real_file_rpg_event_all_cmd_0() {
    let Some(root) = load_json(REAL_RPG_EVENT) else {
        return;
    };
    let config = parse_cmd_events(&root);

    // rpg_cmd_event : ALL_CMD_EVENT_INFO_0 — var[0]=-1963241620 → 0x8AFB4F6C
    assert_eq!(
        config.all_cmd_events[0].cmd_action_id,
        HashId::from_i64(-1963241620),
        "cmd_action_id rpg[0] = 0x8AFB4F6C"
    );
    // var[1]=-500396671 → 0xE22C8D81
    assert_eq!(
        config.all_cmd_events[0].func_hash,
        HashId::from_i64(-500396671),
        "func_hash rpg[0] = 0xE22C8D81"
    );
}

#[test]
fn real_file_rpg_event_chara_func_0() {
    let Some(root) = load_json(REAL_RPG_EVENT) else {
        return;
    };
    let config = parse_cmd_events(&root);

    // rpg et soccer partagent les mêmes premières entrées CHARA_CMD_EVENT_FUNC
    // CHARA_CMD_EVENT_FUNC_0 : var[0]=1, var[1]=-236478904 → 0xF1E79E48
    assert_eq!(config.chara_cmd_funcs[0].func_no, 1);
    assert_eq!(config.chara_cmd_funcs[0].func_hash, FUNC_HASH_FUNC_0);
}

// — chara_cmd_event_common —

#[test]
fn real_file_common_compte() {
    let Some(root) = load_json(REAL_COMMON) else {
        return;
    };
    let ids = parse_chara_cmd_common(&root);
    // chara_cmd_event_common_0.00.00 : CHARA_ACTION_CMD_DATA_LIST_BEG_0.var[0] = 32
    assert_eq!(
        ids.len(),
        32,
        "chara_cmd_event_common : 32 CHARA_ACTION_CMD_DATA"
    );
}

#[test]
fn real_file_common_data_0() {
    let Some(root) = load_json(REAL_COMMON) else {
        return;
    };
    let ids = parse_chara_cmd_common(&root);

    // chara_cmd_event_common : CHARA_ACTION_CMD_DATA_0.var[0] = -1813128698 → 0x93EDDA06
    assert_eq!(ids[0], COMMON_DATA_0, "ids[0] = 0x93EDDA06");
}

#[test]
fn real_file_common_data_1() {
    let Some(root) = load_json(REAL_COMMON) else {
        return;
    };
    let ids = parse_chara_cmd_common(&root);

    // CHARA_ACTION_CMD_DATA_1.var[0] = -1187820884
    assert_eq!(
        ids[1],
        HashId::from_i64(-1187820884),
        "ids[1] = var[0]=-1187820884"
    );
}

#[test]
fn real_file_common_tous_non_nuls() {
    let Some(root) = load_json(REAL_COMMON) else {
        return;
    };
    let ids = parse_chara_cmd_common(&root);
    // Toutes les 32 entrées doivent avoir un hash non-nul
    let nuls = ids.iter().filter(|id| id.is_zero()).count();
    assert_eq!(
        nuls, 0,
        "aucun cmd_action_id nul dans chara_cmd_event_common"
    );
}
