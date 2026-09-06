//! Mesure d'une image, comparaison de deux images, rastérisation d'un SVG.
//!
//! Le socle de la skill `pixel-perfect` : reproduire une image du jeu commence par la
//! **mesurer**, et se termine par une **comparaison chiffrée**. Toute valeur écrite dans du code
//! — couleur, rayon, épaisseur — doit se rattacher à une sortie de ce module ; ce qui ne s'y
//! rattache pas vient du souvenir.
//!
//! Trois choix, et leur raison :
//!
//! - le k-means est **maison et travaille en Oklab** (par `palette`) plutôt qu'en sRGB : deux
//!   couleurs à distance euclidienne égale en sRGB ne sont pas également différentes à l'œil,
//!   et une palette calculée en sRGB fusionne les tons sombres tout en éclatant les clairs ;
//! - la comparaison passe par `image-compare` (MIT) et **jamais** par `dssim`, qui est AGPL et
//!   contaminerait tout binaire distribué ;
//! - la vectorisation ne tire **aucune** dépendance : suivre le bord d'un masque puis simplifier
//!   tient en deux fonctions, et rend des chemins lisibles là où un traceur générique en rend des
//!   milliers. Elle reste un **décalque** — bonne pour un logo plat, jamais pour prétendre
//!   produire un SVG conçu comme vectoriel.

use crate::Error;
#[cfg(feature = "fs")]
use std::path::Path;

/// Une couleur de la palette mesurée, dans les trois espaces qui servent à décider.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Couleur {
    /// Part des pixels retenus qui tombent dans cette classe, en pourcentage.
    pub part_pct: f64,
    /// Forme `#RRGGBB`, à recopier telle quelle dans une feuille de style.
    pub hex: String,
    /// Teinte (0-360), saturation et luminosité HSL (0-1) — l'espace des dégradés d'aplat.
    pub hsl: [f64; 3],
    /// Luminance, chroma et teinte Oklch — l'espace où juger si deux couleurs sont proches.
    pub oklch: [f64; 3],
}

/// La boîte englobante du sujet, en pixels, bornes incluses.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Boite {
    /// Abscisse du bord gauche.
    pub x0: u32,
    /// Ordonnée du bord haut.
    pub y0: u32,
    /// Abscisse du bord droit, incluse.
    pub x1: u32,
    /// Ordonnée du bord bas, incluse.
    pub y1: u32,
}

impl Boite {
    /// Largeur de la boîte, bornes incluses.
    #[must_use]
    pub const fn largeur(&self) -> u32 {
        self.x1 - self.x0 + 1
    }
    /// Hauteur de la boîte, bornes incluses.
    #[must_use]
    pub const fn hauteur(&self) -> u32 {
        self.y1 - self.y0 + 1
    }
}

/// Ce qu'une mesure rend, et ce que chaque grandeur sert à décider.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Mesure {
    /// Dimensions de l'image source.
    pub source: [u32; 2],
    /// Boîte englobante du sujet.
    pub boite: Boite,
    /// Largeur / hauteur de la boîte. 1,0 = la forme tient dans un carré.
    pub ratio: f64,
    /// Part de la boîte réellement occupée. ≈ 78,5 % (π/4) pour un disque plein, ≈ 100 % pour un
    /// rectangle. Une valeur bien plus basse dit une silhouette ajourée — ou un masque trop large.
    pub remplissage_pct: f64,
    /// Part de l'image entière occupée par le sujet.
    pub part_image_pct: f64,
    /// Palette k-means en Oklab, triée par part décroissante.
    pub palette: Vec<Couleur>,
    /// Épaisseur médiane du trait, en pourcentage de la largeur de la forme.
    ///
    /// Un contour d'encre tient entre 0,5 % et 1,5 %. Au-delà, la segmentation a attrapé un
    /// aplat et non un cerne : resserrer la boîte, ne pas croire le chiffre.
    pub trait_pct: f64,
    /// Nombre de pixels pleins par colonne, normalisé sur la hauteur de la boîte — le profil de
    /// silhouette, où se lisent les creux à poser avant de bomber entre eux.
    pub profil_colonnes: Vec<f64>,
    /// Idem par ligne.
    pub profil_lignes: Vec<f64>,
    /// Abscisse du premier pixel plein de chaque ligne de la boîte, quand la ligne en a un.
    pub bord_gauche: Vec<Option<u32>>,
    /// Abscisse du dernier pixel plein de chaque ligne.
    pub bord_droit: Vec<Option<u32>>,
    /// Pente du bord gauche, en dx/dy, ajustée aux moindres carrés — et son angle par rapport à
    /// la verticale. C'est **la** constante d'une DA en parallélogrammes : elle se traduit
    /// directement en `transform: skewX(-angle)`. Ne rien en conclure si `bord_droiture` est bas.
    pub pente_gauche: f64,
    /// Angle du bord gauche par rapport à la verticale, en degrés (positif = penché à droite).
    pub angle_gauche_deg: f64,
    /// Pente du bord droit, mêmes conventions.
    pub pente_droite: f64,
    /// Angle du bord droit par rapport à la verticale, en degrés.
    pub angle_droit_deg: f64,
    /// R² de l'ajustement du bord gauche, 0 à 1.
    pub droiture_gauche: f64,
    /// R² de l'ajustement du bord droit, 0 à 1.
    pub droiture_droite: f64,
    /// Le pire des deux R², celui qui décide si l'angle veut dire quelque chose.
    ///
    /// Un bord réellement droit rend > 0,99. En dessous de 0,95, la forme n'est pas un
    /// parallélogramme (ou la boîte contient plusieurs objets) et l'angle ci-dessus ne veut rien
    /// dire — c'est le garde-fou qui évite d'écrire un `skewX` inventé dans une feuille de style.
    pub bord_droiture: f64,
}

