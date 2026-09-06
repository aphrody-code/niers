//! Validation END-TO-END de `nie_data::shop::resolve_name` + `text::parse_text_file` : charge le
//! vrai `shop_config` et `shop_text` (fr) du VFS et résout le nom localisé de chaque boutique
//! (modèle documenté inagle : `name_hash` → `shop_text`).
//!
//! Usage : `cargo run -p nie-game --example extract_shop_text`
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
                    .is_some_and(|b| b.starts_with("shop_config") && b.ends_with(".cfg.bin"))
        },
        "shop_config",
    );
    let shops = nie_data::shop::parse_shop_config(&cfg);
    eprintln!("[shop] boutiques = {}", shops.len());
    assert!(!shops.is_empty());

    let text = nie_data::text::parse_text_file(&load(
        &vfs,
        |p| {
            p.contains("/fr/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("shop_text") && b.ends_with(".cfg.bin"))
        },
        "shop_text fr",
    ));
    eprintln!("[shop_text fr] entrées = {}", text.len());

    let named = shops
        .iter()
        .filter(|s| nie_data::shop::resolve_name(s, &text).is_some())
        .count();
    eprintln!("noms de boutique résolus = {named}/{}", shops.len());
    assert!(named > 0, "au moins un nom de boutique résolu");

    for s in shops
        .iter()
        .filter(|s| nie_data::shop::resolve_name(s, &text).is_some())
        .take(8)
    {
        let n = nie_data::shop::resolve_name(s, &text).unwrap_or("");
        eprintln!(
            "  shop {} ({} items) → {:?}",
            s.shop_id.to_hex(),
            s.items.len(),
            n
        );
    }
    eprintln!("✓ END-TO-END OK : {named} noms de boutique résolus");
}
