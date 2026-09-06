#![allow(clippy::pedantic)]
//! Tests golden `chara_series` — port 1:1 d'inagle `belong-team.ts` (`loadCharaSeriesInfo`,
//! `SERIES_NAMES_BY_TYPE`) + `chara_base::resolve_series_name_fr`.
//!
//! Vérité terrain (via `cargo run -p nie-game --example extract_chara_series`) : **9 séries**,
//! `0x62E8448F` = type 1 = « Inazuma Eleven » ; Endou `c01000010` (series_id `0x62E8448F`) →
//! « Inazuma Eleven » (protagoniste d'IE1). Distribution réelle sur le roster : IE 1058, IE2 671,
//! IE3 646, GO 980, Chrono Stone 543, Galaxy 395, Ares 244, Orion 210, Victory Road 1149.

use nie_data::chara_base::{CharaBase, resolve_series_name_fr};
use nie_data::chara_series::{
    CharaSeriesInfo, find_by_id, parse_chara_series_config, series_name_en, series_name_fr,
};
use nie_data::hash::HashId;
use serde_json::json;

#[test]
fn series_names_by_type() {
    assert_eq!(series_name_fr(1), Some("Inazuma Eleven"));
    assert_eq!(series_name_fr(4), Some("Inazuma Eleven GO"));
    assert_eq!(series_name_fr(5), Some("Chrono Stone"));
    assert_eq!(series_name_fr(9), Some("Victory Road"));
    assert_eq!(series_name_en(7), Some("Ares"));
    assert_eq!(series_name_fr(0), None);
    assert_eq!(series_name_fr(10), None);
}

#[test]
fn parses_config_and_lookup() {
    let root = json!({
        "lists": [{
            "name": "m_charaSeriesInfoList",
            "values": [
                { "charaSeriesId": "0x62E8448F", "charaSeriesType": 1, "charaSeriesNameTextId": "0xE6D1097B" },
                { "charaSeriesId": "0xE177FC30", "charaSeriesType": 9, "charaSeriesNameTextId": "0x00000000" },
            ]
        }]
    });
    let series = parse_chara_series_config(&root);
    assert_eq!(series.len(), 2);
    assert_eq!(series[0].series_id, HashId(0x62E8_448F));
    assert_eq!(series[0].series_type, 1);
    let found = find_by_id(&series, HashId(0xE177_FC30)).expect("série 9 présente");
    assert_eq!(found.series_type, 9);
    assert_eq!(find_by_id(&series, HashId(0xDEAD_BEEF)), None);
}

#[test]
fn chara_base_resolves_series() {
    let series = vec![CharaSeriesInfo {
        series_id: HashId(0x62E8_448F),
        series_type: 1,
        name_text_id: HashId(0xE6D1_097B),
    }];
    let mut endou = CharaBase {
        chara_id: HashId(0x99A1_C150),
        internal_code: "c01000010".into(),
        name_hash: HashId(0xE551_FF12),
        last_name_hash: None,
        description_hash: None,
        gender: 1,
        series_id: Some(HashId(0x62E8_448F)),
        belong_team_id: None,
    };
    assert_eq!(
        resolve_series_name_fr(&endou, &series),
        Some("Inazuma Eleven")
    );
    // series_id absent → None.
    endou.series_id = None;
    assert_eq!(resolve_series_name_fr(&endou, &series), None);
}
