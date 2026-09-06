//! `FormationConfig` — port Rust de `formation_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `/home/ubuntu/niers/data/common/gamedata/formation/formation_config_0.02.16.cfg.bin.json`
//! - 5 listes dans ce fichier :
//!   - `m_SoccerPositionInfoList` — 10 types de position (poids offensif/défensif/ligne centrale).
//!   - `m_SoccerFormPlacementInfoList` — 1 073 slots de placement (11 par formation réelle ;
//!     quelques formations spéciales en fin de liste ont 5-10 slots).
//!   - `m_SoccerFormationInfoList` — 115 formations, chacune référençant sa plage dans la
//!     liste ci-dessus via `placementInfo = [offset, count]`.
//!   - `m_SoccerCurvePointInfoList` — 32 points de courbe Bézier pour les animations de passe.
//!   - `m_SoccerLineCurveInfoList` — 7 définitions de courbes de trajectoire.
//!
//! - Les positions terrain sont encodées en **16 hex-chars = deux f32 little-endian** dans le
//!   JSON (pas de champs `x`/`y` séparés) — validé byte-à-byte sur le dump réel :
//!   GK `defensePos = "000000008FC2753F"` → `Vec2 { x: 0.0, y: 0.9599... }` (centre, profond).
//!
//! ## Intégration possible dans `nie-core/match_sim`
//!
//! `FormationConfig::placements_of(formation)` renvoie la tranche de 11 `SoccerFormPlacementInfo`
//! d'une formation. Ces slots contiennent les positions au coup d'envoi (`start_pos`) et en
//! phases de jeu (`defense_pos` / `offense_pos`), directement utilisables pour initialiser
//! les positions des 11 joueurs dans la simulation de match.
//! **Le port dans `nie-core/match_sim.rs` est hors scope de cette tâche** — ce module expose
//! les données ; la logique d'application reste à faire.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_bool, field_hash, field_i64, field_str, list_values};
use crate::hash::HashId;

// ─── Vec2 ─────────────────────────────────────────────────────────────────────

/// Point 2D encodé en 16 hex-chars (deux `f32` little-endian) dans le JSON `formation_config`.
///
/// Système de coordonnées IEVR d'après les positions observées : `x` = horizontal (symétrie
/// gauche/droite, −1..+1), `y` = profondeur (0 ≈ camp adverse → 1 ≈ propre but, d'après
/// GK `defensePos.y ≈ 0.96`). Les `bustupPos` sont en coordonnées UI distinctes.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    /// Point nul.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Décode un point 2D depuis 16 hex-chars (deux `f32` little-endian).
    ///
    /// Format trouvé dans `defensePos`, `offensePos`, `startPos`, etc. de
    /// `SOCCER_FORM_PLACEMENT_INFO`. Renvoie [`Vec2::ZERO`] si la chaîne n'a pas exactement
    /// 16 caractères ASCII hexadécimaux.
    ///
    /// # Exemple
    ///
    /// ```
    /// use nie_data::formation::Vec2;
    /// // Gardien (positionNo=0) : defensePos réelle du dump formation_config_0.02.16.
    /// let p = Vec2::parse_hex("000000008FC2753F");
    /// assert_eq!(p.x, 0.0_f32); // centre horizontal
    /// assert!((p.y - 0.96_f32).abs() < 1e-3); // profond (≈ 0.9599...)
    /// ```
    #[must_use]
    pub fn parse_hex(s: &str) -> Self {
        let b = s.as_bytes();
        if b.len() != 16 {
            return Self::ZERO;
        }
        Self {
            x: le_f32_from_hex_slice(&b[0..8]),
            y: le_f32_from_hex_slice(&b[8..16]),
        }
    }
}

/// Décode un nibble hexadécimal ASCII (insensible à la casse, 0 si invalide).
#[inline]
fn hex_nibble(b: u8) -> u32 {
    match b {
        b'0'..=b'9' => (b - b'0') as u32,
        b'A'..=b'F' => (b - b'A' + 10) as u32,
        b'a'..=b'f' => (b - b'a' + 10) as u32,
        _ => 0,
    }
}

/// Décode 8 hex-chars (slice de longueur ≥ 8) en `f32` little-endian.
/// Panique si `s.len() < 8` — appelé uniquement depuis `Vec2::parse_hex` qui garantit len=16.
fn le_f32_from_hex_slice(s: &[u8]) -> f32 {
    let b0 = ((hex_nibble(s[0]) << 4) | hex_nibble(s[1])) as u8;
    let b1 = ((hex_nibble(s[2]) << 4) | hex_nibble(s[3])) as u8;
    let b2 = ((hex_nibble(s[4]) << 4) | hex_nibble(s[5])) as u8;
    let b3 = ((hex_nibble(s[6]) << 4) | hex_nibble(s[7])) as u8;
    f32::from_bits(u32::from_le_bytes([b0, b1, b2, b3]))
}

