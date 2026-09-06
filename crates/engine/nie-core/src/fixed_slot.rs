//! Port byte-exact de `FUN_140506730` (nie_eacpatched.exe @ 0x140506730).
//!
//! Sémantique : *append* d'un élément dans un tableau à capacité fixe. La structure conteneur
//! expose un pointeur de base (`+0x60`), une capacité `u32` (`+0x6c`) et un compteur `u32`
//! (`+0x70`). Chaque élément fait `0xB8` (184) octets et semble regrouper 3 « canaux » de 0x38
//! octets (chacun pré-rempli avec 1.0, 1.0, 2.0 en f32 — gabarit d'animation/tween probable) plus
//! un petit en-tête en fin d'élément.
//!
//! Si le pointeur de base est non nul **et** `count < cap`, la fonction initialise l'élément
//! d'indice `count` avec des constantes par défaut puis y dépose 4 paramètres `u32` :
//!   - `p2` (rdx)        → offset `0xa8`
//!   - `p3` (r8)         → offset `0x00`
//!   - `p4` (r9)         → offset `0x38`
//!   - `p5` (pile)       → offset `0x70`
//!
//! puis incrémente `count`. Sinon, no-op.
//!
//! Important : la fonction n'écrit QUE des offsets précis ; les octets non touchés conservent
//! leur valeur antérieure (pas de mise à zéro complète de l'élément). Le port reflète exactement
//! cet ensemble d'écritures (largeurs et ordre inclus).
//!
//! Validation : oracle uemu (`scripts/validate_fixed_slot_append.py`), 600 cas fuzz seedés (LCG
//! déterministe) comparés BYTE-EXACT — couvre base nul, `count >= cap` (no-op) et append.

#![allow(dead_code)]

/// Taille d'un élément du tableau, en octets.
pub const ELEM_SIZE: usize = 0xB8;

const F1_0: u32 = 0x3F80_0000; // 1.0_f32
const F2_0: u32 = 0x4000_0000; // 2.0_f32

#[inline]
fn w32(buf: &mut [u8], off: usize, val: u32) {
    buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
}

#[inline]
fn w16(buf: &mut [u8], off: usize, val: u16) {
    buf[off..off + 2].copy_from_slice(&val.to_le_bytes());
}

/// Initialise un élément (`>= 0xB8` octets) en place, reproduisant les deux boucles et les
/// écritures post-boucle de `FUN_140506730`. Les octets hors des offsets écrits sont laissés
/// intacts.
fn init_elem(e: &mut [u8], p2: u32, p3: u32, p4: u32, p5: u32) {
    // Boucle 1 : 3 sous-blocs de 0x38 octets. rax pointe e+8+s ; les écritures qword 8 octets
    // (constante 0x3f800000 / 0) sont scindées en deux u32 (low = constante, high = 0).
    for i in 0..3 {
        let s = i * 0x38;
        let r = 8 + s; // = offset de rax dans l'élément
        w32(e, r - 4, 0);
        w32(e, r, F1_0); // qword 0x3f800000
        w32(e, r + 4, 0);
        w32(e, r + 8, F1_0); // qword 0x3f800000
        w32(e, r + 12, 0);
        w32(e, r + 0x10, F2_0); // dword 0x40000000
        w32(e, r + 0x20, 0); // qword 0
        w32(e, r + 0x24, 0);
        w16(e, r + 0x28, 0);
        e[r + 0x2a] = 0;
        w32(e, r + 0x2c, 0);
    }
    // Après-boucle (en-tête de fin d'élément).
    w32(e, 0xB0, 0);
    w32(e, 0xB4, F1_0);
    w32(e, 0xA8, 0);
    e[0xAC] = 1;
    w32(e, 0xA8, p2); // écrase le 0 précédent
    // Boucle 2 : dépose p3, p4, p5 aux offsets 0, 0x38, 0x70.
    w32(e, 0x00, p3);
    w32(e, 0x38, p4);
    w32(e, 0x70, p5);
}

