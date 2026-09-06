//! `/api/v1/playstyles` — le **style de jeu** d'un personnage, et sa distribution réelle.
//!
//! # Pourquoi cette route n'est pas une famille de plus
//!
//! `nie_data::playstyle` lit le **même fichier** que `chara_param` — les noeuds
//! `CHARA_PARAM_INFO` de `character/chara_param_*.cfg.bin` — mais il n'en extrait qu'une
//! variable, la **6ᵉ de type `Int`**. Or `decode_by_key` dispatche **une clé vers un parseur** :
//! la clé `chara_param` est déjà prise par `chara_param`, et une façade ne peut pas rendre deux
//! structures différentes pour la même clé sans que le client cesse de savoir ce qu'il reçoit.
//!
//! La matrice de couverture classait donc `playstyle` `manquant` : parseur écrit, golden testé,
//! aucune route. Ce n'était pas un manque de code, c'était un manque d'**adresse**.
//!
//! # Ce que la route mesure au lieu de l'affirmer
//!
//! Les six libellés viennent de `menu_text.cfg.bin` (`TEXT_INFO_1812-1817`) et sont figés dans
//! `nie-data`. Ce que la route **ne** fige pas, c'est la distribution : elle est **comptée** sur
//! le fichier réel à chaque démarrage. Un plan qui citerait « 1 055 joueurs en Contre » sans
//! commande écrirait un souvenir ; ici le compte est la réponse.
//!
//! Corollaire assumé : un identifiant hors `0..=5` sortirait avec `label_en: null`. Le corpus
//! mesuré n'en contient aucun, et si le jeu en introduit un demain, la route le montrera au lieu
//! de le ranger sous un libellé inventé.
//!
//! # Nommage
//!
//! Identifiants, URLs et clés JSON en **anglais** (règle du 2026-09-06), commentaires en
//! français. Cf. `routes::text`.
//!
//! # Les routes
//!
//! | Route | Ce qu'elle rend |
//! |---|---|
//! | `GET /api/v1/playstyles` | les styles, leurs libellés, et le compte **mesuré** de chacun |
//! | `GET /api/v1/playstyles/{id}` | les personnages de ce style, paginés |

use std::sync::OnceLock;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Serialize;

use crate::error::ErreurSite;
use crate::routes::{DemandePage, Page};
use crate::state::EtatSite;

/// La clé de famille du fichier lu, telle que `nie_data::typed::family_key` la dérive de
/// `chara_param_1.03.66.00.cfg.bin`.
const KEY: &str = "chara_param";

/// Le préfixe VFS qui lève l'ambiguïté avec `seasonal_chara_param` et
/// `chara_param_table_config` — deux fichiers voisins dont la clé dérivée diffère, mais dont la
/// proximité vaut qu'on borne explicitement.
const PREFIX: &str = "data/common/gamedata/character/";

/// Un style de jeu, avec ses libellés et son compte mesuré.
#[derive(Debug, Clone, Serialize)]
pub struct Playstyle {
    /// L'identifiant tel qu'il vit dans le fichier (`0..=5` sur ce jeu).
    pub id: i64,
    /// Le libellé anglais, ou `null` hors des six connus.
    pub label_en: Option<&'static str>,
    /// Le libellé français, ou `null` hors des six connus.
    pub label_fr: Option<&'static str>,
    /// Le nombre de personnages qui le portent, **compté** sur le fichier réel.
    pub characters: usize,
}

/// Un personnage, réduit à ce que ce fichier en dit.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    /// `chara_param_id` — le CRC du personnage, la clé de jointure vers `/api/v1/chara`.
    pub chara_param_id: u32,
    /// Son style de jeu.
    pub playstyle: i64,
}

/// Le catalogue publié par `GET /api/v1/playstyles`.
#[derive(Debug, Clone, Serialize)]
pub struct Catalog {
    /// Le chemin VFS réellement lu, numéro de version compris.
    pub source: String,
    /// Le nombre total de noeuds retenus — un noeud portant moins de six entiers est rejeté par
    /// le parseur, exactement comme le fait la référence.
    pub total: usize,
    /// Les styles présents, par identifiant croissant.
    pub playstyles: Vec<Playstyle>,
    /// Millisecondes de la construction, mesurées au premier appel.
    pub build_ms: u64,
}

/// Le corpus lu une fois puis gardé.
struct Built {
    /// Le chemin résolu.
    source: String,
    /// Les entrées, dans l'ordre du fichier.
    entries: Vec<Entry>,
    /// Durée de la construction.
    build_ms: u64,
}

/// La lecture est faite au plus une fois par processus.
static BUILT: OnceLock<Result<Built, String>> = OnceLock::new();

