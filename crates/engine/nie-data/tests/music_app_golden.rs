#![allow(clippy::pedantic)]
//! Tests golden `music_app` — valeurs réelles tirées de :
//! `data/common/gamedata/music_app/music_app_config.cfg.bin.json`
//!
//! ## Vérifications champ par champ
//!
//! ### `MUSIC_APP_INFO_ITEM_0` (groupe 1, premier item)
//! - var\[0\] = 1 → `app_category = 1`
//! - var\[1\] = 1653231064 → `music_id = 0x628A4DD8`
//! - var\[2\] = 2 → `track_no = 2`
//! - var\[3\] = 0 → `variant = 0`
//! - var\[4\] = 1169637250 → `entry_id = 0x45B73F82`
//! - var\[5\] = `"AAAAAA8FNbkZNtoAAQAyAAAnTHE="` (String) → `has_path = true`
//! - var\[6\] = 500 → `volume = 500`
//! - var\[7\] = 1 → `sort_index = 1`
//!
//! ### `MUSIC_APP_INFO_ITEM_1` (groupe 1, deuxième item)
//! - var\[1\] = 1653231064 → `music_id = 0x628A4DD8` (même ID que item_0)
//! - var\[2\] = 9 → `track_no = 9`
//! - var\[4\] = 1695668893 → `entry_id = 0x6511DA9D`
//! - var\[7\] = 2 → `sort_index = 2`
//!
//! ### `MUSIC_APP_INFO_ITEM_20` (premier item du groupe 2)
//! - var\[0\] = 2 → `app_category = 2`
//! - var\[1\] = 1009845833 → `music_id = 0x3C310649`
//! - var\[4\] = 878330871 → `entry_id = 0x345A43F7`
//! - var\[5\] = Int "0" → `path_b64 = ""`, `has_path = false`
//! - var\[7\] = 21 → `sort_index = 21`
//!
//! ### `MUSIC_APP_INFO_ITEM_21` (deuxième item du groupe 2, seul avec variant=1)
//! - var\[3\] = 1 → `variant = 1`
//! - var\[7\] = 22 → `sort_index = 22`
//!
//! ### `MUSIC_APP_INFO_ITEM_52` (premier item du groupe 3)
//! - var\[0\] = 3 → `app_category = 3`
//! - var\[1\] = 22097913 → `music_id = 0x01512FF9`
//! - var\[4\] = 154823239 → `entry_id = 0x093A6A47`
//! - var\[5\] = Int "0" → `has_path = false`
//! - var\[7\] = 53 → `sort_index = 53`
//!
//! ### `MUSIC_APP_INFO_ITEM_102` (premier item du groupe 4)
//! - var\[0\] = 4 → `app_category = 4`
//! - var\[1\] = 117412874 → `music_id = 0x06FF940A`
//! - var\[4\] = 244634036 → `entry_id = 0x0E94D1B4`
//! - var\[7\] = 103 → `sort_index = 103`

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::music_app::parse_music_app_config;
use serde_json::json;

// ─── Constantes golden ────────────────────────────────────────────────────────
//
// Toutes les valeurs hex sont calculées par `v & 0xFFFFFFFF` (Python).

