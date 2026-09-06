//! `AiConfig` — port des familles de configuration IA (`ai/*.cfg.bin.json`) (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! Cinq fichiers dans `data/common/gamedata/ai/` :
//!
//! | Fichier | Format | Listes | Comptes |
//! |---------|--------|--------|---------|
//! | `soccer_ai_cmd_config_0.00.00.cfg.bin.json` | `lists` | 2 | 73 UPDATE_TIMING + 71 AI_CMD |
//! | `soccer_ai_cmd_config_0.05.91.cfg.bin.json` | `lists` | 2 | 80 UPDATE_TIMING + 77 AI_CMD |
//! | `soccer_user_ai_config_1.01.50.cfg.bin.json` | `lists` | 7 | 26+20+11+4+13+6+4 |
//! | `strategy_ai_config_1.01.50.cfg.bin.json` | `lists` | 9 | 66+62+182+74+60+15+14+56+5 |
//! | `tactics_ai_config_0.06.44.cfg.bin.json` | `lists` | 5 | 52+60+60+30+5 |
//!
//! Tous les fichiers utilisent le format **`lists`** (champs nommés dans `values[]`),
//! et non le format `entries` (noeuds positionnels). Les champs sont lus par nom
//! via les helpers `field_*` de `cfgbin.rs`.
//!
//! ## `soccer_ai_cmd_config` (`SoccerAiCmdConfig`)
//!
//! Deux listes :
//! - `m_updateTimingInfoList` — `UPDATE_TIMING_INFO` : `{updateTimingId: i64}`.
//!   Vérité terrain : ligne 9 de `soccer_ai_cmd_config_0.00.00.cfg.bin.json`,
//!   `values[0].updateTimingId = 2022`.
//! - `m_conditionInfoList` — `AI_CMD_INFO` : `{conditionId: i64, updateTimingInfo: [i64, i64]}`.
//!   `updateTimingInfo[0]` = offset, `[1]` = count, index dans `m_updateTimingInfoList`.
//!   Vérité terrain : ligne 234, `values[0] = {conditionId: 1001, updateTimingInfo: [0, 1]}`.
//!
//! ## `soccer_user_ai_config` (`SoccerUserAiConfig`)
//!
//! Sept listes :
//! - `m_userParam` — `{id: i64, param: f32}` (26 entrées).
//!   Vérité terrain : `values[0] = {id: 1, param: 0.3}`.
//! - `m_coachEffect` — `{type: i64, calc: i64, value: f32}` (20 entrées).
//! - `m_coachChoise` — `{idx, textId, timing, geoId, cameraId, time, offsetX, offsetZ, speachId, effect: [i64, i64]}` (11 entrées).
//! - `m_coachInfo` — `{id: HashId, textId: HashId, choise: [i64, i64]}` (4 entrées).
//! - `m_titleEffect` — `{type: i64, calc: i64, value: f32}` (13 entrées).
//! - `m_titleConditionId` — `{conditionId: i64, condition: String}` (6 entrées).
//! - `m_titleInfo` — `{id, textId, type, limitType, limitParam, condition, conditionId: [i64,i64], effect: [i64,i64]}` (4 entrées).
//!
//! ## `strategy_ai_config` (`StrategyAiConfig`)
//!
//! Neuf listes :
//! - `m_StrategyAITacticsInfoList` — `{tacticsId: HashId, tacticsParam: i64}` (66 entrées).
//!   Vérité terrain : ligne 9 de `strategy_ai_config_1.01.50.cfg.bin.json`,
//!   `values[0] = {tacticsId: "0x906D3411", tacticsParam: 0}`.
//! - `m_StrategyAIInfoList` — 7 champs + `tacticsInfo: [i64, i64]` (62 entrées).
//!   Vérité terrain : ligne 279, `values[0].strategyId = "0x7C24BB03"`.
//! - `m_ConditionIdList` — `CONDITION_ID_INFO` : `{conditionId: i64, condition: String}` (182 entrées).
//! - `m_ConditionIdList2` — même type, valeurs différentes (74 entrées).
//! - `m_PhaseCheckInfoList` — `{phaseId, phaseStateId, condition, condition2, conditionIdList: [i64,i64], conditionIdList2: [i64,i64]}` (60 entrées).
//! - `m_EvaluationAdjustParamInfoList` — `{adjustType: i64, param: i64}` (15 entrées).
//! - `m_EvaluationAdjustInfoList` — `{evaluationAdustId: HashId, adjustInfo: [i64,i64]}` (14 entrées).
//!   Note : `evaluationAdustId` contient une faute de frappe Level-5 (`Adust` sans `j`) — conservée.
//! - `m_TeamStrategyAIDataInfoList` — `{strategyId: HashId, enableLv: i64}` (56 entrées).
//! - `m_TeamStrategyAIInfoList` — `{groupId: HashId, strategyInfo: [i64,i64]}` (5 entrées).
//!   Vérité terrain : ligne 3311, `values[0] = {groupId: "0x9714544E", strategyInfo: [0, 3]}`.
//!
//! ## `tactics_ai_config` (`TacticsAiConfig`)
//!
//! Cinq listes :
//! - `m_TacticsAITargetInfoList` — `{targetId: HashId, funcName: String}` (52 entrées).
//!   Vérité terrain : ligne 9 de `tactics_ai_config_0.06.44.cfg.bin.json`,
//!   `values[0] = {targetId: "0x1C44B74C", funcName: "CalcTargetChara_None"}`.
//! - `m_TacticsAIActionInfoList` — `{actionId, targetCharaType, targetPosType, targetId: HashId}` (60 entrées).
//! - `m_TacticsAIInfoList` — `{tacticsId, nameId, descId, actionInfo: [i64,i64]}` (60 entrées).
//! - `m_CharaTacticsAIDataInfoList` — `{tacticsId: HashId, enableLv: i64}` (30 entrées).
//! - `m_CharaTacticsAIInfoList` — `{groupId: HashId, tacticsInfo: [i64,i64]}` (5 entrées).
//!   Vérité terrain : ligne 1261, `values[0] = {groupId: "0x35664547", tacticsInfo: [0, 2]}`.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_str, list_values, owned};
use crate::hash::HashId;

// ─── Helpers internes ────────────────────────────────────────────────────────

/// Extrait un tableau `[i64, i64]` nommé `key` dans un objet `Value`.
/// Renvoie `(0, 0)` si le champ est absent ou malformé.
fn array2_i64(v: &Value, key: &str) -> (i64, i64) {
    let arr = match v.get(key).and_then(Value::as_array) {
        Some(a) => a,
        None => return (0, 0),
    };
    let a = arr.first().and_then(Value::as_i64).unwrap_or(0);
    let b = arr.get(1).and_then(Value::as_i64).unwrap_or(0);
    (a, b)
}

