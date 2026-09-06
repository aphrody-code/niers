// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::pedantic)]
//! Tests golden `soccer_performance` — **les 16 entrées réelles** de la liste
//! `m_soccerPerformanceConfigList` (type `SOCCER_PERFORMANCE_CONFIG`) tirées de :
//! `data/common/gamedata/soccer/soccer_performance_config_0.00.00.00.cfg.bin`
//! (VFS du jeu Steam ; extrait via `nie_formats::cfgbin` → forme iecode `lists`).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/performance-config.ts`
//! (`parseContent` l.51-77, `extractImageName` l.43-45). Valeurs copiées telles quelles
//! du dump (les `textureFilePath` couvrent `type_01`..`type_06` puis `type_08`..`type_17` —
//! `type_07` est réellement absent, 6 + 10 = 16 entrées).

use nie_data::hash::HashId;
use nie_data::soccer_performance::{
    SoccerPerformanceConfig, find_performance, parse_performance_config,
};
use serde_json::json;

/// Fixture = les 16 valeurs réelles de `m_soccerPerformanceConfigList` (dump VFS), forme iecode.
fn node_fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [{
            "name": "m_soccerPerformanceConfigList",
            "typeName": "SOCCER_PERFORMANCE_CONFIG",
            "values": [
                { "performanceId": "0x16E827AF", "eventId": "0x83BCE8A4", "eventNameTextId": "0xD5826DD1",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_01.g4tx",
                  "textureId": "0x31D18379", "validCond": "AAAAABgFNRftNPcACgEoAAYCNBboJ68yAAAAAXg=" },
                { "performanceId": "0x8FE17615", "eventId": "0xA891BB67", "eventNameTextId": "0x4C8B3C6B",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_02.g4tx",
                  "textureId": "0xA8D8D2C3", "validCond": "AAAAABgFNRftNPcACgEoAAYCNI/hdhUyAAAAAXg=" },
                { "performanceId": "0xF8E64683", "eventId": "0xB18A8A26", "eventNameTextId": "0x3B8C0CFD",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_03.g4tx",
                  "textureId": "0xDFDFE255", "validCond": "AAAAABgFNRftNPcACgEoAAYCNPjmRoMyAAAAAXg=" },
                { "performanceId": "0x6682D320", "eventId": "0xFECB1CE1", "eventNameTextId": "0xA5E8995E",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_04.g4tx",
                  "textureId": "0x41BB77F6", "validCond": "AAAAABgFNRftNPcACgEoAAYCNGaC0yAyAAAAAXg=" },
                { "performanceId": "0x1185E3B6", "eventId": "0xE7D02DA0", "eventNameTextId": "0xD2EFA9C8",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_05.g4tx",
                  "textureId": "0x36BC4760", "validCond": "AAAAABgFNRftNPcACgEoAAYCNBGF47YyAAAAAXg=" },
                { "performanceId": "0x888CB20C", "eventId": "0xCCFD7E63", "eventNameTextId": "0x4BE6F872",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_06.g4tx",
                  "textureId": "0xAFB516DA", "validCond": "AAAAABgFNRftNPcACgEoAAYCNIiMsgwyAAAAAXg=" },
                { "performanceId": "0x6F349F0B", "eventId": "0x527E53ED", "eventNameTextId": "0x3CE1C8E4",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_08.g4tx",
                  "textureId": "0x480D3BDD", "validCond": "AAAAABgFNRftNPcACgEoAAYCNG80nwsyAAAAAXg=" },
                { "performanceId": "0x1833AF9D", "eventId": "0x4B6562AC", "eventNameTextId": "0xAC5ED575",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_09.g4tx",
                  "textureId": "0x3F0A0B4B", "validCond": "AAAAABgFNRftNPcACgEoAAYCNBgzr50yAAAAAXg=" },
                { "performanceId": "0x78F42678", "eventId": "0x9B65B3D2", "eventNameTextId": "0xDB59E5E3",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_10.g4tx",
                  "textureId": "0x5FCD82AE", "validCond": "AAAAABgFNRftNPcACgEoAAYCNHj0JngyAAAAAXg=" },
                { "performanceId": "0x0FF316EE", "eventId": "0x827E8293", "eventNameTextId": "0xBB9E6C06",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_11.g4tx",
                  "textureId": "0x28CAB238", "validCond": "AAAAABgFNRftNPcACgEoAAYCNA/zFu4yAAAAAXg=" },
                { "performanceId": "0x96FA4754", "eventId": "0xA953D150", "eventNameTextId": "0xCC995C90",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_12.g4tx",
                  "textureId": "0xB1C3E382", "validCond": "AAAAABgFNRftNPcACgEoAAYCNJb6R1QyAAAAAXg=" },
                { "performanceId": "0xE1FD77C2", "eventId": "0xB048E011", "eventNameTextId": "0x55900D2A",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_13.g4tx",
                  "textureId": "0xC6C4D314", "validCond": "AAAAABgFNRftNPcACgEoAAYCNOH9d8IyAAAAAXg=" },
                { "performanceId": "0x7F99E261", "eventId": "0xFF0976D6", "eventNameTextId": "0x22973DBC",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_14.g4tx",
                  "textureId": "0x58A046B7", "validCond": "AAAAABgFNRftNPcACgEoAAYCNH+Z4mEyAAAAAXg=" },
                { "performanceId": "0x089ED2F7", "eventId": "0xE6124797", "eventNameTextId": "0xBCF3A81F",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_15.g4tx",
                  "textureId": "0x2FA77621", "validCond": "AAAAABgFNRftNPcACgEoAAYCNAie0vcyAAAAAXg=" },
                { "performanceId": "0x9197834D", "eventId": "0xCD3F1454", "eventNameTextId": "0xCBF49889",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_16.g4tx",
                  "textureId": "0xB6AE279B", "validCond": "AAAAABgFNRftNPcACgEoAAYCNJGXg00yAAAAAXg=" },
                { "performanceId": "0xE690B3DB", "eventId": "0xD4242515", "eventNameTextId": "0x52FDC933",
                  "textureFilePath": "#/menu/220_img/performance_img/img_performance_type_17.g4tx",
                  "textureId": "0xC1A9170D", "validCond": "AAAAABgFNRftNPcACgEoAAYCNOaQs9syAAAAAXg=" }
            ]
        }]
    })
}

