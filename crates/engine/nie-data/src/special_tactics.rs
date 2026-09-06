//! `SpecialTacticsConfig` — port de `special_tactics_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! Variante **DLC Orion** des super-tactiques (les « Tactiques Spéciales »), distincte de
//! la variante base portée dans [`crate::super_tactics`]. Activées à l'échelle de l'équipe
//! pendant un match, elles ajoutent un système de **conditions** (`COND_ID`) et de
//! **conditions de réussite** (`SUCCESS_COND_ID`) absent de la variante base.
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/skill/special_tactics_config_1.04.09.10.cfg.bin`
//!   (extrait du VFS Steam `INAZUMA ELEVEN Victory Road/data` via l'exemple jetable
//!   `nie-game/examples/extract_special_tactics.rs`, depuis supprimé).
//! - Format **T2B `entries`** (noeuds nommés à variables positionnelles, suffixées `_<i>`
//!   par occurrence — même convention iecode que `super_tactics.rs` / `boost_grp.rs`).
//! - Port 1:1 de `packages/inagle/src/parsers/special-tactics-config.ts` (`parseEntries`).
//!   Comme pour `super_tactics.rs`, on **sépare** proprement infos et liens de référence
//!   en listes parallèles (alignées par rang) là où inagle agrège les refs dans la tactique.
//!   La résolution de texte (noms/descriptions via `item_text`) reste hors `nie-data`.
//!
//! ## Quatre listes racine + une liste de tri
//!
//! | Noeud racine | Enfants (dump réel) | Variables/enfant | Rôle |
//! |--------------|---------------------|------------------|------|
//! | `SPECIAL_TACTICS_EFFECT_LIST_BEG_*`            | 192 `SPECIAL_TACTICS_EFFECT_*`            | 9 Int        | `effectId` (CRC) + `power` + 7 conditions |
//! | `SPECIAL_TACTICS_COND_ID_LIST_BEG_*`           | 126 `SPECIAL_TACTICS_COND_ID_*`           | Int + String | `condId` + données base64 |
//! | `SPECIAL_TACTICS_SUCCESS_COND_ID_LIST_BEG_*`   | 3 `SPECIAL_TACTICS_SUCCESS_COND_ID_*`     | Int + String | `condId` + données base64 |
//! | `SPECIAL_TACTICS_INFO_LIST_BEG_*`              | 86 `SPECIAL_TACTICS_INFO_<n>` + 86 de chaque `..._REF_EFFECT_*` / `..._REF_COND_ID_*` / `..._REF_SUCCESS_COND_ID_*` + 86 `__SORT_INDEX_*` | 17 / 2 / 2 / 2 / 1 | tactiques, liens, ordre de tri |
//!
//! La sentinelle « pas de valeur » vaut `-992181094` (= `0xC4DC849A`) : filtrée comme dans
//! inagle (`NULL_SENTINEL`). Le hash nul `0x00000000` filtre les emplacements de partenaire vides.
//!
//! ## Pattern ref-list (tactique → tranches)
//!
//! Chaque tactique `SPECIAL_TACTICS_INFO_<n>` est immédiatement suivie de trois couples
//! `[offset, count]` (`REF_EFFECT`, `REF_COND_ID`, `REF_SUCCESS_COND_ID`) qui désignent des
//! tranches des listes correspondantes. Le i-ème lien correspond à la i-ème tactique (même
//! ordre dans le dump). Vérité terrain : les `REF_EFFECT` se chaînent et couvrent exactement
//! les 192 effets (`[0,3] [3,2] [5,2] … [191,1]`). Résolution via [`SpecialTacticsConfig::effects_of`],
//! [`SpecialTacticsConfig::cond_ids_of`], [`SpecialTacticsConfig::success_cond_ids_of`].

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, owned, walk_named};
use crate::hash::HashId;

/// Sentinelle « valeur absente » (`0xC4DC849A` en non signé) — filtrée à l'identique
/// d'inagle (`const NULL_SENTINEL = -992181094`).
pub const NULL_SENTINEL: i64 = -992_181_094;

/// Nom français de l'élément d'une tactique (`ELEMENT_MAP` d'inagle).
///
/// `0` Néant, `1` Vent, `2` Forêt, `3` Feu, `4` Montagne ; toute autre valeur → `"Néant"`.
#[must_use]
pub fn element_name_fr(element: i64) -> &'static str {
    match element {
        1 => "Vent",
        2 => "Forêt",
        3 => "Feu",
        4 => "Montagne",
        _ => "Néant",
    }
}

// ─── SpecialTacticsEffect ────────────────────────────────────────────────────────

/// Un effet de tactique spéciale (`SPECIAL_TACTICS_EFFECT_*`, 9 Int).
///
/// `var[0]` = identifiant CRC de l'effet ; `var[1]` = puissance ; `var[2..=8]` = jusqu'à 7
/// conditions (valeurs/CRC), la sentinelle [`NULL_SENTINEL`] marquant les emplacements vides.
/// La puissance, elle, n'est **pas** filtrée (un effet peut légitimement avoir
/// `power == NULL_SENTINEL`, cf. `SPECIAL_TACTICS_EFFECT_28` du dump).
///
/// Vérité terrain (`special_tactics_config_1.04.09.10`) :
/// - `SPECIAL_TACTICS_EFFECT_0` : `[1477956381, 40, sentinelle ×7]` → effet `0x5817D31D`, power 40, 0 condition.
/// - `SPECIAL_TACTICS_EFFECT_28` : `[-1704204520, sentinelle, 1178143904, 10, sentinelle ×5]`
///   → `0x9A6BE718`, power = sentinelle, 2 conditions (`0x46390CA0`, `0x0000000A`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecialTacticsEffect {
    /// `var[0]` — hash CRC identifiant l'effet appliqué par la tactique.
    pub effect_id: HashId,
    /// `var[1]` — puissance de l'effet (brute, non filtrée de la sentinelle).
    pub power: i64,
    /// `var[2..]` — conditions non sentinelles, dans l'ordre du dump.
    pub conditions: Vec<HashId>,
}

impl SpecialTacticsEffect {
    /// Parse un noeud `SPECIAL_TACTICS_EFFECT_*`. `None` si moins de 9 variables
    /// (`vars.length < 9` côté inagle).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        if node.var_count() < 9 {
            return None;
        }
        let effect_id = node.hash(0);
        let power = node.int(1);
        let mut conditions = Vec::new();
        for i in 2..9 {
            let raw = node.int(i);
            if raw != NULL_SENTINEL {
                conditions.push(HashId::from_i64(raw));
            }
        }
        Some(Self {
            effect_id,
            power,
            conditions,
        })
    }
}

// ─── SpecialTacticsCondId ────────────────────────────────────────────────────────

/// Une condition (`SPECIAL_TACTICS_COND_ID_*`) ou condition de réussite
/// (`SPECIAL_TACTICS_SUCCESS_COND_ID_*`) : un identifiant entier + des données encodées
/// en base64 (script de condition opaque, conservé brut).
///
/// Vérité terrain : `SPECIAL_TACTICS_COND_ID_0` : `[1103, "AAAAAA8FNeHe3UoAAQAyAAAAAHg="]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecialTacticsCondId {
    /// `var[0]` — identifiant numérique de la condition.
    pub cond_id: i64,
    /// `var[1]` — données de condition encodées en base64 (opaques).
    pub cond_data: String,
}

