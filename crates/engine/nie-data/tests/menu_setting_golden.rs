#![allow(clippy::pedantic)]
//! Tests golden `menu_setting` — écran réel tiré du VFS IEVR :
//! `data/common/gamedata/menu/cfg/main_menu_setting.cfg.bin` (T2B `entries`).
//!
//! RE originale niers (aucun parseur inagle). Les 13 `MENU_LAYER_INFO` + 3 `MENU_RES`
//! ci-dessous sont extraits TELS QUELS du dump (probe `parse_t2b`), aucune valeur inventée.
//! **Validation forte end-to-end** : pour chaque layer, `layer_id == CRC32(name)` — l'identifiant
//! EST le CRC32 du nom (poly 0xEDB88320), ce qui prouve l'interprétation positionnelle des champs.

mod common;

use nie_data::menu_setting::{MenuSetting, parse};
use serde_json::{Value, json};

/// CRC32 IEEE (poly réfléchi 0xEDB88320) — même algo que `binascii.crc32` / `cfgbin::crc32`.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Construit un noeud iecode `MENU_LAYER_INFO_<idx>` (forme T2B `{name, variables, children}`).
fn layer(idx: usize, id: i64, name: &str, objbin: &str, params: &[i64]) -> Value {
    let mut variables = vec![
        json!({ "type": "Int", "value": id.to_string() }),
        json!({ "type": "String", "value": name }),
        json!({ "type": "String", "value": objbin }),
    ];
    for p in params {
        variables.push(json!({ "type": "Int", "value": p.to_string() }));
    }
    json!({ "name": format!("MENU_LAYER_INFO_{idx}"), "variables": variables, "children": [] })
}

fn res(idx: usize, path: &str, kind: i64) -> Value {
    json!({
        "name": format!("MENU_RES_{idx}"),
        "variables": [
            json!({ "type": "String", "value": path }),
            json!({ "type": "Int", "value": kind.to_string() }),
        ],
        "children": [],
    })
}

/// Construit un noeud `MENU_CMD_INFO_<idx>` : layer_id, command_hash, nom, puis args (hashes/Int).
fn cmd(idx: usize, layer_id: i64, command_hash: i64, name: &str, args: &[i64]) -> Value {
    let mut variables = vec![
        json!({ "type": "Int", "value": layer_id.to_string() }),
        json!({ "type": "Int", "value": command_hash.to_string() }),
        json!({ "type": "String", "value": name }),
    ];
    for a in args {
        variables.push(json!({ "type": "Int", "value": a.to_string() }));
    }
    json!({ "name": format!("MENU_CMD_INFO_{idx}"), "variables": variables, "children": [] })
}

