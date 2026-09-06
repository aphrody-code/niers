#![allow(clippy::pedantic)]
//! Tests golden `chara_costume` — vrais enfants `CHARA_COSTUME_MODEL_*` tires du VFS :
//! `data/common/gamedata/character/chara_costume_1.02.28.00.cfg.bin`
//! (jeu Steam « INAZUMA ELEVEN Victory Road »).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/chara-costume-config.ts` (`parseEntries`
//! l.39-61) : une seule liste `CHARA_COSTUME_MODEL_LIST_BEG_0` contenant 577 enfants
//! (le commentaire d'inagle dit « 576 » mais le dump reel en a 577 ; sa variable d'en-tete
//! vaut 577). Chaque enfant = 4 variables Int `[type, modelRefCrc, flag1, flag2]`,
//! `modelRefCrc` rendu en hex non signe via `toHex`.
//!
//! Les valeurs ci-dessous sont les **5 premiers enfants contigus** (index 0..4) du vrai
//! fichier, extraits via `nie-formats` (CfgBin T2B) puis les conventions de nommage iecode
//! (`<base>_<i>`). Verite terrain = la sortie d'inagle sur le meme fichier.

use nie_data::chara_costume::{CharaCostume, parse_all_chara_costumes};
use nie_data::hash::HashId;
use serde_json::json;

/// Les 5 premiers enfants reels (index 0..4) : `[type, modelRefCrc, flag1, flag2]`.
/// Source : `CHARA_COSTUME_MODEL_LIST_BEG`, enfants 0..4 du dump reel (voir en-tete).
const RAW: [[i64; 4]; 5] = [
    [0, 590574594, 0, 0],  // 0x23337402
    [0, 2015922578, 0, 0], // 0x78288992
    [0, 1875023921, 0, 0], // 0x6FC29831
    [2, 1057950082, 0, 0], // 0x3F0F0982
    [0, -972583323, 0, 0], // 0xC6078E65 (modelRef signe -> non signe via toHex)
];

/// Construit un noeud iecode `entries` : la liste MODEL (avec les 5 enfants reels) plus
/// les deux listes freres DATA/INFO **vides** (pour prouver qu'elles sont ignorees, sans
/// inventer de donnees). Noms suffixes `_<i>` comme le rendu iecode.
fn node_fixture() -> serde_json::Value {
    let children: Vec<_> = RAW
        .iter()
        .enumerate()
        .map(|(i, [kind, model_ref, f1, f2])| {
            json!({
                "name": format!("CHARA_COSTUME_MODEL_{i}"),
                "variables": [
                    { "type": "Int", "value": kind.to_string() },
                    { "type": "Int", "value": model_ref.to_string() },
                    { "type": "Int", "value": f1.to_string() },
                    { "type": "Int", "value": f2.to_string() },
                ],
                "children": []
            })
        })
        .collect();
    json!({
        "entries": [
            { "name": "CHARA_COSTUME_MODEL_LIST_BEG_0", "variables": [], "children": children },
            { "name": "CHARA_COSTUME_DATA_LIST_BEG_0",  "variables": [], "children": [] },
            { "name": "CHARA_COSTUME_INFO_LIST_BEG_0",  "variables": [], "children": [] },
        ]
    })
}

#[test]
fn chara_costume_compte_et_index() {
    let parsed = parse_all_chara_costumes(&node_fixture());
    assert_eq!(parsed.len(), 5, "5 costumes (DATA/INFO freres ignores)");
    // L'index est la position brute dans le LIST_BEG (le `i` d'inagle), 0..4.
    for (i, c) in parsed.iter().enumerate() {
        assert_eq!(c.index, i, "index du costume {i}");
    }
}

#[test]
fn chara_costume_champs_reels() {
    let parsed = parse_all_chara_costumes(&node_fixture());
    for (i, raw) in RAW.iter().enumerate() {
        let c: &CharaCostume = &parsed[i];
        assert_eq!(c.kind, raw[0], "type (kind) du costume {i}");
        assert_eq!(
            c.model_ref,
            HashId::from_i64(raw[1]),
            "modelRef du costume {i}"
        );
        assert_eq!(c.flag1, raw[2], "flag1 du costume {i}");
        assert_eq!(c.flag2, raw[3], "flag2 du costume {i}");
    }
}

#[test]
fn chara_costume_modelref_hex_to_hex() {
    // modelRef rendu via `toHex` (signe -> non signe), identique a inagle (data-loader.ts l.456).
    let parsed = parse_all_chara_costumes(&node_fixture());
    let expected_hex = [
        "0x23337402",
        "0x78288992",
        "0x6FC29831",
        "0x3F0F0982",
        "0xC6078E65", // -972583323 >>> 0
    ];
    for (i, hex) in expected_hex.iter().enumerate() {
        assert_eq!(
            parsed[i].model_ref.to_hex(),
            *hex,
            "hex modelRef costume {i}"
        );
    }
}

#[test]
fn chara_costume_listes_freres_ignorees() {
    // Garde anti-regression : seules les entrees `CHARA_COSTUME_MODEL_LIST_BEG*` sont
    // descendues. DATA/INFO (memes prefixes longs) ne doivent jamais produire de costume.
    let only_data = json!({
        "entries": [
            { "name": "CHARA_COSTUME_DATA_LIST_BEG_0", "variables": [], "children": [
                { "name": "CHARA_COSTUME_DATA_0", "variables": [
                    { "type": "Int", "value": "0" }, { "type": "Int", "value": "0" },
                    { "type": "Int", "value": "0" }, { "type": "Int", "value": "0" },
                ], "children": [] }
            ] },
        ]
    });
    assert!(parse_all_chara_costumes(&only_data).is_empty());
}

#[test]
fn chara_costume_enfant_trop_court_saute() {
    // inagle : `if (vars.length < 4) continue;` — un enfant a 3 vars est saute.
    let short = json!({
        "entries": [
            { "name": "CHARA_COSTUME_MODEL_LIST_BEG_0", "variables": [], "children": [
                { "name": "CHARA_COSTUME_MODEL_0", "variables": [
                    { "type": "Int", "value": "1" }, { "type": "Int", "value": "2" },
                    { "type": "Int", "value": "3" },
                ], "children": [] },
                { "name": "CHARA_COSTUME_MODEL_1", "variables": [
                    { "type": "Int", "value": "0" }, { "type": "Int", "value": "590574594" },
                    { "type": "Int", "value": "0" }, { "type": "Int", "value": "0" },
                ], "children": [] },
            ] },
        ]
    });
    let parsed = parse_all_chara_costumes(&short);
    assert_eq!(
        parsed.len(),
        1,
        "l'enfant a 3 vars est saute, le valide reste"
    );
    // L'index reste la position brute : le costume valide est l'enfant n.1.
    assert_eq!(parsed[0].index, 1);
    assert_eq!(parsed[0].model_ref, HashId(0x2333_7402));
}
