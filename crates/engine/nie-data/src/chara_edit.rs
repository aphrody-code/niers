//! `CharaEditPartsTypeConfig` — port Rust de `character/chara_edit_parts_type_config_*.cfg.bin.json`
//! (Level-5 IEVR, **éditeur d'avatar / création de personnage**).
//!
//! ## Quoi
//!
//! Le catalogue des **parties customisables** de l'éditeur d'avatar : les **types de visage** et
//! les **parts de corps (accessoires)**, chacun avec une ressource (modèle 3D) **par morphologie**
//! (les 8 *body types* du jeu : male/female/small/smallfat/tall/tallmuscle/muscle/big).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/character/chara_edit_parts_type_config_1.03.75.00.cfg.bin.json`.
//! - Format **`lists`** (champs nommés). 4 listes :
//!   - `m_CharaEditFaceTypeDataList` (42) — parts de visage par type de nez, [`CharaEditFaceTypeData`].
//!   - `m_CharaEditFaceTypeInfoList` (6) — types de modèle de visage + plage `[offset,count]` dans
//!     la data list, [`CharaEditFaceTypeInfo`].
//!   - `m_CharaEditPartsBodyTypeDataList` (24) — parts de corps (accessoires), [`CharaEditPartsBodyData`].
//!   - `m_CharaEditPartsBodyTypeInfoList` (1) — type de parts + plage, [`CharaEditPartsBodyInfo`].
//!
//! Les `*PatternID` et `*Crc` sont des hashes ; les `resource_*` sont des noms de modèle 3D
//! (`face51_nose01`, `accessory001`…). Les `*Info` portent un `[offset, count]` indexant la data list
//! associée (chaque type de visage couvre une plage de parts).

use alloc::{format, string::String, vec::Vec};
use serde_json::Value;

use crate::cfgbin::{
    field_bool, field_f64, field_hash, field_i64, field_pair, field_str, list_values,
};
use crate::hash::HashId;

/// Les 8 morphologies du jeu, dans l'ordre des suffixes de champs (`resource_<bt>`, `facePatternID_<bt>`).
pub const BODY_TYPES: [&str; 8] = [
    "male",
    "female",
    "small",
    "smallfat",
    "tall",
    "tallmuscle",
    "muscle",
    "big",
];

/// Lit un champ `String` (vide si absent), en chaîne possédée.
fn s(v: &Value, key: &str) -> String {
    String::from(field_str(v, key).unwrap_or(""))
}

/// Une part de **visage** (`m_CharaEditFaceTypeDataList`) — un type de nez, décliné par morphologie.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditFaceTypeData {
    /// `noseType` — nom du type de nez (ex. `nose_type_01`).
    pub nose_type: String,
    /// `noseTypeCrc` — hash du type de nez.
    pub nose_type_crc: HashId,
    /// `facePatternID_<bt>` — hash du pattern de visage, indexé par [`BODY_TYPES`].
    pub face_pattern_id: [HashId; 8],
    /// `resource_<bt>` — nom du modèle 3D, indexé par [`BODY_TYPES`] (ex. `face51_nose01`).
    pub resource: [String; 8],
}

impl CharaEditFaceTypeData {
    fn from_value(v: &Value) -> Self {
        Self {
            nose_type: s(v, "noseType"),
            nose_type_crc: field_hash(v, "noseTypeCrc"),
            face_pattern_id: core::array::from_fn(|i| {
                field_hash(v, &format!("facePatternID_{}", BODY_TYPES[i]))
            }),
            resource: core::array::from_fn(|i| s(v, &format!("resource_{}", BODY_TYPES[i]))),
        }
    }
}

/// Un **type de modèle de visage** (`m_CharaEditFaceTypeInfoList`) + sa plage de parts.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditFaceTypeInfo {
    /// `faceType` — nom du type de visage (ex. `face_mdl_type_01`).
    pub face_type: String,
    /// `faceTypeCrc` — hash du type.
    pub face_type_crc: HashId,
    /// `faceTypeData[0]` — offset dans `m_CharaEditFaceTypeDataList`.
    pub data_offset: i64,
    /// `faceTypeData[1]` — nombre de parts couvertes.
    pub data_count: i64,
}

