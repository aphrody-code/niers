//! `nie-match3d` — **le moteur, intégré** : la simulation physique de [`nie_runtime`] pilote un
//! rendu **3D** (caméra télé) d'un match. Terrain rayé + lignes + buts, 22 joueurs (boîtes corps +
//! tête, couleurs d'équipe), ballon. Chaque pas de sim → une frame 3D → MP4. C'est rendu + gameplay
//! + physique + affichage réunis, sur le vrai terrain réglementaire (105×68 m).
//!
//! ```text
//! nie-match3d --frames 300 --fps 30 --out /tmp/match3d.mp4
//! ```

#![forbid(unsafe_code)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Parser;
use nie_render3d::glb;
use nie_render3d::scene::{
    Camera, Instance, Mat4, Tri, mat_identity, mat_mul, mat_rot_y, mat_scale, mat_translate,
    render_scene,
};
use nie_runtime::{GOAL_HALF, GOAL_HEIGHT, HALF_LEN, HALF_WID, Player, Role, World};

#[derive(Parser, Debug)]
#[command(
    about = "Rend un match 3D : la physique nie-runtime pilote le rendu 3D (caméra télé) → MP4"
)]
struct Cli {
    #[arg(long, default_value = "/tmp/niers-match3d.mp4")]
    out: PathBuf,
    #[arg(long, default_value_t = 300)]
    frames: u32,
    #[arg(long, default_value_t = 30)]
    fps: u32,
    #[arg(long, default_value_t = 960)]
    width: u32,
    #[arg(long, default_value_t = 540)]
    height: u32,
    /// Pas de simulation par frame (s).
    #[arg(long, default_value_t = 1.0 / 30.0)]
    dt: f32,
    /// Rendre une seule image PNG (la première) au lieu de la vidéo.
    #[arg(long)]
    png: bool,
    /// GLB de l'équipe domicile : si fourni avec --away-glb, les joueurs sont les VRAIS modèles
    /// texturés (au lieu de boîtes).
    #[arg(long)]
    home_glb: Option<PathBuf>,
    /// GLB de l'équipe extérieure.
    #[arg(long)]
    away_glb: Option<PathBuf>,
    /// Chunk(s) GLB du VRAI stade (répétable) : remplacent la pelouse procédurale par le décor 3D
    /// réel du jeu. Les chunks sont en coordonnées monde (cf. nie-render3d --map).
    #[arg(long)]
    stadium: Vec<PathBuf>,
}

/// Un modèle de joueur chargé + ses paramètres de pose (échelle vers ~1,85 m, centre, pieds).
struct PlayerModel {
    model: glb::Model,
    s: f32,
    cx: f32,
    cz: f32,
    min_y: f32,
}

impl PlayerModel {
    fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read(path).with_context(|| format!("lire {}", path.display()))?;
        let model = glb::parse(&data)?;
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &model.primitives {
            for v in &p.positions {
                for k in 0..3 {
                    lo[k] = lo[k].min(v[k]);
                    hi[k] = hi[k].max(v[k]);
                }
            }
        }
        Ok(Self {
            s: 1.85 / (hi[1] - lo[1]).max(1e-3),
            cx: (lo[0] + hi[0]) * 0.5,
            cz: (lo[2] + hi[2]) * 0.5,
            min_y: lo[1],
            model,
        })
    }

    /// Transform modèle→monde : pieds à y=0 au point terrain (px,pz), orienté `heading` (rad).
    fn xform(&self, px: f32, pz: f32, heading: f32) -> Mat4 {
        let recenter = mat_translate([-self.cx, -self.min_y, -self.cz]);
        let m = mat_mul(&mat_scale(self.s), &recenter);
        let m = mat_mul(&mat_rot_y(heading), &m);
        mat_mul(&mat_translate([px, 0.0, pz]), &m)
    }
}

/// Cap d'un joueur : direction de course si en mouvement, sinon vers le but attaqué.
fn heading(p: &Player) -> f32 {
    if p.vel.len() > 1.2 {
        p.vel.x.atan2(p.vel.y)
    } else if p.team == 0 {
        std::f32::consts::FRAC_PI_2
    } else {
        -std::f32::consts::FRAC_PI_2
    }
}

/// Mappe un point terrain (longueur `x`, largeur `z`, hauteur `y`) vers l'espace monde du renderer.
const fn w(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x, y, z]
}

