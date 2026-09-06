//! Les surfaces des écrans du jeu — les couleurs `--screen-*` MESURÉES sur les captures de
//! `data/menu/` (2560×1440), et l'angle du parallélogramme de la DA.
//!
//! ## D'où viennent ces valeurs
//!
//! Chaque constante cite la capture, le recadrage et la part de la classe retenue, tels que
//! rendus par l'instrument du dépôt :
//!
//! ```sh
//! cargo run -p nie-aphrody --bin pixel -- capture data/menu/<capture>.png --crop X,Y,W,H --k N
//! ```
//!
//! (`capture` = `mesurer` avec le masque alpha par défaut — une capture est opaque — et un
//! recadrage origine + taille ; k-means Oklab **déterministe**, cf. `nie_aphrody::pixel`). Le
//! test [`tests::les_ancres_suivent_la_mesure_reelle_de_nie_aphrody`] rappelle cette fonction
//! sur trois recadrages et exige un ΔE Oklab < 0,02 avec la constante transposée — une capture
//! remplacée ou une constante retouchée y devient un test rouge.
//!
//! Aucune de ces couleurs n'est un jeton `--jeu-*` : `game-tokens.css` dérive de l'atlas du
//! personnage (cf. [`crate::color`]), ces surfaces-ci sont lues sur les écrans réels. Quand un
//! rôle existe déjà côté `--jeu-*` (typographie, rythme, ombres), `game-screens.css` l'emploie
//! par `var(--jeu-…)` au lieu de mesurer une seconde fois.

use crate::color::Oklch;

/// Une couleur mesurée sur une capture d'écran, avec sa provenance complète.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenColor {
    /// Le nom de la propriété CSS personnalisée, sans les deux tirets (`screen-…`).
    pub name: &'static str,
    /// La valeur Oklch, telle que `pixel capture` l'a rendue (4/4/2 décimales).
    pub oklch: Oklch,
    /// Le représentant hexadécimal de la classe (un pixel RÉEL de l'image, jamais une moyenne).
    pub hex: &'static str,
    /// Le fichier de `data/menu/` mesuré.
    pub capture: &'static str,
    /// Le recadrage `x, y, w, h` passé à `--crop`.
    pub crop: (u32, u32, u32, u32),
    /// Le `--k` du k-means.
    pub k: u8,
    /// La part de la classe retenue, en pourcent (deux décimales).
    pub share_pct: f64,
    /// Le rôle de cette couleur dans l'écran, en une phrase.
    pub role: &'static str,
}

impl ScreenColor {
    /// La ligne `:root` de cette couleur :
    /// `\t--nom: oklch(…);  /* #hex - capture crop x,y,w,h k=N (part %) - rôle */`.
    #[must_use]
    pub fn css_line(&self) -> String {
        let (x, y, w, h) = self.crop;
        format!(
            "\t--{}: {};  /* {} - {} crop {x},{y},{w},{h} k={} ({:.2} %) - {} */",
            self.name,
            self.oklch.to_css(),
            self.hex,
            self.capture,
            self.k,
            self.share_pct,
            self.role
        )
    }
}

macro_rules! screen_color {
    ($doc:literal, $ident:ident, $name:literal, $l:literal, $c:literal, $h:literal, $hex:literal,
     $capture:literal, ($x:literal, $y:literal, $w:literal, $hh:literal), $k:literal, $pct:literal, $role:literal) => {
        #[doc = $doc]
        pub const $ident: ScreenColor = ScreenColor {
            name: $name,
            oklch: Oklch {
                l: $l,
                c: $c,
                h: $h,
            },
            hex: $hex,
            capture: $capture,
            crop: ($x, $y, $w, $hh),
            k: $k,
            share_pct: $pct,
            role: $role,
        };
    };
}

// --- Barre de titre (options.png, controls.png, filters_*.png) --------------------------------
screen_color!(
    "Le bleu saturé de la barre de titre. `pixel capture options.png --crop 700,40,1701,81 --k 3` → #0874FF (35.61 %).",
    HEADER_BLUE,
    "screen-header-blue",
    0.5912,
    0.2241,
    258.91,
    "#0874FF",
    "options.png",
    (700, 40, 1701, 81),
    3,
    35.61,
    "la barre de titre"
);
screen_color!(
    "Le bleu profond des rayures diagonales de la barre. Même recadrage, 2e classe (34.21 %).",
    HEADER_BLUE_DEEP,
    "screen-header-blue-deep",
    0.5523,
    0.2353,
    261.08,
    "#0663F8",
    "options.png",
    (700, 40, 1701, 81),
    3,
    34.21,
    "les rayures diagonales de la barre de titre"
);
screen_color!(
    "Le gris de la tuile d'icône à gauche de la barre. `--crop 0,0,151,151 --k 4` → #A6A6A6 (30.83 %), le blanc du glyphe en 1re classe.",
    HEADER_ICON_GREY,
    "screen-header-icon-grey",
    0.7252,
    0.0000,
    180.00,
    "#A6A6A6",
    "options.png",
    (0, 0, 151, 151),
    4,
    30.83,
    "la tuile d'icône à gauche de la barre de titre"
);

// --- Bandeau d'onglets (options.png) ---------------------------------------------------------
screen_color!(
    "Le gris foncé des touches « W »/« C » aux deux bouts du bandeau. `--crop 700,265,161,56 --k 3` → #4E4E4E (95.42 %).",
    TAB_KEY,
    "screen-tab-key",
    0.4239,
    0.0000,
    21.80,
    "#4E4E4E",
    "options.png",
    (700, 265, 161, 56),
    3,
    95.42,
    "les touches clavier aux bouts du bandeau d'onglets"
);
screen_color!(
    "Le bleu clair du milieu du bandeau. `--crop 900,265,141,56 --k 3` → #6CA8F0 (100 %).",
    TAB_LIGHT,
    "screen-tab-light",
    0.7206,
    0.1229,
    253.81,
    "#6CA8F0",
    "options.png",
    (900, 265, 141, 56),
    3,
    100.0,
    "le fond clair du bandeau d'onglets"
);
screen_color!(
    "Le bleu saturé de l'onglet actif. `--crop 1065,265,111,56 --k 3` → #0149FF (83.17 %), le glyphe blanc en 2e classe.",
    TAB_ACTIVE,
    "screen-tab-active",
    0.5141,
    0.2718,
    263.49,
    "#0149FF",
    "options.png",
    (1065, 265, 111, 56),
    3,
    83.17,
    "l'onglet actif"
);

