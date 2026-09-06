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
//! ## Trois formes de filtre, parce que l'égalité seule ment par omission
//!
//! `scripts/validation/mesurer-matrice-filtres.sh` a mesuré le 2026-09-06 que deux des quatorze
//! manques restants n'étaient pas des données absentes mais des **formes** que cette route ne
//! savait pas exprimer : « puissance entre 400 et 880 » et « a une vidéo ». Les colonnes
//! existaient ; seule l'égalité était servie, et l'égalité ne sait dire ni l'intervalle ni la
//! présence. Une facette qui n'existe pas est un manque visible ; une facette qu'on approxime
//! par l'égalité est un résultat faux.
//!
//! | Forme | Écriture | Sur quelles colonnes |
//! |---|---|---|
//! | Égalité | `?element=Feu` | toutes |
//! | Intervalle | `?power_max__min=400&power_max__max=880` | **numériques seulement** |
//! | Présence | `?video_url=__present__` / `__absent__` | toutes |
//!
//! Chacune refuse plutôt que d'approximer, et c'est le point :
//!
//! - `__min`/`__max` sur une colonne **texte** est un `400`. SQLite comparerait volontiers
//!   `'Mark' >= '400'` par ordre lexicographique et rendrait une page pleine de lignes
//!   plausibles : le pire résultat possible, faux sans en avoir l'air ;
//! - une borne non numérique est un `400` ;
//! - `__present__` compte `NULL` **et** la chaîne vide comme absents. Le miroir mélange les deux
//!   (`age_group` est vide, pas nul, sur les 6 166 personnages) ; distinguer un `NULL` d'un `''`
//!   ici publierait une nuance de l'importeur, pas une du jeu ;
//! - un `?colonne=__present__` sur une colonne qui contiendrait littéralement la chaîne
//!   `__present__` serait détourné. Mesuré avant d'écrire : **0 occurrence** des deux jetons
//!   dans les 165 249 lignes des 219 tables.
//!
//! ## L'export : la même page, dans un autre format
//!
//! `?format=csv` rend la page **exactement telle qu'elle est filtrée et triée**, en CSV, avec
//! un `Content-Disposition` dont le nom porte la table et la page — jamais un nom générique,
//! sinon deux exports se recouvrent dans le dossier de téléchargement (leçon déjà payée sur
//! les cues audio).
//!
//! Ce n'est pas un dump : la pagination continue de s'appliquer, `per_page` reste plafonné, et
//! l'export d'un corpus entier passe par autant d'appels que de pages. Un export qui
//! ignorerait la pagination serait une seconde route déguisée, avec un coût que personne
//! n'aurait choisi.
//!
//! Sans miroir, les trois routes répondent `503` avec la raison : le service démarre toujours.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
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
pub const PARAMS_RESERVES: [&str; 8] = [
    "page", "par_page", "per_page", "tri", "ordre", "q", "format", "facets",
];

/// Combien de colonnes une seule demande peut faceter.
///
/// Chaque facette est un `GROUP BY` de plus sur la même table : douze est déjà généreux pour
/// une barre de filtres, et au-delà c'est un client qui demande le schéma entier en croyant
/// demander des filtres. **Refusé**, pas tronqué — une demande tronquée en silence rend des
/// comptes justes sur des colonnes que le client croyait avoir demandées en plus.
pub const FACETS_MAX: usize = 12;

/// Combien de valeurs distinctes une facette republie.
///
/// Au-delà, la liste est **coupée et le dit** (`truncated`), avec le nombre de valeurs
/// distinctes réellement présentes (`distinct`) : une facette de 199 équipes se dessine en
/// « les 60 plus fournies + 139 autres », jamais en une liste qui ment sur sa longueur.
pub const FACET_VALEURS_MAX: usize = 60;

/// Les formats de sortie servis par `/api/v1/entites/{table}`.
///
/// Volontairement courte : `json` (le défaut) et `csv`. Un format inconnu est un `400` — le
/// rendre en JSON « par défaut » ferait télécharger un fichier au mauvais format sans un mot.
pub const FORMATS: [&str; 2] = ["json", "csv"];

/// Suffixe de borne basse — `?power_max__min=400`.
pub const SUFFIXE_MIN: &str = "__min";

/// Suffixe de borne haute — `?power_max__max=880`.
pub const SUFFIXE_MAX: &str = "__max";

/// Valeur-jeton demandant les lignes où la colonne est renseignée.
///
/// Mesuré avant d'être choisi : **0 des 165 249 lignes** des 219 tables ne porte cette chaîne,
/// donc aucun filtre d'égalité légitime n'est détourné par elle.
pub const JETON_PRESENT: &str = "__present__";

/// Valeur-jeton demandant les lignes où la colonne est nulle ou vide.
pub const JETON_ABSENT: &str = "__absent__";

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

/// Nom public du gisement du miroir `inagle_*`.
pub const GISEMENT_EXTRAIT: &str = "extrait";

/// Nom public du gisement des épisodes de la série.
pub const GISEMENT_ANIME: &str = "anime";

/// Une table servable, avec son schéma mesuré.
#[derive(Debug, Clone, Serialize)]
pub struct TableServie {
    /// Gisement d'où elle vient — `extrait` (le miroir) ou `anime` (la série).
    ///
    /// Publié, parce que sans lui la route ferait passer deux corpus pour un seul. Ils n'ont
    /// **aucune clé commune** (CLAUDE.md § *Les quatre gisements*) : les servir par la même
    /// route générique n'est pas les joindre, et un client qui les croirait joignables se
    /// tromperait sans que rien ne l'en avertisse.
    pub gisement: &'static str,
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
    /// Bornes retenues, `colonne__min` / `colonne__max` → valeur numérique.
    pub bornes: BTreeMap<String, f64>,
    /// Tests de présence retenus, colonne → `"present"` ou `"absent"`.
    pub presences: BTreeMap<String, &'static str>,
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
    /// Bornes demandées : colonne du catalogue, sens, valeur.
    pub bornes: Vec<(String, Borne, f64)>,
    /// Présences demandées : colonne du catalogue, et si l'on veut ce qui est renseigné.
    pub presences: Vec<(String, bool)>,
    /// Colonnes à faceter — des noms du catalogue, jamais ceux envoyés par le client.
    pub facets: Vec<String>,
}

