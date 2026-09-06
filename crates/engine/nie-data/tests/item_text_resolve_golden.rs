#![allow(clippy::pedantic)]
//! Tests golden `item::resolve_name`/`resolve_description` — port de la jointure texte de
//! `buildItemDatabase` (`item-config.ts`). Nom ET description sont résolus depuis la **même** table
//! `item_text` (via `nameId`/`descId`).
//!
//! Vérité terrain (via `cargo run -p nie-game --example extract_item_text`) : sur le vrai
//! `item_config` + `item_text` fr, **1767 noms / 324 descriptions** résolus (ex. « Guts Gear »,
//! « Barre protéinée »). Ce fichier fige la logique de jointure sur fixtures contrôlées.

use nie_data::hash::HashId;
use nie_data::item::{ItemCategory, ItemInfo, resolve_description, resolve_name};

fn item(name_id: u32, desc_id: u32) -> ItemInfo {
    ItemInfo {
        item_id: HashId(0xABCD),
        category: ItemCategory::Consume,
        name_id: HashId(name_id),
        desc_id: HashId(desc_id),
        price: None,
        stats: None,
        internal_code: None,
        uniform_id: None,
    }
}

fn item_text() -> Vec<(HashId, String)> {
    vec![
        (HashId(0x6E73_5D7C), String::from("Guts Gear")),
        (
            HashId(0x1111_2222),
            String::from("Boisson en gelée au goût rafraîchissant."),
        ),
        // last-wins sur hash dupliqué.
        (HashId(0x0000_0001), String::from("vieux nom")),
        (HashId(0x0000_0001), String::from("nom courant")),
    ]
}

#[test]
fn resolves_name_and_description() {
    let t = item_text();
    let it = item(0x6E73_5D7C, 0x1111_2222);
    assert_eq!(resolve_name(&it, &t), Some("Guts Gear"));
    assert_eq!(
        resolve_description(&it, &t),
        Some("Boisson en gelée au goût rafraîchissant.")
    );
}

#[test]
fn missing_ids_are_none() {
    let t = item_text();
    let it = item(0xDEAD_BEEF, 0xFEED_FACE);
    assert_eq!(resolve_name(&it, &t), None);
    assert_eq!(resolve_description(&it, &t), None);
}

#[test]
fn shared_table_last_wins() {
    let t = item_text();
    // Nom et description peuvent pointer la même table ; le hash dupliqué garde la dernière valeur.
    let it = item(0x0000_0001, 0x0000_0001);
    assert_eq!(resolve_name(&it, &t), Some("nom courant"));
    assert_eq!(resolve_description(&it, &t), Some("nom courant"));
}
