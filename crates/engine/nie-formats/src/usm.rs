//! Démultiplexeur **USM / Sofdec2** (CRI Middleware) complet : métadonnées, images, pistes.
//!
//! ## Ce que ce module ajoute au démux historique
//!
//! [`crate::cri_audio::usm_demux`] concaténait toutes les images en un seul tampon et renvoyait
//! `width = height = frame_rate = 0` — les trois champs n'étaient jamais renseignés. Impossible,
//! donc, de remuxer correctement : ni dimensions, ni cadence, ni frontières d'images.
//!
//! Ici, les blocs d'en-tête (`block_type == 1`) sont lus : ils portent des tables `@UTF`
//! (`CRIUSF_DIR_STREAM`, `VIDEO_HDRINFO`, `AUDIO_HDRINFO`) qui déclarent le nom d'origine du
//! film, ses dimensions, sa cadence exacte en rationnel (`framerate_n / framerate_d`), son
//! nombre d'images et le format de chaque piste sonore. Chaque bloc de données `@SFV` reste
//! une **unité d'accès distincte**, ce qu'attend le muxeur MP4 ([`crate::mp4`]).
//!
//! ## Structure d'un bloc (vérifiée sur les 194 `.usm` du jeu)
//!
//! ```text
//! [0x00:0x04] stmid        : "CRID" | "@SFV" | "@SFA" | "@SBT" | "@ALP"
//! [0x04:0x08] data_size    : u32 BE — taille du bloc après ces 8 octets
//! [0x09]      data_offset  : u8 — début de la charge utile, depuis 0x08
//! [0x0A:0x0C] padding_size : u16 BE — bourrage en fin de charge
//! [0x0C]      channel_no   : u8 — numéro de piste (audio multipiste)
//! [0x0F]      block_type   : 0 = données, 1 = en-tête @UTF, 2 = fin de section, 3 = index
//! ```
//!
//! ## Les deux fichiers *loose* : chiffrés, et pas dans le même codec
//!
//! Deux des 194 fichiers (`IE_15th.usm`, `L5logo.usm`, les seuls posés hors CPK) ne commencent
//! pas par `CRID`. [`demuxer_nomme`] retente alors l'enveloppe CRI « position-based XOR »
//! clé-par-nom-de-fichier — **la même que celle des packs**, cf. [`crate::cpk`] — et ne conclut
//! au succès que si les octets déchiffrés commencent bien par `CRID`. Aucun octet fabriqué : si
//! l'hypothèse est fausse, l'erreur remonte. Vérifié sur le réel : le `CRIUSF_DIR_STREAM` ainsi
//! obtenu se relit intégralement (`filename = "IE_15th.usm"`, pistes `IE_15th_wide.avi` et un
//! `.wav` au nom japonais), ce qu'un déchiffrement approximatif ne produirait pas.
//!
//! Ces deux-là sont aussi les seuls en **MPEG-2** (`mpeg_codec = 1`), en 2640×1080 — un format
//! ultra-large, à 30 i/s. Les 95 autres sont du H.264 (`mpeg_codec = 5`). Cette distinction ne
//! se voit pas dans les octets : MPEG-2 et H.264 partagent le start-code `00 00 01`, cf.
//! [`CodecVideo`].

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::FormatError;
use crate::cpk::{UtfTable, is_utf, parse_utf};

// ── Identifiants de flux ──────────────────────────────────────────────────────

/// `CRID` — bloc d'en-tête du conteneur.
pub const STMID_CRID: [u8; 4] = *b"CRID";
/// `@SFV` — flux vidéo.
pub const STMID_SFV: [u8; 4] = *b"@SFV";
/// `@SFA` — flux audio.
pub const STMID_SFA: [u8; 4] = *b"@SFA";
/// `@SBT` — flux de sous-titres.
pub const STMID_SBT: [u8; 4] = *b"@SBT";
/// `@ALP` — flux de canal alpha (Sofdec2 « video with alpha »).
pub const STMID_ALP: [u8; 4] = *b"@ALP";

/// Codec vidéo porté par le flux `@SFV`.
///
/// **Le champ `mpeg_codec` de `VIDEO_HDRINFO` fait autorité**, pas le reniflage d'octets : le
/// MPEG-2 et le H.264 partagent le même start-code `00 00 01`, et une tranche MPEG-2 numéro 7
/// (`00 00 01 07`) se lit exactement comme une unité NAL de type 7 (SPS). C'est ce piège qui
/// faisait annoncer « H.264 1920×1080 » sur `IE_15th.usm`, qui est en réalité du MPEG-2 en
/// 2640×1080 — le SPS « lu » était une tranche, et ses dimensions (16×176) du bruit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CodecVideo {
    /// H.264 / AVC (`mpeg_codec = 5`) — le codec des 95 cinématiques du jeu.
    H264,
    /// MPEG-2 Video, Sofdec Prime (`mpeg_codec = 1`) — `IE_15th.usm` et `L5logo.usm`.
    Mpeg2,
    /// VP9 (`mpeg_codec = 9`).
    Vp9,
    /// Aucun flux vidéo, ou codec non identifié.
    Inconnu,
}

