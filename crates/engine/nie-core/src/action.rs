//! Contrôleur d'actions joueur (`game::SoccerActionCtrl`).
//!
//! Porte la structure de slots d'action et l'initialisation depuis
//! `refs/iecode-re/research/ghidra-export/decompiled/soccer_action_ctrl.c`.
//!
//! # Sources RE
//!
//! - `soccer_action_ctrl.c` — `FUN_1412ca580` = `SoccerActionCtrl::SoccerActionCtrl()` (lignes nie.c 3467044-3467078)
//! - `soccer_action_ctrl.c` — `FUN_1412ca660` = `copy_action()` (lignes nie.c 3467082-3467128)
//! - `soccer_action_ctrl.c` — `FUN_1412ca7f0` = `init_transform()` (lignes nie.c 3467134-3467231)
//!
//! # Fidélité
//!
//! - 32 slots, stride 0x120 (288 bytes) : FIABLE (`lVar2 = 0x20`, `lVar1 + 0x120`)
//! - Données par slot = 0x118 bytes + 2 bytes état : FIABLE
//! - Scale identité (1.0f) dans `init_transform()` : FIABLE (bits IEEE 754 explicites)
//! - Taille totale `0xA0` pour `copy_action()` : FIABLE (`FUN_1416709b0(param_1, 0, 0xa0)`)
//! - Contenu interne des slots d'action (layout position/direction/collision) : RECONSTRUIT
//!   — déduit du commentaire Ghidra, non confirmé byte-par-byte

/// Nombre de slots d'action simultanés par joueur.
///
/// Source: `soccer_action_ctrl.c` — `lVar2 = 0x20` (32 itérations).
pub const ACTION_SLOT_COUNT: usize = 32;

/// Taille de la zone de données d'une action (bytes).
///
/// Source: `FUN_1412ca7f0` — la structure copiée dans `copy_action()` est `0xA0 = 160` bytes.
pub const ACTION_DATA_SIZE: usize = 0xA0;

/// État d'un slot d'action.
///
/// Source: `soccer_action_ctrl.c` — `*(undefined2 *)(lVar1 + -1) = 0` dans la boucle d'init.
/// `0` = libre, non-nul = en cours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ActionSlotState {
    /// Slot disponible.
    #[default]
    Free,
    /// Action en cours dans ce slot.
    Active,
}

/// Données brutes d'un slot d'action (160 bytes opaque).
///
/// La structure interne de ces 160 bytes est partiellement reconstruite
/// depuis les commentaires Ghidra mais non vérifiée byte-par-byte.
/// Stockée comme tableau d'octets pour refléter fidèlement l'opacité RE.
///
/// RE incertain: layout interne des 160 bytes. Le commentaire Ghidra suggère:
///
/// - `+0x00`: type d'action (enum)
/// - `+0x10-0x30`: position 3D cible
/// - `+0x30-0x50`: direction
/// - `+0x50-0xA0`: données collision/résultat
///
/// Mais ce n'est pas vérifié par le code C décompilé.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActionData {
    /// Données brutes (160 bytes, initialisées à 0).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_byte_array"))]
    pub raw: [u8; ACTION_DATA_SIZE],
}

impl Default for ActionData {
    fn default() -> Self {
        Self {
            raw: [0u8; ACTION_DATA_SIZE],
        }
    }
}

impl core::fmt::Debug for ActionData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ActionData")
            .field("raw[0..4]", &&self.raw[..4])
            .finish()
    }
}

impl ActionData {
    /// Copie les données depuis une autre `ActionData` (copie complète).
    ///
    /// Source: `FUN_1412ca660` — copie de 20 qwords (0xA0 bytes) via
    /// affectations directes pairées. Retourne une erreur si source nulle.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_core::action::ActionData;
    ///
    /// let mut src = ActionData::default();
    /// src.raw[0] = 42;
    /// let dst = ActionData::copy_from(&src);
    /// assert_eq!(dst.raw[0], 42);
    /// ```
    #[must_use]
    pub fn copy_from(src: &Self) -> Self {
        Self { raw: src.raw }
    }

    /// Remet toutes les données à zéro (équivalent à `memset(0, 0xA0)`).
    pub fn clear(&mut self) {
        self.raw = [0u8; ACTION_DATA_SIZE];
    }

