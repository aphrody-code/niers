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

/// Déclare les routes **une seule fois**, et en tire deux sorties : le montage du routeur et la
/// liste de leurs chemins.
///
/// Ce qui l'a rendue nécessaire, mesuré : `ROUTES` était une constante tenue à la main que
/// `tests/routes.rs` figeait à 19 entrées, alors que le routeur en montait 37. Les sept routes
/// d'`/pet`, les cinq de la 3D puis les six de Lua et des formats y avaient été ajoutées sans
/// entrer dans la liste — chaque lot ayant respecté son périmètre, et la liste n'appartenant à
/// aucun. **Un inventaire qui ne suit pas ce qu'il inventorie n'est pas une garde, c'est un
/// faux document** : il annonçait 19 routes servies sur un site qui en sert 37.
///
/// La macro supprime la classe entière de défaut : une route ajoutée ici est montée **et**
/// listée, une route retirée disparaît des deux. Aucun ordre de déclaration à maintenir, aucune
/// discipline à demander au prochain lot.
macro_rules! declarer_routes {
    ($($chemin:literal => $handler:path),+ $(,)?) => {
        /// Les chemins réellement montés, dans l'ordre de déclaration.
        ///
        /// Cette liste **est** celle du routeur : elles descendent de la même déclaration, et
        /// aucune ne peut être modifiée sans l'autre.
        #[must_use]
        pub fn chemins() -> Vec<&'static str> {
            vec![$($chemin),+]
        }

        /// Monte les routes déclarées sur un routeur nu.
        fn monter(routeur: Router<EtatSite>) -> Router<EtatSite> {
            routeur $(.route($chemin, get($handler)))+
        }
    };
}

// Toutes les routes du site sont en `GET` : le site ne prend aucune écriture, et c'est
// volontaire — la seule chose qu'un visiteur puisse faire est lire. La macro le rend
// structurel plutôt que conventionnel.
declarer_routes! {
    "/healthz" => crate::routes::health::healthz,
    "/robots.txt" => crate::routes::well_known::robots,
    "/llms.txt" => crate::routes::well_known::llms,
    "/llms-full.txt" => crate::routes::well_known::llms_complet,
    // Une route par langue, declarees une par une. Un parametre `/{langue}/manifest…`
    // capturerait n'importe quel segment et servirait le manifeste francais sous autant
    // d'URL qu'on peut en inventer.
    "/manifest.webmanifest" => crate::routes::well_known::manifeste,
    "/en/manifest.webmanifest" => crate::routes::well_known::manifeste,
    "/ja/manifest.webmanifest" => crate::routes::well_known::manifeste,
    "/.well-known/security.txt" => crate::routes::well_known::security,
    "/sitemap.xml" => crate::routes::well_known::sitemap,
    "/feed.atom" => crate::routes::feed::atom,
    "/api/v1/health" => crate::routes::api_v1::health,
    "/api/v1/chara" => crate::routes::api_v1::chara,
    "/api/v1/{vue}" => crate::routes::api_v1::vue,
    "/f/{*chemin}" => crate::routes::vfs::fichier,
    "/b" => crate::routes::vfs::parcours_racine,
    "/b/{*prefixe}" => crate::routes::vfs::parcours,
    "/api/v1/episodes" => crate::routes::episodes::episodes,
    "/assets/{*chemin}" => crate::routes::assets::assets,
    // Aphrody, le personnage du site. Sept routes explicites plutot qu'un
    // `/pet/{*fichier}` : le package n'est pas un dossier de fichiers, et un joker
    // inviterait a en deriver un espace qui n'existe pas. Cf. `routes::aphrody`.
    "/pet/aphrody.json" => crate::routes::aphrody::manifeste,
    "/pet/atlas.webp" => crate::routes::aphrody::atlas,
    "/pet/aphrody.svg" => crate::routes::aphrody::svg,
    "/pet/frame/{animation}/{fichier}" => crate::routes::aphrody::frame,
    "/api/v1/aphrody" => crate::routes::aphrody::dossier,
    "/api/v1/aphrody/diagnostic" => crate::routes::aphrody::diagnostic,
    "/api/v1/aphrody/palette" => crate::routes::aphrody::palette,
    // La couche 3D. Cinq routes, deux espaces : `/api/v1/3d` DECRIT (capacites, catalogue,
    // fiche, geometrie mesuree), `/model` SERT (le GLB assemble, l'apercu rendu). Un
    // catalogue qui rendrait aussi les octets melangerait deux durees de cache et deux
    // politiques d'erreur — un catalogue absent est un 503, un modele absent un 404.
    //
    // `/api/v1/3d` est declare AVANT `/api/v1/{vue}` : matchit prefere de toute facon le
    // segment litteral au parametre, mais l'ordre de lecture doit dire la meme chose que
    // l'ordre de resolution.
    "/api/v1/3d" => crate::routes::modeles3d::capacites,
    "/api/v1/3d/modeles" => crate::routes::modeles3d::catalogue,
    "/api/v1/3d/modeles/{famille}/{code}" => crate::routes::modeles3d::fiche,
    "/api/v1/3d/modeles/{famille}/{code}/analyse" => crate::routes::modeles3d::analyse,
    "/model/{famille}/{fichier}" => crate::routes::modeles3d::modele,
    // La couche Lua et la couche formats. Elles sont declarees AVANT `/api/v1/{vue}`
    // pour la meme raison que `/api/v1/3d` : matchit prefere de toute facon le segment
    // litteral au parametre, mais l'ordre de lecture doit dire ce que fait le routeur.
    //
    // Le desassemblage a son PROPRE prefixe au lieu d'etre un suffixe de `/scripts` : un
    // joker (`{*chemin}`) est terminal chez axum, et `/scripts/{*chemin}/desassemblage`
    // ne se declare pas. Cf. `routes::lua`.
    "/api/v1/lua" => crate::routes::lua::capacites,
    "/api/v1/lua/scripts" => crate::routes::lua::scripts,
    "/api/v1/lua/scripts/{*chemin}" => crate::routes::lua::script,
    "/api/v1/lua/desassemblage/{*chemin}" => crate::routes::lua::desassemblage,
    "/api/v1/formats" => crate::routes::formats::capacites,
    "/api/v1/formats/decode/{*chemin}" => crate::routes::formats::decode,
    // Chercher un fichier dans TOUT le VFS. `/b` ne filtre qu'un niveau — verifie :
    // `/b/data?q=chara_base` rend 0. Cf. `routes::recherche`.
    "/api/v1/recherche" => crate::routes::recherche::recherche,
    // Les donnees du jeu, en structures NOMMEES. Distincte de `/formats/decode`, qui rend la
    // structure generique du conteneur : un consommateur typé qui lit le générique y trouve
    // zero element en annoncant un succes. Cf. `routes::donnees`.
    "/api/v1/donnees" => crate::routes::donnees::capacites,
    "/api/v1/donnees/{*chemin}" => crate::routes::donnees::donnees,
    // La matrice de couverture du plan (§ 4). Elle est LUE, jamais mesuree ici : mesurer,
    // c'est lancer `niers --help`, lire quatre arbres de sources et parcourir 255 308 lignes
    // d'inventaire. Cf. `routes::couverture`.
    "/couverture" => crate::routes::couverture::page,
    "/api/v1/couverture" => crate::routes::couverture::json,
    "/" => crate::routes::pages::coquille,
}

