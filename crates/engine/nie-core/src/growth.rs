//! Pipeline de croissance des statistiques : tables + lookup à fallback.
//!
//! La courbe d'interpolation 3-segments (lv1→30→50→99, lerp floor) vit déjà
//! dans [`crate::stats`]. Ce module ajoute la couche au-dessus, portée *à
//! l'identique* depuis le calculateur de référence d'inagle :
//!
//! - Structs de tables `GrowthTableLv1/Lv30/Main/Sub`
//! - Résolution d'entrée par fallback en cascade
//!   ([`find_lv1`]/[`find_lv30`]/[`find_main`])
//! - [`calculate_stats`] qui combine lookup + lerp
//!
//! # Source de vérité
//!
//! - Logique : `packages/inagle/src/stat-calculator.ts`
//!   (`findLv1Entry` L154-188, `findLv30Entry` L190-243, `findMainEntry`
//!   L245-284, `calculateStats` L289-312, `rarityToGrowthRank` L92-115).
//! - Schéma : `packages/inagle/src/parsers/growth-table-config.ts`.
//! - Données réelles : `common/gamedata/character/growth_table_config_0.00.00.00.cfg.bin.json`
//!   (`m_growthTableLv1List`=36, `m_growthTableLv30List`=144,
//!   `m_growthTableMainList`=48, `m_growthTableSubList`=48 — vérifié).
//!
//! Le mapping rareté→`charaRank` réutilise [`crate::stats::rarity_to_growth_rank`].

use crate::stats::{StatBlock, calculate_single_stat, rarity_to_growth_rank};

/// Entrée de la table des stats de niveau 1 (par position/sous-position/style).
///
/// Source : `m_growthTableLv1List`. Échantillon réel `lv1[0]` :
/// `{mainPosition:2, subPosition:3, playStyle:0, Kc_1:13, Cr_1:14, Tc_1:12,
/// Pr_1:10, Ps_1:10, Ag_1:9, It_1:11}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrowthTableLv1 {
    /// Position principale (1=GK, 2=FW, 3=MF, 4=DF).
    ///
    /// Vérité terrain : `refs/iecode-re/cli/include/iecode/gamedata/types.h:28`
    /// (`enum Position { GK=1, FW=2, MF=3, DF=4 }`) et
    /// `refs/iecode-re/cli/src/gamedata/loader.cpp:178`.
    pub main_position: u8,
    /// Sous-position (0 = aucune).
    pub sub_position: u8,
    /// Style de jeu (0, 1, 2 dans les données réelles).
    pub play_style: u8,
    /// Stats de niveau 1, ordre `[Kc, Cr, Tc, Pr, Ps, Ag, It]`.
    pub stats: [u16; 7],
}

/// Entrée de la table des stats de niveau 30 (par position/pattern/rang).
///
/// Source : `m_growthTableLv30List`. `charaRank` ∈ 0-5, `growthPattern` ∈ 0/1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrowthTableLv30 {
    /// Position principale (1=GK, 2=FW, 3=MF, 4=DF).
    pub main_position: u8,
    /// Sous-position (0 = aucune).
    pub sub_position: u8,
    /// Pattern de croissance (0 ou 1 dans les tables).
    pub growth_pattern: u8,
    /// Rang de table (0=N, 2=R, 3=SR, 4=SSR, 5=UR).
    pub chara_rank: u8,
    /// Stats de niveau 30, ordre `[Kc, Cr, Tc, Pr, Ps, Ag, It]`.
    pub stats: [u16; 7],
}

/// Entrée de la courbe principale (lv50 + lv99) par position/pattern/rang.
///
/// Source : `m_growthTableMainList`. Échantillon `main[0]` (mainPosition 2=FW,
/// pattern 0, rank 5) : lv50 `[164,160,146,130,127,122,144]`,
/// lv99 `[261,258,235,210,202,195,230]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrowthTableMain {
    /// Position principale (1=GK, 2=FW, 3=MF, 4=DF).
    pub main_position: u8,
    /// Pattern de croissance (0 ou 1).
    pub growth_pattern: u8,
    /// Rang de table (0-5).
    pub chara_rank: u8,
    /// Stats de niveau 50, ordre `[Kc, Cr, Tc, Pr, Ps, Ag, It]`.
    pub stats_50: [u16; 7],
    /// Stats de niveau 99, ordre `[Kc, Cr, Tc, Pr, Ps, Ag, It]`.
    pub stats_99: [u16; 7],
}