impl SpecialTacticsCondId {
    /// Parse un noeud `SPECIAL_TACTICS_(SUCCESS_)COND_ID_*`. `None` si moins de 2 variables.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        if node.var_count() < 2 {
            return None;
        }
        Some(Self {
            cond_id: node.int(0),
            cond_data: owned(node.string(1)),
        })
    }
}

// ─── SpecialTacticsInfo ──────────────────────────────────────────────────────────

/// Définition d'une tactique spéciale (`SPECIAL_TACTICS_INFO_<n>`, 17 variables ; les 10
/// premières sont lues, comme inagle).
///
/// `var[0]` identifiant, `var[1]` code interne (`"wht10010"`…), `var[2]`/`var[3]` hash de
/// libellé/description (résolus hors `nie-data`), `var[4]` puissance, `var[5]` temps de
/// recharge, `var[6]` élément (`0..=4`), `var[7..=9]` jusqu'à 3 partenaires (CRC ; les
/// emplacements à `0` ou [`NULL_SENTINEL`] sont filtrés). `var[10..=16]` sont des drapeaux
/// réservés non lus par inagle (parfois un blob base64), volontairement ignorés ici.
///
/// Vérité terrain :
/// - `SPECIAL_TACTICS_INFO_0` : id `0x43D8B1AC`, code `"wht10010"`, power 40, recast 90,
///   élément 0 (Néant), 3 partenaires.
/// - `SPECIAL_TACTICS_INFO_12` : code `"wht20040"`, 2 partenaires à `0` filtrés → 1 partenaire.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecialTacticsInfo {
    /// `var[0]` — hash CRC identifiant la tactique.
    pub tactics_id: HashId,
    /// `var[1]` — code interne (ex. `"wht10010"`).
    pub internal_code: String,
    /// `var[2]` — hash CRC du libellé localisé.
    pub name_text_id: HashId,
    /// `var[3]` — hash CRC de la description localisée.
    pub desc_text_id: HashId,
    /// `var[4]` — puissance.
    pub power: i64,
    /// `var[5]` — temps de recharge (en ticks de jeu).
    pub recast_time: i64,
    /// `var[6]` — élément (`0`=Néant, `1`=Vent, `2`=Forêt, `3`=Feu, `4`=Montagne).
    pub element: i64,
    /// `var[7..=9]` — partenaires (CRC) non vides, dans l'ordre du dump (`0`/sentinelle filtrés).
    pub partner_ids: Vec<HashId>,
}

