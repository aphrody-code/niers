//! `ParsedQuest` — port Rust de la famille `quest/quest_config_*.cfg.bin` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! Fichier ciblé : `data/common/gamedata/quest/quest_config_1.04.11.00.cfg.bin`
//! (isolé parmi les ~65 fichiers `qsa/qsb*_trigger` du dossier `quest/`, non concernés).
//!
//! Format **`entries`** (arbre T2B). La racine contient plusieurs listes `*_LIST_BEG` ;
//! seule `QUEST_DATA_CFG_LIST_BEG_0` (forme iecode, suffixe `_0` ajouté à la conversion)
//! nous intéresse. Ses 546 enfants se répartissent en :
//!
//! | Nom enfant (iecode)               | Nombre | Traitement       |
//! |-----------------------------------|--------|------------------|
//! | `QUEST_DATA_CFG_<i>`              | 182    | quête parsée     |
//! | `QUEST_DATA_CFG_REF_PHASE_<i>`   | 182    | sauté (`_REF_`)  |
//! | `QUEST_DATA_CFG_REF_REWARD_ITEM_<i>` | 182 | sauté (`_REF_`)  |
//!
//! ## Structure `QUEST_DATA_CFG` (20 variables positionnelles)
//!
//! Le parseur ne lit que 5 positions (port 1:1 de
//! `packages/inagle/src/parsers/quest-config.ts`, fn `parseQuestNode`) :
//!
//! - var\[0\] = `quest_id` (Int → hex non signé) — identifiant unique de la quête.
//! - var\[1\] = `phase` (Int, défaut 0) — chapitre/phase.
//! - var\[2\] = `quest_type` (Int, défaut 0) — type de quête.
//! - var\[3\] = `title_hash` (Int → hex non signé) — hash du titre (clé `quest_title`).
//! - var\[16\] = `image` (String optionnelle, ignorée si vide) — nom de l'image.
//!
//! Les positions 4..=15, 17..=19 (chaîne base64 de déclenchement en var\[5\], nom de
//! fichier `.g4tx` en var\[17\], divers entiers) ne sont pas exposées : inagle ne les lit pas.
//!
//! ## Règles de rejet (identiques à inagle)
//!
//! - moins de 4 variables → `None` ;
//! - var\[0\] ou var\[3\] de type ≠ `Int` → `None`.
//!
//! Vérité terrain (premier noeud `QUEST_DATA_CFG_0`, dump réel) :
//! `quest_id = 1732101895 → 0x673DC707`, `phase = 0`, `quest_type = 0`,
//! `title_hash = 185538439 → 0x0B0F1787`, `image = "story_img001_l01"`.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

/// Type de quête (énumération source d'inagle `QuestType`).
///
/// Note : dans `quest_config`, la position var\[2\] contient des valeurs brutes qui ne se
/// limitent pas à cet ensemble (le dump réel observe 0..5 sur les premières quêtes story).
/// `ParsedQuest` conserve donc l'entier brut ; cette énumération sert de classification
/// optionnelle, portée à l'identique de la source TS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum QuestType {
    /// Quête principale (`1`).
    Main,
    /// Quête secondaire (`2`).
    Sub,
    /// Quête journalière (`3`).
    Daily,
}

impl QuestType {
    /// Classe une valeur brute var\[2\] selon l'énumération inagle (`None` hors 1..=3).
    #[must_use]
    pub const fn from_raw(v: i64) -> Option<Self> {
        match v {
            1 => Some(QuestType::Main),
            2 => Some(QuestType::Sub),
            3 => Some(QuestType::Daily),
            _ => None,
        }
    }
}

/// Une quête parsée depuis un noeud `QUEST_DATA_CFG_*`.
///
/// Port 1:1 de l'interface TS `ParsedQuest` (`parsers/quest-config.ts`). Les champs
/// `questId`/`titleHash` côté TS sont des chaînes hex (`toHex`) ; ici on stocke des
/// [`HashId`] qui formatent à l'identique via [`HashId::to_hex`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParsedQuest {
    /// var\[0\] — identifiant unique de la quête (Int signé → hash non signé).
    pub quest_id: HashId,
    /// var\[3\] — hash du titre (clé de jointure `quest_title`, Int signé → hash non signé).
    pub title_hash: HashId,
    /// var\[1\] — phase/chapitre (entier, défaut 0 si type ≠ `Int`).
    pub phase: i64,
    /// var\[2\] — type de quête (entier brut, défaut 0 si type ≠ `Int`).
    pub quest_type: i64,
    /// var\[16\] — nom de l'image (présent uniquement si type `String` et valeur non vide).
    pub image: Option<String>,
}