// --- Lignes de réglage (options.png, controls.png) --------------------------------------------
screen_color!(
    "Le blanc d'une ligne au repos. `--crop 520,515,881,51 --k 3` → #FEFFFF (88.23 %).",
    ROW_WHITE,
    "screen-row-white",
    0.9992,
    0.0011,
    197.15,
    "#FEFFFF",
    "options.png",
    (520, 515, 881, 51),
    3,
    88.23,
    "une ligne de réglage au repos"
);
screen_color!(
    "Le gris du libellé d'une ligne au repos. Même recadrage, 2e classe → #79797A (8.59 %).",
    ROW_LABEL,
    "screen-row-label",
    0.5764,
    0.0015,
    286.35,
    "#79797A",
    "options.png",
    (520, 515, 881, 51),
    3,
    8.59,
    "le libellé d'une ligne au repos"
);
screen_color!(
    "Le bleu de la ligne FOCALISÉE — mesuré PLAT : `--crop 600,415,501,11` (haut) et `600,460,501,11` (bas) rendent tous deux #0078FF (98.93 % / 99.9 %). Il n'y a pas de dégradé vertical.",
    ROW_FOCUS,
    "screen-row-focus",
    0.5987,
    0.2200,
    257.83,
    "#0078FF",
    "options.png",
    (600, 415, 501, 11),
    3,
    98.93,
    "la ligne de réglage focalisée (aplat, pas de dégradé vertical)"
);
screen_color!(
    "Le bleu clair de la colonne de valeur (controls.png, où il est franc ; sur options.png il est presque blanc, #F9FEFF). `--crop 1380,420,381,51 --k 4` → #A7DDFF (92.60 %).",
    ROW_VALUE_STRIP,
    "screen-row-value-strip",
    0.8719,
    0.0729,
    236.34,
    "#A7DDFF",
    "controls.png",
    (1380, 420, 381, 51),
    4,
    92.60,
    "la colonne de valeur d'une ligne au repos"
);
screen_color!(
    "La colonne de valeur d'une ligne focalisée. `--crop 1380,1010,381,51 --k 4` → #08B5FF (94.39 %).",
    ROW_VALUE_STRIP_ACTIVE,
    "screen-row-value-strip-active",
    0.7327,
    0.1590,
    237.05,
    "#08B5FF",
    "controls.png",
    (1380, 1010, 381, 51),
    4,
    94.39,
    "la colonne de valeur d'une ligne focalisée"
);
screen_color!(
    "Le bleu du texte de valeur (« Normal »). `--crop 1700,522,121,39 --k 4` → #5084B7 (23.33 %), le fond en 1re classe.",
    ROW_VALUE_TEXT,
    "screen-row-value-text",
    0.5988,
    0.0963,
    249.27,
    "#5084B7",
    "options.png",
    (1700, 522, 121, 39),
    4,
    23.33,
    "le texte de la colonne de valeur"
);
screen_color!(
    "Le carré arrondi des flèches « < » « > » de la ligne focalisée. `--crop 1470,416,51,52 --k 4` → #0350E6 (62.97 %).",
    ROW_ARROW,
    "screen-row-arrow",
    0.5008,
    0.2353,
    262.40,
    "#0350E6",
    "options.png",
    (1470, 416, 51, 52),
    4,
    62.97,
    "le carré des flèches de la ligne focalisée"
);
screen_color!(
    "Le liseré clair du carré de flèche. Même recadrage, 2e classe → #4782F7 (21.08 %).",
    ROW_ARROW_LIGHT,
    "screen-row-arrow-light",
    0.6281,
    0.1859,
    262.29,
    "#4782F7",
    "options.png",
    (1470, 416, 51, 52),
    4,
    21.08,
    "le liseré du carré de flèche"
);
screen_color!(
    "Le bleu-gris du titre de section (« Paramètres du jeu »). `--crop 1133,352,295,33 --k 4` → #4B6D97 (26.21 %).",
    SECTION_TITLE,
    "screen-section-title",
    0.5270,
    0.0774,
    254.25,
    "#4B6D97",
    "options.png",
    (1133, 352, 295, 33),
    4,
    26.21,
    "le titre de section au-dessus d'une liste"
);
screen_color!(
    "Le blanc de la barre de défilement. `--crop 2112,403,14,368 --k 3` → #FFFFFF (81.62 %).",
    SCROLLBAR,
    "screen-scrollbar",
    1.0000,
    0.0000,
    90.00,
    "#FFFFFF",
    "options.png",
    (2112, 403, 14, 368),
    3,
    81.62,
    "le curseur de la barre de défilement"
);
screen_color!(
    "Le gris-bleu de la piste de défilement. Même recadrage, 2e classe → #96AABB (10.79 %).",
    SCROLLBAR_TRACK,
    "screen-scrollbar-track",
    0.7282,
    0.0333,
    243.86,
    "#96AABB",
    "options.png",
    (2112, 403, 14, 368),
    3,
    10.79,
    "la piste de la barre de défilement"
);

