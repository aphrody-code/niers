//! Les 17 jetons non colorés du jeu — géométrie, rythme, élévation, mouvement, typographie.
//!
//! Transposés de `packages/inacord-ui/src/shell/game-tokens.css`. Contrairement aux couleurs de
//! [`crate::color`], ces valeurs ne dérivent d'aucune mesure de palette — un biseau de 14 px ou
//! une durée de 120 ms ne se lisent pas sur un atlas de personnage. Elles sont donc recopiées
//! telles quelles depuis le CSS, comme le fait déjà `nie_aphrody::design::socle_css` côté
//! générateur existant (voir `crate::css`).
//!
//! Seule exception : les trois valeurs d'**élévation** ([`TILE_SHADOW`], [`PANEL_SHADOW`],
//! [`ACCENT_GLOW`]) sont composites — une géométrie fixe (décalages, flou) plus une couleur.
//! Cette couleur n'est pas recopiée en hexadécimal : elle est lue sur le [`crate::color::ColorToken`]
//! qui la porte déjà, pour qu'une seule ligne du dépôt connaisse chaque hexadécimal.

use crate::color::{ABYSS_BACKGROUND, ColorToken, CYAN_ACCENT, hex_to_rgb_triplet};

/// Un jeton non coloré : un nom de propriété CSS et sa valeur, déjà unitée comme le veut le CSS
/// (`"14px"`, `"120ms"`, `"0.02em"`…). Une seule forme sert toutes les unités : la valeur est la
/// source de vérité, pas un type distinct par unité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawToken {
    /// Le nom de la propriété CSS personnalisée, sans les deux tirets.
    pub name: &'static str,
    /// La valeur CSS telle qu'écrite dans `game-tokens.css`.
    pub value: &'static str,
}

impl RawToken {
    /// La ligne `:root` de ce jeton : `\t--nom: valeur;`.
    #[must_use]
    pub fn css_line(&self) -> String {
        format!("\t--{}: {};", self.name, self.value)
    }
}

macro_rules! raw_token {
    ($doc:literal, $ident:ident, $name:literal, $value:literal) => {
        #[doc = $doc]
        pub const $ident: RawToken = RawToken { name: $name, value: $value };
    };
}

// --- Géométrie : les tuiles du menu sont biseautées, pas rectangulaires ------------------------
raw_token!(
    "Le biseau des tuiles du menu — elles ne sont pas rectangulaires. Valeur CSS `14px`.",
    BEVEL, "jeu-biseau", "14px"
);
raw_token!("Le rayon de bordure par défaut. Valeur CSS `4px`.", RADIUS, "jeu-rayon", "4px");
raw_token!(
    "L'épaisseur de bordure par défaut. Valeur CSS `2px`.",
    BORDER_WIDTH, "jeu-bordure", "2px"
);

// --- Rythme --------------------------------------------------------------------------------
raw_token!("L'espacement le plus fin. Valeur CSS `4px`.", SPACE_XS, "jeu-espace-xs", "4px");
raw_token!("Un petit espacement. Valeur CSS `8px`.", SPACE_S, "jeu-espace-s", "8px");
raw_token!("L'espacement de référence. Valeur CSS `16px`.", SPACE_M, "jeu-espace-m", "16px");
raw_token!("Un grand espacement. Valeur CSS `24px`.", SPACE_L, "jeu-espace-l", "24px");
raw_token!("Le plus grand espacement. Valeur CSS `40px`.", SPACE_XL, "jeu-espace-xl", "40px");

// --- Mouvement : court et net, comme le jeu -------------------------------------------------
raw_token!(
    "La durée d'une transition rapide. Valeur CSS `120ms`.",
    FAST_DURATION, "jeu-duree-rapide", "120ms"
);
raw_token!(
    "La durée d'une transition moyenne. Valeur CSS `220ms`.",
    MEDIUM_DURATION, "jeu-duree-moyenne", "220ms"
);
raw_token!(
    "La courbe d'accélération des transitions. Valeur CSS `cubic-bezier(0.2, 0, 0, 1)`.",
    EASING_CURVE, "jeu-courbe", "cubic-bezier(0.2, 0, 0, 1)"
);

// --- Typographie -----------------------------------------------------------------------------
raw_token!("La graisse des titres. Valeur CSS `800`.", TITLE_WEIGHT, "jeu-titre-poids", "800");
raw_token!(
    "L'espacement des lettres d'un titre. Valeur CSS `0.02em`.",
    TITLE_TRACKING, "jeu-titre-espacement", "0.02em"
);
raw_token!(
    "L'espacement des lettres d'un libellé. Valeur CSS `0.06em`.",
    LABEL_TRACKING, "jeu-libelle-espacement", "0.06em"
);

