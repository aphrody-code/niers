//! `CommandConfig` — port des familles de configuration `command/*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! Cinq fichiers dans `data/common/gamedata/command/` :
//!
//! | Fichier | Taille | Noeuds | Vars/entrée |
//! |---------|--------|--------|-------------|
//! | `chara_cmd_event_common_0.00.00.cfg.bin.json` | 7 674 oct. | 32 `CHARA_ACTION_CMD_DATA_*` | 1 (`Int`) |
//! | `soccer_cmd_action_0.07.70.cfg.bin.json` | 107 381 oct. | 88 `CMD_ACTION_INFO_*` | 12 (`Int`) |
//! | `soccer_cmd_event_0.07.70.cfg.bin.json` | 1 729 086 oct. | 99 `ALL_CMD_EVENT_INFO_*` + 51 `CHARA_CMD_EVENT_FUNC_*` | 3 + 2 |
//! | `rpg_cmd_action_1.03.53.00.cfg.bin.json` | 279 649 oct. | 229 `CMD_ACTION_INFO_*` | 12 (`Int`) |
//! | `rpg_cmd_event_1.02.75.00.cfg.bin.json` | 1 306 959 oct. | 231 `ALL_CMD_EVENT_INFO_*` + 39 `CHARA_CMD_EVENT_FUNC_*` | 3 + 2 |
//!
//! Tous les fichiers utilisent le format **`entries`** (noeuds nommés, variables positionnelles)
//! et **non** le format `lists` (qui a des champs nommés comme dans `formation_config`).
//! Les variables sont donc positionnelles — aucun nom officiel n'est disponible sans la source
//! TS inagle.
//!
//! ## Structure `CMD_ACTION_INFO` (12 vars, partagée soccer + rpg)
//!
//! Exemple réel — `soccer_cmd_action_0.07.70.cfg.bin.json`, `CMD_ACTION_INFO_0` :
//! `[-566667975, 0, 0, 0, -2102675611, 894303940, -817083886, 0, 0, 0, 0, 0]`
//! - var\[0\] = `action_id` = 0xDE395539 : identifiant principal, **100% non-nul**.
//! - var\[6\] = hash secondaire = 0xCF4C4A12 : présent dans **~95%** des entrées soccer et rpg.
//! - var\[4\], var\[5\] : hashes optionnels (5/88 et 1/88 dans soccer, absents dans rpg).
//! - var\[1\], var\[2\], var\[3\] : hashes optionnels (≤ 17/229 dans rpg).
//! - var\[7\], var\[8\] : hashes rpg-only (18/229 et 79/229 dans rpg).
//! - var\[11\] : rarement `Float` (ex. `"0,9"`, `"1,7"`) — tronqué à `HashId(0)` via `as_i64`.
//!
//! ## Structure `ALL_CMD_EVENT_INFO` (3 vars)
//!
//! `ALL_CMD_EVENT_INFO_0` (`soccer_cmd_event_0.07.70.cfg.bin.json`) :
//! `[-566667975, 1356072398, 0]`
//! - var\[0\] = `cmd_action_id` = 0xDE395539 (lien vers `CmdActionInfo.action_id`).
//! - var\[1\] = `func_hash` = 0x50D405CE.
//! - var\[2\] = toujours 0 (ignoré).
//!
//! ## Structure `CHARA_CMD_EVENT_FUNC` (2 vars)
//!
//! `CHARA_CMD_EVENT_FUNC_0` (`soccer_cmd_event_0.07.70.cfg.bin.json`) :
//! `[1, -236478904]`
//! - var\[0\] = `func_no` = 1 (entier séquentiel 1-basé).
//! - var\[1\] = `func_hash` = 0xF1E79E48.
//!
//! ## Structure `CHARA_ACTION_CMD_DATA` (1 var)
//!
//! `CHARA_ACTION_CMD_DATA_0` (`chara_cmd_event_common_0.00.00.cfg.bin.json`) :
//! `[-1813128698]`
//! - var\[0\] = `cmd_action_id` = 0x93EDDA06.
//!
//! ## Utilisation dans `nie-core/match_sim`
//!
//! `parse_cmd_actions` retourne les 88 (soccer) ou 229 (rpg) `CmdActionInfo` définissant les
//! commandes jouables pendant un match. `parse_cmd_events` associe chaque commande à sa fonction
//! d'événement (`func_hash`) dans `CmdEventConfig`. Le port dans la logique de match est
//! **hors scope de ce module** — ce module expose les données brutes.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

// ─── CmdActionInfo ────────────────────────────────────────────────────────────