// --- Chrome : description, touches, compteur, fond ------------------------------------------
screen_color!(
    "Le gris translucide de la barre de description (mesuré composé sur le fond). `--crop 100,1235,401,71 --k 3` → #616E7D (66.94 %).",
    DESCRIPTION_GREY,
    "screen-description-grey",
    0.5328,
    0.0284,
    252.08,
    "#616E7D",
    "options.png",
    (100, 1235, 401, 71),
    3,
    66.94,
    "la barre de description en bas d'écran"
);
screen_color!(
    "Le gris foncé d'une touche (« V »). `--crop 915,1350,41,41 --k 3` → #4A4949 (47.83 %).",
    KEY_CAP,
    "screen-key-cap",
    0.4064,
    0.0013,
    17.21,
    "#4A4949",
    "options.png",
    (915, 1350, 41, 41),
    3,
    47.83,
    "une touche clavier (fond)"
);
screen_color!(
    "Le gris du compteur « 13/13 ». `--crop 1780,1200,51,41 --k 3` → #545454 (79.29 %).",
    COUNT_BADGE,
    "screen-count-badge",
    0.4459,
    0.0000,
    180.00,
    "#545454",
    "filters_elements.png",
    (1780, 1200, 51, 41),
    3,
    79.29,
    "la pastille de compteur"
);
screen_color!(
    "Le fond pâle du canevas. `--crop 128,384,257,257 --k 3` → #E0FAFF (54.19 %).",
    CANVAS_PALE,
    "screen-canvas-pale",
    0.9678,
    0.0279,
    210.53,
    "#E0FAFF",
    "options.png",
    (128, 384, 257, 257),
    3,
    54.19,
    "le fond pâle du canevas"
);

// --- Le curseur (options.png) ----------------------------------------------------------------
screen_color!(
    "Le jaune-vert du curseur triangulaire. `--crop 455,405,71,76 --k 6` → #B5FF6B (29.19 %).",
    CURSOR_YELLOW,
    "screen-cursor-yellow",
    0.9218,
    0.1938,
    131.54,
    "#B5FF6B",
    "options.png",
    (455, 405, 71, 76),
    6,
    29.19,
    "la bande jaune du curseur"
);
screen_color!(
    "Le vert du curseur. Même recadrage, 2e classe → #00CE87 (26.30 %).",
    CURSOR_GREEN,
    "screen-cursor-green",
    0.7510,
    0.1709,
    159.65,
    "#00CE87",
    "options.png",
    (455, 405, 71, 76),
    6,
    26.30,
    "la bande verte du curseur"
);
screen_color!(
    "Le cyan pâle du curseur. Même recadrage, 4e classe → #CDF9FF (9.17 %) — la 3e est le bleu de la ligne derrière.",
    CURSOR_CYAN,
    "screen-cursor-cyan",
    0.9532,
    0.0457,
    206.57,
    "#CDF9FF",
    "options.png",
    (455, 405, 71, 76),
    6,
    9.17,
    "la bande cyan du curseur"
);

// --- Le panneau FILTRES (filters_elements.png) ----------------------------------------------
screen_color!(
    "Le bleu royal du haut du panneau. `--crop 900,160,1151,26 --k 3` → #0048B9 (84.62 %).",
    PANEL_TOP,
    "screen-panel-top",
    0.4431,
    0.1890,
    260.78,
    "#0048B9",
    "filters_elements.png",
    (900, 160, 1151, 26),
    3,
    84.62,
    "le haut du panneau externe"
);
screen_color!(
    "Le bleu plus sombre du bas du panneau — le panneau externe porte un dégradé vertical. `--crop 512,1262,1537,26 --k 3` → #002496 (82.54 %).",
    PANEL_BOTTOM,
    "screen-panel-bottom",
    0.3411,
    0.1862,
    263.67,
    "#002496",
    "filters_elements.png",
    (512, 1262, 1537, 26),
    3,
    82.54,
    "le bas du panneau externe"
);
screen_color!(
    "Le bleu nuit du corps interne. `--crop 540,705,741,321 --k 4` → #001E73 (68.85 %).",
    PANEL_BODY,
    "screen-panel-body",
    0.2896,
    0.1484,
    263.16,
    "#001E73",
    "filters_elements.png",
    (540, 705, 741, 321),
    4,
    68.85,
    "le corps interne du panneau"
);
screen_color!(
    "Les glyphes en filigrane du corps. Même recadrage, 2e classe → #163181 (29.60 %).",
    PANEL_WATERMARK,
    "screen-panel-watermark",
    0.3487,
    0.1389,
    265.33,
    "#163181",
    "filters_elements.png",
    (540, 705, 741, 321),
    4,
    29.60,
    "les glyphes en filigrane du corps"
);
screen_color!(
    "Le bleu clair des lettres du titre « FILTRES ». `--crop 493,166,283,72 --k 4` → #4183F4 (24.11 %), le panneau en 1re classe.",
    PANEL_TITLE,
    "screen-panel-title",
    0.6259,
    0.1826,
    260.40,
    "#4183F4",
    "filters_elements.png",
    (493, 166, 283, 72),
    4,
    24.11,
    "les lettres du titre du panneau"
);

// --- Cases à cocher et pastilles d'icône (filters_elements.png) ------------------------------
screen_color!(
    "Le cyan de la coche. `--crop 636,386,52,50 --k 4` → #45FFF8 (38.00 %).",
    CHECK_CYAN,
    "screen-check-cyan",
    0.9094,
    0.1434,
    191.10,
    "#45FFF8",
    "filters_elements.png",
    (636, 386, 52, 50),
    4,
    38.00,
    "la coche"
);
screen_color!(
    "Le bleu nuit de la case. Même recadrage, 2e classe → #012075 (32.58 %).",
    CHECK_BOX,
    "screen-check-box",
    0.2960,
    0.1484,
    263.25,
    "#012075",
    "filters_elements.png",
    (636, 386, 52, 50),
    4,
    32.58,
    "le fond de la case à cocher"
);
screen_color!(
    "Le liseré de la case. Même recadrage, 3e classe → #2B62B6 (26.08 %).",
    CHECK_BOX_EDGE,
    "screen-check-box-edge",
    0.5053,
    0.1453,
    258.85,
    "#2B62B6",
    "filters_elements.png",
    (636, 386, 52, 50),
    4,
    26.08,
    "le liseré de la case à cocher"
);
screen_color!(
    "La pastille Vent. `--crop 723,476,65,63 --k 3` → #8ED5FF (71.67 %).",
    CHIP_WIND,
    "screen-chip-wind",
    0.8412,
    0.0923,
    235.38,
    "#8ED5FF",
    "filters_elements.png",
    (723, 476, 65, 63),
    3,
    71.67,
    "la pastille d'icône Vent"
);
screen_color!(
    "La pastille Feu. `--crop 723,570,65,62 --k 3` → #FF7155 (69.03 %).",
    CHIP_FIRE,
    "screen-chip-fire",
    0.7160,
    0.1791,
    32.86,
    "#FF7155",
    "filters_elements.png",
    (723, 570, 65, 62),
    3,
    69.03,
    "la pastille d'icône Feu"
);
screen_color!(
    "La pastille Forêt. `--crop 1459,476,65,63 --k 3` → #ABFF38 (72.70 %).",
    CHIP_FOREST,
    "screen-chip-forest",
    0.9124,
    0.2308,
    130.66,
    "#ABFF38",
    "filters_elements.png",
    (1459, 476, 65, 63),
    3,
    72.70,
    "la pastille d'icône Forêt"
);
screen_color!(
    "La pastille Montagne. `--crop 1459,570,65,62 --k 3` → #FFB936 (72.83 %).",
    CHIP_MOUNTAIN,
    "screen-chip-mountain",
    0.8307,
    0.1577,
    78.07,
    "#FFB936",
    "filters_elements.png",
    (1459, 570, 65, 62),
    3,
    72.83,
    "la pastille d'icône Montagne"
);

