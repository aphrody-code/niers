//! `nie-core` — Logique de jeu reversée en Rust pur
//!
//! Ce crate porte en Rust idiomatique les structures et algorithmes de
//! gameplay extraits du pseudo-C Ghidra de `nie.exe` (Inazuma Eleven:
//! Victory Road). Chaque élément porté cite sa source exacte et documente
//! ce qui est fidèle versus incertain.
//!
//! # Modules
//!
//! - [`ball`] — Composant ballon (`game::BallComponent`, contrôleurs de mouvement)
//! - [`match_state`] — Machine à états du match d'entraînement (`game::CSceneSoccer`)
//! - [`match_fsm`] — FSM de match 11 états + score final (`tick`/`final_score`)
//! - [`soccer_ctrl`] — Contrôleur de match et transitions de phase
//! - [`action`] — Contrôleur d'actions joueur (`game::SoccerActionCtrl`)
//! - [`command_effect`] — Tables de slots d'effets de commande de match
//! - [`keeper`] — Calcul d'arrêt du gardien (`game::SoccerCalcKeeperSaveComponent`)
//! - [`tactics`] — IA tactique par joueur (`game::SoccerCharaTacticsAI`)
//! - [`stats`] — Courbe d'interpolation 3-segments des statistiques
//! - [`growth`] — Tables de croissance + lookup à fallback + `calculate_stats`
//! - [`exp`] — Table d'XP par niveau + multiplicateur de rareté
//! - [`skill`] — Modèle de technique (hissatsu) + maps élément/catégorie
//! - [`aura`] — Modèle d'aura (Keshin/Soul/…) + résolution du hissatsu lié
//!
//! # Conventions de portage
//!
//! - Chaque `struct` Rust correspond à une classe C++ identifiée par RTTI/vftable
//! - Les champs `undefined8`/`param_N` Ghidra sont reconstruits sémantiquement
//! - Les constantes flottantes IEEE 754 (`0x3F800000` = 1.0f, etc.) sont nommées
//! - Les incertitudes RE sont documentées `// RE incertain: …`
//! - `#![forbid(unsafe_code)]` — aucun `unsafe` dans ce crate

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::pedantic)]
#![allow(clippy::float_cmp)]
// std est requis pour f32::sqrt, f32::floor sur certaines cibles no_std.
// Pour l'instant on reste std pour simplicité; une feature no_std peut être
// ajoutée plus tard avec libm si nécessaire pour wasm bare-metal.

/// Helpers serde pour les grands tableaux d'octets (serde n'impl. pas les
/// arrays > 32 nativement). (Dé)sérialise un `[u8; N]` comme une séquence.
///
/// N'est compilé que sous la feature `serde`.
#[cfg(feature = "serde")]
pub(crate) mod serde_byte_array {
    use core::convert::TryInto;
    use serde::de::{Deserialize, Deserializer, Error};
    use serde::ser::Serializer;

    /// Sérialise `[u8; N]` comme un slice d'octets.
    pub fn serialize<S, const N: usize>(arr: &[u8; N], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_bytes(arr)
    }

    /// Désérialise un `[u8; N]` depuis un `Vec<u8>` de longueur exacte.
    pub fn deserialize<'de, D, const N: usize>(d: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = <Vec<u8>>::deserialize(d)?;
        v.as_slice()
            .try_into()
            .map_err(|_| D::Error::custom("longueur de tableau d'octets invalide"))
    }
}

pub mod action;
pub mod affine;
pub mod aspect_viewport;
pub mod aura;
pub mod ball;
pub mod byte_keyed_table;
pub mod category_lookup;
pub mod command_effect;
pub mod crand;
pub mod ecs;
pub mod effect_obj_ctor;
pub mod event_check;
pub mod exp;
pub mod fixed_slot;
pub mod growth;
pub mod handle_table;
pub mod imm_batcher;
pub mod intrusive_map;
pub mod keeper;
pub mod keyed_record_table;
pub mod mat4;
pub mod match_fsm;
pub mod match_sim;
pub mod match_state;
pub mod menu_setting;
pub mod play_cmd_manager;
pub mod quat;
pub mod scene;
pub mod skill;
pub mod soccer_ctrl;
pub mod stats;
pub mod tactics;
pub mod typed_list_iter;
pub mod typed_value_reader;

/// Identifiant invalide pour joueur/cible (0xFFFF0000 en binaire IEVR).
///
/// Source: `ball_component.c` offsets 0x14D0, 0x1500 — initialisation des
/// champs `target_id` et `intercept_target_id` à `0xFFFF0000`.
pub const INVALID_TARGET_ID: u32 = 0xFFFF_0000;

