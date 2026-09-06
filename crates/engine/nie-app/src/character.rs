//! Rendu 3D des écrans du jeu : **personnage** skinné texturé (toile de fond menu/histoire) et
//! **scène de match** (joueur 3D sur une pelouse). Réutilise la chaîne validée : g4sk poses +
//! skinning + texture BC7 + `nie_render3d::scene::render_scene`. Pose de repos (pas d'animation ici).

use anyhow::{Context, Result};
use nie_formats::{g4md, g4mg, g4sk};
use nie_render3d::glb::{Model, Primitive, Texture};
use nie_render3d::scene::{self, Camera, Instance, Tri};

fn xf(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

fn decode_bc7(bytes: &[u8]) -> Result<Texture> {
    use nie_formats::g4tx;
    let tx = g4tx::parse(bytes).map_err(|e| anyhow::anyhow!("g4tx: {e:?}"))?;
    let t = tx.textures.first().context("texture")?;
    let dds = &bytes[t.data_offset..];
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
                    rgba[d..d + 4].copy_from_slice(&blk[ry * 16 + rx * 4..ry * 16 + rx * 4 + 4]);
                }
            }
        }
    }
    Ok(Texture {
        width: w as u32,
        height: h as u32,
        rgba,
    })
}

/// Charge un perso en `Model` texturé, pose de repos skinnée (mesh+skin+BC7+FK).
pub fn build_skinned_model(
    md_bytes: &[u8],
    mg: &[u8],
    sk: &[u8],
    tex_bytes: &[u8],
) -> Result<Model> {
    let md = g4md::parse(md_bytes).map_err(|e| anyhow::anyhow!("g4md: {e:?}"))?;
    let geo = g4mg::extract_geometry(mg, &md);
    let geo = geo.first().context("géométrie")?;
    let skin = g4mg::extract_skin(mg, &md, 0).context("skin")?;
    let pos: Vec<[f32; 3]> = geo.positions.iter().map(|p| [p.x, p.y, p.z]).collect();
    let uv: Vec<[f32; 2]> = geo.uv0.iter().map(|u| [u.u, u.v]).collect();
    let tex = decode_bc7(tex_bytes)?;

    let header = g4sk::parse_header(sk).map_err(|e| anyhow::anyhow!("g4sk: {e:?}"))?;
    let bones = g4sk::parse_hierarchy(sk, &header);
    let poses = g4sk::parse_poses(sk, &header).context("poses g4sk")?;
    let parents: Vec<i16> = bones.bones.iter().map(|b| b.parent_index).collect();
    let world = g4sk::rest_world_matrices(&poses, &parents);
    let nb = poses.len();
    let skinm: Vec<[[f32; 4]; 4]> = (0..nb)
        .map(|i| g4sk::mat_mul(&world[i], &poses[i].inverse_bind))
        .collect();

    let sp: Vec<[f32; 3]> = (0..pos.len())
        .map(|v| {
            let s = &skin[v];
            let (mut acc, mut wsum) = ([0.0f32; 3], 0.0f32);
            for k in 0..8 {
                let (wt, b) = (s.weights[k], s.bones[k] as usize);
                if wt <= 0.0 || b >= nb {
                    continue;
                }
                let tp = xf(&skinm[b], pos[v]);
                for j in 0..3 {
                    acc[j] += wt * tp[j];
                }
                wsum += wt;
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
    Ok(Model {
        primitives: vec![prim],
        textures: vec![tex],
    })
}

/// Rend le personnage (pose de repos) sur un cadre `w×h`, fond dégradé. `x_off` le décale.
pub fn render_character(
    model: &Model,
    w: u32,
    h: u32,
    x_off: f32,
    bg_top: [u8; 3],
    bg_bot: [u8; 3],
) -> Vec<u8> {
    let inst = Instance {
        model,
        transform: scene::mat_translate([x_off, 0.0, 0.0]),
        two_sided: true,
    };
    let cam = Camera {
        eye: [0.0, 1.05, 2.7],
        target: [x_off * 0.6, 0.95, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_y: 0.62,
    };
    scene::render_scene(&[], &[inst], &cam, w, h, bg_top, bg_bot)
}

/// Quad plat (2 triangles) sur le plan y=0, couleur `c`.
fn ground_quad(x0: f32, z0: f32, x1: f32, z1: f32, c: [u8; 3]) -> [Tri; 2] {
    let p = |x: f32, z: f32| [x, 0.0, z];
    [
        Tri {
            p: [p(x0, z0), p(x1, z0), p(x1, z1)],
            color: c,
        },
        Tri {
            p: [p(x0, z0), p(x1, z1), p(x0, z1)],
            color: c,
        },
    ]
}

/// Rend une **scène de match 3D** : pelouse rayée + ligne médiane + le joueur 3D dessus.
pub fn render_match_scene(model: &Model, w: u32, h: u32) -> Vec<u8> {
    let mut pitch: Vec<Tri> = Vec::new();
    // Pelouse rayée (bandes alternées de vert).
    for i in -4i32..4 {
        let (z0, z1) = (i as f32 * 1.6, (i + 1) as f32 * 1.6);
        let c = if i.rem_euclid(2) == 0 {
            [34, 110, 48]
        } else {
            [28, 96, 42]
        };
        pitch.extend(ground_quad(-8.0, z0, 8.0, z1, c));
    }
    // Ligne médiane blanche.
    pitch.extend(ground_quad(-8.0, -0.1, 8.0, 0.1, [220, 230, 220]));
    // Le joueur, debout au centre.
    let inst = Instance {
        model,
        transform: scene::mat_identity(),
        two_sided: true,
    };
    let cam = Camera {
        eye: [2.2, 2.0, 3.6],
        target: [0.0, 0.9, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_y: 0.7,
    };
    scene::render_scene(&pitch, &[inst], &cam, w, h, [120, 170, 230], [70, 110, 170])
}
