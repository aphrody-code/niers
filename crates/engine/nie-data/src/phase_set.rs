//! `phase_set` — port des fichiers de **setup de phases de match/event** `*_phase_set.cfg.bin`.
//!
//! ~182 fichiers (`fbtl_cro*_phase_set` matchs, `fbtl_qs*_phase_set`…) au format `entries`
//! `DATA_COUNT`/`DATA_ITEM`. Les items `fbtl_cro` ont la forme `(Int, Int, condition)` — la 3ᵉ
//! variable est un **blob de condition** (cf. [`crate::cond`]) décodé via [`crate::unlock_condition`].
//!
//! Schéma hétérogène entre familles (`fbtl_cro` = 3 vars dont 1 condition ; `fbtl_qs` = 3 Int) →
//! représentation **générique honnête** : chaque item expose ses variables entières dans l'ordre +
//! les conditions décodées de ses variables chaîne. (Les `phase_set_c*` à 11 variables ont une clé
//! en `_cNN`, non captés par le dispatch suffixe `_phase_set`.)

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::Node;
use crate::unlock_condition::{UnlockCondition, decode_unlock_condition};

/// Un item de phase (`DATA_ITEM_<i>`) : variables entières + conditions décodées.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhaseSetItem {
    /// Les variables `Int` de l'item, dans l'ordre (sémantique selon la famille — non reversée).
    pub ints: Vec<i64>,
    /// Les conditions décodées des variables chaîne qui sont des blobs `cond` (en-tête `AAAA`).
    pub conditions: Vec<UnlockCondition>,
}

impl PhaseSetItem {
    /// Parse un nœud `DATA_ITEM_<i>` : variables `Int` → `ints` ; variables `String` ressemblant à
    /// un blob `cond` (préfixe base64 `AAAA`, ≥ 12 car.) → décodées en condition.
    #[must_use]
    pub fn from_node(node: &Node) -> Self {
        let mut ints = Vec::new();
        let mut conditions = Vec::new();
        for i in 0..node.var_count() {
            if let Some(var) = node.var(i) {
                if var.ty == "String" {
                    let s = var.value;
                    if s.len() >= 12 && s.starts_with("AAAA") {
                        conditions.push(decode_unlock_condition(s));
                    }
                } else {
                    ints.push(var.as_i64());
                }
            }
        }
        Self { ints, conditions }
    }
}

/// Contenu d'un `*_phase_set.cfg.bin` : compte déclaré + items.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhaseSetConfig {
    /// `DATA_COUNT_0` — nombre d'items déclaré.
    pub count: i64,
    /// `DATA_ITEM_<i>` — items de phase, dans l'ordre.
    pub items: Vec<PhaseSetItem>,
}

/// Parse un `*_phase_set.cfg.bin.json` (`entries` `DATA_COUNT`/`DATA_ITEM`). Vide si autre format.
#[must_use]
pub fn parse_phase_set(root: &Value) -> PhaseSetConfig {
    let mut count = 0;
    let mut items = Vec::new();
    if let Some(entries) = root.get("entries").and_then(Value::as_array) {
        for e in entries {
            let node = Node::new(e);
            let name = node.name();
            if name.starts_with("DATA_COUNT") {
                count = node.int(0);
            } else if name.starts_with("DATA_ITEM") {
                items.push(PhaseSetItem::from_node(&node));
            }
        }
    }
    PhaseSetConfig { count, items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unlock_condition::UnlockType;
    use alloc::string::ToString;
    use serde_json::json;

    fn ivar(v: i64) -> Value {
        json!({ "type": "Int", "value": v.to_string() })
    }
    fn svar(v: &str) -> Value {
        json!({ "type": "String", "value": v })
    }

    fn fixture() -> Value {
        json!({
            "entries": [
                { "name": "DATA_COUNT_0", "variables": [ivar(2)], "children": [] },
                // fbtl_cro forme (Int, Int, condition).
                { "name": "DATA_ITEM_0", "variables": [
                    ivar(1), ivar(10), svar("AAAAACEFNYdXQAIAEwIoAAYCNFlSejAoAAYCMgAAAAAyAAAA/3g=")
                ], "children": [] },
                // fbtl_qs forme (Int, Int, Int) — aucune condition.
                { "name": "DATA_ITEM_1", "variables": [ivar(1), ivar(20), ivar(5)], "children": [] }
            ]
        })
    }

    #[test]
    fn parse_ints_et_conditions() {
        let cfg = parse_phase_set(&fixture());
        assert_eq!(cfg.count, 2);
        assert_eq!(cfg.items.len(), 2);
        // item 0 : 2 ints + 1 condition décodée.
        assert_eq!(cfg.items[0].ints, [1, 10]);
        assert_eq!(cfg.items[0].conditions.len(), 1);
        assert_ne!(cfg.items[0].conditions[0].kind, UnlockType::Always);
        // item 1 : 3 ints, aucune condition.
        assert_eq!(cfg.items[1].ints, [1, 20, 5]);
        assert!(cfg.items[1].conditions.is_empty());
    }
}