/// Extrait un champ flottant depuis un objet `Value`. 0.0 si absent.
fn field_f32(v: &Value, key: &str) -> f32 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32
}

/// Parcourt une liste nommée du JSON et applique `f` à chaque entrée.
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

// ═══════════════════════════════════════════════════════════════════════════════
// SOCCER_AI_CMD_CONFIG
// ═══════════════════════════════════════════════════════════════════════════════

// ─── UpdateTimingInfo ─────────────────────────────────────────────────────────

/// Timing de mise à jour d'une condition IA (`UPDATE_TIMING_INFO`).
///
/// Entrée de `m_updateTimingInfoList` dans `soccer_ai_cmd_config_*.cfg.bin.json`.
///
/// Vérité terrain : `soccer_ai_cmd_config_0.00.00.cfg.bin.json` ligne 9,
/// `values[0].updateTimingId = 2022`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UpdateTimingInfo {
    /// `updateTimingId` — identifiant du timing de mise à jour.
    pub update_timing_id: i64,
}

impl UpdateTimingInfo {
    /// Parse une entrée de `m_updateTimingInfoList`. `None` si `updateTimingId` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            update_timing_id: field_i64(v, "updateTimingId")?,
        })
    }
}

// ─── AiCmdInfo ────────────────────────────────────────────────────────────────

/// Condition IA avec référence à une plage de timings (`AI_CMD_INFO`).
///
/// Entrée de `m_conditionInfoList` dans `soccer_ai_cmd_config_*.cfg.bin.json`.
/// `timing_offset`/`timing_count` référencent une plage dans `m_updateTimingInfoList`
/// — résoudre via [`SoccerAiCmdConfig::timings_of`].
///
/// Vérité terrain : `soccer_ai_cmd_config_0.00.00.cfg.bin.json` ligne 234,
/// `values[0] = {conditionId: 1001, updateTimingInfo: [0, 1]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AiCmdInfo {
    /// `conditionId` — identifiant de la condition IA.
    pub condition_id: i64,
    /// `updateTimingInfo[0]` — index de début dans `m_updateTimingInfoList`.
    pub timing_offset: i64,
    /// `updateTimingInfo[1]` — nombre de timings (`0` = aucun timing actif).
    pub timing_count: i64,
}

impl AiCmdInfo {
    /// Parse une entrée de `m_conditionInfoList`. `None` si `conditionId` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let condition_id = field_i64(v, "conditionId")?;
        let (timing_offset, timing_count) = array2_i64(v, "updateTimingInfo");
        Some(Self {
            condition_id,
            timing_offset,
            timing_count,
        })
    }
}

// ─── SoccerAiCmdConfig ────────────────────────────────────────────────────────

/// Contenu complet d'un `soccer_ai_cmd_config_*.cfg.bin.json`.
///
/// Deux listes : `update_timings` (timings disponibles) et `conditions` (conditions IA
/// référençant ces timings). Utiliser [`parse_soccer_ai_cmd_config`] pour construire.
///
/// Comptes réels :
/// - v0.00.00 : 73 timings + 71 conditions.
/// - v0.05.91 : 80 timings + 77 conditions.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerAiCmdConfig {
    /// `m_updateTimingInfoList` — timings de mise à jour.
    pub update_timings: Vec<UpdateTimingInfo>,
    /// `m_conditionInfoList` — conditions IA avec références aux timings.
    pub conditions: Vec<AiCmdInfo>,
}

impl SoccerAiCmdConfig {
    /// Renvoie la plage de timings d'une condition (tranche dans `update_timings`).
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    #[must_use]
    pub fn timings_of(&self, cmd: &AiCmdInfo) -> &[UpdateTimingInfo] {
        let start = cmd.timing_offset as usize;
        let end = start.saturating_add(cmd.timing_count as usize);
        self.update_timings.get(start..end).unwrap_or(&[])
    }

    /// Recherche une condition par son `condition_id`. `None` si absente.
    #[must_use]
    pub fn find_condition(&self, condition_id: i64) -> Option<&AiCmdInfo> {
        self.conditions
            .iter()
            .find(|c| c.condition_id == condition_id)
    }
}

/// Parse un `soccer_ai_cmd_config_*.cfg.bin.json` complet (2 listes).
///
/// Les entrées dont `conditionId` est absent sont silencieusement ignorées.
#[must_use]
pub fn parse_soccer_ai_cmd_config(root: &Value) -> SoccerAiCmdConfig {
    SoccerAiCmdConfig {
        update_timings: extract_list(root, "m_updateTimingInfoList", UpdateTimingInfo::from_value),
        conditions: extract_list(root, "m_conditionInfoList", AiCmdInfo::from_value),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SOCCER_USER_AI_CONFIG
// ═══════════════════════════════════════════════════════════════════════════════

// ─── SoccerUserAiParam ────────────────────────────────────────────────────────

/// Paramètre utilisateur IA soccer (`SoccerUserAIParam`).
///
/// Entrée de `m_userParam` dans `soccer_user_ai_config_1.01.50.cfg.bin.json`.
///
/// Vérité terrain : `values[0] = {id: 1, param: 0.3}`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerUserAiParam {
    /// `id` — identifiant du paramètre (1-basé, 1..=26 dans le dump réel).
    pub id: i64,
    /// `param` — valeur du paramètre (typiquement 0.3 dans le dump réel).
    pub param: f32,
}

impl SoccerUserAiParam {
    /// Parse une entrée de `m_userParam`. `None` si `id` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            id: field_i64(v, "id")?,
            param: field_f32(v, "param"),
        })
    }
}

// ─── SoccerCoachAiEffect ──────────────────────────────────────────────────────

/// Effet d'un conseil IA soccer (`SoccerCoachAIEffect`).
///
/// Entrée de `m_coachEffect` dans `soccer_user_ai_config_1.01.50.cfg.bin.json`.
///
/// Vérité terrain : `values[0] = {type: 26, calc: 1, value: 0}`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerCoachAiEffect {
    /// `type` — type d'effet.
    pub effect_type: i64,
    /// `calc` — mode de calcul (0 = additif, 1 = multiplicatif dans les dumps réels).
    pub calc: i64,
    /// `value` — valeur de l'effet (peut être 0, 0.5, 1…).
    pub value: f32,
}

impl SoccerCoachAiEffect {
    /// Parse une entrée de `m_coachEffect`. `None` si `type` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            effect_type: field_i64(v, "type")?,
            calc: field_i64(v, "calc").unwrap_or(0),
            value: field_f32(v, "value"),
        })
    }
}

// ─── SoccerCoachAiChoise ──────────────────────────────────────────────────────