// --- Boutons et texte d'indice (filters_elements.png) ---------------------------------------
screen_color!(
    "Le bleu vif du bouton Confirmer. `--crop 1400,1170,101,71 --k 3` → #009DFF (42.57 %) — trois classes à ±2 % de L : un aplat.",
    BUTTON_PRIMARY,
    "screen-button-primary",
    0.6779,
    0.1797,
    247.35,
    "#009DFF",
    "filters_elements.png",
    (1400, 1170, 101, 71),
    3,
    42.57,
    "le bouton principal (Confirmer)"
);
screen_color!(
    "Le bleu royal du bouton Réinitialiser. `--crop 850,1190,31,41 --k 3` → #3672E5 (100 %).",
    BUTTON_SECONDARY,
    "screen-button-secondary",
    0.5765,
    0.1859,
    261.77,
    "#3672E5",
    "filters_elements.png",
    (850, 1190, 31, 41),
    3,
    100.0,
    "le bouton secondaire (Réinitialiser)"
);
screen_color!(
    "Le bleu sombre du bouton « Tout » dans le panneau. `--crop 1900,1050,81,56 --k 3` → #0030A2 (100 %).",
    BUTTON_TERTIARY,
    "screen-button-tertiary",
    0.3748,
    0.1881,
    262.96,
    "#0030A2",
    "filters_elements.png",
    (1900, 1050, 81, 56),
    3,
    100.0,
    "un bouton posé sur le panneau"
);
screen_color!(
    "Le cyan du texte d'indice « Chercher par nom de joueur ». `--crop 1625,1350,514,41 --k 4` → #0EFFF1 (24.17 %), le fond en 1re classe.",
    SEARCH_HINT,
    "screen-search-hint",
    0.9013,
    0.1559,
    187.70,
    "#0EFFF1",
    "filters_elements.png",
    (1625, 1350, 514, 41),
    4,
    24.17,
    "le texte d'indice cyan de la barre du bas"
);

// --- Tuiles du menu principal et fenêtre Informations (main_menu.png) -----------------------
screen_color!(
    "Le bleu profond dominant d'une tuile (fond photo). `--crop 1062,768,245,167 --k 6` → #09316B (27.12 %), le blanc du glyphe en 2e classe.",
    TILE_DEEP,
    "screen-tile-deep",
    0.3249,
    0.1103,
    258.79,
    "#09316B",
    "main_menu.png",
    (1062, 768, 245, 167),
    6,
    27.12,
    "le bleu profond d'une tuile du menu"
);
screen_color!(
    "Le bleu moyen d'une tuile. Même recadrage, 3e classe → #245293 (14.56 %).",
    TILE_MID,
    "screen-tile-mid",
    0.4422,
    0.1182,
    257.59,
    "#245293",
    "main_menu.png",
    (1062, 768, 245, 167),
    6,
    14.56,
    "le bleu moyen d'une tuile du menu"
);
screen_color!(
    "Le bleu clair d'une tuile. Même recadrage, 4e classe → #4077C0 (14.38 %).",
    TILE_LIGHT,
    "screen-tile-light",
    0.5668,
    0.1277,
    256.22,
    "#4077C0",
    "main_menu.png",
    (1062, 768, 245, 167),
    6,
    14.38,
    "le bleu clair d'une tuile du menu"
);
screen_color!(
    "Le biseau clair sous une tuile. `--crop 1062,928,245,15 --k 3` → #EAEDEC (63.54 %).",
    TILE_BEVEL,
    "screen-tile-bevel",
    0.9435,
    0.0035,
    174.48,
    "#EAEDEC",
    "main_menu.png",
    (1062, 928, 245, 15),
    3,
    63.54,
    "le biseau clair sous une tuile"
);
screen_color!(
    "Le bleu nuit du pied de la fenêtre Informations. `--crop 500,232,77,49 --k 3` → #374469 (100 %).",
    INFO_FOOTER,
    "screen-info-footer",
    0.3928,
    0.0653,
    268.56,
    "#374469",
    "main_menu.png",
    (500, 232, 77, 49),
    3,
    100.0,
    "le pied de la fenêtre Informations"
);

/// Les 45 couleurs `--screen-*`, dans l'ordre du bloc `:root` de `game-screens.css`.
pub const SCREEN_COLORS: [ScreenColor; 45] = [
    HEADER_BLUE,
    HEADER_BLUE_DEEP,
    HEADER_ICON_GREY,
    TAB_KEY,
    TAB_LIGHT,
    TAB_ACTIVE,
    ROW_WHITE,
    ROW_LABEL,
    ROW_FOCUS,
    ROW_VALUE_STRIP,
    ROW_VALUE_STRIP_ACTIVE,
    ROW_VALUE_TEXT,
    ROW_ARROW,
    ROW_ARROW_LIGHT,
    SECTION_TITLE,
    SCROLLBAR,
    SCROLLBAR_TRACK,
    DESCRIPTION_GREY,
    KEY_CAP,
    COUNT_BADGE,
    CANVAS_PALE,
    CURSOR_YELLOW,
    CURSOR_GREEN,
    CURSOR_CYAN,
    PANEL_TOP,
    PANEL_BOTTOM,
    PANEL_BODY,
    PANEL_WATERMARK,
    PANEL_TITLE,
    CHECK_CYAN,
    CHECK_BOX,
    CHECK_BOX_EDGE,
    CHIP_WIND,
    CHIP_FIRE,
    CHIP_FOREST,
    CHIP_MOUNTAIN,
    BUTTON_PRIMARY,
    BUTTON_SECONDARY,
    BUTTON_TERTIARY,
    SEARCH_HINT,
    TILE_DEEP,
    TILE_MID,
    TILE_LIGHT,
    TILE_BEVEL,
    INFO_FOOTER,
];

