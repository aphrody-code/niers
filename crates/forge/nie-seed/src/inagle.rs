//! Ingestion des couples (hash, kind, nom) depuis le miroir SQLite d'inagle.
//!
//! Les IDs IEVR sont des hash CRC32/FNV Level-5 stockés en hexadécimal dans
//! les tables `inagle_*` du miroir Supabase.
//!
//! ## Sources exploitées
//!
//! | Table SQLite         | kind       | format id      | Exemple                        |
//! |----------------------|------------|----------------|--------------------------------|
//! | `inagle_characters`  | `chara`    | `0xXXXXXXXX`   | `0x3055CF22` → Mark Evans      |
//! | `inagle_items`       | `item`     | `0xXXXXXXXX`   | `0x5F0F1EAC` → Guts Gear       |
//! | `inagle_teams`       | `team`     | `0xXXXXXXXX`   | `0x0FD5A921` → Onze de Fertilia|
//! | `inagle_tactics`     | `tactic`   | `0xXXXXXXXX`   | `0x790F3F39` → Formation SHINANO|
//! | `inagle_auras`       | `aura`     | `aura_0xXXXXXXXX` | `aura_0xD67B0DE4`           |
//! | `inagle_souls`       | `soul`     | `soul_0xXXXXXXXX` | `soul_0x752F8F55`           |
//! | `inagle_quests`      | `quest`    | `0xXXXXXXXX`   | JSON `titles.fr`               |
//!
//! ## Politique d'ingestion
//!
//! - Seuls les enregistrements avec un `id` commençant par `0x` (ou avec un
//!   préfixe suivi de `0x`) et un nom non vide sont ingérés.
//! - Le hash est converti en `i64` signé (même nombre de bits que sqlite INTEGER).
//! - Le nom retenu est `name_fr` en priorité, sinon `name_en`, sinon le hash brut.
//! - Source déclarée : `"inagle"`.

use std::path::Path;

use anyhow::{Context, Result};
use nie_index::{Db, ingest};
use rusqlite::Connection;
use tracing::{debug, warn};