/// Les 13 layers réels de `main_menu_setting.cfg.bin` (ordre du fichier) :
/// (layer_id décimal signé, nom, basename d'objbin, params var\[3..\]). Le chemin d'objbin
/// complet du dump = `common/gamedata/menu/obj/<basename>`.
fn main_menu_layers() -> [(i64, &'static str, &'static str, [i64; 7]); 13] {
    [
        (
            367379312,
            "mainmenu90_00_background",
            "mainmenu90_00_background.objbin",
            [1, 0, 0, 0, 0, 1, 1],
        ),
        (
            -710450727,
            "mainmenu90_02_header_tab",
            "mainmenu90_02_header_tab.objbin",
            [1, 2, 4, 0, 0, 1, 1],
        ),
        (
            200360024,
            "mainmenu90_02_2_header_tab_icon",
            "mainmenu90_02_2_header_tab_icon.objbin",
            [9, 2, 4, 0, 0, 1, 1],
        ),
        (
            -1789734899,
            "cmn01_12_new_icon",
            "cmn01_12_new_icon.objbin",
            [9, 2, 4, 0, 0, 1, 1],
        ),
        (
            1561203443,
            "cmn01_13_new_icon_red",
            "cmn01_13_new_icon_middle.objbin",
            [9, 2, 4, 0, 0, 1, 1],
        ),
        (
            575058101,
            "mainmenu90_01_header",
            "mainmenu90_01_header.objbin",
            [1, 2, 4, 0, 0, 1, 1],
        ),
        (
            -2015585191,
            "mainmenu90_31_doc_item",
            "mainmenu90_31_doc_item.objbin",
            [1, 2, 4, 0, 0, 1, 1],
        ),
        (
            1209571854,
            "rpg00_07_weekday_timezone_guide",
            "rpg00_07_weekday_timezone_guide.objbin",
            [1, 0, 0, 0, 0, 1, 1],
        ),
        (
            1138543975,
            "mainmenu01_06_base_button_guide",
            "mainmenu01_06_base_button_guide.objbin",
            [1, 2, 12, 0, 0, 1, 1],
        ),
        (
            9396745,
            "mainmenu01_07_button_guide",
            "mainmenu01_07_button_guide.objbin",
            [10, 2, 12, 0, 0, 1, 1],
        ),
        (
            2005383485,
            "mainmenu01_10_return_arrow_button_guide",
            "mainmenu01_10_return_arrow_button_guide.objbin",
            [1, 1, 12, 0, 0, 1, 1],
        ),
        (
            -805989921,
            "mainmenu01_11_save_button_guide",
            "mainmenu01_11_save_button_guide.objbin",
            [1, 1, 12, 0, 0, 1, 1],
        ),
        (
            -1212813856,
            "cmn01_40_list_base_empty",
            "cmn01_40_list_base_empty.objbin",
            [1, 0, 0, 0, 0, 1, 1],
        ),
    ]
}

fn build_main_menu_setting() -> Value {
    const OBJ: &str = "common/gamedata/menu/obj/";
    let layers: Vec<Value> = main_menu_layers()
        .iter()
        .enumerate()
        .map(|(i, (id, name, obj, params))| layer(i, *id, name, &format!("{OBJ}{obj}"), params))
        .collect();
    let resources = vec![
        res(0, "#/menu/200_icon/100_num/num_menu01.g4tx", 0),
        res(1, "#/menu/200_icon/100_num/<LG>/num_menu01.g4tx", 0),
        res(2, "#/menu/200_icon/15_icon_common/<LG>/icon_common.g4tx", 0),
    ];
    // 4 commandes réelles de `main_menu_setting` (toutes liées au layer 0xBF14058 = header_tab_icon).
    let commands = vec![
        cmd(
            0,
            200360024,
            1970176280,
            "CMD_FCS_BACK",
            &[-186917087, -186917087, 1],
        ),
        cmd(
            1,
            200360024,
            -1590706489,
            "CMD_FCS_NEXT",
            &[-186917087, -186917087, 1],
        ),
        cmd(
            2,
            200360024,
            -1283855071,
            "CMD_FUNCTION",
            &[-825442646, -186917087],
        ),
        cmd(
            3,
            200360024,
            -821903626,
            "CMD_FUNCTION",
            &[-825442646, -186917087, 1],
        ),
    ];
    // 13 `MENU_LAYER_GROUP_BASE` réels : flags=[flag0,1,0,0,1,0], flag0=1 UNIQUEMENT pour le layer
    // interactif 0xBF14058 (200360024 = header_tab_icon, celui qui porte les commandes).
    let layer_groups: Vec<Value> = main_menu_layers()
        .iter()
        .enumerate()
        .map(|(i, (id, _, _, _))| {
            let flag0 = i64::from(*id == 200360024);
            let vars: Vec<Value> = [flag0, 1, 0, 0, 1, 0]
                .iter()
                .map(|v| json!({ "type": "Int", "value": v.to_string() }))
                .collect();
            // var[0] = layer_id devant les flags.
            let mut variables = vec![json!({ "type": "Int", "value": id.to_string() })];
            variables.extend(vars);
            json!({ "name": format!("MENU_LAYER_GROUP_BASE_{i}"), "variables": variables, "children": [] })
        })
        .collect();
    json!({
        "entries": [
            { "name": "MENU_LAYER_INFO_LIST_BEG_0", "variables": [{ "type": "Int", "value": "13" }], "children": layers },
            { "name": "MENU_CMD_INFO_LIST_BEG_0", "variables": [{ "type": "Int", "value": "4" }], "children": commands },
            { "name": "MENU_LAYER_GROUP_BASE_LIST_BEG_0", "variables": [{ "type": "Int", "value": "13" }], "children": layer_groups },
            { "name": "MENU_RES_LIST_BEG_0", "variables": [{ "type": "Int", "value": "3" }], "children": resources },
        ]
    })
}

