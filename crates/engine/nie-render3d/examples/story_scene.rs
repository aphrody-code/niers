//! Scène de mode histoire COMPLÈTE : perso 3D réel (mesh skinné + texture BC7) rendu en perspective,
//! surmonté d'une boîte de dialogue + texte dans la VRAIE police (`font::LatinAtlas`).
//! Usage : `cargo run -p nie-render3d --example story_scene -- <md> <mg> <g4sk> <g4tx> <font.cfg> <font.g4tx> "Locuteur" "Texte"`

use nie_formats::{cfgbin, font, g4md, g4mg, g4sk, g4tx};
use nie_render3d::glb::{Model, Primitive, Texture};
use nie_render3d::scene::{self, Camera, Instance};

const W: usize = 1280;
const H: usize = 720;

fn xf(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

fn decode_bc7(g4tx_bytes: &[u8]) -> Texture {
    let tx = g4tx::parse(g4tx_bytes).unwrap();
    let t = &tx.textures[0];
    let dds = &g4tx_bytes[t.data_offset..];
    let off = if dds.len() >= 88 && &dds[84..88] == b"DX10" {
        148
    } else {
        128
    };
    let data = &dds[off..];
    let (w, h) = (t.width as usize, t.height as usize);
    let (bw, bh) = (w / 4, h / 4);
    let mut rgba = vec![0u8; w * h * 4];
    let mut blk = [0u8; 64];
    for by in 0..bh {
        for bx in 0..bw {
            let o = (by * bw + bx) * 16;
            if o + 16 > data.len() {
                break;
            }
            bcdec_rs::bc7(&data[o..o + 16], &mut blk, 16);
            for ry in 0..4 {
                for rx in 0..4 {
                    let d = ((by * 4 + ry) * w + bx * 4 + rx) * 4;
                    let s = ry * 16 + rx * 4;
                    rgba[d..d + 4].copy_from_slice(&blk[s..s + 4]);
                }
            }
        }
    }
    Texture {
        width: w as u32,
        height: h as u32,
        rgba,
    }
}

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
    let md = g4md::parse(&std::fs::read(&a[1]).unwrap()).unwrap();
    let mg = std::fs::read(&a[2]).unwrap();
    let sk = std::fs::read(&a[3]).unwrap();
    let tex = decode_bc7(&std::fs::read(&a[4]).unwrap());
    let cfg = cfgbin::parse_t2b(&std::fs::read(&a[5]).unwrap()).unwrap();
    let metrics = font::parse_metrics(&cfg);
    let font_g4tx = std::fs::read(&a[6]).unwrap();
    let speaker = a.get(7).cloned().unwrap_or_else(|| "Endou Mamoru".into());
    let text = a
        .get(8)
        .cloned()
        .unwrap_or_else(|| "Let's give it everything we've got!".into());

    // --- 1) Rendu 3D du perso (pose de repos skinnée) ---
    let geo = &g4mg::extract_geometry(&mg, &md)[0];
    let skin = g4mg::extract_skin(&mg, &md, 0).unwrap();
    let pos: Vec<[f32; 3]> = geo.positions.iter().map(|p| [p.x, p.y, p.z]).collect();
    let uv: Vec<[f32; 2]> = geo.uv0.iter().map(|u| [u.u, u.v]).collect();
    let header = g4sk::parse_header(&sk).unwrap();
    let bones = g4sk::parse_hierarchy(&sk, &header);
    let poses = g4sk::parse_poses(&sk, &header).unwrap();
    let parents: Vec<i16> = bones.bones.iter().map(|b| b.parent_index).collect();
    let nb = poses.len();
    let world = g4sk::rest_world_matrices(&poses, &parents);
    let skinm: Vec<[[f32; 4]; 4]> = (0..nb)
        .map(|i| g4sk::mat_mul(&world[i], &poses[i].inverse_bind))
        .collect();
    let sp: Vec<[f32; 3]> = (0..pos.len())
        .map(|v| {
            let s = &skin[v];
            let (mut acc, mut wsum) = ([0.0f32; 3], 0.0f32);
            for k in 0..8 {
                let (w, b) = (s.weights[k], s.bones[k] as usize);
                if w <= 0.0 || b >= nb {
                    continue;
                }
                let tp = xf(&skinm[b], pos[v]);
                for j in 0..3 {
                    acc[j] += w * tp[j];
                }
                wsum += w;
            }
            if wsum > 0.0 {
                [acc[0] / wsum, acc[1] / wsum, acc[2] / wsum]
            } else {
                pos[v]
            }
        })
        .collect();
    let has_uv = uv.len() == pos.len();
    let prim = Primitive {
        positions: sp,
        normals: Vec::new(),
        uv: if has_uv { uv } else { Vec::new() },
        indices: geo.indices.clone(),
        texture: if has_uv { Some(0) } else { None },
    };
    let model = Model {
        primitives: vec![prim],
        textures: vec![tex],
    };
    // Perso décalé à droite (l'espace gauche respire), regard haut du torse.
    let inst = Instance {
        model: &model,
        transform: scene::mat_translate([0.55, 0.0, 0.0]),
        two_sided: true,
    };
    let cam = Camera {
        eye: [0.0, 1.05, 2.6],
        target: [0.35, 0.95, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_y: 0.62,
    };
    let mut buf = scene::render_scene(
        &[],
        &[inst],
        &cam,
        W as u32,
        H as u32,
        [26, 32, 56],
        [10, 12, 22],
    );

    // --- 2) Boîte de dialogue + texte (LatinAtlas) ---
    let tx = g4tx::parse(&font_g4tx).unwrap();
    let ft = &tx.textures[0];
    let fdds = &font_g4tx[ft.data_offset..];
    let foff = if fdds.len() >= 88 && &fdds[84..88] == b"DX10" {
        148
    } else {
        128
    };
    let fatlas = &fdds[foff..];
    let (faw, fah) = (ft.width as usize, ft.height as usize);
    let cell_h = metrics.dims.cell_height;
    let la = font::LatinAtlas::from_atlas(fatlas, faw, fah, 946, cell_h);

    let (bx0, bx1) = (60i32, W as i32 - 60);
    let line_h = cell_h as i32 + 4;
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
    let by1 = H as i32 - 28;
    let by0 = by1 - (lines.len() as i32 * line_h + 36);
    fill_rect(&mut buf, bx0, by0, bx1, by1, [10, 14, 28, 220]);
    fill_rect(&mut buf, bx0, by0, bx1, by0 + 3, [90, 200, 255, 255]);
    let name_w = (la.measure(&speaker) as i32 + 40).min(440);
    fill_rect(
        &mut buf,
        bx0 + 20,
        by0 - 40,
        bx0 + 20 + name_w,
        by0 + 2,
        [30, 60, 110, 235],
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
        fatlas,
        faw,
        &mut buf,
        W,
        bx0 + 38,
        by0 - 32,
        &speaker,
        [200, 235, 255, 255],
    );
    for (i, line) in lines.iter().enumerate() {
        la.blit_line(
            fatlas,
            faw,
            &mut buf,
            W,
            bx0 + 40,
            by0 + 22 + i as i32 * line_h,
            line,
            [240, 244, 250, 255],
        );
    }
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
    std::fs::write("/tmp/story_scene.png", &out).unwrap();
    println!(
        "story scene : {speaker:?}, {} lignes, perso 3D + boîte → /tmp/story_scene.png",
        lines.len()
    );
}
