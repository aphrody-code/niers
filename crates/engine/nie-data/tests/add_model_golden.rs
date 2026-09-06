#![allow(clippy::pedantic)]
//! Golden `add_model` — `data/common/gamedata/character/add_model_config.cfg.bin.json`.

mod common;

extern crate std;

use nie_data::add_model::parse_add_model_config;
use nie_data::hash::HashId;
use serde_json::json;

const REAL: &str = "character/add_model_config.cfg.bin.json";

fn load(path: &str) -> Option<serde_json::Value> {
    std::path::Path::new(path)
        .exists()
        .then(|| serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
}

#[test]
fn fixture_add_model() {
    let v = json!({ "version": "0", "lists": [
        { "name": "m_addModelDataList", "typeName": "x", "values": [
            { "addModelType": 1, "addModelId": "0x88F99320" }
        ]},
        { "name": "m_addModelInfoList", "typeName": "x", "values": [
            { "baseCharaId": "0xEE2D74D5", "addModel": [0, 1] }
        ]}
    ]});
    let c = parse_add_model_config(&v);
    assert_eq!(c.data.len(), 1);
    assert_eq!(c.data[0].add_model_type, 1);
    assert_eq!(c.data[0].add_model_id, HashId::parse("0x88F99320").unwrap());
    assert_eq!(c.info.len(), 1);
    assert_eq!(
        c.info[0].base_chara_id,
        HashId::parse("0xEE2D74D5").unwrap()
    );
    assert_eq!((c.info[0].data_offset, c.info[0].data_count), (0, 1));
}

#[test]
fn golden_dump_reel() {
    let Some(root) = load(REAL) else {
        eprintln!("dump absent — ignoré");
        return;
    };
    let c = parse_add_model_config(&root);
    assert_eq!(c.data.len(), 7, "7 modèles ajoutables");
    assert_eq!(c.info.len(), 7, "7 liaisons perso de base");
    // Chaque plage Info ⊆ data list.
    for i in &c.info {
        assert!(
            (i.data_offset + i.data_count) as usize <= c.data.len(),
            "plage hors data"
        );
        assert!(!i.base_chara_id.is_zero());
    }
}
