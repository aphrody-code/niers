//! `/robots.txt`, `/.well-known/security.txt` et `/sitemap.xml`.
//!
//! Les trois sont rendus par `askama` depuis l'origine configurée : rien n'est codé en dur, un
//! déploiement sur une autre origine (preview, machine de développement) rend des documents
//! cohérents avec lui-même.
//!
//! ## Le plan de site est trilingue
//!
//! Chaque route de navigation y figure **trois fois** — une par langue — et chaque entrée porte
//! le groupe `xhtml:link` complet de ses traductions. C'est la seule forme qu'un moteur sait
//! lire pour associer trois URL au même contenu depuis un plan de site, et elle double le
//! `hreflang` du `<head>` sans le contredire : les deux se calculent depuis la même route nue.

use askama::Template;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::i18n::{Alternative, Langue, alternatives};
use crate::state::EtatSite;

/// Une route de navigation publiée au plan de site, avant traduction.
#[derive(Debug, Clone)]
pub struct UrlPlan {
    /// Chemin absolu **nu**, sans préfixe de langue, commençant par `/`.
    pub chemin: &'static str,
    /// Fréquence de mise à jour annoncée.
    pub frequence: &'static str,
    /// Priorité relative.
    pub priorite: &'static str,
}

/// Les routes de navigation publiées au plan de site. Les espaces `/f` et `/b` n'y sont
/// **jamais** : ce sont 255 000 fichiers, et un plan de site n'est pas un index d'assets.
pub const PLAN: [UrlPlan; 3] = [
    UrlPlan {
        chemin: "/",
        frequence: "daily",
        priorite: "1.0",
    },
    UrlPlan {
        chemin: "/medias",
        frequence: "weekly",
        priorite: "0.8",
    },
    UrlPlan {
        chemin: "/explorateur",
        frequence: "weekly",
        priorite: "0.6",
    },
];

// **Une page, une URL canonique.** Le site avait huit entrées au plan pour trois pages : les
// quatre catalogues ont fusionné dans `/medias` et les trois vues d'exploration dans
// `/explorateur` (2026-09-06, décidé par l'utilisateur). Publier huit URL pour trois pages les
// ferait concourir entre elles aux yeux d'un moteur, et diluerait la seule qui compte.
//
// `/textures`, `/modeles`, `/sons`, `/videos`, `/recherche` et `/donnees` restent **servies** :
// un lien déjà partagé mène toujours à sa page, sur sa vue. Elles ne sont simplement plus
// annoncées.

/// Une entrée rendue du plan : son URL absolue et le groupe de ses traductions.
pub struct EntreePlan {
    /// URL absolue de cette version.
    pub loc: String,
    /// Fréquence annoncée.
    pub frequence: &'static str,
    /// Priorité annoncée.
    pub priorite: &'static str,
    /// Les trois langues plus `x-default`, identiques pour les trois entrées d'une route.
    pub alternatives: Vec<Alternative>,
}

/// Développe [`PLAN`] dans les trois langues.
///
/// L'ordre est *route par route*, langues groupées : les trois versions d'une même page se
/// suivent, ce qui rend le document lisible et le diff d'une modification local.
#[must_use]
pub fn plan_trilingue(origine: &str) -> Vec<EntreePlan> {
    let mut entrees = Vec::with_capacity(PLAN.len() * Langue::TOUTES.len());
    for u in &PLAN {
        let groupe = alternatives(origine, u.chemin);
        for langue in Langue::TOUTES {
            entrees.push(EntreePlan {
                loc: langue.url(origine, u.chemin),
                frequence: u.frequence,
                priorite: u.priorite,
                alternatives: groupe.clone(),
            });
        }
    }
    entrees
}

/// Les chemins que `robots.txt` autorise explicitement — les routes, dans les trois langues.
///
/// Sans eux, `/en/textures` n'est couvert par aucun `Allow` : il resterait indexable (rien ne
/// l'interdit), mais l'intention du document ne se lirait pas, et un `Disallow` ajouté plus tard
/// l'emporterait sans qu'on y pense.
#[must_use]
pub fn chemins_autorises() -> Vec<String> {
    let mut v = Vec::new();
    for u in &PLAN {
        for langue in Langue::TOUTES {
            let chemin = langue.url("", u.chemin);
            // La racine de chaque langue : `/` est déjà couvert par `Allow: /$`.
            if chemin.is_empty() {
                continue;
            }
            v.push(chemin);
        }
    }
    v
}

