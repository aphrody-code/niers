//! Lecteur de tables `@UTF` (Universal Table Format) et d'archives CPK Criware.
//!
//! Port Rust de :
//! - `aphrody/crates/ievr-tools/src/cri.rs` (référence primaire, MIT)
//! - `refs/iecode-re/cli/ffi/rust/cpk-reader/src/utf.rs` (référence secondaire)
//! - `IECODE.Core/Formats/Criware/UtfTable.cs` (référence C# — IECODE)
//!
//! ## Format `@UTF` (wire layout, tout big-endian)
//!
//! ```text
//! Offset  Taille  Champ
//!  0x00     4     Magic "@UTF" (0x40 0x55 0x54 0x46)
//!  0x04     4     table_size  (octets après ces 8 premiers ; « body »)
//!  ─── base relative : byte 8 (= UTF_BASE) ───────────────────────────────────
//!  +0x00    4     rows_offset     (relatif à UTF_BASE)
//!  +0x04    4     string_offset   (relatif à UTF_BASE)
//!  +0x08    4     data_offset     (relatif à UTF_BASE)
//!  +0x0C    4     table_name_off  (offset dans le string pool)
//!  +0x10    2     column_count
//!  +0x12    2     row_stride      (octets par ligne)
//!  +0x14    4     row_count
//!  +0x18    …     descripteurs de colonnes (variable)
//!  …        …     données par ligne
//!  …        …     string pool (chaînes null-terminées UTF-8)
//!  …        …     data pool   (blobs binaires)
//! ```
//!
//! Chaque descripteur de colonne commence par un octet de flags (nibble bas = type,
//! nibble haut = bits indépendants HAS_NAME/HAS_DEFAULT/ROW_STORAGE). Si HAS_NAME est
//! positionné, 4 octets d'offset de nom suivent ; si HAS_DEFAULT est positionné, la valeur
//! par défaut (taille du type) suit ensuite. Voir [`FLAG_HAS_NAME`], [`FLAG_HAS_DEFAULT`],
//! [`FLAG_ROW_STORAGE`].
//!
//! ## Format CPK
//!
//! Un CPK valide (non chiffré) commence par `CPK ` (0x43 0x50 0x4B 0x20)
//! suivi de 12 octets de champs internes, puis d'une table `@UTF` à l'offset 0x10.
//!
//! ## Chiffrement CPK IEVR (déchiffrement porté et VÉRIFIÉ au réel)
//!
//! Les `.cpk` de `data/packs/` d'Inazuma Eleven: Victory Road sont chiffrés par
//! l'enveloppe CRI Middleware « position-based XOR ». La clé n'est **pas** une
//! constante : elle est dérivée du **nom de fichier** par un CRC32 standard
//! (poly `0xEDB88320`, init/xorout `0xFFFFFFFF`). Le flux est ensuite XORé par mots
//! de 4 octets, chaque mot dérivé d'un état CRC réinitialisé sur l'offset du mot.
//!
//! Port fidèle de :
//! - `IECODE.Core/Formats/Criware/CriFs/Encryption/CriwareCrypt.cs` (`CalculateKeyFromFilename`,
//!   `DecryptBlock`, `UpdateCrcStateFast`, `ComputeXorByte`) ;
//! - `IECODE.Core/Native/NativeCrypto.cs` (même algorithme, variante SIMD côté C#) ;
//! - sélection de clé : `CpkDecryptionStream.FromFile` → `CalculateKeyFromFilename(Path.GetFileName(cpkPath))`.
//!
//! **Vérification terrain (octets réels du dump VPS, non fabriqués)** : la clé dérivée
//! du nom déchiffre les CPK réels en un en-tête `CPK ` valide suivi de `@UTF` à 0x10.
//! Exemple recoupé : `1d08dda99e8549431c6c7b459ed8deab.cpk` → clé `0xBD281847` →
//! magic `43 50 4B 20` ("CPK ") + `@UTF` @0x10. Idem pour 3 autres CPK testés.
//!
//! ⚠️ **Distinction critique** : la constante `0x1717E18E` relevée par l'audit iecode
//! (`IEVRGame.CriEncryptionKey`, `NativeCrypto.IEVRCriKey`) est la clé **fixe Viola**
//! utilisée pour les `cfg.bin` empaquetés (`Viola/Pack/Logic/CPack.cs`), **PAS** pour
//! les `.cpk` de `data/packs/`. Appliquée à un CPK réel elle produit du bruit
//! (`14 ff 0a 0b…`, pas `CPK `). Les deux clés partagent le même `DecryptBlock`.
//! [`decrypt_cpk`] dérive donc la clé du nom de fichier (comportement vérifié) ;
//! [`decrypt_block`] reste exposé pour fournir une clé arbitraire (dont la fixe Viola).
//!
//! ## `std`
//!
//! Ce module utilise `alloc` (via `Vec`, `String`) ; compatible no_std+alloc.
//! Pas de `std::io`, pas de `byteorder`.

extern crate alloc;
use alloc::{string::String, vec::Vec};
// ToOwned du prélude std absent en no_std (dédup Phase 3 ; no-op en std via cfg).
#[cfg(not(feature = "std"))]
use alloc::borrow::ToOwned;

use crate::FormatError;

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

/// Magic `@UTF` en big-endian (octets exacts dans le fichier).
const UTF_MAGIC: [u8; 4] = [0x40, 0x55, 0x54, 0x46];

/// Magic `CPK ` en octets.
const CPK_MAGIC: [u8; 4] = *b"CPK ";

/// Offset absolu dans le flux où commence la « base » relative des offsets UTF.
const UTF_BASE: usize = 0x08;

/// Taille minimale d'une table @UTF (header 0x18 + 8 octets outer).
const UTF_MIN_TOTAL: usize = 0x20;

// Bits indépendants du nibble haut (bits 4-7) du byte de flags d'une colonne @UTF.
// Ces bits sont cumulables : une colonne peut avoir FLAG_HAS_NAME|FLAG_HAS_DEFAULT,
// ou FLAG_HAS_NAME|FLAG_ROW_STORAGE, etc.
const FLAG_HAS_NAME: u8 = 0x10; // un offset de nom (4 octets BE) suit dans le schéma
const FLAG_HAS_DEFAULT: u8 = 0x20; // une valeur constante suit dans le schéma ; n'avance PAS row_cursor
const FLAG_ROW_STORAGE: u8 = 0x40; // la valeur est stockée par ligne dans la section rows

/// Polynôme CRC32 réfléchi standard (`CriwareCrypt.cs` L20, `NativeCrypto.cs` L35).
const CRC32_POLY: u32 = 0xEDB8_8320;

/// Clé fixe « Viola » relevée par l'audit iecode (`IEVRGame.CriEncryptionKey`).
///
/// **Ne s'applique PAS** aux `.cpk` de `data/packs/` (qui utilisent une clé dérivée du
/// nom de fichier). Réservée aux `cfg.bin` empaquetés Viola (`CPack.cs`). Exposée pour
/// pouvoir la passer explicitement à [`decrypt_block`] si besoin ; jamais utilisée par
/// [`decrypt_cpk`]. Voir le module-doc.
pub const VIOLA_FIXED_KEY: u32 = 0x1717_E18E;

// ---------------------------------------------------------------------------
// Déchiffrement CRI (port fidèle de CriwareCrypt.cs / NativeCrypto.cs)
// ---------------------------------------------------------------------------

/// Construit la table CRC32 (256 entrées) — port de `InitializeTable` (`CriwareCrypt.cs` L16).
///
/// `const fn` pour être évaluée au build (pas d'état mutable global, compatible wasm + no_std).
const fn crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ CRC32_POLY
            } else {
                crc >> 1
            };
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
}

/// Table CRC32 figée au build.
const CRC32_TABLE: [u32; 256] = crc32_table();

/// Calcule le CRC32 du jeu IEVR pour identifier un asset (ModelIdCrc, etc.).
///
/// Algorithme identique à `nie-engine::g4::crc32_of` : CRC32 standard (init `0xFFFFFFFF`,
/// polynôme `0xEDB88320`) **sans** la finalisation `~crc`. C'est l'accumulateur brut.
///
/// Utilisé pour les champs `uniformKeeperModelIdCrc`, `uniformFielderModelIdCrc`, etc.
/// dans les données d'uniformes (inagle_uniforms.models).
#[must_use]
pub fn crc32_nie(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        let idx = ((crc & 0xFF) ^ u32::from(b)) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
    }
    crc // PAS de ~crc
}

