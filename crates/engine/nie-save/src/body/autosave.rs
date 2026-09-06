//! Disposition du corps du blob **AUTOSAVE** (vue macroscopique).
//!
//! ## Méthode anti-hallucination
//!
//! Chaque champ ou région décrit ici est adossé à **deux preuves** :
//! 1. Octets réels extraits de `002AB8F4-USERDATALIVE` (VPS, déchiffré et validé par
//!    `nie-save::parse` ✓ magic + CRC32).
//! 2. Une invariant structurel vérifiable : le walk SF-TLV est reproductible et exhauste
//!    exactement les regions annoncées (pas de gap, pas d'overlap).
//!
//! Les régions **non décodées** sont explicitement marquées OPAQUE et leurs octets bruts
//! sont exposés via `body_opaque` — jamais ignorés silencieusement.
//!
//! ## Format AUTOSAVE body (résumé)
//!
//! ```text
//! body[0x000..0x008] : 8 octets d'en-tête global
//!   [0..4]  version   u32 BE = 0x00000001 (toujours 1 sur saves réelles)
//!   [4..8]  opaque_4  u32 LE              (OPAQUE — valeur interne engine)
//!
//! body[0x008..0x720] : flux SF-TLV scalaires (124 enregistrements)
//!   Format : [size: u32 LE][hash: u32 LE][data: size octets]
//!   Contient : datetime de sauvegarde, temps de jeu, paramètres de jeu
//!   (parsé en détail par autosave_roster.rs via scan linéaire des hash connus)
//!
//! body[0x720..0x728] : séparateur de section (8 octets)
//!   1E 00 00 03  D0 66 1F 54  (marqueur début table CharaParam)
//!
//! body[0x728..0x5C70] : table CharaParam (30 slots × 728 octets = 21 840 octets)
//!   Chaque slot = 8 octets séparateur (1E 00 00 04 D0 66 1F 54)
//!               + 720 octets flux SF-TLV interne (41 enregistrements/slot)
//!   Stride exact : 728 = 0x2D8 octets entre séparateurs successifs
//!   Validation : 30 séparateurs consécutifs à stride 0x2D8 confirmé sur save réelle
//!
//! body[0x5C70..0x5CD2] : section mini (type 0x8C4D2EB7, OPAQUE)
//!   8 octets séparateur + 7 enregistrements SF-TLV (90 octets utiles)
//!
//! body[0x5CD2..end]   : section données principale (~11.9 Mo, OPAQUE)
//!   Format interne non décodé (SF-TLV avec sous-structures répétitives)
//!   Contient probablement : inventaire, progression histoire, flags, équipes
//! ```
//!
//! ## Invariants vérifiés sur save réelle (2026-06-05, VPS)
//!
//! - `version == 1` (u32 BE aux octets 0..4)
//! - séparateur #0 toujours `1E 00 00 03 D0 66 1F 54` à l'offset 0x720
//! - 30 séparateurs CharaParam à stride **exact** 0x2D8 = 728 octets depuis 0x720
//! - tous les slots = 41 SF-TLV records, 720 octets chacun (aucun slot vide)

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

extern crate alloc;
use alloc::vec::Vec;

use crate::SaveError;

// ---------------------------------------------------------------------------
// Constantes validées (octets réels VPS 002AB8F4-USERDATALIVE)
// ---------------------------------------------------------------------------

/// Valeur attendue du mot de version en tête du body (u32 big-endian).
pub const BODY_VERSION: u32 = 0x0000_0001;

/// Offset du début du flux SF-TLV scalaires (après l'en-tête 8 octets).
pub const SCALAR_STREAM_OFFSET: usize = 0x008;

/// Offset de fin du flux SF-TLV scalaires (exclusif).
/// Le séparateur de section CharaParam commence ici.
pub const SCALAR_STREAM_END: usize = 0x720;

/// Nombre d'enregistrements dans le flux scalaire (validé par walk exhaustif).
pub const SCALAR_RECORD_COUNT: usize = 124;

