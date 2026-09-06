//! ETag conditionnel générique — le `304` sur tout ce que la crate **génère**.
//!
//! ## Le trou, mesuré
//!
//! `tower-http` 0.6.11 n'expose **aucune** couche d'ETag, et `tower-etag-cache`, la seule crate
//! tierce du domaine, n'a pas été publiée depuis 2023. Les seules réponses conditionnelles de
//! cette crate venaient donc de [`crate::routes::static_files::reponse_octets`], que seuls
//! `/f`, `/assets` et le bundle appellent. Tout ce que la crate *fabrique* repartait entier à
//! chaque revalidation. Relevé sur la production le 2026-09-05 (`curl -w '%header{etag}'`
//! contre `127.0.0.1:8085`), douze réponses, **zéro ETag** :
//!
//! | Réponse | Octets | ETag avant |
//! |---|---|---|
//! | `/api/v1/chara?per_page=200` | 54 211 | aucun |
//! | `/api/v1/textures?per_page=200` | 20 659 | aucun |
//! | `/sitemap.xml` | 7 274 | aucun |
//! | `/llms-full.txt` | 4 012 | aucun |
//! | `/` (coquille) | 3 702 | aucun |
//! | `/llms.txt` | 2 280 | aucun |
//! | `/robots.txt` | 1 335 | aucun |
//!
//! La coquille est le cas le plus net : elle est servie `max-age=60`, c'est-à-dire qu'elle est
//! **faite** pour être revalidée — et une revalidation sans validateur ne peut rien rendre
//! d'autre qu'un corps entier. Un `Cache-Control` court sans ETag ne fait pas économiser un
//! octet ; il ne fait qu'annoncer la fraîcheur.
//!
//! ## Ce que la couche ne fait pas
//!
//! Elle ne remplace jamais un ETag déjà posé : `reponse_octets` en pose un, calculé sur les
//! octets **servis** (donc sur la variante `br`/`zstd` réellement envoyée), ce qu'une couche
//! générique placée au-dessus ne saurait pas faire. La présence de l'en-tête est testée
//! **avant** de toucher au corps — un fichier du jeu de 30 Mio n'est jamais rebufferisé ici.
//!
//! Elle ne bufferise pas non plus un corps de taille inconnue : un corps sans
//! `size_hint().exact()` (un flux) passe intact. Aucune route de la crate n'est dans ce cas
//! aujourd'hui, et c'est ce qui doit rester vrai le jour où l'une le deviendra.

use axum::body::{Body, HttpBody as _};
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Taille au-delà de laquelle un corps n'est pas condensé.
///
/// Rien de ce que la crate génère ne s'en approche : le plus gros relevé est
/// `/api/v1/chara?per_page=200`, à 54 211 octets, soit 0,6 % de ce plafond. Il n'existe donc
/// que pour borner ce qu'une route future pourrait produire — une couche générique qui
/// bufferise sans limite est une couche qui finit par bufferiser la mauvaise réponse.
pub const TAILLE_MAX: u64 = 8 * 1024 * 1024;

/// Dit si un client déjà porteur de `If-None-Match` détient cette représentation.
///
/// `*` vaut pour « n'importe quelle représentation existante » (RFC 9110 §13.1.2) ; sinon la
/// comparaison est faible au sens de la RFC, c'est-à-dire entité par entité après découpe sur
/// la virgule. Un `W/` préfixé est comparé sans son préfixe : nos ETag sont forts, mais un
/// intermédiaire a le droit de dégrader ce qu'il retransmet, et le refuser rendrait un corps
/// entier là où un `304` suffisait.
#[must_use]
pub fn correspond(si_aucun: &str, etag: &str) -> bool {
    let nu = |e: &str| e.trim().trim_start_matches("W/").to_owned();
    let attendu = nu(etag);
    si_aucun.split(',').any(|e| {
        let e = e.trim();
        e == "*" || nu(e) == attendu
    })
}

