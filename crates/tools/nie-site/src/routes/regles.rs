//! `/api/v1/regles` — les **règles de jeu** du dépôt, exposées en lecture.
//!
//! `nie-core` porte la logique de jeu pure d'IEVR : la courbe de statistiques à trois segments,
//! les tables de croissance réelles, la comparaison des variantes d'un personnage et les builds
//! BASARA. Elle est écrite, testée par 297 cas, et jusqu'ici **aucune route ne l'appelait** —
//! c'est ce que la matrice de couverture appelle `manquant` (`docs/PLAN-SITE-ULTIME.md`), et ce
//! que `docs/inagle/05-service-et-types.md` § 5.4 classait deuxième dans l'ordre qui fait tomber
//! le plus de code : *`nie-core` est déjà écrit, c'est du câblage*.
//!
//! # Ce que ce module n'est pas
//!
//! Ce n'est pas un second moteur. Aucune formule n'est recopiée ici : chaque handler appelle
//! [`nie_core`] et se contente de traduire ses structures en JSON. Réécrire une interpolation
//! « pour éviter une dépendance » créerait la troisième implémentation du même calcul (le dépôt
//! en porte déjà deux, `nie-core::growth` et `nie-data::growth`, doublon signalé et non traité).
//!
//! # Les quatre divergences d'inagle, préservées et publiées
//!
//! Le portage a fait apparaître quatre comportements du TypeScript d'origine qui ne sont
//! documentés nulle part côté inagle (`docs/inagle/03-migration-rust.md` § 2.4). Ils sont portés
//! **tels quels** et figés par un test côté `nie-core`. Les taire côté API reviendrait à laisser
//! un client les prendre pour des faits du jeu : [`DIVERGENCES`] les rend, et
//! `GET /api/v1/regles` les publie.
//!
//! # Sérialisation
//!
//! Aucune valeur publique ne sort par `Debug`. [`Classification`] est un `enum` Rust : son nom de
//! variante est un détail d'implémentation, et il est traduit en deux champs choisis — un jeton
//! machine stable (`jeton`) et le libellé exact d'inagle (`libelle_inagle`), pour qu'une sortie
//! de cette route se compare sans ambiguïté à celle du TypeScript.
//!
//! # Paramètres
//!
//! Les trois routes à query lisent leurs paramètres **à plat** et les valident une par une : un
//! paramètre hors bornes rend un `400` qui nomme le paramètre et sa borne, et une clé inconnue
//! rend un `400` qui liste les clés acceptées. Une clé silencieusement ignorée serait pire qu'une
//! clé refusée — le client croirait filtrer.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::OnceLock;

use axum::Json;
use axum::extract::Query;
use serde::{Deserialize, Serialize};

use nie_core::comparaison::{
    Classification, NOMS_STATS_COMPAREES, ResultatComparaison, VarianteComparable,
    comparer_variantes,
};
use nie_core::growth::{GrowthParams, GrowthTables, calculate_stats, generate_growth_curve};
use nie_core::optimisation::{BUILDS_BASARA, builds_basara_classes};
use nie_core::stats::{
    LIBELLES_POSITION_INAGLE, LIBELLES_RANG_INAGLE, LIBELLES_STATS, StatBlock, rarity_code_to_name,
    rarity_to_growth_rank,
};

use crate::error::ErreurSite;

/// Niveau maximal admis par la courbe du jeu.
///
/// `calculate_single_stat` sature à 99 : demander 120 rendrait exactement la même chose que 99,
/// en laissant croire que le niveau 120 existe. La borne est donc refusée, pas écrêtée.
pub const NIVEAU_MAX: u8 = 99;

/// Niveau minimal admis. Le niveau 0 est traité comme le niveau 1 par le moteur ; l'accepter
/// ferait rendre la même réponse à deux entrées différentes.
pub const NIVEAU_MIN: u8 = 1;

/// Nombre maximal de variantes acceptées dans une comparaison.
///
/// Le personnage le plus décliné du jeu en compte quelques dizaines ; la borne existe pour
/// qu'un corps de requête ne décide pas seul du temps de calcul du service.
pub const VARIANTES_MAX: usize = 64;

/// Les tables de croissance réelles, chargées une seule fois.
///
/// `GrowthTables::load_embedded()` désérialise un JSON embarqué par `include_str!` : aucun accès
/// disque, aucune dépendance au VFS, et donc **cette route répond même sans le jeu installé**.
/// C'est la seule route d'API de ce service dans ce cas, et c'est ce qui en fait un bon
/// indicateur de vie du moteur.
fn tables() -> &'static GrowthTables {
    static TABLES: OnceLock<GrowthTables> = OnceLock::new();
    TABLES.get_or_init(GrowthTables::load_embedded)
}

// ---------------------------------------------------------------------------------------------
// DTO partagés
// ---------------------------------------------------------------------------------------------

/// Les 7 statistiques et leur total, sous les clés d'inagle.
///
/// L'ordre est celui de `StatBlock::as_array` (`[Kc, Cr, Tc, Pr, Ps, Ag, It]`). `pr` porte
/// *physical* et `ps` porte *pressure* : les intervertir rendrait des blocs plausibles et faux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats7 {
    /// Kick — puissance de tir.
    pub kc: u16,
    /// Control — contrôle du ballon.
    pub cr: u16,
    /// Technique — hissatsu et mouvements techniques.
    pub tc: u16,
    /// Power/Physical — puissance physique, duels.
    pub pr: u16,
    /// Pressure — pressing, interceptions.
    pub ps: u16,
    /// Agility — vitesse, démarrage.
    pub ag: u16,
    /// Intelligence — lecture du jeu, positionnement.
    pub it: u16,
    /// Somme des sept, telle que la calcule `StatBlock::total`.
    pub total: u32,
}

impl From<StatBlock> for Stats7 {
    fn from(b: StatBlock) -> Self {
        Self {
            kc: b.kc,
            cr: b.cr,
            tc: b.tc,
            pr: b.pr,
            ps: b.ps,
            ag: b.ag,
            it: b.it,
            total: b.total(),
        }
    }
}

impl From<Stats7> for StatBlock {
    fn from(s: Stats7) -> Self {
        Self {
            kc: s.kc,
            cr: s.cr,
            tc: s.tc,
            pr: s.pr,
            ps: s.ps,
            ag: s.ag,
            it: s.it,
        }
    }
}

