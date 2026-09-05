//! `/api/v1/*` — lecture seule, DTO `serde`, pagination obligatoire.
//!
//! Deux natures de routes, et il ne faut pas les confondre :
//!
//! - les **filtres enregistrés** (`/textures`, `/modeles`, `/sons`, `/videos`) portent sur les
//!   espaces `/f` et `/b`. Ils ne désignent jamais un fichier et ne créent aucun identifiant :
//!   chaque élément rendu porte son chemin VFS verbatim, qui est aussi son URL sous `/f/`
//!   (amendement A3) ;
//! - les **catalogues** (`/chara`) viennent du miroir SQLite, ouvert en
//!   `SQLITE_OPEN_READ_ONLY`, avec des colonnes nommées une par une — jamais `SELECT *`, dont
//!   le résultat change silencieusement quand une migration ajoute une colonne.
//!
//! Aucune route ne rend une collection entière : 250 800 fichiers et 53 126 textures ne
//! passent pas dans une réponse HTTP, et `per_page` est plafonné à
//! [`crate::config::PER_PAGE_MAX`].

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Serialize;

use crate::erreur::ErreurSite;
use crate::etat::EtatSite;
use crate::index_vfs::{Fichier, VUES, Vue};
use crate::routes::{DemandePage, Page};

/// Table du miroir dont sont tirés les personnages. Constante de la crate : jamais un nom de
/// table venu du client.
pub const TABLE_CHARA: &str = "inagle_characters";

/// Colonnes lues sur [`TABLE_CHARA`], nommées une par une.
pub const COLONNES_CHARA: [&str; 12] = [
    "internal_code",
    "chara_id",
    "base_slug",
    "name_fr",
    "name_en",
    "name_ja",
    "element",
    "position",
    "rarity",
    "series",
    "model_id",
    "zukan_order",
];

/// Corps de `/api/v1/health`.
#[derive(Debug, Serialize)]
pub struct SanteApi {
    /// Toujours `nie-site`.
    pub service: &'static str,
    /// Version de l'API exposée par ce préfixe.
    pub api: &'static str,
    /// Version de la crate.
    pub version: &'static str,
    /// Capacités mesurées.
    pub capacites: crate::etat::Capacites,
    /// Nombre de chemins retenus par chaque filtre enregistré, dans l'ordre de [`VUES`].
    pub vues: Vec<VueResume>,
}

/// Un filtre enregistré et ce qu'il retient.
#[derive(Debug, Serialize)]
pub struct VueResume {
    /// Segment d'URL (`textures`, `modeles`, `sons`, `videos`).
    pub nom: &'static str,
    /// Extensions retenues.
    pub extensions: &'static [&'static str],
    /// Nombre de chemins retenus, ou `None` tant que le VFS n'est pas prêt.
    pub total: Option<usize>,
}

/// `GET /api/v1/health`.
pub async fn health(State(etat): State<EtatSite>) -> Json<SanteApi> {
    let index = etat.index().ok();
    let vues = VUES
        .into_iter()
        .map(|v| VueResume {
            nom: v.segment(),
            extensions: v.extensions(),
            total: index.as_ref().map(|i| i.compte_vue(v)),
        })
        .collect();
    Json(SanteApi {
        service: crate::SERVICE,
        api: "v1",
        version: crate::VERSION,
        capacites: etat.capacites(),
        vues,
    })
}

/// `GET /api/v1/{vue}` — une page d'un filtre enregistré.
///
/// # Errors
///
/// `Introuvable` si le segment ne désigne aucun filtre, `Indisponible` tant que le VFS n'est
/// pas monté.
pub async fn vue(
    State(etat): State<EtatSite>,
    Path(nom): Path<String>,
    Query(demande): Query<DemandePage>,
) -> Result<Json<Page<Fichier>>, ErreurSite> {
    let vue = Vue::depuis_segment(&nom).ok_or_else(|| {
        ErreurSite::Introuvable(format!(
            "filtre inconnu: {nom} (connus: {})",
            VUES.map(Vue::segment).join(", ")
        ))
    })?;
    let index = etat.index()?;
    let p = demande.bornee();
    let elements = index.page_vue(vue, p.offset(), p.per_page as usize);
    Ok(Json(Page::nouvelle(elements, p, index.compte_vue(vue))))
}

/// Un personnage, tel que le miroir le décrit. Aucune colonne n'est inventée : ce sont les
/// noms réels d'`inagle_characters`.
#[derive(Debug, Clone, Serialize)]
pub struct Chara {
    /// Code interne du jeu — c'est l'identifiant stable, et le seul adressable.
    pub internal_code: Option<String>,
    /// Identifiant de personnage tel que le jeu le porte.
    pub chara_id: Option<String>,
    /// Slug de base, **non unique** : 6 168 lignes pour 5 199 valeurs distinctes, dont
    /// `unknown` 65 fois. Affiché et cherchable, jamais adressé.
    pub base_slug: Option<String>,
    /// Nom français.
    pub name_fr: Option<String>,
    /// Nom anglais.
    pub name_en: Option<String>,
    /// Nom japonais.
    pub name_ja: Option<String>,
    /// Élément.
    pub element: Option<String>,
    /// Poste.
    pub position: Option<String>,
    /// Rareté.
    pub rarity: Option<String>,
    /// Série d'origine.
    pub series: Option<String>,
    /// Modèle associé, quand il est connu.
    pub model_id: Option<String>,
    /// Rang au zukan.
    pub zukan_order: Option<i64>,
}

/// `GET /api/v1/chara` — une page du catalogue de personnages.
///
/// # Errors
///
/// `Indisponible` quand le miroir est absent, `Interne` sur défaut de lecture.
pub async fn chara(
    State(etat): State<EtatSite>,
    Query(demande): Query<DemandePage>,
) -> Result<Json<Page<Chara>>, ErreurSite> {
    let p = demande.bornee();
    let gisement = std::sync::Arc::clone(&etat.gisement);
    let page = tokio::task::spawn_blocking(move || {
        gisement.lire(|c| {
            let total: i64 =
                c.query_row(&format!("SELECT count(*) FROM \"{TABLE_CHARA}\""), [], |r| r.get(0))?;
            let sql = format!(
                "SELECT {} FROM \"{TABLE_CHARA}\" ORDER BY \
                 CASE WHEN zukan_order IS NULL THEN 1 ELSE 0 END, zukan_order, internal_code \
                 LIMIT ?1 OFFSET ?2",
                COLONNES_CHARA.map(|c| format!("\"{c}\"")).join(", ")
            );
            let mut stmt = c.prepare(&sql)?;
            let lignes = stmt
                .query_map(
                    rusqlite::params![i64::from(p.per_page), p.offset() as i64],
                    |r| {
                        Ok(Chara {
                            internal_code: r.get(0)?,
                            chara_id: r.get(1)?,
                            base_slug: r.get(2)?,
                            name_fr: r.get(3)?,
                            name_en: r.get(4)?,
                            name_ja: r.get(5)?,
                            element: r.get(6)?,
                            position: r.get(7)?,
                            rarity: r.get(8)?,
                            series: r.get(9)?,
                            model_id: r.get(10)?,
                            zukan_order: r.get(11)?,
                        })
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Page::nouvelle(lignes, p, usize::try_from(total).unwrap_or(0)))
        })
    })
    .await??;
    Ok(Json(page))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colonnes_nommees_une_par_une() {
        assert_eq!(COLONNES_CHARA.len(), 12);
        assert!(!COLONNES_CHARA.contains(&"*"), "jamais SELECT *");
        assert_eq!(TABLE_CHARA, "inagle_characters");
    }
}
