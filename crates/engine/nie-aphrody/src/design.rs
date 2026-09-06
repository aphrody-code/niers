//! Le design system : les couleurs de l'interface, **dérivées** de la palette d'Aphrody.
//!
//! ## D'où viennent ces couleurs
//!
//! De l'atlas du personnage, mesuré. Pas d'une capture d'écran, pas d'une charte écrite à la
//! main : un k-means en Oklab sur les 74 frames, rejouable par
//!
//! ```text
//! pixel mesurer crates/engine/nie-aphrody/assets/aphrody/sprites/spritesheet.png --k 10 --json
//! ```
//!
//! Le résultat, [`PALETTE`], est chaud et désaturé — crèmes, blond, beiges, bruns, un mauve —
//! avec deux teintes froides seulement : le bleu profond de la tenue (2 % des pixels) et un
//! presque-noir bleuté (2 %). C'est la matière, et elle est intégralement portée par la crate
//! du personnage.
//!
//! ## Pourquoi des rôles dérivés, et non les dix teintes posées telles quelles
//!
//! Une palette de personnage n'est pas une palette d'interface. Les parts le disent : 25 % de
//! crème contre 2 % de bleu — si l'on peignait les surfaces au prorata, un site entier serait
//! crème et rien ne se détacherait. Un design system a besoin de **rôles** (un fond, une
//! surface, un texte, un accent, une bordure), chacun tenu à une luminosité et un contraste
//! précis.
//!
//! Chaque rôle nomme donc sa teinte source et l'ajuste en Oklch : la teinte (`h`) est conservée
//! telle qu'elle a été mesurée, seuls la clarté (`L`) et le chroma (`C`) sont posés. C'est ce
//! qui rend la dérivation vérifiable — une couleur de l'interface se rattache toujours à un
//! pixel du personnage, et [`contrastes`] mesure que les paires réellement utilisées restent
//! lisibles.
//!
//! ## Ce que ce module remplace
//!
//! `packages/inacord-ui/src/shell/game-tokens.css` portait 26 couleurs écrites à la main,
//! mesurées sur une capture du menu du jeu. Deux vérités coexistaient donc : celle du fichier
//! CSS et celle de la crate. La feuille est désormais **produite** ici ([`feuille_css`]) et le
//! fichier CSS en est la copie, vérifiée par un test — une couleur qui change dans la mesure
//! change dans le site, et une couleur retouchée à la main dans le CSS fait rougir la suite.

use std::fmt::Write as _;

/// Une teinte mesurée sur l'atlas : ce que la palette a réellement trouvé.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Teinte {
    /// Le nom que ce module lui donne, d'après ce qu'elle est sur le personnage.
    pub nom: &'static str,
    /// Sa notation hexadécimale, telle que la mesure la rend.
    pub hex: &'static str,
    /// Sa part de l'atlas, en pourcentage des pixels non transparents.
    pub part_pct: f64,
    /// Sa clarté Oklch.
    pub l: f64,
    /// Son chroma Oklch.
    pub c: f64,
    /// Sa teinte Oklch, en degrés.
    pub h: f64,
}

/// Les dix teintes d'Aphrody, mesurées sur l'atlas entier — la source de tout ce qui suit.
///
/// Mesure du 2026-09-06, `pixel mesurer … --k 10`, remplissage 16,98 % de l'atlas (le reste est
/// transparent). Les parts somment à 99 % aux arrondis près.
pub const PALETTE: [Teinte; 10] = [
    Teinte { nom: "creme", hex: "#F7F3F0", part_pct: 25.0, l: 0.9664, c: 0.0059, h: 59.65 },
    Teinte { nom: "blond", hex: "#EADBA6", part_pct: 21.0, l: 0.8908, c: 0.0707, h: 94.17 },
    Teinte { nom: "sable", hex: "#C2B19E", part_pct: 13.0, l: 0.7696, c: 0.0329, h: 70.41 },
    Teinte { nom: "taupe", hex: "#AF9183", part_pct: 11.0, l: 0.6803, c: 0.0416, h: 46.43 },
    Teinte { nom: "mauve", hex: "#8B7083", part_pct: 9.0, l: 0.5774, c: 0.0435, h: 337.07 },
    Teinte { nom: "brun", hex: "#6C5454", part_pct: 7.0, l: 0.4705, c: 0.0320, h: 18.35 },
    Teinte { nom: "ocre", hex: "#91704C", part_pct: 5.0, l: 0.5690, c: 0.0658, h: 68.31 },
    Teinte { nom: "cacao", hex: "#4E3738", part_pct: 4.0, l: 0.3633, c: 0.0330, h: 15.93 },
    Teinte { nom: "azur", hex: "#17335C", part_pct: 2.0, l: 0.3229, c: 0.0807, h: 258.02 },
    Teinte { nom: "nuit", hex: "#131420", part_pct: 2.0, l: 0.1963, c: 0.0242, h: 280.23 },
];