impl ParsedQuest {
    /// Parse un noeud `QUEST_DATA_CFG_*`. Renvoie `None` selon les règles de rejet inagle :
    /// moins de 4 variables, ou var\[0\]/var\[3\] de type ≠ `Int`.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        // vars.length < 4 → null.
        if node.var_count() < 4 {
            return None;
        }

        // [0] = quest ID hash — doit être de type Int.
        let id_var = node.var(0)?;
        if id_var.ty != "Int" {
            return None;
        }
        let quest_id = id_var.as_hash();

        // [1] = phase/chapter — 0 si type ≠ Int.
        let phase = node
            .var(1)
            .filter(|v| v.ty == "Int")
            .map_or(0, |v| v.as_i64());

        // [2] = type — 0 si type ≠ Int.
        let quest_type = node
            .var(2)
            .filter(|v| v.ty == "Int")
            .map_or(0, |v| v.as_i64());

        // [3] = title hash — doit être de type Int.
        let title_var = node.var(3)?;
        if title_var.ty != "Int" {
            return None;
        }
        let title_hash = title_var.as_hash();

        // [16] = image — String non vide uniquement.
        let image = if node.var_count() > 16 {
            node.var(16)
                .filter(|v| v.ty == "String" && !v.value.is_empty())
                .map(|v| String::from(v.value))
        } else {
            None
        };

        Some(Self {
            quest_id,
            title_hash,
            phase,
            quest_type,
            image,
        })
    }
}

/// Parse un `quest_config_*.cfg.bin.json` (forme iecode) en liste de quêtes.
///
/// Port 1:1 de `loadQuestConfig` (`parsers/quest-config.ts`) : parcours récursif des
/// `entries`, repérage du noeud exact `QUEST_DATA_CFG_LIST_BEG_0`, itération de ses
/// enfants directs dont le nom commence par `QUEST_DATA_CFG_` (sauf ceux contenant
/// `_REF_`, sautés).
///
/// Compte réel : `quest_config_1.04.11.00.cfg.bin` → 182 quêtes.
///
/// # Exemple
///
/// ```
/// use nie_data::quest::parse_quest_config;
/// use nie_data::hash::HashId;
/// use serde_json::json;
///
/// let root = json!({
///     "entries": [{
///         "name": "QUEST_DATA_CFG_LIST_BEG_0",
///         "variables": [{"type": "Int", "value": "1"}],
///         "children": [{
///             "name": "QUEST_DATA_CFG_0",
///             "variables": [
///                 {"type": "Int", "value": "1732101895"},
///                 {"type": "Int", "value": "0"},
///                 {"type": "Int", "value": "0"},
///                 {"type": "Int", "value": "185538439"}
///             ],
///             "children": []
///         }]
///     }]
/// });
/// let quests = parse_quest_config(&root);
/// assert_eq!(quests.len(), 1);
/// assert_eq!(quests[0].quest_id, HashId::from_i64(1732101895));
/// assert_eq!(quests[0].quest_id.to_hex(), "0x673DC707");
/// ```
#[must_use]
pub fn parse_quest_config(root: &Value) -> Vec<ParsedQuest> {
    let mut out = Vec::new();
    walk_named(root, "QUEST_DATA_CFG_LIST_BEG_0", |node| {
        // walk_named matche par préfixe ; on exige le nom exact comme inagle.
        if node.name() != "QUEST_DATA_CFG_LIST_BEG_0" {
            return;
        }
        for child in node.children() {
            let name = child.name();
            if !name.starts_with("QUEST_DATA_CFG_") {
                continue;
            }
            // Skip REF nodes (QUEST_DATA_CFG_REF_PHASE_*, QUEST_DATA_CFG_REF_REWARD_ITEM_*).
            if name.contains("_REF_") {
                continue;
            }
            if let Some(quest) = ParsedQuest::from_node(child) {
                out.push(quest);
            }
        }
    });
    out
}

/// Résout le **titre localisé** d'une quête via une liste `quest_title_text` (issue de
/// [`crate::text::parse_text_file`]).
///
/// Port de la jointure de `buildQuestDatabase` (`loadTextFile("quest_title", locale).get(titleHash)`,
/// `quest-config.ts` l.158-172) : `quest.titles[locale] = textMap.get(titleHash)`. La sémantique
/// **last-wins** de la map est portée par [`crate::text::find_text`]. Retourne `None` si le hash
/// n'a pas d'entrée de texte (quête sans titre localisé dans la locale donnée).
#[must_use]
pub fn resolve_title<'a>(quest: &ParsedQuest, titles: &'a [(HashId, String)]) -> Option<&'a str> {
    crate::text::find_text(titles, quest.title_hash)
}
