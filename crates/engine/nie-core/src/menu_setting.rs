//! Décodeur runtime des valeurs typées d'un objet-menu (tick de `CMenuStateMachine`).
//!
//! Port **byte-exact** du sous-bloc de résolution de `FUN_141290fc0` (`@0x141290fc0`,
//! `nie_eacpatched.exe`). À chaque tick, le moteur lit **9 champs typés** consécutifs de la
//! struct objet-menu (un champ tous les `0x14` octets à partir de `+0x28` : `tag` en `+0`,
//! valeur brute en `+4`) et écrit les **9 paramètres runtime** correspondants dans la plage
//! `+0x590..=+0x5ba` de la même struct. C'est le pont concret entre les valeurs `cfg.bin`
//! déjà parsées (cf. [`nie_data::menu_setting`]) et l'état vivant de l'écran.
//!
//! ## Sémantique du tag (vérifiée sur le désassemblage iced-x86)
//!
//! | tag | interprétation de la valeur brute |
//! |-----|-----------------------------------|
//! | 1   | entier (`i32`) — ou, pour un champ flottant, `int → float` (`cvtdq2ps`, signé) |
//! | 2   | entier (`i32` / `u32`) |
//! | 3   | flottant brut (`f32::from_bits`) — champ flottant uniquement |
//! | 4   | **référence** à résoudre via `FUN_140453910` si l'octet de garde est non nul (champs résolvables uniquement) |
//! | autre | absent → `0` / `0.0` / `false` |
//!
//! Trois des champs (indices 3/4/5) sont *résolvables* : un tag `4` déclenche une résolution
//! externe (`FUN_140453910`, non modélisée ici — fournie par l'appelant via `resolve_ref`),
//! gardée par `(raw & 0xff) != 0`. Les six autres champs n'utilisent jamais la résolution.
//!
//! ## Validation
//!
//! Oracle **uemu byte-exact** (`scripts/uemu.py`) : émulation de `FUN_141290fc0` sur une struct
//! synthétique, arrêt à `0x1412911bb` (juste avant `FUN_14108d550`, tous les paramètres écrits),
//! lecture de `+0x590..+0x5bb`. Les tests ci-dessous reproduisent à l'octet près les sorties de
//! deux vecteurs émulés (tags `0/1/2/3` + chemin `4`-garde-nulle). Le chemin `4`-résolu appelle
//! `resolve_ref` (résolveur externe), non émulable en isolation.

extern crate alloc;

/// Un champ typé d'un objet-menu : tag de type RDBN + 32 bits de valeur brute.
///
/// Correspond à une paire `(tag @ +0, valeur @ +4)` dans la struct runtime de nie.exe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TypedField {
    /// Tag de type : `1`/`2` = entier, `3` = flottant, `4` = référence à résoudre, autre = absent.
    pub tag: i32,
    /// 32 bits bruts de la valeur, interprétés selon `tag`.
    pub raw: u32,
}

impl TypedField {
    /// Construit un champ typé.
    #[must_use]
    pub const fn new(tag: i32, raw: u32) -> Self {
        Self { tag, raw }
    }
}

/// Paramètres runtime décodés d'un objet-menu, nommés par leur offset dans la struct nie.exe.
///
/// Les noms sémantiques exacts ne sont pas connus (offsets bruts) ; le décodage, lui, est
/// byte-exact. L'ordre des champs *source* (cf. [`resolve_menu_setting`]) est l'ordre des slots
/// `+0x28, +0x3c, … +0xc8` ; chacun atterrit à l'offset indiqué.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuSettingParams {
    /// `+0x590` — champ source 4 (résolvable).
    pub p590: i32,
    /// `+0x594` — champ source 0.
    pub p594: i32,
    /// `+0x598` — champ source 1.
    pub p598: i32,
    /// `+0x59c` — champ source 3 (résolvable).
    pub p59c: i32,
    /// `+0x5a0` — champ source 5 (résolvable).
    pub p5a0: i32,
    /// `+0x5a4` — champ source 6 (drapeaux `u32`).
    pub p5a4: u32,
    /// `+0x5a8` — champ source 7 (flottant).
    pub p5a8: f32,
    /// `+0x5b4` — champ source 2 (octet bas de l'entier décodé).
    pub p5b4: u8,
    /// `+0x5ba` — champ source 8 (booléen).
    pub p5ba: bool,
}

