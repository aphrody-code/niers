#![allow(clippy::pedantic)]
//! Tests golden `basara` — VRAIES valeurs tirées du noeud RDBN réel :
//! `data/common/gamedata/character/basara_chara_config_0.00.00.00.cfg.bin` (VFS du jeu,
//! 4491 octets, RDBN à listes). Extraites via `cargo run -p nie-game --example extract_basara`
//! (forme iecode `lists`), port 1:1 d'inagle `packages/inagle/src/parsers/basara-config.ts`.
//!
//! ## Valeurs réelles vérifiées
//!
//! `m_basaraBuildInfoList` (71 entrées au total) — 3 premières :
//! - `[0]` : `charaParamId = 0xA41870E9`, `typeInfo = [0, 6]`
//! - `[1]` : `charaParamId = 0x36D8C89C`, `typeInfo = [6, 6]`
//! - `[2]` : `charaParamId = 0x499EC7EC`, `typeInfo = [12, 6]`
//!
//! `m_basaraBuildTypeList` (426 entrées au total) — 12 premières (cycle `5,3,1,4,2,0`) :
//! - `[0]` (5, 0x8F8C55E6) `[1]` (3, 0x1685045C) `[2]` (1, 0x618234CA)
//! - `[3]` (4, 0xFFE6A169) `[4]` (2, 0x88E191FF) `[5]` (0, 0x11E8C045)
//! - `[6]` (5, 0x8F8C55E6) `[7]` (3, 0x1685045C) `[8]` (1, 0x618234CA)
//! - `[9]` (4, 0xFFE6A169) `[10]` (2, 0x88E191FF) `[11]` (0, 0x11E8C045)
//!
//! `typeInfo = [offset, count]` → `m_basaraBuildInfoList[0]` résout `m_basaraBuildTypeList[0..6]`.

use nie_data::basara::parse_basara_chara_config;
use nie_data::hash::HashId;
use serde_json::json;

/// Fixture = sous-ensemble exact du dump réel (3 build infos + 12 build types).
fn fixture_basara() -> serde_json::Value {
    json!({
        "lists": [
            {
                "name": "m_basaraBuildInfoList",
                "typeName": "BASARA_BUILD_INFO",
                "values": [
                    { "charaParamId": "0xA41870E9", "typeInfo": [0, 6] },
                    { "charaParamId": "0x36D8C89C", "typeInfo": [6, 6] },
                    { "charaParamId": "0x499EC7EC", "typeInfo": [12, 6] }
                ]
            },
            {
                "name": "m_basaraBuildTypeList",
                "typeName": "BASARA_BUILD_TYPE",
                "values": [
                    { "type": 5, "boardId": "0x8F8C55E6" },
                    { "type": 3, "boardId": "0x1685045C" },
                    { "type": 1, "boardId": "0x618234CA" },
                    { "type": 4, "boardId": "0xFFE6A169" },
                    { "type": 2, "boardId": "0x88E191FF" },
                    { "type": 0, "boardId": "0x11E8C045" },
                    { "type": 5, "boardId": "0x8F8C55E6" },
                    { "type": 3, "boardId": "0x1685045C" },
                    { "type": 1, "boardId": "0x618234CA" },
                    { "type": 4, "boardId": "0xFFE6A169" },
                    { "type": 2, "boardId": "0x88E191FF" },
                    { "type": 0, "boardId": "0x11E8C045" }
                ]
            }
        ]
    })
}

// ─── Comptes ──────────────────────────────────────────────────────────────────

#[test]
fn comptes_fixture() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    assert_eq!(cfg.build_infos.len(), 3, "fixture : 3 BASARA_BUILD_INFO");
    assert_eq!(cfg.build_types.len(), 12, "fixture : 12 BASARA_BUILD_TYPE");
}

// ─── BASARA_BUILD_INFO ──────────────────────────────────────────────────────────

#[test]
fn build_info_0_chara_param_id() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    // m_basaraBuildInfoList[0].charaParamId = "0xA41870E9"
    assert_eq!(cfg.build_infos[0].chara_param_id, HashId(0xA418_70E9));
}

#[test]
fn build_info_0_type_info_offset_count() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    // m_basaraBuildInfoList[0].typeInfo = [0, 6]
    assert_eq!(
        cfg.build_infos[0].type_info,
        vec![0, 6],
        "typeInfo brut [0,6]"
    );
    assert_eq!(cfg.build_infos[0].type_offset(), 0, "offset = 0");
    assert_eq!(cfg.build_infos[0].type_count(), 6, "count = 6");
}

