//! Calcul d'arrêt du gardien (`game::SoccerCalcKeeperSaveComponent`).
//!
//! Porte la structure d'initialisation et les paramètres de calcul depuis
//! `refs/iecode-re/research/ghidra-export/decompiled/soccer_keeper_save.c`.
//!
//! # Sources RE
//!
//! - `soccer_keeper_save.c` — `FUN_14030e5c0` (lignes nie.c 593987-594016)
//! - `soccer_keeper_save.c` — `FUN_14030e680` = `ShootSaveData::ShootSaveData()` (lignes nie.c 594022-594068)
//! - `soccer_keeper_save.c` — `FUN_14030e770` = `ShootSaveData::copy()` (lignes nie.c 594069+)
//!
//! # Fidélité
//!
//! - Paramètres flottants (rayon, distance, probabilité, vitesse, réaction) : FIABLE
//!   (tous confirmés par représentation IEEE 754 hexadécimale dans le source C)
//! - Structure `ShootSaveData` : PARTIELLEMENT FIABLE — les champs de positions
//!   et timestamps sont portés, mais la sémantique exacte de certains champs
//!   (ex. troisième vecteur, `source_id`) est incertaine
//! - Logique de calcul d'arrêt elle-même (update/evaluate) : NON PORTÉE
//!   — absente du pseudo-C disponible

use crate::{INVALID_TARGET_ID, Vec3};
use crate::{
    KEEPER_DIVE_MAX_DIST, KEEPER_DIVE_SPEED, KEEPER_REACTION_TIME_FRAMES,
    KEEPER_SAVE_PROBABILITY_BASE, KEEPER_SAVE_RADIUS_DEFAULT,
};

/// Données d'un tir à évaluer par le gardien (`ShootSaveData`).
///
/// Source: `FUN_14030e680` — sous-structure à offset `+0x070` dans
/// `SoccerCalcKeeperSaveComponent`, taille estimée ~0xF8 bytes.
///
/// # Fidélité des champs
///
/// - `shoot_position`, `shoot_direction` : reconstruits depuis les deux premiers
///   paires de qwords initialisés à `_DAT_14212dc90/_DAT_14212dc98`
///   (vecteurs zéro d'après le contexte). FIABLE comme initialisation à zéro.
/// - `keeper_position`, `dive_direction` : portés fidèlement depuis les
///   `_DAT_142115a70-7c` (vecteur zéro de référence).
/// - `impact_position`, `surface_normal` : reconstruits depuis les vecteurs
///   supplémentaires du ctor. RE incertain: offset exact dans la struct.
/// - `flags`, `state_byte` : portés comme u32/u8. RE incertain: sémantique bits.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShootSaveData {
    /// Position de départ du tir.
    pub shoot_position: Vec3,
    /// Direction du tir (normalisée en théorie).
    pub shoot_direction: Vec3,
    /// Flags d'état du tir.
    ///
    /// Source: `*(undefined4 *)(param_1 + 3) = 0` dans `FUN_14030e680`.
    /// RE incertain: signification des bits.
    pub flags: u32,
    /// Byte d'état supplémentaire.
    ///
    /// Source: `*(undefined1 *)((longlong)param_1 + 0x1c) = 0`.
    /// RE incertain: rôle exact inconnu.
    pub state_byte: u8,
    /// Position courante du gardien.
    pub keeper_position: Vec3,
    /// Direction de plongeon du gardien.
    pub dive_direction: Vec3,
    /// Second vecteur de plongeon (axe latéral?).
    ///
    /// RE incertain: rôle du second vecteur non confirmé.
    pub dive_lateral: Vec3,
    /// Troisième vecteur (impact? normale?).
    ///
    /// Source: troisième vecteur dans la série `_DAT_142115a70-7c` du ctor.
    /// RE incertain: rôle non décodé.
    pub aux_vector: Vec3,
    /// Position d'impact du ballon (sur le corps du gardien?).
    ///
    /// Source: vecteurs à offset 0xB0-0xD0 selon commentaire Ghidra.
    /// RE incertain: offset exact dans `ShootSaveData` non confirmé.
    pub impact_position: Vec3,
    /// Normale de la surface d'impact.
    ///
    /// RE incertain: déduit du pattern, non confirmé.
    pub surface_normal: Vec3,
}

