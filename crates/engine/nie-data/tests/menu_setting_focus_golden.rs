#![allow(clippy::pedantic)]
//! Tests golden `menu_setting` — **navigation** de l'écran réel `main_menu` et **équivalence
//! des deux formes JSON** de variable CfgBin.
//!
//! Vérité terrain : `data/common/gamedata/menu/cfg/main_menu_setting.cfg.bin` (VFS IEVR, 3792
//! octets, T2B, 7 listes racines), décodé par `niers decode`. C'est l'écran que montre une
//! capture du menu principal : bandeau AVATAR / VOTRE ÉQUIPE, rangée de tuiles de mode
//! (`mode_base01_atl` de `mainmenu90_01.g4tx`), rangée secondaire, guides de boutons.
//!
//! Ce fichier couvre ce que `menu_setting_golden.rs` laissait de côté — les listes de
//! **groupes** et de **focus**, c.-à-d. la navigation :
//!
//! | liste | rôle | présence corpus (304 écrans) |
//! |-------|------|------------------------------|
//! | `MENU_LAYER_GROUP` (+ `_REF_`) | l'écran nommé + sa plage d'états de layer | 304 |
//! | `MENU_FOCUS_BASE_INFO`         | les unités focusables                     | 203 |
//! | `MENU_FOCUS_GROUP` (+ `_REF_`) | le layer porteur du focus + sa plage      | 207 |
//! | `MENU_FOCUS_SHIFT*`            | règles de déplacement                     | 31  |
//!
//! Aucune valeur inventée : identifiants, drapeaux et chemins sont ceux du dump. Les
//! `layer_id` sont posés comme `CRC32(nom)` — l'invariant vérifié sur **3307/3307** layers du
//! corpus, ce qui rend le test lisible sans le rendre moins fidèle.

mod common;

use nie_data::hash::HashId;
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

/// Identifiant de layer tel qu'il figure dans le fichier : CRC32 du nom, vu comme `i32` signé.
fn id_of(name: &str) -> i64 {
    i64::from(crc32(name.as_bytes()) as i32)
}

/// Les 13 layers de `main_menu`, dans l'ordre du fichier, avec leurs `params` réels.
/// Trois préfixes d'objbin cohabitent (`mainmenu90_*`, `cmn01_*`, `mainmenu01_*`, `rpg00_*`) :
/// un écran n'est PAS mono-préfixe.
const LAYERS: &[(&str, [i64; 7])] = &[
    ("mainmenu90_00_background", [1, 0, 0, 0, 0, 1, 1]),
    ("mainmenu90_02_header_tab", [1, 2, 4, 0, 0, 1, 1]),
    ("mainmenu90_02_2_header_tab_icon", [9, 2, 4, 0, 0, 1, 1]),
    ("cmn01_12_new_icon", [9, 2, 4, 0, 0, 1, 1]),
    ("cmn01_13_new_icon_red", [9, 2, 4, 0, 0, 1, 1]),
    ("mainmenu90_01_header", [1, 2, 4, 0, 0, 1, 1]),
    ("mainmenu90_31_doc_item", [1, 2, 4, 0, 0, 1, 1]),
    ("rpg00_07_weekday_timezone_guide", [1, 0, 0, 0, 0, 1, 1]),
    ("mainmenu01_06_base_button_guide", [1, 2, 12, 0, 0, 1, 1]),
    ("mainmenu01_07_button_guide", [10, 2, 12, 0, 0, 1, 1]),
    (
        "mainmenu01_10_return_arrow_button_guide",
        [1, 1, 12, 0, 0, 1, 1],
    ),
    ("mainmenu01_11_save_button_guide", [1, 1, 12, 0, 0, 1, 1]),
    ("cmn01_40_list_base_empty", [1, 0, 0, 0, 0, 1, 1]),
];

/// Le layer INTERACTIF de `main_menu` : celui qui porte les 4 commandes, que le groupe de focus
/// désigne, et que `MENU_LAYER_GROUP_BASE` marque `flag0 == 1`. Triple concordance.
const INTERACTIVE: &str = "mainmenu90_02_2_header_tab_icon";

/// Hash de rôle porté par les 9 `MENU_FOCUS_BASE_INFO` de l'écran (13 valeurs distinctes en
/// tout dans le corpus → un rôle, pas une identité par bouton).
const FOCUS_ROLE: i64 = 973_515_837;

