//! Helpers vectoriels 3D partagés par [`render`](crate::render) et [`scene`](crate::scene)
//! (étaient dupliqués **verbatim** dans les deux modules — dédup intra-crate).
//!
//! POD `[f32; 3]` + math **scalaire** (pas de SIMD/FMA) pour le déterminisme du rendu CPU.

pub(crate) type V3 = [f32; 3];

pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn normv(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-9 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}
