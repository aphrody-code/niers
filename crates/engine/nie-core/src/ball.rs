//! Composant ballon de IEVR.
//!
//! Porte les classes C++ `game::BallComponent` et `game::BallMoveNormal`
//! depuis `refs/iecode-re/research/ghidra-export/decompiled/ball_component.c`.
//!
//! # Sources RE
//!
//! - `ball_component.c` — `BallComponent_ctor` (lignes nie.c 175440-175592)
//! - `ball_component.c` — `FUN_14027ac10` = `BallMoveNormal::BallMoveNormal()` (lignes 512870-512921)
//!
//! # Fidélité
//!
//! - Structure générale et champs nommés : FIABLE (dérivés directement des offsets commentés)
//! - Valeurs d'initialisation (gravité=2.0, scale=1.0, IDs=0xFF/0xFFFF0000) : FIABLE (bits IEEE 754 explicites)
//! - Variants de contrôleur de mouvement (`BallMoveKind`) : FIABLE (noms extraits des vftables Ghidra)
//! - Sémantique interne de `BallMoveNormal` (physique exacte) : INCERTAINE — le pseudo-C
//!   ne montre que le constructeur, pas la méthode `update()`.

use crate::{
    BALL_GRAVITY, BALL_SCALE_DEFAULT, DISTANCE_UNINIT, INVALID_PLAYER_IDX, INVALID_TARGET_ID, Vec3,
};

/// Type de contrôleur de mouvement du ballon.
///
/// Source: `ball_component.c` — commentaire « Architecture du mouvement du ballon »
/// et commentaires vftable (`game::BallMoveNormal::vftable`, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum BallMoveKind {
    /// Mouvement physique normal (gravité, rebonds).
    /// Contrôleur par défaut observé dans `FUN_14027ac10`.
    #[default]
    Normal,
    /// Suivi d'une cible (passes, centres).
    /// Utilisé aussi comme contrôleur initial dans `BallComponent_ctor`
    /// avant d'être remplacé par `Normal`.
    TargetFollow,
    /// Courbe de Bézier simple.
    Bezier,
    /// Courbe de Bézier multi-segments.
    MultiBezier,
    /// Tir hissatsu (courbe spéciale).
    RealSkillShootBezier,
    /// Trajectoire parabolique simple.
    SimpleParabola,
    /// Pendant le dribble.
    Dribble,
    /// Interpolation linéaire.
    Lerp,
    /// Mouvement basé sur un taux (fraction du déplacement par frame).
    Rate,
    /// Animation filet de but.
    Goalnet,
}

/// IDs de possession du ballon (jusqu'à 3 joueurs peuvent être concernés
/// simultanément, ex. duel possession vs contestation).
///
/// Source: `ball_component.c` offsets 0x1490-0x14A0 — 3 octets initialisés à `0xFF`.
/// RE incertain: le troisième slot n'est pas clairement documenté dans le RE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PossessionIds {
    /// Joueur principal en possession (`0xFF` = aucun).
    pub owner: u8,
    /// Joueur secondaire impliqué (`0xFF` = aucun).
    pub secondary: u8,
    /// Troisième joueur impliqué (`0xFF` = aucun).
    /// RE incertain: rôle exact du troisième slot inconnu.
    pub tertiary: u8,
}

impl Default for PossessionIds {
    fn default() -> Self {
        Self {
            owner: INVALID_PLAYER_IDX,
            secondary: INVALID_PLAYER_IDX,
            tertiary: INVALID_PLAYER_IDX,
        }
    }
}

impl PossessionIds {
    /// Retourne `true` si personne n'a le ballon.
    #[must_use]
    pub fn is_free(&self) -> bool {
        self.owner == INVALID_PLAYER_IDX
    }
}

/// IDs de cibles actives pour passe/interception.
///
/// Source: `ball_component.c` offsets 0x14D0, 0x14E0 — 2 × `u32` initialisés
/// à `0xFFFF0000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TargetIds {
    /// Cible principale de passe/interception (`INVALID_TARGET_ID` = aucune).
    pub primary: u32,
    /// Cible secondaire (`INVALID_TARGET_ID` = aucune).
    pub secondary: u32,
}

impl Default for TargetIds {
    fn default() -> Self {
        Self {
            primary: INVALID_TARGET_ID,
            secondary: INVALID_TARGET_ID,
        }
    }
}

impl TargetIds {
    /// Retourne `true` si aucune cible n'est définie.
    #[must_use]
    pub fn has_no_target(&self) -> bool {
        self.primary == INVALID_TARGET_ID && self.secondary == INVALID_TARGET_ID
    }
}

/// Données du contrôleur de mouvement normal (`game::BallMoveNormal`).
///
/// Source: `ball_component.c` — `FUN_14027ac10` (lignes nie.c 512870-512921).
///
/// # Fidélité
///
/// Le constructeur est fidèle (positions, direction à zéro, flags à false).
/// La sémantique de la physique (update) n'est PAS portée ici — seule la
/// structure d'initialisation est reconstruite.
///
/// RE incertain: les champs `force_a` et `force_b` (offsets 0x1C0, 0x200 dans
/// `BallMoveNormal`) sont deux vecteurs supplémentaires dont le rôle exact
/// (force externe vs inertie) est unclear.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BallMoveNormal {
    /// Position courante.
    pub position: Vec3,
    /// Position précédente (pour calcul de vitesse).
    pub prev_position: Vec3,
    /// Vecteur direction (normalisé en théorie).
    pub direction: Vec3,
    /// Second vecteur (surface normale? axe de rotation?).
    /// RE incertain: rôle exact non déterminé (offsets 0xE0-0xF0 dans la struct).
    pub direction_b: Vec3,
    /// Vecteur de rebond / normale de surface.
    pub bounce_normal: Vec3,
    /// Second vecteur de surface (axe tangent?).
    /// RE incertain.
    pub surface_tangent: Vec3,
    /// `true` si le ballon est en contact avec le sol.
    /// Source: `FUN_14027ac10` — `*(undefined1 *)(param_1 + 0x18) = 0`.
    pub on_ground: bool,
    /// Force externe A (direction+magnitude).
    /// RE incertain: deux vecteurs supplémentaires dans le ctor, rôle unclear.
    pub force_a: Vec3,
    /// Force externe B.
    /// RE incertain.
    pub force_b: Vec3,
    /// Flag de collision.
    /// Source: `FUN_14027ac10` — `*(undefined1 *)(longlong)param_1 + 300) = 0`.
    pub collision_flag: bool,
}

impl Default for BallMoveNormal {
    fn default() -> Self {
        Self {
            position: Vec3::zero(),
            prev_position: Vec3::zero(),
            direction: Vec3::zero(),
            direction_b: Vec3::zero(),
            bounce_normal: Vec3::zero(),
            surface_tangent: Vec3::zero(),
            on_ground: false,
            force_a: Vec3::zero(),
            force_b: Vec3::zero(),
            collision_flag: false,
        }
    }
}

/// Données d'interception en cours.
///
/// Source: `ball_component.c` — offsets 0x14F0-0x1540 (zone interception/trap).
/// RE incertain: structure interne non entièrement décodée. Seuls l'ID cible
/// et le flag d'état sont reconstruits avec certitude.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InterceptionData {
    /// ID du joueur tentant l'interception (`INVALID_TARGET_ID` = aucun).
    pub interceptor_id: u32,
    /// ID secondaire associé à l'interception.
    /// RE incertain: peut être le passeur ou la cible de la passe interceptée.
    pub source_id: u32,
}

