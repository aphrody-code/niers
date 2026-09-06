//! Census d'objets vivants par classe RTTI dans un dump mémoire de `nie.exe`.
//!
//! Compte les objets du tas par pointeur de vtable (→ `.rdata`), résout le nom de classe
//! MSVC, et liste les classes les plus instanciées avec l'adresse statique de leur vtable.
//!
//! ```text
//! cargo run -p nie-re --example dump_census --release -- <dump.dmp> [topN]
//! ```

use std::collections::HashSet;

use nie_re::dump::{Minidump, NIE_IMAGE_BASE};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: {} <dump.dmp> [topN]",
            args.first().map(String::as_str).unwrap_or("dump_census")
        );
        std::process::exit(2);
    }
    let topn: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(40);

    let mut d = match Minidump::open(&args[1]) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ouverture du dump échouée : {e}");
            std::process::exit(1);
        }
    };

    let Some(base) = d.module("nie.exe").map(|m| m.base) else {
        eprintln!("module nie.exe absent du dump");
        std::process::exit(1);
    };
    let Some((rdata_lo, rdata_sz)) = d.module_section(base, b".rdata") else {
        eprintln!("section .rdata introuvable dans nie.exe");
        std::process::exit(1);
    };
    let rdata_hi = rdata_lo + u64::from(rdata_sz);
    println!(
        "nie.exe base=0x{base:X}  .rdata=[0x{rdata_lo:X}..0x{rdata_hi:X}]  régions={}  mappé={:.2} Go",
        d.regions().len(),
        d.mapped_bytes() as f64 / 1e9
    );

    let census = d.vtable_census(rdata_lo, rdata_hi);
    println!("vtables instanciées distinctes : {}\n", census.len());
    println!(
        "{:>9}  {:<46} {:<16} statique",
        "instances", "classe", "vtable"
    );
    println!("{}", "-".repeat(96));

    let mut seen = HashSet::new();
    let mut shown = 0usize;
    for (va, c) in census {
        if va % 8 != 0 {
            continue; // faux positif : pointeur non aligné
        }
        if let Some(nm) = d.rtti_name(va)
            && seen.insert(nm.clone())
        {
            let rva = va - base;
            println!(
                "{c:>9}  {nm:<46} nie+0x{rva:<9X} 0x{:X}",
                NIE_IMAGE_BASE + rva
            );
            shown += 1;
            if shown >= topn {
                break;
            }
        }
    }
}