/// Les sections du bloc `:root` : (indice de départ dans [`SCREEN_COLORS`], titre).
pub const SCREEN_SECTIONS: [(usize, &str); 9] = [
    (0, "Barre de titre (options.png)"),
    (3, "Bandeau d'onglets (options.png)"),
    (6, "Lignes de reglage (options.png, controls.png)"),
    (17, "Chrome : description, touches, compteur, fond"),
    (21, "Curseur (options.png)"),
    (24, "Panneau FILTRES (filters_elements.png)"),
    (29, "Cases a cocher et pastilles (filters_elements.png)"),
    (36, "Boutons et indice (filters_elements.png)"),
    (40, "Tuiles et fenetre Informations (main_menu.png)"),
];

/// Une mesure d'angle de bord, telle que `pixel mesurer --sombre S` la rend (ajustement aux
/// moindres carrés du bord, R² > 0,95 exigé pour être citée).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlantSample {
    /// La capture mesurée.
    pub capture: &'static str,
    /// La boîte `--boite x0 y0 x1 y1` et le masque `--sombre S`.
    pub command: &'static str,
    /// Angle du bord gauche par rapport à la verticale, en degrés (négatif = penché vers la droite, forme « / »).
    pub left_deg: f64,
    /// Angle du bord droit.
    pub right_deg: f64,
    /// Le pire R² des deux bords.
    pub r2: f64,
}

/// Les quatre ajustements de bord retenus (R² ≥ 0,988). Les tuiles du menu principal
/// (`main_menu.png`, fond photo) ne donnent PAS de bord ajustable (R² 0,60 / 0,01 avec
/// `--sombre 150`) : elles reprennent l'angle mesuré sur les onglets et les lignes.
pub const SLANT_SAMPLES: [SlantSample; 2] = [
    SlantSample {
        capture: "options.png",
        command: "pixel mesurer data/menu/options.png --boite 1050 262 1190 326 --sombre 100",
        left_deg: -10.70,
        right_deg: -10.64,
        r2: 0.992,
    },
    SlantSample {
        capture: "controls.png",
        command: "pixel mesurer data/menu/controls.png --boite 300 1000 420 1085 --sombre 120 (gauche) ; --boite 2200 1000 2330 1085 (droit)",
        left_deg: -9.41,
        right_deg: -9.34,
        r2: 0.988,
    },
];

/// La valeur CSS de `--game-skew` : la moyenne des quatre bords ajustés est −10,02°, arrondie à
/// `-10deg`. Le signe suit la convention CSS (`skewX(-10deg)` penche le haut vers la droite, la
/// forme « / » des captures) — ce que `nie_aphrody::pixel` documente comme `skewX(-angle)`
/// n'est vrai que pour son propre signe ; ici le bord mesuré est déjà négatif.
pub const SKEW_CSS: &str = "-10deg";

/// Les longueurs mesurées sur les captures (px à 2560×1440, ÷ 2 pour le canevas 1280×720 du
/// jeu, cf. `CLAUDE.md` § *Game screens*) : (nom, valeur CSS, provenance).
pub const SCREEN_LENGTHS: [(&str, &str, &str); 5] = [
    (
        "screen-header-height",
        "80px",
        "options.png : la barre couvre y 0..160 (160 px / 2)",
    ),
    (
        "screen-tab-height",
        "32px",
        "options.png : boite ajustee de l'onglet actif y 262..324 (63 px / 2)",
    ),
    (
        "screen-row-height",
        "40px",
        "controls.png : boite ajustee de la ligne focalisee y 1000..1078 (79 px / 2)",
    ),
    (
        "screen-key-cap-size",
        "20px",
        "options.png : la touche « V » tient dans 41x41 (crop 915,1350,41,41) / 2",
    ),
    (
        "screen-tile-height",
        "84px",
        "main_menu.png : la tuile couvre y 768..934 (167 px / 2)",
    ),
];

/// Le nom de la couleur `--screen-*` donné, ou `None`.
#[must_use]
pub fn find(name: &str) -> Option<ScreenColor> {
    SCREEN_COLORS.into_iter().find(|c| c.name == name)
}

/// Les règles de `game-screens.css` — le contrat de classes que consomment
/// `packages/inacord-ui/src/components/game/**` et `apps/nie-web`. Chaque règle cite la capture
/// dont elle vient ; chaque couleur est un `var(--screen-*)` mesuré ci-dessus ou un
/// `var(--jeu-*)` déjà servi par `game-tokens.css`.
pub const RULES: &str = r"
/* ---------------------------------------------------------------------------------------------
 * Utilitaire : le parallelogramme de la DA.
 * `--game-skew` est MESURE (voir SLANT_SAMPLES dans surfaces.rs) :
 *   pixel mesurer data/menu/options.png  --boite 1050 262 1190 326 --sombre 100  → -10.70° / -10.64° (R² 0.992)
 *   pixel mesurer data/menu/controls.png --boite 300 1000 420 1085 --sombre 120  → -9.41°            (R² 0.992)
 *   pixel mesurer data/menu/controls.png --boite 2200 1000 2330 1085 --sombre 120 → -9.34°            (R² 0.988)
 * ------------------------------------------------------------------------------------------- */
.game-skew {
	transform: skewX(var(--game-skew));
}
.game-skew > * {
	transform: skewX(calc(-1 * var(--game-skew)));
}

