//! Déréférencement + avancement d'un **itérateur de liste typée à tags 2 bits** — port
//! **byte-exact** de `FUN_140052040` (@`0x140052040`, `nie_eacpatched.exe`, 291 o). C'est le
//! `operator*()`/`advance` de l'itérateur dont [`crate::typed_value_reader`] décode les valeurs :
//! même paquetage de tags 2 bits, ici résolu en **pointeur** d'élément.
//!
//! ## Disposition (reversée + validée uemu, 800 cas byte-exact)
//!
//! L'itérateur est `{ +0x00: ptr conteneur, +0x08: i32 index }`. Le conteneur est
//! `{ +0x00: ptr descripteur, +0x08: ptr tags 2 bits, +0x10: ptr valeurs u32 }`. À chaque appel :
//! lit le tag 2 bits à `index` ([`read_tag`]), **écrit `index + 1`** (avance), puis résout selon
//! `(tag, champs du descripteur, valeur u32)` ([`resolve`]) :
//! - **tag 3** (si `desc.flag_a1 == 0 && desc.ptr_30 != 0 && desc.count_38 != 0`) :
//!   `value == 0xFFFFFFFF` → [`SENTINEL_TAG3`] ; sinon `value >= 0` → `base + value` avec
//!   `base = ptr_60 ? ptr_60 : (ptr_e8 ? ptr_e8 : ptr_c8)` ;
//! - **tag 0** (si `desc.flag_a1 != 0 && ptr_30 != 0 && count_38 != 0 && ptr_40 != 0 && ptr_50 != 0`) :
//!   `value == 0xFFFFFFFF` → [`SENTINEL_TAG0`] ; sinon `0 <= value < desc.bound` → `base + value`
//!   avec `base = ptr_58 ? ptr_58 : ptr_50` ;
//! - tout le reste (tags 1/2, conditions non remplies, valeur négative/hors borne) → `NULL`.
//!
//! Les sentinelles sont des adresses de l'image (vérité terrain uemu). Ce port renvoie l'adresse
//! absolue résolue (`base + value`, les `ptr_*` étant fournis par l'appelant) ou un variant
//! d'énumération ; l'avance d'index est exposée par [`advance`].
//!
//! Validation : `scripts/validate_typed_list_iter.py` (oracle uemu, fuzz seedé descripteurs +
//! tags + valeurs : sentinelles, sélections de base, branches NULL).

/// Sentinelle renvoyée pour tag 3 quand `value == 0xFFFFFFFF` (`&DAT_14174559d`).
pub const SENTINEL_TAG3: u64 = 0x1_4174_559D;
/// Sentinelle renvoyée pour tag 0 quand `value == 0xFFFFFFFF` (`&DAT_14174559e`).
pub const SENTINEL_TAG0: u64 = 0x1_4174_559E;

/// Champs (pertinents) du descripteur du conteneur (`*(conteneur + 0x00)`).
///
/// Les `ptr_*` sont des adresses runtime fournies par l'appelant ; seuls leur **nullité** et,
/// pour la base sélectionnée, leur **valeur** comptent. `bound` est `*(u32)(ptr_40 + 8)`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Descriptor {
    /// Octet `+0xa1` (drapeau : `== 0` ⇒ chemin tag 3, `!= 0` ⇒ chemin tag 0).
    pub flag_a1: u8,
    /// Pointeur `+0x30` (doit être non nul dans les deux chemins).
    pub ptr_30: u64,
    /// Compteur `u32 +0x38` (doit être non nul dans les deux chemins).
    pub count_38: u32,
    /// Pointeur `+0x40` (struct de borne ; non nul requis au chemin tag 0).
    pub ptr_40: u64,
    /// Borne `u32` lue à `ptr_40 + 8` (`value < bound` requis au chemin tag 0).
    pub bound: u32,
    /// Pointeur `+0x50` (base de repli tag 0).
    pub ptr_50: u64,
    /// Pointeur `+0x58` (base prioritaire tag 0 si non nul).
    pub ptr_58: u64,
    /// Pointeur `+0x60` (base prioritaire tag 3 si non nul).
    pub ptr_60: u64,
    /// Pointeur `+0xc8` (base de dernier repli tag 3).
    pub ptr_c8: u64,
    /// Pointeur `+0xe8` (base intermédiaire tag 3 si non nul).
    pub ptr_e8: u64,
}

