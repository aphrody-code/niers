//! Validation de l'extraction des POIDS DE SKINNING d'un g4md/g4mg de perso.
//! Usage : `cargo run -p nie-formats --example validate_skin -- <chr.g4md> <chr.g4mg>`

use nie_formats::g4md;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let md_bytes = std::fs::read(&args[1]).expect("g4md");
    let mg = std::fs::read(&args[2]).expect("g4mg");
    let md = g4md::parse(&md_bytes).expect("parse g4md");

    println!(
        "submeshes={} bone_count={} attributs:",
        md.submeshes.len(),
        md.header.bone_count
    );
    for a in &md.attributes {
        let sem = match a.vtype {
            1 => "POS",
            2 => "NRM",
            3 => "BINRM",
            5 => "WEIGHTS",
            6 => "INDICES",
            8 => "COLOR",
            10..=14 => "UV/TAN",
            _ => "?",
        };
        println!(
            "  vtype={:2}({sem}) offset={} datatype={}",
            a.vtype, a.offset, a.datatype
        );
    }

    if md.find_attribute(5).is_none() {
        println!("PAS d'attribut WEIGHTS (vtype 5) — mesh non skinné ?");
        return;
    }
    let skin = nie_formats::g4mg::extract_skin(&mg, &md, 0).expect("extract_skin");

    // Validation : somme des poids ≈ 1, indices d'os significatifs < bone_count.
    let mut ok = 0usize;
    let mut bad = 0usize;
    let mut max_idx = 0u8;
    for (v, s) in skin.iter().enumerate() {
        for k in 0..8 {
            if s.weights[k] > 0.0 {
                max_idx = max_idx.max(s.bones[k]);
            }
        }
        let sum: f32 = s.weights.iter().sum();
        if (sum - 1.0).abs() < 0.05 {
            ok += 1;
        } else {
            bad += 1;
        }
        if v < 6 {
            let w4: Vec<f32> = s
                .weights
                .iter()
                .take(4)
                .map(|x| (x * 100.0).round() / 100.0)
                .collect();
            println!(
                "  v{v}: w[..4]={w4:?} sum={sum:.3} idx[..4]={:?}",
                &s.bones[..4]
            );
        }
    }
    println!(
        "poids somment à 1 : {ok}/{} (mauvais {bad}), index d'os max={max_idx} (bone_count={})",
        ok + bad,
        md.header.bone_count
    );
    let valid = ok > bad * 5 && max_idx < md.header.bone_count.max(1);
    println!(
        "{}",
        if valid {
            "VALIDÉ ✓"
        } else {
            "ÉCHEC ✗ (datatype poids à ajuster ?)"
        }
    );
}
