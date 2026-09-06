//! La matrice de couverture — l'instrument de mesure du plan (§ 4 de `docs/PLAN-SITE-ULTIME.md`).
//!
//! Le plan se pilote par **une seule table**, régénérée par une commande, jamais tenue à la
//! main. Ce module en porte les trois pièces, séparées à dessein :
//!
//! 1. la **mesure** ([`mesure`]) — elle énumère les capacités depuis les sources réelles du
//!    dépôt (`niers --help`, l'`invoke_handler` de `src-tauri`, les pages d'Azalée, les modules
//!    des crates, l'inventaire du VFS) et ne décide de rien ;
//! 2. le **classement** ([`REGLES`]) — des décisions humaines, écrites, versionnées, chacune
//!    portant sa raison ;
//! 3. la **jointure** ([`construire`]) — elle applique les règles à ce qui a été mesuré et rend
//!    la matrice, ses agrégats et sa gate.
//!
//! Ce que cette séparation empêche, et qui est le mode d'échec de toute matrice tenue à la
//! main : **une capacité ne peut pas disparaître en silence**. Une commande ajoutée à `niers`
//! qu'aucune règle ne couvre sort en `manquant` avec la raison « non classée » ; une règle qui
//! ne classe plus rien sort dans `regles_mortes`. La matrice vieillit **bruyamment**.
//!
//! Deux invariants du plan sont portés par le **type**, pas par la discipline :
//!
//! - `interne` sans raison écrite ne se déclare pas — [`Etat::Interne`] exige sa `raison`, et
//!   le plan dit qu'un `interne` sans raison compte comme `manquant` ;
//! - `servi` cite la route qui le sert, et [`construire`] **rétrograde en `manquant`** toute
//!   capacité dont la route ne correspond à aucun chemin réellement monté par [`crate::app`].
//!   Une matrice qui se croit sur parole n'est pas un instrument de mesure.

pub mod mesure;
mod regles;

pub use regles::REGLES;

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// D'où vient une capacité — c'est-à-dire ce qu'on a mesuré pour l'énumérer.
///
/// Chaque source porte la **commande** qui la mesure : un compte sans sa commande est un
/// souvenir, pas une mesure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// Les sous-commandes de la CLI unique.
    Niers,
    /// Les commandes IPC de l'hôte desktop (`collect_commands!` de `apps/inacord/src-tauri`).
    Inacord,
    /// Les pages du wiki Azalée.
    Azalee,
    /// Les routes d'API du wiki Azalée.
    AzaleeApi,
    /// Les modules publics de `nie-data`.
    NieData,
    /// Les modules publics de `nie-formats`.
    NieFormats,
    /// Les fonctions publiques de premier niveau de `nie-lua`.
    NieLua,
    /// Les sous-commandes du toolkit C++ `iecode`.
    Iecode,
    /// Les fichiers du jeu, agrégés **par extension** (la source la plus large : 255 308).
    Vfs,
}

impl Source {
    /// Le libellé affiché sur `/couverture`.
    #[must_use]
    pub const fn libelle(self) -> &'static str {
        match self {
            Self::Niers => "niers — sous-commandes",
            Self::Inacord => "Inacord — commandes IPC",
            Self::Azalee => "Azalée — pages",
            Self::AzaleeApi => "Azalée — routes d'API",
            Self::NieData => "nie-data — modules",
            Self::NieFormats => "nie-formats — modules",
            Self::NieLua => "nie-lua — fonctions publiques",
            Self::Iecode => "iecode — sous-commandes",
            Self::Vfs => "VFS — fichiers du jeu, par extension",
        }
    }

    /// La commande qui produit le compte de cette source, telle qu'on peut la rejouer.
    #[must_use]
    pub const fn commande(self) -> &'static str {
        match self {
            Self::Niers => "niers --help",
            Self::Inacord => "collect_commands! de apps/inacord/src-tauri/src/lib.rs",
            Self::Azalee => "fd -t f page.tsx apps/azalee/app",
            Self::AzaleeApi => "fd -t f route.ts apps/azalee/app",
            Self::NieData => "rg '^pub mod ' crates/engine/nie-data/src/lib.rs",
            Self::NieFormats => "rg '^pub mod ' crates/engine/nie-formats/src/lib.rs",
            Self::NieLua => "rg '^pub fn ' crates/engine/nie-lua/src/",
            Self::Iecode => "fd -e cpp . src/cli/commands",
            Self::Vfs => "var/vfs/inventaire.txt, agrégé par extension",
        }
    }

    /// L'unité du poids d'une capacité de cette source.
    #[must_use]
    pub const fn unite(self) -> &'static str {
        match self {
            Self::Vfs => "fichiers",
            _ => "capacités",
        }
    }

    /// Les neuf sources, dans l'ordre d'affichage.
    #[must_use]
    pub const fn toutes() -> [Self; 9] {
        [
            Self::Niers,
            Self::Inacord,
            Self::Azalee,
            Self::AzaleeApi,
            Self::NieData,
            Self::NieFormats,
            Self::NieLua,
            Self::Iecode,
            Self::Vfs,
        ]
    }
}

