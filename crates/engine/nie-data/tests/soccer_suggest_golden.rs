#![allow(clippy::pedantic)]
//! Golden `soccer_suggest` — système de suggestions de match, sur le vrai dump.
mod common;

use nie_data::hash::HashId;
use nie_data::soccer_suggest::parse_soccer_suggest_config;

const PATH: &str = "soccer/soccer_suggest_config_0.01.92.cfg.bin.json";

fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    Some(serde_json::from_str(&std::fs::read_to_string(&chemin_abs).unwrap()).unwrap())
}

#[test]
fn comptes_et_valeurs_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_soccer_suggest_config(&root);
    assert_eq!(cfg.suggests.len(), 5);
    assert_eq!(cfg.cameras.len(), 1);
    assert_eq!(cfg.predict_mots.len(), 5);
    assert_eq!(cfg.predict_phases.len(), 5);
    assert_eq!(cfg.predict_objects.len(), 5);
    assert_eq!(cfg.predict_infos.len(), 5);
    assert_eq!(cfg.pass_extension_data.len(), 132);
    assert_eq!(cfg.pass_extension_infos.len(), 12);
    // suggest[0] byte-exact.
    let s = &cfg.suggests[0];
    assert_eq!(s.id, HashId(0xEC13_A97C));
    assert_eq!(s.cmd_id, HashId(0x9178_C3FE));
    assert!(s.is_def_elected);
    assert_eq!(s.cost, 1);
    assert_eq!(s.counter_suggest, HashId(0xABB3_D3AC));
    // caméra.
    assert_eq!(cfg.cameras[0].distance, 10);
    assert_eq!(cfg.cameras[0].azimuth, 30);
    // pass-extension : zone 0 → 11 cibles.
    assert_eq!(cfg.pass_extension_infos[0].slice, [0, 11]);
    assert_eq!(cfg.pass_extension_data[0].text_id, HashId(0xF323_A876));
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key("soccer_suggest_config_0.01.92.cfg.bin"),
        "soccer_suggest_config"
    );
    let (label, json) = decode_by_key("soccer_suggest_config", &root).expect("câblé");
    assert_eq!(label, "soccer_suggest");
    assert_eq!(
        json["pass_extension_data"].as_array().map(Vec::len),
        Some(132)
    );
}