/// Construit une variable en forme **native** (`nie_formats`, sérialisation serde à tag externe).
fn nat_i(v: i64) -> Value {
    json!({ "Int": v })
}
fn nat_s(v: &str) -> Value {
    json!({ "String": v })
}

/// Construit une variable en forme **iecode** (`{type, value}`, valeur toujours en chaîne).
fn iec_i(v: i64) -> Value {
    json!({ "type": "Int", "value": v.to_string() })
}
fn iec_s(v: &str) -> Value {
    json!({ "type": "String", "value": v })
}

/// Construit l'arbre complet de `main_menu_setting.cfg.bin`.
///
/// `native` choisit la forme des variables ; la structure des nœuds est identique dans les deux
/// cas, ce qui est exactement le point testé par [`les_deux_formes_donnent_le_meme_ecran`].
fn main_menu_tree(native: bool) -> Value {
    let vi = |v: i64| if native { nat_i(v) } else { iec_i(v) };
    let vs = |v: &str| if native { nat_s(v) } else { iec_s(v) };

    let layer_nodes: Vec<Value> = LAYERS
        .iter()
        .enumerate()
        .map(|(i, (name, params))| {
            let mut variables = vec![
                vi(id_of(name)),
                vs(name),
                vs(&format!("common/gamedata/menu/obj/{name}.objbin")),
            ];
            variables.extend(params.iter().map(|p| vi(*p)));
            json!({ "name": format!("MENU_LAYER_INFO_{i}"), "variables": variables, "children": [] })
        })
        .collect();

    // Les 4 commandes, toutes portées par le layer interactif.
    let cmd = |i: usize, hash: i64, name: &str, args: &[i64]| {
        let mut variables = vec![vi(id_of(INTERACTIVE)), vi(hash), vs(name)];
        variables.extend(args.iter().map(|a| vi(*a)));
        json!({ "name": format!("MENU_CMD_INFO_{i}"), "variables": variables, "children": [] })
    };
    // Hashes tels qu'ils figurent dans le fichier, en `i32` signé : `0x756E8118` tient dans un
    // positif, les autres débordent et s'écrivent donc en complément à deux.
    let cmd_nodes = vec![
        cmd(
            0,
            0x756E_8118i64,
            "CMD_FCS_BACK",
            &[-0x0B24_20DFi64, -0x0B24_20DFi64, 1],
        ),
        cmd(
            1,
            -0x5ED0_4139i64,
            "CMD_FCS_NEXT",
            &[-0x0B24_20DFi64, -0x0B24_20DFi64, 1],
        ),
        cmd(
            2,
            -0x4C86_12DFi64,
            "CMD_FUNCTION",
            &[-0x3133_4156i64, -0x0B24_20DFi64],
        ),
        cmd(
            3,
            -0x30FD_410Ai64,
            "CMD_FUNCTION",
            &[-0x3133_4156i64, -0x0B24_20DFi64, 1],
        ),
    ];

    let res = |i: usize, path: &str| {
        json!({
            "name": format!("MENU_RES_{i}"),
            "variables": [vs(path), vi(0)],
            "children": [],
        })
    };
    let res_nodes = vec![
        res(0, "#/menu/200_icon/100_num/num_menu01.g4tx"),
        res(1, "#/menu/200_icon/100_num/<LG>/num_menu01.g4tx"),
        res(2, "#/menu/200_icon/15_icon_common/<LG>/icon_common.g4tx"),
    ];

    // Un état par layer : seul le layer interactif porte `flag0 == 1`.
    let group_base_nodes: Vec<Value> = LAYERS
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            let f0 = i64::from(*name == INTERACTIVE);
            json!({
                "name": format!("MENU_LAYER_GROUP_BASE_{i}"),
                "variables": [vi(id_of(name)), vi(f0), vi(1), vi(0), vi(0), vi(1), vi(0)],
                "children": [],
            })
        })
        .collect();

    let focus_base_nodes: Vec<Value> = (0..9)
        .map(|i| {
            json!({
                "name": format!("MENU_FOCUS_BASE_INFO_{i}"),
                "variables": [vi(FOCUS_ROLE), vi(0), vi(0)],
                "children": [],
            })
        })
        .collect();

    json!({
        "format": "T2b",
        "entries": [
            { "name": "MENU_LAYER_INFO_LIST_BEG", "variables": [vi(13)], "children": layer_nodes },
            { "name": "MENU_CMD_INFO_LIST_BEG", "variables": [vi(4)], "children": cmd_nodes },
            { "name": "MENU_RES_LIST_BEG", "variables": [vi(3)], "children": res_nodes },
            { "name": "MENU_LAYER_GROUP_BASE_LIST_BEG", "variables": [vi(13)], "children": group_base_nodes },
            { "name": "MENU_LAYER_GROUP_LIST_BEG", "variables": [vi(1)], "children": [
                { "name": "MENU_LAYER_GROUP", "variables": [vi(id_of("main_menu")), vs("main_menu"), vi(1)], "children": [] },
                { "name": "MENU_LAYER_GROUP_REF_LAYER_GROUP_BASE", "variables": [vi(0), vi(13)], "children": [] },
            ]},
            { "name": "MENU_FOCUS_BASE_INFO_LIST_BEG", "variables": [vi(9)], "children": focus_base_nodes },
            { "name": "MENU_FOCUS_GROUP_LIST_BEG", "variables": [vi(1)], "children": [
                { "name": "MENU_FOCUS_GROUP", "variables": [vi(id_of(INTERACTIVE)), vi(0), vi(0), vi(0)], "children": [] },
                { "name": "MENU_FOCUS_GROUP_REF_FOCUS_BASE_INFO", "variables": [vi(0), vi(9)], "children": [] },
            ]},
        ]
    })
}

