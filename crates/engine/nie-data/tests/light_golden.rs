#![allow(clippy::pedantic)]
//! Tests golden `light` — valeurs tirées de :
//! `light/light_overwrite_config_1.03.21.00.cfg.bin.json`
//!
//! Vérifications champ par champ (lignes JSON de référence) :
//! - PARAM_0 (l.7-26)  : var[0]=Int("7531368"), var[1]=Int("400")
//! - PARAM_1 (l.27-40) : var[0]=Int("-1910140030"), var[1]=Float("0,7")
//! - PARAM_2 (l.41-61) : var[0]=Int("-1510910904"), var[1..3]=Float 0.85/0.82/0.63
//! - PARAM_7 (l.119-133): var[0]=Int("-2124741719"), var[1]=Float("-0,05")
//! - INFO_0 (l.319-328) : var[0]=Int("896315108")
//! - REF_PARAM_0 (l.329-340): var[0]=Int("0"), var[1]=Int("9")
//! - INFO_1 (l.341-350) : var[0]=Int("-1402601634")
//! - REF_PARAM_1 (l.351-362): var[0]=Int("9"), var[1]=Int("9")
//! - INFO_2 (l.363-372) : var[0]=Int("-614281272")
//! - REF_PARAM_2 (l.373-392): var[0]=Int("18"), var[1]=Int("2")

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::light::{LightVal, parse_light_overwrite_config};
use serde_json::json;

// 7531368 = 0x0072DBE8 — param_id de PARAM_0, PARAM_9, PARAM_18 (JSON l.7)
const PARAM_ID_0: HashId = HashId(7531368);
// 896315108 = 0x357CF2E4 — info_id de LIGHT_OVERWRITE_INFO_0 (JSON l.322)
const INFO_ID_0: HashId = HashId(896315108);

