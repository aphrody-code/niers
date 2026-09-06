//! Lecteur de valeurs typées indexées du moteur Level-5 — port **byte-exact** de `FUN_140051f40`
//! (@`0x140051f40`, `nie_eacpatched.exe`, **~1696 callers**), l'itérateur « lire la valeur suivante
//! en int » du système de valeurs typées (même famille que [`crate::menu_setting`], port de
//! `FUN_141290fc0`).
//!
//! ## Disposition (reversée + validée uemu)
//!
//! Une « liste » est deux tableaux parallèles + un index courant :
//! - `tags` (champ `obj+8`) : tableau de **tags 2 bits packés** (4 par octet) ; le tag de l'index
//!   `i` est `tags[i/4] >> ((i%4)*2) & 3` ;
//! - `data` (champ `obj+0x10`) : valeurs de **4 octets** (i32 brut si tag 1, bits f32 si tag 2) ;
//! - `index` (champ `param+8`) : position courante, **avancée** à chaque lecture.
//!
//! `read_next_int` renvoie : tag 1 → l'i32 brut ; tag 2 → [`cvttss2si`] du f32 ; tag 0/3 → 0.
//!
//! ## Validation
//!
//! Oracle **uemu byte-exact** (`scripts/validate_typed_value_reader.py`, 800 cas fuzz avec floats
//! limites). La conversion float→int reproduit `CVTTSS2SI` du x86, PAS `f as i32` de Rust (qui
//! sature) — voir [`cvttss2si`].

/// Conversion x86 `CVTTSS2SI` exacte : troncature **vers zéro**, avec « integer indefinite »
/// `0x80000000` (= [`i32::MIN`]) pour `NaN`, `±inf` et tout résultat hors `[i32::MIN, i32::MAX]`.
///
/// Diffère de `f as i32` (Rust), qui **sature** (`+inf`→`i32::MAX`, `NaN`→`0`) : indispensable au
/// byte-exact.
#[must_use]
pub fn cvttss2si(f: f32) -> i32 {
    if f.is_nan() {
        return i32::MIN;
    }
    let t = f.trunc();
    // Représentable en i32 ssi `-2^31 <= t < 2^31` (2^31 exactement représentable en f32) ; sinon
    // « integer indefinite ». (Demi-ouvert : `-2^31` est OK, `+2^31` non.)
    if !(-2_147_483_648.0_f32..2_147_483_648.0_f32).contains(&t) {
        return i32::MIN;
    }
    t as i32
}

/// Vue d'une liste de valeurs typées : tags 2 bits packés + valeurs 4 o + index courant.
/// Port de la structure lue par `FUN_140051f40` (`param[0]`=obj, `param[8]`=index ;
/// `obj[8]`=tags, `obj[0x10]`=data).
#[derive(Debug, Clone)]
pub struct TypedValueList<'a> {
    /// Tags 2 bits packés (`obj+8`) : tag de l'index `i` = `tags[i/4] >> ((i%4)*2) & 3`.
    pub tags: &'a [u8],
    /// Valeurs, 4 octets chacune (`obj+0x10`) : i32 brut (tag 1) ou bits f32 (tag 2).
    pub data: &'a [u8],
    /// Index courant (`param+8`), avancé à chaque lecture. `>= 0` en usage réel.
    pub index: i32,
}

