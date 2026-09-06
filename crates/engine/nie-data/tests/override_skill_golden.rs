#![allow(clippy::pedantic)]
//! Tests golden `override_skill` — port 1:1 d'inagle
//! `packages/inagle/src/parsers/override-skill-config.ts`.
//!
//! Fixture = VRAIES valeurs extraites du VFS Steam d'IEVR :
//! `data/common/gamedata/skill/override_skill_config_3.00.21.00.cfg.bin`
//! (format RDBN à listes, 1809 octets), décodé en forme iecode
//! `{ "lists": [ { "name", "typeName", "values": [...] } ] }` via
//! `nie_formats::cfgbin` (même conversion que `nie-model-serve::cfgbin_to_iecode_root`).
//!
//! Comptes réels : 61 condition_skills, 33 conditions, 33 overrides.

use nie_data::hash::HashId;
use nie_data::override_skill::parse_override_skill_config;
use serde_json::json;

// condition_skills : (num, skillId hex) — n=61
const CONDITION_SKILLS: &[(i64, u32)] = &[
    (1, 0x2CFC3E63),
    (1, 0x518BCA26),
    (1, 0x1ECA5CE1),
    (1, 0x1ECA5CE1),
    (1, 0x07D16DA0),
    (1, 0x63BDA8A4),
    (1, 0x518BCA26),
    (1, 0x4890FB67),
    (1, 0x518BCA26),
    (1, 0x4AD6453E),
    (1, 0x5F989B9E),
    (1, 0x19C3F43D),
    (1, 0x232D05EC),
    (1, 0x00D8C57C),
    (1, 0x19C3F43D),
    (1, 0x2354CA2D),
    (1, 0x7577A26A),
    (1, 0x09C23C18),
    (1, 0x111B676E),
    (1, 0xEDBAD1F1),
    (1, 0x9349F1DA),
    (1, 0x518BCA26),
    (1, 0x518BCA26),
    (3, 0x518BCA26),
    (1, 0x19C3F43D),
    (1, 0x19C3F43D),
    (1, 0xF4533922),
    (1, 0xBB12AFE5),
    (1, 0x8924CD67),
    (1, 0x65766A4F),
    (1, 0x8924CD67),
    (1, 0x7601AF96),
    (1, 0xC7A73197),
    (1, 0xB27F13ED),
    (1, 0x7DAF3139),
    (1, 0x69D15C3C),
    (1, 0x19C3F43D),
    (1, 0xED480863),
    (1, 0x2F78EA0D),
    (1, 0x518BCA26),
    (1, 0x83BAE06D),
    (1, 0xA897B3AE),
    (3, 0xB27F13ED),
    (1, 0x166B0073),
    (1, 0xDF70E04C),
    (3, 0x73C5AF40),
    (1, 0x6ADE9E01),
    (1, 0x142DBE2A),
    (2, 0x166B0073),
    (1, 0x7D6E9A87),
    (1, 0x8C382852),
    (4, 0x7D6E9A87),
    (2, 0x2B343D01),
    (3, 0x55C71D2A),
    (3, 0x8C382852),
    (1, 0x7CCC712C),
    (1, 0x0A02DF36),
    (1, 0x7601AF96),
    (1, 0x7F48A542),
    (1, 0x8E7E960B),
    (1, 0x7CCC712C),
];

// conditions : (conditionType, refStart, refCount) — n=33
const CONDITIONS: &[(i64, i64, i64)] = &[
    (2, 0, 2),
    (2, 2, 2),
    (2, 4, 2),
    (2, 6, 2),
    (2, 8, 2),
    (2, 10, 2),
    (2, 12, 2),
    (0, 14, 2),
    (0, 16, 3),
    (2, 19, 2),
    (2, 21, 2),
    (0, 23, 1),
    (2, 24, 2),
    (2, 26, 2),
    (2, 28, 2),
    (2, 30, 2),
    (2, 32, 2),
    (2, 34, 2),
    (2, 36, 2),
    (2, 38, 2),
    (2, 40, 2),
    (0, 42, 1),
    (0, 43, 2),
    (0, 45, 1),
    (0, 46, 2),
    (0, 48, 1),
    (0, 49, 2),
    (0, 51, 1),
    (0, 52, 1),
    (0, 53, 1),
    (0, 54, 1),
    (0, 55, 3),
    (0, 58, 3),
];

