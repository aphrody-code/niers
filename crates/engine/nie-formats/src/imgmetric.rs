//! Comparaison d'images de rendu — le **juge** de la cible pixel-perfect.
//!
//! ## Pourquoi un module et pas une dépendance
//!
//! `dssim-core` est en AGPL-3.0, incompatible avec la licence du dépôt (cf. `docs/STACK.md`), et
//! `image-compare` cacherait derrière une façade les décisions qui font justement le résultat :
//! espace de couleur, gestion de l'alpha, taille de fenêtre. La doctrine du dépôt est de ne pas
//! utiliser de brique qui cache son comportement quand ce comportement *est* la mesure.
//!
//! ## Ce que ce module corrige par rapport au SSIM du gate
//!
//! Le SSIM historique (`nie-game/tests/menu_render_gate.rs`) a quatre biais mesurés, qui le rendent
//! incapable de voir un écart de l'ordre du niveau :
//!
//! | biais | conséquence | correction ici |
//! |---|---|---|
//! | luma seule | aveugle à la teinte : un aplat de bonne luminance et de mauvaise couleur score haut | trois canaux, score = le pire des trois |
//! | alpha ignoré | un canvas transparent est lu comme du noir, ce qui récompense le fait de peindre un fond | `couverture_opaque` rapportée, pixels non couverts exclus |
//! | moyenne 2×2 en octets sRGB | la **référence** est faussée avant toute comparaison | [`downscale_lineaire_2x`] moyenne en lumière linéaire |
//! | fenêtres 8×8 disjointes, moyennées aussitôt | insensible aux décalages, et la carte par bloc est jetée | fenêtres chevauchantes (pas 4), carte conservée |
//!
//! ## Les trois niveaux d'acceptation
//!
//! Dans cet ordre — c'est « égalité d'octets d'abord, tolérance ensuite » transposé à ce qui est
//! mesurable face à une capture d'écran :
//!
//! 1. **T0, identité** — part de pixels dont les trois canaux sont égaux. Le seul chiffre qui vaut
//!    pour du pixel-perfect. Il ne vaudra jamais 100 % face à une capture : le rastériseur du jeu
//!    n'est pas le nôtre (cf. `docs/DESIGN.md`).
//! 2. **T1, imperceptibilité** — part de pixels à ΔE2000 ≤ 1, le seuil sous lequel l'œil ne
//!    sépare plus deux couleurs. Le filet quand l'arrondi de rééchantillonnage interdit T0.
//! 3. **T2, structure** — SSIM par région. Utile pour situer, jamais pour conclure.
//!
//! ## Régions
//!
//! Un score global ment toujours : il mélange une zone parfaite et une zone fausse. La sortie est
//! donc **par région nommée**, et les régions dynamiques (personnage 3D, particules, curseur) sont
//! **exclues** — exclues et déclarées, jamais silencieusement : [`Rapport::surface_exclue_pct`]
//! existe pour que personne ne lise un score sans savoir ce qu'il ne couvre pas.

/// Rôle d'une région dans la mesure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoiKind {
    /// Contenu que le dépôt ne reproduit pas (3D, particules, curseur) : **exclu** de tous les
    /// scores. Sa surface est publiée à part.
    Dynamique,
    /// Zone d'intérêt mesurée séparément, en plus du global.
    Nommee,
}

/// Une région de l'image, en pixels.
#[derive(Debug, Clone)]
pub struct Roi {
    /// Nom lisible, repris tel quel dans le rapport.
    pub nom: String,
    /// `(x, y, largeur, hauteur)`.
    pub rect: (u32, u32, u32, u32),
    /// Exclue ou simplement mesurée à part.
    pub kind: RoiKind,
}

/// Scores d'une zone — l'image entière, ou une région nommée.
#[derive(Debug, Clone)]
pub struct ScoreRegion {
    /// Nom de la zone (`"global"` pour l'image entière).
    pub nom: String,
    /// Pixels effectivement comparés (hors régions dynamiques).
    pub px: u64,
    /// T0 — part de pixels dont les trois canaux sont **égaux**.
    pub exact_pct: f64,
    /// T1 — part de pixels à ΔE2000 ≤ 1.
    pub de1_pct: f64,
    /// Part de pixels dont chaque canal s'écarte d'au plus 2 niveaux.
    pub canal2_pct: f64,
    /// ΔE2000 moyen.
    pub de_moyen: f64,
    /// ΔE2000 au 99ᵉ centile — ce que la moyenne cache.
    pub de_p99: f64,
    /// ΔE2000 maximal.
    pub de_max: f64,
    /// T2 — SSIM, le pire des trois canaux, en lumière linéaire.
    pub ssim: f64,
}

