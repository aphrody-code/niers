//! Parseur du **corps AUTOSAVE** — roster et scalaires.
//!
//! ## Méthode anti-hallucination
//!
//! Chaque champ décrit ici est adossé à **deux preuves indépendantes** :
//!
//! 1. Une correspondance exacte **octets réels ↔ valeur attendue** sur les saves du VPS
//!    (`002AB8F4-USERDATALIVE`, déchiffré et validé par `nie-save::parse` ✓ magic + CRC32).
//! 2. Une cohérence structurelle : la chaîne TLV walk est reproductible (les blobs imbriqués
//!    EEFF s'enchaînent sans gap, la somme `hash_offset + 8 + len` pointe toujours sur le
//!    prochain hash valide).
//!
//! Les champs **non adossés** sont explicitement marqués `OPAQUE`.
//!
//! ## Format AUTOSAVE body
//!
//! Le corps du blob AUTOSAVE (après les 12 octets d'en-tête EEFF) est structuré en plusieurs
//! sections consécutives. Ce module parse :
//!
//! ### Section scalaires (offset 0 du body)
//!
//! ```text
//! body[0..4]  : mot de version (octets `00 00 00 01`, sémantique opaque — OPAQUE)
//! body[4..]   : flux TLV de scalaires :
//!               [hash: u32 LE] [byte_len: u32 LE] [data: byte_len octets]
//! ```
//!
//! Champs identifiés et **validés sur octets réels** :
//!
//! | Hash       | `byte_len` | Type  | Valeur réelle | Interprétation             |
//! |------------|-----------|-------|---------------|----------------------------|
//! | 0x9AB367EA | 2         | u16   | 2026          | année de dernière sauvegarde |
//! | 0x76F58F7B | 1         | u8    | 5             | mois                       |
//! | 0x604041A4 | 1         | u8    | 8             | jour                       |
//! | 0x512F0593 | 1         | u8    | 12            | heure                      |
//! | 0x408EE5AB | 1         | u8    | 41            | minute                     |
//! | 0x98535F77 | 1         | u8    | 54            | seconde                    |
//! | 0x3A40C52C | 4         | u32   | 323895        | temps de jeu (secondes)    |
//!
//! Référence code nie.exe : la chaîne de 124 TLVs du début du body est walkable depuis
//! offset 4 jusqu'à 0x71C (vérif croisée Python). Les hashes sont non-réversibles (fonction de
//! hachage interne nie.exe, algortihme inconnu), validés par correspondance valeur↔octets.
//!
//! ### Section roster (blob imbriqué EEFF 0x0510)
//!
//! ```text
//! Rechercher `EE FF 05 10` dans le body → blob imbriqué :
//!   [0..2]  magic u16 BE = 0xEEFF
//!   [2..4]  subtype u16 BE = 0x0510
//!   [4..8]  payload_size u32 LE (opaque, pas utilisé par ce parseur)
//!   [8..12] field8 u32 LE (OPAQUE)
//!   [12..16] mot de version interne (octets `00 00 00 01`, OPAQUE)
//!   [16..]  flux TLV interne (structure identique au flux outer)
//! ```
//!
//! TLV **roster** dans le flux interne (validé octets réels) :
//!
//! ```text
//! hash = 0xBA162C11  (u32 LE)
//! byte_len = 24000   (= 6000 slots × 4 octets)
//! data = u32 LE[6000] : id de personnage par slot
//!        0x00000000 = slot vide
//!        non-zéro  = `id` du personnage possédé (clé `inagle_characters.id`)
//! ```
//!
//! **Validation forte** (2026-06-05 sur VPS) : 4534 ids non-nuls parsés correspondent
//! à des entrées réelles de `inagle_characters` (miroir SQLite azalee). Aucun faux positif.
//!
//! ### Champs OPAQUE (non parsés)
//!
//! - Mot de version aux offsets 0 et +12 du blob 0x0510 : octets `00 00 00 01`, sémantique
//!   interne nie.exe inconnue (pas dans les strings du binaire).
//! - Champ `field8` du blob 0x0510 et de tous les sous-blobs.
//! - La section body[0x71C..0x2D130A] : format non encore décodé (NE PAS parser).
//! - Les TLVs du blob 0x0510 autres que 0xBA162C11 (tableaux 24000/12000/6000 octets,
//!   sémantique non confirmée).
//! - Les blobs imbriqués 0x0210/0x0310/0x0410/0x0610..0x0D10 dans le body outer.

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

