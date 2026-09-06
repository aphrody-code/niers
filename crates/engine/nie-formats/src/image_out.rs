//! Encodage d'une image décodée vers les formats d'échange.
//!
//! Entrée unique : du RGBA8 (ce que rendent [`crate::g4tx_decode::decode_best_to_rgba`] et le
//! rendu). Sortie : PNG, WebP, GIF, JPEG, BMP, TGA, TIFF, QOI.
//!
//! ## Le PNG ne passe pas par `image`
//!
//! Le PNG produit par [`crate::g4tx_decode::encode_rgba_to_png`] (crate `png`) est **byte-identique
//! aux références publiées** sur `cdn.rosegriffon.fr` — c'est l'oracle de non-régression du
//! projet. Rien ne garantit qu'un autre encodeur choisisse les mêmes filtres ni le même niveau de
//! compression, donc le PNG garde son chemin historique et `image` ne sert qu'aux autres formats.
//! Le test `le_png_reste_sur_la_crate_png` verrouille cette règle.
//!
//! ## Sans perte, sauf JPEG
//!
//! WebP est encodé en **VP8L** (sans perte) : `image-webp` n'implémente que celui-là, ce qui tombe
//! bien — les assets du jeu sont des textures, pas des photos. GIF impose une palette de 256
//! couleurs : la conversion est donc destructrice pour une texture 32 bits, ce que
//! [`ImageOut::sans_perte`] annonce.

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use image::{ExtendedColorType, ImageEncoder};

/// Format d'image en sortie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ImageOut {
    /// PNG (sans perte) — chemin historique, byte-exact contre les références publiées.
    Png,
    /// WebP sans perte (VP8L).
    Webp,
    /// GIF — palette de 256 couleurs, donc **avec perte** sur une texture 32 bits.
    Gif,
    /// JPEG — avec perte, qualité 90.
    Jpeg,
    /// BMP (sans perte).
    Bmp,
    /// TGA (sans perte).
    Tga,
    /// TIFF (sans perte).
    Tiff,
    /// QOI (sans perte).
    Qoi,
}

/// Qualité JPEG utilisée à l'encodage. 90 : le palier au-delà duquel le gain visuel ne paie plus
/// la taille, sur les textures du jeu comme ailleurs.
const QUALITE_JPEG: u8 = 90;

impl ImageOut {
    /// Extension de fichier canonique, sans le point.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            ImageOut::Png => "png",
            ImageOut::Webp => "webp",
            ImageOut::Gif => "gif",
            ImageOut::Jpeg => "jpg",
            ImageOut::Bmp => "bmp",
            ImageOut::Tga => "tga",
            ImageOut::Tiff => "tiff",
            ImageOut::Qoi => "qoi",
        }
    }

    /// `true` si l'encodage conserve exactement les pixels d'entrée.
    ///
    /// GIF quantifie sur 256 couleurs et JPEG est un codec à perte : les deux dégradent une
    /// texture RGBA8. Les autres restituent l'image à l'identique.
    #[must_use]
    pub const fn sans_perte(self) -> bool {
        !matches!(self, ImageOut::Gif | ImageOut::Jpeg)
    }

    /// `true` si le format transporte un canal alpha.
    ///
    /// JPEG n'en a pas : l'alpha est aplati sur du noir à l'encodage. Les textures du jeu étant
    /// très souvent détourées, c'est une raison de plus de préférer WebP.
    #[must_use]
    pub const fn garde_alpha(self) -> bool {
        !matches!(self, ImageOut::Jpeg)
    }

    /// Reconnaît un format depuis une extension ou un nom de format, insensible à la casse
    /// (`"webp"`, `".WebP"`, `"jpeg"` et `"jpg"` sont acceptés).
    #[must_use]
    pub fn depuis_extension(ext: &str) -> Option<Self> {
        let e = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        Some(match e.as_str() {
            "png" => ImageOut::Png,
            "webp" => ImageOut::Webp,
            "gif" => ImageOut::Gif,
            "jpg" | "jpeg" => ImageOut::Jpeg,
            "bmp" => ImageOut::Bmp,
            "tga" => ImageOut::Tga,
            "tif" | "tiff" => ImageOut::Tiff,
            "qoi" => ImageOut::Qoi,
            _ => return None,
        })
    }

    /// Tous les formats gérés, dans l'ordre d'affichage de l'aide.
    pub const TOUS: [ImageOut; 8] = [
        ImageOut::Png,
        ImageOut::Webp,
        ImageOut::Gif,
        ImageOut::Jpeg,
        ImageOut::Bmp,
        ImageOut::Tga,
        ImageOut::Tiff,
        ImageOut::Qoi,
    ];
}

/// Encode une image RGBA8 vers `format`.
///
/// `rgba` doit contenir exactement `largeur × hauteur × 4` octets.
///
/// # Erreurs
///
/// Rend un message si les dimensions ne correspondent pas à la taille du tampon, ou si
/// l'encodeur échoue.
pub fn encoder_rgba(
    rgba: &[u8],
    largeur: u32,
    hauteur: u32,
    format: ImageOut,
) -> Result<Vec<u8>, String> {
    let attendu = (largeur as usize)
        .checked_mul(hauteur as usize)
        .and_then(|p| p.checked_mul(4))
        .ok_or_else(|| "dimensions hors bornes".to_string())?;
    if rgba.len() != attendu {
        return Err(alloc::format!(
            "tampon de {} octets pour {largeur}×{hauteur} RGBA (attendu {attendu})",
            rgba.len()
        ));
    }
    if largeur == 0 || hauteur == 0 {
        return Err("image de dimension nulle".to_string());
    }

    // Le PNG garde la crate `png` : c'est lui qui porte la garantie byte-exact.
    if format == ImageOut::Png {
        return crate::g4tx_decode::encode_rgba_to_png(rgba, largeur as usize, hauteur as usize)
            .ok_or_else(|| "échec de l'encodage PNG".to_string());
    }

    let mut sortie = Vec::new();
    let couleur = if format.garde_alpha() {
        ExtendedColorType::Rgba8
    } else {
        ExtendedColorType::Rgb8
    };

    // JPEG n'a pas d'alpha : on aplatit en RGB plutôt que de laisser l'encodeur refuser
    // l'entrée. Aplatir explicitement rend la perte visible ici, pas dans un message obscur.
    let rgb_aplati;
    let pixels: &[u8] = if format.garde_alpha() {
        rgba
    } else {
        rgb_aplati = aplatir_en_rgb(rgba);
        &rgb_aplati
    };

    let r = match format {
        ImageOut::Png => unreachable!("traité plus haut"),
        ImageOut::Webp => image::codecs::webp::WebPEncoder::new_lossless(&mut sortie)
            .write_image(pixels, largeur, hauteur, couleur),
        ImageOut::Gif => image::codecs::gif::GifEncoder::new(&mut sortie)
            .encode(pixels, largeur, hauteur, couleur),
        ImageOut::Jpeg => {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut sortie, QUALITE_JPEG)
                .write_image(pixels, largeur, hauteur, couleur)
        }
        ImageOut::Bmp => image::codecs::bmp::BmpEncoder::new(&mut sortie)
            .write_image(pixels, largeur, hauteur, couleur),
        ImageOut::Tga => image::codecs::tga::TgaEncoder::new(&mut sortie)
            .write_image(pixels, largeur, hauteur, couleur),
        ImageOut::Tiff => image::codecs::tiff::TiffEncoder::new(std::io::Cursor::new(&mut sortie))
            .write_image(pixels, largeur, hauteur, couleur),
        ImageOut::Qoi => image::codecs::qoi::QoiEncoder::new(&mut sortie)
            .write_image(pixels, largeur, hauteur, couleur),
    };
    r.map_err(|e| alloc::format!("encodage {} : {e}", format.extension()))?;
    Ok(sortie)
}

/// Aplatit du RGBA8 en RGB8 en composant sur du noir (JPEG n'a pas de canal alpha).
fn aplatir_en_rgb(rgba: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for px in rgba.chunks_exact(4) {
        let a = u32::from(px[3]);
        for c in &px[..3] {
            rgb.push(((u32::from(*c) * a) / 255) as u8);
        }
    }
    rgb
}

/// Décode un `.g4tx` puis l'encode vers `format` — le chemin complet d'une conversion de texture.
///
/// `basename` = nom du fichier source sans dossier ni extension (cf.
/// [`crate::g4tx_decode::basename_of`]) : il départage les conteneurs qui portent plusieurs
/// textures. `""` reste licite quand l'appelant n'a que des octets.
///
/// # Erreurs
///
/// Rend un message si le G4TX n'est pas décodable ou si l'encodage échoue.
#[cfg(feature = "textures")]
pub fn g4tx_vers(g4tx: &[u8], basename: &str, format: ImageOut) -> Result<Vec<u8>, String> {
    let (w, h, rgba) = crate::g4tx_decode::decode_best_to_rgba(g4tx, basename)
        .ok_or_else(|| "G4TX non décodable".to_string())?;
    encoder_rgba(&rgba, w, h, format)
}