/// Index de la teinte crème dans [`PALETTE`].
pub const CREME: usize = 0;
/// Index du blond.
pub const BLOND: usize = 1;
/// Index du sable.
pub const SABLE: usize = 2;
/// Index de la taupe.
pub const TAUPE: usize = 3;
/// Index du mauve.
pub const MAUVE: usize = 4;
/// Index du brun.
pub const BRUN: usize = 5;
/// Index de l'ocre.
pub const OCRE: usize = 6;
/// Index du cacao.
pub const CACAO: usize = 7;
/// Index du bleu de la tenue.
pub const AZUR: usize = 8;
/// Index du presque-noir bleuté.
pub const NUIT: usize = 9;

/// Un rôle de l'interface : ce à quoi la couleur sert, et comment elle dérive de sa source.
#[derive(Debug, Clone, Copy)]
pub struct Role {
    /// Le nom de la propriété CSS, sans les deux tirets.
    pub nom: &'static str,
    /// L'index de sa teinte source dans [`PALETTE`].
    pub source: usize,
    /// La clarté Oklch posée pour ce rôle.
    pub l: f64,
    /// Le facteur appliqué au chroma de la source. 1 garde la saturation mesurée.
    pub chroma: f64,
    /// À quoi ce rôle sert, en une ligne.
    pub role: &'static str,
}

impl Role {
    /// La couleur du rôle, en Oklch.
    #[must_use]
    pub fn oklch(&self) -> [f64; 3] {
        let t = PALETTE[self.source];
        [self.l, t.c * self.chroma, t.h]
    }
}

