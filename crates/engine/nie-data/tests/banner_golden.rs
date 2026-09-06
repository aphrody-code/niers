#![allow(clippy::pedantic)]
//! Tests golden `banner` — valeurs réelles tirées de :
//! `banner/tutorial_banner_config_0.00.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ
//!
//! ### m_tutorialBannerInfoList[0]
//! - `id`              = `"0x74AD78BE"` → `HashId(0x74AD78BE)`
//! - `titleTextId`     = `"0x968619E7"` → `HashId(0x968619E7)`
//! - `explainTextId`   = `"0x968619E7"` → `HashId(0x968619E7)`
//! - `iconTextureName` = `"0xCF0200B5"` → `HashId(0xCF0200B5)`
//! - `count`           = `1`
//! - `flagNum`         = `1`
//!
//! ### m_tutorialBannerInfoList[1]
//! - `id`              = `"0xEDA42904"` → `HashId(0xEDA42904)`
//! - `iconTextureName` = `"0x560B510F"` → `HashId(0x560B510F)`
//! - `count`           = `2`
//! - `flagNum`         = `2`
//!
//! ### m_tutorialBannerInfoList[2]
//! - `id`              = `"0x9AA31992"` → `HashId(0x9AA31992)`
//! - `iconTextureName` = `"0x210C6199"` → `HashId(0x210C6199)`
//! - `count`           = `3`
//! - `flagNum`         = `3`

mod common;

use nie_data::banner::parse_banner_config;
use nie_data::hash::HashId;
use serde_json::json;

// ─── Fixture inline ──────────────────────────────────────────────────────────
// Valeurs extraites champ par champ du dump réel — voir commentaires ci-dessus.

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_tutorialBannerInfoList",
                "typeName": "TUTORIAL_BANNER_INFO",
                "values": [
                    // entrée 0 — dump réel lignes 7-13
                    {
                        "id": "0x74AD78BE",
                        "titleTextId": "0x968619E7",
                        "explainTextId": "0x968619E7",
                        "iconTextureName": "0xCF0200B5",
                        "count": 1,
                        "flagNum": 1
                    },
                    // entrée 1 — dump réel lignes 14-20
                    {
                        "id": "0xEDA42904",
                        "titleTextId": "0x968619E7",
                        "explainTextId": "0x968619E7",
                        "iconTextureName": "0x560B510F",
                        "count": 2,
                        "flagNum": 2
                    },
                    // entrée 2 — dump réel lignes 21-27
                    {
                        "id": "0x9AA31992",
                        "titleTextId": "0x968619E7",
                        "explainTextId": "0x968619E7",
                        "iconTextureName": "0x210C6199",
                        "count": 3,
                        "flagNum": 3
                    }
                ]
            }
        ]
    })
}

// ─── Tests fixture inline ────────────────────────────────────────────────────

#[test]
fn banner_count_total() {
    let cfg = parse_banner_config(&fixture());
    // 3 entrées dans le dump réel (m_tutorialBannerInfoList)
    assert_eq!(cfg.banners.len(), 3);
}

#[test]
fn banner_entree_0_id() {
    // m_tutorialBannerInfoList[0].id = "0x74AD78BE" — dump réel ligne 9
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[0].id, HashId(0x74AD78BE));
}

#[test]
fn banner_entree_0_title_text_id() {
    // m_tutorialBannerInfoList[0].titleTextId = "0x968619E7" — dump réel ligne 10
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[0].title_text_id, HashId(0x968619E7));
}

#[test]
fn banner_entree_0_explain_text_id() {
    // m_tutorialBannerInfoList[0].explainTextId = "0x968619E7" — dump réel ligne 11
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[0].explain_text_id, HashId(0x968619E7));
}

#[test]
fn banner_entree_0_icon_texture_name() {
    // m_tutorialBannerInfoList[0].iconTextureName = "0xCF0200B5" — dump réel ligne 12
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[0].icon_texture_name, HashId(0xCF0200B5));
}

#[test]
fn banner_entree_0_count() {
    // m_tutorialBannerInfoList[0].count = 1 — dump réel ligne 13
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[0].count, 1);
}

#[test]
fn banner_entree_0_flag_num() {
    // m_tutorialBannerInfoList[0].flagNum = 1 — dump réel ligne 14
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[0].flag_num, 1);
}

#[test]
fn banner_entree_1_id() {
    // m_tutorialBannerInfoList[1].id = "0xEDA42904" — dump réel ligne 16
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[1].id, HashId(0xEDA42904));
}

#[test]
fn banner_entree_1_icon_texture_name() {
    // m_tutorialBannerInfoList[1].iconTextureName = "0x560B510F" — dump réel ligne 19
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[1].icon_texture_name, HashId(0x560B_510F));
}

#[test]
fn banner_entree_1_count() {
    // m_tutorialBannerInfoList[1].count = 2 — dump réel ligne 20
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[1].count, 2);
}

#[test]
fn banner_entree_1_flag_num() {
    // m_tutorialBannerInfoList[1].flagNum = 2 — dump réel ligne 21
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[1].flag_num, 2);
}

#[test]
fn banner_entree_2_id() {
    // m_tutorialBannerInfoList[2].id = "0x9AA31992" — dump réel ligne 23
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[2].id, HashId(0x9AA31992));
}

#[test]
fn banner_entree_2_icon_texture_name() {
    // m_tutorialBannerInfoList[2].iconTextureName = "0x210C6199" — dump réel ligne 26
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[2].icon_texture_name, HashId(0x210C6199));
}

