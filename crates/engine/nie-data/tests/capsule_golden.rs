#![allow(clippy::pedantic)]
//! Tests golden `capsule` — noeuds réels du `capsule_config_0.00.00.cfg.bin`, tirés de :
//! `data/common/gamedata/capsule/capsule_config_0.00.00.cfg.bin` (VFS Steam),
//! décodé T2B par `nie-formats` (`cfgbin::cfgbin_parse`).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/capsule-config.ts` (`parseEntries` l.68-136).
//! Vérité terrain : valeurs extraites du vrai dump par un exemple jetable, non versionné.
//! La fixture reproduit la forme iecode/T2B **frère** (en-tête `*_INFO` suivi de son
//! `*_DATA_LIST_BEG`) que produit `cfgbin_to_t2b_iecode_root`, et qui est appariée par le parseur.

use nie_data::capsule::{RankRate, WeaponColorRate, parse_capsule_database};
use nie_data::hash::HashId;
use serde_json::{Value, json};

/// Construit une variable Int au format dump (`{type:"Int", value:"<n>"}`).
fn iv(n: i64) -> Value {
    json!({ "type": "Int", "value": n.to_string() })
}

/// Construit un noeud `{name, variables, children}`.
fn node(name: &str, vars: &[i64], children: Vec<Value>) -> Value {
    let variables: Vec<Value> = vars.iter().map(|v| iv(*v)).collect();
    json!({ "name": name, "variables": variables, "children": children })
}

