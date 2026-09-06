#![allow(clippy::pedantic)]
//! Tests golden `exp` — listes réelles tirées de :
//! `character/chara_exp_table_config_0.00.00.00.cfg.bin.json`.
//!
//! m_charaExpTableList = 100 entrées (level 1→100), m_expRarityRateList = 9 entrées.
//! level 1 → needExp 124, level 2 → 130, level 3 → 146 ; cumul lvl5=565, lvl10=1973.
//! rarityRates : 0-3 → ×1, 4-8 → ×3.

mod common;

use nie_data::exp::{CharaExpEntry, ExpRarityRate, parse_exp_table};
use serde_json::json;

/// Fixture avec les 10 premiers niveaux réels (suffit pour les cumuls golden lvl5/lvl10).
fn exp_fixture() -> serde_json::Value {
    // Valeurs réelles needExp niveaux 1..10 (dump chara_exp_table_config_0.00.00.00).
    let real: [(i64, i64); 10] = [
        (1, 124),
        (2, 130),
        (3, 146),
        (4, 165),
        (5, 196),
        (6, 230),
        (7, 275),
        (8, 323),
        (9, 384),
        (10, 449),
    ];
    let exp_list: Vec<serde_json::Value> = real
        .iter()
        .map(|(lvl, need)| json!({ "level": lvl, "needExp": need }))
        .collect();
    let rates: Vec<serde_json::Value> = (0..=8)
        .map(|r| json!({ "rarity": r, "rate": if r <= 3 { 1 } else { 3 } }))
        .collect();
    json!({
        "version": 100,
        "lists": [
            { "name": "m_charaExpTableList", "typeName": "CHARA_EXP_TABLE", "values": exp_list },
            { "name": "m_expRarityRateList", "typeName": "EXP_RARITY_RATE", "values": rates }
        ]
    })
}

#[test]
fn exp_entries_premiers_niveaux() {
    let t = parse_exp_table(&exp_fixture());
    assert_eq!(t.exp_table.len(), 10);
    assert_eq!(
        t.exp_table[0],
        CharaExpEntry {
            level: 1,
            need_exp: 124
        }
    );
    assert_eq!(
        t.exp_table[1],
        CharaExpEntry {
            level: 2,
            need_exp: 130
        }
    );
    assert_eq!(
        t.exp_table[2],
        CharaExpEntry {
            level: 3,
            need_exp: 146
        }
    );
    assert_eq!(t.exp_for_level(1), Some(124));
    assert_eq!(t.exp_for_level(3), Some(146));
    assert_eq!(t.exp_for_level(999), None);
}

#[test]
fn rarity_rates_golden() {
    let t = parse_exp_table(&exp_fixture());
    assert_eq!(t.rarity_rates.len(), 9);
    assert_eq!(t.rarity_rates[0], ExpRarityRate { rarity: 0, rate: 1 });
    assert_eq!(t.rarity_rates[4], ExpRarityRate { rarity: 4, rate: 3 });
    // growthRank 0-3 → ×1, 4-8 → ×3.
    for r in 0..=3 {
        assert_eq!(t.rarity_rate(r), 1, "rank {r} → ×1");
    }
    for r in 4..=8 {
        assert_eq!(t.rarity_rate(r), 3, "rank {r} → ×3");
    }
    // rang absent → défaut 1.
    assert_eq!(t.rarity_rate(42), 1);
}

#[test]
fn cumulative_exp_golden() {
    let t = parse_exp_table(&exp_fixture());
    // Cumul jusqu'au niveau 5 = somme needExp des niveaux < 5 = 124+130+146+165 = 565.
    assert_eq!(t.cumulative_exp(5), 565);
    // Cumul jusqu'au niveau 10 = somme niveaux 1..9 = 124+130+146+165+196+230+275+323+384 = 1973.
    // (le niveau 10 lui-même n'est PAS inclus — getCumulativeExp s'arrête à level >= target.)
    assert_eq!(t.cumulative_exp(10), 1973);
    // Cumul niveau 1 = 0 (aucun niveau < 1).
    assert_eq!(t.cumulative_exp(1), 0);
}
