#![allow(clippy::pedantic)]
//! Tests golden `party` — valeurs réelles tirées de :
//! - `data/common/gamedata/party/ctrl_chara_config_1.04.17.00.cfg.bin.json`
//! - `data/common/gamedata/party/party_departure_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/party/supecify_party0.00.00.cfg.bin.json`
//! - `data/common/gamedata/party/guest_limit_config.cfg.bin.json`
//!
//! ## Vérifications champ par champ
//!
//! ### ctrl_chara_config : CTRL_CHR_DATA_0 (lignes 13-29)
//! - var\[0\] = 1751902334 (ligne 17) → chara_id = 0x686BE87E
//! - var\[1\] = 1 (ligne 21) → ctrl_type = 1
//! - var\[2\] = 0 (ligne 25) → flag = 0
//!
//! ### ctrl_chara_config : CTRL_CHR_DATA_1 (lignes 31-46)
//! - var\[0\] = 1128709053 (ligne 35) → chara_id = 0x4346BBBD
//! - var\[1\] = 2 (ligne 39) → ctrl_type = 2
//!
//! ### party_departure : entrée 0
//! - `charaId`   = "0x686BE87E" (ligne 9)
//! - `textureId` = "0x3CCD6535" (ligne 10)
//!
//! ### party_departure : entrée 1
//! - `charaId`   = "0x4346BBBD" (ligne 13)
//! - `textureId` = "0x17E036F6" (ligne 14)
//!
//! ### supecify_party : m_PartyRefCharaList[0] (ligne 9)
//! - slot=0, type=3, paramId="0xC19FE60C", lv=0, lvType=0
//!
//! ### supecify_party : m_PartyRefCharaList[9] (ligne 92)
//! - slot=0, type=1, paramId="0xBB5FB56C", lv=21
//!
//! ### supecify_party : m_SpecifyPartyList[0] (ligne 177)
//! - partyId="0xA8052252", chara=[0, 1] → chara_offset=0, chara_count=1
//!
//! ### supecify_party : m_SpecifyPartyList[3]
//! - partyId="0x51B82450", chara=[4, 5] → chara_offset=4, chara_count=5
//!
//! ### guest_limit_config : GENERAL_LIMIT_INFO_0 (ligne 13) + GENERAL_LIMIT_INFO_DATA_0 (ligne 39)
//! - rule_hash = -1346664332 (ligne 17) → 0xAFBB8874
//! - condition_data = "AAAAADALNb7FPqIACgEoAAYCNOlC0FgyAAAAAXg1TO8TnAAKASgABgI0MH+9OTIAAAABeY8="
//!
//! ### guest_limit_config : GENERAL_LIMIT_INFO_1
//! - rule_hash = 1457222339 → 0x56DB72C3
//! - condition_data = "AAAAADALNb7FPqIACgEoAAYCNHBLgeIyAAAAAXg1TO8TnAAKASgABgI0MH+9OTIAAAABeY8="

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::party::{
    SpecifyPartyCharaData, SpecifyPartyData, parse_ctrl_chara_config, parse_guest_limit_config,
    parse_party_departure, parse_specify_party,
};
use serde_json::json;

// ─── Constantes golden (vérifiées en Python : v & 0xFFFF_FFFF) ───────────────

