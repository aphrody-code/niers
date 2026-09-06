#![allow(clippy::pedantic)]
//! Tests golden `user_name_plate` — valeurs réelles tirées de :
//! `user_name_plate/user_name_plate_config_1.03.50.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ (fichier → valeur confirmée)
//!
//! ### m_userNamePlateInfoList[0]
//! - `userNamePlateId`      = `"0x0976673C"` → `HashId(0x0976673C)`
//! - `userNamePlateNameId`  = `"0x011D2282"` → `HashId(0x011D2282)`
//! - `sortNo`               = 1
//! - `textureFileNameText`  = `"#/menu/200_icon/25_icon_nameplate/nm00001.g4tx"`
//! - `textureFileNameCrc`   = `"0xAF3937FF"`
//! - `mainTextureNameCrc`   = `"0xA892B3C2"`
//! - `shadowTextureNameCrc` = `"0x319BE278"`
//! - `nameFontStyle`        = `"0x66A0930A"`
//! - `flagIndex`            = 0
//! - `enableCond`           = `""` (active par défaut)
//!
//! ### m_userNamePlateInfoList[1]
//! - `userNamePlateId`      = `"0x907F3686"`
//! - `nameFontStyle`        = `"0x66A0930A"`
//! - `flagIndex`            = 1
//! - `enableCond`           = `"AAAAABgFNRftNPcACgEoAAYCNJB/NoYyAAAAAXg="`
//!
//! ### m_userNamePlateInfoList[18] (première entrée à police 0x72A1CF45)
//! - `userNamePlateId`      = `"0xE2537C50"`
//! - `textureFileNameText`  = `".../nm00307.g4tx"`
//! - `nameFontStyle`        = `"0x72A1CF45"`
//!
//! ### m_userNamePlateInfoList[53] (dernier)
//! - `userNamePlateId`      = `"0x39A13FD2"`
//! - `userNamePlateNameId`  = `"0x31CA7A6C"`
//! - `sortNo`               = 54
//! - `textureFileNameText`  = `".../nm05401.g4tx"`
//! - `nameFontStyle`        = `"0x72A1CF45"`
//! - `flagIndex`            = 53
//! - `enableCond`           = `"AAAAABgFNRftNPcACgEoAAYCNDmhP9IyAAAAAXg="`

mod common;

use nie_data::hash::HashId;
use nie_data::user_name_plate::{
    UserNamePlateConfig, UserNamePlateInfo, parse_user_name_plate_config,
};
use serde_json::json;