/* --- Barre de titre — options.png, controls.png, filters_elements.png ---------------------- */
.game-header-bar {
	display: flex;
	align-items: center;
	height: var(--screen-header-height);
	background: var(--screen-header-blue);
	color: var(--jeu-texte-vif);
	font-weight: var(--jeu-titre-poids);
	letter-spacing: var(--jeu-titre-espacement);
	box-shadow: var(--jeu-ombre-tuile);
}
.game-header-bar__icon {
	display: grid;
	place-items: center;
	width: var(--screen-header-height);
	height: var(--screen-header-height);
	background: var(--screen-header-icon-grey);
	color: var(--jeu-texte-vif);
	/* les rayures diagonales entre la tuile et le titre, penchees comme les onglets */
	border-right: calc(var(--jeu-espace-s) * 3) solid transparent;
	border-image: repeating-linear-gradient(
		calc(90deg + var(--game-skew)),
		var(--screen-header-blue-deep) 0 6px,
		var(--screen-header-blue) 6px 14px
	) 1;
}
.game-header-bar__title {
	padding-inline: var(--jeu-espace-l);
	font-size: 1.5rem;
}

/* --- Bandeau d'onglets — options.png ------------------------------------------------------- */
.game-tab-strip {
	display: flex;
	align-items: stretch;
	height: var(--screen-tab-height);
	background: var(--screen-tab-light);
	transform: skewX(var(--game-skew));
}
.game-tab-strip > * {
	transform: skewX(calc(-1 * var(--game-skew)));
}
.game-tab-strip__label {
	text-align: center;
}
.game-tab-strip__key {
	display: grid;
	place-items: center;
	min-width: calc(var(--screen-tab-height) * 2.5);
	background: var(--screen-tab-key);
	color: var(--jeu-texte-vif);
	font-size: 0.75rem;
}
.game-tab {
	display: grid;
	place-items: center;
	min-width: calc(var(--screen-tab-height) * 1.75);
	color: var(--screen-section-title);
	transition: background var(--jeu-duree-rapide) var(--jeu-courbe);
}
.game-tab--active {
	background: var(--screen-tab-active);
	color: var(--jeu-texte-vif);
}

/* --- Panneau FILTRES — filters_elements.png (et les 7 autres filters_*.png) ---------------- */
/* Le dialogue entier : `.game-panel` porte le cadre, `.game-filter-panel` le pose au centre de
   l'écran comme sur la capture, où il flotte au-dessus de la liste sans la remplacer. */
.game-filter-panel {
	display: flex;
	flex-direction: column;
	gap: var(--jeu-espace-m);
	max-height: 80vh;
	overflow: auto;
}
.game-panel {
	position: relative;
	display: flex;
	flex-direction: column;
	padding: var(--jeu-espace-m);
	border-radius: calc(var(--jeu-rayon) * 3);
	background: linear-gradient(180deg, var(--screen-panel-top), var(--screen-panel-bottom));
	color: var(--jeu-texte-vif);
	box-shadow: var(--jeu-ombre-panneau);
}
.game-panel__title {
	font-weight: 300;
	font-size: 2.25rem;
	line-height: 1;
	letter-spacing: 0.12em;
	text-transform: uppercase;
	color: var(--screen-panel-title);
}
.game-panel__body {
	position: relative;
	overflow: hidden;
	flex: 1;
	margin-top: var(--jeu-espace-m);
	padding: var(--jeu-espace-l);
	border-radius: calc(var(--jeu-rayon) * 2);
	background: var(--screen-panel-body);
}
.game-panel__watermark {
	position: absolute;
	inset: auto 0 0 auto;
	pointer-events: none;
	font-size: 18rem;
	line-height: 1;
	color: var(--screen-panel-watermark);
	transform: rotate(-12deg) translate(10%, 20%);
}
.game-panel__footer {
	display: flex;
	justify-content: space-between;
	align-items: center;
	gap: var(--jeu-espace-m);
	margin-top: var(--jeu-espace-m);
}

/* --- Cases a cocher — filters_*.png -------------------------------------------------------- */
.game-check {
	display: inline-flex;
	align-items: center;
	gap: var(--jeu-espace-m);
	color: var(--jeu-texte-vif);
	font-size: 1.25rem;
}
.game-check__box {
	display: grid;
	place-items: center;
	width: 26px;
	height: 26px;
	border: var(--jeu-bordure) solid var(--screen-check-box-edge);
	border-radius: var(--jeu-rayon);
	background: var(--screen-check-box);
	color: transparent;
	transition: color var(--jeu-duree-rapide) var(--jeu-courbe);
}
.game-check--checked .game-check__box {
	color: var(--screen-check-cyan);
}
.game-check__label {
	letter-spacing: var(--jeu-libelle-espacement);
}
/* Le compte à droite du libellé (« Vent 12 ») : présent, mais en retrait — sur la capture il ne
   dispute jamais la lecture au nom de la famille. */
.game-check__count {
	margin-left: var(--jeu-espace-s);
	opacity: 0.7;
}

/* --- Pastille d'icone coloree — filters_elements.png (Vent, Feu, Foret, Montagne) --------- */
.game-icon-chip {
	display: inline-grid;
	place-items: center;
	width: 32px;
	height: 32px;
	border-radius: var(--jeu-rayon);
	background: var(--game-icon-chip-color, var(--screen-chip-wind));
	color: var(--jeu-texte-vif);
}