/// Dérive la clé de déchiffrement depuis le **nom de fichier** d'un CPK.
///
/// Port exact de `CriwareCrypt.CalculateKeyFromFilename` / `NativeCrypto.CalculateKeyFromFilename` :
/// CRC32 standard (init `0xFFFFFFFF`, xorout `0xFFFFFFFF`) sur les octets UTF-8 du nom.
///
/// `name` doit être le **nom de base** (ex. `1d08dda99e8549431c6c7b459ed8deab.cpk`), pas un
/// chemin complet — le C# applique `Path.GetFileName` en amont. La casse n'est pas modifiée ici
/// (les noms de packs IEVR sont déjà en hexadécimal minuscule).
#[must_use]
pub fn key_from_filename(name: &str) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for b in name.bytes() {
        let idx = ((crc ^ u32::from(b)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
    }
    !crc
}

/// Recalcule l'état CRC pour le mot de 4 octets démarrant à `seed`
/// (port de `UpdateCrcStateFast`, `CriwareCrypt.cs` L113).
fn update_crc_state(seed: u32, k: [u8; 4]) -> u32 {
    let mut crc = !seed;
    let mut i = 0;
    while i < 4 {
        let idx = ((crc & 0xFF) as u8 ^ k[i]) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
        i += 1;
    }
    !crc
}

/// Calcule l'octet XOR pour `position` (0..4) dans le mot CRC courant
/// (port de `ComputeXorByte`, `CriwareCrypt.cs` L101).
fn compute_xor_byte(crc: u32, position: u32) -> u8 {
    let base_shift = position * 2;
    let mut r8 = (crc >> (base_shift + 8)) & 3;
    r8 |= ((crc >> base_shift) & 0xFF) << 2 & 0xFF;
    r8 = ((r8 << 2) & 0xFF) | ((crc >> (base_shift + 16)) & 3);
    r8 = ((r8 << 2) & 0xFF) | ((crc >> (base_shift + 24)) & 3);
    r8 as u8
}

/// Déchiffre (en place, position-aware) un bloc CPK CRI.
///
/// Port fidèle de `CriwareCrypt.DecryptBlock` / `NativeCrypto.DecryptBlock`. Le chiffrement
/// est un XOR involutif : appeler deux fois avec la même clé et le même `file_offset` rétablit
/// le clair. `file_offset` est la position **absolue** du début de `buffer` dans le fichier
/// (permet le déchiffrement aléatoire / par fenêtres).
///
/// `key` est typiquement issue de [`key_from_filename`] ; on peut aussi passer une clé
/// arbitraire (ex. [`VIOLA_FIXED_KEY`]).
pub fn decrypt_block(buffer: &mut [u8], file_offset: u64, key: u32) {
    if buffer.is_empty() {
        return;
    }

    let k = [
        key as u8,
        (key >> 8) as u8,
        (key >> 16) as u8,
        (key >> 24) as u8,
    ];

    let length = buffer.len();
    let mut i = 0usize;

    // Aligner sur une frontière de 4 octets si nécessaire.
    let align_offset = file_offset & 3;
    if align_offset != 0 {
        let current_crc = update_crc_state((file_offset & !3u64) as u32, k);
        let align_count = core::cmp::min((4 - align_offset) as usize, length);
        let mut j = 0;
        while j < align_count {
            let pos = ((file_offset + i as u64) & 3) as u32;
            buffer[i] ^= compute_xor_byte(current_crc, pos);
            j += 1;
            i += 1;
        }
    }

    // Boucle principale : 4 octets à la fois (un mot CRC complet).
    let mut base_offset = file_offset + i as u64;
    while i + 4 <= length {
        let crc = update_crc_state(base_offset as u32, k);
        buffer[i] ^= compute_xor_byte(crc, 0);
        buffer[i + 1] ^= compute_xor_byte(crc, 1);
        buffer[i + 2] ^= compute_xor_byte(crc, 2);
        buffer[i + 3] ^= compute_xor_byte(crc, 3);
        i += 4;
        base_offset += 4;
    }

    // Octets restants (< 4).
    if i < length {
        let crc = update_crc_state(base_offset as u32, k);
        let mut pos = 0u32;
        while i < length {
            buffer[i] ^= compute_xor_byte(crc, pos);
            i += 1;
            pos += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Déchiffrement du cpk_list.cfg.bin (AES-256-CBC, reversé de nie.exe)
// ---------------------------------------------------------------------------
//
// Le `data/cpk_list.cfg.bin` (index logique chemin→CPK des ~250 800 fichiers) N'EST
// PAS chiffré par l'enveloppe CRI position-XOR (ni la clé fixe Viola ni une clé dérivée
// du nom) — c'est une couche distincte, plus forte. Reversé statiquement depuis le
// loader de `nie.exe` (fonction @ VA 0x14168D5E0, build Steam 2026, image base
// 0x140000000) :
//
//   1. KEY (32 o) = decrypt_block(blob_obfusqué_256bits, offset=0, seed=0x8A90ABA9)
//   2. IV  (16 o) = decrypt_block(blob_obfusqué_128bits, offset=0, seed=0x4C801618)
//   3. clair = AES-256-CBC-decrypt(fichier, KEY, IV)   (sans unpad : le T2B se borne
//      par ses offsets d'en-tête ; le résidu de bloc final est ignoré par le parseur)
//
// Les blobs `*_OBF` sont les immédiats exacts posés sur la pile par le loader
// (`mov dword [rbp+…], …`), désobfusqués par le MÊME `decrypt_block` (position-XOR CRC32)
// avec une clé fixe — pas une clé de nom de fichier. Vérifié au réel : l'AES-256-CBC
// produit un T2B valide (footer `01 74 32 62`, en-tête cohérent : entries≈254 203,
// string_table_off+len ≈ taille fichier). iecode N'A PAS ce déchiffrement (sa commande
// `crypto decrypt` renvoie « Unknown encryption »).

/// Blob de clé AES-256 obfusqué, embarqué dans `nie.exe` (8 dwords LE, posés sur la pile
/// par le loader cpk_list). Désobfusqué par [`decrypt_block`] + [`CPK_LIST_KEY_SEED`].
const CPK_LIST_KEY_OBF: [u32; 8] = [
    0x72C9_CB21,
    0x178B_F2F9,
    0x6450_E29D,
    0x4DA9_8CD1,
    0x1E90_8D53,
    0x750D_F696,
    0x43D9_A87A,
    0x584F_E242,
];
/// Clé fixe de désobfuscation de la clé AES (immédiat `mov r8d,8A90ABA9h` du loader).
const CPK_LIST_KEY_SEED: u32 = 0x8A90_ABA9;
/// Blob d'IV AES-128 obfusqué (4 dwords LE). Désobfusqué par [`CPK_LIST_IV_SEED`].
const CPK_LIST_IV_OBF: [u32; 4] = [0x3F8E_4B6D, 0xF0C9_492F, 0x3844_E69D, 0xB0CB_1EE3];
/// Clé fixe de désobfuscation de l'IV (immédiat `mov r8d,4C801618h` du loader).
const CPK_LIST_IV_SEED: u32 = 0x4C80_1618;

/// Dérive la clé AES-256 (32 o) et l'IV (16 o) du `cpk_list.cfg.bin`, byte-exact comme
/// `nie.exe` : désobfuscation des blobs embarqués par [`decrypt_block`] (position-XOR CRC32).
#[must_use]
pub fn cpk_list_key_iv() -> ([u8; 32], [u8; 16]) {
    let mut key = [0u8; 32];
    for (i, w) in CPK_LIST_KEY_OBF.iter().enumerate() {
        key[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    decrypt_block(&mut key, 0, CPK_LIST_KEY_SEED);

    let mut iv = [0u8; 16];
    for (i, w) in CPK_LIST_IV_OBF.iter().enumerate() {
        iv[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    decrypt_block(&mut iv, 0, CPK_LIST_IV_SEED);

    (key, iv)
}

/// Déchiffre le `cpk_list.cfg.bin` (AES-256-CBC) → cfg.bin T2B clair.
///
/// Reversé de `nie.exe` (cf. module-doc de cette section). La clé/IV sont dérivés par
/// [`cpk_list_key_iv`]. `data` doit être le fichier chiffré brut (taille multiple de 16).
///
/// # Errors
/// [`FormatError::Corrupt`] si la taille n'est pas un multiple de bloc AES (16 o) — signe
/// que l'entrée n'est pas un `cpk_list.cfg.bin` AES-CBC de ce build.
pub fn decrypt_cpk_list(data: &[u8]) -> Result<Vec<u8>, FormatError> {
    use aes::Aes256;
    use aes::cipher::generic_array::GenericArray;
    use aes::cipher::{BlockDecrypt, KeyInit};

    if data.is_empty() || !data.len().is_multiple_of(16) {
        return Err(FormatError::Corrupt(
            "cpk_list: taille non multiple de 16 (pas un AES-256-CBC de ce build)",
        ));
    }
    let (key, iv) = cpk_list_key_iv();
    let cipher = Aes256::new(GenericArray::from_slice(&key));

    let mut out = Vec::with_capacity(data.len());
    let mut prev = iv;
    for chunk in data.chunks_exact(16) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        for j in 0..16 {
            out.push(block[j] ^ prev[j]);
        }
        prev.copy_from_slice(chunk); // CBC : le bloc chiffré courant devient le « previous »
    }
    Ok(out)
}

/// Rechiffre un `cpk_list.cfg.bin` clair en AES-256-CBC — inverse exact de
/// [`decrypt_cpk_list`], même clé et même IV ([`cpk_list_key_iv`]).
///
/// Nécessaire au *pack* de mod ([`crate::viola::pack_mod`]) : mettre à jour les tailles des
/// fichiers remplacés suppose de réécrire le fichier dans l'enveloppe où on l'a trouvé.
///
/// **Complément par des zéros jusqu'au multiple de 16.** L'AES-CBC impose des blocs pleins et
/// le fichier d'origine est déjà aligné, mais un `cfg.bin` réencodé ne l'est pas forcément. Le
/// remplissage est nul plutôt que PKCS#7 parce que [`decrypt_cpk_list`] ne retire aucun
/// remplissage : ajouter des octets PKCS#7 les laisserait dans le clair et décalerait la lecture.
/// Un lecteur T2B s'arrête au nombre d'entrées déclaré, donc des zéros en queue sont inertes —
/// **vérifié sur le décodeur du dépôt, pas contre `nie.exe`**.
#[must_use]
pub fn encrypt_cpk_list(data: &[u8]) -> Vec<u8> {
    use aes::Aes256;
    use aes::cipher::generic_array::GenericArray;
    use aes::cipher::{BlockEncrypt, KeyInit};

    let (key, iv) = cpk_list_key_iv();
    let cipher = Aes256::new(GenericArray::from_slice(&key));

    let mut clair = data.to_vec();
    while !clair.len().is_multiple_of(16) {
        clair.push(0);
    }

    let mut out = Vec::with_capacity(clair.len());
    let mut prev = iv;
    for chunk in clair.chunks_exact(16) {
        let mut bloc = [0u8; 16];
        for j in 0..16 {
            bloc[j] = chunk[j] ^ prev[j]; // CBC : XOR avec le bloc chiffré précédent
        }
        let mut bloc = GenericArray::clone_from_slice(&bloc);
        cipher.encrypt_block(&mut bloc);
        prev.copy_from_slice(&bloc);
        out.extend_from_slice(&bloc);
    }
    out
}

/// Déchiffre en place tout un CPK chiffré, clé dérivée de `filename`.
///
/// Le `buffer` doit contenir l'intégralité du fichier à partir de l'offset 0. Après l'appel,
/// `buffer` commence par `CPK ` si le déchiffrement est correct (clé adéquate). Cette fonction
/// **ne vérifie pas** le résultat (utiliser [`decrypt_and_check_cpk`] pour valider).
///
/// `filename` = nom de base du fichier (ex. `1d08dda99e8549431c6c7b459ed8deab.cpk`).
pub fn decrypt_cpk_in_place(buffer: &mut [u8], filename: &str) {
    let key = key_from_filename(filename);
    decrypt_block(buffer, 0, key);
}

/// Déchiffre un CPK chiffré et **vérifie** que le résultat est plausible.
///
/// Dérive la clé du `filename`, déchiffre tout le `buffer` en place, puis exige que le résultat
/// commence par le magic `CPK ` ET porte une table `@UTF` à l'offset 0x10 (en-tête CPK canonique).
/// C'est la garantie anti-hallucination : si la clé ne « colle » pas au réel, l'appel échoue au
/// lieu de renvoyer des octets fabriqués.
///
/// Retourne la clé dérivée en cas de succès.
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si `buffer` < 0x14 octets.
/// - [`FormatError::BadMagic`] si après déchiffrement le magic n'est pas `CPK ` ou si la table
///   `@UTF` attendue à 0x10 est absente.
pub fn decrypt_and_check_cpk(buffer: &mut [u8], filename: &str) -> Result<u32, FormatError> {
    if buffer.len() < 0x14 {
        return Err(FormatError::TooShort {
            got: buffer.len(),
            need: 0x14,
        });
    }
    let key = key_from_filename(filename);
    decrypt_block(buffer, 0, key);

    if buffer[..4] != CPK_MAGIC {
        return Err(FormatError::BadMagic {
            format: "CPK (déchiffrement)",
        });
    }
    if buffer[0x10..0x14] != UTF_MAGIC {
        return Err(FormatError::BadMagic {
            format: "@UTF embarqué CPK (déchiffrement)",
        });
    }
    Ok(key)
}

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

/// Type d'une colonne @UTF (bits 0-3 du byte de flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ColumnType {
    /// `u8`
    U8 = 0,
    /// `i8`
    I8 = 1,
    /// `u16`
    U16 = 2,
    /// `i16`
    I16 = 3,
    /// `u32`
    U32 = 4,
    /// `i32`
    I32 = 5,
    /// `u64`
    U64 = 6,
    /// `i64`
    I64 = 7,
    /// `f32`
    F32 = 8,
    /// `f64`
    F64 = 9,
    /// Chaîne nulle-terminée dans le string pool (4 octets d'offset wire).
    String = 10,
    /// Blob binaire dans le data pool (4 octets offset + 4 octets taille wire).
    Bytes = 11,
    /// GUID 16 octets (rare).
    Guid = 12,
}

impl ColumnType {
    /// Taille wire en octets pour une valeur de ce type.
    #[must_use]
    pub fn wire_size(self) -> usize {
        match self {
            Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 | Self::String => 4,
            Self::U64 | Self::I64 | Self::F64 | Self::Bytes => 8,
            Self::Guid => 16,
        }
    }

    fn from_nibble(v: u8) -> Result<Self, FormatError> {
        Ok(match v {
            0 => Self::U8,
            1 => Self::I8,
            2 => Self::U16,
            3 => Self::I16,
            4 => Self::U32,
            5 => Self::I32,
            6 => Self::U64,
            7 => Self::I64,
            8 => Self::F32,
            9 => Self::F64,
            10 => Self::String,
            11 => Self::Bytes,
            12 => Self::Guid,
            _ => return Err(FormatError::Corrupt("type de colonne @UTF inconnu")),
        })
    }
}

/// Valeur décodée d'une cellule @UTF.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UtfValue {
    /// Octet non signé.
    U8(u8),
    /// Octet signé.
    I8(i8),
    /// `u16`
    U16(u16),
    /// `i16`
    I16(i16),
    /// `u32`
    U32(u32),
    /// `i32`
    I32(i32),
    /// `u64`
    U64(u64),
    /// `i64`
    I64(i64),
    /// `f32`
    F32(f32),
    /// `f64`
    F64(f64),
    /// Chaîne UTF-8 résolue depuis le string pool.
    String(String),
    /// Blob binaire depuis le data pool.
    Bytes(Vec<u8>),
}

impl UtfValue {
    /// Convertit en `i64` (élargissement des entiers et flottants). Retourne `None` pour les
    /// autres variantes.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::U8(v) => Some(*v as i64),
            Self::I8(v) => Some(*v as i64),
            Self::U16(v) => Some(*v as i64),
            Self::I16(v) => Some(*v as i64),
            Self::U32(v) => Some(*v as i64),
            Self::I32(v) => Some(*v as i64),
            Self::U64(v) => Some(*v as i64),
            Self::I64(v) => Some(*v),
            _ => None,
        }
    }

    /// Retourne la référence de chaîne si la valeur est une `String`.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(s) = self {
            Some(s)
        } else {
            None
        }
    }

    /// Retourne le slice du blob si la valeur est `Bytes`.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        if let Self::Bytes(b) = self {
            Some(b)
        } else {
            None
        }
    }
}