/// Une valeur d'une facette, et combien de lignes la portent.
#[derive(Debug, Clone, Serialize)]
pub struct FacetValeur {
    /// La valeur, telle qu'elle est stockée. `null` quand la colonne est vide ou nulle.
    pub value: Option<String>,
    /// Combien de lignes la portent, **sous les autres filtres en cours**.
    pub count: i64,
}

/// Les valeurs d'une colonne, comptées — de quoi dessiner une barre de filtres.
///
/// ## Ce que c'est, et ce que ce n'est pas
///
/// C'est le seul moyen de dessiner un filtre honnête : sans les valeurs et leurs comptes, une
/// interface ne peut proposer qu'un champ de texte libre, où l'utilisateur devine. Avec eux,
/// elle montre `feu 1 203`, et un choix qui rendrait zéro ligne **ne s'affiche pas**.
///
/// ## Le compte est calculé sans le filtre de SA propre colonne
///
/// C'est la seule définition qui rend une facette multi-sélectionnable utilisable. Avec
/// `?element=fire`, la facette `element` calculée sous tous les filtres rendrait une seule
/// valeur — `fire`, et son compte — et l'interface ne pourrait plus proposer « ajouter
/// `wind` ». Les autres filtres (`q`, bornes, présences, et les égalités des **autres**
/// colonnes) s'appliquent bien : c'est ce qui fait que les comptes correspondent à l'écran.
#[derive(Debug, Clone, Serialize)]
pub struct Facet {
    /// La colonne facetée.
    pub column: String,
    /// Combien de valeurs distinctes elle porte sous les filtres en cours.
    pub distinct: usize,
    /// Vrai quand la liste a été coupée à [`FACET_VALEURS_MAX`].
    pub truncated: bool,
    /// Les valeurs, les plus fournies d'abord.
    pub values: Vec<FacetValeur>,
}

/// Le sens d'une borne. Deux variantes closes, jamais une chaîne du client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Borne {
    /// `>=` — le suffixe `__min`.
    Minimum,
    /// `<=` — le suffixe `__max`.
    Maximum,
}

impl Borne {
    /// L'opérateur SQL. **Constant** : aucune chaîne du client n'entre là.
    #[must_use]
    pub fn sql(self) -> &'static str {
        match self {
            Self::Minimum => ">=",
            Self::Maximum => "<=",
        }
    }

    /// Le suffixe public, celui que la réponse republie.
    #[must_use]
    pub fn suffixe(self) -> &'static str {
        match self {
            Self::Minimum => SUFFIXE_MIN,
            Self::Maximum => SUFFIXE_MAX,
        }
    }
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
            bornes: self
                .bornes
                .iter()
                .map(|(c, b, v)| (format!("{c}{}", b.suffixe()), *v))
                .collect(),
            presences: self
                .presences
                .iter()
                .map(|(c, present)| {
                    (
                        c.clone(),
                        if *present { "present" } else { "absent" },
                    )
                })
                .collect(),
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
    /// Gisement d'où la table vient.
    pub gisement: &'static str,
    /// Nom de la table lue.
    pub table: String,
    /// Colonne qui identifie une ligne — celle que `/api/v1/entites/{table}/{id}` attend.
    pub cle: String,
    /// Ce que la route a appliqué.
    pub filtres: FiltresAppliques,
    /// Les facettes demandées, comptées sous les filtres en cours. Absent quand `?facets` ne
    /// l'est pas — une clé toujours présente et toujours vide ferait croire à une capacité
    /// absente.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub facets: Vec<Facet>,
}

/// Une ligne unique.
#[derive(Debug, Serialize)]
pub struct LigneUnique {
    /// Gisement d'où la table vient.
    pub gisement: &'static str,
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
    schema_de(c, GISEMENT_EXTRAIT)
}

