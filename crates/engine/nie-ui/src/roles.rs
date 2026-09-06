//! Les rôles sémantiques (shadcn + Material 3), mappés sur les jetons du jeu.
//!
//! Inspiré de la passerelle `@theme inline` d'`apps/nie-web/src/base.css` (lue, jamais modifiée
//! par cette crate — deux autres agents y travaillent). Ce fichier fait cohabiter deux
//! vocabulaires de rôle : shadcn (`background`, `muted-foreground`, `primary`…) pour les
//! primitives d'Inacord, Material 3 (`surface-container-high`, `on-surface-variant`…) pour les
//! composants venus du wiki. [`SEMANTIC_ROLES`] reprend la même correspondance, sous une forme
//! typée — sans en changer une seule valeur.
//!
//! Trois rôles n'y sont **pas** rattachés à un jeton mesuré : `base.css` les pose en dur à
//! `#fff` (`card`, `popover`, `surface-container-lowest`). [`RoleValue::Literal`] les marque
//! comme tels plutôt que de leur inventer un jeton — cette valeur n'a pas de commentaire de
//! provenance dans le CSS source, donc aucun n'est ajouté ici.

use crate::color::{
    AZURE_ACCENT, BRICK_ACCENT, CHALK_SURFACE, ColorToken, CLEAR_SKY, DEEP_NIGHT, ICE_SURFACE,
    MIST_SURFACE, SOFT_TEXT, TILE_BORDER, TILE_TOP,
};
use crate::tokens::{ElevationToken, RADIUS, RawToken, TILE_SHADOW};

/// La valeur d'un rôle sémantique : un jeton déjà mesuré, ou — pour les rôles que `base.css`
/// pose en dur — la valeur littérale, marquée comme telle plutôt que travestie en jeton.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoleValue {
    /// Reprend un jeton de couleur (rendu `var(--nom-du-jeton)`).
    Token(ColorToken),
    /// Reprend un jeton non coloré, ex. `radius` → `--jeu-rayon` (rendu `var(--nom-du-jeton)`).
    Raw(RawToken),
    /// Reprend un jeton d'élévation, ex. `shadow-level-1` → `--jeu-ombre-tuile`.
    Elevation(ElevationToken),
    /// `base.css` pose cette valeur en dur : ce n'est PAS un jeton mesuré.
    Literal(&'static str),
}

impl RoleValue {
    /// La valeur CSS de ce rôle, telle que `base.css` l'écrirait : une référence `var(--…)` vers
    /// le jeton, ou la valeur littérale.
    #[must_use]
    pub fn to_css_value(&self) -> String {
        match self {
            RoleValue::Token(t) => format!("var(--{})", t.name),
            RoleValue::Raw(t) => format!("var(--{})", t.name),
            RoleValue::Elevation(t) => format!("var(--{})", t.name),
            RoleValue::Literal(v) => (*v).to_string(),
        }
    }
}

/// Un rôle sémantique tel que le bloc `@theme inline` d'`apps/nie-web/src/base.css` le déclare.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticRole {
    /// Le nom de la propriété CSS personnalisée déclarée par `base.css`
    /// (ex. `"color-background"`, `"radius"`, `"shadow-level-1"` — préfixe `color-` inclus quand
    /// `base.css` l'écrit ainsi ; pas de norme uniforme, c'est celle du fichier source).
    pub css_property: &'static str,
    /// Sa valeur.
    pub value: RoleValue,
}

impl SemanticRole {
    /// La déclaration complète : `--{propriete}: {valeur};`, comme dans `base.css`.
    #[must_use]
    pub fn declaration(&self) -> String {
        format!("--{}: {};", self.css_property, self.value.to_css_value())
    }
}

