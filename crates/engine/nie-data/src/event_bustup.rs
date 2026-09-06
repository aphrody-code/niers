//! `event_bustup` — port des `event_bustup_talk_data_config_*.cfg.bin` (Level-5 IEVR).
//!
//! Données de **bustup (portraits) de dialogue** par chapitre : pour chaque slot de personnage à
//! l'écran (`CHR_1`..`CHR_N`), une liste d'entrées pilotant le portrait/expression affiché ligne
//! par ligne. ~34 fichiers (`_c<NN>`). Format `entries` : groupes
//! `EV_BUSTUP_TALK_DATA_CHR_<n>_LIST_BEG` → enfants `EV_BUSTUP_TALK_DATA_CHR_<n>_<i>` (6 variables).
//!
//! Vérité terrain : `event/event_bustup_talk_data_config_c23_3.00.06.cfg.bin.json`. Une entrée a
//! ~46 variables : `var[0]` = chara_id (hash), `var[1]` = motion (hash), puis des flags entiers et
//! **des chemins `.g4pk` de modèles de portrait** (variables `String`, ex.
//! `common/chr/c000401/c000401_p060.g4pk`). On capture les champs **clairs** (chara_id, motion,
//! chemins de modèles) ; la masse d'entiers résiduels reste non exposée (sémantique non reversée —
//! anti-hallucination).

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::Node;
use crate::hash::HashId;

/// Une entrée de bustup (un état de portrait pour une ligne de dialogue).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BustupEntry {
    /// `var[0]` — hash du personnage affiché.
    pub chara_id: HashId,
    /// `var[1]` — hash de motion/expression (`0` = aucun).
    pub motion_id: HashId,
    /// Chemins `.g4pk` des modèles de portrait référencés par l'entrée (variables `String`).
    pub model_paths: Vec<String>,
}

impl BustupEntry {
    /// Parse un nœud d'entrée (≥ 2 variables requises). Collecte les variables `String` non vides
    /// comme chemins de modèle.
    #[must_use]
    pub fn from_node(node: &Node) -> Option<Self> {
        if node.var_count() < 2 {
            return None;
        }
        let mut model_paths = Vec::new();
        for i in 2..node.var_count() {
            if let Some(var) = node.var(i)
                && var.ty == "String"
                && !var.value.is_empty()
            {
                model_paths.push(String::from(var.value));
            }
        }
        Some(Self {
            chara_id: node.hash(0),
            motion_id: node.hash(1),
            model_paths,
        })
    }
}

/// Un slot de personnage à l'écran (`CHR_<slot>`) et ses entrées de bustup.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BustupChrSlot {
    /// Numéro de slot (le `<n>` de `CHR_<n>`).
    pub slot: u32,
    /// Entrées de bustup du slot, dans l'ordre du fichier.
    pub entries: Vec<BustupEntry>,
}

/// Contenu d'un `event_bustup_talk_data_config_*.cfg.bin`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventBustupTalkConfig {
    /// Les slots de personnage (`CHR_1`..`CHR_N`), dans l'ordre du fichier.
    pub chr_slots: Vec<BustupChrSlot>,
}

/// Extrait le numéro de slot d'un nom `EV_BUSTUP_TALK_DATA_CHR_<n>_LIST_BEG_<k>`.
fn slot_of(name: &str) -> Option<u32> {
    // …CHR_<n>_LIST_BEG… : le segment juste après "CHR_".
    let after = name.split("CHR_").nth(1)?;
    let digits: alloc::string::String = after.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Parse un `event_bustup_talk_data_config_*.cfg.bin.json`.
#[must_use]
pub fn parse_event_bustup_talk(root: &Value) -> EventBustupTalkConfig {
    let mut chr_slots = Vec::new();
    if let Some(entries) = root.get("entries").and_then(Value::as_array) {
        for e in entries {
            let node = Node::new(e);
            let name = node.name();
            if !name.contains("CHR_") || !name.contains("LIST_BEG") {
                continue;
            }
            let Some(slot) = slot_of(name) else { continue };
            let mut items = Vec::new();
            for child in node.children() {
                if let Some(entry) = BustupEntry::from_node(&child) {
                    items.push(entry);
                }
            }
            chr_slots.push(BustupChrSlot {
                slot,
                entries: items,
            });
        }
    }
    EventBustupTalkConfig { chr_slots }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                // Forme réelle : chara_id, motion, flags…, 2 chemins g4pk de portrait.
                { "name": "EV_BUSTUP_TALK_DATA_CHR_1_LIST_BEG_0", "variables": [ivar(1)], "children": [
                    { "name": "EV_BUSTUP_TALK_DATA_CHR_1_0", "variables": [
                        ivar(925201769), ivar(0), ivar(-1), ivar(-1), ivar(-1), ivar(0), ivar(0),
                        svar("common/chr/c000401/c000401_p060.g4pk"),
                        svar("common/chr/c000401/c000401_p250.g4pk")
                    ] }
                ]},
                { "name": "EV_BUSTUP_TALK_DATA_CHR_2_LIST_BEG_0", "variables": [ivar(1)], "children": [
                    { "name": "EV_BUSTUP_TALK_DATA_CHR_2_0",
                      "variables": [ivar(-835750149), ivar(-963644590), ivar(1), ivar(1)] }
                ]}
            ]
        })
    }

    #[test]
    fn parse_slots_chara_et_modeles() {
        let cfg = parse_event_bustup_talk(&fixture());
        assert_eq!(cfg.chr_slots.len(), 2);
        assert_eq!(cfg.chr_slots[0].slot, 1);
        let e0 = &cfg.chr_slots[0].entries[0];
        assert_eq!(e0.chara_id, HashId(0x3725_7569)); // 925201769
        assert_eq!(e0.motion_id, HashId::ZERO);
        // Chemins g4pk de portrait capturés (les variables String).
        assert_eq!(e0.model_paths.len(), 2);
        assert_eq!(e0.model_paths[0], "common/chr/c000401/c000401_p060.g4pk");
        // slot 2 : chara_id + motion hash, sans modèle.
        let e = &cfg.chr_slots[1].entries[0];
        assert_eq!(cfg.chr_slots[1].slot, 2);
        assert_eq!(e.chara_id, HashId(0xCE2F_76FB)); // -835750149 → u32
        assert_eq!(e.motion_id, HashId(0xC68F_F352)); // -963644590 → u32
        assert!(e.model_paths.is_empty());
    }
}
