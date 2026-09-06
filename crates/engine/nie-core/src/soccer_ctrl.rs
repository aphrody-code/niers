//! Contrôleur de match (`game::CSoccerCtrl` et `game::CSoccerCtrlAI`).
//!
//! Porte le mécanisme de double-buffering de phases et le dispatch vers
//! les sous-contrôleurs depuis
//! `refs/iecode-re/research/ghidra-export/decompiled/soccer_ctrl.c`.
//!
//! # Sources RE
//!
//! - `soccer_ctrl.c` — `FUN_14140dd00` = `CSoccerCtrl::CSoccerCtrl()` (lignes nie.c 3697419+)
//! - `soccer_ctrl.c` — `FUN_14140dd50` = `CSoccerCtrl::update()` (lignes nie.c 3697419-3697511)
//! - `soccer_ctrl.c` — `FUN_14140df20` = `CSoccerCtrl::reset()` (lignes nie.c 3697535+)
//! - `soccer_ctrl.c` — `FUN_14140df70` = `CSoccerCtrlAI::CSoccerCtrlAI()` (lignes nie.c 3697535-3697546)
//!
//! # Fidélité
//!
//! - Double-buffering de phases (`phase_active`, `phase_prev`, `phase_transition`) : FIABLE
//! - Phase initiale = 1 : FIABLE (`*(undefined2 *)(param_1 + 99) = 1`)
//! - Reset pose `phase_active = 1` : FIABLE (`FUN_14140df20`)
//! - Taille zone IA = 0x400 bytes : FIABLE (observé dans `CSoccerCtrlAI` ctor)
//! - Logique de fallback phase 0 dans `update()` : FIABLE (portée fidèlement)
//! - Vtable calls (`on_enter`/`on_exit`/`update`) : NON PORTÉS (remplacés par trait `PhaseController`)

/// Index de phase de jeu (`CSoccerCtrl` offset 0x700-0x702).
///
/// La valeur `0` est une sentinelle de fallback — elle déclenche la restauration
/// de `phase_prev` lors d'une transition.
/// La valeur `1` est la phase par défaut ("normal match play").
///
/// RE incertain: la sémantique exacte des index (ex. 1=kickoff, 2=play,
/// 3=goal, 4=halftime...) n'est pas décodée dans ce fichier C.
pub type PhaseIndex = u8;

/// Phase en cours du contrôleur.
///
/// Source: `soccer_ctrl.c` offsets 0x700, 0x701, 0x702.
/// Le double-buffering permet des transitions atomiques:
/// - `active` = phase exécutée par le sous-contrôleur courant
/// - `prev` = phase sauvegardée (fallback si `active` redevient 0)
/// - `transition` = phase vers laquelle on transite
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhaseTriple {
    /// Phase précédemment active (`offset 0x700`).
    pub prev: PhaseIndex,
    /// Phase en transition (`offset 0x701`).
    pub transition: PhaseIndex,
    /// Phase actuellement active (`offset 0x702`).
    pub active: PhaseIndex,
}

impl Default for PhaseTriple {
    /// Valeurs issues du constructeur: `*(undefined2 *)(param_1 + 99) = 1`
    /// → les deux premiers bytes à 0, le troisième (0x702 = active) à 1.
    fn default() -> Self {
        Self {
            prev: 0,
            transition: 0,
            active: 1,
        }
    }
}

impl PhaseTriple {
    /// Réinitialise les phases (comme `CSoccerCtrl::reset()`).
    ///
    /// Source: `FUN_14140df20` — `*(undefined2 *)(param_1 + 0x700) = 0` puis
    /// `*(undefined1 *)(param_1 + 0x702) = 1`.
    pub fn reset(&mut self) {
        self.prev = 0;
        self.transition = 0;
        self.active = 1;
    }