// ─── Fixture ────────────────────────────────────────────────────────────────
// Valeurs extraites octet-pour-octet du dump réel — voir commentaires ci-dessus.

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_userNamePlateInfoList",
                "typeName": "USER_NAME_PLATE_INFO",
                "values": [
                    // entrée 0 — enableCond vide (active par défaut)
                    {
                        "userNamePlateId":      "0x0976673C",
                        "userNamePlateNameId":  "0x011D2282",
                        "sortNo":               1,
                        "textureFileNameText":  "#/menu/200_icon/25_icon_nameplate/nm00001.g4tx",
                        "textureFileNameCrc":   "0xAF3937FF",
                        "mainTextureNameCrc":   "0xA892B3C2",
                        "shadowTextureNameCrc": "0x319BE278",
                        "nameFontStyle":        "0x66A0930A",
                        "flagIndex":            0,
                        "enableCond":           ""
                    },
                    // entrée 1 — enableCond base64 opaque
                    {
                        "userNamePlateId":      "0x907F3686",
                        "userNamePlateNameId":  "0x98147338",
                        "sortNo":               2,
                        "textureFileNameText":  "#/menu/200_icon/25_icon_nameplate/nm00002.g4tx",
                        "textureFileNameCrc":   "0x29AD4551",
                        "mainTextureNameCrc":   "0xBA271C2C",
                        "shadowTextureNameCrc": "0x232E4D96",
                        "nameFontStyle":        "0x66A0930A",
                        "flagIndex":            1,
                        "enableCond":           "AAAAABgFNRftNPcACgEoAAYCNJB/NoYyAAAAAXg="
                    },
                    // entrée 18 — première à police 0x72A1CF45
                    {
                        "userNamePlateId":      "0xE2537C50",
                        "userNamePlateNameId":  "0x00000000",
                        "sortNo":               19,
                        "textureFileNameText":  "#/menu/200_icon/25_icon_nameplate/nm00307.g4tx",
                        "textureFileNameCrc":   "0x00000000",
                        "mainTextureNameCrc":   "0x00000000",
                        "shadowTextureNameCrc": "0x00000000",
                        "nameFontStyle":        "0x72A1CF45",
                        "flagIndex":            18,
                        "enableCond":           ""
                    },
                    // entrée 53 (dernière du dump)
                    {
                        "userNamePlateId":      "0x39A13FD2",
                        "userNamePlateNameId":  "0x31CA7A6C",
                        "sortNo":               54,
                        "textureFileNameText":  "#/menu/200_icon/25_icon_nameplate/nm05401.g4tx",
                        "textureFileNameCrc":   "0x61E47C4A",
                        "mainTextureNameCrc":   "0x613BDE73",
                        "shadowTextureNameCrc": "0xF8328FC9",
                        "nameFontStyle":        "0x72A1CF45",
                        "flagIndex":            53,
                        "enableCond":           "AAAAABgFNRftNPcACgEoAAYCNDmhP9IyAAAAAXg="
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn fixture_compte_entrees() {
    let cfg = parse_user_name_plate_config(&fixture());
    assert_eq!(cfg.entries.len(), 4);
}

#[test]
fn fixture_entree0_champs_complets() {
    let cfg = parse_user_name_plate_config(&fixture());
    let e: &UserNamePlateInfo = &cfg.entries[0];

    assert_eq!(e.user_name_plate_id, HashId(0x0976_673C));
    assert_eq!(e.user_name_plate_name_id, HashId(0x011D_2282));
    assert_eq!(e.sort_no, 1);
    assert_eq!(
        e.texture_file_name_text,
        "#/menu/200_icon/25_icon_nameplate/nm00001.g4tx"
    );
    assert_eq!(e.texture_file_name_crc, HashId(0xAF39_37FF));
    assert_eq!(e.main_texture_name_crc, HashId(0xA892_B3C2));
    assert_eq!(e.shadow_texture_name_crc, HashId(0x319B_E278));
    assert_eq!(e.name_font_style, HashId(0x66A0_930A));
    assert_eq!(e.flag_index, 0);
    assert_eq!(e.enable_cond, "");
}

#[test]
fn fixture_entree1_enable_cond() {
    let cfg = parse_user_name_plate_config(&fixture());
    let e = &cfg.entries[1];
    assert_eq!(e.user_name_plate_id, HashId(0x907F_3686));
    assert_eq!(e.flag_index, 1);
    assert_eq!(e.enable_cond, "AAAAABgFNRftNPcACgEoAAYCNJB/NoYyAAAAAXg=");
}

#[test]
fn fixture_entree18_police_alternative() {
    let cfg = parse_user_name_plate_config(&fixture());
    let e = &cfg.entries[2];
    assert_eq!(e.user_name_plate_id, HashId(0xE253_7C50));
    assert_eq!(e.name_font_style, HashId(0x72A1_CF45));
}

#[test]
fn fixture_find_by_id() {
    let cfg = parse_user_name_plate_config(&fixture());
    let found = cfg.find_by_id(HashId(0x0976_673C));
    assert!(found.is_some());
    assert_eq!(found.unwrap().sort_no, 1);
    assert!(cfg.find_by_id(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn fixture_find_by_flag_index() {
    let cfg = parse_user_name_plate_config(&fixture());
    let found = cfg.find_by_flag_index(53);
    assert!(found.is_some());
    assert_eq!(found.unwrap().user_name_plate_id, HashId(0x39A1_3FD2));
    assert!(cfg.find_by_flag_index(999).is_none());
}

#[test]
fn fixture_liste_manquante_renvoie_vide() {
    let root = json!({ "version": 100, "lists": [] });
    let cfg = parse_user_name_plate_config(&root);
    assert_eq!(cfg.entries.len(), 0);
}

// ─── Test sur le vrai fichier (skip si absent du VPS) ────────────────────────

const REAL_PATH: &str = "user_name_plate/user_name_plate_config_1.03.50.00.cfg.bin.json";

fn load_real() -> Option<UserNamePlateConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_user_name_plate_config(&root))
}

#[test]
fn real_file_compte_total() {
    let Some(cfg) = load_real() else { return };
    assert_eq!(cfg.entries.len(), 54, "54 entrées USER_NAME_PLATE_INFO");
}

#[test]
fn real_file_entree0_valeurs() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.entries[0];
    assert_eq!(e.user_name_plate_id, HashId(0x0976_673C));
    assert_eq!(e.user_name_plate_name_id, HashId(0x011D_2282));
    assert_eq!(e.sort_no, 1);
    assert_eq!(
        e.texture_file_name_text,
        "#/menu/200_icon/25_icon_nameplate/nm00001.g4tx"
    );
    assert_eq!(e.texture_file_name_crc, HashId(0xAF39_37FF));
    assert_eq!(e.main_texture_name_crc, HashId(0xA892_B3C2));
    assert_eq!(e.shadow_texture_name_crc, HashId(0x319B_E278));
    assert_eq!(e.name_font_style, HashId(0x66A0_930A));
    assert_eq!(e.flag_index, 0);
    assert_eq!(e.enable_cond, "");
}

#[test]
fn real_file_entree1_valeurs() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.entries[1];
    assert_eq!(e.user_name_plate_id, HashId(0x907F_3686));
    assert_eq!(e.user_name_plate_name_id, HashId(0x9814_7338));
    assert_eq!(e.sort_no, 2);
    assert_eq!(e.flag_index, 1);
    assert_eq!(e.enable_cond, "AAAAABgFNRftNPcACgEoAAYCNJB/NoYyAAAAAXg=");
}

#[test]
fn real_file_entree18_police_alternative() {
    let Some(cfg) = load_real() else { return };
    // Première entrée à police 0x72A1CF45 (nm00307).
    let e = &cfg.entries[18];
    assert_eq!(e.user_name_plate_id, HashId(0xE253_7C50));
    assert_eq!(
        e.texture_file_name_text,
        "#/menu/200_icon/25_icon_nameplate/nm00307.g4tx"
    );
    assert_eq!(e.name_font_style, HashId(0x72A1_CF45));
}

#[test]
fn real_file_dernier_entree() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.entries[53];
    assert_eq!(e.user_name_plate_id, HashId(0x39A1_3FD2));
    assert_eq!(e.user_name_plate_name_id, HashId(0x31CA_7A6C));
    assert_eq!(e.sort_no, 54);
    assert_eq!(
        e.texture_file_name_text,
        "#/menu/200_icon/25_icon_nameplate/nm05401.g4tx"
    );
    assert_eq!(e.name_font_style, HashId(0x72A1_CF45));
    assert_eq!(e.flag_index, 53);
    assert_eq!(e.enable_cond, "AAAAABgFNRftNPcACgEoAAYCNDmhP9IyAAAAAXg=");
}

