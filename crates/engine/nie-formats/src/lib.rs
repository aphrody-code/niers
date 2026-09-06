//! Parsers de formats Level-5 / Criware portés en Rust, `no_std`-friendly (alloc) pour
//! la portabilité wasm.
//!
//! ## Modules disponibles
//!
//! - [`crilayla`] — décompresseur CRILAYLA complet (LZ bitstream inversé).
//! - [`cpk`] — lecteur de tables `@UTF` et d'archives CPK Criware (big-endian).
//! - [`cfgbin`] — lecteur de fichiers RDBN (cfg.bin Level-5) + décodage des VALEURS typées.
//! - [`g4tx`] — conteneurs de textures G4TX (header + entrées + atlas + noms).
//! - [`nxtch`] — chunks de texture Switch NXTCH + déswizzle GOB Tegra X1.
//! - [`g4md`] — métadonnées de modèle G4MD (sous-mailles + attributs vertex + materials).
//! - [`g4mg`] — géométrie G4MG (positions/normales/UV0/indices) pilotée par le `.g4md`.
//! - [`g4sk`] — squelettes G4SK (header garanti + hiérarchie d'os RÉSOLUE via la table
//!   d'offsets de l'en-tête, recoupée sur un `.g4sk` réel ; fallback heuristique).
//! - [`g4pk`] — archives Level-5 G4PK / G4RA (table offsets/tailles/hashes/noms).
//!
//! ## Compatibilité `no_std`
//!
//! Ce crate utilise `alloc` (via `extern crate alloc`). `FormatError`/`AssembleError`
//! implémentent `core::error::Error` **à la main** (plus de dépendance `thiserror`, Phase 3 dédup) →
//! types d'erreur `no_std`-ready. NB : le `#![no_std]` strict reste à faire (l'I/O des parseurs
//! utilise encore `std::io::Cursor` ; gating à venir, cf. `docs/ARCHITECTURE.md` Phase 3).
//!
//! ## Chiffrement CPK IEVR (déchiffré, vérifié au réel)
//!
//! Les CPK de `data/packs/` d'Inazuma Eleven: Victory Road sont chiffrés par l'enveloppe
//! CRI Middleware « position-based XOR », clé dérivée du **nom de fichier** (CRC32 standard).
//! [`cpk::decrypt_and_check_cpk`] déchiffre en place et vérifie que le résultat est un en-tête
//! `CPK ` + table `@UTF` (anti-hallucination). Porté de `CriwareCrypt.cs` / `NativeCrypto.cs`
//! et recoupé sur des octets réels du dump VPS. La constante d'audit `0x1717E18E`
//! ([`cpk::VIOLA_FIXED_KEY`]) est la clé Viola des `cfg.bin`, PAS celle des packs.
#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]
// `#![no_std]` strict hors feature `std` (dédup Phase 3). Cœur de parseurs = alloc-only ;
// assemble/vfs (I/O fichiers) + cri_audio (maths flottantes ADX f64::cos/sqrt) + textures/audio-decode
// = std. `default = ["std"]` → build par défaut INCHANGÉ ; `--no-default-features` = cœur no_std (nie-data).
#![cfg_attr(not(feature = "std"), no_std)]

// `FormatError::Malformed` porte un message construit : la crate a besoin d'`alloc` au niveau
// racine, pas seulement dans les modules qui le déclarent chacun de leur côté.
extern crate alloc;

