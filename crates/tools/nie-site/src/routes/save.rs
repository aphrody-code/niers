//! `/api/v1/save` — résoudre en **lot** les identifiants d'un effectif de sauvegarde.
//!
//! # Ce que la route fait, et surtout ce qu'elle ne reçoit pas
//!
//! Une sauvegarde d'IEVR est lue **chez le joueur** : `nie-save` est compilé en wasm et
//! `parse_autosave_roster` tourne dans le navigateur. Ce qui traverse le réseau n'est donc
//! **jamais** un fichier de sauvegarde, jamais une progression, jamais un pseudonyme — c'est une
//! liste d'identifiants de personnages, les mêmes que ceux du fichier `chara_base` du jeu.
//!
//! Cette distinction est la raison pour laquelle la route existe alors que `niers save` reste
//! classé `interne` (« données personnelles, hors périmètre contractuel ») : lire une sauvegarde
//! et **nommer une liste de codes du jeu** ne sont pas la même capacité.
//!
//! # Pourquoi une route de lot, et pas 8 000 appels
//!
//! Un effectif compte des centaines d'entrées. `/api/v1/entites/inagle_characters/{id}` les
//! résout une par une : c'est un aller-retour par joueur, et le client finit par écrire sa
//! propre boucle avec sa propre gestion des inconnus. La route de lot fait ce regroupement une
//! fois, ici, avec **une** règle pour les inconnus.
//!
//! # La règle des inconnus, qui est la seule qui compte
//!
//! Un identifiant que le miroir ne connaît pas ressort avec `name: null` — **jamais deviné,
//! jamais omis**. L'omettre serait pire : le client compterait ses résultats, en trouverait
//! moins qu'il n'a envoyé, et ne saurait pas lesquels manquent. `matched` et `total` publient
//! l'écart au lieu de le laisser déduire.
//!
//! # Nommage
//!
//! Identifiants, URLs et clés JSON en **anglais** (règle du 2026-09-06) ; commentaires en
//! français.
//!
//! | Route | Ce qu'elle rend |
//! |---|---|
//! | `GET /api/v1/save/roster` | le contrat : formes d'identifiant acceptées, bornes, champs rendus |
//! | `POST /api/v1/save/roster` | les identifiants résolus, dans l'ordre d'envoi, doublons retirés |

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::error::ErreurSite;
use crate::state::EtatSite;

/// Nombre maximal d'identifiants acceptés en une requête.
///
/// Le plus gros effectif que le jeu peut atteindre est très en deçà ; la borne existe pour
/// qu'une requête absurde soit refusée avant d'atteindre SQLite, pas pour brider un usage réel.
pub const IDS_MAX: usize = 8000;

/// Taille des paquets envoyés à SQLite.
///
/// SQLite accepte par défaut 999 paramètres liés (`SQLITE_MAX_VARIABLE_NUMBER`). On découpe en
/// deçà : dépasser ne rend pas une erreur lisible, ça rend « too many SQL variables ».
const CHUNK: usize = 900;

/// La table du miroir interrogée. Elle vient d'une **constante de la crate**, jamais du client :
/// un nom de table qui traverse le réseau est une injection en attente.
const TABLE: &str = "inagle_characters";

/// Le corps de `POST /api/v1/save/roster`.
#[derive(Debug, Clone, Deserialize)]
pub struct RosterRequest {
    /// Les identifiants, sous n'importe laquelle des trois formes acceptées.
    pub ids: Vec<String>,
}

/// Un personnage résolu — ou pas.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedCharacter {
    /// L'identifiant **normalisé**, tel qu'il vit dans le miroir (`0x` + 8 chiffres hexadécimaux
    /// majuscules). C'est lui qu'il faut republier : renvoyer la forme envoyée obligerait le
    /// client à refaire la normalisation pour rapprocher sa demande de la réponse.
    pub id: String,
    /// La forme exacte reçue, pour que le client retrouve sa ligne d'origine.
    pub requested: String,
    /// Le nom français, ou `null` si le miroir ne connaît pas cet identifiant.
    pub name: Option<String>,
    /// Le slug de base, sans le suffixe de variante.
    pub base_slug: Option<String>,
    /// L'élément.
    pub element: Option<String>,
    /// Le poste.
    pub position: Option<String>,
    /// Le libellé de rareté.
    pub rarity: Option<String>,
}

/// La réponse de `POST /api/v1/save/roster`.
#[derive(Debug, Clone, Serialize)]
pub struct RosterResponse {
    /// Les identifiants résolus, dans l'ordre de première apparition dans la demande.
    pub resolved: Vec<ResolvedCharacter>,
    /// Combien portent un nom.
    pub matched: usize,
    /// Combien d'identifiants distincts ont été traités.
    pub total: usize,
    /// Combien d'entrées de la demande étaient des doublons, retirés.
    pub duplicates: usize,
    /// Combien n'étaient d'aucune des trois formes acceptées, donc écartées avant la requête.
    pub rejected: usize,
}

