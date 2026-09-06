//! Muxeur **WebM / Matroska** pur Rust, sans dépendance, pour flux VP9.
//!
//! ## Pourquoi il existe à côté de [`crate::mp4`]
//!
//! Le corpus de cinématiques n'est pas dans un seul codec. Mesuré sur les 97 films du jeu
//! (`niers video catalogue`) : **75 en H.264**, **20 en MPEG-2**, **2 en VP9**. Les deux VP9 sont
//! `ev09_05300` et `ev20_01300` — c'est-à-dire les deux plus longs du jeu, 21 minutes chacun,
//! et leurs conteneurs le disent sans détour (`nomOrigine = "S:\…\VP9\ev20_01300.usm"`).
//!
//! Un `avc1` ne peut pas les transporter. WebM, si — et les moteurs web décodent VP9 nativement,
//! avec accélération matérielle. D'où ce second muxeur, bâti sur le même principe que le premier :
//! les octets du flux ne sont **pas** réencodés, seul le conteneur change.
//!
//! ## Ce qui est produit
//!
//! Un WebM à tailles connues (pas de `unknown size`), avec `Duration`, une grappe (`Cluster`) par
//! image-clé et une table `Cues`. C'est cette table qui rend le déplacement dans la timeline
//! instantané : sans elle, une webview cherche en devinant, et sur un film de 21 minutes elle
//! rate franchement sa cible.
//!
//! Piste vidéo seule, comme pour le MP4 : la bande-son HCA est servie à part (cf. l'en-tête de
//! [`crate::mp4`] pour le raisonnement complet).
//!
//! ## Vérité terrain
//!
//! Le drapeau image-clé et les dimensions viennent de l'**en-tête non compressé** de chaque
//! trame VP9 (§6.2 de la spécification VP9 : `frame_marker`, `profile`, `frame_type`, code de
//! synchronisation `0x49 0x83 0x42`, puis `frame_size`). Le conteneur USM, lui, ne dit ni où
//! sont les images-clés ni le profil.

extern crate alloc;

use alloc::vec::Vec;

use crate::FormatError;
use crate::mp4::Bits;

// ── Identifiants d'éléments EBML ──────────────────────────────────────────────
//
// Écrits en `u32` et sérialisés sur leur longueur naturelle : un identifiant EBML porte sa
// propre taille dans ses bits de tête, on ne peut donc pas le tronquer.

const ID_EBML: u32 = 0x1A45_DFA3;
const ID_EBML_VERSION: u32 = 0x4286;
const ID_EBML_READ_VERSION: u32 = 0x42F7;
const ID_EBML_MAX_ID_LENGTH: u32 = 0x42F2;
const ID_EBML_MAX_SIZE_LENGTH: u32 = 0x42F3;
const ID_DOC_TYPE: u32 = 0x4282;
const ID_DOC_TYPE_VERSION: u32 = 0x4287;
const ID_DOC_TYPE_READ_VERSION: u32 = 0x4285;

const ID_SEGMENT: u32 = 0x1853_8067;
const ID_SEEK_HEAD: u32 = 0x114D_9B74;
const ID_SEEK: u32 = 0x4DBB;
const ID_SEEK_ID: u32 = 0x53AB;
const ID_SEEK_POSITION: u32 = 0x53AC;
const ID_INFO: u32 = 0x1549_A966;
const ID_TIMESTAMP_SCALE: u32 = 0x002A_D7B1;
const ID_DURATION: u32 = 0x4489;
const ID_MUXING_APP: u32 = 0x4D80;
const ID_WRITING_APP: u32 = 0x5741;

const ID_TRACKS: u32 = 0x1654_AE6B;
const ID_TRACK_ENTRY: u32 = 0xAE;
const ID_TRACK_NUMBER: u32 = 0xD7;
const ID_TRACK_UID: u32 = 0x73C5;
const ID_TRACK_TYPE: u32 = 0x83;
const ID_FLAG_LACING: u32 = 0x9C;
const ID_DEFAULT_DURATION: u32 = 0x0023_E383;
const ID_CODEC_ID: u32 = 0x86;
const ID_VIDEO: u32 = 0xE0;
const ID_PIXEL_WIDTH: u32 = 0xB0;
const ID_PIXEL_HEIGHT: u32 = 0xBA;
const ID_DISPLAY_WIDTH: u32 = 0x54B0;
const ID_DISPLAY_HEIGHT: u32 = 0x54BA;

