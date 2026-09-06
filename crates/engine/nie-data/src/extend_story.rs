//! `ExtendStoryConfig` — port de `extend_story_data_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/extend_story/extend_story_data_config_0.00.02.00.cfg.bin`
//! - Fichier unique, très petit (1687 octets), format RDBN à listes nommées.
//! - Port 1:1 d'inagle `packages/inagle/src/parsers/extend-story-config.ts` (`parseContent`).
//!
//! ## Structure (format `lists`, champs nommés)
//!
//! Cinq listes nommées composent la configuration des histoires étendues (contenu DLC /
//! scénarios additionnels) :
//!
//! | Liste                              | Type RDBN                          | Entrées (dump) |
//! |------------------------------------|------------------------------------|----------------|
//! | `m_exStoryDataConfigList`          | `EXTEND_STORY_DATA_CONFIG`         | 1              |
//! | `m_exStoryGameEnterEvConfigList`   | `EXTEND_STORYGAME_ENTER_EV_CONFIG` | 2              |
//! | `m_exStoryGameWinEvConfigList`     | `EXTEND_STORYGAME_WIN_EV_CONFIG`   | 1              |
//! | `m_exStoryGameLoseEvConfigList`    | `EXTEND_STORYGAME_LOSE_EV_CONFIG`  | 1              |
//! | `m_exStoryGameDataConfigList`      | `EXTEND_STORYGAME_DATA_CONFIG`     | 1              |
//!
//! La liste principale (`m_exStoryDataConfigList`) décrit chaque histoire étendue : ses
//! identifiants de texte (titre / explication), sa condition de validité, son type et ses
//! références de données et de drapeau de complétion. Les trois listes d'événements
//! (`*EnterEv*`, `*WinEv*`, `*LoseEv*`) listent les identifiants d'événements d'entrée, de
//! victoire et de défaite. Enfin `m_exStoryGameDataConfigList` relie une histoire à un
//! mini-jeu (`gameId`) et indexe, via des paires `[i32; 2]`, les événements à déclencher
//! à l'entrée, à la victoire et à la défaite.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, field_str, list_values, owned};
use crate::hash::HashId;

// ─── ExtendStoryData ───────────────────────────────────────────────────────────

/// Métadonnées d'une histoire étendue (`EXTEND_STORY_DATA_CONFIG`).
///
/// Entrée de `m_exStoryDataConfigList`. Port 1:1 de l'interface `ExtendStoryData`
/// d'inagle (`extend-story-config.ts` l.24-32).
///
/// Vérité terrain (1 entrée dans le dump) :
/// `extendStoryId = 0xB7DE3E13`, `extendStoryType = 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtendStoryData {
    /// `extendStoryId` — hash identifiant unique de l'histoire étendue.
    ///
    /// Vérité terrain : `0xB7DE3E13`.
    pub extend_story_id: HashId,
    /// `titleTextId` — hash du texte de titre (jointure table de textes du jeu).
    ///
    /// Vérité terrain : `0x9DC1FC67`.
    pub title_text_id: HashId,
    /// `explanationTextId` — hash du texte d'explication / résumé.
    ///
    /// Vérité terrain : `0xA52DB12A`.
    pub explanation_text_id: HashId,
    /// `validCond` — condition de validité (déblocage) de l'histoire, sérialisée en
    /// chaîne (champ `Condition` RDBN). Passée telle quelle, comme côté inagle.
    ///
    /// Vérité terrain : `"AAAAABgFNSo9RUMACgEoAAYCNCy4aiUyAAAAAXg="`.
    pub valid_cond: String,
    /// `extendStoryType` — type d'histoire étendue (entier).
    ///
    /// Vérité terrain : `1`.
    pub extend_story_type: i32,
    /// `extendStoryDataId` — hash de référence vers les données de jeu associées
    /// (relie à `extendStoryGameId` de `m_exStoryGameDataConfigList`).
    ///
    /// Vérité terrain : `0xDFCFCF24`.
    pub extend_story_data_id: HashId,
    /// `extendStoryClearFlgId` — hash du drapeau de progression marquant la complétion.
    ///
    /// Vérité terrain : `0xF2F70331`.
    pub extend_story_clear_flg_id: HashId,
}

impl ExtendStoryData {
    /// Parse une entrée de `m_exStoryDataConfigList`. Port 1:1 d'inagle (l.66-75).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            extend_story_id: field_hash(v, "extendStoryId"),
            title_text_id: field_hash(v, "titleTextId"),
            explanation_text_id: field_hash(v, "explanationTextId"),
            valid_cond: owned(field_str(v, "validCond").unwrap_or("")),
            extend_story_type: field_i64(v, "extendStoryType").unwrap_or(0) as i32,
            extend_story_data_id: field_hash(v, "extendStoryDataId"),
            extend_story_clear_flg_id: field_hash(v, "extendStoryClearFlgId"),
        }
    }
}

