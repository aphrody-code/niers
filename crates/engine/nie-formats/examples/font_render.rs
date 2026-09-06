//! Rendu de texte Latin depuis la police RÉELLE du jeu, via **edge-scan + mapping par ordre ASCII**.
//! Découverte 2026-06-20 : la rangée physique de l'atlas (y≈950) contient les glyphes en ORDRE de
//! codepoint contigu à partir de `!` (0x21). Donc le k-ième span physique = codepoint `0x21 + k`.
//! (L'ancien rendu via col3 métrique garblait ; les spans physiques, eux, sont propres.)
//! Usage : `cargo run -p nie-formats --example font_render -- <font.cfg.bin> <font.g4tx> "TEXTE"`

use nie_formats::{cfgbin, font, g4tx};

const FIRST_CP: u32 = 0x21; // '!'

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cfg = cfgbin::parse_t2b(&std::fs::read(&args[1]).unwrap()).unwrap();
    let metrics = font::parse_metrics(&cfg);
    let g4tx_bytes = std::fs::read(&args[2]).unwrap();
    let text = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "INAZUMA ELEVEN Victory Road - BUT 3-2".into());

    // Atlas BGRA8 mip0.
    let tx = g4tx::parse(&g4tx_bytes).unwrap();
    let t = &tx.textures[0];
    let dds = &g4tx_bytes[t.data_offset..];
    let px_off = if dds.len() >= 88 && &dds[84..88] == b"DX10" {
        148
    } else {
        128
    };
    let atlas = &dds[px_off..];
    let (aw, ah) = (t.width as usize, t.height as usize);
    let stride = aw * 4;
    let cell_h = metrics.dims.cell_height as usize;

    // Rangée physique ASCII (y de base) — réglable par env YBASE.
    let yb: usize = std::env::var("YBASE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(946);

    // EDGE-SCAN de la rangée sur toute la largeur : colonnes d'alpha → spans (x0,x1) de glyphes.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let (mut on, mut start) = (false, 0usize);
    let (y0, y1) = (yb + 4, (yb + cell_h).min(ah));
    for ax in 0..aw {
        let mut s = 0u32;
        for ay in y0..y1 {
            s += u32::from(atlas[ay * stride + ax * 4 + 3]);
        }
        let lit = s > 180;
        if lit && !on {
            start = ax;
            on = true;
        } else if !lit && on {
            // tolère 1-2 px de creux internes : on ferme un span seulement après un vrai blanc.
            spans.push((start, ax));
            on = false;
        }
    }
    if on {
        spans.push((start, aw));
    }
    // Fusionne les micro-spans séparés par <3 px (creux internes d'un même glyphe, ex. ! ou %).
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for &(a, b) in &spans {
        if let Some(last) = merged.last_mut()
            && a.saturating_sub(last.1) < 3
        {
            last.1 = b;
            continue;
        }
        merged.push((a, b));
    }
    let spans = merged;
    println!(
        "atlas {aw}×{ah}, rangée y={yb}, {} spans physiques → codepoints {:#X}..",
        spans.len(),
        FIRST_CP
    );

    // Map codepoint → span physique (x0,x1). span[k] = 0x21 + k.
    let cp_span = |cp: u32| -> Option<(usize, usize)> {
        if cp < FIRST_CP {
            return None;
        }
        spans.get((cp - FIRST_CP) as usize).copied()
    };

    // Rendu : blit chaque glyphe (région atlas physique) sur un canvas.
    let (cw, ch) = (1400u32, cell_h as u32 + 24);
    let mut canvas = vec![0u8; (cw * ch * 4) as usize];
    for p in canvas.chunks_exact_mut(4) {
        p.copy_from_slice(&[20, 26, 42, 255]);
    }
    let mut pen = 12i32;
    for c in text.chars() {
        if c == ' ' {
            pen += (cell_h / 3) as i32;
            continue;
        }
        if let Some((x0, x1)) = cp_span(c as u32) {
            let gw = x1 - x0;
            for gy in 0..cell_h {
                let ay = yb + gy;
                if ay >= ah {
                    break;
                }
                for gx in 0..gw {
                    let a = atlas[ay * stride + (x0 + gx) * 4 + 3];
                    if a < 8 {
                        continue;
                    }
                    let (px, py) = (pen + gx as i32, 12 + gy as i32);
                    if px < 0 || py < 0 || px >= cw as i32 || py >= ch as i32 {
                        continue;
                    }
                    let o = (py as usize * cw as usize + px as usize) * 4;
                    let af = f32::from(a) / 255.0;
                    for k in 0..3 {
                        let fg = [245.0, 245.0, 250.0][k];
                        canvas[o + k] = (fg * af + f32::from(canvas[o + k]) * (1.0 - af)) as u8;
                    }
                }
            }
            pen += gw as i32 + 2;
        } else {
            pen += (cell_h / 3) as i32;
        }
    }
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(std::io::Cursor::new(&mut out), cw, ch);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&canvas).unwrap();
    }
    std::fs::write("/tmp/font_text.png", &out).unwrap();
    println!("texte {text:?} → /tmp/font_text.png");
}
