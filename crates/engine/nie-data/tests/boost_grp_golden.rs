#![allow(clippy::pedantic)]
//! Tests golden `boost_grp` — valeurs réelles tirées de :
//! `boost_grp/boost_player_group_config_0.00.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ
//!
//! ### BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_0
//! - var\[0\] = 0 → table_index = 0
//!   (JSON lignes 13–25, `BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_0.variables[0].value = "0"`)
//! - var\[1\] = -1341563653 → sprite_hash = 0xB0095CFB
//!   (JSON ligne 19, `variables[1].value = "-1341563653"`)
//!
//! ### BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_3
//! - var\[0\] = 3 → table_index = 3
//! - var\[1\] = 1372344836 → sprite_hash = 0x51CC5204
//!   (JSON lignes 56–68, `variables[1].value = "1372344836"`)
//!
//! ### BOOST_PLAYER_GRP_CONFIG_0
//! - var\[0\] = 1814400 → duration = 1814400
//!   (JSON lignes 81–106, `BOOST_PLAYER_GRP_CONFIG_0.variables[0].value = "1814400"`)
//! - var\[1\] = 0 → config_type = 0
//! - var\[2\]=0, var\[3\]=1, var\[4\]=2, var\[5\]=3 → table_refs = [0,1,2,3]
//!
//! ### BOOST_PLAYER_GRP_CONFIG_1
//! - var\[0\] = 1209600 → duration = 1209600
//!   (JSON lignes 109–135, `BOOST_PLAYER_GRP_CONFIG_1.variables[0].value = "1209600"`)
//! - var\[1\] = 1 → config_type = 1
//! - var\[2\]=1, var\[3\]=2, var\[4\]=3, var\[5\]=0 → table_refs = [1,2,3,0]
//!
//! ### BOOST_PLAYER_GRP_INFO_0
//! - var\[0\] = 1762786800 → group_hash = 0x6911FDF0
//!   (JSON lignes 241–249, `BOOST_PLAYER_GRP_INFO_0.variables[0].value = "1762786800"`)
//!
//! ### BOOST_PLAYER_GRP_INFO_REF_CONFIG_0
//! - var\[0\] = 0 → config_offset = 0
//!   (JSON lignes 250–263, `BOOST_PLAYER_GRP_INFO_REF_CONFIG_0.variables[0].value = "0"`)
//! - var\[1\] = 5 → config_count = 5
//!   (JSON ligne 258, `variables[1].value = "5"`)

mod common;

extern crate std;

use nie_data::boost_grp::parse_boost_grp_config;
use nie_data::hash::HashId;
use serde_json::json;

// ─── Constantes golden (vérifiées Python : value & 0xFFFFFFFF) ───────────────

/// -1341563653 & 0xFFFFFFFF = 0xB0095CFB
const SPRITE_HASH_0: HashId = HashId(0xB0095CFB);
/// -1196427471 & 0xFFFFFFFF = 0xB8AFF731
const SPRITE_HASH_1: HashId = HashId(0xB8AFF731);
/// -811022425 & 0xFFFFFFFF = 0xCFA8C7A7
const SPRITE_HASH_2: HashId = HashId(0xCFA8C7A7);
/// 1372344836 (positif) = 0x51CC5204
const SPRITE_HASH_3: HashId = HashId(0x51CC5204);
/// 1762786800 (positif) = 0x6911FDF0
const GROUP_HASH_0: HashId = HashId(0x6911FDF0);

// ─── Fixture inline (extrait exact du dump réel) ─────────────────────────────

