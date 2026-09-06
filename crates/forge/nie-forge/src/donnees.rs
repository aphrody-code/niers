//! Détection des **données déposées au milieu du code**.
//!
//! MSVC place les tables de sauts, les constantes vectorielles alignées et les
//! littéraux flottants entre les instructions qui les consomment, à l'intérieur
//! même du corps des fonctions. Tant que ces octets restent soudés au corps,
//! le désassemblage linéaire bute dessus et **toute** la fonction devient
//! irrelevable : `nie-forge lift` la range alors sous la cause `invalide`.
//!
//! Ce que ça coûtait, mesuré sur `nie.exe` : 998 unités et 1 034 147 octets
//! refusés, dont **990 445 octets (95,8 %) de code parfaitement décodable**
//! bloqués par 39 968 octets de données — 3,9 %.
//!
//! La détection est purement mécanique : on désassemble, on note où ça casse,
//! et on cherche l'offset à partir duquel le reste se décode de nouveau. Rien
//! n'est deviné, et rien n'est prétendu produit : les plages trouvées deviennent
//! des unités [`nie_pe::UnitKind::InlineData`], comptées comme des données. Le
//! seul effet recherché est de rendre relevable le code qui les encadre — et
//! ce relevé, lui, reste soumis au ré-encodage byte-exact.

use iced_x86::{Decoder, DecoderOptions};
use nie_pe::{Cover, PeImage, UnitKind};

/// Pas de recherche de la reprise, en octets.
///
/// Les tables de sauts et les constantes vectorielles sont alignées ; chercher
/// à chaque octet multiplierait le coût sans rien trouver de plus.
const PAS: usize = 4;

/// Ce que la détection a trouvé, et ce qu'elle libère.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Bilan {
    /// Unités de code que le désassemblage refuse en l'état.
    pub unites: usize,
    /// Octets de ces unités, toutes natures confondues.
    pub octets: usize,
    /// Octets identifiés comme des données.
    pub donnees: usize,
    /// Octets de code que l'isolation rend décodables.
    pub code_libere: usize,
    /// Unités où le code reprend après les données.
    pub sandwichs: usize,
}

/// Longueur décodable depuis le début du tampon.
fn decodable(code: &[u8], va: u64) -> usize {
    let mut dec = Decoder::with_ip(64, code, va, DecoderOptions::NONE);
    let mut pos = 0usize;
    while dec.can_decode() {
        let insn = dec.decode();
        if insn.is_invalid() {
            return pos;
        }
        pos += insn.len();
    }
    pos
}

/// Premier offset `>= depuis` à partir duquel **tout** le reste se décode.
fn reprise(code: &[u8], va: u64, depuis: usize) -> Option<usize> {
    let mut off = depuis;
    while off < code.len() {
        if decodable(&code[off..], va + off as u64) == code.len() - off {
            return Some(off);
        }
        off += PAS;
    }
    None
}

/// Recense les plages de données inline `(RVA, longueur)` d'un recouvrement.
///
/// Les plages rendues sont triées et prêtes pour
/// [`nie_pe::Cover::split_with_data`].
#[must_use]
pub fn detecter(img: &PeImage, cover: &Cover) -> (Vec<(u32, u32)>, Bilan) {
    let mut plages = Vec::new();
    let mut b = Bilan::default();

    for u in &cover.units {
        if !matches!(u.kind, UnitKind::Function | UnitKind::CodeResidue) {
            continue;
        }
        let (Some(va), Some(corps)) = (u.va, img.bytes.get(u.range())) else {
            continue;
        };
        let n = decodable(corps, va);
        if n == corps.len() {
            continue;
        }
        b.unites += 1;
        b.octets += corps.len();
        b.code_libere += n;

        let (d0, d1) = match reprise(corps, va, n) {
            Some(r) => {
                b.sandwichs += 1;
                b.code_libere += corps.len() - r;
                (n, r)
            }
            None => (n, corps.len()),
        };
        b.donnees += d1 - d0;
        let Some(rva) = va.checked_sub(img.opt.image_base) else {
            continue;
        };
        if let (Ok(debut), Ok(len)) = (u32::try_from(rva + d0 as u64), u32::try_from(d1 - d0)) {
            plages.push((debut, len));
        }
    }
    plages.sort_unstable();
    (plages, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesure_la_part_decodable_avant_la_casse() {
        // `xor eax,eax` ; `ret` ; puis un opcode que rien ne decode.
        let code = vec![0x31, 0xc0, 0xc3, 0xff, 0xff];
        assert_eq!(decodable(&code, 0x1_4000_0000), 3);
    }

    #[test]
    fn trouve_la_reprise_apres_les_donnees() {
        // code (4 o) | donnees (4 o) | code (4 o)
        let mut code = vec![0x31, 0xc0, 0x31, 0xc9]; // xor eax,eax ; xor ecx,ecx
        code.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
        code.extend_from_slice(&[0x31, 0xc0, 0x31, 0xc9]);
        let n = decodable(&code, 0x1_4000_0000);
        assert_eq!(n, 4);
        assert_eq!(reprise(&code, 0x1_4000_0000, n), Some(8));
    }

    #[test]
    fn sans_reprise_rend_none() {
        let code = vec![0x31, 0xc0, 0xff, 0xff, 0xff];
        let n = decodable(&code, 0x1_4000_0000);
        assert_eq!(reprise(&code, 0x1_4000_0000, n), None);
    }
}
