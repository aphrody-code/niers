//! Rendu ASCII des profils game-data (personnages, skills, items, équipes).
//!
//! Reproduit fidèlement les fonctions `render*Profile` de azalee.ts :
//! - `renderCharaProfile` → `render_chara_profile`
//! - `renderSkillProfile` → `render_skill_profile`
//! - `renderItemProfile` → `render_item_profile`
//! - `renderTeamProfile` → `render_team_profile`
//!
//! Gestion de la largeur visible pour les caractères JP (CJK comptent double).

use rusqlite::Connection;

use crate::{
    model::{
        AuditReport, AuditTable, CharaProfile, CompareResult, DialogueMatch, ItemProfile,
        RandomTeam, RandomTeamPlayer, SearchResult, SkillProfile, StatusReport, TeamBuildEntry,
        TeamProfile,
    },
    query::skill_name_by_id,
};

// ─── Largeur visible ──────────────────────────────────────────────────────────

/// Calcule la largeur visuelle d'une chaîne en terminal.
///
/// Les caractères CJK (pleine largeur) comptent pour 2.
/// Identique à `getVisibleLength` dans azalee.ts.
pub fn visible_len(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let cp = c as u32;
            if (0x3000..=0x9FFF).contains(&cp)
                || (0xF900..=0xFAFF).contains(&cp)
                || (0xFF00..=0xFFEF).contains(&cp)
            {
                2
            } else {
                1
            }
        })
        .sum()
}

/// Pad une chaîne à une largeur visible donnée avec des espaces.
fn pad_to(s: &str, width: usize) -> String {
    let vl = visible_len(s);
    if vl >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - vl))
    }
}

/// Ligne pleine largeur (78 chars de contenu, bordures `│`).
fn one_col(text: &str) -> String {
    // Contenu entre les barres = 78 chars visibles
    let content_width = 78;
    let vl = visible_len(text);
    let pad = if vl >= content_width {
        String::new()
    } else {
        " ".repeat(content_width - vl)
    };
    format!("│ {}{} │", text, pad)
}

/// Ligne deux colonnes (contenu total 78 = 40 + 1 séparateur + 37).
fn two_col(left: &str, right: &str) -> String {
    let left_w = 40;
    let right_w = 35;
    let l = pad_to(left, left_w);
    let r = pad_to(right, right_w);
    format!("│ {}│ {} │", l, r)
}

const TOP: &str =
    "┌──────────────────────────────────────────────────────────────────────────────┐";
const MID: &str =
    "├──────────────────────────────────────────────────────────────────────────────┤";
const BOT: &str =
    "└──────────────────────────────────────────────────────────────────────────────┘";

// ─── Catégories d'items ───────────────────────────────────────────────────────

fn item_category_label(cat: &str) -> &str {
    match cat {
        "consume" => "Consommable",
        "shoes" => "Chaussures",
        "misanga" => "Bracelet",
        "accessory" => "Accessoire",
        "special" => "Special",
        "formation" => "Formation",
        "special_tactics" => "Tactique speciale",
        "super_tactics" => "Super tactique",
        "special_skill" => "Competence speciale",
        "title" => "Titre",
        "fashion" => "Mode",
        "costume" => "Costume",
        "emblem" => "Embleme",
        "unique" => "Unique",
        "craft_obj" => "Materiau",
        "animal" => "Animal",
        "kizuna_link" => "Lien Kizuna",
        "name_plate" => "Plaque",
        "performance" => "Performance",
        "important" => "Objet cle",
        _ => cat,
    }
}

// ─── Personnage ───────────────────────────────────────────────────────────────

