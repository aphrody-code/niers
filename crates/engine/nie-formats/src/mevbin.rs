//! Parseur de fichiers `.mevbin` (MEVBIN — Motion Event Binary, Level-5 IEVR).
//!
//! Port Rust de `IECODE.Core/Formats/Level5/MevbinDocument.cs`.
//!
//! ## Format
//!
//! Un `.mevbin` est un conteneur **cfg.bin T2B** (footer `01 74 32 62`) situé
//! sous `data/common/chr/cXXXXXX/cXXXXXX_pNNN.mevbin` (~328 fichiers).
//! Les records décrivent les événements synchronisés à l'animation d'un
//! personnage.
//!
//! ## Structure des records (plate, dans l'ordre)
//!
//! ```text
//! COUNT_0   — 2 champs : nombre de motions (i32), nombre total d'événements (i32).
//! MOT_n     — 1 champ  : hash CRC-32 Level-5 de l'animation (encodé Int T2B).
//! EVENT_n   — 3 champs : time (f32), type (Int ou Float), param (Int ou Float).
//! ```
//!
//! Chaque `MOT_n` ouvre un groupe ; les `EVENT_n` suivants lui sont rattachés
//! jusqu'au prochain `MOT_n`.
//!
//! ## Interprétation des champs `type` et `param`
//!
//! Les champs `#1` et `#2` d'un `EVENT_n` peuvent être un hash CRC-32 (encodé
//! Int T2B) ou un flottant (durée, intensité). Le bit de type T2B (`Int`=1,
//! `Float`=2) est conservé dans [`MevbinEvent::type_is_float`] /
//! [`MevbinEvent::param_is_float`].
//!
//! Compatible `no_std + alloc`.

extern crate alloc;
use alloc::vec::Vec;

use crate::FormatError;
use crate::cfgbin::{Format, Value, cfgbin_parse};

// ── Types publics ─────────────────────────────────────────────────────────────

/// Événement déclenché à un instant donné de l'animation.
///
/// Provient d'un record `EVENT_n` du cfg.bin sous-jacent (3 champs).
/// Miroir exact de `MevbinEvent` C# (`IECODE.Core`).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct MevbinEvent {
    /// Instant de déclenchement en secondes (champ `#0`, toujours float T2B).
    pub time: f32,
    /// Valeur brute 32 bits du champ `#1` (type de l'événement).
    ///
    /// Si `Int` : les bits du i32 sont réinterprétés en u32.
    /// Si `Float` : les bits du f32 sont réinterprétés en u32 (port de
    /// `BitConverter.SingleToInt32Bits`).
    pub type_raw: u32,
    /// `true` si le champ `#1` est de type `Float` T2B.
    pub type_is_float: bool,
    /// Valeur flottante native du champ `#1` si `type_is_float`, sinon `0.0`.
    pub type_float: f32,
    /// Valeur brute 32 bits du champ `#2` (paramètre de l'événement).
    pub param_raw: u32,
    /// `true` si le champ `#2` est de type `Float` T2B.
    pub param_is_float: bool,
    /// Valeur flottante native du champ `#2` si `param_is_float`, sinon `0.0`.
    pub param_float: f32,
}

/// Groupe d'événements rattachés à une animation/motion donnée.
///
/// Provient d'un record `MOT_n` (un seul champ : hash CRC-32 Level-5)
/// suivi de ses `EVENT_n`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct MevbinMotion {
    /// Hash CRC-32 Level-5 de l'animation (champ unique du record `MOT_n`).
    pub hash: u32,
    /// Événements déclenchés durant cette motion, dans l'ordre du fichier.
    pub events: Vec<MevbinEvent>,
}

/// Document MEVBIN — Motion Event Binary.
///
/// Wrapper sémantique au-dessus de [`crate::cfgbin::cfgbin_parse`] :
/// un `.mevbin` est un cfg.bin T2B dont les records décrivent les événements
/// synchronisés à l'animation d'un personnage IEVR.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct MevbinDocument {
    /// Motions et leurs événements, dans l'ordre du fichier.
    pub motions: Vec<MevbinMotion>,
    /// Nombre de motions déclaré par l'en-tête `COUNT_0.#0`.
    pub header_motion_count: i32,
    /// Nombre total d'événements déclaré par l'en-tête `COUNT_0.#1`.
    pub header_event_count: i32,
}

