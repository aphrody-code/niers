//! `AcademicYearConfig` — port Rust de `character/academic_year_config.cfg.bin.json` (Level-5 IEVR).
//!
//! Les **années scolaires** d'Inazuma Eleven (jeu de foot scolaire) : 3 entrées
//! `m_academicYearInfoList` de type 1/2/3 (1re/2e/3e année). Format **`lists`** (champs nommés).
//!
//! | Champ                    | Type | Exemple (entrée 0) | Sémantique                          |
//! |--------------------------|------|--------------------|-------------------------------------|
//! | `academicYearId`         | hash | `0x923E7C29`       | identifiant de l'année scolaire      |
//! | `academicYearType`       | i64  | `1`                | type/rang (1 = 1re année … 3 = 3e)   |
//! | `academicYearNameTextId` | hash | `0x429F322D`       | hash du nom localisé (jointure texte)|
//!
//! Vérité terrain : `data/common/gamedata/character/academic_year_config.cfg.bin.json` (3 entrées).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, list_values};
use crate::hash::HashId;

/// Une entrée `m_academicYearInfoList` — une année scolaire.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AcademicYearInfo {
    /// `academicYearId` — identifiant de l'année.
    pub id: HashId,
    /// `academicYearType` — rang : 1 = 1re année, 2 = 2e, 3 = 3e.
    pub year_type: i64,
    /// `academicYearNameTextId` — hash du nom localisé (jointure avec la table de textes).
    pub name_text_id: HashId,
}

impl AcademicYearInfo {
    /// Parse une entrée. `None` si `academicYearId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "academicYearId");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            year_type: field_i64(v, "academicYearType").unwrap_or(0),
            name_text_id: field_hash(v, "academicYearNameTextId"),
        })
    }
}

/// Parse `academic_year_config.cfg.bin.json` → la liste des années scolaires.
#[must_use]
pub fn parse_academic_year_config(root: &Value) -> Vec<AcademicYearInfo> {
    list_values(root, "m_academicYearInfoList").map_or_else(Vec::new, |vs| {
        vs.iter().filter_map(AcademicYearInfo::from_value).collect()
    })
}
