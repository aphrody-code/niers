#![allow(clippy::pedantic)]
//! Tests golden `ai_type` — types/hobbies d'IA d'IEVR, sur le vrai dump
//! (skip silencieux si absent — fragment copyright Level-5 non committé) :
//!
//! - `data/common/gamedata/system/ai_type_config.cfg.bin.json`
//!
//! Port direct (pattern entrée+REF) ; valeurs assérées copiées octet pour octet, y compris la
//! résolution du nœud frère REF_HOBY.

mod common;

use nie_data::ai_type::parse_ai_type_config;
use nie_data::hash::HashId;

const PATH: &str = "system/ai_type_config.cfg.bin.json";

fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("lecture {}: {e}", chemin_abs.display()));
    Some(
        serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("JSON {}: {e}", chemin_abs.display())),
    )
}

#[test]
fn comptes_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_ai_type_config(&root);
    assert_eq!(cfg.hobies.len(), 4);
    assert_eq!(
        cfg.ai_infos.len(),
        5,
        "5 infos (les 5 nœuds REF ne sont pas des entrées)"
    );
    assert_eq!(cfg.hobies[0].params, [1, 20, 0]);
    assert_eq!(cfg.hobies[3].params, [20, 10, 0]);
}

#[test]
fn ai_infos_et_refs_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_ai_type_config(&root);
    // ai_id signés → u32 ; slices REF_HOBY.
    assert_eq!(cfg.ai_infos[0].ai_id, HashId(0xC9E1_1EB8)); // -907993416
    assert_eq!(cfg.ai_infos[0].params, [1, 1]);
    assert_eq!(cfg.ai_infos[0].hoby_slice, [0, 2]);
    assert_eq!(cfg.ai_infos[1].ai_id, HashId(0x50E8_4F02)); // 1357401858
    assert_eq!(cfg.ai_infos[1].hoby_slice, [2, 1]);
    assert_eq!(cfg.ai_infos[3].ai_id, HashId(0xB98B_EA37)); // -1182012873
    assert_eq!(cfg.ai_infos[3].hoby_slice, [3, 1]);
    assert_eq!(cfg.ai_infos[4].ai_id, HashId(0xCE8C_DAA1)); // -829629791
}

#[test]
fn resolution_ref_hoby() {
    let Some(root) = load() else { return };
    let cfg = parse_ai_type_config(&root);
    // ai_infos[0].hoby_slice [0,2] → hobies[0],[1].
    let h = cfg.hobies_of(&cfg.ai_infos[0]);
    assert_eq!(h.len(), 2);
    assert_eq!(h[0].params, [1, 20, 0]);
    // ai_infos[3].hoby_slice [3,1] → hobies[3].
    let h3 = cfg.hobies_of(&cfg.ai_infos[3]);
    assert_eq!(h3.len(), 1);
    assert_eq!(h3[0].params, [20, 10, 0]);
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_atteint_azalee() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(family_key("ai_type_config.cfg.bin"), "ai_type_config");
    let (label, json) = decode_by_key("ai_type_config", &root).expect("famille typée câblée");
    assert_eq!(label, "ai_type");
    assert_eq!(json["ai_infos"].as_array().map(Vec::len), Some(5));
    assert_eq!(json["hobies"].as_array().map(Vec::len), Some(4));
}
