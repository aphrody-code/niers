//! Requêtes sur les tables `inagle_*` du miroir SQLite.
//!
//! Reproduit fidèlement les comportements du TS :
//! - fusion `data` + `sheet_data`
//! - résolution element/category depuis les colonnes texte FR
//! - IDs non-strippés (sanitizeFilter ne retire pas `_`)
//! - vues `*_clean` émulées par `GROUP BY name_fr`
//! - lookup d'auras dans keshins/souls/miximax avec prefixe et hex normalisé

use rusqlite::Connection;
use serde_json::Value;

use crate::{
    mirror::{merge_data_sheet, query_one, query_rows},
    model::{
        AuditReport, AuditTable, AuraSummary, CharaCompareSlot, CharaProfile, CharaSummary,
        CompareResult, CompareSkillSlot, DialogueMatch, ItemProfile, RandomTeam, RandomTeamCoord,
        RandomTeamPlayer, RedisStatus, SearchResult, SkillProfile, SkillSlot, SqliteStatus,
        StatBlock, StatusReport, TeamBuildEntry, TeamProfile,
    },
};

// ─── Sanitize ────────────────────────────────────────────────────────────────

/// Sanitize identique à wiki-service.ts `sanitizeFilter` :
/// retire `%`, `,`, `(`, `)`, `.`, `*`, `\` — mais PAS `_` (IDs d'auras).
pub fn sanitize_filter(input: &str) -> String {
    input
        .chars()
        .filter(|&c| !matches!(c, '%' | ',' | '(' | ')' | '.' | '*' | '\\'))
        .collect()
}

// ─── Personnage ───────────────────────────────────────────────────────────────

/// Recherche des personnages par ID ou nom (FR/EN/JA).
///
/// Retourne tous les matches (plusieurs variantes possible pour un même charaId).
pub fn search_characters(conn: &Connection, query: &str) -> anyhow::Result<Vec<CharaSummary>> {
    let q = sanitize_filter(query);
    let like_pat = format!("%{}%", q);

    query_rows(
        conn,
        "SELECT id, chara_id, name_fr, name_en, name_ja, element, position,
                rarity_label, internal_code, slug, base_slug
         FROM inagle_characters
         WHERE id = ?1
            OR chara_id = ?1
            OR internal_code = ?1
            OR slug = ?1
            OR base_slug = ?1
            OR name_fr LIKE ?2
            OR name_en LIKE ?2
            OR name_ja LIKE ?2
         ORDER BY zukan_order ASC NULLS LAST, id ASC
         LIMIT 50",
        &[&q as &dyn rusqlite::ToSql, &like_pat],
        |row| {
            Ok(CharaSummary {
                id: row.get(0)?,
                chara_id: row.get(1)?,
                name_fr: row.get(2)?,
                name_en: row.get(3)?,
                name_ja: row.get(4)?,
                element: row.get(5)?,
                position: row.get(6)?,
                rarity_label: row.get(7)?,
                internal_code: row.get(8)?,
                slug: row.get(9)?,
                base_slug: row.get(10)?,
            })
        },
    )
}

/// Charge le profil complet d'un personnage par son `id` (charaParamId).
pub fn get_character(conn: &Connection, id: &str) -> anyhow::Result<Option<CharaProfile>> {
    let row = query_one(
        conn,
        "SELECT id, chara_id, name_fr, name_en, name_ja, element, position,
                rarity_label, internal_code, slug, base_slug,
                description_fr, description_en, gender, team_id, series, zukan_hash,
                data, sheet_data, skills
         FROM inagle_characters WHERE id = ?1",
        &[&id as &dyn rusqlite::ToSql],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
                row.get::<_, Option<String>>(16)?,
                row.get::<_, Option<String>>(17)?,
                row.get::<_, Option<String>>(18)?,
                row.get::<_, Option<String>>(19)?,
            ))
        },
    )?;

    let Some((
        db_id,
        chara_id,
        name_fr,
        name_en,
        name_ja,
        element,
        position,
        rarity_label,
        internal_code,
        slug,
        base_slug,
        description_fr,
        description_en,
        gender,
        team_id,
        series,
        zukan_hash,
        data,
        sheet_data,
        _skills_col,
    )) = row
    else {
        return Ok(None);
    };

    let merged = merge_data_sheet(data.as_deref(), sheet_data.as_deref());

    // Résoudre le nom de l'équipe depuis data.teams[0].names.fr
    let team_name = merged
        .get("teams")
        .and_then(|t| t.as_array())
        .and_then(|arr| arr.first())
        .and_then(|t| {
            t.get("names")
                .and_then(|n| n.get("fr").and_then(Value::as_str))
                .or_else(|| t.get("name_FR").and_then(Value::as_str))
                .or_else(|| t.get("name_EN").and_then(Value::as_str))
                .or_else(|| t.get("name").and_then(Value::as_str))
        })
        .map(str::to_string)
        .or_else(|| team_id.clone());

    // Stats depuis data.stats
    let (stats_lv1, stats_lv50, stats_lv99) = extract_stats_from_merged(&merged);

    // Skills depuis data.skills
    let skills = extract_skills_from_merged(&merged);

    // Auras du personnage (keshin/soul/miximax)
    let auras = get_auras_for_character(conn, &chara_id, &db_id, &merged)?;

    Ok(Some(CharaProfile {
        id: db_id,
        chara_id,
        name_fr,
        name_en,
        name_ja,
        element,
        position,
        rarity_label,
        internal_code,
        slug,
        base_slug,
        description_fr,
        description_en,
        gender,
        team_name,
        series,
        zukan_hash,
        stats_lv1,
        stats_lv50,
        stats_lv99,
        skills,
        auras,
        merged,
    }))
}

