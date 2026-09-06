//! Famille `post` — port Rust des `*.cfg.bin.json` du système de courrier IEVR
//! (calendrier de l'Avent, livraisons/cadeaux, mots de passe, bannières d'annonces).
//!
//! ## Vérité terrain
//!
//! Cinq fichiers dans `data/common/gamedata/post/` :
//!
//! | Fichier | Layout | Contenu |
//! |---------|--------|---------|
//! | `advent_calendar_config_2.00.17.00.cfg.bin.json` | `lists` | calendrier de l'Avent + bonus de connexion |
//! | `delivery_config_1.03.63.00.cfg.bin.json` | `lists` | contenus de livraison + infos + mots de passe |
//! | `delivery_list_config.cfg.bin.json` | `entries` | arbre `DELIVERY_INFO` → `DELIVERY_INFO_DATA` |
//! | `password_list_config.cfg.bin.json` | `lists` | codes (par langue) + données de mot de passe |
//! | `post_notice_config_1.03.93.00.cfg.bin.json` | `lists` | bannières d'annonces (fond/badge/icône/texte) + infos |
//!
//! Chaque sous-fichier est porté en une struct `*Config` (ou un `Vec` pour
//! `delivery_list_config`). Les hash hex `"0x..."` deviennent des [`HashId`].
//!
//! ## Tableaux index `[offset, count]`
//!
//! Plusieurs champs sont des paires `[offset, count]` référençant une plage dans une
//! autre liste (convention déjà observée dans `search_word`/`info_bookmark`) :
//! - `deliveryContents` → plage dans `m_DeliveryContentsDataList` ;
//! - `codes` → plage dans `m_PasswordCodesList`/`m_Codes` ;
//! - `reward_item_list` / `reward_list` → plages dans les listes de récompenses.
//!
//! La contiguïté des plages sur le dump réel (0..1, 1..6, 6..7, 7..9, …) confirme la
//! sémantique offset/count plutôt qu'une liste d'indices directs.
//!
//! ## Zones d'ombre honnêtes
//!
//! - `valid_cond` (Avent, post-notice) : chaîne opaque. Sur le dump réel elle contient
//!   souvent des fragments tronqués du `typeName` (`"NDAR_INFO"`, `"d_crc"`,
//!   `"cell_type"`, `"POST_NOTICE_INFO"`, `"O"`) — artefact du décodage cfg.bin, conservé
//!   tel quel sans interprétation.
//! - `openCond` (livraisons) : base64 opaque ou chaîne vide.
//! - `conditions` (mot de passe) : base64 opaque.
//! - Les typos Level-5 sont préservées : `PASSWARD_*` (au lieu de `PASSWORD`) dans
//!   `password_list_config`, et la clé `"french "` (espace final) dans `m_Codes`.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, field_bool, field_hash, field_i64, field_str, list_values, owned};
use crate::hash::HashId;

// ─── Helpers ────────────────────────────────────────────────────────────────────

/// Lit un champ paire `[offset, count]` (`(0, 0)` si absent ou mal formé).
fn pair(v: &Value, key: &str) -> (i64, i64) {
    let arr = v.get(key).and_then(Value::as_array);
    let off = arr
        .and_then(|a| a.first())
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let cnt = arr
        .and_then(|a| a.get(1))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    (off, cnt)
}

/// Parcourt une liste nommée `lists[].values` et applique `f` à chaque entrée valide.
fn extract_list<T, F>(root: &Value, name: &str, f: F) -> Vec<T>
where
    F: Fn(&Value) -> Option<T>,
{
    let mut out = Vec::new();
    if let Some(values) = list_values(root, name) {
        for v in values {
            if let Some(item) = f(v) {
                out.push(item);
            }
        }
    }
    out
}

// ══════════════════════════════════════════════════════════════════════════════
//  advent_calendar_config
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `ADVENT_CALENDAR_INFO` — une case du calendrier de l'Avent.
///
/// Vérité terrain : `advent_calendar_config_2.00.17.00.cfg.bin.json`
/// `m_AdventCalendarInfoList[0]` : `id_crc = 0xF43EEA20`, `cell_type = 1`, `flag_no = 0`,
/// `time_start = 2025`, `time_end = 11`, `repeat_type = 11`, `item_info_id_crc = 0x000B07E9`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AdventCalendarInfo {
    /// `id_crc` — identifiant de la case.
    pub id_crc: HashId,
    /// `cell_type` — type de case (= 1 sur tout le dump).
    pub cell_type: i64,
    /// `flag_no` — numéro de flag de progression (0-basé séquentiel).
    pub flag_no: i64,
    /// `time_start` — année de début (ex. `2025`).
    pub time_start: i64,
    /// `time_end` — mois de fin / d'ouverture (ex. `11`).
    pub time_end: i64,
    /// `repeat_type` — jour ou cadence de répétition.
    pub repeat_type: i64,
    /// `data_id_crc` — hash de données associé (= 0 sur tout le dump).
    pub data_id_crc: HashId,
    /// `item_info_id_crc` — hash de l'item délivré.
    pub item_info_id_crc: HashId,
    /// `valid_cond` — condition de validité opaque (fragment de `typeName` sur le dump réel).
    pub valid_cond: String,
}