/// Résultat complet d'une comparaison.
#[derive(Debug, Clone)]
pub struct Rapport {
    /// Scores sur toute l'image, régions dynamiques exclues.
    pub global: ScoreRegion,
    /// Scores des régions nommées.
    pub regions: Vec<ScoreRegion>,
    /// Part de la surface retirée de la mesure par les régions dynamiques.
    pub surface_exclue_pct: f64,
    /// Part des pixels mesurés dont le rendu est **opaque**. En dessous de 100 %, le rendu laisse
    /// voir le fond du canvas : le score porte alors en partie sur du vide, et non sur des pixels.
    pub couverture_opaque_pct: f64,
    /// Carte SSIM par bloc, ligne par ligne. `f64::NAN` pour un bloc entièrement exclu.
    pub bloc_ssim: Vec<f64>,
    /// Nombre de blocs par ligne.
    pub bloc_w: u32,
    /// Nombre de lignes de blocs.
    pub bloc_h: u32,
}

/// Côté d'une fenêtre SSIM, en pixels.
const FENETRE: u32 = 8;
/// Pas entre deux fenêtres. Inférieur à [`FENETRE`], donc fenêtres **chevauchantes** : une fenêtre
/// disjointe ne voit pas un décalage d'un pixel, qui est pourtant exactement ce qu'on traque.
const PAS: u32 = 4;

