//! `RealSkillConfig` — port Rust de `skill/real_skill_config_*.cfg.bin` (Level-5 IEVR).
//!
//! Décrit les trajectoires de tir « real skill » (la physique des tirs spéciaux) :
//! une bibliothèque de courbes de tir et la liste des tirs qui les référencent par
//! tranche `[offset, count]`.
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/skill/real_skill_config_1.03.74.00.cfg.bin` (VFS IEVR).
//! - Référence de portage 1:1 : `packages/inagle/src/parsers/real-skill-config.ts`.
//! - Format : RDBN à listes (`{ "version", "lists": [{ "name", "typeName", "values" }] }`).
//!
//! ## Structure (2 listes)
//!
//! - `m_RealSkillShootCourseInfoList` (type `RealSkillShootCourseInfo`) — 6 courbes de tir.
//! - `m_RealSkillInfoList` (type `RealSkillInfo`) — 19 tirs, chacun référençant une tranche
//!   `shootCourseInfoRef = [start, count]` de la 1re liste.
//!
//! ## Sémantique de référence (`shootCourseInfoRef`)
//!
//! Comme inagle (`getCoursesForSkill`), un tir `[start, count]` désigne
//! `courses[start .. start + count]`. La plupart des tirs valent `[0, 0]` (aucune courbe
//! associée) ; seuls quelques tirs spéciaux tranchent la liste (ex. `0x35342155` → `[2, 2]`).

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, list_values};
use crate::hash::HashId;

/// Lit un champ flottant d'un objet `values[]` (les `Float` RDBN arrivent en `1.0`, `0.65`…).
fn field_f64(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

// ─── RealSkillShootCourse ───────────────────────────────────────────────────────

/// Une courbe de trajectoire de tir (`m_RealSkillShootCourseInfoList`).
///
/// Port 1:1 de `RealSkillShootCourse` (inagle `real-skill-config.ts`). Tous les champs sont
/// des paramètres de physique du tir spécial (rates, offsets, angle de courbe).
///
/// Vérité terrain — `real_skill_config_1.03.74.00`, course `[0]` :
/// `targetRate=1.0, isGroundTarget=false, curveRate≈0.7, curveHeightRate=0.5,
/// curveAngle=65.0, moveTimeRate=1.0`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RealSkillShootCourse {
    /// `targetRate` — ratio de visée vers la cible (0..1).
    pub target_rate: f64,
    /// `isGroundTarget` — la cible est-elle au sol (tir rasant).
    pub is_ground_target: bool,
    /// `targetHeightOffset` — décalage vertical de la cible.
    pub target_height_offset: f64,
    /// `targetHoriOffset` — décalage horizontal de la cible.
    pub target_hori_offset: f64,
    /// `curveRate` — intensité de la courbe.
    pub curve_rate: f64,
    /// `curveHeightRate` — composante verticale de la courbe.
    pub curve_height_rate: f64,
    /// `curveAngle` — angle de la courbe (degrés).
    pub curve_angle: f64,
    /// `moveTimeRate` — multiplicateur de durée de déplacement du ballon.
    pub move_time_rate: f64,
}

impl RealSkillShootCourse {
    /// Parse une entrée de `m_RealSkillShootCourseInfoList` (infaillible, comme inagle).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            target_rate: field_f64(v, "targetRate"),
            is_ground_target: field_bool(v, "isGroundTarget").unwrap_or(false),
            target_height_offset: field_f64(v, "targetHeightOffset"),
            target_hori_offset: field_f64(v, "targetHoriOffset"),
            curve_rate: field_f64(v, "curveRate"),
            curve_height_rate: field_f64(v, "curveHeightRate"),
            curve_angle: field_f64(v, "curveAngle"),
            move_time_rate: field_f64(v, "moveTimeRate"),
        }
    }
}

// ─── RealSkillInfo ──────────────────────────────────────────────────────────────

/// Un tir « real skill » (`m_RealSkillInfoList`) et sa référence vers des courbes.
///
/// Port 1:1 de `RealSkillInfo` (inagle `real-skill-config.ts`). Les typos/casing Level-5
/// des champs sont préservés en commentaire.
///
/// Vérité terrain — `real_skill_config_1.03.74.00`, tir `0x636E86D3` (`[16]`) :
/// `loseType=0, formationCharaLen=3.0, shootCourseInfoRef=[5,1],
/// shootGroundingEffectName=0x1C8B0F11, shootGroundingEffectScale=5.0`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RealSkillInfo {
    /// `id` — hash identifiant le tir (réf. vers `skill_config`).
    pub id: HashId,
    /// `loseType` — type de perte de balle (0/1 observés).
    pub lose_type: i64,
    /// `formationType` — type de formation du tir (0 partout dans le dump).
    pub formation_type: i64,
    /// `formationCharaLen` — nombre de personnages de la formation (Float : 3.0 ou 5.0).
    pub formation_chara_len: f64,
    /// `shootLimitBallHeightAttr` — attribut de hauteur limite du ballon (0/1/6 observés).
    pub shoot_limit_ball_height_attr: i64,
    /// `shootGroundingEffectName` — hash de l'effet au sol (`0x00000000` = aucun).
    pub shoot_grounding_effect_name: HashId,
    /// `shootGroundingEffectIntervalTime` — intervalle de l'effet au sol (-1.0 = aucun).
    pub shoot_grounding_effect_interval_time: f64,
    /// `shootGroundingEffectScale` — échelle de l'effet au sol.
    pub shoot_grounding_effect_scale: f64,
    /// `shootGroundingEffectScatteringPos` — dispersion de l'effet au sol.
    pub shoot_grounding_effect_scattering_pos: f64,
    /// `shootCourseInfoRef` — `[start, count]` : tranche de la liste des courbes.
    pub shoot_course_info_ref: [i64; 2],
}

