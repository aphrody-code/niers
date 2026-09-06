//! `SuperTacticsConfig` — port de `super_tactics_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! Variante **base / pré-DLC** des super-tactiques (l'équipe d'Endou). La variante DLC
//! Orion vit dans `special_tactics_config` (parser distinct, non couvert ici).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/skill/super_tactics_config_0.08.86.cfg.bin`
//!   (extrait du VFS Steam : `INAZUMA ELEVEN Victory Road/data`).
//! - Format **T2B `entries`** (noeuds nommés à variables positionnelles, suffixées `_<i>`
//!   par occurrence — même convention iecode que `boost_grp.rs` / `command.rs`).
//! - Port 1:1 de `packages/inagle/src/parsers/super-tactics-base-config.ts` (`parseEntries`),
//!   enrichi : inagle agrège `SUPER_TACTICS_INFO_*` et `SUPER_TACTICS_INFO_REF_EFFECT_*`
//!   dans une même liste générique `tactics` (var0..varN) ; on les sépare proprement en
//!   trois listes (`infos`, `ref_effects`, `sort_index`) car ce sont trois rôles distincts.
//!
//! ## Deux listes racine
//!
//! | Noeud racine | Enfants | Variables/enfant | Rôle |
//! |--------------|---------|------------------|------|
//! | `SUPER_TACTICS_EFFECT_LIST_BEG_*` | 11 `SUPER_TACTICS_EFFECT_*` | 9 Int | `effectId` (CRC) + 8 conditions |
//! | `SUPER_TACTICS_INFO_LIST_BEG_*`   | 6 `SUPER_TACTICS_INFO_*`, 6 `SUPER_TACTICS_INFO_REF_EFFECT_*`, 6 `__SORT_INDEX_*` | 6 / 2 / 1 Int | infos, liens info→effets, ordre de tri |
//!
//! La sentinelle « pas de condition » vaut `-992181094` (= `0xC4DC849A`) : filtrée comme
//! dans inagle (`NULL_SENTINEL`).
//!
//! ## Pattern ref-list (info → effets)
//!
//! Chaque `SUPER_TACTICS_INFO_REF_EFFECT_*` est un couple `[offset, count]` qui pointe une
//! tranche de la liste `effects`. Vérité terrain : les couples
//! `[0,2] [2,2] [4,2] [6,3] [9,1] [10,1]` se chaînent et couvrent exactement les 11 effets
//! (0→2, 2→4, 4→6, 6→9, 9→10, 10→11). Résolution via [`SuperTacticsConfig::effects_of`].

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

/// Sentinelle « condition absente » (`0xC4DC849A` en non signé) — filtrée à l'identique
/// d'inagle (`const NULL_SENTINEL = -992181094`).
pub const NULL_SENTINEL: i64 = -992_181_094;

// ─── SuperTacticsEffect ────────────────────────────────────────────────────────

/// Un effet de super-tactique (`SUPER_TACTICS_EFFECT_*`, 9 Int).
///
/// `var[0]` = identifiant CRC de l'effet ; `var[1..=8]` = jusqu'à 8 conditions
/// (valeurs/CRC), la sentinelle [`NULL_SENTINEL`] marquant les emplacements vides.
///
/// Vérité terrain (`super_tactics_config_0.08.86`) :
/// - `SUPER_TACTICS_EFFECT_0` : `[672742448, sentinelle ×8]` → effet `0x28193C30`, 0 condition.
/// - `SUPER_TACTICS_EFFECT_2` : `[-1352230702, 20, sentinelle ×7]` → `0xAF6698D2`, condition `0x00000014`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuperTacticsEffect {
    /// `var[0]` — hash CRC identifiant l'effet appliqué par la super-tactique.
    pub effect_id: HashId,
    /// `var[1..]` — conditions non sentinelles, dans l'ordre du dump (souvent 0 ou 1).
    pub conditions: Vec<HashId>,
}

