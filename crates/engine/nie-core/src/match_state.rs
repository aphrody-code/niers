//! Machine à états du match de football IEVR.
//!
//! Porte les transitions d'état de `game::CSceneSoccer` depuis
//! `refs/iecode-re/research/ghidra-export/decompiled/soccer_match_state_machine.c`.
//!
//! # Sources RE
//!
//! - `soccer_match_state_machine.c` — `FUN_1412aa4a0` (lignes nie.c 3444061-3444501)
//! - `soccer_match_state_machine.c` — `FUN_1412ab250` (lignes nie.c 3444583-3444795)
//!
//! # Fidélité
//!
//! - Numéros d'états (0-10) et leurs libellés : FIABLE (strings trouvées dans le binaire)
//! - Logique de transition d'entraînement (succès/échec, retry count) : FIABLE
//! - Score final = `minutes * 10000 + secondes` : FIABLE (calcul exact vu à l'état 7)
//! - Détail du cleanup (appels D3D, systèmes audio) : NON PORTÉ (hors scope)
//! - Sens exact du compteur `retry_count` (iVar18 == 0|1|2) : FIABLE (vérifié état 5)

/// Résultat d'un entraînement.
///
/// Source: `soccer_match_state_machine.c` état 1 —
/// `bVar5 = FUN_1414a01d0()` suivi de `sVar15 = (bVar5 ^ 1) + 0x52`.
/// `0x52` = succès, `0x53` = échec (décalage XOR).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TrainingResult {
    /// Entraînement réussi (code 0x52).
    Success,
    /// Entraînement échoué (code 0x53).
    Failure,
}

impl TrainingResult {
    /// Retourne le code entier utilisé dans l'UI (0x52 ou 0x53).
    #[must_use]
    pub fn ui_code(self) -> i16 {
        match self {
            Self::Success => 0x52,
            Self::Failure => 0x53,
        }
    }
}

/// Comportement après la fin d'un entraînement.
///
/// Source: `soccer_match_state_machine.c` état 5 —
/// switch sur `iVar18 = *(int *)(param_1 + 0x24)`.
/// Valeurs: 0 = restart, 1 = retry, 2 = complétion finale.
/// RE incertain: la distinction entre `Restart` (0) et `Retry` (1) est déduite
/// du contexte mais leurs rôles exacts dans l'UI sont unclear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TrainingEndKind {
    /// Recommencer depuis le début (iVar18 == 0).
    Restart,
    /// Retry standard (iVar18 == 1).
    Retry,
    /// Complétion finale, chargement résultat (iVar18 == 2).
    Complete,
}

impl TryFrom<i32> for TrainingEndKind {
    type Error = ();

    fn try_from(v: i32) -> Result<Self, ()> {
        match v {
            0 => Ok(Self::Restart),
            1 => Ok(Self::Retry),
            2 => Ok(Self::Complete),
            _ => Err(()),
        }
    }
}

/// Score final d'un match d'entraînement.
///
/// Source: `soccer_match_state_machine.c` état 7 —
/// ```c
/// uVar3 = *(ushort *)(... + 0x1e082); // secondes
/// uVar4 = *(ushort *)(... + 0x1e080); // minutes
/// FUN_1414e7c60(bVar19, (uint)uVar4 * 10000 + (uint)uVar3);
/// ```
/// Le score est donc `minutes * 10_000 + secondes`.
/// RE incertain: "minutes" et "secondes" sont des noms reconstruits —
/// le binaire stocke deux `u16` à des offsets contigus sans labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchScore {
    /// Composante "minutes" lue à offset `struct + 0x1e080`.
    pub minutes: u16,
    /// Composante "secondes" lue à offset `struct + 0x1e082`.
    pub seconds: u16,
}

impl MatchScore {
    /// Calcule la valeur encodée utilisée par `FUN_1414e7c60`.
    ///
    /// Formule exacte extraite de l'état 7 de la state machine.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_core::match_state::MatchScore;
    ///
    /// let score = MatchScore { minutes: 1, seconds: 30 };
    /// assert_eq!(score.encoded(), 10030);
    /// ```
    #[must_use]
    pub fn encoded(self) -> u32 {
        u32::from(self.minutes) * 10_000 + u32::from(self.seconds)
    }

    /// Decode une valeur encodée en `MatchScore`.
    // `encoded % 10_000` < 10000 et un score de match réel garde `minutes`
    // largement < 65536 ; la troncature `as u16` est sûre dans ce domaine.
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn decode(encoded: u32) -> Self {
        Self {
            minutes: (encoded / 10_000) as u16,
            seconds: (encoded % 10_000) as u16,
        }
    }
}

