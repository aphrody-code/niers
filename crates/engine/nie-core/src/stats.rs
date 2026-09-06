//! Calcul de statistiques joueur avec courbe de progression.
//!
//! Ce module n'est PAS directement porté d'un fichier C décompilé Ghidra —
//! la logique de croissance de stats IEVR est tirée du cross-référencement
//! avec `packages/inagle/src/stat-calculator.ts` (validé sur dump réel).
//!
//! La courbe de progression (segments lv1→30, 30→50, 50→99) correspond au
//! pattern de `growth_table_lv30` et `growth_table_main` des cfg.bin.
//!
//! # Statbloc IEVR
//!
//! 7 statistiques par joueur (noms confirmés par inagle):
//! - `Kc` = Kick (tir)
//! - `Cr` = Control (contrôle)
//! - `Tc` = Technique
//! - `Pr` = Power/Physical (puissance physique)
//! - `Ps` = Pressure (pressing)
//! - `Ag` = Agility (agilité)
//! - `It` = Intelligence

/// Bloc de 7 statistiques d'un joueur IEVR.
///
/// Noms confirmés par `packages/inagle/src/stat-calculator.ts` (interface `StatBlock`),
/// elle-même validée sur dump réel (`~/.local/share/Steam/iecode/inazuma`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatBlock {
    /// Kick — puissance de tir.
    pub kc: u16,
    /// Control — contrôle du ballon.
    pub cr: u16,
    /// Technique — hissatsu et mouvements techniques.
    pub tc: u16,
    /// Power/Physical — puissance physique, duels.
    pub pr: u16,
    /// Pressure — pressing, interceptions.
    pub ps: u16,
    /// Agility — vitesse, démarrage.
    pub ag: u16,
    /// Intelligence — lecture du jeu, positionnement.
    pub it: u16,
}

impl StatBlock {
    /// Somme de toutes les statistiques (total brut).
    #[must_use]
    pub fn total(self) -> u32 {
        u32::from(self.kc)
            + u32::from(self.cr)
            + u32::from(self.tc)
            + u32::from(self.pr)
            + u32::from(self.ps)
            + u32::from(self.ag)
            + u32::from(self.it)
    }

    /// Retourne les stats comme tableau ordonné `[Kc, Cr, Tc, Pr, Ps, Ag, It]`.
    #[must_use]
    pub fn as_array(self) -> [u16; 7] {
        [
            self.kc, self.cr, self.tc, self.pr, self.ps, self.ag, self.it,
        ]
    }

    /// Crée un `StatBlock` depuis un tableau `[Kc, Cr, Tc, Pr, Ps, Ag, It]`.
    #[must_use]
    pub fn from_array(arr: [u16; 7]) -> Self {
        Self {
            kc: arr[0],
            cr: arr[1],
            tc: arr[2],
            pr: arr[3],
            ps: arr[4],
            ag: arr[5],
            it: arr[6],
        }
    }
}

/// Calcule une statistique unique à un niveau donné par interpolation linéaire.
///
/// La courbe de croissance est divisée en 3 segments:
/// - lv 1–30 : interpolation entre `stat_lv1` et `stat_lv30`
/// - lv 30–50 : interpolation entre `stat_lv30` et `stat_lv50`
/// - lv 50–99 : interpolation entre `stat_lv50` et `stat_lv99`
///
/// Source: `packages/inagle/src/stat-calculator.ts` — `calculateSingleStat()`,
/// validé sur dump réel IEVR.
///
/// # Panics
///
/// Ne panique pas — les niveaux hors range sont clampés.
///
/// # Exemple
///
/// ```
/// use nie_core::stats::calculate_single_stat;
///
/// // Stat à lv 1 = valeur de base
/// assert_eq!(calculate_single_stat(1, 10, 30, 50, 80), 10);
/// // Stat à lv 99 = valeur max
/// assert_eq!(calculate_single_stat(99, 10, 30, 50, 80), 80);
/// // Stat à lv 30 = exactement stat_lv30
/// assert_eq!(calculate_single_stat(30, 10, 30, 50, 80), 30);
/// ```
#[must_use]
pub fn calculate_single_stat(
    level: u8,
    stat_lv1: u16,
    stat_lv30: u16,
    stat_lv50: u16,
    stat_lv99: u16,
) -> u16 {
    if level == 0 || level == 1 {
        return stat_lv1;
    }
    if level >= 99 {
        return stat_lv99;
    }
    if level <= 30 {
        lerp_u16(stat_lv1, stat_lv30, f32::from(level - 1) / 29.0)
    } else if level <= 50 {
        lerp_u16(stat_lv30, stat_lv50, f32::from(level - 30) / 20.0)
    } else {
        lerp_u16(stat_lv50, stat_lv99, f32::from(level - 50) / 49.0)
    }
}