/// `MUSIC_APP_INFO_ITEM_0.var[1] = 1653231064` → 1653231064 & 0xFFFFFFFF = 0x628A4DD8
const MUSIC_ID_0: HashId = HashId(0x628A_4DD8);
/// `MUSIC_APP_INFO_ITEM_0.var[4] = 1169637250` → 0x45B73F82
const ENTRY_ID_0: HashId = HashId(0x45B7_3F82);
/// `MUSIC_APP_INFO_ITEM_1.var[4] = 1695668893` → 0x6511DA9D
const ENTRY_ID_1: HashId = HashId(0x6511_DA9D);
/// `MUSIC_APP_INFO_ITEM_20.var[1] = 1009845833` → 0x3C310649
const MUSIC_ID_20: HashId = HashId(0x3C31_0649);
/// `MUSIC_APP_INFO_ITEM_20.var[4] = 878330871` → 0x345A43F7
const ENTRY_ID_20: HashId = HashId(0x345A_43F7);
/// `MUSIC_APP_INFO_ITEM_52.var[1] = 22097913` → 0x01512FF9
const MUSIC_ID_52: HashId = HashId(0x0151_2FF9);
/// `MUSIC_APP_INFO_ITEM_52.var[4] = 154823239` → 0x093A6A47
const ENTRY_ID_52: HashId = HashId(0x093A_6A47);
/// `MUSIC_APP_INFO_ITEM_102.var[1] = 117412874` → 0x06FF940A
const MUSIC_ID_102: HashId = HashId(0x06FF_940A);
/// `MUSIC_APP_INFO_ITEM_102.var[4] = 244634036` → 0x0E94D1B4
const ENTRY_ID_102: HashId = HashId(0x0E94_D1B4);

// ─── Fixture inline ───────────────────────────────────────────────────────────
//
// Extrait minimal couvrant les cas représentatifs :
// - item avec var[5] String (chemin présent)
// - item avec var[5] Int "0" (chemin absent)
// - item avec variant=1

fn fixture_music_app() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "MUSIC_APP_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    {
                        "name": "MUSIC_APP_INFO_0",
                        "variables": [{"type": "Int", "value": "1"}],
                        "children": [
                            {
                                "name": "MUSIC_APP_INFO_ITEM_LIST_BEG_0",
                                "variables": [{"type": "Int", "value": "2"}],
                                "children": [
                                    // MUSIC_APP_INFO_ITEM_0 — groupe 1, var[5] String
                                    {
                                        "name": "MUSIC_APP_INFO_ITEM_0",
                                        "variables": [
                                            {"type": "Int",    "value": "1"},
                                            {"type": "Int",    "value": "1653231064"},
                                            {"type": "Int",    "value": "2"},
                                            {"type": "Int",    "value": "0"},
                                            {"type": "Int",    "value": "1169637250"},
                                            {"type": "String", "value": "AAAAAA8FNbkZNtoAAQAyAAAnTHE="},
                                            {"type": "Int",    "value": "500"},
                                            {"type": "Int",    "value": "1"},
                                            {"type": "Int",    "value": "1"}
                                        ],
                                        "children": []
                                    },
                                    // MUSIC_APP_INFO_ITEM_1 — groupe 1
                                    {
                                        "name": "MUSIC_APP_INFO_ITEM_1",
                                        "variables": [
                                            {"type": "Int",    "value": "1"},
                                            {"type": "Int",    "value": "1653231064"},
                                            {"type": "Int",    "value": "9"},
                                            {"type": "Int",    "value": "0"},
                                            {"type": "Int",    "value": "1695668893"},
                                            {"type": "String", "value": "AAAAAA8FNbkZNtoAAQAyAAAnTHE="},
                                            {"type": "Int",    "value": "500"},
                                            {"type": "Int",    "value": "2"},
                                            {"type": "Int",    "value": "2"}
                                        ],
                                        "children": []
                                    }
                                ]
                            }
                        ]
                    },
                    {
                        "name": "MUSIC_APP_INFO_1",
                        "variables": [{"type": "Int", "value": "2"}],
                        "children": [
                            {
                                "name": "MUSIC_APP_INFO_ITEM_LIST_BEG_1",
                                "variables": [{"type": "Int", "value": "2"}],
                                "children": [
                                    // MUSIC_APP_INFO_ITEM_20 — groupe 2, var[5] absent (Int "0")
                                    {
                                        "name": "MUSIC_APP_INFO_ITEM_20",
                                        "variables": [
                                            {"type": "Int", "value": "2"},
                                            {"type": "Int", "value": "1009845833"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "878330871"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "500"},
                                            {"type": "Int", "value": "21"},
                                            {"type": "Int", "value": "21"}
                                        ],
                                        "children": []
                                    },
                                    // MUSIC_APP_INFO_ITEM_21 — groupe 2, variant=1
                                    {
                                        "name": "MUSIC_APP_INFO_ITEM_21",
                                        "variables": [
                                            {"type": "Int", "value": "2"},
                                            {"type": "Int", "value": "1009845833"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "1"},
                                            {"type": "Int", "value": "872040851"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "500"},
                                            {"type": "Int", "value": "22"},
                                            {"type": "Int", "value": "22"}
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

// ─── Tests sur fixture ────────────────────────────────────────────────────────

#[test]
fn fixture_compte_total() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // Fixture contient 4 items (2 par groupe × 2 groupes)
    assert_eq!(cfg.items.len(), 4, "fixture: 4 items");
}

#[test]
fn fixture_item_0_app_category() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_0.var[0] = 1 → app_category = 1
    assert_eq!(cfg.items[0].app_category, 1, "item[0].app_category = 1");
}

#[test]
fn fixture_item_0_music_id() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_0.var[1] = 1653231064 → 0x628A4DD8
    assert_eq!(
        cfg.items[0].music_id, MUSIC_ID_0,
        "item[0].music_id = 0x628A4DD8"
    );
}

#[test]
fn fixture_item_0_track_no() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_0.var[2] = 2
    assert_eq!(cfg.items[0].track_no, 2, "item[0].track_no = 2");
}

#[test]
fn fixture_item_0_variant() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_0.var[3] = 0
    assert_eq!(cfg.items[0].variant, 0, "item[0].variant = 0");
}

#[test]
fn fixture_item_0_entry_id() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_0.var[4] = 1169637250 → 0x45B73F82
    assert_eq!(
        cfg.items[0].entry_id, ENTRY_ID_0,
        "item[0].entry_id = 0x45B73F82"
    );
}

