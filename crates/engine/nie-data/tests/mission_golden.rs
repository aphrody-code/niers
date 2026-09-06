#![allow(clippy::pedantic)]
//! Tests golden `mission` — valeurs réelles tirées de :
//! - `data/common/gamedata/mission/mission_config_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/mission/msa999999_trigger_0.04.78.cfg.bin.json`
//!
//! ## Vérifications champ par champ
//!
//! ### mission_config : MISSION_CONFIG_INFO_0 (16 vars)
//! - var\[0\] = 1013400427 → mission_id = 0x3C67436B
//!   (fichier JSON : MISSION_CONFIG_INFO_0.variables\[0\].value = "1013400427")
//! - var\[1\] = "msa999999" → mission_code
//!   (MISSION_CONFIG_INFO_0.variables\[1\].value = "msa999999", type String)
//! - var\[2\] = 1316720857 → raw\[0\] = 0x4E7B90D9
//!   (variables\[2\].value = "1316720857")
//! - var\[3\] = 1 → raw\[1\] = 0x00000001
//!   (variables\[3\].value = "1")
//! - var\[5\] = -736280260 → raw\[3\] = 0xD41D413C
//!   (variables\[5\].value = "-736280260")
//! - var\[9\] = 1475254969 → raw\[7\] = 0x57EE9AB9
//!   (variables\[9\].value = "1475254969")
//! - var\[10\] = -813411175 → raw\[8\] = 0xCF845499
//!   (variables\[10\].value = "-813411175")
//! - var\[11\] = 2083486252 → raw\[9\] = 0x7C2F7A2C
//!   (variables\[11\].value = "2083486252")
//! - var\[4\], var\[6..=8\], var\[12..=15\] = 0
//!
//! ### msa999999_trigger : DATA_ITEM_0 (7 vars)
//! - var\[0\] = 1 → trigger_type = 1
//!   (DATA_ITEM_0.variables\[0\].value = "1")
//! - var\[1\] = -1369358024 → trigger_hash = 0xAE614138
//!   (variables\[1\].value = "-1369358024")
//! - var\[2\] = 0 → flag2 = 0
//! - var\[3\] = "AAAAAAoDNbkZNtoAAQB4" → data (type String)
//!   (variables\[3\].value = "AAAAAAoDNbkZNtoAAQB4")
//! - var\[4\] = 1 → flag4 = 1
//! - var\[5\] = 0 → flag5 = 0
//! - var\[6\] = 1 → flag6 = 1

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::mission::{MissionConfigInfo, parse_mission_config, parse_mission_trigger};
use serde_json::json;

// ─── Helpers de conversion vérifiés ──────────────────────────────────────────
//
// Toutes les valeurs hex sont calculées par `value as u32` (= `>>> 0` JS).
// Vérifiées octet par octet ci-dessous.

/// 1013400427 as u32 = 0x3C67436B
const MISSION_ID_0: HashId = HashId(0x3C67_436B);
/// 1316720857 as u32 = 0x4E7B90D9
const RAW_0_VAR2: HashId = HashId(0x4E7B_90D9);
/// -736280260 as u32 = 0xD41D413C  (= 4294967296 - 736280260 = 3558687036)
const RAW_3_VAR5: HashId = HashId(0xD41D_413C);
/// 1475254969 as u32 = 0x57EE9AB9
const RAW_7_VAR9: HashId = HashId(0x57EE_9AB9);
/// -813411175 as u32 = 0xCF845499 (= 4294967296 - 813411175 = 3481556121)
const RAW_8_VAR10: HashId = HashId(0xCF84_5499);
/// 2083486252 as u32 = 0x7C2F7A2C
const RAW_9_VAR11: HashId = HashId(0x7C2F_7A2C);
/// -1369358024 as u32 = 0xAE614138 (= 4294967296 - 1369358024 = 2925609272)
const TRIGGER_HASH_0: HashId = HashId(0xAE61_4138);

// ─── Fixture mission_config ───────────────────────────────────────────────────
//
// Structure exacte du vrai fichier :
// MISSION_CONFIG_INFO_LIST_BEG_0 (conteneur, var[0]=1) → enfant MISSION_CONFIG_INFO_0

