//! **Moteur de jeu niers** — boucle intégrée qui TOURNE (état-monde + physique + gameplay),
//! en Rust pur, headless et **déterministe**. Le rendu top-down vit dans [`render`], la boucle
//! et la sortie vidéo dans le binaire `nie-runtime`.
//!
//! C'est le *cadre* exécutable du moteur : une simulation de football jouable de bout en bout
//! (22 joueurs + ballon, physique, possession, buts) où la logique reversée de IEVR se branche
//! incrémentalement. Ancrages RE réels : la **gravité du ballon = [`nie_core::BALL_GRAVITY`]**
//! (`2.0`, bits IEEE `0x40000000`, confirmés dans `ball_component.c`), 11 v 11, un GK par camp.
//!
//! Ce qui est **propre au moteur niers** (simulation temps-réel jouable) est distinct de ce qui
//! est **porté byte-exact** de IEVR (constantes, structures). La physique PhysX exacte et la
//! résolution de but event-driven de IEVR sont des pistes séparées (cf. `nie-engine`, le système
//! d'événements reversé) qui remplaceront progressivement les approximations d'ici.

#![forbid(unsafe_code)]

pub mod render;

use nie_core::BALL_GRAVITY;

// Vecteurs : SOURCE UNIQUE `nie_geom` (dédup Phase 2). Alias V2/V3 pour préserver le code local.
// Convention nie-runtime : `z` = hauteur (vit dans le code ; le type est axis-agnostique).
// ⚠ Ne pas convertir vers/depuis `nie_core::Vec3` (y=hauteur) — cf. `docs/ARCHITECTURE.md` landmine #4.
use nie_geom::{Vec2 as V2, Vec3 as V3};

// ── Dimensions du terrain (mètres, origine au centre) ───────────────────────────
/// Demi-longueur (but à but) : terrain 105 m.
pub const HALF_LEN: f32 = 52.5;
/// Demi-largeur (touche à touche) : terrain 68 m.
pub const HALF_WID: f32 = 34.0;
/// Demi-largeur des buts (largeur réglementaire 7,32 m).
pub const GOAL_HALF: f32 = 3.66;
/// Hauteur de la barre transversale (2,44 m).
pub const GOAL_HEIGHT: f32 = 2.44;

// ── Paramètres physiques (unités-jeu ; gravité ancrée sur la RE) ─────────────────
/// Restitution du ballon au sol (rebond).
const RESTITUTION: f32 = 0.6;
/// Friction de roulement au sol (par seconde).
const GROUND_FRICTION: f32 = 0.9;
/// Traînée aérienne légère (par seconde).
const AIR_DRAG: f32 = 0.05;
/// Rayon de contrôle du ballon par un joueur (mètres).
const CONTROL_RADIUS: f32 = 1.4;
/// Rayon de tacle : un adverse à cette distance vole la possession (< contrôle → hystérésis).
const STEAL_RADIUS: f32 = 0.7;
/// Rayon de parade du gardien : il dégage un tir/ballon bas passant à cette distance (m).
const SAVE_RADIUS: f32 = 1.5;
/// Distance au but adverse en deçà de laquelle le porteur tire au lieu de dribbler (m).
const SHOOT_RANGE: f32 = 18.0;
/// Vitesse de dribble du ballon poussé devant le porteur (m/s).
const DRIBBLE_SPEED: f32 = 9.0;
/// Vitesse de course max d'un joueur (m/s).
const PLAYER_SPEED: f32 = 7.0;
/// Puissance de frappe (m/s) imprimée au ballon vers le but adverse.
const KICK_POWER: f32 = 26.0;
/// Composante verticale d'une frappe (loft) — basse pour des tirs tendus qui restent sous la barre.
const KICK_LOFT: f32 = 1.0;
/// Cadence minimale entre deux frappes du même porteur (s).
const KICK_COOLDOWN: f32 = 0.35;
/// Durée d'immunité au tacle après une récupération (s) — crée des courses d'attaque nettes.
const POSSESSION_LOCK: f32 = 0.55;

