#![allow(clippy::pedantic)]
//! Tests golden `input` — valeurs réelles tirées des 3 dumps DualSense/manette :
//! - `data/common/gamedata/input/adaptive_trigger_def_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/input/haptic_feedback_def_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/input/vibration_def_0.00.09.cfg.bin.json`
//!
//! Toutes les valeurs ci-dessous ont été extraites octet-pour-octet des JSON réels.

mod common;

use nie_data::hash::HashId;
use nie_data::input::{
    AdaptiveTriggerDef, HapticFeedbackDef, VibrationDef, parse_adaptive_trigger_def,
    parse_haptic_feedback_def, parse_vibration_def,
};

const ADAPTIVE_PATH: &str = "input/adaptive_trigger_def_0.00.00.cfg.bin.json";
const HAPTIC_PATH: &str = "input/haptic_feedback_def_0.00.00.cfg.bin.json";
const VIBRATION_PATH: &str = "input/vibration_def_0.00.09.cfg.bin.json";

fn load(path: &str) -> Option<serde_json::Value> {
    let path = common::chemin(path)?;
    if !path.is_file() {
        eprintln!("skip : {} absent du corpus", path.display());
        return None;
    }
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", path.display()));
    Some(serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}")))
}

fn load_adaptive() -> Option<AdaptiveTriggerDef> {
    load(ADAPTIVE_PATH).map(|r| parse_adaptive_trigger_def(&r))
}
fn load_haptic() -> Option<HapticFeedbackDef> {
    load(HAPTIC_PATH).map(|r| parse_haptic_feedback_def(&r))
}
fn load_vibration() -> Option<VibrationDef> {
    load(VIBRATION_PATH).map(|r| parse_vibration_def(&r))
}

// ─── adaptive_trigger_def ───────────────────────────────────────────────────────

#[test]
fn adaptive_comptes() {
    let Some(cfg) = load_adaptive() else { return };
    assert_eq!(cfg.mode_items.len(), 7, "7 MODE_ITEM_INFO");
    assert_eq!(cfg.triggers.len(), 3, "3 INFO_ADAPTIVE_TRIGGER_DATA_CONFIG");
}

#[test]
fn adaptive_mode_item0_nul() {
    let Some(cfg) = load_adaptive() else { return };
    // m_ModeItemList[0] entièrement nul (mode « off »).
    let m = &cfg.mode_items[0];
    assert_eq!(m.fd_start_position, 0);
    assert_eq!(m.wp_strength, 0);
    assert_eq!(m.mpvb_amplitude, 0);
    assert_eq!(m.trigger_type, 0);
}

#[test]
fn adaptive_mode_item1_valeurs() {
    let Some(cfg) = load_adaptive() else { return };
    // m_ModeItemList[1].
    let m = &cfg.mode_items[1];
    assert_eq!(m.fd_start_position, 2);
    assert_eq!(m.fd_strength, 2);
    assert_eq!(m.wp_start_position, 2);
    assert_eq!(m.wp_end_position, 8);
    assert_eq!(m.wp_strength, 8);
    assert_eq!(m.mpvb_amplitude, 1);
}

#[test]
fn adaptive_mode_item4_sl_strength() {
    let Some(cfg) = load_adaptive() else { return };
    // m_ModeItemList[4] : seule entrée avec sl_startStrength=1.
    let m = &cfg.mode_items[4];
    assert_eq!(m.fd_start_position, 6);
    assert_eq!(m.wp_end_position, 6);
    assert_eq!(m.sl_start_strength, 1);
}

#[test]
fn adaptive_trigger0_valeurs() {
    let Some(cfg) = load_adaptive() else { return };
    // m_InfoAdaptiveTriggerDataList[0].
    let t = &cfg.triggers[0];
    assert_eq!(t.id, HashId(0xCC92_A4CA));
    assert_eq!(t.id_name, "feedback01");
    assert_eq!(t.mask, 3);
    assert_eq!(t.mode_item_list, vec![1, 2]);
}

#[test]
fn adaptive_trigger2_valeurs() {
    let Some(cfg) = load_adaptive() else { return };
    // m_InfoAdaptiveTriggerDataList[2].
    let t = &cfg.triggers[2];
    assert_eq!(t.id, HashId(0x229C_C5E6));
    assert_eq!(t.id_name, "feedback03");
    assert_eq!(t.mode_item_list, vec![5, 2]);
}

#[test]
fn adaptive_find_by_id() {
    let Some(cfg) = load_adaptive() else { return };
    let found = cfg.find_by_id(HashId(0x559B_F570));
    assert!(found.is_some());
    assert_eq!(found.unwrap().id_name, "feedback02");
    assert!(cfg.find_by_id(HashId(0xDEAD_BEEF)).is_none());
}

// ─── haptic_feedback_def ────────────────────────────────────────────────────────

#[test]
fn haptic_comptes() {
    let Some(cfg) = load_haptic() else { return };
    assert_eq!(cfg.mode_items.len(), 7, "7 MODE_ITEM_INFO");
    assert_eq!(cfg.feedbacks.len(), 3, "3 INFO_HAPTIC_FEEDBACK_DATA_CONFIG");
}

#[test]
fn haptic_mode_items_identiques_a_adaptive() {
    let (Some(h), Some(a)) = (load_haptic(), load_adaptive()) else {
        return;
    };
    // Les deux fichiers partagent un m_ModeItemList identique.
    assert_eq!(h.mode_items, a.mode_items);
}