const ID_CLUSTER: u32 = 0x1F43_B675;
const ID_TIMESTAMP: u32 = 0xE7;
const ID_SIMPLE_BLOCK: u32 = 0xA3;

const ID_CUES: u32 = 0x1C53_BB6B;
const ID_CUE_POINT: u32 = 0xBB;
const ID_CUE_TIME: u32 = 0xB3;
const ID_CUE_TRACK_POSITIONS: u32 = 0xB7;
const ID_CUE_TRACK: u32 = 0xF7;
const ID_CUE_CLUSTER_POSITION: u32 = 0xF1;

/// Base de temps du fichier : 1 ms par unité, la valeur canonique d'un WebM.
const ECHELLE_NS: u64 = 1_000_000;

/// Durée maximale d'une grappe, en millisecondes.
///
/// Le timestamp d'un `SimpleBlock` est **relatif à sa grappe et signé sur 16 bits** : au-delà de
/// 32,767 s d'écart il déborde silencieusement et le film se joue dans le désordre. 30 s laisse
/// une marge franche tout en gardant peu de grappes.
const GRAPPE_MAX_MS: u64 = 30_000;

// ── En-tête non compressé VP9 ─────────────────────────────────────────────────

/// Ce qu'on lit dans l'en-tête non compressé d'une trame VP9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrameVp9 {
    /// Image-clé (`frame_type == 0`) : point de synchronisation.
    pub cle: bool,
    /// Profil VP9 (0 à 3).
    pub profil: u8,
    /// Largeur, renseignée par les seules images-clés.
    pub largeur: u32,
    /// Hauteur, renseignée par les seules images-clés.
    pub hauteur: u32,
}

/// Code de synchronisation d'une image-clé VP9 (§6.2).
const SYNC_VP9: u32 = 0x0049_8342;

/// Lit l'en-tête non compressé d'une trame VP9.
///
/// # Erreurs
///
/// [`FormatError::TooShort`] si la trame est vide, [`FormatError::BadMagic`] si le marqueur de
/// trame ou le code de synchronisation ne correspondent pas — c'est-à-dire si ces octets ne sont
/// pas du VP9.
pub fn lire_trame_vp9(trame: &[u8]) -> Result<TrameVp9, FormatError> {
    if trame.len() < 2 {
        return Err(FormatError::TooShort {
            got: trame.len(),
            need: 2,
        });
    }
    let mut b = Bits::new(trame);
    if b.u(2) != 2 {
        return Err(FormatError::BadMagic {
            format: "VP9/frame_marker",
        });
    }
    let bas = b.u(1);
    let haut = b.u(1);
    let profil = (haut * 2 + bas) as u8;
    if profil == 3 {
        let _reserve = b.u(1);
    }
    if b.u(1) == 1 {
        // `show_existing_frame` : la trame ne fait que réafficher un tampon déjà décodé. Elle
        // n'est jamais une image-clé et ne porte aucune dimension.
        let _idx = b.u(3);
        return Ok(TrameVp9 {
            cle: false,
            profil,
            largeur: 0,
            hauteur: 0,
        });
    }
    let cle = b.u(1) == 0;
    let _show_frame = b.u(1);
    let _error_resilient = b.u(1);
    if !cle {
        return Ok(TrameVp9 {
            cle: false,
            profil,
            largeur: 0,
            hauteur: 0,
        });
    }

    if b.u(24) != SYNC_VP9 {
        return Err(FormatError::BadMagic {
            format: "VP9/frame_sync_code",
        });
    }
    // `color_config()` — traversée exacte, sans quoi `frame_size()` serait lu de travers.
    if profil >= 2 {
        let _dix_ou_douze_bits = b.u(1);
    }
    let espace_couleur = b.u(3);
    if espace_couleur == 7 {
        // SRGB : pas de plage, et un bit réservé sur les profils 1 et 3.
        if profil == 1 || profil == 3 {
            let _reserve = b.u(1);
        }
    } else {
        let _plage = b.u(1);
        if profil == 1 || profil == 3 {
            let _sous_ech_x = b.u(1);
            let _sous_ech_y = b.u(1);
            let _reserve = b.u(1);
        }
    }
    let largeur = b.u(16) + 1;
    let hauteur = b.u(16) + 1;
    Ok(TrameVp9 {
        cle: true,
        profil,
        largeur,
        hauteur,
    })
}

