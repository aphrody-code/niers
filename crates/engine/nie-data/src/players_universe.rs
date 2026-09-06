//! Famille `players_universe` — port Rust des `*.cfg.bin.json` du « Players Universe »
//! (l'écran constellation / gacha de personnages d'IEVR).
//!
//! ## Vérité terrain
//!
//! Deux fichiers dans `data/common/gamedata/players_universe/` :
//!
//! | Fichier | Layout | Listes |
//! |---------|--------|--------|
//! | `players_universe_config_1.03.59.00.cfg.bin.json` | `lists` | 10 listes (univers + étoiles + signes + raretés) |
//! | `players_universe_event_config.cfg.bin.json` | `lists` | 1 liste (effets de résultat) |
//!
//! Parseurs TS de référence (port 1:1) :
//! - `packages/inagle/src/parsers/constellation.ts` (`StarInfo`, `StarSignInfoEntry`,
//!   `StarSignRarityRateInfo` — champs `*NameHash` + paires `[offset, count]`) ;
//! - `packages/inagle/src/parsers/star-sign.ts` (`StarSignCharaInfo` — `charaParamId`,
//!   `charaRarity`, `charaRate{Default,BoostA..D}`, `isRemarkable`, `enableCond`).
//!
//! ## Paires index `[offset, count]`
//!
//! Plusieurs champs sont des paires `[offset, count]` référençant une plage dans une
//! autre liste (même convention que `post`/`search_word`). La liaison offset/count est
//! de la **logique applicative** : le parseur lit juste les lignes telles quelles, et
//! des accesseurs (`star_keys_of`, `charas_of`, `rates_of`, `star_signs_of`) résolvent
//! les tranches à la demande. Contiguïté vérifiée sur le dump réel (raretés
//! `[0,4] [4,5] [9,181] …` → personnages `0..190`).
//!
//! ## Zones d'ombre honnêtes
//!
//! - `enable_cond` : chaîne opaque. Sur le dump réel elle vaut `"0xFFFFFFFF"` (toujours
//!   actif) ou un base64 de condition (`"AAAAABgFNSo9RUMA…"`). Conservée telle quelle,
//!   sans interprétation (1:1 avec le type `string` d'inagle).
//! - Les listes sont lues **intégralement** (aucune ligne sautée), comme inagle qui
//!   mappe le tableau `values` brut. Les comptes correspondent donc exactement au dump.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_str, list_values, owned};
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

/// Lit un champ flottant d'un objet `values[]` (les `Float` RDBN arrivent en `160.0`).
fn field_f64(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Parcourt une liste nommée `lists[].values` et mappe **chaque** ligne via `f`.
///
/// Aucune ligne n'est sautée : le compte de la `Vec` retournée égale celui du dump
/// (port 1:1 d'inagle qui mappe le tableau `values` brut).
fn map_list<T, F>(root: &Value, name: &str, f: F) -> Vec<T>
where
    F: Fn(&Value) -> T,
{
    let mut out = Vec::new();
    if let Some(values) = list_values(root, name) {
        out.reserve(values.len());
        for v in values {
            out.push(f(v));
        }
    }
    out
}

/// Résout une tranche `[offset, count]` dans une liste cible (`&[]` si hors bornes).
fn slice_of<T>(items: &[T], offset: i64, count: i64) -> &[T] {
    let start = offset.max(0) as usize;
    let end = start.saturating_add(count.max(0) as usize);
    items.get(start..end).unwrap_or(&[])
}

// ══════════════════════════════════════════════════════════════════════════════
//  m_universeInfoList — UNIVERSE_INFO (1)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `UNIVERSE_INFO` — métadonnées de l'univers (un seul élément sur le dump).
///
/// Vérité terrain : `m_universeInfoList[0]` : `universeIdCrc = 0xC26FDF3D`,
/// `universeNameId = 0x35101955`, `locatorNameHash = 0x3752BB92`,
/// `enableCond = "0xFFFFFFFF"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UniverseInfo {
    /// `universeIdCrc` — identifiant de l'univers.
    pub universe_id_crc: HashId,
    /// `universeNameId` — hash du nom localisé.
    pub universe_name_id: HashId,
    /// `universeInfoTextId` — hash du texte d'information.
    pub universe_info_text_id: HashId,
    /// `universeLockTextId` — hash du texte « verrouillé ».
    pub universe_lock_text_id: HashId,
    /// `locatorNameHash` — hash du locateur de scène.
    pub locator_name_hash: HashId,
    /// `enableCond` — condition d'activation opaque (`"0xFFFFFFFF"` ou base64).
    pub enable_cond: String,
}

