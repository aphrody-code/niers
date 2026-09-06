//! `LightOverwriteConfig` — port de `light_overwrite_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! Dump réel : `/home/ubuntu/niers/data/common/gamedata/light/light_overwrite_config_1.03.21.00.cfg.bin.json`
//!
//! Le fichier utilise le format **`entries`** (noeuds nommés, variables positionnelles), comme
//! `command/*.cfg.bin.json` — pas de champs nommés (`lists`).
//!
//! Deux listes dans ce fichier :
//! - **20 `LIGHT_OVERWRITE_PARAM_*`** — paramètres d'éclairage nommés par hash. Chaque entrée
//!   porte 1 à 3 valeurs (entières ou flottantes selon le type de paramètre). Exemples :
//!   - `PARAM_0` : `param_id`=7531368, `values`=[Int(400)] — paramètre entier simple.
//!   - `PARAM_2` : `param_id`=-1510910904, `values`=[Float(0.85), Float(0.82), Float(0.63)] — triplet RGB.
//! - **3 `LIGHT_OVERWRITE_INFO_*`** chacun suivi de son **`LIGHT_OVERWRITE_INFO_REF_PARAM_N`** :
//!   l'`info_id` identifie le contexte d'éclairage ; le `REF_PARAM` donne `(offset, count)`
//!   dans la liste de params.
//!
//! Comptes réels (version `1.03.21.00`) : 20 params, 3 infos.
//!   - INFO_0 → offset=0, count=9
//!   - INFO_1 → offset=9, count=9
//!   - INFO_2 → offset=18, count=2

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

// ─── LightVal ─────────────────────────────────────────────────────────────────

/// Valeur d'un paramètre d'éclairage — entière ou flottante.
///
/// Le type est conservé depuis le champ `"type"` du dump JSON (`"Int"` ou `"Float"`).
///
/// Vérité terrain : dans `light_overwrite_config_1.03.21.00`, `PARAM_0` a `Int(400)`,
/// `PARAM_1` a `Float(0.7)`, `PARAM_2` a trois flottants `[0.85, 0.82, 0.63]`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LightVal {
    /// Valeur entière signée (`"type": "Int"` dans le dump).
    Int(i64),
    /// Valeur flottante (`"type": "Float"` dans le dump, décimale virgule convertie en point).
    Float(f32),
}

impl LightVal {
    /// Renvoie la valeur comme `f64`.
    ///
    /// Les entiers sont convertis sans perte pour les petites valeurs (< 2^53).
    #[must_use]
    pub fn as_f64(&self) -> f64 {
        match *self {
            LightVal::Int(i) => i as f64,
            LightVal::Float(f) => f64::from(f),
        }
    }

    /// Renvoie la valeur entière, ou la troncature si Float.
    #[must_use]
    pub fn as_int(&self) -> i64 {
        match *self {
            LightVal::Int(i) => i,
            #[allow(clippy::cast_possible_truncation)]
            LightVal::Float(f) => f as i64,
        }
    }

    /// `true` si la variante est `Float`.
    #[must_use]
    pub fn is_float(&self) -> bool {
        matches!(self, LightVal::Float(_))
    }
}

// ─── LightOverwriteParam ──────────────────────────────────────────────────────

/// Paramètre d'éclairage nommé (`LIGHT_OVERWRITE_PARAM_*`).
///
/// Chaque entrée associe un `param_id` (hash identifiant le type de paramètre) à
/// 1 à 3 valeurs, entières ou flottantes selon le type.
///
/// ## Vérité terrain
///
/// `light_overwrite_config_1.03.21.00.cfg.bin.json` :
/// - `LIGHT_OVERWRITE_PARAM_0` :
///   `var[0]`=7531368 (`param_id`), `var[1]`=Int(400).
/// - `LIGHT_OVERWRITE_PARAM_1` :
///   `var[0]`=-1910140030, `var[1]`=Float(0.7).
/// - `LIGHT_OVERWRITE_PARAM_2` :
///   `var[0]`=-1510910904, `var[1]`=Float(0.85), `var[2]`=Float(0.82), `var[3]`=Float(0.63).
/// - `LIGHT_OVERWRITE_PARAM_7` :
///   `var[0]`=-2124741719, `var[1]`=Float(-0.05).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightOverwriteParam {
    /// `var[0]` — identifiant du type de paramètre d'éclairage (hash 32 bits).
    pub param_id: HashId,
    /// `var[1..]` — valeurs du paramètre (1 à 3 selon le type).
    pub values: Vec<LightVal>,
}

impl LightOverwriteParam {
    /// Parse un noeud `LIGHT_OVERWRITE_PARAM_*`.
    ///
    /// Renvoie `None` si le noeud ne contient aucune variable. Les noeuds conteneurs
    /// (`_LIST_BEG_*`) doivent être filtrés avant appel (voir [`parse_light_overwrite_config`]).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        if node.var_count() == 0 {
            return None;
        }
        let param_id = node.hash(0);
        let mut values = Vec::new();
        for i in 1..node.var_count() {
            if let Some(var) = node.var(i) {
                let val = if var.ty == "Float" {
                    #[allow(clippy::cast_possible_truncation)]
                    LightVal::Float(var.as_f64() as f32)
                } else {
                    LightVal::Int(var.as_i64())
                };
                values.push(val);
            }
        }
        Some(Self { param_id, values })
    }
}

