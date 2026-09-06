//! Muxeur MP4 (ISO/IEC 14496-12 + 14496-15) **pur Rust, sans dépendance**, pour flux H.264.
//!
//! ## Pourquoi ce module existe
//!
//! `nie-model-serve` remuxait les vidéos du jeu en appelant `ffmpeg` en sous-processus
//! (`-c:v copy -movflags frag_keyframe+empty_moov`). Deux défauts mesurables :
//!
//! 1. **`ffmpeg` n'est pas dans le PATH de cette machine.** Le repli servait alors le flux
//!    H.264 Annex-B nu en `video/h264` — qu'**aucun** navigateur ne lit. La lecture vidéo
//!    était donc morte hors d'une machine outillée.
//! 2. Un aller-retour disque + processus par requête, pour une opération qui ne fait que
//!    **recopier** les octets : le remux ne réencode rien, il change de conteneur.
//!
//! Ce module produit le même MP4 en mémoire, sans processus externe. Le flux vidéo est
//! **conservé octet pour octet** (seuls les préfixes de start-code Annex-B `00 00 00 01`
//! deviennent des préfixes de longueur AVCC de 4 octets, cf. ISO/IEC 14496-15 §5.3.4.1.2) :
//! l'opération est sans perte et réversible.
//!
//! ## Ce qui est produit
//!
//! Un MP4 **progressif** (`ftyp` + `moov` + `mdat`, `moov` AVANT `mdat`) : la table
//! d'échantillons est complète dès le premier octet, donc `<video>` connaît la durée, sait
//! chercher (`seek`) et affiche une barre de progression pleine. C'est le point où un fMP4
//! `empty_moov` échoue : il n'a pas de durée tant que le flux n'est pas fini.
//!
//! Piste vidéo seule. La bande-son d'un USM est du HCA Criware, qu'aucun conteneur MP4 ne
//! transporte ; elle est servie à part en WAV (cf. `cri_audio::decode_to_wav`) et resynchronisée
//! par le lecteur. Encoder de l'AAC pour la loger ici demanderait un encodeur AAC — dépendance
//! C, et une perte de qualité sur une piste qu'on vient de décoder sans perte.
//!
//! ## Vérité terrain
//!
//! Le SPS est lu (Exp-Golomb, ISO/IEC 14496-10 §7.3.2.1.1) pour obtenir largeur, hauteur et
//! cadence : ces trois valeurs **ne sont pas fiables dans l'en-tête USM** de tous les fichiers
//! du jeu, alors qu'elles sont exactes dans le bitstream lui-même.

extern crate alloc;

use alloc::vec::Vec;

use crate::FormatError;

// ── Lecture binaire du bitstream H.264 ────────────────────────────────────────

/// Type d'unité NAL H.264 (`nal_unit_type`, ISO/IEC 14496-10 tableau 7-1).
pub mod nal {
    /// Tranche non-IDR.
    pub const SLICE: u8 = 1;
    /// Tranche d'une image IDR (point de synchronisation).
    pub const IDR: u8 = 5;
    /// SEI (informations d'enrichissement).
    pub const SEI: u8 = 6;
    /// Sequence Parameter Set.
    pub const SPS: u8 = 7;
    /// Picture Parameter Set.
    pub const PPS: u8 = 8;
    /// Access Unit Delimiter.
    pub const AUD: u8 = 9;
    /// Données de bourrage.
    pub const FILLER: u8 = 12;
}

/// Une unité NAL repérée dans un flux Annex-B, **sans** son start-code.
#[derive(Debug, Clone, Copy)]
pub struct Nal<'a> {
    /// `nal_unit_type` (5 bits de poids faible du premier octet).
    pub kind: u8,
    /// Octets de l'unité, en-tête NAL inclus.
    pub bytes: &'a [u8],
}

/// Découpe un flux Annex-B en unités NAL.
///
/// Accepte les deux formes de start-code (`00 00 01` et `00 00 00 01`) et tolère les octets
/// de bourrage `00` en fin d'unité (ils sont retirés, comme le veut §7.4.1.2.3).
#[must_use]
pub fn nals(annexb: &[u8]) -> Vec<Nal<'_>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    // Position du premier octet de l'unité courante, `None` tant qu'aucun start-code n'a été vu.
    let mut debut: Option<usize> = None;

    while i + 2 < annexb.len() {
        if annexb[i] == 0 && annexb[i + 1] == 0 && annexb[i + 2] == 1 {
            if let Some(d) = debut {
                pousser(&mut out, &annexb[d..i]);
            }
            i += 3;
            debut = Some(i);
        } else {
            i += 1;
        }
    }
    if let Some(d) = debut
        && d < annexb.len()
    {
        pousser(&mut out, &annexb[d..]);
    }
    out
}

/// Ajoute une unité en retirant le bourrage `00` de queue (`trailing_zero_8bits`).
fn pousser<'a>(out: &mut Vec<Nal<'a>>, brut: &'a [u8]) {
    let mut fin = brut.len();
    while fin > 0 && brut[fin - 1] == 0 {
        fin -= 1;
    }
    if fin == 0 {
        return;
    }
    let bytes = &brut[..fin];
    out.push(Nal {
        kind: bytes[0] & 0x1F,
        bytes,
    });
}

/// Retire les octets d'anti-émulation `0x03` (`00 00 03 xx` → `00 00 xx`) — §7.4.1.1.
#[must_use]
pub fn rbsp(nal: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(nal.len());
    let mut zeros = 0usize;
    for &b in nal {
        if zeros >= 2 && b == 0x03 {
            zeros = 0;
            continue;
        }
        if b == 0 {
            zeros += 1;
        } else {
            zeros = 0;
        }
        out.push(b);
    }
    out
}

