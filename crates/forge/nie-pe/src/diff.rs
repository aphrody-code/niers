//! Comparaison byte-à-byte de deux images, et rapport lisible.
//!
//! La forge n'a qu'un seul critère de succès : `sha256(généré) == sha256(original)`.
//! Quand ce n'est pas le cas, il faut savoir **où** et **combien** — d'où ce module,
//! qui condense les différences en plages contiguës plutôt qu'en octets isolés.

/// Une plage d'octets divergente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffRange {
    /// Offset du premier octet divergent.
    pub off: usize,
    /// Longueur de la plage divergente.
    pub len: usize,
}

/// Résultat d'une comparaison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffReport {
    /// Taille de la référence.
    pub len_ref: usize,
    /// Taille du candidat.
    pub len_got: usize,
    /// Nombre total d'octets divergents.
    pub bytes_differing: usize,
    /// Plages divergentes (tronquées à `max_ranges`).
    pub ranges: Vec<DiffRange>,
    /// Vrai si des plages ont été omises du rapport.
    pub truncated: bool,
}

impl DiffReport {
    /// Vrai si les deux tampons sont identiques.
    #[must_use]
    pub fn is_identical(&self) -> bool {
        self.len_ref == self.len_got && self.bytes_differing == 0
    }

    /// Proportion d'octets identiques sur la référence, dans `[0, 1]`.
    #[must_use]
    pub fn ratio_identical(&self) -> f64 {
        if self.len_ref == 0 {
            return 1.0;
        }
        let ok = self.len_ref.saturating_sub(self.bytes_differing);
        ok as f64 / self.len_ref as f64
    }
}

/// Compare deux tampons et condense les divergences.
#[must_use]
pub fn compare(reference: &[u8], got: &[u8], max_ranges: usize) -> DiffReport {
    let n = reference.len().min(got.len());
    let mut ranges: Vec<DiffRange> = Vec::new();
    let mut bytes_differing = 0usize;
    let mut truncated = false;
    let mut i = 0usize;
    while i < n {
        if reference[i] == got[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && reference[i] != got[i] {
            i += 1;
        }
        bytes_differing += i - start;
        if ranges.len() < max_ranges {
            ranges.push(DiffRange {
                off: start,
                len: i - start,
            });
        } else {
            truncated = true;
        }
    }
    // La partie excédentaire d'un tampon plus long compte comme divergente.
    bytes_differing += reference.len().abs_diff(got.len());
    DiffReport {
        len_ref: reference.len(),
        len_got: got.len(),
        bytes_differing,
        ranges,
        truncated,
    }
}

/// Compare deux tampons en ignorant les octets marqués dans `mask`.
///
/// `mask[i] == true` signifie « octet non comparable » (typiquement un champ
/// réécrit par une relocation : l'adresse diffère entre l'objet compilé et
/// l'image liée, mais le code est bien identique).
#[must_use]
pub fn compare_masked(
    reference: &[u8],
    got: &[u8],
    mask: &[bool],
    max_ranges: usize,
) -> DiffReport {
    let n = reference.len().min(got.len());
    let mut masked_ref = reference[..n].to_vec();
    let mut masked_got = got[..n].to_vec();
    for (i, m) in mask.iter().take(n).enumerate() {
        if *m {
            masked_ref[i] = 0;
            masked_got[i] = 0;
        }
    }
    let mut r = compare(&masked_ref, &masked_got, max_ranges);
    r.len_ref = reference.len();
    r.len_got = got.len();
    r.bytes_differing += reference.len().abs_diff(got.len());
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identique() {
        let r = compare(&[1, 2, 3], &[1, 2, 3], 8);
        assert!(r.is_identical());
        assert!((r.ratio_identical() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn condense_les_plages() {
        let r = compare(&[1, 2, 3, 4, 5], &[1, 9, 9, 4, 8], 8);
        assert_eq!(r.bytes_differing, 3);
        assert_eq!(
            r.ranges,
            vec![DiffRange { off: 1, len: 2 }, DiffRange { off: 4, len: 1 }]
        );
    }

    #[test]
    fn tailles_differentes_comptent_comme_divergence() {
        let r = compare(&[1, 2, 3, 4], &[1, 2], 8);
        assert!(!r.is_identical());
        assert_eq!(r.bytes_differing, 2);
    }

    #[test]
    fn le_masque_neutralise_les_champs_relocalises() {
        let reference = [0xE8, 0x11, 0x22, 0x33, 0x44, 0xC3];
        let got = [0xE8, 0x00, 0x00, 0x00, 0x00, 0xC3];
        let mask = [false, true, true, true, true, false];
        assert!(compare_masked(&reference, &got, &mask, 8).is_identical());
        assert!(!compare(&reference, &got, 8).is_identical());
    }
}
