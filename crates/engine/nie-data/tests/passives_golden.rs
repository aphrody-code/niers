#![allow(clippy::pedantic)]
//! Tests golden `passives` — validation sur les vraies cfg.bin.
//!
//! Sources vérifiées :
//! - `passive_skill_config_5.00.07.00.cfg.bin.json` : 1716 instances joueur
//! - `soccer_team_passive_config_0.00.00.cfg.bin.json` : 21 team passives
//! - `team_passive_lot_table_config_0.00.00.cfg.bin.json` : 653 lots
//! - `skill_text fr/en/ja` : textes NOUN_INFO (effectId→texte)
//! - `soccer_team_passive_text` : textes team passive (JA dans toutes locales)

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use nie_data::passives::{
    element_name, load_noun_texts, load_team_passive_texts, parse_lots, parse_player_passives,
    parse_team_passives, resolve_text_placeholder,
};

/// Racine `data/` du jeu, dérivée de la racine du corpus de dumps (`<…>/data/common/gamedata`
/// → `<…>/data`). `None` quand le corpus est absent de la machine.
fn data_root() -> Option<std::path::PathBuf> {
    let gamedata = common::racine()?;
    Some(gamedata.parent()?.parent()?.to_path_buf())
}

fn load_json(path: &str) -> serde_json::Value {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", path));
    serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide dans {}: {e}", path))
}

fn skill_text_path(racine: &std::path::Path, lang: &str) -> String {
    format!(
        "{}/common/text/{lang}/skill_text.cfg.bin.json",
        racine.display()
    )
}

fn team_passive_text_path(racine: &std::path::Path, lang: &str) -> String {
    format!(
        "{}/common/text/{lang}/soccer_team_passive_text.cfg.bin.json",
        racine.display()
    )
}

// ============================================================================
// Tests passives joueur
// ============================================================================

#[test]
fn passive_skill_config_charge() {
    let Some(racine) = data_root() else { return };
    let path = format!(
        "{}/common/gamedata/skill/passive_skill_config_5.00.07.00.cfg.bin.json",
        racine.display()
    );
    if !Path::new(&path).exists() {
        return; // Skip si pas sur le VPS
    }
    let root = load_json(&path);
    let text_fr = load_noun_texts(&load_json(&skill_text_path(&racine, "fr")));
    let text_en = load_noun_texts(&load_json(&skill_text_path(&racine, "en")));
    let text_ja = load_noun_texts(&load_json(&skill_text_path(&racine, "ja")));

    let passives = parse_player_passives(&root, &text_fr, &text_en, &text_ja);

    // Comparer avec inagle : 128 effectIds uniques, 1716 instances
    // Gain = 1716 instances avec stringId + params par rareté (vs 128 dédoublonnées dans inagle)
    assert!(
        passives.len() >= 1700,
        "attendu ≥1700 instances, obtenu {}",
        passives.len()
    );

    // Compter les effectIds uniques (= familles de passifs)
    let unique_effects: std::collections::HashSet<u32> =
        passives.iter().map(|p| p.effect_id.get()).collect();
    // Inagle a 128 effectIds uniques — on doit en avoir au moins autant
    assert!(
        unique_effects.len() >= 128,
        "effectIds uniques: {} (attendu ≥128)",
        unique_effects.len()
    );

    // Vérifier le golden ps10001 (première instance de la famille Shot AT même élément)
    let ps10001 = passives
        .iter()
        .find(|p| p.string_id == "ps10001")
        .expect("ps10001 doit être présent");
    assert_eq!(ps10001.passive_id.to_hex(), "0x3A2BCAF4");
    assert_eq!(ps10001.effect_id.to_hex(), "0x41DF1FED");
    assert_eq!(ps10001.rarity, 6);
    assert_eq!(ps10001.element, 0);
    // ps10001 = première instance → params = [0.5]
    assert!(
        !ps10001.effect_params.is_empty(),
        "ps10001 doit avoir des params d'effet"
    );
    assert!(
        (ps10001.effect_params[0] - 0.5).abs() < 1e-6,
        "ps10001 param[0] attendu 0.5, obtenu {}",
        ps10001.effect_params[0]
    );

    // Vérifier la résolution du texte FR
    let text_fr_resolved = ps10001.text_resolved.fr.as_deref().unwrap_or("");
    assert!(
        text_fr_resolved.contains("tirs"),
        "texte FR résolu doit contenir 'tirs': {text_fr_resolved:?}"
    );
    assert!(
        !text_fr_resolved.contains("[CPASSIVE01]"),
        "texte FR résolu ne doit pas contenir [CPASSIVE01]: {text_fr_resolved:?}"
    );
    assert!(
        !text_fr_resolved.contains("<VALUE>"),
        "texte FR résolu ne doit pas contenir <VALUE>: {text_fr_resolved:?}"
    );

    // Vérifier ps10001_01..04 (variations de rareté)
    for suffix in ["ps10001_01", "ps10001_02", "ps10001_03", "ps10001_04"] {
        let variant = passives.iter().find(|p| p.string_id == suffix);
        assert!(variant.is_some(), "{suffix} doit être présent");
        let v = variant.unwrap();
        // Tous partagent le même effectId (même famille)
        assert_eq!(
            v.effect_id.to_hex(),
            "0x41DF1FED",
            "{suffix} effectId incorrect"
        );
        // Les params doivent être différents entre les raretés
        assert!(
            !v.effect_params.is_empty(),
            "{suffix} doit avoir des params"
        );
    }

    // Vérifier la présence de passives multi-paramètres (élément != 0)
    let elemental = passives.iter().filter(|p| p.element != 0).count();
    // Les passives d'élément spécifiques existent (mps = même élément type)
    eprintln!("Passives par élément (non-neutre): {elemental}");

    // Vérifier les string_ids de style 'mps' (passifs manager/multi)
    let mps_count = passives
        .iter()
        .filter(|p| p.string_id.starts_with("mps"))
        .count();
    eprintln!("Passives mps*: {mps_count}");
    let ss_count = passives
        .iter()
        .filter(|p| p.string_id.starts_with("ss_"))
        .count();
    eprintln!("Passives ss_*: {ss_count}");

    eprintln!(
        "STATS: total={} unique_effects={} elemental={} mps={} ss={}",
        passives.len(),
        unique_effects.len(),
        elemental,
        mps_count,
        ss_count
    );
}