fn parsed(native: bool) -> MenuSetting {
    parse(&main_menu_tree(native))
}

#[test]
fn les_quatre_listes_historiques_restent_lues() {
    let m = parsed(true);
    assert_eq!(m.layers.len(), 13, "13 MENU_LAYER_INFO");
    assert_eq!(m.commands.len(), 4, "4 MENU_CMD_INFO");
    assert_eq!(m.resources.len(), 3, "3 MENU_RES");
    assert_eq!(m.layer_groups.len(), 13, "13 MENU_LAYER_GROUP_BASE");
    // Le `_REF_` ne doit PAS être compté comme un état de layer malgré le préfixe partagé.
    assert!(m.layer_groups.iter().all(|g| !g.layer_id.is_zero()));
}

#[test]
fn le_groupe_nomme_est_l_ecran_et_couvre_tous_ses_layers() {
    let m = parsed(true);
    assert_eq!(m.groups.len(), 1, "un seul MENU_LAYER_GROUP");
    assert_eq!(m.groups[0].name, "main_menu");
    assert_eq!(m.groups[0].group_id, HashId(crc32(b"main_menu")));
    assert!(m.group_hashes_consistent(), "group_id == CRC32(name)");

    assert_eq!(m.group_refs.len(), 1);
    assert_eq!(m.group_refs[0].start, 0);
    assert_eq!(m.group_refs[0].count, 13, "la plage couvre les 13 états");
    assert_eq!(m.group_layer_states(0).map(<[_]>::len), Some(13));
}

#[test]
fn les_neuf_focus_sont_rattaches_au_layer_interactif() {
    let m = parsed(true);
    assert_eq!(m.focus_count(), 9, "9 MENU_FOCUS_BASE_INFO");
    assert!(
        m.focus_base_infos
            .iter()
            .all(|f| f.role == HashId(FOCUS_ROLE as u32))
    );
    assert!(
        m.focus_base_infos
            .iter()
            .all(|f| f.param == 0 && f.param2 == 0)
    );

    assert_eq!(m.focus_groups.len(), 1);
    let interactive = HashId(crc32(INTERACTIVE.as_bytes()));
    assert_eq!(m.focus_groups[0].layer_id, interactive);
    assert_eq!(m.focus_elements(0).map(<[_]>::len), Some(9));

    // Triple concordance : le layer visé par le focus est AUSSI le layer marqué interactif par
    // MENU_LAYER_GROUP_BASE, et AUSSI le porteur des 4 commandes.
    assert_eq!(m.interactive_layer_id(), Some(interactive));
    assert!(m.commands.iter().all(|c| c.layer_id == interactive));
    assert!(
        m.layer_by_id(interactive).is_some(),
        "et il est déclaré dans l'écran"
    );
}

#[test]
fn les_invariants_de_plage_tiennent() {
    let m = parsed(true);
    assert!(
        m.refs_consistent(),
        "plages non vides = partition contiguë exhaustive"
    );
    assert!(m.refs_pair_groups(), "autant de plages que de groupes");
    assert!(m.layer_hashes_consistent(), "layer_id == CRC32(name)");
}

