#![allow(clippy::pedantic)]
//! Tests golden `happen_event_npc` — événements NPC du monde d'IEVR, sur le vrai dump
//! (skip silencieux si absent — fragment copyright Level-5 non committé) :
//!
//! - `data/common/gamedata/system/happen_event_npc_common.cfg.bin.json`
//!
//! Port direct depuis la structure du dump (pas de référence inagle) ; valeurs assérées copiées
//! octet pour octet, y compris la résolution de tranches imbriquées event → aimed → weapon.

mod common;

use nie_data::happen_event_npc::parse_happen_event_npc_common;
use nie_data::hash::HashId;

const PATH: &str = "system/happen_event_npc_common.cfg.bin.json";

fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("lecture {}: {e}", chemin_abs.display()));
    Some(
        serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("JSON {}: {e}", chemin_abs.display())),
    )
}

#[test]
fn comptes_des_quatre_listes() {
    let Some(root) = load() else { return };
    let cfg = parse_happen_event_npc_common(&root);
    assert_eq!(cfg.weapon_info.len(), 24);
    assert_eq!(cfg.weapon_another_info.len(), 24);
    assert_eq!(cfg.aimed_info.len(), 14);
    assert_eq!(cfg.common_info.len(), 15);
}

#[test]
fn weapon_pools_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_happen_event_npc_common(&root);
    assert_eq!(cfg.weapon_info[0].weapon_id, HashId(0xC4C2_C944));
    assert_eq!(cfg.weapon_info[0].weight, 3);
    assert_eq!(cfg.weapon_info[23].weapon_id, HashId(0x89F3_E005));
    assert_eq!(cfg.weapon_info[23].weight, 1);
    assert_eq!(cfg.weapon_another_info[0].weapon_id, HashId(0x0CD9_5EC2));
}

#[test]
fn aimed_info_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_happen_event_npc_common(&root);
    let a0 = &cfg.aimed_info[0];
    assert_eq!(a0.npc_type_id, HashId(0x11F1_F540));
    assert_eq!(a0.on_loop_mot_id, HashId::ZERO);
    assert_eq!(a0.ow_charaparam_id, HashId(0xEF12_775F));
    assert_eq!(a0.weapon_info, [0, 2]);
    assert_eq!(a0.on_loop_npc_act_type, -1);
    assert!(!a0.is_not_battle_in);
    let a13 = &cfg.aimed_info[13];
    assert_eq!(a13.npc_type_id, HashId(0x08EA_C401));
    assert_eq!(a13.weapon_info, [22, 2]);
    // L'entrée [1] a is_not_battle_in = true (vérité terrain).
    assert!(cfg.aimed_info[1].is_not_battle_in);
}

#[test]
fn common_info_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_happen_event_npc_common(&root);
    let e0 = &cfg.common_info[0];
    assert_eq!(e0.event_id, HashId(0xFDB3_DCB3));
    assert_eq!(e0.event_type, 1);
    assert_eq!(e0.recast_min, 99999);
    assert_eq!(e0.nice_point_max, 40);
    assert_eq!(e0.nice_point_min, 20);
    assert_eq!(e0.on_loop_mot_id, HashId(0x84B1_2BAE));
    assert_eq!(e0.on_loop_npc_act_type, 15);
    let e14 = &cfg.common_info[14];
    assert_eq!(e14.event_id, HashId(0xEB2E_7E07));
    assert_eq!(e14.event_type, 7);
    assert_eq!(e14.nice_point_max, 1000);
    assert_eq!(e14.aimed_info, [13, 1]);
    assert_eq!(e14.weapon_info, [22, 2]);
    assert_eq!(e14.y_times_text_id, HashId(0xE38E_227C));
}

#[test]
fn resolution_imbriquee_event_aimed_weapon() {
    let Some(root) = load() else { return };
    let cfg = parse_happen_event_npc_common(&root);
    // event[14].aimed_info [13,1] → aimed[13] ; aimed[13].weapon_info [22,2] → weapon[22],weapon[23].
    let e14 = &cfg.common_info[14];
    let aimed = cfg.aimed_of_event(e14);
    assert_eq!(aimed.len(), 1);
    assert_eq!(aimed[0].npc_type_id, HashId(0x08EA_C401));
    let weapons = cfg.weapons_of_aimed(&aimed[0]);
    assert_eq!(weapons.len(), 2);
    assert_eq!(weapons[1].weapon_id, HashId(0x89F3_E005));
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_atteint_azalee() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key("happen_event_npc_common.cfg.bin"),
        "happen_event_npc_common"
    );
    let (label, json) =
        decode_by_key("happen_event_npc_common", &root).expect("famille typée câblée");
    assert_eq!(label, "happen_event_npc");
    assert_eq!(json["common_info"].as_array().map(Vec::len), Some(15));
    assert_eq!(json["aimed_info"].as_array().map(Vec::len), Some(14));
}

#[test]
fn cond_decode_reel() {
    use nie_data::unlock_condition::UnlockType;
    let Some(root) = load() else { return };
    let cfg = parse_happen_event_npc_common(&root);
    // [0] : condition story (namespace 0xB91936DA), seuil 10040 (1 feuille, opcode 0x05).
    let c0 = cfg.common_info[0].decode_cond();
    assert_eq!(c0.kind, UnlockType::Story);
    assert_eq!(c0.story_threshold, Some(10040));
    // [14] : opcode 0x17, 4 feuilles = 2 story (b91936da) + 2 event-flag → Composite (story + events).
    let c14 = cfg.common_info[14].decode_cond();
    assert_eq!(c14.kind, UnlockType::Composite);
    assert!(
        c14.story_threshold.is_some(),
        "seuil story présent (2 feuilles story)"
    );
    assert!(
        !c14.required_events.is_empty(),
        "feuilles event-flag présentes"
    );
}
