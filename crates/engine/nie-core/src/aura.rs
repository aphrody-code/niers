//! Modèle d'aura (Keshin / Soul / Awakening / Mode / Miximax) + résolution
//! native du hissatsu lié.
//!
//! # Source de vérité
//!
//! - Logique : `packages/inagle/src/skills/mapper-aura.ts`
//!   (`parseAuraCmdInfo` L389-469 pour les index de variables, `getElement`
//!   vars[8] L320-329, `determineSubType` L232-293, `resolveAuraHissatsu`
//!   L181-216, `toHex` L221-226).
//! - Données réelles : `common/gamedata/skill/aura_skill_config_1.04.09.00.cfg.bin.json`
//!   — `AURA_CMD_INFO_0` (19 vars) :
//!   `[0]=2037965306, [1]='wks00020', [2]=493403631, [3]=-1653680409, [4]=30,
//!   [5]=60, [6]=260858381, [7]=-1368456794, [8]=3, [12]=-1124324279, [16]=1`.
//! - Golden : aura `wks00020` → subType Keshin, element vars[8]=3 (Feu),
//!   hissatsu résolu depuis vars[6]=`0x0F8C620D` (whs01780).

use crate::skill::{Element, SkillInfo};
use std::collections::HashMap;

/// Sous-type d'aura, classé depuis le préfixe de l'`assetCode`.
///
/// Source : `determineSubType` (mapper-aura.ts L232-293) — identification stricte
/// par préfixe d'`assetCode` (marqueur moteur 100% fiable) en priorité 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AuraSubType {
    /// Esprit guerrier (Keshin) — préfixes `wks`/`wko`/`wkk`/`wkd`/`wkt`/`was`.
    Keshin,
    /// Totem (Soul) — `wks` avec « totem »/« soul » dans le nom.
    Soul,
    /// Éveil (Awakening) — préfixe `awakening`.
    Awakening,
    /// Changement de mode — préfixe `mode_change`.
    ModeChange,
    /// Miximax — préfixe `wm` (ou `mixi`).
    Miximax,
    /// Aura générique (défaut).
    #[default]
    Aura,
}

impl AuraSubType {
    /// Libellé FR (identique à `getSubTypeLabel`, mapper-aura.ts L298-314).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Keshin => "Esprit Guerrier",
            Self::Soul => "Totem",
            Self::Awakening => "Éveil",
            Self::ModeChange => "Mode",
            Self::Miximax => "Miximax",
            Self::Aura => "Aura",
        }
    }
}

/// Hissatsu (technique spéciale) débloqué par une aura, résolu nativement.
///
/// Source : `resolveAuraHissatsu` (mapper-aura.ts L181-216). `None` si
/// `skillId1`/`skillId2` est nul ou non résoluble — PAS d'invention.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuraHissatsu {
    /// Hash hex de la technique (= `config.skillId1`).
    pub skill_id: String,
    /// Code interne (ex. `whs01780`).
    pub skill_id_str: String,
    /// Catégorie de la technique.
    pub category: crate::skill::Category,
    /// Élément de la technique.
    pub element: Element,
    /// Plage de puissance `(min, max)`.
    pub power: (u16, u16),
}

/// Commande d'aura parsée depuis un nœud `AURA_CMD_INFO`.
///
/// Layout de variables vérifié sur `AURA_CMD_INFO_0` (mapper-aura.ts
/// `parseAuraCmdInfo`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuraCmd {
    /// vars[0] — hash de l'aura (hex normalisé).
    pub aura_id: String,
    /// vars[1] — code d'asset (ex. `wks00020`).
    pub asset_code: String,
    /// vars[2] — hash du nom (hex).
    pub name_id: String,
    /// vars[3] — hash de la description (hex).
    pub desc_id: String,
    /// vars[4] — valeur de buff/durée 1.
    pub val4: i32,
    /// vars[5] — valeur de buff/durée 2.
    pub val5: i32,
    /// vars[6] — hash de la super technique 1 (`skillId1`), `None` si nul.
    pub skill_id1: Option<String>,
    /// vars[7] — hash de la super technique 2 (`skillId2`), `None` si nul.
    pub skill_id2: Option<String>,
    /// vars[8] — élément (0-4).
    pub element: Element,
    /// vars[12] — hash de buff passif (`buffId`), `None` si nul.
    pub buff_id: Option<String>,
    /// vars[16] — rang/index.
    pub rank: i32,
    /// Sous-type classé depuis l'`assetCode`.
    pub sub_type: AuraSubType,
}

