//! `game_quest` — port de `soccer/game_quest_config_*.cfg.bin` (Level-5 IEVR).
//!
//! Base des **défis/quêtes de match** (les objectifs de match : « gagner avec 3 buts », etc.).
//! Famille non couverte ; port direct. 4 listes, à tranches `[offset, count]` imbriquées.
//!
//! ## Vérité terrain
//!
//! Dump réel : `data/common/gamedata/soccer/game_quest_config_1.02.33.cfg.bin.json` (format `lists`).
//! - `m_iconList` (713) — `paramId` (hash) des icônes/récompenses.
//! - `m_questDataList` (948) — un objectif : `explainTextId`, `condCount`, `questFlagNum`,
//!   `openPhase`, `itemTableId`, `limit` (u32 hex), `icon` `[offset,count]`.
//! - `m_gameInfoList` (175) — `difficultyType`.
//! - `m_gameQuestInfoList` (175) — un défi : `gameQuestId`, `titleTextId`, `gameIdArray`/`questData`
//!   `[offset,count]`, `gameQuestType`, `gameQuestFlagNum`, `exGameSettingId`.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, field_pair, field_str, list_values};
use crate::hash::HashId;

/// `m_questDataList` — un objectif de quête.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QuestData {
    /// `explainTextId` — hash du texte d'explication.
    pub explain_text_id: HashId,
    /// `condCount` — nombre de conditions.
    pub cond_count: i64,
    /// `questFlagNum` — numéro de flag de quête.
    pub quest_flag_num: i64,
    /// `openPhase` — phase d'ouverture.
    pub open_phase: i64,
    /// `itemTableId` — table de récompense (`0` = aucune).
    pub item_table_id: HashId,
    /// `limit` — limite (u32 hex ; `0xFFFFFFFF` = aucune).
    pub limit: u32,
    /// `icon` — `[offset, count]` indexant `m_iconList`.
    pub icon: [i64; 2],
}

impl QuestData {
    /// Parse une entrée (infaillible : indexée par tranche, position significative).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            explain_text_id: field_hash(v, "explainTextId"),
            cond_count: field_i64(v, "condCount").unwrap_or(0),
            quest_flag_num: field_i64(v, "questFlagNum").unwrap_or(0),
            open_phase: field_i64(v, "openPhase").unwrap_or(0),
            item_table_id: field_hash(v, "itemTableId"),
            // `limit` est une chaîne hexadécimale brute (sans préfixe `0x`), ex. "FFFFFFFF".
            limit: field_str(v, "limit")
                .and_then(|s| u32::from_str_radix(s.trim(), 16).ok())
                .unwrap_or(0),
            icon: field_pair(v, "icon").unwrap_or([0, 0]),
        }
    }
}

/// `m_gameQuestInfoList` — un défi de match (référence des objectifs + des matchs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameQuestInfo {
    /// `gameQuestId` — hash du défi.
    pub game_quest_id: HashId,
    /// `titleTextId` — hash du titre.
    pub title_text_id: HashId,
    /// `gameIdArray` — `[offset, count]` indexant `m_gameInfoList`.
    pub game_id_array: [i64; 2],
    /// `questData` — `[offset, count]` indexant `m_questDataList`.
    pub quest_data: [i64; 2],
    /// `gameQuestType` — type de défi.
    pub game_quest_type: i64,
    /// `gameQuestFlagNum` — numéro de flag.
    pub game_quest_flag_num: i64,
    /// `exGameSettingId` — réglage de match étendu (`0` = aucun).
    pub ex_game_setting_id: HashId,
}

impl GameQuestInfo {
    /// Parse une entrée. `None` si `gameQuestId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let game_quest_id = field_hash(v, "gameQuestId");
        if game_quest_id.is_zero() {
            return None;
        }
        Some(Self {
            game_quest_id,
            title_text_id: field_hash(v, "titleTextId"),
            game_id_array: field_pair(v, "gameIdArray").unwrap_or([0, 0]),
            quest_data: field_pair(v, "questData").unwrap_or([0, 0]),
            game_quest_type: field_i64(v, "gameQuestType").unwrap_or(0),
            game_quest_flag_num: field_i64(v, "gameQuestFlagNum").unwrap_or(0),
            ex_game_setting_id: field_hash(v, "exGameSettingId"),
        })
    }
}

