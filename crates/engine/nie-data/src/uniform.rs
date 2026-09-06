//! `UniformConfig` — port Rust de `uniform_config_*.cfg.bin.json` (Level-5 IEVR) :
//! modèles de maillots / tenues (jerseys) des personnages.
//!
//! ## Vérité terrain
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/uniform-config.ts`
//!   (`parseContent` l.80-141, `resolveUniformRows` l.199-215, `buildUniformDatabase`
//!   l.156-172).
//! - Dump réel (VFS Steam) : `data/common/gamedata/character/uniform_config_1.03.52.00.cfg.bin`
//!   (RDBN, format `lists`). Une 2e copie existe sous `item/uniform_config_0.00.00.cfg.bin` ;
//!   inagle pointe la version `character/` — c'est elle qui fait foi ici.
//!
//! ## Structure (5 listes RDBN)
//!
//! | Liste                       | Type RDBN              | Lignes | Sémantique                              |
//! |-----------------------------|------------------------|--------|------------------------------------------|
//! | `m_UniformModelInfoList`    | `UNIFORM_MODEL_INFO`   | 760    | CRC des modèles (fielder/keeper/director/manager + variantes de manches) |
//! | `m_UniformInfoList`         | `UNIFORM_INFO`         | 384    | Uniformes nommés : `nameId` + tranche `modelInfo=[start,count]` |
//! | `m_UniformExModelInfoList`  | `UNIFORM_EX_MODEL_INFO`| 751    | Modèles étendus (ex-models fielder/keeper × 3, face icon…) |
//! | `m_UniformExInfoList`       | `UNIFORM_EX_INFO`      | 378    | Uniformes étendus nommés (+ `tmpFlagIdCrc`) |
//! | `m_CharaUniformExInfoList`  | `CHARA_UNIFORM_EX_INFO`| 234    | Tenues étendues par personnage : `charaId` + tranche `uniformInfo=[start,count]` |
//!
//! Les champs sont majoritairement des CRC 32 bits ([`HashId`]) plus quelques entiers
//! (`typeId`, `shoesModelAttr`, `uniformNgModelAttr`) et un booléen (`shoesModelIdLocked`).
//! Les typos Level-5 des noms de champs sont préservées (port 1:1 d'inagle).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, list_values};
use crate::hash::HashId;

// ─── Helper : tuple [start, count] ──────────────────────────────────────────────

/// Lit un champ tableau de 2 entiers (`modelInfo=[start,count]`, `uniformInfo=[start,count]`).
/// Renvoie `(0, 0)` si absent ou mal formé. Ces tuples sont des `ShortTuple` RDBN sérialisés
/// en tableau JSON `[a, b]`.
fn field_pair(v: &Value, key: &str) -> (i64, i64) {
    if let Some(arr) = v.get(key).and_then(Value::as_array) {
        let a = arr.first().and_then(Value::as_i64).unwrap_or(0);
        let b = arr.get(1).and_then(Value::as_i64).unwrap_or(0);
        (a, b)
    } else {
        (0, 0)
    }
}

// ─── UniformModelInfo (m_UniformModelInfoList) ──────────────────────────────────

/// Entrée `UNIFORM_MODEL_INFO` — CRC des modèles 3D d'un maillot et de ses variantes.
///
/// Port 1:1 d'inagle `UniformModelInfo` (uniform-config.ts l.24-54). 27 champs au total :
/// 4 modèles de base (fielder/keeper/director/manager), 14 variantes de manches/coupe
/// (fielder + keeper × 7 styles), 4 chaussures, 1 gant, puis 4 attributs.
///
/// Vérité terrain : `m_UniformModelInfoList[0]` du dump `uniform_config_1.03.52.00`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UniformModelInfo {
    /// `uniformFielderModelIdCrc` — modèle du maillot de joueur de champ.
    pub uniform_fielder_model_id_crc: HashId,
    /// `uniformKeeperModelIdCrc` — modèle du maillot de gardien.
    pub uniform_keeper_model_id_crc: HashId,
    /// `uniformDirectorModelIdCrc` — modèle de la tenue d'entraîneur (directeur).
    pub uniform_director_model_id_crc: HashId,
    /// `uniformManagerModelIdCrc` — modèle de la tenue de manager.
    pub uniform_manager_model_id_crc: HashId,
    /// `uniformFielderShoulderBaringModelIdCrc` — variante épaules dénudées (joueur de champ).
    pub uniform_fielder_shoulder_baring_model_id_crc: HashId,
    /// `uniformKeeperShoulderBaringModelIdCrc` — variante épaules dénudées (gardien).
    pub uniform_keeper_shoulder_baring_model_id_crc: HashId,
    /// `uniformFielderShortSleeveRollUpArmModelIdCrc` — manches courtes retroussées (joueur de champ).
    pub uniform_fielder_short_sleeve_roll_up_arm_model_id_crc: HashId,
    /// `uniformKeeperShortSleeveRollUpArmModelIdCrc` — manches courtes retroussées (gardien).
    pub uniform_keeper_short_sleeve_roll_up_arm_model_id_crc: HashId,
    /// `uniformFielderShoulderBaringPatternedModelIdCrc` — épaules dénudées à motif (joueur de champ).
    pub uniform_fielder_shoulder_baring_patterned_model_id_crc: HashId,
    /// `uniformKeeperShoulderBaringPatternedModelIdCrc` — épaules dénudées à motif (gardien).
    pub uniform_keeper_shoulder_baring_patterned_model_id_crc: HashId,
    /// `uniformFielderHalfSleeveModelIdCrc` — demi-manches (joueur de champ).
    pub uniform_fielder_half_sleeve_model_id_crc: HashId,
    /// `uniformKeeperHalfSleeveModelIdCrc` — demi-manches (gardien).
    pub uniform_keeper_half_sleeve_model_id_crc: HashId,
    /// `uniformFielderLongSleeveRollUpArmModelIdCrc` — manches longues bras retroussés (joueur de champ).
    pub uniform_fielder_long_sleeve_roll_up_arm_model_id_crc: HashId,
    /// `uniformKeeperLongSleeveRollUpArmModelIdCrc` — manches longues bras retroussés (gardien).
    pub uniform_keeper_long_sleeve_roll_up_arm_model_id_crc: HashId,
    /// `uniformFielderLongSleeveRollUpSleeveModelIdCrc` — manches longues remontées (joueur de champ).
    pub uniform_fielder_long_sleeve_roll_up_sleeve_model_id_crc: HashId,
    /// `uniformKeeperLongSleeveRollUpSleeveModelIdCrc` — manches longues remontées (gardien).
    pub uniform_keeper_long_sleeve_roll_up_sleeve_model_id_crc: HashId,
    /// `uniformFielderNavelBaringModelIdCrc` — variante nombril dénudé (joueur de champ).
    pub uniform_fielder_navel_baring_model_id_crc: HashId,
    /// `uniformKeeperNavelBaringModelIdCrc` — variante nombril dénudé (gardien).
    pub uniform_keeper_navel_baring_model_id_crc: HashId,
    /// `shoesFielderModelIdCrc` — modèle des chaussures de joueur de champ.
    pub shoes_fielder_model_id_crc: HashId,
    /// `shoesKeeperModelIdCrc` — modèle des chaussures de gardien.
    pub shoes_keeper_model_id_crc: HashId,
    /// `shoesDirectorModelIdCrc` — modèle des chaussures d'entraîneur.
    pub shoes_director_model_id_crc: HashId,
    /// `shoesManagerModelIdCrc` — modèle des chaussures de manager.
    pub shoes_manager_model_id_crc: HashId,
    /// `gloveModelIdCrc` — modèle des gants (gardien).
    pub glove_model_id_crc: HashId,
    /// `typeId` — identifiant de type de l'entrée (0 = standard…).
    pub type_id: i64,
    /// `shoesModelAttr` — attribut du modèle de chaussures.
    pub shoes_model_attr: i64,
    /// `uniformNgModelAttr` — attribut « NG » (no-good / restriction) du modèle de maillot.
    pub uniform_ng_model_attr: i64,
    /// `shoesModelIdLocked` — vrai si le modèle de chaussures est verrouillé (non interchangeable).
    pub shoes_model_id_locked: bool,
}

impl UniformModelInfo {
    /// Parse une ligne `m_UniformModelInfoList`. Tous les champs absents valent
    /// [`HashId::ZERO`]/`0`/`false` (port 1:1 d'inagle, aucun filtrage).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            uniform_fielder_model_id_crc: field_hash(v, "uniformFielderModelIdCrc"),
            uniform_keeper_model_id_crc: field_hash(v, "uniformKeeperModelIdCrc"),
            uniform_director_model_id_crc: field_hash(v, "uniformDirectorModelIdCrc"),
            uniform_manager_model_id_crc: field_hash(v, "uniformManagerModelIdCrc"),
            uniform_fielder_shoulder_baring_model_id_crc: field_hash(
                v,
                "uniformFielderShoulderBaringModelIdCrc",
            ),
            uniform_keeper_shoulder_baring_model_id_crc: field_hash(
                v,
                "uniformKeeperShoulderBaringModelIdCrc",
            ),
            uniform_fielder_short_sleeve_roll_up_arm_model_id_crc: field_hash(
                v,
                "uniformFielderShortSleeveRollUpArmModelIdCrc",
            ),
            uniform_keeper_short_sleeve_roll_up_arm_model_id_crc: field_hash(
                v,
                "uniformKeeperShortSleeveRollUpArmModelIdCrc",
            ),
            uniform_fielder_shoulder_baring_patterned_model_id_crc: field_hash(
                v,
                "uniformFielderShoulderBaringPatternedModelIdCrc",
            ),
            uniform_keeper_shoulder_baring_patterned_model_id_crc: field_hash(
                v,
                "uniformKeeperShoulderBaringPatternedModelIdCrc",
            ),
            uniform_fielder_half_sleeve_model_id_crc: field_hash(
                v,
                "uniformFielderHalfSleeveModelIdCrc",
            ),
            uniform_keeper_half_sleeve_model_id_crc: field_hash(
                v,
                "uniformKeeperHalfSleeveModelIdCrc",
            ),
            uniform_fielder_long_sleeve_roll_up_arm_model_id_crc: field_hash(
                v,
                "uniformFielderLongSleeveRollUpArmModelIdCrc",
            ),
            uniform_keeper_long_sleeve_roll_up_arm_model_id_crc: field_hash(
                v,
                "uniformKeeperLongSleeveRollUpArmModelIdCrc",
            ),
            uniform_fielder_long_sleeve_roll_up_sleeve_model_id_crc: field_hash(
                v,
                "uniformFielderLongSleeveRollUpSleeveModelIdCrc",
            ),
            uniform_keeper_long_sleeve_roll_up_sleeve_model_id_crc: field_hash(
                v,
                "uniformKeeperLongSleeveRollUpSleeveModelIdCrc",
            ),
            uniform_fielder_navel_baring_model_id_crc: field_hash(
                v,
                "uniformFielderNavelBaringModelIdCrc",
            ),
            uniform_keeper_navel_baring_model_id_crc: field_hash(
                v,
                "uniformKeeperNavelBaringModelIdCrc",
            ),
            shoes_fielder_model_id_crc: field_hash(v, "shoesFielderModelIdCrc"),
            shoes_keeper_model_id_crc: field_hash(v, "shoesKeeperModelIdCrc"),
            shoes_director_model_id_crc: field_hash(v, "shoesDirectorModelIdCrc"),
            shoes_manager_model_id_crc: field_hash(v, "shoesManagerModelIdCrc"),
            glove_model_id_crc: field_hash(v, "gloveModelIdCrc"),
            type_id: field_i64(v, "typeId").unwrap_or(0),
            shoes_model_attr: field_i64(v, "shoesModelAttr").unwrap_or(0),
            uniform_ng_model_attr: field_i64(v, "uniformNgModelAttr").unwrap_or(0),
            shoes_model_id_locked: v
                .get("shoesModelIdLocked")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }
}

// ─── UniformInfo (m_UniformInfoList) ────────────────────────────────────────────

/// Entrée `UNIFORM_INFO` — uniforme nommé pointant une tranche de [`UniformModelInfo`].
///
/// Port 1:1 d'inagle `UniformInfo` (uniform-config.ts l.61-65) : `nameId` (CRC du libellé,
/// clé stable) + `modelInfo = [start, count]` qui découpe `m_UniformModelInfoList`.
///
/// Vérité terrain : `m_UniformInfoList[0] = { nameId: 0x252CE113, modelInfo: [0, 2] }`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UniformInfo {
    /// `nameId` — CRC du libellé de l'uniforme (clé primaire stable).
    pub name_id: HashId,
    /// `modelInfo[0]` — index de départ dans `m_UniformModelInfoList`.
    pub model_start: i64,
    /// `modelInfo[1]` — nombre de modèles consécutifs rattachés à cet uniforme.
    pub model_count: i64,
}

impl UniformInfo {
    /// Parse une ligne `m_UniformInfoList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        let (model_start, model_count) = field_pair(v, "modelInfo");
        Self {
            name_id: field_hash(v, "nameId"),
            model_start,
            model_count,
        }
    }
}

