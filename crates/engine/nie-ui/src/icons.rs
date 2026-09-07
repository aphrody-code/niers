//! Le pont entre un atlas d'icônes `.g4tx` et les jetons de cette crate.
//!
//! `nie_formats::sprite_sheet` transpose déjà un atlas `.g4tx` d'interface — les rectangles du
//! jeu, **recopiés**, jamais recalculés — en CSS/SVG/JSON exploitables (`depuis_g4tx`,
//! `SpriteSheet::vers_css`…). Ce module ne réimplémente ni le découpage d'atlas ni le calcul de
//! `background-position` : il APPELLE `nie_formats::sprite_sheet` et augmente son résultat avec
//! les jetons de mouvement/géométrie de cette crate — le point où les deux mondes que
//! l'orchestrateur a mesurés (les icônes du jeu, les jetons de son menu) se rejoignent.

use crate::color::AZURE_ACCENT;
use crate::tokens::{FAST_DURATION, RADIUS};
use nie_formats::sprite_sheet::{PREFIXE, SpriteSheet};

/// Le CSS d'une feuille de sprites d'interface, jetons compris.
///
/// Reprend tel quel le CSS des régions ([`SpriteSheet::vers_css`] — non retouché) puis étend la
/// classe de base commune à toutes les régions (`.{PREFIXE}-sprite`) avec l'habillage interactif
/// que cette crate mesure déjà : la durée de transition rapide, le rayon des tuiles, l'accent
/// azur comme couleur de focus. Rien n'y est neuf — ce sont les mêmes jetons que
/// [`crate::compose::TILE`] emploie pour une tuile du menu, posés ici sur une icône.
#[must_use]
pub fn icon_sheet_css(sheet: &SpriteSheet, image_url: &str) -> String {
    let regions = sheet.vers_css(image_url);
    format!(
        "{regions}\n\
         /* Habillage nie-ui : transition, rayon et anneau de focus, jetons du jeu. */\n\
         .{PREFIXE}-sprite {{\n\
         \ttransition: filter var(--{duree});\n\
         \tborder-radius: var(--{rayon});\n\
         \toutline: 2px solid transparent;\n\
         \toutline-offset: 2px;\n\
         }}\n\
         .{PREFIXE}-sprite:hover {{\n\
         \tfilter: brightness(1.08);\n\
         }}\n\
         .{PREFIXE}-sprite:focus-visible {{\n\
         \toutline-color: var(--{accent});\n\
         }}\n",
        duree = FAST_DURATION.name,
        rayon = RADIUS.name,
        accent = AZURE_ACCENT.name,
    )
}

#[cfg(test)]
mod tests {
    use super::icon_sheet_css;
    use nie_formats::sprite_sheet::PREFIXE;
    use std::path::Path;

    /// Golden VFS, comme `nie_wasm::tests_sprite_sheet::feuille_de_sprites_d_un_atlas_reel` :
    /// un vrai atlas d'interface du jeu (`data/dx11/font/gaiji_game.g4tx`, 110 régions mesurées
    /// par `niers vfs find`) doit produire un CSS exploitable. S'auto-saute, à voix haute, si le
    /// jeu n'est pas monté sur cette machine — jamais un vert silencieux.
    #[test]
    fn icon_sheet_css_habille_un_atlas_reel_du_jeu() {
        let dir = nie_formats::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = Path::new(&dir).join("data");
        let mut vfs = nie_formats::vfs::Vfs::new();
        if vfs.init(&data_dir).is_err() {
            eprintln!(
                "skip icon_sheet_css_habille_un_atlas_reel_du_jeu : jeu absent à {}",
                data_dir.display()
            );
            return;
        }
        let Some(chemin) = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .find(|p| p.ends_with("font/gaiji_game.g4tx"))
        else {
            eprintln!(
                "skip icon_sheet_css_habille_un_atlas_reel_du_jeu : gaiji_game.g4tx absent du VFS"
            );
            return;
        };
        let data = vfs.read(&chemin).expect("lecture de l'atlas");
        let atlas = nie_formats::g4tx::parse(&data).expect("gaiji_game.g4tx est un G4TX valide");
        let sheet =
            nie_formats::sprite_sheet::depuis_g4tx(&atlas, 0).expect("l'atlas porte une texture");

        let css = icon_sheet_css(&sheet, "/tex/dx11/font/gaiji_game.png");
        eprintln!(
            "{chemin} : {} régions, {} octets de CSS habillé",
            sheet.len(),
            css.len()
        );

        assert!(
            sheet.len() > 50,
            "atlas d'icônes attendu, {} régions",
            sheet.len()
        );
        assert!(css.contains("background-image: url(\"/tex/dx11/font/gaiji_game.png\")"));
        assert!(css.contains(&format!(".{PREFIXE}-sprite {{")));
        assert!(
            css.contains("var(--jeu-duree-rapide)"),
            "la durée rapide n'est pas posée"
        );
        assert!(css.contains("var(--jeu-rayon)"), "le rayon n'est pas posé");
        assert!(
            css.contains("var(--jeu-accent-azur)"),
            "l'accent de focus n'est pas posé"
        );
    }

    #[test]
    fn icon_sheet_css_sur_une_feuille_vide_ne_panique_pas() {
        // Une texture sans région (image simple) n'est pas une erreur pour
        // `nie_formats::sprite_sheet` (cf. sa doc) : le pont ne doit pas l'être non plus.
        let vide = empty_sprite_sheet();
        let css = icon_sheet_css(&vide, "/tex/exemple.png");
        assert!(css.contains(&format!(".{PREFIXE}-sprite {{")));
    }

    fn empty_sprite_sheet() -> nie_formats::sprite_sheet::SpriteSheet {
        nie_formats::sprite_sheet::SpriteSheet {
            nom: "exemple".to_string(),
            largeur: 4,
            hauteur: 4,
            sprites: Vec::new(),
        }
    }
}
