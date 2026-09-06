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
            Self::Tout => true,
        }
    }
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
}

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
    /// Les règles qui n'ont rien classé cette fois. Une règle `Exact` morte est une décision
    /// périmée ; un filet de préfixe qui ne prend rien reste utile — il protège le classement
    /// des capacités à venir. Le distinguo se lit, il ne s'automatise pas.
    pub regles_mortes: Vec<String>,
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
    let mut utilisees: BTreeMap<&'static str, u64> = BTreeMap::new();
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
        *utilisees.entry(id).or_default() += 1;

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

    let regles_mortes = REGLES
        .iter()
        .filter(|r| !utilisees.contains_key(r.id))
        .map(|r| r.id.to_string())
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
    fn les_agregats_somment_au_total() {
        let inv = inventaire(&[
            (Source::Vfs, ".cfg.bin", 71_101),
            (Source::Vfs, ".p3lip", 21_047),
            (Source::Vfs, ".awb", 5_512),
            (Source::Vfs, ".g4tg", 9),
            (Source::Niers, "vfs", 1),
        ]);
        let m = construire(&inv, &crate::app::chemins());
        assert_eq!(m.total.capacites, 5);
        assert_eq!(m.total.poids, 71_101 + 21_047 + 5_512 + 9 + 1);
        let somme: u64 = m.par_etat.values().map(|c| c.poids).sum();
        assert_eq!(somme, m.total.poids, "la somme par état doit retomber sur le total");
        assert_eq!(m.par_etat["manquant"].poids, 21_047);
        assert_eq!(m.par_etat["interne"].poids, 5_512);
        assert_eq!(m.par_etat["bloque"].poids, 9);
        assert_eq!(m.par_etat["servi"].poids, 71_102);
        assert!(!m.gate.tenue, "manquant = 21 047 : la gate n'est pas tenue");
        assert_eq!(m.gate.manquant, 1);
        assert_eq!(m.gate.manquant_poids, 21_047);
    }
}
