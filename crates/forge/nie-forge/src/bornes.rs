//! Validation des **bornes de fonctions** venues de l'échafaudage RE.
//!
//! `.pdata` ne décrit que les fonctions pourvues de données de déroulement ; le
//! reste du `.text` est découpé grâce aux feuilles mesurées par `nie_re::recover`
//! (table `function` de la base). Ces feuilles font gagner beaucoup — sans elles
//! 1,8 Mo de `.text` reste haché et non relevable — mais elles ne sont pas
//! infaillibles : **une borne peut tomber au milieu d'une instruction**.
//!
//! Une unité ainsi coupée est perdue deux fois. Le relevé la rejette, parce que
//! ses premiers octets sont la queue de l'instruction précédente et que le
//! ré-encodage ne les retrouve pas (`nie-forge lift` les compte alors sous des
//! causes trompeuses comme `encodage:mov` ou `encodage:lea`, qui accusent
//! l'encodeur au lieu de la découpe) ; et la classification hérite d'un offset
//! faux.
//!
//! Le filtre appliqué ici est celui du désassembleur, pas une heuristique :
//! une feuille n'est retenue que si une instruction **commence exactement** à
//! son adresse, en désassemblant depuis un point d'ancrage sûr qui la précède.

use iced_x86::{Decoder, DecoderOptions};
use nie_pe::PeImage;

/// Octet de bourrage du linker MSVC : sa fin est une frontière sûre.
const INT3: u8 = 0xCC;

/// Distance maximale de recherche d'un point d'ancrage sûr, en octets.
///
/// Au-delà, aucune conclusion n'est tirée et la feuille est conservée : le
/// filtre ne retire que ce qu'il sait faux, jamais ce qu'il n'a pas su juger.
const FENETRE: usize = 4096;

/// Ce que la validation a écarté, et pourquoi c'est chiffré.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Verdict {
    /// Feuilles soumises au filtre.
    pub soumises: usize,
    /// Feuilles retenues.
    pub retenues: usize,
    /// Feuilles tombant au milieu d'une instruction, donc écartées.
    pub coupantes: usize,
    /// Feuilles conservées faute d'ancrage sûr dans la fenêtre.
    pub indecises: usize,
    /// Octets couverts par les feuilles écartées.
    pub octets_ecartes: usize,
}

/// Vrai si une instruction commence exactement à `cible`, en désassemblant
/// linéairement depuis `ancre`.
fn tombe_sur_une_frontiere(code: &[u8], base_va: u64, ancre: usize, cible: usize) -> bool {
    let mut dec = Decoder::with_ip(
        64,
        &code[ancre..cible.min(code.len())],
        base_va + ancre as u64,
        DecoderOptions::NONE,
    );
    let attendu = base_va + cible as u64;
    let mut ip = dec.ip();
    while ip < attendu {
        if !dec.can_decode() {
            return false;
        }
        let insn = dec.decode();
        if insn.is_invalid() {
            return false;
        }
        ip = dec.ip();
    }
    ip == attendu
}

/// Cherche en arrière le point d'ancrage sûr le plus proche de `cible`.
///
/// Deux ancrages font foi : la fin d'un run de bourrage `int3` — MSVC ne coupe
/// jamais une instruction en deux avec du bourrage — et le début d'une racine
/// `.pdata`, qui est une borne de fonction vérifiée.
fn ancre_sure(code: &[u8], cible: usize, racines: &[usize]) -> Option<usize> {
    let plancher = cible.saturating_sub(FENETRE);
    let mut pos = cible;
    while pos > plancher {
        if code[pos - 1] == INT3 {
            return Some(pos);
        }
        pos -= 1;
    }
    // Dernière racine `.pdata` située dans la fenêtre.
    racines
        .binary_search(&cible)
        .map_or_else(|i| i.checked_sub(1), Some)
        .map(|i| racines[i])
        .filter(|&r| r >= plancher && r < cible)
}

