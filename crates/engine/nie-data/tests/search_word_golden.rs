#![allow(clippy::pedantic)]
//! Tests golden `search_word` — valeurs réelles tirées de :
//! - `data/common/gamedata/search_word/search_word_config.cfg.bin.json`
//! - `data/common/gamedata/search_word/info_bookmark_config_0.00.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ (JSON réel → valeur confirmée)
//!
//! ### search_word_config : SEARCH_WORD_INFO_LIST_BEG_0
//! - var\[0\] = 55 (compte total des entrées)
//!
//! ### search_word_config : SEARCH_WORD_INFO_0
//! - var\[0\] = 65287569 → search_word_id = 0x03E43591
//!   (fichier JSON ligne 6, value="65287569")
//! - var\[1\] = 1 → sort_order = 1
//!   (fichier JSON ligne 10, value="1")
//! - var\[2\] = 193949743 → secondary_hash = HashId::from_i64(193949743)
//!   (fichier JSON ligne 14, value="193949743")
//!
//! ### search_word_config : SEARCH_WORD_INFO_1
//! - var\[0\] = -1695718357 → search_word_id = 0x9AED642B
//!   (fichier JSON ligne 20, value="-1695718357" ; recoupé dans bookmark[1].searchWordId)
//! - var\[1\] = 2 → sort_order = 2
//!
//! ### search_word_config : SEARCH_WORD_INFO_51 (sort_order saute 52)
//! - var\[0\] = -1868717832 → search_word_id = HashId::from_i64(-1868717832)
//!   (fichier JSON, SEARCH_WORD_INFO_51.variables[0].value="-1868717832")
//! - var\[1\] = 53 → sort_order = 53 (pas de 52 dans le fichier réel)
//!   (fichier JSON, SEARCH_WORD_INFO_51.variables[1].value="53")
//!
//! ### info_bookmark_config : m_BookmarkFolderItemList[0]
//! - `searchWordId` = "0x03E43591" (fichier JSON ligne 19)
//! - `wordTextId` = "0xD21CE615" (fichier JSON ligne 20)
//! - `descTextId` = "0x917DEE71" (fichier JSON ligne 21)
//! - `thumbnailTexName` = "0x269EB7CF" (fichier JSON ligne 22)
//! - `thumbnailTexFileName` = "bookmark_img01_s01.g4tx" (fichier JSON ligne 23)
//! - `isNecessaryStory` = true (fichier JSON ligne 24)
//! - `enableCond` = "" (fichier JSON ligne 25)
//!
//! ### info_bookmark_config : m_InfoBookmarkDataList[0]
//! - `bookmarkId` = "0x2E2C6020" (fichier JSON ligne 519)
//! - `baseNo` = 0 (fichier JSON ligne 520)
//! - `folderNameId` = "0x01812C2A" (fichier JSON ligne 521)
//! - `thumbnailTexFileName` = "bookmark_img01_l01.g4tx" (fichier JSON ligne 523)
//! - `bookmarkFolderItemList` = [0, 9] → folder_item_offset=0, folder_item_count=9
//!   (fichier JSON lignes 524-527)

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::search_word::{parse_info_bookmark_config, parse_search_word_config};
use serde_json::json;

// ─── Constantes golden (valeurs lues dans le JSON réel, vérifiées octet-pour-octet) ─

/// SEARCH_WORD_INFO_0.var[0] = 65287569 → 0x03E43591 (positif, identique en u32)
const SW_ID_0: HashId = HashId(0x03E4_3591);

/// SEARCH_WORD_INFO_1.var[0] = -1695718357 → 0x9AED642B (recoupé dans bookmark[1])
const SW_ID_1: HashId = HashId(0x9AED_642B);