#[test]
fn performance_config_compte_16_entrees() {
    let perfs = parse_performance_config(&node_fixture());
    assert_eq!(
        perfs.len(),
        16,
        "m_soccerPerformanceConfigList = 16 valeurs (dump VFS)"
    );
}

#[test]
fn performance_entree_0_champs_complets() {
    let perfs = parse_performance_config(&node_fixture());
    let p: &SoccerPerformanceConfig = &perfs[0];
    assert_eq!(p.performance_id, HashId(0x16E8_27AF), "performanceId");
    assert_eq!(p.event_id, HashId(0x83BC_E8A4), "eventId");
    assert_eq!(p.event_name_text_id, HashId(0xD582_6DD1), "eventNameTextId");
    assert_eq!(
        p.texture_file_path, "#/menu/220_img/performance_img/img_performance_type_01.g4tx",
        "textureFilePath brut"
    );
    assert_eq!(p.texture_id, HashId(0x31D1_8379), "textureId");
    assert_eq!(
        p.valid_cond, "AAAAABgFNRftNPcACgEoAAYCNBboJ68yAAAAAXg=",
        "validCond brut"
    );
}

#[test]
fn performance_image_name_extrait_comme_inagle() {
    let perfs = parse_performance_config(&node_fixture());
    // extractImageName : retire "#/menu/220_img/" et ".g4tx" (première occurrence).
    assert_eq!(
        perfs[0].image_name(),
        "performance_img/img_performance_type_01"
    );
    assert_eq!(
        perfs[6].image_name(),
        "performance_img/img_performance_type_08"
    );
    assert_eq!(
        perfs[15].image_name(),
        "performance_img/img_performance_type_17"
    );
}

#[test]
fn performance_entree_6_saut_de_type_07() {
    // type_07 est réellement absent du dump : l'entrée d'index 6 pointe sur type_08.
    let perfs = parse_performance_config(&node_fixture());
    let p = &perfs[6];
    assert_eq!(p.performance_id, HashId(0x6F34_9F0B));
    assert_eq!(
        p.texture_file_path,
        "#/menu/220_img/performance_img/img_performance_type_08.g4tx"
    );
}

#[test]
fn performance_derniere_entree() {
    let perfs = parse_performance_config(&node_fixture());
    let p = &perfs[15];
    assert_eq!(
        p.performance_id,
        HashId(0xE690_B3DB),
        "performanceId dernière entrée"
    );
    assert_eq!(p.event_id, HashId(0xD424_2515), "eventId");
    assert_eq!(p.event_name_text_id, HashId(0x52FD_C933), "eventNameTextId");
    assert_eq!(p.texture_id, HashId(0xC1A9_170D), "textureId");
}

#[test]
fn performance_find_by_id() {
    let perfs = parse_performance_config(&node_fixture());
    let found =
        find_performance(&perfs, HashId(0x888C_B20C)).expect("performance 0x888CB20C trouvée");
    assert_eq!(found.texture_id, HashId(0xAFB5_16DA));
    assert_eq!(
        found.image_name(),
        "performance_img/img_performance_type_06"
    );
    assert!(
        find_performance(&perfs, HashId(0xDEAD_BEEF)).is_none(),
        "id inconnu → None"
    );
}

#[test]
fn performance_tous_les_ids_distincts() {
    // Les 16 performanceId sont tous distincts (clé de map byId d'inagle sans collision).
    let perfs = parse_performance_config(&node_fixture());
    for (i, a) in perfs.iter().enumerate() {
        for b in &perfs[i + 1..] {
            assert_ne!(a.performance_id, b.performance_id, "performanceId dupliqué");
        }
    }
}
