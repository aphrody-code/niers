#![allow(clippy::pedantic)]
//! Tests golden `chara_edit` (éditeur d'avatar) — valeurs réelles tirées de :
//! `data/common/gamedata/character/chara_edit_parts_type_config_1.03.75.00.cfg.bin.json`.
//!
//! Format `lists` (champs nommés). 4 listes : 42 parts de visage, 6 types de visage,
//! 24 parts de corps, 1 type de parts de corps.

mod common;

extern crate std;

use nie_data::chara_edit::{BODY_TYPES, parse_chara_edit_parts_type_config};
use nie_data::hash::HashId;
use serde_json::json;

const REAL: &str = "character/chara_edit_parts_type_config_1.03.75.00.cfg.bin.json";

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

fn fixture() -> serde_json::Value {
    json!({
        "version": "1.03.75.00",
        "lists": [
            { "name": "m_CharaEditFaceTypeDataList", "typeName": "x", "values": [
                { "noseType": "nose_type_01", "noseTypeCrc": "0xD64E1016",
                  "facePatternID_male": "0x84A3AD8D", "facePatternID_female": "0x84A3AD8D",
                  "facePatternID_small": "0xC686AAF0", "facePatternID_smallfat": "0xC686AAF0",
                  "facePatternID_tall": "0x00E9A377", "facePatternID_tallmuscle": "0x00E9A377",
                  "facePatternID_muscle": "0x8E66A494", "facePatternID_big": "0x8E66A494",
                  "resource_male": "face51_nose01", "resource_female": "face51_nose01",
                  "resource_small": "face53_nose01", "resource_smallfat": "face53_nose01",
                  "resource_tall": "face55_nose01", "resource_tallmuscle": "face55_nose01",
                  "resource_muscle": "face56_nose01", "resource_big": "face56_nose01" }
            ]},
            { "name": "m_CharaEditFaceTypeInfoList", "typeName": "x", "values": [
                { "faceType": "face_mdl_type_01", "faceTypeCrc": "0x8D64CB9B", "faceTypeData": [0, 7] }
            ]},
            { "name": "m_CharaEditPartsBodyTypeDataList", "typeName": "x", "values": [
                { "presetID": "0x6A03969D", "resource_male": "accessory001", "resource_female": "accessory001",
                  "resource_small": "accessory001", "resource_smallfat": "accessory001",
                  "resource_tall": "accessory001", "resource_tallmuscle": "accessory001",
                  "resource_muscle": "accessory001_07", "resource_big": "accessory001_07" }
            ]},
            { "name": "m_CharaEditPartsBodyTypeInfoList", "typeName": "x", "values": [
                { "partsType": 14, "partsTypeData": [0, 24] }
            ]}
        ]
    })
}

#[test]
fn fixture_parts_avatar() {
    let c = parse_chara_edit_parts_type_config(&fixture());
    assert_eq!(BODY_TYPES.len(), 8);
    // Visage.
    assert_eq!(c.face_data.len(), 1);
    let f = &c.face_data[0];
    assert_eq!(f.nose_type, "nose_type_01");
    assert_eq!(f.nose_type_crc, HashId::parse("0xD64E1016").unwrap());
    assert_eq!(f.resource[0], "face51_nose01"); // male
    assert_eq!(f.resource[7], "face56_nose01"); // big
    assert_eq!(f.face_pattern_id[0], HashId::parse("0x84A3AD8D").unwrap());
    // Type de visage + plage.
    assert_eq!(c.face_info.len(), 1);
    assert_eq!(c.face_info[0].face_type, "face_mdl_type_01");
    assert_eq!(
        (c.face_info[0].data_offset, c.face_info[0].data_count),
        (0, 7)
    );
    // Corps.
    assert_eq!(c.body_data.len(), 1);
    assert_eq!(c.body_data[0].resource[7], "accessory001_07"); // big diffère du male
    assert_eq!(c.body_info.len(), 1);
    assert_eq!(c.body_info[0].parts_type, 14);
    assert_eq!(
        (c.body_info[0].data_offset, c.body_info[0].data_count),
        (0, 24)
    );
}

