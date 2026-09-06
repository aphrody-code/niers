//! `SoccerDropConfig` — port Rust de `soccer/soccer_drop_config_*.cfg.bin` (Level-5 IEVR).
//!
//! Système complet de **butin de match** (drops de fin de rencontre) : tables d'items et
//! d'esprits/joueurs, poids de loterie, raretés, exceptions. Famille game-data **jusqu'ici
//! non couverte** par nie-data ; seul le couple `(dropRarity, rarity) -> weight` du tirage
//! d'esprit était reversé côté inagle (`loadSpiritDropRates`).
//!
//! ## Vérité terrain
//!
//! - Dumps réels (deux versions présentes sur le VPS) :
//!   - `data/common/gamedata/soccer/soccer_drop_config_5.00.27.00.cfg.bin.json`
//!   - `data/common/gamedata/soccer/soccer_drop_config_1.03.20.00.cfg.bin.json`
//! - Référence de portage : `packages/inagle/src/parsers/drop-rates.ts`
//!   (`loadSpiritDropRates`, l.123-154) — porté 1:1 par [`SoccerDropConfig::spirit_drop_rates`].
//! - Format **`lists`** (RDBN à listes plates). 12 listes (la 13ᵉ visible ci-dessous, file
//!   `version = 100`).
//!
//! ## Les 12 listes (comptes vérifiés sur le dump `5.00.27.00`)
//!
//! | Liste                          | type cfg.bin            | n  | Sémantique                                   |
//! |--------------------------------|-------------------------|----|----------------------------------------------|
//! | `m_rarityDecideTableList`      | `RARITY_DECIDE_TABLE`   | 6  | rareté → table décidée par palier (6 paliers) |
//! | `m_lotteryNumInfoList`         | `LOTTERY_NUM_INFO`      | 10 | id → `[minNum, maxNum]` (nb de drops tirés)   |
//! | `m_itemDropDataList`           | `ITEM_DROP_DATA`        | 94 | `(dropRarity) -> weight` (poids entier)        |
//! | `m_itemDropTableList`          | `ITEM_DROP_TABLE`       | 26 | id → tranche `[offset,count]` de itemDropData |
//! | `m_spiritDropDataList`         | `SPIRIT_DROP_DATA`      | 54 | `(dropRarity, rarity) -> weight` (poids float) |
//! | `m_spiritDropTableList`        | `SPIRIT_DROP_TABLE`     | 22 | id → tranche `[offset,count]` de spiritDropData |
//! | `m_spiritTableDataList`        | `SPIRIT_TABLE_DATA`     | 568| `(charaId) -> weight` + condition d'exécution  |
//! | `m_spiritCharaTableList`       | `SPIRIT_CHARA_TABLE`    | 34 | id → tranche `[offset,count]` de spiritTableData |
//! | `m_decideTableDataList`        | `DECIDE_TABLE_DATA`     | 15 | crc de palier de rareté → id de table d'items  |
//! | `m_itemTableDecideTableList`   | `ITEM_TABLE_DECIDE_TABLE`| 2 | id → tranche `[offset,count]` de decideTableData |
//! | `m_exceptionDropCharaList`     | `EXCEPTION_DROP_CHARA_LIST`| 113 | persos exclus du drop                      |
//! | `m_exceptionDropList`          | `EXCEPTION_DROP_LIST`   | 1  | id → tranche `[offset,count]` de exceptionDropChara |
//!
//! Anti-hallucination : chaque champ provient d'un octet réel du dump. La faute de frappe
//! Level-5 `rairtyId` (au lieu de `rarityId`) dans `RARITY_DECIDE_TABLE` est **préservée**
//! telle quelle (clé JSON réelle) — cf. [`RarityDecideTable::from_value`].

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_f64, field_hash, field_i64, field_pair, field_str, list_values};
use crate::hash::HashId;

// ─── RARITY_DECIDE_TABLE ────────────────────────────────────────────────────────

