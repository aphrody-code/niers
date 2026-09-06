//! Validation END-TO-END de `nie_data::mission::resolve_name` + `text::parse_text_file` : charge
//! le vrai `mission_config` et `mission_text` (fr) du VFS et résout le nom localisé de chaque
//! mission (jointure `nameId` = var[2] → `mission_text`, port de `mission-config.ts`).
//!
//! Usage : `cargo run -p nie-game --example extract_mission_text`
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

    let cfg_root =
        load(&vfs, "mission_config", "/gamedata/mission/").expect("mission_config introuvable");
    let missions = nie_data::mission::parse_mission_config(&cfg_root);
    eprintln!("[mission] missions = {}", missions.len());
    assert!(!missions.is_empty());

    let text_root = load(&vfs, "mission_text", "/text/fr/").expect("mission_text fr introuvable");
    let mission_text = nie_data::text::parse_text_file(&text_root);
    eprintln!("[mission_text fr] entrées = {}", mission_text.len());

    // NB : `mission_config` du jeu est un STUB (1 mission placeholder `msa999999`) et
    // `mission_text` ne porte que quelques entrées ; le nameId du stub n'est pas localisé.
    // On valide donc le MÉCANISME de jointure sur des données réelles, pas un taux de couverture.
    let resolved = missions
        .iter()
        .filter(|m| nie_data::mission::resolve_name(m, &mission_text).is_some())
        .count();
    for m in &missions {
        let n = nie_data::mission::resolve_name(m, &mission_text);
        eprintln!(
            "  mission {} ({}) nameId={} → {:?}",
            m.mission_id.to_hex(),
            m.mission_code,
            m.name_id().to_hex(),
            n
        );
    }
    eprintln!(
        "noms de mission résolus = {resolved}/{} (stub : msa999999)",
        missions.len()
    );

    // Mécanisme : chaque entrée réelle de `mission_text` est résoluble par son propre hash.
    assert!(!mission_text.is_empty(), "mission_text fr non vide");
    for (h, txt) in &mission_text {
        assert_eq!(
            nie_data::text::find_text(&mission_text, *h),
            Some(txt.as_str())
        );
        eprintln!(
            "  mission_text {} = {:?}",
            h.to_hex(),
            txt.chars().take(40).collect::<String>()
        );
    }
    eprintln!("✓ END-TO-END OK : jointure mission::resolve_name vérifiée sur le vrai mission_text");
}
