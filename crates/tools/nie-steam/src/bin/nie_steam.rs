//! CLI `nie-steam` — acquisition Steam native pour IEVR (app 2799860).
//!
//! Modélé sur `steamroom-cli`. Sous-commandes :
//! - `list <appId>` : inspecte depots/tailles sans télécharger
//! - `download <appId>` : télécharge vers --output
//! - `sync` : download IEVR (app 2799860) vers --output
//!
//! Token store par défaut : `~/.local/share/niers/steam-tokens.json`.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use nie_steam::IEVR_STEAM_APP_ID;
use nie_steam::downloader::SteamDepotDownloader;
use nie_steam::options::SteamDownloadOptions;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "nie-steam",
    about = "Acquisition Steam native pour niers (IEVR app 2799860)",
    version
)]
struct Cli {
    /// Compte Steam (laissez vide pour login anonyme).
    #[arg(short = 'u', long, env = "STEAM_USER")]
    username: Option<String>,

    /// Mot de passe Steam (ignoré si refresh token en cache).
    #[arg(short = 'p', long, env = "STEAM_PASSWORD")]
    password: Option<String>,

    /// Branche Steam (défaut : public).
    #[arg(short = 'b', long, default_value = "public")]
    branch: String,

    /// OS ciblé pour le filtrage des depots (windows/macos/linux).
    #[arg(long, default_value = "windows")]
    os: String,

    /// Architecture ciblée (64/32).
    #[arg(long)]
    arch: Option<String>,

    /// Langue ciblée pour les depots localisés.
    #[arg(long, default_value = "english")]
    language: String,

    /// Télécharger tous les depots sans filtrer par OS/arch/langue.
    #[arg(long)]
    all_platforms: bool,

    /// IDs de depots explicites (répétable).
    #[arg(long = "depot", value_name = "ID")]
    depot_ids: Vec<u32>,

    /// Parallélisme de téléchargement (chunks concurrents).
    #[arg(long, default_value_t = 16)]
    max_downloads: usize,

    /// Désactive la vérification par fichier (taille + SHA) : retélécharge tout.
    /// Par défaut, les fichiers déjà à jour sont sautés (MAJ en place).
    #[arg(long)]
    no_verify: bool,

    /// Chemin du token store JSON.
    #[arg(long, env = "NIE_STEAM_TOKEN_STORE")]
    token_store: Option<PathBuf>,

    /// Secondes sans aucun événement avant de déclarer le depot bloqué (0 = désactivé).
    ///
    /// Doit rester supérieur au temps de vérification du plus gros fichier
    /// (~90 s pour le CPK de 4,26 Gio d'IEVR), sinon la passe `verify` déclenche
    /// un faux positif.
    #[arg(long, default_value_t = 300)]
    stall_timeout: u64,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Inspecte les depots (tailles, fichiers) sans télécharger.
    List {
        /// App ID Steam à inspecter.
        #[arg(default_value_t = IEVR_STEAM_APP_ID)]
        app_id: u32,
    },

    /// Télécharge une app Steam vers le répertoire cible.
    Download {
        /// App ID Steam à télécharger.
        #[arg(default_value_t = IEVR_STEAM_APP_ID)]
        app_id: u32,

        /// Répertoire de destination.
        #[arg(short = 'o', long, default_value = ".")]
        output: PathBuf,
    },

    /// Télécharge l'app IEVR (2799860) — raccourci pour `download 2799860`.
    Sync {
        /// Répertoire de destination.
        #[arg(short = 'o', long, default_value = ".")]
        output: PathBuf,
    },

