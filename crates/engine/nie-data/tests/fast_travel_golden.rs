#![allow(clippy::pedantic)]
//! Tests golden `fast_travel` — valeurs réelles tirées de :
//! `fast_travel/fast_travel_config_0.00.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ (fichier:ligne → valeur confirmée)
//!
//! ### m_fastTravelMapInfoList[0]
//! - `id`           = `"0x7225AA02"`  (JSON ligne 9)
//! - `mapId`        = `"0x4E7B90D9"`  (JSON ligne 10)
//! - `mapTextId`    = `"0xD20ECFA3"`  (JSON ligne 11)
//! - `pos`          = `[0, 0, 45, 0]` (JSON lignes 12-17)
//! - `charaRotateY` = `-180`          (JSON ligne 18)
//!
//! ### m_fastTravelMapInfoList[1]
//! - `id`           = `"0x05229A94"`  (JSON ligne 22)
//! - `mapId`        = `"0x4E7B90D9"`  (JSON ligne 23)
//! - `mapTextId`    = `"0xA509FF35"`  (JSON ligne 24)
//! - `pos`          = `[0, 0, -69, 0]` (JSON lignes 25-30)
//! - `charaRotateY` = `0`             (JSON ligne 31)
//!
//! ### m_fastTravelMapInfoList[2]
//! - `id`           = `"0x9C2BCB2E"`  (JSON ligne 35)
//! - `mapId`        = `"0x4E7B90D9"`  (JSON ligne 36)
//! - `pos`          = `[-51.94, 0, -19.47, 0]` (JSON lignes 37-42)
//! - `charaRotateY` = `50`            (JSON ligne 43)
//!
//! ### m_fastTravelMapInfoList[3]
//! - `id`           = `"0xEB2CFBB8"`  (JSON ligne 47)
//! - `pos`          = `[64.53, 0, -193.64, 0]` (JSON lignes 49-54)
//! - `charaRotateY` = `90`            (JSON ligne 55)
//!
//! ### m_fastTravelMapInfoList[4]
//! - `id`           = `"0x6B3E9B43"`  (JSON ligne 59)
//! - `mapId`        = `"0xD772C163"`  (JSON ligne 60) — carte différente des entrées 0–3
//! - `pos`          = `[0, 0, -30, 0]` (JSON lignes 62-67)
//! - `charaRotateY` = `0`             (JSON ligne 68)
//!
//! ### m_fastTravelMapInfoList[5]
//! - `id`           = `"0xA3E7114B"`  (JSON ligne 71)
//! - `mapId`        = `"0x2067910E"`  (JSON ligne 72)
//! - `mapTextId`    = `"0xF15D1497"`  (JSON ligne 73)
//! - `pos`          = `[0, 0, 0, 0]`  (JSON lignes 74-79) — position nulle
//! - `charaRotateY` = `0`             (JSON ligne 80)

mod common;

use nie_data::fast_travel::{FastTravelConfig, parse_fast_travel_config};
use nie_data::hash::HashId;
use serde_json::json;