#[test]
fn golden_dump_reel() {
    let Some(root) = load_json(REAL) else {
        eprintln!("dump chara_edit absent — test data-gated ignoré");
        return;
    };
    let c = parse_chara_edit_parts_type_config(&root);
    assert_eq!(c.face_data.len(), 42, "42 parts de visage");
    assert_eq!(c.face_info.len(), 6, "6 types de visage");
    assert_eq!(c.body_data.len(), 24, "24 parts de corps");
    assert_eq!(c.body_info.len(), 1, "1 type de parts de corps");
    // Entrée 0 = nose_type_01.
    assert_eq!(c.face_data[0].nose_type, "nose_type_01");
    assert_eq!(c.face_data[0].resource[0], "face51_nose01");
    // Les plages des Info indexent la data list (offset+count ≤ taille).
    for fi in &c.face_info {
        assert!(
            (fi.data_offset + fi.data_count) as usize <= c.face_data.len(),
            "plage face hors data"
        );
    }
    assert!((c.body_info[0].data_offset + c.body_info[0].data_count) as usize <= c.body_data.len());
}

// ─────────────────────────────────────────────────────────────────────────────
//  `chara_edit_<ver>.cfg.bin.json` — le catalogue complet (16 listes)
// ─────────────────────────────────────────────────────────────────────────────

use nie_data::chara_edit::parse_chara_edit;

const REAL_FULL: &str = "character/chara_edit_1.03.75.00.cfg.bin.json";