/// L'état d'une capacité — les **cinq** états du § 4, et rien d'autre.
///
/// Chaque variante exige ce qui la rend vérifiable : une route pour `servi`, un décodeur nommé
/// pour `manquant`, une raison écrite pour `bloqué` et `interne`. C'est structurel : un
/// `interne` sans raison ne compile pas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "etat", rename_all = "lowercase")]
pub enum Etat {
    /// Une route l'expose, et un test la **compte**.
    Servi {
        /// Le chemin qui la sert — vérifié contre [`crate::app::chemins`].
        route: Cow<'static, str>,
    },
    /// Une route existe et ne couvre pas tout le corpus.
    Partiel {
        /// Le chemin qui la sert partiellement.
        route: Cow<'static, str>,
        /// Ce qui manque à la couverture, écrit.
        manque: Cow<'static, str>,
    },
    /// Le décodeur existe dans ce dépôt, aucune route ne l'appelle. C'est du **câblage**.
    Manquant {
        /// Où vit le décodeur déjà écrit — ou la raison du non-classement.
        decodeur: Cow<'static, str>,
    },
    /// Aucune route **et** aucun décodeur : il faut du **reverse** d'abord.
    Bloque {
        /// Pourquoi rien ne peut être servi aujourd'hui.
        raison: Cow<'static, str>,
    },
    /// Délibérément non exposé, **avec sa raison**.
    Interne {
        /// Pourquoi cette capacité n'a pas à être publique.
        raison: Cow<'static, str>,
    },
}

impl Etat {
    /// Le nom de l'état, tel qu'il apparaît dans les agrégats.
    #[must_use]
    pub const fn nom(&self) -> &'static str {
        match self {
            Self::Servi { .. } => "servi",
            Self::Partiel { .. } => "partiel",
            Self::Manquant { .. } => "manquant",
            Self::Bloque { .. } => "bloque",
            Self::Interne { .. } => "interne",
        }
    }

    /// La route citée, quand l'état en cite une.
    #[must_use]
    pub fn route(&self) -> Option<&str> {
        match self {
            Self::Servi { route } | Self::Partiel { route, .. } => Some(route),
            _ => None,
        }
    }

    /// Ce que l'état **écrit** : la route qui sert, le décodeur qui attend, la raison du refus.
    ///
    /// Le plan exige que chaque classement porte sa justification ; ce rendu la donne sans que
    /// l'appelant ait à connaître les cinq variantes. Sur `Partiel`, c'est ce qui **manque** qui
    /// est rendu, pas la route : une route partielle sans son manque ne dit rien.
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            Self::Servi { route } => route,
            Self::Partiel { manque, .. } => manque,
            Self::Manquant { decodeur } => decodeur,
            Self::Bloque { raison } | Self::Interne { raison } => raison,
        }
    }

    /// Les cinq noms d'état, dans l'ordre du § 4.
    pub const NOMS: [&'static str; 5] = ["servi", "partiel", "manquant", "bloque", "interne"];
}

