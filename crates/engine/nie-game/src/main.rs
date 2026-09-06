//! `nie-game` — hôte GUI natif wgpu, pilier D1/C4 pixel-perfect de niers.
//!
//! ## Modes d'exécution
//!
//! - `--capture <PNG>` : rendu hors-écran (pas de fenêtre, adaptateur logiciel accepté)
//!   → lecture du framebuffer GPU → écriture d'un fichier PNG. Format render-target :
//!   `Rgba8Unorm`, échantillonneur `Nearest`, sans conversion sRGB : le PNG capturé est
//!   pixelicellement fidèle aux octets RGBA8 décodés depuis le DDS d'origine.
//!
//! - `--window [--frames N]` : ouvre une fenêtre winit + surface wgpu, affiche la texture
//!   en temps réel, se ferme automatiquement après N trames (défaut 120).
//!
//! - `--list <N>` : liste les N premiers fichiers `.g4tx` du VFS (ou CPK scan) avec
//!   leurs dimensions. Utile pour découvrir les assets disponibles.
//!
//! - `--menu <SCREEN> --capture <PNG>` : compose l'écran de menu en PNG via le
//!   compositeur CPU (référence pixel-perfect).
//!
//! - `--menu <SCREEN> --gpu --capture <PNG>` : même composition mais sur GPU (offscreen
//!   wgpu 1280×720 Rgba8Unorm, blend straight-alpha over, filtrage linéaire).
//!   Ajouter `--verify` pour comparer automatiquement CPU vs GPU et vérifier la fidélité
//!   (≥99 % des pixels dans une tolérance de 4/255 par canal).
//!
//! ## Note VFS
//!
//! Le `vfs.init()` est tenté en premier. Sur l'installation Steam courante, le
//! `cpk_list.cfg.bin` utilise un format/chiffrement incompatible avec le parseur T2B de
//! `nie-formats::cfgbin` — ce qui provoque un panic dans les builds de débogage (overflow
//! détecté à `cfgbin.rs:693`). Dans ce cas, `catch_unwind` intercepte l'erreur et bascule
//! sur un scan CPK direct : chaque `.cpk` de `data/packs/` est parcouru via
//! `nie_formats::cpk::CpkReader` pour indexer les `.g4tx` sans passer par le VFS.
//!
//! Ce bug pré-existant dans `nie-formats` affecte tous les binaires du workspace
//! (nie-model-serve, nie-headless, etc.) avec la même installation.

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::Parser;
use tracing::{debug, error, info, warn};
use wgpu::util::DeviceExt;

mod gpu_select;

use nie_formats::vfs::Vfs;
use nie_formats::{cfgbin, font, g4pkm, g4tx, g4tx_decode, menu, objbin};
// Primitives 2D pures centralisées dans nie-formats::raster2d (dédup Phase 2 ; le blend reste local, landmine #5).
use nie_formats::raster2d::{crop_rgba, scale_nearest};

// ── CLI ──────────────────────────────────────────────────────────────────────

// Aucun défaut de racine de jeu n'est codé ici : `nie_formats::vfs::resolve_game_dir` la résout à
// l'exécution (`NIE_GAME_DIR`, puis le répertoire courant et ses ancêtres portant
// `data/cpk_list.cfg.bin`, puis le répertoire de l'exécutable). Un chemin de poste en dur rend le
// binaire inutilisable partout ailleurs — et sur une installation Steam, la racine du jeu EST le
// répertoire courant.

/// Hôte GUI natif wgpu — pilier D1/C4 pixel-perfect pour niers.
///
/// Monte le VFS CPK Steam, décode une texture `.g4tx` réelle en RGBA8, l'envoie
/// sur GPU via un pipeline wgpu plein écran.
#[derive(Parser, Debug)]
#[command(name = "nie-game", version, about, long_about = None)]
struct Cli {
    /// Répertoire racine du jeu (contient `data/cpk_list.cfg.bin`). Résolu automatiquement s'il
    /// est absent : `NIE_GAME_DIR`, sinon le répertoire courant ou l'un de ses ancêtres.
    #[arg(long)]
    game_dir: Option<PathBuf>,

    /// Chemin interne VFS de la texture `.g4tx` à rendre.
    /// Si absent, sélection automatique de la plus grande texture DDS parmi les
    /// candidats `menu`, `title`, `boot`, `ui`, `logo`.
    #[arg(long)]
    g4tx: Option<String>,

    /// Mode rendu hors-écran : écrit le framebuffer en PNG au chemin indiqué.
    /// Format `Rgba8Unorm`, échantillonneur `Nearest`, sans sRGB.
    #[arg(long)]
    capture: Option<PathBuf>,

    /// Mode fenêtré : ouvre une fenêtre winit + surface wgpu.
    #[arg(long)]
    window: bool,

    /// **Mode jouable** : ouvre le jeu et le pilote au clavier (titre → menu → match).
    ///
    /// La FSM et la physique viennent du cœur (`nie_app::flow::Screen`, `nie_runtime::World`) —
    /// ce mode ne fait que les brancher sur une fenêtre : flèches ou ZQSD/WASD pour naviguer,
    /// Entrée ou Espace pour valider, Échap pour revenir. La police est résolue par le VFS,
    /// aucune option n'est requise.
    #[arg(long)]
    play: bool,

    /// Mode découverte : liste les N premiers `.g4tx` du VFS (avec dimensions).
    #[arg(long)]
    list: Option<usize>,

    /// Diagnostic render-from-runtime : charge le `.g4tx` désigné par `--g4tx <path>` et liste ses
    /// **régions d'atlas** (nom + rect `x,y,w,h`) — les cibles de `SetIconSprite(obj, CRC32(path),
    /// CRC32(region))`. Sert à prouver que `region_name` (runtime) → sous-texture réelle.
    #[arg(long)]
    g4tx_regions: bool,

    /// Render-from-runtime : ROGNE la région d'atlas nommée du `.g4tx` (`--g4tx <path>`) et, avec
    /// `--capture <PNG>`, écrit ses pixels réels. C'est l'étape « commande runtime → pixels » : le
    /// crop `(x,y,w,h)` résolu (`g4tx::region_rect`) extrait le sprite exact de la texture DDS.
    #[arg(long)]
    g4tx_region: Option<String>,

    /// Construit l'index `nom-de-région → chemin g4tx` (render-from-runtime) en chargeant les atlas
    /// d'icônes listés dans `data/re/menu-icon-atlases.txt` et en indexant TOUTES leurs régions.
    /// Écrit le JSON au chemin indiqué (consommé pour résoudre les `spriteRegion` du runtime →
    /// fichier g4tx → rect). C'est le chaînon `gtxt_rarity01_05 → icon_rarity.g4tx`.
    #[arg(long)]
    build_region_index: Option<PathBuf>,

    /// Render-from-runtime FINAL : COMPOSE un layout JSON (produit par `--runtime --export-layout`)
    /// en PNG (`--capture`), en rognant chaque `spriteRect` de son `spriteRegionG4tx` et en le posant
    /// au `transform` de l'objet (ancre + échelle). C'est « données runtime → image composée ».
    ///
    /// Répétable : un écran du jeu EMPILE plusieurs calques (l'éditeur d'avatar superpose son écran
    /// principal, son panneau de parts et sa liste). Les objets de tous les layouts sont triés
    /// ensemble par priorité de dessin ; à priorité égale, l'ordre des `--compose-layout` décide.
    #[arg(long)]
    compose_layout: Vec<PathBuf>,

    /// Nombre de trames avant fermeture automatique (`0` = fenêtre persistante).
    ///
    /// Le défaut dépend du mode, et c'est voulu : `--window` sert de visionneuse et de test CI,
    /// donc il se ferme seul après 120 trames ; `--play` est un jeu, il reste ouvert jusqu'à ce
    /// qu'on le ferme. Passer `--frames` explicitement s'applique aux deux — c'est ainsi qu'on
    /// scripte une session de jeu bornée.
    #[arg(long)]
    frames: Option<u32>,

    /// Mode rendu de menu : compose l'écran `SCREEN` en PNG (requiert --capture,
    /// exclusif avec --window/--list). Ex. : `win01_21`, `title00`, `option02_02`.
    #[arg(long)]
    menu: Option<String>,

    /// Compose l'écran depuis sa définition `<menu>_menu_setting.cfg.bin` (liste `MENU_LAYER_INFO`,
    /// D1.c-driver brique (a)) au lieu du filtre par préfixe de nom d'objbin. La valeur de `--menu`
    /// est alors le préfixe du fichier setting (ex. `main_menu` → `main_menu_setting.cfg.bin`).
    #[arg(long)]
    from_setting: bool,

    /// Rendu GPU du menu (requiert --menu + --capture).
    /// Rend les sprites via un pipeline wgpu offscreen 1280×720 Rgba8Unorm
    /// avec blend straight-alpha over et filtrage linéaire, au lieu du compositeur CPU.
    #[arg(long)]
    gpu: bool,

    /// Après rendu GPU, compare pixel-à-pixel avec le compositeur CPU de référence.
    /// Imprime : max diff canal, % pixels dans tolérance 4/255, tailles PNG.
    /// Échoue si moins de 99 % des pixels sont dans la tolérance. Requiert --gpu.
    #[arg(long)]
    verify: bool,

    /// Exporte le LAYOUT de l'écran `--menu` en JSON (contrat `@rose-griffon/menu-render`,
    /// consommé par azalee). Au lieu de composer un PNG, écrit objets + transforms (placement
    /// motion-fallback D1.a + sélection texture D1.b, déterministe) au chemin indiqué.
    #[arg(long)]
    export_layout: Option<PathBuf>,

    /// Nom d'écran à écrire dans le champ `screen` du layout exporté (défaut = valeur de `--menu`).
    /// Ex. : niers énumère le préfixe objbin `mainmenu` mais azalee attend `100_mainmenu`.
    #[arg(long)]
    screen_name: Option<String>,

    /// Génère le layout AU RUNTIME comme nie.exe : au lieu d'exporter le layout statique,
    /// exécute les vrais scripts Lua de l'écran (driver reversé : `OnInit`/`OnSetupLayer`/
    /// `OnOpenLayer`) dans la VM Lua 5.2 réelle, récupère le `MenuState` produit, et l'applique
    /// au layout (visibilité/sprite/texte par objet, joint via crc32 du nom). Requiert
    /// `--menu` + `--export-layout`.
    #[arg(long)]
    runtime: bool,

    /// Diagnostic **C4 / D1.d** (rendu de texte) : blit `TEXT` depuis l'atlas de police bitmap RÉEL
    /// (`font_def/font.g4tx` + métriques `font_def/font.cfg.bin`) via `font::draw_text`. Avec
    /// `--capture`, écrit le PNG. Cible du gate D1.d (« 212 / 99 / COMMENCER » depuis l'atlas pré-cuit).
    #[arg(long)]
    render_text: Option<String>,
}

// ── Point d'entrée ───────────────────────────────────────────────────────────

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    // La racine du jeu vaut celle passée en argument, sinon celle que le contexte désigne. Elle
    // est résolue une fois ici plutôt qu'à chaque site d'appel.
    let game_dir = cli
        .game_dir
        .clone()
        .unwrap_or_else(nie_formats::vfs::resolve_game_dir);
    info!("racine du jeu : {}", game_dir.display());

    // Mode --play : le jeu jouable. Traité en premier — il ne consomme aucune des options de
    // diagnostic (texture, écran de menu, liste) et n'a rien à valider contre elles.
    if cli.play {
        return cmd_play(cli.frames.unwrap_or(0));
    }

    // Mode --menu : rendu d'un écran de menu complet → PNG (requiert --capture,
    // exclusif avec --window/--list). Traité avant la validation générique des modes.
    if let Some(ref screen) = cli.menu {
        if cli.list.is_some() {
            bail!("--menu ne peut pas être combiné avec --list");
        }
        // --menu --export-layout : exporte le layout JSON (azalee) au lieu de composer un PNG.
        if let Some(ref out) = cli.export_layout {
            let name = cli.screen_name.as_deref().unwrap_or(screen);
            // --runtime : génère le layout en exécutant les vrais scripts Lua (comme nie.exe).
            if cli.runtime {
                return cmd_export_layout_runtime(
                    &game_dir,
                    screen,
                    name,
                    out,
                    cli.from_setting,
                    cli.frames.unwrap_or(1),
                );
            }
            return cmd_export_layout(&game_dir, screen, name, out, cli.from_setting);
        }
        if cli.runtime {
            bail!("--runtime requiert --export-layout <JSON>");
        }
        // --menu --window : fenêtre PERSISTANTE affichant l'écran de menu composé
        // (reste ouverte jusqu'à fermeture). C'est le mode « voir le jeu » à l'écran.
        if cli.window {
            return cmd_menu_window(&game_dir, screen);
        }
        let png_out = cli
            .capture
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("--menu requiert --capture <PNG> (ou --window)"))?;
        if cli.gpu {
            return cmd_menu_gpu(&game_dir, screen, png_out, cli.verify);
        }
        if cli.verify {
            bail!("--verify requiert --gpu");
        }
        return cmd_menu(&game_dir, screen, png_out, cli.from_setting);
    }

    if cli.gpu || cli.verify {
        bail!("--gpu/--verify requièrent --menu <SCREEN>");
    }

    // Diagnostic régions d'atlas (render-from-runtime) : indépendant des modes capture/window/list.
    if cli.g4tx_regions {
        return cmd_g4tx_regions(&game_dir, cli.g4tx.as_deref());
    }

    // Render-from-runtime : rogne une région nommée → pixels réels (PNG si --capture).
    if let Some(ref region) = cli.g4tx_region {
        return cmd_g4tx_region(
            &game_dir,
            cli.g4tx.as_deref(),
            region,
            cli.capture.as_deref(),
        );
    }

    // Diagnostic C4/D1.d : rend du texte depuis l'atlas de police bitmap réel.
    if let Some(ref text) = cli.render_text {
        return cmd_render_text(&game_dir, text, cli.capture.as_deref());
    }

    // Construit l'index region->g4tx depuis les atlas d'icônes de menu.
    if let Some(ref out) = cli.build_region_index {
        return cmd_build_region_index(&game_dir, out);
    }

    // Render-from-runtime final : compose un ou plusieurs layouts JSON empilés en PNG.
    if !cli.compose_layout.is_empty() {
        let png = cli
            .capture
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("--compose-layout requiert --capture <PNG>"))?;
        return cmd_compose_layout(&game_dir, &cli.compose_layout, png);
    }

    let n_modes = [cli.capture.is_some(), cli.window, cli.list.is_some()]
        .iter()
        .filter(|&&x| x)
        .count();
    if n_modes == 0 {
        bail!("préciser --capture <PNG>, --window ou --list <N>");
    }
    if n_modes > 1 {
        bail!("--capture, --window et --list sont mutuellement exclusifs");
    }

    if let Some(n) = cli.list {
        return cmd_list(&game_dir, n);
    }

    let (vfs_path, width, height, rgba) = charger_texture(&game_dir, cli.g4tx.as_deref())?;

    info!(
        "texture chargée : {vfs_path}  {width}x{height}  {} octets RGBA8",
        rgba.len()
    );

    if let Some(png_out) = cli.capture {
        return cmd_capture(&rgba, width, height, &png_out);
    }

    if cli.window {
        return cmd_window(&rgba, width, height, cli.frames.unwrap_or(120));
    }

    unreachable!()
}

// ── Décodage DDS → RGBA8 ─────────────────────────────────────────────────────
// Le décodeur G4TX/DDS → RGBA8 est centralisé dans `nie_formats::g4tx_decode`
// (feature `textures`, source unique du workspace — Phase 1b dédup). On l'appelle via
// `g4tx_decode::decode_texture_rgba`. La variante partagée (model-serve) est un superset :
// elle gère DX10 + FourCC legacy + non compressé (BGRA8/RGBA8), donc couvre le cas atlas
// de police (legacy 32 bpp) que gérait l'ancienne copie locale `decode_legacy_uncompressed`.

// ── Source d'assets ──────────────────────────────────────────────────────────

/// Un asset `.g4tx` trouvé dans un CPK (scan direct, sans VFS).
#[derive(Debug, Clone)]
struct CpkAsset {
    /// Chemin interne VFS reconstitué (`directory/filename`).
    internal_path: String,
    /// Nom de base du fichier CPK (ex. `abc123.cpk`).
    cpk_basename: String,
}

/// Tente de monter le VFS (peut paniquer en mode debug sur cette installation Steam :
/// bug cfgbin.rs:693 overflow dans `parse_t2b`, intercepté par `catch_unwind`).
/// Retourne `Some(vfs)` si réussi, `None` sinon.
fn tenter_vfs(data_dir: &Path) -> Option<Vfs> {
    use std::panic::AssertUnwindSafe;
    let data_dir = data_dir.to_path_buf();

    match std::panic::catch_unwind(AssertUnwindSafe(move || {
        let mut vfs = Vfs::new();
        let result = vfs.init(&data_dir);
        (vfs, result)
    })) {
        Ok((vfs, Ok(()))) => {
            info!("VFS monté : {} assets", vfs.asset_count());
            Some(vfs)
        }
        Ok((_, Err(e))) => {
            warn!(
                "VFS init échoué (format cpk_list.cfg.bin incompatible : {e}), \
                 repli sur scan CPK direct"
            );
            None
        }
        Err(_) => {
            warn!(
                "VFS init panique intercepté (bug cfgbin.rs parse_t2b overflow, \
                 build debug) — repli sur scan CPK direct"
            );
            None
        }
    }
}

/// Scanne les CPK du répertoire `packs/` pour indexer les fichiers `.g4tx`.
///
/// Lit seulement les premiers 128 Kio de chaque CPK (suffisant pour le TOC),
/// Détermine si un chemin interne VFS mérite d'être promu en prioritaire (UI, menu…).
///
/// Utilise une vérification par SEGMENT de chemin (pas `contains`) pour éviter les
/// faux positifs : "mannequin" contient "ui" comme sous-chaîne mais n'est pas une
/// texture d'interface.
fn est_chemin_prioritaire(internal_path: &str) -> bool {
    internal_path.split('/').any(|seg| {
        let s = seg.to_ascii_lowercase();
        // Correspondance exacte
        matches!(s.as_str(), "menu" | "ui" | "title" | "boot" | "logo")
            // Préfixes avec séparateur : menu_01, ui_common, title_screen…
            || s.starts_with("menu_")
            || s.starts_with("ui_")
            || s.starts_with("title_")
            || s.starts_with("boot_")
            || s.starts_with("logo_")
            // Répertoires numériques de menu : 00_soccer, 01_title…
            || (s.len() > 3 && s[2..].starts_with("_menu"))
    })
}

/// ce qui permet de parcourir les 933 archives sans tout charger en RAM.
/// Retourne la liste de tous les `.g4tx` trouvés (priorité : chemins menu/ui/title/boot).
fn scanner_cpks_g4tx(game_dir: &Path) -> Result<Vec<CpkAsset>> {
    use nie_formats::cpk::CpkReader;

    let packs_dir = game_dir.join("data").join("packs");
    let mut cpk_fichiers: Vec<PathBuf> = std::fs::read_dir(&packs_dir)
        .with_context(|| format!("lecture packs/ : {}", packs_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "cpk"))
        .collect();
    cpk_fichiers.sort();

    info!(
        "scan CPK direct : {} archives dans {}",
        cpk_fichiers.len(),
        packs_dir.display()
    );

    // Taille de lecture partielle : 128 Kio suffisent pour le header + TOC des CPK IEVR
    // (sondé : TOC à offset ~2048, taille ~1416 octets → total ~3.5 Kio seulement).
    const LECTURE_PARTIELLE: usize = 128 * 1024;

    let mut prioritaires: Vec<CpkAsset> = Vec::new();
    let mut generaux: Vec<CpkAsset> = Vec::new();
    let mut cpk_scanned = 0usize;

    for cpk_path in &cpk_fichiers {
        let basename = cpk_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Lecture partielle du CPK (header + TOC uniquement)
        let donnees = match lire_premiers_octets(cpk_path, LECTURE_PARTIELLE) {
            Ok(d) => d,
            Err(e) => {
                warn!("skip {basename}: {e}");
                continue;
            }
        };

        let reader = match CpkReader::new(&donnees, &basename) {
            Ok(r) => r,
            Err(_) => continue,
        };

        cpk_scanned += 1;
        for entry in &reader.entries {
            if !entry.filename.to_ascii_lowercase().ends_with(".g4tx") {
                continue;
            }
            let path = format!("{}/{}", entry.directory, entry.filename);
            let asset = CpkAsset {
                internal_path: path.clone(),
                cpk_basename: basename.clone(),
            };
            if est_chemin_prioritaire(&path) {
                prioritaires.push(asset);
            } else {
                generaux.push(asset);
            }
        }
    }

    info!(
        "scan terminé : {cpk_scanned} CPK parcourus, {} .g4tx trouvés",
        prioritaires.len() + generaux.len()
    );

    // Tri déterministe pour reproductibilité
    prioritaires.sort_by(|a, b| a.internal_path.cmp(&b.internal_path));
    generaux.sort_by(|a, b| a.internal_path.cmp(&b.internal_path));

    let mut tous = prioritaires;
    tous.extend(generaux);
    Ok(tous)
}

/// Lit les premiers `max_bytes` octets d'un fichier (ou tout le fichier si plus court).
fn lire_premiers_octets(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut buf = vec![0u8; max_bytes];
    let lus = f
        .read(&mut buf)
        .with_context(|| format!("read {}", path.display()))?;
    buf.truncate(lus);
    Ok(buf)
}

/// Extrait les octets bruts d'un `.g4tx` depuis son CPK (lecture complète du CPK).
fn extraire_g4tx_depuis_cpk(game_dir: &Path, asset: &CpkAsset) -> Result<Vec<u8>> {
    use nie_formats::cpk::CpkReader;

    let cpk_path = game_dir
        .join("data")
        .join("packs")
        .join(&asset.cpk_basename);

    info!("lecture CPK complet : {}", asset.cpk_basename);
    let donnees = std::fs::read(&cpk_path)
        .with_context(|| format!("lecture CPK : {}", cpk_path.display()))?;

    let reader = CpkReader::new(&donnees, &asset.cpk_basename)
        .with_context(|| format!("parsing CPK : {}", asset.cpk_basename))?;

    // Trouver l'entrée par chemin interne
    let full_path = |e: &nie_formats::cpk::CpkEntry| format!("{}/{}", e.directory, e.filename);

    let entry = reader
        .entries
        .iter()
        .find(|e| full_path(e) == asset.internal_path)
        .or_else(|| {
            reader
                .entries
                .iter()
                .find(|e| full_path(e).eq_ignore_ascii_case(&asset.internal_path))
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} non trouvé dans {}",
                asset.internal_path,
                asset.cpk_basename
            )
        })?;

    reader
        .extract(&donnees, entry)
        .with_context(|| format!("extraction {} depuis CPK", asset.internal_path))
}

// ── Chargement texture (VFS ou CPK scan) ────────────────────────────────────

