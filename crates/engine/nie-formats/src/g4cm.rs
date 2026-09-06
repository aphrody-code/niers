//! Codec **G4CM** — animations de caméra de cutscene (`common/event/<ev>/<ev>_camera.g4cm`).
//!
//! 1 215 fichiers dans le VFS. Ce module les **décode entièrement** (structure) et les
//! **ré-encode byte-exact**, ce qui en fait une base d'édition sûre : décoder → modifier →
//! encoder rend un fichier identique à l'octet près si rien n'a été touché.
//!
//! Le codec vivait dans `nie-camera` ; il est ici pour que `nie_formats::decode` — donc la FFI
//! (`nie_decode_json`), `niers decode`, l'explorateur et le MCP — l'atteignent. `nie-camera` le
//! réexporte, il n'y a toujours qu'**une** implémentation.
//!
//! ## Structure (reversée, validée sur 150 fichiers réels + confirmée par le code machine)
//!
//! ```text
//!   0x00  en-tete Level-5 commun (cf. [`crate::level5`])
//!         0x00 magic 'G4CM' · 0x04 header_size (0x40) · 0x06 VERSION (0x68)
//!         0x08 endian · 0x0A align (16) · 0x0C data_size
//!   0x20  13 x u16 : compteurs et offsets de sections
//!   0x40  clips     : nobj x 16 octets  {start_frame, end_frame, index, flags, ...}
//!         params    : 12 x 4 octets (7 f32 + hash u32 + f32 + reserve)
//!         noms      : nobj x 6 octets, ASCII zero-termine ("c0010", "c0100", ...)
//!   ...   objets    : nobj x 8 octets {u16, u16 premier_canal, u32 nb_canaux}
//!   ...   canaux    : total x 20 octets (cf. `Channel`)
//!   ...   temps     : u16[] partages, indexes par `time_index`
//!   ...   valeurs   : flux contigu, decoupe par `value_offset` / `count` / `elem_size`
//! ```
//!
//! ### Offsets de sections — formule confirmée par le désassemblage
//!
//! Le loader générique des conteneurs G4 (`nie.exe` @ `0x140506630`) calcule les adresses de
//! section en **dwords** :
//!
//! ```text
//!   section(i) = fichier + ((compteur[i] << shift) + align) * 4     avec shift = compteur[11]
//! ```
//!
//! Le même code établit deux faits utilisés ici : `compteur[0]` = nombre d'objets
//! (`movzx r11d, word [rbx+0x20]`) et **les canaux font 0x14 = 20 octets**
//! (`lea rax, [rax+0x14]` dans la boucle de fixup @ `0x1405067B0`). `compteur[2]` adresse la
//! table d'objets et `compteur[3]` la table de canaux — vérifié sur tous les fichiers testés.
//!
//! ### Ce qui n'est pas résolu — et qui n'est donc pas inventé
//!
//! Les flux de keyframes sur **2 octets** ([`Track::Raw16`]) ne se décodent ni en `f16` ni en
//! `i16` de façon plausible (valeurs incohérentes avec les canaux `f32` voisins du même
//! fichier). Leur encodage exact reste **inconnu** : ils sont exposés bruts. Les flux `f32`
//! ([`Track::F32`]) sont, eux, décodés (coordonnées monde, ordre de grandeur ±50 conforme aux
//! scènes) et les flux 1 octet sont exposés bruts ([`Track::Raw8`]).

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::FormatError;
use crate::level5::{self, Level5Header};

/// Raccourci local : toutes les fonctions du module rendent une [`FormatError`].
type Result<T> = core::result::Result<T, FormatError>;

/// Magic « G4CM » en little-endian.
pub const MAGIC: u32 = 0x4D43_3447;
/// Version de conteneur produite par le jeu (champ `0x06` de l'en-tête).
pub const VERSION: u16 = 0x0068;
/// Nombre de compteurs `u16` lus à l'offset `0x20`.
pub const COUNTER_COUNT: usize = 13;
/// Taille d'une entrée de la table de canaux, confirmée par `lea rax,[rax+0x14]` @ `0x1405067B0`.
pub const CHANNEL_ENTRY_LEN: usize = 20;
/// Taille d'une entrée de la table de clips.
pub const CLIP_ENTRY_LEN: usize = 16;
/// Taille d'une entrée de la table d'objets.
pub const OBJECT_ENTRY_LEN: usize = 8;
/// Longueur d'un nom d'objet dans la table de noms.
pub const NAME_LEN: usize = 6;