impl CharaEditFaceTypeInfo {
    fn from_value(v: &Value) -> Self {
        let arr = v.get("faceTypeData").and_then(Value::as_array);
        let at = |i: usize| {
            arr.and_then(|a| a.get(i))
                .and_then(Value::as_i64)
                .unwrap_or(0)
        };
        Self {
            face_type: s(v, "faceType"),
            face_type_crc: field_hash(v, "faceTypeCrc"),
            data_offset: at(0),
            data_count: at(1),
        }
    }
}

/// Une part de **corps** (`m_CharaEditPartsBodyTypeDataList`) — un accessoire, décliné par morphologie.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPartsBodyData {
    /// `presetID` — hash du preset d'accessoire.
    pub preset_id: HashId,
    /// `resource_<bt>` — nom du modèle 3D, indexé par [`BODY_TYPES`] (ex. `accessory001`).
    pub resource: [String; 8],
}

impl CharaEditPartsBodyData {
    fn from_value(v: &Value) -> Self {
        Self {
            preset_id: field_hash(v, "presetID"),
            resource: core::array::from_fn(|i| s(v, &format!("resource_{}", BODY_TYPES[i]))),
        }
    }
}

/// Un **type de parts de corps** (`m_CharaEditPartsBodyTypeInfoList`) + sa plage.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPartsBodyInfo {
    /// `partsType` — identifiant du type de parts (entier).
    pub parts_type: i64,
    /// `partsTypeData[0]` — offset dans `m_CharaEditPartsBodyTypeDataList`.
    pub data_offset: i64,
    /// `partsTypeData[1]` — nombre de parts couvertes.
    pub data_count: i64,
}

impl CharaEditPartsBodyInfo {
    fn from_value(v: &Value) -> Self {
        let arr = v.get("partsTypeData").and_then(Value::as_array);
        let at = |i: usize| {
            arr.and_then(|a| a.get(i))
                .and_then(Value::as_i64)
                .unwrap_or(0)
        };
        Self {
            parts_type: field_i64(v, "partsType").unwrap_or(0),
            data_offset: at(0),
            data_count: at(1),
        }
    }
}

/// Config complète de l'éditeur d'avatar (4 listes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPartsTypeConfig {
    pub face_data: Vec<CharaEditFaceTypeData>,
    pub face_info: Vec<CharaEditFaceTypeInfo>,
    pub body_data: Vec<CharaEditPartsBodyData>,
    pub body_info: Vec<CharaEditPartsBodyInfo>,
}

/// Collecte une liste nommée en mappant chaque `values[]` par `f`.
fn collect_list<T>(root: &Value, name: &str, f: impl Fn(&Value) -> T) -> Vec<T> {
    list_values(root, name).map_or_else(Vec::new, |vs| vs.iter().map(f).collect())
}

