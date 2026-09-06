//! Couverture réelle des `.cfg.bin` du jeu par les parseurs de conteneur.
//!
//! Deux formats coexistent derrière l'extension : **RDBN** (à listes) et **T2B** (arbre, sans
//! magic de tête, reconnu par son pied de page). Cet exemple les parcourt tous, tente le parseur
//! qui convient, et ventile le résultat **par catégorie** pour que l'échec, s'il y en a, désigne
//! une famille de données plutôt qu'un fichier isolé.
//!
//! ```text
//! cargo run -p nie-formats --example corpus_cfgbin --release
//! ```

use std::collections::BTreeMap;
use std::path::Path;

/// Compteurs d'une catégorie.
#[derive(Default)]
struct Compte {
    rdbn: usize,
    t2b: usize,
    echec: usize,
    /// Premier chemin en échec — de quoi rejouer le cas à la main.
    premier_echec: Option<String>,
}

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let data_dir = Path::new(&dir).join("data");

    let mut vfs = nie_formats::vfs::Vfs::new();
    if vfs.init(&data_dir).is_err() {
        eprintln!("skip : jeu absent à {}", data_dir.display());
        return;
    }

    let chemins: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.ends_with(".cfg.bin"))
        .collect();
    println!("{} fichiers .cfg.bin\n", chemins.len());

    let mut par_categorie: BTreeMap<String, Compte> = BTreeMap::new();
    let (mut rdbn, mut t2b, mut echec) = (0usize, 0usize, 0usize);

    for chemin in &chemins {
        // Catégorie = premier segment sous `gamedata/`, sinon le dossier de tête.
        let categorie = chemin
            .split_once("gamedata/")
            .map_or_else(
                || {
                    chemin
                        .trim_start_matches("data/")
                        .split('/')
                        .next()
                        .unwrap_or("?")
                },
                |(_, reste)| reste.split('/').next().unwrap_or("?"),
            )
            .to_string();
        let c = par_categorie.entry(categorie).or_default();

        let Ok(data) = vfs.read(chemin) else {
            c.echec += 1;
            echec += 1;
            c.premier_echec.get_or_insert_with(|| chemin.clone());
            continue;
        };

        // RDBN se reconnaît à son magic ; tout le reste est tenté en T2B.
        let ok = if nie_formats::cfgbin::is_rdbn(&data) {
            let r = nie_formats::cfgbin::parse(&data).is_ok();
            if r {
                c.rdbn += 1;
                rdbn += 1;
            }
            r
        } else {
            let r = nie_formats::cfgbin::cfgbin_parse(&data).is_ok();
            if r {
                c.t2b += 1;
                t2b += 1;
            }
            r
        };
        if !ok {
            c.echec += 1;
            echec += 1;
            c.premier_echec.get_or_insert_with(|| chemin.clone());
        }
    }

    println!(
        "{:<22} {:>7} {:>7} {:>7}",
        "catégorie", "RDBN", "T2B", "échec"
    );
    println!("{}", "-".repeat(46));
    for (cat, c) in &par_categorie {
        println!("{:<22} {:>7} {:>7} {:>7}", cat, c.rdbn, c.t2b, c.echec);
    }

    let total = rdbn + t2b + echec;
    println!("{}", "-".repeat(46));
    println!("{:<22} {rdbn:>7} {t2b:>7} {echec:>7}", "TOTAL");
    #[allow(clippy::cast_precision_loss)]
    let taux = if total == 0 {
        0.0
    } else {
        (rdbn + t2b) as f64 * 100.0 / total as f64
    };
    println!("\ncouverture : {taux:.2} % ({}/{total})", rdbn + t2b);

    if echec > 0 {
        println!("\npremiers échecs par catégorie :");
        for (cat, c) in &par_categorie {
            if let Some(p) = &c.premier_echec {
                println!("  {cat:<20} {p}");
            }
        }
    }
}
