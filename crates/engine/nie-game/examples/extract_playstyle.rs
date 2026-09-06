//! Extracteur de vérité terrain `playstyle` : lit le vrai `chara_param_1.*.cfg.bin` du VFS,
//! parse en T2B, et pour chaque noeud `CHARA_PARAM_INFO_*` affiche `values[0]` (charaParamId, hex
//! u32) et `values[5]` (playStyle) — réplique exacte de `parsePlaystyleNode` d'inagle (collecte
//! des seules variables `Int`, index 5). Sert à ancrer le golden de `nie_data::playstyle`.
//!
//! Usage : `cargo run -p nie-game --example extract_playstyle`
use nie_formats::cfgbin::{self, CfgEntry, Value};
use nie_formats::vfs::Vfs;
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Convertit des frères T2B en forme iecode `{name, variables:[{type,value}], children}` avec
/// suffixe `_<idx>` par nom (réplique `t2b_siblings_to_iecode` de nie-model-serve) — pour pouvoir
/// faire tourner le VRAI parseur `nie_data::playstyle` sur tout le dump.
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

fn int_values(e: &CfgEntry) -> Vec<i32> {
    e.variables
        .iter()
        .filter_map(|v| {
            if let Value::Int(i) = v {
                Some(*i)
            } else {
                None
            }
        })
        .collect()
}

fn visit(e: &CfgEntry, out: &mut Vec<(String, i32, i32, usize)>) {
    // Nom brut T2B = `CHARA_PARAM_INFO` (le suffixe `_<i>` de la forme iecode est ajouté à la
    // conversion ; inagle voit la forme suffixée et exclut LIST/BEG — équivalent au match exact ici).
    if e.name == "CHARA_PARAM_INFO" {
        let vals = int_values(e);
        if vals.len() >= 6 {
            out.push((e.name.clone(), vals[0], vals[5], vals.len()));
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
                .is_some_and(|b| b.starts_with("chara_param_1") && b.ends_with(".cfg.bin"))
        })
        .min()
        .expect("chara_param introuvable dans le VFS");
    eprintln!("chara_param = {path}");

    let bytes = vfs.read(&path).expect("lire chara_param");
    let file = cfgbin::parse_t2b(&bytes).expect("parse_t2b");

    // DEBUG : distribution des noms (préfixe avant le dernier '_<digits>') sur tout l'arbre.
    let mut name_prefixes: BTreeMap<String, usize> = BTreeMap::new();
    fn collect_names(e: &CfgEntry, m: &mut BTreeMap<String, usize>) {
        *m.entry(e.name.clone()).or_default() += 1;
        for c in &e.children {
            collect_names(c, m);
        }
    }
    for e in &file.entries {
        collect_names(e, &mut name_prefixes);
    }
    eprintln!("noms distincts (total {}) — top 25 :", name_prefixes.len());
    for (n, c) in name_prefixes.iter().take(25) {
        eprintln!("   {n} x{c}");
    }

    let mut nodes = Vec::new();
    for e in &file.entries {
        visit(e, &mut nodes);
    }
    eprintln!("CHARA_PARAM_INFO valides (>=6 ints) = {}", nodes.len());

    // Distribution des playStyle (values[5]).
    let mut dist: BTreeMap<i32, usize> = BTreeMap::new();
    for (_, _, ps, _) in &nodes {
        *dist.entry(*ps).or_default() += 1;
    }
    eprintln!("distribution playStyle = {dist:?}");

    // Un échantillon réel pour CHAQUE playStyle 0..=5 (le 1ᵉʳ rencontré) → fixtures golden.
    let mut sample: BTreeMap<i32, (String, i32, usize)> = BTreeMap::new();
    for (name, id, ps, n) in &nodes {
        sample.entry(*ps).or_insert((name.clone(), *id, *n));
    }
    println!("// échantillons réels (playStyle -> nom, charaParamId hex, nb ints) :");
    for (ps, (name, id, n)) in &sample {
        println!(
            "//   playStyle={ps} : {name} charaParamId={:#010X} (ints={n})",
            *id as u32
        );
    }
    // Les 3 premiers noeuds en entier (values complets) pour fixture inline.
    for (name, id, ps, n) in nodes.iter().take(3) {
        println!(
            "//   {name}: values[0]={:#010X} values[5]={ps} ints={n}",
            *id as u32
        );
    }

    // ── Validation END-TO-END : faire tourner le VRAI module `nie_data::playstyle` sur tout le
    //    dump (converti en forme iecode) et confronter au comptage brut ci-dessus. ──
    let root = json!({ "entries": to_iecode(&file.entries) });
    let parsed = nie_data::playstyle::parse_all_playstyles(&root);
    let mut mdist: BTreeMap<i64, usize> = BTreeMap::new();
    let mut no_label = 0usize;
    for e in &parsed {
        *mdist.entry(e.play_style).or_default() += 1;
        if e.label_en().is_none() {
            no_label += 1;
        }
    }
    let raw_dist: BTreeMap<i64, usize> = dist.iter().map(|(k, v)| (i64::from(*k), *v)).collect();
    eprintln!(
        "[module] nie_data::playstyle::parse_all_playstyles = {} entrées",
        parsed.len()
    );
    eprintln!("[module] distribution = {mdist:?}  sans_libellé = {no_label}");
    assert_eq!(
        parsed.len(),
        nodes.len(),
        "module vs brut : nombre d'entrées"
    );
    assert_eq!(
        mdist, raw_dist,
        "module vs brut : distribution playStyle identique"
    );
    assert_eq!(
        no_label, 0,
        "tous les playStyle réels sont dans 0..=5 (libellé non nul)"
    );
    eprintln!(
        "✓ END-TO-END OK : nie_data::playstyle == extraction brute sur {} noeuds",
        nodes.len()
    );
}