/// Choix de conseil IA soccer (`SoccerCoachAIChoise`).
///
/// Entrée de `m_coachChoise` dans `soccer_user_ai_config_1.01.50.cfg.bin.json`.
/// `effect_offset`/`effect_count` référencent une plage dans `m_coachEffect`.
///
/// Vérité terrain : `values[0] = {idx: 0, textId: "0xD3725F39", timing: 0,
/// geoId: "0x00000000", ..., speachId: "0xC2A4169A", effect: [0, 1]}`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerCoachAiChoise {
    /// `idx` — index du choix (0-basé, peut se répéter entre les groupes).
    pub idx: i64,
    /// `textId` — hash du texte affiché au joueur.
    pub text_id: HashId,
    /// `timing` — timing de déclenchement.
    pub timing: i64,
    /// `geoId` — hash de la géométrie associée (zéro si aucune).
    pub geo_id: HashId,
    /// `cameraId` — hash de la caméra associée (zéro si aucune).
    pub camera_id: HashId,
    /// `time` — durée en frames.
    pub time: i64,
    /// `offsetX` — décalage horizontal.
    pub offset_x: f32,
    /// `offsetZ` — décalage en profondeur.
    pub offset_z: f32,
    /// `speachId` — hash du discours vocal associé.
    pub speach_id: HashId,
    /// `effect[0]` — index de début dans `m_coachEffect`.
    pub effect_offset: i64,
    /// `effect[1]` — nombre d'effets.
    pub effect_count: i64,
}

impl SoccerCoachAiChoise {
    /// Parse une entrée de `m_coachChoise`. `None` si `textId` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let text_id = field_hash(v, "textId");
        if text_id.is_zero() {
            return None;
        }
        let (effect_offset, effect_count) = array2_i64(v, "effect");
        Some(Self {
            idx: field_i64(v, "idx").unwrap_or(0),
            text_id,
            timing: field_i64(v, "timing").unwrap_or(0),
            geo_id: field_hash(v, "geoId"),
            camera_id: field_hash(v, "cameraId"),
            time: field_i64(v, "time").unwrap_or(0),
            offset_x: field_f32(v, "offsetX"),
            offset_z: field_f32(v, "offsetZ"),
            speach_id: field_hash(v, "speachId"),
            effect_offset,
            effect_count,
        })
    }
}

// ─── SoccerCoachAiInfo ────────────────────────────────────────────────────────

/// Informations d'un coach IA soccer (`SoccerCoachAIInfo`).
///
/// Entrée de `m_coachInfo` dans `soccer_user_ai_config_1.01.50.cfg.bin.json`.
/// `choise_offset`/`choise_count` référencent une plage dans `m_coachChoise`.
///
/// Vérité terrain : `values[0] = {id: "0x50317469", textId: "0x809D8D96", choise: [0, 3]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerCoachAiInfo {
    /// `id` — hash identifiant unique du coach.
    pub id: HashId,
    /// `textId` — hash du nom du coach.
    pub text_id: HashId,
    /// `choise[0]` — index de début dans `m_coachChoise`.
    pub choise_offset: i64,
    /// `choise[1]` — nombre de choix disponibles pour ce coach.
    pub choise_count: i64,
}

impl SoccerCoachAiInfo {
    /// Parse une entrée de `m_coachInfo`. `None` si `id` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        let (choise_offset, choise_count) = array2_i64(v, "choise");
        Some(Self {
            id,
            text_id: field_hash(v, "textId"),
            choise_offset,
            choise_count,
        })
    }
}

// ─── SoccerTitleAiEffect ──────────────────────────────────────────────────────

/// Effet d'un titre IA soccer (`SoccerTitleAIEffect`).
///
/// Entrée de `m_titleEffect` dans `soccer_user_ai_config_1.01.50.cfg.bin.json`.
///
/// Vérité terrain : `values[0] = {type: 17, calc: 0, value: 100}`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerTitleAiEffect {
    /// `type` — type d'effet du titre.
    pub effect_type: i64,
    /// `calc` — mode de calcul.
    pub calc: i64,
    /// `value` — valeur de l'effet.
    pub value: f32,
}

impl SoccerTitleAiEffect {
    /// Parse une entrée de `m_titleEffect`. `None` si `type` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            effect_type: field_i64(v, "type")?,
            calc: field_i64(v, "calc").unwrap_or(0),
            value: field_f32(v, "value"),
        })
    }
}

// ─── SoccerTitleAiCondId ──────────────────────────────────────────────────────

/// Condition encodée d'un titre IA soccer (`SoccerTitleAICondId`).
///
/// Entrée de `m_titleConditionId` dans `soccer_user_ai_config_1.01.50.cfg.bin.json`.
/// `condition` est une chaîne base64 encodant des données binaires opaques.
///
/// Vérité terrain : `values[0] = {conditionId: 1113, condition: "AAAAAA8FNWokrVsAAQAyAAAAAXg="}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerTitleAiCondId {
    /// `conditionId` — identifiant de la condition.
    pub condition_id: i64,
    /// `condition` — données de condition encodées en base64.
    pub condition: String,
}

impl SoccerTitleAiCondId {
    /// Parse une entrée de `m_titleConditionId`. `None` si `conditionId` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            condition_id: field_i64(v, "conditionId")?,
            condition: owned(field_str(v, "condition").unwrap_or("")),
        })
    }
}

// ─── SoccerTitleAiInfo ────────────────────────────────────────────────────────

/// Informations d'un titre IA soccer (`SoccerTitleAIInfo`).
///
/// Entrée de `m_titleInfo` dans `soccer_user_ai_config_1.01.50.cfg.bin.json`.
/// `condition_id_offset`/`count` référencent `m_titleConditionId` ;
/// `effect_offset`/`count` référencent `m_titleEffect`.
///
/// Vérité terrain : `values[0] = {id: "0x0312BC99", textId: "0x7C138CF6",
/// type: 0, limitType: 0, limitParam: 1, conditionId: [0, 2], effect: [0, 4]}`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerTitleAiInfo {
    /// `id` — hash identifiant unique du titre.
    pub id: HashId,
    /// `textId` — hash du texte du titre.
    pub text_id: HashId,
    /// `type` — type de titre (0=normal, 1=intermédiaire, 2=avancé, 3=expert).
    pub info_type: i64,
    /// `limitType` — type de limite.
    pub limit_type: i64,
    /// `limitParam` — paramètre de la limite.
    pub limit_param: i64,
    /// `condition` — données binaires de condition encodées en base64 (peut être vide).
    pub condition: String,
    /// `conditionId[0]` — index de début dans `m_titleConditionId`.
    pub condition_id_offset: i64,
    /// `conditionId[1]` — nombre de conditions.
    pub condition_id_count: i64,
    /// `effect[0]` — index de début dans `m_titleEffect`.
    pub effect_offset: i64,
    /// `effect[1]` — nombre d'effets.
    pub effect_count: i64,
}

