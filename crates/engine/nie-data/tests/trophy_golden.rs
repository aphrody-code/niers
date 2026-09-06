#![allow(clippy::pedantic)]
//! Tests golden `trophy` — valeurs réelles tirées de :
//! `trophy/trophy_config_0.00.00.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ (dump → valeur confirmée)
//!
//! ### Racine
//! - `TEXTURE_BASE_PATH_0`.var\[0\] = `"#/menu/220_img/activity_photo/<LG>/"`
//! - `TROPHY_TARGET_MAP_INFO_LIST_BEG_0` : liste vide (var\[0\]=0, 0 enfant)
//!
//! ### TROPHY_REWARD
//! - `TROPHY_REWARD_0` = `[310910628, 10]` → reward_id `0x12881EA4`, count 10
//! - `TROPHY_REWARD_1` = `[-1954459874, 10]` → reward_id `0x8B814F1E`
//! - `TROPHY_REWARD_383` (dernier) = `[56428312, 10]` → reward_id `0x035D0718`
//! - 384 récompenses au total
//!
//! ### TROPHY_TIER (+ ref reward)
//! - `TROPHY_TIER_0` = `[1, 1, -412693377, 0, -1, -1, -1, -1]`, ref reward `[0, 1]`
//!   → category 1, tier_no 1, target `0xE766CC7F`
//! - `TROPHY_TIER_1` = `[1, 2, -1597589841, 0, -1, -1, -1, -1]`, ref reward `[1, 1]`
//! - `TROPHY_TIER_435` (dernier) = `[0, 418, 11, 0, 51, 51, 51, 52]`, ref reward `[0, 0]`
//! - 436 paliers au total
//!
//! ### TROPHY_INFO (+ ref tier)
//! - `TROPHY_INFO_0` : category 1, sort_no 1, trophy_id `0x7D48804A`,
//!   code `"activity_story_miniquest_001"`, name_text_id `0x8840F978`,
//!   debug_name `"鍵をなくしたおじいさん"`, desc_text_id `0xC275B972`,
//!   open_cond `"AAAAAA8FNbkZNtoAAQAyAAAnTHE="`,
//!   photo `"activity_photo_01_001.g4tx"`, note `"activity_note_01_001.g4tx"`,
//!   ref tier `[0, 1]`
//! - `TROPHY_INFO_1` : trophy_id `0xE441D1F0`, code `"activity_story_miniquest_002"`,
//!   debug_name `"今日は大漁"`, ref tier `[1, 1]`
//! - `TROPHY_INFO_229` (dernier) : category 4, sort_no 1051, trophy_id `0x65338D74`,
//!   code `"trophy_collection_13"`, debug_name `"パワフルイレブン"`,
//!   open_cond `"0"` (sentinelle absente), photo `"0"`, note `"0"`, ref tier `[435, 1]`
//! - 230 trophées au total

mod common;

use nie_data::hash::HashId;
use nie_data::trophy::{TrophyConfig, parse_trophy_config};
use serde_json::json;

// ─── Fixture synthétique (valeurs tirées du dump réel) ─────────────────────────

fn var_int(v: i64) -> serde_json::Value {
    json!({ "type": "Int", "value": v.to_string() })
}
fn var_str(s: &str) -> serde_json::Value {
    json!({ "type": "String", "value": s })
}