#[test]
fn focus_shift_absent_de_cet_ecran_ne_casse_rien() {
    // `main_menu` ne porte AUCUNE liste FOCUS_SHIFT (elles n'existent que sur 31 écrans).
    let m = parsed(true);
    assert!(m.focus_shifts.is_empty());
    assert!(m.focus_shift_base_infos.is_empty());
    assert!(m.focus_shift_refs.is_empty());
    // Les invariants restent vrais par vacuité, ils ne doivent pas exiger la présence des listes.
    assert!(m.refs_consistent());
    assert!(m.refs_pair_groups());
}

#[test]
fn les_deux_formes_donnent_le_meme_ecran() {
    // Le cœur du correctif `cfgbin::Node::var` : `{"Int":13}` (natif, produit par `niers decode`)
    // et `{"type":"Int","value":"13"}` (iecode) doivent décrire le MÊME écran. Avant, la forme
    // native se lisait « toutes variables absentes » et le parseur renvoyait des listes vides
    // SANS erreur — un faux vert silencieux.
    let natif = parsed(true);
    let iecode = parsed(false);
    assert_eq!(
        natif, iecode,
        "les deux formes de variable doivent converger"
    );
    assert_eq!(
        natif.layers.len(),
        13,
        "et ce n'est pas une égalité de deux vides"
    );
    assert_eq!(natif.focus_count(), 9);
}

/// Golden EXHAUSTIF data-gated : rejoue les invariants de **groupe** et de **focus** sur tous
/// les `*_setting.cfg.bin.json` réels, pas seulement `main_menu`. Skip annoncé si le dump est
/// absent — un golden muet qui ne s'exécute pas est un faux vert.
#[test]
fn tous_les_ecrans_respectent_les_invariants_de_groupe_et_de_focus() {
    let Some(dir) = common::chemin("menu/cfg") else {
        return;
    };
    if !dir.is_dir() {
        eprintln!(
            "skip invariants groupe/focus : dump menu absent ({})",
            dir.display()
        );
        return;
    }

    let (mut ecrans, mut avec_focus, mut avec_shift, mut focus_total) =
        (0usize, 0usize, 0usize, 0usize);
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
            ms.refs_pair_groups(),
            "{stem} : autant de plages que de groupes"
        );
        assert!(
            ms.refs_consistent(),
            "{stem} : plages non vides = partition contiguë exhaustive"
        );
        assert!(
            ms.group_hashes_consistent(),
            "{stem} : group_id ≠ CRC32(name)"
        );

        // Toute plage doit désigner une tranche réelle : `None` ici = débordement silencieux.
        for i in 0..ms.focus_groups.len() {
            assert!(
                ms.focus_elements(i).is_some(),
                "{stem} : plage de focus {i} hors liste"
            );
        }
        for i in 0..ms.groups.len() {
            assert!(
                ms.group_layer_states(i).is_some(),
                "{stem} : plage de groupe {i} hors liste"
            );
        }

        ecrans += 1;
        focus_total += ms.focus_count();
        avec_focus += usize::from(!ms.focus_groups.is_empty());
        avec_shift += usize::from(!ms.focus_shifts.is_empty());
    }

    assert!(ecrans >= 200, "trop peu d'écrans : {ecrans}");
    assert!(
        avec_focus >= 100,
        "aucun groupe de focus lu — le parsing focus est muet"
    );
    assert!(
        focus_total >= 500,
        "trop peu d'éléments focusables : {focus_total}"
    );
    eprintln!(
        "OK {ecrans} écrans — {avec_focus} avec focus ({focus_total} éléments), {avec_shift} avec focus-shift",
    );
}

#[test]
fn la_forme_native_seule_ne_donne_pas_un_ecran_vide() {
    // Garde-fou dédié : si un jour `Node::var` reperd la forme native, ce test tombe seul et
    // nomme la cause, au lieu de laisser 300 écrans se parser en silence comme vides.
    let m = parsed(true);
    assert!(
        !m.layers.is_empty(),
        "forme native `{{\"Int\":n}}` non lue par cfgbin::Node::var"
    );
    assert_eq!(m.layers[0].name, "mainmenu90_00_background");
    assert_eq!(
        m.layers[0].objbin_path,
        "common/gamedata/menu/obj/mainmenu90_00_background.objbin"
    );
}