// ─── UniformExModelInfo (m_UniformExModelInfoList) ──────────────────────────────

/// Entrée `UNIFORM_EX_MODEL_INFO` — modèle de maillot étendu (variantes spéciales,
/// modèles « ex » fielder/keeper jusqu'à 3 variantes, icône de visage liée…).
///
/// Inagle traite cette liste en passthrough (`any[]`, uniform-config.ts l.132-133) ;
/// on en fait ici une struct typée d'après les 14 champs réels du dump.
///
/// Vérité terrain : `m_UniformExModelInfoList[0]` du dump `uniform_config_1.03.52.00`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UniformExModelInfo {
    /// `charaModelIdCrc` — CRC du modèle de personnage lié (souvent nul).
    pub chara_model_id_crc: HashId,
    /// `faceIconCharaIdCrc` — CRC du personnage de l'icône de visage (souvent nul).
    pub face_icon_chara_id_crc: HashId,
    /// `fielderExModelId1` — 1re variante de modèle étendu de joueur de champ.
    pub fielder_ex_model_id1: HashId,
    /// `fielderExModelId2` — 2e variante.
    pub fielder_ex_model_id2: HashId,
    /// `fielderExModelId3` — 3e variante.
    pub fielder_ex_model_id3: HashId,
    /// `keeperExModelId1` — 1re variante de modèle étendu de gardien.
    pub keeper_ex_model_id1: HashId,
    /// `keeperExModelId2` — 2e variante.
    pub keeper_ex_model_id2: HashId,
    /// `keeperExModelId3` — 3e variante.
    pub keeper_ex_model_id3: HashId,
    /// `gloveModelIdCrc` — modèle des gants.
    pub glove_model_id_crc: HashId,
    /// `shoesFielderModelIdCrc` — chaussures de joueur de champ.
    pub shoes_fielder_model_id_crc: HashId,
    /// `shoesKeeperModelIdCrc` — chaussures de gardien.
    pub shoes_keeper_model_id_crc: HashId,
    /// `uniformFielderModelIdCrc` — modèle de maillot de joueur de champ.
    pub uniform_fielder_model_id_crc: HashId,
    /// `uniformKeeperModelIdCrc` — modèle de maillot de gardien.
    pub uniform_keeper_model_id_crc: HashId,
    /// `typeId` — identifiant de type de l'entrée.
    pub type_id: i64,
}

