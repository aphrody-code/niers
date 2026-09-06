#![allow(clippy::pedantic)]
//! Tests golden `dungeon` — valeurs réelles tirées de :
//! - `data/common/gamedata/dungeon/gimmick_system_num_config.cfg.bin.json`
//!
//! ## Structure vérifiée dans le fichier JSON réel
//!
//! ### DUNGEON_NUM_TABLE_GROUP_0 (ligne 13)
//! - var\[0\] = -1568497890 → group_id (HashId::from_i64(-1568497890))
//! - contient 6 entrées DATA_0..5
//!
//! ### DUNGEON_NUM_TABLE_GROUP_DATA_0 (ligne 31–47)
//! - var\[0\] = -1334119126 → param_id (HashId::from_i64(-1334119126))
//! - var\[1\] = 19 (Int) → value = 19.0
//! - var\[2\] = 0 → flag = 0
//!
//! ### DUNGEON_NUM_TABLE_GROUP_DATA_3 (ligne 83–100)
//! - var\[0\] = 547509227 → param_id (HashId::from_i64(547509227))
//! - var\[1\] = "1,3" (Float) → value ≈ 1.3
//! - var\[2\] = 0 → flag = 0
//!
//! ### DUNGEON_NUM_TABLE_GROUP_1 (ligne 138–141)
//! - var\[0\] = 749769860 → group_id (HashId::from_i64(749769860))
//! - contient 3 entrées DATA_6..8
//!
//! ### DUNGEON_NUM_SYSTEM_INFO_0 (ligne 289–299)
//! - var\[0\] = 1594010342 → info_id (HashId::from_i64(1594010342))
//! - var\[1\] = -1568497890 → group_ref = group_id de GROUP_0
//!
//! ### DUNGEON_NUM_SYSTEM_INFO_GROUP_0 (ligne 311–320)
//! - var\[0\] = 1647881546 → group_id (HashId::from_i64(1647881546))
//! - var\[1\] = "AAAAAA8FNbkZNtoAAQAyAAAAAHE=" → payload base64
//!
//! ### DUNGEON_NUM_SYSTEM_INFO_2 (ligne 359–371)
//! - var\[0\] = 1009861791 → info_id (HashId::from_i64(1009861791))
//! - var\[1\] = 580647011 → group_ref = group_id de GROUP_3
//! - enfants: [] (pas de sous-groupe)

mod common;

extern crate std;

use nie_data::dungeon::{
    GimmickNumSystemInfo, GimmickNumTableGroup, parse_gimmick_system_num_config,
};
use nie_data::hash::HashId;
use serde_json::json;

// ─── Constantes issues du fichier réel ───────────────────────────────────────
//
// Toutes les valeurs sont calculées par `value as u32` (= `>>> 0` JS), comme HashId::from_i64.

/// -1568497890 as u32 = group_id de DUNGEON_NUM_TABLE_GROUP_0 (JSON ligne 14–19)
const GROUP_ID_0: HashId = HashId::from_i64(-1568497890);
/// 749769860 = group_id de DUNGEON_NUM_TABLE_GROUP_1 (JSON ligne 139–141)
const GROUP_ID_1: HashId = HashId::from_i64(749769860);
/// 1647881546 = group_id de DUNGEON_NUM_TABLE_GROUP_2 (JSON ligne 210–211)
const GROUP_ID_2: HashId = HashId::from_i64(1647881546);
/// 580647011 = group_id de DUNGEON_NUM_TABLE_GROUP_3 (JSON ligne 244–246)
const GROUP_ID_3: HashId = HashId::from_i64(580647011);

/// -1334119126 = param_id de DATA_0 (JSON ligne 33–38)
const DATA_0_PARAM_ID: HashId = HashId::from_i64(-1334119126);
/// 547509227 = param_id de DATA_3 (JSON ligne 84–89)
const DATA_3_PARAM_ID: HashId = HashId::from_i64(547509227);
/// -1637982369 = param_id de DATA_5 (JSON ligne 122–128)
const DATA_5_PARAM_ID: HashId = HashId::from_i64(-1637982369);

