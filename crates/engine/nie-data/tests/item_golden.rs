#![allow(clippy::pedantic)]
//! Tests golden `item` — noeud réel `ITEM_SHOES_INFO_0` + `ITEM_CONSUME_INFO_0` tirés de :
//! `item/item_config_1.03.65.00.cfg.bin.json`.
//!
//! ITEM_SHOES_INFO_0 (19 vars) :
//! `[1834815904, 0, 1853054332, 0, 1401, 30, 31, 999, 0, 0, 0, "eq_sh110001", 1, 0, 0, 224, 0, 0, 961180446]`
//! → itemId=0x6D5D11A0, nameId=0x6E70FBBC, price=1401, stat1=30, stat2=31, internalCode=eq_sh110001.
//! ITEM_CONSUME_INFO_0 → itemId=0x5F0F1EAC, nameId=0xD069AE1F, internalCode=btl_re000001.

mod common;

use nie_data::hash::HashId;
use nie_data::item::{ItemCategory, ItemInfo, parse_all_items};
use serde_json::json;

fn item_fixture() -> serde_json::Value {
    let shoe_vars = [
        ("Int", "1834815904"),
        ("Int", "0"),
        ("Int", "1853054332"),
        ("Int", "0"),
        ("Int", "1401"),
        ("Int", "30"),
        ("Int", "31"),
        ("Int", "999"),
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "0"),
        ("String", "eq_sh110001"),
        ("Int", "1"),
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "224"),
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "961180446"),
    ];
    let consume_vars = [
        ("Int", "1594826412"), // 0x5F0F1EAC
        ("Int", "0"),
        ("Int", "-798380513"), // 0xD069AE1F
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "0"),
        ("String", "btl_re000001"),
    ];
    let to_vars = |arr: &[(&str, &str)]| -> Vec<serde_json::Value> {
        arr.iter()
            .map(|(t, v)| json!({ "type": t, "value": v }))
            .collect()
    };
    json!({
        "entries": [
            { "name": "ITEM_SHOES_INFO_0", "variables": to_vars(&shoe_vars), "children": [] },
            { "name": "ITEM_CONSUME_INFO_0", "variables": to_vars(&consume_vars), "children": [] }
        ]
    })
}

#[test]
fn item_shoes_info_0() {
    let items = parse_all_items(&item_fixture());
    let shoe: &ItemInfo = items
        .iter()
        .find(|i| i.category == ItemCategory::Shoes)
        .expect("chaussure présente");

    assert_eq!(shoe.item_id, HashId::from_signed(1834815904));
    assert_eq!(shoe.item_id.to_hex(), "0x6D5D11A0");
    assert_eq!(shoe.category, ItemCategory::Shoes);
    assert_eq!(shoe.category.as_str(), "shoes");
    assert_eq!(shoe.name_id, HashId::from_signed(1853054332));
    assert_eq!(shoe.name_id.to_hex(), "0x6E735D7C");
    assert_eq!(shoe.price, Some(1401));
    let stats = shoe.stats.expect("shoes a des stats");
    assert_eq!(stats.stat1, 30);
    assert_eq!(stats.stat2, 31);
    assert_eq!(shoe.internal_code.as_deref(), Some("eq_sh110001"));
    // Une chaussure n'est pas fashion → pas d'uniformId.
    assert_eq!(shoe.uniform_id, None);
}

#[test]
fn item_consume_info_0_pas_de_stats() {
    let items = parse_all_items(&item_fixture());
    let con = items
        .iter()
        .find(|i| i.category == ItemCategory::Consume)
        .expect("consommable présent");

    assert_eq!(con.item_id.to_hex(), "0x5F0F1EAC");
    assert_eq!(con.name_id.to_hex(), "0xD069AE1F");
    assert_eq!(con.internal_code.as_deref(), Some("btl_re000001"));
    // price = var4 = 0 → None (le TS n'ajoute le prix que si > 0).
    assert_eq!(con.price, None);
    // consume n'est pas une catégorie équipement → pas de stats.
    assert_eq!(con.stats, None);
    assert!(!con.category.has_equipment_stats());
}

#[test]
fn categorie_equipement_flag() {
    assert!(ItemCategory::Shoes.has_equipment_stats());
    assert!(ItemCategory::Misanga.has_equipment_stats());
    assert!(ItemCategory::Accessory.has_equipment_stats());
    assert!(ItemCategory::Special.has_equipment_stats());
    assert!(ItemCategory::Fashion.has_equipment_stats());
    assert!(!ItemCategory::Consume.has_equipment_stats());
    assert!(!ItemCategory::Emblem.has_equipment_stats());
}