impl SuperTacticsEffect {
    /// Parse un noeud `SUPER_TACTICS_EFFECT_*`. `None` si le noeud n'a aucune variable
    /// (`vars.length < 1` côté inagle).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let count = node.var_count();
        if count == 0 {
            return None;
        }
        let effect_id = node.hash(0);
        let mut conditions = Vec::new();
        for i in 1..count {
            let raw = node.int(i);
            if raw != NULL_SENTINEL {
                conditions.push(HashId::from_i64(raw));
            }
        }
        Some(Self {
            effect_id,
            conditions,
        })
    }
}

// ─── SuperTacticsInfo ──────────────────────────────────────────────────────────

/// Définition d'une super-tactique (`SUPER_TACTICS_INFO_*`, 6 Int).
///
/// `var[0]` est l'identifiant de la tactique (celui qu'inagle ré-exporte via `toHex(var0)`).
/// `var[1..=3]` sont trois CRC apparentés (libellé / description / commande liée — rôle
/// exact non confirmé, conservés bruts pour rester bit-exact). `var[4]` et `var[5]` sont
/// deux petits entiers (coût/jauge et catégorie, d'après les valeurs observées).
///
/// Vérité terrain :
/// - `SUPER_TACTICS_INFO_0` : `[1209890895, 1237356186, -1408544942, -580331975, 30, 0]`
///   → id `0x481D784F`.
/// - `SUPER_TACTICS_INFO_3` : `[947358912, 967473685, -597574691, 1147242371, 60, 2]`
///   → id `0x38778CC0`, `value=60`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuperTacticsInfo {
    /// `var[0]` — hash CRC identifiant la super-tactique.
    pub tactics_id: HashId,
    /// `var[1..=3]` — trois CRC apparentés (sémantique non confirmée, gardés bruts).
    pub extra_hashes: [HashId; 3],
    /// `var[4]` — entier (probable coût/jauge : 30/60/10 dans le dump).
    pub value: i64,
    /// `var[5]` — petit entier de catégorie (0/1/2 dans le dump).
    pub kind: i64,
}

impl SuperTacticsInfo {
    /// Parse un noeud `SUPER_TACTICS_INFO_*` (6 Int). `None` si moins de 6 variables.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        if node.var_count() < 6 {
            return None;
        }
        Some(Self {
            tactics_id: node.hash(0),
            extra_hashes: [node.hash(1), node.hash(2), node.hash(3)],
            value: node.int(4),
            kind: node.int(5),
        })
    }
}

// ─── SuperTacticsInfoRefEffect ─────────────────────────────────────────────────

/// Lien info → effets (`SUPER_TACTICS_INFO_REF_EFFECT_*`, 2 Int).
///
/// `[effect_offset, effect_count]` désigne une tranche de la liste `effects`. Le i-ème
/// `ref_effect` correspond au i-ème `info` (même ordre dans le dump).
///
/// Vérité terrain : `[0,2] [2,2] [4,2] [6,3] [9,1] [10,1]` (chaînage couvrant les 11 effets).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuperTacticsInfoRefEffect {
    /// `var[0]` — index de début dans la liste `effects`.
    pub effect_offset: i64,
    /// `var[1]` — nombre d'effets de cette super-tactique.
    pub effect_count: i64,
}

impl SuperTacticsInfoRefEffect {
    /// Parse un noeud `SUPER_TACTICS_INFO_REF_EFFECT_*`. `None` si `effect_count` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let effect_count = node.int(1);
        if effect_count == 0 {
            return None;
        }
        Some(Self {
            effect_offset: node.int(0),
            effect_count,
        })
    }
}

// ─── SuperTacticsConfig ────────────────────────────────────────────────────────

/// Contenu complet d'un `super_tactics_config_*.cfg.bin.json` (variante base / pré-DLC).
///
/// Construire via [`parse_super_tactics`].
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuperTacticsConfig {
    /// 11 effets (`SUPER_TACTICS_EFFECT_*`), dans l'ordre du dump.
    pub effects: Vec<SuperTacticsEffect>,
    /// 6 super-tactiques (`SUPER_TACTICS_INFO_*`).
    pub infos: Vec<SuperTacticsInfo>,
    /// 6 liens info→effets (`SUPER_TACTICS_INFO_REF_EFFECT_*`), alignés sur `infos`.
    pub ref_effects: Vec<SuperTacticsInfoRefEffect>,
    /// 6 indices d'ordre de tri (`__SORT_INDEX_*`).
    pub sort_index: Vec<i64>,
}

