//! `EnjoyModeTeamConfig` — port Rust de `team/enjoy_mode_team_config_*.cfg.bin` (Level-5 IEVR).
//!
//! Équipes prédéfinies du **Mode Plaisir** (« enjoy mode ») : chaque entrée associe une équipe
//! (teamId/subId), sa formation, sa couleur et sa texture d'écusson. Famille game-data **jusqu'ici
//! non couverte** par nie-data.
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/team/enjoy_mode_team_config_1.04.02.00.cfg.bin.json`
//!   (format **`entries`** = arbre T2B ; racine `ENJOY_MODE_TEAM_INFO_LIST_BEG_0` → **28** enfants
//!   `ENJOY_MODE_TEAM_INFO_<i>`, chacun **12 variables** positionnelles).
//! - Référence de portage : `packages/inagle/src/parsers/enjoy-mode-team-config.ts` (`parseEntries`).
//!
//! ## Variables positionnelles (port 1:1 de la référence)
//!
//! | idx | champ            | type   | sémantique (inagle)                  |
//! |-----|------------------|--------|---------------------------------------|
//! | 0   | `team_id`        | hash   | CRC de l'équipe                       |
//! | 1   | `sub_id`         | hash   | CRC secondaire                        |
//! | 2   | `color_crc`      | hash   | CRC de la couleur                     |
//! | 3   | `team_type`      | int    | type d'équipe                         |
//! | 4   | `formation_crc`  | hash   | CRC de la formation                   |
//! | 5   | `texture_path`   | string | chemin g4tx de l'écusson              |
//! | 6   | `texture_name`   | string | nom de la texture                     |
//!
//! Les variables `7..=11` existent dans le dump mais sont **hétérogènes** (Int *ou* String selon
//! l'entrée, ex. base64 opaque) et **aucune source ne les décode** — non exposées (anti-hallucination :
//! on ne nomme pas ce qui n'a pas de vérité terrain). Le port reproduit exactement les 7 champs de la
//! référence validée.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::Node;
use crate::hash::HashId;

/// Une équipe prédéfinie du Mode Plaisir (`ENJOY_MODE_TEAM_INFO_<i>`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnjoyModeTeam {
    /// `var[0]` — CRC de l'équipe (`teamId`).
    pub team_id: HashId,
    /// `var[1]` — CRC secondaire (`subId`).
    pub sub_id: HashId,
    /// `var[2]` — CRC de la couleur (`colorCrc`).
    pub color_crc: HashId,
    /// `var[3]` — type d'équipe (`type`, int).
    pub team_type: i64,
    /// `var[4]` — CRC de la formation (`formationCrc`).
    pub formation_crc: HashId,
    /// `var[5]` — chemin g4tx de l'écusson (`texturePath`).
    pub texture_path: String,
    /// `var[6]` — nom de la texture (`textureName`).
    pub texture_name: String,
}

impl EnjoyModeTeam {
    /// Parse un noeud `ENJOY_MODE_TEAM_INFO_<i>`. `None` s'il a moins de 7 variables
    /// (port du `if (vars.length < 7) continue` d'inagle).
    #[must_use]
    pub fn from_node(node: &Node) -> Option<Self> {
        if node.var_count() < 7 {
            return None;
        }
        Some(Self {
            team_id: node.hash(0),
            sub_id: node.hash(1),
            color_crc: node.hash(2),
            team_type: node.int(3),
            formation_crc: node.hash(4),
            texture_path: node.string(5).into(),
            texture_name: node.string(6).into(),
        })
    }
}

/// Contenu complet d'un `enjoy_mode_team_config_*.cfg.bin` (les équipes du Mode Plaisir).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnjoyModeTeamConfig {
    /// Les équipes prédéfinies, dans l'ordre du fichier.
    pub teams: Vec<EnjoyModeTeam>,
}

impl EnjoyModeTeamConfig {
    /// Recherche une équipe par son `team_id`. `None` si absente.
    #[must_use]
    pub fn find_by_team_id(&self, team_id: HashId) -> Option<&EnjoyModeTeam> {
        self.teams.iter().find(|t| t.team_id == team_id)
    }
}

/// Parse un `enjoy_mode_team_config_*.cfg.bin.json` (format `entries`).
///
/// Port 1:1 de `parseEntries` (inagle) : pour chaque racine `ENJOY_MODE_TEAM_INFO_LIST_BEG*`,
/// itère les enfants `ENJOY_MODE_TEAM_INFO_*` et lit leurs 7 premières variables.
#[must_use]
pub fn parse_enjoy_mode_team_config(root: &Value) -> EnjoyModeTeamConfig {
    let mut teams = Vec::new();
    if let Some(entries) = root.get("entries").and_then(Value::as_array) {
        for e in entries {
            let node = Node::new(e);
            if !node.name().starts_with("ENJOY_MODE_TEAM_INFO_LIST_BEG") {
                continue;
            }
            for child in node.children() {
                if !child.name().starts_with("ENJOY_MODE_TEAM_INFO_") {
                    continue;
                }
                if let Some(team) = EnjoyModeTeam::from_node(&child) {
                    teams.push(team);
                }
            }
        }
    }
    EnjoyModeTeamConfig { teams }
}

/// Les équipes seules, sans le conteneur — la forme que portait le module `team`, fusionné ici
/// le **2026-09-06**.
///
/// # Pourquoi cette fonction existe
///
/// `team.rs` et ce module étaient **deux ports du même fichier**, arrivés dans le même commit
/// (`2291911`, 2026-08-10) : mêmes 7 variables, même `parseEntries` d'inagle en référence, même
/// `EnjoyModeTeam`. Aucune antériorité ne les départageait ; c'est ce module qui était câblé
/// dans `typed::decode_by_key` et cité par les docs, le C++ et le TypeScript, donc c'est lui
/// qui garde le nom. La doctrine du dépôt est de **fusionner, pas de supprimer** : la seule
/// chose que `team.rs` avait en propre — rendre un `Vec` plutôt qu'un conteneur — est ici.
///
/// # Exemple
///
/// ```
/// use nie_data::enjoy_mode_team::parse_enjoy_mode_teams;
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
/// let teams = parse_enjoy_mode_teams(&root);
/// assert_eq!(teams.len(), 1);
/// assert_eq!(teams[0].team_id, HashId(0x35B87230));
/// assert_eq!(teams[0].team_type, 1);
/// assert_eq!(teams[0].texture_name, "ev_bb_s10g001_01");
/// ```
#[must_use]
pub fn parse_enjoy_mode_teams(root: &Value) -> Vec<EnjoyModeTeam> {
    parse_enjoy_mode_team_config(root).teams
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Fixture reproduisant la structure réelle (valeurs copiées du dump `1.04.02.00`).
    fn fixture() -> Value {
        json!({
            "entries": [{
                "name": "ENJOY_MODE_TEAM_INFO_LIST_BEG_0",
                "variables": [{ "type": "Int", "value": "28" }],
                "children": [
                    {
                        "name": "ENJOY_MODE_TEAM_INFO_0",
                        "variables": [
                            { "type": "Int", "value": "901280304" },
                            { "type": "Int", "value": "810929954" },
                            { "type": "Int", "value": "-1306387704" },
                            { "type": "Int", "value": "1" },
                            { "type": "Int", "value": "585721253" },
                            { "type": "String", "value": "ev_chronicle_img/ev_bb_s10g001_01.g4tx" },
                            { "type": "String", "value": "ev_bb_s10g001_01" },
                            { "type": "Int", "value": "0" }
                        ],
                        "children": []
                    },
                    {
                        "name": "ENJOY_MODE_TEAM_INFO_1",
                        "variables": [
                            { "type": "Int", "value": "1424638790" },
                            { "type": "Int", "value": "-1641831799" },
                            { "type": "Int", "value": "-987419746" },
                            { "type": "Int", "value": "3" },
                            { "type": "Int", "value": "529091605" },
                            { "type": "String", "value": "ev_chronicle_img/ev_bb_s11g001_01.g4tx" },
                            { "type": "String", "value": "ev_bb_s11g001_01" }
                        ],
                        "children": []
                    }
                ]
            }]
        })
    }

    #[test]
    fn parse_deux_equipes() {
        let cfg = parse_enjoy_mode_team_config(&fixture());
        assert_eq!(cfg.teams.len(), 2);
    }

    #[test]
    fn team0_valeurs_byte_exact() {
        let cfg = parse_enjoy_mode_team_config(&fixture());
        let t = &cfg.teams[0];
        assert_eq!(t.team_id, HashId(0x35B8_7230)); // 901280304
        assert_eq!(t.sub_id, HashId(0x3055_CF22)); // 810929954
        assert_eq!(t.color_crc, HashId(0xB222_1B08)); // -1306387704 → u32
        assert_eq!(t.team_type, 1);
        assert_eq!(t.formation_crc, HashId(0x22E9_65A5)); // 585721253
        assert_eq!(t.texture_path, "ev_chronicle_img/ev_bb_s10g001_01.g4tx");
        assert_eq!(t.texture_name, "ev_bb_s10g001_01");
    }

    #[test]
    fn crc_signe_vers_non_signe() {
        // var[1] = -1641831799 (i32 signé) doit devenir 0x9E23A289 (u32), comme toHex(parseInt).
        let cfg = parse_enjoy_mode_team_config(&fixture());
        assert_eq!(cfg.teams[1].sub_id, HashId(0x9E23_A289));
        assert_eq!(cfg.teams[1].color_crc, HashId(0xC525_2B9E)); // -987419746
    }

    #[test]
    fn find_by_team_id() {
        let cfg = parse_enjoy_mode_team_config(&fixture());
        assert!(cfg.find_by_team_id(HashId(0x35B8_7230)).is_some());
        assert!(cfg.find_by_team_id(HashId(0xDEAD_BEEF)).is_none());
    }

    #[test]
    fn racine_absente_donne_vec_vide() {
        let cfg = parse_enjoy_mode_team_config(&json!({ "entries": [] }));
        assert!(cfg.teams.is_empty());
    }
}
