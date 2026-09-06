//! Ingestion du catalogue de formats exporté par iecode (`iecode export-knowledge`).
//!
//! iecode (C# .NET 10) ne peut pas être appelé depuis niers (full-Rust). Il publie
//! donc son savoir sur les formats binaires LEVEL-5 / CRIWARE d'IEVR sous forme d'un
//! artefact JSON stable et versionné, produit par :
//!
//! ```sh
//! iecode export-knowledge --out <catalogue.json>
//! ```
//!
//! Ce module lit ce JSON et l'ingère comme ancres de vérité :
//! - chaque format → table `format` (via [`nie_index::ingest::format`]) ;
//! - chaque champ d'en-tête documenté → table `format_field` (offset/taille/type/nom/note).
//!
//! ## Schéma JSON attendu (schema_version 1)
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "generator": "iecode",
//!   "game": "Inazuma Eleven: Victory Road",
//!   "format_count": 31,
//!   "formats": [
//!     {
//!       "name": "G4TX",
//!       "category": "Texture",
//!       "extensions": ["g4tx"],
//!       "magic": "\"G4TX\"",
//!       "magic_hex": 1481913415,
//!       "endianness": "Little",
//!       "header_size": 96,
//!       "description": "Conteneur de textures LEVEL-5 …",
//!       "doc": "Formats/Level5/G4txParser.cs",
//!       "fields": [
//!         { "offset": 0, "size": 4, "type": "magic", "name": "Magic", "note": "\"G4TX\" …" }
//!       ]
//!     }
//!   ]
//! }
//! ```
//!
//! Contrairement à [`crate::formats`] (catalogue minimal codé en dur : nom + magic),
//! ce module récupère le savoir COMPLET d'iecode, y compris le layout des champs
//! d'en-tête, sans dupliquer la connaissance dans le code Rust.

use std::path::Path;

use anyhow::{Context, Result, bail};
use nie_index::{Db, ingest};
use serde::Deserialize;
use tracing::debug;

/// Version de schéma supportée par cet ingesteur. Doit correspondre au
/// `schema_version` produit par `iecode export-knowledge`.
pub const SUPPORTED_SCHEMA_VERSION: i64 = 1;

/// Document racine du catalogue iecode.
#[derive(Debug, Deserialize)]
struct CatalogDocument {
    schema_version: i64,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    formats: Vec<FormatEntry>,
}

/// Une définition de format dans le catalogue.
#[derive(Debug, Deserialize)]
struct FormatEntry {
    name: String,
    magic: Option<String>,
    doc: Option<String>,
    #[serde(default)]
    fields: Vec<FieldEntry>,
}

/// Un champ d'en-tête documenté d'un format.
#[derive(Debug, Deserialize)]
struct FieldEntry {
    offset: Option<i64>,
    size: Option<i64>,
    #[serde(rename = "type")]
    ty: Option<String>,
    name: String,
    note: Option<String>,
}

/// Résultat d'une passe d'ingestion du catalogue.
#[derive(Debug, Default, Clone, Copy)]
pub struct CatalogStats {
    /// Nombre de formats insérés/mis à jour.
    pub formats: usize,
    /// Nombre de champs d'en-tête insérés.
    pub fields: usize,
}

