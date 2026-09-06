#![allow(clippy::pedantic)]
//! Tests golden `post::password_list` — valeurs réelles tirées de :
//! `post/password_list_config.cfg.bin.json`
//!
//! Layout `lists` : 2 listes `m_Codes` (`PASSWARD_CODES`, typo Level-5) et `info`
//! (`PASSWARD_DATA`). Les codes y sont **hashés** (9 langues, même hash sur le dump réel),
//! contrairement à `delivery_config` où ce sont des chaînes littérales. La clé `"french "`
//! porte un espace final (typo Level-5).

mod common;

use nie_data::hash::HashId;
use nie_data::post::{PasswordListConfig, parse_password_list_config};

const REAL_PATH: &str = "post/password_list_config.cfg.bin.json";

fn load_real() -> Option<PasswordListConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_password_list_config(&root))
}

#[test]
fn comptes_listes() {
    let Some(cfg) = load_real() else { return };
    assert_eq!(cfg.codes.len(), 1, "m_Codes");
    assert_eq!(cfg.data.len(), 1, "info");
}

#[test]
fn codes_toutes_langues_meme_hash() {
    let Some(cfg) = load_real() else { return };
    let c = &cfg.codes[0];
    let h = HashId(0xD853_DAB8);
    assert_eq!(c.japanese, h);
    assert_eq!(c.english, h);
    assert_eq!(c.portuguese, h);
    // clé "french " avec espace final (typo Level-5)
    assert_eq!(c.french, h);
    assert_eq!(c.italian, h);
    assert_eq!(c.german, h);
    assert_eq!(c.spanish, h);
    assert_eq!(c.traditional_chinese, h);
    assert_eq!(c.simplified_chinese, h);
}

#[test]
fn data_entree0() {
    let Some(cfg) = load_real() else { return };
    let d = &cfg.data[0];
    assert_eq!(d.id, HashId(0xC4FD_CF84));
    assert_eq!(d.text_id, HashId(0xC4FD_CF84));
    assert_eq!(d.flag_id, HashId(0xC4FD_CF84));
    assert_eq!(d.conditions, "AAAAAA8FNbkZNtoAAQAyAAAAAHg=");
    assert_eq!(d.codes_offset, 0);
    assert_eq!(d.codes_count, 1);
}
