#![allow(clippy::pedantic)]
//! Tests golden `scene_archive` — valeurs réelles tirées de :
//! `scene_archive/scene_archive_config_4.00.18.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ (fichier → valeur confirmée)
//!
//! ### m_sceneArchiveFlags
//! - `[0].activeTempBitFlagCrc` = `"0xADCDB833"` → `HashId(0xADCDB833)`
//! - `[1].activeTempBitFlagCrc` = `"0x584D1EF3"` → `HashId(0x584D1EF3)`
//! - `[3].activeTempBitFlagCrc` = `"0xBAA05350"` → `HashId(0xBAA05350)`
//!
//! ### m_sceneArchiveDataList[0]
//! - `id`                     = `"0xF86C637D"` → `HashId(0xF86C637D)`
//! - `category`               = `0`
//! - `flag_num`               = `1`
//! - `text_id_title`          = `"0x35B80566"` → `HashId(0x35B80566)`
//! - `text_id_explain`        = `"0x00000000"` → `HashId(0)`
//! - `event_type`             = `0`
//! - `event_id_text`          = `"ev01_00050"`
//! - `map_id`                 = `"0x00000000"` → `HashId(0)`
//! - `chapter_no`             = `0`
//! - `thumbnail_texture_name` = `"0x412B0428"` → `HashId(0x412B0428)`
//! - `thumbnail_texture_path` = `"#/menu/220_img/theater_img/theater_img01_02.g4tx"`
//! - `condition`              = `"AAAAABgFNSo9RUMACgEoAAYCNNG3Zz4yAAAAAXg="`
//! - `activeFlags`            = `[0, 0]`
//!
//! ### m_sceneArchiveDataList[4]
//! - `id`          = `"0x9B090EA5"` → `HashId(0x9B090EA5)`
//! - `map_id`      = `"0x4E7B90D9"` → `HashId(0x4E7B90D9)`
//! - `event_id_text` = `"ev01_00410"`
//! - `activeFlags` = `[0, 1]`
//!
//! ### m_sceneArchiveDataList[20]
//! - `id`          = `"0x0CB2C284"` → `HashId(0x0CB2C284)`
//! - `chapter_no`  = `1`
//! - `map_tag_id`  = `"0x11F87078"` → `HashId(0x11F87078)`
//! - `activeFlags` = `[2, 1]`
//!
//! ### m_sceneArchiveDataList[111] (dernier)
//! - `id`          = `"0xEE2B0CA6"` → `HashId(0xEE2B0CA6)`
//! - `category`    = `2`
//! - `event_id_text` = `"ev20_00090"`
//! - `thumbnail_texture_name` = `"0x02416996"` → `HashId(0x02416996)`

mod common;

use nie_data::hash::HashId;
use nie_data::scene_archive::{SceneArchiveConfig, SceneArchiveData, parse_scene_archive_config};
use serde_json::json;

