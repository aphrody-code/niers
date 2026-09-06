//! `SkillInfo` — port Rust d'une valeur `m_skillInfoList` du `skill_config`.
//!
//! ## Vérité terrain
//!
//! - Type TS : `RawSkillConfigInfo`, `packages/inagle/src/skills/types.ts` (l.17-44),
//!   enums `ELEMENT_NAMES` / `CATEGORY_NAMES` (l.85-104).
//! - Données : `data/common/gamedata/skill/skill_config_5.00.07.00.cfg.bin.json`
//!   (version antérieure `4.00.17.00`, disparue après une MAJ du jeu — chemin corrigé,
//!   compte NON revérifié : la MAJ du jeu peut avoir changé le nombre de valeurs de
//!   `m_skillInfoList`, précédemment mesuré à 2627).
//! - Jointure name/desc : `packages/inagle/src/skills/mapper.ts` (`parseSkillText`, l.66-105) :
//!   NOUN_INFO var[5] = nom, TEXT_INFO var[2] = description, var[0] = hash (>>> 0).
//!
//! Première valeur vérifiée (whs00010, « Trampoline du tonnerre ») : skillID=0x63BDA8A4,
//! skillIDStr=whs00010, element=1 (Vent), category=1 (Tir), power 70→440, consumeTp=70,
//! recastTime=90, partnerType=2, partner1=0xAB97A3D2, eldorado=false, seriesIdCrc=0x62E8448F.
//!
//! Le dump v4 réel possède **28** champs : les 26 du type TS + `seriesIdCrc` et
//! `isDisablePlayableUntilNextPatch` (présents dans le JSON, absents du type TS). On porte les 28.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_str, list_values, owned, walk_named};
use crate::hash::HashId;

/// Élément d'une technique (mêmes IDs que les personnages). Source : `ELEMENT_NAMES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SkillElement {
    None,
    /// 1 = Vent (風).
    Wind,
    /// 2 = Forêt (林).
    Forest,
    /// 3 = Feu (火).
    Fire,
    /// 4 = Montagne (山).
    Mountain,
    /// 5 = Néant (無).
    Void,
    /// Valeur hors table.
    Other(i64),
}

impl SkillElement {
    /// Mappe l'ID brut. Source : `ELEMENT_NAMES`, types.ts l.85-93.
    #[must_use]
    pub fn from_id(id: i64) -> Self {
        match id {
            0 => Self::None,
            1 => Self::Wind,
            2 => Self::Forest,
            3 => Self::Fire,
            4 => Self::Mountain,
            5 => Self::Void,
            other => Self::Other(other),
        }
    }

    /// Noms (fr, en, ja) — identiques à `ELEMENT_NAMES`.
    #[must_use]
    pub fn names(self) -> Option<(&'static str, &'static str, &'static str)> {
        match self {
            Self::None => Some(("Aucun", "None", "なし")),
            Self::Wind => Some(("Vent", "Wind", "風")),
            Self::Forest => Some(("Forêt", "Forest", "林")),
            Self::Fire => Some(("Feu", "Fire", "火")),
            Self::Mountain => Some(("Montagne", "Mountain", "山")),
            Self::Void => Some(("Néant", "Void", "無")),
            Self::Other(_) => None,
        }
    }
}

/// Catégorie d'une technique. Source : `CATEGORY_NAMES`, types.ts l.95-104.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SkillCategory {
    None,
    /// 1 = Tir (シュート).
    Shoot,
    /// 2 = Dribble (ドリブル).
    Dribble,
    /// 3 = Défense / Block (ブロック).
    Block,
    /// 4 = Arrêt / Catch (キャッチ).
    Catch,
    /// 5 = Spécial (スペシャル).
    Special,
    /// 6 = Passif (パッシブ).
    Passive,
    /// 9 = Boost de stats (ステータス).
    StatBoost,
    Other(i64),
}

impl SkillCategory {
    /// Mappe l'ID brut. Source : `CATEGORY_NAMES`.
    #[must_use]
    pub fn from_id(id: i64) -> Self {
        match id {
            0 => Self::None,
            1 => Self::Shoot,
            2 => Self::Dribble,
            3 => Self::Block,
            4 => Self::Catch,
            5 => Self::Special,
            6 => Self::Passive,
            9 => Self::StatBoost,
            other => Self::Other(other),
        }
    }

