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

use crate::error::ErreurSite;
use crate::routes::{DemandePage, Page};
use crate::state::EtatSite;
use crate::vfs_index::{
    Compte, DemandeFiltre, Fichier, FiltresAppliques, SEGMENT_TOUT, VUES, Vue,
};

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
    pub capacites: crate::state::Capacites,
    /// Nombre de chemins retenus par chaque filtre enregistré, dans l'ordre de [`VUES`].
    pub vues: Vec<VueResume>,
    /// Histogramme des extensions du VFS, par nombre décroissant.
    ///
    /// C'est la facette qui rend atteignables les 112 062 fichiers hors des quatre vues : un
    /// client ne peut pas deviner que `.p3lip` existe, et un filtre qui annonce « 0 résultat »
    /// après le clic est un filtre raté.
    pub extensions: Vec<Compte>,
    /// Histogramme des CPK d'origine — **seulement** sur `?cpk=1`, parce qu'un montage à packs
    /// en porte 936 et qu'une sonde de santé n'a pas à traîner 47 Ko à chaque appel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpks: Option<Vec<Compte>>,
}

/// Query de `/api/v1/health` : de quoi demander le détail sans le payer par défaut.
#[derive(Debug, Default, Clone, serde::Deserialize)]
pub struct DemandeSante {
    /// `1`, `true` ou `oui` : joindre l'histogramme complet des CPK.
    pub cpk: Option<String>,
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

/// `GET /api/v1/health` — capacités mesurées et facettes du VFS.
///
/// `?cpk=1` y joint l'histogramme des packs d'origine.
pub async fn health(
    State(etat): State<EtatSite>,
    Query(demande): Query<DemandeSante>,
) -> Json<SanteApi> {
    let index = etat.index().ok();
    let vues = VUES
        .into_iter()
        .map(|v| VueResume {
            nom: v.segment(),
            extensions: v.extensions(),
            total: index.as_ref().map(|i| i.compte_vue(v)),
        })
        .collect();
    let extensions = index.as_ref().map(|i| i.extensions()).unwrap_or_default();
    let veut_cpk = matches!(
        demande.cpk.as_deref().map(str::trim),
        Some("1" | "true" | "oui")
    );
    let cpks = if veut_cpk {
        Some(index.as_ref().map(|i| i.cpks()).unwrap_or_default())
    } else {
        None
    };
    Json(SanteApi {
        service: crate::SERVICE,
        api: "v1",
        version: crate::VERSION,
        capacites: etat.capacites(),
        vues,
        extensions,
        cpks,
    })
}

/// Une page de fichiers, augmentée de ce que le serveur a réellement appliqué.
///
/// `Page<T>` est aplatie : le contrat de pagination ne change pas, il gagne un voisin. Sans ce
/// `filtres`, un client ne peut pas distinguer « aucun résultat » de « filtre ignoré ».
#[derive(Debug, Serialize)]
pub struct PageFiltree {
    /// La page elle-même (`elements`, `page`, `per_page`, `total`, `pages`).
    #[serde(flatten)]
    pub page: Page<Fichier>,
    /// Ce qui a été appliqué.
    pub filtres: FiltresAppliques,
}

/// `GET /api/v1/{vue}` — une page d'un filtre enregistré.
///
/// Le segment [`SEGMENT_TOUT`] (`/api/v1/tout`) désigne l'espace VFS entier : c'est le seul
/// moyen d'atteindre les 112 062 fichiers que les quatre vues ne retiennent pas, en le
/// combinant à `?ext=`, `?q=` ou `?cpk=`. Ce n'est pas une route de plus, c'est une valeur du
/// segment déjà routé.
///
/// Query acceptée : `page`, `per_page`, `q`, `ext`, `cpk`, `taille_min`, `taille_max`, `tri`
/// (`nom`|`taille`), `ordre` (`asc`|`desc`). Une valeur inconnue est bornée ou ignorée, jamais
/// refusée — et la réponse dit ce qui a compté.
///
/// # Errors
///
/// `Introuvable` si le segment ne désigne aucun filtre, `Indisponible` tant que le VFS n'est
/// pas monté.
pub async fn vue(
    State(etat): State<EtatSite>,
    Path(nom): Path<String>,
    Query(demande): Query<DemandePage>,
    Query(filtre): Query<DemandeFiltre>,
) -> Result<Json<PageFiltree>, ErreurSite> {
    let vue = if nom == SEGMENT_TOUT {
        None
    } else {
        Some(Vue::depuis_segment(&nom).ok_or_else(|| {
            ErreurSite::Introuvable(format!(
                "filtre inconnu: {nom} (connus: {}, {SEGMENT_TOUT})",
                VUES.map(Vue::segment).join(", ")
            ))
        })?)
    };
    let index = etat.index()?;
    let p = demande.bornee();
    let requete = index
        .resoudre(demande.q.as_deref(), &filtre)
        .paginer(p.offset(), p.per_page as usize);
    let filtres = requete.applique.clone();
    let (elements, total) = index.page_filtree(vue, &requete);
    Ok(Json(PageFiltree {
        page: Page::nouvelle(elements, p, total),
        filtres,
    }))
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

/// Facettes du catalogue de personnages, et le tri demandé.
///
/// Quatre colonnes déjà lues (`element`, `position`, `rarity`, `series`) et jamais exposées :
/// une liste de 6 166 lignes ne devient navigable que par elles.
#[derive(Debug, Default, Clone, serde::Deserialize)]
pub struct DemandeChara {
    /// Élément exact (cardinalité mesurée : 6).
    pub element: Option<String>,
    /// Poste exact (5).
    pub position: Option<String>,
    /// Rareté exacte (4).
    pub rarity: Option<String>,
    /// Série exacte (9).
    pub series: Option<String>,
    /// Colonne de tri, prise dans [`TRI_CHARA`]. Inconnue : le tri par défaut.
    pub tri: Option<String>,
    /// Sens : `asc` (défaut) ou `desc`.
    pub ordre: Option<String>,
}

/// Colonnes de tri autorisées sur `/api/v1/chara`.
///
/// **Liste blanche obligatoire** : un `ORDER BY` venu du client est une injection, et la crate
/// a déjà cette doctrine pour les colonnes lues. Le nom public est à gauche, la colonne réelle
/// à droite — le client ne nomme jamais la base.
pub const TRI_CHARA: [(&str, &str); 6] = [
    ("zukan", "zukan_order"),
    ("code", "internal_code"),
    ("nom_fr", "name_fr"),
    ("nom_en", "name_en"),
    ("nom_ja", "name_ja"),
    ("rarete", "rarity"),
];

/// Colonnes sur lesquelles une facette chiffrée est publiée.
pub const FACETTES_CHARA: [&str; 4] = ["element", "position", "rarity", "series"];

/// Ce qui a été appliqué à une page du catalogue de personnages.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FiltresChara {
    /// Motif appliqué aux noms et aux codes.
    pub q: Option<String>,
    /// Élément appliqué.
    pub element: Option<String>,
    /// Poste appliqué.
    pub position: Option<String>,
    /// Rareté appliquée.
    pub rarity: Option<String>,
    /// Série appliquée.
    pub series: Option<String>,
    /// Nom public de la colonne de tri appliquée.
    pub tri: String,
    /// Sens appliqué.
    pub ordre: &'static str,
}

/// Une page du catalogue de personnages, ses filtres et ses facettes chiffrées.
#[derive(Debug, Serialize)]
pub struct PageChara {
    /// La page elle-même.
    #[serde(flatten)]
    pub page: Page<Chara>,
    /// Ce qui a été appliqué.
    pub filtres: FiltresChara,
    /// Comptes par valeur, **sous le filtre courant** : un choix qui rendrait zéro résultat
    /// n'est pas proposé. Les clés sont celles de [`FACETTES_CHARA`].
    pub facettes: std::collections::BTreeMap<String, Vec<Compte>>,
}

/// Traduit la demande en clauses SQL **paramétrées**. Aucune valeur du client n'entre dans le
/// texte de la requête : seuls des `?` y entrent, et le nom de colonne vient de la liste
/// blanche.
fn clauses_chara(
    q: Option<&str>,
    d: &DemandeChara,
) -> (String, Vec<String>, FiltresChara, &'static str) {
    let mut ou = Vec::new();
    let mut params: Vec<String> = Vec::new();
    let mut appl = FiltresChara::default();

    if let Some(m) = q.map(str::trim).filter(|m| !m.is_empty()) {
        // `%` et `_` sont échappés : un `%` tapé par l'utilisateur ne doit pas agir comme un
        // joker SQL (défaut relevé ailleurs dans le dépôt).
        let motif = format!(
            "%{}%",
            m.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
        );
        ou.push(
            "(name_fr LIKE ?  ESCAPE '\\' OR name_en LIKE ? ESCAPE '\\' \
             OR name_ja LIKE ? ESCAPE '\\' OR base_slug LIKE ? ESCAPE '\\' \
             OR internal_code LIKE ? ESCAPE '\\')"
                .to_owned(),
        );
        for _ in 0..5 {
            params.push(motif.clone());
        }
        appl.q = Some(m.to_owned());
    }

    for (colonne, valeur) in [
        ("element", &d.element),
        ("position", &d.position),
        ("rarity", &d.rarity),
        ("series", &d.series),
    ] {
        if let Some(v) = valeur.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            ou.push(format!("\"{colonne}\" = ?"));
            params.push(v.to_owned());
            match colonne {
                "element" => appl.element = Some(v.to_owned()),
                "position" => appl.position = Some(v.to_owned()),
                "rarity" => appl.rarity = Some(v.to_owned()),
                _ => appl.series = Some(v.to_owned()),
            }
        }
    }