#[test]
fn build_info_1_et_2() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    // [1] : 0x36D8C89C, [6, 6]
    assert_eq!(cfg.build_infos[1].chara_param_id, HashId(0x36D8_C89C));
    assert_eq!(cfg.build_infos[1].type_info, vec![6, 6]);
    // [2] : 0x499EC7EC, [12, 6]
    assert_eq!(cfg.build_infos[2].chara_param_id, HashId(0x499E_C7EC));
    assert_eq!(cfg.build_infos[2].type_offset(), 12);
    assert_eq!(cfg.build_infos[2].type_count(), 6);
}

// ─── BASARA_BUILD_TYPE ──────────────────────────────────────────────────────────

#[test]
fn build_type_0() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    // m_basaraBuildTypeList[0] = { type: 5, boardId: "0x8F8C55E6" }
    assert_eq!(cfg.build_types[0].build_type, 5);
    assert_eq!(cfg.build_types[0].board_id, HashId(0x8F8C_55E6));
}

#[test]
fn build_type_5_type_zero_garde_board_non_nul() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    // m_basaraBuildTypeList[5] = { type: 0, boardId: "0x11E8C045" } — type 0 conservé.
    assert_eq!(
        cfg.build_types[5].build_type, 0,
        "type 0 est valide, non filtré"
    );
    assert_eq!(cfg.build_types[5].board_id, HashId(0x11E8_C045));
}

#[test]
fn build_types_cycle_5_3_1_4_2_0() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    // Le cycle des types observé dans le dump réel.
    let types: Vec<i64> = cfg.build_types.iter().map(|b| b.build_type).collect();
    assert_eq!(types, vec![5, 3, 1, 4, 2, 0, 5, 3, 1, 4, 2, 0]);
}

#[test]
fn build_types_board_ids_premiers_six() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    let boards: Vec<HashId> = cfg.build_types[..6].iter().map(|b| b.board_id).collect();
    assert_eq!(
        boards,
        vec![
            HashId(0x8F8C_55E6),
            HashId(0x1685_045C),
            HashId(0x6182_34CA),
            HashId(0xFFE6_A169),
            HashId(0x88E1_91FF),
            HashId(0x11E8_C045),
        ]
    );
}

// ─── Résolution [offset, count] ──────────────────────────────────────────────────

#[test]
fn types_of_build_info_0_resout_les_6_premiers() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    // build_infos[0].typeInfo = [0, 6] → build_types[0..6]
    let builds = cfg.types_of(&cfg.build_infos[0]);
    assert_eq!(builds.len(), 6, "6 builds pour 0xA41870E9");
    assert_eq!(builds[0].build_type, 5);
    assert_eq!(builds[0].board_id, HashId(0x8F8C_55E6));
    assert_eq!(builds[5].build_type, 0);
    assert_eq!(builds[5].board_id, HashId(0x11E8_C045));
}

#[test]
fn types_of_build_info_1_resout_la_tranche_suivante() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    // build_infos[1].typeInfo = [6, 6] → build_types[6..12]
    let builds = cfg.types_of(&cfg.build_infos[1]);
    assert_eq!(builds.len(), 6, "6 builds pour 0x36D8C89C");
    assert_eq!(builds[0].board_id, HashId(0x8F8C_55E6));
    assert_eq!(builds[5].build_type, 0);
}

#[test]
fn types_of_hors_limites_renvoie_vide() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    // build_infos[2].typeInfo = [12, 6] → [12..18], or la fixture n'a que 12 build_types.
    let builds = cfg.types_of(&cfg.build_infos[2]);
    assert!(
        builds.is_empty(),
        "plage hors limites → tranche vide (pas de panique)"
    );
}

// ─── Accesseurs ──────────────────────────────────────────────────────────────────

#[test]
fn find_build_trouve_et_absent() {
    let cfg = parse_basara_chara_config(&fixture_basara());
    let found = cfg.find_build(HashId(0x36D8_C89C));
    assert!(
        found.is_some(),
        "find_build(0x36D8C89C) doit trouver l'entrée"
    );
    assert_eq!(found.unwrap().type_offset(), 6);

    assert!(
        cfg.find_build(HashId(0xDEAD_BEEF)).is_none(),
        "hash absent → None"
    );
}

#[test]
fn liste_inconnue_renvoie_vide() {
    // Un root sans les listes attendues → config vide, pas de panique.
    let cfg = parse_basara_chara_config(&json!({ "lists": [] }));
    assert!(cfg.build_infos.is_empty());
    assert!(cfg.build_types.is_empty());
}