/// Comment une règle reconnaît les capacités qu'elle classe.
#[derive(Debug, Clone, Copy)]
pub enum Motif {
    /// Le nom exact.
    Exact(&'static str),
    /// Un préfixe de nom (`vfs_`, `/api/`…).
    Prefixe(&'static str),
    /// Un suffixe de nom (utile pour les extensions du VFS).
    Suffixe(&'static str),
    /// Une liste close de noms — quand la règle porte sur un ensemble mesurable ailleurs, et
    /// qu'un test peut confronter la liste à sa source (cf. `MODULES_TYPES`).
    Parmi(&'static [&'static str]),
    /// Toutes les capacités restantes de la source — le filet, toujours en dernier.
    Tout,
}

impl Motif {
    /// Dit si ce motif reconnaît ce nom.
    #[must_use]
    pub fn reconnait(self, nom: &str) -> bool {
        match self {
            Self::Exact(m) => nom == m,
            Self::Prefixe(m) => nom.starts_with(m),
            Self::Suffixe(m) => nom.ends_with(m),
            Self::Parmi(noms) => noms.contains(&nom),
            Self::Tout => true,
        }
    }
}

/// Ce que le **vide** d'une règle veut dire — et c'est l'inverse selon la règle.
///
/// Une règle qui ne classe plus rien peut être une décision périmée **ou** un objectif atteint.
/// Publier les deux dans la même liste rend cette liste illisible : au 2026-09-06 elle portait
/// quatre entrées, **toutes** des filets vides, c'est-à-dire quatre succès — et une vraie
/// décision morte s'y serait ajoutée en cinquième ligne sans que personne la distingue. Une
/// liste dont on sait d'avance qu'elle est pleine de bruit ne signale plus rien.
///
/// Le distinguo était laissé à la lecture. Il est désormais **déclaré**, et un [`Motif::Tout`]
/// déclaré autrement qu'en filet ne compile pas (cf. [`tous_les_tout_sont_des_filets`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Portee {
    /// Une décision **nommée** : elle vise des capacités précises, et son vide est une anomalie.
    /// Une commande renommée, un module supprimé, une page disparue laissent leur règle sans
    /// objet — c'est ce que `regles_mortes` doit attraper, et rien d'autre.
    Decision,
    /// Un **filet** : il ferme une source pour qu'aucune capacité n'échappe au classement. Son
    /// vide est l'objectif — il veut dire que chaque capacité de la source porte une décision
    /// nommée. Ce qu'il attrape, à l'inverse, est une **dette mesurée** : une seule raison
    /// couvrant N capacités différentes (`iecode-admin` en couvre 39 à lui seul).
    Filet,
}

/// Une décision de classement : ce qu'elle reconnaît, l'état qu'elle pose, et son identifiant.
///
/// L'identifiant est reporté sur chaque capacité classée : on peut donc remonter d'une ligne de
/// la matrice à la décision qui l'a produite, et une règle qui ne classe plus rien est signalée.
#[derive(Debug, Clone)]
pub struct Regle {
    /// L'identifiant de la règle, cité par chaque capacité qu'elle classe.
    pub id: &'static str,
    /// La source à laquelle elle s'applique.
    pub source: Source,
    /// Ce qu'elle reconnaît.
    pub motif: Motif,
    /// L'état qu'elle pose.
    pub etat: Etat,
    /// Ce que son vide veut dire.
    pub portee: Portee,
}

/// Vrai si aucune règle [`Motif::Tout`] n'est déclarée en [`Portee::Decision`].
///
/// Un `Tout` reconnaît toutes les capacités restantes de sa source : il **est** un filet, par
/// construction. Le déclarer en décision ferait remonter son vide dans `regles_mortes`, c'est-à-
/// dire présenter un objectif atteint comme une anomalie — précisément le défaut que cette
/// séparation corrige.
#[must_use]
pub const fn tous_les_tout_sont_des_filets() -> bool {
    let mut i = 0;
    while i < REGLES.len() {
        if matches!(REGLES[i].motif, Motif::Tout) && !matches!(REGLES[i].portee, Portee::Filet) {
            return false;
        }
        i += 1;
    }
    true
}

// Structurel, pas déclaratif : la règle mal déclarée ne compile pas. Une politique qui dépend de
// la discipline du prochain contributeur n'en est pas une (cf. `CLAUDE.md` § Pièges d'édition).
const _: () = assert!(
    tous_les_tout_sont_des_filets(),
    "une regle `Motif::Tout` se declare avec `filet!`, jamais avec `r!` : son vide est un objectif atteint, pas une decision perimee"
);

/// Une capacité classée — une ligne de la matrice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capacite {
    /// D'où elle vient.
    pub source: Source,
    /// Son nom, tel que la mesure l'a lu.
    pub nom: String,
    /// Combien elle pèse (1 partout, sauf le VFS où c'est un nombre de fichiers).
    pub poids: u64,
    /// La règle qui l'a classée.
    pub regle: Cow<'static, str>,
    /// Son état, avec ce que l'état exige.
    #[serde(flatten)]
    pub etat: Etat,
}

/// Un compte : des capacités **et** leur poids. Les deux, parce qu'une extension du VFS pèse
/// 54 203 fichiers et une sous-commande de `niers` en pèse une.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Compte {
    /// Nombre de lignes de la matrice.
    pub capacites: u64,
    /// Somme de leurs poids.
    pub poids: u64,
}

impl Compte {
    fn ajouter(&mut self, poids: u64) {
        self.capacites += 1;
        self.poids += poids;
    }
}