/// Entrée de la courbe secondaire (sub) par sous-position/pattern/rang.
///
/// Source : `m_growthTableSubList`. Présente pour fidélité de schéma ; le
/// calculateur d'inagle (`calculateStats`) n'utilise que `lv1/lv30/main`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrowthTableSub {
    /// Sous-position.
    pub sub_position: u8,
    /// Pattern de croissance (0 ou 1).
    pub growth_pattern: u8,
    /// Rang de table (0-5).
    pub chara_rank: u8,
    /// Stats de niveau 50, ordre `[Kc, Cr, Tc, Pr, Ps, Ag, It]`.
    pub stats_50: [u16; 7],
    /// Stats de niveau 99, ordre `[Kc, Cr, Tc, Pr, Ps, Ag, It]`.
    pub stats_99: [u16; 7],
}

/// Ensemble des 4 tables de croissance chargées depuis les vraies données.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrowthTables {
    /// Table niveau 1.
    pub lv1: Vec<GrowthTableLv1>,
    /// Table niveau 30.
    pub lv30: Vec<GrowthTableLv30>,
    /// Courbe principale (lv50/99).
    pub main: Vec<GrowthTableMain>,
    /// Courbe secondaire (lv50/99).
    pub sub: Vec<GrowthTableSub>,
}

/// Paramètres de croissance d'un personnage (clé de lookup).
///
/// Reproduit `CharacterGrowthParams` (stat-calculator.ts L75-81). `chara_rank`
/// est le code de rareté brut (0-7, 20) ; il est converti en rang de table via
/// [`rarity_to_growth_rank`] dans les lookups lv30/main.
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
    /// Pattern de croissance (0, 1, 2+).
    pub growth_pattern: u8,
    /// Code de rareté brut (0=N, 2=R, …, 5=UR, 6=LR, 7=Legend, 20=BASARA).
    pub chara_rank: u8,
    /// Style de jeu (0 par défaut).
    pub play_style: u8,
}

impl GrowthTables {
    /// Résout l'entrée de niveau 1 par fallback en cascade.
    ///
    /// Reproduit EXACTEMENT `findLv1Entry` (stat-calculator.ts L154-188) :
    /// 1. `(mainPosition, subPosition, playStyle)` exact.
    /// 2. Si `playStyle != 0` : `(mainPosition, subPosition, playStyle=0)`.
    /// 3. Si `subPosition != 0` : `(mainPosition, subPosition=0, playStyle=0)`.
    /// 4. `mainPosition` seul (première entrée trouvée).
    #[must_use]
    pub fn find_lv1(&self, params: &GrowthParams) -> Option<&GrowthTableLv1> {
        let mut entry = self.lv1.iter().find(|e| {
            e.main_position == params.main_position
                && e.sub_position == params.sub_position
                && e.play_style == params.play_style
        });

        if entry.is_none() && params.play_style != 0 {
            entry = self.lv1.iter().find(|e| {
                e.main_position == params.main_position
                    && e.sub_position == params.sub_position
                    && e.play_style == 0
            });
        }

        if entry.is_none() && params.sub_position != 0 {
            entry = self.lv1.iter().find(|e| {
                e.main_position == params.main_position && e.sub_position == 0 && e.play_style == 0
            });
        }

        if entry.is_none() {
            entry = self
                .lv1
                .iter()
                .find(|e| e.main_position == params.main_position);
        }

        entry
    }

