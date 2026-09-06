#![allow(clippy::pedantic)]
//! Tests golden `event_subtitle` — port 1:1 d'inagle `packages/inagle/src/parsers/event-subtitles.ts`
//! (parseurs par-fichier `parseSubtitleFile` / `loadTextMap` / `loadWashaMap`).
//!
//! Vérité terrain = l'event voicé réel `ev09_05000` (VFS IEVR), via
//! `cargo run -p nie-game --example extract_event_subtitle` : 22 lignes Subtitle (ja), 22 washa,
//! 22 dialogues (fr) — **jointure hash 22/22**. Ligne 0 : hash `0x6540D3B6`, start `1.1666667`,
//! end `6.9166665`, washa.label `"ev09_05000_010_010"`, dialogue fr brut commençant par
//! « Les champions en titre sont tombés !\n<MNT:NAGUMOHARA>… » (tags **conservés**).

mod common;

use nie_data::event_subtitle::{
    find_event_text, parse_event_text, parse_subtitle_file, parse_washa_map,
};
use nie_data::hash::HashId;
use serde_json::{Value, json};

fn ivar(v: i64) -> Value {
    json!({ "type": "Int", "value": v.to_string() })
}
fn fvar(v: &str) -> Value {
    json!({ "type": "Float", "value": v })
}
fn svar(v: &str) -> Value {
    json!({ "type": "String", "value": v })
}

const H: i64 = 0x6540_D3B6;

#[test]
fn subtitle_timings() {
    let root = json!({ "entries": [{
        "name": "EV_SUBTITLE_DATA_LIST_BEG_0", "variables": [ivar(22)],
        "children": [{
            "name": "EV_SUBTITLE_DATA_0",
            "variables": [ivar(H), fvar("1.1666667"), fvar("6.9166665"), fvar("0"), fvar("0")],
            "children": []
        }]
    }]});
    let rows = parse_subtitle_file(&root);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text_hash, HashId(0x6540_D3B6));
    assert!((rows[0].show_start - 1.1666667).abs() < 1e-6);
    assert!((rows[0].show_end - 6.9166665).abs() < 1e-6);
    // < 5 variables → ignoré.
    let short = json!({ "entries": [{
        "name": "EV_SUBTITLE_DATA_LIST_BEG_0", "variables": [ivar(1)],
        "children": [{ "name": "EV_SUBTITLE_DATA_0",
            "variables": [ivar(1), fvar("0"), fvar("0")], "children": [] }]
    }]});
    assert!(parse_subtitle_file(&short).is_empty());
}

#[test]
fn washa_label_and_lip() {
    // 16 variables : hash@0, lip@5, label@15.
    let mut vars: Vec<Value> = (0..16).map(|_| svar("")).collect();
    vars[0] = ivar(H);
    vars[5] = svar("no_lip");
    vars[15] = svar("ev09_05000_010_010");
    let root = json!({ "entries": [{
        "name": "TEXT_WASHA_MAP_BEGIN_0", "variables": [ivar(22)],
        "children": [{ "name": "TEXT_WASHA_MAP_0", "variables": vars, "children": [] }]
    }]});
    let entries = parse_washa_map(&root);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].text_hash, HashId(0x6540_D3B6));
    assert_eq!(entries[0].lip_sync.as_deref(), Some("no_lip"));
    assert_eq!(entries[0].label.as_deref(), Some("ev09_05000_010_010"));

    // Champs vides → None ; < 16 variables → ignoré.
    let mut v2: Vec<Value> = (0..16).map(|_| svar("")).collect();
    v2[0] = ivar(7);
    let root2 = json!({ "entries": [{
        "name": "TEXT_WASHA_MAP_BEGIN_0", "variables": [ivar(1)],
        "children": [{ "name": "TEXT_WASHA_MAP_0", "variables": v2, "children": [] }]
    }]});
    let e2 = parse_washa_map(&root2);
    assert_eq!(e2[0].lip_sync, None);
    assert_eq!(e2[0].label, None);
}

#[test]
fn dialogue_raw_text_preserved_and_join() {
    let raw = "Les champions en titre sont tombés !\\n<MNT:NAGUMOHARA> récupère le ballon.";
    let root = json!({ "entries": [{
        "name": "TEXT_INFO_BEGIN_0", "variables": [ivar(22)],
        "children": [{ "name": "TEXT_INFO_0",
            "variables": [ivar(H), ivar(0), svar(raw)], "children": [] }]
    }]});
    let lines = parse_event_text(&root);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text_hash, HashId(0x6540_D3B6));
    // Texte BRUT : les tags `<MNT:…>` et `\n` littéral sont conservés (pas de sanitize).
    assert_eq!(lines[0].raw_text, raw);
    assert!(lines[0].raw_text.contains("<MNT:NAGUMOHARA>"));

    // Jointure par hash (la clé partagée subtitle ↔ washa ↔ dialogue).
    assert_eq!(find_event_text(&lines, HashId(0x6540_D3B6)), Some(raw));
    assert_eq!(find_event_text(&lines, HashId(0xDEAD_BEEF)), None);
}

// ─── Fichiers Subtitle_ev réels + dispatch typé par préfixe (2026-06-23) ─────────
// ~1321 fichiers `Subtitle_ev<NN>_<bloc>` du mode Histoire deviennent décodables typé
// via le dispatch par préfixe `Subtitle_ev` (clé par-événement → parse_subtitle_file).
const SUB_PATH: &str = "event/subtitle/pt/Subtitle_ev01_04800.cfg.bin.json";

fn load_sub() -> Option<Value> {
    let chemin_abs = common::chemin(SUB_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let c = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("lecture {}: {e}", chemin_abs.display()));
    Some(serde_json::from_str(&c).unwrap_or_else(|e| panic!("JSON {}: {e}", chemin_abs.display())))
}

#[test]
fn real_subtitle_file_six_lignes_byte_exact() {
    let Some(root) = load_sub() else { return };
    let rows = parse_subtitle_file(&root);
    assert_eq!(rows.len(), 6, "Subtitle_ev01_04800 = 6 lignes timecodées");
    // Hashes byte-exact (clés de jointure vers le texte localisé).
    let hashes: [u32; 6] = [
        0x8BAF_B21B,
        0xA082_E1D8,
        0xB999_D099,
        0xF6D8_465E,
        0xEFC3_771F,
        0xC4EE_24DC,
    ];
    for (i, h) in hashes.iter().enumerate() {
        assert_eq!(rows[i].text_hash, HashId(*h), "hash ligne {i}");
    }
    // Timings de la 1re et dernière ligne (vérité terrain).
    assert_eq!(rows[0].show_start, "24.583334".parse::<f64>().unwrap());
    assert_eq!(rows[0].show_end, "29.083334".parse::<f64>().unwrap());
    assert_eq!(rows[5].show_start, "50.75".parse::<f64>().unwrap());
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_par_prefixe_atteint_azalee() {
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load_sub() else { return };
    // family_key strippe le suffixe `_04800` (vu comme une version) → clé par-événement.
    let key = family_key("Subtitle_ev01_04800.cfg.bin");
    assert!(key.starts_with("Subtitle_ev"), "clé = {key}");
    let (label, jsonv) = decode_by_key(&key, &root).expect("dispatch par préfixe câblé");
    assert_eq!(label, "event_subtitle");
    assert_eq!(jsonv.as_array().map(Vec::len), Some(6));
}
