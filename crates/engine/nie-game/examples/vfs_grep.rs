//! Grep l'index VFS complet (254k fichiers, sans cap) par sous-chaîne de chemin.
//! Usage : `NIE_GAME_DIR=… cargo run -p nie-game --example vfs_grep -- <substr> [<substr>…]`
use nie_formats::vfs::Vfs;
use std::path::PathBuf;

fn main() {
    let needles: Vec<String> = std::env::args().skip(1).map(|s| s.to_lowercase()).collect();
    if needles.is_empty() {
        eprintln!("usage: vfs_grep <substr> [<substr>…]");
        return;
    }
    let game = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let data = PathBuf::from(&game).join("data");
    let mut vfs = Vfs::new();
    vfs.init(&data).expect("vfs init");
    let mut n = 0usize;
    for (path, e) in vfs.iter() {
        let lp = path.to_lowercase();
        if needles.iter().any(|nd| lp.contains(nd.as_str())) {
            println!("{path}  [{}o, {}]", e.file_size, e.cpk_filename);
            n += 1;
        }
    }
    eprintln!("→ {n} correspondance(s) sur {} fichiers", vfs.asset_count());
}
