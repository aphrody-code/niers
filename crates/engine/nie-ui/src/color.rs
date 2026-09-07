//! Les 29 couleurs du jeu, en OKLCH — transposées de
//! `packages/inacord-ui/src/shell/game-tokens.css` (26 `--jeu-*` + 3 `--inacord-*`).
//!
//! ## D'où viennent ces valeurs
//!
//! Chaque constante reprend, telle quelle, la ligne du CSS d'aujourd'hui : la valeur `oklch()`,
//! l'équivalent hexadécimal et le rôle cités dans son commentaire de fin de ligne. Aucune valeur
//! n'est recalculée ici — [`css::root_block`](crate::css::root_block) prouve l'identité par un
//! test qui compare au fichier livré (voir `crate::css`).
//!
//! Depuis le 2026-09-06 (commit `0374333`), ce CSS n'est plus mesuré à la main sur une capture
//! du jeu : il est **dérivé** par [`nie_aphrody::design`], par k-means Oklab sur l'atlas du
//! personnage Aphrody (`pixel mesurer …/spritesheet.png --k 10 --json`). C'est un changement de
//! provenance qui a eu lieu pendant cette même session — antérieurement, le fichier portait 26
//! couleurs mesurées à la main sur `menu.png`/`start.png` (cf. l'historique Git du fichier). Ce
//! module ne réimplémente pas cette dérivation ; le test [`tests::les_couleurs_du_jeu_suivent_le_calcul_reel`]
//! l'appelle réellement (`nie_aphrody::design::role(nom).oklch()`) pour garder les deux en phase
//! — une divergence future y devient un test rouge, pas une supposition.

/// Une couleur OKLCH, avec la précision d'affichage qu'utilise `game-tokens.css`
/// (`L` et `C` à 4 décimales, `h` à 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// Clarté perceptuelle, dans `[0, 1]`.
    pub l: f64,
    /// Chroma (saturation), non borné mais petit en pratique (`< 0.2` ici).
    pub c: f64,
    /// Teinte, en degrés (`[0, 360)`).
    pub h: f64,
}

impl Oklch {
    /// La notation `oklch(L C h)` telle qu'écrite dans `game-tokens.css`.
    #[must_use]
    pub fn to_css(self) -> String {
        format!("oklch({:.4} {:.4} {:.2})", self.l, self.c, self.h)
    }
}

/// Une couleur du design system du jeu : sa valeur OKLCH, et d'où elle vient.
///
/// Les quatre derniers champs recopient le commentaire de fin de ligne du CSS — c'est la preuve
/// de provenance que la consigne du dépôt exige pour chaque jeton.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorToken {
    /// Le nom de la propriété CSS personnalisée, sans les deux tirets
    /// (par ex. `"jeu-fond-abysse"`). Nom déjà servi aux hôtes web : **ne pas renommer**.
    pub name: &'static str,
    /// La valeur OKLCH telle qu'écrite dans le CSS.
    pub oklch: Oklch,
    /// L'équivalent hexadécimal cité en commentaire (la couleur telle qu'elle se rend).
    pub hex: &'static str,
    /// Le nom de la teinte source, dans la palette mesurée d'Aphrody (ex. `"azur"`, `"blond"`).
    pub source_hue: &'static str,
    /// La part de l'atlas source occupée par cette teinte, en pourcent entier.
    pub source_share_pct: u8,
    /// Le rôle de cette couleur dans l'interface, en une phrase (copié du CSS).
    pub role: &'static str,
}

impl ColorToken {
    /// La ligne `:root` complète de ce jeton, identique à celle du CSS livré :
    /// `\t--nom: oklch(…);  /* #hex - teinte (pct %) - rôle */`.
    #[must_use]
    pub fn css_line(&self) -> String {
        format!(
            "\t--{}: {};  /* {} - {} ({} %) - {} */",
            self.name,
            self.oklch.to_css(),
            self.hex,
            self.source_hue,
            self.source_share_pct,
            self.role
        )
    }
}

macro_rules! color_token {
    ($doc:literal, $ident:ident, $name:literal, $l:literal, $c:literal, $h:literal, $hex:literal, $hue:literal, $pct:literal, $role:literal) => {
        #[doc = $doc]
        pub const $ident: ColorToken = ColorToken {
            name: $name,
            oklch: Oklch {
                l: $l,
                c: $c,
                h: $h,
            },
            hex: $hex,
            source_hue: $hue,
            source_share_pct: $pct,
            role: $role,
        };
    };
}

