#![allow(clippy::pedantic)]
//! Tests golden `chara_base` — port 1:1 d'inagle `packages/inagle/src/parsers/chara-base.ts`.
//!
//! Vérité terrain = le vrai `character/chara_base_1.03.98.00.cfg.bin` (VFS IEVR, T2B), via
//! `cargo run -p nie-game --example extract_chara_base` : **7223** noeuds valides, genres
//! `{0:1349, 1:4753, 2:1064, 4:1, 5:56}`. Deux entrées réelles figées : `c01000010` =
//! **Endou Mamoru** (charaId 0x99A1C150, name 0xE551FF12, last 0xE5655F9E, gender 1,
//! series 0x62E8448F, team 0xF01BB293, desc 0xFA43BBBE) ; et `npc0010` (charaId 0xA791C2B5,
//! name 0x0, gender 0, sans série/équipe). L'égalité END-TO-END (7223 noeuds) → l'exemple.

use nie_data::cfgbin::Node;
use nie_data::chara_base::{find_by_chara_id, parse_all_chara_base, parse_base_node};
use nie_data::hash::HashId;
use serde_json::{Value, json};

fn ivar(v: i64) -> Value {
    json!({ "type": "Int", "value": v.to_string() })
}
fn svar(v: &str) -> Value {
    json!({ "type": "String", "value": v })
}

/// Construit un noeud à `n` variables (Int 0 par défaut), avec surcharges Int et String.
fn node(name: &str, n: usize, ints: &[(usize, i64)], strs: &[(usize, &str)]) -> Value {
    let mut vars: Vec<Value> = (0..n).map(|_| ivar(0)).collect();
    for (i, v) in ints {
        vars[*i] = ivar(*v);
    }
    for (i, s) in strs {
        vars[*i] = svar(s);
    }
    json!({ "name": name, "variables": vars, "children": [] })
}

fn endou() -> Value {
    node(
        "CHARA_BASE_INFO_0",
        20,
        &[
            (0, 0x99A1_C150),
            (3, 0xE551_FF12),
            (4, 0xE565_5F9E),
            (11, 1),
            (15, 0x62E8_448F),
            (16, 0xF01B_B293),
            (19, 0xFA43_BBBE),
        ],
        &[(1, "c01000010")],
    )
}

fn npc0010() -> Value {
    node(
        "CHARA_BASE_INFO_1",
        20,
        &[(0, 0xA791_C2B5), (3, 0), (11, 0)],
        &[(1, "npc0010")],
    )
}

#[test]
fn endou_full_fields() {
    let n = endou();
    let b = parse_base_node(&Node::new(&n)).expect("Endou valide");
    assert_eq!(b.chara_id, HashId(0x99A1_C150));
    assert_eq!(b.internal_code, "c01000010");
    assert_eq!(b.name_hash, HashId(0xE551_FF12));
    assert_eq!(b.last_name_hash, Some(HashId(0xE565_5F9E)));
    assert_eq!(b.gender, 1);
    assert_eq!(b.series_id, Some(HashId(0x62E8_448F)));
    assert_eq!(b.belong_team_id, Some(HashId(0xF01B_B293)));
    assert_eq!(b.description_hash, Some(HashId(0xFA43_BBBE)));
}

#[test]
fn npc_minimal_optionals_absent() {
    let n = npc0010();
    let b = parse_base_node(&Node::new(&n)).expect("npc valide");
    assert_eq!(b.chara_id, HashId(0xA791_C2B5));
    assert_eq!(b.internal_code, "npc0010");
    assert_eq!(b.name_hash, HashId::ZERO);
    assert_eq!(b.gender, 0); // var[11] = 0 explicite (pas le défaut 1)
    // Champs optionnels à 0 → None.
    assert_eq!(b.last_name_hash, None);
    assert_eq!(b.series_id, None);
    assert_eq!(b.belong_team_id, None);
    assert_eq!(b.description_hash, None);
}

#[test]
fn gender_defaults_to_one_when_var11_absent() {
    // Noeud minimal (4 vars) : pas de var[11] → genre défaut 1 (port de `let gender = 1`).
    let n = node(
        "CHARA_BASE_INFO_2",
        4,
        &[(0, 0x10), (3, 0x20)],
        &[(1, "c99")],
    );
    let b = parse_base_node(&Node::new(&n)).expect("4 vars suffisent");
    assert_eq!(b.gender, 1);
    assert_eq!(b.last_name_hash, None); // var[4] absent
}

#[test]
fn rejects_bad_types_and_short_nodes() {
    // < 4 variables → None.
    assert_eq!(
        parse_base_node(&Node::new(&node("CHARA_BASE_INFO_3", 3, &[], &[]))),
        None
    );
    // var[0] non-Int (String) → None.
    let n = node("CHARA_BASE_INFO_4", 6, &[(3, 1)], &[(0, "x"), (1, "c")]);
    assert_eq!(parse_base_node(&Node::new(&n)), None);
    // var[1] non-String (Int) → None.
    let n2 = node("CHARA_BASE_INFO_5", 6, &[(0, 1), (1, 2), (3, 3)], &[]);
    assert_eq!(parse_base_node(&Node::new(&n2)), None);
}

#[test]
fn walk_matches_info_and_battle_excludes_list() {
    let root = json!({
        "entries": [{
            "name": "CHARA_BASE_INFO_LIST_BEG_0",
            "variables": [ivar(7223)],
            "children": [
                endou(),
                npc0010(),
                // BATTLE variant doit aussi matcher.
                node("CHARA_BASE_BATTLE_0", 20, &[(0, 0x55), (3, 0x66)], &[(1, "c01000010_b")]),
                // Un noeud sans suffixe numérique ne doit PAS matcher (regex \d+$).
                node("CHARA_BASE_INFO_LIST_BEG_99x", 20, &[(0, 0x77)], &[(1, "noise")]),
            ]
        }]
    });
    let bases = parse_all_chara_base(&root);
    assert_eq!(
        bases.len(),
        3,
        "INFO + npc + BATTLE ; LIST_BEG et non-\\d exclus"
    );

    let endou = find_by_chara_id(&bases, HashId(0x99A1_C150)).expect("Endou");
    assert_eq!(endou.internal_code, "c01000010");
    assert!(bases.iter().any(|b| b.internal_code == "c01000010_b"));
    assert!(find_by_chara_id(&bases, HashId(0xDEAD_BEEF)).is_none());
}