/// Comment décider qu'un pixel appartient au sujet.
#[derive(Debug, Clone, Copy)]
pub enum Masque {
    /// Le canal alpha dépasse le seuil (0-255). Le bon choix pour un asset détouré.
    Alpha(u8),
    /// La luminance est **inférieure** au seuil (0-255) : l'encre sur un fond clair.
    Sombre(u8),
    /// La teinte est dans `[min, max]` degrés et la saturation dépasse `sat` (0-1).
    Teinte {
        /// Borne basse de teinte, en degrés.
        min: f64,
        /// Borne haute de teinte, en degrés.
        max: f64,
        /// Saturation minimale, 0-1.
        sat: f64,
    },
}

/// Les réglages d'une mesure.
#[derive(Debug, Clone, Copy)]
pub struct Reglages {
    /// Comment isoler le sujet.
    pub masque: Masque,
    /// Sous-rectangle à mesurer, ou toute l'image.
    pub boite: Option<Boite>,
    /// Nombre de classes du k-means.
    pub k: usize,
}

impl Default for Reglages {
    fn default() -> Self {
        Self {
            masque: Masque::Alpha(8),
            boite: None,
            k: 5,
        }
    }
}

/// Une image RGBA en mémoire — le seul format que ce module manipule.
#[derive(Debug, Clone)]
pub struct Image {
    /// Largeur en pixels.
    pub largeur: u32,
    /// Hauteur en pixels.
    pub hauteur: u32,
    /// Pixels RGBA8, ligne par ligne.
    pub rgba: Vec<u8>,
}

impl Image {
    /// Construit depuis un tampon RGBA, en vérifiant que sa taille colle aux dimensions.
    ///
    /// # Erreurs
    /// Si le tampon ne fait pas exactement `largeur * hauteur * 4` octets.
    pub fn nouvelle(largeur: u32, hauteur: u32, rgba: Vec<u8>) -> Result<Self, Error> {
        let attendu = (largeur as usize) * (hauteur as usize) * 4;
        if rgba.len() != attendu {
            return Err(Error::Invalid(format!(
                "tampon RGBA de {} octets pour {largeur}x{hauteur} (attendu {attendu})",
                rgba.len()
            )));
        }
        Ok(Self {
            largeur,
            hauteur,
            rgba,
        })
    }

    /// Décode une image en mémoire (PNG, JPEG, WebP) et la ramène en RGBA8.
    ///
    /// C'est **le** point d'entrée portable : un navigateur ou un mobile n'a pas de système de
    /// fichiers à offrir, mais toujours des octets. [`Image::charger`] n'en est que la variante
    /// de commodité pour un hôte qui a un disque.
    ///
    /// # Erreurs
    /// Si le format n'est pas reconnu ou les données sont corrompues.
    pub fn depuis_octets(octets: &[u8]) -> Result<Self, Error> {
        let img = image::load_from_memory(octets)
            .map_err(|e| Error::Invalid(format!("image illisible : {e}")))?
            .to_rgba8();
        let (largeur, hauteur) = img.dimensions();
        Ok(Self {
            largeur,
            hauteur,
            rgba: img.into_raw(),
        })
    }

    /// Charge un fichier image (PNG, JPEG, WebP) et le ramène en RGBA8.
    ///
    /// Réservé aux hôtes qui ont un disque : sur le web, passer par [`Image::depuis_octets`].
    ///
    /// # Erreurs
    /// Si le fichier est illisible ou son format non reconnu.
    #[cfg(feature = "fs")]
    pub fn charger(chemin: &Path) -> Result<Self, Error> {
        let img = image::open(chemin)
            .map_err(|e| Error::Invalid(format!("{} : {e}", chemin.display())))?
            .to_rgba8();
        let (largeur, hauteur) = img.dimensions();
        Ok(Self {
            largeur,
            hauteur,
            rgba: img.into_raw(),
        })
    }

    /// Le pixel en `(x, y)`, sans vérification de bornes par l'appelant.
    #[must_use]
    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y as usize) * (self.largeur as usize) + (x as usize)) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

/// Convertit un sRGB 8 bits en Oklab (L, a, b).
fn oklab(rgb: [u8; 3]) -> [f64; 3] {
    use palette::{FromColor, Oklab, Srgb};
    let src = Srgb::new(rgb[0], rgb[1], rgb[2]).into_format::<f32>();
    let lab = Oklab::from_color(src);
    [f64::from(lab.l), f64::from(lab.a), f64::from(lab.b)]
}

/// Convertit un sRGB 8 bits en Oklch (L, C, h°).
fn oklch(rgb: [u8; 3]) -> [f64; 3] {
    let [l, a, b] = oklab(rgb);
    let c = a.hypot(b);
    let mut h = b.atan2(a).to_degrees();
    if h < 0.0 {
        h += 360.0;
    }
    [l, c, h]
}

