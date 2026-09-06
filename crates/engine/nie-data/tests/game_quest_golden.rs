#![allow(clippy::pedantic)]
//! Golden `game_quest` — base des défis/quêtes de match, sur le vrai dump.
mod common;

use nie_data::game_quest::parse_game_quest_config;
use nie_data::hash::HashId;

const PATH: &str = "soccer/game_quest_config_1.02.33.cfg.bin.json";

fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    Some(serde_json::from_str(&std::fs::read_to_string(&chemin_abs).unwrap()).unwrap())
}

#[test]
fn comptes_et_valeurs() {
    let Some(root) = load() else { return };
    let cfg = parse_game_quest_config(&root);
    assert_eq!(cfg.icons.len(), 713);
    assert_eq!(cfg.quest_data.len(), 948);
    assert_eq!(cfg.game_infos.len(), 175);
    assert_eq!(cfg.game_quests.len(), 175);
    // quest_data[0] byte-exact.
    let q = &cfg.quest_data[0];
    assert_eq!(q.explain_text_id, HashId(0x223C_1733));
    assert_eq!(q.cond_count, 3);
    assert_eq!(q.limit, 0xFFFF_FFFF);
    // game_quest[0] + résolution objectifs.
    let gq = &cfg.game_quests[0];
    assert_eq!(gq.game_quest_id, HashId(0xF1D6_A24D));
    assert_eq!(gq.title_text_id, HashId(0x8CD2_A357));
    assert_eq!(gq.quest_data, [0, 4]);
    assert_eq!(cfg.quests_of(gq).len(), 4);
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key("game_quest_config_1.02.33.cfg.bin"),
        "game_quest_config"
    );
    let (label, json) = decode_by_key("game_quest_config", &root).expect("câblé");
    assert_eq!(label, "game_quest");
    assert_eq!(json["game_quests"].as_array().map(Vec::len), Some(175));
}
