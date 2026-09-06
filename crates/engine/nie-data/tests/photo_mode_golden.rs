#![allow(clippy::pedantic)]
//! Tests golden `photo_mode` — valeurs réelles tirées de :
//! `photo_mode/photo_mode_random_pose_config.cfg.bin.json`
//!
//! ## Vérifications champ par champ (fichier → valeur confirmée)
//!
//! ### m_randomPoseList — 91 entrées au total
//!
//! Entrées « ancres » (charaBodyType ≠ 255), index réel dans la liste :
//! - `[0]`  bt=0  crc=`0xF8CC8EDE`
//! - `[6]`  bt=1  crc=`0x7D478513`
//! - `[11]` bt=2  crc=`0x301504D6`
//! - `[16]` bt=3  crc=`0xA52C614B`
//! - `[59]` bt=11 crc=`0x290E3597`
//! - `[73]` bt=16 crc=`0x97A26A3E`
//! - `[78]` bt=17 crc=`0x97A26A3E`
//! - `[83]` bt=14 crc=`0x290E3597`
//! - `[87]` bt=15 crc=`0x290E3597`
//!
//! Entrées génériques (charaBodyType = 255) : 73 sur 91.
//! - `[1]`  bt=255 crc=`0xD3E1DD1D`
//! - `[50]` bt=255 crc=`0x7D478513`
//! - `[90]` bt=255 crc=`0x192B4017` (dernière entrée)
//!
//! Types de corps distincts observés : 0..17 (séquence non triée dans le fichier :
//! …, 13, 16, 17, 14, 15) + le joker 255.

mod common;

use nie_data::hash::HashId;
use nie_data::photo_mode::{
    PhotoModeRandomPoseConfig, RandomPose, parse_photo_mode_random_pose_config,
};
use serde_json::json;