impl MevbinDocument {
    /// Nombre de motions effectivement extraites.
    #[must_use]
    pub fn motion_count(&self) -> usize {
        self.motions.len()
    }

    /// Nombre d'événements effectivement extraits (somme sur toutes les motions).
    #[must_use]
    pub fn parsed_event_count(&self) -> usize {
        self.motions.iter().map(|m| m.events.len()).sum()
    }
}

// ── Point d'entrée ────────────────────────────────────────────────────────────

/// Décode un `.mevbin` en document sémantique.
///
/// Délègue le parsing T2B à [`crate::cfgbin::cfgbin_parse`], puis regroupe
/// les records `MOT_n` / `EVENT_n` en [`MevbinMotion`].
///
/// Port direct de `MevbinDocument.Decode` (C#) : la logique de regroupement
/// est identique, les conversions de valeurs (AsFloat / AsUInt) sont portées
/// en Rust.
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si le tampon est trop court pour le header T2B.
/// - [`FormatError::Corrupt`] si le cfg.bin est invalide ou si le format
///   retourné n'est pas T2B.
///
/// # Exemple
///
/// ```no_run
/// # fn main() -> Result<(), nie_formats::FormatError> {
/// let data = std::fs::read("data/common/chr/c001000/c001000_p000.mevbin").unwrap();
/// let doc = nie_formats::mevbin::parse(&data)?;
/// println!("{} motions, {} événements", doc.motion_count(), doc.parsed_event_count());
/// # Ok(()) }
/// ```
pub fn parse(data: &[u8]) -> Result<MevbinDocument, FormatError> {
    let file = cfgbin_parse(data)?;

    if file.format != Format::T2b {
        return Err(FormatError::Corrupt("mevbin : format attendu T2B"));
    }

    let mut header_motion_count = 0i32;
    let mut header_event_count = 0i32;
    let mut motions: Vec<MevbinMotion> = Vec::new();
    // Événements de la motion en cours d'accumulation.
    let mut current_events: Option<Vec<MevbinEvent>> = None;
    let mut current_hash = 0u32;
    let mut have_motion = false;

    // Les records MOT_n / EVENT_n ne portent pas de suffixe _BGN/_END, donc
    // cfgbin_parse les retourne tous dans file.entries à plat (aucune imbrication).
    for entry in &file.entries {
        let name = entry.name.as_str();

        if name.starts_with("COUNT") {
            // COUNT_0 : champ #0 = nb motions, champ #1 = nb événements totaux.
            if let Some(v) = entry.variables.first() {
                header_motion_count = val_as_int(v);
            }
            if entry.variables.len() >= 2 {
                header_event_count = val_as_int(&entry.variables[1]);
            }
        } else if name.starts_with("MOT") {
            // Nouvelle motion : clôt la précédente d'abord.
            if have_motion {
                motions.push(MevbinMotion {
                    hash: current_hash,
                    events: current_events.take().unwrap_or_default(),
                });
            }
            current_hash = entry.variables.first().map_or(0u32, val_as_uint);
            current_events = Some(Vec::new());
            have_motion = true;
        } else if name.starts_with("EVENT") {
            // EVENT avant tout MOT (rare) : groupe implicite de hash 0.
            if current_events.is_none() {
                current_events = Some(Vec::new());
                have_motion = true;
                current_hash = 0u32;
            }
            if let Some(ref mut evs) = current_events {
                evs.push(build_event(&entry.variables));
            }
        }
        // Les autres records (COUNT avec un nom inattendu, etc.) sont ignorés.
    }

    // Clôt la dernière motion en cours.
    if have_motion {
        motions.push(MevbinMotion {
            hash: current_hash,
            events: current_events.unwrap_or_default(),
        });
    }

    Ok(MevbinDocument {
        motions,
        header_motion_count,
        header_event_count,
    })
}

// ── Helpers internes ──────────────────────────────────────────────────────────

