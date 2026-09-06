//! Tables de croissance + calcul de stats (interpolation Lv1→99).
//!
//! ## Vérité terrain
//!
//! - Logique TS pure : `packages/inagle/src/stat-calculator.ts` (`calculateSingleStat` l.131-148,
//!   `rarityToGrowthRank` l.92-115, `findLv30/findMain` l.190-284, `StatBlock` l.12-20).
//! - Données : `/home/ubuntu/niers/data/common/gamedata/character/growth_table_config_0.00.00.00.cfg.bin.json`
//!   et export agrégé `/home/ubuntu/niers/data/all-gamedata/growth_tables.json` (mêmes 4 listes :
//!   lv1=36, lv30=144, main=48, sub=48).
//!
//! Échantillon vérifié Lv1[0] : mainPosition=2, subPosition=3, playStyle=0,
//! Kc=13, Cr=14, Tc=12, Pr=10, Ps=10, Ag=9, It=11. Main[0] : mainPosition=2, growthPattern=0,
//! charaRank=5, Kc_50=164 … Kc_99=261.
//!
//! Interpolation (`calculateSingleStat`) : paliers Lv1→30, 30→50, 50→99 avec
//! `lerp(a, b, t) = floor(a + (b - a) * t)`, `t` indexé sur (level-1)/29, (level-30)/20,
//! (level-50)/49. `level<1`→stat1, `level≥99`→stat99.

use alloc::vec::Vec;
use serde_json::Value;

/// Bloc de 7 stats. Source : `StatBlock` (stat-calculator.ts l.12-20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatBlock {
    /// Kick (Shoot).
    pub kc: i64,
    /// Control (Dribble).
    pub cr: i64,
    /// Technique.
    pub tc: i64,
    /// Power (Physical).
    pub pr: i64,
    /// Pressure.
    pub ps: i64,
    /// Agility (Speed).
    pub ag: i64,
    /// Intelligence.
    pub it: i64,
}

impl StatBlock {
    /// Force totale (somme des 7 stats). Source : `calculateTotalPower`.
    #[must_use]
    pub fn total(&self) -> i64 {
        self.kc + self.cr + self.tc + self.pr + self.ps + self.ag + self.it
    }
}

/// Entrée de `m_growthTableLv1List` (stats de base au niveau 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrowthTableLv1Entry {
    pub main_position: i64,
    pub sub_position: i64,
    pub play_style: i64,
    pub kc_1: i64,
    pub cr_1: i64,
    pub tc_1: i64,
    pub pr_1: i64,
    pub ps_1: i64,
    pub ag_1: i64,
    pub it_1: i64,
}

/// Entrée de `m_growthTableLv30List` (stats au niveau 30 par rang).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrowthTableLv30Entry {
    pub main_position: i64,
    pub sub_position: i64,
    pub growth_pattern: i64,
    pub chara_rank: i64,
    pub kc_30: i64,
    pub cr_30: i64,
    pub tc_30: i64,
    pub pr_30: i64,
    pub ps_30: i64,
    pub ag_30: i64,
    pub it_30: i64,
}

/// Entrée de `m_growthTableMainList` (stats aux niveaux 50 et 99).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrowthTableMainEntry {
    pub main_position: i64,
    pub growth_pattern: i64,
    pub chara_rank: i64,
    pub kc_50: i64,
    pub cr_50: i64,
    pub tc_50: i64,
    pub pr_50: i64,
    pub ps_50: i64,
    pub ag_50: i64,
    pub it_50: i64,
    pub kc_99: i64,
    pub cr_99: i64,
    pub tc_99: i64,
    pub pr_99: i64,
    pub ps_99: i64,
    pub ag_99: i64,
    pub it_99: i64,
}

/// Entrée de `m_growthTableSubList` (même forme que Main ; conservée pour complétude).
pub type GrowthTableSubEntry = GrowthTableMainEntry;

/// Les 4 listes de `growth_table_config`.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrowthTables {
    pub lv1: Vec<GrowthTableLv1Entry>,
    pub lv30: Vec<GrowthTableLv30Entry>,
    pub main: Vec<GrowthTableMainEntry>,
    pub sub: Vec<GrowthTableSubEntry>,
}

/// Paramètres de croissance d'un personnage. Source : `CharacterGrowthParams`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharacterGrowthParams {
    pub main_position: i64,
    pub sub_position: i64,
    pub growth_pattern: i64,
    pub chara_rank: i64,
    pub play_style: i64,
}

/// Mappe un code de rareté vers le rang de croissance. Source : `rarityToGrowthRank` (l.92-115).
/// Codes starSign : 0=N, 2=R, 3=SR, 4=SSR, 5=UR, 6=LR, 7=Legend, 20=BASARA.
#[must_use]
pub fn rarity_to_growth_rank(code: i64) -> i64 {
    match code {
        0 => 0,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        6 => 5,  // LR → stats UR
        7 => 5,  // Legend → stats UR
        20 => 5, // BASARA → stats UR
        other => {
            if other <= 5 {
                other
            } else {
                5
            }
        }
    }
}

