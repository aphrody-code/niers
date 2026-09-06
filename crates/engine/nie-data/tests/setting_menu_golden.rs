#![allow(clippy::pedantic)]
//! Tests golden `setting_menu` — valeurs réelles tirées de :
//! `setting_menu/setting_list_config_3.00.18.cfg.bin.json`
//!
//! Format `entries` (noeuds nommés, variables positionnelles). 11 listes racines.
//!
//! ## Valeurs confirmées octet-pour-octet (fichier → valeur)
//!
//! ### SETTING_INFO_0 (8 vars)
//! `[0, 977641936, 1497198424, 1654251620, 1832944595, 1, 1, -1132488914]`
//! → kind=0, setting_id=0x3A45A1D0, name_text_id=0x593D6F58, help_text_id=0x6299E064,
//!   group_id=0x6D4083D3, value=1, sub_kind=1, platform_type_id=0xBC7F972E
//!
//! ### KEYCONFIG_SETTING_INFO_71 (9 vars)
//! `[2, 461918172, -1657685415, 7528472, 110178791, 1, 0, 1255198513, 13]`
//! → kind=2, key_id=0x1B884FDC, name_text_id=0x9D31BA59, group_id=0x0072E018,
//!   help_text_id=0x069131E7, param_a=1, param_b=0, ref_id=0x4AD0CF31, param_c=13
//!
//! ### SETTING_PLATFORM_TYPE_INFO_0 / _1
//! _0 : platform_id=0xBC7F972E, flags=[1,1,1,1,1,1,1,1]
//! _1 : platform_id=0x3E4602C7, flags=[1,1,1,0,0,0,0,1]
//!
//! ### SETTING_OBJ_INFO_2 (motif INFO+REF)
//! clé=0x6D4083D3, ref=[0,3] → objects=[0x9CEBE295, 0x05E2B32F, 0x72E583B9]
//!
//! ### KEYCONFIG_MAPPING_LIST_INFO_0
//! clé=0xC10919BC, ref=[0,7] ; MAPPING_DATA_0=[0x97905153, 0xC10919BC, 0x949EEA04, 0]
//!
//! ### EXPLANATION_TEXT_LIST_INFO_0
//! clé=0x6299E064, ref=[0,1] → lines=[0x3F96387B] (clé = SETTING_INFO_0.help_text_id)
//!
//! ### PAD_GROUPKEY_LIST_INFO_0 / _1
//! _0 : clé=0xD3D99E8B, ref=[0,2] → keys=[12, 15]
//! _1 : clé=0x4AD0CF31, ref=[2,4] → keys=[10, 11, 8, 9]

mod common;

use nie_data::hash::HashId;
use nie_data::setting_menu::{SettingListConfig, parse_setting_list_config};
use serde_json::{Value, json};

// ─── Fixture synthétique (valeurs réelles) ────────────────────────────────────

/// Construit un noeud `{name, variables:[Int...], children:[...]}`.
fn node(name: &str, vars: &[i64], children: Value) -> Value {
    let variables: Vec<Value> = vars
        .iter()
        .map(|v| json!({"type": "Int", "value": v.to_string()}))
        .collect();
    json!({"name": name, "variables": variables, "children": children})
}

