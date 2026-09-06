#![allow(clippy::pedantic)]
//! Tests golden `formation` — valeurs réelles tirées de :
//! `formation/formation_config_0.02.16.cfg.bin.json`
//!
//! ## Vérifications champ par champ (fichier:champ → valeur confirmée)
//!
//! ### m_SoccerPositionInfoList[0] (positionId=1, type GK)
//! - `positionId` = 1
//! - `centerLineWight` = 0.5  (JSON : 0.5)
//! - `offenseLineWeight` = 0.05  (JSON : 0.05)
//! - `defenseLineWeight` = 0.95  (JSON : 0.95)
//!
//! ### m_SoccerFormPlacementInfoList[0] (GK, positionNo=0 de la formation 0)
//! - `positionNo` = 0, `positionId` = 1, `passNo` = 6
//! - `bKickoff` = false, `bFollow` = false
//! - `defensePos` = `"000000008FC2753F"` → Vec2 { x: 0.0, y: 0.9599... }
//!   (décodé byte-à-byte : f32 LE 0x3F75C28F ≈ 0.96)
//! - `offensePos` = `"00000000D7A3703F"` → Vec2 { x: 0.0, y: 0.9399... }
//!   (f32 LE 0x3F70A3D7 ≈ 0.94)
//! - `bustupPos` = `"0000000000008040"` → Vec2 { x: 0.0, y: 4.0 }
//!   (f32 LE 0x40800000 = 4.0 exactement)
//!
//! ### m_SoccerFormPlacementInfoList[8] (bKickoff=true)
//! - `bKickoff` = true — le slot 8 est le seul avec bKickoff=true dans la formation 0
//!
//! ### m_SoccerFormationInfoList[0]
//! - `formId` = 0x2CD9760B
//! - `placementInfo` = [0, 11] → offset=0, count=11
//! - `powerOffense` = 3, `powerDefense` = 3
//! - `nounId` = 0xB477A689, `descId` = 0x6B085B0E
//!
//! ### m_SoccerFormationInfoList[1]
//! - `formId` = 0xB5D027B1
//! - `powerOffense` = 4, `powerDefense` = 2
//!
//! ### m_SoccerCurvePointInfoList[0]
//! - `rate` = -1.0, `point` = -1.0
//!
//! ### m_SoccerLineCurveInfoList[0]
//! - `lineCurveId` = 0xBF81F7F9
//! - `curveInfo` = [0, 2] → offset=0, count=2

mod common;

use nie_data::formation::{
    FormationConfig, SoccerCurvePointInfo, SoccerFormPlacementInfo, SoccerFormationInfo,
    SoccerLineCurveInfo, SoccerPositionInfo, Vec2, parse_formation_config,
};
use nie_data::hash::HashId;
use serde_json::json;

