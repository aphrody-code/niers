//! Sonde générique T2B : dump l'arbre `entries` (nom + variables positionnelles + enfants) d'un
//! `*.cfg.bin` **T2B** du VFS. Analogue de `probe_rdbn` pour le format T2B (arbre imbriqué
//! `CfgEntry { name, variables, children }`, vars accédées par INDEX).
//!
//! Sert à cadrer le port de n'importe quelle famille T2B (cf. template `nie_data::shop`).
//!
//! Usage : `cargo run -p nie-game --example probe_t2b -- <prefix> [max_depth] [max_children]`
//! (ex. `probe_t2b boost_player_group_config 4 6`). Respecte `NIE_GAME_DIR`.

use nie_formats::cfgbin::{self, CfgEntry};
use nie_formats::vfs::Vfs;
use std::path::Path;

fn walk(e: &CfgEntry, depth: usize, max_depth: usize, max_children: usize) {
    let indent = "  ".repeat(depth);
    let vars: Vec<String> = e
        .variables
        .iter()
        .take(10)
        .map(|v| format!("{v:?}"))
        .collect();
    let more_v = if e.variables.len() > 10 {
        format!(" …+{}", e.variables.len() - 10)
    } else {
        String::new()
    };
    println!(
        "{indent}'{}' ({} vars, {} children)  vars=[{}{more_v}]",
        e.name,
        e.variables.len(),
        e.children.len(),
        vars.join(", ")
    );
    if depth >= max_depth {
        return;
    }
    for c in e.children.iter().take(max_children) {
        walk(c, depth + 1, max_depth, max_children);
    }
    if e.children.len() > max_children {
        println!(
            "{indent}  …+{} more children",
            e.children.len() - max_children
        );
    }
}

fn main() {
    let prefix = std::env::args()
        .nth(1)
        .expect("usage: probe_t2b <prefix> [max_depth] [max_children]");
    let max_depth = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let max_children = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.rsplit('/')
                .next()
                .is_some_and(|b| b.starts_with(&prefix) && b.ends_with(".cfg.bin"))
        })
        .min()
        .unwrap_or_else(|| panic!("{prefix} introuvable dans le VFS"));
    eprintln!("file = {path}");
    let bytes = vfs.read(&path).expect("read");
    if cfgbin::is_rdbn(&bytes) {
        eprintln!("(RDBN, pas T2B — utilise probe_rdbn)");
        return;
    }
    let file = cfgbin::parse_t2b(&bytes).expect("parse_t2b");
    println!(
        "=== T2B : {} entrées de premier niveau ===",
        file.entries.len()
    );
    for e in file.entries.iter().take(max_children.max(30)) {
        walk(e, 0, max_depth, max_children);
    }
}
