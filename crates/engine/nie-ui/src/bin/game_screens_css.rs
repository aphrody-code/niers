//! `game_screens_css` — écrit ou vérifie `packages/inacord-ui/src/shell/game-screens.css`.
//!
//! ```sh
//! cargo run -p nie-ui --bin game_screens_css -- --write    # (ré)écrit le fichier
//! cargo run -p nie-ui --bin game_screens_css -- --verify   # 0 si identique, 1 sinon
//! ```
//!
//! Le texte vient entièrement de `nie_ui::css::screens_block()` ; ce binaire ne compose rien.

use std::process::ExitCode;

const AIDE: &str = "\
game_screens_css — la feuille des surfaces d'ecran, engendree par nie-ui

  game_screens_css --write     ecrit packages/inacord-ui/src/shell/game-screens.css
  game_screens_css --verify    compare le fichier au texte engendre (0 identique, 1 sinon)
  game_screens_css --print     imprime le texte engendre sur la sortie standard
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let chemin = nie_ui::css::game_screens_css_path();
    let genere = nie_ui::css::screens_block();
    match args.first().map(String::as_str) {
        Some("--write") => match std::fs::write(&chemin, genere.as_bytes()) {
            Ok(()) => {
                eprintln!("{} ecrit — {} octets, {} lignes", chemin.display(), genere.len(), genere.lines().count());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{} : {e}", chemin.display());
                ExitCode::FAILURE
            }
        },
        Some("--verify") => match std::fs::read(&chemin) {
            Ok(livre) if livre == genere.as_bytes() => {
                eprintln!("{} conforme — {} octets", chemin.display(), livre.len());
                ExitCode::SUCCESS
            }
            Ok(livre) => {
                let livre_txt = String::from_utf8_lossy(&livre);
                let premiere = livre_txt
                    .lines()
                    .zip(genere.lines())
                    .position(|(a, b)| a != b)
                    .map_or_else(
                        || format!("longueurs : {} livres, {} engendres", livre.len(), genere.len()),
                        |n| format!("premiere divergence ligne {}", n + 1),
                    );
                eprintln!("{} DIVERGE — {premiere}. Regenerer : --write", chemin.display());
                ExitCode::FAILURE
            }
            Err(e) => {
                eprintln!("{} : {e}", chemin.display());
                ExitCode::FAILURE
            }
        },
        Some("--print") => {
            print!("{genere}");
            ExitCode::SUCCESS
        }
        _ => {
            print!("{AIDE}");
            ExitCode::FAILURE
        }
    }
}
