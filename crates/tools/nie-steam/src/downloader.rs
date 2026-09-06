//! Orchestration du téléchargement de contenu Steam (depots → disque).
//!
//! Port de `IECODE.Core/Steam/Content/SteamDepotDownloader.cs` (C#, SteamKit2)
//! réécrit sur la fondation `steamroom` / `steamroom-client`.
//!
//! Flux de `download_app` :
//! 1. Connexion + login via [`SteamSession::connect`]
//! 2. Récupération de l'app info (PICS) + résolution des depots ([`depot_resolver`])
//! 3. Pour chaque depot éligible : manifest request code + clé + CDN server
//! 4. [`DepotJob`] steamroom-client pour le téléchargement parallèle + vérification
//!
//! `list_depots` effectue les étapes 1–3 sans télécharger le contenu.
//! `get_build_id` ne fait que login + app info (très rapide).
//!
//! # Note live-Steam
//! Toutes les méthodes async font de vrais appels réseau vers l'infrastructure Steam.
//! Elles compilent et sont correctement structurées. Un E2E complet nécessite des
//! creds Steam valides, l'accès à l'app et une connexion réseau.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use steamroom::cdn::{CdnClient, CdnServerPool};
use steamroom::depot::manifest::DepotManifest;
use steamroom::depot::{DepotId, ManifestId};
use steamroom_client::download::{CdnChunkFetcher, DepotJob};
use steamroom_client::event::DownloadEvent;
use steamroom_client::manifest::ManifestCache;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::depot_resolver::{self, INVALID_MANIFEST};
use crate::options::{
    CollisionReason, DepotInfo, DownloadResult, ManifestCollision, SteamDownloadOptions,
    SteamDownloadPhase, SteamDownloadProgress,
};
use crate::session::SteamSession;

// ─── Downloader ───────────────────────────────────────────────────────────────

/// Téléchargeur natif de contenu Steam — port de `SteamDepotDownloader` (C#).
///
/// Instancier via [`SteamDepotDownloader::new`]. Toutes les méthodes sont `async`.
pub struct SteamDepotDownloader;

impl SteamDepotDownloader {
    /// Crée un nouveau downloader (sans état propre — les sessions sont créées à la demande).
    pub fn new() -> Self {
        Self
    }

    /// Télécharge l'app Steam décrite par `opts` vers `opts.install_dir`.
    ///
    /// Équivalent de `SteamDepotDownloader.DownloadAppAsync` (C#).
    ///
    /// # Progression
    /// Si `progress` est fourni, des événements de progression sont envoyés via le canal.
    /// La progression détaillée (fichier par fichier) est disponible via les [`DownloadEvent`]
    /// steamroom-client (branchez un `mpsc::UnboundedReceiver<DownloadEvent>` via `event_sender`).
    pub async fn download_app(
        &self,
        opts: &SteamDownloadOptions,
        progress: Option<mpsc::UnboundedSender<SteamDownloadProgress>>,
    ) -> DownloadResult {
        let start = Instant::now();
        let install_dir = opts.install_dir.clone();
        let app_id = opts.app_id;

        match self.download_app_inner(opts, progress).await {
            Ok(result) => result,
            Err(e) => DownloadResult::err(app_id, install_dir, &e.to_string(), start.elapsed()),
        }
    }

