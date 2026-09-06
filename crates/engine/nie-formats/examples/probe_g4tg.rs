//! Sonde une charge utile de texture BRUTE, sans en-tête (typiquement un `.g4tg`).
//!
//! Fait trois choses : (1) l'histogramme des **modes BC7** — test de validité indépendant
//! de toute dimension ; (2) l'essai de chaque couple (format de bloc, largeur, hauteur)
//! compatible avec la taille du fichier ; (3) une note de « douceur » (écart moyen entre
//! pixels voisins) horizontale `dh` et verticale `dv`.
//!
//! ## Étalonnage (mesuré le 2026-09-06, deux témoins de 65 536 o)
//!
//! - témoin **BC7 vrai** (charge utile 256x256 extraite de `ef_ev01_02800_c00100.g4tx`) :
//!   modes invalides **0,17 %**, et le rang 0 du classement est bien `BC7 256x256`
//!   (`dh=1,30 dv=1,24`, rapport `dv/dh = 0,95`) ; toute autre largeur donne `dv >= 2,0`.
//! - témoin **bruit** (`/dev/urandom`) : `dh = 53,9` quelle que soit la largeur, et BC7
//!   n'apparaît pas dans les 12 premiers.
//!
//! **Le rapport `dv/dh` seul ne prouve rien** : le bruit donne aussi `dv/dh = 1,00`. Le
//! couple discriminant est (`dh` bas, `dv/dh` proche de 1).
//!
//! ## Le mode BC7 est le test le plus fort
//!
//! Un bloc BC7 encode son mode dans le bit de poids faible allumé du 1er octet : un premier
//! octet **nul est un mode invalide** qu'aucun encodeur ne produit. Mesure sur les 9 `.g4tg`
//! du VFS : 8 en portent **74,6 % à 93,8 %**, ce qui les exclut de BC7. Le neuvième,
//! `ef_ev01_02800_c00100.g4tg`, en porte **0,00 %** sur 4 096 blocs, avec un histogramme de
//! modes de même forme que son témoin (modes 5 et 6 dominants) : celui-là **est** du BC7.
//!
//! Usage : `cargo run -p nie-formats --features images,textures --example probe_g4tg -- <f> [dossier_png]`

use image_dds::{ImageFormat, Surface};

/// Histogramme des modes BC7 : indice 0..7 = mode, indice 8 = **mode invalide**
/// (premier octet nul, qu'aucun encodeur BC7 ne produit).
fn modes_bc7(data: &[u8]) -> [usize; 9] {
    let mut h = [0usize; 9];
    for bloc in data.chunks_exact(16) {
        let b0 = bloc[0];
        let m = if b0 == 0 { 8 } else { b0.trailing_zeros() as usize };
        h[m.min(8)] += 1;
    }
    h
}

fn score(w: u32, h: u32, rgba: &[u8]) -> (f64, f64) {
    let (w, h) = (w as usize, h as usize);
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
                    sh += 0.0;
                    sv += f64::from(rgba[i + c].abs_diff(rgba[i + w * 4 + c]));
                }
                nv += 3;
            }
        }
    }
    #[allow(clippy::cast_precision_loss)]
    (sh / nh.max(1) as f64, sv / nv.max(1) as f64)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: probe_g4tg <fichier> [dossier_png]");
    let outdir = args.next();
    let data = std::fs::read(&path).expect("lecture");
    let n = data.len();
    println!("fichier {path}  {n} octets");
    let h = modes_bc7(&data);
    let blocs: usize = h.iter().sum();
    if blocs > 0 {
        #[allow(clippy::cast_precision_loss)]
        let pct = 100.0 * h[8] as f64 / blocs as f64;
        println!("  modes BC7 sur {blocs} blocs : 0..7 = {:?}, INVALIDE = {} ({pct:.2} %)",
                 &h[..8], h[8]);
    }

    let formats: &[(&str, ImageFormat, usize, usize)] = &[
        ("BC1", ImageFormat::BC1RgbaUnorm, 8, 16),
        ("BC3", ImageFormat::BC3RgbaUnorm, 16, 16),
        ("BC4", ImageFormat::BC4RUnorm, 8, 16),
        ("BC5", ImageFormat::BC5RgUnorm, 16, 16),
        ("BC6", ImageFormat::BC6hRgbUfloat, 16, 16),
        ("BC7", ImageFormat::BC7RgbaUnorm, 16, 16),
        ("RGBA8", ImageFormat::Rgba8Unorm, 4, 1),
    ];

    let mut res = Vec::new();
    for &(nom, fmt, bsize, px_per_block) in formats {
        let px_side = if px_per_block == 16 { 4usize } else { 1 };
        // nombre total de pixels representables par TOUT le fichier
        if !n.is_multiple_of(bsize) {
            continue;
        }
        let blocks = n / bsize;
        // toutes les factorisations en (bw, bh) puissances de deux ou multiples raisonnables
        for bw in 1..=blocks {
            if !blocks.is_multiple_of(bw) {
                continue;
            }
            let bh = blocks / bw;
            let (w, h) = (bw * px_side, bh * px_side);
            if w < 8 || h < 8 || w > 8192 || h > 8192 {
                continue;
            }
            // on se limite aux largeurs multiples de 8 pour ne pas exploser
            if !w.is_multiple_of(8) || !h.is_multiple_of(8) {
                continue;
            }
            let surface = Surface {
                width: w as u32,
                height: h as u32,
                depth: 1,
                layers: 1,
                mipmaps: 1,
                image_format: fmt,
                data: &data[..],
            };
            let Ok(rgba) = surface.decode_rgba8() else {
                continue;
            };
            let (sh, sv) = score(w as u32, h as u32, &rgba.data);
            res.push((sh + sv, sh, sv, nom, w, h, rgba.data));
        }
    }
    res.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    println!("  {} interpretations testees", res.len());
    for (i, (tot, sh, sv, nom, w, h, rgba)) in res.iter().take(12).enumerate() {
        println!("  #{i:2} {nom:5} {w:5}x{h:<5} dh={sh:6.2} dv={sv:6.2} total={tot:6.2}");
        if let Some(d) = &outdir {
            let f = std::path::Path::new(d).join(format!(
                "{}_{i:02}_{nom}_{w}x{h}.png",
                std::path::Path::new(&path)
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
            ));
            if let Some(png) =
                nie_formats::g4tx_decode::encode_rgba_to_png(rgba, *w, *h)
            {
                let _ = std::fs::write(f, png);
            }
        }
    }
}
