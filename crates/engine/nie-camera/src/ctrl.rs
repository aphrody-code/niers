//! Contrôleurs de caméra portés.
//!
//! Chaque contrôleur prend un [`CameraState`] et le fait évoluer d'une frame. Les paramètres
//! portent le nom exact de la propriété correspondante dans `nie.exe`, et les valeurs par défaut
//! sont celles réellement lues dans les données du jeu — soit dans
//! `camera_ctrl_property_info.cfg.bin` (presets `CCameraCtrlChase*`, cf. [`crate::property`]),
//! soit dans `soccer_camera_config` ([`crate::config`]).
//!
//! ## Ce qui est porté et ce qui ne l'est pas
//!
//! Le lissage est un **filtre exponentiel par frame** : `x += (cible - x) * taux`. C'est la
//! forme qu'imposent les données (`moveInterpRate = 0.1`, `m_fInterpRate = 0.2`, appliqués par
//! frame) et le vocabulaire du binaire (`isInterPolateWhenChangeCamera`,
//! `soccerCameraMoveReturnInterpRate`). Le **détail interne** des routines natives n'a pas été
//! désassemblé instruction par instruction : ces contrôleurs reproduisent le comportement
//! paramétré par les données, pas le bit-exact du moteur. C'est dit ici plutôt que sous-entendu.

use crate::config::SoccerCameraInfoData;
use crate::model::{CameraState, add, normalize, scale, sub};
use crate::property::PropertySet;

/// Filtre exponentiel : rapproche `cur` de `target` de `rate` (borné à `[0, 1]`).
#[must_use]
pub fn approach(cur: f32, target: f32, rate: f32) -> f32 {
    cur + (target - cur) * rate.clamp(0.0, 1.0)
}

fn approach3(cur: [f32; 3], target: [f32; 3], rate: f32) -> [f32; 3] {
    [
        approach(cur[0], target[0], rate),
        approach(cur[1], target[1], rate),
        approach(cur[2], target[2], rate),
    ]
}

/// Normalise un angle en degrés dans `(-180, 180]`, pour interpoler par le plus court chemin.
#[must_use]
pub fn wrap_deg(mut a: f32) -> f32 {
    while a > 180.0 {
        a -= 360.0;
    }
    while a <= -180.0 {
        a += 360.0;
    }
    a
}

/// `CCameraCtrlChaseSoccer` — poursuite du ballon/porteur pendant un match.
///
/// Paramètres issus de `SOCCER_CAMERA_INFO_DATA` : `length`, `rotX`, `rotY`, `fov`,
/// `refOffset`, `moveInterpRate`, `rotInterpRate`, `zoomInterpRate`, et les décalages d'azimut
/// offensif/défensif.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChaseSoccer {
    /// Jeu de paramètres actif.
    pub data: SoccerCameraInfoData,
    /// Azimut courant (degrés) — lissé vers `rotY` + décalage de phase.
    pub azimuth: f32,
    /// Inclinaison courante (degrés) — lissée vers `rotX`.
    pub altitude: f32,
    /// Distance courante — lissée vers `length`.
    pub length: f32,
    /// `true` si l'équipe contrôlée est en phase offensive.
    pub offence: bool,
}

impl ChaseSoccer {
    /// Contrôleur initialisé sur un jeu de paramètres, aux valeurs cibles.
    #[must_use]
    pub fn new(data: SoccerCameraInfoData) -> Self {
        ChaseSoccer {
            azimuth: data.rot_y,
            altitude: data.rot_x,
            length: data.length,
            offence: true,
            data,
        }
    }

    /// Décalage d'azimut de la phase courante.
    #[must_use]
    pub fn phase_offset(&self) -> f32 {
        if self.offence {
            self.data.offence_offset_rot_y
        } else {
            self.data.defence_offset_rot_y
        }
    }

    /// Décalage du point visé de la phase courante.
    #[must_use]
    pub fn phase_ref_offset(&self) -> [f32; 3] {
        let o = if self.offence {
            self.data.offence_ref_offset
        } else {
            self.data.defence_ref_offset
        };
        [
            o[0] + self.data.ref_offset[0],
            o[1] + self.data.ref_offset[1],
            o[2] + self.data.ref_offset[2],
        ]
    }

