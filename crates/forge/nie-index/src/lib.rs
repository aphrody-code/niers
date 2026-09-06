//! Base de connaissance sqlite de la boucle RE niers.
//!
//! Tout ce que la boucle découvre (index Ghidra de nie.exe, RTTI, xrefs, strings) ou
//! importe (savoir fusionné iecode/inagle : formats, hash→nom) est durable ici. Le
//! module [`ingest`] fournit des insertions rapides (statements préparés/cachés sous
//! transaction) pour charger les ~60 000 fonctions de `nie-index.json`, et [`query`]
//! les agrégats de couverture.
#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

use rusqlite::Connection;
use thiserror::Error;

pub use rusqlite;

/// Schéma SQL embarqué.
pub const SCHEMA: &str = include_str!("schema.sql");

/// Migration « caméra » embarquée : tables `cam_*` et vues associées.
///
/// Indexe la carte du reverse caméra, les fichiers de données, `soccer_camera_config`,
/// les presets de contrôleur et les 1 215 animations `.g4cm` (jusqu'aux échantillons de
/// keyframes). Peuplée par `nie-cam index` (crate `nie-camera`).
pub const CAMERA_SCHEMA: &str = include_str!("camera.sql");

/// Version du schéma (clé `meta.schema_version`). `2` = schéma de base + migration caméra.
pub const SCHEMA_VERSION: &str = "2";