/// 1751902334 & 0xFFFFFFFF = 0x686BE87E
const CHARA_ID_0: HashId = HashId(0x686BE87E);
/// 1128709053 & 0xFFFFFFFF = 0x4346BBBD
const CHARA_ID_1: HashId = HashId(0x4346BBBD);
/// party_departure[0].textureId = 0x3CCD6535
const TEX_ID_0: HashId = HashId(0x3CCD6535);
/// party_departure[1].textureId = 0x17E036F6
const TEX_ID_1: HashId = HashId(0x17E036F6);
/// m_PartyRefCharaList[0].paramId = 0xC19FE60C
const PARAM_ID_0: HashId = HashId(0xC19FE60C);
/// m_PartyRefCharaList[9].paramId = 0xBB5FB56C
const PARAM_ID_9: HashId = HashId(0xBB5FB56C);
/// m_SpecifyPartyList[0].partyId = 0xA8052252
const PARTY_ID_0: HashId = HashId(0xA8052252);
/// m_SpecifyPartyList[3].partyId = 0x51B82450
const PARTY_ID_3: HashId = HashId(0x51B82450);
/// GENERAL_LIMIT_INFO_0 : -1346664332 & 0xFFFF_FFFF = 0xAFBB8874
const RULE_HASH_0: HashId = HashId(0xAFBB8874);
/// GENERAL_LIMIT_INFO_1 : 1457222339 & 0xFFFF_FFFF = 0x56DB72C3
const RULE_HASH_1: HashId = HashId(0x56DB72C3);

// ─── Fixtures ─────────────────────────────────────────────────────────────────
//
// Valeurs extraites octet-pour-octet des dumps réels — voir références en tête de fichier.

/// Fixture minimale de `ctrl_chara_config` (2 premières entrées + conteneur).
fn fixture_ctrl_chara() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "CTRL_CHR_DATA_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    // CTRL_CHR_DATA_0 — ctrl_chara_config lignes 13-29
                    {
                        "name": "CTRL_CHR_DATA_0",
                        "variables": [
                            {"type": "Int", "value": "1751902334"}, // chara_id (ligne 17)
                            {"type": "Int", "value": "1"},          // ctrl_type (ligne 21)
                            {"type": "Int", "value": "0"}           // flag (ligne 25)
                        ],
                        "children": []
                    },
                    // CTRL_CHR_DATA_1 — ctrl_chara_config lignes 31-46
                    {
                        "name": "CTRL_CHR_DATA_1",
                        "variables": [
                            {"type": "Int", "value": "1128709053"}, // chara_id (ligne 35)
                            {"type": "Int", "value": "2"},          // ctrl_type (ligne 39)
                            {"type": "Int", "value": "0"}
                        ],
                        "children": []
                    }
                ]
            }
        ]
    })
}

/// Fixture minimale de `party_departure` (2 premières entrées).
fn fixture_party_departure() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_partyDeparture",
                "typeName": "PartyDeparture",
                "values": [
                    // entrée 0 — party_departure ligne 9-10
                    { "charaId": "0x686BE87E", "textureId": "0x3CCD6535" },
                    // entrée 1 — party_departure ligne 13-14
                    { "charaId": "0x4346BBBD", "textureId": "0x17E036F6" }
                ]
            }
        ]
    })
}

/// Fixture minimale de `supecify_party` (2 chara + 2 party).
fn fixture_specify_party() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_PartyRefCharaList",
                "typeName": "SPECIFY_PARTY_CHARA_DATA",
                "values": [
                    // m_PartyRefCharaList[0] — ligne 9
                    {
                        "slot": 0,
                        "type": 3,
                        "paramId": "0xC19FE60C",
                        "lv": 0,
                        "lvType": 0,
                        "lvParamId": "0x00000000",
                        "equip": "0x00000000"
                    },
                    // m_PartyRefCharaList[9] (représenté ici à l'index 1) — ligne 92
                    {
                        "slot": 0,
                        "type": 1,
                        "paramId": "0xBB5FB56C",
                        "lv": 21,
                        "lvType": 0,
                        "lvParamId": "0x00000000",
                        "equip": "0x00000000"
                    }
                ]
            },
            {
                "name": "m_SpecifyPartyList",
                "typeName": "SPECIFY_PARTY_DATA",
                "values": [
                    // m_SpecifyPartyList[0] — ligne 177
                    { "partyId": "0xA8052252", "chara": [0, 1] },
                    // m_SpecifyPartyList[3] (représenté ici à l'index 1)
                    { "partyId": "0x51B82450", "chara": [4, 5] }
                ]
            }
        ]
    })
}

