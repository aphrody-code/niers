//! `ctrl_chara` — personnages contrôlables (`party/ctrl_chara_config_*.cfg.bin`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/ctrl-chara-config.ts`
//!   (`parseEntries`, `buildCtrlCharaDatabase`) — **port 1:1**.
//! - Données : `data/common/gamedata/party/ctrl_chara_config_1.04.17.00.cfg.bin` (VFS IEVR),
//!   format **T2B** (`entries`).
//!
//! ## Format
//!
//! Une liste `CTRL_CHR_DATA_LIST_BEG_0` (variable de tête = nombre d'entrées) contenant des
//! enfants `CTRL_CHR_DATA_<i>`, chacun **3 variables** (lues par position, sans contrôle de type,
//! comme inagle) :
//!
//! - `vars[0]` = `charaParamId` (Int signé → hash non signé, `toHex`).
//! - `vars[1]` = `controlType` (Int).
//! - `vars[2]` = `flags` (Int).
//!
//! Un noeud avec moins de 3 variables est rejeté (`None`), conforme à `vars.length < 3`.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

/// Un personnage contrôlable, porté d'un noeud `CTRL_CHR_DATA`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CtrlCharaData {
    /// `vars[0]` — identifiant du `chara_param` (Int signé → hash non signé).
    pub chara_param_id: HashId,
    /// `vars[1]` — type de contrôle.
    pub control_type: i64,
    /// `vars[2]` — drapeaux.
    pub flags: i64,
}

/// Parse un noeud `CTRL_CHR_DATA` en [`CtrlCharaData`].
///
/// Réplique de `parseEntries` (boucle interne) : rejet si moins de 3 variables, sinon
/// `charaParamId = toHex(vars[0])`, `controlType = vars[1]`, `flags = vars[2]` (lecture par
/// position via `parseInt`, sans filtrage de type).
#[must_use]
pub fn parse_ctrl_chara_node(node: &Node<'_>) -> Option<CtrlCharaData> {
    if node.var_count() < 3 {
        return None;
    }
    Some(CtrlCharaData {
        chara_param_id: node.hash(0),
        control_type: node.int(1),
        flags: node.int(2),
    })
}

/// Parse tous les personnages contrôlables d'un `ctrl_chara_config_*.cfg.bin.json` désérialisé.
///
/// Port de `parseEntries` : noeuds `CTRL_CHR_DATA_*` (forme iecode suffixée `_<i>`), hors
/// `*LIST*`/`*BEG*`.
#[must_use]
pub fn parse_all_ctrl_chara(root: &Value) -> Vec<CtrlCharaData> {
    let mut out = Vec::new();
    walk_named(root, "CTRL_CHR_DATA_", |node| {
        let name = node.name();
        if name.contains("LIST") || name.contains("BEG") {
            return;
        }
        if let Some(c) = parse_ctrl_chara_node(&node) {
            out.push(c);
        }
    });
    out
}

/// Vrai si le `charaParamId` donné figure parmi les personnages contrôlables
/// (port de `isControllable`).
#[must_use]
pub fn is_controllable(characters: &[CtrlCharaData], chara_param_id: HashId) -> bool {
    characters
        .iter()
        .any(|c| c.chara_param_id == chara_param_id)
}
