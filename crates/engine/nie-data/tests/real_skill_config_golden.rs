#![allow(clippy::pedantic)]
//! Tests golden `real_skill_config` — VRAIES valeurs tirées du VFS IEVR :
//! `data/common/gamedata/skill/real_skill_config_1.03.74.00.cfg.bin`.
//!
//! Extraction : `cargo run -p nie-game --example extract_real_skill` (RDBN → JSON iecode
//! `lists`, via `cfgbin::read_values`). Port 1:1 d'inagle
//! `packages/inagle/src/parsers/real-skill-config.ts`. Vérité terrain = la sortie du dump.
//!
//! Le dump a 2 listes : `m_RealSkillShootCourseInfoList` (6 courbes) et
//! `m_RealSkillInfoList` (19 tirs). La fixture embarque les 6 courbes + un sous-ensemble
//! représentatif de tirs (dont ceux à `shootCourseInfoRef` non trivial).

use nie_data::hash::HashId;
use nie_data::real_skill_config::{RealSkillConfig, parse_real_skill_config};
use serde_json::json;

/// Fixture = valeurs exactes du dump `real_skill_config_1.03.74.00.cfg.bin`.
fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_RealSkillShootCourseInfoList",
                "typeName": "RealSkillShootCourseInfo",
                "values": [
                    { "targetRate": 1.0, "isGroundTarget": false, "targetHeightOffset": 0.0,
                      "targetHoriOffset": 0.0, "curveRate": 0.699999988079071,
                      "curveHeightRate": 0.5, "curveAngle": 65.0, "moveTimeRate": 1.0 },
                    { "targetRate": 1.0, "isGroundTarget": false, "targetHeightOffset": 0.0,
                      "targetHoriOffset": 0.0, "curveRate": 0.699999988079071,
                      "curveHeightRate": 0.5, "curveAngle": 0.0, "moveTimeRate": 1.0 },
                    { "targetRate": 0.5, "isGroundTarget": true, "targetHeightOffset": 0.0,
                      "targetHoriOffset": 2.0, "curveRate": 0.5, "curveHeightRate": 0.0,
                      "curveAngle": 0.0, "moveTimeRate": 0.6500000357627869 },
                    { "targetRate": 1.0, "isGroundTarget": false, "targetHeightOffset": 0.0,
                      "targetHoriOffset": 0.0, "curveRate": 0.5,
                      "curveHeightRate": 0.10000000149011612, "curveAngle": 0.0,
                      "moveTimeRate": 0.550000011920929 },
                    { "targetRate": 1.0, "isGroundTarget": false, "targetHeightOffset": 0.0,
                      "targetHoriOffset": 0.0, "curveRate": 0.5, "curveHeightRate": 0.0,
                      "curveAngle": 0.0, "moveTimeRate": 0.800000011920929 },
                    { "targetRate": 1.0, "isGroundTarget": true, "targetHeightOffset": 0.0,
                      "targetHoriOffset": 0.0, "curveRate": 0.5, "curveHeightRate": 0.0,
                      "curveAngle": 0.0, "moveTimeRate": 1.0 }
                ]
            },
            {
                "name": "m_RealSkillInfoList",
                "typeName": "RealSkillInfo",
                "values": [
                    { "id": "0xA2123954", "loseType": 0, "formationType": 0,
                      "formationCharaLen": 3.0, "shootLimitBallHeightAttr": 0,
                      "shootGroundingEffectName": "0x00000000",
                      "shootGroundingEffectIntervalTime": -1.0, "shootGroundingEffectScale": 1.0,
                      "shootGroundingEffectScatteringPos": 0.0, "shootCourseInfoRef": [0, 0] },
                    { "id": "0xA8B5FC00", "loseType": 0, "formationType": 0,
                      "formationCharaLen": 5.0, "shootLimitBallHeightAttr": 0,
                      "shootGroundingEffectName": "0x00000000",
                      "shootGroundingEffectIntervalTime": -1.0, "shootGroundingEffectScale": 1.0,
                      "shootGroundingEffectScatteringPos": 0.0, "shootCourseInfoRef": [0, 0] },
                    { "id": "0x070243D7", "loseType": 0, "formationType": 0,
                      "formationCharaLen": 3.0, "shootLimitBallHeightAttr": 0,
                      "shootGroundingEffectName": "0x00000000",
                      "shootGroundingEffectIntervalTime": -1.0, "shootGroundingEffectScale": 1.0,
                      "shootGroundingEffectScatteringPos": 0.0, "shootCourseInfoRef": [0, 1] },
                    { "id": "0x2C2F1014", "loseType": 0, "formationType": 0,
                      "formationCharaLen": 3.0, "shootLimitBallHeightAttr": 0,
                      "shootGroundingEffectName": "0x00000000",
                      "shootGroundingEffectIntervalTime": -1.0, "shootGroundingEffectScale": 1.0,
                      "shootGroundingEffectScatteringPos": 0.0, "shootCourseInfoRef": [1, 1] },
                    { "id": "0x35342155", "loseType": 0, "formationType": 0,
                      "formationCharaLen": 3.0, "shootLimitBallHeightAttr": 1,
                      "shootGroundingEffectName": "0x00000000",
                      "shootGroundingEffectIntervalTime": -1.0, "shootGroundingEffectScale": 1.0,
                      "shootGroundingEffectScatteringPos": 0.0, "shootCourseInfoRef": [2, 2] },
                    { "id": "0x636E86D3", "loseType": 0, "formationType": 0,
                      "formationCharaLen": 3.0, "shootLimitBallHeightAttr": 6,
                      "shootGroundingEffectName": "0x1C8B0F11",
                      "shootGroundingEffectIntervalTime": 0.02499999850988388,
                      "shootGroundingEffectScale": 5.0,
                      "shootGroundingEffectScatteringPos": 0.5, "shootCourseInfoRef": [5, 1] }
                ]
            }
        ]
    })
}

