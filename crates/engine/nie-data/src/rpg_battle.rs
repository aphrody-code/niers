//! Famille `rpg_battle` — port Rust des configs de combat RPG (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! Tous les fichiers source se trouvent dans :
//! `/home/ubuntu/niers/data/common/gamedata/rpg_battle/`
//!
//! ## Périmètre porté (8 parseurs, FAIT)
//!
//! | Fichier | Format | Struct(s) | Comptes réels |
//! |---------|--------|-----------|---------------|
//! | `rpg_battle_rule_config_1.02.84.00.cfg.bin.json` | `lists` | [`RpgBattleRuleConfig`] | 2 start + 13 battle |
//! | `rpg_battle_status_pattern_config_1.03.17.00.cfg.bin.json` | `lists` | [`RpgBattleStatusPatternInfo`] | 28 |
//! | `rpg_battle_chara_swap_motion_config_1.04.06.00.cfg.bin.json` | `lists` | [`RpgBattleCharaSwapMotionConfig`] | 10 swap + 2 chara |
//! | `rpg_battle_party_config_0.00.00.cfg.bin.json` | `entries` | [`RpgBattlePartyConfig`] | 4 disable + 10 parties |
//! | `rpg_battle_cmd_event_config_0.00.00.cfg.bin.json` | `entries` | `Vec<HashId>` | 1 |
//! | `rpg_battle_cmd_obj_config_0.00.00.cfg.bin.json` | `entries` | `Vec<HashId>` | 1 |
//! | `rpg_battle_cmd_set_config_1.01.31.00.cfg.bin.json` | `entries` | [`RpgBattleCmdSetConfig`] | 1 128 cmd_data + 186 sets |
//! | `rpg_battle_add_status_config_0.00.00.cfg.bin.json` | `entries` | [`RpgBattleAddStatusConfig`] | 11 types + 6 infos + 1 attr |
//!
//! ## Fichiers NON_FAIT (hors périmètre de ce module)
//!
//! - `rpg_battle_cmd_config_1.02.82.00.cfg.bin.json` (7,7 Mo — 229 CMD_ACTION_INFO, voir [`crate::command`])
//! - `rpg_battle_result_config_1.00.08.cfg.bin.json` + `rpg_battle_result_config_.cfg.bin.json` (résultats de combat)
//! - `rpg_battle_food_config_1.02.98.00.cfg.bin.json` (nourriture en combat RPG)
//! - `rpg_battle_message_config_1.00.11.cfg.bin.json` (messages, 141 Ko)
//! - `rpg_battle_scramble_config_1.02.50.cfg.bin.json` (scramble battles, 113 Ko)
//! - `rpg_battle_dance_battle_config_1.01.78.00.cfg.bin.json` (danse, 129 Ko)
//! - `rpg_battle_formation_config_1.02.21.00.cfg.bin.json` (233 placements + 44 formations RPG, 7 vars/slot)
//! - `rpg_battle_reaction_config_0.00.00.cfg.bin.json` (réactions, 35 Ko)
//! - Tous les training configs (dribble, lifting, sled_dash, stairs_race, tire, zigzagdribble, special)

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, list_values, walk_named};
use crate::hash::HashId;

// ─── Utilitaire interne ────────────────────────────────────────────────────────

/// Parcourt une liste `lists[name].values` et applique `f` à chaque entrée.
fn map_list<T, F>(root: &Value, name: &str, f: F) -> Vec<T>
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

/// Lit un tableau JSON `[i64, i64]` depuis un champ nommé, renvoie `[0, 0]` si absent.
fn read_i64_pair(v: &Value, key: &str) -> [i64; 2] {
    let arr = match v.get(key).and_then(Value::as_array) {
        Some(a) => a,
        None => return [0, 0],
    };
    let v0 = arr.first().and_then(Value::as_i64).unwrap_or(0);
    let v1 = arr.get(1).and_then(Value::as_i64).unwrap_or(0);
    [v0, v1]
}

// ─── rpg_battle_rule_config ────────────────────────────────────────────────────

