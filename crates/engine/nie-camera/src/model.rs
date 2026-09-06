//! Modèle d'état de caméra et taxonomie des contrôleurs natifs.
//!
//! L'état ([`CameraState`]) reprend les champs que `CGameCameraCtrl` expose réellement dans
//! `nie.exe` (noms de propriétés reversés, cf. `docs/game-data/camera.md` §4) : position, point
//! de référence visé, FOV, roll, plans de clipping. Les matrices produites suivent la convention
//! de [`nie_formats`]/`nie-render3d` : repère main droite, look-at, `focal = 1/tan(fov_y/2)`.

/// Vecteur 3D (x, y, z) en espace monde.
pub type V3 = [f32; 3];

/// Matrice 4×4 **row-major** (ligne `i` = `m[i]`), comme `nie_render3d::scene::Mat4`.
pub type Mat4 = [[f32; 4]; 4];

/// Les contrôleurs de caméra natifs, tels qu'ils existent dans `nie.exe`.
///
/// La hiérarchie est **prouvée** par les symboles RTTI `TAddPropertyCreator<Dérivée, Base>` du
/// binaire : tout ce qui est listé ici sous [`CtrlKind::base`] `= GameCameraCtrl` dérive
/// effectivement de `game::CGameCameraCtrl`, lui-même dérivé de `lives::CCameraCtrl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum CtrlKind {
    /// `lives::CCameraCtrl` — racine de la hiérarchie.
    CameraCtrl,
    /// `lives::CCameraAnimeCtrl` — lecture d'une animation caméra.
    CameraAnimeCtrl,
    /// `game::CGameCameraBlender` — fondu entre deux contrôleurs.
    GameCameraBlender,
    /// `game::CGameCameraCtrl` — base commune des contrôleurs de jeu.
    GameCameraCtrl,
    /// `game::CCameraCtrlEvent` — cutscene (pilotée par un `.g4cm`).
    Event,
    /// `game::CCameraCtrlFixPos` — position fixe.
    FixPos,
    /// `game::CCameraCtrlFps` — vue subjective.
    Fps,
    /// `game::CCameraCtrlInterPolate` — interpolation entre deux états.
    InterPolate,
    /// `game::CCameraCtrlMenu` — caméra de menu.
    Menu,
    /// `game::CCameraCtrlNearFar` — pilotage des plans de clipping.
    NearFar,
    /// `game::CCameraCtrlOffset` — décalage additif temporaire.
    Offset,
    /// `game::CCameraCtrlPickUp` — mise en avant d'une cible.
    PickUp,
    /// `game::CCameraCtrlRail` — déplacement sur rail (`GDSRailCamera`).
    Rail,
    /// `game::CCameraCtrlSelfie` — mode photo / selfie.
    Selfie,
    /// `game::CCameraCtrlShake` — tremblement.
    Shake,
    /// `game::CCameraCtrlShooting` — tir.
    Shooting,
    /// `game::CCameraCtrlSoccerMenu` — menu affiché pendant un match.
    SoccerMenu,
    /// `game::CCameraCtrlSpecialAttack` — hissatsu.
    SpecialAttack,
    /// `game::CGameCameraAnimeCtrl` — animation caméra côté jeu.
    GameCameraAnimeCtrl,
    /// `game::CGameCameraAnimeRateCtrl` — animation à vitesse variable.
    GameCameraAnimeRateCtrl,
    /// `game::CCameraCtrlChaseBase` — base des caméras de poursuite.
    ChaseBase,
    /// `game::CCameraCtrlChase` — poursuite générique.
    Chase,
    /// `game::CCameraCtrlChaseSoccer` — poursuite ballon/joueur en match.
    ChaseSoccer,
}

impl CtrlKind {
    /// Les 23 contrôleurs, dans l'ordre de la hiérarchie.
    pub const ALL: [CtrlKind; 23] = [
        CtrlKind::CameraCtrl,
        CtrlKind::CameraAnimeCtrl,
        CtrlKind::GameCameraBlender,
        CtrlKind::GameCameraCtrl,
        CtrlKind::Event,
        CtrlKind::FixPos,
        CtrlKind::Fps,
        CtrlKind::InterPolate,
        CtrlKind::Menu,
        CtrlKind::NearFar,
        CtrlKind::Offset,
        CtrlKind::PickUp,
        CtrlKind::Rail,
        CtrlKind::Selfie,
        CtrlKind::Shake,
        CtrlKind::Shooting,
        CtrlKind::SoccerMenu,
        CtrlKind::SpecialAttack,
        CtrlKind::GameCameraAnimeCtrl,
        CtrlKind::GameCameraAnimeRateCtrl,
        CtrlKind::ChaseBase,
        CtrlKind::Chase,
        CtrlKind::ChaseSoccer,
    ];

