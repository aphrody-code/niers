//! `DictionaryConfig` — port Rust de `dictionary_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `/home/ubuntu/niers/data/common/gamedata/dictionary/dictionary_config_0.00.00.cfg.bin.json`
//! - 6 listes dans ce fichier :
//!   - `m_HabitatList` — 43 zones d'habitat (lieux où apparaissent les personnages en mode
//!     encyclopédie, `DICTIONARY_HABITAT_DATA`).
//!   - `m_ObservationDataList` — 280 références observation (association personnage→type
//!     d'action, `DICTIONARY_OBSERVATION_REF_DATA`).
//!   - `m_ParamList` — 280 entrées principales de l'encyclopédie des personnages
//!     (`DICTIONARY_PARAM_DATA`) ; chaque entrée référence une tranche de `m_ObservationDataList`
//!     via `observation_data = [offset, count]`.
//!   - `m_ObservationActionIdList` — 178 identifiants d'action d'observation
//!     (`DICTIONARY_OBSERVATION_ACTIONID_REF_DATA`).
//!   - `m_ObservationActionPlayList` — 94 playlists d'actions (`DICTIONARY_OBSERVATION_ACTIONPLAY_REF_DATA`) ;
//!     chaque entrée référence une tranche de `m_ObservationActionIdList` via `acttion_list = [offset, count]`
//!     (typo Level-5 — double « t »).
//!   - `m_ObservationActionDataList` — 28 types d'action d'observation
//!     (`DICTIONARY_OBSERVATION_ACTION_DATA`) ; chaque entrée référence une tranche de
//!     `m_ObservationActionPlayList` via `playlist = [offset, count]`.
//!
//! ## Résolution des tranches
//!
//! Les trois niveaux d'indirection se lisent de bas en haut :
//! 1. `DictionaryParamData.observation_offset/count` → tranche de `DictionaryObservationRefData`
//!    (via [`DictionaryConfig::observation_refs_of`]).
//! 2. `DictionaryObservationActionData.play_offset/count` → tranche de `DictionaryObservationActionPlay`
//!    (via [`DictionaryConfig::plays_of`]).
//! 3. `DictionaryObservationActionPlay.action_offset/count` → tranche de `DictionaryObservationActionId`
//!    (via [`DictionaryConfig::action_ids_of`]).

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_str, list_values, owned};
use crate::hash::HashId;

// ─── DICTIONARY_HABITAT_DATA ──────────────────────────────────────────────────

/// Zone d'habitat d'un personnage de l'encyclopédie (`DICTIONARY_HABITAT_DATA`).
///
/// Définit la carte et le nom affiché pour la zone d'apparition du personnage.
///
/// Dump réel :
/// `/home/ubuntu/niers/data/common/gamedata/dictionary/dictionary_config_0.00.00.cfg.bin.json`,
/// liste `m_HabitatList` (43 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DictionaryHabitatData {
    /// `habitatID` — identifiant unique de la zone d'habitat.
    pub habitat_id: HashId,
    /// `mapID` — identifiant de la carte associée (nul si global).
    pub map_id: HashId,
    /// `mapNameID` — hash du nom de la carte (jointure texte).
    pub map_name_id: HashId,
    /// `fileName` — nom du fichier de ressource (vide dans le dump réel).
    pub file_name: String,
    /// `textureNameCrc` — CRC de texture (nul dans le dump réel).
    pub texture_name_crc: HashId,
    /// `isShowAreaTexture` — afficher la texture de zone sur la carte.
    pub is_show_area_texture: bool,
}

impl DictionaryHabitatData {
    /// Parse une entrée de `m_HabitatList`. `None` si `habitatID` est absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let habitat_id = field_hash(v, "habitatID");
        Some(Self {
            habitat_id,
            map_id: field_hash(v, "mapID"),
            map_name_id: field_hash(v, "mapNameID"),
            file_name: owned(field_str(v, "fileName").unwrap_or("")),
            texture_name_crc: field_hash(v, "textureNameCrc"),
            is_show_area_texture: field_bool(v, "isShowAreaTexture").unwrap_or(false),
        })
    }
}

// ─── DICTIONARY_OBSERVATION_REF_DATA ─────────────────────────────────────────

/// Référence observation personnage→type d'action (`DICTIONARY_OBSERVATION_REF_DATA`).
///
/// Associe un personnage (ou zéro pour « générique ») à un type d'action d'observation.
/// Résoudre via [`DictionaryConfig::observation_refs_of`] depuis un `DictionaryParamData`.
///
/// Dump réel : liste `m_ObservationDataList` (280 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DictionaryObservationRefData {
    /// `charaid` — hash du personnage concerné (nul = comportement par défaut).
    pub chara_id: HashId,
    /// `actiontype_id` — hash du type d'action d'observation.
    pub actiontype_id: HashId,
}