/// Rendu ASCII complet d'un profil de personnage.
pub fn render_chara_profile(chara: &CharaProfile, conn: &Connection) -> String {
    let mut lines: Vec<String> = Vec::new();

    let name_fr = chara.name_fr.as_deref().unwrap_or("N/A");
    let name_en = chara.name_en.as_deref().unwrap_or("N/A");
    let name_ja = chara.name_ja.as_deref().unwrap_or("N/A");
    let element = chara.element.as_deref().unwrap_or("N/A");
    let position = chara.position.as_deref().unwrap_or("N/A");
    let rarity = chara.rarity_label.as_deref().unwrap_or("N/A");
    let internal_code = chara.internal_code.as_deref().unwrap_or("N/A");

    lines.push(TOP.to_string());
    lines.push(two_col(
        &format!("{} ({})", name_fr, rarity),
        &format!("ID: {}", chara.id),
    ));
    lines.push(two_col(
        &format!("EN: {}", name_en),
        &format!("JA: {}", name_ja),
    ));
    lines.push(two_col(
        &format!("Element: {}  Poste: {}", element, position),
        &format!("Code: {}", internal_code),
    ));
    lines.push(MID.to_string());

    if let Some(team) = &chara.team_name {
        lines.push(one_col(&format!("Equipe : {}", team)));
    }
    if let Some(series) = &chara.series {
        lines.push(one_col(&format!("Serie  : {}", series)));
    }

    // Description
    if let Some(desc) = &chara.description_fr {
        lines.push(MID.to_string());
        lines.push(one_col("Description :"));
        for line in word_wrap(desc, 74) {
            lines.push(one_col(&format!("  {}", line)));
        }
    }

    // Courbe de progression des stats
    let lv1 = chara.stats_lv1;
    let lv50 = chara.stats_lv50;
    let lv99 = chara.stats_lv99;

    if let (Some(s1), Some(s50), Some(s99)) = (lv1, lv50, lv99) {
        lines.push(MID.to_string());
        lines.push(one_col("Courbe de progression des statistiques :"));
        lines.push(one_col("  ┌──────────────┬─────────┬─────────┬─────────┐"));
        lines.push(one_col("  │ Stat         │ Niv. 1  │ Niv. 50 │ Niv. 99 │"));
        lines.push(one_col("  ├──────────────┼─────────┼─────────┼─────────┤"));

        let stat_row = |label: &str, v1: u16, v50: u16, v99: u16| -> String {
            one_col(&format!(
                "  │ {:<12} │ {:>7} │ {:>7} │ {:>7} │",
                label, v1, v50, v99
            ))
        };
        let t1 = s1.total();
        let t50 = s50.total();
        let t99 = s99.total();

        lines.push(stat_row("Frappe", s1.kick, s50.kick, s99.kick));
        lines.push(stat_row("Controle", s1.control, s50.control, s99.control));
        lines.push(stat_row(
            "Physique",
            s1.physical,
            s50.physical,
            s99.physical,
        ));
        lines.push(stat_row(
            "Pression",
            s1.pressure,
            s50.pressure,
            s99.pressure,
        ));
        lines.push(stat_row(
            "Technique",
            s1.technique,
            s50.technique,
            s99.technique,
        ));
        lines.push(stat_row("Agilite", s1.agility, s50.agility, s99.agility));
        lines.push(stat_row(
            "Intelligence",
            s1.intelligence,
            s50.intelligence,
            s99.intelligence,
        ));
        lines.push(one_col("  ├──────────────┼─────────┼─────────┼─────────┤"));
        lines.push(one_col(&format!(
            "  │ {:<12} │ {:>7} │ {:>7} │ {:>7} │",
            "TOTAL", t1, t50, t99
        )));
        lines.push(one_col("  └──────────────┴─────────┴─────────┴─────────┘"));
    }

    // Techniques
    if !chara.skills.is_empty() {
        lines.push(MID.to_string());
        lines.push(one_col(&format!("Techniques ({}) :", chara.skills.len())));
        for slot in &chara.skills {
            let name = skill_name_by_id(conn, &slot.skill_id);
            lines.push(one_col(&format!(
                "  - {} (Niv. {})",
                name, slot.learn_level
            )));
        }
    }

    // Auras
    if !chara.auras.is_empty() {
        lines.push(MID.to_string());
        lines.push(one_col(&format!(
            "Auras / Keshin / Totem ({}) :",
            chara.auras.len()
        )));
        for aura in &chara.auras {
            let type_label = match aura.aura_type.as_str() {
                "keshin" => "Esprit Guerrier",
                "soul" => "Totem",
                "miximax" => "Miximax",
                other => other,
            };
            lines.push(one_col(&format!("  - {} [{}]", aura.name, type_label)));
        }
    }

    lines.push(BOT.to_string());
    lines.join("\n")
}

// ─── Skill ───────────────────────────────────────────────────────────────────

