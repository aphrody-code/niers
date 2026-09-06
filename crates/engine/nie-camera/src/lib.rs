//! `nie-camera` — **la caméra d'Inazuma Eleven: Victory Road**, de bout en bout.
//!
//! Une seule crate pour tout ce qui touche à la caméra : le modèle d'état et les contrôleurs
//! portés depuis `nie.exe`, le codec des animations de cutscene `.g4cm`, les configurations
//! `cfg.bin`, la carte du reverse-engineering, et le pilotage de la caméra du **jeu en cours
//! d'exécution**.
//!
//! ## Modules
//!
//! | Module | Rôle |
//! |---|---|
//! | [`model`] | état de caméra ([`model::CameraState`]), matrices vue/projection, les 19 contrôleurs natifs ([`model::CtrlKind`]) |
//! | [`g4cm`] | **codec** des animations caméra de cutscene : décodage complet + ré-encodage **byte-exact** |
//! | [`config`] | `soccer_camera_config` (13 listes) et les autres `cfg.bin` caméra, typés |
//! | [`property`] | `camera_ctrl_property_info*` — paramètres du contrôleur par contexte |
//! | [`ctrl`] | contrôleurs portés : poursuite, shake de tir, interpolation, offset, blend |
//! | [`map`] | carte RE statique : VA, tables de dispatch, hiérarchie RTTI, chemins d'assets |
//! | [`live`] | lecture/écriture de la caméra dans le process `nie.exe` vivant |
//! | [`db`] | indexation de tout ce savoir dans `var/niers.sqlite` (tables `cam_*`) |
//!
//! ## Provenance des faits
//!
//! Tout ce qui est affirmé ici est **vérifié sur le binaire ou sur les données** (cf.
//! `docs/game-data/camera.md`). Ce qui n'a pas pu être confirmé est signalé comme tel dans la
//! doc de l'item concerné plutôt que deviné — en particulier l'encodage des flux de keyframes
//! sur 2 octets ([`g4cm::Track::Raw16`]).

#![warn(missing_docs)]

pub mod config;
pub mod ctrl;
pub mod db;
pub mod g4cm;
pub mod live;
pub mod map;
pub mod model;
pub mod property;

pub use model::{CameraState, CtrlKind};

/// Erreurs de la crate.
#[derive(Debug, thiserror::Error)]
pub enum CameraError {
    /// Le tampon est trop court pour la structure attendue.
    #[error("données trop courtes : {got} octets, il en faut {need} (à l'offset 0x{at:X})")]
    TooShort {
        /// Taille disponible.
        got: usize,
        /// Taille requise.
        need: usize,
        /// Offset où la lecture a échoué.
        at: usize,
    },
    /// Le magic n'est pas celui attendu.
    #[error("magic invalide : ce n'est pas un fichier {format}")]
    BadMagic {
        /// Format attendu.
        format: &'static str,
    },
    /// Champ structurellement incohérent (offset hors bornes, compteur absurde…).
    #[error("structure incohérente : {0}")]
    Malformed(String),
    /// Version de conteneur non gérée.
    #[error("version de conteneur {got:#06X} non gérée (attendu {expected:#06X})")]
    UnsupportedVersion {
        /// Version lue.
        got: u16,
        /// Version supportée.
        expected: u16,
    },
    /// Erreur de format déléguée à `nie-formats`.
    #[error(transparent)]
    Format(#[from] nie_formats::FormatError),
}

/// Résultat de la crate.
pub type Result<T> = core::result::Result<T, CameraError>;
