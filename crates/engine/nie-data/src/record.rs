//! `RecordConfig` — port de `record_config.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `/home/ubuntu/niers/data/common/gamedata/record/record_config.cfg.bin.json`
//! - Fichier de 30 232 octets, 31 entrées `RECORD_INFO_*` enfants d'un noeud
//!   `RECORD_INFO_LIST_BEG_0` (var\[0\]=31).
//!
//! ## Format
//!
//! Le fichier utilise le format **`entries`** (noeuds nommés, variables positionnelles),
//! identique à `command/*.cfg.bin.json` — pas le format `lists` de `formation_config`.
//!
//! Chaque noeud `RECORD_INFO_N` contient **9 variables positionnelles** :
//!
//! | Pos | Type JSON | Sémantique observée |
//! |-----|-----------|---------------------|
//! | 0 | Int | `record_id` — hash identifiant unique du record |
//! | 1 | Int | `ref_id` — hash de référence (catégorie ou cible) |
//! | 2 | Int | `flag1` — indicateur entier (0 ou 1) |
//! | 3 | Int | `group_id` — hash de groupe constant (438306617 = 0x1A200739 dans tous les dumps) |
//! | 4 | Int | `opt_id1` — hash optionnel, 0 si absent |
//! | 5 | Int | `opt_id2` — hash optionnel, 0 si absent |
//! | 6 | Int | `flag2` — indicateur entier (0 ou 1) |
//! | 7 | Int | `record_no` — numéro de record dans la séquence du jeu (non continu : 1–34 avec trous) |
//! | 8 | Int\|String | `extra_data` — données binaires encodées en base64, ou "0" si absentes |
//!
//! ## Observations sur les données
//!
//! - Le champ `group_id` (var\[3\]) vaut systématiquement 438306617 = **0x1A200739** dans les
//!   31 entrées — il s'agit probablement d'un hash de type de record ou d'un identifiant parent.
//! - `record_no` (var\[7\]) est non-continu : les index 1–34 sont présents mais 9, 23, 33 sont
//!   absents. Les records probablement supprimés au développement.
//! - `extra_data` (var\[8\]) est une chaîne base64 dans 11 entrées sur 31 (records avec données
//!   binaires associées, comme des cibles de progression encodées) ; les 20 autres contiennent "0".
//!
//! ## Utilisation prévue dans `nie-core`
//!
//! `RecordConfig` permettra à la logique de suivi de progression (victoires, buts, etc.)
//! d'identifier et de mettre à jour les records du joueur. **Le port dans la logique de jeu
//! est hors scope de ce module** — ce module expose uniquement les données de configuration.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

// ─── RecordInfo ───────────────────────────────────────────────────────────────

/// Entrée `RECORD_INFO_N` — définition d'un record de progression du joueur.
///
/// Les 9 variables sont positionnelles (format `entries` cfgbin, aucun nom officiel sans
/// la source TS inagle). Vérité terrain :
/// `record_config.cfg.bin.json`, `RECORD_INFO_0` :
/// `[-1957215473, -1229922355, 0, 438306617, 0, 0, 0, 1, 0]`
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RecordInfo {
    /// var\[0\] — hash identifiant unique du record (toujours non-nul dans le dump réel).
    ///
    /// Vérité terrain : `RECORD_INFO_0` var\[0\] = -1957215473 → `HashId::from_i64(-1957215473)`.
    pub record_id: HashId,

    /// var\[1\] — hash de référence (catégorie ou cible du record).
    ///
    /// Vérité terrain : `RECORD_INFO_0` var\[1\] = -1229922355 → `HashId::from_i64(-1229922355)`.
    pub ref_id: HashId,

    /// var\[2\] — indicateur booléen (0 ou 1).
    ///
    /// Vaut 1 dans `RECORD_INFO_2`, `RECORD_INFO_3` et quelques autres.
    pub flag1: i64,

    /// var\[3\] — hash de groupe constant (438306617 = 0x1A200739 dans les 31 entrées du dump réel).
    ///
    /// Représente probablement un type ou une catégorie de record au niveau du jeu.
    pub group_id: HashId,

    /// var\[4\] — hash optionnel (condition ou identifiant supplémentaire), `HashId::ZERO` si absent.
    pub opt_id1: HashId,

    /// var\[5\] — hash optionnel (récompense ou identifiant supplémentaire), `HashId::ZERO` si absent.
    ///
    /// Vérité terrain : `RECORD_INFO_7` var\[5\] = -1478521859.
    pub opt_id2: HashId,

    /// var\[6\] — indicateur booléen (0 ou 1).
    pub flag2: i64,

    /// var\[7\] — numéro du record dans la séquence du jeu (non-continu, 1–34 avec trous).
    ///
    /// Vérité terrain : `RECORD_INFO_0` var\[7\] = 1 ; `RECORD_INFO_7` var\[7\] = 8 ;
    /// `RECORD_INFO_30` var\[7\] = 34.
    pub record_no: i64,

    /// var\[8\] — données binaires encodées en base64, ou `"0"` si absentes.
    ///
    /// Vérité terrain : `RECORD_INFO_7` var\[8\] = `"AAAAAA8FNcMu49QAAQAyAAAAAXE="` (base64) ;
    /// `RECORD_INFO_0` var\[8\] = `"0"` (entier nul, pas de données).
    pub extra_data: String,
}