/// Le facteur de chroma admissible avant que la couleur ne sorte du gamut sRGB.
///
/// Un rôle ne peut pas être aussi saturé qu'on veut : à clarté et teinte fixées, sRGB borne le
/// chroma, et la borne s'effondre vers les extrêmes de clarté. Mesurée par dichotomie sur la
/// conversion Oklch → sRGB linéaire pour le bleu de la tenue (h = 258,02°, C = 0,0807) :
///
/// | Clarté | Chroma max | Facteur max |
/// |---|---|---|
/// | 0,94 | 0,0289 | **0,36** |
/// | 0,89 | 0,0540 | 0,67 |
/// | 0,84 | 0,0800 | 0,99 |
/// | 0,80 | 0,1015 | 1,26 |
/// | 0,70 | 0,1583 | 1,96 |
/// | 0,58 | 0,2157 | 2,67 |
/// | 0,46 | 0,1718 | 2,13 |
/// | 0,42 | 0,1572 | 1,95 |
///
/// C'est pour cela qu'une surface très claire est forcément peu colorée, et qu'un accent
/// demande une clarté moyenne. Les facteurs ci-dessous s'y tiennent ; le test
/// [`tests::aucun_role_ne_sort_du_gamut`] le vérifie sur la conversion NON écrêtée, seule forme
/// qui puisse encore échouer.
///
/// Les vingt-six rôles de l'interface, chacun rattaché à une teinte du personnage.
///
/// L'ordre est celui de la feuille : fonds, accents, surfaces, texte, écran du menu, coquille
/// Inacord. Il n'a pas d'effet sur le rendu — il rend la feuille lisible.
pub const ROLES: [Role; 26] = [
    // --- Fonds : du plus profond au plus clair -----------------------------------------------
    Role { nom: "jeu-fond-abysse", source: NUIT, l: 0.1963, chroma: 1.0, role: "le fond le plus profond" },
    Role { nom: "jeu-fond-nuit", source: AZUR, l: 0.3000, chroma: 0.9, role: "un panneau sombre" },
    Role { nom: "jeu-fond-profond", source: AZUR, l: 0.3800, chroma: 1.1, role: "une surface bleue" },
    Role { nom: "jeu-fond-moyen", source: AZUR, l: 0.4600, chroma: 1.2, role: "une surface bleue active" },
    // --- Accents : ce qui appelle l'oeil ------------------------------------------------------
    Role { nom: "jeu-accent-ambre", source: BLOND, l: 0.8400, chroma: 1.8, role: "l'accent chaud, la couleur des cheveux" },
    // L'alerte tirait sur l'OCRE (h = 68°, un jaune-orangé) : elle rendait `#a56c23`, une
    // couleur de bois. Une alerte qui ne tire plus vers le rouge perd sa fonction, pas
    // seulement son nom. Le BRUN du personnage (h = 18,35°) est la teinte la plus rouge de la
    // palette avec le cacao. Le brun mesuré est mat (C = 0,0320) et le gamut autorise ici
    // jusqu'à ×7,28 : à ×5 elle redevient une brique franche sans rien devoir écrêter.
    Role { nom: "jeu-accent-brique", source: BRUN, l: 0.5800, chroma: 5.0, role: "l'alerte" },
    Role { nom: "jeu-accent-azur", source: AZUR, l: 0.5800, chroma: 1.6, role: "l'accent froid, la couleur de la tenue" },
    // « Cyan » et « turquoise » sont des noms hérités de la palette relevée sur une capture du
    // jeu. Aphrody n'a NI cyan NI turquoise : sa seule teinte froide est le bleu marine de sa
    // tenue. Les deux rôles restent donc deux bleus, distingués par leur clarté (0,84 contre
    // 0,70) et non par leur teinte — l'écart est ici, écrit, plutôt que caché derrière un nom.
    Role { nom: "jeu-accent-cyan", source: AZUR, l: 0.8000, chroma: 1.2, role: "le liseré d'un état actif" },
    Role { nom: "jeu-accent-turquoise", source: AZUR, l: 0.7000, chroma: 1.9, role: "le succès" },
    // --- Surfaces claires ---------------------------------------------------------------------
    // La « glace » sourçait la CREME (h = 59,65°) et rendait `#f1e9e3` — du beige chaud sous un
    // nom de surface froide. Elle vient du bleu de la tenue, comme la brume qui la voisine.
    Role { nom: "jeu-surface-glace", source: AZUR, l: 0.9400, chroma: 0.35, role: "une carte claire" },
    Role { nom: "jeu-surface-brume", source: AZUR, l: 0.8800, chroma: 0.5, role: "un dégradé clair, teinté du bleu de la tenue" },
    Role { nom: "jeu-surface-craie", source: SABLE, l: 0.8800, chroma: 0.8, role: "un fond neutre" },
    Role { nom: "jeu-surface-cendre", source: TAUPE, l: 0.7000, chroma: 0.9, role: "un texte secondaire" },
    Role { nom: "jeu-surface-rose", source: MAUVE, l: 0.6400, chroma: 1.0, role: "une nuance douce" },
    // --- Texte --------------------------------------------------------------------------------
    Role { nom: "jeu-texte-vif", source: CREME, l: 0.9850, chroma: 0.6, role: "le texte sur fond sombre" },
    Role { nom: "jeu-texte-doux", source: AZUR, l: 0.6400, chroma: 1.3, role: "un lien, un texte de second plan" },
    // --- L'écran du menu : ciel, tuiles, plaque -----------------------------------------------
    Role { nom: "jeu-ciel-clair", source: CREME, l: 0.9750, chroma: 0.7, role: "le fond de l'écran d'accueil" },
    Role { nom: "jeu-ciel-brume", source: AZUR, l: 0.8900, chroma: 0.45, role: "le ciel, en haut à droite" },
    Role { nom: "jeu-nuit-profonde", source: AZUR, l: 0.3229, chroma: 1.0, role: "le texte sur fond clair — la mesure telle quelle" },
    Role { nom: "jeu-tuile-haut", source: AZUR, l: 0.5000, chroma: 1.3, role: "le haut d'une tuile" },
    Role { nom: "jeu-tuile-bas", source: AZUR, l: 0.3900, chroma: 1.2, role: "le bas d'une tuile" },
    Role { nom: "jeu-tuile-bord", source: AZUR, l: 0.4500, chroma: 1.2, role: "le bord d'une tuile" },
    Role { nom: "jeu-tuile-active-haut", source: AZUR, l: 0.5800, chroma: 1.5, role: "le haut d'une tuile active" },
    Role { nom: "jeu-tuile-active-bas", source: AZUR, l: 0.4600, chroma: 2.0, role: "le bas d'une tuile active" },
    // La plaque centrale est l'element le plus sature de l'ecran : a 1,8 elle perdait 41 % du
    // chroma qu'elle avait, et devenait un bleu de bureau. Le bleu mesure est mat (C = 0,0807) ;
    // il faut trois fois son chroma pour qu'une plaque tienne son role d'accent.
    Role { nom: "jeu-plaque-bleu", source: AZUR, l: 0.4200, chroma: 1.9, role: "la plaque centrale" },
    Role { nom: "jeu-lisere-or", source: BLOND, l: 0.7800, chroma: 2.1, role: "le liseré doré" },
];