/// Convertit un sRGB 8 bits en HSL (h°, s, l).
fn hsl(rgb: [u8; 3]) -> [f64; 3] {
    let (r, g, b) = (
        f64::from(rgb[0]) / 255.0,
        f64::from(rgb[1]) / 255.0,
        f64::from(rgb[2]) / 255.0,
    );
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = f64::midpoint(max, min);
    let d = max - min;
    if d.abs() < f64::EPSILON {
        return [0.0, 0.0, l];
    }
    let s = d / (1.0 - (2.0f64.mul_add(l, -1.0)).abs());
    let h = if (max - r).abs() < f64::EPSILON {
        60.0 * (((g - b) / d) % 6.0)
    } else if (max - g).abs() < f64::EPSILON {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    [if h < 0.0 { h + 360.0 } else { h }, s, l]
}

/// Un pixel appartient-il au sujet ?
fn retenu(px: [u8; 4], masque: Masque) -> bool {
    if px[3] == 0 {
        return false;
    }
    match masque {
        Masque::Alpha(seuil) => px[3] > seuil,
        Masque::Sombre(seuil) => {
            let lum = 0.0722f64.mul_add(
                f64::from(px[2]),
                0.2126f64.mul_add(f64::from(px[0]), 0.7152 * f64::from(px[1])),
            );
            lum < f64::from(seuil)
        }
        Masque::Teinte { min, max, sat } => {
            let [h, s, _] = hsl([px[0], px[1], px[2]]);
            s >= sat && h >= min && h <= max
        }
    }
}

/// k-means en Oklab sur les pixels retenus, initialisé par k-means++ déterministe.
///
/// Déterministe **volontairement** : une mesure qui change d'une exécution à l'autre ne peut pas
/// servir de référence dans un document d'analyse. Le premier centre est le pixel médian, les
/// suivants sont les plus éloignés des centres déjà posés — pas de tirage aléatoire.
fn kmeans(echantillons: &[[u8; 3]], k: usize) -> Vec<(usize, [u8; 3])> {
    if echantillons.is_empty() || k == 0 {
        return Vec::new();
    }
    let labs: Vec<[f64; 3]> = echantillons.iter().map(|c| oklab(*c)).collect();
    let k = k.min(labs.len());

    let mut centres: Vec<[f64; 3]> = vec![labs[labs.len() / 2]];
    while centres.len() < k {
        let (mut meilleur, mut dmax) = (0usize, -1.0f64);
        for (i, l) in labs.iter().enumerate() {
            let d = centres
                .iter()
                .map(|c| distance2(*l, *c))
                .fold(f64::MAX, f64::min);
            if d > dmax {
                dmax = d;
                meilleur = i;
            }
        }
        centres.push(labs[meilleur]);
    }

    let mut classes = vec![0usize; labs.len()];
    for _ in 0..24 {
        let mut bouge = false;
        for (i, l) in labs.iter().enumerate() {
            let mut best = 0usize;
            let mut bd = f64::MAX;
            for (j, c) in centres.iter().enumerate() {
                let d = distance2(*l, *c);
                if d < bd {
                    bd = d;
                    best = j;
                }
            }
            if classes[i] != best {
                classes[i] = best;
                bouge = true;
            }
        }
        let mut somme = vec![[0.0f64; 3]; centres.len()];
        let mut compte = vec![0usize; centres.len()];
        for (i, l) in labs.iter().enumerate() {
            let s = &mut somme[classes[i]];
            for d in 0..3 {
                s[d] += l[d];
            }
            compte[classes[i]] += 1;
        }
        for (j, c) in centres.iter_mut().enumerate() {
            if compte[j] > 0 {
                #[expect(clippy::cast_precision_loss, reason = "compte tient largement en f64")]
                let n = compte[j] as f64;
                for d in 0..3 {
                    c[d] = somme[j][d] / n;
                }
            }
        }
        if !bouge {
            break;
        }
    }

    // Représentant = le pixel réel le plus proche du centre, jamais la moyenne reconvertie : une
    // moyenne d'Oklab peut retomber hors du gamut sRGB et rendre un HEX qui n'existe pas dans
    // l'image.
    let mut sortie = Vec::with_capacity(centres.len());
    for (j, c) in centres.iter().enumerate() {
        let mut compte = 0usize;
        let mut rep = echantillons[0];
        let mut bd = f64::MAX;
        for (i, l) in labs.iter().enumerate() {
            if classes[i] != j {
                continue;
            }
            compte += 1;
            let d = distance2(*l, *c);
            if d < bd {
                bd = d;
                rep = echantillons[i];
            }
        }
        if compte > 0 {
            sortie.push((compte, rep));
        }
    }
    sortie.sort_unstable_by_key(|(compte, _)| std::cmp::Reverse(*compte));
    sortie
}

fn distance2(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).mul_add(
        a[0] - b[0],
        (a[1] - b[1]).mul_add(a[1] - b[1], (a[2] - b[2]).powi(2)),
    )
}

/// Épaisseur médiane des segments pleins, ligne à ligne — la mesure du trait.
fn epaisseur_mediane(plein: &[bool], w: usize, h: usize) -> f64 {
    let mut runs: Vec<usize> = Vec::new();
    for y in 0..h {
        let mut courant = 0usize;
        for x in 0..w {
            if plein[y * w + x] {
                courant += 1;
            } else if courant > 0 {
                runs.push(courant);
                courant = 0;
            }
        }
        if courant > 0 {
            runs.push(courant);
        }
    }
    if runs.is_empty() {
        return 0.0;
    }
    runs.sort_unstable();
    #[expect(clippy::cast_precision_loss, reason = "longueurs de segments, petites")]
    let med = runs[runs.len() / 2] as f64;
    med
}