impl CodecVideo {
    /// Nom court pour l'affichage et le JSON.
    #[must_use]
    pub fn nom(self) -> &'static str {
        match self {
            Self::H264 => "h264",
            Self::Mpeg2 => "mpeg2",
            Self::Vp9 => "vp9",
            Self::Inconnu => "inconnu",
        }
    }

    /// Extension d'un flux élémentaire brut extrait tel quel.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::H264 => "h264",
            Self::Mpeg2 => "m2v",
            Self::Vp9 => "vp9",
            Self::Inconnu => "bin",
        }
    }

    /// Vrai si un navigateur sait décoder ce codec, et si le dépôt sait l'emballer pour lui.
    ///
    /// H.264 part en MP4 ([`crate::mp4`]), VP9 en WebM ([`crate::webm`]). Le MPEG-2 n'est décodé
    /// par aucun moteur web courant : un conteneur valide ne suffirait pas, personne ne lirait
    /// son contenu. Mieux vaut le dire que produire un `<video>` noir.
    #[must_use]
    pub fn lisible_par_navigateur(self) -> bool {
        matches!(self, Self::H264 | Self::Vp9)
    }

    /// Type MIME du conteneur web dans lequel ce codec s'emballe, s'il y en a un.
    #[must_use]
    pub fn type_mime_web(self) -> Option<&'static str> {
        match self {
            Self::H264 => Some("video/mp4"),
            Self::Vp9 => Some("video/webm"),
            _ => None,
        }
    }

    /// Traduit le champ `mpeg_codec` de `VIDEO_HDRINFO`.
    #[must_use]
    pub fn depuis_mpeg_codec(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::Mpeg2),
            5 => Some(Self::H264),
            9 => Some(Self::Vp9),
            _ => None,
        }
    }
}

/// Codec d'une piste sonore, déduit des octets (l'en-tête `audio_codec` n'est pas toujours là).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CodecAudio {
    /// HCA Criware (chiffré `ciph_type=56` sur IEVR).
    Hca,
    /// ADX Criware.
    Adx,
    /// Octets non reconnus.
    Inconnu,
}

impl CodecAudio {
    /// Nom court pour l'affichage et le JSON.
    #[must_use]
    pub fn nom(self) -> &'static str {
        match self {
            Self::Hca => "hca",
            Self::Adx => "adx",
            Self::Inconnu => "inconnu",
        }
    }
}

// ── Métadonnées ───────────────────────────────────────────────────────────────

/// En-tête vidéo (`VIDEO_HDRINFO`), tel que déclaré par l'encodeur Sofdec2.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnteteVideo {
    /// Largeur codée (multiple de 16).
    pub largeur: u32,
    /// Hauteur codée (multiple de 16).
    pub hauteur: u32,
    /// Largeur d'affichage (recadrée).
    pub largeur_affichee: u32,
    /// Hauteur d'affichage (recadrée).
    pub hauteur_affichee: u32,
    /// Numérateur de la cadence (`framerate_n`) — souvent 2997 pour 29,97 i/s.
    pub cadence_num: u32,
    /// Dénominateur de la cadence (`framerate_d`) — souvent 100.
    pub cadence_den: u32,
    /// Nombre total d'images annoncé.
    pub total_images: u32,
    /// `mpeg_codec` brut (identifiant interne CRI).
    pub mpeg_codec: u32,
    /// `alpha_type` : non nul quand le film porte un canal alpha.
    pub alpha: u32,
}

impl EnteteVideo {
    /// Cadence rationnelle `(num, den)` si elle est déclarée et cohérente.
    #[must_use]
    pub fn cadence(&self) -> Option<(u32, u32)> {
        (self.cadence_num > 0 && self.cadence_den > 0)
            .then_some((self.cadence_num, self.cadence_den))
    }

    /// Cadence en images par seconde.
    #[must_use]
    pub fn images_par_seconde(&self) -> Option<f64> {
        self.cadence().map(|(n, d)| f64::from(n) / f64::from(d))
    }
}

/// Une piste sonore extraite du conteneur.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PisteAudio {
    /// Numéro de canal (`chno`) — l'ordre des pistes dans le fichier.
    pub canal: u8,
    /// Codec détecté sur les octets.
    pub codec: CodecAudio,
    /// Fréquence d'échantillonnage déclarée (`sampling_rate`), `0` si absente.
    pub frequence: u32,
    /// Nombre de canaux déclaré (`num_channels`), `0` si absent.
    pub canaux: u32,
    /// Nombre total d'échantillons déclaré, `0` si absent.
    pub echantillons: u32,
    /// Taille du flux en octets — renseignée même quand les octets ne sont pas retenus.
    pub taille: u64,
    /// Octets bruts du flux, prêts pour `cri_audio::decode_to_wav`. Vide après [`inspecter`].
    #[cfg_attr(feature = "serde", serde(skip))]
    pub octets: Vec<u8>,
}

/// Piste vidéo emballée dans un conteneur lisible par un navigateur.
#[derive(Debug, Clone)]
pub struct ConteneurWeb {
    /// Type MIME à annoncer (`video/mp4` ou `video/webm`).
    pub mime: &'static str,
    /// Octets du conteneur.
    pub octets: Vec<u8>,
    /// Largeur codée, lue dans le bitstream.
    pub largeur: u32,
    /// Hauteur codée, lue dans le bitstream.
    pub hauteur: u32,
    /// Nombre d'images écrites.
    pub images: u32,
    /// Nombre d'images-clés — les points où le lecteur peut entrer.
    pub cles: u32,
    /// Durée totale, en secondes.
    pub secondes: f64,
}

/// Ce qu'on apprend d'un film **sans en garder une seule image**.
///
/// Le catalogue de l'explorateur n'a besoin que de ça : durée, définition, codec, pistes. Un
/// [`Usm`] complet retiendrait tout le film — jusqu'à 312 Mo, et 38 081 allocations pour les
/// seules images de `ev09_05300`. Mesuré : l'explorateur montait à 15 Go de mémoire de travail
/// en enrichissant son catalogue film par film. [`inspecter`] parcourt les mêmes blocs et ne
/// retient que des compteurs.
#[derive(Debug, Clone)]
pub struct Apercu {
    /// Nom d'origine du film, quand le conteneur le déclare.
    pub nom: Option<String>,
    /// En-tête vidéo déclaré.
    pub entete: EnteteVideo,
    /// Codec vidéo constaté.
    pub codec: CodecVideo,
    /// Nombre d'images.
    pub images: u32,
    /// Total des octets vidéo.
    pub octets_video: u64,
    /// Pistes sonores, sans leurs octets (`taille` reste renseignée).
    pub pistes: Vec<PisteAudio>,
    /// Vrai si les octets ont dû être déchiffrés par l'enveloppe CRI.
    pub dechiffre: bool,
}

