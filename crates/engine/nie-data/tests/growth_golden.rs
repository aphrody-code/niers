#![allow(clippy::pedantic)]
//! Tests golden `growth` — tables réelles tirées de :
//! `/home/ubuntu/niers/data/all-gamedata/growth_tables.json` (lv1=36, lv30=144, main=48, sub=48).
//!
//! Échantillons : Lv1[0] mainPos=2 sub=3 → Kc=13,Cr=14,Tc=12,Pr=10,Ps=10,Ag=9,It=11 ;
//! Lv30[0] charaRank=5 → Kc_30=106… ; Main[0] charaRank=5 → Kc_50=164,Kc_99=261.
//!
//! Golden StatBlock calculés via le VRAI `calculateStats` d'inagle (bun, mêmes tables) :
//! pour params {mainPos=2, sub=3, growthPattern=0, charaRank=5, playStyle=0} :
//!   Lv1  : Kc=13  Cr=14  Tc=12  Pr=10  Ps=10  Ag=9   It=11
//!   Lv15 : Kc=57  Cr=60  Tc=57  Pr=52  Ps=47  Ag=46  It=53
//!   Lv30 : Kc=106 Cr=110 Tc=106 Pr=98  Ps=87  Ag=87  It=99
//!   Lv40 : Kc=135 Cr=135 Tc=126 Pr=114 Ps=107 Ag=104 It=121
//!   Lv50 : Kc=164 Cr=160 Tc=146 Pr=130 Ps=127 Ag=122 It=144
//!   Lv75 : Kc=213 Cr=210 Tc=191 Pr=170 Ps=165 Ag=159 It=187
//!   Lv99 : Kc=261 Cr=258 Tc=235 Pr=210 Ps=202 Ag=195 It=230

use nie_data::growth::{
    CharacterGrowthParams, StatBlock, calculate_single_stat, calculate_stats, parse_growth_tables,
    rarity_to_growth_rank,
};
use serde_json::json;

/// Tables minimales (entrée pour mainPos=2/sub=3/rank=5) suffisantes au calcul Lv1→99.
fn tables_fixture() -> serde_json::Value {
    json!({
        "lv1": [
            { "mainPosition": 2, "subPosition": 3, "playStyle": 0,
              "Kc_1": 13, "Cr_1": 14, "Tc_1": 12, "Pr_1": 10, "Ps_1": 10, "Ag_1": 9, "It_1": 11 }
        ],
        "lv30": [
            { "mainPosition": 2, "subPosition": 3, "growthPattern": 0, "charaRank": 5,
              "Kc_30": 106, "Cr_30": 110, "Tc_30": 106, "Pr_30": 98, "Ps_30": 87, "Ag_30": 87, "It_30": 99 }
        ],
        "main": [
            { "mainPosition": 2, "growthPattern": 0, "charaRank": 5,
              "Kc_50": 164, "Cr_50": 160, "Tc_50": 146, "Pr_50": 130, "Ps_50": 127, "Ag_50": 122, "It_50": 144,
              "Kc_99": 261, "Cr_99": 258, "Tc_99": 235, "Pr_99": 210, "Ps_99": 202, "Ag_99": 195, "It_99": 230 }
        ],
        "sub": []
    })
}

fn params() -> CharacterGrowthParams {
    CharacterGrowthParams {
        main_position: 2,
        sub_position: 3,
        growth_pattern: 0,
        chara_rank: 5,
        play_style: 0,
    }
}

fn block(kc: i64, cr: i64, tc: i64, pr: i64, ps: i64, ag: i64, it: i64) -> StatBlock {
    StatBlock {
        kc,
        cr,
        tc,
        pr,
        ps,
        ag,
        it,
    }
}

#[test]
fn stat_block_par_palier_golden_inagle() {
    let t = parse_growth_tables(&tables_fixture());
    assert_eq!(t.lv1.len(), 1);
    assert_eq!(t.lv30.len(), 1);
    assert_eq!(t.main.len(), 1);
    let p = params();

    assert_eq!(calculate_stats(&t, &p, 1), block(13, 14, 12, 10, 10, 9, 11));
    assert_eq!(
        calculate_stats(&t, &p, 15),
        block(57, 60, 57, 52, 47, 46, 53)
    );
    assert_eq!(
        calculate_stats(&t, &p, 30),
        block(106, 110, 106, 98, 87, 87, 99)
    );
    assert_eq!(
        calculate_stats(&t, &p, 40),
        block(135, 135, 126, 114, 107, 104, 121)
    );
    assert_eq!(
        calculate_stats(&t, &p, 50),
        block(164, 160, 146, 130, 127, 122, 144)
    );
    assert_eq!(
        calculate_stats(&t, &p, 75),
        block(213, 210, 191, 170, 165, 159, 187)
    );
    assert_eq!(
        calculate_stats(&t, &p, 99),
        block(261, 258, 235, 210, 202, 195, 230)
    );
}

#[test]
fn total_power_lv99() {
    let t = parse_growth_tables(&tables_fixture());
    let s = calculate_stats(&t, &params(), 99);
    // 261+258+235+210+202+195+230 = 1591.
    assert_eq!(s.total(), 1591);
}

#[test]
fn single_stat_bornes_et_interpolation() {
    // Bornes exactes (golden trivial mais vérifie floor + paliers).
    assert_eq!(calculate_single_stat(0, 10, 30, 50, 100), 10);
    assert_eq!(calculate_single_stat(1, 10, 30, 50, 100), 10);
    assert_eq!(calculate_single_stat(30, 10, 30, 50, 100), 30);
    assert_eq!(calculate_single_stat(50, 10, 30, 50, 100), 50);
    assert_eq!(calculate_single_stat(99, 10, 30, 50, 100), 100);
    assert_eq!(calculate_single_stat(150, 10, 30, 50, 100), 100);
    // Lv15 sur la table réelle Kc : lerp(13, 106, 14/29) = floor(13 + 93*0.4827...) = 57.
    assert_eq!(calculate_single_stat(15, 13, 106, 164, 261), 57);
}

#[test]
fn rarity_to_growth_rank_golden() {
    assert_eq!(rarity_to_growth_rank(0), 0); // N
    assert_eq!(rarity_to_growth_rank(2), 2); // R
    assert_eq!(rarity_to_growth_rank(3), 3); // SR
    assert_eq!(rarity_to_growth_rank(4), 4); // SSR
    assert_eq!(rarity_to_growth_rank(5), 5); // UR
    assert_eq!(rarity_to_growth_rank(6), 5); // LR → UR
    assert_eq!(rarity_to_growth_rank(7), 5); // Legend → UR
    assert_eq!(rarity_to_growth_rank(20), 5); // BASARA → UR
    assert_eq!(rarity_to_growth_rank(1), 1); // défaut ≤5
    assert_eq!(rarity_to_growth_rank(99), 5); // défaut >5
}
