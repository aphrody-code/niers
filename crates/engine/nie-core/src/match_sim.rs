//! Simulation d'un match de football complet : kickoff → 90 minutes → FSM fin.
//!
//! ## Ce qui est CONFIRMÉ par le C décompilé
//!
//! - Formule de score horloge : `minutes * 10_000 + seconds`
//!   (`FUN_1412aa4a0` case 7, L282-288 de `soccer_match_state_machine.c`).
//! - Transitions FSM des 11 états dans [`crate::match_fsm`] (portées fidèlement
//!   du `switch` L79-316).
//! - Match normal (`is_training = false`) : `WaitTimer → Transition (5)`.
//! - `LoadNext` (case 10) : `return 1` = transition immédiate.
//!
//! ## Ce qui est NOMINAL (reconstruit, non confirmé byte-à-byte)
//!
//! - Durée de match : 90 minutes (convention football standard ; l'horloge
//!   interne nie.exe n'est pas encore décodée).
//! - Modèle de probabilité de but : taux de base 3,5 %/minute pondéré par
//!   `kc_attaquant / (kc_attaquant + ps_défenseur)`. Formule simple, non portée
//!   du C.
//! - Algorithme PRNG : le **vrai** `lives::CRand` (MT19937 32-bit, byte-exact, cf.
//!   [`crate::crand`]) — confirmé par décompilation de nie.exe, plus de Splitmix64 nominal.
//! - Structure [`TeamSetup`] : agrégat de stats — nie.exe travaille avec des
//!   structures de joueurs individuels (stride 0x570), non portées ici.

use crate::{
    match_fsm::{MatchContext, MatchState, final_score, tick},
    stats::StatBlock,
};
use nie_data::formation::{FormationConfig, SoccerFormPlacementInfo, SoccerFormationInfo};

// ============================================================================
// PRNG du match — le VRAI `lives::CRand` (MT19937), cf. crate::crand.
// La décompilation Ghidra a confirmé MT19937 32-bit canonique dans nie.exe ;
// on l'utilise ici à la place de l'ancien Splitmix64 nominal.
// NB : le MODÈLE de but qui consomme ce RNG reste nominal (la vraie résolution
// est event-driven + data-driven, pas une formule inline — cf.
// docs/modele-de-match.md). Seul le RNG est désormais réel.
// ============================================================================

use crate::crand::CRand;

// ============================================================================
// Types publics — position terrain
// ============================================================================

/// Coordonnée terrain 2D (x, y), dans le système de coordonnées IEVR.
///
/// Dérivée de `nie_data::formation::Vec2` lors du chargement de formation.
/// Système de coordonnées observé : `x` = horizontal (−1..+1), `y` = profondeur
/// (0 ≈ camp adverse, 1 ≈ propre but — GK `start_pos.y ≈ 0.96`).
///
/// ## CONFIRMÉ
///
/// Les valeurs sont copiées byte-à-byte depuis `formation_config_*.cfg.bin.json`
/// via [`PlayerPlacement::from_slot`]. Voir le test golden
/// `nie_data::tests::formation_golden` pour la validation octet-par-octet.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FieldPos {
    /// Composante horizontale (symétrie gauche/droite).
    pub x: f32,
    /// Composante profondeur (0 = devant, 1 = derrière).
    pub y: f32,
}

impl FieldPos {
    /// Position nulle.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
}

/// Placement initial d'un joueur dans la simulation, dérivé des vraies données IEVR.
///
/// Construit depuis [`SoccerFormPlacementInfo`] via [`TeamSetup::with_formation`] +
/// [`FormationConfig::placements_of`]. Chaque champ correspond directement à un champ
/// du dump `formation_config_*.cfg.bin.json`.
///
/// ## CONFIRMÉ
///
/// Les positions terrain (`start_pos`, `defense_pos`, `offense_pos`) proviennent
/// des vraies données Level-5, validées byte-à-byte dans `nie_data::tests::formation_golden`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerPlacement {
    /// Position au coup d'envoi (`startPos` du placement IEVR).
    pub start_pos: FieldPos,
    /// Position cible en phase défensive (`defensePos`).
    pub defense_pos: FieldPos,
    /// Position cible en phase offensive (`offensePos`).
    pub offense_pos: FieldPos,
    /// Index du slot dans la formation (0–10 pour une formation standard).
    pub position_no: i64,
    /// Type de position (1=GK, 2..=10 = positions de champ ; voir
    /// `SoccerPositionInfo::position_id`).
    pub position_id: i64,
    /// `true` si ce joueur tire le coup d'envoi.
    pub b_kickoff: bool,
}

impl PlayerPlacement {
    /// Construit un [`PlayerPlacement`] depuis un slot réel `SoccerFormPlacementInfo`.
    ///
    /// Copie byte-à-byte les champs de position terrain. Les champs annexes
    /// (corner, pk, bustup) ne sont pas stockés car hors scope de la simulation actuelle.
    fn from_slot(slot: &SoccerFormPlacementInfo) -> Self {
        Self {
            start_pos: FieldPos {
                x: slot.start_pos.x,
                y: slot.start_pos.y,
            },
            defense_pos: FieldPos {
                x: slot.defense_pos.x,
                y: slot.defense_pos.y,
            },
            offense_pos: FieldPos {
                x: slot.offense_pos.x,
                y: slot.offense_pos.y,
            },
            position_no: slot.position_no,
            position_id: slot.position_id,
            b_kickoff: slot.b_kickoff,
        }
    }
}

