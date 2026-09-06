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

/// Le chemin de `game-screens.css`, résolu comme [`game_tokens_css_path`].
#[must_use]
pub fn game_screens_css_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("packages/inacord-ui/src/shell/game-screens.css")
}

/// Le texte ENTIER de `packages/inacord-ui/src/shell/game-screens.css` : l'en-tête de
/// provenance, le bloc `:root` des `--screen-*` mesurés ([`crate::surfaces::SCREEN_COLORS`]),
/// `--game-skew` et les longueurs, puis les règles de classes ([`crate::surfaces::RULES`]).
///
/// Écrit par `cargo run -p nie-ui --bin game_screens_css -- --write`, prouvé octet à octet par
/// [`tests::game_screens_css_est_identique_au_fichier_livre`].
#[must_use]
pub fn screens_block() -> String {
    use crate::surfaces::{RULES, SCREEN_COLORS, SCREEN_LENGTHS, SCREEN_SECTIONS, SKEW_CSS, SLANT_SAMPLES};
    let mut s = String::from(
        "/*\n\
         \x20* game-screens.css — les surfaces des ecrans du jeu, MESUREES sur les captures de data/menu.\n\
         \x20*\n\
         \x20* FICHIER ENGENDRE — ne pas retoucher a la main.\n\
         \x20*   Regenerer :  cargo run -p nie-ui --bin game_screens_css -- --write\n\
         \x20*   Verifier   :  cargo run -p nie-ui --bin game_screens_css -- --verify\n\
         \x20*   Source     :  crates/engine/nie-ui/src/surfaces.rs (constantes) + css.rs (assemblage)\n\
         \x20*   Instrument :  cargo run -p nie-aphrody --bin pixel -- capture data/menu/<png> --crop X,Y,W,H --k N\n\
         \x20*\n\
         \x20* Chaque --screen-* cite la capture (2560x1440), le recadrage --crop, le --k et la part de la\n\
         \x20* classe k-means Oklab retenue. Les roles deja servis par game-tokens.css (typographie, rythme,\n\
         \x20* ombres, texte) sont repris par var(--jeu-*) : cette feuille la complete, elle ne la remplace pas.\n\
         \x20* Consommateurs : packages/inacord-ui/src/components/game/** et apps/nie-web.\n\
         \x20*/\n\n:root {\n",
    );
    for (i, (start, title)) in SCREEN_SECTIONS.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        let _ = writeln!(s, "\t/* --- {title} --- */");
        let end = SCREEN_SECTIONS.get(i + 1).map_or(SCREEN_COLORS.len(), |(next, _)| *next);
        for c in &SCREEN_COLORS[*start..end] {
            s.push_str(&c.css_line());
            s.push('\n');
        }
    }
    s.push_str("\n\t/* --- Geometrie mesuree --- */\n");
    for sample in &SLANT_SAMPLES {
        let _ = writeln!(
            s,
            "\t/* {} : bord gauche {:.2} deg, bord droit {:.2} deg, R2 {:.3} — {} */",
            sample.capture, sample.left_deg, sample.right_deg, sample.r2, sample.command
        );
    }
    let _ = writeln!(s, "\t--game-skew: {SKEW_CSS};  /* moyenne des quatre bords ajustes : -10.02 deg */");
    for (name, value, provenance) in &SCREEN_LENGTHS {
        let _ = writeln!(s, "\t--{name}: {value};  /* {provenance} */");
    }
    s.push_str("}\n");
    s.push_str(RULES);
    s
}

#[cfg(test)]
mod tests {
    use super::{extract_root_block, game_screens_css_path, game_tokens_css_path, root_block, screens_block};

    #[test]
    fn screens_block_porte_les_45_couleurs_le_skew_et_les_classes_du_contrat() {
        let css = screens_block();
        let screen_props = css.lines().filter(|l| l.trim_start().starts_with("--screen-")).count();
        // 45 couleurs + 5 longueurs.
        assert_eq!(screen_props, 50, "le compte de propriétés --screen-* a changé");
        assert!(css.contains("\t--game-skew: -10deg;"));
        for classe in [
            ".game-header-bar", ".game-header-bar__icon", ".game-header-bar__title",
            ".game-tab-strip", ".game-tab", ".game-tab--active", ".game-tab-strip__key",
            ".game-panel", ".game-panel__title", ".game-panel__body", ".game-panel__footer", ".game-panel__watermark",
            ".game-check", ".game-check__box", ".game-check__label", ".game-check--checked",
            ".game-icon-chip",
            ".game-setting-row", ".game-setting-row--focused", ".game-setting-row__label",
            ".game-setting-row__value", ".game-setting-row__arrow", ".game-setting-row__more",
            ".game-setting-list", ".game-setting-list__scrollbar",
            ".game-button-primary", ".game-button-secondary",
            ".game-key-cap", ".game-key-hint", ".game-hint-bar",
            ".game-cursor",
            ".game-tile-row", ".game-tile", ".game-tile__icon", ".game-tile--active",
            ".game-search-bar", ".game-search-bar__input", ".game-search-bar__key",
            ".game-description-bar", ".game-count-badge",
            ".game-info-window", ".game-info-window__title", ".game-skew",
        ] {
            assert!(
                css.contains(&format!("{classe} {{")) || css.contains(&format!("{classe},")) || css.contains(&format!("{classe} .")),
                "classe absente du contrat : {classe}"
            );
        }
        // Chaque var(--screen-*) employée par une règle est déclarée dans le :root.
        for var in css.split("var(--screen-").skip(1) {
            let nom = var.split([')', ',']).next().unwrap_or("");
            assert!(css.contains(&format!("\t--screen-{nom}:")), "var(--screen-{nom}) non déclarée");
        }
        // Aucune couleur hexadécimale nue hors commentaire : tout passe par var().
        for ligne in css.lines().filter(|l| !l.trim_start().starts_with("/*") && !l.trim_start().starts_with('*')) {
            let code = ligne.split("/*").next().unwrap_or("");
            assert!(!code.contains('#'), "couleur nue hors var() : {ligne}");
        }
    }

    /// Golden : le fichier livré est identique, octet pour octet, à `screens_block()`.
    /// Falsification documentée dans `docs/DESIGN-UI.md` (casser une constante de `surfaces`,
    /// rouge ; restaurer par copie, vert).
    #[test]
    fn game_screens_css_est_identique_au_fichier_livre() {
        let chemin = game_screens_css_path();
        let Ok(livre) = std::fs::read(&chemin) else {
            let message = format!(
                "GOLDEN SAUTE — {} est introuvable. Le régénérer par\n  \
                 cargo run -p nie-ui --bin game_screens_css -- --write",
                chemin.display()
            );
            eprintln!("{message}");
            println!("{message}");
            return;
        };
        let genere = screens_block();
        if livre != genere.as_bytes() {
            let livre_txt = String::from_utf8_lossy(&livre);
            let ecart = livre_txt
                .lines()
                .zip(genere.lines())
                .enumerate()
                .find(|(_, (a, b))| a != b)
                .map_or_else(
                    || format!("longueurs différentes : {} octets livrés, {} générés", livre.len(), genere.len()),
                    |(n, (a, b))| format!("ligne {} :\n  livre  : {a}\n  genere : {b}", n + 1),
                );
            panic!(
                "{} : game-screens.css ne correspond plus à nie_ui::css::screens_block().\n{ecart}\n\
                 Régénérer par `cargo run -p nie-ui --bin game_screens_css -- --write`.",
                chemin.display()
            );
        }
    }

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
