//! `GimmickSystemNumConfig` — port Rust de `dungeon/gimmick_system_num_config.cfg.bin.json`
//! (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `/home/ubuntu/niers/data/common/gamedata/dungeon/gimmick_system_num_config.cfg.bin.json`
//! - Format : `entries` (noeuds nommés, variables positionnelles).
//! - Structure récursive : **4 groupes** (`DUNGEON_NUM_TABLE_GROUP_0..3`) imbriqués via leurs
//!   entrées de données. GROUP_0 contient 6 entrées, dont DATA_5 pointe vers GROUP_1 (3
//!   entrées), dont DATA_8 pointe vers GROUP_2 (1 entrée), dont DATA_9 pointe vers GROUP_3
//!   (1 entrée + 3 infos système).
//! - **11 entrées de données** réparties : GROUP_0=6, GROUP_1=3, GROUP_2=1, GROUP_3=1.
//! - **3 infos système** (`DUNGEON_NUM_SYSTEM_INFO_0..2`) nichées dans `DATA_LIST_BEG_3`,
//!   chacune portant 0 ou 1 sous-groupe (`DUNGEON_NUM_SYSTEM_INFO_GROUP_*`).
//!
//! ## Structure des noeuds
//!
//! | Noeud                             | Variables positionnelles              |
//! |-----------------------------------|---------------------------------------|
//! | `DUNGEON_NUM_TABLE_GROUP_N`       | \[0\] group_id : `Int` (HashId)       |
//! | `DUNGEON_NUM_TABLE_GROUP_DATA_N`  | \[0\] param_id, \[1\] valeur (Int ou Float), \[2\] flag |
//! | `DUNGEON_NUM_SYSTEM_INFO_N`       | \[0\] info_id, \[1\] group_ref        |
//! | `DUNGEON_NUM_SYSTEM_INFO_GROUP_N` | \[0\] group_id, \[1\] charge binaire base64 |
//!
//! ## Hors périmètre (z01_debug/)
//!
//! - `z01_debug_gimmick_layout.cfg.bin.json` — noeuds `LAYOUT_BASE_*` (entries) : non porté.
//! - `z01_debug_gimmick_oneplace_0.00.00.cfg.bin.json` — `m_GimmickOneplaceList` (lists) : non porté.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

// ─── GimmickNumTableGroupData ─────────────────────────────────────────────────

/// Entrée de paramètre numérique dungeon (`DUNGEON_NUM_TABLE_GROUP_DATA_*`).
///
/// Chaque entrée associe un identifiant de paramètre (`param_id`) à une valeur numérique
/// pouvant être entière ou flottante selon la définition du paramètre.
///
/// Vérité terrain — `gimmick_system_num_config.cfg.bin.json` :
/// - `DATA_0` : `param_id = -1334119126`, `value = 19`, `flag = 0` (ligne 36–45).
/// - `DATA_3` : `param_id = 547509227`, `value = "1,3"` (Float), `flag = 0` (ligne 91–100).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GimmickNumTableGroupData {
    /// var\[0\] — identifiant du paramètre numérique (hash).
    pub param_id: HashId,
    /// var\[1\] — valeur du paramètre (entier ou flottant selon le paramètre ; stocké en `f64`).
    ///
    /// Exemples réels : `19.0`, `5.0`, `100.0`, `1.3`, `80.0`, `0.0`, `-10.0`, `40.0`, `2.5`.
    pub value: f64,
    /// var\[2\] — flag (vaut `0` dans tous les noeuds du fichier réel).
    pub flag: i64,
}

// ─── GimmickNumTableGroup ─────────────────────────────────────────────────────

/// Groupe de paramètres numériques dungeon (`DUNGEON_NUM_TABLE_GROUP_*`).
///
/// La structure est récursive : certaines entrées de données (`GimmickNumTableGroupData`)
/// portent elles-mêmes un groupe imbriqué. Le parseur collecte tous les groupes à plat
/// (4 groupes dans le fichier réel), chacun avec ses entrées directes.
///
/// Vérité terrain — `gimmick_system_num_config.cfg.bin.json` :
/// - GROUP_0 : `group_id = -1568497890`, 6 entrées (lignes 13–19).
/// - GROUP_1 : `group_id = 749769860`, 3 entrées (ligne 139–141).
/// - GROUP_2 : `group_id = 1647881546`, 1 entrée (ligne 210–211).
/// - GROUP_3 : `group_id = 580647011`, 1 entrée (ligne 244–246).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GimmickNumTableGroup {
    /// var\[0\] — identifiant du groupe (hash).
    pub group_id: HashId,
    /// Entrées de paramètres directement portées par ce groupe.
    pub entries: Vec<GimmickNumTableGroupData>,
}