impl Apercu {
    /// Durée en secondes d'après le nombre d'images et la cadence déclarée.
    #[must_use]
    pub fn duree(&self) -> Option<f64> {
        let ips = self.entete.images_par_seconde()?;
        (ips > 0.0).then(|| f64::from(self.images) / ips)
    }
}

/// Résultat complet d'un démultiplexage.
#[derive(Debug, Clone)]
pub struct Usm {
    /// Nom d'origine du film (`filename` du `CRIUSF_DIR_STREAM`), quand il est déclaré.
    pub nom: Option<String>,
    /// En-tête vidéo déclaré.
    pub entete: EnteteVideo,
    /// Codec vidéo constaté sur le bitstream.
    pub codec: CodecVideo,
    /// Une unité d'accès (image) par entrée, en Annex-B, dans l'ordre du fichier.
    pub images: Vec<Vec<u8>>,
    /// Pistes sonores, triées par canal.
    pub pistes: Vec<PisteAudio>,
    /// Blocs de sous-titres bruts (`@SBT`), s'il y en a.
    pub sous_titres: Vec<Vec<u8>>,
    /// Vrai si les octets ont dû être déchiffrés par l'enveloppe CRI.
    pub dechiffre: bool,
    /// Total des octets vidéo — renseigné même quand les images ne sont pas retenues.
    pub octets_video: u64,
    /// Tables `@UTF` des blocs d'en-tête, dans l'ordre du fichier — la source de vérité du
    /// conteneur, gardée telle quelle pour l'inspection (`niers video info --tables`).
    pub entetes: Vec<UtfTable>,
}

impl Usm {
    /// Cadence retenue : celle de l'en-tête, sinon `None` (le SPS tranchera).
    #[must_use]
    pub fn cadence(&self) -> Option<(u32, u32)> {
        self.entete.cadence()
    }

    /// Durée en secondes d'après le nombre d'images démultiplexées et la cadence.
    #[must_use]
    pub fn duree(&self) -> Option<f64> {
        let ips = self.entete.images_par_seconde()?;
        (ips > 0.0).then(|| self.images.len() as f64 / ips)
    }

    /// Flux vidéo élémentaire, images concaténées dans l'ordre — la forme qu'attend un lecteur
    /// externe (`.m2v` pour du MPEG-2, `.h264` pour de l'Annex-B).
    #[must_use]
    pub fn flux_brut(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.octets_video as usize);
        for img in &self.images {
            out.extend_from_slice(img);
        }
        out
    }

    /// Taille de présentation à écrire dans le conteneur, quand elle diffère de la taille codée.
    ///
    /// `disp_*` n'est repris que s'il est plus PETIT que la taille codée : c'est le cas exact des
    /// cinématiques 1920×1088 destinées à 1920×1080. Un `disp_*` égal ou plus grand n'apprend
    /// rien et vaut mieux être ignoré que recopié à l'aveugle.
    #[must_use]
    pub fn affichage(&self) -> Option<(u32, u32)> {
        let e = &self.entete;
        (e.largeur_affichee > 0
            && e.hauteur_affichee > 0
            && (e.largeur_affichee < e.largeur || e.hauteur_affichee < e.hauteur))
            .then_some((e.largeur_affichee, e.hauteur_affichee))
    }

    /// Emballe la piste vidéo dans le conteneur que le navigateur sait lire, **sans réencodage**.
    ///
    /// H.264 → MP4, VP9 → WebM.
    ///
    /// C'est la porte d'entrée des trois consommateurs — la CLI, le serveur de modèles et
    /// l'explorateur —, pour qu'aucun d'eux n'ait à savoir quel codec va dans quel conteneur,
    /// ni à muxer deux fois pour obtenir à la fois les octets et la mesure.
    ///
    /// # Erreurs
    ///
    /// [`FormatError::Corrupt`] si le codec n'a pas de conteneur web (MPEG-2), sinon les erreurs
    /// du muxeur concerné.
    pub fn en_conteneur_web(&self) -> Result<ConteneurWeb, FormatError> {
        match self.codec {
            CodecVideo::H264 => {
                let (octets, r) = self.en_mp4()?;
                Ok(ConteneurWeb {
                    mime: "video/mp4",
                    octets,
                    largeur: r.sps.width,
                    hauteur: r.sps.height,
                    images: r.images,
                    cles: r.cles,
                    secondes: r.secondes(),
                })
            }
            CodecVideo::Vp9 => {
                let (octets, r) = self.en_webm()?;
                Ok(ConteneurWeb {
                    mime: "video/webm",
                    octets,
                    largeur: r.largeur,
                    hauteur: r.hauteur,
                    images: r.images,
                    cles: r.cles,
                    secondes: r.secondes,
                })
            }
            _ => Err(FormatError::Corrupt(
                "USM : ce codec n'a pas de conteneur web",
            )),
        }
    }

    /// Muxe la piste vidéo VP9 en WebM, sans réencodage.
    ///
    /// # Erreurs
    ///
    /// [`FormatError::Corrupt`] si le codec n'est pas du VP9 ; sinon les erreurs de
    /// [`crate::webm::muxer_vp9`] (flux vide, aucune image-clé, octets non VP9).
    pub fn en_webm(&self) -> Result<(Vec<u8>, crate::webm::Resume), FormatError> {
        if self.codec != CodecVideo::Vp9 {
            return Err(FormatError::Corrupt("USM : muxage WebM réservé au VP9"));
        }
        let trames: Vec<&[u8]> = self.images.iter().map(Vec::as_slice).collect();
        crate::webm::muxer_vp9(&trames, self.cadence().unwrap_or((30, 1)), self.affichage())
    }

    /// Remuxe la piste vidéo en MP4 progressif, sans réencodage.
    ///
    /// # Erreurs
    ///
    /// Remonte les erreurs de [`crate::mp4::muxer_h264`] : flux vide, absence de SPS/PPS,
    /// SPS illisible. Renvoie [`FormatError::Corrupt`] si le codec n'est pas H.264 — un flux
    /// VP9 n'a rien à faire dans un `avc1`.
    pub fn en_mp4(&self) -> Result<(Vec<u8>, crate::mp4::Resume), FormatError> {
        if self.codec != CodecVideo::H264 {
            return Err(FormatError::Corrupt("USM : remux MP4 réservé au H.264"));
        }
        let unites: Vec<&[u8]> = self.images.iter().map(Vec::as_slice).collect();
        crate::mp4::muxer_h264_avec(
            &unites,
            &crate::mp4::Options {
                cadence: self.cadence(),
                affichage: self.affichage(),
            },
        )
    }
}