fn fixture() -> Value {
    json!({
        "entries": [
            node("SETTING_INFO_LIST_BEG_0", &[2], json!([
                node("SETTING_INFO_0",
                    &[0, 977641936, 1497198424, 1654251620, 1832944595, 1, 1, -1132488914], json!([])),
                node("SETTING_INFO_1",
                    &[0, -1555238806, -1070317854, -74403362, 1832944595, 2, 1, -1132488914], json!([])),
            ])),
            node("SETTING_PLATFORM_TYPE_INFO_LIST_BEG_0", &[2], json!([
                node("SETTING_PLATFORM_TYPE_INFO_0",
                    &[-1132488914, 1, 1, 1, 1, 1, 1, 1, 1], json!([])),
                node("SETTING_PLATFORM_TYPE_INFO_1",
                    &[1044775623, 1, 1, 1, 0, 0, 0, 0, 1], json!([])),
            ])),
            node("SETTING_OBJ_DATA_LIST_BEG_0", &[3], json!([
                node("SETTING_OBJ_DATA_0", &[-1662262635], json!([])),
                node("SETTING_OBJ_DATA_1", &[98743087], json!([])),
                node("SETTING_OBJ_DATA_2", &[1927644089], json!([])),
            ])),
            node("SETTING_OBJ_INFO_LIST_BEG_0", &[1], json!([
                node("SETTING_OBJ_INFO_0", &[1832944595], json!([])),
                node("SETTING_OBJ_INFO_REF_DATA_0", &[0, 3], json!([])),
            ])),
            node("PAD_GROUPKEY_LIST_DATA_LIST_BEG_0", &[6], json!([
                node("PAD_GROUPKEY_LIST_DATA_0", &[12], json!([])),
                node("PAD_GROUPKEY_LIST_DATA_1", &[15], json!([])),
                node("PAD_GROUPKEY_LIST_DATA_2", &[10], json!([])),
                node("PAD_GROUPKEY_LIST_DATA_3", &[11], json!([])),
                node("PAD_GROUPKEY_LIST_DATA_4", &[8], json!([])),
                node("PAD_GROUPKEY_LIST_DATA_5", &[9], json!([])),
            ])),
            node("PAD_GROUPKEY_LIST_INFO_LIST_BEG_0", &[2], json!([
                node("PAD_GROUPKEY_LIST_INFO_0", &[-740712821], json!([])),
                node("PAD_GROUPKEY_LIST_INFO_REF_DATA_0", &[0, 2], json!([])),
                node("PAD_GROUPKEY_LIST_INFO_1", &[1255198513], json!([])),
                node("PAD_GROUPKEY_LIST_INFO_REF_DATA_1", &[2, 4], json!([])),
            ])),
        ]
    })
}

#[test]
fn fixture_comptes() {
    let cfg = parse_setting_list_config(&fixture());
    assert_eq!(cfg.settings.len(), 2);
    assert_eq!(cfg.platform_types.len(), 2);
    assert_eq!(cfg.obj_infos.len(), 1);
    assert_eq!(cfg.pad_groupkeys.len(), 2);
    // Listes absentes de la fixture → vecteurs vides, sans panique.
    assert_eq!(cfg.keyconfig_settings.len(), 0);
    assert_eq!(cfg.mapping_infos.len(), 0);
    assert_eq!(cfg.explanation_texts.len(), 0);
}

#[test]
fn fixture_setting_info_0() {
    let cfg = parse_setting_list_config(&fixture());
    let s = &cfg.settings[0];
    assert_eq!(s.kind, 0);
    assert_eq!(s.setting_id, HashId(0x3A45_A1D0));
    assert_eq!(s.name_text_id, HashId(0x593D_6F58));
    assert_eq!(s.help_text_id, HashId(0x6299_E064));
    assert_eq!(s.group_id, HashId(0x6D40_83D3));
    assert_eq!(s.value, 1);
    assert_eq!(s.sub_kind, 1);
    assert_eq!(s.platform_type_id, HashId(0xBC7F_972E));
}

#[test]
fn fixture_platform_flags() {
    let cfg = parse_setting_list_config(&fixture());
    assert_eq!(cfg.platform_types[0].platform_id, HashId(0xBC7F_972E));
    assert_eq!(cfg.platform_types[0].flags, vec![1, 1, 1, 1, 1, 1, 1, 1]);
    assert_eq!(cfg.platform_types[1].platform_id, HashId(0x3E46_02C7));
    assert_eq!(cfg.platform_types[1].flags, vec![1, 1, 1, 0, 0, 0, 0, 1]);
}

#[test]
fn fixture_obj_info_ref_resolue() {
    let cfg = parse_setting_list_config(&fixture());
    let o = &cfg.obj_infos[0];
    assert_eq!(o.key, HashId(0x6D40_83D3));
    assert_eq!(
        o.objects,
        vec![
            HashId(0x9CEB_E295),
            HashId(0x05E2_B32F),
            HashId(0x72E5_83B9)
        ]
    );
}

