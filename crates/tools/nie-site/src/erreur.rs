//! Erreurs du serveur et leur traduction en réponse HTTP.
//!
//! Règle : le client reçoit un **code stable et un message court**, jamais un détail SQL, un
//! chemin de machine ou un message d'erreur d'amont. Le détail part dans les journaux.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Erreur du serveur, telle qu'elle est rendue au client.
#[derive(Debug, thiserror::Error)]
pub enum ErreurSite {
    /// La ressource demandée n'existe pas (chemin VFS inconnu, route d'API inconnue).
    #[error("{0}")]
    Introuvable(String),

    /// La demande est mal formée : chemin qui sort du VFS, paramètre non numérique.
    #[error("{0}")]
    Demande(String),

    /// Une capacité est absente (VFS non monté, miroir absent) : le service tourne, mais pas
    /// cette route. C'est un `503`, jamais un `500` — l'appelant peut réessayer plus tard.
    #[error("{0}")]
    Indisponible(String),

    /// L'amont (`nie-model-serve`) n'a pas répondu dans le délai imparti.
    #[error("{0}")]
    Delai(String),

    /// L'amont a répondu, mais mal (connexion refusée, corps tronqué, taille excessive).
    #[error("{0}")]
    Amont(String),

    /// Défaut interne : entrée/sortie, SQL, tâche annulée. Le détail reste dans les journaux.
    #[error("{0}")]
    Interne(String),
}

impl ErreurSite {
    /// Code HTTP associé.
    #[must_use]
    pub fn statut(&self) -> StatusCode {
        match self {
            Self::Introuvable(_) => StatusCode::NOT_FOUND,
            Self::Demande(_) => StatusCode::BAD_REQUEST,
            Self::Indisponible(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Delai(_) => StatusCode::GATEWAY_TIMEOUT,
            Self::Amont(_) => StatusCode::BAD_GATEWAY,
            Self::Interne(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Jeton machine du genre d'erreur, stable dans le temps (le message, lui, peut évoluer).
    #[must_use]
    pub fn genre(&self) -> &'static str {
        match self {
            Self::Introuvable(_) => "introuvable",
            Self::Demande(_) => "demande_invalide",
            Self::Indisponible(_) => "indisponible",
            Self::Delai(_) => "delai_amont",
            Self::Amont(_) => "amont",
            Self::Interne(_) => "interne",
        }
    }
}

/// Corps JSON d'une erreur. Deux champs, tous deux stables : `genre` pour la machine,
/// `message` pour l'humain.
#[derive(Debug, Serialize)]
pub struct CorpsErreur {
    /// Jeton machine ([`ErreurSite::genre`]).
    pub genre: &'static str,
    /// Message court, en français, sans détail interne.
    pub message: String,
}

impl IntoResponse for ErreurSite {
    fn into_response(self) -> Response {
        let statut = self.statut();
        if statut == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(erreur = %self, "defaut interne");
        } else {
            tracing::debug!(erreur = %self, statut = statut.as_u16(), "reponse d'erreur");
        }
        let corps = CorpsErreur { genre: self.genre(), message: self.to_string() };
        (statut, Json(corps)).into_response()
    }
}

/// Convertit une erreur de tâche bloquante (`spawn_blocking` annulée ou paniquée).
impl From<tokio::task::JoinError> for ErreurSite {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::Interne(format!("tache interrompue: {e}"))
    }
}

/// Convertit une erreur `rusqlite` sans laisser fuiter le SQL vers le client.
impl From<rusqlite::Error> for ErreurSite {
    fn from(e: rusqlite::Error) -> Self {
        tracing::error!(erreur = %e, "erreur sqlite");
        Self::Interne("lecture du gisement impossible".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_et_genres() {
        let cas = [
            (ErreurSite::Introuvable("x".into()), 404, "introuvable"),
            (ErreurSite::Demande("x".into()), 400, "demande_invalide"),
            (ErreurSite::Indisponible("x".into()), 503, "indisponible"),
            (ErreurSite::Delai("x".into()), 504, "delai_amont"),
            (ErreurSite::Amont("x".into()), 502, "amont"),
            (ErreurSite::Interne("x".into()), 500, "interne"),
        ];
        assert_eq!(cas.len(), 6, "six genres d'erreur, pas un de plus");
        for (e, code, genre) in cas {
            assert_eq!(e.statut().as_u16(), code);
            assert_eq!(e.genre(), genre);
        }
    }

    #[test]
    fn sqlite_ne_fuit_pas_le_detail() {
        let e: ErreurSite = rusqlite::Error::InvalidQuery.into();
        assert_eq!(e.statut().as_u16(), 500);
        assert!(!e.to_string().to_lowercase().contains("query"), "aucun detail SQL au client");
    }
}