/// 1594010342 = info_id de DUNGEON_NUM_SYSTEM_INFO_0 (JSON ligne 290–293)
const INFO_ID_0: HashId = HashId::from_i64(1594010342);
/// 2025856298 = info_id de DUNGEON_NUM_SYSTEM_INFO_1 (JSON ligne 324–327)
const INFO_ID_1: HashId = HashId::from_i64(2025856298);
/// 1009861791 = info_id de DUNGEON_NUM_SYSTEM_INFO_2 (JSON ligne 359–362)
const INFO_ID_2: HashId = HashId::from_i64(1009861791);

// ─── Fixture inline ───────────────────────────────────────────────────────────
//
// Extrait minimal du vrai fichier : GROUP_0 avec 2 données (int + float) et 1 info système.
// Valeurs lues octet-pour-octet dans gimmick_system_num_config.cfg.bin.json.

fn fixture_minimal() -> serde_json::Value {
    json!({
        "entries": [
            {
                // DUNGEON_NUM_TABLE_GROUP_LIST_BEG_0 — ligne 2–4
                "name": "DUNGEON_NUM_TABLE_GROUP_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    {
                        // DUNGEON_NUM_TABLE_GROUP_0 — ligne 13, var[0]=-1568497890
                        "name": "DUNGEON_NUM_TABLE_GROUP_0",
                        "variables": [{"type": "Int", "value": "-1568497890"}],
                        "children": [
                            {
                                "name": "DUNGEON_NUM_TABLE_GROUP_DATA_LIST_BEG_0",
                                "variables": [{"type": "Int", "value": "2"}],
                                "children": [
                                    {
                                        // DATA_0 — ligne 31, var[0]=-1334119126, var[1]=19, var[2]=0
                                        "name": "DUNGEON_NUM_TABLE_GROUP_DATA_0",
                                        "variables": [
                                            {"type": "Int", "value": "-1334119126"},
                                            {"type": "Int", "value": "19"},
                                            {"type": "Int", "value": "0"}
                                        ],
                                        "children": []
                                    },
                                    {
                                        // DATA_3 — ligne 83, var[0]=547509227, var[1]="1,3" (Float), var[2]=0
                                        "name": "DUNGEON_NUM_TABLE_GROUP_DATA_3",
                                        "variables": [
                                            {"type": "Int", "value": "547509227"},
                                            {"type": "Float", "value": "1,3"},
                                            {"type": "Int", "value": "0"}
                                        ],
                                        "children": []
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        ]
    })
}

fn fixture_avec_system_info() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "DUNGEON_NUM_TABLE_GROUP_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    {
                        "name": "DUNGEON_NUM_TABLE_GROUP_0",
                        "variables": [{"type": "Int", "value": "-1568497890"}],
                        "children": [
                            {
                                "name": "DUNGEON_NUM_TABLE_GROUP_DATA_LIST_BEG_0",
                                "variables": [{"type": "Int", "value": "1"}],
                                "children": [
                                    {
                                        // DATA_10 — ligne 262, var[0]=547509227, var[1]="2,5" (Float), var[2]=0
                                        "name": "DUNGEON_NUM_TABLE_GROUP_DATA_10",
                                        "variables": [
                                            {"type": "Int", "value": "547509227"},
                                            {"type": "Float", "value": "2,5"},
                                            {"type": "Int", "value": "0"}
                                        ],
                                        "children": []
                                    },
                                    {
                                        // DUNGEON_NUM_SYSTEM_INFO_LIST_BEG_0 — ligne 281, count=1
                                        "name": "DUNGEON_NUM_SYSTEM_INFO_LIST_BEG_0",
                                        "variables": [{"type": "Int", "value": "1"}],
                                        "children": [
                                            {
                                                // INFO_0 — ligne 289, var[0]=1594010342, var[1]=-1568497890
                                                "name": "DUNGEON_NUM_SYSTEM_INFO_0",
                                                "variables": [
                                                    {"type": "Int", "value": "1594010342"},
                                                    {"type": "Int", "value": "-1568497890"}
                                                ],
                                                "children": [
                                                    {
                                                        "name": "DUNGEON_NUM_SYSTEM_INFO_GROUP_LIST_BEG_0",
                                                        "variables": [{"type": "Int", "value": "1"}],
                                                        "children": [
                                                            {
                                                                // INFO_GROUP_0 — ligne 311,
                                                                // var[0]=1647881546, var[1]="AAAAAA8FNbkZNtoAAQAyAAAAAHE="
                                                                "name": "DUNGEON_NUM_SYSTEM_INFO_GROUP_0",
                                                                "variables": [
                                                                    {"type": "Int", "value": "1647881546"},
                                                                    {"type": "String", "value": "AAAAAA8FNbkZNtoAAQAyAAAAAHE="}
                                                                ],
                                                                "children": []
                                                            }
                                                        ]
                                                    }
                                                ]
                                            }
                                        ]
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur fixture minimale ───────────────────────────────────────────────

#[test]
fn fixture_groupe_compte() {
    let cfg = parse_gimmick_system_num_config(&fixture_minimal());
    // 1 seul GROUP dans la fixture (GROUP_0)
    assert_eq!(cfg.groups.len(), 1, "1 groupe dans la fixture minimale");
}

#[test]
fn fixture_groupe_id_0() {
    let cfg = parse_gimmick_system_num_config(&fixture_minimal());
    // GROUP_0 : var[0] = -1568497890 (ligne 13–19)
    assert_eq!(
        cfg.groups[0].group_id, GROUP_ID_0,
        "group_id[0] = -1568497890"
    );
}

#[test]
fn fixture_entrees_compte() {
    let cfg = parse_gimmick_system_num_config(&fixture_minimal());
    // 2 entrées DATA dans le groupe (DATA_0 et DATA_3)
    assert_eq!(
        cfg.groups[0].entries.len(),
        2,
        "2 entrées de données dans GROUP_0 (fixture)"
    );
}

#[test]
fn fixture_data_0_param_id_et_valeur() {
    let cfg = parse_gimmick_system_num_config(&fixture_minimal());
    let d = &cfg.groups[0].entries[0];
    // DATA_0 : var[0]=-1334119126 → param_id, var[1]=19 → value=19.0, var[2]=0 → flag=0
    assert_eq!(d.param_id, DATA_0_PARAM_ID, "param_id[0] = -1334119126");
    assert_eq!(d.value, 19.0_f64, "value[0] = 19.0 (Int 19)");
    assert_eq!(d.flag, 0, "flag[0] = 0");
}

#[test]
fn fixture_data_3_float() {
    let cfg = parse_gimmick_system_num_config(&fixture_minimal());
    let d = &cfg.groups[0].entries[1];
    // DATA_3 : var[0]=547509227, var[1]="1,3" (Float) → value≈1.3, var[2]=0
    assert_eq!(d.param_id, DATA_3_PARAM_ID, "param_id[3] = 547509227");
    assert!(
        (d.value - 1.3_f64).abs() < 1e-9,
        "value[3] ≈ 1.3 (Float '1,3'); réel = {}",
        d.value
    );
    assert_eq!(d.flag, 0, "flag[3] = 0");
}

#[test]
fn fixture_list_beg_ignore() {
    // Les conteneurs DUNGEON_NUM_TABLE_GROUP_LIST_BEG_* ne doivent pas être comptés comme groupes
    let cfg = parse_gimmick_system_num_config(&fixture_minimal());
    assert_eq!(cfg.groups.len(), 1, "LIST_BEG ignoré → 1 groupe");
}

#[test]
fn fixture_data_list_beg_ignore() {
    // Les conteneurs DATA_LIST_BEG_* ne doivent pas apparaître comme entrées de données
    let cfg = parse_gimmick_system_num_config(&fixture_minimal());
    assert_eq!(
        cfg.groups[0].entries.len(),
        2,
        "DATA_LIST_BEG ignoré → 2 entrées"
    );
}

#[test]
fn fixture_find_group() {
    let cfg = parse_gimmick_system_num_config(&fixture_minimal());
    // find_group doit retrouver GROUP_0
    assert!(
        cfg.find_group(GROUP_ID_0).is_some(),
        "find_group(GROUP_ID_0) doit trouver le groupe"
    );
    assert!(
        cfg.find_group(HashId(0xDEADBEEF)).is_none(),
        "find_group(inconnu) → None"
    );
}

// ─── Tests sur fixture avec info système ─────────────────────────────────────

#[test]
fn fixture_system_info_compte() {
    let cfg = parse_gimmick_system_num_config(&fixture_avec_system_info());
    // 1 DUNGEON_NUM_SYSTEM_INFO dans la fixture
    assert_eq!(cfg.system_infos.len(), 1, "1 SYSTEM_INFO dans la fixture");
}

#[test]
fn fixture_system_info_0_ids() {
    let cfg = parse_gimmick_system_num_config(&fixture_avec_system_info());
    let info: &GimmickNumSystemInfo = &cfg.system_infos[0];
    // INFO_0 : var[0]=1594010342 → info_id, var[1]=-1568497890 → group_ref
    assert_eq!(info.info_id, INFO_ID_0, "info_id[0] = 1594010342");
    assert_eq!(
        info.group_ref, GROUP_ID_0,
        "group_ref[0] = -1568497890 (= GROUP_ID_0)"
    );
}

#[test]
fn fixture_system_info_sous_groupe() {
    let cfg = parse_gimmick_system_num_config(&fixture_avec_system_info());
    let info = &cfg.system_infos[0];
    // INFO_0 a 1 sous-groupe
    assert_eq!(info.sub_groups.len(), 1, "INFO_0 a 1 sous-groupe");

    // INFO_GROUP_0 : var[0]=1647881546 → group_id
    assert_eq!(
        info.sub_groups[0].group_id, GROUP_ID_2,
        "sub_group_id = 1647881546 (= GROUP_ID_2)"
    );
    // INFO_GROUP_0 : var[1] = payload base64 (ligne 319)
    assert_eq!(
        info.sub_groups[0].payload_b64, "AAAAAA8FNbkZNtoAAQAyAAAAAHE=",
        "payload_b64 INFO_GROUP_0"
    );
}

#[test]
fn fixture_data_10_float_25() {
    let cfg = parse_gimmick_system_num_config(&fixture_avec_system_info());
    // DATA_10 : var[0]=547509227, var[1]="2,5" (Float), var[2]=0
    let g = &cfg.groups[0];
    assert_eq!(
        g.entries.len(),
        1,
        "1 entrée de données dans GROUP_0 (fixture info)"
    );
    let d = &g.entries[0];
    assert_eq!(d.param_id, DATA_3_PARAM_ID, "param_id DATA_10 = 547509227");
    assert!(
        (d.value - 2.5_f64).abs() < 1e-9,
        "value DATA_10 ≈ 2.5 (Float '2,5'); réel = {}",
        d.value
    );
}

#[test]
fn fixture_system_info_list_beg_ignore() {
    // DUNGEON_NUM_SYSTEM_INFO_LIST_BEG_* ne doit pas être compté comme info système
    let cfg = parse_gimmick_system_num_config(&fixture_avec_system_info());
    assert_eq!(
        cfg.system_infos.len(),
        1,
        "SYSTEM_INFO_LIST_BEG ignoré → 1 info système"
    );
}

#[test]
fn fixture_find_system_info() {
    let cfg = parse_gimmick_system_num_config(&fixture_avec_system_info());
    assert!(
        cfg.find_system_info(INFO_ID_0).is_some(),
        "find_system_info(INFO_ID_0) doit trouver l'info"
    );
    assert!(
        cfg.find_system_info(HashId(0xDEADBEEF)).is_none(),
        "find_system_info(inconnu) → None"
    );
}

// ─── Tests sur fichier réel (skip silencieux si absent) ───────────────────────

const REAL_GIMMICK_NUM: &str = "dungeon/gimmick_system_num_config.cfg.bin.json";

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

#[test]
fn real_file_groupes_compte() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // 4 groupes (DUNGEON_NUM_TABLE_GROUP_0..3) dans le fichier réel
    assert_eq!(cfg.groups.len(), 4, "4 groupes dans le fichier réel");
}

#[test]
fn real_file_groupe_0_id() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // DUNGEON_NUM_TABLE_GROUP_0 : var[0] = -1568497890 (JSON ligne 14–19)
    assert_eq!(
        cfg.groups[0].group_id, GROUP_ID_0,
        "group_id[0] = -1568497890"
    );
}

#[test]
fn real_file_groupe_0_entrees_compte() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // GROUP_0 porte 6 entrées directes (DATA_0..5) selon DATA_LIST_BEG_0.var[0]=6 (ligne 24)
    assert_eq!(
        cfg.groups[0].entries.len(),
        6,
        "GROUP_0 : 6 entrées de données"
    );
}

#[test]
fn real_file_data_0_param_id() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // DATA_0 : var[0] = -1334119126 (JSON ligne 36–39)
    assert_eq!(
        cfg.groups[0].entries[0].param_id, DATA_0_PARAM_ID,
        "DATA_0.param_id = -1334119126"
    );
}

#[test]
fn real_file_data_0_valeur_entiere() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // DATA_0 : var[1] = 19 (Int, JSON ligne 40–43)
    assert_eq!(
        cfg.groups[0].entries[0].value, 19.0_f64,
        "DATA_0.value = 19.0"
    );
}

#[test]
fn real_file_data_0_flag() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // DATA_0 : var[2] = 0 (JSON ligne 44–47)
    assert_eq!(cfg.groups[0].entries[0].flag, 0, "DATA_0.flag = 0");
}

#[test]
fn real_file_data_3_float() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // DATA_3 : var[1] = "1,3" (Float, JSON ligne 91–98)
    let v = cfg.groups[0].entries[3].value;
    assert!(
        (v - 1.3_f64).abs() < 1e-9,
        "DATA_3.value ≈ 1.3 (Float '1,3'); réel = {v}"
    );
}

#[test]
fn real_file_data_3_param_id() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // DATA_3 : var[0] = 547509227 (JSON ligne 84–89)
    assert_eq!(
        cfg.groups[0].entries[3].param_id, DATA_3_PARAM_ID,
        "DATA_3.param_id = 547509227"
    );
}

#[test]
fn real_file_groupe_1_id_et_entrees() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // GROUP_1 : group_id=749769860 (ligne 139), 3 entrées (DATA_6..8, ligne 147)
    assert_eq!(
        cfg.groups[1].group_id, GROUP_ID_1,
        "group_id[1] = 749769860"
    );
    assert_eq!(
        cfg.groups[1].entries.len(),
        3,
        "GROUP_1 : 3 entrées de données"
    );
}

