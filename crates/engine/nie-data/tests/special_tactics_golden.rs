#![allow(clippy::pedantic)]
//! Tests golden `special_tactics` — noeuds réels extraits du VFS Steam :
//! `data/common/gamedata/skill/special_tactics_config_1.04.09.10.cfg.bin`
//! (jeu monté : `INAZUMA ELEVEN Victory Road/data`).
//!
//! Variante **DLC Orion**, port 1:1 d'inagle
//! `packages/inagle/src/parsers/special-tactics-config.ts` (`parseEntries`).
//! Toutes les valeurs ci-dessous sont les vraies variables du dump (extraites via l'exemple
//! jetable `nie-game/examples/extract_special_tactics.rs`, depuis supprimé). Le dump complet
//! compte 192 effets / 126 conditions / 3 conditions de réussite / 86 tactiques ; on embarque
//! une tranche réelle et cohérente (effets 0..33, conditions 0..6, les 3 réussites,
//! tactiques 0..15) suffisante pour valider le parse et la résolution des tranches.

use nie_data::hash::HashId;
use nie_data::special_tactics::{NULL_SENTINEL, element_name_fr, parse_special_tactics};
use serde_json::{Value, json};

/// Les 33 premiers effets `SPECIAL_TACTICS_EFFECT_*` (9 Int chacun) — valeurs réelles du dump.
const EFFECTS: [[i64; 9]; 33] = [
    [
        1477956381, 40, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -1358789776,
        5,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        -525618056, 25, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -1720251172,
        25,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        906744522, 50, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        1091363420, 25, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -294634422, 25, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        164106573, 20, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        1456233263, 50, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -965523778, 40, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -670714906, 50, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -406425503, 10, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -1317767640,
        30,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        828774099, 5, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        2127503835, 25, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        679290770, 40, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        245749076, 10, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        2040718786, 100, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -1239829967,
        200,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        -1866383113,
        25,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        1601836804, 50, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -1492398424,
        -2111144831,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        -1603925327,
        20,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        1315466141, 20, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -1492398424,
        -2111144831,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        -1603925327,
        50,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        1315466141, 50, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        1839518793,
        -1459146942,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        -1704204520,
        -992181094,
        1178143904,
        10,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        1622791299, 15, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
    [
        -1302700257,
        20,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        -1866383113,
        25,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
        -992181094,
    ],
    [
        164106573, 25, -992181094, -992181094, -992181094, -992181094, -992181094, -992181094,
        -992181094,
    ],
];

/// Les 6 premières conditions `SPECIAL_TACTICS_COND_ID_*` (`condId` Int + base64 String).
const COND_IDS: [(i64, &str); 6] = [
    (1103, "AAAAAA8FNeHe3UoAAQAyAAAAAHg="),
    (1113, "AAAAAA8FNWokrVsAAQAyAAAAAXg="),
    (32, "AAAAAA8FNZACw8AAAQAyAAAAAXg="),
    (1215, "AAAAAA8FNWx/LeMAAQAyAAAAAG8="),
    (11, "AAAAAA8FNQDUTJQAAQAyAAAAAXg="),
    (15, "AAAAAA8FNRVRO9sAAQAyAAAAD3E="),
];

/// Les 3 conditions de réussite `SPECIAL_TACTICS_SUCCESS_COND_ID_*` (totalité du dump).
const SUCCESS_COND_IDS: [(i64, &str); 3] = [
    (1550, "AAAAABsCNUaTCPYAEwIoAAYCMgAAAAAoAAYCMz8AAAA="),
    (1550, "AAAAABsCNUaTCPYAEwIoAAYCMgAAAAAoAAYCMz8AAAA="),
    (1552, "AAAAABsCNUaTCPYAEwIoAAYCMgAAAAIoAAYCMgAAAAI="),
];

/// Les 15 premières tactiques `SPECIAL_TACTICS_INFO_<n>` — 10 vars lues : id, code interne,
/// nameTextId, descTextId, power, recast, element, partenaire×3 (vars 10..16 réservées omises).
struct Info {
    vals: [i64; 9],
    code: &'static str,
}
const INFOS: [Info; 15] = [
    Info {
        code: "wht10010",
        vals: [
            1138274732,
            1107642233,
            -1479637327,
            40,
            90,
            0,
            -1151742935,
            -885835610,
            -1386101235,
        ],
    },
    Info {
        code: "wht10020",
        vals: [
            1760944751,
            1764234426,
            -1931225742,
            40,
            90,
            0,
            -1871405078,
            -534877339,
            -2041801266,
        ],
    },
    Info {
        code: "wht10030",
        vals: [
            1911477038,
            1882400251,
            -1778850765,
            40,
            90,
            1,
            -1989169493,
            -117089756,
            -1621646193,
        ],
    },
    Info {
        code: "wht10040",
        vals: [
            1051674089,
            1064481596,
            -625380620,
            40,
            90,
            2,
            -970049428,
            -1237005085,
            -803851704,
        ],
    },
    Info {
        code: "wht10050",
        vals: [
            666137768,
            644448893,
            -1012776011,
            40,
            90,
            2,
            -550172371,
            -1352663646,
            -921894135,
        ],
    },
    Info {
        code: "wht10060",
        vals: [
            211363691,
            222613950,
            -393277322,
            40,
            90,
            1,
            -199730450,
            -2072859039,
            -501198646,
        ],
    },
    Info {
        code: "wht10070",
        vals: [
            360846890,
            341827839,
            -241950409,
            40,
            90,
            1,
            -318542929,
            -1654022368,
            -79994485,
        ],
    },
    Info {
        code: "wht10080",
        vals: [
            -1843787035,
            -1815610320,
            1980513784,
            40,
            90,
            0,
            1788573536,
            452035567,
            2091089220,
        ],
    },
    Info {
        code: "wht10090",
        vals: [
            -1962853468,
            -1965240975,
            1863781561,
            40,
            90,
            2,
            1937786401,
            65712814,
            1706576901,
        ],
    },
    Info {
        code: "wht20010",
        vals: [
            75025276, 94715305, -529597343, 10, 90, 1, -50743559, -50743559, -356401955,
        ],
    },
    Info {
        code: "wht20010_st0301",
        vals: [
            -2018737520,
            94715305,
            -529597343,
            40,
            90,
            1,
            -50743559,
            -50743559,
            -356401955,
        ],
    },
    Info {
        code: "wht20020",
        vals: [
            794138815,
            780693098,
            -884756574,
            10,
            50,
            0,
            -2121383236,
            -2121383236,
            -1041436898,
        ],
    },
    Info {
        code: "wht20040",
        vals: [
            2031042361,
            2027080172,
            -1659303900,
            30,
            90,
            0,
            0,
            0,
            -1749661544,
        ],
    },
    Info {
        code: "wht20050",
        vals: [
            1611927160,
            1640626349,
            -2080229019,
            30,
            90,
            0,
            1022940574,
            1284645137,
            -1901233703,
        ],
    },
    Info {
        code: "wht20060",
        vals: [
            1262050747,
            1256500078,
            -1355847002,
            99999,
            90,
            2,
            1941521241,
            64206806,
            -1518327270,
        ],
    },
];

/// Liens `..._REF_EFFECT_*` (`[offset, count]`) des 15 premières tactiques.
const REF_EFFECTS: [[i64; 2]; 15] = [
    [0, 3],
    [3, 2],
    [5, 2],
    [7, 2],
    [9, 3],
    [12, 3],
    [15, 2],
    [17, 2],
    [19, 2],
    [21, 3],
    [24, 3],
    [27, 1],
    [28, 2],
    [30, 1],
    [31, 2],
];

/// Liens `..._REF_COND_ID_*` ; la plupart sont `[0,0]` (aucune condition).
const REF_COND_IDS: [[i64; 2]; 15] = [
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 4],
    [0, 0],
    [4, 2],
];