// ─── Fixture ────────────────────────────────────────────────────────────────
// Valeurs extraites octet-pour-octet du dump réel — voir commentaires ci-dessus.

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_randomPoseList",
                "typeName": "RANDOM_POSE",
                "values": [
                    // index 0 — première ancre (charaBodyType=0)
                    { "charaBodyType": 0,   "motionNameCrc": "0xF8CC8EDE" },
                    // index 1 — entrée générique (charaBodyType=255)
                    { "charaBodyType": 255, "motionNameCrc": "0xD3E1DD1D" },
                    // index 2 — ancre charaBodyType=2 (déplacée ici pour la fixture)
                    { "charaBodyType": 2,   "motionNameCrc": "0x301504D6" },
                    // index 3 — dernière entrée réelle (charaBodyType=255)
                    { "charaBodyType": 255, "motionNameCrc": "0x192B4017" }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ────────────────────────────────────────────────────

#[test]
fn fixture_compte_entrees() {
    let cfg = parse_photo_mode_random_pose_config(&fixture());
    assert_eq!(cfg.poses.len(), 4);
}

#[test]
fn fixture_entree0_champs() {
    let cfg = parse_photo_mode_random_pose_config(&fixture());
    let p: &RandomPose = &cfg.poses[0];
    assert_eq!(p.chara_body_type, 0);
    assert_eq!(p.motion_name_crc, HashId(0xF8CC_8EDE));
}

#[test]
fn fixture_entree_generique_255() {
    let cfg = parse_photo_mode_random_pose_config(&fixture());
    let p = &cfg.poses[1];
    assert_eq!(p.chara_body_type, 255);
    assert_eq!(p.motion_name_crc, HashId(0xD3E1_DD1D));
}

#[test]
fn fixture_find_by_motion_present() {
    let cfg = parse_photo_mode_random_pose_config(&fixture());
    let found = cfg.find_by_motion(HashId(0x3015_04D6));
    assert!(found.is_some());
    assert_eq!(found.unwrap().chara_body_type, 2);
}

#[test]
fn fixture_find_by_motion_absent() {
    let cfg = parse_photo_mode_random_pose_config(&fixture());
    assert!(cfg.find_by_motion(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn fixture_poses_for_body_type() {
    let cfg = parse_photo_mode_random_pose_config(&fixture());
    assert_eq!(cfg.poses_for_body_type(2).len(), 1);
    assert_eq!(cfg.poses_for_body_type(255).len(), 2);
    assert_eq!(cfg.poses_for_body_type(99).len(), 0);
}

#[test]
fn fixture_liste_manquante_renvoie_vide() {
    let root = serde_json::json!({ "version": 100, "lists": [] });
    let cfg = parse_photo_mode_random_pose_config(&root);
    assert_eq!(cfg.poses.len(), 0);
}

// ─── Test sur le vrai fichier (skip si absent du VPS) ───────────────────────

const REAL_PATH: &str = "photo_mode/photo_mode_random_pose_config.cfg.bin.json";

fn load_real() -> Option<PhotoModeRandomPoseConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_photo_mode_random_pose_config(&root))
}

#[test]
fn real_file_compte_total() {
    let Some(cfg) = load_real() else { return };
    // photo_mode_random_pose_config : 91 entrées dans m_randomPoseList
    assert_eq!(
        cfg.poses.len(),
        91,
        "91 entrées RANDOM_POSE dans le dump réel"
    );
}

#[test]
fn real_file_entree0() {
    let Some(cfg) = load_real() else { return };
    let p = &cfg.poses[0];
    assert_eq!(p.chara_body_type, 0, "charaBodyType[0]");
    assert_eq!(p.motion_name_crc, HashId(0xF8CC_8EDE), "motionNameCrc[0]");
}

#[test]
fn real_file_entree1_generique() {
    let Some(cfg) = load_real() else { return };
    let p = &cfg.poses[1];
    assert_eq!(p.chara_body_type, 255, "charaBodyType[1]");
    assert_eq!(p.motion_name_crc, HashId(0xD3E1_DD1D), "motionNameCrc[1]");
}

#[test]
fn real_file_ancres_par_index() {
    let Some(cfg) = load_real() else { return };
    // Index réels des entrées non-255 (charaBodyType, motionNameCrc).
    let ancres: &[(usize, i64, u32)] = &[
        (6, 1, 0x7D47_8513),
        (11, 2, 0x3015_04D6),
        (16, 3, 0xA52C_614B),
        (59, 11, 0x290E_3597),
        (73, 16, 0x97A2_6A3E),
        (78, 17, 0x97A2_6A3E),
        (83, 14, 0x290E_3597),
        (87, 15, 0x290E_3597),
    ];
    for &(idx, bt, crc) in ancres {
        let p = &cfg.poses[idx];
        assert_eq!(p.chara_body_type, bt, "charaBodyType[{idx}]");
        assert_eq!(p.motion_name_crc, HashId(crc), "motionNameCrc[{idx}]");
    }
}

#[test]
fn real_file_entree50() {
    let Some(cfg) = load_real() else { return };
    let p = &cfg.poses[50];
    assert_eq!(p.chara_body_type, 255, "charaBodyType[50]");
    assert_eq!(p.motion_name_crc, HashId(0x7D47_8513), "motionNameCrc[50]");
}

#[test]
fn real_file_derniere_entree() {
    let Some(cfg) = load_real() else { return };
    // m_randomPoseList[90] (dernier)
    let p = &cfg.poses[90];
    assert_eq!(p.chara_body_type, 255, "charaBodyType[90]");
    assert_eq!(p.motion_name_crc, HashId(0x192B_4017), "motionNameCrc[90]");
}

#[test]
fn real_file_generiques_255_compte() {
    let Some(cfg) = load_real() else { return };
    // 73 entrées génériques (charaBodyType=255) sur 91.
    assert_eq!(cfg.poses_for_body_type(255).len(), 73);
}

#[test]
fn real_file_body_type_0_unique() {
    let Some(cfg) = load_real() else { return };
    // charaBodyType=0 n'apparaît qu'une fois (entrée 0).
    assert_eq!(cfg.poses_for_body_type(0).len(), 1);
}

#[test]
fn real_file_find_by_motion() {
    let Some(cfg) = load_real() else { return };
    let found = cfg.find_by_motion(HashId(0xF8CC_8EDE));
    assert!(
        found.is_some(),
        "find_by_motion(0xF8CC8EDE) doit trouver l'entrée 0"
    );
    assert_eq!(found.unwrap().chara_body_type, 0);
}
