//! Jetable : dump du noeud reel `win_treasure_lot_table_config` du VFS pour fixture golden.
//! `cargo run -p nie-game --example extract_win_treasure`
use nie_formats::cfgbin::{self, CfgEntry, Value};
use nie_formats::vfs::Vfs;

fn vstr(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
    }
}

fn to_hex(signed: i64) -> String {
    format!("0x{:08X}", (signed as i32) as u32)
}

/// Renomme `<base>_<i>` par rang d'occurrence parmi les freres (comme t2b_siblings_to_iecode).
fn rename(siblings: &[CfgEntry]) -> Vec<(String, &CfgEntry)> {
    use std::collections::HashMap;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    siblings
        .iter()
        .map(|e| {
            let idx = counts.entry(e.name.as_str()).or_insert(0);
            let name = format!("{}_{}", e.name, *idx);
            *idx += 1;
            (name, e)
        })
        .collect()
}

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir();
    let mut vfs = Vfs::new();
    vfs.init(dir.join("data").as_path()).expect("vfs init");
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.contains("win_treasure_lot_table_config") && p.ends_with(".cfg.bin"))
        .min()
        .expect("win_treasure introuvable");
    eprintln!("path = {path}");
    let bytes = vfs.read(&path).unwrap();
    eprintln!("is_rdbn = {}", cfgbin::is_rdbn(&bytes));
    let cfg = cfgbin::cfgbin_parse(&bytes).expect("parse t2b");

    // Top-level roots renommes.
    let roots = rename(&cfg.entries);
    eprintln!("--- top-level entries ({}) ---", roots.len());
    for (name, e) in &roots {
        eprintln!(
            "  root {name}  vars={}  children={}",
            e.variables.len(),
            e.children.len()
        );
    }

    // Flat ITBL_ITEMS list.
    let mut flat: Vec<(String, i64)> = Vec::new(); // (itemId hex, weight)
    let mut flat_raw: Vec<i64> = Vec::new(); // var0 brut signe
    let mut flat_w: Vec<i64> = Vec::new();
    for (name, root) in &roots {
        if name.starts_with("ITBL_ITEMS_LIST_BEG") {
            for (cn, leaf) in rename(&root.children) {
                if cn.starts_with("ITBL_ITEMS_") && !cn.contains("LIST") {
                    let v0: i64 = vstr(&leaf.variables[0]).parse().unwrap();
                    let v2: i64 = vstr(&leaf.variables[2]).parse().unwrap();
                    flat.push((to_hex(v0), v2));
                    flat_raw.push(v0);
                    flat_w.push(v2);
                    if flat.len() <= 6 {
                        let vs: Vec<String> = leaf.variables.iter().map(vstr).collect();
                        eprintln!("    leaf {cn} = [{}]", vs.join(", "));
                    }
                }
            }
        }
    }
    eprintln!("--- flat ITBL_ITEMS count = {} ---", flat.len());

    // ITBL_BASE pairs.
    let mut base_children: Vec<(String, &CfgEntry)> = Vec::new();
    for (name, root) in &roots {
        if name.starts_with("ITBL_BASE_LIST_BEG") {
            base_children.extend(rename(&root.children));
        }
    }
    eprintln!("--- ITBL_BASE_LIST children = {} ---", base_children.len());
    let mut pairs: Vec<(i64, i64, i64)> = Vec::new(); // (coffre signe, offset, count)
    let mut i = 0;
    while i < base_children.len() {
        let (name, node) = &base_children[i];
        if name.starts_with("ITBL_BASE_")
            && !name.contains("REF")
            && !name.contains("LIST")
            && let Some((rn, refn)) = base_children.get(i + 1)
            && rn.starts_with("ITBL_BASE_REF_ITEMS_")
        {
            let coffre: i64 = vstr(&node.variables[0]).parse().unwrap();
            let off: i64 = vstr(&refn.variables[0]).parse().unwrap();
            let cnt: i64 = vstr(&refn.variables[1]).parse().unwrap();
            pairs.push((coffre, off, cnt));
        }
        i += 1;
    }
    eprintln!("--- base pairs = {} ---", pairs.len());
    for (c, o, n) in &pairs {
        eprintln!("  coffre {} off={} count={}", to_hex(*c), o, n);
    }

    // Dump exploitable pour la fixture (Rust array literals).
    println!(
        "// === FLAT ITBL_ITEMS (itemId signe brut, poids) — count {} ===",
        flat.len()
    );
    println!("const ITEMS_RAW: &[(i64, i64)] = &[");
    for (idx, ((_, _), (rawid, w))) in flat
        .iter()
        .zip(flat_raw.iter().zip(flat_w.iter()))
        .enumerate()
    {
        println!("    ({rawid}, {w}), // [{idx}] -> {}", to_hex(*rawid));
        let _ = w;
    }
    println!("];");
    println!(
        "// === BASE pairs (coffre signe, offset, count) — count {} ===",
        pairs.len()
    );
    println!("const BASE_RAW: &[(i64, i64, i64)] = &[");
    for (c, o, n) in &pairs {
        println!("    ({c}, {o}, {n}), // coffre {}", to_hex(*c));
    }
    println!("];");

    // Expected rows (sourceId coffre hex, itemId hex, weight) — golden de reference.
    println!("// === EXPECTED ROWS (coffre_hex, item_hex, weight) ===");
    let mut total = 0usize;
    for (c, o, n) in &pairs {
        let chex = to_hex(*c);
        for j in *o..(*o + *n) {
            if (j as usize) < flat.len() {
                let (ihex, w) = &flat[j as usize];
                if total < 12 {
                    println!("    (\"{chex}\", \"{ihex}\", {w}),");
                }
                total += 1;
            }
        }
    }
    println!("// total rows = {total}");
}