/// Le schéma d'une connexion, étiqueté par le gisement d'où elle vient.
///
/// # Errors
///
/// Toute erreur SQLite.
pub fn schema_de(c: &Connection, gisement: &'static str) -> Result<Vec<TableServie>, ErreurSite> {
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
            gisement,
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

    // Les facettes se declarent en UNE liste, pas en un parametre par colonne : un parametre
    // par colonne serait indistinguable d'une egalite (`?element=` est deja refuse comme
    // « valeur de filtre vide »), et le client ne saurait plus ce qu'il demande.
    let mut facets: Vec<String> = Vec::new();
    if let Some(liste) = brut.get("facets").map(|v| v.trim()).filter(|v| !v.is_empty()) {
        for demande in liste.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let colonne = table.colonne(demande).ok_or_else(|| {
                ErreurSite::Demande(format!(
                    "`facets={demande}` : `{}` n'a pas cette colonne ; son schema est sur \
                     /api/v1/entites",
                    table.nom
                ))
            })?;
            if !facets.contains(&colonne.nom) {
                facets.push(colonne.nom.clone());
            }
        }
        if facets.len() > FACETS_MAX {
            return Err(ErreurSite::Demande(format!(
                "`facets` : {} colonnes demandees, {FACETS_MAX} au maximum — au-dela c'est le \
                 schema qui est demande, pas des filtres",
                facets.len()
            )));
        }
    }

    let mut egalites = Vec::new();
    let mut bornes = Vec::new();
    let mut presences = Vec::new();
    for (cle, valeur) in brut {
        if PARAMS_RESERVES.contains(&cle.as_str()) {
            continue;
        }

        // Une borne se reconnait au SUFFIXE du parametre, pas a sa valeur : `power_max__min`
        // vise la colonne `power_max`. Le suffixe est retire avant de chercher la colonne, sans
        // quoi la recherche echouerait sur un nom qui n'existe pas.
        if let Some((base, sens)) = cle
            .strip_suffix(SUFFIXE_MIN)
            .map(|b| (b, Borne::Minimum))
            .or_else(|| cle.strip_suffix(SUFFIXE_MAX).map(|b| (b, Borne::Maximum)))
        {
            let colonne = table.colonne(base).ok_or_else(|| {
                ErreurSite::Demande(format!(
                    "`{cle}` : `{}` n'a pas de colonne `{base}` ; son schema est sur \
                     /api/v1/entites",
                    table.nom
                ))
            })?;
            // Refuser plutot qu'approximer : SQLite comparerait `'Mark' >= '400'` par ordre
            // lexicographique et rendrait une page de lignes plausibles — faux sans en avoir
            // l'air, ce qui est le pire des resultats.
            if colonne.texte {
                return Err(ErreurSite::Demande(format!(
                    "`{cle}` : `{base}` est une colonne texte ({}), et une fourchette sur du \
                     texte comparerait des mots par ordre alphabetique ; utilisez l'egalite",
                    colonne.type_sql
                )));
            }
            let n: f64 = valeur.trim().parse().map_err(|_| {
                ErreurSite::Demande(format!("`{cle}={valeur}` : une borne est un nombre"))
            })?;
            bornes.push((colonne.nom.clone(), sens, n));
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
        // La presence se reconnait a la VALEUR, parce qu'elle porte sur la colonne elle-meme et
        // non sur un contenu. Les deux jetons sont surs : 0 des 165 249 lignes ne les porte.
        match valeur.as_str() {
            JETON_PRESENT => presences.push((colonne.nom.clone(), true)),
            JETON_ABSENT => presences.push((colonne.nom.clone(), false)),
            _ => egalites.push((colonne.nom.clone(), valeur.clone())),
        }
    }
    egalites.sort_by(|a, b| a.0.cmp(&b.0));
    bornes.sort_by(|a, b| (&a.0, a.1.suffixe()).cmp(&(&b.0, b.1.suffixe())));
    presences.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(Demande {
        pagination,
        q,
        tri,
        ordre,
        egalites,
        bornes,
        presences,
        facets,
    })
}

/// Construit la clause `WHERE` d'une demande déjà validée.
///
/// Aucune valeur n'entre dans le texte : seuls des `?` y entrent, et les noms de colonnes
/// viennent de `table`, c'est-à-dire de la base.
#[must_use]
pub fn clause(table: &TableServie, d: &Demande) -> Clause {
    clause_sauf(table, d, None)
}

/// La même clause, en **retirant** l'égalité portant sur une colonne donnée.
///
/// C'est ce qui rend une facette multi-sélectionnable : le compte de `element` se calcule sous
/// tous les filtres SAUF le sien, sinon `?element=fire` ferait rendre à la facette une unique
/// valeur et l'interface ne pourrait plus proposer d'en ajouter une seconde. Seule l'**égalité**
/// est retirée — une borne ou une présence sur la même colonne reste appliquée, parce qu'elles
/// ne se cumulent pas avec un choix de valeur, elles le restreignent.
///
/// Aucune valeur n'entre dans le texte : seuls des `?` y entrent, et les noms de colonnes
/// viennent de `table`.
#[must_use]
pub fn clause_sauf(table: &TableServie, d: &Demande, sauf: Option<&str>) -> Clause {
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
        if sauf == Some(colonne.as_str()) {
            continue;
        }
        morceaux.push(format!("\"{colonne}\" = ?"));
        params.push(ValeurSql::Text(valeur.clone()));
    }

    for (colonne, sens, valeur) in &d.bornes {
        // `CAST(... AS REAL)` parce que l'importeur a stocke des nombres dans des colonnes
        // declarees INTEGER *et* dans des colonnes declarees REAL, et que SQLite compare selon
        // le TYPE STOCKE : sans le cast, une valeur ecrite en texte serait comparee comme du
        // texte, ce que le refus ci-dessus vise justement a empecher.
        morceaux.push(format!(
            "CAST(\"{colonne}\" AS REAL) {} ?",
            sens.sql()
        ));
        params.push(ValeurSql::Real(*valeur));
    }

    for (colonne, present) in &d.presences {
        // `NULL` et la chaine vide comptent pour la meme chose : le miroir melange les deux
        // (`age_group` est vide, pas nul), et publier la nuance publierait une propriete de
        // l'importeur, pas une du jeu.
        morceaux.push(if *present {
            format!("(\"{colonne}\" IS NOT NULL AND \"{colonne}\" != '')")
        } else {
            format!("(\"{colonne}\" IS NULL OR \"{colonne}\" = '')")
        });
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
    catalogue_compte_de(c, GISEMENT_EXTRAIT)
}