fn fixture_mission_config() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "MISSION_CONFIG_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    {
                        "name": "MISSION_CONFIG_INFO_0",
                        "variables": [
                            // var[0] mission_id : mission_config_0.00.00, variables[0]
                            {"type": "Int",    "value": "1013400427"},
                            // var[1] mission_code : variables[1]
                            {"type": "String", "value": "msa999999"},
                            // var[2] raw[0] : variables[2]
                            {"type": "Int",    "value": "1316720857"},
                            // var[3] raw[1] : variables[3]
                            {"type": "Int",    "value": "1"},
                            // var[4] raw[2] : variables[4]
                            {"type": "Int",    "value": "0"},
                            // var[5] raw[3] : variables[5]
                            {"type": "Int",    "value": "-736280260"},
                            // var[6] raw[4] : variables[6]
                            {"type": "Int",    "value": "0"},
                            // var[7] raw[5] : variables[7]
                            {"type": "Int",    "value": "0"},
                            // var[8] raw[6] : variables[8]
                            {"type": "Int",    "value": "0"},
                            // var[9] raw[7] : variables[9]
                            {"type": "Int",    "value": "1475254969"},
                            // var[10] raw[8] : variables[10]
                            {"type": "Int",    "value": "-813411175"},
                            // var[11] raw[9] : variables[11]
                            {"type": "Int",    "value": "2083486252"},
                            // var[12..=15] raw[10..=13] = 0 : variables[12..=15]
                            {"type": "Int",    "value": "0"},
                            {"type": "Int",    "value": "0"},
                            {"type": "Int",    "value": "0"},
                            {"type": "Int",    "value": "0"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

// ─── Fixture msa999999_trigger ────────────────────────────────────────────────
//
// Structure exacte : DATA_COUNT_0 et DATA_ITEM_0 sont des entrées de même niveau.

fn fixture_mission_trigger() -> serde_json::Value {
    json!({
        "entries": [
            // DATA_COUNT_0 : msa999999_trigger_0.04.78, entries[0]
            {
                "name": "DATA_COUNT_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": []
            },
            // DATA_ITEM_0 : entries[1]
            {
                "name": "DATA_ITEM_0",
                "variables": [
                    // var[0] trigger_type : variables[0]
                    {"type": "Int",    "value": "1"},
                    // var[1] trigger_hash : variables[1]
                    {"type": "Int",    "value": "-1369358024"},
                    // var[2] flag2 : variables[2]
                    {"type": "Int",    "value": "0"},
                    // var[3] data : variables[3]
                    {"type": "String", "value": "AAAAAAoDNbkZNtoAAQB4"},
                    // var[4] flag4 : variables[4]
                    {"type": "Int",    "value": "1"},
                    // var[5] flag5 : variables[5]
                    {"type": "Int",    "value": "0"},
                    // var[6] flag6 : variables[6]
                    {"type": "Int",    "value": "1"}
                ],
                "children": []
            }
        ]
    })
}

// ─── Tests sur fixture : parse_mission_config ─────────────────────────────────

#[test]
fn fixture_mission_config_compte() {
    let infos = parse_mission_config(&fixture_mission_config());
    // Fixture contient 1 MISSION_CONFIG_INFO_0 (enfant du conteneur)
    assert_eq!(infos.len(), 1, "fixture: 1 MISSION_CONFIG_INFO");
}

#[test]
fn fixture_mission_config_list_beg_ignore() {
    // Le noeud MISSION_CONFIG_INFO_LIST_BEG_0 ne doit pas être compté
    let infos = parse_mission_config(&fixture_mission_config());
    assert_eq!(infos.len(), 1, "LIST_BEG non compté");
}

#[test]
fn fixture_mission_config_mission_id() {
    let infos = parse_mission_config(&fixture_mission_config());
    let info: &MissionConfigInfo = &infos[0];

    // mission_config_0.00.00 : MISSION_CONFIG_INFO_0.variables[0].value = "1013400427"
    // → HashId::from_i64(1013400427) = HashId(0x3C67436B)
    assert_eq!(info.mission_id, MISSION_ID_0, "mission_id = 0x3C67436B");
}

#[test]
fn fixture_mission_config_mission_code() {
    let infos = parse_mission_config(&fixture_mission_config());
    let info: &MissionConfigInfo = &infos[0];

    // MISSION_CONFIG_INFO_0.variables[1] : type=String, value="msa999999"
    assert_eq!(
        info.mission_code, "msa999999",
        "mission_code = \"msa999999\""
    );
}

#[test]
fn fixture_mission_config_raw_valeurs() {
    let infos = parse_mission_config(&fixture_mission_config());
    let info: &MissionConfigInfo = &infos[0];

    // raw[0] = var[2] = 1316720857 → 0x4E7B90D9
    assert_eq!(info.raw[0], RAW_0_VAR2, "raw[0] = var[2] = 0x4E7B90D9");
    // raw[1] = var[3] = 1 → 0x00000001
    assert_eq!(
        info.raw[1],
        HashId::from_i64(1),
        "raw[1] = var[3] = 0x00000001"
    );
    // raw[2] = var[4] = 0 → ZERO
    assert_eq!(info.raw[2], HashId::ZERO, "raw[2] = var[4] = 0");
    // raw[3] = var[5] = -736280260 → 0xD41D413C
    assert_eq!(info.raw[3], RAW_3_VAR5, "raw[3] = var[5] = 0xD41D413C");
    // raw[4..=6] = var[6..=8] = 0
    assert_eq!(info.raw[4], HashId::ZERO, "raw[4] = var[6] = 0");
    assert_eq!(info.raw[5], HashId::ZERO, "raw[5] = var[7] = 0");
    assert_eq!(info.raw[6], HashId::ZERO, "raw[6] = var[8] = 0");
    // raw[7] = var[9] = 1475254969 → 0x57EE9AB9
    assert_eq!(info.raw[7], RAW_7_VAR9, "raw[7] = var[9] = 0x57EE9AB9");
    // raw[8] = var[10] = -813411175 → 0xCF845499
    assert_eq!(info.raw[8], RAW_8_VAR10, "raw[8] = var[10] = 0xCF845499");
    // raw[9] = var[11] = 2083486252 → 0x7C2F7A2C
    assert_eq!(info.raw[9], RAW_9_VAR11, "raw[9] = var[11] = 0x7C2F7A2C");
    // raw[10..=13] = var[12..=15] = 0
    assert_eq!(info.raw[10], HashId::ZERO, "raw[10] = var[12] = 0");
    assert_eq!(info.raw[13], HashId::ZERO, "raw[13] = var[15] = 0");
}

// ─── Tests sur fixture : parse_mission_trigger ────────────────────────────────

#[test]
fn fixture_trigger_compte() {
    let items = parse_mission_trigger(&fixture_mission_trigger());
    // Fixture contient 1 DATA_ITEM_0 (DATA_COUNT_0 ignoré)
    assert_eq!(items.len(), 1, "fixture: 1 DATA_ITEM");
}

#[test]
fn fixture_trigger_data_count_ignore() {
    // DATA_COUNT_0 ne doit pas être parsé comme un DATA_ITEM
    let items = parse_mission_trigger(&fixture_mission_trigger());
    assert_eq!(items.len(), 1, "DATA_COUNT non compté");
}

#[test]
fn fixture_trigger_item0_type_et_hash() {
    let items = parse_mission_trigger(&fixture_mission_trigger());
    let item = &items[0];

    // DATA_ITEM_0.variables[0].value = "1" → trigger_type = 1
    assert_eq!(item.trigger_type, 1, "trigger_type = 1");
    // DATA_ITEM_0.variables[1].value = "-1369358024" → 0xAE614138
    assert_eq!(
        item.trigger_hash, TRIGGER_HASH_0,
        "trigger_hash = 0xAE614138"
    );
}

#[test]
fn fixture_trigger_item0_data() {
    let items = parse_mission_trigger(&fixture_mission_trigger());
    let item = &items[0];

    // DATA_ITEM_0.variables[3] : type=String, value="AAAAAAoDNbkZNtoAAQB4"
    assert_eq!(
        item.data, "AAAAAAoDNbkZNtoAAQB4",
        "data = \"AAAAAAoDNbkZNtoAAQB4\""
    );
}

#[test]
fn fixture_trigger_item0_flags() {
    let items = parse_mission_trigger(&fixture_mission_trigger());
    let item = &items[0];

    // DATA_ITEM_0 : var[2]=0, var[4]=1, var[5]=0, var[6]=1
    assert_eq!(item.flag2, 0, "flag2 = 0");
    assert_eq!(item.flag4, 1, "flag4 = 1");
    assert_eq!(item.flag5, 0, "flag5 = 0");
    assert_eq!(item.flag6, 1, "flag6 = 1");
}

// ─── Tests sur fichiers réels (skip silencieux si absents) ────────────────────

const REAL_MISSION_CONFIG: &str = "mission/mission_config_0.00.00.cfg.bin.json";
const REAL_TRIGGER: &str = "mission/msa999999_trigger_0.04.78.cfg.bin.json";

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

// — mission_config —

#[test]
fn real_file_mission_config_compte() {
    let Some(root) = load_json(REAL_MISSION_CONFIG) else {
        return;
    };
    let infos = parse_mission_config(&root);
    // mission_config_0.00.00 : MISSION_CONFIG_INFO_LIST_BEG_0.variables[0].value = "1"
    assert_eq!(infos.len(), 1, "mission_config: 1 MISSION_CONFIG_INFO");
}

#[test]
fn real_file_mission_config_id() {
    let Some(root) = load_json(REAL_MISSION_CONFIG) else {
        return;
    };
    let infos = parse_mission_config(&root);

    // mission_config_0.00.00 : MISSION_CONFIG_INFO_0.variables[0].value = "1013400427"
    // → HashId::from_i64(1013400427) = HashId(0x3C67436B)
    assert_eq!(infos[0].mission_id, MISSION_ID_0, "mission_id = 0x3C67436B");
}

#[test]
fn real_file_mission_config_code() {
    let Some(root) = load_json(REAL_MISSION_CONFIG) else {
        return;
    };
    let infos = parse_mission_config(&root);

    // mission_config_0.00.00 : MISSION_CONFIG_INFO_0.variables[1].value = "msa999999"
    assert_eq!(
        infos[0].mission_code, "msa999999",
        "mission_code = \"msa999999\""
    );
}

#[test]
fn real_file_mission_config_raw_hashes_non_nuls() {
    let Some(root) = load_json(REAL_MISSION_CONFIG) else {
        return;
    };
    let infos = parse_mission_config(&root);
    let info = &infos[0];

    // raw[0] = var[2] = 1316720857 → 0x4E7B90D9 (non-nul)
    assert_eq!(
        info.raw[0],
        HashId::from_i64(1316720857),
        "raw[0] = var[2] = 0x4E7B90D9"
    );
    // raw[3] = var[5] = -736280260 → 0xD41D413C (non-nul)
    assert_eq!(
        info.raw[3],
        HashId::from_i64(-736280260),
        "raw[3] = var[5] = 0xD41D413C"
    );
    // raw[7] = var[9] = 1475254969 → 0x57EE9AB9
    assert_eq!(
        info.raw[7],
        HashId::from_i64(1475254969),
        "raw[7] = var[9] = 0x57EE9AB9"
    );
    // raw[8] = var[10] = -813411175 → 0xCF845499
    assert_eq!(
        info.raw[8],
        HashId::from_i64(-813411175),
        "raw[8] = var[10] = 0xCF845499"
    );
    // raw[9] = var[11] = 2083486252 → 0x7C2F7A2C
    assert_eq!(
        info.raw[9],
        HashId::from_i64(2083486252),
        "raw[9] = var[11] = 0x7C2F7A2C"
    );
}

// — msa999999_trigger —

#[test]
fn real_file_trigger_compte() {
    let Some(root) = load_json(REAL_TRIGGER) else {
        return;
    };
    let items = parse_mission_trigger(&root);
    // msa999999_trigger_0.04.78 : DATA_COUNT_0.variables[0].value = "1" → 1 DATA_ITEM
    assert_eq!(items.len(), 1, "trigger: 1 DATA_ITEM");
}

#[test]
fn real_file_trigger_item0_type_et_hash() {
    let Some(root) = load_json(REAL_TRIGGER) else {
        return;
    };
    let items = parse_mission_trigger(&root);

    // DATA_ITEM_0.variables[0].value = "1" → trigger_type = 1
    assert_eq!(items[0].trigger_type, 1, "trigger_type = 1");
    // DATA_ITEM_0.variables[1].value = "-1369358024" → 0xAE614138
    assert_eq!(
        items[0].trigger_hash,
        HashId::from_i64(-1369358024),
        "trigger_hash = 0xAE614138"
    );
}

#[test]
fn real_file_trigger_item0_data() {
    let Some(root) = load_json(REAL_TRIGGER) else {
        return;
    };
    let items = parse_mission_trigger(&root);

    // DATA_ITEM_0.variables[3] : type=String, value="AAAAAAoDNbkZNtoAAQB4"
    assert_eq!(
        items[0].data, "AAAAAAoDNbkZNtoAAQB4",
        "data = \"AAAAAAoDNbkZNtoAAQB4\""
    );
    assert!(!items[0].data.is_empty(), "data non vide");
}

#[test]
fn real_file_trigger_item0_flags() {
    let Some(root) = load_json(REAL_TRIGGER) else {
        return;
    };
    let items = parse_mission_trigger(&root);

    // DATA_ITEM_0 : var[2]=0, var[4]=1, var[5]=0, var[6]=1
    assert_eq!(items[0].flag2, 0, "flag2 = 0");
    assert_eq!(items[0].flag4, 1, "flag4 = 1");
    assert_eq!(items[0].flag5, 0, "flag5 = 0");
    assert_eq!(items[0].flag6, 1, "flag6 = 1");
}