#[test]
fn fixture_item_0_path_b64_present() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_0.var[5] = String → has_path = true
    assert!(cfg.items[0].has_path(), "item[0].has_path = true");
    assert_eq!(
        cfg.items[0].path_b64, "AAAAAA8FNbkZNtoAAQAyAAAnTHE=",
        "item[0].path_b64"
    );
}

#[test]
fn fixture_item_0_volume() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_0.var[6] = 500
    assert_eq!(cfg.items[0].volume, 500, "item[0].volume = 500");
}

#[test]
fn fixture_item_0_sort_index() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_0.var[7] = 1
    assert_eq!(cfg.items[0].sort_index, 1, "item[0].sort_index = 1");
}

#[test]
fn fixture_item_1_entry_id_et_track_no() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_1.var[4] = 1695668893 → 0x6511DA9D, var[2] = 9
    assert_eq!(
        cfg.items[1].entry_id, ENTRY_ID_1,
        "item[1].entry_id = 0x6511DA9D"
    );
    assert_eq!(cfg.items[1].track_no, 9, "item[1].track_no = 9");
    assert_eq!(cfg.items[1].sort_index, 2, "item[1].sort_index = 2");
}

#[test]
fn fixture_item_2_path_absent() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_20 (index 2 dans la fixture) : var[5] = Int "0" → path absent
    assert_eq!(cfg.items[2].app_category, 2, "item[2].app_category = 2");
    assert!(!cfg.items[2].has_path(), "item[2].has_path = false");
    assert_eq!(cfg.items[2].path_b64, "", "item[2].path_b64 = vide");
    assert_eq!(
        cfg.items[2].entry_id, ENTRY_ID_20,
        "item[2].entry_id = 0x345A43F7"
    );
    assert_eq!(cfg.items[2].sort_index, 21, "item[2].sort_index = 21");
}