/* --- Liste et lignes de reglage — options.png, controls.png -------------------------------- */
.game-setting-list {
	position: relative;
	display: flex;
	flex-direction: column;
	gap: var(--jeu-espace-s);
	padding-right: var(--jeu-espace-l);
}
.game-setting-list__scrollbar {
	position: absolute;
	top: 0;
	right: 0;
	bottom: 0;
	width: 6px;
	border-radius: 3px;
	background: var(--screen-scrollbar-track);
}
.game-setting-list__scrollbar::before {
	content: '';
	position: absolute;
	inset: 0 0 auto;
	height: var(--game-scroll-thumb, 40%);
	border-radius: inherit;
	background: var(--screen-scrollbar);
}
.game-setting-row {
	display: grid;
	grid-template-columns: 1fr minmax(12rem, 30%);
	align-items: center;
	height: var(--screen-row-height);
	background: var(--screen-row-white);
	color: var(--screen-row-label);
	box-shadow: var(--jeu-ombre-tuile);
	transform: skewX(var(--game-skew));
	transition: background var(--jeu-duree-rapide) var(--jeu-courbe);
}
.game-setting-row > * {
	transform: skewX(calc(-1 * var(--game-skew)));
}
.game-setting-row__label {
	padding-inline: var(--jeu-espace-l);
	font-size: 1.125rem;
}
.game-setting-row__value {
	display: flex;
	justify-content: center;
	align-items: center;
	align-self: stretch;
	gap: var(--jeu-espace-s);
	background: var(--screen-row-value-strip);
	color: var(--screen-row-value-text);
}
.game-setting-row__arrow {
	display: grid;
	place-items: center;
	width: 26px;
	height: 26px;
	border: var(--jeu-bordure) solid var(--screen-row-arrow-light);
	border-radius: var(--jeu-rayon);
	background: var(--screen-row-arrow);
	color: var(--jeu-texte-vif);
	visibility: hidden;
}
.game-setting-row__more {
	margin-left: auto;
	margin-right: var(--jeu-espace-s);
	color: var(--screen-scrollbar-track);
}
.game-setting-row--focused {
	background: var(--screen-row-focus);
	color: var(--jeu-texte-vif);
	box-shadow: var(--jeu-lueur-accent);
}
.game-setting-row--focused .game-setting-row__value {
	background: var(--screen-row-value-strip-active);
	color: var(--jeu-texte-vif);
}
.game-setting-row--focused .game-setting-row__arrow {
	visibility: visible;
}

/* --- Boutons — filters_elements.png (Confirmer, Reinitialiser) ---------------------------- */
.game-button-primary,
.game-button-secondary {
	display: inline-flex;
	justify-content: center;
	align-items: center;
	gap: var(--jeu-espace-m);
	min-height: var(--screen-row-height);
	padding-inline: var(--jeu-espace-xl);
	border: 0;
	color: var(--jeu-texte-vif);
	font-weight: var(--jeu-titre-poids);
	font-size: 1.25rem;
	letter-spacing: var(--jeu-titre-espacement);
	transform: skewX(var(--game-skew));
	transition: filter var(--jeu-duree-rapide) var(--jeu-courbe);
}
.game-button-primary > *,
.game-button-secondary > * {
	transform: skewX(calc(-1 * var(--game-skew)));
}
.game-button-primary {
	background: var(--screen-button-primary);
	box-shadow: var(--jeu-ombre-tuile);
}
.game-button-secondary {
	background: var(--screen-button-secondary);
}
.game-button-primary:hover,
.game-button-secondary:hover {
	filter: brightness(1.1);
}

/* --- Touches et barre d'indices — le bas de chaque capture ------------------------------- */
.game-key-cap {
	display: inline-grid;
	place-items: center;
	min-width: var(--screen-key-cap-size);
	height: var(--screen-key-cap-size);
	padding-inline: 4px;
	border-radius: var(--jeu-rayon);
	background: var(--screen-key-cap);
	color: var(--jeu-texte-vif);
	font-size: 0.75rem;
	font-weight: var(--jeu-titre-poids);
	line-height: 1;
}
.game-key-hint {
	display: inline-flex;
	align-items: center;
	gap: var(--jeu-espace-s);
	color: var(--jeu-nuit-profonde);
	font-size: 1.125rem;
}
.game-hint-bar {
	display: flex;
	justify-content: center;
	align-items: center;
	gap: var(--jeu-espace-xl);
	min-height: calc(var(--screen-row-height) * 1.5);
	padding-inline: var(--jeu-espace-l);
}

/* --- Curseur triangulaire — options.png, filters_elements.png, main_menu.png -------------- */
.game-cursor {
	display: inline-block;
	width: 36px;
	height: 38px;
	background: linear-gradient(
		135deg,
		var(--screen-cursor-yellow) 0 40%,
		var(--screen-cursor-green) 40% 70%,
		var(--screen-cursor-cyan) 70%
	);
	clip-path: polygon(0 0, 100% 50%, 0 100%);
	animation: game-cursor-nudge var(--jeu-duree-moyenne) var(--jeu-courbe) infinite alternate;
}
@keyframes game-cursor-nudge {
	from { transform: translateX(0); }
	to { transform: translateX(4px); }
}

/* --- Tuiles du menu principal — main_menu.png, main_menu_alt.png -------------------------- */
.game-tile-row {
	display: flex;
	gap: var(--jeu-espace-s);
	justify-content: center;
}
.game-tile {
	position: relative;
	display: grid;
	place-items: center;
	width: calc(var(--screen-tile-height) * 1.45);
	height: var(--screen-tile-height);
	background: linear-gradient(160deg, var(--screen-tile-light), var(--screen-tile-mid) 55%, var(--screen-tile-deep));
	border-bottom: 4px solid var(--screen-tile-bevel);
	color: var(--jeu-texte-vif);
	box-shadow: var(--jeu-ombre-tuile);
	transform: skewX(var(--game-skew));
	transition: transform var(--jeu-duree-rapide) var(--jeu-courbe), box-shadow var(--jeu-duree-rapide) var(--jeu-courbe);
}
.game-tile > * {
	transform: skewX(calc(-1 * var(--game-skew)));
}
.game-tile__icon {
	font-size: 2.5rem;
	line-height: 1;
}
.game-tile--active,
.game-tile:hover {
	transform: skewX(var(--game-skew)) translateY(-2px);
	box-shadow: var(--jeu-lueur-accent);
	outline: var(--jeu-bordure) solid var(--jeu-accent-cyan);
}