#[test]
fn main_menu_setting_parses_13_layers() {
    let ms: MenuSetting = parse(&build_main_menu_setting());
    assert_eq!(ms.layers.len(), 13, "13 MENU_LAYER_INFO attendus");
    assert_eq!(ms.resources.len(), 3, "3 MENU_RES attendus");

    // Layer 0 = fond ; layer 8 = button-guide mainmenu01_06.
    assert_eq!(ms.layers[0].name, "mainmenu90_00_background");
    assert_eq!(
        ms.layers[0].objbin_path,
        "common/gamedata/menu/obj/mainmenu90_00_background.objbin"
    );
    assert_eq!(ms.layers[8].name, "mainmenu01_06_base_button_guide");
    assert_eq!(ms.layers[0].params, vec![1, 0, 0, 0, 0, 1, 1]);

    // Ressources g4tx partagées.
    assert_eq!(
        ms.resources[0].logical_path,
        "#/menu/200_icon/100_num/num_menu01.g4tx"
    );
}

#[test]
fn main_menu_setting_parses_commands() {
    let ms = parse(&build_main_menu_setting());
    assert_eq!(ms.commands.len(), 4, "4 MENU_CMD_INFO attendus");
    assert_eq!(ms.commands[0].name, "CMD_FCS_BACK");
    assert_eq!(ms.commands[1].name, "CMD_FCS_NEXT");
    assert_eq!(ms.commands[2].name, "CMD_FUNCTION");
    assert_eq!(ms.commands[3].name, "CMD_FUNCTION");
    // Toutes les commandes sont liées au layer header_tab_icon (0xBF14058).
    assert!(ms.commands.iter().all(|c| c.layer_id.0 == 0x0BF1_4058));
    // command_hash distinct par commande (identité).
    assert_eq!(ms.commands[0].command_hash.0, 0x756E_8118);
    assert_eq!(ms.commands[1].command_hash.0, 0xA12F_BEC7);
    // args préservés (hashes de focus/handler).
    assert_eq!(ms.commands[2].args[0].0, 0xCECC_BEAA);
}

#[test]
fn layer_groups_mark_interactive_layer() {
    let ms = parse(&build_main_menu_setting());
    assert_eq!(
        ms.layer_groups.len(),
        13,
        "13 MENU_LAYER_GROUP_BASE attendus"
    );
    // Le SEUL layer interactif (flag0=1) = header_tab_icon (0xBF14058), qui porte les commandes.
    assert_eq!(ms.interactive_layer_id().map(|h| h.0), Some(0x0BF1_4058));
    let interactive = ms
        .layer_groups
        .iter()
        .filter(|g| g.flags.first() == Some(&1))
        .count();
    assert_eq!(interactive, 1, "exactement 1 layer interactif");
    // Le layer interactif est bien celui des commandes (cohérence cross-liste).
    assert!(
        ms.commands
            .iter()
            .all(|c| Some(c.layer_id) == ms.interactive_layer_id())
    );
}

#[test]
fn layer_id_equals_crc32_of_name() {
    // Validation forte : l'identifiant de layer EST le CRC32 du nom (preuve du mapping de champs).
    let ms = parse(&build_main_menu_setting());
    for l in &ms.layers {
        assert_eq!(
            l.layer_id.0,
            crc32(l.name.as_bytes()),
            "layer_id {:#010X} != CRC32(\"{}\") {:#010X}",
            l.layer_id.0,
            l.name,
            crc32(l.name.as_bytes())
        );
    }
}

