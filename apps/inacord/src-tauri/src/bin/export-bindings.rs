//! Régénère `src/lib/bindings.ts` depuis les signatures Rust, sans ouvrir de fenêtre.
//!
//! `cargo run --bin export-bindings` depuis `apps/nie-explorer/src-tauri`.
//!
//! `export_bindings` est gaté `#[cfg(debug_assertions)]` — la réflexion specta n'a rien à faire
//! dans un binaire distribué. Ce binaire doit donc l'être aussi, sinon `cargo build --release`
//! échoue à le lier et fait tomber toute la release avec lui.

#[cfg(debug_assertions)]
fn main() {
    match nie_explorer_lib::export_bindings() {
        Ok(()) => println!("bindings TypeScript régénérés : ../src/lib/bindings.ts"),
        Err(e) => {
            eprintln!("échec de l'export des bindings : {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(debug_assertions))]
fn main() {
    eprintln!(
        "export-bindings n'existe qu'en profil debug — relancer sans --release \
         (la génération des bindings est un outil de développement, pas un livrable)."
    );
    std::process::exit(1);
}
