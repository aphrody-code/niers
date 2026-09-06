//! Parseur du corps du blob **HEADERSAVE** (2045 octets).
//!
//! ## Méthode anti-hallucination
//!
//! Chaque champ est adossé à une correspondance octets↔valeur sur la save réelle
//! `002AB8F4-USERDATALIVE` (VPS, déchiffré et validé par `nie-save::parse` ✓ magic + CRC32).
//! Les champs dont l'interprétation n'est pas certifiée sont marqués **OPAQUE**.
//!
//! ## Format HEADERSAVE body (2045 octets)
//!
//! ```text
//! [0x0000..0x0003] format_version  u32 BE = 1
//!                  preuve : octets 00 00 00 01 ↔ valeur 1 ✓
//!
//! [0x0004..0x0007] opaque_u32      u32 LE (OPAQUE : valeur réelle 0xB264BCB7)
//!                  signification inconnue — candidat checksum interne engine
//!
//! [0x0008..0x000B] max_slots       u32 LE = 10
//!                  preuve : octets 0A 00 00 00 ↔ valeur 10 ✓
//!
//! [0x000C..0x000F] used_slots      u32 LE = 7
//!                  preuve : octets 07 00 00 00 ↔ valeur 7 ✓
//!
//! [0x0010..0x001D] pre_slot_opaque [14 octets] OPAQUE (métadonnées de section inconnues)
//!
//! [0x001E..0x0586] slot_array : 28 × 41 octets (2 sections × 14 enregistrements)
//!   Chaque enregistrement de 41 octets :
//!     [+0..+3]   hash_outer  = 0xC8A77417 (constant, "slot_record" field ID)
//!     [+4]       inner_type  = 0x0D (constant)
//!     [+5..+6]   padding zero
//!     [+7]       section_role : 0x03 = premier slot d'une section,
//!                               0x04 = slot régulier
//!     [+8..+11]  hash_inner2 : 0x8480C2C1 (type A) ou 0x0877F0CD (type B)
//!
//!   Type A (hash_inner2 == 0x8480C2C1) :
//!     [+12..+15] slot_chapter   u32 LE = numéro de chapitre (valeur 4 sur save réelle)
//!     [+16..+19] slot_variant   u32 LE = hash per-slot (≠ 0 sur slots actifs)
//!     [+20..+23] hash_inner3    = 0x07F65D6B (constant)
//!     [+24..+27] slot_value3    u32 LE (4 = chapitre, 0x32 = progression?)
//!     [+28..+31] padding zero
//!     [+32..+35] hash_inner4    = 0x1E15D868 (constant)
//!     [+36..+39] slot_value4    u32 LE (0 ou 1)
//!     [+40]      is_active      u8 (0 = inactif, 1 = actif)
//!
//!   Type B (hash_inner2 == 0x0877F0CD) — slot avec horodatage :
//!     [+12..+15] datetime_len   u32 LE = 8 (toujours)
//!     [+16..+23] datetime_data  : u16_LE year, u8 month, day, hour, min, sec, u8 zero
//!     [+24..+27] hash_playtime  = 0x64916694 (champ temps de jeu)
//!     [+28..+31] playtime_len   u32 LE = 4
//!     [+32..+35] playtime_secs  u32 LE (secondes de jeu, ex. 324919 = 90h15m)
//!     [+36..+39] hash_inner4    = 0xECB44D39 (constant)
//!     [+40]      slot_flag      u8 (0 = slot horodatage vide, 1 = slot actif)
//!
//! [0x0587..0x0663] section de champs globaux OPAQUE (métadonnées engine TLV)
//!                  contient plusieurs enregistrements [hash4][len4][data] + blocs imbriqués
//!
//! [0x0664..0x066B] dernier enregistrement scalaire avant section joueur
//!                  hash=0x8D4978FB, len=1, val=0x01
//!
//! [0x066C..0x0673] en-tête de section joueur
//!                  [0x066C..0x066F] hash=0x28677E98 (section_type "player_header")
//!                  [0x0670..0x0673] type/padding/sous-type [02 00 00 04]
//!
//! [0x0674..0x067B] conteneur [hash=0x9F726A26][count=0x00000001]
//!
//! [0x067C..0x068B] enregistrement save_timestamp :
//!                  hash=0x0877F0CD, len=8,
//!                  data = u16_LE year, u8 month, day, hour, min, sec, u8 zero
//!                  preuve : octets EA 07 02 15 0D 0F 16 00 ↔ 2026-02-21T13:15:22 ✓
//!
//! [0x068C..0x06E1] autres champs scalaires OPAQUE (valeurs zéro sur save réelle)
//!
//! [0x06E2..0x06E9] conteneur [hash=0x9F726A26][count=0x00000002]
//!
//! [0x06EA..0x06F1] sous-conteneur [hash=0x519B0530][count=0x00000001]
//!
//! [0x06F2..0x071A] enregistrement player_name :
//!                  hash=0xD5499347, len=33,
//!                  data = buffer 33 octets : player_name (null-term) ++ level_str (null-term) ++ padding
//!                  preuve : "AstraJinWoo\0218\0" à partir de 0x06FA ↔ nom/niveau ✓
//!
//! [0x071B..0x0743] enregistrement unique_id :
//!                  hash=0x76B7C825, len=33,
//!                  data = buffer 33 octets : unique_id ASCII hex (null-term)
//!                  preuve : "00023798fc9b4d49b9a36b6b0cb8d50b\0" à partir de 0x0723 ✓
//!
//! [0x0744..0x07FC] champs supplémentaires OPAQUE (progression, team, flags)
//!
//! [0x07FC..0x07FC] 1 octet de terminaison (fin de body 2045 octets)
//! ```
//!
//! ## Invariants vérifiés sur save réelle (002AB8F4-USERDATALIVE, VPS)
//!
//! - `format_version == 1` (u32 BE à l'offset 0x0000)
//! - `max_slots == 10` (u32 LE à l'offset 0x0008)
//! - `used_slots <= max_slots` (7 ≤ 10 ✓)
//! - 28 enregistrements slot (hash 0xC8A77417) à stride exact 41 octets depuis 0x001E
//! - `player_name == "AstraJinWoo"` (octets 0x06FA..0x0704, ancre confirmée)
//! - `level_str == "218"` (octets 0x0705..0x0707, ancre confirmée)
//! - `unique_id == "00023798fc9b4d49b9a36b6b0cb8d50b"` (octets 0x0723..0x0742, ancre confirmée)

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

