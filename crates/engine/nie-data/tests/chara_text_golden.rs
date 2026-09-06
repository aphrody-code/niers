#![allow(clippy::pedantic)]
//! Tests golden `chara_text` — port 1:1 d'inagle `packages/inagle/src/parsers/chara-text.ts`.
//!
//! Vérité terrain = le vrai `common/text/fr/chara_text.cfg.bin` (VFS IEVR, T2B), via
//! `cargo run -p nie-game --example extract_chara_text` : **20802** noms `NOUN_INFO`. Le hash
//! d'Endou `0xE551FF12` (= `chara_base.nameHash` de `c01000010`) a **3 entrées** dans l'ordre
//! `["Mark Evans", "Evans", "Mark"]` ; la sémantique **last-wins** de `buildNameMap` résout donc
//! `nameHash → "Mark"` (prénom) et `lastNameHash 0xE5655F9E → "Evans"`.

use nie_data::cfgbin::Node;
use nie_data::chara_text::{find_name, parse_all_nouns, parse_noun_node, parse_roma_node};
use nie_data::hash::HashId;
use serde_json::{Value, json};

fn ivar(v: i64) -> Value {
    json!({ "type": "Int", "value": v.to_string() })
}
fn svar(v: &str) -> Value {
    json!({ "type": "String", "value": v })
}

/// Noeud `NOUN_INFO` : `[hash, 0, "", "", "", <strings…>]` (≥ 6 variables).
fn noun(name: &str, hash: i64, strings: &[&str]) -> Value {
    let mut vars = vec![ivar(hash), ivar(0), svar(""), svar(""), svar("")];
    for s in strings {
        vars.push(svar(s));
    }
    while vars.len() < 6 {
        vars.push(svar(""));
    }
    json!({ "name": name, "variables": vars, "children": [] })
}

#[test]
fn primary_name_and_alts() {
    // Première chaîne non vide → name ; suivantes → alt_names ; nettoyage appliqué.
    let n = noun(
        "NOUN_INFO_0",
        0x1,
        &["<COL:F00>Mark<CLO>", "Marco", "マーク"],
    );
    let p = parse_noun_node(&Node::new(&n)).expect("valide");
    assert_eq!(p.hash_id, HashId(0x1));
    assert_eq!(p.name, "Mark"); // balise retirée
    assert_eq!(p.alt_names, ["Marco", "マーク"]);
}

#[test]
fn rejects_short_bad_type_and_empty() {
    // < 6 variables.
    let short = json!({ "name": "NOUN_INFO_1", "variables": [ivar(1), ivar(0), svar("x")], "children": [] });
    assert_eq!(parse_noun_node(&Node::new(&short)), None);
    // var[0] non-Int.
    let bad = noun("NOUN_INFO_2", 0x2, &["X"]);
    let mut bad = bad;
    bad["variables"][0] = svar("nope");
    assert_eq!(parse_noun_node(&Node::new(&bad)), None);
    // Aucune chaîne non vide → None.
    let empty = noun("NOUN_INFO_3", 0x3, &["", "   "]);
    assert_eq!(parse_noun_node(&Node::new(&empty)), None);
}

#[test]
fn walk_last_wins_for_duplicate_hash() {
    // Réplique exacte des 3 entrées réelles d'Endou (0xE551FF12) + le nom de famille (0xE5655F9E).
    let root = json!({
        "entries": [{
            "name": "NOUN_INFO_BEGIN_0",
            "variables": [ivar(4)], // compteur → écarté (BEGIN)
            "children": [
                noun("NOUN_INFO_0", 0xE551_FF12, &["Mark Evans"]),
                noun("NOUN_INFO_1", 0xE551_FF12, &["Evans"]),
                noun("NOUN_INFO_2", 0xE551_FF12, &["Mark"]),
                noun("NOUN_INFO_3", 0xE565_5F9E, &["Evans"]),
            ]
        }]
    });
    let nouns = parse_all_nouns(&root);
    assert_eq!(nouns.len(), 4, "4 NOUN_INFO (BEGIN exclu)");
    // last-wins : la 3ᵉ entrée (« Mark ») gagne pour le hash dupliqué.
    assert_eq!(find_name(&nouns, HashId(0xE551_FF12)), Some("Mark"));
    assert_eq!(find_name(&nouns, HashId(0xE565_5F9E)), Some("Evans"));
    assert_eq!(find_name(&nouns, HashId(0xDEAD_BEEF)), None);
}

#[test]
fn roma_lastname_then_firstname_trim_only() {
    // Romanisé : 1ʳᵉ chaîne → nom de famille, 2ᵉ → prénom, trim sans sanitize.
    let n = json!({
        "name": "NOUN_INFO_0",
        "variables": [ivar(0x10), svar("  Evans  "), svar(" Mark "), svar("ignored")],
        "children": []
    });
    let r = parse_roma_node(&Node::new(&n)).expect("valide");
    assert_eq!(r.hash_id, HashId(0x10));
    assert_eq!(r.last_name, "Evans");
    assert_eq!(r.first_name, "Mark");

    // < 3 variables → None.
    let short = json!({ "name": "NOUN_INFO_1", "variables": [ivar(1), svar("x")], "children": [] });
    assert_eq!(parse_roma_node(&Node::new(&short)), None);
}