/// Charge et décode une texture `.g4tx` en RGBA8.
/// Tente le VFS en premier ; si échoué (bug cfgbin ou format incompatible),
/// utilise le scan CPK direct.
/// Retourne `(chemin_vfs, largeur, hauteur, données_rgba8)`.
fn charger_texture(
    game_dir: &Path,
    g4tx_path: Option<&str>,
) -> Result<(String, u32, u32, Vec<u8>)> {
    let data_dir = game_dir.join("data");

    // ── Tentative VFS ────────────────────────────────────────────────────────
    if let Some(vfs) = tenter_vfs(&data_dir) {
        let chemin = if let Some(p) = g4tx_path {
            p.to_string()
        } else {
            auto_choisir_g4tx_vfs(&vfs).ok_or_else(|| anyhow::anyhow!("aucun .g4tx dans le VFS"))?
        };

        info!("lecture VFS : {chemin}");
        let donnees = vfs
            .read(&chemin)
            .with_context(|| format!("lecture VFS : {chemin}"))?;
        return decoder_g4tx_bytes(&donnees, &chemin);
    }

    // ── Repli scan CPK ───────────────────────────────────────────────────────
    let assets = scanner_cpks_g4tx(game_dir)?;

    let asset = if let Some(p) = g4tx_path {
        // Cherche par chemin exact ou sous-chemin
        assets
            .iter()
            .find(|a| a.internal_path == p || a.internal_path.ends_with(p))
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "chemin '{p}' non trouvé dans les CPK ({} assets)",
                    assets.len()
                )
            })?
    } else {
        assets
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("aucun .g4tx trouvé dans les CPK"))?
    };

    info!(
        "g4tx choisi : {} (CPK : {})",
        asset.internal_path, asset.cpk_basename
    );
    let donnees = extraire_g4tx_depuis_cpk(game_dir, &asset)?;
    decoder_g4tx_bytes(&donnees, &asset.internal_path)
}

/// Parse un buffer G4TX et décode la texture la plus grande en RGBA8.
fn decoder_g4tx_bytes(donnees: &[u8], chemin: &str) -> Result<(String, u32, u32, Vec<u8>)> {
    let parsed = g4tx::parse(donnees).with_context(|| format!("parsing G4TX : {chemin}"))?;

    let tex = parsed
        .textures
        .iter()
        .filter(|t| t.is_dds)
        .max_by_key(|t| (t.width as u64) * (t.height as u64))
        .ok_or_else(|| anyhow::anyhow!("aucune texture DDS dans {chemin}"))?;

    info!(
        "texture : \"{}\" id={} {}x{} DDS",
        tex.name, tex.id, tex.width, tex.height
    );

    let (w, h, rgba) = g4tx_decode::decode_texture_rgba(donnees, tex)
        .ok_or_else(|| anyhow::anyhow!("échec décodage DDS dans {chemin}"))?;

    Ok((chemin.to_string(), w, h, rgba))
}

/// Récupère les octets bruts d'un `.g4tx` par chemin logique (basename ou chemin VFS).
/// VFS d'abord (résolution locale via [`resolve_vfs_basename`]), repli sur le scan CPK direct
/// (`internal_path == p` ou `ends_with(p)`). Retourne `(chemin résolu, octets)`.
fn obtenir_g4tx_bytes(game_dir: &Path, path: &str) -> Result<(String, Vec<u8>)> {
    let data_dir = game_dir.join("data");
    if let Some(vfs) = tenter_vfs(&data_dir) {
        // Chemin VFS exact, sinon résolution par basename (gère le placeholder de locale `<LG>`).
        if let Ok(d) = vfs.read(path) {
            return Ok((path.to_string(), d));
        }
        if let Some(resolved) = resolve_vfs_basename(&vfs, path, MENU_LOCALE)
            && let Ok(d) = vfs.read(&resolved)
        {
            return Ok((resolved, d));
        }
    }
    let assets = scanner_cpks_g4tx(game_dir)?;
    let asset = assets
        .iter()
        .find(|a| a.internal_path == path || a.internal_path.ends_with(path))
        .cloned()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "chemin '{path}' non trouvé (VFS + {} CPK assets)",
                assets.len()
            )
        })?;
    let donnees = extraire_g4tx_depuis_cpk(game_dir, &asset)?;
    Ok((asset.internal_path, donnees))
}

/// Diagnostic render-from-runtime : liste les régions d'atlas (sous-textures) d'un `.g4tx`.
///
/// Prouve que la cible d'un `SetIconSprite(obj, CRC32(chemin), CRC32(region))` résolu au runtime
/// (ex. `icon_rarity.g4tx` + `gtxt_rarity01_05`) correspond à une vraie sous-texture `(x,y,w,h)`.
fn cmd_g4tx_regions(game_dir: &Path, g4tx_path: Option<&str>) -> Result<()> {
    let path = g4tx_path
        .ok_or_else(|| anyhow::anyhow!("--g4tx-regions requiert --g4tx <chemin|basename>"))?;
    let (resolved, bytes) = obtenir_g4tx_bytes(game_dir, path)?;
    let parsed = g4tx::parse(&bytes).with_context(|| format!("parsing G4TX : {resolved}"))?;

    println!(
        "=== {resolved} : {} texture(s), {} régions totales (header) ===",
        parsed.textures.len(),
        parsed.header.sub_texture_count
    );
    let mut n = 0usize;
    for t in &parsed.textures {
        if t.sub_textures.is_empty() {
            continue;
        }
        println!(
            "texture \"{}\" id={} {}x{} dds={} — {} régions :",
            t.name,
            t.id,
            t.width,
            t.height,
            t.is_dds,
            t.sub_textures.len()
        );
        for s in &t.sub_textures {
            println!(
                "  {:<30} x={:>5} y={:>5}  {:>4}x{:<4}",
                s.name, s.x, s.y, s.width, s.height
            );
            n += 1;
        }
    }
    println!("total: {n} régions d'atlas nommées");
    Ok(())
}

/// Render-from-runtime — rogne la **région d'atlas nommée** d'un g4tx en ses pixels réels.
///
/// Étape finale « commande runtime → pixels » : résout `region` → `(texture porteuse, rect)` via
/// [`g4tx::G4tx::region`], décode la texture DDS, rogne le rect. Avec `capture`, écrit le PNG.
fn cmd_g4tx_region(
    game_dir: &Path,
    g4tx_path: Option<&str>,
    region: &str,
    capture: Option<&Path>,
) -> Result<()> {
    let path = g4tx_path
        .ok_or_else(|| anyhow::anyhow!("--g4tx-region requiert --g4tx <chemin|basename>"))?;
    let (resolved, bytes) = obtenir_g4tx_bytes(game_dir, path)?;
    let parsed = g4tx::parse(&bytes).with_context(|| format!("parsing G4TX : {resolved}"))?;

    let (tex, sub) = parsed
        .region(region)
        .ok_or_else(|| anyhow::anyhow!("région '{region}' absente de {resolved}"))?;
    let rect = (sub.x, sub.y, sub.width, sub.height);
    let (rx, ry) = (sub.x, sub.y);

    let (fw, fh, full) = g4tx_decode::decode_texture_rgba(&bytes, tex).ok_or_else(|| {
        anyhow::anyhow!("échec décodage de la texture '{}' de {resolved}", tex.name)
    })?;
    let tex_name = tex.name.clone();
    let (cw, ch, crop) = crop_rgba(&full, fw, fh, rect)
        .ok_or_else(|| anyhow::anyhow!("rect {rect:?} hors de la texture {fw}x{fh}"))?;

    // Compte de pixels non transparents : preuve grossière que le crop n'est pas vide.
    let opaques = crop.chunks_exact(4).filter(|p| p[3] != 0).count();
    println!(
        "région '{region}' → texture '{tex_name}' {fw}x{fh} DDS → crop {cw}x{ch} @ ({rx},{ry}) ; \
         {opaques}/{} px non transparents",
        cw as usize * ch as usize
    );

    if let Some(out) = capture {
        let png = encoder_rgba_png(&crop, cw, ch)?;
        std::fs::write(out, &png).with_context(|| format!("écriture {}", out.display()))?;
        println!("PNG écrit : {} ({} octets)", out.display(), png.len());
    }
    Ok(())
}

/// Diagnostic **C4 / D1.d (rendu de texte)** : prouve le pipeline de texte de menu sur les VRAIES
/// données. Décode l'atlas de police bitmap pré-cuit (`font_def/font.g4tx`) + les métriques de
/// glyphes (`font_def/font.cfg.bin`, T2B), puis blit `text` via `font::draw_text` (atlas → glyphes).
/// Avec `--capture`, écrit le PNG. C'est le premier pas C4 du rendu de texte (cf. gate D1.d).
fn cmd_render_text(game_dir: &Path, text: &str, capture: Option<&Path>) -> Result<()> {
    const ATLAS: &str = "data/dx11/font/font_def/font.g4tx";
    const METRICS: &str = "data/common/font/font/font_def/font.cfg.bin";

    // 1. Atlas de police : g4tx → texture principale → RGBA8.
    let (atlas_path, atlas_bytes) = obtenir_g4tx_bytes(game_dir, ATLAS)?;
    let parsed =
        g4tx::parse(&atlas_bytes).with_context(|| format!("parsing G4TX police : {atlas_path}"))?;
    let tex = g4tx::select_main_texture(&parsed, "font_def").ok_or_else(|| {
        anyhow::anyhow!("texture principale de l'atlas police introuvable dans {atlas_path}")
    })?;
    let (aw, ah, atlas) = g4tx_decode::decode_texture_rgba(&atlas_bytes, tex)
        .ok_or_else(|| anyhow::anyhow!("échec décodage de l'atlas police '{}'", tex.name))?;

    // 2. Métriques de glyphes : font.cfg.bin (T2B) → FontMetrics.
    let (m_path, m_bytes) = obtenir_g4tx_bytes(game_dir, METRICS)?;
    let cfg = cfgbin::parse_t2b(&m_bytes)
        .with_context(|| format!("parse_t2b métriques police : {m_path}"))?;
    let metrics = font::parse_metrics(&cfg);

    // 3. Canvas + draw_text (sommet de cellule = pen_y − ascent ⇒ pen_y = ascent, dst_y = 0).
    let ch = u32::from(metrics.dims.cell_height).max(1);
    let n_cp = text.chars().count() as u32;
    let cw = (n_cp.max(1) * ch).max(ch); // borne large : avance ≤ n_cp × cell_height
    let stride = cw * 4;
    let mut canvas = alloc_canvas(cw, ch);
    let advance = font::draw_text(
        &atlas,
        aw,
        &metrics,
        text,
        &mut canvas,
        stride,
        0,
        i32::from(metrics.dims.ascent),
        [255, 255, 255, 255],
    );
    let opaques = canvas.chunks_exact(4).filter(|p| p[3] != 0).count();
    let resolved = text
        .chars()
        .filter(|c| metrics.glyph(*c as u32).is_some())
        .count();
    println!(
        "rendu texte {text:?} : atlas '{}' {aw}x{ah} ({} glyphes en table) ; {resolved}/{} codepoints \
         résolus ; avance {advance}px ; {opaques} px opaques",
        tex.name,
        metrics.glyph_count(),
        n_cp
    );

    if let Some(out) = capture {
        let w = (advance.max(1) as u32).min(cw);
        let (ow, oh, rgba) = crop_rgba(
            &canvas,
            cw,
            ch,
            (
                0,
                0,
                w.min(i16::MAX as u32) as i16,
                ch.min(i16::MAX as u32) as i16,
            ),
        )
        .unwrap_or((cw, ch, canvas));
        let png = encoder_rgba_png(&rgba, ow, oh)?;
        std::fs::write(out, &png).with_context(|| format!("écriture {}", out.display()))?;
        println!("PNG écrit : {} ({} octets)", out.display(), png.len());
    }
    Ok(())
}

/// Alloue un canevas RGBA8 transparent `w × h`.
fn alloc_canvas(w: u32, h: u32) -> Vec<u8> {
    vec![0u8; (w as usize) * (h as usize) * 4]
}

/// Charge la police de menu : atlas `font_def/font.g4tx` (RGBA8, legacy BGRA8) + métriques
/// `font_def/font.cfg.bin` (T2B). Renvoie `(atlas_rgba, atlas_w, metrics)` ou `None` si absent.
fn load_menu_font(game_dir: &Path) -> Option<(Vec<u8>, u32, font::FontMetrics)> {
    const ATLAS: &str = "data/dx11/font/font_def/font.g4tx";
    const METRICS: &str = "data/common/font/font/font_def/font.cfg.bin";
    let (_, ab) = obtenir_g4tx_bytes(game_dir, ATLAS).ok()?;
    let parsed = g4tx::parse(&ab).ok()?;
    let tex = g4tx::select_main_texture(&parsed, "font_def")?;
    let (aw, _ah, atlas) = g4tx_decode::decode_texture_rgba(&ab, tex)?;
    let (_, mb) = obtenir_g4tx_bytes(game_dir, METRICS).ok()?;
    let cfg = cfgbin::parse_t2b(&mb).ok()?;
    Some((atlas, aw, font::parse_metrics(&cfg)))
}

/// Extrait le libellé texte RÉSOLU d'un objet de layout. Le champ `text` est hétérogène : un hash
/// `"0x…"` non résolu ou un nombre → `None` (rien à rendre) ; un tableau `[{slot, text}]` (forme
/// résolue par le résolveur de texte universel) → concatène les `text` non vides.
fn resolved_text_label(text_val: &serde_json::Value) -> Option<String> {
    let arr = text_val.as_array()?;
    let parts: Vec<&str> = arr
        .iter()
        .filter_map(|e| e.get("text").and_then(serde_json::Value::as_str))
        .filter(|s| !s.is_empty() && !s.starts_with("0x"))
        .collect();
    if parts.is_empty() {
        return None;
    }
    Some(parts.join(" "))
}

/// Chemin du fichier listant les atlas d'icônes de menu (un chemin VFS logique par ligne, `#`=commentaire).
const MENU_ICON_ATLASES: &str = "data/re/menu-icon-atlases.txt";

