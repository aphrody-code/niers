#![allow(clippy::pedantic)]
//! Tests golden `ability_learning` — sections réelles tirées du VFS IEVR :
//! `data/common/gamedata/skill/ability_learning_config_1.03.63.00.cfg.bin`
//! (format T2B `entries`, dump complet = 23790 effets / 877 plateaux).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/ability-learning.ts` :
//! effet = `(hash=toHex(var0), type=var2, value=var3)`, `id` = index du nœud ;
//! plateau = `toHex(BOARD_INFO_N.var0)` → empile `REF_EFFECT_N.var0` (frère de même index).
//! Vérité terrain = la sortie d'inagle sur le vrai fichier. Valeurs extraites du VFS, non inventées.

use nie_data::ability_learning::{AbilityBoardEffect, parse_ability_learning_config};
use nie_data::hash::HashId;
use serde_json::{Value, json};

/// Construit un nœud `{name, variables:[Int...], children:[]}` à partir de valeurs entières brutes.
fn node(name: &str, vars: &[i64]) -> Value {
    let variables: Vec<_> = vars
        .iter()
        .map(|v| json!({ "type": "Int", "value": v.to_string() }))
        .collect();
    json!({ "name": name, "variables": variables, "children": [] })
}

/// Fixture = sous-ensemble RÉEL des deux sections exploitées (valeurs exactes du dump).
fn fixture() -> Value {
    // ABILITY_LEARNING_BOARD_EFFECT_N : [var0(hash), var1, var2(type), var3(value), var4, var5].
    let effects = vec![
        node(
            "ABILITY_LEARNING_BOARD_EFFECT_0",
            &[1_530_105_677, 0, 0, 0, 0, 0],
        ),
        node(
            "ABILITY_LEARNING_BOARD_EFFECT_1",
            &[-2_085_093_640, 0, 0, 0, 0, 0],
        ),
        node(
            "ABILITY_LEARNING_BOARD_EFFECT_2",
            &[-1_036_329_225, 0, 0, 0, 0, 0],
        ),
        node(
            "ABILITY_LEARNING_BOARD_EFFECT_3",
            &[448_704_322, 0, 0, 0, 0, 0],
        ),
        node(
            "ABILITY_LEARNING_BOARD_EFFECT_4",
            &[-1_254_232_479, 0, 0, 0, 0, 0],
        ),
        node(
            "ABILITY_LEARNING_BOARD_EFFECT_5",
            &[-1_099_900_038, 0, 0, 0, 0, 0],
        ),
        node(
            "ABILITY_LEARNING_BOARD_EFFECT_28",
            &[-992_181_094, 0, 1, 0, 0, 0],
        ),
        node(
            "ABILITY_LEARNING_BOARD_EFFECT_34",
            &[541_516_885, 0, 6, 0, 0, 0],
        ),
        node(
            "ABILITY_LEARNING_BOARD_EFFECT_35",
            &[541_516_885, 0, 12, 0, 0, 0],
        ),
    ];
    // ABILITY_LEARNING_BOARD_INFO_N : [var0(boardId hash), var1] ;
    // ABILITY_LEARNING_BOARD_INFO_REF_EFFECT_N : [var0(offset effet), var1(nb)].
    // __SORT_INDEX_0 : nœud parasite que le parseur DOIT ignorer.
    let info = vec![
        node("ABILITY_LEARNING_BOARD_INFO_0", &[-522_614_678, 0]),
        node("ABILITY_LEARNING_BOARD_INFO_REF_EFFECT_0", &[0, 0]),
        node("ABILITY_LEARNING_BOARD_INFO_1", &[-2_012_826_428, 0]),
        node("ABILITY_LEARNING_BOARD_INFO_REF_EFFECT_1", &[0, 28]),
        node("ABILITY_LEARNING_BOARD_INFO_2", &[223_714_263, 183_387_409]),
        node("ABILITY_LEARNING_BOARD_INFO_REF_EFFECT_2", &[28, 12]),
        node("ABILITY_LEARNING_BOARD_INFO_3", &[385_138_438, 0]),
        node("ABILITY_LEARNING_BOARD_INFO_REF_EFFECT_3", &[40, 6]),
        node("ABILITY_LEARNING_BOARD_INFO_4", &[-210_370_828, 0]),
        node("ABILITY_LEARNING_BOARD_INFO_REF_EFFECT_4", &[46, 6]),
        node(
            "ABILITY_LEARNING_BOARD_INFO_5",
            &[-1_688_650_719, -1_331_090_332],
        ),
        node("ABILITY_LEARNING_BOARD_INFO_REF_EFFECT_5", &[52, 53]),
        node("ABILITY_LEARNING_BOARD_INFO_6", &[38_825_371, 698_383_838]),
        node("ABILITY_LEARNING_BOARD_INFO_REF_EFFECT_6", &[105, 76]),
        node("__SORT_INDEX_0", &[873]),
    ];
    json!({
        "entries": [
            { "name": "ABILITY_LEARNING_BOARD_EFFECT_LIST_BEG_0", "variables": [], "children": effects },
            { "name": "ABILITY_LEARNING_BOARD_INFO_LIST_BEG_0", "variables": [], "children": info },
        ]
    })
}

