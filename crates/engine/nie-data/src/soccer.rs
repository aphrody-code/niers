//! Famille `soccer` — port Rust des configs numériques de match IEVR.
//!
//! ## Périmètre porté dans cette version
//!
//! | Fichier | Format | Structs | Statut |
//! |---------|--------|---------|--------|
//! | `soccer_focus_battle_effect_config.cfg.bin.json` | `lists` | [`FocusBattleEffectConfig`] | FAIT |
//! | `soccer_technic_config.cfg.bin.json` | `lists` | [`TechnicConfig`] | FAIT |
//! | `soccer_basic_effect_config_1.01.79.00.cfg.bin.json` | `lists` | [`ScBasicEffectInfo`] | FAIT |
//! | `soccer_game_additional_config_1.04.14.00.cfg.bin.json` | `lists` | [`SoccerGameAdditionalConfig`] (3/17 listes) | PARTIEL |
//! | `soccer_game_config_plus_1.04.08.00.cfg.bin.json` | `entries` | [`SoccerGameConfig`] | FAIT |
//! | `soccer_game_config_1.04.08.00.cfg.bin.json` | `entries` | [`SoccerGameConfig`] (même parseur) | FAIT |
//!
//! ## Fichiers laissés (NON_FAIT dans ce PR)
//!
//! - `soccer_game_option.cfg.bin.json` (93 Ko) — options globales
//! - `soccer_camera_config_1.03.21.cfg.bin.json` (163 Ko) — caméra
//! - `soccer_command_effect_config_1.01.79.00.cfg.bin.json` (583 Ko) — effets de commande hissatsu
//! - `soccer_chara_placement_1.01.97.00.cfg.bin.json` (707 Ko) — placement de personnages
//! - `soccer_add_status_1.01.30.00.cfg.bin.json` (979 Ko) — statuts additionnels
//! - `soccer_game_additional_config` (14/17 listes non portées : casters, growths, ex-rules, etc.)
//! - `soccer_drop_config_*`, `soccer_prize_config_*`, `soccer_costume_config_*` — loot/UI
//! - `soccer_condition_cmd_config_*`, `soccer_common_trigger_*`, `soccer_quick_action_config_*`
//! - `soccer_chara_unique_rarity_config_*`, `soccer_studium_gimmick_config_*`
//! - `soccer_suggest_config_*` (5 versions), `soccer_rankmatch_*`, `soccer_scramble_config_*`
//! - `special_tactics_effect_config_*`, `super_tactics_effect_config_*`, `team_build_effect_config_*`
//! - `studium_gimmick_effect_config_*`, `soccer_performance_config_*`, `trick_config_*`
//!
//! ## Paramètres de match numériques identifiés
//!
//! | Fichier | Champ | Valeur | Rôle probable |
//! |---------|-------|--------|---------------|
//! | `soccer_focus_battle_effect_config` | `rangeParam1` de l'entrée active | 30 | Portée (rayon ?) du FocusBattle |
//! | `soccer_focus_battle_effect_config` | `rangeParam2` de l'entrée active | 10 | Second paramètre de zone |
//! | `soccer_technic_config` | `recastTime` des 8 techniques | 3–8 | Temps de rechargement en secondes |
//! | `soccer_technic_config` | `usableCondition[].param2` | 30–40 | Seuil de stat pour déclencher |
//! | `soccer_game_additional_config` | `m_SoccerGetExpDataTable[0].ratio` | 7.8 | Ratio XP victoire niveau 0 |
//! | `soccer_game_config` | `CPU_BEANS_RATE_INFO[0].rate` | 1.0 | Ratio ressources CPU (difficile=7) |
//! | `soccer_game_config` | `CPU_BEANS_RATE_INFO[3].rate` | 0.0 | CPU de type 4 sans ressources |

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, field_hash, field_i64, field_str, list_values, walk_named};
use crate::hash::HashId;

// ─── Utilitaire interne ────────────────────────────────────────────────────────

/// Parcourt une liste `lists[name].values` et applique `f` à chaque entrée.
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
// soccer_focus_battle_effect_config.cfg.bin.json
// Format : lists — 4 listes
// ═══════════════════════════════════════════════════════════════════════════════

/// Plage d'effet d'un combat de focus (`m_soccerFocusBattleEffectRangeList`, type `ScFbtlEffectRange`).
///
/// ## Vérité terrain
///
/// `soccer_focus_battle_effect_config.cfg.bin.json`, `m_soccerFocusBattleEffectRangeList[1]` :
/// `{ rangeType: 1, rangeParam1: 30, rangeParam2: 10, rangeParam3: 0 }`
///
/// 10 entrées au total ; l'entrée d'index 1 est la seule active (`rangeType=1`).
/// Les 9 autres ont `rangeType=0` et tous les paramètres à 0.
///
/// `rangeType=1` signifie probablement que la zone est active lors d'un FocusBattle ;
/// `rangeParam1=30` et `rangeParam2=10` sont des dimensions de zone non documentées
/// sans le header C++ `GDSSoccerFocusBattleEffectConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScFbtlEffectRange {
    /// `rangeType` — type de plage : 0 = inactif, 1 = actif.
    pub range_type: i64,
    /// `rangeParam1` — premier paramètre de zone (30 pour l'entrée active).
    ///
    /// Rôle probable : rayon ou dimension primaire de la zone du FocusBattle.
    pub range_param1: i64,
    /// `rangeParam2` — deuxième paramètre (10 pour l'entrée active).
    pub range_param2: i64,
    /// `rangeParam3` — toujours 0 dans le dump réel.
    pub range_param3: i64,
}