/// Entrée `CMD_ACTION_INFO_*` — définition d'une action de commande de match.
///
/// Partagée par `soccer_cmd_action_*.cfg.bin.json` et `rpg_cmd_action_*.cfg.bin.json`.
/// Les 12 variables sont positionnelles (format `entries` cfgbin, pas de noms officiels).
///
/// # Positions connues
///
/// | Position | Présence | Sémantique observée |
/// |----------|----------|---------------------|
/// | var\[0\] | 100 % | `action_id` — identifiant principal de la commande |
/// | var\[6\] | ~95 % | hash secondaire (type de référence inconnu sans source TS) |
/// | var\[4\], var\[5\] | ~6 %, ~1 % | hashes optionnels (soccer uniquement) |
/// | var\[1\], var\[2\], var\[3\] | ≤ 17 % | hashes optionnels (rpg) |
/// | var\[7\], var\[8\] | ≤ 35 % | hashes optionnels (rpg) |
/// | var\[11\] | < 2 % | float tronqué (ex. `"0,9"` → `HashId(0)` ; rpg uniquement) |
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CmdActionInfo {
    /// var\[0\] — identifiant principal de l'action (toujours non-nul).
    ///
    /// Vérité terrain : `soccer_cmd_action_0.07.70.cfg.bin.json` CMD_ACTION_INFO_0.var[0]
    /// = -566667975 → 0xDE395539.
    pub action_id: HashId,
    /// var\[1\..=11\] — variables positionnelles brutes.
    ///
    /// `raw[5]` (= var\[6\]) est le hash secondaire, présent dans ~95 % des entrées.
    /// Accès pratique via [`CmdActionInfo::secondary_hash`].
    pub raw: [HashId; 11],
}

impl CmdActionInfo {
    /// Parse un noeud `CMD_ACTION_INFO_*`. Renvoie `None` si `action_id` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let action_id = node.hash(0);
        if action_id.is_zero() {
            return None;
        }
        let mut raw = [HashId::ZERO; 11];
        for (i, slot) in raw.iter_mut().enumerate() {
            *slot = node.hash(i + 1);
        }
        Some(Self { action_id, raw })
    }

    /// var\[6\] (`raw[5]`) — hash secondaire, présent dans ~95 % des entrées.
    ///
    /// Vérité terrain : `soccer_cmd_action_0.07.70.cfg.bin.json` CMD_ACTION_INFO_0.var[6]
    /// = -817083886 → 0xCF4C4A12.
    #[must_use]
    #[inline]
    pub fn secondary_hash(&self) -> HashId {
        self.raw[5]
    }

    /// `true` si l'entrée est valide (identifiant non-nul).
    #[must_use]
    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.action_id.is_zero()
    }
}

// ─── AllCmdEventInfo ──────────────────────────────────────────────────────────

/// Entrée `ALL_CMD_EVENT_INFO_*` — association commande ↔ fonction d'événement.
///
/// Partagée par `soccer_cmd_event_*.cfg.bin.json` et `rpg_cmd_event_*.cfg.bin.json`.
/// Chaque entrée lie un `cmd_action_id` (référence vers [`CmdActionInfo::action_id`])
/// à un `func_hash` identifiant la fonction d'événement déclenchée.
///
/// Vérité terrain : `soccer_cmd_event_0.07.70.cfg.bin.json` ALL_CMD_EVENT_INFO_0 :
/// `[-566667975, 1356072398, 0]` → cmd_action_id = 0xDE395539, func_hash = 0x50D405CE.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllCmdEventInfo {
    /// var\[0\] — référence vers `CmdActionInfo.action_id`.
    pub cmd_action_id: HashId,
    /// var\[1\] — hash de la fonction d'événement.
    pub func_hash: HashId,
    // var[2] est systématiquement 0 dans les fichiers réels — ignoré.
}

impl AllCmdEventInfo {
    /// Parse un noeud `ALL_CMD_EVENT_INFO_*`. Renvoie `None` si `cmd_action_id` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let cmd_action_id = node.hash(0);
        if cmd_action_id.is_zero() {
            return None;
        }
        Some(Self {
            cmd_action_id,
            func_hash: node.hash(1),
        })
    }
}

// ─── CharaCmdEventFunc ────────────────────────────────────────────────────────

/// Entrée `CHARA_CMD_EVENT_FUNC_*` — index numérique d'une fonction d'événement de commande.
///
/// Partagée par `soccer_cmd_event_*.cfg.bin.json` et `rpg_cmd_event_*.cfg.bin.json`.
///
/// Vérité terrain : `soccer_cmd_event_0.07.70.cfg.bin.json` CHARA_CMD_EVENT_FUNC_0 :
/// `[1, -236478904]` → func_no = 1, func_hash = 0xF1E79E48.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaCmdEventFunc {
    /// var\[0\] — numéro de fonction (entier séquentiel 1-basé ; non-nul).
    pub func_no: i64,
    /// var\[1\] — hash identifiant la fonction.
    pub func_hash: HashId,
}

impl CharaCmdEventFunc {
    /// Parse un noeud `CHARA_CMD_EVENT_FUNC_*`. Renvoie `None` si `func_no` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let func_no = node.int(0);
        if func_no == 0 {
            return None;
        }
        Some(Self {
            func_no,
            func_hash: node.hash(1),
        })
    }
}

