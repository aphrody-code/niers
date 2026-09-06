#![allow(clippy::pedantic)]
//! Tests golden `team_build_config` (système « Team Build » du DLC Orion) — valeurs réelles
//! extraites du VFS du jeu :
//! `data/common/gamedata/skill/team_build_config_6.00.23.cfg.bin`
//! (monté depuis `/mnt/c/Program Files (x86)/Steam/steamapps/common/INAZUMA ELEVEN
//! Victory Road`, décodé via `nie_formats::cfgbin` → forme iecode `entries`/`children`).
//!
//! Port 1:1 d'inagle `parsers/team-build-config.ts`. Cinq listes :
//! `EFFECT_DATA` (35), `EFFECT_INFO` (30), `UP_DATA` (9), `DOWN_DATA` (6), `INFO` (6).
//! Les valeurs ci-dessous sont les **vraies** lignes du dump, embarquées en fixture.
//!
//! Quirk vérifié : certains `threshold`/`value` d'`EFFECT_DATA` sont des `Float` (`12.5`,
//! `0.5`) que inagle tronque via `parseInt` → on valide la même troncature (`12`, `0`).

use nie_data::hash::HashId;
use nie_data::team_build_config::{NULL_SENTINEL, parse_team_build_config};
use serde_json::{Value, json};

// ════════════════════════════════════════════════════════════════════════════════
//  Données réelles du dump (transcription exacte des `variables[].value`).
// ════════════════════════════════════════════════════════════════════════════════

const N: &str = "-992181094"; // sentinelle null (= 0xC4DC849A).

/// `TEAM_BUILD_EFFECT_DATA` — 35 lignes × 9 vars (effectId, threshold, value, 6 conditions).
/// Les flottants (`12.5`, `0.5`, …) sont gardés tels quels pour valider la troncature.
const EFFECT_DATA: &[[&str; 9]] = &[
    ["-693499735", "0", "5", N, N, N, N, N, N],
    ["-693499735", "5", "5", N, N, N, N, N, N],
    ["-693499735", "10", "5", N, N, N, N, N, N],
    ["-693499735", "15", "5", N, N, N, N, N, N],
    ["-693499735", "20", "5", N, N, N, N, N, N],
    ["1336105235", "0", "5", N, N, N, N, N, N],
    ["1336105235", "5", "5", N, N, N, N, N, N],
    ["1336105235", "10", "5", N, N, N, N, N, N],
    ["1336105235", "15", "5", N, N, N, N, N, N],
    ["1336105235", "20", "5", N, N, N, N, N, N],
    ["950299013", "0", "12.5", N, N, N, N, N, N],
    ["950299013", "12.5", "12.5", N, N, N, N, N, N],
    ["950299013", "25", "12.5", N, N, N, N, N, N],
    ["950299013", "37.5", "12.5", N, N, N, N, N, N],
    ["950299013", "50", "12.5", N, N, N, N, N, N],
    ["651336925", "0", "25", "25", N, N, N, N, N],
    ["-1234337460", "0", "25", "25", N, N, N, N, N],
    ["651336925", "25", "25", "25", N, N, N, N, N],
    ["-1234337460", "25", "25", "25", N, N, N, N, N],
    ["651336925", "50", "25", "25", N, N, N, N, N],
    ["-1234337460", "50", "25", "25", N, N, N, N, N],
    ["651336925", "75", "25", "25", N, N, N, N, N],
    ["-1234337460", "75", "25", "25", N, N, N, N, N],
    ["651336925", "100", "25", "25", N, N, N, N, N],
    ["-1234337460", "100", "25", "25", N, N, N, N, N],
    ["566188228", "60", "60", "15", N, N, N, N, N],
    ["566188228", "45", "60", "15", N, N, N, N, N],
    ["566188228", "30", "60", "15", N, N, N, N, N],
    ["566188228", "15", "60", "15", N, N, N, N, N],
    ["566188228", "0", "60", "15", N, N, N, N, N],
    ["-1049972262", "0", "0.5", N, N, N, N, N, N],
    ["-1049972262", "0.5", "0.5", N, N, N, N, N, N],
    ["-1049972262", "1", "0.5", N, N, N, N, N, N],
    ["-1049972262", "1.5", "0.5", N, N, N, N, N, N],
    ["-1049972262", "2", "0.5", N, N, N, N, N, N],
];

