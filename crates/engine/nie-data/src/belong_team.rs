//! Famille `belong_team` — port Rust de `belong_team_config_*.cfg.bin` (affiliation
//! d'équipe des personnages : équipe d'appartenance, emblèmes/maillots par série et
//! numéros d'apparition par saison).
//!
//! ## Vérité terrain
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/belong-team.ts`
//!   (interface `BelongTeamInfo` l.24-45 + lecture `lists[0].values` l.213-234), port **1:1** ici.
//! - Dump réel : `data/common/gamedata/character/belong_team_config_0.00.00.cfg.bin`
//!   (VFS IEVR), format **RDBN** (`lists`) : une seule liste `m_belongTeamInfoList`
//!   (`typeName` `BELONG_TEAM_INFO`), **208 entrées**.
//!
//! ## Format (RDBN `lists`)
//!
//! Une seule liste `m_belongTeamInfoList`. Chaque valeur `BELONG_TEAM_INFO` porte
//! (champs réels du dump, dans l'ordre de l'interface inagle) :
//!
//! | champ inagle           | type | sémantique                                              |
//! |------------------------|------|---------------------------------------------------------|
//! | `belongTeamId`         | CRC  | identifiant d'équipe d'appartenance (clé stable)        |
//! | `binderTeamOrderType`  | Int  | ordre de tri dans le classeur (binder)                  |
//! | `teamNameTextId`       | CRC  | hash du texte de nom d'équipe (jointure `team_text`)    |
//! | `teamEmblemId_IE`      | CRC  | emblème série IE (`0x00000000` = aucun)                 |
//! | `teamEmblemId_GO`      | CRC  | emblème série GO                                         |
//! | `teamEmblemId_AREORI`  | CRC  | emblème séries Ares/Orion                                |
//! | `teamEmblemId_V`       | CRC  | emblème série Victory Road                               |
//! | `teamNumber_IE1..IE3`  | Int  | numéro d'apparition saisons IE1/IE2/IE3 (`0` = absent)  |
//! | `teamNumber_GO1..GO3`  | Int  | numéro d'apparition saisons GO1/GO2/GO3                 |
//! | `teamNumber_ARES`      | Int  | numéro d'apparition saison Ares                         |
//! | `teamNumber_ORION`     | Int  | numéro d'apparition saison Orion                        |
//! | `teamNumber_V`         | Int  | numéro d'apparition saison Victory Road                 |
//! | `teamKit_IE`           | CRC  | maillot série IE (`0x00000000` = aucun)                 |
//! | `teamKit_GO`           | CRC  | maillot série GO                                         |
//! | `teamKit_AREORI`       | CRC  | maillot séries Ares/Orion                                |
//! | `teamKit_V`            | CRC  | maillot série Victory Road                               |
//!
//! ## Hors scope (laissé au consommateur, comme les autres modules `nie-data`)
//!
//! La fusion texte (jointure `team_text` pour les noms localisés via `teamNameTextId`)
//! et l'enrichissement `charaSeriesInfo` que réalise `buildTeamMapping` (inagle l.266-320)
//! restent **hors** de ce port `no_std` : ce module n'expose que le noeud brut, comme
//! `opponent_team`/`team`.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, list_values};
use crate::hash::HashId;

/// Codes de saison de la franchise Inazuma Eleven (ordre des numéros d'apparition).
///
/// Port 1:1 du type `Season` d'inagle (`belong-team.ts` l.48).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    /// Inazuma Eleven, saison 1.
    Ie1,
    /// Inazuma Eleven, saison 2.
    Ie2,
    /// Inazuma Eleven, saison 3.
    Ie3,
    /// Inazuma Eleven GO, saison 1.
    Go1,
    /// Inazuma Eleven GO Chrono Stone.
    Go2,
    /// Inazuma Eleven GO Galaxy.
    Go3,
    /// Inazuma Eleven Ares.
    Ares,
    /// Inazuma Eleven Orion.
    Orion,
    /// Inazuma Eleven Victory Road.
    V,
}

/// Une entrée `BELONG_TEAM_INFO` (`m_belongTeamInfoList`) — une équipe d'appartenance.
///
/// Port 1:1 de l'interface `BelongTeamInfo` (inagle `belong-team.ts` l.24-45). Les champs
/// hash/CRC sont surfacés en [`HashId`] (recalcul au bit) ; `0x00000000` reste `HashId::ZERO`
/// (sentinel « aucun » qu'inagle traite en `undefined` côté enrichissement).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BelongTeamInfo {
    /// `belongTeamId` — identifiant d'équipe d'appartenance (clé stable).
    pub belong_team_id: HashId,
    /// `binderTeamOrderType` — ordre de tri dans le classeur (binder).
    pub binder_team_order_type: i64,
    /// `teamNameTextId` — hash du texte de nom d'équipe (jointure `team_text`).
    pub team_name_text_id: HashId,
    /// `teamEmblemId_IE` — emblème série IE (`HashId::ZERO` = aucun).
    pub team_emblem_id_ie: HashId,
    /// `teamEmblemId_GO` — emblème série GO.
    pub team_emblem_id_go: HashId,
    /// `teamEmblemId_AREORI` — emblème séries Ares/Orion.
    pub team_emblem_id_areori: HashId,
    /// `teamEmblemId_V` — emblème série Victory Road.
    pub team_emblem_id_v: HashId,
    /// `teamNumber_IE1` — numéro d'apparition saison IE1 (`0` = absent).
    pub team_number_ie1: i64,
    /// `teamNumber_IE2` — numéro d'apparition saison IE2.
    pub team_number_ie2: i64,
    /// `teamNumber_IE3` — numéro d'apparition saison IE3.
    pub team_number_ie3: i64,
    /// `teamNumber_GO1` — numéro d'apparition saison GO1.
    pub team_number_go1: i64,
    /// `teamNumber_GO2` — numéro d'apparition saison GO2 (Chrono Stone).
    pub team_number_go2: i64,
    /// `teamNumber_GO3` — numéro d'apparition saison GO3 (Galaxy).
    pub team_number_go3: i64,
    /// `teamNumber_ARES` — numéro d'apparition saison Ares.
    pub team_number_ares: i64,
    /// `teamNumber_ORION` — numéro d'apparition saison Orion.
    pub team_number_orion: i64,
    /// `teamNumber_V` — numéro d'apparition saison Victory Road.
    pub team_number_v: i64,
    /// `teamKit_IE` — maillot série IE (`HashId::ZERO` = aucun).
    pub team_kit_ie: HashId,
    /// `teamKit_GO` — maillot série GO.
    pub team_kit_go: HashId,
    /// `teamKit_AREORI` — maillot séries Ares/Orion.
    pub team_kit_areori: HashId,
    /// `teamKit_V` — maillot série Victory Road.
    pub team_kit_v: HashId,
}

impl BelongTeamInfo {
    /// Parse une valeur `BELONG_TEAM_INFO`. Port direct de la lecture brute d'inagle
    /// (`belong-team.ts` l.227-233 : `data.lists[0].values`, champs lus tels quels).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            belong_team_id: field_hash(v, "belongTeamId"),
            binder_team_order_type: field_i64(v, "binderTeamOrderType").unwrap_or(0),
            team_name_text_id: field_hash(v, "teamNameTextId"),
            team_emblem_id_ie: field_hash(v, "teamEmblemId_IE"),
            team_emblem_id_go: field_hash(v, "teamEmblemId_GO"),
            team_emblem_id_areori: field_hash(v, "teamEmblemId_AREORI"),
            team_emblem_id_v: field_hash(v, "teamEmblemId_V"),
            team_number_ie1: field_i64(v, "teamNumber_IE1").unwrap_or(0),
            team_number_ie2: field_i64(v, "teamNumber_IE2").unwrap_or(0),
            team_number_ie3: field_i64(v, "teamNumber_IE3").unwrap_or(0),
            team_number_go1: field_i64(v, "teamNumber_GO1").unwrap_or(0),
            team_number_go2: field_i64(v, "teamNumber_GO2").unwrap_or(0),
            team_number_go3: field_i64(v, "teamNumber_GO3").unwrap_or(0),
            team_number_ares: field_i64(v, "teamNumber_ARES").unwrap_or(0),
            team_number_orion: field_i64(v, "teamNumber_ORION").unwrap_or(0),
            team_number_v: field_i64(v, "teamNumber_V").unwrap_or(0),
            team_kit_ie: field_hash(v, "teamKit_IE"),
            team_kit_go: field_hash(v, "teamKit_GO"),
            team_kit_areori: field_hash(v, "teamKit_AREORI"),
            team_kit_v: field_hash(v, "teamKit_V"),
        }
    }

    /// Numéro d'apparition pour une saison donnée (`0` = absent de la saison).
    ///
    /// Équivalent de la table `teamNumber_*` lue par `buildTeamMapping` (inagle l.283-291),
    /// mais sans le filtre `> 0` (qui relève de l'enrichissement, laissé au consommateur).
    #[must_use]
    pub fn team_number(&self, season: Season) -> i64 {
        match season {
            Season::Ie1 => self.team_number_ie1,
            Season::Ie2 => self.team_number_ie2,
            Season::Ie3 => self.team_number_ie3,
            Season::Go1 => self.team_number_go1,
            Season::Go2 => self.team_number_go2,
            Season::Go3 => self.team_number_go3,
            Season::Ares => self.team_number_ares,
            Season::Orion => self.team_number_orion,
            Season::V => self.team_number_v,
        }
    }

    /// `true` si l'équipe apparaît dans la saison donnée (`teamNumber > 0`), comme le filtre
    /// d'inagle `if (raw.teamNumber_X > 0) seasons.X = …` (l.283-291).
    #[must_use]
    pub fn appears_in(&self, season: Season) -> bool {
        self.team_number(season) > 0
    }
}

/// Parse le `belong_team_config_*.cfg.bin.json` complet (forme iecode `lists`).
///
/// Port 1:1 de `loadBelongTeamInfo` (`belong-team.ts` l.214-234) : on lit l'unique liste
/// `m_belongTeamInfoList` ; liste absente → `Vec` vide (`return []`).
///
/// # Exemple
///
/// ```
/// use nie_data::belong_team::{parse_belong_team_config, Season};
/// use nie_data::hash::HashId;
/// use serde_json::json;
///
/// let root = json!({
///     "lists": [{
///         "name": "m_belongTeamInfoList",
///         "typeName": "BELONG_TEAM_INFO",
///         "values": [{
///             "belongTeamId": "0xF01BB293", "binderTeamOrderType": 1,
///             "teamNameTextId": "0x89A9174E",
///             "teamEmblemId_IE": "0xD5ABE1F0", "teamEmblemId_GO": "0x1D4B6E80",
///             "teamEmblemId_AREORI": "0x5AEB1450", "teamEmblemId_V": "0x87FE63EF",
///             "teamNumber_IE1": 1, "teamNumber_IE2": 18, "teamNumber_IE3": 0,
///             "teamNumber_GO1": 0, "teamNumber_GO2": 0, "teamNumber_GO3": 0,
///             "teamNumber_ARES": 1, "teamNumber_ORION": 0, "teamNumber_V": 7,
///             "teamKit_IE": "0x252CE113", "teamKit_GO": "0xAECB5B3E",
///             "teamKit_AREORI": "0x00000000", "teamKit_V": "0xBBE7C49D"
///         }]
///     }]
/// });
/// let teams = parse_belong_team_config(&root);
/// assert_eq!(teams.len(), 1);
/// assert_eq!(teams[0].belong_team_id, HashId(0xF01B_B293));
/// assert!(teams[0].appears_in(Season::Ie1));
/// assert!(!teams[0].appears_in(Season::Go1));
/// ```
#[must_use]
pub fn parse_belong_team_config(root: &Value) -> Vec<BelongTeamInfo> {
    list_values(root, "m_belongTeamInfoList")
        .map(|vs| vs.iter().map(BelongTeamInfo::from_value).collect())
        .unwrap_or_default()
}

