#![allow(clippy::pedantic)]
//! Tests golden `trial_take_over` — report de progression d'essai d'IEVR, sur le vrai dump
//! (skip silencieux si absent — fragment copyright Level-5 non committé) :
//!
//! - `data/common/gamedata/system/trial_take_over_config.cfg.bin.json`

mod common;

use nie_data::hash::HashId;
use nie_data::trial_take_over::parse_trial_take_over_config;

const PATH: &str = "system/trial_take_over_config.cfg.bin.json";

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
fn comptes_et_valeurs_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_trial_take_over_config(&root);
    assert_eq!(cfg.take_over.len(), 5);
    assert_eq!(cfg.part_take_over.len(), 9);
    assert_eq!(cfg.take_over[0].id, HashId(0x4689_6BAB));
    assert_eq!(cfg.take_over[0].flag_no, 0);
    assert_eq!(cfg.take_over[0].condition, "");
    let last = &cfg.take_over[4];
    assert_eq!(last.id, HashId(0x41E4_AFB2));
    assert_eq!(last.flag_no, 4);
    assert!(
        !last.condition.is_empty(),
        "condition base64 non vide pour le dernier"
    );
    assert_eq!(cfg.part_take_over[0].id, HashId(0x26EA_0F62));
    assert_eq!(cfg.part_take_over[8].id, HashId(0x2831_8750));
    assert_eq!(cfg.part_take_over[8].flag_no, 8);
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_atteint_azalee() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key("trial_take_over_config.cfg.bin"),
        "trial_take_over_config"
    );
    let (label, json) =
        decode_by_key("trial_take_over_config", &root).expect("famille typée câblée");
    assert_eq!(label, "trial_take_over");
    assert_eq!(json["take_over"].as_array().map(Vec::len), Some(5));
    assert_eq!(json["part_take_over"].as_array().map(Vec::len), Some(9));
}

#[test]
fn condition_decode_reel() {
    use nie_data::unlock_condition::UnlockType;
    let Some(root) = load() else { return };
    let cfg = parse_trial_take_over_config(&root);
    // [0] : condition vide ⇒ Always.
    assert_eq!(cfg.take_over[0].decode_condition().kind, UnlockType::Always);
    // [4] : condition event-flag (non vide), au moins une feuille requise.
    let c4 = cfg.take_over[4].decode_condition();
    assert_eq!(c4.kind, UnlockType::EventFlag);
    assert!(!c4.required_events.is_empty(), "feuille event-flag décodée");
}
