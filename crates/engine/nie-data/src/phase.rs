//! `PhaseSet` et structures associées — port Rust des familles `phase/*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! Trois sous-familles dans `data/common/gamedata/phase/` :
//!
//! | Sous-famille | Fichiers | Format | Rôle |
//! |---|---|---|---|
//! | `phase_set_c*.cfg.bin.json` | 19 | `entries` plat (noeuds positionnels) | Phases de match par chapitre |
//! | `phase_set_layout_c*.cfg.bin.json` | 9 | `entries` + enfants | Positions terrain 3D |
//! | `phase_title_config_*.cfg.bin.json` | 1 | `entries` + enfants | Textures de titres de chapitre |
//!
//! ## Structure `phase_set_cNN.cfg.bin.json`
//!
//! ### `DATA_ITEM_N` (11 variables)
//!
//! Exemple réel — `phase_set_c01_0.00.00.cfg.bin.json`, `DATA_ITEM_0` :
//! `[1, 10, 0, 1, 0, 0, 0, 0, 0, "AAAAABgFNSo9RUMACgEoAAYCNK6GLSIyAAAAAXg=", 0]`
//! - var\[0\] = 1  → `chapter_no` (numéro de chapitre).
//! - var\[1\] = 10 → `phase_no` (valeurs observées : 10, 13, 15, 20 … 180).
//! - var\[2\] = 0  → `seq_index` (index séquentiel 0-basé croissant).
//! - var\[3\] = 1  → `type_flag` (0 = simple, 1 = avec événement, 2 = dialogue, 3 = spécial).
//! - var\[4\..=6\] = 0 dans tous les fichiers réels — ignorés.
//! - var\[7\]      → `raw_7` (référence non décodée sans source TS).
//! - var\[8\]      → `raw_8` (référence non décodée sans source TS).
//! - var\[9\]      → `lua_path` (base64 si présent, `"0"` si absent).
//! - var\[10\] = 0 dans tous les fichiers réels — ignoré.
//!
//! ### `PURPOSE_TEXT_ITEM_N` (2 variables)
//!
//! Exemple réel — `phase_set_c91_0.00.00.cfg.bin.json`, `PURPOSE_TEXT_ITEM_0` :
//! `[1381738245, 0]`
//! - var\[0\] = 1381738245 → `text_hash` = `HashId(0x525BA705)`.
//! - var\[1\]              → `lua_data` (base64 ou `"0"`).
//!
//! ### `PURPOSE_POS_ITEM_N` (9 variables)
//!
//! Exemple réel — `phase_set_c05_0.00.00.cfg.bin.json`, `PURPOSE_POS_ITEM_0` :
//! `[856450204, 1773916758, 0, 1, 0, 0, 0, 0, 0]`
//! - var\[0\] = 856450204  → `pos_id`      = `HashId(0x330F74DC)`.
//! - var\[1\] = 1773916758 → `target_hash` = `HashId(0x69B24696)`.
//! - var\[2\]              → `lua_data` (base64 ou `"0"`).
//! - var\[3\] = 1          → `active_flag`.
//! - var\[4\..=8\] = 0 dans tous les fichiers réels — ignorés.
//!
//! ## Structure `phase_set_layout_cNN.cfg.bin.json`
//!
//! `LAYOUT_BASE_LIST_BEG_0` (noeud avec enfants) contient `3 × N` enfants groupés :
//! `LAYOUT_BASE_N`, `LAYOUT_BASE_SHAPE_N`, `LAYOUT_BASE_POINT_N`.
//!
//! Exemple réel — `phase_set_layout_c01_0.00.00.cfg.bin.json`, base 0 :
//! - `LAYOUT_BASE_0.var[0]`       = 1339075865 → `base_hash`.
//! - `LAYOUT_BASE_SHAPE_0.var[*]` = `[0, 0, 0, 0]` (tous 0 dans les fichiers réels).
//! - `LAYOUT_BASE_POINT_0.var[*]` = `["22,31", "1,25", "-21,01", "0"]` → `point`.
//!
//! ## Structure `phase_title_config_*.cfg.bin.json`
//!
//! Deux noeuds avec enfants :
//! - `PHASE_TITLE_TEX_INFO_LIST_BEG_0` : 9 enfants `PHASE_TITLE_TEX_INFO_N` (hash_a, hash_b, tex_path).
//! - `PHASE_TITLE_CFG_LIST_BEG_0`      : 18 enfants alternant `PHASE_TITLE_CFG_N` (chapter_no,
//!   title_hash) et `PHASE_TITLE_CFG_REF_TEX_INFO_N` (tex_index).
//!
//! Exemple réel — `phase_title_config_0.08.56.cfg.bin.json` :
//! - `PHASE_TITLE_TEX_INFO_0` : `[-16726348, 667504474, "#/menu/220_img/chapter_title/<LG>/chapter01.g4tx"]`
//!   → `hash_a` = `HashId(0xFF00C6B4)`, `hash_b` = `HashId(0x27C94F5A)`.
//! - `PHASE_TITLE_CFG_0` : `[1, 185538439]` + `PHASE_TITLE_CFG_REF_TEX_INFO_0` : `[0, 1]`
//!   → `chapter_no` = 1, `title_hash` = `HashId(0x0B0F1787)`, `tex_index` = 0.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, owned, walk_named};
use crate::hash::HashId;