impl AdventCalendarInfo {
    /// Parse une entrée. `None` si `id_crc` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id_crc = field_hash(v, "id_crc");
        if id_crc.is_zero() {
            return None;
        }
        Some(Self {
            id_crc,
            cell_type: field_i64(v, "cell_type").unwrap_or(0),
            flag_no: field_i64(v, "flag_no").unwrap_or(0),
            time_start: field_i64(v, "time_start").unwrap_or(0),
            time_end: field_i64(v, "time_end").unwrap_or(0),
            repeat_type: field_i64(v, "repeat_type").unwrap_or(0),
            data_id_crc: field_hash(v, "data_id_crc"),
            item_info_id_crc: field_hash(v, "item_info_id_crc"),
            valid_cond: owned(field_str(v, "valid_cond").unwrap_or("")),
        })
    }
}

/// Entrée `NEWS_INFO` — bannière d'actualité associée au calendrier.
///
/// Vérité terrain : `m_NewsInfoList[1]` : `id_crc = 0x7A12D002`,
/// `headline_small_text_id = 0xA0EC5DEA`, `news_type = 0`,
/// `banner_start_res_name = "calendar_update_img_01_001"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NewsInfo {
    /// `id_crc` — identifiant (recoupe `AdventCalendarInfo.id_crc`).
    pub id_crc: HashId,
    /// `headline_small_text_id` — hash du petit titre.
    pub headline_small_text_id: HashId,
    /// `headline_large_text_id` — hash du grand titre.
    pub headline_large_text_id: HashId,
    /// `detail_text_id` — hash du texte de détail.
    pub detail_text_id: HashId,
    /// `banner_start_res_name` — nom de ressource bannière de début (peut être vide).
    pub banner_start_res_name: String,
    /// `banner_end_res_name` — nom de ressource bannière de fin (peut être vide).
    pub banner_end_res_name: String,
    /// `news_type` — type d'actualité.
    pub news_type: i64,
    /// `detail_window_res_name` — nom de ressource de la fenêtre de détail (peut être vide).
    pub detail_window_res_name: String,
}

impl NewsInfo {
    /// Parse une entrée. `None` si `id_crc` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id_crc = field_hash(v, "id_crc");
        if id_crc.is_zero() {
            return None;
        }
        Some(Self {
            id_crc,
            headline_small_text_id: field_hash(v, "headline_small_text_id"),
            headline_large_text_id: field_hash(v, "headline_large_text_id"),
            detail_text_id: field_hash(v, "detail_text_id"),
            banner_start_res_name: owned(field_str(v, "banner_start_res_name").unwrap_or("")),
            banner_end_res_name: owned(field_str(v, "banner_end_res_name").unwrap_or("")),
            news_type: field_i64(v, "news_type").unwrap_or(0),
            detail_window_res_name: owned(field_str(v, "detail_window_res_name").unwrap_or("")),
        })
    }
}

/// Entrée `LOGIN_BONUS_REWARD_ITEM` — un item de récompense de connexion.
///
/// Vérité terrain : `m_LoginBonusRewardItem[0]` : `item_id = 0x02601663`, `item_num = 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LoginBonusRewardItem {
    /// `item_id` — hash de l'item.
    pub item_id: HashId,
    /// `item_num` — quantité.
    pub item_num: i64,
}

impl LoginBonusRewardItem {
    /// Parse une entrée. `None` si `item_id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let item_id = field_hash(v, "item_id");
        if item_id.is_zero() {
            return None;
        }
        Some(Self {
            item_id,
            item_num: field_i64(v, "item_num").unwrap_or(0),
        })
    }
}

/// Entrée `LOGIN_BONUS_REWARD` — récompense d'un jour donné.
///
/// `reward_item_list = [offset, count]` référence une plage dans `m_LoginBonusRewardItem`.
///
/// Vérité terrain : `m_LoginBonusReward[0]` : `day_count = 1`, `reward_item_list = [0, 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LoginBonusReward {
    /// `day_count` — jour de connexion (1-basé).
    pub day_count: i64,
    /// `reward_item_list[0]` — index de début dans `m_LoginBonusRewardItem`.
    pub reward_item_offset: i64,
    /// `reward_item_list[1]` — nombre d'items.
    pub reward_item_count: i64,
}

impl LoginBonusReward {
    /// Parse une entrée (toujours `Some`).
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let (reward_item_offset, reward_item_count) = pair(v, "reward_item_list");
        Some(Self {
            day_count: field_i64(v, "day_count").unwrap_or(0),
            reward_item_offset,
            reward_item_count,
        })
    }
}

/// Entrée `LOGIN_BONUS_INFO` — groupe de récompenses de connexion.
///
/// `reward_list = [offset, count]` référence une plage dans `m_LoginBonusReward`.
///
/// Vérité terrain : `m_LoginBonusInfo[0]` : `id_crc = 0x447A797C`, `reward_list = [0, 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LoginBonusInfo {
    /// `id_crc` — identifiant du groupe.
    pub id_crc: HashId,
    /// `reward_list[0]` — index de début dans `m_LoginBonusReward`.
    pub reward_offset: i64,
    /// `reward_list[1]` — nombre de récompenses.
    pub reward_count: i64,
}

impl LoginBonusInfo {
    /// Parse une entrée. `None` si `id_crc` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id_crc = field_hash(v, "id_crc");
        if id_crc.is_zero() {
            return None;
        }
        let (reward_offset, reward_count) = pair(v, "reward_list");
        Some(Self {
            id_crc,
            reward_offset,
            reward_count,
        })
    }
}

