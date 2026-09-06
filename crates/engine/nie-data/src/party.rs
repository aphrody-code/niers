//! `PartyConfig` — port Rust des familles `party/*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! Quatre fichiers dans `data/common/gamedata/party/` :
//!
//! | Fichier | Format | Taille | Entrées |
//! |---------|--------|--------|---------|
//! | `ctrl_chara_config_1.04.17.00.cfg.bin.json` | `entries` | 86 Ko | 155 `CTRL_CHR_DATA_*` |
//! | `party_departure_0.00.00.cfg.bin.json` | `lists` | 827 oct. | 7 `PartyDeparture` |
//! | `supecify_party0.00.00.cfg.bin.json` | `lists` | 5 Ko | 18 `SPECIFY_PARTY_CHARA_DATA` + 7 `SPECIFY_PARTY_DATA` |
//! | `guest_limit_config.cfg.bin.json` | `entries` (niché) | 25 Ko | 9 `GENERAL_LIMIT_INFO_*` |
//!
//! ## Formats détectés
//!
//! - `ctrl_chara_config` et `guest_limit_config` utilisent le format **`entries`** (noeuds CfgBin
//!   positionnels, comme `command/*.cfg.bin.json`).
//! - `party_departure` et `supecify_party` utilisent le format **`lists`** (champs nommés,
//!   comme `formation_config*.cfg.bin.json`).
//!
//! ## Structure `CTRL_CHR_DATA` (3 vars)
//!
//! Exemple réel — `ctrl_chara_config_1.04.17.00.cfg.bin.json`, `CTRL_CHR_DATA_0` (ligne 13) :
//! `[1751902334, 1, 0]`
//! - var\[0\] = 1751902334 (ligne 17) → `chara_id` = 0x686BE87E.
//! - var\[1\] = 1 (ligne 21) → `ctrl_type` = 1.
//! - var\[2\] = 0 (ligne 25) → `flag` = 0 (toujours 0 sur les 155 entrées réelles).
//!
//! Distribution `ctrl_type` réelle : 0 = 22 entrées, 1 = 26 entrées, 2 = 107 entrées.
//!
//! ## Structure `PartyDeparture` (2 champs)
//!
//! Exemple réel — `party_departure_0.00.00.cfg.bin.json`, entrée 0 (ligne 9) :
//! `{ "charaId": "0x686BE87E", "textureId": "0x3CCD6535" }`.
//! - `charaId` : identifiant du personnage (même espace que `CTRL_CHR_DATA.chara_id`).
//! - `textureId` : hash de la texture de départ.
//!
//! ## Structure `SPECIFY_PARTY_CHARA_DATA` (7 champs)
//!
//! Exemple réel — `supecify_party0.00.00.cfg.bin.json`, `m_PartyRefCharaList[0]` (ligne 9) :
//! `{ slot:0, type:3, paramId:"0xC19FE60C", lv:0, lvType:0, lvParamId:"0x00000000", equip:"0x00000000" }`.
//! - `type` : 1 = personnage joueur nommé, 3 = référence générique.
//! - L'entrée `m_PartyRefCharaList[9]` (ligne 92) est la seule avec `lv=21` (niveau imposé).
//!
//! ## Structure `SPECIFY_PARTY_DATA` (2 champs)
//!
//! Exemple réel — `supecify_party0.00.00.cfg.bin.json`, `m_SpecifyPartyList[0]` (ligne 177) :
//! `{ "partyId": "0xA8052252", "chara": [0, 1] }`.
//! - `chara = [offset, count]` : plage dans `m_PartyRefCharaList` —
//!   utiliser [`SpecifyPartyConfig::chara_ref_of`] pour la résoudre.
//!
//! ## Structure `GENERAL_LIMIT_INFO` (structure CfgBin arborescente)
//!
//! `guest_limit_config.cfg.bin.json` encode 9 règles dans un arbre CfgBin profondément niché
//! (liste chaînée) : `GENERAL_LIMIT_INFO_N` (rule_hash) → `GENERAL_LIMIT_INFO_DATA_N`
//! (flag=0, condition_data=base64).
//! - `GENERAL_LIMIT_INFO_0` (ligne 13) : var\[0\] = -1346664332 (ligne 17)
//!   → `rule_hash` = 0xAFBB8874.
//! - `GENERAL_LIMIT_INFO_DATA_0` (ligne ~39) : var\[1\] = `"AAAAADALNb7F…"` → `condition_data`.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, field_hash, field_i64, list_values, walk_named};
use crate::hash::HashId;

