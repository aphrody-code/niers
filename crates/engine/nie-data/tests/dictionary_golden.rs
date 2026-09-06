#![allow(clippy::pedantic)]
//! Tests golden `dictionary` — valeurs réelles tirées de :
//! `dictionary/dictionary_config_0.00.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ (liste[index].champ → valeur confirmée)
//!
//! ### m_HabitatList[0]
//! - `habitatID` = "0xCA6655CD"
//! - `mapID`     = "0x00000000" (nul → habitat global)
//! - `mapNameID` = "0x79995A45"
//! - `fileName`  = "" (vide dans tout le dump)
//! - `textureNameCrc` = "0x00000000"
//! - `isShowAreaTexture` = false
//!
//! ### m_HabitatList[1]
//! - `habitatID` = "0x1A3F75EB"
//! - `mapID`     = "0x63A5A6D9"
//! - `isShowAreaTexture` = true
//!
//! ### m_ObservationDataList[0]
//! - `charaid`       = "0x00000000" (comportement par défaut)
//! - `actiontype_id` = "0x877B7FFF"
//!
//! ### m_ParamList[0]
//! - `charaID`           = "0xED912800"
//! - `medalID`           = "0x00000000"
//! - `weaponItemID`      = "0x00000000"
//! - `habitatID`         = "0x00000000"
//! - `descTextID`        = "0x8E7352EE"
//! - `viewDictNo`        = 1
//! - `viewType`          = 1
//! - `isButtle`          = false
//! - `category`          = 1
//! - `sub_category`      = 0
//! - `cond2`             = ""
//! - `cond3`             = "AAAAAA8FNbkZNtoAAQAyAAAnJHE=" (base64)
//! - `observation_data`  = [0, 1] → observation_offset=0, observation_count=1
//!
//! ### m_ParamList[5]
//! - `charaID`    = "0xF09418B8"
//! - `viewDictNo` = 6
//! - `observation_data` = [5, 1] → observation_offset=5, observation_count=1
//!
//! ### m_ObservationActionIdList[0]
//! - `action_id`         = "0x3226E9CA"
//! - `emotion_id`        = "0xD17DAF62"
//! - `disable_repeat_se` = false
//!
//! ### m_ObservationActionPlayList[0]
//! - `play_no`     = 0
//! - `acttion_list` = [0, 2] → action_offset=0, action_count=2
//!
//! ### m_ObservationActionPlayList[5]
//! - `play_no`     = 5
//! - `acttion_list` = [7, 2] → action_offset=7, action_count=2
//!
//! ### m_ObservationActionDataList[0]
//! - `actiontype_id` = "0x877B7FFF"
//! - `playlist`      = [0, 6] → play_offset=0, play_count=6
//!
//! ### m_ObservationActionDataList[5]
//! - `actiontype_id` = "0x5667F514"
//! - `playlist`      = [18, 3] → play_offset=18, play_count=3
//!
//! ### m_ObservationActionDataList[27] (dernière entrée)
//! - `actiontype_id` = "0x167C9D19"
//! - `playlist`      = [91, 3] → play_offset=91, play_count=3

mod common;

use nie_data::dictionary::{
    DictionaryConfig, DictionaryHabitatData, DictionaryObservationActionData,
    DictionaryObservationActionId, DictionaryObservationActionPlay, DictionaryObservationRefData,
    DictionaryParamData, parse_dictionary_config,
};
use nie_data::hash::HashId;
use serde_json::json;