impl InterceptionData {
    /// Crée une interception vide.
    #[must_use]
    pub fn none() -> Self {
        Self {
            interceptor_id: INVALID_TARGET_ID,
            source_id: INVALID_TARGET_ID,
        }
    }

    /// Retourne `true` si aucune interception n'est en cours.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.interceptor_id == INVALID_TARGET_ID
    }
}

/// Composant principal du ballon (`game::BallComponent`).
///
/// Source: `ball_component.c` — `BallComponent_ctor` (lignes nie.c 175440-175592).
/// Taille originale estimée à `~0x17C0` bytes (6080 bytes).
///
/// # Fidélité
///
/// - Toutes les valeurs d'initialisation sont confirmées par les bits IEEE 754 explicites.
/// - Le contrôleur de mouvement actif au démarrage est `BallMoveKind::TargetFollow`
///   (le ctor pose d'abord `IBallMoveController::vftable` puis `BallMoveTargetFollow::vftable`
///   avant le retour — `Normal` n'est pas le défaut réel pour un ball fraîchement créé).
/// - RE incertain: la grande zone `0x130-0x1450` (initialisée via `FUN_14132dc00`)
///   n'est pas décomposée ici, son contenu exact est inconnu.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BallComponent {
    /// Contrôleur de mouvement actif.
    pub move_controller: BallMoveKind,
    /// Position courante du ballon.
    pub position: Vec3,
    /// Position précédente (frame N-1).
    pub prev_position: Vec3,
    /// Vecteur d'orientation (direction du mouvement).
    pub orientation: Vec3,
    /// Vecteur de vitesse.
    /// RE incertain: ce champ est déduit de l'offset 0x0E0 dans la struct C++,
    /// mais son rôle exact (vitesse ou accélération) n'est pas confirmé.
    pub velocity: Vec3,
    /// IDs des joueurs en possession.
    pub possession: PossessionIds,
    /// IDs des cibles actives.
    pub targets: TargetIds,
    /// Données d'interception en cours.
    pub interception: InterceptionData,
    /// Gravité appliquée au ballon (unités/frame²).
    pub gravity: f32,
    /// Nombre maximum de collisions par frame.
    /// Source: `BallComponent_ctor` — `*(undefined1 *)(param_1 + 0x148a) = 5`.
    pub max_collisions_per_frame: u8,
    /// Scale du ballon (1.0 = normal).
    pub scale: f32,
    /// Distance de détection primaire (`-1.0` = non calculée).
    pub detection_dist_primary: f32,
    /// Distance de détection secondaire (`-1.0` = non calculée).
    pub detection_dist_secondary: f32,
    /// Données du contrôleur de mouvement normal (embarquées pour éviter alloc).
    pub move_normal: BallMoveNormal,
    /// Contrôleur de mouvement actif (dispatche vers les physiques byte-fidèles validées).
    /// Modélise la vftable `IBallMoveController` polymorphe du C++.
    pub mover: BallMover,
}

impl Default for BallComponent {
    /// Initialise le composant ballon avec les valeurs du constructeur C++.
    ///
    /// Source: `BallComponent_ctor` dans `ball_component.c`.
    fn default() -> Self {
        Self {
            // Le constructeur pose TargetFollow comme vftable finale
            move_controller: BallMoveKind::TargetFollow,
            position: Vec3::zero(),
            prev_position: Vec3::zero(),
            orientation: Vec3::zero(),
            velocity: Vec3::zero(),
            possession: PossessionIds::default(),
            targets: TargetIds::default(),
            interception: InterceptionData::none(),
            gravity: BALL_GRAVITY,
            max_collisions_per_frame: 5,
            scale: BALL_SCALE_DEFAULT,
            detection_dist_primary: DISTANCE_UNINIT,
            detection_dist_secondary: DISTANCE_UNINIT,
            move_normal: BallMoveNormal::default(),
            mover: BallMover::Idle,
        }
    }
}

impl BallComponent {
    /// Crée un composant ballon avec les valeurs d'initialisation par défaut.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_core::ball::BallComponent;
    /// use nie_core::{BALL_GRAVITY, INVALID_PLAYER_IDX};
    ///
    /// let ball = BallComponent::new();
    /// assert_eq!(ball.gravity, BALL_GRAVITY);
    /// assert!(ball.possession.is_free());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Retourne `true` si le ballon est libre (personne en possession).
    #[must_use]
    pub fn is_free(&self) -> bool {
        self.possession.is_free()
    }

    /// Retourne `true` si une interception est en cours.
    #[must_use]
    pub fn is_being_intercepted(&self) -> bool {
        !self.interception.is_none()
    }

    /// Définit le joueur propriétaire du ballon.
    pub fn set_owner(&mut self, player_idx: u8) {
        self.possession.owner = player_idx;
    }

    /// Libère la possession du ballon (remet tous les IDs à `INVALID_PLAYER_IDX`).
    pub fn release_possession(&mut self) {
        self.possession = PossessionIds::default();
        self.interception = InterceptionData::none();
        self.targets = TargetIds::default();
    }

    /// Avance le ballon d'un pas `dt` via le contrôleur de mouvement actif ([`BallMover`]).
    ///
    /// Met à jour `prev_position` puis `position` selon la physique byte-fidèle reversée
    /// (parabole / lerp / suivi-cible — validées vs le binaire). Câble enfin les boucles de
    /// physique prouvées dans la logique du ballon (remplace l'approximation best-effort).
    pub fn update(&mut self, dt: f32) {
        self.prev_position = self.position;
        self.position = self.mover.step(self.position, dt);
    }
}

/// Mouvement parabolique du ballon (projectile sous accélération constante).
///
/// Port BYTE-FIDÈLE de `game::BallMoveSimpleParabora::vmethod_3` (`0x141334600`), reversé de l'asm
/// et **validé byte-exact** contre l'émulation Unicorn du binaire réel (`scripts/validate_parabola.py`).
/// Remplace l'approximation best-effort de `nie-runtime` par la vraie physique du jeu.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParabolaMove {
    /// Accélération constante (gravité du jeu), offset `0x160` de l'objet jeu.
    pub accel: Vec3,
    /// Vitesse courante (intégrée à chaque pas), offset `0x180`.
    pub velocity: Vec3,
    /// Temps écoulé du mouvement, offset `0x190`.
    pub t: f32,
    /// Durée totale du mouvement, offset `0x170`.
    pub t_max: f32,
}

impl ParabolaMove {
    /// Avance d'un pas `dt` depuis la position initiale `p0`. Renvoie `(nouvelle_position, fini)` où
    /// `fini` = le temps a atteint `t_max` (sémantique `setae` du binaire).
    ///
    /// Formule reversée (ordre SSE EXACT du binaire, f32 simple précision = byte-fidèle) :
    /// `pos = ((0.5·a)·dt)·dt + ((dt·v) + p0)` ; `v += dt·a` ; `t += dt`.
    #[must_use]
    pub fn step(&mut self, p0: Vec3, dt: f32) -> (Vec3, bool) {
        if dt <= 0.0 {
            return (p0, false);
        }
        if self.t_max <= self.t {
            return (p0, true);
        }
        let comp = |a: f32, v: f32, p: f32| (((0.5_f32 * a) * dt) * dt) + ((dt * v) + p);
        let new_pos = Vec3 {
            x: comp(self.accel.x, self.velocity.x, p0.x),
            y: comp(self.accel.y, self.velocity.y, p0.y),
            z: comp(self.accel.z, self.velocity.z, p0.z),
        };
        self.velocity = Vec3 {
            x: self.velocity.x + dt * self.accel.x,
            y: self.velocity.y + dt * self.accel.y,
            z: self.velocity.z + dt * self.accel.z,
        };
        self.t += dt;
        (new_pos, self.t >= self.t_max)
    }
}