impl TypedValueList<'_> {
    /// Lit la valeur à l'index courant **en int** et **avance** l'index — port byte-exact de
    /// `FUN_140051f40`.
    ///
    /// Tag 1 → i32 brut ; tag 2 → [`cvttss2si`] du f32 ; tag 0/3 → 0. Renvoie `None` si le tag ou
    /// la valeur est hors limites (jamais en usage valide : `0 <= index` et index `<` nombre
    /// d'entrées). L'index est avancé dès que le tag est lu, comme le binaire.
    pub fn read_next_int(&mut self) -> Option<i32> {
        let (tag, i) = self.tag_and_advance()?;
        Some(match tag {
            1 | 2 => {
                let raw = self.word_at(i)?;
                if tag == 1 {
                    raw as i32 // i32 brut
                } else {
                    cvttss2si(f32::from_bits(raw)) // float → int x86 (troncature/indefinite)
                }
            }
            _ => 0, // tag 0/3 : ne lit même pas `data`
        })
    }

    /// Lit la valeur à l'index courant **en float** et **avance** l'index — port byte-exact de
    /// `FUN_140051fc0` (~331 callers), le sibling float de [`Self::read_next_int`]. Ghidra avait
    /// typé cette fonction `void` (retour `xmm0` perdu).
    ///
    /// Tag 1 → `(f32)i32` (`cvtsi2ss`, arrondi au plus proche pair = `as f32` de Rust) ; tag 2 → le
    /// f32 brut (passthrough) ; tag 0/3 → `0.0`. `None` si hors limites (jamais en usage valide).
    pub fn read_next_float(&mut self) -> Option<f32> {
        let (tag, i) = self.tag_and_advance()?;
        Some(match tag {
            1 | 2 => {
                let raw = self.word_at(i)?;
                if tag == 1 {
                    raw as i32 as f32 // cvtsi2ss : i32 → f32, arrondi au plus proche
                } else {
                    f32::from_bits(raw) // f32 brut
                }
            }
            _ => 0.0, // tag 0/3 : ne lit même pas `data`
        })
    }

    /// Lit le **tag 2 bits** de l'index courant puis avance l'index. Renvoie `(tag, index_lu)` ou
    /// `None` (tag hors limites). Le binaire avance après la lecture du tag, avant tout branch.
    fn tag_and_advance(&mut self) -> Option<(u8, i32)> {
        let i = self.index;
        let corr = (i >> 31) & 3; // 0 si i >= 0, 3 si i < 0 (correction de division signée)
        let ic = i.wrapping_add(corr);
        let byteoff = (ic >> 2) as usize;
        let slot = (ic & 3) - corr;
        let shift = ((slot * 2) & 0x1f) as u32;
        let tag = (self.tags.get(byteoff)? >> shift) & 3;
        self.index = i.wrapping_add(1);
        Some((tag, i))
    }

    /// Mot de 4 octets de `data` à l'index `i` (`data[i*4..]`). `None` si hors limites.
    #[inline]
    fn word_at(&self, i: i32) -> Option<u32> {
        let off = (i as usize).checked_mul(4)?;
        let w = self.data.get(off..off + 4)?;
        Some(u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    use alloc::vec::Vec;

    /// CVTTSS2SI exact vs la saturation de `f as i32` (cas limites).
    #[test]
    fn cvttss2si_matches_x86() {
        assert_eq!(cvttss2si(1.9), 1);
        assert_eq!(cvttss2si(-1.9), -1);
        assert_eq!(cvttss2si(0.0), 0);
        assert_eq!(cvttss2si(255.0), 255);
        assert_eq!(cvttss2si(-2_147_483_648.0), i32::MIN, "-2^31 représentable");
        assert_eq!(
            cvttss2si(2_147_483_648.0),
            i32::MIN,
            "+2^31 hors i32 → indefinite"
        );
        assert_eq!(
            cvttss2si(1e20),
            i32::MIN,
            "overflow → indefinite (≠ saturation)"
        );
        assert_eq!(cvttss2si(-1e20), i32::MIN);
        assert_eq!(
            cvttss2si(f32::NAN),
            i32::MIN,
            "NaN → indefinite (≠ 0 de `as`)"
        );
        assert_eq!(cvttss2si(f32::INFINITY), i32::MIN);
        assert_eq!(cvttss2si(f32::NEG_INFINITY), i32::MIN);
    }

    /// Construit tags packés + buffer de données depuis (tag, mot 4 o).
    fn build(entries: &[(u8, [u8; 4])]) -> (Vec<u8>, Vec<u8>) {
        let mut tags = alloc::vec![0u8; entries.len().div_ceil(4) + 4];
        let mut data = Vec::new();
        for (i, &(t, w)) in entries.iter().enumerate() {
            tags[i >> 2] |= (t & 3) << ((i & 3) * 2);
            data.extend_from_slice(&w);
        }
        (tags, data)
    }

    #[test]
    fn read_next_int_tags_and_advance() {
        // index 0: tag1 int 42 ; 1: tag2 float 1.5 ; 2: tag0 → 0 ; 3: tag2 float 2^31 → MIN.
        let entries = [
            (1u8, 42i32.to_le_bytes()),
            (2u8, 1.5f32.to_bits().to_le_bytes()),
            (0u8, 7i32.to_le_bytes()),
            (2u8, 2_147_483_648.0f32.to_bits().to_le_bytes()),
        ];
        let (tags, data) = build(&entries);
        let mut r = TypedValueList {
            tags: &tags,
            data: &data,
            index: 0,
        };
        assert_eq!(r.read_next_int(), Some(42));
        assert_eq!(r.index, 1, "index avancé");
        assert_eq!(r.read_next_int(), Some(1), "cvttss2si(1.5)=1");
        assert_eq!(r.read_next_int(), Some(0), "tag 0 → 0");
        assert_eq!(
            r.read_next_int(),
            Some(i32::MIN),
            "cvttss2si(2^31)=indefinite"
        );
        assert_eq!(r.index, 4);
    }

    /// read_next_float : tag 1 → (f32)i32 ; tag 2 → f32 brut ; tag 0/3 → 0.0 ; avance l'index.
    #[test]
    fn read_next_float_tags_and_advance() {
        let entries = [
            (1u8, 42i32.to_le_bytes()),            // int 42 → 42.0
            (2u8, 1.5f32.to_bits().to_le_bytes()), // float 1.5 → 1.5
            (0u8, 99i32.to_le_bytes()),            // tag 0 → 0.0
            (1u8, (-7i32).to_le_bytes()),          // int -7 → -7.0
            (1u8, 16_777_219i32.to_le_bytes()),    // > 2^24 → arrondi cvtsi2ss
        ];
        let (tags, data) = build(&entries);
        let mut r = TypedValueList {
            tags: &tags,
            data: &data,
            index: 0,
        };
        assert_eq!(r.read_next_float(), Some(42.0));
        assert_eq!(r.index, 1);
        assert_eq!(r.read_next_float(), Some(1.5));
        assert_eq!(r.read_next_float(), Some(0.0), "tag 0 → 0.0");
        assert_eq!(r.read_next_float(), Some(-7.0));
        assert_eq!(
            r.read_next_float(),
            Some(16_777_219i32 as f32),
            "cvtsi2ss arrondi RNE"
        );
        assert_eq!(r.index, 5);
    }

    /// Lecture à un index de départ non nul.
    #[test]
    fn read_next_int_midlist() {
        let entries = [
            (1u8, 10i32.to_le_bytes()),
            (1u8, 20i32.to_le_bytes()),
            (1u8, (-5i32).to_le_bytes()),
        ];
        let (tags, data) = build(&entries);
        let mut r = TypedValueList {
            tags: &tags,
            data: &data,
            index: 2,
        };
        assert_eq!(r.read_next_int(), Some(-5));
        assert_eq!(r.index, 3);
    }
}