// ── Écriture EBML ─────────────────────────────────────────────────────────────

/// Tampon EBML avec pile d'éléments maîtres ouverts.
///
/// Les tailles sont réservées sur **8 octets** (`VINT` de longueur maximale) puis réécrites à la
/// fermeture : ça garantit qu'aucune position déjà calculée ne bouge, ce dont dépend la table
/// `Cues`, qui pointe des offsets absolus de grappes.
struct Ebml {
    o: Vec<u8>,
    ouverts: Vec<usize>,
}

impl Ebml {
    fn new() -> Self {
        Self {
            o: Vec::with_capacity(64 * 1024),
            ouverts: Vec::new(),
        }
    }

    /// Écrit un identifiant sur sa longueur naturelle (1 à 4 octets).
    fn id(&mut self, id: u32) {
        let octets = id.to_be_bytes();
        let debut = octets.iter().position(|&b| b != 0).unwrap_or(3);
        self.o.extend_from_slice(&octets[debut..]);
    }

    /// Écrit une taille sur 8 octets (`VINT` de longueur 8, marqueur `0x01`).
    fn taille8(&mut self, v: u64) {
        let mut octets = v.to_be_bytes();
        octets[0] |= 0x01;
        self.o.extend_from_slice(&octets);
    }

    /// Ouvre un élément maître : identifiant + taille réservée.
    fn ouvrir(&mut self, id: u32) {
        self.id(id);
        self.ouverts.push(self.o.len());
        self.taille8(0);
    }

    /// Referme le dernier élément ouvert et inscrit sa taille réelle.
    fn fermer(&mut self) {
        let pos = self.ouverts.pop().expect("fermer() sans ouvrir()");
        let taille = (self.o.len() - pos - 8) as u64;
        let mut octets = taille.to_be_bytes();
        octets[0] |= 0x01;
        self.o[pos..pos + 8].copy_from_slice(&octets);
    }

    /// Élément entier non signé, sur le nombre d'octets strictement nécessaire.
    fn entier(&mut self, id: u32, v: u64) {
        self.id(id);
        let octets = v.to_be_bytes();
        let debut = octets.iter().position(|&b| b != 0).unwrap_or(7);
        let n = 8 - debut;
        self.o.push(0x80 | n as u8);
        self.o.extend_from_slice(&octets[debut..]);
    }

    /// Élément flottant 64 bits (`Duration`).
    fn flottant(&mut self, id: u32, v: f64) {
        self.id(id);
        self.o.push(0x88);
        self.o.extend_from_slice(&v.to_be_bytes());
    }

    /// Entier non signé sur un nombre d'octets IMPOSÉ (8), pour rester patchable après coup.
    ///
    /// [`Ebml::entier`] écrit au plus court, donc sa taille dépend de la valeur : impossible d'y
    /// réserver une place et d'y revenir. C'est ce qu'exige le `SeekHead`, écrit avant les
    /// éléments dont il donne les positions.
    fn entier_fixe8(&mut self, id: u32) -> usize {
        self.id(id);
        self.o.push(0x88); // taille 8, sur un octet
        let pos = self.o.len();
        self.o.extend_from_slice(&[0u8; 8]);
        pos
    }

    /// Inscrit une valeur dans un emplacement réservé par [`Ebml::entier_fixe8`].
    fn patcher8(&mut self, pos: usize, v: u64) {
        self.o[pos..pos + 8].copy_from_slice(&v.to_be_bytes());
    }

    /// Élément binaire portant l'identifiant d'un autre élément (`SeekID`).
    fn id_binaire(&mut self, id: u32, cible: u32) {
        self.id(id);
        let octets = cible.to_be_bytes();
        let debut = octets.iter().position(|&b| b != 0).unwrap_or(3);
        self.o.push(0x80 | (4 - debut) as u8);
        self.o.extend_from_slice(&octets[debut..]);
    }

    /// Élément chaîne UTF-8.
    fn texte(&mut self, id: u32, s: &str) {
        self.id(id);
        let n = s.len();
        // Les chaînes écrites ici sont courtes et connues ; une taille sur 1 octet suffit.
        self.o.push(0x80 | n.min(127) as u8);
        self.o.extend_from_slice(&s.as_bytes()[..n.min(127)]);
    }
}