impl UniformExModelInfo {
    /// Parse une ligne `m_UniformExModelInfoList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            chara_model_id_crc: field_hash(v, "charaModelIdCrc"),
            face_icon_chara_id_crc: field_hash(v, "faceIconCharaIdCrc"),
            fielder_ex_model_id1: field_hash(v, "fielderExModelId1"),
            fielder_ex_model_id2: field_hash(v, "fielderExModelId2"),
            fielder_ex_model_id3: field_hash(v, "fielderExModelId3"),
            keeper_ex_model_id1: field_hash(v, "keeperExModelId1"),
            keeper_ex_model_id2: field_hash(v, "keeperExModelId2"),
            keeper_ex_model_id3: field_hash(v, "keeperExModelId3"),
            glove_model_id_crc: field_hash(v, "gloveModelIdCrc"),
            shoes_fielder_model_id_crc: field_hash(v, "shoesFielderModelIdCrc"),
            shoes_keeper_model_id_crc: field_hash(v, "shoesKeeperModelIdCrc"),
            uniform_fielder_model_id_crc: field_hash(v, "uniformFielderModelIdCrc"),
            uniform_keeper_model_id_crc: field_hash(v, "uniformKeeperModelIdCrc"),
            type_id: field_i64(v, "typeId").unwrap_or(0),
        }
    }
}