/// Liens `..._REF_SUCCESS_COND_ID_*` ; seules les tactiques 9, 10, 11 en ont.
const REF_SUCCESS_COND_IDS: [[i64; 2]; 15] = [
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 1],
    [1, 1],
    [2, 1],
    [0, 0],
    [0, 0],
    [0, 0],
];

/// Les 15 premiers `__SORT_INDEX_*`.
const SORT_INDEX: [i64; 15] = [32, 9, 24, 81, 56, 66, 46, 36, 76, 5, 65, 55, 35, 45, 75];

/// Construit des variables Int au format iecode `{type:"Int", value:"<n>"}`.
fn int_vars(vals: &[i64]) -> Vec<Value> {
    vals.iter()
        .map(|v| json!({ "type": "Int", "value": v.to_string() }))
        .collect()
}

/// Construit le noeud d'une tactique (var[1] = code interne String, le reste Int).
fn info_node(idx: usize, info: &Info) -> Value {
    let v = &info.vals;
    let variables = json!([
        { "type": "Int", "value": v[0].to_string() },
        { "type": "String", "value": info.code },
        { "type": "Int", "value": v[1].to_string() },
        { "type": "Int", "value": v[2].to_string() },
        { "type": "Int", "value": v[3].to_string() },
        { "type": "Int", "value": v[4].to_string() },
        { "type": "Int", "value": v[5].to_string() },
        { "type": "Int", "value": v[6].to_string() },
        { "type": "Int", "value": v[7].to_string() },
        { "type": "Int", "value": v[8].to_string() },
    ]);
    json!({ "name": format!("SPECIAL_TACTICS_INFO_{idx}"), "variables": variables, "children": [] })
}