    /// Effectue la transition de phase selon la logique de `CSoccerCtrl::update()`.
    ///
    /// Source: `FUN_14140dd50` — section "Détection de changement de phase".
    ///
    /// Règles de transition:
    /// 1. Si `new_phase != 0` : `active = new_phase`, `transition = ancien active`
    /// 2. Si `new_phase == 0` : restaure `active = prev`, efface `prev`
    ///    - Si `prev` était aussi 0 : `active = transition`
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_core::soccer_ctrl::PhaseTriple;
    ///
    /// let mut phases = PhaseTriple::default();
    /// assert_eq!(phases.active, 1);
    /// phases.transition_to(3);
    /// assert_eq!(phases.active, 3);
    /// // Retour en arrière via phase 0
    /// phases.transition_to(0);
    /// assert_eq!(phases.active, 1); // restaure prev=1 (était active avant)
    /// ```
    pub fn transition_to(&mut self, new_phase: PhaseIndex) {
        if new_phase != 0 {
            // Cas normal: sauvegarde l'active en transition, pose le nouveau
            let old_active = self.active;
            self.active = new_phase;
            self.transition = old_active;
        } else {
            // Phase 0 = fallback: restaure prev
            let saved_prev = self.prev;
            self.active = if saved_prev != 0 {
                self.prev = 0;
                saved_prev
            } else {
                // Double fallback: utilise transition
                self.transition
            };
        }
    }
}

/// Contrôleur de match principal (`game::CSoccerCtrl`).
///
/// Gère les transitions de phase et dispatch vers les sous-contrôleurs.
/// La zone `0x2F0-0x6FF` (zone du dispatcher) et le pointeur vers le
/// sous-contrôleur actif (`0x708`) sont modélisés ici sans les vtables C++.
///
/// Source: `soccer_ctrl.c` — `FUN_14140dd00`, `FUN_14140dd50`, `FUN_14140df20`.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerCtrl {
    /// Triple de phases (double-buffering).
    pub phases: PhaseTriple,
    /// `true` si un sous-contrôleur est actif.
    ///
    /// Source: `param_1 + 0x708` — non nul si sous-contrôleur existant.
    pub has_active_controller: bool,
}

impl SoccerCtrl {
    /// Crée un contrôleur avec les valeurs initiales du constructeur C++.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Réinitialise le contrôleur (comme `CSoccerCtrl::reset()`).
    ///
    /// Source: `FUN_14140df20`.
    pub fn reset(&mut self) {
        self.phases.reset();
        self.has_active_controller = false;
    }

    /// Effectue une transition de phase.
    ///
    /// Appelle `PhaseTriple::transition_to()` et signale qu'il faut
    /// créer un nouveau sous-contrôleur au prochain `update`.
    pub fn transition_phase(&mut self, new_phase: PhaseIndex) {
        self.phases.transition_to(new_phase);
        self.has_active_controller = false;
    }

    /// Retourne la phase actuellement active.
    #[must_use]
    pub fn active_phase(&self) -> PhaseIndex {
        self.phases.active
    }
}

/// Contrôleur IA de match (`game::CSoccerCtrlAI`), hérite de `CSoccerCtrl`.
///
/// Source: `soccer_ctrl.c` — `FUN_14140df70`.
///
/// Ajoute une zone de 0x400 bytes (1024 bytes) pour les données de décision
/// IA + une state machine embarquée (`CSoccerCtrlAIStateMachine`).
///
/// RE incertain: le contenu de la zone IA (`ai_data`) n'est pas décodé —
/// seule la taille (0x400) et l'initialisation à 0 sont connues.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerCtrlAi {
    /// Contrôleur de base.
    pub base: SoccerCtrl,
    /// Zone de données IA (1024 bytes initialisés à 0).
    ///
    /// Source: `FUN_1416709b0(param_1 + 0x60, 0, 0x400)` dans le ctor.
    /// RE incertain: structure interne inconnue (table de décision? poids?).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_byte_array"))]
    pub ai_data: [u8; 1024],
    /// Phase IA (offset 0x702 dans le ctor de `CSoccerCtrlAI`, remis à 0).
    ///
    /// Source: `*(undefined1 *)((longlong)param_1 + 0x702) = 0` dans `FUN_14140df70`.
    /// Note: diffère du constructeur parent (qui met `active = 1`) — l'IA
    /// réinitialise la phase active à 0 explicitement.
    pub ai_phase: PhaseIndex,
}

