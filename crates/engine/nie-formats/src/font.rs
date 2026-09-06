//! Métriques de glyphes et blit de la police bitmap IEVR — interprète `font.cfg.bin`.
//!
//! ## Découverte (2026-06-16, RE originale validée)
//!
//! Le rendu de texte composé (« couche 4 » de `docs/DESIGN.md` : `ver 6.0.2`, `212`,
//! `99`, libellés runtime) était réputé bloqué par un parseur **`.g4tg`** « à reverser ».
//! C'est **faux** : aucun `.g4tg` de police n'existe dans le VFS (les seuls `.g4tg` sont
//! des textures d'effets `effect/…`). Les métriques de glyphes vivent dans
//! **`data/common/font/font/font_def/font.cfg.bin`** — un **T2B** que `cfgbin::parse_t2b`
//! décode déjà. L'atlas est `data/dx11/font/font_def/font.g4tx` (DDS 4096×2048, ~44 Mo).
//!
//! ## Layout (validé contre des largeurs ASCII connues)
//!
//! Le fichier a **DEUX** définitions de police (entrées `INF`, col[0]=index 0/1 = grande /
//! petite), puis une entrée `CHR` par glyphe, une table `KERN`/`KERNINF`, et `END` :
//!
//! ```text
//! INF.variables = [font, ascent, cell_height, descent, nGlyphs, ATLAS_W, ATLAS_H, _]
//!                  0     1       2            3        4         5        6
//! CHR.variables = [font, base, codepoint, atlasX, atlasY, width, bearingX, advance, page]
//!                  0     1     2          3       4       5      6         7        8
//! ```
//!
//! ⚠ **Le point de code est la colonne 2, PAS la 1** : la col[1] est un id de groupe
//! partagé (ex. 65 pour 'A' ET pour des variantes CJK 50048+) ; seule la col[2] est unique
//! par glyphe (validé : col2==65 → glyphe 'A' à atlasX=1157 w=38 adv=39 ; col2==50048+ =
//! glyphes CJK). Clé = (font, col[2]).
//!
//! Preuves (cp police 0 → width/advance) : `A` 38/39, `W` 47/48, `i` 7/12, `m` 38/46,
//! `0` 31/38, `!` 7/11, espace 1/16. `atlasX` ∈ [1,4090] ⊂ 4096 ; `atlasY` ∈ [1,1973] ⊂
//! 2048 ; `page` ∈ [0,3]. Kerning (`KERN`) non encore interprété (champ futur).
//!
//! ## Format des pixels de l'atlas (DDS, validé)
//!
//! L'atlas `font.g4tx` contient un payload DDS **non compressé BGRA8** (32 bpp, format
//! DDS R mask=`0x00ff0000`, G=`0x0000ff00`, B=`0x000000ff`, A=`0xff000000`). Les
//! pixels sont stockés `[B, G, R, A]` par octet. Le canal **A** (octet 3) est le masque
//! du glyphe — validé sur le glyphe `A` : A=251 à la ligne relative 20, colonne 0.
//! Les canaux B/G/R sont ≈ 251 là où le glyphe est opaque (anticrénelage blanc).
//!
//! Voir [`glyph_blitter`] pour la primitive de rendu.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::cfgbin::{CfgBinFile, Value};

/// Métrique d'un glyphe : où le découper dans l'atlas et comment l'avancer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GlyphMetric {
    /// Index de police (0 = grande/principale, 1 = petite). `CHR` col[0].
    pub font: u8,
    /// Id de groupe (`CHR` col[1]) — partagé entre variantes ; sémantique exacte à confirmer.
    pub base: u32,
    /// Point de code du glyphe (`CHR` col[2] — la clé unique, PAS col[1]).
    pub codepoint: u32,
    /// Position X (gauche) du glyphe dans l'atlas, en pixels.
    pub x: u16,
    /// Position Y (haut) du glyphe dans l'atlas, en pixels.
    pub y: u16,
    /// Largeur du glyphe dans l'atlas, en pixels.
    pub width: u16,
    /// Décalage horizontal (left side bearing), en pixels (peut être négatif).
    pub bearing_x: i16,
    /// Avance horizontale du curseur après ce glyphe, en pixels.
    pub advance: u16,
    /// Page d'atlas (0..=3) — l'atlas de police peut comporter plusieurs couches (CJK).
    pub page: u8,
}

/// Dimensions d'une police, extraites d'une entrée `INF`.
///
/// La relation exacte est : `ascent + descent == cell_height` (ex. 46 + 25 = 71 pour
/// la police principale). Ces valeurs permettent de calculer la ligne de base lors
/// du rendu : sommet de cellule = `pen_y − ascent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FontDimensions {
    /// Montante : pixels au-dessus de la ligne de base. `INF` col[1] (ex. 46).
    pub ascent: u16,
    /// Hauteur totale d'une cellule de glyphe. `INF` col[2] (ex. 71).
    pub cell_height: u16,
    /// Descendante : pixels en-dessous de la ligne de base. `INF` col[3] (ex. 25).
    pub descent: u16,
}

/// Table de métriques de glyphes complète (issue de `font.cfg.bin`).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FontMetrics {
    /// Largeur de l'atlas de police, en pixels (entrée `INF`, ex. 4096).
    pub atlas_width: u32,
    /// Hauteur de l'atlas de police, en pixels (entrée `INF`, ex. 2048).
    pub atlas_height: u32,
    /// Champs bruts des en-têtes `INF` (une ligne par police).
    pub inf_raw: Vec<Vec<i32>>,
    /// Dimensions de la police principale (font 0).
    pub dims: FontDimensions,
    /// Dimensions de la petite police (font 1).
    pub dims_small: FontDimensions,
    /// Glyphes de la police 0 (principale), par point de code.
    pub glyphs: BTreeMap<u32, GlyphMetric>,
    /// Glyphes de la police 1 (petite), par point de code.
    pub glyphs_small: BTreeMap<u32, GlyphMetric>,
}