fn fixture_boost_grp() -> serde_json::Value {
    json!({
        "entries": [
            // BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_LIST_BEG_0 — JSON lignes 2-69
            {
                "name": "BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "4"}],
                "children": [
                    // BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_0 — JSON lignes 13-25
                    {
                        "name": "BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "0"},          // table_index
                            {"type": "Int", "value": "-1341563653"} // sprite_hash → 0xB0095CFB
                        ],
                        "children": []
                    },
                    // BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_3 — JSON lignes 56-68
                    {
                        "name": "BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_3",
                        "variables": [
                            {"type": "Int", "value": "3"},         // table_index
                            {"type": "Int", "value": "1372344836"} // sprite_hash → 0x51CC5204
                        ],
                        "children": []
                    }
                ]
            },
            // BOOST_PLAYER_GRP_CONFIG_LIST_BEG_0 — JSON lignes 70-230
            {
                "name": "BOOST_PLAYER_GRP_CONFIG_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    // BOOST_PLAYER_GRP_CONFIG_0 — JSON lignes 81-106
                    {
                        "name": "BOOST_PLAYER_GRP_CONFIG_0",
                        "variables": [
                            {"type": "Int", "value": "1814400"}, // duration
                            {"type": "Int", "value": "0"},        // config_type
                            {"type": "Int", "value": "0"},        // table_refs[0]
                            {"type": "Int", "value": "1"},        // table_refs[1]
                            {"type": "Int", "value": "2"},        // table_refs[2]
                            {"type": "Int", "value": "3"}         // table_refs[3]
                        ],
                        "children": []
                    },
                    // BOOST_PLAYER_GRP_CONFIG_1 — JSON lignes 109-135
                    {
                        "name": "BOOST_PLAYER_GRP_CONFIG_1",
                        "variables": [
                            {"type": "Int", "value": "1209600"}, // duration
                            {"type": "Int", "value": "1"},        // config_type
                            {"type": "Int", "value": "1"},        // table_refs[0]
                            {"type": "Int", "value": "2"},        // table_refs[1]
                            {"type": "Int", "value": "3"},        // table_refs[2]
                            {"type": "Int", "value": "0"}         // table_refs[3]
                        ],
                        "children": []
                    }
                ]
            },
            // BOOST_PLAYER_GRP_INFO_LIST_BEG_0 — JSON lignes 231-265
            {
                "name": "BOOST_PLAYER_GRP_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    // BOOST_PLAYER_GRP_INFO_0 — JSON lignes 241-249
                    {
                        "name": "BOOST_PLAYER_GRP_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "1762786800"} // group_hash → 0x6911FDF0
                        ],
                        "children": []
                    },
                    // BOOST_PLAYER_GRP_INFO_REF_CONFIG_0 — JSON lignes 250-263
                    {
                        "name": "BOOST_PLAYER_GRP_INFO_REF_CONFIG_0",
                        "variables": [
                            {"type": "Int", "value": "0"}, // config_offset
                            {"type": "Int", "value": "2"}  // config_count (fixture: 2 configs)
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur fixture : BoostSpritTableInfo ─────────────────────────────────

#[test]
fn sprit_table_compte() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // fixture : 2 BOOST_PLAYER_GRP_SPRIT_TABLE_INFO (LIST_BEG ignoré)
    assert_eq!(cfg.sprit_table.len(), 2, "fixture : 2 sprit_table");
}

#[test]
fn sprit_table_info_0_table_index() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_0.var[0] = 0 → table_index = 0
    assert_eq!(cfg.sprit_table[0].table_index, 0, "table_index[0] = 0");
}

#[test]
fn sprit_table_info_0_sprite_hash() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_0.var[1] = -1341563653 → 0xB0095CFB
    assert_eq!(
        cfg.sprit_table[0].sprite_hash, SPRITE_HASH_0,
        "sprite_hash[0] = 0xB0095CFB"
    );
}

#[test]
fn sprit_table_info_3_table_index_et_hash() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_3.var[0] = 3, var[1] = 1372344836 → 0x51CC5204
    assert_eq!(cfg.sprit_table[1].table_index, 3, "table_index[3] = 3");
    assert_eq!(
        cfg.sprit_table[1].sprite_hash, SPRITE_HASH_3,
        "sprite_hash[3] = 0x51CC5204"
    );
}

// ─── Tests sur fixture : BoostPlayerGrpConfig ────────────────────────────────

#[test]
fn config_compte() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // fixture : 2 BOOST_PLAYER_GRP_CONFIG
    assert_eq!(cfg.configs.len(), 2, "fixture : 2 configs");
}

#[test]
fn config_0_duration() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // BOOST_PLAYER_GRP_CONFIG_0.var[0] = 1814400
    assert_eq!(cfg.configs[0].duration, 1814400, "duration[0] = 1814400");
}

#[test]
fn config_0_type_et_table_refs() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // BOOST_PLAYER_GRP_CONFIG_0.var[1] = 0, var[2..5] = [0,1,2,3]
    assert_eq!(cfg.configs[0].config_type, 0, "config_type[0] = 0");
    assert_eq!(
        cfg.configs[0].table_refs,
        [0, 1, 2, 3],
        "table_refs[0] = [0,1,2,3]"
    );
}

#[test]
fn config_1_duration_et_rotation() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // BOOST_PLAYER_GRP_CONFIG_1 : duration=1209600, config_type=1, table_refs=[1,2,3,0]
    assert_eq!(cfg.configs[1].duration, 1209600, "duration[1] = 1209600");
    assert_eq!(cfg.configs[1].config_type, 1, "config_type[1] = 1");
    assert_eq!(
        cfg.configs[1].table_refs,
        [1, 2, 3, 0],
        "table_refs[1] = [1,2,3,0]"
    );
}

// ─── Tests sur fixture : BoostPlayerGrpInfo ──────────────────────────────────

#[test]
fn group_compte() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // fixture : 1 BOOST_PLAYER_GRP_INFO
    assert_eq!(cfg.groups.len(), 1, "fixture : 1 group");
}