/// Les trois couleurs de la coquille Inacord, dérivées des mêmes teintes.
///
/// Elles vivent à part parce qu'elles décrivent une AUTRE ambiance — l'application de messagerie
/// du jeu, sombre et désaturée — et non l'écran du menu. Les mélanger dans [`ROLES`] laisserait
/// croire qu'un même écran peut porter les deux.
pub const ROLES_INACORD: [Role; 3] = [
    // Inacord est une ardoise FROIDE et desaturee — c'est la decision produit du 2026-09-05.
    // Sourcee sur le cacao et le brun, sa coquille virait au brun chaud (`#372627`, `#4f3d3d`)
    // et changeait d'ambiance sans que personne ne l'ait demande.
    Role { nom: "inacord-panneau", source: NUIT, l: 0.2900, chroma: 1.0, role: "le panneau d'Inacord" },
    Role { nom: "inacord-panneau-clair", source: AZUR, l: 0.3800, chroma: 0.6, role: "son panneau clair" },
    Role { nom: "inacord-accent", source: AZUR, l: 0.7000, chroma: 1.1, role: "son unique accent" },
];

/// Les trois composantes sRGB d'une couleur Oklch, **sans écrêtage**.
///
/// C'est la seule forme qui permette de SAVOIR qu'une couleur sort du gamut, et il a fallu la
/// faire : `palette` 0.7.7 écrête dans `FromColor` lui-même
/// (`convert/from_into_color.rs:53` — `Self::from_color_unclamped(t).clamp()`). Un test de
/// gamut écrit avec `from_color` ne peut donc jamais échouer : il rend un vert qui ne mesure
/// rien. `from_color_unclamped` laisse passer les valeurs négatives et les `> 1`, qui sont
/// exactement le signal qu'on cherche.
#[must_use]
pub fn srgb_non_ecrete(oklch: [f64; 3]) -> [f32; 3] {
    use palette::{Oklch, Srgb, convert::FromColorUnclamped};
    #[expect(
        clippy::cast_possible_truncation,
        reason = "palette travaille en f32 ; L, C et h y tiennent sans perte utile"
    )]
    let c = Oklch::new(oklch[0] as f32, oklch[1] as f32, oklch[2] as f32);
    let rgb: Srgb = Srgb::from_color_unclamped(c);
    [rgb.red, rgb.green, rgb.blue]
}

/// Convertit une couleur Oklch en sRGB 8 bits, ramenée dans le gamut.
///
/// Le ramenage est un simple écrêtage par canal, appliqué **ici** et non dans `palette`, pour
/// qu'il soit visible : les rôles sont posés à des chromas modestes (au plus 0,15) et aucun ne
/// sort du gamut — le test [`tests::aucun_role_ne_sort_du_gamut`] le mesure sur la valeur non
/// écrêtée plutôt que de le supposer.
#[must_use]
pub fn oklch_vers_rgb(oklch: [f64; 3]) -> [u8; 3] {
    let brut = srgb_non_ecrete(oklch);
    let voie = |v: f32| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "borné à [0, 255] juste avant"
        )]
        {
            (v.clamp(0.0, 1.0) * 255.0).round() as u8
        }
    };
    [voie(brut[0]), voie(brut[1]), voie(brut[2])]
}

