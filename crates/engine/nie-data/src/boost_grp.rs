//! `BoostGrpConfig` — port de `boost_player_group_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `/home/ubuntu/niers/data/common/gamedata/boost_grp/boost_player_group_config_0.00.00.cfg.bin.json`
//! - Format **`entries`** (noeuds nommés, variables positionnelles) — même convention que `command.rs`.
//! - 3 listes dans le fichier, identifiées par leur préfixe de noeud :
//!
//! | Préfixe | Entrées | Variables | Sémantique |
//! |---------|---------|-----------|-----------|
//! | `BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_*` | 4 | 2 | table de sprites par niveau de boost |
//! | `BOOST_PLAYER_GRP_CONFIG_*` | 5 | 6 | configs de rotation temporelle |
//! | `BOOST_PLAYER_GRP_INFO_*` | 1 | 1 | identifiant du groupe |
//! | `BOOST_PLAYER_GRP_INFO_REF_CONFIG_*` | 1 | 2 | plage de configs du groupe |
//!
//! Note : "SPRIT" est la typo Level-5 (le E final de "SPRITE" manque) — conservée à l'identique
//! pour rester byte-exact avec les dumps.
//!
//! ## Valeurs réelles observées
//!
//! `BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_0` : `[0, -1341563653]`
//! → table_index=0, sprite_hash=0xB0095CFB
//!
//! `BOOST_PLAYER_GRP_CONFIG_0` : `[1814400, 0, 0, 1, 2, 3]`
//! → duration=1814400 (≈ 21 jours en secondes), config_type=0, table_refs=[0,1,2,3]
//!
//! `BOOST_PLAYER_GRP_INFO_0` : `[1762786800]`
//! → group_hash=0x6911FDF0
//!
//! `BOOST_PLAYER_GRP_INFO_REF_CONFIG_0` : `[0, 5]`
//! → config_offset=0, config_count=5 (toutes les configs appartiennent à ce groupe)

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

// ─── BoostSpritTableInfo ──────────────────────────────────────────────────────

/// Entrée de table de sprites de boost (`BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_*`).
///
/// Associe un indice séquentiel (0–3) à un hash de ressource sprite pour le rendu du
/// niveau de boost d'un joueur dans le groupe.
///
/// Vérité terrain :
/// - `BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_0` : var\[0\]=0, var\[1\]=-1341563653 → 0xB0095CFB
/// - `BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_3` : var\[0\]=3, var\[1\]=1372344836 → 0x51CC5204
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoostSpritTableInfo {
    /// var\[0\] — index séquentiel de la table (0–3 dans le dump réel).
    pub table_index: i64,
    /// var\[1\] — hash identifiant la ressource sprite pour ce niveau de boost.
    pub sprite_hash: HashId,
}

impl BoostSpritTableInfo {
    /// Parse un noeud `BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_*`.
    /// Renvoie `None` si `table_index` et `sprite_hash` sont tous deux nuls.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let table_index = node.int(0);
        let sprite_hash = node.hash(1);
        if table_index == 0 && sprite_hash.is_zero() {
            return None;
        }
        Some(Self {
            table_index,
            sprite_hash,
        })
    }
}

// ─── BoostPlayerGrpConfig ─────────────────────────────────────────────────────

/// Configuration temporelle d'un groupe de boost (`BOOST_PLAYER_GRP_CONFIG_*`).
///
/// Définit une durée, un type de configuration, et 4 références aux indices de
/// [`BoostSpritTableInfo`] à afficher pendant cette phase.
///
/// Vérité terrain (`boost_player_group_config_0.00.00.cfg.bin.json`) :
/// - `BOOST_PLAYER_GRP_CONFIG_0` : `[1814400, 0, 0, 1, 2, 3]`
///   → duration=1814400, config_type=0, table_refs=[0,1,2,3]
/// - `BOOST_PLAYER_GRP_CONFIG_1` : `[1209600, 1, 1, 2, 3, 0]`
///   → duration=1209600, config_type=1, table_refs=[1,2,3,0]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoostPlayerGrpConfig {
    /// var\[0\] — durée/seuil en unité interne (1814400 ≈ 21 jours en secondes ; 1209600 ≈ 14 jours).
    pub duration: i64,
    /// var\[1\] — type de configuration (0 = état initial, 1 = rotation).
    pub config_type: i64,
    /// var\[2..=5\] — 4 références séquentielles aux indices de `sprit_table` (valeurs 0–3).
    pub table_refs: [i64; 4],
}

