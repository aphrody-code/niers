#![allow(clippy::pedantic)]
//! Golden `add_content_equip` — `data/common/gamedata/system/add_content_equip_config.cfg.bin.json`.

mod common;

extern crate std;

use nie_data::add_content_equip::parse_add_content_equip_config;
use nie_data::hash::HashId;
use serde_json::json;

const REAL: &str = "system/add_content_equip_config.cfg.bin.json";

fn load(path: &str) -> Option<serde_json::Value> {
    std::path::Path::new(path)
        .exists()
        .then(|| serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
}

#[test]
fn fixture_aoc_equip() {
    let v = json!({ "version": "0", "lists": [
        { "name": "m_aocEquipConfigInfo", "typeName": "x", "values": [
            { "aocCondition": "AAAAABgFNZ8=", "equipID": "0xC5A27A3C" },
            { "aocCondition": "AAAAABgFNZ8=", "equipID": "0xEE8F29FF" }
        ]}
    ]});
    let es = parse_add_content_equip_config(&v);
    assert_eq!(es.len(), 2);
    assert_eq!(es[0].equip_id, HashId::parse("0xC5A27A3C").unwrap());
    assert!(!es[0].aoc_condition.is_empty());
}

#[test]
fn golden_dump_reel() {
    let Some(root) = load(REAL) else {
        eprintln!("dump absent — ignoré");
        return;
    };
    let es = parse_add_content_equip_config(&root);
    assert_eq!(es.len(), 22, "22 équipements AOC");
    assert_eq!(es[0].equip_id, HashId::parse("0xC5A27A3C").unwrap());
    assert!(
        es.iter()
            .all(|e| !e.equip_id.is_zero() && !e.aoc_condition.is_empty())
    );
}
