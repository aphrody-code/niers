#![allow(clippy::pedantic)]
//! Tests golden `record` — valeurs réelles tirées de :
//! `record/record_config.cfg.bin.json`
//!
//! ## Vérifications champ par champ (ligne JSON → assertion)
//!
//! ### RECORD_INFO_0 (premier record, record_no=1)
//! - var\[0\] = -1957215473 → `record_id` = HashId::from_i64(-1957215473)
//! - var\[1\] = -1229922355 → `ref_id` = HashId::from_i64(-1229922355)
//! - var\[2\] = 0 → `flag1` = 0
//! - var\[3\] = 438306617 → `group_id` = HashId(438306617) (0x1A200739)
//! - var\[4\] = 0 → `opt_id1` = HashId::ZERO
//! - var\[5\] = 0 → `opt_id2` = HashId::ZERO
//! - var\[6\] = 0 → `flag2` = 0
//! - var\[7\] = 1 → `record_no` = 1
//! - var\[8\] = Int "0" → `extra_data` = "0"
//!
//! ### RECORD_INFO_7 (record_no=8, avec extra_data base64)
//! - var\[0\] = -225707093 → `record_id` = HashId::from_i64(-225707093)
//! - var\[5\] = -1478521859 → `opt_id2` = HashId::from_i64(-1478521859)
//! - var\[7\] = 8 → `record_no` = 8
//! - var\[8\] = String "AAAAAA8FNcMu49QAAQAyAAAAAXE=" → `extra_data` = "AAAAAA8FNcMu49QAAQAyAAAAAXE="
//!
//! ### RECORD_INFO_30 (dernier record, record_no=34)
//! - var\[7\] = 34 → `record_no` = 34
//! - var\[8\] = String "AAAAAA8FNfmGU3YAAQAyAAAAAXE=" → `extra_data` = "AAAAAA8FNfmGU3YAAQAyAAAAAXE="
//!
//! ### Décompte global
//! - 31 entrées `RECORD_INFO_*` au total (RECORD_INFO_0 à RECORD_INFO_30)
//! - 9 entrées ont un extra_data non-nul (var\[8\] de type String)

mod common;

use nie_data::hash::HashId;
use nie_data::record::{RecordConfig, RecordInfo, parse_record_config};
use serde_json::json;

// ─── Fixture inline ───────────────────────────────────────────────────────────
// Extrait byte-pour-byte du dump réel (deux entrées représentatives).