/// Contenu complet d'un `advent_calendar_config_*.cfg.bin.json` (5 listes).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AdventCalendarConfig {
    /// `m_AdventCalendarInfoList` — cases du calendrier.
    pub calendar: Vec<AdventCalendarInfo>,
    /// `m_NewsInfoList` — bannières d'actualité.
    pub news: Vec<NewsInfo>,
    /// `m_LoginBonusRewardItem` — items de récompense de connexion.
    pub login_bonus_reward_items: Vec<LoginBonusRewardItem>,
    /// `m_LoginBonusReward` — récompenses par jour.
    pub login_bonus_rewards: Vec<LoginBonusReward>,
    /// `m_LoginBonusInfo` — groupes de récompenses.
    pub login_bonus_infos: Vec<LoginBonusInfo>,
}

/// Parse un `advent_calendar_config_*.cfg.bin.json` complet.
///
/// Vérité terrain : `advent_calendar_config_2.00.17.00.cfg.bin.json` →
/// 5 cases, 6 news, 1 item, 1 reward, 1 info.
#[must_use]
pub fn parse_advent_calendar_config(root: &Value) -> AdventCalendarConfig {
    AdventCalendarConfig {
        calendar: extract_list(
            root,
            "m_AdventCalendarInfoList",
            AdventCalendarInfo::from_value,
        ),
        news: extract_list(root, "m_NewsInfoList", NewsInfo::from_value),
        login_bonus_reward_items: extract_list(
            root,
            "m_LoginBonusRewardItem",
            LoginBonusRewardItem::from_value,
        ),
        login_bonus_rewards: extract_list(root, "m_LoginBonusReward", LoginBonusReward::from_value),
        login_bonus_infos: extract_list(root, "m_LoginBonusInfo", LoginBonusInfo::from_value),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  delivery_config
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `DELIVERY_CONTENTS_DATA` — un contenu (item/cadeau) d'une livraison.
///
/// Vérité terrain : `delivery_config_1.03.63.00.cfg.bin.json`
/// `m_DeliveryContentsDataList[0]` : `contents_type = 1`, `itemIdCrc = 0x6BB24A2D`,
/// `num = 1`, `isPassphrase = false`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeliveryContentsData {
    /// `contents_type` — type de contenu (= 1 = item sur tout le dump).
    pub contents_type: i64,
    /// `itemIdCrc` — hash de l'item.
    pub item_id_crc: HashId,
    /// `charaParamIdCrc` — hash de paramètre de personnage (souvent 0).
    pub chara_param_id_crc: HashId,
    /// `rarity` — rareté.
    pub rarity: i64,
    /// `num` — quantité.
    pub num: i64,
    /// `aocIdCrc` — hash de contenu additionnel (DLC, souvent 0).
    pub aoc_id_crc: HashId,
    /// `replaceItemIdCrc` — hash d'item de remplacement (souvent 0).
    pub replace_item_id_crc: HashId,
    /// `isPassphrase` — `true` si le contenu provient d'un mot de passe.
    pub is_passphrase: bool,
}

impl DeliveryContentsData {
    /// Parse une entrée (toujours `Some` — peut avoir un `itemIdCrc` nul selon `contents_type`).
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            contents_type: field_i64(v, "contents_type").unwrap_or(0),
            item_id_crc: field_hash(v, "itemIdCrc"),
            chara_param_id_crc: field_hash(v, "charaParamIdCrc"),
            rarity: field_i64(v, "rarity").unwrap_or(0),
            num: field_i64(v, "num").unwrap_or(0),
            aoc_id_crc: field_hash(v, "aocIdCrc"),
            replace_item_id_crc: field_hash(v, "replaceItemIdCrc"),
            is_passphrase: field_bool(v, "isPassphrase").unwrap_or(false),
        })
    }
}

/// Entrée `DELIVERY_INFO` — une livraison (courrier/cadeau).
///
/// `deliveryContents = [offset, count]` référence une plage dans `m_DeliveryContentsDataList`.
///
/// Vérité terrain : `m_DeliveryInfoList[0]` : `idCrc = 0x626B2A47`, `title = 0x6145669B`,
/// `receivedFlag = 0x066D378F`, `newFlag = 1`, `sendTargetType = 2`,
/// `deliveryContents = [0, 1]`, `openCond = ""`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeliveryInfo {
    /// `idCrc` — identifiant de la livraison.
    pub id_crc: HashId,
    /// `title` — hash du titre.
    pub title: HashId,
    /// `receivedFlag` — hash du flag « déjà reçu ».
    pub received_flag: HashId,
    /// `newFlag` — numéro de flag « nouveau » (entier).
    pub new_flag: i64,
    /// `sendTargetType` — cible d'envoi (1 ou 2 sur le dump).
    pub send_target_type: i64,
    /// `deliveryContents[0]` — index de début dans `m_DeliveryContentsDataList`.
    pub contents_offset: i64,
    /// `deliveryContents[1]` — nombre de contenus.
    pub contents_count: i64,
    /// `openCond` — condition d'ouverture (base64 opaque ou chaîne vide).
    pub open_cond: String,
}

impl DeliveryInfo {
    /// Parse une entrée. `None` si `idCrc` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id_crc = field_hash(v, "idCrc");
        if id_crc.is_zero() {
            return None;
        }
        let (contents_offset, contents_count) = pair(v, "deliveryContents");
        Some(Self {
            id_crc,
            title: field_hash(v, "title"),
            received_flag: field_hash(v, "receivedFlag"),
            new_flag: field_i64(v, "newFlag").unwrap_or(0),
            send_target_type: field_i64(v, "sendTargetType").unwrap_or(0),
            contents_offset,
            contents_count,
            open_cond: owned(field_str(v, "openCond").unwrap_or("")),
        })
    }
}

