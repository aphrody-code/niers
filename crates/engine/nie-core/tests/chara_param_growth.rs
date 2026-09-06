//! Tests d'intégration : `TeamSetup::from_chara_params_and_levels`.
//!
//! ## Vérité terrain
//!
//! Personnage CHARA_PARAM_INFO_1 (dump réel
//! `chara_param_1.03.66.00.cfg.bin.json`) :
//! - `main_position = 2` (FW — GK=1, FW=2, MF=3, DF=4 selon
//!   `refs/iecode-re/cli/include/iecode/gamedata/types.h:28`)
//! - `sub_position  = 4` (DF)
//! - `growth_pattern = 0`
//! - `chara_rank` absent de `CharaParam` → défaut `0` (N) dans
//!   `from_chara_params_and_levels`.
//!
//! ## Valeurs calculées byte-à-byte sur les tables embarquées
//!
//! Extraites depuis `crates/engine/nie-core/data/growth_table.json` :
//!
//! | Table | clé                        | stats                             |
//! |-------|----------------------------|-----------------------------------|
//! | lv1   | (main=2, sub=4, style=0)   | [12, 13, 12, 10, 11, 9, 11]       |
//! | lv30  | (main=2, sub=4, pat=0, r=0)| [63, 64, 62, 63, 59, 57, 66]      |
//! | main  | (main=2, pat=0, rank=0)    | lv50=[111,108,98,87,85,80,96]     |
//! |       |                            | lv99=[177,173,156,139,135,130,155]|
//!
//! À lv1  : lerp(lv1, lv30, 0/29) = lv1   → [12,13,12,10,11,9,11]
//! À lv30 : lerp(lv1, lv30, 29/29) = lv30 → [63,64,62,63,59,57,66]
//! À lv99 : lv99 exact               → [177,173,156,139,135,130,155]

#![allow(clippy::pedantic)]

#[cfg(feature = "data")]
mod tests {
    use nie_core::match_sim::TeamSetup;
    use nie_data::chara_param::parse_all_chara_params;
    use serde_json::json;

    /// Les 43 valeurs Int exactes de CHARA_PARAM_INFO_1 (identiques au dump réel).
    /// Source : `data/common/gamedata/character/chara_param_1.03.66.00.cfg.bin.json`,
    /// confirmées dans `nie_data::tests::chara_param_golden`.
    const RAW: &[i64] = &[
        -357_386_801,
        1_128_709_053,
        4,
        2,
        4,
        1,
        11,
        1,
        0,
        3,
        0,
        604_761_586,
        1,
        598_373_713,
        13,
        1_591_574_804,
        20,
        724_843_777,
        30,
        1_988_803_013,
        38,
        -1_508_771_477,
        43,
        724_843_777,
        30,
        -1_325_922_806,
        38,
        1_210_030_151,
        43,
        2,
        0,
        -1,
        2,
        1,
        -2,
        -2,
        1,
        0,
        0,
        0,
        743_008_281,
        0,
        200_626_314,
    ];

    /// Construit un `serde_json::Value` mimant un noeud `CHARA_PARAM_INFO_1`
    /// depuis les valeurs brutes.
    fn fixture_node() -> serde_json::Value {
        let variables: Vec<_> = RAW
            .iter()
            .map(|v| json!({ "type": "Int", "value": v.to_string() }))
            .collect();
        json!({
            "entries": [{
                "name": "CHARA_PARAM_INFO_1",
                "variables": variables,
                "children": []
            }]
        })
    }

    /// Charge CHARA_PARAM_INFO_1 depuis le fixture inline.
    fn chara_param_info_1() -> nie_data::chara_param::CharaParam {
        let root = fixture_node();
        let mut params = parse_all_chara_params(&root);
        assert_eq!(
            params.len(),
            1,
            "fixture doit produire exactement un CharaParam"
        );
        let cp = params.remove(0);
        // Vérifications de cohérence du fixture (garde anti-régression).
        assert_eq!(
            cp.main_position, 2,
            "CHARA_PARAM_INFO_1 : main_position = 2 (FW)"
        );
        assert_eq!(
            cp.sub_position, 4,
            "CHARA_PARAM_INFO_1 : sub_position = 4 (DF)"
        );
        assert_eq!(
            cp.growth_pattern, 0,
            "CHARA_PARAM_INFO_1 : growth_pattern = 0"
        );
        cp
    }

