//! `nie-ffi` — Frontière FFI C-ABI.
//!
//! Expose la logique niers à des runtimes extérieurs (Bun, Node, C, Python…) via une interface
//! `extern "C"` stable.
//!
//! # Fonctions exportées — existantes
//!
//! | Symbole C              | Sémantique Rust                                           |
//! |------------------------|-----------------------------------------------------------|
//! | `nie_crc32`            | CRC32 IEEE 802.3 d'un tampon `(ptr, len)`                 |
//! | `nie_crand_new`        | Crée un `CRand` (MT19937) depuis une graine `u32`         |
//! | `nie_crand_from_u64`   | Crée un `CRand` depuis une graine `u64` (repli XOR)      |
//! | `nie_crand_next_u32`   | Tire le prochain `u32`                                    |
//! | `nie_crand_bounded`    | Tire un entier dans `[0, bound)` (Lemire + rejet)         |
//! | `nie_crand_next_f32`   | Tire un `f32` dans `[0.0, 1.0)`                           |
//! | `nie_crand_free`       | Libère un handle `CRand` (exactement une fois)            |
//! | `nie_version`          | Version du crate en C-string statique                     |
//!
//! # Fonctions exportées — décodage formats + VFS
//!
//! | Symbole C                  | Sémantique Rust                                                  |
//! |----------------------------|------------------------------------------------------------------|
//! | `nie_bytes_free`           | Libère un `NieBytes` (exactement une fois, via `Vec::from_raw`) |
//! | `nie_bytes_free_fields`    | Libère par champs (`ptr, len, cap`) — pratique Bun FFI           |
//! | `nie_detect`               | Détecte le format d'un tampon → discriminant `u32` stable        |
//! | `nie_format_name`          | Nom court du format (C-string statique)                          |
//! | `nie_decode_json`          | Auto-détection → JSON UTF-8 ; `NieBytes::empty` si non supporté |
//! | `nie_decode_json_out`      | Idem, via paramètre de sortie `*mut NieBytes` (Bun FFI-friendly)|
//! | `nie_menu_setting_json_out`| `*_menu_setting.cfg.bin` → structure de menu typée       |
//! | `nie_g4tx_to_png`          | Première texture G4TX → octets PNG ; empty sur erreur            |
//! | `nie_g4tx_to_png_out`      | Idem, via `*mut NieBytes`                                        |
//! | `nie_vfs_open`             | Monte le VFS (AES-256-CBC cpk_list) ; null sur erreur            |
//! | `nie_vfs_read`             | Lit un fichier virtuel → octets bruts                            |
//! | `nie_vfs_read_out`         | Idem, via `*mut NieBytes`                                        |
//! | `nie_vfs_list_json`        | JSON `[{path,cpk,size}]` ; plafonné à 50 000 entrées             |
//! | `nie_vfs_list_json_out`    | Idem, via `*mut NieBytes`                                        |
//! | `nie_vfs_count`            | Nombre total d'entrées indexées (sans plafond)                   |
//! | `nie_vfs_is_readable`      | Présence servable d'un chemin, **sans lire le contenu**          |
//! | `nie_match_simulate_json_out` | Simulation de match à graine et à statistiques → JSON        |
//! | `nie_vfs_list_range_json_out` | Tranche `[offset, offset+limit)` de l'index, via `*mut NieBytes` |
//! | `nie_vfs_free`             | Libère le handle VFS                                             |
//!
//! # Fonctions exportées — rendu de texte (police du jeu)
//!
//! | Symbole C                  | Sémantique Rust                                                  |
//! |----------------------------|------------------------------------------------------------------|
//! | `nie_font_open`            | Charge métriques + atlas depuis le VFS → handle `FontCtx`         |
//! | `nie_font_render_text`     | Rend une chaîne UTF-8 avec la police → octets PNG RGBA8          |
//! | `nie_font_render_text_out` | Idem, via `*mut NieBytes` (Bun FFI-friendly)                     |
//! | `nie_font_free`            | Libère le handle `FontCtx`                                       |
//!
//! # Invariants de sécurité pour les appelants
//!
//! - Les handles `*mut CRand` / VFS sont des pointeurs opaques alloués par ce crate.
//! - Chaque handle doit être libéré **exactement une fois** via la fonction `_free` correspondante.
//! - `null` est accepté en entrée par toutes les fonctions `unsafe` (no-op ou retour vide).
//! - `NieBytes` retourné par valeur utilise la convention sret x86-64 : **en Bun FFI, utiliser
//!   les variantes `_out` (paramètre `*mut NieBytes`) pour éviter le sret implicite.**
//!
//! # Utilisation Bun — variantes `_out` recommandées
//!
//! ```js
//! import { dlopen, FFIType, ptr, toArrayBuffer, suffix } from "bun:ffi";
//! // 1. Allouer un slot de sortie de 24 octets (sizeof NieBytes sur x86-64)
//! const outBuf = new Uint8Array(24);
//! // 2. Appeler la variante _out
//! symbols.nie_decode_json_out(inputPtr, BigInt(inputLen), ptr(outBuf));
//! // 3. Lire les champs depuis le slot via DataView
//! const dv  = new DataView(outBuf.buffer);
//! const p   = Number(dv.getBigUint64(0, true));   // ptr
//! const len = Number(dv.getBigUint64(8, true));   // len
//! const cap = dv.getBigUint64(16, true);           // cap (BigInt pour free)
//! // 4. Copier les données avant de libérer
//! const copy = new Uint8Array(toArrayBuffer(p, 0, len));
//! // 5. Libérer l'allocation Rust
//! symbols.nie_bytes_free_fields(BigInt(p), BigInt(len), cap);
//! ```

#![warn(missing_docs)]

use core::ffi::{c_char, c_void};
use nie_core::crand::CRand;

// ─────────────────────────────────────────────────────────────────────────────
// CRC32 (IEEE 802.3, polynôme 0xEDB88320)
// ─────────────────────────────────────────────────────────────────────────────

/// Calcule le CRC32 IEEE 802.3 d'un tampon mémoire `(ptr, len)`.
///
/// Identique à `nie_formats::cfgbin::crc32` : init `0xFFFFFFFF`, poly `0xEDB88320`, XOR final.
///
/// # Safety
///
/// - Si `len > 0`, `ptr` doit pointer vers au moins `len` octets valides et contigus.
/// - Si `len == 0`, `ptr` **n'est pas déréférencé** (retour immédiat `0`) ; `ptr` peut être null.
///
/// # Vecteurs de vérification (IEEE 802.3 / zlib)
///
/// - `crc32(b"Focus",       5) == 0xA301_65ED`
/// - `crc32(b"",            0) == 0x0000_0000`
/// - `crc32(b"123456789",   9) == 0xCBF4_3926`
///
/// # Bun
///
/// ```js
/// // Passer Buffer.from(s, "utf8") directement (zero-copy) et buf.byteLength.
/// const buf = Buffer.from("Focus", "utf8");
/// nie_crc32(buf, buf.byteLength); // => 0xA30165ED
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_crc32(ptr: *const u8, len: usize) -> u32 {
    if len == 0 {
        // Ne pas déréférencer ptr : peut être null ou dangling.
        return nie_formats::cfgbin::crc32(&[]);
    }
    // SAFETY: l'appelant garantit que ptr pointe vers au moins `len` octets valides.
    let data = unsafe { core::slice::from_raw_parts(ptr, len) };
    nie_formats::cfgbin::crc32(data)
}

// ─────────────────────────────────────────────────────────────────────────────
// CRand — handle opaque boxé (Box<CRand>)
// ─────────────────────────────────────────────────────────────────────────────

/// Crée un nouveau `CRand` (MT19937) depuis une graine 32 bits.
///
/// Retourne un handle opaque alloué sur le tas. Le propriétaire doit appeler
/// [`nie_crand_free`] **exactement une fois** pour libérer la mémoire.
///
/// # Propriété
///
/// ```text
/// let h = nie_crand_new(5489);
/// // ... utiliser h ...
/// nie_crand_free(h);  // une seule fois
/// // h est invalide après ce point
/// ```
///
/// # Bun
///
/// ```js
/// const h = symbols.nie_crand_new(5489); // FFIType.u32 -> FFIType.ptr (JS number)
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn nie_crand_new(seed: u32) -> *mut CRand {
    Box::into_raw(Box::new(CRand::new(seed)))
}

/// Crée un nouveau `CRand` depuis une graine 64 bits (repli XOR des deux moitiés sur 32 bits).
///
/// Équivalent à `CRand::from_u64(seed)`. Côté Bun, la graine est un `BigInt` (`FFIType.u64`).
///
/// Même règle de propriété que [`nie_crand_new`] : libérer exactement une fois.
#[unsafe(no_mangle)]
pub extern "C" fn nie_crand_from_u64(seed: u64) -> *mut CRand {
    Box::into_raw(Box::new(CRand::from_u64(seed)))
}