/// Décode les planches de couleur d'un conteneur de visage.
///
/// Un conteneur en porte souvent plusieurs, latéralisées (`eye_L_00` et `eye_R_00`), chacune
/// accompagnée d'un `<nom>msk` de mêmes dimensions.
///
/// Le compagnon `<nom>msk` porte l'information **quand il varie**, et seulement alors. C'est une
/// règle mesurée, après deux erreurs symétriques :
///
/// - le poser systématiquement en alpha est faux : sur `face_00msk` comme sur `pupil_L_00msk`, il
///   est uniforme à 0,5 (écart-type **nul**), et l'appliquer rend toute la planche uniformément
///   semi-transparente, effaçant les variations ;
/// - l'ignorer systématiquement est faux aussi : les planches de couleur des reflets sont blanches
///   et **identiques** d'une variante à l'autre (`highlight_L_00` et `highlight_L_09` : R = G = B =
///   A = 255, écart-type nul partout), alors que leurs masques diffèrent — `highlight_L_00msk` est
///   uniforme, `highlight_L_09msk` varie (écart-type 41). Pour cette famille, **tout** le dessin
///   est dans le masque.
///
/// Un masque n'est donc posé en alpha que s'il varie **et** que la planche de couleur, elle, est
/// muette. Sans cette seconde condition, la bouche devient inerte : son dessin vit dans la
/// couleur, et le masque l'écrase.
#[cfg(feature = "textures")]
#[must_use]
pub fn decoder_planches(g4tx: &[u8]) -> Vec<(u32, u32, Vec<u8>)> {
    crate::g4tx::base_color_texture_names(g4tx)
        .into_iter()
        .filter_map(|nom| {
            let (w, h, mut rgba) = crate::g4tx_decode::decode_named_to_rgba(g4tx, &nom)?;
            // L'information est soit dans la COULEUR, soit dans le MASQUE — jamais dans les deux.
            // Le masque ne sert donc que là où la planche de couleur est muette : appliqué
            // partout, il rend la bouche inerte, dont le dessin vit bel et bien dans la couleur.
            let couleur_muette = canal_uniforme(&rgba);
            if let Some((mw, mh, masque)) =
                crate::g4tx_decode::decode_named_to_rgba(g4tx, &alloc::format!("{nom}msk")).filter(
                    |(mw, mh, m)| {
                        couleur_muette
                            && (*mw, *mh) == (w, h)
                            && m.len() >= rgba.len()
                            && !canal_uniforme(m)
                    },
                )
            {
                let _ = (mw, mh);
                for i in (0..rgba.len()).step_by(4) {
                    rgba[i + 3] = masque[i];
                }
            }
            Some((w, h, rgba))
        })
        .collect()
}

/// Vrai si ce masque désigne des ZONES : un fond rouge franc et des régions d'une autre couleur.
///
/// Un masque de découpe est gris — son canal rouge EST l'opacité. Un masque de zones a un fond
/// rouge saturé et peint ses régions autrement : noir sur `mouth_01msk`, vert sur le contour de
/// paupière de `eye_L_01msk`, bleu sur l'ovale de `pupil_L_01msk`.
#[cfg(feature = "textures")]
#[must_use]
pub fn masque_de_zones(masque: &[u8]) -> bool {
    crate::planche::part_zone_brute(masque, crate::planche::Zone::Rouge)
        > crate::planche::PART_FOND_ZONES
}

/// Une planche décodée et, s'il en existe un, son masque de zones.
#[cfg(feature = "textures")]
pub type PlancheEtMasque = (u32, u32, alloc::vec::Vec<u8>, Option<alloc::vec::Vec<u8>>);

/// Décode les planches d'un G4TX en rendant à part leur masque de zones, quand elles en ont un.
#[cfg(feature = "textures")]
#[must_use]
pub fn decoder_planches_et_masques(g4tx: &[u8]) -> alloc::vec::Vec<PlancheEtMasque> {
    crate::g4tx::base_color_texture_names(g4tx)
        .into_iter()
        .filter_map(|nom| {
            let (w, h, mut rgba) = crate::g4tx_decode::decode_named_to_rgba(g4tx, &nom)?;
            let couleur_muette = canal_uniforme(&rgba);
            let masque =
                crate::g4tx_decode::decode_named_to_rgba(g4tx, &alloc::format!("{nom}msk")).filter(
                    |(mw, mh, m)| {
                        (*mw, *mh) == (w, h) && m.len() >= rgba.len() && !canal_uniforme(m)
                    },
                );
            let Some((_, _, m)) = masque else {
                return Some((w, h, rgba, None));
            };
            if masque_de_zones(&m) {
                return Some((w, h, rgba, Some(m)));
            }
            if couleur_muette {
                for i in (0..rgba.len()).step_by(4) {
                    rgba[i + 3] = m[i];
                }
            }
            Some((w, h, rgba, None))
        })
        .collect()
}

/// Emprise d'un œil dans le carré du visage, en fraction de la texture.
///
/// **Lue sur une grille témoin**, et non devinée. Le dépliage du visage n'est pas un plan frontal :
/// ni le calage à l'œil ni le calcul depuis les sommets ne convergeaient. On pose donc sur la
/// couche du visage une grille 8 × 8 dont chaque case porte une teinte qui l'identifie — rouge =
/// colonne × 32, vert = ligne × 32 — on capture le modèle, et on lit la couleur à l'endroit des
/// yeux.
///
/// Le relevé donne `srgb(87,150,200)` à gauche et `srgb(161,132,29)` à droite, soit les cases
/// `(2,4)` et `(5,4)` : `u ∈ [0,250 ; 0,375]` et `u ∈ [0,625 ; 0,750]`, toutes deux
/// `v ∈ [0,500 ; 0,625]`. L'emprise retenue est cette case resserrée à 80 %.
///
/// La grille se rejoue en compilant avec `NIE_UV_GRID=1`.
#[cfg(feature = "textures")]
const YEUX_EMPRISE: [(f32, f32, f32, f32); 2] =
    [(0.262, 0.512, 0.366, 0.616), (0.637, 0.512, 0.741, 0.616)];

/// Dessine une couche d'yeux RGBA, transparente partout ailleurs.
///
/// ⚠️ **Cette couche est RECONSTITUÉE, pas extraite du jeu.** Les fichiers ne portent aucun tracé
/// d'œil — vingt variantes de `_facetex/01_eye` mesurées à 0,000 % d'encre — et les zones de leurs
/// masques couvrent 4,8 % et 15,6 % de la surface là où le visage du jeu n'en montre que 1,530 % :
/// aucune composition ne peut en tirer un œil. Le dessin est donc produit ici, à la demande
/// explicite de l'auteur du projet, pour que l'avatar soit complet.
///
/// Ce qui vient du jeu : l'**emprise** de chaque œil, relevée sur `base_elderlywoman/face_10`, et
/// la **couleur d'iris**, relevée sur l'écran de l'éditeur. Ce qui est reconstitué : la forme du
/// globe, du contour, de la pupille et du reflet.
#[cfg(feature = "textures")]
#[must_use]
pub fn dessiner_yeux(largeur: u32, hauteur: u32, iris: [u8; 3]) -> alloc::vec::Vec<u8> {
    let (lw, lh) = (largeur as usize, hauteur as usize);
    let mut out = alloc::vec![0u8; lw * lh * 4];
    // GRILLE TÉMOIN — 8 × 8 cases de teintes distinctes, pour lire à l'écran quelle case du
    // dépliage tombe sur les yeux. Le calage à l'œil ne converge pas : le dépliage du visage n'est
    // pas un plan frontal.
    if core::option_env!("NIE_UV_GRID").is_some() {
        for y in 0..lh {
            for x in 0..lw {
                let (cx, cy) = (x * 8 / lw, y * 8 / lh);
                let i = (y * lw + x) * 4;
                out[i] = (cx * 32) as u8;
                out[i + 1] = (cy * 32) as u8;
                out[i + 2] = if (cx + cy) % 2 == 0 { 220 } else { 60 };
                out[i + 3] = 255;
            }
        }
        return out;
    }
    for (x0, y0, x1, y1) in YEUX_EMPRISE {
        let (px0, py0) = (
            (x0 * largeur as f32) as usize,
            (y0 * hauteur as f32) as usize,
        );
        let (px1, py1) = (
            (x1 * largeur as f32) as usize,
            (y1 * hauteur as f32) as usize,
        );
        if px1 <= px0 || py1 <= py0 {
            continue;
        }
        let (cx, cy) = ((px0 + px1) as f32 / 2.0, (py0 + py1) as f32 / 2.0);
        let (rx, ry) = ((px1 - px0) as f32 / 2.0, (py1 - py0) as f32 / 2.0);
        for y in py0..py1.min(lh) {
            for x in px0..px1.min(lw) {
                let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
                let d = dx * dx + dy * dy;
                if d > 1.0 {
                    continue;
                }
                // Rayons relatifs : globe, iris, pupille, et l'épaisseur du trait de paupière.
                // Le trait sombre cerne l'œil et s'épaissit en paupière vers le haut.
                let (couleur, alpha) = if d > 0.86 || dy < -0.62 {
                    ([28u8, 24, 26], 255u8)
                } else {
                    let di = (dx * 1.32).powi(2) + (dy * 0.98).powi(2);
                    if di < 0.10 {
                        ([18, 15, 17], 255) // la pupille
                    } else if di < 0.44 {
                        // l'iris, éclairci vers le bas comme le fait le jeu
                        let k = 1.0 + 0.35 * (dy + 0.3).max(0.0);
                        (
                            [
                                (f32::from(iris[0]) * k).min(255.0) as u8,
                                (f32::from(iris[1]) * k).min(255.0) as u8,
                                (f32::from(iris[2]) * k).min(255.0) as u8,
                            ],
                            255,
                        )
                    } else {
                        ([250, 249, 250], 255) // le blanc de l'œil
                    }
                };
                let i = (y * lw + x) * 4;
                out[i] = couleur[0];
                out[i + 1] = couleur[1];
                out[i + 2] = couleur[2];
                out[i + 3] = alpha;
            }
        }
        // Le reflet : un disque clair en haut à gauche de l'iris.
        let (sx, sy) = (cx - rx * 0.30, cy - ry * 0.30);
        let sr = rx * 0.17;
        for y in (sy - sr) as usize..((sy + sr) as usize).min(lh) {
            for x in (sx - sr) as usize..((sx + sr) as usize).min(lw) {
                let (dx, dy) = (x as f32 - sx, y as f32 - sy);
                if dx * dx + dy * dy <= sr * sr {
                    let i = (y * lw + x) * 4;
                    out[i] = 255;
                    out[i + 1] = 255;
                    out[i + 2] = 255;
                    out[i + 3] = 255;
                }
            }
        }
    }
    out
}