impl SoccerTitleAiInfo {
    /// Parse une entrée de `m_titleInfo`. `None` si `id` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        let (condition_id_offset, condition_id_count) = array2_i64(v, "conditionId");
        let (effect_offset, effect_count) = array2_i64(v, "effect");
        Some(Self {
            id,
            text_id: field_hash(v, "textId"),
            info_type: field_i64(v, "type").unwrap_or(0),
            limit_type: field_i64(v, "limitType").unwrap_or(0),
            limit_param: field_i64(v, "limitParam").unwrap_or(0),
            condition: owned(field_str(v, "condition").unwrap_or("")),
            condition_id_offset,
            condition_id_count,
            effect_offset,
            effect_count,
        })
    }
}

// ─── SoccerUserAiConfig ───────────────────────────────────────────────────────

/// Contenu complet de `soccer_user_ai_config_1.01.50.cfg.bin.json`.
///
/// Sept listes : paramètres utilisateur, effets et infos coach, effets/conditions/infos titre.
/// Utiliser [`parse_soccer_user_ai_config`] pour construire.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerUserAiConfig {
    /// `m_userParam` — 26 paramètres IA utilisateur.
    pub user_params: Vec<SoccerUserAiParam>,
    /// `m_coachEffect` — 20 effets de conseil.
    pub coach_effects: Vec<SoccerCoachAiEffect>,
    /// `m_coachChoise` — 11 choix de conseil.
    pub coach_choises: Vec<SoccerCoachAiChoise>,
    /// `m_coachInfo` — 4 définitions de coach.
    pub coach_infos: Vec<SoccerCoachAiInfo>,
    /// `m_titleEffect` — 13 effets de titre.
    pub title_effects: Vec<SoccerTitleAiEffect>,
    /// `m_titleConditionId` — 6 conditions de titre.
    pub title_cond_ids: Vec<SoccerTitleAiCondId>,
    /// `m_titleInfo` — 4 définitions de titre.
    pub title_infos: Vec<SoccerTitleAiInfo>,
}

impl SoccerUserAiConfig {
    /// Recherche un paramètre utilisateur par son `id`. `None` si absent.
    #[must_use]
    pub fn find_user_param(&self, id: i64) -> Option<&SoccerUserAiParam> {
        self.user_params.iter().find(|p| p.id == id)
    }

    /// Renvoie les choix disponibles pour un coach (tranche dans `coach_choises`).
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    #[must_use]
    pub fn choises_of(&self, coach: &SoccerCoachAiInfo) -> &[SoccerCoachAiChoise] {
        let start = coach.choise_offset as usize;
        let end = start.saturating_add(coach.choise_count as usize);
        self.coach_choises.get(start..end).unwrap_or(&[])
    }

    /// Renvoie les effets d'un titre (tranche dans `title_effects`).
    #[must_use]
    pub fn title_effects_of(&self, title: &SoccerTitleAiInfo) -> &[SoccerTitleAiEffect] {
        let start = title.effect_offset as usize;
        let end = start.saturating_add(title.effect_count as usize);
        self.title_effects.get(start..end).unwrap_or(&[])
    }
}

/// Parse un `soccer_user_ai_config_1.01.50.cfg.bin.json` complet (7 listes).
#[must_use]
pub fn parse_soccer_user_ai_config(root: &Value) -> SoccerUserAiConfig {
    SoccerUserAiConfig {
        user_params: extract_list(root, "m_userParam", SoccerUserAiParam::from_value),
        coach_effects: extract_list(root, "m_coachEffect", SoccerCoachAiEffect::from_value),
        coach_choises: extract_list(root, "m_coachChoise", SoccerCoachAiChoise::from_value),
        coach_infos: extract_list(root, "m_coachInfo", SoccerCoachAiInfo::from_value),
        title_effects: extract_list(root, "m_titleEffect", SoccerTitleAiEffect::from_value),
        title_cond_ids: extract_list(root, "m_titleConditionId", SoccerTitleAiCondId::from_value),
        title_infos: extract_list(root, "m_titleInfo", SoccerTitleAiInfo::from_value),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STRATEGY_AI_CONFIG
// ═══════════════════════════════════════════════════════════════════════════════

// ─── StrategyAiTacticsInfo ────────────────────────────────────────────────────

/// Tactique référencée dans une stratégie IA (`STRATEGY_AI_TACTICS_INFO`).
///
/// Entrée de `m_StrategyAITacticsInfoList` dans `strategy_ai_config_1.01.50.cfg.bin.json`.
///
/// Vérité terrain : ligne 9, `values[0] = {tacticsId: "0x906D3411", tacticsParam: 0}`.
/// `values[6] = {tacticsId: "0x6F57E0D8", tacticsParam: 100}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StrategyAiTacticsInfo {
    /// `tacticsId` — hash identifiant la tactique référencée.
    pub tactics_id: HashId,
    /// `tacticsParam` — paramètre de la tactique (0 = défaut ; peut valoir 1, 2, 50, 100, 101…).
    pub tactics_param: i64,
}

impl StrategyAiTacticsInfo {
    /// Parse une entrée de `m_StrategyAITacticsInfoList`. `None` si `tacticsId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let tactics_id = field_hash(v, "tacticsId");
        if tactics_id.is_zero() {
            return None;
        }
        Some(Self {
            tactics_id,
            tactics_param: field_i64(v, "tacticsParam").unwrap_or(0),
        })
    }
}

// ─── StrategyAiInfo ───────────────────────────────────────────────────────────

/// Définition d'une stratégie IA complète (`STRATEGY_AI_INFO`).
///
/// Entrée de `m_StrategyAIInfoList` dans `strategy_ai_config_1.01.50.cfg.bin.json`.
/// `tactics_offset`/`tactics_count` référencent une plage dans `m_StrategyAITacticsInfoList`
/// — résoudre via [`StrategyAiConfig::tactics_of`].
///
/// Vérité terrain : ligne 279, `values[0] = {strategyId: "0x7C24BB03", nameId: "0x7F0AF7DF",
/// descId: "0x0C099EA1", phaseId: "0xD854BC64", evaluationAdjustId: "0x00000000",
/// isConfirm: false, tacticsInfo: [0, 2]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StrategyAiInfo {
    /// `strategyId` — hash identifiant unique de la stratégie.
    pub strategy_id: HashId,
    /// `nameId` — hash du nom affiché.
    pub name_id: HashId,
    /// `descId` — hash de la description.
    pub desc_id: HashId,
    /// `phaseId` — hash de la phase associée (lien vers `m_PhaseCheckInfoList`).
    pub phase_id: HashId,
    /// `evaluationAdjustId` — hash de l'ajustement d'évaluation (zéro si aucun).
    pub evaluation_adjust_id: HashId,
    /// `isConfirm` — la stratégie est-elle confirmée/validée.
    pub is_confirm: bool,
    /// `tacticsInfo[0]` — index de début dans `m_StrategyAITacticsInfoList`.
    pub tactics_offset: i64,
    /// `tacticsInfo[1]` — nombre de tactiques dans cette stratégie.
    pub tactics_count: i64,
}

