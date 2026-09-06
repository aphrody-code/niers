//! Assemble un GLB texturé (g4md+g4mg+g4tx) et l'écrit sur disque — même recette que
//! `apps/inacord/src-tauri/src/lib.rs::vfs_glb_preview_png_b64` (bouton « Aperçu 3D »
//! de niers-explorer), en CLI pour vérification/scripting hors app.
//! Usage : `cargo run -p nie-formats --example model_glb_preview -- <mesh.g4md> <mesh.g4mg> <mesh.g4tx> <out.glb>`
//! Rendu ensuite en PNG via : `cargo run -p nie-render3d -- --glb <out.glb> --out <out.png> --frames 1`

use nie_formats::assemble::{
    EmbeddedTexture, GenericModelInput, MeshComponent, assemble_generic_model,
};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let g4md = std::fs::read(&a[1]).expect("lecture g4md");
    let g4mg = std::fs::read(&a[2]).expect("lecture g4mg");
    let g4tx = std::fs::read(&a[3]).expect("lecture g4tx");
    let out = &a[4];

    let stem = std::path::Path::new(&a[1])
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model")
        .to_string();

    let mut model = assemble_generic_model(GenericModelInput {
        code: stem.clone(),
        g4md,
        g4mg,
        component: MeshComponent::Generic,
    })
    .expect("assemblage GLB");

    if let Some(png) = nie_formats::g4tx_decode::decode_best_to_png(
        &g4tx,
        nie_formats::g4tx_decode::basename_of(&a[3]),
    ) {
        model.embedded_textures.push(EmbeddedTexture {
            component: MeshComponent::Generic,
            name: format!("{stem}_tex"),
            png_bytes: png,
        });
    } else {
        eprintln!("(pas de texture décodée depuis {})", a[3]);
    }

    let glb = model.to_glb_embedded();
    std::fs::write(out, &glb).expect("écriture glb");
    println!("GLB écrit -> {out} ({} octets)", glb.len());
}