// ============================================================================
// Types publics — équipe
// ============================================================================

/// Configuration d'une équipe pour la simulation.
///
/// Les `aggregate_stats` représentent les statistiques moyennes de l'équipe
/// (typiquement la moyenne des 11 titulaires calculée avec [`StatBlock`]).
/// Le champ `placements` est optionnel : s'il est fourni (via [`Self::with_formation`]),
/// les positions initiales des joueurs sont dérivées des vraies données IEVR.
///
/// ## NOMINAL
///
/// La structure réelle de nie.exe traite les joueurs individuellement (stride
/// 0x570, champ `@+0xe4` pour l'ID joueur, `@+0x94` pour les stats) ; cette
/// abstraction agrégée ne reflète pas le C décompilé.
///
/// # Exemple
///
/// ```
/// use nie_core::{match_sim::TeamSetup, stats::StatBlock};
///
/// // Stats fixtures FW UR lv99 (confirmées par growth.rs golden_fw_ur)
/// let stats = StatBlock::from_array([207, 216, 218, 235, 242, 210, 261]);
/// let equipe = TeamSetup::new("Alpha", stats);
/// assert_eq!(equipe.aggregate_stats.kc, 207);
/// assert!(equipe.placements.is_none());
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TeamSetup {
    /// Nom de l'équipe (affiché dans les logs/sorties).
    pub name: String,
    /// Stats agrégées : `[Kc, Cr, Tc, Pr, Ps, Ag, It]`.
    pub aggregate_stats: StatBlock,
    /// Placements initiaux des 11 joueurs (issus des vraies formations IEVR).
    ///
    /// `None` si non initialisé (simulation nominale). Rempli par
    /// [`Self::with_formation`] depuis les vraies données `formation_config`.
    pub placements: Option<Vec<PlayerPlacement>>,
}

impl TeamSetup {
    /// Crée une équipe depuis ses stats agrégées (chemin nominal, placements absents).
    #[must_use]
    pub fn new(name: impl Into<String>, stats: StatBlock) -> Self {
        Self {
            name: name.into(),
            aggregate_stats: stats,
            placements: None,
        }
    }

    /// Crée une équipe depuis une liste de joueurs (moyenne des stats).
    ///
    /// ## NOMINAL
    ///
    /// L'agrégation par moyenne simple ne reflète pas la logique IEVR
    /// (pondération par position, rôle, etc.).
    ///
    /// # Panics
    ///
    /// Panique si `players` est vide.
    #[must_use]
    pub fn from_players(name: impl Into<String>, players: &[StatBlock]) -> Self {
        assert!(!players.is_empty(), "TeamSetup::from_players : liste vide");
        let n = players.len() as u32;
        let sum = players.iter().fold([0u32; 7], |mut acc, p| {
            let a = p.as_array();
            for (i, v) in acc.iter_mut().enumerate() {
                *v += u32::from(a[i]);
            }
            acc
        });
        // Les stats IEVR max ~300, n ≥ 1 → sum[i]/n ≤ 300 < 65535 : troncature sûre.
        #[allow(clippy::cast_possible_truncation)]
        let avg = StatBlock::from_array([
            (sum[0] / n) as u16,
            (sum[1] / n) as u16,
            (sum[2] / n) as u16,
            (sum[3] / n) as u16,
            (sum[4] / n) as u16,
            (sum[5] / n) as u16,
            (sum[6] / n) as u16,
        ]);
        Self::new(name, avg)
    }

    /// Initialise le placement des 11 joueurs depuis les vraies données IEVR.
    ///
    /// `cfg` est la [`FormationConfig`] chargée depuis
    /// `formation_config_*.cfg.bin.json` ; `formation` est l'entrée de formation
    /// choisie (voir [`FormationConfig::formations`] ou [`FormationConfig::find_by_id`]).
    ///
    /// Les positions [`FieldPos`] de chaque [`PlayerPlacement`] sont copiées
    /// byte-à-byte depuis `SoccerFormPlacementInfo` — elles sont donc confirmées
    /// par les tests golden de `nie_data`.
    ///
    /// ## CONFIRMÉ (données)
    ///
    /// Les valeurs `start_pos`, `defense_pos`, `offense_pos` proviennent
    /// de `formation_config_*.cfg.bin.json` Level-5, validées byte-à-byte.
    ///
    /// ## Exemple d'usage
    ///
    /// Voir le test d'intégration `crates/engine/nie-core/tests/simulation_formation.rs`
    /// pour un exemple complet sur le vrai fichier.
    #[must_use]
    pub fn with_formation(
        mut self,
        cfg: &FormationConfig,
        formation: &SoccerFormationInfo,
    ) -> Self {
        let slots = cfg.placements_of(formation);
        self.placements = Some(slots.iter().map(PlayerPlacement::from_slot).collect());
        self
    }