/// Entrée `RARITY_DECIDE_TABLE` — pour une rareté donnée, la table d'items décidée selon le
/// **palier de drop** (normal / growing / advanced / top / legendary / hero).
///
/// Vérité terrain : `m_rarityDecideTableList[0] = { rairtyId: "0xD17DAF62", normal: "0x81A08AD6",
/// growing: "0x81A08AD6", advanced: "0x81A08AD6", top: "0x81A08AD6", legendary: "0x18A9DB6C",
/// hero: "0x18A9DB6C" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RarityDecideTable {
    /// `rairtyId` — hash de la rareté (la clé JSON réelle porte la **typo Level-5** `rairtyId`).
    pub rarity_id: HashId,
    /// `normal` — table décidée au palier « normal ».
    pub normal: HashId,
    /// `growing` — table décidée au palier « growing ».
    pub growing: HashId,
    /// `advanced` — table décidée au palier « advanced ».
    pub advanced: HashId,
    /// `top` — table décidée au palier « top ».
    pub top: HashId,
    /// `legendary` — table décidée au palier « legendary ».
    pub legendary: HashId,
    /// `hero` — table décidée au palier « hero ».
    pub hero: HashId,
}

impl RarityDecideTable {
    /// Parse une entrée. `None` si `rairtyId` est nul/absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        // La clé réelle est `rairtyId` (faute de frappe Level-5 conservée byte-exact).
        let rarity_id = field_hash(v, "rairtyId");
        if rarity_id.is_zero() {
            return None;
        }
        Some(Self {
            rarity_id,
            normal: field_hash(v, "normal"),
            growing: field_hash(v, "growing"),
            advanced: field_hash(v, "advanced"),
            top: field_hash(v, "top"),
            legendary: field_hash(v, "legendary"),
            hero: field_hash(v, "hero"),
        })
    }
}

// ─── LOTTERY_NUM_INFO ───────────────────────────────────────────────────────────

/// Entrée `LOTTERY_NUM_INFO` — combien de drops sont tirés (intervalle `[minNum, maxNum]`).
///
/// Vérité terrain : `m_lotteryNumInfoList[0] = { id: "0x43E99C1C", minNum: 3, maxNum: 7 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LotteryNumInfo {
    /// `id` — hash identifiant l'info de loterie.
    pub id: HashId,
    /// `minNum` — nombre minimum de drops.
    pub min_num: i64,
    /// `maxNum` — nombre maximum de drops.
    pub max_num: i64,
}

impl LotteryNumInfo {
    /// Parse une entrée. `None` si `id` est nul/absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            min_num: field_i64(v, "minNum").unwrap_or(0),
            max_num: field_i64(v, "maxNum").unwrap_or(0),
        })
    }
}

// ─── ITEM_DROP_DATA ─────────────────────────────────────────────────────────────

/// Entrée `ITEM_DROP_DATA` — un poids de loterie d'item pour un palier de drop.
///
/// Vérité terrain : `m_itemDropDataList[0] = { dropRarity: 0, weight: 5 }`. Poids **entier**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ItemDropData {
    /// `dropRarity` — palier de drop (0..).
    pub drop_rarity: i64,
    /// `weight` — poids de loterie entier.
    pub weight: i64,
}

impl ItemDropData {
    /// Parse une entrée (toujours `Some` : pas de clé identifiante, position significative).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            drop_rarity: field_i64(v, "dropRarity").unwrap_or(0),
            weight: field_i64(v, "weight").unwrap_or(0),
        }
    }
}

// ─── SPIRIT_DROP_DATA ───────────────────────────────────────────────────────────

/// Entrée `SPIRIT_DROP_DATA` — poids de tirage d'esprit pour un couple `(dropRarity, rarity)`.
///
/// Vérité terrain : `m_spiritDropDataList[0] = { dropRarity: 0, rarity: 1, weight: 20.75 }`.
/// Poids **flottant** (0.05 .. 83.8 observés).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpiritDropData {
    /// `dropRarity` — palier de drop (0 / 10 / 20 / 21 observés).
    pub drop_rarity: i64,
    /// `rarity` — rareté de l'esprit tiré (1..=5).
    pub rarity: i64,
    /// `weight` — poids de loterie flottant.
    pub weight: f64,
}

impl SpiritDropData {
    /// Parse une entrée (toujours `Some` : position significative, pas de clé identifiante).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            drop_rarity: field_i64(v, "dropRarity").unwrap_or(0),
            rarity: field_i64(v, "rarity").unwrap_or(0),
            weight: field_f64(v, "weight").unwrap_or(0.0),
        }
    }
}

// ─── SPIRIT_TABLE_DATA ──────────────────────────────────────────────────────────

