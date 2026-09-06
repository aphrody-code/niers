//! Les assets de marque produits depuis l'atlas : favicons, icônes, SVG, manifeste web.
//!
//! Tout part du même endroit — l'atlas RGBA lossless du pet — plutôt que d'un fichier dessiné
//! à côté qui se périmerait. Une icône est donc toujours *le* pet, à la frame près, et se
//! régénère si l'atlas change.
//!
//! ## Pourquoi un réducteur maison
//!
//! `nie_formats::image_out` sait encoder, mais son `redimensionner_rgba` est privé, gaté
//! derrière la feature `textures`, et fait du **plus proche voisin** — c'est le bon choix pour
//! son usage (ramener des planches à un facteur entier), et le mauvais ici : réduire 192 px en
//! 32 px au plus proche voisin jette 35 pixels sur 36 et rend un bord haché. La moyenne de
//! zone ci-dessous prend la moyenne pondérée de tous les pixels source qui tombent dans le
//! pixel cible, ce qui est le filtre correct pour une réduction franche.
//!
//! L'alpha est prémultiplié pendant le calcul puis redivisé : sans cela, les pixels
//! transparents (dont la couleur est arbitraire) tirent la moyenne vers eux et cernent le
//! sujet d'un halo — le défaut classique du redimensionnement RGBA naïf.

use crate::{Error, Frame, Pet, Rect};
use std::io::Cursor;

/// Tailles produites par défaut, et à quoi chacune sert.
///
/// Ce ne sont pas des puissances de deux choisies au hasard : 180 est l'`apple-touch-icon`,
/// 192 et 512 sont les deux tailles exigées par un manifeste web installable, 32 est la
/// favicon de barre d'onglets, 16 celle des favoris.
pub const TAILLES_FAVICON: &[u32] = &[16, 32, 48, 64, 128, 180, 192, 512];

/// Une image encodée, prête à écrire.
#[derive(Debug, Clone)]
pub struct Fichier {
    /// Nom relatif, extension comprise.
    pub nom: String,
    /// Contenu encodé.
    pub octets: Vec<u8>,
}

/// Réduit une image RGBA par moyenne de zone, en travaillant en alpha prémultiplié.
///
/// Rend `None` si les dimensions ne collent pas au tampon, ou si l'une d'elles est nulle.
#[must_use]
pub fn reduire_rgba(src: &[u8], w: u32, h: u32, vers_w: u32, vers_h: u32) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || vers_w == 0 || vers_h == 0 {
        return None;
    }
    if src.len() != (w as usize) * (h as usize) * 4 {
        return None;
    }
    let mut out = vec![0u8; (vers_w as usize) * (vers_h as usize) * 4];
    let (fx, fy) = (
        f64::from(w) / f64::from(vers_w),
        f64::from(h) / f64::from(vers_h),
    );

    for y in 0..vers_h {
        // Bornes de la zone source couverte par ce pixel cible. Au moins un pixel, même
        // quand le facteur est inférieur à 1 (agrandissement) — sinon la zone est vide.
        let y0 = (f64::from(y) * fy).floor() as u32;
        let y1 = (((f64::from(y) + 1.0) * fy).ceil() as u32).clamp(y0 + 1, h);
        for x in 0..vers_w {
            let x0 = (f64::from(x) * fx).floor() as u32;
            let x1 = (((f64::from(x) + 1.0) * fx).ceil() as u32).clamp(x0 + 1, w);

            let (mut r, mut g, mut b, mut a, mut n) = (0.0f64, 0.0, 0.0, 0.0, 0.0f64);
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy as usize) * (w as usize) + sx as usize) * 4;
                    let alpha = f64::from(src[i + 3]) / 255.0;
                    // Prémultiplication : un pixel transparent ne doit pas peser sur la teinte.
                    r += f64::from(src[i]) * alpha;
                    g += f64::from(src[i + 1]) * alpha;
                    b += f64::from(src[i + 2]) * alpha;
                    a += alpha;
                    n += 1.0;
                }
            }
            let o = ((y as usize) * (vers_w as usize) + x as usize) * 4;
            if a > 0.0 {
                out[o] = (r / a).round().clamp(0.0, 255.0) as u8;
                out[o + 1] = (g / a).round().clamp(0.0, 255.0) as u8;
                out[o + 2] = (b / a).round().clamp(0.0, 255.0) as u8;
            }
            out[o + 3] = ((a / n) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    Some(out)
}

