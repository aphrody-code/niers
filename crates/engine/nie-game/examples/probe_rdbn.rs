//! Sonde générique RDBN : dump les noms de listes + champs (et 2 lignes) d'un `*.cfg.bin` du VFS.
//! Réutilisable pour cadrer le port de n'importe quelle famille RDBN.
//!
//! Usage : `cargo run -p nie-game --example probe_rdbn -- <filename_prefix>`
//! (ex. `event_movie_config`, `chara_details_config`).
use nie_formats::cfgbin::{self, RdbnValue};
use nie_formats::vfs::Vfs;
use std::path::Path;

fn show(v: &RdbnValue) -> String {
    match v {
        RdbnValue::Hash(h) => format!("0x{h:08X}"),
        RdbnValue::Int(n) | RdbnValue::Flag(n) => n.to_string(),
        RdbnValue::Short(n) | RdbnValue::ActType(n) => n.to_string(),
        RdbnValue::Byte(n) => n.to_string(),
        RdbnValue::Bool(b) => b.to_string(),
        RdbnValue::Float(f) => format!("{f}"),
        RdbnValue::Condition(s) => format!("str:{s:?}"),
        other => format!("{other:?}"),
    }
}

fn main() {
    let prefix = std::env::args()
        .nth(1)
        .expect("usage: probe_rdbn <filename_prefix>");
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
    if !cfgbin::is_rdbn(&bytes) {
        eprintln!("(T2B, pas RDBN)");
        return;
    }
    let rdbn = cfgbin::parse(&bytes).expect("parse");
    for l in cfgbin::read_values(&rdbn, &bytes) {
        eprintln!(
            "\n=== list '{}' type '{}' rows={} ===",
            l.name,
            l.type_name,
            l.rows.len()
        );
        if let Some(first) = l.rows.first() {
            let fields: Vec<&str> = first.fields.iter().map(|(n, _)| n.as_str()).collect();
            eprintln!("fields: {}", fields.join(", "));
        }
        for (i, row) in l.rows.iter().take(2).enumerate() {
            let kv: Vec<String> = row
                .fields
                .iter()
                .map(|(n, v)| format!("{n}={}", show(v)))
                .collect();
            eprintln!("  row{i}: {}", kv.join("  "));
        }
    }
}
