//! Famille `opponent_team` — port Rust de `opponent_team_config_*.cfg.bin` (équipes
//! adverses, difficultés et matchs d'entraînement).
//!
//! ## Vérité terrain
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/opponent-team-config.ts`
//!   (`parseContent`, l.66-112), port **1:1** ici.
//! - Sortie de référence (vérité terrain) : `packages/inagle/src/entries/opponent_teams.json`.
//! - Dump réel : `data/common/gamedata/team/opponent_team_config_1.03.05.00.cfg.bin`
//!   (VFS IEVR), format **RDBN** (`lists`).
//!
//! ## Format (RDBN `lists`)
//!
//! Un seul fichier, **quatre listes parallèles** :
//!
//! | Liste | `typeName` | Compte réel | Rôle |
//! |-------|------------|-------------|------|
//! | `m_OpponentTeamInfoList`     | `OPPONENT_TEAM_INFO`        | 17  | 1 équipe adverse jouable |
//! | `m_MatchDifficultyInfoList`  | `PRACTICE_MATCH_DIFFICULTY` | 404 | palier de difficulté + condition d'ouverture |
//! | `m_PracticeMatchGameInfoList`| `PRACTICE_MATCH_GAME_INFO`  | 101 | partie d'entraînement (index difficulté, condition) |
//! | `m_PracticeMatchInfoList`    | `PRACTICE_MATCH_INFO`       | 101 | match d'entraînement (catégorie, index partie) |
//!
//! ## Port 1:1 d'inagle — typos Level-5 préservées
//!
//! - Le champ brut de l'« id d'événement de rencontre » s'appelle **`meetingtEventId`**
//!   (typo Level-5 : un `t` en trop) ; inagle le lit tel quel (`val.meetingtEventId`) et
//!   l'expose sous `meetingEventId`. On préserve la lecture de la clé typotée.
//! - Le dump porte aussi un champ `meetingCond` (Hash) qu'inagle **n'expose pas** ; il est
//!   donc volontairement omis ici pour rester aligné sur la sortie de référence.
//! - inagle ne typifie que `m_OpponentTeamInfoList` ; les trois autres listes sont poussées
//!   telles quelles (`any`). On les surface ici en structs nommées d'après les **vrais**
//!   champs RDBN du dump (aucune valeur inventée), sans changer la sémantique d'inagle.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, field_str, list_values, owned};
use crate::hash::HashId;

/// Une entrée `m_OpponentTeamInfoList` (`OPPONENT_TEAM_INFO`) — une équipe adverse.
///
/// Port 1:1 de `ParsedOpponentTeam` (inagle `opponent-team-config.ts` l.24-41).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpponentTeam {
    /// `id` — identifiant de l'équipe adverse (hash, clé stable / `opponentId` côté inagle).
    pub opponent_id: HashId,
    /// `type` — type d'équipe adverse (`0` dans le dump réel).
    pub team_type: i64,
    /// `teamId` — hash de l'équipe associée (jointure vers les données d'équipe).
    pub team_id: HashId,
    /// `descTextId` — hash du texte de description.
    pub desc_text_id: HashId,
    /// `pointTextId` — hash du texte de points.
    pub point_text_id: HashId,
    /// `meetingtEventId` (typo Level-5 préservée) — hash de l'événement de rencontre
    /// (exposé `meetingEventId` côté inagle ; `0x00000000` = aucun).
    pub meeting_event_id: HashId,
    /// `flagNo` — numéro de drapeau de progression.
    pub flag_no: i64,
    /// `openCond` — condition d'ouverture (blob base64 d'octets de condition, ou `0xFFFFFFFF`).
    pub open_cond: String,
    /// `formationCond` — condition de formation (blob base64, ou `0xFFFFFFFF`).
    pub formation_cond: String,
    /// `menuBaseColor` — couleur de fond du menu (ARGB signé).
    pub menu_base_color: i64,
    /// `menuTextColor` — couleur du texte du menu (ARGB signé).
    pub menu_text_color: i64,
    /// `menuPlateColor` — couleur de la plaque du menu (ARGB signé).
    pub menu_plate_color: i64,
    /// `bgTextureName` — nom de la texture de fond (= `name` côté inagle).
    pub bg_texture_name: String,
    /// `bgTextureNameCrc` — CRC du nom de la texture de fond.
    pub bg_texture_name_crc: HashId,
    /// `gameId` — hash du game/partie associé.
    pub game_id: HashId,
    /// `difficultyType` — type de difficulté (`1` dans le dump réel).
    pub difficulty_type: i64,
}

impl OpponentTeam {
    /// Parse une valeur `OPPONENT_TEAM_INFO`. Port direct du `push({...})` d'inagle
    /// (l.77-100), avec la clé typotée `meetingtEventId` préservée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            opponent_id: field_hash(v, "id"),
            team_type: field_i64(v, "type").unwrap_or(0),
            team_id: field_hash(v, "teamId"),
            desc_text_id: field_hash(v, "descTextId"),
            point_text_id: field_hash(v, "pointTextId"),
            meeting_event_id: field_hash(v, "meetingtEventId"),
            flag_no: field_i64(v, "flagNo").unwrap_or(0),
            open_cond: owned(field_str(v, "openCond").unwrap_or("")),
            formation_cond: owned(field_str(v, "formationCond").unwrap_or("")),
            menu_base_color: field_i64(v, "menuBaseColor").unwrap_or(0),
            menu_text_color: field_i64(v, "menuTextColor").unwrap_or(0),
            menu_plate_color: field_i64(v, "menuPlateColor").unwrap_or(0),
            bg_texture_name: owned(field_str(v, "bgTextureName").unwrap_or("")),
            bg_texture_name_crc: field_hash(v, "bgTextureNameCrc"),
            game_id: field_hash(v, "gameId"),
            difficulty_type: field_i64(v, "difficultyType").unwrap_or(0),
        }
    }
}

/// Une entrée `m_MatchDifficultyInfoList` (`PRACTICE_MATCH_DIFFICULTY`).
///
/// inagle pousse cette liste telle quelle (`any`) ; champs réels du dump.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchDifficultyInfo {
    /// `difficulty` — palier de difficulté (1, 2, 3, … dans le dump réel).
    pub difficulty: i64,
    /// `openCond` — condition d'ouverture (blob base64, ou `0xFFFFFFFF` = aucune).
    pub open_cond: String,
}

impl MatchDifficultyInfo {
    /// Parse une valeur `PRACTICE_MATCH_DIFFICULTY`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            difficulty: field_i64(v, "difficulty").unwrap_or(0),
            open_cond: owned(field_str(v, "openCond").unwrap_or("")),
        }
    }
}

/// Une entrée `m_PracticeMatchGameInfoList` (`PRACTICE_MATCH_GAME_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PracticeMatchGameInfo {
    /// `difficultyInfo` — couple `[index, count]` dans `m_MatchDifficultyInfoList`
    /// (`ShortTuple` RDBN).
    pub difficulty_info: [i64; 2],
    /// `gameId` — hash du game/partie.
    pub game_id: HashId,
    /// `openCond` — condition d'ouverture (blob base64, ou `0xFFFFFFFF`).
    pub open_cond: String,
}