/// Convertit un i32 signé en hex `0x` + MAJ 8 chiffres (parité `toHex`).
///
/// Source : `toHex` (mapper-aura.ts L221-226) — valeurs négatives reconverties
/// non-signées sur 32 bits.
#[must_use]
pub fn to_hex(value: i32) -> String {
    // Réinterprétation 2's-complement voulue (parité avec `value >>> 0` du TS).
    format!("0x{:08X}", value.cast_unsigned())
}

/// `None` si le hash résout à 0 (`0x00000000`), sinon le hex normalisé.
///
/// Source : `getHash` (mapper-aura.ts L428-433).
fn hash_or_none(value: i32) -> Option<String> {
    if value == 0 {
        return None;
    }
    let h = to_hex(value);
    if h == "0x00000000" { None } else { Some(h) }
}

/// Élément depuis vars[8] (clampé 0-4, sinon Néant).
///
/// Source : `getElement` (mapper-aura.ts L320-329) — `val >= 0 && val <= 4`.
fn element_from_var8(v8: i32) -> Element {
    if (0..=4).contains(&v8) {
        Element::from_code(v8)
    } else {
        Element::Neant
    }
}

/// Classe le sous-type depuis le préfixe d'`assetCode` (priorité 1, fiable).
///
/// Source : `determineSubType` (mapper-aura.ts L232-293), partie « STRICT ASSET
/// CODE IDENTIFICATION ». La distinction Soul vs Keshin pour `wks*` dépend du
/// nom (« totem »/« soul ») ; sans nom on retombe sur Keshin (parité TS).
#[must_use]
pub fn classify_sub_type(asset_code: &str, name_fr: Option<&str>) -> AuraSubType {
    let asset = asset_code.to_ascii_lowercase();
    let name = name_fr.unwrap_or("").to_ascii_lowercase();

    if asset.starts_with("wks") {
        if name.contains("totem") || name.contains("soul") {
            return AuraSubType::Soul;
        }
        return AuraSubType::Keshin;
    }
    if asset.starts_with("awakening") {
        return AuraSubType::Awakening;
    }
    if asset.starts_with("mode_change") {
        return AuraSubType::ModeChange;
    }
    if asset.starts_with("wm") || asset.contains("mixi") {
        return AuraSubType::Miximax;
    }
    if asset.starts_with("wko")
        || asset.starts_with("wkk")
        || asset.starts_with("wkd")
        || asset.starts_with("wkt")
        || asset.starts_with("was")
    {
        return AuraSubType::Keshin;
    }

    AuraSubType::Aura
}

/// Une variable de config (`type`, `value` brut). Reproduit `ConfigVariable`.
#[derive(Debug, Clone)]
pub struct ConfigVar {
    /// Type Level-5 (`Int`, `String`, …).
    pub ty: String,
    /// Valeur brute sous forme de chaîne (décimal pour `Int`).
    pub value: String,
}

impl ConfigVar {
    /// Lit la variable comme entier signé (parité `Number.parseInt`).
    fn as_int(&self) -> Option<i32> {
        if self.ty == "Int" {
            self.value.parse::<i32>().ok()
        } else {
            None
        }
    }
}

