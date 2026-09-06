//! Rendu TEXTURÉ d'un personnage skinné animé : mesh (g4md/g4mg) déformé par un squelette g4sk
//! animé d'un g4mt, rasterisé texturé (BC7 décodé du g4tx) + z-buffer → frames PNG.
//! Usage : `cargo run -p nie-render3d --example anim_char -- <md> <mg> <g4sk> <g4pk> <g4tx> <outdir>`

use nie_formats::{g4md, g4mg, g4mt, g4pk, g4sk, g4tx};
use nie_render3d::glb::{Model, Primitive, Texture};
use nie_render3d::scene::{self, Camera, Instance};

fn xf(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

/// Décode la première texture BC7 (DXGI 98) d'un g4tx en RGBA8.
fn decode_texture(g4tx_bytes: &[u8]) -> Texture {
    let tx = g4tx::parse(g4tx_bytes).expect("parse g4tx");
    let t = &tx.textures[0];
    let dds = &g4tx_bytes[t.data_offset..];
    let px_off = if dds.len() >= 88 && &dds[84..88] == b"DX10" {
        148
    } else {
        128
    };
    let data = &dds[px_off..];
    let (w, h) = (t.width as usize, t.height as usize);
    let (bw, bh) = (w / 4, h / 4);
    let mut rgba = vec![0u8; w * h * 4];
    let mut block = [0u8; 64];
    for by in 0..bh {
        for bx in 0..bw {
            let off = (by * bw + bx) * 16;
            if off + 16 > data.len() {
                break;
            }
            bcdec_rs::bc7(&data[off..off + 16], &mut block, 16);
            for ry in 0..4 {
                for rx in 0..4 {
                    let dst = ((by * 4 + ry) * w + bx * 4 + rx) * 4;
                    let src = ry * 16 + rx * 4;
                    rgba[dst..dst + 4].copy_from_slice(&block[src..src + 4]);
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

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let md = g4md::parse(&std::fs::read(&a[1]).unwrap()).unwrap();
    let mg = std::fs::read(&a[2]).unwrap();
    let sk_bytes = std::fs::read(&a[3]).unwrap();
    let pk_bytes = std::fs::read(&a[4]).unwrap();
    let tex = decode_texture(&std::fs::read(&a[5]).unwrap());
    let outdir = a.get(6).map_or("/tmp/animchar", String::as_str);
    std::fs::create_dir_all(outdir).unwrap();

    let geo = &g4mg::extract_geometry(&mg, &md)[0];
    let skin = g4mg::extract_skin(&mg, &md, 0).unwrap();
    let pos: Vec<[f32; 3]> = geo.positions.iter().map(|p| [p.x, p.y, p.z]).collect();
    let uv: Vec<[f32; 2]> = geo.uv0.iter().map(|u| [u.u, u.v]).collect();
    let idx: Vec<u32> = geo.indices.clone();

    let header = g4sk::parse_header(&sk_bytes).unwrap();
    let bones = g4sk::parse_hierarchy(&sk_bytes, &header);
    let poses = g4sk::parse_poses(&sk_bytes, &header).unwrap();
    let parents: Vec<i16> = bones.bones.iter().map(|b| b.parent_index).collect();
    let nb = poses.len();

    let pk = g4pk::parse(&pk_bytes).unwrap();
    let f = pk.files.iter().find(|f| f.name.ends_with(".g4mt")).unwrap();
    let g4mt_bytes = &pk_bytes[f.offset..f.offset + f.size];
    let motion = g4mt::Motion::parse(g4mt_bytes).unwrap();
    let clip_selector: usize = std::env::var("CLIP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let clip = &motion.clips[clip_selector.min(motion.clips.len() - 1)];

    // Mapping correct : cible → hash CRC32 du nom d'os, résolu contre le G4SK réel (remplace
    // l'ancienne heuristique manuelle `BASE`).
    let bone_names: Vec<&str> = bones.bones.iter().map(|b| b.name.as_str()).collect();
    let resolved = g4mt::resolve_targets(&motion.target_hashes, &bone_names);
    let mut bone_target: Vec<Option<u16>> = vec![None; nb];
    for t in motion.target_indices(clip) {
        if let Some(Some(b)) = resolved.get(t as usize) {
            bone_target[*b] = Some(t);
        }
    }
    let has_uv = uv.len() == pos.len();
    println!(
        "verts={} tris={} os={nb} clip=\"{}\" frames={} uv={has_uv} tex={}x{}",
        pos.len(),
        idx.len() / 3,
        clip.name,
        clip.frame_count(),
        tex.width,
        tex.height
    );

    let cam = Camera {
        eye: [0.0, 1.0, 3.2],
        target: [0.0, 0.95, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_y: 0.6,
    };

    for frame in 0..clip.frame_count() as usize {
        let mut world: Vec<[[f32; 4]; 4]> = Vec::with_capacity(nb);
        for i in 0..nb {
            let mut trs = poses[i].local;
            if let Some(t) = bone_target[i]
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

        let prim = Primitive {
            positions: sp,
            normals: Vec::new(),
            uv: if has_uv { uv.clone() } else { Vec::new() },
            indices: idx.clone(),
            texture: if has_uv { Some(0) } else { None },
        };
        let model = Model {
            primitives: vec![prim],
            textures: vec![tex_clone(&tex)],
        };
        let inst = Instance {
            model: &model,
            transform: scene::mat_identity(),
            two_sided: true,
        };
        let px = scene::render_scene(&[], &[inst], &cam, 480, 640, [30, 36, 54], [12, 14, 22]);
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(std::io::Cursor::new(&mut out), 480, 640);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&px).unwrap();
        }
        std::fs::write(format!("{outdir}/f{frame:03}.png"), &out).unwrap();
    }
    println!("→ {outdir}/f###.png");
}

fn tex_clone(t: &Texture) -> Texture {
    Texture {
        width: t.width,
        height: t.height,
        rgba: t.rgba.clone(),
    }
}