/// Interpolation linéaire avec troncature (`Math.floor`). Source : `lerp` (l.127-129).
#[must_use]
fn lerp(start: i64, end: i64, t: f64) -> i64 {
    let v = start as f64 + (end as f64 - start as f64) * t;
    // floor (les valeurs réelles de stats sont positives, mais on suit floor strict).
    libm_floor(v) as i64
}

/// `floor` sans dépendre de `std` (no_std). Implémentation directe correcte pour les plages stats.
#[must_use]
fn libm_floor(v: f64) -> f64 {
    let truncated = v as i64 as f64;
    if v < 0.0 && (v - truncated).abs() > f64::EPSILON {
        truncated - 1.0
    } else {
        truncated
    }
}

/// Calcule une stat individuelle à un niveau donné. Source : `calculateSingleStat` (l.131-148).
#[must_use]
pub fn calculate_single_stat(level: i64, stat1: i64, stat30: i64, stat50: i64, stat99: i64) -> i64 {
    if level < 1 {
        return stat1;
    }
    if level >= 99 {
        return stat99;
    }
    if level <= 30 {
        return lerp(stat1, stat30, (level - 1) as f64 / 29.0);
    }
    if level <= 50 {
        return lerp(stat30, stat50, (level - 30) as f64 / 20.0);
    }
    lerp(stat50, stat99, (level - 50) as f64 / 49.0)
}

/// Lookup Lv1 avec chaîne de fallback (playStyle→0, subPosition→0, mainPosition seul).
/// Source : `findLv1Entry` (l.151-188).
#[must_use]
pub fn find_lv1_entry<'a>(
    tables: &'a GrowthTables,
    p: &CharacterGrowthParams,
) -> Option<&'a GrowthTableLv1Entry> {
    let exact = tables.lv1.iter().find(|e| {
        e.main_position == p.main_position
            && e.sub_position == p.sub_position
            && e.play_style == p.play_style
    });
    if exact.is_some() {
        return exact;
    }
    if p.play_style != 0 {
        let f = tables.lv1.iter().find(|e| {
            e.main_position == p.main_position
                && e.sub_position == p.sub_position
                && e.play_style == 0
        });
        if f.is_some() {
            return f;
        }
    }
    if p.sub_position != 0 {
        let f = tables.lv1.iter().find(|e| {
            e.main_position == p.main_position && e.sub_position == 0 && e.play_style == 0
        });
        if f.is_some() {
            return f;
        }
    }
    tables
        .lv1
        .iter()
        .find(|e| e.main_position == p.main_position)
}

/// Lookup Lv30 avec chaîne de fallback (pattern≥2→0, subPosition→0, charaRank→0).
/// Source : `findLv30Entry` (l.190-242).
#[must_use]
pub fn find_lv30_entry<'a>(
    tables: &'a GrowthTables,
    p: &CharacterGrowthParams,
) -> Option<&'a GrowthTableLv30Entry> {
    let rank = rarity_to_growth_rank(p.chara_rank);
    let exact = tables.lv30.iter().find(|e| {
        e.main_position == p.main_position
            && e.sub_position == p.sub_position
            && e.growth_pattern == p.growth_pattern
            && e.chara_rank == rank
    });
    if exact.is_some() {
        return exact;
    }
    if p.growth_pattern >= 2 {
        let f = tables.lv30.iter().find(|e| {
            e.main_position == p.main_position
                && e.sub_position == p.sub_position
                && e.growth_pattern == 0
                && e.chara_rank == rank
        });
        if f.is_some() {
            return f;
        }
    }
    if p.sub_position != 0 {
        let f = tables.lv30.iter().find(|e| {
            e.main_position == p.main_position
                && e.sub_position == 0
                && e.growth_pattern == 0
                && e.chara_rank == rank
        });
        if f.is_some() {
            return f;
        }
    }
    if rank != 0 {
        let f = tables.lv30.iter().find(|e| {
            e.main_position == p.main_position
                && e.sub_position == 0
                && e.growth_pattern == 0
                && e.chara_rank == 0
        });
        if f.is_some() {
            return f;
        }
    }
    tables
        .lv30
        .iter()
        .find(|e| e.main_position == p.main_position)
}

/// Lookup Main avec chaîne de fallback (pattern≥2→0, charaRank→0). Source : `findMainEntry` (l.244-284).
#[must_use]
pub fn find_main_entry<'a>(
    tables: &'a GrowthTables,
    p: &CharacterGrowthParams,
) -> Option<&'a GrowthTableMainEntry> {
    let rank = rarity_to_growth_rank(p.chara_rank);
    let exact = tables.main.iter().find(|e| {
        e.main_position == p.main_position
            && e.growth_pattern == p.growth_pattern
            && e.chara_rank == rank
    });
    if exact.is_some() {
        return exact;
    }
    if p.growth_pattern >= 2 {
        let f = tables.main.iter().find(|e| {
            e.main_position == p.main_position && e.growth_pattern == 0 && e.chara_rank == rank
        });
        if f.is_some() {
            return f;
        }
    }
    if rank != 0 {
        let f = tables.main.iter().find(|e| {
            e.main_position == p.main_position && e.growth_pattern == 0 && e.chara_rank == 0
        });
        if f.is_some() {
            return f;
        }
    }
    tables
        .main
        .iter()
        .find(|e| e.main_position == p.main_position)
}

