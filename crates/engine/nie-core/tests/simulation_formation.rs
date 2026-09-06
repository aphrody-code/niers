//! Tests d'intégration : simulation de match avec vraies données de formation IEVR.
//!
//! ## Fichier de référence
//!
//! `data/common/gamedata/formation/formation_config_0.02.16.cfg.bin.json`
//!
//! Ce fichier est gitignore (données Level-5, copyright) et présent uniquement sur le VPS.
//! Tous les tests qui le lisent appliquent un **skip silencieux** si le fichier est absent,
//! identique au pattern de `crates/engine/nie-data/tests/formation_golden.rs`.
//!
//! ## Ce qui est validé
//!
//! - [`nie_core::match_sim::TeamSetup::with_formation`] initialise bien les placements depuis
//!   [`nie_data::formation::FormationConfig::placements_of`].
//! - Les valeurs `start_pos`, `defense_pos`, `offense_pos` du GK (slot 0) correspondent
//!   octet-pour-octet aux valeurs du dump réel (déjà validées dans `nie_data/tests/formation_golden.rs`).
//! - Le déterminisme de la simulation tient avec des formations réelles.
//! - `simulate_match` sans formation → `home_placements = None`.

#![allow(clippy::pedantic)]

use nie_core::match_sim::{FieldPos, TeamSetup, simulate_match};
use nie_core::stats::StatBlock;
use nie_data::formation::{FormationConfig, parse_formation_config};

const REAL_PATH: &str = "data/common/gamedata/formation/formation_config_0.02.16.cfg.bin.json";

/// Charge le vrai fichier de formation ; retourne `None` si absent (skip silencieux).
fn load_real() -> Option<FormationConfig> {
    if !std::path::Path::new(REAL_PATH).exists() {
        return None;
    }
    let content =
        std::fs::read_to_string(REAL_PATH).unwrap_or_else(|e| panic!("lecture {REAL_PATH}: {e}"));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_formation_config(&root))
}

fn stats_fixture() -> StatBlock {
    StatBlock::from_array([200, 190, 185, 175, 160, 170, 155])
}

// ─── Tests sans fichier réel ─────────────────────────────────────────────────

/// Sans formation, `simulate_match` renvoie `home_placements = None`.
///
/// Ce test ne dépend pas du fichier réel.
#[test]
fn sans_formation_placements_absents() {
    let stats = stats_fixture();
    let home = TeamSetup::new("Raimon", stats);
    let away = TeamSetup::new("Teikoku", stats);
    let res = simulate_match(home, away, 42);
    assert!(
        res.home_placements.is_none(),
        "home_placements = None sans formation fournie"
    );
    assert!(
        res.away_placements.is_none(),
        "away_placements = None sans formation fournie"
    );
}

// ─── Tests sur le vrai fichier (skip si absent) ───────────────────────────────

/// Les placements initiaux viennent bien des vraies positions du dump.
///
/// ## Valeurs confirmées byte-à-byte
///
/// GK slot 0, formation 0 (`formId = 0x2CD9760B`) :
/// - `positionId = 1` (GK)
/// - `startPos = "000000008FC2753F"` → x=0.0, y=f32::from_bits(0x3F75C28F) ≈ 0.96
///   (déjà validé octet-par-octet dans `nie_data::tests::formation_golden::real_file_gk_slot0_positions`)
#[test]
fn placement_initial_depuis_vraies_positions() {
    let Some(cfg) = load_real() else { return };

    let stats = stats_fixture();
    let formation = &cfg.formations[0]; // formId=0x2CD9760B, offset=0, count=11
    let equipe = TeamSetup::new("Raimon", stats).with_formation(&cfg, formation);

    let placements = equipe
        .placements
        .as_ref()
        .expect("placements initialisés par with_formation");
    assert_eq!(placements.len(), 11, "formation standard = 11 joueurs");

    // Slot 0 = GK (positionId=1, positionNo=0)
    let gk = &placements[0];
    assert_eq!(gk.position_id, 1, "slot 0 = GK (positionId=1)");
    assert_eq!(gk.position_no, 0, "positionNo=0 pour le GK");
    assert!(!gk.b_kickoff, "le GK ne tire pas le coup d'envoi");

    // startPos = "000000008FC2753F" → (0.0, f32::from_bits(0x3F75C28F))
    // Confirmé byte-à-byte dans nie_data/tests/formation_golden.rs::real_file_gk_slot0_positions
    assert_eq!(
        gk.start_pos.x, 0.0_f32,
        "GK start_pos.x = 0.0 (centre horizontal)"
    );
    assert_eq!(
        gk.start_pos.y,
        f32::from_bits(0x3F75_C28F),
        "GK start_pos.y = f32::from_bits(0x3F75C28F) — validé byte-à-byte sur le dump"
    );
    assert!(
        (gk.start_pos.y - 0.96_f32).abs() < 1e-3,
        "GK start_pos.y ≈ 0.96 (défense profonde)"
    );

    // defensePos = "000000008FC2753F" (identique à startPos pour le GK)
    assert_eq!(
        gk.defense_pos,
        FieldPos {
            x: 0.0_f32,
            y: f32::from_bits(0x3F75_C28F)
        },
        "GK defense_pos byte-exact"
    );

    // offensePos = "00000000D7A3703F" → (0.0, f32::from_bits(0x3F70A3D7))
    assert_eq!(gk.offense_pos.x, 0.0_f32, "GK offense_pos.x = 0.0");
    assert_eq!(
        gk.offense_pos.y,
        f32::from_bits(0x3F70_A3D7),
        "GK offense_pos.y = f32::from_bits(0x3F70A3D7) — validé byte-à-byte"
    );
}

