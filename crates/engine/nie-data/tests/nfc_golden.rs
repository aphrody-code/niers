#![allow(clippy::pedantic)]
//! Tests golden `nfc` — vraies valeurs du noeud `NFC_LOTTERY_INFO_LIST_BEG` tirées de :
//! `data/common/gamedata/nfc/nfc_lottery_config.cfg.bin` (T2B, 85568 octets), extrait du
//! VFS Steam IEVR via `nie-formats` (`cfgbin_to_t2b_iecode_root`).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/nfc-lottery-config.ts` (`parseEntries`).
//! Vérité terrain = sortie de l'extraction `nie-game/examples/extract_nfc` (loterie/table
//! représentative embarquée ; le fichier réel a 3 loteries × jusqu'à 34 tables × 15 items).
//!
//! ## Valeurs réelles embarquées (hex calculé = id & 0xFFFFFFFF)
//!
//! - Loterie 1 (lotteryId=1), TABLE_0 tableId=0xCBA1009F, 15 items (somme des poids = 100).
//! - Loterie 2 (lotteryId=2), TABLE_0 tableId=0x3319A8FD, 1 item (0xFC434F28, type=0, weight=100).
//! - Loterie 3 (lotteryId=3), TABLE_0 tableId=0xCBA1009F, 5 items (somme des poids = 100).

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::nfc::{NfcLotteryConfig, parse_nfc_lottery_config};
use serde_json::{Value, json};

// ─── Helpers de construction de la fixture (forme iecode `entries`) ──────────

fn int_var(v: i64) -> Value {
    json!({ "type": "Int", "value": v.to_string() })
}

fn item_node(idx: usize, item_id: i64, item_type: i64, weight: i64) -> Value {
    json!({
        "name": format!("NFC_LOTTERY_INFO_TABLE_ITEM_{idx}"),
        "variables": [int_var(item_id), int_var(item_type), int_var(weight)],
        "children": []
    })
}

/// Construit la paire BEG plate `[TABLE_n, ITEM_LIST_BEG_n]` pour une table.
fn table_pair(t_idx: usize, table_id: i64, items: &[(i64, i64, i64)]) -> Vec<Value> {
    let item_nodes: Vec<Value> = items
        .iter()
        .enumerate()
        .map(|(i, (id, ty, w))| item_node(i, *id, *ty, *w))
        .collect();
    vec![
        json!({
            "name": format!("NFC_LOTTERY_INFO_TABLE_{t_idx}"),
            "variables": [int_var(table_id)],
            "children": []
        }),
        json!({
            "name": format!("NFC_LOTTERY_INFO_TABLE_ITEM_LIST_BEG_{t_idx}"),
            "variables": [int_var(items.len() as i64)],
            "children": item_nodes
        }),
    ]
}

/// Construit la paire BEG plate `[LOTTERY_n, TABLE_LIST_BEG_n]` pour une loterie à 1 table.
fn lottery_pair(
    l_idx: usize,
    lottery_id: i64,
    table_id: i64,
    items: &[(i64, i64, i64)],
) -> Vec<Value> {
    vec![
        json!({
            "name": format!("NFC_LOTTERY_INFO_{l_idx}"),
            "variables": [int_var(lottery_id)],
            "children": []
        }),
        json!({
            "name": format!("NFC_LOTTERY_INFO_TABLE_LIST_BEG_{l_idx}"),
            "variables": [int_var(1)],
            "children": table_pair(0, table_id, items)
        }),
    ]
}

