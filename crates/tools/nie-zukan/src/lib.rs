//! nie-zukan — ingesteur de l'encyclopédie officielle Level-5 « Inagle »
//! (zukan.inazuma.jp) pour les 3 langues : JP (racine), FR (/fr/), EN (/en/).
//!
//! # Architecture
//!
//! - [`forge`] : encodage/décodage du paramètre `?q=` (bit-invert + base64url + urlencode)
//! - [`client`] : client HTTP poli (rate-limit, retry, cache disque)
//! - [`parser`] : parsers HTML → structs typées [`ZukanChara`], [`ZukanSkill`], [`ZukanItem`]
//! - [`pull`] : orchestration du pull complet (`chara_list` → IDs → `chara_param` + skills + items)
//! - [`cross`] : croisement avec le miroir `SQLite` inagle (égalité exacte)
//! - [`appariement`] : appariement FLOU zukan ↔ inagle + audit (port d'inagle)

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
// Lints pedantic de pur bruit doc/style désactivés (cohérent avec `module_name_repetitions`) :
// `missing_errors_doc`/`missing_panics_doc` exigeraient une section par fonction faillible sur un
// crate d'I/O réseau ; `similar_names`/`too_many_lines`/`unnecessary_wraps` sont stylistiques ;
// `cast_sign_loss` sur des stats sémantiquement positives. Les lints pedantic substantiels restent.
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::cast_sign_loss
)]

pub mod appariement;
pub mod client;
pub mod cross;
pub mod forge;
pub mod models;
pub mod parser;
pub mod pull;