/// Condition de déclenchement d'une règle de démarrage de combat RPG (`RpgBattleStartRuleInfo`).
///
/// Vérité terrain : `rpg_battle_rule_config_1.02.84.00.cfg.bin.json`,
/// liste `m_StartRuleList` (2 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBattleStartRuleInfo {
    /// `type` — type de règle de démarrage.
    /// Mot-clé Rust réservé, renommé `rule_type`.
    pub rule_type: i64,
    /// `param` — paramètre associé au type.
    pub param: i64,
    /// `trg` — déclencheur (trigger).
    pub trg: i64,
    /// `paramId` — hash identifiant du paramètre (souvent `0x00000000`).
    pub param_id: HashId,
}

impl RpgBattleStartRuleInfo {
    /// Parse une entrée de `m_StartRuleList`. Renvoie `None` si le type est absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            rule_type: field_i64(v, "type").unwrap_or(0),
            param: field_i64(v, "param").unwrap_or(0),
            trg: field_i64(v, "trg").unwrap_or(0),
            param_id: field_hash(v, "paramId"),
        })
    }
}

/// Règle de combat RPG (`RpgBattleRuleInfo`).
///
/// Vérité terrain : `rpg_battle_rule_config_1.02.84.00.cfg.bin.json`,
/// liste `m_BattleRuleList` (13 entrées).
/// Exemple réel — `m_BattleRuleList[0]` (ligne 28) : `ruleId = "0x42D90CBC"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBattleRuleInfo {
    /// `ruleId` — hash identifiant unique de la règle.
    pub rule_id: HashId,
    /// `start[0..1]` — paramètres de démarrage (plage dans `m_StartRuleList`).
    pub start: [i64; 2],
    /// `end[0..1]` — paramètres de fin de règle.
    pub end: [i64; 2],
    /// `result[0..1]` — paramètres de résultat.
    pub result: [i64; 2],
}

impl RpgBattleRuleInfo {
    /// Parse une entrée de `m_BattleRuleList`. `None` si `ruleId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let rule_id = field_hash(v, "ruleId");
        if rule_id.is_zero() {
            return None;
        }
        Some(Self {
            rule_id,
            start: read_i64_pair(v, "start"),
            end: read_i64_pair(v, "end"),
            result: read_i64_pair(v, "result"),
        })
    }
}

/// Contenu complet d'un `rpg_battle_rule_config_*.cfg.bin.json`.
///
/// Utiliser [`parse_rule_config`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBattleRuleConfig {
    /// 2 règles de démarrage (`m_StartRuleList`).
    pub start_rules: Vec<RpgBattleStartRuleInfo>,
    /// 13 règles de combat (`m_BattleRuleList`).
    pub battle_rules: Vec<RpgBattleRuleInfo>,
}

impl RpgBattleRuleConfig {
    /// Recherche une règle de combat par son `ruleId`. `None` si absente.
    #[must_use]
    pub fn find_battle_rule(&self, rule_id: HashId) -> Option<&RpgBattleRuleInfo> {
        self.battle_rules.iter().find(|r| r.rule_id == rule_id)
    }
}

/// Parse un `rpg_battle_rule_config_*.cfg.bin.json` complet.
///
/// Comptes réels : 2 `RpgBattleStartRuleInfo` + 13 `RpgBattleRuleInfo`.
#[must_use]
pub fn parse_rule_config(root: &Value) -> RpgBattleRuleConfig {
    RpgBattleRuleConfig {
        start_rules: map_list(root, "m_StartRuleList", RpgBattleStartRuleInfo::from_value),
        battle_rules: map_list(root, "m_BattleRuleList", RpgBattleRuleInfo::from_value),
    }
}

// ─── rpg_battle_status_pattern_config ─────────────────────────────────────────

/// Modificateurs de statut pour un pattern RPG (`RPG_BTL_STATUS_PATTERN_INFO`).
///
/// Vérité terrain : `rpg_battle_status_pattern_config_1.03.17.00.cfg.bin.json`,
/// liste `m_statusPatternInfoList` (28 entrées).
///
/// Exemple réel — entrée 0 (ligne 9) : `id = "0xD1002353"`,
/// `hpAddRate = 0.2`, `attackAdd = -2`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBattleStatusPatternInfo {
    /// `id` — hash identifiant unique du pattern.
    pub id: HashId,
    /// `hpAddRate` — taux d'ajout de HP (ex. 0.2 = +20 %).
    pub hp_add_rate: f32,
    /// `attackAdd` — ajout d'attaque (négatif = pénalité).
    pub attack_add: i64,
    /// `defenseAdd` — ajout de défense.
    pub defense_add: i64,
}