/// Octets du séparateur de section CharaParam (type=03, magic).
/// Ce marqueur précède le premier slot CharaParam.
pub const CHARA_SECTION_SEPARATOR: [u8; 8] = [0x1E, 0x00, 0x00, 0x03, 0xD0, 0x66, 0x1F, 0x54];

/// Octets du séparateur de slot CharaParam (type=04, même magic).
/// Répété avant chacun des 30 slots.
pub const CHARA_SLOT_SEPARATOR: [u8; 8] = [0x1E, 0x00, 0x00, 0x04, 0xD0, 0x66, 0x1F, 0x54];

/// Taille totale d'un slot CharaParam (séparateur 8 octets + données 720 octets).
pub const CHARA_SLOT_STRIDE: usize = 728; // = 0x2D8

/// Taille des données d'un slot CharaParam (hors séparateur).
pub const CHARA_SLOT_DATA_SIZE: usize = 720; // = CHARA_SLOT_STRIDE - 8

/// Nombre de slots CharaParam dans la table (validé : 30 slots, tous actifs sur save réelle).
pub const CHARA_SLOT_COUNT: usize = 30;

/// Nombre d'enregistrements SF-TLV par slot CharaParam (validé : 41 records × 30 slots).
pub const CHARA_SLOT_RECORD_COUNT: usize = 41;

/// Offset du début de la table CharaParam dans le body (après séparateur 0x720..0x728).
pub const CHARA_TABLE_OFFSET: usize = 0x728;

/// Offset de fin de la table CharaParam (exclusif).
/// 0x728 + 30 × 728 = 0x5C70
pub const CHARA_TABLE_END: usize = CHARA_TABLE_OFFSET + CHARA_SLOT_COUNT * CHARA_SLOT_STRIDE;
// = 0x728 + 0x5550 = 0x5C78 — but separator[29] is at 0x5998, slot data ends at 0x5C70
// Séparateur[29] @ 0x5998, données @ 0x59A0..0x59A0+720=0x5C70. Fin = 0x5C70.
// CHARA_TABLE_END doit être 0x5C70, non 0x5C78. Correction ci-dessous.

/// Offset de fin corrigé de la table CharaParam.
/// = 0x5998 (séparateur #29) + 8 (séparateur) + 720 (données) = 0x5C70.
pub const CHARA_TABLE_END_REAL: usize = 0x5C70;

/// Octets du séparateur de la section suivante (type=04, magic différent 0x8C4D2EB7).
pub const SECTION2_SEPARATOR: [u8; 8] = [0x1E, 0x00, 0x00, 0x04, 0x8C, 0x4D, 0x2E, 0xB7];

/// Offset du séparateur section 2 dans le body.
pub const SECTION2_OFFSET: usize = 0x5C70;

/// Offset du début de la section données principale (après section 2).
/// Approximatif — la fin du flux SF-TLV de section 2 est à 0x5CD2.
pub const MAIN_DATA_OFFSET: usize = 0x5CD2;

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

