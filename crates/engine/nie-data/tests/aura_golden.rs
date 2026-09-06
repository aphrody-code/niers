#![allow(clippy::pedantic)]
//! Tests golden `aura` — noeud réel `AURA_CMD_INFO_0` tiré de :
//! `skill/aura_skill_config_1.04.09.00.cfg.bin.json`.
//!
//! Valeurs (19 vars) : `[2037965306, "wks00020", 493403631, -1653680409, 30, 60, 260858381,
//! -1368456794, 3, 8, 0, 1, -1124324279, 0, 0, 0, 1, 0, 0]`.
//! → auraId=0x7978E1FA, assetCode=wks00020, skillId1(var6)=0x0F8C620D, element(var8)=3.
//!
//! Résolution hissatsu : AURA_CMD_INFO_0 skillId1 0x0F8C620D résout vers **whs01780**
//! (Feu/Tir, 100-640) dans le vrai skill_config_4. Le fichier contient 387 AURA_CMD_INFO
//! réels (19 vars) ; les 1161 AURA_CMD_INFO_REF (2 vars) sont filtrés (var_count < 4).
//! La conclusion « 0/1549 → None » était hallucinée (bun-check bugué).
//! Tests : carte vide → None ; inline whs01780 ; vrais dumps fichiers (skip si absent VPS).

mod common;

use nie_data::aura::{
    AuraCmd, AuraSubType, build_skill_map, determine_sub_type, parse_all_aura_cmds,
    resolve_aura_hissatsu,
};
use nie_data::hash::HashId;
use nie_data::skill::{SkillCategory, SkillElement, SkillInfo};
use serde_json::json;

fn aura_node_fixture() -> serde_json::Value {
    let raw: [(&str, &str); 19] = [
        ("Int", "2037965306"),
        ("String", "wks00020"),
        ("Int", "493403631"),
        ("Int", "-1653680409"),
        ("Int", "30"),
        ("Int", "60"),
        ("Int", "260858381"),
        ("Int", "-1368456794"),
        ("Int", "3"),
        ("Int", "8"),
        ("Int", "0"),
        ("Int", "1"),
        ("Int", "-1124324279"),
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "0"),
        ("Int", "1"),
        ("Int", "0"),
        ("Int", "0"),
    ];
    let variables: Vec<_> = raw
        .iter()
        .map(|(t, v)| json!({ "type": t, "value": v }))
        .collect();
    json!({
        "entries": [{
            "name": "AURA_CMD_INFO_0",
            "variables": variables,
            "children": []
        }]
    })
}

#[test]
fn aura_cmd_info_0_champs() {
    let auras = parse_all_aura_cmds(&aura_node_fixture());
    assert_eq!(auras.len(), 1);
    let a: &AuraCmd = &auras[0];

    assert_eq!(a.aura_id, HashId::from_signed(2037965306));
    assert_eq!(a.aura_id.to_hex(), "0x7978E1FA");
    assert_eq!(a.asset_code, "wks00020");
    assert_eq!(a.name_id, HashId::from_signed(493403631));
    assert_eq!(a.desc_id, HashId::from_signed(-1653680409));
    assert_eq!(a.element, 3); // var8 ∈ [0,4]
    assert_eq!(a.element(), SkillElement::Fire);

    // Sous-type : préfixe wks (pas de "totem"/"soul" dans le nom) → Keshin.
    assert_eq!(a.sub_type, AuraSubType::Keshin);
    assert_eq!(a.sub_type.label_fr(), "Esprit Guerrier");

    // Sous-config.
    assert_eq!(a.config.val4, Some(30));
    assert_eq!(a.config.val5, Some(60));
    assert_eq!(a.config.skill_id1, Some(HashId::from_signed(260858381)));
    assert_eq!(a.config.skill_id1.unwrap().to_hex(), "0x0F8C620D");
    assert_eq!(a.config.skill_id2, Some(HashId::from_signed(-1368456794)));
    assert_eq!(a.config.val9, Some(8));
    assert_eq!(a.config.val11, Some(1));
    assert_eq!(a.config.buff_id, Some(HashId::from_signed(-1124324279)));
    assert_eq!(a.config.rank, Some(1)); // var16
}

#[test]
fn hissatsu_non_resolu_sans_skill_pas_d_invention() {
    let auras = parse_all_aura_cmds(&aura_node_fixture());
    let a = &auras[0];
    // Carte de skills VIDE → None (pas d'invention) : sans table de skills chargée on ne
    // résout rien. (NB : avec le vrai skill_config, skillId1 0x0F8C620D RÉSOUT bien vers
    // whs01780 — cf. le test positif ci-dessous ; ~61/1548 auras résolvent réellement.)
    let empty = build_skill_map(Vec::new());
    assert!(resolve_aura_hissatsu(&a.config, &empty).is_none());
}