/// Couche : pose un ETag `blake3` sur les réponses générées, et rend `304` quand le client
/// détient déjà la représentation.
pub async fn conditionnel(requete: Request, suite: Next) -> Response {
    // Les deux valeurs sont relevées AVANT que la requête ne parte dans la pile : elle est
    // consommée par `run`, et son en-tête ne serait plus lisible ensuite.
    let methode = requete.method().clone();
    let si_aucun = requete
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    let reponse = suite.run(requete).await;

    // Un ETag ne qualifie qu'une lecture réussie. Sur une erreur, il ferait cacher le message
    // d'erreur ; sur une redirection, il n'a aucun sens.
    if (methode != Method::GET && methode != Method::HEAD) || reponse.status() != StatusCode::OK {
        return reponse;
    }
    // Testé avant de toucher au corps : c'est ce qui garantit que `/f` et `/assets`, qui
    // posent le leur sur les octets réellement servis, ne sont jamais rebufferisés ici.
    if reponse.headers().contains_key(header::ETAG) {
        return reponse;
    }
    let Some(taille) = reponse.body().size_hint().exact() else {
        return reponse;
    };
    if taille > TAILLE_MAX {
        return reponse;
    }

    let (mut parties, corps) = reponse.into_parts();
    let octets = match axum::body::to_bytes(corps, TAILLE_MAX as usize).await {
        Ok(o) => o,
        Err(e) => {
            // Le corps a été consommé : il n'y a plus de réponse à rendre intacte. Le cas est
            // inatteignable — la taille exacte vient d'être vérifiée sous le plafond — mais un
            // corps tronqué servi en 200 serait pire que ce 500.
            tracing::error!(erreur = %e, "corps illisible lors du calcul de l'ETag");
            return crate::ErreurSite::Interne("reponse illisible".to_owned()).into_response();
        }
    };

    let etag = crate::routes::static_files::etiquette(&octets);
    let detenu = si_aucun.is_some_and(|v| correspond(&v, &etag));

    if let Ok(v) = HeaderValue::from_str(&etag) {
        parties.headers.insert(header::ETAG, v);
    }
    if detenu {
        parties.status = StatusCode::NOT_MODIFIED;
        // Un `304` ne porte pas de corps : annoncer la longueur de celui qu'on ne rend pas
        // laisse certains clients attendre des octets qui n'arriveront jamais.
        parties.headers.remove(header::CONTENT_LENGTH);
        return Response::from_parts(parties, Body::empty());
    }
    Response::from_parts(parties, Body::from(octets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparaison_des_etiquettes() {
        let e = "\"abc\"";
        assert!(correspond(e, e));
        assert!(
            correspond("*", e),
            "l'etoile vaut pour toute representation"
        );
        assert!(
            correspond("\"x\", \"abc\" , \"y\"", e),
            "liste, espaces compris"
        );
        assert!(
            correspond("W/\"abc\"", e),
            "un intermediaire a le droit de degrader"
        );
        assert!(!correspond("\"abcd\"", e), "prefixe n'est pas egalite");
        assert!(!correspond("", e));
        assert!(!correspond("\"x\",\"y\"", e));
    }

    /// Un routeur minimal portant les trois cas que la couche doit distinguer.
    fn routeur_temoin() -> axum::Router {
        use axum::routing::get;
        axum::Router::new()
            .route(
                "/json",
                get(|| async { axum::Json(serde_json::json!({"a": 1, "b": "é"})) }),
            )
            // Une route qui pose DÉJÀ son ETag — comme `reponse_octets` le fait pour `/f` et
            // le bundle. La couche doit la laisser strictement intacte.
            .route(
                "/deja",
                get(|| async { ([(header::ETAG, "\"fige\"")], "corps deja etiquete") }),
            )
            // Une erreur : jamais d'ETag, sans quoi le message d'erreur se ferait cacher.
            .route(
                "/erreur",
                get(|| async { (StatusCode::NOT_FOUND, "absent") }),
            )
            .layer(axum::middleware::from_fn(conditionnel))
    }

    async fn appel(uri: &str, si_aucun: Option<&str>) -> (StatusCode, Option<String>, usize) {
        use http_body_util::BodyExt as _;
        use tower::ServiceExt as _;
        let mut b = axum::http::Request::builder().uri(uri);
        if let Some(v) = si_aucun {
            b = b.header(header::IF_NONE_MATCH, v);
        }
        let r = routeur_temoin()
            .oneshot(b.body(Body::empty()).expect("requete"))
            .await
            .expect("reponse");
        let statut = r.status();
        let etag = r
            .headers()
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let corps = r
            .into_body()
            .collect()
            .await
            .expect("corps")
            .to_bytes()
            .len();
        (statut, etag, corps)
    }

    #[tokio::test]
    async fn la_couche_etiquette_puis_rend_304() {
        let (statut, etag, taille) = appel("/json", None).await;
        assert_eq!(statut, StatusCode::OK);
        let etag = etag.expect("un ETag est pose sur une reponse generee");
        // `blake3` en hexadecimal : 64 caracteres entre guillemets.
        assert_eq!(etag.len(), 66, "\"{{64 hexa}}\"");
        assert_eq!(
            taille, 16,
            "{{\"a\":1,\"b\":\"é\"}} fait 16 octets en UTF-8"
        );

        // Le meme corps redonne la meme etiquette : sans cela, aucun 304 ne serait jamais rendu.
        let (_, encore, _) = appel("/json", None).await;
        assert_eq!(
            encore.as_deref(),
            Some(etag.as_str()),
            "l'etiquette est deterministe"
        );

        // Le client la presente : 304, zero octet, et l'etiquette rappelee (RFC 9110 §15.4.5).
        let (statut, rappel, taille) = appel("/json", Some(&etag)).await;
        assert_eq!(statut, StatusCode::NOT_MODIFIED);
        assert_eq!(taille, 0, "un 304 ne porte pas de corps");
        assert_eq!(rappel.as_deref(), Some(etag.as_str()));

        // Une etiquette perimee ne fait rien economiser : corps entier, code 200.
        let (statut, _, taille) = appel("/json", Some("\"perimee\"")).await;
        assert_eq!(statut, StatusCode::OK);
        assert_eq!(taille, 16);
    }

    #[tokio::test]
    async fn la_couche_ne_touche_ni_aux_etiquettes_posees_ni_aux_erreurs() {
        // `/f`, `/assets` et le bundle calculent leur ETag sur les octets REELLEMENT servis
        // (donc sur la variante `br`/`zstd`) : la couche generique ne saurait pas le refaire,
        // et l'ecraser servirait une etiquette qui ne decrit pas ce qui est dans le tuyau.
        let (statut, etag, taille) = appel("/deja", None).await;
        assert_eq!(statut, StatusCode::OK);
        assert_eq!(
            etag.as_deref(),
            Some("\"fige\""),
            "etiquette de la route conservee"
        );
        assert_eq!(taille, "corps deja etiquete".len());
        // Et elle continue de fonctionner : c'est `reponse_octets` qui rend le 304, pas nous.
        let (statut, _, _) = appel("/deja", Some("\"fige\"")).await;
        assert_eq!(
            statut,
            StatusCode::OK,
            "la couche ne s'immisce pas dans le 304 d'autrui"
        );

        // Une erreur ne recoit jamais d'etiquette : elle serait cachee comme une reponse.
        let (statut, etag, _) = appel("/erreur", None).await;
        assert_eq!(statut, StatusCode::NOT_FOUND);
        assert_eq!(etag, None, "aucun ETag sur autre chose qu'un 200");
    }
}
