//! `MissionConfig` — port Rust des familles `mission/*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! Fichiers dans `data/common/gamedata/mission/` :
//!
//! | Fichier | Noeuds | Vars/entrée |
//! |---------|--------|-------------|
//! | `mission_config_0.00.00.cfg.bin.json` | 1 `MISSION_CONFIG_INFO_*` | 16 (Int + String) |
//! | `msa999999_trigger_0.04.78.cfg.bin.json` | 1 `DATA_ITEM_*` | 7 (Int + String) |
//! | `msa999999_layout_config.cfg.bin.json` | 0 (liste vide) | — |
//! | `msa999999_oneplace_setting.cfg.bin.json` | 7 listes vides | — |
//!
//! Tous les fichiers utilisent le format **`entries`** (noeuds positionnels, variables
//! sans noms officiels — pas de format `lists` comme dans `formation_config`).
//!
//! ## Structure `MISSION_CONFIG_INFO` (16 vars)
//!
//! `MISSION_CONFIG_INFO_0` (`mission_config_0.00.00.cfg.bin.json`) :
//! - var\[0\] = 1013400427 → `mission_id` = `0x3C67436B` (**non-nul**, identifiant principal)
//! - var\[1\] = `"msa999999"` (String — code court de la mission)
//! - var\[2\] = 1316720857 → raw\[0\] = `0x4E7B90D9`
//! - var\[3\] = 1 → raw\[1\] = `0x00000001`
//! - var\[4\] = 0 → raw\[2\] = `0x00000000`
//! - var\[5\] = −736280260 → raw\[3\] = `0xD41D413C`
//! - var\[6..=8\] = 0 → raw\[4..=6\] = zéro
//! - var\[9\] = 1475254969 → raw\[7\] = `0x57EE9AB9`
//! - var\[10\] = −813411175 → raw\[8\] = `0xCF845499`
//! - var\[11\] = 2083486252 → raw\[9\] = `0x7C2F7A2C`
//! - var\[12..=15\] = 0 → raw\[10..=13\] = zéro
//!
//! ## Structure `DATA_ITEM` (7 vars)
//!
//! `DATA_ITEM_0` (`msa999999_trigger_0.04.78.cfg.bin.json`) :
//! - var\[0\] = 1 (`trigger_type`)
//! - var\[1\] = −1369358024 → `trigger_hash` = `0xAE614138`
//! - var\[2\] = 0 (flag entier)
//! - var\[3\] = `"AAAAAAoDNbkZNtoAAQB4"` (String — données sérialisées, probablement base64)
//! - var\[4\] = 1, var\[5\] = 0, var\[6\] = 1 (flags entiers)
//!
//! ## Périmètre non couvert
//!
//! - `*_layout_config.cfg.bin.json` : liste `LAYOUT_BASE` vide (count=0) dans le seul
//!   dump disponible (`msa999999`). Structure interne inconnue sans exemple non-vide.
//! - `*_oneplace_setting.cfg.bin.json` : 7 listes vides (`OP_TBOX`, `OP_FUNC`,
//!   `COLLISION_COL`, `OP_EFFECT_CFG`, `EXCEPTION_OP_INFO`, `WATCH_LOCK_INFO`,
//!   `VENDING_MACHINE_INFO`). Structures internes inconnues sans exemple non-vide.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

// ─── MissionConfigInfo ────────────────────────────────────────────────────────