/// Mesure une image selon des réglages.
///
/// # Erreurs
/// Si la boîte demandée sort de l'image, ou si aucun pixel n'est retenu par le masque — c'est
/// alors le masque qu'il faut corriger, pas l'image.
pub fn mesurer(img: &Image, reglages: Reglages) -> Result<Mesure, Error> {
    let zone = reglages.boite.unwrap_or(Boite {
        x0: 0,
        y0: 0,
        x1: img.largeur.saturating_sub(1),
        y1: img.hauteur.saturating_sub(1),
    });
    if zone.x1 >= img.largeur || zone.y1 >= img.hauteur || zone.x0 > zone.x1 || zone.y0 > zone.y1 {
        return Err(Error::Invalid(format!(
            "boîte {zone:?} hors de l'image {}x{}",
            img.largeur, img.hauteur
        )));
    }

    let (zw, zh) = (zone.largeur() as usize, zone.hauteur() as usize);
    let mut plein = vec![false; zw * zh];
    let mut echantillons: Vec<[u8; 3]> = Vec::new();
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);

    for y in zone.y0..=zone.y1 {
        for x in zone.x0..=zone.x1 {
            let px = img.pixel(x, y);
            if !retenu(px, reglages.masque) {
                continue;
            }
            plein[((y - zone.y0) as usize) * zw + ((x - zone.x0) as usize)] = true;
            echantillons.push([px[0], px[1], px[2]]);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }

    if echantillons.is_empty() {
        return Err(Error::Invalid(
            "aucun pixel retenu : c'est le masque qu'il faut corriger, pas l'image".into(),
        ));
    }

    let boite = Boite { x0, y0, x1, y1 };
    let (bw, bh) = (f64::from(boite.largeur()), f64::from(boite.hauteur()));
    #[expect(
        clippy::cast_precision_loss,
        reason = "compte de pixels, bien sous 2^53"
    )]
    let pleins = echantillons.len() as f64;

    let palette = kmeans(&echantillons, reglages.k)
        .into_iter()
        .map(|(compte, rgb)| {
            #[expect(clippy::cast_precision_loss, reason = "compte de pixels")]
            let part = (compte as f64) * 100.0 / pleins;
            Couleur {
                part_pct: part,
                hex: format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
                hsl: hsl(rgb),
                oklch: oklch(rgb),
            }
        })
        .collect();

    let mut profil_colonnes = vec![0.0f64; boite.largeur() as usize];
    let mut profil_lignes = vec![0.0f64; boite.hauteur() as usize];
    for y in boite.y0..=boite.y1 {
        for x in boite.x0..=boite.x1 {
            if plein[((y - zone.y0) as usize) * zw + ((x - zone.x0) as usize)] {
                profil_colonnes[(x - boite.x0) as usize] += 1.0;
                profil_lignes[(y - boite.y0) as usize] += 1.0;
            }
        }
    }
    for v in &mut profil_colonnes {
        *v /= bh;
    }
    for v in &mut profil_lignes {
        *v /= bw;
    }

    // Bords et pente. Le premier/dernier pixel plein de chaque ligne, puis un ajustement aux
    // moindres carrés x = a·y + b sur chaque bord — l'angle de la DA se lit là, et nulle part
    // dans une capture regardée à l'œil.
    let mut bord_gauche: Vec<Option<u32>> = Vec::with_capacity(boite.hauteur() as usize);
    let mut bord_droit: Vec<Option<u32>> = Vec::with_capacity(boite.hauteur() as usize);
    for y in boite.y0..=boite.y1 {
        let ligne: Vec<u32> = (boite.x0..=boite.x1)
            .filter(|x| plein[((y - zone.y0) as usize) * zw + ((x - zone.x0) as usize)])
            .collect();
        bord_gauche.push(ligne.first().copied());
        bord_droit.push(ligne.last().copied());
    }
    let (pente_gauche, r2g) = ajuster(&bord_gauche, boite.y0);
    let (pente_droite, r2d) = ajuster(&bord_droit, boite.y0);

    let trait_px = epaisseur_mediane(&plein, zw, zh);

    Ok(Mesure {
        source: [img.largeur, img.hauteur],
        boite,
        ratio: bw / bh,
        remplissage_pct: pleins * 100.0 / (bw * bh),
        part_image_pct: pleins * 100.0 / (f64::from(img.largeur) * f64::from(img.hauteur)),
        palette,
        trait_pct: trait_px * 100.0 / bw,
        profil_colonnes,
        profil_lignes,
        bord_gauche,
        bord_droit,
        pente_gauche,
        angle_gauche_deg: pente_gauche.atan().to_degrees(),
        pente_droite,
        angle_droit_deg: pente_droite.atan().to_degrees(),
        droiture_gauche: r2g,
        droiture_droite: r2d,
        bord_droiture: r2g.min(r2d),
    })
}

/// Ajuste `x = a·y + b` aux moindres carrés sur un bord, et rend `(a, R²)`.
///
/// Rend `(0, 0)` s'il y a moins de trois points : une pente sur deux points est un trait tiré au
/// hasard, pas une mesure.
fn ajuster(bord: &[Option<u32>], y0: u32) -> (f64, f64) {
    let points: Vec<(f64, f64)> = bord
        .iter()
        .enumerate()
        .filter_map(|(i, x)| {
            x.map(|x| {
                #[expect(clippy::cast_precision_loss, reason = "index de ligne, petit")]
                let y = (i as f64) + f64::from(y0);
                (y, f64::from(x))
            })
        })
        .collect();
    if points.len() < 3 {
        return (0.0, 0.0);
    }
    #[expect(clippy::cast_precision_loss, reason = "nombre de lignes, petit")]
    let n = points.len() as f64;
    let my = points.iter().map(|p| p.0).sum::<f64>() / n;
    let mx = points.iter().map(|p| p.1).sum::<f64>() / n;
    let mut sxy = 0.0;
    let mut syy = 0.0;
    for (y, x) in &points {
        sxy += (y - my) * (x - mx);
        syy += (y - my) * (y - my);
    }
    if syy.abs() < f64::EPSILON {
        return (0.0, 0.0);
    }
    let a = sxy / syy;
    let b = a.mul_add(-my, mx);
    let (mut res, mut tot) = (0.0, 0.0);
    for (y, x) in &points {
        res += (x - a.mul_add(*y, b)).powi(2);
        tot += (x - mx).powi(2);
    }
    let r2 = if tot.abs() < f64::EPSILON {
        1.0
    } else {
        (1.0 - res / tot).max(0.0)
    };
    (a, r2)
}

/// Le verdict d'une comparaison entre deux images.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Comparaison {
    /// Similarité structurelle hybride RGB, 0 à 1. 1 = identique.
    pub ssim: f64,
    /// Part des pixels dont chaque canal tient dans la tolérance demandée.
    pub pixels_dans_tolerance_pct: f64,
    /// Tolérance appliquée, par canal, en niveaux 0-255.
    pub tolerance: u8,
    /// Vrai si les deux tampons RGBA sont rigoureusement égaux.
    pub identique: bool,
}