impl DictionaryObservationRefData {
    /// Parse une entrée de `m_ObservationDataList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            chara_id: field_hash(v, "charaid"),
            actiontype_id: field_hash(v, "actiontype_id"),
        })
    }
}

// ─── DICTIONARY_PARAM_DATA ────────────────────────────────────────────────────

/// Entrée principale de l'encyclopédie d'un personnage (`DICTIONARY_PARAM_DATA`).
///
/// Chaque entrée correspond à une fiche de personnage dans l'encyclopédie (280 fiches au
/// total dans le dump réel). La tranche d'observations est résolue via
/// [`DictionaryConfig::observation_refs_of`].
///
/// Le champ `cond3` est une chaîne base64 dont le décodage binaire n'est pas encore documenté ;
/// il est stocké tel quel.
///
/// Dump réel : liste `m_ParamList` (280 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DictionaryParamData {
    /// `charaID` — hash du personnage (clé principale de la fiche).
    pub chara_id: HashId,
    /// `medalID` — hash de la médaille associée (nul si aucune).
    pub medal_id: HashId,
    /// `weaponItemID` — hash de l'arme associée (nul si aucune).
    pub weapon_item_id: HashId,
    /// `habitatID` — hash de la zone d'habitat (jointure avec `m_HabitatList`, nul si global).
    pub habitat_id: HashId,
    /// `descTextID` — hash du texte de description (jointure texte).
    pub desc_text_id: HashId,
    /// `viewDictNo` — numéro d'affichage dans l'encyclopédie (1-basé, séquentiel).
    pub view_dict_no: i64,
    /// `viewType` — type d'affichage (1 = normal, 2 = spécial, 4 = autre ; valeurs observées).
    pub view_type: i64,
    /// `isButtle` — personnage de combat uniquement (toujours `false` dans le dump réel).
    pub is_buttle: bool,
    /// `category` — catégorie encyclopédie (1–6 dans le dump réel).
    pub category: i64,
    /// `sub_category` — sous-catégorie (0 ou 1 dans le dump réel).
    pub sub_category: i64,
    /// `cond2` — condition secondaire (vide dans le dump réel).
    pub cond2: String,
    /// `cond3` — blob encodé en base64 (sémantique binaire non documentée).
    pub cond3: String,
    /// `observation_data[0]` — index de début dans `m_ObservationDataList`.
    pub observation_offset: i64,
    /// `observation_data[1]` — nombre d'entrées dans `m_ObservationDataList`.
    pub observation_count: i64,
}

impl DictionaryParamData {
    /// Parse une entrée de `m_ParamList`. `None` si `charaID` est absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let obs = v.get("observation_data").and_then(Value::as_array);
        Some(Self {
            chara_id: field_hash(v, "charaID"),
            medal_id: field_hash(v, "medalID"),
            weapon_item_id: field_hash(v, "weaponItemID"),
            habitat_id: field_hash(v, "habitatID"),
            desc_text_id: field_hash(v, "descTextID"),
            view_dict_no: field_i64(v, "viewDictNo").unwrap_or(0),
            view_type: field_i64(v, "viewType").unwrap_or(0),
            is_buttle: field_bool(v, "isButtle").unwrap_or(false),
            category: field_i64(v, "category").unwrap_or(0),
            sub_category: field_i64(v, "sub_category").unwrap_or(0),
            cond2: owned(field_str(v, "cond2").unwrap_or("")),
            cond3: owned(field_str(v, "cond3").unwrap_or("")),
            observation_offset: obs
                .and_then(|a| a.first())
                .and_then(Value::as_i64)
                .unwrap_or(0),
            observation_count: obs
                .and_then(|a| a.get(1))
                .and_then(Value::as_i64)
                .unwrap_or(0),
        })
    }
}

// ─── DICTIONARY_OBSERVATION_ACTIONID_REF_DATA ─────────────────────────────────

/// Identifiant d'action d'observation (`DICTIONARY_OBSERVATION_ACTIONID_REF_DATA`).
///
/// Référencé via une tranche depuis `DictionaryObservationActionPlay`
/// (résoudre avec [`DictionaryConfig::action_ids_of`]).
///
/// Dump réel : liste `m_ObservationActionIdList` (178 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DictionaryObservationActionId {
    /// `action_id` — hash de l'action à jouer.
    pub action_id: HashId,
    /// `emotion_id` — hash de l'émotion associée (nul si aucune).
    pub emotion_id: HashId,
    /// `disable_repeat_se` — désactiver le son si l'action se répète.
    pub disable_repeat_se: bool,
}

