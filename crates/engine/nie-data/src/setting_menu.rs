//! `SettingListConfig` — port Rust de `setting_menu/setting_list_config_*.cfg.bin.json`
//! (Level-5 IEVR : configuration du menu des réglages / options).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/setting_menu/setting_list_config_7.00.17.cfg.bin.json`
//! - Format **`entries`** (noeuds nommés, variables positionnelles) — comme `command` et `music_app`.
//! - Aucun parseur TS inagle n'existe pour cette famille : la **sémantique fine des colonnes
//!   est observée** (statistiques + recoupements), non confirmée par une source. Les noms de
//!   champs reflètent l'observation et sont documentés comme tels.
//!
//! Le fichier contient **11 noeuds-listes racines**. La var\[0\] du noeud `*_BEG_0` est le
//! nombre d'entrées **logiques** (pas forcément le nombre d'enfants : voir les listes INFO+REF).
//!
//! | Noeud racine                                    | var\[0\] | enfants | structure |
//! |-------------------------------------------------|---------|---------|-----------|
//! | `SETTING_INFO_LIST_BEG_0`                       | 73      | 73      | 73 × `SETTING_INFO_N` (8 vars) |
//! | `KEYCONFIG_SETTING_INFO_LIST_BEG_0`             | 72      | 72      | 72 × `KEYCONFIG_SETTING_INFO_N` (9 vars) |
//! | `SETTING_PLATFORM_TYPE_INFO_LIST_BEG_0`         | 7       | 7       | 7 × `SETTING_PLATFORM_TYPE_INFO_N` (9 vars : id + 8 flags) |
//! | `SETTING_OBJ_DATA_LIST_BEG_0`                    | 58      | 58      | pool de 58 hash (`SETTING_OBJ_DATA_N`, 1 var) |
//! | `SETTING_OBJ_INFO_LIST_BEG_0`                    | 29      | 58      | 29 × (`INFO_N` clé + `INFO_REF_DATA_N` \[offset,count\]) |
//! | `KEYCONFIG_MAPPING_LIST_MAPPING_DATA_LIST_BEG_0` | 268     | 268     | pool de 268 `MAPPING_DATA_N` (4 vars) |
//! | `KEYCONFIG_MAPPING_LIST_INFO_LIST_BEG_0`        | 86      | 172     | 86 × (`INFO_N` clé + `INFO_REF_MAPPING_DATA_N` \[offset,count\]) |
//! | `EXPLANATION_TEXT_LIST_TEXT_DATA_LIST_BEG_0`     | 91      | 91      | pool de 91 hash (`TEXT_DATA_N`, 1 var) |
//! | `EXPLANATION_TEXT_LIST_INFO_LIST_BEG_0`          | 90      | 180     | 90 × (`INFO_N` clé + `INFO_REF_TEXT_DATA_N` \[offset,count\]) |
//! | `PAD_GROUPKEY_LIST_DATA_LIST_BEG_0`             | 41      | 41      | pool de 41 entiers (`DATA_N`, 1 var) |
//! | `PAD_GROUPKEY_LIST_INFO_LIST_BEG_0`             | 11      | 22      | 11 × (`INFO_N` clé + `INFO_REF_DATA_N` \[offset,count\]) |
//!
//! ## Le motif INFO + REF (tableau aplati)
//!
//! Quatre listes suivent le motif Level-5 « struct contenant un tableau » : le tableau est
//! sérialisé séparément dans une liste-pool (`*_DATA_LIST`), et chaque entrée logique est
//! découpée en deux noeuds frères consécutifs :
//! - `*_INFO_N` — 1 variable : la **clé** (un hash de groupe) ;
//! - `*_INFO_REF_*_N` — 2 variables : `[offset, count]` dans le pool correspondant.
//!
//! Au parse, on résout chaque ref en découpant `pool[offset .. offset+count]`. Les valeurs
//! résolues sont stockées directement dans l'entrée (voir [`SettingObjInfo`],
//! [`KeyConfigMappingInfo`], [`ExplanationTextInfo`], [`PadGroupKeyInfo`]).
//!
//! ## Recoupements observés (validation croisée)
//!
//! - `EXPLANATION_TEXT_LIST_INFO_0` a pour clé `0x6299E064` (1654251620), qui est exactement
//!   `SETTING_INFO_0.help_text_id` (var\[3\]) → la liste d'explications est indexée par
//!   l'identifiant de texte d'aide d'un réglage.
//! - `SETTING_INFO_*.platform_type_id` (var\[7\]) ne prend que 7 valeurs distinctes, toutes
//!   présentes comme `SETTING_PLATFORM_TYPE_INFO_*.platform_id` (var\[0\]).
//! - `KEYCONFIG_MAPPING_LIST_INFO_0` a pour clé `0xC10919BC` (-1056368196), qui est la valeur
//!   de `MAPPING_DATA_*.var[1]` des 7 premières entrées du pool (sa plage `[0, 7]`).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::Node;
use crate::hash::HashId;

