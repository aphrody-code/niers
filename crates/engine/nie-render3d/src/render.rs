//! Rastériseur 3D CPU : projette un [`Model`](crate::glb::Model) en perspective et le remplit
//! triangle par triangle avec **z-buffer**, **backface culling**, **textures** (échantillonnage
//! nearest perspective-correct, cutout alpha) modulées par un **éclairage Lambert**. Fallback
//! argile pour les primitives sans texture. Headless, déterministe. Le portage GPU/wgpu est le pas
//! suivant ; ici on rend les **vrais maillages + atlas CPK**.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use crate::glb::{Model, Texture};
use crate::vecmath::{V3, cross, dot, normv, sub};

/// Boîte englobante de toutes les primitives → (centre, rayon).
#[must_use]
pub fn bounds(model: &Model) -> (V3, f32) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in &model.primitives {
        for v in &p.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
    }
    let c = [
        (lo[0] + hi[0]) * 0.5,
        (lo[1] + hi[1]) * 0.5,
        (lo[2] + hi[2]) * 0.5,
    ];
    let r = (0..3)
        .map(|k| (hi[k] - lo[k]) * 0.5)
        .fold(0.0_f32, f32::max)
        .max(1e-3);
    (c, r)
}

/// Rotation autour de Y (turntable) puis léger tilt autour de X (vue plongeante douce).
fn orient(v: V3, cy: f32, sy: f32, cx: f32, sx: f32) -> V3 {
    // Y
    let x = v[0] * cy + v[2] * sy;
    let z = -v[0] * sy + v[2] * cy;
    let y = v[1];
    // X (tilt)
    let y2 = y * cx - z * sx;
    let z2 = y * sx + z * cx;
    [x, y2, z2]
}

/// Focale de la projection perspective de référence.
///
/// Un point dont le rapport `x/profondeur` vaut `1/FOCALE` tombe exactement sur le bord de
/// l'image : le demi-champ vertical est donc `atan(1/FOCALE)`. Exposée pour que le pipeline GPU
/// cadre **la même vue** — deux champs de vision différents produisent un modèle plus ou moins
/// gros, ce qui se lit comme une divergence de rendu alors que ce n'est qu'un réglage de caméra.
pub const FOCALE: f32 = 1.7;

/// Distance de la caméra au centre du modèle, en rayons de sa sphère englobante.
pub const DISTANCE_CAMERA: f32 = 3.1;

/// Inclinaison verticale de la vue de référence, en radians.
pub const TILT: f32 = 0.20;

/// Couleur du fond à la ligne `y` d'une image haute de `h` — dégradé vertical sombre, opaque.
///
/// Exposée parce que le fond du rastériseur CPU est **opaque**, là où le pipeline GPU laisse
/// l'arrière-plan transparent (c'est l'interface qui le décide). Comparer les deux rendus exige
/// donc de savoir ce qui est du fond côté CPU, et le recalculer chez l'appelant dupliquerait
/// silencieusement ces constantes : le jour où le dégradé change, la comparaison mentirait.
#[must_use]
pub fn couleur_fond(y: u32, h: u32) -> [u8; 4] {
    let t = y as f32 / h as f32;
    [
        (24.0 + 26.0 * t) as u8,
        (28.0 + 30.0 * t) as u8,
        (40.0 + 34.0 * t) as u8,
        255,
    ]
}