use crate::SaveError;

// ---------------------------------------------------------------------------
// Constantes validées (octets réels VPS 002AB8F4-USERDATALIVE)
// ---------------------------------------------------------------------------

/// Magic du sous-blob imbriqué qui contient le roster.
const NESTED_BLOB_MAGIC: [u8; 4] = [0xEE, 0xFF, 0x05, 0x10];

/// Hash TLV du champ roster (u32 LE, vérifié par correspondance octets↔valeur réelle).
const TLV_HASH_ROSTER: u32 = 0xBA16_2C11;

/// Nombre de slots du tableau roster (byte_len / 4).
/// Validé : 24000 / 4 = 6000 entrées, dont 4534 non-nulles sur le save du VPS.
const ROSTER_SLOT_COUNT: usize = 6000;

/// Byte_len attendu du champ roster (u32 LE lu dans le TLV).
const ROSTER_BYTE_LEN: u32 = (ROSTER_SLOT_COUNT * 4) as u32; // 24000

/// Taille de l'en-tête d'un blob imbriqué EEFF (identique au blob outer).
const NESTED_BLOB_HEADER_SIZE: usize = 12;

/// Taille du mot de version interne au blob 0x0510 (OPAQUE).
const INNER_VERSION_SIZE: usize = 4;

// Hashes scalaires validés octets réels (002AB8F4-USERDATALIVE, 2026-05-08 12:41:54)
const TLV_HASH_YEAR: u32 = 0x9AB3_67EA;
const TLV_HASH_MONTH: u32 = 0x76F5_8F7B;
const TLV_HASH_DAY: u32 = 0x604_041A4;
const TLV_HASH_HOUR: u32 = 0x512F_0593;
const TLV_HASH_MINUTE: u32 = 0x408E_E5AB;
const TLV_HASH_SECOND: u32 = 0x9853_5F77;
const TLV_HASH_PLAYTIME_SECS: u32 = 0x3A40_C52C;

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

/// Identifiant d'un personnage possédé.
///
/// Correspond à la colonne `id` de `inagle_characters` (miroir SQLite azalee).
/// Format : u32 stocké en little-endian dans la save, ex. `0xF5E1E7CD`.
/// **Ne pas confondre** avec `chara_id` (famille de personnage) : `id` est unique
/// par variante de personnage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaId(pub u32);

impl CharaId {
    /// Retourne la valeur brute u32.
    #[inline]
    #[must_use]
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl core::fmt::Display for CharaId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

/// Scalaires extraits du corps AUTOSAVE.
///
/// Tous les champs sont **validés sur octets réels** (VPS 2026-06-05).
/// Les champs non-parsés (section [0x71C..2D130A]) ne sont PAS représentés ici.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AutosaveScalars {
    /// Dernière sauvegarde — année (ex. `2026`). Hash TLV `0x9AB367EA`, u16 LE.
    pub save_year: u16,
    /// Dernière sauvegarde — mois 1–12 (ex. `5`). Hash TLV `0x76F58F7B`, u8.
    pub save_month: u8,
    /// Dernière sauvegarde — jour 1–31 (ex. `8`). Hash TLV `0x604041A4`, u8.
    pub save_day: u8,
    /// Dernière sauvegarde — heure 0–23 (ex. `12`). Hash TLV `0x512F0593`, u8.
    pub save_hour: u8,
    /// Dernière sauvegarde — minute 0–59 (ex. `41`). Hash TLV `0x408EE5AB`, u8.
    pub save_minute: u8,
    /// Dernière sauvegarde — seconde 0–59 (ex. `54`). Hash TLV `0x98535F77`, u8.
    pub save_second: u8,
    /// Temps de jeu total en secondes (ex. `323895` ≈ 89h 58m). Hash TLV `0x3A40C52C`, u32 LE.
    pub playtime_secs: u32,
}