// ─── PhaseDataItem ─────────────────────────────────────────────────────────────

/// Entrée `DATA_ITEM_N` — définition d'une phase de match.
///
/// Parsée depuis `phase_set_cNN_*.cfg.bin.json` (format `entries`, variables positionnelles).
///
/// Vérité terrain : `phase_set_c01_0.00.00.cfg.bin.json`, `DATA_ITEM_0` :
/// `[1, 10, 0, 1, 0, 0, 0, 0, 0, "AAAAABgFNSo9RUMACgEoAAYCNK6GLSIyAAAAAXg=", 0]`
/// - var\[0\] = 1  → `chapter_no` = 1.
/// - var\[1\] = 10 → `phase_no`   = 10.
/// - var\[2\] = 0  → `seq_index`  = 0.
/// - var\[3\] = 1  → `type_flag`  = 1 (avec événement).
/// - var\[9\]      → `lua_path`   = base64 du script Lua associé.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhaseDataItem {
    /// var\[0\] — numéro de chapitre (ex. : 1 pour `phase_set_c01`, 90 pour `phase_set_c90`).
    pub chapter_no: i64,
    /// var\[1\] — numéro de phase (10 = intro, 20 = coup d'envoi, 30 = mi-temps…).
    ///
    /// Valeurs observées sur c01 : 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 115, 120,
    /// 130, 140, 150, 160, 170, 180.
    pub phase_no: i64,
    /// var\[2\] — index séquentiel 0-basé croissant dans la liste du fichier.
    pub seq_index: i64,
    /// var\[3\] — indicateur de type.
    ///
    /// Valeurs observées : 0 = sans événement, 1 = avec événement Lua, 2 = dialogue,
    /// 3 = phase spéciale.
    pub type_flag: i64,
    /// var\[7\] — référence non décodée (liée aux textes d'objectif, sémantique sans source TS).
    pub raw_7: i64,
    /// var\[8\] — référence non décodée (liée aux positions d'objectif, sémantique sans source TS).
    pub raw_8: i64,
    /// var\[9\] — chemin script Lua en base64, ou `"0"` si la phase n'a pas de script.
    pub lua_path: String,
}

impl PhaseDataItem {
    /// Parse un noeud `DATA_ITEM_N`. Renvoie `None` si le nom ne commence pas par `"DATA_ITEM_"`.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        if !node.name().starts_with("DATA_ITEM_") {
            return None;
        }
        Some(Self {
            chapter_no: node.int(0),
            phase_no: node.int(1),
            seq_index: node.int(2),
            type_flag: node.int(3),
            raw_7: node.int(7),
            raw_8: node.int(8),
            lua_path: owned(node.string(9)),
        })
    }

    /// `true` si un script Lua est associé à cette phase (`lua_path` ≠ `"0"` et non vide).
    ///
    /// Vérité terrain : `phase_set_c01` — 19 sur 19 phases ont un script Lua ; `phase_set_c21`
    /// — la seule phase n'en a pas (`lua_path = "0"`).
    #[must_use]
    #[inline]
    pub fn has_lua(&self) -> bool {
        self.lua_path != "0" && !self.lua_path.is_empty()
    }
}

// ─── PurposeTextItem ──────────────────────────────────────────────────────────

