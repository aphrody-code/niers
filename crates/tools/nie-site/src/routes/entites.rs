//! `/api/v1/entites` — la lecture **générique** du miroir SQLite (`var/mirror.sqlite`).
//!
//! Le gisement `extrait` porte 219 tables `inagle_*` et 165 249 lignes. Jusqu'ici le site n'en
//! servait qu'une poignée, chacune par une route écrite à la main (`/api/v1/chara`,
//! `/api/v1/3d/perso`, …) : tout le reste — les 153 tables `inagle_cross_*`, les inventaires
//! d'icônes, les tables de drop, les boutiques — était présent sur la machine et inatteignable.
//! Écrire 219 routes n'était pas la réponse ; une route qui **mesure** le schéma en est une.
//!
//! Trois routes, et rien d'autre :
//!
//! | Route | Ce qu'elle rend |
//! |---|---|
//! | `GET /api/v1/entites` | le catalogue : les tables servables, leurs colonnes, leur compte de lignes |
//! | `GET /api/v1/entites/{table}` | une page de lignes, filtrée, triée, bornée |
//! | `GET /api/v1/entites/{table}/{id}` | une ligne, par sa clé |
//!
//! ## Ce qui rend l'exercice sûr
//!
//! Une route générique sur une base, c'est une injection SQL si on la construit naïvement. La
//! règle tenue ici est **structurelle**, pas déclarative :
//!
//! - **aucun nom de table ni de colonne ne vient du client**. Le client fournit une chaîne ;
//!   cette chaîne sert à *retrouver* une entrée dans le catalogue mesuré sur `sqlite_master` et
//!   `PRAGMA table_info`, et c'est le nom **du catalogue** — donc de la base — qui est écrit
//!   dans le SQL. Une table inconnue est un `404`, une colonne inconnue un `400` : jamais une
//!   requête exécutée ;
//! - **toutes les valeurs sont des paramètres liés**. Il n'y a pas une seule valeur du client
//!   dans le texte d'une requête, y compris dans le motif `LIKE` (dont les jokers `%` et `_`
//!   sont d'ailleurs échappés — un `%` tapé par un humain est un pourcent) ;
//! - la connexion est ouverte en `SQLITE_OPEN_READ_ONLY` par [`crate::dataset::Gisement`], qui
//!   porte aussi la parade au lien symbolique rebasculé chaque nuit. Aucune seconde connexion
//!   n'est ouverte ici.
//!
//! ## Un paramètre accepté est un paramètre honoré
//!
//! Le dépôt a déjà payé le contraire (`/b` déclarait `q` et ne l'appliquait pas : un client qui
//! filtre croit filtrer, et la liste entière passe pour un résultat). Donc :
//!
//! - `tri` sur une colonne inconnue est un `400`, jamais un tri silencieusement ignoré ;
//! - `ordre` hors de `asc`/`desc` est un `400` ;
//! - `q` sur une table **sans aucune colonne texte** est un `400` qui le dit, plutôt qu'un
//!   filtre qui ne filtre rien (c'est le cas de `inagle_exp_table`, la seule des 219) ;
//! - un filtre à valeur vide (`?element=`) est un `400` : ni « pas de filtre » ni « égal à la
//!   chaîne vide » ne sont devinables, et deviner serait mentir dans un sens ou dans l'autre ;
//! - la réponse republie ce qui a été appliqué (`filtres`), pour qu'un client puisse le vérifier
//!   sans relire ce fichier.
//!
//! Sans miroir, les trois routes répondent `503` avec la raison : le service démarre toujours.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use rusqlite::Connection;
use rusqlite::types::Value as ValeurSql;
use serde::Serialize;
use serde_json::{Map as MapJson, Value as ValeurJson};

use crate::config::Pagination;
use crate::error::ErreurSite;
use crate::routes::Page;
use crate::state::EtatSite;

/// Préfixe des tables **non servies**.
///
/// `_meta` porte l'horodatage du miroir et la chaîne de connexion d'amont : de la plomberie,
/// exactement ce qu'une façade ne montre pas. L'exclure fait tomber le catalogue de 220 à 219
/// tables — le compte que `_meta` s'annonce lui-même (`tables_count = 219`).
pub const PREFIXE_INTERNE: char = '_';

/// Noms de paramètres réservés par la route : tout le reste est lu comme un filtre d'égalité
/// sur une colonne.
///
/// `par_page` et `per_page` sont acceptés tous les deux — le premier parce que c'est le nom
/// français de la route, le second parce que c'est celui du reste de l'API
/// ([`crate::routes::DemandePage`]) et qu'un client qui l'emploie ne doit pas se retrouver avec
/// un `400` sur une « colonne inconnue `per_page` ».
pub const PARAMS_RESERVES: [&str; 6] = ["page", "par_page", "per_page", "tri", "ordre", "q"];

/// Longueur maximale d'un identifiant SQL accepté depuis le client.
///
/// Ce n'est pas la garde de sécurité — celle-ci est la recherche dans le catalogue — mais elle
/// évite de promener une chaîne d'un mégaoctet jusqu'à la comparaison.
pub const NOM_MAX: usize = 64;

/// Une colonne, telle que `PRAGMA table_info` la décrit.
#[derive(Debug, Clone, Serialize)]
pub struct Colonne {
    /// Nom de la colonne, **verbatim depuis la base**.
    pub nom: String,
    /// Type déclaré (`TEXT`, `INTEGER`, `REAL`). Vide quand la colonne n'en déclare aucun.
    pub type_sql: String,
    /// `true` quand l'affinité de la colonne est textuelle : c'est l'ensemble sur lequel `q`
    /// cherche.
    pub texte: bool,
}

