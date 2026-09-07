//! La couche 3D du dépôt, servie par Aphrody — `/api/v1/3d/*` et `/model/*`.
//!
//! ## Ce que le dépôt sait faire en 3D, et qui le fait
//!
//! Trois crates se partagent le travail, et ce module est le seul endroit du site qui les
//! connaisse toutes les trois :
//!
//! | Étage | Crate | Ce qu'il produit |
//! |---|---|---|
//! | formats | `nie-formats` (`g4md`, `g4mg`, `g4sk`, `g4mt`, `g4pk`) | maillage, squelette, atlas |
//! | assemblage | `nie-model-serve` (amont, `127.0.0.1:8790`) | un **GLB** par code de modèle |
//! | rendu | `nie-render3d` | une **image** depuis un GLB, sans pilote graphique |
//!
//! L'assemblage reste chez l'amont : il lit les catalogues `chara_model`/`chara_parts` du jeu
//! pour savoir quelles pièces composent un personnage, et le réimplémenter ici créerait une
//! seconde recette qui dériverait de la première. Le site le **borne** (cf. [`super::assets`])
//! et le republie sous un espace de noms qui parle de modèles plutôt que de chemins d'amont.
//!
//! Le **rendu**, lui, n'existe nulle part ailleurs : l'amont sert des octets de GLB, il ne sait
//! pas en faire une image. `/model/{famille}/{code}.png` est donc du code de ce processus —
//! `nie_render3d::glb::parse` puis `nie_render3d::render::render`, le rastériseur CPU à
//! z-buffer qui sert de vérité terrain aux tests golden du moteur. C'est ce qui permet à une
//! grille de soixante modèles de s'afficher **sans un seul contexte WebGL** : le navigateur ne
//! reçoit que des PNG, et le viewport interactif n'est monté que pour le modèle qu'on regarde.
//!
//! ## Les six familles, et pourquoi elles ne se listent pas de la même façon
//!
//! Un « modèle » n'a pas d'identifiant unique dans le jeu : il a un **code**, et le sens du
//! code dépend de sa famille.
//!
//! - `perso` (`c…`) : le code n'est pas un dossier. Le corps, le visage et l'uniforme vivent
//!   dans trois arbres distincts, et c'est `chara_model_*.cfg.bin` qui les relie. La seule
//!   liste disponible ici est donc celle du **miroir** (`inagle_characters.internal_code`,
//!   tronqué à son préfixe : `c01000010_5000` et `c01000010` désignent le même modèle). Elle
//!   est *déclarée*, pas vérifiée : un code du miroir peut ne pas s'assembler, et l'amont
//!   répondra alors `404`.
//! - les cinq autres (`waza`, `item`, `animal`, `keshin`, `armd`) : un code = un sous-dossier
//!   de `data/common/chr/_<famille>`, et le critère d'assemblabilité est **mesurable depuis
//!   l'index** — la présence de `<code>/<code>.g4mg`. Vérifié sur l'amont : `_item/b000003`
//!   n'a qu'un `.g4sk` et un `.objbin`, et l'amont répond « G4MG … » en `404`. La liste servie
//!   ici est donc filtrée sur ce fichier, et elle ne propose que des modèles qui existent.
//!
//! Une seule famille par requête : mélanger une pagination SQL (le miroir) et une pagination
//! d'index (le VFS) obligerait à matérialiser les ~6 300 entrées à chaque appel pour les
//! retrancher ensuite.
//!
//! ## Le coût du rendu, et comment il est borné
//!
//! Rendre une image, c'est tirer le GLB de l'amont (jusqu'à quelques mégaoctets), décoder ses
//! atlas PNG et rastériser. C'est du travail **CPU**, donc :
//!
//! - il part en `spawn_blocking` — jamais sur un worker Tokio ;
//! - il passe par [`jetons_rendu`], un sémaphore propre au module, dimensionné sur le nombre de
//!   cœurs. Sans lui, une grille qui demande soixante vignettes d'un coup lancerait soixante
//!   rastérisations simultanées sur le pool bloquant (512 threads par défaut) et mettrait le
//!   service à genoux ;
//! - le résultat entre dans le **même cache** que les réponses d'amont, avec son ETag : une
//!   vignette n'est calculée qu'une fois par angle et par taille ;
//! - la taille demandée est plafonnée à [`TAILLE_RENDU_MAX`], l'angle est ramené au degré
//!   entier — sans quoi `?angle=0.0001` créerait une entrée de cache par requête.
//!
//! Le GLB tiré pour le rendu est mis en cache par [`super::assets::proxy`] sous **la même
//! clé** que celui servi à `/model/{famille}/{code}.glb` : regarder une vignette puis ouvrir le
//! viewport ne fait qu'un seul aller-retour vers l'amont.

use std::sync::{Arc, OnceLock};

use axum::Json;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use http_body_util::BodyExt as _;
use serde::{Deserialize, Serialize};

use crate::error::ErreurSite;
use crate::routes::static_files::{Encodage, etiquette, reponse_octets};
use crate::routes::{DemandePage, Page};
use crate::state::{EtatSite, ReponseCachee};
use crate::vfs_index::IndexVfs;

/// Racine des familles de modèles non-personnages dans le VFS.
pub const RACINE_CHR: &str = "data/common/chr";

/// Table du miroir dont sont tirés les codes de modèles de personnages.
///
/// La même que [`super::api_v1::TABLE_CHARA`] : c'est délibéré, il n'existe pas d'autre liste
/// de personnages, et en inventer une seconde ferait diverger le catalogue 3D du catalogue de
/// fiches.
pub const TABLE_CHARA: &str = super::api_v1::TABLE_CHARA;

/// La sous-requête qui rend les codes d'assemblage de la famille `perso`, une ligne du miroir
/// par variante. Écrite **une fois** : le compte de `/api/v1/3d` et la page de
/// `/api/v1/3d/modeles` doivent porter sur exactement le même ensemble, faute de quoi le
/// catalogue annoncerait un total qu'il ne sait pas parcourir.
///
/// Deux clauses, chacune payée d'un défaut mesuré le 2026-09-06 :
///
/// - `instr(internal_code, '_')` rend `0` quand le souligné est absent, et `substr(x, 1, -1)`
///   rendrait la chaîne vide : sans le `CASE`, tous les personnages **sans variante**
///   disparaissaient du catalogue ;
/// - `LIKE 'c%'` : `inagle_characters` ne porte pas que des personnages. Sur 5 721 codes
///   distincts, 66 commencent par `n`, `e`, `s`, `k`, `a` ou `i` — des animaux (`an…`) et des
///   entrées hors modèle. `/model-full` n'accepte que `c`, `k` et `ka`, et le catalogue
///   proposait donc des vignettes qui répondaient toutes `404`. Les keshin ont leur propre
///   famille, lue sur le VFS et vérifiée : cette liste-ci est celle des `c…`, et rien d'autre.
const SOURCE_PERSO: &str = "SELECT CASE WHEN instr(internal_code, '_') > 0 \
     THEN substr(internal_code, 1, instr(internal_code, '_') - 1) \
     ELSE internal_code END AS code, \
     coalesce(nullif(name_fr, ''), nullif(name_en, ''), nullif(name_ja, '')) AS nom \
     FROM \"inagle_characters\" \
     WHERE internal_code IS NOT NULL AND internal_code <> '' AND internal_code LIKE 'c%'";

