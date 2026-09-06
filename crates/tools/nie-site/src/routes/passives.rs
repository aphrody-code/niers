//! `/api/v1/passives` — les compétences passives du jeu : joueur, équipe et lots.
//!
//! # Pourquoi cette route n'est pas une famille de plus
//!
//! Les 121 familles servies par `/api/v1/donnees/famille/{cle}` ont toutes la même forme :
//! **un** `.cfg.bin` entre, **une** structure nommée sort. `nie_data::passives` ne rentre pas
//! dans ce moule, et c'est exactement ce que la matrice de couverture avait classé
//! `manquant` avec sa raison écrite :
//!
//! > `parse_player_passives(root, text_fr, text_en, text_ja)` prend **trois tables de texte**
//! > en plus du conteneur : la façade `decode_by_key(cle, root)` ne les a pas.
//!
//! Le module joint **cinq** fichiers du VFS (trois de données, deux de texte × trois langues)
//! pour produire une base unifiée. Aucune signature de façade à un argument ne peut l'exprimer :
//! il fallait une route, pas une entrée de plus dans `decode_by_key`.
//!
//! # Nommage
//!
//! Écrit sous la règle du 2026-09-06 : identifiants, URLs et clés JSON en **anglais**,
//! commentaires en français. Cf. `routes::text`, premier module de cette règle.
//!
//! # Les routes
//!
//! | Route | Ce qu'elle rend |
//! |---|---|
//! | `GET /api/v1/passives` | le catalogue **mesuré** : sources résolues, comptes, temps de construction |
//! | `GET /api/v1/passives/player` | les instances joueur, paginées, filtrables par `q` |
//! | `GET /api/v1/passives/team` | les passifs d'équipe, paginés |
//! | `GET /api/v1/passives/lots` | les lots de tirage, paginés |
//!
//! # Ce que la réponse ne promet pas
//!
//! Les textes des passifs d'équipe sont **en japonais dans toutes les locales** : le jeu ne les
//! a pas localisés. Le catalogue le dit (`team_text_localized: false`) plutôt que de laisser
//! croire à un défaut de décodage — un champ qui surprend sans être expliqué est un bug qu'on
//! ira chercher ailleurs.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Serialize;

use crate::error::ErreurSite;
use crate::routes::{DemandePage, Page};
use crate::state::EtatSite;
use crate::vfs_index::IndexVfs;

/// Les trois langues dont `parse_player_passives` a besoin, dans son ordre d'argument.
///
/// Ce ne sont pas les seules langues du jeu (il y en a dix sous `common/text/`) : ce sont
/// celles que le parseur consomme. En ajouter une ici ne servirait à rien tant que
/// `LocalizedText` porte trois champs.
const LANGUAGES: [&str; 3] = ["fr", "en", "ja"];

/// Les clés de famille des cinq sources, telles que `nie_data::typed::family_key` les dérive.
///
/// Mesuré, pas supposé (`niers vfs find 'passive_skill' -n 20`, 2026-09-06) : les fichiers du
/// jeu portent un numéro de version (`passive_skill_config_5.00.07.00.cfg.bin`) que personne ne
/// devine. On résout donc par **clé de famille**, jamais par chemin écrit à la main — c'est la
/// même règle que `routes::donnees` et elle survit à la prochaine mise à jour du jeu.
mod keys {
    /// Les 1 716 instances de passif joueur.
    pub const PLAYER: &str = "passive_skill_config";
    /// Les 21 passifs d'équipe.
    pub const TEAM: &str = "soccer_team_passive_config";
    /// Les 653 lots de tirage.
    pub const LOTS: &str = "team_passive_lot_table_config";
    /// Les textes `NOUN_INFO` qui nomment et décrivent les passifs joueur.
    pub const PLAYER_TEXT: &str = "skill_text";
    /// Les textes `TEXT_INFO` des passifs d'équipe.
    pub const TEAM_TEXT: &str = "soccer_team_passive_text";
}

/// Où vit chaque source dans le VFS — le préfixe qui lève l'ambiguïté quand plusieurs fichiers
/// partagent une clé.
///
/// `passive_skill_effect_config` existe **deux fois** dans ce jeu (`gamedata/skill/` et
/// `gamedata/soccer/`) : sans préfixe, la résolution par clé seule tomberait sur l'un ou
/// l'autre selon l'ordre d'indexation, et personne ne verrait laquelle a été prise.
mod prefixes {
    /// Les données de compétences.
    pub const SKILL: &str = "data/common/gamedata/skill/";
    /// Les données de match.
    pub const SOCCER: &str = "data/common/gamedata/soccer/";
    /// Les données de personnage.
    pub const CHARACTER: &str = "data/common/gamedata/character/";
    /// Le texte localisé.
    pub const TEXT: &str = "data/common/text/";
}