/// Dit si un motif de route d'axum reconnaît une URI concrète.
///
/// Elle vit **ici** et non dans les tests parce que la matrice de couverture en dépend : une
/// règle de classement cite la route qui sert une capacité, et [`crate::couverture::construire`]
/// rétrograde en `manquant` toute capacité dont la route n'est montée nulle part. Sans cette
/// fonction, la matrice se croirait sur parole — et une matrice qu'on ne peut pas contredire
/// n'est pas un instrument de mesure.
///
/// Elle reproduit la règle de `matchit` telle qu'axum 0.8 l'emploie : un segment `{param}`
/// consomme exactement un segment non vide, un `{*joker}` consomme tout le reste (au moins un
/// segment), et tout autre segment doit être égal.
#[must_use]
pub fn correspond(motif: &str, uri: &str) -> bool {
    let mut segments_uri = uri.trim_start_matches('/').split('/');
    let segments_motif: Vec<&str> = motif.trim_start_matches('/').split('/').collect();
    for m in &segments_motif {
        if m.starts_with("{*") {
            return segments_uri.next().is_some();
        }
        match segments_uri.next() {
            None => return false,
            Some(s) => {
                if m.starts_with('{') {
                    if s.is_empty() {
                        return false;
                    }
                } else if *m != s {
                    return false;
                }
            }
        }
    }
    segments_uri.next().is_none()
}

/// Construit le routeur complet : les routes déclarées ci-dessus, le repli statique et les
/// couches.
///
/// Syntaxe de route d'axum 0.8 : `{param}` et `{*wildcard}`. L'ancienne forme (`:id`, `*path`)
/// **panique** au `route()` — elle ne dégrade pas.
pub fn routeur(etat: EtatSite) -> Router {
    monter(Router::new())
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
        .layer(axum::middleware::from_fn_with_state(
            etat.clone(),
            crate::debit::limiter,
        ))
        .layer(axum::middleware::from_fn(entetes_securite))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            DELAI_REQUETE,
        ))
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
            assert!(
                directives.contains(&attendue),
                "directive absente: {attendue}"
            );
        }
        assert!(
            !CSP.contains(" 'unsafe-eval'"),
            "pas d'eval JavaScript (wasm excepte)"
        );
        assert!(
            !CSP.contains("script-src 'self' 'unsafe-inline'"),
            "pas de script inline"
        );
        assert_eq!(entetes_securite_liste().len(), NB_ENTETES_SECURITE);
    }

    #[test]
    fn correspondance_de_motif() {
        assert!(correspond("/f/{*chemin}", "/f/data/common/x.g4tx"));
        assert!(!correspond("/f/{*chemin}", "/f"));
        assert!(correspond("/api/v1/{vue}", "/api/v1/textures"));
        assert!(!correspond("/api/v1/{vue}", "/api/v1/textures/1"));
        assert!(correspond("/healthz", "/healthz"));
        assert!(!correspond("/healthz", "/healthz/x"));
        // Un motif ne reconnait pas un segment vide : `/api/v1/` n'est pas `/api/v1/{vue}`.
        assert!(!correspond("/api/v1/{vue}", "/api/v1/"));
    }

    #[test]
    fn contrat_de_routes() {
        let routes = chemins();
        assert_eq!(routes.len(), 42, "42 routes montees");
        for r in &routes {
            assert!(r.starts_with('/'), "{r}");
            // Syntaxe axum 0.7 (`:id`, `*path`) : elle PANIQUE au `route()`, elle ne degrade
            // pas. Un test la refuse ici plutot qu'au demarrage du service.
            assert!(!r.contains(":{"), "syntaxe axum 0.7 interdite: {r}");
            assert!(!r.contains("/:"), "syntaxe axum 0.7 interdite: {r}");
        }
        // Aucun doublon : deux fois le meme chemin, et c'est la seconde declaration qui
        // gagnerait en silence.
        let mut tries = routes.clone();
        tries.sort_unstable();
        tries.dedup();
        assert_eq!(tries.len(), routes.len(), "chemin declare deux fois");
    }
}
