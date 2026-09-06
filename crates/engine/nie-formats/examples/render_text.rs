//! **Inspecteur de l'atlas de police** (RE du pipeline texte / mode histoire).
//!
//! Usage : `cargo run -p nie-formats --example render_text -- <font.cfg.bin> <font.g4tx> <out.png>`
//!
//! ## État RE (2026-06-20) — progrès majeur, rendu texte encore PARTIEL
//! - **Décodage de l'atlas : RÉSOLU.** `font.g4tx` → 1 texture DDS `4096×2048` BGRA8 non compressé,
//!   pixels mip0 à `data_offset(176) + 128` (en-tête DDS), 3 mips. Alpha = couverture des glyphes
//!   (CJK denses en haut, ASCII vers la rangée physique y≈950, kana/symboles ≈1035).
//! - **Y logique RÉSOLU.** col[4] du record CHR = **Y de l'atlas LOGIQUE** : valeurs en rangées tous
//!   les ~73 px (1, 74, 147, 220, …). Tout l'ASCII a col[4]=1 (rangée logique 0).
//! - **Permutation des rangées.** font.g4tx **réordonne** les rangées vs l'atlas logique : la rangée
//!   ASCII (logique 0) est physiquement à y≈950 (≈ rangée 13). On rend l'ASCII en forçant ce Y
//!   (env `YBASE=950`).
//! - **X NON RÉSOLU.** En forçant YBASE=950, les CHIFFRES rendent NET et une séquence consécutive
//!   (ABCDEF…) sort dans l'ordre, mais du texte arbitraire (BUT, INAZUMA) GARBLE → l'X physique de
//!   font.g4tx ne suit pas col[3] de façon cohérente (offset par bloc/permutation X). À RE avant un
//!   rendu fidèle. Env de debug : `CROP="x0,x1,y0,y1"`, `TEXT="…" YBASE=950 XOFF=42`.
//!
//! Décodage atlas + Y = FAIT ; rendu texte = PARTIEL (chiffres OK) ; X = prochain pas RE.