#[test]
fn group_0_hash() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // BOOST_PLAYER_GRP_INFO_0.var[0] = 1762786800 → 0x6911FDF0
    assert_eq!(
        cfg.groups[0].group_hash, GROUP_HASH_0,
        "group_hash[0] = 0x6911FDF0"
    );
}

// ─── Tests sur fixture : BoostPlayerGrpInfoRefConfig ─────────────────────────

#[test]
fn ref_config_compte() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // fixture : 1 BOOST_PLAYER_GRP_INFO_REF_CONFIG
    assert_eq!(cfg.ref_configs.len(), 1, "fixture : 1 ref_config");
}

#[test]
fn ref_config_0_offset_et_count() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // BOOST_PLAYER_GRP_INFO_REF_CONFIG_0.var[0] = 0, var[1] = 2 (fixture)
    assert_eq!(cfg.ref_configs[0].config_offset, 0, "config_offset[0] = 0");
    assert_eq!(
        cfg.ref_configs[0].config_count, 2,
        "config_count[0] = 2 (fixture)"
    );
}

// ─── Tests : accesseurs ──────────────────────────────────────────────────────

#[test]
fn configs_of_plage_correcte() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    // configs_of doit renvoyer la tranche correcte
    let slice = cfg.configs_of(&cfg.ref_configs[0]);
    assert_eq!(
        slice.len(),
        2,
        "configs_of : 2 configs pour offset=0, count=2"
    );
    assert_eq!(
        slice[0].duration, 1814400,
        "premier config : duration=1814400"
    );
}

#[test]
fn find_group_trouve_et_absent() {
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);

    // find_group(0x6911FDF0) doit trouver le groupe
    let found = cfg.find_group(GROUP_HASH_0);
    assert!(found.is_some(), "find_group(GROUP_HASH_0)");
    assert_eq!(found.unwrap().group_hash, GROUP_HASH_0);

    // hash absent → None
    assert!(cfg.find_group(HashId(0xDEADBEEF)).is_none());
}

#[test]
fn list_beg_ignore() {
    // Les noeuds *_LIST_BEG_* ne doivent pas être comptés
    let root = fixture_boost_grp();
    let cfg = parse_boost_grp_config(&root);
    assert_eq!(cfg.sprit_table.len(), 2, "LIST_BEG ignoré dans sprit_table");
    assert_eq!(cfg.configs.len(), 2, "LIST_BEG ignoré dans configs");
    assert_eq!(cfg.groups.len(), 1, "LIST_BEG ignoré dans groups");
    assert_eq!(cfg.ref_configs.len(), 1, "LIST_BEG ignoré dans ref_configs");
}

// ─── Tests sur fichier réel (skip silencieux si absent) ───────────────────────

const REAL_FILE: &str = "boost_grp/boost_player_group_config_0.00.00.cfg.bin.json";

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
fn real_file_sprit_table_compte() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);
    // dump réel : BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_LIST_BEG_0.var[0] = 4
    assert_eq!(
        cfg.sprit_table.len(),
        4,
        "dump réel : 4 BOOST_PLAYER_GRP_SPRIT_TABLE_INFO"
    );
}

#[test]
fn real_file_config_compte() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);
    // dump réel : BOOST_PLAYER_GRP_CONFIG_LIST_BEG_0.var[0] = 5
    assert_eq!(
        cfg.configs.len(),
        5,
        "dump réel : 5 BOOST_PLAYER_GRP_CONFIG"
    );
}

#[test]
fn real_file_group_compte() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);
    // dump réel : 1 BOOST_PLAYER_GRP_INFO + 1 REF_CONFIG
    assert_eq!(cfg.groups.len(), 1, "dump réel : 1 BOOST_PLAYER_GRP_INFO");
    assert_eq!(cfg.ref_configs.len(), 1, "dump réel : 1 REF_CONFIG");
}

#[test]
fn real_file_sprit_table_info_0() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_0.var[0] = 0, var[1] = -1341563653 → 0xB0095CFB
    assert_eq!(
        cfg.sprit_table[0].table_index, 0,
        "sprit_table[0].table_index = 0"
    );
    assert_eq!(
        cfg.sprit_table[0].sprite_hash, SPRITE_HASH_0,
        "sprit_table[0].sprite_hash = 0xB0095CFB"
    );
}

#[test]
fn real_file_sprit_table_info_1() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_1.var[0] = 1, var[1] = -1196427471 → 0xB8AFF731
    assert_eq!(
        cfg.sprit_table[1].table_index, 1,
        "sprit_table[1].table_index = 1"
    );
    assert_eq!(
        cfg.sprit_table[1].sprite_hash, SPRITE_HASH_1,
        "sprit_table[1].sprite_hash = 0xB8AFF731"
    );
}