/// Entrée `SPIRIT_TABLE_DATA` — un perso/esprit droppable avec son poids et sa condition.
///
/// Vérité terrain : `m_spiritTableDataList[0] = { charaId: "0xFDCD0454", weight: 0, runCond: "" }`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpiritTableData {
    /// `charaId` — hash du personnage/esprit droppable.
    pub chara_id: HashId,
    /// `weight` — poids de tirage entier.
    pub weight: i64,
    /// `runCond` — condition d'exécution (chaîne opaque, souvent vide).
    pub run_cond: String,
}

impl SpiritTableData {
    /// Parse une entrée. `None` si `charaId` est nul/absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let chara_id = field_hash(v, "charaId");
        if chara_id.is_zero() {
            return None;
        }
        Some(Self {
            chara_id,
            weight: field_i64(v, "weight").unwrap_or(0),
            run_cond: field_str(v, "runCond").unwrap_or("").into(),
        })
    }
}

// ─── DECIDE_TABLE_DATA ──────────────────────────────────────────────────────────

/// Entrée `DECIDE_TABLE_DATA` — associe un crc de palier de rareté à une table d'items.
///
/// Vérité terrain : `m_decideTableDataList[0] = { dropRarityNameCrc: "0x7EFD704E",
/// itemTableId: "0xF0F3FEE3" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DecideTableData {
    /// `dropRarityNameCrc` — crc du nom de palier de rareté de drop.
    pub drop_rarity_name_crc: HashId,
    /// `itemTableId` — id de la table d'items décidée.
    pub item_table_id: HashId,
}

impl DecideTableData {
    /// Parse une entrée. `None` si `dropRarityNameCrc` est nul/absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let drop_rarity_name_crc = field_hash(v, "dropRarityNameCrc");
        if drop_rarity_name_crc.is_zero() {
            return None;
        }
        Some(Self {
            drop_rarity_name_crc,
            item_table_id: field_hash(v, "itemTableId"),
        })
    }
}

// ─── EXCEPTION_DROP_CHARA_LIST ──────────────────────────────────────────────────

/// Entrée `EXCEPTION_DROP_CHARA_LIST` — un perso **exclu** du tirage de drop.
///
/// Vérité terrain : `m_exceptionDropCharaList[0] = { exceptionDropCharaId: "0x835222D3" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExceptionDropChara {
    /// `exceptionDropCharaId` — hash du perso exclu.
    pub exception_drop_chara_id: HashId,
}

impl ExceptionDropChara {
    /// Parse une entrée. `None` si l'id est nul/absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let exception_drop_chara_id = field_hash(v, "exceptionDropCharaId");
        if exception_drop_chara_id.is_zero() {
            return None;
        }
        Some(Self {
            exception_drop_chara_id,
        })
    }
}

// ─── Tables à tranches `[offset, count]` ────────────────────────────────────────

/// Une table à id + tranche `[offset, count]` indexant une liste de données sœur.
///
/// Forme commune de `m_itemDropTableList`, `m_spiritDropTableList`, `m_spiritCharaTableList`,
/// `m_itemTableDecideTableList`, `m_exceptionDropList` (tous : `{ id|*Id, data: [offset, count] }`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SliceTable {
    /// `id` (ou `exceptionDropListId`) — hash identifiant la table.
    pub id: HashId,
    /// `data` — `[offset, count]` indexant la liste de données associée.
    pub data: [i64; 2],
}

impl SliceTable {
    /// Parse une table sous la clé d'id `id_key` (ex. `"id"`, `"exceptionDropListId"`).
    /// `None` si `data` n'est pas un tableau (port du `if (!Array.isArray) continue` inagle).
    #[must_use]
    pub fn from_value(v: &Value, id_key: &str) -> Option<Self> {
        let data = field_pair(v, "data")?;
        Some(Self {
            id: field_hash(v, id_key),
            data,
        })
    }

    /// Borne de fin réelle de la tranche, plafonnée à `len` (port `j < list.length` inagle).
    #[must_use]
    pub fn slice_end(&self, len: usize) -> usize {
        let start = self.data[0].max(0) as usize;
        let count = self.data[1].max(0) as usize;
        start.saturating_add(count).min(len)
    }

    /// Début réel de la tranche, plafonné à `len`.
    #[must_use]
    pub fn slice_start(&self, len: usize) -> usize {
        (self.data[0].max(0) as usize).min(len)
    }
}

// ─── SoccerDropConfig (agrégat) ─────────────────────────────────────────────────