impl SpecialTacticsInfo {
    /// Parse un noeud `SPECIAL_TACTICS_INFO_<n>`. `None` si moins de 10 variables
    /// (`vars.length < 10` côté inagle).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let count = node.var_count();
        if count < 10 {
            return None;
        }
        let mut partner_ids = Vec::new();
        for i in 7..=9 {
            if i >= count {
                break;
            }
            let raw = node.int(i);
            if raw != 0 && raw != NULL_SENTINEL {
                partner_ids.push(HashId::from_i64(raw));
            }
        }
        Some(Self {
            tactics_id: node.hash(0),
            internal_code: owned(node.string(1)),
            name_text_id: node.hash(2),
            desc_text_id: node.hash(3),
            power: node.int(4),
            recast_time: node.int(5),
            element: node.int(6),
            partner_ids,
        })
    }

    /// Nom français de l'élément de la tactique (`element_name_fr(self.element)`).
    #[must_use]
    pub fn element_name(&self) -> &'static str {
        element_name_fr(self.element)
    }
}

// ─── SpecialTacticsRef ───────────────────────────────────────────────────────────

/// Lien tactique → tranche (`SPECIAL_TACTICS_INFO_REF_{EFFECT,COND_ID,SUCCESS_COND_ID}_*`,
/// 2 Int) : `[offset, count]` désigne une tranche de la liste correspondante. Le i-ème lien
/// d'un type donné correspond à la i-ème tactique. Un couple `[0, 0]` signifie « aucune
/// tranche » (fréquent pour les conditions).
///
/// Vérité terrain : `REF_EFFECT_0` = `[0, 3]`, `REF_SUCCESS_COND_ID_9` = `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecialTacticsRef {
    /// `var[0]` — index de début dans la liste cible.
    pub offset: i64,
    /// `var[1]` — nombre d'éléments de cette tranche (`0` = aucun).
    pub count: i64,
}

impl SpecialTacticsRef {
    /// Parse un noeud `..._REF_*`. `None` si moins de 2 variables (préserve l'alignement :
    /// tous les refs du dump réel ont exactement 2 variables, donc aucun n'est sauté).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        if node.var_count() < 2 {
            return None;
        }
        Some(Self {
            offset: node.int(0),
            count: node.int(1),
        })
    }

    /// `true` si la tranche est vide (`count == 0`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

// ─── SpecialTacticsConfig ────────────────────────────────────────────────────────