/// Extrait les blocs de stats lv1/lv50/lv99 depuis le JSON fusionné.
fn extract_stats_from_merged(
    merged: &Value,
) -> (Option<StatBlock>, Option<StatBlock>, Option<StatBlock>) {
    let stats_obj = merged.get("stats");
    let lv1 = stats_obj
        .and_then(|s| s.get("lv1"))
        .and_then(StatBlock::from_json);
    let lv50 = stats_obj
        .and_then(|s| s.get("lv50"))
        .and_then(StatBlock::from_json);
    let lv99 = stats_obj
        .and_then(|s| s.get("lv99"))
        .and_then(StatBlock::from_json);
    (lv1, lv50, lv99)
}

/// Extrait les slots de skills depuis le JSON fusionné (data.skills).
fn extract_skills_from_merged(merged: &Value) -> Vec<SkillSlot> {
    merged
        .get("skills")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let skill_id = entry.get("skillId")?.as_str()?.to_string();
                    let learn_level = entry
                        .get("learnLevel")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    Some(SkillSlot {
                        skill_id,
                        learn_level,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Calcule les stats interpolées à un niveau donné.
///
/// Identique à `getInterpolatedStats` dans azalee.ts, qui implémente
/// la même logique segmentée que nie-core::stats::calculate_single_stat.
pub fn interpolate_stats(lv1: StatBlock, lv50: StatBlock, lv99: StatBlock, level: u8) -> StatBlock {
    // La DB n'expose pas de lv30 séparé dans stats.lv30 (pas présent pour tous les persos).
    // On utilise la courbe simplifiée à 2 segments : lv1→lv50→lv99.
    // Si on avait lv30 on ferait 3 segments, mais le TS lui-même fallback sur 2 segments
    // quand hasCompleteData = false.
    if level == 0 || level == 1 {
        return lv1;
    }
    if level >= 99 {
        return lv99;
    }
    // Segment 1 : lv1 → lv50
    if level <= 50 {
        let t = f64::from(level - 1) / 49.0;
        return lv1.lerp(lv50, t);
    }
    // Segment 2 : lv50 → lv99
    let t = f64::from(level - 50) / 49.0;
    lv50.lerp(lv99, t)
}

/// Recherche les auras (keshins/souls/miximax) associées à un personnage.
///
/// Reproduit fidèlement `getAurasForChara` de azalee.ts :
/// - lit `data.auras[].skillId` (hex)
/// - pour chaque hex : cherche dans keshin_/soul_/miximax_ avec prefixe normalisé
fn get_auras_for_character(
    conn: &Connection,
    _chara_id: &str,
    _param_id: &str,
    merged: &Value,
) -> anyhow::Result<Vec<AuraSummary>> {
    // Collecte les hex IDs d'aura depuis data.auras
    let mut hex_ids: Vec<String> = merged
        .get("auras")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    entry
                        .get("skillId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_lowercase())
                })
                .collect()
        })
        .unwrap_or_default();

    // Dédoublonnage
    hex_ids.sort();
    hex_ids.dedup();

    let tables = [
        ("inagle_keshins", "keshin_", "keshin"),
        ("inagle_souls", "soul_", "soul"),
        ("inagle_miximax", "miximax_", "miximax"),
        ("inagle_auras", "aura_", "aura"),
    ];

    let mut results = Vec::new();

    for hex_id in &hex_ids {
        let clean_hex = hex_id.trim_start_matches("0x").to_uppercase();
        let formatted = format!("0x{clean_hex}");

        let mut found = false;
        for (table, prefix, aura_type) in &tables {
            if found {
                break;
            }
            let prefixed_id = format!("{}{}", prefix, formatted);
            // Essaie prefixed_id, formatted_id, et hex_id brut (identique au TS)
            let possible: [&str; 3] = [prefixed_id.as_str(), formatted.as_str(), hex_id.as_str()];

            for candidate in &possible {
                let maybe = query_one(
                    conn,
                    &format!(
                        "SELECT id, name_fr, name_en, name_ja FROM {} WHERE id = ?1 LIMIT 1",
                        table
                    ),
                    &[candidate as &dyn rusqlite::ToSql],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    },
                )?;

                if let Some((aid, a_name_fr, a_name_en, a_name_ja)) = maybe {
                    let name = a_name_fr
                        .or(a_name_en)
                        .or(a_name_ja)
                        .unwrap_or_else(|| "Inconnu".to_string());
                    results.push(AuraSummary {
                        id: aid,
                        name,
                        aura_type: aura_type.to_string(),
                    });
                    found = true;
                    break;
                }
            }
        }
    }

    Ok(results)
}

// ─── Skill ───────────────────────────────────────────────────────────────────

/// Recherche des skills par ID ou nom.
pub fn search_skills(conn: &Connection, query: &str) -> anyhow::Result<Vec<SkillProfile>> {
    let q = sanitize_filter(query);
    // Quand le CLI demande seulement des filtres (catégorie/élément), une
    // recherche vide ne doit pas tronquer le corpus aux 20 premières lignes :
    // les filtres sont appliqués après cette fonction.
    if q.is_empty() {
        return query_rows(
            conn,
            "SELECT id, name_fr, name_en, name_ja,
                    category, element,
                    power_max, power_min, tp_cost,
                    description_fr, description_en,
                    internal_code, is_hyper,
                    data, sheet_data
             FROM inagle_skills
             ORDER BY name_fr ASC",
            &[],
            skill_row_map,
        );
    }
    let like_pat = format!("%{}%", q);

    query_rows(
        conn,
        "SELECT id, name_fr, name_en, name_ja,
                category, element,
                power_max, power_min, tp_cost,
                description_fr, description_en,
                internal_code, is_hyper,
                data, sheet_data
         FROM inagle_skills
         WHERE id = ?1
            OR internal_code = ?1
            OR name_fr LIKE ?2
            OR name_en LIKE ?2
            OR name_ja LIKE ?2
         ORDER BY name_fr ASC
         LIMIT 20",
        &[&q as &dyn rusqlite::ToSql, &like_pat],
        skill_row_map,
    )
}