impl ScFbtlEffectRange {
    /// Parse une entrée de `m_soccerFocusBattleEffectRangeList`. `None` si la valeur est nulle.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            range_type: field_i64(v, "rangeType").unwrap_or(0),
            range_param1: field_i64(v, "rangeParam1").unwrap_or(0),
            range_param2: field_i64(v, "rangeParam2").unwrap_or(0),
            range_param3: field_i64(v, "rangeParam3").unwrap_or(0),
        })
    }

    /// `true` si cette plage est active (`rangeType != 0`).
    #[must_use]
    #[inline]
    pub fn is_active(&self) -> bool {
        self.range_type != 0
    }
}

/// Effet activé lors d'un combat de focus (`m_soccerFocusBattleActivatedEffectList`, type `ScFbtlActivatedEffect`).
///
/// ## Vérité terrain
///
/// `soccer_focus_battle_effect_config.cfg.bin.json`, `m_soccerFocusBattleActivatedEffectList[0]` :
/// `{ effectId: "0xBEBB2EE8", effectParam1..8: 0 }`
///
/// 10 entrées au total. Les 8 paramètres sont tous 0 dans le dump réel ; leurs rôles
/// sont inconnus sans le header `ScFbtlActivatedEffect`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScFbtlActivatedEffect {
    /// `effectId` — hash identifiant l'effet activé.
    ///
    /// Vérité terrain : `m_soccerFocusBattleActivatedEffectList[0].effectId` = `"0xBEBB2EE8"`.
    pub effect_id: HashId,
    /// `effectParam1..=8` — 8 paramètres numériques, tous 0 dans le dump réel.
    ///
    /// Signification inconnue sans le header C++ ; réservés pour paramétrage futur.
    pub effect_params: [i64; 8],
}

impl ScFbtlActivatedEffect {
    /// Parse une entrée de `m_soccerFocusBattleActivatedEffectList`. `None` si `effectId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let effect_id = field_hash(v, "effectId");
        if effect_id.is_zero() {
            return None;
        }
        Some(Self {
            effect_id,
            effect_params: [
                field_i64(v, "effectParam1").unwrap_or(0),
                field_i64(v, "effectParam2").unwrap_or(0),
                field_i64(v, "effectParam3").unwrap_or(0),
                field_i64(v, "effectParam4").unwrap_or(0),
                field_i64(v, "effectParam5").unwrap_or(0),
                field_i64(v, "effectParam6").unwrap_or(0),
                field_i64(v, "effectParam7").unwrap_or(0),
                field_i64(v, "effectParam8").unwrap_or(0),
            ],
        })
    }
}

/// Déclencheur de démonstration pour un combat de focus (`m_soccerFocusBattleDemoTriggerList`, type `ScFbtlDemoTrigger`).
///
/// ## Vérité terrain
///
/// `soccer_focus_battle_effect_config.cfg.bin.json`, `m_soccerFocusBattleDemoTriggerList[0]` :
/// `{ demoTriggerType: 3, demoTriggerParam1..3: 0, demoTriggerTextId: "0x8901F53D" }`
///
/// `demoTriggerType` : 0 = inactif, 2 = type 2, 3 = type 3.
/// Probablement l'événement déclenchant l'animation de démo associée au FocusBattle.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScFbtlDemoTrigger {
    /// `demoTriggerType` — type de déclencheur (0=inactif, 2..3=actifs).
    pub demo_trigger_type: i64,
    /// `demoTriggerParam1` — paramètre 1 (tous 0 dans le dump réel).
    pub demo_trigger_param1: i64,
    /// `demoTriggerParam2` — paramètre 2 (tous 0 dans le dump réel).
    pub demo_trigger_param2: i64,
    /// `demoTriggerParam3` — paramètre 3 (tous 0 dans le dump réel).
    pub demo_trigger_param3: i64,
    /// `demoTriggerTextId` — hash du texte associé à la démo.
    ///
    /// Vérité terrain : `m_soccerFocusBattleDemoTriggerList[0].demoTriggerTextId` = `"0x8901F53D"`.
    pub demo_trigger_text_id: HashId,
}

impl ScFbtlDemoTrigger {
    /// Parse une entrée de `m_soccerFocusBattleDemoTriggerList`.
    /// Renvoie toujours `Some` (même pour les entrées inactives avec type=0).
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            demo_trigger_type: field_i64(v, "demoTriggerType").unwrap_or(0),
            demo_trigger_param1: field_i64(v, "demoTriggerParam1").unwrap_or(0),
            demo_trigger_param2: field_i64(v, "demoTriggerParam2").unwrap_or(0),
            demo_trigger_param3: field_i64(v, "demoTriggerParam3").unwrap_or(0),
            demo_trigger_text_id: field_hash(v, "demoTriggerTextId"),
        })
    }
}

/// Association range/activated/demo pour un effet de combat de focus
/// (`m_soccerFocusBattleEffectInfoList`, type `ScFbtlEffectInfo`).
///
/// Chaque `ScFbtlEffectInfo` regroupe une plage, un effet activé et un déclencheur via
/// des paires `[offset, count]` indexant les listes correspondantes de [`FocusBattleEffectConfig`].
///
/// ## Vérité terrain
///
/// `soccer_focus_battle_effect_config.cfg.bin.json`, `m_soccerFocusBattleEffectInfoList[0]` :
/// `{ id: "0x64659E21", range: [0,1], activatedEffect: [0,1], demoTrigger: [0,1] }`
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScFbtlEffectInfo {
    /// `id` — hash identifiant cet effet de FocusBattle.
    ///
    /// Vérité terrain : `m_soccerFocusBattleEffectInfoList[0].id` = `"0x64659E21"`.
    /// Référencé par [`ScTechnicInfo::focus_battle_effect_id`].
    pub id: HashId,
    /// `range[0..=1]` — `[offset, count]` dans [`FocusBattleEffectConfig::ranges`].
    pub range: [i64; 2],
    /// `activatedEffect[0..=1]` — `[offset, count]` dans [`FocusBattleEffectConfig::activated_effects`].
    pub activated_effect: [i64; 2],
    /// `demoTrigger[0..=1]` — `[offset, count]` dans [`FocusBattleEffectConfig::demo_triggers`].
    pub demo_trigger: [i64; 2],
}