// ── Conventions de nommage des films ──────────────────────────────────────────
//
// Les noms de fichiers portent la seule classification que le jeu donne de ses cinématiques :
// le chapitre (`ev09_05300`), la nature (`NIE_Title`, `Chronicle_Title`) et la langue
// (`NIE_Title_fr_01`). Trois consommateurs en avaient chacun leur copie — la CLI, le serveur de
// modèles et l'explorateur — donc trois occasions de diverger. Elle vit ici, une fois.

/// Codes de langue employés par les noms de films, avec leur libellé en clair.
pub const LANGUES: [(&str, &str); 9] = [
    ("JP", "Japonais"),
    ("EN", "Anglais"),
    ("CN", "Chinois simplifié"),
    ("TW", "Chinois traditionnel"),
    ("fr", "Français"),
    ("de", "Allemand"),
    ("es", "Espagnol"),
    ("it", "Italien"),
    ("pt", "Portugais"),
];

/// Nom de fichier d'un chemin VFS — c'est lui qui sert de clé à l'enveloppe CRI.
#[must_use]
pub fn nom_fichier_de(chemin: &str) -> &str {
    chemin.rsplit('/').next().unwrap_or(chemin)
}

/// Radical d'un chemin VFS : `data/common/movie/ev01_00050.usm` → `ev01_00050`.
#[must_use]
pub fn radical_de(chemin: &str) -> &str {
    let nom = nom_fichier_de(chemin);
    nom.strip_suffix(".usm").unwrap_or(nom)
}

/// Rubrique d'affichage d'un film, déduite de son radical.
///
/// Les cinématiques d'histoire sont nommées `ev<chapitre>_<index>` : le chapitre est la seule
/// arborescence que le jeu leur donne, et c'est donc la seule qui ait un sens à l'écran.
#[must_use]
pub fn rubrique_de(radical: &str) -> alloc::string::String {
    use alloc::string::ToString;
    if radical.starts_with("NIE_Title") {
        return "Écrans-titres".to_string();
    }
    if radical.starts_with("Chronicle_Title") {
        return "Chronicle".to_string();
    }
    if let Some(reste) = radical.strip_prefix("ev")
        && let Some(chapitre) = reste.split('_').next()
        && chapitre.len() == 2
        && chapitre.bytes().all(|c| c.is_ascii_digit())
    {
        let mut s = "Chapitre ".to_string();
        s.push_str(chapitre);
        return s;
    }
    "Logos et intros".to_string()
}

/// Code de langue porté par un radical (`NIE_Title_fr_01` → `fr`), `None` s'il n'en porte pas.
#[must_use]
pub fn langue_de(radical: &str) -> Option<&'static str> {
    LANGUES.iter().find_map(|(code, _)| {
        // Recherche du motif `_<code>_` sans allouer : `contains` sur une `String` formatée
        // ferait une allocation par code et par film.
        radical
            .match_indices(code)
            .any(|(i, _)| {
                i > 0
                    && radical.as_bytes()[i - 1] == b'_'
                    && radical.as_bytes().get(i + code.len()) == Some(&b'_')
            })
            .then_some(*code)
    })
}

// ── Démultiplexage ────────────────────────────────────────────────────────────

/// Démultiplexe un USM déjà en clair.
///
/// # Erreurs
///
/// [`FormatError::TooShort`] si le tampon fait moins de 8 octets, [`FormatError::BadMagic`] si
/// le fichier ne commence pas par `CRID` (fichier chiffré : passer par [`demuxer_nomme`]).
pub fn demuxer(data: &[u8]) -> Result<Usm, FormatError> {
    if data.len() < 8 {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: 8,
        });
    }
    if data[..4] != STMID_CRID {
        return Err(FormatError::BadMagic { format: "USM/CRID" });
    }
    Ok(parcourir(data, false, true))
}

/// Démultiplexe un USM, en retentant l'enveloppe CRI si les octets ne sont pas en clair.
///
/// `nom_fichier` est le **nom de base** du fichier (`IE_15th.usm`), qui sert de clé.
///
/// # Erreurs
///
/// Comme [`demuxer`] ; le `BadMagic` n'est renvoyé que si le déchiffrement échoue AUSSI.
pub fn demuxer_nomme(data: &[u8], nom_fichier: &str) -> Result<Usm, FormatError> {
    match demuxer(data) {
        Ok(u) => Ok(u),
        Err(FormatError::BadMagic { .. }) => {
            let mut clair = data.to_vec();
            let cle = crate::cpk::key_from_filename(nom_fichier);
            crate::cpk::decrypt_block(&mut clair, 0, cle);
            if clair.len() >= 4 && clair[..4] == STMID_CRID {
                Ok(parcourir(&clair, true, true))
            } else {
                Err(FormatError::BadMagic {
                    format: "USM/CRID (même déchiffré)",
                })
            }
        }
        Err(e) => Err(e),
    }
}