impl StrategyAiInfo {
    /// Parse une entrée de `m_StrategyAIInfoList`. `None` si `strategyId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let strategy_id = field_hash(v, "strategyId");
        if strategy_id.is_zero() {
            return None;
        }
        let (tactics_offset, tactics_count) = array2_i64(v, "tacticsInfo");
        Some(Self {
            strategy_id,
            name_id: field_hash(v, "nameId"),
            desc_id: field_hash(v, "descId"),
            phase_id: field_hash(v, "phaseId"),
            evaluation_adjust_id: field_hash(v, "evaluationAdjustId"),
            is_confirm: field_bool(v, "isConfirm").unwrap_or(false),
            tactics_offset,
            tactics_count,
        })
    }
}

// ─── ConditionIdInfo ──────────────────────────────────────────────────────────

/// Condition encodée identifiée (`CONDITION_ID_INFO`).
///
/// Utilisée pour `m_ConditionIdList` (182 entrées) et `m_ConditionIdList2` (74 entrées)
/// dans `strategy_ai_config_1.01.50.cfg.bin.json`.
/// `condition` est une chaîne base64 encodant des données binaires opaques.
///
/// Vérité terrain : `m_ConditionIdList[0] = {conditionId: 1102, condition: "AAAAAAkCNeHe3UoAAQA="}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConditionIdInfo {
    /// `conditionId` — identifiant de la condition.
    pub condition_id: i64,
    /// `condition` — données de condition encodées en base64.
    pub condition: String,
}

impl ConditionIdInfo {
    /// Parse une entrée de `m_ConditionIdList` ou `m_ConditionIdList2`. `None` si `conditionId` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            condition_id: field_i64(v, "conditionId")?,
            condition: owned(field_str(v, "condition").unwrap_or("")),
        })
    }
}

// ─── PhaseCheckInfo ───────────────────────────────────────────────────────────

/// Vérification de phase IA (`PHASE_CHECK_INFO`).
///
/// Entrée de `m_PhaseCheckInfoList` dans `strategy_ai_config_1.01.50.cfg.bin.json`.
/// `cond_id_list_offset`/`count` référencent `m_ConditionIdList` ;
/// `cond_id_list2_offset`/`count` référencent `m_ConditionIdList2`.
///
/// Vérité terrain : `values[0] = {phaseId: "0x3E7D8B80", phaseStateId: 0, condition: "",
/// condition2: "", conditionIdList: [0, 0], conditionIdList2: [0, 0]}`.
/// `values[2] = {phaseId: "0xD073EAAC", phaseStateId: 1, conditionIdList: [0, 1], conditionIdList2: [0, 1]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhaseCheckInfo {
    /// `phaseId` — hash identifiant la phase de jeu.
    pub phase_id: HashId,
    /// `phaseStateId` — état de la phase (0 = neutre, 1 = actif dans les dumps).
    pub phase_state_id: i64,
    /// `condition` — données binaires de la condition principale (base64, peut être vide).
    pub condition: String,
    /// `condition2` — données binaires de la condition secondaire (base64, peut être vide).
    pub condition2: String,
    /// `conditionIdList[0]` — index de début dans `m_ConditionIdList`.
    pub cond_id_list_offset: i64,
    /// `conditionIdList[1]` — nombre d'entrées dans `m_ConditionIdList`.
    pub cond_id_list_count: i64,
    /// `conditionIdList2[0]` — index de début dans `m_ConditionIdList2`.
    pub cond_id_list2_offset: i64,
    /// `conditionIdList2[1]` — nombre d'entrées dans `m_ConditionIdList2`.
    pub cond_id_list2_count: i64,
}

impl PhaseCheckInfo {
    /// Parse une entrée de `m_PhaseCheckInfoList`. `None` si `phaseId` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let phase_id = field_hash(v, "phaseId");
        // phaseId peut être zéro (cf. entrée 0, phaseStateId=0) — on parse tous les cas
        let phase_id_present = v.get("phaseId").is_some();
        if !phase_id_present {
            return None;
        }
        let (cond_id_list_offset, cond_id_list_count) = array2_i64(v, "conditionIdList");
        let (cond_id_list2_offset, cond_id_list2_count) = array2_i64(v, "conditionIdList2");
        Some(Self {
            phase_id,
            phase_state_id: field_i64(v, "phaseStateId").unwrap_or(0),
            condition: owned(field_str(v, "condition").unwrap_or("")),
            condition2: owned(field_str(v, "condition2").unwrap_or("")),
            cond_id_list_offset,
            cond_id_list_count,
            cond_id_list2_offset,
            cond_id_list2_count,
        })
    }
}

// ─── EvaluationAdjustParamInfo ────────────────────────────────────────────────

/// Paramètre d'ajustement d'évaluation IA (`EVALUATION_ADJUST_PARAM_INFO`).
///
/// Entrée de `m_EvaluationAdjustParamInfoList` dans `strategy_ai_config_1.01.50.cfg.bin.json`.
///
/// Vérité terrain : `values[0] = {adjustType: 1, param: 0}`,
/// `values[2] = {adjustType: 0, param: 5}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvaluationAdjustParamInfo {
    /// `adjustType` — type d'ajustement.
    pub adjust_type: i64,
    /// `param` — valeur du paramètre.
    pub param: i64,
}

impl EvaluationAdjustParamInfo {
    /// Parse une entrée de `m_EvaluationAdjustParamInfoList`. `None` si `adjustType` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            adjust_type: field_i64(v, "adjustType")?,
            param: field_i64(v, "param").unwrap_or(0),
        })
    }
}

// ─── EvaluationAdjustInfo ─────────────────────────────────────────────────────

/// Groupe d'ajustements d'évaluation IA (`EVALUATION_ADJUST_INFO`).
///
/// Entrée de `m_EvaluationAdjustInfoList` dans `strategy_ai_config_1.01.50.cfg.bin.json`.
/// `adjust_offset`/`adjust_count` référencent une plage dans `m_EvaluationAdjustParamInfoList`.
///
/// Note : le champ source s'appelle `evaluationAdustId` (faute de frappe Level-5 — `j` manquant
/// dans `Adjust`) ; conservée à l'identique dans le JSON.
///
/// Vérité terrain : `values[0] = {evaluationAdustId: "0xAD1047E1", adjustInfo: [0, 2]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvaluationAdjustInfo {
    /// `evaluationAdustId` — hash identifiant unique du groupe d'ajustements.
    pub evaluation_adjust_id: HashId,
    /// `adjustInfo[0]` — index de début dans `m_EvaluationAdjustParamInfoList`.
    pub adjust_offset: i64,
    /// `adjustInfo[1]` — nombre de paramètres dans ce groupe.
    pub adjust_count: i64,
}