/// Construit un [`MevbinEvent`] depuis les variables d'un record `EVENT_n`.
///
/// Port de `MevbinDocument.BuildEvent` (C#).
fn build_event(vars: &[Value]) -> MevbinEvent {
    // Champ #0 : time (toujours float — AsFloat réinterprète les bits si Int).
    let time = vars.first().map_or(0.0f32, val_as_float);

    // Champ #1 : type (Int = hash CRC-32 ; Float = durée/intensité).
    let (type_raw, type_is_float, type_float) = match vars.get(1) {
        Some(v) => {
            let is_float = matches!(v, Value::Float(_));
            let raw = val_as_uint(v);
            let fval = if is_float {
                val_as_float_direct(v)
            } else {
                0.0
            };
            (raw, is_float, fval)
        }
        None => (0u32, false, 0.0f32),
    };

    // Champ #2 : param (Int = index/flag/hash ; Float = scalaire).
    let (param_raw, param_is_float, param_float) = match vars.get(2) {
        Some(v) => {
            let is_float = matches!(v, Value::Float(_));
            let raw = val_as_uint(v);
            let fval = if is_float {
                val_as_float_direct(v)
            } else {
                0.0
            };
            (raw, is_float, fval)
        }
        None => (0u32, false, 0.0f32),
    };

    MevbinEvent {
        time,
        type_raw,
        type_is_float,
        type_float,
        param_raw,
        param_is_float,
        param_float,
    }
}

/// Extrait une valeur entière signée depuis un `Value` T2B (port de `AsInt` C#).
fn val_as_int(v: &Value) -> i32 {
    match v {
        Value::Int(i) => *i,
        Value::Float(f) => *f as i32,
        Value::String(_) => 0,
    }
}

/// Extrait une valeur entière non signée 32 bits depuis un `Value` T2B.
///
/// Port de `AsUInt` C# : pour un `Float`, les bits du f32 sont réinterprétés
/// en u32 (identique à `BitConverter.SingleToInt32Bits`).
fn val_as_uint(v: &Value) -> u32 {
    match v {
        Value::Int(i) => *i as u32,
        Value::Float(f) => f.to_bits(),
        Value::String(_) => 0u32,
    }
}

/// Extrait une valeur flottante depuis un `Value` T2B.
///
/// Port de `AsFloat` C# : pour un `Int`, les bits sont réinterprétés en f32
/// (`BitConverter.Int32BitsToSingle`). Utilisé pour le champ `time`.
fn val_as_float(v: &Value) -> f32 {
    match v {
        Value::Float(f) => *f,
        Value::Int(i) => f32::from_bits(*i as u32),
        Value::String(_) => 0.0,
    }
}

