//! Binaire `nie-site` — Aphrody sur `127.0.0.1:8085`, derrière nginx.
//!
//! Le processus démarre **avant** que le VFS ne soit monté : le montage d'un dump de 255 000
//! fichiers prend du temps, et un serveur qui n'écoute qu'après lui est un serveur que
//! `systemd` déclare en échec pour rien. `/healthz` répond dès la première milliseconde et dit
//! honnêtement ce qui est prêt.

#![warn(missing_docs)]

use anyhow::Context as _;
use clap::Parser as _;
use nie_site::config::Options;
use nie_site::etat::EtatSite;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Options::parse().en_config().context("adresse d'ecoute invalide")?;
    let adresse = config.adresse;
    tracing::info!(
        version = nie_site::VERSION,
        adresse = %adresse,
        db = %config.db.display(),
        amont = %config.amont,
        statique = %config.statique.display(),
        "demarrage de nie-site"
    );

    let etat = EtatSite::nouveau(config);
    etat.monter_vfs_en_fond();
    let app = nie_site::routeur(etat);

    let ecoute = tokio::net::TcpListener::bind(adresse)
        .await
        .with_context(|| format!("ecoute impossible sur {adresse}"))?;
    tracing::info!(adresse = %adresse, "nie-site ecoute");

    axum::serve(ecoute, app)
        .with_graceful_shutdown(arret())
        .await
        .context("le serveur s'est arrete sur une erreur")?;
    Ok(())
}

/// Attend `SIGTERM` (systemd) ou `Ctrl-C`.
async fn arret() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminaison = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => tracing::error!(erreur = %e, "SIGTERM non ecoutable"),
        }
    };
    #[cfg(not(unix))]
    let terminaison = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("arret demande (Ctrl-C)"),
        () = terminaison => tracing::info!("arret demande (SIGTERM)"),
    }
}