/// `TEAM_BUILD_EFFECT_INFO` — 30 entrées : `count` (en-tête) + paire `[offset, count]`.
const EFFECT_INFO_COUNT: [i64; 30] = [
    1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5,
];
const EFFECT_INFO_REF: [[i64; 2]; 30] = [
    [0, 1],
    [1, 1],
    [2, 1],
    [3, 1],
    [4, 1],
    [5, 1],
    [6, 1],
    [7, 1],
    [8, 1],
    [9, 1],
    [10, 1],
    [11, 1],
    [12, 1],
    [13, 1],
    [14, 1],
    [15, 2],
    [17, 2],
    [19, 2],
    [21, 2],
    [23, 2],
    [25, 1],
    [26, 1],
    [27, 1],
    [28, 1],
    [29, 1],
    [30, 1],
    [31, 1],
    [32, 1],
    [33, 1],
    [34, 1],
];

/// `TEAM_BUILD_UP_DATA` — 9 lignes × 7 vars (type, threshold, multiplier, cv1, cv2, refId, count).
const UP_DATA: [[i64; 7]; 9] = [
    [1, 25, 1, NULL_SENTINEL, NULL_SENTINEL, -936656357, 1],
    [
        2,
        NULL_SENTINEL,
        1,
        NULL_SENTINEL,
        NULL_SENTINEL,
        1361220513,
        1,
    ],
    [
        3,
        NULL_SENTINEL,
        NULL_SENTINEL,
        NULL_SENTINEL,
        NULL_SENTINEL,
        0,
        1,
    ],
    [
        11,
        NULL_SENTINEL,
        NULL_SENTINEL,
        NULL_SENTINEL,
        NULL_SENTINEL,
        0,
        1,
    ],
    [4, 15, 1, NULL_SENTINEL, NULL_SENTINEL, 640000823, 1],
    [5, 15, 1, NULL_SENTINEL, NULL_SENTINEL, -1203685740, 1],
    [17, NULL_SENTINEL, 4, 2, NULL_SENTINEL, -817494526, 4],
    [
        18,
        NULL_SENTINEL,
        NULL_SENTINEL,
        NULL_SENTINEL,
        NULL_SENTINEL,
        0,
        2,
    ],
    [13, 50, 1, NULL_SENTINEL, NULL_SENTINEL, 1448040376, 1],
];

/// `TEAM_BUILD_DOWN_DATA` — 6 lignes × 7 vars.
const DOWN_DATA: [[i64; 7]; 6] = [
    [
        6,
        NULL_SENTINEL,
        3,
        NULL_SENTINEL,
        NULL_SENTINEL,
        -1845035560,
        3,
    ],
    [
        6,
        NULL_SENTINEL,
        1,
        NULL_SENTINEL,
        NULL_SENTINEL,
        -1845035560,
        1,
    ],
    [16, 2, 1, NULL_SENTINEL, NULL_SENTINEL, 185576546, 1],
    [
        7,
        NULL_SENTINEL,
        4,
        NULL_SENTINEL,
        NULL_SENTINEL,
        2080939252,
        4,
    ],
    [9, 15, 1, NULL_SENTINEL, NULL_SENTINEL, -496236201, 1],
    [15, 45, 1, NULL_SENTINEL, NULL_SENTINEL, -1788134975, 1],
];

/// `TEAM_BUILD_INFO` — 6 builds : en-tête `[buildType, buildLevel]` + 3 refs `[up, down, effInfo]`.
const INFO_HEADER: [[i64; 2]; 6] = [[5, 1], [4, 1], [2, 1], [0, 1], [1, 1], [3, 1]];
const INFO_UP_REF: [[i64; 2]; 6] = [[0, 1], [1, 3], [4, 1], [5, 1], [6, 2], [8, 1]];
const INFO_DOWN_REF: [[i64; 2]; 6] = [[0, 1], [1, 1], [2, 1], [3, 1], [4, 1], [5, 1]];
const INFO_EFFINFO_REF: [[i64; 2]; 6] = [[0, 5], [5, 5], [10, 5], [15, 5], [20, 5], [25, 5]];

// ════════════════════════════════════════════════════════════════════════════════
//  Construction de la fixture `{ "entries": [...] }` (forme iecode).
// ════════════════════════════════════════════════════════════════════════════════