fn fixture_full() -> serde_json::Value {
    json!({
        "lists": [
            { "name": "m_CharaEditRecipeInfoList", "typeName": "CHARA_EDIT_RECIPE_INFO", "values": [
                { "bitNum": 1, "category": 0, "categoryParam": 0, "categoryParamSub": 0,
                  "id": "0x5EA7D1E4", "num": 2, "type": 0 },
                { "bitNum": 4, "category": 1, "categoryParam": 2, "categoryParamSub": 0,
                  "id": "0x29A0E172", "num": 13, "type": 1 }
            ]},
            { "name": "m_CharaEditCodeInfoList", "typeName": "CHARA_EDIT_CODE_INFO", "values": [
                { "codeChar": "3", "id": "0x8FF6794B", "num": 0 }
            ]},
            { "name": "m_CharaEditVoiceInfoList", "typeName": "CHARA_EDIT_VOICE_INFO", "values": [
                { "charaSeName": "scoutMAA01", "gender": 1, "id": "0xBF96B7CD", "itemNo": 0,
                  "personality": 1, "type": 0 }
            ]},
            { "name": "m_CharaEditFashionInfoList", "typeName": "CHARA_EDIT_FASHION_INFO", "values": [
                { "fashionNameCrc": "0xE24565A8", "id": 0 }
            ]},
            { "name": "m_CharaEditPersonalityInfoList", "typeName": "CHARA_EDIT_PERSONALITY_INFO", "values": [
                { "id": "0xC8429370", "performanceType": 0, "personalityType": 0, "viewTextId": "0xFA788253" }
            ]},
            { "name": "m_CharaEditPresetFileInfoList", "typeName": "CHARA_EDIT_PRESET_FILE_INFO", "values": [
                { "charaId": "0x00000000", "id": "0x74BCAF5A", "idString": "mdl_edit_avatar01",
                  "viewNo": 0, "viewTextId": "0x00000000" }
            ]},
            { "name": "m_CharaEditPartsDataList", "typeName": "CHARA_EDIT_PARTS_DATA", "values": [
                { "gender": 0, "id": "0xEDD840B4", "itemNo": 1, "resourceName1": "0x06DE46FB",
                  "resourceName2": "0x00000000", "resourceNameStr1": "preset_01_normal",
                  "resourceNameStr2": "0xFFFFFFFF", "textureName": "0xF6E0589B", "viewNo": 1 },
                { "gender": 0, "id": "0x538EAA58", "itemNo": 28, "resourceName1": "0x8DB02FB6",
                  "resourceName2": "0x63E994E9", "resourceNameStr1": "face_01",
                  "resourceNameStr2": "presetColor_Hair_02_05", "textureName": "0x00DCA28E", "viewNo": 28 }
            ]},
            { "name": "m_CharaEditPartsInfoList", "typeName": "CHARA_EDIT_PARTS_INFO", "values": [
                { "faceSettingType": 1, "partsData": [0, 1] },
                { "faceSettingType": 13, "partsData": [1, 1] }
            ]},
            { "name": "m_CharaEditPartsParamDataList", "typeName": "CHARA_EDIT_PARTS_PARAM_DATA", "values": [
                { "isApplyBig": false, "isApplyFemale": true, "isApplyMale": true, "isApplyMuscle": false,
                  "isApplySmall": false, "isApplySmallfat": false, "isApplyTall": false,
                  "isApplyTallmuscle": false, "paramDefault": 0.0, "paramMax": 0.02, "paramMin": -0.02,
                  "paramType": 0, "partsID": "0xE5EC7051", "resourceName": "0x30C8283E" }
            ]},
            { "name": "m_CharaEditPartsParamInfoList", "typeName": "CHARA_EDIT_PARTS_PARAM_INFO", "values": [
                { "partsParamData": [0, 1], "partsType": 2 }
            ]},
            { "name": "m_CharaEditPresetDataList", "typeName": "CHARA_EDIT_PRESET_DATA", "values": [
                { "colorValue": -1, "partsId": "0x00000000", "recipeNo": 1, "recipeType": 4 }
            ]},
            { "name": "m_CharaEditPresetInfoList", "typeName": "CHARA_EDIT_PRESET_INFO", "values": [
                { "isApplyBig": true, "isApplyFemale": true, "isApplyMale": true, "isApplyMuscle": true,
                  "isApplySmall": true, "isApplySmallfat": true, "isApplyTall": true,
                  "isApplyTallmuscle": true, "presetData": [0, 1], "presetID": "0xE26B7178" }
            ]},
            { "name": "m_CharaEditColorDataList", "typeName": "CHARA_EDIT_COLOR_DATA", "values": [
                { "colorPresetID": "0x7C81F416" }
            ]},
            { "name": "m_CharaEditColorInfoList", "typeName": "CHARA_EDIT_COLOR_INFO", "values": [
                { "colorPresetData": [0, 1], "faceSettingType": 22 }
            ]},
            { "name": "m_CharaEditFashionBodyDataList", "typeName": "CHARA_EDIT_FASHION_BODY_DATA", "values": [
                { "bodyType": 0 }
            ]},
            { "name": "m_CharaEditFashionBodyInfoList", "typeName": "CHARA_EDIT_FASHION_BODY_INFO", "values": [
                { "enableBodyType": [0, 1], "id": "0xE24565A8" }
            ]}
        ]
    })
}