// ─── SettingInfo ──────────────────────────────────────────────────────────────

/// Entrée `SETTING_INFO_N` — un réglage du menu des options (8 variables positionnelles).
///
/// Vérité terrain : `setting_list_config_7.00.17`, `SETTING_INFO_0` =
/// `[0, 977641936, 1497198424, 1654251620, 1832944595, 1, 1, -1132488914]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SettingInfo {
    /// var\[0\] — genre/type de réglage (4 valeurs distinctes, surtout 0). Sémantique non confirmée.
    pub kind: i64,
    /// var\[1\] — identifiant hash du réglage (unique par entrée).
    pub setting_id: HashId,
    /// var\[2\] — hash du texte du nom affiché (jointure texte ; unique).
    pub name_text_id: HashId,
    /// var\[3\] — hash du texte d'aide/explication. Recoupe les clés d'`EXPLANATION_TEXT_LIST`.
    pub help_text_id: HashId,
    /// var\[4\] — hash de groupe/catégorie (25 valeurs distinctes, partagées par paires).
    pub group_id: HashId,
    /// var\[5\] — valeur ou ordre du réglage (entier ; non strictement séquentiel). Sémantique non confirmée.
    pub value: i64,
    /// var\[6\] — sous-genre (3 valeurs distinctes : 1, 2…). Sémantique non confirmée.
    pub sub_kind: i64,
    /// var\[7\] — hash du type de plateforme (recoupe `SettingPlatformType.platform_id` ; 7 valeurs).
    pub platform_type_id: HashId,
}

impl SettingInfo {
    /// Parse un noeud `SETTING_INFO_N`. `None` si `setting_id` (var\[1\]) est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let setting_id = node.hash(1);
        if setting_id.is_zero() {
            return None;
        }
        Some(Self {
            kind: node.int(0),
            setting_id,
            name_text_id: node.hash(2),
            help_text_id: node.hash(3),
            group_id: node.hash(4),
            value: node.int(5),
            sub_kind: node.int(6),
            platform_type_id: node.hash(7),
        })
    }
}

// ─── KeyConfigSettingInfo ───────────────────────────────────────────────────────

/// Entrée `KEYCONFIG_SETTING_INFO_N` — un réglage de configuration des touches (9 variables).
///
/// Vérité terrain : `setting_list_config_7.00.17`, `KEYCONFIG_SETTING_INFO_0` =
/// `[0, 460547571, -719460217, 7528472, 329380469, 2, 0, 0, 2]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyConfigSettingInfo {
    /// var\[0\] — genre (3 valeurs distinctes, surtout 0). Sémantique non confirmée.
    pub kind: i64,
    /// var\[1\] — identifiant hash de la touche/action (unique par entrée).
    pub key_id: HashId,
    /// var\[2\] — hash du texte du nom affiché (jointure texte ; unique).
    pub name_text_id: HashId,
    /// var\[3\] — hash de groupe (2 valeurs distinctes ; `0x0072E018` = 7528472 dominant).
    pub group_id: HashId,
    /// var\[4\] — hash de texte secondaire (aide/description ; 66 valeurs distinctes).
    pub help_text_id: HashId,
    /// var\[5\] — paramètre (4 valeurs distinctes : 1, 2…). Sémantique non confirmée.
    pub param_a: i64,
    /// var\[6\] — drapeau (2 valeurs distinctes : 0, 1). Sémantique non confirmée.
    pub param_b: i64,
    /// var\[7\] — hash de référence (9 valeurs distinctes, souvent 0). Sémantique non confirmée.
    pub ref_id: HashId,
    /// var\[8\] — paramètre (14 valeurs distinctes). Sémantique non confirmée.
    pub param_c: i64,
}

impl KeyConfigSettingInfo {
    /// Parse un noeud `KEYCONFIG_SETTING_INFO_N`. `None` si `key_id` (var\[1\]) est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let key_id = node.hash(1);
        if key_id.is_zero() {
            return None;
        }
        Some(Self {
            kind: node.int(0),
            key_id,
            name_text_id: node.hash(2),
            group_id: node.hash(3),
            help_text_id: node.hash(4),
            param_a: node.int(5),
            param_b: node.int(6),
            ref_id: node.hash(7),
            param_c: node.int(8),
        })
    }
}