/// Une table servable, avec son schéma mesuré.
#[derive(Debug, Clone, Serialize)]
pub struct TableServie {
    /// Nom de la table.
    pub nom: String,
    /// Colonne qui identifie une ligne, telle que [`cle_primaire`] la choisit.
    pub cle: String,
    /// `true` quand la clé est le `rowid` implicite de SQLite plutôt qu'une vraie colonne.
    pub cle_implicite: bool,
    /// Colonnes, dans l'ordre de la table.
    pub colonnes: Vec<Colonne>,
}

impl TableServie {
    /// Retrouve une colonne par son nom, sans casse.
    #[must_use]
    pub fn colonne(&self, nom: &str) -> Option<&Colonne> {
        self.colonnes
            .iter()
            .find(|c| c.nom.eq_ignore_ascii_case(nom))
    }

    /// Les colonnes sur lesquelles `q` peut chercher.
    #[must_use]
    pub fn colonnes_texte(&self) -> Vec<&Colonne> {
        self.colonnes.iter().filter(|c| c.texte).collect()
    }

    /// L'expression SQL qui rend la clé dans un `SELECT` — `"id"` ou `"rowid"`.
    #[must_use]
    pub fn cle_sql(&self) -> String {
        format!("\"{}\"", self.cle)
    }
}

/// Une table du catalogue, avec son compte de lignes.
#[derive(Debug, Clone, Serialize)]
pub struct TableComptee {
    /// Le schéma mesuré.
    #[serde(flatten)]
    pub table: TableServie,
    /// Nombre de lignes, compté à la demande.
    pub lignes: usize,
}

/// Le catalogue rendu par `GET /api/v1/entites`.
#[derive(Debug, Serialize)]
pub struct CatalogueEntites {
    /// La page de tables.
    #[serde(flatten)]
    pub page: Page<TableComptee>,
    /// Nombre de lignes de **toutes** les tables servies, filtre `q` compris.
    ///
    /// Il n'est pas le total de la page : c'est la seule information que la pagination ne porte
    /// pas déjà, et c'est pour cela qu'elle est ici plutôt qu'un compte de tables répété.
    pub lignes_totales: usize,
    /// La route qui rend les lignes d'une table.
    pub route_lignes: &'static str,
    /// La route qui rend une ligne.
    pub route_ligne: &'static str,
}

/// Le sens de tri demandé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ordre {
    /// Croissant — le défaut.
    Croissant,
    /// Décroissant.
    Decroissant,
}

impl Ordre {
    /// Le mot-clé SQL correspondant. Il est **constant** : aucune chaîne du client n'entre là.
    #[must_use]
    pub fn sql(self) -> &'static str {
        match self {
            Self::Croissant => "ASC",
            Self::Decroissant => "DESC",
        }
    }

    /// Le jeton public, celui que la réponse republie.
    #[must_use]
    pub fn jeton(self) -> &'static str {
        match self {
            Self::Croissant => "asc",
            Self::Decroissant => "desc",
        }
    }
}

/// Ce que la route a réellement appliqué, republié dans la réponse.
#[derive(Debug, Clone, Serialize)]
pub struct FiltresAppliques {
    /// Motif de recherche retenu, `null` si aucun.
    pub q: Option<String>,
    /// Colonne de tri effective.
    pub tri: String,
    /// Sens de tri effectif.
    pub ordre: &'static str,
    /// Filtres d'égalité retenus, colonne → valeur.
    pub egalites: BTreeMap<String, String>,
}

/// Une demande analysée et validée contre le schéma d'une table.
#[derive(Debug, Clone)]
pub struct Demande {
    /// Bornes de pagination.
    pub pagination: Pagination,
    /// Motif de recherche, déjà nettoyé.
    pub q: Option<String>,
    /// Colonne de tri — **un nom du catalogue**, jamais celui envoyé par le client.
    pub tri: String,
    /// Sens de tri.
    pub ordre: Ordre,
    /// Égalités demandées, colonne du catalogue → valeur brute (qui sera liée).
    pub egalites: Vec<(String, String)>,
}

impl Demande {
    /// Ce qui a été appliqué, sous forme sérialisable.
    #[must_use]
    pub fn appliques(&self) -> FiltresAppliques {
        FiltresAppliques {
            q: self.q.clone(),
            tri: self.tri.clone(),
            ordre: self.ordre.jeton(),
            egalites: self.egalites.iter().cloned().collect(),
        }
    }
}

/// Une clause `WHERE` construite, avec ses paramètres liés.
#[derive(Debug, Clone, Default)]
pub struct Clause {
    /// Le texte SQL, préfixé de ` WHERE ` quand il n'est pas vide.
    pub sql: String,
    /// Les valeurs, dans l'ordre des `?`.
    pub params: Vec<ValeurSql>,
}

/// Une page de lignes, avec la table, sa clé et ce qui a été appliqué.
#[derive(Debug, Serialize)]
pub struct PageLignes {
    /// La page elle-même.
    #[serde(flatten)]
    pub page: Page<MapJson<String, ValeurJson>>,
    /// Nom de la table lue.
    pub table: String,
    /// Colonne qui identifie une ligne — celle que `/api/v1/entites/{table}/{id}` attend.
    pub cle: String,
    /// Ce que la route a appliqué.
    pub filtres: FiltresAppliques,
}

/// Une ligne unique.
#[derive(Debug, Serialize)]
pub struct LigneUnique {
    /// Nom de la table.
    pub table: String,
    /// Colonne de clé.
    pub cle: String,
    /// Valeur de clé telle que demandée.
    pub id: String,
    /// La ligne, colonne → valeur.
    pub ligne: MapJson<String, ValeurJson>,
}