impl RecordInfo {
    /// Parse un noeud `RECORD_INFO_N`. Renvoie `None` si `record_id` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let record_id = node.hash(0);
        if record_id.is_zero() {
            return None;
        }
        Some(Self {
            record_id,
            ref_id: node.hash(1),
            flag1: node.int(2),
            group_id: node.hash(3),
            opt_id1: node.hash(4),
            opt_id2: node.hash(5),
            flag2: node.int(6),
            record_no: node.int(7),
            extra_data: String::from(node.string(8)),
        })
    }

    /// `true` si ce record possède des données binaires associées (var\[8\] n'est pas `"0"`).
    #[must_use]
    #[inline]
    pub fn has_extra_data(&self) -> bool {
        self.extra_data != "0"
    }
}

// ─── RecordConfig ─────────────────────────────────────────────────────────────

/// Contenu complet de `record_config.cfg.bin.json`.
///
/// Utiliser [`parse_record_config`] pour construire depuis un JSON désérialisé.
///
/// Le dump réel contient 31 entrées (record_no 1–34 avec trous à 9, 23 et 33).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RecordConfig {
    /// 31 définitions de records (`RECORD_INFO_*`).
    pub infos: Vec<RecordInfo>,
}

impl RecordConfig {
    /// Recherche un record par son `record_id`. `None` si absent.
    #[must_use]
    pub fn find_by_id(&self, record_id: HashId) -> Option<&RecordInfo> {
        self.infos.iter().find(|r| r.record_id == record_id)
    }

    /// Recherche un record par son `record_no`. `None` si absent.
    #[must_use]
    pub fn find_by_no(&self, record_no: i64) -> Option<&RecordInfo> {
        self.infos.iter().find(|r| r.record_no == record_no)
    }

    /// Renvoie tous les records qui possèdent des données binaires associées.
    #[must_use]
    pub fn with_extra_data(&self) -> Vec<&RecordInfo> {
        self.infos.iter().filter(|r| r.has_extra_data()).collect()
    }
}

// ─── Parseur principal ────────────────────────────────────────────────────────

/// Parse un `record_config.cfg.bin.json` (format `entries`, nœuds `RECORD_INFO_*`).
///
/// Les noeuds conteneurs (`RECORD_INFO_LIST_BEG_*`) sont ignorés.
/// Les entrées avec `record_id` nul sont silencieusement écartées.
///
/// Compte réel : `record_config.cfg.bin.json` → 31 entrées.
///
/// # Exemple
///
/// ```
/// use nie_data::record::parse_record_config;
/// use serde_json::json;
///
/// let root = json!({
///     "entries": [{
///         "name": "RECORD_INFO_LIST_BEG_0",
///         "variables": [{"type": "Int", "value": "1"}],
///         "children": [{
///             "name": "RECORD_INFO_0",
///             "variables": [
///                 {"type": "Int", "value": "-1957215473"},
///                 {"type": "Int", "value": "-1229922355"},
///                 {"type": "Int", "value": "0"},
///                 {"type": "Int", "value": "438306617"},
///                 {"type": "Int", "value": "0"},
///                 {"type": "Int", "value": "0"},
///                 {"type": "Int", "value": "0"},
///                 {"type": "Int", "value": "1"},
///                 {"type": "Int", "value": "0"}
///             ],
///             "children": []
///         }]
///     }]
/// });
/// let cfg = parse_record_config(&root);
/// assert_eq!(cfg.infos.len(), 1);
/// assert_eq!(cfg.infos[0].record_no, 1);
/// assert_eq!(cfg.infos[0].extra_data, "0");
/// ```
#[must_use]
pub fn parse_record_config(root: &Value) -> RecordConfig {
    let mut infos = Vec::new();
    walk_named(root, "RECORD_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(info) = RecordInfo::from_node(node) {
            infos.push(info);
        }
    });
    RecordConfig { infos }
}