impl RpgBattleStatusPatternInfo {
    /// Parse une entrée de `m_statusPatternInfoList`. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            hp_add_rate: v.get("hpAddRate").and_then(Value::as_f64).unwrap_or(0.0) as f32,
            attack_add: field_i64(v, "attackAdd").unwrap_or(0),
            defense_add: field_i64(v, "defenseAdd").unwrap_or(0),
        })
    }
}

/// Parse un `rpg_battle_status_pattern_config_*.cfg.bin.json`.
///
/// Renvoie les 28 [`RpgBattleStatusPatternInfo`] valides (id non-nul).
#[must_use]
pub fn parse_status_pattern_config(root: &Value) -> Vec<RpgBattleStatusPatternInfo> {
    map_list(
        root,
        "m_statusPatternInfoList",
        RpgBattleStatusPatternInfo::from_value,
    )
}

// ─── rpg_battle_chara_swap_motion_config ──────────────────────────────────────

/// Données de motion de remplacement par type de corps (`SWAP_MOTION_DATA`).
///
/// Vérité terrain : `rpg_battle_chara_swap_motion_config_1.04.06.00.cfg.bin.json`,
/// liste `m_SwapMotionDataList` (10 entrées).
///
/// Exemple réel — entrée 0 (ligne 10) : `bodyType = 1`, `motionId = "0x2C0F2E35"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SwapMotionData {
    /// `bodyType` — type de corps du personnage.
    pub body_type: i64,
    /// `motionId` — hash de la motion associée.
    pub motion_id: HashId,
}

impl SwapMotionData {
    /// Parse une entrée de `m_SwapMotionDataList`. `None` si `motionId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let motion_id = field_hash(v, "motionId");
        if motion_id.is_zero() {
            return None;
        }
        Some(Self {
            body_type: field_i64(v, "bodyType").unwrap_or(0),
            motion_id,
        })
    }
}

/// Données de motion de remplacement spécifiques au combat RPG (`RPG_BATTLE_CHARA_SWAP_MOTION_DATA`).
///
/// Vérité terrain : `rpg_battle_chara_swap_motion_config_1.04.06.00.cfg.bin.json`,
/// liste `m_RpgBattleCharaSwapMotionDataList` (2 entrées).
///
/// Exemple réel — entrée 0 (ligne 55) : `modelAttribute = 3`, `swapMotion = [0, 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBattleCharaSwapMotionData {
    /// `modelAttribute` — attribut du modèle (ex. 3 = grande taille, 2 = normale).
    pub model_attribute: i64,
    /// `openCond` — condition d'ouverture, encodée en base64.
    pub open_cond: String,
    /// `swapMotion[0..1]` — plage dans `m_SwapMotionDataList` `[offset, count]`.
    pub swap_motion: [i64; 2],
}

impl RpgBattleCharaSwapMotionData {
    /// Parse une entrée de `m_RpgBattleCharaSwapMotionDataList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            model_attribute: field_i64(v, "modelAttribute").unwrap_or(0),
            open_cond: v
                .get("openCond")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_default(),
            swap_motion: read_i64_pair(v, "swapMotion"),
        })
    }
}

/// Contenu complet d'un `rpg_battle_chara_swap_motion_config_*.cfg.bin.json`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBattleCharaSwapMotionConfig {
    /// 10 données de motion de corps (`m_SwapMotionDataList`).
    pub swap_motions: Vec<SwapMotionData>,
    /// 2 données de motion RPG par attribut de modèle (`m_RpgBattleCharaSwapMotionDataList`).
    pub chara_swap_motions: Vec<RpgBattleCharaSwapMotionData>,
}

impl RpgBattleCharaSwapMotionConfig {
    /// Renvoie la tranche de [`SwapMotionData`] référencée par une entrée `chara_swap_motion`.
    #[must_use]
    pub fn swap_motions_of(&self, csm: &RpgBattleCharaSwapMotionData) -> &[SwapMotionData] {
        let start = csm.swap_motion[0] as usize;
        let end = start.saturating_add(csm.swap_motion[1] as usize);
        self.swap_motions.get(start..end).unwrap_or(&[])
    }
}