impl UniverseInfo {
    /// Parse une entrée (lit toutes les lignes, comme inagle).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            universe_id_crc: field_hash(v, "universeIdCrc"),
            universe_name_id: field_hash(v, "universeNameId"),
            universe_info_text_id: field_hash(v, "universeInfoTextId"),
            universe_lock_text_id: field_hash(v, "universeLockTextId"),
            locator_name_hash: field_hash(v, "locatorNameHash"),
            enable_cond: owned(field_str(v, "enableCond").unwrap_or("")),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  m_starKeyRouteInfoList — STAR_KEY_ROUTE_INFO (240)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `STAR_KEY_ROUTE_INFO` — graphe de navigation entre clés d'étoile (haut/bas/
/// gauche/droite). Un hash de lien nul (`0x00000000`) = pas de voisin dans cette direction.
///
/// Vérité terrain : `m_starKeyRouteInfoList[0]` : `starKeyNameHash = 0x011F09E8`,
/// `…LinkUp = 0x98165852`, `…LinkDown = 0x0672CDF1`, `…LinkRight = 0x9F7B9C4B`,
/// `…LinkLeft = 0x00000000`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StarKeyRouteInfo {
    /// `starKeyNameHash` — clé d'étoile de départ.
    pub star_key_name_hash: HashId,
    /// `starKeyNameHashLinkUp` — voisin du haut (0 si aucun).
    pub link_up: HashId,
    /// `starKeyNameHashLinkDown` — voisin du bas (0 si aucun).
    pub link_down: HashId,
    /// `starKeyNameHashLinkLeft` — voisin de gauche (0 si aucun).
    pub link_left: HashId,
    /// `starKeyNameHashLinkRight` — voisin de droite (0 si aucun).
    pub link_right: HashId,
}

impl StarKeyRouteInfo {
    /// Parse une entrée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            star_key_name_hash: field_hash(v, "starKeyNameHash"),
            link_up: field_hash(v, "starKeyNameHashLinkUp"),
            link_down: field_hash(v, "starKeyNameHashLinkDown"),
            link_left: field_hash(v, "starKeyNameHashLinkLeft"),
            link_right: field_hash(v, "starKeyNameHashLinkRight"),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  m_starInfoList — STAR_INFO (30)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `STAR_INFO` — une étoile (constellation) : textures + plage de clés d'étoile.
///
/// Port 1:1 de l'interface `StarInfo` d'inagle (`constellation.ts` l.14-25).
/// `starKeyInfoList = [offset, count]` référence une plage dans `m_starKeyInfoList`.
///
/// Vérité terrain : `m_starInfoList[0]` : `starNameHash = 0x193E6BEA`,
/// `starTextureNameHash = 0xFE773B4A`, `starKeyInfoList = [0, 8]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StarInfo {
    /// `starNameHash` — hash identifiant l'étoile (recoupe `StarSignInfo.starKeyNameHash`).
    pub star_name_hash: HashId,
    /// `starLocatorNameHash` — hash du locateur de scène.
    pub star_locator_name_hash: HashId,
    /// `starTextureNameHash` — hash de la texture de l'étoile.
    pub star_texture_name_hash: HashId,
    /// `starAfterTextureNameHash` — hash de la texture « après déblocage ».
    pub star_after_texture_name_hash: HashId,
    /// `rareStarTextureNameHash` — hash de la texture d'étoile rare.
    pub rare_star_texture_name_hash: HashId,
    /// `starSignLayerNameHash` — hash du layer du signe.
    pub star_sign_layer_name_hash: HashId,
    /// `starLayerNameHash` — hash du layer de l'étoile.
    pub star_layer_name_hash: HashId,
    /// `numTextureNameHash` — hash de la texture des numéros.
    pub num_texture_name_hash: HashId,
    /// `starKeyInfoList[0]` — index de début dans `m_starKeyInfoList`.
    pub star_key_offset: i64,
    /// `starKeyInfoList[1]` — nombre de clés d'étoile.
    pub star_key_count: i64,
}