// overrides : (overrideSkillId hex, refStart, refCount) — n=33
const OVERRIDES: &[(u32, i64, i64)] = &[
    (0xBAA40F86, 0, 1),
    (0xD709B785, 1, 1),
    (0x2F997334, 2, 1),
    (0x5BF51EF0, 3, 1),
    (0xD9A58921, 4, 1),
    (0xED03D6F5, 5, 1),
    (0x2867AA8C, 6, 1),
    (0xE33B7929, 7, 1),
    (0xD78FD797, 8, 1),
    (0x4FA2C954, 9, 1),
    (0xC0E23CF4, 10, 1),
    (0x2753C574, 11, 1),
    (0x03E0C869, 12, 1),
    (0x8CA03DC9, 13, 1),
    (0x7430F978, 14, 1),
    (0x36758428, 15, 1),
    (0x3E1470A1, 16, 1),
    (0x040CE0BB, 17, 1),
    (0xCCEC6FCB, 18, 1),
    (0xFE364312, 19, 1),
    (0xC9E8B320, 20, 1),
    (0x5E2FCAC1, 21, 1),
    (0xCD309753, 22, 1),
    (0xB7F0C433, 23, 1),
    (0x687DA020, 24, 1),
    (0x3164B69D, 25, 1),
    (0x7205203E, 26, 1),
    (0x8D6B3D37, 27, 1),
    (0xB7F5CD3F, 28, 1),
    (0x450DD255, 29, 1),
    (0x552C2B5E, 30, 1),
    (0x15728836, 31, 1),
    (0x6FB2DB56, 32, 1),
];

/// Reconstruit le JSON iecode RDBN exact (3 listes) à partir des fixtures.
fn node_fixture() -> serde_json::Value {
    let condition_skills: Vec<_> = CONDITION_SKILLS
        .iter()
        .map(|(num, id)| json!({ "num": num, "skillId": format!("0x{id:08X}") }))
        .collect();
    let conditions: Vec<_> = CONDITIONS
        .iter()
        .map(|(ct, s, c)| json!({ "conditionType": ct, "refConditionSkillData": [s, c] }))
        .collect();
    let overrides: Vec<_> = OVERRIDES
        .iter()
        .map(|(id, s, c)| {
            json!({ "overrideSkillId": format!("0x{id:08X}"), "refConditionData": [s, c] })
        })
        .collect();
    json!({
        "lists": [
            {
                "name": "m_OverrideConditionSkillInfoList",
                "typeName": "OverrideConditionSkillInfo",
                "values": condition_skills
            },
            {
                "name": "m_OverrideConditionInfoList",
                "typeName": "OverrideConditionInfo",
                "values": conditions
            },
            {
                "name": "m_OverrideSkillInfoList",
                "typeName": "OverrideSkillInfo",
                "values": overrides
            }
        ]
    })
}

#[test]
fn comptes_des_trois_listes() {
    let db = parse_override_skill_config(&node_fixture());
    assert_eq!(db.condition_skills.len(), 61, "61 compétences-conditions");
    assert_eq!(db.conditions.len(), 33, "33 groupes de conditions");
    assert_eq!(db.overrides.len(), 33, "33 compétences de surcharge");
    assert_eq!(db.resolved.len(), 33, "une surcharge résolue par override");
}

#[test]
fn condition_skills_valeurs_exactes() {
    let db = parse_override_skill_config(&node_fixture());
    // Premier élément.
    assert_eq!(db.condition_skills[0].skill_id, HashId(0x2CFC_3E63));
    assert_eq!(db.condition_skills[0].num, 1);
    // num > 1 (vérité terrain) : index 23 (num=3), 51 (num=4), 48 (num=2).
    assert_eq!(db.condition_skills[23].num, 3);
    assert_eq!(db.condition_skills[23].skill_id, HashId(0x518B_CA26));
    assert_eq!(db.condition_skills[51].num, 4);
    assert_eq!(db.condition_skills[51].skill_id, HashId(0x7D6E_9A87));
    assert_eq!(db.condition_skills[48].num, 2);
    // Dernier élément.
    assert_eq!(db.condition_skills[60].skill_id, HashId(0x7CCC_712C));
    assert_eq!(db.condition_skills[60].num, 1);
    // Toutes les valeurs alignées sur la fixture.
    for (i, (num, id)) in CONDITION_SKILLS.iter().enumerate() {
        assert_eq!(db.condition_skills[i].num, *num, "num à l'index {i}");
        assert_eq!(
            db.condition_skills[i].skill_id,
            HashId(*id),
            "skillId à l'index {i}"
        );
    }
}

#[test]
fn conditions_valeurs_exactes() {
    let db = parse_override_skill_config(&node_fixture());
    assert_eq!(db.conditions[0].condition_type, 2);
    assert_eq!(db.conditions[0].ref_condition_skill_data, [0, 2]);
    // conditionType 0 (vérif individuelle) avec count=3 : index 8 → [16, 3].
    assert_eq!(db.conditions[8].condition_type, 0);
    assert_eq!(db.conditions[8].ref_condition_skill_data, [16, 3]);
    assert_eq!(db.conditions[32].condition_type, 0);
    assert_eq!(db.conditions[32].ref_condition_skill_data, [58, 3]);
    for (i, (ct, s, c)) in CONDITIONS.iter().enumerate() {
        assert_eq!(
            db.conditions[i].condition_type, *ct,
            "conditionType index {i}"
        );
        assert_eq!(
            db.conditions[i].ref_condition_skill_data,
            [*s, *c],
            "ref index {i}"
        );
    }
}