/// Construit l'index `nom-de-région → chemin g4tx` (render-from-runtime).
///
/// Charge chaque atlas listé dans [`MENU_ICON_ATLASES`], parse ses régions d'atlas, et associe
/// CHAQUE nom de région au chemin g4tx logique qui la contient. C'est le chaînon manquant : le
/// runtime fournit un `spriteRegion` (ex. `gtxt_rarity01_05`) mais pas toujours le chemin g4tx ;
/// cet index le résout (→ `icon_rarity.g4tx` → `region_rect` → pixels).
fn cmd_build_region_index(game_dir: &Path, out: &Path) -> Result<()> {
    let manifest = game_dir.join(MENU_ICON_ATLASES);
    // Repli : chemin relatif au CWD si le manifeste n'est pas sous game_dir (data symlinks cassés).
    let liste = std::fs::read_to_string(&manifest)
        .or_else(|_| std::fs::read_to_string(MENU_ICON_ATLASES))
        .with_context(|| format!("lecture du manifeste d'atlas {}", manifest.display()))?;
    // NB : les chemins VFS commencent par `#` (`#/menu/...`) — on ne peut PAS filtrer les commentaires
    // par `#`. Un atlas valide est une ligne contenant `.g4tx` ; tout le reste (en-tête) est ignoré.
    let paths: Vec<&str> = liste
        .lines()
        .map(str::trim)
        .filter(|l| l.contains(".g4tx"))
        .collect();

    let mut index: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut collisions = 0usize;
    let (mut ok, mut ko) = (0usize, 0usize);
    for logical in &paths {
        match obtenir_g4tx_bytes(game_dir, logical) {
            Ok((_, bytes)) => match g4tx::parse(&bytes) {
                Ok(parsed) => {
                    ok += 1;
                    for t in &parsed.textures {
                        for s in &t.sub_textures {
                            if s.name.is_empty() {
                                continue;
                            }
                            // 1ʳᵉ occurrence gagne (les atlas sont disjoints en pratique ; on log les collisions).
                            if let Some(prev) = index.get(&s.name) {
                                if prev != logical {
                                    collisions += 1;
                                }
                            } else {
                                index.insert(s.name.clone(), (*logical).to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    ko += 1;
                    warn!("parse g4tx '{logical}' : {e}");
                }
            },
            Err(e) => {
                ko += 1;
                warn!("chargement '{logical}' : {e}");
            }
        }
    }

    let json = serde_json::to_string_pretty(&index).context("sérialisation de l'index")?;
    std::fs::write(out, &json).with_context(|| format!("écriture {}", out.display()))?;
    println!(
        "index region->g4tx : {} régions depuis {ok}/{} atlas ({ko} échecs, {collisions} collisions) -> {}",
        index.len(),
        paths.len(),
        out.display()
    );
    Ok(())
}

/// Redimensionne un buffer RGBA8 `sw×sh` en `dw×dh` par échantillonnage au plus proche voisin.
/// Suffisant pour les sprites de menu posés à l'échelle du `transform` (pas de filtrage : on reste
/// pixel-exact sur les zones non redimensionnées, et déterministe).
/// Composite `src` (RGBA8 `sw×sh`) sur `canvas` (`cw×ch`) au coin `(dx,dy)`, **straight alpha-over**
/// (`out = src·a + dst·(1−a)`). Clippe aux bords ; `(dx,dy)` peuvent être négatifs.
fn blit_over(
    canvas: &mut [u8],
    (cw, ch): (u32, u32),
    src: &[u8],
    (sw, sh): (u32, u32),
    (dx, dy): (i32, i32),
) {
    for sy in 0..sh as i32 {
        let cy = dy + sy;
        if cy < 0 || cy >= ch as i32 {
            continue;
        }
        for sx in 0..sw as i32 {
            let cx = dx + sx;
            if cx < 0 || cx >= cw as i32 {
                continue;
            }
            let s = (sy as usize * sw as usize + sx as usize) * 4;
            let a = u32::from(src[s + 3]);
            if a == 0 {
                continue;
            }
            let d = (cy as usize * cw as usize + cx as usize) * 4;
            for k in 0..3 {
                let sc = u32::from(src[s + k]);
                let dc = u32::from(canvas[d + k]);
                canvas[d + k] = ((sc * a + dc * (255 - a) + 127) / 255) as u8;
            }
            let da = u32::from(canvas[d + 3]);
            canvas[d + 3] = (a + da * (255 - a) / 255).min(255) as u8;
        }
    }
}

/// Render-from-runtime FINAL : compose un layout JSON (`--runtime --export-layout`) en image.
///
/// Pour chaque objet VISIBLE portant `runtime.spriteRegionG4tx` + `runtime.spriteRegion` + un
/// `transform`, charge le g4tx, décode la texture porteuse, rogne la région (`region_rect`),
/// l'échelonne (`scaleX/Y` du transform) et la pose au `transform` (ancre `anchorX/Y`) sur un
/// canevas 1280×720, en alpha-over. C'est l'aboutissement « commande runtime → image composée ».
fn cmd_compose_layout(game_dir: &Path, json_in: &[PathBuf], png_out: &Path) -> Result<()> {
    const W: u32 = 1280;
    const H: u32 = 720;
    // Les calques sont concaténés dans l'ordre donné : le tri final se fait sur (priorité, rang),
    // donc à priorité égale un calque déclaré plus tard passe au-dessus — c'est l'empilement du jeu.
    let mut objs: Vec<serde_json::Value> = Vec::new();
    for chemin in json_in {
        let txt = std::fs::read_to_string(chemin)
            .with_context(|| format!("lecture du layout {}", chemin.display()))?;
        let doc: serde_json::Value = serde_json::from_str(&txt)
            .with_context(|| format!("layout JSON invalide : {}", chemin.display()))?;
        objs.extend(doc["objects"].as_array().cloned().unwrap_or_default());
    }

    // Cache (octets, parse) par chemin g4tx logique.
    let mut cache: std::collections::HashMap<String, Option<(Vec<u8>, g4tx::G4tx)>> =
        std::collections::HashMap::new();

    // Un élément à dessiner : pixels RGBA **natifs** + transform écran + priorité (z-order).
    //
    // Les pixels ne sont plus pré-agrandis ici : l'échelle est portée par le transform et
    // appliquée par le compositeur de référence (`nie_formats::menu`), qui échantillonne en
    // bilinéaire et sait tourner un sprite. Pré-agrandir au plus proche voisin, comme le faisait
    // cette voie, jetait de l'information avant la composition et ignorait la rotation.
    struct DrawItem {
        prio: i64,
        order: usize,
        rgba: Vec<u8>,
        w: u32,
        h: u32,
        transform: menu::ScreenTransform,
        anchor_x: f32,
        anchor_y: f32,
        /// Mode de dessin lu dans l'objbin : 1 = additif.
        draw_type: i64,
    }
    let mut items: Vec<DrawItem> = Vec::new();
    let (mut n_region, mut n_static) = (0usize, 0usize);
    let mut n_region_hash = 0usize;

    for (order, o) in objs.iter().enumerate() {
        if !o["visible"].as_bool().unwrap_or(false) {
            continue;
        }
        // Charge la SOURCE de pixels : région d'atlas (runtime) en priorité, sinon texture statique.
        let rt = &o["runtime"];
        let region_src = (|| {
            let (g4tx_path, region) = (
                rt["spriteRegionG4tx"].as_str()?,
                rt["spriteRegion"].as_str()?,
            );
            let entry = cache.entry(g4tx_path.to_string()).or_insert_with(|| {
                obtenir_g4tx_bytes(game_dir, g4tx_path)
                    .ok()
                    .and_then(|(_, b)| g4tx::parse(&b).ok().map(|p| (b, p)))
            });
            let (bytes, parsed) = entry.as_ref()?;
            // Un nom d'icône désigne soit une TEXTURE entière du conteneur, soit une région dans
            // une porteuse (`avatar01_13.g4tx` a les deux). Ne chercher que les sous-textures
            // rendait l'atlas complet à la place de la moitié des icônes.
            match parsed.named(region)? {
                g4tx::NamedTarget::Texture(tex) => g4tx_decode::decode_texture_rgba(bytes, tex),
                g4tx::NamedTarget::Region { texture, sub } => {
                    let (fw, fh, full) = g4tx_decode::decode_texture_rgba(bytes, texture)?;
                    crop_rgba(&full, fw, fh, (sub.x, sub.y, sub.width, sub.height))
                }
            }
        })();
        // Résolution de région par HASH (D1.b) : si l'objet porte un `spriteRegionHash` (= CRC32 du
        // nom de région) non résolu en nom, on croppe la sous-région de SON atlas dont
        // `CRC32(nom) == hash` — au lieu de rendre l'atlas entier (toutes ses sous-textures empilées).
        // Même mécanisme CRC32 que partout (cf. `nie-core::ecs`, `cfgbin::crc32`).
        let region_hash_src = (|| {
            let srh = rt["spriteRegionHash"].as_u64().or_else(|| {
                rt["spriteRegionHash"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("0x"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .map(u64::from)
            })? as u32;
            if srh == 0 {
                return None;
            }
            let logical = o["sprite"]["logicalPath"].as_str()?;
            let basename = logical.rsplit('/').next()?;
            let entry = cache.entry(logical.to_string()).or_insert_with(|| {
                obtenir_g4tx_bytes(game_dir, basename)
                    .ok()
                    .and_then(|(_, b)| g4tx::parse(&b).ok().map(|p| (b, p)))
            });
            let (bytes, parsed) = entry.as_ref()?;
            // Même angle mort que `region_src` : le hash peut être celui d'une TEXTURE du
            // conteneur, pas seulement d'une sous-texture. Les textures d'abord, comme
            // `g4tx::find_named` et `g4tx_decode::decode_named_to_rgba`.
            for t in &parsed.textures {
                if cfgbin::crc32(t.name.as_bytes()) == srh {
                    return g4tx_decode::decode_texture_rgba(bytes, t);
                }
            }
            for t in &parsed.textures {
                for s in &t.sub_textures {
                    if cfgbin::crc32(s.name.as_bytes()) == srh {
                        let (fw, fh, full) = g4tx_decode::decode_texture_rgba(bytes, t)?;
                        return crop_rgba(&full, fw, fh, (s.x, s.y, s.width, s.height));
                    }
                }
            }
            None
        })();
        let mut static_src = || {
            let logical = o["sprite"]["logicalPath"].as_str()?;
            let basename = logical.rsplit('/').next()?;
            let stem = basename.strip_suffix(".g4tx").unwrap_or(basename);
            let entry = cache.entry(logical.to_string()).or_insert_with(|| {
                obtenir_g4tx_bytes(game_dir, basename)
                    .ok()
                    .and_then(|(_, b)| g4tx::parse(&b).ok().map(|p| (b, p)))
            });
            let (bytes, parsed) = entry.as_ref()?;
            let tex = g4tx::select_main_texture(parsed, stem)?;
            g4tx_decode::decode_texture_rgba(bytes, tex)
        };
        let via_hash = region_src.is_none() && region_hash_src.is_some();
        let (is_region, (cw, chh, crop)) = match region_src.or(region_hash_src) {
            Some(src) => (true, src),
            None => match static_src() {
                Some(src) => (false, src),
                None => continue,
            },
        };
        if via_hash {
            n_region_hash += 1;
        }

        // Transform : échelle + ancre. Défauts neutres si absents.
        let tr = &o["transform"];
        let f = |k: &str, def: f64| tr[k].as_f64().unwrap_or(def);
        let (sx, sy) = (f("scaleX", 1.0).max(0.0), f("scaleY", 1.0).max(0.0));
        let (ax, ay) = (f("anchorX", 0.5), f("anchorY", 0.5));
        let (px, py) = (f("x", 0.0), f("y", 0.0));
        if crop.is_empty() || cw == 0 || chh == 0 {
            continue;
        }
        if is_region {
            n_region += 1;
        } else {
            n_static += 1;
        }
        items.push(DrawItem {
            prio: o["drawPriority"].as_i64().unwrap_or(0),
            order,
            rgba: crop,
            w: cw,
            h: chh,
            transform: menu::ScreenTransform {
                x_px: px as f32,
                y_px: py as f32,
                scale_x: sx as f32,
                scale_y: sy as f32,
                rot: f("rot", 0.0) as f32,
            },
            anchor_x: ax as f32,
            anchor_y: ay as f32,
            draw_type: o["drawType"].as_i64().unwrap_or(0),
        });
    }

    // ── Passe TEXTE (D1.d) : pose les libellés RÉSOLUS au transform de l'objet, au-dessus des
    // sprites. Police chargée à la demande (atlas 44 Mo). Les positions viennent du driver — les
    // libellés hors canevas (y>720) sont clippés par `blit_over` (limite de placement connue D1.c).
    let mut font: Option<(Vec<u8>, u32, font::FontMetrics)> = None;
    let mut font_tried = false;
    let mut n_text = 0usize;
    for (order, o) in objs.iter().enumerate() {
        if !o["visible"].as_bool().unwrap_or(false) {
            continue;
        }
        let Some(label) = resolved_text_label(&o["text"]) else {
            continue;
        };
        if !font_tried {
            font = load_menu_font(game_dir);
            font_tried = true;
        }
        let Some((atlas, aw, metrics)) = font.as_ref() else {
            break; // police absente — inutile de réessayer chaque objet
        };
        let ch = u32::from(metrics.dims.cell_height).max(1);
        let cw = ((label.chars().count() as u32).max(1) * ch).max(ch);
        let mut buf = alloc_canvas(cw, ch);
        let adv = font::draw_text(
            atlas,
            *aw,
            metrics,
            &label,
            &mut buf,
            cw * 4,
            0,
            i32::from(metrics.dims.ascent),
            [255, 255, 255, 255],
        );
        if adv <= 0 {
            continue; // aucun glyphe résolu
        }
        // `cw` est une borne haute (une cellule carrée par caractère) ; l'avance rendue est la
        // largeur RÉELLE du libellé. Recadrer dessus est nécessaire pour pouvoir l'ancrer : une
        // boîte surdimensionnée décalerait le texte de la moitié de son excédent.
        let largeur = u32::try_from(adv).unwrap_or(cw).clamp(1, cw);
        let Some((tw, th, texte)) = crop_rgba(&buf, cw, ch, (0, 0, largeur as i16, ch as i16))
        else {
            continue;
        };
        let tr = &o["transform"];
        let f = |k: &str, d: f64| tr[k].as_f64().unwrap_or(d);
        // Même convention d'ancre que les sprites : le transform donne un PIVOT, pas un coin. Le
        // texte se posait jusqu'ici au coin haut-gauche, donc décalé d'une demi-boîte par rapport
        // au widget qu'il étiquette.
        let (ax, ay) = (f("anchorX", 0.5), f("anchorY", 0.5));
        items.push(DrawItem {
            prio: o["drawPriority"].as_i64().unwrap_or(0) + 1000, // texte au-dessus des sprites
            order: order + 1_000_000,
            rgba: texte,
            w: tw,
            h: th,
            // Le libellé est déjà rendu à sa taille : échelle neutre, seule l'ancre le place.
            transform: menu::ScreenTransform {
                x_px: f("x", 0.0) as f32,
                y_px: f("y", 0.0) as f32,
                scale_x: 1.0,
                scale_y: 1.0,
                rot: 0.0,
            },
            anchor_x: ax as f32,
            anchor_y: ay as f32,
            draw_type: 0,
        });
        n_text += 1;
    }

    // Z-order : priorité de dessin croissante (fond d'abord), départage par ordre de déclaration.
    items.sort_by(|a, b| a.prio.cmp(&b.prio).then(a.order.cmp(&b.order)));
    let sprites: Vec<menu::CompositeSprite> = items
        .iter()
        .map(|it| menu::CompositeSprite {
            rgba: &it.rgba,
            width: it.w,
            height: it.h,
            transform: it.transform,
            anchor_x: it.anchor_x,
            anchor_y: it.anchor_y,
            couleur: [1.0; 4],
            // `drawType` vient du composant de rendu de l'objbin. 1 = additif (halos, néons) :
            // les mélanger en « over » les éteint. Les autres valeurs — dont 4, dont la
            // sémantique n'est pas établie — restent en mélange normal plutôt qu'inventées.
            mode: if it.draw_type == 1 {
                menu::BlendMode::Additif
            } else {
                menu::BlendMode::Normal
            },
        })
        .collect();
    let canvas = menu::compose(W, H, &sprites);
    let n_drawn = items.len();

    let png = encoder_rgba_png(&canvas, W, H)?;
    std::fs::write(png_out, &png).with_context(|| format!("écriture {}", png_out.display()))?;
    let opaques = canvas.chunks_exact(4).filter(|p| p[3] != 0).count();
    println!(
        "compose-layout : {n_drawn} éléments ({n_static} sprites statiques + {n_region} régions runtime \
         + {n_region_hash} régions par hash + {n_text} libellés texte) sur {}×{} ; {opaques} px opaques -> {} ({} octets)",
        W,
        H,
        png_out.display(),
        png.len()
    );
    Ok(())
}

/// Sélectionne automatiquement un chemin `.g4tx` prioritaire depuis le VFS.
fn auto_choisir_g4tx_vfs(vfs: &Vfs) -> Option<String> {
    let mut prioritaires: Vec<String> = Vec::new();
    let mut generaux: Vec<String> = Vec::new();

    for (path, _) in vfs.iter() {
        if !path.to_ascii_lowercase().ends_with(".g4tx") {
            continue;
        }
        if est_chemin_prioritaire(path) {
            prioritaires.push(path.to_string());
        } else {
            generaux.push(path.to_string());
        }
    }
    prioritaires.sort();
    generaux.sort();
    prioritaires.into_iter().chain(generaux).next()
}

// ── Mode liste ───────────────────────────────────────────────────────────────

/// Liste les N premiers fichiers `.g4tx` (VFS ou CPK scan) avec dimensions.
fn cmd_list(game_dir: &Path, n: usize) -> Result<()> {
    let data_dir = game_dir.join("data");

    // ── Tentative VFS ────────────────────────────────────────────────────────
    if let Some(vfs) = tenter_vfs(&data_dir) {
        let mut prioritaires: Vec<String> = Vec::new();
        let mut generaux: Vec<String> = Vec::new();
        for (path, _) in vfs.iter() {
            if !path.to_ascii_lowercase().ends_with(".g4tx") {
                continue;
            }
            if est_chemin_prioritaire(path) {
                prioritaires.push(path.to_string());
            } else {
                generaux.push(path.to_string());
            }
        }
        prioritaires.sort();
        generaux.sort();
        let chemins: Vec<String> = prioritaires.into_iter().chain(generaux).take(n).collect();

        println!(
            "=== {} fichiers .g4tx (VFS, sur {} demandés) ===",
            chemins.len(),
            n
        );
        for chemin in &chemins {
            match vfs.read(chemin) {
                Ok(d) => match g4tx::parse(&d) {
                    Ok(parsed) => {
                        let best = parsed
                            .textures
                            .iter()
                            .filter(|t| t.is_dds)
                            .max_by_key(|t| (t.width as u64) * (t.height as u64));
                        match best {
                            Some(t) => println!("{chemin}  {}x{}  DDS", t.width, t.height),
                            None => {
                                if let Some(t) = parsed.textures.first() {
                                    println!("{chemin}  {}x{}  non-DDS", t.width, t.height);
                                } else {
                                    println!("{chemin}  (aucune texture)");
                                }
                            }
                        }
                    }
                    Err(e) => println!("{chemin}  (parse erreur: {e})"),
                },
                Err(e) => println!("{chemin}  (lecture erreur: {e})"),
            }
        }
        return Ok(());
    }

    // ── Repli scan CPK ───────────────────────────────────────────────────────
    let assets = scanner_cpks_g4tx(game_dir)?;
    let selection: Vec<_> = assets.into_iter().take(n).collect();

    println!(
        "=== {} fichiers .g4tx (scan CPK direct, sur {} demandés) ===",
        selection.len(),
        n
    );

    for asset in &selection {
        // Extraction pour obtenir les dimensions
        match extraire_g4tx_depuis_cpk(game_dir, asset) {
            Ok(d) => match g4tx::parse(&d) {
                Ok(parsed) => {
                    let best = parsed
                        .textures
                        .iter()
                        .filter(|t| t.is_dds)
                        .max_by_key(|t| (t.width as u64) * (t.height as u64));
                    match best {
                        Some(t) => {
                            println!(
                                "{}  {}x{}  DDS  [{}]",
                                asset.internal_path, t.width, t.height, asset.cpk_basename
                            )
                        }
                        None => println!(
                            "{}  (non-DDS)  [{}]",
                            asset.internal_path, asset.cpk_basename
                        ),
                    }
                }
                Err(e) => println!(
                    "{}  (parse erreur: {e})  [{}]",
                    asset.internal_path, asset.cpk_basename
                ),
            },
            Err(e) => println!(
                "{}  (lecture erreur: {e})  [{}]",
                asset.internal_path, asset.cpk_basename
            ),
        }
    }

    Ok(())
}

// ── Infrastructure wgpu partagée ─────────────────────────────────────────────

/// Demande un adaptateur wgpu hors-écran : matériel d'abord, logiciel en repli.
///
/// Le matériel est demandé en `HighPerformance` — sur un portable à double GPU, c'est ce qui
/// désigne la carte discrète plutôt que l'iGPU. Sur un serveur sans GPU, la première tentative
/// échoue et le rendu logiciel prend le relais ; c'est le chemin normal, pas une dégradation.
fn demander_adaptateur_hors_ecran(instance: &wgpu::Instance) -> Result<wgpu::Adapter> {
    if !gpu_select::fallback_impose() {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: gpu_select::preference_puissance(),
            compatible_surface: None,
            force_fallback_adapter: false,
        }));
        if let Ok(a) = adapter {
            info!("adaptateur retenu : {}", gpu_select::decrire(&a));
            return Ok(a);
        }
        warn!("pas d'adaptateur matériel, tentative du rendu logiciel...");
    }
    let a = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        compatible_surface: None,
        force_fallback_adapter: true,
    }))
    .context("aucun adaptateur wgpu (ni GPU ni logiciel)")?;
    info!("adaptateur retenu : {}", gpu_select::decrire(&a));
    Ok(a)
}

/// Crée un `(Device, Queue)` depuis un adaptateur.
fn creer_device(adapter: &wgpu::Adapter) -> Result<(wgpu::Device, wgpu::Queue)> {
    // Utilise les limites réelles de l'adaptateur pour ne pas artificiellement
    // brider la taille maximale des textures (downlevel_defaults plafonne à 2048,
    // ce qui est insuffisant pour les sprites IEVR qui peuvent dépasser 4096 px).
    let limits = adapter.limits();
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("nie-game"),
        required_features: wgpu::Features::empty(),
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
    }))
    .context("création device wgpu")
}