impl StarInfo {
    /// Parse une entrée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        let (star_key_offset, star_key_count) = pair(v, "starKeyInfoList");
        Self {
            star_name_hash: field_hash(v, "starNameHash"),
            star_locator_name_hash: field_hash(v, "starLocatorNameHash"),
            star_texture_name_hash: field_hash(v, "starTextureNameHash"),
            star_after_texture_name_hash: field_hash(v, "starAfterTextureNameHash"),
            rare_star_texture_name_hash: field_hash(v, "rareStarTextureNameHash"),
            star_sign_layer_name_hash: field_hash(v, "starSignLayerNameHash"),
            star_layer_name_hash: field_hash(v, "starLayerNameHash"),
            num_texture_name_hash: field_hash(v, "numTextureNameHash"),
            star_key_offset,
            star_key_count,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  m_starKeyInfoList — STAR_KEY_INFO (240)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `STAR_KEY_INFO` — une clé d'étoile (case de la grille de tirage).
///
/// Vérité terrain : `m_starKeyInfoList[0]` : `starKeyNameHash = 0x011F09E8`,
/// `starKeyLocatorNameHash = 0x43C1786E`, `flagIndex = 0`, `isRarePackStar = false`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StarKeyInfo {
    /// `starKeyNameHash` — identifiant de la clé d'étoile.
    pub star_key_name_hash: HashId,
    /// `starKeyLocatorNameHash` — hash du locateur de scène.
    pub star_key_locator_name_hash: HashId,
    /// `lotteryStarSignIdCrc` — signe tiré au sort (0 si aucun).
    pub lottery_star_sign_id_crc: HashId,
    /// `winContentTextureNameHash` — texture du contenu gagné (0 si aucune).
    pub win_content_texture_name_hash: HashId,
    /// `flagIndex` — index de flag de progression.
    pub flag_index: i64,
    /// `isRarePackStar` — `true` si c'est une étoile de pack rare.
    pub is_rare_pack_star: bool,
}

impl StarKeyInfo {
    /// Parse une entrée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            star_key_name_hash: field_hash(v, "starKeyNameHash"),
            star_key_locator_name_hash: field_hash(v, "starKeyLocatorNameHash"),
            lottery_star_sign_id_crc: field_hash(v, "lotteryStarSignIdCrc"),
            win_content_texture_name_hash: field_hash(v, "winContentTextureNameHash"),
            flag_index: field_i64(v, "flagIndex").unwrap_or(0),
            is_rare_pack_star: field_bool(v, "isRarePackStar").unwrap_or(false),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  m_universeStarSignInfoList — UNIVERSE_STAR_SIGN_INFO (30)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `UNIVERSE_STAR_SIGN_INFO` — référence d'un signe appartenant à l'univers.
///
/// Vérité terrain : `m_universeStarSignInfoList[0]` : `starSignIdCrc = 0xCED5BE45`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UniverseStarSignInfo {
    /// `starSignIdCrc` — identifiant du signe (recoupe `StarSignInfo.starSignIdCrc`).
    pub star_sign_id_crc: HashId,
}

impl UniverseStarSignInfo {
    /// Parse une entrée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            star_sign_id_crc: field_hash(v, "starSignIdCrc"),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  m_universeStarSignSetDataList — UNIVERSE_STAR_SIGN_SET_DATA (1)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `UNIVERSE_STAR_SIGN_SET_DATA` — associe un univers à sa plage de signes.
///
/// `universeStarSignInfoList = [offset, count]` → plage dans `m_universeStarSignInfoList`.
///
/// Vérité terrain : `m_universeStarSignSetDataList[0]` : `universeIdCrc = 0xC26FDF3D`,
/// `universeStarSignInfoList = [0, 30]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UniverseStarSignSetData {
    /// `universeIdCrc` — identifiant de l'univers.
    pub universe_id_crc: HashId,
    /// `universeStarSignInfoList[0]` — index de début dans `m_universeStarSignInfoList`.
    pub star_sign_offset: i64,
    /// `universeStarSignInfoList[1]` — nombre de signes.
    pub star_sign_count: i64,
}

impl UniverseStarSignSetData {
    /// Parse une entrée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        let (star_sign_offset, star_sign_count) = pair(v, "universeStarSignInfoList");
        Self {
            universe_id_crc: field_hash(v, "universeIdCrc"),
            star_sign_offset,
            star_sign_count,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  m_starSignInfoList — STAR_SIGN_INFO (30)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `STAR_SIGN_INFO` — un signe (constellation) : nom, item-clé, conditions.
///
/// Port 1:1 de `StarSignInfoEntry` d'inagle (`constellation.ts` l.126-137).
///
/// Vérité terrain : `m_starSignInfoList[0]` : `starSignIdCrc = 0xCED5BE45`,
/// `starSignNameId = 0xE0C8FB97`, `starSignNo = 1`, `keyItemId = 0x64EB1725`,
/// `keyItemNum = 5`, `dropCharacterNum = 12`, `starKeyNameHash = 0x193E6BEA`,
/// `clearFlagIndex = 0`, `enableCond = "0xFFFFFFFF"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StarSignInfo {
    /// `starSignIdCrc` — identifiant du signe.
    pub star_sign_id_crc: HashId,
    /// `starSignNameId` — hash du nom localisé du signe.
    pub star_sign_name_id: HashId,
    /// `starSignInfoTextId` — hash du texte d'information.
    pub star_sign_info_text_id: HashId,
    /// `starSignNo` — numéro d'ordre (1-basé).
    pub star_sign_no: i64,
    /// `keyItemId` — hash de l'item-clé requis.
    pub key_item_id: HashId,
    /// `keyItemNum` — quantité d'item-clé requise.
    pub key_item_num: i64,
    /// `dropCharacterNum` — nombre de personnages tirables.
    pub drop_character_num: i64,
    /// `starKeyNameHash` — hash de la clé (recoupe `StarInfo.starNameHash`).
    pub star_key_name_hash: HashId,
    /// `clearFlagIndex` — index de flag « terminé ».
    pub clear_flag_index: i64,
    /// `enableCond` — condition d'activation opaque (`"0xFFFFFFFF"` ou base64).
    pub enable_cond: String,
}

impl StarSignInfo {
    /// Parse une entrée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            star_sign_id_crc: field_hash(v, "starSignIdCrc"),
            star_sign_name_id: field_hash(v, "starSignNameId"),
            star_sign_info_text_id: field_hash(v, "starSignInfoTextId"),
            star_sign_no: field_i64(v, "starSignNo").unwrap_or(0),
            key_item_id: field_hash(v, "keyItemId"),
            key_item_num: field_i64(v, "keyItemNum").unwrap_or(0),
            drop_character_num: field_i64(v, "dropCharacterNum").unwrap_or(0),
            star_key_name_hash: field_hash(v, "starKeyNameHash"),
            clear_flag_index: field_i64(v, "clearFlagIndex").unwrap_or(0),
            enable_cond: owned(field_str(v, "enableCond").unwrap_or("")),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  m_starSignCharaInfoList — STAR_SIGN_CHARA_INFO (5010)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `STAR_SIGN_CHARA_INFO` — un personnage tirable + ses taux par boost.
///
/// Port 1:1 de `StarSignCharaInfo` d'inagle (`star-sign.ts` l.23-33). Le parseur lit
/// juste la ligne ; la liaison offset/count (depuis `StarSignRarityRateInfo`) est de la
/// logique applicative (cf. [`PlayersUniverseConfig::charas_of`]).
///
/// Vérité terrain : `m_starSignCharaInfoList[0]` : `charaParamId = 0xDCD5DB61`,
/// `charaRarity = 7`, `charaRateDefault = 1`, `isRemarkable = true`,
/// `enableCond = "0xFFFFFFFF"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StarSignCharaInfo {
    /// `charaParamId` — hash du personnage (`charaParamId` du chara_param).
    pub chara_param_id: HashId,
    /// `charaRarity` — code de rareté (0=Normal, 2=R, 5=UR, 6=LR, 7=Legend).
    pub chara_rarity: i64,
    /// `charaRateDefault` — poids de tirage par défaut.
    pub chara_rate_default: i64,
    /// `charaRateBoostA` — poids de tirage avec boost A.
    pub chara_rate_boost_a: i64,
    /// `charaRateBoostB` — poids de tirage avec boost B.
    pub chara_rate_boost_b: i64,
    /// `charaRateBoostC` — poids de tirage avec boost C.
    pub chara_rate_boost_c: i64,
    /// `charaRateBoostD` — poids de tirage avec boost D.
    pub chara_rate_boost_d: i64,
    /// `isRemarkable` — `true` si le personnage est mis en avant (featured).
    pub is_remarkable: bool,
    /// `enableCond` — condition d'activation opaque (`"0xFFFFFFFF"` ou base64).
    pub enable_cond: String,
}

impl StarSignCharaInfo {
    /// Parse une entrée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            chara_param_id: field_hash(v, "charaParamId"),
            chara_rarity: field_i64(v, "charaRarity").unwrap_or(0),
            chara_rate_default: field_i64(v, "charaRateDefault").unwrap_or(0),
            chara_rate_boost_a: field_i64(v, "charaRateBoostA").unwrap_or(0),
            chara_rate_boost_b: field_i64(v, "charaRateBoostB").unwrap_or(0),
            chara_rate_boost_c: field_i64(v, "charaRateBoostC").unwrap_or(0),
            chara_rate_boost_d: field_i64(v, "charaRateBoostD").unwrap_or(0),
            is_remarkable: field_bool(v, "isRemarkable").unwrap_or(false),
            enable_cond: owned(field_str(v, "enableCond").unwrap_or("")),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  m_starSignRarityRateInfoList — STAR_SIGN_RARITY_RATE_INFO (90)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `STAR_SIGN_RARITY_RATE_INFO` — taux d'une rareté + plage de personnages.
///
/// Port 1:1 de `StarSignRarityRateInfo` d'inagle (`constellation.ts` l.28-36).
/// `starSignCharaInfoList = [offset, count]` → plage dans `m_starSignCharaInfoList`.
///
/// Vérité terrain : `m_starSignRarityRateInfoList[0]` : `rarityType = 2`,
/// `rarityRateDefault = 1`, `starSignCharaInfoList = [0, 4]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StarSignRarityRateInfo {
    /// `rarityType` — type de rareté de ce groupe.
    pub rarity_type: i64,
    /// `rarityRateDefault` — poids de tirage par défaut de la rareté.
    pub rarity_rate_default: i64,
    /// `rarityRateBoostA` — poids avec boost A.
    pub rarity_rate_boost_a: i64,
    /// `rarityRateBoostB` — poids avec boost B.
    pub rarity_rate_boost_b: i64,
    /// `rarityRateBoostC` — poids avec boost C.
    pub rarity_rate_boost_c: i64,
    /// `rarityRateBoostD` — poids avec boost D.
    pub rarity_rate_boost_d: i64,
    /// `starSignCharaInfoList[0]` — index de début dans `m_starSignCharaInfoList`.
    pub chara_offset: i64,
    /// `starSignCharaInfoList[1]` — nombre de personnages.
    pub chara_count: i64,
}

impl StarSignRarityRateInfo {
    /// Parse une entrée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        let (chara_offset, chara_count) = pair(v, "starSignCharaInfoList");
        Self {
            rarity_type: field_i64(v, "rarityType").unwrap_or(0),
            rarity_rate_default: field_i64(v, "rarityRateDefault").unwrap_or(0),
            rarity_rate_boost_a: field_i64(v, "rarityRateBoostA").unwrap_or(0),
            rarity_rate_boost_b: field_i64(v, "rarityRateBoostB").unwrap_or(0),
            rarity_rate_boost_c: field_i64(v, "rarityRateBoostC").unwrap_or(0),
            rarity_rate_boost_d: field_i64(v, "rarityRateBoostD").unwrap_or(0),
            chara_offset,
            chara_count,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  m_starSignCharaSetDataList — STAR_SIGN_CHARA_SET_DATA (30)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `STAR_SIGN_CHARA_SET_DATA` — associe un signe à sa plage de raretés.
///
/// `starSignRarityRateInfoList = [offset, count]` → plage dans `m_starSignRarityRateInfoList`
/// (3 raretés par signe sur le dump réel : `[0,3] [3,3] [6,3] …`).
///
/// Vérité terrain : `m_starSignCharaSetDataList[0]` : `starSignIdCrc = 0xCED5BE45`,
/// `starSignRarityRateInfoList = [0, 3]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StarSignCharaSetData {
    /// `starSignIdCrc` — identifiant du signe.
    pub star_sign_id_crc: HashId,
    /// `starSignRarityRateInfoList[0]` — index de début dans `m_starSignRarityRateInfoList`.
    pub rate_offset: i64,
    /// `starSignRarityRateInfoList[1]` — nombre de raretés.
    pub rate_count: i64,
}

impl StarSignCharaSetData {
    /// Parse une entrée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        let (rate_offset, rate_count) = pair(v, "starSignRarityRateInfoList");
        Self {
            star_sign_id_crc: field_hash(v, "starSignIdCrc"),
            rate_offset,
            rate_count,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  players_universe_config (agrégat des 10 listes)
// ══════════════════════════════════════════════════════════════════════════════

/// Contenu complet d'un `players_universe_config_*.cfg.bin.json` (10 listes).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayersUniverseConfig {
    /// `m_universeInfoList` — métadonnées de l'univers (1 entrée).
    pub universe_info: Vec<UniverseInfo>,
    /// `m_starKeyRouteInfoList` — graphe de navigation (240 entrées).
    pub star_key_routes: Vec<StarKeyRouteInfo>,
    /// `m_starInfoList` — étoiles/constellations (30 entrées).
    pub stars: Vec<StarInfo>,
    /// `m_starKeyInfoList` — clés d'étoile (240 entrées).
    pub star_keys: Vec<StarKeyInfo>,
    /// `m_universeStarSignInfoList` — références de signes de l'univers (30 entrées).
    pub universe_star_signs: Vec<UniverseStarSignInfo>,
    /// `m_universeStarSignSetDataList` — set univers→signes (1 entrée).
    pub universe_star_sign_sets: Vec<UniverseStarSignSetData>,
    /// `m_starSignInfoList` — signes/constellations (30 entrées).
    pub star_signs: Vec<StarSignInfo>,
    /// `m_starSignCharaInfoList` — personnages tirables (5010 entrées dans le .json de référence).
    pub star_sign_charas: Vec<StarSignCharaInfo>,
    /// `m_starSignRarityRateInfoList` — taux par rareté (90 entrées).
    pub star_sign_rarity_rates: Vec<StarSignRarityRateInfo>,
    /// `m_starSignCharaSetDataList` — set signe→raretés (30 entrées).
    pub star_sign_chara_sets: Vec<StarSignCharaSetData>,
}

impl PlayersUniverseConfig {
    /// Tranche des [`StarKeyInfo`] d'une étoile (`starKeyInfoList` = `[offset, count]`).
    #[must_use]
    pub fn star_keys_of(&self, star: &StarInfo) -> &[StarKeyInfo] {
        slice_of(&self.star_keys, star.star_key_offset, star.star_key_count)
    }

