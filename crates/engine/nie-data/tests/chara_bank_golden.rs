#![allow(clippy::pedantic)]
//! Tests golden `chara_bank` — valeurs réelles tirées de :
//! `chara_bank/soccer_club_room_config.cfg.bin.json`
//!
//! ## Vérifications champ par champ (fichier → valeur confirmée)
//!
//! Liste `m_soccerClubRoomCharaRestrictionInfoList` (41 entrées, `version = 100`).
//!
//! ### [0] (seule entrée avec is_team_dock_change_disabled = true)
//! - `character_id`                 = `"0x686BE87E"` → `HashId(0x686BE87E)`
//! - `is_remove_club_disabled`      = true
//! - `is_chara_bank_move_disabled`  = true
//! - `is_team_dock_change_disabled` = true
//!
//! ### [1]
//! - `character_id`                 = `"0x4346BBBD"`
//! - `is_chara_bank_move_disabled`  = true
//! - `is_team_dock_change_disabled` = false
//!
//! ### [9] (dernière des 10 entrées is_chara_bank_move_disabled = true)
//! - `character_id`                 = `"0x77BB1BD4"`
//! - `is_chara_bank_move_disabled`  = true
//!
//! ### [10]
//! - `character_id`                 = `"0xB9A95337"`
//! - `is_chara_bank_move_disabled`  = false
//!
//! ### [40] (dernier)
//! - `character_id`                 = `"0x5C2847EE"`
//! - `is_remove_club_disabled`      = true
//! - `is_chara_bank_move_disabled`  = false
//! - `is_team_dock_change_disabled` = false

mod common;

