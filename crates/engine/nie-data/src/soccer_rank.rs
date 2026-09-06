//! `soccer_rank` — port de `soccer/soccer_rank_config_*.cfg.bin` (Level-5 IEVR).
//!
//! Système de **classement de match** (rangs, taux par rang, prix). Famille non couverte ; port
//! direct. Vérité terrain : `soccer_rank_config_*.cfg.bin.json` — `m_SoccerRankInfoList` (4) /
//! `m_SoccerRankRateList` (4) / `m_SoccerPrizePriceList` (3).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, list_values};
use crate::hash::HashId;

/// `m_SoccerRankInfoList` — un rang (seuil de points pour le suivant).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RankInfo {
    /// `id` — hash du rang.
    pub id: HashId,
    /// `soccerRankType` — type/niveau du rang.
    pub rank_type: i64,
    /// `nameTextId` — hash du libellé (`0` = aucun).
    pub name_text_id: HashId,
    /// `nextRankPoint` — points requis pour le rang suivant.
    pub next_rank_point: i64,
}

/// `m_SoccerRankRateList` — taux associé à un type de rang.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RankRate {
    /// `soccerRankType`.
    pub rank_type: i64,
    /// `rankRate`.
    pub rate: i64,
}

/// `m_SoccerPrizePriceList` — prix par type de récompense.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrizePrice {
    /// `soccerPrizeType`.
    pub prize_type: i64,
    /// `prizePrice`.
    pub price: i64,
}

/// Contenu d'un `soccer_rank_config_*.cfg.bin`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerRankConfig {
    /// `m_SoccerRankInfoList` — rangs.
    pub ranks: Vec<RankInfo>,
    /// `m_SoccerRankRateList` — taux par rang.
    pub rates: Vec<RankRate>,
    /// `m_SoccerPrizePriceList` — prix.
    pub prizes: Vec<PrizePrice>,
}

/// Parse un `soccer_rank_config_*.cfg.bin.json`.
#[must_use]
pub fn parse_soccer_rank_config(root: &Value) -> SoccerRankConfig {
    let mut ranks = Vec::new();
    if let Some(values) = list_values(root, "m_SoccerRankInfoList") {
        for v in values {
            let id = field_hash(v, "id");
            if !id.is_zero() {
                ranks.push(RankInfo {
                    id,
                    rank_type: field_i64(v, "soccerRankType").unwrap_or(0),
                    name_text_id: field_hash(v, "nameTextId"),
                    next_rank_point: field_i64(v, "nextRankPoint").unwrap_or(0),
                });
            }
        }
    }
    let mut rates = Vec::new();
    if let Some(values) = list_values(root, "m_SoccerRankRateList") {
        for v in values {
            rates.push(RankRate {
                rank_type: field_i64(v, "soccerRankType").unwrap_or(0),
                rate: field_i64(v, "rankRate").unwrap_or(0),
            });
        }
    }
    let mut prizes = Vec::new();
    if let Some(values) = list_values(root, "m_SoccerPrizePriceList") {
        for v in values {
            prizes.push(PrizePrice {
                prize_type: field_i64(v, "soccerPrizeType").unwrap_or(0),
                price: field_i64(v, "prizePrice").unwrap_or(0),
            });
        }
    }
    SoccerRankConfig {
        ranks,
        rates,
        prizes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_rangs() {
        let root = json!({
            "lists": [
                { "name": "m_SoccerRankInfoList", "values": [
                    { "id": "0x24F00406", "soccerRankType": 1, "nameTextId": "0x00000000", "nextRankPoint": 400 }
                ]},
                { "name": "m_SoccerRankRateList", "values": [{ "soccerRankType": 1, "rankRate": 1 }] },
                { "name": "m_SoccerPrizePriceList", "values": [{ "soccerPrizeType": 0, "prizePrice": 3 }] }
            ]
        });
        let cfg = parse_soccer_rank_config(&root);
        assert_eq!(cfg.ranks.len(), 1);
        assert_eq!(cfg.ranks[0].id, HashId(0x24F0_0406));
        assert_eq!(cfg.ranks[0].next_rank_point, 400);
        assert_eq!(cfg.rates[0].rate, 1);
        assert_eq!(cfg.prizes[0].price, 3);
    }
}