/// Le contrat publié par `GET /api/v1/save/roster`.
#[derive(Debug, Clone, Serialize)]
pub struct RosterContract {
    /// La méthode.
    pub method: &'static str,
    /// Le chemin.
    pub path: &'static str,
    /// Les clés du corps.
    pub body: &'static [&'static str],
    /// Les trois formes d'identifiant acceptées, avec un exemple de chacune.
    pub id_forms: &'static [&'static str],
    /// Les champs rendus par personnage.
    pub fields: &'static [&'static str],
    /// Nombre maximal d'identifiants par requête.
    pub ids_max: usize,
    /// Ce que la route ne reçoit pas — dit, plutôt que supposé.
    pub never_received: &'static [&'static str],
}

/// Normalise un identifiant vers la forme du miroir (`0xXXXXXXXX`).
///
/// Trois formes acceptées, mesurées sur ce que produisent réellement les clients :
/// `0xF5E1E7CD` (ce que rend `nie-save` en wasm), `F5E1E7CD` (le même, sans préfixe), et
/// `4125222861` (le même, en décimal non signé — ce que rend un `JSON.parse` naïf).
///
/// `None` sur tout le reste : un identifiant illisible est **écarté et compté**, pas deviné.
#[must_use]
pub fn normalize(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let sans_prefixe = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    let valeur = if let Some(hex) = sans_prefixe {
        u32::from_str_radix(hex, 16).ok()?
    } else if s.len() == 8 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        // Huit chiffres hexadécimaux sans préfixe : c'est un identifiant, pas un décimal.
        // La longueur tranche l'ambiguïté — `12345678` est lu en hexadécimal ici, et c'est ce
        // que fait le client de référence.
        u32::from_str_radix(s, 16).ok()?
    } else {
        s.parse::<u32>().ok()?
    };
    Some(format!("0x{valeur:08X}"))
}

/// `GET /api/v1/save/roster` — le contrat.
pub async fn contract() -> Json<RosterContract> {
    Json(RosterContract {
        method: "POST",
        path: "/api/v1/save/roster",
        body: &["ids"],
        id_forms: &["0xF5E1E7CD", "F5E1E7CD", "4125222861"],
        fields: &[
            "id",
            "requested",
            "name",
            "base_slug",
            "element",
            "position",
            "rarity",
        ],
        ids_max: IDS_MAX,
        never_received: &[
            "aucun fichier de sauvegarde : `nie-save` tourne en wasm chez le joueur",
            "aucune progression, aucun pseudonyme, aucune donnee de compte",
        ],
    })
}

/// `POST /api/v1/save/roster` — résoudre un effectif.
///
/// # Errors
///
/// `400` si `ids` est vide ou dépasse [`IDS_MAX`], `503` sans miroir.
pub async fn roster(
    State(state): State<EtatSite>,
    Json(demande): Json<RosterRequest>,
) -> Result<Json<RosterResponse>, ErreurSite> {
    if demande.ids.is_empty() {
        return Err(ErreurSite::Demande(
            "`ids` est vide : le contrat est sur GET /api/v1/save/roster".to_owned(),
        ));
    }
    if demande.ids.len() > IDS_MAX {
        return Err(ErreurSite::Demande(format!(
            "trop d'identifiants : {} (borne {IDS_MAX})",
            demande.ids.len()
        )));
    }

    // Normalisation, déduplication ordonnée, comptage de ce qui a été écarté.
    let mut ordre: Vec<(String, String)> = Vec::new();
    let mut vus: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut rejected = 0usize;
    let mut duplicates = 0usize;
    for brut in &demande.ids {
        match normalize(brut) {
            Some(id) => {
                if vus.insert(id.clone()) {
                    ordre.push((id, brut.clone()));
                } else {
                    duplicates += 1;
                }
            }
            None => rejected += 1,
        }
    }

    let ids: Vec<String> = ordre.iter().map(|(id, _)| id.clone()).collect();
    let gisement = state.gisement.clone();
    let lignes = tokio::task::spawn_blocking(move || interroger(&gisement, &ids)).await??;

    let mut resolved = Vec::with_capacity(ordre.len());
    let mut matched = 0usize;
    for (id, requested) in ordre {
        let ligne = lignes.get(&id);
        if ligne.is_some() {
            matched += 1;
        }
        resolved.push(ResolvedCharacter {
            id,
            requested,
            name: ligne.and_then(|l| l.name.clone()),
            base_slug: ligne.and_then(|l| l.base_slug.clone()),
            element: ligne.and_then(|l| l.element.clone()),
            position: ligne.and_then(|l| l.position.clone()),
            rarity: ligne.and_then(|l| l.rarity.clone()),
        });
    }

    Ok(Json(RosterResponse {
        total: resolved.len(),
        matched,
        duplicates,
        rejected,
        resolved,
    }))
}