/// Descripteur d'une colonne @UTF.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UtfColumn {
    /// Nom résolu depuis le string pool.
    pub name: String,
    /// Type des valeurs de cette colonne.
    pub col_type: ColumnType,
    /// Byte de flags brut. Bits 0-3 = type ; bits 4-7 = flags indépendants
    /// (0x10 = HAS_NAME, 0x20 = HAS_DEFAULT, 0x40 = ROW_STORAGE).
    pub flags: u8,
}

/// Table @UTF complètement décodée.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UtfTable {
    /// Nom de la table (depuis le string pool).
    pub name: String,
    /// Descripteurs de colonnes.
    pub columns: Vec<UtfColumn>,
    /// Lignes de données. Chaque ligne a exactement `columns.len()` valeurs.
    pub rows: Vec<Vec<UtfValue>>,
}

impl UtfTable {
    /// Retourne l'index de la colonne par nom, ou `None`.
    #[must_use]
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    /// Retourne la valeur d'une cellule par (ligne, nom de colonne).
    #[must_use]
    pub fn get(&self, row: usize, col: &str) -> Option<&UtfValue> {
        let ci = self.column_index(col)?;
        self.rows.get(row)?.get(ci)
    }

    /// Raccourci : valeur entière (élargissement).
    #[must_use]
    pub fn get_i64(&self, row: usize, col: &str) -> Option<i64> {
        self.get(row, col)?.as_i64()
    }