/* --- Champ de recherche — filters_elements.png (« Chercher par nom de joueur ») ----------- */
.game-search-bar {
	display: flex;
	align-items: center;
	gap: var(--jeu-espace-m);
	height: var(--screen-row-height);
	padding-inline: var(--jeu-espace-m);
	background: var(--screen-row-white);
	color: var(--screen-row-label);
	transform: skewX(var(--game-skew));
}
.game-search-bar > * {
	transform: skewX(calc(-1 * var(--game-skew)));
}
.game-search-bar__input {
	flex: 1;
	border: 0;
	background: transparent;
	color: inherit;
	font: inherit;
	outline: none;
}
.game-search-bar__input::placeholder {
	color: var(--screen-search-hint);
}
.game-search-bar__key {
	color: var(--jeu-texte-vif);
}

/* --- Barre de description — options.png, controls.png, shop.png --------------------------- */
.game-description-bar {
	display: flex;
	justify-content: center;
	align-items: center;
	min-height: calc(var(--screen-row-height) * 1.5);
	padding-inline: var(--jeu-espace-l);
	background: var(--screen-description-grey);
	color: var(--jeu-texte-vif);
	font-size: 1.25rem;
}

/* --- Compteur « 13/13 » — filters_elements.png -------------------------------------------- */
.game-count-badge {
	display: inline-flex;
	align-items: center;
	gap: var(--jeu-espace-s);
	padding: 2px var(--jeu-espace-m);
	border-radius: var(--jeu-rayon);
	background: var(--screen-count-badge);
	color: var(--jeu-texte-vif);
	font-variant-numeric: tabular-nums;
}

/* --- Fenetre Informations — main_menu.png (haut gauche) ----------------------------------- */
.game-info-window {
	overflow: hidden;
	border-radius: calc(var(--jeu-rayon) * 3);
	background: var(--screen-row-white);
	color: var(--jeu-nuit-profonde);
	box-shadow: var(--jeu-ombre-panneau);
}
.game-info-window__title {
	display: flex;
	justify-content: center;
	align-items: center;
	gap: var(--jeu-espace-m);
	padding: var(--jeu-espace-s) var(--jeu-espace-l);
	background: var(--screen-info-footer);
	color: var(--jeu-texte-vif);
	font-size: 1.125rem;
}
";

#[cfg(test)]
mod tests {
    use super::{SCREEN_COLORS, SCREEN_SECTIONS, SLANT_SAMPLES, find};

    #[test]
    fn chaque_nom_screen_est_unique_et_prefixe() {
        let mut noms: Vec<&str> = SCREEN_COLORS.iter().map(|c| c.name).collect();
        assert!(noms.iter().all(|n| n.starts_with("screen-")));
        let avant = noms.len();
        noms.sort_unstable();
        noms.dedup();
        assert_eq!(avant, noms.len(), "un nom --screen-* est dupliqué");
    }

    #[test]
    fn les_sections_sont_croissantes_et_dans_le_tableau() {
        let mut prev = 0;
        for (i, (start, _)) in SCREEN_SECTIONS.iter().enumerate() {
            assert!(i == 0 || *start > prev, "section {i} hors ordre");
            assert!(*start < SCREEN_COLORS.len());
            prev = *start;
        }
    }

    #[test]
    fn chaque_couleur_cite_une_capture_du_manifeste() {
        for c in SCREEN_COLORS {
            assert!(
                crate::screens::CAPTURES
                    .iter()
                    .any(|cap| cap.file == c.capture),
                "{} cite {} qui n'est pas une capture connue",
                c.name,
                c.capture
            );
            assert!(c.crop.2 > 0 && c.crop.3 > 0);
            assert!(c.share_pct > 0.0 && c.share_pct <= 100.0);
        }
    }

    #[test]
    fn find_retrouve_une_surface_connue() {
        assert_eq!(find("screen-panel-body"), Some(super::PANEL_BODY));
        assert_eq!(find("screen-inconnu"), None);
    }

    #[test]
    fn les_bords_ajustes_ont_un_r2_qui_autorise_a_les_citer() {
        for s in SLANT_SAMPLES {
            assert!(
                s.r2 >= 0.95,
                "{} : R² {} trop bas pour citer un angle",
                s.capture,
                s.r2
            );
            assert!(s.left_deg < 0.0 && s.right_deg < 0.0);
        }
    }

    /// Cross-check RÉEL : `nie_aphrody::pixel::palette_crop` est rappelée sur les captures
    /// locales pour trois ancres, et la classe dominante doit rester à ΔE Oklab < 0,02 de la
    /// constante transposée. Saute À VOIX HAUTE si `data/menu` est absent.
    #[test]
    fn les_ancres_suivent_la_mesure_reelle_de_nie_aphrody() {
        use nie_aphrody::pixel::{Crop, Image, palette_crop};
        let Some(dir) = crate::screens::captures_dir_if_present() else {
            let m =
                "GOLDEN SAUTE — data/menu absent : les ancres --screen-* ne sont pas re-mesurées";
            eprintln!("{m}");
            println!("{m}");
            return;
        };
        for ancre in [super::PANEL_BODY, super::ROW_FOCUS, super::TILE_DEEP] {
            let img = Image::charger(&dir.join(ancre.capture)).expect("capture lisible");
            let (x, y, w, h) = ancre.crop;
            let palette =
                palette_crop(&img, Crop { x, y, w, h }, usize::from(ancre.k)).expect("mesure");
            let dominante = &palette[0];
            assert_eq!(
                dominante.hex, ancre.hex,
                "{} : la classe dominante a changé",
                ancre.name
            );
            let [l, c, hh] = dominante.oklch;
            let (a1, b1) = (c * hh.to_radians().cos(), c * hh.to_radians().sin());
            let (a2, b2) = (
                ancre.oklch.c * ancre.oklch.h.to_radians().cos(),
                ancre.oklch.c * ancre.oklch.h.to_radians().sin(),
            );
            let de = ((l - ancre.oklch.l).powi(2) + (a1 - a2).powi(2) + (b1 - b2).powi(2)).sqrt();
            assert!(
                de < 0.02,
                "{} : ΔE {de:.4} entre la mesure et la constante",
                ancre.name
            );
            assert!(
                (dominante.part_pct - ancre.share_pct).abs() < 0.5,
                "{} : part {:.2} % mesurée contre {:.2} % transposée",
                ancre.name,
                dominante.part_pct,
                ancre.share_pct
            );
        }
    }
}