// ─── SOCCER_POSITION_INFO ─────────────────────────────────────────────────────

/// Poids de ligne pour un type de position (`SOCCER_POSITION_INFO`).
///
/// Paramètre la répartition d'un joueur selon sa position en phase offensive ou défensive.
///
/// > **Note** : le champ source s'appelle `centerLineWight` (typo Level-5 — le « h » manque),
/// > conservée à l'identique dans le JSON.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerPositionInfo {
    /// `positionId` — identifiant du type (1=GK, 2..10=positions de champ).
    /// Correspondance complète non confirmée officiellement ; indicatif.
    pub position_id: i64,
    /// `centerLineWight` — poids ligne centrale (0.5 pour tous les types dans le dump réel).
    pub center_line_weight: f32,
    /// `offenseLineWeight`.
    pub offense_line_weight: f32,
    /// `defenseLineWeight`.
    pub defense_line_weight: f32,
}

impl SoccerPositionInfo {
    /// Parse une entrée de `m_SoccerPositionInfoList`. `None` si `positionId` est absent.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            position_id: field_i64(v, "positionId")?,
            center_line_weight: v
                .get("centerLineWight")
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32,
            offense_line_weight: v
                .get("offenseLineWeight")
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32,
            defense_line_weight: v
                .get("defenseLineWeight")
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32,
        })
    }
}

// ─── SOCCER_FORM_PLACEMENT_INFO ───────────────────────────────────────────────

/// Placement d'un joueur dans une formation (`SOCCER_FORM_PLACEMENT_INFO`).
///
/// Toutes les positions terrain sont des [`Vec2`] encodées en 16 hex-chars. Consulter les
/// 11 slots consécutifs d'une formation via [`FormationConfig::placements_of`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerFormPlacementInfo {
    /// `defensePos` — position en phase défensive.
    pub defense_pos: Vec2,
    /// `offensePos` — position en phase offensive.
    pub offense_pos: Vec2,
    /// `startPos` — position au coup d'envoi.
    pub start_pos: Vec2,
    /// `ckDefenseLeftPos` — corner défense, côté gauche.
    pub ck_defense_left_pos: Vec2,
    /// `ckDefenseRightPos` — corner défense, côté droit.
    pub ck_defense_right_pos: Vec2,
    /// `ckOffenseLeftPos` — corner attaque, côté gauche.
    pub ck_offense_left_pos: Vec2,
    /// `ckOffenseRightPos` — corner attaque, côté droit.
    pub ck_offense_right_pos: Vec2,
    /// `pkDefensePos` — position tir au but défense.
    pub pk_defense_pos: Vec2,
    /// `pkOffensePos` — position tir au but attaque.
    pub pk_offense_pos: Vec2,
    /// `bustupPos` — position du portrait du joueur (espace UI, différent du terrain).
    pub bustup_pos: Vec2,
    /// `positionNo` — index du slot dans la formation (0–10 pour une formation à 11 joueurs).
    pub position_no: i64,
    /// `positionId` — type de position (voir [`SoccerPositionInfo::position_id`]).
    pub position_id: i64,
    /// `passNo` — ordre dans la séquence de passes du coup d'envoi.
    pub pass_no: i64,
    /// `bKickoff` — ce joueur frappe le coup d'envoi.
    pub b_kickoff: bool,
    /// `bFollow` — ce joueur suit le buteur du coup d'envoi.
    pub b_follow: bool,
}

