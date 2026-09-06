//! `SearchWordConfig` + `InfoBookmarkConfig` — port Rust de la famille
//! `search_word/*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! Deux fichiers dans `data/common/gamedata/search_word/` :
//!
//! ### `search_word_config.cfg.bin.json` (format `entries`)
//!
//! Un seul nœud racine `SEARCH_WORD_INFO_LIST_BEG_0` (var\[0\]=55, le compte),
//! avec 55 enfants `SEARCH_WORD_INFO_N` à 3 variables positionnelles chacun :
//!
//! | var | sémantique | exemple (entrée 0) |
//! |-----|-----------|-------------------|
//! | \[0\] | `search_word_id` (HashId) | 65287569 → 0x03E43591 |
//! | \[1\] | `sort_order` (entier 1-basé) | 1 |
//! | \[2\] | `secondary_hash` (HashId, sémantique inconnue sans source TS) | 193949743 |
//!
//! Note : le `sort_order` saute de 51 à 53 (pas de valeur 52) à l'entrée d'index 51.
//! Le `search_word_id` recoupe exactement les `searchWordId` de `m_BookmarkFolderItemList`.
//!
//! ### `info_bookmark_config_0.00.00.cfg.bin.json` (format `lists`, version 100)
//!
//! Trois listes :
//! - `m_TexBasePathList` (1 chemin de base texture) ;
//! - `m_BookmarkFolderItemList` (55 dossiers de marque-pages, référençant chacun un
//!   `search_word_id`) ;
//! - `m_InfoBookmarkDataList` (11 groupes de marque-pages, chacun pointant une plage de
//!   `folder_items` via `bookmarkFolderItemList = [offset, count]`).
//!
//! ## Utilisation envisagée
//!
//! `InfoBookmarkConfig::folder_items_of` résout la plage de `BookmarkFolderItem` d'un groupe.
//! `InfoBookmarkConfig::find_folder_item` recherche un dossier par `search_word_id`, utile pour
//! lier les deux fichiers (chaque `SearchWordInfo.search_word_id` correspond à un
//! `BookmarkFolderItem.search_word_id`).

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{
    Node, field_bool, field_hash, field_i64, field_str, list_values, owned, walk_named,
};
use crate::hash::HashId;

// ─── SearchWordInfo ────────────────────────────────────────────────────────────

/// Entrée `SEARCH_WORD_INFO_N` — mot-clé de recherche du jeu.
///
/// Format `entries` (variables positionnelles), fichier
/// `search_word_config.cfg.bin.json`. Les 55 entrées réelles ont
/// des `search_word_id` qui recoupent exactement les `searchWordId`
/// de `m_BookmarkFolderItemList` dans `info_bookmark_config`.
///
/// # Exemple
///
/// ```
/// use nie_data::search_word::SearchWordInfo;
/// use nie_data::hash::HashId;
/// use serde_json::json;
///
/// let root = json!({
///     "entries": [{
///         "name": "SEARCH_WORD_INFO_LIST_BEG_0",
///         "variables": [{"type": "Int", "value": "1"}],
///         "children": [{
///             "name": "SEARCH_WORD_INFO_0",
///             "variables": [
///                 {"type": "Int", "value": "65287569"},
///                 {"type": "Int", "value": "1"},
///                 {"type": "Int", "value": "193949743"}
///             ],
///             "children": []
///         }]
///     }]
/// });
/// let words = nie_data::search_word::parse_search_word_config(&root);
/// assert_eq!(words.len(), 1);
/// assert_eq!(words[0].search_word_id, HashId(0x03E43591));
/// assert_eq!(words[0].sort_order, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SearchWordInfo {
    /// var\[0\] — identifiant du mot de recherche.
    ///
    /// Vérité terrain : `search_word_config.cfg.bin.json` SEARCH_WORD_INFO_0.var\[0\]
    /// = 65287569 → 0x03E43591 (recoupé dans `m_BookmarkFolderItemList[0].searchWordId`).
    pub search_word_id: HashId,
    /// var\[1\] — ordre de tri (entier 1-basé ; saute la valeur 52 à l'index 51).
    ///
    /// Vérité terrain : SEARCH_WORD_INFO_0.var\[1\] = 1, …,
    /// SEARCH_WORD_INFO_51.var\[1\] = 53 (pas de 52).
    pub sort_order: i64,
    /// var\[2\] — hash secondaire (sémantique inconnue sans source TS inagle).
    ///
    /// Vérité terrain : SEARCH_WORD_INFO_0.var\[2\] = 193949743 → HashId(0x0B8F702F).
    pub secondary_hash: HashId,
}