/// Les 20 rôles shadcn du bloc `@theme inline`, dans l'ordre où `base.css` les déclare.
pub const SHADCN_ROLES: [SemanticRole; 20] = [
    SemanticRole { css_property: "color-background", value: RoleValue::Token(CLEAR_SKY) },
    SemanticRole { css_property: "color-foreground", value: RoleValue::Token(DEEP_NIGHT) },
    SemanticRole { css_property: "color-card", value: RoleValue::Literal("#fff") },
    SemanticRole { css_property: "color-card-foreground", value: RoleValue::Token(DEEP_NIGHT) },
    SemanticRole { css_property: "color-popover", value: RoleValue::Literal("#fff") },
    SemanticRole { css_property: "color-popover-foreground", value: RoleValue::Token(DEEP_NIGHT) },
    SemanticRole { css_property: "color-primary", value: RoleValue::Token(TILE_TOP) },
    SemanticRole { css_property: "color-primary-foreground", value: RoleValue::Token(CLEAR_SKY) },
    SemanticRole { css_property: "color-secondary", value: RoleValue::Token(ICE_SURFACE) },
    SemanticRole { css_property: "color-secondary-foreground", value: RoleValue::Token(DEEP_NIGHT) },
    SemanticRole { css_property: "color-muted", value: RoleValue::Token(CHALK_SURFACE) },
    SemanticRole { css_property: "color-muted-foreground", value: RoleValue::Token(SOFT_TEXT) },
    SemanticRole { css_property: "color-accent", value: RoleValue::Token(ICE_SURFACE) },
    SemanticRole { css_property: "color-accent-foreground", value: RoleValue::Token(DEEP_NIGHT) },
    SemanticRole { css_property: "color-destructive", value: RoleValue::Token(BRICK_ACCENT) },
    SemanticRole { css_property: "color-destructive-foreground", value: RoleValue::Token(CLEAR_SKY) },
    SemanticRole { css_property: "color-border", value: RoleValue::Token(TILE_BORDER) },
    SemanticRole { css_property: "color-input", value: RoleValue::Token(TILE_BORDER) },
    SemanticRole { css_property: "color-ring", value: RoleValue::Token(AZURE_ACCENT) },
    SemanticRole { css_property: "radius", value: RoleValue::Raw(RADIUS) },
];

/// Les 7 rôles Material 3 du même bloc, employés par les composants venus du wiki.
pub const MATERIAL_ROLES: [SemanticRole; 7] = [
    SemanticRole {
        css_property: "color-surface-container-lowest",
        value: RoleValue::Literal("#fff"),
    },
    SemanticRole { css_property: "color-surface-container-low", value: RoleValue::Token(ICE_SURFACE) },
    SemanticRole { css_property: "color-surface-container-high", value: RoleValue::Token(CHALK_SURFACE) },
    SemanticRole {
        css_property: "color-surface-container-highest",
        value: RoleValue::Token(MIST_SURFACE),
    },
    SemanticRole { css_property: "color-on-surface", value: RoleValue::Token(DEEP_NIGHT) },
    SemanticRole { css_property: "color-on-surface-variant", value: RoleValue::Token(SOFT_TEXT) },
    SemanticRole { css_property: "shadow-level-1", value: RoleValue::Elevation(TILE_SHADOW) },
];

/// Le rôle nommé, shadcn ou Material 3, ou `None`.
#[must_use]
pub fn find(css_property: &str) -> Option<SemanticRole> {
    SHADCN_ROLES
        .into_iter()
        .chain(MATERIAL_ROLES)
        .find(|r| r.css_property == css_property)
}

#[cfg(test)]
mod tests {
    use super::{MATERIAL_ROLES, RoleValue, SHADCN_ROLES, find};

    #[test]
    fn aucun_nom_de_role_nest_duplique() {
        let mut noms: Vec<&str> =
            SHADCN_ROLES.iter().chain(&MATERIAL_ROLES).map(|r| r.css_property).collect();
        let avant = noms.len();
        noms.sort_unstable();
        noms.dedup();
        assert_eq!(avant, noms.len(), "un nom de rôle est dupliqué");
    }

    #[test]
    fn trois_roles_seulement_sont_litteraux() {
        // Falsifiable : ajouter ou retirer un `#fff` en dur dans `base.css` sans mettre ce
        // compte à jour fait rougir ce test plutôt que de laisser la divergence passer inaperçue.
        let litteraux = SHADCN_ROLES
            .iter()
            .chain(&MATERIAL_ROLES)
            .filter(|r| matches!(r.value, RoleValue::Literal(_)))
            .count();
        assert_eq!(litteraux, 3, "le nombre de rôles non rattachés à un jeton a changé");
    }

    #[test]
    fn les_declarations_correspondent_a_base_css() {
        // Quatre lignes recopiées telles quelles depuis `apps/nie-web/src/base.css` (lu, jamais
        // modifié) : la preuve que la table ci-dessus reproduit vraiment ce fichier.
        assert_eq!(
            find("color-background").unwrap().declaration(),
            "--color-background: var(--jeu-ciel-clair);"
        );
        assert_eq!(find("color-card").unwrap().declaration(), "--color-card: #fff;");
        assert_eq!(find("radius").unwrap().declaration(), "--radius: var(--jeu-rayon);");
        assert_eq!(
            find("shadow-level-1").unwrap().declaration(),
            "--shadow-level-1: var(--jeu-ombre-tuile);"
        );
    }

    #[test]
    fn find_rend_none_pour_un_role_absent() {
        assert_eq!(find("role-qui-nexiste-pas"), None);
    }
}