/// Lecteur de bits big-endian avec codes Exp-Golomb (§9.1).
///
/// `pub(crate)` : le parseur d'en-tête VP9 de [`crate::webm`] lit les mêmes bits, sans les codes
/// Exp-Golomb. En réécrire un second serait deux fois la même arithmétique de décalage.
pub(crate) struct Bits<'a> {
    d: &'a [u8],
    /// Position courante, en bits.
    p: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(d: &'a [u8]) -> Self {
        Self { d, p: 0 }
    }

    /// Lit un bit ; `0` une fois le tampon épuisé (le parseur s'arrête sur une valeur bornée).
    pub(crate) fn bit(&mut self) -> u32 {
        let octet = self.p >> 3;
        if octet >= self.d.len() {
            self.p += 1;
            return 0;
        }
        let b = (self.d[octet] >> (7 - (self.p & 7))) & 1;
        self.p += 1;
        u32::from(b)
    }

    /// Lit `n` bits (n ≤ 32).
    pub(crate) fn u(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.bit();
        }
        v
    }

    /// Code Exp-Golomb non signé `ue(v)`.
    fn ue(&mut self) -> u32 {
        let mut zeros = 0u32;
        // Borne dure : un `ue(v)` légal tient sur 32 bits de préfixe.
        while self.bit() == 0 && zeros < 32 && (self.p >> 3) <= self.d.len() {
            zeros += 1;
        }
        if zeros == 0 {
            return 0;
        }
        (1u32 << zeros) - 1 + self.u(zeros)
    }

    /// Code Exp-Golomb signé `se(v)`.
    fn se(&mut self) -> i32 {
        let k = self.ue();
        let mag = i64::from(k.div_ceil(2));
        if k.is_multiple_of(2) {
            -(mag as i32)
        } else {
            mag as i32
        }
    }

    /// Épuisé ? Sert de garde-fou aux boucles pilotées par des `ue(v)` lus.
    fn fini(&self) -> bool {
        (self.p >> 3) > self.d.len()
    }
}

// ── SPS ───────────────────────────────────────────────────────────────────────

/// Ce qu'on retient d'un Sequence Parameter Set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sps {
    /// `profile_idc` (66 = Baseline, 77 = Main, 100 = High).
    pub profile_idc: u8,
    /// Octet de contraintes + bits réservés (`constraint_set*_flag`).
    pub constraints: u8,
    /// `level_idc` ×10 (40 = niveau 4.0).
    pub level_idc: u8,
    /// Largeur en pixels, recadrage appliqué.
    pub width: u32,
    /// Hauteur en pixels, recadrage appliqué.
    pub height: u32,
    /// Numérateur de la cadence (`time_scale`), `0` si le VUI ne la porte pas.
    pub fps_num: u32,
    /// Dénominateur de la cadence (`2 × num_units_in_tick`), `0` si absente.
    pub fps_den: u32,
}

impl Sps {
    /// Cadence en images par seconde, ou `None` si le SPS ne la déclare pas.
    #[must_use]
    pub fn fps(&self) -> Option<f64> {
        if self.fps_den == 0 || self.fps_num == 0 {
            return None;
        }
        Some(f64::from(self.fps_num) / f64::from(self.fps_den))
    }
}

/// Parse un SPS (unité NAL de type 7, en-tête compris).
///
/// # Erreurs
///
/// [`FormatError::BadMagic`] si l'unité n'est pas un SPS, [`FormatError::TooShort`] si elle est
/// tronquée, [`FormatError::Corrupt`] si les dimensions calculées sont absurdes.
pub fn parse_sps(nal_sps: &[u8]) -> Result<Sps, FormatError> {
    if nal_sps.len() < 5 {
        return Err(FormatError::TooShort {
            got: nal_sps.len(),
            need: 5,
        });
    }
    if nal_sps[0] & 0x1F != nal::SPS {
        return Err(FormatError::BadMagic {
            format: "H.264/SPS",
        });
    }
    let d = rbsp(&nal_sps[1..]);
    let mut b = Bits::new(&d);

    let profile_idc = b.u(8) as u8;
    let constraints = b.u(8) as u8;
    let level_idc = b.u(8) as u8;
    let _sps_id = b.ue();

    // Profils « High » : le SPS porte en plus le format de chrominance et les matrices.
    let mut chroma_format_idc = 1u32; // 4:2:0 par défaut (§7.4.2.1.1)
    let mut separate_colour_plane = 0u32;
    if matches!(
        profile_idc,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        chroma_format_idc = b.ue();
        if chroma_format_idc == 3 {
            separate_colour_plane = b.u(1);
        }
        let _bit_depth_luma = b.ue();
        let _bit_depth_chroma = b.ue();
        let _qpprime_y_zero = b.u(1);
        if b.u(1) == 1 {
            let listes = if chroma_format_idc == 3 { 12 } else { 8 };
            for i in 0..listes {
                if b.u(1) == 1 {
                    sauter_liste_echelle(&mut b, if i < 6 { 16 } else { 64 });
                }
            }
        }
    }

    let _log2_max_frame_num = b.ue();
    let pic_order_cnt_type = b.ue();
    if pic_order_cnt_type == 0 {
        let _log2_max_poc_lsb = b.ue();
    } else if pic_order_cnt_type == 1 {
        let _delta_pic_order_always_zero = b.u(1);
        let _offset_non_ref = b.se();
        let _offset_top_bottom = b.se();
        let cycle = b.ue().min(256);
        for _ in 0..cycle {
            let _ = b.se();
        }
    }
    let _max_num_ref_frames = b.ue();
    let _gaps_allowed = b.u(1);

    let width_mbs = b.ue() + 1;
    let height_map_units = b.ue() + 1;
    let frame_mbs_only = b.u(1);
    if frame_mbs_only == 0 {
        let _mb_adaptive = b.u(1);
    }
    let _direct_8x8 = b.u(1);

    let (mut crop_l, mut crop_r, mut crop_t, mut crop_b) = (0u32, 0u32, 0u32, 0u32);
    if b.u(1) == 1 {
        crop_l = b.ue();
        crop_r = b.ue();
        crop_t = b.ue();
        crop_b = b.ue();
    }

    // VUI : seule la section `timing_info` nous intéresse, mais elle vient après plusieurs
    // sections optionnelles qu'il faut traverser exactement.
    let (mut fps_num, mut fps_den) = (0u32, 0u32);
    if b.u(1) == 1 && !b.fini() {
        if b.u(1) == 1 {
            // aspect_ratio_info_present_flag
            let idc = b.u(8);
            if idc == 255 {
                let _sar_w = b.u(16);
                let _sar_h = b.u(16);
            }
        }
        if b.u(1) == 1 {
            let _overscan_appropriate = b.u(1);
        }
        if b.u(1) == 1 {
            let _video_format = b.u(3);
            let _full_range = b.u(1);
            if b.u(1) == 1 {
                let _primaries = b.u(8);
                let _transfer = b.u(8);
                let _matrix = b.u(8);
            }
        }
        if b.u(1) == 1 {
            let _top = b.ue();
            let _bottom = b.ue();
        }
        if b.u(1) == 1 && !b.fini() {
            // timing_info_present_flag
            let num_units_in_tick = b.u(32);
            let time_scale = b.u(32);
            let _fixed_frame_rate = b.u(1);
            if num_units_in_tick > 0 && time_scale > 0 {
                fps_num = time_scale;
                fps_den = num_units_in_tick.saturating_mul(2);
            }
        }
    }

    // Dimensions : §7.4.2.1.1, formules (7-13) à (7-19).
    let (sub_w, sub_h) = match (chroma_format_idc, separate_colour_plane) {
        (1, 0) => (2u32, 2u32), // 4:2:0
        (2, 0) => (2, 1),       // 4:2:2
        _ => (1, 1),            // 4:4:4 ou monochrome
    };
    let unites_v = (2 - frame_mbs_only) * height_map_units;
    let brut_w = width_mbs * 16;
    let brut_h = unites_v * 16;
    let coupe_w = (crop_l + crop_r) * sub_w;
    let coupe_h = (crop_t + crop_b) * sub_h * (2 - frame_mbs_only);
    if coupe_w >= brut_w || coupe_h >= brut_h {
        return Err(FormatError::Corrupt(
            "SPS : recadrage plus grand que l'image",
        ));
    }
    let width = brut_w - coupe_w;
    let height = brut_h - coupe_h;
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err(FormatError::Corrupt("SPS : dimensions hors bornes"));
    }

    Ok(Sps {
        profile_idc,
        constraints,
        level_idc,
        width,
        height,
        fps_num,
        fps_den,
    })
}