/// Parse `chara_edit_parts_type_config.cfg.bin.json` → les 4 listes de l'éditeur d'avatar.
#[must_use]
pub fn parse_chara_edit_parts_type_config(root: &Value) -> CharaEditPartsTypeConfig {
    CharaEditPartsTypeConfig {
        face_data: collect_list(
            root,
            "m_CharaEditFaceTypeDataList",
            CharaEditFaceTypeData::from_value,
        ),
        face_info: collect_list(
            root,
            "m_CharaEditFaceTypeInfoList",
            CharaEditFaceTypeInfo::from_value,
        ),
        body_data: collect_list(
            root,
            "m_CharaEditPartsBodyTypeDataList",
            CharaEditPartsBodyData::from_value,
        ),
        body_info: collect_list(
            root,
            "m_CharaEditPartsBodyTypeInfoList",
            CharaEditPartsBodyInfo::from_value,
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  `chara_edit_<ver>.cfg.bin.json` — le catalogue complet de l'éditeur (16 listes)
// ─────────────────────────────────────────────────────────────────────────────
//
//  Le fichier précédent (`chara_edit_parts_type_config`) ne décrit que les **modèles de base**
//  (types de visage, accessoires) déclinés par morphologie. Celui-ci porte tout le reste :
//  les 502 **parts** sélectionnables, les 218 **curseurs** de morphing, les 470 **couleurs**,
//  et surtout les 38 **presets** (visages prédéfinis) — chacun étant une *recette* de 62 à 72
//  entrées `(recipeType, recipeNo, partsId, colorValue)`.
//
//  Vérité terrain : `data/common/gamedata/character/chara_edit_1.03.75.00.cfg.bin.json`.

/// Lit le masque `isApply<BodyType>` d'une entrée, indexé par [`BODY_TYPES`].
fn apply_mask(v: &Value) -> [bool; 8] {
    core::array::from_fn(|i| {
        let bt = BODY_TYPES[i];
        let mut key = String::from("isApply");
        let mut chars = bt.chars();
        if let Some(c) = chars.next() {
            key.extend(c.to_uppercase());
            key.push_str(chars.as_str());
        }
        field_bool(v, &key).unwrap_or(false)
    })
}

/// Lit un champ `[offset, count]`, `[0, 0]` s'il est absent.
fn pair(v: &Value, key: &str) -> (i64, i64) {
    let [o, c] = field_pair(v, key).unwrap_or([0, 0]);
    (o, c)
}

/// Une **catégorie de recette** (`m_CharaEditRecipeInfoList`, 86 entrées).
///
/// Chaque catégorie décrit un emplacement du code de partage : `bit_num` bits pour coder
/// `num` valeurs possibles. `category`/`category_param` renvoient au type de réglage éditeur.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditRecipeInfo {
    /// `id` — hash de la catégorie de recette.
    pub id: HashId,
    /// `bitNum` — largeur en bits du champ dans le code de partage.
    pub bit_num: i64,
    /// `category` — famille de réglage.
    pub category: i64,
    /// `categoryParam` — paramètre de la famille (souvent le `faceSettingType` visé).
    pub category_param: i64,
    /// `categoryParamSub` — second paramètre (sous-emplacement, ex. œil gauche/droit).
    pub category_param_sub: i64,
    /// `num` — nombre de valeurs distinctes codables.
    pub num: i64,
    /// `type` — indice de la recette dans l'ordre d'encodage (`recipeType` des presets).
    pub recipe_type: i64,
}

impl CharaEditRecipeInfo {
    fn from_value(v: &Value) -> Self {
        Self {
            id: field_hash(v, "id"),
            bit_num: field_i64(v, "bitNum").unwrap_or(0),
            category: field_i64(v, "category").unwrap_or(0),
            category_param: field_i64(v, "categoryParam").unwrap_or(0),
            category_param_sub: field_i64(v, "categoryParamSub").unwrap_or(0),
            num: field_i64(v, "num").unwrap_or(0),
            recipe_type: field_i64(v, "type").unwrap_or(0),
        }
    }
}

/// Un **caractère de l'alphabet du code de partage** (`m_CharaEditCodeInfoList`, 64 entrées).
///
/// L'avatar se partage sous forme de chaîne : la recette est sérialisée en bits (cf.
/// [`CharaEditRecipeInfo::bit_num`]) puis découpée en groupes de 6, chaque groupe indexant
/// ce tableau — un alphabet base64 propre au jeu.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditCodeInfo {
    /// `id` — hash du caractère.
    pub id: HashId,
    /// `codeChar` — le caractère affiché.
    pub code_char: String,
    /// `num` — sa valeur (0..63).
    pub num: i64,
}

impl CharaEditCodeInfo {
    fn from_value(v: &Value) -> Self {
        Self {
            id: field_hash(v, "id"),
            code_char: s(v, "codeChar"),
            num: field_i64(v, "num").unwrap_or(0),
        }
    }
}

/// Une **voix** sélectionnable (`m_CharaEditVoiceInfoList`, 96 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditVoiceInfo {
    /// `id` — hash de la voix.
    pub id: HashId,
    /// `charaSeName` — nom de la banque de sons (ex. `scoutMAA01`).
    pub chara_se_name: String,
    /// `gender` — genre auquel la voix est proposée.
    pub gender: i64,
    /// `itemNo` — rang dans la liste de l'éditeur.
    pub item_no: i64,
    /// `personality` — personnalité associée (cf. [`CharaEditPersonalityInfo`]).
    pub personality: i64,
    /// `type` — variante de ton dans la personnalité.
    pub voice_type: i64,
}

