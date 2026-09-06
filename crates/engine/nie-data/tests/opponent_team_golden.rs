#![allow(clippy::pedantic)]
//! Tests golden `opponent_team` — noeud réel tiré du VFS IEVR :
//! `data/common/gamedata/team/opponent_team_config_1.03.05.00.cfg.bin`
//! (RDBN `lists`, 4 listes : 17 / 404 / 101 / 101 entrées).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/opponent-team-config.ts`
//! (`parseContent`, l.66-112) : clé typotée Level-5 `meetingtEventId` lue telle quelle,
//! `meetingCond` non exposé (comme inagle). Valeurs ci-dessous extraites telles quelles du
//! dump (via `cfgbin::read_values` → forme iecode), aucune valeur inventée.

use nie_data::hash::HashId;
use nie_data::opponent_team::{
    MatchDifficultyInfo, OpponentTeam, PracticeMatchGameInfo, PracticeMatchInfo,
    parse_opponent_team_config,
};
use serde_json::{Value, json};

/// 3 premières entrées réelles de `m_OpponentTeamInfoList` (index 0,1,2 du dump).
fn opponents_head() -> Vec<Value> {
    vec![
        json!({
            "id": "0x286AD48A", "type": 0, "teamId": "0xB247E862",
            "descTextId": "0x781C740D", "pointTextId": "0x65B26158",
            "meetingCond": "0xFFFFFFFF", "meetingtEventId": "0x00000000", "flagNo": 4,
            "openCond": "AAAAAA8FNbkZNtoAAQAyAAERynE=",
            "formationCond": "AAAAAA8FNbkZNtoAAQAyAAERynE=",
            "menuBaseColor": 11765759, "menuTextColor": -1761623809,
            "menuPlateColor": 5665535, "bgTextureName": "opp110005",
            "bgTextureNameCrc": "0x2B39B1BF", "gameId": "0x0148BC19", "difficultyType": 1
        }),
        json!({
            "id": "0xC664B5A6", "type": 0, "teamId": "0xC5D90D26",
            "descTextId": "0x96121521", "pointTextId": "0x8BBC0074",
            "meetingCond": "0xFFFFFFFF", "meetingtEventId": "0x00000000", "flagNo": 2,
            "openCond": "AAAAAA8FNbkZNtoAAQAyAADqanE=",
            "formationCond": "AAAAAA8FNbkZNtoAAQAyAADqanE=",
            "menuBaseColor": -192675585, "menuTextColor": -944385,
            "menuPlateColor": -1051918081, "bgTextureName": "opp110003",
            "bgTextureNameCrc": "0xC25A148A", "gameId": "0x961A3DE4", "difficultyType": 1
        }),
        json!({
            "id": "0xBFB80D02", "type": 0, "teamId": "0x853F4DCA",
            "descTextId": "0x160075DA", "pointTextId": "0x0BAE608F",
            "meetingCond": "0xFFFFFFFF", "meetingtEventId": "0x00000000", "flagNo": 9,
            "openCond": "AAAAAA8FNbkZNtoAAQAyAAERynE=",
            "formationCond": "AAAAAA8FNbkZNtoAAQAyAAERynE=",
            "menuBaseColor": -1638774017, "menuTextColor": -2437633,
            "menuPlateColor": -1643114241, "bgTextureName": "opp110006",
            "bgTextureNameCrc": "0xB230E005", "gameId": "0x03C3E5FC", "difficultyType": 1
        }),
    ]
}

/// 3 premières entrées réelles de `m_MatchDifficultyInfoList` (index 0,1,2 du dump).
fn match_difficulties_head() -> Vec<Value> {
    vec![
        json!({ "difficulty": 1, "openCond": "0xFFFFFFFF" }),
        json!({ "difficulty": 2,
                "openCond": "AAAAACEFNbgXycUAEwIoAAYCNPYugXcoAAYCMgAAAAEyAAAAAXg=" }),
        json!({ "difficulty": 3,
                "openCond": "AAAAACEFNbgXycUAEwIoAAYCNPYugXcoAAYCMgAAAAIyAAAAAXg=" }),
    ]
}