#[test]
fn fixture_catalogue_complet() {
    let c = parse_chara_edit(&fixture_full());
    // Les 16 listes sont lues.
    assert_eq!(c.recipes.len(), 2);
    assert_eq!(c.codes.len(), 1);
    assert_eq!(c.voices.len(), 1);
    assert_eq!(c.fashions.len(), 1);
    assert_eq!(c.personalities.len(), 1);
    assert_eq!(c.preset_files.len(), 1);
    assert_eq!(c.parts.len(), 2);
    assert_eq!(c.parts_info.len(), 2);
    assert_eq!(c.params.len(), 1);
    assert_eq!(c.params_info.len(), 1);
    assert_eq!(c.preset_data.len(), 1);
    assert_eq!(c.preset_info.len(), 1);
    assert_eq!(c.colors.len(), 1);
    assert_eq!(c.colors_info.len(), 1);
    assert_eq!(c.fashion_body.len(), 1);
    assert_eq!(c.fashion_body_info.len(), 1);

    // Champs typés.
    assert_eq!(c.recipes[1].bit_num, 4);
    assert_eq!(c.recipes[1].num, 13);
    assert_eq!(c.recipes[1].recipe_type, 1);
    assert_eq!(c.code_bit_width(), 5, "1 + 4 bits");
    assert_eq!(c.codes[0].code_char, "3");
    assert_eq!(c.voices[0].chara_se_name, "scoutMAA01");
    assert_eq!(c.preset_files[0].id_string, "mdl_edit_avatar01");
    assert_eq!(c.parts[1].resource_name_str2, "presetColor_Hair_02_05");
    assert_eq!(c.params[0].param_min, -0.02);
    assert_eq!(
        c.params[0].apply,
        [true, true, false, false, false, false, false, false]
    );
    assert_eq!(c.preset_info[0].apply, [true; 8]);

    // Résolution par plage.
    assert_eq!(c.parts_of(13).len(), 1);
    assert_eq!(c.parts_of(13)[0].resource_name_str1, "face_01");
    assert!(
        c.parts_of(99).is_empty(),
        "catégorie inconnue → vide, pas de panique"
    );
    assert_eq!(c.params_of(2).len(), 1);
    assert_eq!(c.colors_of(22).len(), 1);
    assert_eq!(c.recipe_of(HashId::parse("0xE26B7178").unwrap()).len(), 1);
    assert_eq!(
        c.body_types_of_fashion(HashId::parse("0xE24565A8").unwrap()),
        &[0]
    );
    assert!(c.part(HashId::parse("0xEDD840B4").unwrap()).is_some());
}