    /// Copie fidèle de `copy_action` (`FUN_1412ca660`) avec sémantique EINVAL.
    ///
    /// Source : `soccer_action_ctrl.c` L106-135. Reproduit EXACTEMENT le contrat :
    /// - dest nulle (`param_1 == 0`) → erreur `0x16` (EINVAL), rien copié.
    /// - dest non nulle, source nulle (`param_3 == 0`) → la dest est mise à zéro
    ///   (`memset(param_1, 0, 0xA0)`) PUIS erreur `0x16` est renvoyée.
    /// - source ET dest non nulles → copie de 0xA0 bytes (20 qwords), `Ok(())`.
    ///
    /// En Rust `self` (dest) n'est jamais nul ; on modélise donc la dest nulle
    /// via [`SoccerActionCtrl::copy_action`]. Ici on traite le cas source nulle.
    ///
    /// # Errors
    ///
    /// Renvoie `Err(0x16)` (EINVAL) si `src` est `None` ; la dest est alors
    /// remise à zéro, fidèle au `memset` du décompilé.
    pub fn copy_action(&mut self, src: Option<&Self>) -> Result<(), u32> {
        if let Some(s) = src {
            // Copie des 0xA0 bytes = 20 qwords (affectations pairées du C).
            self.raw = s.raw;
            Ok(())
        } else {
            // Source nulle : mise à zéro de la dest puis EINVAL.
            self.clear();
            Err(EINVAL)
        }
    }
}

/// Code d'erreur `EINVAL` (0x16) renvoyé par `copy_action`/`init_transform`.
///
/// Source : `soccer_action_ctrl.c` — `*puVar2 = 0x16` (L132, L156).
pub const EINVAL: u32 = 0x16;

/// Un slot d'action individuel (partie d'un `SoccerActionCtrl`).
///
/// Source: `soccer_action_ctrl.c` — stride `0x120` bytes par slot.
/// Structure: `0x118` bytes données + `2` bytes état.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActionSlot {
    /// Données de l'action (parties visibles = 0xA0 bytes au minimum).
    pub data: ActionData,
    /// État du slot (libre ou actif).
    pub state: ActionSlotState,
}

impl ActionSlot {
    /// Libère le slot (réinitialise données + état).
    pub fn free(&mut self) {
        self.data.clear();
        self.state = ActionSlotState::Free;
    }

    /// Retourne `true` si le slot est disponible.
    #[must_use]
    pub fn is_free(&self) -> bool {
        self.state == ActionSlotState::Free
    }
}

/// Données de transformation pour le rendu d'un slot.
///
/// Source: `FUN_1412ca7f0` (`init_transform`) — initialisées avec des scales
/// à 1.0f et des positions/rotations à zéro.
///
/// RE incertain: les 3 paires position/orientation puis les 6 scales sont
/// déduites du pseudo-C mais le nombre exact de vecteurs n'est pas absolu.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActionTransform {
    /// 3 paires position/rotation, initialisées à zéro.
    ///
    /// Source: `FUN_1412ca7f0` L162-167 — `*param_1 = 0` … `param_1[5] = 0`
    /// (6 qwords = 3 paires (0, 0)).
    pub positions: [[f32; 2]; 3],
    /// 6 paires d'échelle (1.0f, 1.0f).
    ///
    /// Source: `param_1[6..11] = 0x3f8000003f800000` (deux f32 = 1.0 dans un qword).
    /// Valeur exacte confirmée par les bits IEEE 754.
    pub scales: [[f32; 2]; 6],
}

impl Default for ActionTransform {
    fn default() -> Self {
        Self::init_transform()
    }
}

impl ActionTransform {
    /// Reproduit `init_transform` (`FUN_1412ca7f0`, L144-192).
    ///
    /// Pose 3 paires position/rotation à zéro (L162-167) et 6 paires d'échelle
    /// à `(1.0, 1.0)` = `0x3F800000` en IEEE 754 (L171-176).
    #[must_use]
    pub fn init_transform() -> Self {
        Self {
            positions: [[0.0f32, 0.0f32]; 3],
            scales: [[1.0f32, 1.0f32]; 6],
        }
    }

