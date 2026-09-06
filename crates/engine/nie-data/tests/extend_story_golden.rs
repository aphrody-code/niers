#![allow(clippy::pedantic)]
//! Tests golden `extend_story` — listes réelles du noeud RDBN tiré de :
//! `data/common/gamedata/extend_story/extend_story_data_config_0.00.02.00.cfg.bin`
//! (VFS du jeu monté ; extrait via l'exemple jetable `nie-game --example extract_extend_story`).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/extend-story-config.ts` (`parseContent`).
//! Vérité terrain = la sortie iecode (format `lists`, champs nommés) du vrai fichier :
//! 1 histoire, 2 events d'entrée, 1 event de victoire, 1 event de défaite, 1 bloc de données.

use nie_data::extend_story::parse_extend_story_config;
use nie_data::hash::HashId;
use serde_json::{Value, json};

/// Reconstruit EXACTEMENT le JSON iecode du dump réel `extend_story_data_config_0.00.02.00`.
fn node_fixture() -> Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_exStoryDataConfigList",
                "typeName": "EXTEND_STORY_DATA_CONFIG",
                "values": [{
                    "explanationTextId": "0xA52DB12A",
                    "extendStoryClearFlgId": "0xF2F70331",
                    "extendStoryDataId": "0xDFCFCF24",
                    "extendStoryId": "0xB7DE3E13",
                    "extendStoryType": 1,
                    "titleTextId": "0x9DC1FC67",
                    "validCond": "AAAAABgFNSo9RUMACgEoAAYCNCy4aiUyAAAAAXg="
                }]
            },
            {
                "name": "m_exStoryGameEnterEvConfigList",
                "typeName": "EXTEND_STORYGAME_ENTER_EV_CONFIG",
                "values": [
                    { "enterEventId": "0x9425B7F6" },
                    { "enterEventId": "0x966309AF" }
                ]
            },
            {
                "name": "m_exStoryGameWinEvConfigList",
                "typeName": "EXTEND_STORYGAME_WIN_EV_CONFIG",
                "values": [{ "winEventId": "0x3B6369F8" }]
            },
            {
                "name": "m_exStoryGameLoseEvConfigList",
                "typeName": "EXTEND_STORYGAME_LOSE_EV_CONFIG",
                "values": [{ "loseEventId": "0x3925D7A1" }]
            },
            {
                "name": "m_exStoryGameDataConfigList",
                "typeName": "EXTEND_STORYGAME_DATA_CONFIG",
                "values": [{
                    "enterEvConfig": [0, 2],
                    "extendStoryGameId": "0xDFCFCF24",
                    "gameId": "0x68E507A0",
                    "loseEvConfig": [0, 1],
                    "winEvConfig": [0, 1]
                }]
            }
        ]
    })
}

#[test]
fn extend_story_cardinalites_des_cinq_listes() {
    let cfg = parse_extend_story_config(&node_fixture());
    // Cardinalités exactes du dump réel.
    assert_eq!(cfg.stories.len(), 1, "1 histoire étendue");
    assert_eq!(cfg.enter_events.len(), 2, "2 events d'entrée");
    assert_eq!(cfg.win_events.len(), 1, "1 event de victoire");
    assert_eq!(cfg.lose_events.len(), 1, "1 event de défaite");
    assert_eq!(cfg.game_data.len(), 1, "1 bloc de données de jeu");
}

#[test]
fn extend_story_data_champs_exacts() {
    let cfg = parse_extend_story_config(&node_fixture());
    let s = &cfg.stories[0];
    assert_eq!(s.extend_story_id, HashId(0xB7DE_3E13));
    assert_eq!(s.title_text_id, HashId(0x9DC1_FC67));
    assert_eq!(s.explanation_text_id, HashId(0xA52D_B12A));
    assert_eq!(s.extend_story_type, 1);
    assert_eq!(s.extend_story_data_id, HashId(0xDFCF_CF24));
    assert_eq!(s.extend_story_clear_flg_id, HashId(0xF2F7_0331));
    // validCond passé tel quel (champ Condition sérialisé), comme côté inagle.
    assert_eq!(s.valid_cond, "AAAAABgFNSo9RUMACgEoAAYCNCy4aiUyAAAAAXg=");
}

#[test]
fn extend_story_events_enter_win_lose() {
    let cfg = parse_extend_story_config(&node_fixture());
    assert_eq!(cfg.enter_events[0].enter_event_id, HashId(0x9425_B7F6));
    assert_eq!(cfg.enter_events[1].enter_event_id, HashId(0x9663_09AF));
    assert_eq!(cfg.win_events[0].win_event_id, HashId(0x3B63_69F8));
    assert_eq!(cfg.lose_events[0].lose_event_id, HashId(0x3925_D7A1));
}

#[test]
fn extend_story_game_data_paires_de_config() {
    let cfg = parse_extend_story_config(&node_fixture());
    let g = &cfg.game_data[0];
    assert_eq!(g.extend_story_game_id, HashId(0xDFCF_CF24));
    assert_eq!(g.game_id, HashId(0x68E5_07A0));
    // Paires [i32; 2] : enter=[0,2], win=[0,1], lose=[0,1].
    assert_eq!(g.enter_ev_config, [0, 2]);
    assert_eq!(g.win_ev_config, [0, 1]);
    assert_eq!(g.lose_ev_config, [0, 1]);
}

#[test]
fn extend_story_recherches_par_id() {
    let cfg = parse_extend_story_config(&node_fixture());
    // L'extendStoryDataId de l'histoire pointe vers l'extendStoryGameId du bloc de données.
    let story = cfg
        .find_by_story_id(HashId(0xB7DE_3E13))
        .expect("histoire trouvée");
    let game = cfg
        .find_game_data(story.extend_story_data_id)
        .expect("données de jeu reliées trouvées");
    assert_eq!(game.game_id, HashId(0x68E5_07A0));
    assert!(cfg.find_by_story_id(HashId(0xDEAD_BEEF)).is_none());
    assert!(cfg.find_game_data(HashId(0xDEAD_BEEF)).is_none());
}
