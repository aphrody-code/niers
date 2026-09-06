//! Test d'intégration : parse le body HEADERSAVE réel et valide les ancres connues.
//!
//! Ce test nécessite le fichier de save réel sur le VPS.
//! Si le fichier est absent, le test est ignoré (skip soft).

use nie_save::body::headersave::parse_headersave;
use nie_save::{BlobSubtype, parse};

const SAVE_PATH: &str = "data/saves/002AB8F4-USERDATALIVE";
const SAVE_NAME: &str = "002AB8F4-USERDATALIVE";

/// Ancres connues validées manuellement sur la save VPS.
const EXPECTED_PLAYER_NAME: &str = "AstraJinWoo";
const EXPECTED_LEVEL_STR: &str = "218";
const EXPECTED_UNIQUE_ID: &str = "00023798fc9b4d49b9a36b6b0cb8d50b";
const EXPECTED_FORMAT_VERSION: u32 = 1;
const EXPECTED_MAX_SLOTS: u32 = 10;
const EXPECTED_USED_SLOTS: u32 = 7;
const EXPECTED_SAVE_YEAR: u16 = 2026;
const EXPECTED_SAVE_MONTH: u8 = 2;
const EXPECTED_SAVE_DAY: u8 = 21;

#[test]
fn parse_headersave_ancres_validees() {
    if !std::path::Path::new(SAVE_PATH).exists() {
        // Fichier absent : skip silencieux (VPS uniquement)
        return;
    }

    let data = std::fs::read(SAVE_PATH).expect("lecture save");
    let container = parse(&data, SAVE_NAME).expect("parse conteneur");

    let headersave_blob = container
        .blobs
        .iter()
        .find(|b| b.header.subtype == BlobSubtype::Headersave)
        .expect("blob HEADERSAVE absent");

    let hs = parse_headersave(&headersave_blob.body).expect("parse_headersave échoué");

    // --- Champs d'en-tête global ---
    assert_eq!(
        hs.format_version, EXPECTED_FORMAT_VERSION,
        "format_version incorrect"
    );
    assert_eq!(hs.max_slots, EXPECTED_MAX_SLOTS, "max_slots incorrect");
    assert_eq!(hs.used_slots, EXPECTED_USED_SLOTS, "used_slots incorrect");
    assert!(hs.used_slots <= hs.max_slots, "used_slots > max_slots");

    // --- Horodatage de sauvegarde ---
    assert!(
        !hs.save_timestamp.is_unset(),
        "save_timestamp est unset (attendu 2026-02-21)"
    );
    assert_eq!(
        hs.save_timestamp.year, EXPECTED_SAVE_YEAR,
        "année incorrecte"
    );
    assert_eq!(
        hs.save_timestamp.month, EXPECTED_SAVE_MONTH,
        "mois incorrect"
    );
    assert_eq!(hs.save_timestamp.day, EXPECTED_SAVE_DAY, "jour incorrect");

    // --- Ancres joueur ---
    assert_eq!(
        hs.player_name, EXPECTED_PLAYER_NAME,
        "player_name incorrect"
    );
    assert_eq!(hs.level_str, EXPECTED_LEVEL_STR, "level_str incorrect");
    assert_eq!(hs.unique_id, EXPECTED_UNIQUE_ID, "unique_id incorrect");

    // --- Cohérence de la table de slots ---
    assert!(!hs.slots.is_empty(), "table de slots vide");
    assert!(hs.slots.len() <= 28, "plus de 28 slots (max supporté)");

    // Au moins un slot actif dans la section secondaire (index ≥ 14)
    let active_count = hs.slots.iter().filter(|s| s.is_active).count();
    assert!(active_count > 0, "aucun slot actif trouvé");

    // Le slot TYPE-B avec l'horodatage 2026-05-08T12:41:54 doit exister
    let type_b_with_dt = hs.slots.iter().find(|s| {
        s.slot_datetime
            .as_ref()
            .is_some_and(|dt| dt.year == 2026 && !dt.is_unset())
    });
    assert!(
        type_b_with_dt.is_some(),
        "aucun slot TYPE-B avec horodatage 2026 trouvé"
    );

    // body_opaque doit contenir le body complet
    assert_eq!(
        hs.body_opaque.len(),
        headersave_blob.body.len(),
        "body_opaque taille incorrecte"
    );

    // Sortie terse (format niers convention : clé=val)
    println!(
        "headersave ok joueur={} niveau={} uid={} slots={} actifs={} ts={:04}-{:02}-{:02}",
        hs.player_name,
        hs.level_str,
        hs.unique_id,
        hs.slots.len(),
        active_count,
        hs.save_timestamp.year,
        hs.save_timestamp.month,
        hs.save_timestamp.day,
    );
}
