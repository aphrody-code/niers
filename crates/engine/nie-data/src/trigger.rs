//! `trigger` — port des fichiers de **scripting de déclencheurs** `*_trigger.cfg.bin` (Level-5 IEVR).
//!
//! ~287 fichiers (`qsb*`/`qsa*` quêtes, `fbtl_cro*` matchs, `c21`/`c93` events…) partagent le format
//! `entries` : un `DATA_COUNT_0` (nombre d'items) + des `DATA_ITEM_<i>` à **7 variables**. La 4ᵉ
//! variable est un **blob de condition** (cf. [`crate::cond`]) décodé via [`crate::unlock_condition`] —
//! ce qui rend ces fichiers, longtemps « opaques », **structurés et lisibles**.
//!
//! ## Vérité terrain
//!
//! - Dumps réels : `data/common/gamedata/quest/qsb*_trigger_*.cfg.bin.json`,
//!   `data/common/gamedata/soccer/game/fbtl_cro*_trigger_*.cfg.bin.json`, etc.
//! - `DATA_ITEM_<i>` : `var[0]` = `kind` (catégorie d'action, ex. 3/22/90) ; `var[1]` = `target`
//!   (hash CRC) ; `var[2]` = `value` (paramètre) ; **`var[3]` = condition** (blob base64, décodé) ;
//!   `var[4..6]` = entiers résiduels (sémantique non reversée → `extra`).
//!
//! Anti-hallucination : seule la **condition** (`var[3]`) a une sémantique reversée+validée (système
//! cond/unlock_condition). Les noms `kind`/`target`/`value` sont structurels ; `extra` reste brut.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::Node;
use crate::hash::HashId;
use crate::unlock_condition::{UnlockCondition, decode_unlock_condition};

/// Un item de déclencheur (`DATA_ITEM_<i>`), avec sa condition décodée.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TriggerItem {
    /// `var[0]` — catégorie d'action du déclencheur (sémantique précise non reversée).
    pub kind: i64,
    /// `var[1]` — hash cible (CRC).
    pub target: HashId,
    /// `var[2]` — paramètre/valeur associé.
    pub value: i64,
    /// `var[3]` — **condition de déclenchement décodée** (Always si vide/absente).
    pub condition: UnlockCondition,
    /// `var[4..=6]` — 3 entiers résiduels (sémantique non reversée).
    pub extra: [i64; 3],
}

impl TriggerItem {
    /// Parse un nœud `DATA_ITEM_<i>` (au moins 4 variables requises). `None` sinon.
    #[must_use]
    pub fn from_node(node: &Node) -> Option<Self> {
        if node.var_count() < 4 {
            return None;
        }
        Some(Self {
            kind: node.int(0),
            target: node.hash(1),
            value: node.int(2),
            condition: decode_unlock_condition(node.string(3)),
            extra: [node.int(4), node.int(5), node.int(6)],
        })
    }
}

/// Contenu d'un `*_trigger.cfg.bin` : le compte déclaré + les items (conditions décodées).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TriggerConfig {
    /// `DATA_COUNT_0` — nombre d'items déclaré (`0` si absent).
    pub count: i64,
    /// `DATA_ITEM_<i>` — les items du déclencheur, dans l'ordre du fichier.
    pub items: Vec<TriggerItem>,
}

/// Parse un `*_trigger.cfg.bin.json` (format `entries` `DATA_COUNT`/`DATA_ITEM`).
///
/// Renvoie un [`TriggerConfig`] vide si le fichier n'a pas ce format (sûr pour le dispatch par
/// suffixe `_trigger`, qui couvre aussi quelques fichiers d'autre forme).
#[must_use]
pub fn parse_trigger(root: &Value) -> TriggerConfig {
    let mut count = 0;
    let mut items = Vec::new();
    if let Some(entries) = root.get("entries").and_then(Value::as_array) {
        for e in entries {
            let node = Node::new(e);
            let name = node.name();
            if name.starts_with("DATA_COUNT") {
                count = node.int(0);
            } else if name.starts_with("DATA_ITEM")
                && let Some(item) = TriggerItem::from_node(&node)
            {
                items.push(item);
            }
        }
    }
    TriggerConfig { count, items }
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
                // item 0 : condition event-flag réelle (qsb040400 DATA_ITEM_0).
                { "name": "DATA_ITEM_0", "variables": [
                    ivar(3), ivar(-687566521), ivar(0),
                    svar("AAAAABgFNb4EpZgACgEoAAYCNEUqot4yAAAAAXg="),
                    ivar(0), ivar(0), ivar(1)
                ], "children": [] },
                // item 1 : pas de condition (var[3] = "0").
                { "name": "DATA_ITEM_1", "variables": [
                    ivar(90), ivar(170504797), ivar(782182720), svar("0"), ivar(0), ivar(0), ivar(2)
                ], "children": [] }
            ]
        })
    }

    #[test]
    fn parse_count_et_items() {
        let cfg = parse_trigger(&fixture());
        assert_eq!(cfg.count, 2);
        assert_eq!(cfg.items.len(), 2);
    }

    #[test]
    fn item0_condition_event_flag_decodee() {
        let cfg = parse_trigger(&fixture());
        let it = &cfg.items[0];
        assert_eq!(it.kind, 3);
        assert_eq!(it.target, HashId(0xD704_9147)); // -687566521 → u32
        assert_eq!(it.extra, [0, 0, 1]);
        // var[3] décode en condition event-flag (namespace non-story).
        assert_eq!(it.condition.kind, UnlockType::EventFlag);
        assert!(!it.condition.required_events.is_empty());
    }

    #[test]
    fn item_sans_condition_est_always() {
        let cfg = parse_trigger(&fixture());
        // var[3] = "0" (non-blob) → condition Always.
        assert_eq!(cfg.items[1].condition.kind, UnlockType::Always);
    }
}