/// Crée le bind group layout (texture 2D + sampler).
fn creer_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("texture_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Crée le pipeline de rendu plein écran pour le format de sortie donné.
fn creer_pipeline(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fullscreen"),
        source: wgpu::ShaderSource::Wgsl(include_str!("fullscreen.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pipeline_layout"),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fullscreen_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

/// Charge les données RGBA8 dans une texture GPU et retourne texture + vue + sampler.
fn charger_gpu_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("g4tx_source"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        texture.as_image_copy(),
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        extent,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("nearest_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    (texture, view, sampler)
}

/// Crée le bind group (lie texture + sampler au bind group layout).
fn creer_bind_group(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("texture_bind_group"),
        layout: bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

// ── Mode capture (hors-écran) ────────────────────────────────────────────────

/// Rendu hors-écran → PNG. Format `Rgba8Unorm`, échantillonneur `Nearest`, sans sRGB.
fn cmd_capture(rgba: &[u8], width: u32, height: u32, png_out: &Path) -> Result<()> {
    info!("mode capture hors-écran → {}", png_out.display());

    let instance = gpu_select::instance();

    let adapter = demander_adaptateur_hors_ecran(&instance)?;
    info!("adaptateur : {:?}", adapter.get_info());

    let (device, queue) = creer_device(&adapter)?;

    let bgl = creer_bgl(&device);
    let pipeline = creer_pipeline(&device, &bgl, wgpu::TextureFormat::Rgba8Unorm);
    let (_, view, sampler) = charger_gpu_texture(&device, &queue, rgba, width, height);
    let bind_group = creer_bind_group(&device, &bgl, &view, &sampler);

    // Render target hors-écran
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let render_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("render_target"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let render_view = render_tex.create_view(&wgpu::TextureViewDescriptor::default());

    // Rendu
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("capture_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("capture_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &render_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    // Copie render target → buffer de lecture
    const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let bytes_per_row_padded = (4 * width).div_ceil(ALIGN) * ALIGN;
    let buf_size = (bytes_per_row_padded * height) as u64;

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: buf_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        render_tex.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row_padded),
                rows_per_image: Some(height),
            },
        },
        extent,
    );

    queue.submit([encoder.finish()]);

    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

    let mapped = readback.slice(..).get_mapped_range();

    // Suppression du padding ligne par ligne
    let unpadded_bpr = (4 * width) as usize;
    let padded_bpr = bytes_per_row_padded as usize;
    let mut pixels: Vec<u8> = Vec::with_capacity(unpadded_bpr * height as usize);
    for row in 0..height as usize {
        pixels.extend_from_slice(&mapped[row * padded_bpr..row * padded_bpr + unpadded_bpr]);
    }
    drop(mapped);
    readback.unmap();

    // Encodage PNG
    let png_bytes = encoder_rgba_png(&pixels, width, height)?;

    std::fs::write(png_out, &png_bytes)
        .with_context(|| format!("écriture PNG : {}", png_out.display()))?;

    info!(
        "capture écrite : {}  {}x{}  {} octets",
        png_out.display(),
        width,
        height,
        png_bytes.len()
    );
    println!(
        "capture: {}  {}x{}  {} octets PNG",
        png_out.display(),
        width,
        height,
        png_bytes.len()
    );

    Ok(())
}

/// Encode un buffer RGBA8 brut en PNG.
fn encoder_rgba_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    {
        let mut enc = png::Encoder::new(std::io::Cursor::new(&mut out), width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().context("en-tête PNG")?;
        wr.write_image_data(rgba).context("données PNG")?;
    }
    Ok(out)
}

// ── Mode fenêtré (winit 0.30 + wgpu 22) ─────────────────────────────────────

/// Ouvre une fenêtre winit et affiche la texture via wgpu.
/// Chemins logiques de la police du jeu — ceux que `nie.exe` charge lui-même.
const FONT_CFG: &str = "data/common/font/font/font_def/font.cfg.bin";
/// Atlas correspondant à [`FONT_CFG`], côté ressources rendues.
const FONT_G4TX: &str = "data/dx11/font/font_def/font.g4tx";

/// Ouvre le jeu et le rend jouable au clavier.
///
/// Rien de la logique n'est ici : la FSM ([`nie_app::flow::Screen`]) et la physique
/// ([`nie_runtime::World`]) existaient déjà et tournaient dans le navigateur via `nie-wasm`. Ce
/// qui manquait, c'était le front natif — la boucle qui lit le clavier, avance le temps et
/// téléverse le framebuffer. Le cœur reste partagé : corriger un comportement de menu le corrige
/// pour les deux fronts.
///
/// La police vient du VFS (installation ou dump, indifféremment) : demander deux chemins de
/// fichiers pour lancer un jeu serait absurde.
fn cmd_play(max_frames: u32) -> Result<()> {
    use winit::event_loop::EventLoop;

    let vfs = nie_formats::vfs::open_game()
        .map_err(|e| anyhow::anyhow!("aucune donnée de jeu (ni installation ni dump) : {e:?}"))?;
    info!(
        "VFS monté ({}) : {} fichiers",
        if vfs.is_dump() { "dump" } else { "packs" },
        vfs.asset_count()
    );
    let cfg = vfs
        .read(FONT_CFG)
        .map_err(|e| anyhow::anyhow!("police {FONT_CFG} : {e:?}"))?;
    let atlas = vfs
        .read(FONT_G4TX)
        .map_err(|e| anyhow::anyhow!("atlas {FONT_G4TX} : {e:?}"))?;
    let police = nie_app::Font::from_bytes(&cfg, &atlas).context("chargement de la police")?;

    let ecran = nie_app::flow::Screen::new();
    let premiere = ecran.render(&police);
    let (w, h) = (nie_app::W as u32, nie_app::H as u32);
    info!("jeu prêt — Entrée/Espace : valider, flèches ou ZQSD : naviguer, Échap : retour");

    let event_loop = EventLoop::new().context("création EventLoop winit")?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = AppFenetre {
        instance: gpu_select::instance(),
        rgba: premiere,
        tex_width: w,
        tex_height: h,
        max_frames,
        frames_rendues: 0,
        etat: None,
        erreur: None,
        jeu: Some(Jeu {
            ecran,
            police,
            dernier: std::time::Instant::now(),
            dernier_score: vec![0, 0],
            enfoncees: std::collections::HashSet::new(),
            vfs,
            cache: std::collections::HashMap::new(),
            dialogue: None,
            vue3d: false,
            modele3d: None,
        }),
    };
    event_loop
        .run_app(&mut app)
        .context("boucle événements winit")?;
    if let Some(e) = app.erreur {
        return Err(e);
    }
    info!("session terminée ({} images)", app.frames_rendues);
    Ok(())
}

fn cmd_window(rgba: &[u8], width: u32, height: u32, max_frames: u32) -> Result<()> {
    use winit::event_loop::EventLoop;

    info!(
        "mode fenêtré : {}x{} pendant {} trames",
        width, height, max_frames
    );

    // Même sélection de backend que le chemin hors-écran : D3D12 sur Windows (natif, pilotes
    // NVIDIA/AMD de première classe), Vulkan sur Linux — où GLES/Zink échoue à initialiser une
    // surface Wayland sous WSLg (DRI2/ZINK → SIGSEGV).
    let instance = gpu_select::instance();

    let event_loop = EventLoop::new().context("création EventLoop winit")?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let mut app = AppFenetre {
        instance,
        rgba: rgba.to_vec(),
        tex_width: width,
        tex_height: height,
        max_frames,
        frames_rendues: 0,
        etat: None,
        erreur: None,
        // Visionneuse : image figée, aucune FSM (cf. `cmd_play` pour le mode jouable).
        jeu: None,
    };

    event_loop
        .run_app(&mut app)
        .context("boucle événements winit")?;

    if let Some(e) = app.erreur {
        error!("mode fenêtré terminé avec erreur : {e}");
        return Err(e);
    }

    info!("mode fenêtré terminé ({} trames)", app.frames_rendues);
    Ok(())
}

// ── Struct de l'application winit ────────────────────────────────────────────

/// État de rendu lié à la fenêtre (créé dans `resumed`).
struct EtatFenetre {
    fenetre: Arc<winit::window::Window>,
    /// Instance wgpu conservée pour recréer la surface si elle est perdue (variante
    /// `CurrentSurfaceTexture::Lost` en wgpu 29). `Instance` est Arc-backed (clone bon marché).
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    /// Texture source, conservée pour être RÉÉCRITE à chaque image en mode jouable.
    ///
    /// La visionneuse n'en avait pas besoin — une image figée s'écrit une fois. Un jeu réécrit
    /// son framebuffer 60 fois par seconde, et recréer la texture (donc le groupe de liaison et
    /// la vue) à chaque image gaspillerait une allocation GPU par trame.
    texture: wgpu::Texture,
}

impl EtatFenetre {
    /// Remplace le contenu de la texture affichée par `rgba` (`W`×`H`, RGBA8).
    fn televerser(&self, rgba: &[u8], width: u32, height: u32) {
        self.queue.write_texture(
            self.texture.as_image_copy(),
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
}

impl EtatFenetre {
    fn redimensionner(&mut self, nouvelle_taille: winit::dpi::PhysicalSize<u32>) {
        if nouvelle_taille.width > 0 && nouvelle_taille.height > 0 {
            self.config.width = nouvelle_taille.width;
            self.config.height = nouvelle_taille.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn rendre(&mut self) -> Result<()> {
        // wgpu 29 : `get_current_texture` renvoie l'enum `CurrentSurfaceTexture` (7 variantes),
        // plus de `Result<_, SurfaceError>`.
        use wgpu::CurrentSurfaceTexture as Cst;
        let output = match self.surface.get_current_texture() {
            Cst::Success(frame) | Cst::Suboptimal(frame) => frame,
            Cst::Outdated => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    Cst::Success(f) | Cst::Suboptimal(f) => f,
                    _ => return Ok(()), // sauter la trame
                }
            }
            Cst::Lost => {
                // Surface perdue : la recréer (pas juste reconfigure), re-tenter au prochain redraw.
                self.surface = self.instance.create_surface(self.fenetre.clone())?;
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            // Trame sautée : `about_to_wait` re-demande un redraw.
            Cst::Timeout | Cst::Occluded | Cst::Validation => return Ok(()),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        output.present();
        Ok(())
    }
}

/// Application winit 0.30 (trait `ApplicationHandler`).
struct AppFenetre {
    instance: wgpu::Instance,
    rgba: Vec<u8>,
    tex_width: u32,
    tex_height: u32,
    max_frames: u32,
    frames_rendues: u32,
    etat: Option<EtatFenetre>,
    erreur: Option<anyhow::Error>,
    /// Présent en mode `--play` : la fenêtre cesse d'afficher une image figée et devient le
    /// front-end du jeu. `None` = visionneuse de texture, le comportement d'origine.
    jeu: Option<Jeu>,
}

/// État du mode jouable : la FSM du cœur, la police qui la rend, et l'horloge de la boucle.
///
/// La logique n'est PAS ici — elle est dans [`nie_app::flow::Screen`], que partagent déjà le web
/// (`nie-wasm`) et le rendu headless. Ce front ne fait que traduire : clavier → commande de menu
/// IEVR, temps écoulé → `update`, framebuffer → texture GPU. C'est ce qui manquait pour que le
/// jeu soit jouable en natif, et rien d'autre.
struct Jeu {
    ecran: nie_app::flow::Screen,
    police: nie_app::Font,
    /// Instant de la dernière image, pour un `dt` réel plutôt qu'un pas fixe supposé.
    dernier: std::time::Instant,
    /// Score affiché au dernier changement, pour ne journaliser qu'aux buts.
    dernier_score: Vec<u32>,
    /// Touches actuellement ENFONCÉES, pour le déplacement en match.
    ///
    /// Un menu se pilote par événements — un appui, une action. Un joueur de football se dirige
    /// par état : tant que la touche est tenue, il court. Les deux coexistent donc, et c'est le
    /// même clavier.
    enfoncees: std::collections::HashSet<winit::keyboard::KeyCode>,
    /// VFS monté, gardé pour charger les données des onglets à la demande.
    vfs: nie_formats::vfs::Vfs,
    /// Lignes déjà résolues par onglet (une entrée vide = tentative échouée, non réessayée).
    ///
    /// Chaque jointure lit des tables de plusieurs mégaoctets : la refaire à chaque ouverture de
    /// l'onglet se sentirait à la navigation.
    cache: std::collections::HashMap<Onglet, Vec<String>>,
    /// Vue 3D du match active (touche V).
    vue3d: bool,
    /// Modèle 3D des joueurs (`Some(None)` = chargement tenté et échoué).
    modele3d: Option<Option<nie_render3d::glb::Model>>,
    /// Scène de dialogue retenue pour le mode Histoire (`Some(None)` = recherche infructueuse).
    dialogue: Option<Option<(String, Vec<String>)>>,
}

/// Onglet du menu principal dont ce front sait charger les données réelles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Onglet {
    /// « Composition d'équipe » — personnages, postes, éléments.
    Effectif,
    /// « Objets » — inventaire du jeu, catégories et prix.
    Objets,
    /// Onglet dont le contenu EST une table de texte localisée (aide, fichier de données,
    /// Inacord, options) : le nom du fichier suffit à le décrire.
    Texte(&'static str),
}

impl Jeu {
    /// Lignes à afficher pour l'onglet `titre`, ou vide si le jeu ne sait pas encore le remplir.
    ///
    /// L'effectif est chargé **à la demande** et mémorisé : la jointure `chara_param` ×
    /// `chara_base` × `chara_text` lit trois tables de plusieurs mégaoctets, ce qui se sent si on
    /// la refait à chaque ouverture de l'onglet.
    fn lignes_onglet(&mut self, titre: &str) -> Vec<String> {
        // L'onglet est identifié par son libellé — celui du jeu, pas un index qu'un
        // réordonnancement de `MENU` rendrait silencieusement faux.
        let quoi = if titre == nie_app::MENU[0] {
            Onglet::Effectif
        } else if titre == nie_app::MENU[1] {
            Onglet::Objets
        } else if titre == nie_app::MENU[3] {
            Onglet::Texte("inacode_text")
        } else if titre == nie_app::MENU[4] {
            Onglet::Texte("data_file_text")
        } else if titre == nie_app::MENU[6] {
            Onglet::Texte("help_list_text")
        } else if titre == nie_app::MENU[7] {
            Onglet::Texte("setting_text")
        } else {
            return Vec::new();
        };
        if let Some(deja) = self.cache.get(&quoi) {
            return deja.clone();
        }
        // Un échec ici n'est pas fatal : l'écran d'information reste affiché, et la raison
        // part dans le journal plutôt que sous les yeux de la joueuse.
        let lignes = match quoi {
            Onglet::Effectif => nie_app::effectif::charger(&self.vfs, 200, "fr")
                .map(|v| v.iter().map(nie_app::effectif::Joueur::ligne).collect()),
            Onglet::Objets => nie_app::effectif::charger_objets(&self.vfs, 200, "fr")
                .map(|v| v.iter().map(nie_app::effectif::Objet::ligne).collect()),
            Onglet::Texte(fichier) => {
                nie_app::effectif::charger_textes(&self.vfs, fichier, 200, "fr")
            }
        }
        .unwrap_or_else(|e| {
            warn!("{titre} indisponible : {e:#}");
            Vec::new()
        });
        self.cache.insert(quoi, lignes.clone());
        lignes
    }

    /// Image de l'écran courant, vue 3D comprise.
    ///
    /// La FSM rend le match en vue de dessus ; ce front peut lui substituer la vue en perspective
    /// (`nie_app::match3d`) parce qu'il a le VFS, que la FSM n'a pas. Le bandeau de score est
    /// composé dans les deux cas — c'est l'information, pas la caméra.
    fn image(&mut self) -> Vec<u8> {
        if self.vue3d
            && let Some(world) = self.ecran.world()
        {
            if self.modele3d.is_none() {
                // Le chargement décode une texture BC7 : quelques secondes, une seule fois.
                // L'échec est mémorisé pour ne pas le retenter à chaque image.
                info!("chargement du modèle 3D…");
                match nie_app::match3d::charger_modele_joueur(&self.vfs, 40) {
                    Ok(m) => self.modele3d = Some(Some(m)),
                    Err(e) => {
                        warn!("vue 3D indisponible : {e:#}");
                        self.modele3d = Some(None);
                    }
                }
            }
            if let Some(Some(modele)) = &self.modele3d {
                let px = nie_app::match3d::rendre(world, modele);
                return nie_app::render::hud_match(&px, &self.police, world.score, world.time);
            }
        }
        self.ecran.render(&self.police)
    }

    /// Scène de dialogue à jouer dans le mode Histoire, chargée une fois puis mémorisée.
    ///
    /// Le choix de la scène coûte cher — près de 4 000 fichiers d'événement, dont beaucoup ne
    /// portent que des marqueurs de test japonais — donc on ne le refait pas à chaque ouverture.
    fn dialogue(&mut self) -> Option<(String, Vec<String>)> {
        if self.dialogue.is_none() {
            let choisi =
                nie_app::effectif::premier_dialogue_traduit(&self.vfs, "fr", 5).and_then(|id| {
                    nie_app::effectif::charger_dialogue(&self.vfs, &id, "fr")
                        .ok()
                        .map(|l| (id, l))
                });
            if choisi.is_none() {
                warn!("aucune scène de dialogue traduite trouvée — scène de démonstration gardée");
            }
            // `Some(None)` mémorise l'échec : inutile de rescanner 4 000 fichiers à chaque fois.
            self.dialogue = Some(choisi);
        }
        self.dialogue.clone().flatten()
    }

    /// Direction demandée par les touches tenues, en repère terrain.
    ///
    /// L'axe X est la longueur du terrain (le but adverse est en +x pour l'équipe domicile), l'axe
    /// Y sa largeur. « Haut » à l'écran correspond à −y : le rendu place l'origine en haut.
    fn direction(&self) -> (f32, f32) {
        use winit::keyboard::KeyCode as K;
        let tenue = |touches: &[K]| touches.iter().any(|k| self.enfoncees.contains(k));
        let x = f32::from(u8::from(tenue(&[K::ArrowRight, K::KeyD])))
            - f32::from(u8::from(tenue(&[K::ArrowLeft, K::KeyA, K::KeyQ])));
        let y = f32::from(u8::from(tenue(&[K::ArrowDown, K::KeyS])))
            - f32::from(u8::from(tenue(&[K::ArrowUp, K::KeyW, K::KeyZ])));
        (x, y)
    }
}

/// Nomme l'écran courant pour les traces — la FSM n'expose pas de `Debug` utile.
fn decrire_ecran(e: &nie_app::flow::Screen) -> String {
    use nie_app::flow::Screen as S;
    match e {
        S::Title => "titre".into(),
        S::Menu { sel } => format!("menu[{sel}] {}", nie_app::MENU[*sel]),
        S::ModeSelect { sel } => format!("mode[{sel}] {}", nie_app::MODES[*sel]),
        S::Match { .. } => "match".into(),
        S::Story {
            idx,
            titre,
            repliques,
        } => {
            format!("histoire[{idx}] {titre} ({} repliques)", repliques.len())
        }
        S::Info { title } => format!("info « {title} »"),
        S::Liste { titre, lignes, sel } => format!("liste « {titre} » [{sel}/{}]", lignes.len()),
    }
}

/// Traduit une touche en commande de menu IEVR (`MENU_CMD_INFO` / `input_ctrl`).
///
/// Le mapping vit ici, côté front, comme le veut la FSM : le cœur ne connaît que des commandes.
/// Les flèches ET ZQSD/WASD naviguent — un clavier AZERTY et un QWERTY doivent tous deux marcher
/// sans réglage.
fn touche_vers_commande(code: winit::keyboard::KeyCode) -> Option<&'static str> {
    use winit::keyboard::KeyCode as K;
    Some(match code {
        K::ArrowUp | K::KeyW | K::KeyZ => "CMD_FCS_MTX_UP",
        K::ArrowDown | K::KeyS => "CMD_FCS_MTX_DOWN",
        K::ArrowLeft | K::KeyA | K::KeyQ => "CMD_FCS_MTX_LEFT",
        K::ArrowRight | K::KeyD => "CMD_FCS_MTX_RIGHT",
        K::Enter | K::NumpadEnter | K::Space => "CMD_ENTER",
        K::Escape | K::Backspace => "CMD_BACK",
        _ => return None,
    })
}

// ── Résolution VFS + chargement sprites menu ─────────────────────────────────

/// Locale par défaut pour les assets de menu localisés (`<LG>`). Les captures de référence du repo
/// (`start.png`/`menu.png`) sont en français → `fr`. TODO : exposer via `--locale`.
const MENU_LOCALE: &str = "fr";

/// Résout un chemin logique VFS en cherchant une entrée dont le chemin se termine
/// par `/<basename>`, où `basename` est le dernier segment après `/`.
///
/// Les chemins qui contiennent `<LG>` le portent uniquement dans la partie
/// répertoire — jamais dans le basename lui-même — donc aucun stripping n'est requis.
fn resolve_vfs_basename(vfs: &Vfs, logical_path: &str, locale: &str) -> Option<String> {
    let basename = logical_path.rsplit('/').next().filter(|s| !s.is_empty())?;

    // Tous les chemins VFS finissant par `/basename`. Le VFS est indexé par HashMap (ordre
    // d'itération NON déterministe) → on COLLECTE + TRIE pour un résultat reproductible : un
    // `.find()` direct choisissait une locale au hasard à chaque run (rendu non déterministe,
    // fatal pour le gate pixel-perfect byte-exact).
    let mut matches: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.ends_with(basename)
                && (p.len() == basename.len()
                    || p.as_bytes().get(p.len() - basename.len() - 1) == Some(&b'/'))
        })
        .collect();
    matches.sort_unstable();

    // Segment de répertoire juste avant le basename (= tag de locale pour les assets localisés,
    // ex. `.../title02_01/fr/title02_01.g4tx` → "fr" ; non-localisé → nom du dossier de texture).
    let parent_seg = |p: &str| -> String {
        p.get(..p.len() - basename.len() - 1)
            .and_then(|d| d.rsplit('/').next())
            .unwrap_or("")
            .to_string()
    };

    // Priorité (port de l'ordre iecode `MenuLayoutExporter.RenderSpriteAsync`) :
    // 1. locale demandée ; 2. non-localisé (parent ≠ tag de locale) ; 3. common ; 4. en ;
    // 5. à défaut, le 1ᵉʳ par ordre lexicographique (toujours déterministe).
    if let Some(p) = matches.iter().find(|p| parent_seg(p) == locale) {
        return Some(p.clone());
    }
    if let Some(p) = matches.iter().find(|p| !is_locale_tag(&parent_seg(p))) {
        return Some(p.clone());
    }
    for fb in ["common", "en"] {
        if let Some(p) = matches.iter().find(|p| parent_seg(p) == fb) {
            return Some(p.clone());
        }
    }
    matches.into_iter().next()
}

/// Tags de locale connus du jeu (sous-dossiers `<LG>` des assets de menu).
fn is_locale_tag(seg: &str) -> bool {
    matches!(
        seg,
        "de" | "en" | "es" | "fr" | "it" | "pt" | "ja" | "ko" | "zh_hans" | "zh_hant" | "common"
    )
}

// NB (expérience FOND plein écran — gate SSIM 2026-06-15, REVERTÉE) : rendre le `bg_*` (`bg_title02_02`
// 2640×1200) plein écran pour `title02_00` n'améliore PAS la SSIM (0.2511 → 0.2497) et la texture rend
// **noir** (alpha/format, ou elle exige le mapping UV du mesh, pas un blit plein cadre). Le fond reste
// bloqué sur la compose mesh-UV (cf. DESIGN.md §6 : géométrie décodée, mais UV atlas-region + rendu
// mesh-UV à faire). Pas de sprite faux conservé (anti-faux-FAIT).

/// Sprite positionné prêt pour composition : transform écran, dimensions, pixels RGBA8.
type SpritePositionne = (menu::ScreenTransform, u32, u32, Vec<u8>);

/// Charge tous les sprites de l'écran `screen`, les décode et les trie par
/// `draw_priority` croissant (back-to-front, prêts pour composition).
///
/// Retourne `Vec<SpritePositionne>` = `Vec<(ScreenTransform, width, height, rgba_pixels)>`.
///
/// # Flux
///
/// 1. Monte le VFS directement.
/// 2. Filtre les objbin : `/menu/obj/`, basename préfixé par `screen`.
/// 3. Pour chaque objbin : parse → g4pkm → g4tx → décode RGBA → positionne.
/// 4. Trie par `draw_priority` croissant.
fn build_sprite_list(game_dir: &Path, screen: &str) -> Result<Vec<SpritePositionne>> {
    let data_dir = game_dir.join("data");

    let mut vfs = Vfs::new();
    vfs.init(&data_dir)
        .context("VFS init échoué (cpk_list.cfg.bin)")?;
    info!("VFS monté : {} assets", vfs.asset_count());

    let screen_lower = screen.to_ascii_lowercase();
    let mut obj_paths: Vec<String> = vfs
        .iter()
        .filter_map(|(path, _)| {
            if !path.contains("/menu/obj/") || !path.ends_with(".objbin") {
                return None;
            }
            let basename = path.rsplit('/').next()?;
            if basename
                .to_ascii_lowercase()
                .starts_with(screen_lower.as_str())
            {
                Some(path.to_string())
            } else {
                None
            }
        })
        .collect();
    // Le VFS est indexé par HashMap (ordre non déterministe). On TRIE pour un ordre de
    // traitement reproductible : le tri stable par `draw_priority` en aval préserve cet ordre
    // pour les ex æquo → rendu byte-identique d'un run à l'autre (prérequis du gate pixel-perfect).
    obj_paths.sort_unstable();

    info!(
        "écran '{}' : {} objbin correspondants",
        screen,
        obj_paths.len()
    );

    // (draw_priority, transform, w, h, rgba)
    let blacklist = screen_lower == "main_menu" || screen_lower.starts_with("mainmenu");
    let mut sprite_entries: Vec<(i32, menu::ScreenTransform, u32, u32, Vec<u8>)> = Vec::new();
    for obj_path in &obj_paths {
        if let Some(entry) = process_objbin_layer(&vfs, obj_path, blacklist) {
            sprite_entries.push(entry);
        }
    }

    // Tri back-to-front
    sprite_entries.sort_by_key(|(prio, _, _, _, _)| *prio);

    Ok(sprite_entries
        .into_iter()
        .map(|(_, t, w, h, rgba)| (t, w, h, rgba))
        .collect())
}

/// Traite UN objbin de layer (commun à `build_sprite_list` filtre-par-nom et
/// `build_sprite_list_from_setting` liste-de-layers) : objbin → g4pkm → g4tx (param `Texture`
/// OU dérivé co-localisé, D1.c) → décodage DDS → placement. `None` si une étape manque (le layer
/// n'a pas de sprite statique — contenu runtime), avec un `warn!` traçant la cause.
///
/// Retour : `(draw_priority, transform écran, largeur, hauteur, pixels RGBA8)`.
fn process_objbin_layer(
    vfs: &Vfs,
    obj_path: &str,
    apply_blacklist: bool,
) -> Option<(i32, menu::ScreenTransform, u32, u32, Vec<u8>)> {
    let obj_basename = obj_path.rsplit('/').next().unwrap_or(obj_path);

    // BLACKLIST de composition (D1.c) — layers dont le sprite statique décodé est un PARASITE
    // vs la capture réelle du main_menu : (1) le FOND objbin `mainmenu90_00_background` est une
    // texture bleu SATURÉ plein cadre (le vrai fond est pastel quasi-blanc) → remplacé par le
    // dégradé peint en dur (`paint_menu_background`) ; (2) la bande `mainmenu90_02_header_tab`
    // (atlas 5280×520) est posée à l'échelle 1.0 (fallback motion) → bande 4× la largeur écran qui
    // couvre tout le centre ; (3) les badges « new » (icône orange `!`, pastille verte de check)
    // n'existent pas dans la capture. Ces layers exigent le driver-transform C++/Lua (cf. §6/§13) ;
    // tant qu'il n'est pas émulé, leur sprite brut dégrade la SSIM au lieu de l'améliorer.
    const COMPOSE_BLACKLIST: &[&str] = &[
        "mainmenu90_00_background", // fond bleu saturé → dégradé pastel peint à la place
        "mainmenu90_01_header",     // bande 5280×296 à l'échelle 1.0 → couvre le centre
        "mainmenu90_02_header_tab", // bande 5280×520 à l'échelle 1.0 → couvre le centre
        "mainmenu90_02_2_header_tab_icon",
        "cmn01_13_new_icon_middle",        // badge orange « ! »
        "cmn01_12_new_icon",               // pastille verte de check
        "cmn01_40_list_base_empty", // panneau gris + box verte + toggle (placement fallback centre)
        "mainmenu90_31_doc_item", // dossier translucide + pastille check (placement fallback centre)
        "mainmenu01_06_base_button_guide", // bande 4×92 dégénérée au centre
        "mainmenu01_07_button_guide", // 16×16 au centre
        "rpg00_07_weekday_timezone_guide", // toggle décodé placé au centre (fallback)
    ];
    let obj_stem = obj_basename.strip_suffix(".objbin").unwrap_or(obj_basename);
    if apply_blacklist && COMPOSE_BLACKLIST.contains(&obj_stem) {
        warn!("skip {obj_basename} : layer blacklisté (parasite de composition statique)");
        return None;
    }

    let obj_bytes = match vfs.read(obj_path) {
        Ok(d) => d,
        Err(e) => {
            warn!("skip {obj_basename} : lecture erreur : {e}");
            return None;
        }
    };
    let obj = match objbin::parse(&obj_bytes) {
        Ok(o) => o,
        Err(e) => {
            warn!("skip {obj_basename} : parse erreur : {e}");
            return None;
        }
    };

    // Résolution g4pkm
    let g4pkm_logical = match obj.g4pkm_path.as_deref() {
        Some(p) => p.to_string(),
        None => {
            warn!("skip {obj_basename} : pas de g4pkm_path");
            return None;
        }
    };
    let g4pkm_vfs = match resolve_vfs_basename(vfs, &g4pkm_logical, MENU_LOCALE) {
        Some(p) => p,
        None => {
            warn!("skip {obj_basename} : g4pkm '{g4pkm_logical}' absent du VFS");
            return None;
        }
    };
    let g4pkm_bytes = match vfs.read(&g4pkm_vfs) {
        Ok(d) => d,
        Err(e) => {
            warn!("skip {obj_basename} : lecture g4pkm erreur : {e}");
            return None;
        }
    };
    let layout = match g4pkm::parse(&g4pkm_bytes) {
        Ok(l) => l,
        Err(e) => {
            warn!("skip {obj_basename} : parse g4pkm erreur : {e}");
            return None;
        }
    };

    // Résolution g4tx : param `Texture` explicite OU dérivé co-localisé du g4pkm (D1.c — cas
    // mainmenu : le g4md de menu n'a pas de `material_base_names`, la texture est nommée comme le
    // conteneur du mesh ; chemin g4pkm AUTORITAIRE, ex. objbin `mainmenu01_07` → mesh `mainmenu01_07c`).
    // iecode a la MÊME limite (sprite gated sur `G4txPath`) → RE originale au-delà d'iecode.
    let g4tx_logical = match obj.g4tx_path.as_deref() {
        Some(p) => p.to_string(),
        None => match g4pkm_logical
            .rsplit('/')
            .next()
            .and_then(|b| b.strip_suffix(".g4pkm"))
            .map(|stem| format!("{stem}.g4tx"))
        {
            Some(derived) => derived,
            None => {
                warn!("skip {obj_basename} : pas de g4tx_path (ni dérivable du g4pkm)");
                return None;
            }
        },
    };
    let g4tx_vfs = match resolve_vfs_basename(vfs, &g4tx_logical, MENU_LOCALE) {
        Some(p) => p,
        None => {
            warn!("skip {obj_basename} : g4tx '{g4tx_logical}' absent du VFS");
            return None;
        }
    };
    let g4tx_bytes = match vfs.read(&g4tx_vfs) {
        Ok(d) => d,
        Err(e) => {
            warn!("skip {obj_basename} : lecture g4tx erreur : {e}");
            return None;
        }
    };
    let g4tx_parsed = match g4tx::parse(&g4tx_bytes) {
        Ok(p) => p,
        Err(e) => {
            warn!("skip {obj_basename} : parse g4tx erreur : {e}");
            return None;
        }
    };

    // Basename du conteneur (sans `.g4tx`) pour la résolution par nom (D1.b) :
    // évite de piocher un dummy 4×4 placé en tête de l'atlas.
    let g4tx_base = g4tx_vfs
        .rsplit('/')
        .next()
        .unwrap_or(g4tx_vfs.as_str())
        .strip_suffix(".g4tx")
        .unwrap_or("");
    let tex = match g4tx::select_main_texture(&g4tx_parsed, g4tx_base) {
        Some(t) => t,
        None => {
            warn!("skip {obj_basename} : aucune texture DDS dans g4tx");
            return None;
        }
    };
    let (w, h, rgba) = match g4tx_decode::decode_texture_rgba(&g4tx_bytes, tex) {
        Some(r) => r,
        None => {
            warn!("skip {obj_basename} : échec décodage DDS ({g4tx_vfs})");
            return None;
        }
    };

    let positioned = menu::assemble_object(&obj, &layout, w, h);
    Some((positioned.draw_priority, positioned.transform, w, h, rgba))
}

/// Compose un écran depuis sa **définition `*_menu_setting.cfg.bin`** (D1.c-driver, brique (a)) :
/// itère la liste `MENU_LAYER_INFO` (`nie_data::menu_setting`) au lieu du filtre par préfixe de nom.
///
/// C'est la composition CORRECTE d'un écran : `main_menu` mêle des layers `mainmenu90_*` (fond,
/// en-tête), `cmn01_*`, `mainmenu01_*` et `rpg00_*` — que le filtre `starts_with(screen)` rate. Le
/// `setting` est le préfixe du fichier (`main_menu` → `main_menu_setting.cfg.bin`).
///
/// NB : le PLACEMENT des widgets animés reste imparfait (bind pose hors-écran → driver, cf. §6) ;
/// mais les layers statiques on-écran (fond plein cadre, en-tête) se composent correctement.
fn build_sprite_list_from_setting(game_dir: &Path, setting: &str) -> Result<Vec<SpritePositionne>> {
    let data_dir = game_dir.join("data");
    let mut vfs = Vfs::new();
    vfs.init(&data_dir)
        .context("VFS init échoué (cpk_list.cfg.bin)")?;
    info!("VFS monté : {} assets", vfs.asset_count());

    let obj_paths = setting_objbin_paths(&vfs, setting);
    if obj_paths.is_empty() {
        bail!("menu_setting '{setting}_setting.cfg.bin' introuvable / sans layer dans le VFS");
    }
    info!("menu_setting '{setting}' : {} layers", obj_paths.len());

    // BLACKLIST de parasites + FOND peint : scopés au main_menu (cf. `cmd_menu`/`process_objbin_layer`).
    let blacklist = setting == "main_menu";
    let mut sprite_entries: Vec<(i32, menu::ScreenTransform, u32, u32, Vec<u8>)> = Vec::new();
    for obj_path in &obj_paths {
        if let Some(entry) = process_objbin_layer(&vfs, obj_path, blacklist) {
            sprite_entries.push(entry);
        }
    }

    // NB logo : le logo « INAZUMA ELEVEN Victory Road » du main_menu n'est PAS un layer du
    // menu_setting (posé au runtime par le driver). Tenté en STATIQUE depuis l'atlas de titre
    // `title00_03.g4tx` placé centre-haut (rect réel ~ x435,y108,415×98) — mais MESURÉ comme
    // RÉGRESSION SSIM (8×8 luma) : même bbox calée (w≈434/cy≈109 vs réel w419/cy118), les glyphes
    // haute-fréquence ne s'alignent pas au pixel près et les pixels sombres du logo débordant sur le
    // fond clair pénalisent plus que ne rapporte la zone alignée (0.6209 → 0.61 selon l'échelle).
    // Conservé documenté ; à reprendre quand le placement viendra du driver (cf. §6/§13), pas en dur.

    // Tri back-to-front (stable sur l'ordre du menu_setting pour les ex æquo).
    sprite_entries.sort_by_key(|(prio, _, _, _, _)| *prio);
    Ok(sprite_entries
        .into_iter()
        .map(|(_, t, w, h, rgba)| (t, w, h, rgba))
        .collect())
}

/// Résout, dans l'ordre de composition, les chemins VFS des objbin d'un écran depuis sa **définition
/// `<setting>_setting.cfg.bin`** (liste `MENU_LAYER_INFO`, `nie_data::menu_setting`). C'est la
/// composition CORRECTE (multi-préfixes : `mainmenu90_*`/`cmn01_*`/`mainmenu01_*`/`rpg00_*`), partagée
/// par le rendu PNG (`build_sprite_list_from_setting`) et l'export azalee (`collect_layout_objects`).
/// `Vec` vide si le setting est absent/illisible. Basename EXACT → déterministe (évite
/// `victory_road_main_menu_setting` pour `main_menu`).
fn setting_objbin_paths(vfs: &Vfs, setting: &str) -> Vec<String> {
    let target = format!("{setting}_setting.cfg.bin");
    let Some(setting_path) = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.contains("/menu/cfg/") && p.rsplit('/').next() == Some(target.as_str()))
        .min()
    else {
        return Vec::new();
    };
    let Ok(bytes) = vfs.read(&setting_path) else {
        return Vec::new();
    };
    let Ok(parsed) = cfgbin::parse_t2b(&bytes) else {
        return Vec::new();
    };
    let root = serde_json::json!({ "entries": t2b_siblings_to_iecode(&parsed.entries) });
    let ms = nie_data::menu_setting::parse(&root);
    ms.layers
        .iter()
        .filter_map(|l| {
            let basename = l.objbin_path.rsplit('/').next().unwrap_or(&l.objbin_path);
            let stem = basename.strip_suffix(".objbin").unwrap_or(basename);
            let pref = format!("{stem}_");
            // Le setting référence souvent le PRÉFIXE (`btl01_10.objbin`) alors que le fichier réel
            // porte un suffixe descriptif (`btl01_10_battle_title.objbin`). On résout par match exact,
            // sinon par préfixe + `_` (le plus court = le plus proche du préfixe).
            let resolved = vfs
                .iter()
                .map(|(p, _)| p.to_string())
                .filter(|p| {
                    let b = p.rsplit('/').next().unwrap_or(p);
                    b == basename
                        || b.strip_suffix(".objbin")
                            .is_some_and(|s| s == stem || s.starts_with(&pref))
                })
                .min_by_key(|p| p.rsplit('/').next().unwrap_or(p).len());
            if resolved.is_none() {
                warn!("layer '{}' : objbin '{basename}' absent du VFS", l.name);
            }
            resolved
        })
        .collect()
}

// ── Export de layout (contrat azalee `@rose-griffon/menu-render`) ─────────────

/// Exporte le LAYOUT d'un écran de menu en JSON consommé par azalee (renderer WebGPU /
/// PNG serveur), en lieu et place de l'export iecode. Réutilise le pipeline niers amélioré :
/// placement **motion-fallback** (D1.a, `menu::place_on_canvas`) + sélection de texture
/// non-dummy (D1.b, `g4tx::select_main_texture`) + résolution de locale déterministe.
///
/// Inclut TOUS les objbin de l'écran (ceux sans sprite statique → `sprite: null`, contenu
/// instancié au runtime côté jeu — cf. DESIGN.md §5/§13). Schéma = `MenuLayout` de
/// `packages/menu-render/src/types.ts`.
fn cmd_export_layout(
    game_dir: &Path,
    screen: &str,
    screen_name: &str,
    out: &Path,
    from_setting: bool,
) -> Result<()> {
    use serde_json::json;

    let data_dir = game_dir.join("data");
    let mut vfs = Vfs::new();
    vfs.init(&data_dir)
        .context("VFS init échoué (cpk_list.cfg.bin)")?;
    info!("VFS monté : {} assets", vfs.asset_count());

    let (objs, n_sprites) = collect_layout_objects(&vfs, screen, from_setting);
    let n_objects = objs.len();
    let objects: Vec<serde_json::Value> = objs.iter().map(LayoutObj::to_json).collect();

    let layout = json!({
        "screen": screen_name,
        "locale": MENU_LOCALE,
        "canvas": { "w": 1280, "h": 720 },
        "objects": objects,
    });
    let txt = serde_json::to_string_pretty(&layout)?;
    std::fs::write(out, &txt).with_context(|| format!("écriture {}", out.display()))?;
    println!(
        "export-layout: screen={screen_name} objets={n_objects} sprites={n_sprites} -> {} ({} octets)",
        out.display(),
        txt.len()
    );
    Ok(())
}

/// Un objet de layout de menu en cours de construction (placement statique + overrides runtime).
///
/// Les champs `visible`/`text`/`runtime` ne sont pris en compte que par la sérialisation runtime
/// ([`LayoutObj::to_json_runtime`]) ; la sérialisation statique ([`LayoutObj::to_json`]) émet
/// exactement le schéma `MenuLayout` historique (contrat azalee inchangé).
struct LayoutObj {
    /// Nom de l'objbin (clé du join crc32 avec le `MenuState`).
    name: String,
    /// Transform écran (placement motion-fallback D1.a).
    transform: serde_json::Value,
    draw_priority: i32,
    draw_type: i32,
    camera: u32,
    /// Sprite statique résolu (texture non-dummy D1.b) ou `Null`.
    sprite: serde_json::Value,
    /// Métadonnées d'animation (`open`/`close`) ou `Null`.
    anim: serde_json::Value,
    /// Visibilité — défaut `true`, mise à `false` par le runtime Lua (`SetObjectVisible`).
    visible: bool,
    /// Texte affiché — `Null` en statique, renseigné par le runtime (`SetText`/`SetObjectNum`).
    text: serde_json::Value,
    /// Diagnostic des mutations runtime appliquées à cet objet (ou `Null` si non joint).
    runtime: serde_json::Value,
}

impl LayoutObj {
    /// Sérialise au schéma `MenuLayout` historique (azalee) — strictement les champs d'origine.
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "parent": serde_json::Value::Null,
            "transform": self.transform,
            "drawPriority": self.draw_priority,
            "drawType": self.draw_type,
            "camera": format!("0x{:08X}", self.camera),
            "sprite": self.sprite,
            "text": self.text,
            "anim": self.anim,
            "primitive": serde_json::Value::Null,
            "charModel": serde_json::Value::Null,
        })
    }

    /// Sérialise au schéma `MenuLayout` + champs runtime (`visible`, `runtime`) — layout généré
    /// par l'exécution des scripts Lua.
    fn to_json_runtime(&self) -> serde_json::Value {
        let mut v = self.to_json();
        if let serde_json::Value::Object(m) = &mut v {
            m.insert("visible".into(), serde_json::Value::Bool(self.visible));
            m.insert("runtime".into(), self.runtime.clone());
        }
        v
    }
}

