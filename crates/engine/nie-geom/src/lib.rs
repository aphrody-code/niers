//! **nie-geom** — types géométriques POD partagés du workspace niers (dédup Phase 2).
//!
//! `Vec2`/`Vec3` sont des conteneurs `[f32]` **axis-agnostiques** : le système de coordonnées
//! (quel axe est « vertical ») vit dans le CODE de chaque consommateur, PAS dans le type.
//!
//! ⚠ **Landmine #4** (cf. `docs/ARCHITECTURE.md`) : `nie-core` traite `y` comme hauteur, `nie-runtime`
//! traite `z` comme hauteur. Le type unifié ne change RIEN à cela (chaque crate garde sa convention
//! dans son code). Mais **ne jamais convertir implicitement** un `Vec3` d'un système vers l'autre :
//! la similarité de layout ne vaut pas équivalence sémantique.
//!
//! `no_std` par défaut hors feature `std` : les méthodes à `f32::sqrt`/`hypot`
//! (`length`/`normalize`/`len`/`norm`) sont gatées `#[cfg(feature = "std")]`.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

/// Vecteur 2D `[f32; 2]` (plan). Sémantique des axes = celle du consommateur.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    /// Construit un vecteur 2D.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Norme euclidienne.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }

    /// Vecteur unitaire (ou zéro si longueur nulle).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn norm(self) -> Self {
        let l = self.len();
        if l > 1e-6 {
            self * (1.0 / l)
        } else {
            Self::default()
        }
    }
}

impl core::ops::Add for Vec2 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
}

impl core::ops::Sub for Vec2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
}

impl core::ops::Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s)
    }
}