impl ShootSaveData {
    /// Copie partielle (22 itérations de 2 words).
    ///
    /// Source: `FUN_14030e770` — `lVar2 = 0x16` (22) itérations de 2 words.
    /// La copie porte tous les champs de la struct.
    #[must_use]
    pub fn copy_from(src: &Self) -> Self {
        src.clone()
    }
}

/// Données de possession/interception associées au tir.
///
/// Source: `soccer_keeper_save.c` — champ intermédiaire dans la structure principale.
/// RE incertain: structure exacte non décodée, seul l'ID de cible est certain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeeperTargetRef {
    /// ID de la cible principale (`INVALID_TARGET_ID` = aucune).
    pub target_id: u32,
}

impl KeeperTargetRef {
    /// Crée une référence vide.
    #[must_use]
    pub fn none() -> Self {
        Self {
            target_id: INVALID_TARGET_ID,
        }
    }

    /// Retourne `true` si aucune cible n'est définie.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.target_id == INVALID_TARGET_ID
    }
}

/// Composant de calcul d'arrêt du gardien (`game::SoccerCalcKeeperSaveComponent`).
///
/// Source: `soccer_keeper_save.c` — `FUN_14030e5c0`.
/// Taille originale estimée ~`0x180` bytes.
///
/// # Paramètres physiques
///
/// Tous les flottants sont confirmés par représentation IEEE 754 hexadécimale
/// dans le source C décompilé (aucune ambiguïté):
///
/// | Champ                    | Hex IEEE 754 | Valeur f32 |
/// |--------------------------|-------------|------------|
/// | `save_radius`            | `0x3F800000` | 1.0        |
/// | `max_dive_distance`      | `0x40A00000` | 5.0        |
/// | `save_probability`       | `0x3F4CCCCD` | ≈0.8       |
/// | `reaction_time_frames`   | `0x40975C29` | ≈4.73      |
/// | `dive_speed`             | `0x402AE148` | ≈2.67      |
///
/// # Exemple
///
/// ```
/// use nie_core::keeper::KeeperSaveComponent;
///
/// let keeper = KeeperSaveComponent::new();
/// // Rayon d'arrêt = 1.0f, confirmé par 0x3F800000
/// assert_eq!(keeper.save_radius.to_bits(), 0x3F80_0000);
/// // Distance plongeon = 5.0f, confirmé par 0x40A00000
/// assert_eq!(keeper.max_dive_distance.to_bits(), 0x40A0_0000);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeeperSaveComponent {
    /// Identifiant du composant (dword, init 0).
    ///
    /// Source: `*(undefined4 *)(param_1 + 0xc) = 0`.
    pub component_id: u32,
    /// Données du tir actif à évaluer.
    pub shoot_data: ShootSaveData,
    /// Rayon de capture du gardien (zone autour du body).
    pub save_radius: f32,
    /// Distance maximale de plongeon.
    pub max_dive_distance: f32,
    /// Probabilité de base d'arrêt (0.0 – 1.0).
    pub save_probability: f32,
    /// Temps de réaction en frames (environ 4.73 frames à 60Hz).
    ///
    /// RE incertain: l'unité exacte n'est pas confirmée.
    pub reaction_time_frames: f32,
    /// Vitesse de plongeon (unités/frame).
    ///
    /// RE incertain: unité non confirmée.
    pub dive_speed: f32,
}

impl Default for KeeperSaveComponent {
    fn default() -> Self {
        Self {
            component_id: 0,
            shoot_data: ShootSaveData::default(),
            save_radius: KEEPER_SAVE_RADIUS_DEFAULT,
            max_dive_distance: KEEPER_DIVE_MAX_DIST,
            save_probability: KEEPER_SAVE_PROBABILITY_BASE,
            reaction_time_frames: KEEPER_REACTION_TIME_FRAMES,
            dive_speed: KEEPER_DIVE_SPEED,
        }
    }
}