// --- Fonds : du plus profond au plus clair -------------------------------------------------
color_token!(
    "Le fond le plus profond. Mesuré `#131420` — teinte nuit (2 % de l'atlas source).",
    ABYSS_BACKGROUND,
    "jeu-fond-abysse",
    0.1963,
    0.0242,
    280.23,
    "#131420",
    "nuit",
    2,
    "le fond le plus profond"
);
color_token!(
    "Un panneau sombre. Mesuré `#152d52` — teinte azur (2 % de l'atlas source).",
    NIGHT_BACKGROUND,
    "jeu-fond-nuit",
    0.3000,
    0.0726,
    258.02,
    "#152d52",
    "azur",
    2,
    "un panneau sombre"
);
color_token!(
    "Une surface bleue. Mesuré `#224271` — teinte azur (2 % de l'atlas source).",
    DEEP_BACKGROUND,
    "jeu-fond-profond",
    0.3800,
    0.0888,
    258.02,
    "#224271",
    "azur",
    2,
    "une surface bleue"
);
color_token!(
    "Une surface bleue active. Mesuré `#34588d` — teinte azur (2 % de l'atlas source).",
    MEDIUM_BACKGROUND,
    "jeu-fond-moyen",
    0.4600,
    0.0968,
    258.02,
    "#34588d",
    "azur",
    2,
    "une surface bleue active"
);

// --- Accents : ce qui appelle l'oeil --------------------------------------------------------
color_token!(
    "L'accent chaud, la couleur des cheveux. Mesuré `#e6c961` — teinte blond (21 % de l'atlas).",
    AMBER_ACCENT,
    "jeu-accent-ambre",
    0.8400,
    0.1273,
    94.17,
    "#e6c961",
    "blond",
    21,
    "l'accent chaud, la couleur des cheveux"
);
color_token!(
    "L'alerte. Mesuré `#c74955` — teinte brun (7 % de l'atlas source).",
    BRICK_ACCENT,
    "jeu-accent-brique",
    0.5800,
    0.1600,
    18.35,
    "#c74955",
    "brun",
    7,
    "l'alerte"
);
color_token!(
    "L'accent froid, la couleur de la tenue. Mesuré `#477ac6` — teinte azur (2 % de l'atlas).",
    AZURE_ACCENT,
    "jeu-accent-azur",
    0.5800,
    0.1291,
    258.02,
    "#477ac6",
    "azur",
    2,
    "l'accent froid, la couleur de la tenue"
);
color_token!(
    "Le liseré d'un état actif. Mesuré `#97c0fc` — teinte azur (2 % de l'atlas source).",
    CYAN_ACCENT,
    "jeu-accent-cyan",
    0.8000,
    0.0968,
    258.02,
    "#97c0fc",
    "azur",
    2,
    "le liseré d'un état actif"
);
color_token!(
    "Le succès. Mesuré `#5f9efc` — teinte azur (2 % de l'atlas source).",
    TURQUOISE_ACCENT,
    "jeu-accent-turquoise",
    0.7000,
    0.1533,
    258.02,
    "#5f9efc",
    "azur",
    2,
    "le succès"
);

// --- Surfaces claires ------------------------------------------------------------------------
color_token!(
    "Une carte claire. Mesuré `#e0ecff` — teinte azur (2 % de l'atlas source).",
    ICE_SURFACE,
    "jeu-surface-glace",
    0.9400,
    0.0282,
    258.02,
    "#e0ecff",
    "azur",
    2,
    "une carte claire"
);
color_token!(
    "Un dégradé clair, teinté du bleu de la tenue. Mesuré `#c7d9f3` — teinte azur (2 %).",
    MIST_SURFACE,
    "jeu-surface-brume",
    0.8800,
    0.0403,
    258.02,
    "#c7d9f3",
    "azur",
    2,
    "un dégradé clair, teinté du bleu de la tenue"
);
color_token!(
    "Un fond neutre. Mesuré `#e3d5c5` — teinte sable (13 % de l'atlas source).",
    CHALK_SURFACE,
    "jeu-surface-craie",
    0.8800,
    0.0263,
    70.41,
    "#e3d5c5",
    "sable",
    13,
    "un fond neutre"
);
color_token!(
    "Un texte secondaire. Mesuré `#b3988b` — teinte taupe (11 % de l'atlas source).",
    ASH_SURFACE,
    "jeu-surface-cendre",
    0.7000,
    0.0374,
    46.43,
    "#b3988b",
    "taupe",
    11,
    "un texte secondaire"
);
color_token!(
    "Une nuance douce. Mesuré `#9e8296` — teinte mauve (9 % de l'atlas source).",
    PINK_SURFACE,
    "jeu-surface-rose",
    0.6400,
    0.0435,
    337.07,
    "#9e8296",
    "mauve",
    9,
    "une nuance douce"
);