/// Traverse une liste d'échelle (`scaling_list`, §7.3.2.1.1.1) sans la conserver.
fn sauter_liste_echelle(b: &mut Bits<'_>, taille: u32) {
    let mut last = 8i32;
    let mut next = 8i32;
    for _ in 0..taille {
        if next != 0 {
            let delta = b.se();
            next = (last + delta + 256) % 256;
        }
        last = if next == 0 { last } else { next };
    }
}

// ── Écriture de boîtes ISO-BMFF ───────────────────────────────────────────────

/// Tampon d'écriture big-endian avec pile de boîtes ouvertes.
struct Boites {
    o: Vec<u8>,
    ouvertes: Vec<usize>,
}

impl Boites {
    fn new() -> Self {
        Self {
            o: Vec::with_capacity(64 * 1024),
            ouvertes: Vec::new(),
        }
    }

    fn u8(&mut self, v: u8) {
        self.o.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.o.extend_from_slice(&v.to_be_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.o.extend_from_slice(&v.to_be_bytes());
    }
    fn brut(&mut self, v: &[u8]) {
        self.o.extend_from_slice(v);
    }
    /// Boîte pleine : version 0 + flags nuls.
    fn version0(&mut self) {
        self.u32(0);
    }

    /// Ouvre une boîte : réserve la taille, écrit le type.
    fn ouvrir(&mut self, kind: &[u8; 4]) {
        self.ouvertes.push(self.o.len());
        self.u32(0);
        self.brut(kind);
    }

    /// Referme la boîte ouverte la plus récente en écrivant sa taille réelle.
    fn fermer(&mut self) {
        let debut = self.ouvertes.pop().expect("fermer() sans ouvrir()");
        let taille = (self.o.len() - debut) as u32;
        self.o[debut..debut + 4].copy_from_slice(&taille.to_be_bytes());
    }
}

// ── Muxage ────────────────────────────────────────────────────────────────────

/// Un échantillon MP4 : une unité d'accès complète, au format AVCC (NAL préfixées de leur
/// longueur sur 4 octets).
#[derive(Debug, Clone)]
pub struct Echantillon {
    /// Octets AVCC de l'unité d'accès.
    pub bytes: Vec<u8>,
    /// Vrai si l'unité contient une tranche IDR (échantillon de synchronisation).
    pub cle: bool,
}

/// Ce que le muxeur a produit, en plus des octets.
#[derive(Debug, Clone, Copy)]
pub struct Resume {
    /// SPS retenu pour la configuration du décodeur.
    pub sps: Sps,
    /// Nombre d'échantillons (images) écrits.
    pub images: u32,
    /// Nombre d'échantillons de synchronisation (IDR).
    pub cles: u32,
    /// Base de temps de la piste (unités par seconde).
    pub timescale: u32,
    /// Durée d'un échantillon, dans la base de temps.
    pub duree_image: u32,
}

impl Resume {
    /// Durée totale en secondes.
    #[must_use]
    pub fn secondes(&self) -> f64 {
        if self.timescale == 0 {
            return 0.0;
        }
        f64::from(self.images) * f64::from(self.duree_image) / f64::from(self.timescale)
    }
}

/// Convertit une unité d'accès Annex-B en échantillon AVCC.
///
/// Les délimiteurs (`AUD`) et le bourrage (`FILLER`) sont retirés : ils n'ont pas de sens dans
/// un conteneur qui délimite déjà les échantillons. SPS et PPS restent **en bande** en plus
/// d'être dans `avcC` — c'est légal (14496-15 §5.3.4.1) et rend chaque IDR autonome.
fn en_avcc(unite: &[u8]) -> Echantillon {
    let mut bytes = Vec::with_capacity(unite.len() + 16);
    let mut cle = false;
    for n in nals(unite) {
        if n.kind == nal::AUD || n.kind == nal::FILLER {
            continue;
        }
        if n.kind == nal::IDR {
            cle = true;
        }
        bytes.extend_from_slice(&(n.bytes.len() as u32).to_be_bytes());
        bytes.extend_from_slice(n.bytes);
    }
    Echantillon { bytes, cle }
}

/// Cadence par défaut quand ni l'USM ni le SPS ne la déclarent : 30 i/s.
///
/// Choisie plutôt que 60 (l'ancienne valeur passée à `ffmpeg -r 60`) parce que c'est la cadence
/// des cinématiques du jeu ; une erreur ici n'altère aucun pixel, seulement le rythme.
pub const FPS_DEFAUT: (u32, u32) = (30, 1);

/// Réglages du muxage.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Cadence forcée `(numérateur, dénominateur)`. `None` : le SPS décide, puis [`FPS_DEFAUT`].
    pub cadence: Option<(u32, u32)>,
    /// Taille de **présentation** `(largeur, hauteur)`, écrite dans `tkhd`.
    ///
    /// Les cinématiques du jeu sont codées en 1920×**1088** (multiple de 16) mais destinées à
    /// 1920×**1080** : leur `VIDEO_HDRINFO` porte `disp_height = 1080` alors que le SPS n'a
    /// aucun rectangle de recadrage. Sans cette indication, un `<video>` affiche les 8 lignes
    /// de remplissage du bas. `tkhd` est précisément la boîte prévue pour ça (ISO/IEC 14496-12
    /// §8.3.2 : dimensions de présentation, distinctes des dimensions codées de `avc1`).
    /// Le navigateur met alors à l'échelle plutôt que de recadrer — 0,7 % d'écart vertical,
    /// invisible, contre une bande parasite bien visible.
    pub affichage: Option<(u32, u32)>,
}