impl BoostPlayerGrpConfig {
    /// Parse un noeud `BOOST_PLAYER_GRP_CONFIG_*`.
    /// Renvoie `None` si `duration` est nul (entier guard minimal).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let duration = node.int(0);
        if duration == 0 {
            return None;
        }
        Some(Self {
            duration,
            config_type: node.int(1),
            table_refs: [node.int(2), node.int(3), node.int(4), node.int(5)],
        })
    }
}

// ─── BoostPlayerGrpInfo ───────────────────────────────────────────────────────

/// Identifiant d'un groupe de boost (`BOOST_PLAYER_GRP_INFO_*`).
///
/// Dans le dump réel, un seul groupe est défini.
///
/// Vérité terrain :
/// `BOOST_PLAYER_GRP_INFO_0` : var\[0\]=1762786800 → group_hash=0x6911FDF0
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoostPlayerGrpInfo {
    /// var\[0\] — hash identifiant unique du groupe de boost.
    pub group_hash: HashId,
}

impl BoostPlayerGrpInfo {
    /// Parse un noeud `BOOST_PLAYER_GRP_INFO_*` (hors `_REF_CONFIG_`). `None` si hash nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let group_hash = node.hash(0);
        if group_hash.is_zero() {
            return None;
        }
        Some(Self { group_hash })
    }
}

// ─── BoostPlayerGrpInfoRefConfig ─────────────────────────────────────────────

/// Référence aux configs d'un groupe de boost (`BOOST_PLAYER_GRP_INFO_REF_CONFIG_*`).
///
/// `[config_offset, config_count]` référence la plage dans la liste `configs` —
/// utiliser [`BoostGrpConfig::configs_of`] pour la résoudre.
///
/// Vérité terrain :
/// `BOOST_PLAYER_GRP_INFO_REF_CONFIG_0` : `[0, 5]`
/// → config_offset=0, config_count=5 (toutes les configs du dump appartiennent à ce groupe)
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoostPlayerGrpInfoRefConfig {
    /// var\[0\] — index de début dans la liste `configs`.
    pub config_offset: i64,
    /// var\[1\] — nombre de configs pour ce groupe.
    pub config_count: i64,
}

impl BoostPlayerGrpInfoRefConfig {
    /// Parse un noeud `BOOST_PLAYER_GRP_INFO_REF_CONFIG_*`. `None` si `config_count` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let config_count = node.int(1);
        if config_count == 0 {
            return None;
        }
        Some(Self {
            config_offset: node.int(0),
            config_count,
        })
    }
}

// ─── BoostGrpConfig ───────────────────────────────────────────────────────────

/// Contenu complet d'un `boost_player_group_config_*.cfg.bin.json`.
///
/// Utiliser [`parse_boost_grp_config`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoostGrpConfig {
    /// 4 entrées de table de sprites (`BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_*`).
    pub sprit_table: Vec<BoostSpritTableInfo>,
    /// 5 configs de rotation (`BOOST_PLAYER_GRP_CONFIG_*`).
    pub configs: Vec<BoostPlayerGrpConfig>,
    /// 1 groupe de boost (`BOOST_PLAYER_GRP_INFO_*`).
    pub groups: Vec<BoostPlayerGrpInfo>,
    /// 1 référence de configs (`BOOST_PLAYER_GRP_INFO_REF_CONFIG_*`).
    pub ref_configs: Vec<BoostPlayerGrpInfoRefConfig>,
}