// --- Texte -------------------------------------------------------------------------------------
color_token!(
    "Le texte sur fond sombre. Mesuré `#fcfaf8` — teinte crème (25 % de l'atlas source).",
    VIVID_TEXT,
    "jeu-texte-vif",
    0.9850,
    0.0035,
    59.65,
    "#fcfaf8",
    "creme",
    25,
    "le texte sur fond sombre"
);
color_token!(
    "Un lien, un texte de second plan. Mesuré `#638dcb` — teinte azur (2 % de l'atlas).",
    SOFT_TEXT,
    "jeu-texte-doux",
    0.6400,
    0.1049,
    258.02,
    "#638dcb",
    "azur",
    2,
    "un lien, un texte de second plan"
);

// --- Écran du menu : ciel, tuiles, plaque -------------------------------------------------------
color_token!(
    "Le fond de l'écran d'accueil. Mesuré `#f9f6f4` — teinte crème (25 % de l'atlas).",
    CLEAR_SKY,
    "jeu-ciel-clair",
    0.9750,
    0.0041,
    59.65,
    "#f9f6f4",
    "creme",
    25,
    "le fond de l'écran d'accueil"
);
color_token!(
    "Le ciel, en haut à droite. Mesuré `#ccdcf3` — teinte azur (2 % de l'atlas source).",
    MISTY_SKY,
    "jeu-ciel-brume",
    0.8900,
    0.0363,
    258.02,
    "#ccdcf3",
    "azur",
    2,
    "le ciel, en haut à droite"
);
color_token!(
    "Le texte sur fond clair — la mesure telle quelle. Mesuré `#17335c` — teinte azur (2 %).",
    DEEP_NIGHT,
    "jeu-nuit-profonde",
    0.3229,
    0.0807,
    258.02,
    "#17335c",
    "azur",
    2,
    "le texte sur fond clair — la mesure telle quelle"
);
color_token!(
    "Le haut d'une tuile. Mesuré `#3b639e` — teinte azur (2 % de l'atlas source).",
    TILE_TOP,
    "jeu-tuile-haut",
    0.5000,
    0.1049,
    258.02,
    "#3b639e",
    "azur",
    2,
    "le haut d'une tuile"
);
color_token!(
    "Le bas d'une tuile. Mesuré `#214478` — teinte azur (2 % de l'atlas source).",
    TILE_BOTTOM,
    "jeu-tuile-bas",
    0.3900,
    0.0968,
    258.02,
    "#214478",
    "azur",
    2,
    "le bas d'une tuile"
);
color_token!(
    "Le bord d'une tuile. Mesuré `#31558a` — teinte azur (2 % de l'atlas source).",
    TILE_BORDER,
    "jeu-tuile-bord",
    0.4500,
    0.0968,
    258.02,
    "#31558a",
    "azur",
    2,
    "le bord d'une tuile"
);
color_token!(
    "Le haut d'une tuile active. Mesuré `#4b7ac1` — teinte azur (2 % de l'atlas source).",
    ACTIVE_TILE_TOP,
    "jeu-tuile-active-haut",
    0.5800,
    0.1210,
    258.02,
    "#4b7ac1",
    "azur",
    2,
    "le haut d'une tuile active"
);
color_token!(
    "Le bas d'une tuile active. Mesuré `#0c53b0` — teinte azur (2 % de l'atlas source).",
    ACTIVE_TILE_BOTTOM,
    "jeu-tuile-active-bas",
    0.4600,
    0.1614,
    258.02,
    "#0c53b0",
    "azur",
    2,
    "le bas d'une tuile active"
);
color_token!(
    "La plaque centrale. Mesuré `#02489e` — teinte azur (2 % de l'atlas source).",
    BLUE_PLATE,
    "jeu-plaque-bleu",
    0.4200,
    0.1533,
    258.02,
    "#02489e",
    "azur",
    2,
    "la plaque centrale"
);
color_token!(
    "Le liseré doré. Mesuré `#d6b52c` — teinte blond (21 % de l'atlas source).",
    GOLD_TRIM,
    "jeu-lisere-or",
    0.7800,
    0.1485,
    94.17,
    "#d6b52c",
    "blond",
    21,
    "le liseré doré"
);