    /// Crée une équipe en calculant les stats depuis les vraies tables de croissance
    /// IEVR embarquées (nécessite la feature `data`).
    ///
    /// Chaque entrée `(params, level)` calcule un [`StatBlock`] via
    /// [`crate::growth::calculate_stats`] + [`crate::growth::GrowthTables::load_embedded`].
    /// Les stats individuelles sont agrégées par [`Self::from_players`].
    ///
    /// ## NOMINAL
    ///
    /// L'agrégation par moyenne simple reste nominale ; le poids par position
    /// (GK / FW / MF / DF) n'est pas implémenté.
    ///
    /// # Panics
    ///
    /// Panique si `players` est vide.
    #[cfg(feature = "data")]
    #[must_use]
    pub fn from_growth_params(
        name: impl Into<String>,
        players: &[(crate::growth::GrowthParams, u8)],
    ) -> Self {
        use crate::growth::{GrowthTables, calculate_stats};
        assert!(
            !players.is_empty(),
            "TeamSetup::from_growth_params : liste vide"
        );
        let tables = GrowthTables::load_embedded();
        let stats: Vec<StatBlock> = players
            .iter()
            .map(|(params, level)| calculate_stats(&tables, params, *level))
            .collect();
        Self::from_players(name, &stats)
    }

    /// Crée une équipe en calculant les stats depuis des
    /// [`nie_data::chara_param::CharaParam`] et des niveaux.
    ///
    /// Pour chaque joueur `(chara_param, level)`, les champs `main_position`,
    /// `sub_position` et `growth_pattern` sont projetés vers un
    /// [`crate::growth::GrowthParams`], puis les stats sont calculées via les
    /// tables de croissance IEVR embarquées et agrégées par [`Self::from_players`].
    ///
    /// ## NOMINAL
    ///
    /// - `chara_rank` (rareté) est absent de [`nie_data::chara_param::CharaParam`] :
    ///   il est fixé à `0` (rang N) par défaut. Les stats résultantes sont
    ///   **sous-estimées** pour les personnages de rareté R/SR/SSR/UR/LR.
    ///   Pour un calcul exact, utiliser [`Self::from_growth_params`] en précisant
    ///   le `chara_rank` manuellement.
    /// - `play_style` est aussi absent de `CharaParam` et défaut à `0`.
    /// - L'agrégation reste une moyenne simple (même réserve que
    ///   [`Self::from_players`]).
    ///
    /// # Panics
    ///
    /// Panique si `players` est vide.
    #[cfg(feature = "data")]
    #[must_use]
    pub fn from_chara_params_and_levels(
        name: impl Into<String>,
        players: &[(&nie_data::chara_param::CharaParam, u8)],
    ) -> Self {
        use crate::growth::{GrowthParams, GrowthTables, calculate_stats};
        assert!(
            !players.is_empty(),
            "TeamSetup::from_chara_params_and_levels : liste vide"
        );
        let tables = GrowthTables::load_embedded();
        // Les codes de position (1-4), sous-position (0-4) et growth_pattern (0-2)
        // tiennent tous dans un u8 (données réelles bornées, vérifiées via les golden
        // tests de nie-data). nie-core autorise clippy::pedantic, donc pas de lint ici.
        let stats: Vec<StatBlock> = players
            .iter()
            .map(|(cp, level)| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let params = GrowthParams {
                    main_position: cp.main_position as u8,
                    sub_position: cp.sub_position as u8,
                    growth_pattern: cp.growth_pattern as u8,
                    // chara_rank absent de CharaParam : défaut N (rang 0).
                    // Réserve documentée : les stats sont sous-estimées pour R à LR.
                    // Utiliser from_growth_params pour préciser le rang exact.
                    chara_rank: 0,
                    play_style: 0,
                };
                calculate_stats(&tables, &params, *level)
            })
            .collect();
        Self::from_players(name, &stats)
    }
}

/// Événement survenu pendant un match simulé.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MatchEvent {
    /// Coup de sifflet d'ouverture (minute 0).
    ///
    /// NOMINAL : point d'entrée de la boucle de jeu.
    Kickoff,
    /// But marqué à la `minute` donnée (1–90).
    ///
    /// ## NOMINAL
    ///
    /// Produit par le modèle probabiliste `kc/ps`, non confirmé depuis nie.exe.
    Goal {
        /// Minute de match (1–90).
        minute: u8,
        /// `true` si but de l'équipe domicile.
        is_home: bool,
    },
    /// Mi-temps (fin de la minute 45).
    Halftime {
        /// Score domicile à la mi-temps.
        home_score: u8,
        /// Score visiteur à la mi-temps.
        away_score: u8,
    },
    /// Coup de sifflet final (fin de la minute 90).
    FinalWhistle {
        /// Score final domicile.
        home_score: u8,
        /// Score final visiteur.
        away_score: u8,
    },
    /// Transition FSM enregistrée après le coup de sifflet final.
    ///
    /// ## CONFIRMÉ
    ///
    /// Séquence déterminée par le switch de `FUN_1412aa4a0` (match normal) :
    /// `Init → WaitTimer → Transition → Fade → Cleanup → PostMatch → FadeOut
    /// → LoadNext → LoadNext (immediate)`.
    FsmTransition {
        /// État avant la transition.
        from: MatchState,
        /// État après la transition.
        to: MatchState,
    },
}