#[test]
fn fixture_item_3_variant_1() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // MUSIC_APP_INFO_ITEM_21 (index 3 dans la fixture) : var[3] = 1
    assert_eq!(cfg.items[3].variant, 1, "item[3].variant = 1");
    assert_eq!(cfg.items[3].sort_index, 22, "item[3].sort_index = 22");
}

#[test]
fn fixture_list_beg_ignores() {
    let cfg = parse_music_app_config(&fixture_music_app());
    // Les noeuds conteneurs _LIST_BEG_ et MUSIC_APP_INFO_N ne doivent pas être comptés
    assert_eq!(cfg.items.len(), 4, "LIST_BEG et groupes non comptés");
}

#[test]
fn fixture_find_by_entry_id() {
    let cfg = parse_music_app_config(&fixture_music_app());
    let found = cfg.find_by_entry_id(ENTRY_ID_0);
    assert!(found.is_some(), "find_by_entry_id(0x45B73F82)");
    assert_eq!(found.unwrap().music_id, MUSIC_ID_0);
    assert!(cfg.find_by_entry_id(HashId(0xDEADBEEF)).is_none());
}

#[test]
fn fixture_items_of_category() {
    let cfg = parse_music_app_config(&fixture_music_app());
    let cat1 = cfg.items_of_category(1);
    let cat2 = cfg.items_of_category(2);
    assert_eq!(cat1.len(), 2, "2 items de catégorie 1 dans la fixture");
    assert_eq!(cat2.len(), 2, "2 items de catégorie 2 dans la fixture");
    assert_eq!(
        cfg.items_of_category(3).len(),
        0,
        "0 items de catégorie 3 dans la fixture"
    );
}

// ─── Tests sur le fichier réel (skip silencieux si absent) ────────────────────

const REAL_MUSIC_APP: &str = "music_app/music_app_config.cfg.bin.json";

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
fn real_file_compte_total() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);
    // MUSIC_APP_INFO_LIST_BEG_0.var[0] = 4 groupes, 20+32+50+6 = 108 items
    assert_eq!(cfg.items.len(), 108, "music_app_config: 108 items");
}

#[test]
fn real_file_item_0_champs() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // MUSIC_APP_INFO_ITEM_0 (premier item, groupe 1)
    // var[0] = 1 → app_category
    assert_eq!(cfg.items[0].app_category, 1, "item[0].app_category = 1");
    // var[1] = 1653231064 → music_id = 0x628A4DD8
    assert_eq!(
        cfg.items[0].music_id, MUSIC_ID_0,
        "item[0].music_id = 0x628A4DD8"
    );
    // var[2] = 2
    assert_eq!(cfg.items[0].track_no, 2, "item[0].track_no = 2");
    // var[3] = 0
    assert_eq!(cfg.items[0].variant, 0, "item[0].variant = 0");
    // var[4] = 1169637250 → entry_id = 0x45B73F82
    assert_eq!(
        cfg.items[0].entry_id, ENTRY_ID_0,
        "item[0].entry_id = 0x45B73F82"
    );
    // var[5] = String présent
    assert!(cfg.items[0].has_path(), "item[0].has_path = true");
    assert_eq!(
        cfg.items[0].path_b64, "AAAAAA8FNbkZNtoAAQAyAAAnTHE=",
        "item[0].path_b64"
    );
    // var[6] = 500
    assert_eq!(cfg.items[0].volume, 500, "item[0].volume = 500");
    // var[7] = 1
    assert_eq!(cfg.items[0].sort_index, 1, "item[0].sort_index = 1");
}

#[test]
fn real_file_item_1_champs() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // MUSIC_APP_INFO_ITEM_1 (deuxième item du groupe 1)
    // var[1] = 1653231064 → même music_id que item_0
    assert_eq!(
        cfg.items[1].music_id, MUSIC_ID_0,
        "item[1].music_id = 0x628A4DD8"
    );
    // var[2] = 9 → track_no différent
    assert_eq!(cfg.items[1].track_no, 9, "item[1].track_no = 9");
    // var[4] = 1695668893 → entry_id = 0x6511DA9D
    assert_eq!(
        cfg.items[1].entry_id, ENTRY_ID_1,
        "item[1].entry_id = 0x6511DA9D"
    );
    // var[7] = 2
    assert_eq!(cfg.items[1].sort_index, 2, "item[1].sort_index = 2");
}