// ─── Fixture inline ───────────────────────────────────────────────────────────
// Valeurs extraites octet-pour-octet du dump réel — voir commentaires ci-dessus.

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_fastTravelMapInfoList",
                "typeName": "FAST_TRAVEL_MAP_INFO",
                "values": [
                    // entrée 0 — dump réel lignes 8-19
                    {
                        "id": "0x7225AA02",
                        "mapId": "0x4E7B90D9",
                        "mapTextId": "0xD20ECFA3",
                        "pos": [0, 0, 45, 0],
                        "charaRotateY": -180
                    },
                    // entrée 1 — dump réel lignes 20-32
                    {
                        "id": "0x05229A94",
                        "mapId": "0x4E7B90D9",
                        "mapTextId": "0xA509FF35",
                        "pos": [0, 0, -69, 0],
                        "charaRotateY": 0
                    },
                    // entrée 2 — dump réel lignes 33-44
                    {
                        "id": "0x9C2BCB2E",
                        "mapId": "0x4E7B90D9",
                        "mapTextId": "0x3C00AE8F",
                        "pos": [-51.94, 0, -19.47, 0],
                        "charaRotateY": 50
                    },
                    // entrée 3 — dump réel lignes 45-56
                    {
                        "id": "0xEB2CFBB8",
                        "mapId": "0x4E7B90D9",
                        "mapTextId": "0x4B079E19",
                        "pos": [64.53, 0, -193.64, 0],
                        "charaRotateY": 90
                    },
                    // entrée 4 — dump réel lignes 57-69
                    {
                        "id": "0x6B3E9B43",
                        "mapId": "0xD772C163",
                        "mapTextId": "0x2E4427EC",
                        "pos": [0, 0, -30, 0],
                        "charaRotateY": 0
                    },
                    // entrée 5 — dump réel lignes 70-81
                    {
                        "id": "0xA3E7114B",
                        "mapId": "0x2067910E",
                        "mapTextId": "0xF15D1497",
                        "pos": [0, 0, 0, 0],
                        "charaRotateY": 0
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn fixture_compte_entrees() {
    let cfg = parse_fast_travel_config(&fixture());
    // fast_travel_config_0.00.00 : m_fastTravelMapInfoList contient 6 entrées
    assert_eq!(
        cfg.map_infos.len(),
        6,
        "6 points de voyage rapide dans ce dump"
    );
}

#[test]
fn entree0_champs_scalaires() {
    let cfg = parse_fast_travel_config(&fixture());
    let e = &cfg.map_infos[0];

    // fast_travel_config_0.00.00 : m_fastTravelMapInfoList[0] (lignes 8-19)
    assert_eq!(e.id, HashId(0x7225_AA02), "id entrée 0");
    assert_eq!(e.map_id, HashId(0x4E7B_90D9), "mapId entrée 0");
    assert_eq!(e.map_text_id, HashId(0xD20E_CFA3), "mapTextId entrée 0");
    assert_eq!(
        e.chara_rotate_y, -180.0_f32,
        "charaRotateY entrée 0 = -180°"
    );
}

#[test]
fn entree0_pos() {
    let cfg = parse_fast_travel_config(&fixture());
    let e = &cfg.map_infos[0];

    // pos = [0, 0, 45, 0] — JSON lignes 12-17
    assert_eq!(e.pos[0], 0.0_f32, "pos.x = 0");
    assert_eq!(e.pos[1], 0.0_f32, "pos.y = 0");
    assert_eq!(e.pos[2], 45.0_f32, "pos.z = 45");
    assert_eq!(e.pos[3], 0.0_f32, "pos.w = 0 (padding)");
}

#[test]
fn entree1_champs_scalaires() {
    let cfg = parse_fast_travel_config(&fixture());
    let e = &cfg.map_infos[1];

    // fast_travel_config_0.00.00 : m_fastTravelMapInfoList[1] (lignes 20-32)
    assert_eq!(e.id, HashId(0x0522_9A94), "id entrée 1");
    assert_eq!(
        e.map_id,
        HashId(0x4E7B_90D9),
        "mapId entrée 1 = même zone qu'entrée 0"
    );
    assert_eq!(e.map_text_id, HashId(0xA509_FF35), "mapTextId entrée 1");
    assert_eq!(e.chara_rotate_y, 0.0_f32, "charaRotateY entrée 1 = 0°");
}

#[test]
fn entree1_pos_z_negatif() {
    let cfg = parse_fast_travel_config(&fixture());
    let e = &cfg.map_infos[1];

    // pos = [0, 0, -69, 0] — JSON lignes 25-30
    assert_eq!(e.pos[2], -69.0_f32, "pos.z = -69");
}

#[test]
fn entree2_pos_flottants() {
    let cfg = parse_fast_travel_config(&fixture());
    let e = &cfg.map_infos[2];

    // pos = [-51.94, 0, -19.47, 0] — JSON lignes 37-42
    // Valeurs non exactement représentables en f32 : tolérance 1e-3.
    assert!(
        (e.pos[0] - (-51.94_f32)).abs() < 1e-3,
        "pos.x ≈ -51.94 (JSON ligne 38)"
    );
    assert_eq!(e.pos[1], 0.0_f32, "pos.y = 0");
    assert!(
        (e.pos[2] - (-19.47_f32)).abs() < 1e-3,
        "pos.z ≈ -19.47 (JSON ligne 40)"
    );
    assert_eq!(e.pos[3], 0.0_f32, "pos.w = 0");
    // charaRotateY = 50 (JSON ligne 43)
    assert_eq!(e.chara_rotate_y, 50.0_f32, "charaRotateY entrée 2 = 50°");
}

#[test]
fn entree3_pos_et_rotation() {
    let cfg = parse_fast_travel_config(&fixture());
    let e = &cfg.map_infos[3];

    // id = 0xEB2CFBB8 (JSON ligne 47)
    assert_eq!(e.id, HashId(0xEB2C_FBB8), "id entrée 3");
    // pos = [64.53, 0, -193.64, 0] — JSON lignes 49-54
    assert!((e.pos[0] - 64.53_f32).abs() < 1e-3, "pos.x ≈ 64.53");
    assert!((e.pos[2] - (-193.64_f32)).abs() < 1e-2, "pos.z ≈ -193.64");
    // charaRotateY = 90 (JSON ligne 55)
    assert_eq!(e.chara_rotate_y, 90.0_f32, "charaRotateY entrée 3 = 90°");
}

#[test]
fn entree4_carte_differente() {
    let cfg = parse_fast_travel_config(&fixture());
    let e = &cfg.map_infos[4];

    // mapId = 0xD772C163 (JSON ligne 60) — carte distincte des entrées 0-3
    assert_eq!(e.id, HashId(0x6B3E_9B43), "id entrée 4");
    assert_eq!(
        e.map_id,
        HashId(0xD772_C163),
        "mapId entrée 4 = carte différente"
    );
    assert_eq!(e.pos[2], -30.0_f32, "pos.z = -30");
}

#[test]
fn entree5_position_nulle() {
    let cfg = parse_fast_travel_config(&fixture());
    let e = &cfg.map_infos[5];

    // id = 0xA3E7114B (JSON ligne 71), mapId = 0x2067910E (JSON ligne 72)
    assert_eq!(e.id, HashId(0xA3E7_114B), "id entrée 5");
    assert_eq!(
        e.map_id,
        HashId(0x2067_910E),
        "mapId entrée 5 = hub principal ?"
    );
    assert_eq!(e.map_text_id, HashId(0xF15D_1497), "mapTextId entrée 5");
    // pos = [0, 0, 0, 0] — position nulle (JSON lignes 74-79)
    assert_eq!(e.pos, [0.0_f32; 4], "pos entrée 5 = [0, 0, 0, 0]");
    assert_eq!(e.chara_rotate_y, 0.0_f32, "charaRotateY entrée 5 = 0°");
}

#[test]
fn find_by_id_present() {
    let cfg = parse_fast_travel_config(&fixture());
    // 0x7225AA02 = entrée 0 (JSON ligne 9)
    let found = cfg.find_by_id(HashId(0x7225_AA02));
    assert!(found.is_some(), "find_by_id doit trouver l'entrée 0");
    assert_eq!(found.unwrap().map_id, HashId(0x4E7B_90D9));
}

#[test]
fn find_by_id_absent() {
    let cfg = parse_fast_travel_config(&fixture());
    assert!(
        cfg.find_by_id(HashId(0xDEAD_BEEF)).is_none(),
        "hash inexistant → None"
    );
}

#[test]
fn find_by_map_quatre_entrees() {
    let cfg = parse_fast_travel_config(&fixture());
    // mapId = 0x4E7B90D9 → entrées 0, 1, 2, 3 partagent cette carte (JSON lignes 10, 23, 36, 48)
    let pts = cfg.find_by_map(HashId(0x4E7B_90D9));
    assert_eq!(pts.len(), 4, "4 points sur la même carte 0x4E7B90D9");
}

#[test]
fn find_by_map_carte_unique() {
    let cfg = parse_fast_travel_config(&fixture());
    // mapId = 0xD772C163 → seulement l'entrée 4 (JSON ligne 60)
    let pts = cfg.find_by_map(HashId(0xD772_C163));
    assert_eq!(pts.len(), 1, "1 seul point sur la carte 0xD772C163");
    assert_eq!(pts[0].id, HashId(0x6B3E_9B43));
}

#[test]
fn find_by_map_absent() {
    let cfg = parse_fast_travel_config(&fixture());
    assert_eq!(cfg.find_by_map(HashId(0xDEAD_BEEF)).len(), 0);
}

#[test]
fn json_vide_renvoie_config_vide() {
    let cfg = parse_fast_travel_config(&serde_json::Value::Object(Default::default()));
    assert_eq!(cfg.map_infos.len(), 0, "JSON vide → liste vide");
}

#[test]
fn entree_id_nul_ignoree() {
    let data = json!({
        "version": 100,
        "lists": [{
            "name": "m_fastTravelMapInfoList",
            "typeName": "FAST_TRAVEL_MAP_INFO",
            "values": [
                // entrée valide
                { "id": "0x7225AA02", "mapId": "0x4E7B90D9", "mapTextId": "0xD20ECFA3",
                  "pos": [0, 0, 45, 0], "charaRotateY": -180 },
                // entrée avec id nul → doit être ignorée
                { "id": "0x00000000", "mapId": "0x4E7B90D9", "mapTextId": "0xD20ECFA3",
                  "pos": [0, 0, 0, 0], "charaRotateY": 0 }
            ]
        }]
    });
    let cfg = parse_fast_travel_config(&data);
    assert_eq!(cfg.map_infos.len(), 1, "l'entrée à id nul est ignorée");
}

// ─── Tests sur le vrai fichier (skip silencieux si absent du VPS) ─────────────

const REAL_PATH: &str = "fast_travel/fast_travel_config_0.00.00.cfg.bin.json";

fn load_real() -> Option<FastTravelConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_fast_travel_config(&root))
}

