#![allow(clippy::pedantic)]
//! Tests golden `chat_emote` — valeurs réelles tirées des dumps :
//! `chat_emote/`
//!   - `chat_emote_config_{0.00.00,1.02.03.00,1.03.17.00}.cfg.bin.json`
//!   - `chat_emote_def_set_config_{0.00.00,1.02.03.00,1.03.17.00}.cfg.bin.json`
//!
//! Version golden principale : **1.03.17.00** (la plus récente, 57 emotes).
//!
//! ## Valeurs confirmées (1.03.17.00, m_ChatEmoteInfoList)
//! - `[0]` : id=0x6AE23243, flag_idx=0, sort_id=0, type=0, texFilePath=SENTINEL, texName=0, text_id=0x55FBBA47, stamp_idx=0
//! - `[27]` : id=0x0738FF66, flag_idx=100, type=1, texFilePath=emote_img01_01.g4tx, texName=0x2EF86069, text_id=0x88B089E4, motion_id=0x4A6673E1
//! - `[35]` : id=0x6DD3C782, type=3, stamp_idx=1, texName=0x7E17D87D
//! - `[56]` : id=0x36922181, flag_idx=313, sort_id=313, type=2, texFilePath=stamp_img02_14.g4tx, texName=0x05D3B25D
//!
//! ## Valeurs confirmées (def_set)
//! - 1.03.17.00 : 4 entrées, champ `type` (0..3), chat_id_array[0]=0x6AE23243
//! - 0.00.00 : 3 entrées, champ `page_idx` (1..3), chat_id_array[0]=0x6AE23243

mod common;

use nie_data::chat_emote::{
    ChatEmoteConfig, ChatEmoteDefSetConfig, SENTINEL_CONFIG, parse_chat_emote_config,
    parse_chat_emote_def_set_config,
};
use nie_data::hash::HashId;
use serde_json::json;

// ─── Fixtures (valeurs extraites octet-pour-octet des dumps) ────────────────────

fn fixture_config() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [{
            "name": "m_ChatEmoteInfoList",
            "typeName": "CHAT_EMOTE_CONFIG",
            "values": [
                {
                    "id": "0x6AE23243", "flag_idx": 0, "sort_id": 0, "type": 0,
                    "texFilePath": "\u{FFFD}CHAT_EMOTE_CONFIG", "texName": "0x00000000",
                    "text_id": "0x55FBBA47", "stamp_idx": 0, "motion_id": "0x00000000",
                    "motion_loop": false, "se_id": 0, "se_sub_id": 0,
                    "enable_cond": "\u{FFFD}CHAT_EMOTE_CONFIG"
                },
                {
                    "id": "0x0738FF66", "flag_idx": 100, "sort_id": 100, "type": 1,
                    "texFilePath": "#/menu/220_img/stamp_img/<LG>/emote_img01_01.g4tx",
                    "texName": "0x2EF86069", "text_id": "0x88B089E4", "stamp_idx": 0,
                    "motion_id": "0x4A6673E1", "motion_loop": false, "se_id": 0, "se_sub_id": 0,
                    "enable_cond": "\u{FFFD}CHAT_EMOTE_CONFIG"
                }
            ]
        }]
    })
}

fn fixture_def_set() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [{
            "name": "m_ChatEmoteDefSetInfoList",
            "typeName": "CHAT_EMOTE_DEF_SET_CONFIG",
            "values": [
                { "type": 0, "chat_id_array": "0x6AE23243" },
                { "type": 1, "chat_id_array": "0x0738FF66" }
            ]
        }]
    })
}

// ─── Tests fixtures ─────────────────────────────────────────────────────────────

#[test]
fn fixture_config_entree0() {
    let cfg = parse_chat_emote_config(&fixture_config());
    assert_eq!(cfg.entries.len(), 2);
    let e = &cfg.entries[0];
    assert_eq!(e.id, HashId(0x6AE2_3243));
    assert_eq!(e.flag_idx, 0);
    assert_eq!(e.sort_id, 0);
    assert_eq!(e.type_, 0);
    assert_eq!(e.tex_file_path, SENTINEL_CONFIG);
    assert_eq!(e.tex_name, HashId::ZERO);
    assert_eq!(e.text_id, HashId(0x55FB_BA47));
    assert!(
        !e.has_real_texture(),
        "entrée 0 : pas de texture (sentinel)"
    );
    assert!(!e.motion_loop);
}

#[test]
fn fixture_config_entree_avec_texture() {
    let cfg = parse_chat_emote_config(&fixture_config());
    let e = &cfg.entries[1];
    assert_eq!(e.id, HashId(0x0738_FF66));
    assert_eq!(e.flag_idx, 100);
    assert_eq!(e.type_, 1);
    assert_eq!(
        e.tex_file_path,
        "#/menu/220_img/stamp_img/<LG>/emote_img01_01.g4tx"
    );
    assert_eq!(e.tex_name, HashId(0x2EF8_6069));
    assert_eq!(e.motion_id, HashId(0x4A66_73E1));
    assert!(e.has_real_texture());
}