extern crate alloc;
use alloc::{string::String, vec::Vec};

use crate::SaveError;

// ---------------------------------------------------------------------------
// Constantes validées sur octets réels (002AB8F4-USERDATALIVE)
// ---------------------------------------------------------------------------

/// Hash du champ « slot_record » : identifie chaque enregistrement de la table de slots.
/// Valeur : 0xC8A77417 (little-endian). Confirmé sur 28 occurrences à stride 41.
const HASH_SLOT_RECORD: u32 = 0x1774_A7C8;

/// Type B : hash « slot_datetime » (champ horodatage dans les slots de type B).
const HASH_SLOT_DATETIME: u32 = 0x0877_F0CD;

/// Hash du champ « save_timestamp » dans la section joueur.
const HASH_SAVE_TIMESTAMP: u32 = 0x0877_F0CD;

/// Hash du champ « player_name » (buffer 33 octets : name + level_str).
const HASH_PLAYER_NAME: u32 = 0xD549_9347;

/// Hash du champ « unique_id » (buffer 33 octets, UUID hexadécimal ASCII).
const HASH_UNIQUE_ID: u32 = 0x76B7_C825;

/// Longueur en octets de chaque enregistrement de slot.
const SLOT_STRIDE: usize = 41;

/// Offset du premier enregistrement de slot dans le body.
const SLOT_ARRAY_OFFSET: usize = 0x001E;

/// Nombre maximal de slots supportés par ce parseur.
const MAX_SLOTS_SUPPORTED: usize = 28;

/// Longueur du buffer « player_name+level_str » (octets).
const PLAYER_BUF_LEN: usize = 33;

/// Longueur du buffer « unique_id » (octets).
const UID_BUF_LEN: usize = 33;

