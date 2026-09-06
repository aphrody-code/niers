//! Exporte la base de données unifiée des passives IEVR vers `apps/azalee/data/passives-full.json`.
//!
//! # Usage
//!
//! ```text
//! cargo run --bin export_passives --features serde -- \
//!     --data $NIE_GAME_DIR/data \
//!     --out export/passives-full.json
//! ```
//!
//! # Sources lues
//!
//! - `common/gamedata/skill/passive_skill_config_5.00.07.00.cfg.bin.json` (1716 passives joueur)
//! - `common/gamedata/soccer/soccer_team_passive_config_0.00.00.cfg.bin.json` (21 team passives)
//! - `common/gamedata/character/team_passive_lot_table_config_0.00.00.cfg.bin.json` (653 lots)
//! - `common/text/{fr,en,ja}/skill_text.cfg.bin.json` (NOUN_INFO)
//! - `common/text/{fr,en,ja}/soccer_team_passive_text.cfg.bin.json` (TEXT_INFO team passives)

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use nie_data::passives::{
    UnifiedPassiveDb, element_name, load_noun_texts, load_team_passive_texts, parse_lots,
    parse_player_passives, parse_team_passives,
};
use serde_json::{Value, json};

/// Racine des données décodées, sans chemin de poste en dur : `NIE_GAME_DIR/data` si la
/// variable est posée, sinon `./data` (le dépôt est fusionné avec l'installation du jeu).
fn default_data_root() -> String {
    std::env::var("NIE_GAME_DIR")
        .map(|d| format!("{d}/data"))
        .unwrap_or_else(|_| "data".to_string())
}

fn main() {
    // Chemins par défaut (peuvent être surchargés par les args)
    let data_root = std::env::args()
        .skip_while(|a| a != "--data")
        .nth(1)
        .unwrap_or_else(default_data_root);

    let out_path = std::env::args()
        .skip_while(|a| a != "--out")
        .nth(1)
        .unwrap_or_else(|| "export/passives-full.json".to_string());

    let data_root = PathBuf::from(&data_root);

    eprintln!("[export_passives] data={data_root:?}  out={out_path:?}");

    // ── 1. Charger les textes skill_text (NOUN_INFO) ──────────────────────────
    let text_fr = load_text_json(
        &data_root.join("common/text/fr/skill_text.cfg.bin.json"),
        "fr/skill_text",
        load_noun_texts,
    );
    let text_en = load_text_json(
        &data_root.join("common/text/en/skill_text.cfg.bin.json"),
        "en/skill_text",
        load_noun_texts,
    );
    let text_ja = load_text_json(
        &data_root.join("common/text/ja/skill_text.cfg.bin.json"),
        "ja/skill_text",
        load_noun_texts,
    );

    eprintln!(
        "[export_passives] NOUN_INFO chargés : fr={} en={} ja={}",
        text_fr.len(),
        text_en.len(),
        text_ja.len(),
    );

    // ── 2. Parse des passives joueur ──────────────────────────────────────────
    let passive_cfg = find_cfg(&data_root, "common/gamedata/skill", "passive_skill_config_");
    eprintln!("[export_passives] passive_skill_config → {passive_cfg:?}");
    let passive_root = read_json(&passive_cfg);
    let player = parse_player_passives(&passive_root, &text_fr, &text_en, &text_ja);
    eprintln!(
        "[export_passives] passives joueur parsés : {}",
        player.len()
    );

    // ── 3. Textes team passives ───────────────────────────────────────────────
    let team_text_ja = load_text_json(
        &data_root.join("common/text/ja/soccer_team_passive_text.cfg.bin.json"),
        "ja/soccer_team_passive_text",
        load_team_passive_texts,
    );
    eprintln!(
        "[export_passives] soccer_team_passive_text (ja) : {} entrées",
        team_text_ja.len()
    );

    // ── 4. Parse des team passives ────────────────────────────────────────────
    let team_cfg = find_cfg(
        &data_root,
        "common/gamedata/soccer",
        "soccer_team_passive_config_",
    );
    eprintln!("[export_passives] soccer_team_passive_config → {team_cfg:?}");
    let team_root = read_json(&team_cfg);
    let team = parse_team_passives(&team_root, &team_text_ja);
    eprintln!("[export_passives] team passives parsés : {}", team.len());

    // ── 5. Parse des lots ─────────────────────────────────────────────────────
    let lot_cfg = find_cfg(
        &data_root,
        "common/gamedata/character",
        "team_passive_lot_table_config_",
    );
    eprintln!("[export_passives] team_passive_lot_table_config → {lot_cfg:?}");
    let lot_root = read_json(&lot_cfg);
    let lots = parse_lots(&lot_root);
    eprintln!("[export_passives] lots parsés : {}", lots.len());

    // ── 6. Calcul des métriques ───────────────────────────────────────────────
    let unique_effect_count = player
        .iter()
        .map(|p| p.effect_id.get())
        .collect::<BTreeSet<_>>()
        .len();
    let unique_string_id_count = player
        .iter()
        .map(|p| p.string_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();

    let db = UnifiedPassiveDb {
        unique_effect_count,
        unique_string_id_count,
        player,
        team,
        lots,
    };

    // ── 7. Sérialisation JSON ─────────────────────────────────────────────────
    let json_val = build_json(&db);

    // Vérification counts
    let player_count = json_val["meta"]["player_count"].as_u64().unwrap_or(0);
    let team_count = json_val["meta"]["team_count"].as_u64().unwrap_or(0);
    let lot_count = json_val["meta"]["lot_count"].as_u64().unwrap_or(0);

    // Echantillon : première passive résolue FR non vide
    if let Some(first) = db.player.iter().find(|p| p.text_resolved.fr.is_some()) {
        eprintln!(
            "[export_passives] échantillon player[0]: string_id={} rarity={} element={} value={:?} text_fr={:?}",
            first.string_id,
            first.rarity,
            element_name(first.element),
            first.main_value(),
            first.text_resolved.fr.as_deref().unwrap_or(""),
        );
    }
    if let Some(first_team) = db.team.first() {
        eprintln!(
            "[export_passives] échantillon team[0]: id={} value_min={} value_max={} text_ja={:?}",
            first_team.team_passive_id,
            first_team.effect_value_min,
            first_team.effect_value_max,
            first_team.text.ja.as_deref().unwrap_or("(vide)"),
        );
    }

    let json_bytes = serde_json::to_vec_pretty(&json_val).expect("sérialisation JSON impossible");

    // Créer le dossier parent si nécessaire
    if let Some(parent) = Path::new(&out_path).parent() {
        fs::create_dir_all(parent).expect("impossible de créer le dossier de sortie");
    }

    fs::write(&out_path, &json_bytes).expect("impossible d'écrire le fichier JSON");

    let kb = json_bytes.len() / 1024;
    eprintln!(
        "[export_passives] OK — player_count={player_count} team_count={team_count} lot_count={lot_count} taille={kb}Ko → {out_path}"
    );

    // Sortie standard machine-readable (1 ligne terse comme convention niers CLI)
    println!(
        "player_count={player_count} team_count={team_count} lot_count={lot_count} unique_effects={unique_effect_count} unique_string_ids={unique_string_id_count} out={out_path} size_kb={kb}"
    );
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Lit un fichier JSON de textes (NOUN_INFO ou TEXT_INFO) — retourne un BTreeMap vide si absent.
fn load_text_json<F>(path: &Path, label: &str, loader: F) -> std::collections::BTreeMap<u32, String>
where
    F: Fn(&Value) -> std::collections::BTreeMap<u32, String>,
{
    match fs::read_to_string(path) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(v) => loader(&v),
            Err(e) => {
                eprintln!("[export_passives] WARN parse {label}: {e}");
                Default::default()
            }
        },
        Err(e) => {
            eprintln!("[export_passives] WARN lecture {label} ({path:?}): {e}");
            Default::default()
        }
    }
}