impl ScFbtlEffectInfo {
    /// Parse une entrée de `m_soccerFocusBattleEffectInfoList`. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        let arr2 = |key: &str| -> [i64; 2] {
            let a = v.get(key).and_then(Value::as_array);
            [
                a.and_then(|a| a.first())
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                a.and_then(|a| a.get(1))
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
            ]
        };
        Some(Self {
            id,
            range: arr2("range"),
            activated_effect: arr2("activatedEffect"),
            demo_trigger: arr2("demoTrigger"),
        })
    }
}

/// Config complète de `soccer_focus_battle_effect_config.cfg.bin.json` (4 listes).
///
/// Point d'entrée des paramètres numériques du combat de focus (état `SoccerPhase::FocusBattle`).
///
/// Utiliser [`parse_focus_battle_effect_config`] pour construire depuis un JSON désérialisé.
///
/// ## Comptes réels
///
/// 10 [`ScFbtlEffectRange`], 10 [`ScFbtlActivatedEffect`], 10 [`ScFbtlDemoTrigger`],
/// 10 [`ScFbtlEffectInfo`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FocusBattleEffectConfig {
    /// Plages d'effet (`m_soccerFocusBattleEffectRangeList`) — 10 entrées.
    pub ranges: Vec<ScFbtlEffectRange>,
    /// Effets activés (`m_soccerFocusBattleActivatedEffectList`) — 10 entrées.
    pub activated_effects: Vec<ScFbtlActivatedEffect>,
    /// Déclencheurs de démo (`m_soccerFocusBattleDemoTriggerList`) — 10 entrées.
    pub demo_triggers: Vec<ScFbtlDemoTrigger>,
    /// Associations d'effets (`m_soccerFocusBattleEffectInfoList`) — 10 entrées.
    pub effect_infos: Vec<ScFbtlEffectInfo>,
}

impl FocusBattleEffectConfig {
    /// Résout la [`ScFbtlEffectInfo`] associée à un `id`. `None` si absente.
    ///
    /// Utilisé pour retrouver les paramètres de zone d'une technique via son
    /// [`ScTechnicInfo::focus_battle_effect_id`].
    #[must_use]
    pub fn find_effect(&self, id: HashId) -> Option<&ScFbtlEffectInfo> {
        self.effect_infos.iter().find(|e| e.id == id)
    }

    /// Résout la plage d'un [`ScFbtlEffectInfo`] (tranche dans `self.ranges`).
    #[must_use]
    pub fn ranges_of(&self, info: &ScFbtlEffectInfo) -> &[ScFbtlEffectRange] {
        let start = info.range[0] as usize;
        let end = start.saturating_add(info.range[1] as usize);
        self.ranges.get(start..end).unwrap_or(&[])
    }
}

/// Parse un `soccer_focus_battle_effect_config.cfg.bin.json` complet.
///
/// Comptes réels (vérifiés sur le dump) :
/// 10 ranges + 10 activated effects + 10 demo triggers + 10 effect infos.
#[must_use]
pub fn parse_focus_battle_effect_config(root: &Value) -> FocusBattleEffectConfig {
    FocusBattleEffectConfig {
        ranges: extract_list(
            root,
            "m_soccerFocusBattleEffectRangeList",
            ScFbtlEffectRange::from_value,
        ),
        activated_effects: extract_list(
            root,
            "m_soccerFocusBattleActivatedEffectList",
            ScFbtlActivatedEffect::from_value,
        ),
        demo_triggers: extract_list(
            root,
            "m_soccerFocusBattleDemoTriggerList",
            ScFbtlDemoTrigger::from_value,
        ),
        effect_infos: extract_list(
            root,
            "m_soccerFocusBattleEffectInfoList",
            ScFbtlEffectInfo::from_value,
        ),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// soccer_technic_config.cfg.bin.json
// Format : lists — 2 listes
// ═══════════════════════════════════════════════════════════════════════════════

/// Condition d'utilisation d'une technique (`m_soccerTechnicUsableConditionList`, type `ScTechnicUsableCondition`).
///
/// ## Vérité terrain
///
/// `soccer_technic_config.cfg.bin.json`, `m_soccerTechnicUsableConditionList[0]` :
/// `{ type: 1, param1: 3, param2: 30, param3: 0 }`
///
/// `type=1` : condition basée sur une stat ; `type=2` : possession de balle.
/// `param1` probablement l'index de la stat cible, `param2` le seuil à atteindre (30 ou 40).
/// 15 entrées au total.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScTechnicUsableCondition {
    /// `type` — type de condition (1=stat requirement, 2=possession balle).
    pub cond_type: i64,
    /// `param1` — index de stat ou indicateur de possession.
    ///
    /// Vérité terrain : condition[0].param1 = 3 ; condition[1].param1 = 1.
    pub param1: i64,
    /// `param2` — seuil de la condition (30 ou 40 dans le dump réel).
    ///
    /// Rôle probable : valeur minimale de `param1` pour satisfaire la condition.
    pub param2: i64,
    /// `param3` — toujours 0 dans le dump réel.
    pub param3: i64,
}

impl ScTechnicUsableCondition {
    /// Parse une entrée de `m_soccerTechnicUsableConditionList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            cond_type: field_i64(v, "type").unwrap_or(0),
            param1: field_i64(v, "param1").unwrap_or(0),
            param2: field_i64(v, "param2").unwrap_or(0),
            param3: field_i64(v, "param3").unwrap_or(0),
        })
    }
}

