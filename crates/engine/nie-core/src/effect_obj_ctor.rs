//! Constructeur de `lives::CEffectObjComponent` — port **byte-exact** de `FUN_140071d70`
//! (@`0x140071d70`, `nie_eacpatched.exe`, 296 o). Initialise un objet composant d'effet de
//! 0xD0 (208) octets : zero-fill par régions + valeurs par défaut typées + pointeur de vtable
//! au `+0x00`, et incrémente le compteur global d'instances `_DAT_141c3b144`.
//!
//! ## Disposition (reversée + validée uemu, 600 cas byte-exact)
//!
//! La fonction écrit un ensemble **précis** d'offsets ; six octets restent **intacts**
//! (`0x7a, 0x7b, 0xae, 0xaf, 0xce, 0xcf` — interstices entre deux écritures), preuve que le
//! port reproduit exactement les largeurs/offsets et non un memset global. Valeurs par défaut :
//! - `+0x00` : pointeur de vtable `lives::CEffectObjComponent::vftable` ([`VTABLE`]) ;
//! - `+0x54` : `1.0f` (`0x3F800000`) ; `+0x5c` : `1` (u32) ;
//! - `+0xa8` : `10.0f` (`0x41200000`) ; `+0xb8` : `30.0f` (`0x41F00000`) ;
//! - `+0xc4`/`+0xc8` : `0x10000` (u32) ; `+0xcc` : `0x100` (u16) ;
//! - tout le reste des octets écrits : `0`.
//!
//! Le pointeur de vtable est une **adresse réelle de l'image** (invariant de ce binaire),
//! donc reproduit tel quel. Le compteur d'instances global est un état externe : il est modélisé
//! en paramètre/retour (`instance_count → instance_count + 1`).
//!
//! Validation : `scripts/validate_effect_obj_ctor.py` (oracle uemu, struct 0xD0 + compteur +
//! voisins, entrée fuzzée pour prouver l'ensemble exact écrit/préservé).

/// Taille de la structure `CEffectObjComponent`, en octets.
pub const STRUCT_SIZE: usize = 0xD0;

/// Adresse de `lives::CEffectObjComponent::vftable` dans l'image (`lea` final du ctor).
pub const VTABLE: u64 = 0x1_417B_8F50;

const F1_0: u32 = 0x3F80_0000; // 1.0f
const F10_0: u32 = 0x4120_0000; // 10.0f
const F30_0: u32 = 0x41F0_0000; // 30.0f

#[inline]
fn w16(buf: &mut [u8], off: usize, val: u16) {
    buf[off..off + 2].copy_from_slice(&val.to_le_bytes());
}

#[inline]
fn w32(buf: &mut [u8], off: usize, val: u32) {
    buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
}

#[inline]
fn w64(buf: &mut [u8], off: usize, val: u64) {
    buf[off..off + 8].copy_from_slice(&val.to_le_bytes());
}

/// Construit un `CEffectObjComponent` dans `buf` (au moins [`STRUCT_SIZE`] octets) et renvoie
/// le compteur d'instances incrémenté — port byte-exact de `FUN_140071d70`.
///
/// Les octets `0x7a, 0x7b, 0xae, 0xaf, 0xce, 0xcf` ne sont **pas** touchés (interstices). Le
/// pointeur de vtable est écrit au `+0x00` ; passer un `buf` pré-zéro donne une sortie
/// entièrement déterministe.
///
/// # Panics
/// Si `buf.len() < STRUCT_SIZE`.
pub fn construct(buf: &mut [u8], instance_count: u32) -> u32 {
    assert!(
        buf.len() >= STRUCT_SIZE,
        "buffer trop petit pour CEffectObjComponent"
    );
    // memset [0x00, 0x60) (movups xmm0=0 + stores r8=0)
    buf[0..0x60].fill(0);
    w64(buf, 0x54, u64::from(F1_0)); // 1.0f @0x54, 0 @0x58
    w32(buf, 0x5C, 1);
    w64(buf, 0x00, VTABLE); // vtable (écrase le 0 initial / le TAddPropertyCreator transitoire)
    w64(buf, 0x60, 0);
    w64(buf, 0x68, 0);
    w64(buf, 0x70, 0);
    w16(buf, 0x78, 0); // → 0x7a,0x7b intacts
    buf[0x7C..0xA0].fill(0); // boucle 3×12 octets
    w64(buf, 0xA0, 0);
    w32(buf, 0xA8, F10_0); // 10.0f
    w16(buf, 0xAC, 0); // → 0xae,0xaf intacts
    w64(buf, 0xB0, 0);
    w64(buf, 0xB8, u64::from(F30_0)); // 30.0f @0xb8, 0 @0xbc
    w32(buf, 0xC0, 0);
    w32(buf, 0xC4, 0x1_0000);
    w32(buf, 0xC8, 0x1_0000);
    w16(buf, 0xCC, 0x100); // → 0xce,0xcf intacts
    instance_count.wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    use alloc::vec;

    /// Buffer golden capturé de l'oracle uemu (entrée à zéro).
    const GOLDEN_HEX: &str = "508f7b4101000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000803f0000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000020410000000000000000000000000000f0410000000000000000000001000000010000010000";

    fn hexbuf(h: &str) -> alloc::vec::Vec<u8> {
        (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn golden_zero_input() {
        let mut buf = vec![0u8; STRUCT_SIZE];
        let n = construct(&mut buf, 41);
        assert_eq!(n, 42, "compteur d'instances incrémenté");
        assert_eq!(buf, hexbuf(GOLDEN_HEX), "struct 0xD0 byte-exact vs oracle");
    }

    #[test]
    fn preserves_interstices() {
        // Octets non écrits par le ctor : doivent rester tels quels (0x7a,0x7b,0xae,0xaf,0xce,0xcf).
        let mut buf = vec![0xABu8; STRUCT_SIZE];
        construct(&mut buf, 0);
        for &off in &[0x7a, 0x7b, 0xae, 0xaf, 0xce, 0xcf] {
            assert_eq!(buf[off], 0xAB, "octet +{off:#x} doit rester intact");
        }
        // Et une valeur écrite voisine est bien posée.
        assert_eq!(&buf[0xA8..0xAC], &F10_0.to_le_bytes());
    }

    #[test]
    fn counter_wraps() {
        let mut buf = vec![0u8; STRUCT_SIZE];
        assert_eq!(
            construct(&mut buf, u32::MAX),
            0,
            "wrap 32 bits comme inc dword"
        );
    }
}