/// Rôle d'un joueur sur le terrain (détermine sa zone de base).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Goalkeeper,
    Defender,
    Midfielder,
    Forward,
}

/// Un joueur : position/vitesse au sol, équipe (0 = domicile attaque +x, 1 = extérieur attaque −x).
#[derive(Debug, Clone, Copy)]
pub struct Player {
    pub pos: V2,
    pub vel: V2,
    pub team: u8,
    pub role: Role,
    /// Position de base (ancrage de formation) vers laquelle le joueur revient hors action.
    pub home: V2,
}

/// Le ballon : position 3D + vitesse 3D + gravité (ancrée RE).
#[derive(Debug, Clone, Copy)]
pub struct Ball {
    pub pos: V3,
    pub vel: V3,
    pub gravity: f32,
}

impl Default for Ball {
    fn default() -> Self {
        Self {
            pos: V3::new(0.0, 0.0, 0.11),
            vel: V3::default(),
            gravity: BALL_GRAVITY,
        }
    }
}

/// Ce que la joueuse demande au joueur qu'elle contrôle, pour le pas de simulation à venir.
///
/// Un état, pas un événement : une direction se **maintient** tant qu'une touche est enfoncée,
/// là où un menu réagit à des appuis. Le front remplit cette structure à chaque image ; le moteur
/// ne sait pas d'où elle vient (clavier, manette, rejeu enregistré).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Input {
    /// Direction voulue, en mètres par seconde normalisés — zéro = le joueur s'arrête.
    pub dir: V2,
    /// Frapper le ballon cette image, si le joueur contrôlé le possède.
    pub shoot: bool,
}

/// État complet du monde simulé à un instant donné.
#[derive(Debug, Clone)]
pub struct World {
    pub ball: Ball,
    pub players: Vec<Player>,
    /// Entrée de la joueuse pour le prochain [`World::step`].
    ///
    /// Laissée à zéro, la simulation reste **exactement** ce qu'elle était : les 22 joueurs sont
    /// pilotés par l'IA. C'est ce qui permet d'ajouter le contrôle sans invalider les rejeux
    /// déterministes ni les tests de simulation existants.
    pub input: Input,
    /// Buts marqués `[domicile, extérieur]`.
    pub score: [u32; 2],
    /// Temps de jeu simulé (s).
    pub time: f32,
    /// Compteur de pas de simulation (déterministe).
    pub tick: u64,
    /// Index du dernier porteur (pour le cooldown de frappe), et son chrono.
    possessor: Option<usize>,
    kick_timer: f32,
    /// Immunité au tacle après récupération (s) : donne au porteur un burst d'attaque net.
    steal_lock: f32,
}

/// Formation 4-4-2 en fractions du demi-terrain pour l'équipe domicile (attaque +x).
/// `(x_frac, y_frac)` ∈ [−1, 1] ; mappé en mètres par [`HALF_LEN`]/[`HALF_WID`].
const FORMATION_442: [(f32, f32, Role); 11] = [
    (-0.95, 0.0, Role::Goalkeeper),
    (-0.6, -0.7, Role::Defender),
    (-0.6, -0.25, Role::Defender),
    (-0.6, 0.25, Role::Defender),
    (-0.6, 0.7, Role::Defender),
    (-0.15, -0.7, Role::Midfielder),
    (-0.15, -0.25, Role::Midfielder),
    (-0.15, 0.25, Role::Midfielder),
    (-0.15, 0.7, Role::Midfielder),
    (-0.3, -0.2, Role::Forward),
    (-0.3, 0.2, Role::Forward),
];