/// Dessine UN œil qui remplit toute l'image, en UV `0..1`.
///
/// ⚠️ **Reconstitution assumée**, cf. [`dessiner_yeux`] : les fichiers ne portent aucun tracé d'œil.
/// Cette variante-ci est faite pour être posée sur un quad placé en 3D, ce qui affranchit du
/// dépliage du visage — celui-ci n'étant pas un plan frontal, aucun calage dans sa texture n'a
/// abouti.
///
/// Le tracé suit ce que montre l'écran du jeu : un globe clair cerné d'un trait sombre qui
/// s'épaissit en paupière, un iris de la couleur choisie, une pupille et un reflet.
#[cfg(feature = "textures")]
#[must_use]
pub fn dessiner_oeil(cote: u32, iris: [u8; 3]) -> alloc::vec::Vec<u8> {
    let n = cote as usize;
    let mut out = alloc::vec![0u8; n * n * 4];
    let c = cote as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            // Ellipse : l'œil est plus large que haut.
            let dx = (x as f32 - c) / (c * 0.96);
            let dy = (y as f32 - c) / (c * 0.62);
            let d = dx * dx + dy * dy;
            if d > 1.0 {
                continue;
            }
            let (couleur, _) = if d > 0.80 || dy < -0.55 {
                ([26u8, 22, 24], 255u8) // trait de contour et paupière
            } else {
                let di = (dx * 1.55).powi(2) + (dy * 0.92).powi(2);
                if di < 0.09 {
                    ([16, 13, 15], 255) // pupille
                } else if di < 0.42 {
                    let k = 1.0 + 0.30 * (dy + 0.25).max(0.0);
                    (
                        [
                            (f32::from(iris[0]) * k).min(255.0) as u8,
                            (f32::from(iris[1]) * k).min(255.0) as u8,
                            (f32::from(iris[2]) * k).min(255.0) as u8,
                        ],
                        255,
                    )
                } else {
                    ([252, 250, 251], 255) // blanc de l'œil
                }
            };
            let i = (y * n + x) * 4;
            out[i] = couleur[0];
            out[i + 1] = couleur[1];
            out[i + 2] = couleur[2];
            out[i + 3] = 255;
        }
    }
    // Reflet : un disque clair en haut à gauche de l'iris.
    let (sx, sy, sr) = (c - c * 0.26, c - c * 0.20, c * 0.15);
    for y in (sy - sr).max(0.0) as usize..((sy + sr) as usize).min(n) {
        for x in (sx - sr).max(0.0) as usize..((sx + sr) as usize).min(n) {
            let (dx, dy) = (x as f32 - sx, y as f32 - sy);
            if dx * dx + dy * dy <= sr * sr {
                let i = (y * n + x) * 4;
                out[i] = 255;
                out[i + 1] = 255;
                out[i + 2] = 255;
                out[i + 3] = 255;
            }
        }
    }
    out
}

/// Vrai si cette planche porte un TRAIT dessiné, et non une simple teinte claire.
///
/// La distinction sépare deux familles de `_facetex` qui se ressemblent par leur histogramme mais
/// pas par leur rôle. `mouth_01` peint quatre bouches au trait noir : elle descend jusqu'à 0.
/// `eye_L_01` est blanche avec de pâles ovales gris — elle ne dessine rien, elle marque une zone,
/// et c'est son masque qui porte la forme.
///
/// Les traiter pareil rendait le visage entièrement blanc : la planche d'œil, conservée telle
/// quelle et rendue opaque par son masque, recouvrait la peau.
///
/// Le critère est la présence d'une **encre** : au moins un demi pour cent de pixels franchement
/// sombres. Un liseré gris clair n'en produit aucun.
///
/// Seuils et classification viennent de [`crate::planche`], qui les emploie aussi pour mesurer le
/// corpus : les tenir en double faisait diverger ce que l'analyse constate de ce que la
/// composition applique.
#[cfg(feature = "textures")]
#[must_use]
pub fn porte_un_trait(rgba: &[u8]) -> bool {
    crate::planche::part_encre_brute(rgba) > crate::planche::PART_ENCRE_TRACE
}

/// Découpe une planche DESSINÉE par son masque de zones, sans la teindre.
///
/// Certaines planches de `_facetex` portent le trait dans leur couleur : `mouth_01` montre quatre
/// bouches complètes, contour noir, lèvres et dents, et son masque les cerne en noir sur fond
/// rouge. Les faire passer par la teinte par canaux détruisait ce dessin — le contour noir n'a
/// aucun canal dominant, donc devenait transparent, et les lèvres tombaient sur le canal du fond.
///
/// Ici la couleur est conservée telle quelle, et le masque ne sert qu'à découper : tout ce que le
/// fond rouge recouvre disparaît, le reste est opaque. C'est le geste minimal, et le seul qui
/// respecte un dessin déjà peint.
#[cfg(feature = "textures")]
#[must_use]
pub fn decouper_par_zones(
    largeur: u32,
    hauteur: u32,
    planche: &[u8],
    masque: &[u8],
) -> Option<alloc::vec::Vec<u8>> {
    let attendu = largeur as usize * hauteur as usize * 4;
    if attendu == 0 || planche.len() < attendu || masque.len() < attendu {
        return None;
    }
    let mut sortie = planche[..attendu].to_vec();
    for i in (0..attendu).step_by(4) {
        let (r, v, b) = (masque[i], masque[i + 1], masque[i + 2]);
        let fond = r > 160 && v < 96 && b < 96;
        if fond {
            sortie[i + 3] = 0;
        }
    }
    Some(sortie)
}

/// Découpe une planche d'**œil** (`01_eye`) selon sa convention de masque, qui est l'INVERSE de
/// celle de la bouche.
///
/// Mesuré le 2026-08-31 sur `01_eye/eye_01.g4tx`, sous-planche `eye_L_01msk` (2048×1024) — les
/// trois zones sont portées par le MASQUE, la planche `eye_L_01` n'en porte aucune (0,00 % sur
/// les trois critères) :
///
/// | zone du masque | part | rôle |
/// |---|---|---|
/// | rouge (`r > 200`, `v < 50`) | 79,38 % | zone morte |
/// | noir (`r < 10`, `v < 10`) | 15,52 % | **l'ouverture de l'œil** — doit rester un trou |
/// | vert (`v > 128`, `r < 128`) | 4,89 % | **le trait de paupière et de cils** — le tracé |
///
/// [`decouper_par_zones`] applique la convention de `05_mouth` (noir = encre, rouge = fond) : elle
/// ne retire que le rouge, garde le noir, et laisse la teinte par canaux peindre l'ouverture en
/// carnation. Résultat mesuré sur la sortie : l'ouverture opaque à 99,59 %, le liseré à 22,37 %,
/// soit une rondelle de peau posée SUR l'œil et le tracé jeté (IoU de l'alpha avec le vert :
/// 6,99 %). D'où un visage lisse, sans yeux.
///
/// Ici, seul le vert est conservé ; le rouge et le noir deviennent transparents, ce qui laisse
/// voir la maille des yeux placée dessous.
///
/// **L'opacité est POSÉE, pas héritée.** La planche `eye_L_01` ne porte pas le tracé : mesurée sur
/// la zone verte du masque, elle y est grise et quasi transparente — couleur moyenne (211, 211,
/// 211), alpha moyen 1,1 sur 25 626 pixels. Conserver son alpha rendait donc la planche entière
/// invisible. La couleur, elle, vient de la teinte par canaux appliquée en amont : sur une planche
/// `_facetex`, le vert porte l'iris.
///
/// `planche` est donc attendue **déjà teintée** ; cette fonction ne décide que de l'alpha.
///
/// Rend `None` si les tampons sont trop courts pour la taille annoncée.
#[cfg(feature = "textures")]
#[must_use]
pub fn decouper_oeil(
    largeur: u32,
    hauteur: u32,
    planche: &[u8],
    masque: &[u8],
) -> Option<alloc::vec::Vec<u8>> {
    let attendu = largeur as usize * hauteur as usize * 4;
    if attendu == 0 || planche.len() < attendu || masque.len() < attendu {
        return None;
    }
    let mut sortie = planche[..attendu].to_vec();
    for i in (0..attendu).step_by(4) {
        let (r, v) = (masque[i], masque[i + 1]);
        // Le tracé est ce que le vert domine. Le seuil sur `v` écarte les pixels sombres, où les
        // trois canaux sont bas et où aucune dominance n'a de sens.
        let trace = v > 128 && v > r;
        sortie[i + 3] = if trace { 255 } else { 0 };
    }
    Some(sortie)
}

/// Vrai si le canal rouge d'une image RGBA est constant — donc sans information spatiale.
///
/// Sert à décider si un masque `msk` mérite d'être posé en alpha : uniforme, il n'apporte rien et
/// l'appliquer efface les variations de la planche.
#[cfg(feature = "textures")]
#[must_use]
pub fn canal_uniforme(rgba: &[u8]) -> bool {
    crate::planche::canal_uniforme(rgba)
}

/// Redimensionne une image RGBA vers une taille donnée, au plus proche voisin.
///
/// Sert à ramener les couches d'un visage à la toile commune. Le plus proche voisin suffit ici :
/// les planches partagent le dépliage, l'écart de définition n'est qu'un facteur entier ou proche.
#[cfg(feature = "textures")]
fn redimensionner_rgba(rgba: &[u8], w: u32, h: u32, vers_w: u32, vers_h: u32) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || vers_w == 0 || vers_h == 0 {
        return None;
    }
    if rgba.len() < (w as usize) * (h as usize) * 4 {
        return None;
    }
    let mut out = vec![0u8; (vers_w as usize) * (vers_h as usize) * 4];
    for y in 0..vers_h {
        let sy = (u64::from(y) * u64::from(h) / u64::from(vers_h)).min(u64::from(h) - 1) as usize;
        for x in 0..vers_w {
            let sx =
                (u64::from(x) * u64::from(w) / u64::from(vers_w)).min(u64::from(w) - 1) as usize;
            let src = (sy * w as usize + sx) * 4;
            let dst = (y as usize * vers_w as usize + x as usize) * 4;
            out[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
        }
    }
    Some(out)
}

/// Une couleur de teinte d'une pièce de visage : RGB, plus un alpha qui dit si le canal est actif.
#[cfg(feature = "textures")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeinteCanal {
    /// Composantes de la teinte.
    pub rgb: [u8; 3],
    /// 0 = ce canal ne participe pas à la composition.
    pub actif: bool,
}