/// Assemblage GLB d'un personnage (feature `std` : I/O fichiers + `HashMap`).
#[cfg(feature = "std")]
pub mod assemble;
pub mod cfgbin;
#[cfg(feature = "std")]
pub mod col;
pub mod cpk;
/// Encodeur CPK/@UTF (contrepartie écriture de [`cpk`]) — même gating (aucun, `alloc`-only).
pub mod cpk_encode;
/// Parseurs/décodeurs audio Criware (feature `std` : ADX décode via `f64::cos`/`sqrt`).
#[cfg(feature = "std")]
pub mod cri_audio;
pub mod crilayla;
/// Dispatch « octets → JSON », partagé par la FFI, la CLI et le décodage en lot.
#[cfg(all(feature = "std", feature = "serde"))]
pub mod decode;
#[cfg(feature = "std")]
pub mod dxbc;
#[cfg(feature = "std")]
pub mod font;
#[cfg(feature = "std")]
pub mod g4cm;
#[cfg(feature = "std")]
pub mod g4la;
#[cfg(feature = "std")]
pub mod g4ma;
#[cfg(feature = "std")]
pub mod g4md;
#[cfg(feature = "std")]
pub mod g4mg;
#[cfg(feature = "std")]
pub mod g4mt;
#[cfg(feature = "std")]
pub mod g4pk;
#[cfg(feature = "std")]
pub mod g4pkm;
#[cfg(feature = "std")]
pub mod g4pkm_motion;
#[cfg(feature = "std")]
pub mod g4sk;
#[cfg(feature = "std")]
pub mod g4tx;
/// Décodeur G4TX/DDS → RGBA8/PNG (source unique du workspace, feature `textures`).
#[cfg(feature = "textures")]
pub mod g4tx_decode;
/// Encodeur G4TX/DDS (RGBA8 → BGRA8 non compressé → conteneur G4TX mono-texture) — contrepartie
/// écriture de [`g4tx_decode`], même gating (ses tests round-trip en dépendent).
#[cfg(feature = "textures")]
pub mod g4tx_encode;
#[cfg(feature = "std")]
pub mod g4vs;
/// Encodage d'une image RGBA8 vers les formats d'échange (WebP sans perte, GIF, JPEG, BMP, TGA,
/// TIFF, QOI) — feature `images`. Le PNG y garde son chemin `png` historique, byte-exact.
#[cfg(feature = "images")]
pub mod image_out;
/// Comparaison d'images de rendu : identité, ΔE2000, SSIM par région, carte par bloc, masques ROI.
#[cfg(feature = "std")]
pub mod imgmetric;
#[cfg(feature = "std")]
pub mod level5;
#[cfg(feature = "std")]
pub mod lip;
#[cfg(feature = "std")]
pub mod menu;
#[cfg(feature = "std")]
pub mod mevbin;
/// Muxeur MP4/AVC pur Rust (H.264 Annex-B → `ftyp`/`moov`/`mdat`) — remplace l'appel `ffmpeg`.
pub mod mp4;
#[cfg(feature = "std")]
pub mod navm;
#[cfg(feature = "std")]
pub mod nxtch;
#[cfg(feature = "std")]
pub mod objbin;
/// Utilitaires de noms de chemins d'assets (file-stem byte-exact `FUN_140452820`), no_std.
pub mod pathname;
/// Analyse mesurée des planches de personnage (`chr/_face/20_EDIT`) : zones, rôle, convention de
/// composition. Source unique des seuils employés par [`image_out`].
pub mod planche;
/// Primitives 2D RGBA8 pures (crop/scale nearest) — source unique, no_std (le blend reste landmine #5).
pub mod raster2d;
pub mod rdbn_patch;
/// Atlas d'icônes `.g4tx` → feuille de sprites CSS / SVG / JSON, pour le web et l'explorateur.
#[cfg(feature = "std")]
pub mod sprite_sheet;
/// `strcmp` byte-exact (`FUN_14168b570`, MSVC SIMD) — comparaison C non signée, no_std.
pub mod strcmp;
/// `strlen` byte-exact (`FUN_14168b5f0`, MSVC SWAR) — longueur de chaîne C, no_std.
pub mod strlen;
/// `strncmp` byte-exact (`FUN_14168b330`, MSVC) — comparaison C bornée non signée, no_std.
pub mod strncmp;
pub mod t2b_patch;
/// Démultiplexeur USM/Sofdec2 complet : métadonnées `@UTF`, images, pistes sonores.
#[cfg(feature = "std")]
pub mod usm;
/// VFS multi-CPK (feature `std` : lit `cpk_list.cfg.bin` + CPK via `std::fs`).
#[cfg(feature = "std")]
pub mod vfs;
/// Muxeur WebM/Matroska pur Rust (VP9) — contrepartie de [`mp4`] pour les deux plus longs films.
pub mod webm;

