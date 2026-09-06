#![allow(clippy::pedantic)]
//! Tests golden `skill_technic` — liste réelle `m_SkillTechnicInfoList` tirée de :
//! `data/common/gamedata/skill/skill_technic_config_1.01.28.00.cfg.bin` (VFS du jeu, RDBN).
//!
//! Extraction : `cargo run -p nie-model-serve --example extract_skill_technic` (jetable),
//! qui décode le RDBN via `nie_formats::cfgbin` puis le projette en JSON iecode
//! (`Hash → "0x........"`, `Byte`/`Float → nombre`). Port 1:1 d'inagle
//! `packages/inagle/src/parsers/skill-technic-config.ts` (`parseContent`, l.44-66).
//!
//! Vérité terrain = la sortie de l'exemple d'extraction sur le vrai nœud du VFS :
//! 6 valeurs, 5 champs (`id`, `winSubMotionNameCrc`, `loseSubMotionNameCrc`, `formationType`,
//! `formationCharaLen`). Toutes les `formationType = 0` et `formationCharaLen = 3.0` ; seul
//! l'index 3 a un `winSubMotionNameCrc` non nul (`0xF03A0B78`).

use nie_data::hash::HashId;
use nie_data::skill_technic::{SkillTechnicInfo, build_by_id, parse_skill_technic_config};
use serde_json::json;

/// Les 6 lignes exactes de `m_SkillTechnicInfoList` (dump réel 1.01.28.00).
/// `(id, winSubMotionNameCrc, loseSubMotionNameCrc)` en hex non signé.
const RAW: &[(&str, &str, &str)] = &[
    ("0xA2123954", "0x00000000", "0x00000000"),
    ("0x893F6A97", "0x00000000", "0x00000000"),
    ("0x90245BD6", "0x00000000", "0x00000000"),
    ("0xD5C20845", "0xF03A0B78", "0x00000000"),
    ("0xFEEF5B86", "0x00000000", "0x00000000"),
    ("0xE7F46AC7", "0x00000000", "0x00000000"),
];

fn root_fixture() -> serde_json::Value {
    let values: Vec<_> = RAW
        .iter()
        .map(|(id, win, lose)| {
            json!({
                "id": id,
                "winSubMotionNameCrc": win,
                "loseSubMotionNameCrc": lose,
                "formationType": 0,
                "formationCharaLen": 3.0
            })
        })
        .collect();
    json!({
        "lists": [{
            "name": "m_SkillTechnicInfoList",
            "typeName": "SkillTechnicInfo",
            "values": values
        }]
    })
}

#[test]
fn skill_technic_six_entrees() {
    let parsed = parse_skill_technic_config(&root_fixture());
    assert_eq!(
        parsed.len(),
        6,
        "exactement 6 entrées dans m_SkillTechnicInfoList"
    );
}

#[test]
fn skill_technic_premiere_entree() {
    let parsed = parse_skill_technic_config(&root_fixture());
    let p: &SkillTechnicInfo = &parsed[0];
    assert_eq!(p.id, HashId(0xA212_3954));
    assert_eq!(p.win_sub_motion_name_crc, HashId::ZERO);
    assert_eq!(p.lose_sub_motion_name_crc, HashId::ZERO);
    assert_eq!(p.formation_type, 0);
    assert_eq!(p.formation_chara_len, 3.0);
    assert_eq!(p.id.to_hex(), "0xA2123954");
}

#[test]
fn skill_technic_tous_les_ids() {
    let parsed = parse_skill_technic_config(&root_fixture());
    let ids: Vec<HashId> = parsed.iter().map(|p| p.id).collect();
    assert_eq!(
        ids,
        [
            HashId(0xA212_3954),
            HashId(0x893F_6A97),
            HashId(0x9024_5BD6),
            HashId(0xD5C2_0845),
            HashId(0xFEEF_5B86),
            HashId(0xE7F4_6AC7),
        ]
    );
}

#[test]
fn skill_technic_index3_win_sub_motion_non_nul() {
    // Seule ligne du dump avec un winSubMotionNameCrc non nul : index 3, id 0xD5C20845.
    let parsed = parse_skill_technic_config(&root_fixture());
    let p = &parsed[3];
    assert_eq!(p.id, HashId(0xD5C2_0845));
    assert_eq!(p.win_sub_motion_name_crc, HashId(0xF03A_0B78));
    assert_eq!(p.lose_sub_motion_name_crc, HashId::ZERO);
    // Les autres ont bien un winSubMotionNameCrc nul.
    for (i, e) in parsed.iter().enumerate() {
        if i != 3 {
            assert_eq!(e.win_sub_motion_name_crc, HashId::ZERO, "ligne {i}");
        }
    }
}

#[test]
fn skill_technic_formation_constante() {
    // Toutes les lignes du dump 1.01.28.00 : formationType = 0, formationCharaLen = 3.0.
    let parsed = parse_skill_technic_config(&root_fixture());
    for (i, e) in parsed.iter().enumerate() {
        assert_eq!(e.formation_type, 0, "ligne {i} formationType");
        assert_eq!(e.formation_chara_len, 3.0, "ligne {i} formationCharaLen");
    }
}

#[test]
fn skill_technic_index_par_id() {
    let parsed = parse_skill_technic_config(&root_fixture());
    let map = build_by_id(&parsed);
    assert_eq!(map.len(), 6);
    let e = map.get(&HashId(0xD5C2_0845)).expect("id 0xD5C20845 indexé");
    assert_eq!(e.win_sub_motion_name_crc, HashId(0xF03A_0B78));
    assert!(map.contains_key(&HashId(0xA212_3954)));
    assert!(!map.contains_key(&HashId(0x0000_0000)));
}

#[test]
fn skill_technic_liste_absente_vide() {
    // Robustesse : root sans la liste → vecteur vide (pas de panique).
    let empty = json!({ "lists": [] });
    assert!(parse_skill_technic_config(&empty).is_empty());
}
