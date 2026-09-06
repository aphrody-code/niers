#![allow(clippy::pedantic)]
//! Tests golden `system_unlock` — gating de progression (fenêtres « système débloqué »), sur le
//! vrai dump (skip silencieux si absent — fragment copyright Level-5 non committé) :
//!
//! - `data/common/gamedata/system/system_unlock_window_config_0.00.00.00.cfg.bin.json`
//!
//! Port direct depuis la structure du dump (pas de référence inagle) ; valeurs assérées copiées
//! octet pour octet.

mod common;

use nie_data::hash::HashId;
use nie_data::system_unlock::parse_system_unlock_window_config;

const PATH: &str = "system/system_unlock_window_config_0.00.00.00.cfg.bin.json";

fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("lecture {}: {e}", chemin_abs.display()));
    Some(
        serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("JSON {}: {e}", chemin_abs.display())),
    )
}

#[test]
fn comptes_des_deux_listes() {
    let Some(root) = load() else { return };
    let cfg = parse_system_unlock_window_config(&root);
    assert_eq!(cfg.infos.len(), 98);
    assert_eq!(cfg.sets.len(), 26);
}

#[test]
fn info_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_system_unlock_window_config(&root);
    let i0 = &cfg.infos[0];
    assert_eq!(i0.text_id, HashId(0x395A_EAB5));
    assert_eq!(i0.tag_type_id, 1);
    assert_eq!(i0.replace_id, HashId(0xEBBE_E959));
    assert_eq!(i0.gaiji_icon_crc, HashId(0xA6F6_14F6));
    let i97 = &cfg.infos[97];
    assert_eq!(i97.text_id, HashId(0xCA0B_59FA));
    assert_eq!(i97.tag_type_id, 2);
    assert_eq!(i97.replace_id, HashId(0x41B2_14AA));
    assert_eq!(i97.gaiji_icon_crc, HashId(0x48F8_75DA));
}

#[test]
fn set_et_resolution_de_tranche() {
    let Some(root) = load() else { return };
    let cfg = parse_system_unlock_window_config(&root);
    assert_eq!(cfg.sets[0].id, HashId(0x23CA_11FE));
    assert_eq!(cfg.sets[0].info_slice, [0, 1]);
    assert_eq!(cfg.sets[25].id, HashId(0x38CF_853B));
    assert_eq!(cfg.sets[25].info_slice, [88, 10]);
    // Résolution : set[25] → 10 infos (88..98).
    let slice = cfg.resolve_set(HashId(0x38CF_853B)).expect("set présent");
    assert_eq!(slice.len(), 10);
    assert_eq!(slice[9].text_id, cfg.infos[97].text_id);
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_atteint_azalee() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key("system_unlock_window_config_0.00.00.00.cfg.bin"),
        "system_unlock_window_config"
    );
    let (label, json) =
        decode_by_key("system_unlock_window_config", &root).expect("famille typée câblée");
    assert_eq!(label, "system_unlock_window");
    assert_eq!(json["infos"].as_array().map(Vec::len), Some(98));
    assert_eq!(json["sets"].as_array().map(Vec::len), Some(26));
}