/// Phase d'un match d'entraînement (state machine principale).
///
/// Source: `soccer_match_state_machine.c` — `*(byte *)(param_1 + 0x20)` est
/// l'index de la phase, interprété via le `switch` de `FUN_1412aa4a0`.
///
/// Les libellés sont extraits des strings embarquées dans le binaire
/// (`TrainingResetWaitTimer`, `TrainingWaitEndTimer`, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum MatchPhase {
    /// Phase 0 : initialisation, démarrage du timer de reset.
    #[default]
    Init = 0,
    /// Phase 1 : attente de la fin du timer + reset des animations joueurs.
    WaitTimer = 1,
    /// Phase 2 : attente de la fin de l'UI de résultat.
    WaitResultUi = 2,
    /// Phase 3 : vérification du telop d'échec (asset `btl10_02_failure_telop`).
    CheckFailureTelop = 3,
    /// Phase 4 : attente fin animation résultat.
    WaitResultAnim = 4,
    /// Phase 5 : transition — fade standard ou logique restart/complétion.
    Transition = 5,
    /// Phase 6 : attente fin de fade.
    WaitFade = 6,
    /// Phase 7 : nettoyage final + calcul du score (`minutes * 10000 + secondes`).
    Cleanup = 7,
    /// Phase 8 : affichage UI résultat final.
    ResultUi = 8,
    /// Phase 9 : attente fade out final.
    WaitFadeOut = 9,
    /// Phase 10 : chargement de la scène suivante.
    LoadNextScene = 10,
}

impl TryFrom<u8> for MatchPhase {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, ()> {
        match v {
            0 => Ok(Self::Init),
            1 => Ok(Self::WaitTimer),
            2 => Ok(Self::WaitResultUi),
            3 => Ok(Self::CheckFailureTelop),
            4 => Ok(Self::WaitResultAnim),
            5 => Ok(Self::Transition),
            6 => Ok(Self::WaitFade),
            7 => Ok(Self::Cleanup),
            8 => Ok(Self::ResultUi),
            9 => Ok(Self::WaitFadeOut),
            10 => Ok(Self::LoadNextScene),
            _ => Err(()),
        }
    }
}

/// Contexte d'état pour la machine à états de match d'entraînement.
///
/// Source: `soccer_match_state_machine.c` — structure `param_1` passée à
/// `FUN_1412aa4a0`. Seuls les champs interprétés sont portés.
///
/// RE incertain: `flags` correspond à `*(uint *)(param_1 + 0x10)` — sens exact
/// des bits non décodé (seul le bit 2 est observé comme "reset terrain").
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrainingStateMachine {
    /// Phase courante de la machine à états.
    pub phase: MatchPhase,
    /// Compteur de résultat / type de fin.
    ///
    /// Source: `param_1 + 0x24` — utilisé comme switch `iVar18` dans l'état 5.
    pub end_kind: i32,
    /// Flags internes (bit 2 = "reset terrain actif").
    ///
    /// Source: `param_1 + 0x10` — `*(uint *)(param_1 + 0x10) | 2` dans l'état 0.
    /// RE incertain: autres bits non décodés.
    pub flags: u32,
    /// Résultat de l'entraînement (calculé en phase 1 pour les matchs spéciaux).
    pub training_result: Option<TrainingResult>,
    /// Score final calculé en phase 7.
    pub final_score: Option<MatchScore>,
}

impl TrainingStateMachine {
    /// Crée une machine à états d'entraînement à l'état initial.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Retourne la phase courante.
    #[must_use]
    pub fn phase(&self) -> MatchPhase {
        self.phase
    }

    /// Simule la transition vers l'état 1 (démarrage du timer de reset).
    ///
    /// Source: `FUN_1412aa4a0` case 0 — pose le flag bit 2 et passe à l'état 1.
    pub fn start_reset_timer(&mut self) {
        self.flags |= 0b10; // bit 1 = flag de reset terrain
        self.phase = MatchPhase::WaitTimer;
    }

    /// Simule la validation du timer et le reset vers la phase appropriée.
    ///
    /// `is_training` : `true` si c'est un match d'entraînement spécial (flag `0x4`
    /// à l'offset `0x14` du game manager). `false` = match normal → phase 5.
    ///
    /// Source: `FUN_1412aa4a0` case 1.
    pub fn on_timer_done(&mut self, is_training: bool, result: TrainingResult) {
        if is_training {
            self.training_result = Some(result);
            self.phase = MatchPhase::WaitResultUi;
        } else {
            self.phase = MatchPhase::Transition;
        }
    }

