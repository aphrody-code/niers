#![allow(clippy::pedantic)]
//! Tests golden — résolution de la **chaîne personnage** : `chara_base` → `chara_text` (prénom/nom)
//! → `chara_description` (bio), via les jointures `nameHash`/`lastNameHash`/`descriptionHash`.
//!
//! Vérité terrain (via `cargo run -p nie-game --example extract_chara_full`) : sur le vrai jeu,
//! **6470/7223** personnages ont un prénom résolu ; Endou `c01000010` (charaId `0x99A1C150`) →
//! prénom « Mark », nom « Evans », bio « La passion du football l'emportera toujours.… ».

use nie_data::chara_base::{CharaBase, resolve_description, resolve_first_name, resolve_last_name};
use nie_data::chara_description::ParsedDescription;
use nie_data::chara_text::ParsedNoun;
use nie_data::hash::HashId;

fn endou() -> CharaBase {
    CharaBase {
        chara_id: HashId(0x99A1_C150),
        internal_code: "c01000010".into(),
        name_hash: HashId(0xE551_FF12),
        last_name_hash: Some(HashId(0xE565_5F9E)),
        description_hash: Some(HashId(0xFA43_BBBE)),
        gender: 1,
        series_id: Some(HashId(0x62E8_448F)),
        belong_team_id: Some(HashId(0xF01B_B293)),
    }
}

fn nouns() -> Vec<ParsedNoun> {
    vec![
        ParsedNoun {
            hash_id: HashId(0xE551_FF12),
            name: "Mark".into(),
            alt_names: vec![],
        },
        ParsedNoun {
            hash_id: HashId(0xE565_5F9E),
            name: "Evans".into(),
            alt_names: vec![],
        },
    ]
}

fn descs() -> Vec<ParsedDescription> {
    vec![ParsedDescription {
        hash_id: HashId(0xFA43_BBBE),
        description: "La passion du football l'emportera toujours.".into(),
    }]
}

#[test]
fn resolves_full_identity() {
    let e = endou();
    assert_eq!(resolve_first_name(&e, &nouns()), Some("Mark"));
    assert_eq!(resolve_last_name(&e, &nouns()), Some("Evans"));
    assert_eq!(
        resolve_description(&e, &descs()),
        Some("La passion du football l'emportera toujours.")
    );
}

#[test]
fn absent_optional_hashes_are_none() {
    let mut npc = endou();
    npc.last_name_hash = None;
    npc.description_hash = None;
    // Prénom toujours résolu, mais nom/bio absents (hash nul) → None sans même consulter la table.
    assert_eq!(resolve_first_name(&npc, &nouns()), Some("Mark"));
    assert_eq!(resolve_last_name(&npc, &nouns()), None);
    assert_eq!(resolve_description(&npc, &descs()), None);
}

#[test]
fn unresolved_hash_is_none() {
    let mut e = endou();
    e.name_hash = HashId(0xDEAD_BEEF); // pas dans la table
    assert_eq!(resolve_first_name(&e, &nouns()), None);
}
