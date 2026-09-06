//! `nie-cam` — outil caméra d'Inazuma Eleven: Victory Road.
//!
//! Carte du reverse, extraction depuis le VFS, décodage/encodage des animations `.g4cm`,
//! lecture des configurations, et pilotage de la caméra du jeu en cours d'exécution.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use nie_camera::config::SoccerCameraConfig;
use nie_camera::db;
use nie_camera::g4cm::{self, Track};
use nie_camera::live::{self, CameraLayout, LiveCamera, PlausibleRange};
use nie_camera::map;
use nie_camera::model::CtrlKind;
use nie_camera::property::{FlatProperty, PropertySet};

#[derive(Parser)]
#[command(
    name = "nie-cam",
    about = "Caméra IEVR : carte RE, G4CM, configs, live"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Affiche la carte du reverse-engineering caméra (adresses, tables, assets, classes).
    Map {
        /// Vérifie la carte contre un `nie.exe`.
        #[arg(long)]
        exe: Option<PathBuf>,
    },
    /// Extrait les fichiers caméra du VFS vers un dossier.
    Extract {
        /// Dossier de destination.
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// Racine du jeu (défaut : détection automatique).
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Extrait aussi les `.g4cm` d'événement (1 215 fichiers).
        #[arg(long)]
        anims: bool,
    },
    /// Décode un `.g4cm` et décrit son contenu.
    Decode {
        /// Fichier `.g4cm`.
        file: PathBuf,
        /// Affiche le détail des canaux et leurs valeurs.
        #[arg(long)]
        verbose: bool,
    },
    /// Ré-encode un `.g4cm` (contrôle du round-trip byte-exact).
    Encode {
        /// Fichier source.
        file: PathBuf,
        /// Fichier de sortie (défaut : vérification seule, sans écrire).
        #[arg(long, short = 'o')]
        out: Option<PathBuf>,
        /// Multiplie le FOV de tous les canaux `f32` par ce facteur (démonstration d'édition).
        #[arg(long)]
        scale_pos: Option<f32>,
    },
    /// Vérifie le round-trip sur tous les `.g4cm` d'un dossier.
    Verify {
        /// Dossier contenant des `.g4cm`.
        dir: PathBuf,
    },
    /// Affiche une configuration caméra (`soccer_camera_config`, `camera_ctrl_property_info`…).
    Config {
        /// Fichier `.cfg.bin`.
        file: PathBuf,
        /// Résout et affiche les paramètres effectifs de ce preset.
        #[arg(long)]
        preset: Option<String>,
    },
    /// Pilote la caméra du jeu en cours d'exécution.
    Live {
        #[command(subcommand)]
        op: LiveOp,
    },
    /// Indexe tout le savoir caméra dans la base de connaissance (tables `cam_*`).
    Index {
        /// Base SQLite (défaut : `var/niers.sqlite`).
        #[arg(long, default_value = "var/niers.sqlite")]
        db: PathBuf,
        /// Racine du jeu (défaut : détection automatique).
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// `nie.exe` à indexer (carte RE + noms de paramètres).
        #[arg(long, default_value = "nie.exe")]
        exe: Option<PathBuf>,
        /// N'indexe pas les 1 215 animations `.g4cm`.
        #[arg(long)]
        no_anims: bool,
        /// Indexe **chaque échantillon** de keyframe (des millions de lignes).
        #[arg(long)]
        samples: bool,
        /// Limite le nombre d'animations indexées (mise au point).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Affiche l'état de l'index caméra de la base.
    Stats {
        /// Base SQLite.
        #[arg(long, default_value = "var/niers.sqlite")]
        db: PathBuf,
    },
}