    /// Résout l'entrée de niveau 30 par fallback en cascade.
    ///
    /// Reproduit EXACTEMENT `findLv30Entry` (stat-calculator.ts L190-243).
    /// `growthRank = rarityToGrowthRank(charaRank)` est calculé d'abord :
    /// 1. `(mainPosition, subPosition, growthPattern, charaRank=growthRank)` exact.
    /// 2. Si `growthPattern >= 2` : `growthPattern=0`.
    /// 3. Si `subPosition != 0` : `subPosition=0, growthPattern=0`.
    /// 4. Si `growthRank != 0` : `subPosition=0, growthPattern=0, charaRank=0`.
    /// 5. `mainPosition` seul.
    #[must_use]
    pub fn find_lv30(&self, params: &GrowthParams) -> Option<&GrowthTableLv30> {
        let growth_rank = rarity_to_growth_rank(params.chara_rank);

        let mut entry = self.lv30.iter().find(|e| {
            e.main_position == params.main_position
                && e.sub_position == params.sub_position
                && e.growth_pattern == params.growth_pattern
                && e.chara_rank == growth_rank
        });

        if entry.is_none() && params.growth_pattern >= 2 {
            entry = self.lv30.iter().find(|e| {
                e.main_position == params.main_position
                    && e.sub_position == params.sub_position
                    && e.growth_pattern == 0
                    && e.chara_rank == growth_rank
            });
        }

        if entry.is_none() && params.sub_position != 0 {
            entry = self.lv30.iter().find(|e| {
                e.main_position == params.main_position
                    && e.sub_position == 0
                    && e.growth_pattern == 0
                    && e.chara_rank == growth_rank
            });
        }

        if entry.is_none() && growth_rank != 0 {
            entry = self.lv30.iter().find(|e| {
                e.main_position == params.main_position
                    && e.sub_position == 0
                    && e.growth_pattern == 0
                    && e.chara_rank == 0
            });
        }

        if entry.is_none() {
            entry = self
                .lv30
                .iter()
                .find(|e| e.main_position == params.main_position);
        }

        entry
    }

    /// Résout l'entrée de la courbe principale (lv50/99) par fallback en cascade.
    ///
    /// Reproduit EXACTEMENT `findMainEntry` (stat-calculator.ts L245-284) :
    /// 1. `(mainPosition, growthPattern, charaRank=growthRank)` exact.
    /// 2. Si `growthPattern >= 2` : `growthPattern=0`.
    /// 3. Si `growthRank != 0` : `growthPattern=0, charaRank=0`.
    /// 4. `mainPosition` seul.
    #[must_use]
    pub fn find_main(&self, params: &GrowthParams) -> Option<&GrowthTableMain> {
        let growth_rank = rarity_to_growth_rank(params.chara_rank);

        let mut entry = self.main.iter().find(|e| {
            e.main_position == params.main_position
                && e.growth_pattern == params.growth_pattern
                && e.chara_rank == growth_rank
        });

        if entry.is_none() && params.growth_pattern >= 2 {
            entry = self.main.iter().find(|e| {
                e.main_position == params.main_position
                    && e.growth_pattern == 0
                    && e.chara_rank == growth_rank
            });
        }

        if entry.is_none() && growth_rank != 0 {
            entry = self.main.iter().find(|e| {
                e.main_position == params.main_position
                    && e.growth_pattern == 0
                    && e.chara_rank == 0
            });
        }

        if entry.is_none() {
            entry = self
                .main
                .iter()
                .find(|e| e.main_position == params.main_position);
        }

        entry
    }
}

/// Calcule le bloc de 7 statistiques en combinant lookup + lerp.
///
/// Reproduit EXACTEMENT `calculateStats` (stat-calculator.ts L289-312) :
/// résout `lv1`/`lv30`/`main` par fallback, puis applique
/// [`calculate_single_stat`] à chacune des 7 stats. Si une des 3 entrées est
/// introuvable, retourne un bloc à zéro (comportement TS : warn + stats 0).
///
/// Ordre des stats : `Kc, Cr, Tc, Pr, Ps, Ag, It`.
#[must_use]
pub fn calculate_stats(tables: &GrowthTables, params: &GrowthParams, level: u8) -> StatBlock {
    let (Some(lv1), Some(lv30), Some(main)) = (
        tables.find_lv1(params),
        tables.find_lv30(params),
        tables.find_main(params),
    ) else {
        // Parité avec le TS : données absentes → stats 0 (pas d'invention).
        return StatBlock::default();
    };

    let mut out = [0u16; 7];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = calculate_single_stat(
            level,
            lv1.stats[i],
            lv30.stats[i],
            main.stats_50[i],
            main.stats_99[i],
        );
    }
    StatBlock::from_array(out)
}

#[cfg(feature = "data")]
mod loader {
    //! Chargement des tables réelles embarquées (`data/growth_table.json`).
    //!
    //! Le JSON est une projection compacte des vraies listes IEVR (arrays
    //! positionnels) générée depuis
    //! `growth_table_config_0.00.00.00.cfg.bin.json`.

    use super::{GrowthTableLv1, GrowthTableLv30, GrowthTableMain, GrowthTableSub, GrowthTables};

    /// JSON projeté des 4 tables, embarqué à la compilation.
    const GROWTH_JSON: &str = include_str!("../data/growth_table.json");

