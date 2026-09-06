//! Validation END-TO-END de `nie_data::aura::resolve_name`/`resolve_description` + `text::parse_text_file` :
//! charge le vrai `aura_skill_config` et `skill_text` (fr) du VFS et résout le nom (+ description)
//! de chaque aura (Avatar / Keshin / hissatsu) via la table `skill_text` (`name_id`/`desc_id`).
//!
//! Usage : `cargo run -p nie-game --example extract_aura_names`
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
                    .is_some_and(|b| b.starts_with("aura_skill_config") && b.ends_with(".cfg.bin"))
        },
        "aura_skill_config",
    );
    let auras = nie_data::aura::parse_all_aura_cmds(&cfg);
    eprintln!("[aura] auras = {}", auras.len());
    assert!(!auras.is_empty());

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
    eprintln!("[skill_text fr] entrées = {}", text.len());

    use nie_data::aura::{resolve_description, resolve_name};
    let named = auras
        .iter()
        .filter(|a| resolve_name(a, &text).is_some())
        .count();
    let described = auras
        .iter()
        .filter(|a| resolve_description(a, &text).is_some())
        .count();
    eprintln!(
        "noms d'aura résolus = {named}/{} ; descriptions = {described}",
        auras.len()
    );
    assert_eq!(
        named,
        auras.len(),
        "toutes les auras ont un nom (vérifié 443/443)"
    );

    for a in auras.iter().take(6) {
        let n = resolve_name(a, &text).unwrap_or("");
        let d = resolve_description(a, &text).map(|s| s.chars().take(40).collect::<String>());
        eprintln!(
            "  aura {} ({}) → {:?}  desc={:?}",
            a.aura_id.to_hex(),
            a.asset_code,
            n,
            d
        );
    }
    eprintln!(
        "✓ END-TO-END OK : {named}/{} noms d'aura (Avatar/Keshin) résolus",
        auras.len()
    );
}
