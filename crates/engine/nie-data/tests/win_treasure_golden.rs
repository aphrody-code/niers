#![allow(clippy::pedantic)]
//! Tests golden `win_treasure` — valeurs réelles tirées du VFS IEVR :
//! `data/common/gamedata/item/win_treasure_lot_table_config_0.00.00.cfg.bin`.
//!
//! Le fichier est au format T2B (`entries`). Deux racines : `ITBL_ITEMS_LIST_BEG`
//! (113 feuilles `[itemId, 1, poids, 0, 0]`) et `ITBL_BASE_LIST_BEG` (44 enfants en
//! 22 paires `ITBL_BASE` / `ITBL_BASE_REF_ITEMS [offset, count]`). Port 1:1 d'inagle
//! `packages/inagle/src/parsers/drop-rates.ts` (`loadWinTreasureRates`, l.162-217).
//!
//! Les `const` ci-dessous sont les octets réels extraits du dump (itemId/coffre en
//! `Int` **signé** ; on attend la réinterprétation `>>> 0` → hex non signé). Les
//! assertions golden citent les valeurs hex/poids attendues, indépendantes du fixtures.

use nie_data::hash::HashId;
use nie_data::win_treasure::{collect_chests, flatten_items, parse_win_treasure};
use serde_json::json;

/// Feuilles `ITBL_ITEMS_*` réelles : `(itemId signé brut, poids)`. 113 entrées, ordre du fichier.
/// Source : `win_treasure_lot_table_config_0.00.00.cfg.bin`, racine `ITBL_ITEMS_LIST_BEG`.
const ITEMS_RAW: &[(i64, i64)] = &[
    (-1198613823, 10),
    (-813061545, 10),
    (-917760558, 5),
    (1346716776, 5),
    (658666750, 5),
    (-1188634275, 5),
    (-836644405, 5),
    (1462272113, 5),
    (539996391, 5),
    (-924553650, 10),
    (562547579, 10),
    (-917760558, 5),
    (1346716776, 5),
    (658666750, 5),
    (-1188634275, 5),
    (-836644405, 5),
    (1462272113, 5),
    (539996391, 5),
    (1451293677, 10),
    (-1075618088, 10),
    (652873570, 10),
    (-121077251, 10),
    (-2002619022, 10),
    (1722553438, 10),
    (296428744, 10),
    (-1882214037, 10),
    (382132433, 10),
    (382132433, 10),
    (-2002619022, 10),
    (-121077251, 5),
    (-1882214037, 5),
    (296428744, 2),
    (-2002619022, 10),
    (-1882214037, 10),
    (-121077251, 15),
    (296428744, 10),
    (382132433, 5),
    (1722553438, 10),
    (-2002619022, 10),
    (-1882214037, 10),
    (-121077251, 10),
    (296428744, 10),
    (382132433, 10),
    (1722553438, 10),
    (-2002619022, 10),
    (-1882214037, 5),
    (-121077251, 5),
    (296428744, 15),
    (382132433, 20),
    (1722553438, 5),
    (382132433, 10),
    (-1882214037, 10),
    (-2002619022, 10),
    (-121077251, 10),
    (296428744, 10),
    (1722553438, 10),
    (382132433, 10),
    (-1882214037, 10),
    (-2002619022, 10),
    (-121077251, 10),
    (296428744, 10),
    (1722553438, 10),
    (-917760558, 1),
    (1346716776, 1),
    (658666750, 1),
    (-1188634275, 1),
    (-836644405, 1),
    (1462272113, 1),
    (539996391, 1),
    (-917760558, 1),
    (1346716776, 1),
    (658666750, 1),
    (-1188634275, 1),
    (-836644405, 1),
    (1462272113, 1),
    (539996391, 1),
    (-917760558, 36),
    (1346716776, 36),
    (658666750, 36),
    (-1188634275, 36),
    (-836644405, 36),
    (1462272113, 36),
    (539996391, 36),
    (1659038251, 36),
    (-68437103, 36),
    (-1930654969, 10),
    (310910628, 36),
    (1703882290, 36),
    (-58294392, 10),
    (-1954459874, 36),
    (-789278876, 25),
    (1241374430, 25),
    (1056632392, 25),
    (2055323580, 25),
    (-917760558, 5),
    (1346716776, 5),
    (658666750, 5),
    (-1188634275, 5),
    (-836644405, 5),
    (1462272113, 5),
    (539996391, 5),
    (-477565434, 10),
    (-1802510704, 10),
    (704832591, 33),
    (-1291078155, 33),
    (-1005812381, 33),
    (-917760558, 5),
    (1346716776, 5),
    (658666750, 5),
    (-1188634275, 5),
    (-836644405, 5),
    (1462272113, 5),
    (539996391, 5),
];

