// SPDX-License-Identifier: Apache-2.0
//! Famille `soccer_chara_unique_rarity` — variantes « Hero » (Feu / Noir / Rose) par personnage.
//!
//! ## Rôle
//!
//! Le config `soccer_chara_unique_rarity_config` associe chaque `charaId` de base à ses
//! éventuelles variantes Hero (火ヒーロー = Feu, 黒ヒーロー = Noir, 桃ヒーロー = Rose) :
//! un booléen de disponibilité (`isHeroFire`/`isHeroBlack`/`isHeroPink`) et le `charaParamId`
//! de la variante correspondante (`charaParamIdHeroFire`/`...Black`/`...Pink`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! - VFS : `data/common/gamedata/soccer/soccer_chara_unique_rarity_config_1.03.00.00.cfg.bin`
//! - Format : RDBN `lists`, mono-liste `m_soccerCharaUniqueRarityList` (type
//!   `SOCCER_CHARA_UNIQUE_RARITY`), **71 entrées**.
//! - Port 1:1 de inagle `packages/inagle/src/parsers/hero-config.ts` (`loadHeroConfig`).
//!
//! ## Détail Level-5 préservé (important)
//!
//! Le booléen `isHero*` **et** le `charaParamIdHero*` sont indépendants dans le dump : il
//! existe des entrées où le flag est `false` alors qu'un `charaParamId` non nul est présent
//! (ex. index 1 `0x13B82253` : `isHeroBlack=false` mais `charaParamIdHeroBlack=0x04F417C5`).
//! Comme inagle, le `charaParamId` n'est **exposé** ([`HeroVariantInfo::fire_param_id`], etc.)
//! que si `flag == true` **et** l'id n'est pas la sentinelle `0x00000000`. Sinon → `None`.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, list_values};
use crate::hash::HashId;

/// Nom de l'unique liste portée du config.
pub const LIST_NAME: &str = "m_soccerCharaUniqueRarityList";

/// Les trois saveurs de variante Hero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HeroType {
    /// 火ヒーロー — Hero Feu (`charaParamIdHeroFire`).
    Fire,
    /// 黒ヒーロー — Hero Noir (`charaParamIdHeroBlack`).
    Black,
    /// 桃ヒーロー — Hero Rose (`charaParamIdHeroPink`).
    Pink,
}

/// Variantes Hero disponibles pour un personnage de base.
///
/// ## Vérité terrain
///
/// `m_soccerCharaUniqueRarityList[0]` :
/// `{ charaId: 0x81789A26, isHeroFire: false, charaParamIdHeroFire: 0x00000000,
///    isHeroBlack: true, charaParamIdHeroBlack: 0x9634AFB0,
///    isHeroPink: true, charaParamIdHeroPink: 0xBFFC1B42 }`
/// → `has_fire=false`, `fire_param_id=None` (flag faux), `black_param_id=Some(0x9634AFB0)`,
///   `pink_param_id=Some(0xBFFC1B42)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HeroVariantInfo {
    /// `charaId` — hash du personnage de base.
    pub chara_id: HashId,
    /// `isHeroFire` — la variante Feu est-elle déclarée disponible ?
    pub has_fire: bool,
    /// `charaParamIdHeroFire`, exposé uniquement si `has_fire` et id non nul.
    pub fire_param_id: Option<HashId>,
    /// `isHeroBlack` — la variante Noir est-elle déclarée disponible ?
    pub has_black: bool,
    /// `charaParamIdHeroBlack`, exposé uniquement si `has_black` et id non nul.
    pub black_param_id: Option<HashId>,
    /// `isHeroPink` — la variante Rose est-elle déclarée disponible ?
    pub has_pink: bool,
    /// `charaParamIdHeroPink`, exposé uniquement si `has_pink` et id non nul.
    pub pink_param_id: Option<HashId>,
}

impl HeroVariantInfo {
    /// Parse une entrée de `m_soccerCharaUniqueRarityList`.
    ///
    /// Port 1:1 de `hero-config.ts` (l.81-104) : `hasX = isHeroX === true`, puis le paramId
    /// n'est conservé que si `hasX && id !== "0x00000000"`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            chara_id: field_hash(v, "charaId"),
            has_fire: field_bool(v, "isHeroFire").unwrap_or(false),
            fire_param_id: variant_param(v, "isHeroFire", "charaParamIdHeroFire"),
            has_black: field_bool(v, "isHeroBlack").unwrap_or(false),
            black_param_id: variant_param(v, "isHeroBlack", "charaParamIdHeroBlack"),
            has_pink: field_bool(v, "isHeroPink").unwrap_or(false),
            pink_param_id: variant_param(v, "isHeroPink", "charaParamIdHeroPink"),
        }
    }

    /// `charaParamId` exposé pour le type de Hero demandé (`None` si indisponible).
    #[must_use]
    pub fn param_id(&self, ty: HeroType) -> Option<HashId> {
        match ty {
            HeroType::Fire => self.fire_param_id,
            HeroType::Black => self.black_param_id,
            HeroType::Pink => self.pink_param_id,
        }
    }
}

/// Renvoie `Some(id)` uniquement si le flag `flag_key` est vrai et l'id `id_key` non nul
/// (sentinelle `0x00000000`). Reproduit la garde d'inagle `hasX && id !== NULL_ID`.
fn variant_param(v: &Value, flag_key: &str, id_key: &str) -> Option<HashId> {
    let has = field_bool(v, flag_key).unwrap_or(false);
    let id = field_hash(v, id_key);
    if has && !id.is_zero() { Some(id) } else { None }
}

/// Parse tout le config : la liste `m_soccerCharaUniqueRarityList` en ordre du fichier.
///
/// Équivaut au remplissage de `byCharaId` dans `loadHeroConfig` (hero-config.ts).
#[must_use]
pub fn parse_hero_config(root: &Value) -> Vec<HeroVariantInfo> {
    let mut out = Vec::new();
    if let Some(values) = list_values(root, LIST_NAME) {
        out.reserve(values.len());
        for v in values {
            out.push(HeroVariantInfo::from_value(v));
        }
    }
    out
}

/// Type de Hero associé à un `charaParamId` (équivalent de `byParamId` / `getHeroType`).
///
/// Comme la `Map` d'inagle (`set` écrase), en cas de collision improbable la **dernière**
/// affectation en ordre de fichier (puis Feu→Noir→Rose dans une entrée) l'emporte.
#[must_use]
pub fn hero_type_for_param(list: &[HeroVariantInfo], param_id: HashId) -> Option<HeroType> {
    let mut found = None;
    for e in list {
        if e.fire_param_id == Some(param_id) {
            found = Some(HeroType::Fire);
        }
        if e.black_param_id == Some(param_id) {
            found = Some(HeroType::Black);
        }
        if e.pink_param_id == Some(param_id) {
            found = Some(HeroType::Pink);
        }
    }
    found
}

/// Variantes Hero d'un personnage de base (équivalent de `getHeroVariants` / `byCharaId`).
///
/// Renvoie la **première** entrée dont `chara_id == chara_id` (un seul match attendu : 71
/// `charaId` tous distincts dans le dump réel).
#[must_use]
pub fn variants_for_chara(list: &[HeroVariantInfo], chara_id: HashId) -> Option<&HeroVariantInfo> {
    list.iter().find(|e| e.chara_id == chara_id)
}