/// Muxe des unités d'accès H.264 Annex-B en un MP4 progressif.
///
/// `cadence` force la cadence (numérateur, dénominateur) ; passer `None` laisse le SPS décider,
/// puis [`FPS_DEFAUT`] en dernier recours.
///
/// # Erreurs
///
/// Voir [`muxer_h264_avec`].
pub fn muxer_h264(
    unites: &[&[u8]],
    cadence: Option<(u32, u32)>,
) -> Result<(Vec<u8>, Resume), FormatError> {
    muxer_h264_avec(
        unites,
        &Options {
            cadence,
            affichage: None,
        },
    )
}

/// Muxe des unités d'accès H.264 Annex-B, réglages explicites.
///
/// # Erreurs
///
/// [`FormatError::Corrupt`] s'il n'y a aucune image, ou aucun SPS/PPS dans le flux — sans eux le
/// MP4 n'aurait pas de configuration de décodeur et ne serait lisible par rien.
pub fn muxer_h264_avec(
    unites: &[&[u8]],
    options: &Options,
) -> Result<(Vec<u8>, Resume), FormatError> {
    let cadence = options.cadence;
    if unites.is_empty() {
        return Err(FormatError::Corrupt("MP4 : aucune image à muxer"));
    }

    // Premier SPS et premier PPS rencontrés : ce sont eux qui configurent le décodeur.
    let mut sps_brut: Option<Vec<u8>> = None;
    let mut pps_brut: Option<Vec<u8>> = None;
    for u in unites {
        for n in nals(u) {
            match n.kind {
                nal::SPS if sps_brut.is_none() => sps_brut = Some(n.bytes.to_vec()),
                nal::PPS if pps_brut.is_none() => pps_brut = Some(n.bytes.to_vec()),
                _ => {}
            }
        }
        if sps_brut.is_some() && pps_brut.is_some() {
            break;
        }
    }
    let (Some(sps_brut), Some(pps_brut)) = (sps_brut, pps_brut) else {
        return Err(FormatError::Corrupt("MP4 : flux H.264 sans SPS/PPS"));
    };
    let sps = parse_sps(&sps_brut)?;

    let echantillons: Vec<Echantillon> = unites
        .iter()
        .map(|u| en_avcc(u))
        .filter(|e| !e.bytes.is_empty())
        .collect();
    if echantillons.is_empty() {
        return Err(FormatError::Corrupt(
            "MP4 : aucune unité d'accès exploitable",
        ));
    }

    // Base de temps DÉRIVÉE de la cadence, pas fixée à 90 kHz.
    //
    // Avec une horloge de 90 kHz, une image à 24000/1001 dure 3753,75 unités — un entier ne peut
    // pas l'exprimer, et l'erreur s'accumule : mesuré sur `ev01_00050`, 93,533 s annoncées contre
    // 93,552 s réelles, soit une demi-image de retard en fin de film. En prenant `timescale = num`
    // et `duree_image = den`, la durée redevient EXACTE pour toute cadence rationnelle. Le
    // multiplicateur ne sert qu'à éviter une horloge trop lente (30 Hz pour du 30 i/s), que
    // certains lecteurs rendent mal.
    let (num, den) = cadence
        .filter(|(n, d)| *n > 0 && *d > 0)
        .or_else(|| (sps.fps_num > 0 && sps.fps_den > 0).then_some((sps.fps_num, sps.fps_den)))
        .unwrap_or(FPS_DEFAUT);
    let m = 1000u64.div_ceil(u64::from(num)).max(1);
    let timescale = (u64::from(num) * m).min(u64::from(u32::MAX)) as u32;
    let duree_image = (u64::from(den) * m).max(1) as u32;
    let duree_totale = duree_image as u64 * echantillons.len() as u64;

    let cles: Vec<u32> = echantillons
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.cle.then_some(i as u32 + 1))
        .collect();

    // ── ftyp ──
    let mut b = Boites::new();
    b.ouvrir(b"ftyp");
    b.brut(b"isom");
    b.u32(0x0000_0200);
    b.brut(b"isom");
    b.brut(b"iso2");
    b.brut(b"avc1");
    b.brut(b"mp41");
    b.fermer();
    let ftyp_len = b.o.len();

    // Taille de présentation : celle demandée si elle est plausible, sinon celle du SPS.
    let affichage = match options.affichage {
        Some((l, h)) if l > 0 && h > 0 => (l, h),
        _ => (sps.width, sps.height),
    };

    // `moov` est écrit deux fois : la première passe donne sa taille (donc la position de
    // `mdat`), la seconde y inscrit les offsets réels. Les offsets occupent 4 octets fixes,
    // donc la taille ne change pas entre les deux passes — l'égalité est vérifiée plus bas.
    let mut moov = ecrire_moov(
        &sps,
        &sps_brut,
        &pps_brut,
        &echantillons,
        &cles,
        timescale,
        duree_image,
        duree_totale,
        0,
        affichage,
    );
    let debut_mdat = (ftyp_len + moov.len() + 8) as u32;
    let moov2 = ecrire_moov(
        &sps,
        &sps_brut,
        &pps_brut,
        &echantillons,
        &cles,
        timescale,
        duree_image,
        duree_totale,
        debut_mdat,
        affichage,
    );
    if moov2.len() != moov.len() {
        return Err(FormatError::Corrupt(
            "MP4 : taille de moov instable entre les deux passes",
        ));
    }
    moov = moov2;
    b.brut(&moov);

    // ── mdat ──
    let charge: usize = echantillons.iter().map(|e| e.bytes.len()).sum();
    b.u32((charge + 8) as u32);
    b.brut(b"mdat");
    for e in &echantillons {
        b.brut(&e.bytes);
    }

    let resume = Resume {
        sps,
        images: echantillons.len() as u32,
        cles: cles.len() as u32,
        timescale,
        duree_image,
    };
    Ok((b.o, resume))
}

