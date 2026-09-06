//! Orchestration du pull complet du Zukan Inagle.
//!
//! Flux :
//! 1. `chara_list` pages 1..N × 3 langues → collecte des IDs + cache HTML
//! 2. `chara_param` par ID × 3 langues → stats + métadonnées → cache HTML
//! 3. `skill` pages 1..18 × 3 langues → 900 skills
//! 4. `item/equip` pages × catégorie × 3 langues → 223 items
//!
//! Toutes les sorties sont en NDJSON sous `var/zukan/<lang>/<type>.ndjson`.

use crate::{
    client::ZukanClient,
    forge,
    models::{CharaListEntry, Lang},
    parser,
};
use anyhow::{Context, Result};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Configuration du pull.
pub struct PullConfig {
    /// Répertoire racine du cache HTML.
    pub cache_root: PathBuf,
    /// Répertoire de sortie NDJSON.
    pub output_root: PathBuf,
    /// Langues à puller.
    pub langs: Vec<Lang>,
    /// Nombre maximum de perso à puller via `chara_param` (0 = tous).
    pub chara_param_limit: usize,
}

impl Default for PullConfig {
    fn default() -> Self {
        Self {
            cache_root: PathBuf::from("var/zukan"),
            output_root: PathBuf::from("var/zukan"),
            langs: Lang::all().to_vec(),
            chara_param_limit: 0,
        }
    }
}

/// Résultat d'un pull complet.
#[derive(Debug, Default)]
pub struct PullStats {
    pub chara_ids_discovered: usize,
    pub chara_params_fetched: usize,
    pub skills_fetched: usize,
    pub items_fetched: usize,
    pub errors: usize,
}

/// Exécute le pull complet.
pub fn run_pull(config: &PullConfig) -> Result<PullStats> {
    let client = ZukanClient::new(config.cache_root.clone())?;
    let mut stats = PullStats::default();

    for &lang in &config.langs {
        info!(lang = %lang, "début pull chara_list");
        let ids = pull_chara_list(&client, lang, config)?;
        stats.chara_ids_discovered += ids.len();
        info!(lang = %lang, count = ids.len(), "chara_list terminé");

        // Pull chara_param
        let limit = if config.chara_param_limit == 0 {
            ids.len()
        } else {
            config.chara_param_limit.min(ids.len())
        };
        let to_fetch = &ids[..limit];
        info!(lang = %lang, count = limit, "début pull chara_param");
        let fetched = pull_chara_params(&client, lang, to_fetch, config)?;
        stats.chara_params_fetched += fetched;

        // Pull skills
        info!(lang = %lang, "début pull skills");
        let skill_count = pull_skills(&client, lang, config)?;
        stats.skills_fetched += skill_count;
        info!(lang = %lang, count = skill_count, "skills terminés");

        // Pull items
        info!(lang = %lang, "début pull items");
        let item_count = pull_items(&client, lang, config)?;
        stats.items_fetched += item_count;
        info!(lang = %lang, count = item_count, "items terminés");
    }

    Ok(stats)
}

// ---------------------------------------------------------------------------
// chara_list
// ---------------------------------------------------------------------------

/// Pull toutes les pages de `chara_list` pour une langue.
/// Retourne la liste de toutes les entrées (`game_id` + q).
pub fn pull_chara_list(
    client: &ZukanClient,
    lang: Lang,
    config: &PullConfig,
) -> Result<Vec<CharaListEntry>> {
    let base = format!("https://zukan.inazuma.jp{}/chara_list/", lang.path_prefix());
    let mut all_entries = Vec::new();

    // D'abord fetch la page 1 pour connaître le nombre total de pages
    let cache_path = client.cache_path(lang.code(), "chara_list", "page_1");
    let url_p1 = format!("{base}?page=1");
    let html = client.get_cached(&url_p1, &cache_path)?;
    let (entries, total_pages) =
        parser::parse_chara_list(&html).context("parse chara_list page 1")?;

    info!(lang = %lang, total_pages, "pages chara_list détectées");
    all_entries.extend(entries);

    // Fetch les pages restantes
    for page in 2..=total_pages {
        let cache_path = client.cache_path(lang.code(), "chara_list", &format!("page_{page}"));
        let url = format!("{base}?page={page}");
        match client.get_cached(&url, &cache_path) {
            Ok(html) => match parser::parse_chara_list(&html) {
                Ok((entries, _)) => {
                    all_entries.extend(entries);
                }
                Err(e) => {
                    warn!(lang = %lang, page, error = %e, "parse chara_list échoué");
                }
            },
            Err(e) => {
                warn!(lang = %lang, page, error = %e, "fetch chara_list échoué");
            }
        }
    }

    // Dédupliquer par game_id (certaines variantes peuvent apparaître plusieurs fois)
    all_entries.dedup_by(|a, b| a.game_id == b.game_id);

    // Sauvegarder la liste des IDs en NDJSON
    let out_path = config
        .output_root
        .join(lang.code())
        .join("chara_ids.ndjson");
    write_ndjson(&out_path, &all_entries)?;
    info!(lang = %lang, path = %out_path.display(), count = all_entries.len(), "chara_ids sauvegardés");

    Ok(all_entries)
}