/// Décode un entier de tag `1`/`2` (sinon `0`). Port de la branche
/// `cmp eax,2 / cmp eax,1 → mov reg,[val]` (sinon `mov reg,edi=0`).
#[inline]
fn decode_int(f: TypedField) -> i32 {
    if f.tag == 2 || f.tag == 1 {
        f.raw as i32
    } else {
        0
    }
}

/// Décode un champ *résolvable* (indices source 3/4/5). Tag `4` → résolution externe si l'octet
/// de garde `(raw & 0xff) != 0`, sinon `0` ; tag `1`/`2` → entier ; autre → `0`.
#[inline]
fn decode_resolvable(f: TypedField, resolve_ref: &impl Fn(TypedField) -> i32) -> i32 {
    if f.tag == 4 {
        // Garde du désassemblage : `cmp [rbx+val_off],dil` (dil=0) → résout si l'octet ≠ 0.
        if (f.raw & 0xff) != 0 {
            resolve_ref(f)
        } else {
            0
        }
    } else if f.tag == 2 || f.tag == 1 {
        f.raw as i32
    } else {
        0
    }
}

/// Décode le champ flottant (index source 7). Tag `3` → `f32` brut ; tag `1` → `int → float`
/// (signé, `cvtdq2ps`) ; autre → `0.0`.
#[inline]
fn decode_float(f: TypedField) -> f32 {
    if f.tag == 3 {
        f32::from_bits(f.raw)
    } else if f.tag == 1 {
        // `movd xmm0,[val]; cvtdq2ps xmm0,xmm0` — conversion d'un entier SIGNÉ 32 bits.
        f.raw as i32 as f32
    } else {
        0.0
    }
}