    /// Confronte le manifest à une install existante sans rien écrire.
    ///
    /// Nomme les entrées qui feraient échouer le download sur
    /// `Is a directory (os error 21)` — erreur que Steam remonte sans dire
    /// quel chemin la provoque, et qui avorte le depot entier.
    Audit {
        /// App ID Steam à auditer.
        #[arg(default_value_t = IEVR_STEAM_APP_ID)]
        app_id: u32,

        /// Répertoire de l'install à confronter au manifest.
        #[arg(short = 'o', long, default_value = ".")]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            // `steamroom_client` DOIT figurer explicitement : la directive
            // `steamroom=info` porte sur le crate `steamroom` seul et ne couvre
            // pas `steamroom_client`. Sans lui, les avertissements qui expliquent
            // un blocage — « all CDN servers in cooldown, waiting », « CDN rate
            // limited, backing off » — sont filtrés, et un téléchargement figé
            // ressemble à un téléchargement lent. C'est ce qui a rendu le gel du
            // 2026-08-15 muet pendant tout son déroulement.
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("nie_steam=info,steamroom=info,steamroom_client=info")
            }),
        )
        .init();

    let cli = Cli::parse();

    let token_store_path = cli
        .token_store
        .unwrap_or_else(nie_steam::token_store::default_path);

    let make_opts = |app_id: u32, output: PathBuf| -> SteamDownloadOptions {
        SteamDownloadOptions {
            app_id,
            install_dir: output,
            username: cli.username.clone(),
            password: cli.password.clone(),
            branch: cli.branch.clone(),
            beta_password: None,
            os: cli.os.clone(),
            arch: cli.arch.clone(),
            language: cli.language.clone(),
            all_platforms: cli.all_platforms,
            depot_ids: cli.depot_ids.clone(),
            max_downloads: cli.max_downloads,
            verify: !cli.no_verify,
            token_store_path: Some(token_store_path.clone()),
            guard_provider: None,
            stall_timeout: (cli.stall_timeout > 0)
                .then(|| std::time::Duration::from_secs(cli.stall_timeout)),
        }
    };

    let dl = SteamDepotDownloader::new();

    match cli.command {
        Commands::List { app_id } => {
            let opts = make_opts(app_id, PathBuf::from("."));
            let infos = dl.list_depots(&opts).await?;
            if infos.is_empty() {
                println!("app_id={app_id} depots=0");
            } else {
                for info in &infos {
                    let name = info.name.as_deref().unwrap_or("(sans nom)");
                    let mb = info.total_bytes as f64 / (1024.0 * 1024.0);
                    println!(
                        "depot={} name={name} manifest={} files={} size={:.1}MB",
                        info.depot_id, info.manifest_id, info.file_count, mb
                    );
                }
            }
        }

        Commands::Download { app_id, output } => {
            let opts = make_opts(app_id, output);
            let result = dl.download_app(&opts, None).await;
            if result.success {
                println!(
                    "ok app_id={} depots={} files={} bytes={} duration={:.1}s",
                    result.app_id,
                    result.depots.len(),
                    result.file_count,
                    result.downloaded_bytes,
                    result.duration.as_secs_f64()
                );
            } else {
                let err = result.error.as_deref().unwrap_or("erreur inconnue");
                eprintln!("error={err}");
                std::process::exit(1);
            }
        }

        Commands::Sync { output } => {
            let opts = make_opts(IEVR_STEAM_APP_ID, output);
            let result = dl.download_app(&opts, None).await;
            if result.success {
                println!(
                    "ok app_id={} depots={} files={} bytes={} duration={:.1}s",
                    result.app_id,
                    result.depots.len(),
                    result.file_count,
                    result.downloaded_bytes,
                    result.duration.as_secs_f64()
                );
            } else {
                let err = result.error.as_deref().unwrap_or("erreur inconnue");
                eprintln!("error={err}");
                std::process::exit(1);
            }
        }

        Commands::Audit { app_id, output } => {
            let opts = make_opts(app_id, output);
            let collisions = dl.audit_manifest(&opts).await?;
            if collisions.is_empty() {
                println!("ok app_id={app_id} collisions=0 — manifest applicable tel quel");
            } else {
                for c in &collisions {
                    println!(
                        "collision depot={} flags={} size={} chunks={} raison={} chemin={}",
                        c.depot_id,
                        c.flags,
                        c.size,
                        c.chunk_count,
                        c.reason.label(),
                        if c.path.is_empty() { "(vide)" } else { &c.path }
                    );
                }
                println!("collisions={}", collisions.len());
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
