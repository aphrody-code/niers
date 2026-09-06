//! Inspecteur G4TX : structure (textures principales + sous-textures nommées + dims).
//! Usage : `cargo run -p nie-formats --example inspect_g4tx -- <fichier.g4tx>`

use nie_formats::g4tx;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: inspect_g4tx <fichier.g4tx>");
    let data = std::fs::read(&path).expect("lire g4tx");
    let tx = g4tx::parse(&data).expect("parse g4tx");
    println!("{path} : {} texture(s) principale(s)", tx.textures.len());
    for (i, t) in tx.textures.iter().enumerate() {
        println!(
            "  [{i}] id={} name={:?} {}x{} is_dds={} data@{}+{} sub_textures={}",
            t.id,
            t.name,
            t.width,
            t.height,
            t.is_dds,
            t.data_offset,
            t.data_size,
            t.sub_textures.len()
        );
        for st in t.sub_textures.iter().take(20) {
            println!(
                "      sub id={} name={:?} x={} y={} w={} h={}",
                st.id, st.name, st.x, st.y, st.width, st.height
            );
        }
        if t.sub_textures.len() > 20 {
            println!("      … (+{} autres)", t.sub_textures.len() - 20);
        }
    }
}