/// Rend `model` vu sous l'angle `angle` (radians) en RGBA8 `w`×`h`.
#[must_use]
pub fn render(model: &Model, angle: f32, w: u32, h: u32) -> Vec<u8> {
    let (center, radius) = bounds(model);
    let inv = 1.0 / radius;
    let (cy, sy) = (angle.cos(), angle.sin());
    let tilt = TILT;
    let (cx, sx) = (tilt.cos(), tilt.sin());

    let cam_d = DISTANCE_CAMERA; // distance caméra (sur +z), modèle normalisé ~[-1,1]
    let focal = FOCALE;
    let scale = w as f32 * 0.5;

    let light = normv([0.35, 0.75, 0.55]);
    let mut px = vec![0u8; (w * h * 4) as usize];
    // Fond : dégradé vertical sombre.
    for y in 0..h {
        let bg = couleur_fond(y, h);
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&bg);
        }
    }
    let mut zbuf = vec![f32::INFINITY; (w * h) as usize];

    // Projette un sommet (centré/normalisé/orienté) → (sx_px, sy_px, depth) ou None si derrière.
    let project = |v: V3| -> Option<(f32, f32, f32)> {
        let n = [
            (v[0] - center[0]) * inv,
            (v[1] - center[1]) * inv,
            (v[2] - center[2]) * inv,
        ];
        let r = orient(n, cy, sy, cx, sx);
        let depth = cam_d - r[2]; // >0 = devant la caméra
        if depth <= 0.05 {
            return None;
        }
        let sxp = w as f32 * 0.5 + focal * r[0] / depth * scale;
        let syp = h as f32 * 0.5 - focal * r[1] / depth * scale;
        Some((sxp, syp, depth))
    };

    for prim in &model.primitives {
        let tex = prim.texture.and_then(|t| model.textures.get(t));
        for tri in prim.indices.chunks_exact(3) {
            let (ia, ib, ic) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            if ia >= prim.positions.len()
                || ib >= prim.positions.len()
                || ic >= prim.positions.len()
            {
                continue;
            }
            let (wa, wb, wc) = (prim.positions[ia], prim.positions[ib], prim.positions[ic]);
            let (Some(a), Some(b), Some(c)) = (project(wa), project(wb), project(wc)) else {
                continue;
            };
            // Backface culling : aire signée écran (y vers le bas). Les faces arrière (crâne, intérieur
            // des cheveux) sont éliminées → la tête cesse d'être « traversée ».
            let screen_area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
            // glTF définit les faces avant en CCW ; la projection inverse l'axe Y.
            if screen_area >= 0.0 {
                continue;
            }
            // Normale de face (espace monde centré/orienté) pour Lambert.
            let na = [
                (wa[0] - center[0]) * inv,
                (wa[1] - center[1]) * inv,
                (wa[2] - center[2]) * inv,
            ];
            let nb = [
                (wb[0] - center[0]) * inv,
                (wb[1] - center[1]) * inv,
                (wb[2] - center[2]) * inv,
            ];
            let nc = [
                (wc[0] - center[0]) * inv,
                (wc[1] - center[1]) * inv,
                (wc[2] - center[2]) * inv,
            ];
            let fn_ = normv(cross(
                sub(orient(nb, cy, sy, cx, sx), orient(na, cy, sy, cx, sx)),
                sub(orient(nc, cy, sy, cx, sx), orient(na, cy, sy, cx, sx)),
            ));
            let lambert = dot(fn_, light).abs();
            let shade = 0.35 + 0.65 * lambert; // texturé : moins d'écrasement que l'argile
            // UV par sommet (vides si la primitive n'est pas texturée → fallback argile).
            let uv = if tex.is_some()
                && ia < prim.uv.len()
                && ib < prim.uv.len()
                && ic < prim.uv.len()
            {
                [prim.uv[ia], prim.uv[ib], prim.uv[ic]]
            } else {
                [[0.0, 0.0]; 3]
            };
            let use_tex = if uv == [[0.0, 0.0]; 3] { None } else { tex };
            fill_triangle(&mut px, &mut zbuf, w, h, a, b, c, uv, use_tex, shade);
        }
    }
    px
}

/// Échantillonnage nearest d'une texture en UV [0,1] (CLAMP_TO_EDGE) → RGBA.
pub(crate) fn sample(t: &Texture, u: f32, v: f32) -> [u8; 4] {
    let uu = u.clamp(0.0, 1.0);
    let vv = v.clamp(0.0, 1.0);
    // Convention nearest : texel = floor(uv * dim), borné à dim-1 (le cas uv==1 retombe sur le dernier).
    let x = ((uu * t.width as f32) as u32).min(t.width.saturating_sub(1));
    let y = ((vv * t.height as f32) as u32).min(t.height.saturating_sub(1));
    let idx = ((y * t.width + x) * 4) as usize;
    [
        t.rgba[idx],
        t.rgba[idx + 1],
        t.rgba[idx + 2],
        t.rgba[idx + 3],
    ]
}