/// Le catalogue d'une connexion, étiqueté par son gisement.
///
/// # Errors
///
/// Toute erreur SQLite.
pub fn catalogue_compte_de(
    c: &Connection,
    gisement: &'static str,
) -> Result<Vec<TableComptee>, ErreurSite> {
    let mut sortie = Vec::new();
    for table in schema_de(c, gisement)? {
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

/// Compte les valeurs de chaque colonne facetée, sous les filtres en cours.
///
/// Un `GROUP BY` par colonne demandée, chacun sous [`clause_sauf`] — c'est-à-dire sous tous les
/// filtres **sauf l'égalité de cette colonne-là** (cf. [`Facet`]). Les valeurs les plus
/// fournies d'abord, à égalité par ordre alphabétique pour que deux appels rendent la même
/// liste.
///
/// `distinct` est compté **avant** la coupe : une facette qui rend 60 valeurs sur 199 le dit,
/// au lieu de laisser croire que la colonne n'en porte que 60.
///
/// # Errors
///
/// Toute erreur SQLite.
pub fn facettes(
    c: &Connection,
    table: &TableServie,
    d: &Demande,
) -> Result<Vec<Facet>, ErreurSite> {
    let mut sorties = Vec::with_capacity(d.facets.len());
    for colonne in &d.facets {
        let cl = clause_sauf(table, d, Some(colonne));
        // `NULL` et la chaine vide sont rendus comme UNE seule valeur nulle : le miroir melange
        // les deux (`age_group` est vide, pas nul), et les distinguer publierait une propriete
        // de l'importeur, pas une du jeu. Meme choix que les tests de presence.
        let expr = format!("nullif(\"{colonne}\", '')");
        // Le compte distinct passe par le MEME `GROUP BY` que la liste, pas par un
        // `count(DISTINCT ...)` : ce dernier ignore les `NULL`, si bien qu'une colonne dont la
        // moitie des lignes est vide se serait annoncee avec une valeur distincte de moins que
        // celles qu'elle rend. Ici les deux requetes voient exactement les memes groupes.
        let distinct: i64 = c.query_row(
            &format!(
                "SELECT count(*) FROM (SELECT {expr} AS v FROM \"{}\"{} GROUP BY v)",
                table.nom, cl.sql
            ),
            rusqlite::params_from_iter(cl.params.iter()),
            |r| r.get(0),
        )?;

        let sql = format!(
            "SELECT {expr} AS v, count(*) AS n FROM \"{}\"{} GROUP BY v \
             ORDER BY n DESC, v IS NULL, v ASC LIMIT ?",
            table.nom, cl.sql
        );
        let mut stmt = c.prepare(&sql)?;
        let mut params = cl.params.clone();
        params.push(ValeurSql::Integer(
            i64::try_from(FACET_VALEURS_MAX).unwrap_or(i64::MAX),
        ));
        let values = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |r| {
                Ok(FacetValeur {
                    value: r.get::<_, Option<String>>(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let distinct = usize::try_from(distinct).unwrap_or(values.len());
        sorties.push(Facet {
            column: colonne.clone(),
            distinct,
            truncated: values.len() < distinct,
            values,
        });
    }
    Ok(sorties)
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
    let anime = std::sync::Arc::clone(&etat.anime);
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
        // Les deux gisements sont concaténés, jamais fusionnés : chaque table dit d'où elle
        // vient. Un gisement absent n'est pas une erreur — il manque de son catalogue, et le
        // reste répond. C'est la même règle qu'au démarrage : un corpus absent dégrade, il
        // n'éteint pas.
        let mut t = gisement.lire(catalogue_compte)?;
        if anime.present() {
            t.extend(anime.lire(|c| catalogue_compte_de(c, GISEMENT_ANIME))?);
        }
        Ok::<_, ErreurSite>((t, Pagination::borner(page, par_page), motif))
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
) -> Result<axum::response::Response, ErreurSite> {
    let gisement = std::sync::Arc::clone(&etat.gisement);
    let anime = std::sync::Arc::clone(&etat.anime);
    tokio::task::spawn_blocking(move || {
        let csv = match brut.get("format").map(|v| v.trim().to_ascii_lowercase()) {
            None => false,
            Some(f) if f == "json" => false,
            Some(f) if f == "csv" => true,
            Some(f) => {
                return Err(ErreurSite::Demande(format!(
                    "`format={f}` : seuls {} sont servis",
                    FORMATS.join(" et ")
                )));
            }
        };
        dans_le_gisement(&gisement, &anime, &nom, |c, table| {
            let demande = analyser(table, &brut)?;
            let page = page_lignes(c, table, &demande)?;
            let corps = PageLignes {
                page,
                gisement: table.gisement,
                table: table.nom.clone(),
                cle: table.cle.clone(),
                filtres: demande.appliques(),
                facets: facettes(c, table, &demande)?,
            };
            Ok(if csv {
                reponse_csv(&corps)
            } else {
                Json(corps).into_response()
            })
        })
    })
    .await?
}

/// Rend une page en CSV, avec le nom de fichier qui la désigne.
///
/// Les colonnes sont l'UNION des clés rencontrées, dans l'ordre stable de `serde_json::Map` —
/// une ligne du miroir peut omettre une colonne nulle, et prendre les clés de la première
/// ligne perdrait silencieusement les colonnes suivantes.
fn reponse_csv(p: &PageLignes) -> axum::response::Response {
    let mut colonnes: Vec<String> = Vec::new();
    for ligne in &p.page.elements {
        for cle in ligne.keys() {
            if !colonnes.iter().any(|c| c == cle) {
                colonnes.push(cle.clone());
            }
        }
    }
    let mut corps = String::new();
    corps.push_str(&colonnes.iter().map(|c| echapper_csv(c)).collect::<Vec<_>>().join(","));
    corps.push('\n');
    for ligne in &p.page.elements {
        let cellules: Vec<String> = colonnes
            .iter()
            .map(|c| match ligne.get(c) {
                None | Some(ValeurJson::Null) => String::new(),
                Some(ValeurJson::String(s)) => echapper_csv(s),
                Some(v) => echapper_csv(&v.to_string()),
            })
            .collect();
        corps.push_str(&cellules.join(","));
        corps.push('\n');
    }
    // Le nom porte la TABLE et la PAGE : sans la seconde, deux exports du même corpus se
    // recouvrent dans le dossier de téléchargement et le lecteur croit n'en avoir qu'un.
    let nom = format!("{}-page{}.csv", p.table, p.page.page);
    let mut reponse = (
        [(
            header::CONTENT_TYPE,
            "text/csv; charset=utf-8",
        )],
        corps,
    )
        .into_response();
    if let Ok(v) = format!("attachment; filename=\"{nom}\"").parse() {
        reponse.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }
    reponse
}

/// Échappe une cellule CSV selon RFC 4180.
///
/// Le guillemet se double, et toute cellule qui porte une virgule, un guillemet ou un saut de
/// ligne est encadrée. Les descriptions du jeu contiennent les trois : sans cet échappement,
/// une seule ligne décale toutes les colonnes de la suivante, et le fichier s'ouvre
/// « correctement » avec des valeurs dans les mauvaises cases.
fn echapper_csv(v: &str) -> String {
    if v.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_owned()
    }
}

/// Exécute une lecture sur le gisement qui porte cette table, le miroir d'abord.
///
/// L'ordre n'est pas arbitraire : le miroir est le corpus de loin le plus consulté (219 tables
/// contre 5), et ses noms sont préfixés `inagle_`, donc aucune collision n'est possible avec
/// ceux de la série. Le second gisement n'est ouvert que si le premier ne connaît pas le nom.
///
/// # Errors
///
/// `404` si aucun des deux ne sert cette table — avec le compte des deux catalogues, pour que
/// le message ne laisse pas croire qu'un seul a été consulté.
pub fn dans_le_gisement<T>(
    miroir: &crate::dataset::Gisement,
    anime: &crate::dataset::Gisement,
    nom: &str,
    f: impl FnOnce(&Connection, &TableServie) -> Result<T, ErreurSite>,
) -> Result<T, ErreurSite> {
    // `Option` puis `take` : `f` est un `FnOnce` et ne peut pas entrer dans deux fermetures,
    // alors qu'il n'est appelé qu'une fois — sur le gisement qui porte la table.
    let mut f = Some(f);
    let mut connues = 0usize;
    let mut premiere_erreur = None;
    let mut consulte = 0usize;

    for (gisement, etiquette) in [(miroir, GISEMENT_EXTRAIT), (anime, GISEMENT_ANIME)] {
        // Un gisement absent n'éteint pas la route : il manque de son catalogue, l'autre
        // répond. C'est la même règle qu'au démarrage — un corpus absent dégrade.
        if !gisement.present() {
            continue;
        }
        consulte += 1;
        let sortie = gisement.lire(|c| {
            let catalogue = schema_de(c, etiquette)?;
            let n = catalogue.len();
            match catalogue.iter().find(|t| t.nom.eq_ignore_ascii_case(nom)) {
                Some(table) if nom_sql_valide(nom) => {
                    // `take` ne peut rendre `None` ici : la boucle sort dès qu'il a servi.
                    let r = f.take().expect("f n'est consommee qu'une fois")(c, table)?;
                    Ok((Some(r), n))
                }
                _ => Ok((None, n)),
            }
        });
        match sortie {
            Ok((Some(v), _)) => return Ok(v),
            Ok((None, n)) => connues += n,
            // Une erreur APRES que `f` ait ete consommee vient de `f`, pas de la recherche :
            // c'est le `400` d'un `tri=` sur une colonne inconnue, ou une panne SQLite sur la
            // bonne table. La retenir pour continuer la boucle la transformerait en `404`
            // « aucune table ne se nomme ainsi » — un message qui envoie corriger un nom de
            // table parfaitement juste. Mesure du 2026-09-06 : c'est exactement ce que la
            // premiere version faisait sur `entites/episodes?tri=pertinence`.
            Err(e) if f.is_none() => return Err(e),
            Err(e) => {
                premiere_erreur.get_or_insert(e);
            }
        }
    }

    // Aucun gisement lisible : c'est une indisponibilité, pas un 404. Les confondre ferait
    // passer une panne — ou un miroir qui n'a pas encore tourné — pour une table inexistante,
    // et un client corrigerait alors son URL au lieu d'attendre. Deux causes, même conclusion :
    // aucun fichier sur le disque, ou tous illisibles.
    if consulte == 0 {
        return Err(ErreurSite::Indisponible(format!(
            "aucun gisement n'est monte : ni le miroir ({}) ni le catalogue de la serie ({})",
            miroir.chemin().display(),
            anime.chemin().display()
        )));
    }
    if connues == 0 && let Some(e) = premiere_erreur {
        return Err(e);
    }
    Err(ErreurSite::Introuvable(format!(
        "aucune des {connues} tables servies ne se nomme `{nom}` ; elles sont sur /api/v1/entites"
    )))
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
    let anime = std::sync::Arc::clone(&etat.anime);
    tokio::task::spawn_blocking(move || {
        dans_le_gisement(&gisement, &anime, &nom, |c, table| {
            let ligne = lire_ligne(c, table, &id)?;
            Ok(Json(LigneUnique {
                gisement: table.gisement,
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

    /// Retrouve une table du catalogue de test.
    fn table_de(g: &crate::dataset::Gisement, nom: &str) -> TableServie {
        g.lire(catalogue_compte)
            .unwrap()
            .into_iter()
            .map(|t| t.table)
            .find(|t| t.nom == nom)
            .unwrap()
    }

    /// Compte les lignes que rend une demande, en passant par la clause reellement construite.
    fn compter(g: &crate::dataset::Gisement, table: &TableServie, d: &Demande) -> usize {
        let cl = clause(table, d);
        let sql = format!("SELECT COUNT(*) FROM \"{}\"{}", table.nom, cl.sql);
        g.lire(move |c| {
            let n: i64 = c.query_row(&sql, rusqlite::params_from_iter(cl.params.iter()), |r| {
                r.get(0)
            })?;
            Ok(usize::try_from(n).unwrap_or(0))
        })
        .unwrap()
    }

    /// Calcule les facettes d'une demande sur un gisement de test.
    fn faceter(g: &crate::dataset::Gisement, table: &TableServie, d: &Demande) -> Vec<Facet> {
        let table = table.clone();
        let d = d.clone();
        g.lire(move |c| facettes(c, &table, &d)).unwrap()
    }

    /// Retrouve une valeur de facette par son libellé, ou `None` si la facette ne la porte pas.
    fn compte_de(f: &Facet, valeur: &str) -> Option<i64> {
        f.values
            .iter()
            .find(|v| v.value.as_deref() == Some(valeur))
            .map(|v| v.count)
    }

    #[test]
    fn une_facette_compte_les_valeurs_sous_les_filtres_en_cours() {
        // Une facette qui ne verrait pas les filtres rendrait les mêmes comptes que la table
        // entière — des chiffres justes à côté d'un écran qui n'en montre pas autant. Les deux
        // moitiés sont nécessaires : sans le cas filtré, une implémentation qui ignore la
        // clause passerait aussi.
        let (_d, g) = base();
        let t = table_de(&g, "inagle_characters");

        let large = faceter(&g, &t, &analyser(&t, &q(&[("facets", "element")])).unwrap());
        assert_eq!(large.len(), 1);
        assert_eq!(large[0].column, "element");
        assert_eq!(large[0].distinct, 2);
        assert!(!large[0].truncated);
        assert_eq!(compte_de(&large[0], "Feu"), Some(2));
        assert_eq!(compte_de(&large[0], "Bois"), Some(1));

        // `q=Mark` ne retient que `c1`, donc la facette ne doit plus voir qu'un `Feu` — et plus
        // du tout de `Bois` : une valeur qui rendrait zéro ligne ne se propose pas.
        let etroit = faceter(
            &g,
            &t,
            &analyser(&t, &q(&[("facets", "element"), ("q", "Mark")])).unwrap(),
        );
        assert_eq!(compte_de(&etroit[0], "Feu"), Some(1));
        assert_eq!(compte_de(&etroit[0], "Bois"), None);
        assert_eq!(etroit[0].distinct, 1);
    }

    #[test]
    fn une_facette_ignore_le_filtre_de_sa_propre_colonne() {
        // LA propriété qui rend une facette multi-sélectionnable. Sous `?element=Feu`, une
        // facette calculée avec TOUS les filtres ne rendrait que `Feu` — et l'interface ne
        // pourrait plus proposer d'ajouter `Bois`, c'est-à-dire qu'un filtre choisi fermerait
        // la porte à tous les autres. C'est `clause_sauf` qui l'évite, et ce test rougit si
        // l'appel repasse par `clause`.
        let (_d, g) = base();
        let t = table_de(&g, "inagle_characters");
        let d = analyser(&t, &q(&[("facets", "element"), ("element", "Feu")])).unwrap();

        // La page, elle, EST filtrée : les deux comptes disent bien deux choses différentes.
        assert_eq!(compter(&g, &t, &d), 2);

        let f = faceter(&g, &t, &d);
        assert_eq!(compte_de(&f[0], "Feu"), Some(2));
        assert_eq!(
            compte_de(&f[0], "Bois"),
            Some(1),
            "la facette doit continuer d'offrir les autres valeurs de sa propre colonne"
        );

        // Mais un filtre sur une AUTRE colonne s'applique bien à elle : sinon les comptes ne
        // correspondraient plus à l'écran.
        let croise = analyser(
            &t,
            &q(&[("facets", "element"), ("element", "Feu"), ("zukan__max", "1")]),
        )
        .unwrap();
        let f = faceter(&g, &t, &croise);
        assert_eq!(compte_de(&f[0], "Feu"), Some(1));
        assert_eq!(compte_de(&f[0], "Bois"), None);
    }

    #[test]
    fn une_facette_sur_une_colonne_inconnue_est_refusee() {
        // Le piège n° 1 de ce dépôt : un paramètre accepté et jamais appliqué. Un client qui
        // demande une facette inexistante doit recevoir un 400, pas une réponse sans le champ
        // — il croirait que la colonne n'a aucune valeur.
        let (_d, g) = base();
        let t = table_de(&g, "inagle_characters");
        let e = analyser(&t, &q(&[("facets", "couleur_preferee")])).unwrap_err();
        assert!(
            matches!(&e, ErreurSite::Demande(m) if m.contains("couleur_preferee")),
            "attendu un 400 nommant la colonne, recu {e:?}"
        );

        // Et la liste est bornée : au-delà, c'est le schéma qui est demandé.
        let colonnes = vec!["id"; FACETS_MAX + 1].join(",");
        assert!(
            analyser(&t, &q(&[("facets", colonnes.as_str())])).is_ok(),
            "les doublons se dedupliquent avant d'etre comptes"
        );
    }

    #[test]
    fn une_facette_groupe_le_vide_avec_le_nul() {
        // Le miroir mélange les deux — `age_group` est vide, pas nul — et publier la nuance
        // publierait une propriété de l'importeur, pas une du jeu. Même choix que les tests de
        // présence, et il faut que la MÊME décision se lise dans `distinct` : un
        // `count(DISTINCT ...)` ignorerait les nuls et annoncerait une valeur de moins que
        // celles que la liste rend juste à côté.
        let dir = tempfile::tempdir().unwrap();
        let chemin = dir.path().join("mirror.sqlite");
        let c = rusqlite::Connection::open(&chemin).unwrap();
        c.execute_batch(
            "CREATE TABLE _meta(cle TEXT, valeur TEXT);
             INSERT INTO _meta VALUES ('source', 'pg:DATABASE_URL');
             CREATE TABLE inagle_creux(id TEXT, groupe TEXT);
             INSERT INTO inagle_creux VALUES ('a', 'plein');
             INSERT INTO inagle_creux VALUES ('b', '');
             INSERT INTO inagle_creux VALUES ('c', NULL);",
        )
        .unwrap();
        drop(c);
        let g = crate::dataset::Gisement::nouveau(&chemin);
        let t = table_de(&g, "inagle_creux");

        let f = faceter(&g, &t, &analyser(&t, &q(&[("facets", "groupe")])).unwrap());
        assert_eq!(f[0].distinct, 2, "`plein` et le creux, pas trois groupes");
        assert_eq!(f[0].values.len(), 2, "`distinct` et la liste comptent pareil");
        assert_eq!(compte_de(&f[0], "plein"), Some(1));
        let creux = f[0]
            .values
            .iter()
            .find(|v| v.value.is_none())
            .expect("le creux est une valeur, rendue `null`");
        assert_eq!(creux.count, 2, "la chaine vide et le NULL comptent ensemble");
    }

    #[test]
    fn une_borne_retient_un_intervalle_et_ses_deux_bords() {
        // La moitie positive seule ne prouverait rien : une clause qui ne filtrerait pas
        // rendrait 3 aux quatre appels. Les bords sont testes parce que `>=`/`<=` sont un
        // choix — un intervalle exclusif serait une autre reponse, et il faut qu'elle rougisse.
        let (_d, g) = base();
        let t = table_de(&g, "inagle_characters");
        let cas = [
            (vec![("zukan__min", "2")], 2),
            (vec![("zukan__max", "2")], 2),
            (vec![("zukan__min", "2"), ("zukan__max", "2")], 1),
            (vec![("zukan__min", "9")], 0),
        ];
        for (params, attendu) in cas {
            let d = analyser(&t, &q(&params)).unwrap();
            assert_eq!(compter(&g, &t, &d), attendu, "pour {params:?}");
        }
    }

    #[test]
    fn une_borne_sur_une_colonne_texte_est_refusee_pas_approximee() {
        // C'est le coeur de la regle : SQLite comparerait volontiers `'Mark' >= '400'` et
        // rendrait une page plausible. Un 400 dit ce qui ne peut pas etre demande.
        let (_d, g) = base();
        let t = table_de(&g, "inagle_characters");
        let e = analyser(&t, &q(&[("name_fr__min", "400")])).unwrap_err();
        assert!(matches!(e, ErreurSite::Demande(_)));
        // Et la meme borne sur la colonne numerique passe : sans cette moitie, un refus
        // universel passerait aussi le test.
        assert!(analyser(&t, &q(&[("zukan__min", "400")])).is_ok());
    }

    #[test]
    fn une_borne_non_numerique_et_une_colonne_inconnue_sont_deux_400_distincts() {
        let (_d, g) = base();
        let t = table_de(&g, "inagle_characters");
        for cle in ["zukan__min", "zukan__max"] {
            assert!(analyser(&t, &q(&[(cle, "beaucoup")])).is_err());
        }
        assert!(analyser(&t, &q(&[("absente__min", "1")])).is_err());
    }

    #[test]
    fn la_presence_separe_le_renseigne_du_vide_et_du_nul() {
        // Le miroir melange NULL et chaine vide ; le test le reproduit exactement, sinon il
        // testerait une base plus propre que la vraie.
        let (dir, g) = base();
        // L'ecriture passe par une connexion a part : le gisement est ouvert en LECTURE SEULE,
        // et un `execute_batch` a travers lui echouerait — silencieusement si on l'ignorait.
        let ecriture = Connection::open(dir.path().join("mirror.sqlite")).unwrap();
        ecriture
            .execute_batch(
                "INSERT INTO inagle_characters VALUES ('c4', NULL, 'Feu', 4);
                 INSERT INTO inagle_characters VALUES ('c5', '', 'Feu', 5);",
            )
            .unwrap();
        drop(ecriture);
        let t = table_de(&g, "inagle_characters");
        let present = analyser(&t, &q(&[("name_fr", JETON_PRESENT)])).unwrap();
        let absent = analyser(&t, &q(&[("name_fr", JETON_ABSENT)])).unwrap();
        let total = analyser(&t, &q(&[])).unwrap();
        let (p, a, n) = (
            compter(&g, &t, &present),
            compter(&g, &t, &absent),
            compter(&g, &t, &total),
        );
        assert_eq!(p + a, n, "les deux moities doivent partitionner la table");
        assert!(p > 0 && p < n, "un filtre qui ne retient ni tout ni rien");
    }

    #[test]
    fn les_trois_formes_sont_republiees_telles_qu_appliquees() {
        // La lecon du lot 8 : appliquer sans avouer est le meme aveuglement que ne pas
        // appliquer, vu du client.
        let (_d, g) = base();
        let t = table_de(&g, "inagle_characters");
        let d = analyser(
            &t,
            &q(&[
                ("element", "Feu"),
                ("zukan__min", "2"),
                ("name_fr", JETON_PRESENT),
            ]),
        )
        .unwrap();
        let f = d.appliques();
        assert_eq!(f.egalites.get("element").map(String::as_str), Some("Feu"));
        assert_eq!(f.bornes.get("zukan__min"), Some(&2.0));
        assert_eq!(f.presences.get("name_fr"), Some(&"present"));
    }

    /// Un second gisement, avec les tables de la série — noms disjoints de `inagle_*`.
    fn base_anime() -> (tempfile::TempDir, crate::dataset::Gisement) {
        let dir = tempfile::tempdir().unwrap();
        let chemin = dir.path().join("episodes.db");
        let c = Connection::open(&chemin).unwrap();
        c.execute_batch(
            "CREATE TABLE episodes(id INTEGER PRIMARY KEY, season INTEGER, title TEXT);
             INSERT INTO episodes VALUES (1, 1, 'Le premier match');
             INSERT INTO episodes VALUES (2, 3, 'La revanche');
             CREATE TABLE seasons(id INTEGER PRIMARY KEY, nom TEXT);
             INSERT INTO seasons VALUES (1, 'Saison 1');",
        )
        .unwrap();
        drop(c);
        let g = crate::dataset::Gisement::nouveau(&chemin);
        (dir, g)
    }

    /// Un gisement qui ne pointe sur rien — l'état d'un miroir qui n'a pas encore tourné.
    fn base_absente() -> crate::dataset::Gisement {
        crate::dataset::Gisement::nouveau("/inexistant/aucun-gisement.sqlite")
    }

    #[test]
    fn une_cellule_csv_est_echappee_selon_rfc_4180() {
        // Moitie positive ET negative : sans la seconde, un echappement universel passerait,
        // et le fichier serait plein de guillemets inutiles.
        assert_eq!(echapper_csv("Mark"), "Mark");
        assert_eq!(echapper_csv("Feu, Vent"), "\"Feu, Vent\"");
        assert_eq!(echapper_csv("il dit \"non\""), "\"il dit \"\"non\"\"\"");
        assert_eq!(echapper_csv("deux\nlignes"), "\"deux\nlignes\"");
    }

    #[test]
    fn les_colonnes_csv_sont_l_union_pas_celles_de_la_premiere_ligne() {
        // Une ligne du miroir peut omettre une colonne nulle. Prendre les cles de la premiere
        // ligne perdrait la colonne suivante en silence — et un CSV ampute ne se voit pas.
        let mut a = MapJson::new();
        a.insert("id".into(), ValeurJson::String("c1".into()));
        let mut b = MapJson::new();
        b.insert("id".into(), ValeurJson::String("c2".into()));
        b.insert("element".into(), ValeurJson::String("Feu".into()));
        let page = PageLignes {
            page: Page::nouvelle(vec![a, b], Pagination::borner(None, None), 2),
            gisement: GISEMENT_EXTRAIT,
            table: "inagle_characters".to_owned(),
            cle: "id".to_owned(),
            filtres: FiltresAppliques {
                q: None,
                tri: "id".to_owned(),
                ordre: "asc",
                egalites: BTreeMap::new(),
                bornes: BTreeMap::new(),
                presences: BTreeMap::new(),
            },
            facets: Vec::new(),
        };
        let corps = reponse_csv(&page);
        assert_eq!(corps.status(), axum::http::StatusCode::OK);
        let nom = corps
            .headers()
            .get(axum::http::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        assert!(
            nom.contains("inagle_characters-page1.csv"),
            "le nom porte la table ET la page, sinon deux exports se recouvrent : {nom}"
        );
    }

    #[test]
    fn un_format_inconnu_est_refuse_pas_rendu_en_json() {
        // Rendre du JSON « par defaut » ferait telecharger un fichier au mauvais format sans
        // un mot. La liste servie tient en deux entrees, et elle est close.
        assert_eq!(FORMATS, ["json", "csv"]);
        assert!(PARAMS_RESERVES.contains(&"format"));
    }

    #[test]
    fn une_table_se_lit_dans_le_gisement_qui_la_porte() {
        let (_d, miroir) = base();
        let (_d2, anime) = base_anime();
        // La moitie positive de chaque cote : chacun trouve SA table.
        let a = dans_le_gisement(&miroir, &anime, "inagle_characters", |_, t| {
            Ok(t.gisement)
        })
        .unwrap();
        let b = dans_le_gisement(&miroir, &anime, "episodes", |_, t| Ok(t.gisement)).unwrap();
        assert_eq!(a, GISEMENT_EXTRAIT);
        assert_eq!(b, GISEMENT_ANIME);
        // Et la negative : un nom qu'aucun des deux ne porte est un 404, pas un 503.
        let e = dans_le_gisement(&miroir, &anime, "inagle_absente", |_, _| Ok(())).unwrap_err();
        assert!(matches!(e, ErreurSite::Introuvable(_)));
    }

    #[test]
    fn un_gisement_absent_degrade_au_lieu_d_eteindre() {
        // La regle du demarrage, tenue ici : le miroir manque, la serie repond quand meme.
        let (_d2, anime) = base_anime();
        let v = dans_le_gisement(&base_absente(), &anime, "episodes", |_, t| Ok(t.nom.clone()));
        assert_eq!(v.unwrap(), "episodes");
    }

    #[test]
    fn aucun_gisement_monte_est_un_503_pas_un_404() {
        // Le distinguo compte : un 404 ferait corriger son URL a un client qui devrait
        // simplement attendre que le miroir tourne.
        let e = dans_le_gisement(&base_absente(), &base_absente(), "episodes", |_, _| Ok(()))
            .unwrap_err();
        assert!(
            matches!(e, ErreurSite::Indisponible(_)),
            "sans gisement monte, la route est indisponible, pas introuvable"
        );
    }

    #[test]
    fn une_erreur_de_la_table_trouvee_n_est_pas_un_404() {
        // Le defaut mesure le 2026-09-06 : `tri=` sur une colonne inconnue d'une table du
        // SECOND gisement ressortait en 404 « aucune table ne se nomme `episodes` », parce que
        // la boucle continuait apres l'echec de `f`. Un message qui envoie corriger un nom de
        // table juste est pire qu'une erreur brute.
        let (_d, miroir) = base();
        let (_d2, anime) = base_anime();
        let e = dans_le_gisement(&miroir, &anime, "episodes", |_, t| {
            analyser(t, &q(&[("tri", "pertinence")])).map(|_| ())
        })
        .unwrap_err();
        assert!(
            matches!(e, ErreurSite::Demande(_)),
            "un tri sur une colonne inconnue est un 400, pas un 404 : {e:?}"
        );
    }

    #[test]
    fn les_filtres_generiques_marchent_aussi_sur_la_serie() {
        // Le gain reel de ce lot : les quatre filtres des episodes (#37-40 de docs/FILTRES.md)
        // ne sont pas quatre lignes de code, ce sont ZERO — ils viennent avec la route.
        let (_d, anime) = base_anime();
        let n = dans_le_gisement(&base_absente(), &anime, "episodes", |c, t| {
            let d = analyser(t, &q(&[("season", "3")]))?;
            let cl = clause(t, &d);
            let sql = format!("SELECT COUNT(*) FROM \"{}\"{}", t.nom, cl.sql);
            let n: i64 = c.query_row(&sql, rusqlite::params_from_iter(cl.params.iter()), |r| {
                r.get(0)
            })?;
            Ok(n)
        })
        .unwrap();
        assert_eq!(n, 1, "une seule saison 3 sur les deux episodes");
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