impl CharaEditVoiceInfo {
    fn from_value(v: &Value) -> Self {
        Self {
            id: field_hash(v, "id"),
            chara_se_name: s(v, "charaSeName"),
            gender: field_i64(v, "gender").unwrap_or(0),
            item_no: field_i64(v, "itemNo").unwrap_or(0),
            personality: field_i64(v, "personality").unwrap_or(0),
            voice_type: field_i64(v, "type").unwrap_or(0),
        }
    }
}

/// Une **tenue** (`m_CharaEditFashionInfoList`, 5 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditFashionInfo {
    /// `id` — index de la tenue.
    pub id: i64,
    /// `fashionNameCrc` — hash du nom de la tenue ; sert de clé dans
    /// [`CharaEditFashionBodyInfo::id`].
    pub fashion_name_crc: HashId,
}

impl CharaEditFashionInfo {
    fn from_value(v: &Value) -> Self {
        Self {
            id: field_i64(v, "id").unwrap_or(0),
            fashion_name_crc: field_hash(v, "fashionNameCrc"),
        }
    }
}

/// Une **personnalité** (`m_CharaEditPersonalityInfoList`, 24 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPersonalityInfo {
    /// `id` — hash de la personnalité.
    pub id: HashId,
    /// `performanceType` — type d'animation de présentation jouée à la sélection.
    pub performance_type: i64,
    /// `personalityType` — indice de personnalité (référencé par [`CharaEditVoiceInfo::personality`]).
    pub personality_type: i64,
    /// `viewTextId` — hash du libellé affiché.
    pub view_text_id: HashId,
}

impl CharaEditPersonalityInfo {
    fn from_value(v: &Value) -> Self {
        Self {
            id: field_hash(v, "id"),
            performance_type: field_i64(v, "performanceType").unwrap_or(0),
            personality_type: field_i64(v, "personalityType").unwrap_or(0),
            view_text_id: field_hash(v, "viewTextId"),
        }
    }
}

/// Un **fichier de preset d'avatar** (`m_CharaEditPresetFileInfoList`, 31 entrées).
///
/// Contrairement aux presets-recettes ([`CharaEditPresetInfo`]), ceux-ci désignent un modèle
/// complet déjà construit (`mdl_edit_avatar01`…).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPresetFileInfo {
    /// `id` — hash du preset (= crc32 de [`Self::id_string`]).
    pub id: HashId,
    /// `charaId` — personnage du jeu dont il reprend l'apparence (`0` si aucun).
    pub chara_id: HashId,
    /// `idString` — nom de la ressource (ex. `mdl_edit_avatar01`).
    pub id_string: String,
    /// `viewNo` — rang d'affichage.
    pub view_no: i64,
    /// `viewTextId` — hash du libellé affiché (`0` si aucun).
    pub view_text_id: HashId,
}

impl CharaEditPresetFileInfo {
    fn from_value(v: &Value) -> Self {
        Self {
            id: field_hash(v, "id"),
            chara_id: field_hash(v, "charaId"),
            id_string: s(v, "idString"),
            view_no: field_i64(v, "viewNo").unwrap_or(0),
            view_text_id: field_hash(v, "viewTextId"),
        }
    }
}