/// Parse un `rpg_battle_chara_swap_motion_config_*.cfg.bin.json` complet.
///
/// Comptes réels : 10 [`SwapMotionData`] + 2 [`RpgBattleCharaSwapMotionData`].
#[must_use]
pub fn parse_chara_swap_motion_config(root: &Value) -> RpgBattleCharaSwapMotionConfig {
    RpgBattleCharaSwapMotionConfig {
        swap_motions: map_list(root, "m_SwapMotionDataList", SwapMotionData::from_value),
        chara_swap_motions: map_list(
            root,
            "m_RpgBattleCharaSwapMotionDataList",
            RpgBattleCharaSwapMotionData::from_value,
        ),
    }
}

// ─── rpg_battle_party_config ──────────────────────────────────────────────────

/// Action désactivée pour une configuration de groupe RPG (`RPG_BTL_PARTY_DISABLE_ACTION_UNIQUE_INFO_*`).
///
/// Vérité terrain : `rpg_battle_party_config_0.00.00.cfg.bin.json`,
/// conteneur `RPG_BTL_PARTY_DISABLE_ACTION_UNIQUE_INFO_LIST_BEG_0` (4 entrées).
///
/// Exemple réel — entrée 0 (ligne 17) :
/// `var[0] = -1046485492` → `action_id = 0xC19FE60C`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBtlPartyDisableActionUniqueInfo {
    /// var\[0\] — hash de l'action désactivée (non-nul).
    pub action_id: HashId,
    /// var\[1\] — indicateur complémentaire (vaut 0 dans tous les cas du fichier réel).
    pub var1: i64,
}

impl RpgBtlPartyDisableActionUniqueInfo {
    #[must_use]
    fn from_node(node: crate::cfgbin::Node<'_>) -> Option<Self> {
        let action_id = node.hash(0);
        if action_id.is_zero() {
            return None;
        }
        Some(Self {
            action_id,
            var1: node.int(1),
        })
    }
}

/// Groupe de combat RPG avec ses restrictions d'action (`RPG_BTL_PARTY_INFO_*`).
///
/// Vérité terrain : `rpg_battle_party_config_0.00.00.cfg.bin.json`,
/// conteneur `RPG_BTL_PARTY_INFO_LIST_BEG_0` (10 entrées + 10 REF).
///
/// Les champs `disable_offset`/`disable_count` proviennent des noeuds
/// `RPG_BTL_PARTY_INFO_REF_DISABLE_ACTION_UNIQUE_INFO_N` (même index).
///
/// Exemple réel — entrée 0 (ligne 84) :
/// `var[0] = -1758152796` → `party_id = 0x9734B7A4`,
/// REF (ligne 97) : `offset=0, count=1`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBtlPartyInfo {
    /// var\[0\] — hash identifiant unique du groupe (non-nul).
    pub party_id: HashId,
    /// var\[1\] — hash secondaire (souvent nul, renseigné pour certains groupes).
    pub secondary_id: HashId,
    /// Offset dans `disable_actions` (source : `REF_DISABLE_ACTION_UNIQUE_INFO.var[0]`).
    pub disable_offset: i64,
    /// Nombre d'actions désactivées (source : `REF_DISABLE_ACTION_UNIQUE_INFO.var[1]`).
    pub disable_count: i64,
}

/// Contenu complet d'un `rpg_battle_party_config_*.cfg.bin.json`.
///
/// Utiliser [`parse_party_config`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBattlePartyConfig {
    /// 4 actions désactivées (`RPG_BTL_PARTY_DISABLE_ACTION_UNIQUE_INFO_*`).
    pub disable_actions: Vec<RpgBtlPartyDisableActionUniqueInfo>,
    /// 10 groupes avec leurs restrictions (`RPG_BTL_PARTY_INFO_*` + REF combinés).
    pub parties: Vec<RpgBtlPartyInfo>,
}

impl RpgBattlePartyConfig {
    /// Renvoie la tranche d'actions désactivées d'un groupe.
    ///
    /// Utilise `party.disable_offset` et `party.disable_count` comme indices dans
    /// `self.disable_actions`. Renvoie `&[]` si les indices dépassent la liste.
    #[must_use]
    pub fn disable_actions_of(
        &self,
        party: &RpgBtlPartyInfo,
    ) -> &[RpgBtlPartyDisableActionUniqueInfo] {
        let start = party.disable_offset as usize;
        let end = start.saturating_add(party.disable_count as usize);
        self.disable_actions.get(start..end).unwrap_or(&[])
    }