/// Compare deux images de mêmes dimensions.
///
/// Rend le SSIM **et** la part de pixels dans la tolérance : ce ne sont pas les mêmes critères.
/// Le SSIM juge une reproduction d'interface, la tolérance juge un rendu qui doit être identique
/// (c'est le critère de `nie-game --verify`, qui échoue sous 99 %).
///
/// # Erreurs
/// Si les dimensions diffèrent, ou si le calcul de similarité échoue.
pub fn comparer(a: &Image, b: &Image, tolerance: u8) -> Result<Comparaison, Error> {
    if a.largeur != b.largeur || a.hauteur != b.hauteur {
        return Err(Error::Invalid(format!(
            "dimensions différentes : {}x{} contre {}x{}",
            a.largeur, a.hauteur, b.largeur, b.hauteur
        )));
    }
    let mut dedans = 0usize;
    for (pa, pb) in a.rgba.chunks_exact(4).zip(b.rgba.chunks_exact(4)) {
        if pa.iter().zip(pb).all(|(x, y)| x.abs_diff(*y) <= tolerance) {
            dedans += 1;
        }
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "compte de pixels, bien sous 2^53"
    )]
    let pct = (dedans as f64) * 100.0 / ((a.rgba.len() / 4) as f64);

    let vers_rgb = |img: &Image| {
        let plat: Vec<u8> = img
            .rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        image::RgbImage::from_raw(img.largeur, img.hauteur, plat)
            .ok_or_else(|| Error::Invalid("tampon RGB incohérent".into()))
    };
    let ssim = image_compare::rgb_hybrid_compare(&vers_rgb(a)?, &vers_rgb(b)?)
        .map_err(|e| Error::Invalid(format!("comparaison impossible : {e}")))?
        .score;

    Ok(Comparaison {
        ssim,
        pixels_dans_tolerance_pct: pct,
        tolerance,
        identique: a.rgba == b.rgba,
    })
}

/// Rastérise un SVG à la largeur demandée, hauteur déduite du `viewBox`.
///
/// C'est ce qui permet de **regarder** un SVG produit : une géométrie qui tient à 512 px se ferme
/// souvent en dessous de 64 px, et cela ne se voit que sur la planche rendue.
///
/// # Erreurs
/// Si le SVG est invalide, ou si le tampon de sortie ne peut être alloué.
pub fn rasteriser_svg(svg: &str, largeur: u32) -> Result<Image, Error> {
    let arbre = usvg::Tree::from_str(svg, &usvg::Options::default())
        .map_err(|e| Error::Invalid(format!("SVG invalide : {e}")))?;
    let taille = arbre.size();
    if taille.width() <= 0.0 || largeur == 0 {
        return Err(Error::Invalid("SVG sans dimensions exploitables".into()));
    }
    let echelle = f32::from(u16::try_from(largeur.min(u32::from(u16::MAX))).unwrap_or(u16::MAX))
        / taille.width();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "hauteur > 0"
    )]
    let hauteur = (taille.height() * echelle).ceil().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(largeur, hauteur)
        .ok_or_else(|| Error::Invalid(format!("pixmap {largeur}x{hauteur} impossible")))?;
    resvg::render(
        &arbre,
        tiny_skia::Transform::from_scale(echelle, echelle),
        &mut pixmap.as_mut(),
    );
    Image::nouvelle(largeur, hauteur, pixmap.take())
}

// ---------------------------------------------------------------------------------------------
// Vectorisation
// ---------------------------------------------------------------------------------------------

/// Réglages de la vectorisation.
#[derive(Debug, Clone, Copy)]
pub struct ReglagesVecteur {
    /// Nombre de couches de couleur. 1 = silhouette monochrome.
    pub k: usize,
    /// Tolérance de simplification, en pixels. 0 garde l'escalier de la grille ; 0,75 efface le
    /// crénelage sans déplacer les angles ; au-delà de 2 la forme commence à mentir.
    pub tolerance: f64,
    /// Ignorer les composantes plus petites que ce nombre de pixels — le bruit de bord.
    pub aire_min: usize,
    /// Comment isoler le sujet du fond.
    pub masque: Masque,
}

impl Default for ReglagesVecteur {
    fn default() -> Self {
        Self {
            k: 4,
            tolerance: 0.75,
            aire_min: 12,
            masque: Masque::Alpha(8),
        }
    }
}

/// Suit le bord d'un masque booléen et rend ses contours fermés, en coordonnées de coins.
///
/// Chaque pixel plein dont le voisin est vide donne une arête orientée de façon que le plein soit
/// à gauche ; les arêtes se chaînent en boucles. Les trous ressortent donc avec l'enroulement
/// inverse, ce qui les creuse tout seuls sous la règle `nonzero` — inutile de les détecter.
fn contours(masque: &[bool], w: usize, h: usize) -> Vec<Vec<(f64, f64)>> {
    let plein = |x: isize, y: isize| -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < w
            && (y as usize) < h
            && masque[(y as usize) * w + (x as usize)]
    };
    let mut aretes: std::collections::HashMap<(i64, i64), Vec<(i64, i64)>> =
        std::collections::HashMap::new();
    for y in 0..h {
        for x in 0..w {
            if !masque[y * w + x] {
                continue;
            }
            let (xi, yi) = (x as isize, y as isize);
            let (x0, y0, x1, y1) = (x as i64, y as i64, x as i64 + 1, y as i64 + 1);
            if !plein(xi, yi - 1) {
                aretes.entry((x0, y0)).or_default().push((x1, y0));
            }
            if !plein(xi + 1, yi) {
                aretes.entry((x1, y0)).or_default().push((x1, y1));
            }
            if !plein(xi, yi + 1) {
                aretes.entry((x1, y1)).or_default().push((x0, y1));
            }
            if !plein(xi - 1, yi) {
                aretes.entry((x0, y1)).or_default().push((x0, y0));
            }
        }
    }

    let mut boucles = Vec::new();
    let mut departs: Vec<(i64, i64)> = aretes.keys().copied().collect();
    departs.sort_unstable();
    for depart in departs {
        while aretes.get(&depart).is_some_and(|v| !v.is_empty()) {
            let mut boucle = Vec::new();
            let mut point = depart;
            while let Some(suivants) = aretes.get_mut(&point) {
                let Some(suivant) = suivants.pop() else { break };
                #[expect(clippy::cast_precision_loss, reason = "coordonnées de pixels")]
                boucle.push((point.0 as f64, point.1 as f64));
                point = suivant;
                if point == depart {
                    break;
                }
            }
            if boucle.len() >= 4 {
                boucles.push(boucle);
            }
        }
    }
    boucles
}