/// Résultat complet d'un match simulé.
///
/// # Exemple
///
/// ```
/// use nie_core::{match_sim::{simulate_match, TeamSetup}, stats::StatBlock};
///
/// let stats = StatBlock::from_array([200, 190, 185, 175, 160, 170, 155]);
/// let home = TeamSetup::new("Raimon", stats);
/// let away = TeamSetup::new("Teikoku", stats);
///
/// let res = simulate_match(home, away, 42);
/// assert_eq!(res.final_clock, 900_000); // 90 minutes × 10_000 + 0 secondes
/// assert!(res.home_placements.is_none()); // pas de formation fournie
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchResult {
    /// Buts de l'équipe domicile.
    pub home_score: u8,
    /// Buts de l'équipe visiteuse.
    pub away_score: u8,
    /// Horloge finale encodée : `minutes * 10_000 + seconds`.
    ///
    /// ## CONFIRMÉ
    ///
    /// Formule portée depuis `FUN_1412aa4a0` case 7 (L282-288) :
    /// `(uint)uVar4 * 10000 + (uint)uVar3`. Pour un match de 90 minutes
    /// terminé à 90:00, `final_clock = 900_000`.
    pub final_clock: u32,
    /// Séquence complète d'événements (jeu + FSM post-match).
    pub events: Vec<MatchEvent>,
    /// Placements initiaux des joueurs domicile (issus des vraies données IEVR).
    ///
    /// `None` si aucune [`FormationConfig`] n'a été fournie via
    /// [`TeamSetup::with_formation`].
    pub home_placements: Option<Vec<PlayerPlacement>>,
    /// Placements initiaux des joueurs visiteur (issus des vraies données IEVR).
    ///
    /// `None` si aucune [`FormationConfig`] n'a été fournie via
    /// [`TeamSetup::with_formation`].
    pub away_placements: Option<Vec<PlayerPlacement>>,
}

// ============================================================================
// Constantes de simulation
// ============================================================================

/// Durée d'un match en minutes.
///
/// ## NOMINAL
///
/// Durée standard du football ; l'horloge interne de nie.exe n'est pas encore
/// décompilée.
pub const MATCH_DURATION_MINUTES: u8 = 90;

/// Taux de base de but par minute, par équipe.
///
/// ## NOMINAL
///
/// Calibré pour ~3 buts/match en moyenne avec des stats équilibrées.
/// Calcul : `GOAL_RATE_BASE * kc / (kc + ps)` × 90 min ≈ 1,45 buts/équipe.
const GOAL_RATE_BASE: f32 = 0.035;

// ============================================================================
// Simulation
// ============================================================================