#[test]
fn real_file_item_20_path_absent() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // MUSIC_APP_INFO_ITEM_20 (premier item du groupe 2, index 20 dans la liste globale)
    // var[0] = 2 → app_category = 2
    assert_eq!(cfg.items[20].app_category, 2, "item[20].app_category = 2");
    // var[1] = 1009845833 → music_id = 0x3C310649
    assert_eq!(
        cfg.items[20].music_id, MUSIC_ID_20,
        "item[20].music_id = 0x3C310649"
    );
    // var[4] = 878330871 → entry_id = 0x345A43F7
    assert_eq!(
        cfg.items[20].entry_id, ENTRY_ID_20,
        "item[20].entry_id = 0x345A43F7"
    );
    // var[5] = Int "0" → chemin absent
    assert!(!cfg.items[20].has_path(), "item[20].has_path = false");
    assert_eq!(cfg.items[20].path_b64, "", "item[20].path_b64 = vide");
    // var[7] = 21
    assert_eq!(cfg.items[20].sort_index, 21, "item[20].sort_index = 21");
}

#[test]
fn real_file_item_21_variant() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // MUSIC_APP_INFO_ITEM_21 : seul item avec variant=1 dans le dump réel
    // var[3] = 1
    assert_eq!(cfg.items[21].variant, 1, "item[21].variant = 1");
    // var[7] = 22
    assert_eq!(cfg.items[21].sort_index, 22, "item[21].sort_index = 22");
}

#[test]
fn real_file_item_52_groupe_3() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // MUSIC_APP_INFO_ITEM_52 (premier item du groupe 3, index 52 global)
    // var[0] = 3 → app_category = 3
    assert_eq!(cfg.items[52].app_category, 3, "item[52].app_category = 3");
    // var[1] = 22097913 → music_id = 0x01512FF9
    assert_eq!(
        cfg.items[52].music_id, MUSIC_ID_52,
        "item[52].music_id = 0x01512FF9"
    );
    // var[4] = 154823239 → entry_id = 0x093A6A47
    assert_eq!(
        cfg.items[52].entry_id, ENTRY_ID_52,
        "item[52].entry_id = 0x093A6A47"
    );
    // var[5] = Int "0" → chemin absent
    assert!(!cfg.items[52].has_path(), "item[52].has_path = false");
    // var[7] = 53
    assert_eq!(cfg.items[52].sort_index, 53, "item[52].sort_index = 53");
}

#[test]
fn real_file_item_102_groupe_4() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // MUSIC_APP_INFO_ITEM_102 (premier item du groupe 4, index 102 global)
    // var[0] = 4 → app_category = 4
    assert_eq!(cfg.items[102].app_category, 4, "item[102].app_category = 4");
    // var[1] = 117412874 → music_id = 0x06FF940A
    assert_eq!(
        cfg.items[102].music_id, MUSIC_ID_102,
        "item[102].music_id = 0x06FF940A"
    );
    // var[4] = 244634036 → entry_id = 0x0E94D1B4
    assert_eq!(
        cfg.items[102].entry_id, ENTRY_ID_102,
        "item[102].entry_id = 0x0E94D1B4"
    );
    // var[7] = 103
    assert_eq!(cfg.items[102].sort_index, 103, "item[102].sort_index = 103");
}

#[test]
fn real_file_dernier_item() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // MUSIC_APP_INFO_ITEM_107 (dernier item, index 107, groupe 4)
    // var[0] = 4 → app_category = 4
    assert_eq!(cfg.items[107].app_category, 4, "item[107].app_category = 4");
    // var[7] = 108 → sort_index = 108 (dernier)
    assert_eq!(cfg.items[107].sort_index, 108, "item[107].sort_index = 108");
}

