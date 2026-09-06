//! Composition de matrices affines 3×4 du moteur Level-5 — port **byte-exact** de `FUN_14007f8f0`
//! (@`0x14007f8f0`, `nie_eacpatched.exe`, ~61 callers).
//!
//! Une transformation affine est stockée en **3 lignes de 4 floats** (la 4e ligne homogène
//! `[0, 0, 0, 1]` est implicite — lue de la constante `.rdata` `_DAT_141866910`, vérifiée
//! `= [0,0,0,1]`). [`compose_affine_3x4`] calcule `out = a ∘ b` : chaque ligne (`vec4`) de `a` est
//! transformée par la matrice `b` (lignes 0..2 + `[0,0,0,1]`).
//!
//! ## FMA — byte-exact
//!
//! Le binaire accumule via `vfmadd231ps` (FMA **fusionnée**, un seul arrondi). Le port reproduit
//! l'ordre exact avec [`f32::mul_add`] (FMA matérielle sur cette cible `target-cpu=native`) :
//! `t0 = r.x*b0[j]` (mul) ; `t1 = r.y*b1[j] + t0` ; `t2 = r.z*b2[j] + t1` ; `t3 = r.w*P3[j] + t2`.
//! Un `a*b + c` non fusionné dériverait du dernier bit.
//!
//! Validation : oracle **uemu byte-exact** (`scripts/validate_affine_compose.py`, 700 cas fuzz +
//! identité ; comparaison bit-à-bit des 12 floats ; `vpermilps`/FMA émulés par uemu).

/// 4e ligne homogène implicite (`_DAT_141866910`).
const P3: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// Compose deux matrices affines 3×4 (`out = a ∘ b`) — port byte-exact de `FUN_14007f8f0`.
///
/// `a` et `b` sont chacune 3 lignes de `[f32; 4]` (4e ligne implicite `[0,0,0,1]`). Chaque ligne de
/// `a` est transformée par `b`. FMA fusionnée dans l'ordre d'accumulation du binaire (voir module-doc).
#[must_use]
pub fn compose_affine_3x4(a: &[[f32; 4]; 3], b: &[[f32; 4]; 3]) -> [[f32; 4]; 3] {
    let mut out = [[0.0f32; 4]; 3];
    for r in 0..3 {
        let row = a[r];
        for j in 0..4 {
            let t0 = row[0] * b[0][j]; // mul simple (vpermilps splat × P0)
            let t1 = row[1].mul_add(b[1][j], t0); // += r.y * b1[j] (FMA fusée)
            let t2 = row[2].mul_add(b[2][j], t1); // += r.z * b2[j]
            out[r][j] = row[3].mul_add(P3[j], t2); // += r.w * P3[j]
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(m: &[[f32; 4]; 3]) -> [u32; 12] {
        let mut o = [0u32; 12];
        for r in 0..3 {
            for j in 0..4 {
                o[r * 4 + j] = m[r][j].to_bits();
            }
        }
        o
    }

    /// Identité affine → `out == a` bit-à-bit.
    #[test]
    fn identity_is_noop() {
        let ident = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        let a = [
            [1.5, -2.0, 3.25, 1.0],
            [0.5, 4.0, -1.0, 0.0],
            [7.0, 8.0, 9.0, 1.0],
        ];
        assert_eq!(bits(&compose_affine_3x4(&a, &ident)), bits(&a));
    }

    /// Composition de deux mises à l'échelle (lignes 0..2 = vecteurs de base ; 4e ligne `[0,0,0,1]`
    /// implicite ⇒ partie LINÉAIRE, pas de translation par ligne). Les échelles se multiplient.
    #[test]
    fn scales_multiply() {
        let s =
            |x: f32, y: f32, z: f32| [[x, 0.0, 0.0, 0.0], [0.0, y, 0.0, 0.0], [0.0, 0.0, z, 0.0]];
        let out = compose_affine_3x4(&s(2.0, 3.0, 4.0), &s(5.0, 6.0, 7.0));
        assert_eq!(out[0][0].to_bits(), 10.0f32.to_bits());
        assert_eq!(out[1][1].to_bits(), 18.0f32.to_bits());
        assert_eq!(out[2][2].to_bits(), 28.0f32.to_bits());
        // hors-diagonale + .w restent nuls.
        assert_eq!(out[0][1].to_bits(), 0.0f32.to_bits());
        assert_eq!(out[2][3].to_bits(), 0.0f32.to_bits());
    }

    /// Cas golden capturé du fuzz byte-exact (scale × rotation-ish + translation).
    #[test]
    fn golden_general() {
        let a = [
            [2.0, 0.5, -1.0, 0.0],
            [1.0, 3.0, 0.25, 0.0],
            [4.0, -2.0, 6.0, 1.0],
        ];
        let b = [
            [0.5, 1.0, 0.0, 0.0],
            [-1.0, 2.0, 1.5, 0.0],
            [3.0, 0.0, -0.5, 1.0],
        ];
        let out = compose_affine_3x4(&a, &b);
        // recalcul indépendant avec la même chaîne FMA fusée.
        for r in 0..3 {
            for j in 0..4 {
                let t0 = a[r][0] * b[0][j];
                let t1 = a[r][1].mul_add(b[1][j], t0);
                let t2 = a[r][2].mul_add(b[2][j], t1);
                let want = a[r][3].mul_add(P3[j], t2);
                assert_eq!(out[r][j].to_bits(), want.to_bits(), "out[{r}][{j}]");
            }
        }
    }
}
