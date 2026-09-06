//! `aphrody` — dossier agrégé du personnage **Aphrody / Byron Love**
//! (亜風炉 照美 アフロディ) d'*Inazuma Eleven: Victory Road*.
//!
//! Ce module ne définit pas de nouveau format : il **croise** les parseurs existants
//! ([`nie_data::chara_param`], [`nie_data::skill`], [`nie_data::aura`]) pour produire un dossier
//! navigable d'un personnage précis, identifié par ses 3 `charaBaseId`.
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! Toutes les valeurs « identité » sont des constantes **vérifiées** contre les fichiers réels du
//! jeu. Les parties data-driven (techniques, auras, variantes) sont lues
//! au runtime depuis les dumps `cfg.bin.json` passés à [`build_aphrody_dossier`] — rien n'est codé
//! en dur côté gameplay.
//!
//! Faits vérifiés (sources : `inagle_characters`, `chara_param_1.03.66.00`, `skill_config_5.00.07.00`,
//! `aura_skill_config_1.04.09.00`, textes `data/common/text/{fr,en,ja}`) :
//! - 3 codes / séries : `c01001900`/`0x37D7ACFB` (IE1, primaire), `c05026590`/`0xFC57A11C` (GO),
//!   `c07080010`/`0xCA01BFAB` (Ares) ; élément Forêt (2) ; poste MF (main=3, sub FW=2).
//! - Primaire `chara_param_id=0x9E23A289` : 9 slots = 7 techniques + 2 auras (Lv38).
//! - Techniques : `rho10010`(0x62294D90), `whs00340`(0x1C8CE2B8), `who00200`(0x0D368F6B),
//!   `whs00900`(0x7577A26A), `whs01250`(0xBCE9DEAB), `whs00310`(0x61FB16FD), `whs01080`(0x0AC37488).
//! - Auras : `wap01005`(0xA17C3D72 « Instant Burst »), `wap01001`(0xA611F96B « Burning Overdrive »)
//!   — `skillId1=0` (pas de hissatsu lié), `var8=5` clampé à élément 0.

use std::string::ToString;

use serde_json::Value;

use nie_data::aura::{AuraCmd, parse_all_aura_cmds};
use nie_data::chara_param::{CharaParam, parse_all_chara_params};
use nie_data::hash::HashId;
use nie_data::skill::{CutinAssets, SkillInfo, parse_skill_config};

/// Les 3 séries/codes d'Aphrody (code interne, `charaBaseId`, libellé, sous-dossier visage).
///
/// Vérifié : `inagle_characters` (8 lignes) + index CPK (`_face/<sous-dir>/<code>/<code>.g4*`).
///
/// `face_subdir` en MAJUSCULES : casse réelle du VFS (`niers vfs find "_face/"`), pas une
/// convention libre — un `face_subdir` en minuscules construit une URL byte-différente que le
/// CDN/CPK, insensible à la casse pour le proxy de redimensionnement mais PAS pour le
/// passthrough direct (404 vécu sur `/demo`, 2026-08-15).
pub const SERIES: [AphrodySeriesDef; 3] = [
    AphrodySeriesDef {
        code: "c01001900",
        base_id: HashId(0x37D7_ACFB),
        label: "Inazuma Eleven (IE1)",
        face_subdir: "01_IE1",
    },
    AphrodySeriesDef {
        code: "c05026590",
        base_id: HashId(0xFC57_A11C),
        label: "Inazuma Eleven GO",
        face_subdir: "05_GO2",
    },
    AphrodySeriesDef {
        code: "c07080010",
        base_id: HashId(0xCA01_BFAB),
        label: "Ares",
        face_subdir: "07_ARES",
    },
];

/// `chara_param_id` de la carte **primaire** (IE1, Normal, MF). Source : `inagle_characters`
/// `is_primary=1` + `/typed` (`0x9E23A289`).
pub const PRIMARY_CHARA_PARAM_ID: HashId = HashId(0x9E23_A289);

/// Définition statique d'une série (constante de [`SERIES`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AphrodySeriesDef {
    /// Code interne (`c01001900`).
    pub code: &'static str,
    /// `charaBaseId` correspondant.
    pub base_id: HashId,
    /// Libellé lisible de la série.
    pub label: &'static str,
    /// Sous-dossier des assets visage (`01_ie1`).
    pub face_subdir: &'static str,
}

/// Une ligne de stats (7 attributs IEVR). Source : `inagle_characters.data.stats`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct StatLine {
    pub kick: u16,
    pub control: u16,
    pub technique: u16,
    pub pressure: u16,
    pub physical: u16,
    pub agility: u16,
    pub intelligence: u16,
}

