//! Décode un RDBN depuis un **fichier précis** (et non depuis le VFS), pour comparer un vanilla
//! et la version d'un mod côte à côte.
//!
//! Avec `--only <liste>`, ne dump que la liste nommée. Avec `--max`, n'affiche que les lignes
//! dont une valeur dépasse `--seuil` (999 par défaut) — pratique pour repérer d'un coup ce qu'un
//! mod a « maxé ».
//!
//! Usage : `cargo run -p nie-formats --example rdbn_dump -- <f.cfg.bin> [--only <liste>] [--max]`

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut only = None;
    let mut seulement_max = false;
    let mut seuil = 999i64;
    if let Some(i) = args.iter().position(|a| a == "--only") {
        only = args.get(i + 1).cloned();
        args.drain(i..=i + 1);
    }
    if let Some(i) = args.iter().position(|a| a == "--seuil") {
        seuil = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(999);
        args.drain(i..=i + 1);
    }
    if let Some(i) = args.iter().position(|a| a == "--max") {
        seulement_max = true;
        args.remove(i);
    }
    let path = args
        .first()
        .expect("usage: rdbn_dump <f.cfg.bin> [--only <liste>] [--max]");

    let data = std::fs::read(path).expect("lecture");
    let rdbn = nie_formats::cfgbin::parse(&data).expect("ce fichier n'est pas un RDBN");
    let lists = nie_formats::cfgbin::read_values(&rdbn, &data);
    println!("{path}\n  {} liste(s)", lists.len());

    for l in &lists {
        if only.as_ref().is_some_and(|o| &l.name != o) {
            continue;
        }
        println!(
            "\n=== {} ({}) — {} ligne(s) ===",
            l.name,
            l.type_name,
            l.rows.len()
        );
        let mut montrees = 0usize;
        for (i, row) in l.rows.iter().enumerate() {
            let champs: Vec<String> = row
                .fields
                .iter()
                .map(|(k, v)| format!("{k}={v:?}"))
                .collect();
            if seulement_max {
                // Ne garder que les lignes portant une valeur ≥ seuil : c'est la signature d'un
                // « max » posé par un mod.
                // `RdbnValue` n'expose pas d'accesseur numérique : on lit le Debug, qui rend
                // « Int(999) », et on en extrait l'entier entre parenthèses.
                let atteint = row.fields.iter().any(|(_, v)| {
                    let d = format!("{v:?}");
                    d.split(['(', ')'])
                        .nth(1)
                        .and_then(|n| n.parse::<i64>().ok())
                        .is_some_and(|n| n >= seuil)
                });
                if !atteint {
                    continue;
                }
            }
            println!("  row{i}: {}", champs.join("  "));
            montrees += 1;
            if montrees >= 40 {
                println!("  … ({} lignes restantes)", l.rows.len() - i - 1);
                break;
            }
        }
        if montrees == 0 {
            println!("  (aucune ligne ne dépasse {seuil})");
        }
    }
}
