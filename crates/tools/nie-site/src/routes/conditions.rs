//! `/api/v1/conditions` — le décodeur des **blobs de condition** du jeu.
//!
//! # Ce qu'est un blob de condition
//!
//! Le même blob base64 apparaît partout dans les données d'IEVR sous six noms différents :
//! `openCond` (galerie), `cond` (événements, déclencheurs `DATA_ITEM`), `condition`
//! (`trial_take_over`, archives de scènes), `runCond` (`soccer_drop`), `aocCondition`
//! (`add_content_equip`). C'est **un seul système d'expressions** — déblocages et
//! déclencheurs — et `nie-data` en porte les deux couches :
//!
//! - le **cadrage** ([`nie_data::cond`]), validé sur les **17 788 blobs réels** du corpus ;
//! - la **sémantique** ([`nie_data::unlock_condition`]), port 1:1 d'inagle, qui résout les
//!   seuils d'histoire et les event-flags par CRC32.
//!
//! # Pourquoi une route, et pas une famille de plus
//!
//! Ces deux modules ne prennent pas un `.cfg.bin` : ils prennent une **chaîne**, extraite d'un
//! champ d'un autre fichier. `decode_by_key(cle, root)` ne peut pas les appeler — sa signature
//! commence par un conteneur. La matrice de couverture les classait donc `manquant` alors que
//! le décodeur existait et était testé : c'était un manque de **route**, pas de code.
//!
//! Un consommateur d'`/api/v1/donnees/famille/gallery_config` reçoit aujourd'hui des `openCond`
//! bruts, en base64, qu'il ne sait pas lire. Cette route est la moitié manquante.
//!
//! # Nommage
//!
//! Identifiants, URLs et clés JSON en **anglais** (règle du 2026-09-06) ; commentaires en
//! français. Cf. `routes::text`.
//!
//! # Les routes
//!
//! | Route | Ce qu'elle rend |
//! |---|---|
//! | `GET /api/v1/conditions` | ce que le décodeur sait et ce qu'il ne sait pas, chiffré |
//! | `GET /api/v1/conditions/{blob}` | un blob décodé : cadrage **et** sémantique |
//!
//! # Ce que la réponse ne promet pas
//!
//! La charge utile de la version 1 (**forme liste**, 34 blobs sur 17 788) n'est **pas**
//! reversée : `framing.valid` vaut alors `false` et le champ `payload_hex` est rendu tel quel.
//! Publier une sémantique inventée pour ces 34 blobs coûterait plus cher que de dire qu'on ne
//! sait pas.

use axum::Json;
use axum::extract::Path;
use serde::Serialize;

use crate::error::ErreurSite;

/// Longueur maximale acceptée pour un blob, en caractères base64.
///
/// Mesurée, pas choisie : le plus long blob du corpus fait 35 octets décodés, soit 48
/// caractères. La borne est posée à 4 096 — deux ordres de grandeur au-dessus du réel, et
/// assez bas pour qu'une URL absurde soit refusée avant tout décodage.
pub const BLOB_MAX: usize = 4096;

/// Le cadrage décodé — la couche validée sur la totalité du corpus.
#[derive(Debug, Clone, Serialize)]
pub struct Framing {
    /// Version du format, lue en **big-endian** sur `b[0..4]` : `0` ou `1`.
    pub version: u32,
    /// Longueur déclarée du reste, `b[4]`.
    pub declared_length: u8,
    /// Opcode de l'expression, `b[5]`.
    pub opcode: u8,
    /// Taille de la charge utile brute, en octets.
    pub payload_bytes: usize,
    /// La charge utile, en hexadécimal — brute, ses clauses ne sont pas décodées ici.
    pub payload_hex: String,
    /// `true` si le cadrage version 0 est cohérent. `false` sur la forme liste (version 1),
    /// dont le cadrage interne n'est pas reversé.
    pub valid: bool,
}

/// Un event-flag exigé par la condition.
#[derive(Debug, Clone, Serialize)]
pub struct Requirement {
    /// Espace de noms de l'event-flag.
    pub namespace: u32,
    /// CRC32 (polynôme `0xEDB88320`) de l'identifiant d'événement.
    pub crc: u32,
    /// Nombre d'occurrences exigées (comparateur `>=`).
    pub count: u32,
    /// L'identifiant résolu, quand le reverse-lookup le connaît.
    pub event_id: Option<String>,
}

