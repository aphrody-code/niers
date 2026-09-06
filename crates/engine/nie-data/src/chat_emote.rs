//! `ChatEmote` — port Rust de `chat_emote_config_*.cfg.bin.json` et
//! `chat_emote_def_set_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! Ces deux fichiers décrivent les emotes/stamps de chat (la « roue » d'emotes
//! échangeables en multijoueur) et leur regroupement par page/catégorie.
//!
//! ## Vérité terrain
//!
//! Dumps réels (dossier `data/common/gamedata/chat_emote/`) :
//! - `chat_emote_config_{0.00.00,1.02.03.00,1.03.17.00}.cfg.bin.json`
//!   → liste `m_ChatEmoteInfoList`, type `CHAT_EMOTE_CONFIG`.
//! - `chat_emote_def_set_config_{0.00.00,1.02.03.00,1.03.17.00}.cfg.bin.json`
//!   → liste `m_ChatEmoteDefSetInfoList`, type `CHAT_EMOTE_DEF_SET_CONFIG`.
//!
//! Parser de référence inagle : `packages/inagle/src/parsers/chat-emote-config.ts`
//! (n'extrait qu'un sous-ensemble des champs ; ce port couvre **tous** les champs
//! observés sur les dumps).
//!
//! ## Évolution des versions (layout `lists`)
//!
//! `m_ChatEmoteInfoList` (compte : 8 → 57 → 57 entrées) :
//!
//! | Champ          | 0.00.00 | 1.02.03.00 | 1.03.17.00 |
//! |----------------|:-------:|:----------:|:----------:|
//! | `id`           |   oui   |    oui     |    oui     |
//! | `flag_idx`     |    —    |     —      |    oui     |
//! | `sort_id`      |   oui   |    oui     |    oui     |
//! | `type`         |   oui   |    oui     |    oui     |
//! | `texFilePath`  |    —    |     —      |    oui     |
//! | `texName`      |    —    |     —      |    oui     |
//! | `text_id`      |   oui   |    oui     |    oui     |
//! | `stamp_idx`    |    —    |    oui     |    oui     |
//! | `motion_id`    |   oui   |    oui     |    oui     |
//! | `motion_loop`  |   oui   |    oui     |    oui     |
//! | `se_id`        |   oui   |    oui     |    oui     |
//! | `se_sub_id`    |   oui   |    oui     |    oui     |
//! | `enable_cond`  |   oui   |    oui     |    oui     |
//!
//! `m_ChatEmoteDefSetInfoList` (compte : 3 → 3 → 4 entrées) :
//!
//! | Champ           | 0.00.00 | 1.02.03.00 | 1.03.17.00 |
//! |-----------------|:-------:|:----------:|:----------:|
//! | `page_idx`      |   oui   |    oui     |     —      |
//! | `type`          |    —    |     —      |    oui     |
//! | `chat_id_array` |   oui   |    oui     |    oui     |
//!
//! ## Zones d'ombre honnêtes
//!
//! - `texFilePath` et `enable_cond` valent souvent le **sentinel interne**
//!   [`SENTINEL_CONFIG`] = `U+FFFD` (caractère de remplacement) suivi de
//!   `"CHAT_EMOTE_CONFIG"` — marqueur de type du format cfg.bin, équivalent au
//!   `"\x01GALLERY_INFO"` de la galerie. Pas de chemin/condition réel·le dans ce cas.
//! - `chat_id_array` (def_set) est nommé « array » mais ne contient qu'**un seul**
//!   hash dans tous les dumps (l'identifiant du premier emote de la page).
//! - `se_id`/`se_sub_id` valent 0 partout dans les dumps observés.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_str, list_values};
use crate::hash::HashId;

/// Sentinel interne du format cfg.bin : `U+FFFD` + `"CHAT_EMOTE_CONFIG"`.
///
/// Apparaît dans `texFilePath`/`enable_cond` quand aucune valeur réelle n'est
/// présente (l'outil de dump réinjecte le `typeName` derrière le caractère de
/// remplacement). À traiter comme « absence de valeur ».
pub const SENTINEL_CONFIG: &str = "\u{FFFD}CHAT_EMOTE_CONFIG";

