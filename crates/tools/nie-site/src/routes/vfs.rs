//! Les deux espaces d'adressage du jeu — amendement A3.
//!
//! - `/f/<chemin VFS verbatim>` : **une** ressource. Le chemin est un **segment**, jamais une
//!   query, et l'extension du jeu est conservée
//!   (`/f/data/common/chr/_face/01_IE1/c01000010/c01000010.g4md`). Toute conversion est un
//!   suffixe ou un paramètre explicite — jamais une réécriture du nom.
//! - `/b/<préfixe VFS>` : le **parcours** d'un dossier, en JSON (sous-dossiers et fichiers).
//!
//! Cela corrige deux verrues mesurées de `nie-model-serve` : `/vfs/*` qui prend `?path=` quand
//! les autres routes prennent un segment, et `/tex/` qui exige de retirer `.g4tx`.
//!
//! Le slug d'une entité est son **code de jeu** (`c01000010`), jamais un nom traduit : 6 168
//! personnages ne portent que 5 199 `base_slug` distincts, et `unknown` y sert 65 fois. Les
//! noms restent affichés et cherchables ; ils ne sont jamais adressés.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use bytes::Bytes;

use crate::error::ErreurSite;
use crate::routes::DemandePage;
use crate::routes::static_files::{Encodage, etiquette, reponse_octets};
use crate::state::{EtatSite, ReponseCachee};
use crate::vfs_index::Dossier;

/// Taille au-delà de laquelle une ressource du jeu n'est plus gardée en mémoire.
pub const TAILLE_MAX_CACHE: usize = 2 * 1024 * 1024;

/// `Cache-Control` d'une ressource du jeu : le contenu d'un chemin ne change qu'avec une mise
/// à jour du jeu, mais il **change** — donc une journée, pas `immutable`.
pub const CONTROLE: &str = "public, max-age=86400, stale-while-revalidate=604800";

/// Normalise un chemin VFS reçu en segment.
///
/// Le wildcard d'axum 0.8 ne rend **pas** le `/` initial : `GET /f/data/a.g4tx` donne
/// `"data/a.g4tx"`. On refuse tout ce qui remonterait ailleurs (`..`, chemin absolu,
/// composant vide) plutôt que de le nettoyer en silence.
///
/// # Errors
///
/// `Demande` quand le chemin est vide ou sort de l'espace VFS.
pub fn normaliser(brut: &str) -> Result<String, ErreurSite> {
    let brut = brut.trim_start_matches('/');
    if brut.is_empty() {
        return Err(ErreurSite::Demande("chemin VFS vide".to_owned()));
    }
    if brut.contains('\0') || brut.contains('\\') {
        return Err(ErreurSite::Demande("chemin VFS invalide".to_owned()));
    }
    let mut segments = Vec::new();
    for s in brut.split('/') {
        match s {
            "" | "." => return Err(ErreurSite::Demande("chemin VFS non normalise".to_owned())),
            ".." => return Err(ErreurSite::Demande("chemin VFS sortant".to_owned())),
            autre => segments.push(autre),
        }
    }
    Ok(segments.join("/"))
}

/// Normalise un préfixe de parcours. Le préfixe vide est licite : c'est la racine du VFS.
///
/// # Errors
///
/// `Demande` quand le préfixe sort de l'espace VFS.
pub fn normaliser_prefixe(brut: &str) -> Result<String, ErreurSite> {
    let brut = brut.trim_matches('/');
    if brut.is_empty() {
        return Ok(String::new());
    }
    normaliser(brut)
}

/// Type de contenu d'une ressource du jeu.
///
/// Les formats Level-5 n'ont pas de type IANA : ils sortent en `application/octet-stream`,
/// avec un `Content-Disposition` qui conserve leur nom **et leur extension d'origine** — c'est
/// ce nom-là que l'utilisateur doit retrouver sur son disque.
#[must_use]
pub fn type_contenu(chemin: &str) -> &'static str {
    let ext = chemin
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "dds" => "image/vnd-ms.dds",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "json" => "application/json",
        "txt" | "csv" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// `GET /f/{*chemin}` — une ressource du jeu, octets bruts, extension conservée.
