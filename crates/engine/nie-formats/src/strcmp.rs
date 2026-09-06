//! Port byte-exact de `FUN_14168b570` = `strcmp` (implementation SIMD-optimisee de MSVC,
//! VS 2012-2019), extraite de `nie_eacpatched.exe`.
//!
//! Semantique : comparaison lexicographique de deux chaines C par octets NON SIGNES.
//! La fonction native lit 8 octets a la fois quand `_Str1` est aligne sur 8 et qu'une garde
//! de page le permet (`(_Str2 & 0xfff) <= 0xff8`), sinon octet-par-octet ; les deux chemins
//! produisent la meme semantique. Le registre `rax` natif vaut :
//!   - `0`                   si les chaines sont egales,
//!   - `1`                   au premier octet ou `str1[i] > str2[i]`,
//!   - `0xFFFFFFFFFFFFFFFF`  au premier octet ou `str1[i] < str2[i]`
//!     (`sbb rax,rax ; or rax,1` sur 64 bits).
//! Le resultat `int` visible par l'appelant (eax) est donc `0`, `1` ou `-1`.
//!
//! Ce port prend deux tranches d'octets representant le CONTENU des chaines C
//! (terminateur NUL exclu). La comparaison traite l'octet implicite de fin (0) :
//! une chaine prefixe de l'autre est consideree plus petite.
//!
//! Validation : `scripts/validate_strcmp.py` confronte ce miroir a l'oracle uemu sur
//! 668 cas (alignements de `_Str1` varies, prefixes communs, chaines egales, garbage
//! divergent apres le NUL, garde de page) -> BYTE-EXACT (0 ecart).

/// Comparaison `strcmp` (octets non signes) sur le contenu de deux chaines C.
///
/// `a` et `b` sont les octets des chaines SANS le terminateur NUL.
/// Retourne `-1`, `0` ou `1`, identique a l'`int` renvoye par `FUN_14168b570`.
pub fn strcmp(a: &[u8], b: &[u8]) -> i32 {
    let n = if a.len() < b.len() { a.len() } else { b.len() };
    let mut i = 0;
    while i < n {
        // Comparaison NON signee (movzx + cmp al, byte -> drapeau de retenue non signe).
        if a[i] != b[i] {
            return if a[i] > b[i] { 1 } else { -1 };
        }
        i += 1;
    }
    // Prefixe commun jusqu'a min(len) ; la chaine la plus courte a son NUL (0) ici.
    match a.len().cmp(&b.len()) {
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Less => -1,   // a[n]=0 < b[n]
        core::cmp::Ordering::Greater => 1, // a[n] > 0 = b[n]
    }
}

/// Valeur brute du registre `rax` telle que renvoyee par la fonction native, pour
/// reproduction exacte au niveau registre (`-1` devient `0xFFFFFFFFFFFFFFFF`).
pub fn strcmp_rax(a: &[u8], b: &[u8]) -> u64 {
    match strcmp(a, b) {
        0 => 0,
        1 => 1,
        _ => 0xFFFF_FFFF_FFFF_FFFF, // -1 etendu sur 64 bits (sbb rax,rax ; or rax,1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Valeurs golden capturees de l'oracle uemu (rax brut + int).
    #[test]
    fn golden_oracle() {
        assert_eq!(strcmp(b"abc", b"abc"), 0);
        assert_eq!(strcmp(b"abc", b"abd"), -1);
        assert_eq!(strcmp(b"abd", b"abc"), 1);
        assert_eq!(strcmp(b"abc", b"abcd"), -1); // prefixe -> plus court = plus petit
        assert_eq!(strcmp(b"abcd", b"abc"), 1);
        assert_eq!(strcmp(b"", b""), 0);
        assert_eq!(strcmp(b"\xff", b"\x01"), 1); // octets NON signes : 0xff > 0x01
        assert_eq!(strcmp(b"hello world test", b"hello world tesu"), -1);
    }

    #[test]
    fn golden_rax_register() {
        assert_eq!(strcmp_rax(b"abc", b"abc"), 0x0000_0000_0000_0000);
        assert_eq!(strcmp_rax(b"abc", b"abd"), 0xFFFF_FFFF_FFFF_FFFF);
        assert_eq!(strcmp_rax(b"abd", b"abc"), 0x0000_0000_0000_0001);
        assert_eq!(strcmp_rax(b"abc", b"abcd"), 0xFFFF_FFFF_FFFF_FFFF);
    }

    #[test]
    fn unsigned_byte_order() {
        // Le tri est non signe : 0x80 > 0x7f.
        assert_eq!(strcmp(b"\x80abc", b"\x7fabc"), 1);
        assert_eq!(strcmp(b"\x01", b"\xff"), -1);
    }

    #[test]
    fn chunk_boundaries() {
        // Exactement 8 (un chunk plein) et au-dela (deux chunks) du chemin rapide.
        assert_eq!(strcmp(b"abcdefgh", b"abcdefgh"), 0);
        assert_eq!(strcmp(b"abcdefgh", b"abcdefgi"), -1);
        assert_eq!(strcmp(b"abcdefghij", b"abcdefghij"), 0);
        assert_eq!(strcmp(b"abcdefghij", b"abcdefghik"), -1);
    }
}
