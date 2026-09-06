#![allow(clippy::pedantic)]
//! Tests golden `enjoy_mode_team` — équipes du **Mode Plaisir** d'IEVR, sur le vrai dump
//! (skip silencieux si absent du VPS — fragment copyright Level-5 non committé) :
//!
//! - `data/common/gamedata/team/enjoy_mode_team_config_1.04.02.00.cfg.bin.json`
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/enjoy-mode-team-config.ts`. Chaque valeur
//! assérée est copiée octet pour octet du dump réel (CRC signés convertis en u32 comme `toHex`).

mod common;

use nie_data::enjoy_mode_team::parse_enjoy_mode_team_config;
use nie_data::hash::HashId;

const PATH: &str = "team/enjoy_mode_team_config_1.04.02.00.cfg.bin.json";

fn load() -> Option<serde_json::Value> {
    let chemin_abs = common::chemin(PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("lecture {}: {e}", chemin_abs.display()));
    Some(
        serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("JSON {}: {e}", chemin_abs.display())),
    )
}

#[test]
fn vingt_huit_equipes() {
    let Some(root) = load() else { return };
    let cfg = parse_enjoy_mode_team_config(&root);
    assert_eq!(
        cfg.teams.len(),
        28,
        "28 équipes dans ENJOY_MODE_TEAM_INFO_LIST_BEG_0"
    );
}

#[test]
fn team0_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_enjoy_mode_team_config(&root);
    let t = &cfg.teams[0];
    assert_eq!(t.team_id, HashId(0x35B8_7230)); // 901280304
    assert_eq!(t.sub_id, HashId(0x3055_CF22)); // 810929954
    assert_eq!(t.color_crc, HashId(0xB222_1B08)); // -1306387704 → u32
    assert_eq!(t.team_type, 1);
    assert_eq!(t.formation_crc, HashId(0x22E9_65A5)); // 585721253
    assert_eq!(t.texture_path, "ev_chronicle_img/ev_bb_s10g001_01.g4tx");
    assert_eq!(t.texture_name, "ev_bb_s10g001_01");
}

#[test]
fn team27_derniere_byte_exact() {
    let Some(root) = load() else { return };
    let cfg = parse_enjoy_mode_team_config(&root);
    let t = &cfg.teams[27];
    assert_eq!(t.team_id, HashId(0xFEB0_25C9)); // -22010423 → u32
    assert_eq!(t.sub_id, HashId(0xCBF7_3DD7)); // -872989225 → u32
    assert_eq!(t.color_crc, HashId(0x6977_DCBF)); // 1769462975
    assert_eq!(t.team_type, 28);
    assert_eq!(t.formation_crc, HashId(0x66F5_43A0)); // 1727349664
    assert_eq!(t.texture_path, "ev_chronicle_img/ev_ch_s06g001_0121.g4tx");
    assert_eq!(t.texture_name, "ev_ch_s06g001_0121");
}

#[test]
fn recherche_par_team_id() {
    let Some(root) = load() else { return };
    let cfg = parse_enjoy_mode_team_config(&root);
    assert!(cfg.find_by_team_id(HashId(0x35B8_7230)).is_some());
    assert!(cfg.find_by_team_id(HashId(0x0000_0000)).is_none());
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_atteint_azalee() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load() else { return };
    assert_eq!(
        family_key("enjoy_mode_team_config_1.04.02.00.cfg.bin"),
        "enjoy_mode_team_config"
    );
    let (label, json) =
        decode_by_key("enjoy_mode_team_config", &root).expect("famille typée câblée");
    assert_eq!(label, "enjoy_mode_team");
    assert_eq!(json["teams"].as_array().map(Vec::len), Some(28));
}