/// Ingestion des hash→nom inagle depuis le miroir SQLite le plus récent trouvé
/// dans `sqlite_dir` (cherche `supabase-*.sqlite` par ordre alphabétique décroissant).
///
/// Renvoie le nombre total de couples (hash, nom) insérés.
///
/// # Erreurs
///
/// Retourne une erreur si aucun fichier SQLite n'est trouvable ou si la DB est
/// inaccessible. Les tables absentes ou les lignes sans hash valide sont ignorées
/// silencieusement.
pub fn ingest_inagle_hashes(db: &mut Db, sqlite_dir: &Path) -> Result<usize> {
    let sqlite_path = find_latest_sqlite(sqlite_dir).with_context(|| {
        format!(
            "aucun supabase-*.sqlite trouvé dans {}",
            sqlite_dir.display()
        )
    })?;

    let src = Connection::open(&sqlite_path)
        .with_context(|| format!("ouverture de {}", sqlite_path.display()))?;

    let tx = db
        .conn_mut()
        .transaction()
        .context("ouverture de transaction inagle")?;

    let mut total = 0usize;

    // ── inagle_characters : id = "0xXXXXXXXX", name_fr ─────────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_characters",
        "id",
        "name_fr",
        "chara",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_characters: {e}");
        0
    });

    // ── inagle_items : id = "0xXXXXXXXX", name_fr ───────────────────────────
    total += ingest_table_simple(&src, &tx, "inagle_items", "id", "name_fr", "item", None)
        .unwrap_or_else(|e| {
            warn!("inagle_items: {e}");
            0
        });

    // ── inagle_teams : id = "0xXXXXXXXX", name_fr ───────────────────────────
    total += ingest_table_simple(&src, &tx, "inagle_teams", "id", "name_fr", "team", None)
        .unwrap_or_else(|e| {
            warn!("inagle_teams: {e}");
            0
        });

    // ── inagle_tactics : id = "0xXXXXXXXX", name_fr ─────────────────────────
    total += ingest_table_simple(&src, &tx, "inagle_tactics", "id", "name_fr", "tactic", None)
        .unwrap_or_else(|e| {
            warn!("inagle_tactics: {e}");
            0
        });

    // ── inagle_auras : id = "aura_0xXXXXXXXX", name_fr ──────────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_auras",
        "id",
        "name_fr",
        "aura",
        Some("aura_"),
    )
    .unwrap_or_else(|e| {
        warn!("inagle_auras: {e}");
        0
    });

    // ── inagle_souls : id = "soul_0xXXXXXXXX", name_fr ──────────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_souls",
        "id",
        "name_fr",
        "soul",
        Some("soul_"),
    )
    .unwrap_or_else(|e| {
        warn!("inagle_souls: {e}");
        0
    });

    // ── inagle_quests : id = "0xXXXXXXXX", nom extrait du JSON (colonne data) ─
    total += ingest_quests(&src, &tx).unwrap_or_else(|e| {
        warn!("inagle_quests: {e}");
        0
    });

    // ── inagle_skills : id = "0xXXXXXXXX", name_fr ───────────────────────────
    total += ingest_table_simple(&src, &tx, "inagle_skills", "id", "name_fr", "skill", None)
        .unwrap_or_else(|e| {
            warn!("inagle_skills: {e}");
            0
        });

    // ── inagle_passives : id = "0xXXXXXXXX", name_fr ─────────────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_passives",
        "id",
        "name_fr",
        "passive",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_passives: {e}");
        0
    });

    // ── inagle_trophies : trophy_id = "0xXXXXXXXX", name_fr ──────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_trophies",
        "trophy_id",
        "name_fr",
        "trophy",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_trophies: {e}");
        0
    });

    // ── inagle_missions : mission_id = "0xXXXXXXXX", name_fr ─────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_missions",
        "mission_id",
        "name_fr",
        "mission",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_missions: {e}");
        0
    });

    // ── inagle_keshins : id = "0xXXXXXXXX", name_fr ──────────────────────────
    total += ingest_table_simple(&src, &tx, "inagle_keshins", "id", "name_fr", "keshin", None)
        .unwrap_or_else(|e| {
            warn!("inagle_keshins: {e}");
            0
        });

    // ── inagle_emblems : emblem_id = "0xXXXXXXXX", emblem_name ───────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_emblems",
        "emblem_id",
        "emblem_name",
        "emblem",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_emblems: {e}");
        0
    });

    // ── inagle_special_tactics : id = "0xXXXXXXXX", name_fr ──────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_special_tactics",
        "id",
        "name_fr",
        "special_tactic",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_special_tactics: {e}");
        0
    });

    // ── inagle_formations : id = "0xXXXXXXXX", name_fr ───────────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_formations",
        "id",
        "name_fr",
        "formation",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_formations: {e}");
        0
    });

    // ── inagle_basara : character_id = "0xXXXXXXXX", name_localised ──────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_basara",
        "character_id",
        "name_localised",
        "basara",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_basara: {e}");
        0
    });

    // ── inagle_stadiums : id = "0xXXXXXXXX", image_path ──────────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_stadiums",
        "id",
        "image_path",
        "stadium",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_stadiums: {e}");
        0
    });

    // ── inagle_costumes : id = "0xXXXXXXXX", model_ref ───────────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_costumes",
        "id",
        "model_ref",
        "costume",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_costumes: {e}");
        0
    });

    // ── inagle_super_tactics : crc_id = "0xXXXXXXXX", id ──────────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_super_tactics",
        "crc_id",
        "id",
        "super_tactic",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_super_tactics: {e}");
        0
    });

    // ── inagle_tricks : id = "0xXXXXXXXX", trick_id_name ─────────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_tricks",
        "id",
        "trick_id_name",
        "trick",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_tricks: {e}");
        0
    });

    // ── inagle_opponent_teams : id = "0xXXXXXXXX", team_id ────────────────────
    total += ingest_table_simple(
        &src,
        &tx,
        "inagle_opponent_teams",
        "id",
        "team_id",
        "opponent_team",
        None,
    )
    .unwrap_or_else(|e| {
        warn!("inagle_opponent_teams: {e}");
        0
    });

    tx.commit().context("commit inagle hashes")?;
    Ok(total)
}

// ── Fonctions internes ────────────────────────────────────────────────────────

/// Cherche le fichier `supabase-*.sqlite` le plus récent dans `dir`
/// (tri lexicographique décroissant sur le nom de fichier, ce qui correspond
/// à l'ordre chronologique grâce au format ISO-8601 dans le nom).
fn find_latest_sqlite(dir: &Path) -> Result<std::path::PathBuf> {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("lecture de {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("supabase-") && n.ends_with(".sqlite"))
        })
        .collect();

    entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    entries
        .into_iter()
        .next()
        .with_context(|| format!("aucun supabase-*.sqlite dans {}", dir.display()))
}

