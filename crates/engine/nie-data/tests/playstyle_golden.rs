#![allow(clippy::pedantic)]
//! Tests golden `playstyle` — port 1:1 d'inagle `packages/inagle/src/parsers/playstyle.ts`.
//!
//! Vérité terrain = le vrai `chara_param_1.03.66.00.cfg.bin` (VFS IEVR), via
//! `cargo run -p nie-game --example extract_playstyle` :
//! - **6166** noeuds `CHARA_PARAM_INFO` valides, distribution `playStyle`
//!   `{0:1055, 1:1034, 2:1059, 3:1003, 4:989, 5:1026}` (couvre exactement `0..=5`) ;
//! - échantillon réel par `playStyle` (charaParamId = `values[0]`, hex u32) :
//!   `0→0xEB70DFF8, 1→0xEAB2B5CF, 2→0xA5F32308, 3→0xBCE81249, 4→0x09466C04, 5→0xC19FE60C`.
//!
//! Le noeud `CHARA_PARAM_INFO_1` (43 vars Int, `values[0]=-357386801=0xEAB2B5CF`,
//! `values[5]=1`) est recoupé indépendamment par `nie_data::chara_param` (doc-comment du module)
//! ET l'extraction live → ancre golden de confiance maximale.

use nie_data::cfgbin::Node;
use nie_data::hash::HashId;
use nie_data::playstyle::{
    PlaystyleEntry, parse_all_playstyles, parse_playstyle_node, playstyle_id_to_en,
    playstyle_id_to_fr,
};
use serde_json::{Value, json};

/// Variable `Int` typée (le dump stocke toujours la valeur en chaîne décimale).
fn ivar(v: i64) -> Value {
    json!({ "type": "Int", "value": v.to_string() })
}

/// Variable `String` typée.
fn svar(v: &str) -> Value {
    json!({ "type": "String", "value": v })
}

/// Noeud `CHARA_PARAM_INFO_<i>` à partir d'une liste d'entiers (tous `Int`).
fn int_node(name: &str, ints: &[i64]) -> Value {
    let vars: Vec<Value> = ints.iter().map(|i| ivar(*i)).collect();
    json!({ "name": name, "variables": vars, "children": [] })
}

/// Les 43 variables Int réelles de `CHARA_PARAM_INFO_1` (dump `chara_param_1.03.66.00`).
const CPI_1: [i64; 43] = [
    -357386801,
    1128709053,
    4,
    2,
    4,
    1,
    11,
    1,
    0,
    3,
    0,
    604761586,
    1,
    598373713,
    13,
    1591574804,
    20,
    724843777,
    30,
    1988803013,
    38,
    -1508771477,
    43,
    724843777,
    30,
    -1325922806,
    38,
    1210030151,
    43,
    2,
    0,
    -1,
    2,
    1,
    -2,
    -2,
    1,
    0,
    0,
    0,
    743008281,
    0,
    200626314,
];

#[test]
fn chara_param_info_1_real_node() {
    let node = int_node("CHARA_PARAM_INFO_1", &CPI_1);
    let e = parse_playstyle_node(&Node::new(&node)).expect("noeud valide (43 ints)");
    assert_eq!(e.chara_param_id, HashId(0xEAB2_B5CF));
    assert_eq!(e.chara_param_id.to_hex(), "0xEAB2B5CF");
    assert_eq!(e.play_style, 1);
    assert_eq!(e.label_en(), Some("Bond"));
    assert_eq!(e.label_fr(), Some("Lien"));
}

#[test]
fn all_six_real_samples_via_walk() {
    // Un échantillon réel par playStyle, sous un noeud de liste (qui DOIT être ignoré).
    let samples: [(u32, i64); 6] = [
        (0xEB70_DFF8, 0),
        (0xEAB2_B5CF, 1),
        (0xA5F3_2308, 2),
        (0xBCE8_1249, 3),
        (0x0946_6C04, 4),
        (0xC19F_E60C, 5),
    ];
    let children: Vec<Value> = samples
        .iter()
        .enumerate()
        .map(|(i, (id, ps))| {
            // [id, _, _, _, _, playStyle] : 6 ints, playStyle à l'index 5.
            int_node(
                &format!("CHARA_PARAM_INFO_{i}"),
                &[i64::from(*id), 0, 0, 0, 0, *ps],
            )
        })
        .collect();

    let root = json!({
        "entries": [
            { "name": "CHARA_PARAM_INFO_LIST_BEG_0", "variables": [ivar(6166)],
              "children": children }
        ]
    });

    let parsed = parse_all_playstyles(&root);
    assert_eq!(
        parsed.len(),
        6,
        "6 noeuds CHARA_PARAM_INFO (LIST_BEG exclu)"
    );

    let expected_en = [
        "Counter",
        "Bond",
        "Tension",
        "Rough Play",
        "Justice",
        "Freedom",
    ];
    let expected_fr = [
        "Contre",
        "Lien",
        "Tension",
        "Jeu violent",
        "Justice",
        "Liberté",
    ];
    for (e, (id, ps)) in parsed.iter().zip(samples.iter()) {
        assert_eq!(e.chara_param_id, HashId(*id));
        assert_eq!(e.play_style, *ps);
        assert_eq!(e.label_en(), Some(expected_en[*ps as usize]));
        assert_eq!(e.label_fr(), Some(expected_fr[*ps as usize]));
    }
}

#[test]
fn labels_cover_0_to_5_only() {
    assert_eq!(playstyle_id_to_en(0), Some("Counter"));
    assert_eq!(playstyle_id_to_en(5), Some("Freedom"));
    assert_eq!(playstyle_id_to_fr(3), Some("Jeu violent"));
    // Hors plage → None (les données réelles n'observent que 0..=5).
    assert_eq!(playstyle_id_to_en(6), None);
    assert_eq!(playstyle_id_to_en(-1), None);
    assert_eq!(playstyle_id_to_fr(99), None);
}

#[test]
fn fewer_than_six_ints_rejected() {
    let node = int_node("CHARA_PARAM_INFO_9", &[100, 1, 2, 3, 4]); // 5 ints
    assert_eq!(parse_playstyle_node(&Node::new(&node)), None);
}

#[test]
fn non_int_vars_are_skipped_in_indexing() {
    // Une `String` en position brute 1 décale l'indexation : `values` = [100,11,12,13,14,15],
    // donc `play_style = values[5] = 15` (PAS la var brute d'index 5 = 14). Réplique fidèle de
    // `parsePlaystyleNode` (collecte des seuls `Int`).
    let node = json!({
        "name": "CHARA_PARAM_INFO_42",
        "variables": [
            ivar(100), svar("skip_me"), ivar(11), ivar(12), ivar(13), ivar(14), ivar(15)
        ],
        "children": []
    });
    let e = parse_playstyle_node(&Node::new(&node)).expect("6 ints présents");
    assert_eq!(e.chara_param_id, HashId(100));
    assert_eq!(
        e.play_style, 15,
        "values[5] = 6ᵉ Int (15), pas la var brute #5 (14)"
    );
    assert_eq!(e.label_en(), None, "15 hors 0..=5");
}

#[test]
fn entry_is_copy_and_eq() {
    let a = PlaystyleEntry {
        chara_param_id: HashId(1),
        play_style: 2,
    };
    let b = a; // Copy
    assert_eq!(a, b);
    assert_eq!(b.label_en(), Some("Tension"));
}