// ─── Fixture ──────────────────────────────────────────────────────────────────
// Valeurs extraites octet-pour-octet du dump réel — voir commentaires ci-dessus.

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_HabitatList",
                "typeName": "DICTIONARY_HABITAT_DATA",
                "values": [
                    // entrée 0 — dump réel ligne 6-13
                    {
                        "habitatID": "0xCA6655CD",
                        "mapID": "0x00000000",
                        "mapNameID": "0x79995A45",
                        "fileName": "",
                        "textureNameCrc": "0x00000000",
                        "isShowAreaTexture": false
                    },
                    // entrée 1 — dump réel lignes 14-21
                    {
                        "habitatID": "0x1A3F75EB",
                        "mapID": "0x63A5A6D9",
                        "mapNameID": "0xBFEA3E1C",
                        "fileName": "",
                        "textureNameCrc": "0x00000000",
                        "isShowAreaTexture": true
                    }
                ]
            },
            {
                "name": "m_ObservationDataList",
                "typeName": "DICTIONARY_OBSERVATION_REF_DATA",
                "values": [
                    // entrée 0 — dump réel
                    { "charaid": "0x00000000", "actiontype_id": "0x877B7FFF" },
                    // entrée 1
                    { "charaid": "0x00000000", "actiontype_id": "0x877B7FFF" }
                ]
            },
            {
                "name": "m_ParamList",
                "typeName": "DICTIONARY_PARAM_DATA",
                "values": [
                    // entrée 0 — dump réel
                    {
                        "charaID": "0xED912800",
                        "medalID": "0x00000000",
                        "weaponItemID": "0x00000000",
                        "habitatID": "0x00000000",
                        "descTextID": "0x8E7352EE",
                        "viewDictNo": 1,
                        "viewType": 1,
                        "isButtle": false,
                        "category": 1,
                        "sub_category": 0,
                        "cond2": "",
                        "cond3": "AAAAAA8FNbkZNtoAAQAyAAAnJHE=",
                        "observation_data": [0, 1]
                    },
                    // entrée 1 (pour tester la tranche d'observations [1,1])
                    {
                        "charaID": "0x6B055AAE",
                        "medalID": "0x00000000",
                        "weaponItemID": "0x00000000",
                        "habitatID": "0x00000000",
                        "descTextID": "0x08E72040",
                        "viewDictNo": 2,
                        "viewType": 1,
                        "isButtle": false,
                        "category": 1,
                        "sub_category": 0,
                        "cond2": "",
                        "cond3": "AAAAAA8FNbkZNtoAAQAyAAAnJHE=",
                        "observation_data": [1, 1]
                    }
                ]
            },
            {
                "name": "m_ObservationActionIdList",
                "typeName": "DICTIONARY_OBSERVATION_ACTIONID_REF_DATA",
                "values": [
                    // entrée 0 — dump réel
                    { "action_id": "0x3226E9CA", "emotion_id": "0xD17DAF62", "disable_repeat_se": false },
                    // entrée 1
                    { "action_id": "0x86F0C191", "emotion_id": "0xD17DAF62", "disable_repeat_se": false }
                ]
            },
            {
                "name": "m_ObservationActionPlayList",
                "typeName": "DICTIONARY_OBSERVATION_ACTIONPLAY_REF_DATA",
                "values": [
                    // entrée 0 — dump réel
                    { "play_no": 0, "acttion_list": [0, 2] },
                    // entrée 1 — dump réel
                    { "play_no": 1, "acttion_list": [2, 2] }
                ]
            },
            {
                "name": "m_ObservationActionDataList",
                "typeName": "DICTIONARY_OBSERVATION_ACTION_DATA",
                "values": [
                    // entrée 0 — dump réel
                    { "actiontype_id": "0x877B7FFF", "playlist": [0, 2] },
                    // entrée 1 — dump réel (playlist réduite pour la fixture)
                    { "actiontype_id": "0x9D17E124", "playlist": [1, 1] }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn habitat_0_champs_complets() {
    let cfg = parse_dictionary_config(&fixture());
    assert_eq!(cfg.habitats.len(), 2);
    let h: &DictionaryHabitatData = &cfg.habitats[0];

    // dictionary_config_0.00.00 : m_HabitatList[0]
    assert_eq!(h.habitat_id, HashId(0xCA66_55CD));
    assert_eq!(h.map_id, HashId::ZERO, "mapID nul pour l'habitat global");
    assert_eq!(h.map_name_id, HashId(0x7999_5A45));
    assert_eq!(h.file_name, "");
    assert_eq!(h.texture_name_crc, HashId::ZERO);
    assert!(
        !h.is_show_area_texture,
        "isShowAreaTexture=false pour l'entrée 0"
    );
}

#[test]
fn habitat_1_show_area_texture() {
    let cfg = parse_dictionary_config(&fixture());
    let h: &DictionaryHabitatData = &cfg.habitats[1];

    // dictionary_config_0.00.00 : m_HabitatList[1]
    assert_eq!(h.habitat_id, HashId(0x1A3F_75EB));
    assert_eq!(h.map_id, HashId(0x63A5_A6D9));
    assert!(
        h.is_show_area_texture,
        "isShowAreaTexture=true pour l'entrée 1"
    );
}

#[test]
fn observation_ref_0_charaid_nul() {
    let cfg = parse_dictionary_config(&fixture());
    assert_eq!(cfg.observation_refs.len(), 2);
    let o: &DictionaryObservationRefData = &cfg.observation_refs[0];

    // dictionary_config_0.00.00 : m_ObservationDataList[0]
    assert_eq!(
        o.chara_id,
        HashId::ZERO,
        "charaid nul = comportement par défaut"
    );
    assert_eq!(o.actiontype_id, HashId(0x877B_7FFF));
}

#[test]
fn param_0_champs_complets() {
    let cfg = parse_dictionary_config(&fixture());
    assert_eq!(cfg.params.len(), 2);
    let p: &DictionaryParamData = &cfg.params[0];

    // dictionary_config_0.00.00 : m_ParamList[0]
    assert_eq!(p.chara_id, HashId(0xED91_2800));
    assert_eq!(p.medal_id, HashId::ZERO);
    assert_eq!(p.weapon_item_id, HashId::ZERO);
    assert_eq!(p.habitat_id, HashId::ZERO);
    assert_eq!(p.desc_text_id, HashId(0x8E73_52EE));
    assert_eq!(p.view_dict_no, 1);
    assert_eq!(p.view_type, 1);
    assert!(!p.is_buttle);
    assert_eq!(p.category, 1);
    assert_eq!(p.sub_category, 0);
    assert_eq!(p.cond2, "");
    assert_eq!(p.cond3, "AAAAAA8FNbkZNtoAAQAyAAAnJHE=");
    assert_eq!(p.observation_offset, 0);
    assert_eq!(p.observation_count, 1);
}

#[test]
fn param_1_observation_tranche() {
    let cfg = parse_dictionary_config(&fixture());
    let p: &DictionaryParamData = &cfg.params[1];

    // dictionary_config_0.00.00 : m_ParamList[1] — observation_data=[1,1]
    assert_eq!(p.chara_id, HashId(0x6B05_5AAE));
    assert_eq!(p.view_dict_no, 2);
    assert_eq!(p.observation_offset, 1);
    assert_eq!(p.observation_count, 1);
}

#[test]
fn action_id_0_champs() {
    let cfg = parse_dictionary_config(&fixture());
    assert_eq!(cfg.observation_action_ids.len(), 2);
    let a: &DictionaryObservationActionId = &cfg.observation_action_ids[0];

    // dictionary_config_0.00.00 : m_ObservationActionIdList[0]
    assert_eq!(a.action_id, HashId(0x3226_E9CA));
    assert_eq!(a.emotion_id, HashId(0xD17D_AF62));
    assert!(!a.disable_repeat_se);
}

#[test]
fn action_play_0_champs() {
    let cfg = parse_dictionary_config(&fixture());
    assert_eq!(cfg.observation_action_plays.len(), 2);
    let pl: &DictionaryObservationActionPlay = &cfg.observation_action_plays[0];

    // dictionary_config_0.00.00 : m_ObservationActionPlayList[0]
    assert_eq!(pl.play_no, 0);
    assert_eq!(pl.action_offset, 0);
    assert_eq!(pl.action_count, 2);
}

#[test]
fn action_data_0_champs() {
    let cfg = parse_dictionary_config(&fixture());
    assert_eq!(cfg.observation_action_data.len(), 2);
    let d: &DictionaryObservationActionData = &cfg.observation_action_data[0];

    // dictionary_config_0.00.00 : m_ObservationActionDataList[0]
    assert_eq!(d.actiontype_id, HashId(0x877B_7FFF));
    assert_eq!(d.play_offset, 0);
    assert_eq!(d.play_count, 2);
}

#[test]
fn observation_refs_of_tranche() {
    let cfg = parse_dictionary_config(&fixture());
    // param[0] référence observation_refs[0..1]
    let refs = cfg.observation_refs_of(&cfg.params[0]);
    assert_eq!(refs.len(), 1, "observation_count=1");
    assert_eq!(refs[0].actiontype_id, HashId(0x877B_7FFF));
}

#[test]
fn plays_of_tranche() {
    let cfg = parse_dictionary_config(&fixture());
    // observation_action_data[0] a play_offset=0, play_count=2
    let plays = cfg.plays_of(&cfg.observation_action_data[0]);
    assert_eq!(plays.len(), 2, "play_count=2");
    assert_eq!(plays[0].play_no, 0);
    assert_eq!(plays[1].play_no, 1);
}

#[test]
fn action_ids_of_tranche() {
    let cfg = parse_dictionary_config(&fixture());
    // observation_action_plays[0] a action_offset=0, action_count=2
    let ids = cfg.action_ids_of(&cfg.observation_action_plays[0]);
    assert_eq!(ids.len(), 2, "action_count=2");
    assert_eq!(ids[0].action_id, HashId(0x3226_E9CA));
    assert_eq!(ids[1].action_id, HashId(0x86F0_C191));
}

#[test]
fn find_param_by_chara() {
    let cfg = parse_dictionary_config(&fixture());
    assert!(cfg.find_param_by_chara(HashId(0xED91_2800)).is_some());
    assert!(cfg.find_param_by_chara(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn find_habitat() {
    let cfg = parse_dictionary_config(&fixture());
    assert!(cfg.find_habitat(HashId(0xCA66_55CD)).is_some());
    assert!(cfg.find_habitat(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn tranche_hors_bornes_renvoie_vide() {
    let cfg = parse_dictionary_config(&fixture());
    // Tranche au-delà de la taille de la liste → &[]
    let fake_play = DictionaryObservationActionPlay {
        play_no: 0,
        action_offset: 9999,
        action_count: 1,
    };
    assert_eq!(cfg.action_ids_of(&fake_play).len(), 0);
}

// ─── Test sur le vrai fichier (skip si absent du VPS) ─────────────────────────

const REAL_PATH: &str = "dictionary/dictionary_config_0.00.00.cfg.bin.json";

fn load_real() -> Option<DictionaryConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_dictionary_config(&root))
}

#[test]
fn real_file_comptes() {
    let Some(cfg) = load_real() else { return };

    // Comptes vérifiés sur le dump réel
    assert_eq!(cfg.habitats.len(), 43, "43 zones d'habitat");
    assert_eq!(
        cfg.observation_refs.len(),
        280,
        "280 références observation"
    );
    assert_eq!(cfg.params.len(), 280, "280 fiches encyclopédie");
    assert_eq!(
        cfg.observation_action_ids.len(),
        178,
        "178 identifiants d'action"
    );
    assert_eq!(
        cfg.observation_action_plays.len(),
        94,
        "94 playlists d'action"
    );
    assert_eq!(
        cfg.observation_action_data.len(),
        28,
        "28 types d'action d'observation"
    );
}

#[test]
fn real_file_habitat_0() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_HabitatList[0]
    let h = &cfg.habitats[0];
    assert_eq!(h.habitat_id, HashId(0xCA66_55CD));
    assert_eq!(h.map_id, HashId::ZERO, "mapID nul");
    assert_eq!(h.map_name_id, HashId(0x7999_5A45));
    assert!(!h.is_show_area_texture);
}

#[test]
fn real_file_habitat_1_show_area() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_HabitatList[1]
    let h = &cfg.habitats[1];
    assert_eq!(h.habitat_id, HashId(0x1A3F_75EB));
    assert_eq!(h.map_id, HashId(0x63A5_A6D9));
    assert!(
        h.is_show_area_texture,
        "HabitatList[1] isShowAreaTexture=true"
    );
}

#[test]
fn real_file_param_0() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_ParamList[0]
    let p = &cfg.params[0];
    assert_eq!(p.chara_id, HashId(0xED91_2800));
    assert_eq!(p.desc_text_id, HashId(0x8E73_52EE));
    assert_eq!(p.view_dict_no, 1);
    assert_eq!(p.view_type, 1);
    assert_eq!(p.category, 1);
    assert_eq!(p.sub_category, 0);
    assert!(!p.is_buttle);
    assert_eq!(p.cond3, "AAAAAA8FNbkZNtoAAQAyAAAnJHE=");
    assert_eq!(p.observation_offset, 0);
    assert_eq!(p.observation_count, 1);
}

#[test]
fn real_file_param_5() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_ParamList[5]
    let p = &cfg.params[5];
    assert_eq!(p.chara_id, HashId(0xF094_18B8));
    assert_eq!(p.view_dict_no, 6);
    assert_eq!(p.observation_offset, 5);
    assert_eq!(p.observation_count, 1);
}

#[test]
fn real_file_observation_data_0() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_ObservationDataList[0]
    let o = &cfg.observation_refs[0];
    assert_eq!(o.chara_id, HashId::ZERO, "charaid nul = défaut");
    assert_eq!(o.actiontype_id, HashId(0x877B_7FFF));
}

#[test]
fn real_file_action_id_0() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_ObservationActionIdList[0]
    let a = &cfg.observation_action_ids[0];
    assert_eq!(a.action_id, HashId(0x3226_E9CA));
    assert_eq!(a.emotion_id, HashId(0xD17D_AF62));
    assert!(!a.disable_repeat_se);
}

