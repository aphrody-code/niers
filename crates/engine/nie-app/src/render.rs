//! Rendu CPU des écrans du jeu : [`Frame`] (cadre RGBA + helpers), [`Font`] (police réelle via
//! `LatinAtlas`), [`render_state`] (un état → un cadre) et [`CpuRenderer`] (impl [`Renderer`] :
//! police + toiles de fond 3D pré-rendues). Front-end headless/golden ; le temps-réel passe par wgpu.

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use anyhow::{Context, Result};
use nie_formats::font::{self, LatinAtlas};
use nie_formats::{cfgbin, g4tx};

#[cfg(not(target_arch = "wasm32"))]
use crate::Renderer;
#[cfg(not(target_arch = "wasm32"))]
use crate::character;
use crate::{GameState, MENU};

/// Largeur du canevas de rendu (px).
pub const W: usize = 1280;
/// Hauteur du canevas de rendu (px).
pub const H: usize = 720;

/// Atlas de police chargé (octets + `LatinAtlas` + largeur), pour le rendu de texte.
pub struct Font {
    atlas: Vec<u8>,
    la: LatinAtlas,
    aw: usize,
}

/// Repli ASCII d'un caractère que l'atlas latin ne porte pas.
///
/// **Pis-aller assumé, et voici pourquoi.** L'atlas `font_def` ne contient pas les lettres
/// accentuées : sa rangée latine est l'ASCII imprimable (0x21..0x7E) suivi de quinze katakana
/// demi-chasse — vérifié en extrayant les pixels, pas supposé. Les métriques de `font.cfg.bin`
/// connaissent pourtant les 38 caractères français (sous leurs octets UTF-8 empaquetés,
/// cf. `FontMetrics::glyph_char`), mais leurs coordonnées pointent, DANS CET ATLAS, sur des
/// idéogrammes : elles décrivent une autre planche. Retrouver la bonne est un travail de
/// rétro-ingénierie, pas une correction de deux lignes.
///
/// En attendant, sans repli, le moteur dessinait une espace : « Composition d' quipe »,
/// « Fichier de donn es ». Un mot amputé ne se lit pas ; un mot sans accent se lit. On dégrade
/// donc de façon lisible et prévisible, plutôt que de laisser des trous ou — pire — de blitter
/// le glyphe que les métriques désignent, qui est un idéogramme.
fn repli_ascii(c: char) -> Option<&'static str> {
    Some(match c {
        'é' | 'è' | 'ê' | 'ë' => "e",
        'à' | 'â' | 'ä' => "a",
        'î' | 'ï' => "i",
        'ô' | 'ö' => "o",
        'ù' | 'û' | 'ü' => "u",
        'ç' => "c",
        'É' | 'È' | 'Ê' | 'Ë' => "E",
        'À' | 'Â' | 'Ä' => "A",
        'Î' | 'Ï' => "I",
        'Ô' | 'Ö' => "O",
        'Ù' | 'Û' | 'Ü' => "U",
        'Ç' => "C",
        'œ' => "oe",
        'Œ' => "OE",
        'æ' => "ae",
        'Æ' => "AE",
        // Ponctuation française : les guillemets et l'apostrophe typographique ont un
        // équivalent ASCII exact, il n'y a rien à perdre à les convertir.
        '«' | '»' => "\"",
        '’' | '‘' => "'",
        '…' => "...",
        '–' | '—' => "-",
        // Séparateurs typographiques : sans repli, ils disparaissaient et deux champs se
        // retrouvaient collés par une espace (« Milieu   Foret » au lieu de « Milieu · Foret »).
        '·' | '•' => "-",
        '×' => "x",
        '°' => "o",
        _ => return None,
    })
}

impl Font {
    /// Construit la police depuis les OCTETS de `font.cfg.bin` (métriques) + `font.g4tx` (atlas).
    /// Cœur wasm-safe (aucune I/O) : l'appelant fournit les octets (fichier natif ou fetch web).
    pub fn from_bytes(cfg_bytes: &[u8], g4tx_bytes: &[u8]) -> Result<Self> {
        let cfg = cfgbin::parse_t2b(cfg_bytes).map_err(|e| anyhow::anyhow!("cfg: {e:?}"))?;
        let metrics = font::parse_metrics(&cfg);
        let tx = g4tx::parse(g4tx_bytes).map_err(|e| anyhow::anyhow!("g4tx: {e:?}"))?;
        let t = tx.textures.first().context("pas de texture police")?;
        let dds = &g4tx_bytes[t.data_offset..];
        let off = if dds.len() >= 88 && &dds[84..88] == b"DX10" {
            148
        } else {
            128
        };
        let atlas = dds[off..].to_vec();
        let (aw, ah) = (t.width as usize, t.height as usize);
        let la = LatinAtlas::from_atlas(&atlas, aw, ah, 946, metrics.dims.cell_height);
        Ok(Self { atlas, la, aw })
    }

