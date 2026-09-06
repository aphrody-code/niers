#![allow(clippy::pedantic)]
//! Tests golden `phase` — valeurs réelles tirées de :
//! - `data/common/gamedata/phase/phase_set_c21_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/phase/phase_set_c91_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/phase/phase_set_c97_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/phase/phase_set_c01_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/phase/phase_set_layout_c01_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/phase/phase_title_config_0.08.56.cfg.bin.json`
//!
//! ## Vérifications champ par champ
//!
//! ### phase_set_c21 : DATA_ITEM_0 (fichier JSON, entrée 2)
//! - var\[0\] = "21"  → `chapter_no` = 21
//! - var\[1\] = "10"  → `phase_no`   = 10
//! - var\[2\] = "0"   → `seq_index`  = 0
//! - var\[3\] = "0"   → `type_flag`  = 0
//! - var\[9\] = "0"   → `lua_path`   = "0" (pas de Lua)
//! - DATA_COUNT_0.var\[0\] = "1" → 1 DATA_ITEM
//! - PURPOSE_TEXT_COUNT_0.var\[0\] = "0" → 0 textes
//!
//! ### phase_set_c91 : PURPOSE_TEXT_ITEM_0
//! - var\[0\] = "1381738245" → `text_hash` = `HashId(0x525BA705)`
//!   (1381738245 & 0xFFFF_FFFF = 0x525BA705, vérifié Python)
//! - var\[1\] = "0" → `lua_data` = "0"
//!
//! ### phase_set_c97 : trois DATA_ITEM
//! - DATA_COUNT_0.var\[0\] = "3"
//! - DATA_ITEM_0 : chapter=97, phase=10, seq=0, flag=0
//! - DATA_ITEM_1 : chapter=97, phase=20, seq=0, flag=0
//! - DATA_ITEM_2 : chapter=97, phase=30, seq=0, flag=0
//!
//! ### phase_set_c01 : counts
//! - DATA_COUNT_0.var\[0\]          = "19" → 19 DATA_ITEM
//! - PURPOSE_TEXT_COUNT_0.var\[0\]  = "27" → 27 PURPOSE_TEXT_ITEM
//! - PURPOSE_POS_COUNT_0.var\[0\]   = "38" → 38 PURPOSE_POS_ITEM
//! - DATA_ITEM_0 : chapter=1, phase=10, seq=0, flag=1, lua_path non-"0"
//!
//! ### phase_set_layout_c01 : LAYOUT_BASE_LIST_BEG_0
//! - count = 3 bases
//! - LAYOUT_BASE_0.var\[0\]       = "1339075865"
//! - LAYOUT_BASE_SHAPE_0.var\[*\] = [0, 0, 0, 0]
//! - LAYOUT_BASE_POINT_0.var\[*\] = ["22,31", "1,25", "-21,01", "0"]
//!   → point[0]=22.31, point[1]=1.25, point[2]=-21.01, point[3]=0.0
//!
//! ### phase_title_config : PHASE_TITLE_TEX_INFO_0
//! - var\[0\] = "-16726348"  → `hash_a` = `HashId(0xFF00C6B4)`
//!   (-16726348 & 0xFFFF_FFFF = 0xFF00C6B4, vérifié Python)
//! - var\[1\] = "667504474"  → `hash_b` = `HashId(0x27C94F5A)`
//! - var\[2\] = "#/menu/220_img/chapter_title/<LG>/chapter01.g4tx"
//! ### phase_title_config : PHASE_TITLE_CFG_0 + PHASE_TITLE_CFG_REF_TEX_INFO_0
//! - PHASE_TITLE_CFG_0.var\[0\] = "1"         → `chapter_no` = 1
//! - PHASE_TITLE_CFG_0.var\[1\] = "185538439" → `title_hash` = `HashId(0x0B0F1787)`
//!   (185538439 & 0xFFFF_FFFF = 0x0B0F1787)
//! - PHASE_TITLE_CFG_REF_TEX_INFO_0.var\[0\] = "0" → `tex_index` = 0

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::phase::{
    PhaseDataItem, PurposePosItem, PurposeTextItem, parse_phase_set, parse_phase_set_layout,
    parse_phase_title_config,
};
use serde_json::json;