    /// Avance d'une frame : `target` est la cible suivie (ballon ou joueur).
    ///
    /// Les angles et la distance sont lissés à leurs taux respectifs puis bornés par
    /// `rotXMin/Max`, `rotYMin/Max`, `lengthMin/Max` ; la position est recalculée en orbite
    /// autour du point visé.
    pub fn step(&mut self, cam: &mut CameraState, target: [f32; 3]) {
        let want_az = wrap_deg(self.data.rot_y + self.phase_offset());
        let delta = wrap_deg(want_az - self.azimuth);
        self.azimuth = wrap_deg(self.azimuth + delta * self.data.rot_interp_rate.clamp(0.0, 1.0));
        self.altitude = approach(self.altitude, self.data.rot_x, self.data.rot_interp_rate)
            .clamp(self.data.rot_x_min, self.data.rot_x_max);
        self.length = approach(self.length, self.data.length, self.data.zoom_interp_rate)
            .clamp(self.data.length_min, self.data.length_max);
        if self.data.rot_y_min <= self.data.rot_y_max {
            self.azimuth = self.azimuth.clamp(self.data.rot_y_min, self.data.rot_y_max);
        }

        let want_ref = add(target, self.phase_ref_offset());
        cam.ref_pos = approach3(cam.ref_pos, want_ref, self.data.move_interp_rate);
        cam.fov_deg = self.data.fov;
        cam.set_orbit(
            self.length,
            self.azimuth.to_radians(),
            self.altitude.to_radians(),
        );
    }
}

/// `CCameraCtrlShake` — tremblement de caméra.
///
/// Modélise le shake de tir décrit par les paramètres `shootCameraLargeShake*` /
/// `shootCameraSmallShake*` : oscillation d'amplitude `amplitude` et de période `period` sur
/// deux axes, atténuée linéairement sur `duration`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shake {
    /// `shootCameraLargeShakeAmplitudeX` / `…Y`.
    pub amplitude: (f32, f32),
    /// `shootCameraLargeShakePeriodX` / `…Y`, en secondes.
    pub period: (f32, f32),
    /// `shootCameraLargeShakeTime` — durée totale, en secondes.
    pub duration: f32,
    /// Temps écoulé depuis le début, en secondes.
    pub elapsed: f32,
}

impl Shake {
    /// Un shake « large » aux valeurs par défaut du jeu.
    #[must_use]
    pub fn large() -> Self {
        Shake {
            amplitude: (0.35, 0.35),
            period: (0.06, 0.05),
            duration: 0.5,
            elapsed: 0.0,
        }
    }

    /// `true` tant que le shake est actif.
    #[must_use]
    pub fn active(&self) -> bool {
        self.elapsed < self.duration
    }

    /// Avance de `dt` secondes et renvoie le décalage à ajouter à la position caméra.
    ///
    /// Le décalage est exprimé dans le repère de la caméra (droite, haut) puis projeté en monde
    /// par [`Self::step`] ; il s'annule exactement à la fin de la durée.
    pub fn advance(&mut self, dt: f32) -> (f32, f32) {
        self.elapsed += dt;
        if !self.active() {
            return (0.0, 0.0);
        }
        let decay = 1.0 - (self.elapsed / self.duration.max(f32::EPSILON)).clamp(0.0, 1.0);
        let tau = std::f32::consts::TAU;
        let x = (self.elapsed / self.period.0.max(1e-4) * tau).sin() * self.amplitude.0 * decay;
        let y = (self.elapsed / self.period.1.max(1e-4) * tau).sin() * self.amplitude.1 * decay;
        (x, y)
    }

