//! `/healthz` — ce que la machine peut réellement répondre, mesuré à l'instant de l'appel.
//!
//! La gate de J5 cherche la chaîne `nie-site` dans cette réponse : c'est ce qui distingue
//! Aphrody servi par cette crate d'Aphrody servi par l'ancien `aphrody-site` sur `:8083`.

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::state::{Capacites, EtatSite};

/// Corps de `/healthz`.
#[derive(Debug, Serialize)]
pub struct Sante {
    /// Toujours `nie-site`.
    pub service: &'static str,
    /// Version de la crate.
    pub version: &'static str,
    /// `ok` tant que le processus répond — une capacité absente ne rend pas le service malade.
    pub etat: &'static str,
    /// Capacités mesurées, jamais supposées.
    pub capacites: Capacites,
}

/// Répond `/healthz`.
pub async fn healthz(State(etat): State<EtatSite>) -> Json<Sante> {
    Json(Sante {
        service: crate::SERVICE,
        version: crate::VERSION,
        etat: "ok",
        capacites: etat.capacites(),
    })
}
