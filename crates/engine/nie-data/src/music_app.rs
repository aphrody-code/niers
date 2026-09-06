//! `MusicAppConfig` — port Rust de `music_app/music_app_config.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `/home/ubuntu/niers/data/common/gamedata/music_app/music_app_config.cfg.bin.json`
//! - Format **`entries`** (noeuds nommés, variables positionnelles).
//! - Structure à 3 niveaux :
//!   - `MUSIC_APP_INFO_LIST_BEG_0` — conteneur racine (var\[0\] = 4 = nombre de groupes).
//!   - `MUSIC_APP_INFO_N` — groupe par catégorie d'application (var\[0\] = `app_id` ∈ 1..=4).
//!   - `MUSIC_APP_INFO_ITEM_M` — piste musicale individuelle (9 variables positionnelles).
//! - 108 items au total répartis en 4 groupes :
//!   - groupe 1 (`app_id=1`) : 20 items (index de tri 1–20)
//!   - groupe 2 (`app_id=2`) : 32 items (index 21–52)
//!   - groupe 3 (`app_id=3`) : 50 items (index 53–102)
//!   - groupe 4 (`app_id=4`) : 6 items  (index 103–108)
//!
//! ## Positions des variables de `MUSIC_APP_INFO_ITEM_*`
//!
//! | Position | Type JSON | Sémantique observée |
//! |----------|-----------|---------------------|
//! | var\[0\] | Int | `app_category` — catégorie (même valeur que `app_id` du groupe parent) |
//! | var\[1\] | Int | `music_id` — hash de la piste musicale |
//! | var\[2\] | Int | `track_no` — numéro de piste dans le conteneur audio |
//! | var\[3\] | Int | `variant` — variante de piste (0 = principale, 1 = variante) |
//! | var\[4\] | Int | `entry_id` — hash identifiant l'entrée dans la liste musicale |
//! | var\[5\] | String\|Int | `path_b64` — chemin audio encodé base64 (absent = Int "0") |
//! | var\[6\] | Int | `volume` — volume de lecture (toujours 500 dans le dump réel) |
//! | var\[7\] | Int | `sort_index` — ordre de tri séquentiel 1-basé (global, toutes catégories) |
//! | var\[8\] | Int | doublon de var\[7\] — ignoré |
//!
//! ## Note sur `path_b64`
//!
//! 105 des 108 items ont `var[5]` de type `String` (base64 encodant le chemin ADX).
//! Les 3 items sans chemin (`MUSIC_APP_INFO_ITEM_20`, `_21`, `_52`) ont `var[5]` de type
//! `Int` avec valeur `"0"` — représentés ici par une chaîne vide.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

// ─── MusicAppItem ─────────────────────────────────────────────────────────────

/// Piste musicale individuelle (`MUSIC_APP_INFO_ITEM_*`).
///
/// Les variables sont positionnelles (format `entries` cfgbin). Consulter la documentation
/// du module pour le tableau complet des positions.
///
/// # Exemple de vérité terrain
///
/// `MUSIC_APP_INFO_ITEM_0` (`music_app_config.cfg.bin.json`, groupe 0, premier item) :
/// `[1, 1653231064, 2, 0, 1169637250, "AAAAAA8FNbkZNtoAAQAyAAAnTHE=", 500, 1, 1]`
/// - `app_category = 1`, `music_id = 0x628A4DD8`, `track_no = 2`, `variant = 0`,
///   `entry_id = 0x45B73F82`, `volume = 500`, `sort_index = 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MusicAppItem {
    /// var\[0\] — catégorie de l'application (1–4, identique au `app_id` du groupe parent).
    ///
    /// Vérité terrain : `MUSIC_APP_INFO_ITEM_0.var[0] = 1`.
    pub app_category: i64,

    /// var\[1\] — hash identifiant la piste musicale.
    ///
    /// Vérité terrain : `MUSIC_APP_INFO_ITEM_0.var[1] = 1653231064` → `0x628A4DD8`.
    pub music_id: HashId,

    /// var\[2\] — numéro de piste dans le fichier audio (0-basé dans les groupes 2–4,
    /// valeurs variées dans le groupe 1 : 0, 2, 6, 9…).
    ///
    /// Vérité terrain : `MUSIC_APP_INFO_ITEM_0.var[2] = 2`.
    pub track_no: i64,

    /// var\[3\] — variante de piste (0 = principale, 1 = variante).
    ///
    /// Vérité terrain : `MUSIC_APP_INFO_ITEM_21.var[3] = 1` (seul item à 1 dans le dump réel).
    pub variant: i64,

    /// var\[4\] — hash identifiant l'entrée dans la liste musicale du jeu.
    ///
    /// Vérité terrain : `MUSIC_APP_INFO_ITEM_0.var[4] = 1169637250` → `0x45B73F82`.
    pub entry_id: HashId,

    /// var\[5\] — chemin audio encodé en base64 (chaîne vide si absent).
    ///
    /// Quand présent (105/108 items), la valeur est une chaîne base64 encodant le chemin
    /// du fichier ADX dans les CPK du jeu. Les 3 items sans chemin ont `var[5]` de type
    /// `Int` avec valeur `"0"` dans le JSON — stockés ici comme chaîne vide.
    ///
    /// Vérité terrain : `MUSIC_APP_INFO_ITEM_0.var[5] = "AAAAAA8FNbkZNtoAAQAyAAAnTHE="`.
    pub path_b64: String,

    /// var\[6\] — volume de lecture.
    ///
    /// Valeurs observées : 500 (59 items), 2000 (24 items), 5000 (19 items), 30000 (6 items).
    ///
    /// Vérité terrain : `MUSIC_APP_INFO_ITEM_0.var[6] = 500`.
    pub volume: i64,

    /// var\[7\] — index de tri séquentiel 1-basé (global sur les 108 items).
    ///
    /// Vérité terrain : `MUSIC_APP_INFO_ITEM_0.var[7] = 1`, `MUSIC_APP_INFO_ITEM_107.var[7] = 108`.
    /// var\[8\] est le doublon exact de var\[7\] — ignoré.
    pub sort_index: i64,
}