/// Construit la liste des objets de layout d'un écran (placement + sprite statiques), triée
/// back-to-front par `draw_priority`. Logique partagée par l'export statique et l'export runtime.
///
/// Retourne `(objets, nb_sprites_résolus)`.
/// Convertit des frères T2B en forme iecode (suffixe `_<idx>` par nom, récursif) pour le résolveur
/// de texte universel `nie_data::text` (réplique de `t2b_siblings_to_iecode` de nie-model-serve).
fn t2b_siblings_to_iecode(siblings: &[cfgbin::CfgEntry]) -> Vec<serde_json::Value> {
    use serde_json::json;
    use std::collections::HashMap;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    siblings
        .iter()
        .map(|e| {
            let idx = counts.entry(e.name.as_str()).or_insert(0);
            let name = format!("{}_{}", e.name, *idx);
            *idx += 1;
            let variables: Vec<serde_json::Value> = e
                .variables
                .iter()
                .map(|v| match v {
                    cfgbin::Value::String(s) => json!({ "type": "String", "value": s }),
                    cfgbin::Value::Int(n) => json!({ "type": "Int", "value": n.to_string() }),
                    cfgbin::Value::Float(f) => json!({ "type": "Float", "value": f.to_string() }),
                })
                .collect();
            json!({ "name": name, "variables": variables, "children": t2b_siblings_to_iecode(&e.children) })
        })
        .collect()
}

/// Charge la table `menu_text` (locale [`MENU_LOCALE`]) en `(HashId, String)` via le résolveur
/// universel de `nie-data`, pour résoudre les libellés **statiques** de menu. Vide si absente.
///
/// Cf. DESIGN.md §7 : `menu_text` est la source confirmée des libellés UI statiques (ex.
/// `0x40687BAD` → « Informations ») ; les libellés runtime (noms d'items) restent au driver.
fn load_menu_text(vfs: &Vfs) -> Vec<(nie_data::hash::HashId, String)> {
    let needle = format!("/text/{MENU_LOCALE}/");
    let Some(path) = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .find(|p| p.contains(&needle) && p.rsplit('/').next() == Some("menu_text.cfg.bin"))
    else {
        return Vec::new();
    };
    let Ok(bytes) = vfs.read(&path) else {
        return Vec::new();
    };
    let Ok(file) = cfgbin::parse_t2b(&bytes) else {
        return Vec::new();
    };
    let root = serde_json::json!({ "entries": t2b_siblings_to_iecode(&file.entries) });
    nie_data::text::parse_text_file(&root)
}

fn collect_layout_objects(vfs: &Vfs, screen: &str, from_setting: bool) -> (Vec<LayoutObj>, usize) {
    use serde_json::{Value, json};

    // Table de texte de menu (locale fr) : résout les libellés STATIQUES (cf. DESIGN.md §7).
    let menu_text = load_menu_text(vfs);

    // Source des objbin : soit la **définition `menu_setting`** (composition correcte multi-préfixes,
    // D1.c-driver brique (a)), soit le filtre historique par préfixe de nom.
    let obj_paths: Vec<String> = if from_setting {
        setting_objbin_paths(vfs, screen)
    } else {
        let screen_lower = screen.to_ascii_lowercase();
        let mut v: Vec<String> = vfs
            .iter()
            .filter_map(|(path, _)| {
                if !path.contains("/menu/obj/") || !path.ends_with(".objbin") {
                    return None;
                }
                let basename = path.rsplit('/').next()?;
                basename
                    .to_ascii_lowercase()
                    .starts_with(screen_lower.as_str())
                    .then(|| path.to_string())
            })
            .collect();
        v.sort_unstable(); // ordre déterministe (VFS = HashMap)
        v
    };

    // ── Table des points d'attache de l'écran ────────────────────────────────────────────────
    //
    // Un objet porteur d'un `CMenuAttachLocator` ne se dessine pas : il déclare **où vont les
    // autres** (cf. `menu::attach_slots`). Sans cette table, tout objet dont le squelette propre
    // ne donne pas de pose retombe au centre du canvas — c'est ce qui empilait 27 des 42 objets
    // de `victory_road_top_menu` en (640, 360). On la construit avant la boucle : un emplacement
    // peut être déclaré par un objet qu'on n'a pas encore traité.
    let mut attaches: std::collections::BTreeMap<u32, Vec<menu::AttachSlot>> =
        std::collections::BTreeMap::new();
    for obj_path in &obj_paths {
        let Ok(obj_bytes) = vfs.read(obj_path) else {
            continue;
        };
        let Ok(obj) = objbin::parse(&obj_bytes) else {
            continue;
        };
        let Some(g4pkm_logical) = obj.g4pkm_path.as_deref() else {
            continue;
        };
        let Some(g4pkm_vfs) = resolve_vfs_basename(vfs, g4pkm_logical, MENU_LOCALE) else {
            continue;
        };
        let Ok(bytes) = vfs.read(&g4pkm_vfs) else {
            continue;
        };
        let Ok(skel) = g4pkm::parse(&bytes) else {
            continue;
        };
        for slot in menu::attach_slots(&obj, &skel) {
            attaches.entry(slot.target_hash).or_default().push(slot);
        }
    }

    let mut objects: Vec<LayoutObj> = Vec::new();
    let mut n_sprites = 0usize;

    for obj_path in &obj_paths {
        let Ok(obj_bytes) = vfs.read(obj_path) else {
            continue;
        };
        let Ok(obj) = objbin::parse(&obj_bytes) else {
            continue;
        };

        // Métadonnées de composants.
        let (mut draw_priority, mut draw_type, mut camera) = (0i32, 0i32, 0u32);
        let mut anim = Value::Null;
        let mut text_labels: Vec<Value> = Vec::new();
        for c in &obj.components {
            match c {
                objbin::MenuComponent::Render(r) => {
                    draw_priority = r.draw_priority;
                    draw_type = r.draw_type;
                    camera = r.camera_name_hash;
                }
                objbin::MenuComponent::Animation(a) => {
                    let hx = |h: u32| {
                        if h != 0 {
                            json!(format!("0x{h:08X}"))
                        } else {
                            Value::Null
                        }
                    };
                    anim = json!({ "open": hx(a.mot_open_hash), "close": hx(a.mot_close_hash) });
                }
                objbin::MenuComponent::Text(tc) => {
                    // Résolution des libellés STATIQUES : pour chaque slot, le 1ᵉʳ hash présent
                    // dans `menu_text` (pas toujours `hashes[0]` — cf. DESIGN.md §7). Les slots
                    // non résolus (libellés runtime, ex. « COMMENCER ») restent au driver D1.c.
                    for e in &tc.entries {
                        if let Some(label) = e.hashes.iter().find_map(|h| {
                            nie_data::text::find_text(&menu_text, nie_data::hash::HashId(*h))
                        }) {
                            text_labels.push(json!({ "slot": e.key, "text": label }));
                        }
                    }
                }
                _ => {}
            }
        }

        // Placement (motion-fallback) + sprite (texture non-dummy), si assets résolus.
        let mut transform = json!({
            "x": 640.0, "y": 360.0, "scaleX": 1.0, "scaleY": 1.0,
            "rot": 0.0, "anchorX": 0.5, "anchorY": 0.5
        });
        let mut sprite = Value::Null;

        // g4tx explicite (param `Texture`) OU dérivé co-localisé du g4pkm (cas mainmenu01, cf.
        // `build_sprite_list` D1.c) : `<mesh-stem>.g4tx`, résolu par basename dans le VFS.
        let g4tx_logical: Option<String> =
            obj.g4tx_path.as_deref().map(str::to_string).or_else(|| {
                obj.g4pkm_path
                    .as_deref()
                    .and_then(|p| p.rsplit('/').next())
                    .and_then(|b| b.strip_suffix(".g4pkm"))
                    .map(|stem| format!("{stem}.g4tx"))
            });
        if let (Some(g4pkm_logical), Some(g4tx_logical)) =
            (obj.g4pkm_path.as_deref(), g4tx_logical.as_deref())
            && let Some(g4pkm_vfs) = resolve_vfs_basename(vfs, g4pkm_logical, MENU_LOCALE)
            && let Ok(g4pkm_bytes) = vfs.read(&g4pkm_vfs)
            && let Ok(skel) = g4pkm::parse(&g4pkm_bytes)
            && let Some(g4tx_vfs) = resolve_vfs_basename(vfs, g4tx_logical, MENU_LOCALE)
            && let Ok(g4tx_bytes) = vfs.read(&g4tx_vfs)
            && let Ok(parsed) = g4tx::parse(&g4tx_bytes)
        {
            let base = g4tx_vfs
                .rsplit('/')
                .next()
                .unwrap_or("")
                .strip_suffix(".g4tx")
                .unwrap_or("");
            if let Some(tex) = g4tx::select_main_texture(&parsed, base) {
                let (w, h) = (tex.width.max(0) as u32, tex.height.max(0) as u32);
                let st = menu::place_on_canvas(&skel, w, h);
                transform = json!({
                    "x": st.x_px, "y": st.y_px, "scaleX": st.scale_x, "scaleY": st.scale_y,
                    "rot": st.rot, "anchorX": 0.5, "anchorY": 0.5
                });
                // logicalPath = chemin VFS sans `data/` ; pngUrl = `/<logical>` en `.png`.
                let logical = g4tx_vfs
                    .strip_prefix("data/")
                    .unwrap_or(&g4tx_vfs)
                    .to_string();
                let stem = logical.strip_suffix(".g4tx").unwrap_or(&logical);
                sprite = json!({
                    "logicalPath": logical, "pngUrl": format!("/{stem}.png"), "w": w, "h": h
                });
                n_sprites += 1;
            }
        }

        // Position réelle par point d'attache, si un locator de cet écran en déclare une pour cet
        // objet. Elle prime sur le placement issu du squelette propre : le locator dit **où**,
        // le squelette de l'objet dit seulement **quelle taille** (l'échelle reste donc celle
        // déjà calculée). Plusieurs emplacements = plusieurs instances du même objet (les items
        // d'une liste), pas un doublon : on en émet une par emplacement.
        let poses_attache: Vec<(f32, f32)> = attaches
            .get(&nie_formats::cfgbin::crc32(obj.name.as_bytes()))
            .map(|v| {
                v.iter()
                    .map(nie_formats::menu::AttachSlot::to_css)
                    .collect()
            })
            .unwrap_or_default();
        for (i, (x, y)) in poses_attache.iter().enumerate() {
            let mut t = transform.clone();
            t["x"] = json!(x);
            t["y"] = json!(y);
            objects.push(LayoutObj {
                name: obj.name.clone(),
                transform: t,
                draw_priority,
                draw_type,
                camera,
                sprite: sprite.clone(),
                anim: anim.clone(),
                visible: true,
                // Le libellé STATIQUE n'appartient qu'au premier emplacement. Les items d'une
                // liste reçoivent chacun le leur au runtime (`SetText` par index) : recopier le
                // même texte sur les N instances ne reproduit pas l'écran, il empile N fois la
                // même chaîne — sur l'arbre du tournoi (175 emplacements) cela noyait le rendu.
                text: if i == 0 && !text_labels.is_empty() {
                    Value::Array(text_labels.clone())
                } else {
                    Value::Null
                },
                runtime: Value::Null,
            });
        }
        if !poses_attache.is_empty() {
            continue;
        }

        objects.push(LayoutObj {
            name: obj.name,
            transform,
            draw_priority,
            draw_type,
            camera,
            sprite,
            anim,
            visible: true,
            text: if text_labels.is_empty() {
                Value::Null
            } else {
                json!(text_labels)
            },
            runtime: Value::Null,
        });
    }

    // Tri back-to-front (stable : ex æquo en ordre alphabétique déterministe).
    objects.sort_by_key(|o| o.draw_priority);
    (objects, n_sprites)
}

/// Compte les **item-buttons par layer-list** depuis les slots `AttachLocator` des objbin de
/// l'écran — la donnée de scène que `GetObjectAttr` (`0x4612788B`) renvoie au script (lu par
/// `GetItemButtonNum`). Sans elle, le menu reste vide (`GetItemButtonNum` = 0).
///
/// Format `NullLayerName` (vérifié sur `title02_02_item_atc_locator_2`) : liste plate de quads
/// `[hashLocatorA, hashLocatorB, layerHash, slotIndex]`. Le nombre de slots d'un layer = nombre
/// de quads portant ce `layerHash`. Ex. title02 : `2250456639`→8 (#L7_7), `3873872512`→3
/// (#L8_8), `3180406576`→11 — concordent exactement avec les tables du script décompilé.
///
/// Retourne `layerHash -> nombre d'items`.
fn collect_item_counts(vfs: &Vfs, screen: &str) -> std::collections::BTreeMap<u32, i32> {
    // Les calques d'un écran viennent de sa DÉFINITION, pas de son nom. Le filtre par préfixe ne
    // vaut que pour les écrans dont les objbin portent le nom de l'écran (`mainmenu01_*` pour
    // `mainmenu`) ; il ne trouve RIEN pour l'éditeur d'avatar, dont les calques s'appellent
    // `avatar01_*`, `mainmenu90_*`, `soccer11_*`, `cmn05_*`. Le compte d'items restait donc vide,
    // et avec lui les listes que `GetItemButtonNum` interroge. Repli sur le préfixe quand l'écran
    // n'a pas de définition.
    let depuis_setting = setting_objbin_paths(vfs, screen);
    let screen_lower = screen.to_ascii_lowercase();
    let mut counts: std::collections::BTreeMap<u32, i32> = std::collections::BTreeMap::new();
    for (path, _) in vfs.iter() {
        if !path.contains("/menu/obj/") || !path.ends_with(".objbin") {
            continue;
        }
        let Some(basename) = path.rsplit('/').next() else {
            continue;
        };
        let retenu = if depuis_setting.is_empty() {
            basename
                .to_ascii_lowercase()
                .starts_with(screen_lower.as_str())
        } else {
            depuis_setting
                .iter()
                .any(|p| p == path || p.ends_with(basename))
        };
        if !retenu {
            continue;
        }
        let Ok(bytes) = vfs.read(path) else { continue };
        let Ok(obj) = objbin::parse(&bytes) else {
            continue;
        };
        for c in &obj.components {
            if let objbin::MenuComponent::AttachLocator(a) = c {
                // Quads [A, B, layerHash, slotIndex] : compter par layerHash.
                for quad in a.null_layer_hashes.chunks_exact(4) {
                    *counts.entry(quad[2]).or_insert(0) += 1;
                }
            }
        }
    }
    counts
}

// ── Export de layout AU RUNTIME (driver Lua réel, comme nie.exe) ──────────────

/// État runtime fusionné d'un objet de menu (issu du `MenuState` Lua), pour le join crc32.
struct MergedObj {
    /// Visibilité — `false` si un script a appelé `SetObjectVisible(.., false)`.
    visible: bool,
    /// Visibilité par instance, quand la commande a nommé un index. Un écran réplique un même
    /// gabarit une fois par item ; sans cette carte, les exemplaires partagent un seul booléen et
    /// s'affichent ou disparaissent en bloc.
    visible_par_index: std::collections::BTreeMap<i32, bool>,
    /// Visibilité des parts adressées par hash (commande Kizuna dédiée).
    part_visible: std::collections::BTreeMap<u32, bool>,
    /// Couleurs RGBA flottantes des parts adressées par hash (commande Kizuna dédiée).
    part_color_rgba: std::collections::BTreeMap<u32, [f32; 4]>,
    /// Arguments bruts des mutations Kizuna texture/paramètres/flags.
    part_texture_args: std::collections::BTreeMap<u32, Vec<u32>>,
    part_param_args: std::collections::BTreeMap<u32, Vec<u32>>,
    part_flag_args: std::collections::BTreeMap<u32, Vec<u32>>,
    /// Hash de texture/chemin g4tx du sprite (`SetSprite`/`SetIconSprite` arg1).
    sprite_hash: Option<u32>,
    /// Hash de la région/texture dans l'atlas (`SetIconSprite` arg2). Paire (chemin, région).
    sprite_region: Option<u32>,
    /// Texte affiché (`SetText`).
    text: Option<String>,
    /// Valeur numérique (`SetObjectNum`).
    number: Option<i32>,
}