#[derive(Debug)]
pub enum FormatError {
    /// Tampon trop court (`got` octets fournis, `need` requis).
    TooShort { got: usize, need: usize },
    /// Magic invalide pour le format attendu.
    BadMagic { format: &'static str },
    /// Données corrompues.
    Corrupt(&'static str),
    /// Structure incohérente, avec le détail chiffré (offset, compteur…) qui permet de la situer.
    ///
    /// Distinct de [`Self::Corrupt`], qui ne porte qu'un message figé : un décodeur qui suit des
    /// offsets calculés doit pouvoir dire **lesquels** ne retombent pas juste, sinon le
    /// diagnostic se refait à la main à chaque fois.
    Malformed(alloc::string::String),
}

// Display + Error MANUELS (no_std-ready via `core::error::Error`) — `thiserror` retiré (Phase 3 dédup).
impl core::fmt::Display for FormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort { got, need } => {
                write!(f, "tampon trop court : {got} octets, {need} attendus")
            }
            Self::BadMagic { format } => write!(f, "magic invalide pour {format}"),
            Self::Corrupt(s) => write!(f, "données corrompues : {s}"),
            Self::Malformed(s) => write!(f, "structure incohérente : {s}"),
        }
    }
}

impl core::error::Error for FormatError {}

/// Familles de formats reconnues dans l'écosystème IEVR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FileFormat {
    /// Archive Criware (`CPK `).
    Cpk,
    /// Table @UTF Criware (`@UTF`).
    Utf,
    /// Compression CriLayla (`CRILAYLA`).
    CriLayla,
    /// Audio HCA Criware (`HCA\0` ou chiffré).
    Hca,
    /// Banque audio ACB.
    Acb,
    /// Archive audio AWB (`AFS2`).
    Awb,
    /// Vidéo USM (`CRID`).
    Usm,
    /// Config binaire Level-5 (RDBN / cfg.bin).
    CfgBin,
    /// Mesh Level-5 (`G4MG`).
    G4mg,
    /// Métadonnées de modèle (`G4MD`).
    G4md,
    /// Texture (`G4TX`).
    G4tx,
    /// Squelette (`G4SK`).
    G4sk,
    /// Pack/anim Level-5 (`G4PK`).
    G4pk,
    /// Navmesh Level-5 (magic réel `NAVM`, extension `.g4nv`).
    G4nv,
    /// Inconnu.
    Unknown,
}

impl FileFormat {
    /// Nom court lisible.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Cpk => "CPK",
            Self::Utf => "@UTF",
            Self::CriLayla => "CRILAYLA",
            Self::Hca => "HCA",
            Self::Acb => "ACB",
            Self::Awb => "AWB",
            Self::Usm => "USM",
            Self::CfgBin => "cfg.bin",
            Self::G4mg => "G4MG",
            Self::G4md => "G4MD",
            Self::G4tx => "G4TX",
            Self::G4sk => "G4SK",
            Self::G4pk => "G4PK",
            Self::G4nv => "G4NV",
            Self::Unknown => "?",
        }
    }
}