impl DictionaryObservationActionId {
    /// Parse une entrée de `m_ObservationActionIdList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            action_id: field_hash(v, "action_id"),
            emotion_id: field_hash(v, "emotion_id"),
            disable_repeat_se: field_bool(v, "disable_repeat_se").unwrap_or(false),
        })
    }
}

// ─── DICTIONARY_OBSERVATION_ACTIONPLAY_REF_DATA ───────────────────────────────

/// Playlist d'actions pour un numéro de lecture (`DICTIONARY_OBSERVATION_ACTIONPLAY_REF_DATA`).
///
/// Référencée depuis `DictionaryObservationActionData.play_offset/count`
/// (résoudre avec [`DictionaryConfig::plays_of`]). La tranche vers
/// `m_ObservationActionIdList` est résolue avec [`DictionaryConfig::action_ids_of`].
///
/// > **Note** : le champ source s'appelle `acttion_list` (typo Level-5 — double « t »),
/// > conservé dans les champs `action_offset`/`action_count` via le JSON brut.
///
/// Dump réel : liste `m_ObservationActionPlayList` (94 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DictionaryObservationActionPlay {
    /// `play_no` — numéro de lecture (n'est pas forcément l'index dans la liste).
    pub play_no: i64,
    /// `acttion_list[0]` — index de début dans `m_ObservationActionIdList`.
    pub action_offset: i64,
    /// `acttion_list[1]` — nombre d'entrées dans `m_ObservationActionIdList`.
    pub action_count: i64,
}

impl DictionaryObservationActionPlay {
    /// Parse une entrée de `m_ObservationActionPlayList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let al = v.get("acttion_list").and_then(Value::as_array);
        Some(Self {
            play_no: field_i64(v, "play_no").unwrap_or(0),
            action_offset: al
                .and_then(|a| a.first())
                .and_then(Value::as_i64)
                .unwrap_or(0),
            action_count: al
                .and_then(|a| a.get(1))
                .and_then(Value::as_i64)
                .unwrap_or(0),
        })
    }
}

// ─── DICTIONARY_OBSERVATION_ACTION_DATA ───────────────────────────────────────

/// Type d'action d'observation avec sa playlist (`DICTIONARY_OBSERVATION_ACTION_DATA`).
///
/// Chaque entrée associe un `actiontype_id` à une plage de `DictionaryObservationActionPlay`
/// (résoudre avec [`DictionaryConfig::plays_of`]).
///
/// Dump réel : liste `m_ObservationActionDataList` (28 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DictionaryObservationActionData {
    /// `actiontype_id` — hash du type d'action d'observation.
    pub actiontype_id: HashId,
    /// `playlist[0]` — index de début dans `m_ObservationActionPlayList`.
    pub play_offset: i64,
    /// `playlist[1]` — nombre d'entrées dans `m_ObservationActionPlayList`.
    pub play_count: i64,
}

impl DictionaryObservationActionData {
    /// Parse une entrée de `m_ObservationActionDataList`. `None` si `actiontype_id` est absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let pl = v.get("playlist").and_then(Value::as_array);
        Some(Self {
            actiontype_id: field_hash(v, "actiontype_id"),
            play_offset: pl
                .and_then(|a| a.first())
                .and_then(Value::as_i64)
                .unwrap_or(0),
            play_count: pl
                .and_then(|a| a.get(1))
                .and_then(Value::as_i64)
                .unwrap_or(0),
        })
    }
}

// ─── DictionaryConfig ─────────────────────────────────────────────────────────

/// Contenu complet d'un `dictionary_config_*.cfg.bin.json`.
///
/// Utiliser [`parse_dictionary_config`] pour construire depuis un JSON déjà désérialisé.
/// Les indirections par tranche se résolvent via les accesseurs de cette struct.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DictionaryConfig {
    /// 43 zones d'habitat (`m_HabitatList`).
    pub habitats: Vec<DictionaryHabitatData>,
    /// 280 références observation personnage→type d'action (`m_ObservationDataList`).
    pub observation_refs: Vec<DictionaryObservationRefData>,
    /// 280 fiches encyclopédie personnage (`m_ParamList`).
    pub params: Vec<DictionaryParamData>,
    /// 178 identifiants d'action (`m_ObservationActionIdList`).
    pub observation_action_ids: Vec<DictionaryObservationActionId>,
    /// 94 playlists d'actions (`m_ObservationActionPlayList`).
    pub observation_action_plays: Vec<DictionaryObservationActionPlay>,
    /// 28 types d'action d'observation (`m_ObservationActionDataList`).
    pub observation_action_data: Vec<DictionaryObservationActionData>,
}