/// Une divergence volontaire entre inagle et la vérité terrain du dépôt.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Divergence {
    /// Jeton machine, stable.
    pub jeton: &'static str,
    /// Ce que dit inagle.
    pub inagle: &'static str,
    /// Ce que dit le dépôt, et d'où il le tient.
    pub depot: &'static str,
    /// Pourquoi elle n'est pas corrigée ici.
    pub statut: &'static str,
}

/// Les quatre divergences portées telles quelles, chacune figée par un test de `nie-core`.
///
/// Source : `docs/inagle/03-migration-rust.md` § 2.4. Aucune n'est un bug de ce service : ce sont
/// des arbitrages ouverts, et les publier est la seule façon qu'un client ne les prenne pas pour
/// des règles du jeu.
pub const DIVERGENCES: [Divergence; 4] = [
    Divergence {
        jeton: "libelles_position_inverses",
        inagle: "POSITION_LABELS dit 2 = DF et 4 = FW",
        depot: "enum Position { GK=1, FW=2, MF=3, DF=4 } (refs/iecode-re, et les goldens de growth.rs)",
        statut: "porte verbatim sous LIBELLES_POSITION_INAGLE, non corrige : trancher l'affichage est un arbitrage",
    },
    Divergence {
        jeton: "deux_echelles_de_rang",
        inagle: "RANK_LABELS est indexe 1-5 et anglais (R (Rare)) quand rarityCodeToName est indexe 0-20 et francais (Experimente)",
        depot: "les deux tables coexistent et ne decrivent pas la meme echelle",
        statut: "portees separement, aucune des deux corrigee",
    },
    Divergence {
        jeton: "asymetrie_void_synergie",
        inagle: "la garde element dominant != Void protege la cohesion elementaire mais pas les 30 points de score",
        depot: "une equipe integralement Void n'affiche aucune cohesion et encaisse quand meme son bonus",
        statut: "comportement du TS porte tel quel ; deux tests qui affirmaient l'inverse ont ete corriges",
    },
    Divergence {
        jeton: "series_evolution_morte",
        inagle: "la classification Series Evolution est declaree mais seriesChanged est une constante false",
        depot: "aucune combinaison d'entrees ne la produit ; un test le prouve",
        statut: "variante conservee pour que le type reste complet",
    },
];

/// Jeton machine d'une classification de variante.
///
/// Le nom de variante Rust n'est **pas** une sérialisation : ce `match` choisit la chaîne
/// publiée. Il est exhaustif par construction — ajouter une variante à `Classification` casse la
/// compilation de cette fonction, ce qui est exactement le rappel qu'on veut.
#[must_use]
pub fn jeton_classification(c: Classification) -> &'static str {
    match c {
        Classification::VersionBase => "version_base",
        Classification::AmeliorationPure => "amelioration_pure",
        Classification::ChangementElement => "changement_element",
        Classification::ChangementPoste => "changement_poste",
        Classification::VariationTactique => "variation_tactique",
        Classification::EvolutionSerie => "evolution_serie",
        Classification::EvolutionHybride => "evolution_hybride",
    }
}

/// Les sept classifications, dans l'ordre du type.
///
/// Écrite à la main parce que Rust n'énumère pas ses `enum` : le test
/// `les_sept_classifications_sont_toutes_listees` la confronte aux jetons distincts pour qu'un
/// oubli rougisse au lieu de disparaître.
pub const CLASSIFICATIONS: [Classification; 7] = [
    Classification::VersionBase,
    Classification::AmeliorationPure,
    Classification::ChangementElement,
    Classification::ChangementPoste,
    Classification::VariationTactique,
    Classification::EvolutionSerie,
    Classification::EvolutionHybride,
];

// ---------------------------------------------------------------------------------------------
// Lecture des paramètres de query
// ---------------------------------------------------------------------------------------------

/// Refuse toute clé de query hors de la liste acceptée.
///
/// # Errors
///
/// `Demande` si une clé inconnue est présente, en listant les clés acceptées.
fn refuser_cles_inconnues(
    params: &HashMap<String, String>,
    acceptees: &[&str],
) -> Result<(), ErreurSite> {
    let mut inconnues: Vec<&str> = params
        .keys()
        .map(String::as_str)
        .filter(|k| !acceptees.contains(k))
        .collect();
    if inconnues.is_empty() {
        return Ok(());
    }
    inconnues.sort_unstable();
    Err(ErreurSite::Demande(format!(
        "parametre(s) inconnu(s): {} ; acceptes: {}",
        inconnues.join(", "),
        acceptees.join(", ")
    )))
}

/// Lit un nombre optionnel, avec sa valeur par défaut et ses bornes incluses.
///
/// # Errors
///
/// `Demande` si la valeur n'est pas un nombre, ou si elle sort des bornes. Aucune valeur n'est
/// écrêtée en silence : un client qui demande le niveau 150 doit apprendre que 150 n'existe pas,
/// pas recevoir le niveau 99 sous son nom.
fn nombre<T>(
    params: &HashMap<String, String>,
    nom: &str,
    defaut: T,
    min: T,
    max: T,
) -> Result<T, ErreurSite>
where
    T: FromStr + PartialOrd + std::fmt::Display + Copy,
{
    let Some(brut) = params.get(nom) else {
        return Ok(defaut);
    };
    let valeur: T = brut.trim().parse().map_err(|_| {
        ErreurSite::Demande(format!(
            "`{nom}` doit etre un entier (recu: `{brut}`), borne {min}..={max}"
        ))
    })?;
    if valeur < min || valeur > max {
        return Err(ErreurSite::Demande(format!(
            "`{nom}` hors bornes: {valeur} (attendu {min}..={max})"
        )));
    }
    Ok(valeur)
}

// ---------------------------------------------------------------------------------------------
// GET /api/v1/regles
// ---------------------------------------------------------------------------------------------

/// Une route de cette famille, telle qu'elle est publiée par l'index.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct RouteDecrite {
    /// Méthode HTTP.
    pub methode: &'static str,
    /// Chemin, dans la syntaxe Axum 0.8.
    pub chemin: &'static str,
    /// Ce qu'elle rend.
    pub resume: &'static str,
    /// L'item de `nie-core` qui fait réellement le calcul.
    pub moteur: &'static str,
}

/// Ce que les tables de croissance embarquées contiennent, **compté à l'exécution**.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TablesCroissance {
    /// Entrées de la table de niveau 1.
    pub lv1: usize,
    /// Entrées de la table de niveau 30.
    pub lv30: usize,
    /// Entrées de la table principale (paliers 50 et 99).
    pub main: usize,
    /// Entrées de la table de sous-position.
    pub sub: usize,
}