/// Tire le prochain `u32` depuis le PRNG (MT19937 `genrand_int32` avec tempering canonique).
///
/// # Safety
///
/// - `handle` doit être un pointeur valide retourné par [`nie_crand_new`] ou
///   [`nie_crand_from_u64`], non encore libéré.
/// - `null` est accepté : retourne `0` sans déréférencer.
///
/// # Vecteurs (graine 5489)
///
/// ```text
/// new(5489) -> 3_499_211_612, 581_869_302, 3_890_346_734, …
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_crand_next_u32(handle: *mut CRand) -> u32 {
    if handle.is_null() {
        return 0;
    }
    // SAFETY: handle est non-null ; l'appelant garantit qu'il est valide et non libéré.
    unsafe { (*handle).next_u32() }
}

/// Tire un entier dans `[0, bound)` via la méthode de Lemire avec rejet.
///
/// Sémantique byte-exacte du moteur :
/// - `bound == 0` → retourne le tirage **brut** `next_u32()` (pas `0`) ;
/// - sinon → Lemire avec rejet (consommation de tirages identique au moteur nie.exe).
///
/// # Safety
///
/// Mêmes invariants que [`nie_crand_next_u32`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_crand_bounded(handle: *mut CRand, bound: u32) -> u32 {
    if handle.is_null() {
        return 0;
    }
    // SAFETY: handle est non-null ; l'appelant garantit qu'il est valide et non libéré.
    unsafe { (*handle).bounded(bound) }
}

/// Tire un `f32` dans `[0.0, 1.0)` via les 24 bits de poids fort du tirage brut.
///
/// # Safety
///
/// Mêmes invariants que [`nie_crand_next_u32`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_crand_next_f32(handle: *mut CRand) -> f32 {
    if handle.is_null() {
        return 0.0_f32;
    }
    // SAFETY: handle est non-null ; l'appelant garantit qu'il est valide et non libéré.
    unsafe { (*handle).next_f32() }
}

/// Libère un handle `CRand` précédemment retourné par [`nie_crand_new`] ou
/// [`nie_crand_from_u64`].
///
/// # Safety
///
/// - Appeler cette fonction **exactement une fois** par handle.
/// - `null` est un no-op (aucun déréférencement).
/// - Après l'appel, tout usage du pointeur est un comportement indéfini.
///
/// # Recommandation JS / Bun
///
/// Encapsuler dans une classe qui nullifie le pointeur après free et implante
/// `[Symbol.dispose]` (mot-clé `using`) pour garantir un seul appel :
///
/// ```js
/// class CRandHandle {
///   #ptr;
///   constructor(seed) { this.#ptr = symbols.nie_crand_new(seed); }
///   nextU32()  { return symbols.nie_crand_next_u32(this.#ptr); }
///   bounded(n) { return symbols.nie_crand_bounded(this.#ptr, n); }
///   [Symbol.dispose]() {
///     if (this.#ptr) { symbols.nie_crand_free(this.#ptr); this.#ptr = null; }
///   }
/// }
/// // usage : using h = new CRandHandle(5489);
/// ```
///
/// Une `FinalizationRegistry` peut servir de filet de sécurité additionnel.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_crand_free(handle: *mut CRand) {
    if handle.is_null() {
        return;
    }
    // SAFETY: handle est non-null ; l'appelant garantit qu'il provient de nie_crand_new /
    // nie_crand_from_u64 et qu'il n'a pas encore été libéré.
    unsafe { drop(Box::from_raw(handle)) };
}

// ─────────────────────────────────────────────────────────────────────────────
// Méta
// ─────────────────────────────────────────────────────────────────────────────

/// Retourne la version du crate en C-string `'static` ASCII (`CARGO_PKG_VERSION` + `\0`).
///
/// La chaîne est statique : l'appelant **ne doit pas** la libérer.
/// Bun la décode directement en string JS via `FFIType.cstring`.
#[unsafe(no_mangle)]
pub extern "C" fn nie_version() -> *const c_char {
    // SAFETY: la chaîne est ASCII-pur, null-terminée, 'static et non-muable.
    concat!(env!("CARGO_PKG_VERSION"), "\0")
        .as_ptr()
        .cast::<c_char>()
}

// ─────────────────────────────────────────────────────────────────────────────
// NieBytes — tampon d'octets transféré de Rust vers JS
// ─────────────────────────────────────────────────────────────────────────────

/// Tampon d'octets alloué par Rust, passé à JS via l'ABI C.
///
/// JS DOIT appeler [`nie_bytes_free`] (ou [`nie_bytes_free_fields`] en Bun FFI)
/// **exactement une fois** après avoir lu `ptr[0..len]`.
///
/// `NieBytes::empty` (ptr null, len 0, cap 0) signal une erreur ou un résultat vide.
/// `nie_bytes_free` / `nie_bytes_free_fields` acceptent empty comme no-op.
#[repr(C)]
pub struct NieBytes {
    /// Pointeur vers les octets alloués par `Vec::into_raw_parts`.
    pub ptr: *mut u8,
    /// Longueur utile (octets valides).
    pub len: usize,
    /// Capacité interne (requis pour `Vec::from_raw_parts` lors de la libération).
    pub cap: usize,
}

// SAFETY: NieBytes transporte des données immutables après création ;
// le transfert de propriété unique vers JS est géré par convention.
unsafe impl Send for NieBytes {}

impl NieBytes {
    /// Tampon vide (ptr null, len 0, cap 0) — signal d'erreur ou de non-support.
    fn empty() -> Self {
        Self {
            ptr: core::ptr::null_mut(),
            len: 0,
            cap: 0,
        }
    }

    /// Transfère la propriété d'un `Vec<u8>` vers un `NieBytes`.
    /// La mémoire est désormais possédée par JS et doit être libérée via `nie_bytes_free`.
    fn from_vec(v: Vec<u8>) -> Self {
        let mut v = core::mem::ManuallyDrop::new(v);
        Self {
            ptr: v.as_mut_ptr(),
            len: v.len(),
            cap: v.capacity(),
        }
    }
}

/// Libère un [`NieBytes`] alloué par ce crate (via `Vec::from_raw_parts`).
///
/// # Safety
///
/// - Appeler **exactement une fois** par `NieBytes` non vide retourné par ce crate.
/// - `NieBytes::empty` (ptr null) est un no-op.
/// - En Bun FFI, utiliser [`nie_bytes_free_fields`] (évite le sret 24 octets).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_bytes_free(b: NieBytes) {
    if b.ptr.is_null() {
        return;
    }
    // SAFETY: ptr/len/cap proviennent de Vec::into_raw_parts ; appelé exactement une fois.
    unsafe { drop(Vec::from_raw_parts(b.ptr, b.len, b.cap)) };
}

/// Libère un tampon NieBytes en passant ses trois champs séparément.
///
/// Variante Bun-FFI-friendly de [`nie_bytes_free`] qui évite le problème de sret 24 octets :
/// `dlopen` peut déclarer `(ptr: u64, len: u64, cap: u64) → void`.
///
/// # Safety
///
/// - `data_ptr` doit provenir du champ `ptr` d'un `NieBytes` retourné par ce crate.
/// - `len` et `cap` doivent correspondre aux champs `len` et `cap` du même `NieBytes`.
/// - Appeler exactement une fois. `data_ptr == null` est un no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_bytes_free_fields(data_ptr: *mut u8, len: usize, cap: usize) {
    if data_ptr.is_null() {
        return;
    }
    // SAFETY: identique à nie_bytes_free — même contrat de propriété.
    unsafe { drop(Vec::from_raw_parts(data_ptr, len, cap)) };
}

// ─────────────────────────────────────────────────────────────────────────────
// Détection de format
// ─────────────────────────────────────────────────────────────────────────────

/// Discriminant `u32` stable par variante de [`nie_formats::FileFormat`].
/// Ces valeurs ne changent JAMAIS — elles font partie de l'ABI publique.
///
/// | u32 | Format      |
/// |-----|-------------|
/// |   0 | Unknown     |
/// |   1 | CPK         |
/// |   2 | @UTF        |
/// |   3 | CRILAYLA    |
/// |   4 | HCA         |
/// |   5 | ACB         |
/// |   6 | AWB         |
/// |   7 | USM         |
/// |   8 | cfg.bin     |
/// |   9 | G4MG        |
/// |  10 | G4MD        |
/// |  11 | G4TX        |
/// |  12 | G4SK        |
/// |  13 | G4PK/G4PKM  |
/// |  14 | G4NV        |
/// |  15 | LIP         |
const FORMAT_UNKNOWN: u32 = 0;
const FORMAT_LIP: u32 = 15;

