//! **nie-play** — front-end HEADLESS/GOLDEN du jeu niers.
//!
//! Mince client de [`nie_app`] : il fournit un Renderer CPU + un flow scripté, exécute la machine à
//! états du cœur ([`nie_app::GameState`]), et écrit le playthrough en PNG (→ MP4 via ffmpeg). Le
//! match vient de la vraie FSM `nie_core::simulate_match`. Le runner INTERACTIF (GPU, input temps-réel)
//! est `nie-game` — un autre front-end du même cœur (cf. `docs/PLAN.md`).
//!
//! Usage : `nie-play --font-cfg <font.cfg.bin> --font-g4tx <font.g4tx> [--char-{md,mg,sk,tex} …] --out <dir>`

use anyhow::{Context, Result};
use clap::Parser;
use nie_app::render::CharAssets;
use nie_app::{CpuRenderer, H, Renderer, W, demo_flow};
use nie_core::match_sim::{TeamSetup, simulate_match};
use nie_core::stats::StatBlock;

#[derive(Parser)]
struct Cli {
    /// `font.cfg.bin` (métriques de police). Par défaut : résolu par le VFS du jeu.
    #[arg(long)]
    font_cfg: Option<std::path::PathBuf>,
    /// `font.g4tx` (atlas de police). Par défaut : résolu par le VFS du jeu.
    #[arg(long)]
    font_g4tx: Option<std::path::PathBuf>,
    /// Cache des assets matérialisés depuis les packs (inutilisé sur un dump : les fichiers
    /// y sont déjà sur disque et servis sans copie).
    #[arg(long, default_value = "var/cache/assets")]
    cache: std::path::PathBuf,
    /// Répertoire de sortie des frames.
    #[arg(long, default_value = "/tmp/nie-play")]
    out: std::path::PathBuf,
    /// Graine du match.
    #[arg(long, default_value = "12345")]
    seed: u64,
    /// Perso 3D affiché sur les écrans (optionnel) : mesh g4md.
    #[arg(long)]
    char_md: Option<std::path::PathBuf>,
    /// Perso 3D : géométrie g4mg.
    #[arg(long)]
    char_mg: Option<std::path::PathBuf>,
    /// Perso 3D : squelette g4sk.
    #[arg(long)]
    char_sk: Option<std::path::PathBuf>,
    /// Perso 3D : texture g4tx (BC7).
    #[arg(long)]
    char_tex: Option<std::path::PathBuf>,
    /// Équipe domicile depuis de VRAIS chara_param (`chara_param_*.cfg.bin.json`) au lieu de stats en dur.
    #[arg(long)]
    chara_param: Option<std::path::PathBuf>,
    /// Base SQLite (miroir inagle) : charge une VRAIE scène de dialogue (`inagle_event_subtitles`).
    #[arg(long)]
    db: Option<std::path::PathBuf>,
    /// `event_id` de la scène à charger (défaut : une scène multi-lignes connue).
    #[arg(long, default_value = "ev07_02900")]
    event_id: String,
}

/// Charge une vraie scène de dialogue (lignes EN consécutives d'un event), nettoyées.
fn load_scene(db: &std::path::Path, event_id: &str) -> Result<Vec<String>> {
    let conn =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .context("ouverture DB")?;
    let mut stmt = conn.prepare(
        "SELECT text_en FROM inagle_event_subtitles WHERE event_id=?1 AND text_en IS NOT NULL \
         AND length(text_en)>0 ORDER BY line_index",
    )?;
    let lines: Vec<String> = stmt
        .query_map([event_id], |r| r.get::<_, String>(0))?
        .filter_map(Result::ok)
        .map(|s| nie_app::story::clean_dialogue(&s))
        .filter(|s| !s.is_empty())
        .collect();
    anyhow::ensure!(!lines.is_empty(), "aucune ligne pour l'event {event_id}");
    Ok(lines)
}

fn team(name: &str, base: u16) -> TeamSetup {
    TeamSetup::new(
        name,
        StatBlock {
            kc: base,
            cr: base,
            tc: base,
            pr: base,
            ps: base,
            ag: base,
            it: base,
        },
    )
}

fn write_png(buf: &[u8], path: &std::path::Path) -> Result<()> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(std::io::Cursor::new(&mut out), W as u32, H as u32);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()?.write_image_data(buf)?;
    }
    std::fs::write(path, &out)?;
    Ok(())
}

/// Chemins logiques des assets de police par défaut — ceux que le jeu charge lui-même.
const FONT_CFG: &str = "data/common/font/font/font_def/font.cfg.bin";
/// Atlas de police correspondant (`FONT_CFG`), côté ressources rendues.
const FONT_G4TX: &str = "data/dx11/font/font_def/font.g4tx";

