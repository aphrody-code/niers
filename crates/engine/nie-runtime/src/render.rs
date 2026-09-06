//! Rendu **top-down CPU** du [`World`](crate::World) en tampon RGBA8 — terrain, lignes, joueurs,
//! ballon (avec ombre/hauteur). Zéro GPU : déterministe et headless. C'est le « moteur d'affichage »
//! minimal du moteur niers ; il sera remplaçable par le pipeline wgpu/3D sans toucher la simulation.

// La rastérisation convertit en permanence des coordonnées f32 en indices de pixels.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use crate::{GOAL_HALF, HALF_LEN, HALF_WID, Role, World};

/// Marge autour du terrain, en pixels.
const MARGIN: f32 = 28.0;

/// Couleur RGBA.
type Rgba = [u8; 4];

const PITCH: Rgba = [34, 139, 53, 255];
const PITCH_ALT: Rgba = [40, 150, 58, 255];
const LINE: Rgba = [235, 240, 235, 255];
const HOME: Rgba = [40, 110, 230, 255];
const AWAY: Rgba = [225, 60, 60, 255];
const KEEPER_H: Rgba = [250, 230, 60, 255];
const KEEPER_A: Rgba = [60, 230, 200, 255];
const BALL: Rgba = [250, 250, 250, 255];
const SHADOW: Rgba = [20, 70, 30, 255];
/// Curseur du joueur dirigé : magenta.
///
/// Aucune autre teinte du terrain ne s'en approche — le jaune, essayé d'abord, est exactement
/// celui du gardien domicile, et le curseur s'y fondait.
const CURSEUR: Rgba = [255, 60, 235, 255];

/// Tampon image RGBA8 mutable.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Frame {
    #[must_use]
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            px: vec![0; (w * h * 4) as usize],
        }
    }

    fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        self.px[i..i + 4].copy_from_slice(&c);
    }

    /// Mélange `c` (alpha) sur le pixel.
    fn blend(&mut self, x: i32, y: i32, c: Rgba) {
        if c[3] == 255 {
            self.set(x, y, c);
            return;
        }
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        let a = f32::from(c[3]) / 255.0;
        for (k, &cv) in c.iter().take(3).enumerate() {
            let dst = f32::from(self.px[i + k]);
            self.px[i + k] = (f32::from(cv) * a + dst * (1.0 - a)) as u8;
        }
        self.px[i + 3] = 255;
    }

    fn fill_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.set(x, y, c);
            }
        }
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        let r2 = r * r;
        let (x0, x1) = ((cx - r) as i32, (cx + r) as i32 + 1);
        let (y0, y1) = ((cy - r) as i32, (cy + r) as i32 + 1);
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                if dx * dx + dy * dy <= r2 {
                    self.blend(x, y, c);
                }
            }
        }
    }

    /// Anneau (cercle non rempli) d'épaisseur `t`.
    fn ring(&mut self, cx: f32, cy: f32, r: f32, t: f32, c: Rgba) {
        let (ro, ri) = (r + t * 0.5, (r - t * 0.5).max(0.0));
        let (x0, x1) = ((cx - ro) as i32, (cx + ro) as i32 + 1);
        let (y0, y1) = ((cy - ro) as i32, (cy + ro) as i32 + 1);
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let d = (dx * dx + dy * dy).sqrt();
                if d <= ro && d >= ri {
                    self.blend(x, y, c);
                }
            }
        }
    }

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, t: f32, c: Rgba) {
        let n = ((x1 - x0).abs().max((y1 - y0).abs()) as i32).max(1);
        for i in 0..=n {
            let s = i as f32 / n as f32;
            self.disc(x0 + (x1 - x0) * s, y0 + (y1 - y0) * s, t * 0.5, c);
        }
    }
}

/// Caméra orthographique top-down : mappe les mètres du terrain en pixels.
struct Cam {
    ox: f32,
    oy: f32,
    sx: f32,
    sy: f32,
}

impl Cam {
    fn new(w: u32, h: u32) -> Self {
        let iw = w as f32 - 2.0 * MARGIN;
        let ih = h as f32 - 2.0 * MARGIN;
        Self {
            ox: MARGIN,
            oy: MARGIN,
            sx: iw / (2.0 * HALF_LEN),
            sy: ih / (2.0 * HALF_WID),
        }
    }
    /// (mètres terrain) → (pixels). `+x` à droite, `+y` vers le bas.
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.ox + (x + HALF_LEN) * self.sx,
            self.oy + (y + HALF_WID) * self.sy,
        )
    }
}