#[test]
fn golden_catalogue_reel() {
    let Some(root) = load_json(REAL_FULL) else {
        eprintln!("dump chara_edit (catalogue) absent — test data-gated ignoré");
        return;
    };
    let c = parse_chara_edit(&root);

    // Comptes exacts du dump 1.03.75.00.
    assert_eq!(c.recipes.len(), 86, "86 catégories de recette");
    assert_eq!(c.codes.len(), 64, "alphabet de 64 caractères");
    assert_eq!(c.voices.len(), 96, "96 voix");
    assert_eq!(c.fashions.len(), 5, "5 tenues");
    assert_eq!(c.personalities.len(), 24, "24 personnalités");
    assert_eq!(c.preset_files.len(), 31, "31 avatars-fichiers");
    assert_eq!(c.parts.len(), 502, "502 parts");
    assert_eq!(c.parts_info.len(), 20, "20 catégories de parts");
    assert_eq!(c.params.len(), 218, "218 curseurs");
    assert_eq!(c.params_info.len(), 8, "8 groupes de curseurs");
    assert_eq!(c.preset_data.len(), 2704, "2 704 lignes de recette");
    assert_eq!(c.preset_info.len(), 38, "38 visages prédéfinis");
    assert_eq!(c.colors.len(), 470, "470 couleurs");
    assert_eq!(c.colors_info.len(), 8, "8 palettes");
    assert_eq!(c.fashion_body.len(), 50);
    assert_eq!(c.fashion_body_info.len(), 5);

    // Le code de partage : 410 bits, alphabet base-64 numéroté 0..63 sans trou.
    assert_eq!(c.code_bit_width(), 410, "somme des bitNum");
    let mut nums: Vec<i64> = c.codes.iter().map(|x| x.num).collect();
    nums.sort_unstable();
    assert_eq!(nums, (0..64).collect::<Vec<_>>(), "alphabet contigu 0..63");
    // Un `recipeType` par catégorie, 0..85 sans trou : c'est l'ordre d'encodage du code.
    let mut types: Vec<i64> = c.recipes.iter().map(|r| r.recipe_type).collect();
    types.sort_unstable();
    assert_eq!(
        types,
        (0..86).collect::<Vec<_>>(),
        "types de recette contigus"
    );

    // Les cinq familles `Info`/`Data` partitionnent exactement leur data list — aucun trou,
    // aucun recouvrement : c'est ce qui autorise la résolution par simple découpage.
    let partition = |ranges: Vec<(i64, i64)>, total: usize, quoi: &str| {
        let mut rs = ranges;
        rs.sort_unstable();
        let mut attendu = 0i64;
        for (o, n) in rs {
            assert_eq!(o, attendu, "trou ou recouvrement dans {quoi}");
            attendu += n;
        }
        assert_eq!(
            attendu as usize, total,
            "{quoi} : la partition ne couvre pas la data list"
        );
    };
    partition(
        c.parts_info
            .iter()
            .map(|i| (i.data_offset, i.data_count))
            .collect(),
        c.parts.len(),
        "parts",
    );
    partition(
        c.params_info
            .iter()
            .map(|i| (i.data_offset, i.data_count))
            .collect(),
        c.params.len(),
        "params",
    );
    partition(
        c.preset_info
            .iter()
            .map(|i| (i.data_offset, i.data_count))
            .collect(),
        c.preset_data.len(),
        "recettes",
    );
    partition(
        c.colors_info
            .iter()
            .map(|i| (i.data_offset, i.data_count))
            .collect(),
        c.colors.len(),
        "couleurs",
    );
    partition(
        c.fashion_body_info
            .iter()
            .map(|i| (i.data_offset, i.data_count))
            .collect(),
        c.fashion_body.len(),
        "morphologies de tenue",
    );

    // Valeurs témoins.
    assert_eq!(c.parts[0].resource_name_str1, "preset_01_normal");
    assert_eq!(c.parts[0].id, HashId::parse("0xEDD840B4").unwrap());
    assert_eq!(c.voices[0].chara_se_name, "scoutMAA01");
    assert_eq!(c.preset_files[0].id_string, "mdl_edit_avatar01");

    // Le nom de ressource et son hash concordent : `resourceName1` == crc32(`resourceNameStr1`).
    // C'est la charnière qui permet de retrouver les modèles 3D du VFS depuis le catalogue.
    let mut verifies = 0usize;
    for p in &c.parts {
        if p.resource_name_str1.is_empty() || p.resource_name_str1.starts_with("0x") {
            continue;
        }
        assert_eq!(
            p.resource_name1,
            HashId::from_i64(i64::from(nie_data::unlock_condition::crc32_str(
                &p.resource_name_str1
            ))),
            "resourceName1 ≠ crc32({})",
            p.resource_name_str1
        );
        verifies += 1;
    }
    assert!(
        verifies >= 380,
        "trop peu de noms en clair vérifiés : {verifies}"
    );

    // Un preset-recette porte le hash du nom de la part correspondante : `preset_01_normal`
    // est à la fois une part sélectionnable (catégorie 1) et une recette de 72 lignes.
    let id = HashId::from_i64(i64::from(nie_data::unlock_condition::crc32_str(
        "preset_01_normal",
    )));
    assert_eq!(c.recipe_of(id).len(), 72, "la recette de preset_01_normal");
    assert!(
        c.parts.iter().any(|p| p.resource_name1 == id),
        "part homonyme absente"
    );

    // Les catégories connues, telles que les noms de ressources les nomment (cf. le VFS,
    // `data/common/chr/_face/20_EDIT/`) : 13 = contour de visage, 6 = œil, 11 = sourcil.
    assert_eq!(c.parts_of(13).len(), 35);
    assert_eq!(c.parts_of(6).len(), 72);
    assert_eq!(c.parts_of(11).len(), 40);
    assert!(c.parts_of(6).iter().all(
        |p| p.resource_name_str1.starts_with("eye_") || p.resource_name_str1.starts_with("0x")
    ));
}