/// Reconstruit le noeud racine iecode exact du dump (4 entries, enfants suffixés `_<i>`).
fn fixture() -> Value {
    // Liste des effets.
    let effect_children: Vec<Value> = EFFECTS
        .iter()
        .enumerate()
        .map(|(i, e)| {
            json!({ "name": format!("SPECIAL_TACTICS_EFFECT_{i}"), "variables": int_vars(e), "children": [] })
        })
        .collect();

    // Liste des conditions.
    let cond_children: Vec<Value> = COND_IDS
        .iter()
        .enumerate()
        .map(|(i, (id, data))| {
            json!({
                "name": format!("SPECIAL_TACTICS_COND_ID_{i}"),
                "variables": [
                    { "type": "Int", "value": id.to_string() },
                    { "type": "String", "value": data },
                ],
                "children": [],
            })
        })
        .collect();

    // Liste des conditions de réussite.
    let success_children: Vec<Value> = SUCCESS_COND_IDS
        .iter()
        .enumerate()
        .map(|(i, (id, data))| {
            json!({
                "name": format!("SPECIAL_TACTICS_SUCCESS_COND_ID_{i}"),
                "variables": [
                    { "type": "Int", "value": id.to_string() },
                    { "type": "String", "value": data },
                ],
                "children": [],
            })
        })
        .collect();

    // Liste des tactiques : (INFO_i, REF_EFFECT_i, REF_COND_ID_i, REF_SUCCESS_COND_ID_i)
    // entrelacés, puis tous les __SORT_INDEX_i.
    let mut info_children = Vec::new();
    for (i, info) in INFOS.iter().enumerate() {
        info_children.push(info_node(i, info));
        info_children.push(json!({
            "name": format!("SPECIAL_TACTICS_INFO_REF_EFFECT_{i}"),
            "variables": int_vars(&REF_EFFECTS[i]),
            "children": [],
        }));
        info_children.push(json!({
            "name": format!("SPECIAL_TACTICS_INFO_REF_COND_ID_{i}"),
            "variables": int_vars(&REF_COND_IDS[i]),
            "children": [],
        }));
        info_children.push(json!({
            "name": format!("SPECIAL_TACTICS_INFO_REF_SUCCESS_COND_ID_{i}"),
            "variables": int_vars(&REF_SUCCESS_COND_IDS[i]),
            "children": [],
        }));
    }
    for (i, s) in SORT_INDEX.iter().enumerate() {
        info_children.push(json!({
            "name": format!("__SORT_INDEX_{i}"),
            "variables": int_vars(&[*s]),
            "children": [],
        }));
    }

    json!({
        "entries": [
            { "name": "SPECIAL_TACTICS_EFFECT_LIST_BEG_0", "variables": int_vars(&[192]), "children": effect_children },
            { "name": "SPECIAL_TACTICS_COND_ID_LIST_BEG_0", "variables": int_vars(&[126]), "children": cond_children },
            { "name": "SPECIAL_TACTICS_SUCCESS_COND_ID_LIST_BEG_0", "variables": int_vars(&[3]), "children": success_children },
            { "name": "SPECIAL_TACTICS_INFO_LIST_BEG_0", "variables": int_vars(&[86]), "children": info_children },
        ]
    })
}

