//! Teste, sur une charge utile brute de blocs de 16 octets (BC7), trois dispositions :
//! linéaire, Morton (Z-order) sur les blocs, et tuilée (tuiles de 4 Kio = 16x16 blocs
//! = 64x64 pixels, l'alignement mesuré des 9 `.g4tg` du VFS).
//!
//! Critère validé par témoin (`temoin_bc7.bin`, BC7 256x256 connu) : la vraie disposition
//! est celle où l'écart moyen vertical `dv` rejoint l'écart horizontal `dh`. Sur le témoin,
//! linéaire 256x256 donne dh=1.30 / dv=1.24 ; toute autre largeur donne dv >= 2.0.
//!
//! Usage : `cargo run -p nie-formats --features images,textures --example probe_g4tg_swizzle -- <f>`

use image_dds::{ImageFormat, Surface};

const B: usize = 16; // octets par bloc BC7

fn ecarts(w: usize, h: usize, rgba: &[u8]) -> (f64, f64) {
    let (mut sh, mut nh, mut sv, mut nv) = (0f64, 0usize, 0f64, 0usize);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            if x + 1 < w {
                for c in 0..3 {
                    sh += f64::from(rgba[i + c].abs_diff(rgba[i + 4 + c]));
                }
                nh += 3;
            }
            if y + 1 < h {
                for c in 0..3 {
                    sv += f64::from(rgba[i + c].abs_diff(rgba[i + w * 4 + c]));
                }
                nv += 3;
            }
        }
    }
    #[allow(clippy::cast_precision_loss)]
    (sh / nh.max(1) as f64, sv / nv.max(1) as f64)
}

/// Réordonne les blocs : `src_index(bx, by)` donne l'index source du bloc (bx, by).
fn reordonner(data: &[u8], bw: usize, bh: usize, f: impl Fn(usize, usize) -> usize) -> Vec<u8> {
    let mut out = vec![0u8; bw * bh * B];
    for by in 0..bh {
        for bx in 0..bw {
            let s = f(bx, by) * B;
            let d = (by * bw + bx) * B;
            if s + B <= data.len() {
                out[d..d + B].copy_from_slice(&data[s..s + B]);
            }
        }
    }
    out
}

fn morton(bx: usize, by: usize) -> usize {
    let mut r = 0usize;
    for b in 0..16 {
        r |= ((bx >> b) & 1) << (2 * b);
        r |= ((by >> b) & 1) << (2 * b + 1);
    }
    r
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: probe_g4tg_swizzle <fichier>");
    let data = std::fs::read(&path).expect("lecture");
    let n = data.len();
    println!("fichier {path}  {n} octets  {} blocs de 16 o", n / B);
    if !n.is_multiple_of(B) {
        println!("  taille non multiple de 16 : pas un flux de blocs BC7 entier");
    }
    let blocs = n / B;
    for bw in (1..=blocs).filter(|b| blocs.is_multiple_of(*b)) {
        let bh = blocs / bw;
        let (w, h) = (bw * 4, bh * 4);
        if w < 16 || h < 16 || w > 8192 || h > 8192 {
            continue;
        }
        let mut ligne = format!("  {w:5}x{h:<5}");
        for (nom, buf) in [
            ("lin", data.clone()),
            ("mor", reordonner(&data, bw, bh, morton)),
            (
                "tui",
                reordonner(&data, bw, bh, |bx, by| {
                    // tuiles de 16x16 blocs (4 Kio), en ordre ligne-de-tuiles
                    let (tx, ty) = (bx / 16, by / 16);
                    let tw = bw.div_ceil(16);
                    (ty * tw + tx) * 256 + (by % 16) * 16 + (bx % 16)
                }),
            ),
        ] {
            let s = Surface {
                width: w as u32,
                height: h as u32,
                depth: 1,
                layers: 1,
                mipmaps: 1,
                image_format: ImageFormat::BC7RgbaUnorm,
                data: &buf[..],
            };
            let Ok(rgba) = s.decode_rgba8() else { continue };
            let (dh, dv) = ecarts(w, h, &rgba.data);
            ligne += &format!(
                " | {nom} dh={dh:6.2} dv={dv:6.2} r={:5.2}",
                dv / dh.max(0.01)
            );
        }
        println!("{ligne}");
    }
}