/// Entrée `PURPOSE_TEXT_ITEM_N` — texte d'objectif associé à une phase.
///
/// Parsée depuis `phase_set_cNN_*.cfg.bin.json`.
///
/// Vérité terrain : `phase_set_c91_0.00.00.cfg.bin.json`, `PURPOSE_TEXT_ITEM_0` :
/// `[1381738245, 0]`
/// - var\[0\] = 1381738245 → `text_hash` = `HashId(0x525BA705)`.
/// - var\[1\] = `"0"`      → `lua_data` absent.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PurposeTextItem {
    /// var\[0\] — hash Level-5 identifiant le texte d'objectif.
    ///
    /// Vérité terrain : `PURPOSE_TEXT_ITEM_0` (c91) = 1381738245 → `HashId(0x525BA705)`.
    pub text_hash: HashId,
    /// var\[1\] — données Lua base64, ou `"0"` si absent.
    pub lua_data: String,
}

impl PurposeTextItem {
    /// Parse un noeud `PURPOSE_TEXT_ITEM_N`.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Self {
        Self {
            text_hash: node.hash(0),
            lua_data: owned(node.string(1)),
        }
    }
}

// ─── PurposePosItem ───────────────────────────────────────────────────────────

/// Entrée `PURPOSE_POS_ITEM_N` — position d'objectif associée à une phase.
///
/// Parsée depuis `phase_set_cNN_*.cfg.bin.json`.
///
/// Vérité terrain : `phase_set_c05_0.00.00.cfg.bin.json`, `PURPOSE_POS_ITEM_0` :
/// `[856450204, 1773916758, 0, 1, 0, 0, 0, 0, 0]`
/// - var\[0\] = 856450204  → `pos_id`      = `HashId(0x330F74DC)`.
/// - var\[1\] = 1773916758 → `target_hash` = `HashId(0x69B24696)`.
/// - var\[2\] = `"0"`      → `lua_data` absent.
/// - var\[3\] = 1          → `active_flag`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PurposePosItem {
    /// var\[0\] — hash identifiant la position d'objectif.
    pub pos_id: HashId,
    /// var\[1\] — hash de la cible associée à cet objectif.
    pub target_hash: HashId,
    /// var\[2\] — données Lua base64, ou `"0"` si absent.
    pub lua_data: String,
    /// var\[3\] — indicateur d'activation (0 ou 1 dans les fichiers réels).
    pub active_flag: i64,
}

impl PurposePosItem {
    /// Parse un noeud `PURPOSE_POS_ITEM_N`.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Self {
        Self {
            pos_id: node.hash(0),
            target_hash: node.hash(1),
            lua_data: owned(node.string(2)),
            active_flag: node.int(3),
        }
    }
}

// ─── PhaseSet ─────────────────────────────────────────────────────────────────

/// Contenu complet d'un fichier `phase_set_cNN_*.cfg.bin.json`.
///
/// Décrit toutes les phases d'un match pour le chapitre `NN`, avec les textes et positions
/// d'objectif associés.
///
/// Utiliser [`parse_phase_set`] pour construire depuis un `serde_json::Value`.
///
/// Comptes réels (vérifiés sur les dumps `data/common/gamedata/phase/`) :
/// - `phase_set_c01` : 19 phases, 27 textes, 38 positions.
/// - `phase_set_c21` :  1 phase,   0 texte,   0 position.
/// - `phase_set_c97` :  3 phases,  0 texte,   0 position.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhaseSet {
    /// Phases du match (`DATA_ITEM_N`), dans l'ordre du fichier.
    pub items: Vec<PhaseDataItem>,
    /// Textes d'objectif (`PURPOSE_TEXT_ITEM_N`), dans l'ordre du fichier.
    pub purpose_texts: Vec<PurposeTextItem>,
    /// Positions d'objectif (`PURPOSE_POS_ITEM_N`), dans l'ordre du fichier.
    pub purpose_positions: Vec<PurposePosItem>,
}

// ─── LayoutBase ───────────────────────────────────────────────────────────────