fn quad(out: &mut Vec<Tri>, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], color: [u8; 3]) {
    out.push(Tri {
        p: [a, b, c],
        color,
    });
    out.push(Tri {
        p: [a, c, d],
        color,
    });
}

/// Boîte alignée sur les axes (min→max), 12 triangles, couleur plate (éclairage deux-faces).
fn push_box(out: &mut Vec<Tri>, mn: [f32; 3], mx: [f32; 3], color: [u8; 3]) {
    let (x0, y0, z0) = (mn[0], mn[1], mn[2]);
    let (x1, y1, z1) = (mx[0], mx[1], mx[2]);
    quad(
        out,
        w(x0, y0, z0),
        w(x1, y0, z0),
        w(x1, y1, z0),
        w(x0, y1, z0),
        color,
    ); // -z
    quad(
        out,
        w(x1, y0, z1),
        w(x0, y0, z1),
        w(x0, y1, z1),
        w(x1, y1, z1),
        color,
    ); // +z
    quad(
        out,
        w(x0, y0, z1),
        w(x0, y0, z0),
        w(x0, y1, z0),
        w(x0, y1, z1),
        color,
    ); // -x
    quad(
        out,
        w(x1, y0, z0),
        w(x1, y0, z1),
        w(x1, y1, z1),
        w(x1, y1, z0),
        color,
    ); // +x
    quad(
        out,
        w(x0, y1, z0),
        w(x1, y1, z0),
        w(x1, y1, z1),
        w(x0, y1, z1),
        color,
    ); // +y (haut)
    quad(
        out,
        w(x0, y0, z1),
        w(x1, y0, z1),
        w(x1, y0, z0),
        w(x0, y0, z0),
        color,
    ); // -y (bas)
}

/// Segment de ligne blanche au sol (épaisseur `hw`), orientation libre dans le plan terrain.
fn line(out: &mut Vec<Tri>, x0: f32, z0: f32, x1: f32, z1: f32, hw: f32) {
    let col = [232u8, 235, 238];
    let (dx, dz) = (x1 - x0, z1 - z0);
    let len = (dx * dx + dz * dz).sqrt().max(1e-4);
    let (px, pz) = (-dz / len * hw, dx / len * hw);
    let y = 0.05;
    quad(
        out,
        w(x0 + px, y, z0 + pz),
        w(x1 + px, y, z1 + pz),
        w(x1 - px, y, z1 - pz),
        w(x0 - px, y, z0 - pz),
        col,
    );
}

/// Rectangle de lignes (boîtes de surface, buts au sol).
fn line_rect(out: &mut Vec<Tri>, x0: f32, z0: f32, x1: f32, z1: f32, hw: f32) {
    line(out, x0, z0, x1, z0, hw);
    line(out, x1, z0, x1, z1, hw);
    line(out, x1, z1, x0, z1, hw);
    line(out, x0, z1, x0, z0, hw);
}

