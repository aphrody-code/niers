//! Validation END-TO-END de `nie_data::chara_text` sur le VRAI jeu : lit
//! `common/text/fr/chara_text.cfg.bin` (T2B), convertit en iecode, exécute `parse_all_nouns`,
//! puis résout les noms d'Endou via les hash de `chara_base` (`c01000010`) :
//! nameHash `0xE551FF12` (prénom) et lastNameHash `0xE5655F9E` (nom de famille).
//!
//! Usage : `cargo run -p nie-game --example extract_chara_text`
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
            p.contains("/fr/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b == "chara_text.cfg.bin")
        })
        .min()
        .expect("chara_text fr introuvable");
    eprintln!("chara_text = {path}");
    let bytes = vfs.read(&path).expect("read");
    let file = cfgbin::parse_t2b(&bytes).expect("parse_t2b");
    let root = json!({ "entries": to_iecode(&file.entries) });

    let nouns = nie_data::chara_text::parse_all_nouns(&root);
    eprintln!("[module] noms NOUN_INFO non-vides = {}", nouns.len());
    assert!(!nouns.is_empty());

    // Aucun nom ne doit contenir de balise résiduelle.
    let with_tag = nouns
        .iter()
        .filter(|n| n.name.contains('<') && n.name.contains('>'))
        .count();
    assert_eq!(
        with_tag, 0,
        "sanitize_text doit retirer les balises des noms"
    );

    use nie_data::chara_text::find_name;
    use nie_data::hash::HashId;
    // Combien d'entrées partagent le hash d'Endou ? (la map inagle = last-wins).
    let dups: Vec<&str> = nouns
        .iter()
        .filter(|n| n.hash_id == HashId(0xE551_FF12))
        .map(|n| n.name.as_str())
        .collect();
    eprintln!("entrées 0xE551FF12 = {dups:?}");
    let name = find_name(&nouns, HashId(0xE551_FF12));
    let last = find_name(&nouns, HashId(0xE565_5F9E));
    eprintln!("Endou c01000010 → nameHash(0xE551FF12)={name:?} lastNameHash(0xE5655F9E)={last:?}");
    // Sémantique last-wins (= buildNameMap) : prénom « Mark », nom de famille « Evans ».
    assert_eq!(
        dups,
        ["Mark Evans", "Evans", "Mark"],
        "3 formes du hash d'Endou"
    );
    assert_eq!(name, Some("Mark"), "nameHash → prénom (last-wins)");
    assert_eq!(last, Some("Evans"), "lastNameHash → nom de famille");

    for n in nouns.iter().take(3) {
        eprintln!(
            "  {} : name={:?} alt={:?}",
            n.hash_id.to_hex(),
            n.name,
            n.alt_names
        );
    }
    eprintln!(
        "✓ END-TO-END OK : nie_data::chara_text résout {} noms du vrai dump",
        nouns.len()
    );
}