    /// Recherche un groupe par son `party_id`. `None` si absent.
    #[must_use]
    pub fn find_party(&self, party_id: HashId) -> Option<&RpgBtlPartyInfo> {
        self.parties.iter().find(|p| p.party_id == party_id)
    }
}

/// Parse un `rpg_battle_party_config_*.cfg.bin.json` complet.
///
/// Comptes réels : 4 [`RpgBtlPartyDisableActionUniqueInfo`] + 10 [`RpgBtlPartyInfo`].
#[must_use]
pub fn parse_party_config(root: &Value) -> RpgBattlePartyConfig {
    let mut disable_actions = Vec::new();
    walk_named(root, "RPG_BTL_PARTY_DISABLE_ACTION_UNIQUE_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(info) = RpgBtlPartyDisableActionUniqueInfo::from_node(node) {
            disable_actions.push(info);
        }
    });

    // Collecte des PARTY_INFO_N et de leurs REF en deux passes, puis fusion par index.
    let mut party_ids: Vec<(HashId, HashId)> = Vec::new();
    walk_named(root, "RPG_BTL_PARTY_INFO_", |node| {
        let name = node.name();
        if name.contains("_LIST_") || name.contains("_REF_") {
            return;
        }
        let party_id = node.hash(0);
        if party_id.is_zero() {
            return;
        }
        party_ids.push((party_id, node.hash(1)));
    });

    let mut party_refs: Vec<(i64, i64)> = Vec::new();
    walk_named(
        root,
        "RPG_BTL_PARTY_INFO_REF_DISABLE_ACTION_UNIQUE_INFO_",
        |node| {
            party_refs.push((node.int(0), node.int(1)));
        },
    );

    let parties: Vec<RpgBtlPartyInfo> = party_ids
        .into_iter()
        .enumerate()
        .map(|(i, (party_id, secondary_id))| {
            let (disable_offset, disable_count) = party_refs.get(i).copied().unwrap_or((0, 0));
            RpgBtlPartyInfo {
                party_id,
                secondary_id,
                disable_offset,
                disable_count,
            }
        })
        .collect();

    RpgBattlePartyConfig {
        disable_actions,
        parties,
    }
}

// ─── rpg_battle_cmd_event_config ──────────────────────────────────────────────

/// Parse un `rpg_battle_cmd_event_config_*.cfg.bin.json`.
///
/// Chaque noeud `RPG_BTL_CMD_EVENT_INFO_N` contient un hash unique (1 variable).
///
/// Vérité terrain : `rpg_battle_cmd_event_config_0.00.00.cfg.bin.json` → 1 entrée.
/// `RPG_BTL_CMD_EVENT_INFO_0.var[0] = 20635986` → `HashId(0x013AE152)`.
#[must_use]
pub fn parse_cmd_event_config(root: &Value) -> Vec<HashId> {
    let mut out = Vec::new();
    walk_named(root, "RPG_BTL_CMD_EVENT_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        let id = node.hash(0);
        if !id.is_zero() {
            out.push(id);
        }
    });
    out
}

// ─── rpg_battle_cmd_obj_config ────────────────────────────────────────────────

/// Parse un `rpg_battle_cmd_obj_config_*.cfg.bin.json`.
///
/// Chaque noeud `RPG_BTL_CMD_OBJ_INFO_N` contient un hash unique (1 variable).
///
/// Vérité terrain : `rpg_battle_cmd_obj_config_0.00.00.cfg.bin.json` → 1 entrée.
/// `RPG_BTL_CMD_OBJ_INFO_0.var[0] = 894153704` → `HashId(0x354BB3E8)`.
#[must_use]
pub fn parse_cmd_obj_config(root: &Value) -> Vec<HashId> {
    let mut out = Vec::new();
    walk_named(root, "RPG_BTL_CMD_OBJ_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        let id = node.hash(0);
        if !id.is_zero() {
            out.push(id);
        }
    });
    out
}

// ─── rpg_battle_cmd_set_config ────────────────────────────────────────────────

