#![allow(clippy::pedantic)]
//! Golden `soccer_rank` — classement de match, sur le vrai dump.
mod common;

use nie_data::hash::HashId;
use nie_data::soccer_rank::parse_soccer_rank_config;
const PATH: &str = "soccer/soccer_rank_config_0.00.00.cfg.bin.json";
fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    Some(serde_json::from_str(&std::fs::read_to_string(&chemin_abs).unwrap()).unwrap())
}
#[test]
fn ranks_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_soccer_rank_config(&root);
    assert_eq!(cfg.ranks.len(), 4);
    assert_eq!(cfg.ranks[0].id, HashId(0x24F0_0406));
    assert_eq!(cfg.ranks[0].next_rank_point, 400);
}
#[cfg(feature = "serde")]
#[test]
fn dispatch_typed() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key(
            "soccer_rank_config_0.00.00.cfg.bin.json"
                .strip_suffix(".json")
                .unwrap()
        ),
        "soccer_rank_config"
    );
    let (label, _j) = decode_by_key("soccer_rank_config", &root).expect("câblé");
    assert_eq!(label, "soccer_rank");
}
