#![allow(clippy::pedantic)]
//! Tests golden `chronicle_top` — valeurs réelles tirées de :
//! `chronicle_top/chronicle_top_caravan_config.cfg.bin.json`
//!
//! ## Vérifications champ par champ (fichier:champ → valeur confirmée)
//!
//! ### m_chronicleTopCaravanInfoList[0] (id=1, moveType=0)
//! - `id` = 1
//! - `moveType` = 0
//! - `amplitudeMaxX` = 0.25  (JSON ligne 11 : `"amplitudeMaxX": 0.25`)
//! - `amplitudeEasingTypeX` = 0  (JSON ligne 12 : `"amplitudeEasingTypeX": 0`)
//! - `amplitudePeriodTimeX` = 4.0  (JSON ligne 13 : `"amplitudePeriodTimeX": 4`)
//! - `amplitudeStrengthX` = 0.0  (JSON ligne 14 : `"amplitudeStrengthX": 0`)
//! - `amplitudeMaxY` = 0.5  (JSON ligne 15 : `"amplitudeMaxY": 0.5`)
//! - `amplitudeEasingTypeY` = 1  (JSON ligne 16 : `"amplitudeEasingTypeY": 1`)
//! - `amplitudePeriodTimeY` = 2.0  (JSON ligne 17 : `"amplitudePeriodTimeY": 2`)
//! - `amplitudeStrengthY` = 0.0  (JSON ligne 18 : `"amplitudeStrengthY": 0`)
//! - `rotationMaxZ` = 5.0  (JSON ligne 31 : `"rotationMaxZ": 5`)
//! - `rotationEasingTypeZ` = 1  (JSON ligne 32 : `"rotationEasingTypeZ": 1`)
//! - `rotationPeriodTimeZ` = 4.0  (JSON ligne 33 : `"rotationPeriodTimeZ": 4`)
//! - `rotationStrengthZ` = 0.0  (JSON ligne 34 : `"rotationStrengthZ": 0`)
//!
//! ### m_chronicleTopCaravanInfoList[1] (id=2, moveType=1)
//! - `moveType` = 1
//! - `amplitudeStrengthY` = 2.0  (JSON ligne 46 : `"amplitudeStrengthY": 2`)
//! - `rotationMaxZ` = 0.0  (JSON ligne 59 : `"rotationMaxZ": 0`)
//!
//! ### m_chronicleTopCaravanInfoList[3] (id=4, moveType=1)
//! - `amplitudeMaxX` = 0.25  (JSON ligne 95 : `"amplitudeMaxX": 0.25`)
//! - `amplitudeEasingTypeX` = 1  (JSON ligne 96 : `"amplitudeEasingTypeX": 1`)
//! - `amplitudeStrengthX` = 2.0  (JSON ligne 98 : `"amplitudeStrengthX": 2`)
//! - `rotationMaxY` = 1.0  (JSON ligne 111 : `"rotationMaxY": 1`)
//! - `rotationStrengthY` = 5.0  (JSON ligne 114 : `"rotationStrengthY": 5`)
//!
//! ### m_chronicleTopCaravanInfoList[4] (id=5, moveType=1) — animation la plus complète
//! - `amplitudeStrengthX` = 1.0  (JSON ligne 127 : `"amplitudeStrengthX": 1`)
//! - `amplitudeStrengthY` = 0.5  (JSON ligne 130 : `"amplitudeStrengthY": 0.5`)
//! - `rotationMaxY` = 0.5  (JSON ligne 139 : `"rotationMaxY": 0.5`)
//! - `rotationStrengthY` = 2.5  (JSON ligne 142 : `"rotationStrengthY": 2.5`)
//! - `rotationMaxZ` = 5.0  (JSON ligne 143 : `"rotationMaxZ": 5`)

mod common;

use nie_data::chronicle_top::{ChronicleTopCaravanConfig, parse_chronicle_top_caravan_config};
use serde_json::json;

