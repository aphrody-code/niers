//! `CharaParam` — port du noeud `CHARA_PARAM_INFO_*` du `chara_param.cfg.bin`.
//!
//! ## Vérité terrain
//!
//! - Parser TS : `packages/inagle/src/parsers/chara-param.ts` (`parseParamNode` l.71-152,
//!   helpers `positionIdToCode`/`elementIdToNames` l.221-260).
//! - Données : `/home/ubuntu/niers/data/common/gamedata/character/chara_param_1.03.66.00.cfg.bin.json`.
//!
//! Noeud vérifié `CHARA_PARAM_INFO_1` (43 vars Int) :
//! `[-357386801, 1128709053, 4, 2, 4, 1, 11, 1, 0, 3, 0, 604761586, 1, 598373713, 13,
//!   1591574804, 20, 724843777, 30, 1988803013, 38, -1508771477, 43, 724843777, 30,
//!   -1325922806, 38, 1210030151, 43, 2, 0, -1, 2, 1, -2, -2, 1, 0, 0, 0, 743008281, 0, 200626314]`
//! → element=4 (Montagne), mainPosition=2 (FW).
//!
//! ## Extraction des techniques (port 1:1 d'inagle — vérité terrain)
//!
//! On porte **exactement** la logique d'inagle `packages/inagle/src/parsers/chara-param.ts`
//! (l.102-118) : lecture **LEVEL-FIRST à partir de l'index 10**, 9 slots :
//! `(niveau@10, hash@11), (niveau@12, hash@13), …`. On valide le **niveau ∈ [0,99]** (le hash
//! = toute valeur 32 bits ≠ 0) et on **saute** les slots invalides (pas de `break`, comme
//! inagle), borné par la longueur. Pour `CHARA_PARAM_INFO_1` : 9 techniques aux hash
//! `604761586, 598373713, …` avec niveaux `0, 1, 13, 20, 30, 38, 43, 30, 38`.
//! (Une lecture hash-first @11 décalerait les niveaux d'un slot — c'est l'ancien bug rejeté
//! par inagle, voir le commentaire « tronquait/désalignait » du parseur TS.)

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, owned, walk_named};
use crate::hash::HashId;

/// Index du **niveau** de la 1re technique (LEVEL-first @10, port 1:1 d'inagle chara-param.ts).
/// Le hash de la technique suit immédiatement (`SKILL_BLOCK_START + 1`).
pub const SKILL_BLOCK_START: usize = 10;
/// Nombre maximum de slots techniques d'un personnage.
pub const MAX_SKILL_SLOTS: usize = 9;

/// Une technique apprise par un personnage : hash de la technique + niveau d'apprentissage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LearnedSkill {
    /// Hash de la technique (`skillID` du skill_config).
    pub skill_id: HashId,
    /// Niveau d'apprentissage (0..=99).
    pub learn_level: u8,
}

/// Un noeud `CHARA_PARAM_INFO_*` porté.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaParam {
    /// var0 — `charaParamId` (ID unique de la variante).
    pub chara_param_id: HashId,
    /// var1 — `charaBaseId` (perso de base).
    pub chara_base_id: HashId,
    /// var2 — `element` (1=Vent, 2=Forêt, 3=Feu, 4=Montagne).
    pub element: i64,
    /// var3 — `mainPosition` (1=GK, 2=FW, 3=MF, 4=DF).
    pub main_position: i64,
    /// var4 — `subPosition`.
    pub sub_position: i64,
    /// var8 — `growthPattern`.
    pub growth_pattern: i64,
    /// Techniques (level-first @10, hash@11 — port 1:1 inagle chara-param.ts l.102-118).
    pub skills: Vec<LearnedSkill>,
    /// Toutes les variables Int brutes (debug / RE).
    pub raw_variables: Vec<i64>,
}