    /// Noms (fr, en, ja) — identiques à `CATEGORY_NAMES`.
    #[must_use]
    pub fn names(self) -> Option<(&'static str, &'static str, &'static str)> {
        match self {
            Self::None => Some(("Aucun", "None", "なし")),
            Self::Shoot => Some(("Tir", "Shoot", "シュート")),
            Self::Dribble => Some(("Dribble", "Dribble", "ドリブル")),
            Self::Block => Some(("Défense", "Block", "ブロック")),
            Self::Catch => Some(("Arrêt", "Catch", "キャッチ")),
            Self::Special => Some(("Spécial", "Special", "スペシャル")),
            Self::Passive => Some(("Passif", "Passive", "パッシブ")),
            Self::StatBoost => Some(("Boost Stats", "Stat Boost", "ステータス")),
            Self::Other(_) => None,
        }
    }
}

/// Type de partenaire (combo). Source : `partnerType`, types.ts l.39 (0=Solo,1=Duo,2=Trio).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SkillPartnerType {
    Solo,
    Duo,
    Trio,
    Other(i64),
}

impl SkillPartnerType {
    #[must_use]
    pub fn from_id(id: i64) -> Self {
        match id {
            0 => Self::Solo,
            1 => Self::Duo,
            2 => Self::Trio,
            other => Self::Other(other),
        }
    }
}

/// Une entrée `SKILL_INFO` du `skill_config` (28 champs, comme le dump v4 réel).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SkillInfo {
    /// `skillID` — hash de la technique (`0x63BDA8A4`).
    pub skill_id: HashId,
    /// `skillIDStr` — code interne (`whs00010`).
    pub skill_id_str: String,
    /// `eventID` — hash de l'événement de match.
    pub event_id: HashId,
    /// `eventIDName` — nom de l'événement (`ev60_00010`).
    pub event_id_name: String,
    /// `failEventID` — hash de l'événement d'échec.
    pub fail_event_id: HashId,
    /// `failEventIDName`.
    pub fail_event_id_name: String,
    /// `skillNameId` — hash vers le nom (NOUN_INFO de skill_text).
    pub skill_name_id: HashId,
    /// `skillDescId` — hash vers la description (TEXT_INFO de skill_text).
    pub skill_desc_id: HashId,
    /// `cmdOptIdx`.
    pub cmd_opt_idx: i64,
    /// `skillEffectBitFlag`.
    pub skill_effect_bit_flag: i64,
    /// `power_min`.
    pub power_min: i64,
    /// `power_max`.
    pub power_max: i64,
    /// `element` (ID brut) → voir [`SkillInfo::element`].
    pub element: i64,
    /// `colorIdx`.
    pub color_idx: i64,
    /// `category` (ID brut) → voir [`SkillInfo::category`].
    pub category: i64,
    /// `growthType`.
    pub growth_type: i64,
    /// `foulRate`.
    pub foul_rate: i64,
    /// `consumeTp` — coût en Tension (mécanique « Tension » en jeu).
    pub consume_tp: i64,
    /// `focusBattleEffectId`.
    pub focus_battle_effect_id: HashId,
    /// `recastTime`.
    pub recast_time: i64,
    /// `partnerType` (ID brut) → voir [`SkillInfo::partner_type`].
    pub partner_type: i64,
    /// `partner1`.
    pub partner1: HashId,
    /// `partner2`.
    pub partner2: HashId,
    /// `partner3`.
    pub partner3: HashId,
    /// `telopInfoId`.
    pub telop_info_id: HashId,
    /// `eldorado`.
    pub eldorado: bool,
    /// `seriesIdCrc` — présent dans le dump v4 réel (absent du type TS).
    pub series_id_crc: HashId,
    /// `isDisablePlayableUntilNextPatch` — présent dans le dump v4 réel (absent du type TS).
    pub is_disable_playable_until_next_patch: bool,
}

/// Chemins VFS des assets de cut-in d'un hissatsu, dérivés par [`SkillInfo::cutin_assets`].
/// Préfixés `data/` (directement utilisables par `nie-model-serve` `/raw`/`/tex`/`/cfg`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CutinAssets {
    /// Nom d'event lié (ex. `ev60_00340`).
    pub event_id_name: String,
    /// Modèle 3D du cut-in (géométrie) : `data/common/chr/_waza/<ev>/<ev>.g4mg`.
    pub model_g4mg: String,
    /// Descripteur du modèle : `data/common/chr/_waza/<ev>/<ev>.g4md`.
    pub model_g4md: String,
    /// Texture du modèle de cut-in : `data/dx11/chr/_waza/<ev>/<ev>.g4tx`.
    pub texture_g4tx: String,
    /// Config son (cues) : `data/common/event_cfg/snd/<ev>_snd.cfg.bin`.
    pub sound_cfg: String,
    /// Répertoire des effets de particules : `data/common/event/<grp>/<ev>/`.
    pub effects_dir: String,
    /// Telop (nom du skill rendu) par langue : `(lang, chemin .g4tx)`.
    pub telop_by_lang: Vec<(String, String)>,
}