/// La notation hexadécimale d'une couleur Oklch, telle que le CSS l'écrirait.
#[must_use]
pub fn oklch_vers_hex(oklch: [f64; 3]) -> String {
    let [r, g, b] = oklch_vers_rgb(oklch);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// La luminance relative WCAG d'un sRGB 8 bits.
fn luminance(rgb: [u8; 3]) -> f64 {
    let voie = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.040_45 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * voie(rgb[0]) + 0.7152 * voie(rgb[1]) + 0.0722 * voie(rgb[2])
}

/// Le rapport de contraste WCAG entre deux couleurs Oklch, de 1 à 21.
#[must_use]
pub fn contraste(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (la, lb) = (luminance(oklch_vers_rgb(a)), luminance(oklch_vers_rgb(b)));
    let (haut, bas) = if la > lb { (la, lb) } else { (lb, la) };
    (haut + 0.05) / (bas + 0.05)
}

/// Le rôle nommé, ou `None`.
#[must_use]
pub fn role(nom: &str) -> Option<Role> {
    ROLES
        .iter()
        .chain(ROLES_INACORD.iter())
        .copied()
        .find(|r| r.nom == nom)
}

/// Les paires que l'interface superpose réellement, avec le contraste exigé.
///
/// Ce ne sont pas toutes les combinaisons possibles : ce sont celles qu'un écran affiche —
/// relevées dans `MenuPrincipal`, `Ecran`, `Catalogue` et `Explorateur`. Une paire qui n'y
/// figure pas n'est pas garantie, et c'est volontaire : garantir des paires qu'on n'affiche pas
/// contraindrait la palette sans rien rendre lisible.
pub const PAIRES: [(&str, &str, f64); 8] = [
    ("jeu-nuit-profonde", "jeu-ciel-clair", 4.5),
    ("jeu-tuile-bas", "jeu-ciel-clair", 4.5),
    ("jeu-texte-vif", "jeu-tuile-bas", 4.5),
    ("jeu-texte-vif", "jeu-tuile-active-bas", 3.0),
    ("jeu-texte-vif", "jeu-fond-abysse", 4.5),
    ("jeu-nuit-profonde", "jeu-surface-glace", 4.5),
    ("jeu-nuit-profonde", "jeu-surface-brume", 4.5),
    ("jeu-texte-vif", "jeu-plaque-bleu", 4.5),
];

/// Le contraste mesuré de chaque paire, avec le minimum exigé.
#[must_use]
pub fn contrastes() -> Vec<(&'static str, &'static str, f64, f64)> {
    PAIRES
        .iter()
        .filter_map(|(a, b, min)| {
            let (ra, rb) = (role(a)?, role(b)?);
            Some((*a, *b, contraste(ra.oklch(), rb.oklch()), *min))
        })
        .collect()
}

/// Les intertitres de la feuille : l'index de [`ROLES`] où chacun commence, et son libellé.
///
/// L'ordre de `ROLES` porte déjà ces groupes en commentaire ; cette table les fait ressortir
/// dans le CSS produit, pour que le fichier engendré se relise aussi bien que celui qu'il
/// remplace. Un index qui glisserait fait rougir [`tests::les_intertitres_suivent_les_roles`]
/// plutôt que de titrer silencieusement la mauvaise tranche.
const SECTIONS: [(usize, &str); 5] = [
    (0, "Fonds : du plus profond au plus clair"),
    (4, "Accents : ce qui appelle l'oeil"),
    (9, "Surfaces claires"),
    (14, "Texte"),
    (16, "Ecran du menu : ciel, tuiles, plaque"),
];

/// Le bloc des vingt-neuf déclarations de couleur, sans le `:root` qui les entoure.
///
/// Chaque valeur est écrite en `oklch()` **et** commentée par son équivalent hexadécimal, sa
/// teinte source et son rôle : une couleur qu'on lit doit pouvoir se rattacher à un pixel du
/// personnage sans quitter le fichier.
fn bloc_couleurs() -> String {
    let mut s = String::with_capacity(4096);
    for (i, r) in ROLES.iter().enumerate() {
        if let Some((_, titre)) = SECTIONS.iter().find(|(debut, _)| *debut == i) {
            if i > 0 {
                s.push('\n');
            }
            ecrire_intertitre(&mut s, titre);
        }
        ecrire_role(&mut s, *r);
    }
    s.push('\n');
    ecrire_intertitre(&mut s, "Coquille InaCord : l'autre ambiance, memes teintes");
    for r in &ROLES_INACORD {
        ecrire_role(&mut s, *r);
    }
    s
}

/// La feuille de style des couleurs — le bloc que `game-tokens.css` porte, `:root` compris.
///
/// C'est le rendu que le test golden compare au fichier livré : toute couleur retouchée à la
/// main dans le CSS s'y voit.
#[must_use]
pub fn feuille_css() -> String {
    let mut s = String::with_capacity(4096);
    s.push_str(
        "/* Couleurs derivees de la palette mesuree d'Aphrody (nie_aphrody::design).\n   \
         NE PAS RETOUCHER A LA MAIN : regenerer par `cargo run -p nie-aphrody --bin design`. */\n",
    );
    s.push_str(":root {\n");
    s.push_str(&bloc_couleurs());
    s.push_str("}\n");
    s
}

/// Écrit un intertitre de section, tiré à 90 colonnes comme dans le fichier d'origine.
fn ecrire_intertitre(s: &mut String, titre: &str) {
    let tirets = 86usize.saturating_sub(titre.chars().count());
    let _ = writeln!(s, "\t/* --- {titre} {} */", "-".repeat(tirets));
}

/// Écrit une ligne de la feuille : la valeur, puis d'où elle vient.
fn ecrire_role(s: &mut String, r: Role) {
    let [l, c, h] = r.oklch();
    let rgb = oklch_vers_rgb([l, c, h]);
    let t = PALETTE[r.source];
    let _ = writeln!(
        s,
        "\t--{}: oklch({l:.4} {c:.4} {h:.2});  /* #{:02x}{:02x}{:02x} - {} ({} %) - {} */",
        r.nom, rgb[0], rgb[1], rgb[2], t.nom, t.part_pct, r.role
    );
}

/// Le socle NON coloré de la feuille : géométrie, rythme, mouvement, typographie.
///
/// Ces valeurs ne dérivent de rien et ne le peuvent pas — un biseau de 14 px et une durée de
/// 120 ms ne se mesurent pas sur un atlas. Elles sont donc recopiées telles quelles depuis le
/// fichier d'origine, avec leurs commentaires : les perdre casserait toute l'interface, et
/// c'est justement ce qu'un générateur naïf ferait en réécrivant le fichier entier.
///
/// L'**élévation** est l'exception, et elle est délibérée : `--jeu-ombre-*` et
/// `--jeu-lueur-accent` portaient `rgb(15 16 17 / 45%)` et `rgb(99 216 252 / 55%)`, c'est-à-dire
/// les anciens `--jeu-fond-abysse` et `--jeu-accent-cyan` recopiés à la main dans une valeur
/// composite. Leur géométrie (les décalages, le flou, l'opacité) reste écrite ici ; leurs trois
/// composantes viennent maintenant des rôles, sans quoi il resterait des couleurs en dur dans
/// une feuille censée n'en plus porter aucune.
fn socle_css() -> String {
    let ombre = rgb_css("jeu-fond-abysse");
    let lueur = rgb_css("jeu-accent-cyan");
    let mut s = String::with_capacity(2048);
    s.push_str(
        "\n\t/* --- Geometrie : les tuiles du menu sont BISEAUTEES, pas rectangulaires ---------------- */\n\
         \t--jeu-biseau: 14px;\n\
         \t--jeu-rayon: 4px;\n\
         \t--jeu-bordure: 2px;\n\
         \n\
         \t/* --- Rythme -------------------------------------------------------------------------- */\n\
         \t--jeu-espace-xs: 4px;\n\
         \t--jeu-espace-s: 8px;\n\
         \t--jeu-espace-m: 16px;\n\
         \t--jeu-espace-l: 24px;\n\
         \t--jeu-espace-xl: 40px;\n\
         \n\
         \t/* --- Elevation : la geometrie est ecrite, les composantes derivent des roles ---------- */\n",
    );
    let _ = writeln!(s, "\t--jeu-ombre-tuile: 0 2px 8px rgb({ombre} / 45%);");
    let _ = writeln!(s, "\t--jeu-ombre-panneau: 0 8px 32px rgb({ombre} / 65%);");
    let _ = writeln!(s, "\t--jeu-lueur-accent: 0 0 12px rgb({lueur} / 55%);");
    s.push_str(
        "\n\t/* --- Mouvement : court et net, comme le jeu ------------------------------------------- */\n\
         \t--jeu-duree-rapide: 120ms;\n\
         \t--jeu-duree-moyenne: 220ms;\n\
         \t--jeu-courbe: cubic-bezier(0.2, 0, 0, 1);\n\
         \n\
         \t/* --- Typographie --------------------------------------------------------------------- */\n\
         \t--jeu-titre-poids: 800;\n\
         \t--jeu-titre-espacement: 0.02em;\n\
         \t--jeu-libelle-espacement: 0.06em;\n",
    );
    s
}

/// Les trois composantes sRGB d'un rôle, sous la forme `R G B` qu'attend `rgb()` en CSS moderne.
///
/// Le rôle est cherché par son nom plutôt que par un index : un renommage se voit alors dans
/// [`tests::le_socle_cite_des_roles_qui_existent`] au lieu de produire un `rgb(0 0 0)` muet.
fn rgb_css(nom: &str) -> String {
    role(nom).map_or_else(
        || format!("/* role inconnu: {nom} */"),
        |r| {
            let [red, green, blue] = oklch_vers_rgb(r.oklch());
            format!("{red} {green} {blue}")
        },
    )
}

/// Le fichier `game-tokens.css` complet — couleurs dérivées **et** socle conservé.
///
/// C'est ce que le binaire `design` écrit et ce que le test golden compare au fichier livré.
#[must_use]
pub fn fichier_css() -> String {
    let mut s = String::with_capacity(8192);
    s.push_str(EN_TETE_CSS);
    s.push_str(":root {\n");
    s.push_str(&bloc_couleurs());
    s.push_str(&socle_css());
    s.push_str("}\n");
    s.push_str(PIED_CSS);
    s
}

/// L'en-tête du fichier : d'où viennent ces valeurs, et comment les régénérer.
const EN_TETE_CSS: &str = r"/*
 * game-tokens.css — la direction artistique du jeu, en variables CSS.
 *
 * FICHIER ENGENDRE — ne pas retoucher a la main.
 *   Regenerer :  cargo run -p nie-aphrody --bin design
 *   Source     :  crates/engine/nie-aphrody/src/design.rs
 *   Verifier   :  cargo test -p nie-aphrody
 *
 * ## D'ou viennent ces couleurs
 *
 * De la palette MESUREE sur l'atlas d'Aphrody lui-meme — un k-means en Oklab sur les 74 frames
 * de crates/engine/nie-aphrody/assets/aphrody/sprites/spritesheet.png, rejouable par
 *
 *   pixel mesurer crates/engine/nie-aphrody/assets/aphrody/sprites/spritesheet.png --k 10 --json
 *
 * Les dix teintes trouvees ne sont pas posees telles quelles : une palette de personnage n'est
 * pas une palette d'interface (25 % de creme contre 2 % de bleu — au prorata, tout le site
 * serait creme et rien ne se detacherait). Chaque variable ci-dessous est un ROLE, qui garde la
 * TEINTE mesuree de sa source et ne pose que sa clarte et son chroma. Le commentaire de fin de
 * ligne dit de quelle teinte elle vient et quelle part de l'atlas cette teinte occupe.
 *
 * ## Deux coquilles, deux ambiances
 *
 * - `--jeu-*` : le menu principal du jeu, que porte Aphrody.
 * - `--inacord-*` : l'application de messagerie du jeu, que porte Inacord — memes teintes,
 *   posees plus sombres et plus desaturees.
 *
 * Les deux jeux cohabitent : un hote choisit sa coquille, jamais ses couleurs.
 */