fn fileformat_to_u32(f: nie_formats::FileFormat) -> u32 {
    use nie_formats::FileFormat as F;
    match f {
        F::Unknown => 0,
        F::Cpk => 1,
        F::Utf => 2,
        F::CriLayla => 3,
        F::Hca => 4,
        F::Acb => 5,
        F::Awb => 6,
        F::Usm => 7,
        F::CfgBin => 8,
        F::G4mg => 9,
        F::G4md => 10,
        F::G4tx => 11,
        F::G4sk => 12,
        F::G4pk => 13,
        F::G4nv => 14,
    }
}

/// Détecte le format d'un tampon à partir de ses octets magiques.
///
/// Retourne un discriminant `u32` stable (voir la table dans [`nie_bytes_free`] doc).
/// `15` = `lip\0` (non couvert par `nie_formats::detect`) ; `0` = inconnu.
///
/// # Safety
///
/// - Si `len > 0`, `ptr` doit pointer vers au moins `len` octets valides.
/// - `ptr == null` ou `len == 0` → retourne `0` (Unknown).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_detect(ptr: *const u8, len: usize) -> u32 {
    if ptr.is_null() || len == 0 {
        return FORMAT_UNKNOWN;
    }
    // SAFETY: l'appelant garantit ptr..len valides.
    let data = unsafe { core::slice::from_raw_parts(ptr, len) };
    // lip\0 n'est pas dans nie_formats::detect → vérifier en premier.
    if data.len() >= 4 && data[..4] == *b"lip\0" {
        return FORMAT_LIP;
    }
    fileformat_to_u32(nie_formats::detect(data))
}

/// Retourne le nom court du format (C-string statique `'static`).
///
/// La chaîne est statique : l'appelant **ne doit pas** la libérer.
///
/// # Exemples
///
/// ```c
/// printf("%s\n", nie_format_name(11)); // "G4TX"
/// printf("%s\n", nie_format_name(99)); // "?"
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn nie_format_name(kind: u32) -> *const c_char {
    let s: &'static [u8] = match kind {
        0 => b"Unknown\0",
        1 => b"CPK\0",
        2 => b"@UTF\0",
        3 => b"CRILAYLA\0",
        4 => b"HCA\0",
        5 => b"ACB\0",
        6 => b"AWB\0",
        7 => b"USM\0",
        8 => b"cfg.bin\0",
        9 => b"G4MG\0",
        10 => b"G4MD\0",
        11 => b"G4TX\0",
        12 => b"G4SK\0",
        13 => b"G4PK\0",
        14 => b"G4NV\0",
        15 => b"LIP\0",
        _ => b"?\0",
    };
    s.as_ptr().cast()
}

// ─────────────────────────────────────────────────────────────────────────────
// Décodage générique → JSON
// ─────────────────────────────────────────────────────────────────────────────

/// Dispatch interne : détecte le format et sérialise en JSON.
///
/// La table de dispatch vit dans [`nie_formats::decode::to_json`], partagée avec la CLI
/// (`niers decode`) : une famille ajoutée là-bas profite aux deux d'un coup.
fn decode_json_impl(data: &[u8]) -> NieBytes {
    match nie_formats::decode::to_json(data) {
        Some(v) => NieBytes::from_vec(v),
        None => NieBytes::empty(),
    }
}

/// Parse un `*_menu_setting.cfg.bin` T2B vers la structure de menu sémantique de `nie-data`.
///
/// Le résultat JSON contient les douze collections de [`nie_data::menu_setting::MenuSetting`]
/// (`layers`, `resources`, `commands`, groupes et focus), plutôt que l'arbre T2B brut. La
/// fonction est volontairement spécialisée : le nom de fichier n'est pas nécessaire pour
/// décoder et la même ABI sert donc les appels directs et le VFS Bun.
fn menu_setting_json_impl(data: &[u8]) -> NieBytes {
    let parsed = match nie_formats::cfgbin::cfgbin_parse(data) {
        Ok(value) => value,
        Err(_) => return NieBytes::empty(),
    };
    let root = match serde_json::to_value(parsed) {
        Ok(value) => value,
        Err(_) => return NieBytes::empty(),
    };
    let setting = nie_data::menu_setting::parse(&root);
    match serde_json::to_vec(&setting) {
        Ok(value) => NieBytes::from_vec(value),
        Err(_) => NieBytes::empty(),
    }
}

/// Auto-détecte le format d'un tampon et le sérialise en JSON UTF-8.
///
/// Retourne [`NieBytes::empty`] si le format est non supporté ou si le parse échoue.
/// JS doit appeler [`nie_bytes_free`] après avoir consommé le résultat.
///
/// **Note Bun FFI** : cette fonction retourne `NieBytes` par valeur (24 octets, sret x86-64).
/// Préférer [`nie_decode_json_out`] depuis Bun.
///
/// # Safety
///
/// - Si `len > 0`, `ptr` doit pointer vers `len` octets valides.
/// - `ptr == null` ou `len == 0` → retourne `NieBytes::empty`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_decode_json(ptr: *const u8, len: usize) -> NieBytes {
    if ptr.is_null() || len == 0 {
        return NieBytes::empty();
    }
    // SAFETY: l'appelant garantit ptr..len valides.
    let data = unsafe { core::slice::from_raw_parts(ptr, len) };
    decode_json_impl(data)
}

/// Variante Bun-FFI-friendly de [`nie_decode_json`] : écrit le résultat dans `*out`.
///
/// `out` doit pointer vers un `NieBytes` de 24 octets alloué par l'appelant (p.ex. un
/// `Uint8Array(24)` côté JS, dont l'adresse est passée via `ptr(buf)`).
///
/// # Safety
///
/// - `ptr` et `out` ne doivent pas être null et doivent pointer vers des zones valides.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_decode_json_out(ptr: *const u8, len: usize, out: *mut NieBytes) {
    if out.is_null() {
        return;
    }
    let result = if ptr.is_null() || len == 0 {
        NieBytes::empty()
    } else {
        // SAFETY: l'appelant garantit ptr..len valides.
        let data = unsafe { core::slice::from_raw_parts(ptr, len) };
        decode_json_impl(data)
    };
    // SAFETY: out est non-null et aligné (l'appelant alloue 24 octets alignés).
    unsafe { out.write(result) };
}

/// Variante Bun-FFI-friendly du parseur de menu typé.
///
/// `out` doit pointer vers un `NieBytes` de 24 octets alloué par l'appelant. Un pointeur nul,
/// un tampon vide ou un T2B invalide produisent un résultat vide ; aucun détail d'erreur n'est
/// exposé par cette ABI C, conformément aux autres décodeurs `_out`.
///
/// # Safety
///
/// - Si `len > 0`, `ptr` doit pointer vers `len` octets valides et contigus.
/// - `out` doit être non nul, aligné et inscriptible pour un [`NieBytes`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_menu_setting_json_out(ptr: *const u8, len: usize, out: *mut NieBytes) {
    if out.is_null() {
        return;
    }
    let result = if ptr.is_null() || len == 0 {
        NieBytes::empty()
    } else {
        // SAFETY: l'appelant garantit ptr..len valides.
        let data = unsafe { core::slice::from_raw_parts(ptr, len) };
        menu_setting_json_impl(data)
    };
    // SAFETY: out est non-null et aligné (l'appelant alloue 24 octets alignés).
    unsafe { out.write(result) };
}

// ─────────────────────────────────────────────────────────────────────────────
// G4TX → PNG
// ─────────────────────────────────────────────────────────────────────────────

/// Décode la première texture principale d'un fichier G4TX en octets PNG.
///
/// Extraction : `g4tx::parse` → `textures[0]` → payload DDS → `image::load_from_memory_with_format`
/// (BC1/BC2/BC3/BC4/BC5 supportés ; BC7 retourne empty). Encode en PNG via le crate `image`.
///
/// Retourne [`NieBytes::empty`] sur toute erreur (magic invalide, DDS non supporté, etc.).
/// JS doit appeler [`nie_bytes_free`] / [`nie_bytes_free_fields`] après consommation.
///
/// **Note Bun FFI** : préférer [`nie_g4tx_to_png_out`].
///
/// # Safety
///
/// - Si `len > 0`, `ptr` doit pointer vers `len` octets G4TX valides.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_g4tx_to_png(ptr: *const u8, len: usize) -> NieBytes {
    if ptr.is_null() || len == 0 {
        return NieBytes::empty();
    }
    // SAFETY: l'appelant garantit ptr..len valides.
    let data = unsafe { core::slice::from_raw_parts(ptr, len) };
    g4tx_to_png_impl(data)
}