/// Entrée `PASSWORD_CODES` (livraisons) — chaîne(s) de code saisissable(s).
///
/// Contrairement au `PASSWARD_CODES` de `password_list_config` (qui stocke des hash),
/// cette liste stocke les codes **littéraux** : `japanese` en kana, `english` en
/// majuscules (l'un des deux est vide selon la langue).
///
/// Vérité terrain : `m_PasswordCodesList[3]` : `japanese = ""`, `english = "HPKCYJVQWTWX"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PasswordCodes {
    /// `japanese` — code en japonais (kana) ou chaîne vide.
    pub japanese: String,
    /// `english` — code en anglais (majuscules) ou chaîne vide.
    pub english: String,
}

impl PasswordCodes {
    /// Parse une entrée (toujours `Some`).
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            japanese: owned(field_str(v, "japanese").unwrap_or("")),
            english: owned(field_str(v, "english").unwrap_or("")),
        })
    }
}

/// Entrée `PASSWORD_DATA` (livraisons) — données d'un mot de passe.
///
/// `codes = [offset, count]` référence une plage dans `m_PasswordCodesList`.
///
/// Vérité terrain : `m_PasswordDataList[0]` : `id = 0x4E323AA7`, `text_id = 0x4D1C767B`,
/// `flag_id = 0x90908C13`, `codes = [0, 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PasswordData {
    /// `id` — identifiant du mot de passe.
    pub id: HashId,
    /// `text_id` — hash du texte associé.
    pub text_id: HashId,
    /// `flag_id` — hash du flag de déblocage.
    pub flag_id: HashId,
    /// `codes[0]` — index de début dans `m_PasswordCodesList`.
    pub codes_offset: i64,
    /// `codes[1]` — nombre de codes.
    pub codes_count: i64,
}

impl PasswordData {
    /// Parse une entrée. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        let (codes_offset, codes_count) = pair(v, "codes");
        Some(Self {
            id,
            text_id: field_hash(v, "text_id"),
            flag_id: field_hash(v, "flag_id"),
            codes_offset,
            codes_count,
        })
    }
}

/// Contenu complet d'un `delivery_config_*.cfg.bin.json` (4 listes).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeliveryConfig {
    /// `m_DeliveryContentsDataList` — contenus de livraison.
    pub contents: Vec<DeliveryContentsData>,
    /// `m_DeliveryInfoList` — livraisons.
    pub infos: Vec<DeliveryInfo>,
    /// `m_PasswordCodesList` — codes littéraux.
    pub password_codes: Vec<PasswordCodes>,
    /// `m_PasswordDataList` — données de mot de passe.
    pub password_data: Vec<PasswordData>,
}

impl DeliveryConfig {
    /// Renvoie la tranche de [`DeliveryContentsData`] d'une livraison.
    ///
    /// Retourne `&[]` si la plage dépasse la liste.
    #[must_use]
    pub fn contents_of(&self, info: &DeliveryInfo) -> &[DeliveryContentsData] {
        let start = info.contents_offset as usize;
        let end = start.saturating_add(info.contents_count as usize);
        self.contents.get(start..end).unwrap_or(&[])
    }
}

/// Parse un `delivery_config_*.cfg.bin.json` complet.
///
/// Vérité terrain : `delivery_config_1.03.63.00.cfg.bin.json` →
/// 73 contenus, 30 infos, 14 codes, 14 données de mot de passe.
#[must_use]
pub fn parse_delivery_config(root: &Value) -> DeliveryConfig {
    DeliveryConfig {
        contents: extract_list(
            root,
            "m_DeliveryContentsDataList",
            DeliveryContentsData::from_value,
        ),
        infos: extract_list(root, "m_DeliveryInfoList", DeliveryInfo::from_value),
        password_codes: extract_list(root, "m_PasswordCodesList", PasswordCodes::from_value),
        password_data: extract_list(root, "m_PasswordDataList", PasswordData::from_value),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  delivery_list_config (format `entries`)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `DELIVERY_INFO_DATA_*` — un contenu d'une livraison (format `entries`, 3 vars).
///
/// Vérité terrain : `delivery_list_config.cfg.bin.json`
/// `DELIVERY_INFO_DATA_0` : `[-979207620, 1, -312536781]`
/// → `item_id_crc = 0xC5A27A3C`, `num = 1`, `sub_id_crc = 0xED5F1133`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeliveryListData {
    /// var\[0\] — hash de l'item.
    pub item_id_crc: HashId,
    /// var\[1\] — quantité.
    pub num: i64,
    /// var\[2\] — hash secondaire (aoc/remplacement ; sémantique non confirmée sans source TS).
    pub sub_id_crc: HashId,
}

impl DeliveryListData {
    /// Parse un noeud `DELIVERY_INFO_DATA_*`.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Self {
        Self {
            item_id_crc: node.hash(0),
            num: node.int(1),
            sub_id_crc: node.hash(2),
        }
    }
}