/// Contenu complet d'un `game_quest_config_*.cfg.bin`.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameQuestConfig {
    /// `m_iconList` — paramIds d'icônes (indexés par `QuestData::icon`).
    pub icons: Vec<HashId>,
    /// `m_questDataList` — objectifs (indexés par `GameQuestInfo::quest_data`).
    pub quest_data: Vec<QuestData>,
    /// `m_gameInfoList` — `difficultyType` par game (indexé par `GameQuestInfo::game_id_array`).
    pub game_infos: Vec<i64>,
    /// `m_gameQuestInfoList` — défis de match.
    pub game_quests: Vec<GameQuestInfo>,
}

impl GameQuestConfig {
    /// Résout les objectifs d'un défi (`quest_data` slice dans `quest_data`).
    #[must_use]
    pub fn quests_of(&self, q: &GameQuestInfo) -> &[QuestData] {
        slice(&self.quest_data, q.quest_data)
    }
}

fn slice<T>(list: &[T], range: [i64; 2]) -> &[T] {
    let n = list.len();
    let start = (range[0].max(0) as usize).min(n);
    let end = start.saturating_add(range[1].max(0) as usize).min(n);
    list.get(start..end).unwrap_or(&[])
}

/// Parse un `game_quest_config_*.cfg.bin.json` complet.
#[must_use]
pub fn parse_game_quest_config(root: &Value) -> GameQuestConfig {
    let mut icons = Vec::new();
    if let Some(values) = list_values(root, "m_iconList") {
        for v in values {
            icons.push(field_hash(v, "paramId"));
        }
    }
    let mut quest_data = Vec::new();
    if let Some(values) = list_values(root, "m_questDataList") {
        for v in values {
            quest_data.push(QuestData::from_value(v));
        }
    }
    let mut game_infos = Vec::new();
    if let Some(values) = list_values(root, "m_gameInfoList") {
        for v in values {
            game_infos.push(field_i64(v, "difficultyType").unwrap_or(0));
        }
    }
    let mut game_quests = Vec::new();
    if let Some(values) = list_values(root, "m_gameQuestInfoList") {
        for v in values {
            if let Some(q) = GameQuestInfo::from_value(v) {
                game_quests.push(q);
            }
        }
    }
    GameQuestConfig {
        icons,
        quest_data,
        game_infos,
        game_quests,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({
            "lists": [
                { "name": "m_iconList", "values": [{ "paramId": "0xBCE81249" }] },
                { "name": "m_questDataList", "values": [
                    { "explainTextId": "0x223C1733", "condCount": 3, "questFlagNum": 1,
                      "openPhase": 1, "itemTableId": "0x00000000", "limit": "FFFFFFFF", "icon": [0, 0] }
                ]},
                { "name": "m_gameInfoList", "values": [{ "difficultyType": 1 }] },
                { "name": "m_gameQuestInfoList", "values": [
                    { "gameQuestId": "0xF1D6A24D", "titleTextId": "0x8CD2A357", "gameIdArray": [0, 1],
                      "questData": [0, 1], "gameQuestType": 1, "gameQuestFlagNum": 1,
                      "exGameSettingId": "0x00000000" }
                ]}
            ]
        })
    }

    #[test]
    fn parse_et_resolution() {
        let cfg = parse_game_quest_config(&fixture());
        assert_eq!(cfg.icons.len(), 1);
        assert_eq!(cfg.quest_data.len(), 1);
        assert_eq!(cfg.game_quests.len(), 1);
        let q = &cfg.quest_data[0];
        assert_eq!(q.explain_text_id, HashId(0x223C_1733));
        assert_eq!(q.cond_count, 3);
        assert_eq!(q.limit, 0xFFFF_FFFF); // "FFFFFFFF" hex
        let gq = &cfg.game_quests[0];
        assert_eq!(gq.game_quest_id, HashId(0xF1D6_A24D));
        assert_eq!(gq.quest_data, [0, 1]);
        // résolution objectifs.
        let objs = cfg.quests_of(gq);
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].cond_count, 3);
    }
}