// ─── GimmickNumSystemInfoGroup ────────────────────────────────────────────────

/// Sous-groupe d'une info système dungeon (`DUNGEON_NUM_SYSTEM_INFO_GROUP_*`).
///
/// Vérité terrain :
/// - `DUNGEON_NUM_SYSTEM_INFO_GROUP_0` : `group_id = 1647881546` (= GROUP_2),
///   `payload_b64 = "AAAAAA8FNbkZNtoAAQAyAAAAAHE="` (ligne 316–319).
/// - `DUNGEON_NUM_SYSTEM_INFO_GROUP_1` : `group_id = 749769860` (= GROUP_1),
///   `payload_b64 = "AAAAABgFNSo9RUMACgEoAAYCNEIVKDMyAAAAAXg="` (ligne 346–350).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GimmickNumSystemInfoGroup {
    /// var\[0\] — identifiant du groupe référencé (hash).
    pub group_id: HashId,
    /// var\[1\] — charge utile binaire encodée en base64.
    pub payload_b64: String,
}

// ─── GimmickNumSystemInfo ─────────────────────────────────────────────────────

/// Info système dungeon (`DUNGEON_NUM_SYSTEM_INFO_*`).
///
/// Vérité terrain — `gimmick_system_num_config.cfg.bin.json` :
/// - `INFO_0` : `info_id = 1594010342`, `group_ref = -1568497890` (= GROUP_0), 1 sous-groupe.
/// - `INFO_1` : `info_id = 2025856298`, `group_ref = -1568497890` (= GROUP_0), 1 sous-groupe.
/// - `INFO_2` : `info_id = 1009861791`, `group_ref = 580647011` (= GROUP_3), 0 sous-groupe.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GimmickNumSystemInfo {
    /// var\[0\] — identifiant de l'info système (hash).
    pub info_id: HashId,
    /// var\[1\] — référence vers un groupe (`GimmickNumTableGroup::group_id`).
    pub group_ref: HashId,
    /// Sous-groupes portés par cette info (0 ou 1 dans le fichier réel).
    pub sub_groups: Vec<GimmickNumSystemInfoGroup>,
}

// ─── GimmickSystemNumConfig ───────────────────────────────────────────────────

/// Contenu complet de `gimmick_system_num_config.cfg.bin.json`.
///
/// Utiliser [`parse_gimmick_system_num_config`] pour construire depuis un JSON désérialisé.
/// Les groupes sont extraits à plat (tous niveaux d'imbrication) ; l'ordre de parcours
/// est profondeur d'abord (GROUP_0 en premier, GROUP_3 en dernier).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GimmickSystemNumConfig {
    /// Groupes de paramètres numériques (`DUNGEON_NUM_TABLE_GROUP_*`), tous niveaux confondus.
    /// Compte réel : 4 groupes.
    pub groups: Vec<GimmickNumTableGroup>,
    /// Infos système (`DUNGEON_NUM_SYSTEM_INFO_*`), tous niveaux confondus.
    /// Compte réel : 3 infos.
    pub system_infos: Vec<GimmickNumSystemInfo>,
}

impl GimmickSystemNumConfig {
    /// Recherche un groupe par son `group_id`. `None` si absent.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::dungeon::GimmickSystemNumConfig;
    /// use nie_data::hash::HashId;
    ///
    /// let cfg = GimmickSystemNumConfig { groups: vec![], system_infos: vec![] };
    /// assert!(cfg.find_group(HashId(0xDEADBEEF)).is_none());
    /// ```
    #[must_use]
    pub fn find_group(&self, group_id: HashId) -> Option<&GimmickNumTableGroup> {
        self.groups.iter().find(|g| g.group_id == group_id)
    }

    /// Recherche une info système par son `info_id`. `None` si absente.
    #[must_use]
    pub fn find_system_info(&self, info_id: HashId) -> Option<&GimmickNumSystemInfo> {
        self.system_infos.iter().find(|i| i.info_id == info_id)
    }
}

// ─── Parseur principal ────────────────────────────────────────────────────────

