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
use nie_site::state::EtatSite;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let options = Options::parse();
    // Mode outil : on regenere la matrice de couverture et on sort SANS ecouter. C'est la
    // commande du § 4 du plan — la matrice se regenere, elle ne se tient pas a la main.
    if let Some(sortie) = options.regenerer_couverture.clone() {
        let racine = options
            .racine_depot
            .clone()
            .or_else(|| std::env::var_os("NIE_GAME_DIR").map(std::path::PathBuf::from))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        return regenerer_couverture(&racine, &sortie);
    }

    let config = options.en_config().context("adresse d'ecoute invalide")?;
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

/// Regenere la matrice de couverture et publie ses comptes sur la sortie standard.
///
/// Elle **affiche** ce qu'elle a mesure, source par source, plutot que d'ecrire un fichier en
/// silence : un compte que personne ne lit est un compte que personne ne verifie. Le code de
/// retour reste 0 meme quand la gate est rompue — la commande mesure, elle ne juge pas ; c'est
/// le plan qui fixe la cible.
fn regenerer_couverture(racine: &std::path::Path, sortie: &std::path::Path) -> anyhow::Result<()> {
    let matrice = nie_site::couverture::generer(racine, sortie)
        .with_context(|| format!("mesure impossible depuis {}", racine.display()))?;
    println!(
        "matrice ecrite dans {} — {} capacites, {} unites de poids, {} routes montees",
        sortie.display(),
        matrice.total.capacites,
        matrice.total.poids,
        matrice.routes_montees
    );
    for nom in nie_site::couverture::Etat::NOMS {
        let c = matrice.par_etat.get(nom).copied().unwrap_or_default();
        println!(
            "  {nom:<9} {:>5} capacites  {:>8} poids",
            c.capacites, c.poids
        );
    }
    for ligne in &matrice.par_source {
        let manquant = ligne.par_etat.get("manquant").copied().unwrap_or_default();
        let partiel = ligne.par_etat.get("partiel").copied().unwrap_or_default();
        println!(
            "  {:<38} total {:>5}  manquant {:>5}  partiel {:>4}  [{}]",
            ligne.libelle,
            ligne.total.capacites,
            manquant.capacites,
            partiel.capacites,
            ligne.commande
        );
    }
    for i in &matrice.incoherences {
        println!("  INCOHERENCE {i}");
    }
    for r in &matrice.regles_mortes {
        println!("  REGLE MORTE {r}");
    }
    // Les filets : ce qu'ils attrapent est classe EN GROS, d'une seule raison. Zero est
    // l'objectif — la source est alors classee decision par decision.
    let filets_charges: Vec<_> = matrice.filets.iter().filter(|f| f.capacites > 0).collect();
    let (cap_filets, poids_filets) = filets_charges
        .iter()
        .fold((0u64, 0u64), |(c, p), f| (c + f.capacites, p + f.poids));
    println!(
        "filets : {} charges sur {} — {cap_filets} capacites classees en gros ({poids_filets} poids)",
        filets_charges.len(),
        matrice.filets.len()
    );
    for f in filets_charges {
        println!(
            "  FILET {:<24} {:>5} capacites  {:>7} poids  [{}]",
            f.id, f.capacites, f.poids, f.etat
        );
    }
    println!(
        "gate maitresse : manquant = {} ({} poids), partiel = {} ({} poids) -> {}",
        matrice.gate.manquant,
        matrice.gate.manquant_poids,
        matrice.gate.partiel,
        matrice.gate.partiel_poids,
        if matrice.gate.tenue {
            "TENUE"
        } else {
            "ROMPUE"
        }
    );
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