/// Entrée `MISSION_CONFIG_INFO_*` — paramètres de configuration d'une mission.
///
/// Issue du fichier `mission_config_*.cfg.bin.json`, format **`entries`** positionnels.
/// L'entrée est un enfant de `MISSION_CONFIG_INFO_LIST_BEG_*` (noeud conteneur ignoré).
///
/// # Positions connues
///
/// | Position | Type JSON | Présence | Sémantique observée |
/// |----------|-----------|----------|---------------------|
/// | var\[0\] | Int | 100 % | `mission_id` — identifiant hash principal |
/// | var\[1\] | String | 100 % | `mission_code` — code court (ex. `"msa999999"`) |
/// | var\[2..=15\] | Int | — | 14 hashes/flags — sémantique inconnue sans source TS |
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MissionConfigInfo {
    /// var\[0\] — identifiant hash de la mission (toujours non-nul).
    ///
    /// Vérité terrain : `mission_config_0.00.00.cfg.bin.json`
    /// `MISSION_CONFIG_INFO_0.variables[0].value = "1013400427"` → `0x3C67436B`.
    pub mission_id: HashId,

    /// var\[1\] — code court de la mission (type `String` dans le JSON).
    ///
    /// Vérité terrain : `mission_config_0.00.00.cfg.bin.json`
    /// `MISSION_CONFIG_INFO_0.variables[1].value = "msa999999"`.
    pub mission_code: String,

    /// var\[2..=15\] — 14 variables positionnelles brutes.
    ///
    /// Mapping : `raw[k]` = var\[k+2\]. Sémantique inconnue sans la source TS inagle.
    ///
    /// Valeurs non-nulles connues (`MISSION_CONFIG_INFO_0`) :
    /// - raw\[0\] = var\[2\] = 1316720857 → `0x4E7B90D9`
    /// - raw\[1\] = var\[3\] = 1 → `0x00000001`
    /// - raw\[3\] = var\[5\] = −736280260 → `0xD41D413C`
    /// - raw\[7\] = var\[9\] = 1475254969 → `0x57EE9AB9`
    /// - raw\[8\] = var\[10\] = −813411175 → `0xCF845499`
    /// - raw\[9\] = var\[11\] = 2083486252 → `0x7C2F7A2C`
    pub raw: [HashId; 14],
}

impl MissionConfigInfo {
    /// Parse un noeud `MISSION_CONFIG_INFO_*`. Renvoie `None` si `mission_id` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let mission_id = node.hash(0);
        if mission_id.is_zero() {
            return None;
        }
        let mission_code = String::from(node.string(1));
        let mut raw = [HashId::ZERO; 14];
        for (i, slot) in raw.iter_mut().enumerate() {
            *slot = node.hash(i + 2);
        }
        Some(Self {
            mission_id,
            mission_code,
            raw,
        })
    }
}

// ─── MissionTriggerItem ───────────────────────────────────────────────────────

/// Entrée `DATA_ITEM_*` — déclencheur d'un fichier `*_trigger_*.cfg.bin.json`.
///
/// Vérité terrain : `msa999999_trigger_0.04.78.cfg.bin.json` `DATA_ITEM_0` :
/// `[1, −1369358024, 0, "AAAAAAoDNbkZNtoAAQB4", 1, 0, 1]`
///
/// La sémantique complète des 7 variables est inconnue sans la source TS inagle.
/// Le noeud `DATA_COUNT_*` (sibling à la racine) n'est pas parsé ici.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MissionTriggerItem {
    /// var\[0\] — type du déclencheur.
    ///
    /// Vérité terrain : `DATA_ITEM_0.variables[0].value = "1"`.
    pub trigger_type: i64,

    /// var\[1\] — hash identifiant du déclencheur.
    ///
    /// Vérité terrain : `DATA_ITEM_0.variables[1].value = "-1369358024"` → `0xAE614138`.
    pub trigger_hash: HashId,

    /// var\[2\] — flag entier (0 dans le dump réel).
    pub flag2: i64,

    /// var\[3\] — données sérialisées (type `String`, probablement base64).
    ///
    /// Vérité terrain : `DATA_ITEM_0.variables[3].value = "AAAAAAoDNbkZNtoAAQB4"`.
    pub data: String,

    /// var\[4\] — flag entier (1 dans le dump réel).
    pub flag4: i64,

    /// var\[5\] — flag entier (0 dans le dump réel).
    pub flag5: i64,

    /// var\[6\] — flag entier (1 dans le dump réel).
    pub flag6: i64,
}

