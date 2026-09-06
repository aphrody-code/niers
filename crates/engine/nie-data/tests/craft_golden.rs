#![allow(clippy::pedantic)]
//! Tests golden `craft` — valeurs réelles tirées de :
//! - `data/common/gamedata/craft/craft_obj_config_1.04.10.00.cfg.bin.json`
//! - `data/common/gamedata/craft/craft_theme_config_0.00.00.cfg.bin.json`
//!
//! Toutes les valeurs hex sont `v & 0xFFFFFFFF`. Les noeuds sont au format `entries`
//! (variables positionnelles) ; les positions de sémantique inconnue sont vérifiées
//! par leur valeur brute observée, jamais inventée.

mod common;

extern crate std;

use nie_data::craft::{parse_craft_obj_config, parse_craft_theme_config};
use nie_data::hash::HashId;
use serde_json::json;

const REAL_OBJ: &str = "craft/craft_obj_config_1.04.10.00.cfg.bin.json";
const REAL_THEME: &str = "craft/craft_theme_config_0.00.00.cfg.bin.json";

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

// ─── Fixture inline (craft_obj) ────────────────────────────────────────────────
//
// Reproduit la structure interlacée : un CRAFT_OBJ_INFO_0 suivi de ses 4 REF, plus
// une sous-liste de loterie, sockets, points d'accroche, visuels et groupes.

fn fixture_obj() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "CRAFT_OBJ_INTERREST_LOTTERY_UNIQUE_CHARA_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    {
                        "name": "CRAFT_OBJ_INTERREST_LOTTERY_UNIQUE_CHARA_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "-1717452464"},
                            {"type": "Int", "value": "2"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "CRAFT_OBJ_VISUAL_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    {
                        "name": "CRAFT_OBJ_VISUAL_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "757857389"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "1"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "CRAFT_OBJ_VISUAL_GROUP_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    {
                        "name": "CRAFT_OBJ_VISUAL_GROUP_INFO_0",
                        "variables": [{"type": "Int", "value": "-480378657"}],
                        "children": []
                    },
                    {
                        "name": "CRAFT_OBJ_VISUAL_GROUP_INFO_REF_VISUAL_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "1"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "CRAFT_OBJ_INFO_LIST_BEG_0",
                "variables": [
                    {"type": "Int", "value": "1"},
                    {"type": "Int", "value": "1"}
                ],
                "children": [
                    {
                        "name": "CRAFT_OBJ_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "1186156105"},
                            {"type": "Int", "value": "1"},
                            {"type": "Int", "value": "4"},
                            {"type": "Float", "value": "1,5"}
                        ],
                        "children": []
                    },
                    {
                        "name": "CRAFT_OBJ_INFO_REF_INTERREST_LOTTERY_UNIQUE_CHARA_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    {
                        "name": "CRAFT_OBJ_INFO_REF_INTERREST_SOCKET_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    {
                        "name": "CRAFT_OBJ_INFO_REF_NPC_STICK_POINT_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    {
                        "name": "CRAFT_OBJ_INFO_REF_VISUAL_GROUP_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "1"}
                        ],
                        "children": []
                    },
                    {
                        "name": "CRAFT_OBJ_CATEGORY_INFO_LIST_BEG_0",
                        "variables": [{"type": "Int", "value": "1"}],
                        "children": [
                            {
                                "name": "CRAFT_OBJ_CATEGORY_INFO_0",
                                "variables": [
                                    {"type": "Int", "value": "1"},
                                    {"type": "Int", "value": "80"}
                                ],
                                "children": []
                            }
                        ]
                    }
                ]
            }
        ]
    })
}

#[test]
fn fixture_obj_comptes() {
    let cfg = parse_craft_obj_config(&fixture_obj());
    assert_eq!(cfg.lottery_unique_charas.len(), 1);
    assert_eq!(cfg.visual_infos.len(), 1);
    assert_eq!(cfg.visual_groups.len(), 1);
    assert_eq!(cfg.objs.len(), 1);
    assert_eq!(cfg.categories.len(), 1);
}