/// Convertit un octet sRGB en lumière linéaire `0..=1` (courbe sRGB officielle, pas un gamma 2,2).
#[must_use]
pub fn srgb_vers_lineaire(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Moyenne 2×2 **en lumière linéaire** puis retour en sRGB.
///
/// Moyenner des octets sRGB directement — ce que fait le downscale du gate — fausse la référence
/// avant même la comparaison : sur un damier noir/blanc, la moyenne correcte est bien plus claire
/// que la demi-valeur naïve. L'écart introduit est du même ordre que celui qu'on cherche à mesurer.
#[must_use]
pub fn downscale_lineaire_2x(w: u32, h: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let (nw, nh) = (w / 2, h / 2);
    let mut out = vec![0u8; (nw as usize) * (nh as usize) * 4];
    for y in 0..nh {
        for x in 0..nw {
            let mut acc = [0.0f64; 4];
            for dy in 0..2 {
                for dx in 0..2 {
                    let idx = (((y * 2 + dy) as usize) * (w as usize) + (x * 2 + dx) as usize) * 4;
                    for (c, a) in acc.iter_mut().enumerate().take(3) {
                        *a += srgb_vers_lineaire(rgba[idx + c]);
                    }
                    acc[3] += f64::from(rgba[idx + 3]);
                }
            }
            let dst = ((y as usize) * (nw as usize) + x as usize) * 4;
            for c in 0..3 {
                out[dst + c] = lineaire_vers_srgb(acc[c] / 4.0);
            }
            out[dst + 3] = (acc[3] / 4.0).round() as u8;
        }
    }
    (nw, nh, out)
}

/// Retour de la lumière linéaire vers l'octet sRGB.
#[must_use]
fn lineaire_vers_srgb(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// sRGB → CIE Lab (illuminant D65, observateur 2°).
#[must_use]
fn srgb_vers_lab(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (rl, gl, bl) = (
        srgb_vers_lineaire(r),
        srgb_vers_lineaire(g),
        srgb_vers_lineaire(b),
    );
    // Matrice sRGB → XYZ D65.
    let x = 0.412_456_4 * rl + 0.357_576_1 * gl + 0.180_437_5 * bl;
    let y = 0.212_672_9 * rl + 0.715_152_2 * gl + 0.072_175_0 * bl;
    let z = 0.019_333_9 * rl + 0.119_192_0 * gl + 0.950_304_1 * bl;
    // Blanc de référence D65.
    let f = |t: f64| -> f64 {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (841.0 / 108.0) * t + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.950_489), f(y), f(z / 1.088_840));
    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

/// ΔE CIEDE2000 entre deux couleurs sRGB.
///
/// Métrique perceptuelle : sous 1,0 l'œil ne distingue plus les deux couleurs. C'est ce qui permet
/// de dire « imperceptible » sans se mentir, là où une différence de canal brute ne dit rien de la
/// perception (2 niveaux dans les ombres ne valent pas 2 niveaux dans les clairs).
#[must_use]
pub fn delta_e2000(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (l1, a1, b1) = srgb_vers_lab(a.0, a.1, a.2);
    let (l2, a2, b2) = srgb_vers_lab(b.0, b.1, b.2);

    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let c_moy = (c1 + c2) / 2.0;

    let g = 0.5 * (1.0 - (c_moy.powi(7) / (c_moy.powi(7) + 25.0f64.powi(7))).sqrt());
    let (a1p, a2p) = (a1 * (1.0 + g), a2 * (1.0 + g));
    let c1p = (a1p * a1p + b1 * b1).sqrt();
    let c2p = (a2p * a2p + b2 * b2).sqrt();

    let h = |ap: f64, bp: f64| -> f64 {
        if ap == 0.0 && bp == 0.0 {
            return 0.0;
        }
        let d = bp.atan2(ap).to_degrees();
        if d < 0.0 { d + 360.0 } else { d }
    };
    let (h1p, h2p) = (h(a1p, b1), h(a2p, b2));

    let dlp = l2 - l1;
    let dcp = c2p - c1p;
    let dhp = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dhp_grand = 2.0 * (c1p * c2p).sqrt() * (dhp.to_radians() / 2.0).sin();

    let lp_moy = (l1 + l2) / 2.0;
    let cp_moy = (c1p + c2p) / 2.0;
    let hp_moy = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };

    let t = 1.0 - 0.17 * (hp_moy - 30.0).to_radians().cos()
        + 0.24 * (2.0 * hp_moy).to_radians().cos()
        + 0.32 * (3.0 * hp_moy + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hp_moy - 63.0).to_radians().cos();

    let d_theta = 30.0 * (-(((hp_moy - 275.0) / 25.0).powi(2))).exp();
    let rc = 2.0 * (cp_moy.powi(7) / (cp_moy.powi(7) + 25.0f64.powi(7))).sqrt();
    let sl = 1.0 + (0.015 * (lp_moy - 50.0).powi(2)) / (20.0 + (lp_moy - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cp_moy;
    let sh = 1.0 + 0.015 * cp_moy * t;
    let rt = -(2.0 * d_theta.to_radians()).sin() * rc;

    ((dlp / sl).powi(2)
        + (dcp / sc).powi(2)
        + (dhp_grand / sh).powi(2)
        + rt * (dcp / sc) * (dhp_grand / sh))
        .sqrt()
}

/// Construit le masque des pixels à mesurer : `false` sur les régions dynamiques.
fn masque(w: u32, h: u32, rois: &[Roi]) -> Vec<bool> {
    let mut m = vec![true; (w as usize) * (h as usize)];
    for roi in rois.iter().filter(|r| r.kind == RoiKind::Dynamique) {
        let (rx, ry, rw, rh) = roi.rect;
        for y in ry..(ry + rh).min(h) {
            for x in rx..(rx + rw).min(w) {
                m[(y as usize) * (w as usize) + x as usize] = false;
            }
        }
    }
    m
}

/// SSIM d'un canal sur une fenêtre, en lumière linéaire mise à l'échelle `0..=255`.
///
/// Rend `None` si la fenêtre contient un pixel exclu : mieux vaut ne pas noter une fenêtre que la
/// noter sur des pixels qu'on a décidé de ne pas juger.
fn ssim_fenetre(
    a: &[f64],
    b: &[f64],
    m: &[bool],
    w: u32,
    x0: u32,
    y0: u32,
    cote: u32,
) -> Option<f64> {
    const C1: f64 = 6.5025;
    const C2: f64 = 58.5225;
    let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let n = f64::from(cote * cote);
    for y in y0..y0 + cote {
        for x in x0..x0 + cote {
            let i = (y as usize) * (w as usize) + x as usize;
            if !m[i] {
                return None;
            }
            let (va, vb) = (a[i], b[i]);
            sa += va;
            sb += vb;
            saa += va * va;
            sbb += vb * vb;
            sab += va * vb;
        }
    }
    let (ma, mb) = (sa / n, sb / n);
    let va = (saa - sa * ma) / (n - 1.0);
    let vb = (sbb - sb * mb) / (n - 1.0);
    let cov = (sab - sa * mb) / (n - 1.0);
    Some(((2.0 * ma * mb + C1) * (2.0 * cov + C2)) / ((ma * ma + mb * mb + C1) * (va + vb + C2)))
}

/// Compare deux images RGBA8 de mêmes dimensions.
///
/// # Panics
///
/// Si les tampons ne font pas `w * h * 4` octets.
#[must_use]
pub fn comparer(w: u32, h: u32, rendu: &[u8], reference: &[u8], rois: &[Roi]) -> Rapport {
    let n = (w as usize) * (h as usize);
    assert_eq!(rendu.len(), n * 4, "tampon rendu");
    assert_eq!(reference.len(), n * 4, "tampon référence");

    let m = masque(w, h, rois);
    let exclus = m.iter().filter(|v| !**v).count();

    // Canaux en lumière linéaire pour le SSIM : c'est l'espace où une différence de valeur
    // correspond à une différence de lumière, donc le seul où la variance a un sens physique.
    let mut lin_a = vec![[0.0f64; 3]; n];
    let mut lin_b = vec![[0.0f64; 3]; n];
    for i in 0..n {
        for c in 0..3 {
            lin_a[i][c] = srgb_vers_lineaire(rendu[i * 4 + c]) * 255.0;
            lin_b[i][c] = srgb_vers_lineaire(reference[i * 4 + c]) * 255.0;
        }
    }

    let global = scores(
        w,
        h,
        rendu,
        reference,
        &m,
        &lin_a,
        &lin_b,
        "global",
        (0, 0, w, h),
    );
    let regions = rois
        .iter()
        .filter(|r| r.kind == RoiKind::Nommee)
        .map(|r| scores(w, h, rendu, reference, &m, &lin_a, &lin_b, &r.nom, r.rect))
        .collect();

    let mesures = n - exclus;
    let opaques = (0..n).filter(|&i| m[i] && rendu[i * 4 + 3] == 255).count();

    let (bloc_ssim, bloc_w, bloc_h) = carte_blocs(w, h, &m, &lin_a, &lin_b);

    Rapport {
        global,
        regions,
        surface_exclue_pct: pourcent(exclus, n),
        couverture_opaque_pct: pourcent(opaques, mesures.max(1)),
        bloc_ssim,
        bloc_w,
        bloc_h,
    }
}

/// Part en pourcentage, `0.0` si le dénominateur est nul.
fn pourcent(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (part as f64) * 100.0 / (total as f64)
    }
}

/// Calcule tous les scores d'un rectangle.
#[allow(clippy::too_many_arguments)]
fn scores(
    w: u32,
    h: u32,
    rendu: &[u8],
    reference: &[u8],
    m: &[bool],
    lin_a: &[[f64; 3]],
    lin_b: &[[f64; 3]],
    nom: &str,
    rect: (u32, u32, u32, u32),
) -> ScoreRegion {
    let (rx, ry, rw, rh) = rect;
    let (x1, y1) = ((rx + rw).min(w), (ry + rh).min(h));

    let mut px = 0u64;
    let mut exacts = 0u64;
    let mut de1 = 0u64;
    let mut canal2 = 0u64;
    let mut des: Vec<f64> = Vec::new();

    for y in ry..y1 {
        for x in rx..x1 {
            let i = (y as usize) * (w as usize) + x as usize;
            if !m[i] {
                continue;
            }
            px += 1;
            let (a, b) = (&rendu[i * 4..i * 4 + 3], &reference[i * 4..i * 4 + 3]);
            if a == b {
                exacts += 1;
                canal2 += 1;
                des.push(0.0);
                de1 += 1;
                continue;
            }
            if (0..3).all(|c| a[c].abs_diff(b[c]) <= 2) {
                canal2 += 1;
            }
            let d = delta_e2000((a[0], a[1], a[2]), (b[0], b[1], b[2]));
            if d <= 1.0 {
                de1 += 1;
            }
            des.push(d);
        }
    }

    des.sort_by(|p, q| p.partial_cmp(q).unwrap_or(core::cmp::Ordering::Equal));
    let de_moyen = if des.is_empty() {
        0.0
    } else {
        des.iter().sum::<f64>() / des.len() as f64
    };
    let de_p99 = centile(&des, 0.99);
    let de_max = des.last().copied().unwrap_or(0.0);

    // SSIM : le pire des trois canaux. Un canal juste ne rachète pas un canal faux.
    let mut pire = f64::INFINITY;
    for c in 0..3 {
        let a: Vec<f64> = lin_a.iter().map(|p| p[c]).collect();
        let b: Vec<f64> = lin_b.iter().map(|p| p[c]).collect();
        let mut somme = 0.0;
        let mut compte = 0u32;
        let mut y = ry;
        while y + FENETRE <= y1 {
            let mut x = rx;
            while x + FENETRE <= x1 {
                if let Some(s) = ssim_fenetre(&a, &b, m, w, x, y, FENETRE) {
                    somme += s;
                    compte += 1;
                }
                x += PAS;
            }
            y += PAS;
        }
        let s = if compte == 0 {
            f64::NAN
        } else {
            somme / f64::from(compte)
        };
        if s < pire {
            pire = s;
        }
    }

    ScoreRegion {
        nom: String::from(nom),
        px,
        exact_pct: pourcent(exacts as usize, px as usize),
        de1_pct: pourcent(de1 as usize, px as usize),
        canal2_pct: pourcent(canal2 as usize, px as usize),
        de_moyen,
        de_p99,
        de_max,
        ssim: if pire.is_finite() { pire } else { f64::NAN },
    }
}

/// Valeur au centile demandé d'une série **déjà triée**.
fn centile(tries: &[f64], q: f64) -> f64 {
    if tries.is_empty() {
        return 0.0;
    }
    let idx = ((tries.len() as f64 - 1.0) * q).round() as usize;
    tries[idx.min(tries.len() - 1)]
}

/// Carte SSIM par bloc — c'est elle qui dit *où* partent les points, ce qu'un score global ne dira
/// jamais. Blocs disjoints ici : une carte doit se lire, donc un bloc = une tuile d'image.
fn carte_blocs(
    w: u32,
    h: u32,
    m: &[bool],
    lin_a: &[[f64; 3]],
    lin_b: &[[f64; 3]],
) -> (Vec<f64>, u32, u32) {
    let (bw, bh) = (w / FENETRE, h / FENETRE);
    let mut carte = vec![f64::NAN; (bw as usize) * (bh as usize)];
    let canaux: Vec<(Vec<f64>, Vec<f64>)> = (0..3)
        .map(|c| {
            (
                lin_a.iter().map(|p| p[c]).collect(),
                lin_b.iter().map(|p| p[c]).collect(),
            )
        })
        .collect();
    for by in 0..bh {
        for bx in 0..bw {
            let mut pire = f64::INFINITY;
            for (a, b) in &canaux {
                match ssim_fenetre(a, b, m, w, bx * FENETRE, by * FENETRE, FENETRE) {
                    Some(s) if s < pire => pire = s,
                    Some(_) => {}
                    None => {
                        pire = f64::NAN;
                        break;
                    }
                }
            }
            carte[(by as usize) * (bw as usize) + bx as usize] = pire;
        }
    }
    (carte, bw, bh)
}

/// Image de la carte SSIM : bleu = identique, rouge = divergent, gris = exclu.
///
/// Un bloc de la carte devient un carré de `FENETRE` pixels, pour que la heatmap se superpose au
/// rendu sans redimensionnement.
#[cfg(feature = "textures")]
#[must_use]
pub fn heatmap_rgba(r: &Rapport) -> (u32, u32, Vec<u8>) {
    let (w, h) = (r.bloc_w * FENETRE, r.bloc_h * FENETRE);
    let mut out = vec![0u8; (w as usize) * (h as usize) * 4];
    for by in 0..r.bloc_h {
        for bx in 0..r.bloc_w {
            let s = r.bloc_ssim[(by as usize) * (r.bloc_w as usize) + bx as usize];
            let couleur = if s.is_nan() {
                [128, 128, 128, 255]
            } else {
                let t = (1.0 - s.clamp(0.0, 1.0)).clamp(0.0, 1.0);
                [
                    (255.0 * t) as u8,
                    (64.0 * (1.0 - t)) as u8,
                    (255.0 * (1.0 - t)) as u8,
                    255,
                ]
            };
            for y in by * FENETRE..(by + 1) * FENETRE {
                for x in bx * FENETRE..(bx + 1) * FENETRE {
                    let i = ((y as usize) * (w as usize) + x as usize) * 4;
                    out[i..i + 4].copy_from_slice(&couleur);
                }
            }
        }
    }
    (w, h, out)
}

/// Image de l'écart absolu, amplifiée — les régions exclues sont noires.
#[cfg(feature = "textures")]
#[must_use]
pub fn delta_rgba(
    w: u32,
    h: u32,
    rendu: &[u8],
    reference: &[u8],
    rois: &[Roi],
    amplification: u8,
) -> Vec<u8> {
    let m = masque(w, h, rois);
    let n = (w as usize) * (h as usize);
    let mut out = vec![0u8; n * 4];
    for i in 0..n {
        out[i * 4 + 3] = 255;
        if !m[i] {
            continue;
        }
        for c in 0..3 {
            let d = rendu[i * 4 + c].abs_diff(reference[i * 4 + c]);
            out[i * 4 + c] = d.saturating_mul(amplification);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uni(w: u32, h: u32, c: [u8; 4]) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            v.extend_from_slice(&c);
        }
        v
    }

    /// Une image comparée à elle-même : tout est exact, SSIM = 1, carte uniforme.
    #[test]
    fn identite_parfaite() {
        let a = uni(32, 32, [120, 40, 200, 255]);
        let r = comparer(32, 32, &a, &a, &[]);
        assert!((r.global.exact_pct - 100.0).abs() < 1e-9);
        assert!((r.global.ssim - 1.0).abs() < 1e-6, "ssim {}", r.global.ssim);
        assert_eq!(r.global.de_max, 0.0);
        assert!(r.bloc_ssim.iter().all(|s| (s - 1.0).abs() < 1e-6));
        assert!((r.couverture_opaque_pct - 100.0).abs() < 1e-9);
    }

    /// Biais nº1 : deux images de MÊME luma et de teintes opposées. Un SSIM luma les déclare
    /// identiques ; trois canaux doivent les séparer.
    #[test]
    fn la_teinte_ne_passe_plus_inapercue() {
        // Même luma BT.601 (≈ 105) pour deux couleurs très différentes.
        let a = uni(32, 32, [255, 60, 60, 255]);
        let b = uni(32, 32, [0, 130, 130, 255]);
        let r = comparer(32, 32, &a, &b, &[]);
        assert!(
            r.global.ssim < 0.9,
            "ssim {} — la teinte doit compter",
            r.global.ssim
        );
        assert!(
            r.global.de_moyen > 10.0,
            "ΔE {} — écart perceptuel massif",
            r.global.de_moyen
        );
        assert_eq!(r.global.exact_pct, 0.0);
    }

    /// Biais nº2 : un rendu partiellement transparent ne doit pas être noté comme s'il couvrait
    /// tout. Le score reste bon (les pixels comparés le sont), mais la couverture le dit.
    #[test]
    fn la_couverture_incomplete_est_declaree() {
        let mut a = uni(16, 16, [10, 20, 30, 255]);
        for i in 0..64 {
            a[i * 4 + 3] = 0;
        }
        let b = a.clone();
        let r = comparer(16, 16, &a, &b, &[]);
        assert!((r.global.exact_pct - 100.0).abs() < 1e-9);
        assert!(
            r.couverture_opaque_pct < 100.0,
            "couverture {}",
            r.couverture_opaque_pct
        );
        assert!((r.couverture_opaque_pct - 75.0).abs() < 1e-6);
    }

    /// Biais nº3 : le downscale doit moyenner la LUMIÈRE. Sur un damier noir/blanc, la bonne
    /// réponse est ≈ 188, pas 128.
    #[test]
    fn downscale_moyenne_en_lumiere_lineaire() {
        let mut src = vec![0u8; 2 * 2 * 4];
        for (i, p) in [
            [0, 0, 0, 255],
            [255, 255, 255, 255],
            [255, 255, 255, 255],
            [0, 0, 0, 255],
        ]
        .iter()
        .enumerate()
        {
            src[i * 4..i * 4 + 4].copy_from_slice(p);
        }
        let (w, h, out) = downscale_lineaire_2x(2, 2, &src);
        assert_eq!((w, h), (1, 1));
        assert!(
            out[0] > 180 && out[0] < 195,
            "moyenne linéaire attendue ≈188, obtenu {}",
            out[0]
        );
    }

    /// Biais nº4 : la carte par bloc est conservée, et une zone fausse se localise.
    #[test]
    fn la_carte_localise_le_defaut() {
        let a = uni(32, 32, [200, 200, 200, 255]);
        let mut b = a.clone();
        // Coin haut-gauche 8×8 rendu noir.
        for y in 0..8 {
            for x in 0..8 {
                let i = (y * 32 + x) * 4;
                b[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        let r = comparer(32, 32, &a, &b, &[]);
        assert_eq!((r.bloc_w, r.bloc_h), (4, 4));
        assert!(r.bloc_ssim[0] < 0.5, "bloc fautif {}", r.bloc_ssim[0]);
        assert!(r.bloc_ssim[3] > 0.99, "bloc sain {}", r.bloc_ssim[3]);
    }

    /// Une région dynamique est retirée de la mesure, et sa surface est publiée.
    #[test]
    fn la_region_dynamique_est_exclue_et_declaree() {
        let a = uni(32, 32, [10, 10, 10, 255]);
        let mut b = a.clone();
        for y in 0..16 {
            for x in 0..32 {
                let i = (y * 32 + x) * 4;
                b[i..i + 3].copy_from_slice(&[250, 0, 0]);
            }
        }
        let roi = Roi {
            nom: String::from("avatar3d"),
            rect: (0, 0, 32, 16),
            kind: RoiKind::Dynamique,
        };
        let r = comparer(32, 32, &a, &b, core::slice::from_ref(&roi));
        assert!(
            (r.global.exact_pct - 100.0).abs() < 1e-9,
            "la moitié fausse est exclue"
        );
        assert!((r.surface_exclue_pct - 50.0).abs() < 1e-6);
        assert_eq!(r.global.px, 512);
    }

    /// Une région nommée est mesurée à part, sans être retirée du global.
    #[test]
    fn la_region_nommee_est_mesuree_a_part() {
        let a = uni(32, 32, [10, 10, 10, 255]);
        let mut b = a.clone();
        for y in 16..32 {
            for x in 0..32 {
                let i = (y * 32 + x) * 4;
                b[i..i + 3].copy_from_slice(&[11, 10, 10]);
            }
        }
        let roi = Roi {
            nom: String::from("bas"),
            rect: (0, 16, 32, 16),
            kind: RoiKind::Nommee,
        };
        let r = comparer(32, 32, &a, &b, core::slice::from_ref(&roi));
        assert!((r.surface_exclue_pct - 0.0).abs() < 1e-9);
        assert_eq!(r.regions.len(), 1);
        assert_eq!(r.regions[0].nom, "bas");
        assert_eq!(r.regions[0].exact_pct, 0.0, "la région entière diffère");
        assert!(
            (r.global.exact_pct - 50.0).abs() < 1e-6,
            "moitié exacte au global"
        );
    }

    /// ΔE2000 : un niveau d'écart est imperceptible, deux primaires opposées ne le sont pas.
    #[test]
    fn delta_e_se_comporte_comme_la_perception() {
        assert!(delta_e2000((0, 0, 0), (1, 1, 1)) < 1.0);
        assert!(delta_e2000((255, 0, 0), (0, 255, 0)) > 80.0);
        assert_eq!(delta_e2000((77, 12, 200), (77, 12, 200)), 0.0);
    }

    /// Le SSIM à fenêtres chevauchantes doit VOIR un décalage d'un pixel — c'est précisément ce
    /// qu'une grille disjointe laisse passer.
    #[test]
    fn le_decalage_d_un_pixel_se_voit() {
        // Rayures verticales de période 8 : alignées sur une grille disjointe de pas 8.
        let mut a = vec![0u8; 32 * 32 * 4];
        let mut b = vec![0u8; 32 * 32 * 4];
        for y in 0..32 {
            for x in 0..32usize {
                let v = if (x / 4) % 2 == 0 { 255u8 } else { 0u8 };
                let w = if ((x + 1) / 4) % 2 == 0 { 255u8 } else { 0u8 };
                let i = (y * 32 + x) * 4;
                a[i..i + 3].copy_from_slice(&[v, v, v]);
                a[i + 3] = 255;
                b[i..i + 3].copy_from_slice(&[w, w, w]);
                b[i + 3] = 255;
            }
        }
        let r = comparer(32, 32, &a, &b, &[]);
        assert!(
            r.global.ssim < 0.95,
            "un décalage d'1 px doit faire chuter le SSIM, eu {}",
            r.global.ssim
        );
    }
}
