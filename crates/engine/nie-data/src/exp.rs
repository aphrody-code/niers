//! `CharaExpTable` — port de `chara_exp_table_config` (XP/niveau + multiplicateurs de rareté).
//!
//! ## Vérité terrain
//!
//! - Parser TS : `packages/inagle/src/parsers/chara-exp-table.ts` (`CharaExpEntry` l.24-29,
//!   `ExpRarityRate` l.32-35, `parseContent` l.53-70, `getCumulativeExp` l.109-116).
//! - Données : `/home/ubuntu/niers/data/common/gamedata/character/chara_exp_table_config_0.00.00.00.cfg.bin.json`
//!   — `m_charaExpTableList` = 100 entrées (niveaux 1..100), `m_expRarityRateList` = 9 entrées.
//!
//! Échantillon vérifié : level 1 → needExp 124, level 2 → 130, level 3 → 146 ;
//! rarityRates = {0:1, 1:1, 2:1, 3:1, 4:3, 5:3, 6:3, 7:3, 8:3} (growthRank 0-3 → ×1, 4-8 → ×3).
//! Cumul vérifié : jusqu'au niveau 5 = 565, jusqu'au niveau 10 = 1973.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::list_values;

/// XP requise pour atteindre un niveau. Source : `CharaExpEntry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaExpEntry {
    /// Niveau (1..=100).
    pub level: i64,
    /// XP nécessaire pour atteindre ce niveau (depuis le précédent).
    pub need_exp: i64,
}

/// Multiplicateur XP par `growthRank`. Source : `ExpRarityRate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExpRarityRate {
    /// Rang de rareté (0..=8).
    pub rarity: i64,
    /// Multiplicateur appliqué à l'XP.
    pub rate: i64,
}

/// Table d'XP complète + index pour les helpers purs.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaExpTable {
    pub exp_table: Vec<CharaExpEntry>,
    pub rarity_rates: Vec<ExpRarityRate>,
}

impl CharaExpTable {
    /// XP nécessaire pour un niveau précis. `None` si absent. Source : `getExpForLevel`.
    #[must_use]
    pub fn exp_for_level(&self, level: i64) -> Option<i64> {
        self.exp_table
            .iter()
            .find(|e| e.level == level)
            .map(|e| e.need_exp)
    }

    /// XP cumulée pour atteindre `target_level` depuis le niveau 1 (somme des `needExp` des
    /// entrées dont `level < target_level`). Source : `getCumulativeExp` (l.109-116).
    #[must_use]
    pub fn cumulative_exp(&self, target_level: i64) -> i64 {
        let mut total = 0;
        for e in &self.exp_table {
            if e.level >= target_level {
                break;
            }
            total += e.need_exp;
        }
        total
    }

    /// Multiplicateur XP pour un `growthRank` (0-8). Défaut 1 si absent. Source : `getRarityRate`.
    #[must_use]
    pub fn rarity_rate(&self, growth_rank: i64) -> i64 {
        self.rarity_rates
            .iter()
            .find(|r| r.rarity == growth_rank)
            .map_or(1, |r| r.rate)
    }

    /// Index niveau→needExp (commodité).
    #[must_use]
    pub fn by_level(&self) -> BTreeMap<i64, i64> {
        self.exp_table
            .iter()
            .map(|e| (e.level, e.need_exp))
            .collect()
    }
}

/// Parse un `chara_exp_table_config_*.cfg.bin.json` désérialisé. Source : `parseContent` (l.53-70).
#[must_use]
pub fn parse_exp_table(root: &Value) -> CharaExpTable {
    let mut t = CharaExpTable::default();

    if let Some(values) = list_values(root, "m_charaExpTableList") {
        for v in values {
            t.exp_table.push(CharaExpEntry {
                level: v.get("level").and_then(Value::as_i64).unwrap_or(0),
                need_exp: v.get("needExp").and_then(Value::as_i64).unwrap_or(0),
            });
        }
    }
    if let Some(values) = list_values(root, "m_expRarityRateList") {
        for v in values {
            t.rarity_rates.push(ExpRarityRate {
                rarity: v.get("rarity").and_then(Value::as_i64).unwrap_or(0),
                rate: v.get("rate").and_then(Value::as_i64).unwrap_or(1),
            });
        }
    }

    t
}