/// Écrit la boîte `moov` complète. `base_mdat` est l'offset absolu du premier échantillon.
#[allow(clippy::too_many_arguments)]
fn ecrire_moov(
    sps: &Sps,
    sps_brut: &[u8],
    pps_brut: &[u8],
    echantillons: &[Echantillon],
    cles: &[u32],
    timescale: u32,
    duree_image: u32,
    duree_totale: u64,
    base_mdat: u32,
    affichage: (u32, u32),
) -> Vec<u8> {
    let mut b = Boites::new();
    let duree32 = u32::try_from(duree_totale).unwrap_or(u32::MAX);

    b.ouvrir(b"moov");

    // mvhd
    b.ouvrir(b"mvhd");
    b.version0();
    b.u32(0); // création
    b.u32(0); // modification
    b.u32(timescale);
    b.u32(duree32);
    b.u32(0x0001_0000); // vitesse 1.0
    b.u16(0x0100); // volume 1.0
    b.u16(0);
    b.u32(0);
    b.u32(0);
    matrice_identite(&mut b);
    for _ in 0..6 {
        b.u32(0); // pré-défini
    }
    b.u32(2); // prochain identifiant de piste
    b.fermer();

    // trak
    b.ouvrir(b"trak");

    b.ouvrir(b"tkhd");
    b.u32(0x0000_0007); // version 0, flags : activée + dans la présentation + dans l'aperçu
    b.u32(0);
    b.u32(0);
    b.u32(1); // track_id
    b.u32(0);
    b.u32(duree32);
    b.u32(0);
    b.u32(0);
    b.u16(0); // couche
    b.u16(0); // groupe alternatif
    b.u16(0); // volume (0 pour une piste vidéo)
    b.u16(0);
    matrice_identite(&mut b);
    b.u32(affichage.0 << 16); // largeur de présentation, en 16.16
    b.u32(affichage.1 << 16);
    b.fermer();

    b.ouvrir(b"mdia");

    b.ouvrir(b"mdhd");
    b.version0();
    b.u32(0);
    b.u32(0);
    b.u32(timescale);
    b.u32(duree32);
    b.u16(0x55C4); // langue « und » (ISO-639-2/T empaqueté sur 5 bits)
    b.u16(0);
    b.fermer();

    b.ouvrir(b"hdlr");
    b.version0();
    b.u32(0);
    b.brut(b"vide");
    b.u32(0);
    b.u32(0);
    b.u32(0);
    b.brut(b"niers\0"); // nom du gestionnaire, terminé par un nul
    b.fermer();

    b.ouvrir(b"minf");

    b.ouvrir(b"vmhd");
    b.u32(0x0000_0001); // version 0, flags = 1 (obligatoire)
    b.u16(0); // mode graphique
    b.u16(0);
    b.u16(0);
    b.u16(0);
    b.fermer();

    b.ouvrir(b"dinf");
    b.ouvrir(b"dref");
    b.version0();
    b.u32(1);
    b.ouvrir(b"url ");
    b.u32(0x0000_0001); // flags = 1 : la donnée est dans ce fichier
    b.fermer();
    b.fermer();
    b.fermer();

    b.ouvrir(b"stbl");

    // stsd → avc1 → avcC
    b.ouvrir(b"stsd");
    b.version0();
    b.u32(1);
    b.ouvrir(b"avc1");
    for _ in 0..6 {
        b.u8(0); // réservé
    }
    b.u16(1); // index de référence de données
    b.u16(0); // pré-défini
    b.u16(0); // réservé
    for _ in 0..3 {
        b.u32(0); // pré-défini
    }
    b.u16(sps.width as u16);
    b.u16(sps.height as u16);
    b.u32(0x0048_0000); // 72 ppp horizontal
    b.u32(0x0048_0000); // 72 ppp vertical
    b.u32(0); // réservé
    b.u16(1); // images par échantillon
    b.u8(0); // nom du compresseur : chaîne pascal de 32 octets, vide
    for _ in 0..31 {
        b.u8(0);
    }
    b.u16(0x0018); // profondeur 24 bits
    b.u16(0xFFFF); // table de couleurs : aucune
    b.ouvrir(b"avcC");
    b.u8(1); // version de configuration
    b.u8(sps.profile_idc);
    b.u8(sps.constraints);
    b.u8(sps.level_idc);
    b.u8(0xFF); // 6 bits à 1 + lengthSizeMinusOne = 3 (préfixes de 4 octets)
    b.u8(0xE1); // 3 bits à 1 + un seul SPS
    b.u16(sps_brut.len() as u16);
    b.brut(sps_brut);
    b.u8(1); // un seul PPS
    b.u16(pps_brut.len() as u16);
    b.brut(pps_brut);
    b.fermer();
    b.fermer();
    b.fermer();

    // stts : toutes les images ont la même durée.
    b.ouvrir(b"stts");
    b.version0();
    b.u32(1);
    b.u32(echantillons.len() as u32);
    b.u32(duree_image);
    b.fermer();

    // stss : échantillons de synchronisation. Omise si tout est clé (le lecteur le déduit).
    if !cles.is_empty() && cles.len() != echantillons.len() {
        b.ouvrir(b"stss");
        b.version0();
        b.u32(cles.len() as u32);
        for &k in cles {
            b.u32(k);
        }
        b.fermer();
    }

    // stsc : un échantillon par chunk — la forme la plus simple qui reste exacte.
    b.ouvrir(b"stsc");
    b.version0();
    b.u32(1);
    b.u32(1); // premier chunk
    b.u32(1); // échantillons par chunk
    b.u32(1); // index de description
    b.fermer();

    b.ouvrir(b"stsz");
    b.version0();
    b.u32(0); // tailles non uniformes
    b.u32(echantillons.len() as u32);
    for e in echantillons {
        b.u32(e.bytes.len() as u32);
    }
    b.fermer();

    b.ouvrir(b"stco");
    b.version0();
    b.u32(echantillons.len() as u32);
    let mut off = base_mdat;
    for e in echantillons {
        b.u32(off);
        off = off.saturating_add(e.bytes.len() as u32);
    }
    b.fermer();

    b.fermer(); // stbl
    b.fermer(); // minf
    b.fermer(); // mdia
    b.fermer(); // trak
    b.fermer(); // moov

    b.o
}