// ─── Fixture ──────────────────────────────────────────────────────────────────
// Valeurs extraites octet-pour-octet du dump réel — voir commentaires ci-dessus.

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_chronicleTopCaravanInfoList",
                "typeName": "CHRONICLE_TOP_CARAVAN_INFO",
                "values": [
                    // entrée 0 (id=1, moveType=0) — dump réel lignes 8-35
                    {
                        "id": 1,
                        "moveType": 0,
                        "amplitudeMaxX": 0.25,
                        "amplitudeEasingTypeX": 0,
                        "amplitudePeriodTimeX": 4,
                        "amplitudeStrengthX": 0,
                        "amplitudeMaxY": 0.5,
                        "amplitudeEasingTypeY": 1,
                        "amplitudePeriodTimeY": 2,
                        "amplitudeStrengthY": 0,
                        "amplitudeMaxZ": 0,
                        "amplitudeEasingTypeZ": 0,
                        "amplitudePeriodTimeZ": 0,
                        "amplitudeStrengthZ": 0,
                        "rotationMaxX": 0,
                        "rotationEasingTypeX": 0,
                        "rotationPeriodTimeX": 0,
                        "rotationStrengthX": 0,
                        "rotationMaxY": 0,
                        "rotationEasingTypeY": 0,
                        "rotationPeriodTimeY": 0,
                        "rotationStrengthY": 0,
                        "rotationMaxZ": 5,
                        "rotationEasingTypeZ": 1,
                        "rotationPeriodTimeZ": 4,
                        "rotationStrengthZ": 0
                    },
                    // entrée 1 (id=2, moveType=1) — dump réel lignes 36-63
                    {
                        "id": 2,
                        "moveType": 1,
                        "amplitudeMaxX": 0,
                        "amplitudeEasingTypeX": 0,
                        "amplitudePeriodTimeX": 0,
                        "amplitudeStrengthX": 0,
                        "amplitudeMaxY": 0.5,
                        "amplitudeEasingTypeY": 1,
                        "amplitudePeriodTimeY": 2,
                        "amplitudeStrengthY": 2,
                        "amplitudeMaxZ": 0,
                        "amplitudeEasingTypeZ": 0,
                        "amplitudePeriodTimeZ": 0,
                        "amplitudeStrengthZ": 0,
                        "rotationMaxX": 0,
                        "rotationEasingTypeX": 0,
                        "rotationPeriodTimeX": 0,
                        "rotationStrengthX": 0,
                        "rotationMaxY": 0,
                        "rotationEasingTypeY": 0,
                        "rotationPeriodTimeY": 0,
                        "rotationStrengthY": 0,
                        "rotationMaxZ": 0,
                        "rotationEasingTypeZ": 0,
                        "rotationPeriodTimeZ": 0,
                        "rotationStrengthZ": 0
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn parse_renvoie_deux_entrees() {
    let cfg = parse_chronicle_top_caravan_config(&fixture());
    // La fixture contient 2 entrées valides.
    assert_eq!(cfg.caravans.len(), 2);
}

#[test]
fn entree0_id_et_move_type() {
    let cfg = parse_chronicle_top_caravan_config(&fixture());
    let e0 = &cfg.caravans[0];

    // chronicle_top_caravan_config.cfg.bin.json : m_chronicleTopCaravanInfoList[0] (lignes 8-9)
    assert_eq!(e0.id, 1, "id=1");
    assert_eq!(e0.move_type, 0, "moveType=0 (statique)");
}

#[test]
fn entree0_amplitude_x() {
    let cfg = parse_chronicle_top_caravan_config(&fixture());
    let e0 = &cfg.caravans[0];

    // lignes 11-14 du dump réel
    assert_eq!(e0.amplitude_max_x, 0.25_f32, "amplitudeMaxX=0.25");
    assert_eq!(e0.amplitude_easing_type_x, 0, "amplitudeEasingTypeX=0");
    assert_eq!(
        e0.amplitude_period_time_x, 4.0_f32,
        "amplitudePeriodTimeX=4"
    );
    assert_eq!(e0.amplitude_strength_x, 0.0_f32, "amplitudeStrengthX=0");
}

#[test]
fn entree0_amplitude_y() {
    let cfg = parse_chronicle_top_caravan_config(&fixture());
    let e0 = &cfg.caravans[0];

    // lignes 15-18 du dump réel
    assert_eq!(e0.amplitude_max_y, 0.5_f32, "amplitudeMaxY=0.5");
    assert_eq!(e0.amplitude_easing_type_y, 1, "amplitudeEasingTypeY=1");
    assert_eq!(
        e0.amplitude_period_time_y, 2.0_f32,
        "amplitudePeriodTimeY=2"
    );
    assert_eq!(e0.amplitude_strength_y, 0.0_f32, "amplitudeStrengthY=0");
}

#[test]
fn entree0_amplitude_z_nul() {
    let cfg = parse_chronicle_top_caravan_config(&fixture());
    let e0 = &cfg.caravans[0];

    // lignes 19-22 du dump réel : tout à 0
    assert_eq!(e0.amplitude_max_z, 0.0_f32);
    assert_eq!(e0.amplitude_easing_type_z, 0);
    assert_eq!(e0.amplitude_period_time_z, 0.0_f32);
    assert_eq!(e0.amplitude_strength_z, 0.0_f32);
}

#[test]
fn entree0_rotation_xy_nuls() {
    let cfg = parse_chronicle_top_caravan_config(&fixture());
    let e0 = &cfg.caravans[0];

    // lignes 23-30 du dump réel : X et Y à 0
    assert_eq!(e0.rotation_max_x, 0.0_f32);
    assert_eq!(e0.rotation_easing_type_x, 0);
    assert_eq!(e0.rotation_max_y, 0.0_f32);
    assert_eq!(e0.rotation_easing_type_y, 0);
}

#[test]
fn entree0_rotation_z() {
    let cfg = parse_chronicle_top_caravan_config(&fixture());
    let e0 = &cfg.caravans[0];

    // lignes 31-34 du dump réel
    assert_eq!(e0.rotation_max_z, 5.0_f32, "rotationMaxZ=5");
    assert_eq!(e0.rotation_easing_type_z, 1, "rotationEasingTypeZ=1");
    assert_eq!(e0.rotation_period_time_z, 4.0_f32, "rotationPeriodTimeZ=4");
    assert_eq!(e0.rotation_strength_z, 0.0_f32, "rotationStrengthZ=0");
}

#[test]
fn entree1_move_type_et_strength_y() {
    let cfg = parse_chronicle_top_caravan_config(&fixture());
    let e1 = &cfg.caravans[1];

    // lignes 36-63 du dump réel
    assert_eq!(e1.id, 2, "id=2");
    assert_eq!(e1.move_type, 1, "moveType=1 (dynamique)");
    assert_eq!(e1.amplitude_strength_y, 2.0_f32, "amplitudeStrengthY=2");
    assert_eq!(e1.rotation_max_z, 0.0_f32, "rotationMaxZ=0 pour id=2");
}

#[test]
fn entree_sans_id_ignoree() {
    let root = json!({
        "version": 100,
        "lists": [{
            "name": "m_chronicleTopCaravanInfoList",
            "typeName": "CHRONICLE_TOP_CARAVAN_INFO",
            "values": [
                { "id": 1, "moveType": 0 },
                { "moveType": 1 }
            ]
        }]
    });
    let cfg = parse_chronicle_top_caravan_config(&root);
    // L'entrée sans "id" est ignorée.
    assert_eq!(cfg.caravans.len(), 1);
    assert_eq!(cfg.caravans[0].id, 1);
}

#[test]
fn find_by_id() {
    let cfg = parse_chronicle_top_caravan_config(&fixture());
    assert!(cfg.find_by_id(1).is_some(), "id=1 présent");
    assert!(cfg.find_by_id(2).is_some(), "id=2 présent");
    assert!(cfg.find_by_id(99).is_none(), "id=99 absent");
}

#[test]
fn liste_absente_renvoie_vide() {
    let root = json!({ "version": 100, "lists": [] });
    let cfg = parse_chronicle_top_caravan_config(&root);
    assert!(cfg.caravans.is_empty());
}

// ─── Tests sur le vrai fichier (skip si absent du VPS) ───────────────────────

const REAL_PATH: &str = "chronicle_top/chronicle_top_caravan_config.cfg.bin.json";

fn load_real() -> Option<ChronicleTopCaravanConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_chronicle_top_caravan_config(&root))
}