// ─── UniformExInfo (m_UniformExInfoList) ────────────────────────────────────────

/// Entrée `UNIFORM_EX_INFO` — uniforme étendu nommé, pointant une tranche de
/// [`UniformExModelInfo`] (+ un éventuel CRC de flag temporaire).
///
/// Vérité terrain : `m_UniformExInfoList[0] = { nameId: 0x252CE113, modelInfo: [0, 2],
/// tmpFlagIdCrc: 0x00000000 }`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UniformExInfo {
    /// `nameId` — CRC du libellé de l'uniforme étendu.
    pub name_id: HashId,
    /// `modelInfo[0]` — index de départ dans `m_UniformExModelInfoList`.
    pub model_start: i64,
    /// `modelInfo[1]` — nombre de modèles étendus rattachés.
    pub model_count: i64,
    /// `tmpFlagIdCrc` — CRC d'un flag temporaire de déblocage (souvent nul).
    pub tmp_flag_id_crc: HashId,
}

impl UniformExInfo {
    /// Parse une ligne `m_UniformExInfoList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        let (model_start, model_count) = field_pair(v, "modelInfo");
        Self {
            name_id: field_hash(v, "nameId"),
            model_start,
            model_count,
            tmp_flag_id_crc: field_hash(v, "tmpFlagIdCrc"),
        }
    }
}