// --------------------------------------------------------------------------------------------
// Mesure du schéma
// --------------------------------------------------------------------------------------------

/// Dit si une chaîne peut être un identifiant SQL de ce miroir.
///
/// Garde de forme, posée **avant** la recherche dans le catalogue : elle ne remplace pas cette
/// recherche (c'est elle, la sécurité), elle évite juste de la faire sur une chaîne absurde.
#[must_use]
pub fn nom_sql_valide(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= NOM_MAX
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Dit si un type déclaré porte une affinité textuelle, selon la règle de SQLite (le type
/// contient `CHAR`, `CLOB` ou `TEXT`).
///
/// Un type absent donne une affinité `BLOB`, pas `TEXT` : chercher dedans avec `LIKE` reviendrait
/// à annoncer une recherche qui ne trouve rien.
#[must_use]
pub fn colonne_texte(type_sql: &str) -> bool {
    let t = type_sql.to_ascii_uppercase();
    t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT")
}

/// Choisit la colonne qui identifie une ligne.
///
/// Dans l'ordre : la clé primaire quand elle tient en **une** colonne, sinon une colonne nommée
/// `id`, sinon le `rowid` implicite. Mesuré sur ce miroir le 2026-09-06 : 1 table déclare une
/// clé primaire, 201 portent un `id`, 0 est `WITHOUT ROWID` — les 18 restantes retombent donc
/// sur `rowid`, qui existe toujours.
///
/// Rend `(nom, implicite)`.
#[must_use]
pub fn cle_primaire(colonnes: &[(String, String, i32)]) -> (String, bool) {
    let pk: Vec<&(String, String, i32)> = colonnes.iter().filter(|(_, _, pk)| *pk > 0).collect();
    if pk.len() == 1 {
        return (pk[0].0.clone(), false);
    }
    // Une colonne réellement nommée `rowid` masque le rowid implicite : c'est elle la clé, et
    // `SELECT rowid, *` la dupliquerait.
    for cible in ["rowid", "id"] {
        if let Some((nom, _, _)) = colonnes.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(cible))
        {
            return (nom.clone(), false);
        }
    }
    ("rowid".to_owned(), true)
}

/// Mesure le catalogue des tables servables sur la connexion.
///
/// Le schéma est **relu à chaque requête** : le miroir est un lien symbolique rebasculé chaque
/// nuit, et un catalogue mémorisé au démarrage décrirait tôt ou tard une autre base. Le coût
/// est celui de `sqlite_master` plus un `PRAGMA table_info` par table — aucun `count(*)`.
///
/// # Errors
///
/// Toute erreur SQLite, traduite en `500` sans laisser fuiter le SQL.
pub fn schema(c: &Connection) -> Result<Vec<TableServie>, ErreurSite> {
    let mut stmt = c.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' \
         AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let noms: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut tables = Vec::with_capacity(noms.len());
    for nom in noms {
        if nom.starts_with(PREFIXE_INTERNE) || !nom_sql_valide(&nom) {
            continue;
        }
        // `PRAGMA table_info` ne se paramètre pas ; le nom vient de `sqlite_master`, donc de la
        // base elle-même, et il vient de repasser la garde de forme.
        let mut p = c.prepare(&format!("PRAGMA table_info(\"{nom}\")"))?;
        let brutes: Vec<(String, String, i32)> = p
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    r.get::<_, i32>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if brutes.is_empty() || brutes.iter().any(|(n, _, _)| !nom_sql_valide(n)) {
            continue;
        }
        let (cle, cle_implicite) = cle_primaire(&brutes);
        tables.push(TableServie {
            nom,
            cle,
            cle_implicite,
            colonnes: brutes
                .into_iter()
                .map(|(nom, type_sql, _)| Colonne {
                    texte: colonne_texte(&type_sql),
                    nom,
                    type_sql,
                })
                .collect(),
        });
    }
    Ok(tables)
}

/// Retrouve une table dans le catalogue, ou dit en `404` qu'elle n'y est pas.
///
/// **C'est ici que se joue la sûreté** : la valeur rendue est une référence dans le catalogue
/// mesuré ; le nom du client, lui, ne sert qu'à comparer et n'est jamais réécrit dans du SQL.
///
/// # Errors
///
/// `Introuvable` quand aucune table servie ne porte ce nom.
pub fn trouver<'a>(
    catalogue: &'a [TableServie],
    demande: &str,
) -> Result<&'a TableServie, ErreurSite> {
    if nom_sql_valide(demande)
        && let Some(t) = catalogue.iter().find(|t| t.nom.eq_ignore_ascii_case(demande))
    {
        return Ok(t);
    }
    Err(ErreurSite::Introuvable(format!(
        "aucune table servie ne se nomme `{demande}` ; les {} tables servies sont sur \
         /api/v1/entites",
        catalogue.len()
    )))
}

// --------------------------------------------------------------------------------------------
// Analyse de la demande
// --------------------------------------------------------------------------------------------

