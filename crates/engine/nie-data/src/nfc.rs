//! `NfcLottery` — port de `nfc/nfc_lottery_config.cfg.bin` (système de loterie NFC de IEVR).
//!
//! ## Vérité terrain
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/nfc-lottery-config.ts`
//!   (`parseEntries` l.55-103, `buildNfcLotteryDatabase` l.116-124).
//! - Dump réel : `data/common/gamedata/nfc/nfc_lottery_config.cfg.bin` (T2B, 85568 octets),
//!   extrait du VFS Steam via `nie-formats` (`cfgbin_to_t2b_iecode_root`).
//!
//! ## Structure (format `entries`, arbre à imbrication régulière 4 niveaux)
//!
//! Le fichier encode l'arbre à la façon Level-5 « BEG » : un nœud `*_LIST_BEG` contient
//! une **liste plate** où chaque élément (`NFC_LOTTERY_INFO_N`) est **immédiatement suivi**
//! de son sous-conteneur (`NFC_LOTTERY_INFO_TABLE_LIST_BEG_N`). On réassemble cette structure
//! plate en hiérarchie logique, exactement comme la sortie d'inagle :
//!
//! ```text
//! NFC_LOTTERY_INFO_LIST_BEG          (3 loteries)
//!  ├─ NFC_LOTTERY_INFO_0             vars[0] = lotteryId (Int)
//!  ├─ NFC_LOTTERY_INFO_TABLE_LIST_BEG_0   (34 tables)
//!  │   ├─ NFC_LOTTERY_INFO_TABLE_0   vars[0] = tableId (CRC -> hex)
//!  │   ├─ NFC_LOTTERY_INFO_TABLE_ITEM_LIST_BEG_0   (15 items)
//!  │   │   ├─ NFC_LOTTERY_INFO_TABLE_ITEM_0  vars = [itemId(CRC), type, weight]
//!  │   │   └─ … (15 items)
//!  │   └─ … (34 tables × 2 nœuds)
//!  ├─ NFC_LOTTERY_INFO_1             lotteryId = 2
//!  └─ …
//! ```
//!
//! ## Valeurs réelles observées (dump du 2026-06, extrait par `extract_nfc`)
//!
//! - 3 loteries : `lotteryId` = 1, 2, 3.
//! - Loterie 1 : 34 tables. `TABLE_0` tableId = `0xCBA1009F`, 15 items dont
//!   `(0x2E453C01, type=0, weight=32)`, `(0x05686FC2, 0, 8)`, …, `(0x883237B5, 0, 1)`.
//! - Loterie 2 : 1 table. `TABLE_0` tableId = `0x3319A8FD`, 1 item `(0xFC434F28, 0, 100)`.
//! - Loterie 3 : 34 tables. `TABLE_0` tableId = `0xCBA1009F`, 5 items
//!   `(0x3480755F, 0, 20)`, `(0x2D9B441E, 0, 10)`, …, `(0x883237B5, 0, 40)`.
//!
//! ## Sémantique (port 1:1 d'inagle)
//!
//! - `lotteryId` reste un entier décimal brut (inagle : `parseInt`, **pas** de `toHex`).
//! - `tableId` et `itemId` sont des hash 32 bits (inagle : `toHex(parseInt(...))`).
//! - Un item invalide (< 3 variables) est sauté, comme inagle (`if (vars.length < 3) continue`).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::Node;
use crate::hash::HashId;

