//! Export industrialisé de la **base de quêtes** (pilier B′3) : lit le jeu réel (`quest_config` +
//! `quest_title_text` fr) via le VFS, résout le titre localisé de chaque quête, et écrit un JSON
//! stable (azalee/wiki). Complète les 4 bases (personnages/Avatar/objets/succès).
//!
//! Usage : `cargo run -p nie-game --example export_quests -- [out.json]`
//! (défaut : `var/quests-resolved.json`).
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
        .unwrap_or_else(|| "var/quests-resolved.json".into());
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let quests = nie_data::quest::parse_quest_config(&load(
        &vfs,
        |p| {
            p.contains("/gamedata/quest/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("quest_config") && b.ends_with(".cfg.bin"))
        },
        "quest_config",
    ));
    let titles = nie_data::text::parse_text_file(&load(
        &vfs,
        |p| {
            p.contains("/fr/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("quest_title_text") && b.ends_with(".cfg.bin"))
        },
        "quest_title_text fr",
    ));

    let mut list: Vec<serde_json::Value> = Vec::new();
    for q in &quests {
        list.push(json!({
            "questId": q.quest_id.to_hex(),
            "phase": q.phase,
            "questType": q.quest_type,
            "title": nie_data::quest::resolve_title(q, &titles),
            "image": q.image,
        }));
    }

    let doc = json!({
        "schema": "niers/quests-resolved/v1",
        "locale": "fr",
        "count": list.len(),
        "quests": list,
    });
    let txt = serde_json::to_string_pretty(&doc).expect("serialize");
    std::fs::create_dir_all(Path::new(&out).parent().unwrap_or(Path::new("."))).ok();
    std::fs::write(&out, &txt).unwrap_or_else(|e| panic!("écriture {out} : {e}"));
    let resolved = quests
        .iter()
        .filter(|q| nie_data::quest::resolve_title(q, &titles).is_some())
        .count();
    eprintln!(
        "✓ export-quests: {} quêtes ({resolved} titres résolus) → {out} ({} octets)",
        doc["count"],
        txt.len()
    );
}