impl Default for SoccerCtrlAi {
    fn default() -> Self {
        let mut base = SoccerCtrl::default();
        // L'IA pose ai_phase = 0, pas 1 comme le parent
        base.phases.active = 0;
        Self {
            base,
            ai_data: [0u8; 1024],
            ai_phase: 0,
        }
    }
}

impl SoccerCtrlAi {
    /// Crée un contrôleur IA avec les valeurs initiales.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_triple_default() {
        let p = PhaseTriple::default();
        // *(undefined2 *)(param_1 + 99) = 1 → active = 1, les autres = 0
        assert_eq!(p.active, 1);
        assert_eq!(p.prev, 0);
        assert_eq!(p.transition, 0);
    }

    #[test]
    fn phase_triple_reset() {
        let mut p = PhaseTriple {
            active: 5,
            prev: 3,
            transition: 2,
        };
        p.reset();
        assert_eq!(p.active, 1);
        assert_eq!(p.prev, 0);
        assert_eq!(p.transition, 0);
    }

    #[test]
    fn phase_triple_normal_transition() {
        let mut p = PhaseTriple::default();
        // Transition vers phase 3
        p.transition_to(3);
        assert_eq!(p.active, 3);
        assert_eq!(p.transition, 1); // ancien active sauvegardé
    }

    #[test]
    fn phase_triple_fallback_to_prev() {
        // Séquence: active=1, transition vers 3, retour via 0
        let mut p = PhaseTriple::default();
        p.transition_to(3); // active=3, transition=1
        // Simulons que prev avait été posé
        p.prev = 1;
        p.transition_to(0); // doit restaurer active=1, effacer prev
        assert_eq!(p.active, 1);
        assert_eq!(p.prev, 0);
    }

    #[test]
    fn phase_triple_double_fallback_to_transition() {
        // Quand prev est aussi 0, utilise transition
        let mut p = PhaseTriple {
            active: 5,
            prev: 0,
            transition: 2,
        };
        p.transition_to(0);
        assert_eq!(p.active, 2); // fallback sur transition
    }

    #[test]
    fn soccer_ctrl_initial_phase() {
        let ctrl = SoccerCtrl::new();
        assert_eq!(ctrl.active_phase(), 1);
        assert!(!ctrl.has_active_controller);
    }

    #[test]
    fn soccer_ctrl_reset() {
        let mut ctrl = SoccerCtrl::new();
        ctrl.transition_phase(5);
        ctrl.has_active_controller = true;
        ctrl.reset();
        assert_eq!(ctrl.active_phase(), 1);
        assert!(!ctrl.has_active_controller);
    }

    #[test]
    fn soccer_ctrl_transition_phase() {
        let mut ctrl = SoccerCtrl::new();
        ctrl.transition_phase(4);
        assert_eq!(ctrl.active_phase(), 4);
        assert!(
            !ctrl.has_active_controller,
            "doit reinitialiser le controleur"
        );
    }

    #[test]
    fn soccer_ctrl_ai_phase_starts_zero() {
        let ai = SoccerCtrlAi::new();
        // Le ctor CSoccerCtrlAI remet active = 0 (contrairement au parent qui met 1)
        assert_eq!(ai.base.active_phase(), 0);
        assert_eq!(ai.ai_phase, 0);
    }

    #[test]
    fn soccer_ctrl_ai_data_zeroed() {
        let ai = SoccerCtrlAi::new();
        assert!(ai.ai_data.iter().all(|&b| b == 0));
    }
}