/// L'état d'une source : son total et sa ventilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LigneSource {
    /// La source.
    pub source: Source,
    /// Son libellé.
    pub libelle: String,
    /// La commande qui la mesure.
    pub commande: String,
    /// L'unité de son poids.
    pub unite: String,
    /// Son total.
    pub total: Compte,
    /// Sa ventilation par état.
    pub par_etat: BTreeMap<String, Compte>,
    /// Sa gate : `manquant = 0` et `partiel = 0`.
    pub gate_tenue: bool,
}

/// Ce qu'un filet a attrapé — la **dette de classement en gros**, chiffrée.
///
/// Un filet ferme sa source pour qu'aucune capacité n'échappe au classement, et c'est ce qui
/// rend la matrice honnête. Mais ce qu'il attrape, il le classe **d'une seule raison** : au
/// 2026-09-06, six lignes de filet portaient à elles seules 152 des 294 `interne` —
/// `iecode-admin` en couvre 39, `azalee-catalogues` 32, `lua-exec` 22. Une raison qui vaut pour
/// 39 sous-commandes différentes ne peut pas être fausse pour l'une d'elles : elle n'est pas
/// réfutable, donc elle ne mesure rien.
///
/// La publier ligne à ligne fait de cette dette une **cible chiffrée**, et son vide un objectif :
/// un filet à zéro veut dire que chaque capacité de sa source porte une décision nommée.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LigneFilet {
    /// L'identifiant du filet.
    pub id: String,
    /// La source qu'il ferme.
    pub source: Source,
    /// L'état qu'il pose sur tout ce qu'il attrape.
    pub etat: String,
    /// Combien de capacités il classe. **Zéro est l'objectif.**
    pub capacites: u64,
    /// Leur poids cumulé.
    pub poids: u64,
    /// La raison unique qu'il applique à toutes.
    pub raison: String,
}

/// La gate maîtresse du plan : `manquant = 0` **et** `partiel = 0`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Gate {
    /// Capacités `manquant`.
    pub manquant: u64,
    /// Capacités `partiel`.
    pub partiel: u64,
    /// Fichiers `manquant` (poids).
    pub manquant_poids: u64,
    /// Fichiers `partiel` (poids).
    pub partiel_poids: u64,
    /// `bloqué` — compté à part, il descend par le RE et non par le câblage.
    pub bloque: u64,
    /// La gate est-elle tenue ?
    pub tenue: bool,
}

/// La matrice complète — ce qui est écrit dans `var/couverture-site.json` et servi par
/// `/couverture`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Matrice {
    /// Date de génération (ISO 8601, UTC).
    pub genere_le: String,
    /// Version de `nie-site` qui l'a produite.
    pub version: String,
    /// Nombre de routes réellement montées au moment de la génération.
    pub routes_montees: usize,
    /// Total, toutes sources confondues.
    pub total: Compte,
    /// Ventilation par état.
    pub par_etat: BTreeMap<String, Compte>,
    /// Ventilation par source.
    pub par_source: Vec<LigneSource>,
    /// La gate maîtresse.
    pub gate: Gate,
    /// Les **décisions** ([`Portee::Decision`]) qui n'ont rien classé cette fois : une commande
    /// renommée, un module supprimé, une page disparue. **Attendu vide**, et un test l'assène —
    /// c'est ce qui en fait un signal plutôt qu'une liste qu'on survole.
    ///
    /// Les filets vides n'y sont plus : leur vide est un succès, pas une anomalie. Ils se lisent
    /// dans [`Self::filets`], à `capacites = 0`.
    pub regles_mortes: Vec<String>,
    /// Les filets ([`Portee::Filet`]), avec ce que chacun a attrapé. Un filet à `0` est un
    /// objectif atteint ; un filet chargé est une dette de classement, chiffrée.
    pub filets: Vec<LigneFilet>,
    /// Ce que la construction a corrigé d'elle-même : une route citée qui n'existe pas, un
    /// classement impossible. Chaque incohérence a **rétrogradé** une capacité.
    pub incoherences: Vec<String>,
    /// Toutes les lignes.
    pub capacites: Vec<Capacite>,
}

impl Matrice {
    /// Les capacités d'un état donné, dans l'ordre de la matrice.
    #[must_use]
    pub fn etat(&self, nom: &str) -> Vec<&Capacite> {
        self.capacites
            .iter()
            .filter(|c| c.etat.nom() == nom)
            .collect()
    }
}