    /// Raccourci : référence de chaîne.
    #[must_use]
    pub fn get_str(&self, row: usize, col: &str) -> Option<&str> {
        self.get(row, col)?.as_str()
    }

    /// Nombre de lignes.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Nombre de colonnes.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
}

/// En-tête d'archive CPK (table @UTF embarquée).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CpkHeader {
    /// Table @UTF du header CPK.
    pub utf: UtfTable,
}

// ---------------------------------------------------------------------------
// API publique
// ---------------------------------------------------------------------------

/// Vrai si `data` commence par le magic `@UTF`.
#[must_use]
pub fn is_utf(data: &[u8]) -> bool {
    data.starts_with(&UTF_MAGIC)
}

/// Parse une table @UTF depuis un slice d'octets.
///
/// Le slice doit commencer exactement au magic `@UTF`.
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si le tampon est trop court.
/// - [`FormatError::BadMagic`] si le magic est invalide.
/// - [`FormatError::Corrupt`] pour toute incohérence interne.
pub fn parse_utf(data: &[u8]) -> Result<UtfTable, FormatError> {
    if data.len() < UTF_MIN_TOTAL {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: UTF_MIN_TOTAL,
        });
    }
    if !is_utf(data) {
        return Err(FormatError::BadMagic { format: "@UTF" });
    }

    let table_size = read_u32_be(data, 4)? as usize;
    // Vérification souple : on n'exige pas que data.len() == 8 + table_size (sous-table possible).
    let _ = table_size;

    // Tous les offsets internes sont relatifs à UTF_BASE (= 0x08).
    let rows_offset = read_u32_be(data, UTF_BASE)? as usize + UTF_BASE;
    let string_pool = read_u32_be(data, UTF_BASE + 4)? as usize + UTF_BASE;
    let data_pool = read_u32_be(data, UTF_BASE + 8)? as usize + UTF_BASE;
    let table_name_off = read_u32_be(data, UTF_BASE + 12)? as usize;
    let col_count = read_u16_be(data, UTF_BASE + 16)? as usize;
    let row_stride = read_u16_be(data, UTF_BASE + 18)? as usize;
    let row_count = read_u32_be(data, UTF_BASE + 20)? as usize;

    if string_pool > data.len() {
        return Err(FormatError::Corrupt("@UTF : string pool hors limites"));
    }

    let table_name = read_cstr(data, string_pool, table_name_off)?;

    // --- Schéma des colonnes ---
    // Les descripteurs commencent à l'offset 0x20. Chaque descripteur :
    //   1 octet flags
    //   + 4 octets name_off  (UNIQUEMENT si FLAG_HAS_NAME est positionné)
    //   + wire_size(type)    (UNIQUEMENT si FLAG_HAS_DEFAULT est positionné ; valeur constante)
    // Le nibble haut contient des bits indépendants (FLAG_HAS_NAME, FLAG_HAS_DEFAULT,
    // FLAG_ROW_STORAGE) — ce ne sont PAS des valeurs d'un enum de classe de stockage.

    // Structs internes pour les colonnes.
    struct ColDef {
        col_type: ColumnType,
        flags: u8,
        name: String,
        default_value: Option<UtfValue>,
    }

    let mut col_defs: Vec<ColDef> = Vec::with_capacity(col_count);
    let mut col_off = 0x20usize;

    for i in 0..col_count {
        if col_off >= data.len() {
            return Err(FormatError::Corrupt("@UTF : EOF dans le schéma"));
        }
        let flags = data[col_off];
        col_off += 1;

        let type_nibble = flags & 0x0F;
        let col_type = ColumnType::from_nibble(type_nibble)?;

        // Nom : présent uniquement si FLAG_HAS_NAME est positionné.
        let name = if (flags & FLAG_HAS_NAME) != 0 {
            let name_off = read_u32_be(data, col_off)? as usize;
            col_off += 4;
            read_cstr(data, string_pool, name_off).unwrap_or_else(|_| alloc::format!("col_{i}"))
        } else {
            alloc::format!("col_{i}")
        };

        // Valeur par défaut : présente uniquement si FLAG_HAS_DEFAULT est positionné.
        // Cette valeur est constante pour toutes les lignes et ne consomme AUCUN octet
        // par ligne dans la section rows.
        let default_value = if (flags & FLAG_HAS_DEFAULT) != 0 {
            let val = read_value(data, col_off, col_type, string_pool, data_pool)?;
            col_off += col_type.wire_size();
            Some(val)
        } else {
            None
        };

        col_defs.push(ColDef {
            col_type,
            flags,
            name,
            default_value,
        });
    }

    // --- Lignes ---
    // Certains ACB (Criware) encodent un `rows_offset` résiduel qui ne désigne pas la zone
    // des lignes. La zone réelle est alors juste après le schéma de colonnes : elle s'étend de
    // (string_pool - row_stride*row_count) jusqu'à string_pool.
    //
    // La détection porte sur l'INVARIANT DE LAYOUT d'une table @UTF — schéma, puis lignes, puis
    // pool de chaînes, puis pool de données — et non sur la taille du fichier : les lignes
    // finissent au plus tard au début du pool de chaînes. Détecter par `rows_offset >= data.len()`
    // ne marchait que par accident, quand l'offset résiduel dépassait le fichier ; sur une banque
    // assez volumineuse (`bgm.acb` 231 ko, `waza_stream.acb` 372 ko) il tombe DEDANS, le repli ne
    // se déclenchait pas et la ligne était lue dans du bruit — l'ACB entier devenait illisible.
    //
    // La condition est strictement plus générale que l'ancienne : `string_pool <= data.len()` est
    // déjà vérifié plus haut, donc tout cas qui déclenchait l'ancien repli déclenche celui-ci.
    let fin_lignes = rows_offset.saturating_add(row_stride.saturating_mul(row_count));
    let effective_rows_offset = if row_count > 0 && fin_lignes > string_pool {
        string_pool.saturating_sub(row_stride * row_count)
    } else {
        rows_offset
    };

    let mut rows: Vec<Vec<UtfValue>> = Vec::with_capacity(row_count);
    for r in 0..row_count {
        let row_base = effective_rows_offset + r * row_stride;
        let mut row_cursor = row_base;
        let mut row: Vec<UtfValue> = Vec::with_capacity(col_count);

        for def in &col_defs {
            // Priorité : FLAG_HAS_DEFAULT > FLAG_ROW_STORAGE > zéro implicite.
            // FLAG_HAS_DEFAULT : valeur constante stockée dans le schéma, n'avance PAS row_cursor.
            // FLAG_ROW_STORAGE : valeur inline dans la section rows, avance row_cursor.
            // Ni l'un ni l'autre : valeur zéro, n'avance pas row_cursor.
            let val = if (def.flags & FLAG_HAS_DEFAULT) != 0 {
                def.default_value
                    .clone()
                    .ok_or(FormatError::Corrupt("@UTF : valeur constante manquante"))?
            } else if (def.flags & FLAG_ROW_STORAGE) != 0 {
                let v = read_value(data, row_cursor, def.col_type, string_pool, data_pool)?;
                row_cursor += def.col_type.wire_size();
                v
            } else {
                zero_value(def.col_type)
            };
            row.push(val);
        }
        rows.push(row);
    }

    let columns: Vec<UtfColumn> = col_defs
        .into_iter()
        .map(|d| UtfColumn {
            name: d.name,
            col_type: d.col_type,
            flags: d.flags,
        })
        .collect();

    Ok(UtfTable {
        name: table_name,
        columns,
        rows,
    })
}