// ── Muxage ────────────────────────────────────────────────────────────────────

/// Ce que le muxeur WebM a produit.
#[derive(Debug, Clone, Copy)]
pub struct Resume {
    /// Largeur lue dans la première image-clé.
    pub largeur: u32,
    /// Hauteur lue dans la première image-clé.
    pub hauteur: u32,
    /// Profil VP9.
    pub profil: u8,
    /// Nombre d'images écrites.
    pub images: u32,
    /// Nombre d'images-clés.
    pub cles: u32,
    /// Durée totale, en secondes.
    pub secondes: f64,
}

/// Muxe des trames VP9 en un fichier WebM.
///
/// `cadence` est le couple `(numérateur, dénominateur)` de la cadence — l'USM le déclare
/// exactement (`framerate_n`/`framerate_d`), et VP9 ne le porte pas dans son bitstream.
/// `affichage` force les dimensions de présentation quand elles diffèrent des dimensions codées.
///
/// # Erreurs
///
/// [`FormatError::Corrupt`] si le flux est vide ou ne contient aucune image-clé — sans image-clé
/// un décodeur n'a aucun point d'entrée. [`FormatError::BadMagic`] si les octets ne sont pas du
/// VP9.
pub fn muxer_vp9(
    trames: &[&[u8]],
    cadence: (u32, u32),
    affichage: Option<(u32, u32)>,
) -> Result<(Vec<u8>, Resume), FormatError> {
    if trames.is_empty() {
        return Err(FormatError::Corrupt("WebM : aucune image à muxer"));
    }
    let (num, den) = if cadence.0 > 0 && cadence.1 > 0 {
        cadence
    } else {
        (30, 1)
    };

    // Première passe : en-têtes de trame. Elle donne les dimensions, le profil et les clés.
    let mut entetes = Vec::with_capacity(trames.len());
    for t in trames {
        entetes.push(lire_trame_vp9(t)?);
    }
    let Some(premiere_cle) = entetes.iter().find(|e| e.cle && e.largeur > 0) else {
        return Err(FormatError::Corrupt("WebM : flux VP9 sans image-clé"));
    };
    let (largeur, hauteur, profil) = (
        premiere_cle.largeur,
        premiere_cle.hauteur,
        premiere_cle.profil,
    );

    // Horodatage : millisecondes exactes calculées en entiers depuis la cadence rationnelle.
    // Passer par un `f64` accumulé dériverait de plusieurs images sur 21 minutes de film.
    let ms_de = |image: u64| -> u64 { image * 1000 * u64::from(den) / u64::from(num) };
    let duree_ms = ms_de(trames.len() as u64);
    let duree_image_ns = 1_000_000_000u64 * u64::from(den) / u64::from(num);

    let mut e = Ebml::new();

    // ── En-tête EBML ──
    e.ouvrir(ID_EBML);
    e.entier(ID_EBML_VERSION, 1);
    e.entier(ID_EBML_READ_VERSION, 1);
    e.entier(ID_EBML_MAX_ID_LENGTH, 4);
    e.entier(ID_EBML_MAX_SIZE_LENGTH, 8);
    e.texte(ID_DOC_TYPE, "webm");
    e.entier(ID_DOC_TYPE_VERSION, 2);
    e.entier(ID_DOC_TYPE_READ_VERSION, 2);
    e.fermer();

    // ── Segment ──
    e.ouvrir(ID_SEGMENT);
    // Toutes les positions (`SeekHead`, `Cues`) sont relatives au début des DONNÉES du segment.
    let base_segment = e.o.len();

    // `SeekHead` — l'index des index. **Sans lui, un navigateur ne peut pas jouer un gros
    // fichier** : `Cues` est en fin de fichier, et pour la trouver il faut soit un `SeekHead`,
    // soit balayer tout ce qu'il y a avant. Constaté sur `ev09_05300` (311 Mo) : Chrome restait
    // à `readyState = 0` en téléchargeant l'intégralité du flux avant la première image.
    // Les trois positions sont réservées sur 8 octets et inscrites une fois connues.
    e.ouvrir(ID_SEEK_HEAD);
    let mut renvois = Vec::with_capacity(3);
    for cible in [ID_INFO, ID_TRACKS, ID_CUES] {
        e.ouvrir(ID_SEEK);
        e.id_binaire(ID_SEEK_ID, cible);
        renvois.push(e.entier_fixe8(ID_SEEK_POSITION));
        e.fermer();
    }
    e.fermer();

    let pos_info = (e.o.len() - base_segment) as u64;
    e.ouvrir(ID_INFO);
    e.entier(ID_TIMESTAMP_SCALE, ECHELLE_NS);
    e.flottant(ID_DURATION, duree_ms as f64);
    e.texte(ID_MUXING_APP, "niers");
    e.texte(ID_WRITING_APP, "niers");
    e.fermer();

    let pos_tracks = (e.o.len() - base_segment) as u64;
    e.ouvrir(ID_TRACKS);
    e.ouvrir(ID_TRACK_ENTRY);
    e.entier(ID_TRACK_NUMBER, 1);
    e.entier(ID_TRACK_UID, 1);
    e.entier(ID_TRACK_TYPE, 1); // vidéo
    e.entier(ID_FLAG_LACING, 0);
    e.entier(ID_DEFAULT_DURATION, duree_image_ns);
    e.texte(ID_CODEC_ID, "V_VP9");
    e.ouvrir(ID_VIDEO);
    e.entier(ID_PIXEL_WIDTH, u64::from(largeur));
    e.entier(ID_PIXEL_HEIGHT, u64::from(hauteur));
    if let Some((l, h)) =
        affichage.filter(|(l, h)| *l > 0 && *h > 0 && (*l, *h) != (largeur, hauteur))
    {
        e.entier(ID_DISPLAY_WIDTH, u64::from(l));
        e.entier(ID_DISPLAY_HEIGHT, u64::from(h));
    }
    e.fermer(); // Video
    e.fermer(); // TrackEntry
    e.fermer(); // Tracks

    // ── Grappes ──
    //
    // Une nouvelle grappe s'ouvre sur chaque image-clé (c'est ce qui rend le point cherchable)
    // et dès que la précédente atteint GRAPPE_MAX_MS.
    let mut reperes: Vec<(u64, u64)> = Vec::new(); // (temps ms, position de la grappe)
    let mut cles = 0u32;
    let mut i = 0usize;
    while i < trames.len() {
        let debut_ms = ms_de(i as u64);
        let position = (e.o.len() - base_segment) as u64;
        if entetes[i].cle {
            reperes.push((debut_ms, position));
            cles += 1;
        }
        e.ouvrir(ID_CLUSTER);
        e.entier(ID_TIMESTAMP, debut_ms);

        // Remplissage de la grappe.
        while i < trames.len() {
            let ms = ms_de(i as u64);
            let ecart = ms - debut_ms;
            // Une image-clé qui n'ouvre pas cette grappe la referme : sans ça, le repère
            // pointerait au milieu d'une grappe, ce que `Cues` ne sait pas exprimer.
            if ecart > GRAPPE_MAX_MS || (entetes[i].cle && ms != debut_ms) {
                break;
            }
            e.id(ID_SIMPLE_BLOCK);
            e.taille8(trames[i].len() as u64 + 4);
            e.o.push(0x81); // numéro de piste 1, en VINT d'un octet
            e.o.extend_from_slice(&(ecart as i16).to_be_bytes());
            e.o.push(if entetes[i].cle { 0x80 } else { 0x00 });
            e.o.extend_from_slice(trames[i]);
            i += 1;
        }
        e.fermer(); // Cluster
    }

    // ── Cues ──
    let pos_cues = (e.o.len() - base_segment) as u64;
    e.ouvrir(ID_CUES);
    for (ms, position) in &reperes {
        e.ouvrir(ID_CUE_POINT);
        e.entier(ID_CUE_TIME, *ms);
        e.ouvrir(ID_CUE_TRACK_POSITIONS);
        e.entier(ID_CUE_TRACK, 1);
        e.entier(ID_CUE_CLUSTER_POSITION, *position);
        e.fermer();
        e.fermer();
    }
    e.fermer(); // Cues

    // Les trois positions sont désormais connues : on les inscrit dans le `SeekHead`.
    for (emplacement, position) in renvois.iter().zip([pos_info, pos_tracks, pos_cues]) {
        e.patcher8(*emplacement, position);
    }

    e.fermer(); // Segment

    let resume = Resume {
        largeur,
        hauteur,
        profil,
        images: trames.len() as u32,
        cles,
        secondes: duree_ms as f64 / 1000.0,
    };
    Ok((e.o, resume))
}