impl SuperTacticsConfig {
    /// Renvoie les effets de la i-ème super-tactique (tranche de `self.effects` désignée
    /// par le `ref_effect` de même rang). `&[]` si l'index est hors borne ou la plage déborde.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::super_tactics::{
    ///     SuperTacticsConfig, SuperTacticsEffect, SuperTacticsInfoRefEffect,
    /// };
    /// use nie_data::hash::HashId;
    ///
    /// let cfg = SuperTacticsConfig {
    ///     effects: vec![
    ///         SuperTacticsEffect { effect_id: HashId(0x28193C30), conditions: vec![] },
    ///         SuperTacticsEffect { effect_id: HashId(0xB8A621A1), conditions: vec![] },
    ///         SuperTacticsEffect { effect_id: HashId(0xAF6698D2), conditions: vec![HashId(20)] },
    ///     ],
    ///     infos: vec![],
    ///     ref_effects: vec![SuperTacticsInfoRefEffect { effect_offset: 0, effect_count: 2 }],
    ///     sort_index: vec![],
    /// };
    /// let slice = cfg.effects_of(0);
    /// assert_eq!(slice.len(), 2);
    /// assert_eq!(slice[0].effect_id, HashId(0x28193C30));
    /// assert!(cfg.effects_of(9).is_empty()); // index hors borne
    /// ```
    #[must_use]
    pub fn effects_of(&self, info_index: usize) -> &[SuperTacticsEffect] {
        let Some(r) = self.ref_effects.get(info_index) else {
            return &[];
        };
        let start = r.effect_offset as usize;
        let end = start.saturating_add(r.effect_count as usize);
        self.effects.get(start..end).unwrap_or(&[])
    }

    /// Recherche une super-tactique par son `tactics_id`. `None` si absente.
    #[must_use]
    pub fn find_info(&self, tactics_id: HashId) -> Option<&SuperTacticsInfo> {
        self.infos.iter().find(|i| i.tactics_id == tactics_id)
    }
}

// ─── Parseur principal ─────────────────────────────────────────────────────────

/// Parse un `super_tactics_config_*.cfg.bin.json` complet (forme iecode `entries`).
///
/// Comptes réels (`super_tactics_config_0.08.86`) : 11 effets, 6 infos, 6 ref_effects,
/// 6 sort_index. Les noeuds `*_LIST_BEG_*` ne servent que de conteneurs et ne sont pas
/// matérialisés.
#[must_use]
pub fn parse_super_tactics(root: &Value) -> SuperTacticsConfig {
    let mut cfg = SuperTacticsConfig::default();

    // Liste des effets : enfants directs de SUPER_TACTICS_EFFECT_LIST_BEG_*.
    walk_named(root, "SUPER_TACTICS_EFFECT_LIST_BEG", |list| {
        for child in list.children() {
            let name = child.name();
            if !name.starts_with("SUPER_TACTICS_EFFECT_") || name.contains("_LIST_") {
                continue;
            }
            if let Some(effect) = SuperTacticsEffect::from_node(child) {
                cfg.effects.push(effect);
            }
        }
    });

    // Liste des infos + ref_effects + sort_index : enfants de SUPER_TACTICS_INFO_LIST_BEG_*.
    walk_named(root, "SUPER_TACTICS_INFO_LIST_BEG", |list| {
        for child in list.children() {
            let name = child.name();
            // REF_EFFECT en premier : son nom commence aussi par "SUPER_TACTICS_INFO_".
            if name.starts_with("SUPER_TACTICS_INFO_REF_EFFECT") {
                if let Some(rf) = SuperTacticsInfoRefEffect::from_node(child) {
                    cfg.ref_effects.push(rf);
                }
            } else if name.starts_with("SUPER_TACTICS_INFO_") && !name.contains("_LIST_") {
                if let Some(info) = SuperTacticsInfo::from_node(child) {
                    cfg.infos.push(info);
                }
            } else if name.starts_with("__SORT_INDEX") {
                cfg.sort_index.push(child.int(0));
            }
        }
    });

    cfg
}