#[test]
fn real_file_sort_no_sequentiel() {
    let Some(cfg) = load_real() else { return };
    // sortNo = index + 1 sur tout le dump.
    for (i, e) in cfg.entries.iter().enumerate() {
        assert_eq!(e.sort_no, (i + 1) as i64, "sortNo séquentiel à l'index {i}");
    }
}

#[test]
fn real_file_flag_index_sequentiel() {
    let Some(cfg) = load_real() else { return };
    // flagIndex = index sur tout le dump.
    for (i, e) in cfg.entries.iter().enumerate() {
        assert_eq!(e.flag_index, i as i64, "flagIndex séquentiel à l'index {i}");
    }
}

#[test]
fn real_file_find_by_flag_index() {
    let Some(cfg) = load_real() else { return };
    let found = cfg.find_by_flag_index(0);
    assert!(found.is_some());
    assert_eq!(found.unwrap().user_name_plate_id, HashId(0x0976_673C));
}

#[test]
fn real_file_polices_distinctes() {
    let Some(cfg) = load_real() else { return };
    // Deux styles de police observés dans le dump.
    let a = HashId(0x66A0_930A);
    let b = HashId(0x72A1_CF45);
    assert!(
        cfg.entries
            .iter()
            .all(|e| e.name_font_style == a || e.name_font_style == b)
    );
}