/// Entrée d'un set de commandes RPG (`RPG_BTL_CMD_SET_INFO_*`).
///
/// Chaque set référence une plage dans la liste globale de commandes `cmd_data` via
/// `cmd_offset` et `cmd_count`. Utiliser [`RpgBattleCmdSetConfig::cmds_of`] pour la résoudre.
///
/// Vérité terrain : `rpg_battle_cmd_set_config_1.01.31.00.cfg.bin.json`.
/// `RPG_BTL_CMD_SET_INFO_0.var[0] = 1268152025` → `set_id = 0x4B9676D9`.
/// `RPG_BTL_CMD_SET_INFO_REF_CMD_DATA_0` : `offset=0, count=4` (lignes 11320-11328).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBtlCmdSetInfo {
    /// var\[0\] — hash identifiant unique du set de commandes.
    pub set_id: HashId,
    /// Offset dans `cmd_data` (source : `REF_CMD_DATA.var[0]`).
    pub cmd_offset: i64,
    /// Nombre de commandes dans ce set (source : `REF_CMD_DATA.var[1]`).
    pub cmd_count: i64,
}

/// Contenu complet d'un `rpg_battle_cmd_set_config_*.cfg.bin.json`.
///
/// - `cmd_data` : 1 128 hashes de commandes individuelles.
/// - `cmd_sets` : 186 sets référençant des plages dans `cmd_data`.
///
/// Utiliser [`RpgBattleCmdSetConfig::cmds_of`] pour résoudre les commandes d'un set.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBattleCmdSetConfig {
    /// 1 128 commandes individuelles (`RPG_BTL_CMD_SET_CMD_DATA_*`).
    pub cmd_data: Vec<HashId>,
    /// 186 sets de commandes (`RPG_BTL_CMD_SET_INFO_*` + REF fusionnés).
    pub cmd_sets: Vec<RpgBtlCmdSetInfo>,
}

impl RpgBattleCmdSetConfig {
    /// Renvoie la tranche de commandes d'un set.
    ///
    /// Utilise `set.cmd_offset` et `set.cmd_count` comme indices dans `self.cmd_data`.
    /// Renvoie `&[]` si les indices dépassent la taille de la liste.
    #[must_use]
    pub fn cmds_of(&self, set: &RpgBtlCmdSetInfo) -> &[HashId] {
        let start = set.cmd_offset as usize;
        let end = start.saturating_add(set.cmd_count as usize);
        self.cmd_data.get(start..end).unwrap_or(&[])
    }

    /// Recherche un set par son `set_id`. `None` si absent.
    #[must_use]
    pub fn find_set(&self, set_id: HashId) -> Option<&RpgBtlCmdSetInfo> {
        self.cmd_sets.iter().find(|s| s.set_id == set_id)
    }
}

/// Parse un `rpg_battle_cmd_set_config_*.cfg.bin.json` complet.
///
/// Comptes réels : 1 128 [`HashId`] dans `cmd_data` + 186 [`RpgBtlCmdSetInfo`].
#[must_use]
pub fn parse_cmd_set_config(root: &Value) -> RpgBattleCmdSetConfig {
    let mut cmd_data: Vec<HashId> = Vec::new();
    walk_named(root, "RPG_BTL_CMD_SET_CMD_DATA_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        let id = node.hash(0);
        if !id.is_zero() {
            cmd_data.push(id);
        }
    });

    // Deux passes (INFO + REF) puis fusion par index.
    let mut set_ids: Vec<HashId> = Vec::new();
    walk_named(root, "RPG_BTL_CMD_SET_INFO_", |node| {
        let name = node.name();
        if name.contains("_LIST_") || name.contains("_REF_") {
            return;
        }
        let id = node.hash(0);
        if !id.is_zero() {
            set_ids.push(id);
        }
    });

    let mut set_refs: Vec<(i64, i64)> = Vec::new();
    walk_named(root, "RPG_BTL_CMD_SET_INFO_REF_CMD_DATA_", |node| {
        set_refs.push((node.int(0), node.int(1)));
    });

    let cmd_sets: Vec<RpgBtlCmdSetInfo> = set_ids
        .into_iter()
        .enumerate()
        .map(|(i, set_id)| {
            let (cmd_offset, cmd_count) = set_refs.get(i).copied().unwrap_or((0, 0));
            RpgBtlCmdSetInfo {
                set_id,
                cmd_offset,
                cmd_count,
            }
        })
        .collect();

    RpgBattleCmdSetConfig { cmd_data, cmd_sets }
}

// ─── rpg_battle_add_status_config ─────────────────────────────────────────────