#[test]
fn composition_spans_multiple_objbin_prefixes() {
    // Découverte structurante : un écran compose des layers de PLUSIEURS préfixes d'objbin,
    // pas seulement `mainmenu01_*` (ce que suppose le filtre par nom de `build_sprite_list`).
    let ms = parse(&build_main_menu_setting());
    let has = |pre: &str| ms.layers.iter().any(|l| l.name.starts_with(pre));
    assert!(has("mainmenu90_"), "fond/en-tête mainmenu90_*");
    assert!(has("mainmenu01_"), "button-guides mainmenu01_*");
    assert!(has("cmn01_"), "icônes communes cmn01_*");
    assert!(has("rpg00_"), "guide rpg00_*");

    // Lookup helpers.
    let bg = ms
        .layer_by_name("mainmenu90_00_background")
        .expect("layer fond");
    assert_eq!(bg.layer_id, ms.layer_by_id(bg.layer_id).unwrap().layer_id);
}

/// Golden EXHAUSTIF data-gated : itère TOUS les `*_setting.cfg.bin.json` réels (440 = 304
/// `*_menu_setting` + fenêtres/sélecteurs) et prouve l'invariant byte-exact `layer_id ==
/// CRC32(name)` sur CHAQUE layer de CHAQUE écran (pas seulement `main_menu`). Vérifie aussi la
/// **nav-hash d'écran** = `CRC32(stem du fichier)` sur deux ancres connues. Skip silencieux si le
/// dump n'est pas présent (cf. mémoire golden gated).
#[test]
fn all_menu_settings_layer_hashes_consistent() {
    // Résolution par `common::chemin`, pas par `CARGO_MANIFEST_DIR` : le crate étant sous
    // `crates/engine/nie-data`, un `../../` ne remonte qu'à `crates/` et ce test ne trouvait
    // jamais le corpus, quel que soit l'état de la machine.
    let Some(dir) = common::chemin("menu/cfg") else {
        return;
    };
    if !dir.is_dir() {
        eprintln!(
            "skip all_menu_settings : dump menu absent ({})",
            dir.display()
        );
        return;
    }
    let mut screens = 0usize;
    let mut layers = 0usize;
    let mut anchors = std::collections::HashMap::new();
    for entry in std::fs::read_dir(&dir).expect("read_dir menu/cfg") {
        let path = entry.expect("entry").path();
        let Some(fname) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(stem) = fname.strip_suffix("_setting.cfg.bin.json") else {
            continue;
        };
        let txt = std::fs::read_to_string(&path).expect("read json");
        let root: Value = serde_json::from_str(&txt).expect("json valide");
        let ms = parse(&root);
        assert!(
            ms.layer_hashes_consistent(),
            "{stem} : un layer_id ≠ CRC32(name) — interprétation positionnelle cassée",
        );
        screens += 1;
        layers += ms.layers.len();
        // Nav-hash d'écran = CRC32(stem) (ce que le manager/Lua utilise pour ouvrir l'écran).
        anchors.insert(stem.to_string(), crc32(stem.as_bytes()));
    }
    assert!(screens >= 200, "trop peu d'écrans extraits : {screens}");
    assert!(layers >= 2000, "trop peu de layers : {layers}");
    // Ancres byte-exact (recoupées sur le dico CRC32 du jeu).
    assert_eq!(
        anchors.get("main_menu"),
        Some(&0x9DB6_08F1),
        "nav-hash main_menu"
    );
    assert_eq!(
        anchors.get("soccer_top_menu"),
        Some(&0x305E_72CF),
        "nav-hash soccer_top_menu"
    );
    eprintln!("OK {screens} écrans / {layers} layers — tous layer_id == CRC32(name)");
}
