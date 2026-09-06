//! Export industrialisé de la **base d'équipes** (pilier B′3) : lit le jeu réel (`belong_team_config`
//! et `team_text` fr) via le VFS, résout le nom de chaque équipe, et écrit un JSON stable (azalee/wiki).
//! Complète la base personnages (qui référence `teamId`).
//!
//! Usage : `cargo run -p nie-game --example export_teams -- [out.json]`
//! (défaut : `var/teams-resolved.json`).
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

fn load_t2b(vfs: &Vfs, prefix: &str, must: &str) -> serde_json::Value {
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
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "var/teams-resolved.json".into());
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let teams =
        nie_data::belong_team::parse_belong_team_config(&load_rdbn(&vfs, "belong_team_config"));
    let team_text = nie_data::text::parse_text_file(&load_t2b(&vfs, "team_text", "/fr/"));

    use nie_data::belong_team::resolve_team_name;
    let mut list: Vec<serde_json::Value> = Vec::new();
    for t in &teams {
        let Some(name) = resolve_team_name(t, &team_text) else {
            continue;
        };
        list.push(json!({
            "teamId": t.belong_team_id.to_hex(),
            "name": name,
            "binderOrder": t.binder_team_order_type,
        }));
    }

    let doc = json!({
        "schema": "niers/teams-resolved/v1",
        "locale": "fr",
        "count": list.len(),
        "teams": list,
    });
    let txt = serde_json::to_string_pretty(&doc).expect("serialize");
    std::fs::create_dir_all(Path::new(&out).parent().unwrap_or(Path::new("."))).ok();
    std::fs::write(&out, &txt).unwrap_or_else(|e| panic!("écriture {out} : {e}"));
    eprintln!(
        "✓ export-teams: {} équipes résolues → {out} ({} octets)",
        doc["count"],
        txt.len()
    );
}