#[test]
fn real_file_compte_total() {
    let Some(cfg) = load_real() else { return };
    // fast_travel_config_0.00.00.cfg.bin.json : 6 entrées dans m_fastTravelMapInfoList
    assert_eq!(
        cfg.map_infos.len(),
        6,
        "6 points de voyage rapide dans le dump réel"
    );
}

#[test]
fn real_file_entree0_id_et_rotation() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.map_infos[0];
    // m_fastTravelMapInfoList[0] : id = "0x7225AA02" (JSON ligne 9), charaRotateY = -180 (ligne 18)
    assert_eq!(e.id, HashId(0x7225_AA02), "entrée 0 id = 0x7225AA02");
    assert_eq!(
        e.chara_rotate_y, -180.0_f32,
        "entrée 0 charaRotateY = -180°"
    );
}

#[test]
fn real_file_entree0_map_ids() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.map_infos[0];
    // mapId = "0x4E7B90D9" (JSON ligne 10), mapTextId = "0xD20ECFA3" (JSON ligne 11)
    assert_eq!(e.map_id, HashId(0x4E7B_90D9), "entrée 0 mapId = 0x4E7B90D9");
    assert_eq!(
        e.map_text_id,
        HashId(0xD20E_CFA3),
        "entrée 0 mapTextId = 0xD20ECFA3"
    );
}