    #[derive(serde::Deserialize)]
    struct RawTables {
        lv1: Vec<[u16; 10]>,
        lv30: Vec<[u16; 11]>,
        main: Vec<[u16; 17]>,
        sub: Vec<[u16; 17]>,
    }

    impl GrowthTables {
        /// Charge les vraies tables de croissance IEVR embarquées.
        ///
        /// # Panics
        ///
        /// Panique si le JSON embarqué est corrompu (impossible en pratique :
        /// il est figé à la compilation et testé).
        // Les positions (1-4), sous-positions (0-4), patterns (0/1) et rangs
        // (0-5) tiennent tous dans un u8 — la troncature `u16 as u8` est sûre
        // (données réelles bornées, vérifiées par les tests de comptes).
        #[allow(clippy::cast_possible_truncation)]
        #[must_use]
        pub fn load_embedded() -> Self {
            let raw: RawTables =
                serde_json::from_str(GROWTH_JSON).expect("growth_table.json embarqué valide");

            let lv1 = raw
                .lv1
                .iter()
                .map(|a| GrowthTableLv1 {
                    main_position: a[0] as u8,
                    sub_position: a[1] as u8,
                    play_style: a[2] as u8,
                    stats: [a[3], a[4], a[5], a[6], a[7], a[8], a[9]],
                })
                .collect();

            let lv30 = raw
                .lv30
                .iter()
                .map(|a| GrowthTableLv30 {
                    main_position: a[0] as u8,
                    sub_position: a[1] as u8,
                    growth_pattern: a[2] as u8,
                    chara_rank: a[3] as u8,
                    stats: [a[4], a[5], a[6], a[7], a[8], a[9], a[10]],
                })
                .collect();

            let main = raw
                .main
                .iter()
                .map(|a| GrowthTableMain {
                    main_position: a[0] as u8,
                    growth_pattern: a[1] as u8,
                    chara_rank: a[2] as u8,
                    stats_50: [a[3], a[4], a[5], a[6], a[7], a[8], a[9]],
                    stats_99: [a[10], a[11], a[12], a[13], a[14], a[15], a[16]],
                })
                .collect();

            let sub = raw
                .sub
                .iter()
                .map(|a| GrowthTableSub {
                    sub_position: a[0] as u8,
                    growth_pattern: a[1] as u8,
                    chara_rank: a[2] as u8,
                    stats_50: [a[3], a[4], a[5], a[6], a[7], a[8], a[9]],
                    stats_99: [a[10], a[11], a[12], a[13], a[14], a[15], a[16]],
                })
                .collect();

            Self {
                lv1,
                lv30,
                main,
                sub,
            }
        }
    }
}

#[cfg(all(test, feature = "data"))]
mod tests {
    use super::*;

    /// Golden : comptes des listes réelles (vérifié sur le cfg.bin.json).
    #[test]
    fn embedded_counts_match_real_data() {
        let t = GrowthTables::load_embedded();
        assert_eq!(t.lv1.len(), 36, "m_growthTableLv1List = 36");
        assert_eq!(t.lv30.len(), 144, "m_growthTableLv30List = 144");
        assert_eq!(t.main.len(), 48, "m_growthTableMainList = 48");
        assert_eq!(t.sub.len(), 48, "m_growthTableSubList = 48");
    }

    /// Golden : échantillon `lv1[0]` (mainPosition 2, subPosition 3).
    #[test]
    fn embedded_lv1_first_entry() {
        let t = GrowthTables::load_embedded();
        let e = t.lv1[0];
        assert_eq!(e.main_position, 2);
        assert_eq!(e.sub_position, 3);
        assert_eq!(e.play_style, 0);
        // Kc_1..It_1 = 13,14,12,10,10,9,11
        assert_eq!(e.stats, [13, 14, 12, 10, 10, 9, 11]);
    }

    /// Golden : échantillon `main[0]` (mainPosition 2, pattern 0, rank 5).
    #[test]
    fn embedded_main_first_entry() {
        let t = GrowthTables::load_embedded();
        let e = t.main[0];
        assert_eq!(e.main_position, 2);
        assert_eq!(e.growth_pattern, 0);
        assert_eq!(e.chara_rank, 5);
        assert_eq!(e.stats_50, [164, 160, 146, 130, 127, 122, 144]);
        assert_eq!(e.stats_99, [261, 258, 235, 210, 202, 195, 230]);
    }

    // Golden de bout en bout : sorties REELLES d'inagle (calculateStats du
    // stat-calculator.ts exécuté sur les vraies tables). Voir résumé de portage.
    // Ordre des stats : [Kc, Cr, Tc, Pr, Ps, Ag, It].

