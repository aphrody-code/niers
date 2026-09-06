#![allow(clippy::pedantic)]
//! Tests golden `talk_select` — menus de choix de dialogue d'IEVR, sur le vrai dump
//! (skip silencieux si absent — fragment copyright Level-5 non committé) :
//!
//! - `data/common/gamedata/system/talk_select_config.cfg_1.01.41.00.cfg.bin.json`
//!
//! Port direct depuis la structure du dump ; valeurs assérées copiées octet pour octet.

mod common;

use nie_data::hash::HashId;
use nie_data::talk_select::parse_talk_select_config;

const PATH: &str = "system/talk_select_config.cfg_1.01.41.00.cfg.bin.json";

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
fn neuf_menus_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_talk_select_config(&root);
    assert_eq!(cfg.entries.len(), 9);
    let e0 = &cfg.entries[0];
    assert_eq!(e0.id, HashId(0x30B9_C867));
    assert_eq!(e0.text_id, HashId(0x01DE_969B));
    assert_eq!(e0.kind, 1);
    assert_eq!(e0.cursor_init_pos, 0);
    assert_eq!(e0.cursor_cancel_pos, 1);
    assert_eq!(e0.after_decision_param, [0, 0]);
    let e8 = &cfg.entries[8];
    assert_eq!(e8.id, HashId(0xC04F_DBE7));
    assert_eq!(e8.text_id, HashId(0x742E_7D20));
    assert_eq!(e8.kind, 5);
    assert_eq!(e8.cursor_cancel_pos, 2);
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_atteint_azalee() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    // family_key garde le `.cfg` du nom de fichier inhabituel `talk_select_config.cfg_<ver>.cfg.bin`.
    assert_eq!(
        family_key("talk_select_config.cfg_1.01.41.00.cfg.bin"),
        "talk_select_config.cfg"
    );
    let (label, json) =
        decode_by_key("talk_select_config.cfg", &root).expect("famille typée câblée");
    assert_eq!(label, "talk_select");
    assert_eq!(json["entries"].as_array().map(Vec::len), Some(9));
}