/// Fixture = vrais noeuds du dump, en forme T2B frère (en-tête + `*_DATA_LIST_BEG` frère).
fn fixture() -> Value {
    // --- WEAPON COLOR TABLE 0 : id 401513875, 6 lignes {colorType, weight=17}. ---
    let weapon_rows: Vec<Value> = [(1, 17), (2, 17), (3, 17), (4, 17), (5, 17), (6, 17)]
        .iter()
        .map(|(c, w)| node("CPSL_LOT_WEAPON_COLOR_TABLE_INFO_DATA", &[*c, *w], vec![]))
        .collect();
    let weapon_list = node(
        "CPSL_LOT_WEAPON_COLOR_TABLE_INFO_LIST_BEG",
        &[],
        vec![
            node("CPSL_LOT_WEAPON_COLOR_TABLE_INFO", &[401_513_875], vec![]),
            node(
                "CPSL_LOT_WEAPON_COLOR_TABLE_INFO_DATA_LIST_BEG",
                &[6],
                weapon_rows,
            ),
        ],
    );

    // --- PRIZE INFO : 4 premiers prix réels (vars plates, 6 Int). ---
    let prize_list = node(
        "CPSL_PRIZE_INFO_LIST_BEG",
        &[],
        vec![
            node(
                "CPSL_PRIZE_INFO",
                &[240_076_273, -754_029_353, 0, 0, 0, 0],
                vec![],
            ),
            node(
                "CPSL_PRIZE_INFO",
                &[627_185_202, -131_907_820, 0, 0, 0, 0],
                vec![],
            ),
            node(
                "CPSL_PRIZE_INFO",
                &[1_014_572_915, -516_420_011, 0, 0, 0, 0],
                vec![],
            ),
            node(
                "CPSL_PRIZE_INFO",
                &[1_933_095_348, -1_367_762_798, 0, 0, 0, 0],
                vec![],
            ),
        ],
    );

    // --- PRIZE TABLE 0 : id -164431687, 3 lignes DATA représentatives (sur 445 réelles). ---
    let table_rows: Vec<Value> = [
        [1_517_288_567, 1, 0, 1, 0, 0, 0, 0, 0, 0],
        [1_900_195_764, 1, 0, 1, 0, 0, 0, 0, 0, 0],
        [1_804_068_586, 1, 0, 1, 0, 0, 0, 0, 0, 0],
    ]
    .iter()
    .map(|r| node("CPSL_PRIZE_TABLE_INFO_DATA", r, vec![]))
    .collect();
    let prize_table_list = node(
        "CPSL_PRIZE_TABLE_INFO_LIST_BEG",
        &[],
        vec![
            node("CPSL_PRIZE_TABLE_INFO", &[-164_431_687], vec![]),
            node("CPSL_PRIZE_TABLE_INFO_DATA_LIST_BEG", &[445], table_rows),
        ],
    );

    // --- LOT RANK RATE 0 : id 1541988908, 5 lignes {rank, rate} (somme = 100). ---
    let rank_rows: Vec<Value> = [(5, 4), (4, 12), (3, 31), (2, 30), (1, 23)]
        .iter()
        .map(|(rk, rt)| node("CPSL_LOT_RANK_RATE_INFO_DATA", &[*rk, *rt], vec![]))
        .collect();
    let lot_rank_list = node(
        "CPSL_LOT_RANK_RATE_INFO_LIST_BEG",
        &[],
        vec![
            node("CPSL_LOT_RANK_RATE_INFO", &[1_541_988_908], vec![]),
            node("CPSL_LOT_RANK_RATE_INFO_DATA_LIST_BEG", &[5], rank_rows),
        ],
    );

    // --- CONFIG INFO : 4 configs réelles (vars plates + 1 ligne DATA de 6 vars chacune). ---
    let config_specs: [([i64; 5], [i64; 6]); 4] = [
        (
            [1_048_507_728, 0, 1_541_988_908, 0, 0],
            [
                -164_431_687,
                -1_275_641_544,
                1_934_060_454,
                737_141_143,
                -553_857_320,
                0,
            ],
        ),
        (
            [-1_956_708_521, 0, 753_914_554, 1, 1],
            [
                -1_777_603_265,
                -1_275_856_345,
                993_989_695,
                1_670_880_782,
                1_408_550_806,
                0,
            ],
        ),
        (
            [-1_066_827_929, 0, -1_243_184_384, 2, 2],
            [
                1_362_153_735,
                -1_962_672_800,
                -868_136_450,
                -1_796_619_313,
                -1_149_967_454,
                0,
            ],
        ),
        (
            [-65_255_421, 0, -1_025_395_818, 3, 3],
            [
                32_525_825,
                -1_108_769_549,
                -1_740_601_530,
                -1_058_534_025,
                -767_243_483,
                0,
            ],
        ),
    ];
    let mut config_children = Vec::new();
    for (hdr, data) in &config_specs {
        config_children.push(node("CPSL_CONFIG_INFO", hdr, vec![]));
        config_children.push(node(
            "CPSL_CONFIG_INFO_DATA_LIST_BEG",
            &[1],
            vec![node("CPSL_CONFIG_INFO_DATA", data, vec![])],
        ));
    }
    let config_list = node("CPSL_CONFIG_INFO_LIST_BEG", &[], config_children);

    json!({
        "entries": [
            weapon_list,
            prize_list,
            prize_table_list,
            lot_rank_list,
            config_list,
        ]
    })
}

#[test]
fn weapon_color_table_reelle() {
    let db = parse_capsule_database(&fixture());
    assert_eq!(db.weapon_color_tables.len(), 1);
    let t = &db.weapon_color_tables[0];
    // id = toHex(401513875) = 0x17EE9D93.
    assert_eq!(t.table_id, HashId::from_i64(401_513_875));
    assert_eq!(t.table_id.to_hex(), "0x17EE9D93");
    // 6 lignes {colorType 1..6, weight 17}.
    assert_eq!(t.colors.len(), 6);
    assert_eq!(
        t.colors[0],
        WeaponColorRate {
            color_type: 1,
            weight: 17
        }
    );
    assert_eq!(
        t.colors[5],
        WeaponColorRate {
            color_type: 6,
            weight: 17
        }
    );
    assert!(t.colors.iter().all(|c| c.weight == 17));
}

