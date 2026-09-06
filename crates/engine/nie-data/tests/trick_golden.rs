#![allow(clippy::pedantic)]
//! Tests golden `trick` — **vraies valeurs** de `m_trickInfoList` extraites du VFS :
//! `data/common/gamedata/skill/trick_config.cfg.bin` (RDBN, liste `m_trickInfoList`,
//! type `TRICK_INFO`, 9 lignes × 8 champs), monté depuis
//! `/mnt/c/Program Files (x86)/Steam/steamapps/common/INAZUMA ELEVEN Victory Road`.
//!
//! Extraction : décodage RDBN → forme iecode `lists` (`nie-formats::cfgbin` +
//! `cfgbin_to_iecode_root`), strictement identique à ce que consomment les parseurs typés
//! de `nie-data`. Port 1:1 d'inagle `packages/inagle/src/parsers/trick-config.ts`.
//!
//! Note `failEventIDName` : champ `Condition`. Sans événement d'échec (catégories Tir/Dribble
//! et certains blocs), sa valeur brute est la sentinelle `"0xFFFFFFFF"` ; avec échec
//! (blocs/arrêts contestés), la chaîne réelle `ev6x_xxxx_2` est résolue. Valeurs ci-dessous
//! copiées telles quelles du dump.

use nie_data::hash::HashId;
use nie_data::trick::{TrickCategory, TrickInfo, find_trick, parse_trick_config};
use serde_json::json;

/// Fixture : les **9 valeurs réelles** de `m_trickInfoList` (dump VFS), forme iecode.
fn trick_config_fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [{
            "name": "m_trickInfoList",
            "typeName": "TRICK_INFO",
            "values": [
                // ligne 0 — ファイアトルネード (Fire Tornado), tir
                {
                    "trickID": "0xDAB20CDC", "trickIDName": "whs0010",
                    "eventID": "0xDB8CADA2", "eventIDName": "ev60_0010",
                    "failEventID": "0x00000000", "failEventIDName": "0xFFFFFFFF",
                    "trickName": "ファイアトルネード", "trickCategory": 1
                },
                // ligne 1 — シャイニングバード（アレス）, tir
                {
                    "trickID": "0xDB7066EB", "trickIDName": "whs0110",
                    "eventID": "0xDA4EC795", "eventIDName": "ev60_0110",
                    "failEventID": "0x00000000", "failEventIDName": "0xFFFFFFFF",
                    "trickName": "シャイニングバード（アレス）", "trickCategory": 1
                },
                // ligne 2 — そよかぜステップ, dribble
                {
                    "trickID": "0x7FA2765F", "trickIDName": "who0010",
                    "eventID": "0x10D07E07", "eventIDName": "ev61_0010",
                    "failEventID": "0x00000000", "failEventIDName": "0xFFFFFFFF",
                    "trickName": "そよかぜステップ", "trickCategory": 2
                },
                // ligne 3 — イナビカリダッシュ（アレス）, dribble
                {
                    "trickID": "0x548F259C", "trickIDName": "who0020",
                    "eventID": "0x3BFD2DC4", "eventIDName": "ev61_0020",
                    "failEventID": "0x00000000", "failEventIDName": "0xFFFFFFFF",
                    "trickName": "イナビカリダッシュ（アレス）", "trickCategory": 2
                },
                // ligne 4 — 旋風陣 (Whirlwind), bloc AVEC événement d'échec
                {
                    "trickID": "0x0872474E", "trickIDName": "whd0010",
                    "eventID": "0x5B578E91", "eventIDName": "ev62_0010_1",
                    "failEventID": "0xC25EDF2B", "failEventIDName": "ev62_0010_2",
                    "trickName": "旋風陣", "trickCategory": 3
                },
                // ligne 5 — ザ・ミスト, bloc sans échec
                {
                    "trickID": "0x6C1E824A", "trickIDName": "whd0050",
                    "eventID": "0xF228C9AD", "eventIDName": "ev62_0050",
                    "failEventID": "0x00000000", "failEventIDName": "0xFFFFFFFF",
                    "trickName": "ザ・ミスト", "trickCategory": 3
                },
                // ligne 6 — ゴッドハンド (God Hand), arrêt AVEC échec
                {
                    "trickID": "0x8A22D09F", "trickIDName": "whk0010",
                    "eventID": "0x97FD8E0F", "eventIDName": "ev63_0010_1",
                    "failEventID": "0x0EF4DFB5", "failEventIDName": "ev63_0010_2",
                    "trickName": "ゴッドハンド", "trickCategory": 4
                },
                // ligne 7 — ギガントウォール (Gigant Wall), arrêt AVEC échec
                {
                    "trickID": "0xA10F835C", "trickIDName": "whk0020",
                    "eventID": "0x854821E1", "eventIDName": "ev63_0020_1",
                    "failEventID": "0x1C41705B", "failEventIDName": "ev63_0020_2",
                    "trickName": "ギガントウォール", "trickCategory": 4
                },
                // ligne 8 — 王家の盾（アレス）, arrêt AVEC échec
                {
                    "trickID": "0xB814B21D", "trickIDName": "whk0030",
                    "eventID": "0x3DF44684", "eventIDName": "ev63_0030_1",
                    "failEventID": "0xA4FD173E", "failEventIDName": "ev63_0030_2",
                    "trickName": "王家の盾（アレス）", "trickCategory": 4
                }
            ]
        }]
    })
}