/// Rendu ASCII d'un profil de technique.
pub fn render_skill_profile(skill: &SkillProfile) -> String {
    let mut lines: Vec<String> = Vec::new();

    let name = skill
        .name_fr
        .as_deref()
        .or(skill.name_en.as_deref())
        .unwrap_or("N/A");
    let id_str = &skill.id;
    let type_str = if skill.is_hyper { "HYPER" } else { "TECHNIQUE" };
    let element = skill.element.as_deref().unwrap_or("N/A");
    let category = skill.category.as_deref().unwrap_or("N/A");

    lines.push(TOP.to_string());
    lines.push(two_col(
        &format!("[{}] {}", type_str, name),
        &format!("ID: {}", id_str),
    ));
    if let (Some(en), Some(ja)) = (&skill.name_en, &skill.name_ja) {
        lines.push(two_col(&format!("EN: {}", en), &format!("JA: {}", ja)));
    }

    // Stats
    let power_max = skill.power_max.unwrap_or(0);
    let power_min = skill.power_min.unwrap_or(0);
    let cost = skill.tp_cost.unwrap_or(0);
    lines.push(MID.to_string());
    lines.push(one_col(&format!(
        "Puissance : {}-{}   Cout TP : {}   Element : {}   Categorie : {}",
        power_min, power_max, cost, element, category
    )));

    // Description
    if let Some(desc) = skill
        .description_fr
        .as_deref()
        .or(skill.description_en.as_deref())
    {
        lines.push(MID.to_string());
        lines.push(one_col("Description :"));
        for line in word_wrap(desc, 74) {
            lines.push(one_col(&format!("  {}", line)));
        }
    }

    lines.push(BOT.to_string());
    lines.join("\n")
}

// ─── Item ────────────────────────────────────────────────────────────────────

/// Rendu ASCII d'un profil d'item.
pub fn render_item_profile(item: &ItemProfile) -> String {
    let mut lines: Vec<String> = Vec::new();

    let name_fr = item.name_fr.as_deref().unwrap_or("N/A");
    let name_en = item.name_en.as_deref().unwrap_or("N/A");
    let name_ja = item.name_ja.as_deref().unwrap_or("N/A");
    let cat = item
        .category
        .as_deref()
        .map(item_category_label)
        .unwrap_or("N/A");
    let code = item.internal_code.as_deref().unwrap_or("N/A");
    let price_str = item
        .price
        .map(|p| format!("{} Ptz", p))
        .unwrap_or_else(|| "N/A".to_string());

    lines.push(TOP.to_string());
    lines.push(two_col(name_fr, &format!("ID: {}", item.id)));
    lines.push(two_col(
        &format!("EN: {}", name_en),
        &format!("JA: {}", name_ja),
    ));
    lines.push(two_col(
        &format!("Code: {}", code),
        &format!("Categorie: {}", cat),
    ));
    lines.push(MID.to_string());
    lines.push(one_col(&format!("Prix : {}", price_str)));

    if !item.shops.is_empty() {
        let shops_str = item.shops.join(", ");
        lines.push(one_col(&format!("Boutiques : {}", shops_str)));
    }

    if let Some(desc) = &item.description_fr {
        lines.push(MID.to_string());
        lines.push(one_col("Description :"));
        for line in word_wrap(desc, 74) {
            lines.push(one_col(&format!("  {}", line)));
        }
    }

    lines.push(BOT.to_string());
    lines.join("\n")
}

// ─── Team ────────────────────────────────────────────────────────────────────

/// Rendu ASCII d'un profil d'équipe.
pub fn render_team_profile(team: &TeamProfile) -> String {
    let mut lines: Vec<String> = Vec::new();

    let name_fr = team.name_fr.as_deref().unwrap_or("N/A");
    let name_en = team.name_en.as_deref().unwrap_or("N/A");
    let name_ja = team.name_ja.as_deref().unwrap_or("N/A");
    let code = team.internal_code.as_deref().unwrap_or("N/A");
    let series = team.series.as_deref().unwrap_or("N/A");
    let region = team.region.as_deref().unwrap_or("");

    lines.push(TOP.to_string());
    lines.push(two_col(name_fr, &format!("ID: {}", team.id)));
    lines.push(two_col(
        &format!("EN: {}", name_en),
        &format!("JA: {}", name_ja),
    ));
    lines.push(two_col(
        &format!("Code: {}", code),
        &format!("Region: {}", region),
    ));
    lines.push(MID.to_string());
    lines.push(one_col(&format!("Serie  : {}", series)));

    if !team.seasons.is_empty() {
        let mut seasons = team.seasons.clone();
        seasons.sort();
        lines.push(one_col(&format!("Saisons: {}", seasons.join(", "))));
    }

    if !team.kits.is_empty() {
        let mut kits = team.kits.clone();
        kits.sort();
        lines.push(MID.to_string());
        lines.push(one_col(&format!("Kits ({}) :", kits.len())));
        for k in &kits {
            lines.push(one_col(&format!("  - {}", k)));
        }
    }

    lines.push(BOT.to_string());
    lines.join("\n")
}