impl EvaluationAdjustInfo {
    /// Parse une entrée de `m_EvaluationAdjustInfoList`. `None` si `evaluationAdustId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let evaluation_adjust_id = field_hash(v, "evaluationAdustId");
        if evaluation_adjust_id.is_zero() {
            return None;
        }
        let (adjust_offset, adjust_count) = array2_i64(v, "adjustInfo");
        Some(Self {
            evaluation_adjust_id,
            adjust_offset,
            adjust_count,
        })
    }
}

// ─── TeamStrategyAiDataInfo ───────────────────────────────────────────────────

/// Stratégie disponible pour une équipe avec son niveau de déverrouillage (`TEAM_STRATEGY_AI_DATA_INFO`).
///
/// Entrée de `m_TeamStrategyAIDataInfoList` dans `strategy_ai_config_1.01.50.cfg.bin.json`.
///
/// Vérité terrain : `values[0] = {strategyId: "0x7C24BB03", enableLv: 1}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TeamStrategyAiDataInfo {
    /// `strategyId` — hash identifiant la stratégie.
    pub strategy_id: HashId,
    /// `enableLv` — niveau de déverrouillage requis (1 dans tous les dumps réels).
    pub enable_lv: i64,
}

impl TeamStrategyAiDataInfo {
    /// Parse une entrée de `m_TeamStrategyAIDataInfoList`. `None` si `strategyId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let strategy_id = field_hash(v, "strategyId");
        if strategy_id.is_zero() {
            return None;
        }
        Some(Self {
            strategy_id,
            enable_lv: field_i64(v, "enableLv").unwrap_or(0),
        })
    }
}

// ─── TeamStrategyAiInfo ───────────────────────────────────────────────────────

/// Groupe de stratégies d'équipe (`TEAM_STRATEGY_AI_INFO`).
///
/// Entrée de `m_TeamStrategyAIInfoList` dans `strategy_ai_config_1.01.50.cfg.bin.json`.
/// `strategy_offset`/`strategy_count` référencent une plage dans `m_TeamStrategyAIDataInfoList`.
///
/// Vérité terrain : ligne 3311, `values[0] = {groupId: "0x9714544E", strategyInfo: [0, 3]}`.
/// `values[4] = {groupId: "0x38CB4D40", strategyInfo: [43, 13]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TeamStrategyAiInfo {
    /// `groupId` — hash identifiant le groupe d'équipe.
    pub group_id: HashId,
    /// `strategyInfo[0]` — index de début dans `m_TeamStrategyAIDataInfoList`.
    pub strategy_offset: i64,
    /// `strategyInfo[1]` — nombre de stratégies dans ce groupe.
    pub strategy_count: i64,
}

impl TeamStrategyAiInfo {
    /// Parse une entrée de `m_TeamStrategyAIInfoList`. `None` si `groupId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let group_id = field_hash(v, "groupId");
        if group_id.is_zero() {
            return None;
        }
        let (strategy_offset, strategy_count) = array2_i64(v, "strategyInfo");
        Some(Self {
            group_id,
            strategy_offset,
            strategy_count,
        })
    }
}

// ─── StrategyAiConfig ─────────────────────────────────────────────────────────

/// Contenu complet de `strategy_ai_config_1.01.50.cfg.bin.json`.
///
/// Neuf listes couvrant les stratégies, les phases de jeu, les conditions et les ajustements
/// d'évaluation. Utiliser [`parse_strategy_ai_config`] pour construire.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StrategyAiConfig {
    /// `m_StrategyAITacticsInfoList` — 66 références de tactiques.
    pub tactics_infos: Vec<StrategyAiTacticsInfo>,
    /// `m_StrategyAIInfoList` — 62 stratégies IA.
    pub strategy_infos: Vec<StrategyAiInfo>,
    /// `m_ConditionIdList` — 182 conditions de stratégie (liste principale).
    pub condition_ids: Vec<ConditionIdInfo>,
    /// `m_ConditionIdList2` — 74 conditions de stratégie (liste secondaire).
    pub condition_ids2: Vec<ConditionIdInfo>,
    /// `m_PhaseCheckInfoList` — 60 vérifications de phase.
    pub phase_checks: Vec<PhaseCheckInfo>,
    /// `m_EvaluationAdjustParamInfoList` — 15 paramètres d'ajustement d'évaluation.
    pub eval_adjust_params: Vec<EvaluationAdjustParamInfo>,
    /// `m_EvaluationAdjustInfoList` — 14 groupes d'ajustements.
    pub eval_adjusts: Vec<EvaluationAdjustInfo>,
    /// `m_TeamStrategyAIDataInfoList` — 56 stratégies d'équipe avec niveaux.
    pub team_strategy_data: Vec<TeamStrategyAiDataInfo>,
    /// `m_TeamStrategyAIInfoList` — 5 groupes d'équipes.
    pub team_strategy_infos: Vec<TeamStrategyAiInfo>,
}

impl StrategyAiConfig {
    /// Renvoie les tactiques d'une stratégie (tranche dans `tactics_infos`).
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    #[must_use]
    pub fn tactics_of(&self, strategy: &StrategyAiInfo) -> &[StrategyAiTacticsInfo] {
        let start = strategy.tactics_offset as usize;
        let end = start.saturating_add(strategy.tactics_count as usize);
        self.tactics_infos.get(start..end).unwrap_or(&[])
    }

    /// Recherche une stratégie par son `strategy_id`. `None` si absente.
    #[must_use]
    pub fn find_strategy(&self, strategy_id: HashId) -> Option<&StrategyAiInfo> {
        self.strategy_infos
            .iter()
            .find(|s| s.strategy_id == strategy_id)
    }

    /// Renvoie les paramètres d'un groupe d'ajustements (tranche dans `eval_adjust_params`).
    #[must_use]
    pub fn adjust_params_of(&self, adj: &EvaluationAdjustInfo) -> &[EvaluationAdjustParamInfo] {
        let start = adj.adjust_offset as usize;
        let end = start.saturating_add(adj.adjust_count as usize);
        self.eval_adjust_params.get(start..end).unwrap_or(&[])
    }

    /// Renvoie les stratégies d'un groupe d'équipe (tranche dans `team_strategy_data`).
    #[must_use]
    pub fn strategy_data_of(&self, team: &TeamStrategyAiInfo) -> &[TeamStrategyAiDataInfo] {
        let start = team.strategy_offset as usize;
        let end = start.saturating_add(team.strategy_count as usize);
        self.team_strategy_data.get(start..end).unwrap_or(&[])
    }
}

