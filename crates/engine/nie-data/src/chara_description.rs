//! `chara_description` — descriptions de personnage localisées
//! (`common/text/<locale>/chara_description_text.cfg.bin`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/chara-description.ts`
//!   (`parseDescriptionNode`, `loadCharaDescriptions`) — **port 1:1**.
//! - Données : `data/common/text/fr/chara_description_text.cfg.bin` (et autres locales) du VFS,
//!   format **T2B** (`entries`). Structure (probe live) : un noeud `TEXT_INFO_BEGIN_0` contenant
//!   **5569** enfants `TEXT_INFO_<i>`, chacun à **3 variables** `[hashId (Int), 0 (Int),
//!   description (String)]`.
//!
//! Le texte est nettoyé par [`crate::text::sanitize_text`] (port de `sanitizeText`) ; une
//! description vide après nettoyage est **rejetée** (`None`), comme inagle.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;
use crate::text::sanitize_text;

/// Une description de personnage portée (port de `ParsedDescription`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParsedDescription {
    /// `[0]` — hash de l'entrée (lie au `nameHash` de `chara_base`).
    pub hash_id: HashId,
    /// `[2]` — texte de description **nettoyé** (`sanitize_text`).
    pub description: String,
}

/// Parse un noeud `TEXT_INFO` en [`ParsedDescription`] (port de `parseDescriptionNode`).
///
/// Rejette (`None`) si moins de 3 variables, si `[0]` n'est pas `Int`, si `[2]` n'est pas
/// `String`, ou si la description nettoyée est vide.
#[must_use]
pub fn parse_description_node(node: &Node<'_>) -> Option<ParsedDescription> {
    if node.var_count() < 3 {
        return None;
    }
    let hash_var = node.var(0)?;
    if hash_var.ty != "Int" {
        return None;
    }
    let desc_var = node.var(2)?;
    if desc_var.ty != "String" {
        return None;
    }
    let description = sanitize_text(desc_var.value);
    if description.is_empty() {
        return None;
    }
    Some(ParsedDescription {
        hash_id: hash_var.as_hash(),
        description,
    })
}

/// Parse toutes les descriptions d'un `chara_description_text.cfg.bin.json` désérialisé
/// (port de `loadCharaDescriptions`).
///
/// Visite les noeuds `TEXT_INFO_*` (le noeud de liste `TEXT_INFO_BEGIN_0` est naturellement
/// écarté : sa variable unique échoue au contrôle `≥ 3 variables`).
#[must_use]
pub fn parse_chara_descriptions(root: &Value) -> Vec<ParsedDescription> {
    let mut out = Vec::new();
    walk_named(root, "TEXT_INFO_", |node| {
        if let Some(d) = parse_description_node(&node) {
            out.push(d);
        }
    });
    out
}

/// Recherche une description par hash (port de l'accès `Map.get`).
#[must_use]
pub fn find_by_hash(descriptions: &[ParsedDescription], hash_id: HashId) -> Option<&str> {
    descriptions
        .iter()
        .find(|d| d.hash_id == hash_id)
        .map(|d| d.description.as_str())
}