/// Les agents auxquels `robots.txt` ouvre l'API JSON.
///
/// Un agent qui vient lire des données n'a rien à faire dans le HTML : l'API rend la même chose
/// déjà structurée, paginée et bornée. Les nommer un par un plutôt que d'ouvrir `/api/` à tout
/// le monde garde le budget de crawl des moteurs de recherche sur les pages, qui est ce qu'ils
/// savent indexer.
///
/// La liste est **ouverte, pas restrictive** : un agent absent d'ici garde l'accès complet aux
/// pages par la règle générale. Elle ne bloque personne — elle donne un raccourci.
pub const AGENTS_IA: [&str; 19] = [
    "GPTBot",
    "ChatGPT-User",
    "OAI-SearchBot",
    "ClaudeBot",
    "Claude-Web",
    "Claude-SearchBot",
    "anthropic-ai",
    "Google-Extended",
    "PerplexityBot",
    "Perplexity-User",
    "CCBot",
    "Bytespider",
    "Amazonbot",
    "Applebot",
    "Applebot-Extended",
    "cohere-ai",
    "FacebookBot",
    "Meta-ExternalAgent",
    "MistralAI-User",
];

#[derive(Template)]
#[template(path = "robots.txt")]
struct Robots<'a> {
    origine: &'a str,
    chemins: Vec<String>,
    agents: &'a [&'a str],
}

#[derive(Template)]
#[template(path = "llms.txt")]
struct Llms<'a> {
    origine: &'a str,
}

#[derive(Template)]
#[template(path = "llms-full.txt")]
struct LlmsComplet<'a> {
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
    urls: &'a [EntreePlan],
    lastmod: Option<String>,
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
    texte(
        Robots {
            origine: &etat.config.origine,
            chemins: chemins_autorises(),
            agents: &AGENTS_IA,
        }
        .render(),
        "text/plain; charset=utf-8",
    )
}

/// `/.well-known/security.txt`.
///
/// `Expires` est obligatoire (RFC 9116) et doit être dans le futur : il est calculé à
/// l'exécution, un an après le démarrage, plutôt que figé dans le dépôt où il périme sans
/// que personne ne le voie.
pub async fn security(State(etat): State<EtatSite>) -> Response {
    texte(
        Securite {
            origine: &etat.config.origine,
            expiration: expiration_dans_un_an(),
        }
        .render(),
        "text/plain; charset=utf-8",
    )
}

/// `/llms.txt` — l'index, au format de <https://llmstxt.org>.
///
/// Servi en `text/plain` : c'est ce que la convention demande, et c'est ce qu'un agent obtient
/// sans négocier. Le document est court **par construction** — il oriente, il ne documente pas.
pub async fn llms(State(etat): State<EtatSite>) -> Response {
    texte(
        Llms {
            origine: &etat.config.origine,
        }
        .render(),
        "text/plain; charset=utf-8",
    )
}

/// `/llms-full.txt` — la référence complète : conventions d'URL, formats, limites, exemples.
pub async fn llms_complet(State(etat): State<EtatSite>) -> Response {
    texte(
        LlmsComplet {
            origine: &etat.config.origine,
        }
        .render(),
        "text/plain; charset=utf-8",
    )
}

/// `/manifest.webmanifest` — le manifeste d'application web.
///
/// Construit avec `serde_json` plutot qu'avec un template : askama choisit son echappeur sur
/// l'extension du fichier, et `.webmanifest` ne lui dit rien. Un guillemet dans un libelle
/// traduit produirait alors du JSON invalide, que rien ne signalerait — un manifeste casse ne
/// leve pas, il est simplement ignore.
///
/// La langue vient de l'URL, comme partout ailleurs : `/ja/manifest.webmanifest` decrit la
/// meme application en japonais, avec sa propre `start_url`.
pub async fn manifeste(uri: axum::http::Uri) -> Response {
    let langue = Langue::separer(uri.path()).langue;
    // Hors macro : `json!` lit une accolade comme le debut d'un objet, pas comme un bloc.
    let depart = match langue.url("", "/").as_str() {
        "" => "/".to_owned(),
        u => u.to_owned(),
    };
    let (nom, description) = match langue {
        Langue::Fr => (
            "Aphrody",
            "Explorer, décoder et exporter les fichiers d'Inazuma Eleven: Victory Road.",
        ),
        Langue::En => (
            "Aphrody",
            "Browse, decode and export the files of Inazuma Eleven: Victory Road.",
        ),
        Langue::Ja => (
            "Aphrody",
            "イナズマイレブン Victory Road のファイルを閲覧・デコード・書き出しできます。",
        ),
    };
    let doc = serde_json::json!({
        "name": nom,
        "short_name": nom,
        "description": description,
        "lang": langue.code(),
        "start_url": depart,
        "scope": "/",
        "display": "standalone",
        "background_color": crate::routes::pages::COULEUR_THEME,
        "theme_color": crate::routes::pages::COULEUR_THEME,
        "icons": [
            { "src": "/static/icone-192.png", "sizes": "192x192", "type": "image/png", "purpose": "any" },
            { "src": "/static/icone-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any" },
        ],
    });
    texte(
        Ok(doc.to_string()),
        "application/manifest+json; charset=utf-8",
    )
}

