//! `unlock_condition` — décodeur des conditions de déblocage (`openCond` de `gallery_config`,
//! `condition` de `scene_archive_config`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/unlock-condition.ts`
//!   (`decodeUnlockCondition`, `tokenizeLeaves`, `storyThresholdToEpisode`,
//!   `buildEventCrcLookup`) — **port 1:1**.
//! - Vecteurs golden : `packages/inagle/src/parsers/unlock-condition.test.ts` (fixtures réelles
//!   extraites de `gallery_config` / `scene_archive_config` — repris tels quels dans
//!   `tests/unlock_condition_golden.rs`).
//!
//! ## Format du blob (base64 → octets)
//!
//! ```text
//! [00 00 00 00]      en-tête nul (4 octets)
//! [len:1]            longueur du reste
//! [OPCODE racine:1]  0x3F trivial / 0x0B,0x17 composé (AND) / autre = feuille simple
//! [corps…]           tokens (à partir de l'octet 6)
//! ```
//!
//! Tokens du corps : `0x35 <ns:4 BE>` (début de feuille, namespace) ; `0x34 <val:4 BE>`
//! (valeur taguée = CRC32 de l'event_id) ; `0x32 <cmp:4 BE>` (comparateur `>=` ; **premier
//! seulement** par feuille). Tout autre octet est ignoré.
//!
//! Namespace `0xB91936DA` = progression de l'histoire (le comparateur EST le seuil ; grille
//! `ev01 = 20010`, `+10000` par épisode). Tout autre namespace = event-flag identifié par CRC32
//! (poly `0xEDB88320`). Plusieurs feuilles (opcode `0x0B`/`0x17`) = combinaison **AND**. Opcode
//! `0x3F` (ou aucune feuille) = condition triviale (toujours débloqué).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::hash::HashId;

/// Namespace de la progression de l'histoire (le comparateur est un seuil BE).
pub const STORY_NAMESPACE: u32 = 0xB919_36DA;
/// Seuil de progression du 1ᵉʳ épisode (`ev01`).
pub const STORY_EPISODE_BASE: u32 = 20010;
/// Incrément de seuil entre deux épisodes successifs.
pub const STORY_EPISODE_STEP: u32 = 10000;

const TOKEN_LEAF: u8 = 0x35;
const TOKEN_VALUE: u8 = 0x34;
const TOKEN_COMPARE: u8 = 0x32;
const ROOT_TRIVIAL: u8 = 0x3F;

/// Opérateur de combinaison entre les feuilles d'une condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UnlockOp {
    /// Aucune feuille.
    None,
    /// Une seule feuille.
    Single,
    /// Plusieurs feuilles combinées en ET logique.
    And,
}

/// Catégorie d'une condition de déblocage décodée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UnlockType {
    /// Aucune exigence (opcode trivial / blob vide).
    Always,
    /// Uniquement un seuil de progression de l'histoire.
    Story,
    /// Uniquement des event-flags.
    EventFlag,
    /// Seuil story + event-flags combinés.
    Composite,
}

/// Une exigence d'event-flag (event accompli au moins `count` fois).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RequiredEvent {
    /// Namespace de l'event-flag (ex. `0x2A3D4543`).
    pub namespace: HashId,
    /// CRC32 (poly `0xEDB88320`) de l'`event_id`, non signé.
    pub crc: u32,
    /// Nombre d'occurrences requises (comparateur `>=`).
    pub count: u32,
    /// `event_id` résolu via le reverse-lookup, si disponible.
    pub event_id: Option<String>,
}

impl RequiredEvent {
    /// CRC32 au format hex `0xABCD1234` (équivalent `crcHex` d'inagle).
    #[must_use]
    pub fn crc_hex(&self) -> String {
        HashId(self.crc).to_hex()
    }
}

/// Condition de déblocage décodée (port de `UnlockCondition`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnlockCondition {
    /// Catégorie de la condition.
    pub kind: UnlockType,
    /// Opérateur entre les feuilles.
    pub op: UnlockOp,
    /// Seuil de progression de l'histoire (BE), si présent.
    pub story_threshold: Option<u32>,
    /// Numéro d'épisode déduit du seuil (`ev01 = 1`), si applicable.
    pub story_episode: Option<u32>,
    /// Event-flags requis (combinés en AND), si présents.
    pub required_events: Vec<RequiredEvent>,
    /// Le blob base64 d'origine (conservé pour debug/round-trip).
    pub raw: String,
}

impl UnlockCondition {
    /// Condition « toujours débloqué » (blob vide/trivial), conservant le `raw`.
    fn always(raw: String) -> Self {
        UnlockCondition {
            kind: UnlockType::Always,
            op: UnlockOp::None,
            story_threshold: None,
            story_episode: None,
            required_events: Vec::new(),
            raw,
        }
    }

    /// Annote chaque event-flag avec son `event_id` via une map `CRC32 → event_id`.
    pub fn resolve_events(&mut self, lookup: &BTreeMap<u32, String>) {
        for e in &mut self.required_events {
            if let Some(id) = lookup.get(&e.crc) {
                e.event_id = Some(id.clone());
            }
        }
    }
}

/// Feuille brute du corps du blob.
struct Leaf {
    ns: u32,
    value: Option<u32>,
    compare: Option<u32>,
}

/// Déduit le numéro d'épisode (1-based) depuis un seuil de progression.
///
/// Les seuils non alignés sur la grille des épisodes renvoient `None` (port de
/// `storyThresholdToEpisode`).
#[must_use]
pub fn story_threshold_to_episode(threshold: u32) -> Option<u32> {
    if threshold < STORY_EPISODE_BASE {
        return None;
    }
    let delta = threshold - STORY_EPISODE_BASE;
    if !delta.is_multiple_of(STORY_EPISODE_STEP) {
        return None;
    }
    Some(delta / STORY_EPISODE_STEP + 1)
}