impl DictionaryConfig {
    /// Renvoie la tranche de références observation d'une fiche encyclopédie.
    ///
    /// Retourne `&[]` si la tranche dépasse la taille de `observation_refs`.
    #[must_use]
    pub fn observation_refs_of(
        &self,
        param: &DictionaryParamData,
    ) -> &[DictionaryObservationRefData] {
        let start = param.observation_offset as usize;
        let end = start.saturating_add(param.observation_count as usize);
        self.observation_refs.get(start..end).unwrap_or(&[])
    }

    /// Renvoie la tranche de playlists d'un type d'action d'observation.
    ///
    /// Retourne `&[]` si la tranche dépasse la taille de `observation_action_plays`.
    #[must_use]
    pub fn plays_of(
        &self,
        action: &DictionaryObservationActionData,
    ) -> &[DictionaryObservationActionPlay] {
        let start = action.play_offset as usize;
        let end = start.saturating_add(action.play_count as usize);
        self.observation_action_plays.get(start..end).unwrap_or(&[])
    }

    /// Renvoie la tranche d'identifiants d'action d'une playlist.
    ///
    /// Retourne `&[]` si la tranche dépasse la taille de `observation_action_ids`.
    #[must_use]
    pub fn action_ids_of(
        &self,
        play: &DictionaryObservationActionPlay,
    ) -> &[DictionaryObservationActionId] {
        let start = play.action_offset as usize;
        let end = start.saturating_add(play.action_count as usize);
        self.observation_action_ids.get(start..end).unwrap_or(&[])
    }

    /// Recherche une fiche encyclopédie par `charaID`. `None` si absente.
    #[must_use]
    pub fn find_param_by_chara(&self, chara_id: HashId) -> Option<&DictionaryParamData> {
        self.params.iter().find(|p| p.chara_id == chara_id)
    }

    /// Recherche un habitat par `habitatID`. `None` si absent.
    #[must_use]
    pub fn find_habitat(&self, habitat_id: HashId) -> Option<&DictionaryHabitatData> {
        self.habitats.iter().find(|h| h.habitat_id == habitat_id)
    }
}

// ─── Parseur principal ────────────────────────────────────────────────────────

/// Parse un `dictionary_config_*.cfg.bin.json` complet (6 listes).
///
/// Renvoie un [`DictionaryConfig`] avec toutes les listes remplies. Les entrées invalides
/// sont silencieusement ignorées.
///
/// # Exemple
///
/// ```
/// use nie_data::dictionary::parse_dictionary_config;
/// use serde_json::json;
///
/// let root = json!({
///     "version": 100,
///     "lists": [
///         { "name": "m_HabitatList",              "typeName": "DICTIONARY_HABITAT_DATA",                    "values": [] },
///         { "name": "m_ObservationDataList",       "typeName": "DICTIONARY_OBSERVATION_REF_DATA",            "values": [] },
///         { "name": "m_ParamList",                 "typeName": "DICTIONARY_PARAM_DATA",                      "values": [] },
///         { "name": "m_ObservationActionIdList",   "typeName": "DICTIONARY_OBSERVATION_ACTIONID_REF_DATA",   "values": [] },
///         { "name": "m_ObservationActionPlayList", "typeName": "DICTIONARY_OBSERVATION_ACTIONPLAY_REF_DATA", "values": [] },
///         { "name": "m_ObservationActionDataList", "typeName": "DICTIONARY_OBSERVATION_ACTION_DATA",         "values": [] }
///     ]
/// });
/// let cfg = parse_dictionary_config(&root);
/// assert_eq!(cfg.habitats.len(), 0);
/// assert_eq!(cfg.params.len(), 0);
/// ```
#[must_use]
pub fn parse_dictionary_config(root: &Value) -> DictionaryConfig {
    DictionaryConfig {
        habitats: extract_list(root, "m_HabitatList", DictionaryHabitatData::from_value),
        observation_refs: extract_list(
            root,
            "m_ObservationDataList",
            DictionaryObservationRefData::from_value,
        ),
        params: extract_list(root, "m_ParamList", DictionaryParamData::from_value),
        observation_action_ids: extract_list(
            root,
            "m_ObservationActionIdList",
            DictionaryObservationActionId::from_value,
        ),
        observation_action_plays: extract_list(
            root,
            "m_ObservationActionPlayList",
            DictionaryObservationActionPlay::from_value,
        ),
        observation_action_data: extract_list(
            root,
            "m_ObservationActionDataList",
            DictionaryObservationActionData::from_value,
        ),
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