/// Une classification de variante, publiée avec son jeton et son libellé inagle.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ClassificationDecrite {
    /// Jeton machine ([`jeton_classification`]).
    pub jeton: &'static str,
    /// Libellé exact d'inagle, pour comparer les sorties sans ambiguïté.
    pub libelle_inagle: &'static str,
}

/// Un build BASARA, publié avec son type et ses multiplicateurs.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BuildDecrit {
    /// Type de build (0 à 5), tel que le jeu l'indexe.
    pub type_build: u8,
    /// Nom affiché.
    pub nom: &'static str,
    /// Description française.
    pub description: &'static str,
    /// Multiplicateurs par statistique, dans l'ordre `[Kc, Cr, Tc, Pr, Ps, Ag, It]`.
    /// `null` = statistique inchangée par ce build.
    pub multiplicateurs: [Option<f64>; 7],
}

/// Un libellé `(code, texte)` porté verbatim d'inagle.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct LibelleCode {
    /// Le code, tel qu'il apparaît dans les données du jeu.
    pub code: u8,
    /// Le libellé d'inagle.
    pub libelle: &'static str,
}

/// Un libellé de statistique, dans les trois langues d'inagle.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct LibelleStat {
    /// Clé courte (`Kc`, `Cr`, …), dans l'ordre de `StatBlock::as_array`.
    pub cle: &'static str,
    /// Nom anglais.
    pub anglais: &'static str,
    /// Nom japonais.
    pub japonais: &'static str,
}

/// Ce que le moteur sait calculer — **mesuré depuis le code**, pas déclaré.
///
/// Tous les comptes de cette structure sont des `.len()` sur les constantes et les tables
/// réelles de `nie-core`. Aucun n'est écrit à la main : un compte écrit à la main se périme à la
/// première mise à jour du moteur, et le dépôt s'est déjà trompé sur ses propres chiffres.
#[derive(Debug, Clone, Serialize)]
pub struct Capacites {
    /// Les routes de cette famille.
    pub routes: Vec<RouteDecrite>,
    /// Contenu des tables de croissance embarquées.
    pub tables_croissance: TablesCroissance,
    /// Nombre de segments de la courbe de statistiques (1→30, 30→50, 50→99).
    pub segments_courbe: u8,
    /// Niveau minimal accepté.
    pub niveau_min: u8,
    /// Niveau maximal accepté.
    pub niveau_max: u8,
    /// Les 7 statistiques et leurs libellés.
    pub statistiques: Vec<LibelleStat>,
    /// Les noms de stats employés par la comparaison (ceux d'inagle, en anglais long).
    pub stats_comparees: Vec<&'static str>,
    /// Les classifications de variante.
    pub classifications: Vec<ClassificationDecrite>,
    /// Les builds BASARA.
    pub builds: Vec<BuildDecrit>,
    /// Libellés de position d'inagle — voir la divergence `libelles_position_inverses`.
    pub positions_inagle: Vec<LibelleCode>,
    /// Libellés de rang d'inagle — voir la divergence `deux_echelles_de_rang`.
    pub rangs_inagle: Vec<LibelleCode>,
    /// Les divergences volontaires, publiées.
    pub divergences: [Divergence; 4],
    /// D'où vient le calcul. Nommer la source unique évite qu'on en écrive une seconde.
    pub moteur: &'static str,
}

/// `GET /api/v1/regles` — ce que le moteur de règles sait calculer.
///
/// # Errors
///
/// Ne peut pas échouer : tout vient de constantes et de tables embarquées. Le `Result` est
/// conservé pour que la signature reste homogène avec les autres routes de l'API.
pub async fn capacites() -> Result<Json<Capacites>, ErreurSite> {
    let t = tables();
    Ok(Json(Capacites {
        routes: vec![
            RouteDecrite {
                methode: "GET",
                chemin: "/api/v1/regles",
                resume: "ce que le moteur sait calculer",
                moteur: "nie_core::{stats,growth,comparaison,optimisation}",
            },
            RouteDecrite {
                methode: "GET",
                chemin: "/api/v1/regles/stats",
                resume: "statistiques a un niveau donne, et courbe de croissance complete",
                moteur: "nie_core::growth::{calculate_stats,generate_growth_curve}",
            },
            RouteDecrite {
                methode: "GET",
                chemin: "/api/v1/regles/comparaison",
                resume: "le contrat de la comparaison et son vocabulaire",
                moteur: "nie_core::comparaison",
            },
            RouteDecrite {
                methode: "POST",
                chemin: "/api/v1/regles/comparaison",
                resume: "comparer deux personnages (ou toutes les variantes d'un personnage)",
                moteur: "nie_core::comparaison::comparer_variantes",
            },
            RouteDecrite {
                methode: "GET",
                chemin: "/api/v1/regles/rarete",
                resume: "code de rarete vers son nom et son rang de table",
                moteur: "nie_core::stats::{rarity_code_to_name,rarity_to_growth_rank}",
            },
            RouteDecrite {
                methode: "GET",
                chemin: "/api/v1/regles/builds",
                resume: "les 6 builds BASARA projetes sur un bloc de statistiques",
                moteur: "nie_core::optimisation::builds_basara_classes",
            },
        ],
        tables_croissance: TablesCroissance {
            lv1: t.lv1.len(),
            lv30: t.lv30.len(),
            main: t.main.len(),
            sub: t.sub.len(),
        },
        segments_courbe: 3,
        niveau_min: NIVEAU_MIN,
        niveau_max: NIVEAU_MAX,
        statistiques: LIBELLES_STATS
            .iter()
            .map(|(cle, anglais, japonais)| LibelleStat {
                cle,
                anglais,
                japonais,
            })
            .collect(),
        stats_comparees: NOMS_STATS_COMPAREES.to_vec(),
        classifications: CLASSIFICATIONS
            .iter()
            .map(|c| ClassificationDecrite {
                jeton: jeton_classification(*c),
                libelle_inagle: c.libelle_inagle(),
            })
            .collect(),
        builds: BUILDS_BASARA
            .iter()
            .enumerate()
            .map(|(i, b)| BuildDecrit {
                // 6 builds : l'index tient dans un u8 sans discussion.
                type_build: u8::try_from(i).unwrap_or(0),
                nom: b.nom,
                description: b.description,
                multiplicateurs: b.multiplicateurs,
            })
            .collect(),
        positions_inagle: LIBELLES_POSITION_INAGLE
            .iter()
            .map(|(code, libelle)| LibelleCode {
                code: *code,
                libelle,
            })
            .collect(),
        rangs_inagle: LIBELLES_RANG_INAGLE
            .iter()
            .map(|(code, libelle)| LibelleCode {
                code: *code,
                libelle,
            })
            .collect(),
        divergences: DIVERGENCES,
        moteur: "nie_core",
    }))
}