/// Construit une variable `{type, value}`. Type `Float` si la chaîne contient un point
/// (fidèle au dump), `Int` sinon — sans incidence sur le parse (qui re-parse `value`).
fn var(s: &str) -> Value {
    let ty = if s.contains('.') { "Float" } else { "Int" };
    json!({ "type": ty, "value": s })
}

fn var_i(n: i64) -> Value {
    json!({ "type": "Int", "value": n.to_string() })
}

fn child(name: String, vars: Vec<Value>) -> Value {
    json!({ "name": name, "variables": vars, "children": [] })
}

fn entry(name: &str, children: Vec<Value>) -> Value {
    json!({ "name": name, "variables": [], "children": children })
}

fn fixture() -> Value {
    // EFFECT_DATA
    let effect_data: Vec<Value> = EFFECT_DATA
        .iter()
        .enumerate()
        .map(|(i, row)| {
            child(
                format!("TEAM_BUILD_EFFECT_DATA_{i}"),
                row.iter().map(|s| var(s)).collect(),
            )
        })
        .collect();

    // EFFECT_INFO : en-tête + ref alternés.
    let mut effect_info = Vec::new();
    for i in 0..EFFECT_INFO_COUNT.len() {
        effect_info.push(child(
            format!("TEAM_BUILD_EFFECT_INFO_{i}"),
            vec![var_i(EFFECT_INFO_COUNT[i])],
        ));
        effect_info.push(child(
            format!("TEAM_BUILD_EFFECT_INFO_REF_EFFECT_DATA_{i}"),
            vec![var_i(EFFECT_INFO_REF[i][0]), var_i(EFFECT_INFO_REF[i][1])],
        ));
    }

    // UP_DATA
    let up_data: Vec<Value> = UP_DATA
        .iter()
        .enumerate()
        .map(|(i, row)| {
            child(
                format!("TEAM_BUILD_UP_DATA_{i}"),
                row.iter().map(|&n| var_i(n)).collect(),
            )
        })
        .collect();

    // DOWN_DATA
    let down_data: Vec<Value> = DOWN_DATA
        .iter()
        .enumerate()
        .map(|(i, row)| {
            child(
                format!("TEAM_BUILD_DOWN_DATA_{i}"),
                row.iter().map(|&n| var_i(n)).collect(),
            )
        })
        .collect();

    // INFO : en-tête + 3 refs.
    let mut info = Vec::new();
    for i in 0..INFO_HEADER.len() {
        info.push(child(
            format!("TEAM_BUILD_INFO_{i}"),
            vec![var_i(INFO_HEADER[i][0]), var_i(INFO_HEADER[i][1])],
        ));
        info.push(child(
            format!("TEAM_BUILD_INFO_REF_UP_DATA_{i}"),
            vec![var_i(INFO_UP_REF[i][0]), var_i(INFO_UP_REF[i][1])],
        ));
        info.push(child(
            format!("TEAM_BUILD_INFO_REF_DOWN_DATA_{i}"),
            vec![var_i(INFO_DOWN_REF[i][0]), var_i(INFO_DOWN_REF[i][1])],
        ));
        info.push(child(
            format!("TEAM_BUILD_INFO_REF_EFFECT_INFO_{i}"),
            vec![var_i(INFO_EFFINFO_REF[i][0]), var_i(INFO_EFFINFO_REF[i][1])],
        ));
    }

    json!({
        "entries": [
            entry("TEAM_BUILD_EFFECT_DATA_LIST_BEG_0", effect_data),
            entry("TEAM_BUILD_EFFECT_INFO_LIST_BEG_0", effect_info),
            entry("TEAM_BUILD_UP_DATA_LIST_BEG_0", up_data),
            entry("TEAM_BUILD_DOWN_DATA_LIST_BEG_0", down_data),
            entry("TEAM_BUILD_INFO_LIST_BEG_0", info),
        ]
    })
}

// ════════════════════════════════════════════════════════════════════════════════
//  Tests
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn null_sentinel_vaut_0xc4dc849a() {
    // Sentinelle confirmée par la consigne : -992181094 >>> 0 = 0xC4DC849A.
    assert_eq!(NULL_SENTINEL, -992_181_094);
    assert_eq!(HashId::from_i64(NULL_SENTINEL).to_hex(), "0xC4DC849A");
}