#[test]
fn real_file_entree0_pos() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.map_infos[0];
    // pos = [0, 0, 45, 0] (JSON lignes 12-17)
    assert_eq!(e.pos[0], 0.0_f32, "entrée 0 pos.x = 0");
    assert_eq!(e.pos[2], 45.0_f32, "entrée 0 pos.z = 45");
    assert_eq!(e.pos[3], 0.0_f32, "entrée 0 pos.w = 0 (padding)");
}

#[test]
fn real_file_entree2_pos_flottants() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.map_infos[2];
    // pos = [-51.94, 0, -19.47, 0] (JSON lignes 37-42)
    assert!(
        (e.pos[0] - (-51.94_f32)).abs() < 1e-3,
        "entrée 2 pos.x ≈ -51.94 (JSON ligne 38)"
    );
    assert!(
        (e.pos[2] - (-19.47_f32)).abs() < 1e-3,
        "entrée 2 pos.z ≈ -19.47 (JSON ligne 40)"
    );
    // charaRotateY = 50 (JSON ligne 43)
    assert_eq!(e.chara_rotate_y, 50.0_f32, "entrée 2 charaRotateY = 50°");
}

#[test]
fn real_file_entree3_pos_et_rotation() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.map_infos[3];
    // pos = [64.53, 0, -193.64, 0] (JSON lignes 49-54), charaRotateY = 90 (ligne 55)
    assert_eq!(
        e.id,
        HashId(0xEB2C_FBB8),
        "entrée 3 id = 0xEB2CFBB8 (JSON ligne 47)"
    );
    assert!(
        (e.pos[0] - 64.53_f32).abs() < 1e-3,
        "entrée 3 pos.x ≈ 64.53 (JSON ligne 50)"
    );
    assert!(
        (e.pos[2] - (-193.64_f32)).abs() < 1e-2,
        "entrée 3 pos.z ≈ -193.64 (JSON ligne 52)"
    );
    assert_eq!(
        e.chara_rotate_y, 90.0_f32,
        "entrée 3 charaRotateY = 90° (JSON ligne 55)"
    );
}