    /// Nom C++ complet tel qu'il apparaît dans le RTTI de `nie.exe`.
    #[must_use]
    pub const fn cpp_name(self) -> &'static str {
        match self {
            CtrlKind::CameraCtrl => "lives::CCameraCtrl",
            CtrlKind::CameraAnimeCtrl => "lives::CCameraAnimeCtrl",
            CtrlKind::GameCameraBlender => "game::CGameCameraBlender",
            CtrlKind::GameCameraCtrl => "game::CGameCameraCtrl",
            CtrlKind::Event => "game::CCameraCtrlEvent",
            CtrlKind::FixPos => "game::CCameraCtrlFixPos",
            CtrlKind::Fps => "game::CCameraCtrlFps",
            CtrlKind::InterPolate => "game::CCameraCtrlInterPolate",
            CtrlKind::Menu => "game::CCameraCtrlMenu",
            CtrlKind::NearFar => "game::CCameraCtrlNearFar",
            CtrlKind::Offset => "game::CCameraCtrlOffset",
            CtrlKind::PickUp => "game::CCameraCtrlPickUp",
            CtrlKind::Rail => "game::CCameraCtrlRail",
            CtrlKind::Selfie => "game::CCameraCtrlSelfie",
            CtrlKind::Shake => "game::CCameraCtrlShake",
            CtrlKind::Shooting => "game::CCameraCtrlShooting",
            CtrlKind::SoccerMenu => "game::CCameraCtrlSoccerMenu",
            CtrlKind::SpecialAttack => "game::CCameraCtrlSpecialAttack",
            CtrlKind::GameCameraAnimeCtrl => "game::CGameCameraAnimeCtrl",
            CtrlKind::GameCameraAnimeRateCtrl => "game::CGameCameraAnimeRateCtrl",
            CtrlKind::ChaseBase => "game::CCameraCtrlChaseBase",
            CtrlKind::Chase => "game::CCameraCtrlChase",
            CtrlKind::ChaseSoccer => "game::CCameraCtrlChaseSoccer",
        }
    }

    /// Classe de base directe, `None` pour la racine `lives::CCameraCtrl`.
    #[must_use]
    pub const fn base(self) -> Option<CtrlKind> {
        match self {
            CtrlKind::CameraCtrl => None,
            CtrlKind::CameraAnimeCtrl | CtrlKind::GameCameraBlender | CtrlKind::GameCameraCtrl => {
                Some(CtrlKind::CameraCtrl)
            }
            CtrlKind::Chase | CtrlKind::ChaseSoccer => Some(CtrlKind::ChaseBase),
            _ => Some(CtrlKind::GameCameraCtrl),
        }
    }

    /// `true` si ce contrôleur est porté en Rust dans [`crate::ctrl`].
    #[must_use]
    pub const fn is_ported(self) -> bool {
        matches!(
            self,
            CtrlKind::ChaseSoccer
                | CtrlKind::Chase
                | CtrlKind::Shake
                | CtrlKind::InterPolate
                | CtrlKind::Offset
                | CtrlKind::FixPos
                | CtrlKind::NearFar
                | CtrlKind::GameCameraBlender
                | CtrlKind::Event
        )
    }
}

/// État complet d'une caméra à un instant donné.
///
/// Les noms de champs suivent les propriétés reversées de `CGameCameraCtrl` :
/// `m_cameraPosDistanceFromRefPos`, `m_cameraAzimuth`, `m_cameraAltitude`, `m_cameraFov`,
/// `m_cameraRoll`, `m_cameraRefPosOffset`. On stocke ici la forme **cartésienne** (position +
/// point visé), la forme polaire étant obtenue par [`CameraState::orbit`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraState {
    /// Position monde de l'œil (`m_worldCameraPos`).
    pub pos: V3,
    /// Point visé (`refPos` — centre d'orbite des contrôleurs de poursuite).
    pub ref_pos: V3,
    /// Champ de vision **vertical**, en degrés (`cameraFov` / `m_cameraFov` ; les données du jeu
    /// sont en degrés, ex. `fov: 45` dans `m_scGoalnetCameraInfoList`).
    pub fov_deg: f32,
    /// Roulis en degrés (`cameraRoll` / `m_cameraRoll`).
    pub roll_deg: f32,
    /// Plan de clipping proche (`resetCameraNear`, `overwriteCameraClipParam_nearClip`).
    pub near: f32,
    /// Plan de clipping lointain (`resetCameraFar`).
    pub far: f32,
}