    /// Largeur d'un texte en pixels, telle qu'il sera dessiné (replis ASCII compris).
    #[must_use]
    pub fn largeur(&self, texte: &str) -> u32 {
        self.la.measure(&self.texte_rendu(texte))
    }

    /// Texte rendu tel qu'il sera réellement dessiné : les caractères absents de l'atlas latin
    /// sont remplacés par leur équivalent ASCII (cf. [`repli_ascii`]).
    ///
    /// Mesure et dessin passent par ici tous les deux — sinon un libellé accentué serait centré
    /// sur une largeur qu'il n'occupe pas.
    fn texte_rendu(&self, texte: &str) -> String {
        texte
            .chars()
            .map(|c| {
                if c == ' ' || self.la.span(c as u32).is_some() {
                    c.to_string()
                } else {
                    repli_ascii(c).unwrap_or(" ").to_string()
                }
            })
            .collect()
    }

    /// Dessine `texte` à `(x, y)` sur `canvas` (RGBA8, `cw` px de large).
    ///
    /// `y` désigne le haut de l'**encre**, pas le haut de la cellule : une cellule de `font_def`
    /// porte 21 px de vide au-dessus des lettres, et une mise en page qui l'ignore décale tout
    /// son texte vers le bas.
    fn dessiner(&self, canvas: &mut [u8], cw: usize, x: i32, y: i32, texte: &str, fg: [u8; 4]) {
        let rendu = self.texte_rendu(texte);
        let y_cellule = y - i32::from(self.la.ink_top);
        self.la
            .blit_line(&self.atlas, self.aw, canvas, cw, x, y_cellule, &rendu, fg);
    }

    /// Tronque `texte` pour qu'il tienne dans `max_px`, en terminant par « … » s'il a été coupé.
    ///
    /// Les données du jeu ne sont pas calibrées pour une largeur d'écran : une entrée de
    /// « Fichier de données » est une phrase entière, et sans coupe elle sort du cadre par la
    /// droite — la fin du mot est perdue sans que rien ne l'indique. Mieux vaut une ellipse, qui
    /// dit qu'il y a une suite.
    #[must_use]
    pub fn tronquer(&self, texte: &str, max_px: i32) -> String {
        if max_px <= 0 || self.largeur(texte) as i32 <= max_px {
            return texte.to_string();
        }
        // Réserve la place de l'ellipse avant de couper, sinon le résultat dépasse encore.
        let reserve = max_px - self.largeur("...") as i32;
        let mut coupe = String::new();
        for c in texte.chars() {
            let essai_len = {
                coupe.push(c);
                let l = self.largeur(&coupe) as i32;
                coupe.pop();
                l
            };
            if essai_len > reserve {
                break;
            }
            coupe.push(c);
        }
        // Ne pas couper en plein mot quand un espace proche permet de faire mieux.
        if let Some(pos) = coupe.rfind(' ')
            && pos > coupe.len().saturating_sub(14)
        {
            coupe.truncate(pos);
        }
        format!("{}...", coupe.trim_end())
    }

    /// Hauteur d'une ligne de texte telle qu'elle s'affiche, en pixels.
    ///
    /// C'est la hauteur à réserver dans une mise en page — la cellule complète en gaspillerait
    /// un tiers, et neuf lignes n'entreraient pas dans l'écran.
    #[must_use]
    pub fn hauteur_ligne(&self) -> i32 {
        i32::from(self.la.ink_h)
    }

    /// Charge la police depuis le disque (natif) — délègue à [`Font::from_bytes`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(cfg_path: &Path, g4tx_path: &Path) -> Result<Self> {
        Self::from_bytes(&std::fs::read(cfg_path)?, &std::fs::read(g4tx_path)?)
    }
}

/// Cadre de rendu RGBA + helpers de dessin.
pub struct Frame<'a> {
    /// Tampon RGBA8 `W*H*4`.
    pub buf: Vec<u8>,
    f: &'a Font,
}

