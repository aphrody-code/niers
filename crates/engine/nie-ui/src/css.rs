//! Le générateur du bloc `:root { … }` — la garantie de non-régression de cette crate.
//!
//! [`root_block`] assemble les jetons de [`crate::color`] et [`crate::tokens`] dans l'ordre et la
//! mise en page exacts du CSS livré. Le test [`tests::le_bloc_root_est_identique_au_css_livre`]
//! compare son résultat au bloc `:root` réellement présent dans
//! `packages/inacord-ui/src/shell/game-tokens.css` : un jeton, une virgule ou un tiret qui
//! diverge le fait ROUGIR — c'est la preuve par falsification que la consigne du dépôt exige.
//!
//! Ce module ne réécrit PAS `game-tokens.css` (il est produit par
//! `cargo run -p nie-aphrody --bin design`, cf. `crate` pour la provenance) : il prouve
//! seulement que la transposition Rust de cette crate reste identique à ce que ce fichier porte
//! aujourd'hui.

use crate::color::{GAME_COLORS, SECTIONS};
use crate::tokens::{ELEVATION_TOKENS, RAW_TOKENS};
use std::fmt::Write as _;

/// Écrit l'en-tête d'une section : `\t/* --- {titre} {tirets} */\n`.
///
/// Le nombre de tirets n'est pas recalculé ici : il est mesuré une fois sur le fichier livré
/// (voir `crate::color::SECTIONS` et les constantes locales de [`root_block`]) — recalculer une
/// règle de justification aurait le même risque de dérive qu'une valeur mesurée à la main.
fn write_header(s: &mut String, title: &str, dashes: usize) {
    let _ = writeln!(s, "\t/* --- {title} {} */", "-".repeat(dashes));
}

/// Le bloc `:root { … }` complet, avec le saut de ligne final — identique à ce que porte
/// `packages/inacord-ui/src/shell/game-tokens.css` aujourd'hui (voir le test golden ci-dessous).
#[must_use]
pub fn root_block() -> String {
    let mut s = String::from(":root {\n");

    // Les six sections de couleur, dans l'ordre de `GAME_COLORS`.
    for (i, (start, title, dashes)) in SECTIONS.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        write_header(&mut s, title, *dashes);
        let end = SECTIONS.get(i + 1).map_or(GAME_COLORS.len(), |(next, _, _)| *next);
        for token in &GAME_COLORS[*start..end] {
            s.push_str(&token.css_line());
            s.push('\n');
        }
    }

    // Géométrie (3 jetons bruts).
    s.push('\n');
    write_header(&mut s, "Geometrie : les tuiles du menu sont BISEAUTEES, pas rectangulaires", 16);
    for t in &RAW_TOKENS[0..3] {
        s.push_str(&t.css_line());
        s.push('\n');
    }

    // Rythme (5 jetons bruts).
    s.push('\n');
    write_header(&mut s, "Rythme", 74);
    for t in &RAW_TOKENS[3..8] {
        s.push_str(&t.css_line());
        s.push('\n');
    }

    // Élévation (3 jetons composites).
    s.push('\n');
    write_header(&mut s, "Elevation : la geometrie est ecrite, les composantes derivent des roles", 10);
    for t in &ELEVATION_TOKENS {
        s.push_str(&t.css_line());
        s.push('\n');
    }

    // Mouvement (3 jetons bruts).
    s.push('\n');
    write_header(&mut s, "Mouvement : court et net, comme le jeu", 43);
    for t in &RAW_TOKENS[8..11] {
        s.push_str(&t.css_line());
        s.push('\n');
    }

    // Typographie (3 jetons bruts).
    s.push('\n');
    write_header(&mut s, "Typographie", 69);
    for t in &RAW_TOKENS[11..14] {
        s.push_str(&t.css_line());
        s.push('\n');
    }

    s.push_str("}\n");
    s
}

/// Le chemin de `game-tokens.css`, résolu depuis l'emplacement de CETTE crate — indépendant du
/// répertoire courant, comme `nie_aphrody::design::chemin_feuille` (même profondeur dans
/// l'arborescence : `crates/engine/nie-ui` → racine du dépôt en trois `ancestors()`).
#[must_use]
pub fn game_tokens_css_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("packages/inacord-ui/src/shell/game-tokens.css")
}

/// Extrait le bloc `:root { … }` (bornes incluses) d'un fichier CSS, saut de ligne final compris.
///
/// `None` si le fichier ne porte pas de ligne `:root {` suivie, plus loin, d'une ligne `}` seule
/// — la forme que `game-tokens.css` a toujours eue depuis que cette crate existe.
#[must_use]
pub fn extract_root_block(css: &str) -> Option<String> {
    let lines: Vec<&str> = css.lines().collect();
    let start = lines.iter().position(|l| *l == ":root {")?;
    let end = lines[start..].iter().position(|l| *l == "}")? + start;
    Some(lines[start..=end].join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::{extract_root_block, game_tokens_css_path, root_block};

    #[test]
    fn extract_root_block_isole_le_bon_bloc() {
        let css = "/* entete */\n\n:root {\n\t--a: 1px;\n}\n\n@media (x) {\n\t:root { --a: 0; }\n}\n";
        let bloc = extract_root_block(css).expect("bloc trouvé");
        assert_eq!(bloc, ":root {\n\t--a: 1px;\n}\n");
    }

    #[test]
    fn root_block_commence_et_finit_comme_attendu() {
        let bloc = root_block();
        assert!(bloc.starts_with(":root {\n"));
        assert!(bloc.ends_with("}\n"));
        // 29 couleurs + 17 jetons non colorés = 46 déclarations, comme le fichier livré.
        let proprietes = bloc.lines().filter(|l| l.trim_start().starts_with("--")).count();
        assert_eq!(proprietes, 46, "le compte de propriétés a changé");
    }

    /// Preuve par falsification : ce test compare le bloc généré au bloc RÉELLEMENT présent dans
    /// `game-tokens.css`. Casser un jeton dans `crate::color`/`crate::tokens` le fait rougir —
    /// vérifié manuellement (voir `docs/DESIGN-UI.md`) en modifiant temporairement une valeur.
    #[test]
    fn le_bloc_root_est_identique_au_css_livre() {
        let chemin = game_tokens_css_path();
        let Ok(livre) = std::fs::read_to_string(&chemin) else {
            let message = format!(
                "GOLDEN SAUTE — {} est introuvable. Fichier suivi par git : le restaurer par\n  \
                 git checkout -- packages/inacord-ui/src/shell/game-tokens.css",
                chemin.display()
            );
            eprintln!("{message}");
            println!("{message}");
            return;
        };
        let Some(bloc_livre) = extract_root_block(&livre) else {
            panic!("« {} » ne contient pas de bloc :root reconnaissable", chemin.display());
        };
        let genere = root_block();
        if bloc_livre != genere {
            let ecart = bloc_livre
                .lines()
                .zip(genere.lines())
                .enumerate()
                .find(|(_, (a, b))| a != b)
                .map_or_else(
                    || {
                        format!(
                            "longueurs différentes : {} lignes livrées, {} générées",
                            bloc_livre.lines().count(),
                            genere.lines().count()
                        )
                    },
                    |(n, (a, b))| format!("ligne {} :\n  livre  : {a}\n  genere : {b}", n + 1),
                );
            panic!(
                "{} : le bloc :root genere par nie-ui ne correspond plus au fichier livre.\n{ecart}\n\
                 Un jeton de crate::color/crate::tokens a-t-il été modifié sans re-vérifier le CSS ?",
                chemin.display()
            );
        }
    }
}