impl Default for CameraState {
    /// Valeurs par défaut plausibles : caméra à 10 m derrière l'origine, FOV 45°.
    ///
    /// `45` est le FOV réellement présent dans les données (`m_scGoalnetCameraInfoList`), et
    /// `near/far` reprennent l'ordre de grandeur de `defaultCameraFadeNear/Far`.
    fn default() -> Self {
        CameraState {
            pos: [0.0, 2.5, 10.0],
            ref_pos: [0.0, 0.0, 0.0],
            fov_deg: 45.0,
            roll_deg: 0.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl CameraState {
    /// Distance œil → point visé (`m_cameraPosDistanceFromRefPos`).
    #[must_use]
    pub fn length(&self) -> f32 {
        let d = sub(self.pos, self.ref_pos);
        dot(d, d).sqrt()
    }

    /// Forme polaire `(distance, azimut_rad, altitude_rad)` autour de [`Self::ref_pos`].
    ///
    /// Correspond au triplet `m_cameraPosDistanceFromRefPos` / `m_cameraAzimuth` /
    /// `m_cameraAltitude` du contrôleur natif. L'azimut est mesuré autour de `+Y`, l'altitude
    /// au-dessus du plan `XZ`.
    #[must_use]
    pub fn orbit(&self) -> (f32, f32, f32) {
        let d = sub(self.pos, self.ref_pos);
        let len = dot(d, d).sqrt();
        if len <= f32::EPSILON {
            return (0.0, 0.0, 0.0);
        }
        let azimuth = d[0].atan2(d[2]);
        let altitude = (d[1] / len).clamp(-1.0, 1.0).asin();
        (len, azimuth, altitude)
    }

    /// Reconstruit la position depuis la forme polaire (inverse de [`Self::orbit`]).
    pub fn set_orbit(&mut self, length: f32, azimuth: f32, altitude: f32) {
        let ca = altitude.cos();
        self.pos = [
            self.ref_pos[0] + length * ca * azimuth.sin(),
            self.ref_pos[1] + length * altitude.sin(),
            self.ref_pos[2] + length * ca * azimuth.cos(),
        ];
    }

    /// Matrice de vue look-at (repère main droite, `up` = `+Y` tourné de [`Self::roll_deg`]).
    #[must_use]
    pub fn view_matrix(&self) -> Mat4 {
        let f = normalize(sub(self.ref_pos, self.pos));
        let roll = self.roll_deg.to_radians();
        // `up` de base tourné de `roll` autour de l'axe de visée.
        let up0 = [0.0, 1.0, 0.0];
        let r0 = normalize(cross(f, up0));
        let u0 = cross(r0, f);
        let (s, c) = (roll.sin(), roll.cos());
        let up = [
            u0[0] * c + r0[0] * s,
            u0[1] * c + r0[1] * s,
            u0[2] * c + r0[2] * s,
        ];
        let r = normalize(cross(f, up));
        let u = cross(r, f);
        [
            [r[0], r[1], r[2], -dot(r, self.pos)],
            [u[0], u[1], u[2], -dot(u, self.pos)],
            [-f[0], -f[1], -f[2], dot(f, self.pos)],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    /// Matrice de projection perspective (profondeur `[0, 1]`, comme wgpu/D3D).
    #[must_use]
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        let focal = 1.0 / (self.fov_deg.to_radians() * 0.5).tan();
        let (n, f) = (self.near, self.far);
        [
            [focal / aspect.max(f32::EPSILON), 0.0, 0.0, 0.0],
            [0.0, focal, 0.0, 0.0],
            [0.0, 0.0, f / (n - f), f * n / (n - f)],
            [0.0, 0.0, -1.0, 0.0],
        ]
    }

    /// Interpolation linéaire d'état (base de `CCameraCtrlInterPolate`).
    ///
    /// `t` est borné à `[0, 1]`. Les angles sont interpolés linéairement — c'est bien ce que
    /// fait le jeu pour `fov`/`roll`, dont les variations restent petites entre deux états.
    #[must_use]
    pub fn lerp(a: &CameraState, b: &CameraState, t: f32) -> CameraState {
        let t = t.clamp(0.0, 1.0);
        let l = |x: f32, y: f32| x + (y - x) * t;
        CameraState {
            pos: [
                l(a.pos[0], b.pos[0]),
                l(a.pos[1], b.pos[1]),
                l(a.pos[2], b.pos[2]),
            ],
            ref_pos: [
                l(a.ref_pos[0], b.ref_pos[0]),
                l(a.ref_pos[1], b.ref_pos[1]),
                l(a.ref_pos[2], b.ref_pos[2]),
            ],
            fov_deg: l(a.fov_deg, b.fov_deg),
            roll_deg: l(a.roll_deg, b.roll_deg),
            near: l(a.near, b.near),
            far: l(a.far, b.far),
        }
    }
}

/// `a - b`.
#[must_use]
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a + b`.
#[must_use]
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `a * k`.
#[must_use]
pub fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// Produit scalaire.
#[must_use]
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Produit vectoriel.
#[must_use]
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Normalise (renvoie le vecteur tel quel s'il est nul).
#[must_use]
pub fn normalize(a: V3) -> V3 {
    let n = dot(a, a).sqrt();
    if n <= f32::EPSILON {
        a
    } else {
        scale(a, 1.0 / n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchie_rtti_coherente() {
        // La racine n'a pas de base ; tout le reste remonte à CameraCtrl en un nombre fini d'étapes.
        for k in CtrlKind::ALL {
            let mut cur = k;
            let mut hops = 0;
            while let Some(b) = cur.base() {
                cur = b;
                hops += 1;
                assert!(hops < 8, "cycle dans la hiérarchie depuis {k:?}");
            }
            assert_eq!(cur, CtrlKind::CameraCtrl);
        }
        assert_eq!(CtrlKind::ChaseSoccer.base(), Some(CtrlKind::ChaseBase));
        assert_eq!(CtrlKind::ChaseBase.base(), Some(CtrlKind::GameCameraCtrl));
        assert_eq!(CtrlKind::Menu.cpp_name(), "game::CCameraCtrlMenu");
    }

    #[test]
    fn orbite_aller_retour() {
        let mut c = CameraState {
            pos: [3.0, 4.0, 5.0],
            ref_pos: [1.0, 1.0, 1.0],
            ..CameraState::default()
        };
        let (len, az, alt) = c.orbit();
        let before = c.pos;
        c.set_orbit(len, az, alt);
        for (i, b) in before.iter().enumerate() {
            assert!((c.pos[i] - b).abs() < 1e-4, "axe {i}");
        }
    }

    #[test]
    fn vue_regarde_bien_la_cible() {
        let c = CameraState {
            pos: [0.0, 0.0, 10.0],
            ref_pos: [0.0, 0.0, 0.0],
            ..CameraState::default()
        };
        let v = c.view_matrix();
        // La cible doit se projeter sur l'axe optique : x = y = 0 en espace vue.
        let p = [0.0f32, 0.0, 0.0, 1.0];
        let x: f32 = (0..4).map(|i| v[0][i] * p[i]).sum();
        let y: f32 = (0..4).map(|i| v[1][i] * p[i]).sum();
        let z: f32 = (0..4).map(|i| v[2][i] * p[i]).sum();
        assert!(x.abs() < 1e-5 && y.abs() < 1e-5);
        assert!((z + 10.0).abs() < 1e-4, "la cible est à 10 devant : z={z}");
    }

    #[test]
    fn lerp_borne() {
        let a = CameraState::default();
        let b = CameraState {
            fov_deg: 90.0,
            ..CameraState::default()
        };
        assert_eq!(CameraState::lerp(&a, &b, -1.0).fov_deg, 45.0);
        assert_eq!(CameraState::lerp(&a, &b, 2.0).fov_deg, 90.0);
        assert!((CameraState::lerp(&a, &b, 0.5).fov_deg - 67.5).abs() < 1e-4);
    }
}