/// `/sitemap.xml`.
pub async fn sitemap(State(etat): State<EtatSite>) -> Response {
    let urls = plan_trilingue(&etat.config.origine);
    texte(
        Plan {
            urls: &urls,
            lastmod: lastmod_du_gisement(&etat.config.db),
        }
        .render(),
        "application/xml; charset=utf-8",
    )
}

/// Date de dernière modification annoncée : celle du gisement, pas celle du démarrage.
///
/// Un `lastmod` posé à `maintenant()` à chaque requête est un mensonge que les moteurs
/// apprennent à ignorer — et une fois ignoré, il ne revient pas. Quand le fichier est
/// introuvable, l'attribut est **omis** : ne rien dire vaut mieux que dire faux.
fn lastmod_du_gisement(db: &std::path::Path) -> Option<String> {
    let modifie = std::fs::metadata(db).ok()?.modified().ok()?;
    let secondes = modifie
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    // Le jour suffit : un plan de site n'a pas de sémantique à la seconde, et une date nue
    // évite d'annoncer une fraîcheur que le contenu n'a pas.
    Some(iso8601_utc(secondes).chars().take(10).collect())
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
///
/// Visible dans la crate parce que le flux Atom en a besoin mot pour mot : un flux exige du
/// RFC 3339, le plan du site en tronque les dix premiers caractères, et les deux doivent venir
/// du même calcul — deux implémentations d'un calendrier finissent toujours par diverger.
pub(crate) fn iso8601_utc(secondes: u64) -> String {
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
        assert_eq!(PLAN.len(), 3);
        let urls = plan_trilingue("https://aphrody.com");
        assert_eq!(urls.len(), 9, "3 routes x 3 langues");
        let rendu = Plan {
            urls: &urls,
            lastmod: Some("2026-09-05".to_owned()),
        }
        .render()
        .unwrap();
        assert_eq!(rendu.matches("<url>").count(), 9);
        assert!(rendu.starts_with("<?xml"));
        assert!(rendu.contains("https://aphrody.com/medias"));
        assert!(rendu.contains("https://aphrody.com/ja/medias"));
        // Les quatre URL de catalogue restent SERVIES, mais ne sont plus annoncées : une page,
        // une URL canonique.
        assert!(!rendu.contains("https://aphrody.com/textures"));
        assert!(!rendu.contains("https://aphrody.com/videos"));
        assert!(!rendu.contains("https://aphrody.com/en/videos"));
        // L'explorateur est une entrée du site : il a sa page, donc sa place au plan.
        assert!(rendu.contains("https://aphrody.com/explorateur"));
        assert!(rendu.contains("https://aphrody.com/ja/explorateur"));
        // `/recherche` et `/donnees`, elles, n'y sont PAS : elles mènent à l'explorateur, et
        // trois URL pour une page les feraient concourir entre elles.
        assert!(!rendu.contains("https://aphrody.com/donnees"));
        assert!(!rendu.contains("https://aphrody.com/recherche"));
        // Chaque entrée porte son groupe complet : 9 x 4 liens alternatifs.
        assert_eq!(rendu.matches("xhtml:link").count(), 36);
        assert_eq!(rendu.matches("hreflang=\"x-default\"").count(), 9);
        assert_eq!(rendu.matches("<lastmod>2026-09-05</lastmod>").count(), 9);
        // L'espace de noms xhtml doit être déclaré, sinon les `xhtml:link` sont du bruit.
        assert!(rendu.contains("xmlns:xhtml=\"http://www.w3.org/1999/xhtml\""));
    }

    #[test]
    fn un_gisement_absent_n_invente_pas_de_date() {
        let urls = plan_trilingue("https://aphrody.com");
        let rendu = Plan {
            urls: &urls,
            lastmod: None,
        }
        .render()
        .unwrap();
        assert!(!rendu.contains("<lastmod>"), "mieux vaut rien que faux");
        assert_eq!(
            lastmod_du_gisement(std::path::Path::new("/nonexistent/x.sqlite")),
            None
        );
    }

    #[test]
    fn robots_pointe_le_plan() {
        let r = Robots {
            origine: "https://aphrody.com",
            chemins: chemins_autorises(),
            agents: &AGENTS_IA,
        }
        .render()
        .unwrap();
        assert!(r.contains("Sitemap: https://aphrody.com/sitemap.xml"));
        // 9 : les 5 du regime general, plus les 4 que le regime des agents repete. Un
        // `Disallow` pose dans un bloc ne vaut QUE pour ce bloc — l'oublier dans le second
        // ouvrirait les 255 000 fichiers aux agents.
        assert_eq!(r.matches("Disallow:").count(), 9);
    }

    #[test]
    fn robots_autorise_les_trois_langues() {
        let chemins = chemins_autorises();
        // 3 routes x 3 langues, moins la racine française déjà couverte par `Allow: /$`.
        assert_eq!(chemins.len(), 8);
        for attendu in ["/medias", "/en/medias", "/ja/medias", "/en", "/ja"] {
            assert!(
                chemins.iter().any(|c| c == attendu),
                "{attendu} non autorisé"
            );
        }
        let r = Robots {
            origine: "https://aphrody.com",
            chemins,
            agents: &AGENTS_IA,
        }
        .render()
        .unwrap();
        // 16 : 8 chemins + `/$` + `/llms.txt` + `/feed.atom` pour le regime general, puis les
        // 5 du regime des agents (`/`, `/llms.txt`, `/llms-full.txt`, `/feed.atom`, `/api/v1/`).
        assert_eq!(
            r.matches("Allow: ").count(),
            16,
            "les deux regimes, chemin par chemin"
        );
        assert!(r.contains("Allow: /$"));
    }

    #[test]
    fn les_agents_ont_leur_propre_regime() {
        let r = Robots {
            origine: "https://aphrody.com",
            chemins: chemins_autorises(),
            agents: &AGENTS_IA,
        }
        .render()
        .unwrap();
        // Un `User-agent:` par agent, plus le bloc general.
        assert_eq!(r.matches("User-agent:").count(), AGENTS_IA.len() + 1);
        for agent in ["GPTBot", "ClaudeBot", "PerplexityBot", "Google-Extended"] {
            assert!(
                r.contains(&format!("User-agent: {agent}")),
                "{agent} absent"
            );
        }
        // Le regime general FERME l'API, celui des agents l'OUVRE : les deux doivent coexister.
        assert!(
            r.contains("Disallow: /api/"),
            "l'API reste fermee aux moteurs"
        );
        assert!(
            r.contains("Allow: /api/v1/"),
            "l'API est ouverte aux agents"
        );
        // Les 255 000 fichiers restent hors de portee des deux regimes.
        assert_eq!(r.matches("Disallow: /f/").count(), 2);
        assert_eq!(r.matches("Disallow: /b/").count(), 2);
        assert!(r.contains("Allow: /llms.txt"));
    }

    #[test]
    fn les_documents_pour_agents_citent_l_origine_configuree() {
        let court = Llms {
            origine: "https://exemple.test",
        }
        .render()
        .unwrap();
        let complet = LlmsComplet {
            origine: "https://exemple.test",
        }
        .render()
        .unwrap();
        // Aucune origine en dur : un deploiement de preview doit se decrire lui-meme.
        for doc in [&court, &complet] {
            assert!(!doc.contains("aphrody.com"), "origine codee en dur");
            assert!(doc.contains("https://exemple.test"));
        }
        assert!(
            court.starts_with("# Aphrody"),
            "llms.txt commence par son titre"
        );
        assert!(court.contains("> "), "llms.txt porte son resume");
        // Le court oriente vers le complet, sinon personne ne le trouve.
        assert!(court.contains("/llms-full.txt"));
        // Le complet dit les trois langues et les deux espaces de fichiers.
        for attendu in ["/en/", "/ja/", "/f/", "/b/", "per_page"] {
            assert!(
                complet.contains(attendu),
                "{attendu} absent de llms-full.txt"
            );
        }
        assert!(
            complet.len() > court.len(),
            "le complet doit etre plus complet"
        );
    }

    #[tokio::test]
    async fn le_manifeste_suit_la_langue_de_l_url() {
        use axum::body::to_bytes;
        use axum::http::Uri;
        for (chemin, code, depart) in [
            ("/manifest.webmanifest", "fr", "/"),
            ("/en/manifest.webmanifest", "en", "/en"),
            ("/ja/manifest.webmanifest", "ja", "/ja"),
        ] {
            let r = manifeste(chemin.parse::<Uri>().expect("uri")).await;
            let corps = to_bytes(r.into_body(), 64 * 1024).await.expect("corps");
            let v: serde_json::Value = serde_json::from_slice(&corps).expect("json valide");
            assert_eq!(v["lang"], code, "{chemin}");
            assert_eq!(v["start_url"], depart, "{chemin}");
            assert_eq!(v["icons"].as_array().expect("icones").len(), 2);
            assert_eq!(v["theme_color"], crate::routes::pages::COULEUR_THEME);
        }
    }

    #[test]
    fn horodatage_iso8601() {
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
        // 2026-09-05T00:00:00Z
        assert_eq!(iso8601_utc(1_788_566_400), "2026-09-05T00:00:00Z");
        assert!(expiration_dans_un_an().as_str() > "2027-01-01T00:00:00Z");
    }
}