#[test]
fn hissatsu_resolu_chaine_skillid1_vers_skill_config() {
    // Résolution POSITIVE avec le VRAI skill whs01780 (= skillId1 0x0F8C620D de l'aura).
    // Valeurs réelles tirées de skill_config_4.00.17.00 (vérifiées) :
    //   0x0F8C620D / whs01780 / nameId 0x6B9C3E18 / power 100-640 / element 3 (Feu) /
    //   category 1 (Tir) / consumeTp 100 / recastTime 90.
    let skill_value = json!({
        "skillID": "0x0F8C620D",
        "skillIDStr": "whs01780",
        "eventID": "0x00000000", "eventIDName": "",
        "failEventID": "0x00000000", "failEventIDName": "",
        "skillNameId": "0x6B9C3E18", "skillDescId": "0x00000000",
        "cmdOptIdx": 0, "skillEffectBitFlag": 0,
        "power_min": 100, "power_max": 640,
        "element": 3, "colorIdx": 1, "category": 1, "growthType": 0,
        "foulRate": 0, "consumeTp": 100, "focusBattleEffectId": "0x00000000",
        "recastTime": 90, "partnerType": 0,
        "partner1": "0x00000000", "partner2": "0x00000000", "partner3": "0x00000000",
        "telopInfoId": "0x00000000", "eldorado": false,
        "seriesIdCrc": "0x00000000", "isDisablePlayableUntilNextPatch": false
    });
    let skill = SkillInfo::from_value(&skill_value).unwrap();
    let map = build_skill_map(alloc_vec(skill));

    let auras = parse_all_aura_cmds(&aura_node_fixture());
    let h = resolve_aura_hissatsu(&auras[0].config, &map)
        .expect("skillId1 0x0F8C620D doit résoudre vers whs01780");

    assert_eq!(h.skill_id, HashId(0x0F8C_620D));
    assert_eq!(h.skill_id_str.as_deref(), Some("whs01780"));
    assert_eq!(h.category, SkillCategory::Shoot); // category 1 = Tir
    assert_eq!(h.element, SkillElement::Fire); // element 3 = Feu
    assert_eq!(h.power, (100, 640));
}

#[test]
fn determine_sub_type_prefixes() {
    assert_eq!(determine_sub_type("wks00020", None), AuraSubType::Keshin);
    assert_eq!(
        determine_sub_type("wks00240", Some("Totem de feu")),
        AuraSubType::Soul
    );
    assert_eq!(
        determine_sub_type("awakening_001", None),
        AuraSubType::Awakening
    );
    assert_eq!(
        determine_sub_type("mode_change_x", None),
        AuraSubType::ModeChange
    );
    assert_eq!(determine_sub_type("wmm00210", None), AuraSubType::Miximax);
    assert_eq!(determine_sub_type("wko00010", None), AuraSubType::Keshin);
    assert_eq!(determine_sub_type("was00010", None), AuraSubType::Keshin);
    assert_eq!(determine_sub_type("xyz00010", None), AuraSubType::Aura);
}

#[test]
fn hissatsu_0f8c620d_resout_whs01780_vrai_fichier() {
    // Validation byte-à-byte contre les vrais dumps. Skip si absents du VPS.
    // Vérité terrain : AURA_CMD_INFO_0 var6 = 260858381 = 0x0F8C620D (aura_skill_config),
    // skill_id_str = "whs01780", power 100-640, element 3 (Feu), category 1 (Tir)
    // (skill_config_4.00.17.00). ~61/1548 auras résolvent réellement.
    let aura_path = "skill/aura_skill_config_1.04.09.00.cfg.bin.json";
    let skill_path = "skill/skill_config_4.00.17.00.cfg.bin.json";
    if !std::path::Path::new(aura_path).exists() || !std::path::Path::new(skill_path).exists() {
        return;
    }

    let aura_root: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(aura_path).expect("lecture aura"))
            .expect("JSON aura");
    let skill_root: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(skill_path).expect("lecture skill"))
            .expect("JSON skill");

    let auras = parse_all_aura_cmds(&aura_root);
    // 387 AURA_CMD_INFO réels (19 vars) ; les 1161 AURA_CMD_INFO_REF (2 vars) sont filtrés
    // par from_node (var_count < 4). Nombre total dans le fichier : 1548 enfants.
    assert_eq!(
        auras.len(),
        387,
        "387 AURA_CMD_INFO réels (non-REF, ≥4 vars) attendus dans aura_skill_config_1.04.09.00"
    );

    let skills = nie_data::skill::parse_skill_config(&skill_root);
    let map = build_skill_map(skills);

    // AURA_CMD_INFO_0 : auraId=0x7978E1FA, skillId1=0x0F8C620D → whs01780.
    let a0 = &auras[0];
    assert_eq!(a0.aura_id.to_hex(), "0x7978E1FA", "AURA_CMD_INFO_0 auraId");
    assert_eq!(
        a0.config.skill_id1.unwrap().to_hex(),
        "0x0F8C620D",
        "AURA_CMD_INFO_0 skillId1"
    );

    let h = resolve_aura_hissatsu(&a0.config, &map)
        .expect("0x0F8C620D doit résoudre vers whs01780 dans skill_config_4.00.17.00");
    assert_eq!(h.skill_id, HashId(0x0F8C_620D));
    assert_eq!(h.skill_id_str.as_deref(), Some("whs01780"));
    assert_eq!(h.element, SkillElement::Fire); // element 3
    assert_eq!(h.category, SkillCategory::Shoot); // category 1
    assert_eq!(h.power, (100, 640));
}

/// Helper de test : Vec à un élément.
fn alloc_vec(s: SkillInfo) -> Vec<SkillInfo> {
    vec![s]
}
