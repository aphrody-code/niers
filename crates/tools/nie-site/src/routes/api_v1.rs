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
use crate::vfs_index::{Compte, DemandeFiltre, Fichier, FiltresAppliques, SEGMENT_TOUT, VUES, Vue};

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
    /// Plusieurs éléments à la fois — `?element__in=Feu,Vent`.
    #[serde(rename = "element__in")]
    pub element_in: Option<String>,
    /// Plusieurs postes à la fois.
    #[serde(rename = "position__in")]
    pub position_in: Option<String>,
    /// Plusieurs raretés à la fois.
    #[serde(rename = "rarity__in")]
    pub rarity_in: Option<String>,
    /// Plusieurs séries à la fois.
    #[serde(rename = "series__in")]
    pub series_in: Option<String>,
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
    /// Choix multiples retenus, `colonne__in` → les valeurs acceptées.
    pub listes: std::collections::BTreeMap<String, Vec<String>>,
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

/// Une condition, et **la colonne dont elle vient**.
///
/// La colonne est retenue pour une seule raison : une facette se compte sans le filtre de sa
/// PROPRE colonne. Sans ce découpage, `?element=Feu` faisait rendre à la facette `element` une
/// unique valeur — mesuré en production le 2026-09-06, `Feu:1528` et les cinq autres éléments
/// disparus — donc l'interface ne pouvait plus proposer d'en ajouter un second. La liste des
/// choix se refermait sur le premier clic.
struct ClauseChara {
    /// La colonne visée, ou `None` pour la recherche libre — qui ne se retire jamais : elle ne
    /// porte sur aucune colonne facetée, et l'exclure élargirait les comptes sans raison.
    colonne: Option<&'static str>,
    /// Le SQL, avec ses `?`.
    sql: String,
    /// Les valeurs à lier, dans l'ordre des `?`.
    params: Vec<String>,
}

/// Assemble le `WHERE`, en retirant les conditions d'une colonne.
///
/// `sauf = None` rend la clause complète — celle de la page. `sauf = Some(colonne)` rend celle
/// d'une facette.
fn ou_chara(clauses: &[ClauseChara], sauf: Option<&str>) -> (String, Vec<String>) {
    let retenues: Vec<&ClauseChara> = clauses
        .iter()
        .filter(|c| c.colonne.is_none() || c.colonne != sauf)
        .collect();
    let sql = if retenues.is_empty() {
        String::new()
    } else {
        format!(
            " WHERE {}",
            retenues
                .iter()
                .map(|c| c.sql.clone())
                .collect::<Vec<_>>()
                .join(" AND ")
        )
    };
    let params = retenues
        .iter()
        .flat_map(|c| c.params.iter().cloned())
        .collect();
    (sql, params)
}

/// Découpe un `?colonne__in=a,b` en valeurs, sans doublon ni vide.
fn valeurs_in(brut: Option<&String>) -> Vec<String> {
    let mut sorties: Vec<String> = Vec::new();
    for v in brut
        .map(String::as_str)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if !sorties.iter().any(|d| d == v) {
            sorties.push(v.to_owned());
        }
    }
    sorties
}