/// Inspecte un USM **sans retenir ses images** : métadonnées, compteurs, tailles.
///
/// Même parcours de blocs que [`demuxer_nomme`], mais la mémoire reste constante quelle que
/// soit la taille du film. C'est ce qu'il faut pour remplir un catalogue : le lecteur, lui,
/// appellera [`demuxer_nomme`] au moment de jouer.
///
/// # Erreurs
///
/// Comme [`demuxer_nomme`].
pub fn inspecter(data: &[u8], nom_fichier: &str) -> Result<Apercu, FormatError> {
    let u = match demuxer_sans_images(data) {
        Ok(u) => u,
        Err(FormatError::BadMagic { .. }) => {
            let mut clair = data.to_vec();
            let cle = crate::cpk::key_from_filename(nom_fichier);
            crate::cpk::decrypt_block(&mut clair, 0, cle);
            if clair.len() >= 4 && clair[..4] == STMID_CRID {
                parcourir(&clair, true, false)
            } else {
                return Err(FormatError::BadMagic {
                    format: "USM/CRID (même déchiffré)",
                });
            }
        }
        Err(e) => return Err(e),
    };
    Ok(Apercu {
        nom: u.nom,
        entete: u.entete,
        codec: u.codec,
        images: u.images.len() as u32,
        octets_video: u.octets_video,
        pistes: u.pistes,
        dechiffre: u.dechiffre,
    })
}

/// Parcours sans rétention, sur des octets déjà en clair.
fn demuxer_sans_images(data: &[u8]) -> Result<Usm, FormatError> {
    if data.len() < 8 {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: 8,
        });
    }
    if data[..4] != STMID_CRID {
        return Err(FormatError::BadMagic { format: "USM/CRID" });
    }
    Ok(parcourir(data, false, false))
}

/// Parcourt les blocs et assemble le résultat. `data` commence forcément par `CRID`.
fn parcourir(data: &[u8], dechiffre: bool, retenir: bool) -> Usm {
    let mut octets_video = 0u64;
    let mut nom = None;
    let mut entete = EnteteVideo::default();
    let mut codec = CodecVideo::Inconnu;
    let mut images: Vec<Vec<u8>> = Vec::new();
    let mut sous_titres: Vec<Vec<u8>> = Vec::new();
    let mut entetes: Vec<UtfTable> = Vec::new();
    // Emballage IVF du flux VP9 (cf. [`IVF_MAGIC`]) : détecté au premier bloc de données.
    let mut ivf = false;
    let mut entete_ivf_lu = false;
    let mut tampon_ivf: Vec<u8> = Vec::new();
    // Pistes indexées par canal ; `Vec` plutôt que `HashMap` pour rester `alloc`-only.
    let mut pistes: Vec<PisteAudio> = Vec::new();

    let mut pos = 0usize;
    while pos + 0x20 <= data.len() {
        let mut stmid = [0u8; 4];
        stmid.copy_from_slice(&data[pos..pos + 4]);
        let data_size =
            u32::from_be_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;
        let bloc_total = 8 + data_size;
        if data_size < 0x18 || pos + bloc_total > data.len() {
            break;
        }

        let data_offset = data[pos + 0x09] as usize;
        let padding = u16::from_be_bytes([data[pos + 0x0A], data[pos + 0x0B]]) as usize;
        let canal = data[pos + 0x0C];
        let block_type = data[pos + 0x0F];

        let debut = pos + 8 + data_offset;
        let dispo = data_size
            .saturating_sub(data_offset)
            .saturating_sub(padding);
        if dispo > 0 && debut + dispo <= data.len() {
            let charge = &data[debut..debut + dispo];
            match block_type {
                // 0 = données utiles.
                0 => match stmid {
                    STMID_SFV | STMID_ALP => {
                        if codec == CodecVideo::Inconnu {
                            codec = deviner_codec(charge);
                        }
                        if ivf || charge.starts_with(&IVF_MAGIC) {
                            ivf = true;
                            tampon_ivf.extend_from_slice(charge);
                            extraire_ivf(
                                &mut tampon_ivf,
                                &mut entete_ivf_lu,
                                &mut images,
                                &mut octets_video,
                                retenir,
                            );
                        } else {
                            images.push(if retenir { charge.to_vec() } else { Vec::new() });
                            octets_video += charge.len() as u64;
                        }
                    }
                    STMID_SFA => {
                        let piste = piste_mut(&mut pistes, canal);
                        if piste.codec == CodecAudio::Inconnu {
                            piste.codec = deviner_codec_audio(charge);
                        }
                        piste.taille += charge.len() as u64;
                        if retenir {
                            piste.octets.extend_from_slice(charge);
                        }
                    }
                    STMID_SBT => sous_titres.push(charge.to_vec()),
                    _ => {}
                },
                // 1 = en-tête : une table @UTF décrivant le flux.
                1 => {
                    if let Some(t) = table(charge) {
                        appliquer_entete(&t, stmid, canal, &mut nom, &mut entete, &mut pistes);
                        entetes.push(t);
                    }
                }
                // 2 = fin de section, 3 = index de recherche : rien à en tirer ici.
                _ => {}
            }
        }

        pos += bloc_total;
    }

    pistes.sort_by_key(|p| p.canal);
    // L'en-tête a le dernier mot : le reniflage de start-code ne distingue pas MPEG-2 de H.264.
    if let Some(c) = CodecVideo::depuis_mpeg_codec(entete.mpeg_codec) {
        codec = c;
    }
    Usm {
        nom,
        entete,
        codec,
        images,
        pistes,
        sous_titres,
        dechiffre,
        octets_video,
        entetes,
    }
}

/// Magic d'un flux IVF (`DKIF`) — l'emballage des flux VP9 dans les blocs `@SFV`.
pub const IVF_MAGIC: [u8; 4] = *b"DKIF";

