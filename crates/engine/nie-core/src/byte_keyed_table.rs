//! Lookup par **clé 1 octet** dans une table de records de 8 octets — port **byte-exact** de
//! `FUN_140d8f2b0` (@`0x140d8f2b0`, `nie_eacpatched.exe`, taille 104 o).
//!
//! ## Disposition (reversée + validée uemu)
//!
//! La signature binaire est `FUN_140d8f2b0(struct* p1, u32* out, char key)` :
//! - `count = *(u32*)(p1 + 0x90)` — nombre d'enregistrements ;
//! - `array = *(i64*)(p1 + 0x88)` — pointeur vers le tableau d'enregistrements ;
//! - chaque enregistrement fait **8 octets** : `[key:u8 @ +0][pad 3][value:u32 LE @ +4]`.
//!
//! La fonction scanne linéairement `[0, count)` et, pour le **premier** enregistrement dont
//! l'octet de tête vaut `key`, écrit `*out = value` et renvoie `out`. Si aucun ne correspond,
//! `*out = 0`. Seuls les 8 bits de poids faible du paramètre `key` comptent (`movzx ebx, r8b`).
//!
//! Ce port renvoie directement la valeur `u32` (0 si absente), invariant indépendant de l'adresse
//! du pointeur de sortie que renvoie le binaire. Le test de pointeur non nul et la comparaison
//! signée de l'index du binaire sont sans objet sur une `&[u8]` Rust sûre (tableau toujours
//! alloué, count == nombre d'enregistrements fournis).
//!
//! Validation : `scripts/validate_byte_keyed_table.py` (608/608 cas byte-exact, oracle uemu).

/// Taille d'un enregistrement, en octets (clé + padding + valeur).
pub const STRIDE: usize = 8;

/// Recherche la valeur associée à `key` dans une table brute d'enregistrements de 8 octets.
///
/// `table` est le tableau d'enregistrements contigus (`array` du binaire) ; le nombre
/// d'enregistrements examinés est `table.len() / 8`. Renvoie la valeur (`u32` LE @ +4) du
/// **premier** enregistrement dont l'octet de clé (@ +0) vaut `key`, sinon `0`.
#[inline]
pub fn lookup_raw(table: &[u8], key: u8) -> u32 {
    let count = table.len() / STRIDE;
    for i in 0..count {
        let rec = &table[i * STRIDE..i * STRIDE + STRIDE];
        if rec[0] == key {
            return u32::from_le_bytes([rec[4], rec[5], rec[6], rec[7]]);
        }
    }
    0
}

/// Un enregistrement typé de la table : clé sur 1 octet + valeur `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    /// Octet de clé (champ @ +0).
    pub key: u8,
    /// Valeur associée (champ `u32` @ +4).
    pub value: u32,
}

/// Variante typée de [`lookup_raw`] : renvoie la valeur du premier enregistrement de clé `key`,
/// sinon `0` (mêmes invariants byte-exact que le binaire).
#[inline]
pub fn lookup(records: &[Record], key: u8) -> u32 {
    for r in records {
        if r.key == key {
            return r.value;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden capturés de l'oracle uemu (scripts/validate_byte_keyed_table.py).
    #[test]
    fn golden_hit_and_miss() {
        let recs = [Record {
            key: 0x41,
            value: 0xDEAD_BEEF,
        }];
        assert_eq!(lookup(&recs, 0x41), 0xDEAD_BEEF);
        assert_eq!(lookup(&recs, 0x42), 0); // absent → 0
        assert_eq!(lookup(&[], 0x41), 0); // table vide → 0
    }

    #[test]
    fn golden_first_occurrence_wins() {
        let recs = [
            Record {
                key: 7,
                value: 0x1111_1111,
            },
            Record {
                key: 7,
                value: 0x2222_2222,
            },
            Record {
                key: 9,
                value: 0x3333_3333,
            },
        ];
        assert_eq!(lookup(&recs, 7), 0x1111_1111);
        assert_eq!(lookup(&recs, 9), 0x3333_3333);
    }

    #[test]
    fn golden_key_zero_matches_zero_byte() {
        let recs = [
            Record {
                key: 0,
                value: 0x0BAD_F00D,
            },
            Record {
                key: 1,
                value: 0xCAFE_BABE,
            },
        ];
        assert_eq!(lookup(&recs, 0), 0x0BAD_F00D);
    }

    #[test]
    fn golden_raw_layout_matches_typed() {
        // tableau brut : [0x80, pad, pad, pad, value LE] x2 ; value 0x12345678 puis 0x9ABCDEF0.
        let table: [u8; 16] = [
            0x80, 0, 0, 0, 0x78, 0x56, 0x34, 0x12, //
            0x81, 0, 0, 0, 0xF0, 0xDE, 0xBC, 0x9A,
        ];
        assert_eq!(lookup_raw(&table, 0x80), 0x1234_5678);
        assert_eq!(lookup_raw(&table, 0x81), 0x9ABC_DEF0);
        assert_eq!(lookup_raw(&table, 0x00), 0);
    }
}