// ---------------------------------------------------------------------------------------------
// GET /api/v1/regles/stats
// ---------------------------------------------------------------------------------------------

/// Les clés de query acceptées par [`stats`].
pub const CLES_STATS: [&str; 7] = [
    "niveau",
    "position",
    "sous_position",
    "pattern",
    "rarete",
    "style",
    "courbe",
];

/// Un point de la courbe de croissance, tel qu'il est publié.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PointCourbe {
    /// Niveau.
    pub niveau: u8,
    /// Statistiques à ce niveau.
    pub stats: Stats7,
}

/// Les paramètres de croissance effectivement appliqués.
///
/// Ils sont **renvoyés** : la résolution d'une entrée de table se fait par repli en cascade (4,
/// 5 et 4 niveaux selon la table), et un client qui reçoit des stats sans savoir sur quels
/// paramètres elles ont été calculées ne peut pas les reproduire.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ParametresAppliques {
    /// Position principale (`1=GK, 2=FW, 3=MF, 4=DF` — la convention du RE, pas celle d'inagle).
    pub position: u8,
    /// Sous-position (0 = aucune).
    pub sous_position: u8,
    /// Pattern de croissance (0 ou 1).
    pub pattern: u8,
    /// Code de rareté brut, tel qu'il apparaît dans `starSignCharaInfo.charaRarity`.
    pub rarete: u8,
    /// Rang de table déduit de la rareté par `rarity_to_growth_rank`.
    pub rang_table: u8,
    /// Style de jeu (0 = aucun).
    pub style: u8,
}

/// Réponse de `/api/v1/regles/stats`.
#[derive(Debug, Clone, Serialize)]
pub struct ReponseStats {
    /// Niveau demandé.
    pub niveau: u8,
    /// Statistiques à ce niveau.
    pub stats: Stats7,
    /// Les paramètres appliqués, y compris ceux qui viennent d'un défaut.
    pub parametres: ParametresAppliques,
    /// Les trois entrées de table ont-elles toutes été résolues ?
    ///
    /// `false` signifie que le moteur a rendu un bloc **à zéro** — parité voulue avec le
    /// TypeScript, qui journalise un avertissement et rend des stats nulles. Sans ce drapeau,
    /// un client prendrait ce zéro pour un personnage très faible.
    pub tables_resolues: bool,
    /// La courbe complète, seulement si `courbe=1` a été demandé.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub courbe: Option<Vec<PointCourbe>>,
}

/// `GET /api/v1/regles/stats` — les statistiques d'un personnage à un niveau donné.
///
/// Exemple : `?niveau=99&position=2&sous_position=3&rarete=5`.
///
/// # Errors
///
/// `Demande` (400) si une clé est inconnue, si une valeur n'est pas un entier, ou si elle sort
/// de ses bornes.
pub async fn stats(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ReponseStats>, ErreurSite> {
    refuser_cles_inconnues(&params, &CLES_STATS)?;

    let niveau = nombre(&params, "niveau", NIVEAU_MAX, NIVEAU_MIN, NIVEAU_MAX)?;
    let position = nombre(&params, "position", 2u8, 1, 4)?;
    let sous_position = nombre(&params, "sous_position", 0u8, 0, 4)?;
    let pattern = nombre(&params, "pattern", 0u8, 0, 1)?;
    // La rareté est le code BRUT du jeu (0..20), pas le rang de table : c'est
    // `rarity_to_growth_rank` qui fait la conversion, et la publier permet de la vérifier.
    let rarete = nombre(&params, "rarete", 5u8, 0, 20)?;
    let style = nombre(&params, "style", 0u8, 0, 9)?;
    let courbe_demandee = nombre(&params, "courbe", 0u8, 0, 1)? == 1;

    let p = GrowthParams {
        main_position: position,
        sub_position: sous_position,
        growth_pattern: pattern,
        chara_rank: rarete,
        play_style: style,
    };

    let t = tables();
    let tables_resolues =
        t.find_lv1(&p).is_some() && t.find_lv30(&p).is_some() && t.find_main(&p).is_some();
    let bloc = calculate_stats(t, &p, niveau);

    let courbe = courbe_demandee.then(|| {
        generate_growth_curve(t, &p, NIVEAU_MIN, NIVEAU_MAX)
            .into_iter()
            .map(|pt| PointCourbe {
                niveau: pt.niveau,
                stats: pt.stats.into(),
            })
            .collect()
    });

    Ok(Json(ReponseStats {
        niveau,
        stats: bloc.into(),
        parametres: ParametresAppliques {
            position,
            sous_position,
            pattern,
            rarete,
            rang_table: rarity_to_growth_rank(rarete),
            style,
        },
        tables_resolues,
        courbe,
    }))
}

// ---------------------------------------------------------------------------------------------
// /api/v1/regles/comparaison
// ---------------------------------------------------------------------------------------------

/// Une variante de personnage, telle qu'elle entre dans une comparaison.
#[derive(Debug, Clone, Deserialize)]
pub struct VarianteEntrante {
    /// Identifiant de la variante (`charaParamId`).
    pub chara_param_id: String,
    /// Libellé de rareté, tel qu'affiché.
    #[serde(default)]
    pub rarete: String,
    /// Élément, sous sa forme d'affichage.
    #[serde(default)]
    pub element: String,
    /// Poste, sous sa forme d'affichage.
    #[serde(default)]
    pub position: String,
    /// Statistiques au niveau 99 — le seul palier que compare inagle.
    pub stats_lv99: Stats7,
    /// Identifiants des techniques apprises.
    #[serde(default)]
    pub techniques: Vec<String>,
}

impl From<VarianteEntrante> for VarianteComparable {
    fn from(v: VarianteEntrante) -> Self {
        Self {
            chara_param_id: v.chara_param_id,
            rarete: v.rarete,
            element: v.element,
            position: v.position,
            stats_lv99: v.stats_lv99.into(),
            techniques: v.techniques,
        }
    }
}

/// Corps de `POST /api/v1/regles/comparaison`.
///
/// Deux formes, une seule règle : la **base** est le premier élément, comme dans le TypeScript,
/// qui suppose `variants[0]` la variante la plus jeune.
#[derive(Debug, Clone, Deserialize)]
pub struct DemandeComparaison {
    /// La variante de référence.
    pub base: VarianteEntrante,
    /// La ou les variantes à lui comparer.
    pub variantes: Vec<VarianteEntrante>,
    /// Table `identifiant de technique -> nom affichable`. Un identifiant absent est rendu tel
    /// quel, comme le `skillsMap.get(id) || id` du TypeScript.
    #[serde(default)]
    pub noms_techniques: HashMap<String, String>,
}

/// L'écart d'une statistique entre la base et la variante.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct EcartPublie {
    /// Nom de la stat, parmi ceux d'inagle.
    pub stat: &'static str,
    /// Valeur chez la base.
    pub valeur_base: i32,
    /// Valeur chez la variante.
    pub valeur_variante: i32,
    /// `valeur_variante - valeur_base`.
    pub ecart: i32,
}