/// Technique spéciale disponible en match (`m_soccerTechnicInfoList`, type `ScTechnicInfo`).
///
/// ## Vérité terrain
///
/// `soccer_technic_config.cfg.bin.json`, `m_soccerTechnicInfoList[0]` :
/// `{ id: "0x07857A3F", nameTextId: "0x7B5D17D1", descTextId: "0x1702D47E",`
/// `  recastTime: 5, usableCondition: [0,2], focusBattleEffectId: "0x8A6BFF0D" }`
///
/// 8 techniques au total. `recastTime` est le seul paramètre numérique de gameplay
/// directement utilisable dans `match_sim` (temps de rechargement en secondes).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScTechnicInfo {
    /// `id` — hash identifiant la technique.
    ///
    /// Vérité terrain : `m_soccerTechnicInfoList[0].id` = `"0x07857A3F"`.
    pub id: HashId,
    /// `nameTextId` — hash du texte de nom de la technique.
    pub name_text_id: HashId,
    /// `descTextId` — hash du texte de description.
    pub desc_text_id: HashId,
    /// `recastTime` — temps de rechargement en secondes de jeu (3 à 8 selon la technique).
    ///
    /// Vérité terrain : techniques 0..7 → recastTime = [5, 5, 5, 8, 3, 4, 3, 4].
    /// Paramètre de gameplay direct pour `match_sim` : détermine la fréquence d'usage.
    pub recast_time: i64,
    /// `usableCondition[0..=1]` — `[offset, count]` dans [`TechnicConfig::conditions`].
    ///
    /// Vérité terrain : technique 0 → `[0, 2]` (2 conditions depuis l'index 0).
    pub usable_condition: [i64; 2],
    /// `focusBattleEffectId` — hash référençant [`ScFbtlEffectInfo::id`] dans [`FocusBattleEffectConfig`].
    ///
    /// Vérité terrain : technique 0 → `"0x8A6BFF0D"`.
    pub focus_battle_effect_id: HashId,
}

impl ScTechnicInfo {
    /// Parse une entrée de `m_soccerTechnicInfoList`. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        let uc = v.get("usableCondition").and_then(Value::as_array);
        Some(Self {
            id,
            name_text_id: field_hash(v, "nameTextId"),
            desc_text_id: field_hash(v, "descTextId"),
            recast_time: field_i64(v, "recastTime").unwrap_or(0),
            usable_condition: [
                uc.and_then(|a| a.first())
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                uc.and_then(|a| a.get(1))
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
            ],
            focus_battle_effect_id: field_hash(v, "focusBattleEffectId"),
        })
    }
}

/// Config complète de `soccer_technic_config.cfg.bin.json` (2 listes).
///
/// Utiliser [`parse_technic_config`] pour construire depuis un JSON désérialisé.
///
/// ## Comptes réels
///
/// 15 [`ScTechnicUsableCondition`] + 8 [`ScTechnicInfo`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TechnicConfig {
    /// Conditions d'utilisation (`m_soccerTechnicUsableConditionList`) — 15 entrées.
    pub conditions: Vec<ScTechnicUsableCondition>,
    /// Techniques (`m_soccerTechnicInfoList`) — 8 entrées.
    pub technics: Vec<ScTechnicInfo>,
}

impl TechnicConfig {
    /// Résout les conditions d'utilisation d'une technique (tranche dans `self.conditions`).
    #[must_use]
    pub fn conditions_of(&self, technic: &ScTechnicInfo) -> &[ScTechnicUsableCondition] {
        let start = technic.usable_condition[0] as usize;
        let end = start.saturating_add(technic.usable_condition[1] as usize);
        self.conditions.get(start..end).unwrap_or(&[])
    }

    /// Recherche une technique par son `id`. `None` si absente.
    #[must_use]
    pub fn find_technic(&self, id: HashId) -> Option<&ScTechnicInfo> {
        self.technics.iter().find(|t| t.id == id)
    }
}

/// Parse un `soccer_technic_config.cfg.bin.json` complet.
///
/// Comptes réels : 15 conditions + 8 techniques.
#[must_use]
pub fn parse_technic_config(root: &Value) -> TechnicConfig {
    TechnicConfig {
        conditions: extract_list(
            root,
            "m_soccerTechnicUsableConditionList",
            ScTechnicUsableCondition::from_value,
        ),
        technics: extract_list(root, "m_soccerTechnicInfoList", ScTechnicInfo::from_value),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// soccer_basic_effect_config_1.01.79.00.cfg.bin.json
// Format : lists — 1 liste
// ═══════════════════════════════════════════════════════════════════════════════

/// Effet de base du soccer — action hissatsu ou commande de match
/// (`m_soccerBasicEffectInfoList`, type `ScBasicEffectInfo`).
///
/// ## Vérité terrain
///
/// `soccer_basic_effect_config_1.01.79.00.cfg.bin.json`, `m_soccerBasicEffectInfoList[0]` :
/// `{ id: "0x83DB0758", funcName: "0xC9D604D4", buildIconType: 6 }`
///
/// 102 entrées au total. `buildIconType=6` pour toutes les entrées du dump réel.
/// `funcName` est le hash du nom de la fonction d'effet — référence vers le registre CMD.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScBasicEffectInfo {
    /// `id` — hash identifiant l'effet de base.
    ///
    /// Vérité terrain : `m_soccerBasicEffectInfoList[0].id` = `"0x83DB0758"`.
    pub id: HashId,
    /// `funcName` — hash du nom de la fonction d'effet (référence CMD).
    ///
    /// Vérité terrain : `m_soccerBasicEffectInfoList[0].funcName` = `"0xC9D604D4"`.
    pub func_name: HashId,
    /// `buildIconType` — type d'icône de construction (6 pour toutes les entrées du dump réel).
    pub build_icon_type: i64,
}

impl ScBasicEffectInfo {
    /// Parse une entrée de `m_soccerBasicEffectInfoList`. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            func_name: field_hash(v, "funcName"),
            build_icon_type: field_i64(v, "buildIconType").unwrap_or(0),
        })
    }
}

