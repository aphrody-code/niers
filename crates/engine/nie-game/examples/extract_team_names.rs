//! Validation END-TO-END de `nie_data::belong_team::resolve_team_name` + `text::parse_text_file` :
//! charge `belong_team_config` + `team_text` (fr) du VFS et résout le nom de chaque équipe. Démontre
//! aussi la chaîne `chara_base.belong_team_id` → `belong_team` → nom d'équipe (Endou `c01000010`).
//!
//! Usage : `cargo run -p nie-game --example extract_team_names`
use nie_formats::cfgbin::{self, CfgEntry, RdbnValue, Value};
use nie_formats::vfs::Vfs;
use serde_json::{Map, json};
use std::collections::HashMap;
use std::path::Path;

fn t2b_to_iecode(siblings: &[CfgEntry]) -> Vec<serde_json::Value> {
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
            json!({ "name": name, "variables": variables, "children": t2b_to_iecode(&e.children) })
        })
        .collect()
}

fn rdbn_value_to_json(v: &RdbnValue) -> serde_json::Value {
    use RdbnValue as R;
    match v {
        R::Bool(b) => json!(b),
        R::Byte(n) => json!(n),
        R::Short(n) | R::ActType(n) => json!(n),
        R::Int(n) | R::Flag(n) => json!(n),
        R::Float(f) => json!(f),
        R::Hash(h) => json!(format!("0x{h:08X}")),
        R::Rates(a) | R::Position(a) => json!(a),
        R::Condition(s) => json!(s),
        R::ShortTuple(t) => json!(t),
        _ => serde_json::Value::Null,
    }
}

fn load_rdbn(vfs: &Vfs, prefix: &str) -> serde_json::Value {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.contains("/gamedata/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with(prefix) && b.ends_with(".cfg.bin"))
        })
        .min()
        .unwrap_or_else(|| panic!("{prefix} introuvable"));
    eprintln!("  {path}");
    let bytes = vfs.read(&path).expect("read");
    let rdbn = cfgbin::parse(&bytes).expect("parse rdbn");
    let lists: Vec<serde_json::Value> = cfgbin::read_values(&rdbn, &bytes)
        .iter()
        .map(|l| {
            let values: Vec<serde_json::Value> = l
                .rows
                .iter()
                .map(|row| {
                    let mut m = Map::new();
                    for (n, v) in &row.fields {
                        m.insert(n.clone(), rdbn_value_to_json(v));
                    }
                    serde_json::Value::Object(m)
                })
                .collect();
            json!({ "name": l.name, "values": values })
        })
        .collect();
    json!({ "lists": lists })
}

fn load_t2b(vfs: &Vfs, exact: &str, must: &str) -> serde_json::Value {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.contains(must) && p.rsplit('/').next() == Some(exact))
        .min()
        .unwrap_or_else(|| panic!("{exact} introuvable"));
    eprintln!("  {path}");
    let file = cfgbin::parse_t2b(&vfs.read(&path).expect("read")).expect("parse");
    json!({ "entries": t2b_to_iecode(&file.entries) })
}

fn load_t2b_prefix(vfs: &Vfs, prefix: &str, must: &str) -> serde_json::Value {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.contains(must)
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with(prefix) && b.ends_with(".cfg.bin"))
        })
        .min()
        .unwrap_or_else(|| panic!("{prefix} introuvable"));
    eprintln!("  {path}");
    let file = cfgbin::parse_t2b(&vfs.read(&path).expect("read")).expect("parse");
    json!({ "entries": t2b_to_iecode(&file.entries) })
}

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let teams =
        nie_data::belong_team::parse_belong_team_config(&load_rdbn(&vfs, "belong_team_config"));
    eprintln!("[belong_team] équipes = {}", teams.len());
    assert!(!teams.is_empty());

    let team_text = nie_data::text::parse_text_file(&load_t2b(&vfs, "team_text.cfg.bin", "/fr/"));
    eprintln!("[team_text fr] entrées = {}", team_text.len());

    use nie_data::belong_team::resolve_team_name;
    let named = teams
        .iter()
        .filter(|t| resolve_team_name(t, &team_text).is_some())
        .count();
    eprintln!("noms d'équipe résolus = {named}/{}", teams.len());
    assert!(named > 0, "au moins un nom d'équipe résolu");
    for t in teams
        .iter()
        .filter(|t| resolve_team_name(t, &team_text).is_some())
        .take(8)
    {
        eprintln!(
            "  team {} → {:?}",
            t.belong_team_id.to_hex(),
            resolve_team_name(t, &team_text)
        );
    }

    // Chaîne chara_base → belong_team : l'équipe d'Endou (c01000010, belong_team_id 0xF01BB293).
    let bases = nie_data::chara_base::parse_all_chara_base(&load_t2b_prefix(
        &vfs,
        "chara_base_1",
        "/character/",
    ));
    use nie_data::hash::HashId;
    if let Some(endou) = nie_data::chara_base::find_by_chara_id(&bases, HashId(0x99A1_C150)) {
        let team = endou
            .belong_team_id
            .and_then(|id| nie_data::belong_team::find_by_id(&teams, id))
            .and_then(|t| resolve_team_name(t, &team_text));
        eprintln!("Endou (c01000010) → équipe = {team:?}");
    }
    eprintln!("✓ END-TO-END OK : {named} noms d'équipe résolus + chaîne perso→équipe");
}