impl KeeperSaveComponent {
    /// Crée un composant avec les paramètres par défaut du constructeur C++.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Estime la distance maximale parcourue pendant le temps de réaction.
    ///
    /// `distance_reaction = dive_speed * reaction_time_frames`
    ///
    /// Note: ce calcul est **reconstruit** — il n'est pas directement visible
    /// dans le pseudo-C disponible. C'est une déduction logique des paramètres.
    /// RE incertain: la formule réelle de calcul d'arrêt n'est pas portée.
    #[must_use]
    pub fn reaction_distance(&self) -> f32 {
        self.dive_speed * self.reaction_time_frames
    }

    /// Retourne `true` si le gardien peut potentiellement atteindre la position
    /// `target` depuis `keeper_position` (heuristique basée sur `max_dive_distance`).
    ///
    /// RE incertain: la vraie condition de `evaluate()` dans le jeu implique
    /// probablement des vecteurs de trajectoire, pas juste la distance euclidienne.
    #[must_use]
    pub fn can_reach(&self, target: crate::Vec3) -> bool {
        let keeper_pos = self.shoot_data.keeper_position;
        let dx = target.x - keeper_pos.x;
        let dy = target.y - keeper_pos.y;
        let dz = target.z - keeper_pos.z;
        let dist_sq = dx * dx + dy * dy + dz * dz;
        dist_sq <= self.max_dive_distance * self.max_dive_distance
    }

    /// Portée² d'arrêt **réelle** depuis les champs que le binaire lit (`FUN_1413dcfe0` : 3 floats à
    /// `+0x170`/`+0x174`/`+0x178` = `save_radius`/`max_dive_distance`/`save_probability`, utilisés comme
    /// dimensions de plongeon) × `scale`. Délègue à [`keeper_reach_sq`] (byte-exact). `scale` = consts
    /// monde runtime, fourni par le contexte.
    #[must_use]
    pub fn reach_sq(&self, scale: f32) -> f32 {
        keeper_reach_sq(
            crate::Vec3 {
                x: self.save_radius,
                y: self.max_dive_distance,
                z: self.save_probability,
            },
            scale,
        )
    }

    /// `true` si le gardien atteint le ballon : distance horizontale² (XZ) `<` portée². Combine
    /// [`Self::reach_sq`] et [`is_within_save_reach`] (toutes deux byte-exact vs binaire) — la vraie
    /// décision d'arrêt géométrique (≠ heuristique [`Self::can_reach`]).
    #[must_use]
    pub fn attempts_save(&self, ball: crate::Vec3, keeper: crate::Vec3, scale: f32) -> bool {
        is_within_save_reach(ball, keeper, self.reach_sq(scale))
    }
}

/// Décision d'arrêt géométrique **réelle** du gardien, portée du C décompilé `FUN_1413dcfe0`
/// (calc d'arrêt, slot 10 de la vftable). **À distinguer de l'heuristique [`KeeperSaveComponent::can_reach`]**
/// (3D euclidienne `≤`, une reconstruction non validée) : le binaire teste la **distance horizontale²
/// (plan XZ uniquement)** STRICTEMENT inférieure au rayon de portée².
///
/// Ordre f32 du binaire : `dz² + dx²` (la composante Y est ignorée). `reach_sq` (portée²) dérive des
/// paramètres de plongeon × constantes monde **runtime** (`DAT_142060e00·DAT_142060e10`) → fourni par
/// le contexte (non statiquement résoluble, pattern ECS-init).
#[must_use]
pub fn is_within_save_reach(ball: crate::Vec3, keeper: crate::Vec3, reach_sq: f32) -> bool {
    let dx = ball.x - keeper.x;
    let dz = ball.z - keeper.z;
    (dz * dz + dx * dx) < reach_sq
}

