//! Validation END-TO-END de `nie_data::event_subtitle` sur le VRAI jeu : prend un event voicé
//! (ex. `ev09_05000`), parse son `Subtitle_<id>` (ja), sa table washa `<id>_map`, et son dialogue
//! `text/fr/event/<id>`, puis joint les trois par le hash texte (`var[0]`).
//!
//! Usage : `cargo run -p nie-game --example extract_event_subtitle [event_id]`
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

fn load(vfs: &Vfs, suffix: &str, must_contain: &str) -> Option<serde_json::Value> {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.contains(must_contain) && p.rsplit('/').next().is_some_and(|b| b == suffix))
        .min()?;
    eprintln!("  {path}");
    let bytes = vfs.read(&path).ok()?;
    let file = cfgbin::parse_t2b(&bytes).ok()?;
    Some(json!({ "entries": to_iecode(&file.entries) }))
}

fn main() {
    let event = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ev09_05000".into());
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    eprintln!("event = {event}");
    let sub_root = load(&vfs, &format!("Subtitle_{event}.cfg.bin"), "/subtitle/ja/")
        .expect("Subtitle ja introuvable");
    let subs = nie_data::event_subtitle::parse_subtitle_file(&sub_root);
    eprintln!("[subtitle] lignes = {}", subs.len());
    assert!(!subs.is_empty(), "au moins une ligne de sous-titre");

    let washa = load(&vfs, &format!("{event}_map.cfg.bin"), "/text/event/")
        .map(|r| nie_data::event_subtitle::parse_washa_map(&r))
        .unwrap_or_default();
    eprintln!("[washa] entrées = {}", washa.len());

    let text_fr = load(&vfs, &format!("{event}.cfg.bin"), "/text/fr/event/")
        .map(|r| nie_data::event_subtitle::parse_event_text(&r))
        .unwrap_or_default();
    eprintln!("[dialogue fr] lignes = {}", text_fr.len());

    // Jointure par hash sur la 1ʳᵉ ligne de sous-titre.
    let first = &subs[0];
    eprintln!(
        "ligne[0] hash={} start={} end={}",
        first.text_hash.to_hex(),
        first.show_start,
        first.show_end
    );
    let label = washa
        .iter()
        .find(|w| w.text_hash == first.text_hash)
        .and_then(|w| w.label.clone());
    let txt = nie_data::event_subtitle::find_event_text(&text_fr, first.text_hash);
    eprintln!("  washa.label={label:?}");
    eprintln!(
        "  dialogue fr (brut) = {:?}",
        txt.map(|t| t.chars().take(60).collect::<String>())
    );

    // Au moins une jointure dialogue doit aboutir si le fichier fr existe.
    if !text_fr.is_empty() {
        let joined = subs
            .iter()
            .filter(|s| find_has(&text_fr, s.text_hash))
            .count();
        eprintln!(
            "sous-titres joints à un dialogue fr = {joined}/{}",
            subs.len()
        );
        assert!(joined > 0, "au moins une jointure hash subtitle↔dialogue");
    }
    eprintln!("✓ END-TO-END OK : event_subtitle parse+joint le vrai event {event}");
}

fn find_has(lines: &[nie_data::event_subtitle::EventTextLine], h: nie_data::hash::HashId) -> bool {
    lines.iter().any(|l| l.text_hash == h)
}
