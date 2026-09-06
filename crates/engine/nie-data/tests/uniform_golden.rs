#![allow(clippy::pedantic)]
//! Tests golden `uniform` — valeurs réelles extraites du VFS Steam :
//! `data/common/gamedata/character/uniform_config_1.03.52.00.cfg.bin`
//! (RDBN, format `lists`, décodé via `nie_formats::cfgbin::read_values`).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/uniform-config.ts`.
//!
//! ## Vérité terrain (octet-pour-octet du dump réel)
//!
//! Comptes des 5 listes (confirmés sur le .json de référence ET la lecture LIVE du VFS
//! via model-serve `/cfg` — identiques) :
//! - `m_UniformModelInfoList`    : 760 lignes (`UNIFORM_MODEL_INFO`, 27 champs)
//! - `m_UniformInfoList`         : 384 lignes (`UNIFORM_INFO`)
//! - `m_UniformExModelInfoList`  : 751 lignes (`UNIFORM_EX_MODEL_INFO`, 14 champs)
//! - `m_UniformExInfoList`       : 378 lignes (`UNIFORM_EX_INFO`)
//! - `m_CharaUniformExInfoList`  : 234 lignes (`CHARA_UNIFORM_EX_INFO`)
//!
//! ### m_UniformModelInfoList[0]
//! - `uniformFielderModelIdCrc` = `0x9D2BBD10`, `uniformKeeperModelIdCrc` = `0x55CB3260`
//! - `gloveModelIdCrc` = `0xBE70ECC5`, `shoesFielderModelIdCrc` = `0x9035CD57`
//! - `uniformFielderNavelBaringModelIdCrc` = `0xA8140C3F`, `typeId` = 0, `shoesModelIdLocked` = false
//!
//! ### m_UniformInfoList[0]
//! - `nameId` = `0x252CE113`, `modelInfo` = `[0, 2]`
//!
//! ### m_UniformExModelInfoList[0]
//! - `fielderExModelId1` = `0x8D9E34B8`, `keeperExModelId1` = `0x8D9E34B8`
//! - `shoesFielderModelIdCrc` = `0xEF70E39A`, `typeId` = 0
//!
//! ### m_CharaUniformExInfoList[0]
//! - `charaId` = `0x99A1C150`, `uniformInfo` = `[0, 10]`

mod common;

use nie_data::hash::HashId;
use nie_data::uniform::{UniformConfig, parse_uniform_config};
use serde_json::json;