// ---------------------------------------------------------------------------
// chara_param
// ---------------------------------------------------------------------------

/// Pull les `chara_param` pour une slice d'entrées.
/// Écrit les résultats en NDJSON (append).
pub fn pull_chara_params(
    client: &ZukanClient,
    lang: Lang,
    entries: &[CharaListEntry],
    config: &PullConfig,
) -> Result<usize> {
    let base = format!(
        "https://zukan.inazuma.jp{}/chara_param/",
        lang.path_prefix()
    );

    let out_path = config
        .output_root
        .join(lang.code())
        .join("chara_param.ndjson");
    std::fs::create_dir_all(out_path.parent().unwrap())?;
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)?;

    // Construire un set des IDs déjà présents dans le ndjson
    let already_done = already_fetched_ids(&out_path, "game_id")?;
    let mut count = 0;

    for entry in entries {
        if already_done.contains(&entry.game_id) {
            continue;
        }

        // Forger le q pour chara_param
        let q = match forge::q_for_chara_param(&entry.game_id) {
            Ok(q) => q,
            Err(e) => {
                warn!(id = %entry.game_id, error = %e, "forge q chara_param échoué");
                continue;
            }
        };

        let cache_path = client.cache_path(lang.code(), "chara_param", &entry.game_id);
        let url = format!("{base}?q={q}");

        match client.get_cached(&url, &cache_path) {
            Ok(html) => match parser::parse_chara_param(&html, &entry.game_id, lang) {
                Ok(charas) => {
                    for chara in &charas {
                        let line = serde_json::to_string(chara)?;
                        writeln!(out, "{line}")?;
                        count += 1;
                    }
                    if charas.is_empty() {
                        warn!(id = %entry.game_id, lang = %lang, "aucun chara parsé");
                    }
                }
                Err(e) => {
                    warn!(id = %entry.game_id, lang = %lang, error = %e, "parse chara_param échoué");
                }
            },
            Err(e) => {
                warn!(id = %entry.game_id, lang = %lang, error = %e, "fetch chara_param échoué");
            }
        }
    }

    info!(lang = %lang, count, path = %out_path.display(), "chara_param écrits");
    Ok(count)
}

// ---------------------------------------------------------------------------
// skills
// ---------------------------------------------------------------------------

/// Catégories de skills (pour référence documentaire).
/// La liste complète est paginée sans filtre catégorie — on pull toutes les pages en une fois.
#[allow(dead_code)]
const SKILL_CATEGORIES: &[(&str, u32)] =
    &[("shoot", 1), ("offense", 2), ("defense", 3), ("keeper", 4)];

/// Pull toutes les pages de skills pour une langue.
pub fn pull_skills(client: &ZukanClient, lang: Lang, config: &PullConfig) -> Result<usize> {
    let base = format!("https://zukan.inazuma.jp{}/skill/", lang.path_prefix());

    let out_path = config.output_root.join(lang.code()).join("skills.ndjson");

    // Si le fichier existe et n'est pas vide, les skills sont déjà pullés — skip
    if out_path.exists() && std::fs::metadata(&out_path)?.len() > 0 {
        let count = std::fs::read_to_string(&out_path)?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        info!(lang = %lang, count, "skills déjà en cache → skip");
        return Ok(count);
    }

    std::fs::create_dir_all(out_path.parent().unwrap())?;
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)?;

    // Page 1 sans filtre pour connaître le total de pages
    let cache_path = client.cache_path(lang.code(), "skill", "all_page_1");
    let html = client.get_cached(&format!("{base}?page=1"), &cache_path)?;
    let total_pages = extract_last_page_from_html(&html);
    info!(lang = %lang, total_pages, "pages skills détectées");

    let mut count = 0;

    for page in 1..=total_pages {
        let cache_path = client.cache_path(lang.code(), "skill", &format!("all_page_{page}"));
        let url = format!("{base}?page={page}");
        let html = match client.get_cached(&url, &cache_path) {
            Ok(h) => h,
            Err(e) => {
                warn!(lang = %lang, page, error = %e, "fetch skill page échoué");
                continue;
            }
        };
        match crate::parser::parse_skill_list(&html, lang, page) {
            Ok(skills) => {
                for skill in &skills {
                    let line = serde_json::to_string(skill)?;
                    writeln!(out, "{line}")?;
                    count += 1;
                }
            }
            Err(e) => {
                warn!(lang = %lang, page, error = %e, "parse skill page échoué");
            }
        }
    }

    info!(lang = %lang, count, path = %out_path.display(), "skills écrits");
    Ok(count)
}