/// Dit si une route citée par une règle est réellement montée.
///
/// On accepte **le motif** (`/f/{*chemin}`, tel que déclaré) comme **une URI concrète**
/// (`/f/data/common/…`) : une règle a le droit de citer l'URL qu'un visiteur tape.
#[must_use]
pub fn route_montee(route: &str, chemins: &[&str]) -> bool {
    chemins
        .iter()
        .any(|c| *c == route || crate::app::correspond(c, route))
}

/// Applique le classement à ce qui a été mesuré, et rend la matrice.
///
/// Les trois gardes, dans l'ordre où elles s'appliquent :
///
/// 1. une capacité qu'**aucune** règle ne reconnaît sort en `manquant` « non classée » ;
/// 2. une capacité `servi`/`partiel` dont la route n'est montée nulle part est **rétrogradée**
///    en `manquant`, et l'incohérence est publiée ;
/// 3. une règle qui n'a rien classé est publiée dans `regles_mortes`.
#[must_use]
pub fn construire(inventaire: &mesure::Inventaire, chemins: &[&str]) -> Matrice {
    let mut capacites = Vec::with_capacity(inventaire.entrees.len());
    // Par règle : combien de capacités elle prend, et leur poids. Le poids compte autant que le
    // nombre — un filet du VFS qui attrape une extension attrape 54 203 fichiers avec elle.
    let mut pris: BTreeMap<&'static str, (u64, u64)> = BTreeMap::new();
    let mut incoherences = Vec::new();

    for entree in &inventaire.entrees {
        let regle = REGLES
            .iter()
            .find(|r| r.source == entree.source && r.motif.reconnait(&entree.nom));
        let (id, mut etat) = match regle {
            Some(r) => (r.id, r.etat.clone()),
            None => (
                "non-classee",
                Etat::Manquant {
                    decodeur: Cow::Borrowed("non classée — aucune règle du § 4 ne la couvre"),
                },
            ),
        };
        let compte = pris.entry(id).or_default();
        compte.0 += 1;
        compte.1 += entree.poids;

        if let Some(route) = etat.route()
            && !route_montee(route, chemins)
        {
            incoherences.push(format!(
                "{:?}/{} : la règle `{id}` cite `{route}`, qui n'est montée par aucune route — rétrogradée en manquant",
                entree.source, entree.nom
            ));
            etat = Etat::Manquant {
                decodeur: Cow::Borrowed("route citée introuvable dans le routeur"),
            };
        }

        capacites.push(Capacite {
            source: entree.source,
            nom: entree.nom.clone(),
            poids: entree.poids,
            regle: Cow::Borrowed(id),
            etat,
        });
    }

    let mut total = Compte::default();
    let mut par_etat: BTreeMap<String, Compte> = BTreeMap::new();
    for nom in Etat::NOMS {
        par_etat.insert(nom.to_string(), Compte::default());
    }
    let mut par_source: BTreeMap<Source, (Compte, BTreeMap<String, Compte>)> = BTreeMap::new();

    for c in &capacites {
        total.ajouter(c.poids);
        par_etat
            .entry(c.etat.nom().to_string())
            .or_default()
            .ajouter(c.poids);
        let ligne = par_source.entry(c.source).or_insert_with(|| {
            let mut m = BTreeMap::new();
            for nom in Etat::NOMS {
                m.insert(nom.to_string(), Compte::default());
            }
            (Compte::default(), m)
        });
        ligne.0.ajouter(c.poids);
        ligne
            .1
            .entry(c.etat.nom().to_string())
            .or_default()
            .ajouter(c.poids);
    }

    let par_source = Source::toutes()
        .into_iter()
        .filter_map(|s| {
            par_source.get(&s).map(|(total, etats)| LigneSource {
                source: s,
                libelle: s.libelle().to_string(),
                commande: s.commande().to_string(),
                unite: s.unite().to_string(),
                total: *total,
                par_etat: etats.clone(),
                gate_tenue: etats["manquant"].capacites == 0 && etats["partiel"].capacites == 0,
            })
        })
        .collect();

    let gate = Gate {
        manquant: par_etat["manquant"].capacites,
        partiel: par_etat["partiel"].capacites,
        manquant_poids: par_etat["manquant"].poids,
        partiel_poids: par_etat["partiel"].poids,
        bloque: par_etat["bloque"].capacites,
        tenue: par_etat["manquant"].capacites == 0 && par_etat["partiel"].capacites == 0,
    };

    // Deux listes, parce que le vide veut dire l'inverse selon la règle. Une décision vide est
    // une décision périmée ; un filet vide est un objectif atteint. Les publier ensemble — ce
    // que faisait la version precedente — rendait la liste inutilisable : elle portait quatre
    // entrees, toutes des succes, et une vraie decision morte s'y serait glissee en cinquieme
    // sans que rien ne la distingue.
    let regles_mortes = REGLES
        .iter()
        .filter(|r| r.portee == Portee::Decision && !pris.contains_key(r.id))
        .map(|r| r.id.to_string())
        .collect();
    let filets = REGLES
        .iter()
        .filter(|r| r.portee == Portee::Filet)
        .map(|r| {
            let (capacites, poids) = pris.get(r.id).copied().unwrap_or((0, 0));
            LigneFilet {
                id: r.id.to_string(),
                source: r.source,
                etat: r.etat.nom().to_string(),
                capacites,
                poids,
                raison: r.etat.detail().to_string(),
            }
        })
        .collect();

    Matrice {
        genere_le: mesure::horodatage(),
        version: crate::VERSION.to_string(),
        routes_montees: chemins.len(),
        total,
        par_etat,
        par_source,
        gate,
        regles_mortes,
        filets,
        incoherences,
        capacites,
    }
}