/// Parse un `strategy_ai_config_1.01.50.cfg.bin.json` complet (9 listes).
///
/// Les entrées dont l'identifiant principal est nul ou absent sont silencieusement ignorées.
#[must_use]
pub fn parse_strategy_ai_config(root: &Value) -> StrategyAiConfig {
    StrategyAiConfig {
        tactics_infos: extract_list(
            root,
            "m_StrategyAITacticsInfoList",
            StrategyAiTacticsInfo::from_value,
        ),
        strategy_infos: extract_list(root, "m_StrategyAIInfoList", StrategyAiInfo::from_value),
        condition_ids: extract_list(root, "m_ConditionIdList", ConditionIdInfo::from_value),
        condition_ids2: extract_list(root, "m_ConditionIdList2", ConditionIdInfo::from_value),
        phase_checks: extract_list(root, "m_PhaseCheckInfoList", PhaseCheckInfo::from_value),
        eval_adjust_params: extract_list(
            root,
            "m_EvaluationAdjustParamInfoList",
            EvaluationAdjustParamInfo::from_value,
        ),
        eval_adjusts: extract_list(
            root,
            "m_EvaluationAdjustInfoList",
            EvaluationAdjustInfo::from_value,
        ),
        team_strategy_data: extract_list(
            root,
            "m_TeamStrategyAIDataInfoList",
            TeamStrategyAiDataInfo::from_value,
        ),
        team_strategy_infos: extract_list(
            root,
            "m_TeamStrategyAIInfoList",
            TeamStrategyAiInfo::from_value,
        ),
    }
}

/// Résout le **nom localisé** d'une stratégie IA via une liste `ai_text` (issue de
/// [`crate::text::parse_text_file`]).
///
/// `name_id` (var du `m_StrategyAIInfoList`) est la clé de jointure. Modèle de données validé
/// **end-to-end** (pas de parseur inagle pour ce config) ; `None` si absent.
#[must_use]
pub fn resolve_strategy_name<'a>(
    strategy: &StrategyAiInfo,
    ai_text: &'a [(HashId, String)],
) -> Option<&'a str> {
    crate::text::find_text(ai_text, strategy.name_id)
}

/// Résout la **description localisée** d'une stratégie IA via la **même** table `ai_text`
/// (clé `desc_id`). `None` si absent.
#[must_use]
pub fn resolve_strategy_description<'a>(
    strategy: &StrategyAiInfo,
    ai_text: &'a [(HashId, String)],
) -> Option<&'a str> {
    crate::text::find_text(ai_text, strategy.desc_id)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TACTICS_AI_CONFIG
// ═══════════════════════════════════════════════════════════════════════════════

// ─── TacticsAiTargetInfo ──────────────────────────────────────────────────────

/// Cible d'une tactique IA (`TACTICS_AI_TARGET_INFO`).
///
/// Entrée de `m_TacticsAITargetInfoList` dans `tactics_ai_config_0.06.44.cfg.bin.json`.
/// `func_name` est le nom C++ de la fonction de calcul de cible (conservé tel quel).
///
/// Vérité terrain : ligne 9, `values[0] = {targetId: "0x1C44B74C", funcName: "CalcTargetChara_None"}`.
/// `values[11] = {targetId: "0xA4F8D029", funcName: "CalcTargetPos_None"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TacticsAiTargetInfo {
    /// `targetId` — hash identifiant la cible.
    pub target_id: HashId,
    /// `funcName` — nom C++ de la fonction de calcul (ex. `"CalcTargetChara_Shoot"`).
    pub func_name: String,
}

impl TacticsAiTargetInfo {
    /// Parse une entrée de `m_TacticsAITargetInfoList`. `None` si `targetId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let target_id = field_hash(v, "targetId");
        if target_id.is_zero() {
            return None;
        }
        Some(Self {
            target_id,
            func_name: owned(field_str(v, "funcName").unwrap_or("")),
        })
    }
}

// ─── TacticsAiActionInfo ──────────────────────────────────────────────────────

/// Action d'une tactique IA (`TACTICS_AI_ACTION_INFO`).
///
/// Entrée de `m_TacticsAIActionInfoList` dans `tactics_ai_config_0.06.44.cfg.bin.json`.
/// `target_id` est un hash référençant un `TacticsAiTargetInfo`.
///
/// Vérité terrain : `values[0] = {actionId: 1, targetCharaType: 0, targetPosType: 10, targetId: "0xBDE3E168"}`.
/// `values[2] = {actionId: 9, targetCharaType: 0, targetPosType: 0, targetId: "0x00000000"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TacticsAiActionInfo {
    /// `actionId` — type d'action (1=déplacement, 9=action spéciale dans les dumps réels).
    pub action_id: i64,
    /// `targetCharaType` — type de personnage cible.
    pub target_chara_type: i64,
    /// `targetPosType` — type de position cible (0=aucune, 10=positionnement, 11=autour…).
    pub target_pos_type: i64,
    /// `targetId` — hash de la cible, lien vers `TacticsAiTargetInfo.target_id`.
    pub target_id: HashId,
}

impl TacticsAiActionInfo {
    /// Parse une entrée de `m_TacticsAIActionInfoList`. `None` si `actionId` absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            action_id: field_i64(v, "actionId")?,
            target_chara_type: field_i64(v, "targetCharaType").unwrap_or(0),
            target_pos_type: field_i64(v, "targetPosType").unwrap_or(0),
            target_id: field_hash(v, "targetId"),
        })
    }
}

// ─── TacticsAiInfo ────────────────────────────────────────────────────────────

/// Définition d'une tactique IA (`TACTICS_AI_INFO`).
///
/// Entrée de `m_TacticsAIInfoList` dans `tactics_ai_config_0.06.44.cfg.bin.json`.
/// `action_offset`/`action_count` référencent une plage dans `m_TacticsAIActionInfoList`
/// — résoudre via [`TacticsAiConfig::actions_of`].
///
/// Vérité terrain : `values[0] = {tacticsId: "0xD23CF887", nameId: "0xD4446BCA",
/// descId: "0x9D6D0350", actionInfo: [0, 1]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TacticsAiInfo {
    /// `tacticsId` — hash identifiant unique de la tactique.
    pub tactics_id: HashId,
    /// `nameId` — hash du nom affiché.
    pub name_id: HashId,
    /// `descId` — hash de la description.
    pub desc_id: HashId,
    /// `actionInfo[0]` — index de début dans `m_TacticsAIActionInfoList`.
    pub action_offset: i64,
    /// `actionInfo[1]` — nombre d'actions dans cette tactique.
    pub action_count: i64,
}

