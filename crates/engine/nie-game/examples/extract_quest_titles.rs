//! Validation END-TO-END de `nie_data::quest::resolve_title` + `text::parse_text_file` : charge
//! le vrai `quest_config` et le `quest_title_text` (fr) du VFS, et résout le titre localisé de
//! chaque quête par jointure de hash (port de `buildQuestDatabase`).
//!
//! Usage : `cargo run -p nie-game --example extract_quest_titles`
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

    let quest_root =
        load(&vfs, "quest_config", "/gamedata/quest/").expect("quest_config introuvable");
    let quests = nie_data::quest::parse_quest_config(&quest_root);
    eprintln!("[quest] quêtes = {}", quests.len());
    assert!(!quests.is_empty());

    let title_root =
        load(&vfs, "quest_title_text", "/text/fr/").expect("quest_title_text fr introuvable");
    let titles = nie_data::text::parse_text_file(&title_root);
    eprintln!("[quest_title_text fr] entrées = {}", titles.len());

    let mut resolved = 0usize;
    for q in &quests {
        if nie_data::quest::resolve_title(q, &titles).is_some() {
            resolved += 1;
        }
    }
    eprintln!("titres résolus = {resolved}/{}", quests.len());
    assert!(resolved > 0, "au moins un titre de quête résolu");

    for q in quests.iter().take(6) {
        let t = nie_data::quest::resolve_title(q, &titles);
        eprintln!(
            "  quest {} title {} → {:?}",
            q.quest_id.to_hex(),
            q.title_hash.to_hex(),
            t
        );
    }
    eprintln!(
        "✓ END-TO-END OK : {resolved} titres de quête résolus via le résolveur de texte universel"
    );
}