// ─── Fixture ──────────────────────────────────────────────────────────────────

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_sceneArchiveFlags",
                "typeName": "SCENE_ARCHIVE_FLAGS",
                "values": [
                    // [0] — scene_archive_config_4.00.18.00, m_sceneArchiveFlags[0]
                    { "activeTempBitFlagCrc": "0xADCDB833" },
                    // [1] — m_sceneArchiveFlags[1]
                    { "activeTempBitFlagCrc": "0x584D1EF3" },
                    // [3] — m_sceneArchiveFlags[3]
                    { "activeTempBitFlagCrc": "0xBAA05350" }
                ]
            },
            {
                "name": "m_sceneArchiveDataList",
                "typeName": "SCENE_ARCHIVE_LIST_DATA",
                "values": [
                    // [0] — scene_archive_config_4.00.18.00, m_sceneArchiveDataList[0]
                    {
                        "id": "0xF86C637D",
                        "category": 0,
                        "flag_num": 1,
                        "text_id_title": "0x35B80566",
                        "text_id_explain": "0x00000000",
                        "event_type": 0,
                        "event_id_text": "ev01_00050",
                        "map_id": "0x00000000",
                        "map_tag_id": "0x00000000",
                        "map_jump_pos_x": 0,
                        "map_jump_pos_y": 0,
                        "map_jump_pos_z": 0,
                        "chapter_no": 0,
                        "is_full_screen_view": false,
                        "thumbnail_texture_name": "0x412B0428",
                        "thumbnail_texture_path": "#/menu/220_img/theater_img/theater_img01_02.g4tx",
                        "condition": "AAAAABgFNSo9RUMACgEoAAYCNNG3Zz4yAAAAAXg=",
                        "activeFlags": [0, 0]
                    },
                    // [4] — m_sceneArchiveDataList[4] : activeFlags=[0,1], map_id non-nul
                    {
                        "id": "0x9B090EA5",
                        "category": 0,
                        "flag_num": 1,
                        "text_id_title": "0x56DD68BE",
                        "text_id_explain": "0x00000000",
                        "event_type": 0,
                        "event_id_text": "ev01_00410",
                        "map_id": "0x4E7B90D9",
                        "map_tag_id": "0x00000000",
                        "map_jump_pos_x": 0,
                        "map_jump_pos_y": 0,
                        "map_jump_pos_z": 0,
                        "chapter_no": 0,
                        "is_full_screen_view": false,
                        "thumbnail_texture_name": "0x4646C031",
                        "thumbnail_texture_path": "#/menu/220_img/theater_img/theater_img01_06.g4tx",
                        "condition": "AAAAABgFNSo9RUMACgEoAAYCNLMQYNEyAAAAAXg=",
                        "activeFlags": [0, 1]
                    },
                    // [20] — m_sceneArchiveDataList[20] : chapter_no=1, map_tag_id non-nul
                    {
                        "id": "0x0CB2C284",
                        "category": 0,
                        "flag_num": 1,
                        "text_id_title": "0xC9BE37CC",
                        "text_id_explain": "0x00000000",
                        "event_type": 0,
                        "event_id_text": "ev01_04500",
                        "map_id": "0x4E7B90D9",
                        "map_tag_id": "0x11F87078",
                        "map_jump_pos_x": 0,
                        "map_jump_pos_y": 0,
                        "map_jump_pos_z": 0,
                        "chapter_no": 1,
                        "is_full_screen_view": false,
                        "thumbnail_texture_name": "0x09E0EEDB",
                        "thumbnail_texture_path": "#/menu/220_img/theater_img/theater_img01_91.g4tx",
                        "condition": "AAAAABgFNSo9RUMACgEoAAYCNCSrrPAyAAAAAXg=",
                        "activeFlags": [2, 1]
                    },
                    // [111] — m_sceneArchiveDataList[111] : category=2, side story
                    {
                        "id": "0xEE2B0CA6",
                        "category": 2,
                        "flag_num": 1,
                        "text_id_title": "0x23FF6ABD",
                        "text_id_explain": "0x00000000",
                        "event_type": 0,
                        "event_id_text": "ev20_00090",
                        "map_id": "0x00000000",
                        "map_tag_id": "0x00000000",
                        "map_jump_pos_x": 0,
                        "map_jump_pos_y": 0,
                        "map_jump_pos_z": 0,
                        "chapter_no": 0,
                        "is_full_screen_view": false,
                        "thumbnail_texture_name": "0x02416996",
                        "thumbnail_texture_path": "#/menu/220_img/theater_img/theater_img03_04.g4tx",
                        "condition": "AAAAACEFNcl4Pb8AEwIoAAYCNJAYVCcoAAYCMgAAAAIyAAAAAXg=",
                        "activeFlags": [0, 0]
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn fixture_compte_flags() {
    let cfg = parse_scene_archive_config(&fixture());
    // 3 flags dans la fixture
    assert_eq!(cfg.flags.len(), 3);
}

#[test]
fn fixture_flags_valeurs() {
    let cfg = parse_scene_archive_config(&fixture());

    // m_sceneArchiveFlags[0].activeTempBitFlagCrc = "0xADCDB833"
    assert_eq!(
        cfg.flags[0].active_temp_bit_flag_crc,
        HashId(0xADCDB833),
        "flags[0]"
    );
    // m_sceneArchiveFlags[1].activeTempBitFlagCrc = "0x584D1EF3"
    assert_eq!(
        cfg.flags[1].active_temp_bit_flag_crc,
        HashId(0x584D1EF3),
        "flags[1]"
    );
    // m_sceneArchiveFlags[3].activeTempBitFlagCrc = "0xBAA05350"
    assert_eq!(
        cfg.flags[2].active_temp_bit_flag_crc,
        HashId(0xBAA05350),
        "flags[2]"
    );
}

#[test]
fn fixture_compte_scenes() {
    let cfg = parse_scene_archive_config(&fixture());
    assert_eq!(cfg.scenes.len(), 4);
}

#[test]
fn fixture_scene0_champs_complets() {
    let cfg = parse_scene_archive_config(&fixture());
    let s: &SceneArchiveData = &cfg.scenes[0];

    // scene_archive_config_4.00.18.00 : m_sceneArchiveDataList[0]
    assert_eq!(s.id, HashId(0xF86C637D), "id[0]");
    assert_eq!(s.category, 0, "category[0]");
    assert_eq!(s.flag_num, 1, "flag_num[0]");
    assert_eq!(s.text_id_title, HashId(0x35B80566), "text_id_title[0]");
    assert_eq!(s.text_id_explain, HashId(0), "text_id_explain[0]");
    assert_eq!(s.event_type, 0, "event_type[0]");
    assert_eq!(s.event_id_text, "ev01_00050", "event_id_text[0]");
    assert_eq!(s.map_id, HashId(0), "map_id[0]");
    assert_eq!(s.map_tag_id, HashId(0), "map_tag_id[0]");
    assert_eq!(s.map_jump_pos_x, 0, "map_jump_pos_x[0]");
    assert_eq!(s.map_jump_pos_y, 0, "map_jump_pos_y[0]");
    assert_eq!(s.map_jump_pos_z, 0, "map_jump_pos_z[0]");
    assert_eq!(s.chapter_no, 0, "chapter_no[0]");
    assert!(!s.is_full_screen_view, "is_full_screen_view[0]");
    assert_eq!(
        s.thumbnail_texture_name,
        HashId(0x412B0428),
        "thumbnail_texture_name[0]"
    );
    assert_eq!(
        s.thumbnail_texture_path, "#/menu/220_img/theater_img/theater_img01_02.g4tx",
        "thumbnail_texture_path[0]"
    );
    assert_eq!(
        s.condition, "AAAAABgFNSo9RUMACgEoAAYCNNG3Zz4yAAAAAXg=",
        "condition[0]"
    );
    assert_eq!(s.active_flags, vec![0_i64, 0], "activeFlags[0]");
}

#[test]
fn fixture_scene1_active_flags_non_nuls() {
    let cfg = parse_scene_archive_config(&fixture());
    let s = &cfg.scenes[1];

    // m_sceneArchiveDataList[4] : activeFlags=[0,1], map_id non-nul
    assert_eq!(s.id, HashId(0x9B090EA5), "id[4]");
    assert_eq!(s.map_id, HashId(0x4E7B90D9), "map_id[4]");
    assert_eq!(s.event_id_text, "ev01_00410", "event_id_text[4]");
    assert_eq!(s.active_flags, vec![0_i64, 1], "activeFlags[4]");
}

#[test]
fn fixture_scene2_chapter_no() {
    let cfg = parse_scene_archive_config(&fixture());
    let s = &cfg.scenes[2];

    // m_sceneArchiveDataList[20] : chapter_no=1, map_tag_id non-nul
    assert_eq!(s.id, HashId(0x0CB2C284), "id[20]");
    assert_eq!(s.chapter_no, 1, "chapter_no[20]");
    assert_eq!(s.map_tag_id, HashId(0x11F87078), "map_tag_id[20]");
    assert_eq!(s.active_flags, vec![2_i64, 1], "activeFlags[20]");
}

#[test]
fn fixture_scene3_category_side_story() {
    let cfg = parse_scene_archive_config(&fixture());
    let s = &cfg.scenes[3];

    // m_sceneArchiveDataList[111] : category=2 (side story)
    assert_eq!(s.id, HashId(0xEE2B0CA6), "id[111]");
    assert_eq!(s.category, 2, "category[111]");
    assert_eq!(s.event_id_text, "ev20_00090", "event_id_text[111]");
    assert_eq!(
        s.thumbnail_texture_name,
        HashId(0x02416996),
        "thumbnail_texture_name[111]"
    );
}

#[test]
fn fixture_find_by_id_present() {
    let cfg = parse_scene_archive_config(&fixture());
    let found = cfg.find_by_id(HashId(0xF86C637D));
    assert!(found.is_some(), "find_by_id doit trouver 0xF86C637D");
    assert_eq!(found.unwrap().event_id_text, "ev01_00050");
}

#[test]
fn fixture_find_by_id_absent() {
    let cfg = parse_scene_archive_config(&fixture());
    assert!(cfg.find_by_id(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn fixture_filter_by_chapter() {
    let cfg = parse_scene_archive_config(&fixture());
    // chapter_no=1 → seulement la scène [20]
    let ch1 = cfg.filter_by_chapter(1);
    assert_eq!(ch1.len(), 1, "un seul chapter_no=1 dans la fixture");
    assert_eq!(ch1[0].id, HashId(0x0CB2C284));

    // chapter_no=0 → scènes [0], [4] et [111] de la fixture
    let ch0 = cfg.filter_by_chapter(0);
    assert_eq!(ch0.len(), 3, "trois scènes chapter_no=0 dans la fixture");
}

#[test]
fn fixture_filter_by_category() {
    let cfg = parse_scene_archive_config(&fixture());
    // category=2 → seulement la scène [111]
    let side = cfg.filter_by_category(2);
    assert_eq!(side.len(), 1, "une seule scène category=2 dans la fixture");
    assert_eq!(side[0].event_id_text, "ev20_00090");

    // category=0 → trois scènes
    let main = cfg.filter_by_category(0);
    assert_eq!(main.len(), 3, "trois scènes category=0 dans la fixture");
}

#[test]
fn fixture_listes_absentes_renvoient_vide() {
    let root = serde_json::json!({ "version": 100, "lists": [] });
    let cfg = parse_scene_archive_config(&root);
    assert_eq!(cfg.flags.len(), 0);
    assert_eq!(cfg.scenes.len(), 0);
}

// ─── Tests sur le vrai fichier (skip silencieux si absent du VPS) ─────────────

const REAL_PATH: &str = "scene_archive/scene_archive_config_4.00.18.00.cfg.bin.json";

fn load_real() -> Option<SceneArchiveConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_scene_archive_config(&root))
}

#[test]
fn real_file_compte_flags() {
    let Some(cfg) = load_real() else { return };
    // scene_archive_config_4.00.18.00 : 6 flags dans m_sceneArchiveFlags
    assert_eq!(
        cfg.flags.len(),
        6,
        "6 SCENE_ARCHIVE_FLAGS dans le dump réel"
    );
}

#[test]
fn real_file_compte_scenes() {
    let Some(cfg) = load_real() else { return };
    // scene_archive_config_4.00.18.00 : 112 scènes dans m_sceneArchiveDataList
    assert_eq!(
        cfg.scenes.len(),
        112,
        "112 SCENE_ARCHIVE_LIST_DATA dans le dump réel"
    );
}

#[test]
fn real_file_flags_valeurs() {
    let Some(cfg) = load_real() else { return };

    // m_sceneArchiveFlags[0].activeTempBitFlagCrc = "0xADCDB833"
    assert_eq!(
        cfg.flags[0].active_temp_bit_flag_crc,
        HashId(0xADCDB833),
        "flags[0]"
    );
    // m_sceneArchiveFlags[1].activeTempBitFlagCrc = "0x584D1EF3"
    assert_eq!(
        cfg.flags[1].active_temp_bit_flag_crc,
        HashId(0x584D1EF3),
        "flags[1]"
    );
    // m_sceneArchiveFlags[3].activeTempBitFlagCrc = "0xBAA05350"
    assert_eq!(
        cfg.flags[3].active_temp_bit_flag_crc,
        HashId(0xBAA05350),
        "flags[3]"
    );
    // m_sceneArchiveFlags[4].activeTempBitFlagCrc = "0x584D1EF3" (doublon de [1])
    assert_eq!(
        cfg.flags[4].active_temp_bit_flag_crc,
        HashId(0x584D1EF3),
        "flags[4]"
    );
}

#[test]
fn real_file_scene0_valeurs() {
    let Some(cfg) = load_real() else { return };

    // scene_archive_config_4.00.18.00 : m_sceneArchiveDataList[0]
    let s = &cfg.scenes[0];
    assert_eq!(s.id, HashId(0xF86C637D), "id[0]");
    assert_eq!(s.category, 0, "category[0]");
    assert_eq!(s.flag_num, 1, "flag_num[0]");
    assert_eq!(s.text_id_title, HashId(0x35B80566), "text_id_title[0]");
    assert_eq!(s.event_id_text, "ev01_00050", "event_id_text[0]");
    assert_eq!(
        s.thumbnail_texture_name,
        HashId(0x412B0428),
        "thumbnail_texture_name[0]"
    );
    assert_eq!(
        s.thumbnail_texture_path, "#/menu/220_img/theater_img/theater_img01_02.g4tx",
        "thumbnail_texture_path[0]"
    );
    assert_eq!(
        s.condition, "AAAAABgFNSo9RUMACgEoAAYCNNG3Zz4yAAAAAXg=",
        "condition[0]"
    );
    assert_eq!(s.active_flags, vec![0_i64, 0], "activeFlags[0]");
}

#[test]
fn real_file_scene4_active_flags() {
    let Some(cfg) = load_real() else { return };

    // m_sceneArchiveDataList[4] : activeFlags=[0,1], map_id="0x4E7B90D9"
    let s = &cfg.scenes[4];
    assert_eq!(s.id, HashId(0x9B090EA5), "id[4]");
    assert_eq!(s.map_id, HashId(0x4E7B90D9), "map_id[4]");
    assert_eq!(s.event_id_text, "ev01_00410", "event_id_text[4]");
    assert_eq!(s.active_flags, vec![0_i64, 1], "activeFlags[4]");
}

#[test]
fn real_file_scene20_chapter_et_map_tag() {
    let Some(cfg) = load_real() else { return };

    // m_sceneArchiveDataList[20] : chapter_no=1, map_tag_id="0x11F87078"
    let s = &cfg.scenes[20];
    assert_eq!(s.id, HashId(0x0CB2C284), "id[20]");
    assert_eq!(s.chapter_no, 1, "chapter_no[20]");
    assert_eq!(s.map_tag_id, HashId(0x11F87078), "map_tag_id[20]");
    assert_eq!(s.event_id_text, "ev01_04500", "event_id_text[20]");
    assert_eq!(s.active_flags, vec![2_i64, 1], "activeFlags[20]");
}

#[test]
fn real_file_scene111_side_story() {
    let Some(cfg) = load_real() else { return };

    // m_sceneArchiveDataList[111] (dernier) : category=2, side story
    let s = &cfg.scenes[111];
    assert_eq!(s.id, HashId(0xEE2B0CA6), "id[111]");
    assert_eq!(s.category, 2, "category[111]");
    assert_eq!(s.event_id_text, "ev20_00090", "event_id_text[111]");
    assert_eq!(
        s.thumbnail_texture_name,
        HashId(0x02416996),
        "thumbnail_texture_name[111]"
    );
}

#[test]
fn real_file_flag_num_toujours_1() {
    let Some(cfg) = load_real() else { return };
    // Dans tout le dump, flag_num = 1 pour toutes les entrées.
    let tous_un = cfg.scenes.iter().all(|s| s.flag_num == 1);
    assert!(
        tous_un,
        "flag_num = 1 pour toutes les 112 scènes du dump réel"
    );
}

#[test]
fn real_file_is_full_screen_view_toujours_false() {
    let Some(cfg) = load_real() else { return };
    // Dans tout le dump, is_full_screen_view = false pour toutes les entrées.
    let tous_false = cfg.scenes.iter().all(|s| !s.is_full_screen_view);
    assert!(
        tous_false,
        "is_full_screen_view = false pour toutes les 112 scènes du dump réel"
    );
}

#[test]
fn real_file_filter_by_chapter1() {
    let Some(cfg) = load_real() else { return };

    // chapter_no=1 : scène [20] id=0x0CB2C284 (confirmé dans le dump)
    let ch1 = cfg.filter_by_chapter(1);
    assert!(!ch1.is_empty(), "au moins une scène chapter_no=1");
    assert!(
        ch1.iter().any(|s| s.id == HashId(0x0CB2C284)),
        "scène 0x0CB2C284 dans chapter_no=1"
    );
}

#[test]
fn real_file_find_by_id() {
    let Some(cfg) = load_real() else { return };

    // m_sceneArchiveDataList[0].id = "0xF86C637D"
    let found = cfg.find_by_id(HashId(0xF86C637D));
    assert!(
        found.is_some(),
        "find_by_id(0xF86C637D) doit trouver l'entrée 0"
    );
    assert_eq!(found.unwrap().event_id_text, "ev01_00050");
}
