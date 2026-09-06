#![allow(clippy::pedantic)]
//! Tests golden `friendmap` — valeurs réelles tirées de :
//! `friendmap/friendmap_config_0.00.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ (index → valeur confirmée)
//!
//! ### m_friendMapLineInfo[0]
//! - `lineNo` = 0
//! - `lineElemAry` = `0x00000000` (ligne vide)
//!
//! ### m_friendMapLineInfo[5] (premier non-vide de la carte 0)
//! - `lineNo` = 5
//! - `lineElemAry` = `0xE22BE2FA`
//!
//! ### m_friendMapLineInfo[7]
//! - `lineNo` = 7
//! - `lineElemAry` = `0x5DF86A72`
//!
//! ### m_friendMapBaseInfo[0]
//! - `friendMapID` = `0x62975AF6`
//! - `startY` = 8, `startX` = 7
//! - `lineInfo` = [0, 14] → `line_offset` = 0, `line_count` = 14
//!
//! ### m_friendMapBaseInfo[1]
//! - `friendMapID` = `0xFB9E0B4C`
//! - `startY` = 2, `startX` = 7
//! - `lineInfo` = [14, 7] → `line_offset` = 14, `line_count` = 7
//!
//! ### m_friendMapBaseInfo[2]
//! - `friendMapID` = `0x8C993BDA`
//! - `startY` = 4, `startX` = 4
//! - `lineInfo` = [21, 9] → `line_offset` = 21, `line_count` = 9
//!
//! ### m_friendMapBaseInfo[3]
//! - `friendMapID` = `0x12FDAE79`
//! - `startY` = 4, `startX` = 5
//! - `lineInfo` = [30, 9] → `line_offset` = 30, `line_count` = 9

mod common;

use nie_data::friendmap::{
    FriendMapBaseInfo, FriendMapConfig, FriendMapLineInfo, parse_friendmap_config,
};
use nie_data::hash::HashId;
use serde_json::json;