fn fixture() -> serde_json::Value {
    json!({
        "entries": [
            { "name": "TROPHY_TARGET_MAP_INFO_LIST_BEG_0", "variables": [var_int(0)], "children": [] },
            { "name": "TEXTURE_BASE_PATH_0", "variables": [var_str("#/menu/220_img/activity_photo/<LG>/")], "children": [] },
            {
                "name": "TROPHY_REWARD_LIST_BEG_0",
                "variables": [var_int(0)],
                "children": [
                    { "name": "TROPHY_REWARD_0", "variables": [var_int(310910628), var_int(10)], "children": [] },
                    { "name": "TROPHY_REWARD_1", "variables": [var_int(-1954459874_i64), var_int(10)], "children": [] }
                ]
            },
            {
                "name": "TROPHY_TIER_LIST_BEG_0",
                "variables": [var_int(0)],
                "children": [
                    { "name": "TROPHY_TIER_0", "variables": [
                        var_int(1), var_int(1), var_int(-412693377), var_int(0),
                        var_int(-1), var_int(-1), var_int(-1), var_int(-1)
                    ], "children": [] },
                    { "name": "TROPHY_TIER_REF_REWARD_0", "variables": [var_int(0), var_int(1)], "children": [] },
                    { "name": "TROPHY_TIER_1", "variables": [
                        var_int(1), var_int(2), var_int(-1597589841_i64), var_int(0),
                        var_int(-1), var_int(-1), var_int(-1), var_int(-1)
                    ], "children": [] },
                    { "name": "TROPHY_TIER_REF_REWARD_1", "variables": [var_int(1), var_int(1)], "children": [] }
                ]
            },
            {
                "name": "TROPHY_INFO_LIST_BEG_0",
                "variables": [var_int(0), var_int(0)],
                "children": [
                    { "name": "TROPHY_INFO_0", "variables": [
                        var_int(1), var_int(1), var_int(2101903434_i64),
                        var_str("activity_story_miniquest_001"),
                        var_int(-2009007752_i64), var_str("鍵をなくしたおじいさん"),
                        var_int(-1032472206_i64), var_int(0),
                        var_str("AAAAAA8FNbkZNtoAAQAyAAAnTHE="),
                        var_int(-828788701), var_int(1281576700), var_int(104384077),
                        var_int(2), var_int(1),
                        var_str("activity_photo_01_001.g4tx"),
                        var_str("activity_note_01_001.g4tx"),
                        var_int(1), var_int(0)
                    ], "children": [] },
                    { "name": "TROPHY_INFO_REF_TIER_0", "variables": [var_int(0), var_int(1)], "children": [] }
                ]
            }
        ]
    })
}

#[test]
fn fixture_texture_base_path() {
    let cfg = parse_trophy_config(&fixture());
    assert_eq!(cfg.texture_base_path, "#/menu/220_img/activity_photo/<LG>/");
}

#[test]
fn fixture_rewards() {
    let cfg = parse_trophy_config(&fixture());
    assert_eq!(cfg.rewards.len(), 2);
    assert_eq!(cfg.rewards[0].reward_id, HashId(0x1288_1EA4));
    assert_eq!(cfg.rewards[0].count, 10);
    assert_eq!(cfg.rewards[1].reward_id, HashId(0x8B81_4F1E));
}

#[test]
fn fixture_tier_avec_ref_reward() {
    let cfg = parse_trophy_config(&fixture());
    assert_eq!(cfg.tiers.len(), 2);
    let t = &cfg.tiers[0];
    assert_eq!(t.category, 1);
    assert_eq!(t.tier_no, 1);
    assert_eq!(t.target, -412693377);
    assert_eq!(t.target_hash(), HashId(0xE766_CC7F));
    assert_eq!(t.reserved, 0);
    assert_eq!(t.aux, vec![-1, -1, -1, -1]);
    assert_eq!(t.reward_offset, 0);
    assert_eq!(t.reward_count, 1);
    // ref reward du tier 1
    assert_eq!(cfg.tiers[1].reward_offset, 1);
}

#[test]
fn fixture_info_complet() {
    let cfg = parse_trophy_config(&fixture());
    assert_eq!(cfg.infos.len(), 1);
    let i = &cfg.infos[0];
    assert_eq!(i.category, 1);
    assert_eq!(i.sort_no, 1);
    assert_eq!(i.trophy_id, HashId(0x7D48_804A));
    assert_eq!(i.code, "activity_story_miniquest_001");
    assert_eq!(i.name_text_id, HashId(0x8840_F978));
    assert_eq!(i.debug_name, "鍵をなくしたおじいさん");
    assert_eq!(i.desc_text_id, HashId(0xC275_B972));
    assert_eq!(i.open_cond, "AAAAAA8FNbkZNtoAAQAyAAAnTHE=");
    assert_eq!(i.photo_tex_file_name, "activity_photo_01_001.g4tx");
    assert_eq!(i.note_tex_file_name, "activity_note_01_001.g4tx");
    assert_eq!(i.tier_offset, 0);
    assert_eq!(i.tier_count, 1);
}