/// Applique la teinte d'une planche de visage, **canal par canal**.
///
/// C'est la règle réelle de composition du visage, et elle n'a rien d'un empilement alpha : les
/// planches de `_facetex` sont des **masques à trois canaux**. Les recettes de l'éditeur
/// (`common/chr/_test/default/mdl_edit_avatar*.cfg.bin`) donnent, pour chaque pièce de texture,
/// trois `CHARA_EDIT_PARAM_TEX_PARTS_COLOR` dont les identifiants sont les CRC-32 de `red`,
/// `green` et `blue` — résolus depuis les chaînes du binaire. Chaque canal de la planche désigne
/// donc une zone, et chaque zone reçoit sa propre couleur ; l'alpha de la couleur dit si le canal
/// participe.
///
/// C'est ce qui explique que les planches paraissent « opaques » et que les composer en alpha ne
/// donnait rien : leur canal alpha n'a jamais été le véhicule de l'information.
///
/// Le canal dominant sélectionne la teinte ; il n'y a pas d'addition. `est_fond` distingue la
/// planche de base des planches posées par-dessus : sur ces dernières, une zone de canal rouge
/// — le fond, la carnation — devient transparente au lieu d'effacer ce qui est en dessous.
/// Écart minimum, sur 255, pour qu'un canal soit tenu pour dominant.
///
/// En deçà, l'avance est du bruit de quantification et non une désignation de zone.
#[cfg(feature = "textures")]
const MARGE_DOMINANCE: u32 = 8;

#[cfg(feature = "textures")]
#[must_use]
pub fn teinter_par_canaux(
    largeur: u32,
    hauteur: u32,
    planche: &[u8],
    teintes: [TeinteCanal; 3],
    est_fond: bool,
) -> Option<Vec<u8>> {
    let attendu = largeur as usize * hauteur as usize * 4;
    if attendu == 0 || planche.len() < attendu {
        return None;
    }
    let opaque_partout = couche_totalement_opaque(&planche[..attendu]);
    let mut sortie = vec![0u8; attendu];
    for i in (0..attendu).step_by(4) {
        // Le canal DOMINANT désigne la zone, et c'est sa couleur qui s'applique — un masque
        // sélectionne, il n'additionne pas. Additionner saturait systématiquement : la teinte par
        // défaut du canal bleu est blanche, et blanc + n'importe quoi = blanc.
        //
        // À égalité, l'ordre rouge > vert > bleu tranche. C'est le cas d'une planche neutre comme
        // `face_00`, blanche partout (R = G = B = 255) : elle prend donc la teinte du canal rouge,
        // celle que les recettes réservent à la carnation (`#F3CAC1` dans `mdl_edit_avatar01`).
        let mut choisi: Option<(u32, &TeinteCanal, usize)> = None;
        let mut second = 0u32;
        for (canal, teinte) in teintes.iter().enumerate() {
            if !teinte.actif {
                continue;
            }
            let poids = u32::from(planche[i + canal]);
            if poids == 0 {
                continue;
            }
            if choisi.is_none_or(|(p, _, _)| poids > p) {
                if let Some((p, _, _)) = choisi {
                    second = second.max(p);
                }
                choisi = Some((poids, teinte, canal));
            } else {
                second = second.max(poids);
            }
        }
        // Une dominance d'une unité ne désigne rien. La planche `eye_L_01` est blanche à 255 sur
        // les trois canaux, sauf des ovales à peine plus gris où un canal passe devant d'un ou
        // deux crans : sans marge, ces ovales basculaient d'un bloc sur la couleur de l'iris et
        // sortaient en blobs opaques par-dessus les yeux. En deçà de la marge, la zone est traitée
        // comme du fond — le canal rouge — ce qu'elle est : une planche presque neutre ne dit rien
        // d'autre que « rien à ajouter ici ».
        if let Some((poids, _, canal)) = choisi
            && canal != 0
            && poids.saturating_sub(second) < MARGE_DOMINANCE
            && teintes[0].actif
            && planche[i] > 0
        {
            choisi = Some((u32::from(planche[i]), &teintes[0], 0));
        }
        match choisi {
            Some((poids, teinte, canal)) => {
                for (c, composante) in teinte.rgb.iter().enumerate() {
                    sortie[i + c] = (poids * u32::from(*composante) / 255).min(255) as u8;
                }
                // Le canal ROUGE est le fond — la carnation. Une planche neutre l'a partout :
                // `face_00`, `eye_00`, `highlight_00` sont uniformément rouges, c'est ainsi que
                // le jeu dit « rien à ajouter ici ». Posée sur une autre, une telle zone doit
                // donc laisser voir ce qui est dessous au lieu de l'effacer ; seule la planche de
                // fond garde son opacité. C'est ce qui rendait quatre familles sur six inertes.
                // La règle du fond ne s'applique qu'aux planches OPAQUES. Une planche dont
                // l'information vit dans son alpha — parce que sa couleur était muette et que son
                // masque a été posé, cas des reflets — porte déjà sa propre découpe : la forcer
                // transparente sur le canal rouge l'effacerait entièrement.
                let porte_son_alpha = !opaque_partout;
                sortie[i + 3] = if canal == 0 && !est_fond && !porte_son_alpha {
                    0
                } else {
                    planche[i + 3]
                };
            }
            // Aucun canal actif ici : le pixel n'appartient à aucune zone.
            None => sortie[i + 3] = 0,
        }
    }
    Some(sortie)
}

/// Vrai si cette couche RGBA est opaque partout — auquel cas, composée par-dessus, elle masque
/// tout ce qui précède.
///
/// Les planches de `_facetex` sont dans ce cas presque toutes (`face_00`, `eye_00`, `mouth_00`,
/// `highlight_00`), ce qui rend la composition alpha inopérante entre elles : seule la dernière
/// survit. C'est ainsi que plusieurs familles de traits restaient sans effet. L'appelant s'en
/// sert pour le SIGNALER plutôt que de perdre des couches en silence.
#[cfg(feature = "textures")]
#[must_use]
pub fn couche_totalement_opaque(rgba: &[u8]) -> bool {
    !rgba.is_empty() && rgba.iter().skip(3).step_by(4).all(|&a| a == 255)
}

