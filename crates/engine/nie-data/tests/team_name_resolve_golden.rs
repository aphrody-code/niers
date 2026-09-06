#![allow(clippy::pedantic)]
//! Tests golden `belong_team::resolve_team_name` — port de la jointure nom d'équipe de
//! `belong-team.ts` (`team_text.get(teamNameTextId)`).
//!
//! Vérité terrain (via `cargo run -p nie-game --example extract_team_names`) : **208/208** équipes
//! résolues en fr — « Raimon », « Royal Academy », « Inazuma KFT », « Cybertech »… ; et la chaîne
//! `chara_base.belong_team_id` → `belong_team` → nom : **Endou `c01000010` → « Raimon »**.

use nie_data::belong_team::{BelongTeamInfo, resolve_team_name};
use nie_data::hash::HashId;
use serde_json::json;

/// Construit un `BelongTeamInfo` via `from_value` (forme RDBN) avec un `teamNameTextId` donné.
fn team(name_text_id: u32) -> BelongTeamInfo {
    let v = json!({
        "belongTeamId": "0xF01BB293",
        "teamNameTextId": format!("0x{name_text_id:08X}"),
    });
    BelongTeamInfo::from_value(&v)
}

fn team_text() -> Vec<(HashId, String)> {
    vec![
        (HashId(0x0000_5001), String::from("Raimon")),
        (HashId(0x0000_5002), String::from("Royal Academy")),
    ]
}

#[test]
fn resolves_real_team_names() {
    let t = team_text();
    assert_eq!(resolve_team_name(&team(0x0000_5001), &t), Some("Raimon"));
    assert_eq!(
        resolve_team_name(&team(0x0000_5002), &t),
        Some("Royal Academy")
    );
}

#[test]
fn missing_hash_is_none() {
    assert_eq!(resolve_team_name(&team(0xDEAD_BEEF), &team_text()), None);
}