    /// Tranche des [`StarSignCharaInfo`] d'une rareté (`starSignCharaInfoList` = `[offset, count]`).
    #[must_use]
    pub fn charas_of(&self, rate: &StarSignRarityRateInfo) -> &[StarSignCharaInfo] {
        slice_of(&self.star_sign_charas, rate.chara_offset, rate.chara_count)
    }

    /// Tranche des [`StarSignRarityRateInfo`] d'un signe (`starSignRarityRateInfoList`).
    #[must_use]
    pub fn rates_of(&self, set: &StarSignCharaSetData) -> &[StarSignRarityRateInfo] {
        slice_of(
            &self.star_sign_rarity_rates,
            set.rate_offset,
            set.rate_count,
        )
    }

    /// Tranche des [`UniverseStarSignInfo`] d'un univers (`universeStarSignInfoList`).
    #[must_use]
    pub fn star_signs_of(&self, set: &UniverseStarSignSetData) -> &[UniverseStarSignInfo] {
        slice_of(
            &self.universe_star_signs,
            set.star_sign_offset,
            set.star_sign_count,
        )
    }
}

/// Parse un `players_universe_config_*.cfg.bin.json` complet (10 listes).
///
/// Vérité terrain : `players_universe_config_1.03.59.00.cfg.bin.json` →
/// 1 / 240 / 30 / 240 / 30 / 1 / 30 / 5010 / 90 / 30 entrées.
#[must_use]
pub fn parse_players_universe_config(root: &Value) -> PlayersUniverseConfig {
    PlayersUniverseConfig {
        universe_info: map_list(root, "m_universeInfoList", UniverseInfo::from_value),
        star_key_routes: map_list(root, "m_starKeyRouteInfoList", StarKeyRouteInfo::from_value),
        stars: map_list(root, "m_starInfoList", StarInfo::from_value),
        star_keys: map_list(root, "m_starKeyInfoList", StarKeyInfo::from_value),
        universe_star_signs: map_list(
            root,
            "m_universeStarSignInfoList",
            UniverseStarSignInfo::from_value,
        ),
        universe_star_sign_sets: map_list(
            root,
            "m_universeStarSignSetDataList",
            UniverseStarSignSetData::from_value,
        ),
        star_signs: map_list(root, "m_starSignInfoList", StarSignInfo::from_value),
        star_sign_charas: map_list(
            root,
            "m_starSignCharaInfoList",
            StarSignCharaInfo::from_value,
        ),
        star_sign_rarity_rates: map_list(
            root,
            "m_starSignRarityRateInfoList",
            StarSignRarityRateInfo::from_value,
        ),
        star_sign_chara_sets: map_list(
            root,
            "m_starSignCharaSetDataList",
            StarSignCharaSetData::from_value,
        ),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  players_universe_event_config — PLAYERS_UNIVERSE_RESULT_EFFECT_INFO (12)
// ══════════════════════════════════════════════════════════════════════════════

/// Entrée `PLAYERS_UNIVERSE_RESULT_EFFECT_INFO` — un effet d'animation de résultat
/// (la trajectoire d'un objet vers un locateur, lors de l'écran de tirage).
///
/// Vérité terrain : `players_universe_event_config.cfg.bin.json`
/// `m_playersUniverseResultEffectInfoList[0]` : `id = 1`, `moveDegree = 160.0`,
/// `moveDistance = 25`, `targetLocatorCrc = 0xED2AA647`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayersUniverseResultEffectInfo {
    /// `id` — identifiant de l'effet (1-basé).
    pub id: i64,
    /// `moveDegree` — angle de déplacement en degrés (flottant, peut être négatif).
    pub move_degree: f64,
    /// `moveDistance` — distance de déplacement.
    pub move_distance: i64,
    /// `targetLocatorCrc` — hash du locateur cible.
    pub target_locator_crc: HashId,
}

impl PlayersUniverseResultEffectInfo {
    /// Parse une entrée.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            id: field_i64(v, "id").unwrap_or(0),
            move_degree: field_f64(v, "moveDegree"),
            move_distance: field_i64(v, "moveDistance").unwrap_or(0),
            target_locator_crc: field_hash(v, "targetLocatorCrc"),
        }
    }
}

/// Parse un `players_universe_event_config.cfg.bin.json` (1 liste).
///
/// Vérité terrain : `players_universe_event_config.cfg.bin.json` →
/// `m_playersUniverseResultEffectInfoList` = 12 entrées.
#[must_use]
pub fn parse_players_universe_event_config(root: &Value) -> Vec<PlayersUniverseResultEffectInfo> {
    map_list(
        root,
        "m_playersUniverseResultEffectInfoList",
        PlayersUniverseResultEffectInfo::from_value,
    )
}
