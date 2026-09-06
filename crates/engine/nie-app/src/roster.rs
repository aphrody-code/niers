//! Pont DONNÉES → équipe (unification, Phase 2 — comble la fracture #2).
//!
//! Charge de VRAIS paramètres de personnage (`chara_param_*.cfg.bin.json`, parsés par `nie-data`) et
//! construit une [`TeamSetup`] via `nie_core::match_sim::TeamSetup::from_chara_params_and_levels`
//! (qui calcule les stats par la vraie courbe de croissance). Fin des `StatBlock` codés en dur : le
//! match tourne enfin sur les données du jeu.
//!
//! Le ROSTER exact d'une équipe (quels 11 personnages) vient à terme de `team_config`/`nie-wiki`
//! `get_team` ; ici on prend les `n` premiers personnages réels (données réelles, équipe non
//! canonique) — premier câblage nie-data → loop de match.

use std::path::Path;

use anyhow::{Context, Result};
use nie_core::match_sim::TeamSetup;
use nie_data::chara_param::parse_all_chara_params;

/// Construit une équipe de `n` joueurs réels au niveau `level` depuis un `chara_param_*.cfg.bin.json`.
///
/// Renvoie une erreur si le fichier est illisible ou ne contient aucun personnage.
pub fn team_from_chara_param_json(
    name: &str,
    json_path: &Path,
    n: usize,
    level: u8,
) -> Result<TeamSetup> {
    let txt =
        std::fs::read_to_string(json_path).with_context(|| format!("lecture {json_path:?}"))?;
    let root: serde_json::Value =
        serde_json::from_str(&txt).context("JSON chara_param invalide")?;
    let all = parse_all_chara_params(&root);
    anyhow::ensure!(!all.is_empty(), "aucun CharaParam dans {json_path:?}");
    let take = n.min(all.len()).max(1);
    let players: Vec<(&nie_data::chara_param::CharaParam, u8)> =
        all.iter().take(take).map(|cp| (cp, level)).collect();
    Ok(TeamSetup::from_chara_params_and_levels(name, &players))
}
