#![allow(clippy::pedantic)]
//! Golden `event_map_tag` — tags de map d'événements, sur le vrai dump.
mod common;

use nie_data::event_map_tag::parse_event_map_tag_config;
use nie_data::hash::HashId;
const PATH: &str = "event/event_map_tag_config_0.00.00.cfg.bin.json";
fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    Some(serde_json::from_str(&std::fs::read_to_string(&chemin_abs).unwrap()).unwrap())
}
#[test]
fn comptes_et_resolution() {
    let Some(root) = load() else { return };
    let cfg = parse_event_map_tag_config(&root);
    assert_eq!(cfg.tag_data.len(), 98);
    assert_eq!(cfg.tag_infos.len(), 43);
    assert_eq!(cfg.tag_ids.len(), 200);
    assert_eq!(cfg.event_settings.len(), 147);
    assert_eq!(cfg.tag_ids[0], HashId(0x4E56_90B9));
    let s = &cfg.event_settings[0];
    assert_eq!(s.event_id, HashId(0x7D1E_95AC));
    assert!(!cfg.tags_of_event(s).is_empty());
}
#[cfg(feature = "serde")]
#[test]
fn dispatch_typed() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key(
            "event_map_tag_config_0.00.00.cfg.bin.json"
                .strip_suffix(".json")
                .unwrap()
        ),
        "event_map_tag_config"
    );
    let (label, _j) = decode_by_key("event_map_tag_config", &root).expect("câblé");
    assert_eq!(label, "event_map_tag");
}
