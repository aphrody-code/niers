//! Anime un squelette g4sk avec une animation G4MT et rend le squelette (os = lignes) en frames PNG.
//! Valide visuellement toute la chaîne : g4sk (poses) + g4mt (quaternions) + FK animée.
//! Usage : `cargo run -p nie-formats --example anim_skeleton -- <body.g4sk> <walk.g4pk> <outdir>`

use nie_formats::{g4mt, g4pk, g4sk};

const W: usize = 480;
const H: usize = 640;

fn put(buf: &mut [u8], x: i32, y: i32, col: [u8; 3]) {
    if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
        return;
    }
    let o = (y as usize * W + x as usize) * 3;
    buf[o..o + 3].copy_from_slice(&col);
}

fn line(buf: &mut [u8], a: (i32, i32), b: (i32, i32), col: [u8; 3]) {
    let (mut x0, mut y0) = a;
    let (x1, y1) = b;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        put(buf, x0, y0, col);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Multiplie une matrice col-major 4×4 par un point (x,y,z,1).
fn xform(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let skel_bytes = std::fs::read(&args[1]).expect("g4sk");
    let pk_bytes = std::fs::read(&args[2]).expect("g4pk");
    let outdir = args.get(3).map_or("/tmp/anim", String::as_str);
    std::fs::create_dir_all(outdir).unwrap();

    let header = g4sk::parse_header(&skel_bytes).unwrap();
    let bones = g4sk::parse_hierarchy(&skel_bytes, &header);
    let poses = g4sk::parse_poses(&skel_bytes, &header).unwrap();
    let parents: Vec<i16> = bones.bones.iter().map(|b| b.parent_index).collect();
    let n = poses.len();

    let pk = g4pk::parse(&pk_bytes).unwrap();
    let f = pk.files.iter().find(|f| f.name.ends_with(".g4mt")).unwrap();
    let g4mt_bytes = &pk_bytes[f.offset..f.offset + f.size];
    let motion = g4mt::Motion::parse(g4mt_bytes).unwrap();
    let clip = &motion.clips[0];

    // Mapping correct : cible → hash CRC32 du nom d'os, résolu contre le G4SK réel (remplace
    // l'ancienne heuristique « canal i → os i »/BASE, invalidée par le format structurel réel).
    let bone_names: Vec<&str> = bones.bones.iter().map(|b| b.name.as_str()).collect();
    let resolved = g4mt::resolve_targets(&motion.target_hashes, &bone_names);
    let mut bone_target: Vec<Option<u16>> = vec![None; n];
    for t in motion.target_indices(clip) {
        if let Some(Some(b)) = resolved.get(t as usize) {
            bone_target[*b] = Some(t);
        }
    }
    println!(
        "os={n} clip=\"{}\" frames={} cibles_résolues={}/{}",
        clip.name,
        clip.frame_count(),
        resolved.iter().filter(|r| r.is_some()).count(),
        motion.target_hashes.len()
    );

    // Bornes (depuis la pose de repos monde) pour cadrer la projection.
    let rest_world = g4sk::rest_world_matrices(&poses, &parents);
    let (mut miny, mut maxy, mut minx, mut maxx) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for m in &rest_world {
        let (x, y) = (m[3][0], m[3][1]);
        miny = miny.min(y);
        maxy = maxy.max(y);
        minx = minx.min(x);
        maxx = maxx.max(x);
    }
    let span = (maxy - miny).max(maxx - minx).max(0.1);
    let scale = (H as f32 * 0.8) / span;
    let (cx, cy) = (minx + (maxx - minx) * 0.5, miny + (maxy - miny) * 0.5);
    let proj = |p: [f32; 3]| -> (i32, i32) {
        let sx = (W as f32 * 0.5 + (p[0] - cx) * scale) as i32;
        let sy = (H as f32 * 0.5 - (p[1] - cy) * scale) as i32;
        (sx, sy)
    };

    for frame in 0..clip.frame_count() as usize {
        // FK animée : local = TRS de repos avec rotation remplacée par le quaternion échantillonné
        // (interpolation SLERP entre clés voisines — le format est keyframe, pas dense par frame).
        let mut world: Vec<[[f32; 4]; 4]> = Vec::with_capacity(n);
        for i in 0..n {
            let mut trs = poses[i].local;
            if let Some(t) = bone_target[i]
                && let Some(q) = motion.sample_rotation(g4mt_bytes, clip, t, frame as f32)
            {
                trs.quat = q;
            }
            let local = g4sk::local_matrix(&trs);
            let par = parents[i];
            let m = if par < 0 || (par as usize) >= i {
                local
            } else {
                g4sk::mat_mul(&world[par as usize], &local)
            };
            world.push(m);
        }

        let mut buf = vec![12u8; W * H * 3];
        for i in 0..n {
            let pj = proj(xform(&world[i], [0.0, 0.0, 0.0]));
            let par = parents[i];
            if par >= 0 && (par as usize) < n {
                let pp = proj(xform(&world[par as usize], [0.0, 0.0, 0.0]));
                let lit = bone_target[i].is_some();
                line(
                    &mut buf,
                    pp,
                    pj,
                    if lit { [90, 200, 255] } else { [70, 70, 90] },
                );
            }
            // articulation.
            for dy in -1..=1 {
                for dx in -1..=1 {
                    put(&mut buf, pj.0 + dx, pj.1 + dy, [255, 230, 120]);
                }
            }
        }
        let path = format!("{outdir}/f{frame:03}.png");
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(std::io::Cursor::new(&mut out), W as u32, H as u32);
            e.set_color(png::ColorType::Rgb);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&buf).unwrap();
        }
        std::fs::write(&path, &out).unwrap();
    }
    println!("frames → {outdir}/f###.png (ffmpeg -i {outdir}/f%03d.png anim.mp4)");
}
