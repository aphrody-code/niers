#![allow(clippy::pedantic)]
//! Tests golden `skill_view` — valeurs réelles tirées de :
//! - `data/common/gamedata/skill_view/skill_view_preset_config_0.00.28.cfg.bin.json` (golden principal)
//! - `data/common/gamedata/skill_view/skill_view_preset_config.cfg.bin.json` (version de base)
//!
//! ## Vérifications champ par champ (JSON réel → valeur confirmée)
//!
//! ### m_SkillViewPresetDataList (17 entrées, conservées positionnellement)
//!
//! Entrée 0 (0.00.28, lignes 8-17) :
//! - `charaId` = "0xB7127F3A", `skillType` = 0, `uniformId` = "0x47A6D3D3",
//!   `shoesId` = "0xD233B8D6", `gloveId` = "0x00000000", `bAway` = false,
//!   `bKeeper` = false, `uniformNumber` = 8
//!   (version de base : `uniformId` = "0x26E5E3F1", `shoesId` = "0x00000000")
//!
//! Entrée 3 (lignes 38-47) :
//! - `charaId` = "0xAC4BB951", `skillType` = 1, `uniformId` = "0xBF5742E0",
//!   `uniformNumber` = 13
//!
//! Entrée 6 (lignes 68-77) :
//! - `charaId` = "0x38528A9A", `skillType` = 0, `uniformId` = "0xC68BFA44",
//!   `shoesId` = "0x0B4EC959", `bKeeper` = true
//!
//! Entrée 8 (lignes 88-97) : emplacement vide `charaId` = "0x00000000", `skillType` = 1
//!
//! Entrée 13 (lignes 138-147) :
//! - `charaId` = "0x80D8F165", `gloveId` = "0xFB74C292", `bKeeper` = true (gardien gantsé)
//!
//! Entrée 16 / dernière (lignes 168-177) :
//! - `charaId` = "0xD65CEBB6", `skillType` = 1
//!
//! ### m_SkillViewPresetInfoList (5 presets, identiques entre les deux versions)
//!
//! - [0] `presetId` = "0x6744A776", `presetName` = "プリセット【base01】", `presetData` = [0, 6], `bDefault` = true
//! - [1] `presetId` = "0x940F8BB9", `presetName` = "プリセット【普通02】", `presetData` = [6, 4], `bDefault` = false
//! - [2] `presetId` = "0x1D0E83DB", `presetName` = "プリセット【チビ01】", `presetData` = [10, 3]
//! - [3] `presetId` = "0x0FBB2C35", `presetName` = "プリセット【チビ02】", `presetData` = [13, 2]
//! - [4] `presetId` = "0x921B7D7F", `presetName` = "プリセット【発動者0人テスト】", `presetData` = [15, 2]

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::skill_view::{SkillViewPresetData, parse_skill_view_preset_config};
use serde_json::json;

// ─── Constantes golden (valeurs lues dans le JSON réel 0.00.28) ──────────────────

const DATA0_CHARA: HashId = HashId(0xB712_7F3A);
const DATA0_UNIFORM_028: HashId = HashId(0x47A6_D3D3);
const DATA0_SHOES_028: HashId = HashId(0xD233_B8D6);
const DATA0_UNIFORM_BASE: HashId = HashId(0x26E5_E3F1);

const DATA3_CHARA: HashId = HashId(0xAC4B_B951);
const DATA3_UNIFORM: HashId = HashId(0xBF57_42E0);

const DATA6_CHARA: HashId = HashId(0x3852_8A9A);
const DATA6_UNIFORM: HashId = HashId(0xC68B_FA44);
const DATA6_SHOES: HashId = HashId(0x0B4E_C959);

const DATA13_CHARA: HashId = HashId(0x80D8_F165);
const DATA13_GLOVE: HashId = HashId(0xFB74_C292);

const DATA16_CHARA: HashId = HashId(0xD65C_EBB6);

const PRESET0_ID: HashId = HashId(0x6744_A776);
const PRESET1_ID: HashId = HashId(0x940F_8BB9);
const PRESET4_ID: HashId = HashId(0x921B_7D7F);

// ─── Fixture inline (valeurs extraites octet-pour-octet du JSON 0.00.28) ─────────

