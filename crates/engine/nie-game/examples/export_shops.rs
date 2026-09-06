//! Export industrialisé de la **base de boutiques** (pilier B′3) : lit le jeu réel (`shop_config` +
//! `shop_text` fr) via le VFS, résout le nom de chaque boutique et liste ses items (références vers
//! la base d'objets), et écrit un JSON stable (azalee/wiki).
//!
//! Usage : `cargo run -p nie-game --example export_shops -- [out.json]`
//! (défaut : `var/shops-resolved.json`).
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
        .unwrap_or_else(|| "var/shops-resolved.json".into());
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let shops = nie_data::shop::parse_shop_config(&load(
        &vfs,
        |p| {
            p.contains("/gamedata/shop/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("shop_config") && b.ends_with(".cfg.bin"))
        },
        "shop_config",
    ));
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

    use nie_data::shop::resolve_name;
    let mut list: Vec<serde_json::Value> = Vec::new();
    for s in &shops {
        let Some(name) = resolve_name(s, &text) else {
            continue;
        };
        list.push(json!({
            "shopId": s.shop_id.to_hex(),
            "name": name,
            "itemCount": s.items.len(),
            "items": s.items.iter().map(|i| i.to_hex()).collect::<Vec<_>>(),
        }));
    }

    let doc = json!({
        "schema": "niers/shops-resolved/v1",
        "locale": "fr",
        "count": list.len(),
        "shops": list,
    });
    let txt = serde_json::to_string_pretty(&doc).expect("serialize");
    std::fs::create_dir_all(Path::new(&out).parent().unwrap_or(Path::new("."))).ok();
    std::fs::write(&out, &txt).unwrap_or_else(|e| panic!("écriture {out} : {e}"));
    eprintln!(
        "✓ export-shops: {} boutiques résolues → {out} ({} octets)",
        doc["count"],
        txt.len()
    );
}