impl SoccerFormPlacementInfo {
    /// Parse une entrée de `m_SoccerFormPlacementInfoList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            defense_pos: Vec2::parse_hex(field_str(v, "defensePos").unwrap_or("")),
            offense_pos: Vec2::parse_hex(field_str(v, "offensePos").unwrap_or("")),
            start_pos: Vec2::parse_hex(field_str(v, "startPos").unwrap_or("")),
            ck_defense_left_pos: Vec2::parse_hex(field_str(v, "ckDefenseLeftPos").unwrap_or("")),
            ck_defense_right_pos: Vec2::parse_hex(field_str(v, "ckDefenseRightPos").unwrap_or("")),
            ck_offense_left_pos: Vec2::parse_hex(field_str(v, "ckOffenseLeftPos").unwrap_or("")),
            ck_offense_right_pos: Vec2::parse_hex(field_str(v, "ckOffenseRightPos").unwrap_or("")),
            pk_defense_pos: Vec2::parse_hex(field_str(v, "pkDefensePos").unwrap_or("")),
            pk_offense_pos: Vec2::parse_hex(field_str(v, "pkOffensePos").unwrap_or("")),
            bustup_pos: Vec2::parse_hex(field_str(v, "bustupPos").unwrap_or("")),
            position_no: field_i64(v, "positionNo").unwrap_or(0),
            position_id: field_i64(v, "positionId").unwrap_or(0),
            pass_no: field_i64(v, "passNo").unwrap_or(0),
            b_kickoff: field_bool(v, "bKickoff").unwrap_or(false),
            b_follow: field_bool(v, "bFollow").unwrap_or(false),
        })
    }
}

// ─── SOCCER_FORMATION_INFO ────────────────────────────────────────────────────

/// Définition d'une formation de jeu (`SOCCER_FORMATION_INFO`).
///
/// `placementInfo = [offset, count]` référence la plage de slots dans
/// `m_SoccerFormPlacementInfoList` — utiliser [`FormationConfig::placements_of`] pour la résoudre.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerFormationInfo {
    /// `formId` — hash identifiant unique de la formation.
    pub form_id: HashId,
    /// `placementInfo[0]` — index de début dans `m_SoccerFormPlacementInfoList`.
    pub placement_offset: i64,
    /// `placementInfo[1]` — nombre de slots (11 pour une formation standard).
    pub placement_count: i64,
    /// `powerOffense` — indice offensif (1–5 ; 0 pour les formations internes sans texte).
    pub power_offense: i64,
    /// `powerDefense` — indice défensif (1–5 ; 0 pour les formations internes sans texte).
    pub power_defense: i64,
    /// `nounId` — hash du nom de la formation (jointure texte, peut être zéro).
    pub noun_id: HashId,
    /// `descId` — hash de la description (peut être zéro).
    pub desc_id: HashId,
}

impl SoccerFormationInfo {
    /// Parse une entrée de `m_SoccerFormationInfoList`. `None` si `formId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let form_id = field_hash(v, "formId");
        if form_id.is_zero() {
            return None;
        }
        let pi = v.get("placementInfo").and_then(Value::as_array);
        Some(Self {
            form_id,
            placement_offset: pi
                .and_then(|a| a.first())
                .and_then(Value::as_i64)
                .unwrap_or(0),
            placement_count: pi
                .and_then(|a| a.get(1))
                .and_then(Value::as_i64)
                .unwrap_or(0),
            power_offense: field_i64(v, "powerOffense").unwrap_or(0),
            power_defense: field_i64(v, "powerDefense").unwrap_or(0),
            noun_id: field_hash(v, "nounId"),
            desc_id: field_hash(v, "descId"),
        })
    }
}

// ─── SOCCER_CURVE_POINT_INFO ──────────────────────────────────────────────────

/// Point d'une courbe Bézier pour l'animation d'une passe (`SOCCER_CURVE_POINT_INFO`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerCurvePointInfo {
    /// `rate` — paramètre t de la courbe (typiquement −1.0 à 1.0).
    pub rate: f32,
    /// `point` — valeur de la courbe à ce paramètre.
    pub point: f32,
}

impl SoccerCurvePointInfo {
    /// Parse une entrée de `m_SoccerCurvePointInfoList`.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            rate: v.get("rate").and_then(Value::as_f64).unwrap_or(0.0) as f32,
            point: v.get("point").and_then(Value::as_f64).unwrap_or(0.0) as f32,
        })
    }
}

// ─── SOCCER_LINE_CURVE_INFO ───────────────────────────────────────────────────

/// Courbe de trajectoire de balle (`SOCCER_LINE_CURVE_INFO`).
///
/// `curveInfo = [offset, count]` référence la plage de points dans
/// `m_SoccerCurvePointInfoList` — utiliser [`FormationConfig::curve_points_of`] pour la résoudre.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerLineCurveInfo {
    /// `lineCurveId` — hash identifiant de la courbe.
    pub line_curve_id: HashId,
    /// `curveInfo[0]` — index de début dans `m_SoccerCurvePointInfoList`.
    pub curve_offset: i64,
    /// `curveInfo[1]` — nombre de points dans cette courbe.
    pub curve_count: i64,
}

