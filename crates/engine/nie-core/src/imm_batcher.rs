//! Assembleur de primitives en **mode immédiat** (style `glBegin`/`glVertex`) — port
//! **byte-exact** de `FUN_14075c030` (@`0x14075c030`, `nie_eacpatched.exe`, 1249 o) + son
//! sous-helper de transform `FUN_14075df00`.
//!
//! ## Sémantique (reversée + validée uemu, 800 cas byte-exact)
//!
//! Chaque sommet entrant `(x, y, z)` est transformé par la matrice 4×4 du batcher (`w = 1`),
//! puis émis selon le **mode** courant dans l'un de trois tampons (points/lignes/triangles),
//! en maintenant un compteur, le sommet courant `cur` et le précédent `prev` :
//! - [`MODE_POINTS`] (0) : émet un point à chaque sommet ;
//! - [`MODE_LINES`] (1) : émet une ligne `(cur, new)` tous les 2 sommets (puis remet le compteur à 0) ;
//! - [`MODE_LINE_STRIP`] (2) : émet une ligne `(cur, new)` dès le 2ᵉ sommet ;
//! - [`MODE_TRIANGLES`] (3) : émet un triangle `(prev, cur, new)` tous les 3 sommets (compteur→0) ;
//! - [`MODE_TRIANGLE_STRIP`] (4) : émet un triangle dès le 3ᵉ sommet, **winding alterné**
//!   (`compteur` pair → `(prev, cur, new)` ; impair → `(cur, prev, new)`).
//!
//! Après émission : si `compteur > 1`, `prev ← cur` ; puis `cur ← new`. La couleur du batcher
//! (`+0x04`) est dupliquée sur chaque sommet émis. Seul le **fast-path** (capacité suffisante,
//! pas de réallocation) est porté ; la croissance des `std::vector` est hors périmètre.
//!
//! L'ordre d'accumulation de [`transform_vertex`] est tiré du code machine (`mulss`/`addss` en
//! **arbre** : `(x·m0 + z·m8) + (y·m4 + w·m12)`, **pas** de FMA) → byte-exact sur les bits f32.
//!
//! Validation : `scripts/validate_imm_batcher.py` (oracle uemu, fuzz seedé matrices/sommets/modes,
//! compare sommet transformé + état 0x70 + compteurs conteneur + tampons émis).

extern crate alloc;
use alloc::vec::Vec;

/// Mode points.
pub const MODE_POINTS: u32 = 0;
/// Mode lignes (paires disjointes).
pub const MODE_LINES: u32 = 1;
/// Mode bande de lignes.
pub const MODE_LINE_STRIP: u32 = 2;
/// Mode triangles (triplets disjoints).
pub const MODE_TRIANGLES: u32 = 3;
/// Mode bande de triangles (winding alterné).
pub const MODE_TRIANGLE_STRIP: u32 = 4;

/// Transforme un point `(x, y, z)` par la matrice 4×4 row-major `m` avec `w = 1` — port byte-exact
/// de `FUN_14075df00`. Renvoie les 3 composantes spatiales (la 4ᵉ n'est pas écrite par l'appelant).
///
/// Ordre d'accumulation en arbre, sans fused-multiply-add (chaque `·` est un `mulss` arrondi,
/// chaque `+` un `addss` arrondi) : à reproduire tel quel pour rester bit-pour-bit identique.
#[must_use]
pub fn transform_vertex(m: &[f32; 16], v: [f32; 3]) -> [f32; 3] {
    let [x, y, z] = v;
    let w = 1.0_f32;
    let o0 = (x * m[0] + z * m[8]) + (y * m[4] + w * m[12]);
    let o1 = (z * m[9] + w * m[13]) + (x * m[1] + y * m[5]);
    let o2 = (z * m[10] + w * m[14]) + (x * m[2] + y * m[6]);
    [o0, o1, o2]
}

/// Un sommet émis : position transformée + couleur dupliquée.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Position transformée `(x, y, z)`.
    pub pos: [f32; 3],
    /// Couleur (`+0x04` du batcher), dupliquée sur le sommet.
    pub color: u32,
}

/// État d'un batcher de primitives en mode immédiat.
#[derive(Debug, Clone, Default)]
pub struct ImmediateBatcher {
    /// Mode courant (voir `MODE_*`).
    pub mode: u32,
    /// Couleur courante.
    pub color: u32,
    /// Matrice 4×4 row-major appliquée à chaque sommet.
    pub matrix: [f32; 16],
    /// Sommet courant (dernier transformé).
    pub cur: [f32; 3],
    /// Sommet précédent.
    pub prev: [f32; 3],
    /// Compteur de sommets de la primitive en cours.
    pub count: u32,
    /// Tampon de points émis.
    pub points: Vec<Vertex>,
    /// Tampon de lignes émises (2 sommets chacune).
    pub lines: Vec<[Vertex; 2]>,
    /// Tampon de triangles émis (3 sommets chacun).
    pub triangles: Vec<[Vertex; 3]>,
}