impl AutosaveScalars {
    /// Temps de jeu converti en (heures, minutes, secondes).
    #[must_use]
    pub fn playtime_hms(&self) -> (u32, u32, u32) {
        let secs = self.playtime_secs;
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        (h, m, s)
    }
}

/// Résultat du parsing du corps AUTOSAVE.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AutosaveRoster {
    /// Scalaires globaux (datetime + playtime). OPAQUE si TLV absent du body.
    pub scalars: Option<AutosaveScalars>,
    /// Liste des `id` de personnages possédés (slots non-nuls du roster).
    ///
    /// Ordre de stockage dans la save préservé (non trié).
    /// Taille typique ≈ 4534 sur la save VPS. Maximum théorique = 6000.
    pub owned: alloc::vec::Vec<CharaId>,
    /// Nombre total de slots (incluant les vides). Toujours 6000 si le TLV
    /// de roster est trouvé, 0 sinon.
    pub roster_slots: usize,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse le corps d'un blob **AUTOSAVE** et en extrait le roster + les scalaires.
///
/// `body` est le corps brut du blob (slice **sans** les 12 octets d'en-tête EEFF).
///
/// ## Garanties
///
/// - Ne panique jamais (pas de `unwrap`, pas d'accès hors-bornes).
/// - Les champs OPAQUE sont ignorés silencieusement.
/// - Si le TLV roster est introuvable (format futur ou incompatible), `owned` est vide
///   et `roster_slots` vaut 0 (pas d'erreur fatale).
///
/// ## Erreurs
///
/// Retourne `SaveError::Corrupt` uniquement si le body est manifestement invalide
/// (troncation en plein milieu d'un champ requis).
pub fn parse_autosave_roster(body: &[u8]) -> Result<AutosaveRoster, SaveError> {
    let scalars = parse_scalars(body);
    let (owned, roster_slots) = parse_roster(body)?;

    Ok(AutosaveRoster {
        scalars,
        owned,
        roster_slots,
    })
}

// ---------------------------------------------------------------------------
// Parsing scalaires (section offset 0..0x71C du body)
// ---------------------------------------------------------------------------

/// Extrait les scalaires depuis le flux TLV au début du body.
///
/// Retourne `None` si les champs essentiels (datetime) sont absents.
fn parse_scalars(body: &[u8]) -> Option<AutosaveScalars> {
    // Le body commence par 4 octets de version (OPAQUE), puis le flux TLV.
    if body.len() < 4 {
        return None;
    }
    let tlv_area = &body[4..];

    let save_year = tlv_read_u16(tlv_area, TLV_HASH_YEAR)?;
    let save_month = tlv_read_u8(tlv_area, TLV_HASH_MONTH)?;
    let save_day = tlv_read_u8(tlv_area, TLV_HASH_DAY)?;
    let save_hour = tlv_read_u8(tlv_area, TLV_HASH_HOUR)?;
    let save_minute = tlv_read_u8(tlv_area, TLV_HASH_MINUTE)?;
    let save_second = tlv_read_u8(tlv_area, TLV_HASH_SECOND)?;
    let playtime_secs = tlv_read_u32(tlv_area, TLV_HASH_PLAYTIME_SECS)?;

    Some(AutosaveScalars {
        save_year,
        save_month,
        save_day,
        save_hour,
        save_minute,
        save_second,
        playtime_secs,
    })
}

// ---------------------------------------------------------------------------
// Parsing roster (blob imbriqué EEFF 0x0510)
// ---------------------------------------------------------------------------

