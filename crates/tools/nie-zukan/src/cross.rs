//! Croisement des données Zukan avec le miroir inagle `SQLite`.
//!
//! Aligne `game_id` (ex. `c01000010`) avec `inagle_characters.chara_id`.
//! Mesure :
//! - combien matchent
//! - combien d'IDs zukan sont absents d'inagle (trous à combler)
//! - des exemples de champs que le zukan a et pas le wiki

use crate::models::{CrossResult, EnrichmentExample, ZukanChara};
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::info;

/// Effectue le croisement entre les données zukan (JP) et le miroir inagle.
pub fn cross_with_inagle(zukan_charas: &[ZukanChara], mirror_path: &Path) -> Result<CrossResult> {
    let conn = Connection::open(mirror_path).context("ouverture miroir SQLite")?;

    // Charger tous les internal_code distincts d'inagle (= IDs de jeu style c01000010)
    // Note : chara_id dans inagle = hash 0x... ; internal_code = ID de jeu alphanumérique
    let inagle_ids: HashSet<String> = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT internal_code FROM inagle_characters WHERE internal_code IS NOT NULL",
        )?;
        stmt.query_map([], |row| row.get(0))?
            .filter_map(std::result::Result::ok)
            .collect()
    };

    info!(inagle_total = inagle_ids.len(), "IDs inagle chargés");

    // Charger les stats inagle pour comparaison via internal_code
    // (stat_frappe=kick, stat_controle=control, …)
    let inagle_stats: HashMap<String, InagleStats> = {
        let mut stmt = conn.prepare(
            "SELECT internal_code, name_ja, stat_frappe, stat_controle, stat_technique,
             stat_pression, stat_physique, stat_agilite, stat_intelligence,
             description_ja, element, position
             FROM inagle_characters
             WHERE internal_code IS NOT NULL
             GROUP BY internal_code",
        )?;
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                InagleStats {
                    name_ja: row.get(1).unwrap_or_default(),
                    kick: row.get(2).unwrap_or(0),
                    control: row.get(3).unwrap_or(0),
                    technique: row.get(4).unwrap_or(0),
                    pressure: row.get(5).unwrap_or(0),
                    physical: row.get(6).unwrap_or(0),
                    agility: row.get(7).unwrap_or(0),
                    intelligence: row.get(8).unwrap_or(0),
                    description_ja: row.get(9).unwrap_or_default(),
                    element: row.get(10).unwrap_or_default(),
                    position: row.get(11).unwrap_or_default(),
                },
            ))
        })?
        .filter_map(std::result::Result::ok)
        .collect()
    };

    // IDs uniques du zukan
    let zukan_ids: HashSet<String> = zukan_charas.iter().map(|c| c.game_id.clone()).collect();
    let zukan_total = zukan_ids.len();

    // Matchés
    let matched_ids: HashSet<&String> = zukan_ids.intersection(&inagle_ids).collect();
    let matched = matched_ids.len();

    // Absents d'inagle
    let absent_from_inagle: Vec<String> = {
        let mut v: Vec<String> = zukan_ids.difference(&inagle_ids).cloned().collect();
        v.sort();
        v
    };

    // Exemples d'enrichissement : champs présents dans zukan mais absents/différents dans inagle
    let mut enrichment_examples = Vec::new();

    // On prend les 20 premiers matchés pour les exemples
    let sample: Vec<&ZukanChara> = zukan_charas
        .iter()
        .filter(|c| matched_ids.contains(&c.game_id))
        .take(20)
        .collect();

    for chara in sample {
        let Some(inagle) = inagle_stats.get(&chara.game_id) else {
            continue;
        };

        // Exemple 1 : description localisée (le zukan a souvent une bio que le wiki n'a pas)
        if let Some(ref desc) = chara.description
            && !desc.is_empty()
        {
            let inagle_desc = if inagle.description_ja.is_empty() {
                None
            } else {
                Some(inagle.description_ja.chars().take(50).collect::<String>())
            };
            if inagle_desc.is_none() || inagle_desc.as_deref() != Some(desc.as_str()) {
                enrichment_examples.push(EnrichmentExample {
                    game_id: chara.game_id.clone(),
                    name_ja: inagle.name_ja.clone(),
                    field: "description_ja".to_owned(),
                    zukan_value: desc.chars().take(80).collect(),
                    inagle_value: inagle_desc,
                });
            }
        }

        // Exemple 2 : courbes de stats (le zukan donne des valeurs Lv50 différentes)
        if let Some(ref stats) = chara.stats.lv50 {
            // Comparer le kick
            if stats.kick != inagle.kick as u32 && inagle.kick != 0 {
                enrichment_examples.push(EnrichmentExample {
                    game_id: chara.game_id.clone(),
                    name_ja: inagle.name_ja.clone(),
                    field: "stat_kick_lv50".to_owned(),
                    zukan_value: stats.kick.to_string(),
                    inagle_value: Some(inagle.kick.to_string()),
                });
            }
        }

        // Exemple 3 : acquisition (入手方法 — absent d'inagle)
        if let Some(ref acq) = chara.acquisition
            && !acq.is_empty()
        {
            enrichment_examples.push(EnrichmentExample {
                game_id: chara.game_id.clone(),
                name_ja: inagle.name_ja.clone(),
                field: "acquisition_ja".to_owned(),
                zukan_value: acq.chars().take(100).collect(),
                inagle_value: None, // inagle n'a pas ce champ
            });
        }

        if enrichment_examples.len() >= 15 {
            break;
        }
    }

    let result = CrossResult {
        zukan_total,
        matched,
        absent_from_inagle,
        enrichment_examples,
    };

    info!(
        zukan_total,
        matched,
        absent = result.absent_from_inagle.len(),
        "croisement terminé"
    );

    Ok(result)
}

#[derive(Debug)]
struct InagleStats {
    name_ja: String,
    kick: i32,
    #[allow(dead_code)]
    control: i32,
    #[allow(dead_code)]
    technique: i32,
    #[allow(dead_code)]
    pressure: i32,
    #[allow(dead_code)]
    physical: i32,
    #[allow(dead_code)]
    agility: i32,
    #[allow(dead_code)]
    intelligence: i32,
    description_ja: String,
    #[allow(dead_code)]
    element: String,
    #[allow(dead_code)]
    position: String,
}

/// Charge les personnages zukan depuis un fichier NDJSON.
pub fn load_zukan_charas_from_ndjson(path: &Path) -> Result<Vec<ZukanChara>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let mut results = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<ZukanChara>(line) {
            Ok(c) => results.push(c),
            Err(e) => {
                tracing::warn!(error = %e, line = &line[..line.len().min(80)], "parse ZukanChara échoué");
            }
        }
    }
    Ok(results)
}