#[test]
fn fixture_resolution_jointures() {
    let cfg = parse_trophy_config(&fixture());
    let info = &cfg.infos[0];
    let tiers = cfg.tiers_of(info);
    assert_eq!(tiers.len(), 1);
    let rewards = cfg.rewards_of(&tiers[0]);
    assert_eq!(rewards.len(), 1);
    assert_eq!(rewards[0].reward_id, HashId(0x1288_1EA4));
}

#[test]
fn fixture_target_map_vide_ne_panique_pas() {
    // La liste TROPHY_TARGET_MAP est vide dans le dump réel : le parse doit l'ignorer.
    let cfg = parse_trophy_config(&fixture());
    assert!(!cfg.rewards.is_empty());
}

// ─── Test sur le vrai fichier (skip si absent du VPS) ────────────────────────

const REAL_PATH: &str = "trophy/trophy_config_0.00.00.00.cfg.bin.json";

fn load_real() -> Option<TrophyConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_trophy_config(&root))
}

#[test]
fn real_file_comptes_totaux() {
    let Some(cfg) = load_real() else { return };
    assert_eq!(cfg.rewards.len(), 384, "384 récompenses");
    assert_eq!(cfg.tiers.len(), 436, "436 paliers");
    assert_eq!(cfg.infos.len(), 230, "230 trophées");
}

#[test]
fn real_file_texture_base_path() {
    let Some(cfg) = load_real() else { return };
    assert_eq!(cfg.texture_base_path, "#/menu/220_img/activity_photo/<LG>/");
}

#[test]
fn real_file_reward_premier_et_dernier() {
    let Some(cfg) = load_real() else { return };
    assert_eq!(cfg.rewards[0].reward_id, HashId(0x1288_1EA4), "reward[0]");
    assert_eq!(cfg.rewards[0].count, 10);
    assert_eq!(cfg.rewards[1].reward_id, HashId(0x8B81_4F1E), "reward[1]");
    assert_eq!(
        cfg.rewards[383].reward_id,
        HashId(0x035D_0718),
        "reward[383]"
    );
    // la quantité varie (1, 2, 3, 5, 10, 15, 20) : reward[9] = id 0x62E2EA2B, count 5
    assert_eq!(cfg.rewards[9].reward_id, HashId(0x62E2_EA2B), "reward[9]");
    assert_eq!(
        cfg.rewards[9].count, 5,
        "reward[9].count = 5 (pas toujours 10)"
    );
}

#[test]
fn real_file_tier_premier() {
    let Some(cfg) = load_real() else { return };
    let t = &cfg.tiers[0];
    assert_eq!(t.category, 1);
    assert_eq!(t.tier_no, 1);
    assert_eq!(t.target, -412693377);
    assert_eq!(t.target_hash(), HashId(0xE766_CC7F));
    assert_eq!(t.aux, vec![-1, -1, -1, -1]);
    assert_eq!(t.reward_offset, 0);
    assert_eq!(t.reward_count, 1);
}

#[test]
fn real_file_tier_second_et_dernier() {
    let Some(cfg) = load_real() else { return };
    assert_eq!(cfg.tiers[1].tier_no, 2);
    assert_eq!(cfg.tiers[1].target, -1597589841);
    assert_eq!(cfg.tiers[1].reward_offset, 1);
    let last = &cfg.tiers[435];
    assert_eq!(last.category, 0);
    assert_eq!(last.tier_no, 418);
    assert_eq!(last.target, 11);
    assert_eq!(last.aux, vec![51, 51, 51, 52]);
}

