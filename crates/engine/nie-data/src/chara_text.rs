//! `chara_text` — noms de personnage localisés (`common/text/<locale>/chara_text.cfg.bin`,
//! + variante romanisée `chara_text_roma`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/chara-text.ts`
//!   (`parseNounNode`, `parseNounsFromConfig`, `parseRomaNode`, `buildRomanizedMap`) — **port 1:1**.
//! - Données : `data/common/text/<locale>/chara_text.cfg.bin` (VFS IEVR), format **T2B**
//!   (`entries`). Noeuds `NOUN_INFO_<i>` dans un dump JSON inagle (le dumper JS suffixe chaque
//!   enfant par son index ; le noeud `NOUN_INFO_BEGIN` est exclu). Sur le T2B binaire LU EN
//!   DIRECT (`nie_explore::bridge::t2b_to_json`, vérifié sur `chara_text.cfg.bin` réel — racine
//!   `NOUN_INFO_BEGIN` + 21081 enfants), les enfants sont nommés `NOUN_INFO` SANS suffixe : les
//!   deux formes sont acceptées (préfixe de parcours élargi à `"NOUN_INFO"`, filtre interne
//!   `is_noun_node` acceptant le nom brut en plus du `_<digits>`).
//!
//! ## `NOUN_INFO` (noms localisés)
//!
//! `[0]` = hashId (Int, **obligatoire**) ; `[1]` = inconnu ; les variables `String` à partir de
//! l'index **2** donnent le nom : la **première** non vide (après `sanitize_text`) est le nom
//! principal, les suivantes sont des formes alternatives. Rejet si moins de 6 variables, si
//! `[0]`≠Int, ou si aucun nom non vide.
//!
//! ## `NOUN_INFO` romanisé (`chara_text_roma`)
//!
//! `[0]` = hashId (Int) ; les `String` non vides (à partir de l'index **1**, **trim** seulement,
//! pas de `sanitize_text`) donnent d'abord le **nom de famille** puis le **prénom**. Rejet si
//! moins de 3 variables, si `[0]`≠Int, ou si les deux noms sont vides.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;
use crate::text::sanitize_text;

/// Un nom de personnage localisé (port de `ParsedNoun`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParsedNoun {
    /// `[0]` — hash de l'entrée (lie au `nameHash`/`lastNameHash` de `chara_base`).
    pub hash_id: HashId,
    /// Nom principal (1ʳᵉ chaîne non vide ≥ index 2, nettoyée).
    pub name: String,
    /// Formes alternatives (chaînes non vides suivantes).
    pub alt_names: Vec<String>,
}

/// Un nom romanisé (port de `ParsedRomanizedName`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParsedRomanizedName {
    /// `[0]` — hash de l'entrée.
    pub hash_id: HashId,
    /// Prénom (2ᵉ chaîne non vide).
    pub first_name: String,
    /// Nom de famille (1ʳᵉ chaîne non vide).
    pub last_name: String,
}

/// Vrai si le noeud est un `NOUN_INFO`/`NOUN_INFO_<i>` à parser (hors `*BEGIN*`) — le nom brut
/// SANS suffixe est la forme réelle des enfants dans le T2B binaire (cf. commentaire d'en-tête) ;
/// le suffixe `_<i>` est un artefact du dumper JSON inagle, accepté pour compatibilité.
fn is_noun_node(name: &str) -> bool {
    (name == "NOUN_INFO" || name.starts_with("NOUN_INFO_")) && !name.contains("BEGIN")
}

/// Parse un noeud `NOUN_INFO` en [`ParsedNoun`] (port de `parseNounNode`).
#[must_use]
pub fn parse_noun_node(node: &Node<'_>) -> Option<ParsedNoun> {
    if node.var_count() < 6 {
        return None;
    }
    let hash_var = node.var(0)?;
    if hash_var.ty != "Int" {
        return None;
    }

    let mut name = String::new();
    let mut alt_names = Vec::new();
    for i in 2..node.var_count() {
        if let Some(v) = node.var(i)
            && v.ty == "String"
            && !v.value.trim().is_empty()
        {
            let cleaned = sanitize_text(v.value);
            if !cleaned.is_empty() {
                if name.is_empty() {
                    name = cleaned;
                } else {
                    alt_names.push(cleaned);
                }
            }
        }
    }

    if name.is_empty() {
        return None;
    }
    Some(ParsedNoun {
        hash_id: hash_var.as_hash(),
        name,
        alt_names,
    })
}

/// Parse tous les noms `NOUN_INFO` d'un `chara_text.cfg.bin.json` désérialisé
/// (port de `parseNounsFromConfig`).
#[must_use]
pub fn parse_all_nouns(root: &Value) -> Vec<ParsedNoun> {
    let mut out = Vec::new();
    walk_named(root, "NOUN_INFO", |node| {
        if is_noun_node(node.name())
            && let Some(n) = parse_noun_node(&node)
        {
            out.push(n);
        }
    });
    out
}

/// Parse un noeud `NOUN_INFO` romanisé en [`ParsedRomanizedName`] (port de `parseRomaNode`).
///
/// Les chaînes non vides (≥ index 1, **trim** seulement) donnent d'abord `last_name` puis
/// `first_name`.
#[must_use]
pub fn parse_roma_node(node: &Node<'_>) -> Option<ParsedRomanizedName> {
    if node.var_count() < 3 {
        return None;
    }
    let hash_var = node.var(0)?;
    if hash_var.ty != "Int" {
        return None;
    }

    let mut first_name = String::new();
    let mut last_name = String::new();
    for i in 1..node.var_count() {
        if let Some(v) = node.var(i)
            && v.ty == "String"
        {
            let t = v.value.trim();
            if !t.is_empty() {
                if last_name.is_empty() {
                    last_name = t.into();
                } else if first_name.is_empty() {
                    first_name = t.into();
                }
            }
        }
    }

    if first_name.is_empty() && last_name.is_empty() {
        return None;
    }
    Some(ParsedRomanizedName {
        hash_id: hash_var.as_hash(),
        first_name,
        last_name,
    })
}

/// Parse tous les noms romanisés d'un `chara_text_roma.cfg.bin.json` désérialisé
/// (port de `buildRomanizedMap`).
#[must_use]
pub fn parse_all_roma(root: &Value) -> Vec<ParsedRomanizedName> {
    let mut out = Vec::new();
    walk_named(root, "NOUN_INFO", |node| {
        if is_noun_node(node.name())
            && let Some(r) = parse_roma_node(&node)
        {
            out.push(r);
        }
    });
    out
}

/// Recherche un nom par hash (port de `buildNameMap().get`).
///
/// `buildNameMap` insère dans l'ordre (`map.set`) ; en cas de **hash dupliqué**, la **dernière**
/// entrée l'emporte. On reproduit donc cette sémantique « last-wins » (itération inverse).
#[must_use]
pub fn find_name(nouns: &[ParsedNoun], hash_id: HashId) -> Option<&str> {
    nouns
        .iter()
        .rev()
        .find(|n| n.hash_id == hash_id)
        .map(|n| n.name.as_str())
}
