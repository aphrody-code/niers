//! `nie-site` — le serveur d'**Aphrody** (`aphrody.com`), 100 % Rust.
//!
//! Un seul processus, écoutant sur `127.0.0.1:8085` derrière nginx, qui :
//!
//! - sert le bundle d'`apps/nie-web` (pré-compressé `br`/`zstd`, immuable par empreinte) ;
//! - adresse les fichiers du jeu **par leur chemin VFS verbatim** — `/f/<chemin>` pour une
//!   ressource, `/b/<préfixe>` pour parcourir un dossier (amendement A3 : le chemin est un
//!   **segment**, jamais une query, et l'extension du jeu est conservée) ;
//! - répond `/api/v1/*` en lecture seule : DTO `serde`, pagination bornée, `rusqlite` en
//!   `SQLITE_OPEN_READ_ONLY` sur le miroir ;
//! - proxifie `nie-model-serve` sous `/assets/*` (concurrence bornée, délai maximal, taille
//!   de réponse bornée, cache `moka`, ETag `blake3`) ;
//! - rend `/healthz`, `/robots.txt`, `/.well-known/security.txt`, `/sitemap.xml` et les pages
//!   d'erreur, et pose **lui-même** sa `Content-Security-Policy` (nginx n'en pose aucune :
//!   deux CSP s'additionnent et la plus stricte gagne).
//!
//! Le service **démarre toujours**, même sans VFS, sans miroir et sans amont : chaque capacité
//! absente se signale en `503` avec un message explicite (cf. [`state::Capacites`]). Un serveur
//! qui refuse de démarrer parce qu'un gisement manque est un serveur qu'on ne peut pas
//! diagnostiquer.

#![warn(missing_docs)]

pub mod app;
pub mod config;
pub mod dataset;
pub mod error;
pub mod routes;
pub mod state;
pub mod vfs_index;

pub use app::routeur;
pub use config::Config;
pub use error::ErreurSite;
pub use state::EtatSite;

/// Version de la crate, telle qu'annoncée par `/healthz` et `/api/v1/health`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Nom du service, tel qu'annoncé par `/healthz` (c'est la chaîne que la gate de J5 cherche).
pub const SERVICE: &str = "nie-site";