/// Contenu complet d'un `special_tactics_config_*.cfg.bin.json` (variante DLC Orion).
///
/// Construire via [`parse_special_tactics`]. Les listes `infos`, `ref_effects`,
/// `ref_cond_ids`, `ref_success_cond_ids` et `sort_index` sont **alignées par rang** :
/// l'élément `i` de chacune décrit la même tactique.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecialTacticsConfig {
    /// `SPECIAL_TACTICS_EFFECT_*`, dans l'ordre du dump (192 dans le dump réel).
    pub effects: Vec<SpecialTacticsEffect>,
    /// `SPECIAL_TACTICS_COND_ID_*` (126 dans le dump réel).
    pub cond_ids: Vec<SpecialTacticsCondId>,
    /// `SPECIAL_TACTICS_SUCCESS_COND_ID_*` (3 dans le dump réel).
    pub success_cond_ids: Vec<SpecialTacticsCondId>,
    /// `SPECIAL_TACTICS_INFO_<n>` (86 dans le dump réel).
    pub infos: Vec<SpecialTacticsInfo>,
    /// `SPECIAL_TACTICS_INFO_REF_EFFECT_*`, aligné sur `infos`.
    pub ref_effects: Vec<SpecialTacticsRef>,
    /// `SPECIAL_TACTICS_INFO_REF_COND_ID_*`, aligné sur `infos`.
    pub ref_cond_ids: Vec<SpecialTacticsRef>,
    /// `SPECIAL_TACTICS_INFO_REF_SUCCESS_COND_ID_*`, aligné sur `infos`.
    pub ref_success_cond_ids: Vec<SpecialTacticsRef>,
    /// `__SORT_INDEX_*` (ordre d'affichage), aligné sur `infos`.
    pub sort_index: Vec<i64>,
}

/// Résout une tranche `[offset, count]` d'une liste générique. `&[]` si l'index de tactique
/// est hors borne, si la tranche est vide ou si elle déborde.
fn slice_of<'a, T>(items: &'a [T], refs: &[SpecialTacticsRef], info_index: usize) -> &'a [T] {
    let Some(r) = refs.get(info_index) else {
        return &[];
    };
    let start = r.offset as usize;
    let end = start.saturating_add(r.count as usize);
    items.get(start..end).unwrap_or(&[])
}

