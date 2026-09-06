//! Calcul de viewport **letterbox/pillarbox** au ratio cible — port **byte-exact** de
//! `FUN_14045ff50` (@`0x14045ff50`, `nie_eacpatched.exe`, ~35 callers). Logique de rendu :
//! ajuste un rectangle de viewport pour respecter un ratio d'aspect cible sur un affichage donné.
//!
//! Étant donné les dimensions d'affichage `disp_w × disp_h` et un ratio `(num, den)`, calcule
//! `scaled = disp_w * den / num` (arithmétique **u32**, troncature entière) :
//! - `scaled == disp_h` → aucun ajustement (le binaire n'écrit rien) → [`None`] ;
//! - `scaled < disp_h`  → **letterbox** (barres horizontales) : `x=0, y=(disp_h-scaled)/2,
//!   w=disp_w, h=scaled` ;
//! - `scaled > disp_h`  → **pillarbox** (barres verticales) : `new_w = disp_h*num/den`,
//!   `x=(disp_w-new_w)/2, y=0, w=new_w, h=disp_h`.
//!
//! Le centrage `/2` est un décalage **non signé** `>> 1` sur la soustraction u32 (le binaire
//! n'utilise pas de saturation). Validation : oracle **uemu byte-exact**
//! (`scripts/validate_aspect_viewport.py`, 511 cas — ratios réels 16:9/4:3/21:9 + fuzz).

/// Rectangle de viewport ajusté (champs `+0x28a0..+0x28ac` du binaire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AspectViewport {
    /// Décalage X (`+0x28a0`).
    pub x: u32,
    /// Décalage Y (`+0x28a4`).
    pub y: u32,
    /// Largeur (`+0x28a8`).
    pub w: u32,
    /// Hauteur (`+0x28ac`).
    pub h: u32,
}

/// Viewport letterbox/pillarbox pour `disp_w × disp_h` au ratio `(num, den)` — port byte-exact de
/// `FUN_14045ff50`. `None` si aucun ajustement n'est requis (le ratio tombe juste), auquel cas le
/// binaire ne touche pas les champs de viewport. `num`/`den` doivent être non nuls (division).
#[must_use]
pub fn compute_aspect_viewport(
    disp_w: u32,
    disp_h: u32,
    num: u32,
    den: u32,
) -> Option<AspectViewport> {
    let scaled = disp_w.wrapping_mul(den) / num; // uVar3 (u32, troncature)
    if scaled == disp_h {
        return None; // pas d'ajustement → champs non écrits
    }
    if scaled < disp_h {
        // letterbox : barres horizontales, contenu centré verticalement.
        Some(AspectViewport {
            x: 0,
            y: (disp_h.wrapping_sub(scaled)) >> 1,
            w: disp_w,
            h: scaled,
        })
    } else {
        // pillarbox : barres verticales, contenu centré horizontalement.
        let new_w = disp_h.wrapping_mul(num) / den;
        Some(AspectViewport {
            x: (disp_w.wrapping_sub(new_w)) >> 1,
            y: 0,
            w: new_w,
            h: disp_h,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ratio cible déjà respecté → aucun ajustement.
    #[test]
    fn exact_ratio_no_adjust() {
        // 1920×1080, ratio 1920:1080 → scaled = 1920*1080/1920 = 1080 == disp_h.
        assert_eq!(compute_aspect_viewport(1920, 1080, 1920, 1080), None);
        assert_eq!(compute_aspect_viewport(100, 100, 1, 1), None);
    }

    /// Letterbox : contenu plus court que l'affichage (barres haut/bas).
    #[test]
    fn letterbox() {
        // 1920×1080 cible 4:3 → scaled = 1920*3/4 = 1440 > 1080 → en fait pillarbox ; choisissons un
        // cas letterbox : 1920×1080 cible 21:9 → scaled = 1920*9/21 = 822 < 1080.
        let v = compute_aspect_viewport(1920, 1080, 21, 9).unwrap();
        assert_eq!(
            v,
            AspectViewport {
                x: 0,
                y: (1080 - 822) / 2,
                w: 1920,
                h: 822
            }
        );
        assert_eq!(v.h, 822); // 1920*9/21 = 822 (troncature)
    }

    /// Pillarbox : contenu plus large (barres gauche/droite).
    #[test]
    fn pillarbox() {
        // 1920×1080 cible 4:3 → scaled = 1920*3/4 = 1440 > 1080 → pillarbox.
        let v = compute_aspect_viewport(1920, 1080, 4, 3).unwrap();
        let new_w = 1080u32 * 4 / 3; // 1440
        assert_eq!(
            v,
            AspectViewport {
                x: (1920 - new_w) / 2,
                y: 0,
                w: new_w,
                h: 1080
            }
        );
        assert_eq!(v.w, 1440);
    }

    /// Troncature entière exacte (pas d'arrondi) sur un cas non divisible.
    #[test]
    fn integer_truncation() {
        // 1000×1000 cible 16:9 → scaled = 1000*9/16 = 562 (562.5 tronqué) < 1000 → letterbox.
        let v = compute_aspect_viewport(1000, 1000, 16, 9).unwrap();
        assert_eq!(v.h, 562, "1000*9/16 = 562 (tronqué)");
        assert_eq!(v.y, (1000 - 562) / 2);
    }
}