/// Une **part sélectionnable** (`m_CharaEditPartsDataList`, 502 entrées).
///
/// C'est l'unité de choix de l'éditeur : une coupe de cheveux, une bouche, un sourcil… Les
/// `resource_name*` sont les hashes des modèles 3D, `texture_name` celui de la texture appliquée.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPartsData {
    /// `id` — hash de la part (référencé par [`CharaEditPresetData::parts_id`]).
    pub id: HashId,
    /// `gender` — restriction de genre (`0` = tous).
    pub gender: i64,
    /// `itemNo` — rang dans la catégorie.
    pub item_no: i64,
    /// `resourceName1` — hash du modèle principal (crc32 de [`Self::resource_name_str1`]).
    pub resource_name1: HashId,
    /// `resourceName2` — hash du modèle secondaire (`0` si aucun ; ex. cheveux arrière).
    pub resource_name2: HashId,
    /// `resourceNameStr1` — nom en clair du modèle principal quand il est fourni.
    pub resource_name_str1: String,
    /// `resourceNameStr2` — nom en clair du modèle secondaire (`0xFFFFFFFF` = absent).
    pub resource_name_str2: String,
    /// `textureName` — hash de la texture appliquée.
    pub texture_name: HashId,
    /// `viewNo` — rang d'affichage dans la grille de l'éditeur.
    pub view_no: i64,
}

impl CharaEditPartsData {
    fn from_value(v: &Value) -> Self {
        Self {
            id: field_hash(v, "id"),
            gender: field_i64(v, "gender").unwrap_or(0),
            item_no: field_i64(v, "itemNo").unwrap_or(0),
            resource_name1: field_hash(v, "resourceName1"),
            resource_name2: field_hash(v, "resourceName2"),
            resource_name_str1: s(v, "resourceNameStr1"),
            resource_name_str2: s(v, "resourceNameStr2"),
            texture_name: field_hash(v, "textureName"),
            view_no: field_i64(v, "viewNo").unwrap_or(0),
        }
    }
}

/// Une **catégorie de parts** (`m_CharaEditPartsInfoList`, 20 entrées) + sa plage.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPartsInfo {
    /// `faceSettingType` — identifiant de la catégorie de réglage (cheveux, œil, bouche…).
    pub face_setting_type: i64,
    /// `partsData[0]` — offset dans `m_CharaEditPartsDataList`.
    pub data_offset: i64,
    /// `partsData[1]` — nombre de parts de la catégorie.
    pub data_count: i64,
}

impl CharaEditPartsInfo {
    fn from_value(v: &Value) -> Self {
        let (data_offset, data_count) = pair(v, "partsData");
        Self {
            face_setting_type: field_i64(v, "faceSettingType").unwrap_or(0),
            data_offset,
            data_count,
        }
    }
}

/// Un **curseur de morphing** (`m_CharaEditPartsParamDataList`, 218 entrées).
///
/// Décrit une déformation applicable à une part : `param_type` en désigne l'axe (translation,
/// échelle, rotation…), les bornes donnent la course du curseur, et le masque d'application
/// dit à quelles morphologies elle s'applique.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPartsParamData {
    /// `partsID` — part à laquelle le curseur s'applique.
    pub parts_id: HashId,
    /// `resourceName` — hash de la ressource déformée.
    pub resource_name: HashId,
    /// `paramType` — axe de la déformation.
    pub param_type: i64,
    /// `paramDefault` — valeur par défaut du curseur.
    pub param_default: f64,
    /// `paramMin` — borne basse.
    pub param_min: f64,
    /// `paramMax` — borne haute.
    pub param_max: f64,
    /// `isApply<BodyType>` — morphologies concernées, indexé par [`BODY_TYPES`].
    pub apply: [bool; 8],
}

impl CharaEditPartsParamData {
    fn from_value(v: &Value) -> Self {
        Self {
            parts_id: field_hash(v, "partsID"),
            resource_name: field_hash(v, "resourceName"),
            param_type: field_i64(v, "paramType").unwrap_or(0),
            param_default: field_f64(v, "paramDefault").unwrap_or(0.0),
            param_min: field_f64(v, "paramMin").unwrap_or(0.0),
            param_max: field_f64(v, "paramMax").unwrap_or(0.0),
            apply: apply_mask(v),
        }
    }
}