/// Fixture minimale : 3 entrées de data (dont 1 emplacement vide) + 1 preset.
fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_SkillViewPresetDataList",
                "typeName": "SKILL_VIEW_PRESET_DATA",
                "values": [
                    // entrée 0 (JSON 0.00.28 lignes 8-17)
                    {
                        "charaId": "0xB7127F3A",
                        "skillType": 0,
                        "uniformId": "0x47A6D3D3",
                        "shoesId": "0xD233B8D6",
                        "gloveId": "0x00000000",
                        "bAway": false,
                        "bKeeper": false,
                        "uniformNumber": 8
                    },
                    // entrée vide (charaId nul) — DOIT être conservée
                    {
                        "charaId": "0x00000000",
                        "skillType": 1,
                        "uniformId": "0x00000000",
                        "shoesId": "0x00000000",
                        "gloveId": "0x00000000",
                        "bAway": false,
                        "bKeeper": false,
                        "uniformNumber": 0
                    },
                    // entrée gardien gantsé
                    {
                        "charaId": "0x80D8F165",
                        "skillType": 0,
                        "uniformId": "0x947A1123",
                        "shoesId": "0x00000000",
                        "gloveId": "0xFB74C292",
                        "bAway": false,
                        "bKeeper": true,
                        "uniformNumber": 0
                    }
                ]
            },
            {
                "name": "m_SkillViewPresetInfoList",
                "typeName": "SKILL_VIEW_PRESET_INFO",
                "values": [
                    {
                        "presetId": "0x6744A776",
                        "presetName": "プリセット【base01】",
                        "presetData": [0, 2],
                        "bDefault": true
                    }
                ]
            }
        ]
    })
}

// ─── Tests fixture ───────────────────────────────────────────────────────────────

#[test]
fn fixture_compte() {
    let cfg = parse_skill_view_preset_config(&fixture());
    // 3 entrées data (y compris l'emplacement vide), 1 preset
    assert_eq!(
        cfg.preset_data.len(),
        3,
        "3 SkillViewPresetData (vide conservé)"
    );
    assert_eq!(cfg.presets.len(), 1, "1 SkillViewPresetInfo");
}

#[test]
fn fixture_data_0() {
    let cfg = parse_skill_view_preset_config(&fixture());
    let d = &cfg.preset_data[0];
    assert_eq!(d.chara_id, DATA0_CHARA, "charaId[0] = 0xB7127F3A");
    assert_eq!(d.skill_type, 0, "skillType[0] = 0");
    assert_eq!(d.uniform_id, DATA0_UNIFORM_028, "uniformId[0] = 0x47A6D3D3");
    assert_eq!(d.shoes_id, DATA0_SHOES_028, "shoesId[0] = 0xD233B8D6");
    assert_eq!(d.glove_id, HashId::ZERO, "gloveId[0] = 0");
    assert!(!d.b_away, "bAway[0] = false");
    assert!(!d.b_keeper, "bKeeper[0] = false");
    assert_eq!(d.uniform_number, 8, "uniformNumber[0] = 8");
}

#[test]
fn fixture_data_1_emplacement_vide_conserve() {
    let cfg = parse_skill_view_preset_config(&fixture());
    let d = &cfg.preset_data[1];
    // L'emplacement vide (charaId nul) est conservé positionnellement
    assert_eq!(d.chara_id, HashId::ZERO, "charaId[1] = 0 (vide conservé)");
    assert_eq!(d.skill_type, 1, "skillType[1] = 1");
}

#[test]
fn fixture_data_2_gardien_gantse() {
    let cfg = parse_skill_view_preset_config(&fixture());
    let d = &cfg.preset_data[2];
    assert_eq!(d.chara_id, DATA13_CHARA, "charaId[2] = 0x80D8F165");
    assert_eq!(d.glove_id, DATA13_GLOVE, "gloveId[2] = 0xFB74C292");
    assert!(d.b_keeper, "bKeeper[2] = true");
}

#[test]
fn fixture_preset_0() {
    let cfg = parse_skill_view_preset_config(&fixture());
    let p = &cfg.presets[0];
    assert_eq!(p.preset_id, PRESET0_ID, "presetId[0] = 0x6744A776");
    assert_eq!(p.preset_name, "プリセット【base01】", "presetName[0]");
    assert_eq!(p.data_offset, 0, "data_offset[0] = 0");
    assert_eq!(p.data_count, 2, "data_count[0] = 2");
    assert!(p.b_default, "bDefault[0] = true");
}