/// Mouvement par interpolation adoucie (quartic ease-out) entre `origin` et `target`,
/// **avec clamp de bord en y et snap à la complétion**.
///
/// Port BYTE-FIDÈLE de `game::BallMoveLerp::vmethod_3` (`0x141339ba0`, SSE + FMA3), reversé de l'asm
/// et **validé byte-exact** contre l'émulation Unicorn — en **single-step** (`scripts/validate_lerp.py`)
/// **ET en trajectoire multi-frames** (`scripts/validate_trajectory.py`, 16 frames, état réinjecté).
/// La FMA `vfmadd231ps` correspond à `f32::mul_add` (rounding unique).
///
/// # Correction 2026-06-23 (faux-FAIT révélé par la validation multi-frames)
///
/// Le port single-step initial **manquait** le clamp de bord en y + le snap à la complétion :
/// le test single-step (`duration = 2`, `t = 0.5`) n'atteignait jamais `t ≥ duration` ni
/// `lerped.y < bound`, donc ces branches restaient non exercées. La trajectoire multi-frames les a
/// révélées (la composante y plongeait à `bound_y` dès la complétion dans le binaire, pas dans le port).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LerpMove {
    /// Point cible (atteint à `t == duration`), offset `0x60` de l'objet jeu.
    pub target: Vec3,
    /// Temps écoulé, offset `0xa0`.
    pub t: f32,
    /// Durée totale, offset `0xa4`.
    pub duration: f32,
    /// Plancher/snap de la composante y : offset `0x94` (si flag `0x9c == 0`) ou `0x98` du binaire.
    /// `y_out = (complété || lerped.y < bound_y) ? bound_y : lerped.y` (reversé + validé 2026-06-23).
    pub bound_y: f32,
}

impl LerpMove {
    /// Avance d'un pas `dt` depuis `origin` (origine FIXE du lerp). Renvoie la nouvelle position.
    ///
    /// Formule reversée (f32, ordre du binaire) : `t += dt` ; `s = t/duration` ;
    /// `ease = min(1 − (s−1)⁴, 1) puis max(_, 0)` ; `lerped = origin + ease·(target − origin)` (FMA) ;
    /// `complété = (t ≥ duration) || (ease ≥ 1)` ; `y = (complété || lerped.y < bound_y) ? bound_y :
    /// lerped.y` ; `x,z = lerped`.
    ///
    /// (Le chemin `dt ≤ 0` du binaire renvoie `[r8+0x10]` — un buffer non modélisé ici ; `dt > 0`
    /// dans la boucle de match.)
    #[must_use]
    #[allow(clippy::manual_clamp)] // ordre minss(1.0) PUIS maxss(0.0) du binaire (≠ clamp : sémantique NaN/ordre).
    pub fn step(&mut self, origin: Vec3, dt: f32) -> Vec3 {
        self.t += dt;
        let s = self.t / self.duration;
        let e2 = (s - 1.0) * (s - 1.0);
        let e4 = e2 * e2;
        let ease = (1.0_f32 - e4).min(1.0).max(0.0);
        // `setae` du binaire : t ≥ duration, OU ease ≥ 1 (les deux `or`és dans r8b).
        let complete = self.t >= self.duration || ease >= 1.0;
        // lerped[i] = ease·(target−origin) + origin  (vfmadd231ps = f32::mul_add).
        let lerped = Vec3 {
            x: (self.target.x - origin.x).mul_add(ease, origin.x),
            y: (self.target.y - origin.y).mul_add(ease, origin.y),
            z: (self.target.z - origin.z).mul_add(ease, origin.z),
        };
        let y = if complete || lerped.y < self.bound_y {
            self.bound_y
        } else {
            lerped.y
        };
        Vec3 {
            x: lerped.x,
            y,
            z: lerped.z,
        }
    }
}

/// Mouvement de suivi de cible (easing linéaire borné + clamp de la composante y).
///
/// Port BYTE-FIDÈLE de `game::BallMoveTargetFollow::vmethod_3` (`0x14133c080`, SSE3/4 + FMA3),
/// reversé de l'asm et **validé byte-exact** contre l'émulation Unicorn (`scripts/validate_targetfollow.py`,
/// 3 cas). Les constantes `.data` (offset/biais) sont nulles dans le binaire (no-op).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TargetFollowMove {
    /// Cible suivie, offset `0x70` de l'objet jeu.
    pub target: Vec3,
    /// Plancher de la composante y (`target.y` est borné `>= bound_y`), offset `0x94/0x98`.
    pub bound_y: f32,
    /// Temps écoulé, offset `0xb4`.
    pub t: f32,
    /// Durée totale (≤ 0 ⇒ ease = 1), offset `0xac`.
    pub duration: f32,
}

impl TargetFollowMove {
    /// Avance d'un pas `dt` depuis `origin`. `new_pos = origin + ease·(target_clampé − origin)`,
    /// `ease = clamp(t/duration, 0, 1)` (1 si `duration ≤ 0`), `target.y` borné `≥ bound_y`.
    #[must_use]
    #[allow(clippy::manual_clamp)] // ordre minss(1.0) puis maxss(0.0) du binaire.
    pub fn step(&mut self, origin: Vec3, dt: f32) -> Vec3 {
        self.t += dt;
        let ty = self.target.y.max(self.bound_y);
        let ease = if self.duration > 0.0 {
            (self.t / self.duration).min(1.0).max(0.0)
        } else {
            1.0
        };
        // step[i] = (delta[i]).mul_add(ease, 0) (consts .data = 0) ; new_pos = origin + step.
        Vec3 {
            x: origin.x + (self.target.x - origin.x).mul_add(ease, 0.0),
            y: origin.y + (ty - origin.y).mul_add(ease, 0.0),
            z: origin.z + (self.target.z - origin.z).mul_add(ease, 0.0),
        }
    }
}

/// Indice du point le plus proche de `target` parmi `points` (argmin de la distance²).
///
/// Port BYTE-FIDÈLE de la boucle initiale de `game::BallMoveGoalnet::vmethod_3` (`0x1413385C0`) :
/// recherche du point de filet de but le plus proche du ballon. **Validé byte-exact** contre
/// l'émulation Unicorn (`scripts/validate_goalnet.py`, 4 cas).
///
/// Détails de fidélité f32 (ordre des ops SSE du binaire) : `d2 = (dx·dx + dy·dy) + (dz·dz + 0)`
/// (insertps zéro-w + mulps + 2×haddps) ; min initial `FLT_MAX`, mise à jour **stricte** (`d2 < min`,
/// comiss/jae) ⇒ en cas d'égalité, le **premier** indice gagne. `None` si `points` est vide.
#[must_use]
pub fn nearest_point_index(points: &[Vec3], target: Vec3) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, p) in points.iter().enumerate() {
        let dx = p.x - target.x;
        let dy = p.y - target.y;
        let dz = p.z - target.z;
        // Ordre haddps du binaire : (dx²+dy²) + (dz²+0).
        let d2 = (dx * dx + dy * dy) + (dz * dz + 0.0);
        // Mise à jour si d2 < min courant (strict ⇒ first-win) ; le 1er point passe toujours (None).
        if best.is_none_or(|(_, m)| d2 < m) {
            best = Some((i, d2));
        }
    }
    best.map(|(i, _)| i)
}