#[test]
fn comptes_des_listes() {
    let cfg = parse_special_tactics(&fixture());
    assert_eq!(cfg.effects.len(), 33, "33 effets embarqués");
    assert_eq!(cfg.cond_ids.len(), 6, "6 conditions embarquées");
    assert_eq!(
        cfg.success_cond_ids.len(),
        3,
        "3 conditions de réussite (totalité du dump)"
    );
    assert_eq!(cfg.infos.len(), 15, "15 tactiques embarquées");
    assert_eq!(cfg.ref_effects.len(), 15);
    assert_eq!(cfg.ref_cond_ids.len(), 15);
    assert_eq!(cfg.ref_success_cond_ids.len(), 15);
    assert_eq!(cfg.sort_index.len(), 15);
}

#[test]
fn effets_id_power_conditions() {
    let cfg = parse_special_tactics(&fixture());

    // Effet 0 : id 0x5817D31D, power 40, aucune condition (tout sentinelle).
    assert_eq!(cfg.effects[0].effect_id, HashId::from_signed(1477956381));
    assert_eq!(cfg.effects[0].effect_id.to_hex(), "0x5817D31D");
    assert_eq!(cfg.effects[0].power, 40);
    assert!(cfg.effects[0].conditions.is_empty());

    // Effet 28 : power = NULL_SENTINEL (conservé brut), 2 conditions (var[2], var[3]).
    let e28 = &cfg.effects[28];
    assert_eq!(e28.effect_id.to_hex(), "0x9A6BE718");
    assert_eq!(
        e28.power, NULL_SENTINEL,
        "la puissance n'est PAS filtrée de la sentinelle"
    );
    assert_eq!(e28.conditions.len(), 2);
    assert_eq!(e28.conditions[0], HashId::from_signed(1178143904));
    assert_eq!(e28.conditions[0].to_hex(), "0x46390CA0");
    assert_eq!(e28.conditions[1], HashId(10));
    assert_eq!(e28.conditions[1].to_hex(), "0x0000000A");

    // Doublon réel : effets 21 et 24 partagent id 0xA6F6D9E8 et power -2111144831.
    assert_eq!(cfg.effects[21].effect_id, cfg.effects[24].effect_id);
    assert_eq!(cfg.effects[21].power, -2111144831);
}

#[test]
fn sentinelle_null_filtree_des_conditions() {
    assert_eq!(NULL_SENTINEL, -992_181_094);
    let cfg = parse_special_tactics(&fixture());
    for effect in &cfg.effects {
        for cond in &effect.conditions {
            assert_ne!(
                cond.get(),
                0xC4DC_849A,
                "la sentinelle ne doit jamais rester en condition"
            );
        }
    }
}

#[test]
fn conditions_id_et_base64() {
    let cfg = parse_special_tactics(&fixture());

    // Condition 0.
    assert_eq!(cfg.cond_ids[0].cond_id, 1103);
    assert_eq!(cfg.cond_ids[0].cond_data, "AAAAAA8FNeHe3UoAAQAyAAAAAHg=");
    // Condition 2.
    assert_eq!(cfg.cond_ids[2].cond_id, 32);

    // Conditions de réussite : la 0 et la 1 partagent les mêmes données (doublon réel).
    assert_eq!(cfg.success_cond_ids[0].cond_id, 1550);
    assert_eq!(
        cfg.success_cond_ids[0].cond_data,
        cfg.success_cond_ids[1].cond_data
    );
    assert_eq!(cfg.success_cond_ids[2].cond_id, 1552);
}