/// Côté d'un aperçu quand la requête n'en demande pas. 320 px : la taille d'une carte de la
/// grille sur un écran à densité double, donc le cas majoritaire, donc celui qui doit être en
/// cache.
pub const TAILLE_RENDU_DEFAUT: u32 = 320;

/// Côté maximal d'un aperçu. Le rastériseur est en O(pixels) : 1024² est déjà quatre fois le
/// coût d'une vignette, et rien dans l'interface n'affiche plus grand.
pub const TAILLE_RENDU_MAX: u32 = 1024;

/// Côté minimal d'un aperçu — en deçà, le modèle n'est plus reconnaissable et la requête ne
/// peut être qu'une erreur de calcul côté client.
pub const TAILLE_RENDU_MIN: u32 = 32;

/// `Cache-Control` des aperçus et des GLB : le rendu d'un code à un angle donné est
/// déterministe, mais la recette d'assemblage de l'amont évolue (`ASSEMBLER_VERSION`).
pub const CONTROLE: &str = "public, max-age=3600, stale-while-revalidate=86400";

/// Nombre maximal de rastérisations simultanées, faute de mieux quand le nombre de cœurs est
/// indisponible.
const RENDUS_SIMULTANES_DEFAUT: usize = 4;

/// Le sémaphore des rendus, dimensionné une seule fois sur le parallélisme réel de la machine.
///
/// Il vit dans le module plutôt que dans [`EtatSite`] parce qu'il ne borne pas une ressource du
/// service (l'amont a déjà le sien) mais **le CPU de la machine**, qui est partagé par tous les
/// états, y compris ceux qu'un test construit.
fn jetons_rendu() -> &'static tokio::sync::Semaphore {
    static JETONS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    JETONS.get_or_init(|| {
        let n = std::thread::available_parallelism()
            .map_or(RENDUS_SIMULTANES_DEFAUT, std::num::NonZeroUsize::get);
        tokio::sync::Semaphore::new(n.max(1))
    })
}

/// Une famille de modèles : ce qui décide d'où vient la liste et quelle route de l'amont
/// assemble le GLB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Famille {
    /// Les personnages jouables et non jouables (`c…`), assemblés depuis leurs catalogues.
    Perso,
    /// Les modèles de techniques et de cut-in (`data/common/chr/_waza`).
    Waza,
    /// Les objets et accessoires 3D (`data/common/chr/_item`).
    Item,
    /// Les animaux (`data/common/chr/_animal`).
    Animal,
    /// Les keshin (`data/common/chr/_keshin`).
    Keshin,
    /// Les armures (`data/common/chr/_armd`).
    Armd,
}

/// Les six familles, dans l'ordre où elles sont exposées.
pub const FAMILLES: [Famille; 6] = [
    Famille::Perso,
    Famille::Waza,
    Famille::Item,
    Famille::Animal,
    Famille::Keshin,
    Famille::Armd,
];

impl Famille {
    /// Segment d'URL de la famille.
    #[must_use]
    pub fn segment(self) -> &'static str {
        match self {
            Self::Perso => "perso",
            Self::Waza => "waza",
            Self::Item => "item",
            Self::Animal => "animal",
            Self::Keshin => "keshin",
            Self::Armd => "armd",
        }
    }

    /// Libellé français, tel que l'interface l'affiche.
    #[must_use]
    pub fn libelle(self) -> &'static str {
        match self {
            Self::Perso => "Personnages",
            Self::Waza => "Techniques",
            Self::Item => "Objets",
            Self::Animal => "Animaux",
            Self::Keshin => "Keshin",
            Self::Armd => "Armures",
        }
    }

    /// Sous-dossier de [`RACINE_CHR`] qui porte les codes de la famille, `None` pour `perso`
    /// dont les pièces ne vivent pas dans un dossier par code.
    #[must_use]
    pub fn sous_dossier(self) -> Option<&'static str> {
        match self {
            Self::Perso => None,
            Self::Waza => Some("_waza"),
            Self::Item => Some("_item"),
            Self::Animal => Some("_animal"),
            Self::Keshin => Some("_keshin"),
            Self::Armd => Some("_armd"),
        }
    }

    /// Préfixe VFS complet de la famille, `None` pour `perso`.
    #[must_use]
    pub fn dossier_vfs(self) -> Option<String> {
        self.sous_dossier().map(|s| format!("{RACINE_CHR}/{s}"))
    }

    /// D'où vient la liste des codes : le miroir SQLite, ou l'index du VFS.
    #[must_use]
    pub fn source(self) -> &'static str {
        match self {
            Self::Perso => "miroir",
            _ => "vfs",
        }
    }

    /// Reconnaît une famille depuis son segment d'URL.
    #[must_use]
    pub fn depuis_segment(s: &str) -> Option<Self> {
        FAMILLES.into_iter().find(|f| f.segment() == s)
    }

    /// Chemin d'amont qui assemble le GLB de ce code, relatif à la base de `nie-model-serve`.
    ///
    /// Deux routes distinctes chez l'amont, et elles ne sont pas interchangeables :
    /// `/model-full` part du code de personnage et remonte ses catalogues, `/model-chr` part
    /// d'un couple (sous-domaine, code) et lit directement la paire `g4md`/`g4mg`.
    #[must_use]
    pub fn chemin_amont(self, code: &str) -> String {
        match self.sous_dossier() {
            None => format!("model-full/{code}.glb"),
            // Le sous-domaine attendu par `/model-chr` est le nom du dossier SANS son
            // souligné : `_waza` est un dossier, `waza` est un sous-domaine.
            Some(dossier) => format!("model-chr/{}/{code}.glb", dossier.trim_start_matches('_')),
        }
    }
}