// ─── LightOverwriteInfo ───────────────────────────────────────────────────────

/// Référence à un groupe de paramètres d'éclairage.
///
/// Regroupe les données de deux noeuds consécutifs dans le fichier :
/// - `LIGHT_OVERWRITE_INFO_N` → `info_id` (hash du contexte).
/// - `LIGHT_OVERWRITE_INFO_REF_PARAM_N` → `(param_offset, param_count)`.
///
/// ## Vérité terrain
///
/// `light_overwrite_config_1.03.21.00.cfg.bin.json`, liste `LIGHT_OVERWRITE_INFO_LIST_BEG_0` :
/// - `INFO_0` : `var[0]`=896315108, `REF_PARAM_0` : `var[0]`=0, `var[1]`=9.
/// - `INFO_1` : `var[0]`=-1402601634, `REF_PARAM_1` : `var[0]`=9, `var[1]`=9.
/// - `INFO_2` : `var[0]`=-614281272, `REF_PARAM_2` : `var[0]`=18, `var[1]`=2.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightOverwriteInfo {
    /// `LIGHT_OVERWRITE_INFO_N.var[0]` — identifiant du contexte d'éclairage.
    pub info_id: HashId,
    /// `LIGHT_OVERWRITE_INFO_REF_PARAM_N.var[0]` — index de début dans la liste de params.
    pub param_offset: i64,
    /// `LIGHT_OVERWRITE_INFO_REF_PARAM_N.var[1]` — nombre de params associés.
    pub param_count: i64,
}

// ─── LightOverwriteConfig ─────────────────────────────────────────────────────

/// Contenu complet d'un `light_overwrite_config_*.cfg.bin.json`.
///
/// Utiliser [`parse_light_overwrite_config`] pour construire depuis un JSON désérialisé.
///
/// Comptes réels (version `1.03.21.00`) : 20 params, 3 infos.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightOverwriteConfig {
    /// Paramètres d'éclairage (`LIGHT_OVERWRITE_PARAM_*`), 20 entrées dans le dump réel.
    pub params: Vec<LightOverwriteParam>,
    /// Références aux groupes de paramètres (3 entrées dans le dump réel).
    pub infos: Vec<LightOverwriteInfo>,
}

impl LightOverwriteConfig {
    /// Renvoie les paramètres référencés par une `info` (tranche dans `self.params`).
    ///
    /// Retourne `&[]` si la plage `(param_offset, param_count)` dépasse la liste.
    #[must_use]
    pub fn params_of(&self, info: &LightOverwriteInfo) -> &[LightOverwriteParam] {
        let start = info.param_offset as usize;
        let end = start.saturating_add(info.param_count as usize);
        self.params.get(start..end).unwrap_or(&[])
    }

    /// Recherche une info par son `info_id`. `None` si absente.
    #[must_use]
    pub fn find_info(&self, info_id: HashId) -> Option<&LightOverwriteInfo> {
        self.infos.iter().find(|i| i.info_id == info_id)
    }
}

// ─── Parseur principal ────────────────────────────────────────────────────────

/// Parse un `light_overwrite_config_*.cfg.bin.json` complet.
///
/// Renvoie un [`LightOverwriteConfig`] avec les 20 params et 3 infos du fichier réel.
/// Les noeuds conteneurs (`*_LIST_BEG_*`) sont ignorés. Les noeuds `LIGHT_OVERWRITE_INFO_*`
/// et `LIGHT_OVERWRITE_INFO_REF_PARAM_*` sont fusionnés par position.
#[must_use]
pub fn parse_light_overwrite_config(root: &Value) -> LightOverwriteConfig {
    // Collecte des LIGHT_OVERWRITE_PARAM_* (hors noeuds conteneurs _LIST_BEG_*)
    let mut params = Vec::new();
    walk_named(root, "LIGHT_OVERWRITE_PARAM_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(p) = LightOverwriteParam::from_node(node) {
            params.push(p);
        }
    });

    // Collecte des LIGHT_OVERWRITE_INFO_* (hors _LIST_BEG_* et _REF_PARAM_*)
    let mut info_ids: Vec<HashId> = Vec::new();
    walk_named(root, "LIGHT_OVERWRITE_INFO_", |node| {
        let name = node.name();
        if name.contains("_LIST_") || name.contains("_REF_PARAM_") {
            return;
        }
        info_ids.push(node.hash(0));
    });

    // Collecte des LIGHT_OVERWRITE_INFO_REF_PARAM_* (offset + count)
    let mut ref_params: Vec<(i64, i64)> = Vec::new();
    walk_named(root, "LIGHT_OVERWRITE_INFO_REF_PARAM_", |node| {
        ref_params.push((node.int(0), node.int(1)));
    });

    // Fusion INFO_N ↔ REF_PARAM_N par position
    let infos = info_ids
        .into_iter()
        .zip(ref_params)
        .map(
            |(info_id, (param_offset, param_count))| LightOverwriteInfo {
                info_id,
                param_offset,
                param_count,
            },
        )
        .collect();

    LightOverwriteConfig { params, infos }
}