/// Position cinématique du contrôleur **Rate** (path-eval `game::BallMoveRate`, `FUN_1413428b0`,
/// décompilé Ghidra puis porté). Projectile : `out = base + horiz·dir̂_xz + vert·Ŷ`, où
/// `horiz = (accel_h·½·t + |dir_xz|)·t` (avance horizontale, normalisée sur le plan XZ),
/// `vert = (accel_v·½·t + dir.y)·t` (déplacement vertical : `v₀·t + ½·a·t²`).
///
/// **Byte-exact, validé vs binaire** (uemu, `scripts/validate_path_eval.py`) APRÈS résolution de la
/// gravité runtime : `DAT_142157570 = (0,1,0,0)` (axe Y) — un const `.bss` copié à l'init depuis
/// `.rdata` (`movaps` à `0x140027528`), retrouvé via xref Ghidra. Méthode « modéliser le runtime » :
/// trouver l'init du const `.bss` → lire la source → valider. La gravité (axe Y) est ici intégrée
/// (le terme vertical va en Y). `accel = (horizontal, vertical)`.
#[must_use]
pub fn rate_path_eval(base: Vec3, dir: Vec3, accel: (f32, f32), t: f32) -> Vec3 {
    let lxz = (dir.x * dir.x + dir.z * dir.z).sqrt();
    let (hx, hz) = if lxz > 0.0 {
        let inv = 1.0 / lxz; // réciproque-produit (normalisation XZ) — ordre f32 du binaire
        (inv * dir.x, inv * dir.z)
    } else {
        (0.0, 0.0)
    };
    let horiz = (accel.0 * 0.5 * t + lxz) * t;
    let vert = (accel.1 * 0.5 * t + dir.y) * t;
    Vec3 {
        x: base.x + horiz * hx,
        y: base.y + vert,
        z: base.z + horiz * hz,
    }
}

/// Intégration de vitesse du contrôleur **Normal** (`game::BallMoveNormal`, `FUN_14133ae10`, lignes
/// 144-163, décompilé Ghidra puis porté). Met à jour la vitesse scalaire + la direction du ballon
/// libre avant la collision : `s = (dt·accel + prev_speed)·factor` ; `new_speed = |s|` ; si `|s| > 0`,
/// `new_dir = ((1/|s|)·s)·dir` (= signe(s)·dir, en **réciproque-produit** fidèle au binaire), sinon
/// `dir` inchangé. Renvoie `(new_dir, new_speed)`.
///
/// **Byte-exact, validé vs binaire** (uemu, `scripts/validate_normal_vel.py`). `accel`/`factor`
/// proviennent des champs de traînée du contrôleur + du contexte d'entrée. (La collision mesh/mur du
/// Normal — `GetComponent`, réflexion — est runtime-couplée et hors de cette pièce de math pure.)
#[must_use]
pub fn normal_integrate_velocity(
    dir: Vec3,
    prev_speed: f32,
    accel: f32,
    dt: f32,
    factor: f32,
) -> (Vec3, f32) {
    let s = (dt * accel + prev_speed) * factor;
    let spd = s.abs();
    if spd > 0.0 {
        let k = (1.0 / spd) * s; // réciproque-produit (= ±1) — ordre f32 du binaire
        (
            Vec3 {
                x: k * dir.x,
                y: k * dir.y,
                z: k * dir.z,
            },
            spd,
        )
    } else {
        (dir, spd)
    }
}

/// Contrôleur **Dribble** (`game::BallMoveDribble`, step au slot 4 `0x14133A8F0`, décompilé Ghidra
/// puis porté). Le ballon suit le joueur qui dribble : la position cible = `sway + position_joueur`
/// avec le rayon du ballon ajouté en Y ; puis le contrôleur stocke direction et vitesse du
/// déplacement.
///
/// # Fidélité (byte-exact par composition)
///
/// Décompilé `FUN_14133a8f0` : `newpos = sway + player + (0, radius, 0)` (additions f32),
/// `vel = (newpos − prev)·(1/dt)` puis `dir = vel/|vel|`, `speed = |vel|`. Les deux dernières
/// étapes sont **exactement** [`Vec3::displacement_rate`] et [`Vec3::normalize_game`], validées
/// byte-exact contre le binaire (uemu). La const par défaut `DAT_142141330 = (0,0,0,0)` (vérifiée)
/// rend le cas `|vel| ≤ 0 → dir nul` identique à `normalize_game`.
///
/// Les entrées (`player`, `sway`, `radius`) sont fournies par le contexte ECS/monde (le binaire les
/// lit via le composant joueur `FUN_141533030` et le singleton monde `DAT_141c27230`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DribbleMove {
    /// Direction normalisée du déplacement (écrite à `+0x40` dans le C++).
    pub direction: Vec3,
    /// Norme `|v|` de la vitesse de déplacement (écrite à `+0x50`).
    pub speed: f32,
}

impl DribbleMove {
    /// Avance le ballon en suivant le joueur dribbleur et renvoie la nouvelle position.
    ///
    /// - `player` : position du joueur qui dribble.
    /// - `sway` : offset horizontal de balancement (calculé par le monde ; `Vec3::zero()` si absent).
    /// - `ball_radius` : rayon du ballon, ajouté en Y.
    /// - `prev` : position précédente du ballon. `dt` : pas de temps.
    ///
    /// Met à jour [`Self::direction`]/[`Self::speed`] et renvoie la position cible.
    #[must_use]
    pub fn step(
        &mut self,
        player: Vec3,
        sway: Vec3,
        ball_radius: f32,
        prev: Vec3,
        dt: f32,
    ) -> Vec3 {
        // newpos = sway + player + (0, radius, 0) — ordre f32 du décompilé : (sway.y+player.y)+radius.
        let newpos = Vec3 {
            x: sway.x + player.x,
            y: (sway.y + player.y) + ball_radius,
            z: sway.z + player.z,
        };
        // vel = (newpos − prev)·(1/dt) ; dir/speed = normalize(vel) — primitives validées byte-exact.
        let vel = newpos.displacement_rate(prev, dt);
        let (dir, speed) = vel.normalize_game();
        self.direction = dir;
        self.speed = speed;
        newpos
    }
}

/// Contrôleur **Bezier** (`game::BallMoveBezier`, step au slot 4 `0x1413359B0`, décompilé Ghidra
/// puis porté). Le ballon suit une courbe de Bézier **quadratique** (3 points de contrôle) puis
/// stocke direction/vitesse — utilisé pour les trajectoires courbes (tirs à effet, passes courbées).
///
/// # Fidélité (byte-exact par composition)
///
/// Décompilé `FUN_1413359b0` : `B(t)` par de Casteljau FMA = [`Vec3::bezier_quadratic`] (validée
/// byte-exact via uemu) ; puis `vel = (B(t) − prev)·(1/dt)`, `dir = vel/|vel|`, `speed = |vel|` =
/// [`Vec3::displacement_rate`] + [`Vec3::normalize_game`] (validées byte-exact). `t = param/total`.
/// (`total ≤ 0` → le point est l'extrémité `p3`.) Les points de contrôle + l'avance de `param` sont
/// gérés par le contexte (le binaire avance `param` via `FUN_1413356e0` et lit la vftable pour la
/// tangente/collision, hors math de trajectoire).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BezierMove {
    /// 1er point de contrôle de la Bézier quadratique (`+0x214`).
    pub p1: Vec3,
    /// 2e point de contrôle (`+0x216`).
    pub p2: Vec3,
    /// 3e point de contrôle / extrémité (`+0x218`).
    pub p3: Vec3,
    /// Avancement courant le long de la courbe (`+0x21c`).
    pub param: f32,
    /// Longueur totale paramétrique (`+0x21a`).
    pub total: f32,
    /// Direction normalisée du déplacement (écrite à `+0x40`).
    pub direction: Vec3,
    /// Norme `|v|` de la vitesse (écrite à `+0x50`).
    pub speed: f32,
}