/// Une source résolue : la clé demandée, et le chemin VFS réellement lu.
#[derive(Debug, Clone, Serialize)]
pub struct Source {
    /// Le rôle de la source dans la jointure (`player`, `team`, `lots`, `text:fr`…).
    pub role: String,
    /// La clé de famille qui l'a désignée.
    pub key: &'static str,
    /// Le chemin VFS retenu, numéro de version compris.
    pub path: String,
    /// Sa taille en octets, telle que l'index la donne.
    pub bytes: u32,
}

/// Le catalogue publié par `GET /api/v1/passives`.
#[derive(Debug, Clone, Serialize)]
pub struct Catalog {
    /// Les cinq (ou plus) fichiers réellement joints, avec leur chemin résolu.
    pub sources: Vec<Source>,
    /// Nombre d'instances de passif joueur.
    pub player_count: usize,
    /// Nombre de passifs d'équipe.
    pub team_count: usize,
    /// Nombre de lots de tirage.
    pub lot_count: usize,
    /// Nombre d'`effect_id` joueur distincts — le nombre de *familles* de passifs, à
    /// distinguer du nombre d'instances (une famille en génère une par rareté).
    pub unique_effect_count: usize,
    /// Nombre de `string_id` distincts.
    pub unique_string_id_count: usize,
    /// `false` : les textes d'équipe sont en japonais dans **toutes** les locales du jeu.
    /// Ce n'est pas un défaut de décodage, c'est ce que le jeu contient.
    pub team_text_localized: bool,
    /// Millisecondes de la construction, mesurées au premier appel.
    pub build_ms: u64,
}

/// La base construite une fois, puis gardée. Les handlers n'en lisent que des tranches.
struct Built {
    /// Les sources résolues, pour le catalogue.
    sources: Vec<Source>,
    /// La base unifiée de `nie-data`.
    db: nie_data::passives::UnifiedPassiveDb,
    /// Durée de la construction.
    build_ms: u64,
}

/// La construction est faite **au plus une fois** par processus.
static BUILT: OnceLock<Result<Built, String>> = OnceLock::new();

/// Lit un fichier du VFS et le rend sous la forme iecode que `nie-data` consomme.
fn read_iecode(
    vfs: &nie_formats::vfs::Vfs,
    path: &str,
) -> Result<serde_json::Value, String> {
    let bytes = vfs
        .read(path)
        .map_err(|e| format!("lecture impossible de `{path}` : {e}"))?;
    nie_formats::cfgbin::to_iecode_json(&bytes)
        .ok_or_else(|| format!("`{path}` n'est ni RDBN ni T2B"))
}