/// Parse un `soccer_basic_effect_config_*.cfg.bin.json`.
///
/// Renvoie les 102 [`ScBasicEffectInfo`] valides (id non-nul).
#[must_use]
pub fn parse_basic_effect_config(root: &Value) -> Vec<ScBasicEffectInfo> {
    extract_list(
        root,
        "m_soccerBasicEffectInfoList",
        ScBasicEffectInfo::from_value,
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// soccer_game_additional_config_1.04.14.00.cfg.bin.json
// Format : lists — 3/17 listes portées
// ═══════════════════════════════════════════════════════════════════════════════

/// Données IA d'une équipe CPU (`m_SoccerTeamAIDataList`, type `SOCCER_TEAM_AI_DATA`).
///
/// ## Vérité terrain
///
/// `soccer_game_additional_config_1.04.14.00.cfg.bin.json`, `m_SoccerTeamAIDataList[0]` :
/// `{ id: "0x6FF00159", quoteId: "0x00000000", paramData: "01010101", strategyId: "0x1CE5F4E2" }`
///
/// 9 entrées au total. `paramData` est une chaîne hex de 4 octets encodant les paramètres IA
/// (ex. `"01010101"` = niveau 1 pour les 4 axes, `"04010101"` = niveau 4 pour l'axe principal).
/// Le mapping des axes est inconnu sans le header C++ `GDSSoccerTeamAIData`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerTeamAiData {
    /// `id` — hash identifiant cette configuration IA d'équipe.
    pub id: HashId,
    /// `quoteId` — référence vers une configuration parente (0 = aucune).
    pub quote_id: HashId,
    /// `paramData` — 4 octets hex de paramètres IA (ex. `"01010101"`).
    ///
    /// Rôle probable : 4 niveaux d'agressivité/défense/pression/vitesse de l'IA.
    pub param_data: String,
    /// `strategyId` — hash de la stratégie associée (`"0x00000000"` si héritée de `quoteId`).
    pub strategy_id: HashId,
}

impl SoccerTeamAiData {
    /// Parse une entrée de `m_SoccerTeamAIDataList`. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            quote_id: field_hash(v, "quoteId"),
            param_data: field_str(v, "paramData").unwrap_or("").into(),
            strategy_id: field_hash(v, "strategyId"),
        })
    }
}

/// Ratio d'XP obtenu selon l'écart de niveau (`m_SoccerGetExpDataTable`, type `SOCCER_GET_EXP_DATA`).
///
/// ## Vérité terrain
///
/// `soccer_game_additional_config_1.04.14.00.cfg.bin.json`, `m_SoccerGetExpDataTable[0]` :
/// `{ id: 0, ratio: 7.8 }` (niveau égal → ratio 7.8)
/// `m_SoccerGetExpDataTable[16]` : `{ id: 16, ratio: 4.0 }` (écart max → ratio 4.0)
///
/// 17 entrées (id 0..=16). `ratio` diminue avec l'écart : 7.8 → 4.0.
/// Utilisé dans `match_sim` pour calculer l'XP post-match selon l'écart de niveaux.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerGetExpData {
    /// `id` — index 0-basé (0 = même niveau, 16 = écart maximal).
    pub id: i64,
    /// `ratio` — multiplicateur d'XP (7.8 pour id=0, 4.0 pour id=16).
    ///
    /// Paramètre de gameplay : `xp_gain = base_xp * ratio`.
    pub ratio: f64,
}

impl SoccerGetExpData {
    /// Parse une entrée de `m_SoccerGetExpDataTable`.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            id: field_i64(v, "id").unwrap_or(0),
            ratio: v.get("ratio").and_then(Value::as_f64).unwrap_or(0.0),
        })
    }
}

/// XP accordé pour une "belle action" en match (`m_SoccerNicePlayExpDataList`, type `SOCCER_NICE_PLAY_EXP_DATA`).
///
/// ## Vérité terrain
///
/// `soccer_game_additional_config_1.04.14.00.cfg.bin.json`, `m_SoccerNicePlayExpDataList[0]` :
/// `{ id: "0x439CE11B", shootExp: 10, skillExp: 10, focusExp: 10, scrambleExp: 10 }`
///
/// 7 entrées au total. Les XP varient de 10 à 40 selon la rareté de l'action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerNicePlayExpData {
    /// `id` — hash identifiant le niveau de belle action.
    pub id: HashId,
    /// `shootExp` — XP de tir accordé.
    pub shoot_exp: i64,
    /// `skillExp` — XP de skill accordé.
    pub skill_exp: i64,
    /// `focusExp` — XP de focus accordé.
    pub focus_exp: i64,
    /// `scrambleExp` — XP de scramble accordé.
    pub scramble_exp: i64,
}

impl SoccerNicePlayExpData {
    /// Parse une entrée de `m_SoccerNicePlayExpDataList`. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            shoot_exp: field_i64(v, "shootExp").unwrap_or(0),
            skill_exp: field_i64(v, "skillExp").unwrap_or(0),
            focus_exp: field_i64(v, "focusExp").unwrap_or(0),
            scramble_exp: field_i64(v, "scrambleExp").unwrap_or(0),
        })
    }
}

/// Config additionnelle partielle de `soccer_game_additional_config_*.cfg.bin.json`.
///
/// **Périmètre porté : 3/17 listes** (les plus utiles pour la simulation de match).
///
/// Listes non portées : casters, caster settings, away uniforms, growth data/list,
/// ex-own/opp placements, ex-rules, soccer game ex, end event infos/chara listes,
/// soccer training end event.
///
/// Utiliser [`parse_soccer_game_additional_config`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerGameAdditionalConfig {
    /// Données IA des équipes (`m_SoccerTeamAIDataList`) — 9 entrées.
    pub team_ai_data: Vec<SoccerTeamAiData>,
    /// Table de ratios d'XP (`m_SoccerGetExpDataTable`) — 17 entrées.
    pub get_exp_table: Vec<SoccerGetExpData>,
    /// XP de belles actions (`m_SoccerNicePlayExpDataList`) — 7 entrées.
    pub nice_play_exp: Vec<SoccerNicePlayExpData>,
}