#[test]
fn real_file_info_premier() {
    let Some(cfg) = load_real() else { return };
    let i = &cfg.infos[0];
    assert_eq!(i.category, 1);
    assert_eq!(i.sort_no, 1);
    assert_eq!(i.trophy_id, HashId(0x7D48_804A));
    assert_eq!(i.code, "activity_story_miniquest_001");
    assert_eq!(i.name_text_id, HashId(0x8840_F978));
    assert_eq!(i.debug_name, "鍵をなくしたおじいさん");
    assert_eq!(i.desc_text_id, HashId(0xC275_B972));
    assert_eq!(i.open_cond, "AAAAAA8FNbkZNtoAAQAyAAAnTHE=");
    assert_eq!(i.photo_tex_file_name, "activity_photo_01_001.g4tx");
    assert_eq!(i.note_tex_file_name, "activity_note_01_001.g4tx");
    assert_eq!(i.tier_offset, 0);
    assert_eq!(i.tier_count, 1);
}

#[test]
fn real_file_info_second() {
    let Some(cfg) = load_real() else { return };
    let i = &cfg.infos[1];
    assert_eq!(i.trophy_id, HashId(0xE441_D1F0));
    assert_eq!(i.code, "activity_story_miniquest_002");
    assert_eq!(i.debug_name, "今日は大漁");
    assert_eq!(i.tier_offset, 1);
    assert_eq!(i.tier_count, 1);
}

#[test]
fn real_file_info_dernier_sentinelles_absentes() {
    let Some(cfg) = load_real() else { return };
    let i = &cfg.infos[229];
    assert_eq!(i.category, 4);
    assert_eq!(i.sort_no, 1051);
    assert_eq!(i.trophy_id, HashId(0x6533_8D74));
    assert_eq!(i.code, "trophy_collection_13");
    assert_eq!(i.debug_name, "パワフルイレブン");
    // sentinelles "0" (var de type Int) pour les champs textuels absents
    assert_eq!(i.open_cond, "0");
    assert_eq!(i.photo_tex_file_name, "0");
    assert_eq!(i.note_tex_file_name, "0");
    assert_eq!(i.cond_hash_a, HashId::ZERO);
    assert_eq!(i.tier_offset, 435);
    assert_eq!(i.tier_count, 1);
}

#[test]
fn real_file_jointure_info_tier_reward() {
    let Some(cfg) = load_real() else { return };
    // info[0] → tier[0] → reward[0]
    let info = &cfg.infos[0];
    let tiers = cfg.tiers_of(info);
    assert_eq!(tiers.len(), 1);
    assert_eq!(tiers[0].tier_no, 1);
    let rewards = cfg.rewards_of(&tiers[0]);
    assert_eq!(rewards.len(), 1);
    assert_eq!(rewards[0].reward_id, HashId(0x1288_1EA4));
}

#[test]
fn real_file_plages_tuilent_les_listes() {
    let Some(cfg) = load_real() else { return };
    // La somme des tier_count des trophées = nombre total de paliers.
    let total_tiers: i64 = cfg.infos.iter().map(|i| i.tier_count).sum();
    assert_eq!(
        total_tiers as usize,
        cfg.tiers.len(),
        "les trophées tuilent les paliers"
    );
    // La somme des reward_count des paliers = nombre total de récompenses.
    let total_rewards: i64 = cfg.tiers.iter().map(|t| t.reward_count).sum();
    assert_eq!(
        total_rewards as usize,
        cfg.rewards.len(),
        "les paliers tuilent les récompenses"
    );
}

#[test]
fn real_file_find_info_by_code() {
    let Some(cfg) = load_real() else { return };
    let found = cfg.find_info_by_code("activity_story_miniquest_002");
    assert!(found.is_some());
    assert_eq!(found.unwrap().trophy_id, HashId(0xE441_D1F0));
}

#[test]
fn real_file_find_info_by_trophy_id() {
    let Some(cfg) = load_real() else { return };
    let found = cfg.find_info_by_trophy_id(HashId(0x7D48_804A));
    assert!(found.is_some());
    assert_eq!(found.unwrap().code, "activity_story_miniquest_001");
}