// ─── ChatEmoteInfo ──────────────────────────────────────────────────────────

/// Entrée `CHAT_EMOTE_CONFIG` — un emote/stamp de chat.
///
/// Vérité terrain : `m_ChatEmoteInfoList`. Les champs absents des versions
/// anciennes (cf. tableau du module) sont parsés à leur valeur par défaut.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChatEmoteInfo {
    /// `id` — identifiant hash de l'emote (ex. `0x6AE23243`).
    pub id: HashId,
    /// `flag_idx` — index de flag de déblocage (1.03.17.00 ; 0 avant).
    pub flag_idx: i64,
    /// `sort_id` — clé de tri dans la roue d'emotes (0,1,… puis 100+ par catégorie).
    pub sort_id: i64,
    /// `type` — catégorie d'emote (observé : 0..3).
    pub type_: i64,
    /// `texFilePath` — chemin de la texture du stamp
    /// (ex. `"#/menu/220_img/stamp_img/<LG>/emote_img01_01.g4tx"`),
    /// ou [`SENTINEL_CONFIG`] si aucune texture. Absent avant 1.03.17.00 (`""`).
    pub tex_file_path: String,
    /// `texName` — hash du nom de texture (`0x00000000` si aucun). Absent avant 1.03.17.00.
    pub tex_name: HashId,
    /// `text_id` — hash du texte affiché (`0x00000000` pour les stamps purement visuels).
    pub text_id: HashId,
    /// `stamp_idx` — index du stamp dans sa texture (0..8 observés). Absent en 0.00.00.
    pub stamp_idx: i64,
    /// `motion_id` — hash de la motion jouée (`0x00000000` si aucune).
    pub motion_id: HashId,
    /// `motion_loop` — la motion boucle-t-elle (faux partout dans les dumps).
    pub motion_loop: bool,
    /// `se_id` — identifiant d'effet sonore (0 partout dans les dumps).
    pub se_id: i64,
    /// `se_sub_id` — sous-identifiant d'effet sonore (0 partout dans les dumps).
    pub se_sub_id: i64,
    /// `enable_cond` — condition d'activation : [`SENTINEL_CONFIG`] (aucune) ou
    /// chaîne opaque le cas échéant.
    pub enable_cond: String,
}

impl ChatEmoteInfo {
    /// Parse une entrée de `m_ChatEmoteInfoList`. `None` si `id` est nul/absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            flag_idx: field_i64(v, "flag_idx").unwrap_or(0),
            sort_id: field_i64(v, "sort_id").unwrap_or(0),
            type_: field_i64(v, "type").unwrap_or(0),
            tex_file_path: field_str(v, "texFilePath").unwrap_or("").into(),
            tex_name: field_hash(v, "texName"),
            text_id: field_hash(v, "text_id"),
            stamp_idx: field_i64(v, "stamp_idx").unwrap_or(0),
            motion_id: field_hash(v, "motion_id"),
            motion_loop: field_bool(v, "motion_loop").unwrap_or(false),
            se_id: field_i64(v, "se_id").unwrap_or(0),
            se_sub_id: field_i64(v, "se_sub_id").unwrap_or(0),
            enable_cond: field_str(v, "enable_cond").unwrap_or("").into(),
        })
    }

    /// `true` si `tex_file_path` est le sentinel interne (pas de texture réelle).
    #[must_use]
    pub fn has_real_texture(&self) -> bool {
        !self.tex_file_path.is_empty() && self.tex_file_path != SENTINEL_CONFIG
    }
}

// ─── ChatEmoteDefSet ──────────────────────────────────────────────────────────