fn fixture_light() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "LIGHT_OVERWRITE_PARAM_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "4"}],
                "children": [
                    {
                        "name": "LIGHT_OVERWRITE_PARAM_0",
                        "variables": [
                            {"type": "Int",   "value": "7531368"},
                            {"type": "Int",   "value": "400"}
                        ],
                        "children": []
                    },
                    {
                        "name": "LIGHT_OVERWRITE_PARAM_1",
                        "variables": [
                            {"type": "Int",   "value": "-1910140030"},
                            {"type": "Float", "value": "0,7"}
                        ],
                        "children": []
                    },
                    {
                        "name": "LIGHT_OVERWRITE_PARAM_2",
                        "variables": [
                            {"type": "Int",   "value": "-1510910904"},
                            {"type": "Float", "value": "0,85"},
                            {"type": "Float", "value": "0,82"},
                            {"type": "Float", "value": "0,63"}
                        ],
                        "children": []
                    },
                    {
                        "name": "LIGHT_OVERWRITE_PARAM_7",
                        "variables": [
                            {"type": "Int",   "value": "-2124741719"},
                            {"type": "Float", "value": "-0,05"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "LIGHT_OVERWRITE_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    {
                        "name": "LIGHT_OVERWRITE_INFO_0",
                        "variables": [{"type": "Int", "value": "896315108"}],
                        "children": []
                    },
                    {
                        "name": "LIGHT_OVERWRITE_INFO_REF_PARAM_0",
                        "variables": [
                            {"type": "Int", "value": "0"},
                            {"type": "Int", "value": "2"}
                        ],
                        "children": []
                    },
                    {
                        "name": "LIGHT_OVERWRITE_INFO_1",
                        "variables": [{"type": "Int", "value": "-1402601634"}],
                        "children": []
                    },
                    {
                        "name": "LIGHT_OVERWRITE_INFO_REF_PARAM_1",
                        "variables": [
                            {"type": "Int", "value": "2"},
                            {"type": "Int", "value": "2"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur fixture ────────────────────────────────────────────────────────

#[test]
fn fixture_comptes() {
    let cfg = parse_light_overwrite_config(&fixture_light());
    assert_eq!(cfg.params.len(), 4, "4 params dans la fixture");
    assert_eq!(cfg.infos.len(), 2, "2 infos dans la fixture");
}

#[test]
fn fixture_list_beg_ignore() {
    let cfg = parse_light_overwrite_config(&fixture_light());
    // LIST_BEG ne doit pas apparaître dans les comptes
    assert_eq!(cfg.params.len(), 4, "PARAM_LIST_BEG non compté");
    assert_eq!(cfg.infos.len(), 2, "INFO_LIST_BEG non compté");
}

#[test]
fn fixture_param_0_id_et_valeur_int() {
    let cfg = parse_light_overwrite_config(&fixture_light());
    // PARAM_0.var[0]=7531368 → param_id=0x0072DBE8 (JSON l.7)
    assert_eq!(cfg.params[0].param_id, PARAM_ID_0, "param_id[0]=0x0072DBE8");
    // PARAM_0.var[1]=Int("400") (JSON l.10)
    assert_eq!(cfg.params[0].values.len(), 1, "PARAM_0 a 1 valeur");
    assert_eq!(
        cfg.params[0].values[0],
        LightVal::Int(400),
        "values[0]=Int(400)"
    );
}

#[test]
fn fixture_param_1_float() {
    let cfg = parse_light_overwrite_config(&fixture_light());
    // PARAM_1.var[0]=-1910140030, var[1]=Float("0,7") (JSON l.33)
    assert_eq!(
        cfg.params[1].param_id,
        HashId::from_i64(-1910140030),
        "param_id[1]=-1910140030 as u32"
    );
    assert!(
        cfg.params[1].values[0].is_float(),
        "PARAM_1 values[0] est Float"
    );
    let v = cfg.params[1].values[0].as_f64();
    assert!(
        (v - 0.7).abs() < 1e-5,
        "PARAM_1 values[0] ≈ 0.7 (réel: {v})"
    );
}

#[test]
fn fixture_param_2_triple_float() {
    let cfg = parse_light_overwrite_config(&fixture_light());
    // PARAM_2.var[1..3]=Float 0.85/0.82/0.63 (JSON l.50,54,58)
    assert_eq!(cfg.params[2].values.len(), 3, "PARAM_2 a 3 valeurs");
    let v0 = cfg.params[2].values[0].as_f64();
    let v1 = cfg.params[2].values[1].as_f64();
    let v2 = cfg.params[2].values[2].as_f64();
    assert!((v0 - 0.85).abs() < 1e-5, "values[0] ≈ 0.85 (réel: {v0})");
    assert!((v1 - 0.82).abs() < 1e-5, "values[1] ≈ 0.82 (réel: {v1})");
    assert!((v2 - 0.63).abs() < 1e-5, "values[2] ≈ 0.63 (réel: {v2})");
}

#[test]
fn fixture_param_7_float_negatif() {
    let cfg = parse_light_overwrite_config(&fixture_light());
    // PARAM_7.var[1]=Float("-0,05") (JSON l.129)
    assert!(
        cfg.params[3].values[0].is_float(),
        "PARAM_7 values[0] Float"
    );
    let v = cfg.params[3].values[0].as_f64();
    assert!(
        (v - (-0.05)).abs() < 1e-5,
        "PARAM_7 values[0] ≈ -0.05 (réel: {v})"
    );
}

#[test]
fn fixture_info_0_champs() {
    let cfg = parse_light_overwrite_config(&fixture_light());
    // INFO_0.var[0]=896315108 (JSON l.322), REF_PARAM_0 : offset=0, count=2
    assert_eq!(
        cfg.infos[0].info_id, INFO_ID_0,
        "info_id[0]=896315108=0x357CF2E4"
    );
    assert_eq!(cfg.infos[0].param_offset, 0, "param_offset[0]=0");
    assert_eq!(cfg.infos[0].param_count, 2, "param_count[0]=2");
}

#[test]
fn fixture_info_1_champs() {
    let cfg = parse_light_overwrite_config(&fixture_light());
    // INFO_1.var[0]=-1402601634 (JSON l.343), REF_PARAM_1 : offset=2, count=2
    assert_eq!(
        cfg.infos[1].info_id,
        HashId::from_i64(-1402601634),
        "info_id[1]=-1402601634 as u32"
    );
    assert_eq!(cfg.infos[1].param_offset, 2, "param_offset[1]=2");
    assert_eq!(cfg.infos[1].param_count, 2, "param_count[1]=2");
}

#[test]
fn fixture_params_of() {
    let cfg = parse_light_overwrite_config(&fixture_light());
    // params_of(infos[0]) → tranche [0..2] → 2 params
    let slice = cfg.params_of(&cfg.infos[0]);
    assert_eq!(slice.len(), 2, "params_of(infos[0]).len()=2");
    assert_eq!(
        slice[0].param_id, PARAM_ID_0,
        "slice[0].param_id=PARAM_ID_0"
    );
}

#[test]
fn fixture_find_info() {
    let cfg = parse_light_overwrite_config(&fixture_light());
    let found = cfg.find_info(INFO_ID_0);
    assert!(
        found.is_some(),
        "find_info(INFO_ID_0) doit trouver infos[0]"
    );
    assert_eq!(found.unwrap().param_count, 2);
    assert!(cfg.find_info(HashId(0xDEADBEEF)).is_none(), "absent → None");
}

// ─── Tests sur fichier réel (skip silencieux si absent) ───────────────────────

const REAL_LIGHT: &str = "light/light_overwrite_config_1.03.21.00.cfg.bin.json";

fn load_json(path: &str) -> Option<serde_json::Value> {
    let path = common::chemin(path)?;
    if !path.is_file() {
        eprintln!("skip : {} absent du corpus", path.display());
        return None;
    }
    let s = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", path.display()));
    Some(
        serde_json::from_str(&s)
            .unwrap_or_else(|e| panic!("JSON invalide {}: {e}", path.display())),
    )
}

#[test]
fn real_file_comptes() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // PARAM_LIST_BEG_0.var[0]=20 (JSON l.5), INFO_LIST_BEG_0.var[0]=3 (JSON l.311)
    assert_eq!(
        cfg.params.len(),
        20,
        "20 LIGHT_OVERWRITE_PARAM dans le fichier réel"
    );
    assert_eq!(
        cfg.infos.len(),
        3,
        "3 LIGHT_OVERWRITE_INFO dans le fichier réel"
    );
}

#[test]
fn real_file_param_0_id_et_valeur() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // PARAM_0.var[0]=Int("7531368") (JSON l.7), var[1]=Int("400") (JSON l.10)
    assert_eq!(
        cfg.params[0].param_id, PARAM_ID_0,
        "PARAM_0.param_id=0x0072DBE8"
    );
    assert_eq!(
        cfg.params[0].values[0],
        LightVal::Int(400),
        "PARAM_0.values[0]=Int(400)"
    );
}

#[test]
fn real_file_param_1_float() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // PARAM_1.var[1]=Float("0,7") (JSON l.33)
    assert!(
        cfg.params[1].values[0].is_float(),
        "PARAM_1.values[0] Float"
    );
    let v = cfg.params[1].values[0].as_f64();
    assert!(
        (v - 0.7).abs() < 1e-5,
        "PARAM_1.values[0] ≈ 0.7 (réel: {v})"
    );
}

#[test]
fn real_file_param_2_triple_float() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // PARAM_2.var[1..3]=0.85/0.82/0.63 (JSON l.49,53,57)
    assert_eq!(cfg.params[2].values.len(), 3, "PARAM_2 a 3 valeurs");
    let v0 = cfg.params[2].values[0].as_f64();
    let v1 = cfg.params[2].values[1].as_f64();
    let v2 = cfg.params[2].values[2].as_f64();
    assert!((v0 - 0.85).abs() < 1e-5, "values[0] ≈ 0.85 (réel: {v0})");
    assert!((v1 - 0.82).abs() < 1e-5, "values[1] ≈ 0.82 (réel: {v1})");
    assert!((v2 - 0.63).abs() < 1e-5, "values[2] ≈ 0.63 (réel: {v2})");
}

#[test]
fn real_file_param_7_float_negatif() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // PARAM_7.var[0]=-2124741719 (JSON l.121), var[1]=Float("-0,05") (JSON l.124)
    assert_eq!(
        cfg.params[7].param_id,
        HashId::from_i64(-2124741719),
        "PARAM_7.param_id=-2124741719 as u32"
    );
    let v = cfg.params[7].values[0].as_f64();
    assert!(
        (v - (-0.05)).abs() < 1e-5,
        "PARAM_7.values[0] ≈ -0.05 (réel: {v})"
    );
}

#[test]
fn real_file_info_0_champs() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // INFO_0.var[0]=896315108 (JSON l.322), REF_PARAM_0.var[0]=0 (l.332), var[1]=9 (l.336)
    assert_eq!(cfg.infos[0].info_id, INFO_ID_0, "INFO_0.info_id=896315108");
    assert_eq!(cfg.infos[0].param_offset, 0, "INFO_0.param_offset=0");
    assert_eq!(cfg.infos[0].param_count, 9, "INFO_0.param_count=9");
}