/// 15 items réels de la loterie 1, TABLE_0 (`tableId=0xCBA1009F`).
const LOTTERY1_TABLE0: &[(i64, i64, i64)] = &[
    (776289281, 0, 32),  // 0x2E453C01
    (90730434, 0, 8),    // 0x05686FC2
    (531441308, 0, 32),  // 0x1FAD269C
    (880833887, 0, 7),   // 0x3480755F
    (-754029353, 0, 2),  // 0xD30E6CD7
    (-131907820, 0, 2),  // 0xF8233F14
    (-516420011, 0, 2),  // 0xE1380E55
    (-1367762798, 0, 2), // 0xAE799892
    (-1218270765, 0, 2), // 0xB762A9D3
    (-1672480240, 0, 2), // 0x9C4FFA10
    (-2058040495, 0, 2), // 0x8554CB51
    (1806830514, 0, 2),  // 0x6BB20BB2
    (1084184689, 0, 2),  // 0x409F5871
    (1501849904, 0, 2),  // 0x59846930
    (-2009974859, 0, 1), // 0x883237B5
];

/// 1 item réel de la loterie 2, TABLE_0 (`tableId=0x3319A8FD`).
const LOTTERY2_TABLE0: &[(i64, i64, i64)] = &[
    (-62697688, 0, 100), // 0xFC434F28
];

/// 5 items réels de la loterie 3, TABLE_0 (`tableId=0xCBA1009F`).
const LOTTERY3_TABLE0: &[(i64, i64, i64)] = &[
    (880833887, 0, 20),   // 0x3480755F
    (765150238, 0, 10),   // 0x2D9B441E
    (477322883, 0, 20),   // 0x1C735E83
    (1395836996, 0, 10),  // 0x5332C844
    (-2009974859, 0, 40), // 0x883237B5
];

