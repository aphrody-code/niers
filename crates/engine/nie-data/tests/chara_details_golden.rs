#![allow(clippy::pedantic)]
//! Tests golden `chara_details` — port 1:1 d'inagle `packages/inagle/src/parsers/chara-details.ts`.
//!
//! Vérité terrain = le vrai `chara_details_config_0.00.00.00.cfg.bin` (VFS IEVR, liste RDBN
//! `m_charaDetailsList`, **550 lignes**), via `cargo run -p nie-game --example extract_chara_details`.
//! Trois premières lignes réelles figées ci-dessous ; l'égalité END-TO-END (550 lignes, ligne 0
//! exacte) est asserée par l'exemple.

use nie_data::chara_details::{CharaDetails, find_by_chara_id, parse_chara_details};
use nie_data::hash::HashId;
use serde_json::{Value, json};

/// Une ligne RDBN `m_charaDetailsList` (hash en chaîne hex, `personality_type` en nombre).
fn row(chara_id: &str, pers: i64, first: &str, m2: &str, f2: &str) -> Value {
    json!({
        "chara_id": chara_id,
        "personality_type": pers,
        "first_person": first,
        "second_person_male": m2,
        "second_person_female": f2,
    })
}

fn root_fixture() -> Value {
    json!({
        "lists": [{
            "name": "m_charaDetailsList",
            "typeName": "CHARA_DETAILS",
            "values": [
                row("0x677ADF02", 1, "0xF705095D", "0x00000000", "0x00000000"),
                row("0x39AB12D3", 1, "0x00000000", "0x00000000", "0x00000000"),
                row("0x596C9B36", 1, "0x00000000", "0x00000000", "0x00000000"),
            ]
        }]
    })
}

#[test]
fn parses_three_real_rows() {
    let d = parse_chara_details(&root_fixture());
    assert_eq!(d.len(), 3);
    assert_eq!(d[0].chara_id, HashId(0x677A_DF02));
    assert_eq!(d[0].personality_type, 1);
    assert_eq!(d[0].first_person, HashId(0xF705_095D));
    assert_eq!(d[0].second_person_male, HashId::ZERO);
    assert_eq!(d[0].second_person_female, HashId::ZERO);
    assert_eq!(d[1].chara_id.to_hex(), "0x39AB12D3");
    assert_eq!(d[2].chara_id.to_hex(), "0x596C9B36");
    // Pas de pronom spécifique → hash nul.
    assert_eq!(d[1].first_person, HashId::ZERO);
}

#[test]
fn lookup_by_chara_id() {
    let d = parse_chara_details(&root_fixture());
    let found = find_by_chara_id(&d, HashId(0x596C_9B36)).expect("présent");
    assert_eq!(found.personality_type, 1);
    assert!(find_by_chara_id(&d, HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn empty_or_missing_list_yields_nothing() {
    assert!(parse_chara_details(&json!({ "lists": [] })).is_empty());
    assert!(parse_chara_details(&json!({})).is_empty());
    // Mauvais nom de liste → vide.
    let other =
        json!({ "lists": [{ "name": "m_other", "values": [row("0x1", 0, "0x0", "0x0", "0x0")] }] });
    assert!(parse_chara_details(&other).is_empty());
}

#[test]
fn entry_is_copy() {
    let a = CharaDetails {
        chara_id: HashId(1),
        personality_type: 2,
        first_person: HashId(3),
        second_person_male: HashId(4),
        second_person_female: HashId(5),
    };
    let b = a;
    assert_eq!(a, b);
}