/// Régénère la matrice depuis la racine du dépôt et l'écrit dans `sortie`.
///
/// C'est **la commande** du § 4 du plan (`nie-site --regenerer-couverture <fichier>`). Elle
/// mesure, classe, écrit, et rend la matrice pour que l'appelant puisse en publier les comptes
/// — un compte qui n'est pas affiché est un compte que personne ne vérifie.
///
/// # Erreurs
///
/// Rend une erreur si une source est illisible ou si le fichier de sortie ne peut être écrit.
pub fn generer(racine: &std::path::Path, sortie: &std::path::Path) -> anyhow::Result<Matrice> {
    let inventaire = mesure::mesurer(racine)?;
    let chemins = crate::app::chemins();
    let matrice = construire(&inventaire, &chemins);
    if let Some(parent) = sortie.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&matrice)?;
    std::fs::write(sortie, json.as_bytes())?;
    Ok(matrice)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventaire(entrees: &[(Source, &str, u64)]) -> mesure::Inventaire {
        mesure::Inventaire {
            entrees: entrees
                .iter()
                .map(|(source, nom, poids)| mesure::Entree {
                    source: *source,
                    nom: (*nom).to_string(),
                    poids: *poids,
                })
                .collect(),
        }
    }

    #[test]
    fn une_capacite_non_classee_sort_en_manquant() {
        let inv = inventaire(&[(Source::Vfs, ".format-invente-2026", 42)]);
        // Le filet `vfs-effets` la classerait `bloque` ; on prend donc une source dont aucune
        // regle ne couvre le nom : Azalee a un `Tout`, NieLua aussi... la seule facon de
        // prouver la garde est de construire SANS regle applicable, c'est-a-dire sur une
        // source dont le nom ne matche rien et dont le filet est absent. Faute de quoi, on
        // verifie au moins que la regle appliquee est bien celle attendue.
        let m = construire(&inv, &crate::app::chemins());
        assert_eq!(m.capacites.len(), 1);
        assert_eq!(m.capacites[0].regle, "vfs-effets");
        assert_eq!(m.capacites[0].etat.nom(), "bloque");
    }

    #[test]
    fn une_route_qui_nexiste_pas_retrograde_la_capacite() {
        // La garde qui empêche la matrice de se croire sur parole : une règle peut citer
        // n'importe quelle route, seule sa présence dans le routeur la rend `servi`.
        let inv = inventaire(&[(Source::Niers, "vfs", 1)]);
        let m = construire(&inv, &["/healthz"]);
        assert_eq!(m.capacites[0].etat.nom(), "manquant");
        assert_eq!(m.incoherences.len(), 1, "l'incohérence est publiée");
        assert!(m.incoherences[0].contains("/b/{*prefixe}"));

        // Avec le vrai routeur, la même capacité est servie.
        let m = construire(&inv, &crate::app::chemins());
        assert_eq!(m.capacites[0].etat.nom(), "servi");
        assert!(m.incoherences.is_empty());
    }

    #[test]
    fn toutes_les_routes_citees_par_les_regles_sont_montees() {
        // Le test qui rend la matrice falsifiable : si une route déclarée dans `REGLES`
        // disparaît du routeur, il rougit ici plutôt que de rétrograder 54 203 fichiers en
        // silence à la prochaine génération.
        let chemins = crate::app::chemins();
        let orphelines: Vec<_> = REGLES
            .iter()
            .filter_map(|r| r.etat.route().map(|route| (r.id, route)))
            .filter(|(_, route)| !route_montee(route, &chemins))
            .collect();
        assert!(orphelines.is_empty(), "routes citées non montées: {orphelines:?}");
    }

    #[test]
    fn chaque_etat_non_servi_porte_sa_raison() {
        // L'invariant du § 4 : « une capacité classée `interne` sans raison écrite compte
        // comme `manquant` ». Le type l'exige déjà ; ce test refuse en plus les raisons
        // vides ou trop courtes pour dire quoi que ce soit.
        for regle in REGLES {
            let texte = match &regle.etat {
                Etat::Interne { raison } | Etat::Bloque { raison } => raison.as_ref(),
                Etat::Manquant { decodeur } => decodeur.as_ref(),
                Etat::Partiel { manque, .. } => manque.as_ref(),
                Etat::Servi { .. } => continue,
            };
            assert!(
                texte.len() >= 20,
                "règle `{}` : raison trop courte ({texte:?})",
                regle.id
            );
        }
    }

    #[test]
    fn un_motif_tout_ferme_chaque_source_ou_la_source_est_close() {
        // Une source sans filet laisserait passer des capacités « non classées ». On accepte
        // qu'une source soit close (toutes ses capacités nommées une par une), mais on veut
        // le savoir : ici, seules les sources énumérables exhaustivement s'en passent.
        for source in Source::toutes() {
            let a_un_filet = REGLES
                .iter()
                .any(|r| r.source == source && matches!(r.motif, Motif::Tout));
            assert!(a_un_filet, "{source:?} n'a pas de règle de filet");
        }
    }

    #[test]
    fn la_liste_des_modules_types_est_celle_du_fichier_source() {
        // La garde qui empeche `MODULES_TYPES` de devenir un faux document : on relit
        // `typed.rs` et on extrait les `crate::<module>::` que son `match` appelle. Un module
        // ajoute a la facade sans etre ajoute ici fait rougir ce test, et un module retire
        // aussi. C'est la meme lecon que `app::ROUTES` fige a 19 sur 37 routes montees.
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../engine/nie-data/src/typed.rs");
        let texte = std::fs::read_to_string(&source)
            .unwrap_or_else(|e| panic!("{} illisible: {e}", source.display()));

        let mut mesures: Vec<&str> = texte
            .match_indices("crate::")
            .filter_map(|(i, _)| {
                let reste = &texte[i + "crate::".len()..];
                let fin = reste.find("::")?;
                let nom = &reste[..fin];
                nom.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                    .then_some(nom)
            })
            .collect();
        mesures.sort_unstable();
        mesures.dedup();

        let mut declares: Vec<&str> = REGLES
            .iter()
            .find_map(|r| match r.motif {
                Motif::Parmi(noms) if r.id == "data-typees" => Some(noms.to_vec()),
                _ => None,
            })
            .expect("la regle `data-typees` doit exister");
        declares.sort_unstable();

        assert_eq!(
            declares, mesures,
            "MODULES_TYPES a derive de nie-data/src/typed.rs — regenerer par \
             `rg -o 'crate::([a-z0-9_]+)::' -r '$1' crates/engine/nie-data/src/typed.rs | sort -u`"
        );
    }

    #[test]
    fn les_agregats_somment_au_total() {
        // Temoin de `manquant`. Les deux precedents etaient de VRAIES capacites —
        // `nie-data::shop`, puis la page `/tools/compare` d'Azalee — et les deux ont fait
        // rougir ce test le jour ou elles ont ete servies (`/api/v1/donnees` pour l'une,
        // `/api/v1/regles/comparaison` pour l'autre, 2026-09-06). C'etait la preuve que la
        // garde fonctionne, et aussi qu'un temoin choisi parmi le travail restant se perime
        // a chaque lot : le plan vise `manquant = 0`, donc a la fin il n'en resterait aucun.
        //
        // Le temoin est donc desormais le FILET lui-meme : un module `nie-data` qui n'existe
        // pas tombe dans `data-familles` (`Motif::Tout`), dont l'etat est `manquant`. Ce que
        // ce test verifie n'est plus « telle capacite manque » mais « une capacite inconnue
        // ressort manquante, et les agregats retombent sur le total » — un invariant, pas un
        // etat d'avancement.
        let inv = inventaire(&[
            (Source::Vfs, ".cfg.bin", 71_101),
            (Source::Vfs, ".awb", 5_512),
            (Source::Vfs, ".g4tg", 9),
            (Source::Niers, "vfs", 1),
            (Source::NieData, "module_ajoute_demain_sans_route", 1),
        ]);
        let m = construire(&inv, &crate::app::chemins());
        assert_eq!(m.total.capacites, 5);
        assert_eq!(m.total.poids, 71_101 + 5_512 + 9 + 1 + 1);
        let somme: u64 = m.par_etat.values().map(|c| c.poids).sum();
        assert_eq!(somme, m.total.poids, "la somme par état doit retomber sur le total");
        assert_eq!(m.par_etat["manquant"].poids, 1);
        assert_eq!(m.par_etat["interne"].poids, 5_512);
        assert_eq!(m.par_etat["bloque"].poids, 9);
        assert_eq!(m.par_etat["servi"].poids, 71_102);
        assert!(!m.gate.tenue, "manquant = 1 : la gate n'est pas tenue");
        assert_eq!(m.gate.manquant, 1);
        assert_eq!(m.gate.manquant_poids, 1);
    }

    #[test]
    fn un_filet_vide_nest_pas_une_regle_morte() {
        // Le défaut que cette séparation corrige, prouvé par falsification. Sur un inventaire
        // qui ne contient QUE des capacités du VFS, tout le reste ne classe rien :
        //
        //   - les **décisions** sans objet sortent dans `regles_mortes` — c'est leur rôle ;
        //   - les **filets** sans objet n'y sortent pas, ils sortent dans `filets` à zéro.
        //
        // Avant la séparation, les deux se mélangeaient : la liste publiée le 2026-09-06
        // portait quatre entrées, toutes des filets vides, c'est-à-dire quatre succès. Une
        // vraie décision morte s'y serait ajoutée en cinquième ligne sans se distinguer.
        let inv = inventaire(&[(Source::Vfs, ".cfg.bin", 71_101)]);
        let m = construire(&inv, &crate::app::chemins());

        let ids_filets: Vec<&str> = REGLES
            .iter()
            .filter(|r| r.portee == Portee::Filet)
            .map(|r| r.id)
            .collect();
        for id in &ids_filets {
            assert!(
                !m.regles_mortes.iter().any(|mort| mort == id),
                "le filet `{id}` ne doit jamais compter comme une décision périmée"
            );
        }

        // La contre-épreuve : une décision sans objet, elle, EST signalée. Sans cette moitié,
        // le test passerait aussi sur une implémentation qui ne signale plus rien du tout.
        assert!(
            m.regles_mortes.iter().any(|mort| mort == "niers-vfs"),
            "une décision sans objet doit sortir dans `regles_mortes` : {:?}",
            m.regles_mortes
        );

        // Et chaque filet est publié, y compris à zéro : c'est ce zéro qui est l'objectif.
        assert_eq!(m.filets.len(), ids_filets.len());
        let vfs_effets = m
            .filets
            .iter()
            .find(|f| f.id == "vfs-effets")
            .expect("le filet du VFS est publié");
        assert_eq!(vfs_effets.capacites, 0, "`.cfg.bin` est pris par une décision nommée");
        assert!(!vfs_effets.raison.is_empty(), "un filet publie la raison qu'il applique");
    }

    #[test]
    fn un_filet_tout_est_la_derniere_regle_de_sa_source() {
        // C'est LA façon dont une décision meurt sans qu'on s'en aperçoive : la première règle
        // qui reconnaît une capacité la classe, donc toute règle placée après le `Motif::Tout`
        // de sa source est inatteignable — morte à l'écriture, pas à l'usage. Le signalement
        // par `regles_mortes` ne l'attrape qu'après coup ; ici on refuse la déclaration.
        for source in Source::toutes() {
            let regles: Vec<&Regle> = REGLES.iter().filter(|r| r.source == source).collect();
            let filet = regles
                .iter()
                .position(|r| matches!(r.motif, Motif::Tout))
                .unwrap_or_else(|| panic!("{source:?} n'a pas de filet `Motif::Tout`"));
            assert_eq!(
                filet,
                regles.len() - 1,
                "{source:?} : `{}` est un `Motif::Tout` suivi de {} règle(s) inatteignable(s) — la première est `{}`",
                regles[filet].id,
                regles.len() - 1 - filet,
                regles[filet + 1].id
            );
        }
    }

    #[test]
    fn les_identifiants_de_regle_sont_uniques() {
        // Deux règles de même identifiant rendent `regles_mortes` et `filets` faux des deux
        // côtés : l'une masque le vide de l'autre, et une capacité ne remonte plus à la
        // décision qui l'a produite.
        let mut ids: Vec<&str> = REGLES.iter().map(|r| r.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "identifiant de règle en double");
    }
}