/// Le résultat d'une comparaison, tel qu'il est publié.
#[derive(Debug, Clone, Serialize)]
pub struct ComparaisonPubliee {
    /// `charaParamId` de la variante comparée.
    pub variante_id: String,
    /// Rareté de la variante.
    pub rarete: String,
    /// Élément de la variante.
    pub element: String,
    /// Poste de la variante.
    pub position: String,
    /// Jeton machine de la classification.
    pub classification: &'static str,
    /// Libellé exact d'inagle, pour une comparaison directe avec la sortie du TypeScript.
    pub classification_inagle: &'static str,
    /// L'élément diffère-t-il ?
    pub element_change: bool,
    /// Le poste diffère-t-il ?
    pub position_changee: bool,
    /// Écarts stat par stat.
    pub ecarts_stats: Vec<EcartPublie>,
    /// Somme des écarts.
    pub ecart_total: i32,
    /// Techniques présentes chez la variante et absentes de la base.
    pub techniques_ajoutees: Vec<String>,
    /// Techniques présentes chez la base et absentes de la variante.
    pub techniques_retirees: Vec<String>,
    /// Phrase d'explication, en français, telle que la construit inagle.
    pub explication: String,
}

impl From<ResultatComparaison> for ComparaisonPubliee {
    fn from(r: ResultatComparaison) -> Self {
        Self {
            variante_id: r.variante_id,
            rarete: r.rarete,
            element: r.element,
            position: r.position,
            classification: jeton_classification(r.classification),
            classification_inagle: r.classification.libelle_inagle(),
            element_change: r.element_change,
            position_changee: r.position_changee,
            ecarts_stats: r
                .ecarts_stats
                .into_iter()
                .map(|e| EcartPublie {
                    stat: e.stat,
                    valeur_base: e.valeur_base,
                    valeur_variante: e.valeur_variante,
                    ecart: e.ecart,
                })
                .collect(),
            ecart_total: r.ecart_total,
            techniques_ajoutees: r.techniques_ajoutees,
            techniques_retirees: r.techniques_retirees,
            explication: r.explication,
        }
    }
}

/// Réponse de la comparaison.
#[derive(Debug, Clone, Serialize)]
pub struct ReponseComparaison {
    /// `charaParamId` de la base.
    pub base_id: String,
    /// Un résultat par variante soumise, dans l'ordre de la demande.
    pub resultats: Vec<ComparaisonPubliee>,
    /// Les divergences volontaires qui s'appliquent à cette route.
    pub divergences: [Divergence; 4],
}

/// Le contrat de la comparaison, rendu en `GET`.
#[derive(Debug, Clone, Serialize)]
pub struct ContratComparaison {
    /// Méthode à employer pour comparer.
    pub methode: &'static str,
    /// Chemin de la route.
    pub chemin: &'static str,
    /// Champs attendus dans le corps.
    pub corps: Vec<&'static str>,
    /// Nombre maximal de variantes acceptées.
    pub variantes_max: usize,
    /// Le vocabulaire des classifications.
    pub classifications: Vec<ClassificationDecrite>,
    /// Les noms de stats employés dans les écarts.
    pub stats_comparees: Vec<&'static str>,
    /// Les divergences volontaires.
    pub divergences: [Divergence; 4],
}

/// `GET /api/v1/regles/comparaison` — le contrat, plutôt qu'un `405`.
///
/// La comparaison prend deux personnages entiers : elle ne tient pas dans une query, donc elle
/// se demande en `POST`. Une route qui n'existerait qu'en `POST` rendrait un `405` muet au
/// premier client qui l'explore ; celle-ci dit ce qu'il faut envoyer.
///
/// # Errors
///
/// Ne peut pas échouer.
pub async fn contrat_comparaison() -> Result<Json<ContratComparaison>, ErreurSite> {
    Ok(Json(ContratComparaison {
        methode: "POST",
        chemin: "/api/v1/regles/comparaison",
        corps: vec!["base", "variantes", "noms_techniques"],
        variantes_max: VARIANTES_MAX,
        classifications: CLASSIFICATIONS
            .iter()
            .map(|c| ClassificationDecrite {
                jeton: jeton_classification(*c),
                libelle_inagle: c.libelle_inagle(),
            })
            .collect(),
        stats_comparees: NOMS_STATS_COMPAREES.to_vec(),
        divergences: DIVERGENCES,
    }))
}

/// `POST /api/v1/regles/comparaison` — comparer deux personnages.
///
/// # Errors
///
/// `Demande` (400) si `variantes` est vide ou dépasse [`VARIANTES_MAX`].
pub async fn comparaison(
    Json(demande): Json<DemandeComparaison>,
) -> Result<Json<ReponseComparaison>, ErreurSite> {
    if demande.variantes.is_empty() {
        return Err(ErreurSite::Demande(
            "`variantes` est vide : il faut au moins une variante a comparer a `base`".to_owned(),
        ));
    }
    if demande.variantes.len() > VARIANTES_MAX {
        return Err(ErreurSite::Demande(format!(
            "trop de variantes: {} (borne {VARIANTES_MAX})",
            demande.variantes.len()
        )));
    }

    let base: VarianteComparable = demande.base.into();
    let noms = demande.noms_techniques;
    let resultats = demande
        .variantes
        .into_iter()
        .map(|v| comparer_variantes(&base, &VarianteComparable::from(v), &noms).into())
        .collect();

    Ok(Json(ReponseComparaison {
        base_id: base.chara_param_id,
        resultats,
        divergences: DIVERGENCES,
    }))
}

// ---------------------------------------------------------------------------------------------
// GET /api/v1/regles/rarete
// ---------------------------------------------------------------------------------------------

/// Les clés de query acceptées par [`rarete`].
pub const CLES_RARETE: [&str; 1] = ["code"];