/// Interpolation linéaire entière (floor) entre deux valeurs `u16`.
///
/// Source: `stat-calculator.ts` — `lerp(start, end, t) = floor(start + (end - start) * t)`.
// Le résultat est borné entre `start` et `end` (deux u16 ≥ 0) pour t ∈ [0,1],
// donc le floor+cast est sûr (jamais négatif, jamais > 65535).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn lerp_u16(start: u16, end: u16, t: f32) -> u16 {
    let s = f32::from(start);
    let e = f32::from(end);
    (s + (e - s) * t).floor() as u16
}

/// Paramètres de croissance d'un personnage.
///
/// Correspond aux tables `growth_table_lv1/lv30/main` des cfg.bin IEVR,
/// croisées avec `packages/inagle/src/stat-calculator.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrowthParams {
    /// Position principale (1=GK, 2=FW, 3=MF, 4=DF).
    ///
    /// Vérité terrain : `refs/iecode-re/cli/include/iecode/gamedata/types.h:28`
    /// (`enum Position { GK=1, FW=2, MF=3, DF=4 }`) et
    /// `refs/iecode-re/cli/src/gamedata/loader.cpp:178`.
    pub main_position: u8,
    /// Sous-position (0 = aucune).
    pub sub_position: u8,
    /// Pattern de croissance (0 ou 1 dans les tables).
    pub growth_pattern: u8,
    /// Rang de rareté pour les tables (0=N, 2=R, 3=SR, 4=SSR, 5=UR).
    pub chara_rank: u8,
}

/// Calcule le bloc de statistiques à un niveau donné.
///
/// Applique `calculate_single_stat()` à chacune des 7 statistiques.
///
/// # Exemple
///
/// ```
/// use nie_core::stats::{StatBlock, calculate_stat_block};
///
/// let lv1  = StatBlock::from_array([10, 12, 8, 15, 9, 11, 7]);
/// let lv30 = StatBlock::from_array([30, 35, 25, 40, 28, 32, 22]);
/// let lv50 = StatBlock::from_array([50, 55, 45, 60, 48, 52, 42]);
/// let lv99 = StatBlock::from_array([80, 85, 75, 90, 78, 82, 72]);
///
/// let at_lv1 = calculate_stat_block(1, lv1, lv30, lv50, lv99);
/// assert_eq!(at_lv1, lv1);
///
/// let at_lv99 = calculate_stat_block(99, lv1, lv30, lv50, lv99);
/// assert_eq!(at_lv99, lv99);
/// ```
#[must_use]
pub fn calculate_stat_block(
    level: u8,
    lv1: StatBlock,
    lv30: StatBlock,
    lv50: StatBlock,
    lv99: StatBlock,
) -> StatBlock {
    StatBlock {
        kc: calculate_single_stat(level, lv1.kc, lv30.kc, lv50.kc, lv99.kc),
        cr: calculate_single_stat(level, lv1.cr, lv30.cr, lv50.cr, lv99.cr),
        tc: calculate_single_stat(level, lv1.tc, lv30.tc, lv50.tc, lv99.tc),
        pr: calculate_single_stat(level, lv1.pr, lv30.pr, lv50.pr, lv99.pr),
        ps: calculate_single_stat(level, lv1.ps, lv30.ps, lv50.ps, lv99.ps),
        ag: calculate_single_stat(level, lv1.ag, lv30.ag, lv50.ag, lv99.ag),
        it: calculate_single_stat(level, lv1.it, lv30.it, lv50.it, lv99.it),
    }
}