    async fn download_app_inner(
        &self,
        opts: &SteamDownloadOptions,
        progress: Option<mpsc::UnboundedSender<SteamDownloadProgress>>,
    ) -> Result<DownloadResult> {
        let start = Instant::now();

        emit(
            &progress,
            SteamDownloadPhase::Connecting,
            "Connexion à Steam…",
            0,
            0,
        );
        let mut session = SteamSession::connect(opts)
            .await
            .context("connexion Steam")?;

        emit(
            &progress,
            SteamDownloadPhase::FetchingAppInfo,
            "Récupération app info…",
            0,
            0,
        );
        session.request_app_info(opts.app_id, false).await?;

        let app_kv = session.app_kv(opts.app_id).ok_or_else(|| {
            anyhow::anyhow!(
                "Impossible de récupérer l'app info {} (compte sans accès ?)",
                opts.app_id
            )
        })?;

        let depots_kv = app_kv
            .get("depots")
            .ok_or_else(|| anyhow::anyhow!("app info {} : section 'depots' absente", opts.app_id))?
            .clone();

        emit(
            &progress,
            SteamDownloadPhase::FetchingManifests,
            "Résolution des depots…",
            0,
            0,
        );

        // Résout les depots éligibles via le resolver (logique pure, identique C#).
        let depot_ids = depot_resolver::select_depot_ids(
            &depots_kv,
            &opts.depot_ids,
            opts.all_platforms,
            &opts.os,
            opts.arch.as_deref(),
            &opts.language,
        );

        if depot_ids.is_empty() {
            anyhow::bail!(
                "Aucun depot éligible pour l'app {} (branche '{}', os '{}')",
                opts.app_id,
                opts.branch,
                opts.os
            );
        }

        // Pour chaque depot : résoudre manifest + clé.
        let plans = build_depot_plans(&mut session, opts, &depot_ids, &depots_kv).await?;

        if plans.is_empty() {
            anyhow::bail!("Aucun depot téléchargeable (accès/manifest/clé manquants).");
        }

        // CDN servers.
        let cdn_servers = session
            .cdn_servers(0, Some(20))
            .await
            .context("CDN servers")?;
        if cdn_servers.is_empty() {
            anyhow::bail!("Aucun serveur CDN Steam disponible.");
        }
        let cdn_host = cdn_servers[0].host.clone();

        std::fs::create_dir_all(&opts.install_dir)
            .with_context(|| format!("créer {}", opts.install_dir.display()))?;

        let manifest_cache = ManifestCache::new(ManifestCache::default_path());
        let cdn = CdnClient::new().context("CdnClient")?;

        let mut total_downloaded: u64 = 0;
        let mut total_files: u64 = 0;
        let mut completed_depots: Vec<u32> = Vec::new();

        emit(
            &progress,
            SteamDownloadPhase::Downloading,
            "Téléchargement en cours…",
            0,
            0,
        );

        for plan in &plans {
            let cdn_auth_token = session
                .cdn_auth_token(plan.app_id, plan.depot_id, &cdn_host)
                .await;

            let request_code = session
                .manifest_request_code(plan.app_id, plan.depot_id, plan.manifest_id, &plan.branch)
                .await;

            // Manifest : cache ou CDN.
            let manifest_bytes = load_or_fetch_manifest(
                &cdn,
                &cdn_servers[0],
                plan,
                request_code,
                cdn_auth_token.as_deref(),
                &manifest_cache,
            )
            .await?;

            let mut manifest = DepotManifest::parse(&manifest_bytes).context("parse manifest")?;
            if manifest.filenames_encrypted {
                let _ = manifest.decrypt_filenames(&plan.depot_key);
            }

            // Les entrées répertoire portent `flags=64`, que steamroom-client ne
            // reconnaît pas (cf. STEAM_FLAG_DIRECTORY) : laissées dans le manifest,
            // elles font échouer le depot entier sur `Is a directory (os error 21)`.
            // On honore leur sémantique ici — créer le répertoire — puis on les
            // retire, ne laissant que de vrais fichiers au DepotJob.
            let dir_count = {
                let before = manifest.files.len();
                for f in manifest
                    .files
                    .iter()
                    .filter(|f| is_manifest_directory(f.flags))
                {
                    let path = opts.install_dir.join(f.normalized_path());
                    std::fs::create_dir_all(&path)
                        .with_context(|| format!("créer le répertoire {}", path.display()))?;
                }
                manifest.files.retain(|f| !is_manifest_directory(f.flags));
                before - manifest.files.len()
            };
            if dir_count > 0 {
                info!(
                    depot = plan.depot_id,
                    directories = dir_count,
                    "entrées répertoire créées puis retirées du manifest"
                );
            }

            let (event_tx, mut event_rx) = mpsc::unbounded_channel::<DownloadEvent>();

            // DepotJob steamroom-client.
            let job = DepotJob::builder()
                .depot_id(DepotId(plan.depot_id))
                .depot_key(plan.depot_key.clone())
                .install_dir(opts.install_dir.clone())
                .max_downloads(opts.max_downloads)
                .verify(opts.verify)
                .event_sender(event_tx)
                .build()
                .map_err(|e| anyhow::anyhow!("DepotJob build : {e}"))?;

            let fetcher = Arc::new(CdnChunkFetcher::new(
                CdnClient::new().context("CdnClient fetcher")?,
                CdnServerPool::new(cdn_servers.clone()),
                cdn_auth_token,
            ));

            // Horloge de vie du depot : millisecondes écoulées au dernier
            // événement reçu, quel qu'il soit. C'est la seule chose que regarde
            // le chien de garde — il n'a pas à savoir ce qui progresse.
            let depot_start = Instant::now();
            let last_progress = Arc::new(AtomicU64::new(0));

            // Consomme les DownloadEvent dans une tâche séparée pour ne pas bloquer.
            let progress_clone = progress.clone();
            let depot_id_for_task = plan.depot_id;
            let last_progress_task = last_progress.clone();
            let event_task = tokio::spawn(async move {
                let mut depot_total: u64 = 0;
                let mut depot_completed: u64 = 0;
                let mut skipped: u64 = 0;
                let mut failed_chunks: u64 = 0;
                let mut last_log = Instant::now();
                while let Some(evt) = event_rx.recv().await {
                    // Tout événement vaut signe de vie, y compris FileSkipped :
                    // la passe de vérification peut durer des minutes sans
                    // télécharger le moindre octet.
                    last_progress_task
                        .store(depot_start.elapsed().as_millis() as u64, Ordering::Relaxed);
                    match &evt {
                        DownloadEvent::DownloadStarted { total_bytes, .. } => {
                            depot_total = *total_bytes;
                        }
                        DownloadEvent::FileSkipped { .. } => skipped += 1,
                        DownloadEvent::ChunkFailed { error } => {
                            failed_chunks += 1;
                            // Les premiers échecs disent la cause ; ensuite on
                            // échantillonne pour ne pas noyer le journal.
                            if failed_chunks <= 5 || failed_chunks.is_multiple_of(100) {
                                warn!(
                                    depot = depot_id_for_task,
                                    total = failed_chunks,
                                    "chunk en échec : {error}"
                                );
                            }
                        }
                        DownloadEvent::ChunkCompleted { bytes } => {
                            depot_completed += bytes;
                            emit(
                                &progress_clone,
                                SteamDownloadPhase::Downloading,
                                &format!("depot {depot_id_for_task}"),
                                depot_completed,
                                depot_total,
                            );
                        }
                        _ => {}
                    }
                    // Battement régulier : sans lui le journal reste vide toute
                    // la synchro, et un gel est indiscernable d'un travail long.
                    if last_log.elapsed() >= Duration::from_secs(30) {
                        last_log = Instant::now();
                        let pct = if depot_total > 0 {
                            depot_completed as f64 * 100.0 / depot_total as f64
                        } else {
                            0.0
                        };
                        info!(
                            depot = depot_id_for_task,
                            "progression {pct:.1} % — {:.2} / {:.2} Gio, \
                             {skipped} fichiers déjà à jour, {failed_chunks} chunks en échec",
                            depot_completed as f64 / 1_073_741_824.0,
                            depot_total as f64 / 1_073_741_824.0,
                        );
                    }
                }
                (depot_completed, depot_total)
            });

            // Chien de garde : convertit un blocage silencieux en erreur nommée.
            let stall_timeout = opts.stall_timeout;
            let stall_secs = stall_timeout.map_or(0.0, |d| d.as_secs_f64());
            let last_progress_wd = last_progress.clone();
            let watchdog = async move {
                let Some(timeout) = stall_timeout else {
                    // Garde désactivée : ne se résout jamais, `select!` retient
                    // donc toujours la branche du téléchargement.
                    return std::future::pending::<Duration>().await;
                };
                let tick = Duration::from_secs(15).min(timeout);
                loop {
                    tokio::time::sleep(tick).await;
                    let last = Duration::from_millis(last_progress_wd.load(Ordering::Relaxed));
                    let idle = depot_start.elapsed().saturating_sub(last);
                    if idle >= timeout {
                        return idle;
                    }
                }
            };

            let stats = tokio::select! {
                r = job.download(&manifest, fetcher) => {
                    r.map_err(|e| anyhow::anyhow!("download depot {} : {e}", plan.depot_id))?
                }
                idle = watchdog => {
                    anyhow::bail!(
                        "depot {} bloqué : aucun événement depuis {:.0} s (seuil {:.0} s). \
                         Cause connue : tous les serveurs CDN passés en cooldown après un 429 \
                         — chaque tâche de chunk part alors en sleep, le process tombe à zéro \
                         CPU et zéro socket, et rien ne le réveille. Relancer reprend au même \
                         point (les fichiers déjà à jour sont sautés).",
                        plan.depot_id,
                        idle.as_secs_f64(),
                        stall_secs,
                    );
                }
            };

            // `DepotJob::download` prend `&self` : le job SURVIT à l'appel et
            // garde vivant le `event_tx` qu'il détient. Sans ce `drop`, le canal
            // ne se ferme jamais, la boucle `recv()` de `event_task` ne se
            // termine pas, et l'`await` ci-dessous bloque POUR TOUJOURS — process
            // vivant, RSS effondré, zéro CPU, zéro socket, aucun message. Les
            // deux gels du 2026-08-15 sont exactement cela : le téléchargement
            // était en réalité TERMINÉ, seule la collecte d'événements pendait.
            drop(job);

            // Filet : même canal fermé, on ne rend pas l'attente inconditionnelle.
            let (d, _) = match tokio::time::timeout(Duration::from_secs(30), event_task).await {
                Ok(joined) => joined.unwrap_or((0, 0)),
                Err(_) => {
                    warn!(
                        depot = plan.depot_id,
                        "collecte des événements toujours ouverte après 30 s — \
                         comptage partiel, téléchargement néanmoins terminé"
                    );
                    (0, 0)
                }
            };
            total_downloaded += d;
            total_files += stats.files_completed;
            completed_depots.push(plan.depot_id);

            info!(
                "depot {} : {} fichiers, {} octets",
                plan.depot_id, stats.files_completed, stats.bytes_downloaded
            );
        }

        let duration = start.elapsed();
        emit(
            &progress,
            SteamDownloadPhase::Completed,
            "Téléchargement terminé.",
            total_downloaded,
            total_downloaded,
        );

        Ok(DownloadResult {
            success: true,
            error: None,
            app_id: opts.app_id,
            install_dir: opts.install_dir.clone(),
            depots: completed_depots,
            downloaded_bytes: total_downloaded,
            file_count: total_files,
            duration,
        })
    }