/// Variante Bun-FFI-friendly de [`nie_g4tx_to_png`] : écrit le résultat dans `*out`.
///
/// # Safety
///
/// Mêmes invariants que [`nie_decode_json_out`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_g4tx_to_png_out(ptr: *const u8, len: usize, out: *mut NieBytes) {
    if out.is_null() {
        return;
    }
    let result = if ptr.is_null() || len == 0 {
        NieBytes::empty()
    } else {
        // SAFETY: l'appelant garantit ptr..len valides.
        let data = unsafe { core::slice::from_raw_parts(ptr, len) };
        g4tx_to_png_impl(data)
    };
    // SAFETY: out est non-null et aligné.
    unsafe { out.write(result) };
}

/// Implémentation commune G4TX → PNG.
///
/// Délègue au décodeur partagé `nie_formats::g4tx_decode` (feature `textures`, source unique
/// du workspace — Phase 1b dédup). Bonus vs l'ancienne copie locale (DX10 seul) : support
/// FourCC legacy + non compressé + sélecteur anti-dummy [`g4tx::select_main_texture`].
fn g4tx_to_png_impl(data: &[u8]) -> NieBytes {
    // Basename vide ASSUMÉ : l'ABI C ne reçoit que des octets, jamais le nom du fichier source
    // (cf. `decode_best_to_png`). La sélection retombe sur « la plus grande non-dummy ».
    match nie_formats::g4tx_decode::decode_best_to_png(data, "") {
        Some(png) => NieBytes::from_vec(png),
        None => NieBytes::empty(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// VFS — Virtual File System (CPK/cpk_list)
// ─────────────────────────────────────────────────────────────────────────────

/// Nombre maximum d'entrées dans la liste JSON du VFS (cap mémoire).
///
/// Le VFS IEVR indexe ~254 200 fichiers. Sérialiser la totalité en JSON représente
/// ~40 Mo. Cette constante borne le tableau à 50 000 entrées ; les entrées suivantes
/// sont silencieusement tronquées. Utiliser `nie_vfs_read` pour accéder à n'importe
/// quel fichier par chemin interne.
const VFS_LIST_CAP: usize = 50_000;

/// Monte le VFS IEVR depuis le répertoire contenant `cpk_list.cfg.bin`.
///
/// Le chemin attendu est le répertoire `data/` du jeu (celui qui contient
/// `cpk_list.cfg.bin` et le sous-répertoire `packs/`), PAS le répertoire parent.
///
/// Exemple côté Bun :
/// ```js
/// const gameDataDir = "/home/user/niers/data"; // contient cpk_list.cfg.bin
/// const vfs = nie_vfs_open(gameDataDir);        // null sur erreur
/// ```
///
/// Retourne un handle opaque (`*mut c_void = Box<Vfs>`) ou null sur erreur.
/// Libérer avec [`nie_vfs_free`] exactement une fois.
///
/// # Safety
///
/// - `game_data_dir` doit être une C-string UTF-8 null-terminée valide, ou null.
/// - null → retourne null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_open(game_data_dir: *const c_char) -> *mut c_void {
    if game_data_dir.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: l'appelant garantit une C-string null-terminée valide.
    let path_str = match unsafe { std::ffi::CStr::from_ptr(game_data_dir) }.to_str() {
        Ok(s) => s,
        Err(_) => return core::ptr::null_mut(),
    };
    let mut vfs = Box::new(nie_formats::vfs::Vfs::new());
    if vfs.init(path_str).is_err() {
        return core::ptr::null_mut();
    }
    Box::into_raw(vfs).cast()
}

/// Lit un fichier du VFS par son chemin interne (déchiffrement + décompression CPK inclus).
///
/// Retourne les octets bruts du fichier, ou [`NieBytes::empty`] si le fichier est absent
/// ou si une erreur d'extraction se produit. JS doit appeler [`nie_bytes_free`] après
/// consommation.
///
/// **Note Bun FFI** : préférer [`nie_vfs_read_out`].
///
/// # Safety
///
/// - `vfs` doit être un handle valide retourné par [`nie_vfs_open`], non encore libéré.
/// - `internal_path` doit être une C-string UTF-8 null-terminée valide.
/// - null sur l'un ou l'autre des deux paramètres → retourne empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_read(vfs: *mut c_void, internal_path: *const c_char) -> NieBytes {
    if vfs.is_null() || internal_path.is_null() {
        return NieBytes::empty();
    }
    // SAFETY: vfs provient de nie_vfs_open ; internal_path est null-terminé.
    let vfs_ref = unsafe { &*(vfs.cast::<nie_formats::vfs::Vfs>()) };
    let path = match unsafe { std::ffi::CStr::from_ptr(internal_path) }.to_str() {
        Ok(s) => s,
        Err(_) => return NieBytes::empty(),
    };
    match vfs_ref.read(path) {
        Ok(v) => NieBytes::from_vec(v),
        Err(_) => NieBytes::empty(),
    }
}

/// Variante Bun-FFI-friendly de [`nie_vfs_read`] : écrit le résultat dans `*out`.
///
/// # Safety
///
/// Mêmes invariants que [`nie_vfs_read`], plus `out` non-null et aligné 8 octets.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_read_out(
    vfs: *mut c_void,
    internal_path: *const c_char,
    out: *mut NieBytes,
) {
    if out.is_null() {
        return;
    }
    // SAFETY: délègue à nie_vfs_read qui gère les null.
    let result = unsafe { nie_vfs_read(vfs, internal_path) };
    // SAFETY: out est non-null et aligné.
    unsafe { out.write(result) };
}

/// Retourne un tableau JSON `[{path, cpk, size}]` des fichiers indexés dans le VFS.
///
/// Plafonné à [`VFS_LIST_CAP`] entrées (50 000) pour limiter l'utilisation mémoire.
/// Si le VFS contient plus d'entrées, les suivantes sont silencieusement omises.
/// Appelez `nie_vfs_read("data/relative/path")` pour accéder à tout fichier par nom.
///
/// **Note Bun FFI** : préférer [`nie_vfs_list_json_out`].
///
/// # Safety
///
/// - `vfs` doit être un handle valide retourné par [`nie_vfs_open`], non encore libéré.
/// - null → retourne empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_list_json(vfs: *mut c_void) -> NieBytes {
    if vfs.is_null() {
        return NieBytes::empty();
    }
    // SAFETY: vfs provient de nie_vfs_open.
    let vfs_ref = unsafe { &*(vfs.cast::<nie_formats::vfs::Vfs>()) };
    let entries: Vec<_> = vfs_ref
        .iter()
        .take(VFS_LIST_CAP)
        .map(|(path, e)| {
            serde_json::json!({
                "path": path,
                "cpk":  e.cpk_filename,
                "size": e.file_size,
            })
        })
        .collect();
    match serde_json::to_vec(&entries) {
        Ok(v) => NieBytes::from_vec(v),
        Err(_) => NieBytes::empty(),
    }
}

/// Variante Bun-FFI-friendly de [`nie_vfs_list_json`] : écrit le résultat dans `*out`.
///
/// # Safety
///
/// Mêmes invariants que [`nie_vfs_list_json`], plus `out` non-null et aligné 8 octets.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_list_json_out(vfs: *mut c_void, out: *mut NieBytes) {
    if out.is_null() {
        return;
    }
    // SAFETY: délègue à nie_vfs_list_json.
    let result = unsafe { nie_vfs_list_json(vfs) };
    // SAFETY: out est non-null et aligné.
    unsafe { out.write(result) };
}

/// Nombre total d'entrées indexées dans le VFS — sans plafond, contrairement à
/// [`nie_vfs_list_json`].
///
/// C'est la borne à utiliser pour paginer avec [`nie_vfs_list_range_json_out`].
///
/// # Safety
///
/// - `vfs` doit être un handle valide retourné par [`nie_vfs_open`], non encore libéré.
/// - null → retourne 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_count(vfs: *mut c_void) -> u64 {
    if vfs.is_null() {
        return 0;
    }
    // SAFETY: vfs provient de nie_vfs_open.
    let vfs_ref = unsafe { &*(vfs.cast::<nie_formats::vfs::Vfs>()) };
    vfs_ref.iter().count() as u64
}