/// Descripteur de type de statut additionnel (`RPG_BTL_ADD_STATUS_TYPE_INFO_*`).
///
/// Vérité terrain : `rpg_battle_add_status_config_0.00.00.cfg.bin.json`,
/// conteneur `RPG_BTL_ADD_STATUS_TYPE_INFO_LIST_BEG_0` (11 entrées, indices 0-10).
///
/// Exemple réel — entrée 0 (ligne ~22) : `type_id = -1` (toutes catégories), `params = [0; 5]`.
/// Entrée 2 (ligne ~73) : `type_id = 29`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBtlAddStatusTypeInfo {
    /// var\[0\] — identifiant de catégorie de statut (`-1` = toutes, `0` = aucune, sinon spécifique).
    pub type_id: i64,
    /// var\[1..5\] — paramètres complémentaires (généralement 0 dans le fichier réel).
    pub params: [i64; 5],
}

impl RpgBtlAddStatusTypeInfo {
    #[must_use]
    fn from_node(node: crate::cfgbin::Node<'_>) -> Self {
        Self {
            type_id: node.int(0),
            params: [
                node.int(1),
                node.int(2),
                node.int(3),
                node.int(4),
                node.int(5),
            ],
        }
    }
}

/// Paramètre d'un effet de statut additionnel (`RPG_BTL_ADD_STATUS_INFO_PARAM_*`).
///
/// Chaque noeud contient 15 variables positionnelles. Les sémantiques exactes
/// sont partiellement inconnues sans la source TS inagle.
///
/// Vérité terrain : `rpg_battle_add_status_config_0.00.00.cfg.bin.json`.
/// `RPG_BTL_ADD_STATUS_INFO_PARAM_0` (ligne 372) :
/// `var[0]=32`, `var[1]=-1`, `var[5]="0,2"` (rate=0.2), `var[14]=-1`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBtlAddStatusInfoParam {
    /// var\[0\] — identifiant du type de paramètre (32, 34, 29, 30…).
    pub param_type: i64,
    /// var\[1\] — filtre cible (`-1` = tous).
    pub target: i64,
    /// var\[2\] — valeur de condition.
    pub v2: i64,
    /// var\[3\] — mode ou sous-type.
    pub v3: i64,
    /// var\[4\] — paramètre de mode.
    pub v4: i64,
    /// var\[5\] — taux flottant (probabilité, magnitude ; ex. 0.2, 0.5).
    pub rate: f32,
    /// var\[6..13\] — paramètres complémentaires (généralement 0).
    pub raw: [i64; 8],
    /// var\[14\] — drapeau terminal (0 ou -1).
    pub flag: i64,
}

impl RpgBtlAddStatusInfoParam {
    #[must_use]
    fn from_node(node: crate::cfgbin::Node<'_>) -> Self {
        Self {
            param_type: node.int(0),
            target: node.int(1),
            v2: node.int(2),
            v3: node.int(3),
            v4: node.int(4),
            rate: node.var(5).map_or(0.0, |v| v.as_f64() as f32),
            raw: [
                node.int(6),
                node.int(7),
                node.int(8),
                node.int(9),
                node.int(10),
                node.int(11),
                node.int(12),
                node.int(13),
            ],
            flag: node.int(14),
        }
    }
}

/// Effet de statut additionnel avec ses paramètres (`RPG_BTL_ADD_STATUS_INFO_*`).
///
/// Vérité terrain : `rpg_battle_add_status_config_0.00.00.cfg.bin.json`,
/// conteneur `RPG_BTL_ADD_STATUS_INFO_LIST_BEG_0` (6 entrées).
///
/// Exemple réel — entrée 0 (ligne 354) :
/// `var[0] = -408109186` → `status_id = 0xE7ACBF7E`, 2 paramètres.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBtlAddStatusInfo {
    /// var\[0\] — hash identifiant unique du statut.
    pub status_id: HashId,
    /// Paramètres d'effet (issus des noeuds `RPG_BTL_ADD_STATUS_INFO_PARAM_*` enfants).
    pub params: Vec<RpgBtlAddStatusInfoParam>,
}

/// Entrée d'attribut de statut dans `RPG_BTL_ADD_STATUS_INFO_ATTR_0`.
///
/// Les 36 variables du noeud ATTR sont interprétées comme 18 paires `(enabled, id)`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBtlAddStatusAttrEntry {
    /// `true` si l'attribut est activé (var paire vaut 1).
    pub enabled: bool,
    /// Hash de l'attribut (var impaire).
    pub id: HashId,
}