#[test]
fn banner_entree_2_count() {
    // m_tutorialBannerInfoList[2].count = 3 — dump réel ligne 27
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[2].count, 3);
}

#[test]
fn banner_entree_2_flag_num() {
    // m_tutorialBannerInfoList[2].flagNum = 3 — dump réel ligne 28
    let cfg = parse_banner_config(&fixture());
    assert_eq!(cfg.banners[2].flag_num, 3);
}

#[test]
fn banner_find_by_id_trouve() {
    // find_by_id doit retrouver la bannière 1 par son hash
    let cfg = parse_banner_config(&fixture());
    let found = cfg.find_by_id(HashId(0xEDA42904));
    assert!(found.is_some());
    assert_eq!(found.unwrap().count, 2);
}

#[test]
fn banner_find_by_id_absent() {
    // find_by_id renvoie None pour un hash inconnu
    let cfg = parse_banner_config(&fixture());
    assert!(cfg.find_by_id(HashId(0xDEADBEEF)).is_none());
}

#[test]
fn banner_entree_invalide_ignoree() {
    // Une entrée sans champ `id` doit être silencieusement ignorée
    let json = json!({
        "version": 100,
        "liste_vide": [],
        "lists": [{
            "name": "m_tutorialBannerInfoList",
            "typeName": "TUTORIAL_BANNER_INFO",
            "values": [
                { "count": 1, "flagNum": 1 },  // pas de id -> ignoré
                { "id": "0x74AD78BE", "count": 1, "flagNum": 1 }
            ]
        }]
    });
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners.len(), 1);
}

#[test]
fn banner_liste_absente_donne_vide() {
    // Un JSON sans la liste attendue renvoie un BannerConfig vide
    let json = json!({ "version": 100, "lists": [] });
    let cfg = parse_banner_config(&json);
    assert!(cfg.banners.is_empty());
}

// ─── Tests sur le vrai fichier (skip silencieux si absent) ───────────────────

const CHEMIN_REEL: &str = "banner/tutorial_banner_config_0.00.00.cfg.bin.json";

/// Charge le dump réel si présent, sinon renvoie `None` (skip silencieux).
fn charger_reel() -> Option<serde_json::Value> {
    let contenu = std::fs::read_to_string(CHEMIN_REEL).ok()?;
    serde_json::from_str(&contenu).ok()
}

#[test]
fn real_file_banner_count() {
    // Le dump réel contient exactement 3 bannières tutoriel.
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(
        cfg.banners.len(),
        3,
        "m_tutorialBannerInfoList doit avoir 3 entrées"
    );
}

#[test]
fn real_file_banner_0_id() {
    // m_tutorialBannerInfoList[0].id = "0x74AD78BE" (dump réel ligne 9)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[0].id, HashId(0x74AD78BE));
}

#[test]
fn real_file_banner_0_title_text_id() {
    // m_tutorialBannerInfoList[0].titleTextId = "0x968619E7" (dump réel ligne 10)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[0].title_text_id, HashId(0x968619E7));
}

#[test]
fn real_file_banner_0_explain_text_id() {
    // m_tutorialBannerInfoList[0].explainTextId = "0x968619E7" (dump réel ligne 11)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[0].explain_text_id, HashId(0x968619E7));
}

#[test]
fn real_file_banner_0_icon_texture_name() {
    // m_tutorialBannerInfoList[0].iconTextureName = "0xCF0200B5" (dump réel ligne 12)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[0].icon_texture_name, HashId(0xCF0200B5));
}

#[test]
fn real_file_banner_0_count() {
    // m_tutorialBannerInfoList[0].count = 1 (dump réel ligne 13)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[0].count, 1);
}

#[test]
fn real_file_banner_0_flag_num() {
    // m_tutorialBannerInfoList[0].flagNum = 1 (dump réel ligne 14)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[0].flag_num, 1);
}

#[test]
fn real_file_banner_1_id() {
    // m_tutorialBannerInfoList[1].id = "0xEDA42904" (dump réel ligne 16)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[1].id, HashId(0xEDA42904));
}

#[test]
fn real_file_banner_1_icon_texture_name() {
    // m_tutorialBannerInfoList[1].iconTextureName = "0x560B510F" (dump réel ligne 19)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[1].icon_texture_name, HashId(0x560B510F));
}

#[test]
fn real_file_banner_1_count() {
    // m_tutorialBannerInfoList[1].count = 2 (dump réel ligne 20)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[1].count, 2);
}

#[test]
fn real_file_banner_1_flag_num() {
    // m_tutorialBannerInfoList[1].flagNum = 2 (dump réel ligne 21)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[1].flag_num, 2);
}

#[test]
fn real_file_banner_2_id() {
    // m_tutorialBannerInfoList[2].id = "0x9AA31992" (dump réel ligne 23)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[2].id, HashId(0x9AA31992));
}

#[test]
fn real_file_banner_2_icon_texture_name() {
    // m_tutorialBannerInfoList[2].iconTextureName = "0x210C6199" (dump réel ligne 26)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[2].icon_texture_name, HashId(0x210C6199));
}

#[test]
fn real_file_banner_2_count() {
    // m_tutorialBannerInfoList[2].count = 3 (dump réel ligne 27)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[2].count, 3);
}

#[test]
fn real_file_banner_2_flag_num() {
    // m_tutorialBannerInfoList[2].flagNum = 3 (dump réel ligne 28)
    let Some(json) = charger_reel() else { return };
    let cfg = parse_banner_config(&json);
    assert_eq!(cfg.banners[2].flag_num, 3);
}
