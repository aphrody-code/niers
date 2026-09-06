//! Vérification byte-exacte d'un fichier dumpé par dump_packs contre le VFS CPK.
//!
//! Usage :
//! ```text
//! NIE_GAME_DIR=/home/aphrody/niers \
//!   cargo run -p nie-game --example verify_dump -- \
//!     <chemin_interne_vfs> <chemin_fichier_dumpé>
//! ```
//! Retourne 0 si identique, 1 si différent.
use nie_formats::vfs::Vfs;
use std::path::Path;

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let vfs_path = args
        .next()
        .expect("arg 1 : chemin interne VFS (ex. data/dx11/menu/.../foo.g4tx)");
    let dump_path = args
        .next()
        .expect("arg 2 : chemin fichier dumpé sur disque");

    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let vfs_bytes = vfs
        .read(&vfs_path)
        .unwrap_or_else(|e| panic!("vfs.read({vfs_path}): {e}"));

    let dump_bytes =
        std::fs::read(&dump_path).unwrap_or_else(|e| panic!("fs::read({dump_path}): {e}"));

    if vfs_bytes == dump_bytes {
        println!(
            "OK — {vfs_path} : {} octets identiques (VFS == dump)",
            vfs_bytes.len()
        );
        std::process::ExitCode::SUCCESS
    } else {
        eprintln!(
            "DIFF — {vfs_path} : VFS={} octets vs dump={} octets",
            vfs_bytes.len(),
            dump_bytes.len()
        );
        // Afficher les premiers octets divergents pour diagnostic.
        for (i, (a, b)) in vfs_bytes.iter().zip(dump_bytes.iter()).enumerate() {
            if a != b {
                eprintln!("  première divergence @ offset {i:#010x} : VFS={a:#04x} dump={b:#04x}");
                break;
            }
        }
        std::process::ExitCode::FAILURE
    }
}