#[test]
fn real_file_comptes_par_categorie() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // Groupes réels : 20 + 32 + 50 + 6 = 108
    assert_eq!(cfg.items_of_category(1).len(), 20, "catégorie 1: 20 items");
    assert_eq!(cfg.items_of_category(2).len(), 32, "catégorie 2: 32 items");
    assert_eq!(cfg.items_of_category(3).len(), 50, "catégorie 3: 50 items");
    assert_eq!(cfg.items_of_category(4).len(), 6, "catégorie 4: 6 items");
}

#[test]
fn real_file_chemin_present_compte() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // 105 items ont un chemin, 3 n'en ont pas (items 20, 21, 52)
    let avec_chemin = cfg.items.iter().filter(|it| it.has_path()).count();
    let sans_chemin = cfg.items.iter().filter(|it| !it.has_path()).count();
    assert_eq!(avec_chemin, 105, "105 items avec chemin audio");
    assert_eq!(
        sans_chemin, 3,
        "3 items sans chemin audio (items 20, 21, 52)"
    );
}

#[test]
fn real_file_sort_index_sequentiel() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // Les sort_index doivent être 1..=108 dans l'ordre croissant
    for (i, item) in cfg.items.iter().enumerate() {
        assert_eq!(
            item.sort_index,
            (i + 1) as i64,
            "sort_index[{i}] = {}",
            i + 1
        );
    }
}

#[test]
fn real_file_volume_distribution() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // Volumes observés : 500 (59 items), 2000 (24), 5000 (19), 30000 (6)
    let v500 = cfg.items.iter().filter(|it| it.volume == 500).count();
    let v2000 = cfg.items.iter().filter(|it| it.volume == 2000).count();
    let v5000 = cfg.items.iter().filter(|it| it.volume == 5000).count();
    let v30000 = cfg.items.iter().filter(|it| it.volume == 30000).count();
    assert_eq!(v500, 59, "59 items avec volume=500");
    assert_eq!(v2000, 24, "24 items avec volume=2000");
    assert_eq!(v5000, 19, "19 items avec volume=5000");
    assert_eq!(v30000, 6, "6 items avec volume=30000");
    // item_0 a volume=500
    assert_eq!(cfg.items[0].volume, 500, "item[0].volume = 500");
}

#[test]
fn real_file_find_by_entry_id() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // Trouver l'item 0 via son entry_id = 0x45B73F82
    let found = cfg.find_by_entry_id(ENTRY_ID_0);
    assert!(found.is_some(), "find_by_entry_id(0x45B73F82)");
    let it = found.unwrap();
    assert_eq!(
        it.music_id, MUSIC_ID_0,
        "item trouvé: music_id = 0x628A4DD8"
    );
    assert_eq!(it.sort_index, 1, "item trouvé: sort_index = 1");

    // Identifiant absent → None
    assert!(cfg.find_by_entry_id(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn real_file_variants_distribution() {
    let Some(root) = load_json(REAL_MUSIC_APP) else {
        return;
    };
    let cfg = parse_music_app_config(&root);

    // 95 items avec variant=0, 13 avec variant=1 (distribution observée dans le dump réel)
    let v0 = cfg.items.iter().filter(|it| it.variant == 0).count();
    let v1 = cfg.items.iter().filter(|it| it.variant == 1).count();
    assert_eq!(v0, 95, "95 items avec variant=0");
    assert_eq!(v1, 13, "13 items avec variant=1");

    // Le premier item avec variant=1 est MUSIC_APP_INFO_ITEM_21 (sort_index=22)
    let first_v1 = cfg.items.iter().find(|it| it.variant == 1).unwrap();
    assert_eq!(
        first_v1.sort_index, 22,
        "premier item variant=1: sort_index = 22"
    );
}
