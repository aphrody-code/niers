//! **nie-app** — le CŒUR du jeu *Inazuma Eleven: Victory Road* réimplémenté (niers).
//!
//! Possède le **DTO de rendu** ([`GameState`] — « quoi dessiner ») + le rendu abstrait (trait
//! [`Renderer`]) ET la **FSM interactive** ([`flow::Screen`] — la navigation : input/update/score).
//! Les deux sont **complémentaires** (DTO de rendu vs machine à états), pas dupliqués : `Screen`
//! délègue son rendu à `GameState` via `render_state`. Consommateurs réels : `nie-play`
//! (headless/golden, Renderer CPU → PNG/MP4) et `nie-wasm` (web interactif, délègue à `flow::Screen`).
//! `nie-game` (wgpu natif 60 fps) suivra. Fondation de l'unification (cf. `docs/PLAN.md`, Phase 0).
//!
//! La logique (match `nie-core`), les données (`nie-data`), les menus (`nie-lua`) se branchent ici
//! au fil des phases ; pour l'instant le cœur tient la FSM + l'orchestration du rendu.

pub mod character;
/// Effectif réel chargé depuis le VFS — natif seulement : le VFS lit des fichiers, ce que le web
/// ne fait pas (il reçoit ses octets par `fetch`).
#[cfg(not(target_arch = "wasm32"))]
pub mod effectif;
pub mod flow;
/// Rendu 3D d'un match (vrais modèles du VFS) — natif seulement, comme `effectif`.
#[cfg(not(target_arch = "wasm32"))]
pub mod match3d;
pub mod render;
#[cfg(not(target_arch = "wasm32"))]
pub mod roster;
pub mod story;

/// Renderer CPU natif (charge les assets disque) — absent en wasm (le web utilise [`render::render_state`]).
#[cfg(not(target_arch = "wasm32"))]
pub use render::CpuRenderer;
pub use render::{Font, Frame, H, W};

/// État du jeu — la machine à états centrale, partagée par tous les front-ends.
///
/// S'étendra (TeamSelect, KizunaTown, AvatarEdit, …) au fil de l'unification.
#[derive(Debug, Clone)]
pub enum GameState {
    /// Écran-titre (logo + PRESS START).
    Title,
    /// Menu principal, `sel` = option surlignée.
    MainMenu { sel: usize },
    /// Résultat / déroulé de match (score domicile-extérieur).
    Match { home: u8, away: u8 },
    /// Scène de dialogue (mode histoire).
    Story { speaker: String, line: String },
}

/// Les 9 onglets RÉELS du menu principal d'IEVR, prouvés byte-exact dans `main_menu_1.02.92.00.lua.bin`
/// (types {10,20,30,70,40,80,60,50,90} → obj_hash CRC32 + text_id). Libellés = vrais textes FR
/// (`data/common/text/fr/menu_text.cfg.bin.json`, résolus par hash). Source vérifiée, pas inventé.
pub const MENU: [&str; 9] = [
    "Composition d'équipe",        // team_dock_menu
    "Objets",                      // item_menu
    "Marque-pages d'informations", // info_bookmark_menu
    "Inacord",                     // inacode_menu
    "Fichier de données",          // datafile_menu
    "Adversaires",                 // opponent_team_menu (→ sélection de mode/match)
    "Aide",                        // help_menu
    "Options",                     // setting_menu
    "Sauvegarder",                 // rpg_save_menu
];

/// Les 5 MODES DE JEU racine d'IEVR (libellés FR réels, hash menu_text). Atteints via « Adversaires ».
/// Source : menu_text + cfg `*_mode_top_menu_setting.cfg.bin` / `bb_stadium_top_menu_setting.cfg.bin`.
pub const MODES: [&str; 5] = [
    "Mode Histoire",
    "Mode Chronique",
    "Mode Compétition",
    "Victory Road",
    "Stade BB",
];

/// Abstraction de rendu : un front-end fournit un `Renderer` qui transforme un état en frame RGBA
/// (1280×720, 4 octets/px). CPU (`CpuRenderer`) aujourd'hui, wgpu demain (nie-game, Phase 4).
pub trait Renderer {
    /// Rend l'état en une frame RGBA8 `W*H*4`.
    fn render(&self, state: &GameState) -> Vec<u8>;
}

/// Le playthrough scripté (mode headless/golden) : suite de `(état, nombre de frames)`.
///
/// Le score vient d'un match réel (`nie_core::simulate_match`) côté front-end ; le flux reste ici
/// pour qu'il soit partagé et déterministe (gate de non-régression PNG/MP4).
#[must_use]
pub fn demo_flow(home: u8, away: u8) -> Vec<(GameState, u32)> {
    vec![
        (GameState::Title, 30),
        (GameState::MainMenu { sel: 0 }, 20),
        (GameState::Match { home, away }, 40),
        (GameState::MainMenu { sel: 1 }, 20),
        (
            GameState::Story {
                speaker: String::from("Endou Mamoru"),
                line: String::from("Can anyone bring down Raimon's unshakable fortress?!"),
            },
            40,
        ),
        (GameState::MainMenu { sel: 3 }, 20),
    ]
}
