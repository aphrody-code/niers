//! `/robots.txt`, `/.well-known/security.txt` et `/sitemap.xml`.
//!
//! Les trois sont rendus par `askama` depuis l'origine configurée : rien n'est codé en dur, un
//! déploiement sur une autre origine (preview, machine de développement) rend des documents
//! cohérents avec lui-même.

use askama::Template;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::etat::EtatSite;

/// Une URL du plan de site.
#[derive(Debug, Clone)]
pub struct UrlPlan {
    /// Chemin absolu, commençant par `/`.
    pub chemin: &'static str,
    /// Fréquence de mise à jour annoncée.
    pub frequence: &'static str,
    /// Priorité relative.
    pub priorite: &'static str,
}

/// Les routes de navigation publiées au plan de site. Les espaces `/f` et `/b` n'y sont
/// **jamais** : ce sont 255 000 fichiers, et un plan de site n'est pas un index d'assets.
pub const PLAN: [UrlPlan; 5] = [
    UrlPlan { chemin: "/", frequence: "daily", priorite: "1.0" },
    UrlPlan { chemin: "/textures", frequence: "weekly", priorite: "0.8" },
    UrlPlan { chemin: "/modeles", frequence: "weekly", priorite: "0.8" },
    UrlPlan { chemin: "/sons", frequence: "weekly", priorite: "0.6" },
    UrlPlan { chemin: "/videos", frequence: "weekly", priorite: "0.6" },
];

#[derive(Template)]
#[template(path = "robots.txt")]
struct Robots<'a> {
    origine: &'a str,
}

#[derive(Template)]
#[template(path = "security.txt")]
struct Securite<'a> {
    origine: &'a str,
    expiration: String,
}

#[derive(Template)]
#[template(path = "sitemap.xml")]
struct Plan<'a> {
    origine: &'a str,
    urls: &'a [UrlPlan],
}

fn texte(corps: Result<String, askama::Error>, type_contenu: &'static str) -> Response {
    match corps {
        Ok(c) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, type_contenu),
                (header::CACHE_CONTROL, "public, max-age=3600"),
            ],
            c,
        )
            .into_response(),
        Err(e) => {
            tracing::error!(erreur = %e, "rendu d'un document well-known impossible");
            crate::ErreurSite::Interne("document indisponible".to_owned()).into_response()
        }
    }
}

/// `/robots.txt`.
pub async fn robots(State(etat): State<EtatSite>) -> Response {
    texte(Robots { origine: &etat.config.origine }.render(), "text/plain; charset=utf-8")
}

/// `/.well-known/security.txt`.
///
/// `Expires` est obligatoire (RFC 9116) et doit être dans le futur : il est calculé à
/// l'exécution, un an après le démarrage, plutôt que figé dans le dépôt où il périme sans
/// que personne ne le voie.
pub async fn security(State(etat): State<EtatSite>) -> Response {
    texte(
        Securite { origine: &etat.config.origine, expiration: expiration_dans_un_an() }.render(),
        "text/plain; charset=utf-8",
    )
}

/// `/sitemap.xml`.
pub async fn sitemap(State(etat): State<EtatSite>) -> Response {
    texte(
        Plan { origine: &etat.config.origine, urls: &PLAN }.render(),
        "application/xml; charset=utf-8",
    )
}

/// Horodatage ISO 8601 UTC, un an après maintenant, sans dépendance de calendrier.
fn expiration_dans_un_an() -> String {
    let secondes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        + 365 * 24 * 3600;
    iso8601_utc(secondes)
}

/// Convertit un instant Unix en `AAAA-MM-JJTHH:MM:SSZ` (calendrier grégorien proleptique).
fn iso8601_utc(secondes: u64) -> String {
    let jours = (secondes / 86_400) as i64;
    let reste = secondes % 86_400;
    // Algorithme civil_from_days de Howard Hinnant, domaine public.
    let z = jours + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        reste / 3600,
        (reste % 3600) / 60,
        reste % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_complet() {
        assert_eq!(PLAN.len(), 5);
        let rendu = Plan { origine: "https://aphrody.com", urls: &PLAN }.render().unwrap();
        assert_eq!(rendu.matches("<url>").count(), 5);
        assert!(rendu.starts_with("<?xml"));
        assert!(rendu.contains("https://aphrody.com/textures"));
    }

    #[test]
    fn robots_pointe_le_plan() {
        let r = Robots { origine: "https://aphrody.com" }.render().unwrap();
        assert!(r.contains("Sitemap: https://aphrody.com/sitemap.xml"));
        assert_eq!(r.matches("Disallow:").count(), 5);
    }

    #[test]
    fn horodatage_iso8601() {
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
        // 2026-09-05T00:00:00Z
        assert_eq!(iso8601_utc(1_788_566_400), "2026-09-05T00:00:00Z");
        assert!(expiration_dans_un_an().as_str() > "2027-01-01T00:00:00Z");
    }
}
