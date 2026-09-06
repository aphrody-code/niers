//! `AiTypeConfig` — port Rust de `system/ai_type_config.cfg.bin` (Level-5 IEVR).
//!
//! **Types/hobbies d'IA** : chaque type d'IA (`AI_AI_INFO`) référence une tranche de « hobbies »
//! (`AI_HOBY`). Famille game-data **non couverte** ; **pas de parseur inagle** → port direct.
//! Les valeurs entières des hobbies/infos n'ont **pas de sémantique documentée** → conservées en
//! `params` bruts (anti-hallucination : on ne nomme pas ce qu'on ne sait pas).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/system/ai_type_config.cfg.bin.json` (format `entries`/arbre).
//! - `AI_HOBY_LIST_BEG_0` → **4** `AI_HOBY_<i>` (3 ints chacun).
//! - `AI_AI_INFO_LIST_BEG_0` → **5** `AI_AI_INFO_<i>` (3 ints), **chacun suivi** d'un nœud frère
//!   `AI_AI_INFO_REF_HOBY_<i>` (`[offset, count]` indexant `AI_HOBY`) — pattern entrée+REF
//!   (identique à `win_treasure` ITBL_BASE/REF).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::Node;
use crate::hash::HashId;

/// Entrée `AI_HOBY` — un « hobby » d'IA (3 valeurs entières, sémantique non documentée).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AiHoby {
    /// Les 3 paramètres bruts du hobby (`var[0..3]`). Sémantique non reversée.
    pub params: [i64; 3],
}

/// Entrée `AI_AI_INFO` — un type d'IA + sa tranche de hobbies (via le nœud frère REF).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AiInfo {
    /// `var[0]` — hash/crc identifiant le type d'IA (int signé → u32).
    pub ai_id: HashId,
    /// `var[1..3]` — 2 paramètres bruts (sémantique non reversée).
    pub params: [i64; 2],
    /// `[offset, count]` du nœud frère `AI_AI_INFO_REF_HOBY_<i>` indexant `AI_HOBY`.
    pub hoby_slice: [i64; 2],
}

/// Contenu complet d'un `ai_type_config.cfg.bin`.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AiTypeConfig {
    /// `AI_HOBY_LIST_BEG_0` — les hobbies (indexés par tranche).
    pub hobies: Vec<AiHoby>,
    /// `AI_AI_INFO_LIST_BEG_0` — les types d'IA.
    pub ai_infos: Vec<AiInfo>,
}

impl AiTypeConfig {
    /// Résout la tranche de hobbies d'un [`AiInfo`] dans `hobies`. Bornes plafonnées à `hobies.len()`.
    #[must_use]
    pub fn hobies_of(&self, info: &AiInfo) -> &[AiHoby] {
        let n = self.hobies.len();
        let start = (info.hoby_slice[0].max(0) as usize).min(n);
        let end = start
            .saturating_add(info.hoby_slice[1].max(0) as usize)
            .min(n);
        self.hobies.get(start..end).unwrap_or(&[])
    }
}

/// Trouve la racine dont le nom débute par `prefix` et renvoie ses enfants.
fn group_children<'a>(root: &'a Value, prefix: &str) -> Vec<Node<'a>> {
    if let Some(entries) = root.get("entries").and_then(Value::as_array) {
        for e in entries {
            let node = Node::new(e);
            if node.name().starts_with(prefix) {
                return node.children();
            }
        }
    }
    Vec::new()
}

/// Parse un `ai_type_config.cfg.bin.json`.
#[must_use]
pub fn parse_ai_type_config(root: &Value) -> AiTypeConfig {
    let hobies = group_children(root, "AI_HOBY_LIST_BEG")
        .iter()
        .map(|c| AiHoby {
            params: [c.int(0), c.int(1), c.int(2)],
        })
        .collect();

    // AI_AI_INFO_<i> est suivi de son nœud frère AI_AI_INFO_REF_HOBY_<i> (tranche de hobbies).
    let ai_children = group_children(root, "AI_AI_INFO_LIST_BEG");
    let mut ai_infos = Vec::new();
    for (i, c) in ai_children.iter().enumerate() {
        if c.name().starts_with("AI_AI_INFO_REF") || !c.name().starts_with("AI_AI_INFO_") {
            continue;
        }
        let hoby_slice = match ai_children.get(i + 1) {
            Some(r) if r.name().starts_with("AI_AI_INFO_REF_HOBY") => [r.int(0), r.int(1)],
            _ => [0, 0],
        };
        ai_infos.push(AiInfo {
            ai_id: c.hash(0),
            params: [c.int(1), c.int(2)],
            hoby_slice,
        });
    }
    AiTypeConfig { hobies, ai_infos }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({
            "entries": [
                { "name": "AI_HOBY_LIST_BEG_0", "variables": [], "children": [
                    { "name": "AI_HOBY_0", "variables": [
                        { "type": "Int", "value": "1" }, { "type": "Int", "value": "20" }, { "type": "Int", "value": "0" }] },
                    { "name": "AI_HOBY_1", "variables": [
                        { "type": "Int", "value": "2" }, { "type": "Int", "value": "50" }, { "type": "Int", "value": "0" }] }
                ]},
                { "name": "AI_AI_INFO_LIST_BEG_0", "variables": [], "children": [
                    { "name": "AI_AI_INFO_0", "variables": [
                        { "type": "Int", "value": "-907993416" }, { "type": "Int", "value": "1" }, { "type": "Int", "value": "1" }] },
                    { "name": "AI_AI_INFO_REF_HOBY_0", "variables": [
                        { "type": "Int", "value": "0" }, { "type": "Int", "value": "2" }] }
                ]}
            ]
        })
    }

    #[test]
    fn parse_hobies_et_infos() {
        let cfg = parse_ai_type_config(&fixture());
        assert_eq!(cfg.hobies.len(), 2);
        assert_eq!(cfg.hobies[0].params, [1, 20, 0]);
        assert_eq!(
            cfg.ai_infos.len(),
            1,
            "1 info (le REF n'est pas une entrée)"
        );
        let info = &cfg.ai_infos[0];
        assert_eq!(info.ai_id, HashId(0xC9E1_1EB8)); // -907993416 → u32
        assert_eq!(info.params, [1, 1]);
        assert_eq!(info.hoby_slice, [0, 2]);
    }

    #[test]
    fn resolution_ref_hoby() {
        let cfg = parse_ai_type_config(&fixture());
        let info = &cfg.ai_infos[0];
        let hobies = cfg.hobies_of(info);
        assert_eq!(hobies.len(), 2); // slice [0,2] → hobies[0],[1]
        assert_eq!(hobies[1].params, [2, 50, 0]);
    }
}