/// Charge un skill par son ID exact.
pub fn get_skill(conn: &Connection, id: &str) -> anyhow::Result<Option<SkillProfile>> {
    let q = sanitize_filter(id);
    query_one(
        conn,
        "SELECT id, name_fr, name_en, name_ja,
                category, element,
                power_max, power_min, tp_cost,
                description_fr, description_en,
                internal_code, is_hyper,
                data, sheet_data
         FROM inagle_skills
         WHERE id = ?1 OR internal_code = ?1",
        &[&q as &dyn rusqlite::ToSql],
        skill_row_map,
    )
}

fn skill_row_map(row: &rusqlite::Row<'_>) -> rusqlite::Result<SkillProfile> {
    let id: String = row.get(0)?;
    let name_fr: Option<String> = row.get(1)?;
    let name_en: Option<String> = row.get(2)?;
    let name_ja: Option<String> = row.get(3)?;
    // category et element : colonnes texte FR (category_id/element_id sont NULL)
    let category_fr: Option<String> = row.get(4)?;
    let element_fr: Option<String> = row.get(5)?;
    let power_max: Option<i64> = row.get(6)?;
    let power_min: Option<i64> = row.get(7)?;
    let tp_cost: Option<i64> = row.get(8)?;
    let description_fr: Option<String> = row.get(9)?;
    let description_en: Option<String> = row.get(10)?;
    let internal_code: Option<String> = row.get(11)?;
    let is_hyper: Option<i64> = row.get(12)?;
    let data: Option<String> = row.get(13)?;
    let sheet_data: Option<String> = row.get(14)?;

    let merged = merge_data_sheet(data.as_deref(), sheet_data.as_deref());

    // Résolution element et category : priorité colonne DB → merged JSON
    let element = element_fr
        .as_deref()
        .or_else(|| {
            merged.get("element").and_then(|v| v.as_str())
        })
        .map(crate::model::element_fr_to_en);

    let category = category_fr
        .as_deref()
        .or_else(|| {
            merged
                .get("categoryName")
                .and_then(|cn| cn.get("fr").and_then(|v| v.as_str()))
        })
        .or_else(|| {
            merged.get("category").and_then(|v| v.as_str())
        })
        .map(crate::model::category_fr_to_en);

    // description fallbacks depuis merged
    let desc_fr = description_fr.or_else(|| {
        merged
            .get("desc_FR")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    });
    let desc_en = description_en.or_else(|| {
        merged
            .get("desc_EN")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    });

    Ok(SkillProfile {
        id,
        name_fr,
        name_en,
        name_ja,
        category,
        element,
        power_max,
        power_min,
        tp_cost,
        description_fr: desc_fr,
        description_en: desc_en,
        internal_code,
        is_hyper: is_hyper.unwrap_or(0) != 0,
        merged,
    })
}

// ─── Item ────────────────────────────────────────────────────────────────────

/// Recherche des items par ID ou nom.
pub fn search_items(conn: &Connection, query: &str) -> anyhow::Result<Vec<ItemProfile>> {
    let q = sanitize_filter(query);
    let like_pat = format!("%{}%", q);

    query_rows(
        conn,
        "SELECT id, name_fr, name_en, name_ja,
                category, rarity, description_fr,
                internal_code, price, shops,
                data, sheet_data
         FROM inagle_items
         WHERE id = ?1
            OR internal_code = ?1
            OR name_fr LIKE ?2
            OR name_en LIKE ?2
            OR name_ja LIKE ?2
         ORDER BY name_fr ASC
         LIMIT 20",
        &[&q as &dyn rusqlite::ToSql, &like_pat],
        item_row_map,
    )
}

/// Charge un item par son ID exact.
pub fn get_item(conn: &Connection, id: &str) -> anyhow::Result<Option<ItemProfile>> {
    let q = sanitize_filter(id);
    query_one(
        conn,
        "SELECT id, name_fr, name_en, name_ja,
                category, rarity, description_fr,
                internal_code, price, shops,
                data, sheet_data
         FROM inagle_items WHERE id = ?1 OR internal_code = ?1",
        &[&q as &dyn rusqlite::ToSql],
        item_row_map,
    )
}

fn item_row_map(row: &rusqlite::Row<'_>) -> rusqlite::Result<ItemProfile> {
    let id: String = row.get(0)?;
    let name_fr: Option<String> = row.get(1)?;
    let name_en: Option<String> = row.get(2)?;
    let name_ja: Option<String> = row.get(3)?;
    let category: Option<String> = row.get(4)?;
    let rarity: Option<i64> = row.get(5)?;
    let description_fr: Option<String> = row.get(6)?;
    let internal_code: Option<String> = row.get(7)?;
    let price_col: Option<i64> = row.get(8)?;
    let shops_col: Option<String> = row.get(9)?;
    let data: Option<String> = row.get(10)?;
    let sheet_data: Option<String> = row.get(11)?;

    let merged = merge_data_sheet(data.as_deref(), sheet_data.as_deref());

    // Prix : depuis sheet_data.price (la colonne price contient des sort IDs, pas de vrais prix)
    let price = merged
        .get("price")
        .and_then(|v| v.as_i64())
        .or(price_col.filter(|_| false)); // on ignore la colonne price brute comme le TS

    // Shops : colonne shops (JSON array) ou sheet_data.shops
    let shops = parse_shops(shops_col.as_deref(), &merged);

    // description fallback depuis merged
    let desc_fr = description_fr.or_else(|| {
        merged
            .get("descriptions")
            .and_then(|d| d.get("fr").and_then(|v| v.as_str()))
            .map(str::to_string)
    });

    Ok(ItemProfile {
        id,
        name_fr,
        name_en,
        name_ja,
        category,
        rarity,
        description_fr: desc_fr,
        internal_code,
        price,
        shops,
        merged,
    })
}