    /// Retourne uniquement le `buildid` de la branche (login + app info, SANS download).
    ///
    /// Équivalent de `GetBuildIdAsync` (C#). Sert à détecter une nouvelle version.
    /// Renvoie `0` si le buildid est introuvable.
    pub async fn get_build_id(&self, opts: &SteamDownloadOptions) -> Result<u32> {
        let mut session = SteamSession::connect(opts).await?;
        session.request_app_info(opts.app_id, false).await?;

        let app_kv = session
            .app_kv(opts.app_id)
            .ok_or_else(|| anyhow::anyhow!("app info {} introuvable", opts.app_id))?;

        let depots_kv = app_kv
            .get("depots")
            .ok_or_else(|| anyhow::anyhow!("section 'depots' absente dans app info"))?;

        Ok(depot_resolver::read_branch_build_id(
            depots_kv,
            &opts.branch,
        ))
    }

    /// Inspecte les depots sans télécharger : retourne taille + fichiers par depot.
    ///
    /// Équivalent de `ListDepotsAsync` (C#). Utile avant un pull multi-Go.
    pub async fn list_depots(&self, opts: &SteamDownloadOptions) -> Result<Vec<DepotInfo>> {
        let mut session = SteamSession::connect(opts).await?;
        session.request_app_info(opts.app_id, false).await?;

        let app_kv = session
            .app_kv(opts.app_id)
            .ok_or_else(|| anyhow::anyhow!("app info {} introuvable", opts.app_id))?
            .clone();

        let depots_kv = app_kv
            .get("depots")
            .ok_or_else(|| anyhow::anyhow!("section 'depots' absente"))?
            .clone();

        let depot_ids = depot_resolver::select_depot_ids(
            &depots_kv,
            &opts.depot_ids,
            opts.all_platforms,
            &opts.os,
            opts.arch.as_deref(),
            &opts.language,
        );

        let plans = build_depot_plans(&mut session, opts, &depot_ids, &depots_kv).await?;

        let cdn_servers = session.cdn_servers(0, Some(5)).await?;
        let cdn_server = cdn_servers
            .first()
            .ok_or_else(|| anyhow::anyhow!("aucun serveur CDN"))?;
        let cdn = CdnClient::new().context("CdnClient")?;
        let manifest_cache = ManifestCache::new(ManifestCache::default_path());

        let mut infos = Vec::new();
        for plan in &plans {
            let cdn_auth_token = session
                .cdn_auth_token(plan.app_id, plan.depot_id, &cdn_server.host)
                .await;
            let request_code = session
                .manifest_request_code(plan.app_id, plan.depot_id, plan.manifest_id, &plan.branch)
                .await;

            let manifest_bytes = load_or_fetch_manifest(
                &cdn,
                cdn_server,
                plan,
                request_code,
                cdn_auth_token.as_deref(),
                &manifest_cache,
            )
            .await?;

            let mut manifest = DepotManifest::parse(&manifest_bytes).context("parse manifest")?;
            if manifest.filenames_encrypted {
                let _ = manifest.decrypt_filenames(&plan.depot_key);
            }

            let mut total_bytes: u64 = 0;
            let mut file_count: u32 = 0;

            for f in &manifest.files {
                if is_manifest_directory(f.flags) {
                    continue;
                }
                total_bytes += f.size;
                file_count += 1;
            }

            let name = depots_kv
                .get(&plan.depot_id.to_string())
                .and_then(|d| d.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_owned());

            infos.push(DepotInfo {
                depot_id: plan.depot_id,
                manifest_id: plan.manifest_id,
                name,
                file_count,
                total_bytes,
            });
        }

        Ok(infos)
    }
}

