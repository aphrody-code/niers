//! Extracteur de vérité terrain `ctrl_chara` : lit `party/ctrl_chara_config_*.cfg.bin` du VFS,
//! parse en T2B, et affiche les noeuds `CTRL_CHR_DATA` (charaParamId hex, controlType, flags) +
//! le nombre total et la distribution de controlType. Ancre le golden de `nie_data::ctrl_chara`.
//!
//! Usage : `cargo run -p nie-game --example extract_ctrl_chara`
use nie_formats::cfgbin::{self, CfgEntry, Value};
use nie_formats::vfs::Vfs;
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
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

fn visit(e: &CfgEntry, out: &mut Vec<(i32, i32, i32)>) {
    if e.name == "CTRL_CHR_DATA" {
        let ints: Vec<i32> = e
            .variables
            .iter()
            .map(|v| match v {
                Value::Int(i) => *i,
                Value::Float(f) => *f as i32,
                Value::String(s) => s.trim().parse().unwrap_or(0),
            })
            .collect();
        if ints.len() >= 3 {
            out.push((ints[0], ints[1], ints[2]));
        }
    }
    for c in &e.children {
        visit(c, out);
    }
}

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.rsplit('/')
                .next()
                .is_some_and(|b| b.starts_with("ctrl_chara_config") && b.ends_with(".cfg.bin"))
        })
        .min()
        .expect("ctrl_chara_config introuvable");
    eprintln!("ctrl_chara = {path}");
    let bytes = vfs.read(&path).expect("read");
    let file = cfgbin::parse_t2b(&bytes).expect("parse_t2b");

    let mut nodes = Vec::new();
    for e in &file.entries {
        visit(e, &mut nodes);
    }
    eprintln!("CTRL_CHR_DATA (>=3 ints) = {}", nodes.len());

    let mut ctype: BTreeMap<i32, usize> = BTreeMap::new();
    let mut flags: BTreeMap<i32, usize> = BTreeMap::new();
    for (_, ct, fl) in &nodes {
        *ctype.entry(*ct).or_default() += 1;
        *flags.entry(*fl).or_default() += 1;
    }
    eprintln!("distribution controlType = {ctype:?}");
    eprintln!("distribution flags = {flags:?}");
    println!("// 5 premiers CTRL_CHR_DATA réels (charaParamId hex, controlType, flags) :");
    for (id, ct, fl) in nodes.iter().take(5) {
        println!("//   {:#010X}  ctrl={ct} flags={fl}", *id as u32);
    }

    // END-TO-END : le vrai module sur tout le dump.
    let root = json!({ "entries": to_iecode(&file.entries) });
    let parsed = nie_data::ctrl_chara::parse_all_ctrl_chara(&root);
    eprintln!("[module] parse_all_ctrl_chara = {} entrées", parsed.len());
    assert_eq!(
        parsed.len(),
        nodes.len(),
        "module vs brut : nombre d'entrées"
    );
    for (p, (id, ct, fl)) in parsed.iter().zip(nodes.iter()) {
        assert_eq!(p.chara_param_id, nie_data::hash::HashId(*id as u32));
        assert_eq!(p.control_type, i64::from(*ct));
        assert_eq!(p.flags, i64::from(*fl));
    }
    eprintln!(
        "✓ END-TO-END OK : nie_data::ctrl_chara == extraction brute sur {} noeuds",
        nodes.len()
    );
}