/// Simplification de Douglas-Peucker sur une boucle fermée.
fn simplifier(points: &[(f64, f64)], tolerance: f64) -> Vec<(f64, f64)> {
    if tolerance <= 0.0 || points.len() < 3 {
        return points.to_vec();
    }
    fn recurse(pts: &[(f64, f64)], tol: f64, out: &mut Vec<(f64, f64)>) {
        let (Some(a), Some(b)) = (pts.first(), pts.last()) else {
            return;
        };
        let mut dmax = 0.0f64;
        let mut idx = 0usize;
        for (i, p) in pts
            .iter()
            .enumerate()
            .take(pts.len().saturating_sub(1))
            .skip(1)
        {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let norme = dx.hypot(dy);
            let d = if norme < f64::EPSILON {
                (p.0 - a.0).hypot(p.1 - a.1)
            } else {
                (dy.mul_add(p.0 - a.0, -(dx * (p.1 - a.1)))).abs() / norme
            };
            if d > dmax {
                dmax = d;
                idx = i;
            }
        }
        if dmax > tol && idx > 0 {
            recurse(&pts[..=idx], tol, out);
            out.pop();
            recurse(&pts[idx..], tol, out);
        } else {
            out.push(*a);
            out.push(*b);
        }
    }
    let mut ferme = points.to_vec();
    ferme.push(points[0]);
    let mut out = Vec::new();
    recurse(&ferme, tolerance, &mut out);
    out.pop();
    out
}

/// Écrit une boucle en données de `path`, arrondies au centième.
fn path_de_boucle(boucle: &[(f64, f64)]) -> String {
    let mut d = String::new();
    for (i, (x, y)) in boucle.iter().enumerate() {
        let commande = if i == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{commande}{:.2} {:.2}", x, y));
        d.push(' ');
    }
    d.push('Z');
    d
}

/// Vectorise une image en SVG : une couche de `path` par couleur, du fond vers le sujet.
///
/// **C'est un décalque.** Le tracé suit la grille de pixels : il rend un logo plat très
/// correctement, et un dégradé en escalier. Pour un dessin conçu comme vectoriel, écrire la
/// géométrie à la main donne un fichier plus léger, éditable et juste.
///
/// # Erreurs
/// Si aucun pixel n'est retenu par le masque, ou si l'image est vide.
pub fn vectoriser(img: &Image, reglages: ReglagesVecteur) -> Result<String, Error> {
    if img.largeur == 0 || img.hauteur == 0 {
        return Err(Error::Invalid("image vide".into()));
    }
    let (w, h) = (img.largeur as usize, img.hauteur as usize);

    let mut indices = Vec::new();
    let mut echantillons = Vec::new();
    for y in 0..img.hauteur {
        for x in 0..img.largeur {
            let px = img.pixel(x, y);
            if retenu(px, reglages.masque) {
                indices.push((y as usize) * w + (x as usize));
                echantillons.push([px[0], px[1], px[2]]);
            }
        }
    }
    if echantillons.is_empty() {
        return Err(Error::Invalid(
            "aucun pixel retenu : c'est le masque qu'il faut corriger, pas l'image".into(),
        ));
    }

    // Les représentants du k-means servent de couleurs de couche ; chaque pixel retenu rejoint
    // le représentant le plus proche EN OKLAB — reclasser en sRGB rouvrirait le défaut que le
    // k-means évite.
    let classes = kmeans(&echantillons, reglages.k);
    let labs: Vec<[f64; 3]> = classes.iter().map(|(_, c)| oklab(*c)).collect();

    let mut couches: Vec<Vec<bool>> = vec![vec![false; w * h]; classes.len()];
    for (i, ech) in echantillons.iter().enumerate() {
        let l = oklab(*ech);
        let mut best = 0usize;
        let mut bd = f64::MAX;
        for (j, c) in labs.iter().enumerate() {
            let d = distance2(l, *c);
            if d < bd {
                bd = d;
                best = j;
            }
        }
        couches[best][indices[i]] = true;
    }

    let mut corps = String::new();
    for (couche, (compte, rgb)) in couches.iter().zip(classes.iter()) {
        if *compte < reglages.aire_min {
            continue;
        }
        let mut d = String::new();
        for boucle in contours(couche, w, h) {
            let simple = simplifier(&boucle, reglages.tolerance);
            if simple.len() < 3 {
                continue;
            }
            if !d.is_empty() {
                d.push(' ');
            }
            d.push_str(&path_de_boucle(&simple));
        }
        if d.is_empty() {
            continue;
        }
        corps.push_str(&format!(
            "\n  <path fill=\"#{:02X}{:02X}{:02X}\" fill-rule=\"nonzero\" d=\"{d}\"/>",
            rgb[0], rgb[1], rgb[2]
        ));
    }

    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\">{corps}\n</svg>\n",
        img.largeur, img.hauteur, img.largeur, img.hauteur
    ))
}

// ---------------------------------------------------------------------------------------------
// Planche de sprites et jetons CSS — le pont vers `nie_formats::sprite_sheet`
// ---------------------------------------------------------------------------------------------