    /// Enregistre le score final (phase 7).
    ///
    /// Source: `FUN_1412aa4a0` case 7 — calcul `minutes * 10000 + secondes`.
    pub fn set_final_score(&mut self, minutes: u16, seconds: u16) {
        self.final_score = Some(MatchScore { minutes, seconds });
        self.phase = MatchPhase::ResultUi;
    }

    /// Détermine le comportement de fin d'entraînement depuis `end_kind`.
    ///
    /// Source: `FUN_1412aa4a0` case 5, switch iVar18.
    #[must_use]
    pub fn training_end_kind(&self) -> Option<TrainingEndKind> {
        TrainingEndKind::try_from(self.end_kind).ok()
    }

    /// Passe à la scène suivante (phase terminale).
    ///
    /// Source: `FUN_1412aa4a0` case 10 — retourne 1 (transition immédiate).
    pub fn load_next_scene(&mut self) {
        self.phase = MatchPhase::LoadNextScene;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_phase_roundtrip() {
        for i in 0u8..=10 {
            let phase = MatchPhase::try_from(i).expect("phase valide");
            assert_eq!(phase as u8, i);
        }
    }

    #[test]
    fn match_phase_invalid() {
        assert!(MatchPhase::try_from(11).is_err());
        assert!(MatchPhase::try_from(255).is_err());
    }

    #[test]
    fn match_score_encoded() {
        // Formule exacte: minutes * 10000 + secondes
        let s = MatchScore {
            minutes: 2,
            seconds: 45,
        };
        assert_eq!(s.encoded(), 20045);
    }

    #[test]
    fn match_score_roundtrip() {
        let orig = MatchScore {
            minutes: 3,
            seconds: 15,
        };
        let decoded = MatchScore::decode(orig.encoded());
        assert_eq!(decoded, orig);
    }

    #[test]
    fn match_score_zero() {
        let s = MatchScore {
            minutes: 0,
            seconds: 0,
        };
        assert_eq!(s.encoded(), 0);
    }

    #[test]
    fn training_result_ui_code() {
        // Confirmé: (bVar5 ^ 1) + 0x52 → success=0x52, failure=0x53
        assert_eq!(TrainingResult::Success.ui_code(), 0x52);
        assert_eq!(TrainingResult::Failure.ui_code(), 0x53);
    }

    #[test]
    fn training_end_kind_from_int() {
        assert_eq!(TrainingEndKind::try_from(0), Ok(TrainingEndKind::Restart));
        assert_eq!(TrainingEndKind::try_from(1), Ok(TrainingEndKind::Retry));
        assert_eq!(TrainingEndKind::try_from(2), Ok(TrainingEndKind::Complete));
        assert!(TrainingEndKind::try_from(3).is_err());
    }

    #[test]
    fn state_machine_init() {
        let sm = TrainingStateMachine::new();
        assert_eq!(sm.phase(), MatchPhase::Init);
        assert!(sm.training_result.is_none());
        assert!(sm.final_score.is_none());
    }

    #[test]
    fn state_machine_start_reset_timer() {
        let mut sm = TrainingStateMachine::new();
        sm.start_reset_timer();
        assert_eq!(sm.phase(), MatchPhase::WaitTimer);
        assert_ne!(sm.flags & 0b10, 0, "flag reset terrain doit etre actif");
    }

    #[test]
    fn state_machine_normal_match_goes_to_transition() {
        let mut sm = TrainingStateMachine::new();
        sm.start_reset_timer();
        sm.on_timer_done(false, TrainingResult::Success);
        assert_eq!(sm.phase(), MatchPhase::Transition);
        assert!(
            sm.training_result.is_none(),
            "pas de resultat en match normal"
        );
    }

    #[test]
    fn state_machine_training_failure() {
        let mut sm = TrainingStateMachine::new();
        sm.start_reset_timer();
        sm.on_timer_done(true, TrainingResult::Failure);
        assert_eq!(sm.phase(), MatchPhase::WaitResultUi);
        assert_eq!(sm.training_result, Some(TrainingResult::Failure));
    }

    #[test]
    fn state_machine_set_final_score() {
        let mut sm = TrainingStateMachine::new();
        sm.set_final_score(1, 30);
        let score = sm.final_score.expect("score défini");
        assert_eq!(score.encoded(), 10030);
        assert_eq!(sm.phase(), MatchPhase::ResultUi);
    }
}