impl ImmediateBatcher {
    /// Soumet un sommet `(x, y, z)` : le transforme, émet la primitive selon le mode, met à jour
    /// l'état. Renvoie le sommet **transformé** (comme le binaire réécrit `param_2`). Port
    /// byte-exact de `FUN_14075c030` (fast-path, sans réallocation).
    pub fn vertex(&mut self, v: [f32; 3]) -> [f32; 3] {
        let new = transform_vertex(&self.matrix, v);
        let count = self.count.wrapping_add(1);
        let mut state_count = count;
        let color = self.color;
        let cur = self.cur;
        let prev = self.prev;
        match self.mode {
            MODE_POINTS => self.points.push(Vertex { pos: new, color }),
            MODE_LINES if count == 2 => {
                self.lines
                    .push([Vertex { pos: cur, color }, Vertex { pos: new, color }]);
                state_count = 0;
            }
            MODE_LINE_STRIP if count > 1 => {
                self.lines
                    .push([Vertex { pos: cur, color }, Vertex { pos: new, color }]);
            }
            MODE_TRIANGLES if count == 3 => {
                self.triangles.push([
                    Vertex { pos: prev, color },
                    Vertex { pos: cur, color },
                    Vertex { pos: new, color },
                ]);
                state_count = 0;
            }
            MODE_TRIANGLE_STRIP if count > 2 => {
                // winding alterné : compteur pair → (prev, cur) ; impair → (cur, prev)
                let (v0, v1) = if count & 1 == 0 {
                    (prev, cur)
                } else {
                    (cur, prev)
                };
                self.triangles.push([
                    Vertex { pos: v0, color },
                    Vertex { pos: v1, color },
                    Vertex { pos: new, color },
                ]);
            }
            // modes inconnus, ou seuil de comptage non atteint : pas d'émission (état mis à jour ensuite)
            _ => {}
        }
        if state_count > 1 {
            self.prev = self.cur;
        }
        self.cur = new;
        self.count = state_count;
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Matrice de test (row-major) : scale (2,3,4) + translation (10,20,30) via la 4ᵉ ligne.
    const M: [f32; 16] = [
        2.0, 0.0, 0.0, 0.0, //
        0.0, 3.0, 0.0, 0.0, //
        0.0, 0.0, 4.0, 0.0, //
        10.0, 20.0, 30.0, 1.0,
    ];
    const COLOR: u32 = 0xFF00_FF00;
    const CUR: [f32; 3] = [1.0, 2.0, 3.0];
    const PREV: [f32; 3] = [4.0, 5.0, 6.0];

    fn bits3(p: [f32; 3]) -> [u32; 3] {
        [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]
    }

    fn mk(mode: u32, count: u32) -> ImmediateBatcher {
        ImmediateBatcher {
            mode,
            color: COLOR,
            matrix: M,
            cur: CUR,
            prev: PREV,
            count,
            ..Default::default()
        }
    }

    #[test]
    fn transform_golden() {
        // (1,1,1) → (12, 23, 34) (cf. oracle uemu).
        let o = transform_vertex(&M, [1.0, 1.0, 1.0]);
        assert_eq!(bits3(o), [0x4140_0000, 0x41B8_0000, 0x4208_0000]);
    }

    #[test]
    fn points_emits_each() {
        let mut b = mk(MODE_POINTS, 0);
        b.vertex([1.0, 1.0, 1.0]);
        assert_eq!(b.points.len(), 1);
        assert_eq!(
            bits3(b.points[0].pos),
            [0x4140_0000, 0x41B8_0000, 0x4208_0000]
        );
        assert_eq!(b.points[0].color, COLOR);
        assert_eq!(b.count, 1);
        assert_eq!(bits3(b.cur), [0x4140_0000, 0x41B8_0000, 0x4208_0000]);
        assert_eq!(bits3(b.prev), bits3(PREV)); // count==1, prev inchangé
    }

    #[test]
    fn lines_emits_on_second() {
        let mut b = mk(MODE_LINES, 1); // count → 2
        b.vertex([1.0, 1.0, 1.0]);
        assert_eq!(b.lines.len(), 1);
        assert_eq!(bits3(b.lines[0][0].pos), bits3(CUR)); // 1er sommet = cur
        assert_eq!(
            bits3(b.lines[0][1].pos),
            [0x4140_0000, 0x41B8_0000, 0x4208_0000]
        ); // 2e = new
        assert_eq!(b.count, 0); // compteur remis à 0
        assert_eq!(bits3(b.prev), bits3(PREV)); // state_count==0, prev inchangé
    }

    #[test]
    fn line_strip_emits_and_advances_prev() {
        let mut b = mk(MODE_LINE_STRIP, 1); // count → 2
        b.vertex([1.0, 1.0, 1.0]);
        assert_eq!(b.lines.len(), 1);
        assert_eq!(b.count, 2);
        assert_eq!(bits3(b.prev), bits3(CUR)); // state_count==2 > 1 → prev = ancien cur
    }

    #[test]
    fn triangles_emits_on_third() {
        let mut b = mk(MODE_TRIANGLES, 2); // count → 3
        b.vertex([1.0, 1.0, 1.0]);
        assert_eq!(b.triangles.len(), 1);
        assert_eq!(bits3(b.triangles[0][0].pos), bits3(PREV));
        assert_eq!(bits3(b.triangles[0][1].pos), bits3(CUR));
        assert_eq!(
            bits3(b.triangles[0][2].pos),
            [0x4140_0000, 0x41B8_0000, 0x4208_0000]
        );
        assert_eq!(b.count, 0);
    }

    #[test]
    fn triangle_strip_winding() {
        // compteur pair (count_in 3 → 4) : (prev, cur, new)
        let mut be = mk(MODE_TRIANGLE_STRIP, 3);
        be.vertex([1.0, 1.0, 1.0]);
        assert_eq!(bits3(be.triangles[0][0].pos), bits3(PREV));
        assert_eq!(bits3(be.triangles[0][1].pos), bits3(CUR));
        // compteur impair (count_in 4 → 5) : (cur, prev, new)
        let mut bo = mk(MODE_TRIANGLE_STRIP, 4);
        bo.vertex([1.0, 1.0, 1.0]);
        assert_eq!(bits3(bo.triangles[0][0].pos), bits3(CUR));
        assert_eq!(bits3(bo.triangles[0][1].pos), bits3(PREV));
    }
}