#[test]
fn real_file_groupe_1_data_8_valeur_negative() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // DATA_8 (troisième entrée de GROUP_1) : var[1] = -10 (Int, ligne 197–200)
    let v = cfg.groups[1].entries[2].value;
    assert_eq!(v, -10.0_f64, "DATA_8.value = -10.0");
}

#[test]
fn real_file_groupe_2_id_et_entrees() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // GROUP_2 : group_id=1647881546 (ligne 210), 1 entrée (DATA_9)
    assert_eq!(
        cfg.groups[2].group_id, GROUP_ID_2,
        "group_id[2] = 1647881546"
    );
    assert_eq!(
        cfg.groups[2].entries.len(),
        1,
        "GROUP_2 : 1 entrée de données"
    );
}

#[test]
fn real_file_groupe_3_id_et_entrees() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // GROUP_3 : group_id=580647011 (ligne 244), 1 entrée (DATA_10)
    assert_eq!(
        cfg.groups[3].group_id, GROUP_ID_3,
        "group_id[3] = 580647011"
    );
    assert_eq!(
        cfg.groups[3].entries.len(),
        1,
        "GROUP_3 : 1 entrée de données"
    );
}

#[test]
fn real_file_data_10_float_25() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // DATA_10 (unique entrée de GROUP_3) : var[1] = "2,5" (Float, ligne 268–273)
    let v = cfg.groups[3].entries[0].value;
    assert!(
        (v - 2.5_f64).abs() < 1e-9,
        "DATA_10.value ≈ 2.5 (Float '2,5'); réel = {v}"
    );
}

