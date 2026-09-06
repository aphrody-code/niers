#![allow(clippy::pedantic)]
//! Tests golden `chara_description` — port 1:1 d'inagle
//! `packages/inagle/src/parsers/chara-description.ts` (+ `sanitize_text`).
//!
//! Vérité terrain = le vrai `common/text/fr/chara_description_text.cfg.bin` (VFS IEVR, T2B), via
//! `cargo run -p nie-game --example extract_chara_description` : **5568** descriptions non-vides,
//! **0** balise `<>` résiduelle, et le hash `0xFA43BBBE` = description d'**Endou** (`c01000010`,
//! cross-link `chara_base.description_hash`) commençant par « La passion du football l'emportera
//! toujours. » sur 2 lignes (LF préservé). L'exemple asserte ces faits sur le dump réel ; ce
//! fichier fige la logique de parse + nettoyage sur fixtures contrôlées.

use nie_data::cfgbin::Node;
use nie_data::chara_description::{find_by_hash, parse_chara_descriptions, parse_description_node};
use nie_data::hash::HashId;
use serde_json::{Value, json};

fn ivar(v: i64) -> Value {
    json!({ "type": "Int", "value": v.to_string() })
}
fn svar(v: &str) -> Value {
    json!({ "type": "String", "value": v })
}

/// Noeud `TEXT_INFO` = `[hashId (Int), 0 (Int), description (String)]`.
fn text_info(name: &str, hash: i64, desc: &str) -> Value {
    json!({ "name": name, "variables": [ivar(hash), ivar(0), svar(desc)], "children": [] })
}

#[test]
fn parses_and_sanitizes() {
    // Entrée brute avec balise couleur + furigana + balise valeur → texte nettoyé.
    let n = text_info(
        "TEXT_INFO_0",
        0x1234_5678,
        "<COL:FF0000>Texte<CLO> avec [漢字/かんじ] et <VAL:5>",
    );
    let d = parse_description_node(&Node::new(&n)).expect("valide");
    assert_eq!(d.hash_id, HashId(0x1234_5678));
    assert_eq!(d.description, "Texte avec 漢字 et 5");
}

#[test]
fn rejects_empty_after_sanitize() {
    // Description faite uniquement de balises → vide après nettoyage → rejetée.
    let n = text_info("TEXT_INFO_1", 0x1, "<COL:FF0000><CLO>");
    assert_eq!(parse_description_node(&Node::new(&n)), None);
    // Chaîne vide.
    let n2 = text_info("TEXT_INFO_2", 0x2, "");
    assert_eq!(parse_description_node(&Node::new(&n2)), None);
}

#[test]
fn rejects_bad_types_and_short() {
    // < 3 variables.
    let short = json!({ "name": "TEXT_INFO_3", "variables": [ivar(1), ivar(0)], "children": [] });
    assert_eq!(parse_description_node(&Node::new(&short)), None);
    // var[0] non-Int.
    let bad0 = json!({ "name": "TEXT_INFO_4",
        "variables": [svar("x"), ivar(0), svar("desc")], "children": [] });
    assert_eq!(parse_description_node(&Node::new(&bad0)), None);
    // var[2] non-String.
    let bad2 = json!({ "name": "TEXT_INFO_5",
        "variables": [ivar(1), ivar(0), ivar(9)], "children": [] });
    assert_eq!(parse_description_node(&Node::new(&bad2)), None);
}

#[test]
fn walk_under_begin_node() {
    let root = json!({
        "entries": [{
            "name": "TEXT_INFO_BEGIN_0",
            "variables": [ivar(3)], // compteur → écarté (< 3 vars)
            "children": [
                text_info("TEXT_INFO_0", 0xAAAA_0001, "Première description"),
                text_info("TEXT_INFO_1", 0xAAAA_0002, "<CLO>Deuxième<TX0>"),
                text_info("TEXT_INFO_2", 0xAAAA_0003, "   "), // vide après trim → rejetée
            ]
        }]
    });
    let descs = parse_chara_descriptions(&root);
    assert_eq!(descs.len(), 2, "2 descriptions non-vides (la 3ᵉ est vide)");
    assert_eq!(
        find_by_hash(&descs, HashId(0xAAAA_0001)),
        Some("Première description")
    );
    assert_eq!(find_by_hash(&descs, HashId(0xAAAA_0002)), Some("Deuxième"));
    assert_eq!(find_by_hash(&descs, HashId(0xAAAA_0003)), None);
    assert_eq!(find_by_hash(&descs, HashId(0xDEAD_BEEF)), None);
}
