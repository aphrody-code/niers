//! Pilotage de la caméra du **jeu en cours d'exécution**.
//!
//! S'appuie sur [`nie_trace`] (Windows natif : `OpenProcess`/`ReadProcessMemory` ; Linux/Wine :
//! `process_vm_readv`). Ce module ajoute la couche caméra : un layout mémoire décrivant où
//! trouver position/cible/FOV dans un objet caméra, la lecture et l'écriture d'un
//! [`CameraState`] complet, un scan heuristique pour retrouver l'objet, et un gel de valeur.
//!
//! ## Ce que ce module ne fait pas
//!
//! Le catalogue AOB de `nie-trace` (`catalog.rs`) ne contient **aucun localisateur caméra** :
//! aucune signature n'a été validée sur un dump pour l'objet caméra. On ne fabrique donc pas une
//! adresse en dur. Le chemin nominal est [`LiveCamera::scan`] — un scan de plausibilité qui
//! propose des candidats, à confirmer en bougeant la caméra en jeu et en relançant le scan
//! ([`LiveCamera::confirm`]). Une fois l'adresse connue, elle se réutilise dans la session.
//!
//! ## Écriture
//!
//! Écrire dans un process vivant peut le déstabiliser. Toutes les écritures passent par
//! [`LiveCamera::write_state`], qui refuse les valeurs non finies et hors des bornes de
//! [`PlausibleRange`] — une caméra à `NaN` fige le rendu.

use nie_trace::{MemError, read_exact, write_exact};

use crate::CameraState;

/// Où se trouvent les champs de caméra dans un objet mémoire, en octets depuis sa base.
///
/// Le layout par défaut correspond à la disposition la plus courante des caméras Level-5 :
/// deux `float[3]` consécutifs (position puis point visé) suivis du FOV. Il est **paramétrable**
/// parce qu'il n'a pas été confirmé sur `CGameCameraCtrl` : les noms de propriétés du binaire
/// (`m_worldCameraPos`, `m_cameraRefPosOffset`, `m_cameraFov`, `m_cameraRoll`) attestent des
/// champs, pas de leurs offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraLayout {
    /// Offset de `float[3]` position monde.
    pub pos: usize,
    /// Offset de `float[3]` point visé.
    pub ref_pos: usize,
    /// Offset du `float` FOV (degrés), `None` si absent du layout.
    pub fov: Option<usize>,
    /// Offset du `float` roll (degrés), `None` si absent.
    pub roll: Option<usize>,
}

impl Default for CameraLayout {
    fn default() -> Self {
        CameraLayout {
            pos: 0x00,
            ref_pos: 0x0C,
            fov: Some(0x18),
            roll: Some(0x1C),
        }
    }
}

impl CameraLayout {
    /// Nombre d'octets à lire pour couvrir tous les champs du layout.
    #[must_use]
    pub fn span(&self) -> usize {
        let mut end = self.pos + 12;
        end = end.max(self.ref_pos + 12);
        if let Some(f) = self.fov {
            end = end.max(f + 4);
        }
        if let Some(r) = self.roll {
            end = end.max(r + 4);
        }
        end
    }