impl SteamDepotDownloader {
    /// Confronte le manifest à l'état réel de `opts.install_dir`, sans rien écrire.
    ///
    /// Le downloader de `steamroom-client` applique chaque entrée telle quelle :
    /// répertoire → `create_dir_all`, entrée vide → `fs::write`, fichier →
    /// staging puis `rename`. Si le disque contredit la nature attendue, l'appel
    /// remonte un `Is a directory (os error 21)` **anonyme** qui avorte le depot
    /// entier. Cette passe nomme les entrées fautives avant de télécharger.
    ///
    /// Renvoie les collisions dans l'ordre du manifest (donc l'ordre où le
    /// downloader butera dessus) ; vide = le depot est applicable tel quel.
    pub async fn audit_manifest(
        &self,
        opts: &SteamDownloadOptions,
    ) -> Result<Vec<ManifestCollision>> {
        let mut session = SteamSession::connect(opts).await?;
        session.request_app_info(opts.app_id, false).await?;

        let app_kv = session
            .app_kv(opts.app_id)
            .ok_or_else(|| anyhow::anyhow!("app info {} introuvable", opts.app_id))?
            .clone();

        let depots_kv = app_kv
            .get("depots")
            .ok_or_else(|| anyhow::anyhow!("section 'depots' absente"))?
            .clone();

        let depot_ids = depot_resolver::select_depot_ids(
            &depots_kv,
            &opts.depot_ids,
            opts.all_platforms,
            &opts.os,
            opts.arch.as_deref(),
            &opts.language,
        );

        let plans = build_depot_plans(&mut session, opts, &depot_ids, &depots_kv).await?;

        let cdn_servers = session.cdn_servers(0, Some(5)).await?;
        let cdn_server = cdn_servers
            .first()
            .ok_or_else(|| anyhow::anyhow!("aucun serveur CDN"))?;
        let cdn = CdnClient::new().context("CdnClient")?;
        let manifest_cache = ManifestCache::new(ManifestCache::default_path());

        let mut collisions = Vec::new();
        for plan in &plans {
            let cdn_auth_token = session
                .cdn_auth_token(plan.app_id, plan.depot_id, &cdn_server.host)
                .await;
            let request_code = session
                .manifest_request_code(plan.app_id, plan.depot_id, plan.manifest_id, &plan.branch)
                .await;

            let manifest_bytes = load_or_fetch_manifest(
                &cdn,
                cdn_server,
                plan,
                request_code,
                cdn_auth_token.as_deref(),
                &manifest_cache,
            )
            .await?;

            let mut manifest = DepotManifest::parse(&manifest_bytes).context("parse manifest")?;
            if manifest.filenames_encrypted {
                // Contrairement au chemin de download, on ne tait pas l'échec :
                // des noms restés chiffrés produiraient un audit sans valeur.
                manifest
                    .decrypt_filenames(&plan.depot_key)
                    .map_err(|e| anyhow::anyhow!("déchiffrement des noms du manifest : {e}"))?;
            }

            for f in &manifest.files {
                let is_dir = is_manifest_directory(f.flags);
                let rel = f.normalized_path();
                let trimmed = rel.trim_matches('/');

                let reason = if trimmed.is_empty() {
                    Some(CollisionReason::EmptyPath)
                } else {
                    let path = opts.install_dir.join(&rel);
                    match std::fs::symlink_metadata(&path) {
                        Ok(md) if md.is_dir() && !is_dir => Some(CollisionReason::DirectoryOnDisk),
                        Ok(md) if !md.is_dir() && is_dir => Some(CollisionReason::FileOnDisk),
                        _ => None,
                    }
                };

                if let Some(reason) = reason {
                    collisions.push(ManifestCollision {
                        depot_id: plan.depot_id,
                        path: rel,
                        flags: f.flags,
                        size: f.size,
                        chunk_count: f.chunks.len(),
                        reason,
                    });
                }
            }
        }

        Ok(collisions)
    }
}

