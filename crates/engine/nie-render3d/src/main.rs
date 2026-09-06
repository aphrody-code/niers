//! Binaire `nie-render3d` — charge un GLB (modèle réel reconstruit des CPK) et le rend en 3D :
//! PNG (vue fixe) ou MP4 turntable. Headless.
//!
//! ```text
//! nie-render3d --glb /tmp/c01000010.glb --frames 120 --out /tmp/chr.mp4
//! ```
//!
//! Avec la feature `gpu`, `--gpu` bascule sur le pipeline wgpu (2,16 ms/image contre 9,38 au CPU
//! en 1920×1080), et `--verify` compare les deux chemins sur la même vue. La comparaison porte sur
//! la **silhouette** : les deux rastériseurs ne rendent pas les mêmes octets par conception
//! (cf. la table d'écarts assumés dans la doc du crate).

#![forbid(unsafe_code)]
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Parser;
use nie_render3d::{glb, render, scene};

/// Boîte englobante (min,max) de toutes les positions du modèle.
fn aabb(model: &glb::Model) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in &model.primitives {
        for v in &p.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
    }
    (lo, hi)
}

/// Sol herbeux rayé autour de l'origine (quads plats).
fn ground() -> Vec<scene::Tri> {
    let mut t = Vec::new();
    let r = 4.0f32;
    let stripes = 8;
    for i in 0..stripes {
        let z0 = -r + 2.0 * r * (i as f32) / stripes as f32;
        let z1 = -r + 2.0 * r * ((i + 1) as f32) / stripes as f32;
        let g = if i % 2 == 0 {
            [46u8, 150, 64]
        } else {
            [40u8, 134, 58]
        };
        t.push(scene::Tri {
            p: [[-r, 0.0, z0], [r, 0.0, z0], [r, 0.0, z1]],
            color: g,
        });
        t.push(scene::Tri {
            p: [[-r, 0.0, z0], [r, 0.0, z1], [-r, 0.0, z1]],
            color: g,
        });
    }
    t
}

/// Transform modèle→monde : centré en x/z, pieds à y=0, mis à l'échelle ~1,7 m, tourné de `angle`.
fn place(model: &glb::Model, angle: f32) -> scene::Mat4 {
    let (lo, hi) = aabb(model);
    let s = 1.7 / (hi[1] - lo[1]).max(1e-3);
    let (cx, cz) = ((lo[0] + hi[0]) * 0.5, (lo[2] + hi[2]) * 0.5);
    let m = scene::mat_mul(
        &scene::mat_scale(s),
        &scene::mat_translate([-cx, -lo[1], -cz]),
    );
    scene::mat_mul(&scene::mat_rot_y(angle), &m)
}

/// Vrai (fini et de magnitude raisonnable) — filtre les sommets aberrants des submeshes de map
/// dont le layout vertex n'est pas encore RE (positions à ~1e33).
fn finite_ok(v: [f32; 3]) -> bool {
    v.iter().all(|c| c.is_finite() && c.abs() < 1.0e6)
}