fn parse_shops(shops_col: Option<&str>, merged: &Value) -> Vec<String> {
    // Tente la colonne shops (JSON) puis merged.shops.fr
    if let Some(s) = shops_col
        && let Ok(val) = serde_json::from_str::<Value>(s) {
            if let Some(arr) = val.as_array() {
                let names: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
                if !names.is_empty() {
                    return names;
                }
            }
            // shops peut être {"fr": [...], "en": [...]}
            if let Some(fr) = val.get("fr").and_then(|v| v.as_array()) {
                let names: Vec<String> = fr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
                if !names.is_empty() {
                    return names;
                }
            }
        }
    // Fallback merged.shops
    merged
        .get("shops")
        .and_then(|s| {
            s.get("fr")
                .and_then(|v| v.as_array())
                .or_else(|| s.as_array())
        })
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// ─── Team ────────────────────────────────────────────────────────────────────

/// Recherche des équipes par ID ou nom.
pub fn search_teams(conn: &Connection, query: &str) -> anyhow::Result<Vec<TeamProfile>> {
    let q = sanitize_filter(query);
    let like_pat = format!("%{}%", q);

    query_rows(
        conn,
        "SELECT id, name_fr, name_en, name_ja,
                internal_code, series, region,
                data, sheet_data
         FROM inagle_teams
         WHERE id = ?1
            OR internal_code = ?1
            OR name_fr LIKE ?2
            OR name_en LIKE ?2
            OR name_ja LIKE ?2
         ORDER BY name_fr ASC
         LIMIT 20",
        &[&q as &dyn rusqlite::ToSql, &like_pat],
        team_row_map,
    )
}

/// Charge une équipe par son ID exact.
pub fn get_team(conn: &Connection, id: &str) -> anyhow::Result<Option<TeamProfile>> {
    let q = sanitize_filter(id);
    query_one(
        conn,
        "SELECT id, name_fr, name_en, name_ja,
                internal_code, series, region,
                data, sheet_data
         FROM inagle_teams WHERE id = ?1 OR internal_code = ?1",
        &[&q as &dyn rusqlite::ToSql],
        team_row_map,
    )
}

fn team_row_map(row: &rusqlite::Row<'_>) -> rusqlite::Result<TeamProfile> {
    let id: String = row.get(0)?;
    let name_fr: Option<String> = row.get(1)?;
    let name_en: Option<String> = row.get(2)?;
    let name_ja: Option<String> = row.get(3)?;
    let internal_code: Option<String> = row.get(4)?;
    let series: Option<String> = row.get(5)?;
    let region: Option<String> = row.get(6)?;
    let data: Option<String> = row.get(7)?;
    let sheet_data: Option<String> = row.get(8)?;

    let merged = merge_data_sheet(data.as_deref(), sheet_data.as_deref());

    // Kits depuis data.kits (objet avec clés de version: ie, go, v)
    let kits = merged
        .get("kits")
        .and_then(|k| k.as_object())
        .map(|obj| {
            obj.values()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    // Seasons depuis data.seasons (objet {V: 7, IE1: 1, ...})
    let seasons = merged
        .get("seasons")
        .and_then(|s| s.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();

    Ok(TeamProfile {
        id,
        name_fr,
        name_en,
        name_ja,
        internal_code,
        series,
        region,
        kits,
        seasons,
        merged,
    })
}

// ─── Résolution nom de skill ──────────────────────────────────────────────────

/// Retourne le nom FR d'un skill depuis son ID (internal_code) ou son hex skillID.
///
/// Les personnages référencent leurs skills via un hex hash (ex: `0xF9B80F86`).
/// Ce hash est stocké dans `data->>'skillID'` de la table inagle_skills.
/// On essaie d'abord par `id` (internal_code), puis par le JSON data.
pub fn skill_name_by_id(conn: &Connection, skill_id: &str) -> String {
    let q = sanitize_filter(skill_id);
    // Essai 1 : correspondance directe sur id (internal_code comme rhd10010)
    let direct = query_one(
        conn,
        "SELECT name_fr, name_en FROM inagle_skills WHERE id = ?1 LIMIT 1",
        &[&q as &dyn rusqlite::ToSql],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        },
    )
    .ok()
    .flatten()
    .and_then(|(fr, en)| fr.or(en));

    if let Some(name) = direct {
        return name;
    }

    // Essai 2 : recherche dans data->>'skillID' (hex hash de l'inagle_skills)
    // Les IDs de skills dans les persos sont des hex 0xXXXXXXXX stockés en JSON.
    let like_pattern = format!("%\"skillID\":\"{}\",%", q);
    let via_data = query_one(
        conn,
        "SELECT name_fr, name_en FROM inagle_skills WHERE data LIKE ?1 LIMIT 1",
        &[&like_pattern as &dyn rusqlite::ToSql],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        },
    )
    .ok()
    .flatten()
    .and_then(|(fr, en)| fr.or(en));

    via_data.unwrap_or_else(|| skill_id.to_string())
}

// ─── Compare ─────────────────────────────────────────────────────────────────

/// Compare deux personnages par ID/nom et retourne leurs stats interpolées + skills.
///
/// Identique à `compare` TS : recherche les profils, interpole au niveau demandé,
/// résout les skills via `skill_name_by_id`.
pub fn compare_characters(
    conn: &Connection,
    query1: &str,
    query2: &str,
    level: u8,
) -> anyhow::Result<CompareResult> {
    let m1 = search_characters(conn, query1)?;
    let m2 = search_characters(conn, query2)?;

    anyhow::ensure!(!m1.is_empty(), "aucun personnage trouvé pour : \"{}\"", query1);
    anyhow::ensure!(!m2.is_empty(), "aucun personnage trouvé pour : \"{}\"", query2);

    let p1 = get_character(conn, &m1[0].id)?
        .ok_or_else(|| anyhow::anyhow!("profil introuvable pour {}", m1[0].id))?;
    let p2 = get_character(conn, &m2[0].id)?
        .ok_or_else(|| anyhow::anyhow!("profil introuvable pour {}", m2[0].id))?;

    let slot = |p: &CharaProfile| -> CharaCompareSlot {
        let stats = match (p.stats_lv1, p.stats_lv50, p.stats_lv99) {
            (Some(s1), Some(s50), Some(s99)) => interpolate_stats(s1, s50, s99, level),
            (Some(s1), _, Some(s99)) => {
                // 2 points → segment linéaire lv1→lv99
                let t = f64::from(level.saturating_sub(1)) / 98.0;
                s1.lerp(s99, t)
            }
            (_, _, Some(s99)) => s99,
            _ => StatBlock::default(),
        };

        let name = p
            .name_fr
            .clone()
            .or_else(|| p.name_en.clone())
            .or_else(|| p.name_ja.clone())
            .unwrap_or_else(|| p.id.clone());

        let skills = p
            .skills
            .iter()
            .map(|sk| {
                let n = skill_name_by_id(conn, &sk.skill_id);
                // Récupère power_max depuis la table skills
                let power = conn
                    .query_row(
                        "SELECT power_max FROM inagle_skills WHERE id = ?1 OR internal_code = ?1 LIMIT 1",
                        rusqlite::params![&sk.skill_id],
                        |row| row.get::<_, Option<i64>>(0),
                    )
                    .ok()
                    .flatten();
                CompareSkillSlot {
                    learn_level: sk.learn_level,
                    skill_id: sk.skill_id.clone(),
                    name: n,
                    power,
                }
            })
            .collect();

        CharaCompareSlot {
            id: p.id.clone(),
            chara_id: p.chara_id.clone(),
            name,
            position: p.position.clone(),
            element: p.element.clone(),
            rarity: p.rarity_label.clone(),
            stats,
            skills,
        }
    };

    Ok(CompareResult {
        level,
        chara1: slot(&p1),
        chara2: slot(&p2),
    })
}

// ─── Search multi-tables ──────────────────────────────────────────────────────

/// Recherche multi-tables (characters/skills/items/teams/auras/keshins/souls).
///
/// Identique à `search` TS : cherche dans chaque table et agrège les résultats.
pub fn search_all(conn: &Connection, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
    let q = sanitize_filter(query);
    let like_pat = format!("%{}%", q);
    let mut results = Vec::new();

    // Characters
    let chars = query_rows(
        conn,
        "SELECT id, name_fr, name_en, position FROM inagle_characters
         WHERE id = ?1 OR chara_id = ?1 OR internal_code = ?1 OR slug = ?1
            OR name_fr LIKE ?2 OR name_en LIKE ?2 OR name_ja LIKE ?2
         ORDER BY name_fr ASC LIMIT 10",
        &[&q as &dyn rusqlite::ToSql, &like_pat],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        },
    )?;
    for (id, name_fr, name_en, position) in chars {
        let name = name_fr.or(name_en).unwrap_or_else(|| id.clone());
        results.push(SearchResult {
            entity_type: "chara".to_string(),
            id,
            name,
            extra: position,
        });
    }

    // Skills
    let skills = query_rows(
        conn,
        "SELECT id, name_fr, name_en, category FROM inagle_skills
         WHERE id = ?1 OR internal_code = ?1
            OR name_fr LIKE ?2 OR name_en LIKE ?2 OR name_ja LIKE ?2
         ORDER BY name_fr ASC LIMIT 10",
        &[&q as &dyn rusqlite::ToSql, &like_pat],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        },
    )?;
    for (id, name_fr, name_en, cat) in skills {
        let name = name_fr.or(name_en).unwrap_or_else(|| id.clone());
        results.push(SearchResult {
            entity_type: "skill".to_string(),
            id,
            name,
            extra: cat,
        });
    }

    // Items
    let items = query_rows(
        conn,
        "SELECT id, name_fr, name_en, category FROM inagle_items
         WHERE id = ?1 OR internal_code = ?1
            OR name_fr LIKE ?2 OR name_en LIKE ?2 OR name_ja LIKE ?2
         ORDER BY name_fr ASC LIMIT 10",
        &[&q as &dyn rusqlite::ToSql, &like_pat],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        },
    )?;
    for (id, name_fr, name_en, cat) in items {
        let name = name_fr.or(name_en).unwrap_or_else(|| id.clone());
        results.push(SearchResult {
            entity_type: "item".to_string(),
            id,
            name,
            extra: cat,
        });
    }

    // Teams
    let teams = query_rows(
        conn,
        "SELECT id, name_fr, name_en, series FROM inagle_teams
         WHERE id = ?1 OR internal_code = ?1
            OR name_fr LIKE ?2 OR name_en LIKE ?2 OR name_ja LIKE ?2
         ORDER BY name_fr ASC LIMIT 5",
        &[&q as &dyn rusqlite::ToSql, &like_pat],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        },
    )?;
    for (id, name_fr, name_en, series) in teams {
        let name = name_fr.or(name_en).unwrap_or_else(|| id.clone());
        results.push(SearchResult {
            entity_type: "team".to_string(),
            id,
            name,
            extra: series,
        });
    }

    // Auras / Keshins / Souls
    for (table, entity_type) in &[
        ("inagle_auras", "aura"),
        ("inagle_keshins", "keshin"),
        ("inagle_souls", "soul"),
    ] {
        let sql = format!(
            "SELECT id, name_fr, name_en FROM {table}
             WHERE id = ?1 OR name_fr LIKE ?2 OR name_en LIKE ?2
             ORDER BY name_fr ASC LIMIT 5"
        );
        let rows = query_rows(
            conn,
            &sql,
            &[&q as &dyn rusqlite::ToSql, &like_pat],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )?;
        for (id, name_fr, name_en) in rows {
            let name = name_fr.or(name_en).unwrap_or_else(|| id.clone());
            results.push(SearchResult {
                entity_type: entity_type.to_string(),
                id,
                name,
                extra: None,
            });
        }
    }

    results.truncate(limit);
    Ok(results)
}

