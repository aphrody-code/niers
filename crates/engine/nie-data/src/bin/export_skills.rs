//! Exporte les **hissatsu + leurs assets de cut-in** vers `apps/azalee/data/skills-cutin.json`.
//!
//! Pour chaque skill du `skill_config`, émet l'identité (élément/catégorie/puissance/TP…) et
//! les chemins VFS des assets de cut-in dérivés ([`SkillInfo::cutin_assets`]) : modèle 3D
//! `chr/_waza/<ev>/<ev>.g4mg`, texture, son `event_cfg/snd/<ev>_snd.cfg.bin`, effets, telop
//! par langue. Permet à la page skill d'azalee d'afficher le cut-in 3D, le telop et le son —
//! données déjà servies live par `nie-model-serve` mais absentes du miroir azalee.
//!
//! # Usage
//! ```text
//! cargo run --bin export_skills --features serde,std -- \
//!     --data $NIE_GAME_DIR/data --out export/skills-cutin.json
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use nie_data::skill::parse_skill_config;
use serde_json::{Value, json};

/// Racine des données décodées, sans chemin de poste en dur : `NIE_GAME_DIR/data` si la
/// variable est posée, sinon `./data` (le dépôt est fusionné avec l'installation du jeu).
fn default_data_root() -> String {
    std::env::var("NIE_GAME_DIR")
        .map(|d| format!("{d}/data"))
        .unwrap_or_else(|_| "data".to_string())
}

fn main() {
    let data_root = std::env::args()
        .skip_while(|a| a != "--data")
        .nth(1)
        .unwrap_or_else(default_data_root);
    let out_path = std::env::args()
        .skip_while(|a| a != "--out")
        .nth(1)
        .unwrap_or_else(|| format!("{data_root}/../export/skills-cutin.json"));

    let data_root = PathBuf::from(&data_root);
    let cfg_path = find_cfg(&data_root, "common/gamedata/skill", "skill_config_");
    eprintln!("[export_skills] skill_config → {cfg_path:?}");
    let root = read_json(&cfg_path);
    let skills = parse_skill_config(&root);
    eprintln!("[export_skills] {} skills parsés", skills.len());

    let mut with_cutin = 0usize;
    let arr: Vec<Value> = skills
        .iter()
        .map(|s| {
            let (el_fr, el_en, el_ja) = s.element().names().unwrap_or(("", "", ""));
            let (cat_fr, cat_en, cat_ja) = s.category().names().unwrap_or(("", "", ""));
            let cutin = s.cutin_assets();
            if cutin.is_some() {
                with_cutin += 1;
            }
            json!({
                "skill_id_str": s.skill_id_str,
                "skill_id": s.skill_id.to_hex(),
                "event_id_name": s.event_id_name,
                "element": s.element,
                "element_name": { "fr": el_fr, "en": el_en, "ja": el_ja },
                "category": s.category,
                "category_name": { "fr": cat_fr, "en": cat_en, "ja": cat_ja },
                "power_min": s.power_min,
                "power_max": s.power_max,
                "consume_tp": s.consume_tp,
                "foul_rate": s.foul_rate,
                "growth_type": s.growth_type,
                "recast_time": s.recast_time,
                "partner_type": s.partner_type,
                "cutin": cutin,
            })
        })
        .collect();

    let out = json!({
        "meta": {
            "skill_count": skills.len(),
            "cutin_count": with_cutin,
            "schema_version": 1,
            "source": "nie-data/skill.rs (cutin_assets) via export_skills bin",
        },
        "skills": arr,
    });

    let bytes = serde_json::to_vec_pretty(&out).expect("sérialisation JSON impossible");
    if let Some(parent) = Path::new(&out_path).parent() {
        fs::create_dir_all(parent).expect("création dossier de sortie");
    }
    fs::write(&out_path, &bytes).expect("écriture JSON");
    let kb = bytes.len() / 1024;
    eprintln!(
        "[export_skills] OK — {} skills ({with_cutin} avec cut-in), {kb}Ko → {out_path}",
        skills.len()
    );
    println!(
        "skill_count={} cutin_count={with_cutin} size_kb={kb}",
        skills.len()
    );
}

/// Trouve le `*.cfg.bin.json` le plus récent de `subdir` commençant par `prefix`.
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
    assert!(
        !candidates.is_empty(),
        "[export_skills] introuvable : {subdir}/{prefix}*.cfg.bin.json"
    );
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    candidates.remove(0)
}

/// Lit et parse un fichier JSON.
fn read_json(path: &Path) -> Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("[export_skills] lecture {path:?}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("[export_skills] parse {path:?}: {e}"))
}