// ─── Fixture ──────────────────────────────────────────────────────────────────
// Valeurs extraites octet-pour-octet du dump réel — voir commentaires ci-dessus.

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_SoccerPositionInfoList",
                "typeName": "SOCCER_POSITION_INFO",
                "values": [
                    // entrée 0 (positionId=1, GK type) — dump réel lignes 8-13
                    {
                        "positionId": 1,
                        "centerLineWight": 0.5,
                        "offenseLineWeight": 0.05,
                        "defenseLineWeight": 0.95
                    }
                ]
            },
            {
                "name": "m_SoccerFormPlacementInfoList",
                "typeName": "SOCCER_FORM_PLACEMENT_INFO",
                "values": [
                    // slot 0 (GK) — dump réel lignes 73-90
                    {
                        "defensePos": "000000008FC2753F",
                        "offensePos": "00000000D7A3703F",
                        "startPos":   "000000008FC2753F",
                        "ckDefenseLeftPos":  "000000008FC2753F",
                        "ckDefenseRightPos": "000000008FC2753F",
                        "ckOffenseLeftPos":  "00000000D7A3703F",
                        "ckOffenseRightPos": "00000000D7A3703F",
                        "pkDefensePos": "000000008FC2753F",
                        "pkOffensePos": "00000000D7A3703F",
                        "bustupPos":    "0000000000008040",
                        "positionNo": 0,
                        "positionId": 1,
                        "passNo":     6,
                        "bKickoff":   false,
                        "bFollow":    false
                    }
                ]
            },
            {
                "name": "m_SoccerFormationInfoList",
                "typeName": "SOCCER_FORMATION_INFO",
                "values": [
                    // formation 0 — dump réel lignes 18321-18330
                    {
                        "formId": "0x2CD9760B",
                        "placementInfo": [0, 1],
                        "powerOffense": 3,
                        "powerDefense": 3,
                        "nounId": "0xB477A689",
                        "descId": "0x6B085B0E"
                    },
                    // formation 1 — dump réel lignes 18332-18341
                    {
                        "formId": "0xB5D027B1",
                        "placementInfo": [1, 0],
                        "powerOffense": 4,
                        "powerDefense": 2,
                        "nounId": "0x2D7EF733",
                        "descId": "0xF2010AB4"
                    }
                ]
            },
            {
                "name": "m_SoccerCurvePointInfoList",
                "typeName": "SOCCER_CURVE_POINT_INFO",
                "values": [
                    // entrée 0 — dump réel lignes 19592-19595
                    { "rate": -1, "point": -1 },
                    // entrée 1 — dump réel lignes 19596-19599
                    { "rate": 1,  "point": 1  }
                ]
            },
            {
                "name": "m_SoccerLineCurveInfoList",
                "typeName": "SOCCER_LINE_CURVE_INFO",
                "values": [
                    // entrée 0 — dump réel lignes 19726-19731
                    {
                        "lineCurveId": "0xBF81F7F9",
                        "curveInfo": [0, 2]
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn position_info_type_gk() {
    let cfg = parse_formation_config(&fixture());
    assert_eq!(cfg.positions.len(), 1);
    let p: &SoccerPositionInfo = &cfg.positions[0];

    // formation_config_0.02.16.cfg.bin.json : m_SoccerPositionInfoList[0]
    assert_eq!(p.position_id, 1);
    assert_eq!(p.center_line_weight, 0.5_f32);
    // 0.05 et 0.95 ne sont pas exactement représentables en f32 ; on valide à 1 ulp f32.
    assert!(
        (p.offense_line_weight - 0.05_f32).abs() < 1e-6,
        "offenseLineWeight ≈ 0.05"
    );
    assert!(
        (p.defense_line_weight - 0.95_f32).abs() < 1e-6,
        "defenseLineWeight ≈ 0.95"
    );
}

#[test]
fn placement_gk_slot0_champs_scalaires() {
    let cfg = parse_formation_config(&fixture());
    assert_eq!(cfg.placements.len(), 1);
    let s: &SoccerFormPlacementInfo = &cfg.placements[0];

    // formation_config_0.02.16 : m_SoccerFormPlacementInfoList[0] (lignes 73-90)
    assert_eq!(s.position_no, 0, "GK positionNo");
    assert_eq!(s.position_id, 1, "GK positionId");
    assert_eq!(s.pass_no, 6, "GK passNo");
    assert!(!s.b_kickoff, "GK ne tire pas le coup d'envoi");
    assert!(!s.b_follow, "GK ne suit pas");
}

#[test]
fn placement_gk_defense_pos_hex_decode() {
    let cfg = parse_formation_config(&fixture());
    let s = &cfg.placements[0];

    // defensePos = "000000008FC2753F"
    // → x: f32::from_bits(0x00000000) = 0.0
    // → y: f32::from_bits(0x3F75C28F) ≈ 0.9599... (GK au centre, profond en défense)
    assert_eq!(s.defense_pos.x, 0.0_f32, "GK defensePos.x = 0 (centre)");
    assert_eq!(
        s.defense_pos.y,
        f32::from_bits(0x3F75_C28F),
        "GK defensePos.y = f32::from_bits(0x3F75C28F)"
    );
    assert!(
        (s.defense_pos.y - 0.96_f32).abs() < 1e-3,
        "GK defensePos.y ≈ 0.96 (profond)"
    );
}

#[test]
fn placement_gk_offense_pos_hex_decode() {
    let cfg = parse_formation_config(&fixture());
    let s = &cfg.placements[0];

    // offensePos = "00000000D7A3703F"
    // → x: 0.0
    // → y: f32::from_bits(0x3F70A3D7) ≈ 0.9399... (GK légèrement plus avancé)
    assert_eq!(s.offense_pos.x, 0.0_f32);
    assert_eq!(s.offense_pos.y, f32::from_bits(0x3F70_A3D7));
    assert!(
        (s.offense_pos.y - 0.94_f32).abs() < 1e-3,
        "GK offensePos.y ≈ 0.94"
    );
}

#[test]
fn placement_gk_bustup_pos_exact() {
    let cfg = parse_formation_config(&fixture());
    let s = &cfg.placements[0];

    // bustupPos = "0000000000008040"
    // → x: 0.0, y: f32::from_bits(0x40800000) = 4.0 (exactement représentable)
    assert_eq!(
        s.bustup_pos,
        Vec2 { x: 0.0, y: 4.0 },
        "bustupPos GK = (0.0, 4.0)"
    );
}

#[test]
fn formation_0_champs() {
    let cfg = parse_formation_config(&fixture());
    assert_eq!(cfg.formations.len(), 2);
    let f0: &SoccerFormationInfo = &cfg.formations[0];

    // formation_config_0.02.16 : m_SoccerFormationInfoList[0] (lignes 18321-18330)
    assert_eq!(f0.form_id, HashId(0x2CD9_760B));
    assert_eq!(f0.placement_offset, 0);
    assert_eq!(f0.placement_count, 1);
    assert_eq!(f0.power_offense, 3);
    assert_eq!(f0.power_defense, 3);
    assert_eq!(f0.noun_id, HashId(0xB477_A689));
    assert_eq!(f0.desc_id, HashId(0x6B08_5B0E));
}

#[test]
fn formation_1_offensif() {
    let cfg = parse_formation_config(&fixture());
    let f1 = &cfg.formations[1];

    // formation_config_0.02.16 : m_SoccerFormationInfoList[1] (lignes 18332-18341)
    assert_eq!(f1.form_id, HashId(0xB5D0_27B1));
    assert_eq!(f1.power_offense, 4, "formation 1 plus offensive");
    assert_eq!(f1.power_defense, 2);
}

#[test]
fn curve_point_premiere_entree() {
    let cfg = parse_formation_config(&fixture());
    assert_eq!(cfg.curve_points.len(), 2);
    let cp0: &SoccerCurvePointInfo = &cfg.curve_points[0];

    // formation_config_0.02.16 : m_SoccerCurvePointInfoList[0] (lignes 19592-19595)
    assert_eq!(cp0.rate, -1.0_f32);
    assert_eq!(cp0.point, -1.0_f32);
}

#[test]
fn line_curve_premiere_entree() {
    let cfg = parse_formation_config(&fixture());
    assert_eq!(cfg.line_curves.len(), 1);
    let lc: &SoccerLineCurveInfo = &cfg.line_curves[0];

    // formation_config_0.02.16 : m_SoccerLineCurveInfoList[0] (lignes 19726-19731)
    assert_eq!(lc.line_curve_id, HashId(0xBF81_F7F9));
    assert_eq!(lc.curve_offset, 0);
    assert_eq!(lc.curve_count, 2);
}

#[test]
fn placements_of_slice() {
    let cfg = parse_formation_config(&fixture());
    // La formation 0 a placement_offset=0, placement_count=1 dans la fixture.
    let slots = cfg.placements_of(&cfg.formations[0]);
    assert_eq!(
        slots.len(),
        1,
        "placements_of renvoie exactement placement_count éléments"
    );
    assert_eq!(slots[0].position_id, 1, "le seul slot est le GK");
}

#[test]
fn curve_points_of_slice() {
    let cfg = parse_formation_config(&fixture());
    // La courbe 0 a curve_offset=0, curve_count=2 dans la fixture (2 points présents).
    let pts = cfg.curve_points_of(&cfg.line_curves[0]);
    assert_eq!(pts.len(), 2);
    assert_eq!(pts[0].rate, -1.0_f32);
    assert_eq!(pts[1].rate, 1.0_f32);
}

#[test]
fn find_by_id() {
    let cfg = parse_formation_config(&fixture());
    assert!(cfg.find_by_id(HashId(0x2CD9_760B)).is_some());
    assert!(cfg.find_by_id(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn vec2_parse_hex_zero_string() {
    // Chaîne de mauvaise longueur → Vec2::ZERO.
    assert_eq!(Vec2::parse_hex(""), Vec2::ZERO);
    assert_eq!(Vec2::parse_hex("0000000000000000"), Vec2 { x: 0.0, y: 0.0 });
}

// ─── Test sur le vrai fichier (skip si absent du VPS) ────────────────────────

const REAL_PATH: &str = "formation/formation_config_0.02.16.cfg.bin.json";

fn load_real() -> Option<FormationConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_formation_config(&root))
}

#[test]
fn real_file_comptes() {
    let Some(cfg) = load_real() else { return };

    // 10 types de position (positionId 1..10), 1073 slots, 115 formations, 7 courbes.
    assert_eq!(cfg.positions.len(), 10, "10 types de position");
    assert_eq!(cfg.placements.len(), 1073, "1073 slots de placement");
    assert_eq!(cfg.formations.len(), 115, "115 formations");
    assert_eq!(cfg.line_curves.len(), 7, "7 courbes de trajectoire");
}

#[test]
fn real_file_formation0_valeurs() {
    let Some(cfg) = load_real() else { return };

    // formation_config_0.02.16 : m_SoccerFormationInfoList[0] (lignes 18321-18330)
    let f0 = &cfg.formations[0];
    assert_eq!(f0.form_id, HashId(0x2CD9_760B), "formation 0 formId");
    assert_eq!(f0.placement_offset, 0);
    assert_eq!(f0.placement_count, 11);
    assert_eq!(f0.power_offense, 3);
    assert_eq!(f0.power_defense, 3);
    assert_eq!(f0.noun_id, HashId(0xB477_A689));
    assert_eq!(f0.desc_id, HashId(0x6B08_5B0E));
}

#[test]
fn real_file_gk_slot0_positions() {
    let Some(cfg) = load_real() else { return };

    // m_SoccerFormPlacementInfoList[0] — GK de la formation 0
    // formation_config_0.02.16 lignes 73-90
    let gk = &cfg.placements[0];
    assert_eq!(gk.position_no, 0, "positionNo=0");
    assert_eq!(gk.position_id, 1, "positionId=1 (GK)");
    assert_eq!(gk.pass_no, 6);
    assert!(!gk.b_kickoff);
    assert!(!gk.b_follow);

    // defensePos = "000000008FC2753F" → (0.0, f32::from_bits(0x3F75C28F))
    assert_eq!(gk.defense_pos.x, 0.0_f32);
    assert_eq!(gk.defense_pos.y, f32::from_bits(0x3F75_C28F));

    // offensePos = "00000000D7A3703F" → (0.0, f32::from_bits(0x3F70A3D7))
    assert_eq!(gk.offense_pos.x, 0.0_f32);
    assert_eq!(gk.offense_pos.y, f32::from_bits(0x3F70_A3D7));

    // bustupPos = "0000000000008040" → (0.0, 4.0)
    assert_eq!(gk.bustup_pos, Vec2 { x: 0.0, y: 4.0 });
}

#[test]
fn real_file_slot8_kickoff() {
    let Some(cfg) = load_real() else { return };

    // m_SoccerFormPlacementInfoList[8] — bKickoff=true
    // formation_config_0.02.16 lignes 210-226
    let slot8 = &cfg.placements[8];
    assert!(slot8.b_kickoff, "slot 8 doit avoir bKickoff=true");
    assert_eq!(slot8.position_no, 8);
    assert_eq!(slot8.position_id, 8);
}

#[test]
fn real_file_position_info_type_gk() {
    let Some(cfg) = load_real() else { return };

    // m_SoccerPositionInfoList[0] — positionId=1 (type GK)
    // formation_config_0.02.16 lignes 8-13
    let p = &cfg.positions[0];
    assert_eq!(p.position_id, 1);
    assert_eq!(p.center_line_weight, 0.5_f32);
    assert!((p.offense_line_weight - 0.05_f32).abs() < 1e-6);
    assert!((p.defense_line_weight - 0.95_f32).abs() < 1e-6);
}

#[test]
fn real_file_line_curve0() {
    let Some(cfg) = load_real() else { return };

    // m_SoccerLineCurveInfoList[0] — formation_config_0.02.16 lignes 19726-19731
    let lc = &cfg.line_curves[0];
    assert_eq!(lc.line_curve_id, HashId(0xBF81_F7F9));
    assert_eq!(lc.curve_offset, 0);
    assert_eq!(lc.curve_count, 2);
}

#[test]
fn real_file_placements_of_formation0() {
    let Some(cfg) = load_real() else { return };

    // La formation 0 référence 11 slots à partir de l'offset 0.
    let slots = cfg.placements_of(&cfg.formations[0]);
    assert_eq!(slots.len(), 11, "formation 0 a exactement 11 joueurs");
    // Le premier slot (GK) doit avoir positionId=1.
    assert_eq!(slots[0].position_id, 1, "premier slot = GK (positionId=1)");
    // Un seul joueur a bKickoff=true dans cette formation.
    let kickoff_count = slots.iter().filter(|s| s.b_kickoff).count();
    assert_eq!(kickoff_count, 1, "exactement 1 joueur tire le coup d'envoi");
}

#[test]
fn real_file_formation1_offensif() {
    let Some(cfg) = load_real() else { return };

    // formation_config_0.02.16 : m_SoccerFormationInfoList[1] (lignes 18332-18341)
    let f1 = &cfg.formations[1];
    assert_eq!(f1.form_id, HashId(0xB5D0_27B1));
    assert_eq!(f1.power_offense, 4);
    assert_eq!(f1.power_defense, 2);
}