/// La sémantique décodée.
#[derive(Debug, Clone, Serialize)]
pub struct Semantics {
    /// Catégorie : `always`, `story`, `event_flag`, `composite`.
    pub kind: &'static str,
    /// Opérateur entre les feuilles : `none`, `single`, `and`.
    pub op: &'static str,
    /// Seuil de progression de l'histoire, s'il y en a un.
    pub story_threshold: Option<u32>,
    /// Numéro d'épisode déduit du seuil, s'il est déductible.
    pub story_episode: Option<u32>,
    /// Les event-flags exigés, combinés en ET.
    pub required_events: Vec<Requirement>,
}

/// La réponse de `GET /api/v1/conditions/{blob}`.
#[derive(Debug, Clone, Serialize)]
pub struct Decoded {
    /// Le blob tel qu'il a été reçu.
    pub raw: String,
    /// Le cadrage, ou `None` si le blob fait moins de 6 octets une fois décodé.
    pub framing: Option<Framing>,
    /// La sémantique. Un blob illisible donne `always` — c'est le comportement du jeu, pas un
    /// masquage d'erreur, et `framing: null` le signale au même endroit.
    pub semantics: Semantics,
}

/// Ce que la route sait faire, publié plutôt qu'affirmé.
#[derive(Debug, Clone, Serialize)]
pub struct Capabilities {
    /// Les six champs des données du jeu qui portent ce format.
    pub fields: &'static [&'static str],
    /// Les catégories que la sémantique distingue.
    pub kinds: &'static [&'static str],
    /// Les opérateurs reconnus.
    pub operators: &'static [&'static str],
    /// Les versions de cadrage lues, et ce qu'on en fait.
    pub framing_versions: Vec<FramingVersion>,
    /// Taille maximale acceptée, en caractères base64.
    pub blob_max_chars: usize,
}

/// Une version du cadrage, et l'état de son reverse.
#[derive(Debug, Clone, Serialize)]
pub struct FramingVersion {
    /// Le numéro de version lu en tête du blob.
    pub version: u32,
    /// Nombre de blobs de cette version dans le corpus mesuré (17 788 au total).
    pub corpus_count: u32,
    /// `true` si la structure interne est reversée.
    pub reversed: bool,
    /// Ce que la route en rend.
    pub served: &'static str,
}

/// Traduit la catégorie en une chaîne **choisie**.
///
/// Jamais `format!("{:?}")` : ce dépôt a déjà publié `"Some(V2)"` dans un JSON destiné à être
/// lu. Un champ public se `match`e.
const fn kind_of(k: nie_data::unlock_condition::UnlockType) -> &'static str {
    use nie_data::unlock_condition::UnlockType as T;
    match k {
        T::Always => "always",
        T::Story => "story",
        T::EventFlag => "event_flag",
        T::Composite => "composite",
    }
}

/// Traduit l'opérateur en une chaîne choisie, pour la même raison.
const fn op_of(o: nie_data::unlock_condition::UnlockOp) -> &'static str {
    use nie_data::unlock_condition::UnlockOp as O;
    match o {
        O::None => "none",
        O::Single => "single",
        O::And => "and",
    }
}

/// Rend les octets en hexadécimal minuscule, sans séparateur.
fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Décode un blob. Séparée du handler pour être testable sans HTTP.
///
/// # Errors
///
/// `Demande` (400) si le blob est vide ou plus long que [`BLOB_MAX`].
pub fn decode(blob: &str) -> Result<Decoded, ErreurSite> {
    if blob.is_empty() {
        return Err(ErreurSite::Demande(
            "blob vide : cette route decode une chaine base64 lue dans un champ du jeu \
             (openCond, cond, condition, runCond, aocCondition)"
                .to_owned(),
        ));
    }
    if blob.len() > BLOB_MAX {
        return Err(ErreurSite::Demande(format!(
            "blob de {} caracteres : la borne est {BLOB_MAX} (le plus long du corpus reel en \
             fait 48)",
            blob.len()
        )));
    }

    let framing = nie_data::cond::CondBlob::parse_base64(blob).map(|b| Framing {
        version: b.version,
        declared_length: b.declared_len,
        opcode: b.opcode,
        payload_bytes: b.payload.len(),
        payload_hex: to_hex(&b.payload),
        valid: b.framing_valid_v0(),
    });

    let u = nie_data::unlock_condition::decode_unlock_condition(blob);
    Ok(Decoded {
        raw: blob.to_owned(),
        framing,
        semantics: Semantics {
            kind: kind_of(u.kind),
            op: op_of(u.op),
            story_threshold: u.story_threshold,
            story_episode: u.story_episode,
            required_events: u
                .required_events
                .iter()
                .map(|e| Requirement {
                    namespace: e.namespace.0,
                    crc: e.crc,
                    count: e.count,
                    event_id: e.event_id.clone(),
                })
                .collect(),
        },
    })
}