#[test]
fn real_file_system_infos_compte() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // 3 infos système (INFO_0, INFO_1, INFO_2) dans le fichier réel
    assert_eq!(
        cfg.system_infos.len(),
        3,
        "3 infos système dans le fichier réel"
    );
}

#[test]
fn real_file_system_info_0() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    let info: &GimmickNumSystemInfo = &cfg.system_infos[0];
    // INFO_0 : var[0]=1594010342 (ligne 292–295), var[1]=-1568497890 (ligne 296–299)
    assert_eq!(info.info_id, INFO_ID_0, "info_id[0] = 1594010342");
    assert_eq!(
        info.group_ref, GROUP_ID_0,
        "group_ref[0] = -1568497890 (GROUP_ID_0)"
    );
    // INFO_0 a 1 sous-groupe
    assert_eq!(info.sub_groups.len(), 1, "INFO_0 : 1 sous-groupe");
}

#[test]
fn real_file_system_info_0_sous_groupe() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    let sg = &cfg.system_infos[0].sub_groups[0];
    // INFO_GROUP_0 : var[0]=1647881546 (ligne 315–318) → GROUP_ID_2
    assert_eq!(
        sg.group_id, GROUP_ID_2,
        "INFO_GROUP_0.group_id = 1647881546 (GROUP_ID_2)"
    );
    // INFO_GROUP_0 : var[1]="AAAAAA8FNbkZNtoAAQAyAAAAAHE=" (ligne 319–322)
    assert_eq!(
        sg.payload_b64, "AAAAAA8FNbkZNtoAAQAyAAAAAHE=",
        "INFO_GROUP_0.payload_b64"
    );
}