#[derive(Subcommand)]
enum LiveOp {
    /// Cherche des objets caméra plausibles dans le process.
    Scan {
        /// Nom du process.
        #[arg(long, default_value = "nie.exe")]
        process: String,
        /// Nombre maximum de candidats.
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Lit l'état de la caméra à une adresse.
    Get {
        /// Adresse de l'objet caméra (hex).
        addr: String,
        #[arg(long, default_value = "nie.exe")]
        process: String,
    },
    /// Écrit l'état de la caméra à une adresse.
    Set {
        /// Adresse de l'objet caméra (hex).
        addr: String,
        #[arg(long, default_value = "nie.exe")]
        process: String,
        /// Position `x,y,z`.
        #[arg(long)]
        pos: Option<String>,
        /// Point visé `x,y,z`.
        #[arg(long)]
        target: Option<String>,
        /// Champ de vision, en degrés.
        #[arg(long)]
        fov: Option<f32>,
        /// Roulis, en degrés.
        #[arg(long)]
        roll: Option<f32>,
    },
}

fn parse_v3(s: &str) -> Result<[f32; 3]> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        bail!("attendu « x,y,z », reçu « {s} »");
    }
    let mut v = [0.0f32; 3];
    for (i, p) in parts.iter().enumerate() {
        v[i] = p
            .trim()
            .parse()
            .with_context(|| format!("« {p} » n'est pas un nombre"))?;
    }
    Ok(v)
}

fn parse_addr(s: &str) -> Result<u64> {
    let t = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    u64::from_str_radix(t, 16).with_context(|| format!("adresse hexadécimale invalide : « {s} »"))
}

fn cmd_map(exe: Option<PathBuf>) -> Result<()> {
    println!("== dispatchers funcLua* (build cartographié) ==");
    for d in map::DISPATCHERS {
        let star = if d.name == map::CAMERA_DISPATCHER.name {
            " <= caméra"
        } else {
            ""
        };
        println!(
            "  {:<26} table 0x{:X}  {:>5} commandes{star}",
            d.name, d.table_va, d.count
        );
    }
    println!("\n== points d'entrée caméra ==");
    println!(
        "  chaîne du dispatcher   0x{:X}",
        map::CAMERA_DISPATCHER_NAME_VA
    );
    println!(
        "  lua_CFunction          0x{:X}",
        map::CAMERA_DISPATCHER_ENTRY_VA
    );
    println!(
        "  variante interne       0x{:X}",
        map::CAMERA_DISPATCHER_ALT_VA
    );
    println!("  routine de dispatch    0x{:X}", map::DISPATCH_ROUTINE_VA);
    println!(
        "  réservoir funcLua      0x{:X} ({} entrées)",
        map::FUNCLUA_POOL_VA,
        map::FUNCLUA_POOL_COUNT
    );
    println!("  loader G4              0x{:X}", map::G4_LOADER_VA);
    println!(
        "  table des magics G4    0x{:X} (G4CM = index {})",
        map::G4_MAGIC_TABLE_VA,
        map::G4CM_MAGIC_INDEX
    );

    println!("\n== hiérarchie des contrôleurs ==");
    for k in CtrlKind::ALL {
        let depth = {
            let mut d = 0;
            let mut c = k;
            while let Some(b) = c.base() {
                c = b;
                d += 1;
            }
            d
        };
        let ported = if k.is_ported() { "porté" } else { "-" };
        println!(
            "  {:width$}{:<34} {ported}",
            "",
            k.cpp_name(),
            width = depth * 2
        );
    }

    println!("\n== caméras nommées de la scène ==");
    println!("  {}", map::SCENE_CAMERAS.join(", "));

    println!("\n== commandes d'entrée ==");
    for c in map::INPUT_COMMANDS.chunks(3) {
        println!("  {}", c.join("  "));
    }

    println!("\n== assets ==");
    for a in map::ASSETS {
        println!("  {:<62} {}", a.path, a.role);
    }

    if let Some(path) = exe {
        let bytes =
            std::fs::read(&path).with_context(|| format!("lecture de {}", path.display()))?;
        let issues = map::verify_against(&bytes);
        println!("\n== vérification contre {} ==", path.display());
        if issues.is_empty() {
            println!("  carte applicable : taille et ancre du dispatcher conformes");
        } else {
            for i in issues {
                println!("  ÉCART : {i}");
            }
        }
    }
    Ok(())
}