/// Résout les deux assets de police : ceux passés en argument, sinon ceux du jeu.
///
/// Le VFS sert les mêmes chemins logiques qu'on tourne sur une installation ou sur un dump
/// extrait — c'est ce qui permet de lancer le playthrough sans connaître ni chemin disque ni
/// mode de montage. Sur un dump, aucun octet n'est copié.
fn assets_police(cli: &Cli) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    if let (Some(cfg), Some(g4tx)) = (&cli.font_cfg, &cli.font_g4tx) {
        return Ok((cfg.clone(), g4tx.clone()));
    }
    let vfs = nie_formats::vfs::open_game()
        .map_err(|e| anyhow::anyhow!("aucune donnee de jeu (ni installation ni dump) : {e:?}"))?;
    let montage = if vfs.is_dump() { "dump" } else { "packs" };
    let mat = |logique: &str| -> Result<std::path::PathBuf> {
        vfs.materialiser(logique, &cli.cache)
            .map_err(|e| anyhow::anyhow!("asset {logique} indisponible : {e:?}"))
    };
    let cfg = match &cli.font_cfg {
        Some(p) => p.clone(),
        None => mat(FONT_CFG)?,
    };
    let g4tx = match &cli.font_g4tx {
        Some(p) => p.clone(),
        None => mat(FONT_G4TX)?,
    };
    println!(
        "[nie-play] police resolue par le VFS ({montage}) : {}",
        cfg.display()
    );
    Ok((cfg, g4tx))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    std::fs::create_dir_all(&cli.out)?;
    let (font_cfg, font_g4tx) = assets_police(&cli)?;

    // Équipe domicile : vrais chara_param si fournis (pont données Phase 2), sinon stats en dur.
    let home = match &cli.chara_param {
        Some(p) => {
            let t = nie_app::roster::team_from_chara_param_json("RAIMON", p, 11, 99)
                .context("chargement équipe depuis chara_param")?;
            println!("[nie-play] équipe RAIMON = 11 vrais chara_param (lv99) → stats réelles");
            t
        }
        None => team("RAIMON", 120),
    };
    // Match RÉEL via la FSM portée (nie-core).
    let res = simulate_match(home, team("ROYAL ACADEMY", 95), cli.seed);

    // Renderer CPU (police + perso 3D optionnel), fourni au cœur nie-app.
    let chr = match (&cli.char_md, &cli.char_mg, &cli.char_sk, &cli.char_tex) {
        (Some(md), Some(mg), Some(sk), Some(tex)) => Some(CharAssets { md, mg, sk, tex }),
        _ => None,
    };
    let renderer = CpuRenderer::new(&font_cfg, &font_g4tx, chr).context("init renderer")?;

    // Playthrough : la machine à états de nie-app. Avec --db, le segment Histoire joue une VRAIE
    // scène de dialogue IEVR (sinon le flow de démo).
    let flow: Vec<(nie_app::GameState, u32)> = if let Some(db) = &cli.db {
        let scene = load_scene(db, &cli.event_id).context("chargement scène")?;
        println!(
            "[nie-play] scène réelle '{}' = {} lignes de dialogue",
            cli.event_id,
            scene.len()
        );
        let mut f = vec![
            (nie_app::GameState::Title, 30),
            (nie_app::GameState::MainMenu { sel: 0 }, 20),
            (
                nie_app::GameState::Match {
                    home: res.home_score,
                    away: res.away_score,
                },
                40,
            ),
            (nie_app::GameState::MainMenu { sel: 1 }, 20),
        ];
        // Locuteur exact non résolu (line_label = id, pas un nom) → libellé de scène honnête.
        let speaker = format!("Mode Histoire — {}", cli.event_id);
        for line in scene {
            f.push((
                nie_app::GameState::Story {
                    speaker: speaker.clone(),
                    line,
                },
                35,
            ));
        }
        f.push((nie_app::GameState::MainMenu { sel: 3 }, 20));
        f
    } else {
        demo_flow(res.home_score, res.away_score)
    };
    let mut frame = 0u32;
    for (state, dur) in &flow {
        println!("[nie-play] etat = {state:?}");
        match state {
            // Le match se JOUE : sim physique nie-runtime (best-effort, approximative — PAS la
            // boucle C++ byte-fidèle, cf. docs/PLAN.md fracture #1) rendue frame par frame.
            nie_app::GameState::Match { .. } => {
                let mut world = nie_runtime::World::kickoff();
                for _ in 0..*dur {
                    world.step(1.0 / 30.0);
                    let fr = nie_runtime::render::render(&world, W as u32, H as u32);
                    write_png(&fr.px, &cli.out.join(format!("f{frame:04}.png")))?;
                    frame += 1;
                }
                println!(
                    "[nie-play]   match JOUÉ (nie-runtime, physique) score={:?}",
                    world.score
                );
            }
            _ => {
                let buf = renderer.render(state);
                for _ in 0..*dur {
                    write_png(&buf, &cli.out.join(format!("f{frame:04}.png")))?;
                    frame += 1;
                }
            }
        }
    }
    println!(
        "[nie-play] playthrough = {frame} frames → {} (match RAIMON {}-{} ROYAL via FSM nie-core, coeur nie-app)",
        cli.out.display(),
        res.home_score,
        res.away_score
    );
    Ok(())
}