/// Analyse la query string contre le schéma d'une table.
///
/// # Errors
///
/// `Demande` (400) pour un `page`/`par_page` non numérique, un `tri` ou un filtre visant une
/// colonne inconnue, un `ordre` autre que `asc`/`desc`, une valeur de filtre vide, ou un `q`
/// demandé sur une table sans colonne texte.
pub fn analyser(
    table: &TableServie,
    brut: &BTreeMap<String, String>,
) -> Result<Demande, ErreurSite> {
    let entier = |cle: &str| -> Result<Option<u32>, ErreurSite> {
        match brut.get(cle) {
            None => Ok(None),
            Some(v) => v.trim().parse::<u32>().map(Some).map_err(|_| {
                ErreurSite::Demande(format!("`{cle}` doit etre un entier, recu `{v}`"))
            }),
        }
    };
    let page = entier("page")?;
    let par_page = match entier("par_page")? {
        Some(n) => Some(n),
        None => entier("per_page")?,
    };
    let pagination = Pagination::borner(page, par_page);

    let q = match brut.get("q").map(|v| v.trim()).filter(|v| !v.is_empty()) {
        None => None,
        Some(m) => {
            if table.colonnes_texte().is_empty() {
                return Err(ErreurSite::Demande(format!(
                    "`{}` n'a aucune colonne texte : `q` n'y chercherait nulle part",
                    table.nom
                )));
            }
            Some(m.to_owned())
        }
    };

    let tri = match brut.get("tri").map(|v| v.trim()).filter(|v| !v.is_empty()) {
        None => table.cle.clone(),
        Some(demande) => {
            let c = table.colonne(demande).ok_or_else(|| {
                ErreurSite::Demande(format!(
                    "`tri={demande}` : `{}` n'a pas cette colonne ; son schema est sur \
                     /api/v1/entites",
                    table.nom
                ))
            })?;
            c.nom.clone()
        }
    };

    let ordre = match brut.get("ordre").map(|v| v.trim()).filter(|v| !v.is_empty()) {
        None => Ordre::Croissant,
        Some(o) if o.eq_ignore_ascii_case("asc") => Ordre::Croissant,
        Some(o) if o.eq_ignore_ascii_case("desc") => Ordre::Decroissant,
        Some(o) => {
            return Err(ErreurSite::Demande(format!(
                "`ordre={o}` : seuls `asc` et `desc` sont acceptes"
            )));
        }
    };

    let mut egalites = Vec::new();
    for (cle, valeur) in brut {
        if PARAMS_RESERVES.contains(&cle.as_str()) {
            continue;
        }
        let colonne = table.colonne(cle).ok_or_else(|| {
            ErreurSite::Demande(format!(
                "`{cle}` n'est ni un parametre de cette route ni une colonne de `{}` ; son \
                 schema est sur /api/v1/entites",
                table.nom
            ))
        })?;
        if valeur.trim().is_empty() {
            return Err(ErreurSite::Demande(format!(
                "`{cle}=` : une valeur de filtre vide n'a pas de sens ici — retirez le \
                 parametre pour ne pas filtrer"
            )));
        }
        egalites.push((colonne.nom.clone(), valeur.clone()));
    }
    egalites.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(Demande {
        pagination,
        q,
        tri,
        ordre,
        egalites,
    })
}

/// Construit la clause `WHERE` d'une demande déjà validée.
///
/// Aucune valeur n'entre dans le texte : seuls des `?` y entrent, et les noms de colonnes
/// viennent de `table`, c'est-à-dire de la base.
#[must_use]
pub fn clause(table: &TableServie, d: &Demande) -> Clause {
    let mut morceaux: Vec<String> = Vec::new();
    let mut params: Vec<ValeurSql> = Vec::new();

    if let Some(m) = d.q.as_deref() {
        // Les jokers sont échappés : un `%` tapé par un humain est un pourcent, pas « tout ».
        let motif = format!(
            "%{}%",
            m.to_lowercase()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let colonnes = table.colonnes_texte();
        let ors: Vec<String> = colonnes
            .iter()
            .map(|c| format!("lower(\"{}\") LIKE ? ESCAPE '\\'", c.nom))
            .collect();
        if !ors.is_empty() {
            morceaux.push(format!("({})", ors.join(" OR ")));
            for _ in 0..ors.len() {
                params.push(ValeurSql::Text(motif.clone()));
            }
        }
    }

    for (colonne, valeur) in &d.egalites {
        morceaux.push(format!("\"{colonne}\" = ?"));
        params.push(ValeurSql::Text(valeur.clone()));
    }

    Clause {
        sql: if morceaux.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", morceaux.join(" AND "))
        },
        params,
    }
}

/// Traduit une ligne SQLite en objet JSON, colonne par colonne.
///
/// Les blobs ne sont pas rendus en base64 : ce miroir n'en contient aucun (mesuré le
/// 2026-09-06 : 1 073 colonnes `TEXT`, 332 `INTEGER`, 9 `REAL`, 0 autre), et inventer un
/// encodage pour un cas qui n'existe pas serait du code que rien ne vérifie. Leur taille est
/// rendue, ce qui suffit à voir qu'il y en a un.
///
/// # Errors
///
/// Toute erreur SQLite de lecture de colonne.
pub fn ligne_en_json(
    ligne: &rusqlite::Row<'_>,
    colonnes: &[String],
) -> Result<MapJson<String, ValeurJson>, rusqlite::Error> {
    let mut objet = MapJson::new();
    for (i, nom) in colonnes.iter().enumerate() {
        let valeur = match ligne.get_ref(i)? {
            rusqlite::types::ValueRef::Null => ValeurJson::Null,
            rusqlite::types::ValueRef::Integer(n) => ValeurJson::from(n),
            rusqlite::types::ValueRef::Real(x) => serde_json::Number::from_f64(x)
                .map_or(ValeurJson::Null, ValeurJson::Number),
            rusqlite::types::ValueRef::Text(t) => {
                ValeurJson::String(String::from_utf8_lossy(t).into_owned())
            }
            rusqlite::types::ValueRef::Blob(b) => {
                let mut o = MapJson::new();
                o.insert("blob_octets".to_owned(), ValeurJson::from(b.len()));
                ValeurJson::Object(o)
            }
        };
        objet.insert(nom.clone(), valeur);
    }
    Ok(objet)
}