/// Extrait la valeur flottante native d'un `Value::Float` sans réinterprétation.
///
/// Utilisé pour `type_float` / `param_float` quand le champ est bien un `Float`
/// T2B (is_float == true). Pour un `Int`, retourne `0.0` (conforme au C# :
/// `typeFloat = typeIsFloat ? AsFloat(f) : 0f`).
fn val_as_float_direct(v: &Value) -> f32 {
    match v {
        Value::Float(f) => *f,
        Value::Int(_) | Value::String(_) => 0.0,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test sur fichier réel via le VFS IEVR ────────────────────────────────

    /// Valide le parser sur un vrai `.mevbin` extrait du CPK du jeu.
    ///
    /// Skip automatiquement si le jeu est absent (CI sans Steam).
    /// Quand le jeu est présent, le test DOIT passer — toute assertion fausse
    /// indique un bug de parser à corriger.
    ///
    /// Miroir des tests C# `Decode_RealSample_FormatAndStructure` et
    /// `Decode_RealSample_HeaderCountsMatchParsed`.
    #[test]
    fn real_file_golden() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");

        let mut vfs = crate::vfs::Vfs::new();
        if vfs.init(&data_dir).is_err() {
            eprintln!("skip real_file_golden (mevbin) : init VFS échoué");
            return;
        }

        // .mevbin du VFS, choisi de façon DÉTERMINISTE (tri lexicographique du chemin) : `iter()`
        // est un HashMap → `find()` piochait un fichier au hasard, rendant ce golden FLAKY. Certains
        // `.mevbin` ont un en-tête `COUNT` qui SUR-COMPTE les motions : `c000302_p070` déclare
        // `COUNT=[178, 457]` mais ne contient que **177 records MOT** (le compte d'EVENT, 457, est lui
        // exact). Le parseur est donc CORRECT (177 motions parsées) — l'invariant fort header==parsé
        // ne tient juste pas universellement (donnée jeu, pas un bug de parse ; vérifié par dump des
        // records). Le tri ancre le golden sur le 1ᵉʳ fichier conforme (`_animal/an000100_p010`).
        let mut mevbins: alloc::vec::Vec<alloc::string::String> = vfs
            .iter()
            .map(|(k, _)| k.to_string())
            .filter(|k| k.ends_with(".mevbin"))
            .collect();
        mevbins.sort_unstable();
        let mevbin_path = mevbins.into_iter().next();

        let Some(path) = mevbin_path else {
            eprintln!("skip real_file_golden (mevbin) : aucun .mevbin dans le VFS");
            return;
        };

        let raw = vfs.read(&path).expect("lecture .mevbin via VFS");
        let doc = parse(&raw).unwrap_or_else(|e| panic!("parse({path}) : {e}"));

        // Invariant structure : au moins une motion et un événement.
        assert!(!doc.motions.is_empty(), "aucune motion dans {path}");
        assert!(doc.parsed_event_count() > 0, "aucun événement dans {path}");

        // Invariant temporel : time >= 0.0 pour tous les événements.
        for motion in &doc.motions {
            for ev in &motion.events {
                assert!(ev.time >= 0.0, "time négatif ({}) dans {path}", ev.time);
            }
        }

        // Invariant fort (validé sur le corpus C#) : les compteurs d'en-tête
        // COUNT_0 correspondent exactement aux éléments parsés.
        assert_eq!(
            doc.header_motion_count as usize,
            doc.motion_count(),
            "header_motion_count ({}) != motion_count ({}) dans {path}",
            doc.header_motion_count,
            doc.motion_count()
        );
        assert_eq!(
            doc.header_event_count as usize,
            doc.parsed_event_count(),
            "header_event_count ({}) != parsed_event_count ({}) dans {path}",
            doc.header_event_count,
            doc.parsed_event_count()
        );

        eprintln!(
            "mevbin golden : {} — {} motions, {} événements",
            path,
            doc.motion_count(),
            doc.parsed_event_count(),
        );
    }

    // ── Tests unitaires non-gated ────────────────────────────────────────────

    /// Vérifie les conversions de valeurs scalaires (AsInt / AsUInt / AsFloat).
    #[test]
    fn value_conversions() {
        // val_as_int
        assert_eq!(val_as_int(&Value::Int(42)), 42);
        assert_eq!(val_as_int(&Value::Int(-1)), -1);
        assert_eq!(val_as_int(&Value::Float(3.0)), 3);
        assert_eq!(val_as_int(&Value::String(alloc::string::String::new())), 0);

        // val_as_uint : Int → bits reinterpret comme u32
        assert_eq!(val_as_uint(&Value::Int(0)), 0u32);
        assert_eq!(val_as_uint(&Value::Int(-1)), 0xFFFF_FFFFu32);
        // val_as_uint : Float → bits du f32 réinterprétés en u32
        let f = 1.0f32;
        assert_eq!(val_as_uint(&Value::Float(f)), f.to_bits());
        assert_eq!(
            val_as_uint(&Value::String(alloc::string::String::new())),
            0u32
        );

        // val_as_float : Float → valeur directe
        assert_eq!(val_as_float(&Value::Float(2.5)), 2.5f32);
        // val_as_float : Int → bits réinterprétés (BitConverter.Int32BitsToSingle)
        let bits = 1.5f32.to_bits() as i32;
        assert_eq!(val_as_float(&Value::Int(bits)), 1.5f32);
        assert_eq!(
            val_as_float(&Value::String(alloc::string::String::new())),
            0.0f32
        );

        // val_as_float_direct : seul Float renvoie une valeur non nulle
        assert_eq!(val_as_float_direct(&Value::Float(7.0)), 7.0f32);
        assert_eq!(val_as_float_direct(&Value::Int(42)), 0.0f32);
    }

    /// Vérifie que `parse` échoue correctement sur un tampon vide.
    #[test]
    fn parse_tampon_vide_erreur() {
        let r = parse(&[]);
        assert!(r.is_err(), "tampon vide doit échouer");
    }
}
