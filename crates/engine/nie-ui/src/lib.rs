//! `nie-ui` — la source unique, typée, des jetons de design du jeu.
//!
//! ## Le problème que cette crate règle
//!
//! Les jetons de la direction artistique du jeu (couleurs, géométrie, mouvement) vivaient dans
//! **un fichier CSS** — `packages/inacord-ui/src/shell/game-tokens.css` — lu par deux hôtes
//! TypeScript et par `crates/tools/nie-site`. Le Rust qui compose des images (`nie-game`,
//! `nie-render3d`) ne pouvait pas les lire : aucune source ne les exposait comme des valeurs
//! Rust typées. `nie-ui` transpose ces jetons en constantes documentées et fournit le
//! générateur qui prouve, par un test qui peut ROUGIR ([`css::tests`]), que cette transposition
//! reproduit le bloc `:root { … }` du CSS actuel **à l'octet près**.
//!
//! ## Ce que cette crate NE fait PAS
//!
//! Elle ne mesure aucune couleur et ne dérive aucune palette. Cette mesure existe déjà :
//! [`nie_aphrody::design`] (k-means Oklab sur l'atlas du personnage, `pixel mesurer … --k 10`)
//! est la source dont *dérivent* les jetons `--jeu-*`/`--inacord-*` depuis le 2026-09-06
//! (commit `0374333`, `cargo run -p nie-aphrody --bin design`). `nie-ui` transpose le RENDU de
//! cette dérivation (les valeurs écrites dans le CSS aujourd'hui) et vérifie, en test, que cette
//! transposition reste synchronisée avec le calcul réel — voir [`color`] et son module de test.
//! Elle ne redécoupe pas non plus un atlas d'icônes : [`icons`] appelle
//! `nie_formats::sprite_sheet`, qui le fait déjà.
//!
//! ## Organisation
//!
//! - [`color`] : les 29 couleurs `--jeu-*`/`--inacord-*`, en OKLCH, chacune documentée avec son
//!   hexadécimal, sa teinte source et son rôle — copiés du commentaire CSS.
//! - [`tokens`] : les 17 jetons non colorés (géométrie, rythme, mouvement, typographie) et les
//!   trois valeurs d'élévation (composites, dérivées de deux jetons de couleur).
//! - [`css`] : le générateur du bloc `:root { … }`, et son test de non-régression falsifiable.
//! - [`roles`] : les rôles sémantiques (shadcn + Material 3) mappés sur les jetons, tels
//!   qu'`apps/nie-web/src/base.css` les emploie déjà.
//! - [`compose`] : une API minimale pour décrire une tuile ou un panneau du menu à partir des
//!   jetons qui portent déjà ce rôle — rien n'y est inventé.
//! - [`icons`] : le pont entre un atlas `.g4tx` d'interface (`nie_formats::sprite_sheet`) et les
//!   jetons de cette crate.
//! - [`screens`] : l'inventaire typé des 33 captures de référence de `data/menu/` (transposition
//!   de `manifest.json`, prouvée entrée par entrée quand le dossier est là).
//! - [`surfaces`] : les 45 couleurs `--screen-*` MESURÉES sur ces captures (`pixel capture`),
//!   l'angle du parallélogramme, et les règles de `game-screens.css` — engendré par
//!   [`css::screens_block`] et `cargo run -p nie-ui --bin game_screens_css -- --write`.
//!
//! Voir `docs/DESIGN-UI.md` pour l'état, la commande de preuve, et ce qui reste à faire —
//! notamment le texte et les polices du jeu, hors périmètre ici (cf. `docs/DESIGN.md`).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod color;
pub mod compose;
pub mod css;
pub mod icons;
pub mod roles;
pub mod screens;
pub mod surfaces;
pub mod tokens;

pub use color::{ColorToken, Oklch};
pub use tokens::RawToken;