/// Dit si un chemin interne est réellement servable, **sans extraire son contenu**.
///
/// Délègue à [`nie_formats::vfs::Vfs::is_readable`], donc tient compte des fichiers
/// « loose » que `cpk_list.cfg.bin` déclare sans qu'ils existent sur une installation
/// donnée : une simple présence dans l'index annoncerait des fichiers que
/// [`nie_vfs_read`] refuserait ensuite.
///
/// C'est le test qu'un consommateur interactif doit employer pour décider s'il peut
/// afficher un asset. L'alternative — appeler [`nie_vfs_read`] et regarder si le
/// tampon est vide — extrait le CPK entier pour répondre par oui ou par non, et
/// remplit le cache de paquets au passage : mesuré à 4,9 Go retenus pour soixante
/// vérifications de textures dispersées.
///
/// # Safety
///
/// - `vfs` doit être un handle valide retourné par [`nie_vfs_open`], non encore libéré.
/// - `internal_path` doit être une chaîne C null-terminée valide.
/// - null (l'un ou l'autre) → retourne 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_is_readable(
    vfs: *mut c_void,
    internal_path: *const c_char,
) -> u32 {
    if vfs.is_null() || internal_path.is_null() {
        return 0;
    }
    // SAFETY: vfs provient de nie_vfs_open ; internal_path est null-terminé.
    let vfs_ref = unsafe { &*(vfs.cast::<nie_formats::vfs::Vfs>()) };
    let path = match unsafe { std::ffi::CStr::from_ptr(internal_path) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    u32::from(vfs_ref.is_readable(path))
}

/// Occupation du cache CPK — miroir de `nie_formats::vfs::CacheCpkStats`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NieCacheStats {
    /// Octets bruts actuellement retenus.
    pub octets: u64,
    /// Nombre de paquets CPK en cache.
    pub entrees: u64,
    /// Budget au-delà duquel l'éviction LRU se déclenche.
    pub budget: u64,
}

/// Écrit l'occupation du cache CPK dans `out`.
///
/// Le VFS garde les octets **bruts** de chaque paquet ouvert : quelques lectures dans des
/// paquets différents suffisent à retenir plusieurs centaines de mégaoctets. Un hôte qui
/// embarque cette bibliothèque — un jeu, par exemple — a besoin de le voir pour décider, et le
/// budget par défaut (16 Gio) est dimensionné pour un traitement par lots, pas pour lui.
///
/// # Safety
///
/// - `vfs` doit provenir de [`nie_vfs_open`] et ne pas avoir été libéré ; null est un no-op.
/// - `out` doit pointer un [`NieCacheStats`] inscriptible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_cache_stats(vfs: *mut c_void, out: *mut NieCacheStats) {
    if vfs.is_null() || out.is_null() {
        return;
    }
    // SAFETY: vfs provient de nie_vfs_open ; out est inscriptible par contrat.
    let vfs_ref = unsafe { &*(vfs.cast::<nie_formats::vfs::Vfs>()) };
    let s = vfs_ref.cache_stats();
    unsafe {
        *out = NieCacheStats {
            octets: s.octets as u64,
            entrees: s.entrees as u64,
            budget: s.budget as u64,
        };
    }
}

/// Vide le cache CPK et rend les octets libérés.
///
/// Sans danger pour les lectures en cours : chacune détient sa donnée par `Arc` jusqu'à la fin
/// de l'extraction. Les lectures suivantes relisent le paquet depuis le disque.
///
/// # Safety
///
/// `vfs` doit provenir de [`nie_vfs_open`] et ne pas avoir été libéré ; null rend 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_cache_vider(vfs: *mut c_void) -> u64 {
    if vfs.is_null() {
        return 0;
    }
    // SAFETY: vfs provient de nie_vfs_open.
    let vfs_ref = unsafe { &*(vfs.cast::<nie_formats::vfs::Vfs>()) };
    vfs_ref.vider_cache() as u64
}

/// Change le budget du cache CPK et évince immédiatement ce qui dépasse.
///
/// Rend les octets libérés par l'éviction déclenchée. L'éviction garde toujours un paquet :
/// évincer celui qu'on vient de demander ferait relire le disque au prochain appel, en boucle.
///
/// # Safety
///
/// `vfs` doit provenir de [`nie_vfs_open`] et ne pas avoir été libéré ; null rend 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_cache_budget(vfs: *mut c_void, budget: u64) -> u64 {
    if vfs.is_null() {
        return 0;
    }
    // SAFETY: vfs provient de nie_vfs_open.
    let vfs_ref = unsafe { &*(vfs.cast::<nie_formats::vfs::Vfs>()) };
    vfs_ref.regler_budget_cache(usize::try_from(budget).unwrap_or(usize::MAX)) as u64
}

/// Tranche `[offset, offset + limit)` de l'index VFS, en JSON `[{path, cpk, size}]`.
///
/// Complète [`nie_vfs_list_json`], dont le plafond de 50 000 entrées tronque **en silence**
/// un VFS qui en compte ~255 000 : paginer avec cette fonction permet d'énumérer la totalité
/// de l'index sans jamais matérialiser plus d'une page en mémoire. L'ordre d'itération est
/// celui de [`nie_formats::vfs::Vfs::iter`] : stable pour un même handle, non trié.
///
/// `limit == 0` renvoie un tableau vide ; un `offset` au-delà de la fin aussi.
///
/// # Safety
///
/// - `vfs` doit être un handle valide retourné par [`nie_vfs_open`], non encore libéré.
/// - `out` non-null et aligné 8 octets.
/// - null → écrit un `NieBytes` vide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_list_range_json_out(
    vfs: *mut c_void,
    offset: u64,
    limit: u64,
    out: *mut NieBytes,
) {
    if out.is_null() {
        return;
    }
    if vfs.is_null() {
        // SAFETY: out est non-null et aligné.
        unsafe { out.write(NieBytes::empty()) };
        return;
    }
    // SAFETY: vfs provient de nie_vfs_open.
    let vfs_ref = unsafe { &*(vfs.cast::<nie_formats::vfs::Vfs>()) };
    let entries: Vec<_> = vfs_ref
        .iter()
        .skip(usize::try_from(offset).unwrap_or(usize::MAX))
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .map(|(path, e)| {
            serde_json::json!({
                "path": path,
                "cpk":  e.cpk_filename,
                "size": e.file_size,
            })
        })
        .collect();
    let result = match serde_json::to_vec(&entries) {
        Ok(v) => NieBytes::from_vec(v),
        Err(_) => NieBytes::empty(),
    };
    // SAFETY: out est non-null et aligné.
    unsafe { out.write(result) };
}

/// Libère un handle VFS retourné par [`nie_vfs_open`].
///
/// # Safety
///
/// - Appeler **exactement une fois** par handle non-null.
/// - null → no-op.
/// - Après l'appel, tout usage du handle est un comportement indéfini.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_vfs_free(vfs: *mut c_void) {
    if vfs.is_null() {
        return;
    }
    // SAFETY: vfs provient de nie_vfs_open → Box<Vfs> ; appelé exactement une fois.
    unsafe { drop(Box::from_raw(vfs.cast::<nie_formats::vfs::Vfs>())) };
}

// ─────────────────────────────────────────────────────────────────────────────
// Police — rendu de texte (couche 4) : métriques font.cfg.bin + atlas font.g4tx
// ─────────────────────────────────────────────────────────────────────────────

/// Chemin VFS des métriques de glyphes (T2B).
const FONT_METRICS_PATH: &str = "data/common/font/font/font_def/font.cfg.bin";
/// Chemin VFS de l'atlas de police (DDS BGRA8 non compressé, 4096×2048).
const FONT_ATLAS_PATH: &str = "data/dx11/font/font_def/font.g4tx";
/// Décalage des pixels mip0 dans le payload DDS (magic 4 + DDS_HEADER 124, sans ext DX10).
const FONT_DDS_PIXEL_OFFSET: usize = 128;

/// Contexte de police chargé : métriques + atlas BGRA8, prêt pour le rendu répété.
///
/// Construit une fois par [`nie_font_open`] pour éviter de relire/reparser l'atlas de
/// ~33 Mo à chaque rendu.
struct FontCtx {
    metrics: nie_formats::font::FontMetrics,
    atlas: Vec<u8>,
    atlas_w: u32,
}

/// Construit un [`FontCtx`] depuis un VFS monté (lecture + parse métriques et atlas).
fn build_font_ctx(vfs: &nie_formats::vfs::Vfs) -> Option<FontCtx> {
    let cfg_bytes = vfs.read(FONT_METRICS_PATH).ok()?;
    let cfg = nie_formats::cfgbin::parse_t2b(&cfg_bytes).ok()?;
    let metrics = nie_formats::font::parse_metrics(&cfg);
    if metrics.dims.cell_height == 0 {
        return None;
    }
    let g4tx_bytes = vfs.read(FONT_ATLAS_PATH).ok()?;
    let g4tx = nie_formats::g4tx::parse(&g4tx_bytes).ok()?;
    let tex = g4tx.textures.first()?;
    if !tex.is_dds || tex.width <= 0 {
        return None;
    }
    let start = tex.data_offset + FONT_DDS_PIXEL_OFFSET;
    let atlas = g4tx_bytes.get(start..)?.to_vec();
    Some(FontCtx {
        metrics,
        atlas,
        atlas_w: tex.width as u32,
    })
}