/// Vrai si `name` vaut `<prefix><chiffres>` (préfixe avec underscore final inclus).
///
/// Permet de distinguer `NFC_LOTTERY_INFO_0` (match) de `NFC_LOTTERY_INFO_TABLE_LIST_BEG_0`
/// (pas de match), reproduisant la regex `^NFC_LOTTERY_INFO_\d+$` d'inagle.
fn is_indexed(name: &str, prefix: &str) -> bool {
    match name.strip_prefix(prefix) {
        Some(rest) => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

/// Un item d'une table de loterie NFC (`NFC_LOTTERY_INFO_TABLE_ITEM_*`).
///
/// Port d'`NfcLotteryItem` (`nfc-lottery-config.ts` l.26-30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NfcLotteryItem {
    /// vars\[0\] — `itemId` : hash CRC de la récompense (inagle : `toHex`).
    pub item_id: HashId,
    /// vars\[1\] — `type` (renommé `item_type`, `type` étant réservé en Rust).
    pub item_type: i64,
    /// vars\[2\] — `weight` : poids de tirage de l'item dans la table.
    pub weight: i64,
}

/// Une table de récompenses d'une loterie (`NFC_LOTTERY_INFO_TABLE_*`).
///
/// Port d'`NfcLotteryTable` (`nfc-lottery-config.ts` l.32-35).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NfcLotteryTable {
    /// vars\[0\] — `tableId` : hash CRC de la table (inagle : `toHex`).
    pub table_id: HashId,
    /// Items de la table (`NFC_LOTTERY_INFO_TABLE_ITEM_*`).
    pub items: Vec<NfcLotteryItem>,
}

impl NfcLotteryTable {
    /// Somme des poids de tirage de tous les items (dénominateur des probabilités).
    ///
    /// Pour `TABLE_0` de la loterie 1 (15 items : 32+8+32+7 + 2×10 + 1) = 100.
    #[must_use]
    pub fn total_weight(&self) -> i64 {
        self.items.iter().map(|i| i.weight).sum()
    }
}

/// Une loterie NFC (`NFC_LOTTERY_INFO_*`) et ses tables.
///
/// Port d'`NfcLottery` (`nfc-lottery-config.ts` l.37-40).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NfcLottery {
    /// vars\[0\] — `lotteryId` : entier décimal brut (inagle : pas de `toHex`).
    pub lottery_id: i64,
    /// Tables de la loterie (`NFC_LOTTERY_INFO_TABLE_*`).
    pub tables: Vec<NfcLotteryTable>,
}

impl NfcLottery {
    /// Recherche une table par son `table_id`. `None` si absente.
    #[must_use]
    pub fn find_table(&self, table_id: HashId) -> Option<&NfcLotteryTable> {
        self.tables.iter().find(|t| t.table_id == table_id)
    }
}

/// Base de loteries NFC complète d'un `nfc_lottery_config.cfg.bin`.
///
/// Port d'`NfcLotteryDatabase` (`nfc-lottery-config.ts` l.42-45) : la map `byLotteryId`
/// est remplacée par [`NfcLotteryConfig::find_lottery`] (recherche linéaire, `no_std`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NfcLotteryConfig {
    /// Toutes les loteries du fichier, dans l'ordre du dump.
    pub lotteries: Vec<NfcLottery>,
}

impl NfcLotteryConfig {
    /// Parse un `nfc_lottery_config.cfg.bin.json` désérialisé (forme iecode `entries`).
    #[must_use]
    pub fn from_root(root: &Value) -> Self {
        Self {
            lotteries: parse_nfc_lottery_config(root),
        }
    }

    /// Recherche une loterie par son `lottery_id`. `None` si absente.
    #[must_use]
    pub fn find_lottery(&self, lottery_id: i64) -> Option<&NfcLottery> {
        self.lotteries.iter().find(|l| l.lottery_id == lottery_id)
    }
}

/// Parse les items d'un conteneur `NFC_LOTTERY_INFO_TABLE_ITEM_LIST_BEG_*`.
///
/// Port d'inagle l.84-94 : pour chaque `NFC_LOTTERY_INFO_TABLE_ITEM_*` ayant ≥ 3
/// variables, lit `[itemId, type, weight]`. Les nœuds < 3 vars sont sautés.
fn parse_items(item_list: Node<'_>) -> Vec<NfcLotteryItem> {
    let mut items = Vec::new();
    for item in item_list.children() {
        if !is_indexed(item.name(), "NFC_LOTTERY_INFO_TABLE_ITEM_") {
            continue;
        }
        if item.var_count() < 3 {
            continue;
        }
        items.push(NfcLotteryItem {
            item_id: item.hash(0),
            item_type: item.int(1),
            weight: item.int(2),
        });
    }
    items
}