#[test]
fn fixture_preset_data_of() {
    let cfg = parse_skill_view_preset_config(&fixture());
    let p = &cfg.presets[0]; // [0, 2]
    let data = cfg.preset_data_of(p);
    assert_eq!(data.len(), 2, "preset_data_of → 2 entrées");
    assert_eq!(data[0].chara_id, DATA0_CHARA, "data[0] = 0xB7127F3A");
}

#[test]
fn fixture_default_preset() {
    let cfg = parse_skill_view_preset_config(&fixture());
    let def = cfg.default_preset();
    assert!(def.is_some(), "default_preset présent");
    assert_eq!(def.unwrap().preset_id, PRESET0_ID, "défaut = 0x6744A776");
}

#[test]
fn from_value_emplacement_vide() {
    // L'API from_value ne filtre jamais (entrées positionnelles)
    let v = json!({ "charaId": "0x00000000", "skillType": 0 });
    let d = SkillViewPresetData::from_value(&v);
    assert_eq!(d.chara_id, HashId::ZERO);
}

// ─── Tests sur fichiers réels (skip silencieux si absents) ───────────────────────

const REAL_028: &str = "skill_view/skill_view_preset_config_0.00.28.cfg.bin.json";
const REAL_BASE: &str = "skill_view/skill_view_preset_config.cfg.bin.json";

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

// — version 0.00.28 (golden principal) —

#[test]
fn real_028_compte() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    assert_eq!(
        cfg.preset_data.len(),
        17,
        "m_SkillViewPresetDataList : 17 entrées"
    );
    assert_eq!(
        cfg.presets.len(),
        5,
        "m_SkillViewPresetInfoList : 5 presets"
    );
}

#[test]
fn real_028_data_0() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    let d = &cfg.preset_data[0];
    assert_eq!(d.chara_id, DATA0_CHARA, "charaId[0] = 0xB7127F3A");
    assert_eq!(d.skill_type, 0, "skillType[0] = 0");
    assert_eq!(
        d.uniform_id, DATA0_UNIFORM_028,
        "uniformId[0] = 0x47A6D3D3 (0.00.28)"
    );
    assert_eq!(
        d.shoes_id, DATA0_SHOES_028,
        "shoesId[0] = 0xD233B8D6 (0.00.28)"
    );
    assert_eq!(d.uniform_number, 8, "uniformNumber[0] = 8");
}

#[test]
fn real_028_data_3() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    let d = &cfg.preset_data[3];
    assert_eq!(d.chara_id, DATA3_CHARA, "charaId[3] = 0xAC4BB951");
    assert_eq!(d.skill_type, 1, "skillType[3] = 1");
    assert_eq!(d.uniform_id, DATA3_UNIFORM, "uniformId[3] = 0xBF5742E0");
    assert_eq!(d.uniform_number, 13, "uniformNumber[3] = 13");
}

#[test]
fn real_028_data_6_gardien() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    let d = &cfg.preset_data[6];
    assert_eq!(d.chara_id, DATA6_CHARA, "charaId[6] = 0x38528A9A");
    assert_eq!(d.uniform_id, DATA6_UNIFORM, "uniformId[6] = 0xC68BFA44");
    assert_eq!(d.shoes_id, DATA6_SHOES, "shoesId[6] = 0x0B4EC959");
    assert!(d.b_keeper, "bKeeper[6] = true");
}

#[test]
fn real_028_data_8_emplacement_vide() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    let d = &cfg.preset_data[8];
    // emplacement vide conservé positionnellement
    assert_eq!(d.chara_id, HashId::ZERO, "charaId[8] = 0 (vide)");
    assert_eq!(d.skill_type, 1, "skillType[8] = 1");
}

#[test]
fn real_028_data_13_gardien_gantse() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    let d = &cfg.preset_data[13];
    assert_eq!(d.chara_id, DATA13_CHARA, "charaId[13] = 0x80D8F165");
    assert_eq!(d.glove_id, DATA13_GLOVE, "gloveId[13] = 0xFB74C292");
    assert!(d.b_keeper, "bKeeper[13] = true");
}

#[test]
fn real_028_data_last() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    let d = &cfg.preset_data[16];
    assert_eq!(d.chara_id, DATA16_CHARA, "charaId[16] = 0xD65CEBB6");
    assert_eq!(d.skill_type, 1, "skillType[16] = 1");
}