    // -----------------------------------------------------------------------
    // Test principal : valeurs précises calculées byte-à-byte
    // -----------------------------------------------------------------------

    /// Pont CharaParam → from_chara_params_and_levels : valeurs concrètes vérifiées.
    ///
    /// CHARA_PARAM_INFO_1 (main=2=FW, sub=4=DF, pat=0, chara_rank défaut N=0) :
    /// - lv1  → stats = lv1 exact  = [12, 13, 12, 10, 11, 9, 11]
    /// - lv30 → stats = lv30 exact = [63, 64, 62, 63, 59, 57, 66]
    /// - lv99 → stats = lv99 exact = [177, 173, 156, 139, 135, 130, 155]
    ///
    /// Ces valeurs ont été lues depuis `crates/engine/nie-core/data/growth_table.json`
    /// (lv1[1], lv30 entrée main=2/sub=4/pat=0/rank=0, main entrée main=2/pat=0/rank=0)
    /// et recalculées via la formule `calculateSingleStat` (lerp floor).
    #[test]
    fn chara_param_info_1_lv1_stats_concretes() {
        let cp = chara_param_info_1();
        let equipe = TeamSetup::from_chara_params_and_levels("CHARA_PARAM_INFO_1", &[(&cp, 1)]);
        assert_eq!(
            equipe.aggregate_stats.as_array(),
            [12, 13, 12, 10, 11, 9, 11],
            "lv1 : lerp(lv1, lv30, 0.0) = lv1 exact [12,13,12,10,11,9,11]"
        );
    }

    #[test]
    fn chara_param_info_1_lv30_stats_concretes() {
        let cp = chara_param_info_1();
        let equipe = TeamSetup::from_chara_params_and_levels("CHARA_PARAM_INFO_1", &[(&cp, 30)]);
        assert_eq!(
            equipe.aggregate_stats.as_array(),
            [63, 64, 62, 63, 59, 57, 66],
            "lv30 : lerp(lv1, lv30, 29/29) = lv30 exact [63,64,62,63,59,57,66]"
        );
    }

    #[test]
    fn chara_param_info_1_lv99_stats_concretes() {
        let cp = chara_param_info_1();
        let equipe = TeamSetup::from_chara_params_and_levels("CHARA_PARAM_INFO_1", &[(&cp, 99)]);
        assert_eq!(
            equipe.aggregate_stats.as_array(),
            [177, 173, 156, 139, 135, 130, 155],
            "lv99 : stats lv99 exactes [177,173,156,139,135,130,155]"
        );
    }

    // -----------------------------------------------------------------------
    // Déterminisme
    // -----------------------------------------------------------------------

    /// Deux appels identiques donnent le même résultat (déterminisme garanti).
    #[test]
    fn determinisme_from_chara_params_and_levels() {
        let cp = chara_param_info_1();
        let r1 = TeamSetup::from_chara_params_and_levels("Test", &[(&cp, 50)]);
        let r2 = TeamSetup::from_chara_params_and_levels("Test", &[(&cp, 50)]);
        assert_eq!(
            r1.aggregate_stats, r2.aggregate_stats,
            "deux appels avec les mêmes params → stats identiques"
        );
    }

    /// Niveaux différents produisent des stats différentes (sanity check).
    #[test]
    fn niveaux_differents_donnent_stats_differentes() {
        let cp = chara_param_info_1();
        let lv1 = TeamSetup::from_chara_params_and_levels("Test", &[(&cp, 1)]);
        let lv99 = TeamSetup::from_chara_params_and_levels("Test", &[(&cp, 99)]);
        assert_ne!(
            lv1.aggregate_stats, lv99.aggregate_stats,
            "lv1 et lv99 doivent avoir des stats différentes"
        );
        assert!(
            lv99.aggregate_stats.kc > lv1.aggregate_stats.kc,
            "Kc lv99 ({}) doit dépasser Kc lv1 ({})",
            lv99.aggregate_stats.kc,
            lv1.aggregate_stats.kc
        );
    }