/// Rôle d'un canal, identifié par son code `kind`.
///
/// Les huit codes ci-dessous sont les **seuls** rencontrés : sur les 150 fichiers de contrôle,
/// chaque objet porte exactement ces 8 canaux (602 objets × 8 = 4 816 canaux, sans exception).
/// L'affectation aux axes s'appuie sur les canaux décodables (`f32`) : seuls `PosX`/`PosZ` et
/// `RefX`/`RefZ` apparaissent en `f32`, avec des valeurs de l'ordre de ±50 (coordonnées monde
/// d'une scène) ; `PosY`/`RefY`, `Fov` et `Roll` ne sont jamais stockés en `f32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChannelKind {
    /// `0x16` — position X.
    PosX,
    /// `0x17` — position Y.
    PosY,
    /// `0x18` — position Z.
    PosZ,
    /// `0x1A` — point visé X.
    RefX,
    /// `0x1B` — point visé Y.
    RefY,
    /// `0x1C` — point visé Z.
    RefZ,
    /// `0x1E` — champ de vision.
    Fov,
    /// `0x1F` — roulis.
    Roll,
    /// Code non répertorié (aucun rencontré sur l'échantillon de contrôle).
    Other(u8),
}

impl ChannelKind {
    /// Décode un code brut.
    #[must_use]
    pub const fn from_code(code: u8) -> ChannelKind {
        match code {
            0x16 => ChannelKind::PosX,
            0x17 => ChannelKind::PosY,
            0x18 => ChannelKind::PosZ,
            0x1A => ChannelKind::RefX,
            0x1B => ChannelKind::RefY,
            0x1C => ChannelKind::RefZ,
            0x1E => ChannelKind::Fov,
            0x1F => ChannelKind::Roll,
            other => ChannelKind::Other(other),
        }
    }

    /// Code brut correspondant.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            ChannelKind::PosX => 0x16,
            ChannelKind::PosY => 0x17,
            ChannelKind::PosZ => 0x18,
            ChannelKind::RefX => 0x1A,
            ChannelKind::RefY => 0x1B,
            ChannelKind::RefZ => 0x1C,
            ChannelKind::Fov => 0x1E,
            ChannelKind::Roll => 0x1F,
            ChannelKind::Other(c) => c,
        }
    }

    /// Nom court lisible.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ChannelKind::PosX => "posX",
            ChannelKind::PosY => "posY",
            ChannelKind::PosZ => "posZ",
            ChannelKind::RefX => "refX",
            ChannelKind::RefY => "refY",
            ChannelKind::RefZ => "refZ",
            ChannelKind::Fov => "fov",
            ChannelKind::Roll => "roll",
            ChannelKind::Other(_) => "?",
        }
    }
}

/// Les échantillons d'un canal, dans leur encodage d'origine.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Track {
    /// Flux `f32` — **décodé** : coordonnées monde.
    F32(Vec<f32>),
    /// Flux 2 octets — encodage **non résolu**, exposé brut (little-endian).
    ///
    /// Ni `f16` ni `i16` ne produisent de valeurs cohérentes avec les canaux `f32` du même
    /// fichier ; on ne devine pas. Le ré-encodage les restitue à l'identique.
    Raw16(Vec<u16>),
    /// Flux 1 octet — exposé brut.
    Raw8(Vec<u8>),
}

impl Track {
    /// Nombre d'échantillons.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Track::F32(v) => v.len(),
            Track::Raw16(v) => v.len(),
            Track::Raw8(v) => v.len(),
        }
    }

    /// `true` si le canal ne porte aucun échantillon.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Taille d'un échantillon en octets (1, 2 ou 4).
    #[must_use]
    pub const fn elem_size(&self) -> usize {
        match self {
            Track::F32(_) => 4,
            Track::Raw16(_) => 2,
            Track::Raw8(_) => 1,
        }
    }

    /// Les valeurs si le flux est décodable (`f32`), sinon `None`.
    #[must_use]
    pub fn values(&self) -> Option<&[f32]> {
        match self {
            Track::F32(v) => Some(v),
            _ => None,
        }
    }
}

/// Un canal d'animation : 20 octets d'en-tête + son flux d'échantillons.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Channel {
    /// Rôle du canal (`kind`, octet 0).
    pub kind: ChannelKind,
    /// Octet 1 — mode d'interpolation présumé (valeurs observées : 1, 2, 3). Non interprété.
    pub mode: u8,
    /// Octets 2 et 4 — nombre de composantes (toujours 1 sur l'échantillon de contrôle).
    pub components: (u8, u8),
    /// Octets 3 et 5 — taille d'échantillon déclarée, toujours égale des deux côtés.
    pub declared_size: (u8, u8),
    /// Octets 6-7 — index du canal dans l'objet.
    pub index: u16,
    /// Index du premier temps dans la table de temps partagée.
    pub time_index: u32,
    /// Offset du flux, en octets, depuis le début de la section « valeurs ».
    pub value_offset: u32,
    /// Échantillons.
    pub track: Track,
}

