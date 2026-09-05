//! `/api/v1/episodes` — le catalogue de la série, pour les Inacord déjà installés.
//!
//! ## Pourquoi cette route existe
//!
//! L'installeur d'Inacord embarque `data/anime/episodes.db` : un catalogue figé au jour du
//! build. La série continue d'être publiée, et le cron du VPS rafraîchit la base chaque nuit.
//! Sans porte de sortie, la seule façon de mettre à jour une installation serait de la
//! réinstaller.
//!
//! Cette porte existait sur le wiki (`apps/azalee/app/api/ietv`). Elle en sort, parce qu'elle
//! lit un fichier local et que le wiki devient serverless — et **elle doit exister ici AVANT
//! que le wiki ne s'arrête**, faute de quoi les clients installés cessent silencieusement de
//! recevoir les nouveaux épisodes : leur repli rend un 503, qu'ils lisent comme « ce serveur ne
//! moissonne pas la série ».
//!
//! ## Ce qu'elle sert, et ce qu'elle ne sert pas
//!
//! Du **JSON**, jamais le fichier SQLite. Remplacer sous les pieds d'une application une base
//! qu'elle tient ouverte est le genre de manœuvre qui ne casse qu'une fois sur dix, et jamais
//! sur la machine où on l'a testée. Le client fusionne ligne à ligne et garde la main.
//!
//! `?since=<epoch ms>` ne rend que ce qui a été moissonné après cette date. Un client à jour
//! reçoit alors un tableau vide et quelques centaines d'octets.

use axum::Json;
use axum::extract::{Query, State};
use rusqlite::OpenFlags;
use serde::{Deserialize, Serialize};

use crate::error::ErreurSite;
use crate::state::EtatSite;

/// Borne haute du nombre d'épisodes rendus en une fois.
///
/// La base en compte 1 141 : le catalogue entier tient donc largement sous cette limite, et
/// elle ne sert qu'à empêcher qu'une base future ne fasse rendre un corps sans fin.
pub const LIMITE_MAX: u32 = 20_000;

/// Nombre d'épisodes rendus quand le client n'en demande pas un nombre précis.
pub const LIMITE_DEFAUT: u32 = 5_000;

/// Paramètres acceptés par la route.
#[derive(Debug, Deserialize)]
pub struct Demande {
    /// Ne rendre que ce qui a été moissonné après cette date (epoch ms).
    #[serde(default)]
    pub since: i64,
    /// Nombre maximal d'épisodes. Borné par [`LIMITE_MAX`].
    pub limit: Option<u32>,
}

/// Un épisode, tel que la base le décrit.
///
/// Les noms de champs sont ceux des colonnes réelles de `episodes` — relevés par
/// `PRAGMA table_info`, jamais devinés. Un nom inventé compile et rend `null` en silence.
#[derive(Debug, Serialize)]
pub struct Episode {
    /// Identifiant interne, et clé de fusion côté client.
    pub id: i64,
    /// Saison, telle que la chaîne la numérote.
    pub season: Option<i64>,
    /// Numéro d'épisode dans la saison, `None` pour un hors-série.
    pub episode: Option<i64>,
    /// Identifiant de la vidéo chez l'hébergeur.
    pub video_id: Option<String>,
    /// Titre affiché.
    pub title: Option<String>,
    /// Adresse de la vidéo.
    pub url: Option<String>,
    /// Titre japonais.
    pub title_jp: Option<String>,
    /// Romanisation du titre japonais.
    pub romaji: Option<String>,
    /// Vignette.
    pub thumbnail: Option<String>,
    /// Date de publication déclarée par l'hébergeur.
    pub publish_date: Option<String>,
    /// Langue de la piste.
    pub language: Option<String>,
    /// Durée en secondes.
    pub duration: Option<i64>,
    /// Date de moisson (epoch ms) — c'est elle que `?since=` compare.
    pub created_at: Option<i64>,
}

/// Corps de la réponse.
#[derive(Debug, Serialize)]
pub struct PageEpisodes {
    /// Les épisodes retenus.
    pub elements: Vec<Episode>,
    /// Nombre d'épisodes rendus.
    pub total: usize,
    /// Date de moisson la plus récente parmi eux — le `since` du prochain appel.
    pub dernier_moissonne: Option<i64>,
}

/// `GET /api/v1/episodes`.
///
/// Rend `Indisponible` quand la base des épisodes n'est pas là : ce serveur ne moissonne alors
/// pas la série, et le dire vaut mieux que rendre un catalogue vide qu'un client prendrait pour
/// un catalogue à jour.
pub async fn episodes(
    State(etat): State<EtatSite>,
    Query(demande): Query<Demande>,
) -> Result<Json<PageEpisodes>, ErreurSite> {
    let chemin = etat.config.episodes.clone();
    if !chemin.is_file() {
        return Err(ErreurSite::Indisponible(
            "catalogue des épisodes absent : ce serveur ne moissonne pas la série".to_owned(),
        ));
    }
    let limite = demande.limit.unwrap_or(LIMITE_DEFAUT).min(LIMITE_MAX);
    let depuis = demande.since;

    // La lecture est bloquante : elle sort du réacteur pour ne pas retenir un fil d'exécution
    // pendant que SQLite travaille.
    let elements = tokio::task::spawn_blocking(move || lire(&chemin, depuis, limite))
        .await
        .map_err(|e| ErreurSite::Interne(format!("lecture des épisodes interrompue: {e}")))??;

    let dernier_moissonne = elements.iter().filter_map(|e| e.created_at).max();
    Ok(Json(PageEpisodes {
        total: elements.len(),
        dernier_moissonne,
        elements,
    }))
}

/// Lit la base en lecture seule. Aucune colonne n'est inventée : ce sont celles de la table.
fn lire(chemin: &std::path::Path, depuis: i64, limite: u32) -> Result<Vec<Episode>, ErreurSite> {
    let cx = rusqlite::Connection::open_with_flags(chemin, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| ErreurSite::Interne(format!("ouverture du catalogue: {e}")))?;
    let mut requete = cx
        .prepare(
            "SELECT id, season, episode, videoId, title, url, titleJp, romaji, thumbnail, \
             publishDate, language, duration, createdAt \
             FROM episodes WHERE createdAt > ?1 ORDER BY createdAt ASC LIMIT ?2",
        )
        .map_err(|e| ErreurSite::Interne(format!("requête des épisodes: {e}")))?;
    let lignes = requete
        .query_map(rusqlite::params![depuis, limite], |l| {
            Ok(Episode {
                id: l.get(0)?,
                season: l.get(1)?,
                episode: l.get(2)?,
                video_id: l.get(3)?,
                title: l.get(4)?,
                url: l.get(5)?,
                title_jp: l.get(6)?,
                romaji: l.get(7)?,
                thumbnail: l.get(8)?,
                publish_date: l.get(9)?,
                language: l.get(10)?,
                duration: l.get(11)?,
                created_at: l.get(12)?,
            })
        })
        .map_err(|e| ErreurSite::Interne(format!("lecture des épisodes: {e}")))?;
    lignes
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ErreurSite::Interne(format!("ligne d'épisode illisible: {e}")))
}