impl Default for SteamDepotDownloader {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Flags de manifest ────────────────────────────────────────────────────────

/// Bit « répertoire » de `EDepotFileFlag`, tel que Steam l'émet réellement.
///
/// `steamroom::enums::DepotFileFlags::DIRECTORY` vaut `2` — ce n'est **pas** la
/// valeur du protocole. Valve numérote : `UserConfig=1`, `VersionedUserConfig=2`,
/// `Encrypted=4`, `ReadOnly=8`, `Hidden=16`, `Executable=32`, `Directory=64`,
/// `Symlink=128`. `DepotFileFlags::is_directory()` teste donc le mauvais bit.
///
/// Conséquence observée sur le depot 2799861 : ses 12 entrées répertoire
/// (`data`, `data/packs`, `EasyAntiCheat`, …) portent `flags=64`, échappent au
/// test de `steamroom-client`, et retombent sur la branche « fichier vide »
/// (`size == 0 && chunks.is_empty()`) qui appelle `fs::write` sur un répertoire
/// existant. Résultat : `Is a directory (os error 21)`, sans nom de chemin, qui
/// avorte le depot **entier** dès la première entrée. Diagnostiqué par
/// [`SteamDepotDownloader::audit_manifest`].
pub const STEAM_FLAG_DIRECTORY: u32 = 64;

/// Vrai si l'entrée de manifest décrit un répertoire (cf. [`STEAM_FLAG_DIRECTORY`]).
///
/// À préférer à `DepotFileFlags::is_directory()`, qui teste le bit `2`.
pub fn is_manifest_directory(flags: u32) -> bool {
    flags & STEAM_FLAG_DIRECTORY != 0
}

// ─── Plan interne ─────────────────────────────────────────────────────────────

/// Plan de téléchargement pour un depot : toutes les infos nécessaires.
struct DepotPlan {
    depot_id: u32,
    app_id: u32,
    manifest_id: u64,
    branch: String,
    depot_key: steamroom::depot::DepotKey,
}

/// Pour chaque depot ID éligible : résout le manifest GID et la clé de déchiffrement.
async fn build_depot_plans(
    session: &mut SteamSession,
    opts: &SteamDownloadOptions,
    depot_ids: &[u32],
    depots_kv: &steamroom::types::key_value::KeyValue,
) -> Result<Vec<DepotPlan>> {
    let mut plans = Vec::new();

    for &depot_id in depot_ids {
        let manifest_id = resolve_manifest(session, opts, depot_id, depots_kv).await?;
        if manifest_id == INVALID_MANIFEST {
            warn!(
                "depot {depot_id} : pas de manifest pour la branche '{}' — ignoré",
                opts.branch
            );
            continue;
        }

        let Some(key) = session.request_depot_key(depot_id, opts.app_id).await? else {
            warn!("depot {depot_id} : clé indisponible — ignoré");
            continue;
        };

        plans.push(DepotPlan {
            depot_id,
            app_id: opts.app_id,
            manifest_id,
            branch: opts.branch.clone(),
            depot_key: key,
        });
    }

    Ok(plans)
}

/// Résout le manifest GID d'un depot via le resolver, avec support proxy (`depotfromapp`).
async fn resolve_manifest(
    session: &mut SteamSession,
    opts: &SteamDownloadOptions,
    depot_id: u32,
    depots_kv: &steamroom::types::key_value::KeyValue,
) -> Result<u64> {
    let depot_child = depots_kv.get(&depot_id.to_string());

    let Some(depot_child) = depot_child else {
        return Ok(INVALID_MANIFEST);
    };

    // Depot proxié → résoudre depuis l'app cible.
    let proxy_app = depot_resolver::proxied_from_app(depot_child);
    if proxy_app != 0 && proxy_app != opts.app_id {
        session.request_app_info(proxy_app, false).await?;
        if let Some(proxy_kv) = session.app_kv(proxy_app)
            && let Some(proxy_depots) = proxy_kv.get("depots")
        {
            let proxy_depots = proxy_depots.clone();
            return resolve_manifest_from_depots(&proxy_depots, depot_id, &opts.branch);
        }
        return Ok(INVALID_MANIFEST);
    }

    Ok(depot_resolver::read_manifest_gid(depot_child, &opts.branch))
}

fn resolve_manifest_from_depots(
    depots: &steamroom::types::key_value::KeyValue,
    depot_id: u32,
    branch: &str,
) -> Result<u64> {
    let child = depots.get(&depot_id.to_string());
    Ok(child.map_or(INVALID_MANIFEST, |d| {
        depot_resolver::read_manifest_gid(d, branch)
    }))
}

/// Charge un manifest depuis le cache local, ou le télécharge depuis le CDN.
async fn load_or_fetch_manifest(
    cdn: &CdnClient,
    server: &steamroom::cdn::CdnServer,
    plan: &DepotPlan,
    request_code: u64,
    cdn_auth_token: Option<&str>,
    cache: &ManifestCache,
) -> Result<Vec<u8>> {
    let depot_id = DepotId(plan.depot_id);
    let manifest_id = ManifestId(plan.manifest_id);

    if let Some(cached) = cache.load(depot_id, manifest_id) {
        return Ok(cached);
    }

    let raw = cdn
        .download_manifest(server, depot_id, manifest_id, request_code, cdn_auth_token)
        .await
        .with_context(|| {
            format!(
                "download manifest {} depot {}",
                plan.manifest_id, plan.depot_id
            )
        })?;

    // Décompresse si zippé.
    let bytes = decompress_if_zipped(&raw)?;
    let _ = cache.save(depot_id, manifest_id, &bytes, &raw);
    Ok(bytes)
}

/// Décompresse un manifest zip si nécessaire (format CDN SteamPipe).
fn decompress_if_zipped(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() > 2 && data[0] == 0x50 && data[1] == 0x4B {
        let cursor = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor).context("parse manifest zip")?;
        if archive.is_empty() {
            anyhow::bail!("archive manifest vide");
        }
        let mut file = archive.by_index(0).context("manifest zip entry 0")?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut buf).context("lire manifest zip")?;
        Ok(buf)
    } else {
        Ok(data.to_vec())
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn emit(
    tx: &Option<mpsc::UnboundedSender<SteamDownloadProgress>>,
    phase: SteamDownloadPhase,
    message: &str,
    downloaded: u64,
    total: u64,
) {
    if let Some(tx) = tx {
        let _ = tx.send(SteamDownloadProgress {
            phase,
            message: message.to_owned(),
            downloaded_bytes: downloaded,
            total_bytes: total,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Valeurs réelles de `EDepotFileFlag` côté Valve, telles qu'observées sur
    /// le manifest 7633204652048533395 du depot 2799861.
    #[test]
    fn directory_flag_is_bit_64() {
        assert!(is_manifest_directory(64));
        assert!(is_manifest_directory(STEAM_FLAG_DIRECTORY));
        // Un répertoire combiné à d'autres attributs reste un répertoire.
        assert!(is_manifest_directory(64 | 8));
        assert!(is_manifest_directory(64 | 16));
    }

    /// Le piège d'origine : `steamroom` définit `DIRECTORY = 2`, qui côté Steam
    /// est `VersionedUserConfig`. Confondre les deux fait écrire un fichier
    /// par-dessus un répertoire (`Is a directory`, os error 21).
    #[test]
    fn bit_2_is_not_a_directory() {
        assert!(!is_manifest_directory(2));
        assert!(!is_manifest_directory(
            steamroom::enums::DepotFileFlags::DIRECTORY.0
        ));
    }

    #[test]
    fn ordinary_files_are_not_directories() {
        assert!(!is_manifest_directory(0)); // fichier nu
        assert!(!is_manifest_directory(1)); // UserConfig
        assert!(!is_manifest_directory(32)); // Executable
        assert!(!is_manifest_directory(128)); // Symlink
    }
}
