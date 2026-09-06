#![allow(clippy::pedantic)]
//! Tests golden `telop_waza` — noeud réel tiré du VFS IEVR :
//! `data/common/gamedata/skill/skill_telop_info_config_0.00.00.cfg.bin`
//! (RDBN `lists`, 2 listes parallèles de 926 entrées).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/telop-waza.ts`
//! (`parseTelopWazaContent`, l.101-135) : `eldoradoId == 0x00000000` → `None`, index
//! `blankSizeInfo` hors borne → marges nulles. Valeurs ci-dessous extraites telles quelles
//! du dump (via `cfgbin::read_values` → forme iecode), aucune valeur inventée.

use nie_data::hash::HashId;
use nie_data::telop_waza::{
    LangMargins, ParsedTelopWaza, TELOP_LANGS, find_by_skill_id, parse_telop_waza_config,
};
use serde_json::{Value, json};

/// Construit une entrée `m_blankSizeInfoList` depuis 9 couples `(left, right)` donnés dans
/// l'ordre `TELOP_LANGS` (ja, en, pt, fr, it, de, es, zh_hant, zh_hans), extraits du dump.
fn blank(rows: [(i64, i64); 9]) -> Value {
    json!({
        "ja_left": rows[0].0, "ja_right": rows[0].1,
        "en_left": rows[1].0, "en_right": rows[1].1,
        "pt_left": rows[2].0, "pt_right": rows[2].1,
        "fr_left": rows[3].0, "fr_right": rows[3].1,
        "it_left": rows[4].0, "it_right": rows[4].1,
        "de_left": rows[5].0, "de_right": rows[5].1,
        "es_left": rows[6].0, "es_right": rows[6].1,
        "zh_hant_left": rows[7].0, "zh_hant_right": rows[7].1,
        "zh_hans_left": rows[8].0, "zh_hans_right": rows[8].1,
    })
}

/// Les 3 premières entrées réelles de `m_blankSizeInfoList` (couples `(left, right)`).
fn blank_head() -> Vec<Value> {
    vec![
        // blank[0]
        blank([
            (246, 246),
            (324, 327),
            (290, 291),
            (6, 28),
            (124, 132),
            (307, 306),
            (87, 86),
            (370, 376),
            (347, 356),
        ]),
        // blank[1]
        blank([
            (270, 275),
            (417, 418),
            (306, 306),
            (243, 245),
            (308, 308),
            (379, 382),
            (175, 176),
            (355, 360),
            (354, 358),
        ]),
        // blank[2]
        blank([
            (213, 218),
            (400, 401),
            (239, 239),
            (252, 252),
            (174, 172),
            (346, 345),
            (164, 161),
            (200, 208),
            (222, 228),
        ]),
    ]
}

/// Fixture principale : 3 entrées skill réelles (index 0,1,2) + 3 blancs réels (0,1,2).
/// Skill[i].blankSizeInfo = [i, 1] (gauche = blank[i], droite = blank[1]), eldoradoId = 0.
fn node_fixture() -> Value {
    let skills = vec![
        json!({ "id": "0x5B6A04E9", "blankSizeInfo": [0, 1], "eldoradoId": "0x00000000" }),
        json!({ "id": "0x7047572A", "blankSizeInfo": [1, 1], "eldoradoId": "0x00000000" }),
        json!({ "id": "0x695C666B", "blankSizeInfo": [2, 1], "eldoradoId": "0x00000000" }),
    ];
    json!({
        "lists": [
            { "name": "m_skillTelopInfoList", "typeName": "SkillTelopInfo", "values": skills },
            { "name": "m_blankSizeInfoList", "typeName": "BlankSizeInfo", "values": blank_head() },
        ]
    })
}

/// Vecteur des 9 marges dans l'ordre `TELOP_LANGS` (ja, en, pt, fr, it, de, es, zh_hant, zh_hans).
fn margins_vec(m: &LangMargins) -> Vec<i64> {
    TELOP_LANGS.iter().map(|l| m.get(l).unwrap()).collect()
}

#[test]
fn parse_compte_et_champs_de_base() {
    let telops = parse_telop_waza_config(&node_fixture());
    assert_eq!(telops.len(), 3, "3 entrées m_skillTelopInfoList");

    let t0: &ParsedTelopWaza = &telops[0];
    assert_eq!(t0.skill_id, HashId(0x5B6A_04E9));
    assert_eq!(t0.blank_left_index, 0);
    assert_eq!(t0.blank_right_index, 1);
    assert_eq!(
        t0.eldorado_id, None,
        "0x00000000 → None (port inagle ZERO_HASH)"
    );

    assert_eq!(telops[1].skill_id, HashId(0x7047_572A));
    assert_eq!(telops[1].blank_left_index, 1);
    assert_eq!(telops[2].skill_id, HashId(0x695C_666B));
    assert_eq!(telops[2].blank_left_index, 2);
}