/// `GET /api/v1/conditions` — ce que le décodeur sait, et ce qu'il ne sait pas.
pub async fn capabilities() -> Json<Capabilities> {
    Json(Capabilities {
        fields: &[
            "openCond",
            "cond",
            "condition",
            "runCond",
            "aocCondition",
            "cond3",
        ],
        kinds: &["always", "story", "event_flag", "composite"],
        operators: &["none", "single", "and"],
        framing_versions: vec![
            FramingVersion {
                version: 0,
                corpus_count: 17_754,
                reversed: true,
                served: "cadrage et semantique",
            },
            FramingVersion {
                version: 1,
                corpus_count: 34,
                reversed: false,
                served: "cadrage seul, forme liste non reversee : payload_hex brut",
            },
        ],
        blob_max_chars: BLOB_MAX,
    })
}

/// `GET /api/v1/conditions/{blob}` — un blob décodé.
///
/// # Errors
///
/// `400` sur un blob vide ou hors borne.
pub async fn condition(Path(blob): Path<String>) -> Result<Json<Decoded>, ErreurSite> {
    decode(&blob).map(Json)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Blob réel `openCond` de `gallery_config`, en base64 — celui des tests de `nie_data::cond`.
    const GALLERY_B64: &str = "AAAAAA8FNbkZNtoAAQAyAABOKnE=";

    #[test]
    fn un_blob_reel_rend_son_cadrage_et_sa_semantique() {
        let d = decode(GALLERY_B64).expect("blob valide");
        let f = d.framing.expect("20 octets : le cadrage existe");
        assert_eq!(f.version, 0);
        assert_eq!(f.declared_length, 0x0f);
        assert_eq!(f.opcode, 0x05);
        assert_eq!(f.payload_bytes, 14);
        assert!(f.valid, "cadrage v0 coherent sur un blob reel");
        assert_eq!(f.payload_hex.len(), 28, "2 caracteres par octet");
        // Preuve par falsification du sens : ce blob n'est PAS `always`, sinon la route
        // rendrait la meme chose sur n'importe quoi.
        assert_ne!(
            d.semantics.kind, "always",
            "un blob porteur d'un seuil ne doit pas se lire `always`"
        );
    }

    #[test]
    fn les_bornes_refusent_ce_qu_elles_doivent_refuser() {
        assert_eq!(decode("").unwrap_err().statut().as_u16(), 400);
        let trop_long = "A".repeat(BLOB_MAX + 1);
        assert_eq!(decode(&trop_long).unwrap_err().statut().as_u16(), 400);
        // Et la borne accepte ce qui est dedans : sans cette moitie, un `decode` qui refuserait
        // tout passerait le test precedent.
        assert!(decode(GALLERY_B64).is_ok());
    }

    #[test]
    fn un_base64_invalide_ne_pretend_pas_avoir_un_cadrage() {
        let d = decode("!!!pas du base64!!!").expect("la route ne rejette pas, elle constate");
        assert!(
            d.framing.is_none(),
            "un blob illisible n'a pas de cadrage, et la reponse doit le dire"
        );
        assert_eq!(d.semantics.kind, "always");
        assert_eq!(d.raw, "!!!pas du base64!!!");
    }

    #[test]
    fn les_enums_sortent_en_chaines_choisies_pas_en_debug() {
        use nie_data::unlock_condition::{UnlockOp, UnlockType};
        assert_eq!(kind_of(UnlockType::EventFlag), "event_flag");
        assert_eq!(op_of(UnlockOp::And), "and");
        // `format!("{:?}")` rendrait "EventFlag" et "And" — le nom Rust, pas un contrat.
        assert_ne!(kind_of(UnlockType::EventFlag), "EventFlag");
    }

    #[test]
    fn hex_sans_separateur_et_en_minuscules() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xab, 0xff]), "000fabff");
        assert_eq!(to_hex(&[]), "");
    }
}