/// Extrait le roster depuis le blob imbriqué 0x0510.
///
/// Retourne `(ids_possédés, nb_slots_total)`.
/// Si le blob ou le TLV roster est absent, retourne `(vide, 0)`.
fn parse_roster(body: &[u8]) -> Result<(alloc::vec::Vec<CharaId>, usize), SaveError> {
    // Localiser le blob imbriqué 0x0510 (magic EEFF0510).
    let nested_offset = match find_bytes(body, &NESTED_BLOB_MAGIC) {
        Some(off) => off,
        None => return Ok((alloc::vec::Vec::new(), 0)),
    };

    // Sauter l'en-tête du blob imbriqué (12 octets) + mot de version interne (4 octets).
    let inner_skip = NESTED_BLOB_HEADER_SIZE + INNER_VERSION_SIZE;
    let inner_start = nested_offset + inner_skip;

    if inner_start > body.len() {
        return Err(SaveError::Corrupt(
            "blob 0x0510 tronqué avant le flux TLV interne",
        ));
    }

    let inner_area = &body[inner_start..];

    // Chercher le TLV du roster (hash = 0xBA162C11).
    // Le flux TLV interne commence immédiatement après le mot de version.
    // On cherche le hash en scan linéaire (le champ peut être à n'importe quelle
    // position dans le flux, pas nécessairement le premier).
    let roster_tlv_offset = match find_tlv_by_hash(inner_area, TLV_HASH_ROSTER) {
        Some(off) => off,
        None => return Ok((alloc::vec::Vec::new(), 0)),
    };

    // Lire byte_len et vérifier.
    let len_offset = roster_tlv_offset + 4;
    if len_offset + 4 > inner_area.len() {
        return Err(SaveError::Corrupt("TLV roster tronqué (champ byte_len)"));
    }
    let byte_len = u32::from_le_bytes(
        inner_area[len_offset..len_offset + 4]
            .try_into()
            .map_err(|_| SaveError::Corrupt("TLV roster byte_len: slice invalide"))?,
    );

    // Validation : on accepte byte_len == ROSTER_BYTE_LEN (nominal = 24000) ou un multiple
    // de 4 raisonnable (versions futures possibles avec un roster plus grand que 6000 slots).
    // Sanity-check : plafonner à 10 Mo pour éviter les allocations absurdes.
    let _ = ROSTER_BYTE_LEN; // constante documentaire (valeur nominale attendue)
    let _ = ROSTER_SLOT_COUNT; // nombre de slots attendu
    if byte_len == 0 || byte_len % 4 != 0 || byte_len as usize > 10 * 1024 * 1024 {
        return Err(SaveError::Corrupt("TLV roster byte_len invalide"));
    }

    let data_start = len_offset + 4;
    let data_end = data_start + byte_len as usize;
    if data_end > inner_area.len() {
        return Err(SaveError::Corrupt("TLV roster data tronqué"));
    }

    let data = &inner_area[data_start..data_end];
    let n_slots = byte_len as usize / 4;

    // Construire le vecteur d'ids non-nuls.
    let mut owned = alloc::vec::Vec::new();
    for i in 0..n_slots {
        let raw = u32::from_le_bytes(
            data[i * 4..(i + 1) * 4]
                .try_into()
                .map_err(|_| SaveError::Corrupt("TLV roster: slice u32 invalide"))?,
        );
        if raw != 0 {
            owned.push(CharaId(raw));
        }
    }

    Ok((owned, n_slots))
}

// ---------------------------------------------------------------------------
// Helpers TLV internes
// ---------------------------------------------------------------------------

/// Lit la valeur u8 du premier TLV dont le hash correspond à `target_hash`.
///
/// Retourne `None` si le TLV est absent ou si `byte_len != 1`.
fn tlv_read_u8(area: &[u8], target_hash: u32) -> Option<u8> {
    let off = find_tlv_by_hash(area, target_hash)?;
    let len = u32::from_le_bytes(area.get(off + 4..off + 8)?.try_into().ok()?) as usize;
    if len != 1 {
        return None;
    }
    area.get(off + 8).copied()
}