// ─── CtrlCharaData ────────────────────────────────────────────────────────────

/// Entrée `CTRL_CHR_DATA_*` — personnage contrôlable d'un scénario RPG ou d'un match.
///
/// Parsée depuis `ctrl_chara_config_*.cfg.bin.json` (format `entries`).
/// Utiliser [`parse_ctrl_chara_config`] pour construire la liste complète.
///
/// Vérité terrain : `ctrl_chara_config_1.04.17.00.cfg.bin.json`
/// `CTRL_CHR_DATA_0` (ligne 13) : `[1751902334, 1, 0]`
/// - var\[0\] (ligne 17) = 1751902334 → `chara_id` = 0x686BE87E.
/// - var\[1\] (ligne 21) = 1           → `ctrl_type` = 1.
/// - var\[2\] (ligne 25) = 0           → `flag` = 0.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CtrlCharaData {
    /// var\[0\] — identifiant du personnage (toujours non-nul dans le dump réel).
    ///
    /// Vérité terrain : `CTRL_CHR_DATA_0.var[0]` = 1751902334 → 0x686BE87E.
    pub chara_id: HashId,
    /// var\[1\] — type de contrôle.
    ///
    /// Distribution réelle (155 entrées) : 0 = 22, 1 = 26, 2 = 107.
    /// La sémantique exacte de ces valeurs n'est pas documentée dans les sources disponibles.
    pub ctrl_type: i64,
    /// var\[2\] — indicateur complémentaire (0 dans toutes les 155 entrées réelles).
    pub flag: i64,
}

impl CtrlCharaData {
    /// Parse un noeud `CTRL_CHR_DATA_*`. Renvoie `None` si `chara_id` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let chara_id = node.hash(0);
        if chara_id.is_zero() {
            return None;
        }
        Some(Self {
            chara_id,
            ctrl_type: node.int(1),
            flag: node.int(2),
        })
    }
}

// ─── PartyDeparture ───────────────────────────────────────────────────────────

/// Entrée `m_partyDeparture` — personnage et texture d'intro de départ d'équipe.
///
/// Parsée depuis `party_departure_*.cfg.bin.json` (format `lists`).
///
/// Vérité terrain : `party_departure_0.00.00.cfg.bin.json`, entrée 0 (ligne 9) :
/// `{ "charaId": "0x686BE87E", "textureId": "0x3CCD6535" }`.
/// - 7 entrées au total dans le dump réel.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartyDeparture {
    /// `charaId` — identifiant du personnage.
    ///
    /// Vérité terrain : entrée 0 → 0x686BE87E (ligne 9).
    pub chara_id: HashId,
    /// `textureId` — hash de la texture d'intro associée au personnage.
    ///
    /// Vérité terrain : entrée 0 → 0x3CCD6535 (ligne 10).
    pub texture_id: HashId,
}

impl PartyDeparture {
    /// Parse une entrée de `m_partyDeparture`. Renvoie `None` si `charaId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let chara_id = field_hash(v, "charaId");
        if chara_id.is_zero() {
            return None;
        }
        Some(Self {
            chara_id,
            texture_id: field_hash(v, "textureId"),
        })
    }
}

// ─── SpecifyPartyCharaData ────────────────────────────────────────────────────

