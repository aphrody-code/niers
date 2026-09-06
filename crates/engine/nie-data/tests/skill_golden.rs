#![allow(clippy::pedantic)]
//! Tests golden `skill` — valeurs réelles tirées de :
//! `skill/skill_config_4.00.17.00.cfg.bin.json`
//! (première valeur de `m_skillInfoList`, whs00010) et
//! `/home/ubuntu/niers/data/common/text/fr/skill_text.cfg.bin.json` (nom/desc whs00010).

mod common;

use nie_data::hash::HashId;
use nie_data::skill::{
    SkillCategory, SkillElement, SkillInfo, SkillPartnerType, join_skill_text, parse_skill_config,
    parse_skill_text,
};
use serde_json::json;

/// Fixture : un `skill_config` réduit à la 1re valeur réelle de `m_skillInfoList` (whs00010).
/// Octet-pour-octet identique au dump réel (28 champs).
fn skill_config_fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            { "name": "m_skillOptionInfoList", "typeName": "SKILL_OPTION_INFO", "values": [] },
            {
                "name": "m_skillInfoList",
                "typeName": "SKILL_INFO",
                "values": [{
                    "skillID": "0x63BDA8A4",
                    "skillIDStr": "whs00010",
                    "eventID": "0xD401CA3E",
                    "eventIDName": "ev60_00010",
                    "failEventID": "0x00000000",
                    "failEventIDName": "",
                    "skillNameId": "0x07ADF4B1",
                    "skillDescId": "0x87AB9FB9",
                    "cmdOptIdx": 2,
                    "skillEffectBitFlag": 16,
                    "power_min": 70,
                    "power_max": 440,
                    "element": 1,
                    "colorIdx": 1,
                    "category": 1,
                    "growthType": 4,
                    "foulRate": 0,
                    "consumeTp": 70,
                    "focusBattleEffectId": "0x00000000",
                    "recastTime": 90,
                    "partnerType": 2,
                    "partner1": "0xAB97A3D2",
                    "partner2": "0x00000000",
                    "partner3": "0x00000000",
                    "telopInfoId": "0x5B6A04E9",
                    "eldorado": false,
                    "seriesIdCrc": "0x62E8448F",
                    "isDisablePlayableUntilNextPatch": false
                }]
            }
        ]
    })
}

#[test]
fn skill_info_whs00010_28_champs() {
    let root = skill_config_fixture();
    let skills = parse_skill_config(&root);
    assert_eq!(skills.len(), 1, "une seule valeur dans la fixture");
    let s: &SkillInfo = &skills[0];

    // Identité / hash.
    assert_eq!(s.skill_id, HashId(0x63BD_A8A4));
    assert_eq!(s.skill_id_str, "whs00010");
    assert_eq!(s.event_id, HashId(0xD401_CA3E));
    assert_eq!(s.event_id_name, "ev60_00010");
    assert_eq!(s.fail_event_id, HashId::ZERO);
    assert_eq!(s.fail_event_id_name, "");
    assert_eq!(s.skill_name_id, HashId(0x07AD_F4B1));
    assert_eq!(s.skill_desc_id, HashId(0x87AB_9FB9));

    // Numériques.
    assert_eq!(s.cmd_opt_idx, 2);
    assert_eq!(s.skill_effect_bit_flag, 16);
    assert_eq!(s.power_min, 70);
    assert_eq!(s.power_max, 440);
    assert_eq!(s.element, 1);
    assert_eq!(s.color_idx, 1);
    assert_eq!(s.category, 1);
    assert_eq!(s.growth_type, 4);
    assert_eq!(s.foul_rate, 0);
    assert_eq!(s.consume_tp, 70);
    assert_eq!(s.recast_time, 90);
    assert_eq!(s.partner_type, 2);

    // Hash partenaires / divers.
    assert_eq!(s.focus_battle_effect_id, HashId::ZERO);
    assert_eq!(s.partner1, HashId(0xAB97_A3D2));
    assert_eq!(s.partner2, HashId::ZERO);
    assert_eq!(s.partner3, HashId::ZERO);
    assert_eq!(s.telop_info_id, HashId(0x5B6A_04E9));
    assert!(!s.eldorado);

    // Champs réels du dump v4 absents du type TS.
    assert_eq!(s.series_id_crc, HashId(0x62E8_448F));
    assert!(!s.is_disable_playable_until_next_patch);

    // Enums typés.
    assert_eq!(s.element(), SkillElement::Wind);
    assert_eq!(s.category(), SkillCategory::Shoot);
    assert_eq!(s.partner_type(), SkillPartnerType::Trio);
    assert_eq!(s.element().names(), Some(("Vent", "Wind", "風")));
    assert_eq!(s.category().names(), Some(("Tir", "Shoot", "シュート")));
}

/// Fixture skill_text réelle (FR) : nom + description de whs00010.
fn skill_text_fixture() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "TEXT_INFO_BEGIN_0",
                "variables": [],
                "children": [{
                    "name": "TEXT_INFO_0",
                    "variables": [
                        { "type": "Int", "value": "-2018795591" },
                        { "type": "Int", "value": "0" },
                        { "type": "String", "value": "En vous sautant dessus comme un tremplin,\nvotre coéquipier exécutera un tir foudroyant." }
                    ],
                    "children": []
                }]
            },
            {
                "name": "NOUN_INFO_BEGIN_0",
                "variables": [],
                "children": [{
                    "name": "NOUN_INFO_0",
                    "variables": [
                        { "type": "Int", "value": "128840881" },
                        { "type": "Int", "value": "0" },
                        { "type": "String", "value": "" },
                        { "type": "String", "value": "" },
                        { "type": "String", "value": "" },
                        { "type": "String", "value": "Trampoline du tonnerre" }
                    ],
                    "children": []
                }]
            }
        ]
    })
}

#[test]
fn skill_text_join_whs00010() {
    // Le hash du nom (skillNameId) = 0x07ADF4B1 ; en décimal signé = 128840881.
    // -2018795591 >>> 0 = 0x87AB9FB9 = skillDescId de whs00010 → vérifie la jointure desc.
    assert_eq!(HashId::from_signed(128_840_881), HashId(0x07AD_F4B1));
    assert_eq!(HashId::from_signed(-2_018_795_591), HashId(0x87AB_9FB9));

    let text = parse_skill_text(&skill_text_fixture());
    assert_eq!(
        text.names.get(&HashId(0x07AD_F4B1)).map(String::as_str),
        Some("Trampoline du tonnerre")
    );
    assert_eq!(
        text.descriptions
            .get(&HashId(0x87AB_9FB9))
            .map(String::as_str),
        Some(
            "En vous sautant dessus comme un tremplin,\nvotre coéquipier exécutera un tir foudroyant."
        )
    );

    // Jointure complète skill ↔ texte.
    let skills = parse_skill_config(&skill_config_fixture());
    let joined = join_skill_text(&skills[0], &text);
    assert_eq!(joined.name.as_deref(), Some("Trampoline du tonnerre"));
    assert!(
        joined
            .description
            .unwrap()
            .starts_with("En vous sautant dessus")
    );
}