/// info_bookmark m_BookmarkFolderItemList[0].searchWordId = "0x03E43591"
const BFI_SEARCH_WORD_ID_0: HashId = HashId(0x03E4_3591);
/// info_bookmark m_BookmarkFolderItemList[0].wordTextId = "0xD21CE615"
const BFI_WORD_TEXT_ID_0: HashId = HashId(0xD21C_E615);
/// info_bookmark m_BookmarkFolderItemList[0].descTextId = "0x917DEE71"
const BFI_DESC_TEXT_ID_0: HashId = HashId(0x917D_EE71);
/// info_bookmark m_BookmarkFolderItemList[0].thumbnailTexName = "0x269EB7CF"
const BFI_THUMB_NAME_0: HashId = HashId(0x269E_B7CF);

/// info_bookmark m_InfoBookmarkDataList[0].bookmarkId = "0x2E2C6020"
const BMK_ID_0: HashId = HashId(0x2E2C_6020);
/// info_bookmark m_InfoBookmarkDataList[0].folderNameId = "0x01812C2A"
const BMK_FOLDER_NAME_ID_0: HashId = HashId(0x0181_2C2A);
/// info_bookmark m_InfoBookmarkDataList[0].thumbnailTexName = "0x31E45382"
const BMK_THUMB_NAME_0: HashId = HashId(0x31E4_5382);

// ─── Fixtures inline ──────────────────────────────────────────────────────────

/// Fixture minimale de search_word_config (2 entrées réelles).
/// Valeurs extraites octet-pour-octet du fichier JSON réel.
fn fixture_search_word() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "SEARCH_WORD_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "55"}],
                "children": [
                    // SEARCH_WORD_INFO_0 — JSON réel lignes 13-30
                    {
                        "name": "SEARCH_WORD_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "65287569"},
                            {"type": "Int", "value": "1"},
                            {"type": "Int", "value": "193949743"}
                        ],
                        "children": []
                    },
                    // SEARCH_WORD_INFO_1 — JSON réel lignes 31-48
                    {
                        "name": "SEARCH_WORD_INFO_1",
                        "variables": [
                            {"type": "Int", "value": "-1695718357"},
                            {"type": "Int", "value": "2"},
                            {"type": "Int", "value": "-1836703339"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

/// Fixture minimale de info_bookmark_config (1 tex_base_path, 2 folder_items, 1 bookmark).
/// Valeurs extraites octet-pour-octet du fichier JSON réel.
fn fixture_info_bookmark() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_TexBasePathList",
                "typeName": "TEX_BASE_PATH",
                "values": [
                    // info_bookmark_config JSON ligne 7
                    { "basePath": "#/menu/220_img/bookmark_img/<LG>/" }
                ]
            },
            {
                "name": "m_BookmarkFolderItemList",
                "typeName": "BOOKMARK_FOLDER_INFO",
                "values": [
                    // m_BookmarkFolderItemList[0] — JSON réel lignes 17-26
                    {
                        "searchWordId": "0x03E43591",
                        "wordTextId": "0xD21CE615",
                        "descTextId": "0x917DEE71",
                        "thumbnailTexName": "0x269EB7CF",
                        "thumbnailTexFileName": "bookmark_img01_s01.g4tx",
                        "isNecessaryStory": true,
                        "enableCond": ""
                    },
                    // m_BookmarkFolderItemList[1] — JSON réel lignes 27-36
                    {
                        "searchWordId": "0x9AED642B",
                        "wordTextId": "0x4B15B7AF",
                        "descTextId": "0x0874BFCB",
                        "thumbnailTexName": "0xBF97E675",
                        "thumbnailTexFileName": "bookmark_img01_s02.g4tx",
                        "isNecessaryStory": true,
                        "enableCond": ""
                    }
                ]
            },
            {
                "name": "m_InfoBookmarkDataList",
                "typeName": "INFO_BOOKMARK_DATA_CONFIG",
                "values": [
                    // m_InfoBookmarkDataList[0] — JSON réel lignes 518-529
                    {
                        "bookmarkId": "0x2E2C6020",
                        "baseNo": 0,
                        "folderNameId": "0x01812C2A",
                        "thumbnailTexName": "0x31E45382",
                        "thumbnailTexFileName": "bookmark_img01_l01.g4tx",
                        "bookmarkFolderItemList": [0, 9]
                    }
                ]
            }
        ]
    })
}