#[test]
fn real_file_entree4_carte_differente() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.map_infos[4];
    // mapId = "0xD772C163" (JSON ligne 60) — carte distincte des entrées 0-3
    assert_eq!(
        e.id,
        HashId(0x6B3E_9B43),
        "entrée 4 id = 0x6B3E9B43 (JSON ligne 59)"
    );
    assert_eq!(
        e.map_id,
        HashId(0xD772_C163),
        "entrée 4 mapId = 0xD772C163 (JSON ligne 60)"
    );
    assert_eq!(e.pos[2], -30.0_f32, "entrée 4 pos.z = -30 (JSON ligne 65)");
}

#[test]
fn real_file_entree5_position_nulle() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.map_infos[5];
    // id = "0xA3E7114B" (JSON ligne 71), mapId = "0x2067910E" (JSON ligne 72)
    assert_eq!(
        e.id,
        HashId(0xA3E7_114B),
        "entrée 5 id = 0xA3E7114B (JSON ligne 71)"
    );
    assert_eq!(
        e.map_id,
        HashId(0x2067_910E),
        "entrée 5 mapId = 0x2067910E (JSON ligne 72)"
    );
    assert_eq!(
        e.map_text_id,
        HashId(0xF15D_1497),
        "entrée 5 mapTextId = 0xF15D1497 (JSON ligne 73)"
    );
    assert_eq!(
        e.pos, [0.0_f32; 4],
        "entrée 5 pos = [0, 0, 0, 0] (JSON lignes 74-79)"
    );
}

#[test]
fn real_file_find_by_map_quatre_entrees() {
    let Some(cfg) = load_real() else { return };
    // 4 entrées partagent mapId = 0x4E7B90D9 (entrées 0-3, JSON lignes 10, 23, 36, 48)
    let pts = cfg.find_by_map(HashId(0x4E7B_90D9));
    assert_eq!(pts.len(), 4, "4 points sur la carte 0x4E7B90D9");
}