// ─── CmdEventConfig ───────────────────────────────────────────────────────────

/// Contenu complet d'un fichier `*_cmd_event_*.cfg.bin.json` (soccer ou rpg).
///
/// Contient deux listes :
/// - `all_cmd_events` : 99 (soccer) ou 231 (rpg) associations commande↔événement.
/// - `chara_cmd_funcs` : 51 (soccer) ou 39 (rpg) définitions de fonction.
///
/// Utiliser [`parse_cmd_events`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CmdEventConfig {
    /// Entrées `ALL_CMD_EVENT_INFO_*` — association commande ↔ fonction.
    pub all_cmd_events: Vec<AllCmdEventInfo>,
    /// Entrées `CHARA_CMD_EVENT_FUNC_*` — index numérique des fonctions d'événement.
    pub chara_cmd_funcs: Vec<CharaCmdEventFunc>,
}

impl CmdEventConfig {
    /// Recherche la fonction d'événement associée à un `cmd_action_id`. `None` si absente.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::command::{AllCmdEventInfo, CmdEventConfig};
    /// use nie_data::hash::HashId;
    ///
    /// let config = CmdEventConfig {
    ///     all_cmd_events: vec![AllCmdEventInfo {
    ///         cmd_action_id: HashId(0xDE395539),
    ///         func_hash: HashId(0x50D405CE),
    ///     }],
    ///     chara_cmd_funcs: vec![],
    /// };
    /// let ev = config.find_event(HashId(0xDE395539));
    /// assert!(ev.is_some());
    /// assert_eq!(ev.unwrap().func_hash, HashId(0x50D405CE));
    /// ```
    #[must_use]
    pub fn find_event(&self, cmd_action_id: HashId) -> Option<&AllCmdEventInfo> {
        self.all_cmd_events
            .iter()
            .find(|e| e.cmd_action_id == cmd_action_id)
    }
}

// ─── Parseurs principaux ──────────────────────────────────────────────────────

/// Parse les `CMD_ACTION_INFO_*` d'un `*_cmd_action_*.cfg.bin.json` (soccer ou rpg).
///
/// Renvoie la liste de toutes les [`CmdActionInfo`] valides (action_id non-nul).
/// Les noeuds conteneurs (`*_LIST_BEG_*`) sont ignorés.
///
/// Comptes réels :
/// - `soccer_cmd_action_0.07.70.cfg.bin.json` → 88 entrées.
/// - `rpg_cmd_action_1.03.53.00.cfg.bin.json` → 229 entrées.
#[must_use]
pub fn parse_cmd_actions(root: &Value) -> Vec<CmdActionInfo> {
    let mut out = Vec::new();
    walk_named(root, "CMD_ACTION_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(info) = CmdActionInfo::from_node(node) {
            out.push(info);
        }
    });
    out
}

/// Parse un `*_cmd_event_*.cfg.bin.json` complet (soccer ou rpg) — deux listes.
///
/// Renvoie un [`CmdEventConfig`] avec `all_cmd_events` et `chara_cmd_funcs`.
///
/// Comptes réels :
/// - soccer : 99 `ALL_CMD_EVENT_INFO` + 51 `CHARA_CMD_EVENT_FUNC`.
/// - rpg    : 231 `ALL_CMD_EVENT_INFO` + 39 `CHARA_CMD_EVENT_FUNC`.
#[must_use]
pub fn parse_cmd_events(root: &Value) -> CmdEventConfig {
    let mut all_cmd_events = Vec::new();
    walk_named(root, "ALL_CMD_EVENT_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(ev) = AllCmdEventInfo::from_node(node) {
            all_cmd_events.push(ev);
        }
    });

    let mut chara_cmd_funcs = Vec::new();
    walk_named(root, "CHARA_CMD_EVENT_FUNC_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(f) = CharaCmdEventFunc::from_node(node) {
            chara_cmd_funcs.push(f);
        }
    });

    CmdEventConfig {
        all_cmd_events,
        chara_cmd_funcs,
    }
}

/// Parse un `chara_cmd_event_common_*.cfg.bin.json` — liste de `CHARA_ACTION_CMD_DATA_*`.
///
/// Chaque noeud `CHARA_ACTION_CMD_DATA_N` contient un seul entier (hash `cmd_action_id`).
/// Renvoie les 32 [`HashId`] valides (non-nuls) du fichier réel.
///
/// Vérité terrain : `chara_cmd_event_common_0.00.00.cfg.bin.json` → 32 entrées.
/// `CHARA_ACTION_CMD_DATA_0` : var\[0\] = -1813128698 → `HashId` = 0x93EDDA06.
#[must_use]
pub fn parse_chara_cmd_common(root: &Value) -> Vec<HashId> {
    let mut out = Vec::new();
    walk_named(root, "CHARA_ACTION_CMD_DATA_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        let id = node.hash(0);
        if !id.is_zero() {
            out.push(id);
        }
    });
    out
}