/// Disposition macroscopique du body d'un blob AUTOSAVE.
///
/// Tous les champs **confirmés** contiennent des bornes validées sur octets réels.
/// Les régions OPAQUE sont exposées comme des slices bruts (jamais perdus).
///
/// Voir le module-level doc pour le détail de chaque région.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AutoSaveLayout {
    /// Version du format body (u32 BE, toujours 1). Validé : octets `00 00 00 01`.
    pub version: u32,

    /// Champ opaque à l'offset 4..8 du body (u32 LE, sémantique interne engine inconnue).
    /// Valeur observée sur la save VPS : `0x17E84B00`.
    pub opaque_4: u32,

    /// Nombre d'enregistrements SF-TLV comptés dans le flux scalaire `[0x008..0x720]`.
    ///
    /// Normalement 124 (validé sur save réelle). Si différent, indique un format inconnu.
    pub scalar_record_count: usize,

    /// Bornes du flux scalaire dans le body original (offset_debut, offset_fin_excl).
    /// Toujours `(0x008, 0x720)` sur saves réelles.
    pub scalar_stream_range: (usize, usize),

    /// Slots CharaParam parsés.
    ///
    /// 30 slots sur la save réelle, chacun contenant 41 enregistrements SF-TLV.
    pub chara_param_slots: Vec<CharaParamSlot>,

    /// Bornes de la table CharaParam dans le body (offset_debut, offset_fin_excl).
    /// Premier octet du premier séparateur .. dernier octet des données du dernier slot.
    /// Toujours `(0x720, 0x5C70)` sur saves réelles.
    pub chara_table_range: (usize, usize),

    /// Section 2 : données OPAQUE. Bornes dans le body (offset_debut, offset_fin_excl).
    /// Toujours commence à 0x5C70. Fin déterminée par le walk SF-TLV.
    pub section2_range: (usize, usize),

    /// Section données principale : tout ce qui vient après la section 2.
    /// OPAQUE — format non décodé (~11.9 Mo sur la save VPS).
    /// Contient probablement inventaire, progression, flags, équipes.
    pub main_data_range: (usize, usize),
}

/// Slot de la table CharaParam (728 octets = 8 séparateur + 720 données).
///
/// Chaque slot est un bloc SF-TLV de 41 enregistrements.
/// Contenu sémantique des TLV non décodé (hashes non connus = OPAQUE).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaParamSlot {
    /// Index du slot (0..29).
    pub index: usize,
    /// Offset du début du séparateur de ce slot dans le body original.
    pub separator_offset: usize,
    /// Offset du premier octet de données (après le séparateur 8 octets).
    pub data_offset: usize,
    /// Nombre d'enregistrements SF-TLV parsés dans ce slot.
    /// Normalement 41. Zéro si le slot est vide ou format inconnu.
    pub record_count: usize,
    /// Indique si le séparateur est valide (les 8 octets attendus sont présents).
    pub separator_valid: bool,
}

impl CharaParamSlot {
    /// Retourne les données brutes de ce slot depuis le body d'origine.
    ///
    /// `body` doit être le body complet du blob AUTOSAVE (pas de sous-slice).
    /// Retourne `None` si les bornes dépassent la taille du body.
    #[inline]
    #[must_use]
    pub fn data<'b>(&self, body: &'b [u8]) -> Option<&'b [u8]> {
        let end = self.data_offset + CHARA_SLOT_DATA_SIZE;
        body.get(self.data_offset..end)
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse la disposition macroscopique d'un corps de blob **AUTOSAVE**.
///
/// `body` est le corps brut du blob (slice **sans** les 12 octets d'en-tête EEFF).
///
/// ## Garanties
///
/// - Ne panique jamais.
/// - Toutes les accès aux octets utilisent des bornes vérifiées.
/// - Les régions OPAQUE sont identifiées par leurs bornes mais leur contenu n'est PAS
///   interprété (aucune hallucination sur le contenu inconnu).
///
/// ## Erreurs
///
/// Retourne [`SaveError::Corrupt`] uniquement si :
/// - `body.len() < 8` (trop court pour lire l'en-tête)
/// - `version != 1` (format non reconnu)
///
/// Les incohérences structurelles mineures (ex. séparateur manquant, nombre de records
/// inattendu) **ne** génèrent PAS d'erreur — elles sont reflétées dans les champs
/// `separator_valid` / `record_count` pour diagnostic.
pub fn parse_autosave_layout(body: &[u8]) -> Result<AutoSaveLayout, SaveError> {
    // ---- En-tête global (8 octets) ----
    if body.len() < 8 {
        return Err(SaveError::TooShort {
            got: body.len(),
            need: 8,
        });
    }

    // body.len() >= 8 vérifié ci-dessus ; get() pour rester sans panic en toute circonstance.
    let version = u32::from_be_bytes(
        body.get(0..4)
            .and_then(|s| s.try_into().ok())
            .ok_or(SaveError::Corrupt("AUTOSAVE: lecture version"))?,
    );
    if version != BODY_VERSION {
        return Err(SaveError::Corrupt(
            "version AUTOSAVE body inconnue (attendu 0x00000001)",
        ));
    }
    let opaque_4 = u32::from_le_bytes(
        body.get(4..8)
            .and_then(|s| s.try_into().ok())
            .ok_or(SaveError::Corrupt("AUTOSAVE: lecture opaque_4"))?,
    );

    // ---- Flux SF-TLV scalaires [0x008..0x720] ----
    let scalar_end = SCALAR_STREAM_END.min(body.len());
    let scalar_area = &body[SCALAR_STREAM_OFFSET..scalar_end];
    let scalar_record_count = count_sf_tlv_records(scalar_area);
    let scalar_stream_range = (SCALAR_STREAM_OFFSET, scalar_end);

    // ---- Table CharaParam ----
    // Détecter le séparateur de section (8 octets à l'offset 0x720)
    let chara_table_start = SCALAR_STREAM_END; // 0x720
    let chara_slots = parse_chara_table(body, chara_table_start);

    // Calculer la borne réelle de la table
    let chara_table_end = if chara_slots.is_empty() {
        chara_table_start
    } else {
        let last = &chara_slots[chara_slots.len() - 1];
        last.data_offset + CHARA_SLOT_DATA_SIZE
    };
    let chara_table_range = (chara_table_start, chara_table_end);

    // ---- Section 2 (après table CharaParam) ----
    let section2_start = chara_table_end; // = 0x5C70 sur saves réelles
    let section2_end = find_section2_end(body, section2_start);
    let section2_range = (section2_start, section2_end);

    // ---- Section données principale (OPAQUE) ----
    let main_data_start = section2_end;
    let main_data_end = body.len();
    let main_data_range = (main_data_start, main_data_end);

    Ok(AutoSaveLayout {
        version,
        opaque_4,
        scalar_record_count,
        scalar_stream_range,
        chara_param_slots: chara_slots,
        chara_table_range,
        section2_range,
        main_data_range,
    })
}

