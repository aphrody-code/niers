//! Recherche du **premier enregistrement actif** via une table de handles validés — port
//! **byte-exact** de `FUN_1411da770` (@`0x1411da770`, `nie_eacpatched.exe`, ~27 callers). Motif
//! classique de moteur : des **handles** `(génération << 16) | (index+1)` indexent des
//! enregistrements, la génération protégeant contre les handles périmés (stale).
//!
//! ## Disposition (reversée + validée uemu)
//!
//! Le manager singleton expose : `count` (`u32 @ +0x110`), un tableau d'enregistrements de
//! **0x1480** octets (`*(ptr) @ +0x80` ; génération `u16 @ +0x1462`, drapeau actif `u8 @ +0x48`)
//! et un tableau de handles `u32` (`*(ptr) @ +0x118`).
//!
//! [`HandleTable::find_active`] parcourt les handles : un handle **nul** ou de **génération
//! incohérente** interrompt et renvoie `None` ; le premier enregistrement avec drapeau actif `== 1`
//! (et génération valide) est renvoyé. Le binaire renvoie le **pointeur** de l'enregistrement ; ce
//! port renvoie l'**index** (`Option<u32>`), invariant indépendant de l'adresse de base.
//!
//! Validation : `scripts/validate_handle_table.py` (le manager est fourni en mémoire uemu — global
//! paramétré, non dégénéré).

const REC_STRIDE: usize = 0x1480;

#[inline]
fn read_u16(buf: &[u8], off: usize) -> Option<u16> {
    let b = buf.get(off..off + 2)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

/// Vue en lecture d'une table de handles + enregistrements.
#[derive(Debug, Clone, Copy)]
pub struct HandleTable<'a> {
    /// Buffer des enregistrements (`*(mgr+0x80)`), `REC_STRIDE` octets chacun.
    pub records: &'a [u8],
    /// Tableau de handles `u32` (`*(mgr+0x118)`) : `(génération << 16) | (index+1)`.
    pub handles: &'a [u32],
    /// Nombre de handles (`*(mgr+0x110)`).
    pub count: u32,
}

impl HandleTable<'_> {
    /// Index du **premier enregistrement actif** (drapeau `+0x48 == 1`, génération valide), ou
    /// `None` — port byte-exact de `FUN_1411da770`. S'arrête (`None`) au premier handle nul ou de
    /// génération incohérente.
    #[must_use]
    pub fn find_active(&self) -> Option<u32> {
        for i in 0..self.count as usize {
            let h = *self.handles.get(i)?;
            if h == 0 {
                return None; // handle nul → arrêt
            }
            let idx = ((h & 0xffff) as usize).checked_sub(1)?; // index = (h&0xffff) - 1
            let base = idx.checked_mul(REC_STRIDE)?;
            let genr = read_u16(self.records, base + 0x1462)?;
            if u32::from(genr) != (h >> 16) {
                return None; // génération périmée → arrêt
            }
            if *self.records.get(base + 0x48)? == 1 {
                return Some(idx as u32); // 1er actif
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    use alloc::vec;
    use alloc::vec::Vec;

    /// Construit le buffer d'enregistrements (gen @+0x1462, actif @+0x48).
    fn build_records(recs: &[(u16, u8)]) -> Vec<u8> {
        let mut buf = vec![0u8; REC_STRIDE * (recs.len() + 1)];
        for (i, &(genr, act)) in recs.iter().enumerate() {
            buf[i * REC_STRIDE + 0x1462..i * REC_STRIDE + 0x1464]
                .copy_from_slice(&genr.to_le_bytes());
            buf[i * REC_STRIDE + 0x48] = act;
        }
        buf
    }

    fn handle(genr: u16, idx0: u32) -> u32 {
        (u32::from(genr) << 16) | (idx0 + 1)
    }

    /// Premier actif renvoyé ; les inactifs précédents sont sautés.
    #[test]
    fn returns_first_active() {
        let recs = [(1u16, 0u8), (2, 1), (3, 1)]; // rec0 inactif, rec1/rec2 actifs
        let buf = build_records(&recs);
        let handles = [handle(1, 0), handle(2, 1), handle(3, 2)];
        let t = HandleTable {
            records: &buf,
            handles: &handles,
            count: 3,
        };
        assert_eq!(t.find_active(), Some(1), "saute rec0 inactif → rec1 actif");
    }

    /// Handle nul → arrêt immédiat (None), même si un actif suit.
    #[test]
    fn null_handle_bails() {
        let recs = [(1u16, 1u8)];
        let buf = build_records(&recs);
        let handles = [0, handle(1, 0)];
        let t = HandleTable {
            records: &buf,
            handles: &handles,
            count: 2,
        };
        assert_eq!(t.find_active(), None);
    }

    /// Génération incohérente (handle périmé) → arrêt (None).
    #[test]
    fn stale_generation_bails() {
        let recs = [(5u16, 1u8)];
        let buf = build_records(&recs);
        let handles = [handle(9, 0)]; // gen 9 ≠ rec gen 5
        let t = HandleTable {
            records: &buf,
            handles: &handles,
            count: 1,
        };
        assert_eq!(t.find_active(), None);
    }

    /// Aucun actif (tous inactifs, générations valides) → None après parcours complet.
    #[test]
    fn no_active_returns_none() {
        let recs = [(1u16, 0u8), (2, 0)];
        let buf = build_records(&recs);
        let handles = [handle(1, 0), handle(2, 1)];
        let t = HandleTable {
            records: &buf,
            handles: &handles,
            count: 2,
        };
        assert_eq!(t.find_active(), None);
    }
}