impl AuraCmd {
    /// Parse un nœud `AURA_CMD_INFO` depuis ses variables ordonnées.
    ///
    /// Reproduit `parseAuraCmdInfo` (mapper-aura.ts L389-469) pour les index de
    /// variables. Le nom (pour la distinction Soul) n'est pas résolu ici (pas de
    /// `skill_text` embarqué) — passez-le via `name_fr` si disponible.
    ///
    /// Retourne `None` si moins de 4 variables (parité `vars.length < 4`).
    #[must_use]
    pub fn parse(vars: &[ConfigVar], name_fr: Option<&str>) -> Option<Self> {
        if vars.len() < 4 {
            return None;
        }

        let get_int = |i: usize| vars.get(i).and_then(ConfigVar::as_int);

        let aura_id_int = get_int(0)?;
        let asset_code = vars.get(1).map(|v| v.value.clone()).unwrap_or_default();
        let name_id_int = get_int(2).unwrap_or(0);
        let desc_id_int = get_int(3).unwrap_or(0);

        let sub_type = classify_sub_type(&asset_code, name_fr);
        let element = element_from_var8(get_int(8).unwrap_or(0));

        Some(Self {
            aura_id: to_hex(aura_id_int),
            asset_code,
            name_id: to_hex(name_id_int),
            desc_id: to_hex(desc_id_int),
            val4: get_int(4).unwrap_or(0),
            val5: get_int(5).unwrap_or(0),
            skill_id1: get_int(6).and_then(hash_or_none),
            skill_id2: get_int(7).and_then(hash_or_none),
            element,
            buff_id: get_int(12).and_then(hash_or_none),
            rank: get_int(16).unwrap_or(0),
            sub_type,
        })
    }

    /// Résout le hissatsu natif de l'aura via `skillId1` (fallback `skillId2`).
    ///
    /// Reproduit `resolveAuraHissatsu` (mapper-aura.ts L181-216) : essaie
    /// `skillId1` puis `skillId2`, ignore les `0x00000000`. Retourne `None` si
    /// aucun n'est résoluble dans `skill_map` — AUCUNE invention de hissatsu.
    #[must_use]
    pub fn resolve_hissatsu(&self, skill_map: &HashMap<String, SkillInfo>) -> Option<AuraHissatsu> {
        let candidates = [self.skill_id1.as_deref(), self.skill_id2.as_deref()];
        for cand in candidates.into_iter().flatten() {
            if cand == "0x00000000" {
                continue;
            }
            if let Some(skill) = skill_map.get(cand) {
                return Some(AuraHissatsu {
                    skill_id: skill.skill_id.clone(),
                    skill_id_str: skill.skill_id_str.clone(),
                    category: skill.category,
                    element: skill.element,
                    power: skill.power_range(),
                });
            }
        }
        None
    }
}

#[cfg(feature = "data")]
mod loader {
    //! Chargement de la fixture d'aura golden (`data/aura_fixture.json`).

    use super::ConfigVar;

    /// JSON curé d'une aura réelle (wks00020), embarqué à la compilation.
    const AURA_JSON: &str = include_str!("../data/aura_fixture.json");

    #[derive(serde::Deserialize)]
    struct RawFixture {
        #[allow(dead_code)]
        name: String,
        /// Liste de paires [type, value].
        vars: Vec<(String, String)>,
    }

    /// Charge les variables brutes de la fixture d'aura (wks00020).
    ///
    /// # Panics
    ///
    /// Panique si le JSON embarqué est corrompu (figé à la compilation).
    #[must_use]
    pub fn load_aura_fixture_vars() -> Vec<ConfigVar> {
        let raw: RawFixture =
            serde_json::from_str(AURA_JSON).expect("aura_fixture.json embarqué valide");
        raw.vars
            .into_iter()
            .map(|(ty, value)| ConfigVar { ty, value })
            .collect()
    }
}