/// Entrée `CHAT_EMOTE_DEF_SET_CONFIG` — regroupement (page/catégorie) d'emotes.
///
/// Vérité terrain : `m_ChatEmoteDefSetInfoList`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChatEmoteDefSet {
    /// `page_idx` — index de page (1,2,3 en 0.00.00/1.02.03.00 ; absent → 0 en 1.03.17.00).
    pub page_idx: i64,
    /// `type` — type/catégorie de jeu d'emotes (0..3 en 1.03.17.00 ; absent → 0 avant).
    pub set_type: i64,
    /// `chat_id_array` — hash de l'emote de tête du jeu (un seul hash malgré le nom « array »).
    pub chat_id_array: HashId,
}

impl ChatEmoteDefSet {
    /// Parse une entrée de `m_ChatEmoteDefSetInfoList`. `None` si `chat_id_array` est nul/absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let chat_id_array = field_hash(v, "chat_id_array");
        if chat_id_array.is_zero() {
            return None;
        }
        Some(Self {
            page_idx: field_i64(v, "page_idx").unwrap_or(0),
            set_type: field_i64(v, "type").unwrap_or(0),
            chat_id_array,
        })
    }
}

// ─── ChatEmoteConfig ──────────────────────────────────────────────────────────

/// Contenu complet d'un `chat_emote_config_*.cfg.bin.json`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChatEmoteConfig {
    /// Entrées `m_ChatEmoteInfoList` (8 en 0.00.00, 57 en 1.02.03.00 et 1.03.17.00).
    pub entries: Vec<ChatEmoteInfo>,
}

impl ChatEmoteConfig {
    /// Recherche une entrée par son `id`. `None` si absente.
    #[must_use]
    pub fn find_by_id(&self, id: HashId) -> Option<&ChatEmoteInfo> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Recherche une entrée par son `sort_id`. `None` si absente.
    #[must_use]
    pub fn find_by_sort_id(&self, sort_id: i64) -> Option<&ChatEmoteInfo> {
        self.entries.iter().find(|e| e.sort_id == sort_id)
    }
}

/// Contenu complet d'un `chat_emote_def_set_config_*.cfg.bin.json`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChatEmoteDefSetConfig {
    /// Entrées `m_ChatEmoteDefSetInfoList` (3 en 0.00.00/1.02.03.00, 4 en 1.03.17.00).
    pub entries: Vec<ChatEmoteDefSet>,
}

impl ChatEmoteDefSetConfig {
    /// Recherche un jeu par son `chat_id_array`. `None` si absent.
    #[must_use]
    pub fn find_by_chat_id(&self, chat_id: HashId) -> Option<&ChatEmoteDefSet> {
        self.entries.iter().find(|e| e.chat_id_array == chat_id)
    }
}

// ─── Parseurs principaux ──────────────────────────────────────────────────────

/// Parse un `chat_emote_config_*.cfg.bin.json` (liste `m_ChatEmoteInfoList`).
///
/// Les entrées invalides (`id` nul) sont silencieusement ignorées.
#[must_use]
pub fn parse_chat_emote_config(root: &Value) -> ChatEmoteConfig {
    let entries = list_values(root, "m_ChatEmoteInfoList")
        .map(|values| {
            values
                .iter()
                .filter_map(ChatEmoteInfo::from_value)
                .collect()
        })
        .unwrap_or_default();
    ChatEmoteConfig { entries }
}

/// Parse un `chat_emote_def_set_config_*.cfg.bin.json` (liste `m_ChatEmoteDefSetInfoList`).
///
/// Les entrées invalides (`chat_id_array` nul) sont silencieusement ignorées.
#[must_use]
pub fn parse_chat_emote_def_set_config(root: &Value) -> ChatEmoteDefSetConfig {
    let entries = list_values(root, "m_ChatEmoteDefSetInfoList")
        .map(|values| {
            values
                .iter()
                .filter_map(ChatEmoteDefSet::from_value)
                .collect()
        })
        .unwrap_or_default();
    ChatEmoteDefSetConfig { entries }
}
