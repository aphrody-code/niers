#![allow(clippy::pedantic)]
//! Tests golden `activity` — valeurs réelles tirées de :
//! - `data/common/gamedata/system/activity_config.cfg.bin.json`
//!
//! Format `entries` (variables positionnelles). On vérifie l'arbre observé :
//! `StoryMode` (racine, kind 1) + `StoryMode_SubTask_01..04` (kind 5, parent = id de StoryMode).

mod common;

extern crate std;

use nie_data::activity::parse_activity_config;
use serde_json::json;

const REAL: &str = "system/activity_config.cfg.bin.json";

fn load_json(path: &str) -> Option<serde_json::Value> {
    let path = common::chemin(path)?;
    if !path.is_file() {
        eprintln!("skip : {} absent du corpus", path.display());
        return None;
    }
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", path.display()));
    Some(
        serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("JSON invalide {}: {e}", path.display())),
    )
}

/// Fixture inline reproduisant la structure : 1 LIST_BEG (1 var, ignoré) + une racine + 1 sous-tâche.
fn fixture() -> serde_json::Value {
    json!({
        "entries": [
            { "name": "ACTIVITY_CONFIG_LIST_BEG_0", "variables": [{"type": "Int", "value": "2"}],
              "children": [
                { "name": "ACTIVITY_CONFIG_0", "children": [], "variables": [
                    {"type": "Int", "value": "583576710"},
                    {"type": "String", "value": "StoryMode"},
                    {"type": "Int", "value": "1"},
                    {"type": "Int", "value": "0"},
                    {"type": "String", "value": "AAAAAA8FNbkZNtoAAQAyAAAnEHE="}
                ]},
                { "name": "ACTIVITY_CONFIG_1", "children": [], "variables": [
                    {"type": "Int", "value": "1439669264"},
                    {"type": "String", "value": "StoryMode_SubTask_01"},
                    {"type": "Int", "value": "5"},
                    {"type": "Int", "value": "583576710"},
                    {"type": "String", "value": "AAAAAA8FNbkZNtoAAQAyAABOIHE="}
                ]}
              ]
            }
        ]
    })
}

#[test]
fn fixture_arbre_storymode() {
    let acts = parse_activity_config(&fixture());
    assert_eq!(
        acts.len(),
        2,
        "le LIST_BEG (1 var) doit être filtré, 2 vraies entrées"
    );
    let root = &acts[0];
    assert_eq!(root.name, "StoryMode");
    assert_eq!(root.kind, 1);
    assert!(root.is_root());
    let sub = &acts[1];
    assert_eq!(sub.name, "StoryMode_SubTask_01");
    assert_eq!(sub.kind, 5);
    assert!(!sub.is_root());
    // La sous-tâche pointe vers l'id de la racine.
    assert_eq!(
        sub.parent_id, root.id,
        "parent_id de la sous-tâche = id de StoryMode"
    );
    assert!(!root.data.is_empty(), "le blob base64 est conservé brut");
}

#[test]
fn golden_dump_reel() {
    let Some(root) = load_json(REAL) else {
        eprintln!("dump activity absent — test data-gated ignoré");
        return;
    };
    let acts = parse_activity_config(&root);
    assert_eq!(acts.len(), 13, "13 activités dans activity_config réel");
    // Première = StoryMode racine.
    assert_eq!(acts[0].name, "StoryMode");
    assert!(acts[0].is_root());
    // Toutes les sous-tâches (kind 5) ont un parent non nul présent dans la liste.
    for a in acts.iter().filter(|a| a.kind == 5) {
        assert!(!a.parent_id.is_zero(), "sous-tâche {} sans parent", a.name);
        assert!(
            acts.iter().any(|p| p.id == a.parent_id),
            "parent de {} introuvable dans la liste",
            a.name
        );
    }
}