// --------------------------------------------------------------------------------------------
// Lectures (bloquantes — appelées depuis `spawn_blocking`)
// --------------------------------------------------------------------------------------------

/// Le catalogue complet, chaque table avec son compte de lignes.
///
/// # Errors
///
/// Toute erreur SQLite.
pub fn catalogue_compte(c: &Connection) -> Result<Vec<TableComptee>, ErreurSite> {
    let mut sortie = Vec::new();
    for table in schema(c)? {
        let n: i64 = c.query_row(&format!("SELECT count(*) FROM \"{}\"", table.nom), [], |r| {
            r.get(0)
        })?;
        sortie.push(TableComptee {
            table,
            lignes: usize::try_from(n).unwrap_or(0),
        });
    }
    Ok(sortie)
}

/// Lit une page de lignes d'une table déjà validée.
///
/// # Errors
///
/// Toute erreur SQLite.
pub fn page_lignes(
    c: &Connection,
    table: &TableServie,
    d: &Demande,
) -> Result<Page<MapJson<String, ValeurJson>>, ErreurSite> {
    let cl = clause(table, d);
    let total: i64 = c.query_row(
        &format!("SELECT count(*) FROM \"{}\"{}", table.nom, cl.sql),
        rusqlite::params_from_iter(cl.params.iter()),
        |r| r.get(0),
    )?;

    // La clé est projetée explicitement quand elle est implicite : sans elle, la page rend des
    // lignes que `/api/v1/entites/{table}/{id}` ne saurait pas réadresser.
    let projection = if table.cle_implicite {
        "\"rowid\" AS \"rowid\", *"
    } else {
        "*"
    };
    let sql = format!(
        "SELECT {projection} FROM \"{}\"{} ORDER BY \"{}\" {}, {} LIMIT ? OFFSET ?",
        table.nom,
        cl.sql,
        d.tri,
        d.ordre.sql(),
        table.cle_sql(),
    );
    let mut stmt = c.prepare(&sql)?;
    let colonnes: Vec<String> = stmt.column_names().into_iter().map(str::to_owned).collect();
    let mut params = cl.params.clone();
    params.push(ValeurSql::Integer(i64::from(d.pagination.per_page)));
    params.push(ValeurSql::Integer(
        i64::try_from(d.pagination.offset()).unwrap_or(i64::MAX),
    ));
    let elements = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |r| {
            ligne_en_json(r, &colonnes)
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Page::nouvelle(
        elements,
        d.pagination,
        usize::try_from(total).unwrap_or(0),
    ))
}

/// Lit une ligne par sa clé.
///
/// # Errors
///
/// `Introuvable` quand aucune ligne ne porte cette clé, ou quand la clé est un `rowid` et que
/// la valeur demandée n'est pas un entier.
pub fn lire_ligne(
    c: &Connection,
    table: &TableServie,
    id: &str,
) -> Result<MapJson<String, ValeurJson>, ErreurSite> {
    let valeur = if table.cle_implicite {
        let n = id.parse::<i64>().map_err(|_| {
            ErreurSite::Introuvable(format!(
                "`{}` s'adresse par son rowid, un entier ; `{id}` n'en est pas un",
                table.nom
            ))
        })?;
        ValeurSql::Integer(n)
    } else {
        ValeurSql::Text(id.to_owned())
    };

    let projection = if table.cle_implicite {
        "\"rowid\" AS \"rowid\", *"
    } else {
        "*"
    };
    let sql = format!(
        "SELECT {projection} FROM \"{}\" WHERE {} = ? LIMIT 1",
        table.nom,
        table.cle_sql()
    );
    let mut stmt = c.prepare(&sql)?;
    let colonnes: Vec<String> = stmt.column_names().into_iter().map(str::to_owned).collect();
    let mut lignes = stmt.query(rusqlite::params_from_iter([valeur].iter()))?;
    match lignes.next()? {
        Some(r) => Ok(ligne_en_json(r, &colonnes)?),
        None => Err(ErreurSite::Introuvable(format!(
            "aucune ligne de `{}` ne porte `{}` = `{id}`",
            table.nom, table.cle
        ))),
    }
}

// --------------------------------------------------------------------------------------------
// Handlers
// --------------------------------------------------------------------------------------------

/// `GET /api/v1/entites` — le catalogue des tables servables.
///
/// Chaque compte est **mesuré** à la demande, jamais écrit à la main : le miroir est régénéré
/// chaque nuit, et une liste versionnée annoncerait des tables disparues ou tairait les
/// nouvelles.
///
/// # Errors
///
/// `503` quand le miroir est absent, `500` sur erreur SQLite.
pub async fn catalogue(
    State(etat): State<EtatSite>,
    Query(brut): Query<BTreeMap<String, String>>,
) -> Result<Json<CatalogueEntites>, ErreurSite> {
    let gisement = std::sync::Arc::clone(&etat.gisement);
    let (tables, pagination, motif) = tokio::task::spawn_blocking(move || {
        let page = brut
            .get("page")
            .and_then(|v| v.trim().parse::<u32>().ok());
        let par_page = brut
            .get("par_page")
            .or_else(|| brut.get("per_page"))
            .and_then(|v| v.trim().parse::<u32>().ok());
        let motif = brut
            .get("q")
            .map(|v| v.trim().to_lowercase())
            .filter(|v| !v.is_empty());
        gisement
            .lire(catalogue_compte)
            .map(|t| (t, Pagination::borner(page, par_page), motif))
    })
    .await??;

    let retenues: Vec<&TableComptee> = tables
        .iter()
        .filter(|t| {
            motif
                .as_ref()
                .is_none_or(|m| t.table.nom.to_lowercase().contains(m))
        })
        .collect();
    let lignes_totales = retenues.iter().map(|t| t.lignes).sum();
    let total = retenues.len();
    let elements: Vec<TableComptee> = retenues
        .into_iter()
        .skip(pagination.offset())
        .take(pagination.per_page as usize)
        .cloned()
        .collect();

    Ok(Json(CatalogueEntites {
        page: Page::nouvelle(elements, pagination, total),
        lignes_totales,
        route_lignes: "/api/v1/entites/{table}",
        route_ligne: "/api/v1/entites/{table}/{id}",
    }))
}