impl SearchWordInfo {
    /// Parse un nœud `SEARCH_WORD_INFO_N`. Renvoie `None` si `search_word_id` est nul.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let search_word_id = node.hash(0);
        if search_word_id.is_zero() {
            return None;
        }
        Some(Self {
            search_word_id,
            sort_order: node.int(1),
            secondary_hash: node.hash(2),
        })
    }
}

// ─── TexBasePath ───────────────────────────────────────────────────────────────

/// Chemin de base des textures — `m_TexBasePathList` (TEX_BASE_PATH).
///
/// Vérité terrain : `info_bookmark_config_0.00.00.cfg.bin.json` contient 1 entrée :
/// `basePath = "#/menu/220_img/bookmark_img/<LG>/"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TexBasePath {
    /// `basePath` — chemin de base avec marqueur de langue `<LG>`.
    pub base_path: String,
}

impl TexBasePath {
    /// Parse une entrée de `m_TexBasePathList`. `None` si `basePath` est absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let base_path = field_str(v, "basePath")?;
        Some(Self {
            base_path: owned(base_path),
        })
    }
}

// ─── BookmarkFolderItem ────────────────────────────────────────────────────────

/// Dossier de marque-pages — `m_BookmarkFolderItemList` (BOOKMARK_FOLDER_INFO).
///
/// Chaque entrée référence un `search_word_id` (lien vers [`SearchWordInfo`]),
/// des identifiants texte pour le nom et la description, et une texture miniature.
///
/// Vérité terrain : `info_bookmark_config_0.00.00.cfg.bin.json`
/// `m_BookmarkFolderItemList[0]` :
/// - `searchWordId = 0x03E43591`, `wordTextId = 0xD21CE615`,
///   `descTextId = 0x917DEE71`, `thumbnailTexFileName = "bookmark_img01_s01.g4tx"`,
///   `isNecessaryStory = true`, `enableCond = ""`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BookmarkFolderItem {
    /// `searchWordId` — lien vers `SearchWordInfo.search_word_id`.
    pub search_word_id: HashId,
    /// `wordTextId` — hash du nom affiché du marque-page (jointure texte).
    pub word_text_id: HashId,
    /// `descTextId` — hash de la description du marque-page.
    pub desc_text_id: HashId,
    /// `thumbnailTexName` — hash du nom interne de la texture miniature.
    pub thumbnail_tex_name: HashId,
    /// `thumbnailTexFileName` — nom de fichier de la texture miniature (`.g4tx`).
    pub thumbnail_tex_file_name: String,
    /// `isNecessaryStory` — `true` si ce marque-page est lié à la progression scénaristique.
    pub is_necessary_story: bool,
    /// `enableCond` — condition d'activation (chaîne vide dans tous les cas réels).
    pub enable_cond: String,
}

impl BookmarkFolderItem {
    /// Parse une entrée de `m_BookmarkFolderItemList`. `None` si `searchWordId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let search_word_id = field_hash(v, "searchWordId");
        if search_word_id.is_zero() {
            return None;
        }
        Some(Self {
            search_word_id,
            word_text_id: field_hash(v, "wordTextId"),
            desc_text_id: field_hash(v, "descTextId"),
            thumbnail_tex_name: field_hash(v, "thumbnailTexName"),
            thumbnail_tex_file_name: owned(field_str(v, "thumbnailTexFileName").unwrap_or("")),
            is_necessary_story: field_bool(v, "isNecessaryStory").unwrap_or(false),
            enable_cond: owned(field_str(v, "enableCond").unwrap_or("")),
        })
    }
}

// ─── InfoBookmarkData ──────────────────────────────────────────────────────────

/// Groupe de marque-pages — `m_InfoBookmarkDataList` (INFO_BOOKMARK_DATA_CONFIG).
///
/// `folder_item_offset` + `folder_item_count` référencent une plage dans
/// `m_BookmarkFolderItemList` — utiliser [`InfoBookmarkConfig::folder_items_of`]
/// pour la résoudre.
///
/// Vérité terrain : `info_bookmark_config_0.00.00.cfg.bin.json`
/// `m_InfoBookmarkDataList[0]` :
/// - `bookmarkId = 0x2E2C6020`, `baseNo = 0`,
///   `folderNameId = 0x01812C2A`,
///   `thumbnailTexFileName = "bookmark_img01_l01.g4tx"`,
///   `bookmarkFolderItemList = [0, 9]` → offset=0, count=9.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InfoBookmarkData {
    /// `bookmarkId` — identifiant du groupe de marque-pages.
    pub bookmark_id: HashId,
    /// `baseNo` — numéro d'ordre du groupe (0-basé, séquentiel).
    pub base_no: i64,
    /// `folderNameId` — hash du nom affiché du groupe (jointure texte).
    pub folder_name_id: HashId,
    /// `thumbnailTexName` — hash du nom interne de la texture miniature.
    pub thumbnail_tex_name: HashId,
    /// `thumbnailTexFileName` — nom de fichier de la texture miniature grand format (`.g4tx`).
    pub thumbnail_tex_file_name: String,
    /// `bookmarkFolderItemList[0]` — index de début dans `m_BookmarkFolderItemList`.
    pub folder_item_offset: i64,
    /// `bookmarkFolderItemList[1]` — nombre de dossiers dans ce groupe.
    pub folder_item_count: i64,
}