// ─── CharaUniformExInfo (m_CharaUniformExInfoList) ──────────────────────────────

/// Entrée `CHARA_UNIFORM_EX_INFO` — tenues étendues disponibles pour un personnage,
/// pointant une tranche de [`UniformExInfo`].
///
/// Vérité terrain : `m_CharaUniformExInfoList[0] = { charaId: 0x99A1C150,
/// uniformInfo: [0, 10] }`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaUniformExInfo {
    /// `charaId` — CRC du personnage.
    pub chara_id: HashId,
    /// `uniformInfo[0]` — index de départ dans `m_UniformExInfoList`.
    pub uniform_start: i64,
    /// `uniformInfo[1]` — nombre d'uniformes étendus rattachés au personnage.
    pub uniform_count: i64,
}

impl CharaUniformExInfo {
    /// Parse une ligne `m_CharaUniformExInfoList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        let (uniform_start, uniform_count) = field_pair(v, "uniformInfo");
        Self {
            chara_id: field_hash(v, "charaId"),
            uniform_start,
            uniform_count,
        }
    }
}

// ─── UniformConfig (racine) ─────────────────────────────────────────────────────

/// Contenu complet d'un `uniform_config_*.cfg.bin.json` (5 listes RDBN).
///
/// Construire via [`parse_uniform_config`].
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UniformConfig {
    /// `m_UniformModelInfoList` — modèles de maillots (760 entrées dans le dump réel).
    pub models: Vec<UniformModelInfo>,
    /// `m_UniformInfoList` — uniformes nommés (384 entrées).
    pub uniforms: Vec<UniformInfo>,
    /// `m_UniformExModelInfoList` — modèles étendus (751 entrées).
    pub ex_models: Vec<UniformExModelInfo>,
    /// `m_UniformExInfoList` — uniformes étendus nommés (378 entrées).
    pub ex_uniforms: Vec<UniformExInfo>,
    /// `m_CharaUniformExInfoList` — tenues étendues par personnage (234 entrées).
    pub chara_ex_uniforms: Vec<CharaUniformExInfo>,
}

