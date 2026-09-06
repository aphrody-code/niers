#![allow(clippy::pedantic)]
//! Golden `trigger` — scripting de déclencheurs (~287 fichiers `*_trigger`), sur de vrais dumps
//! (skip silencieux si absents). Les items `DATA_ITEM` ont une condition `var[3]` décodée via le
//! système cond/unlock_condition. Dispatch typé par SUFFIXE `_trigger`.

mod common;

use nie_data::hash::HashId;
use nie_data::trigger::parse_trigger;
use nie_data::unlock_condition::UnlockType;

fn load(rel: &str) -> Option<serde_json::Value> {
    let p = rel.to_string();
    let p = common::chemin(&p)?;
    if !p.is_file() {
        eprintln!("skip : {} absent du corpus", p.display());
        return None;
    }
    let c = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("lecture {}: {e}", p.display()));
    Some(serde_json::from_str(&c).unwrap_or_else(|e| panic!("JSON {}: {e}", p.display())))
}

#[test]
fn qsb_trigger_items_et_conditions() {
    let Some(root) = load("quest/qsb040400_trigger_0.04.78.cfg.bin.json") else {
        return;
    };
    let cfg = parse_trigger(&root);
    assert_eq!(cfg.count, 8);
    assert_eq!(cfg.items.len(), 8);
    // DATA_ITEM_0 : kind 3, target 0xD7049147, condition event-flag décodée.
    let it0 = &cfg.items[0];
    assert_eq!(it0.kind, 3);
    assert_eq!(it0.target, HashId(0xD704_9147));
    assert_eq!(it0.condition.kind, UnlockType::EventFlag);
    assert!(!it0.condition.required_events.is_empty());
    // DATA_ITEM_1 : var[3] = "0" → condition Always.
    assert_eq!(cfg.items[1].condition.kind, UnlockType::Always);
    // DATA_ITEM_3 : 3 feuilles, toutes event-flag (ns 0xBE04A598/0x2A3D4543) → EventFlag multiple.
    let it3 = &cfg.items[3];
    assert_eq!(it3.condition.kind, UnlockType::EventFlag);
    assert!(
        it3.condition.required_events.len() >= 2,
        "plusieurs event-flags requis"
    );
}

#[test]
fn fbtl_cro_trigger_se_parse() {
    let Some(root) = load("soccer/game/fbtl_cro07_120_010_trigger_0.04.78.cfg.bin.json") else {
        return;
    };
    let cfg = parse_trigger(&root);
    // Même format DATA_COUNT/DATA_ITEM ; compte cohérent avec le nombre d'items.
    assert!(cfg.count >= 0);
    assert_eq!(cfg.items.len() as i64, cfg.count, "count == nombre d'items");
    assert!(!cfg.items.is_empty(), "fbtl_cro a des items");
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_par_suffixe() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load("quest/qsb040400_trigger_0.04.78.cfg.bin.json") else {
        return;
    };
    let key = family_key("qsb040400_trigger_0.04.78.cfg.bin");
    assert!(key.ends_with("_trigger"), "clé = {key}");
    let (label, json) = decode_by_key(&key, &root).expect("dispatch suffixe câblé");
    assert_eq!(label, "trigger");
    assert_eq!(json["items"].as_array().map(Vec::len), Some(8));
}