/// Matrice de transformation identité (format 16.16 sauf la dernière colonne, en 2.30).
fn matrice_identite(b: &mut Boites) {
    for v in [0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
        b.u32(v);
    }
}

// ── Inspection ────────────────────────────────────────────────────────────────

/// Parcourt les boîtes de premier niveau d'un MP4 et renvoie `(type, taille)`.
///
/// Sert aux tests et au diagnostic : un fichier dont les tailles ne se chaînent pas jusqu'à la
/// fin exacte du tampon est malformé, et c'est le premier symptôme d'un muxage cassé.
#[must_use]
pub fn boites_racine(mp4: &[u8]) -> Vec<([u8; 4], u64)> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while p + 8 <= mp4.len() {
        let taille = u32::from_be_bytes([mp4[p], mp4[p + 1], mp4[p + 2], mp4[p + 3]]) as u64;
        let mut kind = [0u8; 4];
        kind.copy_from_slice(&mp4[p + 4..p + 8]);
        let taille = if taille == 0 {
            (mp4.len() - p) as u64
        } else {
            taille
        };
        if taille < 8 || p as u64 + taille > mp4.len() as u64 {
            break;
        }
        out.push((kind, taille));
        p += taille as usize;
    }
    out
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    /// Écrivain de bits, miroir de [`Bits`] — sert à fabriquer des SPS de test exacts plutôt
    /// que d'embarquer des octets du jeu (assets © Level-5, hors dépôt).
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
        fn bit(&mut self, v: u32) {
            self.cur = (self.cur << 1) | (v as u8 & 1);
            self.n += 1;
            if self.n == 8 {
                self.o.push(self.cur);
                self.cur = 0;
                self.n = 0;
            }
        }
        fn u(&mut self, v: u32, n: u32) {
            for i in (0..n).rev() {
                self.bit((v >> i) & 1);
            }
        }
        fn ue(&mut self, v: u32) {
            let code = v + 1;
            let bits = 32 - code.leading_zeros();
            for _ in 0..bits - 1 {
                self.bit(0);
            }
            self.u(code, bits);
        }
        fn fin(mut self) -> Vec<u8> {
            self.bit(1); // rbsp_stop_one_bit
            while self.n != 0 {
                self.bit(0);
            }
            self.o
        }
    }

    /// Fabrique un SPS Baseline 4:2:0 aux dimensions voulues.
    ///
    /// `vui` porte le couple VUI brut `(time_scale, num_units_in_tick)` — la cadence vaut
    /// `time_scale / (2 × num_units_in_tick)`, cf. §E.2.1.
    fn sps_synthetique(mbs_w: u32, mbs_h: u32, crop_b: u32, vui: Option<(u32, u32)>) -> Vec<u8> {
        let mut w = Ecrivain::new();
        w.u(66, 8); // profile_idc = Baseline
        w.u(0, 8); // contraintes
        w.u(30, 8); // level 3.0
        w.ue(0); // sps_id
        w.ue(4); // log2_max_frame_num_minus4
        w.ue(2); // pic_order_cnt_type = 2 (aucun champ additionnel)
        w.ue(1); // max_num_ref_frames
        w.bit(0); // gaps_in_frame_num_value_allowed
        w.ue(mbs_w - 1);
        w.ue(mbs_h - 1);
        w.bit(1); // frame_mbs_only_flag
        w.bit(1); // direct_8x8_inference
        if crop_b > 0 {
            w.bit(1); // frame_cropping_flag
            w.ue(0);
            w.ue(0);
            w.ue(0);
            w.ue(crop_b);
        } else {
            w.bit(0);
        }
        match vui {
            None => w.bit(0),
            Some((time_scale, num_units_in_tick)) => {
                w.bit(1); // vui_parameters_present
                w.bit(0); // aspect_ratio_info_present
                w.bit(0); // overscan_info_present
                w.bit(0); // video_signal_type_present
                w.bit(0); // chroma_loc_info_present
                w.bit(1); // timing_info_present
                w.u(num_units_in_tick, 32);
                w.u(time_scale, 32);
                w.bit(1); // fixed_frame_rate_flag
            }
        }
        let mut nal = vec![0x67u8]; // nal_ref_idc = 3, type = 7
        nal.extend_from_slice(&w.fin());
        nal
    }

    fn annexb(nals: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        for n in nals {
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(n);
        }
        out
    }

    #[test]
    fn les_start_codes_de_trois_et_quatre_octets_decoupent_pareil() {
        let quatre = [0u8, 0, 0, 1, 0x67, 0xAA, 0, 0, 0, 1, 0x68, 0xBB];
        let trois = [0u8, 0, 1, 0x67, 0xAA, 0, 0, 1, 0x68, 0xBB];
        let a = nals(&quatre);
        let b = nals(&trois);
        assert_eq!(a.len(), 2);
        assert_eq!(b.len(), 2);
        assert_eq!(a[0].kind, nal::SPS);
        assert_eq!(a[1].kind, nal::PPS);
        assert_eq!(a[0].bytes, b[0].bytes);
        assert_eq!(a[1].bytes, b[1].bytes);
    }

    #[test]
    fn le_bourrage_de_queue_ne_fait_pas_partie_de_l_unite() {
        let flux = [0u8, 0, 0, 1, 0x65, 0x11, 0x22, 0, 0, 0];
        let n = nals(&flux);
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].bytes, &[0x65, 0x11, 0x22]);
    }

    #[test]
    fn l_anti_emulation_est_retiree() {
        assert_eq!(
            rbsp(&[0x67, 0, 0, 3, 1, 0, 0, 3, 2]),
            vec![0x67, 0, 0, 1, 0, 0, 2]
        );
        // Un 0x03 qui ne suit pas deux zéros n'est PAS un octet d'échappement.
        assert_eq!(rbsp(&[1, 3, 0, 3, 4]), vec![1, 3, 0, 3, 4]);
    }

    #[test]
    fn le_sps_donne_les_dimensions_recadrees() {
        // 40×30 macroblocs = 640×480 sans recadrage.
        let sps = parse_sps(&sps_synthetique(40, 30, 0, None)).expect("SPS");
        assert_eq!((sps.width, sps.height), (640, 480));
        assert_eq!(sps.profile_idc, 66);
        assert_eq!(sps.level_idc, 30);
        assert_eq!(sps.fps(), None);

        // 1920×1088 recadré de 4 unités chroma en bas → 1920×1080, le cas de toutes les
        // vidéos 1080p (1080 n'est pas un multiple de 16).
        let sps = parse_sps(&sps_synthetique(120, 68, 4, None)).expect("SPS 1080p");
        assert_eq!((sps.width, sps.height), (1920, 1080));
    }

    #[test]
    fn le_vui_donne_la_cadence() {
        let sps = parse_sps(&sps_synthetique(40, 30, 0, Some((60_000, 1001)))).expect("SPS");
        assert_eq!((sps.fps_num, sps.fps_den), (60_000, 2002));
        let fps = sps.fps().expect("cadence");
        assert!((fps - 29.97).abs() < 0.001, "cadence lue {fps}");
    }

    #[test]
    fn un_sps_qui_n_en_est_pas_un_est_refuse() {
        assert!(matches!(
            parse_sps(&[0x68, 1, 2, 3, 4]),
            Err(FormatError::BadMagic {
                format: "H.264/SPS"
            })
        ));
        assert!(matches!(
            parse_sps(&[0x67]),
            Err(FormatError::TooShort { .. })
        ));
    }

    #[test]
    fn le_mp4_produit_a_des_boites_qui_se_chainent() {
        let sps = sps_synthetique(40, 30, 0, Some((60, 1))); // 60 / (2×1) = 30 i/s
        let pps = vec![0x68u8, 0xCE, 0x3C, 0x80];
        let idr = vec![0x65u8, 0x88, 0x84, 0x00, 0x10, 0xFF];
        let inter = vec![0x41u8, 0x9A, 0x00, 0x20];

        let f0 = annexb(&[&[0x09u8, 0x10], &sps, &pps, &idr]);
        let f1 = annexb(&[&[0x09u8, 0x30], &inter]);
        let f2 = annexb(&[&inter]);
        let unites: Vec<&[u8]> = vec![&f0, &f1, &f2];

        let (mp4, r) = muxer_h264(&unites, None).expect("muxage");
        assert_eq!(r.images, 3);
        assert_eq!(r.cles, 1);
        assert_eq!((r.sps.width, r.sps.height), (640, 480));
        // 30 i/s exactement : base 60 × 17 = 1020, une image = 2 × 17 = 34 unités.
        assert_eq!((r.timescale, r.duree_image), (1020, 34));
        assert!(
            (r.secondes() - 0.1).abs() < 1e-9,
            "3 images à 30 i/s = 0,1 s"
        );

        let racine = boites_racine(&mp4);
        let types: Vec<&[u8]> = racine.iter().map(|(k, _)| &k[..]).collect();
        assert_eq!(
            types,
            vec![b"ftyp".as_slice(), b"moov".as_slice(), b"mdat".as_slice()]
        );
        let total: u64 = racine.iter().map(|(_, t)| t).sum();
        assert_eq!(
            total,
            mp4.len() as u64,
            "les tailles doivent couvrir tout le fichier"
        );
    }

    #[test]
    fn les_offsets_de_stco_pointent_sur_les_echantillons() {
        let sps = sps_synthetique(40, 30, 0, None);
        let pps = vec![0x68u8, 0xCE, 0x3C, 0x80];
        let idr = vec![0x65u8, 0xAA, 0xBB, 0xCC];
        let f0 = annexb(&[&sps, &pps, &idr]);
        let unites: Vec<&[u8]> = vec![&f0];
        let (mp4, _) = muxer_h264(&unites, Some((25, 1))).expect("muxage");

        // Le premier échantillon commence 8 octets après le début de `mdat`.
        let racine = boites_racine(&mp4);
        let debut_mdat: u64 = racine.iter().take(2).map(|(_, t)| t).sum();
        let attendu = (debut_mdat + 8) as u32;

        // `stco` : on la retrouve par son type, puis on lit son unique offset.
        let pos = mp4
            .windows(4)
            .position(|w| w == b"stco")
            .expect("stco présente");
        let offset =
            u32::from_be_bytes([mp4[pos + 12], mp4[pos + 13], mp4[pos + 14], mp4[pos + 15]]);
        assert_eq!(offset, attendu);
        // Et cet offset désigne bien le préfixe de longueur de la première NAL.
        let taille = u32::from_be_bytes([
            mp4[offset as usize],
            mp4[offset as usize + 1],
            mp4[offset as usize + 2],
            mp4[offset as usize + 3],
        ]);
        assert_eq!(
            taille as usize,
            sps.len(),
            "première NAL du premier échantillon = le SPS"
        );
    }

    #[test]
    fn la_duree_est_exacte_pour_une_cadence_non_entiere() {
        // 2243 images à 24000/1001 (23,976 i/s) = 93,551 916… s. Une horloge de 90 kHz aurait
        // arrondi chaque image à 3753 unités et perdu 19 ms sur le film.
        let sps = sps_synthetique(120, 68, 0, None);
        let pps = vec![0x68u8, 0xCE, 0x3C, 0x80];
        let idr = vec![0x65u8, 1, 2, 3];
        let f = annexb(&[&sps, &pps, &idr]);
        let unites: Vec<&[u8]> = vec![&f; 2243];

        let (_, r) = muxer_h264(&unites, Some((24_000, 1001))).expect("muxage");
        assert_eq!((r.timescale, r.duree_image), (24_000, 1001));
        let attendu = 2243.0 * 1001.0 / 24_000.0;
        assert!(
            (r.secondes() - attendu).abs() < 1e-9,
            "durée {}",
            r.secondes()
        );
    }

    #[test]
    fn tkhd_porte_la_taille_de_presentation_et_avc1_la_taille_codee() {
        // 1920×1088 codé (120×68 macroblocs, sans recadrage), 1920×1080 à l'affichage.
        let sps = sps_synthetique(120, 68, 0, None);
        let pps = vec![0x68u8, 0xCE, 0x3C, 0x80];
        let idr = vec![0x65u8, 1, 2, 3];
        let f = annexb(&[&sps, &pps, &idr]);
        let unites: Vec<&[u8]> = vec![&f];

        let opts = Options {
            cadence: Some((24_000, 1001)),
            affichage: Some((1920, 1080)),
        };
        let (mp4, r) = muxer_h264_avec(&unites, &opts).expect("muxage");
        assert_eq!(
            (r.sps.width, r.sps.height),
            (1920, 1088),
            "le SPS reste la taille codée"
        );

        // `tkhd` : largeur/hauteur 16.16 aux deux derniers champs de la boîte (version 0).
        let pos = mp4
            .windows(4)
            .position(|w| w == b"tkhd")
            .expect("tkhd présente");
        let fin = pos + 4 + 84; // 84 octets de charge en version 0 (ISO 14496-12 §8.3.2)
        let lire = |o: usize| u32::from_be_bytes([mp4[o], mp4[o + 1], mp4[o + 2], mp4[o + 3]]);
        assert_eq!(lire(fin - 8) >> 16, 1920);
        assert_eq!(
            lire(fin - 4) >> 16,
            1080,
            "tkhd doit annoncer 1080, pas 1088"
        );

        // `avc1` : largeur/hauteur sur 16 bits, 24 octets après le début de la charge. La
        // recherche part de `stsd` — la marque `avc1` figure AUSSI dans la liste de `ftyp`.
        let stsd = mp4
            .windows(4)
            .position(|w| w == b"stsd")
            .expect("stsd présente");
        let av = stsd
            + mp4[stsd..]
                .windows(4)
                .position(|w| w == b"avc1")
                .expect("avc1 présente");
        let w = u16::from_be_bytes([mp4[av + 4 + 24], mp4[av + 4 + 25]]);
        let h = u16::from_be_bytes([mp4[av + 4 + 26], mp4[av + 4 + 27]]);
        assert_eq!(
            (w, h),
            (1920, 1088),
            "avc1 garde la taille réellement codée"
        );

        // Sans indication, les deux coïncident.
        let (mp4, _) = muxer_h264(&unites, None).expect("muxage sans affichage");
        let pos = mp4.windows(4).position(|w| w == b"tkhd").expect("tkhd");
        let fin = pos + 4 + 84;
        let lire = |o: usize| u32::from_be_bytes([mp4[o], mp4[o + 1], mp4[o + 2], mp4[o + 3]]);
        assert_eq!(lire(fin - 4) >> 16, 1088);
    }

    #[test]
    fn un_flux_sans_sps_est_refuse() {
        let idr = vec![0x65u8, 1, 2, 3];
        let f = annexb(&[&idr]);
        let unites: Vec<&[u8]> = vec![&f];
        assert!(matches!(
            muxer_h264(&unites, None),
            Err(FormatError::Corrupt(_))
        ));
        assert!(matches!(
            muxer_h264(&[], None),
            Err(FormatError::Corrupt(_))
        ));
    }

    #[test]
    fn les_delimiteurs_et_le_bourrage_sortent_de_l_echantillon() {
        let idr = vec![0x65u8, 1, 2, 3];
        let filler = vec![0x0Cu8, 0xFF, 0xFF];
        let u = annexb(&[&[0x09u8, 0x10], &idr, &filler]);
        let e = en_avcc(&u);
        assert!(e.cle);
        assert_eq!(e.bytes.len(), 4 + idr.len(), "seule l'IDR est conservée");
    }
}