// ─── DB SQL ───────────────────────────────────────────────────────────────────

/// Vérifie que la requête SQL est en lecture seule (SELECT ou PRAGMA uniquement).
///
/// Refuse les writes pour la sécurité des données du miroir.
pub fn check_readonly_sql(sql: &str) -> anyhow::Result<()> {
    let trimmed = sql.trim().to_uppercase();
    let allowed = trimmed.starts_with("SELECT")
        || trimmed.starts_with("PRAGMA")
        || trimmed.starts_with("EXPLAIN")
        || trimmed.starts_with("WITH"); // CTE SELECT
    anyhow::ensure!(
        allowed,
        "requête non autorisée : seuls SELECT, PRAGMA, EXPLAIN et WITH … SELECT sont acceptés.\n\
         Requête reçue : {}",
        &sql[..sql.len().min(80)]
    );
    Ok(())
}

/// Exécute une requête SELECT sur le miroir et retourne les lignes sous forme JSON.
pub fn exec_readonly_sql(conn: &Connection, sql: &str) -> anyhow::Result<Vec<serde_json::Value>> {
    check_readonly_sql(sql)?;

    let mut stmt = conn.prepare(sql).map_err(|e| anyhow::anyhow!("préparation SQL : {e}"))?;
    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("col").to_string())
        .collect();

    let rows = stmt
        .query_map([], |row| {
            let mut obj = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let val: rusqlite::types::Value = row.get(i)?;
                let jval = match val {
                    rusqlite::types::Value::Null => serde_json::Value::Null,
                    rusqlite::types::Value::Integer(n) => serde_json::Value::Number(n.into()),
                    rusqlite::types::Value::Real(f) => {
                        serde_json::Number::from_f64(f)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null)
                    }
                    rusqlite::types::Value::Text(s) => serde_json::Value::String(s),
                    rusqlite::types::Value::Blob(b) => {
                        serde_json::Value::String(format!("<blob {} bytes>", b.len()))
                    }
                };
                obj.insert(name.clone(), jval);
            }
            Ok(serde_json::Value::Object(obj))
        })
        .map_err(|e| anyhow::anyhow!("exécution SQL : {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| anyhow::anyhow!("lecture ligne : {e}"))?);
    }
    Ok(result)
}