impl SoccerGameAdditionalConfig {
    /// Cherche le ratio d'XP pour un écart de niveau donné. `None` si `id` hors plage.
    #[must_use]
    pub fn exp_ratio(&self, level_gap: i64) -> Option<f64> {
        self.get_exp_table
            .iter()
            .find(|e| e.id == level_gap)
            .map(|e| e.ratio)
    }
}

/// Parse un `soccer_game_additional_config_*.cfg.bin.json` (3 listes portées).
///
/// Comptes réels : 9 team AI + 17 exp table + 7 nice play exp.
#[must_use]
pub fn parse_soccer_game_additional_config(root: &Value) -> SoccerGameAdditionalConfig {
    SoccerGameAdditionalConfig {
        team_ai_data: extract_list(root, "m_SoccerTeamAIDataList", SoccerTeamAiData::from_value),
        get_exp_table: extract_list(
            root,
            "m_SoccerGetExpDataTable",
            SoccerGetExpData::from_value,
        ),
        nice_play_exp: extract_list(
            root,
            "m_SoccerNicePlayExpDataList",
            SoccerNicePlayExpData::from_value,
        ),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// soccer_game_config_*.cfg.bin.json (plus + full)
// Format : entries — 3 listes positionnelles
// ═══════════════════════════════════════════════════════════════════════════════

/// Ratio de ressources CPU selon le type (`CPU_BEANS_RATE_INFO_*`).
///
/// ## Vérité terrain
///
/// `soccer_game_config_1.04.08.00.cfg.bin.json`, `CPU_BEANS_RATE_INFO_LIST_BEG_0` :
/// 7 entrées (kind 1–7). Valeurs : 1=1.0, 2=0.5, 3=0.5, 4=0.0, 5=0.5, 6=0.5, 7=1.0.
///
/// `rate` est un multiplicateur de ressources pour le CPU IA selon son type de difficulté.
/// Type 4 (`rate=0.0`) : le CPU ne reçoit pas de ressources pour ce type.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CpuBeansRateInfo {
    /// var\[0\] — type de ressource (1-based, 1..=7 dans le dump réel).
    pub kind: i64,
    /// var\[1\] — ratio (Float : 1.0, 0.5 ou 0.0).
    ///
    /// Vérité terrain : `CPU_BEANS_RATE_INFO_0` = `[1, 1]` → kind=1, rate=1.0.
    /// `CPU_BEANS_RATE_INFO_3` = `[4, 0]` → kind=4, rate=0.0.
    pub rate: f64,
}

impl CpuBeansRateInfo {
    /// Parse un noeud `CPU_BEANS_RATE_INFO_N`. `None` si `kind` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let kind = node.int(0);
        if kind == 0 {
            return None;
        }
        Some(Self {
            kind,
            rate: node.var(1).map_or(0.0_f64, |v| v.as_f64()),
        })
    }
}

/// Paramètres de difficulté d'un match (`SOCCER_GAME_DIFFICULTY_N`).
///
/// ## Vérité terrain
///
/// `soccer_game_config_1.04.08.00.cfg.bin.json`, `SOCCER_GAME_DIFFICULTY_0` : 46 variables.
/// - var\[0\] = 1 → `difficulty_id`.
/// - var\[2\] = 23 (full) / 40 (plus) — paramètre opaque.
/// - var\[6\] = 20 (full) — paramètre opaque.
/// - var\[9\] = 1027 (full) — paramètre opaque.
/// - var\[20\] = 30 — paramètre opaque commun aux deux fichiers.
/// - var\[35\] = 1 (full) / 13 (plus) — paramètre opaque.
///
/// Le schéma positionnel est entièrement opaque sans le header C++ `GDSSoccerGameConfig`.
/// 1772 entrées dans le fichier complet (une par configuration de match), 1 dans `_plus`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerGameDifficulty {
    /// var\[0\] — identifiant de difficulté (ex. 1 pour le premier niveau).
    pub difficulty_id: i64,
    /// var\[1\..=45\] — 45 paramètres opaques (sans noms sans le header C++).
    ///
    /// Indices notables (heuristiques, non confirmées) :
    /// - raw\[1\] (var\[2\]) : semble varier par profil de difficulté (23 vs 40).
    /// - raw\[19\] (var\[20\]) = 30 : commun aux deux fichiers — seuil possible.
    ///
    /// `Vec` (longueur fixe 45) plutôt que `[i64; 45]` : serde ne dérive `Deserialize`
    /// que pour les tableaux jusqu'à 32 éléments.
    pub raw: Vec<i64>,
}

impl SoccerGameDifficulty {
    /// Parse un noeud `SOCCER_GAME_DIFFICULTY_N`. `None` si `difficulty_id` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let difficulty_id = node.int(0);
        if difficulty_id == 0 {
            return None;
        }
        let raw: Vec<i64> = (0..45).map(|i| node.int(i + 1)).collect();
        Some(Self { difficulty_id, raw })
    }
}

/// Informations d'un match (`SOCCER_GAME_INFO_N` + `SOCCER_GAME_INFO_REF_DIFFICULTY_N`).
///
/// ## Vérité terrain
///
/// `soccer_game_config_1.04.08.00.cfg.bin.json`, `SOCCER_GAME_INFO_0` :
/// - var\[0\] = -778368270 → `match_id` = `0xD19B0AF2`.
/// - var\[1\] = `"fbtl_st_0101"` → `id_str` (identifiant lisible du match FocusBattle).
/// - var\[2\] = 1 → `raw[0]` (type ou index).
/// - var\[3\] = -724197456 → `raw[1]` = `0xD4D59FB0`.
/// - `SOCCER_GAME_INFO_REF_DIFFICULTY_0.var[0]` = 0 → `difficulty_index` = 0
///   (référence vers `SoccerGameConfig::difficulties[0]`).
///
/// 325 entrées dans le fichier complet, 1 dans `_plus`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerGameInfo {
    /// var\[0\] — hash identifiant le match.
    ///
    /// Vérité terrain : `SOCCER_GAME_INFO_0` (full) = -778368270 → `0xD19B0AF2`.
    pub match_id: HashId,
    /// var\[1\] — identifiant lisible du match (ex. `"fbtl_st_0101"` = FocusBattle scène 01).
    pub id_str: String,
    /// var\[2..=9\] — 8 champs additionnels positionnels (opaques sans le header C++).
    pub raw: [i64; 8],
    /// `SOCCER_GAME_INFO_REF_DIFFICULTY_N.var[0]` — index dans [`SoccerGameConfig::difficulties`].
    ///
    /// Vérité terrain : `SOCCER_GAME_INFO_REF_DIFFICULTY_0.var[0]` = 0.
    pub difficulty_index: i64,
}