/// Colonnes ajoutées après coup : `(table, colonne, type)`.
///
/// `CREATE TABLE IF NOT EXISTS` ne modifie pas une table déjà créée, et SQLite n'a pas
/// d'`ALTER TABLE … ADD COLUMN IF NOT EXISTS` : ces colonnes sont donc déclarées dans
/// `schema.sql` (bases neuves) **et** ajoutées ici une à une (bases existantes), après
/// interrogation de `pragma_table_info`. Ajouter une colonne nullable est une opération de
/// métadonnées en SQLite : le coût ne dépend pas du nombre de lignes.
const ADDED_COLUMNS: &[(&str, &str, &str)] = &[
    // Encodage lu dans l'image (`ascii`|`utf16`).
    ("str", "kind", "TEXT"),
    // Provenance : NULL = index Ghidra replié, `rdata-xref` = désassemblage réel.
    ("func_str_ref", "source", "TEXT"),
];

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("redis pool error: {0}")]
    RedisPool(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, IndexError>;

/// Connexion à la base de connaissance (SQLite + optionnellement Redis).
pub struct Db {
    conn: Connection,
    redis_pool: Option<r2d2::Pool<redis::Client>>,
}

impl Db {
    /// Ouvre (ou crée) la base au chemin donné et applique le schéma.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn,
            redis_pool: None,
        };
        db.init()?;
        Ok(db)
    }

    /// Ouvre la base SQLite et initialise également la connexion à Redis via un pool.
    pub fn open_with_redis(path: impl AsRef<std::path::Path>, redis_url: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let client = redis::Client::open(redis_url)?;
        let redis_pool = r2d2::Pool::builder()
            .build(client)
            .map_err(|e| IndexError::RedisPool(e.to_string()))?;

        let db = Self {
            conn,
            redis_pool: Some(redis_pool),
        };
        db.init()?;
        Ok(db)
    }

    /// Base en mémoire (tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn,
            redis_pool: None,
        };
        db.init()?;
        Ok(db)
    }

    /// Applique le schéma **et** les migrations (idempotent), puis enregistre la version.
    pub fn init(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        self.conn.execute_batch(CAMERA_SCHEMA)?;
        self.migrate_columns()?;
        self.set_meta("schema_version", SCHEMA_VERSION)?;
        Ok(())
    }

    /// Ajoute les colonnes de [`ADDED_COLUMNS`] absentes de la base (idempotent).
    fn migrate_columns(&self) -> Result<()> {
        for (table, column, ty) in ADDED_COLUMNS {
            let present: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
                (table, column),
                |r| r.get(0),
            )?;
            if present == 0 {
                self.conn
                    .execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {ty}"))?;
            }
        }
        Ok(())
    }

    /// Applique seulement la migration caméra (tables `cam_*` + vues).
    ///
    /// `init` l'applique déjà ; cette fonction sert à l'appliquer sur une base ouverte
    /// autrement, ou à la rejouer après édition du fichier SQL.
    pub fn init_camera(&self) -> Result<()> {
        self.conn.execute_batch(CAMERA_SCHEMA)?;
        Ok(())
    }

    /// Accès lecture seule à la connexion sous-jacente.
    #[must_use]
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Accès mutable (pour ouvrir une transaction de chargement en masse).
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value),
        )?;
        if let Some(ref pool) = self.redis_pool
            && let Ok(mut rconn) = pool.get()
        {
            use redis::Commands;
            let redis_key = format!("niers:meta:{}", key);
            let _: std::result::Result<(), redis::RedisError> = rconn.set(redis_key, value);
        }
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        if let Some(ref pool) = self.redis_pool
            && let Ok(mut rconn) = pool.get()
        {
            use redis::Commands;
            let redis_key = format!("niers:meta:{}", key);
            if let Ok(val) = rconn.get::<_, String>(redis_key) {
                return Ok(Some(val));
            }
        }
        let v = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .ok();
        Ok(v)
    }

    /// Insère un binaire (idempotent par sha256) et renvoie son id.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_binary(
        &self,
        path: &str,
        sha256: &str,
        arch: &str,
        bits: i64,
        base_addr: i64,
        size: i64,
        pdb_ref: Option<&str>,
        compiled: Option<&str>,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO binary(path, sha256, arch, bits, base_addr, size, pdb_ref, compiled)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(sha256) DO UPDATE SET path = excluded.path",
            rusqlite::params![path, sha256, arch, bits, base_addr, size, pdb_ref, compiled],
        )?;
        let id = self
            .conn
            .query_row("SELECT id FROM binary WHERE sha256 = ?1", [sha256], |r| {
                r.get(0)
            })?;
        Ok(id)
    }

    /// Snapshot de couverture courant et enregistrement dans la table `coverage`.
    pub fn snapshot_coverage(&self, binary_id: i64) -> Result<query::Coverage> {
        let cov = query::coverage(&self.conn, binary_id)?;
        self.conn.execute(
            "INSERT INTO coverage(binary_id, total_funcs, named, classified, pct)
             VALUES(?1,?2,?3,?4,?5)",
            rusqlite::params![binary_id, cov.total, cov.named, cov.classified, cov.pct],
        )?;
        Ok(cov)
    }
}

/// Insertions rapides sous transaction. Chaque fonction utilise `prepare_cached` :
/// la première préparation compile le statement, les suivantes le réutilisent — ce qui
/// permet de charger ~60 000 fonctions + call-graph en quelques secondes.
///
/// Usage typique :
/// ```no_run
/// # use nie_index::{Db, ingest};
/// let mut db = Db::open("nie.sqlite").unwrap();
/// let bin = db.upsert_binary("nie.exe", "deadbeef", "x86_64", 64, 0x1_4000_0000, 0, None, None).unwrap();
/// let tx = db.conn_mut().transaction().unwrap();
/// let fid = ingest::function(&tx, bin, 0x1_4000_1000, Some("entry"), Some("re"), "lives", "entry_point", 0.9).unwrap();
/// ingest::str_ref(&tx, bin, fid, "CHARA_PARAM_INFO").unwrap();
/// tx.commit().unwrap();
/// ```
pub mod ingest {
    use super::Result;
    use rusqlite::{Connection, params};