    /// Décode un [`CameraState`] depuis un bloc d'octets lu à la base de l'objet.
    ///
    /// Renvoie `None` si le bloc est trop court.
    #[must_use]
    pub fn decode(&self, buf: &[u8]) -> Option<CameraState> {
        let f32_at = |o: usize| -> Option<f32> {
            buf.get(o..o + 4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let v3 =
            |o: usize| -> Option<[f32; 3]> { Some([f32_at(o)?, f32_at(o + 4)?, f32_at(o + 8)?]) };
        Some(CameraState {
            pos: v3(self.pos)?,
            ref_pos: v3(self.ref_pos)?,
            fov_deg: self.fov.and_then(f32_at).unwrap_or(45.0),
            roll_deg: self.roll.and_then(f32_at).unwrap_or(0.0),
            ..CameraState::default()
        })
    }

    /// Écrit un [`CameraState`] dans un bloc d'octets (les champs hors layout sont laissés tels
    /// quels — on ne réécrit que ce qu'on connaît).
    pub fn encode_into(&self, st: &CameraState, buf: &mut [u8]) {
        let put = |buf: &mut [u8], o: usize, v: f32| {
            if let Some(s) = buf.get_mut(o..o + 4) {
                s.copy_from_slice(&v.to_le_bytes());
            }
        };
        for (i, v) in st.pos.iter().enumerate() {
            put(buf, self.pos + i * 4, *v);
        }
        for (i, v) in st.ref_pos.iter().enumerate() {
            put(buf, self.ref_pos + i * 4, *v);
        }
        if let Some(o) = self.fov {
            put(buf, o, st.fov_deg);
        }
        if let Some(o) = self.roll {
            put(buf, o, st.roll_deg);
        }
    }
}

/// Bornes de plausibilité d'un état de caméra.
///
/// Les ordres de grandeur viennent des données réelles : coordonnées de scène de l'ordre de
/// ±50 dans les `.g4cm`, FOV de 45° dans `soccer_camera_config`, distance de poursuite de 4,5 à
/// 20 unités dans les presets `CCameraCtrlChase*`. On garde une marge large : le but est de
/// rejeter le bruit mémoire et les valeurs destructrices, pas de contraindre le jeu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlausibleRange {
    /// Valeur absolue maximale d'une coordonnée.
    pub max_coord: f32,
    /// Bornes du FOV, en degrés.
    pub fov: (f32, f32),
    /// Distance maximale entre l'œil et le point visé.
    pub max_length: f32,
}

impl Default for PlausibleRange {
    fn default() -> Self {
        PlausibleRange {
            max_coord: 5_000.0,
            fov: (1.0, 179.0),
            max_length: 5_000.0,
        }
    }
}

impl PlausibleRange {
    /// `true` si l'état est exploitable (fini, borné, non dégénéré).
    #[must_use]
    pub fn accepts(&self, st: &CameraState) -> bool {
        let finite = st
            .pos
            .iter()
            .chain(st.ref_pos.iter())
            .chain([st.fov_deg, st.roll_deg].iter())
            .all(|v| v.is_finite());
        if !finite {
            return false;
        }
        if st
            .pos
            .iter()
            .chain(st.ref_pos.iter())
            .any(|v| v.abs() > self.max_coord)
        {
            return false;
        }
        if st.fov_deg < self.fov.0 || st.fov_deg > self.fov.1 {
            return false;
        }
        let len = st.length();
        len > 1e-3 && len <= self.max_length
    }
}

/// Une caméra localisée dans le process du jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveCamera {
    /// PID du process.
    pub pid: i32,
    /// Adresse de base de l'objet caméra.
    pub addr: u64,
    /// Layout mémoire utilisé.
    pub layout: CameraLayout,
}

/// Trouve le PID de `nie.exe` (ou d'un process dont le nom contient `name`).
#[must_use]
pub fn find_game(name: &str) -> Option<i32> {
    nie_trace::find_pid_by_name(name)
}

impl LiveCamera {
    /// Attache une caméra déjà localisée.
    #[must_use]
    pub fn at(pid: i32, addr: u64, layout: CameraLayout) -> Self {
        LiveCamera { pid, addr, layout }
    }

    /// Lit l'état courant.
    ///
    /// # Errors
    /// [`MemError`] si la lecture échoue (process disparu, page non lisible).
    pub fn read_state(&self) -> Result<CameraState, MemError> {
        let buf = read_exact(self.pid, self.addr, self.layout.span())?;
        self.layout.decode(&buf).ok_or(MemError::Partial {
            op: "read_state",
            pid: self.pid,
            addr: self.addr,
            requested: self.layout.span(),
            got: buf.len(),
        })
    }

    /// Écrit un état, après contrôle de plausibilité.
    ///
    /// Lit d'abord le bloc courant pour ne réécrire que les champs du layout, laissant intacts
    /// les octets voisins (l'objet caméra contient bien d'autres champs).
    ///
    /// # Errors
    /// [`MemError::Unsupported`] si l'état est refusé par `range` (valeur non finie, hors
    /// bornes, caméra dégénérée), ou l'erreur mémoire sous-jacente.
    pub fn write_state(&self, st: &CameraState, range: PlausibleRange) -> Result<(), MemError> {
        if !range.accepts(st) {
            return Err(MemError::Unsupported);
        }
        let mut buf = read_exact(self.pid, self.addr, self.layout.span())?;
        self.layout.encode_into(st, &mut buf);
        write_exact(self.pid, self.addr, &buf)
    }

    /// Vérifie que l'adresse porte toujours un état plausible (l'objet a pu être libéré).
    #[must_use]
    pub fn confirm(&self, range: PlausibleRange) -> bool {
        self.read_state().is_ok_and(|st| range.accepts(&st))
    }
}

/// Un candidat trouvé par [`scan`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// Adresse de base présumée de l'objet.
    pub addr: u64,
    /// État lu à cette adresse.
    pub state: CameraState,
}