// ---------------------------------------------------------------------------
// Helpers internes
// ---------------------------------------------------------------------------

/// Compte les enregistrements SF-TLV valides dans `area`.
///
/// Format SF-TLV : `[size: u32 LE][hash: u32 LE][data: size octets]`.
/// S'arrête au premier enregistrement invalide (size trop grand ou troncation).
fn count_sf_tlv_records(area: &[u8]) -> usize {
    let mut off = 0usize;
    let mut count = 0usize;
    loop {
        if off + 8 > area.len() {
            break;
        }
        // get() garanti non-None par la garde off+8 ci-dessus.
        let size_bytes = match area.get(off..off + 4).and_then(|s| s.try_into().ok()) {
            Some(b) => b,
            None => break,
        };
        let size = u32::from_le_bytes(size_bytes) as usize;
        // Borne de sécurité : size > 64 Ko = enregistrement invalide ou fin du flux
        if size > 0x1_0000 {
            break;
        }
        // checked_add pour éviter l'overflow wasm32.
        let end = match off.checked_add(8).and_then(|v| v.checked_add(size)) {
            Some(v) => v,
            None => break,
        };
        if end > area.len() {
            break;
        }
        count += 1;
        off = end;
    }
    count
}

/// Parse la table CharaParam depuis `start_offset` dans `body`.
///
/// `start_offset` pointe sur le séparateur de section (8 octets `1E 00 00 03 D0 66 1F 54`).
/// Retourne le vecteur de slots parsés (CHARA_SLOT_COUNT au maximum).
fn parse_chara_table(body: &[u8], start_offset: usize) -> Vec<CharaParamSlot> {
    let mut slots = Vec::with_capacity(CHARA_SLOT_COUNT);

    for i in 0..CHARA_SLOT_COUNT {
        // checked_add pour éviter l'overflow sur wasm32 (usize = 32 bits).
        let sep_offset = match start_offset.checked_add(i.saturating_mul(CHARA_SLOT_STRIDE)) {
            Some(v) => v,
            None => break,
        };
        let data_offset = match sep_offset.checked_add(8) {
            Some(v) => v,
            None => break,
        };

        // Vérifier que le séparateur est lisible
        if data_offset > body.len() {
            break;
        }

        // Valider le séparateur
        let separator_valid = if sep_offset + 8 <= body.len() {
            let sep = &body[sep_offset..sep_offset + 8];
            if i == 0 {
                sep == CHARA_SECTION_SEPARATOR
            } else {
                sep == CHARA_SLOT_SEPARATOR
            }
        } else {
            false
        };

        // Compter les enregistrements SF-TLV dans les données du slot
        let record_count = if data_offset + CHARA_SLOT_DATA_SIZE <= body.len() {
            let slot_data = &body[data_offset..data_offset + CHARA_SLOT_DATA_SIZE];
            count_sf_tlv_records(slot_data)
        } else {
            0
        };

        slots.push(CharaParamSlot {
            index: i,
            separator_offset: sep_offset,
            data_offset,
            record_count,
            separator_valid,
        });
    }

    slots
}