impl SpecialTacticsConfig {
    /// Effets de la i-ème tactique (tranche de `self.effects` désignée par `ref_effects[i]`).
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::special_tactics::{
    ///     SpecialTacticsConfig, SpecialTacticsEffect, SpecialTacticsRef,
    /// };
    /// use nie_data::hash::HashId;
    ///
    /// let cfg = SpecialTacticsConfig {
    ///     effects: vec![
    ///         SpecialTacticsEffect { effect_id: HashId(0x5817D31D), power: 40, conditions: vec![] },
    ///         SpecialTacticsEffect { effect_id: HashId(0xAF000001), power: 5, conditions: vec![] },
    ///         SpecialTacticsEffect { effect_id: HashId(0xAF000002), power: 25, conditions: vec![] },
    ///     ],
    ///     ref_effects: vec![SpecialTacticsRef { offset: 0, count: 3 }],
    ///     ..Default::default()
    /// };
    /// let slice = cfg.effects_of(0);
    /// assert_eq!(slice.len(), 3);
    /// assert_eq!(slice[0].effect_id, HashId(0x5817D31D));
    /// assert!(cfg.effects_of(9).is_empty()); // index hors borne
    /// ```
    #[must_use]
    pub fn effects_of(&self, info_index: usize) -> &[SpecialTacticsEffect] {
        slice_of(&self.effects, &self.ref_effects, info_index)
    }

    /// Conditions de la i-ème tactique (tranche de `self.cond_ids` via `ref_cond_ids[i]`).
    #[must_use]
    pub fn cond_ids_of(&self, info_index: usize) -> &[SpecialTacticsCondId] {
        slice_of(&self.cond_ids, &self.ref_cond_ids, info_index)
    }

    /// Conditions de réussite de la i-ème tactique (via `ref_success_cond_ids[i]`).
    #[must_use]
    pub fn success_cond_ids_of(&self, info_index: usize) -> &[SpecialTacticsCondId] {
        slice_of(
            &self.success_cond_ids,
            &self.ref_success_cond_ids,
            info_index,
        )
    }

    /// Recherche une tactique par son `tactics_id`. `None` si absente.
    #[must_use]
    pub fn find_info(&self, tactics_id: HashId) -> Option<&SpecialTacticsInfo> {
        self.infos.iter().find(|i| i.tactics_id == tactics_id)
    }
}

// ─── Parseur principal ─────────────────────────────────────────────────────────

/// Parse un `special_tactics_config_*.cfg.bin.json` complet (forme iecode `entries`).
///
/// Comptes réels (`special_tactics_config_1.04.09.10`) : 192 effets, 126 conditions,
/// 3 conditions de réussite, 86 tactiques (et 86 de chaque liste de refs + sort_index).
/// Les noeuds `*_LIST_BEG_*` ne servent que de conteneurs et ne sont pas matérialisés.
#[must_use]
pub fn parse_special_tactics(root: &Value) -> SpecialTacticsConfig {
    let mut cfg = SpecialTacticsConfig::default();

    // Liste des effets.
    walk_named(root, "SPECIAL_TACTICS_EFFECT_LIST_BEG", |list| {
        for child in list.children() {
            if !child.name().starts_with("SPECIAL_TACTICS_EFFECT_")
                || child.name().contains("_LIST_")
            {
                continue;
            }
            if let Some(effect) = SpecialTacticsEffect::from_node(child) {
                cfg.effects.push(effect);
            }
        }
    });

    // Liste des conditions de réussite : préfixe plus spécifique, traité avant les conditions
    // « simples » pour éviter toute confusion de préfixe.
    walk_named(root, "SPECIAL_TACTICS_SUCCESS_COND_ID_LIST_BEG", |list| {
        for child in list.children() {
            if !child.name().starts_with("SPECIAL_TACTICS_SUCCESS_COND_ID_")
                || child.name().contains("_LIST_")
            {
                continue;
            }
            if let Some(c) = SpecialTacticsCondId::from_node(child) {
                cfg.success_cond_ids.push(c);
            }
        }
    });

    // Liste des conditions « simples ».
    walk_named(root, "SPECIAL_TACTICS_COND_ID_LIST_BEG", |list| {
        for child in list.children() {
            let name = child.name();
            if !name.starts_with("SPECIAL_TACTICS_COND_ID_") || name.contains("_LIST_") {
                continue;
            }
            if let Some(c) = SpecialTacticsCondId::from_node(child) {
                cfg.cond_ids.push(c);
            }
        }
    });

    // Liste des tactiques + leurs refs entrelacés + sort_index.
    walk_named(root, "SPECIAL_TACTICS_INFO_LIST_BEG", |list| {
        for child in list.children() {
            let name = child.name();
            if name.starts_with("SPECIAL_TACTICS_INFO_REF_EFFECT") {
                if let Some(r) = SpecialTacticsRef::from_node(child) {
                    cfg.ref_effects.push(r);
                }
            } else if name.starts_with("SPECIAL_TACTICS_INFO_REF_SUCCESS_COND_ID") {
                if let Some(r) = SpecialTacticsRef::from_node(child) {
                    cfg.ref_success_cond_ids.push(r);
                }
            } else if name.starts_with("SPECIAL_TACTICS_INFO_REF_COND_ID") {
                if let Some(r) = SpecialTacticsRef::from_node(child) {
                    cfg.ref_cond_ids.push(r);
                }
            } else if is_info_node(name) {
                if let Some(info) = SpecialTacticsInfo::from_node(child) {
                    cfg.infos.push(info);
                }
            } else if name.starts_with("__SORT_INDEX") {
                cfg.sort_index.push(child.int(0));
            }
        }
    });

    cfg
}

/// `true` si `name` matche `^SPECIAL_TACTICS_INFO_\d+$` (tactique, pas un noeud de ref).
fn is_info_node(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("SPECIAL_TACTICS_INFO_") else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}