/// Encode un tampon RGBA en PNG sans perte.
///
/// # Errors
/// Rend [`Error::Invalid`] si l'encodeur PNG échoue.
pub fn encoder_png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(Cursor::new(&mut out), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc
            .write_header()
            .map_err(|e| Error::Invalid(format!("en-tête PNG : {e}")))?;
        w.write_image_data(rgba)
            .map_err(|e| Error::Invalid(format!("données PNG : {e}")))?;
    }
    Ok(out)
}

/// Assemble un `.ico` à partir d'images PNG déjà encodées.
///
/// Le format ICO accepte des PNG tels quels depuis Windows Vista, ce qui évite de réencoder en
/// BMP et de gérer son masque de transparence à part. Les tailles supérieures à 255 px se
/// codent par un `0` dans l'en-tête — une valeur littérale de 256 déborderait l'octet.
///
/// # Errors
/// Rend [`Error::Invalid`] si aucune image n'est fournie.
pub fn assembler_ico(images: &[(u32, Vec<u8>)]) -> Result<Vec<u8>, Error> {
    if images.is_empty() {
        return Err(Error::Invalid("un .ico sans image".into()));
    }
    let n = u16::try_from(images.len()).map_err(|_| Error::Invalid("trop d'images".into()))?;
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // réservé
    out.extend_from_slice(&1u16.to_le_bytes()); // type : icône
    out.extend_from_slice(&n.to_le_bytes());

    let mut offset = 6 + 16 * images.len();
    for (taille, png) in images {
        let dim = u8::try_from(*taille).unwrap_or(0); // 256 → 0, par convention du format
        out.push(dim);
        out.push(dim);
        out.push(0); // couleurs de palette
        out.push(0); // réservé
        out.extend_from_slice(&1u16.to_le_bytes()); // plans
        out.extend_from_slice(&32u16.to_le_bytes()); // bits par pixel
        out.extend_from_slice(&u32::try_from(png.len()).unwrap_or(0).to_le_bytes());
        out.extend_from_slice(&u32::try_from(offset).unwrap_or(0).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in images {
        out.extend_from_slice(png);
    }
    Ok(out)
}

/// Enveloppe un PNG dans un SVG dimensionnable.
///
/// Le sujet est un sprite : il n'y a rien à vectoriser, et une « vectorisation » automatique
/// produirait des courbes qui ne sont pas le dessin. Le SVG sert ici à ce pour quoi il est
/// utile — être posé à n'importe quelle taille dans une page, avec un rendu au pixel près
/// grâce à `image-rendering: pixelated`.
#[must_use]
pub fn svg_depuis_png(png: &[u8], w: u32, h: u32, titre: &str) -> String {
    let b64 = base64(png);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img" aria-label="{titre}">
  <title>{titre}</title>
  <image href="data:image/png;base64,{b64}" width="{w}" height="{h}" style="image-rendering: pixelated"/>
</svg>
"#
    )
}

/// Encodage base64 standard, sans dépendance.
fn base64(src: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(src.len().div_ceil(3) * 4);
    for bloc in src.chunks(3) {
        let (b0, b1, b2) = (
            u32::from(bloc[0]),
            bloc.get(1).copied().map_or(0, u32::from),
            bloc.get(2).copied().map_or(0, u32::from),
        );
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if bloc.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if bloc.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

impl Pet {
    /// Extrait une frame **cadrée sur ses pixels visibles**, en carré.
    ///
    /// Les bornes alpha valent mieux que la cellule : une cellule de 192 × 208 est surtout du
    /// vide, et une favicon tirée de la cellule entière montre un pet minuscule au milieu de
    /// rien. Le carré est centré sur les bornes et étendu au plus grand côté, pour ne pas
    /// déformer le sujet.
    ///
    /// # Errors
    /// Rend [`Error::Invalid`] si la frame ne porte pas de bornes alpha ou sort de l'atlas.
    pub fn vignette_carree(&self, frame: &Frame) -> Result<(Vec<u8>, u32), Error> {
        let b = &frame.alpha_bounds_in_atlas;
        if b.width == 0 || b.height == 0 {
            return Err(Error::Invalid(format!(
                "frame {} sans pixel visible : rien a cadrer",
                frame.index
            )));
        }
        let cote = b.width.max(b.height);
        let cx = b.x + b.width / 2;
        let cy = b.y + b.height / 2;
        let largeur = self.manifest.atlas.width;
        let hauteur = self.manifest.atlas.height;
        // Recentrer sans jamais sortir de l'atlas : un carré qui déborde produirait un crop
        // vide, et `crop_rgba` rendrait `None` sans dire pourquoi.
        let x = cx
            .saturating_sub(cote / 2)
            .min(largeur.saturating_sub(cote));
        let y = cy
            .saturating_sub(cote / 2)
            .min(hauteur.saturating_sub(cote));
        let rect = Rect {
            x,
            y,
            width: cote,
            height: cote,
        };
        let rgba = crate::crop_rgba(&self.rgba, largeur, hauteur, rect)
            .ok_or_else(|| Error::Invalid(format!("carré {cote}px hors de l'atlas")))?;
        Ok((rgba, cote))
    }

    /// Produit le jeu complet d'assets de marque depuis une frame.
    ///
    /// Rend les PNG de chaque taille, le `.ico` multi-résolutions, le SVG et le manifeste web.
    ///
    /// # Errors
    /// Rend [`Error::Invalid`] si la frame est inutilisable ou si un encodage échoue.
    pub fn assets_de_marque(&self, frame: &Frame, tailles: &[u32]) -> Result<Vec<Fichier>, Error> {
        let (source, cote) = self.vignette_carree(frame)?;
        let mut fichiers = Vec::new();
        let mut pour_ico = Vec::new();

        for &t in tailles {
            let rgba = reduire_rgba(&source, cote, cote, t, t)
                .ok_or_else(|| Error::Invalid(format!("réduction impossible vers {t}px")))?;
            let png = encoder_png(&rgba, t, t)?;
            // Un .ico au-delà de 64 px n'apporte rien : Windows n'y puise que les petites
            // tailles, et chaque entrée alourdit un fichier chargé à chaque onglet.
            if t <= 64 {
                pour_ico.push((t, png.clone()));
            }
            fichiers.push(Fichier {
                nom: format!("icone-{t}.png"),
                octets: png,
            });
        }

        // Le SVG n'embarque PAS la plus grande taille. Un favicon SVG doit rester léger : il
        // est chargé à chaque page, et la 512 pesait 138 Ko une fois en base64 (qui gonfle de
        // 33 %) — plus lourd que toutes les autres icônes réunies. La 128 suffit largement,
        // `image-rendering: pixelated` faisant le reste à l'affichage.
        const TAILLE_SVG: u32 = 128;
        let taille_svg = tailles
            .iter()
            .copied()
            .filter(|t| *t <= TAILLE_SVG)
            .max()
            .or_else(|| tailles.iter().copied().min())
            .ok_or_else(|| Error::Invalid("aucune taille demandée".into()))?;
        let png_svg = fichiers
            .iter()
            .find(|f| f.nom == format!("icone-{taille_svg}.png"))
            .ok_or_else(|| Error::Invalid("icône du SVG introuvable".into()))?
            .octets
            .clone();
        fichiers.push(Fichier {
            nom: "favicon.ico".into(),
            octets: assembler_ico(&pour_ico)?,
        });
        fichiers.push(Fichier {
            nom: "icone.svg".into(),
            octets: svg_depuis_png(&png_svg, taille_svg, taille_svg, &self.pet.display_name)
                .into_bytes(),
        });
        fichiers.push(Fichier {
            nom: "site.webmanifest".into(),
            octets: self.manifeste_web(tailles).into_bytes(),
        });
        Ok(fichiers)
    }

    /// Le manifeste web, avec les icônes que [`Pet::assets_de_marque`] vient de produire.
    #[must_use]
    pub fn manifeste_web(&self, tailles: &[u32]) -> String {
        let icones: Vec<String> = tailles
            .iter()
            .map(|t| {
                let usage = if *t >= 192 { r#", "purpose": "any maskable""# } else { "" };
                format!(
                    r#"    {{ "src": "/icone-{t}.png", "sizes": "{t}x{t}", "type": "image/png"{usage} }}"#
                )
            })
            .collect();
        format!(
            r##"{{
  "name": "{nom}",
  "short_name": "{nom}",
  "description": "{desc}",
  "icons": [
{icones}
  ],
  "display": "standalone",
  "background_color": "#0b0b0f",
  "theme_color": "#0b0b0f"
}}
"##,
            nom = self.pet.display_name,
            desc = self.pet.description.replace('"', "'"),
            icones = icones.join(",\n"),
        )
    }
}