/// Codes de rareté que le jeu attribue réellement, dans l'ordre.
///
/// La borne haute est 20 (BASARA). Les codes intermédiaires non listés existent en tant
/// qu'entiers mais ne sont attribués par aucun système : `rarity_code_to_name` leur rend
/// `Rank<n>`, et les publier comme des raretés serait inventer une échelle.
pub const CODES_RARETE: [u8; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 20];

/// Un code de rareté et ce que le moteur en fait.
#[derive(Debug, Clone, Serialize)]
pub struct RareteDecrite {
    /// Le code brut (`starSignCharaInfo.charaRarity`).
    pub code: u8,
    /// Le nom d'affichage français.
    pub nom: String,
    /// Le rang de table de croissance déduit.
    pub rang_table: u8,
    /// Le code est-il attribué par un système du jeu, ou seulement calculable ?
    pub attribue: bool,
}

/// Réponse de `/api/v1/regles/rarete`.
#[derive(Debug, Clone, Serialize)]
pub struct ReponseRarete {
    /// Les raretés demandées (toutes, ou celle de `?code=`).
    pub raretes: Vec<RareteDecrite>,
    /// Les fonctions qui font la conversion.
    pub moteur: &'static str,
}

/// Décrit un code de rareté.
fn decrire_rarete(code: u8) -> RareteDecrite {
    RareteDecrite {
        code,
        nom: rarity_code_to_name(code),
        rang_table: rarity_to_growth_rank(code),
        attribue: CODES_RARETE.contains(&code),
    }
}

/// `GET /api/v1/regles/rarete` — le code de rareté vers son nom.
///
/// Sans paramètre, rend les 9 codes attribués. Avec `?code=N`, rend ce seul code — y compris un
/// code non attribué, en le marquant `attribue: false` : le moteur sait le nommer (`Rank42`), et
/// le taire ferait passer une réponse valide pour une absence.
///
/// # Errors
///
/// `Demande` (400) si une clé est inconnue ou si `code` n'est pas un entier de 0 à 255.
pub async fn rarete(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ReponseRarete>, ErreurSite> {
    refuser_cles_inconnues(&params, &CLES_RARETE)?;

    let raretes = if params.contains_key("code") {
        let code = nombre(&params, "code", 0u8, 0, u8::MAX)?;
        vec![decrire_rarete(code)]
    } else {
        CODES_RARETE.iter().copied().map(decrire_rarete).collect()
    };

    Ok(Json(ReponseRarete {
        raretes,
        moteur: "nie_core::stats::{rarity_code_to_name,rarity_to_growth_rank}",
    }))
}

// ---------------------------------------------------------------------------------------------
// GET /api/v1/regles/builds
// ---------------------------------------------------------------------------------------------

/// Les clés de query acceptées par [`builds`].
pub const CLES_BUILDS: [&str; 7] = ["kc", "cr", "tc", "pr", "ps", "ag", "it"];

/// Un build projeté sur les statistiques soumises.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BuildProjete {
    /// Type de build (0 à 5).
    pub type_build: u8,
    /// Nom du build.
    pub nom: &'static str,
    /// Statistiques projetées.
    pub stats: Stats7,
}

/// Réponse de `/api/v1/regles/builds`.
#[derive(Debug, Clone, Serialize)]
pub struct ReponseBuilds {
    /// Les statistiques de départ, telles qu'elles ont été lues.
    pub base: Stats7,
    /// Les 6 builds, classés par puissance totale décroissante.
    pub builds: Vec<BuildProjete>,
    /// La fonction qui fait le classement.
    pub moteur: &'static str,
}