#[test]
fn real_file_info_1_champs() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // INFO_1.var[0]=-1402601634 (JSON l.345), REF_PARAM_1 : offset=9 (l.355), count=9 (l.359)
    assert_eq!(
        cfg.infos[1].info_id,
        HashId::from_i64(-1402601634),
        "INFO_1.info_id=-1402601634 as u32"
    );
    assert_eq!(cfg.infos[1].param_offset, 9, "INFO_1.param_offset=9");
    assert_eq!(cfg.infos[1].param_count, 9, "INFO_1.param_count=9");
}

#[test]
fn real_file_info_2_champs() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // INFO_2.var[0]=-614281272 (JSON l.368), REF_PARAM_2 : offset=18 (l.378), count=2 (l.382)
    assert_eq!(
        cfg.infos[2].info_id,
        HashId::from_i64(-614281272),
        "INFO_2.info_id=-614281272 as u32"
    );
    assert_eq!(cfg.infos[2].param_offset, 18, "INFO_2.param_offset=18");
    assert_eq!(cfg.infos[2].param_count, 2, "INFO_2.param_count=2");
}

#[test]
fn real_file_params_of_info_0() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // INFO_0 → params[0..9] → 9 params, le premier est PARAM_0
    let slice = cfg.params_of(&cfg.infos[0]);
    assert_eq!(slice.len(), 9, "params_of(infos[0]).len()=9");
    assert_eq!(
        slice[0].param_id, PARAM_ID_0,
        "slice[0].param_id=PARAM_ID_0"
    );
}

#[test]
fn real_file_params_of_info_2_borne() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // INFO_2 → params[18..20] → 2 params
    let slice = cfg.params_of(&cfg.infos[2]);
    assert_eq!(slice.len(), 2, "params_of(infos[2]).len()=2");
}

#[test]
fn real_file_find_info() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    let found = cfg.find_info(INFO_ID_0);
    assert!(
        found.is_some(),
        "find_info(INFO_ID_0) doit trouver infos[0]"
    );
    assert_eq!(found.unwrap().param_count, 9);
}

#[test]
fn real_file_tous_params_ont_valeurs() {
    let Some(root) = load_json(REAL_LIGHT) else {
        return;
    };
    let cfg = parse_light_overwrite_config(&root);
    // Tous les 20 params ont au moins 1 valeur
    let sans = cfg.params.iter().filter(|p| p.values.is_empty()).count();
    assert_eq!(sans, 0, "tous les params ont au moins 1 valeur");
}