/// Parse un `gimmick_system_num_config.cfg.bin.json` complet.
///
/// Renvoie un [`GimmickSystemNumConfig`] avec tous les groupes et infos système extraits.
/// Le parcours est récursif (profondeur d'abord), les entrées invalides sont ignorées.
///
/// Comptes réels (`gimmick_system_num_config.cfg.bin.json`) :
/// - 4 groupes (`DUNGEON_NUM_TABLE_GROUP_0..3`).
/// - 11 entrées de données (`DUNGEON_NUM_TABLE_GROUP_DATA_0..10`).
/// - 3 infos système (`DUNGEON_NUM_SYSTEM_INFO_0..2`).
#[must_use]
pub fn parse_gimmick_system_num_config(root: &Value) -> GimmickSystemNumConfig {
    let mut groups = Vec::new();
    walk_named(root, "DUNGEON_NUM_TABLE_GROUP_", |node| {
        // Ignorer les conteneurs (_LIST_) et les entrées de données (_DATA_)
        let name = node.name();
        if name.contains("_DATA_") || name.contains("_LIST_") {
            return;
        }
        // Noeud DUNGEON_NUM_TABLE_GROUP_N — var[0] = group_id
        let group_id = node.hash(0);
        let entries = collect_data_entries(node);
        groups.push(GimmickNumTableGroup { group_id, entries });
    });

    let mut system_infos = Vec::new();
    walk_named(root, "DUNGEON_NUM_SYSTEM_INFO_", |node| {
        // Ignorer les conteneurs (_LIST_) et les sous-groupes (_GROUP_)
        let name = node.name();
        if name.contains("_GROUP_") || name.contains("_LIST_") {
            return;
        }
        // Noeud DUNGEON_NUM_SYSTEM_INFO_N — var[0] = info_id, var[1] = group_ref
        let info_id = node.hash(0);
        let group_ref = node.hash(1);
        let sub_groups = collect_system_info_groups(node);
        system_infos.push(GimmickNumSystemInfo {
            info_id,
            group_ref,
            sub_groups,
        });
    });

    GimmickSystemNumConfig {
        groups,
        system_infos,
    }
}

// ─── Helpers internes ─────────────────────────────────────────────────────────

/// Collecte les entrées de données directes d'un groupe.
///
/// Parcourt les enfants `DUNGEON_NUM_TABLE_GROUP_DATA_LIST_BEG_N` du noeud groupe,
/// puis les enfants `DUNGEON_NUM_TABLE_GROUP_DATA_N` de chaque conteneur.
fn collect_data_entries(group_node: Node<'_>) -> Vec<GimmickNumTableGroupData> {
    let mut entries = Vec::new();
    for list_beg in group_node.children() {
        // Seuls les conteneurs de données nous intéressent
        if !list_beg
            .name()
            .starts_with("DUNGEON_NUM_TABLE_GROUP_DATA_LIST_BEG_")
        {
            continue;
        }
        for data_node in list_beg.children() {
            let data_name = data_node.name();
            // Entrées DATA_N uniquement (pas les LIST_BEG ni les SYSTEM_INFO_LIST_BEG)
            if data_name.starts_with("DUNGEON_NUM_TABLE_GROUP_DATA_")
                && !data_name.contains("_LIST_")
            {
                // var[1] peut être Int ou Float ("1,3", "2,5") — as_f64 gère la virgule
                let value = data_node.var(1).map_or(0.0, |v| v.as_f64());
                entries.push(GimmickNumTableGroupData {
                    param_id: data_node.hash(0),
                    value,
                    flag: data_node.int(2),
                });
            }
        }
    }
    entries
}

/// Collecte les sous-groupes d'une info système.
///
/// Parcourt les enfants `DUNGEON_NUM_SYSTEM_INFO_GROUP_LIST_BEG_N` du noeud info,
/// puis les enfants `DUNGEON_NUM_SYSTEM_INFO_GROUP_N` de chaque conteneur.
fn collect_system_info_groups(info_node: Node<'_>) -> Vec<GimmickNumSystemInfoGroup> {
    let mut sub_groups = Vec::new();
    for list_beg in info_node.children() {
        if !list_beg
            .name()
            .starts_with("DUNGEON_NUM_SYSTEM_INFO_GROUP_LIST_BEG_")
        {
            continue;
        }
        for group_node in list_beg.children() {
            let gname = group_node.name();
            if gname.starts_with("DUNGEON_NUM_SYSTEM_INFO_GROUP_") && !gname.contains("_LIST_") {
                // var[0] = group_id (Int), var[1] = payload (String base64)
                sub_groups.push(GimmickNumSystemInfoGroup {
                    group_id: group_node.hash(0),
                    payload_b64: String::from(group_node.string(1)),
                });
            }
        }
    }
    sub_groups
}
