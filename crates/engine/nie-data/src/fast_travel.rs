//! `FastTravelConfig` — port de `fast_travel_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `/home/ubuntu/niers/data/common/gamedata/fast_travel/fast_travel_config_0.00.00.cfg.bin.json`
//! - 1 liste : `m_fastTravelMapInfoList` — 6 entrées dans ce dump.
//!   Chaque entrée `FAST_TRAVEL_MAP_INFO` contient :
//!   - `id` — hash identifiant unique du point de voyage rapide.
//!   - `mapId` — hash de la carte cible (partagé par plusieurs points sur la même zone).
//!   - `mapTextId` — hash du texte d'affichage (jointure table de textes du jeu).
//!   - `pos` — position 3D + padding w du personnage à l'arrivée : `[x, y, z, w]`.
//!   - `charaRotateY` — angle de rotation du personnage autour de l'axe Y à l'arrivée (degrés).
//!
//! ## Contenu du dump réel (6 entrées)
//!
//! | idx | id           | mapId        | pos (x, y, z, w)           | charaRotateY |
//! |-----|--------------|--------------|----------------------------|--------------|
//! | 0   | 0x7225AA02   | 0x4E7B90D9   | [0.0, 0.0, 45.0, 0.0]      | −180         |
//! | 1   | 0x05229A94   | 0x4E7B90D9   | [0.0, 0.0, −69.0, 0.0]     | 0            |
//! | 2   | 0x9C2BCB2E   | 0x4E7B90D9   | [−51.94, 0.0, −19.47, 0.0] | 50           |
//! | 3   | 0xEB2CFBB8   | 0x4E7B90D9   | [64.53, 0.0, −193.64, 0.0] | 90           |
//! | 4   | 0x6B3E9B43   | 0xD772C163   | [0.0, 0.0, −30.0, 0.0]     | 0            |
//! | 5   | 0xA3E7114B   | 0x2067910E   | [0.0, 0.0, 0.0, 0.0]       | 0            |
//!
//! Les entrées 0–3 partagent le même `mapId` (0x4E7B90D9) : elles désignent toutes des
//! points de la même zone de carte. L'entrée 5 a une position nulle — sentinelle probable
//! (position par défaut du hub principal).
//!
//! ## Format JSON
//!
//! Format `lists` (champs nommés, comme `formation_config`) — **non** le format `entries`
//! (variables positionnelles, comme `command`).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, list_values};
use crate::hash::HashId;

// ─── FastTravelMapInfo ─────────────────────────────────────────────────────────

/// Point de voyage rapide (`FAST_TRAVEL_MAP_INFO`).
///
/// Définit un point de téléportation accessible depuis le menu de voyage rapide du jeu.
/// Chaque entrée associe un identifiant unique à une carte cible, une position monde
/// et une orientation du personnage à l'arrivée.
///
/// Vérité terrain :
/// `fast_travel_config_0.00.00.cfg.bin.json` — `m_fastTravelMapInfoList[0..6]`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FastTravelMapInfo {
    /// `id` — hash identifiant unique du point de voyage rapide.
    ///
    /// Vérité terrain : entrée 0 = `"0x7225AA02"`.
    pub id: HashId,
    /// `mapId` — hash de la carte cible.
    ///
    /// Vérité terrain : entrées 0–3 partagent `"0x4E7B90D9"` (même zone de carte).
    pub map_id: HashId,
    /// `mapTextId` — hash du texte d'affichage du nom de lieu (jointure table de textes).
    ///
    /// Vérité terrain : entrée 0 = `"0xD20ECFA3"`.
    pub map_text_id: HashId,
    /// `pos` — position 3D + padding w du personnage à l'arrivée dans l'espace monde.
    ///
    /// Composantes : `[x, y, z, w]`. Le composant `w` est systématiquement 0 dans ce dump.
    /// Exemple — entrée 2 : `[−51.94, 0.0, −19.47, 0.0]`.
    pub pos: [f32; 4],
    /// `charaRotateY` — rotation du personnage autour de l'axe Y à l'arrivée (degrés).
    ///
    /// Vérité terrain : entrée 0 = −180.0, entrée 3 = 90.0.
    pub chara_rotate_y: f32,
}

impl FastTravelMapInfo {
    /// Parse une entrée de `m_fastTravelMapInfoList`. Renvoie `None` si `id` est absent ou nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            map_id: field_hash(v, "mapId"),
            map_text_id: field_hash(v, "mapTextId"),
            pos: parse_pos4(v),
            chara_rotate_y: v.get("charaRotateY").and_then(Value::as_f64).unwrap_or(0.0) as f32,
        })
    }
}

/// Extrait le tableau `pos = [x, y, z, w]` depuis un objet JSON.
///
/// Retourne `[0.0; 4]` si le champ est absent ou mal formé. Convertit chaque
/// élément du tableau JSON (entier ou flottant) en `f32`.
fn parse_pos4(v: &Value) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    if let Some(arr) = v.get("pos").and_then(Value::as_array) {
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(n) = arr.get(i).and_then(Value::as_f64) {
                *slot = n as f32;
            }
        }
    }
    out
}