/// 3 premières entrées réelles de `m_PracticeMatchGameInfoList` (index 0,1,2 du dump).
fn practice_match_games_head() -> Vec<Value> {
    vec![
        json!({ "difficultyInfo": [0, 4], "gameId": "0xF62E8177",
                "openCond": "AAAAAD8RNbkZNtoAAQAyAAB1nnE1vgSlmAAKASgABgI0QafebDIAAAACeY81vgSlmAAKASgABgI0BgekvDIAAAACeY8=" }),
        json!({ "difficultyInfo": [4, 4], "gameId": "0x729DEDAD",
                "openCond": "AAAAABgFNb4EpZgACgEoAAYCNEGn3mwyAAAAAng=" }),
        json!({ "difficultyInfo": [8, 4], "gameId": "0x353D977D",
                "openCond": "AAAAABgFNb4EpZgACgEoAAYCNAYHpLwyAAAAAng=" }),
    ]
}

/// 3 premières entrées réelles de `m_PracticeMatchInfoList` (index 0,1,2 du dump).
fn practice_matches_head() -> Vec<Value> {
    vec![
        json!({ "category": 0, "gameInfo": [0, 1], "id": "0x2F071093", "newFlag": "0xB52AF77B" }),
        json!({ "category": 0, "gameInfo": [1, 1], "id": "0x132E8DE9", "newFlag": "0x2C23A6C1" }),
        json!({ "category": 0, "gameInfo": [2, 1], "id": "0x548EF739", "newFlag": "0x5B249657" }),
    ]
}

/// Fixture iecode complète : les 4 listes (3 entrées réelles chacune).
fn node_fixture() -> Value {
    json!({
        "lists": [
            { "name": "m_OpponentTeamInfoList", "typeName": "OPPONENT_TEAM_INFO",
              "values": opponents_head() },
            { "name": "m_MatchDifficultyInfoList", "typeName": "PRACTICE_MATCH_DIFFICULTY",
              "values": match_difficulties_head() },
            { "name": "m_PracticeMatchGameInfoList", "typeName": "PRACTICE_MATCH_GAME_INFO",
              "values": practice_match_games_head() },
            { "name": "m_PracticeMatchInfoList", "typeName": "PRACTICE_MATCH_INFO",
              "values": practice_matches_head() },
        ]
    })
}

#[test]
fn parse_compte_les_quatre_listes() {
    let cfg = parse_opponent_team_config(&node_fixture());
    assert_eq!(cfg.opponents.len(), 3);
    assert_eq!(cfg.match_difficulties.len(), 3);
    assert_eq!(cfg.practice_match_games.len(), 3);
    assert_eq!(cfg.practice_matches.len(), 3);
}

#[test]
fn opponent_zero_champs_exacts() {
    let cfg = parse_opponent_team_config(&node_fixture());
    let o: &OpponentTeam = &cfg.opponents[0];
    assert_eq!(o.opponent_id, HashId(0x286A_D48A));
    assert_eq!(o.team_type, 0);
    assert_eq!(o.team_id, HashId(0xB247_E862));
    assert_eq!(o.desc_text_id, HashId(0x781C_740D));
    assert_eq!(o.point_text_id, HashId(0x65B2_6158));
    // Clé typotée Level-5 `meetingtEventId` = 0x00000000 → HashId::ZERO.
    assert_eq!(o.meeting_event_id, HashId::ZERO);
    assert_eq!(o.flag_no, 4);
    assert_eq!(o.open_cond, "AAAAAA8FNbkZNtoAAQAyAAERynE=");
    assert_eq!(o.formation_cond, "AAAAAA8FNbkZNtoAAQAyAAERynE=");
    assert_eq!(o.menu_base_color, 11_765_759);
    assert_eq!(o.menu_text_color, -1_761_623_809);
    assert_eq!(o.menu_plate_color, 5_665_535);
    assert_eq!(o.bg_texture_name, "opp110005");
    assert_eq!(o.bg_texture_name_crc, HashId(0x2B39_B1BF));
    assert_eq!(o.game_id, HashId(0x0148_BC19));
    assert_eq!(o.difficulty_type, 1);
}

#[test]
fn opponent_un_couleurs_signees() {
    // Valide la conservation des couleurs ARGB signées négatives (Int RDBN, pas de hash).
    let cfg = parse_opponent_team_config(&node_fixture());
    let o = &cfg.opponents[1];
    assert_eq!(o.opponent_id, HashId(0xC664_B5A6));
    assert_eq!(o.bg_texture_name, "opp110003");
    assert_eq!(o.menu_base_color, -192_675_585);
    assert_eq!(o.menu_text_color, -944_385);
    assert_eq!(o.menu_plate_color, -1_051_918_081);
}