#[test]
fn real_028_preset_0() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    let p = &cfg.presets[0];
    assert_eq!(p.preset_id, PRESET0_ID, "presetId[0] = 0x6744A776");
    assert_eq!(p.preset_name, "プリセット【base01】", "presetName[0]");
    assert_eq!(p.data_offset, 0, "data_offset[0] = 0");
    assert_eq!(p.data_count, 6, "data_count[0] = 6");
    assert!(p.b_default, "bDefault[0] = true");
}

#[test]
fn real_028_preset_1() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    let p = &cfg.presets[1];
    assert_eq!(p.preset_id, PRESET1_ID, "presetId[1] = 0x940F8BB9");
    assert_eq!(p.preset_name, "プリセット【普通02】", "presetName[1]");
    assert_eq!(p.data_offset, 6, "data_offset[1] = 6");
    assert_eq!(p.data_count, 4, "data_count[1] = 4");
    assert!(!p.b_default, "bDefault[1] = false");
}

#[test]
fn real_028_preset_last() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    let p = &cfg.presets[4];
    assert_eq!(p.preset_id, PRESET4_ID, "presetId[4] = 0x921B7D7F");
    assert_eq!(
        p.preset_name, "プリセット【発動者0人テスト】",
        "presetName[4]"
    );
    assert_eq!(p.data_offset, 15, "data_offset[4] = 15");
    assert_eq!(p.data_count, 2, "data_count[4] = 2");
}

#[test]
fn real_028_preset_data_of_resolu() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    // preset[0] = [0, 6] → 6 entrées, première = 0xB7127F3A
    let data = cfg.preset_data_of(&cfg.presets[0]);
    assert_eq!(data.len(), 6, "preset_data_of(preset[0]) = 6 éléments");
    assert_eq!(data[0].chara_id, DATA0_CHARA, "data[0] = 0xB7127F3A");
    // somme des counts = 17 (couvre toutes les entrées)
    let total: i64 = cfg.presets.iter().map(|p| p.data_count).sum();
    assert_eq!(
        total, 17,
        "somme des data_count = 17 (= toutes les entrées)"
    );
}

#[test]
fn real_028_find_et_default() {
    let Some(root) = load_json(REAL_028) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    assert!(
        cfg.find_preset(PRESET4_ID).is_some(),
        "find_preset(0x921B7D7F)"
    );
    assert!(
        cfg.find_preset(HashId(0xDEAD_BEEF)).is_none(),
        "find_preset inconnu → None"
    );
    assert_eq!(
        cfg.default_preset().unwrap().preset_id,
        PRESET0_ID,
        "default_preset = 0x6744A776"
    );
}

// — version de base : ne diffère que par uniformId/shoesId de l'entrée 0 —

#[test]
fn real_base_compte() {
    let Some(root) = load_json(REAL_BASE) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    assert_eq!(cfg.preset_data.len(), 17, "base : 17 entrées data");
    assert_eq!(cfg.presets.len(), 5, "base : 5 presets");
}

#[test]
fn real_base_data_0_uniform_differe() {
    let Some(root) = load_json(REAL_BASE) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    let d = &cfg.preset_data[0];
    // dans la version de base, l'entrée 0 a un uniforme différent et pas de chaussures
    assert_eq!(
        d.chara_id, DATA0_CHARA,
        "charaId[0] = 0xB7127F3A (inchangé)"
    );
    assert_eq!(
        d.uniform_id, DATA0_UNIFORM_BASE,
        "uniformId[0] = 0x26E5E3F1 (base)"
    );
    assert_eq!(d.shoes_id, HashId::ZERO, "shoesId[0] = 0 (base)");
    assert_eq!(d.uniform_number, 8, "uniformNumber[0] = 8");
}

#[test]
fn real_base_presets_identiques() {
    let Some(root) = load_json(REAL_BASE) else {
        return;
    };
    let cfg = parse_skill_view_preset_config(&root);
    // la liste des presets est identique entre les deux versions
    assert_eq!(cfg.presets[0].preset_id, PRESET0_ID);
    assert_eq!(cfg.presets[0].preset_name, "プリセット【base01】");
    assert_eq!(cfg.presets[4].preset_id, PRESET4_ID);
}