// --- Coquille InaCord : l'autre ambiance, mêmes teintes -----------------------------------------
color_token!(
    "Le panneau d'Inacord. Mesuré `#292a37` — teinte nuit (2 % de l'atlas source).",
    INACORD_PANEL,
    "inacord-panneau",
    0.2900,
    0.0242,
    280.23,
    "#292a37",
    "nuit",
    2,
    "le panneau d'Inacord"
);
color_token!(
    "Son panneau clair. Mesuré `#32435c` — teinte azur (2 % de l'atlas source).",
    INACORD_PANEL_LIGHT,
    "inacord-panneau-clair",
    0.3800,
    0.0484,
    258.02,
    "#32435c",
    "azur",
    2,
    "son panneau clair"
);
color_token!(
    "Son unique accent. Mesuré `#7ca0d6` — teinte azur (2 % de l'atlas source).",
    INACORD_ACCENT,
    "inacord-accent",
    0.7000,
    0.0888,
    258.02,
    "#7ca0d6",
    "azur",
    2,
    "son unique accent"
);

/// Une section du bloc `:root` : l'index de [`GAME_COLORS`] où elle commence, son titre, et le
/// nombre de tirets qui referment son en-tête — mesuré sur le fichier livré (voir
/// `docs/DESIGN-UI.md`), pas recalculé : les en-têtes des cinq premières sections sont posées à
/// une largeur totale fixe par le générateur qui a produit ce CSS
/// ([`nie_aphrody::design::feuille_css`]), la sixième (Coquille InaCord) suit la même règle.
pub const SECTIONS: [(usize, &str, usize); 6] = [
    (0, "Fonds : du plus profond au plus clair", 49),
    (4, "Accents : ce qui appelle l'oeil", 55),
    (9, "Surfaces claires", 70),
    (14, "Texte", 81),
    (16, "Ecran du menu : ciel, tuiles, plaque", 50),
    (26, "Coquille InaCord : l'autre ambiance, memes teintes", 36),
];

/// Les 29 couleurs, dans l'ordre exact du CSS — c'est aussi l'ordre que suit [`SECTIONS`].
pub const GAME_COLORS: [ColorToken; 29] = [
    ABYSS_BACKGROUND,
    NIGHT_BACKGROUND,
    DEEP_BACKGROUND,
    MEDIUM_BACKGROUND,
    AMBER_ACCENT,
    BRICK_ACCENT,
    AZURE_ACCENT,
    CYAN_ACCENT,
    TURQUOISE_ACCENT,
    ICE_SURFACE,
    MIST_SURFACE,
    CHALK_SURFACE,
    ASH_SURFACE,
    PINK_SURFACE,
    VIVID_TEXT,
    SOFT_TEXT,
    CLEAR_SKY,
    MISTY_SKY,
    DEEP_NIGHT,
    TILE_TOP,
    TILE_BOTTOM,
    TILE_BORDER,
    ACTIVE_TILE_TOP,
    ACTIVE_TILE_BOTTOM,
    BLUE_PLATE,
    GOLD_TRIM,
    INACORD_PANEL,
    INACORD_PANEL_LIGHT,
    INACORD_ACCENT,
];