/// Entrée `m_PartyRefCharaList` — référence à un personnage dans une équipe spécifiée.
///
/// Parsée depuis `supecify_party*.cfg.bin.json` (format `lists`).
///
/// Vérité terrain : `supecify_party0.00.00.cfg.bin.json`, `m_PartyRefCharaList[0]` (ligne 9) :
/// `{ slot:0, type:3, paramId:"0xC19FE60C", lv:0, lvType:0, lvParamId:"0x00000000",
/// equip:"0x00000000" }`.
///
/// Entrée `m_PartyRefCharaList[9]` (ligne 92) est la seule avec `lv = 21` (niveau imposé).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecifyPartyCharaData {
    /// `slot` — position dans la formation de l'équipe (0-basé).
    pub slot: i64,
    /// `type` — type de personnage (1 = joueur nommé, 3 = personnage générique).
    ///
    /// Nommé `chara_type` pour éviter la collision avec le mot-clé Rust `type`.
    pub chara_type: i64,
    /// `paramId` — hash des paramètres du personnage (toujours non-nul).
    pub param_id: HashId,
    /// `lv` — niveau forcé (0 = libre, valeur non-nulle = niveau imposé).
    pub lv: i64,
    /// `lvType` — type de contrainte de niveau (0 dans toutes les entrées réelles).
    pub lv_type: i64,
    /// `lvParamId` — référence de paramètre de niveau (0x00000000 si absent).
    pub lv_param_id: HashId,
    /// `equip` — hash d'équipement forcé (0x00000000 si absent).
    pub equip: HashId,
}

impl SpecifyPartyCharaData {
    /// Parse une entrée de `m_PartyRefCharaList`. Renvoie `None` si `paramId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let param_id = field_hash(v, "paramId");
        if param_id.is_zero() {
            return None;
        }
        Some(Self {
            slot: field_i64(v, "slot").unwrap_or(0),
            chara_type: field_i64(v, "type").unwrap_or(0),
            param_id,
            lv: field_i64(v, "lv").unwrap_or(0),
            lv_type: field_i64(v, "lvType").unwrap_or(0),
            lv_param_id: field_hash(v, "lvParamId"),
            equip: field_hash(v, "equip"),
        })
    }
}

// ─── SpecifyPartyData ─────────────────────────────────────────────────────────

/// Entrée `m_SpecifyPartyList` — définition d'une équipe spécifiée.
///
/// `chara = [offset, count]` référence la plage dans `m_PartyRefCharaList` —
/// utiliser [`SpecifyPartyConfig::chara_ref_of`] pour la résoudre.
///
/// Vérité terrain : `supecify_party0.00.00.cfg.bin.json`, `m_SpecifyPartyList[0]` (ligne 177) :
/// `{ "partyId": "0xA8052252", "chara": [0, 1] }` → offset=0, count=1.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecifyPartyData {
    /// `partyId` — identifiant unique de l'équipe (toujours non-nul).
    pub party_id: HashId,
    /// `chara[0]` — index de début dans `m_PartyRefCharaList`.
    pub chara_offset: i64,
    /// `chara[1]` — nombre de membres dans cette équipe.
    pub chara_count: i64,
}

impl SpecifyPartyData {
    /// Parse une entrée de `m_SpecifyPartyList`. Renvoie `None` si `partyId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let party_id = field_hash(v, "partyId");
        if party_id.is_zero() {
            return None;
        }
        let chara = v.get("chara").and_then(Value::as_array);
        Some(Self {
            party_id,
            chara_offset: chara
                .and_then(|a| a.first())
                .and_then(Value::as_i64)
                .unwrap_or(0),
            chara_count: chara
                .and_then(|a| a.get(1))
                .and_then(Value::as_i64)
                .unwrap_or(0),
        })
    }
}

// ─── SpecifyPartyConfig ───────────────────────────────────────────────────────

