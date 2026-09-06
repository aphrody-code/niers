#![allow(clippy::pedantic)]
//! Couverture du dispatch typé sur le corpus réel — combien de `.cfg.bin` du jeu reçoivent des
//! champs nommés plutôt que des colonnes numérotées.
//!
//! `nie_data::typed::decode_by_key` est ce qui sépare « var3 = 1852 » de « `consume_tp` = 1852 ».
//! Ce test mesure sa portée sur le corpus de dumps, et vérifie que chaque famille qui répond
//! rend bien une valeur exploitable — pas un objet vide.

mod common;

use std::collections::{BTreeMap, BTreeSet};

/// Familles dont on exige qu'elles répondent : ce sont celles que l'interface expose.
const ATTENDUES: [&str; 8] = [
    "skill_config",
    "item_config",
    "formation_config",
    "trophy_config",
    "gallery_config",
    "record_config",
    "dictionary_config",
    "mission_config",
];

#[test]
fn le_dispatch_type_couvre_les_familles_attendues() {
    let Some(racine) = common::chemin("") else {
        return;
    };
    if !racine.is_dir() {
        eprintln!("skip typed_corpus : {} absent", racine.display());
        return;
    }

    // Un fichier par famille suffit : le dispatch se fait sur la clé, pas sur le contenu.
    let mut par_famille: BTreeMap<String, std::path::PathBuf> = BTreeMap::new();
    let mut pile = vec![racine.clone()];
    while let Some(dir) = pile.pop() {
        let Ok(entrees) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entrees.flatten() {
            let p = e.path();
            if p.is_dir() {
                pile.push(p);
            } else if p.to_string_lossy().ends_with(".cfg.bin.json") {
                let cle = nie_data::typed::family_key(&p.to_string_lossy());
                par_famille.entry(cle).or_insert(p);
            }
        }
    }
    assert!(
        !par_famille.is_empty(),
        "corpus présent mais aucun .cfg.bin.json trouvé"
    );

    let (mut typees, mut generiques) = (0usize, 0usize);
    let mut labels: BTreeSet<&'static str> = BTreeSet::new();
    let mut repondues: BTreeSet<String> = BTreeSet::new();

    for (cle, chemin) in &par_famille {
        let Ok(texte) = std::fs::read_to_string(chemin) else {
            continue;
        };
        let Ok(root) = serde_json::from_str::<serde_json::Value>(&texte) else {
            continue;
        };
        match nie_data::typed::decode_by_key(cle, &root) {
            Some((label, valeur)) => {
                typees += 1;
                labels.insert(label);
                repondues.insert(cle.clone());
                assert!(
                    !valeur.is_null(),
                    "{cle} → {label} : le parseur a répondu null sur {}",
                    chemin.display()
                );
            }
            None => generiques += 1,
        }
    }

    let total = typees + generiques;
    #[allow(clippy::cast_precision_loss)]
    let taux = typees as f64 * 100.0 / total as f64;
    eprintln!(
        "familles du corpus : {total} — typées {typees} ({taux:.1} %), génériques {generiques}, \
         {} parseurs distincts sollicités",
        labels.len()
    );

    // Les familles que l'interface expose doivent répondre, sans quoi elle retomberait sur la
    // vue générique là où elle promet des champs nommés.
    for f in ATTENDUES {
        assert!(
            repondues.contains(f) || !par_famille.contains_key(f),
            "famille attendue non typée : {f}"
        );
    }
    assert!(
        typees > 0,
        "aucune famille typée : le dispatch ne répond plus"
    );
}