#[test]
fn fixture_obj_lottery() {
    let cfg = parse_craft_obj_config(&fixture_obj());
    let l = &cfg.lottery_unique_charas[0];
    // var[0] = -1717452464 → 0x99A1C150 ; var[1] = 2
    assert_eq!(l.chara_id, HashId::from_i64(-1717452464));
    assert_eq!(l.chara_id, HashId(0x99A1_C150));
    assert_eq!(l.weight, 2.0);
}

#[test]
fn fixture_obj_main_et_refs() {
    let cfg = parse_craft_obj_config(&fixture_obj());
    let o = &cfg.objs[0];
    // var[0] = 1186156105 → 0x46B34E49
    assert_eq!(o.craft_id, HashId(0x46B3_4E49));
    // raw = var[1..] = [1.0, 4.0, 1.5]
    assert_eq!(o.raw, vec![1.0, 4.0, 1.5]);
    // ref_visual_group = [0, 1] ; les 3 autres = [0, 0]
    assert_eq!(o.ref_visual_group.offset, 0);
    assert_eq!(o.ref_visual_group.count, 1);
    assert_eq!(o.ref_lottery.count, 0);
    assert_eq!(o.ref_socket.count, 0);
    assert_eq!(o.ref_npc_stick.count, 0);
}

#[test]
fn fixture_obj_group_apparie() {
    let cfg = parse_craft_obj_config(&fixture_obj());
    let g = &cfg.visual_groups[0];
    assert_eq!(g.group_id, HashId(0xE35E_00DF));
    assert_eq!(g.visual_index, 0);
    assert_eq!(g.count, 1);
    // visuals_of_group résout la plage
    let vis = cfg.visuals_of_group(g);
    assert_eq!(vis.len(), 1);
    assert_eq!(vis[0].visual_id, HashId(0x2D2B_FC6D));
}

#[test]
fn fixture_obj_category() {
    let cfg = parse_craft_obj_config(&fixture_obj());
    assert_eq!(cfg.categories[0].category, 1);
    assert_eq!(cfg.categories[0].value, 80);
}

#[test]
fn fixture_obj_find() {
    let cfg = parse_craft_obj_config(&fixture_obj());
    assert!(cfg.find_obj(HashId(0x46B3_4E49)).is_some());
    assert!(cfg.find_obj(HashId(0xDEAD_BEEF)).is_none());
}

// ─── Fichier réel : craft_obj_config ───────────────────────────────────────────

#[test]
fn real_obj_comptes() {
    let Some(root) = load_json(REAL_OBJ) else {
        return;
    };
    let cfg = parse_craft_obj_config(&root);
    assert_eq!(cfg.lottery_unique_charas.len(), 1005, "loteries");
    assert_eq!(cfg.sockets.len(), 87, "sockets");
    assert_eq!(cfg.npc_stick_points.len(), 105, "npc stick points");
    assert_eq!(cfg.visual_infos.len(), 187, "visuals");
    assert_eq!(cfg.visual_groups.len(), 167, "groupes de visuels");
    assert_eq!(cfg.objs.len(), 157, "objets de craft");
    assert_eq!(cfg.categories.len(), 5, "catégories");
}

#[test]
fn real_obj_lottery_0_et_688() {
    let Some(root) = load_json(REAL_OBJ) else {
        return;
    };
    let cfg = parse_craft_obj_config(&root);
    // entrée 0 : [-1717452464, 2]
    assert_eq!(
        cfg.lottery_unique_charas[0].chara_id,
        HashId::from_i64(-1717452464)
    );
    assert_eq!(cfg.lottery_unique_charas[0].weight, 2.0);
    // entrée 688 : [150881673, 1,5] — seul Float de la liste
    assert_eq!(cfg.lottery_unique_charas[688].chara_id, HashId(0x08FE_4589));
    assert_eq!(cfg.lottery_unique_charas[688].weight, 1.5);
}

