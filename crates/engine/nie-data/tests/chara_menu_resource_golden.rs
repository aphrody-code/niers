#![allow(clippy::pedantic)]
//! Tests golden `chara_menu_resource` — noeuds réels tirés du VFS :
//! `data/common/gamedata/menu/chara_menu_resource_0.00.00.cfg.bin` (fichier unique, format T2B).
//!
//! Extraits via `nie-game --example extract_chara_menu_resource` (jetable, supprimé après) :
//! 1 `BASE_PATH` (`#/menu/`) + 92 `INFO` (template `INFO_0` + 91 overrides). La fixture embarque
//! les variables BRUTES (type+value) de `INFO_0` (template), `INFO_1`, `INFO_2` et `INFO_91`
//! (cas « chaînes en fin de bloc »). Vérité terrain = la sortie d'inagle
//! `packages/inagle/src/parsers/chara-menu-resource-config.ts` (`parseContent` l.100-160),
//! recalculée à l'identique en Python lors de l'extraction.

use nie_data::chara_menu_resource::{CharaResourcePaths, parse_chara_menu_resource};
use nie_data::hash::HashId;
use serde_json::{Value, json};

/// Construit un noeud iecode `{name, variables, children}` depuis `(type, value)`.
fn node(name: &str, vars: &[(&str, &str)], children: Vec<Value>) -> Value {
    let variables: Vec<Value> = vars
        .iter()
        .map(|(t, v)| json!({ "type": t, "value": v }))
        .collect();
    json!({ "name": name, "variables": variables, "children": children })
}

/// Variables BRUTES de `CHARA_MENU_RESOURCE_INFO_0` (template, 18 vars, dump réel).
const INFO_0: &[(&str, &str)] = &[
    ("Int", "-480378657"),
    ("String", "200_icon/10_icon_chr/<charaID>.g4tx"),
    ("String", "200_icon/10_icon_chr/<charaID>_l.g4tx"),
    ("String", "200_icon/16_icon_minichr/<charaID>_dot.g4tx"),
    ("Int", "0"),
    ("String", "200_icon/25_icon_win06_chr/icon_<charaID>.g4tx"),
    ("String", "200_icon/10_icon_chr/coach/<charaID>.g4tx"),
    ("String", "200_icon/10_icon_chr/coach/<charaID>_l.g4tx"),
    ("String", "200_icon/10_icon_chr/aura_fs/<charaID>.g4tx"),
    ("String", "200_icon/10_icon_chr/aura_fs/<charaID>_l.g4tx"),
    ("String", "200_icon/10_icon_chr/aura_armed/<charaID>.g4tx"),
    ("String", "200_icon/10_icon_chr/aura_armed/<charaID>_l.g4tx"),
    ("String", "200_icon/10_icon_chr/aura_mixi/<charaID>.g4tx"),
    ("String", "200_icon/10_icon_chr/aura_mixi/<charaID>_l.g4tx"),
    ("String", "200_icon/10_icon_chr/aura_soul/<charaID>.g4tx"),
    ("String", "200_icon/10_icon_chr/aura_soul/<charaID>_l.g4tx"),
    ("String", "220_img/soul_effect/ef_<skillID>.g4tx"),
    ("Int", "556686433"),
];

