#![allow(clippy::pedantic)]
//! Tests golden `change_aura_skill_config` (table de remplacement d'aura par personnage) —
//! VRAIES valeurs du dump RDBN à listes extrait du VFS :
//! `data/common/gamedata/skill/change_aura_skill_config_1.01.73.00.cfg.bin`
//! (décodé en forme iecode par `nie-model-serve::cfgbin_to_iecode_root`).
//!
//! Référence de portage : `packages/inagle/src/parsers/change-aura-skill-config.ts`
//! (`parseContent`, `buildChangeAuraSkillDatabase`). Le dump réel contient 227 lignes
//! `m_ChangeAuraSkillDataList` + 96 lignes `m_ChangeAuraSkillInfoList` ; la fixture ci-dessous
//! embarque les **8 premières lignes réelles de chaque liste** (têtes de dump), suffisantes pour
//! valider parsing, sentinelle générique `0x00000000` et résolution du slice `[offset, count]`.

use nie_data::change_aura_skill_config::{
    ChangeAuraSkillDatabase, GENERIC_CHARA_PARAM, parse_change_aura_skill_config,
};
use nie_data::hash::HashId;
use serde_json::json;

/// Fixture = forme iecode `{ "lists": [...] }`, têtes (8 lignes) des deux listes réelles du dump
/// `change_aura_skill_config_1.01.73.00.cfg.bin`.
fn root_fixture() -> serde_json::Value {
    json!({
        "lists": [
            {
                "name": "m_ChangeAuraSkillDataList",
                "typeName": "CHANGE_AURA_SKILL_DATA",
                "values": [
                    { "id": "0xDD08BEB4", "charaParamId": "0x7AB4BBF3" },
                    { "id": "0xDD08BEB4", "charaParamId": "0xC40C8017" },
                    { "id": "0xDD08BEB4", "charaParamId": "0xEDC434E5" },
                    { "id": "0xDD08BEB4", "charaParamId": "0xF6205F4E" },
                    { "id": "0xAA0F8E22", "charaParamId": "0xCEE7150E" },
                    { "id": "0xC4138FF5", "charaParamId": "0x00000000" },
                    { "id": "0x8B521932", "charaParamId": "0xDEAE22C0" },
                    { "id": "0xFC5529A4", "charaParamId": "0xA9AD6367" }
                ]
            },
            {
                "name": "m_ChangeAuraSkillInfoList",
                "typeName": "CHANGE_AURA_SKILL_INFO",
                "values": [
                    { "id": "0x7978E1FA", "data": [0, 5] },
                    { "id": "0x6063D0BB", "data": [5, 1] },
                    { "id": "0x2F22467C", "data": [6, 2] },
                    { "id": "0x3639773D", "data": [8, 2] },
                    { "id": "0x9908EC5F", "data": [10, 6] },
                    { "id": "0x62256EE2", "data": [16, 1] },
                    { "id": "0x29E98497", "data": [17, 1] },
                    { "id": "0x56D8CE8B", "data": [18, 1] }
                ]
            }
        ]
    })
}

fn parsed() -> ChangeAuraSkillDatabase {
    parse_change_aura_skill_config(&root_fixture())
}

#[test]
fn comptes_des_deux_listes() {
    let db = parsed();
    assert_eq!(
        db.data.len(),
        8,
        "8 lignes CHANGE_AURA_SKILL_DATA dans la fixture"
    );
    assert_eq!(
        db.info.len(),
        8,
        "8 lignes CHANGE_AURA_SKILL_INFO dans la fixture"
    );
}

#[test]
fn data_list_valeurs_reelles() {
    let db = parsed();
    // 1re ligne : id 0xDD08BEB4, charaParamId 0x7AB4BBF3.
    assert_eq!(db.data[0].id, HashId(0xDD08_BEB4));
    assert_eq!(db.data[0].chara_param_id, HashId(0x7AB4_BBF3));
    assert!(!db.data[0].is_generic());
    // 4 premières lignes partagent le même id (0xDD08BEB4).
    assert!(db.data[..4].iter().all(|d| d.id == HashId(0xDD08_BEB4)));
    assert_eq!(db.data[3].chara_param_id, HashId(0xF620_5F4E));
    // 5e ligne : nouvel id.
    assert_eq!(db.data[4].id, HashId(0xAA0F_8E22));
    assert_eq!(db.data[4].chara_param_id, HashId(0xCEE7_150E));
}

