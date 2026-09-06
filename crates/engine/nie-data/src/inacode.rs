//! `inacode` — port du `inacode_config_*.cfg.bin` (donnees du reseau social in-game
//! « Inacode » : stickers/stamps, auteurs, salons, commentaires facon SNS).
//!
//! ## Vérité terrain
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/inacode-config.ts`
//!   (`parseContent`, switch sur `list.name`). inagle ne typait explicitement que
//!   `m_InacodeStampDataList` (`{idCrc, imgNameCrc, imgPathCrc}`) et passait les autres
//!   listes en brut ; **ce port couvre les 10 listes réelles** avec des structs 1:1 sur
//!   les noms de champs Level-5 de la table de types RDBN (préservés tels quels).
//! - Données : `data/common/gamedata/inacode/inacode_config_1.01.57.00.cfg.bin` (VFS Steam),
//!   format **RDBN à listes** (`{ "lists": [ { "name", "typeName", "values": [...] } ] }`).
//!
//! ## Les 10 listes (nom → type Level-5, nombre d'entrées du dump réel)
//!
//! | Liste                            | Type RDBN                  | n   |
//! |----------------------------------|----------------------------|-----|
//! | `m_InacodeStampDataList`         | `INACODE_STAMP_DATA`       | 15  |
//! | `m_InacodeAuthorNameDataList`    | `INACODE_AUTHOR_NAME_DATA` | 29  |
//! | `m_InacodePhotoImgDataList`      | `INACODE_PHOTO_IMG_DATA`   | 10  |
//! | `m_InacodeMemberDataList`        | `INACODE_MEMBER_DATA`      | 33  |
//! | `m_InacodeCommentSetIdDataList`  | `INACODE_COMMENT_SET_ID_DATA` | 74 |
//! | `m_InacodeRoomDataList`          | `INACODE_ROOM_DATA`        | 2   |
//! | `m_InacodeSelectionDataList`     | `INACODE_SELECTION_DATA`   | 8   |
//! | `m_InacodeMentionDataList`       | `INACODE_MENTION_DATA`     | 6   |
//! | `m_InacodeCommentDataList`       | `INACODE_COMMENT_DATA`     | 968 |
//! | `m_InacodeCommentSetDataList`    | `INACODE_COMMENT_SET_DATA` | 83  |
//!
//! ## Encodage iecode des champs (cf. `cfgbin_to_iecode_root` de `nie-model-serve`)
//!
//! - `Hash` (CRC u32) → chaîne `"0x........"` (lue en [`HashId`]).
//! - `Short`/`Byte` → entier JSON (lu en `i64`).
//! - `Bool` → booléen JSON.
//! - `ShortTuple` (2 × i16) → tableau JSON `[offset, count]` ([`InacodeRange`]) : une plage
//!   `[offset, offset+count)` dans une liste fratrie (ex. `room.member` → `m_InacodeMemberDataList`).
//! - `Condition` (offset de chaîne) → chaîne JSON : soit une condition base64 résolue
//!   (`"AAAA…"`), soit le repli `"0xFFFFFFFF"` quand l'offset ne pointe pas une chaîne.
//!   Conservé **brut** en `String` pour ne perdre aucune donnée (champs `*Cond`).

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_str, list_values, owned};
use crate::hash::HashId;

/// Plage `(offset, count)` d'un champ RDBN `ShortTuple` (2 × i16) : référence l'intervalle
/// `[offset, offset+count)` d'une liste fratrie (ex. `room.member` indexe les 28 entrées
/// `m_InacodeMemberDataList` à partir de `offset`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeRange {
    /// Index de départ dans la liste fratrie référencée.
    pub offset: i16,
    /// Nombre d'entrées consécutives.
    pub count: i16,
}

/// `INACODE_STAMP_DATA` — un sticker/stamp postable (`m_InacodeStampDataList`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeStampData {
    /// `idCrc` — CRC identifiant du stamp.
    pub id_crc: HashId,
    /// `imgNameCrc` — CRC du nom d'image.
    pub img_name_crc: HashId,
    /// `imgPathCrc` — CRC du chemin d'image.
    pub img_path_crc: HashId,
}

impl InacodeStampData {
    /// Parse une entrée `INACODE_STAMP_DATA`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            id_crc: field_hash(v, "idCrc"),
            img_name_crc: field_hash(v, "imgNameCrc"),
            img_path_crc: field_hash(v, "imgPathCrc"),
        }
    }
}

/// `INACODE_AUTHOR_NAME_DATA` — un auteur de message (`m_InacodeAuthorNameDataList`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeAuthorNameData {
    /// `authorCrc` — CRC identifiant de l'auteur (recoupe `member.idCrc`).
    pub author_crc: HashId,
    /// `authorNameTextIdCrc` — CRC du texte localisé du nom d'auteur.
    pub author_name_text_id_crc: HashId,
}

impl InacodeAuthorNameData {
    /// Parse une entrée `INACODE_AUTHOR_NAME_DATA`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            author_crc: field_hash(v, "authorCrc"),
            author_name_text_id_crc: field_hash(v, "authorNameTextIdCrc"),
        }
    }
}

/// `INACODE_PHOTO_IMG_DATA` — une image attachable (`m_InacodePhotoImgDataList`).
/// Mêmes champs que [`InacodeStampData`] mais type Level-5 distinct (conservé 1:1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodePhotoImgData {
    /// `idCrc` — CRC identifiant de la photo.
    pub id_crc: HashId,
    /// `imgNameCrc` — CRC du nom d'image.
    pub img_name_crc: HashId,
    /// `imgPathCrc` — CRC du chemin d'image.
    pub img_path_crc: HashId,
}

impl InacodePhotoImgData {
    /// Parse une entrée `INACODE_PHOTO_IMG_DATA`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            id_crc: field_hash(v, "idCrc"),
            img_name_crc: field_hash(v, "imgNameCrc"),
            img_path_crc: field_hash(v, "imgPathCrc"),
        }
    }
}

/// `INACODE_MEMBER_DATA` — un membre du réseau (`m_InacodeMemberDataList`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeMemberData {
    /// `idCrc` — CRC identifiant du membre (recoupe `author.authorCrc`).
    pub id_crc: HashId,
    /// `memberCond` — condition de déblocage (`Condition`, brut : `"0xFFFFFFFF"` ou base64).
    pub member_cond: String,
}

impl InacodeMemberData {
    /// Parse une entrée `INACODE_MEMBER_DATA`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            id_crc: field_hash(v, "idCrc"),
            member_cond: owned(field_str(v, "memberCond").unwrap_or("")),
        }
    }
}

/// `INACODE_COMMENT_SET_ID_DATA` — un identifiant de jeu de commentaires
/// (`m_InacodeCommentSetIdDataList`), référencé par les plages `room.comment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeCommentSetIdData {
    /// `idCrc` — CRC identifiant du jeu de commentaires (recoupe `commentSet.idCrc`).
    pub id_crc: HashId,
}