/// Parse l'en-tête d'une archive CPK (non chiffrée).
///
/// `data` doit pointer vers l'offset 0 du fichier CPK.
///
/// # Erreurs
///
/// - [`FormatError::BadMagic`] si le magic n'est pas `CPK ` (les CPK IEVR chiffrés
///   échouent ici systématiquement — c'est attendu).
/// - Toute erreur de [`parse_utf`] pour la table embarquée.
pub fn parse_cpk(data: &[u8]) -> Result<CpkHeader, FormatError> {
    if data.len() < 0x14 {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: 0x14,
        });
    }
    if data[..4] != CPK_MAGIC {
        return Err(FormatError::BadMagic { format: "CPK" });
    }
    // 12 octets de champs internes CPK → la table @UTF commence à 0x10.
    let utf = parse_utf(&data[0x10..])?;
    Ok(CpkHeader { utf })
}

/// Déchiffre une table @UTF (XOR progressif avec multiplicateur 0x15).
pub fn decrypt_table(data: &mut [u8]) {
    let mut xor_val: u8 = 0x5F;
    for b in data.iter_mut() {
        *b ^= xor_val;
        xor_val = xor_val.wrapping_mul(0x15);
    }
}

/// Parse une table @UTF depuis un conteneur CRI (CPK/TOC/ETOC).
///
/// Le conteneur fait 16 octets : magic(4) + pad(4) + size_le(4) + pad(4),
/// suivi des données de la table.
pub fn parse_table_container(
    data: &[u8],
    container_offset: usize,
) -> Result<UtfTable, FormatError> {
    if container_offset + 16 > data.len() {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: container_offset + 16,
        });
    }
    let table_size = u32::from_le_bytes(
        data[container_offset + 8..container_offset + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    let table_start = container_offset + 16;
    if table_start + table_size > data.len() {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: table_start + table_size,
        });
    }

    let mut table_data = data[table_start..table_start + table_size].to_vec();
    if table_size >= 4 {
        let magic = u32::from_le_bytes(table_data[0..4].try_into().unwrap());
        // 0x46545540 = "@UTF" en LE
        if magic != 0x46545540 {
            decrypt_table(&mut table_data);
            let dec_magic = u32::from_le_bytes(table_data[0..4].try_into().unwrap());
            if dec_magic != 0x46545540 {
                return Err(FormatError::BadMagic {
                    format: "@UTF dechiffree",
                });
            }
        }
    }

    parse_utf(&table_data)
}

/// Entrée décrivant un fichier dans un CPK.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CpkEntry {
    pub filename: String,
    pub directory: String,
    pub offset: u64,
    pub size: u64,
    pub extract_size: u64,
    pub is_compressed: bool,
}

/// Lecteur d'archive CPK IEVR complet.
pub struct CpkReader {
    pub entries: Vec<CpkEntry>,
    pub file_key: Option<u32>,
    pub is_encrypted: bool,
}

impl CpkReader {
    /// Initialise et parse l'archive CPK (détecte le chiffrement à la volée).
    pub fn new(data: &[u8], filename: &str) -> Result<Self, FormatError> {
        let mut is_encrypted = false;
        let mut file_key = None;

        if data.len() < 16 {
            return Err(FormatError::TooShort {
                got: data.len(),
                need: 16,
            });
        }

        let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());
        if magic != 0x204B5043 {
            // "CPK " en LE
            let name_with_appid = alloc::format!("002AB8F4-{}", filename);
            let key_with_appid = key_from_filename(&name_with_appid);
            let key_without = key_from_filename(filename);

            let try_key = |k: u32| -> bool {
                let mut test = [0u8; 4];
                test.copy_from_slice(&data[0..4]);
                decrypt_block(&mut test, 0, k);
                let dm = u32::from_le_bytes(test);
                dm == 0x204B5043
            };

            if try_key(key_with_appid) {
                is_encrypted = true;
                file_key = Some(key_with_appid);
            } else if try_key(key_without) {
                is_encrypted = true;
                file_key = Some(key_without);
            } else {
                return Err(FormatError::BadMagic {
                    format: "CPK (non dechiffrable)",
                });
            }
        }

        let read_region = |offset: usize, size: usize| -> Result<Vec<u8>, FormatError> {
            if offset + size > data.len() {
                return Err(FormatError::TooShort {
                    got: data.len(),
                    need: offset + size,
                });
            }
            let mut region = data[offset..offset + size].to_vec();
            if is_encrypted && let Some(k) = file_key {
                decrypt_block(&mut region, offset as u64, k);
            }
            Ok(region)
        };

        let header_container = read_region(0, 16)?;
        let header_table_size =
            u32::from_le_bytes(header_container[8..12].try_into().unwrap()) as usize;
        let header_total = 16 + header_table_size;

        let header_data = read_region(0, header_total)?;
        let header_utf = parse_table_container(&header_data, 0)?;

        let toc_offset = header_utf.get_i64(0, "TocOffset").unwrap_or(-1);
        let mut content_offset = header_utf.get_i64(0, "ContentOffset").unwrap_or(-1);
        if content_offset < 0 || toc_offset < content_offset {
            content_offset = toc_offset;
        }

        if toc_offset < 0 {
            return Err(FormatError::Corrupt("TocOffset introuvable"));
        }

        let toc_abs = toc_offset as usize;
        let toc_container = read_region(toc_abs, 16)?;
        let toc_table_size = u32::from_le_bytes(toc_container[8..12].try_into().unwrap()) as usize;
        let toc_total = 16 + toc_table_size;

        let toc_data = read_region(toc_abs, toc_total)?;
        let toc_utf = parse_table_container(&toc_data, 0)?;

        let mut entries = Vec::with_capacity(toc_utf.row_count());
        for i in 0..toc_utf.row_count() {
            let fname = toc_utf.get_str(i, "FileName").unwrap_or("").to_owned();
            let dirname = toc_utf.get_str(i, "DirName").unwrap_or("").to_owned();
            let file_off = toc_utf.get_i64(i, "FileOffset").unwrap_or(0);
            let file_size = toc_utf.get_i64(i, "FileSize").unwrap_or(0) as u64;
            let extract_size = toc_utf.get_i64(i, "ExtractSize").unwrap_or(0) as u64;
            let offset = (file_off + content_offset) as u64;
            let is_compressed = extract_size != file_size && file_size > 0;

            entries.push(CpkEntry {
                filename: fname,
                directory: dirname,
                offset,
                size: file_size,
                extract_size,
                is_compressed,
            });
        }