// ─── Utilitaires ─────────────────────────────────────────────────────────────

/// Découpe un texte en lignes de largeur visible maximum `max_w`.
fn word_wrap(text: &str, max_w: usize) -> Vec<String> {
    let mut result = Vec::new();
    for paragraph in text.split('\n') {
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        if words.is_empty() {
            result.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in words {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current, word)
            };
            if visible_len(&candidate) <= max_w {
                current = candidate;
            } else {
                if !current.is_empty() {
                    result.push(current);
                }
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            result.push(current);
        }
    }
    result
}

// ─── Compare ─────────────────────────────────────────────────────────────────

/// Rendu ASCII d'une comparaison de deux personnages côte à côte.
///
/// Reproduit le rendu `compare` TS : 3 colonnes (gauche / label / droite).
pub fn render_compare(result: &CompareResult) -> String {
    let col_l = 35usize;
    let col_m = 16usize;
    let col_r = 35usize;

    let border_top = format!(
        "┌─{}─┬─{}─┬─{}─┐",
        "─".repeat(col_l),
        "─".repeat(col_m),
        "─".repeat(col_r)
    );
    let border_mid = format!(
        "├─{}─┼─{}─┼─{}─┤",
        "─".repeat(col_l),
        "─".repeat(col_m),
        "─".repeat(col_r)
    );
    let border_bot = format!(
        "└─{}─┴─{}─┴─{}─┘",
        "─".repeat(col_l),
        "─".repeat(col_m),
        "─".repeat(col_r)
    );

    let row3 = |left: &str, mid: &str, right: &str| -> String {
        let l = &left[..left.len().min(col_l)];
        let m_text = &mid[..mid.len().min(col_m)];
        let r = &right[..right.len().min(col_r)];
        // centre mid
        let pad_total = col_m.saturating_sub(m_text.len());
        let pad_left = pad_total / 2;
        let pad_right = pad_total - pad_left;
        format!(
            "│ {:<col_l$} │ {}{}{} │ {:>col_r$} │",
            l,
            " ".repeat(pad_left),
            m_text,
            " ".repeat(pad_right),
            r,
        )
    };

    let stat_row = |label: &str, v1: u16, v2: u16| -> String {
        let (s1, s2) = if v1 > v2 {
            (format!("{} (>)", v1), format!("{}", v2))
        } else if v2 > v1 {
            (format!("{}", v1), format!("({}) {}", "<", v2))
        } else {
            (format!("{}", v1), format!("{}", v2))
        };
        row3(&s1, label, &s2)
    };

    let c1 = &result.chara1;
    let c2 = &result.chara2;

    let mut lines = Vec::new();
    lines.push(format!("\n=== COMPARAISON (Niveau {}) ===", result.level));
    lines.push(border_top.clone());
    lines.push(row3(&c1.name, "VS", &c2.name));
    lines.push(border_mid.clone());
    lines.push(row3(
        c1.position.as_deref().unwrap_or("N/A"),
        "Position",
        c2.position.as_deref().unwrap_or("N/A"),
    ));
    lines.push(row3(
        c1.element.as_deref().unwrap_or("N/A"),
        "Element",
        c2.element.as_deref().unwrap_or("N/A"),
    ));
    lines.push(row3(
        c1.rarity.as_deref().unwrap_or("N/A"),
        "Rarete",
        c2.rarity.as_deref().unwrap_or("N/A"),
    ));
    lines.push(border_mid.clone());

    // (libellé, accesseur de stat) — alias pour éviter le type complexe répété.
    type StatDef = (&'static str, fn(&crate::model::StatBlock) -> u16);
    let stat_defs: &[StatDef] = &[
        ("Frappe", |s| s.kick),
        ("Controle", |s| s.control),
        ("Technique", |s| s.technique),
        ("Pression", |s| s.pressure),
        ("Physique", |s| s.physical),
        ("Agilite", |s| s.agility),
        ("Intelligence", |s| s.intelligence),
    ];
    let mut total1: u32 = 0;
    let mut total2: u32 = 0;
    for (label, getter) in stat_defs {
        let v1 = getter(&c1.stats);
        let v2 = getter(&c2.stats);
        total1 += u32::from(v1);
        total2 += u32::from(v2);
        lines.push(stat_row(label, v1, v2));
    }
    lines.push(border_mid.clone());
    lines.push(stat_row("TOTAL", total1 as u16, total2 as u16));
    lines.push(border_mid.clone());

    let max_skills = c1.skills.len().max(c2.skills.len());
    lines.push(row3("", "TECHNIQUES", ""));
    for i in 0..max_skills {
        let s1 = c1
            .skills
            .get(i)
            .map(|sk| {
                if let Some(p) = sk.power {
                    format!("{} (L.{} {}P)", sk.name, sk.learn_level, p)
                } else {
                    format!("{} (L.{})", sk.name, sk.learn_level)
                }
            })
            .unwrap_or_default();
        let s2 = c2
            .skills
            .get(i)
            .map(|sk| {
                if let Some(p) = sk.power {
                    format!("{} (L.{} {}P)", sk.name, sk.learn_level, p)
                } else {
                    format!("{} (L.{})", sk.name, sk.learn_level)
                }
            })
            .unwrap_or_default();
        lines.push(row3(&s1, &format!("Slot {}", i + 1), &s2));
    }

    lines.push(border_bot);
    lines.join("\n")
}

// ─── Search ──────────────────────────────────────────────────────────────────

/// Rendu ASCII de résultats multi-tables.
pub fn render_search_results(results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "Aucun resultat.".to_string();
    }
    let header = format!("{:<10}  {:<30}  {}", "TYPE", "NOM", "ID / EXTRA");
    let sep = "─".repeat(80);
    let mut lines = vec![header, sep.clone()];
    for r in results {
        let extra = r.extra.as_deref().unwrap_or("");
        lines.push(format!(
            "{:<10}  {:<30}  {}{}",
            r.entity_type.to_uppercase(),
            &r.name[..r.name.len().min(30)],
            r.id,
            if extra.is_empty() {
                String::new()
            } else {
                format!(" | {extra}")
            }
        ));
    }
    lines.push(sep);
    lines.push(format!("{} resultat(s)", results.len()));
    lines.join("\n")
}