impl InacodeCommentSetIdData {
    /// Parse une entrée `INACODE_COMMENT_SET_ID_DATA`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            id_crc: field_hash(v, "idCrc"),
        }
    }
}

/// `INACODE_ROOM_DATA` — un salon (`m_InacodeRoomDataList`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeRoomData {
    /// `idCrc` — CRC identifiant du salon.
    pub id_crc: HashId,
    /// `orderby` — ordre d'affichage.
    pub orderby: i64,
    /// `name` — CRC du texte du nom de salon.
    pub name: HashId,
    /// `openBitFlag` — flag binaire d'ouverture.
    pub open_bit_flag: HashId,
    /// `newBitFlag` — flag binaire « nouveau ».
    pub new_bit_flag: HashId,
    /// `newCommentBitFlag` — flag binaire « nouveau commentaire ».
    pub new_comment_bit_flag: HashId,
    /// `readedCommentFlagIndex` — index du flag « commentaire lu ».
    pub readed_comment_flag_index: i64,
    /// `openCond` — condition d'ouverture (`Condition`, brut).
    pub open_cond: String,
    /// `member` — plage dans `m_InacodeMemberDataList`.
    pub member: InacodeRange,
    /// `comment` — plage dans `m_InacodeCommentSetIdDataList`.
    pub comment: InacodeRange,
}

impl InacodeRoomData {
    /// Parse une entrée `INACODE_ROOM_DATA`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            id_crc: field_hash(v, "idCrc"),
            orderby: field_i64(v, "orderby").unwrap_or(0),
            name: field_hash(v, "name"),
            open_bit_flag: field_hash(v, "openBitFlag"),
            new_bit_flag: field_hash(v, "newBitFlag"),
            new_comment_bit_flag: field_hash(v, "newCommentBitFlag"),
            readed_comment_flag_index: field_i64(v, "readedCommentFlagIndex").unwrap_or(0),
            open_cond: owned(field_str(v, "openCond").unwrap_or("")),
            member: field_range(v, "member"),
            comment: field_range(v, "comment"),
        }
    }
}