use nie_data::chara_bank::{
    CharaRestrictionInfo, SoccerClubRoomConfig, parse_soccer_club_room_config,
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
                "name": "m_soccerClubRoomCharaRestrictionInfoList",
                "typeName": "SOCCER_CLUB_ROOM_CHARA_RESTRICTION_INFO",
                "values": [
                    {
                        "character_id": "0x686BE87E",
                        "is_remove_club_disabled": true,
                        "is_chara_bank_move_disabled": true,
                        "is_team_dock_change_disabled": true
                    },
                    {
                        "character_id": "0x4346BBBD",
                        "is_remove_club_disabled": true,
                        "is_chara_bank_move_disabled": true,
                        "is_team_dock_change_disabled": false
                    },
                    {
                        "character_id": "0xB9A95337",
                        "is_remove_club_disabled": true,
                        "is_chara_bank_move_disabled": false,
                        "is_team_dock_change_disabled": false
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn fixture_compte_entrees() {
    let cfg = parse_soccer_club_room_config(&fixture());
    assert_eq!(cfg.restrictions.len(), 3);
}

#[test]
fn fixture_entree0_champs_complets() {
    let cfg = parse_soccer_club_room_config(&fixture());
    let e: &CharaRestrictionInfo = &cfg.restrictions[0];

    // soccer_club_room_config : m_soccerClubRoomCharaRestrictionInfoList[0]
    assert_eq!(e.character_id, HashId(0x686B_E87E));
    assert!(e.is_remove_club_disabled);
    assert!(e.is_chara_bank_move_disabled);
    assert!(e.is_team_dock_change_disabled);
}

#[test]
fn fixture_entree1_dock_change_false() {
    let cfg = parse_soccer_club_room_config(&fixture());
    let e = &cfg.restrictions[1];

    assert_eq!(e.character_id, HashId(0x4346_BBBD));
    assert!(e.is_chara_bank_move_disabled);
    assert!(!e.is_team_dock_change_disabled);
}

#[test]
fn fixture_entree2_bank_move_false() {
    let cfg = parse_soccer_club_room_config(&fixture());
    let e = &cfg.restrictions[2];

    assert_eq!(e.character_id, HashId(0xB9A9_5337));
    assert!(e.is_remove_club_disabled);
    assert!(!e.is_chara_bank_move_disabled);
    assert!(!e.is_team_dock_change_disabled);
}

#[test]
fn fixture_find_by_character_present() {
    let cfg = parse_soccer_club_room_config(&fixture());
    let found = cfg.find_by_character(HashId(0x686B_E87E));
    assert!(found.is_some(), "find_by_character doit trouver 0x686BE87E");
    assert!(found.unwrap().is_team_dock_change_disabled);
}

#[test]
fn fixture_find_by_character_absent() {
    let cfg = parse_soccer_club_room_config(&fixture());
    assert!(
        cfg.find_by_character(HashId(0xDEAD_BEEF)).is_none(),
        "find_by_character doit renvoyer None pour un id absent"
    );
}

#[test]
fn fixture_liste_manquante_renvoie_vide() {
    // Un JSON sans la liste doit renvoyer une config vide sans paniquer.
    let root = serde_json::json!({ "version": 100, "lists": [] });
    let cfg = parse_soccer_club_room_config(&root);
    assert_eq!(cfg.restrictions.len(), 0);
}

// ─── Test sur le vrai fichier (skip si absent du VPS) ────────────────────────

const REAL_PATH: &str = "chara_bank/soccer_club_room_config.cfg.bin.json";

fn load_real() -> Option<SoccerClubRoomConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_soccer_club_room_config(&root))
}

#[test]
fn real_file_compte_total() {
    let Some(cfg) = load_real() else { return };
    // soccer_club_room_config : 41 entrées dans m_soccerClubRoomCharaRestrictionInfoList
    assert_eq!(
        cfg.restrictions.len(),
        41,
        "41 entrées SOCCER_CLUB_ROOM_CHARA_RESTRICTION_INFO dans le dump réel"
    );
}

#[test]
fn real_file_entree0_valeurs() {
    let Some(cfg) = load_real() else { return };

    // m_soccerClubRoomCharaRestrictionInfoList[0] — seule entrée avec dock_change = true
    let e = &cfg.restrictions[0];
    assert_eq!(e.character_id, HashId(0x686B_E87E), "character_id[0]");
    assert!(e.is_remove_club_disabled, "is_remove_club_disabled[0]");
    assert!(
        e.is_chara_bank_move_disabled,
        "is_chara_bank_move_disabled[0]"
    );
    assert!(
        e.is_team_dock_change_disabled,
        "is_team_dock_change_disabled[0]"
    );
}

#[test]
fn real_file_entree1_valeurs() {
    let Some(cfg) = load_real() else { return };

    // m_soccerClubRoomCharaRestrictionInfoList[1]
    let e = &cfg.restrictions[1];
    assert_eq!(e.character_id, HashId(0x4346_BBBD), "character_id[1]");
    assert!(e.is_remove_club_disabled, "is_remove_club_disabled[1]");
    assert!(
        e.is_chara_bank_move_disabled,
        "is_chara_bank_move_disabled[1]"
    );
    assert!(
        !e.is_team_dock_change_disabled,
        "is_team_dock_change_disabled[1] = false"
    );
}

#[test]
fn real_file_entree9_bank_move_dernier_true() {
    let Some(cfg) = load_real() else { return };

    // m_soccerClubRoomCharaRestrictionInfoList[9] — dernière des 10 entrées bank_move = true
    let e = &cfg.restrictions[9];
    assert_eq!(e.character_id, HashId(0x77BB_1BD4), "character_id[9]");
    assert!(
        e.is_chara_bank_move_disabled,
        "is_chara_bank_move_disabled[9]"
    );
    assert!(
        !e.is_team_dock_change_disabled,
        "is_team_dock_change_disabled[9] = false"
    );
}

#[test]
fn real_file_entree10_bank_move_false() {
    let Some(cfg) = load_real() else { return };

    // m_soccerClubRoomCharaRestrictionInfoList[10] — bank_move repasse à false
    let e = &cfg.restrictions[10];
    assert_eq!(e.character_id, HashId(0xB9A9_5337), "character_id[10]");
    assert!(e.is_remove_club_disabled, "is_remove_club_disabled[10]");
    assert!(
        !e.is_chara_bank_move_disabled,
        "is_chara_bank_move_disabled[10] = false"
    );
}

#[test]
fn real_file_dernier_entree() {
    let Some(cfg) = load_real() else { return };

    // m_soccerClubRoomCharaRestrictionInfoList[40] (dernier)
    let e = &cfg.restrictions[40];
    assert_eq!(e.character_id, HashId(0x5C28_47EE), "character_id[40]");
    assert!(e.is_remove_club_disabled, "is_remove_club_disabled[40]");
    assert!(
        !e.is_chara_bank_move_disabled,
        "is_chara_bank_move_disabled[40] = false"
    );
    assert!(
        !e.is_team_dock_change_disabled,
        "is_team_dock_change_disabled[40] = false"
    );
}

#[test]
fn real_file_remove_club_toujours_vrai() {
    let Some(cfg) = load_real() else { return };

    // Dans tout le dump, is_remove_club_disabled = true pour les 41 entrées.
    let tous_vrai = cfg.restrictions.iter().all(|e| e.is_remove_club_disabled);
    assert!(
        tous_vrai,
        "is_remove_club_disabled = true pour les 41 entrées du dump réel"
    );
}

#[test]
fn real_file_bank_move_dix_entrees() {
    let Some(cfg) = load_real() else { return };

    // is_chara_bank_move_disabled = true pour exactement 10 entrées (les 10 premières).
    let n = cfg
        .restrictions
        .iter()
        .filter(|e| e.is_chara_bank_move_disabled)
        .count();
    assert_eq!(n, 10, "10 entrées avec is_chara_bank_move_disabled = true");
}

#[test]
fn real_file_dock_change_une_seule_entree() {
    let Some(cfg) = load_real() else { return };

    // is_team_dock_change_disabled = true pour une seule entrée (0x686BE87E).
    let avec_dock: Vec<_> = cfg
        .restrictions
        .iter()
        .filter(|e| e.is_team_dock_change_disabled)
        .collect();
    assert_eq!(
        avec_dock.len(),
        1,
        "une seule entrée avec dock_change = true"
    );
    assert_eq!(avec_dock[0].character_id, HashId(0x686B_E87E));
}

#[test]
fn real_file_find_by_character() {
    let Some(cfg) = load_real() else { return };

    // character_id "0x77BB1BD4" → entrée [9]
    let found = cfg.find_by_character(HashId(0x77BB_1BD4));
    assert!(
        found.is_some(),
        "find_by_character(0x77BB1BD4) doit trouver l'entrée"
    );
    assert!(found.unwrap().is_chara_bank_move_disabled);
}
