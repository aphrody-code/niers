//! Sonde le déchiffrement+parse d'un `cpk_list.cfg.bin` : tente AES-256-CBC puis la clé fixe
//! Viola, et rapporte la méthode gagnante + le nombre d'entrées. Sert à vérifier que les DEUX
//! variantes de build (Steam récent = AES, dump ancien = Viola) sont gérées par `Vfs::init`.
//!
//! Usage : `cargo run -p nie-formats --example probe_cpk_list -- <chemin_cpk_list.cfg.bin>`

use nie_formats::cfgbin::cfgbin_parse;
use nie_formats::cpk::{VIOLA_FIXED_KEY, decrypt_block, decrypt_cpk_list};

fn count(cfg: &nie_formats::cfgbin::CfgBinFile) -> usize {
    cfg.entries.iter().map(|e| e.children.len()).sum()
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: probe_cpk_list <fichier>");
    let data = std::fs::read(&path).expect("lecture fichier");
    println!("fichier {path} ({} o)", data.len());

    if let Some(cfg) = decrypt_cpk_list(&data)
        .ok()
        .and_then(|d| cfgbin_parse(&d).ok())
    {
        println!("✓ AES-256-CBC → {} entrées enfants indexables", count(&cfg));
        return;
    }
    println!("  AES échoue, essai de la clé fixe Viola…");
    let mut viola = data.clone();
    decrypt_block(&mut viola, 0, VIOLA_FIXED_KEY);
    if let Ok(cfg) = cfgbin_parse(&viola) {
        println!("✓ Viola → {} entrées enfants indexables", count(&cfg));
        return;
    }
    println!("✗ AUCUNE méthode ne parse ce cpk_list");
    std::process::exit(1);
}