/// `GET /api/v1/regles/builds` — les 6 builds BASARA projetés sur un bloc de statistiques.
///
/// C'est la moitié **lisible** de `nie_core::optimisation` : une fonction pure d'un bloc de 7
/// entiers vers 6 projections, qui tient donc entièrement dans une query. L'autre moitié
/// (`calculer_synergie_equipe`) prend un effectif complet et un entraîneur : ce n'est pas une
/// lecture, c'est un calculateur à corps de requête, et elle n'est pas routée ici.
///
/// # Errors
///
/// `Demande` (400) si une clé est inconnue ou si une statistique n'est pas un entier de 0 à 999.
pub async fn builds(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ReponseBuilds>, ErreurSite> {
    refuser_cles_inconnues(&params, &CLES_BUILDS)?;

    // 999 : la plus haute statistique réelle du jeu au niveau 99 est 261, et le build le plus
    // multiplicateur vaut 1,3. Une borne large, mais qui refuse quand même l'absurde.
    let mut brut = [0u16; 7];
    for (i, cle) in CLES_BUILDS.iter().enumerate() {
        brut[i] = nombre(&params, cle, 0u16, 0, 999)?;
    }
    let base = StatBlock::from_array(brut);

    let builds = builds_basara_classes(base)
        .into_iter()
        .map(|r| BuildProjete {
            type_build: r.type_build,
            nom: r.nom,
            stats: r.stats.into(),
        })
        .collect();

    Ok(Json(ReponseBuilds {
        base: base.into(),
        builds,
        moteur: "nie_core::optimisation::builds_basara_classes",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(paires: &[(&str, &str)]) -> HashMap<String, String> {
        paires
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    // ------------------------------------------------------------------------------------
    // Les paramètres : honorés, ou refusés — jamais ignorés
    // ------------------------------------------------------------------------------------

    #[test]
    fn une_cle_inconnue_est_un_400_qui_liste_les_cles_acceptees() {
        let e = refuser_cles_inconnues(&query(&[("nivo", "99")]), &CLES_STATS).unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
        let m = format!("{e}");
        assert!(m.contains("nivo"), "{m}");
        assert!(m.contains("niveau"), "{m}");
    }

    #[test]
    fn une_valeur_hors_bornes_est_refusee_pas_ecretee() {
        // Le piege que ce test garde : `calculate_single_stat` sature a 99. Ecreter 150 en 99
        // rendrait une reponse juste sous une question fausse.
        let e = nombre(&query(&[("niveau", "150")]), "niveau", 99u8, 1, 99).unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("hors bornes"), "{e}");
    }

    #[test]
    fn une_valeur_non_numerique_est_un_400_qui_nomme_le_parametre() {
        let e = nombre(&query(&[("niveau", "abc")]), "niveau", 99u8, 1, 99).unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("niveau"), "{e}");
    }

    #[test]
    fn un_parametre_absent_prend_son_defaut() {
        assert_eq!(nombre(&query(&[]), "niveau", 99u8, 1, 99).unwrap(), 99);
    }

    // ------------------------------------------------------------------------------------
    // Les stats : la vérité vient de nie-core, pas d'ici
    // ------------------------------------------------------------------------------------

    #[tokio::test]
    async fn stats_lv1_position_2_sous_3_est_la_premiere_ligne_de_la_table() {
        // Verite terrain : `growth.rs` teste que `lv1[0]` porte main_position=2, sub_position=3
        // et les stats [13,14,12,10,10,9,11]. La route doit rendre EXACTEMENT ces valeurs.
        let r = stats(Query(query(&[
            ("niveau", "1"),
            ("position", "2"),
            ("sous_position", "3"),
            ("rarete", "5"),
        ])))
        .await
        .unwrap()
        .0;
        assert!(r.tables_resolues);
        assert_eq!(
            [
                r.stats.kc, r.stats.cr, r.stats.tc, r.stats.pr, r.stats.ps, r.stats.ag, r.stats.it
            ],
            [13, 14, 12, 10, 10, 9, 11]
        );
        assert_eq!(r.stats.total, 79);
    }

    #[tokio::test]
    async fn le_niveau_99_rend_le_palier_99_de_la_table_main() {
        // `growth.rs` teste que `main[0]` (position 2, pattern 0, rang 5) porte
        // stats_99 = [261, 258, 235, 210, 202, 195, 230].
        let r = stats(Query(query(&[
            ("niveau", "99"),
            ("position", "2"),
            ("sous_position", "3"),
            ("rarete", "5"),
        ])))
        .await
        .unwrap()
        .0;
        assert_eq!(
            [
                r.stats.kc, r.stats.cr, r.stats.tc, r.stats.pr, r.stats.ps, r.stats.ag, r.stats.it
            ],
            [261, 258, 235, 210, 202, 195, 230]
        );
    }

    #[tokio::test]
    async fn la_courbe_n_est_rendue_que_si_on_la_demande() {
        let sans = stats(Query(query(&[("niveau", "50")]))).await.unwrap().0;
        assert!(sans.courbe.is_none());

        let avec = stats(Query(query(&[("niveau", "50"), ("courbe", "1")])))
            .await
            .unwrap()
            .0;
        let courbe = avec.courbe.expect("courbe demandee");
        assert_eq!(courbe.len(), 99, "un point par niveau de 1 a 99");
        assert_eq!(courbe[0].niveau, 1);
        assert_eq!(courbe[98].niveau, 99);
        // Le point de la courbe au niveau demande est le meme que la stat rendue a plat : une
        // divergence ici voudrait dire deux chemins de calcul.
        assert_eq!(courbe[49].stats, avec.stats);
    }

    #[tokio::test]
    async fn le_rang_de_table_publie_est_bien_celui_qui_a_servi() {
        // BASARA (20) tape les tables du rang 5, comme UR. Le publier permet de le verifier
        // sans relire le code.
        let r = stats(Query(query(&[("rarete", "20")]))).await.unwrap().0;
        assert_eq!(r.parametres.rarete, 20);
        assert_eq!(r.parametres.rang_table, 5);
        let ur = stats(Query(query(&[("rarete", "5")]))).await.unwrap().0;
        assert_eq!(
            r.stats, ur.stats,
            "BASARA et UR partagent la table du rang 5"
        );
    }

    #[tokio::test]
    async fn stats_refuse_une_position_inexistante() {
        let e = stats(Query(query(&[("position", "7")]))).await.unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
    }

    // ------------------------------------------------------------------------------------
    // La rareté : parité avec `packages/inagle/src/lib/rarity.ts`
    // ------------------------------------------------------------------------------------

    #[tokio::test]
    async fn la_table_de_rarete_est_celle_du_typescript() {
        let r = rarete(Query(query(&[]))).await.unwrap().0;
        assert_eq!(r.raretes.len(), CODES_RARETE.len());
        // Verite terrain : `rarity.ts` L31-53 (nom) et L66-83 (rang).
        let attendu: [(u8, &str, u8); 9] = [
            (0, "Normal", 0),
            (1, "Normal", 1),
            (2, "Expérimenté", 2),
            (3, "Émérite", 3),
            (4, "Normal", 4),
            (5, "Légendaire", 5),
            (6, "Légendaire", 5),
            (7, "Légendaire", 5),
            (20, "BASARA", 5),
        ];
        for (i, (code, nom, rang)) in attendu.iter().enumerate() {
            assert_eq!(r.raretes[i].code, *code);
            assert_eq!(&r.raretes[i].nom, nom, "code {code}");
            assert_eq!(r.raretes[i].rang_table, *rang, "code {code}");
            assert!(r.raretes[i].attribue);
        }
    }

    #[tokio::test]
    async fn un_code_non_attribue_est_rendu_et_marque_comme_tel() {
        let r = rarete(Query(query(&[("code", "42")]))).await.unwrap().0;
        assert_eq!(r.raretes.len(), 1);
        assert_eq!(r.raretes[0].nom, "Rank42");
        assert!(!r.raretes[0].attribue);
        // `rarityToGrowthRank` par defaut : `code <= 5 ? code : 5`.
        assert_eq!(r.raretes[0].rang_table, 5);
    }

    #[tokio::test]
    async fn rarete_refuse_un_code_qui_ne_tient_pas_dans_un_octet() {
        let e = rarete(Query(query(&[("code", "300")]))).await.unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
    }

    // ------------------------------------------------------------------------------------
    // La comparaison
    // ------------------------------------------------------------------------------------

    fn variante(id: &str, position: &str, element: &str, s: [u16; 7]) -> VarianteEntrante {
        VarianteEntrante {
            chara_param_id: id.to_owned(),
            rarete: "Légendaire".to_owned(),
            element: element.to_owned(),
            position: position.to_owned(),
            stats_lv99: StatBlock::from_array(s).into(),
            techniques: Vec::new(),
        }
    }

    #[tokio::test]
    async fn une_amelioration_pure_est_classee_et_chiffree() {
        let base = variante("0xA", "FW", "Fire", [100; 7]);
        let mieux = variante("0xB", "FW", "Fire", [110, 100, 100, 100, 100, 100, 100]);
        let r = comparaison(Json(DemandeComparaison {
            base,
            variantes: vec![mieux],
            noms_techniques: HashMap::new(),
        }))
        .await
        .unwrap()
        .0;
        assert_eq!(r.resultats.len(), 1);
        let c = &r.resultats[0];
        assert_eq!(c.classification, "amelioration_pure");
        // Le libelle d'inagle, tel quel : c'est lui qui rend les deux sorties comparables.
        assert_eq!(c.classification_inagle, "Pure Upgrade");
        assert_eq!(c.ecart_total, 10);
        assert_eq!(c.ecarts_stats.len(), 7);
        assert_eq!(c.ecarts_stats[0].stat, "kick");
        assert_eq!(c.ecarts_stats[0].ecart, 10);
        assert_eq!(c.ecarts_stats[1].ecart, 0);
    }

    #[tokio::test]
    async fn poste_et_element_changes_donnent_une_evolution_hybride() {
        let base = variante("0xA", "FW", "Fire", [100; 7]);
        let autre = variante("0xB", "DF", "Wind", [100; 7]);
        let r = comparaison(Json(DemandeComparaison {
            base,
            variantes: vec![autre],
            noms_techniques: HashMap::new(),
        }))
        .await
        .unwrap()
        .0;
        let c = &r.resultats[0];
        assert_eq!(c.classification, "evolution_hybride");
        assert_eq!(c.classification_inagle, "Hybrid Evolution");
        assert!(c.element_change && c.position_changee);
    }

    #[tokio::test]
    async fn une_demande_sans_variante_est_un_400() {
        let e = comparaison(Json(DemandeComparaison {
            base: variante("0xA", "FW", "Fire", [100; 7]),
            variantes: Vec::new(),
            noms_techniques: HashMap::new(),
        }))
        .await
        .unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
    }

    #[tokio::test]
    async fn au_dela_de_la_borne_la_comparaison_refuse_au_lieu_de_tronquer() {
        let variantes = (0..=VARIANTES_MAX)
            .map(|i| variante(&format!("0x{i}"), "FW", "Fire", [100; 7]))
            .collect();
        let e = comparaison(Json(DemandeComparaison {
            base: variante("0xA", "FW", "Fire", [100; 7]),
            variantes,
            noms_techniques: HashMap::new(),
        }))
        .await
        .unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("trop de variantes"), "{e}");
    }

    // ------------------------------------------------------------------------------------
    // Les builds
    // ------------------------------------------------------------------------------------

    #[tokio::test]
    async fn les_builds_sont_classes_par_puissance_decroissante() {
        let r = builds(Query(query(&[
            ("kc", "100"),
            ("cr", "100"),
            ("tc", "100"),
            ("pr", "100"),
            ("ps", "100"),
            ("ag", "100"),
            ("it", "100"),
        ])))
        .await
        .unwrap()
        .0;
        assert_eq!(r.builds.len(), 6);
        assert_eq!(r.base.total, 700);
        for paire in r.builds.windows(2) {
            assert!(
                paire[0].stats.total >= paire[1].stats.total,
                "classement rompu: {} avant {}",
                paire[0].stats.total,
                paire[1].stats.total
            );
        }
        // Le striker multiplie la frappe par 1,25 : 100 -> 125. Une valeur, pas un ordre.
        let striker = r
            .builds
            .iter()
            .find(|b| b.type_build == 1)
            .expect("build 1");
        assert_eq!(striker.stats.kc, 125);
        assert_eq!(striker.stats.ps, 80);
    }

    #[tokio::test]
    async fn builds_refuse_une_stat_absurde() {
        let e = builds(Query(query(&[("kc", "100000")]))).await.unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
    }

    // ------------------------------------------------------------------------------------
    // L'index et les divergences
    // ------------------------------------------------------------------------------------

    #[tokio::test]
    async fn les_capacites_comptent_les_vraies_tables() {
        let c = capacites().await.unwrap().0;
        // Comptes de `growth.rs` : m_growthTableLv1List = 36, Lv30 = 144, Main = 48, Sub = 48.
        assert_eq!(c.tables_croissance.lv1, 36);
        assert_eq!(c.tables_croissance.lv30, 144);
        assert_eq!(c.tables_croissance.main, 48);
        assert_eq!(c.tables_croissance.sub, 48);
        assert_eq!(c.statistiques.len(), 7);
        assert_eq!(c.builds.len(), 6);
        assert_eq!(c.classifications.len(), 7);
        assert_eq!(c.routes.len(), 6);
    }

    #[test]
    fn les_sept_classifications_sont_toutes_listees() {
        // Falsifiable : retirer une entree de CLASSIFICATIONS, ou faire rendre deux fois le
        // meme jeton au `match`, rougit ici.
        let jetons: std::collections::BTreeSet<&str> = CLASSIFICATIONS
            .iter()
            .map(|c| jeton_classification(*c))
            .collect();
        assert_eq!(jetons.len(), CLASSIFICATIONS.len());
        assert!(jetons.contains("evolution_serie"));
    }

    #[test]
    fn aucun_champ_public_ne_sort_par_debug() {
        // Le nom de variante Rust ne doit apparaitre nulle part dans la sortie.
        assert_ne!(
            jeton_classification(Classification::EvolutionHybride),
            format!("{:?}", Classification::EvolutionHybride)
        );
    }

    #[tokio::test]
    async fn les_quatre_divergences_sont_publiees_partout_ou_elles_s_appliquent() {
        let c = capacites().await.unwrap().0;
        assert_eq!(c.divergences.len(), 4);
        let jetons: Vec<&str> = c.divergences.iter().map(|d| d.jeton).collect();
        assert_eq!(
            jetons,
            [
                "libelles_position_inverses",
                "deux_echelles_de_rang",
                "asymetrie_void_synergie",
                "series_evolution_morte"
            ]
        );
        let contrat = contrat_comparaison().await.unwrap().0;
        assert_eq!(contrat.divergences.len(), 4);
        assert_eq!(contrat.methode, "POST");
    }

    #[tokio::test]
    async fn les_libelles_de_position_publies_sont_ceux_d_inagle_pas_ceux_du_re() {
        // Ce test GARDE la divergence n1 : inagle dit 2 = DF, le RE dit 2 = FW. Si quelqu'un
        // « corrige » la table amont, ce test rougit et l'arbitrage redevient visible.
        let c = capacites().await.unwrap().0;
        let deux = c
            .positions_inagle
            .iter()
            .find(|p| p.code == 2)
            .expect("code 2");
        assert_eq!(deux.libelle, "DF (Defender)");
        // Et la route de stats, elle, applique la convention du RE : position 2 resout bien une
        // entree de la table, dont growth.rs dit qu'elle est celle d'un FW.
        let s = stats(Query(query(&[("position", "2"), ("sous_position", "3")])))
            .await
            .unwrap()
            .0;
        assert!(s.tables_resolues);
    }
}
