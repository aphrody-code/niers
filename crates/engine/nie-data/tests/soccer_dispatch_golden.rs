#![allow(clippy::pedantic)]
//! Golden : les parseurs soccer existants (golden-testés) atteignent désormais azalee via le
//! dispatch typé (étaient parsés mais non routés). Data-gated.
#![cfg(feature = "serde")]

mod common;

use nie_data::typed::{decode_by_key, family_key};

fn load(name: &str) -> Option<serde_json::Value> {
    let p = std::format!("soccer/{name}");
    let p = common::chemin(&p)?;
    if !p.is_file() {
        eprintln!("skip : {} absent du corpus", p.display());
        return None;
    }
    Some(serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap())
}

#[test]
fn soccer_parsers_existants_routes() {
    let cases: [(&str, &str, &str); 3] = [
        (
            "soccer_focus_battle_effect_config.cfg.bin.json",
            "soccer_focus_battle_effect_config",
            "soccer_focus_battle",
        ),
        (
            "soccer_technic_config.cfg.bin.json",
            "soccer_technic_config",
            "soccer_technic",
        ),
        (
            "soccer_game_additional_config_1.04.14.00.cfg.bin.json",
            "soccer_game_additional_config",
            "soccer_game_additional",
        ),
    ];
    for (file, expect_key, expect_label) in cases {
        let Some(root) = load(file) else { continue };
        assert_eq!(family_key(file), expect_key, "clé de {file}");
        let (label, _json) =
            decode_by_key(expect_key, &root).unwrap_or_else(|| panic!("{expect_key} non routé"));
        assert_eq!(label, expect_label);
    }
}