// ─── FastTravelConfig ──────────────────────────────────────────────────────────

/// Contenu complet d'un `fast_travel_config_*.cfg.bin.json` (1 liste).
///
/// Utiliser [`parse_fast_travel_config`] pour construire depuis un JSON désérialisé.
///
/// # Exemple minimal
///
/// ```
/// use nie_data::fast_travel::parse_fast_travel_config;
/// use nie_data::hash::HashId;
/// use serde_json::json;
///
/// let data = json!({
///     "version": 100,
///     "lists": [{
///         "name": "m_fastTravelMapInfoList",
///         "typeName": "FAST_TRAVEL_MAP_INFO",
///         "values": [{
///             "id": "0x7225AA02",
///             "mapId": "0x4E7B90D9",
///             "mapTextId": "0xD20ECFA3",
///             "pos": [0, 0, 45, 0],
///             "charaRotateY": -180
///         }]
///     }]
/// });
/// let cfg = parse_fast_travel_config(&data);
/// assert_eq!(cfg.map_infos.len(), 1);
/// assert_eq!(cfg.map_infos[0].id, HashId(0x7225_AA02));
/// assert_eq!(cfg.map_infos[0].chara_rotate_y, -180.0_f32);
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FastTravelConfig {
    /// Points de voyage rapide (`m_fastTravelMapInfoList`).
    ///
    /// Vérité terrain : 6 entrées dans `fast_travel_config_0.00.00.cfg.bin.json`.
    pub map_infos: Vec<FastTravelMapInfo>,
}

impl FastTravelConfig {
    /// Recherche un point de voyage rapide par son `id`. Renvoie `None` si absent.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::fast_travel::{FastTravelConfig, FastTravelMapInfo};
    /// use nie_data::hash::HashId;
    ///
    /// let cfg = FastTravelConfig {
    ///     map_infos: vec![FastTravelMapInfo {
    ///         id: HashId(0x7225_AA02),
    ///         map_id: HashId(0x4E7B_90D9),
    ///         map_text_id: HashId(0xD20E_CFA3),
    ///         pos: [0.0, 0.0, 45.0, 0.0],
    ///         chara_rotate_y: -180.0,
    ///     }],
    /// };
    /// assert!(cfg.find_by_id(HashId(0x7225_AA02)).is_some());
    /// assert!(cfg.find_by_id(HashId(0xDEAD_BEEF)).is_none());
    /// ```
    #[must_use]
    pub fn find_by_id(&self, id: HashId) -> Option<&FastTravelMapInfo> {
        self.map_infos.iter().find(|m| m.id == id)
    }

    /// Renvoie tous les points de voyage rapide ciblant une carte donnée (`mapId`).
    ///
    /// Vérité terrain : `mapId = 0x4E7B90D9` → 4 points (entrées 0–3 du dump réel).
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::fast_travel::{FastTravelConfig, FastTravelMapInfo};
    /// use nie_data::hash::HashId;
    ///
    /// let cfg = FastTravelConfig {
    ///     map_infos: vec![
    ///         FastTravelMapInfo {
    ///             id: HashId(0x7225_AA02),
    ///             map_id: HashId(0x4E7B_90D9),
    ///             map_text_id: HashId(0xD20E_CFA3),
    ///             pos: [0.0, 0.0, 45.0, 0.0],
    ///             chara_rotate_y: -180.0,
    ///         },
    ///         FastTravelMapInfo {
    ///             id: HashId(0xA3E7_114B),
    ///             map_id: HashId(0x2067_910E),
    ///             map_text_id: HashId(0xF15D_1497),
    ///             pos: [0.0, 0.0, 0.0, 0.0],
    ///             chara_rotate_y: 0.0,
    ///         },
    ///     ],
    /// };
    /// assert_eq!(cfg.find_by_map(HashId(0x4E7B_90D9)).len(), 1);
    /// assert_eq!(cfg.find_by_map(HashId(0xDEAD_BEEF)).len(), 0);
    /// ```
    #[must_use]
    pub fn find_by_map(&self, map_id: HashId) -> Vec<&FastTravelMapInfo> {
        self.map_infos
            .iter()
            .filter(|m| m.map_id == map_id)
            .collect()
    }
}

// ─── Parseur principal ─────────────────────────────────────────────────────────

/// Parse un `fast_travel_config_*.cfg.bin.json` complet (1 liste).
///
/// Renvoie un [`FastTravelConfig`] avec `map_infos` remplie.
/// Les entrées invalides (id nul ou absent) sont silencieusement ignorées.
///
/// Vérité terrain : `fast_travel_config_0.00.00.cfg.bin.json` → 6 entrées.
#[must_use]
pub fn parse_fast_travel_config(root: &Value) -> FastTravelConfig {
    let mut map_infos = Vec::new();
    if let Some(values) = list_values(root, "m_fastTravelMapInfoList") {
        for v in values {
            if let Some(info) = FastTravelMapInfo::from_value(v) {
                map_infos.push(info);
            }
        }
    }
    FastTravelConfig { map_infos }
}
