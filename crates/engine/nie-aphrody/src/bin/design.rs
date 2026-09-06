//! `design` — engendre `packages/inacord-ui/src/shell/game-tokens.css` depuis la palette mesurée.
//!
//! Le site n'a plus qu'une seule source de couleur : [`nie_aphrody::design`]. Ce binaire est le
//! seul chemin par lequel elle atteint le CSS, et il est **idempotent** — il ne réécrit le
//! fichier que si son contenu change, pour qu'un lancement à vide ne salisse ni `git status` ni
//! l'horodatage que surveille le serveur de développement.
//!
//! ```text
//! cargo run -p nie-aphrody --bin design              # ecrit la feuille
//! cargo run -p nie-aphrody --bin design --verifier   # n'ecrit rien, sort en 1 si ca differe
//! cargo run -p nie-aphrody --bin design <chemin>     # vers un autre fichier
//! ```
//!
//! Il affiche aussi ce qu'il a produit : les vingt-neuf rôles avec leur teinte source et leur
//! hexadécimal, la marge au gamut sRGB, et le contraste WCAG des paires que l'interface
//! superpose réellement. C'est ce tableau qui permet de juger la lisibilité sans ouvrir un
//! navigateur — et, au premier passage, de voir ce que chaque couleur écrite à la main devient.

use nie_aphrody::design::{
    self, PAIRES, PALETTE, ROLES, ROLES_INACORD, Role, chemin_feuille, contrastes, fichier_css,
};
use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let mut verifier = false;
    let mut chemin: Option<PathBuf> = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--verifier" | "--check" => verifier = true,
            "-h" | "--help" => {
                println!("{}", aide());
                return ExitCode::SUCCESS;
            }
            autre if autre.starts_with('-') => {
                eprintln!("option inconnue : {autre}\n\n{}", aide());
                return ExitCode::FAILURE;
            }
            autre => chemin = Some(PathBuf::from(autre)),
        }
    }
    let chemin = chemin.unwrap_or_else(chemin_feuille);

    // Lu AVANT d'écrire : c'est la seule occasion de dire ce que chaque couleur écrite à la main
    // devient une fois dérivée. Après le premier passage il n'y a plus d'hexadécimal à comparer,
    // et la colonne « avant » disparaît d'elle-même.
    let ancien = std::fs::read_to_string(&chemin).ok();
    let attendu = fichier_css();

    afficher_palette();
    afficher_roles(ancien.as_deref());
    afficher_contrastes();

    let identique = ancien.as_deref() == Some(attendu.as_str());
    if verifier {
        if identique {
            println!("\n{} : conforme.", chemin.display());
            return ExitCode::SUCCESS;
        }
        eprintln!(
            "\n{} : DIFFERE de ce que produit la palette. Regenerer par\n  \
             cargo run -p nie-aphrody --bin design",
            chemin.display()
        );
        return ExitCode::FAILURE;
    }
    if identique {
        println!("\n{} : inchange ({} proprietes).", chemin.display(), proprietes(&attendu));
        return ExitCode::SUCCESS;
    }
    if let Some(parent) = chemin.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("impossible de creer {} : {e}", parent.display());
        return ExitCode::FAILURE;
    }
    if let Err(e) = std::fs::write(&chemin, &attendu) {
        eprintln!("ecriture de {} impossible : {e}", chemin.display());
        return ExitCode::FAILURE;
    }
    println!(
        "\n{} : ecrit ({} octets, {} proprietes dont {} couleurs).",
        chemin.display(),
        attendu.len(),
        proprietes(&attendu),
        ROLES.len() + ROLES_INACORD.len()
    );
    ExitCode::SUCCESS
}

/// Le texte d'aide, dans la forme que les autres binaires de la crate emploient.
fn aide() -> String {
    format!(
        "design — engendre game-tokens.css depuis la palette mesuree d'Aphrody.\n\n\
         USAGE:\n  \
         design [--verifier] [<chemin>]\n\n\
         OPTIONS:\n  \
         --verifier   n'ecrit rien ; sort en 1 si le fichier differe de la palette\n  \
         -h, --help   cette aide\n\n\
         Par defaut, le chemin est resolu depuis l'emplacement de la crate :\n  {}\n",
        chemin_feuille().display()
    )
}