/// Entrée `DELIVERY_INFO_*` — une livraison (format `entries`, 10 vars positionnelles).
///
/// Les noms officiels des variables ne sont pas disponibles (format `entries`). Les trois
/// premières recoupent la sémantique de `delivery_config` (`idCrc`, `title`, `receivedFlag`) ;
/// var\[3\] est une chaîne `openCond` (base64 opaque ou `"0"`). Le reste est conservé brut.
///
/// Vérité terrain : `DELIVERY_INFO_0` :
/// `[-579676408, 1499432676, -383593584, "0", 1, -1375673200, 0, 1, 1, 0]`
/// → `id_crc = 0xDD72D708`, `title = 0x595F86E4`, `received_flag = 0xE922D390`.
/// `DELIVERY_INFO_19.var[3]` est un base64 (`"AAAAABgFNSo9RUMA..."`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeliveryListInfo {
    /// var\[0\] — identifiant de la livraison.
    pub id_crc: HashId,
    /// var\[1\] — hash du titre.
    pub title: HashId,
    /// var\[2\] — hash du flag « déjà reçu ».
    pub received_flag: HashId,
    /// var\[3\] — condition d'ouverture (base64 opaque, ou `"0"` quand absente).
    pub open_cond: String,
    /// var\[4\..=9\] — six variables positionnelles brutes (sémantique non confirmée).
    pub raw_tail: [i64; 6],
    /// Contenus (`DELIVERY_INFO_DATA_*` du sous-arbre `DELIVERY_INFO_DATA_LIST_BEG_*`).
    pub data: Vec<DeliveryListData>,
}

impl DeliveryListInfo {
    /// Parse un noeud `DELIVERY_INFO_*` (et son sous-arbre de contenus). `None` si `id_crc` nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let id_crc = node.hash(0);
        if id_crc.is_zero() {
            return None;
        }
        let mut raw_tail = [0_i64; 6];
        for (i, slot) in raw_tail.iter_mut().enumerate() {
            *slot = node.int(i + 4);
        }
        let mut data = Vec::new();
        for child in node.children() {
            if child.name().starts_with("DELIVERY_INFO_DATA_LIST_BEG") {
                for d in child.children() {
                    if d.name().starts_with("DELIVERY_INFO_DATA_") {
                        data.push(DeliveryListData::from_node(d));
                    }
                }
            }
        }
        Some(Self {
            id_crc,
            title: node.hash(1),
            received_flag: node.hash(2),
            open_cond: owned(node.string(3)),
            raw_tail,
            data,
        })
    }
}

/// Parse un `delivery_list_config.cfg.bin.json` (format `entries`).
///
/// Renvoie les `DELIVERY_INFO_*` directement sous `DELIVERY_INFO_LIST_BEG_*`, chacun
/// portant ses `DELIVERY_INFO_DATA_*`.
///
/// Vérité terrain : `delivery_list_config.cfg.bin.json` → 20 livraisons, 74 contenus.
#[must_use]
pub fn parse_delivery_list_config(root: &Value) -> Vec<DeliveryListInfo> {
    let mut out = Vec::new();
    let Some(entries) = root.get("entries").and_then(Value::as_array) else {
        return out;
    };
    for e in entries {
        let beg = Node::new(e);
        if !beg.name().starts_with("DELIVERY_INFO_LIST_BEG") {
            continue;
        }
        for info in beg.children() {
            let name = info.name();
            // Exclure les noeuds de sous-liste/contenu : seuls `DELIVERY_INFO_<n>` comptent.
            if !name.starts_with("DELIVERY_INFO_")
                || name.starts_with("DELIVERY_INFO_DATA")
                || name.contains("_LIST_BEG")
            {
                continue;
            }
            if let Some(parsed) = DeliveryListInfo::from_node(info) {
                out.push(parsed);
            }
        }
    }
    out
}

// ══════════════════════════════════════════════════════════════════════════════
//  password_list_config
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `PASSWARD_CODES` (typo Level-5 conservée) — codes hashés par langue.
///
/// Toutes les langues partagent le **même** hash sur le dump réel. La clé `"french "`
/// porte un espace final (typo Level-5 conservée à l'identique).
///
/// Vérité terrain : `password_list_config.cfg.bin.json` `m_Codes[0]` :
/// les 9 langues valent `0xD853DAB8`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PasswardCodes {
    /// `japanese` — hash du code japonais.
    pub japanese: HashId,
    /// `english` — hash du code anglais.
    pub english: HashId,
    /// `portuguese` — hash du code portugais.
    pub portuguese: HashId,
    /// `"french "` (clé avec espace final, typo Level-5) — hash du code français.
    pub french: HashId,
    /// `italian` — hash du code italien.
    pub italian: HashId,
    /// `german` — hash du code allemand.
    pub german: HashId,
    /// `spanish` — hash du code espagnol.
    pub spanish: HashId,
    /// `traditionalChinese` — hash du code chinois traditionnel.
    pub traditional_chinese: HashId,
    /// `simplifiedChinese` — hash du code chinois simplifié.
    pub simplified_chinese: HashId,
}

impl PasswardCodes {
    /// Parse une entrée (toujours `Some`).
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            japanese: field_hash(v, "japanese"),
            english: field_hash(v, "english"),
            portuguese: field_hash(v, "portuguese"),
            // Clé avec espace final, telle quelle dans le dump Level-5.
            french: field_hash(v, "french "),
            italian: field_hash(v, "italian"),
            german: field_hash(v, "german"),
            spanish: field_hash(v, "spanish"),
            traditional_chinese: field_hash(v, "traditionalChinese"),
            simplified_chinese: field_hash(v, "simplifiedChinese"),
        })
    }
}

