//! Scanne un minidump de `nie.exe` pour des motifs AOB et traduit chaque coup en
//! `nie.exe + RVA` (et adresse statique r2 `0x140000000 + RVA`).
//!
//! ```text
//! cargo run -p nie-re --example dump_scan --release -- <dump.dmp> <patterns.txt> [out.json]
//! ```
//!
//! Format du fichier de motifs (une signature par ligne) :
//! ```text
//! NomFeature = 44 8B 6F 10 8B 47 04
//! AutreFeature = 49 8B ?? 0F B7 D7 E8 ?? ?? ?? ?? 4C   # ?? = wildcard
//! ```
//! Les lignes vides et celles commençant par `#` sont ignorées ; un `#` en fin de ligne
//! termine la valeur.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use nie_re::dump::{Minidump, Pattern};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "usage: {} <dump.dmp> <patterns.txt> [out.json]",
            args.first().map(String::as_str).unwrap_or("dump_scan")
        );
        std::process::exit(2);
    }
    let dmp = &args[1];
    let pats_path = &args[2];
    let json_out = args.get(3);

    let mut dump = match Minidump::open(dmp) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ouverture du dump échouée : {e}");
            std::process::exit(1);
        }
    };

    let nie = dump.module("nie.exe").cloned();
    println!(
        "dump: {} modules, {} régions, {:.2} Go mappés",
        dump.modules.len(),
        dump.range_count(),
        dump.mapped_bytes() as f64 / 1e9
    );
    if let Some(m) = &nie {
        println!(
            "nie.exe: base=0x{:X} taille={} ({:.1} Mo)",
            m.base,
            m.size,
            f64::from(m.size) / 1e6
        );
    } else {
        println!("(attention : module nie.exe absent du dump)");
    }

    // Lecture du fichier de motifs.
    let text = match std::fs::read_to_string(pats_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("lecture des motifs échouée : {e}");
            std::process::exit(1);
        }
    };
    let mut names: Vec<String> = Vec::new();
    let mut patterns: Vec<Pattern> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, rest)) = line.split_once('=') else {
            continue;
        };
        let aob = rest.split('#').next().unwrap_or("").trim();
        match Pattern::parse(aob) {
            Ok(p) => {
                names.push(name.trim().to_string());
                patterns.push(p);
            }
            Err(e) => eprintln!("motif ignoré ({name}) : {e}"),
        }
    }
    println!("motifs chargés : {}\n", patterns.len());

    let results = match dump.scan_many(&patterns) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("scan échoué : {e}");
            std::process::exit(1);
        }
    };

    let mut json = BTreeMap::<String, String>::new();
    println!(
        "{:<34} {:>5} {:>5}  {:<14} {:<14}",
        "feature", "nie", "autre", "nie.exe+RVA", "addr statique"
    );
    println!("{}", "-".repeat(78));
    for (name, hits) in names.iter().zip(&results) {
        let in_nie: Vec<_> = hits
            .iter()
            .filter(|h| h.module.as_deref() == Some("nie.exe"))
            .collect();
        let other = hits.len() - in_nie.len();
        let (rva_s, stat_s) = match in_nie.first() {
            Some(h) => (
                h.rva
                    .map_or_else(|| "-".to_string(), |r| format!("nie+0x{r:X}")),
                h.nie_static()
                    .map_or_else(|| "-".to_string(), |s| format!("0x{s:X}")),
            ),
            None => ("(aucun)".to_string(), "-".to_string()),
        };
        println!(
            "{name:<34} {:>5} {other:>5}  {rva_s:<14} {stat_s:<14}",
            in_nie.len()
        );
        let mut entry = String::new();
        let _ = write!(entry, "hits_nie={} other={}", in_nie.len(), other);
        if let Some(h) = in_nie.first()
            && let Some(r) = h.rva
        {
            let _ = write!(
                entry,
                " rva=0x{r:X} static=0x{:X}",
                h.nie_static().unwrap_or(0)
            );
        }
        json.insert(name.clone(), entry);
    }

    if let Some(path) = json_out {
        let mut s = String::from("{\n");
        for (i, (k, v)) in json.iter().enumerate() {
            let comma = if i + 1 < json.len() { "," } else { "" };
            let _ = writeln!(s, "  {:?}: {:?}{comma}", k, v);
        }
        s.push_str("}\n");
        if let Err(e) = std::fs::write(path, s) {
            eprintln!("écriture JSON échouée : {e}");
        } else {
            println!("\ncarte écrite -> {path}");
        }
    }
}