#[test]
fn recherche_par_id_et_team_id() {
    let cfg = parse_opponent_team_config(&node_fixture());
    let found = cfg.find_opponent_by_id(HashId(0xBFB8_0D02));
    assert!(found.is_some());
    assert_eq!(found.unwrap().bg_texture_name, "opp110006");
    assert!(cfg.find_opponent_by_id(HashId(0xDEAD_BEEF)).is_none());

    // byTeamId : chaque teamId du dump head est unique → 1 résultat.
    let by_team = cfg.opponents_by_team_id(HashId(0xC5D9_0D26));
    assert_eq!(by_team.len(), 1);
    assert_eq!(by_team[0].opponent_id, HashId(0xC664_B5A6));
    assert!(cfg.opponents_by_team_id(HashId(0x0000_0000)).is_empty());
}

#[test]
fn match_difficulty_open_cond_hash_ou_base64() {
    let cfg = parse_opponent_team_config(&node_fixture());
    let d0: &MatchDifficultyInfo = &cfg.match_difficulties[0];
    assert_eq!(d0.difficulty, 1);
    // openCond peut être le sentinel `0xFFFFFFFF` (condition nulle) — lu en String tel quel.
    assert_eq!(d0.open_cond, "0xFFFFFFFF");
    assert_eq!(cfg.match_difficulties[1].difficulty, 2);
    assert_eq!(
        cfg.match_difficulties[1].open_cond,
        "AAAAACEFNbgXycUAEwIoAAYCNPYugXcoAAYCMgAAAAEyAAAAAXg="
    );
    assert_eq!(cfg.match_difficulties[2].difficulty, 3);
}

#[test]
fn practice_match_game_short_tuple() {
    let cfg = parse_opponent_team_config(&node_fixture());
    let g0: &PracticeMatchGameInfo = &cfg.practice_match_games[0];
    assert_eq!(g0.difficulty_info, [0, 4]);
    assert_eq!(g0.game_id, HashId(0xF62E_8177));
    assert_eq!(cfg.practice_match_games[1].difficulty_info, [4, 4]);
    assert_eq!(cfg.practice_match_games[1].game_id, HashId(0x729D_EDAD));
    assert_eq!(cfg.practice_match_games[2].difficulty_info, [8, 4]);
}

#[test]
fn practice_match_info_champs() {
    let cfg = parse_opponent_team_config(&node_fixture());
    let m0: &PracticeMatchInfo = &cfg.practice_matches[0];
    assert_eq!(m0.category, 0);
    assert_eq!(m0.game_info, [0, 1]);
    assert_eq!(m0.id, HashId(0x2F07_1093));
    assert_eq!(m0.new_flag, HashId(0xB52A_F77B));
    assert_eq!(cfg.practice_matches[1].id, HashId(0x132E_8DE9));
    assert_eq!(cfg.practice_matches[1].game_info, [1, 1]);
    assert_eq!(cfg.practice_matches[2].id, HashId(0x548E_F739));
    assert_eq!(cfg.practice_matches[2].new_flag, HashId(0x5B24_9657));
}

#[test]
fn vrai_fichier_si_present() {
    // Validation contre le vrai dump si monté localement (sinon skip).
    // Source VFS : data/common/gamedata/team/opponent_team_config_1.03.05.00.cfg.bin.
    let path = "/home/aphrody/niers/data/common/gamedata/team/\
                opponent_team_config_1.03.05.00.cfg.bin.json";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let content = std::fs::read_to_string(path).unwrap();
    let root: Value = serde_json::from_str(&content).unwrap();
    let cfg = parse_opponent_team_config(&root);
    assert_eq!(
        cfg.opponents.len(),
        17,
        "17 équipes adverses dans le vrai dump"
    );
    assert_eq!(cfg.match_difficulties.len(), 404);
    assert_eq!(cfg.practice_match_games.len(), 101);
    assert_eq!(cfg.practice_matches.len(), 101);
    assert_eq!(cfg.opponents[0].opponent_id, HashId(0x286A_D48A));
}