#[test]
fn tactiques_champs_de_base() {
    let cfg = parse_special_tactics(&fixture());

    // Tactique 0 : "wht10010", élément 0 (Néant), 3 partenaires.
    let t0 = &cfg.infos[0];
    assert_eq!(t0.tactics_id, HashId::from_signed(1138274732));
    assert_eq!(t0.tactics_id.to_hex(), "0x43D8B1AC");
    assert_eq!(t0.internal_code, "wht10010");
    assert_eq!(t0.name_text_id.to_hex(), "0x42054779");
    assert_eq!(t0.power, 40);
    assert_eq!(t0.recast_time, 90);
    assert_eq!(t0.element, 0);
    assert_eq!(t0.element_name(), "Néant");
    assert_eq!(t0.partner_ids.len(), 3);
    assert_eq!(t0.partner_ids[0], HashId::from_signed(-1151742935));
    assert_eq!(t0.partner_ids[0].to_hex(), "0xBB59CC29");

    // Tactique 12 : "wht20040", 2 partenaires à 0 filtrés → 1 seul partenaire conservé.
    let t12 = &cfg.infos[12];
    assert_eq!(t12.tactics_id.to_hex(), "0x790F3F39");
    assert_eq!(t12.internal_code, "wht20040");
    assert_eq!(
        t12.partner_ids.len(),
        1,
        "les 2 partenaires à 0x00000000 sont filtrés"
    );
    assert_eq!(t12.partner_ids[0].to_hex(), "0x97B64898");

    // Tactique 14 : power réel = 99999 (valeur extrême du dump).
    assert_eq!(cfg.infos[14].power, 99999);
}

#[test]
fn ref_effects_chainage() {
    let cfg = parse_special_tactics(&fixture());
    // Les couples [offset, count] des tactiques 0..15 se chaînent sans trou.
    let mut cursor = 0;
    for (i, r) in cfg.ref_effects.iter().enumerate() {
        assert_eq!(r.offset, cursor, "chaînage REF_EFFECT à l'index {i}");
        cursor = r.offset + r.count;
    }
    assert_eq!(
        cursor, 33,
        "les 15 tactiques couvrent exactement les 33 effets embarqués"
    );
}

#[test]
fn effects_of_resout_la_tranche() {
    let cfg = parse_special_tactics(&fixture());

    // Tactique 0 → effets [0,3[.
    let e0 = cfg.effects_of(0);
    assert_eq!(e0.len(), 3);
    assert_eq!(e0[0].effect_id.to_hex(), "0x5817D31D");

    // Tactique 12 → effets [28,30[ : l'effet 28 (power sentinelle, 2 conditions) en tête.
    let e12 = cfg.effects_of(12);
    assert_eq!(e12.len(), 2);
    assert_eq!(e12[0].effect_id.to_hex(), "0x9A6BE718");
    assert_eq!(e12[0].conditions.len(), 2);

    // Index hors borne → tranche vide.
    assert!(cfg.effects_of(99).is_empty());
}

#[test]
fn cond_et_success_resolution() {
    let cfg = parse_special_tactics(&fixture());

    // La plupart des tactiques n'ont aucune condition.
    assert!(cfg.cond_ids_of(0).is_empty());
    // Tactique 12 → conditions [0,4[.
    let c12 = cfg.cond_ids_of(12);
    assert_eq!(c12.len(), 4);
    assert_eq!(c12[0].cond_id, 1103);
    // Tactique 14 → conditions [4,2[.
    assert_eq!(cfg.cond_ids_of(14).len(), 2);
    assert_eq!(cfg.cond_ids_of(14)[0].cond_id, 11);

    // Conditions de réussite : seules les tactiques 9, 10, 11 en ont (1 chacune).
    assert!(cfg.success_cond_ids_of(0).is_empty());
    let s9 = cfg.success_cond_ids_of(9);
    assert_eq!(s9.len(), 1);
    assert_eq!(s9[0].cond_id, 1550);
    assert_eq!(cfg.success_cond_ids_of(11)[0].cond_id, 1552);
}

#[test]
fn sort_index_et_find_info() {
    let cfg = parse_special_tactics(&fixture());
    assert_eq!(
        cfg.sort_index,
        vec![32, 9, 24, 81, 56, 66, 46, 36, 76, 5, 65, 55, 35, 45, 75]
    );

    // find_info par tactics_id.
    let found = cfg
        .find_info(HashId(0x790F_3F39))
        .expect("tactique 0x790F3F39");
    assert_eq!(found.internal_code, "wht20040");
    assert!(cfg.find_info(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn element_name_mapping() {
    assert_eq!(element_name_fr(0), "Néant");
    assert_eq!(element_name_fr(1), "Vent");
    assert_eq!(element_name_fr(2), "Forêt");
    assert_eq!(element_name_fr(3), "Feu");
    assert_eq!(element_name_fr(4), "Montagne");
    assert_eq!(element_name_fr(99), "Néant", "valeur inconnue → Néant");
}