/// Résultat du déréférencement de l'itérateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerefOutcome {
    /// Adresse résolue `base + value`.
    Resolved(u64),
    /// Sentinelle tag 3 ([`SENTINEL_TAG3`]).
    Sentinel3,
    /// Sentinelle tag 0 ([`SENTINEL_TAG0`]).
    Sentinel0,
    /// Pas d'élément (`NULL`).
    Null,
}

/// Lit le **tag 2 bits** à `index` dans le tableau paqueté `tags` — port exact de l'extraction
/// signée du binaire (gère `index < 0` via l'ajustement `index >> 31 & 3`).
///
/// # Panics
/// Indexe `tags[index >> 2]` ; contrat : `index >= -3` et l'octet correspondant doit exister.
#[must_use]
pub fn read_tag(tags: &[u8], index: i32) -> u8 {
    let adj: i32 = if index < 0 { 3 } else { 0 };
    let iv6 = index.wrapping_add(adj);
    let byte_index = iv6 >> 2; // décalage arithmétique 32 bits (asr)
    let v = (iv6 & 3).wrapping_sub(adj); // i32
    let al = (v & 0xFF) as u8;
    let cl = al.wrapping_mul(2); // (al*2) & 0xff
    let shift = u32::from(cl & 0x1F);
    // x86 décale un octet zéro-étendu dans un registre 32 bits (le shift cl est masqué à 0x1f) :
    // reproduire en élargissant l'octet à u32 avant le décalage (un `u8 >> 30` paniquerait).
    ((u32::from(tags[byte_index as usize]) >> shift) & 3) as u8
}

/// Index avancé (`index + 1`) — l'itérateur écrit toujours `index + 1` quel que soit le tag.
#[must_use]
pub fn advance(index: i32) -> i32 {
    index.wrapping_add(1)
}