/// Coffres réels : `(coffre signé brut, offset, count)`. 22 entrées (44 enfants appariés).
/// Source : même fichier, racine `ITBL_BASE_LIST_BEG` (paires `ITBL_BASE` / `_REF_ITEMS`).
const BASE_RAW: &[(i64, i64, i64)] = &[
    (-252444957, 0, 9),
    (1778036569, 9, 9),
    (-396263870, 18, 1),
    (1902653432, 19, 1),
    (2051558742, 20, 1),
    (1942466321, 21, 3),
    (1753284127, 24, 2),
    (718908520, 26, 1),
    (-2110947488, 27, 5),
    (-485980275, 32, 6),
    (2046940727, 38, 6),
    (218556065, 44, 6),
    (1614258446, 50, 6),
    (2032000109, 56, 3),
    (-535523881, 59, 3),
    (-661154020, 62, 7),
    (-1793821100, 69, 7),
    (-548563077, 76, 14),
    (914591195, 90, 11),
    (-1349854111, 101, 1),
    (-661787401, 102, 1),
    (-491351371, 103, 10),
];

/// Reconstruit le noeud `cfg.bin.json` (forme iecode T2B) à partir des octets réels.
fn node_fixture() -> serde_json::Value {
    let item_children: Vec<_> = ITEMS_RAW
        .iter()
        .enumerate()
        .map(|(i, (id, w))| {
            json!({
                "name": format!("ITBL_ITEMS_{i}"),
                "variables": [
                    {"type": "Int", "value": id.to_string()},
                    {"type": "Int", "value": "1"},
                    {"type": "Int", "value": w.to_string()},
                    {"type": "Int", "value": "0"},
                    {"type": "Int", "value": "0"},
                ],
                "children": []
            })
        })
        .collect();

    let mut base_children = Vec::new();
    for (i, (coffre, off, cnt)) in BASE_RAW.iter().enumerate() {
        base_children.push(json!({
            "name": format!("ITBL_BASE_{i}"),
            "variables": [{"type": "Int", "value": coffre.to_string()}],
            "children": []
        }));
        base_children.push(json!({
            "name": format!("ITBL_BASE_REF_ITEMS_{i}"),
            "variables": [
                {"type": "Int", "value": off.to_string()},
                {"type": "Int", "value": cnt.to_string()},
            ],
            "children": []
        }));
    }

    json!({
        "entries": [
            {
                "name": "ITBL_ITEMS_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "113"}],
                "children": item_children
            },
            {
                "name": "ITBL_BASE_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "44"}],
                "children": base_children
            }
        ]
    })
}

#[test]
fn aplatit_les_113_feuilles_items() {
    let flat = flatten_items(&node_fixture());
    assert_eq!(flat.len(), 113, "113 feuilles ITBL_ITEMS aplaties");

    // [0] -1198613823 (signé) → 0xB88E9AC1, poids 10.
    assert_eq!(flat[0].item_id, HashId::from_signed(-1198613823));
    assert_eq!(flat[0].item_id, HashId(0xB88E_9AC1));
    assert_eq!(flat[0].item_id.to_hex(), "0xB88E9AC1");
    assert_eq!(flat[0].weight, 10);

    // [2] 0xC94C15D2 poids 5 (item récurrent des coffres).
    assert_eq!(flat[2].item_id.to_hex(), "0xC94C15D2");
    assert_eq!(flat[2].weight, 5);

    // [112] dernière feuille : 539996391 → 0x202FB0E7, poids 5.
    assert_eq!(flat[112].item_id, HashId(0x202F_B0E7));
    assert_eq!(flat[112].weight, 5);
}