impl CharaParam {
    /// Parse un noeud `CHARA_PARAM_INFO_*`. `None` si moins de 8 variables Int (entrée invalide,
    /// comme `parseParamNode` qui renvoie `null` si `values.length < 8`).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        // Récupère les valeurs des variables de type "Int" (comme le filtre TS).
        let mut values: Vec<i64> = Vec::with_capacity(node.var_count());
        for i in 0..node.var_count() {
            if let Some(var) = node.var(i)
                && var.ty == "Int"
            {
                values.push(var.as_i64());
            }
        }
        if values.len() < 8 {
            return None;
        }

        let chara_param_id = HashId::from_i64(values[0]);
        let chara_base_id = HashId::from_i64(values[1]);
        let element = values[2];
        let main_position = values[3];
        let sub_position = *values.get(4).unwrap_or(&0);
        let growth_pattern = *values.get(8).unwrap_or(&0);

        // Extraction techniques — port 1:1 d'inagle `parsers/chara-param.ts` (l.102-118) :
        // LEVEL-FIRST @10, 9 slots : (niveau@10, hash@11), (niveau@12, hash@13)…
        // On valide le NIVEAU ∈ [0,99] (le hash = toute valeur 32 bits ≠ 0) et on SAUTE les
        // slots invalides (pas de break, comme inagle), borné par la longueur.
        let mut skills = Vec::new();
        for slot in 0..MAX_SKILL_SLOTS {
            let level_idx = SKILL_BLOCK_START + slot * 2;
            let hash_idx = level_idx + 1;
            if hash_idx >= values.len() {
                break;
            }
            let skill_id_num = values[hash_idx];
            let level = values[level_idx];
            if skill_id_num != 0 && (0..=99).contains(&level) {
                skills.push(LearnedSkill {
                    skill_id: HashId::from_i64(skill_id_num),
                    // niveau ∈ [0,99] garanti par le test ci-dessus.
                    learn_level: level as u8,
                });
            }
        }

        Some(CharaParam {
            chara_param_id,
            chara_base_id,
            element,
            main_position,
            sub_position,
            growth_pattern,
            skills,
            raw_variables: values,
        })
    }
}

/// Parse tous les noeuds `CHARA_PARAM_INFO*` d'un `chara_param_*.cfg.bin` (dump JSON ou T2B lu
/// directement).
///
/// Le préfixe est cherché **sans underscore final** : les dumps `*.cfg.bin.json` suffixent les
/// noeuds d'un index (`CHARA_PARAM_INFO_0`), mais le T2B binaire les nomme `CHARA_PARAM_INFO`
/// tout court (même écart que `chara_base`, cf. `is_chara_base_node`). Les conteneurs restent
/// écartés par le filtre `_LIST_`.
#[must_use]
pub fn parse_all_chara_params(root: &Value) -> Vec<CharaParam> {
    let mut out = Vec::new();
    walk_named(root, "CHARA_PARAM_INFO", |node| {
        // On ignore les noeuds conteneurs (`_LIST_BEG_`, etc.) qui n'ont pas le bon format.
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(p) = CharaParam::from_node(node) {
            out.push(p);
        }
    });
    out
}

/// Code de position. Source : `positionIdToCode` (chara-param.ts l.221-240).
#[must_use]
pub fn position_id_to_code(id: i64) -> Option<&'static str> {
    match id {
        0 => Some("Coach"),
        1 => Some("GK"),
        2 => Some("FW"),
        3 => Some("MF"),
        4 => Some("DF"),
        _ => None,
    }
}

/// Noms d'élément (fr, en, ja). Source : `elementIdToNames` (chara-param.ts l.247-260).
#[must_use]
pub fn element_id_to_names(id: i64) -> Option<(&'static str, &'static str, &'static str)> {
    match id {
        1 => Some(("Vent", "Wind", "風")),
        2 => Some(("Forêt", "Forest", "林")),
        3 => Some(("Feu", "Fire", "火")),
        4 => Some(("Montagne", "Mountain", "山")),
        _ => None,
    }
}

/// Variante possédée de [`position_id_to_code`] renvoyant une `String` (commodité).
#[must_use]
pub fn position_code_owned(id: i64) -> Option<String> {
    position_id_to_code(id).map(owned)
}