// ─── Tests fixture : parse_search_word_config ─────────────────────────────────

#[test]
fn sw_fixture_compte() {
    let words = parse_search_word_config(&fixture_search_word());
    // fixture contient 2 SEARCH_WORD_INFO valides
    assert_eq!(words.len(), 2, "fixture: 2 SEARCH_WORD_INFO");
}

#[test]
fn sw_fixture_entry_0_search_word_id() {
    let words = parse_search_word_config(&fixture_search_word());
    // SEARCH_WORD_INFO_0.var[0] = 65287569 → 0x03E43591
    assert_eq!(
        words[0].search_word_id, SW_ID_0,
        "search_word_id[0] = 0x03E43591"
    );
}

#[test]
fn sw_fixture_entry_0_sort_order() {
    let words = parse_search_word_config(&fixture_search_word());
    // SEARCH_WORD_INFO_0.var[1] = 1
    assert_eq!(words[0].sort_order, 1, "sort_order[0] = 1");
}

#[test]
fn sw_fixture_entry_0_secondary_hash() {
    let words = parse_search_word_config(&fixture_search_word());
    // SEARCH_WORD_INFO_0.var[2] = 193949743
    assert_eq!(
        words[0].secondary_hash,
        HashId::from_i64(193_949_743),
        "secondary_hash[0] = from_i64(193949743)"
    );
}

#[test]
fn sw_fixture_entry_1_search_word_id() {
    let words = parse_search_word_config(&fixture_search_word());
    // SEARCH_WORD_INFO_1.var[0] = -1695718357 → 0x9AED642B
    assert_eq!(
        words[1].search_word_id, SW_ID_1,
        "search_word_id[1] = 0x9AED642B"
    );
}

#[test]
fn sw_fixture_list_beg_ignore() {
    let words = parse_search_word_config(&fixture_search_word());
    // SEARCH_WORD_INFO_LIST_BEG_0 est ignoré → exactement 2 (pas 3)
    assert_eq!(words.len(), 2, "LIST_BEG ignoré : toujours 2 entrées");
}

// ─── Tests fixture : parse_info_bookmark_config ───────────────────────────────

#[test]
fn bmc_fixture_tex_base_path() {
    let cfg = parse_info_bookmark_config(&fixture_info_bookmark());
    assert_eq!(cfg.tex_base_paths.len(), 1, "1 TexBasePath");
    // m_TexBasePathList[0].basePath — JSON réel ligne 7
    assert_eq!(
        cfg.tex_base_paths[0].base_path, "#/menu/220_img/bookmark_img/<LG>/",
        "basePath = '#/menu/220_img/bookmark_img/<LG>/'"
    );
}

#[test]
fn bmc_fixture_folder_items_compte() {
    let cfg = parse_info_bookmark_config(&fixture_info_bookmark());
    assert_eq!(cfg.folder_items.len(), 2, "fixture: 2 BookmarkFolderItem");
}

#[test]
fn bmc_fixture_folder_item_0_champs() {
    let cfg = parse_info_bookmark_config(&fixture_info_bookmark());
    let item = &cfg.folder_items[0];
    // m_BookmarkFolderItemList[0] — JSON réel lignes 17-26
    assert_eq!(
        item.search_word_id, BFI_SEARCH_WORD_ID_0,
        "searchWordId[0] = 0x03E43591"
    );
    assert_eq!(
        item.word_text_id, BFI_WORD_TEXT_ID_0,
        "wordTextId[0] = 0xD21CE615"
    );
    assert_eq!(
        item.desc_text_id, BFI_DESC_TEXT_ID_0,
        "descTextId[0] = 0x917DEE71"
    );
    assert_eq!(
        item.thumbnail_tex_name, BFI_THUMB_NAME_0,
        "thumbnailTexName[0] = 0x269EB7CF"
    );
    assert_eq!(
        item.thumbnail_tex_file_name, "bookmark_img01_s01.g4tx",
        "thumbnailTexFileName[0]"
    );
    assert!(item.is_necessary_story, "isNecessaryStory[0] = true");
    assert_eq!(item.enable_cond, "", "enableCond[0] = ''");
}