/// Contenu complet d'un `soccer_drop_config_*.cfg.bin` (toutes les listes du système de butin).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerDropConfig {
    /// `m_rarityDecideTableList` — tables décidées par rareté × palier.
    pub rarity_decide_tables: Vec<RarityDecideTable>,
    /// `m_lotteryNumInfoList` — intervalles de nombre de drops.
    pub lottery_nums: Vec<LotteryNumInfo>,
    /// `m_itemDropDataList` — poids d'items (indexée par les tranches de `item_drop_tables`).
    pub item_drop_data: Vec<ItemDropData>,
    /// `m_itemDropTableList` — tables d'items (id → tranche de `item_drop_data`).
    pub item_drop_tables: Vec<SliceTable>,
    /// `m_spiritDropDataList` — poids d'esprits (indexée par les tranches de `spirit_drop_tables`).
    pub spirit_drop_data: Vec<SpiritDropData>,
    /// `m_spiritDropTableList` — tables d'esprits (id → tranche de `spirit_drop_data`).
    pub spirit_drop_tables: Vec<SliceTable>,
    /// `m_spiritTableDataList` — persos/esprits droppables (indexée par `spirit_chara_tables`).
    pub spirit_table_data: Vec<SpiritTableData>,
    /// `m_spiritCharaTableList` — tables de persos (id → tranche de `spirit_table_data`).
    pub spirit_chara_tables: Vec<SliceTable>,
    /// `m_decideTableDataList` — palier de rareté → table d'items.
    pub decide_table_data: Vec<DecideTableData>,
    /// `m_itemTableDecideTableList` — tables de décision (id → tranche de `decide_table_data`).
    pub item_table_decide_tables: Vec<SliceTable>,
    /// `m_exceptionDropCharaList` — persos exclus du drop.
    pub exception_drop_charas: Vec<ExceptionDropChara>,
    /// `m_exceptionDropList` — listes d'exception (id → tranche de `exception_drop_charas`).
    pub exception_drop_lists: Vec<SliceTable>,
}

/// Une ligne aplatie de **taux de drop d'esprit** : un couple `(dropRarity, rarity) -> weight`
/// rattaché à sa table propriétaire. Port 1:1 de `DropRateRow` inagle (source `spirit_drop`).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpiritDropRate {
    /// Index positionnel dans `m_spiritDropDataList` (clé stable, = suffixe `spirit_drop:<idx>`).
    pub ordinal: usize,
    /// Table d'esprit propriétaire (résolue via `m_spiritDropTableList`) ; `None` = non rattachée
    /// (sentinelle `"spirit:unbound"` chez inagle).
    pub source_id: Option<HashId>,
    /// Palier de drop (`dropRarity`).
    pub drop_rarity: i64,
    /// Rareté de l'esprit (`rarity`).
    pub rarity: i64,
    /// Poids de loterie flottant (`weight`).
    pub weight: f64,
}

impl SoccerDropConfig {
    /// Aplatit `m_spiritDropDataList` en lignes de taux, chaque ligne rattachée à sa table
    /// propriétaire via `m_spiritDropTableList.data = [offset, count]`.
    ///
    /// Port **1:1** de `loadSpiritDropRates` (inagle `drop-rates.ts` l.123-154) : chaque ligne
    /// de `spirit_drop_data` est référencée **exactement une fois** par une table d'esprit
    /// (vérifié sur le dump réel) ; à défaut, `source_id = None`.
    #[must_use]
    pub fn spirit_drop_rates(&self) -> Vec<SpiritDropRate> {
        // ownerById[idx] = id de la table propriétaire (dernière gagnante en cas de chevauchement,
        // comme la `Map.set` d'inagle).
        let n = self.spirit_drop_data.len();
        let mut owner: Vec<Option<HashId>> = alloc::vec![None; n];
        for t in &self.spirit_drop_tables {
            let start = t.slice_start(n);
            let end = t.slice_end(n);
            for slot in &mut owner[start..end] {
                *slot = Some(t.id);
            }
        }
        self.spirit_drop_data
            .iter()
            .enumerate()
            .map(|(ordinal, s)| SpiritDropRate {
                ordinal,
                source_id: owner.get(ordinal).copied().flatten(),
                drop_rarity: s.drop_rarity,
                rarity: s.rarity,
                weight: s.weight,
            })
            .collect()
    }