impl World {
    /// Coup d'envoi : 22 joueurs en 4-4-2 (domicile + extérieur miroir), ballon au centre.
    #[must_use]
    pub fn kickoff() -> Self {
        let mut players = Vec::with_capacity(22);
        for &(xf, yf, role) in &FORMATION_442 {
            // Domicile : attaque +x, sa moitié = −x.
            let home0 = V2::new(xf * HALF_LEN, yf * HALF_WID);
            players.push(Player {
                pos: home0,
                vel: V2::default(),
                team: 0,
                role,
                home: home0,
            });
            // Extérieur : miroir (attaque −x).
            let home1 = V2::new(-xf * HALF_LEN, yf * HALF_WID);
            players.push(Player {
                pos: home1,
                vel: V2::default(),
                team: 1,
                role,
                home: home1,
            });
        }
        Self {
            ball: Ball::default(),
            players,
            input: Input::default(),
            score: [0, 0],
            time: 0.0,
            tick: 0,
            possessor: None,
            kick_timer: 0.0,
            steal_lock: 0.0,
        }
    }

    /// Index du joueur que la joueuse contrôle, dans l'équipe **domicile**.
    ///
    /// Le porteur s'il est de cette équipe, sinon le joueur de champ le plus proche du ballon —
    /// la convention de tous les jeux de football : on tient celui qui compte, et le contrôle
    /// bascule tout seul. Le gardien est exclu : le laisser quitter sa cage parce que le ballon
    /// passe près de lui offrirait le but adverse.
    #[must_use]
    pub fn controlled(&self) -> Option<usize> {
        if let Some(i) = self.possessor
            && self.players.get(i).is_some_and(|p| p.team == 0)
        {
            return Some(i);
        }
        let ball2 = self.ball.pos.ground();
        self.players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.team == 0 && p.role != Role::Goalkeeper)
            .min_by(|(_, a), (_, b)| {
                let (da, db) = ((a.pos - ball2).len(), (b.pos - ball2).len());
                da.partial_cmp(&db).unwrap_or(core::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    /// Avance la simulation d'un pas `dt` (secondes). Déterministe.
    pub fn step(&mut self, dt: f32) {
        self.tick += 1;
        self.time += dt;
        if self.kick_timer > 0.0 {
            self.kick_timer -= dt;
        }
        if self.steal_lock > 0.0 {
            self.steal_lock -= dt;
        }
        self.step_players(dt);
        self.step_ball(dt);
        self.resolve_possession(dt);
        self.keeper_save();
        if self.detect_goal() {
            self.reset_after_goal();
        }
    }

    /// Déplace chaque joueur : le **porteur** fonce vers le but adverse (attaque), le plus proche
    /// de l'équipe défendante chasse le ballon (sauf le GK qui garde sa cage), les autres tiennent
    /// leur position de formation.
    fn step_players(&mut self, dt: f32) {
        let ball2 = self.ball.pos.ground();
        let carrier = self.possessor;
        // Joueur sous contrôle : seulement s'il y a une direction demandée. Sans entrée, il
        // reste piloté par l'IA — la simulation autonome doit rester identique au bit près.
        let pilote = (self.input.dir.len() > 0.01)
            .then(|| self.controlled())
            .flatten();
        // Plus proche par équipe.
        let mut nearest = [usize::MAX, usize::MAX];
        let mut best = [f32::MAX, f32::MAX];
        for (i, p) in self.players.iter().enumerate() {
            let d = (p.pos - ball2).len();
            let t = p.team as usize;
            if d < best[t] {
                best[t] = d;
                nearest[t] = i;
            }
        }
        for (i, p) in self.players.iter_mut().enumerate() {
            let target = if carrier == Some(i) {
                // Porteur : sprinte vers le but adverse en GARDANT sa position latérale
                // (jeu 2D ; léger recentrage pour finir face au but).
                V2::new(
                    if p.team == 0 { HALF_LEN } else { -HALF_LEN },
                    p.pos.y * 0.85,
                )
            } else if nearest[p.team as usize] == i
                && !(p.role == Role::Goalkeeper && ball2.x.abs() < HALF_LEN * 0.5)
            {
                // Chasseur : récupère le ballon (le GK ne sort pas loin de sa cage).
                ball2
            } else {
                p.home
            };
            // Le joueur pilote obéit à la direction demandée, pas à sa cible d'IA.
            let dir = if pilote == Some(i) {
                self.input.dir.norm()
            } else {
                (target - p.pos).norm()
            };
            p.vel = dir * PLAYER_SPEED;
            p.pos = p.pos + p.vel * dt;
            p.pos.x = p.pos.x.clamp(-HALF_LEN, HALF_LEN);
            p.pos.y = p.pos.y.clamp(-HALF_WID, HALF_WID);
        }
    }

    /// Physique du ballon : intégrateur d'Euler semi-implicite + gravité + rebond + frictions.
    fn step_ball(&mut self, dt: f32) {
        let b = &mut self.ball;
        // Gravité (descend) + traînée aérienne.
        b.vel.z -= b.gravity * dt;
        let drag = 1.0 - AIR_DRAG * dt;
        b.vel.x *= drag;
        b.vel.y *= drag;
        // Intégration position.
        b.pos.x += b.vel.x * dt;
        b.pos.y += b.vel.y * dt;
        b.pos.z += b.vel.z * dt;
        // Sol : rebond + friction de roulement.
        if b.pos.z <= 0.0 {
            b.pos.z = 0.0;
            if b.vel.z < 0.0 {
                b.vel.z = -b.vel.z * RESTITUTION;
                if b.vel.z < 0.2 {
                    b.vel.z = 0.0;
                }
            }
            let roll = GROUND_FRICTION.powf(dt);
            b.vel.x *= roll;
            b.vel.y *= roll;
        }
        // Rebond sur les touches (la ligne de but est gérée par detect_goal).
        if b.pos.y.abs() > HALF_WID {
            b.pos.y = b.pos.y.clamp(-HALF_WID, HALF_WID);
            b.vel.y = -b.vel.y * RESTITUTION;
        }
    }

    /// Possession **collante** + dribble/tir. Le porteur courant garde le ballon tant qu'aucun
    /// adverse n'entre dans le rayon de tacle ([`STEAL_RADIUS`]) ; il dribble vers le but adverse
    /// et frappe une fois à portée de tir ([`SHOOT_RANGE`]). Crée un jeu directionnel (→ buts),
    /// au lieu d'une oscillation centrale.
    fn resolve_possession(&mut self, _dt: f32) {
        let ball2 = self.ball.pos.ground();
        let low = self.ball.pos.z < 0.6;

        // Plus proche joueur (global) et son équipe.
        let mut nearest = (usize::MAX, f32::MAX);
        for (i, p) in self.players.iter().enumerate() {
            let d = (p.pos - ball2).len();
            if d < nearest.1 {
                nearest = (i, d);
            }
        }

        // Porteur courant conservé s'il reste à portée de contrôle.
        let prev = self.possessor;
        let keep = prev.filter(|&i| (self.players[i].pos - ball2).len() < CONTROL_RADIUS);
        let owner = match keep {
            // Conserve sauf si un ADVERSE entre dans le rayon de tacle (hors verrou de possession).
            Some(c) => {
                let opp_steal = self.steal_lock <= 0.0
                    && nearest.0 != usize::MAX
                    && self.players[nearest.0].team != self.players[c].team
                    && nearest.1 < STEAL_RADIUS;
                if opp_steal { Some(nearest.0) } else { Some(c) }
            }
            // Sinon le plus proche prend, s'il est à portée de contrôle.
            None if nearest.1 < CONTROL_RADIUS => Some(nearest.0),
            None => None,
        };
        self.possessor = owner;
        if owner.is_some() && owner != prev {
            self.steal_lock = POSSESSION_LOCK; // burst d'attaque à la récupération
        }

        let Some(i) = owner else { return };
        if !low {
            return; // ballon en l'air : pas de contrôle au sol.
        }
        let team = self.players[i].team;

        // Frappe commandée : quand la joueuse tient le ballon et appuie sur tir, elle frappe
        // MAINTENANT, dans la direction qu'elle demande — pas quand l'IA jugerait bon de le
        // faire. Sans direction, la frappe part vers le but adverse, comme un dégagement.
        if self.input.shoot && self.kick_timer <= 0.0 && Some(i) == self.controlled() {
            let vise = if self.input.dir.len() > 0.01 {
                self.input.dir.norm()
            } else {
                (V2::new(if team == 0 { HALF_LEN } else { -HALF_LEN }, 0.0) - ball2).norm()
            };
            self.ball.vel = V3::new(vise.x * KICK_POWER, vise.y * KICK_POWER, KICK_LOFT);
            self.kick_timer = KICK_COOLDOWN;
            return;
        }

        // But adverse : domicile (0) → +x, extérieur (1) → −x. On vise le but en suivant la
        // position latérale du PORTEUR (jeu/tirs 2D au lieu d'un axe central dégénéré) ; la cible
        // reste dans la largeur du but à l'approche.
        let goal_x = if team == 0 { HALF_LEN } else { -HALF_LEN };
        // Dribble : pousse le ballon vers le but en suivant l'angle latéral du porteur.
        let drib_y = (self.players[i].pos.y * 0.7).clamp(-HALF_WID * 0.5, HALF_WID * 0.5);
        let to_goal = V2::new(goal_x, drib_y) - ball2;
        if to_goal.len() < SHOOT_RANGE && self.kick_timer <= 0.0 {
            // Tir placé : vise le poteau OPPOSÉ au gardien adverse (le bat s'il est mal placé).
            let gk_y = self
                .players
                .iter()
                .find(|p| p.team != team && p.role == Role::Goalkeeper)
                .map_or(0.0, |g| g.pos.y);
            let post = if gk_y >= 0.0 {
                -GOAL_HALF * 0.82
            } else {
                GOAL_HALF * 0.82
            };
            let dir = (V2::new(goal_x, post) - ball2).norm();
            self.ball.vel = V3::new(dir.x * KICK_POWER, dir.y * KICK_POWER, KICK_LOFT);
            self.kick_timer = KICK_COOLDOWN;
        } else {
            let dir = to_goal.norm();
            self.ball.vel.x = dir.x * DRIBBLE_SPEED;
            self.ball.vel.y = dir.y * DRIBBLE_SPEED;
        }
    }

    /// Parade du gardien : si un ballon **bas** approche le but d'une équipe et que son GK est à
    /// portée ([`SAVE_RADIUS`]), il le dégage vers le terrain et en reprend possession. Donne à la
    /// défense un vrai outil → les buts se méritent (et le match s'équilibre).
    fn keeper_save(&mut self) {
        if self.ball.pos.z >= GOAL_HEIGHT {
            return; // au-dessus de la barre : pas parable au sol.
        }
        let ball2 = self.ball.pos.ground();
        for team in 0u8..2 {
            let own_goal_x = if team == 0 { -HALF_LEN } else { HALF_LEN };
            if (ball2.x - own_goal_x).abs() > 16.5 {
                continue; // hors du tiers défensif proche du but.
            }
            let Some(gk) = self
                .players
                .iter()
                .position(|p| p.team == team && p.role == Role::Goalkeeper)
            else {
                continue;
            };
            if (self.players[gk].pos - ball2).len() < SAVE_RADIUS {
                // Dégagement en cloche douce vers le centre du terrain.
                let away = if team == 0 { 1.0 } else { -1.0 };
                self.ball.vel = V3::new(away * 9.0, ball2.y * -0.2, 3.0);
                self.possessor = Some(gk);
                self.kick_timer = KICK_COOLDOWN;
            }
        }
    }

    /// `true` si le ballon a franchi une ligne de but entre les poteaux et sous la barre.
    fn detect_goal(&mut self) -> bool {
        let b = self.ball.pos;
        let inside = b.y.abs() < GOAL_HALF && b.z < GOAL_HEIGHT;
        if b.x > HALF_LEN && inside {
            self.score[0] += 1; // domicile marque dans le but +x
            true
        } else if b.x < -HALF_LEN && inside {
            self.score[1] += 1;
            true
        } else {
            // Sortie de but sans but : ramener le ballon dans le terrain (rebond simple).
            if b.x.abs() > HALF_LEN {
                self.ball.pos.x = b.x.clamp(-HALF_LEN, HALF_LEN);
                self.ball.vel.x = -self.ball.vel.x * RESTITUTION;
            }
            false
        }
    }

    /// Remet le ballon au centre et les joueurs à leur formation après un but.
    fn reset_after_goal(&mut self) {
        self.ball = Ball::default();
        for p in &mut self.players {
            p.pos = p.home;
            p.vel = V2::default();
        }
        self.possessor = None;
        self.kick_timer = KICK_COOLDOWN;
        self.steal_lock = 0.0;
    }

    /// Index du porteur actuel (le plus proche à portée de contrôle), s'il y en a un.
    #[must_use]
    pub fn possessor(&self) -> Option<usize> {
        self.possessor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kickoff_pose_22_joueurs_et_ballon_au_centre() {
        let w = World::kickoff();
        assert_eq!(w.players.len(), 22);
        assert_eq!(w.players.iter().filter(|p| p.team == 0).count(), 11);
        assert_eq!(
            w.players
                .iter()
                .filter(|p| p.role == Role::Goalkeeper)
                .count(),
            2
        );
        assert!(w.ball.pos.ground().len() < 0.01, "ballon au centre");
        assert_eq!(w.ball.gravity, BALL_GRAVITY);
    }

    #[test]
    fn ballon_tombe_sous_la_gravite() {
        let mut w = World::kickoff();
        w.players.clear(); // pas d'interférence de possession
        w.ball.pos = V3::new(0.0, 0.0, 5.0);
        w.ball.vel = V3::default();
        let z0 = w.ball.pos.z;
        for _ in 0..10 {
            w.step(1.0 / 60.0);
        }
        assert!(
            w.ball.pos.z < z0,
            "le ballon descend ({} < {z0})",
            w.ball.pos.z
        );
    }

    #[test]
    fn ballon_rebondit_au_sol_avec_perte() {
        let mut w = World::kickoff();
        w.players.clear();
        w.ball.pos = V3::new(0.0, 0.0, 0.05);
        w.ball.vel = V3::new(0.0, 0.0, -5.0);
        w.step(1.0 / 60.0);
        assert!(
            w.ball.vel.z > 0.0,
            "rebond : vitesse verticale inversée ({})",
            w.ball.vel.z
        );
        assert!(w.ball.vel.z < 5.0, "rebond amorti (restitution < 1)");
    }

    #[test]
    fn joueur_le_plus_proche_converge_vers_le_ballon() {
        let mut w = World::kickoff();
        w.ball.pos = V3::new(0.0, 0.0, 0.11);
        let nearest = w
            .players
            .iter()
            .enumerate()
            .min_by(|a, b| {
                a.1.pos
                    .len()
                    .partial_cmp(&b.1.pos.len())
                    .unwrap_or(core::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap();
        let d0 = w.players[nearest].pos.len();
        for _ in 0..30 {
            w.step(1.0 / 60.0);
        }
        let d1 = (w.players[nearest].pos - w.ball.pos.ground()).len();
        assert!(d1 < d0, "le joueur se rapproche du ballon ({d1} < {d0})");
    }

    #[test]
    fn but_incremente_le_score_et_relance_au_centre() {
        let mut w = World::kickoff();
        w.players.clear();
        // Ballon juste devant la ligne de but +x, entre les poteaux, à ras de terre, lancé dedans.
        w.ball.pos = V3::new(HALF_LEN - 0.1, 0.0, 0.1);
        w.ball.vel = V3::new(30.0, 0.0, 0.0);
        w.step(1.0 / 60.0);
        assert_eq!(w.score, [1, 0], "but domicile compté");
        assert!(w.ball.pos.ground().len() < 0.01, "ballon relancé au centre");
    }

    #[test]
    fn simulation_deterministe() {
        let run = || {
            let mut w = World::kickoff();
            for _ in 0..600 {
                w.step(1.0 / 60.0);
            }
            (w.score, w.ball.pos, w.players[0].pos, w.tick)
        };
        let a = run();
        let b = run();
        assert_eq!(a.0, b.0);
        assert_eq!(a.1, b.1);
        assert_eq!(a.2, b.2);
        assert_eq!(a.3, b.3);
    }
}