/// Lit un `u32` big-endian à `buf[i..i+4]`, ou `None` si hors limites.
fn read_be_u32(buf: &[u8], i: usize) -> Option<u32> {
    let b = buf.get(i..i + 4)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Tokenise le corps du blob en feuilles brutes (port de `tokenizeLeaves`).
fn tokenize_leaves(buf: &[u8]) -> Vec<Leaf> {
    let mut leaves: Vec<Leaf> = Vec::new();
    let mut current: Option<Leaf> = None;
    let mut i = 6; // saute [00 00 00 00][len][root]

    while i < buf.len() {
        let token = buf[i];
        if token == TOKEN_LEAF && i + 5 <= buf.len() {
            if let Some(c) = current.take() {
                leaves.push(c);
            }
            current = Some(Leaf {
                ns: read_be_u32(buf, i + 1).unwrap_or(0),
                value: None,
                compare: None,
            });
            i += 5;
            continue;
        }
        if token == TOKEN_VALUE
            && i + 5 <= buf.len()
            && let Some(c) = current.as_mut()
        {
            c.value = read_be_u32(buf, i + 1);
            i += 5;
            continue;
        }
        // Premier comparateur de la feuille uniquement.
        if token == TOKEN_COMPARE
            && i + 5 <= buf.len()
            && let Some(c) = current.as_mut()
            && c.compare.is_none()
        {
            c.compare = read_be_u32(buf, i + 1);
            i += 5;
            continue;
        }
        i += 1;
    }
    if let Some(c) = current {
        leaves.push(c);
    }
    leaves
}

/// Décode une condition de déblocage depuis un blob d'octets déjà dé-base64é.
///
/// `raw` est conservé tel quel dans le résultat (pour le round-trip / debug).
#[must_use]
pub fn decode_unlock_condition_bytes(buf: &[u8], raw: String) -> UnlockCondition {
    if buf.len() < 6 {
        return UnlockCondition::always(raw);
    }

    let root = buf[5];
    let leaves = tokenize_leaves(buf);

    if root == ROOT_TRIVIAL || leaves.is_empty() {
        return UnlockCondition::always(raw);
    }

    let mut story_threshold: Option<u32> = None;
    let mut required_events: Vec<RequiredEvent> = Vec::new();

    for leaf in &leaves {
        if leaf.ns == STORY_NAMESPACE {
            // Pour le story, la valeur de comparaison EST le seuil.
            if let Some(cmp) = leaf.compare {
                story_threshold = Some(cmp);
            }
            continue;
        }
        let crc = leaf.value.unwrap_or(0);
        required_events.push(RequiredEvent {
            namespace: HashId(leaf.ns),
            crc,
            count: leaf.compare.unwrap_or(1),
            event_id: None,
        });
    }

    let has_story = story_threshold.is_some();
    let has_events = !required_events.is_empty();

    let kind = match (has_story, has_events) {
        (true, true) => UnlockType::Composite,
        (true, false) => UnlockType::Story,
        (false, true) => UnlockType::EventFlag,
        (false, false) => UnlockType::Always,
    };

    let leaf_count = usize::from(has_story) + required_events.len();
    let op = match leaf_count {
        0 => UnlockOp::None,
        1 => UnlockOp::Single,
        _ => UnlockOp::And,
    };

    let story_episode = story_threshold.and_then(story_threshold_to_episode);

    UnlockCondition {
        kind,
        op,
        story_threshold,
        story_episode,
        required_events,
        raw,
    }
}

/// Décode une condition de déblocage encodée en base64 (port de `decodeUnlockCondition`).
///
/// Une entrée vide ou invalide donne une condition `Always`.
#[must_use]
pub fn decode_unlock_condition(encoded: &str) -> UnlockCondition {
    match decode_base64(encoded) {
        Some(buf) => decode_unlock_condition_bytes(&buf, String::from(encoded)),
        None => UnlockCondition::always(String::from(encoded)),
    }
}

/// Construit une map `CRC32 → event_id` depuis une liste d'`event_id` connus
/// (port de `buildEventCrcLookup`).
#[must_use]
pub fn build_event_crc_lookup<'a, I: IntoIterator<Item = &'a str>>(
    event_ids: I,
) -> BTreeMap<u32, String> {
    let mut map = BTreeMap::new();
    for id in event_ids {
        if id.is_empty() {
            continue;
        }
        map.insert(crc32_str(id), String::from(id));
    }
    map
}

/// CRC32 (poly `0xEDB88320`, init `0xFFFFFFFF`, complément final) des octets UTF-8 d'une chaîne.
///
/// Équivalent de `crc32String` d'inagle (`hash/crc32.ts`, poly standard).
#[must_use]
pub fn crc32_str(s: &str) -> u32 {
    const POLY: u32 = 0xEDB8_8320;
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in s.as_bytes() {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ POLY
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Décodeur base64 standard (alphabet `A-Za-z0-9+/`, padding `=`), sans dépendance.
///
/// Retourne `None` si l'entrée est vide ou contient un caractère invalide.
///
/// `pub(crate)` depuis le 2026-09-06 : [`crate::cond`] décode le **cadrage** du même blob que
/// ce module décode sémantiquement, et les deux couches doivent partir des mêmes octets. Une
/// seconde implémentation de base64 dans le dépôt serait exactement le doublon que la
/// doctrine interdit — cf. [`crate::cond::CondBlob::parse_base64`].
pub(crate) fn decode_base64(value: &str) -> Option<Vec<u8>> {
    if value.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(value.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut nbits = 0u32;
    for &c in value.as_bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\n' | b'\r' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    Some(out)
}
