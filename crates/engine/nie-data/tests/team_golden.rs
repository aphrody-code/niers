#![allow(clippy::pedantic)]
//! Tests golden `team` (enjoy_mode_team) — noeuds réels `ENJOY_MODE_TEAM_INFO_*` tirés de :
//! `data/common/gamedata/team/enjoy_mode_team_config_1.04.02.00.cfg.bin` (VFS IEVR, format T2B).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/enjoy-mode-team-config.ts` (l.52-90).
//! Vérité terrain = la sortie d'inagle `packages/inagle/src/entries/enjoy_mode_teams.json`
//! (28 entrées). Les valeurs hex ci-dessous ont été vérifiées byte-exact contre ce fichier.

use nie_data::hash::HashId;
use nie_data::team::{EnjoyModeTeam, parse_enjoy_mode_team_config};
use serde_json::{Value, json};

/// Une variable CfgBin entière (les CRC/Int du dump).
fn vi(n: i64) -> Value {
    json!({ "type": "Int", "value": n.to_string() })
}

/// Une variable CfgBin chaîne (texturePath/textureName, et le blob base64 du var7).
fn vs(s: &str) -> Value {
    json!({ "type": "String", "value": s })
}

/// Construit le noeud racine `{entries:[LIST_BEG{children:[ENJOY_MODE_TEAM_INFO_x ...]}]}`
/// à partir des 12 variables réelles (mêmes valeurs/types que le dump VFS).
fn node_fixture(children: Vec<Value>) -> Value {
    json!({
        "entries": [{
            "name": "ENJOY_MODE_TEAM_INFO_LIST_BEG_0",
            "variables": [vi(children.len() as i64)],
            "children": children
        }]
    })
}

/// 6 entrées réelles (indices 0,1,2,5,13,27 du dump), 12 vars chacune. Les couvre :
/// CRC positifs/négatifs, types variés, et le blob base64 au var7 (entrée ev_bb_b10g004_01
/// et ev_ch_s06g001_0121) qu'inagle n'expose pas.
fn child(idx: usize, vars: [Value; 12]) -> Value {
    json!({
        "name": format!("ENJOY_MODE_TEAM_INFO_{idx}"),
        "variables": vars.to_vec(),
        "children": []
    })
}

fn real_children() -> Vec<Value> {
    vec![
        // ev_bb_s10g001_01
        child(
            0,
            [
                vi(901280304),
                vi(810929954),
                vi(-1306387704),
                vi(1),
                vi(585721253),
                vs("ev_chronicle_img/ev_bb_s10g001_01.g4tx"),
                vs("ev_bb_s10g001_01"),
                vi(0),
                vi(1813933277),
                vi(0),
                vi(0),
                vi(-1),
            ],
        ),
        // ev_bb_s11g001_01
        child(
            1,
            [
                vi(1424638790),
                vi(-1641831799),
                vi(-987419746),
                vi(3),
                vi(529091605),
                vs("ev_chronicle_img/ev_bb_s11g001_01.g4tx"),
                vs("ev_bb_s11g001_01"),
                vi(0),
                vi(1550757894),
                vi(0),
                vi(0),
                vi(-1),
            ],
        ),
        // ev_bb_s33g001_01
        child(
            2,
            [
                vi(301450369),
                vi(721961018),
                vi(724257458),
                vi(2),
                vi(679591550),
                vs("ev_chronicle_img/ev_bb_s33g001_01.g4tx"),
                vs("ev_bb_s33g001_01"),
                vi(0),
                vi(141732313),
                vi(0),
                vi(0),
                vi(-1),
            ],
        ),
        // ev_bb_s52g001_01 (teamId négatif)
        child(
            5,
            [
                vi(-1814806523),
                vi(-1069206060),
                vi(-1253056751),
                vi(5),
                vi(-1011321645),
                vs("ev_chronicle_img/ev_bb_s52g001_01.g4tx"),
                vs("ev_bb_s52g001_01"),
                vi(0),
                vi(2000937925),
                vi(0),
                vi(0),
                vi(-1),
            ],
        ),
        // ev_bb_b10g004_01 (var7 = blob base64, var11 = 30000)
        child(
            13,
            [
                vi(1668478357),
                vi(-1046485492),
                vi(1161251685),
                vi(14),
                vi(-1825962416),
                vs("ev_chronicle_img/ev_bb_b10g004_01.g4tx"),
                vs("ev_bb_b10g004_01"),
                vs("AAAAABgFNSo9RUMACgEoAAYCNCy4aiUyAAAAAXg="),
                vi(1748276314),
                vi(-782511925),
                vi(-1390186696),
                vi(30000),
            ],
        ),
        // ev_ch_s06g001_0121 (var7 = blob base64, type 28)
        child(
            27,
            [
                vi(-22010423),
                vi(-872989225),
                vi(1769462975),
                vi(28),
                vi(1727349664),
                vs("ev_chronicle_img/ev_ch_s06g001_0121.g4tx"),
                vs("ev_ch_s06g001_0121"),
                vs("AAAAACEFNcl4Pb8AEwIoAAYCNJAYVCcoAAYCMgAAAAIyAAAAAXg="),
                vi(1121693102),
                vi(730233931),
                vi(1425458077),
                vi(-1),
            ],
        ),
    ]
}