impl Channel {
    /// Temps (numéros de frame) associés à ce canal.
    #[must_use]
    pub fn times<'a>(&self, anim: &'a CameraAnim) -> &'a [u16] {
        let a = self.time_index as usize;
        let b = a.saturating_add(self.track.len()).min(anim.times.len());
        anim.times.get(a..b).unwrap_or(&[])
    }
}

/// Un clip : intervalle de frames déclaré en tête de fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Clip {
    /// Première frame.
    pub start: u16,
    /// Dernière frame.
    pub end: u16,
    /// Index du clip.
    pub index: u16,
    /// Drapeaux (valeur observée : 1).
    pub flags: u16,
    /// Les 8 octets restants de l'entrée, conservés pour le ré-encodage byte-exact.
    pub tail: [u8; 8],
}

/// Un objet animé du fichier (une caméra), avec ses canaux.
///
/// Le **nom** ne fait pas partie de cette entrée : il vit dans une table séparée, qui peut
/// compter plus d'entrées que la table d'objets (`ev63_00420_camera.g4cm` déclare 7 noms pour
/// 6 objets). Utiliser [`CameraAnim::name_of`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AnimObject {
    /// Premier champ de l'entrée de table (u16, toujours 0 sur l'échantillon).
    pub field0: u16,
    /// Index du premier canal de cet objet dans [`CameraAnim::channels`].
    pub first_channel: u16,
    /// Nombre de canaux.
    pub channel_count: u32,
}

/// Une animation caméra `.g4cm` décodée.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CameraAnim {
    /// En-tête commun Level-5.
    pub header: Level5Header,
    /// Les 13 compteurs/offsets bruts de `0x20`.
    pub counters: [u16; COUNTER_COUNT],
    /// Clips.
    pub clips: Vec<Clip>,
    /// Bloc de paramètres suivant les clips, **de taille variable** : il s'étend jusqu'à la
    /// table de noms, dont l'offset est donné par le compteur 10. Contient des `f32` de réglage
    /// et deux mots qui ressemblent à des hashes ; sa structure interne n'est pas établie, il est
    /// donc conservé tel quel.
    pub params: Vec<u8>,

    /// Noms déclarés dans la table de noms (un par clip).
    pub names: Vec<String>,
    /// Objets animés (peut être plus court que [`Self::names`]).
    pub objects: Vec<AnimObject>,
    /// Canaux, dans l'ordre du fichier.
    pub channels: Vec<Channel>,
    /// Table de temps partagée (numéros de frame).
    pub times: Vec<u16>,
    /// Octets entre la fin des noms et la table d'objets — préservés tels quels.
    pub gap_names_objects: Vec<u8>,
    /// Octets entre la fin de la table d'objets et la table de canaux — préservés tels quels.
    pub gap_objects_channels: Vec<u8>,
    /// Octets entre la fin de la table de canaux et la table de temps — préservés tels quels.
    pub gap_channels_times: Vec<u8>,
    /// Octets entre la fin de la table de temps et la section valeurs — préservés tels quels.
    pub gap_times_values: Vec<u8>,
    /// Octets après la dernière valeur (padding de fin) — préservés tels quels.
    pub trailer: Vec<u8>,
}

fn u16_at(d: &[u8], at: usize) -> Result<u16> {
    d.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(FormatError::TooShort {
            got: d.len(),
            need: at + 2,
        })
}

fn u32_at(d: &[u8], at: usize) -> Result<u32> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(FormatError::TooShort {
            got: d.len(),
            need: at + 4,
        })
}

/// `true` si le tampon commence par le magic « G4CM ».
#[must_use]
pub fn is_g4cm(data: &[u8]) -> bool {
    data.len() >= 4 && u32::from_le_bytes([data[0], data[1], data[2], data[3]]) == MAGIC
}

/// Alias de [`decode`], au nom des autres parseurs du crate (`g4mt::parse`, `navm::parse`…).
///
/// # Errors
/// Les mêmes que [`decode`].
pub fn parse(data: &[u8]) -> Result<CameraAnim> {
    decode(data)
}

