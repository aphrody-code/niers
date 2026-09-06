//! Validation END-TO-END de `nie_data::ai::resolve_strategy_name`/`resolve_strategy_description` :
//! charge le vrai `strategy_ai_config` (RDBN) et `ai_text` (T2B, fr) du VFS et résout le nom +
//! description de chaque stratégie IA. Comme `ai.rs` n'a pas de parseur inagle de référence, cette
//! vérification confirme (ou infirme) le modèle de données `name_id`/`desc_id` → `ai_text`.
//!
//! Usage : `cargo run -p nie-game --example extract_ai_strategy_names`
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

fn load_t2b(vfs: &Vfs, prefix: &str) -> serde_json::Value {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.contains("/fr/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with(prefix) && b.ends_with(".cfg.bin"))
        })
        .min()
        .unwrap_or_else(|| panic!("{prefix} fr introuvable"));
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

    let cfg = load_rdbn(&vfs, "strategy_ai_config");
    let config = nie_data::ai::parse_strategy_ai_config(&cfg);
    eprintln!("[strategy_ai] stratégies = {}", config.strategy_infos.len());
    assert!(!config.strategy_infos.is_empty());

    let text = nie_data::text::parse_text_file(&load_t2b(&vfs, "ai_text"));
    eprintln!("[ai_text fr] entrées = {}", text.len());

    use nie_data::ai::{resolve_strategy_description, resolve_strategy_name};
    let named = config
        .strategy_infos
        .iter()
        .filter(|s| resolve_strategy_name(s, &text).is_some())
        .count();
    eprintln!(
        "noms de stratégie résolus = {named}/{}",
        config.strategy_infos.len()
    );

    for s in config.strategy_infos.iter().take(8) {
        let n = resolve_strategy_name(s, &text);
        let d =
            resolve_strategy_description(s, &text).map(|x| x.chars().take(40).collect::<String>());
        eprintln!(
            "  strategy {} → {:?}  desc={:?}",
            s.strategy_id.to_hex(),
            n,
            d
        );
    }
    if named == 0 {
        eprintln!("⚠ VERDICT : name_id → ai_text NE résout PAS (modèle à revoir, ou autre table)");
    } else {
        eprintln!("✓ END-TO-END OK : {named} noms de stratégie IA résolus via ai_text");
    }
}