impl<'a> Frame<'a> {
    fn new(f: &'a Font) -> Self {
        Self {
            buf: vec![0u8; W * H * 4],
            f,
        }
    }
    /// Cadre initialisé depuis une frame RGBA existante (ex. rendu 3D du perso en toile de fond).
    fn from_base(f: &'a Font, base: &[u8]) -> Self {
        let mut buf = vec![0u8; W * H * 4];
        if base.len() == buf.len() {
            buf.copy_from_slice(base);
        }
        Self { buf, f }
    }
    fn gradient(&mut self, top: [u8; 3], bot: [u8; 3]) {
        for y in 0..H {
            let t = y as f32 / H as f32;
            let mix = |a: u8, b: u8| (f32::from(a) * (1.0 - t) + f32::from(b) * t) as u8;
            let c = [
                mix(top[0], bot[0]),
                mix(top[1], bot[1]),
                mix(top[2], bot[2]),
                255,
            ];
            for x in 0..W {
                let o = (y * W + x) * 4;
                self.buf[o..o + 4].copy_from_slice(&c);
            }
        }
    }
    #[allow(clippy::needless_range_loop)] // blend RGBA : indices explicites (c[k] + buf[o+k]).
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 4]) {
        let a = f32::from(c[3]) / 255.0;
        for y in y0.max(0)..y1.min(H as i32) {
            for x in x0.max(0)..x1.min(W as i32) {
                let o = (y as usize * W + x as usize) * 4;
                for k in 0..3 {
                    self.buf[o + k] =
                        (f32::from(c[k]) * a + f32::from(self.buf[o + k]) * (1.0 - a)) as u8;
                }
                self.buf[o + 3] = 255;
            }
        }
    }
    fn text(&mut self, x: i32, y: i32, s: &str, color: [u8; 4]) {
        self.f.dessiner(&mut self.buf, W, x, y, s, color);
    }
    fn text_centered(&mut self, y: i32, s: &str, color: [u8; 4]) {
        let w = self.f.largeur(s) as i32;
        self.text((W as i32 - w) / 2, y, s, color);
    }
    /// Texte avec retour à la ligne par mot dans `max_w` (px). Interligne `line_h`.
    fn text_wrapped(&mut self, x: i32, y: i32, max_w: i32, line_h: i32, s: &str, color: [u8; 4]) {
        let (mut line, mut row) = (String::new(), 0i32);
        for word in s.split_whitespace() {
            let trial = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if self.f.largeur(&trial) as i32 > max_w && !line.is_empty() {
                self.text(x, y + row * line_h, &line, color);
                row += 1;
                line = word.to_string();
            } else {
                line = trial;
            }
        }
        if !line.is_empty() {
            self.text(x, y + row * line_h, &line, color);
        }
    }
}

/// Rend un état du jeu dans un cadre. `bg` = toile de fond 3D propre à cet état (perso, match…).
#[must_use]
pub fn render_state<'a>(state: &GameState, f: &'a Font, bg: Option<&[u8]>) -> Frame<'a> {
    let mut s = match bg {
        Some(b) => Frame::from_base(f, b),
        None => Frame::new(f),
    };
    match state {
        GameState::Title => {
            s.gradient([24, 32, 64], [8, 10, 20]);
            s.text_centered(250, "INAZUMA ELEVEN", [235, 240, 255, 255]);
            s.text_centered(330, "VICTORY ROAD", [120, 200, 255, 255]);
            s.text_centered(560, "PRESS START", [200, 210, 230, 255]);
        }
        GameState::MainMenu { sel } => {
            if bg.is_none() {
                s.gradient([30, 40, 80], [14, 18, 34]);
            }
            s.rect(0, 0, W as i32, 70, [20, 60, 130, 255]);
            s.text(40, 16, "MAIN MENU", [220, 235, 255, 255]);
            for (i, item) in MENU.iter().enumerate() {
                let y = 100 + i as i32 * 72;
                let hot = i == *sel;
                let bg_c = if hot {
                    [60, 130, 220, 235]
                } else {
                    [16, 22, 44, 210]
                };
                s.rect(70, y, 620, y + 56, bg_c);
                if hot {
                    s.rect(70, y, 76, y + 56, [120, 220, 255, 255]);
                }
                s.text(110, y + 14, item, [240, 245, 252, 255]);
            }
        }
        GameState::Match { home, away } => {
            if bg.is_none() {
                s.gradient([18, 60, 30], [8, 24, 14]);
            }
            s.rect(0, 0, W as i32, 70, [20, 60, 130, 255]);
            s.text(40, 16, "MATCH RESULT", [220, 235, 255, 255]);
            s.text_centered(280, "RAIMON", [255, 240, 180, 255]);
            s.text_centered(380, &format!("{home}  -  {away}"), [255, 255, 255, 255]);
            s.text_centered(480, "ROYAL ACADEMY", [200, 210, 255, 255]);
            let verdict = if home > away {
                "RAIMON WINS!"
            } else if home < away {
                "DEFEAT..."
            } else {
                "DRAW"
            };
            s.text_centered(600, verdict, [120, 255, 160, 255]);
        }
        GameState::Story { speaker, line } => {
            if bg.is_none() {
                s.gradient([20, 26, 54], [8, 10, 22]);
            }
            let (bx0, bx1) = (60i32, W as i32 - 60);
            let by1 = H as i32 - 40;
            let by0 = by1 - 150;
            s.rect(bx0, by0, bx1, by1, [10, 14, 28, 224]);
            s.rect(bx0, by0, bx1, by0 + 3, [90, 200, 255, 255]);
            s.rect(bx0 + 20, by0 - 40, bx0 + 280, by0 + 2, [30, 60, 110, 235]);
            s.text(bx0 + 36, by0 - 32, speaker, [200, 235, 255, 255]);
            s.text_wrapped(
                bx0 + 40,
                by0 + 28,
                bx1 - bx0 - 80,
                46,
                line,
                [240, 244, 250, 255],
            );
        }
    }
    s
}