#[test]
fn enjoy_mode_team_compte_et_premiere_entree() {
    let teams = parse_enjoy_mode_team_config(&node_fixture(real_children()));
    assert_eq!(teams.len(), 6, "6 entrées dans la fixture");

    let t: &EnjoyModeTeam = &teams[0];
    // Valeurs golden = sortie inagle enjoy_mode_teams.json[0].
    assert_eq!(t.team_id, HashId(0x35B8_7230));
    assert_eq!(t.team_id, HashId::from_signed(901280304));
    assert_eq!(t.sub_id, HashId(0x3055_CF22));
    assert_eq!(t.color_crc, HashId(0xB222_1B08));
    assert_eq!(t.team_type, 1);
    assert_eq!(t.formation_crc, HashId(0x22E9_65A5));
    assert_eq!(t.texture_path, "ev_chronicle_img/ev_bb_s10g001_01.g4tx");
    assert_eq!(t.texture_name, "ev_bb_s10g001_01");
}

#[test]
fn enjoy_mode_team_crc_negatifs_signe_vers_non_signe() {
    let teams = parse_enjoy_mode_team_config(&node_fixture(real_children()));
    // Entrée ev_bb_s52g001_01 : teamId -1814806523 >>> 0 = 0x93D44005 (signé→non-signé inagle).
    let t = &teams[3];
    assert_eq!(t.team_id, HashId(0x93D4_4005));
    assert_eq!(t.sub_id, HashId(0xC045_35D4));
    assert_eq!(t.color_crc, HashId(0xB54F_DF11));
    assert_eq!(t.team_type, 5);
    assert_eq!(t.formation_crc, HashId(0xC3B8_74D3));
    assert_eq!(t.texture_name, "ev_bb_s52g001_01");
    // Format hex identique au toHex inagle.
    assert_eq!(t.team_id.to_hex(), "0x93D44005");
}

#[test]
fn enjoy_mode_team_var7_blob_ignore_et_champs_corrects() {
    // Les entrées avec un blob base64 au var7 (ev_bb_b10g004_01, ev_ch_s06g001_0121)
    // se parsent normalement : seules les vars 0..6 sont exposées (1:1 inagle).
    let teams = parse_enjoy_mode_team_config(&node_fixture(real_children()));

    let b10 = &teams[4]; // ev_bb_b10g004_01
    assert_eq!(b10.team_id, HashId(0x6372_F595));
    assert_eq!(b10.sub_id, HashId(0xC19F_E60C));
    assert_eq!(b10.color_crc, HashId(0x4537_4B65));
    assert_eq!(b10.team_type, 14);
    assert_eq!(b10.formation_crc, HashId(0x932A_0650));
    assert_eq!(b10.texture_path, "ev_chronicle_img/ev_bb_b10g004_01.g4tx");
    assert_eq!(b10.texture_name, "ev_bb_b10g004_01");

    let ch06 = &teams[5]; // ev_ch_s06g001_0121, type 28 (dernier type du dump)
    assert_eq!(ch06.team_id, HashId(0xFEB0_25C9));
    assert_eq!(ch06.sub_id, HashId(0xCBF7_3DD7));
    assert_eq!(ch06.color_crc, HashId(0x6977_DCBF));
    assert_eq!(ch06.team_type, 28);
    assert_eq!(ch06.formation_crc, HashId(0x66F5_43A0));
    assert_eq!(ch06.texture_name, "ev_ch_s06g001_0121");
}

#[test]
fn enjoy_mode_team_entrees_1_et_2() {
    let teams = parse_enjoy_mode_team_config(&node_fixture(real_children()));

    let t1 = &teams[1]; // ev_bb_s11g001_01
    assert_eq!(t1.team_id, HashId(0x54EA_4346));
    assert_eq!(t1.sub_id, HashId(0x9E23_A289));
    assert_eq!(t1.color_crc, HashId(0xC525_2B9E));
    assert_eq!(t1.team_type, 3);
    assert_eq!(t1.formation_crc, HashId(0x1F89_4C15));

    let t2 = &teams[2]; // ev_bb_s33g001_01
    assert_eq!(t2.team_id, HashId(0x11F7_C481));
    assert_eq!(t2.sub_id, HashId(0x2B08_403A));
    assert_eq!(t2.color_crc, HashId(0x2B2B_4AB2));
    assert_eq!(t2.team_type, 2);
    assert_eq!(t2.formation_crc, HashId(0x2881_BE7E));
}

#[test]
fn enjoy_mode_team_liste_vide_si_pas_de_noeud() {
    // Garde : un root sans liste ENJOY_MODE_TEAM_INFO_LIST_BEG produit une liste vide.
    let empty = json!({ "entries": [] });
    assert!(parse_enjoy_mode_team_config(&empty).is_empty());

    // Une entrée sans la bonne enveloppe LIST_BEG est ignorée (pas de faux positif).
    let stray = json!({
        "entries": [{
            "name": "SOMETHING_ELSE",
            "variables": [],
            "children": [{ "name": "ENJOY_MODE_TEAM_INFO_0", "variables": [], "children": [] }]
        }]
    });
    assert!(parse_enjoy_mode_team_config(&stray).is_empty());
}