impl BoostGrpConfig {
    /// Renvoie les configs d'un groupe (tranche dans `self.configs`).
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::boost_grp::{
    ///     BoostGrpConfig, BoostPlayerGrpConfig, BoostPlayerGrpInfo,
    ///     BoostPlayerGrpInfoRefConfig, BoostSpritTableInfo,
    /// };
    /// use nie_data::hash::HashId;
    ///
    /// let cfg = BoostGrpConfig {
    ///     sprit_table: vec![],
    ///     configs: vec![
    ///         BoostPlayerGrpConfig { duration: 1814400, config_type: 0, table_refs: [0,1,2,3] },
    ///     ],
    ///     groups: vec![BoostPlayerGrpInfo { group_hash: HashId(0x6911FDF0) }],
    ///     ref_configs: vec![BoostPlayerGrpInfoRefConfig { config_offset: 0, config_count: 1 }],
    /// };
    /// let slice = cfg.configs_of(&cfg.ref_configs[0]);
    /// assert_eq!(slice.len(), 1);
    /// assert_eq!(slice[0].duration, 1814400);
    /// ```
    #[must_use]
    pub fn configs_of(&self, ref_cfg: &BoostPlayerGrpInfoRefConfig) -> &[BoostPlayerGrpConfig] {
        let start = ref_cfg.config_offset as usize;
        let end = start.saturating_add(ref_cfg.config_count as usize);
        self.configs.get(start..end).unwrap_or(&[])
    }

    /// Recherche un groupe par son `group_hash`. `None` si absent.
    #[must_use]
    pub fn find_group(&self, group_hash: HashId) -> Option<&BoostPlayerGrpInfo> {
        self.groups.iter().find(|g| g.group_hash == group_hash)
    }
}

// ─── Parseur principal ────────────────────────────────────────────────────────

/// Parse un `boost_player_group_config_*.cfg.bin.json` complet.
///
/// Renvoie un [`BoostGrpConfig`] avec les quatre listes remplies. Les noeuds
/// `*_LIST_BEG_*` sont silencieusement ignorés.
///
/// Comptes réels (`boost_player_group_config_0.00.00.cfg.bin.json`) :
/// - 4 `BOOST_PLAYER_GRP_SPRIT_TABLE_INFO`
/// - 5 `BOOST_PLAYER_GRP_CONFIG`
/// - 1 `BOOST_PLAYER_GRP_INFO`
/// - 1 `BOOST_PLAYER_GRP_INFO_REF_CONFIG`
#[must_use]
pub fn parse_boost_grp_config(root: &Value) -> BoostGrpConfig {
    let mut sprit_table = Vec::new();
    walk_named(root, "BOOST_PLAYER_GRP_SPRIT_TABLE_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(info) = BoostSpritTableInfo::from_node(node) {
            sprit_table.push(info);
        }
    });

    let mut configs = Vec::new();
    walk_named(root, "BOOST_PLAYER_GRP_CONFIG_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(cfg) = BoostPlayerGrpConfig::from_node(node) {
            configs.push(cfg);
        }
    });

    let mut groups = Vec::new();
    let mut ref_configs = Vec::new();
    walk_named(root, "BOOST_PLAYER_GRP_INFO_", |node| {
        let name = node.name();
        if name.contains("_LIST_") {
            return;
        }
        if name.contains("_REF_CONFIG_") {
            if let Some(rc) = BoostPlayerGrpInfoRefConfig::from_node(node) {
                ref_configs.push(rc);
            }
        } else if let Some(grp) = BoostPlayerGrpInfo::from_node(node) {
            groups.push(grp);
        }
    });

    BoostGrpConfig {
        sprit_table,
        configs,
        groups,
        ref_configs,
    }
}