/// Construit la géométrie statique du terrain (gazon rayé + lignes + buts).
fn build_pitch(out: &mut Vec<Tri>) {
    // Surround sombre + gazon rayé en grands quads. Le near-clipping du rastériseur gère les
    // quads qui traversent le plan caméra (plus besoin de tuiler comme contournement).
    quad(
        out,
        w(-HALF_LEN - 12.0, -0.05, -HALF_WID - 8.0),
        w(HALF_LEN + 12.0, -0.05, -HALF_WID - 8.0),
        w(HALF_LEN + 12.0, -0.05, HALF_WID + 8.0),
        w(-HALF_LEN - 12.0, -0.05, HALF_WID + 8.0),
        [22, 86, 40],
    );
    let stripes = 16;
    for i in 0..stripes {
        let x0 = -HALF_LEN + 2.0 * HALF_LEN * (i as f32) / stripes as f32;
        let x1 = -HALF_LEN + 2.0 * HALF_LEN * ((i + 1) as f32) / stripes as f32;
        let g = if i % 2 == 0 {
            [46u8, 150, 64]
        } else {
            [40u8, 134, 58]
        };
        quad(
            out,
            w(x0, 0.0, -HALF_WID),
            w(x1, 0.0, -HALF_WID),
            w(x1, 0.0, HALF_WID),
            w(x0, 0.0, HALF_WID),
            g,
        );
    }
    let hw = 0.12;
    // Bordure + ligne médiane.
    line_rect(out, -HALF_LEN, -HALF_WID, HALF_LEN, HALF_WID, hw);
    line(out, 0.0, -HALF_WID, 0.0, HALF_WID, hw);
    // Rond central (segments) + point central.
    let r = 9.15;
    let seg = 36;
    for i in 0..seg {
        let a0 = std::f32::consts::TAU * (i as f32) / seg as f32;
        let a1 = std::f32::consts::TAU * ((i + 1) as f32) / seg as f32;
        line(
            out,
            r * a0.cos(),
            r * a0.sin(),
            r * a1.cos(),
            r * a1.sin(),
            hw,
        );
    }
    push_box(
        out,
        w(-0.25, 0.04, -0.25),
        w(0.25, 0.06, 0.25),
        [232, 235, 238],
    );
    // Surfaces de réparation + de but + points de penalty, des deux côtés.
    for &s in &[-1.0f32, 1.0] {
        let gl = s * HALF_LEN; // ligne de but
        line_rect(out, gl, -20.15, gl - s * 16.5, 20.15, hw); // surface de réparation
        line_rect(out, gl, -9.16, gl - s * 5.5, 9.16, hw); // surface de but
        push_box(
            out,
            w(gl - s * 11.0 - 0.2, 0.04, -0.2),
            w(gl - s * 11.0 + 0.2, 0.06, 0.2),
            [232, 235, 238],
        );
        // But : deux poteaux + barre transversale (boîtes blanches).
        let post = 0.12;
        for &gz in &[-GOAL_HALF, GOAL_HALF] {
            push_box(
                out,
                w(gl - post, 0.0, gz - post),
                w(gl + post, GOAL_HEIGHT, gz + post),
                [240, 240, 245],
            );
        }
        push_box(
            out,
            w(gl - post, GOAL_HEIGHT - post, -GOAL_HALF),
            w(gl + post, GOAL_HEIGHT, GOAL_HALF),
            [240, 240, 245],
        );
    }
}

/// Couleurs d'équipe (corps), gardien distinct.
fn player_color(p: &Player) -> [u8; 3] {
    match (p.team, p.role) {
        (0, Role::Goalkeeper) => [240, 205, 50],
        (1, Role::Goalkeeper) => [40, 205, 130],
        (0, _) => [222, 66, 52], // domicile : rouge-orangé
        (_, _) => [56, 96, 214], // extérieur : bleu
    }
}

/// Ajoute un joueur (corps + tête) à la scène, à sa position terrain.
fn push_player(out: &mut Vec<Tri>, p: &Player) {
    let (px, pz) = (p.pos.x, p.pos.y);
    let col = player_color(p);
    // Corps (boîte), tête (peau).
    push_box(
        out,
        w(px - 0.42, 0.0, pz - 0.30),
        w(px + 0.42, 1.45, pz + 0.30),
        col,
    );
    push_box(
        out,
        w(px - 0.26, 1.45, pz - 0.26),
        w(px + 0.26, 1.95, pz + 0.26),
        [236, 200, 168],
    );
}

/// Géométrie plate de la scène : terrain + ballon, plus les joueurs-boîtes si `boxes`
/// (sinon les joueurs sont des instances de modèles réels, ajoutées côté appelant).
fn build_flat(world: &World, boxes: bool) -> Vec<Tri> {
    let mut tris = Vec::with_capacity(2048);
    build_pitch(&mut tris);
    if boxes {
        for p in &world.players {
            push_player(&mut tris, p);
        }
    }
    // Ballon (boîte blanche, position 3D : hauteur = z).
    let b = world.ball.pos;
    let s = 0.16;
    push_box(
        &mut tris,
        w(b.x - s, b.z, b.y - s),
        w(b.x + s, b.z + 2.0 * s, b.y + s),
        [245, 245, 248],
    );
    tris
}

/// Caméra. Mode `close` (modèles réels) : plan d'action rapproché qui suit le ballon le long de la
/// touche, pour voir les personnages en détail. Sinon : vue télé large (lisible pour les boîtes).
fn camera(world: &World, close: bool, stadium: bool) -> Camera {
    let b = world.ball.pos;
    if stadium {
        // Vue large surplombant le vrai stade (qui s'étend en z jusqu'à ±125).
        return Camera {
            eye: [b.x * 0.2, 80.0, -170.0],
            target: [b.x * 0.4, 4.0, b.y * 0.25],
            up: [0.0, 1.0, 0.0],
            fov_y: 0.62,
        };
    }
    if close {
        Camera {
            eye: [b.x * 0.6, 9.0, b.y * 0.5 - 17.0],
            target: [b.x * 0.85, 0.6, b.y * 0.5 + 3.0],
            up: [0.0, 1.0, 0.0],
            fov_y: 0.72,
        }
    } else {
        Camera {
            eye: [b.x * 0.18, 44.0, -82.0],
            target: [b.x * 0.45, 1.0, b.y * 0.35 + 2.0],
            up: [0.0, 1.0, 0.0],
            fov_y: 0.62,
        }
    }
}

fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(std::io::Cursor::new(&mut out), width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().context("png header")?;
        wr.write_image_data(rgba).context("png data")?;
    }
    Ok(out)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut world = World::kickoff();
    let (bw, bh) = ([14u8, 20, 34], [30u8, 40, 58]); // ciel de stade

    // Modèles réels si les deux GLB sont fournis ; sinon, joueurs-boîtes.
    let (home, away) = match (&cli.home_glb, &cli.away_glb) {
        (Some(h), Some(a)) => (Some(PlayerModel::load(h)?), Some(PlayerModel::load(a)?)),
        _ => (None, None),
    };
    let real = home.is_some();

    // Vrai stade (chunks GLB en coordonnées monde) → remplace la pelouse procédurale.
    let stadium: Vec<glb::Model> = cli
        .stadium
        .iter()
        .map(|p| {
            let data = std::fs::read(p).with_context(|| format!("lire {}", p.display()))?;
            glb::parse(&data)
        })
        .collect::<Result<_>>()?;
    let has_stadium = !stadium.is_empty();

    // Construit la scène d'une frame : (flat tris, instances). Avec stade : le décor TEXTURÉ est
    // rendu en instances deux-faces (chunks déjà en coords monde), flat = juste le ballon.
    let frame_scene = |world: &World| -> (Vec<Tri>, Vec<Instance>) {
        let mut flat = if has_stadium {
            let mut f = Vec::new();
            let b = world.ball.pos;
            let s = 0.16;
            push_box(
                &mut f,
                w(b.x - s, b.z, b.y - s),
                w(b.x + s, b.z + 2.0 * s, b.y + s),
                [245, 245, 248],
            );
            f
        } else {
            build_flat(world, !real)
        };
        flat.shrink_to_fit();
        let mut inst = Vec::new();
        for m in &stadium {
            inst.push(Instance {
                model: m,
                transform: mat_identity(),
                two_sided: true,
            });
        }
        if let (Some(h), Some(a)) = (&home, &away) {
            for p in &world.players {
                let pm = if p.team == 0 { h } else { a };
                inst.push(Instance {
                    model: &pm.model,
                    transform: pm.xform(p.pos.x, p.pos.y, heading(p)),
                    two_sided: false,
                });
            }
        }
        (flat, inst)
    };

    if cli.png {
        let (flat, inst) = frame_scene(&world);
        let rgba = render_scene(
            &flat,
            &inst,
            &camera(&world, real, has_stadium),
            cli.width,
            cli.height,
            bw,
            bh,
        );
        std::fs::write(&cli.out, encode_png(&rgba, cli.width, cli.height)?)?;
        println!(
            "png={} tris={} instances={}",
            cli.out.display(),
            flat.len(),
            inst.len()
        );
        return Ok(());
    }

    let dir = std::env::temp_dir().join(format!("niers-m3d-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    for i in 0..cli.frames {
        let (flat, inst) = frame_scene(&world);
        let rgba = render_scene(
            &flat,
            &inst,
            &camera(&world, real, has_stadium),
            cli.width,
            cli.height,
            bw,
            bh,
        );
        std::fs::write(
            dir.join(format!("f_{i:04}.png")),
            encode_png(&rgba, cli.width, cli.height)?,
        )?;
        world.step(cli.dt);
    }
    encode_video(&dir, cli.fps, &cli.out)?;
    let _ = std::fs::remove_dir_all(&dir);
    let sz = std::fs::metadata(&cli.out).map(|m| m.len()).unwrap_or(0);
    println!(
        "video={} mode={} score={:?} ({sz} octets)",
        cli.out.display(),
        if real { "modèles" } else { "boîtes" },
        world.score
    );
    Ok(())
}

fn encode_video(dir: &Path, fps: u32, out: &Path) -> Result<()> {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-framerate",
            &fps.to_string(),
            "-i",
        ])
        .arg(dir.join("f_%04d.png"))
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(out)
        .status()
        .context("lancer ffmpeg")?;
    anyhow::ensure!(status.success(), "ffmpeg a échoué");
    Ok(())
}