impl RealSkillInfo {
    /// Parse une entrée de `m_RealSkillInfoList` (infaillible, comme inagle).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        Self {
            id: field_hash(v, "id"),
            lose_type: field_i64(v, "loseType").unwrap_or(0),
            formation_type: field_i64(v, "formationType").unwrap_or(0),
            formation_chara_len: field_f64(v, "formationCharaLen"),
            shoot_limit_ball_height_attr: field_i64(v, "shootLimitBallHeightAttr").unwrap_or(0),
            shoot_grounding_effect_name: field_hash(v, "shootGroundingEffectName"),
            shoot_grounding_effect_interval_time: field_f64(v, "shootGroundingEffectIntervalTime"),
            shoot_grounding_effect_scale: field_f64(v, "shootGroundingEffectScale"),
            shoot_grounding_effect_scattering_pos: field_f64(
                v,
                "shootGroundingEffectScatteringPos",
            ),
            shoot_course_info_ref: read_course_ref(v),
        }
    }

    /// Index de départ de la tranche de courbes (`shootCourseInfoRef[0]`, borné à 0).
    #[must_use]
    pub fn course_start(&self) -> usize {
        self.shoot_course_info_ref[0].max(0) as usize
    }

    /// Nombre de courbes de la tranche (`shootCourseInfoRef[1]`, borné à 0).
    #[must_use]
    pub fn course_count(&self) -> usize {
        self.shoot_course_info_ref[1].max(0) as usize
    }
}

/// Lit `shootCourseInfoRef` (tableau de 2 entiers `[start, count]`). `[0, 0]` si absent.
fn read_course_ref(v: &Value) -> [i64; 2] {
    let arr = match v.get("shootCourseInfoRef").and_then(Value::as_array) {
        Some(a) => a,
        None => return [0, 0],
    };
    let get = |i: usize| arr.get(i).and_then(Value::as_i64).unwrap_or(0);
    [get(0), get(1)]
}

// ─── RealSkillConfig ────────────────────────────────────────────────────────────

/// Contenu complet d'un `real_skill_config_*.cfg.bin` : courbes + tirs.
///
/// Équivalent du `RealSkillDatabase` d'inagle (`buildRealSkillDatabase`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RealSkillConfig {
    /// `m_RealSkillShootCourseInfoList` — bibliothèque de courbes (6 dans le dump 1.03.74.00).
    pub courses: Vec<RealSkillShootCourse>,
    /// `m_RealSkillInfoList` — tirs (19 dans le dump 1.03.74.00).
    pub skills: Vec<RealSkillInfo>,
}

impl RealSkillConfig {
    /// Recherche un tir par son `id`. `None` si absent.
    #[must_use]
    pub fn find_skill(&self, id: HashId) -> Option<&RealSkillInfo> {
        self.skills.iter().find(|s| s.id == id)
    }

    /// Courbes associées à un tir donné (`courses[start .. start + count]`), comme
    /// `getCoursesForSkill` d'inagle. Vide si le tir est inconnu ou si la tranche déborde.
    #[must_use]
    pub fn courses_for_skill(&self, id: HashId) -> &[RealSkillShootCourse] {
        match self.find_skill(id) {
            Some(s) => self.courses_for(s),
            None => &[],
        }
    }

    /// Courbes associées à un tir déjà résolu (slice borné à la liste).
    #[must_use]
    pub fn courses_for(&self, skill: &RealSkillInfo) -> &[RealSkillShootCourse] {
        let start = skill.course_start().min(self.courses.len());
        let end = start
            .saturating_add(skill.course_count())
            .min(self.courses.len());
        &self.courses[start..end]
    }
}

// ─── Parseur principal ──────────────────────────────────────────────────────────

/// Parse un `real_skill_config_*.cfg.bin.json` complet en [`RealSkillConfig`].
///
/// Port 1:1 de `parseContent` (inagle `real-skill-config.ts`) : aucune entrée n'est
/// filtrée (6 courbes + 19 tirs conservés tel quel dans le dump 1.03.74.00).
#[must_use]
pub fn parse_real_skill_config(root: &Value) -> RealSkillConfig {
    let courses = list_values(root, "m_RealSkillShootCourseInfoList")
        .map(|values| {
            values
                .iter()
                .map(RealSkillShootCourse::from_value)
                .collect()
        })
        .unwrap_or_default();
    let skills = list_values(root, "m_RealSkillInfoList")
        .map(|values| values.iter().map(RealSkillInfo::from_value).collect())
        .unwrap_or_default();
    RealSkillConfig { courses, skills }
}