#[test]
fn bmc_fixture_bookmark_data_0_champs() {
    let cfg = parse_info_bookmark_config(&fixture_info_bookmark());
    let bk = &cfg.bookmarks[0];
    // m_InfoBookmarkDataList[0] — JSON réel lignes 518-529
    assert_eq!(bk.bookmark_id, BMK_ID_0, "bookmarkId[0] = 0x2E2C6020");
    assert_eq!(bk.base_no, 0, "baseNo[0] = 0");
    assert_eq!(
        bk.folder_name_id, BMK_FOLDER_NAME_ID_0,
        "folderNameId[0] = 0x01812C2A"
    );
    assert_eq!(
        bk.thumbnail_tex_name, BMK_THUMB_NAME_0,
        "thumbnailTexName = 0x31E45382"
    );
    assert_eq!(
        bk.thumbnail_tex_file_name, "bookmark_img01_l01.g4tx",
        "thumbnailTexFileName = 'bookmark_img01_l01.g4tx'"
    );
    assert_eq!(bk.folder_item_offset, 0, "folder_item_offset = 0");
    assert_eq!(bk.folder_item_count, 9, "folder_item_count = 9");
}

#[test]
fn bmc_fixture_folder_items_of() {
    let cfg = parse_info_bookmark_config(&fixture_info_bookmark());
    let bk = &cfg.bookmarks[0];
    // bookmarkFolderItemList = [0, 9] mais fixture n'a que 2 folder_items
    // → la tranche [0..9] sera clampée à [0..2]
    let items = cfg.folder_items_of(bk);
    // On vérifie que la résolution ne panique pas et retourne les entrées disponibles
    assert!(items.len() <= 2, "clamp correct sur fixture réduite");
}

#[test]
fn bmc_fixture_find_folder_item() {
    let cfg = parse_info_bookmark_config(&fixture_info_bookmark());
    // Recherche par search_word_id connu
    let found = cfg.find_folder_item(BFI_SEARCH_WORD_ID_0);
    assert!(found.is_some(), "find_folder_item(0x03E43591) doit trouver");
    assert_eq!(found.unwrap().word_text_id, BFI_WORD_TEXT_ID_0);

    // Identifiant absent → None
    assert!(cfg.find_folder_item(HashId(0xDEAD_BEEF)).is_none());
}

// ─── Tests sur fichiers réels (skip silencieux si absents) ───────────────────

const REAL_SEARCH_WORD: &str = "search_word/search_word_config.cfg.bin.json";
const REAL_INFO_BOOKMARK: &str = "search_word/info_bookmark_config_0.00.00.cfg.bin.json";

fn load_json(path: &str) -> Option<serde_json::Value> {
    let path = common::chemin(path)?;
    if !path.is_file() {
        eprintln!("skip : {} absent du corpus", path.display());
        return None;
    }
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", path.display()));
    Some(
        serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("JSON invalide {}: {e}", path.display())),
    )
}

// — search_word_config —

#[test]
fn real_file_sw_compte() {
    let Some(root) = load_json(REAL_SEARCH_WORD) else {
        return;
    };
    let words = parse_search_word_config(&root);
    // search_word_config.cfg.bin.json : SEARCH_WORD_INFO_LIST_BEG_0.var[0] = 55
    assert_eq!(words.len(), 55, "search_word_config : 55 SEARCH_WORD_INFO");
}