/// Convertit un hexadécimal `#rrggbb` en triplet décimal `[r, g, b]` — sans dépendance, pour
/// composer les valeurs d'élévation de [`crate::tokens`] (`rgb(r g b / a%)`) à partir du `hex`
/// d'un [`ColorToken`], plutôt que de recopier une deuxième fois la même couleur en décimal.
///
/// # Panics
///
/// Si `hex` n'est pas de la forme `#rrggbb` (7 caractères ASCII hexadécimaux après le `#`).
#[must_use]
pub fn hex_to_rgb_triplet(hex: &str) -> [u8; 3] {
    let digits = hex
        .strip_prefix('#')
        .unwrap_or_else(|| panic!("« {hex} » n'a pas de préfixe #"));
    assert!(
        digits.len() == 6,
        "« {hex} » n'a pas 6 chiffres hexadécimaux"
    );
    let byte = |i: usize| {
        u8::from_str_radix(&digits[i..i + 2], 16).unwrap_or_else(|e| panic!("« {hex} » : {e}"))
    };
    [byte(0), byte(2), byte(4)]
}

/// Le jeton nommé, ou `None` si aucun ne porte ce nom.
#[must_use]
pub fn find(name: &str) -> Option<ColorToken> {
    GAME_COLORS.into_iter().find(|t| t.name == name)
}

#[cfg(test)]
mod tests {
    use super::{CYAN_ACCENT, GAME_COLORS, Oklch, find, hex_to_rgb_triplet};

    #[test]
    fn chaque_nom_de_jeton_est_unique() {
        let mut noms: Vec<&str> = GAME_COLORS.iter().map(|t| t.name).collect();
        let avant = noms.len();
        noms.sort_unstable();
        noms.dedup();
        assert_eq!(avant, noms.len(), "un nom de jeton est dupliqué");
    }

    #[test]
    fn find_retrouve_un_jeton_connu_et_rien_dinvente() {
        assert_eq!(find("jeu-fond-abysse"), Some(super::ABYSS_BACKGROUND));
        assert_eq!(find("jeton-qui-nexiste-pas"), None);
    }

    #[test]
    fn hex_to_rgb_triplet_convertit_les_deux_couleurs_reellement_utilisees_par_lelevation() {
        // Ce sont les deux seules couleurs que `crate::tokens` compose en `rgb()` — cf. le CSS
        // livré : `--jeu-ombre-tuile` cite `#131420` (ABYSS_BACKGROUND), `--jeu-lueur-accent`
        // cite `#97c0fc` (CYAN_ACCENT).
        assert_eq!(
            hex_to_rgb_triplet(super::ABYSS_BACKGROUND.hex),
            [0x13, 0x14, 0x20]
        );
        assert_eq!(hex_to_rgb_triplet(CYAN_ACCENT.hex), [0x97, 0xc0, 0xfc]);
    }

    #[test]
    #[should_panic(expected = "6 chiffres")]
    fn hex_to_rgb_triplet_refuse_un_hex_mal_forme() {
        let _ = hex_to_rgb_triplet("#abc");
    }

    /// Cross-check RÉEL (pas une réimplémentation) : `nie_aphrody::design` dérive déjà chaque
    /// rôle `--jeu-*`/`--inacord-*` depuis la palette mesurée du personnage. Ce test appelle ce
    /// calcul et compare son résultat à la transposition figée ci-dessus — une divergence future
    /// (mesure reprise, rôle retouché) le fait ROUGIR au lieu de rester une hypothèse silencieuse.
    #[test]
    fn les_couleurs_du_jeu_suivent_le_calcul_reel_de_nie_aphrody() {
        for jeton in GAME_COLORS {
            let role = nie_aphrody::design::role(jeton.name).unwrap_or_else(|| {
                panic!(
                    "{} : absent de nie_aphrody::design (rôle renommé ou supprimé ?)",
                    jeton.name
                )
            });
            // Comparaison sur la forme CSS (4/4/2 décimales), pas sur le flottant brut : le CSS
            // livré est déjà arrondi (ex. `jeu-fond-nuit` écrit 0.0726 pour un calcul réel de
            // 0.0807 * 0.9 = 0.07263), et c'est CETTE forme arrondie que `color.rs` transpose.
            // Comparer les flottants bruts à 1e-6 ferait rougir ce test sur un arrondi normal,
            // pas sur une vraie divergence — un faux rouge serait aussi trompeur qu'un faux vert.
            let [l, c, h] = role.oklch();
            let calcule = Oklch { l, c, h }.to_css();
            assert_eq!(
                jeton.oklch.to_css(),
                calcule,
                "{} a divergé de nie_aphrody::design (calcul réel : {calcule})",
                jeton.name
            );
        }
    }
}