// ─── Status ──────────────────────────────────────────────────────────────────

/// Rendu ASCII du rapport de statut.
pub fn render_status(report: &StatusReport) -> String {
    let mut lines = Vec::new();
    lines.push("=== Diagnostic niers-wiki ===\n".to_string());

    lines.push("[SQLite]".to_string());
    if report.sqlite.healthy {
        lines.push("  Statut     : OK".to_string());
        lines.push(format!("  Base       : {}", report.sqlite.path));
        if let Some(mb) = report.sqlite.size_mb {
            lines.push(format!("  Taille     : {:.2} MB", mb));
        }
        if let Some(t) = report.sqlite.table_count {
            lines.push(format!("  Tables     : {}", t));
        }
        if let Some(n) = report.sqlite.characters {
            lines.push(format!("  Personnages: {}", n));
        }
        if let Some(n) = report.sqlite.skills {
            lines.push(format!("  Skills     : {}", n));
        }
        if let Some(n) = report.sqlite.items {
            lines.push(format!("  Items      : {}", n));
        }
        if let Some(n) = report.sqlite.teams {
            lines.push(format!("  Equipes    : {}", n));
        }
    } else {
        lines.push("  Statut     : ERREUR".to_string());
        if let Some(e) = &report.sqlite.error {
            lines.push(format!("  Erreur     : {}", e));
        }
    }

    for (label, rs) in [
        ("Redis db0", &report.redis_db0),
        ("Redis db3", &report.redis_db3),
    ] {
        lines.push(format!("\n[{}]", label));
        if rs.healthy {
            lines.push("  Statut  : OK".to_string());
            if let Some(ms) = rs.latency_ms {
                lines.push(format!("  Latence : {:.2} ms", ms));
            }
        } else {
            lines.push("  Statut  : HORS-LIGNE".to_string());
            if let Some(e) = &rs.error {
                lines.push(format!("  Erreur  : {}", e));
            }
        }
    }

    lines.join("\n")
}

// ─── Audit ───────────────────────────────────────────────────────────────────