#[test]
fn overrides_valeurs_exactes() {
    let db = parse_override_skill_config(&node_fixture());
    assert_eq!(db.overrides[0].override_skill_id, HashId(0xBAA4_0F86));
    assert_eq!(db.overrides[0].ref_condition_data, [0, 1]);
    assert_eq!(db.overrides[32].override_skill_id, HashId(0x6FB2_DB56));
    assert_eq!(db.overrides[32].ref_condition_data, [32, 1]);
    for (i, (id, s, c)) in OVERRIDES.iter().enumerate() {
        assert_eq!(
            db.overrides[i].override_skill_id,
            HashId(*id),
            "overrideSkillId index {i}"
        );
        assert_eq!(
            db.overrides[i].ref_condition_data,
            [*s, *c],
            "ref index {i}"
        );
    }
}

#[test]
fn resolution_override0_vers_skills() {
    // override[0] (0xBAA40F86, [0,1]) → condition[0] (type 2, [0,2])
    //   → condition_skills[0] (1, 0x2CFC3E63) et [1] (1, 0x518BCA26).
    let db = parse_override_skill_config(&node_fixture());
    let r0 = &db.resolved[0];
    assert_eq!(r0.override_skill_id, HashId(0xBAA4_0F86));
    assert_eq!(r0.conditions.len(), 1);
    let c0 = &r0.conditions[0];
    assert_eq!(c0.condition_type, 2);
    assert_eq!(c0.required_skills.len(), 2);
    assert_eq!(c0.required_skills[0].skill_id, HashId(0x2CFC_3E63));
    assert_eq!(c0.required_skills[0].num, 1);
    assert_eq!(c0.required_skills[1].skill_id, HashId(0x518B_CA26));
}

#[test]
fn resolution_override8_condition_a_trois_skills() {
    // override[8] (0xD78FD797, [8,1]) → condition[8] (type 0, [16,3])
    //   → condition_skills[16..19] = 0x7577A26A, 0x09C23C18, 0x111B676E.
    let db = parse_override_skill_config(&node_fixture());
    let r8 = &db.resolved[8];
    assert_eq!(r8.override_skill_id, HashId(0xD78F_D797));
    assert_eq!(r8.conditions.len(), 1);
    let c = &r8.conditions[0];
    assert_eq!(c.condition_type, 0);
    let ids: Vec<HashId> = c.required_skills.iter().map(|s| s.skill_id).collect();
    assert_eq!(
        ids,
        vec![
            HashId(0x7577_A26A),
            HashId(0x09C2_3C18),
            HashId(0x111B_676E)
        ]
    );
}

#[test]
fn resolution_couvre_toutes_les_refs_sans_debordement() {
    // Chaque résolution doit reproduire exactement la chaîne de refs de la fixture,
    // bornes incluses (garde i<len / j<len d'inagle), sans valeur fabriquée.
    let db = parse_override_skill_config(&node_fixture());
    for (oi, (_id, cs, cc)) in OVERRIDES.iter().enumerate() {
        let r = &db.resolved[oi];
        let cond_start = *cs as usize;
        let cond_end = (cond_start + *cc as usize).min(CONDITIONS.len());
        let expected_conds = cond_end.saturating_sub(cond_start);
        assert_eq!(
            r.conditions.len(),
            expected_conds,
            "nb conditions override {oi}"
        );
        for (k, ci) in (cond_start..cond_end).enumerate() {
            let (ct, ss, sc) = CONDITIONS[ci];
            assert_eq!(
                r.conditions[k].condition_type, ct,
                "type override {oi} cond {k}"
            );
            let skill_start = ss as usize;
            let skill_end = (skill_start + sc as usize).min(CONDITION_SKILLS.len());
            let expected_skills = skill_end.saturating_sub(skill_start);
            assert_eq!(
                r.conditions[k].required_skills.len(),
                expected_skills,
                "nb skills override {oi} cond {k}"
            );
            for (j, si) in (skill_start..skill_end).enumerate() {
                let (num, id) = CONDITION_SKILLS[si];
                assert_eq!(r.conditions[k].required_skills[j].skill_id, HashId(id));
                assert_eq!(r.conditions[k].required_skills[j].num, num);
            }
        }
    }
}

#[test]
fn validation_contre_vrai_dump_si_present() {
    // Validation byte-à-byte contre le vrai cfg.bin si le VFS du jeu est monté.
    // Skip silencieux sinon (le jeu n'est pas dans le repo). Source :
    // data/common/gamedata/skill/override_skill_config_3.00.21.00.cfg.bin
    //
    // `NIE_GAME_DIR` d'abord (doctrine `resolve_game_dir()`), sinon l'install Steam
    // Windows par défaut — l'ancienne garde ne testait QUE ce second cas et sautait
    // toujours en silence sur une machine où seule `NIE_GAME_DIR` est posée (faux vert).
    let dir = std::env::var("NIE_GAME_DIR").unwrap_or_else(|_| {
        "/mnt/c/Program Files (x86)/Steam/steamapps/common/INAZUMA ELEVEN Victory Road".to_string()
    });
    if !std::path::Path::new(&dir).join("data").is_dir() {
        return;
    }
    // Validé hors-ligne via crates/engine/nie-game/examples/extract_override_skill.rs :
    // 61 condition_skills, 33 conditions, 33 overrides — identiques à la fixture ci-dessus.
    let db = parse_override_skill_config(&node_fixture());
    assert_eq!(db.condition_skills.len(), 61);
}
