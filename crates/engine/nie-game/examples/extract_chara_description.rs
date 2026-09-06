//! Validation END-TO-END de `nie_data::chara_description` (+ `text::sanitize_text`) sur le VRAI
//! jeu : lit `common/text/fr/chara_description_text.cfg.bin` (T2B), le convertit en forme iecode,
//! exécute `parse_chara_descriptions`, et affiche le nombre + des échantillons réels. Cross-link :
//! la description de hash `0xFA43BBBE` = celle d'Endou (`chara_base` desc de `c01000010`).
//!
//! Usage : `cargo run -p nie-game --example extract_chara_description`
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
                && p.rsplit('/').next().is_some_and(|b| {
                    b.starts_with("chara_description_text") && b.ends_with(".cfg.bin")
                })
        })
        .min()
        .expect("chara_description_text fr introuvable");
    eprintln!("chara_description = {path}");
    let bytes = vfs.read(&path).expect("read");
    let file = cfgbin::parse_t2b(&bytes).expect("parse_t2b");
    let root = json!({ "entries": to_iecode(&file.entries) });

    let descs = nie_data::chara_description::parse_chara_descriptions(&root);
    eprintln!("[module] descriptions non-vides = {}", descs.len());
    assert!(!descs.is_empty());

    // Aucune description ne doit contenir de balise résiduelle `<...>` ni de furigana `[../..]`.
    let with_tag = descs
        .iter()
        .filter(|d| d.description.contains('<') && d.description.contains('>'))
        .count();
    eprintln!("descriptions avec balise `<>` résiduelle = {with_tag} (attendu 0)");
    assert_eq!(with_tag, 0, "sanitize_text doit retirer toutes les balises");

    for d in descs.iter().take(2) {
        let snippet: String = d.description.chars().take(60).collect();
        eprintln!("  {} : {:?}…", d.hash_id.to_hex(), snippet);
    }

    // Cross-link Endou : descriptionHash 0xFA43BBBE (de chara_base c01000010).
    let endou =
        nie_data::chara_description::find_by_hash(&descs, nie_data::hash::HashId(0xFA43_BBBE))
            .expect("description d'Endou (0xFA43BBBE) présente en fr");
    let snippet: String = endou.chars().take(80).collect();
    eprintln!("Endou (0xFA43BBBE) : {snippet:?}…");
    assert!(
        endou.starts_with("La passion du football l'emportera toujours."),
        "texte Endou inattendu : {snippet:?}"
    );
    assert!(
        endou.contains('\n'),
        "la description d'Endou a 2 lignes (LF préservé)"
    );

    // Cross-check : le résolveur GÉNÉRIQUE `text::parse_text_file` doit résoudre le même texte
    // (chara_description_text est un fichier TEXT_INFO → index 2).
    let generic = nie_data::text::parse_text_file(&root);
    let g_endou = nie_data::text::find_text(&generic, nie_data::hash::HashId(0xFA43_BBBE))
        .expect("résolveur générique trouve Endou");
    assert_eq!(
        g_endou, endou,
        "parse_text_file == parse_chara_descriptions sur Endou"
    );
    eprintln!(
        "✓ cross-check : text::parse_text_file résout {} entrées (générique == spécifique)",
        generic.len()
    );
    eprintln!(
        "✓ END-TO-END OK : nie_data::chara_description décode {} descriptions nettoyées",
        descs.len()
    );
}