/// Rendu ASCII du rapport d'audit.
pub fn render_audit(report: &AuditReport) -> String {
    let mut lines = Vec::new();
    lines.push("=== Audit du miroir SQLite ===\n".to_string());
    lines.push(format!(
        "{:<15} {:>8}  {:>12}  {:>12}  {:>10}",
        "Table", "Total", "Sans nom FR", "Sans nom EN", "Null data"
    ));
    lines.push("─".repeat(65));

    let fmt_row = |label: &str, t: &AuditTable| -> String {
        format!(
            "{:<15} {:>8}  {:>12}  {:>12}  {:>10}",
            label, t.total, t.missing_name_fr, t.missing_name_en, t.null_data
        )
    };

    lines.push(fmt_row("characters", &report.characters));
    lines.push(fmt_row("skills", &report.skills));
    lines.push(fmt_row("items", &report.items));
    lines.push(fmt_row("teams", &report.teams));
    lines.push(fmt_row("auras", &report.auras));
    lines.push(fmt_row("keshins", &report.keshins));
    lines.push(fmt_row("souls", &report.souls));
    lines.push("─".repeat(65));

    lines.join("\n")
}

// ─── Dialogue ────────────────────────────────────────────────────────────────

/// Rendu ASCII des résultats de dialogue.
pub fn render_dialogues(matches: &[DialogueMatch], query: &str) -> String {
    if matches.is_empty() {
        return format!("Aucun dialogue trouvé pour \"{}\".", query);
    }
    let mut lines = vec![format!(
        "Dialogues pour \"{}\" ({} resultats) :\n",
        query,
        matches.len()
    )];
    for m in matches {
        lines.push(format!(
            "[{}  l.{}] (ep: {})",
            m.event_id, m.line_index, m.episode
        ));
        if let Some(fr) = &m.text_fr {
            lines.push(format!("  FR: {}", fr));
        }
        if let Some(en) = &m.text_en {
            lines.push(format!("  EN: {}", en));
        }
        lines.push("─".repeat(70));
    }
    lines.join("\n")
}

// ─── Random Team ─────────────────────────────────────────────────────────────

/// Rendu ASCII d'une équipe aléatoire (terrain + stats).
pub fn render_random_team(team: &RandomTeam) -> String {
    let mut lines = Vec::new();

    let title = format!(
        "TERRAIN - FORMATION {} (seed={})",
        team.formation, team.seed
    );
    let w = 78usize;
    let pad = w.saturating_sub(title.len()) / 2;
    lines.push(format!("\n╔{}╗", "═".repeat(w)));
    lines.push(format!(
        "║{}{}{}║",
        " ".repeat(pad),
        title,
        " ".repeat(w.saturating_sub(pad + title.len()))
    ));
    lines.push(format!("╠{}╣", "═".repeat(w)));

    let section_line = |label: &str, players: &[RandomTeamPlayer]| -> String {
        let names: Vec<String> = players
            .iter()
            .map(|p| {
                let el = p.element.as_deref().unwrap_or("Ne");
                let abbr = match el {
                    "Feu" => "Fe",
                    "Vent" => "Ve",
                    "Forêt" => "Fo",
                    "Montagne" => "Mo",
                    _ => "Ne",
                };
                // Tronque le nom si trop long
                let name = if p.name.len() > 14 {
                    format!("{}.", &p.name[..13])
                } else {
                    p.name.clone()
                };
                format!("{} ({})", name, abbr)
            })
            .collect();
        let content = names.join("  |  ");
        let content_w = w.saturating_sub(label.len() + 3);
        let padded = if content.len() < content_w {
            format!("{}{}", content, " ".repeat(content_w - content.len()))
        } else {
            content[..content_w].to_string()
        };
        format!("║ [{}] {}║", label, padded)
    };

    lines.push(section_line("FW", &team.fw));
    lines.push(section_line("MF", &team.mf));
    lines.push(section_line("DF", &team.df));
    lines.push(section_line("GK", &team.gk));

    lines.push(format!("╠{}╣", "═".repeat(w)));

    if let Some(coach) = &team.coach {
        let el = coach.element.as_deref().unwrap_or("N/A");
        lines.push(format!(
            "║ Coach   : {:<30} ({}){}║",
            &coach.name[..coach.name.len().min(30)],
            el,
            " ".repeat(w.saturating_sub(coach.name.len().min(30) + el.len() + 14))
        ));
    }
    for (i, m) in team.managers.iter().enumerate() {
        let el = m.element.as_deref().unwrap_or("N/A");
        lines.push(format!(
            "║ Mgr {}   : {:<30} ({}){}║",
            i + 1,
            &m.name[..m.name.len().min(30)],
            el,
            " ".repeat(w.saturating_sub(m.name.len().min(30) + el.len() + 15))
        ));
    }

    lines.push(format!("╚{}╝", "═".repeat(w)));
    lines.join("\n")
}