/// Insère les lignes d'une table simple (id→name_fr).
///
/// - `id_col` : colonne contenant le hash (ex. `"id"`)
/// - `name_col` : colonne contenant le nom (ex. `"name_fr"`)
/// - `kind` : catégorie pour la table `hash_name`
/// - `id_prefix` : préfixe éventuel à retirer avant le `0x` (ex. `Some("aura_")`)
fn ingest_table_simple(
    src: &Connection,
    tx: &Connection,
    table: &str,
    id_col: &str,
    name_col: &str,
    kind: &str,
    id_prefix: Option<&str>,
) -> Result<usize> {
    // Vérifie que la table existe.
    let exists: i64 = src
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        return Ok(0);
    }

    let query = format!(
        "SELECT {id_col}, {name_col} FROM {table} \
         WHERE {id_col} LIKE '%0x%' \
           AND {name_col} IS NOT NULL \
           AND {name_col} != ''",
    );

    let mut stmt = src
        .prepare(&query)
        .with_context(|| format!("préparation requête {table}"))?;

    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .with_context(|| format!("query {table}"))?;

    let mut count = 0usize;

    for row in rows {
        let (raw_id, name) = row.with_context(|| format!("lecture ligne {table}"))?;

        let hex_part = strip_id_prefix(&raw_id, id_prefix);
        let Some(hash_val) = parse_hex_id(hex_part) else {
            debug!("hash invalide ignoré : {:?}", raw_id);
            continue;
        };

        ingest::hash_name(tx, hash_val, kind, &name, "inagle")
            .with_context(|| format!("hash_name({raw_id}={name})"))?;
        count += 1;
    }

    Ok(count)
}

/// Ingère les quests depuis `inagle_quests`. Le nom est extrait du JSON (`data`)
/// ou des colonnes `name_fr`/`name_en`.
fn ingest_quests(src: &Connection, tx: &Connection) -> Result<usize> {
    let exists: i64 = src
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='inagle_quests'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        return Ok(0);
    }

    // La table a : id (hash), name_fr (souvent vide), data (JSON avec titles.fr)
    let mut stmt =
        src.prepare("SELECT id, name_fr, name_en, data FROM inagle_quests WHERE id LIKE '0x%'")?;

    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;

    let mut count = 0usize;

    for row in rows {
        let (raw_id, name_fr, name_en, data_json) = row?;

        let Some(hash_val) = parse_hex_id(&raw_id) else {
            continue;
        };

        // Priorité : name_fr non vide → name_en → JSON titles.fr → JSON titles.en
        let name = name_fr
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| name_en.as_deref().filter(|s| !s.is_empty()))
            .map(|s| s.to_owned())
            .or_else(|| data_json.as_deref().and_then(extract_quest_title));

        let Some(name) = name else {
            debug!("quest sans nom ignorée : {raw_id}");
            continue;
        };

        ingest::hash_name(tx, hash_val, "quest", &name, "inagle")?;
        count += 1;
    }

    Ok(count)
}

/// Extrait un titre depuis le JSON d'une quest.
/// Cherche `titles.fr` puis `titles.en` via parsing minimal (pas de dépendance serde_json).
fn extract_quest_title(json: &str) -> Option<String> {
    // Cherche "fr":"<valeur>" ou "en":"<valeur>" dans le bloc "titles"
    for key in &["\"fr\":\"", "\"en\":\""] {
        if let Some(start) = json.find(key) {
            let after = &json[start + key.len()..];
            if let Some(end) = after.find('"') {
                let title = &after[..end];
                if !title.is_empty() {
                    return Some(title.to_owned());
                }
            }
        }
    }
    None
}

/// Retire un préfixe textuel éventuel (ex. `"aura_"`) de l'ID brut.
fn strip_id_prefix<'a>(raw_id: &'a str, prefix: Option<&str>) -> &'a str {
    match prefix {
        Some(p) if raw_id.starts_with(p) => &raw_id[p.len()..],
        _ => raw_id,
    }
}