impl SkillInfo {
    /// Élément typé.
    #[must_use]
    pub fn element(&self) -> SkillElement {
        SkillElement::from_id(self.element)
    }

    /// Catégorie typée.
    #[must_use]
    pub fn category(&self) -> SkillCategory {
        SkillCategory::from_id(self.category)
    }

    /// Type de partenaire typé.
    #[must_use]
    pub fn partner_type(&self) -> SkillPartnerType {
        SkillPartnerType::from_id(self.partner_type)
    }

    /// Construit les **chemins VFS des assets de cut-in** du hissatsu, dérivés de
    /// `event_id_name` (ex. `ev60_00340`) et `skill_id_str` (ex. `whs00340`). Templates
    /// vérifiés byte-exact sur whs00340 (« God Knows ») : modèle 3D `chr/_waza/<ev>/<ev>.g4mg`,
    /// texture `dx11/chr/_waza/<ev>/<ev>.g4tx`, son `event_cfg/snd/<ev>_snd.cfg.bin`, effets
    /// `event/<grp>/<ev>/`, telop par langue `menu/220_img/telop_waza/<lang>/<skill_id_str>.g4tx`.
    /// `None` si le skill n'a pas d'event de cut-in (`event_id_name` vide). Le caller vérifie
    /// l'existence réelle (toutes les langues ne sont pas présentes pour tous les skills).
    #[must_use]
    pub fn cutin_assets(&self) -> Option<CutinAssets> {
        let ev = self.event_id_name.as_str();
        if ev.is_empty() {
            return None;
        }
        let grp = ev.split('_').next().unwrap_or(ev);
        // Langues réellement utilisées par les telop de cut-in (pas de `ja` : retombe sur `base`).
        const TELOP_LANGS: [&str; 8] = ["de", "en", "es", "fr", "it", "pt", "zh_hans", "zh_hant"];
        let telop_by_lang = TELOP_LANGS
            .iter()
            .map(|l| {
                (
                    (*l).to_string(),
                    format!(
                        "data/dx11/menu/220_img/telop_waza/{l}/{}.g4tx",
                        self.skill_id_str
                    ),
                )
            })
            .collect();
        Some(CutinAssets {
            event_id_name: ev.to_string(),
            model_g4mg: format!("data/common/chr/_waza/{ev}/{ev}.g4mg"),
            model_g4md: format!("data/common/chr/_waza/{ev}/{ev}.g4md"),
            texture_g4tx: format!("data/dx11/chr/_waza/{ev}/{ev}.g4tx"),
            sound_cfg: format!("data/common/event_cfg/snd/{ev}_snd.cfg.bin"),
            effects_dir: format!("data/common/event/{grp}/{ev}/"),
            telop_by_lang,
        })
    }

    /// Parse une valeur `values[]` de `m_skillInfoList` (objet JSON aux 28 clés). `None` si
    /// `skillIDStr` absent (entrée invalide).
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let skill_id_str = field_str(v, "skillIDStr")?;
        Some(SkillInfo {
            skill_id: field_hash(v, "skillID"),
            skill_id_str: owned(skill_id_str),
            event_id: field_hash(v, "eventID"),
            event_id_name: owned(field_str(v, "eventIDName").unwrap_or("")),
            fail_event_id: field_hash(v, "failEventID"),
            fail_event_id_name: owned(field_str(v, "failEventIDName").unwrap_or("")),
            skill_name_id: field_hash(v, "skillNameId"),
            skill_desc_id: field_hash(v, "skillDescId"),
            cmd_opt_idx: field_i64(v, "cmdOptIdx").unwrap_or(0),
            skill_effect_bit_flag: field_i64(v, "skillEffectBitFlag").unwrap_or(0),
            power_min: field_i64(v, "power_min").unwrap_or(0),
            power_max: field_i64(v, "power_max").unwrap_or(0),
            element: field_i64(v, "element").unwrap_or(0),
            color_idx: field_i64(v, "colorIdx").unwrap_or(0),
            category: field_i64(v, "category").unwrap_or(0),
            growth_type: field_i64(v, "growthType").unwrap_or(0),
            foul_rate: field_i64(v, "foulRate").unwrap_or(0),
            consume_tp: field_i64(v, "consumeTp").unwrap_or(0),
            focus_battle_effect_id: field_hash(v, "focusBattleEffectId"),
            recast_time: field_i64(v, "recastTime").unwrap_or(0),
            partner_type: field_i64(v, "partnerType").unwrap_or(0),
            partner1: field_hash(v, "partner1"),
            partner2: field_hash(v, "partner2"),
            partner3: field_hash(v, "partner3"),
            telop_info_id: field_hash(v, "telopInfoId"),
            eldorado: field_bool(v, "eldorado").unwrap_or(false),
            series_id_crc: field_hash(v, "seriesIdCrc"),
            is_disable_playable_until_next_patch: field_bool(v, "isDisablePlayableUntilNextPatch")
                .unwrap_or(false),
        })
    }
}