/// Implémentation commune : rend `text` sur un canevas RGBA8 ajusté puis encode en PNG.
fn render_text_impl(ctx: &FontCtx, text: &str, color: [u8; 4]) -> NieBytes {
    use std::io::Cursor;
    let ascent = i32::from(ctx.metrics.dims.ascent);
    let cell_h = ctx.metrics.dims.cell_height as usize;
    if cell_h == 0 {
        return NieBytes::empty();
    }
    // Mesure : largeur = max(avance cumulée, extrémité droite du dernier glyphe tracé).
    let mut pen = 0i32;
    let mut max_x = 0i32;
    for c in text.chars() {
        if let Some(m) = ctx.metrics.glyph(c as u32) {
            max_x = max_x.max(pen + i32::from(m.bearing_x) + i32::from(m.width));
            pen += i32::from(m.advance);
        }
    }
    let width = max_x.max(pen).max(1) as usize;
    let mut canvas = vec![0u8; width * cell_h * 4];
    nie_formats::font::draw_text(
        &ctx.atlas,
        ctx.atlas_w,
        &ctx.metrics,
        text,
        &mut canvas,
        (width * 4) as u32,
        0,
        ascent,
        color,
    );
    let img = match image::RgbaImage::from_raw(width as u32, cell_h as u32, canvas) {
        Some(i) => i,
        None => return NieBytes::empty(),
    };
    let mut png: Vec<u8> = Vec::new();
    if img
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .is_err()
    {
        return NieBytes::empty();
    }
    NieBytes::from_vec(png)
}

/// Ouvre un contexte de police depuis un VFS monté (charge + parse métriques et atlas).
///
/// Lit `font.cfg.bin` (métriques T2B) et `font.g4tx` (atlas DDS BGRA8) du VFS, les parse
/// une fois, et retourne un handle réutilisable pour [`nie_font_render_text`]. Évite de
/// relire/reparser l'atlas de ~33 Mo à chaque rendu.
///
/// Retourne null si `vfs` est null, si un fichier de police est absent, ou si le parse
/// échoue. Libérer avec [`nie_font_free`] **exactement une fois**.
///
/// # Safety
///
/// - `vfs` doit être un handle valide retourné par [`nie_vfs_open`], non encore libéré.
/// - null → retourne null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_font_open(vfs: *mut c_void) -> *mut c_void {
    if vfs.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: vfs provient de nie_vfs_open.
    let vfs_ref = unsafe { &*(vfs.cast::<nie_formats::vfs::Vfs>()) };
    match build_font_ctx(vfs_ref) {
        Some(ctx) => Box::into_raw(Box::new(ctx)).cast(),
        None => core::ptr::null_mut(),
    }
}

/// Rend une chaîne UTF-8 avec la police du jeu → octets PNG RGBA8 (fond transparent).
///
/// La couleur de teinte `(r, g, b, a)` module le masque alpha des glyphes (blanc opaque =
/// `255, 255, 255, 255`). Le canevas a la hauteur d'une cellule (`cell_height`, 71 px pour
/// la police principale) et la largeur de l'avance totale du texte. Les points de code
/// absents de la police principale sont ignorés.
///
/// Retourne [`NieBytes::empty`] si `ctx`/`text` est null, si `text` n'est pas de l'UTF-8
/// valide, ou en cas d'erreur d'encodage. JS doit appeler [`nie_bytes_free`] /
/// [`nie_bytes_free_fields`] après consommation.
///
/// **Note Bun FFI** : préférer [`nie_font_render_text_out`] (évite le sret 24 octets).
///
/// # Safety
///
/// - `ctx` doit être un handle valide retourné par [`nie_font_open`], non encore libéré.
/// - `text` doit être une C-string UTF-8 null-terminée valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_font_render_text(
    ctx: *mut c_void,
    text: *const c_char,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) -> NieBytes {
    if ctx.is_null() || text.is_null() {
        return NieBytes::empty();
    }
    // SAFETY: ctx provient de nie_font_open ; text est null-terminé.
    let ctx_ref = unsafe { &*(ctx.cast::<FontCtx>()) };
    let s = match unsafe { std::ffi::CStr::from_ptr(text) }.to_str() {
        Ok(s) => s,
        Err(_) => return NieBytes::empty(),
    };
    render_text_impl(ctx_ref, s, [r, g, b, a])
}

/// Variante Bun-FFI-friendly de [`nie_font_render_text`] : écrit le résultat dans `*out`.
///
/// # Safety
///
/// Mêmes invariants que [`nie_font_render_text`], plus `out` non-null et aligné 8 octets.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_font_render_text_out(
    ctx: *mut c_void,
    text: *const c_char,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    out: *mut NieBytes,
) {
    if out.is_null() {
        return;
    }
    // SAFETY: délègue à nie_font_render_text qui gère les null.
    let result = unsafe { nie_font_render_text(ctx, text, r, g, b, a) };
    // SAFETY: out est non-null et aligné.
    unsafe { out.write(result) };
}

/// Libère un contexte de police retourné par [`nie_font_open`].
///
/// # Safety
///
/// - Appeler **exactement une fois** par handle non-null.
/// - null → no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_font_free(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    // SAFETY: ctx provient de nie_font_open → Box<FontCtx> ; appelé exactement une fois.
    unsafe { drop(Box::from_raw(ctx.cast::<FontCtx>())) };
}

// ─────────────────────────────────────────────────────────────────────────────
// Moteur de match (`nie-runtime`) — la simulation, pilotable depuis l'extérieur
// ─────────────────────────────────────────────────────────────────────────────
//
// Tout ce qui précède LIT le jeu (assets, formats, VFS). Ce bloc le fait TOURNER : il expose
// `nie_runtime::World`, la simulation 11 v 11 déterministe, pour qu'un hôte non-Rust — la lib
// Python `niepy`, et donc Ren'Py — puisse avancer un match tick par tick et lire son état.
//
// Le déterminisme est le contrat : à `dt` et entrées identiques, deux exécutions donnent la
// même suite d'états. C'est ce qui permet à un visual novel de rejouer une action à l'identique.

/// Handle opaque de monde de match, alloué par [`nie_world_new`].
///
/// Le type réel est `nie_runtime::World` ; l'hôte ne le manipule que par ce pointeur.
pub struct NieWorld {
    _priv: [u8; 0],
}

/// Un joueur, aplati en `repr(C)` pour la frontière.
///
/// Les positions sont en mètres, origine au centre du terrain
/// (demi-longueur [`nie_runtime::HALF_LEN`], demi-largeur [`nie_runtime::HALF_WID`]).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NiePlayer {
    /// Position au sol, axe but-à-but.
    pub x: f32,
    /// Position au sol, axe touche-à-touche.
    pub y: f32,
    /// Vitesse au sol, composante `x` (m/s).
    pub vx: f32,
    /// Vitesse au sol, composante `y` (m/s).
    pub vy: f32,
    /// Équipe : `0` = domicile (attaque vers `+x`), `1` = extérieur.
    pub team: u8,
    /// Rôle : `0` gardien, `1` défenseur, `2` milieu, `3` attaquant.
    pub role: u8,
}

/// Le ballon, aplati en `repr(C)`. `z` est la **hauteur** (convention `nie-runtime`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NieBall {
    /// Position, axe but-à-but.
    pub x: f32,
    /// Position, axe touche-à-touche.
    pub y: f32,
    /// Hauteur au-dessus du sol.
    pub z: f32,
    /// Vitesse, composante `x` (m/s).
    pub vx: f32,
    /// Vitesse, composante `y` (m/s).
    pub vy: f32,
    /// Vitesse verticale (m/s).
    pub vz: f32,
}

/// Crée un monde de match au coup d'envoi (22 joueurs + ballon, un gardien par camp).
///
/// Rend un handle à libérer **exactement une fois** par [`nie_world_free`].
#[unsafe(no_mangle)]
pub extern "C" fn nie_world_new() -> *mut NieWorld {
    let w = Box::new(nie_runtime::World::kickoff());
    Box::into_raw(w).cast::<NieWorld>()
}

/// Remet le monde au coup d'envoi, sans réallouer le handle.
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] et ne pas avoir été libéré.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_kickoff(w: *mut NieWorld) {
    if w.is_null() {
        return;
    }
    // SAFETY: w provient de nie_world_new → Box<World>, encore vivant par contrat.
    let world = unsafe { &mut *w.cast::<nie_runtime::World>() };
    *world = nie_runtime::World::kickoff();
}

