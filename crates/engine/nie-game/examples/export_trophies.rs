//! Export industrialisé de la **base de succès** (pilier B′3) : lit le jeu réel (`trophy_config` +
//! `trophy_text` fr) via le VFS, résout nom + description de chaque trophée et **décode sa condition
//! de déblocage** (`open_cond` → story/event-flag), et écrit un JSON stable (azalee/wiki).
//!
//! Usage : `cargo run -p nie-game --example export_trophies -- [out.json]`
//! (défaut : `var/trophies-resolved.json`).
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

fn unlock_kind(k: nie_data::unlock_condition::UnlockType) -> &'static str {
    use nie_data::unlock_condition::UnlockType as U;
    match k {
        U::Always => "always",
        U::Story => "story",
        U::EventFlag => "eventFlag",
        U::Composite => "composite",
    }
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "var/trophies-resolved.json".into());
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let config = nie_data::trophy::parse_trophy_config(&load(
        &vfs,
        |p| {
            p.contains("/gamedata/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("trophy_config") && b.ends_with(".cfg.bin"))
        },
        "trophy_config",
    ));
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

    use nie_data::trophy::{decode_unlock, resolve_description, resolve_name};
    let mut list: Vec<serde_json::Value> = Vec::new();
    for t in &config.infos {
        let Some(name) = resolve_name(t, &text) else {
            continue;
        };
        let cond = decode_unlock(t);
        list.push(json!({
            "trophyId": t.trophy_id.to_hex(),
            "code": t.code,
            "category": t.category,
            "name": name,
            "description": resolve_description(t, &text),
            "unlock": {
                "kind": unlock_kind(cond.kind),
                "storyEpisode": cond.story_episode,
                "eventFlags": cond.required_events.iter().map(|e| e.crc_hex()).collect::<Vec<_>>(),
            },
        }));
    }

    let doc = json!({
        "schema": "niers/trophies-resolved/v1",
        "locale": "fr",
        "count": list.len(),
        "trophies": list,
    });
    let txt = serde_json::to_string_pretty(&doc).expect("serialize");
    std::fs::create_dir_all(Path::new(&out).parent().unwrap_or(Path::new("."))).ok();
    std::fs::write(&out, &txt).unwrap_or_else(|e| panic!("écriture {out} : {e}"));
    eprintln!(
        "✓ export-trophies: {} succès résolus → {out} ({} octets)",
        doc["count"],
        txt.len()
    );
}
