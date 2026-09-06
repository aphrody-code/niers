#![allow(clippy::pedantic)]
//! Tests golden `update_notice`.
//!
//! **Le fichier ne s'appelle pas `update_notice_golden.rs`** : Windows applique une heuristique
//! de compatibilité (« installer detection ») aux exécutables dont le nom contient `update`,
//! `setup`, `install` ou `patch`, et exige alors une élévation UAC. `cargo test` échouait donc
//! avec « L'opération demandée nécessite une élévation » (os error 740), sans que le test soit
//! seulement exécuté. Renommer le fichier suffit ; ne pas le renommer en arrière.
//!
//! Valeurs réelles tirées de :
//! `update_notice/update_notice_config_0.00.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ (fichier → valeur confirmée)
//!
//! ### m_updateNoticeDataList (26 entrées)
//! - `[0]`  = { textureName: "0xAB293174", textId: "0x29F4B231" }
//! - `[2]`  = { textureName: "0xDC2E01E2", textId: "0x6AEDC5B5" }
//! - `[25]` = { textureName: "0xDC2E01E2", textId: "0x1FB66B97" } (dernier)
//!
//! ### m_updateNoticeInfoList (4 entrées)
//! - `[0]` = updateId 0x647E0ABB, globalBitFlagId 0x00C0E385, data [0, 8], enableCond "AAAAABgF...AAAAAXk="
//! - `[1]` = updateId 0xFD775B01, globalBitFlagId 0xEF0288BB, data [8, 6]
//! - `[2]` = updateId 0x8A706B97, globalBitFlagId 0x0DDE93C2, data [14, 5]
//! - `[3]` = updateId 0x1414FE34, globalBitFlagId 0xE21CF8FC, data [19, 7]
//!
//! Les 4 tranches partitionnent exactement les 26 entrées : 0+8=8, 8+6=14, 14+5=19, 19+7=26.

mod common;

use nie_data::hash::HashId;
use nie_data::update_notice::{
    UpdateNoticeConfig, UpdateNoticeData, UpdateNoticeInfo, parse_update_notice_config,
};
use serde_json::json;