// ─── Constantes golden (vérifiées Python : v & 0xFFFF_FFFF) ──────────────────

/// 1381738245 & 0xFFFF_FFFF = 0x525BA705
const TEXT_HASH_C91_0: HashId = HashId(0x525B_A705);
/// -16726348 & 0xFFFF_FFFF = 0xFF00C6B4
const TEX_HASH_A_0: HashId = HashId(0xFF00_C6B4);
/// 667504474 & 0xFFFF_FFFF = 0x27C94F5A
const TEX_HASH_B_0: HashId = HashId(0x27C9_4F5A);
/// 185538439 & 0xFFFF_FFFF = 0x0B0F1787
const TITLE_HASH_C01: HashId = HashId(0x0B0F_1787);

// ─── Helpers ─────────────────────────────────────────────────────────────────

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

// ─── Fixtures inline ─────────────────────────────────────────────────────────

/// Extrait minimal de `phase_set_c21_0.00.00.cfg.bin.json` (entrée DATA_ITEM_0).
fn fixture_c21() -> serde_json::Value {
    json!({
        "entries": [
            // DATA_COUNT_0 — c21 JSON valeur "1"
            {
                "name": "DATA_COUNT_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": []
            },
            // DATA_ITEM_0 — c21 : chapter=21, phase=10, seq=0, flag=0, lua_path="0"
            {
                "name": "DATA_ITEM_0",
                "variables": [
                    {"type": "Int", "value": "21"},  // var[0] chapter_no
                    {"type": "Int", "value": "10"},  // var[1] phase_no
                    {"type": "Int", "value": "0"},   // var[2] seq_index
                    {"type": "Int", "value": "0"},   // var[3] type_flag
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "0"},   // var[7] raw_7
                    {"type": "Int", "value": "0"},   // var[8] raw_8
                    {"type": "Int", "value": "0"},   // var[9] lua_path = "0"
                    {"type": "Int", "value": "0"}
                ],
                "children": []
            },
            // PURPOSE_TEXT_COUNT_0 — 0 textes
            {
                "name": "PURPOSE_TEXT_COUNT_0",
                "variables": [{"type": "Int", "value": "0"}],
                "children": []
            },
            // PURPOSE_SUB_TEXT_COUNT_0 — 0
            {
                "name": "PURPOSE_SUB_TEXT_COUNT_0",
                "variables": [{"type": "Int", "value": "0"}],
                "children": []
            },
            // PURPOSE_POS_COUNT_0 — 0 positions
            {
                "name": "PURPOSE_POS_COUNT_0",
                "variables": [{"type": "Int", "value": "0"}],
                "children": []
            }
        ]
    })
}

/// Extrait minimal de `phase_set_c91_0.00.00.cfg.bin.json` (1 texte d'objectif).
fn fixture_c91() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "DATA_COUNT_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": []
            },
            {
                "name": "DATA_ITEM_0",
                "variables": [
                    {"type": "Int", "value": "91"},
                    {"type": "Int", "value": "10"},
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "1"},
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "0"}
                ],
                "children": []
            },
            {
                "name": "PURPOSE_TEXT_COUNT_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": []
            },
            // PURPOSE_TEXT_ITEM_0 — c91 : text_hash=1381738245, lua_data="0"
            {
                "name": "PURPOSE_TEXT_ITEM_0",
                "variables": [
                    {"type": "Int", "value": "1381738245"},  // var[0] text_hash → 0x525BA705
                    {"type": "Int", "value": "0"}            // var[1] lua_data
                ],
                "children": []
            },
            {
                "name": "PURPOSE_SUB_TEXT_COUNT_0",
                "variables": [{"type": "Int", "value": "0"}],
                "children": []
            },
            {
                "name": "PURPOSE_POS_COUNT_0",
                "variables": [{"type": "Int", "value": "0"}],
                "children": []
            }
        ]
    })
}