/// Assemble des images nommées en une planche, et rend la feuille de sprites qui la décrit.
///
/// Le type de sortie est celui du dépôt, [`nie_formats::sprite_sheet::SpriteSheet`] : la même
/// structure que celle tirée d'un `.g4tx`, donc les mêmes `vers_css` / `vers_svg` / `vers_json`
/// derrière. Ce module **apporte une planche**, il ne réimplémente pas leur écriture.
///
/// Toutes les cases font la taille de la plus grande image, et chaque image est **posée en haut
/// à gauche de sa case, sans rééchantillonnage**. C'est délibéré : recentrer ou redimensionner
/// pose par pose fait sauter le sujet d'une case à l'autre à la lecture, et le défaut ne se voit
/// qu'une fois l'animation en marche.
///
/// # Erreurs
/// Si la liste est vide, ou si la planche demandée dépasse ce que `u32` peut adresser.
pub fn planche(
    images: &[(String, Image)],
    colonnes: Option<usize>,
    nom: &str,
) -> Result<(Image, nie_formats::sprite_sheet::SpriteSheet), Error> {
    use nie_formats::sprite_sheet::{Sprite, SpriteSheet, assainir_nom};

    if images.is_empty() {
        return Err(Error::Invalid("aucune image à assembler".into()));
    }
    let cw = images.iter().map(|(_, i)| i.largeur).max().unwrap_or(0);
    let ch = images.iter().map(|(_, i)| i.hauteur).max().unwrap_or(0);
    if cw == 0 || ch == 0 {
        return Err(Error::Invalid("une image de la planche est vide".into()));
    }
    // Grille par défaut : la plus carrée possible, ce qui donne la planche la plus compacte à
    // décoder pour le GPU comme pour le navigateur.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "≥ 1, borné"
    )]
    let cols = colonnes
        .unwrap_or_else(|| (images.len() as f64).sqrt().ceil().max(1.0) as usize)
        .max(1);
    let lignes = images.len().div_ceil(cols);

    let (Ok(pw), Ok(ph)) = (u32::try_from(cols), u32::try_from(lignes)) else {
        return Err(Error::Invalid("planche trop grande".into()));
    };
    let (largeur, hauteur) = (cw * pw, ch * ph);
    let mut rgba = vec![0u8; (largeur as usize) * (hauteur as usize) * 4];
    let mut sprites = Vec::with_capacity(images.len());

    for (i, (nom_sprite, img)) in images.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "cols et lignes tiennent en u32"
        )]
        let (col, ligne) = ((i % cols) as u32, (i / cols) as u32);
        let (ox, oy) = (col * cw, ligne * ch);
        for y in 0..img.hauteur {
            let src = (y as usize) * (img.largeur as usize) * 4;
            let dst = (((oy + y) as usize) * (largeur as usize) + (ox as usize)) * 4;
            let n = (img.largeur as usize) * 4;
            rgba[dst..dst + n].copy_from_slice(&img.rgba[src..src + n]);
        }
        let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
            i32::try_from(ox),
            i32::try_from(oy),
            i32::try_from(img.largeur),
            i32::try_from(img.hauteur),
        ) else {
            return Err(Error::Invalid("planche trop grande pour i32".into()));
        };
        sprites.push(Sprite {
            classe: assainir_nom(nom_sprite),
            nom: nom_sprite.clone(),
            x,
            y,
            largeur: w,
            hauteur: h,
        });
    }

    let (Ok(pl), Ok(phh)) = (i32::try_from(largeur), i32::try_from(hauteur)) else {
        return Err(Error::Invalid("planche trop grande pour i32".into()));
    };
    let feuille = SpriteSheet {
        nom: nom.to_owned(),
        largeur: pl,
        hauteur: phh,
        sprites,
    };
    Ok((Image::nouvelle(largeur, hauteur, rgba)?, feuille))
}

