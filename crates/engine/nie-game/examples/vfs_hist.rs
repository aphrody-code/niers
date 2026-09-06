//! Histogramme des extensions de fichiers dans le VFS complet.
//! Usage : NIE_GAME_DIR=… cargo run -q -p nie-game --example vfs_hist
use nie_formats::vfs::Vfs;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let game = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let data = PathBuf::from(&game).join("data");
    let mut vfs = Vfs::new();
    vfs.init(&data).expect("vfs init");
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut sample: HashMap<String, Vec<String>> = HashMap::new();
    for (path, _e) in vfs.iter() {
        let base = path.rsplit('/').next().unwrap_or(path);
        let ext = match base.rsplit_once('.') {
            Some((_, e)) => e.to_lowercase(),
            None => "<none>".to_string(),
        };
        *counts.entry(ext.clone()).or_default() += 1;
        let s = sample.entry(ext).or_default();
        if s.len() < 3 {
            s.push(path.to_string());
        }
    }
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by_key(|b| std::cmp::Reverse(b.1));
    println!("total assets = {}", vfs.asset_count());
    for (ext, c) in v.iter().take(50) {
        println!("{:>8}  .{:<14} | {}", c, ext, sample[ext].join("  ::  "));
    }
}