// ─── Événements (enter / win / lose) ───────────────────────────────────────────

/// Événement d'entrée d'un mini-jeu d'histoire étendue (`EXTEND_STORYGAME_ENTER_EV_CONFIG`).
///
/// Entrée de `m_exStoryGameEnterEvConfigList`. Vérité terrain : 2 entrées
/// (`0x9425B7F6`, `0x966309AF`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtendStoryEnterEvent {
    /// `enterEventId` — hash de l'événement déclenché à l'entrée.
    pub enter_event_id: HashId,
}

/// Événement de victoire d'un mini-jeu d'histoire étendue (`EXTEND_STORYGAME_WIN_EV_CONFIG`).
///
/// Entrée de `m_exStoryGameWinEvConfigList`. Vérité terrain : 1 entrée (`0x3B6369F8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtendStoryWinEvent {
    /// `winEventId` — hash de l'événement déclenché à la victoire.
    pub win_event_id: HashId,
}

/// Événement de défaite d'un mini-jeu d'histoire étendue (`EXTEND_STORYGAME_LOSE_EV_CONFIG`).
///
/// Entrée de `m_exStoryGameLoseEvConfigList`. Vérité terrain : 1 entrée (`0x3925D7A1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtendStoryLoseEvent {
    /// `loseEventId` — hash de l'événement déclenché à la défaite.
    pub lose_event_id: HashId,
}

// ─── ExtendStoryGameData ───────────────────────────────────────────────────────

/// Données de jeu d'une histoire étendue (`EXTEND_STORYGAME_DATA_CONFIG`).
///
/// Entrée de `m_exStoryGameDataConfigList`. Relie une histoire à un mini-jeu et indexe
/// les événements d'entrée / victoire / défaite via des paires `[i32; 2]`. Port 1:1 de
/// l'interface `ExtendStoryGameData` d'inagle (`extend-story-config.ts` l.34-40).
///
/// Vérité terrain (1 entrée) : `extendStoryGameId = 0xDFCFCF24`, `gameId = 0x68E507A0`,
/// `enterEvConfig = [0, 2]`, `winEvConfig = [0, 1]`, `loseEvConfig = [0, 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtendStoryGameData {
    /// `extendStoryGameId` — hash identifiant de ce bloc de données de jeu.
    /// Correspond à `extendStoryDataId` de l'histoire parente.
    ///
    /// Vérité terrain : `0xDFCFCF24`.
    pub extend_story_game_id: HashId,
    /// `gameId` — hash du mini-jeu / contenu jouable associé.
    ///
    /// Vérité terrain : `0x68E507A0`.
    pub game_id: HashId,
    /// `enterEvConfig` — paire indexant les événements d'entrée à déclencher.
    ///
    /// Vérité terrain : `[0, 2]` (les deux entrées de `m_exStoryGameEnterEvConfigList`).
    pub enter_ev_config: [i32; 2],
    /// `winEvConfig` — paire indexant les événements de victoire à déclencher.
    ///
    /// Vérité terrain : `[0, 1]`.
    pub win_ev_config: [i32; 2],
    /// `loseEvConfig` — paire indexant les événements de défaite à déclencher.
    ///
    /// Vérité terrain : `[0, 1]`.
    pub lose_ev_config: [i32; 2],
}

impl ExtendStoryGameData {
    /// Parse une entrée de `m_exStoryGameDataConfigList`. Port 1:1 d'inagle (l.85-91).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            extend_story_game_id: field_hash(v, "extendStoryGameId"),
            game_id: field_hash(v, "gameId"),
            enter_ev_config: parse_pair(v, "enterEvConfig"),
            win_ev_config: parse_pair(v, "winEvConfig"),
            lose_ev_config: parse_pair(v, "loseEvConfig"),
        }
    }
}

/// Extrait une paire d'entiers `[i32; 2]` depuis un champ tableau JSON.
///
/// Retourne `[0, 0]` si le champ est absent ou mal formé. Tolère entiers et flottants.
fn parse_pair(v: &Value, key: &str) -> [i32; 2] {
    let mut out = [0i32; 2];
    if let Some(arr) = v.get(key).and_then(Value::as_array) {
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(n) = arr.get(i).and_then(Value::as_i64) {
                *slot = n as i32;
            }
        }
    }
    out
}

// ─── ExtendStoryConfig ─────────────────────────────────────────────────────────