/// Vecteur 3D `[f32; 3]`. Sémantique des axes = celle du consommateur (cf. landmine #4 ci-dessus).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    /// Vecteur nul.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// Construit un vecteur 3D.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Composante X.
    #[must_use]
    pub fn x(self) -> f32 {
        self.x
    }
    /// Composante Y.
    #[must_use]
    pub fn y(self) -> f32 {
        self.y
    }
    /// Composante Z.
    #[must_use]
    pub fn z(self) -> f32 {
        self.z
    }

    /// Projection sur le plan `(x, y)`.
    #[must_use]
    pub const fn ground(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    /// Longueur euclidienne au carré (sans racine, pour comparaisons).
    #[must_use]
    pub fn length_sq(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Longueur euclidienne.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }

    /// Normalise (retourne `zero()` si norme < epsilon).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len < f32::EPSILON {
            return Self::zero();
        }
        Self {
            x: self.x / len,
            y: self.y / len,
            z: self.z / len,
        }
    }

    /// Normalisation **byte-fidèle au jeu** (`game::BallMoveRate::vmethod_3`, plage `0x141339484`,
    /// validée byte-exact via l'oracle uemu — `scripts/validate_normalize.py`). Renvoie `(dir, len)`.
    ///
    /// Diffère de [`Vec3::normalize`] (générique) sur deux points **byte-significatifs** mesurés
    /// dans le binaire :
    /// - réciproque-puis-produit : `dir = v · (1/len)` (un `divss` puis `mulps`), **pas** `v / len`
    ///   (trois `div`) — l'arrondi f32 diffère ;
    /// - garde `len > 0` (`comiss`/`jbe`), **pas** `len < EPSILON`.
    ///
    /// `len` = `sqrt(length_sq)` (= ordre `(x²+y²)+(z²+0)` du binaire, le `+0` étant un no-op).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn normalize_game(self) -> (Self, f32) {
        let len = self.length();
        if len > 0.0 {
            let inv = 1.0 / len; // réciproque unique (divss), puis produit (mulps) — fidèle au binaire
            (
                Self {
                    x: self.x * inv,
                    y: self.y * inv,
                    z: self.z * inv,
                },
                len,
            )
        } else {
            (Self::zero(), len)
        }
    }

    /// Vitesse **byte-fidèle au jeu** depuis la position précédente (`game::BallMoveDribble::vmethod_3`,
    /// plage `0x14133AA69`, validée byte-exact via uemu — `scripts/validate_dribble_vel.py`) :
    /// `delta = self − prev` ; si `dt > 0`, `delta · (1/dt)` (réciproque `divss` PUIS produit `mulps`,
    /// pas `delta / dt`) ; sinon `delta`. No_std (pas de `sqrt`).
    #[must_use]
    pub fn displacement_rate(self, prev: Self, dt: f32) -> Self {
        let delta = Self {
            x: self.x - prev.x,
            y: self.y - prev.y,
            z: self.z - prev.z,
        };
        if dt > 0.0 {
            let inv = 1.0 / dt;
            Self {
                x: delta.x * inv,
                y: delta.y * inv,
                z: delta.z * inv,
            }
        } else {
            delta
        }
    }

    /// Évaluation d'une courbe de Bézier **quadratique** (3 points de contrôle) par de Casteljau,
    /// **byte-fidèle au jeu** (`game::BallMoveBezier`, `FUN_1413359b0`, validée byte-exact via uemu —
    /// `scripts/validate_bezier.py`). `B(t) = lerp(lerp(p1,p2,t), lerp(p2,p3,t), t)` où chaque lerp
    /// est un **FMA fusionné** `(b − a)·t + a` via [`f32::mul_add`] (= `vfmadd231ps`, arrondi unique).
    /// Le `mul_add` est indispensable à la fidélité bit-à-bit (≠ `(b−a)*t + a` en deux arrondis).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn bezier_quadratic(p1: Self, p2: Self, p3: Self, t: f32) -> Self {
        let lerp = |a: Self, b: Self| Self {
            x: (b.x - a.x).mul_add(t, a.x),
            y: (b.y - a.y).mul_add(t, a.y),
            z: (b.z - a.z).mul_add(t, a.z),
        };
        lerp(lerp(p1, p2), lerp(p2, p3))
    }

    /// Réflexion **byte-fidèle au jeu** d'un vecteur sur une surface de normale `n` (collision de
    /// `game::BallMoveNormal`, `FUN_14133ae10` `0x14133B829`, validée byte-exact via uemu —
    /// `scripts/validate_reflect.py`) : `self − 2·(self·n)·n`. Produit scalaire en ordre `haddps`
    /// `(x·nx + y·ny) + (z·nz + 0)`, doublé via `addss`, diffusé, multiplié par `n`, soustrait.
    /// `n` est supposé unitaire (la normale de collision).
    #[must_use]
    pub fn reflect(self, n: Self) -> Self {
        let dot = (self.x * n.x + self.y * n.y) + (self.z * n.z + 0.0);
        let two_dot = dot + dot;
        Self {
            x: self.x - two_dot * n.x,
            y: self.y - two_dot * n.y,
            z: self.z - two_dot * n.z,
        }
    }

    /// Lerp entre `self` et `other` avec poids `t` ∈ [0, 1].
    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
        }
    }
}