/// Fixture complète : 3 loteries (1 table représentative chacune), forme iecode `entries`.
fn fixture_nfc() -> Value {
    let mut children = Vec::new();
    children.extend(lottery_pair(0, 1, -878640993, LOTTERY1_TABLE0));
    children.extend(lottery_pair(1, 2, 857319677, LOTTERY2_TABLE0));
    children.extend(lottery_pair(2, 3, -878640993, LOTTERY3_TABLE0));
    json!({
        "entries": [{
            "name": "NFC_LOTTERY_INFO_LIST_BEG_0",
            "variables": [int_var(3)],
            "children": children
        }]
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[test]
fn nfc_trois_loteries() {
    let lotteries = parse_nfc_lottery_config(&fixture_nfc());
    assert_eq!(
        lotteries.len(),
        3,
        "3 loteries (NFC_LOTTERY_INFO_LIST_BEG var=3)"
    );
    assert_eq!(lotteries[0].lottery_id, 1, "lotteryId[0] = 1");
    assert_eq!(lotteries[1].lottery_id, 2, "lotteryId[1] = 2");
    assert_eq!(lotteries[2].lottery_id, 3, "lotteryId[2] = 3");
}

#[test]
fn nfc_loterie1_table0_tableid() {
    let lotteries = parse_nfc_lottery_config(&fixture_nfc());
    let l1 = &lotteries[0];
    assert_eq!(l1.tables.len(), 1, "fixture : 1 table représentative");
    // tableId -878640993 & 0xFFFFFFFF = 0xCBA1009F
    assert_eq!(
        l1.tables[0].table_id,
        HashId(0xCBA1009F),
        "tableId = 0xCBA1009F"
    );
    assert_eq!(l1.tables[0].table_id.to_hex(), "0xCBA1009F");
}

#[test]
fn nfc_loterie1_table0_15_items() {
    let lotteries = parse_nfc_lottery_config(&fixture_nfc());
    let table = &lotteries[0].tables[0];
    assert_eq!(table.items.len(), 15, "TABLE_0 de la loterie 1 = 15 items");

    // Premier item : 776289281 → 0x2E453C01, type=0, weight=32.
    assert_eq!(table.items[0].item_id, HashId(0x2E453C01));
    assert_eq!(table.items[0].item_id, HashId::from_i64(776289281));
    assert_eq!(table.items[0].item_type, 0);
    assert_eq!(table.items[0].weight, 32);

    // Quatrième item : 880833887 → 0x3480755F, weight=7.
    assert_eq!(table.items[3].item_id, HashId(0x3480755F));
    assert_eq!(table.items[3].weight, 7);

    // Dernier item : -2009974859 → 0x883237B5, weight=1.
    assert_eq!(table.items[14].item_id, HashId(0x883237B5));
    assert_eq!(table.items[14].item_id, HashId::from_i64(-2009974859));
    assert_eq!(table.items[14].weight, 1);
}

#[test]
fn nfc_loterie1_table0_poids_total_100() {
    let lotteries = parse_nfc_lottery_config(&fixture_nfc());
    // 32+8+32+7 + 2×10 + 1 = 100.
    assert_eq!(
        lotteries[0].tables[0].total_weight(),
        100,
        "somme des poids = 100"
    );
}

#[test]
fn nfc_loterie2_un_item_garanti() {
    let lotteries = parse_nfc_lottery_config(&fixture_nfc());
    let table = &lotteries[1].tables[0];
    // tableId 857319677 = 0x3319A8FD (positif).
    assert_eq!(table.table_id, HashId(0x3319A8FD));
    assert_eq!(table.items.len(), 1, "loterie 2 = 1 seul item");
    // itemId -62697688 → 0xFC434F28, weight=100 (garanti).
    assert_eq!(table.items[0].item_id, HashId(0xFC434F28));
    assert_eq!(table.items[0].weight, 100);
    assert_eq!(table.total_weight(), 100);
}

#[test]
fn nfc_loterie3_table0_cinq_items() {
    let lotteries = parse_nfc_lottery_config(&fixture_nfc());
    let table = &lotteries[2].tables[0];
    // Même tableId que la loterie 1 (0xCBA1009F) mais items/poids différents.
    assert_eq!(table.table_id, HashId(0xCBA1009F));
    assert_eq!(table.items.len(), 5);
    assert_eq!(table.items[0].item_id, HashId(0x3480755F));
    assert_eq!(table.items[0].weight, 20);
    assert_eq!(table.items[4].item_id, HashId(0x883237B5));
    assert_eq!(table.items[4].weight, 40);
    assert_eq!(table.total_weight(), 100);
}

#[test]
fn nfc_config_wrapper_et_recherche() {
    let cfg = NfcLotteryConfig::from_root(&fixture_nfc());
    assert_eq!(cfg.lotteries.len(), 3);

    // find_lottery : présent et absent.
    let l2 = cfg.find_lottery(2).expect("loterie 2 présente");
    assert_eq!(l2.lottery_id, 2);
    assert!(cfg.find_lottery(99).is_none(), "loterie 99 absente");

    // find_table sur la loterie 1.
    let l1 = cfg.find_lottery(1).unwrap();
    assert!(l1.find_table(HashId(0xCBA1009F)).is_some());
    assert!(l1.find_table(HashId(0xDEADBEEF)).is_none());
}

#[test]
fn nfc_list_beg_ignore_si_pas_de_lottery() {
    // Un LIST_BEG vide ne produit aucune loterie ; un root sans entries non plus.
    let vide = json!({ "entries": [] });
    assert!(parse_nfc_lottery_config(&vide).is_empty());
    let sans_entries = json!({ "lists": [] });
    assert!(parse_nfc_lottery_config(&sans_entries).is_empty());
}

// ─── Test sur fichier réel (skip silencieux si absent) ───────────────────────

const REAL_FILE: &str = "nfc/nfc_lottery_config.cfg.bin.json";

#[test]
fn nfc_real_file_structure() {
    if !std::path::Path::new(REAL_FILE).exists() {
        return; // fichier gitignore / absent du runner
    }
    let content = std::fs::read_to_string(REAL_FILE)
        .unwrap_or_else(|e| panic!("Impossible de lire {REAL_FILE}: {e}"));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    let lotteries = parse_nfc_lottery_config(&root);
    assert_eq!(lotteries.len(), 3, "dump réel : 3 loteries");
    assert_eq!(lotteries[0].tables.len(), 34, "loterie 1 : 34 tables");
    assert_eq!(lotteries[1].tables.len(), 1, "loterie 2 : 1 table");
}