/// Convertit un code de rareté en rang de table de croissance.
///
/// Source: `stat-calculator.ts` — `rarityToGrowthRank()`, validé sur dump réel.
///
/// | Code | Rareté  | Rang table |
/// |------|---------|------------|
/// | 0    | N       | 0          |
/// | 2    | R       | 2          |
/// | 3    | SR      | 3          |
/// | 4    | SSR     | 4          |
/// | 5    | UR      | 5          |
/// | 6    | LR      | 5 (= UR)   |
/// | 7    | Legend  | 5 (= UR)   |
/// | 20   | BASARA  | 5 (= UR)   |
#[must_use]
pub fn rarity_to_growth_rank(rarity_code: u8) -> u8 {
    match rarity_code {
        0 => 0,
        2 => 2,
        3 => 3,
        4 => 4,
        5 | 6 | 7 | 20 => 5,
        code => code.min(5),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_block_default_zeroed() {
        let sb = StatBlock::default();
        assert_eq!(sb.total(), 0);
    }

    #[test]
    fn stat_block_roundtrip_array() {
        let arr = [10u16, 20, 30, 40, 50, 60, 70];
        let sb = StatBlock::from_array(arr);
        assert_eq!(sb.as_array(), arr);
    }

    #[test]
    fn stat_block_total() {
        let sb = StatBlock::from_array([1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(sb.total(), 28);
    }

    #[test]
    fn calculate_single_stat_lv1() {
        assert_eq!(calculate_single_stat(1, 10, 30, 50, 80), 10);
    }

    #[test]
    fn calculate_single_stat_lv99() {
        assert_eq!(calculate_single_stat(99, 10, 30, 50, 80), 80);
    }

    #[test]
    fn calculate_single_stat_lv0_clamp() {
        // Niveau 0 clampé à lv1
        assert_eq!(calculate_single_stat(0, 10, 30, 50, 80), 10);
    }

    #[test]
    fn calculate_single_stat_lv30_exact() {
        // À lv 30, on doit retourner exactement stat_lv30
        assert_eq!(calculate_single_stat(30, 10, 30, 50, 80), 30);
    }

    #[test]
    fn calculate_single_stat_lv50_exact() {
        // À lv 50, on doit retourner exactement stat_lv50
        assert_eq!(calculate_single_stat(50, 10, 30, 50, 80), 50);
    }

    #[test]
    fn calculate_single_stat_midpoint_lv15() {
        // lv 15 = moitié du segment 1-30
        // lerp(10, 30, 14/29) ≈ floor(10 + 20 * 0.4828) ≈ floor(19.65) = 19
        let result = calculate_single_stat(15, 10, 30, 50, 80);
        assert!((18..=20).contains(&result), "attendu ≈19, obtenu {result}");
    }

    #[test]
    fn calculate_single_stat_lv40_mid_segment2() {
        // lv 40 = moitié du segment 30-50
        // lerp(30, 50, 10/20) = 40
        let result = calculate_single_stat(40, 10, 30, 50, 80);
        assert_eq!(result, 40);
    }

    #[test]
    fn calculate_single_stat_monotone() {
        // La courbe doit être monotone croissante
        let mut prev = calculate_single_stat(1, 10, 30, 50, 80);
        for lv in 2..=99 {
            let cur = calculate_single_stat(lv, 10, 30, 50, 80);
            assert!(
                cur >= prev,
                "stat doit etre croissante: lv{lv}: {cur} < lv{}: {prev}",
                lv - 1
            );
            prev = cur;
        }
    }

    #[test]
    fn calculate_stat_block_lv1() {
        let lv1 = StatBlock::from_array([10, 12, 8, 15, 9, 11, 7]);
        let lv30 = StatBlock::from_array([30, 35, 25, 40, 28, 32, 22]);
        let lv50 = StatBlock::from_array([50, 55, 45, 60, 48, 52, 42]);
        let lv99 = StatBlock::from_array([80, 85, 75, 90, 78, 82, 72]);

        let result = calculate_stat_block(1, lv1, lv30, lv50, lv99);
        assert_eq!(result, lv1);
    }

    #[test]
    fn calculate_stat_block_lv99() {
        let lv1 = StatBlock::from_array([10, 12, 8, 15, 9, 11, 7]);
        let lv30 = StatBlock::from_array([30, 35, 25, 40, 28, 32, 22]);
        let lv50 = StatBlock::from_array([50, 55, 45, 60, 48, 52, 42]);
        let lv99 = StatBlock::from_array([80, 85, 75, 90, 78, 82, 72]);

        let result = calculate_stat_block(99, lv1, lv30, lv50, lv99);
        assert_eq!(result, lv99);
    }

    #[test]
    fn rarity_to_growth_rank_known_values() {
        // Confirmé par stat-calculator.ts
        assert_eq!(rarity_to_growth_rank(0), 0); // N
        assert_eq!(rarity_to_growth_rank(2), 2); // R
        assert_eq!(rarity_to_growth_rank(3), 3); // SR
        assert_eq!(rarity_to_growth_rank(4), 4); // SSR
        assert_eq!(rarity_to_growth_rank(5), 5); // UR
        assert_eq!(rarity_to_growth_rank(6), 5); // LR → UR
        assert_eq!(rarity_to_growth_rank(7), 5); // Legend → UR
        assert_eq!(rarity_to_growth_rank(20), 5); // BASARA → UR
    }

    #[test]
    fn rarity_to_growth_rank_clamp() {
        // Code inconnu → clampé à 5
        assert_eq!(rarity_to_growth_rank(99), 5);
    }
}