impl BezierMove {
    /// Avance le ballon le long de la courbe et renvoie la nouvelle position.
    /// `prev` = position précédente, `dt` = pas de temps. Met à jour direction/speed.
    #[must_use]
    pub fn step(&mut self, prev: Vec3, dt: f32) -> Vec3 {
        // t = param/total ; total ≤ 0 → extrémité p3 (le binaire prend P3 directement).
        let pos = if self.total > 0.0 {
            Vec3::bezier_quadratic(self.p1, self.p2, self.p3, self.param / self.total)
        } else {
            self.p3
        };
        let vel = pos.displacement_rate(prev, dt);
        let (dir, speed) = vel.normalize_game();
        self.direction = dir;
        self.speed = speed;
        pos
    }
}

/// Contrôleur **Rate** (`game::BallMoveRate`, step au slot 4 `0x1413390D0`, décompilé Ghidra puis
/// porté). Avance un **paramètre scalaire** le long d'un chemin à un *taux* recalculé pour atteindre
/// la cible dans le temps/distance restant, puis évalue la position 3D via une cinématique de
/// projectile (`FUN_1413428b0`).
///
/// # Fidélité
///
/// La **progression scalaire** (recalcul du taux + avance) est portée byte-exact et **validée vs
/// binaire** (uemu, `scripts/validate_rate_progress.py`, [`RateMove::advance`]). La position 3D
/// (path-eval `FUN_1413428b0`) est une cinématique `base + dir·t + ½·accel·t² + gravité·t` dont la
/// **constante de gravité (`DAT_142157570`) est runtime-init** (zéro en statique) : la formule est
/// portable mais sa validation byte-exact complète exige la valeur runtime (pattern ECS-init) — non
/// incluse ici (INCOMPLET assumé). La vélocité finale = `(pos3d − entrée)·(1/(rate·dt))` puis
/// normalize (primitive validée).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RateMove {
    /// Taux courant (`+0x1a0`).
    pub rate: f32,
    /// Position scalaire le long du chemin (`+0x174`).
    pub position: f32,
    /// Cible paramétrique (`+0x170`).
    pub target: f32,
}

impl RateMove {
    /// Recalcule le taux et avance la position scalaire — **byte-exact vs binaire** (validé uemu).
    ///
    /// `remaining` = valeur du sous-objet d'entrée (`sub[0x10]`) ; si `decrement`, on en retranche la
    /// position courante. Le taux vise à couvrir `target − position` sur le restant. Met à jour
    /// [`Self::rate`]/[`Self::position`]. Renvoie `overshoot` (`target < nouvelle_pos` ou `target ≤ 0`) :
    /// quand vrai, le binaire bascule sur la physique [`BallMoveNormal`].
    ///
    /// Suppose la condition d'entrée du recalcul (`input[0x80]==1 && sub[0x15]≠0`) ; sinon la
    /// progression se réduit à `position += rate·dt` avec le taux existant.
    #[must_use]
    pub fn advance(&mut self, remaining: f32, dt: f32, decrement: bool) -> bool {
        let mut rate = 1.0;
        let r = if decrement {
            remaining - self.position
        } else {
            remaining
        };
        if r > 0.0 {
            let cand = (self.target - self.position) / r;
            if cand > 0.0 {
                rate = cand;
            }
        } else {
            self.position = self.target;
        }
        self.rate = rate;
        self.position += rate * dt; // binaire : rate·dt + pos ; ≡ ici (addition f32 commutative)
        self.target < self.position || self.target <= 0.0
    }
}

/// Contrôleur de mouvement actif du ballon, modélisant la polymorphie `IBallMoveController` du C++
/// (la vftable du contrôleur actif). Dispatche vers les physiques **byte-fidèles** reversées + validées.
///
/// `step(current, dt) → new_pos` : `current` est la position courante du ballon. `Lerp` interpole
/// depuis un `origin` FIXE (capturé au lancement, ≠ position courante — fidèle au binaire qui lit
/// l'origine via `[[r8]+0x1410]`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BallMover {
    /// Aucun mouvement actif (le ballon reste sur place).
    #[default]
    Idle,
    /// Trajectoire parabolique (projectile sous accélération) — [`ParabolaMove`].
    Parabola(ParabolaMove),
    /// Interpolation adoucie depuis une origine fixe — [`LerpMove`].
    Lerp(LerpMove, Vec3),
    /// Suivi de cible avec easing borné — [`TargetFollowMove`].
    TargetFollow(TargetFollowMove),
    /// Trajectoire courbe (Bézier quadratique) — [`BezierMove`] (couvre Bezier/MultiBezier/RealSkillShoot).
    Bezier(BezierMove),
}