// ─── Team Builder ────────────────────────────────────────────────────────────

/// Rendu ASCII d'une liste d'entrées team-builder.
pub fn render_team_build_list(entries: &[TeamBuildEntry]) -> String {
    if entries.is_empty() {
        return "Aucune entree dans inagle_team_build (table vide ou absente du miroir)."
            .to_string();
    }
    let mut rows: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "nom": e.name,
                "poste": e.position.as_deref().unwrap_or("N/A"),
                "element": e.element.as_deref().unwrap_or("N/A"),
            })
        })
        .collect();
    rows.truncate(50);
    render_ascii_table(&rows)
}

/// Rendu ASCII d'un résumé d'une entrée team-builder.
pub fn render_team_build_entry(e: &TeamBuildEntry) -> String {
    let mut lines = Vec::new();
    lines.push(TOP.to_string());
    lines.push(two_col(&e.name.to_string(), &format!("ID: {}", e.id)));
    lines.push(two_col(
        &format!("Poste: {}", e.position.as_deref().unwrap_or("N/A")),
        &format!("Element: {}", e.element.as_deref().unwrap_or("N/A")),
    ));
    // Stats depuis data si présentes
    if let Some(stats) = e.data.get("stats").and_then(|s| s.get("lv99")) {
        lines.push(MID.to_string());
        lines.push(one_col("Stats lv99 :"));
        for key in &[
            "kick",
            "control",
            "technique",
            "physical",
            "pressure",
            "agility",
            "intelligence",
        ] {
            if let Some(v) = stats.get(*key).and_then(|v| v.as_u64()) {
                lines.push(one_col(&format!("  {}: {}", key, v)));
            }
        }
    }
    lines.push(BOT.to_string());
    lines.join("\n")
}

/// Rendu ASCII d'une table générique.
pub fn render_ascii_table(rows: &[serde_json::Value]) -> String {
    if rows.is_empty() {
        return "Aucun resultat (0 ligne).".to_string();
    }
    let keys: Vec<String> = rows
        .first()
        .and_then(|r| r.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();

    if keys.is_empty() {
        return "Aucun resultat (pas de colonnes).".to_string();
    }

    let stringified: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            keys.iter()
                .map(|k| {
                    let val = row.get(k).cloned().unwrap_or(serde_json::Value::Null);
                    let s = match &val {
                        serde_json::Value::Null => "NULL".to_string(),
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    if s.len() > 45 {
                        format!("{}...", &s[..42])
                    } else {
                        s
                    }
                })
                .collect()
        })
        .collect();

    let col_widths: Vec<usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let max_row = stringified.iter().map(|r| r[i].len()).max().unwrap_or(0);
            k.len().max(max_row)
        })
        .collect();

    let border_top = format!(
        "┌─{}─┐",
        col_widths
            .iter()
            .map(|w| "─".repeat(*w))
            .collect::<Vec<_>>()
            .join("─┬─")
    );
    let border_mid = format!(
        "├─{}─┤",
        col_widths
            .iter()
            .map(|w| "─".repeat(*w))
            .collect::<Vec<_>>()
            .join("─┼─")
    );
    let border_bot = format!(
        "└─{}─┘",
        col_widths
            .iter()
            .map(|w| "─".repeat(*w))
            .collect::<Vec<_>>()
            .join("─┴─")
    );

    let header = format!(
        "│ {} │",
        keys.iter()
            .enumerate()
            .map(|(i, k)| format!("{:w$}", k, w = col_widths[i]))
            .collect::<Vec<_>>()
            .join(" │ ")
    );

    let data_rows: Vec<String> = stringified
        .iter()
        .map(|r| {
            format!(
                "│ {} │",
                r.iter()
                    .enumerate()
                    .map(|(i, v)| format!("{:w$}", v, w = col_widths[i]))
                    .collect::<Vec<_>>()
                    .join(" │ ")
            )
        })
        .collect();

    let mut parts = vec![border_top, header, border_mid];
    parts.extend(data_rows);
    parts.push(border_bot);
    parts.join("\n")
}