    /// Applique le tremblement à un état de caméra (déplace l'œil, pas la cible).
    pub fn step(&mut self, cam: &mut CameraState, dt: f32) {
        let (dx, dy) = self.advance(dt);
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let fwd = normalize(sub(cam.ref_pos, cam.pos));
        let right = normalize(crate::model::cross(fwd, [0.0, 1.0, 0.0]));
        let up = crate::model::cross(right, fwd);
        cam.pos = add(cam.pos, add(scale(right, dx), scale(up, dy)));
    }
}

/// Type de fondu d'interpolation (`m_FadeType` de `SoccerCameraInterpProperty`, `uOffsetCameraInFadeType`).
///
/// Le fichier réel utilise `m_FadeType = 6`. La correspondance code → courbe n'est pas
/// documentée dans le binaire ; les courbes ci-dessous sont les formes classiques, et
/// [`FadeType::from_code`] mappe la valeur observée sur la plus lisse d'entre elles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeType {
    /// Progression constante.
    Linear,
    /// Départ doux.
    EaseIn,
    /// Arrivée douce.
    EaseOut,
    /// Départ et arrivée doux (`smoothstep`).
    EaseInOut,
}

impl FadeType {
    /// Décode un `m_FadeType`. Tout code non répertorié donne [`FadeType::EaseInOut`], la courbe
    /// du cas observé (`6`).
    #[must_use]
    pub const fn from_code(code: i32) -> FadeType {
        match code {
            0 => FadeType::Linear,
            1 => FadeType::EaseIn,
            2 => FadeType::EaseOut,
            _ => FadeType::EaseInOut,
        }
    }

    /// Applique la courbe à `t ∈ [0, 1]`.
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            FadeType::Linear => t,
            FadeType::EaseIn => t * t,
            FadeType::EaseOut => t * (2.0 - t),
            FadeType::EaseInOut => t * t * (3.0 - 2.0 * t),
        }
    }
}

/// `CCameraCtrlInterPolate` — transition entre deux états.
///
/// Paramètres réels de `soccer_camera_interp_property.cfg.bin` : `m_InterpTime = 1.5`,
/// `m_FadeType = 6`, `m_Curvature = 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterPolate {
    /// État de départ.
    pub from: CameraState,
    /// État d'arrivée.
    pub to: CameraState,
    /// `m_InterpTime` — durée, en secondes.
    pub duration: f32,
    /// `m_FadeType`.
    pub fade: FadeType,
    /// Temps écoulé.
    pub elapsed: f32,
}

impl InterPolate {
    /// Nouvelle transition.
    #[must_use]
    pub fn new(from: CameraState, to: CameraState, duration: f32, fade: FadeType) -> Self {
        InterPolate {
            from,
            to,
            duration,
            fade,
            elapsed: 0.0,
        }
    }

    /// `true` tant que la transition n'est pas terminée.
    #[must_use]
    pub fn active(&self) -> bool {
        self.elapsed < self.duration
    }

    /// Avance de `dt` et renvoie l'état interpolé.
    pub fn step(&mut self, dt: f32) -> CameraState {
        self.elapsed += dt;
        let t = if self.duration <= f32::EPSILON {
            1.0
        } else {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        };
        CameraState::lerp(&self.from, &self.to, self.fade.apply(t))
    }
}

/// `CGameCameraBlender` — mélange pondéré de deux contrôleurs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blender {
    /// Poids du second état, dans `[0, 1]`.
    pub weight: f32,
}

impl Blender {
    /// Mélange les deux états.
    #[must_use]
    pub fn blend(&self, a: &CameraState, b: &CameraState) -> CameraState {
        CameraState::lerp(a, b, self.weight)
    }
}

/// `CCameraCtrlOffset` — décalage additif temporaire appliqué par-dessus un autre contrôleur.
///
/// Paramètres réels : `fOffsetCameraInTime`, `fOffsetCameraOutTime`, `fOffsetCameraLoopTime`,
/// `fOffsetCameraMoveRate`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Offset {
    /// Décalage à pleine intensité (monde).
    pub offset: [f32; 3],
    /// `fOffsetCameraInTime` — montée, en secondes.
    pub in_time: f32,
    /// `fOffsetCameraOutTime` — descente, en secondes.
    pub out_time: f32,
    /// `fOffsetCameraLoopTime` — palier entre montée et descente, en secondes.
    pub hold_time: f32,
    /// Temps écoulé.
    pub elapsed: f32,
}