/// Rend `world` dans un tampon RGBA8 de `w`×`h`.
#[must_use]
pub fn render(world: &World, w: u32, h: u32) -> Frame {
    let mut f = Frame::new(w, h);
    let cam = Cam::new(w, h);

    // Pelouse + bandes (tonte) alternées.
    f.fill_rect(0, 0, w as i32, h as i32, PITCH);
    let stripes = 12;
    for i in 0..stripes {
        if i % 2 == 0 {
            let x = -HALF_LEN + (2.0 * HALF_LEN) * (i as f32 / stripes as f32);
            let x2 = -HALF_LEN + (2.0 * HALF_LEN) * ((i + 1) as f32 / stripes as f32);
            let (px0, py0) = cam.p(x, -HALF_WID);
            let (px1, py1) = cam.p(x2, HALF_WID);
            f.fill_rect(px0 as i32, py0 as i32, px1 as i32, py1 as i32, PITCH_ALT);
        }
    }

    // Lignes du terrain.
    let lt = 2.2;
    let (tl_x, tl_y) = cam.p(-HALF_LEN, -HALF_WID);
    let (br_x, br_y) = cam.p(HALF_LEN, HALF_WID);
    f.line(tl_x, tl_y, br_x, tl_y, lt, LINE);
    f.line(tl_x, br_y, br_x, br_y, lt, LINE);
    f.line(tl_x, tl_y, tl_x, br_y, lt, LINE);
    f.line(br_x, tl_y, br_x, br_y, lt, LINE);
    // Ligne médiane + rond central.
    let (mx0, my0) = cam.p(0.0, -HALF_WID);
    let (mx1, my1) = cam.p(0.0, HALF_WID);
    f.line(mx0, my0, mx1, my1, lt, LINE);
    let (cx, cy) = cam.p(0.0, 0.0);
    f.ring(cx, cy, 9.15 * cam.sx, lt, LINE);
    f.disc(cx, cy, 1.6, LINE);
    // Surfaces de réparation + buts.
    for sign in [-1.0_f32, 1.0] {
        let gx = sign * HALF_LEN;
        let (bx0, by0) = cam.p(gx - sign * 16.5, -20.16);
        let (bx1, by1) = cam.p(gx, 20.16);
        f.line(bx0, by0, bx1, by0, lt, LINE);
        f.line(bx0, by1, bx1, by1, lt, LINE);
        f.line(bx0, by0, bx0, by1, lt, LINE);
        // Cage (au-delà de la ligne).
        let (kx0, ky0) = cam.p(gx, -GOAL_HALF);
        let (kx1, ky1) = cam.p(gx + sign * 2.0, GOAL_HALF);
        f.fill_rect(
            kx0.min(kx1) as i32,
            ky0 as i32,
            kx0.max(kx1) as i32,
            ky1 as i32,
            LINE,
        );
    }

    // Joueurs (ombre + disque coloré ; porteur surligné, joueur contrôlé désigné).
    let poss = world.possessor();
    // Sans repère, on ne sait pas qui l'on dirige : indispensable dès que le match se joue, et
    // sans effet sur une simulation autonome (le repère suit alors le joueur que l'IA pilote).
    let pilote = world.controlled();
    for (i, p) in world.players.iter().enumerate() {
        let (sx, sy) = cam.p(p.pos.x, p.pos.y);
        f.disc(sx + 1.5, sy + 2.5, 4.6, SHADOW);
        let col = match (p.team, p.role) {
            (0, Role::Goalkeeper) => KEEPER_H,
            (1, Role::Goalkeeper) => KEEPER_A,
            (0, _) => HOME,
            _ => AWAY,
        };
        if poss == Some(i) {
            f.disc(sx, sy, 6.4, LINE);
        }
        f.disc(sx, sy, 4.6, col);
        // Curseur du joueur dirigé : un chevron au-dessus de la tête, comme dans tout jeu de
        // football. Dessiné APRÈS le disque pour ne pas être recouvert, et en dehors de lui pour
        // rester lisible quand le joueur porte déjà le halo de possession.
        if pilote == Some(i) {
            // Triangle plein pointant vers le joueur, assez large pour se voir au milieu de
            // vingt-deux disques : un trait fin se perdait dans la pelouse.
            const HAUTEUR: f32 = 9.0;
            const DEMI_BASE: f32 = 6.0;
            for k in 0..=HAUTEUR as i32 {
                let t = f32::from(k as u8) / HAUTEUR;
                let demi = DEMI_BASE * (1.0 - t);
                let y = sy - 16.0 + k as f32;
                f.line(sx - demi, y, sx + demi, y, 1.6, CURSEUR);
            }
        }
    }

    // Ballon : ombre au sol + ballon décalé vers le haut selon la hauteur z.
    let (bx, by) = cam.p(world.ball.pos.x, world.ball.pos.y);
    let lift = world.ball.pos.z * cam.sy * 0.8;
    f.disc(bx, by, 3.0, SHADOW);
    f.disc(bx, by - lift, 3.2, BALL);

    // Bandeau score (deux barres : longueur ∝ buts, min 1).
    f.fill_rect(8, 8, 8 + 14 * (world.score[0].max(1)) as i32, 20, HOME);
    f.fill_rect(8, 24, 8 + 14 * (world.score[1].max(1)) as i32, 36, AWAY);

    f
}