impl Default for MergedObj {
    fn default() -> Self {
        Self {
            visible: true,
            visible_par_index: std::collections::BTreeMap::new(),
            part_visible: std::collections::BTreeMap::new(),
            part_color_rgba: std::collections::BTreeMap::new(),
            part_texture_args: std::collections::BTreeMap::new(),
            part_param_args: std::collections::BTreeMap::new(),
            part_flag_args: std::collections::BTreeMap::new(),
            sprite_hash: None,
            sprite_region: None,
            text: None,
            number: None,
        }
    }
}

/// Aiguillage écran → préfixe de script `.lua.bin` (vérité terrain MEMORY.md / DESIGN.md §13).
///
/// Pour les écrans connus, renvoie le préfixe du script PRIMAIRE (évite de piloter les 11
/// variantes `title_menu_*`) ; sinon, recherche par le nom d'écran tel quel.
fn screen_script_needles(screen: &str) -> Vec<String> {
    match screen.to_ascii_lowercase().as_str() {
        "mainmenu" | "main_menu" => vec!["main_menu".into()],
        "title" | "title02" | "titlemenu" => vec!["title_menu_2".into()],
        "topmenu" | "topmenu_top" => vec!["topmenu_top".into()],
        other => vec![other.to_string()],
    }
}

/// Charge le dictionnaire CRC32→nom reversé du corpus Lua DÉCOMPILÉ
/// (`data/re/menu-crc32-dictionary.json`, 160513 entrées, récupéré du VPS 2026-06-16). Les hash de
/// menu (sprites, nœuds, objets) SONT du CRC32 de noms (vérifié `CRC32("Focus")=0xA30165ED`) → ce
/// dico les résout en noms lisibles. `HashMap` vide si le fichier est absent (résolution best-effort).
fn load_menu_crc32_dict() -> std::collections::HashMap<u32, String> {
    let mut m = std::collections::HashMap::new();
    let candidates = [
        std::path::PathBuf::from("data/re/menu-crc32-dictionary.json"),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/re/menu-crc32-dictionary.json"),
    ];
    for cand in candidates {
        if let Ok(txt) = std::fs::read_to_string(&cand)
            && let Ok(raw) = serde_json::from_str::<std::collections::HashMap<String, String>>(&txt)
        {
            for (k, v) in raw {
                if let Some(h) = k
                    .strip_prefix("0x")
                    .and_then(|s| u32::from_str_radix(s, 16).ok())
                {
                    m.insert(h, v);
                }
            }
            break;
        }
    }
    m
}

/// Charge l'index `nom-de-région → chemin g4tx` (généré par `--build-region-index`).
/// Résout les `spriteRegion` du runtime (ex. `gtxt_rarity01_05`) vers le fichier g4tx qui les
/// contient quand le `spritePath` runtime est un nom de nœud et non un chemin g4tx.
fn load_region_index() -> std::collections::HashMap<String, String> {
    let candidates = [
        std::path::PathBuf::from("data/re/menu-region-index.json"),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/re/menu-region-index.json"),
    ];
    for cand in candidates {
        if let Ok(txt) = std::fs::read_to_string(&cand)
            && let Ok(raw) = serde_json::from_str::<std::collections::HashMap<String, String>>(&txt)
        {
            return raw;
        }
    }
    std::collections::HashMap::new()
}

/// Génère le layout d'un écran AU RUNTIME comme `nie.exe` : exécute les vrais scripts Lua de
/// l'écran via le driver reversé (`OnInit` → `OnSetupLayer` → `OnOpenLayer`, manager `0x14109D190`)
/// dans la VM Lua 5.2 réelle ([`nie_lua`]), récupère le `MenuState` produit, puis l'applique au
/// layout statique (placement D1.a + sprite D1.b) en joignant par `crc32(nom d'objbin)`.
///
/// Honnêteté : le `MenuState` peut être PARTIEL (voire vide) tant que la couche données
/// scène/save (lue par les fonctions de script `GetItemButtonNum`/…) n'est pas fournie — le
/// livrable est le CHEMIN runtime câblé + un diagnostic honnête, pas 100% du contenu.
fn cmd_export_layout_runtime(
    game_dir: &Path,
    screen: &str,
    screen_name: &str,
    out: &Path,
    from_setting: bool,
    frames: u32,
) -> Result<()> {
    use std::collections::BTreeMap;
    use std::rc::Rc;

    use nie_formats::cfgbin::crc32;
    use nie_lua::host::{HostRegistry, LogSink};
    use nie_lua::session::LuaSession;
    use nie_lua::{HeaderTab, enumerate_header_tabs};
    use serde_json::{Value, json};

    let data_dir = game_dir.join("data");
    let mut vfs = Vfs::new();
    vfs.init(&data_dir)
        .context("VFS init échoué (cpk_list.cfg.bin)")?;
    info!("VFS monté : {} assets", vfs.asset_count());

    // 1) Layout statique de l'écran (placement + sprite) — base à muter par le runtime.
    let (mut objects, n_sprites) = collect_layout_objects(&vfs, screen, from_setting);
    // La même table alimente le getter général Lua `GetText`, pour les libellés que le script
    // construit dynamiquement (les libellés statiques sont déjà joints dans le layout ci-dessus).
    let menu_text = load_menu_text(&vfs);

    // 2) Inventaire des chemins Lua du VFS (l’index physique/logique est construit par
    //    `LuaSession::with_script_paths` au moment du pilotage live).
    let script_paths: Vec<String> = vfs.iter().map(|(p, _)| p.to_string()).collect();
    let mut menu_scripts: Vec<String> = Vec::new();
    for (p, _) in vfs.iter() {
        if p.starts_with("data/common/script/lua/menu/") && p.ends_with(".lua.bin") {
            menu_scripts.push(p.to_string());
        }
    }
    menu_scripts.sort();

    // 3) Scripts candidats pour cet écran (alias vérité terrain + filtrage par sous-chaîne).
    let needles = screen_script_needles(screen);
    let scripts: Vec<String> = menu_scripts
        .iter()
        .filter(|p| {
            let b = p.rsplit('/').next().unwrap_or(p).to_ascii_lowercase();
            needles.iter().any(|n| b.contains(n.as_str()))
        })
        .cloned()
        .collect();
    if scripts.is_empty() {
        warn!("aucun script .lua.bin pour l'écran '{screen}' (needles={needles:?})");
    }

    // Donnée de scène : nombre d'item-buttons par layer-list (slots AttachLocator des objbin).
    // C'est ce que `GetObjectAttr`/`GetItemButtonNum` lit ; sans elle le menu reste vide.
    let item_counts = collect_item_counts(&vfs, screen);
    info!("item_counts (layer-list -> #items) : {item_counts:?}");

    // layerIds à piloter : les hashes crc32 des NOMS d'objets objbin de l'écran (= les vrais
    // layerIds sur lesquels OnSetupLayer/OnOpenLayer du script dispatchent, donc ce qui joint au
    // layout) + les layers item-list (slots AttachLocator) + 0 (= tous) + crc32(nom d'écran).
    let screen_hash = crc32(screen.as_bytes());
    let mut drive_layers: Vec<u32> = objects.iter().map(|o| crc32(o.name.as_bytes())).collect();
    drive_layers.extend(item_counts.keys().copied());
    drive_layers.push(0);
    drive_layers.push(screen_hash);
    drive_layers.sort_unstable();
    drive_layers.dedup();

    let vfs = Rc::new(vfs);
    // 4) DRIVE chaque script dans sa propre VM (état propre), fusionne les MenuState.
    let mut merged_objs: BTreeMap<u32, MergedObj> = BTreeMap::new();
    // Objets qu'au moins un calque déclare visibles — sert à mesurer ce que la conjonction efface.
    let mut vus_visibles: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut merged_layers: BTreeMap<u32, bool> = BTreeMap::new();
    let mut total_known = 0usize;
    let mut total_list_items = 0usize;
    // cmdId -> (nombre d'appels, échantillon de représentation des args — aide à la RE du handler).
    let mut unknown_cmds: BTreeMap<u32, (usize, String)> = BTreeMap::new();
    // Même télémétrie pour `funcLuaCommand`, séparée des commandes de rendu menu.
    let mut unknown_general_cmds: BTreeMap<u32, (usize, String)> = BTreeMap::new();
    // Commandes reconnues, ventilées par nom : sépare celles qui agissent de celles qui se
    // contentent de rendre 1 au script.
    let mut known_by_name: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing_host_calls: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing_host_paths: BTreeMap<String, usize> = BTreeMap::new();
    let mut loaded_includes: BTreeMap<String, usize> = BTreeMap::new();
    let mut script_reports: Vec<(String, bool, usize, usize, usize)> = Vec::new();
    // Callbacks que les scripts de l'écran DÉFINISSENT. Le driver n'en joue qu'une partie
    // (`OnInit`, `OnSetupLayer`, `OnOpenLayer`, `OnEnter`, `Step`) ; les autres nomment
    // précisément ce qu'une exécution complète devrait déclencher. Sans cet inventaire, « il
    // manque l'état de navigation » reste une phrase ; avec lui, c'est une liste.
    let mut callbacks_definis: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    let mut callback_errors: Vec<String> = Vec::new();
    // Onglets d'en-tête virtuels (sous-items absents de l'objbin), énumérés depuis la vraie
    // logique du script (GetSortOfTabs + GetMenuObjectNameFromTabType). Clé = hash d'objet.
    let mut header_tabs: BTreeMap<u32, HeaderTab> = BTreeMap::new();

    for path in &scripts {
        let Ok(bytes) = vfs.read(path) else { continue };
        let name = path.rsplit('/').next().unwrap_or(path);

        // Le driver passe par la session persistante publique : index physique/logique, reader
        // VFS brut et MenuState live sont ainsi exactement le même chemin que les consommateurs
        // de `nie-lua`, sans reconstruire manuellement une VM instrumentée.
        let logs: LogSink = Rc::new(std::cell::RefCell::new(Vec::new()));
        let registry = HostRegistry::standard(Rc::clone(&logs));
        let session = LuaSession::with_script_paths(registry, logs, true, script_paths.clone(), {
            let vfs = Rc::clone(&vfs);
            move |path| vfs.read(path).ok()
        })
        .map_err(|e| anyhow::anyhow!("création session Lua VFS : {e}"))?;
        let state = session
            .menu_state()
            .ok_or_else(|| anyhow::anyhow!("session Lua sans MenuState"))?;
        {
            let mut state = state.borrow_mut();
            for (id, text) in &menu_text {
                state.set_text(id.0, text.clone());
            }
        }
        // Seed la donnée de scène AVANT le pilotage : GetObjectAttr/GetItemButtonNum la lit
        // pendant OnInit (le compte est mis en cache à ce moment-là).
        state.borrow_mut().object_attr.clone_from(&item_counts);
        let report = match session.drive_menu_for_frames(
            &bytes,
            name,
            &drive_layers,
            &item_counts,
            frames,
        ) {
            Ok(r) => r,
            Err(e) => {
                warn!("drive_menu {name} : {e}");
                continue;
            }
        };

        for include in session.take_loaded_includes() {
            *loaded_includes.entry(include).or_default() += 1;
        }

        // Énumère les onglets d'en-tête PENDANT que la VM est vivante (sous-items virtuels :
        // les 9 onglets du main_menu absents de l'objbin, issus de GetSortOfTabs réel).
        for tab in enumerate_header_tabs(session.lua()) {
            header_tabs.entry(tab.obj_hash).or_insert(tab);
        }

        let st = state.borrow();
        let n_layers = st.layers.len();
        let n_objs: usize = st.layers.values().map(|l| l.objects.len()).sum();
        total_known += st.known_cmd_log.len();
        total_list_items += st
            .layers
            .values()
            .flat_map(|l| l.objects.values())
            .map(|o| o.sub_items.len())
            .sum::<usize>();
        for (c, _, args_repr) in &st.unknown_cmd_log {
            let e = unknown_cmds.entry(*c).or_insert((0, args_repr.clone()));
            e.0 += 1;
        }
        for (c, _, args_repr) in &st.unknown_general_cmd_log {
            let e = unknown_general_cmds
                .entry(*c)
                .or_insert((0, args_repr.clone()));
            e.0 += 1;
        }
        // Ventilation des commandes RECONNUES par nom. Un compteur global d'appels connus ne dit
        // pas s'ils font quelque chose : une commande portée en « renvoie 1 » satisfait le script
        // sans rien peindre. Cette ventilation sépare les deux, et c'est elle qui dit si le driver
        // travaille ou s'il acquiesce.
        for (nom, _) in &st.known_cmd_log {
            *known_by_name.entry(nom.clone()).or_insert(0usize) += 1;
        }
        // Fusion déterministe (itération BTreeMap ordonnée) : visibilité ET-combinée, premiers
        // sprite/text/number gagnants.
        for (lid, layer) in &st.layers {
            let e = merged_layers.entry(*lid).or_insert(true);
            *e = *e && layer.visible;
            for (oid, o) in &layer.objects {
                // Diagnostic AVANT de changer quoi que ce soit : combien d'objets sont réclamés
                // visibles par un calque et cachés par un autre ? Un écran EMPILE ses calques, donc
                // une conjonction pourrait effacer un objet qu'un seul calque cache. Tant que ce
                // compteur n'est pas mesuré, changer la règle serait deviner.
                if o.visible {
                    vus_visibles.insert(*oid);
                }
                let m = merged_objs.entry(*oid).or_default();
                m.visible = m.visible && o.visible;
                for (idx, v) in &o.visible_par_index {
                    m.visible_par_index.entry(*idx).or_insert(*v);
                }
                for (part, v) in &o.part_visible {
                    m.part_visible.entry(*part).or_insert(*v);
                }
                for (part, rgba) in &o.part_color_rgba {
                    m.part_color_rgba.entry(*part).or_insert(*rgba);
                }
                for (part, args) in &o.part_texture_args {
                    m.part_texture_args
                        .entry(*part)
                        .or_insert_with(|| args.clone());
                }
                for (part, args) in &o.part_param_args {
                    m.part_param_args
                        .entry(*part)
                        .or_insert_with(|| args.clone());
                }
                for (part, args) in &o.part_flag_args {
                    m.part_flag_args
                        .entry(*part)
                        .or_insert_with(|| args.clone());
                }
                if m.sprite_hash.is_none() {
                    m.sprite_hash = o.sprite_texture_hash;
                    m.sprite_region = o.sprite_region_hash;
                }
                if m.text.is_none() {
                    m.text = o.text.clone();
                }
                if m.number.is_none() {
                    m.number = o.number;
                }
            }
        }
        info!(
            "driver {name} : top_level_ok={} on_init={:?} on_open={} layers={n_layers} \
             objects={n_objs} known={} unknown={}",
            report.top_level_ok,
            report.on_init,
            report.on_open,
            st.known_cmd_log.len(),
            st.unknown_cmd_log.len() + st.unknown_general_cmd_log.len()
        );
        callbacks_definis.extend(report.callbacks.iter().cloned());
        for call in &report.missing_host_calls {
            *missing_host_calls.entry(call.clone()).or_default() += 1;
        }
        for path in &report.missing_host_paths {
            *missing_host_paths.entry(path.clone()).or_default() += 1;
        }
        callback_errors.extend(
            report
                .callback_errors
                .iter()
                .map(|error| format!("{name}: {error}")),
        );
        script_reports.push((
            name.to_string(),
            report.on_open,
            n_layers,
            n_objs,
            st.known_cmd_log.len(),
        ));
    }

    // 5) APPLIQUE le MenuState fusionné au layout via crc32(objbin.name).
    // Dico CRC32→nom (corpus Lua décompilé) : résout les hash de sprites/nœuds en noms lisibles.
    let crc_dict = load_menu_crc32_dict();
    let region_index = load_region_index();
    // Cache des g4tx parsés (chemin logique → parse), pour résoudre les rects de région sans
    // recharger/remonter le VFS à chaque objet (render-from-runtime, étape rect).
    let mut g4tx_cache: std::collections::HashMap<String, Option<g4tx::G4tx>> =
        std::collections::HashMap::new();
    let (mut n_matched, mut n_hidden, mut n_sprite_mut, mut n_text_mut, mut n_sprite_named) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut n_region_rect = 0usize;
    // Rang d'apparition de chaque nom : les exemplaires d'un gabarit se distinguent par lui.
    let mut rangs_par_nom: std::collections::HashMap<String, i32> =
        std::collections::HashMap::new();
    for o in &mut objects {
        let h = crc32(o.name.as_bytes());
        let Some(m) = merged_objs.get(&h) else {
            continue;
        };
        n_matched += 1;
        let mut rt = serde_json::Map::new();
        rt.insert("matched".into(), Value::Bool(true));
        rt.insert("objHash".into(), json!(format!("0x{h:08X}")));
        if let Some(sh) = m.sprite_hash {
            rt.insert("spriteHash".into(), json!(format!("0x{sh:08X}")));
            // RÉSOLU (2026-06-16) : les hash de sprite SONT du CRC32 de noms — résolus via le dico du
            // corpus Lua décompilé (`menu-crc32-dictionary.json`). Pour `SetIconSprite` : la PAIRE
            // (chemin g4tx, région) = (spriteHash, spriteRegion) → texture réelle (render-from-runtime).
            if let Some(name) = crc_dict.get(&sh) {
                rt.insert("spritePath".into(), Value::String(name.clone()));
                n_sprite_named += 1;
            }
            if let Some(rh) = m.sprite_region {
                rt.insert("spriteRegionHash".into(), json!(format!("0x{rh:08X}")));
                if let Some(rn) = crc_dict.get(&rh) {
                    rt.insert("spriteRegion".into(), Value::String(rn.clone()));
                    // RENDER-FROM-RUNTIME : le nom de région → fichier g4tx (index) → rect de crop.
                    // Câble le chaînon manquant quand le `spritePath` runtime est un nom de nœud.
                    if let Some(g4tx_path) = region_index.get(rn) {
                        rt.insert("spriteRegionG4tx".into(), Value::String(g4tx_path.clone()));
                        let parsed = g4tx_cache.entry(g4tx_path.clone()).or_insert_with(|| {
                            obtenir_g4tx_bytes(game_dir, g4tx_path)
                                .ok()
                                .and_then(|(_, b)| g4tx::parse(&b).ok())
                        });
                        // `named_rect` et non `region_rect` : un nom d'icône désigne aussi bien une
                        // TEXTURE entière du conteneur qu'une sous-texture. Sur `avatar01_13.g4tx`,
                        // `edit_bar_icon14_off` est une région et `edit_bar_icon02_off` une texture
                        // — ne voir que les régions laissait la seconde en atlas complet.
                        if let Some(p) = parsed
                            && let Some((x, y, w, h)) = p.named_rect(rn)
                        {
                            rt.insert("spriteRect".into(), json!({"x": x, "y": y, "w": w, "h": h}));
                            // La texture à décoder n'est pas forcément la première du conteneur :
                            // sans son nom, le consommateur croppe le bon rectangle dans la
                            // mauvaise image.
                            if let Some(cible) = p.named(rn) {
                                let porteuse = match cible {
                                    g4tx::NamedTarget::Texture(t) => &t.name,
                                    g4tx::NamedTarget::Region { texture, .. } => &texture.name,
                                };
                                rt.insert(
                                    "spriteRegionTexture".into(),
                                    Value::String(porteuse.clone()),
                                );
                            }
                            n_region_rect += 1;
                        }
                    }
                }
            }
        }
        // Visibilité : celle de l'INSTANCE si une commande a nommé son index, la visibilité
        // d'objet sinon. Les exemplaires d'un même gabarit se suivent dans l'ordre des slots
        // d'attache, d'où le rang comme index. Sans cela ils s'affichaient en bloc — les 51
        // exemplaires de `avatar01_64_recipe_item_type01` tous masqués, les 16
        // `avatar01_63_recipe_item_title` tous montrés.
        let rang = {
            let r = rangs_par_nom.entry(o.name.clone()).or_insert(0i32);
            let v = *r;
            *r += 1;
            v
        };
        let visible_instance = m.visible_par_index.get(&rang).copied();
        if !visible_instance.unwrap_or(m.visible) {
            o.visible = false;
            n_hidden += 1;
        } else if visible_instance == Some(true) && !m.visible {
            // L'instance est nommée visible alors que l'objet ne l'est pas : l'instance gagne,
            // c'est la commande la plus précise des deux.
            o.visible = true;
        }
        if !m.part_visible.is_empty() {
            rt.insert(
                "partVisible".into(),
                json!(
                    m.part_visible
                        .iter()
                        .map(|(part, visible)| (format!("0x{part:08X}"), *visible))
                        .collect::<std::collections::BTreeMap<_, _>>()
                ),
            );
        }
        if !m.part_color_rgba.is_empty() {
            rt.insert(
                "partColorRgba".into(),
                json!(
                    m.part_color_rgba
                        .iter()
                        .map(|(part, rgba)| (format!("0x{part:08X}"), *rgba))
                        .collect::<std::collections::BTreeMap<_, _>>()
                ),
            );
        }
        for (key, values) in [
            ("partTextureArgs", &m.part_texture_args),
            ("partParamArgs", &m.part_param_args),
            ("partFlagArgs", &m.part_flag_args),
        ] {
            if !values.is_empty() {
                rt.insert(
                    key.into(),
                    json!(
                        values
                            .iter()
                            .map(|(part, args)| (format!("0x{part:08X}"), args))
                            .collect::<std::collections::BTreeMap<_, _>>()
                    ),
                );
            }
        }
        if m.sprite_hash.is_some() {
            n_sprite_mut += 1;
        }
        if let Some(t) = &m.text {
            o.text = Value::String(t.clone());
            rt.insert("text".into(), Value::String(t.clone()));
            n_text_mut += 1;
        } else if let Some(num) = m.number {
            o.text = json!(num);
            rt.insert("number".into(), json!(num));
            n_text_mut += 1;
        }
        o.runtime = Value::Object(rt);
    }

    // 5b) ONGLETS D'EN-TÊTE VIRTUELS : les 9 onglets du main_menu sont des sous-items absents de
    //     l'objbin (cf. MEMORY menu-rendering-data-trilogy + main_menu_1.02.92.00). Issus de la
    //     VRAIE logique du script (GetSortOfTabs -> GetMenuObjectNameFromTabType -> GetTabTextIdCRC),
    //     on les AJOUTE comme objets de layout (légitime : ce sont les vrais onglets). Placement
    //     dérivé : barre horizontale en haut (transform exact non disponible côté objbin).
    let static_hashes: std::collections::HashSet<u32> =
        objects.iter().map(|o| crc32(o.name.as_bytes())).collect();
    let n_tab_total = header_tabs.len();
    let mut n_tabs = 0usize;
    for (hash, tab) in &header_tabs {
        if static_hashes.contains(hash) {
            continue; // déjà présent dans l'objbin (n'arrive pas pour le main_menu) -> pas de doublon
        }
        // Position dérivée selon l'ORDRE VISUEL de l'onglet (tab.index = ordre GetSortOfTabs).
        let frac = if n_tab_total > 1 {
            tab.index as f64 / (n_tab_total as f64 - 1.0)
        } else {
            0.0
        };
        let x = 180.0 + frac * 920.0;
        let transform = json!({
            "x": x, "y": 44.0, "scaleX": 1.0, "scaleY": 1.0,
            "rot": 0.0, "anchorX": 0.5, "anchorY": 0.5
        });
        let text = if tab.text_id != 0 {
            Value::String(format!("0x{:08X}", tab.text_id))
        } else {
            Value::Null
        };
        let mut rt = serde_json::Map::new();
        rt.insert("matched".into(), Value::Bool(true));
        rt.insert("virtual".into(), Value::Bool(true));
        rt.insert("tabType".into(), json!(tab.tab_type));
        rt.insert("tabIndex".into(), json!(tab.index));
        rt.insert("objHash".into(), json!(format!("0x{hash:08X}")));
        if tab.text_id != 0 {
            rt.insert("textId".into(), json!(format!("0x{:08X}", tab.text_id)));
        }
        objects.push(LayoutObj {
            name: format!("mainmenu_header_tab_{:02}", tab.tab_type),
            transform,
            draw_priority: 1000, // au-dessus du fond/contenu (en-tête au premier plan)
            draw_type: 0,
            camera: 0,
            sprite: Value::Null,
            anim: Value::Null,
            visible: true,
            text,
            runtime: Value::Object(rt),
        });
        n_tabs += 1;
    }
    // Re-tri back-to-front (les onglets injectés à priorité 1000 vont au premier plan).
    objects.sort_by_key(|o| o.draw_priority);
    let n_matched_total = n_matched + n_tabs;

    // 6) Sérialise (schéma MenuLayout + champs runtime `visible`/`runtime`).
    let json_objects: Vec<Value> = objects.iter().map(LayoutObj::to_json_runtime).collect();
    let n_objects = json_objects.len();
    let n_visible = objects.iter().filter(|o| o.visible).count();
    let unknown_list: Vec<Value> = unknown_cmds
        .iter()
        .map(|(c, (n, args))| json!({ "cmdId": format!("0x{c:08X}"), "count": n, "args": args }))
        .collect();
    let unknown_general_list: Vec<Value> = unknown_general_cmds
        .iter()
        .map(|(c, (n, args))| json!({ "cmdId": format!("0x{c:08X}"), "count": n, "args": args }))
        .collect();
    let script_names: Vec<&str> = scripts
        .iter()
        .map(|s| s.rsplit('/').next().unwrap_or(s))
        .collect();

    let layout = json!({
        "screen": screen_name,
        "locale": MENU_LOCALE,
        "canvas": { "w": 1280, "h": 720 },
        "generatedBy": "runtime-lua",
        "objects": json_objects,
        "runtimeSummary": {
            "scripts": script_names,
            "layersTouched": merged_layers.len(),
            "objectsInMenuState": merged_objs.len(),
            "objectsMatched": n_matched,
            "headerTabsAdded": n_tabs,
            "objectsMatchedTotal": n_matched_total,
            "listItemsRecorded": total_list_items,
            "objectsHidden": n_hidden,
            "knownCmdsByName": known_by_name,
            "missingHostCalls": missing_host_calls,
            "missingHostPaths": missing_host_paths,
            "loadedIncludes": loaded_includes,
            "callbacksDefinis": callbacks_definis,
            "callbackErrors": callback_errors,
            // Ceux que le driver ne joue pas : la cible exacte du travail de navigation restant.
            "callbacksNonJoues": callbacks_definis
                .iter()
                .filter(|c| {
                    !matches!(
                        c.as_str(),
                        "OnInit" | "OnSetupLayer" | "OnOpenLayer" | "OnEnter" | "Step"
                    )
                })
                .collect::<Vec<_>>(),
            // Combien d'objets reçoivent une visibilité NOMMÉE par index, et sur combien d'index
            // distincts : dit si les scripts distinguent vraiment les exemplaires d'un gabarit, ou
            // s'ils les commandent tous par le même index.
            "objectsWithIndexedVisibility": merged_objs
                .values()
                .filter(|m| !m.visible_par_index.is_empty())
                .count(),
            "distinctVisibilityIndexes": merged_objs
                .values()
                .flat_map(|m| m.visible_par_index.keys())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            // Objets qu'un calque veut visibles et qu'un autre cache : ce que la conjonction efface.
            "objectsHiddenByMerge": vus_visibles
                .iter()
                .filter(|oid| merged_objs.get(oid).is_some_and(|m| !m.visible))
                .count(),
            "spritesMutated": n_sprite_mut,
            "spritesNamed": n_sprite_named,
            "regionRects": n_region_rect,
            "textsMutated": n_text_mut,
            "knownCmdCalls": total_known,
            "unknownCmds": unknown_list,
            "unknownGeneralCmds": unknown_general_list,
        },
    });
    let txt = serde_json::to_string_pretty(&layout)?;
    std::fs::write(out, &txt).with_context(|| format!("écriture {}", out.display()))?;

    // 7) Résumé honnête sur stdout.
    println!(
        "export-layout-runtime: screen={screen_name} objets={n_objects} (sprites statiques={n_sprites}) \
         scripts={} | MenuState: objets={} mutés[matched={n_matched_total} (statique={n_matched} \
         + onglets={n_tabs}) hidden={n_hidden} sprite={n_sprite_mut} text={n_text_mut} listItems={total_list_items}] \
         | cmds connus={total_known} inconnus_menu={} inconnus_generales={} | visibles={n_visible} -> {} ({} octets)",
        scripts.len(),
        merged_objs.len(),
        unknown_cmds.len(),
        unknown_general_cmds.len(),
        out.display(),
        txt.len()
    );
    for (name, on_open, nl, no, nk) in &script_reports {
        println!("  · {name} : on_open={on_open} layers={nl} objects={no} known_calls={nk}");
    }
    if n_matched_total == 0 {
        println!(
            "  NOTE honnête : 0 objet muté par le runtime. Le chemin driver -> MenuState -> layout \
             est CÂBLÉ et exécute les vrais scripts, mais `GetItemButtonNum` (fonction DU script) \
             lit l'état scène/save C++ que niers ne fournit pas encore => OnSetupLayer crée 0 objet. \
             Couche données runtime à compléter pour 100% comme nie.exe (cf. DESIGN.md §13)."
        );
    }
    Ok(())
}