pub async fn fichier(
    State(etat): State<EtatSite>,
    Path(chemin): Path<String>,
    entetes: HeaderMap,
) -> Result<Response, ErreurSite> {
    let chemin = normaliser(&chemin)?;
    let index = etat.index()?;
    if !index.contient(&chemin) {
        return Err(ErreurSite::Introuvable(format!(
            "chemin absent du VFS: {chemin}"
        )));
    }

    let cle = format!("vfs:{chemin}");
    let cachee = if let Some(c) = etat.cache.get(&cle).await {
        c
    } else {
        let vfs = etat.vfs()?;
        let a_lire = chemin.clone();
        let octets = tokio::task::spawn_blocking(move || vfs.read(&a_lire))
            .await?
            .map_err(|e| {
                tracing::debug!(erreur = %e, chemin = %chemin, "lecture VFS impossible");
                ErreurSite::Introuvable(
                    "ressource indexee mais illisible sur ce montage".to_owned(),
                )
            })?;
        let taille = octets.len();
        let corps = Bytes::from(octets);
        let c = ReponseCachee {
            etag: etiquette(&corps),
            type_contenu: type_contenu(&chemin).to_owned(),
            corps,
        };
        if taille <= TAILLE_MAX_CACHE {
            etat.cache.insert(cle, c.clone()).await;
        }
        c
    };

    let mut reponse = reponse_octets(&cachee, CONTROLE, Encodage::Identite, &entetes);
    let nom = chemin.rsplit('/').next().unwrap_or(&chemin);
    if let Ok(v) = HeaderValue::from_str(&format!("inline; filename=\"{nom}\"")) {
        reponse.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(reponse)
}

/// `GET /b/{*prefixe}` — parcours d'un dossier du VFS.
pub async fn parcours(
    State(etat): State<EtatSite>,
    Path(prefixe): Path<String>,
    Query(demande): Query<DemandePage>,
) -> Result<Json<Dossier>, ErreurSite> {
    parcourir(&etat, &prefixe, demande)
}

/// `GET /b` — parcours de la racine du VFS.
pub async fn parcours_racine(
    State(etat): State<EtatSite>,
    Query(demande): Query<DemandePage>,
) -> Result<Json<Dossier>, ErreurSite> {
    parcourir(&etat, "", demande)
}

fn parcourir(
    etat: &EtatSite,
    prefixe: &str,
    demande: DemandePage,
) -> Result<Json<Dossier>, ErreurSite> {
    let prefixe = normaliser_prefixe(prefixe)?;
    let index = etat.index()?;
    let p = demande.bornee();
    Ok(Json(index.dossier(
        &prefixe,
        p.offset(),
        p.per_page as usize,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisation_refuse_les_sorties() {
        assert_eq!(normaliser("data/a.g4tx").unwrap(), "data/a.g4tx");
        assert_eq!(normaliser("/data/a.g4tx").unwrap(), "data/a.g4tx");
        for mauvais in ["", "..", "data/../..", "data//a", "data/./a", "data\\a"] {
            assert!(
                normaliser(mauvais).is_err(),
                "{mauvais} aurait du etre refuse"
            );
        }
        assert_eq!(normaliser("..").unwrap_err().statut().as_u16(), 400);
    }

    #[test]
    fn prefixe_vide_est_la_racine() {
        assert_eq!(normaliser_prefixe("").unwrap(), "");
        assert_eq!(normaliser_prefixe("/").unwrap(), "");
        assert_eq!(normaliser_prefixe("/data/dx11/").unwrap(), "data/dx11");
    }

    #[test]
    fn types_des_formats_du_jeu() {
        assert_eq!(type_contenu("a/b.png"), "image/png");
        assert_eq!(type_contenu("a/b.g4tx"), "application/octet-stream");
        assert_eq!(type_contenu("a/b"), "application/octet-stream");
    }
}
