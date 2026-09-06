//! Lookup d'enregistrement par **clé 40 bits** dans une table de records de `0xb0` octets — port
//! **byte-exact** de `FUN_1404af530` (@`0x1404af530`, `nie_eacpatched.exe`, ~69 callers).
//!
//! ## Disposition (reversée + validée uemu)
//!
//! L'en-tête de table porte : `+0x68` = pointeur vers le tableau de records, `+0x70` = `count`
//! (`u32`), `+0x81` = drapeau **trié** (`u8`). Chaque record fait `0xb0` octets ; sa **clé 40 bits**
//! est `(u32 @ rec+0xa0) << 8 | (u8 @ rec+0xa4)`. La requête est `(hi: u32, lo: u8)` → même clé.
//!
//! Deux modes selon le drapeau :
//! - **non trié** → scan linéaire (1er record dont la clé == requête) ;
//! - **trié** → recherche binaire (records par clé 40 bits croissante).
//!
//! Le binaire décale les clés de `<<24` et compare en `u64` non signé ; comme `x << 24` est une
//! bijection **ordre-préservante** sur 40 bits (40+24 = 64, pas de débordement), comparer la clé
//! 40 bits directement est équivalent (prouvé par le fuzz).
//!
//! Le binaire renvoie le **pointeur** du record ; ce port renvoie l'**index** (`Option<u32>`),
//! invariant indépendant de l'adresse de base. Validation : `scripts/validate_keyed_record_lookup.py`.

const STRIDE: usize = 0xb0;