/// Les 14 jetons non colorés à valeur fixe (hors élévation), dans l'ordre exact du CSS.
pub const RAW_TOKENS: [RawToken; 14] = [
    BEVEL, RADIUS, BORDER_WIDTH, SPACE_XS, SPACE_S, SPACE_M, SPACE_L, SPACE_XL, FAST_DURATION,
    MEDIUM_DURATION, EASING_CURVE, TITLE_WEIGHT, TITLE_TRACKING, LABEL_TRACKING,
];

/// Une ombre ou une lueur : une géométrie fixe (décalages, flou) plus la couleur d'un jeton
/// existant — jamais un hexadécimal recopié une deuxième fois.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElevationToken {
    /// Le nom de la propriété CSS personnalisée, sans les deux tirets.
    pub name: &'static str,
    /// Le début de la valeur CSS : décalages puis flou, avant le `rgb()` (ex. `"0 2px 8px"`).
    pub geometry: &'static str,
    /// Le jeton dont l'hexadécimal alimente la composante `rgb()`.
    pub color: ColorToken,
    /// L'opacité de la couleur, en pourcent entier, telle qu'écrite dans le CSS.
    pub alpha_pct: u8,
}

impl ElevationToken {
    /// La valeur CSS complète : `"{géométrie} rgb(r g b / {alpha}%)"`.
    #[must_use]
    pub fn css_value(&self) -> String {
        let [r, g, b] = hex_to_rgb_triplet(self.color.hex);
        format!("{} rgb({r} {g} {b} / {}%)", self.geometry, self.alpha_pct)
    }

    /// La ligne `:root` de ce jeton : `\t--nom: valeur;`.
    #[must_use]
    pub fn css_line(&self) -> String {
        format!("\t--{}: {};", self.name, self.css_value())
    }
}

// --- Élévation : la géométrie est écrite, les composantes dérivent des rôles -------------------
/// L'ombre portée d'une tuile du menu. Géométrie `0 2px 8px`, couleur = [`ABYSS_BACKGROUND`]
/// (`#131420`) à 45 % — dans le CSS livré : `rgb(19 20 32 / 45%)`.
pub const TILE_SHADOW: ElevationToken =
    ElevationToken { name: "jeu-ombre-tuile", geometry: "0 2px 8px", color: ABYSS_BACKGROUND, alpha_pct: 45 };
/// L'ombre portée d'un panneau. Géométrie `0 8px 32px`, couleur = [`ABYSS_BACKGROUND`]
/// (`#131420`) à 65 % — dans le CSS livré : `rgb(19 20 32 / 65%)`.
pub const PANEL_SHADOW: ElevationToken =
    ElevationToken { name: "jeu-ombre-panneau", geometry: "0 8px 32px", color: ABYSS_BACKGROUND, alpha_pct: 65 };
/// La lueur d'un état actif. Géométrie `0 0 12px`, couleur = [`CYAN_ACCENT`] (`#97c0fc`) à 55 %
/// — dans le CSS livré : `rgb(151 192 252 / 55%)`.
pub const ACCENT_GLOW: ElevationToken =
    ElevationToken { name: "jeu-lueur-accent", geometry: "0 0 12px", color: CYAN_ACCENT, alpha_pct: 55 };

/// Les trois jetons d'élévation, dans l'ordre exact du CSS.
pub const ELEVATION_TOKENS: [ElevationToken; 3] = [TILE_SHADOW, PANEL_SHADOW, ACCENT_GLOW];

#[cfg(test)]
mod tests {
    use super::{ACCENT_GLOW, PANEL_SHADOW, RAW_TOKENS, TILE_SHADOW};

    #[test]
    fn les_valeurs_delevation_correspondent_au_css_livre() {
        // Golden littéral, indépendant de `crate::css` : reproduit les trois lignes exactes du
        // fichier livré pour prouver que la composition (géométrie + couleur du jeton) suffit.
        assert_eq!(TILE_SHADOW.css_value(), "0 2px 8px rgb(19 20 32 / 45%)");
        assert_eq!(PANEL_SHADOW.css_value(), "0 8px 32px rgb(19 20 32 / 65%)");
        assert_eq!(ACCENT_GLOW.css_value(), "0 0 12px rgb(151 192 252 / 55%)");
    }

    #[test]
    fn aucun_nom_de_jeton_brut_nest_duplique() {
        let mut noms: Vec<&str> = RAW_TOKENS.iter().map(|t| t.name).collect();
        let avant = noms.len();
        noms.sort_unstable();
        noms.dedup();
        assert_eq!(avant, noms.len());
    }
}