/// Avance la simulation de `dt` secondes.
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] et ne pas avoir été libéré.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_step(w: *mut NieWorld, dt: f32) {
    if w.is_null() {
        return;
    }
    // SAFETY: idem nie_world_kickoff.
    let world = unsafe { &mut *w.cast::<nie_runtime::World>() };
    world.step(dt);
}

/// Pose l'entrée du joueur contrôlé : direction souhaitée et ordre de frappe.
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] et ne pas avoir été libéré.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_set_input(w: *mut NieWorld, dx: f32, dy: f32, shoot: bool) {
    if w.is_null() {
        return;
    }
    // SAFETY: idem nie_world_kickoff.
    let world = unsafe { &mut *w.cast::<nie_runtime::World>() };
    // Affectation champ par champ : évite de nommer `nie_geom::Vec2` ici, donc évite d'ajouter
    // une dépendance à ce crate juste pour construire deux flottants.
    world.input.dir.x = dx;
    world.input.dir.y = dy;
    world.input.shoot = shoot;
}

/// Nombre de joueurs sur le terrain (22 au coup d'envoi). `0` si `w` est null.
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] et ne pas avoir été libéré.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_player_count(w: *const NieWorld) -> usize {
    if w.is_null() {
        return 0;
    }
    // SAFETY: w provient de nie_world_new ; lecture partagée.
    let world = unsafe { &*w.cast::<nie_runtime::World>() };
    world.players.len()
}

/// Copie le joueur d'indice `i` dans `out`. Rend `false` si `i` est hors bornes.
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] ; `out` doit pointer un [`NiePlayer`] inscriptible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_player(
    w: *const NieWorld,
    i: usize,
    out: *mut NiePlayer,
) -> bool {
    if w.is_null() || out.is_null() {
        return false;
    }
    // SAFETY: w provient de nie_world_new ; lecture partagée.
    let world = unsafe { &*w.cast::<nie_runtime::World>() };
    let Some(p) = world.players.get(i) else {
        return false;
    };
    let role = match p.role {
        nie_runtime::Role::Goalkeeper => 0,
        nie_runtime::Role::Defender => 1,
        nie_runtime::Role::Midfielder => 2,
        nie_runtime::Role::Forward => 3,
    };
    // SAFETY: out est un NiePlayer inscriptible par contrat.
    unsafe {
        *out = NiePlayer {
            x: p.pos.x,
            y: p.pos.y,
            vx: p.vel.x,
            vy: p.vel.y,
            team: p.team,
            role,
        };
    }
    true
}

/// Copie l'état du ballon dans `out`.
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] ; `out` doit pointer un [`NieBall`] inscriptible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_ball(w: *const NieWorld, out: *mut NieBall) {
    if w.is_null() || out.is_null() {
        return;
    }
    // SAFETY: w provient de nie_world_new ; lecture partagée.
    let world = unsafe { &*w.cast::<nie_runtime::World>() };
    let b = &world.ball;
    // SAFETY: out est un NieBall inscriptible par contrat.
    unsafe {
        *out = NieBall {
            x: b.pos.x,
            y: b.pos.y,
            z: b.pos.z,
            vx: b.vel.x,
            vy: b.vel.y,
            vz: b.vel.z,
        };
    }
}

/// Écrit le score dans `home` et `away` (chacun optionnel — un pointeur null est ignoré).
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] ; `home`/`away` sont null ou inscriptibles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_score(w: *const NieWorld, home: *mut u32, away: *mut u32) {
    if w.is_null() {
        return;
    }
    // SAFETY: w provient de nie_world_new ; lecture partagée.
    let world = unsafe { &*w.cast::<nie_runtime::World>() };
    if !home.is_null() {
        // SAFETY: non-null et inscriptible par contrat.
        unsafe { *home = world.score[0] };
    }
    if !away.is_null() {
        // SAFETY: non-null et inscriptible par contrat.
        unsafe { *away = world.score[1] };
    }
}

/// Temps de jeu écoulé, en secondes. `0.0` si `w` est null.
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] et ne pas avoir été libéré.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_time(w: *const NieWorld) -> f32 {
    if w.is_null() {
        return 0.0;
    }
    // SAFETY: w provient de nie_world_new ; lecture partagée.
    unsafe { &*w.cast::<nie_runtime::World>() }.time
}

/// Numéro du tick courant — le compteur qui atteste du déterminisme. `0` si `w` est null.
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] et ne pas avoir été libéré.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_tick(w: *const NieWorld) -> u64 {
    if w.is_null() {
        return 0;
    }
    // SAFETY: w provient de nie_world_new ; lecture partagée.
    unsafe { &*w.cast::<nie_runtime::World>() }.tick
}

/// Indice du joueur qui possède le ballon, ou `-1` si personne ne l'a.
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] et ne pas avoir été libéré.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_possessor(w: *const NieWorld) -> isize {
    if w.is_null() {
        return -1;
    }
    // SAFETY: w provient de nie_world_new ; lecture partagée.
    let world = unsafe { &*w.cast::<nie_runtime::World>() };
    world.possessor().map_or(-1, |i| i as isize)
}

/// État complet du monde en JSON UTF-8 — la voie pratique pour un hôte scripté.
///
/// Forme : `{"tick","time","score":[h,a],"possessor","ball":{…},"players":[{…}]}`.
/// L'hôte DOIT libérer le tampon par [`nie_bytes_free`] / [`nie_bytes_free_fields`].
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] ; `out` doit pointer un [`NieBytes`] inscriptible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_snapshot_json_out(w: *const NieWorld, out: *mut NieBytes) {
    if out.is_null() {
        return;
    }
    if w.is_null() {
        // SAFETY: out non-null et inscriptible par contrat.
        unsafe { *out = NieBytes::empty() };
        return;
    }
    // SAFETY: w provient de nie_world_new ; lecture partagée.
    let world = unsafe { &*w.cast::<nie_runtime::World>() };
    let players: Vec<serde_json::Value> = world
        .players
        .iter()
        .map(|p| {
            serde_json::json!({
                "x": p.pos.x, "y": p.pos.y,
                "vx": p.vel.x, "vy": p.vel.y,
                "team": p.team,
                "role": match p.role {
                    nie_runtime::Role::Goalkeeper => "GK",
                    nie_runtime::Role::Defender => "DF",
                    nie_runtime::Role::Midfielder => "MF",
                    nie_runtime::Role::Forward => "FW",
                },
            })
        })
        .collect();
    let snap = serde_json::json!({
        "tick": world.tick,
        "time": world.time,
        "score": [world.score[0], world.score[1]],
        "possessor": world.possessor().map_or(-1, |i| i as isize),
        "ball": {
            "x": world.ball.pos.x, "y": world.ball.pos.y, "z": world.ball.pos.z,
            "vx": world.ball.vel.x, "vy": world.ball.vel.y, "vz": world.ball.vel.z,
        },
        "players": players,
    });
    let bytes = serde_json::to_vec(&snap).unwrap_or_default();
    // SAFETY: out non-null et inscriptible par contrat.
    unsafe { *out = NieBytes::from_vec(bytes) };
}

/// Libère un handle de monde.
///
/// # Safety
///
/// `w` doit provenir de [`nie_world_new`] et n'être libéré **qu'une fois**. Null est un no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_world_free(w: *mut NieWorld) {
    if w.is_null() {
        return;
    }
    // SAFETY: w provient de nie_world_new → Box<World> ; appelé exactement une fois.
    unsafe { drop(Box::from_raw(w.cast::<nie_runtime::World>())) };
}

// ============================================================================
// Simulation de match — à graine et à statistiques
// ============================================================================

