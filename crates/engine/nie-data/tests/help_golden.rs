#![allow(clippy::pedantic)]
//! Tests golden `help` — valeurs réelles tirées de :
//! - `data/common/gamedata/help/help_list_config_1.04.08.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ (JSON réel → valeur confirmée)
//!
//! Comptes globaux : 322 `HELP_LIST_IMAGE`, 245 `HELP_LIST_INFO`,
//! 245 `HELP_LIST_INFO_REF_IMAGE` (somme des count = 322), 216
//! `HELP_OPERATION_GUIDE_LAYOUT`, 21 `HELP_OPERATION_GUIDE_INFO`, 245 `__SORT_INDEX`.
//!
//! ### HELP_LIST_IMAGE_0
//! - var\[0\] = "hlp_controls.g4tx" ; var\[1\] = 324268305 → 0x1353F111
//! - var\[2\] = 1 (sort_no) ; var\[3\] = 559297902 → 0x2156356E ; var\[4\] = 660902250 → 0x2764916A
//!
//! ### HELP_LIST_IMAGE_1 / _321
//! - IMAGE_1.var\[1\] = -1973772117 → 0x8A5AA0AB ; sort_no = 2
//! - IMAGE_321.var\[0\] = "hlp_7140.g4tx" ; var\[1\] = -570166617 → 0xD9FEF6A7
//!
//! ### HELP_LIST_INFO_0 (apparié à REF_IMAGE_0 = [0,0])
//! - var\[0\] = 714921031 → 0x2A9CD447 (help_id) ; var\[2\] = 1 (display_no)
//! - var\[7\] = 245697788 → 0x0EA50CFC ; var\[8\] = 868558156 → 0x33C5254C
//! - var\[13\] = 398343613 → 0x17BE3DBD ; var\[14\] = 719197197 → 0x2ADE140D
//!
//! ### HELP_LIST_INFO_13 (première entrée à blob, apparié à REF_IMAGE_13 = [0,4])
//! - var\[0\] = → 0xCB28800F ; var\[2\] = 14 (display_no)
//! - var\[12\] = String "AAAAAA8FNWum6wYAAQAyAAAAAHg=" (blob base64 opaque)
//! - image_offset = 0, image_count = 4
//!
//! ### HELP_LIST_INFO_244 (dernière)
//! - var\[0\] = → 0x4EFD9BC9 ; var\[1\] = 3 (flag) ; var\[2\] = 250 (display_no)
//! - var\[3\] = → 0x1EAB3007 ; var\[4\] = → 0x2A54D7AF ; var\[5\] = → 0x478BDBD1
//!
//! ### HELP_OPERATION_GUIDE_LAYOUT_0 / _215
//! - LAYOUT_0 : page_no=1, row_no=2, flag=1, button=0xF2DD6185, sub=0xF52C33F5, coord=(-859, 247)
//! - LAYOUT_215 : page_no=1, row_no=1, flag=0, button=0x07B7FE51, sub=0xC28C1C34, coord=(305, -187)
//!
//! ### HELP_OPERATION_GUIDE_INFO_0 / _20 (appariés à REF_LAYOUT)
//! - INFO_0 : guide_id = 0x2156356E (= IMAGE_0.var\[3\]), text = 0x74C5F394, layout=[0,18]
//! - INFO_20 : guide_id = 0x73C7EE5E, text = 0x686E4428, layout=[207,9]
//!
//! ### __SORT_INDEX (permutation d'affichage, 245 indices)
//! - premiers 8 : [11, 234, 1, 243, 111, 159, 132, 173]

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::help::parse_help_list_config;
use serde_json::json;

// ─── Constantes golden (lues octet-pour-octet dans le JSON réel) ──────────────────

const IMG0_SPRITE: HashId = HashId(0x1353_F111);
const IMG0_GUIDE: HashId = HashId(0x2156_356E);
const IMG0_SUB: HashId = HashId(0x2764_916A);
const IMG1_SPRITE: HashId = HashId(0x8A5A_A0AB);
const IMG321_SPRITE: HashId = HashId(0xD9FE_F6A7);

const INFO0_HELP_ID: HashId = HashId(0x2A9C_D447);
const INFO0_RAW7: HashId = HashId(0x0EA5_0CFC);
const INFO0_RAW8: HashId = HashId(0x33C5_254C);
const INFO0_RAW13: HashId = HashId(0x17BE_3DBD);
const INFO0_RAW14: HashId = HashId(0x2ADE_140D);