impl MusicAppItem {
    /// Parse un noeud `MUSIC_APP_INFO_ITEM_*`. Renvoie `None` si `entry_id` est nul.
    ///
    /// `var[5]` est traité spécialement : si son type JSON est `"String"`, la valeur est
    /// conservée telle quelle (base64). Si le type est `"Int"` (valeur `"0"`), le chemin
    /// est absent et la chaîne est vide.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let entry_id = node.hash(4);
        if entry_id.is_zero() {
            return None;
        }
        // var[5] : String → chemin base64 présent ; Int "0" → absent.
        let path_b64 = match node.var(5) {
            Some(var) if var.ty == "String" => String::from(var.value),
            _ => String::new(),
        };
        Some(Self {
            app_category: node.int(0),
            music_id: node.hash(1),
            track_no: node.int(2),
            variant: node.int(3),
            entry_id,
            path_b64,
            volume: node.int(6),
            sort_index: node.int(7),
        })
    }

    /// `true` si cet item possède un chemin audio (`path_b64` non vide).
    #[must_use]
    #[inline]
    pub fn has_path(&self) -> bool {
        !self.path_b64.is_empty()
    }
}

// ─── MusicAppConfig ───────────────────────────────────────────────────────────

/// Contenu complet de `music_app_config.cfg.bin.json`.
///
/// Utiliser [`parse_music_app_config`] pour construire depuis un JSON désérialisé.
///
/// La liste `items` contient les 108 pistes dans l'ordre de parcours du fichier
/// (ordre de `sort_index` croissant, 1 à 108).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MusicAppConfig {
    /// 108 pistes musicales (`MUSIC_APP_INFO_ITEM_*`), dans l'ordre du fichier.
    pub items: Vec<MusicAppItem>,
}

impl MusicAppConfig {
    /// Recherche une piste par son `entry_id`. `None` si absente.
    #[must_use]
    pub fn find_by_entry_id(&self, entry_id: HashId) -> Option<&MusicAppItem> {
        self.items.iter().find(|it| it.entry_id == entry_id)
    }

    /// Renvoie toutes les pistes d'une catégorie donnée (`app_category`).
    #[must_use]
    pub fn items_of_category(&self, app_category: i64) -> Vec<&MusicAppItem> {
        self.items
            .iter()
            .filter(|it| it.app_category == app_category)
            .collect()
    }
}

// ─── Parseur principal ────────────────────────────────────────────────────────

/// Parse un `music_app_config.cfg.bin.json` complet.
///
/// Renvoie un [`MusicAppConfig`] avec les 108 pistes dans l'ordre du fichier. Les entrées
/// dont `entry_id` est nul sont silencieusement ignorées.
///
/// Comptes réels par catégorie :
/// - `app_category=1` : 20 items (BGM matchs).
/// - `app_category=2` : 32 items.
/// - `app_category=3` : 50 items.
/// - `app_category=4` :  6 items.
#[must_use]
pub fn parse_music_app_config(root: &Value) -> MusicAppConfig {
    let mut items = Vec::new();
    walk_named(root, "MUSIC_APP_INFO_ITEM_", |node| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(item) = MusicAppItem::from_node(node) {
            items.push(item);
        }
    });
    MusicAppConfig { items }
}