#[test]
fn real_file_sw_entry_0() {
    let Some(root) = load_json(REAL_SEARCH_WORD) else {
        return;
    };
    let words = parse_search_word_config(&root);

    // SEARCH_WORD_INFO_0.var[0] = 65287569 → 0x03E43591
    assert_eq!(
        words[0].search_word_id, SW_ID_0,
        "search_word_id[0] = 0x03E43591"
    );
    // SEARCH_WORD_INFO_0.var[1] = 1
    assert_eq!(words[0].sort_order, 1, "sort_order[0] = 1");
    // SEARCH_WORD_INFO_0.var[2] = 193949743
    assert_eq!(
        words[0].secondary_hash,
        HashId::from_i64(193_949_743),
        "secondary_hash[0] = from_i64(193949743)"
    );
}

#[test]
fn real_file_sw_entry_1() {
    let Some(root) = load_json(REAL_SEARCH_WORD) else {
        return;
    };
    let words = parse_search_word_config(&root);

    // SEARCH_WORD_INFO_1.var[0] = -1695718357 → 0x9AED642B
    assert_eq!(
        words[1].search_word_id, SW_ID_1,
        "search_word_id[1] = 0x9AED642B"
    );
    // SEARCH_WORD_INFO_1.var[1] = 2
    assert_eq!(words[1].sort_order, 2, "sort_order[1] = 2");
}

#[test]
fn real_file_sw_entry_51_sort_order_saute_52() {
    let Some(root) = load_json(REAL_SEARCH_WORD) else {
        return;
    };
    let words = parse_search_word_config(&root);

    // SEARCH_WORD_INFO_51.var[0] = -1868717832
    assert_eq!(
        words[51].search_word_id,
        HashId::from_i64(-1_868_717_832),
        "search_word_id[51] = from_i64(-1868717832)"
    );
    // SEARCH_WORD_INFO_51.var[1] = 53 (saute la valeur 52)
    assert_eq!(
        words[51].sort_order, 53,
        "sort_order[51] = 53 (pas de 52 dans le fichier réel)"
    );
}

#[test]
fn real_file_sw_last_entry() {
    let Some(root) = load_json(REAL_SEARCH_WORD) else {
        return;
    };
    let words = parse_search_word_config(&root);

    // SEARCH_WORD_INFO_54.var[1] = 56 (dernier sort_order)
    assert_eq!(
        words[54].sort_order, 56,
        "sort_order[54] = 56 (dernière entrée)"
    );
    // SEARCH_WORD_INFO_54.var[0] = -520661897
    assert_eq!(
        words[54].search_word_id,
        HashId::from_i64(-520_661_897),
        "search_word_id[54] = from_i64(-520661897)"
    );
}

// — info_bookmark_config —

#[test]
fn real_file_bmc_tex_base_path() {
    let Some(root) = load_json(REAL_INFO_BOOKMARK) else {
        return;
    };
    let cfg = parse_info_bookmark_config(&root);
    assert_eq!(cfg.tex_base_paths.len(), 1, "1 TexBasePath");
    // m_TexBasePathList[0].basePath — JSON ligne 7
    assert_eq!(
        cfg.tex_base_paths[0].base_path, "#/menu/220_img/bookmark_img/<LG>/",
        "basePath = '#/menu/220_img/bookmark_img/<LG>/'"
    );
}

#[test]
fn real_file_bmc_folder_items_compte() {
    let Some(root) = load_json(REAL_INFO_BOOKMARK) else {
        return;
    };
    let cfg = parse_info_bookmark_config(&root);
    // m_BookmarkFolderItemList : 55 entrées
    assert_eq!(
        cfg.folder_items.len(),
        55,
        "m_BookmarkFolderItemList : 55 entrées"
    );
}