// ─── Fixture ──────────────────────────────────────────────────────────────────
// Valeurs extraites octet-pour-octet du dump réel — voir commentaires ci-dessus.

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_updateNoticeDataList",
                "typeName": "UPDATE_NOTICE_DATA",
                "values": [
                    { "textureName": "0xAB293174", "textId": "0x29F4B231" },
                    { "textureName": "0xAB293174", "textId": "0x4997560F" },
                    { "textureName": "0xDC2E01E2", "textId": "0x6AEDC5B5" }
                ]
            },
            {
                "name": "m_updateNoticeInfoList",
                "typeName": "UPDATE_NOTICE_INFO",
                "values": [
                    {
                        "updateId": "0x647E0ABB",
                        "globalBitFlagId": "0x00C0E385",
                        "updateNoticeData": [0, 8],
                        "enableCond": "AAAAABgFNSo9RUMACgEoAAYCNADA44UyAAAAAXk="
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn fixture_comptes() {
    let cfg = parse_update_notice_config(&fixture());
    assert_eq!(cfg.data.len(), 3);
    assert_eq!(cfg.infos.len(), 1);
}

#[test]
fn fixture_data0_champs() {
    let cfg = parse_update_notice_config(&fixture());
    let d: &UpdateNoticeData = &cfg.data[0];
    assert_eq!(d.texture_name, HashId(0xAB29_3174));
    assert_eq!(d.text_id, HashId(0x29F4_B231));
}

#[test]
fn fixture_data2_champs() {
    let cfg = parse_update_notice_config(&fixture());
    let d = &cfg.data[2];
    assert_eq!(d.texture_name, HashId(0xDC2E_01E2));
    assert_eq!(d.text_id, HashId(0x6AED_C5B5));
}

#[test]
fn fixture_info0_champs() {
    let cfg = parse_update_notice_config(&fixture());
    let i: &UpdateNoticeInfo = &cfg.infos[0];
    assert_eq!(i.update_id, HashId(0x647E_0ABB));
    assert_eq!(i.global_bit_flag_id, HashId(0x00C0_E385));
    assert_eq!(i.data_start, 0);
    assert_eq!(i.data_count, 8);
    assert_eq!(i.enable_cond, "AAAAABgFNSo9RUMACgEoAAYCNADA44UyAAAAAXk=");
}

#[test]
fn fixture_find_by_update_id() {
    let cfg = parse_update_notice_config(&fixture());
    assert!(cfg.find_by_update_id(HashId(0x647E_0ABB)).is_some());
    assert!(cfg.find_by_update_id(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn fixture_pages_of_borne() {
    // data_count=8 mais seules 3 pages existent dans la fixture → tranche bornée à 3.
    let cfg = parse_update_notice_config(&fixture());
    let pages = cfg.pages_of(&cfg.infos[0]);
    assert_eq!(pages.len(), 3);
    assert_eq!(pages[0].texture_name, HashId(0xAB29_3174));
}

#[test]
fn fixture_listes_manquantes_renvoient_vide() {
    let root = json!({ "version": 100, "lists": [] });
    let cfg = parse_update_notice_config(&root);
    assert_eq!(cfg.data.len(), 0);
    assert_eq!(cfg.infos.len(), 0);
}

// ─── Test sur le vrai fichier (skip si absent du VPS) ────────────────────────

const REAL_PATH: &str = "update_notice/update_notice_config_0.00.00.cfg.bin.json";

fn load_real() -> Option<UpdateNoticeConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_update_notice_config(&root))
}

#[test]
fn real_file_comptes() {
    let Some(cfg) = load_real() else { return };
    // 26 pages dans m_updateNoticeDataList, 4 groupes dans m_updateNoticeInfoList.
    assert_eq!(cfg.data.len(), 26, "26 entrées UPDATE_NOTICE_DATA");
    assert_eq!(cfg.infos.len(), 4, "4 entrées UPDATE_NOTICE_INFO");
}

#[test]
fn real_file_data0() {
    let Some(cfg) = load_real() else { return };
    let d = &cfg.data[0];
    assert_eq!(d.texture_name, HashId(0xAB29_3174), "textureName[0]");
    assert_eq!(d.text_id, HashId(0x29F4_B231), "textId[0]");
}

#[test]
fn real_file_data2() {
    let Some(cfg) = load_real() else { return };
    let d = &cfg.data[2];
    assert_eq!(d.texture_name, HashId(0xDC2E_01E2), "textureName[2]");
    assert_eq!(d.text_id, HashId(0x6AED_C5B5), "textId[2]");
}

#[test]
fn real_file_data_dernier() {
    let Some(cfg) = load_real() else { return };
    // m_updateNoticeDataList[25] (dernier)
    let d = &cfg.data[25];
    assert_eq!(d.texture_name, HashId(0xDC2E_01E2), "textureName[25]");
    assert_eq!(d.text_id, HashId(0x1FB6_6B97), "textId[25]");
}

#[test]
fn real_file_info0() {
    let Some(cfg) = load_real() else { return };
    let i = &cfg.infos[0];
    assert_eq!(i.update_id, HashId(0x647E_0ABB), "updateId[0]");
    assert_eq!(
        i.global_bit_flag_id,
        HashId(0x00C0_E385),
        "globalBitFlagId[0]"
    );
    assert_eq!(i.data_start, 0, "data_start[0]");
    assert_eq!(i.data_count, 8, "data_count[0]");
    assert_eq!(
        i.enable_cond, "AAAAABgFNSo9RUMACgEoAAYCNADA44UyAAAAAXk=",
        "enableCond[0]"
    );
}

#[test]
fn real_file_info_tranches() {
    let Some(cfg) = load_real() else { return };
    // Les 4 tranches [start, count] observées dans le dump réel.
    let attendu = [
        (HashId(0x647E_0ABB), 0_i64, 8_i64),
        (HashId(0xFD77_5B01), 8, 6),
        (HashId(0x8A70_6B97), 14, 5),
        (HashId(0x1414_FE34), 19, 7),
    ];
    for (idx, (id, start, count)) in attendu.iter().enumerate() {
        let i = &cfg.infos[idx];
        assert_eq!(i.update_id, *id, "updateId[{idx}]");
        assert_eq!(i.data_start, *start, "data_start[{idx}]");
        assert_eq!(i.data_count, *count, "data_count[{idx}]");
    }
}

#[test]
fn real_file_tranches_partitionnent() {
    let Some(cfg) = load_real() else { return };
    // start[0]=0 ; chaque groupe suit le précédent ; la fin couvre les 26 pages.
    let mut attendu_start = 0_i64;
    for info in &cfg.infos {
        assert_eq!(info.data_start, attendu_start, "tranches contiguës");
        attendu_start += info.data_count;
    }
    assert_eq!(
        attendu_start as usize,
        cfg.data.len(),
        "couvre toutes les pages"
    );
}

#[test]
fn real_file_pages_of_resout() {
    let Some(cfg) = load_real() else { return };
    // Le dernier groupe [19, 7] résout 7 pages, la première étant data[19].
    let dernier = &cfg.infos[3];
    let pages = cfg.pages_of(dernier);
    assert_eq!(pages.len(), 7);
    assert_eq!(pages[0], cfg.data[19]);
    assert_eq!(pages[6], cfg.data[25]);
}

#[test]
fn real_file_find_by_update_id() {
    let Some(cfg) = load_real() else { return };
    let found = cfg.find_by_update_id(HashId(0x8A70_6B97));
    assert!(found.is_some(), "find_by_update_id(0x8A706B97)");
    assert_eq!(found.unwrap().data_start, 14);
}