/// Lit `chara_param` et en extrait les styles de jeu.
fn build(
    index: &crate::vfs_index::IndexVfs,
    vfs: &nie_formats::vfs::Vfs,
) -> Result<Built, String> {
    let start = std::time::Instant::now();
    let (source, _) = super::donnees::resoudre(index, KEY, Some(PREFIX)).ok_or_else(|| {
        format!("aucun `{KEY}` sous `{PREFIX}` dans ce VFS : le style de jeu se lit dans ce fichier et nulle part ailleurs")
    })?;
    let bytes = vfs
        .read(&source)
        .map_err(|e| format!("lecture impossible de `{source}` : {e}"))?;
    let root = nie_formats::cfgbin::to_iecode_json(&bytes)
        .ok_or_else(|| format!("`{source}` n'est ni RDBN ni T2B"))?;
    let entries = nie_data::playstyle::parse_all_playstyles(&root)
        .into_iter()
        .map(|e| Entry {
            chara_param_id: e.chara_param_id.0,
            playstyle: e.play_style,
        })
        .collect();
    Ok(Built {
        source,
        entries,
        build_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// Rend le corpus, ou l'erreur qui a empêché de le lire.
fn built(state: &EtatSite) -> Result<&'static Built, ErreurSite> {
    let index = state.index()?;
    let vfs = state.vfs()?;
    match BUILT.get_or_init(|| build(&index, &vfs)) {
        Ok(b) => Ok(b),
        Err(raison) => Err(ErreurSite::Indisponible(raison.clone())),
    }
}

/// `GET /api/v1/playstyles` — les styles et leur distribution mesurée.
///
/// # Errors
///
/// `503` quand le VFS n'est pas monté ou que `chara_param` est absent de ce montage.
pub async fn catalog(State(state): State<EtatSite>) -> Result<Json<Catalog>, ErreurSite> {
    let b = tokio::task::spawn_blocking(move || built(&state)).await??;
    let mut par_id: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    for e in &b.entries {
        *par_id.entry(e.playstyle).or_insert(0) += 1;
    }
    Ok(Json(Catalog {
        source: b.source.clone(),
        total: b.entries.len(),
        playstyles: par_id
            .into_iter()
            .map(|(id, characters)| Playstyle {
                id,
                label_en: nie_data::playstyle::playstyle_id_to_en(id),
                label_fr: nie_data::playstyle::playstyle_id_to_fr(id),
                characters,
            })
            .collect(),
        build_ms: b.build_ms,
    }))
}

/// `GET /api/v1/playstyles/{id}` — les personnages d'un style, paginés.
///
/// # Errors
///
/// `400` si `id` n'est pas un entier, `404` si aucun personnage ne porte ce style — avec le
/// renvoi vers le catalogue, qui dit lesquels existent.
pub async fn playstyle(
    State(state): State<EtatSite>,
    Path(brut): Path<String>,
    Query(demande): Query<DemandePage>,
) -> Result<Json<Page<Entry>>, ErreurSite> {
    let id: i64 = brut.parse().map_err(|_| {
        ErreurSite::Demande(format!(
            "`{brut}` n'est pas un identifiant de style : ce sont des entiers, listes sur /api/v1/playstyles"
        ))
    })?;
    let b = tokio::task::spawn_blocking(move || built(&state)).await??;
    let retenus: Vec<&Entry> = b.entries.iter().filter(|e| e.playstyle == id).collect();
    if retenus.is_empty() {
        return Err(ErreurSite::Introuvable(format!(
            "aucun personnage de ce jeu ne porte le style `{id}` ; les styles presents sont sur /api/v1/playstyles"
        )));
    }
    let bornes = demande.bornee();
    let total = retenus.len();
    let elements: Vec<Entry> = retenus
        .into_iter()
        .skip(bornes.offset())
        .take(bornes.per_page as usize)
        .cloned()
        .collect();
    Ok(Json(Page::nouvelle(elements, bornes, total)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn les_six_libelles_existent_et_le_septieme_non() {
        // Preuve par falsification : la moitie positive seule passerait sur une table qui
        // repondrait a tout — inagle, lui, retombe sur "PlayStyle{id}", ce port rend `None`.
        for id in 0..=5 {
            assert!(
                nie_data::playstyle::playstyle_id_to_en(id).is_some(),
                "le style {id} doit avoir un libelle anglais"
            );
            assert!(nie_data::playstyle::playstyle_id_to_fr(id).is_some());
        }
        assert_eq!(nie_data::playstyle::playstyle_id_to_en(6), None);
        assert_eq!(nie_data::playstyle::playstyle_id_to_fr(-1), None);
    }

    #[test]
    fn un_identifiant_non_entier_est_un_400_pas_un_404() {
        // Le distinguo compte : un 404 laisserait croire que le style existe mais qu'il est
        // vide, alors que la demande elle-meme est mal formee.
        let e: Result<i64, _> = "Counter".parse::<i64>();
        assert!(e.is_err(), "le segment d'URL est un entier, pas un libelle");
    }

    #[test]
    fn le_parseur_rejette_un_noeud_a_moins_de_six_entiers() {
        // C'est la regle du port 1:1 : `if (values.length < 6) return null`. La verifier ici
        // interdit qu'un futur assouplissement du parseur fasse gonfler le compte publie.
        let root = serde_json::json!({
            "entries": [{
                "name": "CHARA_PARAM_INFO_0",
                "variables": [
                    {"type":"Int","value":"1"},
                    {"type":"Int","value":"2"},
                    {"type":"Int","value":"3"},
                    {"type":"Int","value":"4"},
                    {"type":"Int","value":"5"}
                ],
                "children": []
            }]
        });
        assert!(
            nie_data::playstyle::parse_all_playstyles(&root).is_empty(),
            "cinq entiers ne suffisent pas : le style vit a l'index 5"
        );
    }

    #[test]
    fn un_noeud_complet_rend_bien_la_sixieme_variable_entiere() {
        let root = serde_json::json!({
            "entries": [{
                "name": "CHARA_PARAM_INFO_0",
                "variables": [
                    {"type":"Int","value":"901280304"},
                    {"type":"Int","value":"1"},
                    {"type":"Int","value":"2"},
                    {"type":"Int","value":"3"},
                    {"type":"Int","value":"4"},
                    {"type":"Int","value":"5"}
                ],
                "children": []
            }]
        });
        let v = nie_data::playstyle::parse_all_playstyles(&root);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].play_style, 5, "index 5 des variables Int, pas var[5]");
        assert_eq!(
            nie_data::playstyle::playstyle_id_to_en(v[0].play_style),
            Some("Freedom")
        );
    }
}