impl MissionTriggerItem {
    /// Parse un noeud `DATA_ITEM_*`. Renvoie `None` si moins de 4 variables sont présentes.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        if node.var_count() < 4 {
            return None;
        }
        Some(Self {
            trigger_type: node.int(0),
            trigger_hash: node.hash(1),
            flag2: node.int(2),
            data: String::from(node.string(3)),
            flag4: node.int(4),
            flag5: node.int(5),
            flag6: node.int(6),
        })
    }
}

// ─── Parseurs principaux ──────────────────────────────────────────────────────

/// Parse les `MISSION_CONFIG_INFO_*` d'un `mission_config_*.cfg.bin.json`.
///
/// Les noeuds conteneurs (`MISSION_CONFIG_INFO_LIST_BEG_*`) sont ignorés.
/// Les enfants du noeud conteneur sont correctement explorés (parcours récursif).
///
/// Comptes réels :
/// - `mission_config_0.00.00.cfg.bin.json` → 1 entrée.
///
/// # Exemple
///
/// ```
/// use nie_data::mission::parse_mission_config;
/// use nie_data::hash::HashId;
/// use serde_json::json;
///
/// let root = json!({
///     "entries": [{
///         "name": "MISSION_CONFIG_INFO_LIST_BEG_0",
///         "variables": [{"type": "Int", "value": "1"}],
///         "children": [{
///             "name": "MISSION_CONFIG_INFO_0",
///             "variables": [
///                 {"type": "Int",    "value": "1013400427"},
///                 {"type": "String", "value": "msa999999"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"},
///                 {"type": "Int",    "value": "0"}
///             ],
///             "children": []
///         }]
///     }]
/// });
/// let infos = parse_mission_config(&root);
/// assert_eq!(infos.len(), 1);
/// assert_eq!(infos[0].mission_id, HashId::from_i64(1013400427));
/// assert_eq!(infos[0].mission_code, "msa999999");
/// ```
#[must_use]
pub fn parse_mission_config(root: &Value) -> Vec<MissionConfigInfo> {
    let mut out = Vec::new();
    walk_named(root, "MISSION_CONFIG_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(info) = MissionConfigInfo::from_node(node) {
            out.push(info);
        }
    });
    out
}

/// Parse les `DATA_ITEM_*` d'un `*_trigger_*.cfg.bin.json`.
///
/// Le noeud `DATA_COUNT_*` (préfixe différent) est ignoré automatiquement.
///
/// Comptes réels :
/// - `msa999999_trigger_0.04.78.cfg.bin.json` → 1 entrée.
#[must_use]
pub fn parse_mission_trigger(root: &Value) -> Vec<MissionTriggerItem> {
    let mut out = Vec::new();
    walk_named(root, "DATA_ITEM_", |node| {
        if let Some(item) = MissionTriggerItem::from_node(node) {
            out.push(item);
        }
    });
    out
}

impl MissionConfigInfo {
    /// `nameId` = var\[2\] = `raw[0]` — clé de jointure vers `mission_text`
    /// (port de `nameId = vars[2]` de `mission-config.ts`).
    #[must_use]
    pub fn name_id(&self) -> HashId {
        self.raw[0]
    }
}

/// Résout le **nom localisé** d'une mission via une liste `mission_text` (issue de
/// [`crate::text::parse_text_file`]).
///
/// Port de la jointure de `mission-config.ts` (`loadTextFile("mission", locale).get(nameIdHex)`)
/// où `nameId` = var\[2\] = [`MissionConfigInfo::name_id`]. `None` si le hash n'a pas d'entrée.
#[must_use]
pub fn resolve_name<'a>(
    mission: &MissionConfigInfo,
    mission_text: &'a [(HashId, String)],
) -> Option<&'a str> {
    crate::text::find_text(mission_text, mission.name_id())
}