/// Une ligne du miroir, réduite aux six colonnes publiées.
#[derive(Debug, Clone, Default)]
struct Ligne {
    /// `name_fr`.
    name: Option<String>,
    /// `base_slug`.
    base_slug: Option<String>,
    /// `element`.
    element: Option<String>,
    /// `position`.
    position: Option<String>,
    /// `rarity_label`.
    rarity: Option<String>,
}

/// Interroge le miroir par paquets de [`CHUNK`] identifiants.
///
/// Bloquant : appelée depuis `spawn_blocking`.
fn interroger(
    gisement: &crate::dataset::Gisement,
    ids: &[String],
) -> Result<std::collections::HashMap<String, Ligne>, ErreurSite> {
    gisement.lire(|c| {
        let mut out = std::collections::HashMap::with_capacity(ids.len());
        for paquet in ids.chunks(CHUNK) {
            // Les `?` sont des paramètres liés — la seule partie variable du SQL est leur
            // NOMBRE, jamais leur contenu.
            let places = std::iter::repeat_n("?", paquet.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT id, name_fr, base_slug, element, position, rarity_label \
                 FROM {TABLE} WHERE id IN ({places})"
            );
            let mut stmt = c.prepare(&sql).map_err(erreur_sql)?;
            let lignes = stmt
                .query_map(rusqlite::params_from_iter(paquet.iter()), |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        Ligne {
                            name: r.get(1)?,
                            base_slug: r.get(2)?,
                            element: r.get(3)?,
                            position: r.get(4)?,
                            rarity: r.get(5)?,
                        },
                    ))
                })
                .map_err(erreur_sql)?;
            for l in lignes {
                let (id, ligne) = l.map_err(erreur_sql)?;
                out.insert(id, ligne);
            }
        }
        Ok(out)
    })
}

/// Traduit une erreur SQLite sans publier la requête ni le chemin de la base.
fn erreur_sql(e: rusqlite::Error) -> ErreurSite {
    tracing::error!(erreur = %e, "lecture du miroir impossible");
    ErreurSite::Interne("lecture du gisement impossible".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_trois_formes_donnent_le_meme_identifiant() {
        let attendu = "0xF5E1E7CD";
        assert_eq!(normalize("0xF5E1E7CD").as_deref(), Some(attendu));
        assert_eq!(normalize("0xf5e1e7cd").as_deref(), Some(attendu));
        assert_eq!(normalize("F5E1E7CD").as_deref(), Some(attendu));
        assert_eq!(normalize("4125222861").as_deref(), Some(attendu));
        assert_eq!(normalize("  0xF5E1E7CD  ").as_deref(), Some(attendu));
    }

    #[test]
    fn ce_qui_n_est_pas_un_identifiant_est_ecarte_pas_devine() {
        // Preuve par falsification : sans cette moitie, un `normalize` qui rendrait toujours
        // `Some` passerait le test precedent.
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("   "), None);
        assert_eq!(normalize("mark-evans"), None);
        assert_eq!(normalize("0xZZZZ"), None);
        assert_eq!(normalize("-1"), None);
        // 2^32 : hors d'un u32, donc pas un identifiant de ce jeu.
        assert_eq!(normalize("4294967296"), None);
    }

    #[test]
    fn la_forme_du_miroir_est_respectee_au_caractere_pres() {
        // Le miroir stocke `0x` + 8 hexa MAJUSCULES, zero-remplis. Une casse ou un remplissage
        // different ferait rater le `WHERE id IN (...)` sans aucun message.
        assert_eq!(normalize("1").as_deref(), Some("0x00000001"));
        assert_eq!(normalize("0xff").as_deref(), Some("0x000000FF"));
        assert_eq!(normalize("4294967295").as_deref(), Some("0xFFFFFFFF"));
    }

    #[test]
    fn huit_chiffres_sans_prefixe_se_lisent_en_hexadecimal() {
        // Ambiguite reelle : `12345678` est un decimal valide ET un hexa valide. La longueur
        // tranche, comme chez le client de reference. Le documenter par un test evite qu'un
        // futur lot "corrige" ce comportement et decale tout un effectif.
        assert_eq!(normalize("12345678").as_deref(), Some("0x12345678"));
        // Sept chiffres : plus la meme forme, donc lu en decimal.
        assert_eq!(normalize("1234567").as_deref(), Some("0x0012D687"));
    }

    #[tokio::test]
    async fn le_contrat_dit_ce_que_la_route_ne_recoit_pas() {
        let c = contract().await.0;
        assert_eq!(c.id_forms.len(), 3);
        assert_eq!(c.ids_max, IDS_MAX);
        assert!(
            c.never_received.iter().any(|s| s.contains("sauvegarde")),
            "la route doit dire qu'aucun fichier de sauvegarde ne la traverse"
        );
    }
}