#[test]
fn fixture_config_find() {
    let cfg = parse_chat_emote_config(&fixture_config());
    assert!(cfg.find_by_id(HashId(0x6AE2_3243)).is_some());
    assert!(cfg.find_by_id(HashId(0xDEAD_BEEF)).is_none());
    assert_eq!(cfg.find_by_sort_id(100).unwrap().id, HashId(0x0738_FF66));
}

#[test]
fn fixture_def_set_parse() {
    let cfg = parse_chat_emote_def_set_config(&fixture_def_set());
    assert_eq!(cfg.entries.len(), 2);
    assert_eq!(cfg.entries[0].set_type, 0);
    assert_eq!(cfg.entries[0].page_idx, 0);
    assert_eq!(cfg.entries[0].chat_id_array, HashId(0x6AE2_3243));
    assert_eq!(
        cfg.find_by_chat_id(HashId(0x0738_FF66)).unwrap().set_type,
        1
    );
}

#[test]
fn fixture_liste_manquante_vide() {
    let root = json!({ "version": 100, "lists": [] });
    assert_eq!(parse_chat_emote_config(&root).entries.len(), 0);
    assert_eq!(parse_chat_emote_def_set_config(&root).entries.len(), 0);
}

// ─── Tests sur les vrais fichiers (skip si absents du VPS) ──────────────────────

const DIR: &str = "chat_emote/";

fn load_config(version: &str) -> Option<ChatEmoteConfig> {
    let path = alloc_path(version, "chat_emote_config");
    load(&path).map(|v| parse_chat_emote_config(&v))
}

fn load_def_set(version: &str) -> Option<ChatEmoteDefSetConfig> {
    let path = alloc_path(version, "chat_emote_def_set_config");
    load(&path).map(|v| parse_chat_emote_def_set_config(&v))
}

fn alloc_path(version: &str, stem: &str) -> String {
    format!("{DIR}{stem}_{version}.cfg.bin.json")
}

fn load(path: &str) -> Option<serde_json::Value> {
    let path = common::chemin(path)?;
    if !path.is_file() {
        eprintln!("skip : {} absent du corpus", path.display());
        return None;
    }
    let content = std::fs::read_to_string(&path).unwrap();
    Some(serde_json::from_str(&content).unwrap())
}

// --- Version golden principale : 1.03.17.00 ---

#[test]
fn real_1_03_17_compte() {
    let Some(cfg) = load_config("1.03.17.00") else {
        return;
    };
    assert_eq!(cfg.entries.len(), 57, "57 emotes en 1.03.17.00");
}

#[test]
fn real_1_03_17_entree0() {
    let Some(cfg) = load_config("1.03.17.00") else {
        return;
    };
    let e = &cfg.entries[0];
    assert_eq!(e.id, HashId(0x6AE2_3243));
    assert_eq!(e.flag_idx, 0);
    assert_eq!(e.sort_id, 0);
    assert_eq!(e.type_, 0);
    assert_eq!(e.tex_file_path, SENTINEL_CONFIG);
    assert_eq!(e.tex_name, HashId::ZERO);
    assert_eq!(e.text_id, HashId(0x55FB_BA47));
    assert_eq!(e.stamp_idx, 0);
    assert_eq!(e.enable_cond, SENTINEL_CONFIG);
}

#[test]
fn real_1_03_17_entree27_texture() {
    let Some(cfg) = load_config("1.03.17.00") else {
        return;
    };
    let e = &cfg.entries[27];
    assert_eq!(e.id, HashId(0x0738_FF66));
    assert_eq!(e.flag_idx, 100);
    assert_eq!(e.sort_id, 100);
    assert_eq!(e.type_, 1);
    assert_eq!(
        e.tex_file_path,
        "#/menu/220_img/stamp_img/<LG>/emote_img01_01.g4tx"
    );
    assert_eq!(e.tex_name, HashId(0x2EF8_6069));
    assert_eq!(e.text_id, HashId(0x88B0_89E4));
    assert_eq!(e.motion_id, HashId(0x4A66_73E1));
    assert!(e.has_real_texture());
}

#[test]
fn real_1_03_17_entree35_stamp_idx() {
    let Some(cfg) = load_config("1.03.17.00") else {
        return;
    };
    let e = &cfg.entries[35];
    assert_eq!(e.id, HashId(0x6DD3_C782));
    assert_eq!(e.type_, 3);
    assert_eq!(e.stamp_idx, 1);
    assert_eq!(e.tex_name, HashId(0x7E17_D87D));
    assert_eq!(e.text_id, HashId::ZERO);
}

#[test]
fn real_1_03_17_derniere() {
    let Some(cfg) = load_config("1.03.17.00") else {
        return;
    };
    let e = &cfg.entries[56];
    assert_eq!(e.id, HashId(0x3692_2181));
    assert_eq!(e.flag_idx, 313);
    assert_eq!(e.sort_id, 313);
    assert_eq!(e.type_, 2);
    assert_eq!(
        e.tex_file_path,
        "#/menu/220_img/stamp_img/<LG>/stamp_img02_14.g4tx"
    );
    assert_eq!(e.tex_name, HashId(0x05D3_B25D));
}