// ─── Random Team ─────────────────────────────────────────────────────────────

/// Génère une équipe aléatoire depuis le miroir, avec un PRNG seédé explicite.
///
/// Utilise `rand::rngs::SmallRng` seédé depuis `seed` pour être déterministe.
/// Reproduit la logique `random-team` TS : GK/DF/MF/FW depuis les positions,
/// avec fallback si le filtre élément/style est trop restrictif.
pub fn random_team(
    conn: &Connection,
    formation: &str,
    element_filter: Option<&str>,
    playstyle_filter: Option<&str>,
    seed: u64,
) -> anyhow::Result<RandomTeam> {
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    // Formations identiques au TS
    let formations: &[(&str, usize, usize, usize)] = &[
        // (layout, df, mf, fw)
        ("4-4-2", 4, 4, 2),
        ("4-3-3", 4, 3, 3),
        ("3-4-3", 3, 4, 3),
        ("3-5-2", 3, 5, 2),
        ("4-5-1", 4, 5, 1),
        ("3-6-1", 3, 6, 1),
        ("5-4-1", 5, 4, 1),
        ("2-4-4", 2, 4, 4),
        ("2-5-3", 2, 5, 3),
        ("5-1-4", 5, 1, 4),
        ("3-3-4", 3, 3, 4),
        ("5-3-2", 5, 3, 2),
    ];
    let (f_df, f_mf, f_fw) = formations
        .iter()
        .find(|(name, _, _, _)| *name == formation)
        .map(|(_, df, mf, fw)| (*df, *mf, *fw))
        .unwrap_or((4, 4, 2)); // défaut 4-4-2

    let mut rng = SmallRng::seed_from_u64(seed);

    // Map élément FR/EN (fidèle au TS)
    let element_db: Option<&str> = element_filter.map(|e| match e {
        "feu" | "fire" | "Feu" | "Fire" => "Feu",
        "vent" | "wind" | "Vent" | "Wind" => "Vent",
        "forêt" | "foret" | "forest" | "Forêt" | "Forest" => "Forêt",
        "montagne" | "mountain" | "Montagne" | "Mountain" => "Montagne",
        "néant" | "neant" | "void" | "Néant" | "Void" => "Néant",
        other => other,
    });

    // Récupère un pool de joueurs pour une position donnée
    let fetch_pool = |position: &str| -> anyhow::Result<Vec<(String, String, Option<String>)>> {
        // Base query : id, name_fr/name_en, element
        let base_sql = "SELECT id, COALESCE(name_fr, name_en, id), element \
                        FROM inagle_characters \
                        WHERE position = ?1 AND stat_frappe IS NOT NULL AND zukan_hash IS NOT NULL";

        // Essai avec filtre élément
        if let Some(el) = element_db {
            let filtered = query_rows(
                conn,
                &format!("{base_sql} AND element = ?2"),
                &[&position as &dyn rusqlite::ToSql, &el],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )?;
            if !filtered.is_empty() {
                return Ok(filtered);
            }
        }

        // Fallback sans filtre
        query_rows(
            conn,
            base_sql,
            &[&position as &dyn rusqlite::ToSql],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
    };

    // Sélection aléatoire de n éléments depuis un pool (sans remise)
    let pick_random = |pool: &mut Vec<(String, String, Option<String>)>,
                       n: usize,
                       rng: &mut SmallRng|
     -> Vec<RandomTeamPlayer> {
        let mut result = Vec::new();
        for _ in 0..n {
            if pool.is_empty() {
                break;
            }
            let idx = rng.gen_range(0..pool.len());
            let (id, name, element) = pool.swap_remove(idx);
            result.push(RandomTeamPlayer {
                id,
                name,
                element,
                position: String::new(), // sera rempli ci-dessous
            });
        }
        result
    };

    let mut gk_pool = fetch_pool("Gardien")?;
    let mut df_pool = fetch_pool("Défenseur")?;
    let mut mf_pool = fetch_pool("Milieu")?;
    let mut fw_pool = fetch_pool("Attaquant")?;

    let mut gk = pick_random(&mut gk_pool, 1, &mut rng);
    let mut df = pick_random(&mut df_pool, f_df, &mut rng);
    let mut mf = pick_random(&mut mf_pool, f_mf, &mut rng);
    let mut fw = pick_random(&mut fw_pool, f_fw, &mut rng);

    // Assigner les codes de position
    for p in &mut gk { p.position = "GK".to_string(); }
    for p in &mut df { p.position = "DF".to_string(); }
    for p in &mut mf { p.position = "MF".to_string(); }
    for p in &mut fw { p.position = "FW".to_string(); }

    // Sélection coach/managers depuis inagle_coordinators
    // On ignore le filtre playstyle pour les coords (table ne le supporte pas facilement)
    let _ = playstyle_filter; // non utilisé pour les coordinators ici
    let all_coords = query_rows(
        conn,
        "SELECT id, COALESCE(name_localised, name_romaji, CAST(id AS TEXT)), element, role, buff \
         FROM inagle_coordinators",
        &[],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        },
    )?;

    // Ligne SQL d'un coordinateur : (game_id, name_ja, role, ?, ?).
    type CoordRow = (i64, String, Option<String>, Option<String>, Option<String>);
    let mut coaches: Vec<_> = all_coords
        .iter()
        .filter(|(_, _, _, role, _)| role.as_deref() == Some("Coach") || role.as_deref() == Some("Manager"))
        .collect();
    let mut mgrs: Vec<_> = all_coords
        .iter()
        .filter(|(_, _, _, role, _)| role.as_deref() == Some("Coordinator"))
        .collect();

    let pick_coord = |pool: &mut Vec<&CoordRow>, rng: &mut SmallRng| -> Option<RandomTeamCoord> {
        if pool.is_empty() {
            return None;
        }
        let idx = rng.gen_range(0..pool.len());
        let (id, name, element, role, buff) = pool.swap_remove(idx);
        Some(RandomTeamCoord {
            id: *id,
            name: name.clone(),
            element: element.clone(),
            role: role.clone().unwrap_or_default(),
            buff: buff.clone(),
        })
    };

    let coach = pick_coord(&mut coaches, &mut rng);
    let managers: Vec<RandomTeamCoord> = (0..3)
        .filter_map(|_| pick_coord(&mut mgrs, &mut rng))
        .collect();

    Ok(RandomTeam {
        seed,
        formation: formation.to_string(),
        gk,
        df,
        mf,
        fw,
        coach,
        managers,
    })
}

