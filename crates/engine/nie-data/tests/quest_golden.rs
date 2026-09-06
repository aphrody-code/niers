#![allow(clippy::pedantic)]
//! Tests golden `quest` — noeuds réels `QUEST_DATA_CFG` tirés de :
//! `data/common/gamedata/quest/quest_config_1.04.11.00.cfg.bin` (VFS IEVR).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/quest-config.ts` (fn `parseQuestNode` /
//! `loadQuestConfig`) : var\[0\]=questId (Int→hex), var\[1\]=phase, var\[2\]=type,
//! var\[3\]=titleHash (Int→hex), var\[16\]=image (String optionnelle). Les enfants
//! `QUEST_DATA_CFG_REF_*` sont sautés. Vérité terrain = le dump brut, valeurs extraites via
//! `cargo run -p nie-game --example extract_quest`.

use nie_data::hash::HashId;
use nie_data::quest::{QuestType, parse_quest_config};
use serde_json::{Value, json};

/// Une variable typée du dump (`type`, `value`), toujours chaîne côté JSON.
fn var(ty: &str, value: &str) -> Value {
    json!({ "type": ty, "value": value })
}

/// Les 6 premiers noeuds `QUEST_DATA_CFG` réels (20 variables chacun), copiés tels quels
/// depuis le dump `quest_config_1.04.11.00.cfg.bin`. Types : tout `Int` sauf var\[5\],
/// var\[16\], var\[17\] qui sont des `String`.
fn quest_node(name: &str, vars: &[(&str, &str)]) -> Value {
    let variables: Vec<Value> = vars.iter().map(|(t, v)| var(t, v)).collect();
    json!({ "name": name, "variables": variables, "children": [] })
}

/// Reconstruit la racine iecode : `entries → QUEST_DATA_CFG_LIST_BEG_0 → enfants`.
fn root_fixture() -> Value {
    // 6 quêtes réelles (full 20 vars).
    let q0 = quest_node(
        "QUEST_DATA_CFG_0",
        &[
            ("Int", "1732101895"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "185538439"),
            ("Int", "0"),
            ("String", "AAAAAA8FNbkZNtoAAQAyAAAnJHE="),
            ("Int", "0"),
            ("Int", "2"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "1"),
            ("Int", "0"),
            ("String", "story_img001_l01"),
            ("String", "story_img001_l01.g4tx"),
            ("Int", "0"),
            ("Int", "0"),
        ],
    );
    let q1 = quest_node(
        "QUEST_DATA_CFG_1",
        &[
            ("Int", "272299921"),
            ("Int", "1"),
            ("Int", "1"),
            ("Int", "-1605602299"),
            ("Int", "1"),
            ("String", "AAAAAA8FNbkZNtoAAQAyAAAnJHE="),
            ("Int", "1"),
            ("Int", "2"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "1"),
            ("Int", "0"),
            ("String", "story_img001_s01"),
            ("String", "story_img001_s01.g4tx"),
            ("Int", "-246642999"),
            ("Int", "2"),
        ],
    );
    let q2 = quest_node(
        "QUEST_DATA_CFG_2",
        &[
            ("Int", "-111586652"),
            ("Int", "2"),
            ("Int", "2"),
            ("Int", "-682933101"),
            ("Int", "1"),
            ("String", "AAAAABgFNb4EpZgACgEoAAYCNPFMhskyAAAAA3E="),
            ("Int", "2"),
            ("Int", "2"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "1"),
            ("Int", "0"),
            ("String", "story_img001_s01"),
            ("String", "story_img001_s01.g4tx"),
            ("Int", "0"),
            ("Int", "0"),
        ],
    );
    let q3 = quest_node(
        "QUEST_DATA_CFG_3",
        &[
            ("Int", "-1993103829"),
            ("Int", "3"),
            ("Int", "3"),
            ("Int", "1312953641"),
            ("Int", "1"),
            ("String", "AAAAAA8FNbkZNtoAAQAyAAAnLnE="),
            ("Int", "3"),
            ("Int", "2"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "1"),
            ("Int", "0"),
            ("String", "story_img001_s02"),
            ("String", "story_img001_s02.g4tx"),
            ("Int", "0"),
            ("Int", "0"),
        ],
    );
    let q4 = quest_node(
        "QUEST_DATA_CFG_4",
        &[
            ("Int", "-30107971"),
            ("Int", "4"),
            ("Int", "4"),
            ("Int", "960832959"),
            ("Int", "2"),
            ("String", "AAAAAA8FNbkZNtoAAQAyAAAnQnE="),
            ("Int", "4"),
            ("Int", "2"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "1"),
            ("Int", "0"),
            ("String", "story_img001_s03"),
            ("String", "story_img001_s03.g4tx"),
            ("Int", "0"),
            ("Int", "0"),
        ],
    );
    let q5 = quest_node(
        "QUEST_DATA_CFG_5",
        &[
            ("Int", "1776701237"),
            ("Int", "5"),
            ("Int", "5"),
            ("Int", "1227870512"),
            ("Int", "2"),
            ("String", "AAAAABgFNSo9RUMACgEoAAYCNN1zAZsyAAAAAXg="),
            ("Int", "5"),
            ("Int", "2"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "0"),
            ("Int", "1"),
            ("Int", "0"),
            ("String", "story_img001_s03"),
            ("String", "story_img001_s03.g4tx"),
            ("Int", "0"),
            ("Int", "0"),
        ],
    );

    // 2 noeuds REF réels (2 vars chacun, à 0) — DOIVENT être sautés par le parseur.
    let ref_phase = quest_node("QUEST_DATA_CFG_REF_PHASE_0", &[("Int", "0"), ("Int", "0")]);
    let ref_reward = quest_node(
        "QUEST_DATA_CFG_REF_REWARD_ITEM_0",
        &[("Int", "0"), ("Int", "0")],
    );

    json!({
        "entries": [{
            "name": "QUEST_DATA_CFG_LIST_BEG_0",
            "variables": [{ "type": "Int", "value": "182" }],
            "children": [q0, ref_phase, ref_reward, q1, q2, q3, q4, q5]
        }]
    })
}