/// Variables BRUTES de `CHARA_MENU_RESOURCE_INFO_1` (override, dump réel).
const INFO_1: &[(&str, &str)] = &[
    ("Int", "1751902334"),
    ("String", "200_icon/10_icon_chr/c090101.g4tx"),
    ("String", "200_icon/10_icon_chr/c090101_l.g4tx"),
    ("Int", "0"),
    ("Int", "0"),
    ("String", "200_icon/10_icon_chr/c090101.g4tx"),
    ("String", "200_icon/10_icon_chr/coach/coach01.g4tx"),
    ("String", "200_icon/10_icon_chr/coach/coach01_l.g4tx"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
];

/// Variables BRUTES de `CHARA_MENU_RESOURCE_INFO_2` (override, dump réel).
const INFO_2: &[(&str, &str)] = &[
    ("Int", "527227112"),
    ("String", "200_icon/10_icon_chr/c090101.g4tx"),
    ("String", "200_icon/10_icon_chr/c090101_l.g4tx"),
    ("Int", "0"),
    ("Int", "0"),
    ("String", "200_icon/10_icon_chr/c090101.g4tx"),
    ("String", "200_icon/10_icon_chr/coach/coach01.g4tx"),
    ("String", "200_icon/10_icon_chr/coach/coach01_l.g4tx"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
];

/// Variables BRUTES de `CHARA_MENU_RESOURCE_INFO_91` (override, dump réel) — cas « chaînes en
/// fin de bloc » : les 3 String aux positions 14/15/16 sont tassées vers face/face_l/mini_chr.
const INFO_91: &[(&str, &str)] = &[
    ("Int", "1756911879"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("Int", "0"),
    ("String", "200_icon/10_icon_chr/aura_soul/a000560.g4tx"),
    ("String", "200_icon/10_icon_chr/aura_soul/a000560_l.g4tx"),
    ("String", "220_img/soul_effect/ef_wso000560.g4tx"),
    ("Int", "0"),
];

/// Reconstitue l'arbre iecode `{ entries: [...] }` du fichier (sous-ensemble représentatif).
fn fixture() -> Value {
    let base = node(
        "CHARA_MENU_RESOURCE_BASE_PATH_LIST_BEG_0",
        &[("Int", "1")],
        vec![node(
            "CHARA_MENU_RESOURCE_BASE_PATH_0",
            &[("String", "#/menu/")],
            vec![],
        )],
    );
    let info = node(
        "CHARA_MENU_RESOURCE_INFO_LIST_BEG_0",
        &[("Int", "1")],
        vec![
            node("CHARA_MENU_RESOURCE_INFO_0", INFO_0, vec![]),
            node("CHARA_MENU_RESOURCE_INFO_1", INFO_1, vec![]),
            node("CHARA_MENU_RESOURCE_INFO_2", INFO_2, vec![]),
            node("CHARA_MENU_RESOURCE_INFO_3", INFO_91, vec![]),
        ],
    );
    json!({ "entries": [base, info] })
}

#[test]
fn base_path_brut_non_nettoye() {
    let db = parse_chara_menu_resource(&fixture());
    // basePath = "#/menu/" tel quel (inagle ne lui applique pas cleanPath).
    assert_eq!(db.base_path, "#/menu/");
}

#[test]
fn template_info0_quinze_chemins_a_placeholders() {
    let db = parse_chara_menu_resource(&fixture());
    let t: &CharaResourcePaths = db.template.as_ref().expect("template présent (INFO_0)");

    // Les 15 champs nettoyés (.g4tx retiré), placeholders <charaID>/<skillID> préservés.
    assert_eq!(
        t.face_small.as_deref(),
        Some("200_icon/10_icon_chr/<charaID>")
    );
    assert_eq!(
        t.face_large.as_deref(),
        Some("200_icon/10_icon_chr/<charaID>_l")
    );
    assert_eq!(
        t.mini_chr.as_deref(),
        Some("200_icon/16_icon_minichr/<charaID>_dot")
    );
    assert_eq!(
        t.win_icon.as_deref(),
        Some("200_icon/25_icon_win06_chr/icon_<charaID>")
    );
    assert_eq!(
        t.coach_small.as_deref(),
        Some("200_icon/10_icon_chr/coach/<charaID>")
    );
    assert_eq!(
        t.coach_large.as_deref(),
        Some("200_icon/10_icon_chr/coach/<charaID>_l")
    );
    assert_eq!(
        t.aura_fs_small.as_deref(),
        Some("200_icon/10_icon_chr/aura_fs/<charaID>")
    );
    assert_eq!(
        t.aura_fs_large.as_deref(),
        Some("200_icon/10_icon_chr/aura_fs/<charaID>_l")
    );
    assert_eq!(
        t.aura_armed_small.as_deref(),
        Some("200_icon/10_icon_chr/aura_armed/<charaID>")
    );
    assert_eq!(
        t.aura_armed_large.as_deref(),
        Some("200_icon/10_icon_chr/aura_armed/<charaID>_l")
    );
    assert_eq!(
        t.aura_mixi_small.as_deref(),
        Some("200_icon/10_icon_chr/aura_mixi/<charaID>")
    );
    assert_eq!(
        t.aura_mixi_large.as_deref(),
        Some("200_icon/10_icon_chr/aura_mixi/<charaID>_l")
    );
    assert_eq!(
        t.aura_soul_small.as_deref(),
        Some("200_icon/10_icon_chr/aura_soul/<charaID>")
    );
    assert_eq!(
        t.aura_soul_large.as_deref(),
        Some("200_icon/10_icon_chr/aura_soul/<charaID>_l")
    );
    assert_eq!(
        t.soul_effect.as_deref(),
        Some("220_img/soul_effect/ef_<skillID>")
    );
    assert_eq!(t.filled_count(), 15);
}

#[test]
fn override_info1_id_et_cinq_chemins_tasses() {
    let db = parse_chara_menu_resource(&fixture());
    // INFO_0 = template (pas un override) ; donc overrides[0] = INFO_1.
    let o = &db.overrides[0];
    // resourceId = toHex(1751902334) (premier var Int) = 0x686BE87E.
    assert_eq!(o.resource_id, HashId::from_signed(1751902334));
    assert_eq!(o.resource_id.to_hex(), "0x686BE87E");
    assert!(!o.is_template, "pas de placeholder dans un override");

    // extractPaths tasse les 5 String (les Int 0 sont sautés) vers les 5 premiers champs.
    assert_eq!(
        o.paths.face_small.as_deref(),
        Some("200_icon/10_icon_chr/c090101")
    );
    assert_eq!(
        o.paths.face_large.as_deref(),
        Some("200_icon/10_icon_chr/c090101_l")
    );
    assert_eq!(
        o.paths.mini_chr.as_deref(),
        Some("200_icon/10_icon_chr/c090101")
    );
    assert_eq!(
        o.paths.win_icon.as_deref(),
        Some("200_icon/10_icon_chr/coach/coach01")
    );
    assert_eq!(
        o.paths.coach_small.as_deref(),
        Some("200_icon/10_icon_chr/coach/coach01_l")
    );
    assert_eq!(o.paths.coach_large, None);
    assert_eq!(o.paths.filled_count(), 5);
}

#[test]
fn override_info2_id_correct() {
    let db = parse_chara_menu_resource(&fixture());
    let o = &db.overrides[1];
    // resourceId = toHex(527227112) = 0x1F6CD8E8.
    assert_eq!(o.resource_id, HashId::from_signed(527227112));
    assert_eq!(o.resource_id.to_hex(), "0x1F6CD8E8");
}

#[test]
fn override_info91_chaines_de_fin_tassees_en_tete() {
    let db = parse_chara_menu_resource(&fixture());
    // 4e enfant = INFO_91 → overrides[2] (après les 2 vrais overrides INFO_1/INFO_2).
    let o = &db.overrides[2];
    // resourceId = toHex(1756911879) = 0x68B85907.
    assert_eq!(o.resource_id, HashId::from_signed(1756911879));
    assert_eq!(o.resource_id.to_hex(), "0x68B85907");

    // Quirk 1:1 inagle : les 3 chaînes (positions 14/15/16) sont TASSÉES en tête.
    assert_eq!(
        o.paths.face_small.as_deref(),
        Some("200_icon/10_icon_chr/aura_soul/a000560")
    );
    assert_eq!(
        o.paths.face_large.as_deref(),
        Some("200_icon/10_icon_chr/aura_soul/a000560_l")
    );
    assert_eq!(
        o.paths.mini_chr.as_deref(),
        Some("220_img/soul_effect/ef_wso000560")
    );
    assert_eq!(o.paths.win_icon, None);
    assert_eq!(o.paths.filled_count(), 3);
}

#[test]
fn comptage_template_et_overrides() {
    let db = parse_chara_menu_resource(&fixture());
    assert!(db.template.is_some());
    // 3 overrides dans la fixture (INFO_1, INFO_2, INFO_91) ; INFO_0 est le template.
    assert_eq!(db.overrides.len(), 3);
    // Lookup par id (équivalent byId d'inagle).
    let found = db.get(HashId::from_signed(527227112));
    assert!(found.is_some());
    assert_eq!(found.unwrap().resource_id.to_hex(), "0x1F6CD8E8");
}
