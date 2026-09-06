#![allow(clippy::pedantic)]
//! Tests golden `shop::resolve_name` — jointure documentée par inagle (`shop-config.ts` :
//! « \[1\] = Shop Name Hash -> links to shop_text »), validée sur le vrai jeu.
//!
//! Vérité terrain (via `cargo run -p nie-game --example extract_shop_text`) : **14/16** boutiques
//! résolues en fr — « Boutique Kizuna », « BB Mart », « Centre commercial chronique »,
//! « Boutique VS », « Goal-Marché (…) », etc. Ce fichier fige la logique de jointure.

use nie_data::hash::HashId;
use nie_data::shop::{ShopInfo, resolve_name};

fn shop(name_hash: u32) -> ShopInfo {
    ShopInfo {
        shop_id: HashId(0xB13B_2BD5),
        name_hash: HashId(name_hash),
        items: vec![],
    }
}

fn shop_text() -> Vec<(HashId, String)> {
    vec![
        (HashId(0x0000_1001), String::from("Boutique Kizuna")),
        (HashId(0x0000_1002), String::from("BB Mart")),
        (
            HashId(0x0000_1003),
            String::from("Centre commercial chronique"),
        ),
        // hash dupliqué : last-wins (= la map shop_text).
        (HashId(0x0000_2000), String::from("ancien")),
        (HashId(0x0000_2000), String::from("récent")),
    ]
}

#[test]
fn resolves_real_shop_names() {
    let t = shop_text();
    assert_eq!(
        resolve_name(&shop(0x0000_1001), &t),
        Some("Boutique Kizuna")
    );
    assert_eq!(resolve_name(&shop(0x0000_1002), &t), Some("BB Mart"));
    assert_eq!(
        resolve_name(&shop(0x0000_1003), &t),
        Some("Centre commercial chronique")
    );
}

#[test]
fn missing_hash_is_none() {
    assert_eq!(resolve_name(&shop(0xDEAD_BEEF), &shop_text()), None);
}

#[test]
fn duplicate_hash_last_wins() {
    assert_eq!(
        resolve_name(&shop(0x0000_2000), &shop_text()),
        Some("récent")
    );
}