/// Parse les tables d'un conteneur `NFC_LOTTERY_INFO_TABLE_LIST_BEG_*`.
///
/// Port d'inagle l.72-97. Gère les DEUX encodages cfg.bin.json : le `_ITEM_LIST_BEG_N` est soit
/// un **enfant** du `NFC_LOTTERY_INFO_TABLE_N` (dumps imbriqués actuels), soit son **frère suivant**
/// (ancien encodage BEG plat / fixtures hermétiques).
fn parse_tables(table_list: Node<'_>) -> Vec<NfcLotteryTable> {
    let kids = table_list.children();
    let mut tables = Vec::new();
    let mut idx = 0;
    while idx < kids.len() {
        let node = kids[idx];
        if is_indexed(node.name(), "NFC_LOTTERY_INFO_TABLE_") {
            let table_id = node.hash(0);
            // Le conteneur d'items est soit un ENFANT du `NFC_LOTTERY_INFO_TABLE_N` (dump
            // imbriqué actuel), soit le FRÈRE suivant (encodage BEG plat). Enfant d'abord.
            let items = if let Some(il) = node
                .children()
                .into_iter()
                .find(|c| c.name().starts_with("NFC_LOTTERY_INFO_TABLE_ITEM_LIST_BEG"))
            {
                parse_items(il)
            } else if let Some(next) = kids
                .get(idx + 1)
                .filter(|n| n.name().starts_with("NFC_LOTTERY_INFO_TABLE_ITEM_LIST_BEG"))
            {
                let parsed = parse_items(*next);
                idx += 1; // consomme le conteneur d'items (frère, encodage plat)
                parsed
            } else {
                Vec::new()
            };
            tables.push(NfcLotteryTable { table_id, items });
        }
        idx += 1;
    }
    tables
}

/// Parse un `nfc_lottery_config.cfg.bin.json` désérialisé en liste de [`NfcLottery`].
///
/// Port 1:1 de `parseEntries` (`nfc-lottery-config.ts` l.55-103). Gère les DEUX encodages
/// cfg.bin.json : sous `NFC_LOTTERY_INFO_LIST_BEG`, le `_TABLE_LIST_BEG_N` de chaque
/// `NFC_LOTTERY_INFO_N` est soit son **enfant** (dumps imbriqués actuels — fichier live du VPS),
/// soit son **frère suivant** (ancien encodage BEG plat / fixtures hermétiques). Validé sur le
/// vrai `nfc_lottery_config.cfg.bin.json` (3 loteries, 34/1/34 tables = 69) ET les fixtures.
#[must_use]
pub fn parse_nfc_lottery_config(root: &Value) -> Vec<NfcLottery> {
    let mut lotteries = Vec::new();
    let Some(entries) = root.get("entries").and_then(Value::as_array) else {
        return lotteries;
    };
    for entry in entries {
        let list = Node::new(entry);
        if !list.name().starts_with("NFC_LOTTERY_INFO_LIST_BEG") {
            continue;
        }
        let kids = list.children();
        let mut idx = 0;
        while idx < kids.len() {
            let node = kids[idx];
            if is_indexed(node.name(), "NFC_LOTTERY_INFO_") {
                let lottery_id = node.int(0);
                // La liste des tables est soit un ENFANT du `NFC_LOTTERY_INFO_N` (dump imbriqué
                // actuel), soit le FRÈRE suivant (encodage BEG plat des anciens dumps/fixtures).
                // On tente l'enfant d'abord, puis le frère — robuste aux DEUX encodages.
                let tables = if let Some(tl) = node
                    .children()
                    .into_iter()
                    .find(|c| c.name().starts_with("NFC_LOTTERY_INFO_TABLE_LIST_BEG"))
                {
                    parse_tables(tl)
                } else if let Some(next) = kids
                    .get(idx + 1)
                    .filter(|n| n.name().starts_with("NFC_LOTTERY_INFO_TABLE_LIST_BEG"))
                {
                    let parsed = parse_tables(*next);
                    idx += 1; // consomme la liste des tables (frère, encodage plat)
                    parsed
                } else {
                    Vec::new()
                };
                lotteries.push(NfcLottery { lottery_id, tables });
            }
            idx += 1;
        }
    }
    lotteries
}