#[test]
fn real_file_action_play_0() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_ObservationActionPlayList[0]
    let pl = &cfg.observation_action_plays[0];
    assert_eq!(pl.play_no, 0);
    assert_eq!(pl.action_offset, 0);
    assert_eq!(pl.action_count, 2);
}

#[test]
fn real_file_action_play_5() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_ObservationActionPlayList[5]
    let pl = &cfg.observation_action_plays[5];
    assert_eq!(pl.play_no, 5);
    assert_eq!(pl.action_offset, 7);
    assert_eq!(pl.action_count, 2);
}

#[test]
fn real_file_action_data_0() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_ObservationActionDataList[0]
    let d = &cfg.observation_action_data[0];
    assert_eq!(d.actiontype_id, HashId(0x877B_7FFF));
    assert_eq!(d.play_offset, 0);
    assert_eq!(d.play_count, 6);
}

#[test]
fn real_file_action_data_5() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_ObservationActionDataList[5]
    let d = &cfg.observation_action_data[5];
    assert_eq!(d.actiontype_id, HashId(0x5667_F514));
    assert_eq!(d.play_offset, 18);
    assert_eq!(d.play_count, 3);
}

#[test]
fn real_file_action_data_27_derniere_entree() {
    let Some(cfg) = load_real() else { return };

    // dictionary_config_0.00.00 : m_ObservationActionDataList[27] (dernière entrée)
    let d = &cfg.observation_action_data[27];
    assert_eq!(d.actiontype_id, HashId(0x167C_9D19));
    assert_eq!(d.play_offset, 91);
    assert_eq!(d.play_count, 3);
}

#[test]
fn real_file_observation_refs_of_param0() {
    let Some(cfg) = load_real() else { return };

    // param[0] → observation_refs[0..1]
    let refs = cfg.observation_refs_of(&cfg.params[0]);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].actiontype_id, HashId(0x877B_7FFF));
}

#[test]
fn real_file_find_param_par_chara() {
    let Some(cfg) = load_real() else { return };

    // Recherche par charaID
    let found = cfg.find_param_by_chara(HashId(0xED91_2800));
    assert!(found.is_some(), "charaID 0xED912800 doit exister");
    assert_eq!(found.unwrap().view_dict_no, 1);
}

#[test]
fn real_file_find_habitat_existant() {
    let Some(cfg) = load_real() else { return };

    let h = cfg.find_habitat(HashId(0xCA66_55CD));
    assert!(h.is_some(), "habitatID 0xCA6655CD doit exister");
    assert!(!h.unwrap().is_show_area_texture);
}