/// Portée² d'arrêt du gardien (`FUN_1413dcfe0`, byte-exact via uemu — `scripts/validate_keeper_reach.py`) :
/// **minimum** sur les composantes de `dive · scale` (mulps puis `cmpps`+`blendvps`). `dive` = paramètres
/// de plongeon (champs propres du composant, `+0x170`..) ; `scale` = produit de consts monde **runtime**
/// (`DAT_142060e00·DAT_142060e10`) → fourni par le contexte. Combiner avec [`is_within_save_reach`].
#[must_use]
pub fn keeper_reach_sq(dive: crate::Vec3, scale: f32) -> f32 {
    let (px, py, pz) = (dive.x * scale, dive.y * scale, dive.z * scale);
    let mut m = px; // min des 3 produits (ordre cmpps/blendvps : ties → 2e opérande)
    if py < m {
        m = py;
    }
    if pz < m {
        m = pz;
    }
    m
}

/// Hauteurs de zone d'arrêt du gardien (`FUN_1413dcfe0`, écrites à `+0xb0`/`+0xb4`/`+0xb8`) : pour
/// trois hauteurs de tir, `max(0, hauteur_tir − y_gardien)` (clamp `≤ 0 → 0`). Pur, byte-fidèle.
#[must_use]
pub fn save_zone_heights(shot_heights: [f32; 3], keeper_y: f32) -> [f32; 3] {
    shot_heights.map(|h| {
        let v = h - keeper_y;
        if v <= 0.0 { 0.0 } else { v }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        KEEPER_DIVE_MAX_DIST, KEEPER_DIVE_SPEED, KEEPER_SAVE_PROBABILITY_BASE,
        KEEPER_SAVE_RADIUS_DEFAULT,
    };

    #[test]
    fn keeper_save_radius_ieee754() {
        let k = KeeperSaveComponent::new();
        // 0x3F800000 = 1.0f — confirmé par soccer_keeper_save.c
        assert_eq!(k.save_radius.to_bits(), 0x3F80_0000);
        assert_eq!(k.save_radius, KEEPER_SAVE_RADIUS_DEFAULT);
    }

    #[test]
    fn save_reach_xz_strict_vs_decompile() {
        // FUN_1413dcfe0 : distance horizontale² (XZ, Y ignoré) STRICTEMENT < portée².
        let ball = crate::Vec3 {
            x: 3.0,
            y: 99.0,
            z: 4.0,
        }; // Y volontairement grand → ignoré
        let keeper = crate::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        // dz²+dx² = 16+9 = 25.
        assert!(is_within_save_reach(ball, keeper, 26.0)); // 25 < 26
        assert!(!is_within_save_reach(ball, keeper, 25.0)); // strict : 25 < 25 faux
        assert!(!is_within_save_reach(ball, keeper, 24.0));
    }

    #[test]
    fn keeper_attempts_save_integration() {
        // reach_sq = min(save_radius=1, max_dive=5, save_proba=0.8)·scale = 0.8·scale.
        let k = KeeperSaveComponent::new();
        assert_eq!(k.reach_sq(1.0).to_bits(), 0.8_f32.to_bits());
        // ballon à dist² horizontale 0.49 (dx=0.7) < 0.8 → arrêt ; à 0.81 (dx=0.9) → non.
        let kp = crate::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        assert!(k.attempts_save(
            crate::Vec3 {
                x: 0.7,
                y: 9.0,
                z: 0.0
            },
            kp,
            1.0
        ));
        assert!(!k.attempts_save(
            crate::Vec3 {
                x: 0.9,
                y: 0.0,
                z: 0.0
            },
            kp,
            1.0
        ));
    }

    #[test]
    fn keeper_reach_sq_min_vs_decompile() {
        // FUN_1413dcfe0 : min des produits (dive·scale). Validé byte-exact vs uemu (validate_keeper_reach.py).
        let d = crate::Vec3 {
            x: 3.0,
            y: 5.0,
            z: 4.0,
        };
        assert_eq!(keeper_reach_sq(d, 1.0).to_bits(), 3.0_f32.to_bits()); // min(3,5,4)
        assert_eq!(
            keeper_reach_sq(
                crate::Vec3 {
                    x: 8.0,
                    y: 2.0,
                    z: 3.0
                },
                1.5
            )
            .to_bits(),
            3.0_f32.to_bits()
        ); // min(12,3,4.5)=3
    }

    #[test]
    fn save_zone_heights_clamp() {
        // FUN_1413dcfe0 : max(0, hauteur_tir − y_gardien) pour 3 hauteurs.
        let z = save_zone_heights([10.0, 3.0, 0.0], 5.0);
        assert_eq!(z[0].to_bits(), 5.0_f32.to_bits()); // 10-5=5
        assert_eq!(z[1].to_bits(), 0.0_f32.to_bits()); // 3-5=-2 → 0
        assert_eq!(z[2].to_bits(), 0.0_f32.to_bits()); // 0-5=-5 → 0
    }

    #[test]
    fn keeper_dive_distance_ieee754() {
        let k = KeeperSaveComponent::new();
        // 0x40A00000 = 5.0f — confirmé par soccer_keeper_save.c
        assert_eq!(k.max_dive_distance.to_bits(), 0x40A0_0000);
        assert_eq!(k.max_dive_distance, KEEPER_DIVE_MAX_DIST);
    }

    #[test]
    fn keeper_probability_ieee754() {
        let k = KeeperSaveComponent::new();
        // 0x3F4CCCCD ≈ 0.8f — confirmé par soccer_keeper_save.c
        assert_eq!(k.save_probability.to_bits(), 0x3F4C_CCCD);
        assert!((k.save_probability - KEEPER_SAVE_PROBABILITY_BASE).abs() < 1e-6);
    }

    #[test]
    fn keeper_reaction_time_ieee754() {
        let k = KeeperSaveComponent::new();
        // 0x40975C29 ≈ 4.73f — confirmé par soccer_keeper_save.c
        assert_eq!(k.reaction_time_frames.to_bits(), 0x4097_5C29);
    }

    #[test]
    fn keeper_dive_speed_ieee754() {
        let k = KeeperSaveComponent::new();
        // 0x402AE148 ≈ 2.67f — confirmé par soccer_keeper_save.c
        assert_eq!(k.dive_speed.to_bits(), 0x402A_E148);
        assert!((k.dive_speed - KEEPER_DIVE_SPEED).abs() < 1e-4);
    }

    #[test]
    fn keeper_component_id_zero() {
        let k = KeeperSaveComponent::new();
        // *(undefined4 *)(param_1 + 0xc) = 0
        assert_eq!(k.component_id, 0);
    }

    #[test]
    fn keeper_reaction_distance() {
        let k = KeeperSaveComponent::new();
        // dive_speed * reaction_time = 2.67 * 4.73 ≈ 12.63
        let d = k.reaction_distance();
        assert!(d > 10.0 && d < 15.0);
    }

    #[test]
    fn keeper_can_reach_near() {
        let mut k = KeeperSaveComponent::new();
        k.shoot_data.keeper_position = Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        // Cible à 3.0 unités — dans la portée de 5.0
        let target = Vec3 {
            x: 3.0,
            y: 0.0,
            z: 0.0,
        };
        assert!(k.can_reach(target));
    }

    #[test]
    fn keeper_cannot_reach_far() {
        let mut k = KeeperSaveComponent::new();
        k.shoot_data.keeper_position = Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        // Cible à 10.0 unités — hors portée de 5.0
        let target = Vec3 {
            x: 10.0,
            y: 0.0,
            z: 0.0,
        };
        assert!(!k.can_reach(target));
    }

    #[test]
    fn shoot_save_data_default_zeroed() {
        let d = ShootSaveData::default();
        assert_eq!(d.shoot_position, Vec3::zero());
        assert_eq!(d.keeper_position, Vec3::zero());
        assert_eq!(d.flags, 0);
        assert_eq!(d.state_byte, 0);
    }

    #[test]
    fn keeper_target_ref_none() {
        let r = KeeperTargetRef::none();
        assert!(r.is_none());
        assert_eq!(r.target_id, INVALID_TARGET_ID);
    }
}
