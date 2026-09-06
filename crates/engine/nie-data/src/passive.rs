//! `PassiveSkillInfo` — port de `passive_skill_config` + classification scope/boost.
//!
//! ## Vérité terrain
//!
//! - Types TS : `packages/inagle/src/skills/types.ts` (`PassiveScope` l.165,
//!   `PassiveBoostType` l.168-184, `PASSIVE_BOOST_NAMES` l.195-212).
//! - Parser : `packages/inagle/src/parsers/passive-skill-config.ts` (collectEffects l.115-133,
//!   collectSkills l.136-180) ; classification : `packages/inagle/src/skills/mapper-passive.ts`
//!   (`detectScope` l.131-161, `detectBoostType` l.165-225).
//! - Données : `data/common/gamedata/skill/passive_skill_config_5.00.07.00.cfg.bin.json`.
//!
//! Noeud vérifié `PASSIVE_SKILL_INFO_0` (6 vars Int) :
//! `[975948532, 1105141741, 0, 0, 6, 0]` → passiveId=0x3A2BCAF4, effectId=0x41DF1FED,
//! nameId=0x00000000, descId=0x00000000, rarity=6.
//! Effet vérifié `PASSIVE_SKILL_EFFECT_0` : Variables[0]=hash (effectId), suivantes = params.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

/// Portée d'un passif. Source : `PassiveScope` (types.ts l.165).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PassiveScope {
    Team,
    Player,
    Enemy,
    Unknown,
}

impl PassiveScope {
    /// Libellés (en, fr). Source : `PASSIVE_SCOPE_NAMES`.
    #[must_use]
    pub fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Team => ("Team", "Équipe"),
            Self::Player => ("Player", "Joueur"),
            Self::Enemy => ("Enemy", "Adversaire"),
            Self::Unknown => ("Unknown", "Inconnu"),
        }
    }
}

/// Type de boost d'un passif (16 variantes). Source : `PassiveBoostType` (types.ts l.168-184).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PassiveBoostType {
    Breach,
    Justice,
    Attack,
    Defense,
    Shoot,
    Dribble,
    Block,
    Catch,
    Speed,
    Stamina,
    Confrontation,
    Tension,
    Recovery,
    Mixed,
    Special,
    Unknown,
}

impl PassiveBoostType {
    /// Libellés (en, fr). Source : `PASSIVE_BOOST_NAMES` (types.ts l.195-212).
    #[must_use]
    pub fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Breach => ("Breach", "Brèche"),
            Self::Justice => ("Justice", "Justice"),
            Self::Attack => ("Attack", "Attaque"),
            Self::Defense => ("Defense", "Défense"),
            Self::Shoot => ("Shoot", "Tir"),
            Self::Dribble => ("Dribble", "Dribble"),
            Self::Block => ("Block", "Défense"),
            Self::Catch => ("Catch", "Arrêt"),
            Self::Speed => ("Speed", "Vitesse"),
            Self::Stamina => ("Stamina", "Physique"),
            Self::Confrontation => ("Confrontation", "Affrontement"),
            Self::Tension => ("Tension", "Tension"),
            Self::Recovery => ("Recovery", "Récupération"),
            Self::Mixed => ("Mixed", "Mixte"),
            Self::Special => ("Special", "Spécial"),
            Self::Unknown => ("Unknown", "Inconnu"),
        }
    }
}

/// Un effet `PASSIVE_SKILL_EFFECT_*` : hash (Variables[0]) + paramètres.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PassiveEffect {
    /// effectId (Variables[0] >>> 0).
    pub effect_id: HashId,
    /// Paramètres (toutes les variables après [0], entiers ou flottants tronqués).
    pub params: Vec<f64>,
}

/// Un passif `PASSIVE_SKILL_INFO_*`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PassiveSkillInfo {
    /// var0 — `passiveId` (Variables[0] = hash).
    pub passive_id: HashId,
    /// var1 — `effectId` (vers PASSIVE_SKILL_EFFECT).
    pub effect_id: HashId,
    /// var2 — `nameId` (vers NOUN_INFO du skill_text).
    pub name_id: HashId,
    /// var3 — `descId` (vers TEXT_INFO).
    pub desc_id: HashId,
    /// var4 — `rarity`.
    pub rarity: i64,
    /// Paramètres d'effet (jointure via effectId, si disponible).
    pub effect_params: Option<Vec<f64>>,
    /// Portée classifiée (depuis le template texte).
    pub scope: PassiveScope,
    /// Type de boost classifié (depuis le template texte).
    pub boost_type: PassiveBoostType,
}

impl PassiveSkillInfo {
    /// Parse un noeud `PASSIVE_SKILL_INFO_*`. `None` si moins de 5 variables (comme `collectSkills`).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        if node.var_count() < 5 {
            return None;
        }
        Some(PassiveSkillInfo {
            passive_id: node.hash(0),
            effect_id: node.hash(1),
            name_id: node.hash(2),
            desc_id: node.hash(3),
            rarity: node.int(4),
            effect_params: None,
            scope: PassiveScope::Unknown,
            boost_type: PassiveBoostType::Unknown,
        })
    }
}