/// Simule une rencontre complète et rend le résultat en JSON UTF-8.
///
/// C'est le complément du bloc `nie_world_*`, et il répond à un autre besoin.
/// `nie_world_*` fait TOURNER un match jouable : on lui pousse des entrées et on
/// l'avance pas à pas. Il n'accepte aucune graine et son onze est le sien — deux
/// matchs laissés à eux-mêmes rendent le même score, et rien ne distingue les
/// joueurs d'un camp de ceux de l'autre.
///
/// [`nie_core::match_sim::simulate_match`] fait l'inverse : il tranche une
/// rencontre d'un bloc, à partir des STATISTIQUES des deux équipes et d'une
/// GRAINE. C'est ce qu'un récit demande — un match que le joueur ne dispute pas,
/// dont le résultat dépend de la force des effectifs et se rejoue à l'identique.
///
/// `home_json` et `away_json` sont des `TeamSetup` sérialisés :
///
/// ```json
/// {"name": "Raimon", "aggregate_stats": {"kc":207,"cr":216,"tc":218,
///  "pr":235,"ps":242,"ag":210,"it":261}, "placements": null}
/// ```
///
/// La sortie est un `MatchResult` : `home_score`, `away_score`, `final_clock`
/// (`minutes * 10_000 + secondes`) et la séquence complète d'`events`.
///
/// `NieBytes::empty` en sortie signale une entrée illisible — JSON invalide,
/// champ manquant, ou pointeur nul.
///
/// # Safety
///
/// - `home_json` et `away_json` doivent être des chaînes C null-terminées valides.
/// - `out` doit pointer sur un `NieBytes` inscriptible et aligné.
/// - null (n'importe lequel) → `NieBytes::empty`, aucun effet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_match_simulate_json_out(
    home_json: *const c_char,
    away_json: *const c_char,
    seed: u64,
    out: *mut NieBytes,
) {
    if out.is_null() {
        return;
    }
    if home_json.is_null() || away_json.is_null() {
        // SAFETY: out est non-null et aligné.
        unsafe { out.write(NieBytes::empty()) };
        return;
    }

    // SAFETY: les deux pointeurs sont non-null et null-terminés.
    let (home_str, away_str) = unsafe {
        (
            std::ffi::CStr::from_ptr(home_json).to_str(),
            std::ffi::CStr::from_ptr(away_json).to_str(),
        )
    };

    let resultat = (|| -> Option<Vec<u8>> {
        let home: nie_core::match_sim::TeamSetup = serde_json::from_str(home_str.ok()?).ok()?;
        let away: nie_core::match_sim::TeamSetup = serde_json::from_str(away_str.ok()?).ok()?;
        let issue = nie_core::match_sim::simulate_match(home, away, seed);
        serde_json::to_vec(&issue).ok()
    })();

    let sortie = match resultat {
        Some(v) => NieBytes::from_vec(v),
        None => NieBytes::empty(),
    };
    // SAFETY: out est non-null et aligné.
    unsafe { out.write(sortie) };
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    // ── CRC32 ────────────────────────────────────────────────────────────────

    #[test]
    fn crc32_ground_truth_focus() {
        let buf = b"Focus";
        let hash = unsafe { nie_crc32(buf.as_ptr(), buf.len()) };
        assert_eq!(hash, 0xA301_65ED, "crc32(b\"Focus\")");
    }

    #[test]
    fn crc32_ground_truth_123456789() {
        let buf = b"123456789";
        let hash = unsafe { nie_crc32(buf.as_ptr(), buf.len()) };
        assert_eq!(hash, 0xCBF4_3926, "crc32(b\"123456789\")");
    }

    #[test]
    fn crc32_empty_len_zero_returns_zero() {
        // len==0 : ptr n'est pas déréférencé, retour 0.
        let hash = unsafe { nie_crc32(core::ptr::null(), 0) };
        assert_eq!(hash, 0, "crc32(null, 0)");
    }

    #[test]
    fn crc32_empty_slice_ptr_returns_zero() {
        // Slice vide via ptr valide (même résultat).
        let hash = unsafe { nie_crc32(b"".as_ptr(), 0) };
        assert_eq!(hash, 0, "crc32(b\"\", 0)");
    }

    // ── CRand ────────────────────────────────────────────────────────────────

    #[test]
    fn crand_new_vecteur_mt19937_seed_5489() {
        let h = nie_crand_new(5489);
        assert_eq!(unsafe { nie_crand_next_u32(h) }, 3_499_211_612, "tirage #1");
        assert_eq!(unsafe { nie_crand_next_u32(h) }, 581_869_302, "tirage #2");
        assert_eq!(unsafe { nie_crand_next_u32(h) }, 3_890_346_734, "tirage #3");
        unsafe { nie_crand_free(h) };
    }

    #[test]
    fn crand_null_handle_safe() {
        let null: *mut CRand = core::ptr::null_mut();
        assert_eq!(unsafe { nie_crand_next_u32(null) }, 0);
        assert_eq!(unsafe { nie_crand_bounded(null, 10) }, 0);
        assert_eq!(unsafe { nie_crand_next_f32(null) }, 0.0_f32);
        // free null = no-op
        unsafe { nie_crand_free(null) };
    }

    #[test]
    fn crand_from_u64_matches_u32_low_half() {
        // from_u64(n) avec moitié haute nulle == new(n as u32)
        let h64 = nie_crand_from_u64(12_345_u64);
        let h32 = nie_crand_new(12_345_u32);
        assert_eq!(unsafe { nie_crand_next_u32(h64) }, unsafe {
            nie_crand_next_u32(h32)
        });
        unsafe { nie_crand_free(h64) };
        unsafe { nie_crand_free(h32) };
    }

    #[test]
    fn crand_bounded_zero_is_raw_next_u32() {
        // bound==0 → tirage brut (pas 0) et consomme un tirage.
        let h = nie_crand_new(99);
        let h2 = nie_crand_new(99);
        let via_bounded = unsafe { nie_crand_bounded(h, 0) };
        let via_next = unsafe { nie_crand_next_u32(h2) };
        assert_eq!(via_bounded, via_next, "bound==0 → tirage brut");
        // Les deux PRNG restent synchronisés après.
        assert_eq!(
            unsafe { nie_crand_next_u32(h) },
            unsafe { nie_crand_next_u32(h2) },
            "consommation identique"
        );
        unsafe { nie_crand_free(h) };
        unsafe { nie_crand_free(h2) };
    }

    #[test]
    fn crand_bounded_nonzero_in_range() {
        let h = nie_crand_new(42);
        for _ in 0..10_000 {
            assert!(unsafe { nie_crand_bounded(h, 6) } < 6);
        }
        unsafe { nie_crand_free(h) };
    }

    #[test]
    fn crand_next_f32_in_range() {
        let h = nie_crand_new(7);
        for _ in 0..10_000 {
            let f = unsafe { nie_crand_next_f32(h) };
            assert!((0.0_f32..1.0_f32).contains(&f));
        }
        unsafe { nie_crand_free(h) };
    }

    // ── Version ──────────────────────────────────────────────────────────────

    #[test]
    fn version_is_static_cstring() {
        let ptr = nie_version();
        // SAFETY: nie_version retourne une C-string 'static null-terminée ASCII.
        let s = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        // La version vient du workspace (`version.workspace = true`) : la comparer à une
        // constante en dur faisait échouer le test à chaque bump (0.1.0 → 0.4.0).
        assert_eq!(s, env!("CARGO_PKG_VERSION"));
    }

    // ── Police : rendu de texte (gated sur le vrai jeu) ────────────────────────

    /// Round-trip complet : VFS → `nie_font_open` → `nie_font_render_text("A")` → PNG.
    /// Valide la dimension (avance 39 × cell_height 71) et le pixel alpha connu (251),
    /// décalé de `bearing_x=1` par `draw_text`. Skip si le jeu est absent.
    #[test]
    fn font_render_text_a_real() {
        use std::ffi::CString;
        let dir = nie_formats::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data) {
            eprintln!("skip font_render_text_a_real : jeu absent");
            return;
        }
        let data_c = CString::new(data.to_str().unwrap()).unwrap();
        // SAFETY: data_c est une C-string valide.
        let vfs = unsafe { nie_vfs_open(data_c.as_ptr()) };
        assert!(!vfs.is_null(), "vfs open");
        // SAFETY: vfs provient de nie_vfs_open.
        let fctx = unsafe { nie_font_open(vfs) };
        assert!(!fctx.is_null(), "font open");

        let text = CString::new("A").unwrap();
        // SAFETY: fctx/text valides.
        let png = unsafe { nie_font_render_text(fctx, text.as_ptr(), 255, 255, 255, 255) };
        assert!(!png.ptr.is_null() && png.len > 8, "PNG non vide");

        // SAFETY: png.ptr..len est un tampon PNG valide alloué par ce crate.
        let bytes = unsafe { core::slice::from_raw_parts(png.ptr, png.len) };
        let img = image::load_from_memory(bytes)
            .expect("décodage PNG")
            .to_rgba8();
        assert_eq!(img.width(), 39, "largeur = avance de 'A'");
        assert_eq!(img.height(), 71, "hauteur = cell_height");
        // 'A' tracé à dst_x = bearing_x = 1 ; le pixel atlas-relatif (row=20, col=0, alpha=251)
        // atterrit donc en (x=1, y=20).
        assert_eq!(
            img.get_pixel(1, 20)[3],
            251,
            "alpha du glyphe A décalé de bearing_x=1"
        );

        // SAFETY: chaque handle/tampon est libéré exactement une fois.
        unsafe { nie_bytes_free(png) };
        unsafe { nie_font_free(fctx) };
        unsafe { nie_vfs_free(vfs) };
    }
}