/// Contenu complet d'un `extend_story_data_config_*.cfg.bin.json` (5 listes).
///
/// Utiliser [`parse_extend_story_config`] pour construire depuis un JSON désérialisé.
///
/// # Exemple minimal
///
/// ```
/// use nie_data::extend_story::parse_extend_story_config;
/// use nie_data::hash::HashId;
/// use serde_json::json;
///
/// let data = json!({
///     "version": 100,
///     "lists": [{
///         "name": "m_exStoryDataConfigList",
///         "typeName": "EXTEND_STORY_DATA_CONFIG",
///         "values": [{
///             "extendStoryId": "0xB7DE3E13",
///             "titleTextId": "0x9DC1FC67",
///             "explanationTextId": "0xA52DB12A",
///             "validCond": "AAAA",
///             "extendStoryType": 1,
///             "extendStoryDataId": "0xDFCFCF24",
///             "extendStoryClearFlgId": "0xF2F70331"
///         }]
///     }]
/// });
/// let cfg = parse_extend_story_config(&data);
/// assert_eq!(cfg.stories.len(), 1);
/// assert_eq!(cfg.stories[0].extend_story_id, HashId(0xB7DE_3E13));
/// assert!(cfg.find_by_story_id(HashId(0xB7DE_3E13)).is_some());
/// ```
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtendStoryConfig {
    /// Métadonnées des histoires étendues (`m_exStoryDataConfigList`).
    pub stories: Vec<ExtendStoryData>,
    /// Événements d'entrée (`m_exStoryGameEnterEvConfigList`).
    pub enter_events: Vec<ExtendStoryEnterEvent>,
    /// Événements de victoire (`m_exStoryGameWinEvConfigList`).
    pub win_events: Vec<ExtendStoryWinEvent>,
    /// Événements de défaite (`m_exStoryGameLoseEvConfigList`).
    pub lose_events: Vec<ExtendStoryLoseEvent>,
    /// Données de jeu (`m_exStoryGameDataConfigList`).
    pub game_data: Vec<ExtendStoryGameData>,
}

impl ExtendStoryConfig {
    /// Recherche une histoire étendue par son `extendStoryId`. Renvoie `None` si absente.
    ///
    /// Équivalent de la `Map byStoryId` construite par `buildExtendStoryDatabase` d'inagle.
    #[must_use]
    pub fn find_by_story_id(&self, id: HashId) -> Option<&ExtendStoryData> {
        self.stories.iter().find(|s| s.extend_story_id == id)
    }

    /// Recherche les données de jeu associées à un `extendStoryGameId`. Renvoie `None`
    /// si absentes. (L'`extendStoryDataId` d'une histoire pointe vers ce `extendStoryGameId`.)
    #[must_use]
    pub fn find_game_data(&self, game_id: HashId) -> Option<&ExtendStoryGameData> {
        self.game_data
            .iter()
            .find(|g| g.extend_story_game_id == game_id)
    }
}

// ─── Parseur principal ─────────────────────────────────────────────────────────

/// Parse un `extend_story_data_config_*.cfg.bin.json` complet (5 listes).
///
/// Port 1:1 d'inagle `parseContent` (`extend-story-config.ts` l.55-95) : aucune entrée
/// n'est filtrée, l'ordre des listes est préservé.
///
/// Vérité terrain : `extend_story_data_config_0.00.02.00.cfg.bin` → 1 histoire, 2 events
/// d'entrée, 1 event de victoire, 1 event de défaite, 1 bloc de données de jeu.
#[must_use]
pub fn parse_extend_story_config(root: &Value) -> ExtendStoryConfig {
    let mut cfg = ExtendStoryConfig::default();

    if let Some(values) = list_values(root, "m_exStoryDataConfigList") {
        cfg.stories = values.iter().map(ExtendStoryData::from_value).collect();
    }
    if let Some(values) = list_values(root, "m_exStoryGameEnterEvConfigList") {
        cfg.enter_events = values
            .iter()
            .map(|v| ExtendStoryEnterEvent {
                enter_event_id: field_hash(v, "enterEventId"),
            })
            .collect();
    }
    if let Some(values) = list_values(root, "m_exStoryGameWinEvConfigList") {
        cfg.win_events = values
            .iter()
            .map(|v| ExtendStoryWinEvent {
                win_event_id: field_hash(v, "winEventId"),
            })
            .collect();
    }
    if let Some(values) = list_values(root, "m_exStoryGameLoseEvConfigList") {
        cfg.lose_events = values
            .iter()
            .map(|v| ExtendStoryLoseEvent {
                lose_event_id: field_hash(v, "loseEventId"),
            })
            .collect();
    }
    if let Some(values) = list_values(root, "m_exStoryGameDataConfigList") {
        cfg.game_data = values.iter().map(ExtendStoryGameData::from_value).collect();
    }

    cfg
}