// ── Mode menu CPU ─────────────────────────────────────────────────────────────

/// Peint le FOND du main_menu : dégradé vertical pastel cyan-très-clair (haut) → blanc (bas),
/// opaque, plein cadre. Le vrai main_menu est un fond pastel quasi-blanc plein cadre ; un canvas
/// transparent (luma 0 = noir) plombe la SSIM sur ~70 % des pixels (marges/zones vides). Peindre
/// ce fond en dur est plus robuste que de dépendre du layer de fond objbin (teinte saturée).
/// Couleurs (cf. plan rendu main_menu) : haut `#d4eef9`, bas `#ffffff`.
fn paint_menu_background(canvas_w: u32, canvas_h: u32) -> Vec<u8> {
    const TOP: [f32; 3] = [0xd4 as f32, 0xee as f32, 0xf9 as f32];
    const BOT: [f32; 3] = [0xff as f32, 0xff as f32, 0xff as f32];
    let mut canvas = vec![0u8; (canvas_w as usize) * (canvas_h as usize) * 4];
    for y in 0..canvas_h {
        // Dégradé sur les ~85 % supérieurs puis blanc plein : le bas de l'écran réel est blanc.
        let t = ((y as f32) / (canvas_h as f32 * 0.85)).min(1.0);
        let r = (TOP[0] + (BOT[0] - TOP[0]) * t).round() as u8;
        let g = (TOP[1] + (BOT[1] - TOP[1]) * t).round() as u8;
        let b = (TOP[2] + (BOT[2] - TOP[2]) * t).round() as u8;
        for x in 0..canvas_w {
            let i = ((y * canvas_w + x) * 4) as usize;
            canvas[i] = r;
            canvas[i + 1] = g;
            canvas[i + 2] = b;
            canvas[i + 3] = 255;
        }
    }
    canvas
}

/// Atlas g4tx de la rangée d'onglets-icônes du menu principal (1 texture BC7, 167 régions
/// 144×96). Repli si l'index `nom→g4tx` (généré par `--build-region-index`) est absent.
const ICON_LIST_TAB_ATLAS: &str = "#/menu/200_icon/16_icon_list_tab/<LG>/icon_list_tab.g4tx";

/// Onglets de la rangée d'icônes centrale du `main_menu`, dans l'ordre gauche→droite. État
/// `…01` = variante claire (icône blanche). Le 1ᵉʳ onglet (Match) est l'entrée surlignée du jeu.
/// L'atlas ne contient QUE le glyphe d'icône (blanc, fond transparent) : la tuile-parallélogramme
/// bleue est dessinée par le moteur, on la reproduit ici ([`fill_parallelogram`]) par fidélité.
const MAIN_MENU_ICON_TABS: [&str; 8] = [
    "icon_list_tab_battle01",
    "icon_list_tab_training01",
    "icon_list_tab_equip01",
    "icon_list_tab_quest01",
    "icon_list_tab_kizuna01",
    "icon_list_tab_tactics01",
    "icon_list_tab_town01",
    "icon_list_tab_record01",
];

/// Remplit un parallélogramme (côtés verticaux, dessus penché à droite par `slant` px) sur
/// `canvas` (`cw×ch`), en dégradé vertical OPAQUE de `top` (rangée du haut) vers `bot` (du bas).
/// `geom = (x_bottom, y_top, w, h, slant)` : `x_bottom` = abscisse du coin INFÉRIEUR gauche, le
/// coin supérieur gauche est `x_bottom+slant`. Clippe aux bords. Reproduit la tuile bleue du menu.
fn fill_parallelogram(
    canvas: &mut [u8],
    (cw, ch): (u32, u32),
    geom: (i32, i32, u32, u32, i32),
    top: [u8; 3],
    bot: [u8; 3],
) {
    let (x_bottom, y_top, w, h, slant) = geom;
    if h == 0 {
        return;
    }
    for r in 0..h as i32 {
        let cy = y_top + r;
        if cy < 0 || cy >= ch as i32 {
            continue;
        }
        // frac : 0 en haut, 1 en bas.
        let frac = if h > 1 {
            f64::from(r) / f64::from(h - 1)
        } else {
            0.0
        };
        let left = x_bottom + (f64::from(slant) * (1.0 - frac)).round() as i32;
        let col = [
            lerp_u8(top[0], bot[0], frac),
            lerp_u8(top[1], bot[1], frac),
            lerp_u8(top[2], bot[2], frac),
        ];
        for dx in 0..w as i32 {
            let cx = left + dx;
            if cx < 0 || cx >= cw as i32 {
                continue;
            }
            let d = (cy as usize * cw as usize + cx as usize) * 4;
            canvas[d] = col[0];
            canvas[d + 1] = col[1];
            canvas[d + 2] = col[2];
            canvas[d + 3] = 255;
        }
    }
}

/// Interpolation linéaire `a→b` (octets) au facteur `t∈[0,1]`.
fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    (f64::from(a) + (f64::from(b) - f64::from(a)) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// Pose la RANGÉE D'ICÔNES centrale du menu principal sur `canvas` (`cw×ch`) — FIDÉLITÉ VISUELLE.
///
/// Géométrie (tuile, slant, dégradé) calée sur la capture réelle `/tmp/real_menu.png` : 8 tuiles
/// parallélogrammes bleues penchées (top y≈386, hauteur ≈80, pas ≈114, slant ≈30 px), 1ʳᵉ tuile
/// (Match) surlignée. Pour chaque onglet ([`MAIN_MENU_ICON_TABS`]) : résout son chemin g4tx via
/// l'index `nom→g4tx` ([`load_region_index`], repli [`ICON_LIST_TAB_ATLAS`]), charge/parse l'atlas
/// via le VFS ([`obtenir_g4tx_bytes`]), décode la texture porteuse UNE fois (cache par `tex.id`,
/// l'atlas BC7 1624×1596 ne se décode pas 8 fois), rogne le rect de la région ([`crop_rgba`], rect
/// résolu par `g4tx::region`/`region_rect`), dessine la tuile bleue puis blitte le glyphe blanc
/// centré ([`scale_nearest`] + [`blit_over`]). Sprites STATIQUES, aucun driver Lua. Renvoie le
/// nombre d'onglets effectivement posés.
fn paint_main_menu_icon_row(game_dir: &Path, canvas: &mut [u8], (cw, ch): (u32, u32)) -> usize {
    // Rangée : tuiles bleues penchées, glyphe blanc centré (cf. capture réelle).
    const TILE_W: u32 = 100; // largeur de l'arête inférieure
    const TILE_H: u32 = 80;
    const SLANT: i32 = 30; // dessus décalé de 30 px à droite ⇒ parallélogramme penché
    const ROW_Y: i32 = 386; // y du bord supérieur de la rangée
    const FIRST_X: i32 = 110; // abscisse du coin inférieur gauche de la 1ʳᵉ tuile
    const STEP: i32 = 114; // pas horizontal entre tuiles
    const ICON_W: u32 = 84; // glyphe (atlas 144×96, ratio 1.5) redimensionné
    const ICON_H: u32 = 56;
    // Dégradés : tuile normale (bleu moyen) vs surlignée (1ʳᵉ, plus claire).
    const TILE_TOP: [u8; 3] = [0x4a, 0x8c, 0xd4];
    const TILE_BOT: [u8; 3] = [0x1a, 0x46, 0x96];
    const SEL_TOP: [u8; 3] = [0x7e, 0xbc, 0xf0];
    const SEL_BOT: [u8; 3] = [0x32, 0x76, 0xcc];

    let index = load_region_index();
    // Toutes les régions vivent dans le même atlas : on résout son chemin une fois (1ʳᵉ région
    // connue de l'index, sinon le chemin codé en dur).
    let g4tx_path = MAIN_MENU_ICON_TABS
        .iter()
        .find_map(|r| index.get(*r).cloned())
        .unwrap_or_else(|| ICON_LIST_TAB_ATLAS.to_string());
    let Ok((_, bytes)) = obtenir_g4tx_bytes(game_dir, &g4tx_path) else {
        warn!("main_menu : atlas d'onglets '{g4tx_path}' introuvable — rangée d'icônes ignorée");
        return 0;
    };
    let Ok(parsed) = g4tx::parse(&bytes) else {
        warn!("main_menu : parse de l'atlas d'onglets '{g4tx_path}' échoué");
        return 0;
    };

    // Cache de la texture porteuse décodée (RGBA8 plein), par `tex.id` : 1 seul décodage BC7.
    let mut decoded: std::collections::HashMap<u8, Option<(u32, u32, Vec<u8>)>> =
        std::collections::HashMap::new();
    let mut posed = 0usize;
    for (i, region) in MAIN_MENU_ICON_TABS.iter().enumerate() {
        let x_bottom = FIRST_X + i as i32 * STEP;
        let selected = i == 0;
        // 1) Tuile bleue penchée (dégradé). Toujours dessinée, même si la région échoue.
        let (top, bot) = if selected {
            (SEL_TOP, SEL_BOT)
        } else {
            (TILE_TOP, TILE_BOT)
        };
        fill_parallelogram(
            canvas,
            (cw, ch),
            (x_bottom, ROW_Y, TILE_W, TILE_H, SLANT),
            top,
            bot,
        );

        // 2) Glyphe blanc centré sur la tuile (à mi-hauteur, slant/2).
        let center_x = x_bottom + SLANT / 2 + TILE_W as i32 / 2;
        let center_y = ROW_Y + TILE_H as i32 / 2;
        let icon_x = center_x - ICON_W as i32 / 2;
        let icon_y = center_y - ICON_H as i32 / 2;

        let Some((tex, sub)) = parsed.region(region) else {
            warn!("main_menu : région '{region}' absente de l'atlas — glyphe ignoré");
            continue;
        };
        let full = decoded
            .entry(tex.id)
            .or_insert_with(|| g4tx_decode::decode_texture_rgba(&bytes, tex));
        let Some((fw, fh, full)) = full.as_ref() else {
            continue;
        };
        let Some((rw, rh, crop)) = crop_rgba(full, *fw, *fh, (sub.x, sub.y, sub.width, sub.height))
        else {
            continue;
        };
        let scaled = scale_nearest(&crop, rw, rh, ICON_W, ICON_H);
        if scaled.is_empty() {
            continue;
        }
        blit_over(
            canvas,
            (cw, ch),
            &scaled,
            (ICON_W, ICON_H),
            (icon_x, icon_y),
        );
        posed += 1;
    }
    posed
}

/// Compose l'écran `screen` via le compositeur CPU (référence pixel-perfect) → PNG.
fn cmd_menu(game_dir: &Path, screen: &str, png_out: &Path, from_setting: bool) -> Result<()> {
    // RENDU RÉEL par défaut : on compose via la DÉFINITION D'ÉCRAN (`<screen>_setting.cfg.bin`,
    // liste MENU_LAYER_INFO) — c'est la vraie composition du jeu (logo, panneaux, icônes), pas un
    // mélange de tous les objbins par préfixe. On essaie `screen` puis `screen_menu` (convention des
    // settings), enfin on retombe sur le préfixe d'objbin pour les écrans sans setting.
    let _ = from_setting; // le mode setting est désormais l'essai prioritaire dans tous les cas.
    let try_set = |name: &str| {
        build_sprite_list_from_setting(game_dir, name)
            .ok()
            .filter(|s| !s.is_empty())
    };
    let sprites = try_set(screen)
        .or_else(|| {
            (!screen.ends_with("_menu"))
                .then(|| try_set(&format!("{screen}_menu")))
                .flatten()
        })
        .or_else(|| {
            build_sprite_list(game_dir, screen.strip_suffix("_menu").unwrap_or(screen)).ok()
        })
        .unwrap_or_default();
    let n_sprites = sprites.len();

    let composite_sprites: Vec<menu::CompositeSprite> = sprites
        .iter()
        .map(|(t, w, h, rgba)| menu::CompositeSprite::neutre(rgba, *w, *h, *t, 0.5, 0.5))
        .collect();

    // Fond pastel plein cadre AVANT compositing, UNIQUEMENT pour le main_menu (son vrai fond est
    // un dégradé pastel quasi-blanc ; un canvas transparent = noir en luma → SSIM plombée). Les
    // autres écrans (ex. title02, fond = scène 3D / key-art) gardent le canvas transparent d'origine
    // pour ne pas régresser leur plancher SSIM.
    let mut canvas = if screen == "main_menu" {
        menu::compose_over(
            paint_menu_background(1280, 720),
            1280,
            720,
            &composite_sprites,
        )
    } else {
        menu::compose(1280, 720, &composite_sprites)
    };

    // Rangée d'icônes centrale (onglets parallélogrammes bleus) — UNIQUEMENT pour le main_menu, par
    // fidélité visuelle au vrai jeu (crops statiques de l'atlas `icon_list_tab`, aucun driver Lua).
    if screen == "main_menu" {
        let n = paint_main_menu_icon_row(game_dir, &mut canvas, (1280, 720));
        info!(
            "main_menu : {n}/{} onglets d'icônes posés",
            MAIN_MENU_ICON_TABS.len()
        );
    }

    let png_bytes = encoder_rgba_png(&canvas, 1280, 720)?;
    std::fs::write(png_out, &png_bytes)
        .with_context(|| format!("écriture PNG : {}", png_out.display()))?;

    println!(
        "menu: screen={screen} sprites={n_sprites} dims=1280x720 output={} size={} octets",
        png_out.display(),
        png_bytes.len()
    );

    Ok(())
}

/// `--menu <SCREEN> --window` : compose l'écran de menu (CPU) sur un canvas 1280×720
/// puis l'affiche dans une fenêtre winit **PERSISTANTE** (reste ouverte jusqu'à fermeture).
/// C'est le mode « voir le jeu à l'écran » (WSLg/Wayland).
fn cmd_menu_window(game_dir: &Path, screen: &str) -> Result<()> {
    let sprites = build_sprite_list(game_dir, screen)?;
    let n_sprites = sprites.len();
    let composite_sprites: Vec<menu::CompositeSprite> = sprites
        .iter()
        .map(|(t, w, h, rgba)| menu::CompositeSprite::neutre(rgba, *w, *h, *t, 0.5, 0.5))
        .collect();
    let canvas = menu::compose(1280, 720, &composite_sprites);
    println!(
        "menu (fenêtre) : screen={screen} sprites={n_sprites} 1280x720 — fermer la fenêtre pour quitter"
    );
    // max_frames = 0 → fenêtre persistante (cf. AppFenetre).
    cmd_window(&canvas, 1280, 720, 0)
}

// ── Mode menu GPU ─────────────────────────────────────────────────────────────

/// Vertex d'un quad sprite : position NDC + coordonnées UV.
///
/// Le quad est un quad 2D avec ancre centre (0.5, 0.5) transformé par la ScreenTransform
/// du sprite. Les positions NDC sont pré-calculées en Rust depuis les coins canvas (px).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SpriteVertex {
    /// Position NDC (x ∈ [-1,1], y ∈ [-1,1], Y-up).
    pos: [f32; 2],
    /// Coordonnée UV normalisée (0..1, origine coin supérieur gauche de la texture).
    uv: [f32; 2],
}

/// Convertit une position canvas (pixels, origine haut-gauche, 1280×720) en NDC wgpu.
///
/// Mapping :
/// - canvas (0, 0)       → NDC (-1,  1)   coin supérieur gauche
/// - canvas (1280, 720)  → NDC ( 1, -1)   coin inférieur droit
///
/// `ndc_x = canvas_x / 640 - 1`
/// `ndc_y = 1 - canvas_y / 360`
#[inline]
fn canvas_to_ndc(cx: f32, cy: f32) -> [f32; 2] {
    [cx / 640.0 - 1.0, 1.0 - cy / 360.0]
}

/// Construit les 6 vertices (2 triangles) du quad affine d'un sprite.
///
/// Reprend exactement le forward-map du compositeur CPU :
/// - `local ∈ {(0,0),(w,0),(0,h),(w,h)}` (coins du sprite dans l'espace sprite)
/// - `v = local − anchor_px`  avec `anchor_px = (0.5·w, 0.5·h)`
/// - `s = v · (scale_x, scale_y)`
/// - `r = R(rot) · s`
/// - `canvas = (x_px, y_px) + r`
///
/// UV : (0,0) pour (0,0), (1,0) pour (w,0), etc.
fn build_sprite_quad(t: &menu::ScreenTransform, w: u32, h: u32) -> [SpriteVertex; 6] {
    let (qw, qh) = (w as f32, h as f32);
    let ax = 0.5 * qw;
    let ay = 0.5 * qh;
    let (sin_r, cos_r) = (t.rot.sin(), t.rot.cos());

    // Forward map : local pixel → canvas pixel → NDC
    let fwd = |lx: f32, ly: f32| -> [f32; 2] {
        let vx = (lx - ax) * t.scale_x;
        let vy = (ly - ay) * t.scale_y;
        let cx = t.x_px + vx * cos_r - vy * sin_r;
        let cy = t.y_px + vx * sin_r + vy * cos_r;
        canvas_to_ndc(cx, cy)
    };

    // 4 coins : TL, TR, BL, BR
    let tl = SpriteVertex {
        pos: fwd(0.0, 0.0),
        uv: [0.0, 0.0],
    };
    let tr = SpriteVertex {
        pos: fwd(qw, 0.0),
        uv: [1.0, 0.0],
    };
    let bl = SpriteVertex {
        pos: fwd(0.0, qh),
        uv: [0.0, 1.0],
    };
    let br = SpriteVertex {
        pos: fwd(qw, qh),
        uv: [1.0, 1.0],
    };

    // Deux triangles CCW (cull_mode = None donc l'ordre est cosmétique)
    [tl, tr, bl, tr, br, bl]
}