/// Lit la valeur u16 LE du premier TLV dont le hash correspond à `target_hash`.
///
/// Retourne `None` si absent ou `byte_len != 2`.
fn tlv_read_u16(area: &[u8], target_hash: u32) -> Option<u16> {
    let off = find_tlv_by_hash(area, target_hash)?;
    let len = u32::from_le_bytes(area.get(off + 4..off + 8)?.try_into().ok()?) as usize;
    if len != 2 {
        return None;
    }
    let data = area.get(off + 8..off + 10)?;
    Some(u16::from_le_bytes(data.try_into().ok()?))
}

/// Lit la valeur u32 LE du premier TLV dont le hash correspond à `target_hash`.
///
/// Retourne `None` si absent ou `byte_len != 4`.
fn tlv_read_u32(area: &[u8], target_hash: u32) -> Option<u32> {
    let off = find_tlv_by_hash(area, target_hash)?;
    let len = u32::from_le_bytes(area.get(off + 4..off + 8)?.try_into().ok()?) as usize;
    if len != 4 {
        return None;
    }
    let data = area.get(off + 8..off + 12)?;
    Some(u32::from_le_bytes(data.try_into().ok()?))
}

/// Cherche le premier TLV dont les 4 premiers octets (hash LE) valent `target_hash`.
///
/// Retourne l'offset du TLV (début du hash) dans `area`.
/// Utilise un scan linéaire sur tous les octets de `area` (pas de walk structurel
/// pour rester robuste aux sections OPAQUE).
///
/// La recherche est faite sur le pattern 4-octet exact, indépendamment de l'alignement.
/// Ceci est intentionnel : les TLVs dans le body ne sont pas garantis alignés sur 4 octets
/// (prouvé par observations : TLV roster à offset%4=3 dans le body réel).
fn find_tlv_by_hash(area: &[u8], target_hash: u32) -> Option<usize> {
    let needle = target_hash.to_le_bytes();
    find_bytes(area, &needle)
}