#[cfg(feature = "data")]
pub use loader::load_aura_fixture_vars;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_hex_signed_and_unsigned() {
        // Vérifié contre toHex de mapper-aura.ts.
        assert_eq!(to_hex(2037965306), "0x7978E1FA"); // auraId wks00020
        assert_eq!(to_hex(493403631), "0x1D68BDEF"); // nameId
        assert_eq!(to_hex(-1653680409), "0x9D6ED6E7"); // descId (négatif)
        assert_eq!(to_hex(260858381), "0x0F8C620D"); // skillId1 → whs01780
        assert_eq!(to_hex(-1368456794), "0xAE6F01A6"); // skillId2 (négatif)
        assert_eq!(to_hex(-1124324279), "0xBCFC2C49"); // buffId (négatif)
        assert_eq!(to_hex(0), "0x00000000");
    }

    #[test]
    fn classify_keshin_from_wks() {
        // wks sans « totem »/« soul » → Keshin (parité TS).
        assert_eq!(classify_sub_type("wks00020", None), AuraSubType::Keshin);
        // wks avec « totem » dans le nom → Soul.
        assert_eq!(
            classify_sub_type("wks00240", Some("Totem de Arthur")),
            AuraSubType::Soul
        );
    }

    #[test]
    fn classify_other_prefixes() {
        assert_eq!(
            classify_sub_type("awakening_01", None),
            AuraSubType::Awakening
        );
        assert_eq!(
            classify_sub_type("mode_change_x", None),
            AuraSubType::ModeChange
        );
        assert_eq!(classify_sub_type("wmm00210_h2", None), AuraSubType::Miximax);
        assert_eq!(classify_sub_type("wko00100", None), AuraSubType::Keshin);
        assert_eq!(classify_sub_type("unknown99", None), AuraSubType::Aura);
    }
}

#[cfg(all(test, feature = "data"))]
mod golden_tests {
    use super::*;
    use crate::skill::{Category, load_skill_fixture};

    /// Golden : parse complet de l'aura réelle wks00020.
    #[test]
    fn golden_parse_wks00020() {
        let vars = load_aura_fixture_vars();
        assert_eq!(vars.len(), 19, "AURA_CMD_INFO_0 a 19 variables");
        let aura = AuraCmd::parse(&vars, None).expect("aura parsée");

        assert_eq!(aura.asset_code, "wks00020");
        assert_eq!(aura.aura_id, "0x7978E1FA");
        assert_eq!(aura.name_id, "0x1D68BDEF");
        assert_eq!(aura.desc_id, "0x9D6ED6E7");
        assert_eq!(aura.val4, 30);
        assert_eq!(aura.val5, 60);
        // vars[8] = 3 → Feu.
        assert_eq!(aura.element, Element::Feu);
        // skillId1 = vars[6] = 0x0F8C620D.
        assert_eq!(aura.skill_id1.as_deref(), Some("0x0F8C620D"));
        assert_eq!(aura.skill_id2.as_deref(), Some("0xAE6F01A6"));
        assert_eq!(aura.buff_id.as_deref(), Some("0xBCFC2C49"));
        assert_eq!(aura.rank, 1);
        // wks sans nom → Keshin.
        assert_eq!(aura.sub_type, AuraSubType::Keshin);
    }

    /// Golden : résolution du hissatsu lié (vars[6] → whs01780).
    #[test]
    fn golden_resolve_hissatsu_wks00020() {
        let vars = load_aura_fixture_vars();
        let aura = AuraCmd::parse(&vars, None).expect("aura parsée");
        let skills = load_skill_fixture();

        let hissatsu = aura
            .resolve_hissatsu(&skills)
            .expect("hissatsu résoluble depuis skillId1");
        assert_eq!(hissatsu.skill_id, "0x0F8C620D");
        assert_eq!(hissatsu.skill_id_str, "whs01780");
        assert_eq!(hissatsu.category, Category::Tir);
        assert_eq!(hissatsu.element, Element::Feu);
        assert_eq!(hissatsu.power, (100, 640));
    }

    /// ANTI-HALLUCINATION : skillId non résoluble → pas de hissatsu inventé.
    #[test]
    fn unresolvable_skill_returns_none() {
        let vars = load_aura_fixture_vars();
        let aura = AuraCmd::parse(&vars, None).expect("aura parsée");
        // Map vide → aucun skill résoluble → None (pas d'invention).
        let empty = std::collections::HashMap::new();
        assert!(aura.resolve_hissatsu(&empty).is_none());
    }
}