fn parsed() -> RealSkillConfig {
    parse_real_skill_config(&fixture())
}

#[test]
fn comptes_listes() {
    let cfg = parsed();
    assert_eq!(cfg.courses.len(), 6, "6 courbes de tir dans le dump");
    assert_eq!(
        cfg.skills.len(),
        6,
        "sous-ensemble de 6 tirs dans la fixture"
    );
}

#[test]
fn course0_valeurs_exactes() {
    let cfg = parsed();
    let c = &cfg.courses[0];
    assert_eq!(c.target_rate, 1.0);
    assert!(!c.is_ground_target);
    assert_eq!(c.target_height_offset, 0.0);
    assert_eq!(c.target_hori_offset, 0.0);
    assert_eq!(c.curve_rate, 0.699999988079071); // f32 0.7 élargi en f64
    assert_eq!(c.curve_height_rate, 0.5);
    assert_eq!(c.curve_angle, 65.0);
    assert_eq!(c.move_time_rate, 1.0);
}

#[test]
fn course2_cible_au_sol() {
    let cfg = parsed();
    let c = &cfg.courses[2];
    assert!(c.is_ground_target, "course[2] = tir rasant");
    assert_eq!(c.target_rate, 0.5);
    assert_eq!(c.target_hori_offset, 2.0);
    assert_eq!(c.move_time_rate, 0.6500000357627869); // f32 0.65
}

#[test]
fn skill_premier_et_formation_chara_len_float() {
    let cfg = parsed();
    let s = &cfg.skills[0];
    assert_eq!(s.id, HashId(0xA212_3954));
    assert_eq!(s.lose_type, 0);
    assert_eq!(s.formation_type, 0);
    assert_eq!(s.formation_chara_len, 3.0);
    assert_eq!(s.shoot_limit_ball_height_attr, 0);
    assert_eq!(s.shoot_grounding_effect_name, HashId::ZERO);
    assert_eq!(s.shoot_grounding_effect_interval_time, -1.0);
    assert_eq!(s.shoot_course_info_ref, [0, 0]);

    // Tir à formation à 5 personnages.
    assert_eq!(cfg.skills[1].id, HashId(0xA8B5_FC00));
    assert_eq!(cfg.skills[1].formation_chara_len, 5.0);
}