#[test]
fn real_obj_socket_floats() {
    let Some(root) = load_json(REAL_OBJ) else {
        return;
    };
    let cfg = parse_craft_obj_config(&root);
    // socket 0 : [1, 0, 3, 0]
    assert_eq!(cfg.sockets[0].raw, [1.0, 0.0, 3.0, 0.0]);
    // socket 6 : [1, 0, 0.5, 0]
    assert_eq!(cfg.sockets[6].raw, [1.0, 0.0, 0.5, 0.0]);
    // socket 7 : [1, 0, 1.5, 0]
    assert_eq!(cfg.sockets[7].raw, [1.0, 0.0, 1.5, 0.0]);
}

#[test]
fn real_obj_npc_stick() {
    let Some(root) = load_json(REAL_OBJ) else {
        return;
    };
    let cfg = parse_craft_obj_config(&root);
    // npc 0 : [0, 0, 1.5, -1, 0, 0]
    assert_eq!(cfg.npc_stick_points[0].raw, [0.0, 0.0, 1.5, -1.0, 0.0, 0.0]);
    // npc 1 : [1.7, 0, 1.5, -1, 0, 0]
    assert_eq!(cfg.npc_stick_points[1].raw, [1.7, 0.0, 1.5, -1.0, 0.0, 0.0]);
    // npc 2 : [-1.7, 0, 1.5, -1, 0, 0]
    assert_eq!(
        cfg.npc_stick_points[2].raw,
        [-1.7, 0.0, 1.5, -1.0, 0.0, 0.0]
    );
    // npc 6 : [-4, 0, 5, -1, 0, 0]
    assert_eq!(
        cfg.npc_stick_points[6].raw,
        [-4.0, 0.0, 5.0, -1.0, 0.0, 0.0]
    );
}

#[test]
fn real_obj_visual_0() {
    let Some(root) = load_json(REAL_OBJ) else {
        return;
    };
    let cfg = parse_craft_obj_config(&root);
    // visual 0 : [757857389, 0, 1]
    assert_eq!(cfg.visual_infos[0].visual_id, HashId(0x2D2B_FC6D));
    assert_eq!(cfg.visual_infos[0].secondary, HashId::ZERO);
    assert_eq!(cfg.visual_infos[0].count, 1);
}

#[test]
fn real_obj_visual_groups_0_1() {
    let Some(root) = load_json(REAL_OBJ) else {
        return;
    };
    let cfg = parse_craft_obj_config(&root);
    // group 0 : group_id 0xE35E00DF, ref [0,1]
    assert_eq!(cfg.visual_groups[0].group_id, HashId(0xE35E_00DF));
    assert_eq!(cfg.visual_groups[0].visual_index, 0);
    assert_eq!(cfg.visual_groups[0].count, 1);
    // group 1 : même group_id, ref [1,1]
    assert_eq!(cfg.visual_groups[1].group_id, HashId(0xE35E_00DF));
    assert_eq!(cfg.visual_groups[1].visual_index, 1);
}

#[test]
fn real_obj_main_0_et_2() {
    let Some(root) = load_json(REAL_OBJ) else {
        return;
    };
    let cfg = parse_craft_obj_config(&root);

    // objet 0 : craft_id 0x46B34E49 ; ref_visual_group [0,1] ; 3 autres refs [0,0]
    let o0 = &cfg.objs[0];
    assert_eq!(o0.craft_id, HashId::from_i64(1186156105));
    assert_eq!(o0.craft_id, HashId(0x46B3_4E49));
    assert_eq!(o0.raw.len(), 27, "27 variables après var[0]");
    assert_eq!(o0.raw[0], 1.0); // var[1]
    assert_eq!(o0.raw[26], 5.0); // var[27]
    assert_eq!(o0.ref_visual_group.offset, 0);
    assert_eq!(o0.ref_visual_group.count, 1);
    assert_eq!(o0.ref_lottery.count, 0);
    assert_eq!(o0.ref_socket.count, 0);
    assert_eq!(o0.ref_npc_stick.count, 0);

    // objet 1 : craft_id 0xDFBA1FF3
    assert_eq!(cfg.objs[1].craft_id, HashId(0xDFBA_1FF3));

    // objet 2 : refs non triviales et hash en raw[14] (var[15])
    let o2 = &cfg.objs[2];
    assert_eq!(o2.craft_id, HashId::from_i64(-732419361));
    assert_eq!(o2.craft_id, HashId(0xD458_2ADF));
    // raw[14] = var[15] = 1659389071 (hash stocké en f64)
    assert_eq!(o2.raw[14], 1659389071.0);
    assert_eq!(o2.raw[26], 20.0); // var[27]
    assert_eq!(o2.ref_lottery.offset, 0);
    assert_eq!(o2.ref_lottery.count, 13);
    assert_eq!(o2.ref_socket.count, 1);
    assert_eq!(o2.ref_npc_stick.count, 3);
    assert_eq!(o2.ref_visual_group.offset, 2);
    assert_eq!(o2.ref_visual_group.count, 1);
}