    /// Résout la tranche d'items d'une table de `item_drop_tables` par son `id`.
    /// `None` si la table est absente.
    #[must_use]
    pub fn item_drop_slice(&self, id: HashId) -> Option<&[ItemDropData]> {
        let t = self.item_drop_tables.iter().find(|t| t.id == id)?;
        let n = self.item_drop_data.len();
        self.item_drop_data.get(t.slice_start(n)..t.slice_end(n))
    }
}

// ─── Parseur principal ──────────────────────────────────────────────────────────

/// Parse un `soccer_drop_config_*.cfg.bin.json` complet (les 12 listes du système de butin).
///
/// Les listes absentes deviennent des `Vec` vides (le dump `1.03.20.00` n'a pas de listes
/// d'exception, p. ex.). Les entrées invalides (id nul) sont silencieusement ignorées.
#[must_use]
pub fn parse_soccer_drop_config(root: &Value) -> SoccerDropConfig {
    SoccerDropConfig {
        rarity_decide_tables: parse_list(
            root,
            "m_rarityDecideTableList",
            RarityDecideTable::from_value,
        ),
        lottery_nums: parse_list(root, "m_lotteryNumInfoList", LotteryNumInfo::from_value),
        item_drop_data: parse_list_infallible(root, "m_itemDropDataList", ItemDropData::from_value),
        item_drop_tables: parse_slice_tables(root, "m_itemDropTableList", "id"),
        spirit_drop_data: parse_list_infallible(
            root,
            "m_spiritDropDataList",
            SpiritDropData::from_value,
        ),
        spirit_drop_tables: parse_slice_tables(root, "m_spiritDropTableList", "id"),
        spirit_table_data: parse_list(root, "m_spiritTableDataList", SpiritTableData::from_value),
        spirit_chara_tables: parse_slice_tables(root, "m_spiritCharaTableList", "id"),
        decide_table_data: parse_list(root, "m_decideTableDataList", DecideTableData::from_value),
        item_table_decide_tables: parse_slice_tables(root, "m_itemTableDecideTableList", "id"),
        exception_drop_charas: parse_list(
            root,
            "m_exceptionDropCharaList",
            ExceptionDropChara::from_value,
        ),
        exception_drop_lists: parse_slice_tables(
            root,
            "m_exceptionDropList",
            "exceptionDropListId",
        ),
    }
}

/// Parse une liste dont les entrées sont faillibles (`from_value -> Option`), filtre les `None`.
fn parse_list<T>(root: &Value, name: &str, f: impl Fn(&Value) -> Option<T>) -> Vec<T> {
    let mut out = Vec::new();
    if let Some(values) = list_values(root, name) {
        for v in values {
            if let Some(item) = f(v) {
                out.push(item);
            }
        }
    }
    out
}

/// Parse une liste dont les entrées sont infaillibles (`from_value -> T`, position significative).
fn parse_list_infallible<T>(root: &Value, name: &str, f: impl Fn(&Value) -> T) -> Vec<T> {
    let mut out = Vec::new();
    if let Some(values) = list_values(root, name) {
        for v in values {
            out.push(f(v));
        }
    }
    out
}

/// Parse une liste de [`SliceTable`] sous la clé d'id donnée (filtre les `data` non-tableau).
fn parse_slice_tables(root: &Value, name: &str, id_key: &str) -> Vec<SliceTable> {
    let mut out = Vec::new();
    if let Some(values) = list_values(root, name) {
        for v in values {
            if let Some(t) = SliceTable::from_value(v, id_key) {
                out.push(t);
            }
        }
    }
    out
}