// ─── Team Builder ─────────────────────────────────────────────────────────────

/// Liste les équipes sauvegardées dans `inagle_team_build` du miroir.
///
/// Note : le TS team-builder list/add/delete opère sur PostgreSQL (`user_teams`).
/// On porte ici uniquement la partie miroir SQLite (`inagle_team_build`).
pub fn team_build_list(conn: &Connection) -> anyhow::Result<Vec<TeamBuildEntry>> {
    let rows = query_rows(
        conn,
        "SELECT id, name_fr, name_en, position, element, data \
         FROM inagle_team_build \
         ORDER BY name_fr ASC NULLS LAST \
         LIMIT 100",
        &[],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        },
    );
    match rows {
        Err(_) => {
            // La table inagle_team_build peut ne pas exister dans certains miroirs
            Ok(Vec::new())
        }
        Ok(rows) => {
            let entries = rows
                .into_iter()
                .map(|(id, name_fr, name_en, position, element, data_str)| {
                    let id = id.unwrap_or_default();
                    let name = name_fr
                        .or(name_en)
                        .unwrap_or_else(|| id.clone());
                    let data = data_str
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or(serde_json::Value::Object(Default::default()));
                    TeamBuildEntry {
                        id,
                        name,
                        slot: None,
                        position,
                        element,
                        data,
                    }
                })
                .collect();
            Ok(entries)
        }
    }
}

/// Calcule un résumé de stats pour un joueur inagle_team_build via son ID.
pub fn team_build_calc(conn: &Connection, id: &str) -> anyhow::Result<Option<TeamBuildEntry>> {
    let q = sanitize_filter(id);
    let row = query_one(
        conn,
        "SELECT id, name_fr, name_en, position, element, data \
         FROM inagle_team_build WHERE id = ?1 OR name_fr = ?1 OR name_en = ?1 LIMIT 1",
        &[&q as &dyn rusqlite::ToSql],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        },
    );
    match row {
        Err(_) => Ok(None),
        Ok(None) => Ok(None),
        Ok(Some((id_opt, name_fr, name_en, position, element, data_str))) => {
            let id_val = id_opt.unwrap_or_default();
            let name = name_fr.or(name_en).unwrap_or_else(|| id_val.clone());
            let data = data_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(serde_json::Value::Object(Default::default()));
            Ok(Some(TeamBuildEntry {
                id: id_val,
                name,
                slot: None,
                position,
                element,
                data,
            }))
        }
    }
}

// ─── Status ───────────────────────────────────────────────────────────────────

