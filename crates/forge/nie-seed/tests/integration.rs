//! Tests d'intégration — lancés avec `cargo test -p nie-seed -- --ignored`.
//!
//! Ces tests requièrent les fichiers de données réels présents sur le VPS.
//! Ils sont ignorés par défaut pour ne pas bloquer la CI sans données.

#![allow(clippy::pedantic)]

use std::path::Path;

use nie_index::Db;
use nie_seed::{formats, inagle, rtti_classes};

/// Chemin de la base niers.sqlite déjà initialisée (binary_id = 1).
const NIERS_SQLITE: &str = "var/niers.sqlite";
const BINARY_ID: i64 = 1;

/// Chemin du fichier RTTI.
const RTTI_PATH: &str = "refs/iecode-re/research/nie-rtti-classes.txt";

/// Répertoire des backups SQLite inagle.
const INAGLE_DIR: &str = "refs/azalee-backups";

/// Ingestion complète via `ingest_all` sur la vraie DB.
#[test]
#[ignore]
fn ingest_all_reel() {
    let mut db = Db::open(NIERS_SQLITE).expect("ouverture niers.sqlite");

    let refs_root = Path::new("refs");
    let inagle_dir = Path::new(INAGLE_DIR);

    // Catalogue iecode optionnel : présent si `iecode export-knowledge` a été lancé.
    let catalog = Path::new("/tmp/iecode-format-catalog.json");
    let catalog_opt = catalog.exists().then_some(catalog);

    let stats = nie_seed::ingest_all(&mut db, BINARY_ID, refs_root, Some(inagle_dir), catalog_opt)
        .expect("ingest_all");

    println!(
        "=== ingest_all résultats ===\n\
         classes RTTI    : {}\n\
         formats         : {}\n\
         catalogue (fmt) : {}\n\
         catalogue (champs) : {}\n\
         hash inagle     : {}\n\
         ancres          : {}",
        stats.rtti_classes,
        stats.formats,
        stats.catalog_formats,
        stats.catalog_fields,
        stats.hash_names,
        stats.anchors
    );

    assert!(
        stats.rtti_classes >= 1234,
        "attendu >= 1234 classes RTTI, obtenu {}",
        stats.rtti_classes
    );
    assert!(
        stats.formats >= 24,
        "attendu >= 24 formats, obtenu {}",
        stats.formats
    );
    assert!(
        stats.hash_names > 0,
        "aucun hash inagle inséré — vérifier {}",
        INAGLE_DIR
    );
}

/// Test d'ingestion RTTI seule contre le fichier réel.
#[test]
#[ignore]
fn ingest_rtti_reel() {
    let mut db = Db::open_in_memory().expect("DB mémoire");
    let bin = db
        .upsert_binary("nie.exe", "test-rtti", "x86_64", 64, 0, 0, None, None)
        .expect("upsert_binary");

    let path = Path::new(RTTI_PATH);
    let count = rtti_classes::ingest_rtti_classes(&mut db, bin, path).expect("ingest_rtti");

    println!("Classes RTTI insérées : {count}");
    assert_eq!(
        count, 1234,
        "le fichier doit contenir exactement 1234 classes"
    );

    // Vérifier quelques classes connues.
    for cls in &["BallComponent", "CCamera", "SoccerValidConditionManager"] {
        let found: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM rtti_class WHERE binary_id=?1 AND name=?2",
                rusqlite::params![bin, cls],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 1, "classe {cls} introuvable");
    }
}

/// Test d'ingestion des formats iecode.
#[test]
#[ignore]
fn ingest_formats_reel() {
    let mut db = Db::open_in_memory().expect("DB mémoire");
    let count = formats::ingest_formats(&mut db).expect("ingest_formats");
    println!("Formats insérés : {count}");
    assert!(count >= 24, "attendu >= 24 formats, obtenu {count}");
}

/// Test d'ingestion inagle contre le vrai miroir SQLite.
#[test]
#[ignore]
fn ingest_inagle_reel() {
    let mut db = Db::open_in_memory().expect("DB mémoire");
    let dir = Path::new(INAGLE_DIR);
    let count = inagle::ingest_inagle_hashes(&mut db, dir).expect("ingest_inagle");

    println!("Hash inagle insérés : {count}");

    // Comptes attendus : ≥ 6090 chara + 1668 items + 208 teams + 70 tactics
    // + 9 auras + 56 souls + ~182 quests avec noms = > 8000.
    assert!(
        count > 7000,
        "attendu > 7000 hash inagle, obtenu {count} — vérifier {}",
        INAGLE_DIR
    );

    // Vérifier Mark Evans (hash connu).
    let mark: Option<String> = db
        .conn()
        .query_row(
            "SELECT name FROM hash_name WHERE hash = ?1 AND kind = 'chara' LIMIT 1",
            [0x3055_CF22_i64],
            |r| r.get(0),
        )
        .ok();
    assert_eq!(
        mark.as_deref(),
        Some("Mark Evans"),
        "Mark Evans (0x3055CF22) introuvable dans hash_name"
    );
}