// ─── Tests unitaires (fixtures embarquées = valeurs réelles du dump) ─────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Fixture minimale reproduisant la structure réelle (valeurs copiées du dump `5.00.27.00`).
    fn fixture() -> Value {
        json!({
            "version": 100,
            "lists": [
                { "name": "m_rarityDecideTableList", "values": [
                    { "rairtyId": "0xD17DAF62", "normal": "0x81A08AD6", "growing": "0x81A08AD6",
                      "advanced": "0x81A08AD6", "top": "0x81A08AD6", "legendary": "0x18A9DB6C",
                      "hero": "0x18A9DB6C" }
                ]},
                { "name": "m_itemDropDataList", "values": [
                    { "dropRarity": 0, "weight": 5 },
                    { "dropRarity": 1, "weight": 5 }
                ]},
                { "name": "m_itemDropTableList", "values": [
                    { "id": "0x9CCAB1C6", "data": [0, 2] }
                ]},
                { "name": "m_spiritDropDataList", "values": [
                    { "dropRarity": 0, "rarity": 1, "weight": 20.75 },
                    { "dropRarity": 10, "rarity": 2, "weight": 4.25 },
                    { "dropRarity": 0, "rarity": 2, "weight": 20 }
                ]},
                { "name": "m_spiritDropTableList", "values": [
                    { "id": "0x26F1316F", "data": [0, 0] },
                    { "id": "0xBFF860D5", "data": [0, 2] },
                    { "id": "0xC8FF5043", "data": [2, 3] }
                ]}
            ]
        })
    }

    #[test]
    fn rarity_decide_table_typo_rairty_id() {
        let cfg = parse_soccer_drop_config(&fixture());
        assert_eq!(cfg.rarity_decide_tables.len(), 1);
        let r = &cfg.rarity_decide_tables[0];
        // La clé JSON réelle est `rairtyId` (typo Level-5) — doit être lue correctement.
        assert_eq!(r.rarity_id, HashId(0xD17D_AF62));
        assert_eq!(r.normal, HashId(0x81A0_8AD6));
        assert_eq!(r.legendary, HashId(0x18A9_DB6C));
        assert_eq!(r.hero, HashId(0x18A9_DB6C));
    }

    #[test]
    fn item_drop_data_infallible_positions() {
        let cfg = parse_soccer_drop_config(&fixture());
        assert_eq!(cfg.item_drop_data.len(), 2);
        assert_eq!(
            cfg.item_drop_data[0],
            ItemDropData {
                drop_rarity: 0,
                weight: 5
            }
        );
        assert_eq!(
            cfg.item_drop_data[1],
            ItemDropData {
                drop_rarity: 1,
                weight: 5
            }
        );
    }

    #[test]
    fn item_drop_slice_resolution() {
        let cfg = parse_soccer_drop_config(&fixture());
        let slice = cfg
            .item_drop_slice(HashId(0x9CCA_B1C6))
            .expect("table présente");
        assert_eq!(slice.len(), 2);
        assert_eq!(slice[1].weight, 5);
        assert!(cfg.item_drop_slice(HashId(0xDEAD_BEEF)).is_none());
    }

    #[test]
    fn spirit_drop_weight_float_byte_exact() {
        let cfg = parse_soccer_drop_config(&fixture());
        assert_eq!(cfg.spirit_drop_data.len(), 3);
        // Poids flottant : 20.75 et 4.25 sont exacts en f64 (vérité terrain).
        assert_eq!(
            cfg.spirit_drop_data[0].weight.to_bits(),
            20.75_f64.to_bits()
        );
        assert_eq!(cfg.spirit_drop_data[1].weight.to_bits(), 4.25_f64.to_bits());
        // weight: 20 (entier dans le JSON) lu en f64 = 20.0.
        assert_eq!(cfg.spirit_drop_data[2].weight.to_bits(), 20.0_f64.to_bits());
    }

    #[test]
    fn spirit_drop_rates_owner_join() {
        // Port de loadSpiritDropRates : table[0] data[0,0] n'owne rien ; table[1] data[0,2]
        // owne idx 0,1 ; table[2] data[2,3] owne idx 2 (et 3,4 hors fixture).
        let cfg = parse_soccer_drop_config(&fixture());
        let rates = cfg.spirit_drop_rates();
        assert_eq!(rates.len(), 3);
        assert_eq!(rates[0].ordinal, 0);
        assert_eq!(rates[0].source_id, Some(HashId(0xBFF8_60D5)));
        assert_eq!(rates[0].drop_rarity, 0);
        assert_eq!(rates[0].rarity, 1);
        assert_eq!(rates[0].weight.to_bits(), 20.75_f64.to_bits());
        assert_eq!(rates[1].source_id, Some(HashId(0xBFF8_60D5)));
        assert_eq!(rates[2].source_id, Some(HashId(0xC8FF_5043)));
    }

    #[test]
    fn listes_absentes_donnent_vecs_vides() {
        // Dump partiel (sans exception lists, comme `1.03.20.00`) → Vec vides, pas de panique.
        let cfg = parse_soccer_drop_config(&json!({ "version": 100, "lists": [] }));
        assert!(cfg.exception_drop_charas.is_empty());
        assert!(cfg.exception_drop_lists.is_empty());
        assert!(cfg.spirit_drop_rates().is_empty());
    }
}
