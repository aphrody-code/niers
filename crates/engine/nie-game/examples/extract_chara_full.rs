//! Validation END-TO-END de la **chaîne personnage complète** : `chara_base` →
//! `chara_text` (prénom/nom) → `chara_description` (bio), résolue par les jointures de hash
//! (`nameHash`/`lastNameHash`/`descriptionHash`). Démontre le résolveur sur Endou (`c01000010`).
//!
//! Usage : `cargo run -p nie-game --example extract_chara_full`
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

fn load(vfs: &Vfs, exact: &str, must_contain: &str) -> serde_json::Value {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.contains(must_contain) && p.rsplit('/').next().is_some_and(|b| b == exact))
        .min()
        .unwrap_or_else(|| panic!("{exact} ({must_contain}) introuvable"));
    eprintln!("  {path}");
    let file = cfgbin::parse_t2b(&vfs.read(&path).expect("read")).expect("parse");
    json!({ "entries": to_iecode(&file.entries) })
}

fn load_prefix(vfs: &Vfs, prefix: &str, must_contain: &str) -> serde_json::Value {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.contains(must_contain)
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with(prefix) && b.ends_with(".cfg.bin"))
        })
        .min()
        .unwrap_or_else(|| panic!("{prefix} introuvable"));
    eprintln!("  {path}");
    let file = cfgbin::parse_t2b(&vfs.read(&path).expect("read")).expect("parse");
    json!({ "entries": to_iecode(&file.entries) })
}

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let base_root = load_prefix(&vfs, "chara_base_1", "/character/");
    let bases = nie_data::chara_base::parse_all_chara_base(&base_root);
    let text_root = load(&vfs, "chara_text.cfg.bin", "/fr/");
    let nouns = nie_data::chara_text::parse_all_nouns(&text_root);
    let desc_root = load(&vfs, "chara_description_text.cfg.bin", "/fr/");
    let descs = nie_data::chara_description::parse_chara_descriptions(&desc_root);
    eprintln!(
        "bases={} nouns={} descriptions={}",
        bases.len(),
        nouns.len(),
        descs.len()
    );

    use nie_data::chara_base::{
        find_by_chara_id, resolve_description, resolve_first_name, resolve_last_name,
    };
    use nie_data::hash::HashId;

    // Endou Mamoru = c01000010 = charaId 0x99A1C150.
    let endou = find_by_chara_id(&bases, HashId(0x99A1_C150)).expect("Endou présent");
    let first = resolve_first_name(endou, &nouns);
    let last = resolve_last_name(endou, &nouns);
    let bio = resolve_description(endou, &descs);
    eprintln!(
        "Endou ({}) : prénom={first:?} nom={last:?}",
        endou.internal_code
    );
    eprintln!(
        "  bio={:?}…",
        bio.map(|b| b.chars().take(60).collect::<String>())
    );
    assert_eq!(first, Some("Mark"));
    assert_eq!(last, Some("Evans"));
    assert!(bio.is_some_and(|b| b.starts_with("La passion du football")));

    // Statistique globale : combien de personnages ont un prénom résolu ?
    let named = bases
        .iter()
        .filter(|b| resolve_first_name(b, &nouns).is_some())
        .count();
    let with_bio = bases
        .iter()
        .filter(|b| resolve_description(b, &descs).is_some())
        .count();
    eprintln!(
        "personnages avec prénom résolu = {named}/{} ; avec bio = {with_bio}",
        bases.len()
    );
    eprintln!("✓ END-TO-END OK : chaîne base→text→description résolue (Endou = Mark Evans)");
}