/// Écrit la palette mesurée en propriétés personnalisées CSS, en `oklch()`.
///
/// `oklch()` plutôt que le HEX mesuré : c'est la forme dans laquelle une couleur se **décline**
/// (éclaircir un ton, c'est monter `L` sans toucher `C` ni `h`), et la seule où deux teintes
/// voisines le restent après ajustement. Le HEX d'origine reste en commentaire sur chaque ligne
/// — sans lui, plus rien ne rattache le jeton à la mesure dont il sort.
#[must_use]
pub fn tokens_css(mesure: &Mesure, prefixe: &str) -> String {
    let mut css = format!(
        "/* Jetons mesurés — {}x{}, boîte {}x{}, remplissage {:.2} %.\n   \
         Régénérer par `pixel mesurer <IMG> --css` ; ne pas retoucher à la main. */\n:root {{\n",
        mesure.source[0],
        mesure.source[1],
        mesure.boite.largeur(),
        mesure.boite.hauteur(),
        mesure.remplissage_pct
    );
    for (i, c) in mesure.palette.iter().enumerate() {
        css.push_str(&format!(
            "  --{prefixe}-{i}: oklch({:.4} {:.4} {:.2});  /* {} — {:.2} % des pixels */\n",
            c.oklch[0], c.oklch[1], c.oklch[2], c.hex, c.part_pct
        ));
    }
    css.push_str("}\n");
    css
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un disque plein sur fond transparent : les grandeurs mesurées doivent être celles de la
    /// géométrie, pas des valeurs plausibles. C'est le test qui empêche la mesure de dériver.
    fn disque(cote: u32) -> Image {
        let mut rgba = vec![0u8; (cote as usize) * (cote as usize) * 4];
        let r = f64::from(cote) / 2.0 - 1.0;
        let c = f64::from(cote) / 2.0;
        for y in 0..cote {
            for x in 0..cote {
                let d = (f64::from(x) + 0.5 - c).hypot(f64::from(y) + 0.5 - c);
                if d <= r {
                    let i = ((y as usize) * (cote as usize) + (x as usize)) * 4;
                    rgba[i..i + 4].copy_from_slice(&[0xF3, 0xA1, 0x3A, 0xFF]);
                }
            }
        }
        Image::nouvelle(cote, cote, rgba).expect("tampon cohérent")
    }

    #[test]
    fn un_disque_mesure_bien_comme_un_disque() {
        let m = mesurer(&disque(128), Reglages::default()).expect("mesure");
        assert!(
            (m.ratio - 1.0).abs() < 0.02,
            "ratio {} attendu ≈ 1",
            m.ratio
        );
        // π/4 = 78,54 % ; on tolère le crénelage du bord.
        assert!(
            (m.remplissage_pct - 78.54).abs() < 2.0,
            "remplissage {} attendu ≈ 78,5 %",
            m.remplissage_pct
        );
        assert_eq!(m.palette.len(), 1, "un seul aplat, donc une seule classe");
        assert_eq!(m.palette[0].hex, "#F3A13A");
        // Le profil de colonnes d'un disque culmine au centre.
        let milieu = m.profil_colonnes.len() / 2;
        assert!(m.profil_colonnes[milieu] > m.profil_colonnes[2]);
    }

    #[test]
    fn un_masque_qui_ne_retient_rien_est_une_erreur_pas_un_zero() {
        let img = Image::nouvelle(4, 4, vec![0u8; 64]).expect("tampon");
        let err = mesurer(&img, Reglages::default()).expect_err("doit échouer");
        assert!(format!("{err}").contains("masque"));
    }

    #[test]
    fn comparer_une_image_a_elle_meme_rend_l_identite() {
        let d = disque(64);
        let c = comparer(&d, &d, 0).expect("comparaison");
        assert!(c.identique);
        assert!((c.pixels_dans_tolerance_pct - 100.0).abs() < f64::EPSILON);
        assert!(c.ssim > 0.999, "ssim {}", c.ssim);
    }

    #[test]
    fn comparer_refuse_deux_tailles_differentes() {
        let err = comparer(&disque(64), &disque(32), 0).expect_err("doit échouer");
        assert!(format!("{err}").contains("dimensions"));
    }

    #[test]
    fn un_svg_se_rasterise_a_la_largeur_demandee() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50"><rect width="100" height="50" fill="#F3A13A"/></svg>"##;
        let img = rasteriser_svg(svg, 200).expect("rendu");
        assert_eq!(img.largeur, 200);
        assert_eq!(
            img.hauteur, 100,
            "la hauteur suit le viewBox, elle ne se devine pas"
        );
        assert_eq!(&img.rgba[0..3], &[0xF3, 0xA1, 0x3A]);
    }

    #[test]
    fn un_carre_se_vectorise_en_quatre_points() {
        let mut rgba = vec![0u8; 16 * 16 * 4];
        for y in 4..12 {
            for x in 4..12 {
                let i = (y * 16 + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[0x20, 0x40, 0x80, 0xFF]);
            }
        }
        let img = Image::nouvelle(16, 16, rgba).expect("tampon");
        let svg = vectoriser(
            &img,
            ReglagesVecteur {
                k: 1,
                ..ReglagesVecteur::default()
            },
        )
        .expect("vectorisation");
        assert!(
            svg.contains("#204080"),
            "la couleur de couche vient de l'image : {svg}"
        );
        // Un carré parfait : 4 sommets, pas 32 marches d'escalier.
        let sommets = svg.matches('L').count() + svg.matches('M').count();
        assert_eq!(
            sommets, 4,
            "un carré doit rendre 4 points, pas {sommets} : {svg}"
        );
    }

    #[test]
    fn le_svg_vectorise_se_rasterise_et_retombe_sur_la_source() {
        let d = disque(64);
        let svg = vectoriser(
            &d,
            ReglagesVecteur {
                k: 1,
                ..ReglagesVecteur::default()
            },
        )
        .expect("vectorisation");
        let rendu = rasteriser_svg(&svg, 64).expect("rendu");
        let c = comparer(&d, &rendu, 24).expect("comparaison");
        // Le contrôle qui compte : le vecteur doit RESSEMBLER à sa source, chiffres à l'appui.
        assert!(c.ssim > 0.90, "ssim {} trop bas", c.ssim);
        assert!(
            c.pixels_dans_tolerance_pct > 95.0,
            "{} % dans la tolérance",
            c.pixels_dans_tolerance_pct
        );
    }

    #[test]
    fn une_planche_pose_les_cases_sans_reechantillonner() {
        let images = vec![
            ("pose idle".to_string(), disque(32)),
            ("pose/court".to_string(), disque(24)),
            ("pose tir".to_string(), disque(32)),
        ];
        let (planche_img, feuille) = planche(&images, Some(2), "poses").expect("planche");
        // 3 images, 2 colonnes → 2 lignes ; case = la plus grande image.
        assert_eq!((planche_img.largeur, planche_img.hauteur), (64, 64));
        assert_eq!(feuille.sprites.len(), 3);
        // La case garde la taille RÉELLE de son image, pas celle de la case.
        assert_eq!(
            (feuille.sprites[1].largeur, feuille.sprites[1].hauteur),
            (24, 24)
        );
        assert_eq!((feuille.sprites[2].x, feuille.sprites[2].y), (0, 32));
        // Le nom est assaini pour servir de classe, l'original est conservé.
        assert_eq!(feuille.sprites[1].nom, "pose/court");
        assert!(
            !feuille.sprites[1].classe.contains('/'),
            "{}",
            feuille.sprites[1].classe
        );
    }

    #[test]
    fn la_planche_se_rend_en_css_et_en_svg_par_nie_formats() {
        let images = vec![("a".to_string(), disque(16)), ("b".to_string(), disque(16))];
        let (_, feuille) = planche(&images, Some(2), "poses").expect("planche");
        let css = feuille.vers_css("poses.webp");
        assert!(css.contains("background-size: 32px 16px;"), "{css}");
        // La forme exacte vient de `nie_formats::sprite_sheet`, elle n'est PAS réécrite ici :
        // c'est tout l'intérêt du branchement, et ce test le verrouille.
        assert!(
            css.contains(".nie-b { width: 16px; height: 16px; background-position: -16px 0px; }"),
            "{css}"
        );
        assert!(
            feuille
                .vers_svg("data:image/png;base64,AA")
                .contains("<symbol")
        );
        assert!(feuille.vers_json().contains("\"nom\": \"poses\""));
    }

    #[test]
    fn les_jetons_css_gardent_le_hex_qui_les_justifie() {
        let m = mesurer(&disque(64), Reglages::default()).expect("mesure");
        let css = tokens_css(&m, "menu");
        assert!(css.contains("--menu-0: oklch("), "{css}");
        assert!(
            css.contains("#F3A13A"),
            "le HEX mesuré doit rester lisible : {css}"
        );
    }

    #[test]
    fn une_planche_vide_est_une_erreur() {
        let err = planche(&[], None, "x").expect_err("doit échouer");
        assert!(format!("{err}").contains("aucune image"));
    }
}