/// Dit si un code de modèle est acceptable — même grammaire que le validateur de l'amont.
///
/// Le code entre dans une URL d'amont : sans cette borne, un `..%2f` s'y promènerait. Elle est
/// posée ici plutôt que déléguée, parce qu'un chemin refusé par l'amont revient en `404`
/// indistinguable d'un modèle absent.
#[must_use]
pub fn code_valide(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 32
        && code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Un modèle du catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Modele {
    /// Code d'assemblage — l'identité du modèle, et le seul segment adressable.
    pub code: String,
    /// Famille d'appartenance.
    pub famille: Famille,
    /// Nom affichable quand le jeu en donne un. `null` pour tout ce qui n'est pas un
    /// personnage : les props et les décors n'ont pas de nom dans les données, et en inventer
    /// un serait une invention.
    pub nom: Option<String>,
    /// Nombre de fichiers dans le dossier du modèle, `null` pour `perso` (dont les pièces sont
    /// éclatées entre corps, visage et uniforme, donc non dénombrables par dossier).
    pub fichiers: Option<usize>,
    /// URL du GLB assemblé.
    pub glb: String,
    /// URL d'un aperçu PNG rendu par `nie-render3d`.
    pub apercu: String,
}

impl Modele {
    /// Construit l'entrée, URL comprises, à partir du couple famille/code.
    fn nouveau(
        famille: Famille,
        code: String,
        nom: Option<String>,
        fichiers: Option<usize>,
    ) -> Self {
        let base = format!("/model/{}/{code}", famille.segment());
        Self {
            glb: format!("{base}.glb"),
            apercu: format!("{base}.png"),
            code,
            famille,
            nom,
            fichiers,
        }
    }
}

/// Une famille, telle que `/api/v1/3d` la décrit.
#[derive(Debug, Clone, Serialize)]
pub struct FamilleResume {
    /// Segment d'URL.
    pub segment: &'static str,
    /// Libellé français.
    pub libelle: &'static str,
    /// `miroir` ou `vfs` — d'où vient la liste des codes.
    pub source: &'static str,
    /// Dossier VFS des codes, `null` pour `perso`.
    pub dossier: Option<String>,
    /// Nombre de modèles retenus, `null` quand la source n'est pas disponible.
    pub total: Option<usize>,
    /// Vrai quand chaque code de la liste a été **vérifié** présent dans le VFS ; faux quand la
    /// liste est seulement déclarée (cas de `perso`, lu au miroir).
    pub verifie: bool,
}

/// Le moteur de rendu, tel que le service le décrit.
#[derive(Debug, Clone, Serialize)]
pub struct MoteurRendu {
    /// Crate qui rastérise.
    pub crate_: &'static str,
    /// Chemin de rendu employé ici.
    pub chemin: &'static str,
    /// Focale de la projection de référence (`nie_render3d::render::FOCALE`).
    pub focale: f32,
    /// Distance de la caméra, en rayons de la sphère englobante.
    pub distance: f32,
    /// Inclinaison verticale de la vue, en radians.
    pub tilt: f32,
    /// Côté par défaut d'un aperçu.
    pub taille_defaut: u32,
    /// Côté maximal accepté.
    pub taille_max: u32,
    /// Rastérisations simultanées autorisées sur cette machine.
    pub simultanes: usize,
}

/// Corps de `/api/v1/3d`.
#[derive(Debug, Clone, Serialize)]
pub struct Capacites3d {
    /// Toujours `nie-site`.
    pub service: &'static str,
    /// Version de la crate.
    pub version: &'static str,
    /// Base de l'amont qui assemble les GLB.
    pub amont: String,
    /// Vrai quand l'index du VFS est prêt : sans lui, les cinq familles de fichiers sont vides.
    pub vfs_pret: bool,
    /// Vrai quand le miroir est présent : sans lui, la famille `perso` est vide.
    pub miroir_present: bool,
    /// Le rendu natif et ses conventions de caméra.
    pub moteur: MoteurRendu,
    /// Les six familles et ce que chacune compte.
    pub familles: Vec<FamilleResume>,
    /// Extensions du jeu qui entrent dans un modèle assemblé.
    pub formats_sources: [&'static str; 7],
    /// Types de contenu que cet espace produit.
    pub sorties: [&'static str; 3],
}

/// Demande adressée à `/api/v1/3d/modeles` : la pagination commune, plus la famille.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DemandeCatalogue {
    /// Famille demandée. Absente, c'est `perso` — la seule qui porte des noms.
    pub famille: Option<String>,
    /// Numéro de page, à partir de 1.
    pub page: Option<u32>,
    /// Taille de page, plafonnée par [`crate::config::PER_PAGE_MAX`].
    pub per_page: Option<u32>,
    /// Motif de recherche, comparé sans casse au code **et** au nom.
    pub q: Option<String>,
}

impl DemandeCatalogue {
    /// La pagination bornée de cette demande.
    fn pagination(&self) -> crate::config::Pagination {
        DemandePage {
            page: self.page,
            per_page: self.per_page,
            q: self.q.clone(),
        }
        .bornee()
    }

    /// La famille demandée, ou l'erreur qui nomme les six possibles.
    fn famille(&self) -> Result<Famille, ErreurSite> {
        match self
            .famille
            .as_deref()
            .map(str::trim)
            .filter(|f| !f.is_empty())
        {
            None => Ok(Famille::Perso),
            Some(f) => Famille::depuis_segment(f).ok_or_else(|| {
                ErreurSite::Introuvable(format!(
                    "famille inconnue: {f} (connues: {})",
                    FAMILLES.map(Famille::segment).join(", ")
                ))
            }),
        }
    }

    /// Le motif de recherche, réduit en minuscules, vide écarté.
    fn motif(&self) -> Option<String> {
        self.q
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(str::to_lowercase)
    }
}

/// Les codes d'une famille de fichiers, lus sur l'index du VFS.
///
/// Un code est retenu **si et seulement si** `<dossier>/<code>/<code>.g4mg` est indexé : c'est
/// le fichier de géométrie, et l'amont échoue sans lui. Le filtre est une recherche
/// dichotomique par candidat, pas un parcours — l'index est trié.
#[must_use]
pub fn codes_vfs(index: &IndexVfs, famille: Famille) -> Vec<Modele> {
    let Some(dossier) = famille.dossier_vfs() else {
        return Vec::new();
    };
    // `limite = 0` : on ne veut que la liste des sous-dossiers et les totaux, jamais les
    // fichiers directs (il n'y en a pas à ce niveau).
    let racine = index.dossier(&dossier, 0, 0);
    racine
        .dossiers
        .iter()
        .filter_map(|chemin| {
            let code = chemin.rsplit('/').next()?;
            if !code_valide(code) {
                return None;
            }
            if !index.contient(&format!("{chemin}/{code}.g4mg")) {
                return None;
            }
            let fichiers = index.dossier(chemin, 0, 0).total_fichiers;
            Some(Modele::nouveau(
                famille,
                code.to_owned(),
                None,
                Some(fichiers),
            ))
        })
        .collect()
}

/// Restreint une liste de modèles à ceux dont le code ou le nom contient `motif`.
fn filtrer(modeles: Vec<Modele>, motif: Option<&str>) -> Vec<Modele> {
    let Some(motif) = motif else {
        return modeles;
    };
    modeles
        .into_iter()
        .filter(|m| {
            m.code.to_lowercase().contains(motif)
                || m.nom
                    .as_deref()
                    .is_some_and(|n| n.to_lowercase().contains(motif))
        })
        .collect()
}

/// Une page de la famille `perso`, lue au miroir.
///
/// Le code d'assemblage est le préfixe d'`internal_code` : les variantes d'un personnage
/// (`c01000010`, `c01000010_5000`, …) partagent un seul modèle, et les lister toutes
/// afficherait la même silhouette des dizaines de fois. Le regroupement est fait par SQL —
/// `GROUP BY` sur le préfixe — pour que la pagination porte sur les codes **distincts** et non
/// sur les lignes.
fn page_perso(
    gisement: &crate::dataset::Gisement,
    p: crate::config::Pagination,
    motif: Option<&str>,
) -> Result<Page<Modele>, ErreurSite> {
    gisement.lire(|c| {
        let source = SOURCE_PERSO;
        // Le motif est comparé au code ET au nom, comme le filtre des autres familles. `?1`
        // vaut `%%` quand aucun motif n'est demandé : `LIKE '%%'` retient tout, ce qui évite
        // deux requêtes distinctes pour une seule différence de clause.
        let motif_sql = motif.map_or_else(|| "%".to_owned(), |m| format!("%{m}%"));
        let filtre = "WHERE lower(s.code) LIKE ?1 OR lower(coalesce(s.nom, '')) LIKE ?1";

        let total: i64 = c.query_row(
            &format!(
                "SELECT count(*) FROM (SELECT s.code FROM ({source}) s {filtre} GROUP BY s.code)"
            ),
            rusqlite::params![&motif_sql],
            |r| r.get(0),
        )?;

        // `min(s.nom)` : plusieurs lignes partagent un code et peuvent porter des noms
        // différents (variantes de tenue). Le minimum lexicographique est arbitraire mais
        // STABLE — un `GROUP BY` sans agrégat rendrait une ligne au hasard, donc un nom qui
        // change d'une requête à l'autre.
        let mut stmt = c.prepare(&format!(
            "SELECT s.code, min(s.nom) FROM ({source}) s {filtre} \
             GROUP BY s.code ORDER BY s.code LIMIT ?2 OFFSET ?3"
        ))?;
        let lignes = stmt
            .query_map(
                rusqlite::params![&motif_sql, i64::from(p.per_page), p.offset() as i64],
                |r| {
                    let code: String = r.get(0)?;
                    let nom: Option<String> = r.get(1)?;
                    Ok((code, nom))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let elements = lignes
            .into_iter()
            .filter(|(code, _)| code_valide(code))
            .map(|(code, nom)| Modele::nouveau(Famille::Perso, code, nom, None))
            .collect();
        Ok(Page::nouvelle(
            elements,
            p,
            usize::try_from(total).unwrap_or(0),
        ))
    })
}

/// `GET /api/v1/3d` — ce que cette machine sait faire en 3D, **mesuré**.
///
/// Chaque total est recompté à la demande : un index VFS qui finit de se monter ou un miroir
/// qui bascule dans la nuit changent ces nombres, et un compte figé au démarrage annoncerait
/// des familles vides sur un service parfaitement fonctionnel.
pub async fn capacites(State(etat): State<EtatSite>) -> Json<Capacites3d> {
    let index = etat.index().ok();
    let gisement = Arc::clone(&etat.gisement);
    let total_perso = tokio::task::spawn_blocking(move || {
        gisement
            .lire(|c| {
                let n: i64 = c.query_row(
                    &format!("SELECT count(DISTINCT s.code) FROM ({SOURCE_PERSO}) s"),
                    [],
                    |r| r.get(0),
                )?;
                Ok(usize::try_from(n).unwrap_or(0))
            })
            .ok()
    })
    .await
    .unwrap_or(None);

    let familles = FAMILLES
        .into_iter()
        .map(|f| FamilleResume {
            segment: f.segment(),
            libelle: f.libelle(),
            source: f.source(),
            dossier: f.dossier_vfs(),
            total: match f {
                Famille::Perso => total_perso,
                _ => index.as_ref().map(|i| codes_vfs(i, f).len()),
            },
            verifie: f.sous_dossier().is_some(),
        })
        .collect();

    Json(Capacites3d {
        service: crate::SERVICE,
        version: crate::VERSION,
        amont: etat.config.amont.clone(),
        vfs_pret: index.is_some(),
        miroir_present: etat.gisement.present(),
        moteur: MoteurRendu {
            crate_: "nie-render3d",
            chemin: "CPU z-buffer (rasterisation logicielle, sans pilote graphique)",
            focale: nie_render3d::render::FOCALE,
            distance: nie_render3d::render::DISTANCE_CAMERA,
            tilt: nie_render3d::render::TILT,
            taille_defaut: TAILLE_RENDU_DEFAUT,
            taille_max: TAILLE_RENDU_MAX,
            simultanes: jetons_rendu().available_permits().max(1),
        },
        familles,
        formats_sources: ["g4md", "g4mg", "g4sk", "g4mt", "g4pk", "g4pkm", "g4tx"],
        sorties: ["model/gltf-binary", "image/png", "application/json"],
    })
}

/// `GET /api/v1/3d/modeles` — une page du catalogue d'une famille.
///
/// # Errors
///
/// `Introuvable` sur famille inconnue, `Indisponible` quand la source de la famille (index VFS
/// ou miroir) n'est pas encore là.
pub async fn catalogue(
    State(etat): State<EtatSite>,
    Query(demande): Query<DemandeCatalogue>,
) -> Result<Json<Page<Modele>>, ErreurSite> {
    let famille = demande.famille()?;
    let p = demande.pagination();
    let motif = demande.motif();

    if famille == Famille::Perso {
        let gisement = Arc::clone(&etat.gisement);
        let page = tokio::task::spawn_blocking(move || page_perso(&gisement, p, motif.as_deref()))
            .await??;
        return Ok(Json(page));
    }

    let index = etat.index()?;
    // Le parcours de l'index est borné par le sous-arbre de la famille (au plus quelques
    // milliers de chemins) : il ne justifie pas un `spawn_blocking`, dont le coût de
    // commutation serait du même ordre que le travail.
    let tous = filtrer(codes_vfs(&index, famille), motif.as_deref());
    let total = tous.len();
    let elements = tous
        .into_iter()
        .skip(p.offset())
        .take(p.per_page as usize)
        .collect();
    Ok(Json(Page::nouvelle(elements, p, total)))
}

/// Une pièce du modèle, telle que l'index du VFS la connaît.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Piece {
    /// Chemin VFS verbatim — c'est aussi son URL sous `/f/`.
    pub chemin: String,
    /// Nom du fichier, extension du jeu conservée.
    pub nom: String,
    /// Extension, en minuscules et sans point.
    pub extension: String,
    /// Taille déclarée par l'index du jeu, en octets.
    pub taille: u32,
}

/// Corps de `/api/v1/3d/modeles/{famille}/{code}`.
#[derive(Debug, Clone, Serialize)]
pub struct FicheModele {
    /// Le modèle et ses URL.
    pub modele: Modele,
    /// Les pièces trouvées dans le VFS. Vide pour `perso`, dont les pièces ne vivent pas dans
    /// un dossier par code — c'est `chara_model` qui les relie, et seul l'amont le lit.
    pub pieces: Vec<Piece>,
    /// Somme des tailles des pièces, en octets.
    pub pieces_octets: u64,
    /// L'identité du personnage quand le miroir la connaît, `null` sinon.
    pub identite: Option<IdentiteModele>,
    /// Le rapport d'assemblage de l'amont, demandé par `?rapport=1`.
    ///
    /// Il n'est **pas** rendu par défaut : le produire déclenche un assemblage complet chez
    /// l'amont (mesuré à 2,4 s à froid pour un personnage), et une fiche doit pouvoir
    /// s'afficher instantanément.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rapport: Option<serde_json::Value>,
}

/// Ce que le miroir sait d'un code de personnage.
#[derive(Debug, Clone, Serialize)]
pub struct IdentiteModele {
    /// Nom français.
    pub nom_fr: Option<String>,
    /// Nom anglais.
    pub nom_en: Option<String>,
    /// Nom japonais.
    pub nom_ja: Option<String>,
    /// Élément.
    pub element: Option<String>,
    /// Poste.
    pub position: Option<String>,
    /// Série d'origine.
    pub serie: Option<String>,
    /// Nombre de lignes du miroir qui partagent ce code d'assemblage — c'est le nombre de
    /// variantes (tenues, raretés) du personnage, pas un doublon.
    pub variantes: usize,
}

/// Réglages d'une fiche.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DemandeFiche {
    /// `1` pour joindre le rapport d'assemblage de l'amont.
    pub rapport: Option<String>,
}

/// `GET /api/v1/3d/modeles/{famille}/{code}` — la fiche d'un modèle.
///
/// # Errors
///
/// `Introuvable` sur famille ou code inconnu, `Demande` sur code mal formé, `Indisponible`
/// quand l'index n'est pas monté.
pub async fn fiche(
    State(etat): State<EtatSite>,
    Path((famille, code)): Path<(String, String)>,
    Query(demande): Query<DemandeFiche>,
) -> Result<Json<FicheModele>, ErreurSite> {
    let (famille, code) = valider(&famille, &code)?;

    let mut pieces = Vec::new();
    let mut pieces_octets = 0u64;
    if let Some(dossier) = famille.dossier_vfs() {
        let index = etat.index()?;
        let d = index.dossier(&format!("{dossier}/{code}"), 0, usize::MAX);
        if d.total_fichiers == 0 {
            return Err(ErreurSite::Introuvable(format!(
                "modele {}/{code} absent du VFS",
                famille.segment()
            )));
        }
        for f in d.fichiers {
            pieces_octets += u64::from(f.taille);
            pieces.push(Piece {
                extension: f
                    .nom
                    .rsplit_once('.')
                    .map_or_else(String::new, |(_, e)| e.to_lowercase()),
                nom: f.nom,
                chemin: f.chemin,
                taille: f.taille,
            });
        }
    }

    let identite = if famille == Famille::Perso {
        let gisement = Arc::clone(&etat.gisement);
        let cle = code.clone();
        tokio::task::spawn_blocking(move || identite_perso(&gisement, &cle).ok().flatten()).await?
    } else {
        None
    };
    if famille == Famille::Perso && identite.is_none() {
        return Err(ErreurSite::Introuvable(format!(
            "code {code} inconnu du miroir des personnages"
        )));
    }

    let nom = identite.as_ref().and_then(|i| {
        i.nom_fr
            .clone()
            .or_else(|| i.nom_en.clone())
            .or_else(|| i.nom_ja.clone())
    });
    let fichiers = (!pieces.is_empty()).then_some(pieces.len());

    // Le rapport est explicitement demandé : un amont muet ne doit pas faire échouer la fiche,
    // qui reste utile sans lui. On rend `null`, pas une erreur.
    let rapport = if demande.rapport.as_deref() == Some("1") {
        let chemin = format!("model-report/{code}.json");
        match octets_amont(&etat, &chemin).await {
            Ok(octets) => serde_json::from_slice(&octets).ok(),
            Err(e) => {
                tracing::debug!(erreur = %e, code = %code, "rapport d'amont indisponible");
                None
            }
        }
    } else {
        None
    };

    Ok(Json(FicheModele {
        modele: Modele::nouveau(famille, code, nom, fichiers),
        pieces,
        pieces_octets,
        identite,
        rapport,
    }))
}

/// L'identité d'un code de personnage, lue au miroir.
fn identite_perso(
    gisement: &crate::dataset::Gisement,
    code: &str,
) -> Result<Option<IdentiteModele>, ErreurSite> {
    gisement.lire(|c| {
        // `internal_code = ?1 OR internal_code LIKE ?1 || '\_%'` : le code d'assemblage est un
        // PRÉFIXE, et le `_` de SQL est un joker d'un caractère. Sans l'échappement, `c0100001`
        // ramènerait aussi `c01000010` — un personnage voisin, silencieusement.
        let mut stmt = c.prepare(&format!(
            "SELECT name_fr, name_en, name_ja, element, position, series, count(*) \
             FROM \"{TABLE_CHARA}\" \
             WHERE internal_code = ?1 OR internal_code LIKE ?1 || '\\_%' ESCAPE '\\'"
        ))?;
        let ligne = stmt.query_row(rusqlite::params![code], |r| {
            let variantes: i64 = r.get(6)?;
            Ok((
                IdentiteModele {
                    nom_fr: r.get(0)?,
                    nom_en: r.get(1)?,
                    nom_ja: r.get(2)?,
                    element: r.get(3)?,
                    position: r.get(4)?,
                    serie: r.get(5)?,
                    variantes: usize::try_from(variantes).unwrap_or(0),
                },
                variantes,
            ))
        });
        match ligne {
            // `count(*)` sur un ensemble vide rend une ligne à 0 : c'est ce zéro, et non
            // l'absence de ligne, qui dit que le code est inconnu.
            Ok((_, 0)) => Ok(None),
            Ok((identite, _)) => Ok(Some(identite)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    })
}

/// Les dimensions d'une texture embarquée dans le GLB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TextureAnalysee {
    /// Largeur en pixels.
    pub largeur: u32,
    /// Hauteur en pixels.
    pub hauteur: u32,
}

/// La boîte englobante du modèle, dans les unités du jeu (mètres).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Boite {
    /// Coin minimal.
    pub min: [f32; 3],
    /// Coin maximal.
    pub max: [f32; 3],
    /// Centre, tel que le rastériseur le calcule.
    pub centre: [f32; 3],
    /// Rayon de la sphère englobante — c'est lui qui normalise la vue.
    pub rayon: f32,
}

/// Corps de `/api/v1/3d/modeles/{famille}/{code}/analyse`.
#[derive(Debug, Clone, Serialize)]
pub struct Analyse {
    /// Code analysé.
    pub code: String,
    /// Famille d'appartenance.
    pub famille: Famille,
    /// Taille du GLB tiré de l'amont, en octets.
    pub glb_octets: usize,
    /// Nombre de primitives (un groupe de triangles partageant un matériau).
    pub primitives: usize,
    /// Nombre de primitives sans texture — elles sont rendues en argile par le rastériseur.
    pub primitives_sans_texture: usize,
    /// Nombre total de sommets.
    pub sommets: usize,
    /// Nombre total de triangles.
    pub triangles: usize,
    /// Les atlas embarqués, dans l'ordre du GLB.
    pub textures: Vec<TextureAnalysee>,
    /// Somme des texels de tous les atlas — la mémoire que le rendu doit tenir résidente.
    pub texels: u64,
    /// Boîte englobante.
    pub boite: Boite,
}

/// `GET /api/v1/3d/modeles/{famille}/{code}/analyse` — la géométrie réelle du GLB.
///
/// C'est la mesure que l'amont ne rend pas : son rapport décrit la **recette** (quelles lignes
/// de quel catalogue ont été suivies), pas le résultat. Ici, le GLB est parsé par le même code
/// que celui qui le rastérise — ce qui est compté est donc exactement ce qui est dessiné.
///
/// # Errors
///
/// `Introuvable` sur code inconnu de l'amont, `Amont`/`Delai` quand l'assemblage échoue,
/// `Interne` quand le GLB est illisible.
pub async fn analyse(
    State(etat): State<EtatSite>,
    Path((famille, code)): Path<(String, String)>,
) -> Result<Json<Analyse>, ErreurSite> {
    let (famille, code) = valider(&famille, &code)?;
    let octets = octets_amont(&etat, &famille.chemin_amont(&code)).await?;
    let glb_octets = octets.len();

    let _jeton = jeton_rendu().await?;
    let code_tache = code.clone();
    let mesure = tokio::task::spawn_blocking(move || {
        let modele = charger_glb(&octets, &code_tache)?;
        Ok::<_, ErreurSite>(mesurer(&modele))
    })
    .await??;

    Ok(Json(Analyse {
        code,
        famille,
        glb_octets,
        ..mesure
    }))
}

/// Mesure un modèle chargé. Séparée du handler pour être testable sans HTTP ni amont.
fn mesurer(modele: &nie_render3d::glb::Model) -> Analyse {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut sommets = 0usize;
    let mut triangles = 0usize;
    let mut sans_texture = 0usize;
    for p in &modele.primitives {
        sommets += p.positions.len();
        triangles += p.indices.len() / 3;
        if p.texture.is_none() {
            sans_texture += 1;
        }
        for v in &p.positions {
            for k in 0..3 {
                min[k] = min[k].min(v[k]);
                max[k] = max[k].max(v[k]);
            }
        }
    }
    // Un modèle sans un seul sommet laisserait les bornes à l'infini : le JSON porterait alors
    // `null` (serde n'encode pas l'infini) et le client lirait un champ manquant sans savoir
    // pourquoi. On rend un volume nul, qui se lit.
    if sommets == 0 {
        min = [0.0; 3];
        max = [0.0; 3];
    }
    let (centre, rayon) = nie_render3d::render::bounds(modele);
    Analyse {
        code: String::new(),
        famille: Famille::Perso,
        glb_octets: 0,
        primitives: modele.primitives.len(),
        primitives_sans_texture: sans_texture,
        sommets,
        triangles,
        textures: modele
            .textures
            .iter()
            .map(|t| TextureAnalysee {
                largeur: t.width,
                hauteur: t.height,
            })
            .collect(),
        texels: modele
            .textures
            .iter()
            .map(|t| u64::from(t.width) * u64::from(t.height))
            .sum(),
        boite: Boite {
            min,
            max,
            centre,
            rayon,
        },
    }
}

/// Réglages d'un aperçu PNG.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DemandeApercu {
    /// Angle de rotation autour de l'axe vertical, en **degrés**. Degrés et non radians : c'est
    /// une URL, elle est lue et écrite à la main.
    pub angle: Option<f32>,
    /// Largeur en pixels.
    pub l: Option<u32>,
    /// Hauteur en pixels. Absente, elle vaut la largeur.
    pub h: Option<u32>,
}

impl DemandeApercu {
    /// Ramène la demande dans ses bornes : angle au degré entier de `[0, 360)`, côtés dans
    /// `[TAILLE_RENDU_MIN, TAILLE_RENDU_MAX]`.
    ///
    /// L'angle est **quantifié** parce qu'il entre dans la clé de cache : au centième de degré,
    /// une caméra qui tourne créerait une entrée par image et le cache ne servirait jamais.
    #[must_use]
    fn bornee(&self) -> (i32, u32, u32) {
        let brut = self.angle.filter(|a| a.is_finite()).unwrap_or(0.0);
        let degres = brut.rem_euclid(360.0).round();
        // `rem_euclid` sur un f32 fini rend `[0, 360)`, `round()` peut le porter à 360 :
        // 360 et 0 sont le même angle, et deux clés de cache pour une seule image.
        let degres = if degres >= 360.0 { 0 } else { degres as i32 };
        let l = self
            .l
            .unwrap_or(TAILLE_RENDU_DEFAUT)
            .clamp(TAILLE_RENDU_MIN, TAILLE_RENDU_MAX);
        let h = self
            .h
            .unwrap_or(l)
            .clamp(TAILLE_RENDU_MIN, TAILLE_RENDU_MAX);
        (degres, l, h)
    }
}

/// `GET /model/{famille}/{fichier}` — le modèle, sous la forme que le suffixe demande.
///
/// Deux suffixes, deux natures, une seule route : `.glb` rend les octets assemblés par l'amont
/// (relayés par [`super::assets::proxy`], donc avec son cache, son ETag et ses bornes), `.png`
/// rend une image produite **ici** par `nie-render3d`. Les séparer en deux routes aurait
/// dupliqué la validation du couple famille/code sans rien clarifier : c'est le même modèle,
/// demandé dans deux formats.
///
/// L'exemple est `/pet/frame/{animation}/{fichier}` : le suffixe vit dans le handler, pas dans
/// le motif de route, parce qu'axum capture un segment entier.
pub async fn modele(
    State(etat): State<EtatSite>,
    Path((famille, fichier)): Path<(String, String)>,
    Query(demande): Query<DemandeApercu>,
    entetes: HeaderMap,
) -> Response {
    let resultat = if let Some(code) = fichier.strip_suffix(".glb") {
        glb(&etat, &famille, code, entetes).await
    } else if let Some(code) = fichier.strip_suffix(".png") {
        apercu(&etat, &famille, code, &demande, &entetes).await
    } else {
        Err(ErreurSite::Introuvable(format!(
            "forme attendue: <code>.glb ou <code>.png (recu: {fichier})"
        )))
    };
    resultat.unwrap_or_else(IntoResponse::into_response)
}

/// Le GLB assemblé, relayé depuis l'amont sans en réimplémenter le décodage.
async fn glb(
    etat: &EtatSite,
    famille: &str,
    code: &str,
    entetes: HeaderMap,
) -> Result<Response, ErreurSite> {
    let (famille, code) = valider(famille, code)?;
    super::assets::proxy(
        State(etat.clone()),
        Path(famille.chemin_amont(&code)),
        RawQuery(None),
        entetes,
    )
    .await
}

/// L'aperçu PNG, rendu par le rastériseur CPU.
async fn apercu(
    etat: &EtatSite,
    famille: &str,
    code: &str,
    demande: &DemandeApercu,
    entetes: &HeaderMap,
) -> Result<Response, ErreurSite> {
    let (famille, code) = valider(famille, code)?;
    let (degres, l, h) = demande.bornee();
    let cle = format!("rendu3d:{}/{code}@{degres}x{l}x{h}", famille.segment());
    if let Some(cachee) = etat.cache.get(&cle).await {
        return Ok(reponse_octets(
            &cachee,
            CONTROLE,
            Encodage::Identite,
            entetes,
        ));
    }

    let octets = octets_amont(etat, &famille.chemin_amont(&code)).await?;
    let _jeton = jeton_rendu().await?;
    let code_tache = code.clone();
    let png = tokio::task::spawn_blocking(move || {
        let modele = charger_glb(&octets, &code_tache)?;
        // Le rastériseur prend des radians ; l'URL parle en degrés.
        let angle = (degres as f32).to_radians();
        let rgba = nie_render3d::render::render(&modele, angle, l, h);
        // L'encodeur PNG du dépôt, celui qui sert déjà les frames d'Aphrody. En ajouter un
        // second ferait cohabiter deux réglages de compression pour un seul format.
        nie_aphrody::assets::encoder_png(&rgba, l, h)
            .map_err(|e| ErreurSite::Interne(format!("encodage PNG: {e}")))
    })
    .await??;

    let cachee = ReponseCachee {
        etag: etiquette(&png),
        type_contenu: "image/png".to_owned(),
        corps: bytes::Bytes::from(png),
    };
    etat.cache.insert(cle, cachee.clone()).await;
    Ok(reponse_octets(
        &cachee,
        CONTROLE,
        Encodage::Identite,
        entetes,
    ))
}

/// Valide le couple famille/code venu de l'URL.
fn valider(famille: &str, code: &str) -> Result<(Famille, String), ErreurSite> {
    let f = Famille::depuis_segment(famille).ok_or_else(|| {
        ErreurSite::Introuvable(format!(
            "famille inconnue: {famille} (connues: {})",
            FAMILLES.map(Famille::segment).join(", ")
        ))
    })?;
    if !code_valide(code) {
        return Err(ErreurSite::Demande(format!(
            "code de modele invalide: {code}"
        )));
    }
    Ok((f, code.to_owned()))
}

/// Charge un GLB déjà tiré, en classant correctement son éventuel refus.
///
/// Un GLB que `nie_render3d::glb::parse` refuse n'est **pas** un défaut de ce service : c'est
/// un artefact que l'amont a produit et que le parseur du dépôt juge malformé. Le rendre en
/// `500` accuserait le site et enverrait le lecteur de journaux chercher un bug qui n'est pas
/// ici ; `502` dit d'où vient la pièce fautive.
///
/// Le cas existe et il est mesuré (2026-09-06) : `/model-chr/keshin/k000010.glb` porte des
/// indices de sommet **globaux** (jusqu'à 11 493) alors que ses accesseurs `POSITION` sont
/// **locaux** à chaque primitive (818, 858, 2 394 sommets) — l'assemblage keshin ne rebase pas
/// ses indices par primitive. Sur huit keshin échantillonnés, un seul est dans ce cas ; les
/// personnages, les techniques, les animaux et les armures testés sont tous conformes
/// (`maxidx == count - 1`). Le correctif appartient à `nie-model-serve`, pas ici.
fn charger_glb(octets: &[u8], code: &str) -> Result<nie_render3d::glb::Model, ErreurSite> {
    nie_render3d::glb::parse(octets).map_err(|e| {
        ErreurSite::Amont(format!(
            "le GLB assemble pour {code} n'est pas lisible par le rendu du depot: {e}"
        ))
    })
}

/// Prend un jeton de rastérisation, ou dit pourquoi il n'y en a plus.
async fn jeton_rendu() -> Result<tokio::sync::SemaphorePermit<'static>, ErreurSite> {
    jetons_rendu()
        .acquire()
        .await
        .map_err(|_| ErreurSite::Interne("limiteur de rendu ferme".to_owned()))
}