/// `INACODE_SELECTION_DATA` — une option de réponse à choix (`m_InacodeSelectionDataList`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeSelectionData {
    /// `index` — index de l'option dans son groupe de sélection.
    pub index: i64,
    /// `text` — CRC du texte localisé de l'option.
    pub text: HashId,
}

impl InacodeSelectionData {
    /// Parse une entrée `INACODE_SELECTION_DATA`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            index: field_i64(v, "index").unwrap_or(0),
            text: field_hash(v, "text"),
        }
    }
}

/// `INACODE_MENTION_DATA` — une mention de personnage (`m_InacodeMentionDataList`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeMentionData {
    /// `charaIdCrc` — CRC identifiant du personnage mentionné (recoupe `comment.author`).
    pub chara_id_crc: HashId,
}

impl InacodeMentionData {
    /// Parse une entrée `INACODE_MENTION_DATA`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            chara_id_crc: field_hash(v, "charaIdCrc"),
        }
    }
}

/// `INACODE_COMMENT_DATA` — un commentaire/message (`m_InacodeCommentDataList`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeCommentData {
    /// `author` — CRC de l'auteur du message.
    pub author: HashId,
    /// `overwriteAuthor` — CRC d'auteur de remplacement (`0x00000000` si aucun).
    pub overwrite_author: HashId,
    /// `text` — CRC du texte localisé du message.
    pub text: HashId,
    /// `isMentionAll` — `true` si le message mentionne tout le monde.
    pub is_mention_all: bool,
    /// `mention` — plage dans `m_InacodeMentionDataList`.
    pub mention: InacodeRange,
    /// `hour` — heure d'horodatage affichée (0-23).
    pub hour: i64,
    /// `minutes` — minutes d'horodatage affichées (0-59).
    pub minutes: i64,
    /// `selectionFlagIndex` — index du flag de la sélection choisie.
    pub selection_flag_index: i64,
    /// `selection` — plage dans `m_InacodeSelectionDataList`.
    pub selection: InacodeRange,
    /// `disableReplyCond` — condition de désactivation de réponse (`Condition`, brut).
    pub disable_reply_cond: String,
    /// `stamp` — CRC du stamp attaché (`0x00000000` si aucun).
    pub stamp: HashId,
    /// `photoImg` — CRC de la photo attachée (`0x00000000` si aucune).
    pub photo_img: HashId,
    /// `needSeparatorLine` — `true` s'il faut une ligne séparatrice avant ce message.
    pub need_separator_line: bool,
}

impl InacodeCommentData {
    /// Parse une entrée `INACODE_COMMENT_DATA`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            author: field_hash(v, "author"),
            overwrite_author: field_hash(v, "overwriteAuthor"),
            text: field_hash(v, "text"),
            is_mention_all: field_bool(v, "isMentionAll").unwrap_or(false),
            mention: field_range(v, "mention"),
            hour: field_i64(v, "hour").unwrap_or(0),
            minutes: field_i64(v, "minutes").unwrap_or(0),
            selection_flag_index: field_i64(v, "selectionFlagIndex").unwrap_or(0),
            selection: field_range(v, "selection"),
            disable_reply_cond: owned(field_str(v, "disableReplyCond").unwrap_or("")),
            stamp: field_hash(v, "stamp"),
            photo_img: field_hash(v, "photoImg"),
            need_separator_line: field_bool(v, "needSeparatorLine").unwrap_or(false),
        }
    }
}

/// `INACODE_COMMENT_SET_DATA` — un jeu de commentaires déblocable (`m_InacodeCommentSetDataList`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeCommentSetData {
    /// `idCrc` — CRC identifiant du jeu de commentaires.
    pub id_crc: HashId,
    /// `openCond` — condition d'ouverture (`Condition`, brut).
    pub open_cond: String,
    /// `openedFlagIndex` — index du flag d'ouverture.
    pub opened_flag_index: i64,
    /// `comment` — plage dans `m_InacodeCommentDataList`.
    pub comment: InacodeRange,
}

impl InacodeCommentSetData {
    /// Parse une entrée `INACODE_COMMENT_SET_DATA`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            id_crc: field_hash(v, "idCrc"),
            open_cond: owned(field_str(v, "openCond").unwrap_or("")),
            opened_flag_index: field_i64(v, "openedFlagIndex").unwrap_or(0),
            comment: field_range(v, "comment"),
        }
    }
}

