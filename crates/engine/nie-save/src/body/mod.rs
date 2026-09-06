//! Parseurs des corps de blobs IEVR.
//!
//! Chaque sous-module parse un type de blob spécifique. Seuls les champs
//! **validés sur octets réels** sont représentés — tout champ non confirmé
//! est explicitement absent ou marqué OPAQUE dans la documentation.

pub mod autosave;
pub mod autosave_roster;
pub mod headersave;

pub use autosave::{AutoSaveLayout, CharaParamSlot, parse_autosave_layout};
pub use autosave_roster::{AutosaveRoster, AutosaveScalars, CharaId, parse_autosave_roster};
pub use headersave::{HEADERSAVE_MIN_LEN, HeaderSave, NativeDateTime, SlotMeta, parse_headersave};