#[test]
fn effets_hash_type_value_du_vrai_dump() {
    let cfg = parse_ability_learning_config(&fixture());
    assert_eq!(cfg.effects.len(), 9, "9 effets dans la fixture");

    // EFFECT_0 : hash=toHex(var0), type=var2=0, value=var3=0.
    let e0: &AbilityBoardEffect = &cfg.effects[&0];
    assert_eq!(e0.id, 0);
    assert_eq!(e0.hash, HashId::from_signed(1_530_105_677));
    assert_eq!(e0.hash.to_hex(), "0x5B338F4D");
    assert_eq!(e0.effect_type, 0);
    assert_eq!(e0.value, 0);

    // var0 signé → non-signé (réinterprétation des bits, comme `>>> 0`).
    assert_eq!(cfg.effects[&1].hash, HashId::from_signed(-2_085_093_640));
    assert_eq!(cfg.effects[&1].hash.to_hex(), "0x83B7FEF8");

    // EFFECT_28 : type=1, hash 0xC4DC849A.
    assert_eq!(cfg.effects[&28].effect_type, 1);
    assert_eq!(cfg.effects[&28].hash.to_hex(), "0xC4DC849A");

    // EFFECT_34 / 35 : même hash (0x2046E455), types distincts (6 puis 12) — var2 bien lu.
    assert_eq!(cfg.effects[&34].hash, HashId::from_signed(541_516_885));
    assert_eq!(cfg.effects[&34].hash.to_hex(), "0x2046E455");
    assert_eq!(cfg.effects[&34].effect_type, 6);
    assert_eq!(cfg.effects[&35].effect_type, 12);
    assert_eq!(cfg.effects[&35].hash, cfg.effects[&34].hash);
}

#[test]
fn plateaux_lient_boardid_a_offset_effet() {
    let cfg = parse_ability_learning_config(&fixture());
    // 7 plateaux (BOARD_INFO_0..6), le nœud __SORT_INDEX_0 est ignoré.
    assert_eq!(cfg.boards.len(), 7, "7 plateaux, __SORT_INDEX exclu");

    // boardId = toHex(BOARD_INFO_N.var0) ; valeur empilée = REF_EFFECT_N.var0 (l'offset).
    assert_eq!(cfg.boards[&HashId::from_signed(-522_614_678)], vec![0]); // 0xE0D9886A → [0]
    assert_eq!(cfg.boards[&HashId(0xE0D9_886A)], vec![0]);
    assert_eq!(cfg.boards[&HashId::from_signed(223_714_263)], vec![28]); // 0x0D559BD7 → [28]
    assert_eq!(cfg.boards[&HashId(0x0D55_9BD7)], vec![28]);
    assert_eq!(cfg.boards[&HashId::from_signed(385_138_438)], vec![40]);
    assert_eq!(cfg.boards[&HashId::from_signed(-210_370_828)], vec![46]);
    assert_eq!(cfg.boards[&HashId::from_signed(-1_688_650_719)], vec![52]); // 0x9B593C21 → [52]
    assert_eq!(cfg.boards[&HashId(0x9B59_3C21)], vec![52]);
    assert_eq!(cfg.boards[&HashId::from_signed(38_825_371)], vec![105]);
}

#[test]
fn empile_var0_du_ref_effect_pas_var1() {
    // Garde anti-régression : on empile bien REF_EFFECT_N.var0 (offset), et NON var1 (le compte).
    // BOARD_INFO_REF_EFFECT_2 = [28, 12] → la valeur attendue est 28, pas 12.
    let cfg = parse_ability_learning_config(&fixture());
    let v = &cfg.boards[&HashId::from_signed(223_714_263)];
    assert_eq!(v, &vec![28], "var0 (offset) empilé, pas var1 (12)");
}

#[test]
fn racine_vide_donne_config_vide() {
    let cfg = parse_ability_learning_config(&json!({ "entries": [] }));
    assert!(cfg.effects.is_empty());
    assert!(cfg.boards.is_empty());
}