/// Écarte les feuilles RE dont l'adresse ne tombe pas sur une frontière
/// d'instruction.
///
/// Les feuilles sont supposées triées par adresse (c'est ce que rend la base).
/// Une feuille hors des sections exécutables, ou dont l'ancrage sûr n'a pas été
/// trouvé, est conservée telle quelle.
#[must_use]
pub fn valider(img: &PeImage, feuilles: &[(u64, u32)]) -> (Vec<(u64, u32)>, Verdict) {
    let mut v = Verdict {
        soumises: feuilles.len(),
        ..Verdict::default()
    };

    // Bornes de fonction vérifiées : les débuts de racines `.pdata`, en offsets
    // fichier et triés, pour la recherche dichotomique de l'ancrage.
    let (ranges, _) = nie_pe::pdata::scan(img);
    let mut racines: Vec<usize> = nie_pe::pdata::merge(&ranges)
        .iter()
        .filter_map(|r| img.rva_to_offset(r.begin))
        .collect();
    racines.sort_unstable();

    let code = &img.bytes;
    let mut retenues = Vec::with_capacity(feuilles.len());
    for &(va, len) in feuilles {
        let Some(off) = img.va_to_offset(va) else {
            retenues.push((va, len));
            v.retenues += 1;
            continue;
        };
        // Le bourrage qui précède clôt l'instruction d'avant : frontière sûre.
        if off == 0 || code[off - 1] == INT3 {
            retenues.push((va, len));
            v.retenues += 1;
            continue;
        }
        let Some(ancre) = ancre_sure(code, off, &racines) else {
            retenues.push((va, len));
            v.retenues += 1;
            v.indecises += 1;
            continue;
        };
        // `base_va` du tampon fichier : l'ancre et la cible sont des offsets,
        // seule leur différence compte pour le décodage.
        if tombe_sur_une_frontiere(code, img.opt.image_base, ancre, off) {
            retenues.push((va, len));
            v.retenues += 1;
        } else {
            v.coupantes += 1;
            v.octets_ecartes += len as usize;
        }
    }
    (retenues, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `mov [rcx+off], rax` répété : chaque instruction fait 4 octets.
    /// Une borne à +2 tombe au milieu de la seconde.
    fn flux() -> Vec<u8> {
        let mut b = vec![INT3; 8];
        for off in [0x38u8, 0x40, 0x50, 0x58] {
            b.extend_from_slice(&[0x48, 0x89, 0x41, off]);
        }
        b
    }

    #[test]
    fn une_borne_au_milieu_d_une_instruction_est_detectee() {
        let code = flux();
        // Ancre = 8 (fin du bourrage). Frontières réelles : 8, 12, 16, 20.
        assert!(tombe_sur_une_frontiere(&code, 0x1_4000_0000, 8, 12));
        assert!(tombe_sur_une_frontiere(&code, 0x1_4000_0000, 8, 16));
        assert!(
            !tombe_sur_une_frontiere(&code, 0x1_4000_0000, 8, 14),
            "+2 tombe au milieu du mov, comme la feuille fautive de fn.1401f4320"
        );
    }

    #[test]
    fn le_bourrage_est_un_ancrage_sur() {
        let code = flux();
        assert_eq!(
            ancre_sure(&code, 8, &[]),
            Some(8),
            "juste après le bourrage"
        );
        assert_eq!(
            ancre_sure(&code, 14, &[]),
            Some(8),
            "remonte jusqu'au bourrage"
        );
    }

    #[test]
    fn une_racine_pdata_sert_d_ancrage_quand_le_bourrage_manque() {
        let code = vec![0x48, 0x89, 0x41, 0x38, 0x48, 0x89, 0x41, 0x40];
        assert_eq!(ancre_sure(&code, 6, &[]), None, "aucun ancrage disponible");
        assert_eq!(ancre_sure(&code, 6, &[0]), Some(0), "la racine ancre");
    }
}
