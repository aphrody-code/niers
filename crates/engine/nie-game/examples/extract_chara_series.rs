//! Validation END-TO-END de `nie_data::chara_series` + `chara_base::resolve_series_name_fr` :
//! charge `chara_series_config` (RDBN) du VFS, parse les 9 séries, et résout la série de chaque
//! personnage. Démontre la chaîne `chara_base.series_id` → série (Endou `c01000010`).
//!
//! Usage : `cargo run -p nie-game --example extract_chara_series`
use nie_formats::cfgbin::{self, CfgEntry, RdbnValue, Value};
use nie_formats::vfs::Vfs;
use serde_json::{Map, json};
use std::collections::{BTreeMap, HashMap};
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

fn load_rdbn(vfs: &Vfs, prefix: &str, must: &str) -> serde_json::Value {
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
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let series = nie_data::chara_series::parse_chara_series_config(&load_rdbn(
        &vfs,
        "chara_series_config",
        "/character/",
    ));
    eprintln!("[chara_series] séries = {}", series.len());
    assert_eq!(series.len(), 9, "9 séries (probe live)");
    for s in &series {
        eprintln!(
            "  série {} type={} → {:?}",
            s.series_id.to_hex(),
            s.series_type,
            nie_data::chara_series::series_name_fr(s.series_type)
        );
    }

    let bases =
        nie_data::chara_base::parse_all_chara_base(&load_t2b(&vfs, "chara_base_1", "/character/"));
    use nie_data::hash::HashId;
    let endou = nie_data::chara_base::find_by_chara_id(&bases, HashId(0x99A1_C150)).expect("Endou");
    let endou_series = nie_data::chara_base::resolve_series_name_fr(endou, &series);
    eprintln!(
        "Endou (c01000010, series_id={:?}) → série = {endou_series:?}",
        endou.series_id.map(|h| h.to_hex())
    );
    assert_eq!(
        endou_series,
        Some("Inazuma Eleven"),
        "Endou = série 1 (IE original)"
    );

    // Distribution des séries sur le roster.
    let mut dist: BTreeMap<&str, usize> = BTreeMap::new();
    for b in &bases {
        if let Some(name) = nie_data::chara_base::resolve_series_name_fr(b, &series) {
            *dist.entry(name).or_default() += 1;
        }
    }
    eprintln!("distribution séries = {dist:?}");
    eprintln!(
        "✓ END-TO-END OK : 9 séries + résolution chara_base.series_id → nom (Endou = Inazuma Eleven)"
    );
}