/// Contenu complet d'un fichier `supecify_party*.cfg.bin.json` — deux listes.
///
/// Utiliser [`parse_specify_party`] pour construire depuis un JSON désérialisé.
///
/// Comptes réels (`supecify_party0.00.00.cfg.bin.json`) :
/// - 18 entrées dans `chara_list` (`m_PartyRefCharaList`).
/// - 7 entrées dans `party_list` (`m_SpecifyPartyList`).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecifyPartyConfig {
    /// `m_PartyRefCharaList` — références de personnages.
    pub chara_list: Vec<SpecifyPartyCharaData>,
    /// `m_SpecifyPartyList` — équipes spécifiées.
    pub party_list: Vec<SpecifyPartyData>,
}

impl SpecifyPartyConfig {
    /// Renvoie la tranche de [`SpecifyPartyCharaData`] pour une équipe donnée.
    ///
    /// Résout `party.chara_offset..party.chara_offset + party.chara_count` dans `chara_list`.
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::party::{SpecifyPartyConfig, SpecifyPartyData, SpecifyPartyCharaData};
    /// use nie_data::hash::HashId;
    ///
    /// let cfg = SpecifyPartyConfig {
    ///     chara_list: vec![
    ///         SpecifyPartyCharaData {
    ///             slot: 0, chara_type: 3,
    ///             param_id: HashId(0xC19FE60C),
    ///             lv: 0, lv_type: 0,
    ///             lv_param_id: HashId::ZERO, equip: HashId::ZERO,
    ///         },
    ///     ],
    ///     party_list: vec![],
    /// };
    /// let party = SpecifyPartyData { party_id: HashId(0xA8052252), chara_offset: 0, chara_count: 1 };
    /// let membres = cfg.chara_ref_of(&party);
    /// assert_eq!(membres.len(), 1);
    /// assert_eq!(membres[0].param_id, HashId(0xC19FE60C));
    /// ```
    #[must_use]
    pub fn chara_ref_of(&self, party: &SpecifyPartyData) -> &[SpecifyPartyCharaData] {
        let start = party.chara_offset as usize;
        let end = start.saturating_add(party.chara_count as usize);
        self.chara_list.get(start..end).unwrap_or(&[])
    }

    /// Recherche une équipe par son `party_id`. `None` si absente.
    #[must_use]
    pub fn find_party(&self, party_id: HashId) -> Option<&SpecifyPartyData> {
        self.party_list.iter().find(|p| p.party_id == party_id)
    }
}

// ─── GeneralLimitInfo ─────────────────────────────────────────────────────────

/// Règle de limite d'invité — paire `GENERAL_LIMIT_INFO_N` / `GENERAL_LIMIT_INFO_DATA_N`.
///
/// Encodée dans `guest_limit_config.cfg.bin.json` sous forme d'arbre CfgBin profondément
/// niché (liste chaînée à 9 nœuds). Le parseur [`parse_guest_limit_config`] utilise une
/// machine d'état DFS pour reconstituer les paires (rule_hash, condition_data).
///
/// Vérité terrain, entrée 0 :
/// - `GENERAL_LIMIT_INFO_0` (ligne 13) : var\[0\] = -1346664332 (ligne 17)
///   → `rule_hash` = 0xAFBB8874.
/// - `GENERAL_LIMIT_INFO_DATA_0` (ligne 39) : var\[1\]
///   = `"AAAAADALNb7FPqIACgEoAAYCNOlC0FgyAAAAAXg1TO8TnAAKASgABgI0MH+9OTIAAAABeY8="`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeneralLimitInfo {
    /// `GENERAL_LIMIT_INFO_N.var[0]` — hash de la règle (toujours non-nul dans le dump réel).
    pub rule_hash: HashId,
    /// `GENERAL_LIMIT_INFO_DATA_N.var[1]` — condition encodée en base64.
    ///
    /// Contenu opaque pour le runtime (données de configuration interne Level-5).
    pub condition_data: String,
}

// ─── Parseurs principaux ──────────────────────────────────────────────────────