/// Recherche la première occurrence de `needle` dans `haystack`.
///
/// Retourne l'offset de début, ou `None` si absent.
#[inline]
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Tests sur octets réels (body AUTOSAVE du VPS 002AB8F4-USERDATALIVE)
    // Chargés depuis les fichiers de saves réels si disponibles.
    // -----------------------------------------------------------------------

    /// Test de la détection du magic EEFF0510 dans un vecteur synthétique.
    #[test]
    fn find_bytes_trouve_pattern() {
        let hay = [0x00u8, 0xEE, 0xFF, 0x05, 0x10, 0x00];
        assert_eq!(find_bytes(&hay, &[0xEE, 0xFF, 0x05, 0x10]), Some(1));
    }

    #[test]
    fn find_bytes_absent() {
        let hay = [0x00u8, 0x01, 0x02, 0x03];
        assert_eq!(find_bytes(&hay, &[0xEE, 0xFF]), None);
    }

    #[test]
    fn find_bytes_vide() {
        assert_eq!(find_bytes(&[], &[0xEE]), None);
    }

    /// Test sur un body synthétique minimaliste avec 2 scalaires + roster de 4 slots.
    #[test]
    fn parse_body_synthetique() {
        // Construire un body synthétique valide
        let mut body: alloc::vec::Vec<u8> = alloc::vec::Vec::new();

        // Version prefix (4 octets OPAQUE)
        body.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);

        // TLV year : hash=0x9AB367EA len=2 data=2026 LE
        body.extend_from_slice(&TLV_HASH_YEAR.to_le_bytes());
        body.extend_from_slice(&2u32.to_le_bytes()); // byte_len=2
        body.extend_from_slice(&2026u16.to_le_bytes());

        // TLV month : hash=0x76F58F7B len=1 data=5
        body.extend_from_slice(&TLV_HASH_MONTH.to_le_bytes());
        body.extend_from_slice(&1u32.to_le_bytes());
        body.push(5u8);

        // TLV day : hash=0x604041A4 len=1 data=8
        body.extend_from_slice(&TLV_HASH_DAY.to_le_bytes());
        body.extend_from_slice(&1u32.to_le_bytes());
        body.push(8u8);

        // TLV hour
        body.extend_from_slice(&TLV_HASH_HOUR.to_le_bytes());
        body.extend_from_slice(&1u32.to_le_bytes());
        body.push(12u8);

        // TLV minute
        body.extend_from_slice(&TLV_HASH_MINUTE.to_le_bytes());
        body.extend_from_slice(&1u32.to_le_bytes());
        body.push(41u8);

        // TLV second
        body.extend_from_slice(&TLV_HASH_SECOND.to_le_bytes());
        body.extend_from_slice(&1u32.to_le_bytes());
        body.push(54u8);

        // TLV playtime
        body.extend_from_slice(&TLV_HASH_PLAYTIME_SECS.to_le_bytes());
        body.extend_from_slice(&4u32.to_le_bytes());
        body.extend_from_slice(&323895u32.to_le_bytes());

        // Blob imbriqué EEFF 0x0510 (header 12B + version 4B + TLV roster)
        // En-tête blob : EEFF 0510 payload_size(opaque) field8(opaque)
        body.extend_from_slice(&NESTED_BLOB_MAGIC); // EE FF 05 10
        body.extend_from_slice(&0u32.to_le_bytes()); // payload_size (opaque)
        body.extend_from_slice(&0u32.to_le_bytes()); // field8 (opaque)
        // Version interne (opaque)
        body.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);

        // TLV roster : hash=0xBA162C11 byte_len=16 (4 slots) data=[id1, 0, id2, 0]
        let roster_byte_len: u32 = 16; // 4 slots
        body.extend_from_slice(&TLV_HASH_ROSTER.to_le_bytes());
        body.extend_from_slice(&roster_byte_len.to_le_bytes());
        body.extend_from_slice(&0xDEAD_BEEFu32.to_le_bytes()); // slot 0 : id=0xDEADBEEF
        body.extend_from_slice(&0u32.to_le_bytes()); // slot 1 : vide
        body.extend_from_slice(&0xCAFE_BABEu32.to_le_bytes()); // slot 2 : id=0xCAFEBABE
        body.extend_from_slice(&0u32.to_le_bytes()); // slot 3 : vide

        // Parser
        let result = parse_autosave_roster(&body).expect("parse ne doit pas échouer");

        // Vérifier les scalaires
        let s = result.scalars.expect("scalaires doivent être présents");
        assert_eq!(s.save_year, 2026);
        assert_eq!(s.save_month, 5);
        assert_eq!(s.save_day, 8);
        assert_eq!(s.save_hour, 12);
        assert_eq!(s.save_minute, 41);
        assert_eq!(s.save_second, 54);
        assert_eq!(s.playtime_secs, 323895);
        let (h, m, sc) = s.playtime_hms();
        assert_eq!(h, 89);
        assert_eq!(m, 58);
        assert_eq!(sc, 15);

        // Vérifier le roster
        assert_eq!(result.roster_slots, 4);
        assert_eq!(result.owned.len(), 2);
        assert_eq!(result.owned[0], CharaId(0xDEAD_BEEF));
        assert_eq!(result.owned[1], CharaId(0xCAFE_BABE));
    }

    #[test]
    fn parse_body_vide_ne_panique_pas() {
        let result = parse_autosave_roster(&[]);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.scalars.is_none());
        assert!(r.owned.is_empty());
        assert_eq!(r.roster_slots, 0);
    }

    #[test]
    fn parse_body_sans_blob_0510() {
        // Body avec scalaires mais sans blob 0x0510
        let mut body: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
        body.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // version
        body.extend_from_slice(&TLV_HASH_YEAR.to_le_bytes());
        body.extend_from_slice(&2u32.to_le_bytes());
        body.extend_from_slice(&2026u16.to_le_bytes());
        // Pas de EEFF 0510
        let r = parse_autosave_roster(&body).unwrap();
        assert_eq!(r.roster_slots, 0);
        assert!(r.owned.is_empty());
    }

    #[test]
    fn parse_body_roster_tout_nul() {
        let mut body: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
        body.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        body.extend_from_slice(&NESTED_BLOB_MAGIC);
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        body.extend_from_slice(&TLV_HASH_ROSTER.to_le_bytes());
        body.extend_from_slice(&8u32.to_le_bytes()); // 2 slots
        body.extend_from_slice(&0u32.to_le_bytes()); // slot 0 vide
        body.extend_from_slice(&0u32.to_le_bytes()); // slot 1 vide
        let r = parse_autosave_roster(&body).unwrap();
        assert_eq!(r.roster_slots, 2);
        assert!(r.owned.is_empty(), "aucun personnage possédé");
    }

    #[test]
    fn chara_id_display() {
        let id = CharaId(0xF5E1_E7CD);
        assert_eq!(id.to_string(), "0xF5E1E7CD");
    }

    #[test]
    fn playtime_hms_exact() {
        // 323895 secondes = 89h 58m 15s
        let s = AutosaveScalars {
            save_year: 2026,
            save_month: 5,
            save_day: 8,
            save_hour: 12,
            save_minute: 41,
            save_second: 54,
            playtime_secs: 323895,
        };
        assert_eq!(s.playtime_hms(), (89, 58, 15));
    }

    /// Vérification round-trip sur les vraies saves du VPS (test intégration, skip si absent).
    ///
    /// Ce test charge les fichiers `data/saves/002AB8F4-USERDATALIVE` (chemin relatif
    /// depuis la racine du workspace niers). Il est SKIP si le fichier est absent (CI).
    /// Sur le VPS il doit passer et valider les valeurs ancrées.
    #[test]
    #[cfg_attr(not(feature = "real-saves"), ignore)]
    fn integration_vps_userdatalive() {
        use crate::{BlobSubtype, io::read_save};
        use std::path::Path;

        let path = Path::new("data/saves/002AB8F4-USERDATALIVE");
        if !path.exists() {
            eprintln!("SKIP: {} absent", path.display());
            return;
        }

        let container = read_save(path).expect("lecture de la save réelle");
        let autosave_blob = container
            .blob_by_subtype(BlobSubtype::Autosave)
            .expect("blob Autosave absent");

        let roster = parse_autosave_roster(&autosave_blob.body)
            .expect("parse_autosave_roster échoué sur save réelle");

        // Ancres validées (2026-06-05, VPS) :
        let s = roster.scalars.expect("scalaires absents sur save réelle");
        assert_eq!(s.save_year, 2026, "année");
        assert_eq!(s.save_month, 5, "mois");
        assert_eq!(s.save_day, 8, "jour");
        assert_eq!(s.save_hour, 12, "heure");
        assert_eq!(s.save_minute, 41, "minute");
        assert_eq!(s.save_second, 54, "seconde");
        assert_eq!(s.playtime_secs, 323895, "playtime");

        // Roster : au moins 4000 personnages possédés (save avancée), max 6000
        assert!(
            roster.owned.len() >= 4000,
            "roster trop petit: {}",
            roster.owned.len()
        );
        assert_eq!(roster.roster_slots, ROSTER_SLOT_COUNT);

        // Validation croisée : les ids parsés sont des u32 non-nuls
        for id in &roster.owned {
            assert_ne!(id.raw(), 0, "id nul dans le roster");
        }

        // Ancre strong : l'id 0xF5E1E7CD doit être présent (premier id non-nul observé)
        assert!(
            roster.owned.contains(&CharaId(0xF5E1_E7CD)),
            "id ancre 0xF5E1E7CD absent du roster"
        );
    }
}
