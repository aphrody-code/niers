//! Skinning : déforme un mesh (g4md/g4mg) par un squelette g4sk animé d'un g4mt. Rend des frames PNG.
//! Usage : `cargo run -p nie-formats --example anim_mesh -- <mesh.g4md> <mesh.g4mg> <body.g4sk> <walk.g4pk> <outdir>`
//!
//! Hypothèses (à valider visuellement) : indices d'os du mesh = GLOBAUX (g4sk) ; mapping canal→os =
//! ordre du squelette. Le bind-pose (frame "bind") doit reproduire le mesh ; l'animé montre la déformation.

use nie_formats::{g4md, g4mg, g4mt, g4pk, g4sk};

const W: usize = 480;
const H: usize = 640;

fn xform(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let md = g4md::parse(&std::fs::read(&a[1]).unwrap()).unwrap();
    let mg = std::fs::read(&a[2]).unwrap();
    let sk_bytes = std::fs::read(&a[3]).unwrap();
    let pk_bytes = std::fs::read(&a[4]).unwrap();
    let outdir = a.get(5).map_or("/tmp/animmesh", String::as_str);
    std::fs::create_dir_all(outdir).unwrap();

    let geo = &g4mg::extract_geometry(&mg, &md)[0];
    let skin = g4mg::extract_skin(&mg, &md, 0).unwrap();
    let positions: Vec<[f32; 3]> = geo.positions.iter().map(|p| [p.x, p.y, p.z]).collect();
    let nverts = positions.len().min(skin.len());

    let header = g4sk::parse_header(&sk_bytes).unwrap();
    let bones = g4sk::parse_hierarchy(&sk_bytes, &header);
    let poses = g4sk::parse_poses(&sk_bytes, &header).unwrap();
    let parents: Vec<i16> = bones.bones.iter().map(|b| b.parent_index).collect();
    let nb = poses.len();

    let pk = g4pk::parse(&pk_bytes).unwrap();
    let f = pk.files.iter().find(|f| f.name.ends_with(".g4mt")).unwrap();
    let g4mt_bytes = &pk_bytes[f.offset..f.offset + f.size];
    let motion = g4mt::Motion::parse(g4mt_bytes).unwrap();
    let clip = &motion.clips[0];

    // Mapping correct : cible → hash CRC32 du nom d'os, résolu contre le G4SK réel (remplace
    // l'ancienne hypothèse « canal i → os i »).
    let bone_names: Vec<&str> = bones.bones.iter().map(|b| b.name.as_str()).collect();
    let resolved = g4mt::resolve_targets(&motion.target_hashes, &bone_names);
    let mut bone_target: Vec<Option<u16>> = vec![None; nb];
    for t in motion.target_indices(clip) {
        if let Some(Some(b)) = resolved.get(t as usize) {
            bone_target[*b] = Some(t);
        }
    }
    println!(
        "verts={nverts} os={nb} clip=\"{}\" frames={}",
        clip.name,
        clip.frame_count()
    );

    // Cadre depuis la bind-pose du mesh.
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in positions.iter().take(nverts) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let span = (hi[1] - lo[1]).max(hi[0] - lo[0]).max(0.1);
    let scale = (H as f32 * 0.85) / span;
    let ctr = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5];
    let proj = |p: [f32; 3]| -> (i32, i32) {
        (
            (W as f32 * 0.5 + (p[0] - ctr[0]) * scale) as i32,
            (H as f32 * 0.5 - (p[1] - ctr[1]) * scale) as i32,
        )
    };

    // "frame" -1 = bind pose (référence), 0..N = animé.
    for frame in -1..clip.frame_count() as i32 {
        // Matrices monde par os.
        let mut world: Vec<[[f32; 4]; 4]> = Vec::with_capacity(nb);
        for i in 0..nb {
            let mut trs = poses[i].local;
            if frame >= 0
                && let Some(t) = bone_target[i]
                && let Some(q) = motion.sample_rotation(g4mt_bytes, clip, t, frame as f32)
            {
                trs.quat = q;
            }
            let local = g4sk::local_matrix(&trs);
            let par = parents[i];
            world.push(if par < 0 || (par as usize) >= i {
                local
            } else {
                g4sk::mat_mul(&world[par as usize], &local)
            });
        }
        // skin matrix = world_animé · inverse_bind.
        let skinm: Vec<[[f32; 4]; 4]> = (0..nb)
            .map(|i| g4sk::mat_mul(&world[i], &poses[i].inverse_bind))
            .collect();

        let mut buf = vec![10u8; W * H * 3];
        for v in 0..nverts {
            let s = &skin[v];
            // position skinnée = Σ w_k · skinm[bone_k] · pos.
            let mut acc = [0.0f32; 3];
            let mut wsum = 0.0f32;
            for k in 0..8 {
                let w = s.weights[k];
                if w <= 0.0 {
                    continue;
                }
                let b = s.bones[k] as usize;
                if b >= nb {
                    continue;
                }
                let tp = xform(&skinm[b], positions[v]);
                for j in 0..3 {
                    acc[j] += w * tp[j];
                }
                wsum += w;
            }
            if wsum <= 0.0 {
                continue;
            }
            let (px, py) = proj([acc[0] / wsum, acc[1] / wsum, acc[2] / wsum]);
            if px >= 0 && py >= 0 && px < W as i32 && py < H as i32 {
                let o = (py as usize * W + px as usize) * 3;
                buf[o..o + 3].copy_from_slice(&[150, 220, 255]);
            }
        }
        let name = if frame < 0 {
            "bind".to_string()
        } else {
            format!("f{frame:03}")
        };
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(std::io::Cursor::new(&mut out), W as u32, H as u32);
            e.set_color(png::ColorType::Rgb);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&buf).unwrap();
        }
        std::fs::write(format!("{outdir}/{name}.png"), &out).unwrap();
    }
    println!("→ {outdir}/bind.png (référence) + f###.png (animé)");
}