/// Base de layout terrain — triplet `LAYOUT_BASE_N` + `LAYOUT_BASE_SHAPE_N` +
/// `LAYOUT_BASE_POINT_N`.
///
/// Vérité terrain : `phase_set_layout_c01_0.00.00.cfg.bin.json`, base 0 :
/// - `LAYOUT_BASE_0.var[0]`       = 1339075865  → `base_hash`.
/// - `LAYOUT_BASE_SHAPE_0.var[*]` = `[0, 0, 0, 0]` (tous 0 dans les 9 fichiers réels).
/// - `LAYOUT_BASE_POINT_0.var[*]` = `["22,31", "1,25", "-21,01", "0"]` → `point`.
///
/// Les `Float` cfgbin utilisent la virgule comme séparateur décimal (`"22,31"` → 22.31 f64) —
/// cf. `cfgbin::Var::as_f64` qui remplace la virgule par un point avant `parse`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutBase {
    /// `LAYOUT_BASE_N.var[0]` — hash identifiant la base de layout.
    pub base_hash: HashId,
    /// `LAYOUT_BASE_SHAPE_N.var[0..=3]` — paramètres de forme (tous 0 dans les fichiers réels).
    pub shape: [i64; 4],
    /// `LAYOUT_BASE_POINT_N.var[0..=3]` — coordonnées 3D + w de la position (x, y, z, w).
    pub point: [f64; 4],
}

// ─── PhaseSetLayout ───────────────────────────────────────────────────────────

/// Contenu complet d'un fichier `phase_set_layout_cNN_*.cfg.bin.json`.
///
/// Contient les bases de layout terrain (positions 3D du rendu de la scène de match).
///
/// Utiliser [`parse_phase_set_layout`] pour construire depuis un `serde_json::Value`.
///
/// Comptes réels : c01 et c05 → 3 bases ; c06 et c09 → 2 bases ;
/// c02, c03, c04, c07, c08 → 1 base.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhaseSetLayout {
    /// Bases de layout (`LAYOUT_BASE_LIST_BEG_0` enfants, groupés par 3).
    pub bases: Vec<LayoutBase>,
}

// ─── PhaseTitleTexInfo ────────────────────────────────────────────────────────

/// Entrée `PHASE_TITLE_TEX_INFO_N` — texture du titre d'un chapitre.
///
/// Vérité terrain : `phase_title_config_0.08.56.cfg.bin.json`, `PHASE_TITLE_TEX_INFO_0` :
/// `[-16726348, 667504474, "#/menu/220_img/chapter_title/<LG>/chapter01.g4tx"]`
/// - var\[0\] = -16726348  → `hash_a` = `HashId(0xFF00C6B4)`.
/// - var\[1\] = 667504474  → `hash_b` = `HashId(0x27C94F5A)`.
/// - var\[2\]              → `tex_path` (localisé via `<LG>`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhaseTitleTexInfo {
    /// var\[0\] — hash A de la texture de titre.
    ///
    /// Vérité terrain : `PHASE_TITLE_TEX_INFO_0.var[0]` = -16726348 → `HashId(0xFF00C6B4)`.
    pub hash_a: HashId,
    /// var\[1\] — hash B de la texture de titre.
    ///
    /// Vérité terrain : `PHASE_TITLE_TEX_INFO_0.var[1]` = 667504474 → `HashId(0x27C94F5A)`.
    pub hash_b: HashId,
    /// var\[2\] — chemin VFS vers la texture `.g4tx` (contient `<LG>` pour la localisation).
    ///
    /// Exemple : `"#/menu/220_img/chapter_title/<LG>/chapter01.g4tx"`.
    pub tex_path: String,
}

impl PhaseTitleTexInfo {
    /// Parse un noeud `PHASE_TITLE_TEX_INFO_N`.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Self {
        Self {
            hash_a: node.hash(0),
            hash_b: node.hash(1),
            tex_path: owned(node.string(2)),
        }
    }
}

// ─── PhaseTitleCfg ────────────────────────────────────────────────────────────

/// Paire `PHASE_TITLE_CFG_N` + `PHASE_TITLE_CFG_REF_TEX_INFO_N` — config de titre de chapitre.
///
/// Vérité terrain : `phase_title_config_0.08.56.cfg.bin.json`, paire 0 :
/// - `PHASE_TITLE_CFG_0`              = `[1, 185538439]`
///   → `chapter_no` = 1, `title_hash` = `HashId(0x0B0F1787)`.
/// - `PHASE_TITLE_CFG_REF_TEX_INFO_0` = `[0, 1]`
///   → `tex_index` = 0 (référence à `PhaseTitleConfig::tex_infos[0]`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhaseTitleCfg {
    /// `PHASE_TITLE_CFG_N.var[0]` — numéro de chapitre (1-basé, 1 à 9 dans le dump réel).
    pub chapter_no: i64,
    /// `PHASE_TITLE_CFG_N.var[1]` — hash Level-5 du titre de chapitre.
    ///
    /// Vérité terrain : chapitre 1 → 185538439 → `HashId(0x0B0F1787)`.
    pub title_hash: HashId,
    /// `PHASE_TITLE_CFG_REF_TEX_INFO_N.var[0]` — index dans [`PhaseTitleConfig::tex_infos`].
    ///
    /// Dans le dump réel, `tex_index[n]` = n (correspondance directe).
    pub tex_index: i64,
}