#[test]
fn quest_config_compte_et_skip_ref() {
    let quests = parse_quest_config(&root_fixture());
    // 6 quêtes réelles ; les 2 noeuds REF (placés entre q0 et q1) sont sautés.
    assert_eq!(quests.len(), 6, "exactement 6 quêtes, REF exclus");
}

#[test]
fn quest_data_cfg_0_champs_de_base() {
    let quests = parse_quest_config(&root_fixture());
    let q = &quests[0];

    // var0 = quest_id = 1732101895 → 0x673DC707.
    assert_eq!(q.quest_id, HashId::from_signed(1732101895));
    assert_eq!(q.quest_id.to_hex(), "0x673DC707");
    // var1 = phase = 0, var2 = type = 0.
    assert_eq!(q.phase, 0);
    assert_eq!(q.quest_type, 0);
    // var3 = title_hash = 185538439 → 0x0B0F1787.
    assert_eq!(q.title_hash, HashId::from_signed(185538439));
    assert_eq!(q.title_hash.to_hex(), "0x0B0F1787");
    // var16 = image.
    assert_eq!(q.image.as_deref(), Some("story_img001_l01"));
}

#[test]
fn quest_hashes_negatifs_reinterpretes_non_signes() {
    let quests = parse_quest_config(&root_fixture());

    // Quête 2 : id négatif -111586652 → 0xF95952A4, title -682933101 → 0xD74B4493.
    let q2 = &quests[2];
    assert_eq!(q2.quest_id, HashId::from_signed(-111586652));
    assert_eq!(q2.quest_id.to_hex(), "0xF95952A4");
    assert_eq!(q2.title_hash.to_hex(), "0xD74B4493");
    assert_eq!(q2.phase, 2);
    assert_eq!(q2.quest_type, 2);

    // Quête 1 : title négatif -1605602299 → 0xA04C7405.
    let q1 = &quests[1];
    assert_eq!(q1.quest_id.to_hex(), "0x103AF791");
    assert_eq!(q1.title_hash.to_hex(), "0xA04C7405");
    assert_eq!(q1.image.as_deref(), Some("story_img001_s01"));
}

#[test]
fn quest_toutes_les_images_et_phases() {
    let quests = parse_quest_config(&root_fixture());
    let attendus: [(&str, &str, i64, &str); 6] = [
        ("0x673DC707", "0x0B0F1787", 0, "story_img001_l01"),
        ("0x103AF791", "0xA04C7405", 1, "story_img001_s01"),
        ("0xF95952A4", "0xD74B4493", 2, "story_img001_s01"),
        ("0x8933A62B", "0x4E421529", 3, "story_img001_s02"),
        ("0xFE3496BD", "0x394525BF", 4, "story_img001_s03"),
        ("0x69E64F35", "0x492FD130", 5, "story_img001_s03"),
    ];
    assert_eq!(quests.len(), attendus.len());
    for (i, (id, title, phase, image)) in attendus.iter().enumerate() {
        assert_eq!(quests[i].quest_id.to_hex(), *id, "quête {i} id");
        assert_eq!(quests[i].title_hash.to_hex(), *title, "quête {i} title");
        assert_eq!(quests[i].phase, *phase, "quête {i} phase");
        assert_eq!(quests[i].image.as_deref(), Some(*image), "quête {i} image");
    }
}

#[test]
fn quest_type_classification_inagle() {
    // QuestType porté 1:1 : 1=Main, 2=Sub, 3=Daily, sinon None.
    assert_eq!(QuestType::from_raw(1), Some(QuestType::Main));
    assert_eq!(QuestType::from_raw(2), Some(QuestType::Sub));
    assert_eq!(QuestType::from_raw(3), Some(QuestType::Daily));
    assert_eq!(QuestType::from_raw(0), None);
    assert_eq!(QuestType::from_raw(5), None);
}

#[test]
fn quest_validation_vrai_fichier_si_present() {
    // Validation byte-à-byte contre le vrai dump si présent (gitignore / hors VFS export).
    // Source : data/common/gamedata/quest/quest_config_1.04.11.00.cfg.bin.json.
    let path =
        "/home/aphrody/niers/data/common/gamedata/quest/quest_config_1.04.11.00.cfg.bin.json";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", path));
    let root: Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    let quests = parse_quest_config(&root);
    assert_eq!(quests.len(), 182, "182 quêtes attendues dans le vrai dump");
    assert_eq!(quests[0].quest_id.to_hex(), "0x673DC707");
}