#[test]
fn sentinelle_generique_0x00000000() {
    let db = parsed();
    // 6e ligne (index 5) : charaParamId générique 0x00000000.
    let g = &db.data[5];
    assert_eq!(g.id, HashId(0xC413_8FF5));
    assert_eq!(g.chara_param_id, GENERIC_CHARA_PARAM);
    assert_eq!(g.chara_param_id, HashId::ZERO);
    assert!(g.is_generic(), "0x00000000 ⇒ règle générique");
    // Les autres lignes ne sont pas génériques.
    assert_eq!(db.data.iter().filter(|d| d.is_generic()).count(), 1);
}

#[test]
fn get_chara_for_skill_last_write_wins() {
    let db = parsed();
    // 0xDD08BEB4 a 4 lignes (0..4) ; inagle (Map.set) garde la DERNIÈRE → 0xF6205F4E.
    assert_eq!(
        db.get_chara_for_skill(HashId(0xDD08_BEB4)),
        Some(HashId(0xF620_5F4E)),
        "last-write-wins comme bySkillId.set d'inagle"
    );
    // id unique non générique.
    assert_eq!(
        db.get_chara_for_skill(HashId(0xAA0F_8E22)),
        Some(HashId(0xCEE7_150E))
    );
    // id générique → None (charaParamId == 0x00000000).
    assert_eq!(db.get_chara_for_skill(HashId(0xC413_8FF5)), None);
    // id absent → None.
    assert_eq!(db.get_chara_for_skill(HashId(0xDEAD_BEEF)), None);
}

#[test]
fn info_list_offset_count_et_slice() {
    let db = parsed();
    // 1re info : aura 0x7978E1FA, data [0, 5].
    let i0 = &db.info[0];
    assert_eq!(i0.id, HashId(0x7978_E1FA));
    assert_eq!(i0.data, vec![0, 5]);
    assert_eq!(i0.data_offset(), 0);
    assert_eq!(i0.data_count(), 5);
    // Le slice désigne les 5 premières lignes de la DataList.
    let slice = db.slice_for_info(i0);
    assert_eq!(slice.len(), 5);
    assert_eq!(slice[0].id, HashId(0xDD08_BEB4));
    assert_eq!(slice[4].id, HashId(0xAA0F_8E22));

    // 2e info : offset 5, count 1 → la ligne générique 0xC4138FF5.
    let i1 = &db.info[1];
    assert_eq!(i1.id, HashId(0x6063_D0BB));
    assert_eq!(i1.data_offset(), 5);
    let slice1 = db.slice_for_info(i1);
    assert_eq!(slice1.len(), 1);
    assert_eq!(slice1[0].id, HashId(0xC413_8FF5));
    assert!(slice1[0].is_generic());
}

#[test]
fn partition_contigue_des_infos() {
    // Vérité terrain : offset[i+1] == offset[i] + count[i] (partition contiguë de la DataList).
    let db = parsed();
    let mut attendu = 0usize;
    for info in &db.info {
        assert_eq!(
            info.data_offset(),
            attendu,
            "partition contiguë (offset suit le cumul)"
        );
        attendu = info.data_offset() + info.data_count();
    }
    // Après les 8 premières infos, le cumul des counts = 19 (0xDD08BEB4×4 + … cf. dump).
    assert_eq!(attendu, 19);
}

#[test]
fn lists_absentes_donnent_des_vecs_vides() {
    // Port de `if (!content?.lists) return { data, info }` : root sans `lists` → DB vide.
    let db = parse_change_aura_skill_config(&json!({}));
    assert!(db.data.is_empty());
    assert!(db.info.is_empty());
    assert_eq!(db.get_chara_for_skill(HashId(0xDD08_BEB4)), None);
}
