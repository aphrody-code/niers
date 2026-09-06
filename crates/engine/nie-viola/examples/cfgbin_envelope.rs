//! Identifie l'enveloppe d'un `.cfg.bin` (AES, clé Viola, ou clair) et, si demandé, écrit le
//! clair sur disque.
//!
//! Plusieurs fichiers de données du jeu sont chiffrés comme le `cpk_list` : le parseur T2B les
//! refuse alors avec « String table offset out of bounds », ce qui ressemble à une corruption
//! alors que le fichier est seulement enveloppé. Ce probe tranche.
//!
//! Usage : `cargo run -p nie-viola --example cfgbin_envelope -- <f.cfg.bin>… [--out-dir <d>]`

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut out_dir = None;
    if let Some(i) = args.iter().position(|a| a == "--out-dir") {
        out_dir = args.get(i + 1).cloned();
        args.drain(i..=i + 1);
    }
    assert!(
        !args.is_empty(),
        "usage: cfgbin_envelope <f.cfg.bin>… [--out-dir <d>]"
    );

    for path in &args {
        let brut = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                println!("{path}\n  illisible : {e}");
                continue;
            }
        };
        let nom = std::path::Path::new(path)
            .file_name()
            .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());

        // Même cascade que `decode_cpk_list`, mais on rapporte le clair au lieu de l'arbre.
        let mut trouve = None;
        if let Ok(clair) = nie_formats::cpk::decrypt_cpk_list(&brut)
            && nie_formats::cfgbin::cfgbin_parse(&clair).is_ok()
        {
            trouve = Some(("AES-256-CBC", clair));
        }
        if trouve.is_none() {
            let mut viola = brut.clone();
            nie_formats::cpk::decrypt_block(&mut viola, 0, nie_formats::cpk::VIOLA_FIXED_KEY);
            if nie_formats::cfgbin::cfgbin_parse(&viola).is_ok() {
                trouve = Some(("clé Viola", viola));
            }
        }
        if trouve.is_none() && nie_formats::cfgbin::cfgbin_parse(&brut).is_ok() {
            trouve = Some(("clair", brut.clone()));
        }

        match trouve {
            None => println!(
                "{nom}\n  {} o — AUCUNE enveloppe connue ne le rend lisible",
                brut.len()
            ),
            Some((quoi, clair)) => {
                let cfg = nie_formats::cfgbin::cfgbin_parse(&clair).expect("déjà validé");
                let enfants = cfg.entries.first().map_or(0, |e| e.children.len());
                println!(
                    "{nom}\n  {} o — enveloppe : {quoi} — {} entrée(s), {enfants} enfant(s) sous la racine",
                    brut.len(),
                    cfg.entries.len()
                );
                if let Some(d) = &out_dir {
                    let p = std::path::Path::new(d).join(format!("clair_{nom}"));
                    std::fs::create_dir_all(d).ok();
                    std::fs::write(&p, &clair).expect("écriture du clair");
                    println!("  clair écrit dans {}", p.display());
                }
            }
        }
    }
}