fn fixture() -> serde_json::Value {
    json!({
        "entries": [{
            "name": "RECORD_INFO_LIST_BEG_0",
            "variables": [{"type": "Int", "value": "2"}],
            "children": [
                // RECORD_INFO_0 — record_no=1, pas d'extra_data
                // record_config.cfg.bin.json lignes 12-51
                {
                    "name": "RECORD_INFO_0",
                    "variables": [
                        {"type": "Int", "value": "-1957215473"},
                        {"type": "Int", "value": "-1229922355"},
                        {"type": "Int", "value": "0"},
                        {"type": "Int", "value": "438306617"},
                        {"type": "Int", "value": "0"},
                        {"type": "Int", "value": "0"},
                        {"type": "Int", "value": "0"},
                        {"type": "Int", "value": "1"},
                        {"type": "Int", "value": "0"}
                    ],
                    "children": []
                },
                // RECORD_INFO_7 — record_no=8, extra_data base64
                // record_config.cfg.bin.json lignes 307-346
                {
                    "name": "RECORD_INFO_7",
                    "variables": [
                        {"type": "Int", "value": "-225707093"},
                        {"type": "Int", "value": "685001326"},
                        {"type": "Int", "value": "0"},
                        {"type": "Int", "value": "438306617"},
                        {"type": "Int", "value": "0"},
                        {"type": "Int", "value": "-1478521859"},
                        {"type": "Int", "value": "0"},
                        {"type": "Int", "value": "8"},
                        {"type": "String", "value": "AAAAAA8FNcMu49QAAQAyAAAAAXE="}
                    ],
                    "children": []
                }
            ]
        }]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn parse_fixture_compte() {
    let cfg = parse_record_config(&fixture());
    // Les deux entrées RECORD_INFO_0 et RECORD_INFO_7 doivent être parsées.
    assert_eq!(cfg.infos.len(), 2, "fixture : 2 entrées valides");
}

#[test]
fn record_info_0_champs_scalaires() {
    let cfg = parse_record_config(&fixture());
    let r: &RecordInfo = &cfg.infos[0];

    // record_config.cfg.bin.json — RECORD_INFO_0, vars 0..8
    assert_eq!(
        r.record_id,
        HashId::from_i64(-1957215473),
        "record_id RECORD_INFO_0"
    );
    assert_eq!(
        r.ref_id,
        HashId::from_i64(-1229922355),
        "ref_id RECORD_INFO_0"
    );
    assert_eq!(r.flag1, 0, "flag1 RECORD_INFO_0");
    assert_eq!(
        r.group_id,
        HashId(438306617),
        "group_id = 0x1A200739 constant"
    );
    assert_eq!(r.opt_id1, HashId::ZERO, "opt_id1 RECORD_INFO_0");
    assert_eq!(r.opt_id2, HashId::ZERO, "opt_id2 RECORD_INFO_0");
    assert_eq!(r.flag2, 0, "flag2 RECORD_INFO_0");
    assert_eq!(r.record_no, 1, "record_no RECORD_INFO_0 = 1");
    assert_eq!(
        r.extra_data, "0",
        "extra_data RECORD_INFO_0 = \"0\" (absent)"
    );
}

#[test]
fn record_info_0_pas_extra_data() {
    let cfg = parse_record_config(&fixture());
    assert!(
        !cfg.infos[0].has_extra_data(),
        "RECORD_INFO_0 n'a pas d'extra_data"
    );
}

#[test]
fn record_info_7_champs_scalaires() {
    let cfg = parse_record_config(&fixture());
    let r: &RecordInfo = &cfg.infos[1];

    // record_config.cfg.bin.json — RECORD_INFO_7, vars 0..8
    assert_eq!(
        r.record_id,
        HashId::from_i64(-225707093),
        "record_id RECORD_INFO_7"
    );
    assert_eq!(
        r.ref_id,
        HashId::from_i64(685001326),
        "ref_id RECORD_INFO_7"
    );
    assert_eq!(
        r.opt_id2,
        HashId::from_i64(-1478521859),
        "opt_id2 RECORD_INFO_7"
    );
    assert_eq!(r.record_no, 8, "record_no RECORD_INFO_7 = 8");
}

#[test]
fn record_info_7_extra_data_base64() {
    let cfg = parse_record_config(&fixture());
    let r = &cfg.infos[1];

    // record_config.cfg.bin.json — RECORD_INFO_7 var[8] = String base64
    assert_eq!(
        r.extra_data, "AAAAAA8FNcMu49QAAQAyAAAAAXE=",
        "extra_data RECORD_INFO_7"
    );
    assert!(r.has_extra_data(), "RECORD_INFO_7 a des extra_data");
}

#[test]
fn find_by_no_fixture() {
    let cfg = parse_record_config(&fixture());
    // Recherche par record_no=1 → RECORD_INFO_0
    let found = cfg.find_by_no(1);
    assert!(found.is_some(), "record_no=1 doit être trouvé");
    assert_eq!(found.unwrap().record_no, 1);

    // record_no=99 inexistant
    assert!(cfg.find_by_no(99).is_none());
}

#[test]
fn find_by_id_fixture() {
    let cfg = parse_record_config(&fixture());
    let id = HashId::from_i64(-1957215473);
    let found = cfg.find_by_id(id);
    assert!(found.is_some(), "record_id RECORD_INFO_0 doit être trouvé");
    assert_eq!(found.unwrap().record_no, 1);
}

#[test]
fn with_extra_data_fixture() {
    let cfg = parse_record_config(&fixture());
    let extras = cfg.with_extra_data();
    // Seul RECORD_INFO_7 a un extra_data non-nul dans la fixture.
    assert_eq!(
        extras.len(),
        1,
        "1 seul record avec extra_data dans la fixture"
    );
    assert_eq!(extras[0].record_no, 8);
}

#[test]
fn liste_beg_ignoree() {
    // Le noeud RECORD_INFO_LIST_BEG doit être ignoré (ne pas être parsé comme RecordInfo).
    let root = json!({
        "entries": [{
            "name": "RECORD_INFO_LIST_BEG_0",
            "variables": [{"type": "Int", "value": "0"}],
            "children": []
        }]
    });
    let cfg = parse_record_config(&root);
    assert_eq!(
        cfg.infos.len(),
        0,
        "RECORD_INFO_LIST_BEG_0 ne doit pas être parsé"
    );
}

#[test]
fn record_id_nul_ignore() {
    // Un noeud avec record_id=0 doit être ignoré silencieusement.
    let root = json!({
        "entries": [{
            "name": "RECORD_INFO_0",
            "variables": [
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"},
                {"type": "Int", "value": "0"}
            ],
            "children": []
        }]
    });
    let cfg = parse_record_config(&root);
    assert_eq!(cfg.infos.len(), 0, "record_id nul → entrée ignorée");
}

// ─── Tests sur le vrai fichier (skip si absent du VPS) ───────────────────────

const REAL_PATH: &str = "record/record_config.cfg.bin.json";

fn load_real() -> Option<RecordConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_record_config(&root))
}

#[test]
fn real_file_compte_total() {
    let Some(cfg) = load_real() else { return };

    // record_config.cfg.bin.json : RECORD_INFO_LIST_BEG_0 var[0] = 31
    // 31 entrées RECORD_INFO_0 à RECORD_INFO_30, toutes avec record_id non-nul.
    assert_eq!(cfg.infos.len(), 31, "31 records dans le dump réel");
}

#[test]
fn real_file_record_info_0_champs() {
    let Some(cfg) = load_real() else { return };

    // record_config.cfg.bin.json — RECORD_INFO_0, lignes 12-51
    let r0 = &cfg.infos[0];
    assert_eq!(
        r0.record_id,
        HashId::from_i64(-1957215473),
        "RECORD_INFO_0 var[0] = -1957215473"
    );
    assert_eq!(
        r0.ref_id,
        HashId::from_i64(-1229922355),
        "RECORD_INFO_0 var[1] = -1229922355"
    );
    assert_eq!(r0.flag1, 0, "RECORD_INFO_0 var[2] = 0");
    assert_eq!(
        r0.group_id,
        HashId(438_306_617),
        "RECORD_INFO_0 var[3] = 438306617 (0x1A200739)"
    );
    assert_eq!(r0.opt_id1, HashId::ZERO, "RECORD_INFO_0 var[4] = 0");
    assert_eq!(r0.opt_id2, HashId::ZERO, "RECORD_INFO_0 var[5] = 0");
    assert_eq!(r0.flag2, 0, "RECORD_INFO_0 var[6] = 0");
    assert_eq!(r0.record_no, 1, "RECORD_INFO_0 var[7] = 1");
    assert_eq!(r0.extra_data, "0", "RECORD_INFO_0 var[8] = \"0\"");
}

#[test]
fn real_file_group_id_constant() {
    let Some(cfg) = load_real() else { return };

    // var[3] = 438306617 (0x1A200739) dans les 31 entrées du dump réel.
    let constante = HashId(438_306_617);
    for (i, r) in cfg.infos.iter().enumerate() {
        assert_eq!(
            r.group_id, constante,
            "RECORD_INFO_{i} group_id doit être 0x1A200739"
        );
    }
}

#[test]
fn real_file_record_no_non_continu() {
    let Some(cfg) = load_real() else { return };

    // record_no attendu : 1..8, 10..22, 24..32, 34 (trous à 9, 23, 33)
    // record_config.cfg.bin.json — vérification des extrêmes.
    assert_eq!(cfg.infos[0].record_no, 1, "premier record_no = 1");
    assert_eq!(cfg.infos[7].record_no, 8, "RECORD_INFO_7 record_no = 8");
    // RECORD_INFO_8 a record_no=10 (pas 9 — 9 est absent du dump)
    assert_eq!(
        cfg.infos[8].record_no, 10,
        "RECORD_INFO_8 record_no = 10 (pas 9)"
    );
    assert_eq!(cfg.infos[30].record_no, 34, "dernier record_no = 34");
}

#[test]
fn real_file_record_info_7_extra_data() {
    let Some(cfg) = load_real() else { return };

    // record_config.cfg.bin.json — RECORD_INFO_7, var[8] = String base64
    let r7 = &cfg.infos[7];
    assert_eq!(r7.record_no, 8, "RECORD_INFO_7 record_no = 8");
    assert_eq!(
        r7.extra_data, "AAAAAA8FNcMu49QAAQAyAAAAAXE=",
        "RECORD_INFO_7 var[8] = base64"
    );
    assert!(r7.has_extra_data(), "RECORD_INFO_7 a des extra_data");
}

#[test]
fn real_file_record_info_30_dernier() {
    let Some(cfg) = load_real() else { return };

    // record_config.cfg.bin.json — RECORD_INFO_30, lignes 1272-1313
    let r30 = &cfg.infos[30];
    assert_eq!(
        r30.record_no, 34,
        "RECORD_INFO_30 var[7] = 34 (dernier record)"
    );
    assert_eq!(
        r30.extra_data, "AAAAAA8FNfmGU3YAAQAyAAAAAXE=",
        "RECORD_INFO_30 var[8] = base64"
    );
}

#[test]
fn real_file_extra_data_count() {
    let Some(cfg) = load_real() else { return };

    // 9 entrées sur 31 ont un extra_data non-nul dans le dump réel
    // (RECORD_INFO_7, 8, 15, 21, 22, 23, 24, 25, 30)
    let count = cfg.with_extra_data().len();
    assert_eq!(count, 9, "9 records ont des extra_data dans le dump réel");
}

#[test]
fn real_file_find_by_no_record1() {
    let Some(cfg) = load_real() else { return };

    // Recherche par record_no = 1 → RECORD_INFO_0
    let found = cfg.find_by_no(1);
    assert!(found.is_some(), "record_no=1 doit exister");
    assert_eq!(
        found.unwrap().record_id,
        HashId::from_i64(-1957215473),
        "record_no=1 → record_id = RECORD_INFO_0"
    );
}

#[test]
fn real_file_find_by_no_absent() {
    let Some(cfg) = load_real() else { return };

    // record_no = 9 est absent du dump (trou dans la séquence)
    assert!(
        cfg.find_by_no(9).is_none(),
        "record_no=9 absent du dump (trou)"
    );
    assert!(
        cfg.find_by_no(23).is_none(),
        "record_no=23 absent du dump (trou)"
    );
    assert!(
        cfg.find_by_no(33).is_none(),
        "record_no=33 absent du dump (trou)"
    );
}