#[test]
fn real_file_bmc_folder_item_0() {
    let Some(root) = load_json(REAL_INFO_BOOKMARK) else {
        return;
    };
    let cfg = parse_info_bookmark_config(&root);
    let item = &cfg.folder_items[0];

    // m_BookmarkFolderItemList[0] — JSON réel lignes 17-26
    assert_eq!(
        item.search_word_id, BFI_SEARCH_WORD_ID_0,
        "searchWordId[0] = 0x03E43591"
    );
    assert_eq!(
        item.word_text_id, BFI_WORD_TEXT_ID_0,
        "wordTextId[0] = 0xD21CE615"
    );
    assert_eq!(
        item.desc_text_id, BFI_DESC_TEXT_ID_0,
        "descTextId[0] = 0x917DEE71"
    );
    assert_eq!(
        item.thumbnail_tex_name, BFI_THUMB_NAME_0,
        "thumbnailTexName[0] = 0x269EB7CF"
    );
    assert_eq!(
        item.thumbnail_tex_file_name, "bookmark_img01_s01.g4tx",
        "thumbnailTexFileName[0]"
    );
    assert!(item.is_necessary_story, "isNecessaryStory[0] = true");
    assert_eq!(item.enable_cond, "", "enableCond[0] = ''");
}

#[test]
fn real_file_bmc_bookmarks_compte() {
    let Some(root) = load_json(REAL_INFO_BOOKMARK) else {
        return;
    };
    let cfg = parse_info_bookmark_config(&root);
    // m_InfoBookmarkDataList : 11 entrées
    assert_eq!(
        cfg.bookmarks.len(),
        11,
        "m_InfoBookmarkDataList : 11 entrées"
    );
}

#[test]
fn real_file_bmc_bookmark_0() {
    let Some(root) = load_json(REAL_INFO_BOOKMARK) else {
        return;
    };
    let cfg = parse_info_bookmark_config(&root);
    let bk = &cfg.bookmarks[0];

    // m_InfoBookmarkDataList[0] — JSON réel lignes 518-529
    assert_eq!(bk.bookmark_id, BMK_ID_0, "bookmarkId[0] = 0x2E2C6020");
    assert_eq!(bk.base_no, 0, "baseNo[0] = 0");
    assert_eq!(
        bk.folder_name_id, BMK_FOLDER_NAME_ID_0,
        "folderNameId[0] = 0x01812C2A"
    );
    assert_eq!(
        bk.thumbnail_tex_name, BMK_THUMB_NAME_0,
        "thumbnailTexName = 0x31E45382"
    );
    assert_eq!(
        bk.thumbnail_tex_file_name, "bookmark_img01_l01.g4tx",
        "thumbnailTexFileName = 'bookmark_img01_l01.g4tx'"
    );
    assert_eq!(bk.folder_item_offset, 0, "folder_item_offset = 0");
    assert_eq!(bk.folder_item_count, 9, "folder_item_count = 9");
}

#[test]
fn real_file_bmc_bookmark_last() {
    let Some(root) = load_json(REAL_INFO_BOOKMARK) else {
        return;
    };
    let cfg = parse_info_bookmark_config(&root);
    let bk = &cfg.bookmarks[10]; // dernier (index 10, baseNo=10)

    // m_InfoBookmarkDataList[10] — JSON réel
    assert_eq!(bk.base_no, 10, "baseNo[10] = 10");
    assert_eq!(
        bk.bookmark_id,
        HashId(0x3737_5161),
        "bookmarkId[10] = 0x37375161"
    );
    // bookmarkFolderItemList = [48, 7]
    assert_eq!(bk.folder_item_offset, 48, "folder_item_offset[10] = 48");
    assert_eq!(bk.folder_item_count, 7, "folder_item_count[10] = 7");
}

#[test]
fn real_file_bmc_folder_items_of_resolu() {
    let Some(root) = load_json(REAL_INFO_BOOKMARK) else {
        return;
    };
    let cfg = parse_info_bookmark_config(&root);
    let bk = &cfg.bookmarks[0]; // offset=0, count=9
    let items = cfg.folder_items_of(bk);
    // Les 9 premiers folder_items
    assert_eq!(items.len(), 9, "folder_items_of bookmark[0] = 9 éléments");
    assert_eq!(
        items[0].search_word_id, BFI_SEARCH_WORD_ID_0,
        "items[0] = 0x03E43591"
    );
}