#[test]
fn trick_config_compte_9_lignes() {
    let tricks = parse_trick_config(&trick_config_fixture());
    assert_eq!(tricks.len(), 9, "m_trickInfoList = 9 valeurs (dump VFS)");
}

#[test]
fn trick_ligne_0_fire_tornado() {
    let tricks = parse_trick_config(&trick_config_fixture());
    let t: &TrickInfo = &tricks[0];
    assert_eq!(t.trick_id, HashId(0xDAB2_0CDC), "trickID = 0xDAB20CDC");
    assert_eq!(t.trick_id_name, "whs0010", "trickIDName = whs0010");
    assert_eq!(t.event_id, HashId(0xDB8C_ADA2), "eventID = 0xDB8CADA2");
    assert_eq!(t.event_id_name, "ev60_0010", "eventIDName = ev60_0010");
    assert_eq!(t.fail_event_id, HashId::ZERO, "failEventID = 0");
    assert_eq!(
        t.fail_event_id_name, "0xFFFFFFFF",
        "sentinelle failEventIDName"
    );
    assert_eq!(t.trick_name, "ファイアトルネード", "trickName JP");
    assert_eq!(t.trick_category, 1, "trickCategory = 1 (Tir)");
    assert_eq!(t.category(), TrickCategory::Shoot);
    assert_eq!(t.category_name(), "Tir");
    assert!(!t.has_fail_event(), "pas d'événement d'échec (tir)");
}

#[test]
fn trick_ligne_4_bloc_avec_echec() {
    let tricks = parse_trick_config(&trick_config_fixture());
    let t = &tricks[4];
    assert_eq!(t.trick_id, HashId(0x0872_474E), "trickID = 0x0872474E");
    assert_eq!(t.trick_id_name, "whd0010");
    assert_eq!(t.event_id_name, "ev62_0010_1");
    // Avec échec : failEventID non nul et failEventIDName = chaîne réelle résolue.
    assert_eq!(
        t.fail_event_id,
        HashId(0xC25E_DF2B),
        "failEventID = 0xC25EDF2B"
    );
    assert_eq!(
        t.fail_event_id_name, "ev62_0010_2",
        "failEventIDName résolu"
    );
    assert_eq!(t.trick_name, "旋風陣");
    assert_eq!(t.category(), TrickCategory::Block);
    assert_eq!(t.category_name(), "Bloc");
    assert!(t.has_fail_event(), "bloc contesté → événement d'échec");
}

#[test]
fn trick_categories_par_prefixe() {
    let tricks = parse_trick_config(&trick_config_fixture());
    // whs* = Tir(1), who* = Dribble(2), whd* = Bloc(3), whk* = Arrêt(4).
    let cat = |i: usize| tricks[i].category();
    assert_eq!(cat(0), TrickCategory::Shoot, "whs0010");
    assert_eq!(cat(1), TrickCategory::Shoot, "whs0110");
    assert_eq!(cat(2), TrickCategory::Dribble, "who0010");
    assert_eq!(cat(3), TrickCategory::Dribble, "who0020");
    assert_eq!(cat(5), TrickCategory::Block, "whd0050");
    assert_eq!(cat(6), TrickCategory::Catch, "whk0010");
    assert_eq!(cat(8), TrickCategory::Catch, "whk0030");
    // Noms français (table inagle, sans accent).
    assert_eq!(tricks[2].category_name(), "Dribble");
    assert_eq!(tricks[6].category_name(), "Arret");
}

#[test]
fn trick_categorie_inconnue_fallback_cat_n() {
    // Port du fallback `Cat${n}` d'inagle pour une catégorie hors table.
    assert_eq!(TrickCategory::from_id(7), TrickCategory::Other(7));
    assert_eq!(TrickCategory::from_id(7).name_fr(), "Cat7");
    let v = json!({ "trickID": "0x00000000", "trickCategory": 9 });
    let t = TrickInfo::from_value(&v);
    assert_eq!(t.category_name(), "Cat9");
}

#[test]
fn trick_find_by_id() {
    let tricks = parse_trick_config(&trick_config_fixture());
    // ゴッドハンド (God Hand) = 0x8A22D09F.
    let found = find_trick(&tricks, HashId(0x8A22_D09F));
    assert!(found.is_some(), "trick 0x8A22D09F trouvée");
    assert_eq!(found.unwrap().trick_id_name, "whk0010");
    assert!(
        find_trick(&tricks, HashId(0xDEAD_BEEF)).is_none(),
        "id inconnu → None"
    );
}

#[test]
fn trick_5_blocs_arrets_ont_un_echec() {
    let tricks = parse_trick_config(&trick_config_fixture());
    // Lignes 4, 6, 7, 8 ont un événement d'échec (4 au total).
    let with_fail: usize = tricks.iter().filter(|t| t.has_fail_event()).count();
    assert_eq!(with_fail, 4, "4 tricks avec événement d'échec (dump réel)");
    // Toutes les tricks sans échec gardent la sentinelle.
    for t in tricks.iter().filter(|t| !t.has_fail_event()) {
        assert_eq!(
            t.fail_event_id_name, "0xFFFFFFFF",
            "{} → sentinelle",
            t.trick_id_name
        );
    }
}