/// Décode un `.g4cm`.
///
/// # Errors
/// [`FormatError::BadMagic`] si le magic diffère, [`FormatError::Malformed`] si le conteneur
/// n'est pas en version [`VERSION`] ou si une section ne retombe pas juste,
/// [`FormatError::TooShort`] si elle sort du tampon.
pub fn decode(data: &[u8]) -> Result<CameraAnim> {
    let header = level5::parse_header(data, MAGIC, "G4CM").map_err(|e| match e {
        FormatError::BadMagic { .. } => FormatError::BadMagic { format: "G4CM" },
        other => other,
    })?;
    if header.type_id != VERSION {
        return Err(FormatError::Malformed(format!(
            "G4CM: version de conteneur {:#06X} non gérée (attendu {VERSION:#06X})",
            header.type_id
        )));
    }
    let hs = header.header_size as usize;
    let align = header.align as usize;

    let mut counters = [0u16; COUNTER_COUNT];
    for (i, c) in counters.iter_mut().enumerate() {
        *c = u16_at(data, 0x20 + i * 2)?;
    }
    let nobj = counters[0] as usize;
    let shift = u32::from(counters[11]);

    // section(i) = ((counters[i] << shift) + align) * 4   — cf. doc du module.
    let section = |i: usize| -> usize { ((usize::from(counters[i]) << shift) + align) * 4 };
    let o_objects = section(2);
    let o_channels = section(3);
    if o_objects < hs || o_channels < o_objects || o_channels > data.len() {
        return Err(FormatError::Malformed(format!(
            "offsets de section incohérents : objets=0x{o_objects:X} canaux=0x{o_channels:X} \
             (fichier {} octets)",
            data.len()
        )));
    }

    // Clips + params + noms.
    let mut clips = Vec::with_capacity(nobj);
    for i in 0..nobj {
        let at = hs + i * CLIP_ENTRY_LEN;
        let mut tail = [0u8; 8];
        tail.copy_from_slice(data.get(at + 8..at + 16).ok_or(FormatError::TooShort {
            got: data.len(),
            need: at + 16,
        })?);
        clips.push(Clip {
            start: u16_at(data, at)?,
            end: u16_at(data, at + 2)?,
            index: u16_at(data, at + 4)?,
            flags: u16_at(data, at + 6)?,
            tail,
        });
    }
    // Le bloc de paramètres court des clips jusqu'à la table de noms, dont l'offset est donné
    // par le compteur 10 (vérifié : ev74 c10=4 → 0x80, ev08_02250 c10=7 → 0xB0). Sa taille varie
    // avec le nombre d'objets, on ne la suppose donc pas.
    let o_params = hs + nobj * CLIP_ENTRY_LEN;
    let o_names_header = section(10);
    if o_names_header < o_params || o_names_header + 4 > data.len() {
        return Err(FormatError::Malformed(format!(
            "table de noms annoncée à 0x{o_names_header:X}, hors de portée"
        )));
    }
    let params = data[o_params..o_names_header].to_vec();

    // La table de noms commence par `nobj` offsets u16 (relatifs à son propre début), alignés
    // sur 4 octets, suivis des chaînes ASCII zéro-terminées. Vérifié : nobj=3 → offsets 8/14/20,
    // nobj=5 → 12/18/24/30/36, nobj=1 → 4.
    let mut names = Vec::with_capacity(nobj);
    let mut names_end = o_names_header;
    for i in 0..nobj {
        let off = usize::from(u16_at(data, o_names_header + i * 2)?);
        let at = o_names_header + off;
        let raw = data.get(at..).ok_or(FormatError::TooShort {
            got: data.len(),
            need: at + 1,
        })?;
        let end = raw
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| FormatError::Malformed(format!("nom d'objet non terminé à 0x{at:X}")))?;
        names.push(String::from_utf8_lossy(&raw[..end]).into_owned());
        names_end = names_end.max(at + end + 1);
    }
    if names_end > o_objects {
        return Err(FormatError::Malformed(format!(
            "table de noms (fin 0x{names_end:X}) déborde sur la table d'objets (0x{o_objects:X})"
        )));
    }
    let gap_names_objects = data[names_end.min(data.len())..o_objects.min(data.len())].to_vec();

    // Table d'objets : son nombre d'entrées se déduit de la place disponible avant les canaux.
    // Il vaut `nobj` dans le cas courant, mais pas toujours (cf. doc de `AnimObject`).
    let n_entries = ((o_channels - o_objects) / OBJECT_ENTRY_LEN).min(nobj);
    let mut objects = Vec::with_capacity(n_entries);
    for i in 0..n_entries {
        let at = o_objects + i * OBJECT_ENTRY_LEN;
        objects.push(AnimObject {
            field0: u16_at(data, at)?,
            first_channel: u16_at(data, at + 2)?,
            channel_count: u32_at(data, at + 4)?,
        });
    }
    let objects_end = o_objects + n_entries * OBJECT_ENTRY_LEN;
    let gap_objects_channels =
        data[objects_end.min(data.len())..o_channels.min(data.len())].to_vec();

    // Table de canaux.
    let total: usize = objects.iter().map(|o| o.channel_count as usize).sum();
    if total > (data.len() - o_channels) / CHANNEL_ENTRY_LEN {
        return Err(FormatError::Malformed(format!(
            "{total} canaux annoncés, le fichier n'en contient pas autant"
        )));
    }
    struct RawChan {
        kind: u8,
        mode: u8,
        c1: u8,
        s1: u8,
        c2: u8,
        s2: u8,
        index: u16,
        time_index: u32,
        value_offset: u32,
        count: u32,
    }
    let mut raws = Vec::with_capacity(total);
    for i in 0..total {
        let at = o_channels + i * CHANNEL_ENTRY_LEN;
        let hdr = data.get(at..at + 8).ok_or(FormatError::TooShort {
            got: data.len(),
            need: at + 8,
        })?;
        raws.push(RawChan {
            kind: hdr[0],
            mode: hdr[1],
            c1: hdr[2],
            s1: hdr[3],
            c2: hdr[4],
            s2: hdr[5],
            index: u16::from_le_bytes([hdr[6], hdr[7]]),
            time_index: u32_at(data, at + 8)?,
            value_offset: u32_at(data, at + 12)?,
            count: u32_at(data, at + 16)?,
        });
    }
    let channels_end = o_channels + total * CHANNEL_ENTRY_LEN;

    // Table de temps : commence après les canaux (aligné 16), longueur = max(time_index + count).
    let o_times = (channels_end + 15) & !15;
    let ntimes = raws
        .iter()
        .map(|r| r.time_index as usize + r.count as usize)
        .max()
        .unwrap_or(0);
    let gap_channels_times = data
        .get(channels_end..o_times.min(data.len()))
        .unwrap_or(&[])
        .to_vec();
    let times_end = o_times + ntimes * 2;
    if times_end > data.len() {
        return Err(FormatError::Malformed(format!(
            "table de temps ({ntimes} entrées) déborde du fichier"
        )));
    }
    let mut times = Vec::with_capacity(ntimes);
    for i in 0..ntimes {
        times.push(u16_at(data, o_times + i * 2)?);
    }

    // Section valeurs : la fin de la dernière valeur doit tomber dans le fichier, et tout ce qui
    // suit doit être nul (padding). On part de l'alignement 16 — le cas nominal — et on élargit
    // la recherche si besoin, sans jamais accepter un placement qui écraserait des données.
    let values_len: usize = raws
        .iter()
        .map(|r| r.value_offset as usize + r.count as usize * usize::from(r.s1.max(1)))
        .max()
        .unwrap_or(0);
    let o_values = locate_values(data, times_end, values_len)?;
    let gap_times_values = data[times_end..o_values].to_vec();

    let mut channels = Vec::with_capacity(total);
    for r in &raws {
        let base = o_values + r.value_offset as usize;
        let n = r.count as usize;
        let track = match r.s1 {
            4 => {
                let mut v = Vec::with_capacity(n);
                for i in 0..n {
                    v.push(f32::from_bits(u32_at(data, base + i * 4)?));
                }
                Track::F32(v)
            }
            2 => {
                let mut v = Vec::with_capacity(n);
                for i in 0..n {
                    v.push(u16_at(data, base + i * 2)?);
                }
                Track::Raw16(v)
            }
            _ => {
                let end = base + n;
                Track::Raw8(
                    data.get(base..end)
                        .ok_or(FormatError::TooShort {
                            got: data.len(),
                            need: end,
                        })?
                        .to_vec(),
                )
            }
        };
        channels.push(Channel {
            kind: ChannelKind::from_code(r.kind),
            mode: r.mode,
            components: (r.c1, r.c2),
            declared_size: (r.s1, r.s2),
            index: r.index,
            time_index: r.time_index,
            value_offset: r.value_offset,
            track,
        });
    }

    let trailer = data[(o_values + values_len).min(data.len())..].to_vec();
    Ok(CameraAnim {
        header,
        counters,
        clips,
        params,
        names,
        objects,
        channels,
        times,
        gap_names_objects,
        gap_objects_channels,
        gap_channels_times,
        gap_times_values,
        trailer,
    })
}