#[test]
fn real_file_sprit_table_info_2() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_2.var[1] = -811022425 → 0xCFA8C7A7
    assert_eq!(
        cfg.sprit_table[2].sprite_hash, SPRITE_HASH_2,
        "sprit_table[2].sprite_hash = 0xCFA8C7A7"
    );
}

#[test]
fn real_file_sprit_table_info_3() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_3.var[0] = 3, var[1] = 1372344836 → 0x51CC5204
    assert_eq!(
        cfg.sprit_table[3].table_index, 3,
        "sprit_table[3].table_index = 3"
    );
    assert_eq!(
        cfg.sprit_table[3].sprite_hash, SPRITE_HASH_3,
        "sprit_table[3].sprite_hash = 0x51CC5204"
    );
}

#[test]
fn real_file_config_0() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // BOOST_PLAYER_GRP_CONFIG_0 : var[0]=1814400, var[1]=0, var[2..5]=[0,1,2,3]
    assert_eq!(
        cfg.configs[0].duration, 1814400,
        "configs[0].duration = 1814400"
    );
    assert_eq!(cfg.configs[0].config_type, 0, "configs[0].config_type = 0");
    assert_eq!(
        cfg.configs[0].table_refs,
        [0, 1, 2, 3],
        "configs[0].table_refs = [0,1,2,3]"
    );
}

#[test]
fn real_file_config_1() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // BOOST_PLAYER_GRP_CONFIG_1 : var[0]=1209600, var[1]=1, var[2..5]=[1,2,3,0]
    assert_eq!(
        cfg.configs[1].duration, 1209600,
        "configs[1].duration = 1209600"
    );
    assert_eq!(cfg.configs[1].config_type, 1, "configs[1].config_type = 1");
    assert_eq!(
        cfg.configs[1].table_refs,
        [1, 2, 3, 0],
        "configs[1].table_refs = [1,2,3,0]"
    );
}

#[test]
fn real_file_config_4_rotation_complete() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // BOOST_PLAYER_GRP_CONFIG_4 : var[0]=1209600, var[1]=1, var[2..5]=[0,1,2,3]
    assert_eq!(
        cfg.configs[4].duration, 1209600,
        "configs[4].duration = 1209600"
    );
    assert_eq!(
        cfg.configs[4].table_refs,
        [0, 1, 2, 3],
        "configs[4].table_refs = [0,1,2,3]"
    );
}

#[test]
fn real_file_group_0_hash() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // BOOST_PLAYER_GRP_INFO_0.var[0] = 1762786800 → 0x6911FDF0
    assert_eq!(
        cfg.groups[0].group_hash, GROUP_HASH_0,
        "groups[0].group_hash = 0x6911FDF0"
    );
}

#[test]
fn real_file_ref_config_0() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // BOOST_PLAYER_GRP_INFO_REF_CONFIG_0 : var[0]=0, var[1]=5
    assert_eq!(
        cfg.ref_configs[0].config_offset, 0,
        "ref_configs[0].config_offset = 0"
    );
    assert_eq!(
        cfg.ref_configs[0].config_count, 5,
        "ref_configs[0].config_count = 5"
    );
}

#[test]
fn real_file_configs_of_toutes_les_configs() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // REF_CONFIG_0 = [0, 5] → configs_of renvoie les 5 configs
    let slice = cfg.configs_of(&cfg.ref_configs[0]);
    assert_eq!(
        slice.len(),
        5,
        "configs_of : 5 configs pour le groupe 0x6911FDF0"
    );
}

#[test]
fn real_file_find_group() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);

    // find_group(0x6911FDF0) doit trouver le groupe
    let found = cfg.find_group(GROUP_HASH_0);
    assert!(
        found.is_some(),
        "find_group(0x6911FDF0) doit trouver le groupe"
    );

    // hash absent → None
    assert!(cfg.find_group(HashId(0xDEADBEEF)).is_none());
}

#[test]
fn real_file_tous_sprit_table_indices_croissants() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);
    // Les indices 0..3 doivent être dans l'ordre croissant dans le dump réel
    for (i, entry) in cfg.sprit_table.iter().enumerate() {
        assert_eq!(
            entry.table_index, i as i64,
            "sprit_table[{i}].table_index = {i}"
        );
    }
}

#[test]
fn real_file_tous_sprit_hashes_non_nuls() {
    let Some(root) = load_json(REAL_FILE) else {
        return;
    };
    let cfg = parse_boost_grp_config(&root);
    let nuls = cfg
        .sprit_table
        .iter()
        .filter(|e| e.sprite_hash.is_zero())
        .count();
    assert_eq!(nuls, 0, "tous les sprite_hash doivent être non-nuls");
}