#[test]
fn real_file_system_info_1() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    let info = &cfg.system_infos[1];
    // INFO_1 : var[0]=2025856298 (ligne 328–331), var[1]=-1568497890 (ligne 332–335)
    assert_eq!(info.info_id, INFO_ID_1, "info_id[1] = 2025856298");
    assert_eq!(
        info.group_ref, GROUP_ID_0,
        "group_ref[1] = -1568497890 (GROUP_ID_0)"
    );
    assert_eq!(info.sub_groups.len(), 1, "INFO_1 : 1 sous-groupe");
}

#[test]
fn real_file_system_info_1_sous_groupe_payload() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    let sg = &cfg.system_infos[1].sub_groups[0];
    // INFO_GROUP_1 : var[0]=749769860 (ligne 348–351) → GROUP_ID_1
    assert_eq!(
        sg.group_id, GROUP_ID_1,
        "INFO_GROUP_1.group_id = 749769860 (GROUP_ID_1)"
    );
    // INFO_GROUP_1 : var[1]="AAAAABgFNSo9RUMACgEoAAYCNEIVKDMyAAAAAXg=" (ligne 352–355)
    assert_eq!(
        sg.payload_b64, "AAAAABgFNSo9RUMACgEoAAYCNEIVKDMyAAAAAXg=",
        "INFO_GROUP_1.payload_b64"
    );
}