/// Traduit la demande en clauses SQL **paramétrées**. Aucune valeur du client n'entre dans le
/// texte de la requête : seuls des `?` y entrent, et le nom de colonne vient de la liste
/// blanche.
fn clauses_chara(
    q: Option<&str>,
    d: &DemandeChara,
) -> (Vec<ClauseChara>, FiltresChara, &'static str) {
    let mut ou: Vec<ClauseChara> = Vec::new();
    let mut appl = FiltresChara::default();

    if let Some(m) = q.map(str::trim).filter(|m| !m.is_empty()) {
        // `%` et `_` sont échappés : un `%` tapé par l'utilisateur ne doit pas agir comme un
        // joker SQL (défaut relevé ailleurs dans le dépôt).
        let motif = format!(
            "%{}%",
            m.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        ou.push(ClauseChara {
            colonne: None,
            sql: "(name_fr LIKE ?  ESCAPE '\\' OR name_en LIKE ? ESCAPE '\\' \
                  OR name_ja LIKE ? ESCAPE '\\' OR base_slug LIKE ? ESCAPE '\\' \
                  OR internal_code LIKE ? ESCAPE '\\')"
                .to_owned(),
            params: vec![motif; 5],
        });
        appl.q = Some(m.to_owned());
    }

    for (colonne, valeur, liste) in [
        ("element", &d.element, &d.element_in),
        ("position", &d.position, &d.position_in),
        ("rarity", &d.rarity, &d.rarity_in),
        ("series", &d.series, &d.series_in),
    ] {
        if let Some(v) = valeur.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            ou.push(ClauseChara {
                colonne: Some(colonne),
                sql: format!("\"{colonne}\" = ?"),
                params: vec![v.to_owned()],
            });
            match colonne {
                "element" => appl.element = Some(v.to_owned()),
                "position" => appl.position = Some(v.to_owned()),
                "rarity" => appl.rarity = Some(v.to_owned()),
                _ => appl.series = Some(v.to_owned()),
            }
        }
        // Le choix multiple s'ajoute a l'egalite plutot que de la remplacer : les deux
        // parametres sont distincts, et un client qui envoie les deux demande bien les deux.
        let valeurs = valeurs_in(liste.as_ref());
        if !valeurs.is_empty() {
            let trous = std::iter::repeat_n("?", valeurs.len())
                .collect::<Vec<_>>()
                .join(", ");
            ou.push(ClauseChara {
                colonne: Some(colonne),
                sql: format!("\"{colonne}\" IN ({trous})"),
                params: valeurs.clone(),
            });
            appl.listes.insert(format!("{colonne}__in"), valeurs);
        }
    }

    let ordre = match d.ordre.as_deref().map(str::trim) {
        Some("desc" | "decroissant" | "descendant") => "DESC",
        _ => "ASC",
    };
    appl.ordre = if ordre == "DESC" { "desc" } else { "asc" };
    (ou, appl, ordre)
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
    let (clauses, mut appl, sens) = clauses_chara(demande.q.as_deref(), &facettes);
    let (ou, params) = ou_chara(&clauses, None);

    // Le tri par défaut est celui du zukan, les non classés en dernier — c'est l'ordre du jeu.
    let tri_defaut =
        "CASE WHEN zukan_order IS NULL THEN 1 ELSE 0 END, zukan_order, internal_code".to_owned();
    let demande_tri = facettes.tri.as_deref().map(str::trim).unwrap_or_default();
    let choisi = TRI_CHARA.iter().find(|(public, _)| *public == demande_tri);
    let (nom_tri, clause_tri) = match choisi {
        Some((public, colonne)) => (
            (*public).to_owned(),
            format!(
                "CASE WHEN \"{colonne}\" IS NULL THEN 1 ELSE 0 END, \"{colonne}\" {sens}, internal_code"
            ),
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
                // Sans le filtre de SA propre colonne : c'est ce qui laisse choisir une seconde
                // valeur. Les autres filtres s'appliquent bien, donc les comptes correspondent
                // toujours a ce que la page montrera.
                let (ou_facette, params_facette) = ou_chara(&clauses, Some(colonne));
                let lies_facette: Vec<&dyn rusqlite::ToSql> = params_facette
                    .iter()
                    .map(|s| s as &dyn rusqlite::ToSql)
                    .collect();
                let sql = format!(
                    "SELECT \"{colonne}\", count(*) FROM \"{TABLE_CHARA}\"{ou_facette} \
                     GROUP BY \"{colonne}\" ORDER BY count(*) DESC, \"{colonne}\""
                );
                let mut stmt = c.prepare(&sql)?;
                let comptes = stmt
                    .query_map(lies_facette.as_slice(), |r| {
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
        let (clauses, appl, sens) = clauses_chara(Some("mark"), &d);
        let (ou, params) = ou_chara(&clauses, None);
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
        let (clauses, _, _) = clauses_chara(Some("100%_a"), &DemandeChara::default());
        let (_, params) = ou_chara(&clauses, None);
        assert_eq!(params[0], "%100\\%\\_a%", "% et _ ne sont pas des jokers");
    }

    #[test]
    fn sans_filtre_aucune_clause() {
        let (clauses, appl, sens) = clauses_chara(None, &DemandeChara::default());
        let (ou, params) = ou_chara(&clauses, None);
        assert!(ou.is_empty());
        assert!(params.is_empty());
        assert_eq!(sens, "ASC");
        assert!(appl.q.is_none());
    }

    #[test]
    fn une_facette_chara_se_compte_sans_le_filtre_de_sa_colonne() {
        // Le defaut mesure en production le 2026-09-06 : `?element=Feu` faisait rendre a la
        // facette `element` la seule valeur `Feu:1528`, les cinq autres elements disparus. Un
        // choix fermait donc la liste des choix, et aucun second element n'etait atteignable.
        //
        // Les trois moities comptent. Sans la premiere, on ne verrait pas que la clause de la
        // page garde bien le filtre ; sans la deuxieme, un `ou_chara` qui retirerait TOUT
        // passerait ; sans la troisieme, un qui ne retirerait RIEN passerait aussi.
        let d = DemandeChara {
            element: Some("Feu".to_owned()),
            position: Some("Milieu".to_owned()),
            ..DemandeChara::default()
        };
        let (clauses, _, _) = clauses_chara(Some("mark"), &d);

        let (page, p_page) = ou_chara(&clauses, None);
        assert_eq!(
            page.matches('?').count(),
            7,
            "5 pour le motif + element + position"
        );
        assert_eq!(p_page.len(), 7);

        let (f_element, p_element) = ou_chara(&clauses, Some("element"));
        assert_eq!(
            f_element.matches('?').count(),
            6,
            "element retire, position gardee"
        );
        assert!(
            f_element.contains("\"position\""),
            "les AUTRES filtres restent : {f_element}"
        );
        assert!(
            !f_element.contains("\"element\""),
            "le sien part : {f_element}"
        );
        assert_eq!(
            p_element,
            ["%mark%"; 5]
                .iter()
                .map(|s| (*s).to_owned())
                .chain(["Milieu".to_owned()])
                .collect::<Vec<_>>()
        );

        // La recherche libre ne se retire jamais : elle ne porte sur aucune colonne facetee.
        let (f_series, _) = ou_chara(&clauses, Some("series"));
        assert_eq!(
            f_series.matches('?').count(),
            7,
            "aucune colonne `series` n'etait filtree"
        );
    }

    #[test]
    fn un_choix_multiple_chara_devient_un_in() {
        // L'affordance que les facettes dessinent : plusieurs valeurs a la fois. Le `IN` est
        // parametre comme le reste, et les doublons ne le gonflent pas.
        let d = DemandeChara {
            element_in: Some("Feu, Vent ,Feu".to_owned()),
            ..DemandeChara::default()
        };
        let (clauses, appl, _) = clauses_chara(None, &d);
        let (ou, params) = ou_chara(&clauses, None);
        assert_eq!(ou, " WHERE \"element\" IN (?, ?)", "deux trous, pas trois");
        assert_eq!(params, ["Feu", "Vent"]);
        assert_eq!(
            appl.listes.get("element__in").map(Vec::as_slice),
            Some(["Feu".to_owned(), "Vent".to_owned()].as_slice()),
            "republie sous le nom que le client a envoye"
        );

        // Et il part avec sa colonne quand on compte cette colonne-la.
        let (facette, _) = ou_chara(&clauses, Some("element"));
        assert!(facette.is_empty(), "rien ne reste : {facette}");

        // Une liste vide ne filtre pas — et surtout ne produit pas un `IN ()`, qui est une
        // erreur de syntaxe SQLite et non un filtre vide.
        let vide = DemandeChara {
            element_in: Some(" , ".to_owned()),
            ..DemandeChara::default()
        };
        let (clauses, appl, _) = clauses_chara(None, &vide);
        assert!(ou_chara(&clauses, None).0.is_empty());
        assert!(appl.listes.is_empty());
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