impl PracticeMatchGameInfo {
    /// Parse une valeur `PRACTICE_MATCH_GAME_INFO`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            difficulty_info: pair(v, "difficultyInfo"),
            game_id: field_hash(v, "gameId"),
            open_cond: owned(field_str(v, "openCond").unwrap_or("")),
        }
    }
}

/// Une entrée `m_PracticeMatchInfoList` (`PRACTICE_MATCH_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PracticeMatchInfo {
    /// `category` — catégorie du match d'entraînement.
    pub category: i64,
    /// `gameInfo` — couple `[index, count]` dans `m_PracticeMatchGameInfoList`
    /// (`ShortTuple` RDBN).
    pub game_info: [i64; 2],
    /// `id` — hash du match d'entraînement (clé stable).
    pub id: HashId,
    /// `newFlag` — hash du flag « nouveau ».
    pub new_flag: HashId,
}

impl PracticeMatchInfo {
    /// Parse une valeur `PRACTICE_MATCH_INFO`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            category: field_i64(v, "category").unwrap_or(0),
            game_info: pair(v, "gameInfo"),
            id: field_hash(v, "id"),
            new_flag: field_hash(v, "newFlag"),
        }
    }
}

/// Configuration complète d'`opponent_team_config` : les quatre listes parsées.
///
/// Port 1:1 du `{ opponents, matchDifficulties, practiceMatches, practiceMatchGames }`
/// d'inagle (`parseContent`, l.66-112).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpponentTeamConfig {
    /// `m_OpponentTeamInfoList` — équipes adverses.
    pub opponents: Vec<OpponentTeam>,
    /// `m_MatchDifficultyInfoList` — paliers de difficulté.
    pub match_difficulties: Vec<MatchDifficultyInfo>,
    /// `m_PracticeMatchGameInfoList` — parties d'entraînement.
    pub practice_match_games: Vec<PracticeMatchGameInfo>,
    /// `m_PracticeMatchInfoList` — matchs d'entraînement.
    pub practice_matches: Vec<PracticeMatchInfo>,
}