/// Résout l'élément pointé selon `(tag, value, desc)` — port byte-exact des deux branches de
/// `FUN_140052040`. Voir la doc du module pour la sémantique complète.
#[must_use]
pub fn resolve(desc: &Descriptor, tag: u8, value: u32) -> DerefOutcome {
    match tag {
        3 => {
            if desc.flag_a1 == 0 && desc.ptr_30 != 0 && desc.count_38 != 0 {
                if value == 0xFFFF_FFFF {
                    return DerefOutcome::Sentinel3;
                }
                if value & 0x8000_0000 == 0 {
                    let base = if desc.ptr_60 != 0 {
                        desc.ptr_60
                    } else if desc.ptr_e8 != 0 {
                        desc.ptr_e8
                    } else {
                        desc.ptr_c8
                    };
                    return DerefOutcome::Resolved(base.wrapping_add(u64::from(value)));
                }
            }
            DerefOutcome::Null
        }
        0 => {
            if desc.flag_a1 != 0
                && desc.ptr_30 != 0
                && desc.count_38 != 0
                && desc.ptr_40 != 0
                && desc.ptr_50 != 0
            {
                if value == 0xFFFF_FFFF {
                    return DerefOutcome::Sentinel0;
                }
                if value & 0x8000_0000 == 0 && value < desc.bound {
                    let base = if desc.ptr_58 != 0 {
                        desc.ptr_58
                    } else {
                        desc.ptr_50
                    };
                    return DerefOutcome::Resolved(base.wrapping_add(u64::from(value)));
                }
            }
            DerefOutcome::Null
        }
        _ => DerefOutcome::Null, // tags 1 et 2 → NULL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // tags paqueté : byte0 = 0b11_10_01_00 = 0xE4 (tag0=0,1,2,3) ; byte1 = 0x1B (tag4=3,5=2,6=1,7=0).
    const TAGS: &[u8] = &[0xE4, 0x1B, 0, 0, 0, 0, 0, 0];

    /// Goldens read_tag capturés du modèle validé byte-exact (FUN_140052040).
    #[test]
    fn golden_read_tag() {
        let expect = [0u8, 1, 2, 3, 3, 2, 1, 0];
        for (i, &e) in expect.iter().enumerate() {
            assert_eq!(read_tag(TAGS, i as i32), e, "index {i}");
        }
        // Indices négatifs (chemin signé) : tag 0 ici.
        assert_eq!(read_tag(TAGS, -1), 0);
        assert_eq!(read_tag(TAGS, -2), 0);
        assert_eq!(read_tag(TAGS, -3), 0);
    }

    #[test]
    fn advance_is_plus_one() {
        assert_eq!(advance(0), 1);
        assert_eq!(advance(-1), 0);
        assert_eq!(advance(i32::MAX), i32::MIN); // wrap
    }

    fn desc_tag3() -> Descriptor {
        Descriptor {
            flag_a1: 0,
            ptr_30: 0x1000,
            count_38: 5,
            ..Default::default()
        }
    }
    fn desc_tag0() -> Descriptor {
        Descriptor {
            flag_a1: 1,
            ptr_30: 0x1000,
            count_38: 5,
            ptr_40: 0x2000,
            bound: 10,
            ptr_50: 0x5000,
            ..Default::default()
        }
    }

    #[test]
    fn tag3_base_priority_and_sentinel() {
        let mut d = desc_tag3();
        d.ptr_c8 = 0xC000;
        // dernier repli : ptr_c8
        assert_eq!(resolve(&d, 3, 7), DerefOutcome::Resolved(0xC007));
        // intermédiaire : ptr_e8 prime sur ptr_c8
        d.ptr_e8 = 0xE000;
        assert_eq!(resolve(&d, 3, 7), DerefOutcome::Resolved(0xE007));
        // prioritaire : ptr_60 prime sur tout
        d.ptr_60 = 0x6000;
        assert_eq!(resolve(&d, 3, 7), DerefOutcome::Resolved(0x6007));
        // sentinelle
        assert_eq!(resolve(&d, 3, 0xFFFF_FFFF), DerefOutcome::Sentinel3);
        // valeur négative (bit de signe) → NULL
        assert_eq!(resolve(&d, 3, 0x8000_0000), DerefOutcome::Null);
    }

    #[test]
    fn tag3_guard_fails_null() {
        let mut d = desc_tag3();
        d.ptr_60 = 0x6000;
        d.flag_a1 = 1; // tag 3 exige flag_a1 == 0
        assert_eq!(resolve(&d, 3, 7), DerefOutcome::Null);
    }

    #[test]
    fn tag0_base_priority_bound_and_sentinel() {
        let mut d = desc_tag0();
        // repli : ptr_50
        assert_eq!(resolve(&d, 0, 3), DerefOutcome::Resolved(0x5003));
        // prioritaire : ptr_58
        d.ptr_58 = 0x5800;
        assert_eq!(resolve(&d, 0, 3), DerefOutcome::Resolved(0x5803));
        // sentinelle
        assert_eq!(resolve(&d, 0, 0xFFFF_FFFF), DerefOutcome::Sentinel0);
        // hors borne (value >= bound) → NULL
        assert_eq!(resolve(&d, 0, 10), DerefOutcome::Null);
        assert_eq!(resolve(&d, 0, 9), DerefOutcome::Resolved(0x5809));
    }

    #[test]
    fn tags_1_2_are_null() {
        assert_eq!(resolve(&desc_tag3(), 1, 0), DerefOutcome::Null);
        assert_eq!(resolve(&desc_tag0(), 2, 0), DerefOutcome::Null);
    }
}