/// Localise le début de la section « valeurs ».
///
/// Cas nominal : juste après la table de temps, aligné sur 16. Sinon on avance de 2 en 2 en
/// n'acceptant qu'un placement où la section entière tient dans le fichier **et** où tout ce
/// qui suit est nul — un placement trop tardif tronquerait des données réelles, un placement
/// trop tôt laisserait des octets non nuls après la fin.
fn locate_values(data: &[u8], times_end: usize, values_len: usize) -> Result<usize> {
    let nominal = (times_end + 15) & !15;
    let fits =
        |v: usize| v + values_len <= data.len() && data[v + values_len..].iter().all(|&b| b == 0);
    if fits(nominal) {
        return Ok(nominal);
    }
    let mut v = times_end;
    while v + values_len <= data.len() {
        if fits(v) {
            return Ok(v);
        }
        v += 2;
    }
    Err(FormatError::Malformed(format!(
        "section valeurs introuvable : {values_len} octets ne tiennent nulle part après \
         0x{times_end:X} dans un fichier de {} octets",
        data.len()
    )))
}

/// Ré-encode une animation.
///
/// Sur une animation issue de [`decode`] et non modifiée, la sortie est **identique à l'octet
/// près** à l'entrée : les sections sont réécrites à leurs offsets d'origine (recalculés par la
/// même formule que le loader) et les interstices sont restitués tels quels.
///
/// # Errors
/// [`FormatError::Malformed`] si les offsets recalculés se chevauchent (structure modifiée de
/// façon incohérente).
pub fn encode(anim: &CameraAnim) -> Result<Vec<u8>> {
    let hs = anim.header.header_size as usize;
    let align = anim.header.align as usize;
    let shift = u32::from(anim.counters[11]);
    let section = |i: usize| ((usize::from(anim.counters[i]) << shift) + align) * 4;
    let o_objects = section(2);
    let o_channels = section(3);

    let mut out = vec![0u8; hs];
    out[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    out[4..6].copy_from_slice(&anim.header.header_size.to_le_bytes());
    out[6..8].copy_from_slice(&anim.header.type_id.to_le_bytes());
    out[8..10].copy_from_slice(&anim.header.reserved0.to_le_bytes());
    out[10..12].copy_from_slice(&anim.header.align.to_le_bytes());
    out[12..16].copy_from_slice(&anim.header.data_size.to_le_bytes());
    for (i, c) in anim.counters.iter().enumerate() {
        out[0x20 + i * 2..0x20 + i * 2 + 2].copy_from_slice(&c.to_le_bytes());
    }

    for c in &anim.clips {
        out.extend_from_slice(&c.start.to_le_bytes());
        out.extend_from_slice(&c.end.to_le_bytes());
        out.extend_from_slice(&c.index.to_le_bytes());
        out.extend_from_slice(&c.flags.to_le_bytes());
        out.extend_from_slice(&c.tail);
    }
    out.extend_from_slice(&anim.params);
    let o_names_header = section(10);
    if out.len() != o_names_header {
        return Err(FormatError::Malformed(format!(
            "table de noms attendue à 0x{o_names_header:X}, l'écriture est arrivée à 0x{:X}",
            out.len()
        )));
    }
    // Table d'offsets (alignée 4) puis les noms zéro-terminés.
    let table_len = (anim.names.len() * 2).div_ceil(4) * 4;
    let mut cursor = table_len;
    for name in &anim.names {
        out.extend_from_slice(
            &u16::try_from(cursor)
                .map_err(|_| {
                    FormatError::Malformed(
                        "table de noms trop grande pour un offset u16".to_string(),
                    )
                })?
                .to_le_bytes(),
        );
        cursor += name.len() + 1;
    }
    out.resize(o_names_header + table_len, 0);
    for name in &anim.names {
        out.extend_from_slice(name.as_bytes());
        out.push(0);
    }
    out.extend_from_slice(&anim.gap_names_objects);
    if out.len() != o_objects {
        return Err(FormatError::Malformed(format!(
            "table d'objets attendue à 0x{o_objects:X}, l'écriture est arrivée à 0x{:X}",
            out.len()
        )));
    }
    for o in &anim.objects {
        out.extend_from_slice(&o.field0.to_le_bytes());
        out.extend_from_slice(&o.first_channel.to_le_bytes());
        out.extend_from_slice(&o.channel_count.to_le_bytes());
    }
    out.extend_from_slice(&anim.gap_objects_channels);
    if out.len() != o_channels {
        return Err(FormatError::Malformed(format!(
            "table de canaux attendue à 0x{o_channels:X}, l'écriture est arrivée à 0x{:X}",
            out.len()
        )));
    }
    for c in &anim.channels {
        out.push(c.kind.code());
        out.push(c.mode);
        out.push(c.components.0);
        out.push(c.declared_size.0);
        out.push(c.components.1);
        out.push(c.declared_size.1);
        out.extend_from_slice(&c.index.to_le_bytes());
        out.extend_from_slice(&c.time_index.to_le_bytes());
        out.extend_from_slice(&c.value_offset.to_le_bytes());
        let count = u32::try_from(c.track.len()).map_err(|_| {
            FormatError::Malformed("canal de plus de 2^32 échantillons".to_string())
        })?;
        out.extend_from_slice(&count.to_le_bytes());
    }
    // Table de temps : alignée 16 après les canaux (l'interstice d'origine est rejoué).
    out.extend_from_slice(&anim.gap_channels_times);
    for t in &anim.times {
        out.extend_from_slice(&t.to_le_bytes());
    }
    out.extend_from_slice(&anim.gap_times_values);

    // Valeurs : chaque canal à son offset déclaré, relatif au début de la section.
    let o_values = out.len();
    let values_len: usize = anim
        .channels
        .iter()
        .map(|c| c.value_offset as usize + c.track.len() * c.track.elem_size())
        .max()
        .unwrap_or(0);
    out.resize(o_values + values_len, 0);
    for c in &anim.channels {
        let at = o_values + c.value_offset as usize;
        match &c.track {
            Track::F32(v) => {
                for (i, x) in v.iter().enumerate() {
                    out[at + i * 4..at + i * 4 + 4].copy_from_slice(&x.to_bits().to_le_bytes());
                }
            }
            Track::Raw16(v) => {
                for (i, x) in v.iter().enumerate() {
                    out[at + i * 2..at + i * 2 + 2].copy_from_slice(&x.to_le_bytes());
                }
            }
            Track::Raw8(v) => out[at..at + v.len()].copy_from_slice(v),
        }
    }
    out.extend_from_slice(&anim.trailer);
    Ok(out)
}

impl CameraAnim {
    /// Nombre d'objets animés (caméras) du fichier.
    #[must_use]
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Nom de l'objet `i` (`""` si la table de noms n'en déclare pas autant).
    #[must_use]
    pub fn name_of(&self, i: usize) -> &str {
        self.names.get(i).map_or("", String::as_str)
    }

    /// Canaux appartenant à l'objet `i`.
    #[must_use]
    pub fn channels_of(&self, i: usize) -> &[Channel] {
        let Some(o) = self.objects.get(i) else {
            return &[];
        };
        let a = o.first_channel as usize;
        let b = a
            .saturating_add(o.channel_count as usize)
            .min(self.channels.len());
        self.channels.get(a..b).unwrap_or(&[])
    }

    /// Intervalle de frames couvert par la table de temps (`None` si vide).
    #[must_use]
    pub fn frame_range(&self) -> Option<(u16, u16)> {
        let lo = *self.times.iter().min()?;
        let hi = *self.times.iter().max()?;
        Some((lo, hi))
    }

    /// Proportion d'échantillons réellement décodés (flux `f32`) sur le total.
    ///
    /// Utile pour savoir ce qu'on peut exploiter d'un fichier donné sans lire son détail.
    #[must_use]
    pub fn decoded_ratio(&self) -> f32 {
        let total: usize = self.channels.iter().map(|c| c.track.len()).sum();
        if total == 0 {
            return 0.0;
        }
        let dec: usize = self
            .channels
            .iter()
            .filter(|c| matches!(c.track, Track::F32(_)))
            .map(|c| c.track.len())
            .sum();
        dec as f32 / total as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un G4CM synthétique minimal : 1 objet, 1 canal `f32` de 2 échantillons.
    fn synthetic() -> Vec<u8> {
        let align = 16usize;
        let shift = 2u32;
        // Choix des compteurs : objets à 0x90 (c2=5), canaux à 0xA0 (c3=6).
        let c2 = 5u16;
        let c3 = 6u16;
        // Table de noms à 0x80 : ((4 << 2) + 16) * 4.
        let c10 = 4u16;
        let o_objects = ((usize::from(c2) << shift) + align) * 4;
        let o_channels = ((usize::from(c3) << shift) + align) * 4;
        let mut d = vec![0u8; 0x40];
        d[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        d[4..6].copy_from_slice(&0x40u16.to_le_bytes());
        d[6..8].copy_from_slice(&VERSION.to_le_bytes());
        d[10..12].copy_from_slice(&(align as u16).to_le_bytes());
        let mut counters = [0u16; COUNTER_COUNT];
        counters[0] = 1;
        counters[2] = c2;
        counters[3] = c3;
        counters[10] = c10;
        counters[11] = shift as u16;
        for (i, c) in counters.iter().enumerate() {
            d[0x20 + i * 2..0x20 + i * 2 + 2].copy_from_slice(&c.to_le_bytes());
        }
        // clip
        d.extend_from_slice(&1000u16.to_le_bytes());
        d.extend_from_slice(&1001u16.to_le_bytes());
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&[0u8; 8]);
        d.extend_from_slice(&[0u8; 48]); // params
        d.extend_from_slice(&4u16.to_le_bytes()); // table d'offsets : 1 nom à +4
        d.extend_from_slice(&0u16.to_le_bytes()); // alignement 4
        d.extend_from_slice(b"c0010"); // nom
        d.push(0); // terminateur
        d.resize(o_objects, 0); // gap
        d.extend_from_slice(&0u16.to_le_bytes()); // field0
        d.extend_from_slice(&0u16.to_le_bytes()); // first_channel
        d.extend_from_slice(&1u32.to_le_bytes()); // channel_count
        d.resize(o_channels, 0); // gap
        // canal : posX, f32, 2 échantillons, temps 0..2, valeurs @0
        d.extend_from_slice(&[0x16, 1, 1, 4, 1, 4]);
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&2u32.to_le_bytes());
        let o_times = (d.len() + 15) & !15;
        d.resize(o_times, 0);
        d.extend_from_slice(&1000u16.to_le_bytes());
        d.extend_from_slice(&1001u16.to_le_bytes());
        let o_values = (d.len() + 15) & !15;
        d.resize(o_values, 0);
        d.extend_from_slice(&(-47.5f32).to_bits().to_le_bytes());
        d.extend_from_slice(&(-48.25f32).to_bits().to_le_bytes());
        let data_size = (d.len() - 0x40) as u32;
        d[12..16].copy_from_slice(&data_size.to_le_bytes());
        d
    }

    #[test]
    fn decode_synthetique() {
        let raw = synthetic();
        let a = decode(&raw).expect("décodage");
        assert_eq!(a.object_count(), 1);
        assert_eq!(a.name_of(0), "c0010");
        assert_eq!(a.clips[0].start, 1000);
        assert_eq!(a.channels.len(), 1);
        assert_eq!(a.channels[0].kind, ChannelKind::PosX);
        assert_eq!(a.channels[0].track.values(), Some(&[-47.5f32, -48.25][..]));
        assert_eq!(a.times, vec![1000, 1001]);
        assert_eq!(a.frame_range(), Some((1000, 1001)));
        assert!((a.decoded_ratio() - 1.0).abs() < f32::EPSILON);
        assert_eq!(a.channels_of(0).len(), 1);
    }

    #[test]
    fn round_trip_byte_exact() {
        let raw = synthetic();
        let a = decode(&raw).expect("décodage");
        let re = encode(&a).expect("encodage");
        assert_eq!(re, raw, "le ré-encodage doit être byte-exact");
    }

    #[test]
    fn edition_puis_encodage() {
        let raw = synthetic();
        let mut a = decode(&raw).expect("décodage");
        if let Track::F32(v) = &mut a.channels[0].track {
            v[0] = -10.0;
        }
        let re = encode(&a).expect("encodage");
        let b = decode(&re).expect("re-décodage");
        assert_eq!(b.channels[0].track.values(), Some(&[-10.0f32, -48.25][..]));
        assert_eq!(
            re.len(),
            raw.len(),
            "l'édition d'une valeur ne change pas la taille"
        );
    }

    #[test]
    fn rejette_magic_et_version() {
        assert!(matches!(
            decode(&[0u8; 0x40]),
            Err(FormatError::BadMagic { .. })
        ));
        let mut raw = synthetic();
        raw[6..8].copy_from_slice(&0x0066u16.to_le_bytes());
        assert!(matches!(decode(&raw), Err(FormatError::Malformed(_))));
        assert!(is_g4cm(b"G4CM____"));
        assert!(!is_g4cm(b"NAVM"));
    }

    #[test]
    fn kinds_aller_retour() {
        for code in [0x16u8, 0x17, 0x18, 0x1A, 0x1B, 0x1C, 0x1E, 0x1F, 0x99] {
            assert_eq!(ChannelKind::from_code(code).code(), code);
        }
        assert_eq!(ChannelKind::from_code(0x1E).label(), "fov");
    }
}