/// Indice de joueur invalide (0xFF = "aucun joueur").
///
/// Source: `ball_component.c` offsets 0x1490-0x14A0 — 3 octets initialisés
/// à `0xFF` pour les IDs de possession.
pub const INVALID_PLAYER_IDX: u8 = 0xFF;

/// Gravité du ballon en unités/frame² (valeur lue à offset 0x1474).
///
/// Source: `ball_component.c` — `*(undefined4 *)(param_1 + 0x1474) = 0x40000000`
/// soit `2.0f` en IEEE 754.
pub const BALL_GRAVITY: f32 = 2.0;

/// Scale du ballon par défaut (1.0f).
///
/// Source: `ball_component.c` — `*(undefined4 *)(param_1 + 0x2ea) = 0x3f800000`.
pub const BALL_SCALE_DEFAULT: f32 = 1.0;

/// Valeur sentinelle pour une distance non encore calculée (-1.0f).
///
/// Source: `ball_component.c` — offsets 0x1764, 0x176c initialisés à
/// `0xBF800000` (-1.0f en IEEE 754).
pub const DISTANCE_UNINIT: f32 = -1.0;

/// Rayon d'arrêt par défaut du gardien (1.0f).
///
/// Source: `soccer_keeper_save.c` — `*(undefined4 *)(param_1 + 0x2e) = 0x3f800000`.
pub const KEEPER_SAVE_RADIUS_DEFAULT: f32 = 1.0;

/// Distance maximale de plongeon du gardien (5.0f).
///
/// Source: `soccer_keeper_save.c` — `*(undefined4 *)(param_1 + 0x174) = 0x40a00000`.
pub const KEEPER_DIVE_MAX_DIST: f32 = 5.0;

/// Probabilité de base d'arrêt du gardien (0.8 = 80 %).
///
/// Source: `soccer_keeper_save.c` — `0x3F4CCCCD` ≈ `0.8f` IEEE 754.
pub const KEEPER_SAVE_PROBABILITY_BASE: f32 = 0.8;

/// Temps de réaction du gardien en frames (valeur réelle 4.73f).
///
/// Source: `soccer_keeper_save.c` — `0x40975C29` ≈ `4.73f` IEEE 754.
/// RE incertain: unité exacte (frames 60Hz? millisecondes?)
pub const KEEPER_REACTION_TIME_FRAMES: f32 = 4.73;

/// Vitesse de plongeon du gardien (2.67f unités/frame).
///
/// Source: `soccer_keeper_save.c` — `0x402AE148` ≈ `2.67f` IEEE 754.
/// RE incertain: unité (unités monde/frame ou m/s?)
pub const KEEPER_DIVE_SPEED: f32 = 2.67;

/// Masque de flags tactiques (0x1000000).
///
/// Source: `soccer_tactics_ai.c` — `*(undefined4 *)(param_1 + 0x154) = 0x1000000`.
/// RE incertain: signification bit-par-bit inconnue.
pub const TACTICS_FLAGS_MASK: u32 = 0x0100_0000;

/// Priorité maximale d'une option tactique (7).
///
/// Source: `soccer_tactics_ai.c` — `*(undefined2 *)(...) = 7` pour chaque
/// niveau de priorité dans les contextes tactiques.
pub const TACTICS_MAX_PRIORITY: u16 = 7;

/// Mode tactique par défaut (2).
///
/// Source: `soccer_tactics_ai.c` — `*(undefined8 *)(param_1 + 0xac) = 2`.
/// RE incertain: signification enum (0=off, 1=défensif, 2=normal, 3=offensif?).
pub const TACTICS_DEFAULT_MODE: u64 = 2;

/// `Vec3` flottant (x, y, z) — **source unique** `nie_geom::Vec3` (dédup Phase 2).
/// Convention IEVR de nie-core : `y` = hauteur (portée dans le CODE ; le type est axis-agnostique).
/// ⚠ Ne pas convertir vers/depuis `nie_runtime::V3` (z=hauteur) — cf. `docs/ARCHITECTURE.md` landmine #4.
pub use nie_geom::Vec3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_coherents() {
        // Vérifie que les constantes matchent les bits IEEE 754 du binaire
        assert_eq!(BALL_GRAVITY.to_bits(), 0x4000_0000);
        assert_eq!(BALL_SCALE_DEFAULT.to_bits(), 0x3F80_0000);
        assert_eq!(DISTANCE_UNINIT.to_bits(), 0xBF80_0000);
        assert_eq!(KEEPER_SAVE_RADIUS_DEFAULT.to_bits(), 0x3F80_0000);
    }
}