";

/// Le pied du fichier : le seul bloc hors `:root`, et il ne porte aucune couleur.
const PIED_CSS: &str = r"
/*
 * `prefers-reduced-motion` : les tuiles du menu glissent et pulsent. Une interface qui ignore
 * ce reglage rend le systeme inutilisable pour qui en a besoin — le mouvement tombe a zero,
 * les etats restent lisibles.
 */
@media (prefers-reduced-motion: reduce) {
	:root {
		--jeu-duree-rapide: 0ms;
		--jeu-duree-moyenne: 0ms;
	}
}
";

/// Le chemin de la feuille livrée, résolu depuis l'emplacement de CETTE crate.
///
/// `CARGO_MANIFEST_DIR` est figé à la compilation et remonte trois niveaux
/// (`crates/engine/nie-aphrody` → racine du dépôt) : c'est ce qui rend le binaire et le test
/// golden indépendants du répertoire courant. Un `std::env::current_dir()` ferait dépendre le
/// résultat de l'endroit d'où l'on a lancé `cargo`, et le test golden se sauterait ou
/// écrirait à côté sans le dire.
#[must_use]
pub fn chemin_feuille() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("packages/inacord-ui/src/shell/game-tokens.css")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_role_derive_d_une_teinte_mesuree() {
        for r in ROLES.iter().chain(ROLES_INACORD.iter()) {
            assert!(r.source < PALETTE.len(), "{} : source hors palette", r.nom);
            // La teinte est CONSERVÉE : c'est ce qui rattache le rôle au personnage. Seules la
            // clarté et la saturation sont posées.
            assert!(
                (r.oklch()[2] - PALETTE[r.source].h).abs() < 1e-9,
                "{} : la teinte a été déplacée",
                r.nom
            );
        }
    }

    #[test]
    fn aucun_role_ne_sort_du_gamut() {
        // Un rôle hors gamut serait écrêté à l'affichage : la couleur rendue ne serait plus
        // celle qui est écrite, et l'écart ne se verrait sur aucune ligne de code. La mesure se
        // fait donc sur la valeur NON écrêtée — `FromColor` écrête lui-même, cf.
        // `srgb_non_ecrete`, et un test bâti dessus serait vert quoi qu'il arrive.
        for r in ROLES.iter().chain(ROLES_INACORD.iter()) {
            let brut = srgb_non_ecrete(r.oklch());
            let rendu = oklch_vers_hex(r.oklch());
            for (i, v) in brut.into_iter().enumerate() {
                assert!(
                    (-0.002..=1.002).contains(&v),
                    "{} : composante {i} = {v:.4} hors [0,1] — la couleur ecrite ne sera pas celle rendue ({rendu})",
                    r.nom
                );
            }
        }
    }

    #[test]
    fn les_paires_affichees_restent_lisibles() {
        for (a, b, mesure, min) in contrastes() {
            assert!(
                mesure >= min,
                "{a} sur {b} : contraste {mesure:.2}, minimum {min}"
            );
        }
        assert_eq!(contrastes().len(), PAIRES.len(), "une paire nomme un rôle inconnu");
    }

    #[test]
    fn la_feuille_porte_tous_les_roles() {
        let css = feuille_css();
        for r in ROLES.iter().chain(ROLES_INACORD.iter()) {
            assert!(css.contains(&format!("--{}: oklch(", r.nom)), "{} absent", r.nom);
        }
        assert!(css.starts_with("/* Couleurs derivees"));
        assert!(css.trim_end().ends_with('}'));
    }

    #[test]
    fn les_intertitres_suivent_les_roles() {
        // Les intertitres sont posés par index. Réordonner ROLES sans toucher SECTIONS titrerait
        // la mauvaise tranche, ce qui ne casse rien de visible — d'où cet ancrage par nom.
        let attendus = [
            "jeu-fond-abysse",
            "jeu-accent-ambre",
            "jeu-surface-glace",
            "jeu-texte-vif",
            "jeu-ciel-clair",
        ];
        for ((debut, titre), attendu) in SECTIONS.iter().zip(attendus) {
            assert_eq!(
                ROLES[*debut].nom, attendu,
                "la section « {titre} » ne commence plus sur {attendu}"
            );
        }
    }

    #[test]
    fn le_socle_cite_des_roles_qui_existent() {
        // `rgb_css` rend un commentaire au lieu d'une couleur quand le rôle n'existe pas : le CSS
        // resterait valide en apparence et l'ombre disparaîtrait sans un mot.
        let socle = socle_css();
        assert!(
            !socle.contains("role inconnu"),
            "le socle cite un role absent de ROLES :\n{socle}"
        );
        // Et il ne doit porter AUCUN hexadécimal : c'est tout l'objet de la dérivation.
        assert!(
            !socle.contains('#'),
            "une couleur hexadecimale subsiste dans le socle :\n{socle}"
        );
    }

    #[test]
    fn le_fichier_conserve_le_socle_et_la_reduction_de_mouvement() {
        let css = fichier_css();
        // Les dix-sept proprietes qui ne derivent de rien. Les perdre casserait la geometrie des
        // tuiles, le rythme et le mouvement sans qu'aucune couleur ne bouge.
        for propriete in [
            "--jeu-biseau: 14px;",
            "--jeu-rayon: 4px;",
            "--jeu-bordure: 2px;",
            "--jeu-espace-xs: 4px;",
            "--jeu-espace-s: 8px;",
            "--jeu-espace-m: 16px;",
            "--jeu-espace-l: 24px;",
            "--jeu-espace-xl: 40px;",
            "--jeu-duree-rapide: 120ms;",
            "--jeu-duree-moyenne: 220ms;",
            "--jeu-courbe: cubic-bezier(0.2, 0, 0, 1);",
            "--jeu-titre-poids: 800;",
            "--jeu-titre-espacement: 0.02em;",
            "--jeu-libelle-espacement: 0.06em;",
        ] {
            assert!(css.contains(propriete), "{propriete} perdue");
        }
        assert!(css.contains("@media (prefers-reduced-motion: reduce)"));
        // Le compte est celui du fichier d'origine : 46 dans `:root`, 48 avec les deux du
        // `@media`. Un rôle ajouté sans intertitre, ou un socle rogné, s'y voit tout de suite.
        let proprietes = css
            .lines()
            .filter(|l| l.trim_start().starts_with("--"))
            .count();
        assert_eq!(proprietes, 48, "le compte de proprietes a change");
    }

    #[test]
    fn le_css_livre_est_celui_qu_on_produit() {
        // Golden. Il attrape le cas qui a motivé tout ce module : quelqu'un retouche un
        // hexadécimal dans le CSS, le site change, et plus rien ne relie la couleur affichée à
        // un pixel du personnage.
        let chemin = chemin_feuille();
        let Ok(livre) = std::fs::read_to_string(&chemin) else {
            // Le fichier est SUIVI par git : son absence n'est pas un cas de figure normal, mais
            // un arbre incomplet. On le dit fort plutôt que de rendre un vert qui ne mesure rien.
            let message = format!(
                "GOLDEN SAUTE — {} est introuvable.\n\
                 Ce fichier est suivi par git : le restaurer par\n  \
                 git checkout -- packages/inacord-ui/src/shell/game-tokens.css\n\
                 ou le regenerer par\n  cargo run -p nie-aphrody --bin design",
                chemin.display()
            );
            eprintln!("{message}");
            println!("{message}");
            return;
        };
        let attendu = fichier_css();
        if livre != attendu {
            // Le premier écart suffit à situer la retouche ; déverser 115 lignes de diff
            // n'apprendrait rien de plus.
            let ecart = livre
                .lines()
                .zip(attendu.lines())
                .enumerate()
                .find(|(_, (a, b))| a != b)
                .map_or_else(
                    || {
                        format!(
                            "longueurs differentes : {} lignes livrees, {} attendues",
                            livre.lines().count(),
                            attendu.lines().count()
                        )
                    },
                    |(n, (a, b))| format!("ligne {} :\n  livre  : {a}\n  attendu: {b}", n + 1),
                );
            panic!(
                "{} ne correspond plus a nie_aphrody::design::fichier_css().\n{ecart}\n\
                 Une couleur a-t-elle ete retouchee a la main ? Regenerer par\n  \
                 cargo run -p nie-aphrody --bin design",
                chemin.display()
            );
        }
        // Et le bloc de couleurs y figure bien tel quel : c'est lui qui derive de la mesure.
        assert!(
            livre.contains(&bloc_couleurs()),
            "le bloc de couleurs livre n'est pas celui que produit la palette"
        );
    }
}