impl SoccerLineCurveInfo {
    /// Parse une entrée de `m_SoccerLineCurveInfoList`. `None` si `lineCurveId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let line_curve_id = field_hash(v, "lineCurveId");
        if line_curve_id.is_zero() {
            return None;
        }
        let ci = v.get("curveInfo").and_then(Value::as_array);
        Some(Self {
            line_curve_id,
            curve_offset: ci
                .and_then(|a| a.first())
                .and_then(Value::as_i64)
                .unwrap_or(0),
            curve_count: ci
                .and_then(|a| a.get(1))
                .and_then(Value::as_i64)
                .unwrap_or(0),
        })
    }
}

// ─── FormationConfig ──────────────────────────────────────────────────────────

/// Contenu complet d'un `formation_config_*.cfg.bin.json`.
///
/// Utiliser [`parse_formation_config`] pour construire depuis un JSON déjà désérialisé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FormationConfig {
    /// 10 types de position (`m_SoccerPositionInfoList`).
    pub positions: Vec<SoccerPositionInfo>,
    /// 1 073 slots de placement (`m_SoccerFormPlacementInfoList`).
    pub placements: Vec<SoccerFormPlacementInfo>,
    /// 115 formations (`m_SoccerFormationInfoList`).
    pub formations: Vec<SoccerFormationInfo>,
    /// Points de courbe pour les animations de passe (`m_SoccerCurvePointInfoList`).
    pub curve_points: Vec<SoccerCurvePointInfo>,
    /// Courbes de trajectoire (`m_SoccerLineCurveInfoList`).
    pub line_curves: Vec<SoccerLineCurveInfo>,
}

impl FormationConfig {
    /// Renvoie les slots de placement d'une formation (tranche dans `self.placements`).
    ///
    /// Retourne `&[]` si `placement_offset + placement_count` dépasse la taille de la liste.
    /// Utilisation principale : initialiser les positions des 11 joueurs dans `match_sim`.
    #[must_use]
    pub fn placements_of(&self, formation: &SoccerFormationInfo) -> &[SoccerFormPlacementInfo] {
        let start = formation.placement_offset as usize;
        let end = start.saturating_add(formation.placement_count as usize);
        self.placements.get(start..end).unwrap_or(&[])
    }

    /// Renvoie les points d'une courbe de trajectoire (tranche dans `self.curve_points`).
    #[must_use]
    pub fn curve_points_of(&self, curve: &SoccerLineCurveInfo) -> &[SoccerCurvePointInfo] {
        let start = curve.curve_offset as usize;
        let end = start.saturating_add(curve.curve_count as usize);
        self.curve_points.get(start..end).unwrap_or(&[])
    }

    /// Recherche une formation par son `formId`. `None` si absente.
    #[must_use]
    pub fn find_by_id(&self, form_id: HashId) -> Option<&SoccerFormationInfo> {
        self.formations.iter().find(|f| f.form_id == form_id)
    }
}

// ─── Parseur principal ────────────────────────────────────────────────────────

/// Parse un `formation_config_*.cfg.bin.json` complet (5 listes).
///
/// Renvoie un [`FormationConfig`] avec toutes les listes remplies. Les entrées invalides
/// (champs manquants, hash nul) sont silencieusement ignorées.
#[must_use]
pub fn parse_formation_config(root: &Value) -> FormationConfig {
    FormationConfig {
        positions: extract_list(
            root,
            "m_SoccerPositionInfoList",
            SoccerPositionInfo::from_value,
        ),
        placements: extract_list(
            root,
            "m_SoccerFormPlacementInfoList",
            SoccerFormPlacementInfo::from_value,
        ),
        formations: extract_list(
            root,
            "m_SoccerFormationInfoList",
            SoccerFormationInfo::from_value,
        ),
        curve_points: extract_list(
            root,
            "m_SoccerCurvePointInfoList",
            SoccerCurvePointInfo::from_value,
        ),
        line_curves: extract_list(
            root,
            "m_SoccerLineCurveInfoList",
            SoccerLineCurveInfo::from_value,
        ),
    }
}

// NB (jointure texte formation NON portée — 2026-06-15) : inagle `formation-config.ts` résout le
// nom/description via `loadTextFile("formation")` → `formation_text.cfg.bin`. Ce fichier **n'existe
// PAS dans cette version du jeu** (vérifié : 0 candidat dans le VFS). Le `noun_id`/`desc_id` des
// formations se résout donc ailleurs (les formations sont des items — cf. `ItemCategory::Formation`
// — probablement via `item_text`), à confirmer end-to-end avant tout `resolve_*`. Pas de fonction
// non validable ajoutée ici (doctrine anti-faux-FAIT).

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