    fn p(main: u8, sub: u8, pat: u8, rank: u8) -> GrowthParams {
        GrowthParams {
            main_position: main,
            sub_position: sub,
            growth_pattern: pat,
            chara_rank: rank,
            play_style: 0,
        }
    }

    #[test]
    fn golden_gk_n() {
        // GK rang N : mainPosition 1, sub 0, pattern 0, rank 0.
        let t = GrowthTables::load_embedded();
        let params = p(1, 0, 0, 0);
        assert_eq!(
            calculate_stats(&t, &params, 1).as_array(),
            [12, 13, 12, 10, 11, 9, 11]
        );
        assert_eq!(
            calculate_stats(&t, &params, 30).as_array(),
            [96, 100, 96, 105, 99, 103, 94]
        );
        assert_eq!(
            calculate_stats(&t, &params, 50).as_array(),
            [84, 92, 86, 97, 101, 104, 92]
        );
        assert_eq!(
            calculate_stats(&t, &params, 99).as_array(),
            [133, 146, 137, 157, 164, 166, 150]
        );
    }

    #[test]
    fn golden_fw_ur() {
        // DF rang UR : mainPosition 4, sub 0, pattern 0, rank 5.
        // Note : le nom de ce test est conservé pour compatibilité ; mainPosition=4 correspond
        // à DF (GK=1, FW=2, MF=3, DF=4 — types.h:28 / loader.cpp:178).
        let t = GrowthTables::load_embedded();
        let params = p(4, 0, 0, 5);
        assert_eq!(
            calculate_stats(&t, &params, 1).as_array(),
            [12, 13, 12, 10, 11, 9, 11]
        );
        assert_eq!(
            calculate_stats(&t, &params, 30).as_array(),
            [96, 99, 99, 105, 99, 92, 105]
        );
        assert_eq!(
            calculate_stats(&t, &params, 50).as_array(),
            [129, 135, 136, 146, 151, 130, 164]
        );
        assert_eq!(
            calculate_stats(&t, &params, 99).as_array(),
            [207, 216, 218, 235, 242, 210, 261]
        );
    }

    #[test]
    fn golden_df_sub3_ur() {
        // FW sub3 rang UR : ancre directe sur lv30[0] et main[0].
        // Note : le nom conservé pour compatibilité ; mainPosition=2 = FW (types.h:28 / loader.cpp:178).
        let t = GrowthTables::load_embedded();
        let params = p(2, 3, 0, 5);
        assert_eq!(
            calculate_stats(&t, &params, 1).as_array(),
            [13, 14, 12, 10, 10, 9, 11]
        );
        assert_eq!(
            calculate_stats(&t, &params, 30).as_array(),
            [106, 110, 106, 98, 87, 87, 99]
        );
        // lv50/99 = main[0] exact (sub3 retombe sur la courbe main par fallback).
        assert_eq!(
            calculate_stats(&t, &params, 50).as_array(),
            [164, 160, 146, 130, 127, 122, 144]
        );
        assert_eq!(
            calculate_stats(&t, &params, 99).as_array(),
            [261, 258, 235, 210, 202, 195, 230]
        );
    }

    #[test]
    fn golden_mf_pattern2_r_fallback() {
        // MF pattern 2 (>=2 → fallback pattern 0) rang R : teste la cascade.
        let t = GrowthTables::load_embedded();
        let params = p(3, 0, 2, 2);
        assert_eq!(
            calculate_stats(&t, &params, 1).as_array(),
            [13, 14, 12, 10, 10, 9, 11]
        );
        assert_eq!(
            calculate_stats(&t, &params, 30).as_array(),
            [103, 110, 106, 98, 92, 87, 99]
        );
        assert_eq!(
            calculate_stats(&t, &params, 50).as_array(),
            [103, 111, 108, 90, 90, 83, 99]
        );
        assert_eq!(
            calculate_stats(&t, &params, 99).as_array(),
            [164, 178, 173, 145, 145, 133, 159]
        );
    }

    #[test]
    fn fallback_unknown_position_returns_zero() {
        // Position inexistante (9) → aucune entrée → stats 0 (parité TS).
        let t = GrowthTables::load_embedded();
        let params = p(9, 0, 0, 0);
        assert_eq!(calculate_stats(&t, &params, 50), StatBlock::default());
    }
}
