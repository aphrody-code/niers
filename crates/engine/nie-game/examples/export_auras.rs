//! Export industrialisé de la **base Avatar / Keshin** (pilier B′3) : lit le jeu réel
//! (`aura_skill_config` + `skill_text` fr) via le VFS, résout nom + description + élément de chaque
//! aura, et écrit un JSON stable (azalee/wiki). Le contenu signature d'IEVR.
//!
//! Usage : `cargo run -p nie-game --example export_auras -- [out.json]`
//! (défaut : `var/auras-resolved.json`).
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
        .unwrap_or_else(|| "var/auras-resolved.json".into());
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
                    .is_some_and(|b| b.starts_with("aura_skill_config") && b.ends_with(".cfg.bin"))
        },
        "aura_skill_config",
    );
    let auras = nie_data::aura::parse_all_aura_cmds(&cfg);
    let text = nie_data::text::parse_text_file(&load(
        &vfs,
        |p| {
            p.contains("/fr/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("skill_text") && b.ends_with(".cfg.bin"))
        },
        "skill_text fr",
    ));

    use nie_data::aura::{resolve_description, resolve_name};
    let mut list: Vec<serde_json::Value> = Vec::new();
    for a in &auras {
        let Some(name) = resolve_name(a, &text) else {
            continue;
        };
        list.push(json!({
            "auraId": a.aura_id.to_hex(),
            "assetCode": a.asset_code,
            "name": name,
            "description": resolve_description(a, &text),
            "element": format!("{:?}", a.element()),
            "subType": format!("{:?}", a.sub_type),
        }));
    }

    let doc = json!({
        "schema": "niers/auras-resolved/v1",
        "locale": "fr",
        "count": list.len(),
        "auras": list,
    });
    let txt = serde_json::to_string_pretty(&doc).expect("serialize");
    std::fs::create_dir_all(Path::new(&out).parent().unwrap_or(Path::new("."))).ok();
    std::fs::write(&out, &txt).unwrap_or_else(|e| panic!("écriture {out} : {e}"));
    eprintln!(
        "✓ export-auras: {} Avatar/Keshin résolus → {out} ({} octets)",
        doc["count"],
        txt.len()
    );
}