// ── Inspection ────────────────────────────────────────────────────────────────

/// Parcourt les éléments de premier niveau d'un WebM et rend `(identifiant, taille)`.
///
/// Sert aux tests et au diagnostic, comme [`crate::mp4::boites_racine`] : un fichier dont les
/// tailles ne se chaînent pas jusqu'à sa fin exacte est malformé.
#[must_use]
pub fn elements_racine(webm: &[u8]) -> Vec<(u32, u64)> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while p < webm.len() {
        let Some((id, n_id)) = lire_id(&webm[p..]) else {
            break;
        };
        let Some((taille, n_taille)) = lire_taille(&webm[p + n_id..]) else {
            break;
        };
        let total = n_id as u64 + n_taille as u64 + taille;
        if p as u64 + total > webm.len() as u64 {
            break;
        }
        out.push((id, total));
        p += total as usize;
    }
    out
}

/// Lit un identifiant EBML, rend `(valeur, longueur)`.
fn lire_id(d: &[u8]) -> Option<(u32, usize)> {
    let premier = *d.first()?;
    let n = match premier.leading_zeros() {
        0 => 1,
        1 => 2,
        2 => 3,
        3 => 4,
        _ => return None,
    };
    if d.len() < n {
        return None;
    }
    let mut v = 0u32;
    for &b in &d[..n] {
        v = (v << 8) | u32::from(b);
    }
    Some((v, n))
}

