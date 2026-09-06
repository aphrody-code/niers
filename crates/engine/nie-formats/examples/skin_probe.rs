//! Sonde le skinning d'une paire G4MD/G4MG : plage des indices d'os et somme des poids.
//!
//! Sert à trancher si les indices de `VertexSkin` sont locaux à une palette ou déjà globaux au
//! squelette — la question qui bloque l'application du skinning.

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(p_md), Some(p_mg)) = (args.next(), args.next()) else {
        eprintln!("usage: skin_probe <g4md> <g4mg>");
        std::process::exit(2);
    };
    let md_bytes = std::fs::read(&p_md).expect("lecture g4md");
    let mg_bytes = std::fs::read(&p_mg).expect("lecture g4mg");
    let md = nie_formats::g4md::parse(&md_bytes).expect("parse g4md");
    println!(
        "  sous-mailles {} | os déclarés {}",
        md.submeshes.len(),
        md.header.bone_count
    );

    for (i, _) in md.submeshes.iter().enumerate() {
        match nie_formats::g4mg::extract_skin(&mg_bytes, &md, i) {
            Some(skin) => {
                let mut mini = u8::MAX;
                let mut maxi = 0u8;
                let mut somme_hors = 0usize;
                for v in &skin {
                    for (b, w) in v.bones.iter().zip(&v.weights) {
                        if *w > 0.0 {
                            mini = mini.min(*b);
                            maxi = maxi.max(*b);
                        }
                    }
                    let s: f32 = v.weights.iter().sum();
                    if !(0.9..=1.1).contains(&s) {
                        somme_hors += 1;
                    }
                }
                println!(
                    "  sm[{i}] {} sommets | indices d'os {}..{} | {} sommets dont les poids ne somment pas à 1",
                    skin.len(),
                    mini,
                    maxi,
                    somme_hors
                );
            }
            None => println!("  sm[{i}] pas de skinning lisible"),
        }
    }
}
