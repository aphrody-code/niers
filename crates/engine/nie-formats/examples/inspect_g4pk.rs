//! Inspecteur G4PK : liste les sous-fichiers (nom, offset, taille, magic du contenu).
//! Usage : `cargo run -p nie-formats --example inspect_g4pk -- <fichier.g4pk>`

use nie_formats::g4pk;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: inspect_g4pk <fichier.g4pk>");
    let data = std::fs::read(&path).expect("lire g4pk");
    let pk = g4pk::parse(&data).expect("parse g4pk");
    println!("{path} : {} sous-fichiers", pk.files.len());
    for f in &pk.files {
        let magic = data
            .get(f.offset..f.offset + 4)
            .map(|m| String::from_utf8_lossy(m).into_owned())
            .unwrap_or_default();
        println!(
            "  {} (offset {}, {} o, magic {:?})",
            f.name, f.offset, f.size, magic
        );
    }
}