/// Marge gauche du texte dans une barre de liste, en pixels.
const TEXTE_X: i32 = 110;

/// Compose le bandeau de score par-dessus une image de match.
///
/// Le rastériseur du moteur (`nie_runtime::render`) n'a pas de police : il rend le score par deux
/// barres dont la longueur est proportionnelle aux buts, ce qui se lit mal dès le deuxième but et
/// pas du tout au premier coup d'œil. La police vit ici, donc le bandeau aussi.
///
/// `temps` est le temps de jeu simulé, en secondes.
#[must_use]
pub fn hud_match(base: &[u8], f: &Font, score: [u32; 2], temps: f32) -> Vec<u8> {
    let mut s = Frame::from_base(f, base);
    let ligne = f.hauteur_ligne();
    let h_bandeau = ligne + 18;
    s.rect(0, 0, W as i32, h_bandeau, [12, 18, 38, 205]);

    let minutes = (temps / 60.0) as u32;
    let secondes = (temps % 60.0) as u32;
    let gauche = format!("DOMICILE {}", score[0]);
    let droite = format!("{} EXTERIEUR", score[1]);
    let chrono = format!("{minutes:02}:{secondes:02}");

    let y = (h_bandeau - ligne) / 2;
    s.text(24, y, &gauche, [140, 190, 255, 255]);
    let w_chrono = f.largeur(&chrono) as i32;
    s.text((W as i32 - w_chrono) / 2, y, &chrono, [235, 240, 250, 255]);
    let w_droite = f.largeur(&droite) as i32;
    s.text(W as i32 - 24 - w_droite, y, &droite, [255, 170, 170, 255]);
    s.buf
}

/// Rendu générique d'un menu-liste (titre + items surlignables), adapté au nombre d'items.
///
/// Sert au menu principal (9 onglets réels), au sélecteur de mode (5 modes réels) et aux listes
/// de données du jeu. RGBA8 `W*H*4`.
#[must_use]
pub fn render_list<'a>(title: &str, items: &[&str], sel: usize, f: &'a Font) -> Frame<'a> {
    let mut s = Frame::new(f);
    s.gradient([30, 40, 80], [14, 18, 34]);
    s.rect(0, 0, W as i32, 70, [20, 60, 130, 255]);
    s.text(40, 16, title, [220, 235, 255, 255]);
    // La mise en page part de la hauteur RÉELLE d'une ligne de texte, mesurée dans l'atlas — pas
    // d'une constante. Avec neuf onglets, une supposition trop généreuse déborde de l'écran (le
    // dernier libellé était coupé) et une supposition trop courte fait sortir le texte de sa
    // barre (c'était le cas : cellule de 71 px pour une boîte de 57).
    let n = (items.len().max(1)) as i32;
    let ligne = f.hauteur_ligne();
    let haut_liste = 100;
    let dispo = H as i32 - haut_liste - 20;
    let gap = (dispo / n).clamp(ligne + 8, ligne + 40);
    let bh = gap - 8;
    for (i, item) in items.iter().enumerate() {
        let y = haut_liste + i as i32 * gap;
        let hot = i == sel;
        let bg_c = if hot {
            [60, 130, 220, 235]
        } else {
            [16, 22, 44, 210]
        };
        s.rect(70, y, W as i32 - 70, y + bh, bg_c);
        if hot {
            s.rect(70, y, 76, y + bh, [120, 220, 255, 255]);
        }
        // Encre centrée dans la barre : `text` prend le haut de l'encre, donc le calcul est
        // direct et ne dépend plus de la hauteur de cellule.
        let coupe = f.tronquer(item, W as i32 - 70 - TEXTE_X - 16);
        s.text(TEXTE_X, y + (bh - ligne) / 2, &coupe, [240, 245, 252, 255]);
    }
    s
}