// ─── PhaseTitleConfig ─────────────────────────────────────────────────────────

/// Contenu complet de `phase_title_config_*.cfg.bin.json`.
///
/// Associe chaque chapitre (1-9) à sa texture de titre et son hash de titre.
///
/// Utiliser [`parse_phase_title_config`] pour construire depuis un `serde_json::Value`.
///
/// Comptes réels : 9 [`PhaseTitleTexInfo`] + 9 [`PhaseTitleCfg`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhaseTitleConfig {
    /// Textures de titre de chapitre (9 dans le dump réel, une par chapitre).
    pub tex_infos: Vec<PhaseTitleTexInfo>,
    /// Configs de chapitre (9 dans le dump réel, `chapter_no` 1 à 9).
    pub cfgs: Vec<PhaseTitleCfg>,
}

impl PhaseTitleConfig {
    /// Trouve la config de titre pour un numéro de chapitre donné. `None` si absent.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::phase::{PhaseTitleConfig, PhaseTitleCfg};
    /// use nie_data::hash::HashId;
    ///
    /// let config = PhaseTitleConfig {
    ///     tex_infos: vec![],
    ///     cfgs: vec![
    ///         PhaseTitleCfg { chapter_no: 1, title_hash: HashId(0x0B0F1787), tex_index: 0 },
    ///     ],
    /// };
    /// assert!(config.find_chapter(1).is_some());
    /// assert!(config.find_chapter(99).is_none());
    /// ```
    #[must_use]
    pub fn find_chapter(&self, chapter_no: i64) -> Option<&PhaseTitleCfg> {
        self.cfgs.iter().find(|c| c.chapter_no == chapter_no)
    }
}

// ─── Parseurs principaux ──────────────────────────────────────────────────────

/// Parse un `phase_set_cNN_*.cfg.bin.json` complet.
///
/// Collecte les noeuds `DATA_ITEM_N`, `PURPOSE_TEXT_ITEM_N` et `PURPOSE_POS_ITEM_N`
/// via un parcours DFS des `entries`.
///
/// Comptes réels :
/// - `phase_set_c01` : 19 items, 27 textes, 38 positions.
/// - `phase_set_c21` :  1 item,   0 texte,   0 position.
/// - `phase_set_c97` :  3 items,  0 texte,   0 position.
#[must_use]
pub fn parse_phase_set(root: &Value) -> PhaseSet {
    let mut items = Vec::new();
    let mut purpose_texts = Vec::new();
    let mut purpose_positions = Vec::new();

    walk_named(root, "DATA_ITEM_", |node| {
        if let Some(item) = PhaseDataItem::from_node(node) {
            items.push(item);
        }
    });
    walk_named(root, "PURPOSE_TEXT_ITEM_", |node| {
        purpose_texts.push(PurposeTextItem::from_node(node));
    });
    walk_named(root, "PURPOSE_POS_ITEM_", |node| {
        purpose_positions.push(PurposePosItem::from_node(node));
    });

    PhaseSet {
        items,
        purpose_texts,
        purpose_positions,
    }
}

/// Parse un `phase_set_layout_cNN_*.cfg.bin.json` complet.
///
/// Lit le noeud `LAYOUT_BASE_LIST_BEG_0` et traite ses enfants en groupes de 3
/// (`LAYOUT_BASE_N`, `LAYOUT_BASE_SHAPE_N`, `LAYOUT_BASE_POINT_N`).
///
/// Comptes réels : c01 et c05 → 3 bases ; c06 et c09 → 2 bases ;
/// c02, c03, c04, c07, c08 → 1 base.
#[must_use]
pub fn parse_phase_set_layout(root: &Value) -> PhaseSetLayout {
    let mut bases = Vec::new();

    walk_named(root, "LAYOUT_BASE_LIST_BEG_", |list_node| {
        let children = list_node.children();
        let mut i = 0;
        while i + 2 < children.len() {
            let base_node = children[i];
            let shape_node = children[i + 1];
            let point_node = children[i + 2];
            bases.push(LayoutBase {
                base_hash: base_node.hash(0),
                shape: [
                    shape_node.int(0),
                    shape_node.int(1),
                    shape_node.int(2),
                    shape_node.int(3),
                ],
                point: [
                    point_node.var(0).map_or(0.0_f64, |v| v.as_f64()),
                    point_node.var(1).map_or(0.0_f64, |v| v.as_f64()),
                    point_node.var(2).map_or(0.0_f64, |v| v.as_f64()),
                    point_node.var(3).map_or(0.0_f64, |v| v.as_f64()),
                ],
            });
            i += 3;
        }
    });

    PhaseSetLayout { bases }
}