#[test]
fn prize_info_reels() {
    let db = parse_capsule_database(&fixture());
    assert_eq!(db.prizes.len(), 4);
    let p0 = &db.prizes[0];
    // id = toHex(vars[0]) = toHex(240076273) = 0x0E4F45F1.
    assert_eq!(p0.id, HashId::from_i64(240_076_273));
    assert_eq!(p0.id.to_hex(), "0x0E4F45F1");
    assert_eq!(p0.vars, vec![240_076_273, -754_029_353, 0, 0, 0, 0]);
    // 2e prix.
    assert_eq!(db.prizes[1].id.to_hex(), "0x25621632");
    assert_eq!(db.prizes[1].vars[1], -131_907_820);
}

#[test]
fn prize_table_reelle() {
    let db = parse_capsule_database(&fixture());
    assert_eq!(db.prize_tables.len(), 1);
    let t = &db.prize_tables[0];
    // id = toHex(-164431687) >>> 0 = 0xF632F8B9.
    assert_eq!(t.id, HashId::from_i64(-164_431_687));
    assert_eq!(t.id.to_hex(), "0xF632F8B9");
    // 3 lignes DATA représentatives (10 vars chacune).
    assert_eq!(t.data.len(), 3);
    assert_eq!(t.data[0].vars.len(), 10);
    assert_eq!(t.data[0].vars[0], 1_517_288_567);
    assert_eq!(t.data[1].vars[0], 1_900_195_764);
}

#[test]
fn lot_rank_rate_reelle() {
    let db = parse_capsule_database(&fixture());
    assert_eq!(db.lot_rank_rates.len(), 1);
    let r = &db.lot_rank_rates[0];
    // id = toHex(1541988908) = 0x5BE8E22C (référencé par CONFIG_INFO_0.vars[2]).
    assert_eq!(r.id, HashId::from_i64(1_541_988_908));
    assert_eq!(r.id.to_hex(), "0x5BE8E22C");
    assert_eq!(r.rates.len(), 5);
    assert_eq!(r.rates[0], RankRate { rank: 5, rate: 4 });
    assert_eq!(r.rates[4], RankRate { rank: 1, rate: 23 });
    // Les taux somment à 100 (cohérence gacha).
    assert_eq!(r.rates.iter().map(|x| x.rate).sum::<i64>(), 100);
}

#[test]
fn config_info_reelles() {
    let db = parse_capsule_database(&fixture());
    assert_eq!(db.configs.len(), 4);
    let c0 = &db.configs[0];
    assert_eq!(c0.vars, vec![1_048_507_728, 0, 1_541_988_908, 0, 0]);
    // vars[2] référence la table de taux par rang 0 (cross-check).
    assert_eq!(HashId::from_i64(c0.vars[2]).to_hex(), "0x5BE8E22C");
    assert_eq!(c0.data.len(), 1);
    assert_eq!(c0.data[0].vars.len(), 6);
    // data[0].vars[0] référence la table de prix 0 (cross-check).
    assert_eq!(c0.data[0].vars[0], -164_431_687);
    assert_eq!(HashId::from_i64(c0.data[0].vars[0]).to_hex(), "0xF632F8B9");
    // 4e config.
    assert_eq!(
        db.configs[3].vars,
        vec![-65_255_421, 0, -1_025_395_818, 3, 3]
    );
}

#[test]
fn cross_references_internes_coherentes() {
    // Garde anti-régression : les identifiants se recoupent entre listes (vérité terrain),
    // ce qui prouve que toHex (signé→non-signé) est appliqué de façon cohérente partout.
    let db = parse_capsule_database(&fixture());
    let rank_id = db.lot_rank_rates[0].id;
    let table_id = db.prize_tables[0].id;
    let cfg = &db.configs[0];
    assert_eq!(
        HashId::from_i64(cfg.vars[2]),
        rank_id,
        "config.vars[2] == rank table id"
    );
    assert_eq!(
        HashId::from_i64(cfg.data[0].vars[0]),
        table_id,
        "config.data[0].vars[0] == prize table id"
    );
}