/// Convertit le GLB en triangles plats colorés par hauteur (visualisation **map** : géométrie
/// d'environnement deux-faces, indépendante des textures/winding). Renvoie (tris, centre, étendue).
/// Ignore les triangles aberrants. Sert à rendre le **monde 3D** du jeu.
fn map_tris(models: &[glb::Model]) -> (Vec<scene::Tri>, [f32; 3], f32) {
    // bbox **robuste par percentiles** : certains submeshes (layout non-RE) ont des positions
    // aberrantes même < 1e6 ; on cadre sur le cœur (centile 1–99) de la distribution. Plusieurs
    // modèles = chunks d'un stage, déjà en coordonnées monde → simple union.
    let mut axes: [Vec<f32>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for model in models {
        for p in &model.primitives {
            for v in &p.positions {
                if finite_ok(*v) {
                    for k in 0..3 {
                        axes[k].push(v[k]);
                    }
                }
            }
        }
    }
    let pct = |sorted: &[f32], q: f32| -> f32 {
        if sorted.is_empty() {
            return 0.0;
        }
        let i = ((sorted.len() - 1) as f32 * q) as usize;
        sorted[i]
    };
    let (mut lo, mut hi) = ([0.0f32; 3], [0.0f32; 3]);
    for k in 0..3 {
        axes[k].sort_by(|a, b| a.partial_cmp(b).unwrap());
        lo[k] = pct(&axes[k], 0.01);
        hi[k] = pct(&axes[k], 0.99);
    }
    let center = [
        (lo[0] + hi[0]) * 0.5,
        (lo[1] + hi[1]) * 0.5,
        (lo[2] + hi[2]) * 0.5,
    ];
    let extent = (0..3)
        .map(|k| hi[k] - lo[k])
        .fold(0.0f32, f32::max)
        .max(1.0);
    let (y0, y1) = (lo[1], hi[1].max(lo[1] + 1.0));
    // On ne garde que les triangles dont les 3 sommets sont dans ~1,5× la bbox robuste du centre
    // (élimine les triangles parasites qui relient un sommet valide à un sommet aberrant).
    let lim = extent * 1.5;
    let near_core = |v: [f32; 3]| (0..3).all(|k| (v[k] - center[k]).abs() <= lim);
    let mut tris = Vec::new();
    for model in models {
        for p in &model.primitives {
            for t in p.indices.chunks_exact(3) {
                let (ia, ib, ic) = (t[0] as usize, t[1] as usize, t[2] as usize);
                if ia.max(ib).max(ic) >= p.positions.len() {
                    continue;
                }
                let (a, b, c) = (p.positions[ia], p.positions[ib], p.positions[ic]);
                if !(near_core(a) && near_core(b) && near_core(c)) {
                    continue;
                }
                let yc = (((a[1] + b[1] + c[1]) / 3.0 - y0) / (y1 - y0)).clamp(0.0, 1.0);
                let col = [
                    (70.0 + 95.0 * yc) as u8,
                    (92.0 + 78.0 * yc) as u8,
                    (72.0 + 58.0 * yc) as u8,
                ];
                tris.push(scene::Tri {
                    p: [a, b, c],
                    color: col,
                });
            }
        }
    }
    (tris, center, extent)
}

/// Caméra orbitale auto-cadrée sur la bbox d'une map.
fn map_camera(center: [f32; 3], extent: f32, angle: f32) -> scene::Camera {
    let r = extent * 1.35;
    scene::Camera {
        eye: [
            center[0] + r * angle.sin(),
            center[1] + extent * 0.65,
            center[2] + r * angle.cos(),
        ],
        target: center,
        up: [0.0, 1.0, 0.0],
        fov_y: 0.72,
    }
}

/// Rend une map en triangles plats colorés par hauteur (caméra orbitale).
fn render_map_frame(
    tris: &[scene::Tri],
    center: [f32; 3],
    extent: f32,
    angle: f32,
    w: u32,
    h: u32,
) -> Vec<u8> {
    scene::render_world(
        tris,
        &map_camera(center, extent, angle),
        w,
        h,
        [120, 150, 210],
        [196, 210, 226],
    )
}

/// Rend une map TEXTURÉE : les modèles (chunks) en instances (transform identité, déjà en monde),
/// échantillonnés avec leurs UV + atlas. Caméra orbitale auto-cadrée.
fn render_map_textured(
    models: &[glb::Model],
    center: [f32; 3],
    extent: f32,
    angle: f32,
    w: u32,
    h: u32,
) -> Vec<u8> {
    let inst: Vec<scene::Instance> = models
        .iter()
        .map(|m| scene::Instance {
            model: m,
            transform: scene::mat_identity(),
            two_sided: true,
        })
        .collect();
    scene::render_scene(
        &[],
        &inst,
        &map_camera(center, extent, angle),
        w,
        h,
        [120, 150, 210],
        [196, 210, 226],
    )
}

/// Rend un modèle posé sur le sol via le compositeur de scène (caméra monde fixe).
fn render_scene_frame(model: &glb::Model, angle: f32, w: u32, h: u32) -> Vec<u8> {
    let cam = scene::Camera {
        eye: [0.0, 1.05, 3.3],
        target: [0.0, 0.95, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_y: 0.72,
    };
    let inst = [scene::Instance {
        model,
        transform: place(model, angle),
        two_sided: false,
    }];
    scene::render_scene(&ground(), &inst, &cam, w, h, [120, 150, 210], [58, 86, 140])
}

#[derive(Parser, Debug)]
#[command(about = "Rend un GLB réel (asset CPK) en 3D → PNG/MP4 turntable (headless)")]
struct Cli {
    /// Fichier(s) GLB d'entrée (ex. /model-full/<code>.glb). Répétable : en mode --map, tous les
    /// GLB sont composés dans un même monde (les chunks de stage sont en coordonnées monde).
    #[arg(long, required = true)]
    glb: Vec<PathBuf>,
    /// Sortie : PNG si --frames 1, sinon MP4 turntable.
    #[arg(long, default_value = "/tmp/niers-model.png")]
    out: PathBuf,
    /// Nombre d'images (1 = vue fixe PNG ; >1 = tour complet → MP4).
    #[arg(long, default_value_t = 1)]
    frames: u32,
    #[arg(long, default_value_t = 30)]
    fps: u32,
    #[arg(long, default_value_t = 720)]
    width: u32,
    #[arg(long, default_value_t = 720)]
    height: u32,
    /// Pose le modèle texturé sur un sol et le rend via le compositeur de scène (caméra monde).
    #[arg(long)]
    scene: bool,
    /// Rend le GLB comme une **map/stage** : géométrie d'environnement deux-faces, caméra orbitale
    /// auto-cadrée, coloration par hauteur (robuste aux submeshes non-RE).
    #[arg(long)]
    map: bool,
    /// Angle de la vue fixe, en radians (défaut : 0.6, le trois-quarts de référence).
    ///
    /// Sans effet quand `--frames > 1` : le turntable balaie le tour complet.
    #[arg(long, default_value_t = 0.6)]
    angle: f32,
    /// Rend sur le **GPU** (wgpu) au lieu du rastériseur CPU.
    ///
    /// Le modèle est téléversé une fois, puis chaque image ne coûte qu'un appel de dessin :
    /// l'écart se creuse avec le nombre d'images, pas sur une vue fixe. Incompatible avec
    /// `--scene` et `--map`, qui composent une géométrie que le pipeline GPU ne connaît pas.
    #[cfg(feature = "gpu")]
    #[arg(long)]
    gpu: bool,
    /// API graphique : dx12, vulkan, gl, webgpu, metal ou auto. Implique le rendu GPU.
    #[cfg(feature = "gpu")]
    #[arg(long)]
    backend: Option<nie_render3d::gpu::Backend>,
    /// Refuse un adaptateur logiciel. Implique le rendu GPU.
    #[cfg(feature = "gpu")]
    #[arg(long)]
    hardware_only: bool,
    /// Compare la première image CPU et GPU et rend le verdict, comme `nie-game --verify`.
    ///
    /// Les deux rastériseurs ne peuvent pas être identiques au bit — le GPU interpole et filtre
    /// dans son propre ordre — mais un écart massif signale une divergence de pipeline, pas un
    /// arrondi. Implique `--gpu`.
    #[cfg(feature = "gpu")]
    #[arg(long)]
    verify: bool,
}

fn encode_png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(std::io::Cursor::new(&mut out), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().context("png header")?;
        wr.write_image_data(rgba).context("png data")?;
    }
    Ok(out)
}

/// Verdict de comparaison entre le rendu CPU et le rendu GPU d'une même vue.
#[cfg(feature = "gpu")]
struct Comparaison {
    /// Pixels couverts par la géométrie côté CPU.
    cpu: usize,
    /// Idem côté GPU.
    gpu: usize,
    /// Recouvrement des deux silhouettes : intersection / union, en pourcentage.
    iou: f64,
    /// Écart de couleur maximal **là où les deux ont dessiné**.
    ecart_max: u8,
    /// Part de cette intersection dont l'écart tient dans la tolérance.
    dans_tolerance: f64,
}

/// Compare un rendu CPU et un rendu GPU de la même vue.
///
/// **Pas une comparaison pixel à pixel du cadre entier** : le rastériseur CPU peint un fond
/// opaque, le pipeline GPU laisse l'arrière-plan transparent — un choix assumé, l'interface
/// décidant de ce qu'il y a derrière le modèle. Confronter les deux cadres tels quels ferait
/// donc échouer 99 % des pixels sur une différence qui n'est pas une erreur.
///
/// Ce qui se compare, c'est ce que les deux prétendent dessiner : la **silhouette** (recouvrement
/// des zones couvertes) et, sur leur intersection, la couleur. Une divergence de projection, de
/// cadrage ou de sens de rotation effondre l'IoU ; une divergence d'éclairage ou de filtrage ne
/// touche que la couleur. Les deux ne se confondent plus.
#[cfg(feature = "gpu")]
fn comparer(cpu: &[u8], gpu: &[u8], w: u32, h: u32, tolerance: u8) -> Comparaison {
    let (mut n_cpu, mut n_gpu, mut inter, mut union) = (0usize, 0usize, 0usize, 0usize);
    let (mut ecart_max, mut dans) = (0u8, 0usize);
    for y in 0..h {
        let fond = render::couleur_fond(y, h);
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let (pc, pg) = (&cpu[i..i + 4], &gpu[i..i + 4]);
            // Côté CPU, « couvert » = différent du fond que le rastériseur aurait peint ici.
            let c = pc != fond;
            // Côté GPU, l'alpha le dit directement.
            let g = pg[3] > 0;
            n_cpu += usize::from(c);
            n_gpu += usize::from(g);
            union += usize::from(c || g);
            if c && g {
                inter += 1;
                let d = (0..3).map(|k| pc[k].abs_diff(pg[k])).max().unwrap_or(0);
                ecart_max = ecart_max.max(d);
                dans += usize::from(d <= tolerance);
            }
        }
    }
    Comparaison {
        cpu: n_cpu,
        gpu: n_gpu,
        iou: if union == 0 {
            0.0
        } else {
            inter as f64 * 100.0 / union as f64
        },
        ecart_max,
        dans_tolerance: if inter == 0 {
            0.0
        } else {
            dans as f64 * 100.0 / inter as f64
        },
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut models = Vec::with_capacity(cli.glb.len());
    for path in &cli.glb {
        let data = std::fs::read(path).with_context(|| format!("lire {}", path.display()))?;
        models.push(glb::parse(&data)?);
    }
    let model = &models[0];
    let tris: usize = models
        .iter()
        .flat_map(|m| &m.primitives)
        .map(|p| p.indices.len() / 3)
        .sum();
    let verts: usize = models
        .iter()
        .flat_map(|m| &m.primitives)
        .map(|p| p.positions.len())
        .sum();
    // Une primitive sans indices ne dessine rien : ni le rastériseur CPU (qui itère les triangles)
    // ni le pipeline GPU ne la voient. Compter ses sommets dans le total laissait croire à une
    // perte au téléversement — sur un keshin, 12 096 annoncés contre 4 070 réellement envoyés.
    let verts_dessines: usize = models
        .iter()
        .flat_map(|m| &m.primitives)
        .filter(|p| !p.indices.is_empty())
        .map(|p| p.positions.len())
        .sum();
    let nprim: usize = models.iter().map(|m| m.primitives.len()).sum();
    print!(
        "glb={} primitives={nprim} vertices={verts} triangles={tris}",
        cli.glb.len()
    );
    if verts_dessines != verts {
        print!(" (dont {verts_dessines} sommets dessinables — le reste est sans indices)");
    }
    println!();

    // Mode map : pré-calcule la géométrie d'environnement une fois (tous les chunks composés).
    let map_data = if cli.map {
        Some(map_tris(&models))
    } else {
        None
    };
    if let Some((tris, _, _)) = &map_data {
        println!(
            "map_tris={} (après filtrage des submeshes aberrants)",
            tris.len()
        );
    }

    // Map texturée si les modèles portent des textures, sinon coloration par hauteur.
    let map_textured = cli.map && models.iter().any(|m| !m.textures.is_empty());
    let frame = |angle: f32| -> Vec<u8> {
        if let Some((tris, center, extent)) = &map_data {
            if map_textured {
                render_map_textured(&models, *center, *extent, angle, cli.width, cli.height)
            } else {
                render_map_frame(tris, *center, *extent, angle, cli.width, cli.height)
            }
        } else if cli.scene {
            render_scene_frame(model, angle, cli.width, cli.height)
        } else {
            render::render(model, angle, cli.width, cli.height)
        }
    };

    // Chemin GPU : le modèle est téléversé UNE fois, chaque image n'est plus qu'un appel de
    // dessin. Il remplace `frame` au lieu de s'y glisser, parce que le contrat n'est pas le même —
    // le renderer garde son état (device, pipeline, tampons) d'une image à l'autre, et c'est
    // précisément ce qui fait la différence.
    #[cfg(feature = "gpu")]
    if cli.gpu || cli.verify || cli.backend.is_some() || cli.hardware_only {
        anyhow::ensure!(
            !cli.scene && !cli.map,
            "--gpu ne compose ni --scene ni --map : ces modes construisent une géométrie \
             (sol, chunks) que le pipeline GPU ne connaît pas"
        );
        let debut = std::time::Instant::now();
        let mut renderer =
            nie_render3d::gpu::GpuRenderer::with_options(nie_render3d::gpu::GpuOptions {
                backend: cli.backend.unwrap_or_default(),
                allow_software: !cli.hardware_only,
            })?;
        let adapter = renderer.adapter_info();
        println!(
            "backend={:?} adapter={:?} type={:?} driver={:?}",
            adapter.backend, adapter.name, adapter.device_type, adapter.driver
        );
        let gm = renderer.upload(model);
        println!(
            "gpu: {} triangles, {} sommets téléversés en {:?}",
            gm.triangle_count,
            gm.vertex_count,
            debut.elapsed(),
        );
        // Les deux rastériseurs ont des conventions OPPOSÉES, et toutes deux légitimes : le CPU
        // fait tourner le MODÈLE devant une caméra fixe (`render::render(model, angle, …)`), le
        // GPU fait ORBITER la caméra autour du modèle. Tourner l'objet de +θ montre la même face
        // que tourner l'observateur de −θ — d'où l'inversion ici, sans laquelle `--angle 0.6`
        // désignerait deux vues en miroir selon le chemin de rendu.
        let cam = |angle: f32| {
            nie_render3d::gpu::Camera {
                yaw: -angle,
                ..Default::default()
            }
            .clamped()
        };

        if cli.verify {
            let gpu_px = renderer.render(&gm, cam(cli.angle), cli.width, cli.height)?;
            let cpu_px = render::render(model, cli.angle, cli.width, cli.height);
            let c = comparer(&cpu_px, &gpu_px, cli.width, cli.height, 32);
            println!("=== vérification CPU vs GPU ===");
            println!("  couverture CPU      : {} px", c.cpu);
            println!("  couverture GPU      : {} px", c.gpu);
            println!("  recouvrement (IoU)  : {:.2}%", c.iou);
            println!(
                "  écart couleur max   : {}/255 (sur l'intersection)",
                c.ecart_max
            );
            println!("  couleur à ≤32/255   : {:.2}%", c.dans_tolerance);
            // Le seuil porte sur la GÉOMÉTRIE. Deux rastériseurs différents ne teintent pas
            // identiquement — filtrage de texture, ordre d'interpolation, gamma — mais ils
            // doivent couvrir la même silhouette : c'est cela qui prouve que la projection, le
            // cadrage et le sens de rotation concordent.
            println!(
                "  {}",
                if c.iou >= 90.0 {
                    "PASS — même silhouette"
                } else {
                    "ÉCART de silhouette : projection, cadrage ou sens de rotation divergent"
                }
            );
            // Dire d'où vient l'écart de couleur évite de le lire comme un défaut : il est
            // attendu, et il a deux causes connues, toutes deux assumées.
            println!(
                "  (l'écart de couleur restant est attendu : ombrage lissé par sommet côté GPU \
                 contre plat par face côté CPU, et filtrage linéaire contre plus-proche-voisin)"
            );
        }

        if cli.frames <= 1 {
            let rgba = renderer.render(&gm, cam(cli.angle), cli.width, cli.height)?;
            std::fs::write(&cli.out, encode_png(&rgba, cli.width, cli.height)?)?;
            println!("png={} (gpu)", cli.out.display());
            return Ok(());
        }

        let dir = std::env::temp_dir().join(format!("niers-r3d-gpu-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        // Rendu et encodage sont chronométrés SÉPARÉMENT : sur une sortie PNG, la compression est
        // le goulot et masque entièrement le gain du GPU. Annoncer un temps global laisserait
        // croire que le rendu coûte ce que coûte le PNG.
        let (mut t_rendu, mut t_png) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
        for i in 0..cli.frames {
            let angle = std::f32::consts::TAU * (i as f32) / (cli.frames as f32);
            let t0 = std::time::Instant::now();
            let rgba = renderer.render(&gm, cam(angle), cli.width, cli.height)?;
            t_rendu += t0.elapsed();
            let t1 = std::time::Instant::now();
            std::fs::write(
                dir.join(format!("f_{i:04}.png")),
                encode_png(&rgba, cli.width, cli.height)?,
            )?;
            t_png += t1.elapsed();
        }
        let n = f64::from(cli.frames);
        println!(
            "gpu: {} images — rendu {:?} ({:.2} ms/image), encodage PNG {:?} ({:.2} ms/image)",
            cli.frames,
            t_rendu,
            t_rendu.as_secs_f64() * 1000.0 / n,
            t_png,
            t_png.as_secs_f64() * 1000.0 / n,
        );
        encode_video(&dir, cli.fps, &cli.out)?;
        let _ = std::fs::remove_dir_all(&dir);
        let sz = std::fs::metadata(&cli.out).map(|m| m.len()).unwrap_or(0);
        println!("video={} ({sz} octets, gpu)", cli.out.display());
        return Ok(());
    }

    if cli.frames <= 1 {
        let rgba = frame(cli.angle);
        std::fs::write(&cli.out, encode_png(&rgba, cli.width, cli.height)?)?;
        println!("png={}", cli.out.display());
        return Ok(());
    }

    let dir = std::env::temp_dir().join(format!("niers-r3d-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    // Même ventilation rendu/encodage que le chemin GPU : comparer deux temps globaux, dont l'un
    // porte un encodage PNG identique de part et d'autre, dirait surtout le coût du PNG.
    let (mut t_rendu, mut t_png) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
    for i in 0..cli.frames {
        let angle = std::f32::consts::TAU * (i as f32) / (cli.frames as f32);
        let t0 = std::time::Instant::now();
        let rgba = frame(angle);
        t_rendu += t0.elapsed();
        let t1 = std::time::Instant::now();
        std::fs::write(
            dir.join(format!("f_{i:04}.png")),
            encode_png(&rgba, cli.width, cli.height)?,
        )?;
        t_png += t1.elapsed();
    }
    let n = f64::from(cli.frames);
    println!(
        "cpu: {} images — rendu {:?} ({:.2} ms/image), encodage PNG {:?} ({:.2} ms/image)",
        cli.frames,
        t_rendu,
        t_rendu.as_secs_f64() * 1000.0 / n,
        t_png,
        t_png.as_secs_f64() * 1000.0 / n,
    );
    encode_video(&dir, cli.fps, &cli.out)?;
    let _ = std::fs::remove_dir_all(&dir);
    let sz = std::fs::metadata(&cli.out).map(|m| m.len()).unwrap_or(0);
    println!("video={} ({sz} octets)", cli.out.display());
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
