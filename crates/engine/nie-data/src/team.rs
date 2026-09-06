//! `EnjoyModeTeam` — port du noeud `ENJOY_MODE_TEAM_INFO_*` de
//! `enjoy_mode_team_config_*.cfg.bin` (famille « team », mode Plaisir / Enjoy).
//!
//! ## Vérité terrain
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/enjoy-mode-team-config.ts`
//!   (`parseEntries` l.52-90), port **1:1** ici.
//! - Sortie de référence (vérité terrain) : `packages/inagle/src/entries/enjoy_mode_teams.json`
//!   (28 entrées).
//! - Dump réel : `data/common/gamedata/team/enjoy_mode_team_config_1.04.02.00.cfg.bin`
//!   (VFS IEVR), format **T2B** (`entries`).
//!
//! ## Format (T2B `entries`)
//!
//! Une seule liste `ENJOY_MODE_TEAM_INFO_LIST_BEG_0` (la variable de tête vaut `28`, le
//! nombre d'entrées) contenant 28 enfants `ENJOY_MODE_TEAM_INFO_x`. Chaque enfant porte
//! **12 variables mixtes** :
//!
//! | idx | champ inagle    | type   | sémantique                                            |
//! |-----|-----------------|--------|-------------------------------------------------------|
//! | 0   | `teamId`        | CRC    | identifiant d'équipe (toHex)                           |
//! | 1   | `subId`         | CRC    | identifiant secondaire (toHex)                         |
//! | 2   | `colorCrc`      | CRC    | hash de la couleur d'équipe (toHex)                    |
//! | 3   | `type`          | Int    | type d'équipe (1..28 dans le dump)                     |
//! | 4   | `formationCrc`  | CRC    | hash de la formation associée (toHex)                  |
//! | 5   | `texturePath`   | String | chemin de la texture (`ev_chronicle_img/...g4tx`)      |
//! | 6   | `textureName`   | String | nom de la texture (= `name` côté inagle)              |
//! | 7   | —               | mixte  | blob base64 de positions OU `0` (non exposé par inagle)|
//! | 8   | —               | Int    | CRC/flag (non exposé par inagle)                       |
//! | 9   | —               | Int    | CRC/flag (non exposé par inagle)                       |
//! | 10  | —               | Int    | CRC/flag (non exposé par inagle)                       |
//! | 11  | —               | Int    | flag (`-1` ou `30000`, non exposé par inagle)          |
//!
//! Conformément à la directive « port 1:1 de inagle », seules les 7 premières variables
//! (indices 0..6) sont exposées : ce sont les champs golden-validés contre la sortie
//! d'inagle. Les variables 7..11 (blob de positions / CRC / flags) ne sont pas surfacées
//! par le pipeline de référence et sont donc volontairement omises pour rester aligné.
//!
//! Cette famille « team » regroupe 4 cfg.bin (`enjoy_mode_team`, `team_build_config`,
//! `opponent_team_config`, `belong_team`) ; ce module ne porte que `enjoy_mode_team_config`.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, owned};
use crate::hash::HashId;

/// Préfixe de la liste conteneur (`ENJOY_MODE_TEAM_INFO_LIST_BEG`, suffixe d'index iecode).
const LIST_PREFIX: &str = "ENJOY_MODE_TEAM_INFO_LIST_BEG";
/// Préfixe (avec underscore final) des entrées-enfants `ENJOY_MODE_TEAM_INFO_x`.
const ENTRY_PREFIX: &str = "ENJOY_MODE_TEAM_INFO_";

/// Une entrée `ENJOY_MODE_TEAM_INFO_*` — une équipe du mode Plaisir.
///
/// Port 1:1 de `ParsedEnjoyModeTeam` (inagle `enjoy-mode-team-config.ts`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnjoyModeTeam {
    /// var0 — `teamId` : identifiant d'équipe (CRC, `toHex` côté inagle).
    pub team_id: HashId,
    /// var1 — `subId` : identifiant secondaire (CRC).
    pub sub_id: HashId,
    /// var2 — `colorCrc` : hash de la couleur d'équipe (CRC).
    pub color_crc: HashId,
    /// var3 — `type` : type d'équipe (1..28 dans le dump réel).
    pub team_type: i64,
    /// var4 — `formationCrc` : hash de la formation associée (CRC).
    pub formation_crc: HashId,
    /// var5 — `texturePath` : chemin de la texture (`ev_chronicle_img/...g4tx`).
    pub texture_path: String,
    /// var6 — `textureName` : nom de la texture (= `name` côté inagle).
    pub texture_name: String,
}

impl EnjoyModeTeam {
    /// Parse une entrée `ENJOY_MODE_TEAM_INFO_*`. `None` si moins de 7 variables
    /// (équivalent de `if (vars.length < 7) continue;` côté inagle).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        if node.var_count() < 7 {
            return None;
        }
        Some(Self {
            team_id: node.hash(0),
            sub_id: node.hash(1),
            color_crc: node.hash(2),
            team_type: node.int(3),
            formation_crc: node.hash(4),
            texture_path: owned(node.string(5)),
            texture_name: owned(node.string(6)),
        })
    }
}

/// Parse toutes les équipes d'un `enjoy_mode_team_config_*.cfg.bin.json` désérialisé.
///
/// Réplique la structure d'inagle : on parcourt les entrées de tête, on retient les
/// noeuds-listes `ENJOY_MODE_TEAM_INFO_LIST_BEG*`, puis leurs enfants `ENJOY_MODE_TEAM_INFO_*`.
///
/// # Exemple
///
/// ```
/// use nie_data::team::parse_enjoy_mode_team_config;
/// use nie_data::hash::HashId;
/// use serde_json::json;
///
/// let root = json!({
///     "entries": [{
///         "name": "ENJOY_MODE_TEAM_INFO_LIST_BEG_0",
///         "variables": [{"type":"Int","value":"1"}],
///         "children": [{
///             "name": "ENJOY_MODE_TEAM_INFO_0",
///             "variables": [
///                 {"type":"Int","value":"901280304"},
///                 {"type":"Int","value":"810929954"},
///                 {"type":"Int","value":"-1306387704"},
///                 {"type":"Int","value":"1"},
///                 {"type":"Int","value":"585721253"},
///                 {"type":"String","value":"ev_chronicle_img/ev_bb_s10g001_01.g4tx"},
///                 {"type":"String","value":"ev_bb_s10g001_01"}
///             ],
///             "children": []
///         }]
///     }]
/// });
/// let teams = parse_enjoy_mode_team_config(&root);
/// assert_eq!(teams.len(), 1);
/// assert_eq!(teams[0].team_id, HashId(0x35B87230));
/// assert_eq!(teams[0].team_type, 1);
/// assert_eq!(teams[0].texture_name, "ev_bb_s10g001_01");
/// ```
#[must_use]
pub fn parse_enjoy_mode_team_config(root: &Value) -> Vec<EnjoyModeTeam> {
    let mut teams = Vec::new();
    let Some(entries) = root.get("entries").and_then(Value::as_array) else {
        return teams;
    };
    for entry in entries {
        let list = Node::new(entry);
        if !list.name().starts_with(LIST_PREFIX) {
            continue;
        }
        for child in list.children() {
            if !child.name().starts_with(ENTRY_PREFIX) {
                continue;
            }
            if let Some(team) = EnjoyModeTeam::from_node(child) {
                teams.push(team);
            }
        }
    }
    teams
}