/// Entrée `PASSWARD_DATA` (typo Level-5 conservée) — données d'un mot de passe.
///
/// `codes = [offset, count]` référence une plage dans la liste `m_Codes`.
///
/// Vérité terrain : `password_list_config.cfg.bin.json` `info[0]` :
/// `id = 0xC4FDCF84`, `text_id = 0xC4FDCF84`, `flag_id = 0xC4FDCF84`,
/// `conditions = "AAAAAA8FNbkZNtoAAQAyAAAAAHg="`, `codes = [0, 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PasswardData {
    /// `id` — identifiant du mot de passe.
    pub id: HashId,
    /// `text_id` — hash du texte associé.
    pub text_id: HashId,
    /// `flag_id` — hash du flag de déblocage.
    pub flag_id: HashId,
    /// `conditions` — condition opaque (base64).
    pub conditions: String,
    /// `codes[0]` — index de début dans `m_Codes`.
    pub codes_offset: i64,
    /// `codes[1]` — nombre de codes.
    pub codes_count: i64,
}

impl PasswardData {
    /// Parse une entrée. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        let (codes_offset, codes_count) = pair(v, "codes");
        Some(Self {
            id,
            text_id: field_hash(v, "text_id"),
            flag_id: field_hash(v, "flag_id"),
            conditions: owned(field_str(v, "conditions").unwrap_or("")),
            codes_offset,
            codes_count,
        })
    }
}

/// Contenu complet d'un `password_list_config.cfg.bin.json` (2 listes).
///
/// Note : les listes s'appellent `m_Codes` (`PASSWARD_CODES`) et `info`
/// (`PASSWARD_DATA`) — les typos `PASSWARD` sont d'origine Level-5.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PasswordListConfig {
    /// `m_Codes` — codes hashés par langue.
    pub codes: Vec<PasswardCodes>,
    /// `info` — données de mot de passe.
    pub data: Vec<PasswardData>,
}

/// Parse un `password_list_config.cfg.bin.json` complet.
///
/// Vérité terrain : `password_list_config.cfg.bin.json` → 1 entrée `m_Codes`, 1 entrée `info`.
#[must_use]
pub fn parse_password_list_config(root: &Value) -> PasswordListConfig {
    PasswordListConfig {
        codes: extract_list(root, "m_Codes", PasswardCodes::from_value),
        data: extract_list(root, "info", PasswardData::from_value),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  post_notice_config
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `POST_NOTICE_BANNER_IMG_INFO` — composition d'une bannière (fond + badge + icône).
///
/// Vérité terrain : `post_notice_config_1.03.93.00.cfg.bin.json`
/// `m_PostNoticeBannerImgInfoList[0]` : `id_crc = 0xEA8A163E`,
/// `banner_bg_id_crc = 0x4E6EE6A6`, `banner_badge_id_crc = 0x73FB83C5`,
/// `banner_icon_id_crc = 0x2B4FECA6`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PostNoticeBannerImgInfo {
    /// `id_crc` — identifiant de la composition.
    pub id_crc: HashId,
    /// `banner_bg_id_crc` — référence vers un `POST_NOTICE_BANNER_BG_INFO`.
    pub banner_bg_id_crc: HashId,
    /// `banner_badge_id_crc` — référence vers un `POST_NOTICE_BANNER_BADGE_INFO`.
    pub banner_badge_id_crc: HashId,
    /// `banner_icon_id_crc` — référence vers un `POST_NOTICE_BANNER_ICON_INFO`.
    pub banner_icon_id_crc: HashId,
}

impl PostNoticeBannerImgInfo {
    /// Parse une entrée. `None` si `id_crc` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id_crc = field_hash(v, "id_crc");
        if id_crc.is_zero() {
            return None;
        }
        Some(Self {
            id_crc,
            banner_bg_id_crc: field_hash(v, "banner_bg_id_crc"),
            banner_badge_id_crc: field_hash(v, "banner_badge_id_crc"),
            banner_icon_id_crc: field_hash(v, "banner_icon_id_crc"),
        })
    }
}

/// Entrée `POST_NOTICE_BANNER_BG_INFO` — texture de fond de bannière.
///
/// Vérité terrain : `m_PostNoticeBannerBgInfoList[0]` : `id_crc = 0x4E6EE6A6`,
/// `banner_bg_texture_name_crc = 0x60FFD0B3`,
/// `banner_bg_texture_path = "#/menu/220_img/banner_img/banner01_0001.g4tx"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PostNoticeBannerBgInfo {
    /// `id_crc` — identifiant du fond.
    pub id_crc: HashId,
    /// `banner_bg_texture_name_crc` — hash du nom interne de la texture.
    pub banner_bg_texture_name_crc: HashId,
    /// `banner_bg_texture_path_crc` — hash du chemin de la texture.
    pub banner_bg_texture_path_crc: HashId,
    /// `banner_bg_texture_path` — chemin VFS de la texture (`.g4tx`).
    pub banner_bg_texture_path: String,
}

impl PostNoticeBannerBgInfo {
    /// Parse une entrée. `None` si `id_crc` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id_crc = field_hash(v, "id_crc");
        if id_crc.is_zero() {
            return None;
        }
        Some(Self {
            id_crc,
            banner_bg_texture_name_crc: field_hash(v, "banner_bg_texture_name_crc"),
            banner_bg_texture_path_crc: field_hash(v, "banner_bg_texture_path_crc"),
            banner_bg_texture_path: owned(field_str(v, "banner_bg_texture_path").unwrap_or("")),
        })
    }
}