/// Détecte le format d'un tampon par ses octets de tête.
#[must_use]
pub fn detect(bytes: &[u8]) -> FileFormat {
    const fn starts(b: &[u8], m: &[u8]) -> bool {
        if b.len() < m.len() {
            return false;
        }
        let mut i = 0;
        while i < m.len() {
            if b[i] != m[i] {
                return false;
            }
            i += 1;
        }
        true
    }
    if starts(bytes, b"CRILAYLA") {
        FileFormat::CriLayla
    } else if starts(bytes, b"CPK ") {
        FileFormat::Cpk
    } else if starts(bytes, b"@UTF") {
        FileFormat::Utf
    } else if starts(bytes, b"AFS2") {
        FileFormat::Awb
    } else if starts(bytes, b"CRID") {
        FileFormat::Usm
    } else if starts(bytes, b"@HCA") || starts(bytes, b"HCA\0") {
        FileFormat::Hca
    } else if starts(bytes, b"G4MG") {
        FileFormat::G4mg
    } else if starts(bytes, b"G4MD") {
        FileFormat::G4md
    } else if starts(bytes, b"G4TX") {
        FileFormat::G4tx
    } else if starts(bytes, b"G4SK") {
        FileFormat::G4sk
    } else if starts(bytes, b"G4PK") {
        FileFormat::G4pk
    } else if starts(bytes, b"NAVM") {
        // Le magic réel des fichiers .g4nv est `NAVM` (vérifié sur 159 vrais fichiers).
        // La variante `G4NV` n'apparaît jamais dans les assets IEVR.
        FileFormat::G4nv
    } else if starts(bytes, b"RDBN") {
        FileFormat::CfgBin
    } else {
        FileFormat::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_magics() {
        assert_eq!(detect(b"CRILAYLA\x00\x00"), FileFormat::CriLayla);
        assert_eq!(detect(b"CPK \x00\x00"), FileFormat::Cpk);
        assert_eq!(detect(b"@UTF\x00"), FileFormat::Utf);
        assert_eq!(detect(b"G4MG....."), FileFormat::G4mg);
        assert_eq!(detect(b"AFS2...."), FileFormat::Awb);
        assert_eq!(detect(b"RDBN...."), FileFormat::CfgBin);
        assert_eq!(detect(b"random"), FileFormat::Unknown);
        assert_eq!(detect(b""), FileFormat::Unknown);
    }

    /// Le magic réel des navmesh Level-5 (.g4nv) est `NAVM`, pas `G4NV`.
    /// Vérifié par dump hexadécimal de 159 vrais fichiers IEVR.
    #[test]
    fn g4nv_magic_is_navm_not_g4nv() {
        // Magic réel observé sur fichiers .g4nv du jeu (ex: k03.g4nv: 4E 41 56 4D ...)
        let navm_header = b"NAVM\x60\x00\x66\x00\x00\x00\x18\x00";
        assert_eq!(
            detect(navm_header),
            FileFormat::G4nv,
            "detect() doit retourner G4nv pour un magic NAVM"
        );
        // L'ancien magic erroné G4NV ne doit PAS être reconnu comme G4nv
        let wrong_magic = b"G4NV....";
        assert_eq!(
            detect(wrong_magic),
            FileFormat::Unknown,
            "detect() ne doit PAS reconnaître G4NV (absent des assets IEVR)"
        );
    }

    /// Test golden gated sur NIE_GAME_DIR : lit le plus petit .g4nv via VFS
    /// et vérifie que detect() retourne FileFormat::G4nv (magic NAVM).
    #[test]
    fn g4nv_detect_golden_via_vfs() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data = std::path::Path::new(&dir).join("data");
        if !crate::vfs::donnees_disponibles(&data) {
            eprintln!("skip g4nv_detect_golden_via_vfs : NIE_GAME_DIR absent");
            return;
        }

        let mut vfs = crate::vfs::Vfs::new();
        vfs.init(&data).expect("Vfs::init");

        // Sélectionne le plus petit .g4nv pour minimiser l'I/O
        let smallest = vfs
            .iter()
            .filter(|(p, e)| p.ends_with(".g4nv") && e.file_size > 0)
            .min_by_key(|(_, e)| e.file_size)
            .map(|(p, _)| p.to_string());

        let path = match smallest {
            Some(p) => p,
            None => {
                eprintln!("skip g4nv_detect_golden_via_vfs : aucun .g4nv dans l'index VFS");
                return;
            }
        };

        let bytes = vfs
            .read(&path)
            .unwrap_or_else(|e| panic!("vfs.read({path}): {e}"));
        assert!(
            bytes.len() >= 4,
            "fichier .g4nv trop court ({} octets) : {path}",
            bytes.len()
        );

        let fmt = detect(&bytes);
        assert_eq!(
            fmt,
            FileFormat::G4nv,
            "detect() a retourné {:?} au lieu de G4nv pour {path} (magic réel: {:?})",
            fmt,
            &bytes[..bytes.len().min(4)]
        );

        // Vérifie que les 4 premiers octets sont bien NAVM
        assert_eq!(
            &bytes[..4],
            b"NAVM",
            "magic inattendu dans {path}: {:?}",
            &bytes[..4]
        );

        eprintln!(
            "OK: detect({path}) == G4nv (magic NAVM, {} octets)",
            bytes.len()
        );
    }
}