        Ok(CpkReader {
            entries,
            file_key,
            is_encrypted,
        })
    }

    /// Extrait et décompresse un fichier depuis le CPK.
    pub fn extract(&self, data: &[u8], entry: &CpkEntry) -> Result<Vec<u8>, FormatError> {
        if entry.size == 0 {
            return Ok(Vec::new());
        }
        let offset = entry.offset as usize;
        let size = entry.size as usize;
        if offset + size > data.len() {
            return Err(FormatError::TooShort {
                got: data.len(),
                need: offset + size,
            });
        }
        let mut raw = data[offset..offset + size].to_vec();
        if self.is_encrypted
            && let Some(k) = self.file_key
        {
            decrypt_block(&mut raw, offset as u64, k);
        }

        if entry.is_compressed && crate::detect(&raw) == crate::FileFormat::CriLayla {
            let decompressed = crate::crilayla::decompress(&raw)
                .map_err(|_| FormatError::Corrupt("echec de decompression CRILAYLA"))?;
            Ok(decompressed)
        } else {
            Ok(raw)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers internes (pas d'unsafe, no_std-friendly)
// ---------------------------------------------------------------------------

fn read_u8(data: &[u8], off: usize) -> Result<u8, FormatError> {
    data.get(off)
        .copied()
        .ok_or(FormatError::Corrupt("@UTF : lecture u8 hors limites"))
}

fn read_u16_be(data: &[u8], off: usize) -> Result<u16, FormatError> {
    let bytes: [u8; 2] = data
        .get(off..off + 2)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("@UTF : lecture u16 hors limites"))?;
    Ok(u16::from_be_bytes(bytes))
}

fn read_u32_be(data: &[u8], off: usize) -> Result<u32, FormatError> {
    let bytes: [u8; 4] = data
        .get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("@UTF : lecture u32 hors limites"))?;
    Ok(u32::from_be_bytes(bytes))
}

fn read_u64_be(data: &[u8], off: usize) -> Result<u64, FormatError> {
    let bytes: [u8; 8] = data
        .get(off..off + 8)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("@UTF : lecture u64 hors limites"))?;
    Ok(u64::from_be_bytes(bytes))
}

fn read_cstr(data: &[u8], pool_base: usize, pool_off: usize) -> Result<String, FormatError> {
    let start = pool_base
        .checked_add(pool_off)
        .ok_or(FormatError::Corrupt("@UTF : overflow offset string pool"))?;
    if start >= data.len() {
        return Err(FormatError::Corrupt(
            "@UTF : offset string pool hors limites",
        ));
    }
    let slice = &data[start..];
    let nul = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    // Accepte du latin-1 de façon permissive via from_utf8_lossy.
    let s =
        alloc::string::ToString::to_string(&alloc::string::String::from_utf8_lossy(&slice[..nul]));
    Ok(s)
}

fn zero_value(col_type: ColumnType) -> UtfValue {
    match col_type {
        ColumnType::U8 => UtfValue::U8(0),
        ColumnType::I8 => UtfValue::I8(0),
        ColumnType::U16 => UtfValue::U16(0),
        ColumnType::I16 => UtfValue::I16(0),
        ColumnType::U32 => UtfValue::U32(0),
        ColumnType::I32 => UtfValue::I32(0),
        ColumnType::U64 => UtfValue::U64(0),
        ColumnType::I64 => UtfValue::I64(0),
        ColumnType::F32 => UtfValue::F32(0.0),
        ColumnType::F64 => UtfValue::F64(0.0),
        ColumnType::String => UtfValue::String(String::new()),
        ColumnType::Bytes | ColumnType::Guid => UtfValue::Bytes(Vec::new()),
    }
}