use nie_formats::{cfgbin, font, g4tx};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cfg_bytes = std::fs::read(&args[1]).expect("lire font.cfg.bin");
    let g4tx_bytes = std::fs::read(&args[2]).expect("lire font.g4tx");
    let out = args
        .get(3)
        .map_or("/tmp/font_atlas_dump.png", String::as_str);

    // Métriques.
    let cfg = cfgbin::parse_t2b(&cfg_bytes).expect("parse font.cfg.bin (T2B)");
    let metrics = font::parse_metrics(&cfg);
    println!(
        "glyphes={} cell_h={} ascent={}",
        metrics.glyph_count(),
        metrics.dims.cell_height,
        metrics.dims.ascent
    );
    for cp in [65u32, 66, 97] {
        if let Some(m) = metrics.glyph(cp) {
            println!(
                "  cp={cp} ({:?}) x={} y={} w={} adv={} page={}",
                char::from_u32(cp),
                m.x,
                m.y,
                m.width,
                m.advance,
                m.page
            );
        }
    }
    // Sonde : records CHR bruts sur différents scripts (rangées atlas différentes) + histo col[4].
    {
        use nie_formats::cfgbin::Value;
        let asi = |v: &Value| match v {
            Value::Int(i) => *i,
            Value::Float(f) => *f as i32,
            Value::String(_) => -999,
        };
        let probe = [65i32, 48, 0x30A2, 0x4E9C, 0x3042, 0x30DE];
        for e in &cfg.entries {
            if e.name == "CHR"
                && e.variables.len() >= 5
                && let Value::Int(cp) = e.variables[2]
                && probe.contains(&cp)
            {
                let cols: Vec<i32> = e.variables.iter().map(asi).collect();
                println!(
                    "  CHR U+{cp:04X} ({:?}) cols={cols:?}",
                    char::from_u32(cp as u32)
                );
            }
        }
        let mut hist: std::collections::BTreeMap<i32, u32> = std::collections::BTreeMap::new();
        for e in &cfg.entries {
            if e.name == "CHR" && e.variables.len() >= 5 {
                *hist.entry(asi(&e.variables[4])).or_default() += 1;
            }
        }
        println!(
            "  col[4] : {} valeurs distinctes, échantillon={:?}",
            hist.len(),
            hist.iter().take(14).collect::<Vec<_>>()
        );
    }

    // Atlas → DDS BGRA8 mip0.
    let tx = g4tx::parse(&g4tx_bytes).expect("parse g4tx");
    let t = &tx.textures[0];
    let dds = &g4tx_bytes[t.data_offset..t.data_offset + t.data_size];
    let px_off = if dds.len() >= 88 && &dds[84..88] == b"DX10" {
        148
    } else {
        128
    };
    let atlas = &dds[px_off..];
    let (aw, ah) = (t.width as usize, t.height as usize);
    println!(
        "atlas {aw}×{ah} BGRA8 (dds@{} mip0@+{px_off}) sub_textures={}",
        t.data_offset,
        t.sub_textures.len()
    );
    for st in t.sub_textures.iter().take(6) {
        println!(
            "    sub id={} name={:?} x={} y={} w={} h={}",
            st.id, st.name, st.x, st.y, st.width, st.height
        );
    }

    // Crop ciblé pleine résolution de la zone Latin (env CROP="x0,x1,y0,y1"), alpha en gris,
    // avec une grille tous les 100 px (repères de coordonnées atlas).
    let stride = aw * 4;
    let crop: Vec<usize> = std::env::var("CROP")
        .unwrap_or_else(|_| "1000,2600,1550,1790".into())
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    let (cx0, cx1, cy0, cy1) = (crop[0], crop[1], crop[2], crop[3]);
    let (dw, dh) = ((cx1 - cx0) as u32, (cy1 - cy0) as u32);
    let mut img = vec![0u8; (dw * dh * 4) as usize];
    for (j, ay) in (cy0..cy1).enumerate() {
        for (i, ax) in (cx0..cx1).enumerate() {
            let a = atlas.get(ay * stride + ax * 4 + 3).copied().unwrap_or(0);
            let o = (j * dw as usize + i) * 4;
            // grille rouge tous les 100 px d'atlas pour lire les coordonnées.
            if ax % 100 == 0 || ay % 100 == 0 {
                img[o..o + 4].copy_from_slice(&[120, 30, 30, 255]);
            } else {
                img[o..o + 4].copy_from_slice(&[a, a, a, 255]);
            }
        }
    }
    println!("crop atlas x[{cx0},{cx1}] y[{cy0},{cy1}] (grille rouge /100 px)");

    // SCAN des bords de glyphes dans la rangée ASCII physique (y≈[955,1015]) : colonnes d'alpha
    // → spans de glyphes. Comparer aux col[3] métriques pour décoder le mapping X.
    {
        let yb: usize = std::env::var("YROW")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(950);
        let mut runs: Vec<(usize, usize)> = Vec::new();
        let (mut on, mut start) = (false, 0usize);
        for ax in 300..2700usize {
            let mut s = 0u32;
            for ay in (yb + 5)..(yb + 65) {
                s += u32::from(atlas.get(ay * stride + ax * 4 + 3).copied().unwrap_or(0));
            }
            let lit = s > 200;
            if lit && !on {
                start = ax;
                on = true;
            } else if !lit && on {
                if ax - start >= 4 {
                    runs.push((start, ax));
                }
                on = false;
            }
        }
        println!("  rangée ASCII y={yb} : {} spans de glyphes", runs.len());
        // ALIGNEMENT : glyphes ASCII visibles triés par col3 ↔ spans physiques triés par X.
        // → (char, col3 métrique, X physique, diff) pour révéler la fonction X.
        let mut glyphs: Vec<(char, i32, i32)> = (33u32..127)
            .filter_map(|cp| {
                metrics.glyph(cp).map(|m| {
                    (
                        char::from_u32(cp).unwrap(),
                        i32::from(m.x),
                        i32::from(m.width),
                    )
                })
            })
            .collect();
        glyphs.sort_by_key(|&(_, x, _)| x);
        let mut spans = runs.clone();
        spans.sort();
        println!("  alignement (char col3 → physX diff) :");
        for (k, (ch, col3, _w)) in glyphs.iter().enumerate() {
            if let Some(&(ps, _)) = spans.get(k) {
                println!("    '{ch}' col3={col3} phys={ps} diff={}", ps as i32 - col3);
            }
        }
    }

    // TEST rendu ASCII : blit une chaîne en forçant le Y physique de la rangée ASCII (env YBASE).
    if let Ok(txt) = std::env::var("TEXT") {
        let yb: u16 = std::env::var("YBASE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(946);
        let cell_h = metrics.dims.cell_height;
        let (tw, th) = (1200u32, cell_h as u32 + 30);
        let mut canvas = vec![0u8; (tw * th * 4) as usize];
        for p in canvas.chunks_exact_mut(4) {
            p.copy_from_slice(&[18, 24, 40, 255]);
        }
        let mut pen = 12i32;
        for c in txt.chars() {
            if let Some(m) = metrics.glyph(c as u32) {
                let xoff: i32 = std::env::var("XOFF")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let mut m2 = *m;
                m2.y = yb; // override : Y physique de la rangée ASCII
                m2.x = (i32::from(m.x) + xoff).clamp(0, aw as i32 - 1) as u16;
                font::glyph_blitter(
                    atlas,
                    aw as u32,
                    &m2,
                    cell_h,
                    &mut canvas,
                    tw * 4,
                    pen + i32::from(m.bearing_x),
                    14,
                    [245, 245, 250, 255],
                );
                pen += i32::from(m.advance);
            } else {
                pen += i32::from(cell_h) / 3;
            }
        }
        let mut tb = Vec::new();
        {
            let mut e = png::Encoder::new(std::io::Cursor::new(&mut tb), tw, th);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&canvas).unwrap();
        }
        std::fs::write("/tmp/text_render.png", &tb).unwrap();
        println!("rendu texte (YBASE={yb}) → /tmp/text_render.png");
    }
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(std::io::Cursor::new(&mut buf), dw, dh);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&img).unwrap();
    }
    std::fs::write(out, &buf).expect("écrire png");
    println!("atlas downscalé → {out} ({dw}×{dh})");
}
