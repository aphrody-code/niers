#![allow(clippy::pedantic)]
//! Tests golden `belong_team` — noeud réel tiré du VFS IEVR :
//! `data/common/gamedata/character/belong_team_config_0.00.00.cfg.bin`
//! (RDBN `lists`, 1 liste `m_belongTeamInfoList` / `typeName` `BELONG_TEAM_INFO`, **208 entrées**).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/belong-team.ts` (interface `BelongTeamInfo`
//! l.24-45, lecture `lists[0].values` l.227-233). Valeurs ci-dessous extraites telles quelles
//! du dump (via `nie_formats::cfgbin::read_values` → forme iecode), aucune valeur inventée.

use nie_data::belong_team::{BelongTeamInfo, Season, find_by_id, parse_belong_team_config};
use nie_data::hash::HashId;
use serde_json::{Value, json};

/// 4 premières entrées réelles de `m_belongTeamInfoList` (index 0..3 du dump).
fn belong_teams_head() -> Vec<Value> {
    vec![
        json!({
            "belongTeamId": "0xF01BB293", "binderTeamOrderType": 1,
            "teamNameTextId": "0x89A9174E",
            "teamEmblemId_IE": "0xD5ABE1F0", "teamEmblemId_GO": "0x1D4B6E80",
            "teamEmblemId_AREORI": "0x5AEB1450", "teamEmblemId_V": "0x87FE63EF",
            "teamNumber_IE1": 1, "teamNumber_IE2": 18, "teamNumber_IE3": 0,
            "teamNumber_GO1": 0, "teamNumber_GO2": 0, "teamNumber_GO3": 0,
            "teamNumber_ARES": 1, "teamNumber_ORION": 0, "teamNumber_V": 7,
            "teamKit_IE": "0x252CE113", "teamKit_GO": "0xAECB5B3E",
            "teamKit_AREORI": "0x00000000", "teamKit_V": "0xBBE7C49D"
        }),
        json!({
            "belongTeamId": "0x699E68EC", "binderTeamOrderType": 2,
            "teamNameTextId": "0x82C9C16B",
            "teamEmblemId_IE": "0x4CA2B04A", "teamEmblemId_GO": "0x832FFB23",
            "teamEmblemId_AREORI": "0xB388B165", "teamEmblemId_V": "0x672B8AF1",
            "teamNumber_IE1": 2, "teamNumber_IE2": 0, "teamNumber_IE3": 0,
            "teamNumber_GO1": 8, "teamNumber_GO2": 0, "teamNumber_GO3": 2,
            "teamNumber_ARES": 7, "teamNumber_ORION": 0, "teamNumber_V": 6,
            "teamKit_IE": "0xD6ABDBB4", "teamKit_GO": "0x93FF607B",
            "teamKit_AREORI": "0x927DBD05", "teamKit_V": "0xDC35BEB6"
        }),
        json!({
            "belongTeamId": "0x83D22BED", "binderTeamOrderType": 3,
            "teamNameTextId": "0x640E03F2",
            "teamEmblemId_IE": "0xCCB0D0B1", "teamEmblemId_GO": "0x00000000",
            "teamEmblemId_AREORI": "0x00000000", "teamEmblemId_V": "0x00000000",
            "teamNumber_IE1": 3, "teamNumber_IE2": 0, "teamNumber_IE3": 0,
            "teamNumber_GO1": 4, "teamNumber_GO2": 0, "teamNumber_GO3": 0,
            "teamNumber_ARES": 0, "teamNumber_ORION": 0, "teamNumber_V": 0,
            "teamKit_IE": "0x297C6041", "teamKit_GO": "0x6A1C4C4D",
            "teamKit_AREORI": "0x00000000", "teamKit_V": "0x00000000"
        }),
        json!({
            "belongTeamId": "0x4DC11E01", "binderTeamOrderType": 4,
            "teamNameTextId": "0x3EF65F2B",
            "teamEmblemId_IE": "0x3BA580DC", "teamEmblemId_GO": "0x00000000",
            "teamEmblemId_AREORI": "0x00000000", "teamEmblemId_V": "0x00000000",
            "teamNumber_IE1": 4, "teamNumber_IE2": 0, "teamNumber_IE3": 0,
            "teamNumber_GO1": 0, "teamNumber_GO2": 0, "teamNumber_GO3": 0,
            "teamNumber_ARES": 0, "teamNumber_ORION": 0, "teamNumber_V": 0,
            "teamKit_IE": "0xB590F066", "teamKit_GO": "0x00000000",
            "teamKit_AREORI": "0x00000000", "teamKit_V": "0x00000000"
        }),
    ]
}

/// Fixture iecode complète : la liste unique (4 entrées réelles).
fn node_fixture() -> Value {
    json!({
        "lists": [
            { "name": "m_belongTeamInfoList", "typeName": "BELONG_TEAM_INFO",
              "values": belong_teams_head() },
        ]
    })
}

#[test]
fn parse_compte_les_entrees() {
    let teams = parse_belong_team_config(&node_fixture());
    assert_eq!(teams.len(), 4);
}

