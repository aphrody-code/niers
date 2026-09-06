#![allow(clippy::pedantic)]
//! Golden `phase_set` — setup de phases de match (~182 fichiers `*_phase_set`), sur de vrais dumps.
mod common;

use nie_data::phase_set::parse_phase_set;

fn load(rel: &str) -> Option<serde_json::Value> {
    let p = rel.to_string();
    let p = common::chemin(&p)?;
    if !p.is_file() {
        eprintln!("skip : {} absent du corpus", p.display());
        return None;
    }
    let c = std::fs::read_to_string(&p).unwrap();
    Some(serde_json::from_str(&c).unwrap())
}

#[test]
fn fbtl_cro_phase_set_conditions() {
    let Some(root) = load("soccer/game/fbtl_cro08_030_010_phase_set_0.00.00.cfg.bin.json") else {
        return;
    };
    let cfg = parse_phase_set(&root);
    assert_eq!(cfg.items.len() as i64, cfg.count);
    assert!(!cfg.items.is_empty());
    // forme fbtl_cro : (Int, Int, condition) — 2 ints + 1 condition par item.
    let it0 = &cfg.items[0];
    assert_eq!(it0.ints.len(), 2);
    assert_eq!(it0.conditions.len(), 1, "var[2] = blob condition décodé");
    // le timing (var[1]) progresse (10, 20, 30…) sur les vrais items.
    assert_eq!(cfg.items[0].ints[1], 10);
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_suffixe() {
    // Importé ici et non en tête : ce test est le seul consommateur, et il est gaté par
    // `serde`. En tête, l'import paraît inutilisé quand la feature est absente — clippy le
    // signale, et le retirer casse le build `--features serde`.
    use nie_data::typed::{decode_by_key, family_key};
    use nie_data::unlock_condition::UnlockType;
    let Some(root) = load("soccer/game/fbtl_cro08_030_010_phase_set_0.00.00.cfg.bin.json") else {
        return;
    };
    let key = family_key("fbtl_cro08_030_010_phase_set_0.00.00.cfg.bin");
    assert!(key.ends_with("_phase_set"), "clé={key}");
    let (label, json) = decode_by_key(&key, &root).expect("dispatch câblé");
    assert_eq!(label, "phase_set");
    assert!(json["items"].as_array().is_some_and(|a| !a.is_empty()));
    let _ = UnlockType::Always;
}