#[test]
fn comptes_des_cinq_listes() {
    let db = parse_team_build_config(&fixture());
    assert_eq!(db.effect_data.len(), 35, "EFFECT_DATA");
    assert_eq!(db.effect_infos.len(), 30, "EFFECT_INFO");
    assert_eq!(db.up_data.len(), 9, "UP_DATA");
    assert_eq!(db.down_data.len(), 6, "DOWN_DATA");
    assert_eq!(db.build_infos.len(), 6, "INFO");
}

#[test]
fn effect_data_valeurs_reelles_et_conditions() {
    let db = parse_team_build_config(&fixture());

    // Ligne 0 : effectId hash, threshold 0, value 5, aucune condition (toutes = sentinelle).
    let e0 = &db.effect_data[0];
    assert_eq!(e0.effect_id, HashId::from_signed(-693499735));
    assert_eq!(e0.effect_id.to_hex(), "0xD6AA08A9");
    assert_eq!(e0.threshold, 0);
    assert_eq!(e0.value, 5);
    assert!(e0.conditions.is_empty());

    // Ligne 15 : value 25, UNE condition non-sentinelle (25 = 0x00000019).
    let e15 = &db.effect_data[15];
    assert_eq!(e15.effect_id.to_hex(), "0x26D29CDD");
    assert_eq!(e15.value, 25);
    assert_eq!(e15.conditions.len(), 1);
    assert_eq!(e15.conditions[0], HashId(0x0000_0019));

    // Ligne 25 : condition 15 = 0x0000000F.
    let e25 = &db.effect_data[25];
    assert_eq!(e25.value, 60);
    assert_eq!(e25.conditions, vec![HashId(0x0000_000F)]);
}

#[test]
fn flottants_tronques_comme_parseint_inagle() {
    let db = parse_team_build_config(&fixture());
    // Ligne 10 : value Float "12.5" -> parseInt -> 12 (troncature, comme inagle).
    assert_eq!(db.effect_data[10].value, 12);
    // Ligne 11 : threshold "12.5" -> 12, value "12.5" -> 12.
    assert_eq!(db.effect_data[11].threshold, 12);
    assert_eq!(db.effect_data[11].value, 12);
    // Ligne 13 : threshold "37.5" -> 37.
    assert_eq!(db.effect_data[13].threshold, 37);
    // Ligne 30 : value "0.5" -> 0.
    assert_eq!(db.effect_data[30].value, 0);
    // Ligne 33 : threshold "1.5" -> 1, value "0.5" -> 0.
    assert_eq!(db.effect_data[33].threshold, 1);
    assert_eq!(db.effect_data[33].value, 0);
}

#[test]
fn effect_info_compte_et_paires() {
    let db = parse_team_build_config(&fixture());
    let i0 = &db.effect_infos[0];
    assert_eq!(i0.count, 1);
    let r0 = i0.effect_data_ref.expect("ref présente");
    assert_eq!((r0.offset, r0.count), (0, 1));

    // Info 15 : count 1, paire [15, 2] (réfère 2 EFFECT_DATA).
    let i15 = &db.effect_infos[15];
    assert_eq!(i15.count, 1);
    let r15 = i15.effect_data_ref.expect("ref présente");
    assert_eq!((r15.offset, r15.count), (15, 2));

    // Info 29 (dernière) : count 5, paire [34, 1].
    let i29 = &db.effect_infos[29];
    assert_eq!(i29.count, 5);
    assert_eq!(
        i29.effect_data_ref.map(|r| (r.offset, r.count)),
        Some((34, 1))
    );
}