// ─── SettingPlatformType ─────────────────────────────────────────────────────────

/// Entrée `SETTING_PLATFORM_TYPE_INFO_N` — capacités d'un type de plateforme (id + 8 drapeaux).
///
/// Vérité terrain : `setting_list_config_7.00.17`, `SETTING_PLATFORM_TYPE_INFO_0` =
/// `[-1132488914, 1, 1, 1, 1, 1, 1, 1, 1]` (toutes capacités activées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SettingPlatformType {
    /// var\[0\] — hash du type de plateforme (référencé par `SettingInfo.platform_type_id`).
    pub platform_id: HashId,
    /// var\[1..=8\] — 8 drapeaux de capacité (0/1). Sémantique individuelle non confirmée.
    pub flags: Vec<i64>,
}

impl SettingPlatformType {
    /// Parse un noeud `SETTING_PLATFORM_TYPE_INFO_N`. `None` si `platform_id` (var\[0\]) est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let platform_id = node.hash(0);
        if platform_id.is_zero() {
            return None;
        }
        let flags = (1..node.var_count()).map(|i| node.int(i)).collect();
        Some(Self { platform_id, flags })
    }
}

// ─── KeyConfigMappingData ────────────────────────────────────────────────────────

/// Entrée `KEYCONFIG_MAPPING_LIST_MAPPING_DATA_N` — une entrée du pool de mappages (4 variables).
///
/// Vérité terrain : `setting_list_config_7.00.17`,
/// `KEYCONFIG_MAPPING_LIST_MAPPING_DATA_0` = `[-1752149677, -1056368196, -1801524732, 0]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyConfigMappingData {
    /// var\[0\] — hash du mappage (25 valeurs distinctes).
    pub mapping_id: HashId,
    /// var\[1\] — hash du groupe parent (recoupe la clé de l'`INFO` propriétaire).
    pub group_id: HashId,
    /// var\[2\] — hash secondaire (20 valeurs distinctes, souvent 0). Sémantique non confirmée.
    pub sub_id: HashId,
    /// var\[3\] — drapeau (3 valeurs distinctes). Sémantique non confirmée.
    pub flag: i64,
}

impl KeyConfigMappingData {
    /// Parse un noeud `KEYCONFIG_MAPPING_LIST_MAPPING_DATA_N`.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Self {
        Self {
            mapping_id: node.hash(0),
            group_id: node.hash(1),
            sub_id: node.hash(2),
            flag: node.int(3),
        }
    }
}

// ─── Entrées à motif INFO + REF ───────────────────────────────────────────────────

/// Entrée `SETTING_OBJ_INFO_N` — groupe d'objets de réglage (clé + plage résolue de hash).
///
/// Vérité terrain : `SETTING_OBJ_INFO_2` (clé `0x6D4083D3` = 1832944595, ref `[0, 3]`) →
/// `objects = [0x9CEBE295, 0x05E2B32F, 0x72E583B9]` (3 hash découpés depuis `SETTING_OBJ_DATA_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SettingObjInfo {
    /// `INFO_N.var[0]` — clé de groupe (hash). Recoupe `SettingInfo.group_id` / `setting_id`.
    pub key: HashId,
    /// Plage `SETTING_OBJ_DATA[offset .. offset+count]` résolue (peut être vide).
    pub objects: Vec<HashId>,
}

/// Entrée `KEYCONFIG_MAPPING_LIST_INFO_N` — groupe de mappages de touches (clé + plage résolue).
///
/// Vérité terrain : `KEYCONFIG_MAPPING_LIST_INFO_0` (clé `0xC10919BC`, ref `[0, 7]`) →
/// 7 `KeyConfigMappingData` découpés depuis le pool `MAPPING_DATA_LIST`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyConfigMappingInfo {
    /// `INFO_N.var[0]` — clé de groupe (hash). Recoupe `KeyConfigMappingData.group_id`.
    pub key: HashId,
    /// Plage `MAPPING_DATA[offset .. offset+count]` résolue (peut être vide).
    pub mappings: Vec<KeyConfigMappingData>,
}