impl InfoBookmarkData {
    /// Parse une entrée de `m_InfoBookmarkDataList`. `None` si `bookmarkId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let bookmark_id = field_hash(v, "bookmarkId");
        if bookmark_id.is_zero() {
            return None;
        }
        let bfil = v.get("bookmarkFolderItemList").and_then(Value::as_array);
        Some(Self {
            bookmark_id,
            base_no: field_i64(v, "baseNo").unwrap_or(0),
            folder_name_id: field_hash(v, "folderNameId"),
            thumbnail_tex_name: field_hash(v, "thumbnailTexName"),
            thumbnail_tex_file_name: owned(field_str(v, "thumbnailTexFileName").unwrap_or("")),
            folder_item_offset: bfil
                .and_then(|a| a.first())
                .and_then(Value::as_i64)
                .unwrap_or(0),
            folder_item_count: bfil
                .and_then(|a| a.get(1))
                .and_then(Value::as_i64)
                .unwrap_or(0),
        })
    }
}

// ─── InfoBookmarkConfig ────────────────────────────────────────────────────────

/// Contenu complet d'un `info_bookmark_config_*.cfg.bin.json` (3 listes).
///
/// Utiliser [`parse_info_bookmark_config`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InfoBookmarkConfig {
    /// `m_TexBasePathList` — 1 chemin de base texture.
    pub tex_base_paths: Vec<TexBasePath>,
    /// `m_BookmarkFolderItemList` — 55 dossiers de marque-pages.
    pub folder_items: Vec<BookmarkFolderItem>,
    /// `m_InfoBookmarkDataList` — 11 groupes de marque-pages.
    pub bookmarks: Vec<InfoBookmarkData>,
}

impl InfoBookmarkConfig {
    /// Renvoie la tranche de [`BookmarkFolderItem`] appartenant à un groupe.
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    #[must_use]
    pub fn folder_items_of(&self, data: &InfoBookmarkData) -> &[BookmarkFolderItem] {
        let start = data.folder_item_offset as usize;
        let end = start.saturating_add(data.folder_item_count as usize);
        self.folder_items.get(start..end).unwrap_or(&[])
    }

    /// Recherche un [`BookmarkFolderItem`] par `search_word_id`. `None` si absent.
    #[must_use]
    pub fn find_folder_item(&self, id: HashId) -> Option<&BookmarkFolderItem> {
        self.folder_items.iter().find(|i| i.search_word_id == id)
    }
}

// ─── Parseurs principaux ───────────────────────────────────────────────────────

/// Parse les `SEARCH_WORD_INFO_*` d'un `search_word_config.cfg.bin.json`.
///
/// Renvoie la liste des 55 [`SearchWordInfo`] valides (format `entries`).
/// Le nœud conteneur `SEARCH_WORD_INFO_LIST_BEG_0` est ignoré.
///
/// Vérité terrain : fichier réel → 55 entrées.
/// `SEARCH_WORD_INFO_0` : var\[0\]=65287569 → search_word_id=0x03E43591.
#[must_use]
pub fn parse_search_word_config(root: &Value) -> Vec<SearchWordInfo> {
    let mut out = Vec::new();
    walk_named(root, "SEARCH_WORD_INFO_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(info) = SearchWordInfo::from_node(node) {
            out.push(info);
        }
    });
    out
}

/// Parse un `info_bookmark_config_*.cfg.bin.json` complet (3 listes).
///
/// Renvoie un [`InfoBookmarkConfig`] avec toutes les listes remplies.
/// Les entrées invalides (hash nul) sont silencieusement ignorées.
///
/// Vérité terrain : `info_bookmark_config_0.00.00.cfg.bin.json` →
/// 1 `TexBasePath`, 55 `BookmarkFolderItem`, 11 `InfoBookmarkData`.
#[must_use]
pub fn parse_info_bookmark_config(root: &Value) -> InfoBookmarkConfig {
    InfoBookmarkConfig {
        tex_base_paths: extract_list(root, "m_TexBasePathList", TexBasePath::from_value),
        folder_items: extract_list(
            root,
            "m_BookmarkFolderItemList",
            BookmarkFolderItem::from_value,
        ),
        bookmarks: extract_list(root, "m_InfoBookmarkDataList", InfoBookmarkData::from_value),
    }
}

/// Parcourt une liste nommée du JSON et applique `f` à chaque entrée.
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