/// Lit une taille EBML (`VINT`), rend `(valeur, longueur)`.
fn lire_taille(d: &[u8]) -> Option<(u64, usize)> {
    let premier = *d.first()?;
    let n = premier.leading_zeros() as usize + 1;
    if n > 8 || d.len() < n {
        return None;
    }
    // `0xFF >> 8` déborde sur un `u8` : un `VINT` de 8 octets n'a aucun bit de valeur dans son
    // premier octet, tout y est marqueur de longueur. C'est précisément la forme qu'écrit ce
    // muxeur, donc le cas nominal et non un cas limite.
    let masque = if n == 8 { 0u8 } else { 0xFFu8 >> n };
    let mut v = u64::from(premier & masque);
    for &b in &d[1..n] {
        v = (v << 8) | u64::from(b);
    }
    Some((v, n))
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    /// Écrivain de bits minimal, pour fabriquer des en-têtes VP9 exacts sans embarquer
    /// d'octets du jeu (assets © Level-5, hors dépôt).
    struct Ecrivain {
        o: Vec<u8>,
        n: u32,
        cur: u8,
    }

    impl Ecrivain {
        fn new() -> Self {
            Self {
                o: Vec::new(),
                n: 0,
                cur: 0,
            }
        }
        fn u(&mut self, v: u32, bits: u32) {
            for i in (0..bits).rev() {
                self.cur = (self.cur << 1) | ((v >> i) & 1) as u8;
                self.n += 1;
                if self.n == 8 {
                    self.o.push(self.cur);
                    self.cur = 0;
                    self.n = 0;
                }
            }
        }
        fn fin(mut self) -> Vec<u8> {
            while self.n != 0 {
                self.u(0, 1);
            }
            self.o
        }
    }

    /// Trame VP9 profil 0, 4:2:0, aux dimensions demandées.
    fn trame(cle: bool, largeur: u32, hauteur: u32) -> Vec<u8> {
        let mut w = Ecrivain::new();
        w.u(2, 2); // frame_marker
        w.u(0, 1); // profile_low_bit
        w.u(0, 1); // profile_high_bit → profil 0
        w.u(0, 1); // show_existing_frame
        w.u(if cle { 0 } else { 1 }, 1); // frame_type
        w.u(1, 1); // show_frame
        w.u(0, 1); // error_resilient
        if cle {
            w.u(SYNC_VP9, 24);
            w.u(0, 3); // color_space (BT.601)
            w.u(0, 1); // color_range
            w.u(largeur - 1, 16);
            w.u(hauteur - 1, 16);
        }
        let mut v = w.fin();
        // Un peu de charge utile : une trame réelle n'est pas que son en-tête.
        v.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        v
    }

    #[test]
    fn l_entete_de_trame_donne_la_cle_et_les_dimensions() {
        let t = lire_trame_vp9(&trame(true, 1920, 1080)).expect("image-clé");
        assert!(t.cle);
        assert_eq!((t.largeur, t.hauteur), (1920, 1080));
        assert_eq!(t.profil, 0);

        let t = lire_trame_vp9(&trame(false, 1920, 1080)).expect("image intermédiaire");
        assert!(!t.cle);
        assert_eq!(
            (t.largeur, t.hauteur),
            (0, 0),
            "seule une image-clé porte les dimensions"
        );
    }

    #[test]
    fn des_octets_qui_ne_sont_pas_du_vp9_sont_refuses() {
        // `frame_marker` doit valoir 2 (bits de tête `10`).
        assert!(matches!(
            lire_trame_vp9(&[0x00, 0x00, 0x00]),
            Err(FormatError::BadMagic {
                format: "VP9/frame_marker"
            })
        ));
        assert!(matches!(
            lire_trame_vp9(&[0x82]),
            Err(FormatError::TooShort { .. })
        ));
        // Bon marqueur, image-clé annoncée, mais pas de code de synchronisation.
        let mut w = Ecrivain::new();
        w.u(2, 2);
        w.u(0, 1);
        w.u(0, 1);
        w.u(0, 1);
        w.u(0, 1); // image-clé
        w.u(1, 1);
        w.u(0, 1);
        w.u(0x00_1234, 24); // mauvais code
        assert!(matches!(
            lire_trame_vp9(&w.fin()),
            Err(FormatError::BadMagic {
                format: "VP9/frame_sync_code"
            })
        ));
    }

    #[test]
    fn le_webm_produit_a_des_elements_qui_se_chainent() {
        let k = trame(true, 1920, 1080);
        let p = trame(false, 1920, 1080);
        let trames: Vec<&[u8]> = vec![&k, &p, &p, &p];
        let (webm, r) = muxer_vp9(&trames, (30, 1), None).expect("muxage");

        assert_eq!((r.largeur, r.hauteur), (1920, 1080));
        assert_eq!(r.images, 4);
        assert_eq!(r.cles, 1);
        // La durée est exprimée en millisecondes ENTIÈRES (base de temps du conteneur) :
        // 4 images à 30 i/s = 133 ms, pas 133,333 — l'arrondi appartient au format.
        assert!(
            (r.secondes - 0.133).abs() < 1e-9,
            "durée obtenue {}",
            r.secondes
        );

        let racine = elements_racine(&webm);
        assert_eq!(racine.len(), 2, "un en-tête EBML et un segment");
        assert_eq!(racine[0].0, ID_EBML);
        assert_eq!(racine[1].0, ID_SEGMENT);
        let total: u64 = racine.iter().map(|(_, t)| t).sum();
        assert_eq!(
            total,
            webm.len() as u64,
            "les tailles couvrent tout le fichier"
        );

        // Le type de document doit être lisible tel quel — c'est lui que teste un navigateur.
        assert!(webm.windows(4).any(|w| w == b"webm"));
        assert!(webm.windows(5).any(|w| w == b"V_VP9"));
    }

    #[test]
    fn chaque_image_cle_ouvre_une_grappe_et_un_repere() {
        let k = trame(true, 640, 480);
        let p = trame(false, 640, 480);
        // Trois images-clés séparées par deux images intermédiaires.
        let trames: Vec<&[u8]> = vec![&k, &p, &p, &k, &p, &p, &k];
        let (webm, r) = muxer_vp9(&trames, (30, 1), None).expect("muxage");
        assert_eq!(r.cles, 3);

        // Autant de grappes que de points d'entrée, et autant de `CuePoint`.
        let grappes = compter(&webm, ID_CLUSTER);
        let points = compter(&webm, ID_CUE_POINT);
        assert_eq!(grappes, 3, "une grappe par image-clé");
        assert_eq!(points, 3, "un repère de recherche par image-clé");
    }

    #[test]
    fn le_seekhead_pointe_reellement_sur_info_tracks_et_cues() {
        let k = trame(true, 640, 480);
        let p = trame(false, 640, 480);
        let trames: Vec<&[u8]> = vec![&k, &p, &k, &p];
        let (webm, _) = muxer_vp9(&trames, (30, 1), None).expect("muxage");

        // Début des données du segment : après l'en-tête EBML, l'identifiant et la taille.
        let racine = elements_racine(&webm);
        let debut_segment_element = racine[0].1 as usize;
        let (_, n_id) = lire_id(&webm[debut_segment_element..]).expect("id segment");
        let (_, n_taille) =
            lire_taille(&webm[debut_segment_element + n_id..]).expect("taille segment");
        let base = debut_segment_element + n_id + n_taille;

        // Les trois `SeekPosition` sont écrites dans l'ordre Info, Tracks, Cues.
        let mut positions = Vec::new();
        let motif = ID_SEEK_POSITION.to_be_bytes();
        let motif = &motif[2..]; // identifiant sur 2 octets
        let mut p = base;
        while let Some(i) = webm[p..].windows(2).position(|w| w == motif) {
            let debut = p + i + 3; // id (2) + octet de taille (1)
            if debut + 8 > webm.len() || positions.len() == 3 {
                break;
            }
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&webm[debut..debut + 8]);
            positions.push(u64::from_be_bytes(buf));
            p = debut + 8;
        }
        assert_eq!(positions.len(), 3, "trois renvois attendus");

        for (position, attendu) in positions.iter().zip([ID_INFO, ID_TRACKS, ID_CUES]) {
            let absolu = base + *position as usize;
            let (trouve, _) = lire_id(&webm[absolu..]).expect("élément à la position visée");
            assert_eq!(trouve, attendu, "le renvoi ne tombe pas sur le bon élément");
        }
    }

    #[test]
    fn un_flux_sans_image_cle_est_refuse() {
        let p = trame(false, 640, 480);
        let trames: Vec<&[u8]> = vec![&p, &p];
        assert!(matches!(
            muxer_vp9(&trames, (30, 1), None),
            Err(FormatError::Corrupt(_))
        ));
        assert!(matches!(
            muxer_vp9(&[], (30, 1), None),
            Err(FormatError::Corrupt(_))
        ));
    }

    #[test]
    fn les_dimensions_de_presentation_ne_sont_ecrites_que_si_elles_different() {
        let k = trame(true, 1920, 1088);
        let trames: Vec<&[u8]> = vec![&k];
        let (avec, _) = muxer_vp9(&trames, (30, 1), Some((1920, 1080))).expect("muxage");
        let (sans, _) = muxer_vp9(&trames, (30, 1), Some((1920, 1088))).expect("muxage");
        assert!(
            avec.len() > sans.len(),
            "DisplayWidth/Height ajoutent des octets"
        );
        assert_eq!(compter(&avec, ID_DISPLAY_WIDTH), 1);
        assert_eq!(compter(&sans, ID_DISPLAY_WIDTH), 0);
    }

    /// Éléments maîtres du fichier produit — les seuls dans lesquels [`compter`] descend.
    const MAITRES: [u32; 12] = [
        ID_EBML,
        ID_SEGMENT,
        ID_SEEK_HEAD,
        ID_SEEK,
        ID_INFO,
        ID_TRACKS,
        ID_TRACK_ENTRY,
        ID_VIDEO,
        ID_CLUSTER,
        ID_CUES,
        ID_CUE_POINT,
        ID_CUE_TRACK_POSITIONS,
    ];

    /// Compte les éléments d'un identifiant donné, en PARCOURANT l'arbre EBML.
    ///
    /// Chercher le motif d'octets suffirait pour un identifiant de 4 octets, mais pas pour
    /// `CuePoint` (`0xBB`) ni `CueTrack` (`0xF7`), qui tiennent sur un seul octet et se
    /// rencontrent par hasard dans n'importe quelle charge utile compressée : la version naïve
    /// comptait 11 `CuePoint` là où il y en avait 3.
    fn compter(d: &[u8], id: u32) -> usize {
        let mut n = 0usize;
        let mut p = 0usize;
        while p < d.len() {
            let Some((element, n_id)) = lire_id(&d[p..]) else {
                break;
            };
            let Some((taille, n_taille)) = lire_taille(&d[p + n_id..]) else {
                break;
            };
            let debut = p + n_id + n_taille;
            let fin = debut + taille as usize;
            if fin > d.len() {
                break;
            }
            if element == id {
                n += 1;
            }
            if MAITRES.contains(&element) {
                n += compter(&d[debut..fin], id);
            }
            p = fin;
        }
        n
    }
}