fn cmd_extract(out: &Path, game_dir: Option<PathBuf>, anims: bool) -> Result<()> {
    let mut vfs = nie_formats::vfs::Vfs::new();
    // `Vfs::init` attend le dossier `data/`, pas la racine du jeu.
    let dir = game_dir
        .unwrap_or_else(nie_formats::vfs::resolve_game_dir)
        .join("data");
    vfs.init(&dir)
        .map_err(|e| anyhow::anyhow!("ouverture du VFS ({}) : {e}", dir.display()))?;
    std::fs::create_dir_all(out)?;

    let mut ok = 0usize;
    let mut missing = Vec::new();
    for a in map::ASSETS {
        let internal = format!("data/{}", a.path);
        match vfs.read(&internal) {
            Ok(bytes) => {
                let name = Path::new(a.path).file_name().unwrap_or_default();
                std::fs::write(out.join(name), &bytes)?;
                ok += 1;
            }
            Err(_) => missing.push(a.path),
        }
    }
    println!("{ok} fichier(s) caméra extrait(s) vers {}", out.display());
    if !missing.is_empty() {
        println!("absents de ce build du jeu :");
        for m in missing {
            println!("  {m}");
        }
    }

    if anims {
        let anim_dir = out.join("anims");
        std::fs::create_dir_all(&anim_dir)?;
        let paths: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| p.ends_with("_camera.g4cm"))
            .collect();
        let mut n = 0usize;
        for p in &paths {
            if let Ok(bytes) = vfs.read(p) {
                let name = Path::new(p).file_name().unwrap_or_default();
                std::fs::write(anim_dir.join(name), &bytes)?;
                n += 1;
            }
        }
        println!(
            "{n} animation(s) .g4cm extraite(s) vers {}",
            anim_dir.display()
        );
    }
    Ok(())
}

fn cmd_decode(file: &Path, verbose: bool) -> Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("lecture de {}", file.display()))?;
    let anim = g4cm::decode(&bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("{}", file.display());
    println!(
        "  {} octets · version 0x{:04X} · {} objet(s) · {} canaux · {} temps",
        bytes.len(),
        anim.header.type_id,
        anim.object_count(),
        anim.channels.len(),
        anim.times.len()
    );
    if let Some((a, b)) = anim.frame_range() {
        println!("  frames {a} → {b}");
    }
    println!(
        "  échantillons décodés (flux f32) : {:.0} %",
        anim.decoded_ratio() * 100.0
    );
    for (i, o) in anim.objects.iter().enumerate() {
        let clip = anim.clips.get(i);
        let range = clip.map_or(String::new(), |c| format!(" clip {}→{}", c.start, c.end));
        println!(
            "  objet {i} « {} » — {} canaux{range}",
            anim.name_of(i),
            o.channel_count
        );
        for c in anim.channels_of(i) {
            let enc = match &c.track {
                Track::F32(_) => "f32",
                Track::Raw16(_) => "brut16",
                Track::Raw8(_) => "brut8",
            };
            let times = c.times(&anim);
            let span = if times.is_empty() {
                "-".to_string()
            } else {
                format!("{}→{}", times[0], times[times.len() - 1])
            };
            let vals = match c.track.values() {
                Some(v) if !v.is_empty() => {
                    let lo = v.iter().copied().fold(f32::INFINITY, f32::min);
                    let hi = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                    format!("[{lo:.3} .. {hi:.3}]")
                }
                _ => "(non décodé)".to_string(),
            };
            println!(
                "     {:<5} {enc:<7} {:>5} pts  frames {span:<13} {vals}",
                c.kind.label(),
                c.track.len()
            );
            if verbose && let Some(v) = c.track.values() {
                for (t, x) in times.iter().zip(v.iter()).take(16) {
                    println!("        {t:>6} {x:12.4}");
                }
            }
        }
    }
    Ok(())
}

