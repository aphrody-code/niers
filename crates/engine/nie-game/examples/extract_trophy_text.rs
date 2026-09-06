//! Validation END-TO-END de `nie_data::trophy::resolve_name`/`resolve_description`/`decode_unlock`
//! avec `text::parse_text_file` : charge le vrai `trophy_config` et `trophy_text` (fr) du VFS et
//! résout nom, description et condition de déblocage de chaque trophée.
//!
//! Usage : `cargo run -p nie-game --example extract_trophy_text`
use nie_formats::cfgbin::{self, CfgEntry, Value};
use nie_formats::vfs::Vfs;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;

fn to_iecode(siblings: &[CfgEntry]) -> Vec<serde_json::Value> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    siblings
        .iter()
        .map(|e| {
            let idx = counts.entry(e.name.as_str()).or_insert(0);
            let name = format!("{}_{}", e.name, *idx);
            *idx += 1;
            let variables: Vec<serde_json::Value> = e
                .variables
                .iter()
                .map(|v| match v {
                    Value::String(s) => json!({ "type": "String", "value": s }),
                    Value::Int(n) => json!({ "type": "Int", "value": n.to_string() }),
                    Value::Float(f) => json!({ "type": "Float", "value": f.to_string() }),
                })
                .collect();
            json!({ "name": name, "variables": variables, "children": to_iecode(&e.children) })
        })
        .collect()
}

fn load(vfs: &Vfs, pred: impl Fn(&str) -> bool, what: &str) -> serde_json::Value {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| pred(p))
        .min()
        .unwrap_or_else(|| panic!("{what} introuvable"));
    eprintln!("  {path}");
    let file = cfgbin::parse_t2b(&vfs.read(&path).expect("read")).expect("parse");
    json!({ "entries": to_iecode(&file.entries) })
}

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let cfg = load(
        &vfs,
        |p| {
            p.contains("/gamedata/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("trophy_config") && b.ends_with(".cfg.bin"))
        },
        "trophy_config",
    );
    let config = nie_data::trophy::parse_trophy_config(&cfg);
    eprintln!("[trophy] trophées = {}", config.infos.len());
    assert!(!config.infos.is_empty());

    let text = nie_data::text::parse_text_file(&load(
        &vfs,
        |p| {
            p.contains("/fr/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("trophy_text") && b.ends_with(".cfg.bin"))
        },
        "trophy_text fr",
    ));
    eprintln!("[trophy_text fr] entrées = {}", text.len());

    use nie_data::trophy::{decode_unlock, resolve_description, resolve_name};
    let named = config
        .infos
        .iter()
        .filter(|t| resolve_name(t, &text).is_some())
        .count();
    let described = config
        .infos
        .iter()
        .filter(|t| resolve_description(t, &text).is_some())
        .count();
    eprintln!(
        "noms résolus = {named}/{} ; descriptions = {described}",
        config.infos.len()
    );
    assert!(named > 0, "au moins un nom de trophée résolu");

    // Distribution des types de condition de déblocage (open_cond décodé).
    let mut kinds: HashMap<&str, usize> = HashMap::new();
    for t in &config.infos {
        let c = decode_unlock(t);
        let k = match c.kind {
            nie_data::unlock_condition::UnlockType::Always => "always",
            nie_data::unlock_condition::UnlockType::Story => "story",
            nie_data::unlock_condition::UnlockType::EventFlag => "eventFlag",
            nie_data::unlock_condition::UnlockType::Composite => "composite",
        };
        *kinds.entry(k).or_default() += 1;
    }
    eprintln!("conditions de déblocage = {kinds:?}");

    for t in config
        .infos
        .iter()
        .filter(|t| resolve_name(t, &text).is_some())
        .take(5)
    {
        let n = resolve_name(t, &text).unwrap_or("");
        let d = resolve_description(t, &text).map(|s| s.chars().take(45).collect::<String>());
        eprintln!(
            "  trophy {} ({}) → {:?}  desc={:?}",
            t.trophy_id.to_hex(),
            t.code,
            n,
            d
        );
    }
    eprintln!(
        "✓ END-TO-END OK : {named} noms / {described} descriptions de trophées résolus + conditions décodées"
    );
}