#[inline]
fn read_u32(buf: &[u8], off: usize) -> Option<u32> {
    let b = buf.get(off..off + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Vue en lecture d'une table de records `0xb0` o indexée par clé 40 bits.
#[derive(Debug, Clone, Copy)]
pub struct KeyedRecordTable<'a> {
    /// Buffer du tableau de records (champ `+0x68` du binaire), `STRIDE` octets chacun.
    pub recs: &'a [u8],
    /// Nombre de records (`+0x70`).
    pub count: u32,
    /// Records triés par clé croissante (`+0x81 != 0`) → recherche binaire ; sinon scan linéaire.
    pub sorted: bool,
}

impl KeyedRecordTable<'_> {
    /// Clé 40 bits du record `i` : `(u32 @ +0xa0) << 8 | (u8 @ +0xa4)`. `None` si hors limites.
    #[inline]
    fn key_at(&self, i: usize) -> Option<u64> {
        let base = i * STRIDE;
        let hi = read_u32(self.recs, base + 0xa0)?;
        let lo = *self.recs.get(base + 0xa4)?;
        Some((u64::from(hi) << 8) | u64::from(lo))
    }

    /// Index du record dont la clé 40 bits vaut `query`, ou `None` — port byte-exact de
    /// `FUN_1404af530` (scan linéaire ou recherche binaire selon [`Self::sorted`]).
    #[must_use]
    pub fn find(&self, query: u64) -> Option<u32> {
        // Le binaire renvoie 0 d'emblée si `param_2` (la moitié haute u32 de la clé) est nul :
        // une clé `< 256` (partie haute = 0) est considérée invalide.
        if (query >> 8) & 0xFFFF_FFFF == 0 {
            return None;
        }
        let n = self.count;
        if n == 0 {
            return None;
        }
        if !self.sorted {
            for i in 0..n {
                if self.key_at(i as usize)? == query {
                    return Some(i);
                }
            }
            return None;
        }
        // Recherche binaire (indices signés comme le binaire ; midpoint = lo + ceil(span/2)).
        let mut lo: i64 = 0;
        let mut hi: i64 = i64::from(n) - 1;
        while lo <= hi {
            let span = hi - lo;
            let mut adj = span + 1;
            if adj < 0 {
                adj = span + 2; // arrondi de division signée du binaire (mort pour span >= 0)
            }
            let mid = (adj >> 1) + lo;
            let rk = self.key_at(mid as usize)?;
            if query == rk {
                return Some(mid as u32);
            }
            let mut nxt_hi = mid - 1;
            if rk <= query {
                lo = mid + 1;
                nxt_hi = hi;
            }
            hi = nxt_hi;
        }
        None
    }

    /// Comme [`Self::find`] mais à partir des deux moitiés de la clé `(hi: u32, lo: u8)`.
    #[must_use]
    pub fn find_key(&self, hi: u32, lo: u8) -> Option<u32> {
        self.find((u64::from(hi) << 8) | u64::from(lo))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    use alloc::vec;
    use alloc::vec::Vec;

    /// Construit un buffer de records `0xb0` o avec les clés 40 bits données (ordre = `keys`).
    fn build(keys: &[u64]) -> Vec<u8> {
        let mut buf = vec![0u8; STRIDE * (keys.len() + 1)];
        for (i, &k) in keys.iter().enumerate() {
            let hi = ((k >> 8) & 0xFFFF_FFFF) as u32;
            let lo = (k & 0xFF) as u8;
            buf[i * STRIDE + 0xa0..i * STRIDE + 0xa4].copy_from_slice(&hi.to_le_bytes());
            buf[i * STRIDE + 0xa4] = lo;
        }
        buf
    }

    #[test]
    fn linear_scan() {
        // clés à partie haute (u32) NON nulle (sinon early-exit param_2==0).
        let keys = [0x10_2030_4050u64, 0x1_07, 0xFF_FFFF_FFFF, 0x1_07];
        let buf = build(&keys);
        let t = KeyedRecordTable {
            recs: &buf,
            count: 4,
            sorted: false,
        };
        assert_eq!(t.find(0x10_2030_4050), Some(0));
        assert_eq!(t.find(0x1_07), Some(1), "1er match en cas de doublon");
        assert_eq!(t.find(0xFF_FFFF_FFFF), Some(2));
        assert_eq!(t.find(0xDE_AD00), None);
        assert_eq!(
            t.find_key(0x1020_3040, 0x50),
            Some(0),
            "clé reconstruite (hi,lo) = 0x1020304050"
        );
    }

    #[test]
    fn binary_search() {
        // clés triées, partie haute non nulle : key_i = (i+1)<<8 | 5.
        let keys: Vec<u64> = (0..16).map(|i| ((i as u64 + 1) << 8) | 5).collect();
        let buf = build(&keys);
        let t = KeyedRecordTable {
            recs: &buf,
            count: 16,
            sorted: true,
        };
        for (i, &k) in keys.iter().enumerate() {
            assert_eq!(t.find(k), Some(i as u32), "trouve {k:#x}");
        }
        assert_eq!(t.find(0x50), None, "absent");
        assert_eq!(t.find(0x1_06), None, "même high, low absent");
    }

    #[test]
    fn empty_and_single() {
        let buf = build(&[0x1_2a]);
        assert_eq!(
            KeyedRecordTable {
                recs: &buf,
                count: 0,
                sorted: true
            }
            .find(0x1_2a),
            None
        );
        assert_eq!(
            KeyedRecordTable {
                recs: &buf,
                count: 1,
                sorted: true
            }
            .find(0x1_2a),
            Some(0)
        );
        assert_eq!(
            KeyedRecordTable {
                recs: &buf,
                count: 1,
                sorted: false
            }
            .find(0x1_2a),
            Some(0)
        );
        assert_eq!(
            KeyedRecordTable {
                recs: &buf,
                count: 1,
                sorted: true
            }
            .find(0x1_2b),
            None
        );
    }

    /// Early-exit byte-exact : partie haute (`param_2`) nulle ⇒ `None`, même clé présente.
    #[test]
    fn high_part_zero_is_rejected() {
        let buf = build(&[0x42]); // clé 0x42 < 256 → partie haute 0
        let t = KeyedRecordTable {
            recs: &buf,
            count: 1,
            sorted: false,
        };
        assert_eq!(
            t.find(0x42),
            None,
            "param_2==0 → le binaire renvoie 0 même si présent"
        );
        assert_eq!(t.find_key(0, 0x42), None);
    }
}