    /// Vérifie que toutes les scales sont à l'identité (1.0).
    #[must_use]
    pub fn is_identity(&self) -> bool {
        // Comparaison exacte par bits IEEE 754 (1.0f = 0x3F800000) — on veut
        // l'égalité stricte aux valeurs initialisées, pas une tolérance.
        self.scales
            .iter()
            .all(|&[a, b]| a.to_bits() == 0x3F80_0000 && b.to_bits() == 0x3F80_0000)
    }
}

/// Contrôleur d'actions d'un joueur (`game::SoccerActionCtrl`).
///
/// Contient 32 slots d'action, un compteur global et un flag d'état.
///
/// Source: `soccer_action_ctrl.c` — taille originale ~0x2488 (9352 bytes).
///
/// # Exemple
///
/// ```
/// use nie_core::action::{SoccerActionCtrl, ActionSlotState};
///
/// let ctrl = SoccerActionCtrl::new();
/// assert_eq!(ctrl.active_count, 0);
/// assert!(ctrl.slots.iter().all(|s| s.is_free()));
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerActionCtrl {
    /// 32 slots d'action simultanés.
    pub slots: [ActionSlot; ACTION_SLOT_COUNT],
    /// Compteur d'actions actives.
    ///
    /// Source: `*(undefined4 *)(param_1 + 0x48e) = 0` dans le ctor.
    pub active_count: u32,
    /// Flag global d'état du contrôleur.
    ///
    /// Source: `*(undefined1 *)(param_1 + 0x490) = 0` dans le ctor.
    pub global_flag: bool,
}

impl Default for SoccerActionCtrl {
    fn default() -> Self {
        Self {
            slots: core::array::from_fn(|_| ActionSlot::default()),
            active_count: 0,
            global_flag: false,
        }
    }
}

impl SoccerActionCtrl {
    /// Crée un contrôleur vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Retourne le premier slot libre, ou `None` si tous sont occupés.
    #[must_use]
    pub fn first_free_slot(&self) -> Option<usize> {
        self.slots.iter().position(ActionSlot::is_free)
    }

    /// Copie une action vers le slot `dst_idx` depuis une source optionnelle.
    ///
    /// Modélise `copy_action` (`FUN_1412ca660`) au niveau contrôleur :
    /// - `dst_idx` hors borne (équivalent dest nulle `param_1 == 0`) → `Err(0x16)`,
    ///   aucun slot modifié.
    /// - `src == None` (source nulle) → le slot dest est mis à zéro puis `Err(0x16)`.
    /// - `src == Some` → copie de 0xA0 bytes, `Ok(())`.
    ///
    /// # Errors
    ///
    /// Renvoie `Err(0x16)` (EINVAL) si `dst_idx` est hors borne ou si `src` est
    /// `None`.
    pub fn copy_action(&mut self, dst_idx: usize, src: Option<&ActionData>) -> Result<(), u32> {
        // dest nulle : index hors borne → EINVAL, rien touché.
        let Some(slot) = self.slots.get_mut(dst_idx) else {
            return Err(EINVAL);
        };
        slot.data.copy_action(src)
    }

    /// Compte les slots actifs (vérifié depuis `active_count`).
    ///
    /// Ce compteur est maintenu côté C++ — en Rust on le resynchronise
    /// si nécessaire via cette méthode.
    #[must_use]
    pub fn count_active(&self) -> usize {
        self.slots.iter().filter(|s| !s.is_free()).count()
    }