/// Extrait du tampon toutes les trames IVF complètes, et les ajoute à `images`.
///
/// **Ce que ça corrige.** Les deux films VP9 du jeu ne mettent pas des trames VP9 nues dans
/// leurs blocs `@SFV` : ils y mettent un flux **IVF**, c'est-à-dire un en-tête `DKIF` de 32
/// octets suivi, pour chaque image, d'un en-tête de 12 octets (taille `u32` LE + horodatage
/// `u64` LE). Servies telles quelles, ces trames commencent par leur taille et non par le
/// `frame_marker` VP9 — le muxeur WebM les refusait, à raison.
///
/// Le tampon permet à une image de traverser une frontière de bloc : rien ne garantit qu'un
/// bloc `@SFV` contienne exactement une image.
fn extraire_ivf(
    tampon: &mut Vec<u8>,
    entete_lu: &mut bool,
    images: &mut Vec<Vec<u8>>,
    octets_video: &mut u64,
    retenir: bool,
) {
    if !*entete_lu {
        if tampon.len() < 8 || tampon[..4] != IVF_MAGIC {
            return;
        }
        // Longueur d'en-tête déclarée (`u16` LE à l'offset 6) : 32 en pratique, mais la lire
        // coûte moins cher que de supposer.
        let n = u16::from_le_bytes([tampon[6], tampon[7]]) as usize;
        if n < 8 || tampon.len() < n {
            return;
        }
        tampon.drain(..n);
        *entete_lu = true;
    }
    while tampon.len() >= 12 {
        let taille = u32::from_le_bytes([tampon[0], tampon[1], tampon[2], tampon[3]]) as usize;
        if taille == 0 || tampon.len() < 12 + taille {
            break;
        }
        images.push(if retenir {
            tampon[12..12 + taille].to_vec()
        } else {
            Vec::new()
        });
        *octets_video += taille as u64;
        tampon.drain(..12 + taille);
    }
}

/// Parse la charge d'un bloc d'en-tête si c'est bien une table `@UTF`.
fn table(charge: &[u8]) -> Option<UtfTable> {
    if !is_utf(charge) {
        return None;
    }
    parse_utf(charge).ok()
}

/// Reporte les colonnes d'une table d'en-tête dans les structures de sortie.
fn appliquer_entete(
    t: &UtfTable,
    stmid: [u8; 4],
    canal: u8,
    nom: &mut Option<String>,
    entete: &mut EnteteVideo,
    pistes: &mut Vec<PisteAudio>,
) {
    let ent = |col: &str| -> Option<u32> { t.get_i64(0, col).and_then(|v| u32::try_from(v).ok()) };

    if stmid == STMID_CRID {
        // `CRIUSF_DIR_STREAM` : la ligne 0 décrit le conteneur, les suivantes chaque flux.
        if nom.is_none()
            && let Some(f) = t.get_str(0, "filename")
            && !f.is_empty()
        {
            *nom = Some(f.to_string());
        }
        return;
    }

    if stmid == STMID_SFV || stmid == STMID_ALP {
        if let Some(v) = ent("width") {
            entete.largeur = v;
        }
        if let Some(v) = ent("height") {
            entete.hauteur = v;
        }
        entete.largeur_affichee = ent("disp_width").unwrap_or(entete.largeur);
        entete.hauteur_affichee = ent("disp_height").unwrap_or(entete.hauteur);
        if let Some(v) = ent("framerate_n") {
            entete.cadence_num = v;
        }
        if let Some(v) = ent("framerate_d") {
            entete.cadence_den = v;
        }
        if let Some(v) = ent("total_frames") {
            entete.total_images = v;
        }
        if let Some(v) = ent("mpeg_codec") {
            entete.mpeg_codec = v;
        }
        if let Some(v) = ent("alpha_type") {
            entete.alpha = v;
        }
        return;
    }

    if stmid == STMID_SFA {
        let p = piste_mut(pistes, canal);
        if let Some(v) = ent("sampling_rate") {
            p.frequence = v;
        }
        if let Some(v) = ent("num_channels") {
            p.canaux = v;
        }
        if let Some(v) = ent("total_samples") {
            p.echantillons = v;
        }
    }
}

/// Retourne la piste du canal donné, en la créant au besoin.
fn piste_mut(pistes: &mut Vec<PisteAudio>, canal: u8) -> &mut PisteAudio {
    if let Some(i) = pistes.iter().position(|p| p.canal == canal) {
        return &mut pistes[i];
    }
    pistes.push(PisteAudio {
        canal,
        codec: CodecAudio::Inconnu,
        frequence: 0,
        canaux: 0,
        echantillons: 0,
        taille: 0,
        octets: Vec::new(),
    });
    let dernier = pistes.len() - 1;
    &mut pistes[dernier]
}

/// Reniflage de repli, quand `VIDEO_HDRINFO` ne déclare pas de `mpeg_codec` connu.
///
/// Ne sait PAS distinguer MPEG-2 de H.264 (start-code commun) : c'est
/// [`CodecVideo::depuis_mpeg_codec`] qui tranche, appelé après le parcours.
fn deviner_codec(charge: &[u8]) -> CodecVideo {
    if charge.len() < 5 {
        return CodecVideo::Inconnu;
    }
    if charge.starts_with(&[0, 0, 0, 1]) || charge.starts_with(&[0, 0, 1]) {
        CodecVideo::H264
    } else {
        CodecVideo::Vp9
    }
}

