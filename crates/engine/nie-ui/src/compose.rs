//! Une API de composition minimale : décrire une tuile, un panneau ou la plaque du menu à
//! partir des jetons qui portent déjà ce rôle — rien n'y est inventé sur ce que le jeu fait.
//!
//! Le CSS ne nomme que deux familles de surface cliquable et une surface singulière :
//! - la **tuile** (`--jeu-tuile-*`) — [`TileStyle`], [`TILE`], [`ACTIVE_TILE`] ;
//! - le **panneau** (`--jeu-fond-*` + `--jeu-ombre-panneau`) — [`PanelStyle`] ;
//! - la **plaque centrale**, unique (`--jeu-plaque-bleu` + `--jeu-lisere-or`) — [`PlateStyle`],
//!   [`PLATE`].
//!
//! Il n'existe **pas** de jeton `--jeu-bouton-*` dans `game-tokens.css`. Dans ce jeu, un bouton
//! du menu est visuellement une tuile : modéliser un `ButtonStyle` séparé inventerait une
//! distinction que le CSS ne fait pas. Un hôte qui a besoin d'un style de bouton réutilise
//! [`TILE`]/[`ACTIVE_TILE`] — voir `docs/DESIGN-UI.md`.

use crate::color::{
    ACTIVE_TILE_BOTTOM, ACTIVE_TILE_TOP, BLUE_PLATE, ColorToken, DEEP_BACKGROUND, GOLD_TRIM,
    NIGHT_BACKGROUND, TILE_BORDER, TILE_BOTTOM, TILE_TOP,
};
use crate::tokens::{BEVEL, BORDER_WIDTH, ElevationToken, PANEL_SHADOW, RADIUS, RawToken, TILE_SHADOW};

/// La composition visuelle d'une tuile du menu : dégradé haut/bas, bordure, biseau, ombre.
/// Chaque champ pointe vers le jeton qui porte déjà ce rôle — aucune valeur n'est nouvelle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileStyle {
    /// Le haut du dégradé de fond.
    pub gradient_top: ColorToken,
    /// Le bas du dégradé de fond.
    pub gradient_bottom: ColorToken,
    /// La couleur de bordure.
    pub border_color: ColorToken,
    /// L'épaisseur de bordure.
    pub border_width: RawToken,
    /// Le rayon des coins.
    pub radius: RawToken,
    /// Le biseau : les tuiles du menu ne sont pas rectangulaires (cf. `crate::tokens::BEVEL`).
    pub bevel: RawToken,
    /// L'ombre portée.
    pub shadow: ElevationToken,
}

/// La tuile au repos — `--jeu-tuile-haut`/`-bas`/`-bord`.
pub const TILE: TileStyle = TileStyle {
    gradient_top: TILE_TOP,
    gradient_bottom: TILE_BOTTOM,
    border_color: TILE_BORDER,
    border_width: BORDER_WIDTH,
    radius: RADIUS,
    bevel: BEVEL,
    shadow: TILE_SHADOW,
};

/// La tuile active ou survolée — `--jeu-tuile-active-haut`/`-bas` ; même bordure, même géométrie
/// que [`TILE`], seul le dégradé change (c'est ce que dit le CSS : la bordure n'a pas de variante
/// « active »).
pub const ACTIVE_TILE: TileStyle = TileStyle {
    gradient_top: ACTIVE_TILE_TOP,
    gradient_bottom: ACTIVE_TILE_BOTTOM,
    border_color: TILE_BORDER,
    border_width: BORDER_WIDTH,
    radius: RADIUS,
    bevel: BEVEL,
    shadow: TILE_SHADOW,
};

/// La composition visuelle d'un panneau : un fond uni profond et une ombre plus large que celle
/// d'une tuile (`--jeu-ombre-panneau` : décalage et flou supérieurs à `--jeu-ombre-tuile`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelStyle {
    /// Le fond du panneau.
    pub background: ColorToken,
    /// L'ombre portée.
    pub shadow: ElevationToken,
    /// Le rayon des coins (le CSS n'a qu'un seul rayon, partagé avec les tuiles).
    pub radius: RawToken,
}

/// Un panneau sur `--jeu-fond-profond` — la profondeur médiane des trois que le CSS propose
/// (`--jeu-fond-nuit` le plus sombre, `--jeu-fond-moyen` le plus clair).
pub const PANEL_DEEP: PanelStyle =
    PanelStyle { background: DEEP_BACKGROUND, shadow: PANEL_SHADOW, radius: RADIUS };
/// Un panneau sur `--jeu-fond-nuit`, le plus sombre des trois profondeurs.
pub const PANEL_NIGHT: PanelStyle =
    PanelStyle { background: NIGHT_BACKGROUND, shadow: PANEL_SHADOW, radius: RADIUS };

/// La plaque centrale du menu — le CSS ne la commente qu'une fois (« la plaque centrale ») et ne
/// décrit qu'un seul exemplaire, borné d'un liseré doré plutôt que d'une bordure ordinaire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateStyle {
    /// Le fond de la plaque.
    pub background: ColorToken,
    /// Le liseré doré qui la borde.
    pub trim: ColorToken,
    /// Le rayon des coins.
    pub radius: RawToken,
}

/// L'unique plaque décrite par le CSS actuel — `--jeu-plaque-bleu` + `--jeu-lisere-or`.
pub const PLATE: PlateStyle = PlateStyle { background: BLUE_PLATE, trim: GOLD_TRIM, radius: RADIUS };

#[cfg(test)]
mod tests {
    use super::{ACTIVE_TILE, PANEL_DEEP, PANEL_NIGHT, PLATE, TILE};

    #[test]
    fn la_tuile_pointe_vers_les_bons_jetons() {
        assert_eq!(TILE.gradient_top.name, "jeu-tuile-haut");
        assert_eq!(TILE.gradient_bottom.name, "jeu-tuile-bas");
        assert_eq!(TILE.border_color.name, "jeu-tuile-bord");
        assert_eq!(TILE.shadow.name, "jeu-ombre-tuile");
    }

    #[test]
    fn la_tuile_active_garde_la_meme_bordure_que_la_tuile_au_repos() {
        assert_eq!(ACTIVE_TILE.gradient_top.name, "jeu-tuile-active-haut");
        assert_eq!(ACTIVE_TILE.gradient_bottom.name, "jeu-tuile-active-bas");
        assert_eq!(ACTIVE_TILE.border_color, TILE.border_color, "aucune bordure « active » dans le CSS");
    }

    #[test]
    fn les_panneaux_partagent_lombre_de_panneau_pas_celle_dune_tuile() {
        assert_eq!(PANEL_DEEP.shadow.name, "jeu-ombre-panneau");
        assert_eq!(PANEL_NIGHT.shadow.name, "jeu-ombre-panneau");
        assert_ne!(PANEL_DEEP.background, PANEL_NIGHT.background);
    }

    #[test]
    fn la_plaque_porte_un_lisere_pas_une_bordure() {
        assert_eq!(PLATE.background.name, "jeu-plaque-bleu");
        assert_eq!(PLATE.trim.name, "jeu-lisere-or");
    }
}