fn cmd_encode(file: &Path, out: Option<PathBuf>, scale_pos: Option<f32>) -> Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("lecture de {}", file.display()))?;
    let mut anim = g4cm::decode(&bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    if let Some(k) = scale_pos {
        for c in &mut anim.channels {
            if let Track::F32(v) = &mut c.track {
                for x in v.iter_mut() {
                    *x *= k;
                }
            }
        }
    }
    let re = g4cm::encode(&anim).map_err(|e| anyhow::anyhow!("{e}"))?;
    if scale_pos.is_none() {
        if re == bytes {
            println!("round-trip byte-exact ({} octets)", re.len());
        } else {
            let diff = re.iter().zip(bytes.iter()).filter(|(a, b)| a != b).count();
            bail!(
                "round-trip NON exact : {diff} octet(s) diffèrent (taille {} vs {})",
                re.len(),
                bytes.len()
            );
        }
    }
    if let Some(o) = out {
        std::fs::write(&o, &re)?;
        println!("écrit : {} ({} octets)", o.display(), re.len());
    }
    Ok(())
}

fn cmd_verify(dir: &Path) -> Result<()> {
    let mut total = 0usize;
    let mut exact = 0usize;
    let mut failed = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("lecture de {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "g4cm"))
        .collect();
    entries.sort();
    for p in &entries {
        let Ok(bytes) = std::fs::read(p) else {
            continue;
        };
        total += 1;
        match g4cm::decode(&bytes).and_then(|a| g4cm::encode(&a)) {
            Ok(re) if re == bytes => exact += 1,
            Ok(re) => {
                let first = re
                    .iter()
                    .zip(bytes.iter())
                    .position(|(a, b)| a != b)
                    .map_or_else(
                        || "taille seule".to_string(),
                        |i| format!("1ᵉʳ écart à 0x{i:X}"),
                    );
                failed.push((
                    p.clone(),
                    format!("{first} (produit {} vs {} octets)", re.len(), bytes.len()),
                ));
            }
            Err(e) => failed.push((p.clone(), e.to_string())),
        }
    }
    println!("{exact}/{total} fichier(s) en round-trip byte-exact");
    for (p, why) in failed.iter().take(20) {
        println!(
            "  ÉCHEC {} : {why}",
            p.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    if failed.len() > 20 {
        println!("  … et {} autre(s)", failed.len() - 20);
    }
    Ok(())
}

fn cmd_config(file: &Path, preset: Option<String>) -> Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("lecture de {}", file.display()))?;

    if let Ok(cfg) = SoccerCameraConfig::parse(&bytes) {
        println!(
            "soccer_camera_config — {} lignes au total",
            cfg.total_rows()
        );
        println!(
            "  m_soccerCameraInfoDataList        {:>4}",
            cfg.camera_data.len()
        );
        println!(
            "  m_soccerCameraInfoList            {:>4}",
            cfg.cameras.len()
        );
        println!(
            "  m_scGoalnetCameraInfoList         {:>4}",
            cfg.goalnet.len()
        );
        println!(
            "  m_scAerialCameraInfoList          {:>4}",
            cfg.aerial.len()
        );
        println!(
            "  m_scAerialCameraMapInfoList       {:>4}",
            cfg.aerial_map.len()
        );
        println!(
            "  m_soccerDirCameraInfoList         {:>4}",
            cfg.dir_cameras.len()
        );
        println!(
            "  m_soccerFixPosCameraInfoDataList  {:>4}",
            cfg.fix_pos_data.len()
        );
        println!(
            "  m_cinematicCameraInfoDataList     {:>4}",
            cfg.cinematic_data.len()
        );
        if let Some(d) = cfg.camera_data.first() {
            println!(
                "\n  exemple (donnée 0) : length {} [{}..{}] rotX {} rotY {} fov {} \
                 interp move/rot/zoom {}/{}/{}",
                d.length,
                d.length_min,
                d.length_max,
                d.rot_x,
                d.rot_y,
                d.fov,
                d.move_interp_rate,
                d.rot_interp_rate,
                d.zoom_interp_rate
            );
        }
        for g in cfg.goalnet.iter().take(3) {
            println!(
                "  goalnet 0x{:08X} pos {:?} fov {} chaseMaxSpeed {}",
                g.id, g.cam_pos, g.fov, g.chase_max_speed
            );
        }
        return Ok(());
    }

    if let Ok(set) = PropertySet::parse(&bytes) {
        println!(
            "camera_ctrl_property_info — {} preset(s)",
            set.presets.len()
        );
        for name in set.names() {
            let p = &set.presets[name];
            let parent = p.parent.as_deref().unwrap_or("-");
            println!(
                "  {:<34} parent {:<24} {} param(s) propres",
                name,
                parent,
                p.params.len()
            );
        }
        if let Some(want) = preset {
            let resolved = set.resolve(&want);
            if resolved.is_empty() {
                bail!("preset « {want} » inconnu");
            }
            println!("\nparamètres effectifs de « {want} » (héritage résolu) :");
            for (k, v) in resolved {
                println!("  {k:<36} {v:?}");
            }
        }
        return Ok(());
    }

    let flat = FlatProperty::parse(&bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("{} — {} paramètre(s)", flat.name, flat.params.len());
    for (k, v) in &flat.params {
        println!("  {k:<28} {v:?}");
    }
    Ok(())
}

fn cmd_live(op: LiveOp) -> Result<()> {
    match op {
        LiveOp::Scan { process, limit } => {
            let pid = live::find_game(&process)
                .with_context(|| format!("process « {process} » introuvable"))?;
            println!("process {process} : pid {pid}");
            let cands = live::scan(
                pid,
                CameraLayout::default(),
                PlausibleRange::default(),
                limit,
            );
            println!(
                "{} candidat(s) (heuristique — à confirmer par un 2ᵉ scan) :",
                cands.len()
            );
            for c in cands.iter().take(limit) {
                println!(
                    "  0x{:X}  pos {:?} ref {:?} fov {:.1}",
                    c.addr, c.state.pos, c.state.ref_pos, c.state.fov_deg
                );
            }
            if !cands.is_empty() {
                println!(
                    "\nBougez la caméra en jeu puis relancez : seules les adresses dont l'état a \
                     changé sont de vrais candidats."
                );
            }
            Ok(())
        }
        LiveOp::Get { addr, process } => {
            let pid = live::find_game(&process)
                .with_context(|| format!("process « {process} » introuvable"))?;
            let cam = LiveCamera::at(pid, parse_addr(&addr)?, CameraLayout::default());
            let st = cam.read_state().map_err(|e| anyhow::anyhow!("{e}"))?;
            println!(
                "pos {:?}\nref {:?}\nfov {:.3}°\nroll {:.3}°\ndistance {:.3}",
                st.pos,
                st.ref_pos,
                st.fov_deg,
                st.roll_deg,
                st.length()
            );
            Ok(())
        }
        LiveOp::Set {
            addr,
            process,
            pos,
            target,
            fov,
            roll,
        } => {
            let pid = live::find_game(&process)
                .with_context(|| format!("process « {process} » introuvable"))?;
            let cam = LiveCamera::at(pid, parse_addr(&addr)?, CameraLayout::default());
            let mut st = cam.read_state().map_err(|e| anyhow::anyhow!("{e}"))?;
            if let Some(p) = pos {
                st.pos = parse_v3(&p)?;
            }
            if let Some(t) = target {
                st.ref_pos = parse_v3(&t)?;
            }
            if let Some(f) = fov {
                st.fov_deg = f;
            }
            if let Some(r) = roll {
                st.roll_deg = r;
            }
            cam.write_state(&st, PlausibleRange::default())
                .map_err(|e| anyhow::anyhow!("écriture refusée ou impossible : {e}"))?;
            println!(
                "écrit : pos {:?} ref {:?} fov {:.2}",
                st.pos, st.ref_pos, st.fov_deg
            );
            Ok(())
        }
    }
}

/// Contexte de preset déduit du nom de fichier `camera_ctrl_property_info*`.
fn preset_context(path: &str) -> &'static str {
    let f = path.rsplit('/').next().unwrap_or(path);
    if f.contains("_photo") {
        "photo"
    } else if f.contains("_rpg_battle") {
        "rpg_battle"
    } else if f.contains("_craft_edit") {
        "craft_edit"
    } else if f.contains("_screenshot") {
        "screenshot"
    } else if f.contains("_battle") {
        "battle"
    } else if f.starts_with("soccer_") {
        "soccer"
    } else {
        "default"
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "orchestration linéaire des six passes d'indexation"
)]
fn cmd_index(
    db_path: &Path,
    game_dir: Option<PathBuf>,
    exe: Option<PathBuf>,
    no_anims: bool,
    samples: bool,
    limit: Option<usize>,
) -> Result<()> {
    if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut database =
        db::open(db_path).with_context(|| format!("ouverture de la base {}", db_path.display()))?;
    println!("base : {} (migration caméra appliquée)", db_path.display());

    let mut total = db::IndexStats::default();

    // 1. Carte du reverse + paramètres du binaire.
    if let Some(exe_path) = exe.filter(|p| p.exists()) {
        let bytes = std::fs::read(&exe_path)
            .with_context(|| format!("lecture de {}", exe_path.display()))?;
        let label = format!("{} ({} octets)", exe_path.display(), bytes.len());
        let conn = database.conn();
        let src = db::upsert_source(
            conn,
            "exe",
            &label,
            Some(&db::sha256_hex(&bytes)),
            Some(bytes.len() as u64),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        let issues = map::verify_against(&bytes);
        if issues.is_empty() {
            println!("  carte RE applicable à ce binaire");
        } else {
            for i in &issues {
                println!(
                    "  AVERTISSEMENT : {i} — la carte reste indexée, marquée sur cette source"
                );
            }
        }
        total = total.merged(db::index_map(conn, src).map_err(|e| anyhow::anyhow!("{e}"))?);
        total = total.merged(
            db::index_binary_params(conn, src, &bytes).map_err(|e| anyhow::anyhow!("{e}"))?,
        );
        println!(
            "  carte : {} contrôleurs, {} dispatchers, {} symboles, {} paramètres",
            total.ctrl_classes, total.dispatchers, total.re_symbols, total.params
        );
    } else {
        println!("  (pas de nie.exe fourni : carte RE et paramètres non indexés)");
    }

    // 2. VFS : assets, configs, presets.
    let mut vfs = nie_formats::vfs::Vfs::new();
    let root = game_dir.unwrap_or_else(nie_formats::vfs::resolve_game_dir);
    let data_dir = root.join("data");
    vfs.init(&data_dir)
        .map_err(|e| anyhow::anyhow!("ouverture du VFS ({}) : {e}", data_dir.display()))?;

    let vfs_src = {
        let conn = database.conn();
        db::upsert_source(conn, "vfs", &root.display().to_string(), None, None)
            .map_err(|e| anyhow::anyhow!("{e}"))?
    };

    for a in map::ASSETS {
        let internal = format!("data/{}", a.path);
        let bytes = vfs.read(&internal).ok();
        let conn = database.conn();
        let asset_id = db::upsert_asset(conn, vfs_src, a.path, Some(a.role), bytes.as_deref())
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        total.assets += 1;
        let Some(bytes) = bytes else { continue };

        if a.path.contains("soccer_camera_config")
            && let Ok(cfg) = SoccerCameraConfig::parse(&bytes)
        {
            let st = db::index_soccer_config(conn, asset_id, &cfg)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("  {} : {} lignes de config", a.path, st.config_rows);
            total = total.merged(st);
        } else if a.path.contains("camera_ctrl_property_info")
            && let Ok(set) = PropertySet::parse(&bytes)
        {
            let st = db::index_property(conn, asset_id, preset_context(a.path), &set)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!(
                "  {} : {} presets, {} paramètres",
                a.path, st.presets, st.preset_params
            );
            total = total.merged(st);
        }
    }

    // 3. Animations.
    if !no_anims {
        let mut paths: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| p.ends_with("_camera.g4cm"))
            .collect();
        paths.sort();
        if let Some(n) = limit {
            paths.truncate(n);
        }
        let anim_src = {
            let conn = database.conn();
            db::upsert_source(
                conn,
                "anim",
                &format!("g4cm @ {}", root.display()),
                None,
                None,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?
        };

        let tx = database
            .conn_mut()
            .transaction()
            .context("ouverture de la transaction d'indexation des animations")?;
        let mut failed = 0usize;
        for (i, p) in paths.iter().enumerate() {
            let Ok(bytes) = vfs.read(p) else {
                failed += 1;
                continue;
            };
            match db::index_anim(&tx, anim_src, p, &bytes, samples) {
                Ok(st) => total = total.merged(st),
                Err(e) => {
                    failed += 1;
                    if failed <= 5 {
                        println!("  ÉCHEC {p} : {e}");
                    }
                }
            }
            if (i + 1) % 250 == 0 {
                println!("  … {} / {} animations", i + 1, paths.len());
            }
        }
        tx.commit().context("validation de la transaction")?;
        println!(
            "  animations : {} indexées ({failed} échec(s)), {} canaux, {} échantillons",
            total.anims, total.channels, total.samples
        );
    }

    print_stats(database.conn())?;
    Ok(())
}

fn print_stats(conn: &nie_index::rusqlite::Connection) -> Result<()> {
    let row: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = conn.query_row(
        "SELECT anims, anims_roundtrip_ok, channels, channels_decoded, samples_total,
                samples_decoded, ctrl_classes, ctrl_ported, params, presets,
                assets_present, assets_known
           FROM v_cam_coverage",
        [],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
                r.get(11)?,
            ))
        },
    )?;
    println!("\n== index caméra ==");
    println!(
        "  animations        {:>8}  (round-trip exact : {})",
        row.0, row.1
    );
    println!("  canaux            {:>8}  (décodés : {})", row.2, row.3);
    println!("  échantillons      {:>8}  (décodés : {})", row.4, row.5);
    println!("  contrôleurs       {:>8}  (portés : {})", row.6, row.7);
    println!("  paramètres        {:>8}", row.8);
    println!("  presets           {:>8}", row.9);
    println!("  assets présents   {:>8} / {}", row.10, row.11);

    let mut stmt = conn.prepare(
        "SELECT kind, encoding, n_channels, n_samples FROM v_cam_channel_stats
          ORDER BY kind, encoding",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    let mut first = true;
    for row in rows {
        let (kind, enc, n, s) = row?;
        if first {
            println!("\n  canal   encodage   canaux   échantillons");
            first = false;
        }
        println!("  {kind:<7} {enc:<9} {n:>7} {s:>14}");
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Map { exe } => cmd_map(exe),
        Cmd::Extract {
            out,
            game_dir,
            anims,
        } => cmd_extract(&out, game_dir, anims),
        Cmd::Decode { file, verbose } => cmd_decode(&file, verbose),
        Cmd::Encode {
            file,
            out,
            scale_pos,
        } => cmd_encode(&file, out, scale_pos),
        Cmd::Verify { dir } => cmd_verify(&dir),
        Cmd::Config { file, preset } => cmd_config(&file, preset),
        Cmd::Live { op } => cmd_live(op),
        Cmd::Index {
            db,
            game_dir,
            exe,
            no_anims,
            samples,
            limit,
        } => cmd_index(&db, game_dir, exe, no_anims, samples, limit),
        Cmd::Stats { db } => {
            let database =
                db::open(&db).with_context(|| format!("ouverture de la base {}", db.display()))?;
            print_stats(database.conn())
        }
    }
}