// ─── Fixture inline ───────────────────────────────────────────────────────────
// Sous-ensemble cohérent du dump réel (3 lignes + 1 carte) — valeurs byte-à-byte.

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_friendMapLineInfo",
                "typeName": "FRIENDMAP_LINE_INFO",
                "values": [
                    // index 0 — m_friendMapLineInfo[0] du dump réel : lineNo=0, ligne vide
                    { "lineNo": 0, "lineElemAry": "0x00000000" },
                    // index 1 — m_friendMapLineInfo[5] du dump réel : lineNo=5, premier non-vide
                    { "lineNo": 5, "lineElemAry": "0xE22BE2FA" },
                    // index 2 — m_friendMapLineInfo[6] du dump réel : lineNo=6
                    { "lineNo": 6, "lineElemAry": "0x1142E426" }
                ]
            },
            {
                "name": "m_friendMapBaseInfo",
                "typeName": "FRIENDMAP_BASE_INFO",
                "values": [
                    // m_friendMapBaseInfo[0] — startY/startX du dump réel ; count réduit à 3 pour la fixture
                    {
                        "friendMapID": "0x62975AF6",
                        "startY": 8,
                        "startX": 7,
                        "lineInfo": [0, 3]
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn comptes_fixture() {
    let cfg = parse_friendmap_config(&fixture());
    assert_eq!(cfg.lines.len(), 3, "3 lignes dans la fixture");
    assert_eq!(cfg.maps.len(), 1, "1 carte dans la fixture");
}

#[test]
fn line_index0_ligne_vide() {
    let cfg = parse_friendmap_config(&fixture());
    let l: &FriendMapLineInfo = &cfg.lines[0];

    // m_friendMapLineInfo[0] du dump réel : lineNo=0, lineElemAry=0x00000000
    assert_eq!(l.line_no, 0);
    assert_eq!(
        l.line_elem_ary,
        HashId(0x0000_0000),
        "ligne vide = hash nul"
    );
    assert!(l.line_elem_ary.is_zero(), "is_zero() doit être vrai");
}

#[test]
fn line_index1_premier_non_vide() {
    let cfg = parse_friendmap_config(&fixture());
    let l: &FriendMapLineInfo = &cfg.lines[1];

    // m_friendMapLineInfo[5] du dump réel : lineNo=5, lineElemAry=0xE22BE2FA
    assert_eq!(l.line_no, 5);
    assert_eq!(l.line_elem_ary, HashId(0xE22B_E2FA));
}

#[test]
fn line_index2_champs() {
    let cfg = parse_friendmap_config(&fixture());
    let l: &FriendMapLineInfo = &cfg.lines[2];

    // m_friendMapLineInfo[6] du dump réel : lineNo=6, lineElemAry=0x1142E426
    assert_eq!(l.line_no, 6);
    assert_eq!(l.line_elem_ary, HashId(0x1142_E426));
}

#[test]
fn base_info_carte0_champs_scalaires() {
    let cfg = parse_friendmap_config(&fixture());
    assert_eq!(cfg.maps.len(), 1);
    let m: &FriendMapBaseInfo = &cfg.maps[0];

    // m_friendMapBaseInfo[0] du dump réel
    assert_eq!(m.friend_map_id, HashId(0x6297_5AF6));
    assert_eq!(m.start_y, 8, "startY");
    assert_eq!(m.start_x, 7, "startX");
}

#[test]
fn base_info_carte0_line_info() {
    let cfg = parse_friendmap_config(&fixture());
    let m: &FriendMapBaseInfo = &cfg.maps[0];

    // lineInfo=[0, 3] dans la fixture (offset réel = 0, count adapté à la fixture)
    assert_eq!(m.line_offset, 0, "lineInfo[0] = offset");
    assert_eq!(m.line_count, 3, "lineInfo[1] = count");
}

#[test]
fn lines_of_slice_longueur() {
    let cfg = parse_friendmap_config(&fixture());
    let slice = cfg.lines_of(&cfg.maps[0]);
    assert_eq!(slice.len(), 3, "lines_of renvoie line_count éléments");
}

#[test]
fn lines_of_premiere_ligne() {
    let cfg = parse_friendmap_config(&fixture());
    let slice = cfg.lines_of(&cfg.maps[0]);

    // La première ligne de la tranche = m_friendMapLineInfo[0] du dump réel
    assert_eq!(slice[0].line_no, 0);
    assert!(slice[0].line_elem_ary.is_zero(), "première ligne vide");
}

#[test]
fn lines_of_deuxieme_ligne() {
    let cfg = parse_friendmap_config(&fixture());
    let slice = cfg.lines_of(&cfg.maps[0]);

    // La deuxième ligne de la tranche = m_friendMapLineInfo[5] du dump réel
    assert_eq!(slice[1].line_elem_ary, HashId(0xE22B_E2FA));
}

#[test]
fn find_by_id_present() {
    let cfg = parse_friendmap_config(&fixture());
    // m_friendMapBaseInfo[0].friendMapID = 0x62975AF6
    let found = cfg.find_by_id(HashId(0x6297_5AF6));
    assert!(found.is_some(), "carte 0 doit être trouvée par son ID");
    assert_eq!(found.unwrap().start_y, 8);
}

#[test]
fn find_by_id_absent() {
    let cfg = parse_friendmap_config(&fixture());
    assert!(cfg.find_by_id(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn lines_of_hors_bornes_retourne_vide() {
    let cfg = parse_friendmap_config(&fixture());
    // Créer une carte fictive avec offset hors bornes
    let carte_hors_bornes = FriendMapBaseInfo {
        friend_map_id: HashId(0x0000_0001),
        start_y: 0,
        start_x: 0,
        line_offset: 100,
        line_count: 5,
    };
    let slice = cfg.lines_of(&carte_hors_bornes);
    assert_eq!(slice.len(), 0, "offset hors bornes → tranche vide");
}

// ─── Tests sur le vrai fichier (skip si absent du VPS) ───────────────────────

const REAL_PATH: &str = "friendmap/friendmap_config_0.00.00.cfg.bin.json";

fn load_real() -> Option<FriendMapConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_friendmap_config(&root))
}

#[test]
fn real_file_comptes() {
    let Some(cfg) = load_real() else { return };

    // Dump réel : 39 lignes réparties sur 4 cartes
    assert_eq!(cfg.lines.len(), 39, "39 entrées dans m_friendMapLineInfo");
    assert_eq!(cfg.maps.len(), 4, "4 cartes dans m_friendMapBaseInfo");
}

#[test]
fn real_file_map0_champs() {
    let Some(cfg) = load_real() else { return };

    // m_friendMapBaseInfo[0] du dump réel
    let m0 = &cfg.maps[0];
    assert_eq!(m0.friend_map_id, HashId(0x6297_5AF6));
    assert_eq!(m0.start_y, 8);
    assert_eq!(m0.start_x, 7);
    assert_eq!(m0.line_offset, 0);
    assert_eq!(m0.line_count, 14);
}

#[test]
fn real_file_map1_champs() {
    let Some(cfg) = load_real() else { return };

    // m_friendMapBaseInfo[1] du dump réel
    let m1 = &cfg.maps[1];
    assert_eq!(m1.friend_map_id, HashId(0xFB9E_0B4C));
    assert_eq!(m1.start_y, 2);
    assert_eq!(m1.start_x, 7);
    assert_eq!(m1.line_offset, 14);
    assert_eq!(m1.line_count, 7);
}

#[test]
fn real_file_map2_champs() {
    let Some(cfg) = load_real() else { return };

    // m_friendMapBaseInfo[2] du dump réel
    let m2 = &cfg.maps[2];
    assert_eq!(m2.friend_map_id, HashId(0x8C99_3BDA));
    assert_eq!(m2.start_y, 4);
    assert_eq!(m2.start_x, 4);
    assert_eq!(m2.line_offset, 21);
    assert_eq!(m2.line_count, 9);
}

#[test]
fn real_file_map3_champs() {
    let Some(cfg) = load_real() else { return };

    // m_friendMapBaseInfo[3] du dump réel
    let m3 = &cfg.maps[3];
    assert_eq!(m3.friend_map_id, HashId(0x12FD_AE79));
    assert_eq!(m3.start_y, 4);
    assert_eq!(m3.start_x, 5);
    assert_eq!(m3.line_offset, 30);
    assert_eq!(m3.line_count, 9);
}

#[test]
fn real_file_line0_vide() {
    let Some(cfg) = load_real() else { return };

    // m_friendMapLineInfo[0] du dump réel : lineNo=0, ligne vide
    let l0 = &cfg.lines[0];
    assert_eq!(l0.line_no, 0);
    assert!(
        l0.line_elem_ary.is_zero(),
        "m_friendMapLineInfo[0] doit être vide"
    );
}

#[test]
fn real_file_line5_premier_non_vide() {
    let Some(cfg) = load_real() else { return };

    // m_friendMapLineInfo[5] du dump réel : lineNo=5, premier non-vide de la carte 0
    let l5 = &cfg.lines[5];
    assert_eq!(l5.line_no, 5);
    assert_eq!(l5.line_elem_ary, HashId(0xE22B_E2FA));
}

#[test]
fn real_file_line7_hash() {
    let Some(cfg) = load_real() else { return };

    // m_friendMapLineInfo[7] du dump réel : lineNo=7, lineElemAry=0x5DF86A72
    let l7 = &cfg.lines[7];
    assert_eq!(l7.line_no, 7);
    assert_eq!(l7.line_elem_ary, HashId(0x5DF8_6A72));
}

#[test]
fn real_file_lines_of_map0() {
    let Some(cfg) = load_real() else { return };

    // Carte 0 : 14 lignes à partir de l'offset 0
    let lines = cfg.lines_of(&cfg.maps[0]);
    assert_eq!(lines.len(), 14, "carte 0 a exactement 14 lignes");
    assert_eq!(lines[0].line_no, 0, "première ligne = lineNo 0");
    assert!(lines[0].line_elem_ary.is_zero(), "lignes[0] = vide");
    assert_eq!(
        lines[5].line_elem_ary,
        HashId(0xE22B_E2FA),
        "lignes[5] = premier non-vide de la carte 0"
    );
}

#[test]
fn real_file_lines_of_map1() {
    let Some(cfg) = load_real() else { return };

    // Carte 1 : 7 lignes à partir de l'offset 14
    let lines = cfg.lines_of(&cfg.maps[1]);
    assert_eq!(lines.len(), 7, "carte 1 a exactement 7 lignes");
    // m_friendMapLineInfo[16] (offset 14 + index 2) : lineNo=2, lineElemAry=0x00739FA6
    assert_eq!(lines[2].line_no, 2);
    assert_eq!(lines[2].line_elem_ary, HashId(0x0073_9FA6));
}

#[test]
fn real_file_offsets_coherents() {
    let Some(cfg) = load_real() else { return };

    // La somme des line_count doit égaler le nombre total de lignes
    let total: i64 = cfg.maps.iter().map(|m| m.line_count).sum();
    assert_eq!(
        total as usize,
        cfg.lines.len(),
        "somme des line_count = total des lignes"
    );
}