/// Détermine la portée depuis le texte d'effet (FR prioritaire). Source : `detectScope` (l.131-161).
#[must_use]
pub fn detect_scope(effect_fr: Option<&str>, effect_en: Option<&str>) -> PassiveScope {
    let text = ascii_lower(effect_fr.or(effect_en).unwrap_or(""));
    if text.contains("équipe")
        || text.contains("(équipe)")
        || text.contains("de l'équipe")
        || text.contains("l'équipe")
        || text.contains("team")
        || text.contains("allies")
        || text.contains("alliés")
    {
        return PassiveScope::Team;
    }
    if text.contains("équipe adverse")
        || text.contains("adversaire")
        || text.contains("ennemi")
        || text.contains("enemy")
        || text.contains("opponent")
    {
        return PassiveScope::Enemy;
    }
    if text.contains("pour les joueurs")
        || text.contains("même élément")
        || text.contains("same element")
        || text.contains("for players")
        || text.contains("joueur")
    {
        return PassiveScope::Player;
    }
    PassiveScope::Unknown
}

/// Détermine le type de boost depuis le texte d'effet. Source : `detectBoostType` (l.165-225).
/// L'ordre des tests est porté à l'identique (premier match gagne).
#[must_use]
pub fn detect_boost_type(effect_fr: Option<&str>, effect_en: Option<&str>) -> PassiveBoostType {
    let t = ascii_lower(effect_fr.or(effect_en).unwrap_or(""));
    if t.contains("brèche") || t.contains("breach") {
        return PassiveBoostType::Breach;
    }
    if t.contains("justice") || t.contains("contre") {
        return PassiveBoostType::Justice;
    }
    if t.contains("affrontement") || t.contains("confrontation") {
        return PassiveBoostType::Confrontation;
    }
    if t.contains("tension") || t.contains("tp ") {
        return PassiveBoostType::Tension;
    }
    if t.contains("récup") || t.contains("recovery") || t.contains("récupér") {
        return PassiveBoostType::Recovery;
    }
    if t.contains("vit ") || t.contains("vitesse") || t.contains("speed") || t.contains("agilité")
    {
        return PassiveBoostType::Speed;
    }
    if t.contains("end ") || t.contains("endurance") || t.contains("stamina") {
        return PassiveBoostType::Stamina;
    }
    if (t.contains("att") && t.contains("déf"))
        || t.contains("&")
        || t.contains("toutes les stats")
        || t.contains("all stats")
    {
        return PassiveBoostType::Mixed;
    }
    if t.contains("tir ") || t.contains("shoot") || t.contains("frappe") {
        return PassiveBoostType::Shoot;
    }
    if t.contains("drb") || t.contains("dribble") || t.contains("contrôle") || t.contains("control")
    {
        return PassiveBoostType::Dribble;
    }
    if t.contains("att ") || t.contains("attack") || t.contains("attaque") {
        return PassiveBoostType::Attack;
    }
    if t.contains("blc")
        || t.contains("block")
        || t.contains("défense")
        || t.contains("defense")
        || t.contains("physique")
    {
        return PassiveBoostType::Block;
    }
    if t.contains("art") || t.contains("arrêt") || t.contains("catch") {
        return PassiveBoostType::Catch;
    }
    if t.contains("déf ") || t.contains("defense") || t.contains("défense") {
        return PassiveBoostType::Defense;
    }
    if t.contains("baisse")
        || t.contains("réduit")
        || t.contains("moitié de terrain")
        || t.contains("zone")
        || t.contains("aligner")
        || t.contains("intelligence")
        || t.contains("boost")
        || t.contains("passion")
        || t.contains("catalyste")
        || t.contains("limiteurs")
        || t.contains("lien")
        || !t.contains(' ')
    {
        return PassiveBoostType::Special;
    }
    PassiveBoostType::Unknown
}

/// Collecte les effets `PASSIVE_SKILL_EFFECT_*` : Variables[0]=hash, suivantes=params.
/// Source : `collectEffects` (passive-skill-config.ts l.115-133).
#[must_use]
pub fn parse_effects(root: &Value) -> Vec<PassiveEffect> {
    let mut out = Vec::new();
    walk_named(root, "PASSIVE_SKILL_EFFECT_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        let n = node.var_count();
        if n < 2 {
            return;
        }
        let effect_id = node.hash(0);
        let mut params = Vec::with_capacity(n - 1);
        for i in 1..n {
            if let Some(v) = node.var(i) {
                // Float → valeur réelle ; sinon entier (parseInt). On stocke en f64 (comme le TS mixte).
                if v.ty == "Float" {
                    params.push(v.as_f64());
                } else {
                    params.push(v.as_i64() as f64);
                }
            }
        }
        out.push(PassiveEffect { effect_id, params });
    });
    out
}

/// Parse tous les `PASSIVE_SKILL_INFO_*` et joint les params d'effet via effectId.
/// Source : `collectSkills` (l.136-180). Les noeuds `_LIST_`/`_REF_` sont exclus (comme le TS).
#[must_use]
pub fn parse_passives(root: &Value) -> Vec<PassiveSkillInfo> {
    let effects = parse_effects(root);
    let mut effect_map: alloc::collections::BTreeMap<HashId, Vec<f64>> =
        alloc::collections::BTreeMap::new();
    for e in effects {
        effect_map.entry(e.effect_id).or_insert(e.params);
    }

    let mut out = Vec::new();
    walk_named(root, "PASSIVE_SKILL_INFO_", |node| {
        let name = node.name();
        if name.contains("_LIST_") || name.contains("_REF_") {
            return;
        }
        if let Some(mut p) = PassiveSkillInfo::from_node(node) {
            p.effect_params = effect_map.get(&p.effect_id).cloned();
            out.push(p);
        }
    });
    out
}

/// Minuscule ASCII conservant les octets UTF-8 (les accents FR restent intacts, on ne casse pas
/// les `contains("équipe")`). On abaisse seulement les A-Z ASCII, suffisant car les littéraux de
/// test côté inagle sont déjà en minuscules.
fn ascii_lower(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        out.push(c.to_ascii_lowercase());
    }
    out
}