#[test]
fn haptic_feedback0_valeurs() {
    let Some(cfg) = load_haptic() else { return };
    // m_InfoHapticFeedbackDataList[0].
    let f = &cfg.feedbacks[0];
    assert_eq!(f.id, HashId(0xCC92_A4CA));
    assert_eq!(f.id_name, "feedback01");
    assert_eq!(f.mask, 3);
    assert_eq!(f.mode_item_list, vec![1, 2]);
    assert_eq!(f.res_path, "#/debug/input/vibration/HandGun_Vibration.wav");
}

#[test]
fn haptic_feedback2_res_path() {
    let Some(cfg) = load_haptic() else { return };
    // m_InfoHapticFeedbackDataList[2].
    let f = &cfg.feedbacks[2];
    assert_eq!(f.id, HashId(0x229C_C5E6));
    assert_eq!(f.res_path, "#/debug/input/vibration/ShotGun_Vibration.wav");
}

#[test]
fn haptic_find_by_id() {
    let Some(cfg) = load_haptic() else { return };
    let found = cfg.find_by_id(HashId(0x559B_F570));
    assert!(found.is_some());
    assert_eq!(
        found.unwrap().res_path,
        "#/debug/input/vibration/MachineGun_Vibration.wav"
    );
}

// ─── vibration_def ──────────────────────────────────────────────────────────────

#[test]
fn vibration_compte() {
    let Some(cfg) = load_vibration() else { return };
    assert_eq!(cfg.vibrations.len(), 63, "63 VIBRATION_DEF_INFO");
}

#[test]
fn vibration_entree0_valeurs() {
    let Some(cfg) = load_vibration() else { return };
    // m_vibrationDefList[0].
    let v = &cfg.vibrations[0];
    assert_eq!(v.vibration_id, HashId(0x23DD_8401));
    assert_eq!(v.high_frequency, 250);
    assert_eq!(v.low_frequency, 50);
    assert!((v.amplitude_high - 0.2).abs() < 1e-6);
    assert!((v.amplitude_low - 0.1).abs() < 1e-6);
    assert!((v.power_param - 0.9).abs() < 1e-6);
    assert!((v.in_time - 0.2).abs() < 1e-6);
    assert!((v.loop_time - 0.1).abs() < 1e-6);
    assert!((v.out_time - 0.1).abs() < 1e-6);
    assert!((v.hold_min_limit_power - 0.2).abs() < 1e-6);
    assert!((v.large_rate - 0.07).abs() < 1e-6);
    assert!((v.small_rate - 1.0).abs() < 1e-6);
    assert!(!v.b_loop_hold);
    assert_eq!(v.priority, 50);
    assert_eq!(v.wav_path, "");
}

#[test]
fn vibration_entree30_valeurs() {
    let Some(cfg) = load_vibration() else { return };
    // m_vibrationDefList[30].
    let v = &cfg.vibrations[30];
    assert_eq!(v.vibration_id, HashId(0xB381_BE7C));
    assert_eq!(v.high_frequency, 250);
    assert_eq!(v.low_frequency, 90);
    assert!((v.power_param - 0.55).abs() < 1e-6);
    assert!((v.small_rate - 0.94).abs() < 1e-6);
    assert_eq!(v.priority, 5);
}

#[test]
fn vibration_derniere_avec_wav() {
    let Some(cfg) = load_vibration() else { return };
    // m_vibrationDefList[62] (dernière) : seule entrée avec wavPath non vide en fin de liste.
    let v = &cfg.vibrations[62];
    assert_eq!(v.vibration_id, HashId(0x07E3_2966));
    assert_eq!(v.high_frequency, 650);
    assert_eq!(v.low_frequency, 130);
    assert!((v.amplitude_high - 0.7).abs() < 1e-6);
    assert!((v.amplitude_low - 0.5).abs() < 1e-6);
    assert!((v.small_rate - 0.8).abs() < 1e-6);
    assert_eq!(v.priority, 0);
    assert_eq!(v.wav_path, "#/input/vibration/vib_janken_damage_01.wav");
}

#[test]
fn vibration_wavpath_compte() {
    let Some(cfg) = load_vibration() else { return };
    // 16 des 63 entrées ont un wavPath non vide dans le dump réel.
    let n = cfg
        .vibrations
        .iter()
        .filter(|v| !v.wav_path.is_empty())
        .count();
    assert_eq!(n, 16);
}

#[test]
fn vibration_find_by_id() {
    let Some(cfg) = load_vibration() else { return };
    let found = cfg.find_by_id(HashId(0x23DD_8401));
    assert!(found.is_some());
    assert_eq!(found.unwrap().priority, 50);
    assert!(cfg.find_by_id(HashId(0x0000_0000)).is_none());
}

// ─── Robustesse : listes manquantes ─────────────────────────────────────────────

#[test]
fn listes_manquantes_renvoient_vide() {
    let root = serde_json::json!({ "version": 100, "lists": [] });
    assert_eq!(parse_adaptive_trigger_def(&root).triggers.len(), 0);
    assert_eq!(parse_haptic_feedback_def(&root).feedbacks.len(), 0);
    assert_eq!(parse_vibration_def(&root).vibrations.len(), 0);
}