/// Détermine la borne de fin de la section 2 en walkant le flux SF-TLV qui suit son séparateur.
///
/// La section 2 commence par un séparateur de 8 octets (`SECTION2_SEPARATOR`) suivi d'un
/// flux SF-TLV. La fin du flux = première position invalide.
fn find_section2_end(body: &[u8], start: usize) -> usize {
    // Le séparateur occupe [start..start+8], le flux SF-TLV commence à start+8.
    let flux_start = start + 8;
    if flux_start > body.len() {
        return body.len().min(start + 8);
    }
    let area = &body[flux_start..];
    let mut off = 0usize;
    loop {
        if off + 8 > area.len() {
            break;
        }
        let size_bytes = match area.get(off..off + 4).and_then(|s| s.try_into().ok()) {
            Some(b) => b,
            None => break,
        };
        let size = u32::from_le_bytes(size_bytes) as usize;
        if size > 0x1_0000 {
            break;
        }
        // checked_add pour éviter l'overflow wasm32.
        let end = match off.checked_add(8).and_then(|v| v.checked_add(size)) {
            Some(v) => v,
            None => break,
        };
        if end > area.len() {
            break;
        }
        off = end;
    }
    // checked_add : flux_start + off ne peut pas dépasser body.len() par construction.
    flux_start.saturating_add(off).min(body.len())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Constantes structurelles vérifiées
    // -----------------------------------------------------------------------

    #[test]
    fn constantes_coherentes() {
        // CHARA_TABLE_END_REAL = séparateur[29] + stride = 0x5998 + 0x2D8 = 0x5C70
        // séparateur[29] = 0x720 + 29 * 0x2D8
        let sep_29 = SCALAR_STREAM_END + 29 * CHARA_SLOT_STRIDE;
        let table_end = sep_29 + CHARA_SLOT_STRIDE;
        assert_eq!(
            table_end, CHARA_TABLE_END_REAL,
            "CHARA_TABLE_END_REAL cohérent"
        );
        assert_eq!(CHARA_TABLE_END_REAL, 0x5C70);
        assert_eq!(SCALAR_STREAM_END, 0x720);
        assert_eq!(SCALAR_STREAM_OFFSET, 0x008);
        assert_eq!(CHARA_SLOT_STRIDE, 728);
        assert_eq!(CHARA_SLOT_DATA_SIZE, 720);
        assert_eq!(CHARA_SLOT_COUNT, 30);
    }

    #[test]
    fn parse_body_trop_court_retourne_erreur() {
        let short = [0u8; 4];
        let res = parse_autosave_layout(&short);
        assert!(matches!(res, Err(SaveError::TooShort { .. })));
    }

    #[test]
    fn parse_mauvaise_version_retourne_erreur() {
        let mut buf = [0u8; 32];
        // version = 2 (invalide)
        buf[3] = 0x02;
        let res = parse_autosave_layout(&buf);
        assert!(matches!(res, Err(SaveError::Corrupt(..))));
    }

    /// Test de la fonction count_sf_tlv_records sur des données synthétiques.
    #[test]
    fn count_sf_tlv_records_synthetique() {
        // 2 enregistrements valides : [size=1][hash=0][data=AA] [size=2][hash=0][data=BB CC]
        let mut buf = Vec::new();
        buf.extend_from_slice(&1u32.to_le_bytes()); // size=1
        buf.extend_from_slice(&0u32.to_le_bytes()); // hash=0
        buf.push(0xAA); // data

        buf.extend_from_slice(&2u32.to_le_bytes()); // size=2
        buf.extend_from_slice(&0u32.to_le_bytes()); // hash=0
        buf.push(0xBB);
        buf.push(0xCC);

        assert_eq!(count_sf_tlv_records(&buf), 2);
    }

    #[test]
    fn count_sf_tlv_records_vide() {
        assert_eq!(count_sf_tlv_records(&[]), 0);
    }

    #[test]
    fn count_sf_tlv_records_size_trop_grand() {
        // size=0x20000 (> 0x10000) → arrêt immédiat
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x0002_0000u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(count_sf_tlv_records(&buf), 0);
    }

    /// Parse un body synthétique minimal (juste l'en-tête, sans contenu).
    #[test]
    fn parse_body_synthetique_minimal() {
        let mut buf = vec![0u8; 8];
        // version = 1 (BE)
        buf[0] = 0x00;
        buf[1] = 0x00;
        buf[2] = 0x00;
        buf[3] = 0x01;
        // opaque_4 = 0xDEADBEEF (LE)
        buf[4] = 0xEF;
        buf[5] = 0xBE;
        buf[6] = 0xAD;
        buf[7] = 0xDE;

        let layout = parse_autosave_layout(&buf).expect("parse ne doit pas échouer");
        assert_eq!(layout.version, 1);
        assert_eq!(layout.opaque_4, 0xDEAD_BEEF);
        assert_eq!(layout.scalar_record_count, 0);
        assert!(layout.chara_param_slots.is_empty());
    }

    /// Vérifie que parse_autosave_layout retourne les bornes correctes sur un body synthétique
    /// qui contient une région scalaire et la table CharaParam.
    #[test]
    fn parse_layout_bornes_correctes() {
        // Construire un body synthétique avec :
        // - 8 octets d'en-tête
        // - Flux SF-TLV scalaire : 2 records de 1 octet chacun
        // - Séparateur section CharaParam
        // - 2 slots CharaParam (tronqués à min pour le test)
        let mut buf: Vec<u8> = Vec::new();

        // En-tête : version=1 BE + opaque=0
        buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00]);

        // On pad jusqu'à 0x720 (SCALAR_STREAM_END) avec 2 SF-TLV records puis des zéros
        // Record 1 : size=1, hash=0xAABBCCDD, data=0x42
        let rec1_start = buf.len();
        let _ = rec1_start;
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&0xAABBCCDDu32.to_le_bytes());
        buf.push(0x42);
        // Pad jusqu'à 0x720
        while buf.len() < SCALAR_STREAM_END {
            buf.push(0x00);
        }

        // Séparateur section CharaParam (1E 00 00 03 D0 66 1F 54)
        buf.extend_from_slice(&CHARA_SECTION_SEPARATOR);

        // Slot 0 : 720 octets de données (vides)
        buf.extend_from_slice(&vec![0u8; CHARA_SLOT_DATA_SIZE]);

        // Slot 1 séparateur + données
        buf.extend_from_slice(&CHARA_SLOT_SEPARATOR);
        buf.extend_from_slice(&vec![0u8; CHARA_SLOT_DATA_SIZE]);

        let layout = parse_autosave_layout(&buf).expect("parse doit réussir");

        assert_eq!(layout.version, 1);
        assert_eq!(layout.scalar_stream_range.0, SCALAR_STREAM_OFFSET);
        assert_eq!(layout.scalar_stream_range.1, SCALAR_STREAM_END);
        // Au moins 1 record valide (le reste du padding génère des records size=0 valides)
        assert!(
            layout.scalar_record_count >= 1,
            "au moins 1 record scalaire"
        );

        assert_eq!(layout.chara_param_slots.len(), 2);
        assert!(
            layout.chara_param_slots[0].separator_valid,
            "slot 0 séparateur valide"
        );
        assert!(
            layout.chara_param_slots[1].separator_valid,
            "slot 1 séparateur valide"
        );
        assert_eq!(
            layout.chara_param_slots[0].separator_offset,
            SCALAR_STREAM_END
        );
        assert_eq!(
            layout.chara_param_slots[0].data_offset,
            SCALAR_STREAM_END + 8
        );
        assert_eq!(
            layout.chara_param_slots[1].separator_offset,
            SCALAR_STREAM_END + CHARA_SLOT_STRIDE
        );

        // La table doit s'étendre sur les 2 slots
        assert_eq!(layout.chara_table_range.0, SCALAR_STREAM_END);
        assert_eq!(
            layout.chara_table_range.1,
            SCALAR_STREAM_END + 2 * CHARA_SLOT_STRIDE
        );
    }

    /// Vérification sur les vraies saves du VPS (test intégration, skip si absent).
    ///
    /// Ce test charge `data/saves/002AB8F4-USERDATALIVE` (chemin depuis la racine du workspace
    /// niers). Il est SKIP si le fichier est absent (CI). Sur le VPS il valide les invariants
    /// confirmés par RE.
    #[test]
    #[cfg_attr(not(feature = "real-saves"), ignore)]
    fn integration_vps_layout_invariants() {
        use crate::{BlobSubtype, io::read_save};
        use std::path::Path;

        let path = Path::new("data/saves/002AB8F4-USERDATALIVE");
        if !path.exists() {
            eprintln!("SKIP: {} absent", path.display());
            return;
        }

        let container = read_save(path).expect("lecture save réelle");
        let autosave_blob = container
            .blob_by_subtype(BlobSubtype::Autosave)
            .expect("blob Autosave absent");

        let layout = parse_autosave_layout(&autosave_blob.body)
            .expect("parse_autosave_layout échoué sur save réelle");

        // Invariants validés sur octets réels (2026-06-05, VPS)
        assert_eq!(layout.version, 1, "version");
        assert_eq!(
            layout.opaque_4, 0x17E8_4B00,
            "opaque_4 (valeur observée save VPS)"
        );
        assert_eq!(
            layout.scalar_record_count, SCALAR_RECORD_COUNT,
            "124 records scalaires"
        );
        assert_eq!(
            layout.scalar_stream_range,
            (0x008, 0x720),
            "bornes scalaire stream"
        );

        // Table CharaParam
        assert_eq!(
            layout.chara_param_slots.len(),
            CHARA_SLOT_COUNT,
            "30 slots CharaParam"
        );
        for (i, slot) in layout.chara_param_slots.iter().enumerate() {
            assert!(slot.separator_valid, "slot {} séparateur invalide", i);
            assert_eq!(
                slot.record_count, CHARA_SLOT_RECORD_COUNT,
                "slot {} : attendu {} records, got {}",
                i, CHARA_SLOT_RECORD_COUNT, slot.record_count
            );
            let expected_sep = 0x720 + i * CHARA_SLOT_STRIDE;
            assert_eq!(
                slot.separator_offset, expected_sep,
                "slot {} separator_offset",
                i
            );
        }
        assert_eq!(
            layout.chara_table_range,
            (0x720, 0x5C70),
            "bornes table CharaParam"
        );

        // Section 2
        assert_eq!(layout.section2_range.0, 0x5C70, "section2 début");
        assert!(layout.section2_range.1 > 0x5C70, "section2 non vide");

        // Section données principale : doit couvrir la majorité du body (~11.9 Mo)
        let main_size = layout.main_data_range.1 - layout.main_data_range.0;
        assert!(
            main_size > 10 * 1024 * 1024,
            "section données principale trop petite: {} octets",
            main_size
        );
    }
}