/// Longueur minimale du body HEADERSAVE pour extraire les champs de base.
pub const HEADERSAVE_MIN_LEN: usize = 0x0744;

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

/// Horodatage décodé depuis le format natif IEVR (8 octets).
///
/// Stocké en little-endian : u16 year, puis 5 octets individuels.
/// Tous les champs sont extraits sans validation calendaire (le parseur
/// restitue la valeur brute du fichier).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NativeDateTime {
    /// Année (u16 LE, ex. 2026).
    pub year: u16,
    /// Mois (1–12).
    pub month: u8,
    /// Jour du mois (1–31).
    pub day: u8,
    /// Heure (0–23).
    pub hour: u8,
    /// Minute (0–59).
    pub minute: u8,
    /// Seconde (0–59).
    pub second: u8,
}

impl NativeDateTime {
    /// Indique si cet horodatage est « non défini » (tous les champs à zéro).
    #[must_use]
    pub fn is_unset(&self) -> bool {
        self.year == 0
            && self.month == 0
            && self.day == 0
            && self.hour == 0
            && self.minute == 0
            && self.second == 0
    }
}

/// Métadonnées d'un slot de sauvegarde individuel.
///
/// Chaque body HEADERSAVE contient 28 enregistrements de slot (2 × 14),
/// identifiés par le hash `0xC8A77417` à stride 41 octets.
///
/// ## Champs validés sur octets réels
///
/// - `section_role` : 0x03 = premier slot d'une section, 0x04 = slot régulier.
/// - `is_active` : 1 = slot possède des données de sauvegarde effectives.
/// - `slot_variant` : valeur per-slot variable (hash de contenu) pour les slots actifs de type A.
/// - `slot_datetime` : horodatage embarqué dans les slots de type B.
/// - `playtime_secs` : temps de jeu en secondes (slot de type B actif uniquement).
#[derive(Debug, Clone)]
pub struct SlotMeta {
    /// Rôle du slot dans sa section (0x03 = premier, 0x04 = régulier).
    pub section_role: u8,

    /// Hash interne du type de champ secondaire (0x8480C2C1 ou 0x0877F0CD).
    /// Exposé pour diagnostic ; la distinction type-A/type-B en dépend.
    pub inner_hash: u32,

    /// Valeur per-slot variant (type A uniquement, 0 si slot vide).
    ///
    /// OPAQUE : signification exacte inconnue (candidat : hash de données de slot).
    pub slot_variant: u32,

    /// Horodatage embarqué dans ce slot (type B uniquement).
    ///
    /// `None` si le slot est de type A, ou si `slot_datetime.is_unset()`.
    pub slot_datetime: Option<NativeDateTime>,

    /// Temps de jeu en secondes (type B, slot actif uniquement).
    ///
    /// `None` si le slot est de type A.
    pub playtime_secs: Option<u32>,

    /// Indique si ce slot est actif (byte[40] du record = 1).
    pub is_active: bool,

    /// Octets bruts du record (41 octets) pour accès complet aux champs OPAQUE.
    pub raw: [u8; 41],
}

/// Résultat du parseur HEADERSAVE body.
///
/// Seuls les champs **confirmés sur octets réels** sont représentés.
/// Les régions non décodées restent accessibles via `body_opaque`.
///
/// # Ancres validées (002AB8F4-USERDATALIVE)
///
/// ```text
/// format_version == 1 ✓
/// max_slots      == 10 ✓
/// used_slots     == 7  ✓
/// save_timestamp == 2026-02-21T13:15:22 ✓
/// player_name    == "AstraJinWoo" ✓
/// level_str      == "218" ✓
/// unique_id      == "00023798fc9b4d49b9a36b6b0cb8d50b" ✓
/// ```
#[derive(Debug, Clone)]
pub struct HeaderSave {
    /// Version de format (u32 BE à l'offset 0x0000, toujours 1).
    pub format_version: u32,

    /// Valeur 32-bit opaque à l'offset 0x0004 (LE).
    ///
    /// OPAQUE : candidat checksum interne engine (0xB264BCB7 sur save VPS).
    pub opaque_u32: u32,