#[test]
fn fixture_pad_groupkey_refs() {
    let cfg = parse_setting_list_config(&fixture());
    assert_eq!(cfg.pad_groupkeys[0].key, HashId(0xD3D9_9E8B));
    assert_eq!(cfg.pad_groupkeys[0].keys, vec![12, 15]);
    assert_eq!(cfg.pad_groupkeys[1].key, HashId(0x4AD0_CF31));
    assert_eq!(cfg.pad_groupkeys[1].keys, vec![10, 11, 8, 9]);
}

#[test]
fn fixture_accesseurs() {
    let cfg = parse_setting_list_config(&fixture());
    assert!(cfg.find_setting(HashId(0x3A45_A1D0)).is_some());
    assert!(cfg.find_setting(HashId(0xDEAD_BEEF)).is_none());
    let s = cfg.find_setting(HashId(0x3A45_A1D0)).unwrap();
    // platform_of résout le platform_type_id du réglage vers son type.
    let p = cfg.platform_of(s).unwrap();
    assert_eq!(p.platform_id, HashId(0xBC7F_972E));
    assert_eq!(p.flags, vec![1, 1, 1, 1, 1, 1, 1, 1]);
}

#[test]
fn fixture_liste_vide() {
    let cfg = parse_setting_list_config(&json!({ "entries": [] }));
    assert_eq!(cfg.settings.len(), 0);
    assert_eq!(cfg.obj_infos.len(), 0);
}

// ─── Test sur le vrai fichier (skip si absent du VPS) ─────────────────────────

const REAL_PATH: &str = "setting_menu/setting_list_config_3.00.18.cfg.bin.json";

fn load_real() -> Option<SettingListConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_setting_list_config(&root))
}

#[test]
fn real_comptes_logiques() {
    let Some(cfg) = load_real() else { return };
    // var[0] des noeuds *_BEG_0 = comptes logiques.
    assert_eq!(cfg.settings.len(), 73, "SETTING_INFO");
    assert_eq!(cfg.keyconfig_settings.len(), 72, "KEYCONFIG_SETTING_INFO");
    assert_eq!(cfg.platform_types.len(), 7, "SETTING_PLATFORM_TYPE_INFO");
    assert_eq!(cfg.obj_infos.len(), 29, "SETTING_OBJ_INFO");
    assert_eq!(cfg.mapping_infos.len(), 86, "KEYCONFIG_MAPPING_LIST_INFO");
    assert_eq!(
        cfg.explanation_texts.len(),
        90,
        "EXPLANATION_TEXT_LIST_INFO"
    );
    assert_eq!(cfg.pad_groupkeys.len(), 11, "PAD_GROUPKEY_LIST_INFO");
}

#[test]
fn real_setting_info_0() {
    let Some(cfg) = load_real() else { return };
    let s = &cfg.settings[0];
    assert_eq!(s.kind, 0);
    assert_eq!(s.setting_id, HashId(0x3A45_A1D0));
    assert_eq!(s.name_text_id, HashId(0x593D_6F58));
    assert_eq!(s.help_text_id, HashId(0x6299_E064));
    assert_eq!(s.group_id, HashId(0x6D40_83D3));
    assert_eq!(s.value, 1);
    assert_eq!(s.sub_kind, 1);
    assert_eq!(s.platform_type_id, HashId(0xBC7F_972E));
}

#[test]
fn real_setting_info_72_dernier() {
    let Some(cfg) = load_real() else { return };
    let s = &cfg.settings[72];
    assert_eq!(s.kind, 3);
    assert_eq!(s.setting_id, HashId(0x50F2_ECE6));
    assert_eq!(s.name_text_id, HashId(0x338A_226E));
    assert_eq!(s.help_text_id, HashId(0x082E_AD52));
    assert_eq!(s.group_id, HashId(0x8D95_6ACD));
    assert_eq!(s.value, 0x31);
    assert_eq!(s.platform_type_id, HashId(0xAA52_53B7));
}

#[test]
fn real_keyconfig_setting_71() {
    let Some(cfg) = load_real() else { return };
    let k = &cfg.keyconfig_settings[71];
    assert_eq!(k.kind, 2);
    assert_eq!(k.key_id, HashId(0x1B88_4FDC));
    assert_eq!(k.name_text_id, HashId(0x9D31_BA59));
    assert_eq!(k.group_id, HashId(0x0072_E018));
    assert_eq!(k.help_text_id, HashId(0x0691_31E7));
    assert_eq!(k.param_a, 1);
    assert_eq!(k.param_b, 0);
    assert_eq!(k.ref_id, HashId(0x4AD0_CF31));
    assert_eq!(k.param_c, 13);
}