/// Tire un corps depuis l'amont **par le proxy du site**, et rend ses octets.
///
/// Passer par [`super::assets::proxy`] plutôt que par un appel direct n'est pas de la
/// politesse : c'est ce qui fait que le GLB tiré pour rendre une vignette et celui servi à
/// `/model/…/x.glb` partagent une seule entrée de cache, un seul jeton de concurrence et une
/// seule borne de taille. Un second client aurait sa propre file, son propre cache, et le
/// service tirerait deux fois les mêmes trois mégaoctets.
async fn octets_amont(etat: &EtatSite, chemin: &str) -> Result<bytes::Bytes, ErreurSite> {
    let reponse = super::assets::proxy(
        State(etat.clone()),
        Path(chemin.to_owned()),
        RawQuery(None),
        HeaderMap::new(),
    )
    .await?;
    let statut = reponse.status();
    if !statut.is_success() {
        return Err(ErreurSite::Amont(format!(
            "amont a repondu {} pour {chemin}",
            statut.as_u16()
        )));
    }
    reponse
        .into_body()
        .collect()
        .await
        .map(http_body_util::Collected::to_bytes)
        .map_err(|e| ErreurSite::Amont(format!("corps d'amont illisible: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_six_familles_sont_distinctes_et_routables() {
        assert_eq!(FAMILLES.len(), 6);
        let mut segments: Vec<&str> = FAMILLES.iter().map(|f| f.segment()).collect();
        segments.sort_unstable();
        segments.dedup();
        assert_eq!(segments.len(), 6, "six segments distincts");
        for f in FAMILLES {
            assert_eq!(Famille::depuis_segment(f.segment()), Some(f));
            assert!(!f.libelle().is_empty());
        }
        assert_eq!(Famille::depuis_segment("inexistante"), None);
    }

    #[test]
    fn le_chemin_d_amont_depend_de_la_famille() {
        // Un personnage passe par la recette complète, une technique par la paire g4md/g4mg.
        assert_eq!(
            Famille::Perso.chemin_amont("c01000010"),
            "model-full/c01000010.glb"
        );
        // Le souligné du DOSSIER (`_waza`) n'est pas dans le SOUS-DOMAINE (`waza`) : les
        // confondre rend un 404 que rien n'explique.
        assert_eq!(
            Famille::Waza.chemin_amont("a000010"),
            "model-chr/waza/a000010.glb"
        );
        assert_eq!(
            Famille::Armd.chemin_amont("ka000101"),
            "model-chr/armd/ka000101.glb"
        );
        assert_eq!(
            Famille::Waza.dossier_vfs().as_deref(),
            Some("data/common/chr/_waza")
        );
        assert_eq!(Famille::Perso.dossier_vfs(), None);
    }

    #[test]
    fn le_code_est_borne_avant_de_partir_vers_l_amont() {
        assert!(code_valide("c01000010"));
        assert!(code_valide("u000101_10"));
        assert!(!code_valide(""));
        assert!(!code_valide("../secret"));
        assert!(!code_valide("a/b"));
        assert!(!code_valide("a b"));
        assert!(!code_valide(&"x".repeat(33)));
    }

    #[test]
    fn l_apercu_quantifie_son_angle_et_borne_sa_taille() {
        let d = |angle, l, h| DemandeApercu { angle, l, h }.bornee();
        assert_eq!(
            d(None, None, None),
            (0, TAILLE_RENDU_DEFAUT, TAILLE_RENDU_DEFAUT)
        );
        // La hauteur suit la largeur quand elle est tue : une vignette est carrée.
        assert_eq!(d(None, Some(200), None), (0, 200, 200));
        // 360° et 0° sont le même angle : une seule clé de cache.
        assert_eq!(d(Some(360.0), None, None).0, 0);
        assert_eq!(d(Some(359.7), None, None).0, 0);
        assert_eq!(d(Some(-90.0), None, None).0, 270);
        assert_eq!(d(Some(45.4), None, None).0, 45);
        // Une demande absurde est ramenée dans les bornes, elle n'échoue pas.
        assert_eq!(
            d(None, Some(99_999), Some(0)),
            (0, TAILLE_RENDU_MAX, TAILLE_RENDU_MIN)
        );
        // Un angle non fini ne doit pas se propager jusqu'au rastériseur.
        assert_eq!(d(Some(f32::NAN), None, None).0, 0);
        assert_eq!(d(Some(f32::INFINITY), None, None).0, 0);
    }

    #[test]
    fn la_source_perso_vise_la_bonne_table_et_ne_retient_que_les_c() {
        // La table est en dur dans la constante (un `const` ne s'interpole pas) : ce test est
        // ce qui empêche les deux de diverger le jour où le miroir renomme sa table.
        assert_eq!(TABLE_CHARA, "inagle_characters");
        assert!(SOURCE_PERSO.contains(TABLE_CHARA));
        assert!(
            SOURCE_PERSO.contains("LIKE 'c%'"),
            "les 66 codes non-`c` du miroir (an…, n…, e…) ne sont pas assemblables par \
             /model-full : les proposer produisait autant de vignettes en 404"
        );
        assert!(!SOURCE_PERSO.contains('*'), "jamais SELECT *");
        // Un seul `?` nulle part : les paramètres sont posés par les appelants, et une
        // sous-requête qui en porterait décalerait leur numérotation.
        assert!(!SOURCE_PERSO.contains('?'));
    }

    #[test]
    fn le_catalogue_filtre_sur_le_code_et_sur_le_nom() {
        let modeles = vec![
            Modele::nouveau(
                Famille::Perso,
                "c01000010".into(),
                Some("Mark Evans".into()),
                None,
            ),
            Modele::nouveau(
                Famille::Perso,
                "c05024610".into(),
                Some("Axel Blaze".into()),
                None,
            ),
            Modele::nouveau(Famille::Waza, "a000010".into(), None, Some(3)),
        ];
        assert_eq!(filtrer(modeles.clone(), None).len(), 3);
        assert_eq!(filtrer(modeles.clone(), Some("mark")).len(), 1);
        assert_eq!(filtrer(modeles.clone(), Some("c0")).len(), 2);
        assert_eq!(filtrer(modeles, Some("introuvable")).len(), 0);
    }

    #[test]
    fn les_urls_du_modele_pointent_sur_ses_deux_formes() {
        let m = Modele::nouveau(Famille::Keshin, "k000010".into(), None, Some(4));
        assert_eq!(m.glb, "/model/keshin/k000010.glb");
        assert_eq!(m.apercu, "/model/keshin/k000010.png");
        assert_eq!(m.fichiers, Some(4));
        assert_eq!(m.nom, None, "aucun nom inventé hors des personnages");
    }

    #[test]
    fn seules_les_familles_de_dossier_se_verifient_sur_le_vfs() {
        // `_item/b000003` existe dans le VFS mais n'a PAS de `.g4mg` : l'amont y répond 404,
        // et le catalogue ne doit donc pas le proposer. C'est le cas mesuré le 2026-09-06.
        let index = IndexVfs::depuis(vec![
            ("data/common/chr/_waza/a000010/a000010.g4mg".to_owned(), 10),
            ("data/common/chr/_waza/a000010/a000010.g4pkm".to_owned(), 20),
            ("data/common/chr/_item/b000003/b000003.g4sk".to_owned(), 30),
            (
                "data/common/chr/_item/b000003/b000003.objbin".to_owned(),
                40,
            ),
            ("data/common/chr/_item/d010000/d010000.g4mg".to_owned(), 50),
            ("data/common/chr/c000101/c000101.g4md".to_owned(), 60),
        ]);
        let waza = codes_vfs(&index, Famille::Waza);
        assert_eq!(waza.len(), 1);
        assert_eq!(waza[0].code, "a000010");
        assert_eq!(waza[0].fichiers, Some(2), "les deux fichiers du dossier");

        let item = codes_vfs(&index, Famille::Item);
        assert_eq!(item.len(), 1, "b000003 est ecarte, faute de g4mg");
        assert_eq!(item[0].code, "d010000");

        assert!(codes_vfs(&index, Famille::Animal).is_empty());
        // `perso` ne se lit jamais sur le VFS, même quand des dossiers `c…` existent.
        assert!(codes_vfs(&index, Famille::Perso).is_empty());
    }

    #[test]
    fn la_demande_de_catalogue_borne_et_nomme() {
        let d = DemandeCatalogue::default();
        assert_eq!(d.famille().unwrap(), Famille::Perso, "perso par defaut");
        assert_eq!(d.motif(), None);
        assert_eq!(d.pagination().page, 1);

        let d = DemandeCatalogue {
            famille: Some("keshin".into()),
            page: Some(3),
            per_page: Some(9_999),
            q: Some("  Mark  ".into()),
        };
        assert_eq!(d.famille().unwrap(), Famille::Keshin);
        assert_eq!(
            d.motif().as_deref(),
            Some("mark"),
            "espaces et casse retires"
        );
        assert_eq!(d.pagination().per_page, crate::config::PER_PAGE_MAX);

        let d = DemandeCatalogue {
            famille: Some("licorne".into()),
            ..Default::default()
        };
        let e = d.famille().unwrap_err();
        assert_eq!(e.statut().as_u16(), 404);
        assert!(
            e.to_string().contains("perso"),
            "l'erreur nomme les familles"
        );
    }

    #[test]
    fn la_mesure_compte_ce_qui_est_dessine() {
        // Un carré : quatre sommets, deux triangles, une texture 2×2.
        let modele = nie_render3d::glb::Model {
            primitives: vec![nie_render3d::glb::Primitive {
                positions: vec![
                    [-1.0, 0.0, -1.0],
                    [1.0, 0.0, -1.0],
                    [1.0, 2.0, -1.0],
                    [-1.0, 2.0, -1.0],
                ],
                normals: vec![[0.0, 0.0, 1.0]; 4],
                uv: vec![[0.0, 0.0]; 4],
                indices: vec![0, 1, 2, 0, 2, 3],
                texture: Some(0),
            }],
            textures: vec![nie_render3d::glb::Texture {
                width: 2,
                height: 2,
                rgba: vec![255; 16],
            }],
        };
        let a = mesurer(&modele);
        assert_eq!(a.primitives, 1);
        assert_eq!(a.primitives_sans_texture, 0);
        assert_eq!(a.sommets, 4);
        assert_eq!(a.triangles, 2);
        assert_eq!(a.texels, 4);
        assert_eq!(a.boite.min, [-1.0, 0.0, -1.0]);
        assert_eq!(a.boite.max, [1.0, 2.0, -1.0]);
        assert_eq!(a.boite.centre, [0.0, 1.0, -1.0]);
        assert!((a.boite.rayon - 1.0).abs() < 1e-6);

        // Un modèle vide rend des bornes LISIBLES, jamais des infinis que JSON ne sait pas
        // encoder.
        let vide = nie_render3d::glb::Model {
            primitives: Vec::new(),
            textures: Vec::new(),
        };
        let a = mesurer(&vide);
        assert_eq!(a.sommets, 0);
        assert_eq!(a.boite.min, [0.0; 3]);
        assert!(a.boite.min.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn le_moteur_annonce_les_conventions_du_rasteriseur() {
        // Les valeurs publiées doivent être CELLES du rastériseur, pas des copies : le viewport
        // WebGL du site cadre la même vue à partir d'elles, et une copie périmée décadrerait
        // l'interactif par rapport à la vignette sans que rien ne le signale.
        assert!((nie_render3d::render::FOCALE - 1.7).abs() < 1e-6);
        assert!((nie_render3d::render::DISTANCE_CAMERA - 3.1).abs() < 1e-6);
        assert!((nie_render3d::render::TILT - 0.20).abs() < 1e-6);
        const {
            assert!(TAILLE_RENDU_MIN < TAILLE_RENDU_DEFAUT);
            assert!(TAILLE_RENDU_DEFAUT < TAILLE_RENDU_MAX);
        }
        assert!(jetons_rendu().available_permits() >= 1);
    }
}
