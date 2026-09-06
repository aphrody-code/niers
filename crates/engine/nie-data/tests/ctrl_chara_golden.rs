#![allow(clippy::pedantic)]
//! Tests golden `ctrl_chara` — port 1:1 d'inagle `packages/inagle/src/parsers/ctrl-chara-config.ts`.
//!
//! Vérité terrain = le vrai `party/ctrl_chara_config_1.04.17.00.cfg.bin` (VFS IEVR), via
//! `cargo run -p nie-game --example extract_ctrl_chara` :
//! - **155** noeuds `CTRL_CHR_DATA` (= « 155 character entries » de la doc inagle) ;
//! - distribution `controlType` `{0:22, 1:26, 2:107}` ; `flags` tous nuls ;
//! - 5 premiers réels (charaParamId, controlType, flags) :
//!   `0x686BE87E/1/0, 0x4346BBBD/2/0, 0x686BE87E/1/0, 0x0C072D7A/0/0, 0x4346BBBD/2/0`.
//!
//! L'égalité **END-TO-END** module-vs-dump sur les 155 noeuds est vérifiée par l'exemple
//! (assert intégré). Ce fichier fige les valeurs réelles en fixtures hors-ligne.

use nie_data::cfgbin::Node;
use nie_data::ctrl_chara::{
    CtrlCharaData, is_controllable, parse_all_ctrl_chara, parse_ctrl_chara_node,
};
use nie_data::hash::HashId;
use serde_json::{Value, json};

fn ivar(v: i64) -> Value {
    json!({ "type": "Int", "value": v.to_string() })
}

fn ctrl_node(name: &str, id: u32, ctrl: i64, flags: i64) -> Value {
    json!({
        "name": name,
        "variables": [ivar(i64::from(id)), ivar(ctrl), ivar(flags)],
        "children": []
    })
}

/// Les 5 premiers `CTRL_CHR_DATA` réels (charaParamId, controlType, flags).
const SAMPLES: [(u32, i64, i64); 5] = [
    (0x686B_E87E, 1, 0),
    (0x4346_BBBD, 2, 0),
    (0x686B_E87E, 1, 0),
    (0x0C07_2D7A, 0, 0),
    (0x4346_BBBD, 2, 0),
];

#[test]
fn single_node_real() {
    let node = ctrl_node("CTRL_CHR_DATA_0", 0x686B_E87E, 1, 0);
    let c = parse_ctrl_chara_node(&Node::new(&node)).expect("3 vars");
    assert_eq!(c.chara_param_id, HashId(0x686B_E87E));
    assert_eq!(c.chara_param_id.to_hex(), "0x686BE87E");
    assert_eq!(c.control_type, 1);
    assert_eq!(c.flags, 0);
}

#[test]
fn walk_real_samples_excludes_list_beg() {
    let children: Vec<Value> = SAMPLES
        .iter()
        .enumerate()
        .map(|(i, (id, ct, fl))| ctrl_node(&format!("CTRL_CHR_DATA_{i}"), *id, *ct, *fl))
        .collect();
    let root = json!({
        "entries": [
            { "name": "CTRL_CHR_DATA_LIST_BEG_0", "variables": [ivar(155)], "children": children }
        ]
    });
    let parsed = parse_all_ctrl_chara(&root);
    assert_eq!(parsed.len(), 5, "LIST_BEG exclu, 5 enfants parsés");
    for (p, (id, ct, fl)) in parsed.iter().zip(SAMPLES.iter()) {
        assert_eq!(p.chara_param_id, HashId(*id));
        assert_eq!(p.control_type, *ct);
        assert_eq!(p.flags, *fl);
    }

    // `is_controllable` (port de `isControllable`) : les ids présents le sont, un absent ne l'est pas.
    assert!(is_controllable(&parsed, HashId(0x686B_E87E)));
    assert!(is_controllable(&parsed, HashId(0x0C07_2D7A)));
    assert!(!is_controllable(&parsed, HashId(0xDEAD_BEEF)));
}

#[test]
fn fewer_than_three_vars_rejected() {
    let node = json!({
        "name": "CTRL_CHR_DATA_9",
        "variables": [ivar(0x1234_5678), ivar(1)], // 2 vars seulement
        "children": []
    });
    assert_eq!(parse_ctrl_chara_node(&Node::new(&node)), None);
}

#[test]
fn entry_is_copy() {
    let a = CtrlCharaData {
        chara_param_id: HashId(7),
        control_type: 2,
        flags: 0,
    };
    let b = a;
    assert_eq!(a, b);
}