/// Construit la base unifiée en joignant les cinq sources.
///
/// Toute source manquante fait échouer la construction **avec son nom** : une base bâtie sur
/// quatre fichiers sur cinq rendrait des passifs sans texte tout en annonçant un succès, et
/// c'est le défaut que ce dépôt a déjà payé sur `/chara` — 200, 87 ms, zéro lien.
fn build(index: &IndexVfs, vfs: &nie_formats::vfs::Vfs) -> Result<Built, String> {
    let start = std::time::Instant::now();
    let mut sources = Vec::new();

    let mut take = |role: &str, key: &'static str, prefix: &str| -> Result<String, String> {
        let (path, bytes) = super::donnees::resoudre(index, key, Some(prefix)).ok_or_else(|| {
            format!("source `{role}` absente : aucun `{key}` sous `{prefix}` dans ce VFS")
        })?;
        sources.push(Source {
            role: role.to_owned(),
            key,
            path: path.clone(),
            bytes,
        });
        Ok(path)
    };

    let player_path = take("player", keys::PLAYER, prefixes::SKILL)?;
    let team_path = take("team", keys::TEAM, prefixes::SOCCER)?;
    let lots_path = take("lots", keys::LOTS, prefixes::CHARACTER)?;

    let mut player_texts: BTreeMap<&str, BTreeMap<u32, String>> = BTreeMap::new();
    let mut team_texts: BTreeMap<&str, BTreeMap<u32, String>> = BTreeMap::new();
    for language in LANGUAGES {
        let prefix = format!("{}{language}/", prefixes::TEXT);
        let path = take(
            &format!("player_text:{language}"),
            keys::PLAYER_TEXT,
            &prefix,
        )?;
        player_texts.insert(
            language,
            nie_data::passives::load_noun_texts(&read_iecode(vfs, &path)?),
        );
        let path = take(&format!("team_text:{language}"), keys::TEAM_TEXT, &prefix)?;
        team_texts.insert(
            language,
            nie_data::passives::load_team_passive_texts(&read_iecode(vfs, &path)?),
        );
    }

    let empty = BTreeMap::new();
    let text_of = |m: &BTreeMap<&str, BTreeMap<u32, String>>, l: &str| -> BTreeMap<u32, String> {
        m.get(l).unwrap_or(&empty).clone()
    };

    let player = nie_data::passives::parse_player_passives(
        &read_iecode(vfs, &player_path)?,
        &text_of(&player_texts, "fr"),
        &text_of(&player_texts, "en"),
        &text_of(&player_texts, "ja"),
    );
    let team = nie_data::passives::parse_team_passives(
        &read_iecode(vfs, &team_path)?,
        &text_of(&team_texts, "ja"),
    );
    let lots = nie_data::passives::parse_lots(&read_iecode(vfs, &lots_path)?);

    let unique_effect_count = player
        .iter()
        .map(|p| p.effect_id)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let unique_string_id_count = player
        .iter()
        .map(|p| p.string_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    Ok(Built {
        sources,
        db: nie_data::passives::UnifiedPassiveDb {
            player,
            team,
            lots,
            unique_effect_count,
            unique_string_id_count,
        },
        build_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// Rend la base construite, ou l'erreur qui a empêché de la bâtir.
fn built(state: &EtatSite) -> Result<&'static Built, ErreurSite> {
    let index = state.index()?;
    let vfs = state.vfs()?;
    match BUILT.get_or_init(|| build(&index, &vfs)) {
        Ok(b) => Ok(b),
        Err(raison) => Err(ErreurSite::Indisponible(raison.clone())),
    }
}

/// `GET /api/v1/passives` — le catalogue mesuré.
///
/// # Errors
///
/// `503` quand le VFS n'est pas monté ou qu'une des cinq sources manque — avec le nom de
/// celle qui manque.
pub async fn catalog(State(state): State<EtatSite>) -> Result<Json<Catalog>, ErreurSite> {
    let b = tokio::task::spawn_blocking(move || built(&state)).await??;
    Ok(Json(Catalog {
        sources: b.sources.clone(),
        player_count: b.db.player.len(),
        team_count: b.db.team.len(),
        lot_count: b.db.lots.len(),
        unique_effect_count: b.db.unique_effect_count,
        unique_string_id_count: b.db.unique_string_id_count,
        team_text_localized: false,
        build_ms: b.build_ms,
    }))
}

/// Les trois espèces adressables sous `/api/v1/passives/{kind}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Les instances de passif joueur.
    Player,
    /// Les passifs d'équipe.
    Team,
    /// Les lots de tirage.
    Lots,
}

impl Kind {
    /// Reconnaît le segment d'URL. `None` sur tout autre mot.
    #[must_use]
    pub fn from_segment(s: &str) -> Option<Self> {
        match s {
            "player" => Some(Self::Player),
            "team" => Some(Self::Team),
            "lots" => Some(Self::Lots),
            _ => None,
        }
    }
}

/// `GET /api/v1/passives/{kind}` — une espèce, paginée.
///
/// `q` filtre sans casse sur ce qui identifie l'élément : `string_id` et texte résolu pour un
/// passif joueur, texte pour un passif d'équipe, condition pour un lot. Le paramètre est
/// **honoré** — un `q` accepté puis ignoré ferait croire à un client qu'il filtre, et c'est le
/// défaut 1 du lot 8 du cap.
///
/// # Errors
///
/// `400` sur un segment inconnu (avec la liste des trois valeurs acceptées), `503` quand la
/// base n'a pas pu être bâtie.
pub async fn kind(
    State(state): State<EtatSite>,
    Path(segment): Path<String>,
    Query(demande): Query<DemandePage>,
) -> Result<Json<Page<serde_json::Value>>, ErreurSite> {
    let kind = Kind::from_segment(&segment).ok_or_else(|| {
        ErreurSite::Demande(format!(
            "espece inconnue `{segment}` ; les trois valeurs servies sont player, team, lots"
        ))
    })?;
    let b = tokio::task::spawn_blocking(move || built(&state)).await??;
    let motif = demande.q.as_deref().map(str::to_lowercase);
    let bornes = demande.bornee();

    // Chaque espèce est sérialisée par `serde`, jamais par `Debug` : un JSON public ne publie
    // pas le nom Rust d'une variante (cf. § 3 du cap, `format!("{:?}")` sur une `Option`).
    let retenus: Vec<serde_json::Value> = match kind {
        Kind::Player => filtrer(&b.db.player, motif.as_deref(), |p| {
            let mut s = p.string_id.clone();
            if let Some(t) = p.text_resolved.best() {
                s.push(' ');
                s.push_str(t);
            }
            s
        }),
        Kind::Team => filtrer(&b.db.team, motif.as_deref(), |t| {
            t.text.best().unwrap_or_default().to_owned()
        }),
        Kind::Lots => filtrer(&b.db.lots, motif.as_deref(), |l| l.condition.clone()),
    };

    let total = retenus.len();
    let elements = retenus
        .into_iter()
        .skip(bornes.offset())
        .take(bornes.per_page as usize)
        .collect();
    Ok(Json(Page::nouvelle(elements, bornes, total)))
}

/// Filtre puis sérialise une tranche de la base. `cle` extrait le texte comparé à `motif`.
fn filtrer<T: Serialize, F: Fn(&T) -> String>(
    items: &[T],
    motif: Option<&str>,
    cle: F,
) -> Vec<serde_json::Value> {
    items
        .iter()
        .filter(|item| motif.is_none_or(|m| cle(item).to_lowercase().contains(m)))
        .filter_map(|item| serde_json::to_value(item).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_trois_especes_sont_reconnues_et_rien_dautre() {
        // Preuve par falsification : la moitié positive passerait aussi sur un `from_segment`
        // qui dirait oui à tout.
        assert_eq!(Kind::from_segment("player"), Some(Kind::Player));
        assert_eq!(Kind::from_segment("team"), Some(Kind::Team));
        assert_eq!(Kind::from_segment("lots"), Some(Kind::Lots));
        assert_eq!(Kind::from_segment("Player"), None);
        assert_eq!(Kind::from_segment("passives"), None);
        assert_eq!(Kind::from_segment(""), None);
    }

    #[test]
    fn le_filtre_est_applique_et_ne_rend_pas_tout() {
        // Le défaut que ce test interdit : `/b` déclarait `q` et ne l'appliquait jamais, si
        // bien que la liste complète passait pour un résultat filtré.
        let lots = vec![
            nie_data::passives::TeamPassiveLot {
                id: nie_data::hash::HashId(1),
                lot_weight: 10,
                condition: "STORY_CLEAR".to_owned(),
                rarity_enable_flag: true,
            },
            nie_data::passives::TeamPassiveLot {
                id: nie_data::hash::HashId(2),
                lot_weight: 20,
                condition: String::new(),
                rarity_enable_flag: false,
            },
        ];
        assert_eq!(filtrer(&lots, None, |l| l.condition.clone()).len(), 2);
        assert_eq!(
            filtrer(&lots, Some("story"), |l| l.condition.clone()).len(),
            1,
            "le filtre doit REDUIRE la liste, sinon il n'est pas applique"
        );
        assert_eq!(
            filtrer(&lots, Some("zzz"), |l| l.condition.clone()).len(),
            0
        );
    }

    #[test]
    fn la_serialisation_publie_des_champs_choisis_pas_du_debug() {
        let lot = nie_data::passives::TeamPassiveLot {
            id: nie_data::hash::HashId(0x1234_5678),
            lot_weight: 7,
            condition: "X".to_owned(),
            rarity_enable_flag: true,
        };
        let v = serde_json::to_value(&lot).expect("TeamPassiveLot derive Serialize");
        assert_eq!(v["id"], 0x1234_5678_u32, "HashId est transparent en u32");
        assert_eq!(v["lot_weight"], 7);
        assert!(
            !v.to_string().contains("HashId("),
            "un JSON public ne porte pas le nom Rust d'un type : {v}"
        );
    }
}