/// Parse toute la liste `m_skillInfoList` d'un `skill_config_*.cfg.bin.json` déjà désérialisé.
#[must_use]
pub fn parse_skill_config(root: &Value) -> Vec<SkillInfo> {
    let mut out = Vec::new();
    if let Some(values) = list_values(root, "m_skillInfoList") {
        for v in values {
            if let Some(s) = SkillInfo::from_value(v) {
                out.push(s);
            }
        }
    }
    out
}

/// Cartes texte issues d'un `skill_text.cfg.bin.json` : hash → nom, hash → description.
#[derive(Debug, Clone, Default)]
pub struct SkillTextMaps {
    /// hash du nom (NOUN_INFO var[0]) → nom (NOUN_INFO var[5], fallback var[2]).
    pub names: BTreeMap<HashId, String>,
    /// hash de la description (TEXT_INFO var[0]) → texte (TEXT_INFO var[2]).
    pub descriptions: BTreeMap<HashId, String>,
}

/// Parse un `skill_text.cfg.bin.json` : NOUN_INFO (nom) + TEXT_INFO (description).
///
/// Reproduit exactement `parseSkillText` (mapper.ts l.66-105) : pour chaque enfant
/// `NOUN_INFO_*`, hash = var[0] (>>> 0), nom = var[5] sinon var[2] ; pour `TEXT_INFO_*`,
/// hash = var[0], desc = var[2]. On ignore les hash nuls et les chaînes vides.
///
/// Le filtre enfant accepte le nom brut SANS suffixe (`NOUN_INFO`/`TEXT_INFO`) **en plus** du
/// `_<i>` historique : sur le T2B binaire lu en direct (`nie_explore::bridge::t2b_to_json`), les
/// enfants n'ont pas ce suffixe — ajouté par le dumper JSON inagle, pas par le format T2B (même
/// constat que `chara_base`/`chara_text`, cf. leurs commentaires d'en-tête).
#[must_use]
pub fn parse_skill_text(root: &Value) -> SkillTextMaps {
    let mut maps = SkillTextMaps::default();

    walk_named(root, "NOUN_INFO_BEGIN", |entry| {
        for child in entry.children() {
            let name = child.name();
            if name != "NOUN_INFO" && !name.starts_with("NOUN_INFO_") {
                continue;
            }
            let hash = child.hash(0);
            let mut name = child.string(5);
            if name.is_empty() {
                name = child.string(2);
            }
            if !hash.is_zero() && !name.is_empty() {
                maps.names.insert(hash, owned(name));
            }
        }
    });

    walk_named(root, "TEXT_INFO_BEGIN", |entry| {
        for child in entry.children() {
            let name = child.name();
            if name != "TEXT_INFO" && !name.starts_with("TEXT_INFO_") {
                continue;
            }
            let hash = child.hash(0);
            let desc = child.string(2);
            if !hash.is_zero() && !desc.is_empty() {
                maps.descriptions.insert(hash, owned(desc));
            }
        }
    });

    maps
}

/// Résultat de jointure : nom + description localisés d'une technique.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SkillText {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// Joint une technique à ses textes via `skillNameId`/`skillDescId`.
#[must_use]
pub fn join_skill_text(skill: &SkillInfo, maps: &SkillTextMaps) -> SkillText {
    SkillText {
        name: maps.names.get(&skill.skill_name_id).cloned(),
        description: maps.descriptions.get(&skill.skill_desc_id).cloned(),
    }
}
