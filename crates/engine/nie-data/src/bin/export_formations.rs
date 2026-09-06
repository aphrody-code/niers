//! Exporte les **115 formations réelles** d'IEVR vers `apps/azalee/data/formations-full.json`.
//!
//! Remplace (côté azalee) les positions approximées codées en dur de `lib/formations.ts`
//! (estimées à l'œil depuis le CSS de zukan.inazuma.jp) par les VRAIES coordonnées `f32`
//! du jeu (`start_pos`/`offense_pos`/`defense_pos` byte-exactes décodées par `formation.rs`).
//!
//! # Usage
//!
//! ```text
//! cargo run --bin export_formations --features serde,std -- \
//!     --data $NIE_GAME_DIR/data \
//!     --out export/formations-full.json
//! ```
//!
//! # Source lue
//!
//! - `common/gamedata/formation/formation_config_*.cfg.bin.json` (115 formations, 1073 placements).

use std::fs;
use std::path::{Path, PathBuf};

use nie_data::formation::{SoccerFormPlacementInfo, parse_formation_config};
use serde_json::{Value, json};

/// `position_id` (1..10, type de poste) → rôle, dérivé des poids offensif/défensif de
/// `m_SoccerPositionInfoList` : id 1 = GK (def 0.95) ; 2-3 = DF (def 0.8-0.9) ;
/// 4-7 = MF (def 0.4-0.6) ; 8-10 = FW (off 0.65-0.8). Vérifié : formation 3-4-3 etc.
fn role_of(position_id: i64) -> &'static str {
    match position_id {
        1 => "GK",
        2 | 3 => "DF",
        4..=7 => "MF",
        8..=10 => "FW",
        _ => "?",
    }
}

/// Étiquette football `DF-MF-FW` dérivée des rôles des joueurs (le GK est implicite).
fn formation_label(placements: &[SoccerFormPlacementInfo]) -> String {
    let mut df = 0;
    let mut mf = 0;
    let mut fw = 0;
    for p in placements {
        match role_of(p.position_id) {
            "DF" => df += 1,
            "MF" => mf += 1,
            "FW" => fw += 1,
            _ => {}
        }
    }
    format!("{df}-{mf}-{fw}")
}

/// Une formation est « valide » (jouable telle quelle dans le builder d'équipe) si elle a
/// exactement 11 joueurs dont 1 GK et aucun poste indéfini (`position_id = 0`, slot vide).
fn is_valid(placements: &[SoccerFormPlacementInfo]) -> bool {
    placements.len() == 11
        && placements.iter().filter(|p| p.position_id == 1).count() == 1
        && placements.iter().all(|p| (1..=10).contains(&p.position_id))
}

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
        .unwrap_or_else(|| format!("{data_root}/../export/formations-full.json"));

    let data_root = PathBuf::from(&data_root);
    eprintln!("[export_formations] data={data_root:?}  out={out_path:?}");

    let cfg_path = find_cfg(&data_root, "common/gamedata/formation", "formation_config_");
    eprintln!("[export_formations] formation_config → {cfg_path:?}");
    let root = read_json(&cfg_path);
    let cfg = parse_formation_config(&root);
    eprintln!(
        "[export_formations] parsé : {} formations, {} placements, {} positions",
        cfg.formations.len(),
        cfg.placements.len(),
        cfg.positions.len()
    );

    let formations: Vec<Value> = cfg
        .formations
        .iter()
        .map(|f| {
            let placements = cfg.placements_of(f);
            let label = formation_label(placements);
            let positions: Vec<Value> = placements
                .iter()
                .map(|p| {
                    json!({
                        "position_no": p.position_no,
                        "position_id": p.position_id,
                        "role": role_of(p.position_id),
                        "pass_no": p.pass_no,
                        "b_kickoff": p.b_kickoff,
                        "start": { "x": p.start_pos.x, "y": p.start_pos.y },
                        "offense": { "x": p.offense_pos.x, "y": p.offense_pos.y },
                        "defense": { "x": p.defense_pos.x, "y": p.defense_pos.y },
                    })
                })
                .collect();
            json!({
                "form_id": f.form_id.to_hex(),
                "label": label,
                "valid": is_valid(placements),
                "power_offense": f.power_offense,
                "power_defense": f.power_defense,
                "noun_id": f.noun_id.to_hex(),
                "desc_id": f.desc_id.to_hex(),
                "positions": positions,
            })
        })
        .collect();

    let valid_count = cfg
        .formations
        .iter()
        .filter(|f| is_valid(cfg.placements_of(f)))
        .count();

    let out = json!({
        "meta": {
            "formation_count": cfg.formations.len(),
            "valid_count": valid_count,
            "placement_count": cfg.placements.len(),
            "schema_version": 1,
            "source": "nie-data/formation.rs via export_formations bin",
            "coords": "x,y f32 jeu (start/offense/defense) ; x=0 centre, y croît vers le but propre (GK y≈0.96)",
        },
        "formations": formations,
    });

    let json_bytes = serde_json::to_vec_pretty(&out).expect("sérialisation JSON impossible");
    if let Some(parent) = Path::new(&out_path).parent() {
        fs::create_dir_all(parent).expect("impossible de créer le dossier de sortie");
    }
    fs::write(&out_path, &json_bytes).expect("impossible d'écrire le fichier JSON");

    let kb = json_bytes.len() / 1024;
    eprintln!(
        "[export_formations] OK — {} formations, {kb}Ko → {out_path}",
        cfg.formations.len()
    );
    println!(
        "formation_count={} size_kb={kb} out={out_path}",
        cfg.formations.len()
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
        "[export_formations] introuvable : {subdir}/{prefix}*.cfg.bin.json (dans {dir:?})"
    );
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    candidates.remove(0)
}

/// Lit et parse un fichier JSON.
fn read_json(path: &Path) -> Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("[export_formations] lecture {path:?}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("[export_formations] parse {path:?}: {e}"))
}