#[test]
fn real_obj_categories() {
    let Some(root) = load_json(REAL_OBJ) else {
        return;
    };
    let cfg = parse_craft_obj_config(&root);
    // [1,80] [2,50] [3,20] [4,100] [5,2]
    let expected = [(1, 80), (2, 50), (3, 20), (4, 100), (5, 2)];
    for (i, (cat, val)) in expected.iter().enumerate() {
        assert_eq!(cfg.categories[i].category, *cat);
        assert_eq!(cfg.categories[i].value, *val);
    }
}

#[test]
fn real_obj_find() {
    let Some(root) = load_json(REAL_OBJ) else {
        return;
    };
    let cfg = parse_craft_obj_config(&root);
    assert!(cfg.find_obj(HashId(0x46B3_4E49)).is_some());
    assert!(cfg.find_obj(HashId(0xDEAD_BEEF)).is_none());
}

// ─── Fixture inline (craft_theme) ──────────────────────────────────────────────

fn fixture_theme() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "CRAFT_THEME_TYPE_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    {
                        "name": "CRAFT_THEME_TYPE_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "-1537270480"},
                            {"type": "Int", "value": "-1537270480"},
                            {"type": "Int", "value": "-406116050"},
                            {"type": "Int", "value": "895208218"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    {
                        "name": "CRAFT_THEME_TYPE_INFO_1",
                        "variables": [
                            {"type": "Int", "value": "1029065866"},
                            {"type": "Int", "value": "1029065866"},
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "-1930867384"},
                            {"type": "String", "value": "AAAAACEFNY12ZtgAEwIoAAYCNLCWyF8oAAYCMgAAAAEyAAAAAXg="},
                            {"type": "Int", "value": "246590570"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "CRAFT_THEME_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "1"}],
                "children": [
                    {
                        "name": "CRAFT_THEME_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "-1917049535"},
                            {"type": "Int", "value": "489051273"},
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    },
                    {
                        "name": "CRAFT_THEME_INFO_REF_TYPE_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "3"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

#[test]
fn fixture_theme_comptes() {
    let cfg = parse_craft_theme_config(&fixture_theme());
    assert_eq!(cfg.theme_types.len(), 2);
    assert_eq!(cfg.themes.len(), 1);
}

#[test]
fn fixture_theme_type_0_et_1() {
    let cfg = parse_craft_theme_config(&fixture_theme());
    // type 0 : var[4] = Int "0" → data_b64 vide ; var[5] = 0
    let t0 = &cfg.theme_types[0];
    assert_eq!(t0.type_id, HashId(0xA45F_1D30));
    assert_eq!(t0.type_id_dup, HashId(0xA45F_1D30));
    assert_eq!(t0.sub_hash, HashId(0xE7CB_292E));
    assert_eq!(t0.variant_hash, HashId(0x355B_CB1A));
    assert_eq!(t0.data_b64, "");
    assert_eq!(t0.extra_hash, HashId::ZERO);
    // type 1 : var[4] = String base64
    let t1 = &cfg.theme_types[1];
    assert_eq!(t1.type_id, HashId(0x3D56_4C8A));
    assert_eq!(t1.variant_hash, HashId(0x8CE9_4D48));
    assert_eq!(
        t1.data_b64,
        "AAAAACEFNY12ZtgAEwIoAAYCNLCWyF8oAAYCMgAAAAEyAAAAAXg="
    );
    assert_eq!(t1.extra_hash, HashId(0x0EB2_AC6A));
}

#[test]
fn fixture_theme_0_ref() {
    let cfg = parse_craft_theme_config(&fixture_theme());
    let th = &cfg.themes[0];
    assert_eq!(th.theme_id, HashId(0x8DBC_2541));
    assert_eq!(th.hash1, HashId(0x1D26_5489));
    assert_eq!(th.hash2, HashId::ZERO);
    assert_eq!(th.ref_types.offset, 0);
    assert_eq!(th.ref_types.count, 3);
}

// ─── Fichier réel : craft_theme_config ─────────────────────────────────────────

#[test]
fn real_theme_comptes() {
    let Some(root) = load_json(REAL_THEME) else {
        return;
    };
    let cfg = parse_craft_theme_config(&root);
    assert_eq!(cfg.theme_types.len(), 9, "9 types");
    assert_eq!(cfg.themes.len(), 3, "3 thèmes");
}

#[test]
fn real_theme_types() {
    let Some(root) = load_json(REAL_THEME) else {
        return;
    };
    let cfg = parse_craft_theme_config(&root);
    // type 0 : data_b64 vide (Int "0")
    assert_eq!(cfg.theme_types[0].type_id, HashId(0xA45F_1D30));
    assert_eq!(cfg.theme_types[0].sub_hash, HashId(0xE7CB_292E));
    assert_eq!(cfg.theme_types[0].data_b64, "");
    // type 1 : data_b64 présent
    assert_eq!(cfg.theme_types[1].type_id, HashId(0x3D56_4C8A));
    assert_eq!(
        cfg.theme_types[1].data_b64,
        "AAAAACEFNY12ZtgAEwIoAAYCNLCWyF8oAAYCMgAAAAEyAAAAAXg="
    );
    assert_eq!(cfg.theme_types[1].extra_hash, HashId(0x0EB2_AC6A));
}

#[test]
fn real_theme_infos_et_refs() {
    let Some(root) = load_json(REAL_THEME) else {
        return;
    };
    let cfg = parse_craft_theme_config(&root);
    // thème 0 : [-1917049535, 489051273, 0], ref [0,3]
    assert_eq!(cfg.themes[0].theme_id, HashId(0x8DBC_2541));
    assert_eq!(cfg.themes[0].hash1, HashId(0x1D26_5489));
    assert_eq!(cfg.themes[0].hash2, HashId::ZERO);
    assert_eq!(cfg.themes[0].ref_types.offset, 0);
    assert_eq!(cfg.themes[0].ref_types.count, 3);
    // thème 1 : [-1042277463, 1422610124, 564730407], ref [3,3]
    assert_eq!(cfg.themes[1].theme_id, HashId(0xC1E0_1BA9));
    assert_eq!(cfg.themes[1].hash1, HashId(0x54CB_4ECC));
    assert_eq!(cfg.themes[1].hash2, HashId(0x21A9_1A27));
    assert_eq!(cfg.themes[1].ref_types.offset, 3);
    // thème 2 : [1837409304, 14910418, 2005717639], ref [6,3]
    assert_eq!(cfg.themes[2].theme_id, HashId(0x6D84_A418));
    assert_eq!(cfg.themes[2].hash1, HashId(0x00E3_83D2));
    assert_eq!(cfg.themes[2].hash2, HashId(0x778C_D287));
    assert_eq!(cfg.themes[2].ref_types.offset, 6);

    // types_of_theme résout la plage (3 types par thème, 3×3 = 9)
    for th in &cfg.themes {
        assert_eq!(cfg.types_of_theme(th).len(), 3);
    }
}