    /// Nombre maximal de slots supportés par cette save (u32 LE, offset 0x0008).
    pub max_slots: u32,

    /// Nombre de slots utilisés (u32 LE, offset 0x000C).
    pub used_slots: u32,

    /// Table des métadonnées de slots (28 enregistrements × 41 octets).
    ///
    /// Contient à la fois les slots vides et actifs des deux sections.
    pub slots: Vec<SlotMeta>,

    /// Horodatage de la dernière sauvegarde (depuis la section joueur, hash 0x0877F0CD).
    ///
    /// Offset absolu 0x0684 sur save connue. Priorité sur les horodatages des slots
    /// individuels pour représenter « quand la save a été créée ».
    pub save_timestamp: NativeDateTime,

    /// Nom du joueur (null-terminé dans un buffer 33 octets, hash 0xD5499347).
    ///
    /// Offset absolu 0x06FA sur save connue. Ancre validée : "AstraJinWoo".
    pub player_name: String,

    /// Niveau du joueur sous forme de chaîne ASCII (ex. "218").
    ///
    /// Stocké immédiatement après le null-terminateur de `player_name` dans le
    /// même buffer 33 octets. Ancre validée : "218".
    pub level_str: String,

    /// Identifiant unique de la save sous forme hexadécimale ASCII 32 caractères.
    ///
    /// Buffer 33 octets, null-terminé (hash 0x76B7C825).
    /// Offset absolu 0x0723 sur save connue.
    /// Ancre validée : "00023798fc9b4d49b9a36b6b0cb8d50b".
    pub unique_id: String,

    /// Octets bruts du body complet pour accès aux régions OPAQUE.
    pub body_opaque: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Utilitaires internes
// ---------------------------------------------------------------------------

/// Lit un u32 en little-endian à l'offset `off` dans `b`.
#[inline]
fn read_u32_le(b: &[u8], off: usize) -> Result<u32, SaveError> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(SaveError::TooShort {
            got: b.len(),
            need: off + 4,
        })
}