/// Chemins des assets d'un personnage 3D (toile de fond menu/histoire). Natif (chemins disque).
#[cfg(not(target_arch = "wasm32"))]
pub struct CharAssets<'a> {
    pub md: &'a Path,
    pub mg: &'a Path,
    pub sk: &'a Path,
    pub tex: &'a Path,
}

/// Renderer CPU : police + toiles de fond 3D pré-rendues (perso, scène match), réutilisées par état.
/// Natif (charge les assets depuis le disque) ; le rendu web passe par [`render_state`] + [`Font::from_bytes`].
#[cfg(not(target_arch = "wasm32"))]
pub struct CpuRenderer {
    font: Font,
    char_bg: Option<Vec<u8>>,
    match_bg: Option<Vec<u8>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl CpuRenderer {
    /// Construit le renderer : charge la police et, si `chr` fourni, pré-rend le perso 3D + la scène
    /// de match (réutilisés comme toiles de fond).
    pub fn new(font_cfg: &Path, font_g4tx: &Path, chr: Option<CharAssets<'_>>) -> Result<Self> {
        let font = Font::load(font_cfg, font_g4tx).context("chargement police")?;
        let (char_bg, match_bg) = match chr {
            Some(c) => {
                let model = character::build_skinned_model(
                    &std::fs::read(c.md)?,
                    &std::fs::read(c.mg)?,
                    &std::fs::read(c.sk)?,
                    &std::fs::read(c.tex)?,
                )
                .context("chargement perso 3D")?;
                let cbg = character::render_character(
                    &model,
                    W as u32,
                    H as u32,
                    0.55,
                    [30, 40, 80],
                    [12, 16, 30],
                );
                let mbg = character::render_match_scene(&model, W as u32, H as u32);
                (Some(cbg), Some(mbg))
            }
            None => (None, None),
        };
        Ok(Self {
            font,
            char_bg,
            match_bg,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Renderer for CpuRenderer {
    fn render(&self, state: &GameState) -> Vec<u8> {
        let bg = match state {
            GameState::Match { .. } => self.match_bg.as_deref(),
            GameState::MainMenu { .. } | GameState::Story { .. } => self.char_bg.as_deref(),
            GameState::Title => None,
        };
        render_state(state, &self.font, bg).buf
    }
}

#[cfg(test)]
mod tests {
    use super::repli_ascii;

    /// Tout caractère du français que l'atlas latin ne porte pas doit avoir un repli ASCII.
    ///
    /// Sans repli, le rendu écrit une espace : « Fichier de donn es ». Le jeu est en français —
    /// les neuf onglets du menu principal, les cinq modes et les dialogues en sont pleins.
    #[test]
    fn tout_le_francais_a_un_repli() {
        // Les caractères réellement produits par les textes du jeu, minuscules et majuscules,
        // plus la ponctuation typographique française.
        for c in "éèêëàâäîïôöùûüçÉÈÊËÀÂÄÎÏÔÖÙÛÜÇœŒæÆ«»’…–—·•×°".chars()
        {
            assert!(repli_ascii(c).is_some(), "aucun repli pour « {c} »");
        }
    }

    /// Le repli conserve la lettre, il ne l'invente pas : `é` donne `e`, jamais autre chose.
    #[test]
    fn le_repli_garde_la_lettre_de_base() {
        assert_eq!(repli_ascii('é'), Some("e"));
        assert_eq!(repli_ascii('È'), Some("E"));
        assert_eq!(repli_ascii('ç'), Some("c"));
        assert_eq!(repli_ascii('œ'), Some("oe"));
        assert_eq!(repli_ascii('«'), Some("\""));
        assert_eq!(repli_ascii('’'), Some("'"));
    }

    /// L'ASCII n'a rien à voir avec ce mécanisme : il est rendu par l'atlas, pas replié.
    ///
    /// Un repli qui capturerait l'ASCII masquerait une régression de l'edge-scan — le texte
    /// continuerait de s'afficher alors que la police ne serait plus lue.
    #[test]
    fn l_ascii_n_est_pas_replie() {
        for c in "AZaz09 !?,.-'\"".chars() {
            assert_eq!(
                repli_ascii(c),
                None,
                "« {c} » ne doit pas passer par le repli"
            );
        }
    }
}