/// `GET /api/v1/entites/{table}` — une page de lignes.
///
/// # Errors
///
/// `404` si la table n'est pas servie, `400` sur un paramètre que la route ne peut pas honorer,
/// `503` sans miroir.
pub async fn lignes(
    State(etat): State<EtatSite>,
    Path(nom): Path<String>,
    Query(brut): Query<BTreeMap<String, String>>,
) -> Result<Json<PageLignes>, ErreurSite> {
    let gisement = std::sync::Arc::clone(&etat.gisement);
    tokio::task::spawn_blocking(move || {
        gisement.lire(|c| {
            let catalogue = schema(c)?;
            let table = trouver(&catalogue, &nom)?;
            let demande = analyser(table, &brut)?;
            let page = page_lignes(c, table, &demande)?;
            Ok(Json(PageLignes {
                page,
                table: table.nom.clone(),
                cle: table.cle.clone(),
                filtres: demande.appliques(),
            }))
        })
    })
    .await?
}

/// `GET /api/v1/entites/{table}/{id}` — une ligne par sa clé.
///
/// # Errors
///
/// `404` si la table n'est pas servie ou si aucune ligne ne porte cette clé, `503` sans miroir.
pub async fn ligne(
    State(etat): State<EtatSite>,
    Path((nom, id)): Path<(String, String)>,
) -> Result<Json<LigneUnique>, ErreurSite> {
    let gisement = std::sync::Arc::clone(&etat.gisement);
    tokio::task::spawn_blocking(move || {
        gisement.lire(|c| {
            let catalogue = schema(c)?;
            let table = trouver(&catalogue, &nom)?;
            let ligne = lire_ligne(c, table, &id)?;
            Ok(Json(LigneUnique {
                table: table.nom.clone(),
                cle: table.cle.clone(),
                id,
                ligne,
            }))
        })
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une base de test qui reproduit les trois formes de clé rencontrées sur le miroir :
    /// une table à `id`, une table à clé primaire déclarée, une table sans ni l'un ni l'autre
    /// (donc `rowid`), plus une table sans aucune colonne texte.
    fn base() -> (tempfile::TempDir, crate::dataset::Gisement) {
        let dir = tempfile::tempdir().unwrap();
        let chemin = dir.path().join("mirror.sqlite");
        let c = Connection::open(&chemin).unwrap();
        c.execute_batch(
            "CREATE TABLE _meta(cle TEXT, valeur TEXT);
             INSERT INTO _meta VALUES ('source', 'pg:DATABASE_URL');
             CREATE TABLE inagle_characters(id TEXT, name_fr TEXT, element TEXT, zukan INTEGER);
             INSERT INTO inagle_characters VALUES ('c1', 'Mark', 'Feu', 1);
             INSERT INTO inagle_characters VALUES ('c2', 'Axel', 'Feu', 2);
             INSERT INTO inagle_characters VALUES ('c3', 'Jude', 'Bois', 3);
             CREATE TABLE inagle_skills(code TEXT PRIMARY KEY, libelle TEXT);
             INSERT INTO inagle_skills VALUES ('s1', 'Tornade');
             CREATE TABLE inagle_liens(source TEXT, cible TEXT);
             INSERT INTO inagle_liens VALUES ('a', 'b');
             CREATE TABLE inagle_exp_table(niveau INTEGER, exp INTEGER);
             INSERT INTO inagle_exp_table VALUES (1, 0);",
        )
        .unwrap();
        drop(c);
        let g = crate::dataset::Gisement::nouveau(&chemin);
        (dir, g)
    }

    fn q(paires: &[(&str, &str)]) -> BTreeMap<String, String> {
        paires
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn le_catalogue_est_mesure_et_exclut_la_plomberie() {
        let (_d, g) = base();
        let cat = g.lire(catalogue_compte).unwrap();
        let noms: Vec<&str> = cat.iter().map(|t| t.table.nom.as_str()).collect();
        assert_eq!(
            noms,
            ["inagle_characters", "inagle_exp_table", "inagle_liens", "inagle_skills"],
            "`_meta` n'est pas servie"
        );
        let chara = &cat[0];
        assert_eq!(chara.lignes, 3);
        assert_eq!(chara.table.cle, "id");
        assert!(!chara.table.cle_implicite);
        assert_eq!(chara.table.colonnes.len(), 4);
        // La clé primaire déclarée l'emporte, et une table sans rien retombe sur `rowid`.
        let skills = cat.iter().find(|t| t.table.nom == "inagle_skills").unwrap();
        assert_eq!(skills.table.cle, "code");
        let liens = cat.iter().find(|t| t.table.nom == "inagle_liens").unwrap();
        assert_eq!(liens.table.cle, "rowid");
        assert!(liens.table.cle_implicite);
    }

    #[test]
    fn une_table_inconnue_est_un_404_et_n_execute_rien() {
        let (_d, g) = base();
        let cat = g.lire(schema).unwrap();
        for tentative in [
            "table_qui_n_existe_pas",
            "inagle_characters; DROP TABLE inagle_characters",
            "sqlite_master",
            "_meta",
            "\"inagle_characters\"",
            "inagle_characters--",
        ] {
            let e = trouver(&cat, tentative).unwrap_err();
            assert_eq!(e.statut().as_u16(), 404, "`{tentative}` doit etre refusee");
        }
        // Preuve par falsification : la base est intacte, donc rien n'a été exécuté.
        assert_eq!(
            g.compte_table("inagle_characters").unwrap(),
            Some(3),
            "aucune tentative n'a touche la base"
        );
    }

    #[test]
    fn une_colonne_fabriquee_est_un_400_et_n_entre_pas_dans_le_sql() {
        let (_d, g) = base();
        let cat = g.lire(schema).unwrap();
        let t = trouver(&cat, "inagle_characters").unwrap();
        for (cle, valeur) in [
            ("colonne_inventee", "x"),
            ("id\" OR 1=1 --", "x"),
            ("name_fr; DROP TABLE inagle_characters", "x"),
        ] {
            let e = analyser(t, &q(&[(cle, valeur)])).unwrap_err();
            assert_eq!(e.statut().as_u16(), 400, "`{cle}` doit etre refusee");
        }
        for tri in ["colonne_inventee", "id\" DESC --", "(SELECT 1)"] {
            let e = analyser(t, &q(&[("tri", tri)])).unwrap_err();
            assert_eq!(e.statut().as_u16(), 400, "`tri={tri}` doit etre refuse");
        }
        assert_eq!(g.compte_table("inagle_characters").unwrap(), Some(3));
    }

    #[test]
    fn le_filtre_d_egalite_est_honore() {
        let (_d, g) = base();
        let cat = g.lire(schema).unwrap();
        let t = trouver(&cat, "inagle_characters").unwrap();
        let d = analyser(t, &q(&[("element", "Feu")])).unwrap();
        assert_eq!(d.egalites, vec![("element".to_owned(), "Feu".to_owned())]);
        let page = g.lire(|c| page_lignes(c, t, &d)).unwrap();
        assert_eq!(page.total, 2, "deux personnages de Feu, pas trois");
        assert!(page.elements.iter().all(|l| l["element"] == "Feu"));
        // Falsification : sans le filtre, le total remonte à 3.
        let sans = analyser(t, &q(&[])).unwrap();
        assert_eq!(g.lire(|c| page_lignes(c, t, &sans)).unwrap().total, 3);
    }

    #[test]
    fn q_cherche_vraiment_sur_les_colonnes_texte() {
        let (_d, g) = base();
        let cat = g.lire(schema).unwrap();
        let t = trouver(&cat, "inagle_characters").unwrap();
        let d = analyser(t, &q(&[("q", "mar")])).unwrap();
        let page = g.lire(|c| page_lignes(c, t, &d)).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.elements[0]["name_fr"], "Mark");
        // Un motif absent ne rend rien — sans quoi `q` serait un paramètre décoratif.
        let vide = analyser(t, &q(&[("q", "zzzz")])).unwrap();
        assert_eq!(g.lire(|c| page_lignes(c, t, &vide)).unwrap().total, 0);
        // Le joker du client est échappé : `%` est un pourcent, pas « tout ».
        let joker = analyser(t, &q(&[("q", "%")])).unwrap();
        assert_eq!(g.lire(|c| page_lignes(c, t, &joker)).unwrap().total, 0);
    }

    #[test]
    fn q_sur_une_table_sans_colonne_texte_est_un_400_franc() {
        let (_d, g) = base();
        let cat = g.lire(schema).unwrap();
        let t = trouver(&cat, "inagle_exp_table").unwrap();
        assert!(t.colonnes_texte().is_empty());
        let e = analyser(t, &q(&[("q", "1")])).unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("colonne texte"), "{e}");
        // Sans `q`, la même table se lit parfaitement.
        assert!(analyser(t, &q(&[])).is_ok());
    }

    #[test]
    fn le_tri_et_l_ordre_sont_honores_ou_refuses() {
        let (_d, g) = base();
        let cat = g.lire(schema).unwrap();
        let t = trouver(&cat, "inagle_characters").unwrap();
        let noms = |d: &Demande| -> Vec<String> {
            g.lire(|c| page_lignes(c, t, d))
                .unwrap()
                .elements
                .iter()
                .map(|l| l["name_fr"].as_str().unwrap().to_owned())
                .collect()
        };
        let asc = analyser(t, &q(&[("tri", "name_fr")])).unwrap();
        assert_eq!(noms(&asc), ["Axel", "Jude", "Mark"]);
        let desc = analyser(t, &q(&[("tri", "name_fr"), ("ordre", "desc")])).unwrap();
        assert_eq!(noms(&desc), ["Mark", "Jude", "Axel"]);
        assert_eq!(desc.appliques().ordre, "desc");

        let e = analyser(t, &q(&[("ordre", "aleatoire")])).unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
    }

    #[test]
    fn la_pagination_est_bornee_et_le_non_numerique_refuse() {
        let (_d, g) = base();
        let cat = g.lire(schema).unwrap();
        let t = trouver(&cat, "inagle_characters").unwrap();
        let d = analyser(t, &q(&[("page", "2"), ("par_page", "1")])).unwrap();
        let page = g.lire(|c| page_lignes(c, t, &d)).unwrap();
        assert_eq!(page.elements.len(), 1);
        assert_eq!(page.total, 3);
        assert_eq!(page.pages, 3);
        assert_eq!(page.elements[0]["id"], "c2");
        // Le plafond dur s'applique, quoi que le client demande.
        let enorme = analyser(t, &q(&[("par_page", "100000")])).unwrap();
        assert_eq!(enorme.pagination.per_page, crate::config::PER_PAGE_MAX);
        // `per_page` reste accepté, comme partout ailleurs dans l'API.
        assert_eq!(
            analyser(t, &q(&[("per_page", "7")])).unwrap().pagination.per_page,
            7
        );
        let e = analyser(t, &q(&[("page", "deux")])).unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
    }

    #[test]
    fn un_filtre_a_valeur_vide_est_refuse_plutot_que_devine() {
        let (_d, g) = base();
        let cat = g.lire(schema).unwrap();
        let t = trouver(&cat, "inagle_characters").unwrap();
        let e = analyser(t, &q(&[("element", "")])).unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
    }

    #[test]
    fn une_ligne_se_lit_par_sa_cle_et_par_son_rowid() {
        let (_d, g) = base();
        let cat = g.lire(schema).unwrap();
        let chara = trouver(&cat, "inagle_characters").unwrap();
        let l = g.lire(|c| lire_ligne(c, chara, "c2")).unwrap();
        assert_eq!(l["name_fr"], "Axel");
        let e = g.lire(|c| lire_ligne(c, chara, "c999")).unwrap_err();
        assert_eq!(e.statut().as_u16(), 404);

        let liens = trouver(&cat, "inagle_liens").unwrap();
        let l = g.lire(|c| lire_ligne(c, liens, "1")).unwrap();
        assert_eq!(l["source"], "a");
        assert_eq!(l["rowid"], 1);
        let e = g.lire(|c| lire_ligne(c, liens, "pas_un_entier")).unwrap_err();
        assert_eq!(e.statut().as_u16(), 404);
    }

    #[test]
    fn sans_miroir_les_lectures_sont_indisponibles() {
        let g = crate::dataset::Gisement::nouveau("/nonexistent/mirror.sqlite");
        assert_eq!(g.lire(schema).unwrap_err().statut().as_u16(), 503);
        assert_eq!(g.lire(catalogue_compte).unwrap_err().statut().as_u16(), 503);
    }

    #[test]
    fn la_garde_de_forme_refuse_ce_qui_n_est_pas_un_identifiant() {
        assert!(nom_sql_valide("inagle_characters"));
        assert!(nom_sql_valide("_meta"));
        assert!(!nom_sql_valide(""));
        assert!(!nom_sql_valide("a b"));
        assert!(!nom_sql_valide("a\"b"));
        assert!(!nom_sql_valide("a;b"));
        assert!(!nom_sql_valide("1table"));
        assert!(!nom_sql_valide(&"x".repeat(NOM_MAX + 1)));
    }

    /// Sert les trois routes sur `127.0.0.1:8099`, contre le **vrai** miroir du dépôt, le temps
    /// de les interroger au `curl`.
    ///
    /// `#[ignore]` : elle a besoin de `var/mirror.sqlite` et d'un port libre, deux choses qu'une
    /// suite ne doit pas exiger. Elle existe parce qu'un test qui appelle le handler ne prouve
    /// pas le routeur — le dépôt l'a déjà payé (`/en/manifest.webmanifest` rendait du HTML
    /// pendant que son test unitaire était vert). Ici le montage reste **local** : câbler ces
    /// routes dans `app::routeur` n'est pas du ressort de ce module.
    ///
    /// ```text
    /// cargo test -p nie-site --lib -- --ignored --nocapture sert_en_reel
    /// ```
    ///
    /// La durée se règle par `NIE_SITE_ESSAI_SECONDES` (60 par défaut) : bornée, pour qu'un
    /// oubli ne laisse pas un port pris.
    #[tokio::test]
    #[ignore = "exige var/mirror.sqlite et le port 8099"]
    async fn sert_en_reel_sur_8099() {
        use std::future::IntoFuture as _;

        let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap()
            .to_path_buf();
        let config = crate::config::Config {
            db: racine.join("var/mirror.sqlite"),
            ..crate::config::Config::default()
        };
        assert!(config.db.is_file(), "miroir absent: {}", config.db.display());
        let etat = EtatSite::nouveau(config);
        let app = axum::Router::new()
            .route("/api/v1/entites", axum::routing::get(catalogue))
            .route("/api/v1/entites/{table}", axum::routing::get(lignes))
            .route("/api/v1/entites/{table}/{id}", axum::routing::get(ligne))
            .with_state(etat);
        let ecoute = tokio::net::TcpListener::bind("127.0.0.1:8099").await.unwrap();
        let secondes = std::env::var("NIE_SITE_ESSAI_SECONDES")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60);
        println!("essai en ecoute sur 127.0.0.1:8099 pendant {secondes} s");
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(secondes),
            axum::serve(ecoute, app).into_future(),
        )
        .await;
    }

    #[test]
    fn l_affinite_texte_suit_la_regle_de_sqlite() {
        assert!(colonne_texte("TEXT"));
        assert!(colonne_texte("varchar(50)"));
        assert!(colonne_texte("NATIVE CHARACTER"));
        assert!(colonne_texte("CLOB"));
        assert!(!colonne_texte("INTEGER"));
        assert!(!colonne_texte("REAL"));
        assert!(!colonne_texte(""), "un type absent a l'affinite BLOB");
    }
}