    /// from_chara_params_and_levels ne fixe pas les placements de formation.
    #[test]
    fn placements_absents() {
        let cp = chara_param_info_1();
        let equipe = TeamSetup::from_chara_params_and_levels("Test", &[(&cp, 30)]);
        assert!(
            equipe.placements.is_none(),
            "from_chara_params_and_levels ne fixe pas les placements"
        );
    }

    /// Moyenne sur deux joueurs identiques = stats du joueur seul.
    #[test]
    fn moyenne_deux_joueurs_identiques() {
        let cp = chara_param_info_1();
        let seul = TeamSetup::from_chara_params_and_levels("Seul", &[(&cp, 30)]);
        let deux = TeamSetup::from_chara_params_and_levels("Deux", &[(&cp, 30), (&cp, 30)]);
        assert_eq!(
            seul.aggregate_stats, deux.aggregate_stats,
            "moyenne de deux fois le même joueur = stats du joueur seul"
        );
    }

    // -----------------------------------------------------------------------
    // Test optionnel sur le vrai fichier (skip si absent du VPS)
    // -----------------------------------------------------------------------

    const REAL_PATH: &str = "data/common/gamedata/character/chara_param_1.03.66.00.cfg.bin.json";

    /// Charge le vrai dump chara_param ; retourne `None` si absent (skip silencieux).
    fn load_real_chara_params() -> Option<Vec<nie_data::chara_param::CharaParam>> {
        if !std::path::Path::new(REAL_PATH).exists() {
            return None;
        }
        let content = std::fs::read_to_string(REAL_PATH)
            .unwrap_or_else(|e| panic!("lecture {REAL_PATH}: {e}"));
        let root: serde_json::Value =
            serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
        Some(parse_all_chara_params(&root))
    }

    /// Vrai fichier : CHARA_PARAM_INFO_1 donne les mêmes stats que la fixture inline.
    ///
    /// Confirme que les données du dump réel sont cohérentes avec les constantes RAW
    /// embarquées dans ce test — validation byte-à-byte du pipeline complet.
    #[test]
    fn vrai_fichier_chara_param_info_1_concorde_avec_fixture() {
        let Some(params) = load_real_chara_params() else {
            return;
        };

        // CHARA_PARAM_INFO_1 a chara_param_id = 0xEAB2B5CF (= u32 de -357386801).
        let target_id = nie_data::hash::HashId::from_signed(-357_386_801);
        let Some(cp_reel) = params.iter().find(|p| p.chara_param_id == target_id) else {
            panic!("CHARA_PARAM_INFO_1 (0xEAB2B5CF) introuvable dans {REAL_PATH}");
        };

        // Vérification des champs extraits du dump réel.
        assert_eq!(
            cp_reel.main_position, 2,
            "dump réel : main_position = 2 (FW)"
        );
        assert_eq!(cp_reel.sub_position, 4, "dump réel : sub_position = 4 (DF)");
        assert_eq!(cp_reel.growth_pattern, 0, "dump réel : growth_pattern = 0");

        // Les stats calculées depuis le dump réel doivent coïncider avec le fixture.
        let equipe_reel = TeamSetup::from_chara_params_and_levels("Réel", &[(cp_reel, 1)]);
        assert_eq!(
            equipe_reel.aggregate_stats.as_array(),
            [12, 13, 12, 10, 11, 9, 11],
            "lv1 depuis dump réel = [12,13,12,10,11,9,11] (identique au fixture inline)"
        );
        let equipe_reel_lv30 =
            TeamSetup::from_chara_params_and_levels("Réel lv30", &[(cp_reel, 30)]);
        assert_eq!(
            equipe_reel_lv30.aggregate_stats.as_array(),
            [63, 64, 62, 63, 59, 57, 66],
            "lv30 depuis dump réel = [63,64,62,63,59,57,66]"
        );
    }
}
