#![allow(clippy::pedantic)]
//! Tests golden `academic_year` — `data/common/gamedata/character/academic_year_config.cfg.bin.json`.
//! 3 années scolaires (type 1/2/3).

mod common;

extern crate std;

use nie_data::academic_year::parse_academic_year_config;
use nie_data::hash::HashId;
use serde_json::json;

const REAL: &str = "character/academic_year_config.cfg.bin.json";

fn load_json(path: &str) -> Option<serde_json::Value> {
    let path = common::chemin(path)?;
    if !path.is_file() {
        eprintln!("skip : {} absent du corpus", path.display());
        return None;
    }
    Some(serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap())
}

fn fixture() -> serde_json::Value {
    json!({ "version": "0", "lists": [
        { "name": "m_academicYearInfoList", "typeName": "x", "values": [
            { "academicYearId": "0x923E7C29", "academicYearType": 1, "academicYearNameTextId": "0x429F322D" },
            { "academicYearId": "0xEDE077B0", "academicYearType": 2, "academicYearNameTextId": "0xEA5D12E7" },
            { "academicYearId": "0x071D8FFD", "academicYearType": 3, "academicYearNameTextId": "0xD7BCC1F9" }
        ]}
    ]})
}

#[test]
fn fixture_annees() {
    let ys = parse_academic_year_config(&fixture());
    assert_eq!(ys.len(), 3);
    assert_eq!(ys[0].id, HashId::parse("0x923E7C29").unwrap());
    assert_eq!(ys[0].year_type, 1);
    assert_eq!(ys[0].name_text_id, HashId::parse("0x429F322D").unwrap());
    assert_eq!(
        [ys[0].year_type, ys[1].year_type, ys[2].year_type],
        [1, 2, 3]
    );
}

#[test]
fn golden_dump_reel() {
    let Some(root) = load_json(REAL) else {
        eprintln!("dump academic_year absent — test data-gated ignoré");
        return;
    };
    let ys = parse_academic_year_config(&root);
    assert_eq!(ys.len(), 3, "3 années scolaires");
    assert_eq!(
        ys.iter().map(|y| y.year_type).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(
        ys.iter()
            .all(|y| !y.id.is_zero() && !y.name_text_id.is_zero())
    );
}