impl BallMover {
    /// Avance le ballon d'un pas `dt` depuis `current` et renvoie la nouvelle position.
    /// Chaque branche appelle la physique byte-fidèle correspondante.
    #[must_use]
    pub fn step(&mut self, current: Vec3, dt: f32) -> Vec3 {
        match self {
            BallMover::Idle => current,
            BallMover::Parabola(m) => m.step(current, dt).0,
            BallMover::Lerp(m, origin) => m.step(*origin, dt),
            BallMover::TargetFollow(m) => m.step(current, dt),
            BallMover::Bezier(m) => m.step(current, dt), // prev = position courante
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ball_component_update_uses_validated_physics() {
        // Le ballon piloté par un mover Parabola avance selon la physique byte-fidèle :
        // `update` doit produire EXACTEMENT le même résultat que `ParabolaMove::step` (dispatch correct).
        let p0 = Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let mv = ParabolaMove {
            accel: Vec3 {
                x: 0.0,
                y: -9.8,
                z: 0.0,
            },
            velocity: Vec3 {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
            t: 0.0,
            t_max: 10.0,
        };
        let (expected, _) = { mv }.step(p0, 0.5);
        let mut ball = BallComponent::new();
        ball.position = p0;
        ball.mover = BallMover::Parabola(mv);
        ball.update(0.5);
        assert_eq!(ball.prev_position, p0);
        assert_eq!(ball.position, expected); // dispatch == physique validée vs binaire
    }

    #[test]
    fn nearest_point_index_byte_exact_vs_binaire() {
        // Cas validés vs uemu (scripts/validate_goalnet.py).
        let v = |x: f32, y: f32, z: f32| Vec3 { x, y, z };
        let pts = [v(5.0, 0.0, 0.0), v(1.0, 1.0, 1.0), v(-3.0, 2.0, 4.0)];
        assert_eq!(nearest_point_index(&pts, v(0.0, 0.0, 0.0)), Some(1));
        let pts2 = [v(2.0, 0.0, 0.0), v(0.0, 2.0, 0.0), v(0.0, 0.0, 1.5)];
        assert_eq!(nearest_point_index(&pts2, v(0.1, 0.1, 0.1)), Some(2));
        // Égalité → premier indice (first-win, comparaison stricte du binaire).
        let pts3 = [v(1.0, 1.0, 1.0), v(1.0, 1.0, 1.0)];
        assert_eq!(nearest_point_index(&pts3, v(0.0, 0.0, 0.0)), Some(0));
        // Vide → None.
        assert_eq!(nearest_point_index(&[], v(0.0, 0.0, 0.0)), None);
    }

    #[test]
    fn dribble_step_byte_exact_par_composition() {
        // Port de FUN_14133a8f0 : newpos = sway+player+(0,r,0) (adds f32) ; puis vel=(newpos−prev)·
        // (1/dt) et dir/speed = normalize(vel) — primitives validées byte-exact vs binaire (nie-geom).
        // Cas concret : player=(10,5,0), sway=0, r=0.5, prev=(10,4,0), dt=0.5.
        let mut d = DribbleMove::default();
        let pos = d.step(
            Vec3 {
                x: 10.0,
                y: 5.0,
                z: 0.0,
            }, // joueur
            Vec3::zero(), // sway
            0.5,          // rayon
            Vec3 {
                x: 10.0,
                y: 4.0,
                z: 0.0,
            }, // prev
            0.5,          // dt
        );
        // newpos = (10, (0+5)+0.5, 0) = (10, 5.5, 0)
        assert_eq!(pos.x.to_bits(), 10.0_f32.to_bits());
        assert_eq!(pos.y.to_bits(), 5.5_f32.to_bits());
        assert_eq!(pos.z.to_bits(), 0.0_f32.to_bits());
        // delta=(0,1.5,0) ; vel=delta·2=(0,3,0) ; |vel|=3 ; dir=(0,1,0).
        assert_eq!(d.speed.to_bits(), 3.0_f32.to_bits());
        assert_eq!(d.direction.x.to_bits(), 0.0_f32.to_bits());
        assert_eq!(d.direction.y.to_bits(), 1.0_f32.to_bits());
        assert_eq!(d.direction.z.to_bits(), 0.0_f32.to_bits());
        // dt ≤ 0 → vel = delta brut (pas de division).
        let mut d2 = DribbleMove::default();
        let _ = d2.step(
            Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            Vec3::zero(),
            0.0,
            Vec3 {
                x: 1.0,
                y: 0.0,
                z: 1.0,
            },
            0.0,
        );
        assert_eq!(d2.speed.to_bits(), 1.0_f32.to_bits()); // |delta|=|(0,1,0)|=1
    }

    #[test]
    fn bezier_step_byte_exact_par_composition() {
        // Point Bezier validé byte-exact vs uemu (scripts/validate_bezier.py) ; vitesse/dir = primitives validées.
        let mut b = BezierMove {
            p1: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            p2: Vec3 {
                x: 10.0,
                y: 10.0,
                z: 0.0,
            },
            p3: Vec3 {
                x: 20.0,
                y: 0.0,
                z: 0.0,
            },
            param: 1.0,
            total: 2.0, // t = 0.5
            ..Default::default()
        };
        // B(0.5) = (10, 5, 0) ; prev=(10,5,0) → delta=0 → speed=0, dir=0.
        let pos = b.step(
            Vec3 {
                x: 10.0,
                y: 5.0,
                z: 0.0,
            },
            0.5,
        );
        assert_eq!(pos.x.to_bits(), 10.0_f32.to_bits());
        assert_eq!(pos.y.to_bits(), 5.0_f32.to_bits());
        assert_eq!(pos.z.to_bits(), 0.0_f32.to_bits());
        assert_eq!(b.speed.to_bits(), 0.0_f32.to_bits());
        // total ≤ 0 → extrémité p3.
        let mut b2 = BezierMove {
            p3: Vec3 {
                x: 7.0,
                y: 8.0,
                z: 9.0,
            },
            total: 0.0,
            ..Default::default()
        };
        let pos2 = b2.step(Vec3::zero(), 0.0);
        assert_eq!(
            pos2,
            Vec3 {
                x: 7.0,
                y: 8.0,
                z: 9.0
            }
        );
    }

    #[test]
    fn rate_advance_byte_exact_vs_binaire() {
        // Cas validés byte-exact vs uemu (scripts/validate_rate_progress.py).
        let mut r = RateMove {
            rate: 0.0,
            position: 0.0,
            target: 10.0,
        };
        let overshoot = r.advance(5.0, 0.5, false); // cand=(10-0)/5=2 ; pos=2*0.5+0=1
        assert_eq!(r.rate.to_bits(), 2.0_f32.to_bits());
        assert_eq!(r.position.to_bits(), 1.0_f32.to_bits());
        assert!(!overshoot); // 10 > 1
        // decrement : rem effectif = 5 - pos.
        let mut r2 = RateMove {
            rate: 0.0,
            position: 0.0,
            target: 10.0,
        };
        let _ = r2.advance(5.0, 0.5, true); // r=5-0=5 → cand=2 ; pos=1
        assert_eq!(r2.position.to_bits(), 1.0_f32.to_bits());
        // rem ≤ 0 → pos = target, puis pos += 1.0·dt.
        let mut r3 = RateMove {
            rate: 0.0,
            position: 5.0,
            target: 5.0,
        };
        let _ = r3.advance(2.0, 0.1, false); // r=2>0 ; cand=(5-5)/2=0 → rate reste 1 ; pos=1*0.1+5=5.1
        assert_eq!(r3.rate.to_bits(), 1.0_f32.to_bits());
        assert_eq!(r3.position.to_bits(), 5.1_f32.to_bits());
    }

    #[test]
    fn normal_integrate_velocity_byte_exact_vs_binaire() {
        // Cas validés byte-exact vs uemu (scripts/validate_normal_vel.py).
        let (d, s) = normal_integrate_velocity(
            Vec3 {
                x: 0.6,
                y: 0.8,
                z: 0.0,
            },
            2.0,
            1.0,
            0.5,
            1.0,
        );
        assert_eq!(s.to_bits(), 2.5_f32.to_bits()); // (0.5+2)·1
        assert_eq!(d.x.to_bits(), 0.6_f32.to_bits()); // signe + → dir inchangé
        assert_eq!(d.y.to_bits(), 0.8_f32.to_bits());
        // s négatif → direction inversée.
        let (d2, s2) = normal_integrate_velocity(
            Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            0.5,
            -3.0,
            0.25,
            2.0,
        );
        assert_eq!(s2.to_bits(), 0.5_f32.to_bits()); // |((-0.75+0.5)·2)| = 0.5
        assert_eq!(d2.y.to_bits(), (-1.0_f32).to_bits()); // signe - → -dir
        // |s| = 0 → dir inchangé.
        let (d3, s3) = normal_integrate_velocity(
            Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            1.0,
            0.0,
            0.1,
            0.0,
        );
        assert_eq!(s3.to_bits(), 0.0_f32.to_bits());
        assert_eq!(d3.x.to_bits(), 1.0_f32.to_bits());
    }

    #[test]
    fn rate_path_eval_byte_exact_vs_binaire() {
        // Cas validés byte-exact vs uemu (scripts/validate_path_eval.py, gravité runtime résolue Y).
        // dir horizontal +x, sans accel, t=2 → avance de 2 sur x.
        let p = rate_path_eval(
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            (0.0, 0.0),
            2.0,
        );
        assert_eq!(p.x.to_bits(), 2.0_f32.to_bits());
        assert_eq!(p.y.to_bits(), 0.0_f32.to_bits());
        // dir vertical v0=3, accel_v=-10, t=1 → y = 3·1 + ½·(-10)·1 = -2 (projectile).
        let p2 = rate_path_eval(
            Vec3::zero(),
            Vec3 {
                x: 0.0,
                y: 3.0,
                z: 0.0,
            },
            (0.0, -10.0),
            1.0,
        );
        assert_eq!(p2.y.to_bits(), (-2.0_f32).to_bits());
        assert_eq!(p2.x.to_bits(), 0.0_f32.to_bits()); // |dir_xz|=0 → pas d'horizontal
    }

    #[test]
    fn ball_mover_idle_keeps_position() {
        let mut m = BallMover::Idle;
        let p = Vec3 {
            x: 7.0,
            y: 8.0,
            z: 9.0,
        };
        assert_eq!(m.step(p, 0.5), p);
    }

    #[test]
    fn target_follow_step_byte_exact_vs_binaire() {
        // Cas 1 de scripts/validate_targetfollow.py (validé byte-exact vs binaire).
        let mut m = TargetFollowMove {
            target: Vec3 {
                x: 5.0,
                y: 8.0,
                z: 5.0,
            },
            bound_y: 0.0,
            t: 0.0,
            duration: 2.0,
        };
        let pos = m.step(
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            0.5,
        );
        assert_eq!(pos.x.to_bits(), 1.25_f32.to_bits());
        assert_eq!(pos.y.to_bits(), 2.0_f32.to_bits());
        assert_eq!(pos.z.to_bits(), 1.25_f32.to_bits());
    }

    #[test]
    fn lerp_step_byte_exact_vs_binaire() {
        // Mêmes entrées que scripts/validate_lerp.py (validé byte-exact vs binaire, FMA3 émulée).
        // bound_y=0 + non complété → y=lerped.y (le clamp/snap est un no-op dans ce régime).
        let mut m = LerpMove {
            target: Vec3 {
                x: 10.0,
                y: 20.0,
                z: 30.0,
            },
            t: 0.0,
            duration: 2.0,
            bound_y: 0.0,
        };
        let pos = m.step(
            Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            0.5,
        );
        assert_eq!(pos.x.to_bits(), f32::from_bits(0x40e4_e000).to_bits()); // 7.15234375
        assert_eq!(pos.y.to_bits(), 14.304_687_5_f32.to_bits());
        assert_eq!(pos.z.to_bits(), f32::from_bits(0x41ab_a800).to_bits()); // 21.45703125
        assert_eq!(m.t.to_bits(), 0.5_f32.to_bits());
    }

    #[test]
    fn parabola_step_byte_exact_vs_binaire() {
        // Mêmes entrées que scripts/validate_parabola.py (validé byte-exact vs émulation du binaire).
        let mut m = ParabolaMove {
            accel: Vec3 {
                x: 0.0,
                y: -9.8,
                z: 0.0,
            },
            velocity: Vec3 {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
            t: 0.0,
            t_max: 10.0,
        };
        let (pos, fini) = m.step(
            Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            0.5,
        );
        // Valeurs EXACTES capturées du binaire réel (f32 simple précision).
        assert_eq!(pos.x.to_bits(), 3.0_f32.to_bits());
        assert_eq!(pos.y.to_bits(), f32::from_bits(0x4051_999a).to_bits()); // 3.2750000953674316
        assert_eq!(pos.z.to_bits(), 6.0_f32.to_bits());
        assert_eq!(
            m.velocity.y.to_bits(),
            f32::from_bits(0x3dcc_ccc0).to_bits()
        ); // 0.09999990463256836
        assert_eq!(m.t.to_bits(), 0.5_f32.to_bits());
        assert!(!fini);
    }
    /// Trajectoires **multi-frames** validées byte-exact contre le binaire réel
    /// (`scripts/validate_trajectory.py` : émulation du vrai `vmethod_3` répété, état réinjecté).
    /// Prouve que le ballon piloté par [`BallMover`] vole EXACTEMENT comme nie.exe sur tout l'arc
    /// (pas juste une frame) — valide le câblage runtime, pas une formule isolée.
    #[test]
    fn trajectory_multiframe_byte_exact_vs_binaire() {
        let cmp = |got: Vec3, exp: [u32; 3]| {
            assert_eq!(got.x.to_bits(), exp[0]);
            assert_eq!(got.y.to_bits(), exp[1]);
            assert_eq!(got.z.to_bits(), exp[2]);
        };

        // ── Parabola : p0 réinjecté ; accel=(0,-9.8,0), vel=(4,5,6), t_max=100 ; dt=0.5 ──────
        const PARABOLA: [[u32; 3]; 24] = [
            [0x4040_0000, 0x4051_999A, 0x40C0_0000],
            [0x40A0_0000, 0x4006_6666, 0x4110_0000],
            [0x40E0_0000, 0xBFC3_3335, 0x4140_0000],
            [0x4110_0000, 0xC0F3_3334, 0x4170_0000],
            [0x4130_0000, 0xC181_0000, 0x4190_0000],
            [0x4150_0000, 0xC1D8_CCCD, 0x41A8_0000],
            [0x4170_0000, 0xC222_1999, 0x41C0_0000],
            [0x4188_0000, 0xC261_9998, 0x41D8_0000],
            [0x4198_0000, 0xC295_7332, 0x41F0_0000],
            [0x41A8_0000, 0xC2BE_FFFF, 0x4204_0000],
            [0x41B8_0000, 0xC2ED_7332, 0x4210_0000],
            [0x41C8_0000, 0xC310_6666, 0x421C_0000],
            [0x41D8_0000, 0xC32C_8667, 0x4228_0000],
            [0x41E8_0000, 0xC34B_199B, 0x4234_0000],
            [0x41F8_0000, 0xC36C_2002, 0x4240_0000],
            [0x4204_0000, 0xC387_CCCE, 0x424C_0000],
            [0x420C_0000, 0xC39A_C335, 0x4258_0000],
            [0x4214_0000, 0xC3AE_F335, 0x4264_0000],
            [0x421C_0000, 0xC3C4_5CCF, 0x4270_0000],
            [0x4224_0000, 0xC3DB_0003, 0x427C_0000],
            [0x422C_0000, 0xC3F2_DCD0, 0x4284_0000],
            [0x4234_0000, 0xC405_F99B, 0x428A_0000],
            [0x423C_0000, 0xC413_219B, 0x4290_0000],
            [0x4244_0000, 0xC420_E668, 0x4296_0000],
        ];
        let mut mover = BallMover::Parabola(ParabolaMove {
            accel: Vec3 {
                x: 0.0,
                y: -9.8,
                z: 0.0,
            },
            velocity: Vec3 {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
            t: 0.0,
            t_max: 100.0,
        });
        let mut pos = Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        for exp in PARABOLA {
            pos = mover.step(pos, 0.5);
            cmp(pos, exp);
        }

        // ── Lerp : origine FIXE (1,2,3), target=(10,20,30), dur=5, bound_y=0 ; dt=0.5 ────────
        // Dès la complétion (frame 9, t≥dur), y plonge à bound_y=0 — le comportement réel.
        const LERP: [[u32; 3]; 16] = [
            [0x4083_0B11, 0x4103_0B11, 0x4144_9099],
            [0x40CA_0902, 0x414A_0902, 0x4197_86C2],
            [0x40FA_D9E9, 0x417A_D9E9, 0x41BC_236F],
            [0x410D_566D, 0x418D_566D, 0x41D4_01A4],
            [0x4117_0000, 0x4197_0000, 0x41E2_8000],
            [0x411C_5048, 0x419C_5048, 0x41EA_786C],
            [0x411E_D567, 0x419E_D567, 0x41EE_401B],
            [0x411F_C504, 0x419F_C504, 0x41EF_A786],
            [0x411F_FC50, 0x419F_FC50, 0x41EF_FA78],
            [0x4120_0000, 0x0000_0000, 0x41F0_0000],
            [0x411F_FC50, 0x0000_0000, 0x41EF_FA78],
            [0x411F_C504, 0x0000_0000, 0x41EF_A786],
            [0x411E_D567, 0x0000_0000, 0x41EE_401B],
            [0x411C_5048, 0x0000_0000, 0x41EA_786C],
            [0x4117_0000, 0x0000_0000, 0x41E2_8000],
            [0x410D_566D, 0x0000_0000, 0x41D4_01A4],
        ];
        let origin = Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let mut mover = BallMover::Lerp(
            LerpMove {
                target: Vec3 {
                    x: 10.0,
                    y: 20.0,
                    z: 30.0,
                },
                t: 0.0,
                duration: 5.0,
                bound_y: 0.0,
            },
            origin,
        );
        for exp in LERP {
            cmp(mover.step(origin, 0.5), exp);
        }

        // ── TargetFollow : p0 réinjecté ; target=(5,8,5), dur=4, bound_y=0 ; dt=0.5 ──────────
        const TARGET_FOLLOW: [[u32; 3]; 16] = [
            [0x3F20_0000, 0x3F80_0000, 0x3F20_0000],
            [0x3FDC_0000, 0x4030_0000, 0x3FDC_0000],
            [0x403C_C000, 0x4097_0000, 0x403C_C000],
            [0x407E_6000, 0x40CB_8000, 0x407E_6000],
            [0x4093_B200, 0x40EC_5000, 0x4093_B200],
            [0x409C_EC80, 0x40FB_1400, 0x409C_EC80],
            [0x409F_9D90, 0x40FF_6280, 0x409F_9D90],
            [0x40A0_0000, 0x4100_0000, 0x40A0_0000],
            [0x40A0_0000, 0x4100_0000, 0x40A0_0000],
            [0x40A0_0000, 0x4100_0000, 0x40A0_0000],
            [0x40A0_0000, 0x4100_0000, 0x40A0_0000],
            [0x40A0_0000, 0x4100_0000, 0x40A0_0000],
            [0x40A0_0000, 0x4100_0000, 0x40A0_0000],
            [0x40A0_0000, 0x4100_0000, 0x40A0_0000],
            [0x40A0_0000, 0x4100_0000, 0x40A0_0000],
            [0x40A0_0000, 0x4100_0000, 0x40A0_0000],
        ];
        let mut mover = BallMover::TargetFollow(TargetFollowMove {
            target: Vec3 {
                x: 5.0,
                y: 8.0,
                z: 5.0,
            },
            bound_y: 0.0,
            t: 0.0,
            duration: 4.0,
        });
        let mut pos = Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        for exp in TARGET_FOLLOW {
            pos = mover.step(pos, 0.5);
            cmp(pos, exp);
        }
    }

    use crate::{BALL_GRAVITY, BALL_SCALE_DEFAULT, DISTANCE_UNINIT, INVALID_PLAYER_IDX};

    #[test]
    fn ball_component_default_gravity() {
        let ball = BallComponent::new();
        // 0x40000000 = 2.0f en IEEE 754 — confirmé par ball_component.c
        assert_eq!(ball.gravity.to_bits(), 0x4000_0000);
        assert_eq!(ball.gravity, BALL_GRAVITY);
    }

    #[test]
    fn ball_component_default_scale() {
        let ball = BallComponent::new();
        // 0x3F800000 = 1.0f en IEEE 754 — confirmé par ball_component.c offset 0x2ea
        assert_eq!(ball.scale.to_bits(), 0x3F80_0000);
        assert_eq!(ball.scale, BALL_SCALE_DEFAULT);
    }

    #[test]
    fn ball_component_default_distances() {
        let ball = BallComponent::new();
        // 0xBF800000 = -1.0f — confirmé par ball_component.c offsets 0x1764, 0x176c
        assert_eq!(ball.detection_dist_primary.to_bits(), 0xBF80_0000);
        assert_eq!(ball.detection_dist_primary, DISTANCE_UNINIT);
        assert_eq!(ball.detection_dist_secondary, DISTANCE_UNINIT);
    }

    #[test]
    fn ball_component_default_max_collisions() {
        let ball = BallComponent::new();
        // ball_component.c : *(undefined1 *)(param_1 + 0x148a) = 5
        assert_eq!(ball.max_collisions_per_frame, 5);
    }

    #[test]
    fn ball_component_possession_ids_default() {
        let ball = BallComponent::new();
        // Tous les IDs de possession doivent être 0xFF (INVALID_PLAYER_IDX)
        assert_eq!(ball.possession.owner, INVALID_PLAYER_IDX);
        assert_eq!(ball.possession.secondary, INVALID_PLAYER_IDX);
        assert_eq!(ball.possession.tertiary, INVALID_PLAYER_IDX);
        assert!(ball.possession.is_free());
    }

    #[test]
    fn ball_component_target_ids_default() {
        let ball = BallComponent::new();
        // Cibles initialisées à 0xFFFF0000
        assert_eq!(ball.targets.primary, 0xFFFF_0000);
        assert_eq!(ball.targets.secondary, 0xFFFF_0000);
        assert!(ball.targets.has_no_target());
    }

    #[test]
    fn ball_component_default_controller_is_target_follow() {
        let ball = BallComponent::new();
        // Le ctor pose BallMoveTargetFollow::vftable en dernier
        assert_eq!(ball.move_controller, BallMoveKind::TargetFollow);
    }

    #[test]
    fn ball_component_set_owner() {
        let mut ball = BallComponent::new();
        assert!(ball.is_free());
        ball.set_owner(3);
        assert!(!ball.is_free());
        assert_eq!(ball.possession.owner, 3);
    }

    #[test]
    fn ball_component_release_possession() {
        let mut ball = BallComponent::new();
        ball.set_owner(5);
        ball.targets.primary = 42;
        ball.release_possession();
        assert!(ball.is_free());
        assert!(ball.targets.has_no_target());
    }

    #[test]
    fn ball_move_normal_default_on_ground_false() {
        let mv = BallMoveNormal::default();
        // ball_component.c : *(undefined1 *)(param_1 + 0x18) = 0 dans FUN_14027ac10
        assert!(!mv.on_ground);
        assert!(!mv.collision_flag);
    }

    #[test]
    fn possession_ids_is_free() {
        let p = PossessionIds::default();
        assert!(p.is_free());
        let p2 = PossessionIds {
            owner: 0,
            secondary: 0xFF,
            tertiary: 0xFF,
        };
        assert!(!p2.is_free());
    }

    #[test]
    fn target_ids_has_no_target() {
        let t = TargetIds::default();
        assert!(t.has_no_target());
        let t2 = TargetIds {
            primary: 1,
            secondary: crate::INVALID_TARGET_ID,
        };
        assert!(!t2.has_no_target());
    }
}