/// Compose des couches RGBA par-dessus la première, en mélange alpha classique.
///
/// C'est ainsi que le visage d'un avatar se fabrique : le jeu ne stocke pas une texture par
/// combinaison, il empile des planches qui partagent le même dépliage UV — la peau
/// (`_facetex/00_face/face_NN`), puis les yeux, les pupilles, les reflets, les sourcils et la
/// bouche. Chaque rubrique de l'éditeur choisit **une** planche de sa famille ; le résultat est
/// cette composition, et c'est elle qui change quand le joueur change de choix.
///
/// La taille retenue est celle de la **plus grande** couche, et non celle de la première.
///
/// Seules les couches de **même rapport largeur/hauteur** sont composées : un rapport différent
/// est un autre dépliage UV, et les superposer placerait les traits n'importe où sur le visage.
/// Une couche au bon rapport mais plus petite est agrandie.
///
/// **L'appelant doit donc grouper les couches par dépliage avant d'appeler cette fonction.** Les
/// planches de `_facetex` en ont deux — la peau, les pupilles et les reflets sont en 512×512, les
/// yeux, les sourcils et la bouche en 2048×1024 — et tout mélanger revenait à en perdre la moitié
/// en silence, dont les pupilles, la seule à porter un dessin. Chaque dépliage donne une texture,
/// et chaque matériau de la tête en reçoit une.
///
/// Les planches de `_facetex` étant OPAQUES, leur alpha doit avoir été posé au préalable depuis
/// leur masque compagnon — cf. [`decoder_planches_masquees`]. Sans cela, la composition ne rend
/// que la dernière couche.
///
/// Rend `None` si la liste est vide ou si aucune couche n'est exploitable.
#[cfg(feature = "textures")]
#[must_use]
pub fn composer_couches(couches: &[(u32, u32, Vec<u8>)]) -> Option<(u32, u32, Vec<u8>)> {
    // La plus grande couche donne la toile ; son rapport donne le dépliage de référence.
    let (largeur, hauteur, _) = *couches
        .iter()
        .filter(|(w, h, d)| *w > 0 && *h > 0 && d.len() >= (*w as usize) * (*h as usize) * 4)
        .max_by_key(|(w, h, _)| u64::from(*w) * u64::from(*h))?;
    let attendu = largeur as usize * hauteur as usize * 4;
    let rapport = f64::from(largeur) / f64::from(hauteur);
    let mut sortie = vec![0u8; attendu];

    for (rang, (lw, lh, couche)) in couches.iter().enumerate() {
        if *lw == 0 || *lh == 0 || couche.len() < (*lw as usize) * (*lh as usize) * 4 {
            continue;
        }
        let _ = rang;
        // Un autre rapport = un autre dépliage UV : on ne le plaque pas sur ce visage.
        if (f64::from(*lw) / f64::from(*lh) - rapport).abs() > 0.01 {
            continue;
        }
        let ajustee = if (*lw, *lh) == (largeur, hauteur) {
            couche.clone()
        } else {
            match redimensionner_rgba(couche, *lw, *lh, largeur, hauteur) {
                Some(r) => r,
                None => continue,
            }
        };
        if ajustee.len() < attendu {
            continue;
        }
        for i in (0..attendu).step_by(4) {
            let a = f32::from(ajustee[i + 3]) / 255.0;
            if a <= 0.0 {
                continue;
            }
            for c in 0..3 {
                let dessus = f32::from(ajustee[i + c]);
                let dessous = f32::from(sortie[i + c]);
                sortie[i + c] = (dessus * a + dessous * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
            }
            let a_dessous = f32::from(sortie[i + 3]) / 255.0;
            sortie[i + 3] = ((a + a_dessous * (1.0 - a)) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
    Some((largeur, hauteur, sortie))
}

/// Réduit une image RGBA8 pour que son plus grand côté n'excède pas `max_cote`, par **moyenne de
/// boîte** (chaque pixel de sortie est la moyenne des pixels source qu'il recouvre).
///
/// Rend l'image telle quelle si elle tient déjà dans la boîte : une vignette ne doit jamais
/// agrandir, ni recompresser pour rien.
///
/// Le filtre est une moyenne, pas un échantillonnage au plus proche : sur les atlas d'icônes du
/// jeu (traits d'un pixel sur fond transparent), le plus proche fait disparaître les traits alors
/// que la moyenne les garde. La moyenne est pondérée par l'alpha prémultiplié — sans ça, les
/// pixels transparents (dont le RGB est arbitraire dans une texture détourée) tirent la couleur
/// des bords vers du noir, et la vignette d'une icône se retrouve cernée.
///
/// # Erreurs
///
/// Rend un message si `rgba` ne fait pas `largeur × hauteur × 4` octets, si une dimension est
/// nulle, ou si `max_cote` est nul.
pub fn reduire_rgba(
    rgba: &[u8],
    largeur: u32,
    hauteur: u32,
    max_cote: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    let attendu = (largeur as usize)
        .checked_mul(hauteur as usize)
        .and_then(|p| p.checked_mul(4))
        .ok_or_else(|| "dimensions hors bornes".to_string())?;
    if rgba.len() != attendu {
        return Err(alloc::format!(
            "tampon de {} octets pour {largeur}×{hauteur} RGBA (attendu {attendu})",
            rgba.len()
        ));
    }
    if largeur == 0 || hauteur == 0 {
        return Err("image de dimension nulle".to_string());
    }
    if max_cote == 0 {
        return Err("côté maximal nul".to_string());
    }

    let cote = largeur.max(hauteur);
    if cote <= max_cote {
        return Ok((largeur, hauteur, rgba.to_vec()));
    }

    // Dimensions cibles en conservant le rapport, au moins 1 pixel : une bande de 2048×8 réduite
    // à 128 donnerait 0 en hauteur par simple division.
    let nw = ((largeur as u64 * max_cote as u64) / cote as u64).max(1) as u32;
    let nh = ((hauteur as u64 * max_cote as u64) / cote as u64).max(1) as u32;

    let mut out = Vec::with_capacity((nw as usize) * (nh as usize) * 4);
    for y in 0..nh {
        // Bornes de la boîte source, en arithmétique entière : pas de flottant, donc le résultat
        // ne dépend pas de la plateforme.
        let y0 = ((y as u64 * hauteur as u64) / nh as u64) as usize;
        let y1 = (((y as u64 + 1) * hauteur as u64) / nh as u64).max(y0 as u64 + 1) as usize;
        for x in 0..nw {
            let x0 = ((x as u64 * largeur as u64) / nw as u64) as usize;
            let x1 = (((x as u64 + 1) * largeur as u64) / nw as u64).max(x0 as u64 + 1) as usize;

            let (mut r, mut g, mut b, mut a) = (0u64, 0u64, 0u64, 0u64);
            let mut n = 0u64;
            for sy in y0..y1.min(hauteur as usize) {
                let ligne = sy * largeur as usize;
                for sx in x0..x1.min(largeur as usize) {
                    let p = (ligne + sx) * 4;
                    let alpha = u64::from(rgba[p + 3]);
                    // Prémultiplication : la couleur d'un pixel transparent ne doit pas peser.
                    r += u64::from(rgba[p]) * alpha;
                    g += u64::from(rgba[p + 1]) * alpha;
                    b += u64::from(rgba[p + 2]) * alpha;
                    a += alpha;
                    n += 1;
                }
            }
            if n == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            // Démultiplication : `a` est la somme des alphas, donc diviser par elle rend la
            // couleur moyenne *visible*. `a == 0` (boîte entièrement transparente) n'a pas de
            // couleur moyenne — et diviser par elle serait une division par zéro.
            match (r.checked_div(a), g.checked_div(a), b.checked_div(a)) {
                (Some(r), Some(g), Some(b)) => {
                    out.push(r as u8);
                    out.push(g as u8);
                    out.push(b as u8);
                    out.push((a / n) as u8);
                }
                _ => out.extend_from_slice(&[0, 0, 0, 0]),
            }
        }
    }
    Ok((nw, nh, out))
}

/// Décode un `.g4tx` et l'encode en vignette : plus grand côté borné à `max_cote`, format libre.
///
/// C'est le chemin des grilles de fichiers (explorateur, navigateur de contenu de l'éditeur) :
/// une texture de personnage décode en 2048×2048 RGBA (16 Mio en mémoire, plusieurs centaines de
/// kio en PNG), or la vignette affichée fait moins de 100 pixels. Servir la pleine résolution à
/// une grille de plusieurs milliers d'entrées sature la mémoire du client bien avant l'écran.
///
/// `basename` : même rôle que dans [`g4tx_vers`] (départage les conteneurs multi-textures).
///
/// # Erreurs
///
/// Rend un message si le G4TX n'est pas décodable, si la réduction échoue ou si l'encodage échoue.
#[cfg(feature = "textures")]
pub fn g4tx_vignette(
    g4tx: &[u8],
    basename: &str,
    max_cote: u32,
    format: ImageOut,
) -> Result<Vec<u8>, String> {
    let (w, h, rgba) = crate::g4tx_decode::decode_best_to_rgba(g4tx, basename)
        .ok_or_else(|| "G4TX non décodable".to_string())?;
    let (vw, vh, petit) = reduire_rgba(&rgba, w, h, max_cote)?;
    encoder_rgba(&petit, vw, vh, format)
}

/// Vignette d'une texture **nommée** d'un conteneur G4TX (cf.
/// [`crate::g4tx_decode::decode_named_to_rgba`]).
///
/// [`g4tx_vignette`] rend UNE image par fichier : la texture principale. Elle ne peut donc pas
/// servir une grille des 80 icônes que porte `icon_item05.g4tx`, où chaque nom a son propre
/// payload. `nom` désigne soit une texture principale, soit une région d'atlas — la sélection est
/// la même que celle du décodage nommé, la réduction se fait ici avant l'IPC.
pub fn g4tx_vignette_nommee(
    g4tx: &[u8],
    nom: &str,
    max_cote: u32,
    format: ImageOut,
) -> Result<Vec<u8>, String> {
    let (w, h, rgba) = crate::g4tx_decode::decode_named_to_rgba(g4tx, nom)
        .ok_or_else(|| format!("texture `{nom}` absente du conteneur G4TX"))?;
    let (vw, vh, petit) = reduire_rgba(&rgba, w, h, max_cote)?;
    encoder_rgba(&petit, vw, vh, format)
}

/// Décode une planche nommée, la **multiplie** par une couleur, et lui applique son masque.
///
/// Certaines planches de l'éditeur ne portent aucune couleur : `hair_10`, la chevelure de
/// `hairF001M`, fait 64 × 32 et vaut 255,255,255 sur tous ses pixels. Elle n'est pas ratée — elle
/// est **neutre**, et c'est la couleur choisie par le joueur qui la colore à l'exécution. Posée
/// telle quelle, elle donnait un casque blanc sur la tête de l'avatar.
///
/// La multiplication est le bon opérateur ici, et pas la sélection par canal dominant employée
/// pour le visage : cette dernière suppose un masque à trois canaux, alors qu'une planche neutre
/// n'a pas de canal dominant. Multiplier préserve en revanche les nuances de la planche quand
/// elle en a — une mèche plus sombre le reste après teinture.
///
/// Le conteneur range à côté un masque `<nom>msk` de même définition. Quand il existe et qu'il
/// varie, son canal rouge devient l'alpha : c'est lui qui découpe les mèches, que la géométrie à
/// 227 sommets ne peut pas porter. Un masque uniforme est ignoré — il ne découpe rien.
#[cfg(feature = "textures")]
pub fn g4tx_vignette_teintee(
    g4tx: &[u8],
    nom: &str,
    max_cote: u32,
    format: ImageOut,
    rgb: [u8; 3],
) -> Result<alloc::vec::Vec<u8>, alloc::string::String> {
    use alloc::format;
    let (w, h, mut rgba) = crate::g4tx_decode::decode_named_to_rgba(g4tx, nom)
        .ok_or_else(|| format!("texture `{nom}` absente du conteneur G4TX"))?;

    for px in rgba.chunks_exact_mut(4) {
        for (c, teinte) in rgb.iter().enumerate() {
            px[c] = ((u16::from(px[c]) * u16::from(*teinte)) / 255) as u8;
        }
    }

    let masque = crate::g4tx_decode::decode_named_to_rgba(g4tx, &format!("{nom}msk"));
    if let Some((mw, mh, m)) = masque
        && mw == w
        && mh == h
        && !canal_uniforme(&m)
    {
        for (px, mp) in rgba.chunks_exact_mut(4).zip(m.chunks_exact(4)) {
            px[3] = px[3].min(mp[0]);
        }
    }

    let (vw, vh, petit) = reduire_rgba(&rgba, w, h, max_cote)?;
    encoder_rgba(&petit, vw, vh, format)
}

// ─────────────────────────────────────────────────────────────────────────────
// Planches (sprite sheets)
// ─────────────────────────────────────────────────────────────────────────────
//
// Assembler plusieurs images en une planche est une opération que le dépôt refaisait à la
// main à chaque besoin, en script jetable. Le nécessaire existait pourtant déjà à côté :
// `decoder_planches` décode les textures d'un `.g4tx`, `decouper_par_zones` en extrait des
// morceaux, et `nie-game --compose-layout` compose un écran — mais rien ne savait poser N
// images côte à côte ET dire où elles ont atterri.
//
// Ce « où » est ce qui distingue une planche d'une simple image : sans les rectangles, la
// sortie se regarde mais ne se réutilise pas.

/// Une image à poser dans une planche.
pub struct CasePlanche {
    /// Nom de la case, repris tel quel dans le manifeste.
    pub nom: String,
    /// Largeur en pixels.
    pub largeur: u32,
    /// Hauteur en pixels.
    pub hauteur: u32,
    /// Pixels RGBA8, `largeur * hauteur * 4` octets.
    pub rgba: Vec<u8>,
}

/// Où une case a atterri dans la planche composée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RectPlanche {
    /// Nom de la case.
    pub nom: String,
    /// Abscisse du coin supérieur gauche.
    pub x: u32,
    /// Ordonnée du coin supérieur gauche.
    pub y: u32,
    /// Largeur réelle de l'image posée — **jamais** celle de la cellule.
    pub w: u32,
    /// Hauteur réelle de l'image posée.
    pub h: u32,
}

/// Une planche composée et son manifeste.
pub struct Planche {
    /// Largeur totale.
    pub largeur: u32,
    /// Hauteur totale.
    pub hauteur: u32,
    /// Pixels RGBA8 de la planche.
    pub rgba: Vec<u8>,
    /// Position de chaque case, dans l'ordre d'entrée.
    pub cases: Vec<RectPlanche>,
}

/// Compose des images en planche, sur une grille de `colonnes`.
///
/// **Aucune image n'est jamais redimensionnée.** Les cellules prennent la taille de la plus
/// grande case, et les plus petites sont centrées dedans. Étirer pour remplir donnerait une
/// planche visuellement régulière au prix de portraits déformés — et le manifeste mentirait,
/// puisqu'il décrirait des rectangles qui ne correspondent plus aux pixels d'origine.
///
/// Les rectangles rendus sont ceux des **images**, pas des cellules : c'est ce qu'un
/// consommateur veut découper.
///
/// Rend une planche vide (0 × 0) si `cases` est vide ou si `colonnes` vaut 0.
#[must_use]
pub fn composer_planche(
    cases: &[CasePlanche],
    colonnes: u32,
    marge: u32,
    gouttiere: u32,
    fond: [u8; 4],
) -> Planche {
    if cases.is_empty() || colonnes == 0 {
        return Planche {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
            cases: Vec::new(),
        };
    }

    let cell_w = cases.iter().map(|c| c.largeur).max().unwrap_or(0);
    let cell_h = cases.iter().map(|c| c.hauteur).max().unwrap_or(0);
    let lignes = (cases.len() as u32).div_ceil(colonnes);

    let largeur = marge * 2 + cell_w * colonnes + gouttiere * colonnes.saturating_sub(1);
    let hauteur = marge * 2 + cell_h * lignes + gouttiere * lignes.saturating_sub(1);

    let mut rgba = Vec::with_capacity((largeur as usize) * (hauteur as usize) * 4);
    for _ in 0..(largeur as usize) * (hauteur as usize) {
        rgba.extend_from_slice(&fond);
    }

    let mut rects = Vec::with_capacity(cases.len());
    for (i, case) in cases.iter().enumerate() {
        let col = (i as u32) % colonnes;
        let ligne = (i as u32) / colonnes;
        let cellule_x = marge + col * (cell_w + gouttiere);
        let cellule_y = marge + ligne * (cell_h + gouttiere);
        // Centrage dans la cellule : une case plus petite ne se colle pas au coin.
        let x = cellule_x + (cell_w.saturating_sub(case.largeur)) / 2;
        let y = cellule_y + (cell_h.saturating_sub(case.hauteur)) / 2;

        poser_rgba(
            &mut rgba,
            (largeur, hauteur),
            &case.rgba,
            (case.largeur, case.hauteur),
            (x, y),
        );
        rects.push(RectPlanche {
            nom: case.nom.clone(),
            x,
            y,
            w: case.largeur,
            h: case.hauteur,
        });
    }

    Planche {
        largeur,
        hauteur,
        rgba,
        cases: rects,
    }
}

/// Recopie une image RGBA dans une autre, **pixel pour pixel, sans compositing**.
///
/// Deux raisons de ne pas mélanger avec le fond, et la seconde est la plus importante :
///
/// 1. Les cases d'une planche ne se chevauchent jamais — il n'y a rien à composer.
/// 2. Une planche existe pour être **redécoupée** d'après son manifeste. Si l'alpha était
///    fusionné avec le fond, redécouper ne rendrait plus l'image d'origine mais une version
///    aplatie sur une couleur arbitraire : le manifeste décrirait des pixels qui ne sont plus
///    ceux de la source. Copier préserve l'alpha exact, donc la réversibilité.
///
/// C'est aussi ce qui évite d'ajouter un blend de plus au workspace : le compositing alpha est
/// la **landmine #5** (`docs/ARCHITECTURE.md`, cf. l'avertissement en tête de
/// [`crate::raster2d`]) — `nie-runtime` et `nie-game` divergent déjà, et `menu.rs` porte son
/// propre compositeur « over ». Un quatrième n'aiderait personne.
///
/// Les pixels qui sortiraient du cadre sont ignorés plutôt que repliés : un dépassement doit
/// tronquer, jamais réapparaître de l'autre côté de l'image.
fn poser_rgba(
    dest: &mut [u8],
    dest_dim: (u32, u32),
    src: &[u8],
    src_dim: (u32, u32),
    pos: (u32, u32),
) {
    let (dest_w, dest_h) = dest_dim;
    let (src_w, src_h) = src_dim;
    let (x, y) = pos;
    for ligne in 0..src_h {
        let cible_y = y + ligne;
        if cible_y >= dest_h {
            break;
        }
        for colonne in 0..src_w {
            let cible_x = x + colonne;
            if cible_x >= dest_w {
                break;
            }
            let is = ((ligne * src_w + colonne) * 4) as usize;
            let id = ((cible_y * dest_w + cible_x) * 4) as usize;
            let (Some(s), Some(d)) = (src.get(is..is + 4), dest.get_mut(id..id + 4)) else {
                continue;
            };
            d.copy_from_slice(s);
        }
    }
}

#[cfg(test)]
mod tests_planche {
    use super::*;

    /// Une case unie de `w × h`.
    fn case(nom: &str, w: u32, h: u32, couleur: [u8; 4]) -> CasePlanche {
        CasePlanche {
            nom: nom.to_string(),
            largeur: w,
            hauteur: h,
            rgba: couleur.repeat((w * h) as usize),
        }
    }

    /// Lit un pixel de la planche.
    fn pixel(p: &Planche, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * p.largeur + x) * 4) as usize;
        [p.rgba[i], p.rgba[i + 1], p.rgba[i + 2], p.rgba[i + 3]]
    }

    #[test]
    fn trois_cases_de_meme_taille_en_ligne() {
        let cases = vec![
            case("a", 10, 6, [255, 0, 0, 255]),
            case("b", 10, 6, [0, 255, 0, 255]),
            case("c", 10, 6, [0, 0, 255, 255]),
        ];
        let p = composer_planche(&cases, 3, 4, 2, [0, 0, 0, 255]);

        // 4 + 10 + 2 + 10 + 2 + 10 + 4
        assert_eq!(p.largeur, 42);
        assert_eq!(p.hauteur, 14, "4 + 6 + 4");
        assert_eq!(p.rgba.len(), (42 * 14 * 4) as usize);

        assert_eq!(
            p.cases[0],
            RectPlanche {
                nom: "a".into(),
                x: 4,
                y: 4,
                w: 10,
                h: 6
            }
        );
        assert_eq!(
            p.cases[1],
            RectPlanche {
                nom: "b".into(),
                x: 16,
                y: 4,
                w: 10,
                h: 6
            }
        );
        assert_eq!(
            p.cases[2],
            RectPlanche {
                nom: "c".into(),
                x: 28,
                y: 4,
                w: 10,
                h: 6
            }
        );

        // Les pixels sont bien là où le manifeste les annonce — sans quoi il mentirait.
        assert_eq!(pixel(&p, 4, 4), [255, 0, 0, 255]);
        assert_eq!(pixel(&p, 16, 4), [0, 255, 0, 255]);
        assert_eq!(pixel(&p, 28, 4), [0, 0, 255, 255]);
        assert_eq!(pixel(&p, 0, 0), [0, 0, 0, 255], "la marge porte le fond");
    }

    #[test]
    fn une_case_plus_petite_est_centree_jamais_etiree() {
        let cases = vec![
            case("grande", 10, 10, [255, 0, 0, 255]),
            case("petite", 4, 4, [0, 255, 0, 255]),
        ];
        let p = composer_planche(&cases, 2, 0, 0, [0, 0, 0, 0]);

        // La petite garde SA taille : le manifeste décrit l'image, pas la cellule.
        let petite = &p.cases[1];
        assert_eq!((petite.w, petite.h), (4, 4));
        // Cellule de 10 à partir de x=10 → décalage de 3 pour centrer une case de 4.
        assert_eq!((petite.x, petite.y), (13, 3));
        assert_eq!(pixel(&p, 13, 3), [0, 255, 0, 255]);
        assert_eq!(
            pixel(&p, 10, 0),
            [0, 0, 0, 0],
            "le reste de la cellule est du fond"
        );
    }

    #[test]
    fn la_grille_passe_a_la_ligne() {
        let cases: Vec<_> = (0..5)
            .map(|i| case(&format!("c{i}"), 8, 8, [1, 2, 3, 255]))
            .collect();
        let p = composer_planche(&cases, 2, 0, 0, [0, 0, 0, 255]);

        assert_eq!(p.largeur, 16);
        assert_eq!(p.hauteur, 24, "5 cases sur 2 colonnes = 3 lignes");
        assert_eq!(
            (p.cases[2].x, p.cases[2].y),
            (0, 8),
            "la troisième ouvre la ligne 2"
        );
        assert_eq!((p.cases[4].x, p.cases[4].y), (0, 16));
    }

    #[test]
    fn l_alpha_source_est_copie_tel_quel_pas_fusionne() {
        // Une planche se REDÉCOUPE d'après son manifeste : l'alpha doit survivre intact.
        // Fusionner avec le fond rendrait le redécoupage lossy — on récupérerait une image
        // aplatie sur une couleur arbitraire au lieu de la source.
        let cases = vec![case("translucide", 4, 4, [255, 255, 255, 128])];
        let p = composer_planche(&cases, 1, 0, 0, [10, 20, 30, 255]);
        assert_eq!(
            pixel(&p, 0, 0),
            [255, 255, 255, 128],
            "les pixels sources sont copiés, pas composés avec le fond"
        );
    }

    #[test]
    fn une_planche_sans_case_est_vide_et_ne_panique_pas() {
        let p = composer_planche(&[], 3, 4, 2, [0, 0, 0, 255]);
        assert_eq!((p.largeur, p.hauteur), (0, 0));
        assert!(p.cases.is_empty());

        // Zéro colonne est une demande absurde, pas une raison de paniquer.
        let une = vec![case("a", 4, 4, [1, 1, 1, 255])];
        assert_eq!(composer_planche(&une, 0, 0, 0, [0, 0, 0, 0]).largeur, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une couche RGBA unie de 2×2.
    #[cfg(feature = "textures")]
    fn couche(r: u8, v: u8, b: u8, a: u8) -> (u32, u32, Vec<u8>) {
        (2, 2, [r, v, b, a].repeat(4))
    }

    #[cfg(feature = "textures")]
    #[test]
    fn une_couche_opaque_recouvre_le_fond() {
        let out = composer_couches(&[couche(255, 0, 0, 255), couche(0, 0, 255, 255)]).unwrap();
        assert_eq!(&out.2[..4], &[0, 0, 255, 255]);
    }

    #[cfg(feature = "textures")]
    #[test]
    fn une_couche_transparente_laisse_le_fond_intact() {
        let out = composer_couches(&[couche(255, 0, 0, 255), couche(0, 0, 255, 0)]).unwrap();
        assert_eq!(&out.2[..4], &[255, 0, 0, 255]);
    }

    #[cfg(feature = "textures")]
    #[test]
    fn une_couche_a_moitie_transparente_melange_les_deux() {
        // 128/255 ≈ 0,502 : le résultat doit tomber entre les deux couleurs, pas sur l'une d'elles.
        let out = composer_couches(&[couche(0, 0, 0, 255), couche(255, 255, 255, 128)]).unwrap();
        assert!((126..=129).contains(&out.2[0]), "obtenu {}", out.2[0]);
    }

    /// Une couche RGBA unie de dimensions données.
    #[cfg(feature = "textures")]
    fn couche_wh(w: u32, h: u32, r: u8, v: u8, b: u8, a: u8) -> (u32, u32, Vec<u8>) {
        (w, h, [r, v, b, a].repeat((w * h) as usize))
    }

    #[cfg(feature = "textures")]
    #[test]
    fn la_toile_prend_la_taille_de_la_plus_grande_couche() {
        // Le cas réel : la peau fait 512×512, les traits 2048×1024. Se caler sur la PREMIÈRE
        // couche jetait silencieusement toutes les autres — c'était le défaut.
        let out = composer_couches(&[
            couche_wh(4, 2, 255, 0, 0, 255),
            couche_wh(8, 4, 0, 255, 0, 255),
        ])
        .unwrap();
        assert_eq!((out.0, out.1), (8, 4));
        assert_eq!(&out.2[..4], &[0, 255, 0, 255]);
    }

    #[cfg(feature = "textures")]
    #[test]
    fn une_couche_d_un_autre_rapport_est_ecartee() {
        // Un rapport différent est un autre dépliage UV : la plaquer placerait les traits
        // n'importe où sur le visage. C'est à l'appelant de grouper par dépliage AVANT d'appeler
        // — ne pas le faire coûtait la moitié des planches du visage, en silence.
        let out = composer_couches(&[
            couche_wh(8, 4, 255, 0, 0, 255),
            couche_wh(4, 4, 0, 255, 0, 255),
        ])
        .unwrap();
        assert_eq!((out.0, out.1), (8, 4));
        assert_eq!(
            &out.2[..4],
            &[255, 0, 0, 255],
            "la couche carrée ne doit pas être composée"
        );
    }

    /// Les six familles de `_facetex` doivent TOUTES pouvoir peser sur le visage composé.
    ///
    /// Le défaut que ce test verrouille : la peau, les pupilles et les reflets sont en 512×512
    /// tandis que les yeux, les sourcils et la bouche sont en 2048×1024. Composer les six sur une
    /// toile unique écartait les trois premières en silence — changer de peau ou de pupille ne
    /// changeait alors pas un octet du rendu. Il faut composer UN visage PAR DÉPLIAGE.
    #[cfg(all(feature = "textures", feature = "images"))]
    #[test]
    fn chaque_depliage_de_visage_est_compose_a_part() {
        use crate::vfs::Vfs;

        let mut vfs = Vfs::new();
        if vfs
            .init(crate::vfs::resolve_game_dir().join("data"))
            .is_err()
        {
            eprintln!("SKIP : VFS non initialisable");
            return;
        }
        let lire = |vfs: &Vfs, rel: &str| {
            vfs.read(&format!("data/dx11/chr/_face/20_EDIT/_facetex/{rel}.g4tx"))
                .ok()
        };

        // Une planche de chaque dépliage, et une seconde peau pour prouver que la variation passe.
        let (Some(peau_a), Some(peau_b), Some(bouche)) = (
            lire(&vfs, "00_face/face_00"),
            lire(&vfs, "00_face/face_34"),
            lire(&vfs, "05_mouth/mouth_00"),
        ) else {
            eprintln!("SKIP : planches de visage absentes");
            return;
        };

        let pa = decoder_planches(&peau_a);
        let pb = decoder_planches(&peau_b);
        let bo = decoder_planches(&bouche);
        assert!(
            !pa.is_empty() && !pb.is_empty() && !bo.is_empty(),
            "planches décodées"
        );

        // Les deux dépliages sont bien distincts : c'est la prémisse du défaut.
        assert_ne!(
            (pa[0].0, pa[0].1),
            (bo[0].0, bo[0].1),
            "la peau et la bouche doivent avoir des dépliages différents"
        );

        // Composées ensemble, la peau disparaît : la toile prend le plus grand dépliage.
        let melange_a = composer_couches(&[pa[0].clone(), bo[0].clone()]).expect("composition");
        let melange_b = composer_couches(&[pb[0].clone(), bo[0].clone()]).expect("composition");
        assert_eq!(
            melange_a.2, melange_b.2,
            "tout mélanger doit bien écraser la peau — c'est le piège que le groupement évite"
        );

        // Groupées par dépliage, les deux peaux restent distinctes.
        let seule_a = composer_couches(&[pa[0].clone()]).expect("composition");
        let seule_b = composer_couches(&[pb[0].clone()]).expect("composition");
        assert_ne!(
            seule_a.2, seule_b.2,
            "deux peaux différentes doivent donner deux compositions différentes"
        );
    }

    /// Une planche RGBA 1×1 aux canaux choisis.
    #[cfg(feature = "textures")]
    fn pixel(r: u8, v: u8, b: u8, a: u8) -> Vec<u8> {
        vec![r, v, b, a]
    }

    #[cfg(feature = "textures")]
    fn teinte(rgb: [u8; 3], actif: bool) -> TeinteCanal {
        TeinteCanal { rgb, actif }
    }

    #[cfg(feature = "textures")]
    #[test]
    fn un_canal_plein_rend_sa_teinte() {
        // Canal rouge à fond, teinte chair : la sortie EST la teinte.
        let out = teinter_par_canaux(
            1,
            1,
            &pixel(255, 0, 0, 255),
            [
                teinte([243, 202, 193], true),
                teinte([0, 0, 0], true),
                teinte([255, 255, 255], true),
            ],
            true,
        )
        .unwrap();
        assert_eq!(&out[..3], &[243, 202, 193]);
    }

    #[cfg(feature = "textures")]
    #[test]
    fn un_canal_inactif_ne_teinte_rien() {
        // Même planche, mais le canal rouge est déclaré inactif (alpha 0 dans la recette).
        let out = teinter_par_canaux(
            1,
            1,
            &pixel(255, 0, 0, 255),
            [
                teinte([243, 202, 193], false),
                teinte([0, 0, 0], true),
                teinte([255, 255, 255], true),
            ],
            true,
        )
        .unwrap();
        assert_eq!(
            &out[..3],
            &[0, 0, 0],
            "un canal inactif ne doit rien apporter"
        );
    }

    #[cfg(feature = "textures")]
    #[test]
    fn un_canal_a_demi_pondere_sa_teinte() {
        let out = teinter_par_canaux(
            1,
            1,
            &pixel(128, 0, 0, 255),
            [
                teinte([200, 100, 50], true),
                teinte([0, 0, 0], true),
                teinte([0, 0, 0], true),
            ],
            true,
        )
        .unwrap();
        // 128/255 ≈ 0,502 : la teinte doit être réduite d'autant, pas rendue pleine.
        assert!((98..=102).contains(&out[0]), "obtenu {}", out[0]);
    }

    #[cfg(feature = "textures")]
    #[test]
    fn une_planche_qui_porte_sa_forme_se_reconnait() {
        // Couleur muette + alpha variable = la forme est dans le masque, la couleur est la bonne.
        // C'est le cas des reflets : les teinter les peindrait en carnation, donc invisibles sur
        // la peau. Le décideur est cette paire de prédicats.
        let reflet = [255, 255, 255, 0, 255, 255, 255, 200];
        assert!(canal_uniforme(&reflet) && !couche_totalement_opaque(&reflet));

        // Une planche qui porte son dessin dans la couleur, elle, doit être teintée.
        let bouche = [255, 0, 0, 255, 120, 0, 0, 255];
        assert!(!canal_uniforme(&bouche));
    }

    #[cfg(feature = "textures")]
    #[test]
    fn un_canal_constant_est_reconnu_uniforme() {
        // Le masque de la peau est uniforme : l'appliquer effacerait les variations de la planche.
        assert!(canal_uniforme(&[128, 0, 0, 255, 128, 9, 9, 255]));
        // Celui des reflets varie : c'est lui, et lui seul, qui porte le dessin.
        assert!(!canal_uniforme(&[128, 0, 0, 255, 200, 0, 0, 255]));
        assert!(canal_uniforme(&[]));
    }

    #[cfg(feature = "textures")]
    #[test]
    fn une_zone_de_carnation_posee_par_dessus_est_transparente() {
        // Le canal rouge est le fond. Une planche NEUTRE l'a partout — `eye_00` et
        // `highlight_00` sont uniformément rouges, c'est ainsi que le jeu dit « rien ici ».
        // Posée sur une autre, une telle zone doit laisser voir ce qu'il y a dessous.
        let teintes = [
            teinte([243, 202, 193], true),
            teinte([0, 0, 0], true),
            teinte([255, 255, 255], true),
        ];
        let posee = teinter_par_canaux(1, 1, &pixel(255, 0, 0, 255), teintes, false).unwrap();
        assert_eq!(
            posee[3], 0,
            "une zone de fond posée par-dessus doit être transparente"
        );

        // La même planche EN fond garde son opacité.
        let fond = teinter_par_canaux(1, 1, &pixel(255, 0, 0, 255), teintes, true).unwrap();
        assert_eq!(fond[3], 255, "la planche de fond, elle, reste opaque");
        assert_eq!(&fond[..3], &[243, 202, 193]);
    }

    #[cfg(feature = "textures")]
    #[test]
    fn le_canal_dominant_l_emporte() {
        // Vert plus fort que rouge : c'est la teinte du vert qui s'applique, pas leur somme.
        let out = teinter_par_canaux(
            1,
            1,
            &pixel(128, 255, 0, 255),
            [
                teinte([200, 0, 0], true),
                teinte([0, 180, 0], true),
                teinte([0, 0, 255], true),
            ],
            true,
        )
        .unwrap();
        assert_eq!(
            &out[..3],
            &[0, 180, 0],
            "le canal dominant sélectionne, il n'additionne pas"
        );
    }

    #[cfg(feature = "textures")]
    #[test]
    fn a_egalite_le_rouge_tranche() {
        // Une planche neutre est blanche partout : elle doit prendre la carnation du canal rouge.
        let out = teinter_par_canaux(
            1,
            1,
            &pixel(255, 255, 255, 255),
            [
                teinte([243, 202, 193], true),
                teinte([0, 0, 0], true),
                teinte([255, 255, 255], true),
            ],
            true,
        )
        .unwrap();
        assert_eq!(
            &out[..3],
            &[243, 202, 193],
            "sinon le blanc du canal bleu sature tout"
        );
    }

    #[cfg(feature = "textures")]
    #[test]
    fn sans_couche_il_n_y_a_rien_a_composer() {
        assert!(composer_couches(&[]).is_none());
    }

    /// Damier RGBA 4×4 avec de l'alpha, pour exercer les chemins avec et sans canal alpha.
    fn damier() -> (u32, u32, Vec<u8>) {
        let (w, h) = (4u32, 4u32);
        let mut rgba = Vec::with_capacity(64);
        for y in 0..h {
            for x in 0..w {
                let clair = (x + y) % 2 == 0;
                rgba.extend_from_slice(&[
                    if clair { 255 } else { 0 },
                    u8::try_from(x * 60).unwrap_or(255),
                    u8::try_from(y * 60).unwrap_or(255),
                    if clair { 255 } else { 128 },
                ]);
            }
        }
        (w, h, rgba)
    }

    #[test]
    fn chaque_format_produit_un_fichier_non_vide() {
        let (w, h, rgba) = damier();
        for f in ImageOut::TOUS {
            let out = encoder_rgba(&rgba, w, h, f).unwrap_or_else(|e| panic!("{f:?} : {e}"));
            assert!(!out.is_empty(), "{f:?} : sortie vide");
        }
    }

    #[test]
    fn les_magics_de_sortie_sont_conformes() {
        let (w, h, rgba) = damier();
        let magic = |f: ImageOut| encoder_rgba(&rgba, w, h, f).unwrap();
        assert_eq!(&magic(ImageOut::Png)[..8], b"\x89PNG\r\n\x1a\n");
        let webp = magic(ImageOut::Webp);
        assert_eq!(&webp[..4], b"RIFF");
        assert_eq!(&webp[8..12], b"WEBP");
        assert_eq!(&webp[12..16], b"VP8L", "WebP doit être sans perte (VP8L)");
        assert_eq!(&magic(ImageOut::Gif)[..6], b"GIF89a");
        assert_eq!(&magic(ImageOut::Jpeg)[..3], &[0xFF, 0xD8, 0xFF]);
        assert_eq!(&magic(ImageOut::Bmp)[..2], b"BM");
        assert_eq!(&magic(ImageOut::Qoi)[..4], b"qoif");
    }

    /// Le PNG doit rester produit par la crate `png` : c'est lui qui porte l'égalité à l'octet
    /// avec les références publiées. Si ce test tombe, l'oracle de non-régression est perdu.
    #[test]
    fn le_png_reste_sur_la_crate_png() {
        let (w, h, rgba) = damier();
        let par_image_out = encoder_rgba(&rgba, w, h, ImageOut::Png).unwrap();
        let par_chemin_historique =
            crate::g4tx_decode::encode_rgba_to_png(&rgba, w as usize, h as usize).unwrap();
        assert_eq!(par_image_out, par_chemin_historique);
    }

    #[test]
    fn les_dimensions_incoherentes_sont_refusees() {
        let (w, h, rgba) = damier();
        assert!(encoder_rgba(&rgba, w + 1, h, ImageOut::Png).is_err());
        assert!(encoder_rgba(&rgba, 0, 0, ImageOut::Png).is_err());
        assert!(reduire_rgba(&rgba, w + 1, h, 2).is_err());
        assert!(reduire_rgba(&rgba, w, h, 0).is_err());
    }

    #[test]
    fn une_image_deja_petite_traverse_la_reduction_intacte() {
        let (w, h, rgba) = damier();
        let (nw, nh, out) = reduire_rgba(&rgba, w, h, 64).unwrap();
        assert_eq!((nw, nh), (w, h));
        assert_eq!(
            out, rgba,
            "aucune vignette ne doit agrandir ni recompresser"
        );
    }

    #[test]
    fn la_reduction_borne_le_plus_grand_cote_et_garde_le_rapport() {
        // Bande large : c'est le cas qui casse une division naïve (hauteur ramenée à 0).
        let (w, h) = (400u32, 5u32);
        let rgba = alloc::vec![200u8; (w * h * 4) as usize];
        let (nw, nh, out) = reduire_rgba(&rgba, w, h, 100).unwrap();
        assert_eq!(nw, 100);
        assert!(nh >= 1, "une dimension ne doit jamais tomber à zéro");
        assert_eq!(out.len(), (nw * nh * 4) as usize);
    }

    /// Une couleur uniforme doit traverser la moyenne sans dériver : c'est le test qui attrape
    /// une erreur de pondération (somme non divisée, alpha compté deux fois…).
    #[test]
    fn une_image_uniforme_reste_de_la_meme_couleur() {
        let (w, h) = (64u32, 64u32);
        let mut rgba = Vec::new();
        for _ in 0..(w * h) {
            rgba.extend_from_slice(&[10, 120, 230, 255]);
        }
        let (_, _, out) = reduire_rgba(&rgba, w, h, 8).unwrap();
        for px in out.chunks_exact(4) {
            assert_eq!(px, [10, 120, 230, 255]);
        }
    }

    /// Moitié opaque rouge, moitié transparente (RGB arbitraire) : la vignette doit rester rouge.
    /// Sans prémultiplication par l'alpha, le noir des pixels transparents assombrirait le bord.
    #[test]
    fn les_pixels_transparents_ne_teintent_pas_la_vignette() {
        let (w, h) = (16u32, 16u32);
        let mut rgba = Vec::new();
        for y in 0..h {
            for _ in 0..w {
                if y < h / 2 {
                    rgba.extend_from_slice(&[255, 0, 0, 255]);
                } else {
                    rgba.extend_from_slice(&[0, 0, 0, 0]); // transparent, couleur arbitraire
                }
            }
        }
        let (_, _, out) = reduire_rgba(&rgba, w, h, 2).unwrap();
        // Ligne du haut : rouge pur, pas un rouge assombri par les voisins transparents.
        assert_eq!(&out[..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn extensions_et_proprietes() {
        assert_eq!(ImageOut::depuis_extension("WEBP"), Some(ImageOut::Webp));
        assert_eq!(ImageOut::depuis_extension(".jpeg"), Some(ImageOut::Jpeg));
        assert_eq!(ImageOut::depuis_extension("jpg"), Some(ImageOut::Jpeg));
        assert_eq!(ImageOut::depuis_extension("tif"), Some(ImageOut::Tiff));
        assert_eq!(ImageOut::depuis_extension("psd"), None);

        assert!(ImageOut::Webp.sans_perte());
        assert!(ImageOut::Png.sans_perte());
        assert!(
            !ImageOut::Gif.sans_perte(),
            "GIF quantifie sur 256 couleurs"
        );
        assert!(!ImageOut::Jpeg.sans_perte());
        assert!(!ImageOut::Jpeg.garde_alpha());
        assert!(ImageOut::Webp.garde_alpha());
    }

    /// L'aller-retour WebP doit rendre les pixels d'origine : c'est ce que « sans perte » veut
    /// dire, et ça se vérifie plutôt que ça ne se déclare.
    #[test]
    fn le_webp_sans_perte_restitue_les_pixels() {
        let (w, h, rgba) = damier();
        let webp = encoder_rgba(&rgba, w, h, ImageOut::Webp).unwrap();
        let relu = image::load_from_memory_with_format(&webp, image::ImageFormat::WebP)
            .expect("relecture WebP")
            .to_rgba8();
        assert_eq!(relu.dimensions(), (w, h));
        assert_eq!(
            relu.as_raw().as_slice(),
            rgba.as_slice(),
            "VP8L doit être exact"
        );
    }
    /// Un masque de zones : fond rouge franc, une région noire au milieu.
    #[cfg(feature = "textures")]
    #[test]
    fn une_planche_dessinee_garde_son_dessin_hors_du_fond() {
        // 2×2 : trois pixels de fond rouge, un pixel de zone noire en dernier.
        let planche = [
            10, 20, 30, 255, // le dessin, sous le fond — doit disparaître
            40, 50, 60, 255, //
            70, 80, 90, 255, //
            11, 22, 33, 255, // le dessin, dans la zone — doit rester
        ];
        let masque = [255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 0, 0, 0, 255];
        let out = decouper_par_zones(2, 2, &planche, &masque).unwrap();
        assert_eq!(
            &out[0..4],
            &[10, 20, 30, 0],
            "le fond rouge devient transparent"
        );
        assert_eq!(
            &out[12..16],
            &[11, 22, 33, 255],
            "la zone garde couleur et opacité"
        );
    }

    #[cfg(feature = "textures")]
    #[test]
    fn un_masque_gris_n_est_pas_un_masque_de_zones() {
        // Une découpe en niveaux de gris : aucun pixel rouge franc.
        let gris: Vec<u8> = [128, 128, 128, 255].repeat(16);
        assert!(!masque_de_zones(&gris));
        // Un masque de zones : fond rouge saturé sur la moitié des pixels.
        let mut zones: Vec<u8> = [255, 0, 0, 255].repeat(8);
        zones.extend([0, 0, 0, 255].repeat(8));
        assert!(masque_de_zones(&zones));
    }

    #[cfg(feature = "textures")]
    #[test]
    fn seule_une_planche_a_encre_porte_un_trait() {
        // Une planche blanche à liseré gris clair — `eye_L_01` : elle marque une zone, elle ne
        // dessine pas. La conserver opaque recouvrait le visage de blanc.
        let mut pale: Vec<u8> = [255, 255, 255, 255].repeat(200);
        pale.extend([200, 200, 200, 255].repeat(24));
        assert!(!porte_un_trait(&pale));

        // Une planche au trait noir — `mouth_01` : 5 % de pixels franchement sombres.
        let mut encre: Vec<u8> = [255, 255, 255, 255].repeat(190);
        encre.extend([10, 10, 10, 255].repeat(10));
        assert!(porte_un_trait(&encre));

        // Du noir TRANSPARENT ne compte pas : c'est du vide, pas de l'encre.
        let vide: Vec<u8> = [0, 0, 0, 0].repeat(200);
        assert!(!porte_un_trait(&vide));
    }
}