// ─── Fixture : valeurs extraites octet-pour-octet du dump réel ──────────────────

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_UniformModelInfoList",
                "typeName": "UNIFORM_MODEL_INFO",
                "values": [
                    // uniform_config_1.03.52.00 : m_UniformModelInfoList[0]
                    {
                        "uniformFielderModelIdCrc": "0x9D2BBD10",
                        "uniformKeeperModelIdCrc": "0x55CB3260",
                        "uniformDirectorModelIdCrc": "0x00000000",
                        "uniformManagerModelIdCrc": "0x00000000",
                        "uniformFielderShoulderBaringModelIdCrc": "0xF1E2CE0F",
                        "uniformKeeperShoulderBaringModelIdCrc": "0x00000000",
                        "uniformFielderShortSleeveRollUpArmModelIdCrc": "0x18B6DEDC",
                        "uniformKeeperShortSleeveRollUpArmModelIdCrc": "0x00000000",
                        "uniformFielderShoulderBaringPatternedModelIdCrc": "0xD740CC5B",
                        "uniformKeeperShoulderBaringPatternedModelIdCrc": "0x00000000",
                        "uniformFielderHalfSleeveModelIdCrc": "0x00000000",
                        "uniformKeeperHalfSleeveModelIdCrc": "0x00000000",
                        "uniformFielderLongSleeveRollUpArmModelIdCrc": "0x00000000",
                        "uniformKeeperLongSleeveRollUpArmModelIdCrc": "0xE5EB204D",
                        "uniformFielderLongSleeveRollUpSleeveModelIdCrc": "0x00000000",
                        "uniformKeeperLongSleeveRollUpSleeveModelIdCrc": "0xE286E454",
                        "uniformFielderNavelBaringModelIdCrc": "0xA8140C3F",
                        "uniformKeeperNavelBaringModelIdCrc": "0x4E3DC77B",
                        "shoesFielderModelIdCrc": "0x9035CD57",
                        "shoesKeeperModelIdCrc": "0x9035CD57",
                        "shoesDirectorModelIdCrc": "0x00000000",
                        "shoesManagerModelIdCrc": "0x00000000",
                        "gloveModelIdCrc": "0xBE70ECC5",
                        "typeId": 0,
                        "shoesModelAttr": 0,
                        "uniformNgModelAttr": 0,
                        "shoesModelIdLocked": false
                    },
                    // uniform_config_1.03.52.00 : m_UniformModelInfoList[1]
                    {
                        "uniformFielderModelIdCrc": "0xB606EED3",
                        "uniformKeeperModelIdCrc": "0x7EE661A3",
                        "uniformDirectorModelIdCrc": "0x00000000",
                        "uniformManagerModelIdCrc": "0x00000000",
                        "uniformFielderShoulderBaringModelIdCrc": "0x7776BCA1",
                        "uniformKeeperShoulderBaringModelIdCrc": "0x00000000",
                        "uniformFielderShortSleeveRollUpArmModelIdCrc": "0x9E22AC72",
                        "uniformKeeperShortSleeveRollUpArmModelIdCrc": "0x00000000",
                        "uniformFielderShoulderBaringPatternedModelIdCrc": "0x51D4BEF5",
                        "uniformKeeperShoulderBaringPatternedModelIdCrc": "0x00000000",
                        "uniformFielderHalfSleeveModelIdCrc": "0x00000000",
                        "uniformKeeperHalfSleeveModelIdCrc": "0x00000000",
                        "uniformFielderLongSleeveRollUpArmModelIdCrc": "0x00000000",
                        "uniformKeeperLongSleeveRollUpArmModelIdCrc": "0x637F52E3",
                        "uniformFielderLongSleeveRollUpSleeveModelIdCrc": "0x00000000",
                        "uniformKeeperLongSleeveRollUpSleeveModelIdCrc": "0x641296FA",
                        "uniformFielderNavelBaringModelIdCrc": "0x99FC16A2",
                        "uniformKeeperNavelBaringModelIdCrc": "0x7FD5DDE6",
                        "shoesFielderModelIdCrc": "0x9035CD57",
                        "shoesKeeperModelIdCrc": "0x9035CD57",
                        "shoesDirectorModelIdCrc": "0x00000000",
                        "shoesManagerModelIdCrc": "0x00000000",
                        "gloveModelIdCrc": "0x955DBF06",
                        "typeId": 1,
                        "shoesModelAttr": 0,
                        "uniformNgModelAttr": 0,
                        "shoesModelIdLocked": false
                    }
                ]
            },
            {
                "name": "m_UniformInfoList",
                "typeName": "UNIFORM_INFO",
                "values": [
                    // m_UniformInfoList[0]
                    { "nameId": "0x252CE113", "modelInfo": [0, 2] },
                    // m_UniformInfoList[1]
                    { "nameId": "0xCF2DC723", "modelInfo": [2, 2] }
                ]
            },
            {
                "name": "m_UniformExModelInfoList",
                "typeName": "UNIFORM_EX_MODEL_INFO",
                "values": [
                    // m_UniformExModelInfoList[0]
                    {
                        "charaModelIdCrc": "0x00000000",
                        "faceIconCharaIdCrc": "0x00000000",
                        "fielderExModelId1": "0x8D9E34B8",
                        "fielderExModelId2": "0x00000000",
                        "fielderExModelId3": "0x00000000",
                        "keeperExModelId1": "0x8D9E34B8",
                        "keeperExModelId2": "0x00000000",
                        "keeperExModelId3": "0x00000000",
                        "gloveModelIdCrc": "0x00000000",
                        "shoesFielderModelIdCrc": "0xEF70E39A",
                        "shoesKeeperModelIdCrc": "0xEF70E39A",
                        "uniformFielderModelIdCrc": "0x00000000",
                        "uniformKeeperModelIdCrc": "0x00000000",
                        "typeId": 0
                    },
                    // m_UniformExModelInfoList[1]
                    {
                        "charaModelIdCrc": "0x00000000",
                        "faceIconCharaIdCrc": "0x00000000",
                        "fielderExModelId1": "0x8D9E34B8",
                        "fielderExModelId2": "0x00000000",
                        "fielderExModelId3": "0x00000000",
                        "keeperExModelId1": "0x8D9E34B8",
                        "keeperExModelId2": "0x00000000",
                        "keeperExModelId3": "0x00000000",
                        "gloveModelIdCrc": "0x00000000",
                        "shoesFielderModelIdCrc": "0xEF70E39A",
                        "shoesKeeperModelIdCrc": "0xEF70E39A",
                        "uniformFielderModelIdCrc": "0x00000000",
                        "uniformKeeperModelIdCrc": "0x00000000",
                        "typeId": 1
                    }
                ]
            },
            {
                "name": "m_UniformExInfoList",
                "typeName": "UNIFORM_EX_INFO",
                "values": [
                    // m_UniformExInfoList[0]
                    { "nameId": "0x252CE113", "modelInfo": [0, 2], "tmpFlagIdCrc": "0x00000000" },
                    // m_UniformExInfoList[1]
                    { "nameId": "0xCF2DC723", "modelInfo": [2, 2], "tmpFlagIdCrc": "0x00000000" },
                    // m_UniformExInfoList[2]
                    { "nameId": "0x59441354", "modelInfo": [4, 2], "tmpFlagIdCrc": "0x00000000" }
                ]
            },
            {
                "name": "m_CharaUniformExInfoList",
                "typeName": "CHARA_UNIFORM_EX_INFO",
                "values": [
                    // m_CharaUniformExInfoList[0]
                    { "charaId": "0x99A1C150", "uniformInfo": [0, 10] },
                    // m_CharaUniformExInfoList[1]
                    { "charaId": "0x27605DBC", "uniformInfo": [10, 10] },
                    // m_CharaUniformExInfoList[2]
                    { "charaId": "0x81789A26", "uniformInfo": [20, 2] }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ───────────────────────────────────────────────────────

#[test]
fn fixture_comptes_des_listes() {
    let cfg = parse_uniform_config(&fixture());
    assert_eq!(cfg.models.len(), 2);
    assert_eq!(cfg.uniforms.len(), 2);
    assert_eq!(cfg.ex_models.len(), 2);
    assert_eq!(cfg.ex_uniforms.len(), 3);
    assert_eq!(cfg.chara_ex_uniforms.len(), 3);
}

#[test]
fn fixture_model0_tous_les_champs() {
    let cfg = parse_uniform_config(&fixture());
    let m = &cfg.models[0];

    // uniform_config_1.03.52.00 : m_UniformModelInfoList[0]
    assert_eq!(m.uniform_fielder_model_id_crc, HashId(0x9D2B_BD10));
    assert_eq!(m.uniform_keeper_model_id_crc, HashId(0x55CB_3260));
    assert_eq!(m.uniform_director_model_id_crc, HashId::ZERO);
    assert_eq!(m.uniform_manager_model_id_crc, HashId::ZERO);
    assert_eq!(
        m.uniform_fielder_shoulder_baring_model_id_crc,
        HashId(0xF1E2_CE0F)
    );
    assert_eq!(m.uniform_keeper_shoulder_baring_model_id_crc, HashId::ZERO);
    assert_eq!(
        m.uniform_fielder_short_sleeve_roll_up_arm_model_id_crc,
        HashId(0x18B6_DEDC)
    );
    assert_eq!(
        m.uniform_fielder_shoulder_baring_patterned_model_id_crc,
        HashId(0xD740_CC5B)
    );
    assert_eq!(
        m.uniform_keeper_long_sleeve_roll_up_arm_model_id_crc,
        HashId(0xE5EB_204D)
    );
    assert_eq!(
        m.uniform_keeper_long_sleeve_roll_up_sleeve_model_id_crc,
        HashId(0xE286_E454)
    );
    assert_eq!(
        m.uniform_fielder_navel_baring_model_id_crc,
        HashId(0xA814_0C3F)
    );
    assert_eq!(
        m.uniform_keeper_navel_baring_model_id_crc,
        HashId(0x4E3D_C77B)
    );
    assert_eq!(m.shoes_fielder_model_id_crc, HashId(0x9035_CD57));
    assert_eq!(m.shoes_keeper_model_id_crc, HashId(0x9035_CD57));
    assert_eq!(m.glove_model_id_crc, HashId(0xBE70_ECC5));
    assert_eq!(m.type_id, 0);
    assert_eq!(m.shoes_model_attr, 0);
    assert_eq!(m.uniform_ng_model_attr, 0);
    assert!(!m.shoes_model_id_locked);
}

#[test]
fn fixture_model1_distinct_du_model0() {
    let cfg = parse_uniform_config(&fixture());
    let m = &cfg.models[1];

    // m_UniformModelInfoList[1] : même slot de chaussures, mais maillots/gants distincts.
    assert_eq!(m.uniform_fielder_model_id_crc, HashId(0xB606_EED3));
    assert_eq!(m.uniform_keeper_model_id_crc, HashId(0x7EE6_61A3));
    assert_eq!(m.glove_model_id_crc, HashId(0x955D_BF06));
    assert_eq!(
        m.uniform_keeper_long_sleeve_roll_up_arm_model_id_crc,
        HashId(0x637F_52E3)
    );
    assert_eq!(m.type_id, 1);
    // Chaussures partagées avec l'entrée 0 (vérité terrain).
    assert_eq!(m.shoes_fielder_model_id_crc, HashId(0x9035_CD57));
}

#[test]
fn fixture_uniform_info_tranches() {
    let cfg = parse_uniform_config(&fixture());

    // m_UniformInfoList[0] : nameId 0x252CE113, tranche [0, 2].
    assert_eq!(cfg.uniforms[0].name_id, HashId(0x252C_E113));
    assert_eq!(cfg.uniforms[0].model_start, 0);
    assert_eq!(cfg.uniforms[0].model_count, 2);
    // m_UniformInfoList[1] : nameId 0xCF2DC723, tranche [2, 2].
    assert_eq!(cfg.uniforms[1].name_id, HashId(0xCF2D_C723));
    assert_eq!(cfg.uniforms[1].model_start, 2);
    assert_eq!(cfg.uniforms[1].model_count, 2);
}

#[test]
fn fixture_ex_model0_champs() {
    let cfg = parse_uniform_config(&fixture());
    let e = &cfg.ex_models[0];

    // m_UniformExModelInfoList[0]
    assert_eq!(e.fielder_ex_model_id1, HashId(0x8D9E_34B8));
    assert_eq!(e.keeper_ex_model_id1, HashId(0x8D9E_34B8));
    assert_eq!(e.fielder_ex_model_id2, HashId::ZERO);
    assert_eq!(e.shoes_fielder_model_id_crc, HashId(0xEF70_E39A));
    assert_eq!(e.shoes_keeper_model_id_crc, HashId(0xEF70_E39A));
    assert_eq!(e.chara_model_id_crc, HashId::ZERO);
    assert_eq!(e.type_id, 0);
    // L'entrée 1 ne diffère que par typeId.
    assert_eq!(cfg.ex_models[1].type_id, 1);
}

#[test]
fn fixture_ex_info_avec_tmp_flag() {
    let cfg = parse_uniform_config(&fixture());

    // m_UniformExInfoList[0] : nameId partagé avec m_UniformInfoList[0], tmpFlagIdCrc nul.
    assert_eq!(cfg.ex_uniforms[0].name_id, HashId(0x252C_E113));
    assert_eq!(cfg.ex_uniforms[0].model_start, 0);
    assert_eq!(cfg.ex_uniforms[0].model_count, 2);
    assert_eq!(cfg.ex_uniforms[0].tmp_flag_id_crc, HashId::ZERO);
    // m_UniformExInfoList[2]
    assert_eq!(cfg.ex_uniforms[2].name_id, HashId(0x5944_1354));
    assert_eq!(cfg.ex_uniforms[2].model_start, 4);
}

#[test]
fn fixture_chara_ex_uniformes() {
    let cfg = parse_uniform_config(&fixture());

    // m_CharaUniformExInfoList[0] : charaId 0x99A1C150, tranche [0, 10].
    assert_eq!(cfg.chara_ex_uniforms[0].chara_id, HashId(0x99A1_C150));
    assert_eq!(cfg.chara_ex_uniforms[0].uniform_start, 0);
    assert_eq!(cfg.chara_ex_uniforms[0].uniform_count, 10);
    // m_CharaUniformExInfoList[1] : tranche [10, 10] (consécutive).
    assert_eq!(cfg.chara_ex_uniforms[1].chara_id, HashId(0x2760_5DBC));
    assert_eq!(cfg.chara_ex_uniforms[1].uniform_start, 10);
    // m_CharaUniformExInfoList[2] : tranche [20, 2].
    assert_eq!(cfg.chara_ex_uniforms[2].chara_id, HashId(0x8178_9A26));
    assert_eq!(cfg.chara_ex_uniforms[2].uniform_count, 2);
}

#[test]
fn fixture_resolve_rows_joint_les_modeles() {
    // Port 1:1 d'inagle resolveUniformRows : la tranche [0,2] de l'uniforme 0 doit
    // joindre exactement les modèles 0 et 1, et typeId = typeId du 1er modèle (0).
    let cfg = parse_uniform_config(&fixture());
    let rows = cfg.resolve_rows();
    assert_eq!(rows.len(), 2);

    let r0 = &rows[0];
    assert_eq!(r0.name_id, HashId(0x252C_E113));
    assert_eq!(r0.model_start, 0);
    assert_eq!(r0.model_count, 2);
    assert_eq!(r0.type_id, Some(0));
    assert_eq!(r0.models.len(), 2);
    assert_eq!(
        r0.models[0].uniform_fielder_model_id_crc,
        HashId(0x9D2B_BD10)
    );
    assert_eq!(
        r0.models[1].uniform_fielder_model_id_crc,
        HashId(0xB606_EED3)
    );

    // L'uniforme 1 pointe [2, 2] : hors de la fixture (2 modèles seulement) → tranche vide,
    // bornée sans panique (comme inagle slice()).
    let r1 = &rows[1];
    assert_eq!(r1.model_start, 2);
    assert!(r1.models.is_empty());
    assert_eq!(r1.type_id, None);
}

#[test]
fn fixture_accesseurs_recherche() {
    let cfg = parse_uniform_config(&fixture());

    assert!(cfg.find_uniform_by_name_id(HashId(0x252C_E113)).is_some());
    assert!(cfg.find_uniform_by_name_id(HashId(0xDEAD_BEEF)).is_none());

    let chara = cfg.find_chara_ex(HashId(0x9919_0000));
    assert!(chara.is_none());
    assert!(cfg.find_chara_ex(HashId(0x99A1_C150)).is_some());

    // models_with_type_id : un modèle de chaque typeId 0 et 1.
    assert_eq!(cfg.models_with_type_id(0).len(), 1);
    assert_eq!(cfg.models_with_type_id(1).len(), 1);
    assert_eq!(cfg.models_with_type_id(99).len(), 0);
}

#[test]
fn fixture_liste_manquante_renvoie_vide() {
    // Un JSON sans aucune des listes uniform doit donner une config vide sans paniquer.
    let cfg = parse_uniform_config(&json!({ "version": 100, "lists": [] }));
    assert_eq!(cfg.models.len(), 0);
    assert_eq!(cfg.uniforms.len(), 0);
    assert!(cfg.resolve_rows().is_empty());
}

// ─── Test sur le vrai fichier (skip si absent — dump gitignore) ──────────────────

const REAL_PATH: &str = "character/uniform_config_1.03.52.00.cfg.bin.json";

fn load_real() -> Option<UniformConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_uniform_config(&root))
}

#[test]
fn real_file_comptes_des_listes() {
    let Some(cfg) = load_real() else { return };
    // Comptes réels du dump uniform_config_1.03.52.00 (.json de référence == lecture
    // LIVE du VFS Steam via model-serve /cfg, vérifié identique le 2026-06-19).
    assert_eq!(cfg.models.len(), 760, "m_UniformModelInfoList");
    assert_eq!(cfg.uniforms.len(), 384, "m_UniformInfoList");
    assert_eq!(cfg.ex_models.len(), 751, "m_UniformExModelInfoList");
    assert_eq!(cfg.ex_uniforms.len(), 378, "m_UniformExInfoList");
    assert_eq!(cfg.chara_ex_uniforms.len(), 234, "m_CharaUniformExInfoList");
}

#[test]
fn real_file_model0_valeurs() {
    let Some(cfg) = load_real() else { return };
    let m = &cfg.models[0];
    assert_eq!(m.uniform_fielder_model_id_crc, HashId(0x9D2B_BD10));
    assert_eq!(m.uniform_keeper_model_id_crc, HashId(0x55CB_3260));
    assert_eq!(m.glove_model_id_crc, HashId(0xBE70_ECC5));
    assert_eq!(m.type_id, 0);
}
