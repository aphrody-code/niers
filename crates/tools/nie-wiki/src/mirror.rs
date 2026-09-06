//! Résolution du miroir SQLite et ouverture en lecture seule.
//!
//! Ordre de résolution (identique à l'azalee CLI TS) :
//! 1. Flag `--db` (prioritaire, passé en argument)
//! 2. Variable d'environnement `NIE_WIKI_DB` ou `SQLITE_DB_PATH`
//! 3. Fichier `supabase-*.sqlite` ou `inagle-*.sqlite` le plus récent dans
//!    `var/miroir` ou `data/backups`

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use rusqlite::{Connection, OpenFlags};

/// Ouvre la connexion SQLite en lecture seule sur le miroir résolu.
///
/// # Erreurs
///
/// Retourne une erreur si aucun miroir n'est trouvé ou si l'ouverture échoue.
pub fn open(db_override: Option<&Path>) -> anyhow::Result<Connection> {
    let path = resolve(db_override)?;
    tracing::debug!("miroir SQLite : {}", path.display());
    let conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("ouverture SQLite {}", path.display()))?;
    // Activer WAL read-only pour les fichiers en cours d'écriture par le backup.
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    Ok(conn)
}

/// Retourne le chemin résolu du miroir SQLite.
pub fn resolve(db_override: Option<&Path>) -> anyhow::Result<PathBuf> {
    // 1. Flag --db passé directement
    if let Some(p) = db_override {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        bail!("base spécifiée introuvable : {}", p.display());
    }

    // 2. Variables d'environnement
    for var in &["NIE_WIKI_DB", "SQLITE_DB_PATH"] {
        if let Ok(v) = std::env::var(var) {
            let p = PathBuf::from(&v);
            if p.exists() {
                return Ok(p);
            }
        }
    }

    // 3. Répertoire de backups : fichier supabase-*.sqlite le plus récent (tri lexicographique)
    let mut candidates = Vec::new();
    for dir in [PathBuf::from("var/miroir"), PathBuf::from("data/backups")] {
        if dir.is_dir() {
            candidates.extend(latest_sqlite_in(&dir));
        }
    }
    if let Some(latest) = candidates.into_iter().max() {
        return Ok(latest);
    }

    bail!(
        "aucun miroir SQLite trouvé — utilisez --db, NIE_WIKI_DB ou placez un fichier \
         supabase-*.sqlite dans data/backups"
    )
}

/// Trouve les fichiers `supabase-*.sqlite` et `inagle-*.sqlite` non vides.
///
/// Les fichiers 0-octet (en cours de création par le backup) sont ignorés.
fn latest_sqlite_in(dir: &Path) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|ext| ext == "sqlite")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("supabase-") || n.starts_with("inagle-"))
                // Ignorer les fichiers vides (en cours de création)
                && p.metadata().is_ok_and(|m| m.len() > 0)
        })
        .collect();
    entries.sort();
    entries.into_iter().next_back()
}

/// Exécute une requête qui retourne des colonnes sous forme de chaînes.
///
/// Commodité pour les appels one-off sans paramètres typés complexes.
pub fn query_rows<F, T>(
    conn: &Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    map: F,
) -> anyhow::Result<Vec<T>>
where
    F: Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut stmt = conn.prepare(sql).context("préparation SQL")?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().copied()), map)
        .context("exécution SQL")?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row.context("lecture ligne")?);
    }
    Ok(results)
}

/// Même signature mais retourne `None` si aucune ligne.
pub fn query_one<F, T>(
    conn: &Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    map: F,
) -> anyhow::Result<Option<T>>
where
    F: Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut stmt = conn.prepare(sql).context("préparation SQL")?;
    let mut rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().copied()), map)
        .context("exécution SQL")?;
    match rows.next() {
        None => Ok(None),
        Some(r) => Ok(Some(r.context("lecture ligne")?)),
    }
}

/// Fusionne `data` JSON + `sheet_data` JSON (sheet_data prioritaire si non-null/non-vide).
///
/// Fidèle au comportement TS : `{ ...row.data, ...row.sheet_data }`.
pub fn merge_data_sheet(data: Option<&str>, sheet_data: Option<&str>) -> serde_json::Value {
    let base = data
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Object(Default::default()));
    let overlay = sheet_data
        .filter(|s| !s.is_empty() && *s != "null")
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Object(Default::default()));

    match (base, overlay) {
        (serde_json::Value::Object(mut b), serde_json::Value::Object(o)) => {
            for (k, v) in o {
                b.insert(k, v);
            }
            serde_json::Value::Object(b)
        }
        // Si l'un n'est pas un objet, sheet_data gagne entièrement si présent.
        (_b, serde_json::Value::Object(o)) if !o.is_empty() => serde_json::Value::Object(o),
        (b, _) => b,
    }
}
