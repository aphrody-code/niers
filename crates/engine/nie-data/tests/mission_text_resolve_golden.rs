#![allow(clippy::pedantic)]
//! Tests golden `mission::resolve_name` — port de la jointure nom de `mission-config.ts`
//! (`loadTextFile("mission").get(nameIdHex)`, `nameId` = var[2] = `raw[0]`).
//!
//! Vérité terrain (via `cargo run -p nie-game --example extract_mission_text`) : le
//! `mission_config` du jeu est un STUB (`msa999999`, mission_id `0x3C67436B`, nameId `0x4E7B90D9`)
//! et `mission_text` fr ne porte que des placeholders (`0x57EE9AB9` = « ミッションタイトル999999 »,
//! `0xCF845499` = « ミッション詳細999999 »). Le mécanisme de jointure est donc vérifié sur fixtures.

use nie_data::hash::HashId;
use nie_data::mission::{MissionConfigInfo, resolve_name};

fn mission(name_id: u32) -> MissionConfigInfo {
    let mut raw = [HashId::ZERO; 14];
    raw[0] = HashId(name_id); // raw[0] = var[2] = nameId
    MissionConfigInfo {
        mission_id: HashId(0x3C67_436B),
        mission_code: "msa999999".into(),
        raw,
    }
}

fn mission_text() -> Vec<(HashId, String)> {
    vec![
        (
            HashId(0x57EE_9AB9),
            String::from("ミッションタイトル999999"),
        ),
        (HashId(0xCF84_5499), String::from("ミッション詳細999999")),
    ]
}

#[test]
fn name_id_is_raw0() {
    assert_eq!(mission(0x4E7B_90D9).name_id(), HashId(0x4E7B_90D9));
}

#[test]
fn resolves_when_present() {
    let t = mission_text();
    // Une mission dont le nameId pointe l'entrée placeholder réelle.
    assert_eq!(
        resolve_name(&mission(0x57EE_9AB9), &t),
        Some("ミッションタイトル999999")
    );
}

#[test]
fn stub_nameid_absent_is_none() {
    let t = mission_text();
    // Le vrai stub `msa999999` : nameId 0x4E7B90D9 n'est PAS dans mission_text → None (fidèle).
    assert_eq!(resolve_name(&mission(0x4E7B_90D9), &t), None);
}