/// Produit un rapport de diagnostic : miroir SQLite + Redis db0 + Redis db3.
///
/// Identique à `status` TS : compte les tables clés, ping Redis.
pub fn status_report(conn: &Connection) -> anyhow::Result<StatusReport> {
    // SQLite
    let mut sqlite = SqliteStatus {
        healthy: false,
        path: String::new(),
        size_mb: None,
        table_count: None,
        characters: None,
        skills: None,
        items: None,
        teams: None,
        error: None,
    };

    // Résolution du chemin depuis le miroir ouvert
    let path = crate::mirror::resolve(None)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "inconnu".to_string());
    sqlite.path = path.clone();

    // Taille du fichier
    if let Ok(meta) = std::fs::metadata(&path) {
        sqlite.size_mb = Some(meta.len() as f64 / (1024.0 * 1024.0));
    }

    let count_table = |sql: &str| -> Option<u64> {
        conn.query_row(sql, [], |row| row.get::<_, i64>(0))
            .ok()
            .map(|n| n as u64)
    };

    sqlite.table_count = count_table(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
    );
    sqlite.characters = count_table("SELECT COUNT(*) FROM inagle_characters");
    sqlite.skills = count_table("SELECT COUNT(*) FROM inagle_skills");
    sqlite.items = count_table("SELECT COUNT(*) FROM inagle_items");
    sqlite.teams = count_table("SELECT COUNT(*) FROM inagle_teams");
    sqlite.healthy = sqlite.characters.is_some();

    // Redis db0 et db3
    let redis_ping = |url: &str, db: u8| -> RedisStatus {
        let client = match redis::Client::open(url) {
            Ok(c) => c,
            Err(e) => {
                return RedisStatus {
                    healthy: false,
                    latency_ms: None,
                    db,
                    error: Some(e.to_string()),
                }
            }
        };
        let mut conn = match client.get_connection() {
            Ok(c) => c,
            Err(e) => {
                return RedisStatus {
                    healthy: false,
                    latency_ms: None,
                    db,
                    error: Some(e.to_string()),
                }
            }
        };
        use redis::Commands;
        let start = std::time::Instant::now();
        let result: Result<(), _> = conn.set_ex("niers:status:ping", "1", 10);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        match result {
            Ok(_) => RedisStatus {
                healthy: true,
                latency_ms: Some(elapsed),
                db,
                error: None,
            },
            Err(e) => RedisStatus {
                healthy: false,
                latency_ms: None,
                db,
                error: Some(e.to_string()),
            },
        }
    };

    let redis_db0 = redis_ping("redis://127.0.0.1/0", 0);
    let redis_db3 = redis_ping("redis://127.0.0.1/3", 3);

    Ok(StatusReport {
        sqlite,
        redis_db0,
        redis_db3,
    })
}

// ─── Audit ────────────────────────────────────────────────────────────────────

/// Audite la cohérence du miroir : counts et nulls sur les tables clés.
///
/// Reproduit fidèlement `audit` TS (counts/nulls), adapté au SQLite.
pub fn audit_mirror(conn: &Connection) -> anyhow::Result<AuditReport> {
    let audit_table = |table: &str| -> AuditTable {
        let count_sql = format!("SELECT COUNT(*) FROM {table}");
        let total = conn
            .query_row(&count_sql, [], |r| r.get::<_, i64>(0))
            .ok()
            .unwrap_or(0) as u64;

        let count_null = |col: &str| -> u64 {
            let sql = format!("SELECT COUNT(*) FROM {table} WHERE {col} IS NULL OR {col} = ''");
            conn.query_row(&sql, [], |r| r.get::<_, i64>(0))
                .ok()
                .unwrap_or(0) as u64
        };
        let null_data = {
            let sql = format!("SELECT COUNT(*) FROM {table} WHERE data IS NULL");
            conn.query_row(&sql, [], |r| r.get::<_, i64>(0))
                .ok()
                .unwrap_or(0) as u64
        };

        AuditTable {
            total,
            missing_name_fr: count_null("name_fr"),
            missing_name_en: count_null("name_en"),
            null_data,
        }
    };

    Ok(AuditReport {
        characters: audit_table("inagle_characters"),
        skills: audit_table("inagle_skills"),
        items: audit_table("inagle_items"),
        teams: audit_table("inagle_teams"),
        auras: audit_table("inagle_auras"),
        keshins: audit_table("inagle_keshins"),
        souls: audit_table("inagle_souls"),
    })
}

// ─── Dialogue ────────────────────────────────────────────────────────────────

/// Recherche dans les sous-titres/dialogues de `inagle_event_subtitles`.
///
/// Reproduit `dialogue` TS : filtre par texte (FR/EN/JA).
/// La table `inagle_event_subtitles` est présente dans le miroir (2093 lignes).
pub fn search_dialogues(
    conn: &Connection,
    text_query: &str,
    limit: usize,
) -> anyhow::Result<Vec<DialogueMatch>> {
    let q = sanitize_filter(text_query);
    let like_pat = format!("%{}%", q);
    let limit_i = limit as i64;

    query_rows(
        conn,
        "SELECT event_id, episode, line_index, text_fr, text_en, text_ja \
         FROM inagle_event_subtitles \
         WHERE text_fr LIKE ?1 OR text_en LIKE ?1 OR text_ja LIKE ?1 \
         ORDER BY event_id ASC, line_index ASC \
         LIMIT ?2",
        &[&like_pat as &dyn rusqlite::ToSql, &limit_i],
        |row| {
            Ok(DialogueMatch {
                event_id: row.get(0)?,
                episode: row.get(1)?,
                line_index: row.get(2)?,
                text_fr: row.get(3)?,
                text_en: row.get(4)?,
                text_ja: row.get(5)?,
            })
        },
    )
}

// ─── Redis get/set/del ────────────────────────────────────────────────────────

/// Exécute une commande Redis simple : get / set / del.
///
/// Identique à `redis` TS : opère sur l'URL fournie (db0 ou db3 selon le contexte).
pub fn redis_cmd(url: &str, cmd: &str, key: &str, val: Option<&str>) -> anyhow::Result<Option<String>> {
    use redis::Commands;
    let client = redis::Client::open(url).map_err(|e| anyhow::anyhow!("connexion Redis : {e}"))?;
    let mut conn = client
        .get_connection()
        .map_err(|e| anyhow::anyhow!("Redis get_connection : {e}"))?;

    match cmd.to_lowercase().as_str() {
        "get" => {
            let result: Option<String> = conn
                .get(key)
                .map_err(|e| anyhow::anyhow!("Redis GET : {e}"))?;
            Ok(result)
        }
        "set" => {
            let v = val.ok_or_else(|| anyhow::anyhow!("valeur requise pour set"))?;
            conn.set::<_, _, ()>(key, v)
                .map_err(|e| anyhow::anyhow!("Redis SET : {e}"))?;
            Ok(Some("OK".to_string()))
        }
        "del" => {
            conn.del::<_, ()>(key)
                .map_err(|e| anyhow::anyhow!("Redis DEL : {e}"))?;
            Ok(Some("DEL OK".to_string()))
        }
        other => anyhow::bail!("commande Redis inconnue : '{}'. Utiliser : get / set / del", other),
    }
}