/// Ajoute un élément au tableau `array` à l'indice `count` si `count < cap` et que le tableau
/// dispose de la place. Renvoie le nouveau compteur (`count + 1` si append, sinon `count`).
///
/// `array` est la zone mémoire brute du tableau (au moins `cap * ELEM_SIZE` octets attendus).
/// Le cas « base nulle » de l'original (no-op) correspond à l'absence de tableau côté appelant :
/// passer un `array` vide / `cap == 0` produit aussi un no-op.
pub fn push(array: &mut [u8], cap: u32, count: u32, p2: u32, p3: u32, p4: u32, p5: u32) -> u32 {
    if count < cap {
        let off = (count as usize).saturating_mul(ELEM_SIZE);
        if off + ELEM_SIZE <= array.len() {
            init_elem(&mut array[off..off + ELEM_SIZE], p2, p3, p4, p5);
            return count + 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hexbuf(h: &str) -> Vec<u8> {
        (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect()
    }

    // CASE1 (oracle uemu) : prior = 0xAA*, cap=4, count=0, p2=0x11223344, p3=0xAABBCCDD,
    // p4=0x01020304, p5=0xDEADBEEF. Vérifie l'élément 0 produit + count -> 1.
    #[test]
    fn case1_append_index0() {
        let mut arr = vec![0xAAu8; 4 * ELEM_SIZE];
        let n = push(
            &mut arr,
            4,
            0,
            0x1122_3344,
            0xAABB_CCDD,
            0x0102_0304,
            0xDEAD_BEEF,
        );
        assert_eq!(n, 1);
        let expect = hexbuf(
            "ddccbbaa000000000000803f000000000000803f0000000000000040aaaaaaaaaaaaaaaaaaaaaaaa\
             0000000000000000000000aa0000000004030201000000000000803f000000000000803f00000000\
             00000040aaaaaaaaaaaaaaaaaaaaaaaa0000000000000000000000aa00000000efbeadde00000000\
             0000803f000000000000803f0000000000000040aaaaaaaaaaaaaaaaaaaaaaaa0000000000000000\
             000000aa000000004433221101aaaaaa000000000000803f",
        );
        assert_eq!(&arr[..ELEM_SIZE], &expect[..]);
    }

    // CASE2 : count >= cap -> no-op, tableau intact, count inchangé.
    #[test]
    fn case2_full_noop() {
        let prior = vec![0x55u8; 2 * ELEM_SIZE];
        let mut arr = prior.clone();
        let n = push(&mut arr, 2, 2, 1, 2, 3, 4);
        assert_eq!(n, 2);
        assert_eq!(arr, prior);
    }

    // CASE3 (oracle uemu) : prior = 0x00*, cap=3, count=1 -> append à l'indice 1, élément 0 reste
    // nul. p2=0x7fffffff, p3=0, p4=0xffffffff, p5=0x40490fdb (pi).
    #[test]
    fn case3_append_index1() {
        let mut arr = vec![0x00u8; 3 * ELEM_SIZE];
        let n = push(&mut arr, 3, 1, 0x7FFF_FFFF, 0, 0xFFFF_FFFF, 0x4049_0FDB);
        assert_eq!(n, 2);
        assert!(arr[..ELEM_SIZE].iter().all(|&b| b == 0)); // élément 0 intact
        let expect = hexbuf(
            "00000000000000000000803f00000000\
             0000803f000000000000004000000000\
             00000000000000000000000000000000\
             0000000000000000ffffffff00000000\
             0000803f000000000000803f00000000\
             00000040000000000000000000000000\
             00000000000000000000000000000000\
             db0f4940000000000000803f00000000\
             0000803f000000000000004000000000\
             00000000000000000000000000000000\
             0000000000000000ffffff7f01000000\
             000000000000803f",
        );
        assert_eq!(&arr[ELEM_SIZE..2 * ELEM_SIZE], &expect[..]);
    }

    // base nulle / cap=0 -> no-op.
    #[test]
    fn empty_array_noop() {
        let mut arr: Vec<u8> = Vec::new();
        assert_eq!(push(&mut arr, 0, 0, 1, 2, 3, 4), 0);
    }
}