    let ou = if ou.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", ou.join(" AND "))
    };

    let ordre = match d.ordre.as_deref().map(str::trim) {
        Some("desc" | "decroissant" | "descendant") => "DESC",
        _ => "ASC",
    };
    appl.ordre = if ordre == "DESC" { "desc" } else { "asc" };
    (ou, params, appl, ordre)
}

/// `GET /api/v1/chara` — une page du catalogue de personnages.
///
/// Query : `page`, `per_page`, `q`, `element`, `position`, `rarity`, `series`, `tri`
/// (cf. [`TRI_CHARA`]), `ordre`.
///
/// # Errors
///
/// `Indisponible` quand le miroir est absent, `Interne` sur défaut de lecture.
pub async fn chara(
    State(etat): State<EtatSite>,
    Query(demande): Query<DemandePage>,
    Query(facettes): Query<DemandeChara>,
) -> Result<Json<PageChara>, ErreurSite> {
    let p = demande.bornee();
    let (ou, params, mut appl, sens) = clauses_chara(demande.q.as_deref(), &facettes);

    // Le tri par défaut est celui du zukan, les non classés en dernier — c'est l'ordre du jeu.
    let tri_defaut =
        "CASE WHEN zukan_order IS NULL THEN 1 ELSE 0 END, zukan_order, internal_code".to_owned();
    let demande_tri = facettes.tri.as_deref().map(str::trim).unwrap_or_default();
    let choisi = TRI_CHARA.iter().find(|(public, _)| *public == demande_tri);
    let (nom_tri, clause_tri) = match choisi {
        Some((public, colonne)) => (
            (*public).to_owned(),
            format!("CASE WHEN \"{colonne}\" IS NULL THEN 1 ELSE 0 END, \"{colonne}\" {sens}, internal_code"),
        ),
        None => ("zukan".to_owned(), tri_defaut),
    };
    appl.tri = nom_tri;

    let gisement = std::sync::Arc::clone(&etat.gisement);
    let page = tokio::task::spawn_blocking(move || {
        gisement.lire(|c| {
            let lies: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

            let total: i64 = c.query_row(
                &format!("SELECT count(*) FROM \"{TABLE_CHARA}\"{ou}"),
                lies.as_slice(),
                |r| r.get(0),
            )?;

            let mut facettes = std::collections::BTreeMap::new();
            for colonne in FACETTES_CHARA {
                let sql = format!(
                    "SELECT \"{colonne}\", count(*) FROM \"{TABLE_CHARA}\"{ou} \
                     GROUP BY \"{colonne}\" ORDER BY count(*) DESC, \"{colonne}\""
                );
                let mut stmt = c.prepare(&sql)?;
                let comptes = stmt
                    .query_map(lies.as_slice(), |r| {
                        let valeur: Option<String> = r.get(0)?;
                        let total: i64 = r.get(1)?;
                        Ok(Compte {
                            valeur: valeur.unwrap_or_default(),
                            total: usize::try_from(total).unwrap_or(0),
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .filter(|c| !c.valeur.is_empty())
                    .collect::<Vec<_>>();
                facettes.insert(colonne.to_owned(), comptes);
            }

            let sql = format!(
                "SELECT {} FROM \"{TABLE_CHARA}\"{ou} ORDER BY {clause_tri} LIMIT ? OFFSET ?",
                COLONNES_CHARA.map(|c| format!("\"{c}\"")).join(", ")
            );
            let mut avec_page: Vec<&dyn rusqlite::ToSql> = lies.clone();
            let limite = i64::from(p.per_page);
            let saut = p.offset() as i64;
            avec_page.push(&limite);
            avec_page.push(&saut);
            let mut stmt = c.prepare(&sql)?;
            let lignes = stmt
                .query_map(avec_page.as_slice(), |r| {
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
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PageChara {
                page: Page::nouvelle(lignes, p, usize::try_from(total).unwrap_or(0)),
                filtres: appl.clone(),
                facettes,
            })
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

    #[test]
    fn clauses_chara_sont_parametrees() {
        let d = DemandeChara {
            element: Some("feu".to_owned()),
            rarity: Some("SSR".to_owned()),
            ordre: Some("desc".to_owned()),
            ..DemandeChara::default()
        };
        let (ou, params, appl, sens) = clauses_chara(Some("mark"), &d);
        assert_eq!(sens, "DESC");
        assert_eq!(appl.ordre, "desc");
        assert_eq!(appl.element.as_deref(), Some("feu"));
        assert_eq!(params.len(), 7, "5 pour le motif, 2 pour les facettes");
        assert!(
            !ou.contains("feu") && !ou.contains("mark"),
            "aucune valeur du client dans le texte SQL: {ou}"
        );
        assert_eq!(ou.matches('?').count(), 7);
    }

    #[test]
    fn motif_chara_echappe_les_jokers() {
        let (_, params, _, _) = clauses_chara(Some("100%_a"), &DemandeChara::default());
        assert_eq!(params[0], "%100\\%\\_a%", "% et _ ne sont pas des jokers");
    }

    #[test]
    fn sans_filtre_aucune_clause() {
        let (ou, params, appl, sens) = clauses_chara(None, &DemandeChara::default());
        assert!(ou.is_empty());
        assert!(params.is_empty());
        assert_eq!(sens, "ASC");
        assert!(appl.q.is_none());
    }

    #[test]
    fn tri_chara_est_une_liste_blanche() {
        // Un nom de colonne venu du client ne doit jamais atteindre l'`ORDER BY`.
        assert!(TRI_CHARA.iter().all(|(_, c)| COLONNES_CHARA.contains(c)));
        assert!(
            !TRI_CHARA
                .iter()
                .any(|(public, _)| *public == "internal_code"),
            "le nom public n'est pas le nom de colonne"
        );
        assert!(FACETTES_CHARA.iter().all(|c| COLONNES_CHARA.contains(c)));
    }
}