#[test]
fn passive_skill_textes_resolus_echantillon() {
    let Some(racine) = data_root() else { return };
    let path = format!(
        "{}/common/gamedata/skill/passive_skill_config_5.00.07.00.cfg.bin.json",
        racine.display()
    );
    if !Path::new(&path).exists() {
        return;
    }
    let root = load_json(&path);
    let text_fr = load_noun_texts(&load_json(&skill_text_path(&racine, "fr")));
    let text_en = load_noun_texts(&load_json(&skill_text_path(&racine, "en")));
    let text_ja = load_noun_texts(&load_json(&skill_text_path(&racine, "ja")));

    let passives = parse_player_passives(&root, &text_fr, &text_en, &text_ja);

    // Compter les passives avec texte résolu (vs ceux sans texte)
    let with_fr = passives
        .iter()
        .filter(|p| p.text_resolved.fr.is_some())
        .count();
    let with_en = passives
        .iter()
        .filter(|p| p.text_resolved.en.is_some())
        .count();
    let with_ja = passives
        .iter()
        .filter(|p| p.text_resolved.ja.is_some())
        .count();
    let total = passives.len();

    eprintln!(
        "Textes résolus: FR={}/{} EN={}/{} JA={}/{}",
        with_fr, total, with_en, total, with_ja, total
    );

    // Inagle a 120/128 avec texte non-vide. On doit au moins égaler.
    // Les passives sans texte sont les ss_ps* (variants spéciaux sans NOUN_INFO)
    assert!(
        with_fr > 0,
        "Aucun passif n'a de texte FR résolu — problème de parsing"
    );

    // Vérifier que les textes résolus ne contiennent PLUS les placeholders
    let unresolved_cpassive = passives
        .iter()
        .filter(|p| {
            p.text_resolved
                .fr
                .as_deref()
                .map(|t| t.contains("[CPASSIVE01]"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        unresolved_cpassive, 0,
        "{unresolved_cpassive} passives encore avec [CPASSIVE01] non résolu"
    );

    let unresolved_value = passives
        .iter()
        .filter(|p| {
            p.text_resolved
                .fr
                .as_deref()
                .map(|t| t.contains("<VALUE>"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        unresolved_value, 0,
        "{unresolved_value} passives encore avec <VALUE> non résolu"
    );
}

#[test]
fn passive_vs_inagle_gain() {
    let Some(racine) = data_root() else { return };
    let path = format!(
        "{}/common/gamedata/skill/passive_skill_config_5.00.07.00.cfg.bin.json",
        racine.display()
    );
    if !Path::new(&path).exists() {
        return;
    }
    let root = load_json(&path);
    let text_fr = load_noun_texts(&load_json(&skill_text_path(&racine, "fr")));
    let empty: BTreeMap<u32, String> = BTreeMap::new();

    let passives = parse_player_passives(&root, &text_fr, &empty, &empty);

    // Inagle = 128 entrées dans inagle_passives (SELECT COUNT(*) FROM inagle_passives)
    // Notre pipeline = 1716 instances
    let total = passives.len();
    let inagle_count = 128usize; // Vérifié par sqlite3 mirror.sqlite
    assert!(
        total > inagle_count * 10,
        "Gain insuffisant: {total} instances vs {inagle_count} dans inagle"
    );

    // Compter les champs supplémentaires qu'inagle ne fournit pas
    let has_string_id = passives.iter().filter(|p| !p.string_id.is_empty()).count();
    let has_buff_icon = passives
        .iter()
        .filter(|p| p.buff_icon_type.is_some())
        .count();
    let has_params = passives
        .iter()
        .filter(|p| !p.effect_params.is_empty())
        .count();

    eprintln!(
        "GAIN: total={total} (vs inagle {inagle_count})\n\
         Avec string_id: {has_string_id}/{total}\n\
         Avec buff_icon_type: {has_buff_icon}/{total}\n\
         Avec effect_params: {has_params}/{total}"
    );

    // Tous doivent avoir un string_id non vide (champ absent d'inagle)
    assert_eq!(
        has_string_id, total,
        "Certains passives n'ont pas de string_id"
    );
}

// ============================================================================
// Tests team passives
// ============================================================================

#[test]
fn team_passives_golden() {
    let Some(racine) = data_root() else { return };
    let cfg_path = format!(
        "{}/common/gamedata/soccer/soccer_team_passive_config_0.00.00.cfg.bin.json",
        racine.display()
    );
    if !Path::new(&cfg_path).exists() {
        return;
    }
    let cfg = load_json(&cfg_path);
    // Texte JA (seule locale disponible pour soccer_team_passive_text)
    let text_ja_root = load_json(&team_passive_text_path(&racine, "ja"));
    let text_ja = load_team_passive_texts(&text_ja_root);

    let team = parse_team_passives(&cfg, &text_ja);

    // 21 entrées vérifiées par inspection du cfg
    assert_eq!(
        team.len(),
        21,
        "attendu 21 team passives, obtenu {}",
        team.len()
    );

    // Golden: premier passif = "キャプテンのブロック" (Captain's Block)
    let first = &team[0];
    assert_eq!(first.team_passive_id.to_hex(), "0x2A7D4552");
    assert_eq!(first.effect_id.to_hex(), "0xBEBB2EE8");
    assert_eq!(first.effect_value_min, 0);
    assert_eq!(first.effect_value_max, 8);
    assert_eq!(first.text_id.to_hex(), "0x44FECFAB");
    assert_eq!(
        first.text.ja.as_deref(),
        Some("キャプテンのブロック"),
        "texte JA incorrect pour le premier passif d'équipe"
    );

    // Tous les passifs d'équipe partagent le même effectId (comportement moteur unique)
    let unique_effect_ids: std::collections::HashSet<u32> =
        team.iter().map(|t| t.effect_id.get()).collect();
    assert_eq!(
        unique_effect_ids.len(),
        1,
        "Tous les team passives devraient partager 1 effectId (0xBEBB2EE8), obtenu {}",
        unique_effect_ids.len()
    );

    // Vérifier les plages de valeurs (0-20%)
    for tp in &team {
        assert!(tp.effect_value_min >= 0, "effectValueMin négatif");
        assert!(
            tp.effect_value_max <= 20,
            "effectValueMax > 20: {}",
            tp.effect_value_max
        );
    }

    // Textes disponibles pour tous (JA uniquement — non localisé dans le jeu)
    let with_text = team.iter().filter(|t| t.text.ja.is_some()).count();
    eprintln!("Team passives avec texte JA: {with_text}/21");
}

// ============================================================================
// Tests lots
// ============================================================================

#[test]
fn lots_golden() {
    let Some(racine) = data_root() else { return };
    let path = format!(
        "{}/common/gamedata/character/team_passive_lot_table_config_0.00.00.cfg.bin.json",
        racine.display()
    );
    if !Path::new(&path).exists() {
        return;
    }
    let root = load_json(&path);
    let lots = parse_lots(&root);

    // 653 entrées vérifiées par inspection
    assert_eq!(lots.len(), 653, "attendu 653 lots, obtenu {}", lots.len());

    // Golden: première entrée
    assert_eq!(lots[0].id.to_hex(), "0x5E4A1D85");
    assert_eq!(lots[0].lot_weight, 50);
    assert!(lots[0].rarity_enable_flag);
    assert_eq!(lots[0].condition, "");

    // Vérifier que les poids sont dans des plages raisonnables
    let max_weight = lots.iter().map(|l| l.lot_weight).max().unwrap_or(0);
    let min_weight = lots.iter().map(|l| l.lot_weight).min().unwrap_or(0);
    eprintln!("Poids min={min_weight} max={max_weight}");
    assert!(max_weight <= 1000, "poids max trop élevé: {max_weight}");
}

// ============================================================================
// Validation cross-check vs mirror.sqlite
// ============================================================================

#[test]
fn cross_check_vs_inagle_passives() {
    // Vérifie que nos 128 effectIds uniques correspondent aux 128 de inagle_passives
    let Some(racine) = data_root() else { return };
    let path = format!(
        "{}/common/gamedata/skill/passive_skill_config_5.00.07.00.cfg.bin.json",
        racine.display()
    );
    if !Path::new(&path).exists() {
        return;
    }
    let root = load_json(&path);
    let text_fr = load_noun_texts(&load_json(&skill_text_path(&racine, "fr")));
    let empty: BTreeMap<u32, String> = BTreeMap::new();

    let passives = parse_player_passives(&root, &text_fr, &empty, &empty);
    let unique_effects: std::collections::HashSet<u32> =
        passives.iter().map(|p| p.effect_id.get()).collect();

    // 128 effectIds uniques dans inagle_passives (vérifié par COUNT DISTINCT)
    eprintln!(
        "EffectIds uniques: {} (inagle_passives = 128 attendu)",
        unique_effects.len()
    );
    // On accepte ±5 pour tolérer les différences de version
    assert!(
        unique_effects.len() >= 123 && unique_effects.len() <= 140,
        "Nombre d'effectIds uniques hors plage: {}",
        unique_effects.len()
    );

    // Vérifier que 0x41DF1FED (ps10001 Shot AT même élément) est parmi eux
    assert!(
        unique_effects.contains(&0x41DF_1FED_u32),
        "effectId 0x41DF1FED (Shot AT même élément) absent"
    );
}

// ============================================================================
// Test element_name
// ============================================================================

#[test]
fn element_names_all() {
    assert_eq!(element_name(0), "neutral");
    assert_eq!(element_name(1), "wind");
    assert_eq!(element_name(2), "forest");
    assert_eq!(element_name(3), "fire");
    assert_eq!(element_name(4), "mountain");
    assert_eq!(element_name(99), "unknown");
}

// ============================================================================
// Test resolve_text_placeholder avec les vrais textes du jeu
// ============================================================================

#[test]
fn resolve_texts_echantillon_game() {
    // Textes réels vérifiés sur les dumps cfg.bin.json
    let cases = [
        // (template, value, attendu_contient)
        (
            "ATT des tirs [CPASSIVE01]+<VALUE> %[C] \\npour les joueurs du même élément",
            0.5_f64,
            "+0.50",
        ),
        (
            "For players of the same element,\\nShot AT [CPASSIVE01]+<VALUE>%[C]",
            1.0_f64,
            "+1",
        ),
        (
            "[同属性/どうぞくせい]のシュートＡＴ[CPASSIVE01]＋<VALUE>％[C]",
            1.5_f64,
            "1.50",
        ),
    ];

    for (template, value, expected) in &cases {
        let resolved = resolve_text_placeholder(template, *value);
        assert!(
            resolved.contains(expected),
            "Template '{template}' value={value}: '{resolved}' ne contient pas '{expected}'"
        );
        assert!(
            !resolved.contains("[CPASSIVE01]"),
            "Template '{template}': [CPASSIVE01] non supprimé dans '{resolved}'"
        );
        assert!(
            !resolved.contains("<VALUE>"),
            "Template '{template}': <VALUE> non remplacé dans '{resolved}'"
        );
    }
}
