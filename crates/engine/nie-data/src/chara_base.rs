//! `chara_base` — table maîtresse des personnages (`character/chara_base_*.cfg.bin`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/chara-base.ts`
//!   (`parseBaseNode`, `parseAllCharaBase`) — **port 1:1**.
//! - Données : `data/common/gamedata/character/chara_base_1.*.cfg.bin` (VFS IEVR), format
//!   **T2B** (`entries`). Noeuds `CHARA_BASE_INFO_<i>` (et `CHARA_BASE_BATTLE_<i>`) dans un dump
//!   JSON inagle (le dumper JS suffixe chaque enfant par son index) ; sur le T2B binaire LU EN
//!   DIRECT (`nie_explore::bridge::t2b_to_json`, pont vérifié sur un fichier réel), les enfants
//!   n'ont PAS ce suffixe (`CHARA_BASE_INFO`/`CHARA_BASE_BATTLE` bruts, répétés) — les deux
//!   formes sont acceptées par le filtre interne `is_chara_base_node`.
//!
//! ## Disposition des variables (positions découvertes par inagle)
//!
//! - `[0]` = `charaId` (Int, **obligatoire**) — identifiant unique → hash.
//! - `[1]` = `internalCode` (String, **obligatoire**) — code interne (`c01000100`, `npc0010`…).
//! - `[3]` = `nameHash` (Int, **obligatoire**) — hash du prénom → `chara_text`.
//! - `[4]` = `lastNameHash` (Int, optionnel ≠ 0) — hash du nom de famille.
//! - `[11]` = `gender` (Int, défaut **1**) — 1 = masculin, 2 = féminin.
//! - `[15]` = `seriesId` (Int, optionnel ≠ 0) → `charaSeriesInfo`.
//! - `[16]` = `belongTeamId` (Int, optionnel ≠ 0) → `belongTeamInfo`.
//! - `[19]` = `descriptionHash` (Int, optionnel ≠ 0) → `chara_description_text`.
//!
//! Un noeud avec moins de 4 variables, ou dont `[0]`≠Int / `[1]`≠String / `[3]`≠Int, est rejeté
//! (`None`). Le champ `rawVariables` d'inagle (vidage de débogage) n'est pas porté.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

/// Une entrée `CHARA_BASE_INFO` portée (port de `ParsedCharaBase`, hors `rawVariables`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaBase {
    /// `[0]` — identifiant unique du personnage (hash).
    pub chara_id: HashId,
    /// `[1]` — code interne (`c01000100`, `npc0010`, …).
    pub internal_code: String,
    /// `[3]` — hash du prénom / nom d'affichage (→ `chara_text`).
    pub name_hash: HashId,
    /// `[4]` — hash du nom de famille (`None` si absent ou nul).
    pub last_name_hash: Option<HashId>,
    /// `[19]` — hash de la description (`None` si absent ou nul).
    pub description_hash: Option<HashId>,
    /// `[11]` — genre (1 = masculin, 2 = féminin ; défaut 1).
    pub gender: i64,
    /// `[15]` — identifiant de série (`None` si absent ou nul).
    pub series_id: Option<HashId>,
    /// `[16]` — identifiant d'équipe d'appartenance (`None` si absent ou nul).
    pub belong_team_id: Option<HashId>,
}

/// Vrai si le nom de noeud correspond à `CHARA_BASE_(INFO|BATTLE)_<digits>` (regex `^CHARA_BASE_
/// (INFO|BATTLE)_\d+$` d'inagle — forme du dump JSON historique, où le sérialiseur JS suffixe
/// chaque enfant d'un même nom par son index) **ou** au nom brut `CHARA_BASE_(INFO|BATTLE)` SANS
/// suffixe — la forme réelle des enfants dans le T2B binaire (vérifié octet sur `chara_base_
/// 1.03.98.00.cfg.bin` réel : 14448 enfants `CHARA_BASE_INFO` / 5898 `CHARA_BASE_BATTLE`, aucun
/// suffixe `_<i>` — le suffixe est un artefact du dumper JS, pas du format T2B). Les deux formes
/// sont acceptées pour rester correct que la source soit un dump JSON inagle ou le T2B en direct.
fn is_chara_base_node(name: &str) -> bool {
    for base in ["CHARA_BASE_INFO", "CHARA_BASE_BATTLE"] {
        if name == base {
            return true;
        }
        if let Some(rest) = name.strip_prefix(base).and_then(|r| r.strip_prefix('_')) {
            return !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit());
        }
    }
    false
}

