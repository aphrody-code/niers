//! Pages HTML : la coquille d'`apps/nie-web` et les pages d'erreur, rendues par `askama`.
//!
//! `index.html` n'est pas servi tel quel : il passe par un template pour recevoir le titre, la
//! description et les balises `og:` **de la route demandée** — une texture ou un modèle partagé
//! sur un réseau social doit avoir sa vignette, et un SPA servi brut n'en a jamais.
//!
//! `askama_axum` est mort (`0.5.0+deprecated`) et il n'existe pas de feature `with-axum` en
//! askama 0.16 : on rend en `String` et on construit la réponse à la main.

use askama::Template;
use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};

use crate::state::EtatSite;

/// Couleur de cadrage de la DA du jeu (`#295B9F`), mesurée sur le menu principal.
pub const COULEUR_THEME: &str = "#295B9F";

/// Coquille HTML servie pour toute route de navigation.
#[derive(Template)]
#[template(path = "index.html")]
pub struct Coquille {
    /// Titre de la page (`<title>` et `og:title`).
    pub titre: String,
    /// Description (`meta description` et `og:description`).
    pub description: String,
    /// URL canonique absolue.
    pub url: String,
    /// Type Open Graph (`website`, `article`…).
    pub type_og: &'static str,
    /// Vignette absolue, quand la route en désigne une.
    pub image: Option<String>,
    /// Route demandée, transmise au bundle par `data-route`.
    pub route: String,
    /// Feuille de style du bundle, quand elle a été trouvée.
    pub feuille: Option<String>,
    /// Point d'entrée JavaScript du bundle, quand il a été trouvé.
    pub script: Option<String>,
    /// Couleur de thème (`theme-color`).
    pub couleur_theme: &'static str,
}

/// Page d'erreur HTML, pour les routes de navigation.
#[derive(Template)]
#[template(path = "erreur.html")]
pub struct PageErreur {
    /// Code HTTP.
    pub code: u16,
    /// Titre court.
    pub titre: &'static str,
    /// Message en français.
    pub message: String,
    /// Couleur de thème.
    pub couleur_theme: &'static str,
}

impl PageErreur {
    /// Construit une page d'erreur et la rend directement en réponse.
    #[must_use]
    pub fn reponse(code: StatusCode, titre: &'static str, message: impl Into<String>) -> Response {
        let page = Self {
            code: code.as_u16(),
            titre,
            message: message.into(),
            couleur_theme: COULEUR_THEME,
        };
        match page.render() {
            Ok(html) => (code, Html(html)).into_response(),
            Err(e) => {
                tracing::error!(erreur = %e, "rendu de la page d'erreur impossible");
                (code, "erreur").into_response()
            }
        }
    }
}

/// Métadonnées d'une route de navigation : ce qui distingue une page d'une autre pour un
/// robot ou un aperçu de réseau social.
#[must_use]
pub fn metadonnees(route: &str) -> (String, String, &'static str) {
    let nu = route.trim_start_matches('/').trim_end_matches('/');
    let premier = nu.split('/').next().unwrap_or("");
    match premier {
        "" => (
            "Aphrody — les fichiers d'Inazuma Eleven: Victory Road".to_owned(),
            "Explorer, décoder et exporter les textures, modèles, sons et vidéos du jeu, depuis leur chemin d'origine.".to_owned(),
            "website",
        ),
        "textures" => (
            "Textures — Aphrody".to_owned(),
            "Toutes les textures du jeu, à leur chemin d'origine, converties à la demande.".to_owned(),
            "website",
        ),
        "modeles" => (
            "Modèles — Aphrody".to_owned(),
            "Les modèles du jeu, assemblés et exportables, à leur chemin d'origine.".to_owned(),
            "website",
        ),
        "sons" => (
            "Sons — Aphrody".to_owned(),
            "Les banques audio du jeu (ACB, AWB, HCA), décodées à la demande.".to_owned(),
            "website",
        ),
        "videos" => (
            "Vidéos — Aphrody".to_owned(),
            "Les vidéos du jeu (USM), lisibles depuis leur chemin d'origine.".to_owned(),
            "website",
        ),
        autre => (
            format!("{autre} — Aphrody"),
            "Explorer les fichiers d'Inazuma Eleven: Victory Road.".to_owned(),
            "article",
        ),
    }
}

/// Sert la coquille pour une route de navigation.
pub async fn coquille(State(etat): State<EtatSite>, uri: Uri) -> Response {
    let route = uri.path().to_owned();
    let (titre, description, type_og) = metadonnees(&route);
    let (feuille, script) = crate::routes::static_files::points_d_entree(&etat.config.statique).await;
    let page = Coquille {
        titre,
        description,
        url: format!("{}{}", etat.config.origine, route),
        type_og,
        image: None,
        route,
        feuille,
        script,
        couleur_theme: COULEUR_THEME,
    };
    match page.render() {
        Ok(html) => (
            StatusCode::OK,
            [(header::CACHE_CONTROL, "public, max-age=60, stale-while-revalidate=600")],
            Html(html),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(erreur = %e, "rendu de la coquille impossible");
            PageErreur::reponse(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Page indisponible",
                "La coquille du site n'a pas pu être rendue.",
            )
        }
    }
}

/// Repli des routes inconnues : JSON sous `/api`, page d'erreur HTML ailleurs.
pub async fn repli(State(etat): State<EtatSite>, uri: Uri) -> Response {
    let chemin = uri.path();
    if chemin.starts_with("/api/") || chemin == "/api" {
        return crate::ErreurSite::Introuvable(format!("route d'API inconnue: {chemin}"))
            .into_response();
    }
    if chemin.starts_with("/assets/") || chemin.starts_with("/f/") || chemin.starts_with("/b/") {
        return crate::ErreurSite::Introuvable(format!("ressource inconnue: {chemin}"))
            .into_response();
    }
    // Toute autre route est une route du bundle : c'est le client qui décide si elle existe.
    coquille(State(etat), uri).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadonnees_par_route() {
        assert!(metadonnees("/").0.starts_with("Aphrody"));
        assert_eq!(metadonnees("/textures").0, "Textures — Aphrody");
        assert_eq!(metadonnees("/modeles/x/y").2, "website");
        assert_eq!(metadonnees("/inconnue").2, "article");
    }

    #[test]
    fn coquille_porte_les_balises_og() {
        let page = Coquille {
            titre: "T".to_owned(),
            description: "D".to_owned(),
            url: "https://aphrody.com/".to_owned(),
            type_og: "website",
            image: Some("https://aphrody.com/i.png".to_owned()),
            route: "/".to_owned(),
            feuille: None,
            script: None,
            couleur_theme: COULEUR_THEME,
        };
        let html = page.render().expect("rendu");
        for balise in [
            "og:title",
            "og:description",
            "og:url",
            "og:image",
            "og:type",
            "og:site_name",
            "twitter:card",
        ] {
            assert!(html.contains(balise), "balise {balise} absente");
        }
        assert_eq!(html.matches("<meta property=\"og:").count(), 7);
    }
}