impl StatLine {
    /// Somme des 7 attributs (= « stat_total »).
    #[must_use]
    pub fn total(self) -> u32 {
        u32::from(self.kick)
            + u32::from(self.control)
            + u32::from(self.technique)
            + u32::from(self.pressure)
            + u32::from(self.physical)
            + u32::from(self.agility)
            + u32::from(self.intelligence)
    }
}

/// Stats résolues de la carte primaire (Lv1 / Lv50 / Lv99).
///
/// **Valeurs golden vérifiées** (`inagle_characters.data.stats`, `0x9E23A289`). La dérivation
/// complète depuis les growth tables (`crate::growth::calculate_stats`) nécessiterait les tables
/// `growth_table_config` + la rareté → `charaRank`, non passées ici ; on expose donc les valeurs
/// validées par inagle. TODO(growth) : recalculer via `crate::growth` si les tables sont fournies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AphrodyStats {
    pub lv1: StatLine,
    pub lv50: StatLine,
    pub lv99: StatLine,
}

impl AphrodyStats {
    /// Stats golden de la primaire.
    #[must_use]
    pub const fn primary() -> Self {
        Self {
            lv1: StatLine {
                kick: 13,
                control: 14,
                technique: 12,
                pressure: 10,
                physical: 10,
                agility: 9,
                intelligence: 11,
            },
            lv50: StatLine {
                kick: 100,
                control: 109,
                technique: 105,
                pressure: 88,
                physical: 87,
                agility: 81,
                intelligence: 97,
            },
            lv99: StatLine {
                kick: 160,
                control: 174,
                technique: 169,
                pressure: 141,
                physical: 140,
                agility: 129,
                intelligence: 155,
            },
        }
    }
}

/// Identité d'Aphrody (3 langues). Constantes vérifiées (`chara_text`, `inagle_characters.data`).
///
/// Le `chara_param` ne porte **pas** le nom (résolu via `chara_text` par hash) : ces champs sont
/// des constantes ancrées sur les fichiers texte réels, pas des valeurs inventées.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AphrodyIdentity {
    pub name_fr: &'static str,
    pub name_en: &'static str,
    pub name_ja: &'static str,
    pub nickname_en: &'static str,
    pub nickname_ja: &'static str,
    /// Élément brut (2 = Forêt) — cf. `nie_data::chara_param::element_id_to_names`.
    pub element_id: i64,
    /// Poste principal (3 = MF) — cf. `nie_data::chara_param::position_id_to_code`.
    pub main_position: i64,
    /// Sous-poste (2 = FW).
    pub sub_position: i64,
    pub gender_male: bool,
    pub series_id: HashId,
    pub team_id: HashId,
    pub team_name_en: &'static str,
    pub constellation_en: &'static str,
    pub constellation_fr: &'static str,
    pub zukan_order: u32,
}

impl AphrodyIdentity {
    /// Identité golden.
    #[must_use]
    pub const fn known() -> Self {
        Self {
            name_fr: "Byron Love Aphrody",
            name_en: "Byron Love Aphrody",
            name_ja: "亜風炉 照美 アフロディ",
            nickname_en: "Aphrody",
            nickname_ja: "アフロディ",
            element_id: 2,
            main_position: 3,
            sub_position: 2,
            gender_male: true,
            series_id: HashId(0x62E8_448F),
            team_id: HashId(0x9BAD_3311),
            team_name_en: "Zeus",
            constellation_en: "Inazumis",
            constellation_fr: "Éclaris",
            zukan_order: 166,
        }
    }
}

/// Chemins VFS des assets d'un code (visage, icône, voix, modèle). Préfixés `data/`, directement
/// servables par `nie-model-serve` (`/raw`, `/tex`, `/audio`, `/model-full`). Templates vérifiés
/// live (HTTP 200).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AphrodyAssets {
    /// Code interne (`c01001900`).
    pub code: String,
    /// Modèle du visage : `data/common/chr/_face/<sub>/<code>/<code>.g4md`.
    pub face_g4md: String,
    /// Mesh/material du visage : `…/<code>.g4mg`.
    pub face_g4mg: String,
    /// Texture du visage (dx11) : `data/dx11/chr/_face/<sub>/<code>/<code>.g4tx` (PNG 2048×1024).
    pub face_g4tx: String,
    /// Icône menu : `data/dx11/menu/200_icon/10_icon_chr/face/<code>_l.g4tx` (PNG 256×256).
    pub icon_g4tx: String,
    /// Voix (japonais) : `data/common/sound_asset/ja/<code>.acb` (+ `.awb`). Pas de doublage en/fr.
    pub voice_acb: String,
    /// Banque audio associée : `data/common/sound_asset/ja/<code>.awb`.
    pub voice_awb: String,
    /// Code à passer à `/model-full/<code>.glb` (corps+visage+uniforme assemblés à la volée).
    pub model_full_code: String,
}