/// Un **groupe de curseurs** (`m_CharaEditPartsParamInfoList`, 8 entrées) + sa plage.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPartsParamInfo {
    /// `partsType` — type de part morphable.
    pub parts_type: i64,
    /// `partsParamData[0]` — offset dans `m_CharaEditPartsParamDataList`.
    pub data_offset: i64,
    /// `partsParamData[1]` — nombre de curseurs.
    pub data_count: i64,
}

impl CharaEditPartsParamInfo {
    fn from_value(v: &Value) -> Self {
        let (data_offset, data_count) = pair(v, "partsParamData");
        Self {
            parts_type: field_i64(v, "partsType").unwrap_or(0),
            data_offset,
            data_count,
        }
    }
}

/// Une **ligne de recette** (`m_CharaEditPresetDataList`, 2 704 entrées).
///
/// C'est l'atome de la description d'un avatar : « pour l'emplacement `recipe_type`, prendre
/// la valeur `recipe_no` / la part `parts_id` / la couleur `color_value` ». Un preset complet
/// en aligne 62 à 72.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPresetData {
    /// `recipeType` — emplacement visé, indexe [`CharaEditRecipeInfo::recipe_type`].
    pub recipe_type: i64,
    /// `recipeNo` — valeur choisie pour cet emplacement.
    pub recipe_no: i64,
    /// `partsId` — part choisie (`0` quand l'emplacement ne désigne pas une part).
    pub parts_id: HashId,
    /// `colorValue` — index de couleur (`-1` = aucune).
    pub color_value: i64,
}

impl CharaEditPresetData {
    fn from_value(v: &Value) -> Self {
        Self {
            recipe_type: field_i64(v, "recipeType").unwrap_or(0),
            recipe_no: field_i64(v, "recipeNo").unwrap_or(0),
            parts_id: field_hash(v, "partsId"),
            color_value: field_i64(v, "colorValue").unwrap_or(-1),
        }
    }
}

/// Un **visage prédéfini** (`m_CharaEditPresetInfoList`, 38 entrées) + sa recette.
///
/// `preset_id` est le crc32 du nom de ressource (`preset_01_normal`…), c'est-à-dire exactement
/// le `resourceName1` de la part correspondante dans [`CharaEditPartsData`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditPresetInfo {
    /// `presetID` — hash du preset.
    pub preset_id: HashId,
    /// `presetData[0]` — offset dans `m_CharaEditPresetDataList`.
    pub data_offset: i64,
    /// `presetData[1]` — nombre de lignes de recette.
    pub data_count: i64,
    /// `isApply<BodyType>` — morphologies pour lesquelles le preset est proposé.
    pub apply: [bool; 8],
}

impl CharaEditPresetInfo {
    fn from_value(v: &Value) -> Self {
        let (data_offset, data_count) = pair(v, "presetData");
        Self {
            preset_id: field_hash(v, "presetID"),
            data_offset,
            data_count,
            apply: apply_mask(v),
        }
    }
}

/// Une **palette de couleurs** (`m_CharaEditColorInfoList`, 8 entrées) + sa plage.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditColorInfo {
    /// `faceSettingType` — catégorie de réglage à laquelle la palette s'applique.
    pub face_setting_type: i64,
    /// `colorPresetData[0]` — offset dans `m_CharaEditColorDataList`.
    pub data_offset: i64,
    /// `colorPresetData[1]` — nombre de couleurs de la palette.
    pub data_count: i64,
}

impl CharaEditColorInfo {
    fn from_value(v: &Value) -> Self {
        let (data_offset, data_count) = pair(v, "colorPresetData");
        Self {
            face_setting_type: field_i64(v, "faceSettingType").unwrap_or(0),
            data_offset,
            data_count,
        }
    }
}