/// Parse le `phase_title_config_*.cfg.bin.json` complet.
///
/// Lit `PHASE_TITLE_TEX_INFO_LIST_BEG_0` (9 textures) et `PHASE_TITLE_CFG_LIST_BEG_0`
/// (9 paires `CFG` + `REF_TEX_INFO`).
///
/// Comptes réels (fichier unique `phase_title_config_0.08.56.cfg.bin.json`) :
/// 9 [`PhaseTitleTexInfo`] + 9 [`PhaseTitleCfg`].
#[must_use]
pub fn parse_phase_title_config(root: &Value) -> PhaseTitleConfig {
    let mut tex_infos = Vec::new();
    let mut cfgs = Vec::new();

    walk_named(root, "PHASE_TITLE_TEX_INFO_LIST_BEG_", |list_node| {
        for child in list_node.children() {
            if child.name().starts_with("PHASE_TITLE_TEX_INFO_") {
                tex_infos.push(PhaseTitleTexInfo::from_node(child));
            }
        }
    });

    walk_named(root, "PHASE_TITLE_CFG_LIST_BEG_", |list_node| {
        let children = list_node.children();
        let mut i = 0;
        while i + 1 < children.len() {
            let cfg_node = children[i];
            let ref_node = children[i + 1];
            // PHASE_TITLE_CFG_N (pas REF) suivi de PHASE_TITLE_CFG_REF_TEX_INFO_N
            if cfg_node.name().starts_with("PHASE_TITLE_CFG_") && !cfg_node.name().contains("REF") {
                cfgs.push(PhaseTitleCfg {
                    chapter_no: cfg_node.int(0),
                    title_hash: cfg_node.hash(1),
                    tex_index: ref_node.int(0),
                });
            }
            i += 2;
        }
    });

    PhaseTitleConfig { tex_infos, cfgs }
}

// ─── Tests unitaires ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn phase_data_item_from_node_valide() {
        let raw = json!({
            "name": "DATA_ITEM_0",
            "variables": [
                {"type": "Int", "value": "1"},
                {"type": "Int", "value": "10"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "1"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "String", "value": "AAAAABgFNSo9RUMACgEoAAYCNK6GLSIyAAAAAXg="},
                {"type": "Int", "value": "0"}
            ],
            "children": []
        });
        let node = Node::new(&raw);
        let item = PhaseDataItem::from_node(node).expect("DATA_ITEM_0 valide");
        assert_eq!(item.chapter_no, 1);
        assert_eq!(item.phase_no, 10);
        assert_eq!(item.seq_index, 0);
        assert_eq!(item.type_flag, 1);
        assert!(item.has_lua(), "lua_path non-zero → has_lua");
    }

    #[test]
    fn phase_data_item_from_node_mauvais_prefixe() {
        let raw = json!({
            "name": "DATA_COUNT_0",
            "variables": [{"type": "Int", "value": "1"}],
            "children": []
        });
        let node = Node::new(&raw);
        assert!(
            PhaseDataItem::from_node(node).is_none(),
            "DATA_COUNT_0 ne doit pas être parsé comme DATA_ITEM"
        );
    }

    #[test]
    fn phase_data_item_sans_lua() {
        let raw = json!({
            "name": "DATA_ITEM_0",
            "variables": [
                {"type": "Int", "value": "21"},
                {"type": "Int", "value": "10"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"}
            ],
            "children": []
        });
        let node = Node::new(&raw);
        let item = PhaseDataItem::from_node(node).unwrap();
        assert!(!item.has_lua(), "lua_path='0' → pas de lua");
    }
}
