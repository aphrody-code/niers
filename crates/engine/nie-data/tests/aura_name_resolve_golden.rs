#![allow(clippy::pedantic)]
//! Tests golden `aura::resolve_name`/`resolve_description` — résolution des noms d'Avatar / Keshin /
//! hissatsu via `skill_text` (NOUN_INFO = nom, TEXT_INFO = description).
//!
//! Vérité terrain (via `cargo run -p nie-game --example extract_aura_names`) : **443/443** auras
//! résolues en fr — noms ET descriptions ; ex. `0x7978E1FA` (wks00020) → « Lancelot, le spadassin
//! héroïque » / « Le spadassin héroïque, un esprit du feu. » ; `0x2F22467C` → « Apollon, dieu du
//! soleil » ; `0x4A8CE94F` → « Surt, géant du feu ».

use nie_data::aura::{AuraCmd, resolve_description, resolve_name};
use nie_data::cfgbin::Node;
use nie_data::hash::HashId;
use serde_json::{Value, json};

fn ivar(v: i64) -> Value {
    json!({ "type": "Int", "value": v.to_string() })
}
fn svar(v: &str) -> Value {
    json!({ "type": "String", "value": v })
}

/// Construit un `AuraCmd` via `from_node` : var0=auraId, var1=assetCode, var2=nameId, var3=descId.
fn aura(name_id: u32, desc_id: u32) -> AuraCmd {
    let vars = vec![
        ivar(0x7978_E1FA), // aura_id
        svar("wks00020"),  // asset_code
        ivar(i64::from(name_id)),
        ivar(i64::from(desc_id)),
        ivar(0),
        ivar(0),
        ivar(0),
        ivar(0),
        ivar(1), // element
        ivar(0),
    ];
    let node = json!({ "name": "AURA_CMD_INFO_0", "variables": vars, "children": [] });
    AuraCmd::from_node(Node::new(&node)).expect("aura valide (≥ 4 vars)")
}

fn skill_text() -> Vec<(HashId, String)> {
    vec![
        (
            HashId(0x0000_A001),
            String::from("Lancelot, le spadassin héroïque"),
        ),
        (
            HashId(0x0000_A002),
            String::from("Le spadassin héroïque, un esprit du feu."),
        ),
    ]
}

#[test]
fn resolves_avatar_name_and_description() {
    let a = aura(0x0000_A001, 0x0000_A002);
    let t = skill_text();
    assert_eq!(
        resolve_name(&a, &t),
        Some("Lancelot, le spadassin héroïque")
    );
    assert_eq!(
        resolve_description(&a, &t),
        Some("Le spadassin héroïque, un esprit du feu.")
    );
    // L'asset_code et l'id sont bien lus par from_node.
    assert_eq!(a.aura_id, HashId(0x7978_E1FA));
    assert_eq!(a.asset_code, "wks00020");
}

#[test]
fn missing_text_is_none() {
    let a = aura(0xDEAD_BEEF, 0xFEED_FACE);
    assert_eq!(resolve_name(&a, &skill_text()), None);
    assert_eq!(resolve_description(&a, &skill_text()), None);
}