#[test]
fn real_1_03_17_se_toujours_zero() {
    let Some(cfg) = load_config("1.03.17.00") else {
        return;
    };
    assert!(cfg.entries.iter().all(|e| e.se_id == 0 && e.se_sub_id == 0));
    assert!(cfg.entries.iter().all(|e| !e.motion_loop));
}

#[test]
fn real_1_03_17_def_set() {
    let Some(cfg) = load_def_set("1.03.17.00") else {
        return;
    };
    assert_eq!(cfg.entries.len(), 4, "4 jeux en 1.03.17.00");
    // 1.03.17.00 utilise le champ `type` (0..3) et non `page_idx`.
    assert_eq!(cfg.entries[0].set_type, 0);
    assert_eq!(cfg.entries[0].page_idx, 0);
    assert_eq!(cfg.entries[0].chat_id_array, HashId(0x6AE2_3243));
    assert_eq!(cfg.entries[1].set_type, 1);
    assert_eq!(cfg.entries[1].chat_id_array, HashId(0x0738_FF66));
    assert_eq!(cfg.entries[2].chat_id_array, HashId(0x5FE3_E44F));
    assert_eq!(cfg.entries[3].chat_id_array, HashId(0x6DD3_C782));
}

// --- Version 1.02.03.00 (57 emotes, sans flag_idx/texFilePath/texName) ---

#[test]
fn real_1_02_03_compte_et_champs() {
    let Some(cfg) = load_config("1.02.03.00") else {
        return;
    };
    assert_eq!(cfg.entries.len(), 57);
    let e0 = &cfg.entries[0];
    assert_eq!(e0.id, HashId(0x6AE2_3243));
    assert_eq!(e0.sort_id, 0);
    assert_eq!(e0.text_id, HashId(0x55FB_BA47));
    // Champs absents de cette version → valeurs par défaut.
    assert_eq!(e0.flag_idx, 0);
    assert_eq!(e0.tex_file_path, "");
    assert_eq!(e0.tex_name, HashId::ZERO);
    assert!(!e0.has_real_texture());
    // stamp_idx existe déjà en 1.02.03.00.
    let e35 = &cfg.entries[35];
    assert_eq!(e35.id, HashId(0x6DD3_C782));
    assert_eq!(e35.stamp_idx, 1);
}

#[test]
fn real_1_02_03_def_set_page_idx() {
    let Some(cfg) = load_def_set("1.02.03.00") else {
        return;
    };
    assert_eq!(cfg.entries.len(), 3);
    // 1.02.03.00 utilise `page_idx` (1..3), pas `type`.
    assert_eq!(cfg.entries[0].page_idx, 1);
    assert_eq!(cfg.entries[0].set_type, 0);
    assert_eq!(cfg.entries[0].chat_id_array, HashId(0x6AE2_3243));
    assert_eq!(cfg.entries[1].page_idx, 2);
    assert_eq!(cfg.entries[1].chat_id_array, HashId(0x04FE_3394));
    assert_eq!(cfg.entries[2].page_idx, 3);
    assert_eq!(cfg.entries[2].chat_id_array, HashId(0x7D22_8B30));
}

// --- Version 0.00.00 (8 emotes, layout minimal) ---

#[test]
fn real_0_00_00_compte_et_champs() {
    let Some(cfg) = load_config("0.00.00") else {
        return;
    };
    assert_eq!(cfg.entries.len(), 8, "8 emotes en 0.00.00");
    let e0 = &cfg.entries[0];
    assert_eq!(e0.id, HashId(0x6AE2_3243));
    assert_eq!(e0.sort_id, 0);
    assert_eq!(e0.text_id, HashId(0x55FB_BA47));
    // 0.00.00 n'a ni stamp_idx ni texFilePath.
    assert_eq!(e0.stamp_idx, 0);
    assert_eq!(e0.tex_file_path, "");
    // Une entrée type=1 avec motion_id non nul.
    let e2 = &cfg.entries[2];
    assert_eq!(e2.id, HashId(0x0738_FF66));
    assert_eq!(e2.type_, 1);
    assert_eq!(e2.sort_id, 100);
    assert_eq!(e2.motion_id, HashId(0x4A66_73E1));
}

#[test]
fn real_0_00_00_def_set_page_idx() {
    let Some(cfg) = load_def_set("0.00.00") else {
        return;
    };
    assert_eq!(cfg.entries.len(), 3);
    assert_eq!(cfg.entries[0].page_idx, 1);
    assert_eq!(cfg.entries[0].chat_id_array, HashId(0x6AE2_3243));
    assert_eq!(cfg.entries[1].chat_id_array, HashId(0x04FE_3394));
    assert_eq!(cfg.entries[2].chat_id_array, HashId(0x7D22_8B30));
}