/// Entrée `POST_NOTICE_BANNER_BADGE_INFO` — texture de badge de bannière.
///
/// Vérité terrain : `m_PostNoticeBannerBadgeInfoList[0]` : `id_crc = 0x73FB83C5`,
/// `banner_badge_texture_name_crc = 0xE586163F`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PostNoticeBannerBadgeInfo {
    /// `id_crc` — identifiant du badge.
    pub id_crc: HashId,
    /// `banner_badge_texture_name_crc` — hash du nom interne de la texture.
    pub banner_badge_texture_name_crc: HashId,
}

impl PostNoticeBannerBadgeInfo {
    /// Parse une entrée. `None` si `id_crc` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id_crc = field_hash(v, "id_crc");
        if id_crc.is_zero() {
            return None;
        }
        Some(Self {
            id_crc,
            banner_badge_texture_name_crc: field_hash(v, "banner_badge_texture_name_crc"),
        })
    }
}

/// Entrée `POST_NOTICE_BANNER_ICON_INFO` — texture d'icône de bannière.
///
/// Vérité terrain : `m_PostNoticeBannerIconInfoList[0]` : `id_crc = 0x2B4FECA6`,
/// `banner_icon_texture_name_crc = 0x24DEC470`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PostNoticeBannerIconInfo {
    /// `id_crc` — identifiant de l'icône.
    pub id_crc: HashId,
    /// `banner_icon_texture_name_crc` — hash du nom interne de la texture.
    pub banner_icon_texture_name_crc: HashId,
}

impl PostNoticeBannerIconInfo {
    /// Parse une entrée. `None` si `id_crc` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id_crc = field_hash(v, "id_crc");
        if id_crc.is_zero() {
            return None;
        }
        Some(Self {
            id_crc,
            banner_icon_texture_name_crc: field_hash(v, "banner_icon_texture_name_crc"),
        })
    }
}

/// Entrée `POST_NOTICE_BANNER_GRAPHICS_TEXT_INFO` — texture de texte graphique de bannière.
///
/// La dernière entrée du dump porte un chemin marqueur opaque (`"\u{FFFD}POST_NOTICE_INFO"`),
/// artefact de décodage conservé tel quel.
///
/// Vérité terrain : `m_PostNoticeBannerGraphicsTextInfo[0]` : `id_crc = 0x2B7725CF`,
/// `banner_graphics_text_texture_path = "#/menu/220_img/banner_img/banner02_0001.g4tx"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PostNoticeBannerGraphicsTextInfo {
    /// `id_crc` — identifiant du texte graphique.
    pub id_crc: HashId,
    /// `banner_graphics_text_texture_name_crc` — hash du nom interne de la texture.
    pub banner_graphics_text_texture_name_crc: HashId,
    /// `banner_graphics_text_texture_path_crc` — hash du chemin de la texture.
    pub banner_graphics_text_texture_path_crc: HashId,
    /// `banner_graphics_text_texture_path` — chemin VFS de la texture (peut contenir `<LG>`).
    pub banner_graphics_text_texture_path: String,
}

impl PostNoticeBannerGraphicsTextInfo {
    /// Parse une entrée. `None` si `id_crc` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id_crc = field_hash(v, "id_crc");
        if id_crc.is_zero() {
            return None;
        }
        Some(Self {
            id_crc,
            banner_graphics_text_texture_name_crc: field_hash(
                v,
                "banner_graphics_text_texture_name_crc",
            ),
            banner_graphics_text_texture_path_crc: field_hash(
                v,
                "banner_graphics_text_texture_path_crc",
            ),
            banner_graphics_text_texture_path: owned(
                field_str(v, "banner_graphics_text_texture_path").unwrap_or(""),
            ),
        })
    }
}

/// Entrée `POST_NOTICE_INFO` — une annonce affichée dans la boîte aux lettres.
///
/// Vérité terrain : `m_PostNoticeInfoList[0]` : `id_crc = 0x3A549FEC`, `flag_no = 19`,
/// `is_advance_notice = false`, `banner_img_id_crc = 0x7D86C4C6`, `is_use_utc = true`,
/// `valid_cond = "POST_NOTICE_INFO"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PostNoticeInfo {
    /// `id_crc` — identifiant de l'annonce.
    pub id_crc: HashId,
    /// `flag_no` — numéro de flag de progression.
    pub flag_no: i64,
    /// `is_advance_notice` — `true` si c'est un avis préalable.
    pub is_advance_notice: bool,
    /// `banner_img_id_crc` — référence vers un `POST_NOTICE_BANNER_IMG_INFO`.
    pub banner_img_id_crc: HashId,
    /// `banner_graphics_text_id_crc` — référence vers un `POST_NOTICE_BANNER_GRAPHICS_TEXT_INFO`.
    pub banner_graphics_text_id_crc: HashId,
    /// `banner_title_text_id_crc` — hash du titre de la bannière.
    pub banner_title_text_id_crc: HashId,
    /// `banner_title_text_font_style_type` — style de police du titre.
    pub banner_title_text_font_style_type: i64,
    /// `banner_overview_two_line_text_id_crc` — hash du résumé sur deux lignes.
    pub banner_overview_two_line_text_id_crc: HashId,
    /// `banner_overview_two_line_text_font_style_type` — style de police du résumé deux lignes.
    pub banner_overview_two_line_text_font_style_type: i64,
    /// `banner_overview_one_line_text_id_crc` — hash du résumé sur une ligne.
    pub banner_overview_one_line_text_id_crc: HashId,
    /// `banner_overview_one_line_text_font_style_type` — style de police du résumé une ligne.
    pub banner_overview_one_line_text_font_style_type: i64,
    /// `detail_window_title_txt_id_crc` — hash du titre de la fenêtre de détail.
    pub detail_window_title_txt_id_crc: HashId,
    /// `detail_window_main_txt_id_crc` — hash du corps de la fenêtre de détail.
    pub detail_window_main_txt_id_crc: HashId,
    /// `is_use_utc` — `true` si les bornes temporelles sont en UTC.
    pub is_use_utc: bool,
    /// `start_enable_time` — borne d'activation de début (0 ou année).
    pub start_enable_time: i64,
    /// `end_enable_time` — borne d'activation de fin (0 ou mois).
    pub end_enable_time: i64,
    /// `valid_cond` — condition de validité opaque (fragment de `typeName` sur le dump réel).
    pub valid_cond: String,
}