/// Trouve le fichier `*.cfg.bin.json` dans `subdir` commençant par `prefix`, en prenant
/// le plus récent (tri lexicographique descendant — les versions Level-5 sont des chaînes
/// comparables : "5.00.07.00" > "0.08.86").
fn find_cfg(data_root: &Path, subdir: &str, prefix: &str) -> PathBuf {
    let dir = data_root.join(subdir);
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(prefix) && name.ends_with(".cfg.bin.json") {
                candidates.push(entry.path());
            }
        }
    }
    if candidates.is_empty() {
        panic!(
            "[export_passives] fichier introuvable : {subdir}/{prefix}*.cfg.bin.json (cherché dans {dir:?})"
        );
    }
    // Tri lexicographique descendant — "5.00.07.00" > "0.08.86" alphabétiquement
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    candidates.remove(0)
}

/// Lit et parse un fichier JSON.
fn read_json(path: &Path) -> Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("[export_passives] lecture {path:?}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("[export_passives] parse {path:?}: {e}"))
}

/// Construit le JSON de sortie complet (schema stable pour azalee).
fn build_json(db: &UnifiedPassiveDb) -> Value {
    let player_arr: Vec<Value> = db
        .player
        .iter()
        .map(|p| {
            json!({
                "passive_id": p.passive_id.to_string(),
                "string_id": p.string_id,
                "effect_id": p.effect_id.to_string(),
                "rarity": p.rarity,
                "element": p.element,
                "element_name": element_name(p.element),
                "buff_icon_type": p.buff_icon_type,
                "effect_params": p.effect_params,
                "main_value": p.main_value(),
                "name": {
                    "fr": p.text_resolved.fr,
                    "en": p.text_resolved.en,
                    "ja": p.text_resolved.ja,
                },
                "description": {
                    "fr": p.text_resolved.fr,
                    "en": p.text_resolved.en,
                    "ja": p.text_resolved.ja,
                },
                "text_raw": {
                    "fr": p.text_raw.fr,
                    "en": p.text_raw.en,
                    "ja": p.text_raw.ja,
                },
            })
        })
        .collect();

    let team_arr: Vec<Value> = db
        .team
        .iter()
        .map(|t| {
            json!({
                "team_passive_id": t.team_passive_id.to_string(),
                "effect_id": t.effect_id.to_string(),
                "value_min": t.effect_value_min,
                "value_max": t.effect_value_max,
                "text_id": t.text_id.to_string(),
                "text": {
                    "fr": t.text.fr,
                    "en": t.text.en,
                    "ja": t.text.ja,
                },
            })
        })
        .collect();

    let lots_arr: Vec<Value> = db
        .lots
        .iter()
        .map(|l| {
            json!({
                "id": l.id.to_string(),
                "lot_weight": l.lot_weight,
                "condition": l.condition,
                "rarity_enable": l.rarity_enable_flag,
            })
        })
        .collect();

    json!({
        "meta": {
            "player_count": db.player.len(),
            "team_count": db.team.len(),
            "lot_count": db.lots.len(),
            "unique_effect_count": db.unique_effect_count,
            "unique_string_id_count": db.unique_string_id_count,
            "schema_version": 1,
            "source": "nie-data/passives.rs via export_passives bin",
        },
        "player": player_arr,
        "team": team_arr,
        "lots": lots_arr,
    })
}