#[test]
fn skill_effet_au_sol_reel() {
    // 0x636E86D3 : seul tir du dump avec un effet au sol réel (hash non nul).
    let cfg = parsed();
    let s = cfg.find_skill(HashId(0x636E_86D3)).expect("tir 0x636E86D3");
    assert_eq!(s.shoot_grounding_effect_name, HashId(0x1C8B_0F11));
    assert_eq!(s.shoot_grounding_effect_interval_time, 0.02499999850988388); // f32 0.025
    assert_eq!(s.shoot_grounding_effect_scale, 5.0);
    assert_eq!(s.shoot_grounding_effect_scattering_pos, 0.5);
    assert_eq!(s.shoot_limit_ball_height_attr, 6);
    assert_eq!(s.shoot_course_info_ref, [5, 1]);
}

#[test]
fn shoot_course_info_ref_tranche_les_courbes() {
    // Sémantique getCoursesForSkill d'inagle : courses[start .. start+count].
    let cfg = parsed();

    // [0, 0] → aucune courbe.
    assert!(cfg.courses_for_skill(HashId(0xA212_3954)).is_empty());

    // 0x2C2F1014 = [1, 1] → la course[1] uniquement.
    let one = cfg.courses_for_skill(HashId(0x2C2F_1014));
    assert_eq!(one.len(), 1);
    assert_eq!(one[0], cfg.courses[1]);

    // 0x35342155 = [2, 2] → courses[2..4].
    let two = cfg.courses_for_skill(HashId(0x3534_2155));
    assert_eq!(two.len(), 2);
    assert_eq!(two[0], cfg.courses[2]);
    assert_eq!(two[1], cfg.courses[3]);

    // 0x636E86D3 = [5, 1] → la dernière course (index 5).
    let last = cfg.courses_for_skill(HashId(0x636E_86D3));
    assert_eq!(last.len(), 1);
    assert_eq!(last[0], cfg.courses[5]);
}

#[test]
fn skill_inconnu_aucune_courbe() {
    let cfg = parsed();
    assert!(cfg.find_skill(HashId(0xDEAD_BEEF)).is_none());
    assert!(cfg.courses_for_skill(HashId(0xDEAD_BEEF)).is_empty());
}

#[test]
fn liste_manquante_vide() {
    let root = json!({ "version": 100, "lists": [] });
    let cfg = parse_real_skill_config(&root);
    assert!(cfg.courses.is_empty());
    assert!(cfg.skills.is_empty());
}

// ─── Test sur le vrai fichier VFS (skip si Steam absent) ────────────────────────

#[test]
fn vrai_fichier_vfs_si_present() {
    // Recalcul byte-exact contre le cfg.bin réel du VFS Steam. Skip si absent
    // (le jeu n'est pas installé sur tous les hôtes). Le dump JSON committé n'existe
    // pas ici : on relit le JSON pretty extrait si présent, sinon on saute.
    let path = "/tmp/real_skill_dump.json";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let content = std::fs::read_to_string(path).unwrap();
    let root: serde_json::Value = serde_json::from_str(&content).unwrap();
    let cfg = parse_real_skill_config(&root);
    assert_eq!(cfg.courses.len(), 6, "6 courbes dans le vrai fichier");
    assert_eq!(cfg.skills.len(), 19, "19 tirs dans le vrai fichier");
    // Le 1er tir et la 1re courbe doivent matcher la fixture.
    assert_eq!(cfg.skills[0].id, HashId(0xA212_3954));
    assert_eq!(cfg.courses[0].curve_angle, 65.0);
    assert_eq!(
        cfg.find_skill(HashId(0x636E_86D3))
            .unwrap()
            .shoot_course_info_ref,
        [5, 1]
    );
}