    /// Libère tous les slots (reset complet).
    pub fn reset_all(&mut self) {
        for slot in &mut self.slots {
            slot.free();
        }
        self.active_count = 0;
        self.global_flag = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ctrl_default_all_free() {
        let ctrl = SoccerActionCtrl::new();
        assert!(ctrl.slots.iter().all(|s| s.is_free()));
        assert_eq!(ctrl.active_count, 0);
        assert!(!ctrl.global_flag);
    }

    #[test]
    fn action_ctrl_slot_count() {
        let ctrl = SoccerActionCtrl::new();
        // 32 slots exactement — confirmé par lVar2 = 0x20
        assert_eq!(ctrl.slots.len(), 32);
    }

    #[test]
    fn action_slot_free_then_active() {
        let mut slot = ActionSlot::default();
        assert!(slot.is_free());
        slot.state = ActionSlotState::Active;
        assert!(!slot.is_free());
        slot.free();
        assert!(slot.is_free());
    }

    #[test]
    fn action_data_copy() {
        let mut src = ActionData::default();
        src.raw[0] = 0xDE;
        src.raw[1] = 0xAD;
        let dst = ActionData::copy_from(&src);
        assert_eq!(dst.raw[0], 0xDE);
        assert_eq!(dst.raw[1], 0xAD);
    }

    #[test]
    fn action_data_size_correct() {
        // 0xA0 = 160 bytes — confirmé par FUN_1412ca660 copie de 20 qwords
        assert_eq!(core::mem::size_of::<ActionData>(), 160);
    }

    #[test]
    fn action_transform_identity() {
        // 0x3f8000003f800000 = (1.0f, 1.0f) × 6 — confirmé par FUN_1412ca7f0
        let t = ActionTransform::default();
        assert!(t.is_identity());
        // Vérifie les bits IEEE 754
        for pair in &t.scales {
            assert_eq!(pair[0].to_bits(), 0x3F80_0000);
            assert_eq!(pair[1].to_bits(), 0x3F80_0000);
        }
    }

    #[test]
    fn init_transform_positions_zero_scales_one() {
        // init_transform pose 6 scales == 1.0f (bits 0x3F800000) et positions à 0.
        let t = ActionTransform::init_transform();
        for pair in &t.positions {
            assert_eq!(pair[0], 0.0);
            assert_eq!(pair[1], 0.0);
        }
        assert_eq!(t.scales.len(), 6);
        for pair in &t.scales {
            assert_eq!(pair[0].to_bits(), 0x3F80_0000);
            assert_eq!(pair[1].to_bits(), 0x3F80_0000);
        }
    }

    #[test]
    fn copy_action_some_copies_data() {
        let mut src = ActionData::default();
        src.raw[3] = 0x7E;
        let mut dst = ActionData::default();
        assert_eq!(dst.copy_action(Some(&src)), Ok(()));
        assert_eq!(dst.raw[3], 0x7E);
    }

    #[test]
    fn copy_action_none_zeroes_and_errors() {
        // Source nulle → slot remis à zéro PUIS Err(0x16).
        let mut dst = ActionData::default();
        dst.raw[0] = 0xAB;
        dst.raw[10] = 0xCD;
        assert_eq!(dst.copy_action(None), Err(EINVAL));
        assert_eq!(EINVAL, 0x16);
        // Mis à zéro fidèle au memset du décompilé.
        assert!(dst.raw.iter().all(|&b| b == 0));
    }

    #[test]
    fn ctrl_copy_action_out_of_bounds_is_einval() {
        // dest nulle (index hors borne) → Err(0x16), rien modifié.
        let mut ctrl = SoccerActionCtrl::new();
        let src = ActionData::default();
        assert_eq!(ctrl.copy_action(99, Some(&src)), Err(EINVAL));
    }

    #[test]
    fn ctrl_copy_action_none_zeroes_slot() {
        let mut ctrl = SoccerActionCtrl::new();
        ctrl.slots[0].data.raw[0] = 0xFF;
        assert_eq!(ctrl.copy_action(0, None), Err(EINVAL));
        assert!(ctrl.slots[0].data.raw.iter().all(|&b| b == 0));
    }

    #[test]
    fn action_ctrl_first_free_slot() {
        let mut ctrl = SoccerActionCtrl::new();
        assert_eq!(ctrl.first_free_slot(), Some(0));
        ctrl.slots[0].state = ActionSlotState::Active;
        assert_eq!(ctrl.first_free_slot(), Some(1));
    }

    #[test]
    fn action_ctrl_count_active() {
        let mut ctrl = SoccerActionCtrl::new();
        assert_eq!(ctrl.count_active(), 0);
        ctrl.slots[0].state = ActionSlotState::Active;
        ctrl.slots[5].state = ActionSlotState::Active;
        assert_eq!(ctrl.count_active(), 2);
    }

    #[test]
    fn action_ctrl_reset_all() {
        let mut ctrl = SoccerActionCtrl::new();
        ctrl.slots[0].state = ActionSlotState::Active;
        ctrl.active_count = 1;
        ctrl.global_flag = true;
        ctrl.reset_all();
        assert_eq!(ctrl.count_active(), 0);
        assert_eq!(ctrl.active_count, 0);
        assert!(!ctrl.global_flag);
    }
}
