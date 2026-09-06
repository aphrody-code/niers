//! Compose une **scène de dialogue** du mode histoire (fond, boîte de dialogue, nom du locuteur,
//! texte réel) rendue dans la VRAIE police du jeu via `font::LatinAtlas` (edge-scan).
//! Usage : `cargo run -p nie-formats --example dialogue_scene -- <font.cfg.bin> <font.g4tx> "Locuteur" "Texte du dialogue (\n autorisé)"`

use nie_formats::{cfgbin, font, g4tx};

const W: usize = 1280;
const H: usize = 720;

fn fill_rect(buf: &mut [u8], x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 4]) {
    let a = f32::from(c[3]) / 255.0;
    for y in y0.max(0)..y1.min(H as i32) {
        for x in x0.max(0)..x1.min(W as i32) {
            let o = (y as usize * W + x as usize) * 4;
            for k in 0..3 {
                buf[o + k] = (f32::from(c[k]) * a + f32::from(buf[o + k]) * (1.0 - a)) as u8;
            }
            buf[o + 3] = 255;
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cfg = cfgbin::parse_t2b(&std::fs::read(&a[1]).unwrap()).unwrap();
    let metrics = font::parse_metrics(&cfg);
    let g4tx_bytes = std::fs::read(&a[2]).unwrap();
    let speaker = a.get(3).cloned().unwrap_or_else(|| "Endou Mamoru".into());
    let text = a
        .get(4)
        .cloned()
        .unwrap_or_else(|| "Can anyone bring down Raimon's unshakable fortress?!".into());

    // Atlas BGRA8 mip0 + LatinAtlas (rangée ASCII y≈946).
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
    let cell_h = metrics.dims.cell_height;
    let la = font::LatinAtlas::from_atlas(atlas, aw, ah, 946, cell_h);

    // Canvas : fond dégradé bleu nuit (placeholder du rendu de scène 3D).
    let mut buf = vec![0u8; W * H * 4];
    for y in 0..H {
        let t = y as f32 / H as f32;
        let (r, g, b) = (
            (18.0 + 30.0 * t) as u8,
            (24.0 + 36.0 * t) as u8,
            (44.0 + 60.0 * (1.0 - t)) as u8,
        );
        for x in 0..W {
            let o = (y * W + x) * 4;
            buf[o..o + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }

    // Wrap d'abord pour dimensionner la boîte aux lignes.
    let line_h = cell_h as i32 + 4;
    let bx0 = 60i32;
    let bx1 = W as i32 - 60;
    let max_w = (bx1 - bx0 - 80) as u32;
    let mut lines: Vec<String> = Vec::new();
    for para in text.split("\\n").flat_map(|p| p.split('\n')) {
        let mut cur = String::new();
        for word in para.split_whitespace() {
            let trial = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{cur} {word}")
            };
            if la.measure(&trial) <= max_w {
                cur = trial;
            } else {
                if !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                }
                cur = word.to_string();
            }
        }
        lines.push(cur);
    }
    // Boîte dimensionnée aux lignes, ancrée en bas.
    let by1 = H as i32 - 28;
    let by0 = by1 - (lines.len() as i32 * line_h + 36);
    fill_rect(&mut buf, bx0, by0, bx1, by1, [10, 14, 28, 224]);
    fill_rect(&mut buf, bx0, by0, bx1, by0 + 3, [90, 200, 255, 255]); // liseré haut
    // Onglet nom du locuteur (au-dessus de la boîte).
    let name_w = (la.measure(&speaker) as i32 + 40).min(440);
    fill_rect(
        &mut buf,
        bx0 + 20,
        by0 - 40,
        bx0 + 20 + name_w,
        by0 + 2,
        [30, 60, 110, 238],
    );
    fill_rect(
        &mut buf,
        bx0 + 20,
        by0 - 40,
        bx0 + 20 + name_w,
        by0 - 37,
        [120, 220, 255, 255],
    );
    la.blit_line(
        atlas,
        aw,
        &mut buf,
        W,
        bx0 + 38,
        by0 - 32,
        &speaker,
        [200, 235, 255, 255],
    );
    // Texte du dialogue.
    for (i, line) in lines.iter().enumerate() {
        la.blit_line(
            atlas,
            aw,
            &mut buf,
            W,
            bx0 + 40,
            by0 + 22 + i as i32 * line_h,
            line,
            [240, 244, 250, 255],
        );
    }
    // Indicateur « suite » (petit carré cyan en bas à droite).
    fill_rect(
        &mut buf,
        bx1 - 36,
        by1 - 24,
        bx1 - 20,
        by1 - 8,
        [120, 220, 255, 255],
    );

    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(std::io::Cursor::new(&mut out), W as u32, H as u32);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&buf).unwrap();
    }
    std::fs::write("/tmp/dialogue_scene.png", &out).unwrap();
    println!(
        "scène : locuteur={speaker:?} {} lignes → /tmp/dialogue_scene.png",
        lines.len()
    );
}
