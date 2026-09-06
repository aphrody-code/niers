//! Cherche la provenance des **hashes de sprite** d'un menu dans le pool de constantes du
//! bytecode des scripts.
//!
//! Contexte (cf. `docs/PLAN.md`) : les objets construits par le driver de menu portent un
//! `sprite_texture_hash` que rien ne résout — ni `hash_name` (48 508 textures indexées), ni un
//! CRC32 de chemin `.g4tx` (9 variantes x 3 casses x 54 203 chemins), ni une constante de table
//! Lua atteignable depuis `_G`. Le RE du handler `0x140CE74D0` montre que ces valeurs arrivent en
//! **nombres**, jamais en chaînes : le nom n'existe donc pas côté hôte.
//!
//! Reste une source : les valeurs sont des **littéraux du bytecode**. Ce probe parcourt les
//! scripts de menu du VFS, collecte toutes les constantes numériques (prototypes imbriqués
//! compris) et dit lesquels portent les hashes cherchés — ce qui désigne le script à lire.
//!
//! Usage : `cargo run -p nie-lua --example hunt_sprite_hash -- 2320109963 2734777837`

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use nie_lua::bytecode::{Constant, Prototype};

/// Collecte les constantes numériques d'un prototype et de tous ses prototypes imbriqués.
fn collect_numbers(p: &Prototype, out: &mut Vec<f64>) {
    for c in &p.constants {
        if let Constant::Number(n) = c {
            out.push(*n);
        }
    }
    for sub in &p.protos {
        collect_numbers(sub, out);
    }
}

/// Collecte les constantes chaînes (octets bruts : encodage hérité possible).
fn collect_strings(p: &Prototype, out: &mut Vec<Vec<u8>>) {
    for c in &p.constants {
        if let Constant::String(s) = c {
            out.push(s.clone());
        }
    }
    for sub in &p.protos {
        collect_strings(sub, out);
    }
}

/// CRC-32 (IEEE, réfléchi) — celui qu'emploient les `cfg.bin`/`objbin` du jeu.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn main() {
    let targets: HashSet<u32> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse::<u32>().ok())
        .collect();
    let targets: HashSet<u32> = if targets.is_empty() {
        // Hashes observés sur `main_menu` (cf. probe_menu_script).
        [2_320_109_963, 2_734_777_837, 1_342_860_085, 849_696_660]
            .into_iter()
            .collect()
    } else {
        targets
    };
    println!("cibles : {} hash(es)", targets.len());

    let dir = nie_formats::vfs::resolve_game_dir();
    let mut vfs = nie_formats::vfs::Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    // Les scripts de menu et leurs includes : c'est là que vivent les constantes d'écran.
    let scripts: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.ends_with(".lua.bin") && p.contains("/script/lua/"))
        .collect();
    println!("{} scripts .lua.bin à scanner", scripts.len());

    let mut found: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let (mut parsed, mut failed) = (0_usize, 0_usize);
    for path in &scripts {
        let Ok(bytes) = vfs.read(path) else { continue };
        let Ok(chunk) = nie_lua::bytecode::parse(&bytes) else {
            failed += 1;
            continue;
        };
        parsed += 1;
        let mut nums = Vec::new();
        collect_numbers(&chunk.main, &mut nums);
        for n in nums {
            // Les hashes transitent en f64 : ne retenir que ceux qui sont des entiers exacts
            // représentables en u32, sinon tout flottant de mise en page ferait du bruit.
            if n >= 0.0 && n <= f64::from(u32::MAX) && n.fract() == 0.0 {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let v = n as u32;
                if targets.contains(&v) {
                    found.entry(v).or_default().push(path.clone());
                }
            }
        }
    }

    println!("{parsed} scripts parsés, {failed} illisibles\n");
    println!(
        "=== {} / {} hash(es) retrouvé(s) ===",
        found.len(),
        targets.len()
    );
    for (h, paths) in &found {
        println!("  0x{h:08X} ({h}) — {} script(s)", paths.len());
        for p in paths.iter().take(4) {
            println!("       {p}");
        }
    }
    for t in &targets {
        if !found.contains_key(t) {
            println!("  0x{t:08X} ({t}) — ABSENT du pool de constantes");
        }
    }

    // Corrélation décisive : un script qui porte le hash porte peut-être aussi le NOM en clair
    // dans ses constantes chaînes. Si `crc32(chaîne)` redonne le hash, on tient à la fois
    // l'algorithme et le nom de la ressource — donc la texture à charger.
    println!("\n=== corrélation crc32(constante chaîne) -> hash cible ===");
    let mut resolved: BTreeMap<u32, String> = BTreeMap::new();
    let mut scanned_strings = 0_usize;
    for path in &scripts {
        let Ok(bytes) = vfs.read(path) else { continue };
        let Ok(chunk) = nie_lua::bytecode::parse(&bytes) else {
            continue;
        };
        let mut strs = Vec::new();
        collect_strings(&chunk.main, &mut strs);
        scanned_strings += strs.len();
        for s in strs {
            let trimmed = s.strip_suffix(b"\0").unwrap_or(&s);
            for cand in [&s[..], trimmed] {
                let h = crc32(cand);
                if targets.contains(&h) {
                    resolved
                        .entry(h)
                        .or_insert_with(|| String::from_utf8_lossy(cand).into_owned());
                }
            }
        }
    }
    println!("{scanned_strings} constantes chaînes examinées");
    if resolved.is_empty() {
        println!(
            "  aucune : le crc32 d'une constante chaîne du bytecode ne donne aucun hash cible"
        );
    } else {
        for (h, name) in &resolved {
            println!("  0x{h:08X} ({h}) <- crc32(\"{name}\")");
        }
    }
}