/// Calcule le bloc de stats complet à un niveau. Source : `calculateStats` (l.290-340).
/// Renvoie `StatBlock` à 0 si les tables ne contiennent pas l'entrée (comme le défaut TS).
#[must_use]
pub fn calculate_stats(tables: &GrowthTables, p: &CharacterGrowthParams, level: i64) -> StatBlock {
    let (lv1, lv30, main) = match (
        find_lv1_entry(tables, p),
        find_lv30_entry(tables, p),
        find_main_entry(tables, p),
    ) {
        (Some(a), Some(b), Some(c)) => (a, b, c),
        _ => return StatBlock::default(),
    };

    StatBlock {
        kc: calculate_single_stat(level, lv1.kc_1, lv30.kc_30, main.kc_50, main.kc_99),
        cr: calculate_single_stat(level, lv1.cr_1, lv30.cr_30, main.cr_50, main.cr_99),
        tc: calculate_single_stat(level, lv1.tc_1, lv30.tc_30, main.tc_50, main.tc_99),
        pr: calculate_single_stat(level, lv1.pr_1, lv30.pr_30, main.pr_50, main.pr_99),
        ps: calculate_single_stat(level, lv1.ps_1, lv30.ps_30, main.ps_50, main.ps_99),
        ag: calculate_single_stat(level, lv1.ag_1, lv30.ag_30, main.ag_50, main.ag_99),
        it: calculate_single_stat(level, lv1.it_1, lv30.it_30, main.it_50, main.it_99),
    }
}

// ---------------------------------------------------------------------------
// Parsing des tables depuis le JSON agrégé / config
// ---------------------------------------------------------------------------

fn gi(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(0)
}

/// Parse les 4 tables depuis l'export agrégé `growth_tables.json`
/// (`{ "lv1": [...], "lv30": [...], "main": [...], "sub": [...] }`).
#[must_use]
pub fn parse_growth_tables(root: &Value) -> GrowthTables {
    let mut t = GrowthTables::default();

    if let Some(arr) = root.get("lv1").and_then(Value::as_array) {
        for v in arr {
            t.lv1.push(GrowthTableLv1Entry {
                main_position: gi(v, "mainPosition"),
                sub_position: gi(v, "subPosition"),
                play_style: gi(v, "playStyle"),
                kc_1: gi(v, "Kc_1"),
                cr_1: gi(v, "Cr_1"),
                tc_1: gi(v, "Tc_1"),
                pr_1: gi(v, "Pr_1"),
                ps_1: gi(v, "Ps_1"),
                ag_1: gi(v, "Ag_1"),
                it_1: gi(v, "It_1"),
            });
        }
    }
    if let Some(arr) = root.get("lv30").and_then(Value::as_array) {
        for v in arr {
            t.lv30.push(GrowthTableLv30Entry {
                main_position: gi(v, "mainPosition"),
                sub_position: gi(v, "subPosition"),
                growth_pattern: gi(v, "growthPattern"),
                chara_rank: gi(v, "charaRank"),
                kc_30: gi(v, "Kc_30"),
                cr_30: gi(v, "Cr_30"),
                tc_30: gi(v, "Tc_30"),
                pr_30: gi(v, "Pr_30"),
                ps_30: gi(v, "Ps_30"),
                ag_30: gi(v, "Ag_30"),
                it_30: gi(v, "It_30"),
            });
        }
    }
    let parse_main = |arr: &[Value], out: &mut Vec<GrowthTableMainEntry>| {
        for v in arr {
            out.push(GrowthTableMainEntry {
                main_position: gi(v, "mainPosition"),
                growth_pattern: gi(v, "growthPattern"),
                chara_rank: gi(v, "charaRank"),
                kc_50: gi(v, "Kc_50"),
                cr_50: gi(v, "Cr_50"),
                tc_50: gi(v, "Tc_50"),
                pr_50: gi(v, "Pr_50"),
                ps_50: gi(v, "Ps_50"),
                ag_50: gi(v, "Ag_50"),
                it_50: gi(v, "It_50"),
                kc_99: gi(v, "Kc_99"),
                cr_99: gi(v, "Cr_99"),
                tc_99: gi(v, "Tc_99"),
                pr_99: gi(v, "Pr_99"),
                ps_99: gi(v, "Ps_99"),
                ag_99: gi(v, "Ag_99"),
                it_99: gi(v, "It_99"),
            });
        }
    };
    if let Some(arr) = root.get("main").and_then(Value::as_array) {
        parse_main(arr, &mut t.main);
    }
    if let Some(arr) = root.get("sub").and_then(Value::as_array) {
        parse_main(arr, &mut t.sub);
    }

    t
}