/// Ligne d'uniforme résolue : une entrée de [`UniformInfo`] jointe à ses modèles réels.
///
/// Port 1:1 d'inagle `UniformRow` + `resolveUniformRows` (uniform-config.ts l.186-215).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniformRow<'a> {
    /// `nameId` — CRC du libellé (clé primaire stable).
    pub name_id: HashId,
    /// Index de départ dans `m_UniformModelInfoList`.
    pub model_start: i64,
    /// Nombre de modèles consécutifs rattachés à cet uniforme.
    pub model_count: i64,
    /// `typeId` du 1er modèle de la tranche (`None` si la tranche est vide).
    pub type_id: Option<i64>,
    /// Les modèles réels de la tranche (références dans [`UniformConfig::models`]).
    pub models: Vec<&'a UniformModelInfo>,
}

impl UniformConfig {
    /// Recherche un uniforme nommé par son `nameId`. `None` si absent.
    #[must_use]
    pub fn find_uniform_by_name_id(&self, name_id: HashId) -> Option<&UniformInfo> {
        self.uniforms.iter().find(|u| u.name_id == name_id)
    }

    /// Recherche les tenues étendues d'un personnage par son `charaId`. `None` si absent.
    #[must_use]
    pub fn find_chara_ex(&self, chara_id: HashId) -> Option<&CharaUniformExInfo> {
        self.chara_ex_uniforms
            .iter()
            .find(|c| c.chara_id == chara_id)
    }

    /// Renvoie les modèles ayant un `typeId` donné (port de `byTypeId`, uniform-config.ts l.164-169).
    #[must_use]
    pub fn models_with_type_id(&self, type_id: i64) -> Vec<&UniformModelInfo> {
        self.models
            .iter()
            .filter(|m| m.type_id == type_id)
            .collect()
    }

    /// Résout chaque [`UniformInfo`] en joignant sa tranche `modelInfo=[start,count]` aux
    /// objets réels de `models`. Port 1:1 d'inagle `resolveUniformRows` (l.199-215) : la
    /// tranche est bornée à la longueur de `models` (pas de panique sur un index hors limite).
    #[must_use]
    pub fn resolve_rows(&self) -> Vec<UniformRow<'_>> {
        let mut rows = Vec::with_capacity(self.uniforms.len());
        for u in &self.uniforms {
            let start = u.model_start.max(0) as usize;
            let count = u.model_count.max(0) as usize;
            let end = start.saturating_add(count).min(self.models.len());
            let slice: Vec<&UniformModelInfo> = if start < self.models.len() {
                self.models[start..end].iter().collect()
            } else {
                Vec::new()
            };
            rows.push(UniformRow {
                name_id: u.name_id,
                model_start: u.model_start,
                model_count: u.model_count,
                type_id: slice.first().map(|m| m.type_id),
                models: slice,
            });
        }
        rows
    }
}

// ─── Parseur principal ──────────────────────────────────────────────────────────

/// Parse un `uniform_config_*.cfg.bin.json` complet (5 listes).
///
/// Port 1:1 d'inagle `parseContent` (uniform-config.ts l.80-141) : chaque liste est
/// itérée et convertie ; les listes absentes donnent un vecteur vide. Aucun filtrage —
/// toutes les lignes sont conservées (comptes réels 760/384/751/378/234).
#[must_use]
pub fn parse_uniform_config(root: &Value) -> UniformConfig {
    let models = list_values(root, "m_UniformModelInfoList")
        .map(|vals| vals.iter().map(UniformModelInfo::from_value).collect())
        .unwrap_or_default();
    let uniforms = list_values(root, "m_UniformInfoList")
        .map(|vals| vals.iter().map(UniformInfo::from_value).collect())
        .unwrap_or_default();
    let ex_models = list_values(root, "m_UniformExModelInfoList")
        .map(|vals| vals.iter().map(UniformExModelInfo::from_value).collect())
        .unwrap_or_default();
    let ex_uniforms = list_values(root, "m_UniformExInfoList")
        .map(|vals| vals.iter().map(UniformExInfo::from_value).collect())
        .unwrap_or_default();
    let chara_ex_uniforms = list_values(root, "m_CharaUniformExInfoList")
        .map(|vals| vals.iter().map(CharaUniformExInfo::from_value).collect())
        .unwrap_or_default();
    UniformConfig {
        models,
        uniforms,
        ex_models,
        ex_uniforms,
        chara_ex_uniforms,
    }
}