/// Une **tenue et ses morphologies autorisées** (`m_CharaEditFashionBodyInfoList`, 5 entrées).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditFashionBodyInfo {
    /// `id` — hash de la tenue (= [`CharaEditFashionInfo::fashion_name_crc`]).
    pub id: HashId,
    /// `enableBodyType[0]` — offset dans `m_CharaEditFashionBodyDataList`.
    pub data_offset: i64,
    /// `enableBodyType[1]` — nombre de morphologies autorisées.
    pub data_count: i64,
}

impl CharaEditFashionBodyInfo {
    fn from_value(v: &Value) -> Self {
        let (data_offset, data_count) = pair(v, "enableBodyType");
        Self {
            id: field_hash(v, "id"),
            data_offset,
            data_count,
        }
    }
}

/// Le catalogue complet de l'éditeur d'avatar — les 16 listes de `chara_edit_<ver>.cfg.bin.json`.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaEditConfig {
    /// Catégories du code de partage (86).
    pub recipes: Vec<CharaEditRecipeInfo>,
    /// Alphabet du code de partage (64).
    pub codes: Vec<CharaEditCodeInfo>,
    /// Voix (96).
    pub voices: Vec<CharaEditVoiceInfo>,
    /// Tenues (5).
    pub fashions: Vec<CharaEditFashionInfo>,
    /// Personnalités (24).
    pub personalities: Vec<CharaEditPersonalityInfo>,
    /// Presets-fichiers, modèles complets (31).
    pub preset_files: Vec<CharaEditPresetFileInfo>,
    /// Parts sélectionnables (502).
    pub parts: Vec<CharaEditPartsData>,
    /// Catégories de parts (20).
    pub parts_info: Vec<CharaEditPartsInfo>,
    /// Curseurs de morphing (218).
    pub params: Vec<CharaEditPartsParamData>,
    /// Groupes de curseurs (8).
    pub params_info: Vec<CharaEditPartsParamInfo>,
    /// Lignes de recette (2 704).
    pub preset_data: Vec<CharaEditPresetData>,
    /// Visages prédéfinis (38).
    pub preset_info: Vec<CharaEditPresetInfo>,
    /// Couleurs, `colorPresetID` (470).
    pub colors: Vec<HashId>,
    /// Palettes de couleurs (8).
    pub colors_info: Vec<CharaEditColorInfo>,
    /// Morphologies autorisées, à plat (50).
    pub fashion_body: Vec<i64>,
    /// Tenues → plage de morphologies (5).
    pub fashion_body_info: Vec<CharaEditFashionBodyInfo>,
}

/// Découpe `[offset, count]` dans un `Vec`, en tolérant une plage hors bornes.
fn slice_range<T>(all: &[T], offset: i64, count: i64) -> &[T] {
    let start = usize::try_from(offset.max(0))
        .unwrap_or(usize::MAX)
        .min(all.len());
    let end = usize::try_from(offset.max(0) + count.max(0))
        .unwrap_or(usize::MAX)
        .min(all.len());
    &all[start..end]
}

impl CharaEditConfig {
    /// Les parts d'une catégorie (`faceSettingType`), résolues par sa plage.
    #[must_use]
    pub fn parts_of(&self, face_setting_type: i64) -> &[CharaEditPartsData] {
        self.parts_info
            .iter()
            .find(|i| i.face_setting_type == face_setting_type)
            .map_or(&[][..], |i| {
                slice_range(&self.parts, i.data_offset, i.data_count)
            })
    }

    /// Les curseurs de morphing d'un `partsType`.
    #[must_use]
    pub fn params_of(&self, parts_type: i64) -> &[CharaEditPartsParamData] {
        self.params_info
            .iter()
            .find(|i| i.parts_type == parts_type)
            .map_or(&[][..], |i| {
                slice_range(&self.params, i.data_offset, i.data_count)
            })
    }

    /// La palette de couleurs d'une catégorie (`faceSettingType`).
    #[must_use]
    pub fn colors_of(&self, face_setting_type: i64) -> &[HashId] {
        self.colors_info
            .iter()
            .find(|i| i.face_setting_type == face_setting_type)
            .map_or(&[][..], |i| {
                slice_range(&self.colors, i.data_offset, i.data_count)
            })
    }