/// Décode les 9 champs typés d'un objet-menu en ses paramètres runtime.
///
/// Port byte-exact de `FUN_141290fc0` (sous-bloc `0x141290fe9..0x1412911ab`). `fields` est dans
/// l'ordre des slots source (`+0x28, +0x3c, +0x50, +0x64, +0x78, +0x8c, +0xa0, +0xb4, +0xc8`).
/// `resolve_ref` modélise `FUN_140453910` (résolution d'une référence de tag `4`) ; il n'est
/// appelé que pour les champs résolvables (indices 3/4/5) dont le tag vaut `4` et l'octet de
/// garde est non nul — jamais sur les chemins entier/flottant/booléen.
#[must_use]
pub fn resolve_menu_setting(
    fields: &[TypedField; 9],
    resolve_ref: impl Fn(TypedField) -> i32,
) -> MenuSettingParams {
    // Ordre des écritures = ordre du binaire (cf. décompile) ; l'ordre n'a pas d'effet de bord.
    let p594 = decode_int(fields[0]); // slot +0x28
    let p598 = decode_int(fields[1]); // slot +0x3c
    let int2 = decode_int(fields[2]); // slot +0x50 → octet bas en +0x5b4
    let p59c = decode_resolvable(fields[3], &resolve_ref); // slot +0x64
    let p590 = decode_resolvable(fields[4], &resolve_ref); // slot +0x78
    let p5a0 = decode_resolvable(fields[5], &resolve_ref); // slot +0x8c
    let p5a4 = if fields[6].tag == 2 || fields[6].tag == 1 {
        fields[6].raw // slot +0xa0 (drapeaux u32)
    } else {
        0
    };
    let p5a8 = decode_float(fields[7]); // slot +0xb4
    let p5ba = decode_int(fields[8]) != 0; // slot +0xc8 (`test edi,edi; setne`)

    MenuSettingParams {
        p590,
        p594,
        p598,
        p59c,
        p5a0,
        p5a4,
        p5a8,
        p5b4: int2 as u8, // `mov [+0x5b4],r12b` — octet bas uniquement
        p5ba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Résolveur qui panique : prouve que les chemins non-`4`-résolus n'invoquent jamais
    /// `FUN_140453910`.
    fn no_resolve(_: TypedField) -> i32 {
        panic!("resolve_ref ne doit pas être appelé sur ce vecteur");
    }

    /// Vecteur V1 de l'oracle uemu (émulé sur `FUN_141290fc0`, arrêt `0x1412911bb`).
    /// tags : 2,1,2,2,1,(4 garde-nulle),2,(3=2.5),(2=5).
    #[test]
    fn matches_uemu_oracle_v1() {
        let f = [
            TypedField::new(2, 0x1111_1111),
            TypedField::new(1, 0x2222_2222),
            TypedField::new(2, 0x0000_00AB),
            TypedField::new(2, 0x3333_3333),
            TypedField::new(1, 0x4444_4444),
            TypedField::new(4, 0x0000_0000), // tag 4, octet de garde nul → 0
            TypedField::new(2, 0xDEAD_BEEF),
            TypedField::new(3, 2.5_f32.to_bits()),
            TypedField::new(2, 5),
        ];
        let p = resolve_menu_setting(&f, no_resolve);
        assert_eq!(p.p590, 0x4444_4444_u32 as i32, "0x590 = 1145324612");
        assert_eq!(p.p594, 0x1111_1111, "0x594 = 286331153");
        assert_eq!(p.p598, 0x2222_2222, "0x598 = 572662306");
        assert_eq!(p.p59c, 0x3333_3333, "0x59c = 858993459");
        assert_eq!(p.p5a0, 0, "0x5a0 = 0 (tag 4 garde-nulle)");
        assert_eq!(p.p5a4, 0xDEAD_BEEF, "0x5a4 = 3735928559");
        assert_eq!(p.p5a8, 2.5, "0x5a8 = 2.5");
        assert_eq!(p.p5b4, 0xAB, "0x5b4 = 171");
        assert!(p.p5ba, "0x5ba = true");
    }

    /// Vecteur V2 de l'oracle uemu : tag 0 → 0 ; flottant via tag 1 (int=7 → 7.0) ;
    /// booléen 0 → false ; octet bas 0x02.
    #[test]
    fn matches_uemu_oracle_v2() {
        let f = [
            TypedField::new(0, 0x9999_9999),
            TypedField::new(2, 0x7FFF_FFFF),
            TypedField::new(1, 0x0000_0102),
            TypedField::new(1, 0x5555_5555),
            TypedField::new(2, 0x6666_6666),
            TypedField::new(2, 0x7777_7777),
            TypedField::new(1, 0x0000_000A),
            TypedField::new(1, 7),
            TypedField::new(2, 0),
        ];
        let p = resolve_menu_setting(&f, no_resolve);
        assert_eq!(p.p590, 0x6666_6666, "0x590 = 1717986918");
        assert_eq!(p.p594, 0, "0x594 = 0 (tag 0)");
        assert_eq!(p.p598, 0x7FFF_FFFF, "0x598 = 2147483647");
        assert_eq!(p.p59c, 0x5555_5555, "0x59c = 1431655765");
        assert_eq!(p.p5a0, 0x7777_7777, "0x5a0 = 2004318071");
        assert_eq!(p.p5a4, 10, "0x5a4 = 10");
        assert_eq!(p.p5a8, 7.0, "0x5a8 = 7.0 (int→float signé)");
        assert_eq!(p.p5b4, 0x02, "0x5b4 = 2 (octet bas)");
        assert!(!p.p5ba, "0x5ba = false");
    }

    /// Le chemin tag `4` à garde non nulle appelle bien `resolve_ref` (résolution externe).
    #[test]
    fn tag4_guard_set_invokes_resolver() {
        let f = [
            TypedField::default(),
            TypedField::default(),
            TypedField::default(),
            TypedField::new(4, 0x0000_0001), // garde non nulle → résout
            TypedField::default(),
            TypedField::default(),
            TypedField::default(),
            TypedField::default(),
            TypedField::default(),
        ];
        let p = resolve_menu_setting(&f, |field| {
            assert_eq!(field.tag, 4);
            0x0BAD_F00D_u32 as i32
        });
        assert_eq!(
            p.p59c, 0x0BAD_F00D_u32 as i32,
            "résolveur appelé pour le champ 3"
        );
    }
}