impl TacticsAiInfo {
    /// Parse une entrée de `m_TacticsAIInfoList`. `None` si `tacticsId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let tactics_id = field_hash(v, "tacticsId");
        if tactics_id.is_zero() {
            return None;
        }
        let (action_offset, action_count) = array2_i64(v, "actionInfo");
        Some(Self {
            tactics_id,
            name_id: field_hash(v, "nameId"),
            desc_id: field_hash(v, "descId"),
            action_offset,
            action_count,
        })
    }
}

// ─── CharaTacticsAiDataInfo ───────────────────────────────────────────────────

/// Tactique disponible pour un personnage avec niveau de déverrouillage (`CHARA_TACTICS_AI_DATA_INFO`).
///
/// Entrée de `m_CharaTacticsAIDataInfoList` dans `tactics_ai_config_0.06.44.cfg.bin.json`.
///
/// Vérité terrain : `values[0] = {tacticsId: "0x906D3411", enableLv: 1}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaTacticsAiDataInfo {
    /// `tacticsId` — hash identifiant la tactique.
    pub tactics_id: HashId,
    /// `enableLv` — niveau requis pour débloquer cette tactique.
    pub enable_lv: i64,
}

impl CharaTacticsAiDataInfo {
    /// Parse une entrée de `m_CharaTacticsAIDataInfoList`. `None` si `tacticsId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let tactics_id = field_hash(v, "tacticsId");
        if tactics_id.is_zero() {
            return None;
        }
        Some(Self {
            tactics_id,
            enable_lv: field_i64(v, "enableLv").unwrap_or(0),
        })
    }
}

// ─── CharaTacticsAiInfo ───────────────────────────────────────────────────────

/// Groupe de tactiques personnage (`CHARA_TACTICS_AI_INFO`).
///
/// Entrée de `m_CharaTacticsAIInfoList` dans `tactics_ai_config_0.06.44.cfg.bin.json`.
/// `tactics_offset`/`tactics_count` référencent une plage dans `m_CharaTacticsAIDataInfoList`.
///
/// Vérité terrain : ligne 1261, `values[0] = {groupId: "0x35664547", tacticsInfo: [0, 2]}`.
/// `values[4] = {groupId: "0xDA9C897B", tacticsInfo: [28, 2]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaTacticsAiInfo {
    /// `groupId` — hash identifiant le groupe de personnage.
    pub group_id: HashId,
    /// `tacticsInfo[0]` — index de début dans `m_CharaTacticsAIDataInfoList`.
    pub tactics_offset: i64,
    /// `tacticsInfo[1]` — nombre de tactiques dans ce groupe.
    pub tactics_count: i64,
}

impl CharaTacticsAiInfo {
    /// Parse une entrée de `m_CharaTacticsAIInfoList`. `None` si `groupId` nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let group_id = field_hash(v, "groupId");
        if group_id.is_zero() {
            return None;
        }
        let (tactics_offset, tactics_count) = array2_i64(v, "tacticsInfo");
        Some(Self {
            group_id,
            tactics_offset,
            tactics_count,
        })
    }
}

// ─── TacticsAiConfig ──────────────────────────────────────────────────────────

/// Contenu complet de `tactics_ai_config_0.06.44.cfg.bin.json`.
///
/// Cinq listes : cibles, actions, tactiques, données personnage, groupes personnage.
/// Utiliser [`parse_tactics_ai_config`] pour construire.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TacticsAiConfig {
    /// `m_TacticsAITargetInfoList` — 52 types de cibles avec noms de fonctions.
    pub target_infos: Vec<TacticsAiTargetInfo>,
    /// `m_TacticsAIActionInfoList` — 60 actions de tactiques.
    pub action_infos: Vec<TacticsAiActionInfo>,
    /// `m_TacticsAIInfoList` — 60 définitions de tactiques.
    pub tactics_infos: Vec<TacticsAiInfo>,
    /// `m_CharaTacticsAIDataInfoList` — 30 tactiques personnage avec niveaux.
    pub chara_tactics_data: Vec<CharaTacticsAiDataInfo>,
    /// `m_CharaTacticsAIInfoList` — 5 groupes de personnages.
    pub chara_tactics_infos: Vec<CharaTacticsAiInfo>,
}

impl TacticsAiConfig {
    /// Renvoie les actions d'une tactique (tranche dans `action_infos`).
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    #[must_use]
    pub fn actions_of(&self, tactics: &TacticsAiInfo) -> &[TacticsAiActionInfo] {
        let start = tactics.action_offset as usize;
        let end = start.saturating_add(tactics.action_count as usize);
        self.action_infos.get(start..end).unwrap_or(&[])
    }

    /// Recherche une cible par son `target_id`. `None` si absente.
    #[must_use]
    pub fn find_target(&self, target_id: HashId) -> Option<&TacticsAiTargetInfo> {
        self.target_infos.iter().find(|t| t.target_id == target_id)
    }

    /// Recherche une tactique par son `tactics_id`. `None` si absente.
    #[must_use]
    pub fn find_tactics(&self, tactics_id: HashId) -> Option<&TacticsAiInfo> {
        self.tactics_infos
            .iter()
            .find(|t| t.tactics_id == tactics_id)
    }

    /// Renvoie les tactiques d'un groupe personnage (tranche dans `chara_tactics_data`).
    #[must_use]
    pub fn tactics_data_of(&self, group: &CharaTacticsAiInfo) -> &[CharaTacticsAiDataInfo] {
        let start = group.tactics_offset as usize;
        let end = start.saturating_add(group.tactics_count as usize);
        self.chara_tactics_data.get(start..end).unwrap_or(&[])
    }
}

/// Parse un `tactics_ai_config_0.06.44.cfg.bin.json` complet (5 listes).
///
/// Les entrées dont l'identifiant principal est nul ou absent sont silencieusement ignorées.
#[must_use]
pub fn parse_tactics_ai_config(root: &Value) -> TacticsAiConfig {
    TacticsAiConfig {
        target_infos: extract_list(
            root,
            "m_TacticsAITargetInfoList",
            TacticsAiTargetInfo::from_value,
        ),
        action_infos: extract_list(
            root,
            "m_TacticsAIActionInfoList",
            TacticsAiActionInfo::from_value,
        ),
        tactics_infos: extract_list(root, "m_TacticsAIInfoList", TacticsAiInfo::from_value),
        chara_tactics_data: extract_list(
            root,
            "m_CharaTacticsAIDataInfoList",
            CharaTacticsAiDataInfo::from_value,
        ),
        chara_tactics_infos: extract_list(
            root,
            "m_CharaTacticsAIInfoList",
            CharaTacticsAiInfo::from_value,
        ),
    }
}