/// Cherche dans les régions inscriptibles du process des blocs ressemblant à une caméra.
///
/// Heuristique, assumée comme telle : on teste chaque offset aligné sur 4 octets, on décode
/// selon `layout`, et on retient ce que `range` accepte. Le résultat contient donc des faux
/// positifs — c'est un point de départ à confirmer en bougeant la caméra en jeu puis en
/// intersectant deux scans (les vrais candidats changent de valeur, le bruit non).
///
/// `limit` borne le nombre de candidats retournés pour ne pas saturer la mémoire.
#[must_use]
pub fn scan(pid: i32, layout: CameraLayout, range: PlausibleRange, limit: usize) -> Vec<Candidate> {
    let mut out = Vec::new();
    let span = layout.span();
    for region in nie_trace::enumerate_regions(pid) {
        if !region.is_writable() || !region.is_readable() || region.size() < span as u64 {
            continue;
        }
        // Une région énorme est lue par tranches pour borner l'empreinte mémoire.
        const CHUNK: u64 = 1 << 20;
        let mut base = region.start;
        while base < region.end {
            let len = CHUNK.min(region.end - base) as usize;
            let Ok(buf) = read_exact(pid, base, len) else {
                base += CHUNK;
                continue;
            };
            let mut off = 0usize;
            while off + span <= buf.len() {
                if let Some(st) = layout.decode(&buf[off..])
                    && range.accepts(&st)
                {
                    out.push(Candidate {
                        addr: base + off as u64,
                        state: st,
                    });
                    if out.len() >= limit {
                        return out;
                    }
                }
                off += 4;
            }
            base += CHUNK;
        }
    }
    out
}

/// Intersecte deux scans : ne garde que les adresses présentes dans les deux **et** dont l'état
/// a changé entre les deux passes.
///
/// C'est la façon fiable de distinguer la vraie caméra du bruit : faire un scan, bouger la
/// caméra en jeu, refaire un scan, intersecter.
#[must_use]
pub fn narrow(before: &[Candidate], after: &[Candidate]) -> Vec<Candidate> {
    let mut out = Vec::new();
    for a in after {
        if let Some(b) = before.iter().find(|b| b.addr == a.addr)
            && b.state != a.state
        {
            out.push(*a);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_aller_retour() {
        let layout = CameraLayout::default();
        let st = CameraState {
            pos: [1.5, 2.5, -3.5],
            ref_pos: [0.0, 1.0, 0.0],
            fov_deg: 55.0,
            roll_deg: -2.0,
            ..CameraState::default()
        };
        let mut buf = vec![0u8; layout.span()];
        layout.encode_into(&st, &mut buf);
        let back = layout.decode(&buf).expect("décodage");
        assert_eq!(back.pos, st.pos);
        assert_eq!(back.ref_pos, st.ref_pos);
        assert!((back.fov_deg - 55.0).abs() < f32::EPSILON);
        assert!((back.roll_deg + 2.0).abs() < f32::EPSILON);
        assert_eq!(layout.span(), 0x20);
    }

    #[test]
    fn layout_sans_fov_ni_roll() {
        let layout = CameraLayout {
            pos: 0,
            ref_pos: 12,
            fov: None,
            roll: None,
        };
        assert_eq!(layout.span(), 24);
        let buf = vec![0u8; 24];
        let st = layout.decode(&buf).expect("décodage");
        assert!((st.fov_deg - 45.0).abs() < f32::EPSILON, "FOV par défaut");
    }

    #[test]
    fn decode_refuse_un_bloc_court() {
        assert!(CameraLayout::default().decode(&[0u8; 8]).is_none());
    }

    #[test]
    fn plausibilite() {
        let r = PlausibleRange::default();
        let ok = CameraState {
            pos: [0.0, 2.0, 10.0],
            ref_pos: [0.0, 0.0, 0.0],
            fov_deg: 45.0,
            ..CameraState::default()
        };
        assert!(r.accepts(&ok));
        // NaN.
        assert!(!r.accepts(&CameraState {
            fov_deg: f32::NAN,
            ..ok
        }));
        // Coordonnée absurde.
        assert!(!r.accepts(&CameraState {
            pos: [1e9, 0.0, 0.0],
            ..ok
        }));
        // FOV hors bornes.
        assert!(!r.accepts(&CameraState { fov_deg: 0.0, ..ok }));
        // Caméra dégénérée (œil sur la cible).
        assert!(!r.accepts(&CameraState {
            pos: [0.0, 0.0, 0.0],
            ..ok
        }));
    }

    #[test]
    fn intersection_de_scans() {
        let a = CameraState::default();
        let b = CameraState {
            fov_deg: 60.0,
            ..CameraState::default()
        };
        let before = [
            Candidate {
                addr: 0x1000,
                state: a,
            },
            Candidate {
                addr: 0x2000,
                state: a,
            },
        ];
        let after = [
            Candidate {
                addr: 0x1000,
                state: b,
            }, // a changé → retenu
            Candidate {
                addr: 0x2000,
                state: a,
            }, // inchangé → écarté
            Candidate {
                addr: 0x3000,
                state: b,
            }, // nouveau → écarté
        ];
        let n = narrow(&before, &after);
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].addr, 0x1000);
    }
}