/// Lit un u32 en big-endian à l'offset `off` dans `b`.
#[inline]
fn read_u32_be(b: &[u8], off: usize) -> Result<u32, SaveError> {
    b.get(off..off + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(SaveError::TooShort {
            got: b.len(),
            need: off + 4,
        })
}

/// Décode 8 octets au format horodatage natif IEVR.
///
/// Format : `[u16_LE year][u8 month][u8 day][u8 hour][u8 min][u8 sec][u8 zero]`
fn decode_datetime(data: &[u8]) -> Result<NativeDateTime, SaveError> {
    if data.len() < 8 {
        return Err(SaveError::TooShort {
            got: data.len(),
            need: 8,
        });
    }
    Ok(NativeDateTime {
        year: u16::from_le_bytes([data[0], data[1]]),
        month: data[2],
        day: data[3],
        hour: data[4],
        minute: data[5],
        second: data[6],
    })
}

/// Extrait une chaîne null-terminée depuis un buffer de longueur `buf_len`
/// à l'offset `off` dans `b`. Retourne la chaîne tronquée au premier `\0`.
fn read_null_term_string(b: &[u8], off: usize, buf_len: usize) -> Result<String, SaveError> {
    let end = off + buf_len;
    if end > b.len() {
        return Err(SaveError::TooShort {
            got: b.len(),
            need: end,
        });
    }
    let raw = &b[off..end];
    let nul_pos = raw.iter().position(|&c| c == 0).unwrap_or(buf_len);
    let bytes = &raw[..nul_pos];
    // Les chaînes de la section joueur sont ASCII (noms IEVR, hex UUID).
    let s = bytes.iter().map(|&c| c as char).collect::<String>();
    Ok(s)
}

/// Recherche un enregistrement TLV [hash4_LE][len4_LE][data] dans `body`
/// à partir de `start_off` avec le hash `target_hash`.
///
/// Retourne l'offset absolu du premier octet de `data` si trouvé.
/// Limite la recherche à `max_scan` octets après `start_off`.
fn find_tlv_data_offset(
    body: &[u8],
    start_off: usize,
    target_hash: u32,
    max_scan: usize,
) -> Option<usize> {
    let end = (start_off + max_scan).min(body.len().saturating_sub(8));
    let mut pos = start_off;
    while pos + 8 <= end {
        if let (Ok(h), Ok(len)) = (read_u32_le(body, pos), read_u32_le(body, pos + 4)) {
            let data_start = pos + 8;
            if h == target_hash && len <= 128 && data_start + (len as usize) <= body.len() {
                return Some(data_start);
            }
        }
        pos += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Point d'entrée public
// ---------------------------------------------------------------------------

/// Parse le **body** d'un blob HEADERSAVE (2045 octets déchiffrés).
///
/// ## Sécurité
///
/// - Aucun `unsafe`, aucun `unwrap`, aucun `expect`.
/// - Toutes les lectures sont vérifiées par les bornes du slice.
/// - Un body court retourne [`SaveError::TooShort`] sans paniquer.
///
/// ## Champs extraits
///
/// Voir [`HeaderSave`] pour la liste complète avec références aux offsets.
///
/// # Errors
///
/// - [`SaveError::TooShort`] si le body est inférieur à 0x0744 octets.
/// - [`SaveError::Corrupt`] si les invariants structurels sont violés.
///
/// # Examples
///
/// ```rust,no_run
/// # use nie_save::body::headersave::parse_headersave;
/// # let body: &[u8] = &[];
/// # use nie_save::SaveError;
/// match parse_headersave(body) {
///     Ok(hs) => {
///         println!("joueur={} niveau={}", hs.player_name, hs.level_str);
///     }
///     Err(e) => eprintln!("erreur: {e}"),
/// }
/// ```
pub fn parse_headersave(body: &[u8]) -> Result<HeaderSave, SaveError> {
    // -----------------------------------------------------------------------
    // 1. Vérification de la taille minimale
    // -----------------------------------------------------------------------
    if body.len() < HEADERSAVE_MIN_LEN {
        return Err(SaveError::TooShort {
            got: body.len(),
            need: HEADERSAVE_MIN_LEN,
        });
    }

    // -----------------------------------------------------------------------
    // 2. En-tête global (offsets 0x0000–0x000F, vérifiés sur octets réels)
    // -----------------------------------------------------------------------
    let format_version = read_u32_be(body, 0x0000)?;
    let opaque_u32 = read_u32_le(body, 0x0004)?;
    let max_slots = read_u32_le(body, 0x0008)?;
    let used_slots = read_u32_le(body, 0x000C)?;

    // Vérification de cohérence : format_version == 1 sur toutes les saves connues.
    if format_version != 1 {
        return Err(SaveError::Corrupt("format_version != 1"));
    }

    if used_slots > max_slots {
        return Err(SaveError::Corrupt("used_slots > max_slots"));
    }

    // -----------------------------------------------------------------------
    // 3. Table de slots — scan par hash (deux groupes séparés par un gap)
    //
    //    Le body contient 2 groupes de 14 enregistrements, chacun à stride 41.
    //    Les groupes ne sont PAS contigus : gap de ~237 octets entre la fin du
    //    groupe 1 (offset 0x025C) et le début du groupe 2 (offset 0x0349).
    //    Un parcours séquentiel s'arrêterait au gap ; on scanne par hash.
    // -----------------------------------------------------------------------
    let mut slots: Vec<SlotMeta> = Vec::new();
    {
        // Borne haute empirique : 0x0590 = après le dernier slot connu (0x055e+41=0x0587).
        let scan_limit = 0x0590_usize.min(body.len());
        let mut scan = SLOT_ARRAY_OFFSET;
        let mut prev_slot_off: Option<usize> = None;

        while scan + SLOT_STRIDE <= scan_limit && slots.len() < MAX_SLOTS_SUPPORTED {
            let maybe_hash = read_u32_le(body, scan).unwrap_or(0);

            if maybe_hash == HASH_SLOT_RECORD && scan + SLOT_STRIDE <= body.len() {
                // Vérification de stride : stride exact 41 depuis le précédent
                // OU début d'un nouveau groupe (gap quelconque > SLOT_STRIDE).
                // stride exact (41) ou saut inter-groupes (delta >= 41) depuis le slot précédent
                let stride_ok = prev_slot_off.is_none_or(|p| scan - p >= SLOT_STRIDE);

                if stride_ok {
                    prev_slot_off = Some(scan);
                    let rec = &body[scan..scan + SLOT_STRIDE];
                    let section_role = rec[7];
                    let inner_hash = read_u32_le(rec, 8)?;

                    let slot = if inner_hash == HASH_SLOT_DATETIME {
                        // Type B : horodatage + playtime
                        // datetime_data à rec[16..24]
                        let dt = decode_datetime(&rec[16..24])?;
                        let slot_datetime = if dt.is_unset() { None } else { Some(dt) };
                        // playtime_secs à rec[32..36]
                        let playtime_secs = Some(read_u32_le(rec, 32)?);
                        let is_active = rec[40] != 0;
                        let mut raw = [0u8; 41];
                        raw.copy_from_slice(rec);
                        SlotMeta {
                            section_role,
                            inner_hash,
                            slot_variant: 0,
                            slot_datetime,
                            playtime_secs,
                            is_active,
                            raw,
                        }
                    } else {
                        // Type A : chapter + variant hash
                        // slot_variant à rec[16..20]
                        let slot_variant = read_u32_le(rec, 16)?;
                        let is_active = rec[40] != 0;
                        let mut raw = [0u8; 41];
                        raw.copy_from_slice(rec);
                        SlotMeta {
                            section_role,
                            inner_hash,
                            slot_variant,
                            slot_datetime: None,
                            playtime_secs: None,
                            is_active,
                            raw,
                        }
                    };

                    slots.push(slot);
                    // Avancer d'un stride exact pour rester aligné sur le groupe courant.
                    scan += SLOT_STRIDE;
                    continue;
                }
            }

            // Byte courant ne correspond pas à un slot valide : avancer d'un octet.
            scan += 1;
        }
    }

    // -----------------------------------------------------------------------
    // 4. Section joueur — horodatage de sauvegarde (offset fixe 0x0684)
    //
    //    L'horodatage est le premier champ de données après le conteneur
    //    external [hash=0x9F726A26][count=0x00000001] à 0x0674.
    //    Le TLV [hash=0x0877F0CD][len=8] se situe à 0x067C, data à 0x0684.
    //    Offset vérifié sur octets réels : EA 07 02 15 0D 0F 16 00 = 2026-02-21T13:15:22 ✓
    // -----------------------------------------------------------------------
    let save_timestamp = {
        // Offset fixe validé ; on effectue quand même une recherche de sécurité
        // si l'offset canonique ne porte pas le bon hash.
        const CANONICAL_TS_DATA_OFF: usize = 0x0684;
        const CANONICAL_TS_HASH_OFF: usize = 0x067C;
        const TS_SCAN_START: usize = 0x0560;
        const TS_SCAN_MAX: usize = 512;

        let data_off = if body.len() >= CANONICAL_TS_HASH_OFF + 4 {
            match read_u32_le(body, CANONICAL_TS_HASH_OFF) {
                Ok(h) if h == HASH_SAVE_TIMESTAMP => CANONICAL_TS_DATA_OFF,
                _ => {
                    // Fallback : recherche par hash
                    find_tlv_data_offset(body, TS_SCAN_START, HASH_SAVE_TIMESTAMP, TS_SCAN_MAX)
                        .ok_or(SaveError::Corrupt("hash save_timestamp introuvable"))?
                }
            }
        } else {
            find_tlv_data_offset(body, TS_SCAN_START, HASH_SAVE_TIMESTAMP, TS_SCAN_MAX)
                .ok_or(SaveError::Corrupt("hash save_timestamp introuvable"))?
        };

        if data_off + 8 > body.len() {
            return Err(SaveError::TooShort {
                got: body.len(),
                need: data_off + 8,
            });
        }
        decode_datetime(&body[data_off..data_off + 8])?
    };

    // -----------------------------------------------------------------------
    // 5. Nom du joueur + niveau (buffer 33 octets, offset fixe 0x06FA)
    //
    //    Contenu du TLV [hash=0xD5499347][len=33] à 0x06F2 :
    //    player_name null-terminé + level_str null-terminé + padding.
    //    Ancres : "AstraJinWoo" à 0x06FA, "218" à 0x0705 ✓
    // -----------------------------------------------------------------------
    let (player_name, level_str) = {
        const CANONICAL_NAME_DATA_OFF: usize = 0x06FA;
        const CANONICAL_NAME_HASH_OFF: usize = 0x06F2;
        const NAME_SCAN_START: usize = 0x06A0;
        const NAME_SCAN_MAX: usize = 256;

        let data_off = if body.len() >= CANONICAL_NAME_HASH_OFF + 4 {
            match read_u32_le(body, CANONICAL_NAME_HASH_OFF) {
                Ok(h) if h == HASH_PLAYER_NAME => CANONICAL_NAME_DATA_OFF,
                _ => find_tlv_data_offset(body, NAME_SCAN_START, HASH_PLAYER_NAME, NAME_SCAN_MAX)
                    .ok_or(SaveError::Corrupt("hash player_name introuvable"))?,
            }
        } else {
            find_tlv_data_offset(body, NAME_SCAN_START, HASH_PLAYER_NAME, NAME_SCAN_MAX)
                .ok_or(SaveError::Corrupt("hash player_name introuvable"))?
        };

        if data_off + PLAYER_BUF_LEN > body.len() {
            return Err(SaveError::TooShort {
                got: body.len(),
                need: data_off + PLAYER_BUF_LEN,
            });
        }

        let buf = &body[data_off..data_off + PLAYER_BUF_LEN];
        let nul1 = buf.iter().position(|&c| c == 0).unwrap_or(PLAYER_BUF_LEN);
        let name_bytes = &buf[..nul1];
        let name: String = name_bytes.iter().map(|&c| c as char).collect();

        // level_str commence après le premier \0
        let level_start = nul1 + 1;
        let level_str = if level_start < PLAYER_BUF_LEN {
            let rest = &buf[level_start..];
            let nul2 = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
            rest[..nul2].iter().map(|&c| c as char).collect::<String>()
        } else {
            String::new()
        };

        (name, level_str)
    };

    // -----------------------------------------------------------------------
    // 6. Identifiant unique (buffer 33 octets, offset fixe 0x0723)
    //
    //    TLV [hash=0x76B7C825][len=33] à 0x071B.
    //    Ancre : "00023798fc9b4d49b9a36b6b0cb8d50b" à 0x0723 ✓
    // -----------------------------------------------------------------------
    let unique_id = {
        const CANONICAL_UID_DATA_OFF: usize = 0x0723;
        const CANONICAL_UID_HASH_OFF: usize = 0x071B;
        const UID_SCAN_START: usize = 0x06F0;
        const UID_SCAN_MAX: usize = 128;

        let data_off = if body.len() >= CANONICAL_UID_HASH_OFF + 4 {
            match read_u32_le(body, CANONICAL_UID_HASH_OFF) {
                Ok(h) if h == HASH_UNIQUE_ID => CANONICAL_UID_DATA_OFF,
                _ => find_tlv_data_offset(body, UID_SCAN_START, HASH_UNIQUE_ID, UID_SCAN_MAX)
                    .ok_or(SaveError::Corrupt("hash unique_id introuvable"))?,
            }
        } else {
            find_tlv_data_offset(body, UID_SCAN_START, HASH_UNIQUE_ID, UID_SCAN_MAX)
                .ok_or(SaveError::Corrupt("hash unique_id introuvable"))?
        };

        read_null_term_string(body, data_off, UID_BUF_LEN)?
    };

    // -----------------------------------------------------------------------
    // 7. Résultat
    // -----------------------------------------------------------------------
    Ok(HeaderSave {
        format_version,
        opaque_u32,
        max_slots,
        used_slots,
        slots,
        save_timestamp,
        player_name,
        level_str,
        unique_id,
        body_opaque: body.to_vec(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Vérifie que le parseur retourne TooShort sur un body vide.
    #[test]
    fn trop_court_retourne_erreur() {
        let result = parse_headersave(&[]);
        assert!(matches!(result, Err(SaveError::TooShort { .. })));
    }

    /// Vérifie que le parseur retourne TooShort sur un body de 16 octets.
    #[test]
    fn trop_court_16_octets() {
        let body = vec![0u8; 16];
        let result = parse_headersave(&body);
        assert!(matches!(result, Err(SaveError::TooShort { .. })));
    }

    /// Vérifie que le parseur retourne Corrupt si format_version != 1.
    #[test]
    fn format_version_invalide() {
        let mut body = vec![0u8; HEADERSAVE_MIN_LEN];
        // format_version = 2 en big-endian
        body[0] = 0x00;
        body[1] = 0x00;
        body[2] = 0x00;
        body[3] = 0x02;
        // max_slots = 10
        body[8] = 10;
        let result = parse_headersave(&body);
        assert!(matches!(result, Err(SaveError::Corrupt(_))));
    }

    /// Vérifie que le parseur retourne Corrupt si used_slots > max_slots.
    #[test]
    fn used_slots_superieur_a_max_slots() {
        let mut body = vec![0u8; HEADERSAVE_MIN_LEN];
        // format_version = 1 BE
        body[3] = 0x01;
        // max_slots = 5 LE
        body[8] = 5;
        // used_slots = 10 LE
        body[12] = 10;
        let result = parse_headersave(&body);
        assert!(matches!(result, Err(SaveError::Corrupt(_))));
    }

    /// Vérifie qu'un body synthétique minimal avec format_version=1 et champs nuls
    /// retourne une erreur sur l'absence du hash save_timestamp.
    #[test]
    fn body_minimal_sans_timestamp_retourne_corrupt() {
        let mut body = vec![0u8; HEADERSAVE_MIN_LEN];
        // format_version = 1 BE
        body[3] = 0x01;
        // max_slots = 10
        body[8] = 10;
        // used_slots = 0
        body[12] = 0;
        let result = parse_headersave(&body);
        // Devrait échouer sur la recherche du hash save_timestamp (absent dans ce body synthétique)
        assert!(result.is_err());
    }

    /// Vérifie que `decode_datetime` retourne is_unset() sur 8 zéros.
    #[test]
    fn decode_datetime_zeros_is_unset() {
        let data = [0u8; 8];
        let dt = decode_datetime(&data).unwrap();
        assert!(dt.is_unset());
    }

    /// Vérifie que `decode_datetime` retourne is_unset() false sur une date réelle.
    #[test]
    fn decode_datetime_date_reelle() {
        // 2026-02-21T13:15:22 → EA 07 02 15 0D 0F 16 00
        let data = [0xEA, 0x07, 0x02, 0x15, 0x0D, 0x0F, 0x16, 0x00];
        let dt = decode_datetime(&data).unwrap();
        assert!(!dt.is_unset());
        assert_eq!(dt.year, 2026);
        assert_eq!(dt.month, 2);
        assert_eq!(dt.day, 21);
        assert_eq!(dt.hour, 13);
        assert_eq!(dt.minute, 15);
        assert_eq!(dt.second, 22);
    }

    /// Vérifie que `find_tlv_data_offset` retourne None si le hash est absent.
    #[test]
    fn find_tlv_data_offset_absent() {
        let body = vec![0u8; 64];
        assert!(find_tlv_data_offset(&body, 0, 0xDEADBEEF, 64).is_none());
    }

    /// Vérifie que `find_tlv_data_offset` trouve un hash présent.
    #[test]
    fn find_tlv_data_offset_present() {
        let mut body = vec![0u8; 32];
        // hash = 0xDEADBEEF LE à offset 4 → octets EF BE AD DE
        body[4] = 0xEF;
        body[5] = 0xBE;
        body[6] = 0xAD;
        body[7] = 0xDE;
        // len = 4 LE
        body[8] = 0x04;
        body[9] = 0x00;
        body[10] = 0x00;
        body[11] = 0x00;
        // data à body[12..16]
        let result = find_tlv_data_offset(&body, 0, 0xDEAD_BEEF, 32);
        assert_eq!(result, Some(12));
    }
}