#[test]
fn apparie_les_22_coffres() {
    let chests = collect_chests(&node_fixture());
    assert_eq!(
        chests.len(),
        22,
        "22 coffres ITBL_BASE appariés à leur _REF_ITEMS"
    );

    // 1er coffre : -252444957 → 0xF0F3FEE3, tranche [0, 9).
    assert_eq!(chests[0].chest_id, HashId::from_signed(-252444957));
    assert_eq!(chests[0].chest_id.to_hex(), "0xF0F3FEE3");
    assert_eq!(chests[0].offset, 0);
    assert_eq!(chests[0].count, 9);

    // 2e coffre : 1778036569 → 0x69FAAF59, tranche [9, 9).
    assert_eq!(chests[1].chest_id.to_hex(), "0x69FAAF59");
    assert_eq!((chests[1].offset, chests[1].count), (9, 9));

    // Dernier coffre : -491351371 → 0xE2B692B5, tranche [103, 10).
    assert_eq!(chests[21].chest_id.to_hex(), "0xE2B692B5");
    assert_eq!((chests[21].offset, chests[21].count), (103, 10));

    // Les tranches couvrent exactement la liste plate (somme des count = 113).
    let total: i64 = chests.iter().map(|c| c.count).sum();
    assert_eq!(total, 113);
}

#[test]
fn parse_complet_lignes_golden() {
    let rows = parse_win_treasure(&node_fixture());
    assert_eq!(
        rows.len(),
        113,
        "113 lignes (une par item, chaque item référencé une fois)"
    );

    // Ordinaux contigus 0..113.
    for (i, r) in rows.iter().enumerate() {
        assert_eq!(r.ordinal, i, "ordinal contigu");
    }

    // Les 9 premières lignes = coffre 0xF0F3FEE3 × flat[0..9]. Vérité terrain du dump
    // (drop-rates.ts EXPECTED ROWS), coffre/item/poids hardcodés indépendamment du fixtures.
    let expected_chest0: [(&str, &str, i64); 9] = [
        ("0xF0F3FEE3", "0xB88E9AC1", 10),
        ("0xF0F3FEE3", "0xCF89AA57", 10),
        ("0xF0F3FEE3", "0xC94C15D2", 5),
        ("0xF0F3FEE3", "0x50454468", 5),
        ("0xF0F3FEE3", "0x274274FE", 5),
        ("0xF0F3FEE3", "0xB926E15D", 5),
        ("0xF0F3FEE3", "0xCE21D1CB", 5),
        ("0xF0F3FEE3", "0x57288071", 5),
        ("0xF0F3FEE3", "0x202FB0E7", 5),
    ];
    for (i, (chest, item, w)) in expected_chest0.iter().enumerate() {
        assert_eq!(rows[i].chest_id.to_hex(), *chest, "ligne {i} coffre");
        assert_eq!(rows[i].item_id.to_hex(), *item, "ligne {i} item");
        assert_eq!(rows[i].weight, *w, "ligne {i} poids");
    }

    // Ligne 9 = 1re du 2e coffre 0x69FAAF59 → flat[9] = -924553650 → 0xC8E46E4E, poids 10.
    assert_eq!(rows[9].chest_id.to_hex(), "0x69FAAF59");
    assert_eq!(rows[9].item_id.to_hex(), "0xC8E46E4E");
    assert_eq!(rows[9].weight, 10);

    // Dernière ligne (112) = dernier coffre 0xE2B692B5 → flat[112] → 0x202FB0E7, poids 5.
    assert_eq!(rows[112].chest_id.to_hex(), "0xE2B692B5");
    assert_eq!(rows[112].item_id.to_hex(), "0x202FB0E7");
    assert_eq!(rows[112].weight, 5);
}

#[test]
fn poids_unique_eleve_item_0x16c6e0d1() {
    // Garde anti-régression d'un octet remarquable : flat[48] = 382132433 (0x16C6E0D1)
    // porte le poids le plus élevé (20) ; il appartient au coffre couvrant l'offset 48.
    let flat = flatten_items(&node_fixture());
    assert_eq!(flat[48].item_id.to_hex(), "0x16C6E0D1");
    assert_eq!(flat[48].weight, 20);
}
