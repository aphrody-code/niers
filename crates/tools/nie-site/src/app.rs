//! Le routeur et les couches communes.
//!
//! La `Content-Security-Policy` est posée **ici**, par la crate, et nulle part ailleurs : deux
//! CSP s'additionnent et la plus stricte gagne, donc le bloc nginx d'`aphrody.com` n'en pose
//! aucune (cf. `docs/stack/web-platform.md`). Un en-tête qui vient de deux endroits est un
//! en-tête que personne ne contrôle.

use std::time::Duration;

use axum::Router;
use axum::http::{HeaderValue, StatusCode, header};
use axum::routing::get;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::state::EtatSite;

/// La politique de sécurité du contenu servie par Aphrody.
///
/// `img-src`/`media-src` acceptent `blob:` et `data:` parce que le site décode des textures et
/// de l'audio côté client à partir d'octets bruts ; `connect-src 'self'` suffit puisque `/f`,
/// `/api/v1` et `/assets` sont sur la même origine — aucune origine tierce n'est nécessaire.
pub const CSP: &str = "default-src 'self'; \
     script-src 'self' 'wasm-unsafe-eval'; \
     style-src 'self' 'unsafe-inline'; \
     img-src 'self' data: blob:; \
     media-src 'self' blob:; \
     font-src 'self' data:; \
     connect-src 'self'; \
     worker-src 'self' blob:; \
     object-src 'none'; \
     base-uri 'none'; \
     form-action 'none'; \
     frame-ancestors 'none'";

/// Nombre d'en-têtes de sécurité posés — compté par les tests.
pub const NB_ENTETES_SECURITE: usize = 5;