// ---------------------------------------------------------------------------
// items
// ---------------------------------------------------------------------------

/// Catégories d'items à puller.
const ITEM_CATEGORIES: &[(&str, &str, u32)] = &[
    ("shoes", "シューズ", 30),
    ("misanga", "ミサンガ", 40),
    ("pendant", "ペンダント", 50),
    ("special", "スペシャル", 60),
];

/// Pull toutes les pages d'items pour une langue.
pub fn pull_items(client: &ZukanClient, lang: Lang, config: &PullConfig) -> Result<usize> {
    let base = format!("https://zukan.inazuma.jp{}/item/equip/", lang.path_prefix());

    let out_path = config.output_root.join(lang.code()).join("items.ndjson");

    // Si le fichier existe et n'est pas vide, les items sont déjà pullés — skip
    if out_path.exists() && std::fs::metadata(&out_path)?.len() > 0 {
        let count = std::fs::read_to_string(&out_path)?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        info!(lang = %lang, count, "items déjà en cache → skip");
        return Ok(count);
    }

    std::fs::create_dir_all(out_path.parent().unwrap())?;
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)?;

    let mut count = 0;

    for &(cat_key, cat_name, cat_id) in ITEM_CATEGORIES {
        let q = forge::q_for_item_category(cat_id)?;

        // Page 1 pour connaître le total
        let cache_path = client.cache_path(lang.code(), "item", &format!("{cat_key}_page_1"));
        let url_p1 = format!("{base}?page=1&q={q}");
        let html_p1 = match client.get_cached(&url_p1, &cache_path) {
            Ok(h) => h,
            Err(e) => {
                warn!(lang = %lang, cat = cat_key, error = %e, "fetch item page 1 échoué");
                continue;
            }
        };
        let total_pages = extract_last_page_from_html(&html_p1);

        // Parse + écriture page 1
        match crate::parser::parse_item_list(&html_p1, lang, cat_name, 1) {
            Ok(items) => {
                for item in &items {
                    writeln!(out, "{}", serde_json::to_string(item)?)?;
                    count += 1;
                }
            }
            Err(e) => {
                warn!(lang = %lang, cat = cat_key, error = %e, "parse item page 1 échoué");
            }
        }

        // Pages restantes
        for page in 2..=total_pages {
            let cache_path =
                client.cache_path(lang.code(), "item", &format!("{cat_key}_page_{page}"));
            let url = format!("{base}?page={page}&q={q}");
            match client.get_cached(&url, &cache_path) {
                Ok(html) => match crate::parser::parse_item_list(&html, lang, cat_name, page) {
                    Ok(items) => {
                        for item in &items {
                            writeln!(out, "{}", serde_json::to_string(item)?)?;
                            count += 1;
                        }
                    }
                    Err(e) => {
                        warn!(lang = %lang, cat = cat_key, page, error = %e, "parse item échoué");
                    }
                },
                Err(e) => {
                    warn!(lang = %lang, cat = cat_key, page, error = %e, "fetch item échoué");
                }
            }
        }
    }

    info!(lang = %lang, count, path = %out_path.display(), "items écrits");
    Ok(count)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Écrit une liste de valeurs sérialisables en NDJSON.
fn write_ndjson<T: serde::Serialize>(path: &Path, items: &[T]) -> Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut out = std::fs::File::create(path)?;
    for item in items {
        let line = serde_json::to_string(item)?;
        writeln!(out, "{line}")?;
    }
    Ok(())
}

/// Lit un fichier NDJSON existant et retourne les valeurs d'un champ string (ex. `game_id`).
fn already_fetched_ids(path: &Path, field: &str) -> Result<std::collections::HashSet<String>> {
    let mut set = std::collections::HashSet::new();
    if !path.exists() {
        return Ok(set);
    }
    let content = std::fs::read_to_string(path)?;
    for line in content.lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line)
            && let Some(id) = v.get(field).and_then(|v| v.as_str())
        {
            set.insert(id.to_owned());
        }
    }
    Ok(set)
}

/// Extrait le numéro de la dernière page d'un HTML de pagination.
fn extract_last_page_from_html(html: &str) -> u32 {
    let prefix = "?page=";
    let mut max = 1u32;
    let mut search = html;
    while let Some(pos) = search.find(prefix) {
        let rest = &search[pos + prefix.len()..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if let Ok(n) = rest[..end].parse::<u32>() {
            max = max.max(n);
        }
        search = &rest[end.max(1)..];
    }
    max
}