/// Config complète de `soccer_game_config_*.cfg.bin.json`.
///
/// Même parseur pour `soccer_game_config_plus_1.04.08.00.cfg.bin.json` (1 difficulté, 1 match)
/// et `soccer_game_config_1.04.08.00.cfg.bin.json` (7 CPU beans, 1772 difficultés, 325 matches).
///
/// Utiliser [`parse_soccer_game_config`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerGameConfig {
    /// Ratios de ressources CPU (`CPU_BEANS_RATE_INFO_*`) — 0 (plus) ou 7 (full) entrées.
    pub cpu_beans_rates: Vec<CpuBeansRateInfo>,
    /// Profils de difficulté (`SOCCER_GAME_DIFFICULTY_*`) — 1 (plus) ou 1772 (full) entrées.
    pub difficulties: Vec<SoccerGameDifficulty>,
    /// Configurations de match (`SOCCER_GAME_INFO_*`) — 1 (plus) ou 325 (full) entrées.
    pub game_infos: Vec<SoccerGameInfo>,
}

impl SoccerGameConfig {
    /// Résout la difficulté associée à un match. `None` si `difficulty_index` hors plage.
    #[must_use]
    pub fn difficulty_of(&self, info: &SoccerGameInfo) -> Option<&SoccerGameDifficulty> {
        self.difficulties.get(info.difficulty_index as usize)
    }

    /// Recherche un match par son `match_id`. `None` si absent.
    #[must_use]
    pub fn find_game(&self, match_id: HashId) -> Option<&SoccerGameInfo> {
        self.game_infos.iter().find(|g| g.match_id == match_id)
    }
}

/// Parse un `soccer_game_config_*.cfg.bin.json` (format `entries`).
///
/// Compatible avec `soccer_game_config_plus_1.04.08.00.cfg.bin.json` et
/// `soccer_game_config_1.04.08.00.cfg.bin.json`.
///
/// Comptes réels :
/// - `_plus` : 0 CPU beans, 1 difficulté, 1 match.
/// - fichier complet : 7 CPU beans, 1772 difficultés, 325 matches.
#[must_use]
pub fn parse_soccer_game_config(root: &Value) -> SoccerGameConfig {
    let mut cpu_beans_rates = Vec::new();
    walk_named(root, "CPU_BEANS_RATE_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(info) = CpuBeansRateInfo::from_node(node) {
            cpu_beans_rates.push(info);
        }
    });

    let mut difficulties = Vec::new();
    walk_named(root, "SOCCER_GAME_DIFFICULTY_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(d) = SoccerGameDifficulty::from_node(node) {
            difficulties.push(d);
        }
    });

    // SOCCER_GAME_INFO_LIST_BEG_0 contient des paires alternées :
    // SOCCER_GAME_INFO_N + SOCCER_GAME_INFO_REF_DIFFICULTY_N.
    // On parcourt le noeud LIST et on traite les enfants par paires.
    let mut game_infos: Vec<SoccerGameInfo> = Vec::new();
    walk_named(root, "SOCCER_GAME_INFO_LIST_BEG_", |list_node| {
        let children = list_node.children();
        let mut i = 0;
        while i < children.len() {
            let child = children[i];
            let cname = child.name();
            // Noeud INFO (pas REF) : commence par "SOCCER_GAME_INFO_" et ne contient pas "REF"
            if cname.starts_with("SOCCER_GAME_INFO_") && !cname.contains("REF") {
                let match_id = child.hash(0);
                if !match_id.is_zero() {
                    let id_str: String = child.string(1).into();
                    let mut raw = [0i64; 8];
                    for (k, slot) in raw.iter_mut().enumerate() {
                        *slot = child.int(k + 2);
                    }
                    // Le noeud suivant doit être le REF_DIFFICULTY
                    let difficulty_index = if i + 1 < children.len() {
                        let ref_node = children[i + 1];
                        if ref_node.name().contains("REF_DIFFICULTY") {
                            ref_node.int(0)
                        } else {
                            0
                        }
                    } else {
                        0
                    };
                    game_infos.push(SoccerGameInfo {
                        match_id,
                        id_str,
                        raw,
                        difficulty_index,
                    });
                }
            }
            i += 1;
        }
    });

    SoccerGameConfig {
        cpu_beans_rates,
        difficulties,
        game_infos,
    }
}

