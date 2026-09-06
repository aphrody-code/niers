#![allow(clippy::pedantic)]
//! Golden `soccer_fixed_reward` — récompenses d'esprits de match, sur le vrai dump.
mod common;

use nie_data::hash::HashId;
use nie_data::soccer_fixed_reward::parse_soccer_fixed_reward_config;

const PATH: &str = "soccer/soccer_fixed_reward_spirit_config_1.02.11.00.cfg.bin.json";

fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    Some(serde_json::from_str(&std::fs::read_to_string(&chemin_abs).unwrap()).unwrap())
}

#[test]
fn rewards_et_resolution() {
    let Some(root) = load() else { return };
    let cfg = parse_soccer_fixed_reward_config(&root);
    assert_eq!(cfg.fixed_data.len(), 42);
    assert_eq!(cfg.fixed_infos.len(), 11);
    assert_eq!(cfg.fixed_data[0].chara_id, HashId(0x99A1_C150));
    assert_eq!(cfg.fixed_data[0].rarity, 3);
    let info = &cfg.fixed_infos[0];
    assert_eq!(info.id, HashId(0x6184_489B));
    assert_eq!(cfg.spirits_of_fixed(info).len(), 4); // spiritData [0,4]
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key(
            "soccer_fixed_reward_spirit_config_1.02.11.00.cfg.bin.json"
                .strip_suffix(".json")
                .unwrap()
        ),
        "soccer_fixed_reward_spirit_config"
    );
    let (label, json) = decode_by_key("soccer_fixed_reward_spirit_config", &root).expect("câblé");
    assert_eq!(label, "soccer_fixed_reward");
    assert_eq!(json["fixed_data"].as_array().map(Vec::len), Some(42));
}