impl FontMetrics {
    /// Métrique d'un point de code dans la police principale (0), ou `None`.
    #[must_use]
    pub fn glyph(&self, codepoint: u32) -> Option<&GlyphMetric> {
        self.glyphs.get(&codepoint)
    }

    /// Métrique d'un **caractère**, quelle que soit la forme sous laquelle le jeu l'a stocké.
    ///
    /// Au-delà de l'ASCII, la table n'est pas indexée par point de code Unicode mais par les
    /// **octets UTF-8 empaquetés** en big-endian (cf. [`decode_packed_codepoint`]) : `é` vit sous
    /// `0xC3A9`, pas `0xE9`. Chercher `c as u32` rate donc tous les accents — et un menu français
    /// s'affiche « Composition d' quipe ». Les deux formes sont essayées ici, une fois pour
    /// toutes, plutôt qu'à chaque site d'appel.
    #[must_use]
    pub fn glyph_char(&self, c: char) -> Option<&GlyphMetric> {
        self.glyph(c as u32)
            .or_else(|| self.glyph(cle_empaquetee(c)))
    }

    /// Métrique d'un point de code dans une police donnée (0 ou 1).
    #[must_use]
    pub fn glyph_in_font(&self, font: u8, codepoint: u32) -> Option<&GlyphMetric> {
        let map = if font == 1 {
            &self.glyphs_small
        } else {
            &self.glyphs
        };
        map.get(&codepoint)
    }

    /// Nombre de glyphes de la police principale (0).
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }
}

/// Clé de table d'un caractère : ses octets UTF-8 empaquetés en big-endian.
///
/// L'inverse de [`decode_packed_codepoint`] — c'est sous cette forme que le jeu range tout ce qui
/// dépasse l'ASCII, parce qu'il lit son texte source en UTF-8 et stocke l'entier sans le décoder.
#[must_use]
pub fn cle_empaquetee(c: char) -> u32 {
    let mut buf = [0u8; 4];
    c.encode_utf8(&mut buf)
        .as_bytes()
        .iter()
        .fold(0u32, |acc, &b| (acc << 8) | u32::from(b))
}

/// Résout un `CHR.codepoint` brut vers son caractère réel.
///
/// Découverte en cataloguant les 8 fontes non-Latin (2026-08-15, `examples/font_catalog.rs`) :
/// au-delà de l'ASCII, `codepoint` n'est **pas** toujours un point de code Unicode direct — c'est
/// souvent la **séquence d'octets UTF-8 du caractère, empaquetée big-endian dans l'entier** (le
/// jeu lit le texte source en UTF-8 et stocke l'entier tel quel, sans le décoder). Exemples
/// mesurés sur `font_zh_hans`/`font_ja_endroll2`/`font_zh_hans2` : `0xC2A1` = octets `[C2,A1]` =
/// `¡` (U+00A1) ; `0xE296A0` = octets `[E2,96,A0]` = `■` (U+25A0) ; `0xE38080` = octets
/// `[E3,80,80]` = U+3000 (espace idéographique). Un point de code BMP stocké DIRECT (ex. `0x3042`
/// 'あ') reste géré : ses octets ne forment pas une séquence UTF-8 valide (`0x00` n'est pas un
/// octet de tête `0xC2..=0xF4`), donc l'essai d'empaquetage échoue et on retombe sur
/// `char::from_u32` brut.
#[must_use]
pub fn decode_packed_codepoint(raw: u32) -> Option<char> {
    if raw <= 0x7F {
        return char::from_u32(raw);
    }
    // 4 octets empaquetés (tête 0xF0..=0xF4) : le plus large repéré tient sur 32 bits sans reste.
    {
        let b = [
            (raw >> 24) as u8,
            (raw >> 16) as u8,
            (raw >> 8) as u8,
            raw as u8,
        ];
        if (0xF0..=0xF4).contains(&b[0])
            && b[1..].iter().all(|&x| (0x80..=0xBF).contains(&x))
            && let Ok(s) = core::str::from_utf8(&b)
            && let Some(c) = s.chars().next()
        {
            return Some(c);
        }
    }
    // 3 octets empaquetés (tête 0xE0..=0xEF).
    if raw <= 0x00FF_FFFF {
        let b = [(raw >> 16) as u8, (raw >> 8) as u8, raw as u8];
        if (0xE0..=0xEF).contains(&b[0])
            && b[1..].iter().all(|&x| (0x80..=0xBF).contains(&x))
            && let Ok(s) = core::str::from_utf8(&b)
            && let Some(c) = s.chars().next()
        {
            return Some(c);
        }
    }
    // 2 octets empaquetés (tête 0xC2..=0xDF — 0xC0/0xC1 sont des surlongueurs UTF-8 invalides).
    if raw <= 0x0000_FFFF {
        let b = [(raw >> 8) as u8, raw as u8];
        if (0xC2..=0xDF).contains(&b[0])
            && (0x80..=0xBF).contains(&b[1])
            && let Ok(s) = core::str::from_utf8(&b)
            && let Some(c) = s.chars().next()
        {
            return Some(c);
        }
    }
    // Aucun motif d'empaquetage reconnu : point de code Unicode direct (BMP, etc.).
    char::from_u32(raw)
}