#[test]
fn team_zero_champs_exacts() {
    let teams = parse_belong_team_config(&node_fixture());
    let t: &BelongTeamInfo = &teams[0];
    assert_eq!(t.belong_team_id, HashId(0xF01B_B293));
    assert_eq!(t.binder_team_order_type, 1);
    assert_eq!(t.team_name_text_id, HashId(0x89A9_174E));
    // Emblèmes par série.
    assert_eq!(t.team_emblem_id_ie, HashId(0xD5AB_E1F0));
    assert_eq!(t.team_emblem_id_go, HashId(0x1D4B_6E80));
    assert_eq!(t.team_emblem_id_areori, HashId(0x5AEB_1450));
    assert_eq!(t.team_emblem_id_v, HashId(0x87FE_63EF));
    // Numéros d'apparition par saison.
    assert_eq!(t.team_number_ie1, 1);
    assert_eq!(t.team_number_ie2, 18);
    assert_eq!(t.team_number_ie3, 0);
    assert_eq!(t.team_number_go1, 0);
    assert_eq!(t.team_number_go2, 0);
    assert_eq!(t.team_number_go3, 0);
    assert_eq!(t.team_number_ares, 1);
    assert_eq!(t.team_number_orion, 0);
    assert_eq!(t.team_number_v, 7);
    // Maillots par série (teamKit_AREORI = sentinel 0x00000000 → ZERO).
    assert_eq!(t.team_kit_ie, HashId(0x252C_E113));
    assert_eq!(t.team_kit_go, HashId(0xAECB_5B3E));
    assert_eq!(t.team_kit_areori, HashId::ZERO);
    assert_eq!(t.team_kit_v, HashId(0xBBE7_C49D));
}

#[test]
fn seasons_helper_correspond_au_filtre_inagle() {
    let teams = parse_belong_team_config(&node_fixture());
    let t = &teams[0];
    // appears_in = teamNumber > 0 (inagle l.283-291).
    assert!(t.appears_in(Season::Ie1));
    assert!(t.appears_in(Season::Ie2));
    assert!(!t.appears_in(Season::Ie3));
    assert!(!t.appears_in(Season::Go1));
    assert!(t.appears_in(Season::Ares));
    assert!(!t.appears_in(Season::Orion));
    assert!(t.appears_in(Season::V));
    // team_number rend la valeur brute.
    assert_eq!(t.team_number(Season::Ie2), 18);
    assert_eq!(t.team_number(Season::V), 7);
    assert_eq!(t.team_number(Season::Orion), 0);
}

#[test]
fn team_un_emblemes_complets_kits_complets() {
    // Entrée 1 : toutes les séries ont emblème + maillot (aucun sentinel ZERO).
    let teams = parse_belong_team_config(&node_fixture());
    let t = &teams[1];
    assert_eq!(t.belong_team_id, HashId(0x699E_68EC));
    assert_eq!(t.team_emblem_id_areori, HashId(0xB388_B165));
    assert_eq!(t.team_kit_areori, HashId(0x927D_BD05));
    assert_eq!(t.team_number_go3, 2);
    assert!(t.appears_in(Season::Go1));
    assert!(t.appears_in(Season::Go3));
    assert!(!t.appears_in(Season::Go2));
}

#[test]
fn sentinel_zero_emblemes_et_kits_absents() {
    // Entrée 2 : seules les séries IE/GO ont des données ; AREORI/V sont 0x00000000 → ZERO.
    let teams = parse_belong_team_config(&node_fixture());
    let t = &teams[2];
    assert_eq!(t.team_emblem_id_ie, HashId(0xCCB0_D0B1));
    assert_eq!(t.team_emblem_id_go, HashId::ZERO);
    assert_eq!(t.team_emblem_id_areori, HashId::ZERO);
    assert_eq!(t.team_emblem_id_v, HashId::ZERO);
    assert_eq!(t.team_kit_go, HashId(0x6A1C_4C4D));
    assert_eq!(t.team_kit_v, HashId::ZERO);
}

#[test]
fn recherche_par_id() {
    let teams = parse_belong_team_config(&node_fixture());
    let found = find_by_id(&teams, HashId(0x4DC1_1E01));
    assert!(found.is_some());
    assert_eq!(found.unwrap().binder_team_order_type, 4);
    assert_eq!(found.unwrap().team_emblem_id_ie, HashId(0x3BA5_80DC));
    assert!(find_by_id(&teams, HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn vrai_fichier_si_present() {
    // Validation contre le vrai dump si monté localement (sinon skip).
    // Source VFS : data/common/gamedata/character/belong_team_config_0.00.00.cfg.bin.
    let path = "/home/aphrody/niers/data/common/gamedata/character/\
                belong_team_config_0.00.00.cfg.bin.json";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let content = std::fs::read_to_string(path).unwrap();
    let root: Value = serde_json::from_str(&content).unwrap();
    let teams = parse_belong_team_config(&root);
    assert_eq!(
        teams.len(),
        208,
        "208 équipes d'appartenance dans le vrai dump"
    );
    assert_eq!(teams[0].belong_team_id, HashId(0xF01B_B293));
    assert_eq!(teams[0].team_number_ie2, 18);
}