fn read_value(
    data: &[u8],
    off: usize,
    col_type: ColumnType,
    string_pool: usize,
    data_pool: usize,
) -> Result<UtfValue, FormatError> {
    Ok(match col_type {
        ColumnType::U8 => UtfValue::U8(read_u8(data, off)?),
        ColumnType::I8 => UtfValue::I8(read_u8(data, off)? as i8),
        ColumnType::U16 => UtfValue::U16(read_u16_be(data, off)?),
        ColumnType::I16 => UtfValue::I16(read_u16_be(data, off)? as i16),
        ColumnType::U32 => UtfValue::U32(read_u32_be(data, off)?),
        ColumnType::I32 => UtfValue::I32(read_u32_be(data, off)? as i32),
        ColumnType::U64 => UtfValue::U64(read_u64_be(data, off)?),
        ColumnType::I64 => UtfValue::I64(read_u64_be(data, off)? as i64),
        ColumnType::F32 => {
            let bits = read_u32_be(data, off)?;
            UtfValue::F32(f32::from_bits(bits))
        }
        ColumnType::F64 => {
            let bits = read_u64_be(data, off)?;
            UtfValue::F64(f64::from_bits(bits))
        }
        ColumnType::String => {
            let pool_off = read_u32_be(data, off)? as usize;
            UtfValue::String(read_cstr(data, string_pool, pool_off)?)
        }
        ColumnType::Bytes => {
            let blob_off = read_u32_be(data, off)? as usize;
            let blob_len = read_u32_be(data, off + 4)? as usize;
            let abs_start = data_pool
                .checked_add(blob_off)
                .ok_or(FormatError::Corrupt("@UTF : overflow data pool"))?;
            if blob_len == 0 {
                UtfValue::Bytes(Vec::new())
            } else {
                let abs_end = abs_start
                    .checked_add(blob_len)
                    .ok_or(FormatError::Corrupt("@UTF : overflow blob end"))?;
                if abs_end > data.len() {
                    return Err(FormatError::Corrupt("@UTF : blob data hors limites"));
                }
                UtfValue::Bytes(data[abs_start..abs_end].to_vec())
            }
        }
        ColumnType::Guid => {
            if off + 16 > data.len() {
                UtfValue::Bytes(Vec::new())
            } else {
                UtfValue::Bytes(data[off..off + 16].to_vec())
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------
    // Constructeur de table @UTF minimale pour les tests
    // -------------------------------------------------------------------

    /// Construit une table @UTF avec 2 colonnes et 2 lignes.
    ///
    /// - Col 0 "ColA" : U32, HAS_NAME|ROW_STORAGE (flags 0x54)
    /// - Col 1 "ColB" : String, HAS_NAME|ROW_STORAGE (flags 0x5A)
    /// - Ligne 0 : ColA=42, ColB="hello"
    /// - Ligne 1 : ColA=99, ColB="world"
    fn build_utf_fixture() -> Vec<u8> {
        // String pool (null-terminated, compact)
        // Offset 0  : "TestTable\0" (10)
        // Offset 10 : "ColA\0"      (5)
        // Offset 15 : "ColB\0"      (5)
        // Offset 20 : "hello\0"     (6)
        // Offset 26 : "world\0"     (6)
        let string_pool: &[u8] = b"TestTable\0ColA\0ColB\0hello\0world\0";
        assert_eq!(string_pool.len(), 32);

        // Schéma : 2 colonnes × 5 octets chacune (1 byte flags + 4 bytes name_off).
        // flags = FLAG_HAS_NAME(0x10) | FLAG_ROW_STORAGE(0x40) | type
        let schema: &[u8] = &[
            0x54, 0x00, 0x00, 0x00,
            0x0A, // Col 0 : flags=0x54 (HAS_NAME|ROW|U32), name_off=10
            0x5A, 0x00, 0x00, 0x00,
            0x0F, // Col 1 : flags=0x5A (HAS_NAME|ROW|String), name_off=15
        ];

        // Row data : 2 lignes, stride = 8 (4 + 4).
        let row_data: &[u8] = &[
            // Ligne 0
            0x00, 0x00, 0x00, 42, // ColA = 42
            0x00, 0x00, 0x00, 20, // ColB → pool[20] = "hello"
            // Ligne 1
            0x00, 0x00, 0x00, 99, // ColA = 99
            0x00, 0x00, 0x00, 26, // ColB → pool[26] = "world"
        ];

        // Calcul des offsets (relatifs à UTF_BASE = 0x08).
        //
        // Layout du body (corps après les 8 octets outer header) :
        //  body[0x00..0x18)  → header body (24 octets)
        //  body[0x18..0x22)  → schéma 2 colonnes × 5 octets = 10 octets
        //  body[0x22..0x32)  → row data 2 lignes × 8 octets = 16 octets
        //  body[0x32..0x52)  → string pool 32 octets
        //  body[0x52..)      → data pool (vide)
        //
        // Les offsets stockés dans le header body sont des positions dans le body
        // (= position absolue dans `data` - UTF_BASE) :
        //   rows_offset_rel   = 0x22  (body[0x22])
        //   string_offset_rel = 0x32  (body[0x32])
        //   data_offset_rel   = 0x52  (body[0x52])
        //   table_name_off    = 0     (pool[0] = "TestTable")

        let rows_offset_rel: u32 = 0x22;
        let string_offset_rel: u32 = 0x32;
        let data_offset_rel: u32 = 0x52;
        let table_name_off: u32 = 0;
        let col_count: u16 = 2;
        let row_stride: u16 = 8;
        let row_count: u32 = 2;

        let mut body: Vec<u8> = Vec::new();
        // Header body (0x18 octets).
        body.extend_from_slice(&rows_offset_rel.to_be_bytes()); // +0x00
        body.extend_from_slice(&string_offset_rel.to_be_bytes()); // +0x04
        body.extend_from_slice(&data_offset_rel.to_be_bytes()); // +0x08
        body.extend_from_slice(&table_name_off.to_be_bytes()); // +0x0C
        body.extend_from_slice(&col_count.to_be_bytes()); // +0x10
        body.extend_from_slice(&row_stride.to_be_bytes()); // +0x12
        body.extend_from_slice(&row_count.to_be_bytes()); // +0x14
        // Schema.
        body.extend_from_slice(schema);
        // Row data.
        body.extend_from_slice(row_data);
        // String pool.
        body.extend_from_slice(string_pool);
        // Data pool (vide).

        // Outer header : magic + table_size.
        let table_size = body.len() as u32;
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(&UTF_MAGIC);
        out.extend_from_slice(&table_size.to_be_bytes());
        out.extend_from_slice(&body);
        out
    }

    #[test]
    fn parse_fixture_deux_lignes() {
        let data = build_utf_fixture();
        let table = parse_utf(&data).expect("parse_utf doit réussir sur la fixture");

        assert_eq!(table.name, "TestTable");
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.columns[0].name, "ColA");
        assert_eq!(table.columns[1].name, "ColB");
        assert_eq!(table.row_count(), 2);

        assert_eq!(table.rows[0][0], UtfValue::U32(42));
        assert_eq!(table.rows[0][1], UtfValue::String("hello".into()));
        assert_eq!(table.rows[1][0], UtfValue::U32(99));
        assert_eq!(table.rows[1][1], UtfValue::String("world".into()));
    }

    #[test]
    fn get_helpers() {
        let data = build_utf_fixture();
        let table = parse_utf(&data).unwrap();

        assert_eq!(table.get_i64(0, "ColA"), Some(42));
        assert_eq!(table.get_str(1, "ColB"), Some("world"));
        assert_eq!(table.get(0, "Inconnu"), None);
    }

    #[test]
    fn mauvais_magic_utf_rejete() {
        let mut data = build_utf_fixture();
        data[0] = b'X';
        assert!(matches!(
            parse_utf(&data),
            Err(FormatError::BadMagic { .. })
        ));
    }

    #[test]
    fn trop_court_rejete() {
        let data = b"@UTF\x00\x00\x00\x0F"; // trop court (< 0x20)
        assert!(matches!(parse_utf(data), Err(FormatError::TooShort { .. })));
    }

    #[test]
    fn cpk_mauvais_magic_rejete() {
        let data = build_utf_fixture();
        // Préfixe avec un faux magic CPK (4 octets) + 12 zéros d'en-tête interne.
        let mut cpk_data = alloc::vec![b'X', b'P', b'K', b' '];
        cpk_data.extend_from_slice(&[0u8; 12]);
        cpk_data.extend_from_slice(&data);
        assert!(matches!(
            parse_cpk(&cpk_data),
            Err(FormatError::BadMagic { .. })
        ));
    }

    #[test]
    fn cpk_valide_parse() {
        let utf_bytes = build_utf_fixture();
        let mut cpk_data: Vec<u8> = Vec::new();
        cpk_data.extend_from_slice(&CPK_MAGIC);
        cpk_data.extend_from_slice(&[0u8; 12]);
        cpk_data.extend_from_slice(&utf_bytes);

        let header = parse_cpk(&cpk_data).expect("CPK valide doit être parsé");
        assert_eq!(header.utf.name, "TestTable");
        assert_eq!(header.utf.row_count(), 2);
    }

    #[test]
    fn is_utf_detection() {
        assert!(is_utf(&UTF_MAGIC));
        assert!(is_utf(b"@UTF\x00\x00\x00\x00\x00"));
        assert!(!is_utf(b"@utF\x00\x00\x00\x00"));
        assert!(!is_utf(b""));
    }

    #[test]
    fn storage_zero_produit_valeur_nulle() {
        // Construire une table avec une colonne nommée sans stockage (pas de HAS_DEFAULT ni
        // ROW_STORAGE). flags = FLAG_HAS_NAME(0x10) | U32(0x04) = 0x14.
        // Elle doit produire UtfValue::U32(0) pour chaque ligne (zéro implicite).

        // String pool minimal.
        let string_pool: &[u8] = b"\0ColZ\0"; // table_name = "" (off 0), col = "ColZ" (off 1)

        // Schema : 1 col, flags = 0x14 (HAS_NAME | U32), name_off = 1.
        // Ni HAS_DEFAULT ni ROW_STORAGE → valeur zéro, n'avance pas row_cursor.
        let schema: &[u8] = &[0x14, 0x00, 0x00, 0x00, 0x01];

        // 1 ligne, stride = 0 (aucune col row-storage).
        let row_data: &[u8] = &[];

        // header body = 0x18 octets, schéma = 5 octets (1 flags + 4 name_off car HAS_NAME)
        // rows à body[0x1D] → rows_offset_rel = 0x1D (relatif à UTF_BASE)
        let rows_offset_rel: u32 = 0x1D;
        let string_offset_rel: u32 = rows_offset_rel; // stride=0 → 0 octets de row data
        let data_offset_rel: u32 = string_offset_rel + string_pool.len() as u32;

        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&rows_offset_rel.to_be_bytes());
        body.extend_from_slice(&string_offset_rel.to_be_bytes());
        body.extend_from_slice(&data_offset_rel.to_be_bytes());
        body.extend_from_slice(&0u32.to_be_bytes()); // table_name_off = 0
        body.extend_from_slice(&1u16.to_be_bytes()); // col_count = 1
        body.extend_from_slice(&0u16.to_be_bytes()); // row_stride = 0
        body.extend_from_slice(&1u32.to_be_bytes()); // row_count = 1
        body.extend_from_slice(schema);
        body.extend_from_slice(row_data);
        body.extend_from_slice(string_pool);

        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(&UTF_MAGIC);
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(&body);

        let table = parse_utf(&out).expect("STORAGE_ZERO doit parser");
        assert_eq!(table.row_count(), 1);
        assert_eq!(table.rows[0][0], UtfValue::U32(0));
    }

    #[test]
    fn storage_const_partage_meme_valeur() {
        // Col HAS_NAME|HAS_DEFAULT U32 = 0xFF pour chaque ligne.
        // flags = 0x34 (FLAG_HAS_NAME=0x10 | FLAG_HAS_DEFAULT=0x20 | U32=0x04).
        // La valeur par défaut (constante) suit le name_off dans le schéma.
        // Cette colonne ne consomme AUCUN octet par ligne (row_stride = 0).

        let string_pool: &[u8] = b"\0ConstCol\0";
        let schema: &[u8] = &[
            0x34, // flags = HAS_NAME | HAS_DEFAULT | U32
            0x00, 0x00, 0x00, 0x01, // name_off = 1 → "ConstCol"
            0x00, 0x00, 0x00, 0xFF, // valeur par défaut U32 = 255
        ];

        // body header = 0x18 octets, schéma = 9 octets (1+4+4) → rows commencent à body[0x21]
        // rows_offset relatif à UTF_BASE (0x08) = 0x21
        let rows_offset_rel: u32 = 0x21;
        let string_offset_rel: u32 = rows_offset_rel; // stride=0, 2 lignes → 0 octets de row data, string pool suit immédiatement
        let data_offset_rel: u32 = string_offset_rel + string_pool.len() as u32;

        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&rows_offset_rel.to_be_bytes());
        body.extend_from_slice(&string_offset_rel.to_be_bytes());
        body.extend_from_slice(&data_offset_rel.to_be_bytes());
        body.extend_from_slice(&0u32.to_be_bytes()); // table_name_off = 0 → ""
        body.extend_from_slice(&1u16.to_be_bytes()); // col_count = 1
        body.extend_from_slice(&0u16.to_be_bytes()); // row_stride = 0 (col est CONST)
        body.extend_from_slice(&2u32.to_be_bytes()); // row_count = 2
        body.extend_from_slice(schema);
        // Pas de row data (stride = 0).
        body.extend_from_slice(string_pool);

        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(&UTF_MAGIC);
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(&body);

        let table = parse_utf(&out).expect("STORAGE_CONST doit parser");
        assert_eq!(table.row_count(), 2);
        assert_eq!(table.rows[0][0], UtfValue::U32(255));
        assert_eq!(table.rows[1][0], UtfValue::U32(255));
    }

    // -------------------------------------------------------------------
    // Déchiffrement CRI — valeurs RÉELLES (octets du dump VPS)
    // -------------------------------------------------------------------

    /// Header chiffré réel (512 octets) du CPK `1d08dda99e8549431c6c7b459ed8deab.cpk`,
    /// extrait tel quel du dump Steam IEVR (`data/packs/`). Octets BRUTS chiffrés.
    #[cfg(feature = "real-fixtures")]
    const REAL_CPK_HEAD_ENC: &[u8] =
        include_bytes!("../tests/fixtures/1d08dda99e8549431c6c7b459ed8deab.cpk.head512.enc");

    /// Nom de base correspondant (la clé en dérive).
    const REAL_CPK_NAME: &str = "1d08dda99e8549431c6c7b459ed8deab.cpk";

    #[test]
    fn key_from_filename_valeur_reelle() {
        // Clé recoupée empiriquement : ce nom déchiffre le CPK réel en "CPK ".
        assert_eq!(key_from_filename(REAL_CPK_NAME), 0xBD28_1847);
        // Autres CPK réels du même dump (clés recoupées par déchiffrement → "CPK "/"@UTF").
        assert_eq!(
            key_from_filename("fc62c6effe5a8c4748ab9cb3721c1017.cpk"),
            0x00DE_44F8
        );
        assert_eq!(
            key_from_filename("48d8f85b93565fb22212eb77d7b9eda0.cpk"),
            0x5629_F92E
        );
        assert_eq!(
            key_from_filename("6a3ab1ffb3d2e6c5dc62215be407f063.cpk"),
            0x52B4_1918
        );
    }

    #[cfg(feature = "real-fixtures")]
    #[test]
    fn dechiffre_cpk_reel_donne_magic_cpk_et_utf() {
        // Le test anti-hallucination : on part d'octets RÉELS chiffrés et on exige
        // que le déchiffrement produise un en-tête CPK canonique.
        let mut buf = REAL_CPK_HEAD_ENC.to_vec();
        let key = decrypt_and_check_cpk(&mut buf, REAL_CPK_NAME)
            .expect("le CPK réel doit se déchiffrer en en-tête CPK valide");
        assert_eq!(key, 0xBD28_1847);
        assert_eq!(&buf[..4], b"CPK ");
        assert_eq!(&buf[0x10..0x14], b"@UTF");
    }

    #[cfg(feature = "real-fixtures")]
    #[test]
    fn cle_fixe_viola_ne_dechiffre_pas_un_pack_cpk() {
        // La constante 0x1717E18E (clé Viola cfg.bin) NE déchiffre PAS un .cpk de packs/ :
        // appliquée au header réel, elle ne produit pas "CPK ". On le prouve.
        let mut buf = REAL_CPK_HEAD_ENC.to_vec();
        decrypt_block(&mut buf, 0, VIOLA_FIXED_KEY);
        assert_ne!(
            &buf[..4],
            b"CPK ",
            "0x1717E18E ne doit PAS donner 'CPK ' sur un pack"
        );
    }

    #[cfg(feature = "real-fixtures")]
    #[test]
    fn decrypt_block_est_involutif() {
        // XOR position-aware : déchiffrer puis re-chiffrer (même clé/offset) = identité.
        let mut buf = REAL_CPK_HEAD_ENC.to_vec();
        let original = buf.clone();
        let key = key_from_filename(REAL_CPK_NAME);
        decrypt_block(&mut buf, 0, key);
        decrypt_block(&mut buf, 0, key);
        assert_eq!(
            buf, original,
            "double déchiffrement doit rétablir le clair chiffré"
        );
    }

    #[cfg(feature = "real-fixtures")]
    #[test]
    fn decrypt_block_position_aware_par_fenetres() {
        // Déchiffrer par fenêtres avec le bon file_offset == déchiffrer d'un coup.
        let key = key_from_filename(REAL_CPK_NAME);
        let mut whole = REAL_CPK_HEAD_ENC.to_vec();
        decrypt_block(&mut whole, 0, key);

        let mut windowed = REAL_CPK_HEAD_ENC.to_vec();
        // Fenêtres volontairement non alignées sur 4 (teste le chemin d'alignement).
        for (start, len) in [(0usize, 7usize), (7, 9), (16, 64), (80, 432)] {
            decrypt_block(&mut windowed[start..start + len], start as u64, key);
        }
        assert_eq!(
            whole, windowed,
            "déchiffrement par fenêtres = déchiffrement global"
        );
    }

    #[test]
    fn crc32_table_valeurs_canoniques() {
        // Table CRC32 poly 0xEDB88320 : valeurs canoniques connues.
        assert_eq!(CRC32_TABLE[0], 0x0000_0000);
        assert_eq!(CRC32_TABLE[1], 0x7707_3096);
        assert_eq!(CRC32_TABLE[255], 0x2D02_EF8D);
    }

    #[test]
    fn test_decrypt_table() {
        let mut data = [0x5F ^ 0x40, 0x5F ^ 0x40]; // décrypté donne '@', etc.
        decrypt_table(&mut data);
        assert_eq!(data[0], 0x40);
    }

    #[test]
    fn test_cpk_reader_invalid_data() {
        let bad_data = [0u8; 10];
        let reader = CpkReader::new(&bad_data, "test.cpk");
        assert!(reader.is_err());
    }

    // -------------------------------------------------------------------
    // cpk_list.cfg.bin : AES-256-CBC (clé/IV reversés de nie.exe)
    // -------------------------------------------------------------------

    fn to_hex(b: &[u8]) -> String {
        b.iter().map(|x| alloc::format!("{x:02x}")).collect()
    }

    /// Pin byte-exact de la dérivation clé/IV : désobfuscation des blobs embarqués de
    /// `nie.exe` (loader cpk_list @ 0x14168D5E0). Valeurs reversées le 2026-06-14.
    /// Ne nécessite AUCUN fichier du jeu — régression pure sur la dérivation.
    #[test]
    fn cpk_list_key_iv_byte_exact() {
        let (key, iv) = cpk_list_key_iv();
        assert_eq!(
            to_hex(&key),
            "99ad1240bac70843e461279d535c86d21f331ebd211bdbb0f7f3fb2b34ea3556"
        );
        assert_eq!(to_hex(&iv), "c1a1e44478f0fbedf0e9828875425566");
    }

    /// Taille non multiple de 16 → erreur propre (pas de panic).
    #[test]
    fn cpk_list_rejects_non_block_aligned() {
        assert!(decrypt_cpk_list(&[0u8; 17]).is_err());
        assert!(decrypt_cpk_list(&[]).is_err());
    }

    /// Bout-en-bout sur le VRAI `cpk_list.cfg.bin` Steam s'il est présent (sinon skip,
    /// comme le test iecode). Déchiffre (AES-256-CBC) → footer T2B `01 74 32 62` présent
    /// → cfgbin_parse → l'entrée racine contient des milliers de fichiers CPK.
    #[test]
    fn cpk_list_decrypts_real_file() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let path = std::path::Path::new(&dir)
            .join("data")
            .join("cpk_list.cfg.bin");
        let Ok(enc) = std::fs::read(&path) else {
            eprintln!(
                "skip cpk_list_decrypts_real_file : {} absent",
                path.display()
            );
            return;
        };
        let clear = decrypt_cpk_list(&enc).expect("déchiffrement AES-256-CBC");

        // Footer T2B dans les 64 derniers octets.
        let tail = &clear[clear.len().saturating_sub(64)..];
        assert!(
            tail.windows(4).any(|w| w == [0x01, 0x74, 0x32, 0x62]),
            "footer T2B 01 74 32 62 absent → déchiffrement faux"
        );

        // En-tête T2B cohérent.
        let entries = i32::from_le_bytes(clear[0..4].try_into().unwrap());
        assert!(
            (1000..1_000_000).contains(&entries),
            "entries_count aberrant : {entries}"
        );

        // Parse réel + nombre d'enfants (fichiers CPK indexés).
        let cfg = crate::cfgbin::cfgbin_parse(&clear).expect("parse T2B cpk_list");
        let children: usize = cfg.entries.iter().map(|e| e.children.len()).sum();
        assert!(
            children > 1000,
            "attendu > 1000 entrées CPK, obtenu {children}"
        );
        eprintln!(
            "cpk_list OK : {} octets clairs, entries_count={entries}, {children} enfants",
            clear.len()
        );
    }
}
