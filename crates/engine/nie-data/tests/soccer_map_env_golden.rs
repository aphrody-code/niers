#![allow(clippy::pedantic)]
//! Golden `soccer_map_env` — environnement de match, sur le vrai dump.
mod common;

use nie_data::hash::HashId;
use nie_data::soccer_map_env::parse_soccer_map_env_config;
const PATH: &str = "soccer/soccer_game_map_enviroment_config_1.02.92.00.cfg.bin.json";
fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    Some(serde_json::from_str(&std::fs::read_to_string(&chemin_abs).unwrap()).unwrap())
}
#[test]
fn env_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_soccer_map_env_config(&root);
    assert_eq!(cfg.tag_data.len(), 28);
    assert_eq!(cfg.envs.len(), 29);
    let e = &cfg.envs[0];
    assert_eq!(e.config_id, HashId(0xE35E_00DF));
    assert_eq!(e.hour, 15);
    assert_eq!(e.weather, 1);
}
#[cfg(feature = "serde")]
#[test]
fn dispatch_typed() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key(
            "soccer_game_map_enviroment_config_1.02.92.00.cfg.bin.json"
                .strip_suffix(".json")
                .unwrap()
        ),
        "soccer_game_map_enviroment_config"
    );
    let (label, _j) = decode_by_key("soccer_game_map_enviroment_config", &root).expect("câblé");
    assert_eq!(label, "soccer_map_env");
}