#[test]
fn real_platform_types() {
    let Some(cfg) = load_real() else { return };
    let p0 = &cfg.platform_types[0];
    assert_eq!(p0.platform_id, HashId(0xBC7F_972E));
    assert_eq!(p0.flags, vec![1, 1, 1, 1, 1, 1, 1, 1]);
    let p1 = &cfg.platform_types[1];
    assert_eq!(p1.platform_id, HashId(0x3E46_02C7));
    assert_eq!(p1.flags, vec![1, 1, 1, 0, 0, 0, 0, 1]);
    // 8 drapeaux par type.
    assert!(cfg.platform_types.iter().all(|p| p.flags.len() == 8));
}

#[test]
fn real_obj_info_2_ref_resolue() {
    let Some(cfg) = load_real() else { return };
    // SETTING_OBJ_INFO_2 : clé 0x6D4083D3, ref [0,3].
    let o = &cfg.obj_infos[2];
    assert_eq!(o.key, HashId(0x6D40_83D3));
    assert_eq!(
        o.objects,
        vec![
            HashId(0x9CEB_E295),
            HashId(0x05E2_B32F),
            HashId(0x72E5_83B9)
        ]
    );
    // Les deux premières entrées ont une plage vide ([0,0]).
    assert!(cfg.obj_infos[0].objects.is_empty());
    assert!(cfg.obj_infos[1].objects.is_empty());
}

#[test]
fn real_obj_info_28_dernier() {
    let Some(cfg) = load_real() else { return };
    // SETTING_OBJ_INFO_28 : clé 0xA863B13C, ref [56,2].
    let o = &cfg.obj_infos[28];
    assert_eq!(o.key, HashId(0xA863_B13C));
    assert_eq!(o.objects, vec![HashId(0x513C_AF69), HashId(0xC835_FED3)]);
}

#[test]
fn real_mapping_info_0() {
    let Some(cfg) = load_real() else { return };
    let m = &cfg.mapping_infos[0];
    assert_eq!(m.key, HashId(0xC109_19BC));
    assert_eq!(m.mappings.len(), 7, "ref [0,7]");
    let d0 = &m.mappings[0];
    assert_eq!(d0.mapping_id, HashId(0x9790_5153));
    assert_eq!(d0.group_id, HashId(0xC109_19BC));
    assert_eq!(d0.sub_id, HashId(0x949E_EA04));
    assert_eq!(d0.flag, 0);
}

#[test]
fn real_explanation_text_0() {
    let Some(cfg) = load_real() else { return };
    // Clé = SETTING_INFO_0.help_text_id (0x6299E064), ref [0,1].
    let e = &cfg.explanation_texts[0];
    assert_eq!(e.key, HashId(0x6299_E064));
    assert_eq!(e.lines, vec![HashId(0x3F96_387B)]);
    // Recoupement : la clé est bien le help_text_id du premier réglage.
    assert_eq!(e.key, cfg.settings[0].help_text_id);
}

#[test]
fn real_pad_groupkey_refs() {
    let Some(cfg) = load_real() else { return };
    let p0 = &cfg.pad_groupkeys[0];
    assert_eq!(p0.key, HashId(0xD3D9_9E8B));
    assert_eq!(p0.keys, vec![12, 15]);
    let p1 = &cfg.pad_groupkeys[1];
    assert_eq!(p1.key, HashId(0x4AD0_CF31));
    assert_eq!(p1.keys, vec![10, 11, 8, 9]);
    let p10 = &cfg.pad_groupkeys[10];
    assert_eq!(p10.key, HashId(0x330C_7795));
    assert_eq!(p10.keys, vec![10, 11, 8, 9, 3, 0]);
}

#[test]
fn real_platform_of_recoupement() {
    let Some(cfg) = load_real() else { return };
    // Tout platform_type_id d'un réglage doit se résoudre vers un type existant.
    for s in &cfg.settings {
        assert!(
            cfg.platform_of(s).is_some(),
            "platform_type_id {:?} introuvable",
            s.platform_type_id
        );
    }
}