#[test]
fn real_file_compte_entrees() {
    let Some(cfg) = load_real() else { return };

    // chronicle_top_caravan_config.cfg.bin.json : 5 entrées dans m_chronicleTopCaravanInfoList
    assert_eq!(
        cfg.caravans.len(),
        5,
        "5 entrées CHRONICLE_TOP_CARAVAN_INFO"
    );
}

#[test]
fn real_file_entree0_champs_scalaires() {
    let Some(cfg) = load_real() else { return };

    // m_chronicleTopCaravanInfoList[0] (lignes 8-35 du dump réel)
    let e0 = &cfg.caravans[0];
    assert_eq!(e0.id, 1, "id=1");
    assert_eq!(e0.move_type, 0, "moveType=0");
    assert_eq!(
        e0.amplitude_max_x, 0.25_f32,
        "amplitudeMaxX=0.25 (ligne 11)"
    );
    assert_eq!(
        e0.amplitude_easing_type_x, 0,
        "amplitudeEasingTypeX=0 (ligne 12)"
    );
    assert_eq!(
        e0.amplitude_period_time_x, 4.0_f32,
        "amplitudePeriodTimeX=4 (ligne 13)"
    );
    assert_eq!(
        e0.amplitude_strength_x, 0.0_f32,
        "amplitudeStrengthX=0 (ligne 14)"
    );
    assert_eq!(e0.amplitude_max_y, 0.5_f32, "amplitudeMaxY=0.5 (ligne 15)");
    assert_eq!(
        e0.amplitude_easing_type_y, 1,
        "amplitudeEasingTypeY=1 (ligne 16)"
    );
    assert_eq!(
        e0.amplitude_period_time_y, 2.0_f32,
        "amplitudePeriodTimeY=2 (ligne 17)"
    );
    assert_eq!(
        e0.amplitude_strength_y, 0.0_f32,
        "amplitudeStrengthY=0 (ligne 18)"
    );
    assert_eq!(e0.rotation_max_z, 5.0_f32, "rotationMaxZ=5 (ligne 31)");
    assert_eq!(
        e0.rotation_easing_type_z, 1,
        "rotationEasingTypeZ=1 (ligne 32)"
    );
    assert_eq!(
        e0.rotation_period_time_z, 4.0_f32,
        "rotationPeriodTimeZ=4 (ligne 33)"
    );
    assert_eq!(
        e0.rotation_strength_z, 0.0_f32,
        "rotationStrengthZ=0 (ligne 34)"
    );
}

