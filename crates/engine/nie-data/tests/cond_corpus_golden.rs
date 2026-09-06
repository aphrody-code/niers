#![allow(clippy::pedantic)]
//! Golden `cond` — validation du **cadrage** des blobs de condition contre le **corpus réel
//! entier** du jeu (skip silencieux si `data/` absent). C'est la vérité terrain du format : si
//! TOUS les blobs version-0 réels satisfont `declared_len == payload.len()+1`, le cadrage est
//! prouvé (pas deviné). Sémantique des clauses = hors scope (exige l'évaluateur, cf. `cond.rs`).

mod common;

use nie_data::cond::CondBlob;

/// Décode base64 (alphabet standard, padding optionnel) → octets. `None` si invalide.
fn b64(s: &str) -> Option<Vec<u8>> {
    const INV: u8 = 0xFF;
    let mut table = [INV; 256];
    for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .iter()
        .enumerate()
    {
        table[*c as usize] = i as u8;
    }
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0;
    for &c in s.as_bytes() {
        if c == b'=' {
            break;
        }
        let v = table[c as usize];
        if v == INV {
            return None;
        }
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// Décode `s` (base64) et pousse si ≥ 6 octets (taille minimale d'un en-tête cond).
fn push_blob(s: &str, out: &mut Vec<Vec<u8>>) {
    if let Some(b) = b64(s)
        && b.len() >= 6
    {
        out.push(b);
    }
}

/// Collecte récursivement toutes les chaînes base64 « cond » (champs connus + variables String
/// commençant par `AAAA` = en-tête version 0/1) d'un `serde_json::Value`.
fn collect(v: &serde_json::Value, out: &mut Vec<Vec<u8>>) {
    const FIELDS: [&str; 5] = ["cond", "openCond", "condition", "runCond", "aocCondition"];
    match v {
        serde_json::Value::Object(m) => {
            // Variable T2B : { "type": "String", "value": "AAAA..." }
            if m.get("type").and_then(|t| t.as_str()) == Some("String")
                && let Some(s) = m.get("value").and_then(|s| s.as_str())
                && s.len() >= 12
                && s.starts_with("AAAA")
            {
                push_blob(s, out);
            }
            for (k, vv) in m {
                if FIELDS.contains(&k.as_str())
                    && let Some(s) = vv.as_str()
                    && s.len() >= 8
                {
                    push_blob(s, out);
                }
                collect(vv, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect(x, out)),
        _ => {}
    }
}

fn walk_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>, cap: usize) {
    if files.len() >= cap {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_files(&p, files, cap);
        } else if p.to_string_lossy().ends_with(".cfg.bin.json") {
            files.push(p);
            if files.len() >= cap {
                return;
            }
        }
    }
}

#[test]
fn cadrage_version0_prouve_sur_corpus_reel() {
    let Some(root) = common::racine() else {
        eprintln!("skip : corpus gamedata introuvable — poser NIE_GAMEDATA_JSON ou NIE_GAME_DIR");
        return;
    };
    if !root.exists() {
        return;
    }
    let mut files = Vec::new();
    walk_files(root, &mut files, 3000);
    if files.is_empty() {
        // La racine existe mais ne porte aucun dump : c'est le cas quand `NIE_GAME_DIR` désigne
        // l'installation Steam, dont `gamedata/` contient les `.cfg.bin` **binaires** et non les
        // `.cfg.bin.json`. Annoncer le saut plutôt que d'échouer sur « eu 0 » — un rouge
        // d'environnement se lit comme une régression.
        eprintln!(
            "skip : {} ne contient aucun *.cfg.bin.json (corpus de dumps absent)",
            root.display()
        );
        return;
    }
    let mut blobs = Vec::new();
    for f in &files {
        if let Ok(txt) = std::fs::read_to_string(f)
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt)
        {
            collect(&v, &mut blobs);
        }
    }
    assert!(
        blobs.len() > 1000,
        "corpus de blobs cond conséquent (eu {})",
        blobs.len()
    );

    let mut v0 = 0usize;
    let mut v0_bad = 0usize;
    let mut versions_other = 0usize;
    for b in &blobs {
        let c = CondBlob::parse(b).expect("≥6 octets garanti");
        match c.version {
            0 => {
                v0 += 1;
                if !c.framing_valid_v0() {
                    v0_bad += 1;
                }
            }
            1 => versions_other += 1,
            _ => panic!("version inattendue {} (cadrage non 0/1)", c.version),
        }
    }
    // Cadrage version-0 prouvé : AUCUN blob version-0 ne viole declared_len == len-5.
    assert_eq!(
        v0_bad, 0,
        "{v0_bad} blobs version-0 violent le cadrage (sur {v0})"
    );
    assert!(v0 > 1000, "majorité de blobs version-0 (eu {v0})");
    eprintln!(
        "cond corpus: {} blobs | v0={v0} (cadrage OK) | v1(liste)={versions_other}",
        blobs.len()
    );

    // ── Décodage SÉMANTIQUE complet sur tout le corpus (unlock_condition, port d'inagle) ──────
    // Prouve que le décodeur de clauses (tokens 0x35/0x34/0x32, namespaces story/event-flag,
    // opcodes single/AND/trivial) tient sur les 17 k+ blobs RÉELS, pas seulement les fixtures.
    use nie_data::unlock_condition::{
        UnlockType, decode_unlock_condition_bytes, story_threshold_to_episode,
    };
    let (mut always, mut story, mut eventflag, mut composite) = (0u32, 0u32, 0u32, 0u32);
    let mut story_on_grid = 0u32;
    let mut total_events = 0u32;
    for b in &blobs {
        let c = decode_unlock_condition_bytes(b, String::new());
        match c.kind {
            UnlockType::Always => always += 1,
            UnlockType::Story => story += 1,
            UnlockType::EventFlag => eventflag += 1,
            UnlockType::Composite => composite += 1,
        }
        if let Some(th) = c.story_threshold
            && story_threshold_to_episode(th).is_some()
        {
            story_on_grid += 1;
        }
        total_events += u32::try_from(c.required_events.len()).unwrap_or(0);
    }
    // Le décodeur trouve de VRAIES conditions (pas tout trivial) : story + event-flags présents.
    assert!(
        story + composite > 100,
        "seuils de progression story décodés (eu {})",
        story + composite
    );
    assert!(
        eventflag + composite > 100,
        "event-flags décodés (eu {})",
        eventflag + composite
    );
    assert!(
        total_events > 1000,
        "feuilles event-flag décodées (eu {total_events})"
    );
    eprintln!(
        "unlock_condition corpus: always={always} story={story} eventFlag={eventflag} composite={composite} | story alignés grille={story_on_grid} | feuilles event={total_events}"
    );
}
