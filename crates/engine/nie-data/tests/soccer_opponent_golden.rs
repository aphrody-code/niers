#![allow(clippy::pedantic)]
//! Golden `soccer_opponent` — adversaires de match + conditions de déblocage, sur le vrai dump.
mod common;

use nie_data::hash::HashId;
use nie_data::soccer_opponent::parse_soccer_opponent_config;

const PATH: &str = "soccer/soccer_opponent_info_0.00.00.cfg.bin.json";

fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    Some(serde_json::from_str(&std::fs::read_to_string(&chemin_abs).unwrap()).unwrap())
}

#[test]
fn opponents_et_conditions() {
    let Some(root) = load() else { return };
    let cfg = parse_soccer_opponent_config(&root);
    assert_eq!(cfg.opponents.len(), 154);
    let o0 = &cfg.opponents[0];
    assert_eq!(o0.battle_id, HashId(0xA57E_ED32));
    assert_eq!(o0.sort_order, 10);
    // les conds non vides sont décodées (l'adversaire 0 a des conditions de déblocage).
    assert!(!o0.conditions.is_empty(), "conditions décodées");
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key("soccer_opponent_info_0.00.00.cfg.bin"),
        "soccer_opponent_info"
    );
    let (label, json) = decode_by_key("soccer_opponent_info", &root).expect("câblé");
    assert_eq!(label, "soccer_opponent");
    assert_eq!(json["opponents"].as_array().map(Vec::len), Some(154));
}
