//! Export industrialisé de la **base d'objets** (pilier B′3) : lit le jeu réel (`item_config` +
//! `item_text` fr) via le VFS, résout nom + description + catégorie + prix + stats de chaque objet,
//! et écrit un JSON stable (azalee/wiki). N'exporte que les objets à nom localisé (roster réel).
//!
//! Usage : `cargo run -p nie-game --example export_items -- [out.json]`
//! (défaut : `var/items-resolved.json`).
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
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "var/items-resolved.json".into());
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let items = nie_data::item::parse_all_items(&load(
        &vfs,
        |p| {
            p.contains("/gamedata/item/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("item_config") && b.ends_with(".cfg.bin"))
        },
        "item_config",
    ));
    let text = nie_data::text::parse_text_file(&load(
        &vfs,
        |p| p.contains("/fr/") && p.rsplit('/').next() == Some("item_text.cfg.bin"),
        "item_text fr",
    ));

    use nie_data::item::{resolve_description, resolve_name};
    let mut list: Vec<serde_json::Value> = Vec::new();
    for it in &items {
        let Some(name) = resolve_name(it, &text) else {
            continue;
        };
        list.push(json!({
            "itemId": it.item_id.to_hex(),
            "category": it.category.as_str(),
            "code": it.internal_code,
            "name": name,
            "description": resolve_description(it, &text),
            "price": it.price,
            "stat1": it.stats.as_ref().map(|s| s.stat1),
            "stat2": it.stats.as_ref().map(|s| s.stat2),
        }));
    }

    let doc = json!({
        "schema": "niers/items-resolved/v1",
        "locale": "fr",
        "count": list.len(),
        "items": list,
    });
    let txt = serde_json::to_string_pretty(&doc).expect("serialize");
    std::fs::create_dir_all(Path::new(&out).parent().unwrap_or(Path::new("."))).ok();
    std::fs::write(&out, &txt).unwrap_or_else(|e| panic!("écriture {out} : {e}"));
    eprintln!(
        "✓ export-items: {} objets résolus → {out} ({} octets)",
        doc["count"],
        txt.len()
    );
}