/// Ingère le catalogue de formats iecode depuis le fichier JSON `path`.
///
/// Source déclarée pour les formats : `"iecode-catalog"` (distincte de la source
/// `"iecode"` du catalogue minimal codé en dur dans [`crate::formats`], pour tracer
/// l'origine machine-readable).
///
/// L'opération est idempotente : ré-ingérer le même catalogue ne crée pas de doublon
/// de format (`ON CONFLICT(name)`), mais purge et réinsère les champs de chaque format
/// pour rester aligné sur la dernière version exportée.
///
/// # Erreurs
///
/// Retourne une erreur si le fichier est introuvable/illisible, si le JSON est
/// invalide, ou si la `schema_version` n'est pas supportée.
pub fn ingest_format_catalog(db: &mut Db, path: &Path) -> Result<CatalogStats> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("lecture du catalogue iecode : {}", path.display()))?;

    let doc: CatalogDocument = serde_json::from_str(&raw)
        .with_context(|| format!("JSON invalide : {}", path.display()))?;

    if doc.schema_version != SUPPORTED_SCHEMA_VERSION {
        bail!(
            "schema_version {} non supportée (attendu {SUPPORTED_SCHEMA_VERSION}) — \
             mettre à jour nie-seed::format_catalog",
            doc.schema_version
        );
    }

    debug!(
        "catalogue iecode : generator={} formats={}",
        doc.generator,
        doc.formats.len()
    );

    let tx = db
        .conn_mut()
        .transaction()
        .context("ouverture de transaction format_catalog")?;

    let mut stats = CatalogStats::default();

    for fmt in &doc.formats {
        let format_id = ingest::format(
            &tx,
            &fmt.name,
            fmt.magic.as_deref(),
            "iecode-catalog",
            fmt.doc.as_deref(),
        )
        .with_context(|| format!("ingest format({})", fmt.name))?;
        stats.formats += 1;

        // Purge les champs existants de ce format puis réinsère (idempotence
        // sur ré-export). La table format_field n'a pas de contrainte d'unicité.
        tx.execute("DELETE FROM format_field WHERE format_id = ?1", [format_id])
            .with_context(|| format!("purge format_field({})", fmt.name))?;

        for field in &fmt.fields {
            tx.execute(
                "INSERT INTO format_field(format_id, offset, size, ty, name, note)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    format_id,
                    field.offset,
                    field.size,
                    field.ty.as_deref(),
                    field.name,
                    field.note.as_deref(),
                ],
            )
            .with_context(|| format!("insert format_field({}.{})", fmt.name, field.name))?;
            stats.fields += 1;
        }

        debug!(
            "format catalogue : {} (id={format_id}, {} champs)",
            fmt.name,
            fmt.fields.len()
        );
    }

    tx.commit().context("commit format_catalog")?;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Écrit un catalogue JSON minimal dans un fichier temporaire et renvoie son chemin.
    fn write_catalog(json: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "nie-seed-catalog-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        path.push(unique);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        path
    }

    const SAMPLE: &str = r#"{
        "schema_version": 1,
        "generator": "iecode",
        "game": "Inazuma Eleven: Victory Road",
        "format_count": 2,
        "formats": [
            {
                "name": "G4TX",
                "category": "Texture",
                "extensions": ["g4tx"],
                "magic": "\"G4TX\"",
                "magic_hex": 1481913415,
                "endianness": "Little",
                "header_size": 96,
                "description": "Conteneur de textures LEVEL-5",
                "doc": "Formats/Level5/G4txParser.cs",
                "fields": [
                    { "offset": 0, "size": 4, "type": "magic", "name": "Magic", "note": "G4TX" },
                    { "offset": 4, "size": 2, "type": "i16", "name": "HeaderSize" }
                ]
            },
            {
                "name": "cfg.bin",
                "category": "Config",
                "extensions": ["cfg.bin"],
                "magic": null,
                "endianness": "Little",
                "description": "Configuration binaire LEVEL-5",
                "doc": "Formats/Level5/CfgBin/CfgBin.cs",
                "fields": []
            }
        ]
    }"#;

    #[test]
    fn ingest_sample_catalogue() {
        let path = write_catalog(SAMPLE);
        let mut db = Db::open_in_memory().unwrap();
        let stats = ingest_format_catalog(&mut db, &path).unwrap();
        assert_eq!(stats.formats, 2, "deux formats attendus");
        assert_eq!(stats.fields, 2, "deux champs attendus (G4TX)");

        // Le format G4TX est présent et ses champs aussi.
        let n_g4tx: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM format WHERE name='G4TX'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n_g4tx, 1);

        let n_fields: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM format_field ff
                 JOIN format f ON f.id = ff.format_id WHERE f.name='G4TX'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n_fields, 2);

        // Source machine-readable correctement tracée.
        let src: String = db
            .conn()
            .query_row("SELECT source FROM format WHERE name='G4TX'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(src, "iecode-catalog");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn ingest_is_idempotent() {
        let path = write_catalog(SAMPLE);
        let mut db = Db::open_in_memory().unwrap();
        let s1 = ingest_format_catalog(&mut db, &path).unwrap();
        let s2 = ingest_format_catalog(&mut db, &path).unwrap();
        assert_eq!(s1.formats, s2.formats);
        assert_eq!(s1.fields, s2.fields);

        // Pas de doublon de format ni de champ après deux passes.
        let n_fmt: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM format", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n_fmt, 2);

        let n_field: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM format_field", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n_field, 2, "les champs ne doivent pas se dupliquer");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn rejette_schema_version_inconnue() {
        let path = write_catalog(r#"{ "schema_version": 999, "formats": [] }"#);
        let mut db = Db::open_in_memory().unwrap();
        let err = ingest_format_catalog(&mut db, &path).unwrap_err();
        assert!(
            err.to_string().contains("schema_version"),
            "l'erreur doit mentionner schema_version : {err}"
        );
        std::fs::remove_file(&path).ok();
    }
}
