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

/// Ouvre le catalogue en lecture seule, y compris quand son répertoire n'est pas inscriptible.
///
/// ## Le `500` de production du 2026-09-05
///
/// `GET /api/v1/episodes` rendait **500** — `requête des épisodes: unable to open database
/// file` — alors que le fichier existe, appartient à l'utilisateur du service et est lisible.
/// Trois faits, mesurés, expliquent la contradiction :
///
/// 1. `sqlite3 data/anime/episodes.db "pragma journal_mode"` rend **`wal`** ;
/// 2. `systemctl show nie-site` rend `ProtectSystem=strict`, `ReadOnlyPaths=/home/ubuntu/niers`
///    et un `ReadWritePaths` **vide** : le processus ne peut rien écrire sous le dépôt ;
/// 3. une base WAL, **même ouverte en lecture seule**, exige de SQLite qu'il crée le fichier
///    de mémoire partagée `-shm` à côté d'elle. Reproduit hors service, avec `chmod 555` sur le
///    répertoire : `attempt to write a readonly database (8)`.
///
/// Autrement dit, ce n'était ni un droit de fichier ni un chemin faux : c'était la conjonction
/// du mode WAL et d'un durcissement systemd. Le paramètre d'URI `immutable=1` dit à SQLite que
/// le fichier ne changera pas sous ses pieds, ce qui lui fait sauter le WAL et le `-shm` — la
/// même reproduction rend alors les 1 141 lignes.
///
/// Il n'est **pas** posé d'emblée, parce qu'il est un mensonge : le cron du VPS réécrit cette
/// base chaque nuit. On tente donc l'ouverture honnête, et on ne se rabat sur `immutable=1` que
/// lorsqu'elle échoue — c'est-à-dire exactement là où l'ouverture honnête n'est de toute façon
/// pas possible, et où le choix n'est pas entre deux lectures mais entre une lecture et un 500.
///
/// # Errors
///
/// `Interne` quand les deux ouvertures échouent : le fichier n'est alors pas une base SQLite.
pub fn ouvrir(chemin: &std::path::Path) -> Result<rusqlite::Connection, ErreurSite> {
    // `open_with_flags` n'ouvre RIEN : SQLite est paresseux et ne touche au fichier qu'à la
    // première requête — c'est exactement pourquoi le défaut se voyait au `prepare` et pas à
    // l'ouverture. `PRAGMA schema_version` lit l'en-tête, donc force la vraie ouverture, et ne
    // coûte qu'une page (à la différence de `quick_check`, qui relirait les 2 Mio).
    let lisible = |cx: &rusqlite::Connection| -> bool {
        cx.query_row("PRAGMA schema_version", [], |l| l.get::<_, i64>(0)).is_ok()
    };
    if let Ok(cx) = rusqlite::Connection::open_with_flags(chemin, OpenFlags::SQLITE_OPEN_READ_ONLY)
        && lisible(&cx)
    {
        return Ok(cx);
    }
    tracing::debug!(
        chemin = %chemin.display(),
        "catalogue illisible en lecture seule ordinaire (WAL + repertoire non inscriptible ?), \
         seconde tentative en immutable=1"
    );
    let cx = rusqlite::Connection::open_with_flags(
        uri_immuable(chemin),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| ErreurSite::Interne(format!("ouverture du catalogue: {e}")))?;
    if !lisible(&cx) {
        return Err(ErreurSite::Interne(
            "catalogue des episodes illisible (fichier absent ou corrompu)".to_owned(),
        ));
    }
    Ok(cx)
}

/// Forme URI d'un chemin, pour l'ouverture `immutable=1`.
///
/// `?` et `#` sont percent-encodés : ils délimitent la query et le fragment d'une URI SQLite,
/// et un chemin qui en porterait un se ferait tronquer en silence — l'ouverture réussirait sur
/// un autre fichier, ou échouerait sans que le message dise pourquoi.
#[must_use]
pub fn uri_immuable(chemin: &std::path::Path) -> String {
    let brut = chemin.display().to_string();
    let mut sortie = String::with_capacity(brut.len() + 24);
    sortie.push_str("file:");
    for c in brut.chars() {
        match c {
            '?' => sortie.push_str("%3f"),
            '#' => sortie.push_str("%23"),
            autre => sortie.push(autre),
        }
    }
    sortie.push_str("?mode=ro&immutable=1");
    sortie
}

/// Lit la base en lecture seule. Aucune colonne n'est inventée : ce sont celles de la table.
fn lire(chemin: &std::path::Path, depuis: i64, limite: u32) -> Result<Vec<Episode>, ErreurSite> {
    let cx = ouvrir(chemin)?;
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