impl core::fmt::Display for Vec3 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "({:.3}, {:.3}, {:.3})", self.x, self.y, self.z)
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn vec3_length() {
        let v = Vec3 {
            x: 3.0,
            y: 0.0,
            z: 4.0,
        };
        assert!((v.length() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn vec3_normalize_zero() {
        assert_eq!(Vec3::zero().normalize(), Vec3::zero());
    }

    #[test]
    fn vec3_lerp() {
        let a = Vec3::zero();
        let b = Vec3 {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        };
        let mid = a.lerp(b, 0.5);
        assert!(
            (mid.x - 5.0).abs() < 1e-6
                && (mid.y - 10.0).abs() < 1e-6
                && (mid.z - 15.0).abs() < 1e-6
        );
    }

    #[test]
    fn vec2_ops() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        assert_eq!(a + b, Vec2::new(4.0, 6.0));
        assert_eq!((b - a), Vec2::new(2.0, 2.0));
        assert!((Vec2::new(3.0, 4.0).len() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn ground_projects_xy() {
        assert_eq!(Vec3::new(1.0, 2.0, 3.0).ground(), Vec2::new(1.0, 2.0));
    }

    #[cfg(feature = "std")]
    #[test]
    fn normalize_game_byte_exact_vs_binaire() {
        // Cas validés byte-exact vs uemu (scripts/validate_normalize.py).
        let (d, l) = Vec3::new(3.0, 4.0, 0.0).normalize_game();
        assert_eq!(l.to_bits(), 5.0_f32.to_bits());
        assert_eq!(d.x.to_bits(), 0.6_f32.to_bits());
        assert_eq!(d.y.to_bits(), 0.8_f32.to_bits());
        assert_eq!(d.z.to_bits(), 0.0_f32.to_bits());
        // (1,2,2) : len 3 ; dir via réciproque-produit = v·(1/3).
        let (d2, l2) = Vec3::new(1.0, 2.0, 2.0).normalize_game();
        assert_eq!(l2.to_bits(), 3.0_f32.to_bits());
        let inv = 1.0_f32 / 3.0;
        assert_eq!(d2.x.to_bits(), (1.0_f32 * inv).to_bits());
        assert_eq!(d2.y.to_bits(), (2.0_f32 * inv).to_bits());
        // Vecteur nul → zéro (garde len > 0).
        let (dz, lz) = Vec3::zero().normalize_game();
        assert_eq!(dz, Vec3::zero());
        assert_eq!(lz.to_bits(), 0.0_f32.to_bits());
    }

    #[test]
    fn displacement_rate_byte_exact_vs_binaire() {
        // Cas validés byte-exact vs uemu (scripts/validate_dribble_vel.py).
        let v = Vec3::new(5.0, 8.0, 3.0).displacement_rate(Vec3::new(1.0, 2.0, 3.0), 0.5);
        assert_eq!(v.x.to_bits(), 8.0_f32.to_bits()); // (5-1)·2
        assert_eq!(v.y.to_bits(), 12.0_f32.to_bits()); // (8-2)·2
        assert_eq!(v.z.to_bits(), 0.0_f32.to_bits());
        let v2 = Vec3::new(10.0, 0.0, -4.0).displacement_rate(Vec3::new(2.0, 1.0, 0.0), 0.25);
        assert_eq!(v2.x.to_bits(), 32.0_f32.to_bits()); // (10-2)·4
        assert_eq!(v2.z.to_bits(), (-16.0_f32).to_bits());
        // dt ≤ 0 → simple delta.
        let v3 = Vec3::new(3.0, 3.0, 3.0).displacement_rate(Vec3::zero(), 0.0);
        assert_eq!(v3, Vec3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn reflect_byte_exact_vs_binaire() {
        // Validé byte-exact vs uemu (scripts/validate_reflect.py).
        let r = Vec3::new(1.0, 2.0, 3.0).reflect(Vec3::new(0.0, 1.0, 0.0)); // dot=2, refl y inversé
        assert_eq!(r.x.to_bits(), 1.0_f32.to_bits());
        assert_eq!(r.y.to_bits(), (-2.0_f32).to_bits()); // 2 - 2*2*1 = -2
        assert_eq!(r.z.to_bits(), 3.0_f32.to_bits());
        let r2 = Vec3::new(3.0, 4.0, 0.0).reflect(Vec3::new(1.0, 0.0, 0.0)); // dot=3
        assert_eq!(r2.x.to_bits(), (-3.0_f32).to_bits()); // 3 - 6 = -3
        assert_eq!(r2.y.to_bits(), 4.0_f32.to_bits());
    }

    #[cfg(feature = "std")]
    #[test]
    fn bezier_quadratic_byte_exact_vs_binaire() {
        // Cas validés byte-exact vs uemu (scripts/validate_bezier.py).
        let b = Vec3::bezier_quadratic(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 10.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
            0.5,
        );
        assert_eq!(b.x.to_bits(), 10.0_f32.to_bits());
        assert_eq!(b.y.to_bits(), 5.0_f32.to_bits());
        assert_eq!(b.z.to_bits(), 0.0_f32.to_bits());
        // Points de contrôle alignés, t=0.25 → (2,0,0).
        let l = Vec3::bezier_quadratic(
            Vec3::zero(),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(8.0, 0.0, 0.0),
            0.25,
        );
        assert_eq!(l.x.to_bits(), 2.0_f32.to_bits());
        // Points égaux → ce point.
        let e = Vec3::bezier_quadratic(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(1.0, 2.0, 3.0),
            0.5,
        );
        assert_eq!(e, Vec3::new(1.0, 2.0, 3.0));
    }
}