#[test]
fn up_et_down_data_valeurs_reelles() {
    let db = parse_team_build_config(&fixture());

    // UP 0 : type 1, threshold 25, multiplier 1, cv sentinelle, refId 0xC82BC21B, count 1.
    let u0 = &db.up_data[0];
    assert_eq!(u0.modifier_type, 1);
    assert_eq!(u0.threshold, 25);
    assert_eq!(u0.multiplier, 1);
    assert_eq!(u0.cond_value1, NULL_SENTINEL);
    assert_eq!(u0.cond_value2, NULL_SENTINEL);
    assert_eq!(u0.effect_ref_id.to_hex(), "0xC82BC21B");
    assert_eq!(u0.effect_count, 1);

    // UP 2 : effectRefId nul -> 0x00000000.
    assert_eq!(db.up_data[2].effect_ref_id, HashId::ZERO);

    // UP 6 : type 17, cond_value1 = 2 (non sentinelle), refId 0xCF460602, count 4.
    let u6 = &db.up_data[6];
    assert_eq!(u6.modifier_type, 17);
    assert_eq!(u6.cond_value1, 2);
    assert_eq!(u6.effect_ref_id.to_hex(), "0xCF460602");
    assert_eq!(u6.effect_count, 4);

    // DOWN 0 : type 6, multiplier 3, refId 0x9206FDD8, count 3.
    let d0 = &db.down_data[0];
    assert_eq!(d0.modifier_type, 6);
    assert_eq!(d0.multiplier, 3);
    assert_eq!(d0.effect_ref_id.to_hex(), "0x9206FDD8");
    assert_eq!(d0.effect_count, 3);

    // DOWN 5 (dernière) : type 15, threshold 45, refId 0x956B39C1.
    let d5 = &db.down_data[5];
    assert_eq!(d5.modifier_type, 15);
    assert_eq!(d5.threshold, 45);
    assert_eq!(d5.effect_ref_id.to_hex(), "0x956B39C1");
}

#[test]
fn build_info_groupes_et_refs() {
    let db = parse_team_build_config(&fixture());

    // Build 0 : type 5, level 1, up[0,1], down[0,1], effInfo[0,5].
    let b0 = &db.build_infos[0];
    assert_eq!(b0.build_type, 5);
    assert_eq!(b0.build_level, 1);
    assert_eq!(b0.up_data_ref.map(|r| (r.offset, r.count)), Some((0, 1)));
    assert_eq!(b0.down_data_ref.map(|r| (r.offset, r.count)), Some((0, 1)));
    assert_eq!(
        b0.effect_info_ref.map(|r| (r.offset, r.count)),
        Some((0, 5))
    );

    // Build 4 : type 1, up[6,2] (2 modifs), down[4,1], effInfo[20,5].
    let b4 = &db.build_infos[4];
    assert_eq!(b4.build_type, 1);
    assert_eq!(b4.up_data_ref.map(|r| (r.offset, r.count)), Some((6, 2)));
    assert_eq!(
        b4.effect_info_ref.map(|r| (r.offset, r.count)),
        Some((20, 5))
    );

    // Build 5 (dernier) : type 3, up[8,1], down[5,1], effInfo[25,5].
    let b5 = &db.build_infos[5];
    assert_eq!(b5.build_type, 3);
    assert_eq!(b5.up_data_ref.map(|r| (r.offset, r.count)), Some((8, 1)));
    assert_eq!(b5.down_data_ref.map(|r| (r.offset, r.count)), Some((5, 1)));
    assert_eq!(
        b5.effect_info_ref.map(|r| (r.offset, r.count)),
        Some((25, 5))
    );
}

#[test]
fn accesseurs_resolvent_les_tranches() {
    let db = parse_team_build_config(&fixture());

    // effect_data_of(info 15) -> 2 entrées (offset 15, count 2) = EFFECT_DATA[15], [16].
    let i15 = &db.effect_infos[15];
    let slice = db.effect_data_of(i15);
    assert_eq!(slice.len(), 2);
    assert_eq!(slice[0].effect_id.to_hex(), "0x26D29CDD"); // 651336925
    assert_eq!(slice[1].effect_id, HashId::from_signed(-1234337460));

    // up_data_of(build 4) -> 2 entrées (offset 6, count 2) = UP_DATA[6], [7].
    let b4 = &db.build_infos[4];
    let ups = db.up_data_of(b4);
    assert_eq!(ups.len(), 2);
    assert_eq!(ups[0].modifier_type, 17);
    assert_eq!(ups[1].modifier_type, 18);

    // effect_infos_of(build 0) -> 5 entrées (offset 0, count 5).
    let b0 = &db.build_infos[0];
    assert_eq!(db.effect_infos_of(b0).len(), 5);
    // down_data_of(build 5) -> 1 entrée (offset 5).
    assert_eq!(db.down_data_of(&db.build_infos[5]).len(), 1);
}