impl AphrodyAssets {
    /// Construit les chemins depuis un code + son sous-dossier visage.
    #[must_use]
    pub fn for_code(code: &str, face_subdir: &str) -> Self {
        Self {
            code: code.to_string(),
            face_g4md: format!("data/common/chr/_face/{face_subdir}/{code}/{code}.g4md"),
            face_g4mg: format!("data/common/chr/_face/{face_subdir}/{code}/{code}.g4mg"),
            face_g4tx: format!("data/dx11/chr/_face/{face_subdir}/{code}/{code}.g4tx"),
            icon_g4tx: format!("data/dx11/menu/200_icon/10_icon_chr/face/{code}_l.g4tx"),
            voice_acb: format!("data/common/sound_asset/ja/{code}.acb"),
            voice_awb: format!("data/common/sound_asset/ja/{code}.awb"),
            model_full_code: code.to_string(),
        }
    }
}

/// Une technique apprise par Aphrody, résolue contre le `skill_config` + ses assets de cut-in.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AphrodyTechnique {
    /// Niveau d'apprentissage (slot du `chara_param`).
    pub learn_level: u8,
    /// Entrée complète du `skill_config` (élément, catégorie, puissance, eventID, …).
    pub info: SkillInfo,
    /// Assets de cut-in dérivés (`SkillInfo::cutin_assets`). `None` si pas d'event de cut-in.
    pub cutin: Option<CutinAssets>,
}

/// Une aura équipée par Aphrody, résolue contre l'`aura_skill_config`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AphrodyAura {
    /// Niveau d'apprentissage (slot du `chara_param`, = 38 pour les deux).
    pub learn_level: u8,
    /// Entrée complète de l'`aura_skill_config` (assetCode, sous-type, config, …).
    pub cmd: AuraCmd,
}

/// Une variante de carte d'Aphrody (une ligne `CHARA_PARAM_INFO`), avec techniques/auras séparées.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AphrodyVariant {
    /// `charaParamId` (ID unique de la variante).
    pub chara_param_id: HashId,
    /// `charaBaseId` (un des 3 codes d'Aphrody).
    pub chara_base_id: HashId,
    /// Code interne résolu (`c01001900`…), `None` si base_id inconnu (ne devrait pas arriver).
    pub code: Option<String>,
    /// `true` si c'est la carte primaire.
    pub is_primary: bool,
    /// Élément brut (2 = Forêt).
    pub element: i64,
    /// Poste principal (3 = MF).
    pub main_position: i64,
    /// Sous-poste (2 = FW).
    pub sub_position: i64,
    /// `growthPattern`.
    pub growth_pattern: i64,
    /// Techniques résolues (hash trouvé dans `skill_config`).
    pub techniques: Vec<AphrodyTechnique>,
    /// Auras résolues (hash trouvé dans `aura_skill_config`).
    pub auras: Vec<AphrodyAura>,
    /// Hashes de slot non résolus (ni technique ni aura) — diagnostic, normalement vide.
    pub unresolved: Vec<HashId>,
}

/// Dossier agrégé complet d'Aphrody.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AphrodyDossier {
    /// Identité (constantes vérifiées 3 langues).
    pub identity: AphrodyIdentity,
    /// Stats golden de la primaire (Lv1/50/99).
    pub stats: AphrodyStats,
    /// Les 3 séries/codes.
    pub series: Vec<AphrodySeriesDef>,
    /// Assets par code (3 entrées).
    pub assets: Vec<AphrodyAssets>,
    /// Toutes les variantes de carte trouvées dans le `chara_param` (typiquement 8).
    pub variants: Vec<AphrodyVariant>,
}

impl AphrodyDossier {
    /// Renvoie la variante primaire si présente.
    #[must_use]
    pub fn primary(&self) -> Option<&AphrodyVariant> {
        self.variants.iter().find(|v| v.is_primary)
    }
}

/// Résout le code interne d'un `charaBaseId` parmi les 3 séries d'Aphrody. `None` sinon.
#[must_use]
fn code_for_base(base_id: HashId) -> Option<&'static AphrodySeriesDef> {
    SERIES.iter().find(|s| s.base_id == base_id)
}