/// Extrait minimal de `phase_set_layout_c01_0.00.00.cfg.bin.json` (3 bases, base 0 seulement).
fn fixture_layout_c01() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "LAYOUT_BASE_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    // LAYOUT_BASE_0 — hash = 1339075865
                    {
                        "name": "LAYOUT_BASE_0",
                        "variables": [{"type": "Int", "value": "1339075865"}],
                        "children": []
                    },
                    // LAYOUT_BASE_SHAPE_0 — [0, 0, 0, 0] dans le fichier réel
                    {
                        "name": "LAYOUT_BASE_SHAPE_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    // LAYOUT_BASE_POINT_0 — ["22,31", "1,25", "-21,01", "0"]
                    {
                        "name": "LAYOUT_BASE_POINT_0",
                        "variables": [
                            {"type": "Float", "value": "22,31"},
                            {"type": "Float", "value": "1,25"},
                            {"type": "Float", "value": "-21,01"},
                            {"type": "Int",   "value": "0"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

/// Extrait minimal de `phase_title_config_0.08.56.cfg.bin.json` (1 entrée de chaque liste).
fn fixture_title_config() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "PHASE_TITLE_TEX_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    // PHASE_TITLE_TEX_INFO_0 — hash_a=-16726348, hash_b=667504474, tex_path
                    {
                        "name": "PHASE_TITLE_TEX_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "-16726348"},
                            {"type": "Int", "value": "667504474"},
                            {"type": "String", "value": "#/menu/220_img/chapter_title/<LG>/chapter01.g4tx"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "PHASE_TITLE_CFG_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    // PHASE_TITLE_CFG_0 — chapter_no=1, title_hash=185538439
                    {
                        "name": "PHASE_TITLE_CFG_0",
                        "variables": [
                            {"type": "Int", "value": "1"},
                            {"type": "Int", "value": "185538439"}
                        ],
                        "children": []
                    },
                    // PHASE_TITLE_CFG_REF_TEX_INFO_0 — tex_index=0, ref_count=1
                    {
                        "name": "PHASE_TITLE_CFG_REF_TEX_INFO_0",
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

// ─── Tests sur fixture : parse_phase_set ─────────────────────────────────────

#[test]
fn fixture_c21_compte_items() {
    let ps = parse_phase_set(&fixture_c21());
    // c21 : 1 DATA_ITEM, 0 PURPOSE_TEXT_ITEM, 0 PURPOSE_POS_ITEM
    assert_eq!(ps.items.len(), 1, "c21 : 1 DATA_ITEM");
    assert_eq!(ps.purpose_texts.len(), 0, "c21 : 0 PURPOSE_TEXT_ITEM");
    assert_eq!(ps.purpose_positions.len(), 0, "c21 : 0 PURPOSE_POS_ITEM");
}

#[test]
fn fixture_c21_data_item_0_champs() {
    let ps = parse_phase_set(&fixture_c21());
    let item: &PhaseDataItem = &ps.items[0];

    // phase_set_c21 DATA_ITEM_0 : var[0]="21", var[1]="10", var[2]="0", var[3]="0"
    assert_eq!(item.chapter_no, 21, "chapter_no = 21");
    assert_eq!(item.phase_no, 10, "phase_no = 10");
    assert_eq!(item.seq_index, 0, "seq_index = 0");
    assert_eq!(item.type_flag, 0, "type_flag = 0");
    assert_eq!(item.raw_7, 0, "raw_7 = 0");
    assert_eq!(item.raw_8, 0, "raw_8 = 0");
    assert_eq!(item.lua_path, "0", "lua_path = \"0\"");
    assert!(!item.has_lua(), "pas de lua pour c21");
}

#[test]
fn fixture_c91_purpose_text_0() {
    let ps = parse_phase_set(&fixture_c91());
    assert_eq!(ps.purpose_texts.len(), 1, "c91 : 1 PURPOSE_TEXT_ITEM");

    let text: &PurposeTextItem = &ps.purpose_texts[0];
    // phase_set_c91 PURPOSE_TEXT_ITEM_0 : var[0]=1381738245 → 0x525BA705
    assert_eq!(
        text.text_hash, TEXT_HASH_C91_0,
        "text_hash = HashId(0x525BA705)"
    );
    assert_eq!(text.lua_data, "0", "lua_data = \"0\"");
}

// ─── Tests sur fixture : parse_phase_set_layout ───────────────────────────────

#[test]
fn fixture_layout_c01_compte_bases() {
    let layout = parse_phase_set_layout(&fixture_layout_c01());
    assert_eq!(layout.bases.len(), 1, "fixture : 1 base");
}

#[test]
fn fixture_layout_c01_base_0_hash() {
    let layout = parse_phase_set_layout(&fixture_layout_c01());
    // LAYOUT_BASE_0.var[0] = 1339075865
    assert_eq!(
        layout.bases[0].base_hash,
        HashId::from_i64(1339075865),
        "base_hash[0] = 1339075865"
    );
}

#[test]
fn fixture_layout_c01_base_0_shape() {
    let layout = parse_phase_set_layout(&fixture_layout_c01());
    // LAYOUT_BASE_SHAPE_0 = [0, 0, 0, 0] dans le fichier réel
    assert_eq!(layout.bases[0].shape, [0, 0, 0, 0], "shape = [0, 0, 0, 0]");
}

#[test]
fn fixture_layout_c01_base_0_point() {
    let layout = parse_phase_set_layout(&fixture_layout_c01());
    // LAYOUT_BASE_POINT_0 = ["22,31", "1,25", "-21,01", "0"]
    let pt = layout.bases[0].point;
    assert!(
        (pt[0] - 22.31_f64).abs() < 1e-9,
        "point[0] ≈ 22.31 (réel: {})",
        pt[0]
    );
    assert!(
        (pt[1] - 1.25_f64).abs() < 1e-9,
        "point[1] ≈ 1.25 (réel: {})",
        pt[1]
    );
    assert!(
        (pt[2] - (-21.01_f64)).abs() < 1e-9,
        "point[2] ≈ -21.01 (réel: {})",
        pt[2]
    );
    assert_eq!(pt[3], 0.0_f64, "point[3] = 0.0");
}

// ─── Tests sur fixture : parse_phase_title_config ────────────────────────────

#[test]
fn fixture_title_config_comptes() {
    let tc = parse_phase_title_config(&fixture_title_config());
    assert_eq!(tc.tex_infos.len(), 1, "fixture : 1 PhaseTitleTexInfo");
    assert_eq!(tc.cfgs.len(), 1, "fixture : 1 PhaseTitleCfg");
}

#[test]
fn fixture_title_config_tex_info_0() {
    let tc = parse_phase_title_config(&fixture_title_config());
    // PHASE_TITLE_TEX_INFO_0 : var[0]=-16726348, var[1]=667504474, var[2]=path
    assert_eq!(tc.tex_infos[0].hash_a, TEX_HASH_A_0, "hash_a = 0xFF00C6B4");
    assert_eq!(tc.tex_infos[0].hash_b, TEX_HASH_B_0, "hash_b = 0x27C94F5A");
    assert!(
        tc.tex_infos[0].tex_path.contains("chapter01.g4tx"),
        "tex_path contient chapter01.g4tx"
    );
}

#[test]
fn fixture_title_config_cfg_0() {
    let tc = parse_phase_title_config(&fixture_title_config());
    // PHASE_TITLE_CFG_0 : chapter_no=1, title_hash=185538439 → 0x0B0F1787
    // PHASE_TITLE_CFG_REF_TEX_INFO_0 : tex_index=0
    assert_eq!(tc.cfgs[0].chapter_no, 1, "chapter_no = 1");
    assert_eq!(
        tc.cfgs[0].title_hash, TITLE_HASH_C01,
        "title_hash = HashId(0x0B0F1787)"
    );
    assert_eq!(tc.cfgs[0].tex_index, 0, "tex_index = 0");
}

#[test]
fn fixture_title_config_find_chapter() {
    let tc = parse_phase_title_config(&fixture_title_config());
    assert!(tc.find_chapter(1).is_some(), "find_chapter(1) trouvé");
    assert!(tc.find_chapter(99).is_none(), "find_chapter(99) absent");
}

// ─── Chemins des fichiers réels ───────────────────────────────────────────────

const REAL_C21: &str = "phase/phase_set_c21_0.00.00.cfg.bin.json";
const REAL_C91: &str = "phase/phase_set_c91_0.00.00.cfg.bin.json";
const REAL_C97: &str = "phase/phase_set_c97_0.00.00.cfg.bin.json";
const REAL_C01: &str = "phase/phase_set_c01_0.00.00.cfg.bin.json";
const REAL_LAYOUT_C01: &str = "phase/phase_set_layout_c01_0.00.00.cfg.bin.json";
const REAL_TITLE_CFG: &str = "phase/phase_title_config_0.08.56.cfg.bin.json";

// ─── Tests sur fichiers réels (skip silencieux si absents) ────────────────────

// — phase_set_c21 —

#[test]
fn real_file_c21_compte() {
    let Some(root) = load_json(REAL_C21) else {
        return;
    };
    let ps = parse_phase_set(&root);
    // phase_set_c21 : DATA_COUNT_0.var[0] = "1"
    assert_eq!(ps.items.len(), 1, "c21 : 1 DATA_ITEM");
    assert_eq!(ps.purpose_texts.len(), 0, "c21 : 0 textes");
    assert_eq!(ps.purpose_positions.len(), 0, "c21 : 0 positions");
}

#[test]
fn real_file_c21_data_item_0() {
    let Some(root) = load_json(REAL_C21) else {
        return;
    };
    let ps = parse_phase_set(&root);

    // phase_set_c21 DATA_ITEM_0 : chapter=21, phase=10, seq=0, flag=0, lua="0"
    assert_eq!(ps.items[0].chapter_no, 21, "chapter_no = 21");
    assert_eq!(ps.items[0].phase_no, 10, "phase_no = 10");
    assert_eq!(ps.items[0].seq_index, 0, "seq_index = 0");
    assert_eq!(ps.items[0].type_flag, 0, "type_flag = 0");
    assert_eq!(ps.items[0].lua_path, "0", "lua_path = \"0\"");
    assert!(!ps.items[0].has_lua(), "c21 n'a pas de Lua");
}

// — phase_set_c91 —

#[test]
fn real_file_c91_purpose_text_0() {
    let Some(root) = load_json(REAL_C91) else {
        return;
    };
    let ps = parse_phase_set(&root);

    // phase_set_c91 : PURPOSE_TEXT_COUNT_0.var[0] = "1"
    assert_eq!(ps.purpose_texts.len(), 1, "c91 : 1 PURPOSE_TEXT_ITEM");

    // PURPOSE_TEXT_ITEM_0.var[0] = "1381738245" → HashId(0x525BA705)
    assert_eq!(
        ps.purpose_texts[0].text_hash, TEXT_HASH_C91_0,
        "text_hash[0] = HashId(0x525BA705)"
    );
    assert_eq!(ps.purpose_texts[0].lua_data, "0", "lua_data[0] = \"0\"");
}

// — phase_set_c97 —

#[test]
fn real_file_c97_compte() {
    let Some(root) = load_json(REAL_C97) else {
        return;
    };
    let ps = parse_phase_set(&root);

    // phase_set_c97 : DATA_COUNT_0.var[0] = "3"
    assert_eq!(ps.items.len(), 3, "c97 : 3 DATA_ITEM");
    assert_eq!(ps.purpose_texts.len(), 0, "c97 : 0 textes");
    assert_eq!(ps.purpose_positions.len(), 0, "c97 : 0 positions");
}

#[test]
fn real_file_c97_data_items_chapter() {
    let Some(root) = load_json(REAL_C97) else {
        return;
    };
    let ps = parse_phase_set(&root);

    // Tous les DATA_ITEM ont chapter_no = 97
    for (i, item) in ps.items.iter().enumerate() {
        assert_eq!(item.chapter_no, 97, "item[{i}].chapter_no = 97");
    }
    // Phases : 10, 20, 30
    assert_eq!(ps.items[0].phase_no, 10, "item[0].phase_no = 10");
    assert_eq!(ps.items[1].phase_no, 20, "item[1].phase_no = 20");
    assert_eq!(ps.items[2].phase_no, 30, "item[2].phase_no = 30");
}

// — phase_set_c01 —

#[test]
fn real_file_c01_comptes() {
    let Some(root) = load_json(REAL_C01) else {
        return;
    };
    let ps = parse_phase_set(&root);

    // phase_set_c01 : DATA_COUNT_0=19, PURPOSE_TEXT_COUNT_0=27, PURPOSE_POS_COUNT_0=38
    assert_eq!(ps.items.len(), 19, "c01 : 19 DATA_ITEM");
    assert_eq!(ps.purpose_texts.len(), 27, "c01 : 27 PURPOSE_TEXT_ITEM");
    assert_eq!(ps.purpose_positions.len(), 38, "c01 : 38 PURPOSE_POS_ITEM");
}

#[test]
fn real_file_c01_data_item_0() {
    let Some(root) = load_json(REAL_C01) else {
        return;
    };
    let ps = parse_phase_set(&root);

    // DATA_ITEM_0 : chapter=1, phase=10, seq=0, flag=1
    assert_eq!(ps.items[0].chapter_no, 1, "chapter_no = 1");
    assert_eq!(ps.items[0].phase_no, 10, "phase_no = 10");
    assert_eq!(ps.items[0].seq_index, 0, "seq_index = 0");
    assert_eq!(ps.items[0].type_flag, 1, "type_flag = 1");
    assert!(ps.items[0].has_lua(), "c01 DATA_ITEM_0 a un Lua");
}

#[test]
fn real_file_c01_tous_chapter_1() {
    let Some(root) = load_json(REAL_C01) else {
        return;
    };
    let ps = parse_phase_set(&root);
    for (i, item) in ps.items.iter().enumerate() {
        assert_eq!(item.chapter_no, 1, "item[{i}].chapter_no = 1");
    }
}

#[test]
fn real_file_c01_phase_pos_item_0() {
    let Some(root) = load_json(REAL_C01) else {
        return;
    };
    let ps = parse_phase_set(&root);

    // PURPOSE_POS_ITEM_0 : pos_id non-nul, 9 variables
    let pos: &PurposePosItem = &ps.purpose_positions[0];
    assert!(!pos.pos_id.is_zero(), "pos_id[0] non-nul");
    assert!(!pos.target_hash.is_zero(), "target_hash[0] non-nul");
}

// — phase_set_layout_c01 —

#[test]
fn real_file_layout_c01_comptes() {
    let Some(root) = load_json(REAL_LAYOUT_C01) else {
        return;
    };
    let layout = parse_phase_set_layout(&root);

    // LAYOUT_BASE_LIST_BEG_0.var[0] = 3
    assert_eq!(layout.bases.len(), 3, "c01 layout : 3 bases");
}

#[test]
fn real_file_layout_c01_base_0_hash() {
    let Some(root) = load_json(REAL_LAYOUT_C01) else {
        return;
    };
    let layout = parse_phase_set_layout(&root);

    // LAYOUT_BASE_0.var[0] = 1339075865
    assert_eq!(
        layout.bases[0].base_hash,
        HashId::from_i64(1339075865),
        "bases[0].base_hash = 1339075865"
    );
}

#[test]
fn real_file_layout_c01_base_0_point() {
    let Some(root) = load_json(REAL_LAYOUT_C01) else {
        return;
    };
    let layout = parse_phase_set_layout(&root);

    // LAYOUT_BASE_POINT_0 = ["22,31", "1,25", "-21,01", "0"]
    let pt = layout.bases[0].point;
    assert!(
        (pt[0] - 22.31_f64).abs() < 1e-9,
        "point[0] ≈ 22.31 (réel: {})",
        pt[0]
    );
    assert!(
        (pt[1] - 1.25_f64).abs() < 1e-9,
        "point[1] ≈ 1.25 (réel: {})",
        pt[1]
    );
    assert!(
        (pt[2] - (-21.01_f64)).abs() < 1e-9,
        "point[2] ≈ -21.01 (réel: {})",
        pt[2]
    );
    assert_eq!(pt[3], 0.0_f64, "point[3] = 0.0");
}

#[test]
fn real_file_layout_c01_shape_tous_zeros() {
    let Some(root) = load_json(REAL_LAYOUT_C01) else {
        return;
    };
    let layout = parse_phase_set_layout(&root);

    // LAYOUT_BASE_SHAPE_N = [0, 0, 0, 0] dans tous les fichiers réels
    for (i, base) in layout.bases.iter().enumerate() {
        assert_eq!(base.shape, [0, 0, 0, 0], "base[{i}].shape = [0, 0, 0, 0]");
    }
}

// — phase_title_config —

#[test]
fn real_file_title_config_comptes() {
    let Some(root) = load_json(REAL_TITLE_CFG) else {
        return;
    };
    let tc = parse_phase_title_config(&root);

    // PHASE_TITLE_TEX_INFO_LIST_BEG_0.var[0] = 9
    assert_eq!(tc.tex_infos.len(), 9, "9 PhaseTitleTexInfo");
    // PHASE_TITLE_CFG_LIST_BEG_0.var[0] = 9 (18 enfants = 9 paires)
    assert_eq!(tc.cfgs.len(), 9, "9 PhaseTitleCfg");
}

#[test]
fn real_file_title_config_tex_info_0() {
    let Some(root) = load_json(REAL_TITLE_CFG) else {
        return;
    };
    let tc = parse_phase_title_config(&root);

    // PHASE_TITLE_TEX_INFO_0 : var[0]=-16726348, var[1]=667504474
    assert_eq!(tc.tex_infos[0].hash_a, TEX_HASH_A_0, "hash_a = 0xFF00C6B4");
    assert_eq!(tc.tex_infos[0].hash_b, TEX_HASH_B_0, "hash_b = 0x27C94F5A");
    assert!(
        tc.tex_infos[0].tex_path.contains("chapter01.g4tx"),
        "tex_path contient chapter01.g4tx"
    );
    assert!(
        tc.tex_infos[0].tex_path.contains("<LG>"),
        "tex_path contient <LG>"
    );
}

#[test]
fn real_file_title_config_cfg_0() {
    let Some(root) = load_json(REAL_TITLE_CFG) else {
        return;
    };
    let tc = parse_phase_title_config(&root);

    // PHASE_TITLE_CFG_0 : chapter_no=1, title_hash=185538439 → 0x0B0F1787
    // PHASE_TITLE_CFG_REF_TEX_INFO_0 : tex_index=0
    assert_eq!(tc.cfgs[0].chapter_no, 1, "chapter_no = 1");
    assert_eq!(
        tc.cfgs[0].title_hash, TITLE_HASH_C01,
        "title_hash = HashId(0x0B0F1787)"
    );
    assert_eq!(tc.cfgs[0].tex_index, 0, "tex_index = 0");
}

#[test]
fn real_file_title_config_chapters_1_a_9() {
    let Some(root) = load_json(REAL_TITLE_CFG) else {
        return;
    };
    let tc = parse_phase_title_config(&root);

    // Les 9 configs couvrent les chapitres 1 à 9 (dans l'ordre)
    for (i, cfg) in tc.cfgs.iter().enumerate() {
        let expected = (i + 1) as i64;
        assert_eq!(
            cfg.chapter_no, expected,
            "cfgs[{i}].chapter_no = {expected}"
        );
    }
}

#[test]
fn real_file_title_config_find_chapter_9() {
    let Some(root) = load_json(REAL_TITLE_CFG) else {
        return;
    };
    let tc = parse_phase_title_config(&root);

    // PHASE_TITLE_CFG_8 : var[0]="9", var[1]="97820597" → HashId(0x05D49FB5)
    let cfg9 = tc.find_chapter(9).expect("chapitre 9 trouvé");
    assert_eq!(cfg9.chapter_no, 9);
    assert_eq!(
        cfg9.title_hash,
        HashId::from_i64(97820597),
        "title_hash chapitre 9 = 97820597"
    );
}