/// Simule un match de football complet depuis le kickoff jusqu'à la transition
/// FSM finale, de façon déterministe pour une graine `seed` donnée.
///
/// ## Déterminisme garanti
///
/// Le même `seed` produit toujours le même [`MatchResult`]. La graine 64 bits est
/// repliée sur 32 bits puis confiée à `CRand` (MT19937) ; `seed == 0` est une graine
/// valide (MT19937 n'a pas de point fixe en 0, contrairement à un LCG).
///
/// ## Phase de jeu (NOMINAL)
///
/// La simulation tourne minute par minute sur 90 minutes. À chaque minute :
/// - Probabilité domicile = `GOAL_RATE_BASE * kc_home / (kc_home + ps_away)`
/// - Probabilité visiteur = `GOAL_RATE_BASE * kc_away / (kc_away + ps_home)`
///
/// Les deux tirages sont indépendants (un but domicile et un but visiteur
/// peuvent survenir la même minute, cas réel de nie.exe non modélisé).
///
/// ## Formations (CONFIRMÉ — données)
///
/// Si `home` ou `away` ont été enrichis avec [`TeamSetup::with_formation`],
/// leurs [`PlayerPlacement`] (positions `start_pos` / `defense_pos` /
/// `offense_pos` issues des vraies données `formation_config`) sont propagés
/// dans [`MatchResult::home_placements`] / [`MatchResult::away_placements`].
///
/// ## FSM post-match (CONFIRMÉ)
///
/// Après le coup de sifflet final, la FSM progresse depuis [`MatchState::Init`]
/// en mode match normal (`is_training = false`, `end_counter = 0`) via
/// [`crate::match_fsm::tick`] jusqu'à [`MatchState::LoadNext`] (transition
/// immédiate). Chaque transition est enregistrée dans `events` sous forme de
/// [`MatchEvent::FsmTransition`].
///
/// ## Valeurs sûres pour les tests
///
/// - `result.final_clock == 900_000` est CONFIRMÉ (formule C, 90 minutes).
/// - La séquence FSM Init→…→LoadNext est CONFIRMÉE (switch C L79-316).
/// - Les scores dépendent du PRNG (NOMINAL) : tester via `result == run2`.
#[must_use]
pub fn simulate_match(home: TeamSetup, away: TeamSetup, seed: u64) -> MatchResult {
    // Récupère les placements avant de consommer les TeamSetup.
    let home_placements = home.placements.clone();
    let away_placements = away.placements.clone();

    let mut rng = CRand::from_u64(seed);
    let mut home_score: u8 = 0;
    let mut away_score: u8 = 0;
    let mut events: Vec<MatchEvent> = Vec::with_capacity(128);

    // ------------------------------------------------------------------
    // Kickoff
    // ------------------------------------------------------------------
    events.push(MatchEvent::Kickoff);

    // ------------------------------------------------------------------
    // Boucle de jeu : 90 minutes
    // NOMINAL : modèle minute-par-minute probabiliste.
    // ------------------------------------------------------------------
    for minute in 1u8..=MATCH_DURATION_MINUTES {
        let kc_home = f32::from(home.aggregate_stats.kc).max(1.0);
        let ps_away = f32::from(away.aggregate_stats.ps).max(1.0);
        let kc_away = f32::from(away.aggregate_stats.kc).max(1.0);
        let ps_home = f32::from(home.aggregate_stats.ps).max(1.0);

        // Probabilité de but domicile : attaque home vs défense away.
        let p_home = GOAL_RATE_BASE * kc_home / (kc_home + ps_away);
        // Probabilité de but visiteur : attaque away vs défense home.
        let p_away = GOAL_RATE_BASE * kc_away / (kc_away + ps_home);

        if rng.next_f32() < p_home {
            home_score = home_score.saturating_add(1);
            events.push(MatchEvent::Goal {
                minute,
                is_home: true,
            });
        }
        if rng.next_f32() < p_away {
            away_score = away_score.saturating_add(1);
            events.push(MatchEvent::Goal {
                minute,
                is_home: false,
            });
        }

        if minute == 45 {
            events.push(MatchEvent::Halftime {
                home_score,
                away_score,
            });
        }
    }

    events.push(MatchEvent::FinalWhistle {
        home_score,
        away_score,
    });

    // ------------------------------------------------------------------
    // Score horloge : final_score(90, 0) = 900_000.
    // CONFIRMÉ : formule FUN_1412aa4a0 case 7 L282-288.
    // ------------------------------------------------------------------
    let final_clock = final_score(u16::from(MATCH_DURATION_MINUTES), 0);

    // ------------------------------------------------------------------
    // FSM post-match : Init → … → LoadNext (immediate).
    // CONFIRMÉ (match normal, is_training=false) sauf PostMatch→FadeOut
    // marqué « nominal » dans match_fsm.rs.
    // ------------------------------------------------------------------
    let ctx = MatchContext {
        is_training: false,
        end_counter: 0,
    };
    let mut fsm_state = MatchState::Init;
    loop {
        let t = tick(fsm_state, ctx);
        events.push(MatchEvent::FsmTransition {
            from: fsm_state,
            to: t.next,
        });
        fsm_state = t.next;
        if t.immediate {
            break;
        }
    }

    MatchResult {
        home_score,
        away_score,
        final_clock,
        events,
        home_placements,
        away_placements,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Stats fixtures reprises des golden tests de `nie_core::growth` —
    /// FW UR lv99, confirmées sur les vrais dumps IEVR.
    fn stats_fw_ur_lv99() -> StatBlock {
        StatBlock::from_array([207, 216, 218, 235, 242, 210, 261])
    }

    /// Stats fixtures GK N lv30, confirmées sur les vrais dumps IEVR.
    fn stats_gk_n_lv30() -> StatBlock {
        StatBlock::from_array([96, 100, 96, 105, 99, 103, 94])
    }

    fn equipes_identiques() -> (TeamSetup, TeamSetup) {
        let s = stats_fw_ur_lv99();
        (TeamSetup::new("Raimon", s), TeamSetup::new("Teikoku", s))
    }

    // ------------------------------------------------------------------
    // Formule de score (CONFIRMÉ)
    // ------------------------------------------------------------------

    /// Golden : final_score(90, 0) = 900_000 (match standard 90 minutes).
    ///
    /// Confirmé : formule C `minutes * 10_000 + seconds` (L282-288).
    #[test]
    fn final_clock_90_minutes() {
        let (home, away) = equipes_identiques();
        let res = simulate_match(home, away, 42);
        assert_eq!(res.final_clock, 900_000);
    }

    /// La formule `final_score` est cohérente pour plusieurs durées.
    /// Confirmé par les tests existants de `match_fsm` + étendu ici.
    #[test]
    fn formule_final_score_etendue() {
        assert_eq!(final_score(90, 0), 900_000); // match standard
        assert_eq!(final_score(45, 0), 450_000); // mi-temps
        assert_eq!(final_score(2, 30), 20_030); // déjà confirmé (match_fsm.rs)
        assert_eq!(final_score(0, 0), 0);
        assert_eq!(final_score(0, 45), 45);
    }

    // ------------------------------------------------------------------
    // Déterminisme (NOMINAL : dépend du PRNG réel `CRand`/MT19937)
    // ------------------------------------------------------------------

    /// Deux exécutions avec la même graine produisent des résultats identiques.
    #[test]
    fn determinisme_meme_seed() {
        let (h1, a1) = equipes_identiques();
        let (h2, a2) = equipes_identiques();
        let r1 = simulate_match(h1, a1, 0xDEAD_BEEF_1234_5678);
        let r2 = simulate_match(h2, a2, 0xDEAD_BEEF_1234_5678);
        assert_eq!(
            r1.home_score, r2.home_score,
            "score domicile doit être reproductible"
        );
        assert_eq!(
            r1.away_score, r2.away_score,
            "score visiteur doit être reproductible"
        );
        assert_eq!(r1.final_clock, r2.final_clock);
        assert_eq!(r1.events.len(), r2.events.len());
        // Événements identiques dans l'ordre
        for (e1, e2) in r1.events.iter().zip(r2.events.iter()) {
            assert_eq!(e1, e2, "les événements doivent être identiques");
        }
    }

    /// Deux graines différentes produisent des résultats différents (non garanti
    /// mais très probable sur 90 tirages).
    #[test]
    fn graines_differentes_donnent_resultats_differents() {
        let (h1, a1) = equipes_identiques();
        let (h2, a2) = equipes_identiques();
        let r1 = simulate_match(h1, a1, 1);
        let r2 = simulate_match(h2, a2, 2);
        // Les événements de but ne peuvent pas tous être identiques
        // sauf collision de PRNG (probabilité négligeable).
        let buts1: Vec<_> = r1
            .events
            .iter()
            .filter(|e| matches!(e, MatchEvent::Goal { .. }))
            .collect();
        let buts2: Vec<_> = r2
            .events
            .iter()
            .filter(|e| matches!(e, MatchEvent::Goal { .. }))
            .collect();
        // Au moins l'un des deux doit différer (longueur ou contenu).
        let identiques =
            buts1.len() == buts2.len() && buts1.iter().zip(buts2.iter()).all(|(a, b)| a == b);
        assert!(
            !identiques,
            "graines différentes doivent produire des buts différents"
        );
    }

    // ------------------------------------------------------------------
    // Structure des événements
    // ------------------------------------------------------------------

    /// Le premier événement est Kickoff, le dernier FsmTransition vers LoadNext.
    #[test]
    fn structure_evenements_kickoff_et_fin() {
        let (home, away) = equipes_identiques();
        let res = simulate_match(home, away, 42);

        assert_eq!(res.events.first(), Some(&MatchEvent::Kickoff));

        // Le dernier événement = FsmTransition{from=LoadNext, to=LoadNext}
        assert_eq!(
            res.events.last(),
            Some(&MatchEvent::FsmTransition {
                from: MatchState::LoadNext,
                to: MatchState::LoadNext,
            })
        );
    }

    /// FinalWhistle contient les mêmes scores que MatchResult.
    #[test]
    fn final_whistle_coherent_avec_result() {
        let (home, away) = equipes_identiques();
        let res = simulate_match(home, away, 99);
        let whistle = res.events.iter().find_map(|e| {
            if let MatchEvent::FinalWhistle {
                home_score,
                away_score,
            } = e
            {
                Some((*home_score, *away_score))
            } else {
                None
            }
        });
        let (wh, wa) = whistle.expect("FinalWhistle présent");
        assert_eq!(wh, res.home_score, "home_score cohérent avec FinalWhistle");
        assert_eq!(wa, res.away_score, "away_score cohérent avec FinalWhistle");
    }

    /// Halftime présent exactement une fois, avant FinalWhistle.
    #[test]
    fn halftime_une_fois_avant_final_whistle() {
        let (home, away) = equipes_identiques();
        let res = simulate_match(home, away, 7);
        let halftime_idx = res
            .events
            .iter()
            .position(|e| matches!(e, MatchEvent::Halftime { .. }));
        let whistle_idx = res
            .events
            .iter()
            .position(|e| matches!(e, MatchEvent::FinalWhistle { .. }));
        assert!(halftime_idx.is_some(), "Halftime présent");
        assert!(whistle_idx.is_some(), "FinalWhistle présent");
        assert!(
            halftime_idx.unwrap() < whistle_idx.unwrap(),
            "Halftime < FinalWhistle"
        );
    }

    /// Tous les buts ont une minute dans [1, 90].
    #[test]
    fn buts_dans_range_valide() {
        let (home, away) = equipes_identiques();
        let res = simulate_match(home, away, 12345);
        for ev in &res.events {
            if let MatchEvent::Goal { minute, .. } = ev {
                assert!(*minute >= 1 && *minute <= 90, "minute hors range: {minute}");
            }
        }
    }

    // ------------------------------------------------------------------
    // Séquence FSM post-match (CONFIRMÉ pour match normal)
    // ------------------------------------------------------------------

    /// La séquence FSM après FinalWhistle est Init→WaitTimer→Transition→Fade
    /// →Cleanup→PostMatch→FadeOut→LoadNext→LoadNext, cohérente avec le switch C.
    ///
    /// ## Statut confirmé/nominal
    ///
    /// - Init→WaitTimer→Transition→Fade→Cleanup : CONFIRMÉ (switch L84,L154,L238,L263,L289).
    /// - Cleanup→PostMatch→FadeOut→LoadNext : CONFIRMÉ (L289, L301).
    /// - LoadNext→LoadNext (immediate) : CONFIRMÉ (`return 1`, L315).
    /// - PostMatch→FadeOut : NOMINAL (documenté comme « avance nominal » dans match_fsm.rs).
    #[test]
    fn fsm_sequence_match_normal_confirmee() {
        let (home, away) = equipes_identiques();
        let res = simulate_match(home, away, 1);

        // Extraire les transitions FSM dans l'ordre
        let transitions: Vec<(MatchState, MatchState)> = res
            .events
            .iter()
            .filter_map(|e| {
                if let MatchEvent::FsmTransition { from, to } = e {
                    Some((*from, *to))
                } else {
                    None
                }
            })
            .collect();

        let attendu = [
            (MatchState::Init, MatchState::WaitTimer), // case 0→1 (fallthrough, CONFIRMÉ)
            (MatchState::WaitTimer, MatchState::Transition), // case 1→5 normal (CONFIRMÉ)
            (MatchState::Transition, MatchState::Fade), // case 5→6 normal (CONFIRMÉ)
            (MatchState::Fade, MatchState::Cleanup),   // case 6→7 fallthrough (CONFIRMÉ)
            (MatchState::Cleanup, MatchState::PostMatch), // case 7→8 (CONFIRMÉ)
            (MatchState::PostMatch, MatchState::FadeOut), // case 8→9 (NOMINAL)
            (MatchState::FadeOut, MatchState::LoadNext), // case 9→10 (CONFIRMÉ)
            (MatchState::LoadNext, MatchState::LoadNext), // case 10 return 1 (CONFIRMÉ)
        ];

        assert_eq!(
            transitions.len(),
            attendu.len(),
            "nombre de transitions FSM"
        );
        for (i, (got, exp)) in transitions.iter().zip(attendu.iter()).enumerate() {
            assert_eq!(got, exp, "transition FSM #{i}");
        }
    }

    /// 8 transitions FSM exactement pour un match normal.
    #[test]
    fn fsm_huit_transitions_match_normal() {
        let (home, away) = equipes_identiques();
        let res = simulate_match(home, away, 1);
        let count = res
            .events
            .iter()
            .filter(|e| matches!(e, MatchEvent::FsmTransition { .. }))
            .count();
        assert_eq!(count, 8, "8 transitions FSM pour un match normal");
    }

    // ------------------------------------------------------------------
    // TeamSetup
    // ------------------------------------------------------------------

    #[test]
    fn team_setup_from_players_moyenne() {
        let s1 = StatBlock::from_array([100, 100, 100, 100, 100, 100, 100]);
        let s2 = StatBlock::from_array([200, 200, 200, 200, 200, 200, 200]);
        let equipe = TeamSetup::from_players("Test", &[s1, s2]);
        assert_eq!(equipe.aggregate_stats.kc, 150);
        assert_eq!(equipe.aggregate_stats.ps, 150);
    }

    /// Les scores restent dans un intervalle raisonnable (sanity check).
    #[test]
    fn scores_dans_intervalle_raisonnable() {
        for seed in [0u64, 1, 42, 9999, 0xFFFF_FFFF] {
            let (home, away) = equipes_identiques();
            let res = simulate_match(home, away, seed);
            assert!(
                res.home_score <= 20,
                "seed={seed}: home_score={} > 20",
                res.home_score
            );
            assert!(
                res.away_score <= 20,
                "seed={seed}: away_score={} > 20",
                res.away_score
            );
        }
    }

    /// Équipe forte (kc=500, ps=100) doit marquer nettement plus qu'équipe faible
    /// (kc=50, ps=100) sur 1000 parties — propriété statistique du modèle NOMINAL.
    ///
    /// p_home = 0.035 * 500/(500+100) ≈ 0.0292 vs p_away = 0.035 * 50/(50+100) ≈ 0.0117.
    /// Différence attendue sur 90 000 tirs ≈ 1575 buts (> 30 σ) — test quasi-certain.
    #[test]
    fn equipe_forte_marque_plus_en_moyenne() {
        // Forte : kc élevé (attaque), ps moyen (défense)
        let forte = StatBlock::from_array([500, 100, 100, 100, 100, 100, 100]);
        // Faible : kc faible (attaque), ps moyen (défense) — même défense que forte
        let faible = StatBlock::from_array([50, 100, 100, 100, 100, 100, 100]);
        let home = TeamSetup::new("Forte", forte);
        let away = TeamSetup::new("Faible", faible);

        let mut total_home: u32 = 0;
        let mut total_away: u32 = 0;
        for seed in 0u64..1000 {
            let h = home.clone();
            let a = away.clone();
            let res = simulate_match(h, a, seed);
            total_home += u32::from(res.home_score);
            total_away += u32::from(res.away_score);
        }
        assert!(
            total_home > total_away,
            "équipe forte ({total_home} buts) doit devancer équipe faible ({total_away})"
        );
    }

    /// Stats GK N lv30 (défense forte) : nombre de buts adverses limité.
    #[test]
    fn stats_gk_n_lv30_defense_forte() {
        // ps=99 pour GK N lv30 : défense moyenne, but les tirages sont raisonnables
        let gk_stats = stats_gk_n_lv30();
        let fw_stats = stats_fw_ur_lv99();
        // Équipe avec défense forte (ps élevé) vs attaque UR
        let defenseurs = TeamSetup::new("Défenseurs", gk_stats);
        let attaquants = TeamSetup::new("Attaquants", fw_stats);
        let res = simulate_match(attaquants.clone(), defenseurs.clone(), 42);
        // La simulation produit des scores raisonnables avec ces stats réelles
        assert!(
            res.home_score <= 15,
            "home_score={} avec stats réelles",
            res.home_score
        );
        assert!(
            res.away_score <= 15,
            "away_score={} avec stats réelles",
            res.away_score
        );
    }

    // ------------------------------------------------------------------
    // Placements (nouveau chemin IEVR)
    // ------------------------------------------------------------------

    /// Sans formation, simulate_match retourne home_placements = None.
    #[test]
    fn sans_formation_placements_none() {
        let (home, away) = equipes_identiques();
        let res = simulate_match(home, away, 42);
        assert!(
            res.home_placements.is_none(),
            "home_placements = None sans formation"
        );
        assert!(
            res.away_placements.is_none(),
            "away_placements = None sans formation"
        );
    }

    /// TeamSetup::new() initialise placements = None.
    #[test]
    fn new_initialise_placements_none() {
        let stats = stats_fw_ur_lv99();
        let equipe = TeamSetup::new("Test", stats);
        assert!(equipe.placements.is_none());
    }

    /// TeamSetup::from_players() initialise placements = None.
    #[test]
    fn from_players_initialise_placements_none() {
        let s = stats_fw_ur_lv99();
        let equipe = TeamSetup::from_players("Test", &[s]);
        assert!(equipe.placements.is_none());
    }

    /// Déterminisme préservé sur les nouveaux champs home_placements / away_placements.
    ///
    /// Deux runs sans formation → placements = None reproductible.
    #[test]
    fn determinisme_champs_placements_none() {
        let (h1, a1) = equipes_identiques();
        let (h2, a2) = equipes_identiques();
        let r1 = simulate_match(h1, a1, 0xABCD_EF01);
        let r2 = simulate_match(h2, a2, 0xABCD_EF01);
        assert_eq!(
            r1.home_placements, r2.home_placements,
            "home_placements identiques"
        );
        assert_eq!(
            r1.away_placements, r2.away_placements,
            "away_placements identiques"
        );
    }

    /// FieldPos::ZERO est correctement défini.
    #[test]
    fn field_pos_zero() {
        assert_eq!(FieldPos::ZERO.x, 0.0_f32);
        assert_eq!(FieldPos::ZERO.y, 0.0_f32);
    }

    // ------------------------------------------------------------------
    // TeamSetup::from_growth_params (feature `data`)
    // ------------------------------------------------------------------

    /// from_growth_params calcule des stats à partir des tables réelles embarquées.
    ///
    /// Utilise GrowthParams FW UR lv99 (confirmé par growth.rs golden_fw_ur) :
    /// le Kc calculé doit correspondre aux stats réelles IEVR.
    #[cfg(feature = "data")]
    #[test]
    fn from_growth_params_fw_ur_lv99() {
        use crate::growth::GrowthParams;
        // FW (mainPosition=2 dans GrowthTables selon growth.rs), pattern=0, rank=5 (UR)
        // Golden confirmé : main[0] lv99 Kc=261 (growth.rs::embedded_main_first_entry)
        let params = GrowthParams {
            main_position: 2,
            sub_position: 0,
            growth_pattern: 0,
            chara_rank: 5, // UR
            play_style: 0,
        };
        let equipe = TeamSetup::from_growth_params("Test", &[(params, 99)]);
        // Kc à lv99 pour FW UR pattern 0 = 261 (golden growth.rs)
        assert_eq!(
            equipe.aggregate_stats.kc, 261,
            "Kc FW UR lv99 depuis tables réelles = 261"
        );
        assert!(
            equipe.placements.is_none(),
            "from_growth_params ne fixe pas les placements"
        );
    }

    /// from_growth_params sur plusieurs joueurs calcule la moyenne correctement.
    #[cfg(feature = "data")]
    #[test]
    fn from_growth_params_moyenne_joueurs() {
        use crate::growth::GrowthParams;
        let params_fw = GrowthParams {
            main_position: 2,
            sub_position: 0,
            growth_pattern: 0,
            chara_rank: 5, // UR
            play_style: 0,
        };
        // Deux fois le même joueur → moyenne = stats individuelles
        let equipe = TeamSetup::from_growth_params("Test", &[(params_fw, 99), (params_fw, 99)]);
        assert_eq!(
            equipe.aggregate_stats.kc, 261,
            "moyenne de deux fois le même Kc = 261"
        );
    }
}