/// Les en-têtes de sécurité posés sur **toutes** les réponses, y compris les erreurs.
#[must_use]
pub fn entetes_securite_liste() -> [(header::HeaderName, &'static str); NB_ENTETES_SECURITE] {
    [
        (header::CONTENT_SECURITY_POLICY, CSP),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::X_FRAME_OPTIONS, "DENY"),
        (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
        (
            header::HeaderName::from_static("permissions-policy"),
            "geolocation=(), camera=(), microphone=(), payment=()",
        ),
    ]
}

/// Délai maximal d'une requête entrante, toutes routes confondues. Il est plus large que le
/// délai d'amont (10 s) pour que le `504` de l'amont arrive au client avant que la requête ne
/// soit coupée ici.
pub const DELAI_REQUETE: Duration = Duration::from_secs(15);

/// Les routes exposées, dans l'ordre où elles sont déclarées. Sert de contrat vérifiable : les
/// tests comptent cette liste et interrogent chacune de ses entrées.
pub const ROUTES: [&str; 19] = [
    "/healthz",
    "/robots.txt",
    "/llms.txt",
    "/llms-full.txt",
    "/manifest.webmanifest",
    "/en/manifest.webmanifest",
    "/ja/manifest.webmanifest",
    "/.well-known/security.txt",
    "/sitemap.xml",
    "/feed.atom",
    "/api/v1/health",
    "/api/v1/chara",
    "/api/v1/{vue}",
    "/f/{*chemin}",
    "/b",
    "/b/{*prefixe}",
    "/api/v1/episodes",
    "/assets/{*chemin}",
    "/",
];

/// Construit le routeur complet.
///
/// Syntaxe de route d'axum 0.8 : `{param}` et `{*wildcard}`. L'ancienne forme (`:id`, `*path`)
/// **panique** au `route()` — elle ne dégrade pas.
pub fn routeur(etat: EtatSite) -> Router {
    Router::new()
        .route("/healthz", get(crate::routes::health::healthz))
        .route("/robots.txt", get(crate::routes::well_known::robots))
        .route("/llms.txt", get(crate::routes::well_known::llms))
        .route("/llms-full.txt", get(crate::routes::well_known::llms_complet))
        // Une route par langue, declarees une par une. Un parametre `/{langue}/manifest…`
        // capturerait n'importe quel segment et servirait le manifeste francais sous autant
        // d'URL qu'on peut en inventer.
        .route("/manifest.webmanifest", get(crate::routes::well_known::manifeste))
        .route("/en/manifest.webmanifest", get(crate::routes::well_known::manifeste))
        .route("/ja/manifest.webmanifest", get(crate::routes::well_known::manifeste))
        .route("/.well-known/security.txt", get(crate::routes::well_known::security))
        .route("/sitemap.xml", get(crate::routes::well_known::sitemap))
        .route("/feed.atom", get(crate::routes::feed::atom))
        .route("/api/v1/health", get(crate::routes::api_v1::health))
        .route("/api/v1/chara", get(crate::routes::api_v1::chara))
        .route("/api/v1/{vue}", get(crate::routes::api_v1::vue))
        .route("/f/{*chemin}", get(crate::routes::vfs::fichier))
        .route("/b", get(crate::routes::vfs::parcours_racine))
        .route("/b/{*prefixe}", get(crate::routes::vfs::parcours))
        .route("/api/v1/episodes", get(crate::routes::episodes::episodes))
        .route("/assets/{*chemin}", get(crate::routes::assets::assets))
        .route("/", get(crate::routes::pages::coquille))
        .fallback(crate::routes::static_files::statique)
        // Les couches s'empilent de la plus INTERNE à la plus externe, et l'ordre est ici un
        // choix, pas une habitude :
        //
        // - l'ETag est au plus près des routes, seul endroit d'où l'on voie le corps final ;
        // - la borne de débit est AU-DESSUS, pour qu'un client refusé ne fasse ni requête SQL
        //   ni condensé — un limiteur qui laisse d'abord travailler ne limite que la bande
        //   passante ;
        // - les en-têtes de sécurité l'enveloppent, pour qu'un `429` les porte aussi ;
        // - le délai maximal et la trace restent les plus externes, faute de quoi ils ne
        //   verraient ni les réponses des couches ci-dessus ni leur latence.
        .layer(axum::middleware::from_fn(crate::etag::conditionnel))
        .layer(axum::middleware::from_fn_with_state(etat.clone(), crate::debit::limiter))
        .layer(axum::middleware::from_fn(entetes_securite))
        .layer(TimeoutLayer::with_status_code(StatusCode::GATEWAY_TIMEOUT, DELAI_REQUETE))
        .layer(TraceLayer::new_for_http())
        .with_state(etat)
}

/// Pose les en-têtes de sécurité sur la réponse, sans jamais écraser un en-tête déjà posé par
/// une route (une route peut avoir une raison de durcir davantage, jamais d'assouplir).
async fn entetes_securite(
    requete: axum::extract::Request,
    suite: axum::middleware::Next,
) -> axum::response::Response {
    let mut reponse = suite.run(requete).await;
    let entetes = reponse.headers_mut();
    for (nom, valeur) in entetes_securite_liste() {
        if !entetes.contains_key(&nom) {
            entetes.insert(nom, HeaderValue::from_static(valeur));
        }
    }
    reponse
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csp_complete_et_stricte() {
        let directives: Vec<&str> = CSP.split(';').map(str::trim).collect();
        assert_eq!(directives.len(), 12, "douze directives, comptees");
        for attendue in [
            "default-src 'self'",
            "object-src 'none'",
            "base-uri 'none'",
            "form-action 'none'",
            "frame-ancestors 'none'",
        ] {
            assert!(directives.contains(&attendue), "directive absente: {attendue}");
        }
        assert!(!CSP.contains(" 'unsafe-eval'"), "pas d'eval JavaScript (wasm excepte)");
        assert!(!CSP.contains("script-src 'self' 'unsafe-inline'"), "pas de script inline");
        assert_eq!(entetes_securite_liste().len(), NB_ENTETES_SECURITE);
    }

    #[test]
    fn contrat_de_routes() {
        assert_eq!(ROUTES.len(), 19);
        for r in ROUTES {
            assert!(r.starts_with('/'), "{r}");
            assert!(!r.contains(":{"), "syntaxe axum 0.7 interdite: {r}");
        }
    }
}
