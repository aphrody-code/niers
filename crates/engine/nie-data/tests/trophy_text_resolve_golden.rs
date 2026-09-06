#![allow(clippy::pedantic)]
//! Tests golden `trophy::resolve_name`/`resolve_description`/`decode_unlock` — port des jointures
//! de `trophy-config.ts` (nom/desc via `trophy_text`) + décodage `open_cond` via `unlock_condition`.
//!
//! Vérité terrain (via `cargo run -p nie-game --example extract_trophy_text`) : **231/231** noms
//! résolus, 108 descriptions, conditions `{always:142, story:35, eventFlag:54}`. `TROPHY_INFO_0`
//! (« 鍵をなくしたおじいさん ») : name_text_id `0x8840F978` → « Vieil homme qui a perdu sa clé »,
//! desc_text_id `0xC275B972` → « Offrir son aide. ».

use nie_data::cfgbin::Node;
use nie_data::hash::HashId;
use nie_data::trophy::{TrophyInfo, decode_unlock, resolve_description, resolve_name};
use nie_data::unlock_condition::UnlockType;
use serde_json::{Value, json};

fn ivar(v: i64) -> Value {
    json!({ "type": "Int", "value": v.to_string() })
}
fn svar(v: &str) -> Value {
    json!({ "type": "String", "value": v })
}

/// Construit un `TrophyInfo` via `from_node` : var[4]=nameId, var[6]=descId, var[8]=openCond.
fn trophy(name_id: u32, desc_id: u32, open_cond: &str) -> TrophyInfo {
    let mut vars: Vec<Value> = (0..18).map(|_| ivar(0)).collect();
    vars[2] = ivar(0x7D48_804A); // trophy_id
    vars[3] = svar("activity_story_miniquest_001"); // code
    vars[4] = ivar(i64::from(name_id));
    vars[5] = svar("鍵をなくしたおじいさん"); // debug_name
    vars[6] = ivar(i64::from(desc_id));
    vars[8] = svar(open_cond);
    let node = json!({ "name": "TROPHY_INFO_0", "variables": vars, "children": [] });
    TrophyInfo::from_node(Node::new(&node))
}

fn trophy_text() -> Vec<(HashId, String)> {
    vec![
        (
            HashId(0x8840_F978),
            String::from("Vieil homme qui a perdu sa clé"),
        ),
        (HashId(0xC275_B972), String::from("Offrir son aide.")),
    ]
}

#[test]
fn resolves_real_name_and_description() {
    let t = trophy(0x8840_F978, 0xC275_B972, "0");
    let txt = trophy_text();
    assert_eq!(
        resolve_name(&t, &txt),
        Some("Vieil homme qui a perdu sa clé")
    );
    assert_eq!(resolve_description(&t, &txt), Some("Offrir son aide."));
}

#[test]
fn missing_text_is_none() {
    let t = trophy(0xDEAD_BEEF, 0xFEED_FACE, "0");
    assert_eq!(resolve_name(&t, &trophy_text()), None);
    assert_eq!(resolve_description(&t, &trophy_text()), None);
}

#[test]
fn open_cond_sentinel_is_always() {
    // var[8] = "0" (sentinelle Int « pas de condition ») → décodage trivial Always.
    let t = trophy(0x8840_F978, 0xC275_B972, "0");
    assert_eq!(decode_unlock(&t).kind, UnlockType::Always);
}

#[test]
fn open_cond_story_blob_decodes() {
    // Un vrai blob story (ev01, seuil 20010) → condition Story (réutilise le décodeur unlock).
    let t = trophy(0x1, 0x2, "AAAAAA8FNbkZNtoAAQAyAABOKnE=");
    let c = decode_unlock(&t);
    assert_eq!(c.kind, UnlockType::Story);
    assert_eq!(c.story_threshold, Some(20010));
}