/// Compte les déclarations de propriété personnalisée d'une feuille.
fn proprietes(css: &str) -> usize {
    css.lines()
        .filter(|l| l.trim_start().starts_with("--"))
        .count()
}

/// Les dix teintes de départ : ce que la mesure a trouvé sur l'atlas.
fn afficher_palette() {
    println!("PALETTE MESUREE — 10 teintes, k-means Oklab sur les 74 frames de l'atlas\n");
    println!("  {:<8} {:<9} {:>6}   {:<28}", "teinte", "hex", "part", "oklch");
    println!("  {}", "-".repeat(58));
    for t in &PALETTE {
        println!(
            "  {:<8} {:<9} {:>5.0} %   oklch({:.4} {:.4} {:.2})",
            t.nom, t.hex, t.part_pct, t.l, t.c, t.h
        );
    }
}

/// Les vingt-neuf rôles dérivés, avec leur marge au gamut et, au premier passage, l'ancienne
/// valeur écrite à la main qu'ils remplacent.
fn afficher_roles(ancien: Option<&str>) {
    println!("\n\nROLES DERIVES — {} couleurs\n", ROLES.len() + ROLES_INACORD.len());
    println!(
        "  {:<24} {:<7} {:<28} {:<9} {:<9} marge gamut",
        "role", "source", "oklch", "avant", "apres"
    );
    println!("  {}", "-".repeat(96));
    for r in ROLES.iter().chain(ROLES_INACORD.iter()) {
        let oklch = r.oklch();
        let [l, c, h] = oklch;
        let apres = design::oklch_vers_hex(oklch);
        let avant = ancien.and_then(|css| hex_ecrit(css, r.nom));
        let marge = marge_gamut(r);
        println!(
            "  {:<24} {:<7} oklch({l:.4} {c:.4} {h:>6.2}) {:<9} {apres:<9} {marge:+.4}{}",
            r.nom,
            PALETTE[r.source].nom,
            avant.as_deref().unwrap_or("-"),
            if marge < 0.0 { "  HORS GAMUT" } else { "" }
        );
    }
}

/// La marge la plus faible d'un rôle aux bornes du gamut sRGB, en composante non écrêtée.
///
/// Positive, la couleur écrite est bien celle qui sera rendue. Négative, l'affichage l'écrête
/// et l'écart ne se voit sur aucune ligne de code — d'où l'affichage systématique du chiffre
/// plutôt qu'un simple « ok ».
fn marge_gamut(r: &Role) -> f32 {
    design::srgb_non_ecrete(r.oklch())
        .into_iter()
        .map(|v| v.min(1.0 - v))
        .fold(f32::INFINITY, f32::min)
}

/// L'hexadécimal écrit à la main pour cette propriété dans une feuille existante, s'il y en a un.
fn hex_ecrit(css: &str, nom: &str) -> Option<String> {
    let prefixe = format!("--{nom}:");
    css.lines()
        .map(str::trim_start)
        .find(|l| l.starts_with(&prefixe))?
        .split_once('#')
        .map(|(_, reste)| {
            let hex: String = reste.chars().take_while(char::is_ascii_hexdigit).collect();
            format!("#{hex}")
        })
        .filter(|h| h.len() == 7)
}

/// Le contraste WCAG des paires que l'interface superpose réellement.
fn afficher_contrastes() {
    println!("\n\nCONTRASTES WCAG — les {} paires que l'interface affiche\n", PAIRES.len());
    println!(
        "  {:<22} {:<22} {:>8} {:>8}   verdict",
        "texte", "sur fond", "mesure", "minimum"
    );
    println!("  {}", "-".repeat(78));
    for (a, b, mesure, min) in contrastes() {
        // AAA est le seuil WCAG du texte courant (7:1) ; il n'est pas exige ici, seulement
        // signale — plusieurs de ces paires sont des aplats, pas du corps de texte.
        let verdict = if mesure < min {
            "ECHEC"
        } else if mesure >= 7.0 {
            "AAA"
        } else {
            "AA"
        };
        println!("  {a:<22} {b:<22} {mesure:>7.2} {min:>7.1}   {verdict}");
    }
}