/// Fixture minimale de `guest_limit_config` (2 premières règles, structure niché).
fn fixture_guest_limit() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "GENERAL_LIMIT_INFO_LIST_BEG_0",
                "variables": [{"type": "Int", "value": "2"}],
                "children": [
                    // GENERAL_LIMIT_INFO_0 — ligne 13 du fichier réel
                    {
                        "name": "GENERAL_LIMIT_INFO_0",
                        "variables": [
                            {"type": "Int", "value": "-1346664332"}  // ligne 17
                        ],
                        "children": [
                            {
                                "name": "GENERAL_LIMIT_INFO_DATA_LIST_BEG_0",
                                "variables": [{"type": "Int", "value": "1"}],
                                "children": [
                                    // GENERAL_LIMIT_INFO_DATA_0 — ligne 39
                                    {
                                        "name": "GENERAL_LIMIT_INFO_DATA_0",
                                        "variables": [
                                            {"type": "Int", "value": "0"},
                                            {"type": "String", "value": "AAAAADALNb7FPqIACgEoAAYCNOlC0FgyAAAAAXg1TO8TnAAKASgABgI0MH+9OTIAAAABeY8="}
                                        ],
                                        "children": [
                                            // GENERAL_LIMIT_INFO_1
                                            {
                                                "name": "GENERAL_LIMIT_INFO_1",
                                                "variables": [
                                                    {"type": "Int", "value": "1457222339"}
                                                ],
                                                "children": [
                                                    {
                                                        "name": "GENERAL_LIMIT_INFO_DATA_LIST_BEG_1",
                                                        "variables": [{"type": "Int", "value": "1"}],
                                                        "children": [
                                                            {
                                                                "name": "GENERAL_LIMIT_INFO_DATA_1",
                                                                "variables": [
                                                                    {"type": "Int", "value": "0"},
                                                                    {"type": "String", "value": "AAAAADALNb7FPqIACgEoAAYCNHBLgeIyAAAAAXg1TO8TnAAKASgABgI0MH+9OTIAAAABeY8="}
                                                                ],
                                                                "children": []
                                                            }
                                                        ]
                                                    }
                                                ]
                                            }
                                        ]
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        ]
    })
}

// ─── Tests fixture : parse_ctrl_chara_config ─────────────────────────────────

#[test]
fn ctrl_chara_compte_et_chara_id_0() {
    let data = parse_ctrl_chara_config(&fixture_ctrl_chara());
    // fixture : 2 CTRL_CHR_DATA valides
    assert_eq!(data.len(), 2, "fixture: 2 CTRL_CHR_DATA");
    // CTRL_CHR_DATA_0 : var[0] = 1751902334 → 0x686BE87E (ligne 17)
    assert_eq!(data[0].chara_id, CHARA_ID_0, "chara_id[0] = 0x686BE87E");
}

#[test]
fn ctrl_chara_ctrl_type_et_flag_0() {
    let data = parse_ctrl_chara_config(&fixture_ctrl_chara());
    // CTRL_CHR_DATA_0 : var[1]=1 (ligne 21), var[2]=0 (ligne 25)
    assert_eq!(data[0].ctrl_type, 1, "ctrl_type[0] = 1");
    assert_eq!(data[0].flag, 0, "flag[0] = 0");
}

#[test]
fn ctrl_chara_entry_1() {
    let data = parse_ctrl_chara_config(&fixture_ctrl_chara());
    // CTRL_CHR_DATA_1 : var[0]=1128709053 → 0x4346BBBD (ligne 35), var[1]=2 (ligne 39)
    assert_eq!(data[1].chara_id, CHARA_ID_1, "chara_id[1] = 0x4346BBBD");
    assert_eq!(data[1].ctrl_type, 2, "ctrl_type[1] = 2");
}

#[test]
fn ctrl_chara_list_beg_ignore() {
    let data = parse_ctrl_chara_config(&fixture_ctrl_chara());
    // CTRL_CHR_DATA_LIST_BEG_0 ne doit pas être compté
    assert_eq!(data.len(), 2, "LIST_BEG non compté");
}

// ─── Tests fixture : parse_party_departure ────────────────────────────────────

#[test]
fn party_departure_compte_et_entree_0() {
    let data = parse_party_departure(&fixture_party_departure());
    // fixture : 2 PartyDeparture
    assert_eq!(data.len(), 2, "fixture: 2 PartyDeparture");
    // entrée 0 : charaId=0x686BE87E (ligne 9), textureId=0x3CCD6535 (ligne 10)
    assert_eq!(data[0].chara_id, CHARA_ID_0, "chara_id[0] = 0x686BE87E");
    assert_eq!(data[0].texture_id, TEX_ID_0, "texture_id[0] = 0x3CCD6535");
}

#[test]
fn party_departure_entree_1() {
    let data = parse_party_departure(&fixture_party_departure());
    // entrée 1 : charaId=0x4346BBBD (ligne 13), textureId=0x17E036F6 (ligne 14)
    assert_eq!(data[1].chara_id, CHARA_ID_1, "chara_id[1] = 0x4346BBBD");
    assert_eq!(data[1].texture_id, TEX_ID_1, "texture_id[1] = 0x17E036F6");
}

// ─── Tests fixture : parse_specify_party ─────────────────────────────────────

#[test]
fn specify_party_comptes() {
    let cfg = parse_specify_party(&fixture_specify_party());
    // fixture : 2 SpecifyPartyCharaData + 2 SpecifyPartyData
    assert_eq!(
        cfg.chara_list.len(),
        2,
        "chara_list: 2 entrées dans la fixture"
    );
    assert_eq!(
        cfg.party_list.len(),
        2,
        "party_list: 2 entrées dans la fixture"
    );
}

#[test]
fn specify_party_chara_0() {
    let cfg = parse_specify_party(&fixture_specify_party());
    let c: &SpecifyPartyCharaData = &cfg.chara_list[0];
    // m_PartyRefCharaList[0] (ligne 9) : slot=0, type=3, paramId=0xC19FE60C, lv=0
    assert_eq!(c.slot, 0, "slot[0] = 0");
    assert_eq!(c.chara_type, 3, "chara_type[0] = 3");
    assert_eq!(c.param_id, PARAM_ID_0, "param_id[0] = 0xC19FE60C");
    assert_eq!(c.lv, 0, "lv[0] = 0");
}

#[test]
fn specify_party_chara_1_niveau_impose() {
    let cfg = parse_specify_party(&fixture_specify_party());
    let c: &SpecifyPartyCharaData = &cfg.chara_list[1];
    // m_PartyRefCharaList[9] (ligne 92) : slot=0, type=1, paramId=0xBB5FB56C, lv=21
    assert_eq!(c.slot, 0, "slot = 0");
    assert_eq!(c.chara_type, 1, "chara_type = 1 (joueur nommé)");
    assert_eq!(c.param_id, PARAM_ID_9, "param_id = 0xBB5FB56C");
    assert_eq!(c.lv, 21, "lv = 21 (niveau imposé)");
}

#[test]
fn specify_party_list_0() {
    let cfg = parse_specify_party(&fixture_specify_party());
    let p: &SpecifyPartyData = &cfg.party_list[0];
    // m_SpecifyPartyList[0] (ligne 177) : partyId=0xA8052252, chara=[0, 1]
    assert_eq!(p.party_id, PARTY_ID_0, "party_id[0] = 0xA8052252");
    assert_eq!(p.chara_offset, 0, "chara_offset[0] = 0");
    assert_eq!(p.chara_count, 1, "chara_count[0] = 1");
}

#[test]
fn specify_party_list_1() {
    let cfg = parse_specify_party(&fixture_specify_party());
    let p: &SpecifyPartyData = &cfg.party_list[1];
    // m_SpecifyPartyList[3] : partyId=0x51B82450, chara=[4, 5]
    assert_eq!(p.party_id, PARTY_ID_3, "party_id[3] = 0x51B82450");
    assert_eq!(p.chara_offset, 4, "chara_offset[3] = 4");
    assert_eq!(p.chara_count, 5, "chara_count[3] = 5");
}

#[test]
fn specify_party_chara_ref_of() {
    let cfg = parse_specify_party(&fixture_specify_party());
    // party[0] → chara_offset=0, chara_count=1 → 1 membre
    let party = &cfg.party_list[0];
    let membres = cfg.chara_ref_of(party);
    assert_eq!(membres.len(), 1, "chara_ref_of(party[0]) → 1 membre");
    assert_eq!(
        membres[0].param_id, PARAM_ID_0,
        "membre[0].param_id = 0xC19FE60C"
    );
}

#[test]
fn specify_party_find_party() {
    let cfg = parse_specify_party(&fixture_specify_party());
    // find_party(0xA8052252) doit retrouver l'entrée 0
    let found = cfg.find_party(PARTY_ID_0);
    assert!(
        found.is_some(),
        "find_party(0xA8052252) doit trouver party[0]"
    );
    assert_eq!(found.unwrap().chara_count, 1);
    // hash absent → None
    assert!(cfg.find_party(HashId(0xDEADBEEF)).is_none());
}

// ─── Tests fixture : parse_guest_limit_config ────────────────────────────────

#[test]
fn guest_limit_compte_et_rule_hash_0() {
    let data = parse_guest_limit_config(&fixture_guest_limit());
    // fixture : 2 GeneralLimitInfo
    assert_eq!(data.len(), 2, "fixture: 2 GeneralLimitInfo");
    // GENERAL_LIMIT_INFO_0 (ligne 17) : -1346664332 → 0xAFBB8874
    assert_eq!(data[0].rule_hash, RULE_HASH_0, "rule_hash[0] = 0xAFBB8874");
}

#[test]
fn guest_limit_condition_data_0() {
    let data = parse_guest_limit_config(&fixture_guest_limit());
    // GENERAL_LIMIT_INFO_DATA_0 (ligne 39) : condition_data exacte
    assert_eq!(
        data[0].condition_data,
        "AAAAADALNb7FPqIACgEoAAYCNOlC0FgyAAAAAXg1TO8TnAAKASgABgI0MH+9OTIAAAABeY8=",
        "condition_data[0] exacte"
    );
}

#[test]
fn guest_limit_entry_1() {
    let data = parse_guest_limit_config(&fixture_guest_limit());
    // GENERAL_LIMIT_INFO_1 : 1457222339 → 0x56DB72C3
    assert_eq!(data[1].rule_hash, RULE_HASH_1, "rule_hash[1] = 0x56DB72C3");
    assert_eq!(
        data[1].condition_data,
        "AAAAADALNb7FPqIACgEoAAYCNHBLgeIyAAAAAXg1TO8TnAAKASgABgI0MH+9OTIAAAABeY8=",
        "condition_data[1] exacte"
    );
}

// ─── Tests sur fichiers réels (skip silencieux si absents) ────────────────────

const REAL_CTRL_CHARA: &str = "party/ctrl_chara_config_1.04.17.00.cfg.bin.json";
const REAL_DEPARTURE: &str = "party/party_departure_0.00.00.cfg.bin.json";
const REAL_SPECIFY: &str = "party/supecify_party0.00.00.cfg.bin.json";
const REAL_GUEST_LIMIT: &str = "party/guest_limit_config.cfg.bin.json";

fn load_json(path: &str) -> Option<serde_json::Value> {
    let path = common::chemin(path)?;
    if !path.is_file() {
        eprintln!("skip : {} absent du corpus", path.display());
        return None;
    }
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", path.display()));
    Some(
        serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("JSON invalide {}: {e}", path.display())),
    )
}

// — ctrl_chara_config —

#[test]
fn real_file_ctrl_chara_compte() {
    let Some(root) = load_json(REAL_CTRL_CHARA) else {
        return;
    };
    let data = parse_ctrl_chara_config(&root);
    // ctrl_chara_config_1.04.17.00 : CTRL_CHR_DATA_LIST_BEG_0.var[0] = 155
    assert_eq!(data.len(), 155, "ctrl_chara_config : 155 CTRL_CHR_DATA");
}

#[test]
fn real_file_ctrl_chara_entry_0() {
    let Some(root) = load_json(REAL_CTRL_CHARA) else {
        return;
    };
    let data = parse_ctrl_chara_config(&root);
    // CTRL_CHR_DATA_0 (ligne 13) : var[0]=1751902334 → 0x686BE87E, var[1]=1, var[2]=0
    assert_eq!(data[0].chara_id, CHARA_ID_0, "chara_id[0] = 0x686BE87E");
    assert_eq!(data[0].ctrl_type, 1, "ctrl_type[0] = 1");
    assert_eq!(data[0].flag, 0, "flag[0] = 0");
}

#[test]
fn real_file_ctrl_chara_entry_1() {
    let Some(root) = load_json(REAL_CTRL_CHARA) else {
        return;
    };
    let data = parse_ctrl_chara_config(&root);
    // CTRL_CHR_DATA_1 (ligne 31) : var[0]=1128709053 → 0x4346BBBD, var[1]=2
    assert_eq!(data[1].chara_id, CHARA_ID_1, "chara_id[1] = 0x4346BBBD");
    assert_eq!(data[1].ctrl_type, 2, "ctrl_type[1] = 2");
}

#[test]
fn real_file_ctrl_chara_distribution_ctrl_type() {
    let Some(root) = load_json(REAL_CTRL_CHARA) else {
        return;
    };
    let data = parse_ctrl_chara_config(&root);
    let nb_0 = data.iter().filter(|e| e.ctrl_type == 0).count();
    let nb_1 = data.iter().filter(|e| e.ctrl_type == 1).count();
    let nb_2 = data.iter().filter(|e| e.ctrl_type == 2).count();
    // Distribution vérifiée par analyse Python : 0=22, 1=26, 2=107
    assert_eq!(nb_0, 22, "ctrl_type=0 : 22 entrées");
    assert_eq!(nb_1, 26, "ctrl_type=1 : 26 entrées");
    assert_eq!(nb_2, 107, "ctrl_type=2 : 107 entrées");
}

#[test]
fn real_file_ctrl_chara_tous_chara_id_non_nuls() {
    let Some(root) = load_json(REAL_CTRL_CHARA) else {
        return;
    };
    let data = parse_ctrl_chara_config(&root);
    let nuls = data.iter().filter(|e| e.chara_id.is_zero()).count();
    assert_eq!(nuls, 0, "aucun chara_id nul dans ctrl_chara_config");
}

#[test]
fn real_file_ctrl_chara_tous_flag_zero() {
    let Some(root) = load_json(REAL_CTRL_CHARA) else {
        return;
    };
    let data = parse_ctrl_chara_config(&root);
    let non_zero_flag = data.iter().filter(|e| e.flag != 0).count();
    assert_eq!(non_zero_flag, 0, "tous les flag doivent être 0");
}

// — party_departure —

#[test]
fn real_file_party_departure_compte() {
    let Some(root) = load_json(REAL_DEPARTURE) else {
        return;
    };
    let data = parse_party_departure(&root);
    // party_departure_0.00.00 : 7 entrées
    assert_eq!(data.len(), 7, "party_departure : 7 PartyDeparture");
}

#[test]
fn real_file_party_departure_entree_0() {
    let Some(root) = load_json(REAL_DEPARTURE) else {
        return;
    };
    let data = parse_party_departure(&root);
    // entrée 0 (ligne 9) : charaId=0x686BE87E, textureId=0x3CCD6535
    assert_eq!(data[0].chara_id, CHARA_ID_0, "charaId[0] = 0x686BE87E");
    assert_eq!(data[0].texture_id, TEX_ID_0, "textureId[0] = 0x3CCD6535");
}

#[test]
fn real_file_party_departure_entree_1() {
    let Some(root) = load_json(REAL_DEPARTURE) else {
        return;
    };
    let data = parse_party_departure(&root);
    // entrée 1 (ligne 13) : charaId=0x4346BBBD, textureId=0x17E036F6
    assert_eq!(data[1].chara_id, CHARA_ID_1, "charaId[1] = 0x4346BBBD");
    assert_eq!(data[1].texture_id, TEX_ID_1, "textureId[1] = 0x17E036F6");
}

#[test]
fn real_file_party_departure_tous_non_nuls() {
    let Some(root) = load_json(REAL_DEPARTURE) else {
        return;
    };
    let data = parse_party_departure(&root);
    let nuls = data.iter().filter(|e| e.chara_id.is_zero()).count();
    assert_eq!(nuls, 0, "aucun charaId nul dans party_departure");
}

// — supecify_party —

#[test]
fn real_file_specify_party_comptes() {
    let Some(root) = load_json(REAL_SPECIFY) else {
        return;
    };
    let cfg = parse_specify_party(&root);
    // supecify_party0.00.00 : 18 SPECIFY_PARTY_CHARA_DATA + 7 SPECIFY_PARTY_DATA
    assert_eq!(cfg.chara_list.len(), 18, "m_PartyRefCharaList : 18 entrées");
    assert_eq!(cfg.party_list.len(), 7, "m_SpecifyPartyList : 7 entrées");
}

#[test]
fn real_file_specify_party_chara_0() {
    let Some(root) = load_json(REAL_SPECIFY) else {
        return;
    };
    let cfg = parse_specify_party(&root);
    // m_PartyRefCharaList[0] (ligne 9) : slot=0, type=3, paramId=0xC19FE60C, lv=0
    let c = &cfg.chara_list[0];
    assert_eq!(c.slot, 0, "slot[0] = 0");
    assert_eq!(c.chara_type, 3, "chara_type[0] = 3");
    assert_eq!(c.param_id, PARAM_ID_0, "param_id[0] = 0xC19FE60C");
    assert_eq!(c.lv, 0, "lv[0] = 0");
}

#[test]
fn real_file_specify_party_chara_9_niveau_impose() {
    let Some(root) = load_json(REAL_SPECIFY) else {
        return;
    };
    let cfg = parse_specify_party(&root);
    // m_PartyRefCharaList[9] (ligne 92) : paramId=0xBB5FB56C, lv=21
    let c = &cfg.chara_list[9];
    assert_eq!(c.param_id, PARAM_ID_9, "param_id[9] = 0xBB5FB56C");
    assert_eq!(c.lv, 21, "lv[9] = 21 (niveau imposé)");
    assert_eq!(c.chara_type, 1, "chara_type[9] = 1 (joueur nommé)");
}

#[test]
fn real_file_specify_party_list_0() {
    let Some(root) = load_json(REAL_SPECIFY) else {
        return;
    };
    let cfg = parse_specify_party(&root);
    // m_SpecifyPartyList[0] (ligne 177) : partyId=0xA8052252, chara=[0, 1]
    let p = &cfg.party_list[0];
    assert_eq!(p.party_id, PARTY_ID_0, "party_id[0] = 0xA8052252");
    assert_eq!(p.chara_offset, 0, "chara_offset[0] = 0");
    assert_eq!(p.chara_count, 1, "chara_count[0] = 1");
}

#[test]
fn real_file_specify_party_list_3() {
    let Some(root) = load_json(REAL_SPECIFY) else {
        return;
    };
    let cfg = parse_specify_party(&root);
    // m_SpecifyPartyList[3] : partyId=0x51B82450, chara=[4, 5]
    let p = &cfg.party_list[3];
    assert_eq!(p.party_id, PARTY_ID_3, "party_id[3] = 0x51B82450");
    assert_eq!(p.chara_offset, 4, "chara_offset[3] = 4");
    assert_eq!(p.chara_count, 5, "chara_count[3] = 5");
}

#[test]
fn real_file_specify_party_chara_ref_of() {
    let Some(root) = load_json(REAL_SPECIFY) else {
        return;
    };
    let cfg = parse_specify_party(&root);
    // party[0] → chara_offset=0, chara_count=1 → 1 membre
    let p = &cfg.party_list[0];
    let membres = cfg.chara_ref_of(p);
    assert_eq!(membres.len(), 1, "chara_ref_of(party[0]) → 1 membre");
    assert_eq!(membres[0].param_id, PARAM_ID_0);
}

#[test]
fn real_file_specify_party_find_party() {
    let Some(root) = load_json(REAL_SPECIFY) else {
        return;
    };
    let cfg = parse_specify_party(&root);
    let found = cfg.find_party(PARTY_ID_0);
    assert!(found.is_some(), "find_party(0xA8052252)");
    assert_eq!(found.unwrap().chara_count, 1);
}

// — guest_limit_config —

#[test]
fn real_file_guest_limit_compte() {
    let Some(root) = load_json(REAL_GUEST_LIMIT) else {
        return;
    };
    let data = parse_guest_limit_config(&root);
    // guest_limit_config.cfg.bin : GENERAL_LIMIT_INFO_LIST_BEG_0.var[0] = 9
    assert_eq!(data.len(), 9, "guest_limit_config : 9 GeneralLimitInfo");
}

#[test]
fn real_file_guest_limit_rule_hash_0() {
    let Some(root) = load_json(REAL_GUEST_LIMIT) else {
        return;
    };
    let data = parse_guest_limit_config(&root);
    // GENERAL_LIMIT_INFO_0 (ligne 17) : -1346664332 → 0xAFBB8874
    assert_eq!(data[0].rule_hash, RULE_HASH_0, "rule_hash[0] = 0xAFBB8874");
}

#[test]
fn real_file_guest_limit_condition_data_0() {
    let Some(root) = load_json(REAL_GUEST_LIMIT) else {
        return;
    };
    let data = parse_guest_limit_config(&root);
    // GENERAL_LIMIT_INFO_DATA_0 (ligne 39) : condition_data exacte
    assert_eq!(
        data[0].condition_data,
        "AAAAADALNb7FPqIACgEoAAYCNOlC0FgyAAAAAXg1TO8TnAAKASgABgI0MH+9OTIAAAABeY8=",
        "condition_data[0] exacte (ligne 39)"
    );
}

#[test]
fn real_file_guest_limit_rule_hash_1() {
    let Some(root) = load_json(REAL_GUEST_LIMIT) else {
        return;
    };
    let data = parse_guest_limit_config(&root);
    // GENERAL_LIMIT_INFO_1 : 1457222339 → 0x56DB72C3
    assert_eq!(data[1].rule_hash, RULE_HASH_1, "rule_hash[1] = 0x56DB72C3");
}

#[test]
fn real_file_guest_limit_tous_rule_hash_non_nuls() {
    let Some(root) = load_json(REAL_GUEST_LIMIT) else {
        return;
    };
    let data = parse_guest_limit_config(&root);
    let nuls = data.iter().filter(|e| e.rule_hash.is_zero()).count();
    assert_eq!(nuls, 0, "aucun rule_hash nul dans guest_limit_config");
}

#[test]
fn real_file_guest_limit_condition_data_non_vide() {
    let Some(root) = load_json(REAL_GUEST_LIMIT) else {
        return;
    };
    let data = parse_guest_limit_config(&root);
    let vides = data.iter().filter(|e| e.condition_data.is_empty()).count();
    assert_eq!(vides, 0, "aucune condition_data vide");
}