impl OpponentTeamConfig {
    /// Recherche une équipe adverse par son `opponentId` (équivalent du `byId` d'inagle).
    #[must_use]
    pub fn find_opponent_by_id(&self, id: HashId) -> Option<&OpponentTeam> {
        self.opponents.iter().find(|o| o.opponent_id == id)
    }

    /// Toutes les équipes adverses rattachées à un `teamId` (équivalent du `byTeamId` d'inagle).
    #[must_use]
    pub fn opponents_by_team_id(&self, team_id: HashId) -> Vec<&OpponentTeam> {
        self.opponents
            .iter()
            .filter(|o| o.team_id == team_id)
            .collect()
    }
}

/// Lit un champ couple `[a, b]` (`ShortTuple` RDBN → tableau JSON `[i16, i16]`), `(0, 0)` si
/// absent ou mal formé.
fn pair(v: &Value, key: &str) -> [i64; 2] {
    let arr = v.get(key).and_then(Value::as_array);
    let a = arr
        .and_then(|a| a.first())
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let b = arr
        .and_then(|a| a.get(1))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    [a, b]
}

/// Parse le `opponent_team_config_*.cfg.bin.json` complet (forme iecode `lists`).
///
/// Port 1:1 de `parseContent` (`opponent-team-config.ts` l.66-112) : on parcourt les quatre
/// listes nommées ; toute liste absente donne un `Vec` vide (`if (!content?.lists) return …`).
///
/// # Exemple
///
/// ```
/// use nie_data::opponent_team::parse_opponent_team_config;
/// use nie_data::hash::HashId;
/// use serde_json::json;
///
/// let root = json!({
///     "lists": [{
///         "name": "m_OpponentTeamInfoList",
///         "typeName": "OPPONENT_TEAM_INFO",
///         "values": [{
///             "id": "0x286AD48A", "type": 0, "teamId": "0xB247E862",
///             "descTextId": "0x781C740D", "pointTextId": "0x65B26158",
///             "meetingtEventId": "0x00000000", "flagNo": 4,
///             "openCond": "AAAAAA8FNbkZNtoAAQAyAAERynE=",
///             "formationCond": "AAAAAA8FNbkZNtoAAQAyAAERynE=",
///             "menuBaseColor": 11765759, "menuTextColor": -1761623809,
///             "menuPlateColor": 5665535, "bgTextureName": "opp110005",
///             "bgTextureNameCrc": "0x2B39B1BF", "gameId": "0x0148BC19",
///             "difficultyType": 1
///         }]
///     }]
/// });
/// let cfg = parse_opponent_team_config(&root);
/// assert_eq!(cfg.opponents.len(), 1);
/// assert_eq!(cfg.opponents[0].opponent_id, HashId(0x286A_D48A));
/// assert_eq!(cfg.opponents[0].meeting_event_id, HashId::ZERO);
/// assert_eq!(cfg.opponents[0].bg_texture_name, "opp110005");
/// ```
#[must_use]
pub fn parse_opponent_team_config(root: &Value) -> OpponentTeamConfig {
    /// Mappe une liste nommée vers un `Vec`, ou `Vec` vide si la liste est absente.
    fn collect<T>(root: &Value, name: &str, f: fn(&Value) -> T) -> Vec<T> {
        list_values(root, name)
            .map(|vs| vs.iter().map(f).collect())
            .unwrap_or_default()
    }
    OpponentTeamConfig {
        opponents: collect(root, "m_OpponentTeamInfoList", OpponentTeam::from_value),
        match_difficulties: collect(
            root,
            "m_MatchDifficultyInfoList",
            MatchDifficultyInfo::from_value,
        ),
        practice_match_games: collect(
            root,
            "m_PracticeMatchGameInfoList",
            PracticeMatchGameInfo::from_value,
        ),
        practice_matches: collect(
            root,
            "m_PracticeMatchInfoList",
            PracticeMatchInfo::from_value,
        ),
    }
}