/// Parse les `CTRL_CHR_DATA_*` d'un `ctrl_chara_config_*.cfg.bin.json`.
///
/// Renvoie la liste des [`CtrlCharaData`] valides (chara_id non-nul). Le noeud conteneur
/// `CTRL_CHR_DATA_LIST_BEG_0` (var\[0\] = 155) est silencieusement ignoré.
///
/// Compte réel : `ctrl_chara_config_1.04.17.00.cfg.bin.json` → **155 entrées**.
#[must_use]
pub fn parse_ctrl_chara_config(root: &Value) -> Vec<CtrlCharaData> {
    let mut out = Vec::new();
    walk_named(root, "CTRL_CHR_DATA_", |node| {
        if node.name().contains("_LIST_BEG_") {
            return;
        }
        if let Some(item) = CtrlCharaData::from_node(node) {
            out.push(item);
        }
    });
    out
}

/// Parse `m_partyDeparture` d'un `party_departure_*.cfg.bin.json`.
///
/// Renvoie les [`PartyDeparture`] valides (charaId non-nul).
///
/// Compte réel : `party_departure_0.00.00.cfg.bin.json` → **7 entrées**.
#[must_use]
pub fn parse_party_departure(root: &Value) -> Vec<PartyDeparture> {
    let mut out = Vec::new();
    if let Some(values) = list_values(root, "m_partyDeparture") {
        for v in values {
            if let Some(item) = PartyDeparture::from_value(v) {
                out.push(item);
            }
        }
    }
    out
}

/// Parse un `supecify_party*.cfg.bin.json` complet — deux listes.
///
/// Renvoie un [`SpecifyPartyConfig`] avec `chara_list` et `party_list`.
///
/// Comptes réels (`supecify_party0.00.00.cfg.bin.json`) :
/// - `m_PartyRefCharaList` → **18 entrées** `SPECIFY_PARTY_CHARA_DATA`.
/// - `m_SpecifyPartyList`  → **7 entrées** `SPECIFY_PARTY_DATA`.
#[must_use]
pub fn parse_specify_party(root: &Value) -> SpecifyPartyConfig {
    let mut chara_list = Vec::new();
    if let Some(values) = list_values(root, "m_PartyRefCharaList") {
        for v in values {
            if let Some(item) = SpecifyPartyCharaData::from_value(v) {
                chara_list.push(item);
            }
        }
    }
    let mut party_list = Vec::new();
    if let Some(values) = list_values(root, "m_SpecifyPartyList") {
        for v in values {
            if let Some(item) = SpecifyPartyData::from_value(v) {
                party_list.push(item);
            }
        }
    }
    SpecifyPartyConfig {
        chara_list,
        party_list,
    }
}

/// Parse un `guest_limit_config.cfg.bin.json` — 9 règles de limite d'invité.
///
/// La structure CfgBin est un arbre profondément niché (liste chaînée). Ce parseur
/// utilise une machine d'état sur le parcours DFS de [`walk_named`] :
/// - Nœud `GENERAL_LIMIT_INFO_N` (pas `_DATA_`, pas `_LIST_BEG_`) → mémorise `rule_hash`.
/// - Nœud `GENERAL_LIMIT_INFO_DATA_N` (pas `_LIST_BEG_`) → émet une [`GeneralLimitInfo`].
///
/// Compte réel : `guest_limit_config.cfg.bin.json` → **9 entrées**.
#[must_use]
pub fn parse_guest_limit_config(root: &Value) -> Vec<GeneralLimitInfo> {
    let mut out = Vec::new();
    let mut pending_hash: Option<HashId> = None;
    walk_named(root, "GENERAL_LIMIT_INFO_", |node| {
        let name = node.name();
        if name.contains("_LIST_BEG_") {
            return;
        }
        if name.contains("_DATA_") {
            if let Some(rule_hash) = pending_hash.take() {
                out.push(GeneralLimitInfo {
                    rule_hash,
                    condition_data: String::from(node.string(1)),
                });
            }
        } else {
            pending_hash = Some(node.hash(0));
        }
    });
    out
}