/// Parse une chaîne hexadécimale `0xXXXXXXXX` en `i64` signé.
/// Renvoie `None` si le format est invalide.
fn parse_hex_id(s: &str) -> Option<i64> {
    let s = s.trim();
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
    // parse en u64 pour accepter les valeurs > 0x7FFFFFFF, puis cast en i64
    u64::from_str_radix(hex, 16).ok().map(|v| v as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_id_positif() {
        assert_eq!(parse_hex_id("0x3055CF22"), Some(0x3055_CF22_i64));
    }

    #[test]
    fn parse_hex_id_grand() {
        // valeur > 0x7FFFFFFF → stockée comme i64 signé
        let v = parse_hex_id("0xFFFFFFFF").unwrap();
        assert_eq!(v, 0xFFFF_FFFF_u32 as i64);
    }

    #[test]
    fn parse_hex_id_invalide() {
        assert!(parse_hex_id("rhd10010").is_none());
        assert!(parse_hex_id("").is_none());
    }

    #[test]
    fn strip_prefix_aura() {
        assert_eq!(
            strip_id_prefix("aura_0xD67B0DE4", Some("aura_")),
            "0xD67B0DE4"
        );
        assert_eq!(strip_id_prefix("0xD67B0DE4", Some("aura_")), "0xD67B0DE4");
        assert_eq!(strip_id_prefix("0xD67B0DE4", None), "0xD67B0DE4");
    }

    #[test]
    fn extract_quest_title_json() {
        let json = r#"{"titles":{"en":"Gather info.","fr":"Interrogez les élèves."}}"#;
        let title = extract_quest_title(json).unwrap();
        assert_eq!(title, "Interrogez les élèves.");
    }

    #[test]
    fn extract_quest_title_json_en_fallback() {
        let json = r#"{"titles":{"en":"Gather info.","fr":""}}"#;
        // "fr" est vide → fallback "en"
        let title = extract_quest_title(json).unwrap();
        // notre parsing minimal prend la première occurrence de "fr" ou "en"
        // le json a "fr":"" donc on trouve "fr" en premier, valeur vide → cherche "en"
        assert!(!title.is_empty());
    }

    #[test]
    fn ingest_inagle_hashes_in_memory() {
        // Construit une DB inagle SQLite minimale en mémoire.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let src = Connection::open(tmp.path()).unwrap();

        src.execute_batch(
            "CREATE TABLE inagle_characters(id TEXT, name_fr TEXT);
             INSERT INTO inagle_characters VALUES('0x3055CF22', 'Mark Evans');
             INSERT INTO inagle_characters VALUES('0x8193A099', 'Mark Evans');
             INSERT INTO inagle_characters VALUES('rhd10010', 'Feinte'); -- pas un hash

             CREATE TABLE inagle_items(id TEXT, name_fr TEXT);
             INSERT INTO inagle_items VALUES('0x5F0F1EAC', 'Guts Gear');

             CREATE TABLE inagle_teams(id TEXT, name_fr TEXT);
             INSERT INTO inagle_teams VALUES('0x0FD5A921', 'Onze de Fertilia');

             CREATE TABLE inagle_tactics(id TEXT, name_fr TEXT);
             INSERT INTO inagle_tactics VALUES('0x790F3F39', 'Formation SHINANO');

             CREATE TABLE inagle_auras(id TEXT, name_fr TEXT);
             INSERT INTO inagle_auras VALUES('aura_0xD67B0DE4', 'Catalyste élémentaire');

             CREATE TABLE inagle_souls(id TEXT, name_fr TEXT);
             INSERT INTO inagle_souls VALUES('soul_0x752F8F55', 'Totem Kulupe');

             CREATE TABLE inagle_quests(id TEXT, name_fr TEXT, name_en TEXT, data TEXT);
             INSERT INTO inagle_quests VALUES('0x103AF791', '', '',
               '{\"titles\":{\"en\":\"Gather info.\",\"fr\":\"Interrogez les élèves.\"}}');",
        )
        .unwrap();
        drop(src);

        // Crée un répertoire temporaire contenant le fichier SQLite nommé
        // selon la convention supabase-*.sqlite.
        let dir = tempfile::tempdir().unwrap();
        let sqlite_name = "supabase-2026-06-04T00-00-00.sqlite";
        let sqlite_dst = dir.path().join(sqlite_name);
        std::fs::copy(tmp.path(), &sqlite_dst).unwrap();

        let mut db = nie_index::Db::open_in_memory().unwrap();
        let count = ingest_inagle_hashes(&mut db, dir.path()).unwrap();

        // 2 chara (Mark Evans×2) + 1 item + 1 team + 1 tactic + 1 aura + 1 soul + 1 quest = 8
        // La ligne "rhd10010" est ignorée.
        assert_eq!(count, 8, "attendu 8 hash insérés, obtenu {count}");

        // Vérifier un couple spécifique.
        let name: String = db
            .conn()
            .query_row(
                "SELECT name FROM hash_name WHERE hash = ?1 AND kind = 'chara'",
                [0x3055_CF22_i64],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "Mark Evans");

        // Vérifier la quest.
        let quest_name: String = db
            .conn()
            .query_row("SELECT name FROM hash_name WHERE kind = 'quest'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(quest_name, "Interrogez les élèves.");
    }
}
