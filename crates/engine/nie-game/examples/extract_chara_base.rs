//! Vérité terrain + validation END-TO-END de `nie_data::chara_base` : lit le vrai
//! `character/chara_base_1.*.cfg.bin` du VFS (T2B), le convertit en forme iecode et fait tourner
//! `parse_all_chara_base`, en confrontant le comptage à l'extraction brute et en affichant des
//! échantillons réels (charaId, internalCode, gender, series, team).
//!
//! Usage : `cargo run -p nie-game --example extract_chara_base`
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

/// Compte brut des noeuds CHARA_BASE_INFO/BATTLE (nom T2B brut = sans suffixe).
fn count_raw(e: &CfgEntry, n: &mut usize) {
    if (e.name == "CHARA_BASE_INFO" || e.name == "CHARA_BASE_BATTLE") && e.variables.len() >= 4 {
        // mêmes garde-fous de type que le module : [0]=Int, [1]=String, [3]=Int.
        let t0 = matches!(e.variables.first(), Some(Value::Int(_)));
        let t1 = matches!(e.variables.get(1), Some(Value::String(_)));
        let t3 = matches!(e.variables.get(3), Some(Value::Int(_)));
        if t0 && t1 && t3 {
            *n += 1;
        }
    }
    for c in &e.children {
        count_raw(c, n);
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
                .is_some_and(|b| b.starts_with("chara_base_1") && b.ends_with(".cfg.bin"))
        })
        .min()
        .expect("chara_base_1 introuvable");
    eprintln!("chara_base = {path}");
    let bytes = vfs.read(&path).expect("read");
    let file = cfgbin::parse_t2b(&bytes).expect("parse_t2b");

    let mut raw = 0usize;
    for e in &file.entries {
        count_raw(e, &mut raw);
    }

    let root = json!({ "entries": to_iecode(&file.entries) });
    let bases = nie_data::chara_base::parse_all_chara_base(&root);
    eprintln!("[brut] noeuds valides = {raw} ; [module] = {}", bases.len());
    assert_eq!(bases.len(), raw, "module vs brut : nombre d'entrées");
    assert!(!bases.is_empty());

    let mut genders: BTreeMap<i64, usize> = BTreeMap::new();
    let (mut with_team, mut with_series, mut with_desc) = (0, 0, 0);
    for b in &bases {
        *genders.entry(b.gender).or_default() += 1;
        with_team += usize::from(b.belong_team_id.is_some());
        with_series += usize::from(b.series_id.is_some());
        with_desc += usize::from(b.description_hash.is_some());
    }
    eprintln!(
        "genders = {genders:?} ; with_team={with_team} with_series={with_series} with_desc={with_desc}"
    );
    eprintln!("// 3 premiers entrées RICHES (team+series+description+lastname) :");
    for b in bases
        .iter()
        .filter(|b| {
            b.belong_team_id.is_some()
                && b.series_id.is_some()
                && b.description_hash.is_some()
                && b.last_name_hash.is_some()
        })
        .take(3)
    {
        eprintln!(
            "//   charaId={} code={:?} name={} last={:?} gender={} team={:?} series={:?} desc={:?}",
            b.chara_id.to_hex(),
            b.internal_code,
            b.name_hash.to_hex(),
            b.last_name_hash.map(|h| h.to_hex()),
            b.gender,
            b.belong_team_id.map(|h| h.to_hex()),
            b.series_id.map(|h| h.to_hex()),
            b.description_hash.map(|h| h.to_hex()),
        );
    }
    eprintln!(
        "✓ END-TO-END OK : nie_data::chara_base == extraction brute sur {} noeuds",
        bases.len()
    );
}