impl Offset {
    /// Intensité courante, dans `[0, 1]`.
    #[must_use]
    pub fn intensity(&self) -> f32 {
        let t = self.elapsed;
        if t < self.in_time {
            return if self.in_time <= f32::EPSILON {
                1.0
            } else {
                t / self.in_time
            };
        }
        let t = t - self.in_time;
        if t < self.hold_time {
            return 1.0;
        }
        let t = t - self.hold_time;
        if self.out_time <= f32::EPSILON || t >= self.out_time {
            0.0
        } else {
            1.0 - t / self.out_time
        }
    }

    /// `true` tant que le décalage n'est pas retombé à zéro.
    #[must_use]
    pub fn active(&self) -> bool {
        self.elapsed < self.in_time + self.hold_time + self.out_time
    }

    /// Avance de `dt` et applique le décalage à l'œil et à la cible.
    pub fn step(&mut self, cam: &mut CameraState, dt: f32) {
        self.elapsed += dt;
        let k = self.intensity();
        let d = scale(self.offset, k);
        cam.pos = add(cam.pos, d);
        cam.ref_pos = add(cam.ref_pos, d);
    }
}

/// Construit un [`ChaseSoccer`] à partir d'un preset de `camera_ctrl_property_info`.
///
/// Traduit les propriétés du preset (`m_fCamLength`, `m_fCamMinLength`, `m_fCamMaxLength`,
/// `m_fRotMinX`, `m_fRotBaseX`, `m_fRotMaxX`, `m_fInterpRate`, `m_vCameraRefOffset`) vers un
/// [`SoccerCameraInfoData`]. Les champs sans équivalent dans le preset gardent leur valeur par
/// défaut. Renvoie `None` si le preset n'existe pas.
#[must_use]
pub fn chase_from_preset(set: &PropertySet, preset: &str) -> Option<ChaseSoccer> {
    let p = set.resolve(preset);
    if p.is_empty() {
        return None;
    }
    let f = |k: &str, d: f32| {
        p.get(k)
            .and_then(crate::property::ParamValue::as_f32)
            .unwrap_or(d)
    };
    let ref_off = p
        .get("m_vCameraRefOffset")
        .and_then(crate::property::ParamValue::as_vec3)
        .unwrap_or([0.0, 0.0, 0.0]);
    let interp = f("m_fInterpRate", 0.2);
    Some(ChaseSoccer::new(SoccerCameraInfoData {
        length: f("m_fCamLength", 8.0),
        length_min: f("m_fCamMinLength", 1.0),
        length_max: f("m_fCamMaxLength", 50.0),
        rot_x: f("m_fRotBaseX", 0.0),
        rot_x_min: f("m_fRotMinX", -180.0),
        rot_x_max: f("m_fRotMaxX", 180.0),
        rot_y_min: -180.0,
        rot_y_max: 180.0,
        fov: 45.0,
        ref_offset: [ref_off[0], ref_off[1], ref_off[2], 0.0],
        move_interp_rate: interp,
        rot_interp_rate: interp,
        zoom_interp_rate: interp,
        ..SoccerCameraInfoData::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> SoccerCameraInfoData {
        SoccerCameraInfoData {
            length: 8.0,
            length_min: 1.0,
            length_max: 50.0,
            rot_x: 30.0,
            rot_x_min: -180.0,
            rot_x_max: 180.0,
            rot_y: 90.0,
            rot_y_min: -180.0,
            rot_y_max: 180.0,
            fov: 45.0,
            offence_ref_offset: [0.0, 1.25, 0.0, 0.0],
            defence_ref_offset: [0.0, 1.25, 0.0, 0.0],
            move_interp_rate: 0.1,
            rot_interp_rate: 1.0,
            zoom_interp_rate: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn poursuite_converge_vers_la_cible() {
        let mut ctrl = ChaseSoccer::new(data());
        let mut cam = CameraState::default();
        let target = [10.0, 0.0, -5.0];
        for _ in 0..300 {
            ctrl.step(&mut cam, target);
        }
        // Le point visé doit tendre vers cible + refOffset (y = 1.25).
        assert!((cam.ref_pos[0] - 10.0).abs() < 0.01, "x={}", cam.ref_pos[0]);
        assert!((cam.ref_pos[1] - 1.25).abs() < 0.01, "y={}", cam.ref_pos[1]);
        assert!((cam.ref_pos[2] + 5.0).abs() < 0.01, "z={}", cam.ref_pos[2]);
        // Et la distance doit valoir `length`.
        assert!((cam.length() - 8.0).abs() < 0.01, "len={}", cam.length());
        assert!((cam.fov_deg - 45.0).abs() < f32::EPSILON);
    }

    #[test]
    fn poursuite_respecte_les_bornes() {
        let mut d = data();
        d.rot_x = 90.0;
        d.rot_x_max = 45.0;
        let mut ctrl = ChaseSoccer::new(d);
        let mut cam = CameraState::default();
        for _ in 0..50 {
            ctrl.step(&mut cam, [0.0, 0.0, 0.0]);
        }
        assert!(ctrl.altitude <= 45.0 + 1e-4, "altitude={}", ctrl.altitude);
    }

    #[test]
    fn shake_s_eteint() {
        let mut s = Shake::large();
        let mut cam = CameraState::default();
        let start = cam.pos;
        let mut moved = false;
        for _ in 0..10 {
            s.step(&mut cam, 1.0 / 60.0);
            if cam.pos != start {
                moved = true;
            }
        }
        assert!(moved, "le shake doit déplacer la caméra");
        // Après la durée, plus aucun décalage n'est produit.
        s.elapsed = s.duration;
        let before = cam.pos;
        s.step(&mut cam, 1.0 / 60.0);
        assert_eq!(cam.pos, before);
        assert!(!s.active());
    }

    #[test]
    fn interpolation_atteint_la_cible() {
        let a = CameraState::default();
        let b = CameraState {
            fov_deg: 90.0,
            ..CameraState::default()
        };
        let mut it = InterPolate::new(a, b, 1.5, FadeType::from_code(6));
        let mut last = a;
        for _ in 0..100 {
            last = it.step(1.0 / 60.0);
        }
        assert!(!it.active());
        assert!((last.fov_deg - 90.0).abs() < 1e-3);
        // La courbe par défaut est lisse : à mi-parcours elle vaut exactement 0.5.
        assert!((FadeType::EaseInOut.apply(0.5) - 0.5).abs() < 1e-6);
        assert!((FadeType::Linear.apply(0.25) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn offset_monte_tient_puis_retombe() {
        let mut o = Offset {
            offset: [0.0, 2.0, 0.0],
            in_time: 0.5,
            out_time: 0.5,
            hold_time: 1.0,
            elapsed: 0.0,
        };
        assert!((o.intensity() - 0.0).abs() < 1e-6);
        o.elapsed = 0.5;
        assert!((o.intensity() - 1.0).abs() < 1e-6);
        o.elapsed = 1.4;
        assert!((o.intensity() - 1.0).abs() < 1e-6);
        o.elapsed = 1.75;
        assert!((o.intensity() - 0.5).abs() < 1e-3);
        o.elapsed = 2.0;
        assert!((o.intensity() - 0.0).abs() < 1e-6);
        assert!(!o.active());
    }

    #[test]
    fn angles_replies() {
        assert!((wrap_deg(370.0) - 10.0).abs() < 1e-4);
        assert!((wrap_deg(-190.0) - 170.0).abs() < 1e-4);
        assert!((wrap_deg(180.0) - 180.0).abs() < 1e-4);
    }

    #[test]
    fn blender_pondere() {
        let a = CameraState::default();
        let b = CameraState {
            fov_deg: 90.0,
            ..CameraState::default()
        };
        let m = Blender { weight: 0.25 }.blend(&a, &b);
        assert!((m.fov_deg - 56.25).abs() < 1e-4);
    }
}