#[test]
fn real_file_system_info_2() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    let info = &cfg.system_infos[2];
    // INFO_2 : var[0]=1009861791 (ligne 363–366), var[1]=580647011 (ligne 367–370) → GROUP_3
    assert_eq!(info.info_id, INFO_ID_2, "info_id[2] = 1009861791");
    assert_eq!(
        info.group_ref, GROUP_ID_3,
        "group_ref[2] = 580647011 (GROUP_ID_3)"
    );
    // INFO_2 n'a pas de sous-groupe (children: [] dans le JSON)
    assert_eq!(
        info.sub_groups.len(),
        0,
        "INFO_2 : 0 sous-groupe (enfants vides)"
    );
}

#[test]
fn real_file_tous_flags_nuls() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // Dans le fichier réel, var[2] (flag) vaut 0 pour toutes les entrées
    for g in &cfg.groups {
        for entry in &g.entries {
            assert_eq!(
                entry.flag, 0,
                "flag non-nul dans groupe {:?}, param {:?}",
                g.group_id, entry.param_id
            );
        }
    }
}

#[test]
fn real_file_find_group_group_3() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // find_group doit retrouver GROUP_3 (noeud le plus profond)
    let found = cfg.find_group(GROUP_ID_3);
    assert!(
        found.is_some(),
        "find_group(GROUP_ID_3) doit trouver le groupe"
    );
    assert_eq!(found.unwrap().entries.len(), 1, "GROUP_3 a 1 entrée");
}

