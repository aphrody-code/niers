//! Validation END-TO-END de `nie_data::item::resolve_name`/`resolve_description` +
//! `text::parse_text_file` : charge le vrai `item_config` et `item_text` (fr) du VFS et résout le
//! nom ET la description localisés de chaque objet (port de `buildItemDatabase` ; les descriptions
//! vivent dans `item_text` via `descId`).
//!
//! Usage : `cargo run -p nie-game --example extract_item_text`
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

fn load(vfs: &Vfs, prefix: &str, must_contain: &str) -> Option<serde_json::Value> {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.contains(must_contain)
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with(prefix) && b.ends_with(".cfg.bin"))
        })
        .min()?;
    eprintln!("  {path}");
    let bytes = vfs.read(&path).ok()?;
    let file = cfgbin::parse_t2b(&bytes).ok()?;
    Some(json!({ "entries": to_iecode(&file.entries) }))
}

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let item_root = load(&vfs, "item_config", "/gamedata/item/").expect("item_config introuvable");
    let items = nie_data::item::parse_all_items(&item_root);
    eprintln!("[item] objets = {}", items.len());
    assert!(!items.is_empty());

    let text_root = load(&vfs, "item_text", "/text/fr/").expect("item_text fr introuvable");
    let item_text = nie_data::text::parse_text_file(&text_root);
    eprintln!("[item_text fr] entrées = {}", item_text.len());

    let mut named = 0usize;
    let mut described = 0usize;
    for it in &items {
        if nie_data::item::resolve_name(it, &item_text).is_some() {
            named += 1;
        }
        if nie_data::item::resolve_description(it, &item_text).is_some() {
            described += 1;
        }
    }
    eprintln!(
        "noms résolus = {named}/{} ; descriptions résolues = {described}/{}",
        items.len(),
        items.len()
    );
    assert!(named > 0, "au moins un nom d'objet résolu");

    for it in items
        .iter()
        .filter(|i| nie_data::item::resolve_name(i, &item_text).is_some())
        .take(5)
    {
        let name = nie_data::item::resolve_name(it, &item_text).unwrap_or("");
        let desc = nie_data::item::resolve_description(it, &item_text);
        let dsnip: Option<String> = desc.map(|d| d.chars().take(50).collect());
        eprintln!(
            "  item {} → {:?}  desc={:?}",
            it.item_id.to_hex(),
            name,
            dsnip
        );
    }
    eprintln!("✓ END-TO-END OK : {named} noms / {described} descriptions d'objets résolus");
}