/// Base de données `inacode` complète : les 10 listes parsées (port de `parseContent` d'inagle,
/// étendu aux 10 listes réelles).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InacodeConfig {
    /// `m_InacodeStampDataList` — stamps postables.
    pub stamps: Vec<InacodeStampData>,
    /// `m_InacodeAuthorNameDataList` — auteurs.
    pub authors: Vec<InacodeAuthorNameData>,
    /// `m_InacodePhotoImgDataList` — images attachables.
    pub photo_imgs: Vec<InacodePhotoImgData>,
    /// `m_InacodeMemberDataList` — membres.
    pub members: Vec<InacodeMemberData>,
    /// `m_InacodeCommentSetIdDataList` — identifiants de jeux de commentaires.
    pub comment_set_ids: Vec<InacodeCommentSetIdData>,
    /// `m_InacodeRoomDataList` — salons.
    pub rooms: Vec<InacodeRoomData>,
    /// `m_InacodeSelectionDataList` — options de réponse.
    pub selections: Vec<InacodeSelectionData>,
    /// `m_InacodeMentionDataList` — mentions.
    pub mentions: Vec<InacodeMentionData>,
    /// `m_InacodeCommentDataList` — commentaires/messages.
    pub comments: Vec<InacodeCommentData>,
    /// `m_InacodeCommentSetDataList` — jeux de commentaires déblocables.
    pub comment_sets: Vec<InacodeCommentSetData>,
}

impl InacodeConfig {
    /// Recherche un stamp par son `idCrc` (port de `byStampId` d'inagle).
    #[must_use]
    pub fn find_stamp(&self, id_crc: HashId) -> Option<&InacodeStampData> {
        self.stamps.iter().find(|s| s.id_crc == id_crc)
    }
}

/// Lit un champ `ShortTuple` (tableau JSON `[offset, count]`) en [`InacodeRange`].
/// Tout élément absent vaut 0 (les tuples du dump sont toujours des paires d'entiers).
fn field_range(v: &Value, key: &str) -> InacodeRange {
    let arr = v.get(key).and_then(Value::as_array);
    let read = |i: usize| -> i16 {
        arr.and_then(|a| a.get(i))
            .and_then(Value::as_i64)
            .unwrap_or(0) as i16
    };
    InacodeRange {
        offset: read(0),
        count: read(1),
    }
}

/// Parse un `inacode_config_*.cfg.bin` désérialisé (forme iecode `{ "lists": [...] }`),
/// switch sur `list.name` à l'identique d'inagle (`parseContent`), étendu aux 10 listes.
#[must_use]
pub fn parse_inacode_config(root: &Value) -> InacodeConfig {
    let mut cfg = InacodeConfig::default();

    if let Some(values) = list_values(root, "m_InacodeStampDataList") {
        cfg.stamps = values.iter().map(InacodeStampData::from_value).collect();
    }
    if let Some(values) = list_values(root, "m_InacodeAuthorNameDataList") {
        cfg.authors = values
            .iter()
            .map(InacodeAuthorNameData::from_value)
            .collect();
    }
    if let Some(values) = list_values(root, "m_InacodePhotoImgDataList") {
        cfg.photo_imgs = values.iter().map(InacodePhotoImgData::from_value).collect();
    }
    if let Some(values) = list_values(root, "m_InacodeMemberDataList") {
        cfg.members = values.iter().map(InacodeMemberData::from_value).collect();
    }
    if let Some(values) = list_values(root, "m_InacodeCommentSetIdDataList") {
        cfg.comment_set_ids = values
            .iter()
            .map(InacodeCommentSetIdData::from_value)
            .collect();
    }
    if let Some(values) = list_values(root, "m_InacodeRoomDataList") {
        cfg.rooms = values.iter().map(InacodeRoomData::from_value).collect();
    }
    if let Some(values) = list_values(root, "m_InacodeSelectionDataList") {
        cfg.selections = values
            .iter()
            .map(InacodeSelectionData::from_value)
            .collect();
    }
    if let Some(values) = list_values(root, "m_InacodeMentionDataList") {
        cfg.mentions = values.iter().map(InacodeMentionData::from_value).collect();
    }
    if let Some(values) = list_values(root, "m_InacodeCommentDataList") {
        cfg.comments = values.iter().map(InacodeCommentData::from_value).collect();
    }
    if let Some(values) = list_values(root, "m_InacodeCommentSetDataList") {
        cfg.comment_sets = values
            .iter()
            .map(InacodeCommentSetData::from_value)
            .collect();
    }

    cfg
}
