//! `/assets/*` — proxy durci vers `nie-model-serve` (`127.0.0.1:8790`).
//!
//! Le décodage (G4TX → PNG, ACB → WAV, assemblage GLB) reste chez `nie-model-serve` : cette
//! crate ne réimplémente rien. Elle en fait en revanche un amont **borné**, ce qu'il n'est pas
//! par lui-même :
//!
//! - concurrence plafonnée (`tower::limit` côté couche, sémaphore côté requête) : un pic de
//!   trafic ne s'y transforme pas en effondrement ;
//! - délai maximal de 10 s, appliqué par le client — un amont qui accepte la connexion sans
//!   jamais répondre (cas observé le 2026-09-05) rend un `504`, pas une connexion pendante ;
//! - taille de réponse **bornée** : au-delà, la réponse est refusée plutôt que bufferisée ;
//! - cache `moka` par clé canonique, ETag `blake3`, `304` sur `If-None-Match`.

use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};

use crate::error::ErreurSite;
use crate::routes::static_files::{Encodage, etiquette, reponse_octets};
use crate::state::{EtatSite, ReponseCachee};

/// `Cache-Control` des rendus d'amont : le décodage d'un chemin donné est déterministe, mais
/// le décodeur évolue — une heure de fraîcheur, une journée de service dégradé toléré.
pub const CONTROLE: &str = "public, max-age=3600, stale-while-revalidate=86400";

/// `GET /assets/{*chemin}` — le fichier du bundle s'il existe, l'amont sinon.
///
/// Les deux cohabitent sous le même préfixe parce que Vite écrit ses fichiers empreintés dans
/// `dist/assets/` : servir le bundle d'abord évite de renommer quoi que ce soit côté
/// `apps/nie-web`, et un chemin de bundle ne peut pas être un chemin d'amont (il porte une
/// empreinte, l'autre un chemin VFS).
pub async fn assets(
    State(etat): State<EtatSite>,
    Path(chemin): Path<String>,
    query: RawQuery,
    entetes: HeaderMap,
) -> Response {
    let relatif = format!("assets/{}", chemin.trim_start_matches('/'));
    if let Some(r) = crate::routes::static_files::servir(&etat, &relatif, &entetes).await {
        return r;
    }
    match proxy(State(etat), Path(chemin), query, entetes).await {
        Ok(r) => r,
        Err(e) => e.into_response(),
    }
}

/// Relaie vers l'amont, avec cache et ETag.
///
/// # Errors
///
/// `Demande` sur chemin invalide, `Delai` quand l'amont ne répond pas dans le délai imparti,
/// `Amont` quand il répond mal ou trop gros, `Introuvable` quand il rend 404.
pub async fn proxy(
    State(etat): State<EtatSite>,
    Path(chemin): Path<String>,
    RawQuery(query): RawQuery,
    entetes: HeaderMap,
) -> Result<Response, ErreurSite> {
    let chemin = crate::routes::vfs::normaliser(&chemin)?;
    let query = query.filter(|q| !q.is_empty());
    let cle = match &query {
        Some(q) => format!("amont:{chemin}?{q}"),
        None => format!("amont:{chemin}"),
    };

    if let Some(cachee) = etat.cache.get(&cle).await {
        return Ok(reponse_octets(
            &cachee,
            CONTROLE,
            Encodage::Identite,
            &entetes,
        ));
    }

    let url = match &query {
        Some(q) => format!("{}/{chemin}?{q}", etat.config.amont),
        None => format!("{}/{chemin}", etat.config.amont),
    };

    // Le sémaphore borne le nombre d'appels simultanés à l'amont. Il est acquis AVANT la
    // requête et relâché à la fin de la fonction : un amont lent fait attendre, il n'écroule pas.
    let _jeton = etat
        .jetons_amont
        .acquire()
        .await
        .map_err(|_| ErreurSite::Interne("limiteur d'amont ferme".to_owned()))?;

    let reponse = etat.client.get(&url).send().await.map_err(|e| {
        if e.is_timeout() || e.is_connect() && e.to_string().contains("timed out") {
            ErreurSite::Delai(format!(
                "nie-model-serve n'a pas repondu en {}s",
                etat.config.delai_amont.as_secs()
            ))
        } else {
            tracing::warn!(erreur = %e, url = %url, "amont injoignable");
            ErreurSite::Amont("nie-model-serve injoignable".to_owned())
        }
    })?;

    let statut = reponse.status();
    if statut == reqwest::StatusCode::NOT_FOUND {
        return Err(ErreurSite::Introuvable(format!(
            "asset inconnu de l'amont: {chemin}"
        )));
    }
    if !statut.is_success() {
        return Err(ErreurSite::Amont(format!(
            "nie-model-serve a repondu {}",
            statut.as_u16()
        )));
    }

    // Une réponse annoncée trop grosse est refusée avant même d'être lue.
    if let Some(taille) = reponse.content_length()
        && taille > etat.config.taille_max_amont as u64
    {
        return Err(ErreurSite::Amont(format!(
            "reponse d'amont trop grosse ({taille} octets, plafond {})",
            etat.config.taille_max_amont
        )));
    }

    let type_contenu = reponse
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();

    let corps = reponse.bytes().await.map_err(|e| {
        if e.is_timeout() {
            ErreurSite::Delai("corps d'amont tronque par le delai".to_owned())
        } else {
            ErreurSite::Amont("corps d'amont illisible".to_owned())
        }
    })?;
    // Ceinture ET bretelles : un amont peut mentir sur `Content-Length` (ou n'en donner aucun).
    if corps.len() > etat.config.taille_max_amont {
        return Err(ErreurSite::Amont(format!(
            "reponse d'amont trop grosse ({} octets)",
            corps.len()
        )));
    }

    let cachee = ReponseCachee {
        etag: etiquette(&corps),
        type_contenu,
        corps,
    };
    etat.cache.insert(cle, cachee.clone()).await;
    Ok(reponse_octets(
        &cachee,
        CONTROLE,
        Encodage::Identite,
        &entetes,
    ))
}
