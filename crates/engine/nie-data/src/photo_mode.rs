//! `PhotoModeRandomPoseConfig` — port Rust de
//! `photo_mode_random_pose_config.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `/home/ubuntu/niers/data/common/gamedata/photo_mode/photo_mode_random_pose_config.cfg.bin.json`
//! - 1 liste dans ce fichier :
//!   - `m_randomPoseList` — 91 entrées `RANDOM_POSE`, chacune décrivant une pose
//!     (animation de motion) tirée au hasard pour un personnage en mode photo.
//!
//! Note : le parser inagle de référence cité par le workflow
//! (`packages/inagle/src/parsers/activity-photo-config.ts`) couvre un AUTRE fichier
//! (`trophy/trophy_config_*`, layout `entries`) ; il ne s'applique pas à cette famille.
//! Le dump JSON `lists` prime et fait foi.
//!
//! ## Champs observés sur le dump réel
//!
//! | Champ           | Type JSON  | Sémantique observée                                          |
//! |-----------------|------------|--------------------------------------------------------------|
//! | `charaBodyType` | integer    | Type de corps du personnage (0..17), ou `255` = joker/tout    |
//! | `motionNameCrc` | hex string | Hash CRC du nom de l'animation de pose (ex. `"0xF8CC8EDE"`)   |
//!
//! Le dump regroupe les entrées par `charaBodyType` réel (0,1,2,…) ; chaque groupe est
//! suivi d'entrées `charaBodyType = 255` qui partagent visiblement le contexte du groupe
//! précédent (poses « génériques » applicables). On préserve l'ordre brut du fichier
//! sans réinterpréter cette sémantique de groupement.
//!
//! ## Accesseurs
//!
//! - [`PhotoModeRandomPoseConfig::find_by_motion`] — recherche par `motionNameCrc`.
//! - [`PhotoModeRandomPoseConfig::poses_for_body_type`] — filtre par `charaBodyType`.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, list_values};
use crate::hash::HashId;

// ─── RandomPose ─────────────────────────────────────────────────────────────

/// Entrée `RANDOM_POSE` — une pose aléatoire de mode photo IEVR.
///
/// Vérité terrain : `photo_mode_random_pose_config.cfg.bin.json`, liste `m_randomPoseList`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RandomPose {
    /// `charaBodyType` — type de corps du personnage (0..17 observés), ou `255`
    /// pour une entrée joker/générique.
    ///
    /// Vérité terrain : `m_randomPoseList[0].charaBodyType = 0`.
    pub chara_body_type: i64,
    /// `motionNameCrc` — hash CRC du nom de l'animation de pose.
    ///
    /// Vérité terrain : `m_randomPoseList[0].motionNameCrc = "0xF8CC8EDE"`.
    pub motion_name_crc: HashId,
}

impl RandomPose {
    /// Parse une entrée de `m_randomPoseList`. `None` si `motionNameCrc` est nul ou absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let motion_name_crc = field_hash(v, "motionNameCrc");
        if motion_name_crc.is_zero() {
            return None;
        }
        Some(Self {
            chara_body_type: field_i64(v, "charaBodyType").unwrap_or(0),
            motion_name_crc,
        })
    }
}

// ─── PhotoModeRandomPoseConfig ──────────────────────────────────────────────

/// Contenu complet d'un `photo_mode_random_pose_config.cfg.bin.json`.
///
/// Utiliser [`parse_photo_mode_random_pose_config`] pour construire depuis un
/// `serde_json::Value`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhotoModeRandomPoseConfig {
    /// Entrées `m_randomPoseList` dans l'ordre brut du fichier (91 dans le dump réel).
    pub poses: Vec<RandomPose>,
}

impl PhotoModeRandomPoseConfig {
    /// Recherche la première entrée ayant ce `motionNameCrc`. `None` si absente.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::photo_mode::{PhotoModeRandomPoseConfig, RandomPose};
    /// use nie_data::hash::HashId;
    ///
    /// let config = PhotoModeRandomPoseConfig {
    ///     poses: vec![RandomPose {
    ///         chara_body_type: 0,
    ///         motion_name_crc: HashId(0xF8CC_8EDE),
    ///     }],
    /// };
    /// assert!(config.find_by_motion(HashId(0xF8CC_8EDE)).is_some());
    /// assert!(config.find_by_motion(HashId(0xDEAD_BEEF)).is_none());
    /// ```
    #[must_use]
    pub fn find_by_motion(&self, motion_name_crc: HashId) -> Option<&RandomPose> {
        self.poses
            .iter()
            .find(|p| p.motion_name_crc == motion_name_crc)
    }

    /// Renvoie toutes les poses ayant ce `charaBodyType` exact (sans joker `255`).
    #[must_use]
    pub fn poses_for_body_type(&self, chara_body_type: i64) -> Vec<&RandomPose> {
        self.poses
            .iter()
            .filter(|p| p.chara_body_type == chara_body_type)
            .collect()
    }
}

// ─── Parseur principal ──────────────────────────────────────────────────────

/// Parse un `photo_mode_random_pose_config.cfg.bin.json` complet (liste `m_randomPoseList`).
///
/// Renvoie un [`PhotoModeRandomPoseConfig`] avec toutes les entrées valides
/// (`motionNameCrc` non-nul), dans l'ordre brut du fichier. Les entrées invalides
/// sont silencieusement ignorées.
///
/// Compte réel : `photo_mode_random_pose_config.cfg.bin.json` → 91 entrées.
#[must_use]
pub fn parse_photo_mode_random_pose_config(root: &Value) -> PhotoModeRandomPoseConfig {
    let poses = if let Some(values) = list_values(root, "m_randomPoseList") {
        let mut out = Vec::new();
        for v in values {
            if let Some(pose) = RandomPose::from_value(v) {
                out.push(pose);
            }
        }
        out
    } else {
        Vec::new()
    };
    PhotoModeRandomPoseConfig { poses }
}