#[test]
fn real_file_entree1_strength_y() {
    let Some(cfg) = load_real() else { return };

    // m_chronicleTopCaravanInfoList[1] (lignes 36-63)
    let e1 = &cfg.caravans[1];
    assert_eq!(e1.id, 2, "id=2");
    assert_eq!(e1.move_type, 1, "moveType=1 (ligne 37)");
    assert_eq!(
        e1.amplitude_strength_y, 2.0_f32,
        "amplitudeStrengthY=2 (ligne 46)"
    );
    assert_eq!(e1.rotation_max_z, 0.0_f32, "rotationMaxZ=0 (ligne 59)");
}

#[test]
fn real_file_entree3_amplitude_x_easing1() {
    let Some(cfg) = load_real() else { return };

    // m_chronicleTopCaravanInfoList[3] (lignes 92-119) — id=4
    let e3 = &cfg.caravans[3];
    assert_eq!(e3.id, 4, "id=4");
    assert_eq!(
        e3.amplitude_max_x, 0.25_f32,
        "amplitudeMaxX=0.25 (ligne 95)"
    );
    assert_eq!(
        e3.amplitude_easing_type_x, 1,
        "amplitudeEasingTypeX=1 (ligne 96)"
    );
    assert_eq!(
        e3.amplitude_strength_x, 2.0_f32,
        "amplitudeStrengthX=2 (ligne 98)"
    );
    assert_eq!(e3.rotation_max_y, 1.0_f32, "rotationMaxY=1 (ligne 111)");
    assert_eq!(
        e3.rotation_strength_y, 5.0_f32,
        "rotationStrengthY=5 (ligne 114)"
    );
    assert_eq!(e3.rotation_max_z, 5.0_f32, "rotationMaxZ=5 (ligne 115)");
}

#[test]
fn real_file_entree4_animation_complete() {
    let Some(cfg) = load_real() else { return };

    // m_chronicleTopCaravanInfoList[4] (lignes 120-147) — id=5, animation la plus complète
    let e4 = &cfg.caravans[4];
    assert_eq!(e4.id, 5, "id=5");
    assert_eq!(e4.move_type, 1, "moveType=1 (ligne 122)");
    assert_eq!(
        e4.amplitude_strength_x, 1.0_f32,
        "amplitudeStrengthX=1 (ligne 127)"
    );
    assert_eq!(
        e4.amplitude_strength_y, 0.5_f32,
        "amplitudeStrengthY=0.5 (ligne 130)"
    );
    assert_eq!(e4.rotation_max_y, 0.5_f32, "rotationMaxY=0.5 (ligne 139)");
    assert_eq!(
        e4.rotation_easing_type_y, 1,
        "rotationEasingTypeY=1 (ligne 140)"
    );
    assert_eq!(
        e4.rotation_period_time_y, 2.0_f32,
        "rotationPeriodTimeY=2 (ligne 141)"
    );
    assert_eq!(
        e4.rotation_strength_y, 2.5_f32,
        "rotationStrengthY=2.5 (ligne 142)"
    );
    assert_eq!(e4.rotation_max_z, 5.0_f32, "rotationMaxZ=5 (ligne 143)");
    assert_eq!(
        e4.rotation_easing_type_z, 1,
        "rotationEasingTypeZ=1 (ligne 144)"
    );
    assert_eq!(
        e4.rotation_period_time_z, 4.0_f32,
        "rotationPeriodTimeZ=4 (ligne 145)"
    );
}

#[test]
fn real_file_find_by_id() {
    let Some(cfg) = load_real() else { return };

    // Tous les id 1-5 doivent être trouvables.
    for id in 1_i64..=5 {
        assert!(
            cfg.find_by_id(id).is_some(),
            "find_by_id({id}) doit réussir"
        );
    }
    assert!(cfg.find_by_id(0).is_none(), "id=0 absent");
    assert!(cfg.find_by_id(6).is_none(), "id=6 absent");
}