impl PostNoticeInfo {
    /// Parse une entrée. `None` si `id_crc` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id_crc = field_hash(v, "id_crc");
        if id_crc.is_zero() {
            return None;
        }
        Some(Self {
            id_crc,
            flag_no: field_i64(v, "flag_no").unwrap_or(0),
            is_advance_notice: field_bool(v, "is_advance_notice").unwrap_or(false),
            banner_img_id_crc: field_hash(v, "banner_img_id_crc"),
            banner_graphics_text_id_crc: field_hash(v, "banner_graphics_text_id_crc"),
            banner_title_text_id_crc: field_hash(v, "banner_title_text_id_crc"),
            banner_title_text_font_style_type: field_i64(v, "banner_title_text_font_style_type")
                .unwrap_or(0),
            banner_overview_two_line_text_id_crc: field_hash(
                v,
                "banner_overview_two_line_text_id_crc",
            ),
            banner_overview_two_line_text_font_style_type: field_i64(
                v,
                "banner_overview_two_line_text_font_style_type",
            )
            .unwrap_or(0),
            banner_overview_one_line_text_id_crc: field_hash(
                v,
                "banner_overview_one_line_text_id_crc",
            ),
            banner_overview_one_line_text_font_style_type: field_i64(
                v,
                "banner_overview_one_line_text_font_style_type",
            )
            .unwrap_or(0),
            detail_window_title_txt_id_crc: field_hash(v, "detail_window_title_txt_id_crc"),
            detail_window_main_txt_id_crc: field_hash(v, "detail_window_main_txt_id_crc"),
            is_use_utc: field_bool(v, "is_use_utc").unwrap_or(false),
            start_enable_time: field_i64(v, "start_enable_time").unwrap_or(0),
            end_enable_time: field_i64(v, "end_enable_time").unwrap_or(0),
            valid_cond: owned(field_str(v, "valid_cond").unwrap_or("")),
        })
    }
}

/// Contenu complet d'un `post_notice_config_*.cfg.bin.json` (6 listes).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PostNoticeConfig {
    /// `m_PostNoticeBannerImgInfoList` — compositions de bannières.
    pub banner_imgs: Vec<PostNoticeBannerImgInfo>,
    /// `m_PostNoticeBannerBgInfoList` — fonds de bannières.
    pub banner_bgs: Vec<PostNoticeBannerBgInfo>,
    /// `m_PostNoticeBannerBadgeInfoList` — badges de bannières.
    pub banner_badges: Vec<PostNoticeBannerBadgeInfo>,
    /// `m_PostNoticeBannerIconInfoList` — icônes de bannières.
    pub banner_icons: Vec<PostNoticeBannerIconInfo>,
    /// `m_PostNoticeBannerGraphicsTextInfo` — textes graphiques de bannières.
    pub banner_graphics_texts: Vec<PostNoticeBannerGraphicsTextInfo>,
    /// `m_PostNoticeInfoList` — annonces.
    pub infos: Vec<PostNoticeInfo>,
}

/// Parse un `post_notice_config_*.cfg.bin.json` complet.
///
/// Vérité terrain : `post_notice_config_1.03.93.00.cfg.bin.json` →
/// 44 compositions, 16 fonds, 3 badges, 9 icônes, 8 textes graphiques, 9 annonces.
#[must_use]
pub fn parse_post_notice_config(root: &Value) -> PostNoticeConfig {
    PostNoticeConfig {
        banner_imgs: extract_list(
            root,
            "m_PostNoticeBannerImgInfoList",
            PostNoticeBannerImgInfo::from_value,
        ),
        banner_bgs: extract_list(
            root,
            "m_PostNoticeBannerBgInfoList",
            PostNoticeBannerBgInfo::from_value,
        ),
        banner_badges: extract_list(
            root,
            "m_PostNoticeBannerBadgeInfoList",
            PostNoticeBannerBadgeInfo::from_value,
        ),
        banner_icons: extract_list(
            root,
            "m_PostNoticeBannerIconInfoList",
            PostNoticeBannerIconInfo::from_value,
        ),
        banner_graphics_texts: extract_list(
            root,
            "m_PostNoticeBannerGraphicsTextInfo",
            PostNoticeBannerGraphicsTextInfo::from_value,
        ),
        infos: extract_list(root, "m_PostNoticeInfoList", PostNoticeInfo::from_value),
    }
}