/// Recherche une équipe d'appartenance par son `belongTeamId` (équivalent du `getTeamById`
/// d'inagle l.362-364).
#[must_use]
pub fn find_by_id(teams: &[BelongTeamInfo], id: HashId) -> Option<&BelongTeamInfo> {
    teams.iter().find(|t| t.belong_team_id == id)
}

/// Résout le **nom localisé** d'une équipe via une liste `team_text` (issue de
/// [`crate::text::parse_text_file`]).
///
/// Port de la jointure de `belong-team.ts` (`team_text.get(nameHash)`) : `team_name_text_id`
/// est la clé. `None` si absent.
#[must_use]
pub fn resolve_team_name<'a>(
    team: &BelongTeamInfo,
    team_text: &'a [(HashId, String)],
) -> Option<&'a str> {
    crate::text::find_text(team_text, team.team_name_text_id)
}

// NB (jointure crest équipe NON portée — 2026-06-15, vérifié end-to-end) : `team_emblem_id_v` ne
// s'apparie PAS par lookup direct dans `emblem_resource_*.cfg.bin` du jeu live (testé : 0/208 ;
// le fichier live n'a que **2** entrées = gabarit `default` + base). Le crest d'équipe est
// vraisemblablement **templé** (chemin `default` avec `<resourceID>` substitué via
// [`crate::emblems::resolve_resource_id`], cf. `emblems.ts` `isTemplate`) ou dans un fichier
// emblem plus complet. À élucider avant tout `resolve_emblem` (doctrine anti-faux-FAIT).