    /// Insère/maj une fonction (idempotent par (binary, vaddr)). Renvoie son id.
    #[allow(clippy::too_many_arguments)]
    pub fn function(
        conn: &Connection,
        binary_id: i64,
        vaddr: i64,
        name: Option<&str>,
        name_source: Option<&str>,
        subsystem: &str,
        role: &str,
        confidence: f64,
    ) -> Result<i64> {
        conn.prepare_cached(
            "INSERT INTO function(binary_id, vaddr, name, name_source, subsystem, subsys_src, role, confidence)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(binary_id, vaddr) DO UPDATE SET
                name = COALESCE(excluded.name, function.name),
                subsystem = excluded.subsystem,
                role = excluded.role",
        )?
        .execute(params![binary_id, vaddr, name, name_source, subsystem, name_source, role, confidence])?;
        let id = conn.query_row(
            "SELECT id FROM function WHERE binary_id = ?1 AND vaddr = ?2",
            params![binary_id, vaddr],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    /// Met à jour les métriques d'une fonction (lignes, complexité, types).
    pub fn function_meta(
        conn: &Connection,
        function_id: i64,
        ret_type: Option<&str>,
        params_sig: Option<&str>,
        complexity: i64,
        n_lines: i64,
        pagerank: f64,
    ) -> Result<()> {
        conn.prepare_cached(
            "UPDATE function SET ret_type=?2, params=?3, complexity=?4, n_lines=?5, pagerank=?6 WHERE id=?1",
        )?
        .execute(params![function_id, ret_type, params_sig, complexity, n_lines, pagerank])?;
        Ok(())
    }

    /// Référence croisée (kind : call|data|code).
    pub fn xref(
        conn: &Connection,
        binary_id: i64,
        from_addr: i64,
        to_addr: i64,
        kind: &str,
    ) -> Result<()> {
        conn.prepare_cached(
            "INSERT OR IGNORE INTO xref(binary_id, from_addr, to_addr, kind) VALUES(?1,?2,?3,?4)",
        )?
        .execute(params![binary_id, from_addr, to_addr, kind])?;
        Ok(())
    }

    /// String référencée par une fonction (ancre de propagation).
    pub fn str_ref(conn: &Connection, binary_id: i64, function_id: i64, value: &str) -> Result<()> {
        conn.prepare_cached(
            "INSERT INTO func_str_ref(binary_id, function_id, value) VALUES(?1,?2,?3)",
        )?
        .execute(params![binary_id, function_id, value])?;
        Ok(())
    }

    /// Constante hex utilisée par une fonction (matching de magic de format).
    pub fn const_value(conn: &Connection, function_id: i64, value: i64) -> Result<()> {
        conn.prepare_cached("INSERT INTO func_const(function_id, value) VALUES(?1,?2)")?
            .execute(params![function_id, value])?;
        Ok(())
    }

    /// Global (DAT_*).
    pub fn glob(conn: &Connection, binary_id: i64, vaddr: i64, name: Option<&str>) -> Result<()> {
        conn.prepare_cached("INSERT OR IGNORE INTO glob(binary_id, vaddr, name) VALUES(?1,?2,?3)")?
            .execute(params![binary_id, vaddr, name])?;
        Ok(())
    }

    /// Classe RTTI (idempotent par nom). Renvoie son id.
    pub fn rtti_class(
        conn: &Connection,
        binary_id: i64,
        name: &str,
        namespace: Option<&str>,
        mangled: Option<&str>,
    ) -> Result<i64> {
        conn.prepare_cached(
            "INSERT OR IGNORE INTO rtti_class(binary_id, name, namespace, mangled) VALUES(?1,?2,?3,?4)",
        )?
        .execute(params![binary_id, name, namespace, mangled])?;
        let id = conn.query_row(
            "SELECT id FROM rtti_class WHERE binary_id=?1 AND name=?2",
            params![binary_id, name],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    /// Ancre de vérité pour la propagation (kind : func|string|rtti|format).
    pub fn anchor(
        conn: &Connection,
        binary_id: i64,
        vaddr: i64,
        label: &str,
        kind: &str,
        source: &str,
        confidence: f64,
    ) -> Result<()> {
        conn.prepare_cached(
            "INSERT OR IGNORE INTO anchor(binary_id, vaddr, label, kind, source, confidence)
             VALUES(?1,?2,?3,?4,?5,?6)",
        )?
        .execute(params![binary_id, vaddr, label, kind, source, confidence])?;
        Ok(())
    }

    /// Hypothèse ML/heuristique scorée.
    pub fn hypothesis(
        conn: &Connection,
        binary_id: i64,
        subject: &str,
        claim: &str,
        confidence: f64,
        source: &str,
    ) -> Result<()> {
        conn.prepare_cached(
            "INSERT INTO hypothesis(binary_id, subject, claim, confidence, source) VALUES(?1,?2,?3,?4,?5)",
        )?
        .execute(params![binary_id, subject, claim, confidence, source])?;
        Ok(())
    }

    /// Format Level-5 documenté (savoir iecode/inagle). Renvoie son id.
    pub fn format(
        conn: &Connection,
        name: &str,
        magic: Option<&str>,
        source: &str,
        doc: Option<&str>,
    ) -> Result<i64> {
        conn.prepare_cached(
            "INSERT INTO format(name, magic, source, doc) VALUES(?1,?2,?3,?4)
             ON CONFLICT(name) DO UPDATE SET magic=excluded.magic, doc=excluded.doc",
        )?
        .execute(params![name, magic, source, doc])?;
        let id = conn.query_row("SELECT id FROM format WHERE name=?1", [name], |r| r.get(0))?;
        Ok(id)
    }

    /// Table hash→nom d'inagle (CRC32/FNV des IDs).
    pub fn hash_name(
        conn: &Connection,
        hash: i64,
        kind: &str,
        name: &str,
        source: &str,
    ) -> Result<()> {
        conn.prepare_cached(
            "INSERT OR IGNORE INTO hash_name(hash, kind, name, source) VALUES(?1,?2,?3,?4)",
        )?
        .execute(params![hash, kind, name, source])?;
        Ok(())
    }
}

/// Requêtes d'agrégat (couverture, comptes).
pub mod query {
    use super::Result;
    use rusqlite::Connection;

    /// Instantané de couverture : fonctions totales, nommées, classifiées (subsystem
    /// résolu hors « standalone »/NULL), et le pourcentage classifié.
    #[derive(Debug, Clone, Copy)]
    pub struct Coverage {
        pub total: i64,
        pub named: i64,
        pub classified: i64,
        pub pct: f64,
    }

    pub fn coverage(conn: &Connection, binary_id: i64) -> Result<Coverage> {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM function WHERE binary_id=?1",
            [binary_id],
            |r| r.get(0),
        )?;
        // `GLOB`, pas `LIKE` : dans un motif `LIKE`, `_` est un joker et la
        // comparaison ignore la casse ASCII, si bien que `'FUN_%'` excluait
        // aussi tout nom commençant par « fun » + un caractère — dont les
        // 6 742 `funcLuaCmd_…`, qui disparaissaient du compte. `GLOB` est
        // sensible à la casse et n'accorde aucun sens spécial à `_`.
        let named: i64 = conn.query_row(
            "SELECT COUNT(*) FROM function WHERE binary_id=?1 AND name IS NOT NULL AND NOT (name GLOB 'FUN_*')",
            [binary_id],
            |r| r.get(0),
        )?;
        let classified: i64 = conn.query_row(
            "SELECT COUNT(*) FROM function
             WHERE binary_id=?1 AND subsystem IS NOT NULL AND subsystem <> 'standalone'",
            [binary_id],
            |r| r.get(0),
        )?;
        let pct = if total > 0 {
            (classified as f64) * 100.0 / (total as f64)
        } else {
            0.0
        };
        Ok(Coverage {
            total,
            named,
            classified,
            pct,
        })
    }

    /// Nombre de fonctions par sous-système.
    pub fn by_subsystem(conn: &Connection, binary_id: i64) -> Result<Vec<(String, i64)>> {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(subsystem,'?') s, COUNT(*) n FROM function WHERE binary_id=?1 GROUP BY s ORDER BY n DESC",
        )?;
        let rows = stmt
            .query_map([binary_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le filtre des noms Ghidra ne doit pas emporter les noms qui commencent
    /// par « fun ». Avec `LIKE 'FUN_%'`, `_` est un joker et la comparaison
    /// ignore la casse : `funcLuaCmd_…` était exclu du compte des fonctions
    /// nommées, soit 6 742 fonctions manquantes sur `nie.exe`.
    #[test]
    fn le_filtre_des_noms_ghidra_epargne_funclua() {
        let mut db = Db::open_in_memory().unwrap();
        let bin = db
            .upsert_binary("t.exe", "sha", "x86_64", 64, 0x1_4000_0000, 0, None, None)
            .unwrap();
        {
            let tx = db.conn_mut().transaction().unwrap();
            for (va, name) in [
                (0x1000i64, "FUN_140001000"),
                (0x2000, "funcLuaCmd_214da123"),
                (0x3000, "fn_UpdateCursorPos"),
                (0x4000, "function_utile"),
            ] {
                ingest::function(&tx, bin, va, Some(name), Some("t"), "menu", "", 0.5).unwrap();
            }
            tx.commit().unwrap();
        }
        let cov = query::coverage(db.conn(), bin).unwrap();
        assert_eq!(cov.total, 4);
        assert_eq!(cov.named, 3, "seul le nom Ghidra `FUN_<hex>` est écarté");
    }

    #[test]
    fn schema_applies_and_coverage_computes() {
        let mut db = Db::open_in_memory().unwrap();
        assert_eq!(
            db.get_meta("schema_version").unwrap().as_deref(),
            Some(SCHEMA_VERSION)
        );
        // La migration caméra est appliquée par `init` : ses tables et vues existent.
        let cam_tables: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
                   AND substr(name, 1, 4) = 'cam_'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cam_tables, 22, "migration caméra appliquée");
        // Les colonnes ajoutées après coup sont présentes, et `init` est rejouable.
        db.init().unwrap();
        for (table, column, _) in ADDED_COLUMNS {
            let n: i64 = db
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
                    (table, column),
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "colonne {table}.{column} absente");
        }
        let bin = db
            .upsert_binary(
                "nie.exe",
                "abc123",
                "x86_64",
                64,
                0x1_4000_0000,
                31_468_032,
                None,
                None,
            )
            .unwrap();
        let tx = db.conn_mut().transaction().unwrap();
        // une fonction non classifiée (standalone) + une classifiée
        ingest::function(
            &tx,
            bin,
            0x1_4000_1000,
            None,
            Some("re"),
            "standalone",
            "leaf",
            0.0,
        )
        .unwrap();
        let fid = ingest::function(
            &tx,
            bin,
            0x1_4002_4b80,
            None,
            Some("re"),
            "game",
            "hub",
            0.5,
        )
        .unwrap();
        ingest::str_ref(&tx, bin, fid, "CHARA_PARAM_INFO").unwrap();
        tx.commit().unwrap();
        let cov = db.snapshot_coverage(bin).unwrap();
        assert_eq!(cov.total, 2);
        assert_eq!(cov.classified, 1);
        assert!((cov.pct - 50.0).abs() < 1e-6);
    }

    #[test]
    fn redis_meta_fallback_ignored_if_no_redis() {
        let db = Db::open_in_memory().unwrap();
        db.set_meta("test_key", "test_val").unwrap();
        assert_eq!(
            db.get_meta("test_key").unwrap().as_deref(),
            Some("test_val")
        );
    }

    #[test]
    fn test_redis_connection_if_available() {
        // Enregistre en base SQLite en mémoire et tente Redis local
        if let Ok(db) = Db::open_with_redis(":memory:", "redis://127.0.0.1/") {
            db.set_meta("redis_test_key", "redis_val").unwrap();
            assert_eq!(
                db.get_meta("redis_test_key").unwrap().as_deref(),
                Some("redis_val")
            );
        }
    }
}