#[test]
fn real_file_find_system_info_2() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // find_system_info(INFO_ID_2) doit retrouver l'entrée sans sous-groupe
    let found = cfg.find_system_info(INFO_ID_2);
    assert!(
        found.is_some(),
        "find_system_info(INFO_ID_2) doit trouver l'info"
    );
    assert_eq!(
        found.unwrap().sub_groups.len(),
        0,
        "INFO_2 n'a pas de sous-groupe"
    );
}

#[test]
fn real_file_total_entrees_donnees() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // Total : 6+3+1+1 = 11 entrées de données réparties sur 4 groupes
    let total: usize = cfg.groups.iter().map(|g| g.entries.len()).sum();
    assert_eq!(total, 11, "11 entrées de données au total");
}

#[test]
fn real_file_data_5_param_id() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // DATA_5 (6ème entrée de GROUP_0) : var[0] = -1637982369 (JSON ligne 122–127)
    // C'est l'entrée qui porte GROUP_1 comme enfant
    assert_eq!(
        cfg.groups[0].entries[5].param_id, DATA_5_PARAM_ID,
        "DATA_5.param_id = -1637982369"
    );
}

#[test]
fn real_file_groupe_ids_uniques() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // Les 4 group_id doivent être distincts
    let ids: Vec<HashId> = cfg.groups.iter().map(|g| g.group_id).collect();
    let unique_count = {
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        sorted.len()
    };
    assert_eq!(unique_count, 4, "4 group_id distincts");
}

// ─── Test réel : groupe dans groupe n'est pas dupliqué ───────────────────────

#[test]
fn real_file_groupe_1_pas_groupe_0() {
    let Some(root) = load_json(REAL_GIMMICK_NUM) else {
        return;
    };
    let cfg = parse_gimmick_system_num_config(&root);
    // GROUP_1 est imbriqué dans GROUP_0 via DATA_5, mais doit être un groupe séparé
    let g: &GimmickNumTableGroup = cfg.find_group(GROUP_ID_1).expect("GROUP_1 doit exister");
    // GROUP_1 n'a que 3 entrées, pas 6 comme GROUP_0
    assert_eq!(
        g.entries.len(),
        3,
        "GROUP_1 a exactement 3 entrées directes"
    );
    // GROUP_1 n'a pas les mêmes entrées que GROUP_0
    assert_ne!(g.group_id, GROUP_ID_0, "GROUP_1 ≠ GROUP_0");
}