/// Crée le pipeline sprite 2D avec blend premultiplié-alpha over.
///
/// Blend state (identique pour color ET alpha) :
/// - **color** : `src_factor=One, dst_factor=OneMinusSrcAlpha`
///   → `out_pm_color = pm_src + (1-a)·pm_dst`
/// - **alpha** : `src_factor=One, dst_factor=OneMinusSrcAlpha`
///   → `out_alpha = a + (1-a)·da`
///
/// Les textures doivent être pré-multipliées avant upload (`premultiply_rgba`).
/// Le render-target est dépré-multiplié après readback (`unpremultiply_rgba`) pour
/// obtenir des valeurs straight-alpha correspondant au compositeur CPU.
/// Écart final vs CPU : ≤1-2 LSB par canal (arrondi entier pré/dépré-multiplication).
fn creer_pipeline_sprite(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("menu_sprite"),
        source: wgpu::ShaderSource::Wgsl(include_str!("menu_sprite.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("sprite_pipeline_layout"),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sprite_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<SpriteVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 1,
                    },
                ],
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                // Blend premultiplié-alpha over (correct pour des textures pré-multipliées).
                //
                // Les sprites sont pré-multipliés avant upload (premultiply_rgba) et le
                // render-target est dépré-multiplié après readback (unpremultiply_rgba).
                // Cela donne le même résultat que le blend « straight-alpha over » du
                // compositeur CPU à ≤1-2 LSB près (arrondi entier lors de la pré/dépré-
                // multiplication).
                //
                // Pourquoi PAS SrcAlpha/OneMinusSrcAlpha (straight-alpha GPU naïf) ?
                // Sur un canvas initialement transparent, ce blend stocke `a·src_color`
                // dans le buffer au lieu de `src_color`, ce qui diverge de jusqu'à 255
                // pour les sprites semi-transparents vs la sortie CPU straight-alpha.
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

/// Upload RGBA8 dans une texture GPU Rgba8Unorm et retourne (texture, view).
fn upload_sprite_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sprite_tex"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        extent,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Crée un sampler bilinéaire (filtre linéaire, ClampToEdge) pour les sprites menu.
///
/// Correspond à l'échantillonnage bilinéaire du compositeur CPU (`sample_bilinear`).
fn creer_sampler_lineaire(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("linear_clamp_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    })
}

/// Pré-multiplie les canaux RGB d'un buffer RGBA8 straight-alpha par son canal alpha.
///
/// Nécessaire avant upload GPU pour un pipeline blend `(One, OneMinusSrcAlpha)`.
/// `pm_r = round(r * a / 255)`, etc. Résultat : même format RGBA8 mais RGB pré-multiplié.
fn premultiply_rgba(src: &[u8]) -> Vec<u8> {
    let mut out = src.to_vec();
    for c in out.chunks_exact_mut(4) {
        let a = c[3] as u32;
        // Arrondi au plus proche : + 127 avant la division par 255
        c[0] = ((c[0] as u32 * a + 127) / 255) as u8;
        c[1] = ((c[1] as u32 * a + 127) / 255) as u8;
        c[2] = ((c[2] as u32 * a + 127) / 255) as u8;
    }
    out
}

/// Dépré-multiplie en place les canaux RGB d'un buffer RGBA8 (résultat du blend GPU).
///
/// Appliqué après readback pour obtenir des valeurs straight-alpha comparables au
/// compositeur CPU de référence.
/// Si `a = 0` : laisse RGB inchangé (pixel transparent, valeur RGB non significative).
fn unpremultiply_rgba(pixels: &mut [u8]) {
    for c in pixels.chunks_exact_mut(4) {
        let a = c[3] as u32;
        // Checked division : a=0 laisse RGB inchangé (pixel transparent, RGB non significatif).
        // Arrondi au plus proche : + a/2 avant la division par a.
        let div = |v: u8| -> u8 {
            (v as u32 * 255 + a / 2)
                .checked_div(a)
                .unwrap_or(0)
                .min(255) as u8
        };
        c[0] = div(c[0]);
        c[1] = div(c[1]);
        c[2] = div(c[2]);
    }
}

/// Compare pixel-à-pixel deux images RGBA8 de mêmes dimensions.
///
/// Retourne `(max_diff_canal, pct_pixels_within_4)` où :
/// - `max_diff_canal` : différence absolue maximale sur tous les canaux RGBA de tous les pixels
/// - `pct_pixels_within_4` : pourcentage de pixels dont la différence max canal ≤ 4/255
fn comparer_cpu_gpu(cpu: &[u8], gpu: &[u8], width: u32, height: u32) -> (u8, f64) {
    assert_eq!(cpu.len(), gpu.len());
    assert_eq!(cpu.len(), (width as usize) * (height as usize) * 4);

    let total_pixels = (width as usize) * (height as usize);
    let mut max_diff: u8 = 0;
    let mut pixels_ok: usize = 0;

    for i in (0..cpu.len()).step_by(4) {
        let d = (0usize..4)
            .map(|c| cpu[i + c].abs_diff(gpu[i + c]))
            .max()
            .unwrap_or(0);
        if d > max_diff {
            max_diff = d;
        }
        if d <= 4 {
            pixels_ok += 1;
        }
    }

    let pct = 100.0 * pixels_ok as f64 / total_pixels as f64;
    (max_diff, pct)
}

/// Rend l'écran `screen` sur GPU (offscreen wgpu 1280×720 Rgba8Unorm) → PNG.
///
/// Si `verify` est vrai, compare le résultat GPU avec le compositeur CPU de référence
/// et échoue si moins de 99 % des pixels sont dans une tolérance de 4/255 par canal.
fn cmd_menu_gpu(game_dir: &Path, screen: &str, png_out: &Path, verify: bool) -> Result<()> {
    info!("mode GPU menu : écran '{}' → {}", screen, png_out.display());

    // ── 1. Sprites (transform + w + h + rgba), triés back-to-front ───────────
    let sprites = build_sprite_list(game_dir, screen)?;
    let n_sprites = sprites.len();
    info!("sprites chargés : {n_sprites}");

    // ── 2. Infrastructure wgpu ────────────────────────────────────────────────
    let instance = gpu_select::instance();
    let adapter = demander_adaptateur_hors_ecran(&instance)?;
    info!("adaptateur GPU menu : {:?}", adapter.get_info());
    let (device, queue) = creer_device(&adapter)?;

    // ── 3. Render target 1280×720 Rgba8Unorm ─────────────────────────────────
    const CW: u32 = 1280;
    const CH: u32 = 720;
    let canvas_extent = wgpu::Extent3d {
        width: CW,
        height: CH,
        depth_or_array_layers: 1,
    };
    let render_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("menu_rt"),
        size: canvas_extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let render_view = render_tex.create_view(&wgpu::TextureViewDescriptor::default());

    // ── 4. Pipeline sprite + ressources partagées ─────────────────────────────
    let bgl = creer_bgl(&device);
    let pipeline = creer_pipeline_sprite(&device, &bgl, wgpu::TextureFormat::Rgba8Unorm);
    let linear_sampler = creer_sampler_lineaire(&device);

    // ── 5. Upload textures + vertex buffers (avant le render pass) ────────────
    //
    // Toutes les ressources GPU sont créées ici pour pouvoir les emprunter dans
    // le render pass sans conflits de lifetime.
    struct SpriteDrawData {
        _tex: wgpu::Texture, // maintenu vivant pour que le BindGroup soit valide
        bind_group: wgpu::BindGroup,
        vbuf: wgpu::Buffer,
    }

    let draw_data: Vec<SpriteDrawData> = sprites
        .iter()
        .map(|(transform, w, h, rgba)| {
            // Pré-multiplication avant upload : nécessaire pour le blend (One, OneMinusSrcAlpha).
            // La dépré-multiplication après readback restaure les valeurs straight-alpha
            // comparables au compositeur CPU de référence.
            let pm = premultiply_rgba(rgba);
            let (tex, view) = upload_sprite_texture(&device, &queue, &pm, *w, *h);
            let bind_group = creer_bind_group(&device, &bgl, &view, &linear_sampler);
            let quad = build_sprite_quad(transform, *w, *h);
            let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sprite_vbuf"),
                contents: bytemuck::cast_slice(&quad),
                usage: wgpu::BufferUsages::VERTEX,
            });
            SpriteDrawData {
                _tex: tex,
                bind_group,
                vbuf,
            }
        })
        .collect();

    // ── 6. Rendu back-to-front ────────────────────────────────────────────────
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("menu_gpu_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("menu_gpu_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &render_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Canvas initialisé transparent (identique au CPU : vec![0u8; …])
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        for sd in &draw_data {
            pass.set_bind_group(0, &sd.bind_group, &[]);
            pass.set_vertex_buffer(0, sd.vbuf.slice(..));
            pass.draw(0..6, 0..1);
        }
    } // render pass libéré ici → encodeur de nouveau disponible

    // ── 7. Readback ───────────────────────────────────────────────────────────
    const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bpr = (4 * CW).div_ceil(ALIGN) * ALIGN;
    let buf_size = (padded_bpr * CH) as u64;

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("menu_gpu_readback"),
        size: buf_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        render_tex.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bpr),
                rows_per_image: Some(CH),
            },
        },
        canvas_extent,
    );

    queue.submit([encoder.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

    // ── 8a. Lecture readback (valeurs encore pré-multipliées) ────────────────
    let gpu_pixels_pm: Vec<u8> = {
        let mapped = readback.slice(..).get_mapped_range();
        let unpadded = (4 * CW) as usize;
        let padded = padded_bpr as usize;
        let mut px = Vec::with_capacity(unpadded * CH as usize);
        for row in 0..CH as usize {
            px.extend_from_slice(&mapped[row * padded..row * padded + unpadded]);
        }
        drop(mapped);
        readback.unmap();
        px
    };

    // ── 8b. Vérification CPU vs GPU (optionnelle) ─────────────────────────────
    //
    // Comparaison en espace PRÉ-MULTIPLIÉ des deux côtés :
    //   GPU PM  = pixels tels que lus depuis le buffer (sortie blend One/1-srcA)
    //   CPU PM  = premultiply_rgba(compose(sprites))
    //
    // Pourquoi PM et non straight-alpha ?
    //   La dépré-multiplication des valeurs GPU vers straight-alpha amplifie les
    //   erreurs d'arrondi pour les pixels quasi-transparents (alpha 1..7) :
    //   round(1 * 255 / 1) = 255 vs CPU straight 200 → diff = 55, alors que la
    //   contribution visuelle de ce pixel est quasi nulle (α/255 ≈ 0%).
    //   En PM, les deux valent round(200 * 1/255) = 1 → diff = 0.
    //   La tolérance 4/255 est ainsi vérifiée sur les valeurs VISUELLEMENT significatives.
    if verify {
        let cpu_sprites: Vec<menu::CompositeSprite> = sprites
            .iter()
            .map(|(t, w, h, rgba)| menu::CompositeSprite::neutre(rgba, *w, *h, *t, 0.5, 0.5))
            .collect();
        let cpu_pixels_straight = menu::compose(CW, CH, &cpu_sprites);
        let cpu_pixels_pm = premultiply_rgba(&cpu_pixels_straight);

        let (max_diff, pct_ok) = comparer_cpu_gpu(&cpu_pixels_pm, &gpu_pixels_pm, CW, CH);

        // Tailles PNG : CPU straight, GPU straight (dépré-mult pour le PNG de sortie)
        let cpu_png_sz = encoder_rgba_png(&cpu_pixels_straight, CW, CH)?.len();
        let mut gpu_straight_tmp = gpu_pixels_pm.clone();
        unpremultiply_rgba(&mut gpu_straight_tmp);
        let gpu_png_sz = encoder_rgba_png(&gpu_straight_tmp, CW, CH)?.len();

        println!("=== CPU vs GPU verification (screen={screen}) ===");
        println!("  comparaison      : valeurs pré-multipliées (PM) — évite l'amplification");
        println!("                     d'erreur sur pixels quasi-transparents (alpha<8)");
        println!("  max channel diff : {max_diff}/255");
        println!("  pixels within 4  : {pct_ok:.3}%  (seuil ≥99%)");
        println!("  CPU PNG size     : {cpu_png_sz} octets");
        println!("  GPU PNG size     : {gpu_png_sz} octets");

        if pct_ok >= 99.0 {
            println!("  PASS");
        } else {
            anyhow::bail!(
                "GPU/CPU divergence trop élevée : {pct_ok:.3}% pixels dans tolérance 4/255 \
                 (requis ≥99%) — vérifier le transform NDC et le blend state"
            );
        }
    }

    // ── 8c. Dépré-multiplication pour PNG straight-alpha ─────────────────────
    //
    // Le buffer GPU contient des valeurs pré-multipliées. On restaure straight-alpha
    // pour écrire un PNG standard (convention PNG = straight-alpha).
    // Pour les pixels quasi-transparents (alpha 1..7), la dépré-multiplication introduit
    // jusqu'à quelques LSB d'erreur sur les canaux RGB — acceptable car leur contribution
    // visuelle est ≤ 3% (α/255).
    let mut gpu_pixels = gpu_pixels_pm;
    unpremultiply_rgba(&mut gpu_pixels);

    // ── 9. Écriture PNG ───────────────────────────────────────────────────────
    let png_bytes = encoder_rgba_png(&gpu_pixels, CW, CH)?;
    std::fs::write(png_out, &png_bytes)
        .with_context(|| format!("écriture PNG : {}", png_out.display()))?;

    info!(
        "GPU menu : {}  {}x{}  {} sprites  {} octets PNG",
        png_out.display(),
        CW,
        CH,
        n_sprites,
        png_bytes.len()
    );
    println!(
        "menu-gpu: screen={screen} sprites={n_sprites} dims={CW}x{CH} \
         output={} size={} octets",
        png_out.display(),
        png_bytes.len()
    );

    Ok(())
}

// ── Implémentation ApplicationHandler winit ───────────────────────────────────

impl winit::application::ApplicationHandler for AppFenetre {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let attrs = winit::window::Window::default_attributes()
            .with_title(if self.jeu.is_some() {
                "niers — Inazuma Eleven: Victory Road"
            } else {
                "nie-game — IEVR texture viewer"
            })
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.tex_width,
                self.tex_height,
            ));

        let fenetre = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                error!("création fenêtre : {e}");
                self.erreur = Some(anyhow::anyhow!("création fenêtre : {e}"));
                event_loop.exit();
                return;
            }
        };

        match self.creer_etat(Arc::clone(&fenetre)) {
            Ok(etat) => {
                self.etat = Some(etat);
            }
            Err(e) => {
                error!("initialisation wgpu fenêtré : {e}");
                self.erreur = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(taille) => {
                if let Some(etat) = &mut self.etat {
                    etat.redimensionner(taille);
                }
            }
            // Mode jouable : chaque touche devient une commande de menu IEVR, que la FSM du cœur
            // interprète selon l'écran courant. Seuls les appuis comptent — une touche maintenue
            // ne doit pas faire défiler un menu à la vitesse de la répétition clavier.
            WindowEvent::KeyboardInput { event, .. } if self.jeu.is_some() => {
                let winit::keyboard::PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                debug!(
                    "touche {code:?} état={:?} repeat={}",
                    event.state, event.repeat
                );
                // L'état des touches est suivi en continu : c'est lui qui dirige le joueur
                // pendant un match, là où les menus réagissent aux appuis.
                if let Some(jeu) = &mut self.jeu {
                    if event.state.is_pressed() {
                        jeu.enfoncees.insert(code);
                    } else {
                        jeu.enfoncees.remove(&code);
                    }
                }
                if !event.state.is_pressed() || event.repeat {
                    return;
                }
                // V bascule la caméra du match : vue de dessus (la référence, sans assets) ou
                // perspective 3D avec les maillages du jeu.
                if code == winit::keyboard::KeyCode::KeyV
                    && let Some(jeu) = &mut self.jeu
                {
                    jeu.vue3d = !jeu.vue3d;
                    info!("vue {}", if jeu.vue3d { "3D" } else { "de dessus" });
                    return;
                }
                if let Some(jeu) = &mut self.jeu {
                    // En match, les directions PILOTENT le joueur et ne doivent pas être aussi
                    // lues comme des commandes de menu : elles y sont sans effet, mais les
                    // confondre brouillerait le journal et coûterait un appel inutile.
                    let en_match = jeu.ecran.in_match();
                    if let Some(cmd) = touche_vers_commande(code)
                        && !(en_match && cmd.starts_with("CMD_FCS"))
                    {
                        jeu.ecran.input(cmd);
                        // Un onglet vient de s'ouvrir : si le VFS sait le remplir, on remplace
                        // le message « en cours d'intégration » par les vraies données. Le
                        // chargement est natif — la FSM, partagée avec le web, ne peut pas le
                        // faire elle-même.
                        if let Some(titre) = jeu.ecran.info_title().map(str::to_owned) {
                            let lignes = jeu.lignes_onglet(&titre);
                            if !lignes.is_empty() {
                                info!("{titre} : {} lignes chargées", lignes.len());
                            }
                            jeu.ecran.fournir_liste(lignes);
                        }
                        // Mode Histoire : remplacer la scène de démonstration par un vrai
                        // dialogue du jeu, dès que l'écran s'ouvre.
                        if jeu.ecran.attend_dialogue()
                            && let Some((id, lignes)) = jeu.dialogue()
                        {
                            info!("dialogue {id} : {} répliques", lignes.len());
                            jeu.ecran.fournir_dialogue(id, lignes);
                        }
                        info!("{cmd} → {}", decrire_ecran(&jeu.ecran));
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                // Avancer le jeu AVANT de dessiner : la physique du match tourne sur le temps
                // réellement écoulé, pas sur un pas fixe qu'un décrochage rendrait faux.
                if let (Some(jeu), Some(etat)) = (&mut self.jeu, &self.etat) {
                    let dt = jeu.dernier.elapsed().as_secs_f32();
                    jeu.dernier = std::time::Instant::now();
                    // Commandes de jeu AVANT la simulation : le joueur bouge sur cette image, pas
                    // sur la suivante. Espace frappe le ballon (Entrée aussi, pour la cohérence
                    // avec « valider » ailleurs).
                    let (dx, dy) = jeu.direction();
                    let tir = jeu.enfoncees.contains(&winit::keyboard::KeyCode::Space)
                        || jeu.enfoncees.contains(&winit::keyboard::KeyCode::Enter);
                    jeu.ecran.set_game_input(dx, dy, tir);
                    // Borne haute : après une pause (fenêtre déplacée, veille), un `dt` de
                    // plusieurs secondes téléporterait le ballon au lieu de le faire avancer.
                    jeu.ecran.update(dt.min(0.1));
                    let score = jeu.ecran.score();
                    if score != jeu.dernier_score {
                        info!("score {}-{}", score[0], score[1]);
                        jeu.dernier_score = score;
                    }
                    let frame = jeu.image();
                    etat.televerser(&frame, self.tex_width, self.tex_height);
                }
                if let Some(etat) = &mut self.etat {
                    match etat.rendre() {
                        Ok(()) => {
                            self.frames_rendues += 1;
                            // max_frames == 0 → fenêtre PERSISTANTE (ferme uniquement sur
                            // CloseRequested). Sinon auto-exit après N trames (mode capture/CI).
                            if self.max_frames != 0 && self.frames_rendues >= self.max_frames {
                                info!("auto-exit après {} trames", self.frames_rendues);
                                event_loop.exit();
                            }
                        }
                        Err(e) => {
                            error!("rendu : {e}");
                            self.erreur = Some(e);
                            event_loop.exit();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(etat) = &self.etat {
            etat.fenetre.request_redraw();
        }
    }
}

impl AppFenetre {
    /// Crée l'état wgpu lié à la fenêtre (surface, device, pipeline, bind group).
    fn creer_etat(&self, fenetre: Arc<winit::window::Window>) -> Result<EtatFenetre> {
        let surface = self
            .instance
            .create_surface(Arc::clone(&fenetre))
            .context("création surface wgpu")?;

        let adapter =
            pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: gpu_select::preference_puissance(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }))
            .context("aucun adaptateur wgpu compatible avec la surface")?;

        info!("adaptateur fenêtré : {}", gpu_select::decrire(&adapter));

        let (device, queue) = creer_device(&adapter)?;

        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let taille = fenetre.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: taille.width.max(1),
            height: taille.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let bgl = creer_bgl(&device);
        let pipeline = creer_pipeline(&device, &bgl, surface_format);
        let (texture, view, sampler) =
            charger_gpu_texture(&device, &queue, &self.rgba, self.tex_width, self.tex_height);
        let bind_group = creer_bind_group(&device, &bgl, &view, &sampler);

        Ok(EtatFenetre {
            fenetre,
            instance: self.instance.clone(),
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            texture,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{blit_over, crop_rgba, scale_nearest};

    /// Buffer 4×2 RGBA où chaque pixel encode son index dans le canal R, pour vérifier que
    /// `crop_rgba` extrait exactement le bon sous-rectangle (ligne par ligne, sans débordement).
    fn ramp(w: u32, h: u32) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for i in 0..(w * h) {
            v.extend_from_slice(&[i as u8, 0, 0, 255]);
        }
        v
    }

    #[test]
    fn crop_rgba_extrait_le_bon_sous_rect() {
        // 4×2 : indices 0..7 (ligne 0 : 0 1 2 3 ; ligne 1 : 4 5 6 7).
        let full = ramp(4, 2);
        // Crop 2×2 à (1,0) → pixels d'index 1,2 (ligne 0) puis 5,6 (ligne 1).
        let (w, h, out) = crop_rgba(&full, 4, 2, (1, 0, 2, 2)).expect("crop valide");
        assert_eq!((w, h), (2, 2));
        let reds: Vec<u8> = out.chunks_exact(4).map(|p| p[0]).collect();
        assert_eq!(reds, [1, 2, 5, 6]);
    }

    #[test]
    fn scale_nearest_double_et_reduit() {
        // 2×1 : rouge (255,0,0) puis vert (0,255,0).
        let src = vec![255, 0, 0, 255, 0, 255, 0, 255];
        // Double en largeur → 4×1 : R R V V.
        let up = scale_nearest(&src, 2, 1, 4, 1);
        let reds: Vec<u8> = up.chunks_exact(4).map(|p| p[0]).collect();
        let greens: Vec<u8> = up.chunks_exact(4).map(|p| p[1]).collect();
        assert_eq!(reds, [255, 255, 0, 0]);
        assert_eq!(greens, [0, 0, 255, 255]);
        // Dimensions nulles → vide (jamais de panique).
        assert!(scale_nearest(&src, 2, 1, 0, 1).is_empty());
    }

    #[test]
    fn blit_over_alpha_et_clip() {
        // Canevas 3×1 noir opaque.
        let mut canvas = vec![0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255];
        // Source 2×1 : blanc semi-transparent (a=128) puis blanc opaque.
        let src = vec![255, 255, 255, 128, 255, 255, 255, 255];
        // Pose à x=1 → couvre pixels 1 et 2.
        blit_over(&mut canvas, (3, 1), &src, (2, 1), (1, 0));
        // Pixel 0 inchangé (noir).
        assert_eq!(&canvas[0..4], &[0, 0, 0, 255]);
        // Pixel 1 : blanc 50% sur noir → ~128.
        assert_eq!(canvas[4], 128);
        // Pixel 2 : blanc opaque → 255.
        assert_eq!(&canvas[8..12], &[255, 255, 255, 255]);
        // Pixel entièrement transparent ne modifie rien : re-blit a=0.
        let before = canvas.clone();
        let transp = vec![9, 9, 9, 0];
        blit_over(&mut canvas, (3, 1), &transp, (1, 1), (0, 0));
        assert_eq!(canvas, before);
        // Hors bornes (dx au-delà du canevas) : pas de panique, pas d'effet.
        blit_over(&mut canvas, (3, 1), &src, (2, 1), (10, 0));
        assert_eq!(canvas, before);
    }

    #[test]
    fn crop_rgba_rejette_hors_bornes_et_degenere() {
        let full = ramp(4, 2);
        // Déborde en largeur (x+w > 4).
        assert!(crop_rgba(&full, 4, 2, (3, 0, 2, 1)).is_none());
        // Déborde en hauteur.
        assert!(crop_rgba(&full, 4, 2, (0, 1, 1, 2)).is_none());
        // Dégénéré (w ≤ 0) et coordonnée négative.
        assert!(crop_rgba(&full, 4, 2, (0, 0, 0, 1)).is_none());
        assert!(crop_rgba(&full, 4, 2, (-1, 0, 2, 1)).is_none());
        // Pleine surface : OK.
        assert!(crop_rgba(&full, 4, 2, (0, 0, 4, 2)).is_some());
    }
}
