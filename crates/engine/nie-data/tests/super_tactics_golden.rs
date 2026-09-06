#![allow(clippy::pedantic)]
//! Tests golden `super_tactics` — noeuds réels extraits du VFS Steam :
//! `data/common/gamedata/skill/super_tactics_config_0.08.86.cfg.bin`
//! (jeu monté : `INAZUMA ELEVEN Victory Road/data`).
//!
//! Variante **base / pré-DLC**, port 1:1 d'inagle
//! `packages/inagle/src/parsers/super-tactics-base-config.ts` (`parseEntries`).
//! Toutes les valeurs ci-dessous sont les vraies variables Int du dump (dumpées via
//! l'exemple jetable `nie-game/examples/extract_super_tactics.rs`, depuis supprimé).

use nie_data::hash::HashId;
use nie_data::super_tactics::{NULL_SENTINEL, parse_super_tactics};
use serde_json::{Value, json};

/// Les 11 effets `SUPER_TACTICS_EFFECT_*` (9 Int chacun) — valeurs réelles du dump.
const EFFECTS: [[i64; 9]; 11] = [
    [
        672742448, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094, -992181094,
    ],
    [
        -1197071967,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        -1352230702,
        20,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        -1048930043,
        50,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        1097398782, 20, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        1483983039, 100, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        1595804838, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094, -992181094,
    ],
    [
        1097398782, 10, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -552833955, 10, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -811527881, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094, -992181094,
    ],
    [
        796194857, 100, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
];

/// Les 6 infos `SUPER_TACTICS_INFO_*` (6 Int chacune).
const INFOS: [[i64; 6]; 6] = [
    [1209890895, 1237356186, -1408544942, -580331975, 30, 0],
    [-787207691, -792076512, 889331432, 1124877210, 30, 2],
    [-1508697757, -1479610442, 1107635838, 873165580, 30, 1],
    [947358912, 967473685, -597574691, 1147242371, 60, 2],
    [1332788310, 1319979651, -1419343029, -1435507025, 10, 1],
    [-696652308, -677110983, 846191345, -637201888, 30, 2],
];

/// Les 6 liens `SUPER_TACTICS_INFO_REF_EFFECT_*` (`[offset, count]`).
const REF_EFFECTS: [[i64; 2]; 6] = [[0, 2], [2, 2], [4, 2], [6, 3], [9, 1], [10, 1]];

/// Les 6 `__SORT_INDEX_*` (1 Int).
const SORT_INDEX: [i64; 6] = [3, 0, 4, 2, 1, 5];

/// Construit une variable Int au format iecode `{type:"Int", value:"<n>"}`.
fn int_vars(vals: &[i64]) -> Vec<Value> {
    vals.iter()
        .map(|v| json!({ "type": "Int", "value": v.to_string() }))
        .collect()
}

/// Reconstruit le noeud racine iecode exact du dump (2 entries, enfants suffixés `_<i>`).
fn fixture() -> Value {
    // Enfants de la liste d'effets.
    let mut effect_children = Vec::new();
    for (i, e) in EFFECTS.iter().enumerate() {
        effect_children.push(json!({
            "name": format!("SUPER_TACTICS_EFFECT_{i}"),
            "variables": int_vars(e),
            "children": [],
        }));
    }

    // Enfants de la liste d'infos : ordre réel = (INFO_i, INFO_REF_EFFECT_i) entrelacés,
    // puis les 6 __SORT_INDEX_i.
    let mut info_children = Vec::new();
    for (i, (info, rf)) in INFOS.iter().zip(REF_EFFECTS.iter()).enumerate() {
        info_children.push(json!({
            "name": format!("SUPER_TACTICS_INFO_{i}"),
            "variables": int_vars(info),
            "children": [],
        }));
        info_children.push(json!({
            "name": format!("SUPER_TACTICS_INFO_REF_EFFECT_{i}"),
            "variables": int_vars(rf),
            "children": [],
        }));
    }
    for (i, s) in SORT_INDEX.iter().enumerate() {
        info_children.push(json!({
            "name": format!("__SORT_INDEX_{i}"),
            "variables": int_vars(&[*s]),
            "children": [],
        }));
    }

    json!({
        "entries": [
            {
                "name": "SUPER_TACTICS_EFFECT_LIST_BEG_0",
                "variables": int_vars(&[11]),
                "children": effect_children,
            },
            {
                "name": "SUPER_TACTICS_INFO_LIST_BEG_0",
                "variables": int_vars(&[6, 1]),
                "children": info_children,
            }
        ]
    })
}

#[test]
fn comptes_des_quatre_listes() {
    let cfg = parse_super_tactics(&fixture());
    assert_eq!(cfg.effects.len(), 11, "11 effets");
    assert_eq!(cfg.infos.len(), 6, "6 infos");
    assert_eq!(cfg.ref_effects.len(), 6, "6 ref_effects");
    assert_eq!(cfg.sort_index.len(), 6, "6 sort_index");
}

#[test]
fn effets_id_et_conditions() {
    let cfg = parse_super_tactics(&fixture());

    // Effet 0 : id 0x28193C30, aucune condition (tout sentinelle).
    assert_eq!(cfg.effects[0].effect_id, HashId::from_signed(672742448));
    assert_eq!(cfg.effects[0].effect_id.to_hex(), "0x28193C30");
    assert!(cfg.effects[0].conditions.is_empty());

    // Effet 2 : id 0xAF6698D2, une condition 0x00000014 (=20).
    assert_eq!(cfg.effects[2].effect_id, HashId::from_signed(-1352230702));
    assert_eq!(cfg.effects[2].effect_id.to_hex(), "0xAF6698D2");
    assert_eq!(cfg.effects[2].conditions, vec![HashId(20)]);
    assert_eq!(cfg.effects[2].conditions[0].to_hex(), "0x00000014");

    // Effet 5 : condition 0x00000064 (=100).
    assert_eq!(cfg.effects[5].conditions, vec![HashId(100)]);

    // Doublon réel : effets 4 et 7 partagent le même effect_id 0x4168F9FE.
    assert_eq!(cfg.effects[4].effect_id, HashId(0x4168_F9FE));
    assert_eq!(cfg.effects[7].effect_id, HashId(0x4168_F9FE));
    // ... mais des conditions différentes (20 vs 10).
    assert_eq!(cfg.effects[4].conditions, vec![HashId(20)]);
    assert_eq!(cfg.effects[7].conditions, vec![HashId(10)]);
}

#[test]
fn sentinelle_null_filtree() {
    // Garde anti-régression : -992181094 (0xC4DC849A) ne doit jamais apparaître en condition.
    assert_eq!(NULL_SENTINEL, -992_181_094);
    let cfg = parse_super_tactics(&fixture());
    for effect in &cfg.effects {
        for cond in &effect.conditions {
            assert_ne!(
                cond.get(),
                0xC4DC_849A,
                "la sentinelle ne doit pas être conservée comme condition"
            );
        }
    }
    // Les effets 0,1,6,9 n'ont que des sentinelles → 0 condition.
    for &i in &[0usize, 1, 6, 9] {
        assert!(
            cfg.effects[i].conditions.is_empty(),
            "effet {i} sans condition"
        );
    }
}

#[test]
fn infos_champs_de_base() {
    let cfg = parse_super_tactics(&fixture());

    // Info 0.
    assert_eq!(cfg.infos[0].tactics_id, HashId::from_signed(1209890895));
    assert_eq!(cfg.infos[0].tactics_id.to_hex(), "0x481D784F");
    assert_eq!(
        cfg.infos[0].extra_hashes,
        [
            HashId::from_signed(1237356186),  // 0x49C08E9A
            HashId::from_signed(-1408544942), // 0xAC0B4F52
            HashId::from_signed(-580331975),  // 0xDD68D639
        ]
    );
    assert_eq!(cfg.infos[0].value, 30);
    assert_eq!(cfg.infos[0].kind, 0);

    // Info 3 : value=60 (la plus chère du dump).
    assert_eq!(cfg.infos[3].tactics_id.to_hex(), "0x38778CC0");
    assert_eq!(cfg.infos[3].value, 60);
    assert_eq!(cfg.infos[3].kind, 2);
}

#[test]
fn ref_effects_chainage_couvre_tous_les_effets() {
    let cfg = parse_super_tactics(&fixture());

    // Les couples [offset, count] se chaînent et couvrent exactement les 11 effets.
    let expected = [(0, 2), (2, 2), (4, 2), (6, 3), (9, 1), (10, 1)];
    let mut cursor = 0;
    for (i, (off, cnt)) in expected.iter().enumerate() {
        assert_eq!(cfg.ref_effects[i].effect_offset, *off, "offset {i}");
        assert_eq!(cfg.ref_effects[i].effect_count, *cnt, "count {i}");
        assert_eq!(cfg.ref_effects[i].effect_offset, cursor, "chaînage {i}");
        cursor = cfg.ref_effects[i].effect_offset + cfg.ref_effects[i].effect_count;
    }
    assert_eq!(cursor, 11, "le chaînage couvre exactement les 11 effets");
}

#[test]
fn effects_of_resout_la_tranche() {
    let cfg = parse_super_tactics(&fixture());

    // Info 0 → effets [0,2[ : 0x28193C30 puis 0xB8A621A1.
    let e0 = cfg.effects_of(0);
    assert_eq!(e0.len(), 2);
    assert_eq!(e0[0].effect_id.to_hex(), "0x28193C30");
    assert_eq!(e0[1].effect_id.to_hex(), "0xB8A621A1");

    // Info 3 → effets [6,9[ : 3 effets.
    assert_eq!(cfg.effects_of(3).len(), 3);
    assert_eq!(
        cfg.effects_of(3)[0].effect_id,
        HashId::from_signed(1595804838)
    );

    // Info 5 → effets [10,11[ : 0x2F74F829 (condition 100).
    let e5 = cfg.effects_of(5);
    assert_eq!(e5.len(), 1);
    assert_eq!(e5[0].effect_id.to_hex(), "0x2F74F829");
    assert_eq!(e5[0].conditions, vec![HashId(100)]);

    // Index hors borne → tranche vide.
    assert!(cfg.effects_of(6).is_empty());
}

#[test]
fn sort_index_valeurs_reelles() {
    let cfg = parse_super_tactics(&fixture());
    assert_eq!(cfg.sort_index, vec![3, 0, 4, 2, 1, 5]);
    // Permutation des 6 indices d'infos (chaque index 0..=5 présent une fois).
    let mut sorted = cfg.sort_index.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn find_info_par_id() {
    let cfg = parse_super_tactics(&fixture());
    let found = cfg.find_info(HashId(0x38778CC0)).expect("info 0x38778CC0");
    assert_eq!(found.value, 60);
    assert!(cfg.find_info(HashId(0xDEAD_BEEF)).is_none());
}