const INFO13_HELP_ID: HashId = HashId(0xCB28_800F);
const INFO13_BLOB: &str = "AAAAAA8FNWum6wYAAQAyAAAAAHg=";

const INFO244_HELP_ID: HashId = HashId(0x4EFD_9BC9);
const INFO244_RAW3: HashId = HashId(0x1EAB_3007);
const INFO244_RAW4: HashId = HashId(0x2A54_D7AF);
const INFO244_RAW5: HashId = HashId(0x478B_DBD1);

const LAY0_BUTTON: HashId = HashId(0xF2DD_6185);
const LAY0_SUB: HashId = HashId(0xF52C_33F5);
const LAY215_BUTTON: HashId = HashId(0x07B7_FE51);
const LAY215_SUB: HashId = HashId(0xC28C_1C34);

const GI0_GUIDE_ID: HashId = HashId(0x2156_356E);
const GI0_TEXT: HashId = HashId(0x74C5_F394);
const GI20_GUIDE_ID: HashId = HashId(0x73C7_EE5E);
const GI20_TEXT: HashId = HashId(0x686E_4428);

// ─── Fixture inline ──────────────────────────────────────────────────────────────

/// Fixture minimale reproduisant l'arrangement réel : liste IMAGE globale, liste
/// INFO avec INFO_N / REF_IMAGE_N interleavés, et sous-arbre GUIDE (LAYOUT + INFO/REF).
/// Valeurs extraites octet-pour-octet du fichier JSON réel.
fn fixture_help() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "HELP_LIST_IMAGE_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    {
                        "name": "HELP_LIST_IMAGE_0",
                        "variables": [
                            {"type": "String", "value": "hlp_controls.g4tx"},
                            {"type": "Int", "value": "324268305"},
                            {"type": "Int", "value": "1"},
                            {"type": "Int", "value": "559297902"},
                            {"type": "Int", "value": "660902250"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    {
                        "name": "HELP_LIST_IMAGE_1",
                        "variables": [
                            {"type": "String", "value": "hlp_controls.g4tx"},
                            {"type": "Int", "value": "-1973772117"},
                            {"type": "Int", "value": "2"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "HELP_LIST_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}, {"type": "Int", "value": "1"}],
                "children": [
                    {
                        "name": "HELP_LIST_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "714921031"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "1"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "245697788"},
                            {"type": "Int", "value": "868558156"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"},
                            {"type": "String", "value": "AAAAAA8FNWum6wYAAQAyAAAAAHg="},
                            {"type": "Int", "value": "398343613"},
                            {"type": "Int", "value": "719197197"}
                        ],
                        "children": []
                    },
                    {
                        "name": "HELP_LIST_INFO_REF_IMAGE_0",
                        "variables": [{"type": "Int", "value": "0"}, {"type": "Int", "value": "2"}],
                        "children": [
                            {
                                "name": "HELP_OPERATION_GUIDE_LAYOUT_LIST_BEG_0",
                                "variables": [{"type": "Int", "value": "1"}],
                                "children": [
                                    {
                                        "name": "HELP_OPERATION_GUIDE_LAYOUT_0",
                                        "variables": [
                                            {"type": "Int", "value": "1"},
                                            {"type": "Int", "value": "2"},
                                            {"type": "Int", "value": "1"},
                                            {"type": "Int", "value": "-220372603"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "-181652491"},
                                            {"type": "Int", "value": "0"},
                                            {"type": "Int", "value": "-859"},
                                            {"type": "Int", "value": "247"}
                                        ],
                                        "children": [
                                            {
                                                "name": "HELP_OPERATION_GUIDE_INFO_LIST_BEG_0",
                                                "variables": [{"type": "Int", "value": "1"}],
                                                "children": [
                                                    {
                                                        "name": "HELP_OPERATION_GUIDE_INFO_0",
                                                        "variables": [
                                                            {"type": "Int", "value": "559297902"},
                                                            {"type": "Int", "value": "1959130004"}
                                                        ],
                                                        "children": []
                                                    },
                                                    {
                                                        "name": "HELP_OPERATION_GUIDE_INFO_REF_LAYOUT_0",
                                                        "variables": [
                                                            {"type": "Int", "value": "0"},
                                                            {"type": "Int", "value": "1"}
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
    })
}

// ─── Tests fixture ────────────────────────────────────────────────────────────────

#[test]
fn fixture_comptes() {
    let cfg = parse_help_list_config(&fixture_help());
    assert_eq!(cfg.images.len(), 2, "2 images");
    assert_eq!(cfg.infos.len(), 1, "1 info (LIST_BEG ignoré)");
    assert_eq!(cfg.guide_layouts.len(), 1, "1 layout");
    assert_eq!(cfg.guide_infos.len(), 1, "1 guide_info");
}

#[test]
fn fixture_image_0() {
    let cfg = parse_help_list_config(&fixture_help());
    let img = &cfg.images[0];
    assert_eq!(img.image_file, "hlp_controls.g4tx");
    assert_eq!(img.sprite_hash, IMG0_SPRITE, "sprite = 0x1353F111");
    assert_eq!(img.sort_no, 1);
    assert_eq!(img.guide_hash, IMG0_GUIDE, "guide_hash = 0x2156356E");
    assert_eq!(img.sub_hash, IMG0_SUB, "sub_hash = 0x2764916A");
}

#[test]
fn fixture_image_1_sprite() {
    let cfg = parse_help_list_config(&fixture_help());
    assert_eq!(
        cfg.images[1].sprite_hash, IMG1_SPRITE,
        "sprite[1] = 0x8A5AA0AB"
    );
    assert_eq!(cfg.images[1].sort_no, 2);
}

#[test]
fn fixture_info_0_champs_et_blob() {
    let cfg = parse_help_list_config(&fixture_help());
    let info = &cfg.infos[0];
    assert_eq!(info.help_id, INFO0_HELP_ID, "help_id = 0x2A9CD447");
    assert_eq!(info.display_no, 1);
    assert_eq!(info.raw.len(), 15, "15 positions");
    assert_eq!(info.raw[7], INFO0_RAW7, "raw[7] = 0x0EA50CFC");
    assert_eq!(info.raw[8], INFO0_RAW8, "raw[8] = 0x33C5254C");
    assert_eq!(info.raw[13], INFO0_RAW13, "raw[13] = 0x17BE3DBD");
    assert_eq!(info.raw[14], INFO0_RAW14, "raw[14] = 0x2ADE140D");
    // var[12] est une String base64 → raw[12] = 0, blob conservé tel quel
    assert_eq!(
        info.raw[12],
        HashId::ZERO,
        "raw[12] = 0 (la String vit dans blob)"
    );
    assert_eq!(
        info.blob.as_deref(),
        Some(INFO13_BLOB),
        "blob base64 préservé"
    );
}

#[test]
fn fixture_info_0_ref_image_pairing() {
    let cfg = parse_help_list_config(&fixture_help());
    let info = &cfg.infos[0];
    // REF_IMAGE_0 = [0, 2]
    assert_eq!(info.image_offset, 0);
    assert_eq!(info.image_count, 2);
    // images_of résout la tranche
    let imgs = cfg.images_of(info);
    assert_eq!(imgs.len(), 2, "images_of = 2 éléments");
    assert_eq!(imgs[0].sprite_hash, IMG0_SPRITE);
}

#[test]
fn fixture_layout_0() {
    let cfg = parse_help_list_config(&fixture_help());
    let lay = &cfg.guide_layouts[0];
    assert_eq!(lay.page_no, 1);
    assert_eq!(lay.row_no, 2);
    assert_eq!(lay.flag, 1);
    assert_eq!(lay.button_hash, LAY0_BUTTON, "button = 0xF2DD6185");
    assert_eq!(lay.sub_hash, LAY0_SUB, "sub = 0xF52C33F5");
    assert_eq!(lay.coord_x, -859, "coord_x = -859 (signé)");
    assert_eq!(lay.coord_y, 247);
}

#[test]
fn fixture_guide_info_0_et_layouts_of() {
    let cfg = parse_help_list_config(&fixture_help());
    let gi = &cfg.guide_infos[0];
    assert_eq!(gi.guide_id, GI0_GUIDE_ID, "guide_id = 0x2156356E");
    assert_eq!(gi.text_id, GI0_TEXT, "text = 0x74C5F394");
    assert_eq!(gi.layout_offset, 0);
    assert_eq!(gi.layout_count, 1);
    let lays = cfg.layouts_of(gi);
    assert_eq!(lays.len(), 1, "layouts_of = 1 élément");
    assert_eq!(lays[0].button_hash, LAY0_BUTTON);
}

#[test]
fn fixture_find_info() {
    let cfg = parse_help_list_config(&fixture_help());
    assert!(cfg.find_info(INFO0_HELP_ID).is_some());
    assert!(cfg.find_info(HashId(0xDEAD_BEEF)).is_none());
}

// ─── Tests sur fichier réel (skip silencieux si absent) ───────────────────────────

const REAL_HELP: &str = "help/help_list_config_1.04.08.00.cfg.bin.json";

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
fn real_comptes() {
    let Some(root) = load_json(REAL_HELP) else {
        return;
    };
    let cfg = parse_help_list_config(&root);
    assert_eq!(cfg.images.len(), 322, "322 HELP_LIST_IMAGE");
    assert_eq!(cfg.infos.len(), 245, "245 HELP_LIST_INFO");
    assert_eq!(
        cfg.guide_layouts.len(),
        216,
        "216 HELP_OPERATION_GUIDE_LAYOUT"
    );
    assert_eq!(cfg.guide_infos.len(), 21, "21 HELP_OPERATION_GUIDE_INFO");
    assert_eq!(cfg.sort_order.len(), 245, "245 __SORT_INDEX");
    // somme des image_count = 322 (toutes les images sont référencées une fois)
    let total: i64 = cfg.infos.iter().map(|i| i.image_count).sum();
    assert_eq!(total, 322, "somme des image_count = 322");
    // somme des layout_count = 216
    let total_lay: i64 = cfg.guide_infos.iter().map(|i| i.layout_count).sum();
    assert_eq!(total_lay, 216, "somme des layout_count = 216");
}

#[test]
fn real_image_0_1_321() {
    let Some(root) = load_json(REAL_HELP) else {
        return;
    };
    let cfg = parse_help_list_config(&root);
    assert_eq!(cfg.images[0].image_file, "hlp_controls.g4tx");
    assert_eq!(cfg.images[0].sprite_hash, IMG0_SPRITE);
    assert_eq!(cfg.images[0].sort_no, 1);
    assert_eq!(cfg.images[0].guide_hash, IMG0_GUIDE);
    assert_eq!(cfg.images[0].sub_hash, IMG0_SUB);

    assert_eq!(cfg.images[1].sprite_hash, IMG1_SPRITE);
    assert_eq!(cfg.images[1].sort_no, 2);

    assert_eq!(cfg.images[321].image_file, "hlp_7140.g4tx");
    assert_eq!(cfg.images[321].sprite_hash, IMG321_SPRITE);
}

#[test]
fn real_info_0() {
    let Some(root) = load_json(REAL_HELP) else {
        return;
    };
    let cfg = parse_help_list_config(&root);
    let info = &cfg.infos[0];
    assert_eq!(info.help_id, INFO0_HELP_ID, "help_id[0] = 0x2A9CD447");
    assert_eq!(info.display_no, 1);
    assert_eq!(info.raw[7], INFO0_RAW7);
    assert_eq!(info.raw[8], INFO0_RAW8);
    assert_eq!(info.raw[13], INFO0_RAW13);
    assert_eq!(info.raw[14], INFO0_RAW14);
    // REF_IMAGE_0 = [0,0] : la première entrée d'aide n'a pas d'image associée
    assert_eq!(
        info.image_count, 0,
        "info[0] sans image (REF_IMAGE_0 = [0,0])"
    );
    assert_eq!(info.blob, None, "info[0] sans blob (var[12] = Int 0)");
}

#[test]
fn real_info_13_blob_et_ref_image() {
    let Some(root) = load_json(REAL_HELP) else {
        return;
    };
    let cfg = parse_help_list_config(&root);
    let info = &cfg.infos[13];
    assert_eq!(info.help_id, INFO13_HELP_ID, "help_id[13] = 0xCB28800F");
    assert_eq!(info.display_no, 14);
    // var[12] = blob base64 opaque
    assert_eq!(info.blob.as_deref(), Some(INFO13_BLOB), "blob[13] préservé");
    assert_eq!(info.raw[12], HashId::ZERO, "raw[12] = 0 (blob ailleurs)");
    // REF_IMAGE_13 = [0, 4] : première entrée avec des images
    assert_eq!(info.image_offset, 0);
    assert_eq!(info.image_count, 4);
    let imgs = cfg.images_of(info);
    assert_eq!(imgs.len(), 4, "images_of(info[13]) = 4");
    // sort_no local 1..4 dans la tranche
    assert_eq!(imgs[0].sort_no, 1);
    assert_eq!(imgs[3].sort_no, 4);
}

#[test]
fn real_info_244_derniere() {
    let Some(root) = load_json(REAL_HELP) else {
        return;
    };
    let cfg = parse_help_list_config(&root);
    let info = &cfg.infos[244];
    assert_eq!(info.help_id, INFO244_HELP_ID, "help_id[244] = 0x4EFD9BC9");
    assert_eq!(info.flag, 3, "flag[244] = 3");
    assert_eq!(info.display_no, 250, "display_no[244] = 250");
    assert_eq!(info.raw[3], INFO244_RAW3, "raw[3] = 0x1EAB3007");
    assert_eq!(info.raw[4], INFO244_RAW4, "raw[4] = 0x2A54D7AF");
    assert_eq!(info.raw[5], INFO244_RAW5, "raw[5] = 0x478BDBD1");
}

#[test]
fn real_layout_0_et_215() {
    let Some(root) = load_json(REAL_HELP) else {
        return;
    };
    let cfg = parse_help_list_config(&root);
    let l0 = &cfg.guide_layouts[0];
    assert_eq!(l0.page_no, 1);
    assert_eq!(l0.row_no, 2);
    assert_eq!(l0.flag, 1);
    assert_eq!(l0.button_hash, LAY0_BUTTON, "button[0] = 0xF2DD6185");
    assert_eq!(l0.sub_hash, LAY0_SUB, "sub[0] = 0xF52C33F5");
    assert_eq!(l0.coord_x, -859, "coord_x[0] = -859 (signé)");
    assert_eq!(l0.coord_y, 247);

    let l215 = &cfg.guide_layouts[215];
    assert_eq!(l215.page_no, 1);
    assert_eq!(l215.row_no, 1);
    assert_eq!(l215.flag, 0);
    assert_eq!(l215.button_hash, LAY215_BUTTON, "button[215] = 0x07B7FE51");
    assert_eq!(l215.sub_hash, LAY215_SUB, "sub[215] = 0xC28C1C34");
    assert_eq!(l215.coord_x, 305, "coord_x[215] = 305");
    assert_eq!(l215.coord_y, -187, "coord_y[215] = -187 (signé)");
}

#[test]
fn real_guide_info_0_et_20() {
    let Some(root) = load_json(REAL_HELP) else {
        return;
    };
    let cfg = parse_help_list_config(&root);
    let g0 = &cfg.guide_infos[0];
    // guide_id[0] = 0x2156356E == IMAGE_0.guide_hash (recoupement)
    assert_eq!(g0.guide_id, GI0_GUIDE_ID, "guide_id[0] = 0x2156356E");
    assert_eq!(
        g0.guide_id, cfg.images[0].guide_hash,
        "recoupe IMAGE_0.guide_hash"
    );
    assert_eq!(g0.text_id, GI0_TEXT, "text[0] = 0x74C5F394");
    assert_eq!(g0.layout_offset, 0);
    assert_eq!(g0.layout_count, 18, "REF_LAYOUT_0 = [0,18]");
    assert_eq!(cfg.layouts_of(g0).len(), 18, "layouts_of(g0) = 18");

    let g20 = &cfg.guide_infos[20];
    assert_eq!(g20.guide_id, GI20_GUIDE_ID, "guide_id[20] = 0x73C7EE5E");
    assert_eq!(g20.text_id, GI20_TEXT, "text[20] = 0x686E4428");
    assert_eq!(g20.layout_offset, 207, "REF_LAYOUT_20 = [207,9]");
    assert_eq!(g20.layout_count, 9);
}

#[test]
fn real_sort_order() {
    let Some(root) = load_json(REAL_HELP) else {
        return;
    };
    let cfg = parse_help_list_config(&root);
    assert_eq!(cfg.sort_order.len(), 245);
    // __SORT_INDEX premiers 8 : [11, 234, 1, 243, 111, 159, 132, 173]
    assert_eq!(
        &cfg.sort_order[..8],
        &[11, 234, 1, 243, 111, 159, 132, 173],
        "premiers 8 indices de tri"
    );
    // permutation de 0..245 : chaque index apparaît une fois
    let mut sorted = cfg.sort_order.clone();
    sorted.sort_unstable();
    let expect: std::vec::Vec<i64> = (0..245).collect();
    assert_eq!(sorted, expect, "__SORT_INDEX est une permutation de 0..245");
}