#[inline]
fn as_i32(v: Option<&Value>) -> Option<i32> {
    match v {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    }
}

/// Interprète un `font.cfg.bin` déjà parsé (T2B) en table de métriques de glyphes.
///
/// Tolérant : ignore les entrées `CHR` malformées (< 9 colonnes) et les codepoints
/// négatifs. L'en-tête `INF` est optionnel (atlas 0×0 s'il manque).
#[must_use]
pub fn parse_metrics(cfg: &CfgBinFile) -> FontMetrics {
    let mut out = FontMetrics::default();

    for e in &cfg.entries {
        match e.name.as_str() {
            "INF" => {
                let row: Vec<i32> = e
                    .variables
                    .iter()
                    .filter_map(|v| {
                        if let Value::Int(n) = v {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .collect();
                // Colonnes 5/6 = largeur/hauteur de l'atlas (validé : 4096/2048), partagées.
                if out.atlas_width == 0 {
                    out.atlas_width = row.get(5).copied().unwrap_or(0).max(0) as u32;
                    out.atlas_height = row.get(6).copied().unwrap_or(0).max(0) as u32;
                }
                // Dimensions : col[1]=montante, col[2]=hauteur_cellule, col[3]=descendante.
                let dims = FontDimensions {
                    ascent: row.get(1).copied().unwrap_or(0).clamp(0, u16::MAX as i32) as u16,
                    cell_height: row.get(2).copied().unwrap_or(0).clamp(0, u16::MAX as i32) as u16,
                    descent: row.get(3).copied().unwrap_or(0).clamp(0, u16::MAX as i32) as u16,
                };
                let font_idx = row.first().copied().unwrap_or(0);
                if font_idx == 0 && out.dims.cell_height == 0 {
                    out.dims = dims;
                } else if font_idx == 1 && out.dims_small.cell_height == 0 {
                    out.dims_small = dims;
                }
                out.inf_raw.push(row);
            }
            "CHR" => {
                if e.variables.len() < 9 {
                    continue;
                }
                // Le point de code unique est la COLONNE 2 (col[1] est un id de groupe partagé).
                let cp = match as_i32(e.variables.get(2)) {
                    Some(n) if n >= 0 => n as u32,
                    _ => continue,
                };
                let font = as_i32(e.variables.first())
                    .unwrap_or(0)
                    .clamp(0, u8::MAX as i32) as u8;
                let g = GlyphMetric {
                    font,
                    base: as_i32(e.variables.get(1)).unwrap_or(0).max(0) as u32,
                    codepoint: cp,
                    x: as_i32(e.variables.get(3))
                        .unwrap_or(0)
                        .clamp(0, u16::MAX as i32) as u16,
                    y: as_i32(e.variables.get(4))
                        .unwrap_or(0)
                        .clamp(0, u16::MAX as i32) as u16,
                    width: as_i32(e.variables.get(5))
                        .unwrap_or(0)
                        .clamp(0, u16::MAX as i32) as u16,
                    bearing_x: as_i32(e.variables.get(6))
                        .unwrap_or(0)
                        .clamp(i16::MIN as i32, i16::MAX as i32)
                        as i16,
                    advance: as_i32(e.variables.get(7))
                        .unwrap_or(0)
                        .clamp(0, u16::MAX as i32) as u16,
                    page: as_i32(e.variables.get(8))
                        .unwrap_or(0)
                        .clamp(0, u8::MAX as i32) as u8,
                };
                if font == 1 {
                    out.glyphs_small.insert(cp, g);
                } else {
                    out.glyphs.insert(cp, g);
                }
            }
            _ => {}
        }
    }
    out
}

/// Trace un glyphe sur un canevas RGBA8 depuis l'atlas de police BGRA8.
///
/// ## Format de l'atlas
///
/// `atlas` doit pointer sur les **pixels bruts mip0** du DDS contenu dans `font.g4tx` :
/// octets à partir de `G4txTexture::data_offset + 128` (4 magic + 124 en-tête DDS,
/// sans extension DX10). Chaque pixel occupe 4 octets `[B, G, R, A]` (canal A = masque
/// du glyphe, validé sur l'atlas réel). `atlas_w` est la largeur en pixels (ex. 4096).
///
/// ## Rendu
///
/// Pour chaque pixel du rectangle `[metric.x, metric.y, metric.x+metric.width,
/// metric.y+cell_height]` dans l'atlas :
/// - Si le canal A de l'atlas est nul → pixel sauté (transparent).
/// - Sinon : écriture dans le canevas en RGBA8 à `(dst_x + col, dst_y + row)` :
///   `[color[0], color[1], color[2], atlas_A * color[3] / 255]`.
///
/// Le compositage est une opération **src-over** simplifiée (le pixel source remplace le
/// pixel destination). Pour les pixels transparents de l'atlas, le canevas est inchangé.
/// Cette sémantique suffit pour le rendu de texte sur fond transparent.
///
/// ## Paramètres
///
/// - `atlas`        : pixels BGRA8 du mip0 de l'atlas (voir ci-dessus).
/// - `atlas_w`      : largeur de l'atlas en pixels (ex. 4096 pour `font.g4tx`).
/// - `metric`       : métrique du glyphe à tracer (issue de [`FontMetrics::glyph`]).
/// - `cell_height`  : hauteur de cellule de la police (ex. `FontMetrics::dims.cell_height` = 71).
/// - `canvas`       : tampon de sortie RGBA8 (stride = `canvas_stride` octets par ligne).
/// - `canvas_stride`: stride du canevas en octets (= largeur_en_pixels × 4 pour un buffer compact).
/// - `dst_x`        : colonne gauche du coin haut-gauche du glyphe dans le canevas.
/// - `dst_y`        : ligne du haut du coin haut-gauche du glyphe dans le canevas.
/// - `color`        : teinte RGBA `[R, G, B, A]` appliquée au glyphe. Pour du blanc opaque : `[255, 255, 255, 255]`.
///
/// ## Exemple
///
/// ```rust
/// use nie_formats::font::{GlyphMetric, glyph_blitter};
///
/// // Atlas 4×4 BGRA8 : pixel opaque à (col=2, row=1), A=200.
/// let mut atlas = [0u8; 4 * 4 * 4];
/// // index = (row=1 * atlas_w=4 + col=2) * 4 + 3 = 6*4+3 = 27
/// atlas[27] = 200;
///
/// let metric = GlyphMetric {
///     font: 0, base: 0, codepoint: 65,
///     x: 2, y: 1, width: 1, bearing_x: 0, advance: 4, page: 0,
/// };
/// let mut canvas = [0u8; 8 * 8 * 4];
/// glyph_blitter(&atlas, 4, &metric, 2, &mut canvas, 8 * 4, 0, 0, [255, 255, 255, 255]);
///
/// // row=0 de la boucle → atlas_row=ay+0=1, canvas_row=dst_y+0=0.
/// // atlas(col=2, row=1) → canvas(col=0, row=0) = offset 0.
/// assert_eq!(canvas[3], 200, "alpha tracé à la ligne 0 du canevas");
/// ```
#[allow(clippy::too_many_arguments)]
pub fn glyph_blitter(
    atlas: &[u8],
    atlas_w: u32,
    metric: &GlyphMetric,
    cell_height: u16,
    canvas: &mut [u8],
    canvas_stride: u32,
    dst_x: i32,
    dst_y: i32,
    color: [u8; 4],
) {
    let atlas_stride = atlas_w as usize * 4;
    let cv_stride = canvas_stride as usize;
    let h = cell_height as i32;
    let w = metric.width as i32;
    let ax = metric.x as i32;
    let ay = metric.y as i32;

    for row in 0..h {
        let canvas_row = dst_y + row;
        if canvas_row < 0 {
            continue;
        }
        let atlas_row = ay + row;
        if atlas_row < 0 {
            continue;
        }
        let atlas_row_off = atlas_row as usize * atlas_stride;
        let canvas_row_off = canvas_row as usize * cv_stride;

        for col in 0..w {
            let canvas_col = dst_x + col;
            if canvas_col < 0 {
                continue;
            }
            let atlas_col = ax + col;
            if atlas_col < 0 {
                continue;
            }

            // Octet 3 du pixel BGRA8 = canal alpha (masque du glyphe).
            let a_off = atlas_row_off + atlas_col as usize * 4;
            let atlas_alpha = match atlas.get(a_off + 3) {
                Some(&v) => v,
                None => continue,
            };
            if atlas_alpha == 0 {
                continue;
            }

            // Alpha de sortie : produit de l'alpha du glyphe et de la couleur de teinte.
            let out_a = (atlas_alpha as u32 * color[3] as u32 / 255) as u8;

            let c_off = canvas_row_off + canvas_col as usize * 4;
            let canvas_slice = match canvas.get_mut(c_off..c_off + 4) {
                Some(s) => s,
                None => continue,
            };
            // Écriture src-over simplifiée (source remplace destination).
            canvas_slice[0] = color[0]; // R
            canvas_slice[1] = color[1]; // G
            canvas_slice[2] = color[2]; // B
            canvas_slice[3] = out_a; // A
        }
    }
}

/// Trace une chaîne de caractères UTF-8 sur un canevas RGBA8.
///
/// Calcule la position de chaque glyphe à partir de la ligne de base (`pen_y`) et
/// du curseur horizontal (`pen_x_start`). Pour chaque point de code :
/// 1. Recherche la métrique dans `metrics`.
/// 2. Calcule `dst_x = pen_x + metric.bearing_x` et `dst_y = pen_y − metrics.dims.ascent`.
/// 3. Appelle [`glyph_blitter`].
/// 4. Avance `pen_x += metric.advance`.
///
/// Les points de code sans métrique dans la police principale sont ignorés.
///
/// Renvoie l'avance totale en pixels (largeur rendue approximative du texte).
#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    atlas: &[u8],
    atlas_w: u32,
    metrics: &FontMetrics,
    text: &str,
    canvas: &mut [u8],
    canvas_stride: u32,
    pen_x_start: i32,
    pen_y: i32,
    color: [u8; 4],
) -> i32 {
    let mut pen_x = pen_x_start;
    let cell_height = metrics.dims.cell_height;
    let ascent = metrics.dims.ascent as i32;
    for ch in text.chars() {
        let cp = ch as u32;
        let Some(m) = metrics.glyph(cp) else { continue };
        let dst_x = pen_x + m.bearing_x as i32;
        let dst_y = pen_y - ascent;
        glyph_blitter(
            atlas,
            atlas_w,
            m,
            cell_height,
            canvas,
            canvas_stride,
            dst_x,
            dst_y,
            color,
        );
        pen_x += m.advance as i32;
    }
    pen_x - pen_x_start
}

// ============================================================================
// LatinAtlas — rendu de texte Latin par EDGE-SCAN (2026-06-20).
// L'atlas `font.g4tx` REPACKE les glyphes (X non résolu via les métriques col3). MAIS la rangée
// physique ASCII range les glyphes en ORDRE DE CODEPOINT CONTIGU depuis '!' (0x21) — vérifié
// visuellement. On edge-scan l'alpha de cette rangée → spans (x0,x1) par colonne lumineuse → le
// k-ième span = codepoint 0x21+k. Permet le rendu de texte Latin arbitraire sans le repacking.
// ============================================================================

/// Premier codepoint de la rangée physique ASCII (`!`).
pub const LATIN_FIRST: u32 = 0x21;

/// Atlas Latin résolu par edge-scan : spans physiques `(x0,x1)` par codepoint depuis [`LATIN_FIRST`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LatinAtlas {
    /// `(x0, x1)` du glyphe, indexé depuis `LATIN_FIRST`.
    pub spans: Vec<(u16, u16)>,
    /// Y physique de la rangée dans l'atlas.
    pub y_base: u16,
    /// Hauteur de cellule (à blitter).
    pub cell_h: u16,
    /// Première ligne ENCRÉE de la cellule, comptée depuis son haut.
    ///
    /// Une cellule n'est pas pleine : sur `font_def`, l'encre commence à la ligne 21 sur 71.
    /// Une mise en page qui l'ignore place le texte 21 px trop bas et le fait déborder de sa
    /// boîte — c'est ce qui arrivait aux libellés du menu, qui dépassaient sous leur surlignage.
    pub ink_top: u16,
    /// Hauteur réellement encrée (50 sur `font_def`, contre 71 de cellule).
    ///
    /// C'est cette hauteur-là qu'une mise en page doit réserver, pas `cell_h` : réserver la
    /// cellule entière gaspille un tiers de la place et ne tient pas sur neuf lignes.
    pub ink_h: u16,
}

impl LatinAtlas {
    /// Edge-scan d'une rangée physique de l'atlas. `atlas` = pixels BGRA8/RGBA8 (**alpha @ +3**),
    /// `aw`×`ah` ; `y_base` = Y de la rangée ASCII (≈946 pour font_def) ; `cell_h` = hauteur cellule.
    /// Fusionne les creux internes < 3 px (glyphes troués `!`, `%`…).
    #[must_use]
    pub fn from_atlas(atlas: &[u8], aw: usize, ah: usize, y_base: u16, cell_h: u16) -> Self {
        let stride = aw * 4;
        let (y0, y1) = (
            y_base as usize + 4,
            (y_base as usize + cell_h as usize).min(ah),
        );
        let mut raw: Vec<(u16, u16)> = Vec::new();
        let (mut on, mut start) = (false, 0usize);
        for ax in 0..aw {
            let mut s = 0u32;
            for ay in y0..y1 {
                s += u32::from(atlas.get(ay * stride + ax * 4 + 3).copied().unwrap_or(0));
            }
            let lit = s > 180;
            if lit && !on {
                start = ax;
                on = true;
            } else if !lit && on {
                raw.push((start as u16, ax as u16));
                on = false;
            }
        }
        if on {
            raw.push((start as u16, aw as u16));
        }
        // Fusion des micro-creux internes (< 3 px) d'un même glyphe.
        let mut spans: Vec<(u16, u16)> = Vec::new();
        for (a, b) in raw {
            if let Some(last) = spans.last_mut()
                && a.saturating_sub(last.1) < 3
            {
                last.1 = b;
                continue;
            }
            spans.push((a, b));
        }

        // Bornes verticales de l'encre : la même rangée, scannée en lignes plutôt qu'en colonnes.
        // Mesurée ici plutôt que déduite d'`ascent`, parce que c'est la seule valeur qui décrit
        // ce que l'atlas contient VRAIMENT — les métriques décrivent une autre planche.
        let (mut haut, mut bas) = (None::<u16>, 0u16);
        for gy in 0..cell_h {
            let ay = y_base as usize + gy as usize;
            if ay >= ah {
                break;
            }
            let encre =
                (0..aw).any(|ax| atlas.get(ay * stride + ax * 4 + 3).copied().unwrap_or(0) > 32);
            if encre {
                haut.get_or_insert(gy);
                bas = gy;
            }
        }
        let ink_top = haut.unwrap_or(0);
        let ink_h = bas.saturating_sub(ink_top) + 1;

        Self {
            spans,
            y_base,
            cell_h,
            ink_top,
            ink_h,
        }
    }

    /// Span `(x0,x1)` d'un codepoint Latin, ou `None` si hors de la rangée scannée.
    #[must_use]
    pub fn span(&self, cp: u32) -> Option<(u16, u16)> {
        if cp < LATIN_FIRST {
            return None;
        }
        self.spans.get((cp - LATIN_FIRST) as usize).copied()
    }

    /// Largeur en pixels d'une ligne (espaces = `cell_h/3`, gap inter-glyphe 2 px).
    #[must_use]
    pub fn measure(&self, text: &str) -> u32 {
        let mut w = 0i32;
        for c in text.chars() {
            if c == ' ' {
                w += i32::from(self.cell_h) / 3;
            } else if let Some((x0, x1)) = self.span(c as u32) {
                w += i32::from(x1 - x0) + 2;
            } else {
                w += i32::from(self.cell_h) / 3;
            }
        }
        w.max(0) as u32
    }

    /// Blit une ligne de texte sur un canvas RGBA8 existant (`cw` px de large), pen à `(x,y)`.
    /// `atlas` = mêmes pixels que `from_atlas` (alpha @ +3). Couleur `fg` RGBA.
    #[allow(clippy::too_many_arguments)] // primitive de blit bas-niveau (cf. glyph_blitter).
    pub fn blit_line(
        &self,
        atlas: &[u8],
        aw: usize,
        canvas: &mut [u8],
        cw: usize,
        x: i32,
        y: i32,
        text: &str,
        fg: [u8; 4],
    ) {
        let stride = aw * 4;
        let ch = canvas.len() / (cw.max(1) * 4);
        let mut pen = x;
        for c in text.chars() {
            if c == ' ' {
                pen += i32::from(self.cell_h) / 3;
                continue;
            }
            let Some((x0, x1)) = self.span(c as u32) else {
                pen += i32::from(self.cell_h) / 3;
                continue;
            };
            let gw = (x1 - x0) as usize;
            for gy in 0..self.cell_h as usize {
                let ay = self.y_base as usize + gy;
                for gx in 0..gw {
                    let a = atlas
                        .get(ay * stride + (x0 as usize + gx) * 4 + 3)
                        .copied()
                        .unwrap_or(0);
                    if a < 8 {
                        continue;
                    }
                    let (px, py) = (pen + gx as i32, y + gy as i32);
                    if px < 0 || py < 0 || px >= cw as i32 || py >= ch as i32 {
                        continue;
                    }
                    let o = (py as usize * cw + px as usize) * 4;
                    let af = f32::from(a) / 255.0;
                    for k in 0..3 {
                        canvas[o + k] =
                            (f32::from(fg[k]) * af + f32::from(canvas[o + k]) * (1.0 - af)) as u8;
                    }
                    canvas[o + 3] = 255;
                }
            }
            pen += gw as i32 + 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfgbin::CfgEntry;

    /// `decode_packed_codepoint` : cas mesurés sur les vraies fontes (2026-08-15) — 2/3 octets
    /// UTF-8 empaquetés, et non-régression sur l'ASCII et un BMP direct.
    #[test]
    fn decode_packed_codepoint_cas_reels() {
        assert_eq!(decode_packed_codepoint(0x21), Some('!'), "ASCII direct");
        assert_eq!(
            decode_packed_codepoint(0xC2A1),
            Some('¡'),
            "2 octets empaquetés, font_zh_hans"
        );
        assert_eq!(
            decode_packed_codepoint(0xE296A0),
            Some('■'),
            "3 octets empaquetés, font_ja_endroll2"
        );
        assert_eq!(
            decode_packed_codepoint(0xE38080),
            Some('\u{3000}'),
            "3 octets empaquetés, font_zh_hans2"
        );
        assert_eq!(
            decode_packed_codepoint(0x3042),
            Some('あ'),
            "BMP direct (octets non-UTF-8 valides ensemble)"
        );
    }

    /// Construit une entrée `CHR` à partir de ses 9 colonnes (col[2] = codepoint).
    fn chr(cols: [i32; 9]) -> CfgEntry {
        CfgEntry {
            name: "CHR".into(),
            variables: cols.iter().map(|n| Value::Int(*n)).collect(),
            children: alloc::vec::Vec::new(),
        }
    }

    fn inf(cols: [i32; 8]) -> CfgEntry {
        CfgEntry {
            name: "INF".into(),
            variables: cols.iter().map(|n| Value::Int(*n)).collect(),
            children: alloc::vec::Vec::new(),
        }
    }

    /// Synthétique : layout de colonnes décodé correctement, dimensions INF extraites.
    #[test]
    fn parse_metrics_decodes_columns() {
        let cfg = CfgBinFile {
            format: crate::cfgbin::Format::T2b,
            entries: alloc::vec![
                // INF police 0 : ascent=46, cell_height=71, descent=25, nGlyphs=7469, w=4096, h=2048
                inf([0, 46, 71, 25, 7469, 4096, 2048, 0]),
                // 'A' (65) : atlasX=1157 atlasY=1 width=38 bearingX=1 advance=39 page=0
                chr([0, 65, 65, 1157, 1, 38, 1, 39, 0]),
                // 'i' (105) : width=7 advance=12
                chr([0, 105, 105, 2809, 1, 7, 3, 12, 0]),
            ],
        };
        let fm = parse_metrics(&cfg);
        assert_eq!(fm.atlas_width, 4096);
        assert_eq!(fm.atlas_height, 2048);
        assert_eq!(fm.glyph_count(), 2);
        // Dimensions INF extraites correctement.
        assert_eq!(fm.dims.ascent, 46, "montante");
        assert_eq!(fm.dims.cell_height, 71, "hauteur cellule");
        assert_eq!(fm.dims.descent, 25, "descendante");
        let a = fm.glyph(65).unwrap();
        assert_eq!(
            (a.x, a.width, a.bearing_x, a.advance, a.page),
            (1157, 38, 1, 39, 0)
        );
        let i = fm.glyph(105).unwrap();
        assert_eq!((i.width, i.advance), (7, 12));
    }

    /// Synthétique : [`glyph_blitter`] trace correctement un pixel alpha depuis un atlas minimal.
    ///
    /// Atlas 4×4 BGRA8, un seul pixel opaque en (2, 1). Le blitter doit le retrouver
    /// et l'écrire à la bonne position du canevas.
    #[test]
    fn glyph_blitter_synthetic() {
        // Atlas 4×4 BGRA8 (64 octets), tout à zéro sauf (col=2, row=1) → A=200.
        let mut atlas = [0u8; 4 * 4 * 4];
        // Offset pixel (col=2, row=1) : (row=1, col=2) → index=(1*4+2)*4+3.
        atlas[(6) * 4 + 3] = 200;

        // Glyphe : x=2, y=1, width=1, cell_height=2.
        let metric = GlyphMetric {
            font: 0,
            base: 0,
            codepoint: 65,
            x: 2,
            y: 1,
            width: 1,
            bearing_x: 0,
            advance: 4,
            page: 0,
        };

        // Canevas 8×8 RGBA8, tout à zéro.
        let mut canvas = [0u8; 8 * 8 * 4];

        // Blitter : dst_x=0, dst_y=0, cell_height=2, couleur blanche opaque.
        glyph_blitter(
            &atlas,
            4,
            &metric,
            2,
            &mut canvas,
            8 * 4,
            0,
            0,
            [255, 255, 255, 255],
        );

        // Le blitter trace atlas(ax+col, ay+row) → canvas(dst_x+col, dst_y+row).
        // Pour row=0 : atlas_row = ay+0 = 1, canvas_row = dst_y+0 = 0.
        // Donc atlas(col=2, row=1) → canvas(col=0, row=0) = offset 0 dans le canevas.
        assert_eq!(canvas[0], 255, "R blanc (row=0, col=0)");
        assert_eq!(canvas[1], 255, "G blanc (row=0, col=0)");
        assert_eq!(canvas[2], 255, "B blanc (row=0, col=0)");
        assert_eq!(canvas[3], 200, "A=200 (row=0, col=0)");

        // La ligne 1 doit être nulle : atlas(col=2, row=2) a A=0 (non initialisé).
        assert_eq!(canvas[8 * 4 + 3], 0, "ligne 1 inchangée (atlas A=0)");
    }

    /// Synthétique : teinte semi-transparente est multipliée correctement par l'alpha atlas.
    #[test]
    fn glyph_blitter_color_alpha_modulation() {
        let mut atlas = [0u8; 4 * 4];
        atlas[3] = 200; // pixel (0,0), A=200

        let metric = GlyphMetric {
            font: 0,
            base: 0,
            codepoint: 65,
            x: 0,
            y: 0,
            width: 1,
            bearing_x: 0,
            advance: 1,
            page: 0,
        };
        let mut canvas = [0u8; 4];
        // Teinte rouge semi-transparente : color[3]=128.
        glyph_blitter(
            &atlas,
            1,
            &metric,
            1,
            &mut canvas,
            4,
            0,
            0,
            [255, 0, 0, 128],
        );

        assert_eq!(canvas[0], 255, "R");
        assert_eq!(canvas[1], 0, "G");
        assert_eq!(canvas[2], 0, "B");
        // out_a = 200 * 128 / 255 = 100 (entier).
        assert_eq!(canvas[3], 100, "A = 200*128/255");
    }

    /// Synthétique : pixels hors canevas sont ignorés sans panique.
    #[test]
    fn glyph_blitter_clip_out_of_bounds() {
        let atlas = [0u8, 0, 0, 255]; // 1×1 pixel opaque
        let metric = GlyphMetric {
            font: 0,
            base: 0,
            codepoint: 65,
            x: 0,
            y: 0,
            width: 1,
            bearing_x: 0,
            advance: 1,
            page: 0,
        };
        let mut canvas = [0u8; 4];
        // Positionner hors canevas : dst_x = -1 → ignoré.
        glyph_blitter(
            &atlas,
            1,
            &metric,
            1,
            &mut canvas,
            4,
            -1,
            0,
            [255, 255, 255, 255],
        );
        assert_eq!(canvas, [0u8; 4], "pixel hors limites ignoré");
    }

    /// Synthétique : `draw_text` avance le curseur correctement.
    #[test]
    fn draw_text_advances_cursor() {
        // Deux glyphes : 'A' (advance=10) et 'B' (advance=12).
        let cfg = CfgBinFile {
            format: crate::cfgbin::Format::T2b,
            entries: alloc::vec![
                inf([0, 5, 10, 5, 2, 20, 10, 0]),
                chr([0, 65, 65, 0, 0, 8, 0, 10, 0]),
                chr([0, 66, 66, 8, 0, 9, 0, 12, 0]),
            ],
        };
        let fm = parse_metrics(&cfg);
        // Atlas 20×10 BGRA8 (tous zéros → aucun pixel tracé, mais l'avance est quand même calculée).
        let atlas = alloc::vec![0u8; 20 * 10 * 4];
        let mut canvas = alloc::vec![0u8; 100 * 20 * 4];
        let advance = draw_text(
            &atlas,
            20,
            &fm,
            "AB",
            &mut canvas,
            100 * 4,
            0,
            5,
            [255, 255, 255, 255],
        );
        assert_eq!(advance, 10 + 12, "avance totale A+B");
    }

    /// Golden gated sur le VRAI `font.cfg.bin` + `font.g4tx` (skip si jeu absent).
    /// Valide le layout des colonnes INF ET le blit d'un glyphe réel depuis l'atlas.
    ///
    /// Pixels testés (validés sur l'atlas réel 2026-06-16) :
    /// - Glyphe `A` (U+0041) : atlas (x=1157, y=1, w=38), police 0, cell_height=71.
    /// - Ligne relative 20 (atlas row 21), colonne relative 0 : alpha = 251.
    /// - Ligne relative 19 (atlas row 20), colonne relative 34 : alpha = 47.
    #[test]
    fn real_font_metrics_match() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data = std::path::Path::new(&dir).join("data");
        if !crate::vfs::donnees_disponibles(&data) {
            eprintln!("skip real_font_metrics_match : jeu absent");
            return;
        }
        let mut vfs = crate::vfs::Vfs::new();
        if vfs.init(&data).is_err() {
            eprintln!("skip : vfs init");
            return;
        }
        let bytes = vfs
            .read("data/common/font/font/font_def/font.cfg.bin")
            .expect("lire font.cfg.bin");
        let cfg = crate::cfgbin::parse_t2b(&bytes).expect("parse T2B");
        let fm = parse_metrics(&cfg);

        assert_eq!(fm.atlas_width, 4096, "largeur atlas");
        assert_eq!(fm.atlas_height, 2048, "hauteur atlas");
        assert!(
            fm.glyph_count() > 7000,
            "trop peu de glyphes ({})",
            fm.glyph_count()
        );
        // Métriques ASCII réelles (validées par dump).
        let a = fm.glyph(65).expect("A");
        assert_eq!(
            (a.x, a.y, a.width, a.advance),
            (1157, 1, 38, 39),
            "glyphe A"
        );
        let w = fm.glyph(87).expect("W");
        assert_eq!((w.width, w.advance), (47, 48), "glyphe W");
        let i = fm.glyph(105).expect("i");
        assert_eq!((i.width, i.advance), (7, 12), "glyphe i");
        // Dimensions INF police 0 (validées : ascent=46, cell_height=71, descent=25).
        assert_eq!(fm.dims.ascent, 46, "montante police 0");
        assert_eq!(fm.dims.cell_height, 71, "hauteur cellule police 0");
        assert_eq!(fm.dims.descent, 25, "descendante police 0");
        // ASCII imprimable couvert.
        for cp in 0x21..=0x7e {
            assert!(fm.glyph(cp).is_some(), "ASCII {cp:#x} manquant");
        }
    }

    /// Golden gated : blit du glyphe `A` depuis l'atlas réel, vérifie des pixels connus.
    ///
    /// Pixels testés (mesurés sur `font.g4tx` réel, 2026-06-16) :
    /// - Canvas [20][0].alpha = 251  (atlas BGRA8 [row=21,col=1157], A=251)
    /// - Canvas [19][34].alpha = 47  (atlas BGRA8 [row=20,col=1191], A=47)
    #[test]
    fn real_glyph_blitter_a() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data = std::path::Path::new(&dir).join("data");
        if !crate::vfs::donnees_disponibles(&data) {
            eprintln!("skip real_glyph_blitter_A : jeu absent");
            return;
        }
        let mut vfs = crate::vfs::Vfs::new();
        if vfs.init(&data).is_err() {
            eprintln!("skip : vfs init");
            return;
        }

        // Charger les métriques.
        let cfg_bytes = vfs
            .read("data/common/font/font/font_def/font.cfg.bin")
            .expect("font.cfg.bin");
        let cfg = crate::cfgbin::parse_t2b(&cfg_bytes).expect("parse T2B");
        let fm = parse_metrics(&cfg);
        let a_metric = *fm.glyph(65).expect("glyphe A");
        let cell_height = fm.dims.cell_height;

        // Charger l'atlas font.g4tx.
        let g4tx_bytes = vfs
            .read("data/dx11/font/font_def/font.g4tx")
            .expect("font.g4tx");
        let g4tx = crate::g4tx::parse(&g4tx_bytes).expect("parse G4TX");
        let tex = &g4tx.textures[0];
        assert!(tex.is_dds, "atlas doit être un DDS");
        // Pixels mip0 : data_offset + 128 (DDS magic 4 + DDS_HEADER 124 = 128, sans DX10 ext).
        let pixel_start = tex.data_offset + 128;
        let atlas = g4tx_bytes
            .get(pixel_start..)
            .expect("pixels atlas hors limites");
        let atlas_w = 4096u32;

        // Canevas 200×71 RGBA8.
        let canvas_w = 200usize;
        let canvas_h = cell_height as usize;
        let mut canvas = alloc::vec![0u8; canvas_w * canvas_h * 4];

        glyph_blitter(
            atlas,
            atlas_w,
            &a_metric,
            cell_height,
            &mut canvas,
            (canvas_w * 4) as u32,
            0,
            0,
            [255, 255, 255, 255],
        );

        // Pixel [row=20, col=0] : alpha attendu = 251 (mesuré sur l'atlas réel).
        let off_20_0 = 20 * canvas_w * 4;
        assert_eq!(
            canvas[off_20_0 + 3],
            251,
            "alpha row=20 col=0 doit être 251"
        );

        // Pixel [row=19, col=34] : alpha attendu = 47.
        let off_19_34 = (19 * canvas_w + 34) * 4;
        assert_eq!(
            canvas[off_19_34 + 3],
            47,
            "alpha row=19 col=34 doit être 47"
        );

        // La zone supérieure [0..19] doit être entièrement nulle (blank rows).
        for row in 0..19 {
            for col in 0..38 {
                let off = (row * canvas_w + col) * 4;
                assert_eq!(
                    canvas[off + 3],
                    0,
                    "row={row} col={col} doit être transparent"
                );
            }
        }

        // Vérification de `draw_text` sur "A" : au moins un pixel non-nul dans le canevas.
        let mut canvas2 = alloc::vec![0u8; canvas_w * canvas_h * 4];
        let advance = draw_text(
            atlas,
            atlas_w,
            &fm,
            "A",
            &mut canvas2,
            (canvas_w * 4) as u32,
            0,
            fm.dims.ascent as i32,
            [255, 255, 255, 255],
        );
        assert_eq!(advance, 39, "avance du glyphe A");
        // draw_text calcule dst_x = pen_x + bearing_x = 0 + 1 = 1 pour 'A' (bearing_x=1).
        // Avec pen_y=46, dst_y = 46 - 46 = 0. Le pixel [row=20][col=0+1=1] doit avoir alpha=251.
        let off_draw_row20_col1 = (20 * canvas_w + 1) * 4;
        assert_eq!(
            canvas2[off_draw_row20_col1 + 3],
            251,
            "draw_text : pixel [20][1] alpha=251 (bearing_x=1)"
        );
    }
}
