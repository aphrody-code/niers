//! Sonde la structure interne d'un ACB (Atom Cue Bank) : colonnes de la table racine et de
//! chaque sous-table `@UTF` embarquée, avec leurs premières lignes.
//!
//! Sert à établir, sur des fichiers RÉELS, ce qu'un ACB sait dire de ses cues sans jamais
//! ouvrir l'AWB — condition nécessaire pour cataloguer les 5 403 banques du jeu, dont l'AWB
//! pèse 7,49 Gio (un seul fichier atteint 1,25 Gio).
//!
//! ```text
//! NIE_GAME_DIR=… cargo run -p nie-formats --example probe_acb -- <chemin/vfs.acb> [colonne]
//! ```

use nie_formats::cpk::{UtfValue, parse_utf};
use nie_formats::vfs::Vfs;

/// Rend une valeur `@UTF` lisible, en bornant les blobs (une colonne `Bytes` peut peser des Mo).
fn apercu(v: &UtfValue) -> String {
    match v {
        UtfValue::String(s) => format!("{s:?}"),
        // Un blob court est presque toujours un pointeur encodé (ex. `ReferenceItems` = 4 octets
        // BE : type + index) : l'imprimer en hexa est le seul moyen d'établir la chaîne
        // cue → synth → waveform sur des données réelles.
        UtfValue::Bytes(b) if b.len() <= 8 => {
            format!(
                "<{}: {}>",
                b.len(),
                b.iter().map(|x| format!("{x:02x}")).collect::<String>()
            )
        }
        UtfValue::Bytes(b) => format!("<{} octets>", b.len()),
        autre => format!("{autre:?}"),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(chemin) = args.next() else {
        eprintln!("usage : probe_acb <chemin/vfs.acb> [sous-table]");
        std::process::exit(2);
    };
    let filtre = args.next();

    let racine = nie_formats::vfs::resolve_game_dir();
    let mut vfs = Vfs::new();
    vfs.init(racine.join("data")).expect("init VFS");
    let data = vfs.read(&chemin).expect("lecture ACB");
    println!("== {chemin} ({} octets)", data.len());

    let table = parse_utf(&data).expect("parse @UTF racine");
    println!(
        "table racine `{}` : {} colonnes, {} ligne(s)",
        table.name,
        table.columns.len(),
        table.rows.len()
    );

    // Colonnes de la racine : on n'imprime que celles qui portent quelque chose, sinon le bruit
    // des ~96 colonnes vides noie le signal.
    for col in &table.columns {
        let Some(v) = table.get(0, &col.name) else {
            continue;
        };
        let est_vide = matches!(v, UtfValue::Bytes(b) if b.is_empty());
        if est_vide {
            continue;
        }
        println!("  {:<28} {}", col.name, apercu(v));
    }

    // Sous-tables : toute colonne `Bytes` qui commence par `@UTF`.
    for col in &table.columns {
        if let Some(f) = &filtre
            && &col.name != f
        {
            continue;
        }
        let Some(UtfValue::Bytes(b)) = table.get(0, &col.name) else {
            continue;
        };
        if !b.starts_with(b"@UTF") {
            continue;
        }
        let Ok(sub) = parse_utf(b) else {
            println!("\n-- {} : @UTF ILLISIBLE ({} octets)", col.name, b.len());
            continue;
        };
        println!(
            "\n-- {} : {} colonnes, {} lignes",
            col.name,
            sub.columns.len(),
            sub.rows.len()
        );
        println!(
            "   colonnes : {}",
            sub.columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        for r in 0..sub.rows.len().min(4) {
            let ligne: Vec<String> = sub
                .columns
                .iter()
                .map(|c| sub.get(r, &c.name).map_or_else(|| "-".to_string(), apercu))
                .collect();
            println!("   [{r}] {}", ligne.join(" | "));
        }
    }
}