/// Lit une variable `Int` optionnelle à l'index `i` comme hash, `None` si absente, de type ≠ `Int`
/// ou de valeur nulle (réplique de `type==="Int" && parseInt!==0`).
fn opt_int_hash(node: &Node<'_>, i: usize) -> Option<HashId> {
    let v = node.var(i)?;
    if v.ty != "Int" {
        return None;
    }
    let h = v.as_hash();
    (h != HashId::ZERO).then_some(h)
}

/// Parse un noeud `CHARA_BASE_INFO` en [`CharaBase`] (port de `parseBaseNode`).
#[must_use]
pub fn parse_base_node(node: &Node<'_>) -> Option<CharaBase> {
    if node.var_count() < 4 {
        return None;
    }
    let v0 = node.var(0)?;
    if v0.ty != "Int" {
        return None;
    }
    let v1 = node.var(1)?;
    if v1.ty != "String" {
        return None;
    }
    let v3 = node.var(3)?;
    if v3.ty != "Int" {
        return None;
    }

    // `[11]` gender : Int si présent, sinon défaut 1.
    let gender = node
        .var(11)
        .filter(|v| v.ty == "Int")
        .map_or(1, |v| v.as_i64());

    Some(CharaBase {
        chara_id: v0.as_hash(),
        internal_code: v1.value.into(),
        name_hash: v3.as_hash(),
        last_name_hash: opt_int_hash(node, 4),
        description_hash: opt_int_hash(node, 19),
        gender,
        series_id: opt_int_hash(node, 15),
        belong_team_id: opt_int_hash(node, 16),
    })
}

/// Parse toutes les entrées `CHARA_BASE_INFO`/`CHARA_BASE_BATTLE` d'un `chara_base_*.cfg.bin.json`
/// désérialisé (port de `parseAllCharaBase`).
#[must_use]
pub fn parse_all_chara_base(root: &Value) -> Vec<CharaBase> {
    let mut out = Vec::new();
    walk_named(root, "CHARA_BASE_", |node| {
        if is_chara_base_node(node.name())
            && let Some(b) = parse_base_node(&node)
        {
            out.push(b);
        }
    });
    out
}

/// Recherche une entrée par `charaId` (port de l'accès `buildCharaBaseMap().get`).
#[must_use]
pub fn find_by_chara_id(bases: &[CharaBase], chara_id: HashId) -> Option<&CharaBase> {
    bases.iter().find(|b| b.chara_id == chara_id)
}

/// Résout le **prénom** d'un personnage via les noms `chara_text` (issus de
/// [`crate::chara_text::parse_all_nouns`]).
///
/// Port de la jointure `chara_base.nameHash → chara_text` (« name hash -> chara_text (first
/// name) », `chara-base.ts`). `name_hash` = `[3]`. `None` si non résolu.
#[must_use]
pub fn resolve_first_name<'a>(
    base: &CharaBase,
    nouns: &'a [crate::chara_text::ParsedNoun],
) -> Option<&'a str> {
    crate::chara_text::find_name(nouns, base.name_hash)
}

/// Résout le **nom de famille** d'un personnage via `chara_text` (`lastNameHash` = `[4]`).
///
/// `None` si absent (`last_name_hash` nul) ou non résolu.
#[must_use]
pub fn resolve_last_name<'a>(
    base: &CharaBase,
    nouns: &'a [crate::chara_text::ParsedNoun],
) -> Option<&'a str> {
    base.last_name_hash
        .and_then(|h| crate::chara_text::find_name(nouns, h))
}

/// Résout la **description** d'un personnage via `chara_description` (`descriptionHash` = `[19]`).
///
/// `None` si absent (`description_hash` nul) ou non résolu.
#[must_use]
pub fn resolve_description<'a>(
    base: &CharaBase,
    descriptions: &'a [crate::chara_description::ParsedDescription],
) -> Option<&'a str> {
    base.description_hash
        .and_then(|h| crate::chara_description::find_by_hash(descriptions, h))
}

/// Résout le **nom de série FR** d'un personnage : `series_id` (`[15]`) → `chara_series` → type →
/// nom codé en dur (port de `buildSeriesMapping`). `None` si série absente ou type hors `1..=9`.
#[must_use]
pub fn resolve_series_name_fr(
    base: &CharaBase,
    series: &[crate::chara_series::CharaSeriesInfo],
) -> Option<&'static str> {
    let s = crate::chara_series::find_by_id(series, base.series_id?)?;
    crate::chara_series::series_name_fr(s.series_type)
}