/// Construit le **dossier agrégé** d'Aphrody en croisant les trois dumps `cfg.bin.json` déjà
/// désérialisés (`chara_param`, `skill_config`, `aura_skill_config`).
///
/// Logique :
/// 1. parse tous les `CHARA_PARAM_INFO`, ne garde que ceux dont `charaBaseId` ∈ [`SERIES`] ;
/// 2. construit les cartes `skillID → SkillInfo` et `auraId → AuraCmd` ;
/// 3. pour chaque slot d'une variante : si le hash est dans le `skill_config` → technique (+ cut-in),
///    sinon s'il est dans l'`aura_skill_config` → aura, sinon → `unresolved`.
///
/// Aucune donnée gameplay n'est inventée : tout vient des dumps. L'identité/les stats sont des
/// constantes vérifiées (cf. doc-comments).
#[must_use]
pub fn build_aphrody_dossier(
    chara_param_root: &Value,
    skill_root: &Value,
    aura_root: &Value,
) -> AphrodyDossier {
    // Cartes de résolution.
    let skills = parse_skill_config(skill_root);
    let mut skill_pairs: Vec<(HashId, SkillInfo)> =
        skills.into_iter().map(|s| (s.skill_id, s)).collect();
    skill_pairs.sort_by_key(|(h, _)| h.0);

    let auras = parse_all_aura_cmds(aura_root);
    let mut aura_pairs: Vec<(HashId, AuraCmd)> =
        auras.into_iter().map(|a| (a.aura_id, a)).collect();
    aura_pairs.sort_by_key(|(h, _)| h.0);

    let find_skill = |h: HashId| -> Option<&SkillInfo> {
        skill_pairs
            .binary_search_by_key(&h.0, |(k, _)| k.0)
            .ok()
            .map(|i| &skill_pairs[i].1)
    };
    let find_aura = |h: HashId| -> Option<&AuraCmd> {
        aura_pairs
            .binary_search_by_key(&h.0, |(k, _)| k.0)
            .ok()
            .map(|i| &aura_pairs[i].1)
    };

    // Variantes d'Aphrody.
    let all_params = parse_all_chara_params(chara_param_root);
    let mut variants = Vec::new();
    for p in &all_params {
        let Some(series) = code_for_base(p.chara_base_id) else {
            continue;
        };
        variants.push(build_variant(p, series, &find_skill, &find_aura));
    }

    // Assets par code (3 entrées, toujours disponibles).
    let assets = SERIES
        .iter()
        .map(|s| AphrodyAssets::for_code(s.code, s.face_subdir))
        .collect();

    AphrodyDossier {
        identity: AphrodyIdentity::known(),
        stats: AphrodyStats::primary(),
        series: SERIES.to_vec(),
        assets,
        variants,
    }
}

/// Construit une [`AphrodyVariant`] depuis un `CharaParam` et ses cartes de résolution.
fn build_variant<'a, S, A>(
    p: &CharaParam,
    series: &AphrodySeriesDef,
    find_skill: &S,
    find_aura: &A,
) -> AphrodyVariant
where
    S: Fn(HashId) -> Option<&'a SkillInfo>,
    A: Fn(HashId) -> Option<&'a AuraCmd>,
{
    let mut techniques = Vec::new();
    let mut auras = Vec::new();
    let mut unresolved = Vec::new();

    for slot in &p.skills {
        if let Some(info) = find_skill(slot.skill_id) {
            let cutin = info.cutin_assets();
            techniques.push(AphrodyTechnique {
                learn_level: slot.learn_level,
                info: info.clone(),
                cutin,
            });
        } else if let Some(cmd) = find_aura(slot.skill_id) {
            auras.push(AphrodyAura {
                learn_level: slot.learn_level,
                cmd: cmd.clone(),
            });
        } else {
            unresolved.push(slot.skill_id);
        }
    }

    AphrodyVariant {
        chara_param_id: p.chara_param_id,
        chara_base_id: p.chara_base_id,
        code: Some(series.code.to_string()),
        is_primary: p.chara_param_id == PRIMARY_CHARA_PARAM_ID,
        element: p.element,
        main_position: p.main_position,
        sub_position: p.sub_position,
        growth_pattern: p.growth_pattern,
        techniques,
        auras,
        unresolved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_table_coherente() {
        // Les 3 codes/base_ids vérifiés (inagle_characters).
        assert_eq!(SERIES.len(), 3);
        assert_eq!(SERIES[0].base_id, HashId(0x37D7_ACFB));
        assert_eq!(
            code_for_base(HashId(0xFC57_A11C)).unwrap().code,
            "c05026590"
        );
        assert_eq!(
            code_for_base(HashId(0xCA01_BFAB)).unwrap().code,
            "c07080010"
        );
        assert!(code_for_base(HashId(0x0000_0001)).is_none());
    }

    #[test]
    fn stats_total_golden() {
        // Somme Lv99 vérifiée = 1068.
        assert_eq!(AphrodyStats::primary().lv99.total(), 1068);
    }

    #[test]
    fn assets_templates() {
        let a = AphrodyAssets::for_code("c01001900", "01_ie1");
        assert_eq!(
            a.face_g4tx,
            "data/dx11/chr/_face/01_ie1/c01001900/c01001900.g4tx"
        );
        assert_eq!(a.voice_acb, "data/common/sound_asset/ja/c01001900.acb");
        assert_eq!(
            a.icon_g4tx,
            "data/dx11/menu/200_icon/10_icon_chr/face/c01001900_l.g4tx"
        );
    }
}