/// Reconnaît le codec audio au premier bloc de données.
fn deviner_codec_audio(charge: &[u8]) -> CodecAudio {
    if crate::cri_audio::is_hca(charge) {
        CodecAudio::Hca
    } else if crate::cri_audio::is_adx(charge) {
        CodecAudio::Adx
    } else {
        CodecAudio::Inconnu
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    /// Fabrique un bloc USM minimal : en-tête de 0x20 octets + charge.
    fn bloc(stmid: &[u8; 4], block_type: u8, canal: u8, charge: &[u8]) -> Vec<u8> {
        let data_offset = 0x18usize; // charge à 0x20 depuis le début du bloc
        let data_size = data_offset + charge.len();
        let mut b = Vec::with_capacity(8 + data_size);
        b.extend_from_slice(stmid);
        b.extend_from_slice(&(data_size as u32).to_be_bytes());
        b.push(0); // 0x08
        b.push(data_offset as u8); // 0x09
        b.extend_from_slice(&0u16.to_be_bytes()); // 0x0A padding
        b.push(canal); // 0x0C
        b.push(0); // 0x0D
        b.push(0); // 0x0E
        b.push(block_type); // 0x0F
        b.extend_from_slice(&[0u8; 0x10]); // 0x10..0x20
        b.extend_from_slice(charge);
        b
    }

    #[test]
    fn les_rubriques_suivent_les_conventions_de_nom() {
        assert_eq!(rubrique_de("ev01_00050"), "Chapitre 01");
        assert_eq!(rubrique_de("ev26_07102"), "Chapitre 26");
        assert_eq!(rubrique_de("NIE_Title_fr_01"), "Écrans-titres");
        assert_eq!(rubrique_de("Chronicle_Title_JP_01"), "Chronicle");
        assert_eq!(rubrique_de("IE_15th"), "Logos et intros");
        assert_eq!(rubrique_de("L5logo"), "Logos et intros");
        // `ev` suivi d'autre chose que deux chiffres n'est pas un chapitre.
        assert_eq!(rubrique_de("event_x"), "Logos et intros");
        assert_eq!(rubrique_de("ev1_00050"), "Logos et intros");
    }

    #[test]
    fn la_langue_se_lit_entre_deux_soulignes() {
        assert_eq!(langue_de("NIE_Title_fr_01"), Some("fr"));
        assert_eq!(langue_de("Chronicle_Title_TW_01"), Some("TW"));
        assert_eq!(langue_de("ev01_00050"), None);
        assert_eq!(langue_de("IE_15th"), None);
        // Le code doit être DÉLIMITÉ : `_EN` en fin de nom, ou `xENy`, n'en sont pas.
        assert_eq!(langue_de("un_film_EN"), None);
        assert_eq!(langue_de("SCENE_01"), None);
    }

    #[test]
    fn le_radical_et_le_nom_de_fichier_se_derivent_du_chemin() {
        assert_eq!(radical_de("data/common/movie/ev01_00050.usm"), "ev01_00050");
        assert_eq!(
            nom_fichier_de("data/common/movie/ev01_00050.usm"),
            "ev01_00050.usm"
        );
        assert_eq!(radical_de("L5logo.usm"), "L5logo");
        // Un chemin sans extension reste entier — on ne coupe pas ce qu'on ne reconnaît pas.
        assert_eq!(radical_de("dossier/fichier"), "fichier");
    }

    #[test]
    fn un_fichier_qui_ne_commence_pas_par_crid_est_refuse() {
        assert!(matches!(
            demuxer(b"XXXX0000"),
            Err(FormatError::BadMagic { .. })
        ));
        assert!(matches!(demuxer(b"CRI"), Err(FormatError::TooShort { .. })));
    }

    #[test]
    fn chaque_bloc_video_devient_une_image_distincte() {
        let mut f = bloc(&STMID_CRID, 1, 0, b"pas une table");
        f.extend(bloc(&STMID_SFV, 0, 0, &[0, 0, 0, 1, 0x67, 0xAA]));
        f.extend(bloc(&STMID_SFV, 0, 0, &[0, 0, 0, 1, 0x41, 0xBB]));
        f.extend(bloc(&STMID_SFA, 0, 0, b"HCA\0abcd"));
        f.extend(bloc(&STMID_SFA, 0, 0, b"efgh"));
        f.extend(bloc(&STMID_SFA, 0, 1, b"\x80\x00zzzz"));
        f.extend(bloc(&STMID_SFV, 2, 0, b""));

        let u = demuxer(&f).expect("démux");
        assert_eq!(u.images.len(), 2, "deux blocs vidéo = deux unités d'accès");
        assert_eq!(u.images[0], vec![0, 0, 0, 1, 0x67, 0xAA]);
        assert_eq!(u.codec, CodecVideo::H264);
        assert!(!u.dechiffre);

        assert_eq!(u.pistes.len(), 2, "deux canaux audio");
        assert_eq!(u.pistes[0].canal, 0);
        assert_eq!(u.pistes[0].codec, CodecAudio::Hca);
        assert_eq!(
            u.pistes[0].octets, b"HCA\0abcdefgh",
            "les blocs d'un canal se concatènent"
        );
        assert_eq!(u.pistes[1].codec, CodecAudio::Adx);
    }

    #[test]
    fn le_bourrage_de_fin_de_bloc_est_retire() {
        // Bloc bâti à la main avec padding_size = 3.
        let charge = [0u8, 0, 0, 1, 0x65, 0x11, 0xFF, 0xFF, 0xFF];
        let data_offset = 0x18usize;
        let data_size = data_offset + charge.len();
        let mut b = Vec::new();
        b.extend_from_slice(&STMID_SFV);
        b.extend_from_slice(&(data_size as u32).to_be_bytes());
        b.push(0);
        b.push(data_offset as u8);
        b.extend_from_slice(&3u16.to_be_bytes()); // padding
        b.extend_from_slice(&[0, 0, 0, 0]); // canal + type 0
        b.extend_from_slice(&[0u8; 0x10]);
        b.extend_from_slice(&charge);

        let mut f = bloc(&STMID_CRID, 1, 0, b"");
        f.extend(b);
        let u = demuxer(&f).expect("démux");
        assert_eq!(
            u.images[0],
            vec![0, 0, 0, 1, 0x65, 0x11],
            "les 3 octets de bourrage sautent"
        );
    }

    #[test]
    fn un_bloc_tronque_arrete_le_parcours_sans_paniquer() {
        let mut f = bloc(&STMID_CRID, 1, 0, b"");
        f.extend(bloc(&STMID_SFV, 0, 0, &[0, 0, 0, 1, 0x65]));
        // Bloc annoncé plus gros que ce qui reste.
        f.extend_from_slice(&STMID_SFV);
        f.extend_from_slice(&0xFFFF_u32.to_be_bytes());
        f.extend_from_slice(&[0u8; 0x18]);
        let u = demuxer(&f).expect("démux");
        assert_eq!(u.images.len(), 1);
    }

    #[test]
    fn l_enveloppe_cri_est_retentee_sur_un_fichier_chiffre() {
        let mut clair = bloc(&STMID_CRID, 1, 0, b"");
        clair.extend(bloc(&STMID_SFV, 0, 0, &[0, 0, 0, 1, 0x67, 0x42]));

        let nom = "film_de_test.usm";
        let mut chiffre = clair.clone();
        crate::cpk::decrypt_block(&mut chiffre, 0, crate::cpk::key_from_filename(nom));
        assert_ne!(
            &chiffre[..4],
            &STMID_CRID,
            "les octets chiffrés ne portent plus le magic"
        );

        // Sans le nom : refus net, aucun octet fabriqué.
        assert!(demuxer(&chiffre).is_err());
        // Avec le bon nom : le XOR est involutif, on retombe sur le clair.
        let u = demuxer_nomme(&chiffre, nom).expect("déchiffrement");
        assert!(u.dechiffre);
        assert_eq!(u.images[0], vec![0, 0, 0, 1, 0x67, 0x42]);
        // Avec un mauvais nom : refus, pas de démux hasardeux.
        assert!(demuxer_nomme(&chiffre, "autre.usm").is_err());
    }

    #[test]
    fn la_cadence_et_la_duree_viennent_de_l_entete() {
        let e = EnteteVideo {
            cadence_num: 2997,
            cadence_den: 100,
            ..EnteteVideo::default()
        };
        assert_eq!(e.cadence(), Some((2997, 100)));
        let ips = e.images_par_seconde().expect("cadence");
        assert!((ips - 29.97).abs() < 1e-9);

        let u = Usm {
            nom: None,
            entete: e,
            codec: CodecVideo::H264,
            images: alloc::vec![Vec::new(); 2997],
            pistes: Vec::new(),
            sous_titres: Vec::new(),
            dechiffre: false,
            octets_video: 0,
            entetes: Vec::new(),
        };
        let d = u.duree().expect("durée");
        assert!(
            (d - 100.0).abs() < 1e-9,
            "2997 images à 29,97 i/s = 100 s, obtenu {d}"
        );
    }

    #[test]
    fn l_entete_impose_le_codec_contre_le_reniflage() {
        // `00 00 01 07` : une tranche MPEG-2 n° 7, que le reniflage prend pour un SPS H.264.
        let charge = [0u8, 0, 1, 0x07, 0xAA, 0xBB];
        assert_eq!(
            deviner_codec(&charge),
            CodecVideo::H264,
            "le reniflage seul se trompe"
        );

        let mut f = bloc(&STMID_CRID, 1, 0, b"");
        f.extend(bloc(&STMID_SFV, 0, 0, &charge));
        let u = demuxer(&f).expect("démux");
        assert_eq!(
            u.codec,
            CodecVideo::H264,
            "sans en-tête, le reniflage reste seul juge"
        );

        // La correspondance `mpeg_codec` est ce qui redresse le verdict.
        assert_eq!(CodecVideo::depuis_mpeg_codec(1), Some(CodecVideo::Mpeg2));
        assert_eq!(CodecVideo::depuis_mpeg_codec(5), Some(CodecVideo::H264));
        assert_eq!(CodecVideo::depuis_mpeg_codec(9), Some(CodecVideo::Vp9));
        assert_eq!(CodecVideo::depuis_mpeg_codec(42), None);
        assert!(!CodecVideo::Mpeg2.lisible_par_navigateur());
        assert!(CodecVideo::H264.lisible_par_navigateur());
        assert_eq!(CodecVideo::Mpeg2.extension(), "m2v");
    }

    #[test]
    fn l_apercu_voit_la_meme_chose_sans_rien_retenir() {
        let mut f = bloc(&STMID_CRID, 1, 0, b"");
        f.extend(bloc(&STMID_SFV, 0, 0, &[0, 0, 0, 1, 0x67, 0xAA]));
        f.extend(bloc(&STMID_SFV, 0, 0, &[0, 0, 0, 1, 0x41, 0xBB, 0xCC]));
        f.extend(bloc(&STMID_SFA, 0, 0, b"HCA\0abcd"));

        let complet = demuxer(&f).expect("démux complet");
        let apercu = inspecter(&f, "x.usm").expect("aperçu");

        assert_eq!(apercu.images, complet.images.len() as u32);
        assert_eq!(apercu.octets_video, complet.octets_video);
        assert_eq!(apercu.codec, complet.codec);
        assert_eq!(apercu.entete, complet.entete);
        assert_eq!(apercu.pistes.len(), complet.pistes.len());
        assert_eq!(apercu.pistes[0].codec, complet.pistes[0].codec);
        assert_eq!(
            apercu.pistes[0].taille,
            complet.pistes[0].octets.len() as u64
        );

        // Et surtout : l'aperçu ne garde AUCUN octet, c'est toute sa raison d'être.
        assert!(
            apercu.pistes[0].octets.is_empty(),
            "l'aperçu ne retient pas l'audio"
        );
        assert_eq!(apercu.octets_video, 13, "6 + 7 octets de charge vidéo");
    }

    #[test]
    fn le_flux_brut_concatene_les_images_dans_l_ordre() {
        let mut f = bloc(&STMID_CRID, 1, 0, b"");
        f.extend(bloc(&STMID_SFV, 0, 0, &[0, 0, 1, 0xB3, 1]));
        f.extend(bloc(&STMID_SFV, 0, 0, &[0, 0, 1, 0x00, 2]));
        let u = demuxer(&f).expect("démux");
        assert_eq!(u.flux_brut(), vec![0, 0, 1, 0xB3, 1, 0, 0, 1, 0x00, 2]);
        assert_eq!(u.octets_video, 10);
    }

    #[test]
    fn un_flux_non_h264_refuse_le_remux_mp4() {
        let u = Usm {
            nom: None,
            entete: EnteteVideo::default(),
            codec: CodecVideo::Vp9,
            images: alloc::vec![alloc::vec![1, 2, 3]],
            pistes: Vec::new(),
            sous_titres: Vec::new(),
            dechiffre: false,
            octets_video: 0,
            entetes: Vec::new(),
        };
        assert!(matches!(u.en_mp4(), Err(FormatError::Corrupt(_))));
    }
}