#[test]
fn resolution_marges_gauche_droite_par_langue() {
    let telops = parse_telop_waza_config(&node_fixture());

    // Entrée 0 : gauche = blank[0].left, droite = blank[1].right.
    // blank[0].left  = ja246 en324 pt290 fr6   it124 de307 es87  zh_hant370 zh_hans347
    // blank[1].right = ja275 en418 pt306 fr245 it308 de382 es176 zh_hant360 zh_hans358
    let t0 = &telops[0];
    assert_eq!(
        margins_vec(&t0.left),
        vec![246, 324, 290, 6, 124, 307, 87, 370, 347]
    );
    assert_eq!(
        margins_vec(&t0.right),
        vec![275, 418, 306, 245, 308, 382, 176, 360, 358]
    );
    // Accès nommé : cohérence du champ struct et de l'accesseur `get`.
    assert_eq!(t0.left.fr, 6);
    assert_eq!(t0.right.de, 382);
    assert_eq!(t0.left.get("ja"), Some(246));
    assert_eq!(t0.left.get("xx"), None);

    // Entrée 1 : gauche = blank[1].left, droite = blank[1].right.
    let t1 = &telops[1];
    assert_eq!(
        margins_vec(&t1.left),
        vec![270, 417, 306, 243, 308, 379, 175, 355, 354]
    );
    // Entrée 2 : gauche = blank[2].left.
    let t2 = &telops[2];
    assert_eq!(
        margins_vec(&t2.left),
        vec![213, 400, 239, 252, 174, 346, 164, 200, 222]
    );
}

#[test]
fn recherche_par_skill_id() {
    let telops = parse_telop_waza_config(&node_fixture());
    let found = find_by_skill_id(&telops, HashId(0x695C_666B));
    assert!(found.is_some());
    assert_eq!(found.unwrap().blank_left_index, 2);
    assert!(find_by_skill_id(&telops, HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn eldorado_id_non_nul_et_index_hors_borne() {
    // Vraie entrée `m_skillTelopInfoList[195]` : id=0x886B9D07, blankSizeInfo=[195, 1],
    // eldoradoId=0xDFEF5884 (la 1re des 27 entrées à eldoradoId non nul du dump réel).
    // La table de blancs de cette fixture n'a que 3 entrées : l'index gauche 195 est hors
    // borne → marges gauche nulles (port inagle « jamais inventer une valeur », l.116),
    // tandis que l'index droit 1 est résolu vers blank[1].right (valeurs réelles).
    let skills =
        vec![json!({ "id": "0x886B9D07", "blankSizeInfo": [195, 1], "eldoradoId": "0xDFEF5884" })];
    let node = json!({
        "lists": [
            { "name": "m_skillTelopInfoList", "typeName": "SkillTelopInfo", "values": skills },
            { "name": "m_blankSizeInfoList", "typeName": "BlankSizeInfo", "values": blank_head() },
        ]
    });
    let telops = parse_telop_waza_config(&node);
    assert_eq!(telops.len(), 1);
    let t = &telops[0];
    assert_eq!(t.skill_id, HashId(0x886B_9D07));
    assert_eq!(t.blank_left_index, 195);
    assert_eq!(t.blank_right_index, 1);
    assert_eq!(t.eldorado_id, Some(HashId(0xDFEF_5884)));
    // Gauche hors borne → toutes nulles.
    assert_eq!(margins_vec(&t.left), vec![0; 9]);
    // Droite = blank[1].right, résolu.
    assert_eq!(
        margins_vec(&t.right),
        vec![275, 418, 306, 245, 308, 382, 176, 360, 358]
    );
}

#[test]
fn vrai_fichier_si_present() {
    // Validation byte-à-byte contre le vrai dump si monté localement (sinon skip).
    // Source VFS : data/common/gamedata/skill/skill_telop_info_config_0.00.00.cfg.bin.
    let path = "/home/aphrody/niers/data/common/gamedata/skill/\
                skill_telop_info_config_0.00.00.cfg.bin.json";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let content = std::fs::read_to_string(path).unwrap();
    let root: Value = serde_json::from_str(&content).unwrap();
    let telops = parse_telop_waza_config(&root);
    assert_eq!(telops.len(), 926, "926 télops dans le vrai dump");
    assert_eq!(telops[0].skill_id, HashId(0x5B6A_04E9));
}