// ─── Tests unitaires ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fbtl_effect_range_active() {
        let v = json!({ "rangeType": 1, "rangeParam1": 30, "rangeParam2": 10, "rangeParam3": 0 });
        let r = ScFbtlEffectRange::from_value(&v).unwrap();
        assert_eq!(r.range_type, 1);
        assert_eq!(r.range_param1, 30);
        assert_eq!(r.range_param2, 10);
        assert!(r.is_active());
    }

    #[test]
    fn fbtl_effect_range_inactive() {
        let v = json!({ "rangeType": 0, "rangeParam1": 0, "rangeParam2": 0, "rangeParam3": 0 });
        let r = ScFbtlEffectRange::from_value(&v).unwrap();
        assert!(!r.is_active());
    }

    #[test]
    fn fbtl_activated_effect_id_nul_rejete() {
        let v = json!({ "effectId": "0x00000000" });
        assert!(ScFbtlActivatedEffect::from_value(&v).is_none());
    }

    #[test]
    fn fbtl_demo_trigger_type3() {
        let v = json!({
            "demoTriggerType": 3,
            "demoTriggerParam1": 0,
            "demoTriggerParam2": 0,
            "demoTriggerParam3": 0,
            "demoTriggerTextId": "0x8901F53D"
        });
        let t = ScFbtlDemoTrigger::from_value(&v).unwrap();
        assert_eq!(t.demo_trigger_type, 3);
        assert_eq!(t.demo_trigger_text_id, HashId(0x8901_F53D));
    }

    #[test]
    fn fbtl_effect_info_arrays() {
        let v = json!({
            "id": "0x64659E21",
            "range": [0, 1],
            "activatedEffect": [0, 1],
            "demoTrigger": [0, 1]
        });
        let info = ScFbtlEffectInfo::from_value(&v).unwrap();
        assert_eq!(info.id, HashId(0x6465_9E21));
        assert_eq!(info.range, [0, 1]);
        assert_eq!(info.activated_effect, [0, 1]);
        assert_eq!(info.demo_trigger, [0, 1]);
    }

    #[test]
    fn technic_info_recast_time() {
        let v = json!({
            "id": "0x07857A3F",
            "nameTextId": "0x7B5D17D1",
            "descTextId": "0x1702D47E",
            "recastTime": 5,
            "usableCondition": [0, 2],
            "focusBattleEffectId": "0x8A6BFF0D"
        });
        let t = ScTechnicInfo::from_value(&v).unwrap();
        assert_eq!(t.id, HashId(0x0785_7A3F));
        assert_eq!(t.recast_time, 5);
        assert_eq!(t.usable_condition, [0, 2]);
        assert_eq!(t.focus_battle_effect_id, HashId(0x8A6B_FF0D));
    }

    #[test]
    fn basic_effect_info_parse() {
        let v = json!({ "id": "0x83DB0758", "funcName": "0xC9D604D4", "buildIconType": 6 });
        let b = ScBasicEffectInfo::from_value(&v).unwrap();
        assert_eq!(b.id, HashId(0x83DB_0758));
        assert_eq!(b.func_name, HashId(0xC9D6_04D4));
        assert_eq!(b.build_icon_type, 6);
    }

    #[test]
    fn team_ai_data_param_data() {
        let v = json!({
            "id": "0x6FF00159",
            "quoteId": "0x00000000",
            "paramData": "01010101",
            "strategyId": "0x1CE5F4E2"
        });
        let ai = SoccerTeamAiData::from_value(&v).unwrap();
        assert_eq!(ai.id, HashId(0x6FF0_0159));
        assert_eq!(ai.param_data, "01010101");
        assert_eq!(ai.strategy_id, HashId(0x1CE5_F4E2));
    }

    #[test]
    fn get_exp_data_ratio() {
        let v = json!({ "id": 0, "ratio": 7.8 });
        let e = SoccerGetExpData::from_value(&v).unwrap();
        assert_eq!(e.id, 0);
        assert!((e.ratio - 7.8_f64).abs() < 1e-9);
    }

    #[test]
    fn soccer_game_config_parse_entries() {
        let root = json!({
            "entries": [
                {
                    "name": "SOCCER_GAME_DIFFICULTY_LIST_BEG_0",
                    "variables": [{"type": "Int", "value": "1"}],
                    "children": [
                        {
                            "name": "SOCCER_GAME_DIFFICULTY_0",
                            "variables": [
                                {"type": "Int", "value": "1"},
                                {"type": "Int", "value": "42"}
                            ],
                            "children": []
                        }
                    ]
                },
                {
                    "name": "SOCCER_GAME_INFO_LIST_BEG_0",
                    "variables": [{"type": "Int", "value": "1"}, {"type": "Int", "value": "1"}],
                    "children": [
                        {
                            "name": "SOCCER_GAME_INFO_0",
                            "variables": [
                                {"type": "Int", "value": "-1344098363"},
                                {"type": "String", "value": "fbtl_st_0904"},
                                {"type": "Int", "value": "1"}
                            ],
                            "children": []
                        },
                        {
                            "name": "SOCCER_GAME_INFO_REF_DIFFICULTY_0",
                            "variables": [
                                {"type": "Int", "value": "0"},
                                {"type": "Int", "value": "1"}
                            ],
                            "children": []
                        }
                    ]
                }
            ]
        });
        let cfg = parse_soccer_game_config(&root);
        assert_eq!(cfg.difficulties.len(), 1);
        assert_eq!(cfg.difficulties[0].difficulty_id, 1);
        assert_eq!(cfg.difficulties[0].raw[0], 42);
        assert_eq!(cfg.game_infos.len(), 1);
        // -1344098363 as u32 = 0xAFE2AFC5
        assert_eq!(cfg.game_infos[0].match_id, HashId(0xAFE2_AFC5));
        assert_eq!(cfg.game_infos[0].id_str, "fbtl_st_0904");
        assert_eq!(cfg.game_infos[0].difficulty_index, 0);
    }

    #[test]
    fn difficulty_of_resolution() {
        let cfg = SoccerGameConfig {
            cpu_beans_rates: Vec::new(),
            difficulties: alloc::vec![SoccerGameDifficulty {
                difficulty_id: 1,
                raw: alloc::vec![0i64; 45]
            }],
            game_infos: alloc::vec![SoccerGameInfo {
                match_id: HashId(1),
                id_str: String::from("test"),
                raw: [0i64; 8],
                difficulty_index: 0,
            }],
        };
        let info = &cfg.game_infos[0];
        let diff = cfg
            .difficulty_of(info)
            .expect("difficulty_of doit résoudre");
        assert_eq!(diff.difficulty_id, 1);
    }
}