/// Remplit un triangle écran (barycentrique) avec test/écriture z-buffer. Si `tex` est fourni,
/// échantillonne la texture en UV **perspective-correct** (pondéré par 1/profondeur) et applique
/// `shade` (Lambert) ; sinon remplit en argile teintée. Cutout : texel d'alpha < 8 ignoré.
#[allow(clippy::too_many_arguments)]
fn fill_triangle(
    px: &mut [u8],
    zbuf: &mut [f32],
    w: u32,
    h: u32,
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    c: (f32, f32, f32),
    uv: [[f32; 2]; 3],
    tex: Option<&Texture>,
    shade: f32,
) {
    let minx = a.0.min(b.0).min(c.0).floor().max(0.0) as i32;
    let maxx = a.0.max(b.0).max(c.0).ceil().min(w as f32 - 1.0) as i32;
    let miny = a.1.min(b.1).min(c.1).floor().max(0.0) as i32;
    let maxy = a.1.max(b.1).max(c.1).ceil().min(h as f32 - 1.0) as i32;
    let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    if area.abs() < 1e-6 {
        return;
    }
    let inv_area = 1.0 / area;
    let clay = [206.0 * shade, 198.0 * shade, 188.0 * shade]; // fallback non texturé
    for y in miny..=maxy {
        for x in minx..=maxx {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let w0 = ((b.0 - fx) * (c.1 - fy) - (b.1 - fy) * (c.0 - fx)) * inv_area;
            let w1 = ((c.0 - fx) * (a.1 - fy) - (c.1 - fy) * (a.0 - fx)) * inv_area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let depth = 1.0 / (w0 / a.2 + w1 / b.2 + w2 / c.2);
            let zi = (y as u32 * w + x as u32) as usize;
            if depth >= zbuf[zi] {
                continue;
            }
            let col = if let Some(t) = tex {
                // Interpolation perspective-correct : on pondère u/z, v/z et 1/z.
                let invz = w0 / a.2 + w1 / b.2 + w2 / c.2;
                let u = (w0 * uv[0][0] / a.2 + w1 * uv[1][0] / b.2 + w2 * uv[2][0] / c.2) / invz;
                let v = (w0 * uv[0][1] / a.2 + w1 * uv[1][1] / b.2 + w2 * uv[2][1] / c.2) / invz;
                let s = sample(t, u, v);
                if s[3] < 8 {
                    continue; // texel transparent (cheveux/visage en cartes alpha)
                }
                [
                    (f32::from(s[0]) * shade) as u8,
                    (f32::from(s[1]) * shade) as u8,
                    (f32::from(s[2]) * shade) as u8,
                    255,
                ]
            } else {
                [clay[0] as u8, clay[1] as u8, clay[2] as u8, 255]
            };
            zbuf[zi] = depth;
            let i = zi * 4;
            px[i..i + 4].copy_from_slice(&col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb::{Model, Primitive};

    fn tri_model() -> Model {
        Model {
            primitives: vec![Primitive {
                positions: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                uv: Vec::new(),
                indices: vec![0, 1, 2], // CCW, normale +Z : face avant glTF.
                texture: None,
            }],
            textures: Vec::new(),
        }
    }

    #[test]
    fn texture_modulee_par_shade() {
        // Primitive texturée : un quad mappé sur une texture 2×2 rouge/vert/bleu/blanc.
        let tex = Texture {
            width: 2,
            height: 2,
            rgba: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ],
        };
        let model = Model {
            primitives: vec![Primitive {
                positions: vec![
                    [-1.0, -1.0, 0.0],
                    [1.0, -1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [-1.0, 1.0, 0.0],
                ],
                normals: vec![[0.0, 0.0, 1.0]; 4],
                uv: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                indices: vec![0, 1, 2, 0, 2, 3], // CCW glTF.
                texture: Some(0),
            }],
            textures: vec![tex],
        };
        let (w, h) = (96, 96);
        let buf = render(&model, 0.0, w, h);
        // Dominance de teinte (robuste au shade) : un coin où le rouge domine, un autre le bleu —
        // preuve que la texture est bel et bien échantillonnée (et pas un aplat uniforme).
        let red = buf
            .chunks_exact(4)
            .any(|p| p[0] > 60 && p[0] > p[1] + 40 && p[0] > p[2] + 40);
        let blue = buf
            .chunks_exact(4)
            .any(|p| p[2] > 60 && p[2] > p[0] + 40 && p[2] > p[1] + 40);
        assert!(red, "un coin rouge de la texture doit apparaître");
        assert!(blue, "un coin bleu de la texture doit apparaître");
    }

    #[test]
    fn bounds_centre_et_rayon() {
        let (c, r) = bounds(&tri_model());
        assert!((c[0]).abs() < 1e-6 && (c[1]).abs() < 1e-6);
        assert!((r - 1.0).abs() < 1e-6);
    }

    #[test]
    fn render_produit_des_pixels_de_mesh() {
        let (w, h) = (128, 128);
        let buf = render(&tri_model(), 0.0, w, h);
        assert_eq!(buf.len(), (w * h * 4) as usize);
        // Au moins quelques pixels de mesh (argile ~100-200) au-dessus du fond sombre (<60).
        let lit = buf
            .chunks_exact(4)
            .filter(|p| p[0] > 80 && p[1] > 80 && p[2] > 80)
            .count();
        assert!(
            lit > 50,
            "le triangle doit produire des pixels de mesh ({lit})"
        );
    }

    #[test]
    fn face_ccw_visible_et_face_inverse_masquee() {
        let mut model = tri_model();
        let front = render(&model, 0.0, 64, 64);
        model.primitives[0].indices.reverse();
        let back = render(&model, 0.0, 64, 64);
        let background: Vec<u8> = (0..64)
            .flat_map(|y| (0..64).flat_map(move |_| couleur_fond(y, 64)))
            .collect();
        assert_ne!(front, background, "face CCW tournée vers +Z visible");
        assert_eq!(back, background, "face arrière éliminée");
    }
}