/// Exactement un joueur tire le coup d'envoi dans la formation 0.
#[test]
fn kickoff_player_unique_dans_formation() {
    let Some(cfg) = load_real() else { return };

    let stats = stats_fixture();
    let formation = &cfg.formations[0];
    let equipe = TeamSetup::new("Raimon", stats).with_formation(&cfg, formation);

    let placements = equipe.placements.as_ref().unwrap();
    let kickoff_count = placements.iter().filter(|p| p.b_kickoff).count();
    assert_eq!(
        kickoff_count, 1,
        "exactement 1 joueur tire le coup d'envoi (formation 0)"
    );
}

/// Les placements propagés dans `MatchResult` correspondent exactement aux données du dump.
#[test]
fn match_result_home_placements_coherents() {
    let Some(cfg) = load_real() else { return };

    let stats = stats_fixture();
    let formation = &cfg.formations[0];
    let home = TeamSetup::new("Raimon", stats).with_formation(&cfg, formation);
    let away = TeamSetup::new("Teikoku", stats).with_formation(&cfg, formation);
    let res = simulate_match(home, away, 7);

    let home_pl = res
        .home_placements
        .as_ref()
        .expect("home_placements dans MatchResult");
    let away_pl = res
        .away_placements
        .as_ref()
        .expect("away_placements dans MatchResult");

    assert_eq!(home_pl.len(), 11, "home : 11 placements");
    assert_eq!(away_pl.len(), 11, "away : 11 placements");

    // Les deux équipes ont la même formation → mêmes positions.
    assert_eq!(
        home_pl[0].start_pos, away_pl[0].start_pos,
        "même start_pos pour les deux équipes issues de la même formation"
    );

    // Vérification byte-exacte du GK dans le MatchResult.
    assert_eq!(
        home_pl[0].start_pos.y,
        f32::from_bits(0x3F75_C28F),
        "GK start_pos.y byte-exact dans MatchResult"
    );
}

/// Le déterminisme tient avec des équipes portant des formations réelles.
///
/// Même seed + mêmes formations → même MatchResult (scores, placements).
#[test]
fn determinisme_avec_formations_reelles() {
    let Some(cfg) = load_real() else { return };

    let stats = stats_fixture();
    let formation = &cfg.formations[0];

    let h1 = TeamSetup::new("Raimon", stats).with_formation(&cfg, formation);
    let a1 = TeamSetup::new("Teikoku", stats).with_formation(&cfg, formation);
    let h2 = TeamSetup::new("Raimon", stats).with_formation(&cfg, formation);
    let a2 = TeamSetup::new("Teikoku", stats).with_formation(&cfg, formation);

    let r1 = simulate_match(h1, a1, 0xCAFE_BABE_1234_5678);
    let r2 = simulate_match(h2, a2, 0xCAFE_BABE_1234_5678);

    assert_eq!(
        r1.home_score, r2.home_score,
        "score domicile reproductible avec formations"
    );
    assert_eq!(
        r1.away_score, r2.away_score,
        "score visiteur reproductible avec formations"
    );
    assert_eq!(r1.final_clock, r2.final_clock, "final_clock reproductible");
    assert_eq!(
        r1.home_placements, r2.home_placements,
        "home_placements identiques"
    );
    assert_eq!(
        r1.away_placements, r2.away_placements,
        "away_placements identiques"
    );
}

/// Les 11 placements de la formation 0 ont tous `position_no` distinct (0..=10).
#[test]
fn placements_position_no_uniques_0_a_10() {
    let Some(cfg) = load_real() else { return };

    let stats = stats_fixture();
    let formation = &cfg.formations[0];
    let equipe = TeamSetup::new("Test", stats).with_formation(&cfg, formation);

    let placements = equipe.placements.as_ref().unwrap();
    let mut nos: Vec<i64> = placements.iter().map(|p| p.position_no).collect();
    nos.sort_unstable();
    let attendu: Vec<i64> = (0..11).collect();
    assert_eq!(
        nos, attendu,
        "position_no 0..=10 tous présents et distincts"
    );
}