/// Entrée `EXPLANATION_TEXT_LIST_INFO_N` — bloc de texte d'explication (clé + plage résolue de hash).
///
/// Vérité terrain : `EXPLANATION_TEXT_LIST_INFO_0` (clé `0x6299E064` = 1654251620, ref `[0, 1]`)
/// → `lines = [0x3F96387B]` (1 hash de texte). La clé recoupe `SettingInfo.help_text_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExplanationTextInfo {
    /// `INFO_N.var[0]` — clé (hash de texte d'aide). Recoupe `SettingInfo.help_text_id`.
    pub key: HashId,
    /// Plage `TEXT_DATA[offset .. offset+count]` résolue : hash des lignes de texte (peut être vide).
    pub lines: Vec<HashId>,
}

/// Entrée `PAD_GROUPKEY_LIST_INFO_N` — groupe de touches manette (clé + plage résolue d'entiers).
///
/// Vérité terrain : `PAD_GROUPKEY_LIST_INFO_0` (clé `0xD3D99E8B`, ref `[0, 2]`) →
/// `keys = [12, 15]` (2 entiers découpés depuis `PAD_GROUPKEY_LIST_DATA_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PadGroupKeyInfo {
    /// `INFO_N.var[0]` — clé de groupe (hash).
    pub key: HashId,
    /// Plage `PAD_GROUPKEY_DATA[offset .. offset+count]` résolue : codes de touches (peut être vide).
    pub keys: Vec<i64>,
}

// ─── SettingListConfig ────────────────────────────────────────────────────────────

/// Contenu complet d'un `setting_list_config_*.cfg.bin.json` (les 11 listes).
///
/// Construire via [`parse_setting_list_config`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SettingListConfig {
    /// `SETTING_INFO_LIST` — 73 réglages du menu d'options.
    pub settings: Vec<SettingInfo>,
    /// `KEYCONFIG_SETTING_INFO_LIST` — 72 réglages de configuration des touches.
    pub keyconfig_settings: Vec<KeyConfigSettingInfo>,
    /// `SETTING_PLATFORM_TYPE_INFO_LIST` — 7 types de plateforme et leurs capacités.
    pub platform_types: Vec<SettingPlatformType>,
    /// `SETTING_OBJ_INFO_LIST` — 29 groupes d'objets de réglage (refs résolues).
    pub obj_infos: Vec<SettingObjInfo>,
    /// `KEYCONFIG_MAPPING_LIST_INFO_LIST` — 86 groupes de mappages (refs résolues).
    pub mapping_infos: Vec<KeyConfigMappingInfo>,
    /// `EXPLANATION_TEXT_LIST_INFO_LIST` — 90 blocs de texte d'explication (refs résolues).
    pub explanation_texts: Vec<ExplanationTextInfo>,
    /// `PAD_GROUPKEY_LIST_INFO_LIST` — 11 groupes de touches manette (refs résolues).
    pub pad_groupkeys: Vec<PadGroupKeyInfo>,
}

impl SettingListConfig {
    /// Recherche un réglage par son `setting_id`. `None` si absent.
    #[must_use]
    pub fn find_setting(&self, setting_id: HashId) -> Option<&SettingInfo> {
        self.settings.iter().find(|s| s.setting_id == setting_id)
    }

    /// Recherche un type de plateforme par son `platform_id`. `None` si absent.
    #[must_use]
    pub fn find_platform_type(&self, platform_id: HashId) -> Option<&SettingPlatformType> {
        self.platform_types
            .iter()
            .find(|p| p.platform_id == platform_id)
    }

    /// Résout le type de plateforme référencé par un réglage (`platform_type_id`).
    #[must_use]
    pub fn platform_of(&self, setting: &SettingInfo) -> Option<&SettingPlatformType> {
        self.find_platform_type(setting.platform_type_id)
    }
}

// ─── Parseur principal ────────────────────────────────────────────────────────────

/// Récupère le noeud racine de nom exact `name` dans `root["entries"]`.
fn root_node<'a>(root: &'a Value, name: &str) -> Option<Node<'a>> {
    let entries = root.get("entries")?.as_array()?;
    entries.iter().map(Node::new).find(|n| n.name() == name)
}