/// Tableau d'attributs de statut (`RPG_BTL_ADD_STATUS_INFO_ATTR_0`).
///
/// Vérité terrain : `rpg_battle_add_status_config_0.00.00.cfg.bin.json`,
/// 1 noeud ATTR avec 36 variables → 18 paires `(enabled, id)`.
///
/// Exemple réel — paire 0 (ligne 598-604) : `enabled=true`, `id = 0xAF28407D`
/// (var\[1\] = -1356316547).
///
/// **Note serde** : le champ `entries` utilise `Vec<…>` pour éviter la limite de 32
/// éléments de `serde::Deserialize` sur les tableaux `[T; N]`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBtlAddStatusInfoAttr {
    /// 18 paires `(enabled, id)` issues des 36 variables positionnelles.
    pub entries: Vec<RpgBtlAddStatusAttrEntry>,
}

impl RpgBtlAddStatusInfoAttr {
    #[must_use]
    fn from_node(node: crate::cfgbin::Node<'_>) -> Self {
        let var_count = node.var_count();
        let pair_count = var_count / 2;
        let mut entries = Vec::with_capacity(pair_count);
        for i in 0..pair_count {
            let enabled = node.int(i * 2) != 0;
            let id = node.hash(i * 2 + 1);
            entries.push(RpgBtlAddStatusAttrEntry { enabled, id });
        }
        Self { entries }
    }
}

/// Contenu complet d'un `rpg_battle_add_status_config_*.cfg.bin.json`.
///
/// Utiliser [`parse_add_status_config`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RpgBattleAddStatusConfig {
    /// 11 descripteurs de type de statut (`RPG_BTL_ADD_STATUS_TYPE_INFO_*`).
    pub type_infos: Vec<RpgBtlAddStatusTypeInfo>,
    /// 6 effets de statut avec paramètres (`RPG_BTL_ADD_STATUS_INFO_*`).
    pub status_infos: Vec<RpgBtlAddStatusInfo>,
    /// Table d'attributs associés (1 noeud `RPG_BTL_ADD_STATUS_INFO_ATTR_0`, 18 paires).
    pub attr: Option<RpgBtlAddStatusInfoAttr>,
}

/// Parse un `rpg_battle_add_status_config_*.cfg.bin.json` complet.
///
/// Comptes réels :
/// - 11 [`RpgBtlAddStatusTypeInfo`]
/// - 6 [`RpgBtlAddStatusInfo`] (7 paramètres au total : 2+1+1+1+1+1)
/// - 1 [`RpgBtlAddStatusInfoAttr`] (18 paires)
#[must_use]
pub fn parse_add_status_config(root: &Value) -> RpgBattleAddStatusConfig {
    let mut type_infos = Vec::new();
    walk_named(root, "RPG_BTL_ADD_STATUS_TYPE_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        type_infos.push(RpgBtlAddStatusTypeInfo::from_node(node));
    });

    let mut status_infos: Vec<RpgBtlAddStatusInfo> = Vec::new();
    walk_named(root, "RPG_BTL_ADD_STATUS_INFO_", |node| {
        let name = node.name();
        if name.contains("_LIST_")
            || name.starts_with("RPG_BTL_ADD_STATUS_INFO_PARAM")
            || name.starts_with("RPG_BTL_ADD_STATUS_INFO_ATTR")
        {
            return;
        }
        let status_id = node.hash(0);
        if status_id.is_zero() {
            return;
        }
        // Collecte des PARAMs depuis les enfants du noeud INFO.
        let mut params = Vec::new();
        for list_child in node.children() {
            for param_child in list_child.children() {
                let pname = param_child.name();
                if pname.starts_with("RPG_BTL_ADD_STATUS_INFO_PARAM_") && !pname.contains("_LIST_")
                {
                    params.push(RpgBtlAddStatusInfoParam::from_node(param_child));
                }
            }
        }
        status_infos.push(RpgBtlAddStatusInfo { status_id, params });
    });

    let mut attr: Option<RpgBtlAddStatusInfoAttr> = None;
    walk_named(root, "RPG_BTL_ADD_STATUS_INFO_ATTR_", |node| {
        attr = Some(RpgBtlAddStatusInfoAttr::from_node(node));
    });

    RpgBattleAddStatusConfig {
        type_infos,
        status_infos,
        attr,
    }
}