    /// La recette d'un visage prédéfini, par son `presetID`.
    #[must_use]
    pub fn recipe_of(&self, preset_id: HashId) -> &[CharaEditPresetData] {
        self.preset_info
            .iter()
            .find(|i| i.preset_id == preset_id)
            .map_or(&[][..], |i| {
                slice_range(&self.preset_data, i.data_offset, i.data_count)
            })
    }

    /// Les morphologies autorisées pour une tenue, par son hash de nom.
    #[must_use]
    pub fn body_types_of_fashion(&self, fashion_id: HashId) -> &[i64] {
        self.fashion_body_info
            .iter()
            .find(|i| i.id == fashion_id)
            .map_or(&[][..], |i| {
                slice_range(&self.fashion_body, i.data_offset, i.data_count)
            })
    }

    /// Retrouve une part par son hash d'identifiant.
    #[must_use]
    pub fn part(&self, id: HashId) -> Option<&CharaEditPartsData> {
        self.parts.iter().find(|p| p.id == id)
    }

    /// Largeur totale, en bits, du code de partage — la somme des `bitNum` des recettes.
    #[must_use]
    pub fn code_bit_width(&self) -> i64 {
        self.recipes.iter().map(|r| r.bit_num).sum()
    }
}

/// Parse `chara_edit_<ver>.cfg.bin.json` → le catalogue complet de l'éditeur d'avatar.
#[must_use]
pub fn parse_chara_edit(root: &Value) -> CharaEditConfig {
    CharaEditConfig {
        recipes: collect_list(
            root,
            "m_CharaEditRecipeInfoList",
            CharaEditRecipeInfo::from_value,
        ),
        codes: collect_list(
            root,
            "m_CharaEditCodeInfoList",
            CharaEditCodeInfo::from_value,
        ),
        voices: collect_list(
            root,
            "m_CharaEditVoiceInfoList",
            CharaEditVoiceInfo::from_value,
        ),
        fashions: collect_list(
            root,
            "m_CharaEditFashionInfoList",
            CharaEditFashionInfo::from_value,
        ),
        personalities: collect_list(
            root,
            "m_CharaEditPersonalityInfoList",
            CharaEditPersonalityInfo::from_value,
        ),
        preset_files: collect_list(
            root,
            "m_CharaEditPresetFileInfoList",
            CharaEditPresetFileInfo::from_value,
        ),
        parts: collect_list(
            root,
            "m_CharaEditPartsDataList",
            CharaEditPartsData::from_value,
        ),
        parts_info: collect_list(
            root,
            "m_CharaEditPartsInfoList",
            CharaEditPartsInfo::from_value,
        ),
        params: collect_list(
            root,
            "m_CharaEditPartsParamDataList",
            CharaEditPartsParamData::from_value,
        ),
        params_info: collect_list(
            root,
            "m_CharaEditPartsParamInfoList",
            CharaEditPartsParamInfo::from_value,
        ),
        preset_data: collect_list(
            root,
            "m_CharaEditPresetDataList",
            CharaEditPresetData::from_value,
        ),
        preset_info: collect_list(
            root,
            "m_CharaEditPresetInfoList",
            CharaEditPresetInfo::from_value,
        ),
        colors: collect_list(root, "m_CharaEditColorDataList", |v| {
            field_hash(v, "colorPresetID")
        }),
        colors_info: collect_list(
            root,
            "m_CharaEditColorInfoList",
            CharaEditColorInfo::from_value,
        ),
        fashion_body: collect_list(root, "m_CharaEditFashionBodyDataList", |v| {
            field_i64(v, "bodyType").unwrap_or(0)
        }),
        fashion_body_info: collect_list(
            root,
            "m_CharaEditFashionBodyInfoList",
            CharaEditFashionBodyInfo::from_value,
        ),
    }
}
