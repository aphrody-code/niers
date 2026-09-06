#![allow(clippy::pedantic)]
//! Golden `event_bustup` — portraits de dialogue par chapitre, sur un vrai dump.
mod common;

use nie_data::event_bustup::parse_event_bustup_talk;
use nie_data::hash::HashId;

const PATH: &str = "event/event_bustup_talk_data_config_c23_3.00.06.cfg.bin.json";

fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    Some(serde_json::from_str(&std::fs::read_to_string(&chemin_abs).unwrap()).unwrap())
}

#[test]
fn slots_et_modeles_portrait_reels() {
    let Some(root) = load() else { return };
    let cfg = parse_event_bustup_talk(&root);
    assert_eq!(cfg.chr_slots.len(), 6, "6 slots CHR");
    assert_eq!(cfg.chr_slots[0].slot, 1);
    assert_eq!(cfg.chr_slots[0].entries.len(), 1339);
    let e0 = &cfg.chr_slots[0].entries[0];
    assert_eq!(e0.chara_id, HashId(0x3725_7569)); // 925201769
    // entrée 0 référence 2 modèles g4pk de portrait (vérité terrain).
    assert_eq!(e0.model_paths.len(), 2);
    assert!(e0.model_paths[0].ends_with(".g4pk"));
    assert!(e0.model_paths[0].contains("c000401"));
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_prefixe() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    let key = family_key("event_bustup_talk_data_config_c23_3.00.06.cfg.bin");
    assert!(
        key.starts_with("event_bustup_talk_data_config"),
        "clé={key}"
    );
    let (label, json) = decode_by_key(&key, &root).expect("dispatch câblé");
    assert_eq!(label, "event_bustup");
    assert_eq!(json["chr_slots"].as_array().map(Vec::len), Some(6));
}