/// Parse un pool simple (1 variable par enfant) via `f` appliqué à chaque noeud.
fn parse_pool<T, F>(root: &Value, list_name: &str, f: F) -> Vec<T>
where
    F: Fn(Node<'_>) -> T,
{
    let Some(node) = root_node(root, list_name) else {
        return Vec::new();
    };
    node.children().into_iter().map(f).collect()
}

/// Parse une liste de scalaires (`SETTING_INFO_LIST` etc.) via `f`, ignorant les entrées nulles.
fn parse_scalar_list<T, F>(root: &Value, list_name: &str, f: F) -> Vec<T>
where
    F: Fn(Node<'_>) -> Option<T>,
{
    let Some(node) = root_node(root, list_name) else {
        return Vec::new();
    };
    node.children().into_iter().filter_map(f).collect()
}

/// Découpe `pool[offset .. offset+count]` en clonant ; tronque si la plage déborde.
fn slice_pool<T: Clone>(pool: &[T], offset: i64, count: i64) -> Vec<T> {
    let start = offset.max(0) as usize;
    let len = count.max(0) as usize;
    let end = start.saturating_add(len).min(pool.len());
    pool.get(start.min(pool.len())..end)
        .map(<[T]>::to_vec)
        .unwrap_or_default()
}

/// Parse une liste à motif INFO + REF : enfants en paires (`INFO_N` clé, `*_REF_*_N` \[offset,count\]).
///
/// `build` reçoit la clé et la plage résolue depuis `pool`, et produit l'entrée.
fn parse_ref_list<T, P, B>(root: &Value, list_name: &str, pool: &[P], build: B) -> Vec<T>
where
    P: Clone,
    B: Fn(HashId, Vec<P>) -> T,
{
    let Some(node) = root_node(root, list_name) else {
        return Vec::new();
    };
    let children = node.children();
    let mut out = Vec::new();
    let mut i = 0;
    while i < children.len() {
        let info = children[i];
        // L'enfant suivant doit être le noeud REF (contient "_REF_").
        if let Some(refn) = children.get(i + 1).copied()
            && refn.name().contains("_REF_")
        {
            let key = info.hash(0);
            let resolved = slice_pool(pool, refn.int(0), refn.int(1));
            out.push(build(key, resolved));
            i += 2;
            continue;
        }
        // Pas de ref appariée : entrée à plage vide (robustesse).
        out.push(build(info.hash(0), Vec::new()));
        i += 1;
    }
    out
}

/// Parse un `setting_list_config_*.cfg.bin.json` complet (11 listes, refs résolues).
///
/// Compte réel (`setting_list_config_7.00.17`) : 73 settings, 72 keyconfig settings,
/// 7 platform types, 29 obj infos, 86 mapping infos, 90 explanation texts, 11 pad groupkeys.
#[must_use]
pub fn parse_setting_list_config(root: &Value) -> SettingListConfig {
    // Pools (résolus avant les listes INFO qui les référencent).
    let obj_data: Vec<HashId> = parse_pool(root, "SETTING_OBJ_DATA_LIST_BEG_0", |n| n.hash(0));
    let mapping_data: Vec<KeyConfigMappingData> = parse_pool(
        root,
        "KEYCONFIG_MAPPING_LIST_MAPPING_DATA_LIST_BEG_0",
        KeyConfigMappingData::from_node,
    );
    let text_data: Vec<HashId> =
        parse_pool(root, "EXPLANATION_TEXT_LIST_TEXT_DATA_LIST_BEG_0", |n| {
            n.hash(0)
        });
    let pad_data: Vec<i64> = parse_pool(root, "PAD_GROUPKEY_LIST_DATA_LIST_BEG_0", |n| n.int(0));

    SettingListConfig {
        settings: parse_scalar_list(root, "SETTING_INFO_LIST_BEG_0", SettingInfo::from_node),
        keyconfig_settings: parse_scalar_list(
            root,
            "KEYCONFIG_SETTING_INFO_LIST_BEG_0",
            KeyConfigSettingInfo::from_node,
        ),
        platform_types: parse_scalar_list(
            root,
            "SETTING_PLATFORM_TYPE_INFO_LIST_BEG_0",
            SettingPlatformType::from_node,
        ),
        obj_infos: parse_ref_list(
            root,
            "SETTING_OBJ_INFO_LIST_BEG_0",
            &obj_data,
            |key, objects| SettingObjInfo { key, objects },
        ),
        mapping_infos: parse_ref_list(
            root,
            "KEYCONFIG_MAPPING_LIST_INFO_LIST_BEG_0",
            &mapping_data,
            |key, mappings| KeyConfigMappingInfo { key, mappings },
        ),
        explanation_texts: parse_ref_list(
            root,
            "EXPLANATION_TEXT_LIST_INFO_LIST_BEG_0",
            &text_data,
            |key, lines| ExplanationTextInfo { key, lines },
        ),
        pad_groupkeys: parse_ref_list(
            root,
            "PAD_GROUPKEY_LIST_INFO_LIST_BEG_0",
            &pad_data,
            |key, keys| PadGroupKeyInfo { key, keys },
        ),
    }
}
