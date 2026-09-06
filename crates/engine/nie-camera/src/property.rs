//! `camera_ctrl_property_info*.cfg.bin` — les **presets de contrôleur** et leurs paramètres.
//!
//! Un fichier par contexte (`` = défaut, `_photo`, `_rpg_battle`, `_craft_edit`,
//! `_screenshot_mode`), chacun décrivant des presets nommés d'après les classes C++ :
//!
//! ```text
//! PROP_INFO_BGN = ["CCameraCtrlChase_Soccer", "CCameraCtrlChase"]   <- preset, classe parente
//!   PROP_PARAM  = ["m_fCamLength", 16.0]
//!   PROP_PARAM  = ["m_vCameraRefOffset", 0.0, 1.0, 0.0]
//! ```
//!
//! Le 2ᵉ champ du `PROP_INFO_BGN` est le **parent** : les paramètres absents d'un preset sont
//! hérités de lui. [`PropertySet::resolve`] applique cette résolution, c'est elle qui donne les
//! valeurs réellement utilisées par le jeu.
//!
//! Ces presets alimentent directement [`crate::ctrl`] : `CCameraCtrlChase_Soccer` fournit par
//! exemple `m_fCamLength = 16`, `m_fInterpRate = 0.2`, `m_fRotMinX = -20`, `m_fRotMaxX = 45`.

use std::collections::BTreeMap;

use nie_formats::cfgbin::{self, CfgEntry, Value};

use crate::{CameraError, Result};

/// Valeur d'un paramètre de preset.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    /// Scalaire entier (`m_priority`, `m_FadeType`…).
    Int(i32),
    /// Scalaire flottant.
    Float(f32),
    /// Vecteur (`m_vCameraRefOffset`, `m_vLocalPosOffset`…).
    Vec3([f32; 3]),
    /// Chaîne.
    Text(String),
}

impl ParamValue {
    /// Valeur en `f32` si c'est un scalaire numérique.
    #[must_use]
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            ParamValue::Float(f) => Some(*f),
            #[expect(
                clippy::cast_precision_loss,
                reason = "les entiers de config sont de petites valeurs (priorités, types)"
            )]
            ParamValue::Int(i) => Some(*i as f32),
            _ => None,
        }
    }

    /// Valeur en `i32` si c'est un entier.
    #[must_use]
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            ParamValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Vecteur si c'en est un.
    #[must_use]
    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        match self {
            ParamValue::Vec3(v) => Some(*v),
            _ => None,
        }
    }
}

/// Un preset de contrôleur : nom, parent éventuel, paramètres **déclarés** (hors héritage).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Preset {
    /// Nom du preset (= nom de classe C++, éventuellement suffixé : `CCameraCtrlChase_Soccer`).
    pub name: String,
    /// Preset parent dont hériter, s'il y en a un.
    pub parent: Option<String>,
    /// Paramètres déclarés directement sur ce preset.
    pub params: BTreeMap<String, ParamValue>,
}

/// Tous les presets d'un fichier `camera_ctrl_property_info*`.
#[derive(Debug, Clone, Default)]
pub struct PropertySet {
    /// Presets indexés par nom, dans l'ordre alphabétique.
    pub presets: BTreeMap<String, Preset>,
}

fn to_param(vars: &[Value]) -> Option<(String, ParamValue)> {
    let Some(Value::String(name)) = vars.first() else {
        return None;
    };
    let rest = &vars[1..];
    let value = match rest {
        [Value::Int(i)] => ParamValue::Int(*i),
        [Value::Float(f)] => ParamValue::Float(*f),
        [Value::Float(x), Value::Float(y), Value::Float(z)] => ParamValue::Vec3([*x, *y, *z]),
        [Value::String(s)] => ParamValue::Text(s.clone()),
        _ => return None,
    };
    Some((name.clone(), value))
}

fn collect_params(entry: &CfgEntry, out: &mut BTreeMap<String, ParamValue>) {
    for child in &entry.children {
        if (child.name == "PROP_PARAM" || child.name == "INFO_PARAM")
            && let Some((k, v)) = to_param(&child.variables)
        {
            out.insert(k, v);
        }
        // `INFO_PARAM_BGN` regroupe des `INFO_PARAM` d'un cran plus bas ; on ne descend pas dans
        // un `PROP_INFO_BGN` imbriqué, ses paramètres appartiennent à ce preset-là.
        if !child.children.is_empty() && child.name != "PROP_INFO_BGN" {
            collect_params(child, out);
        }
    }
}

/// Collecte les presets, y compris ceux imbriqués dans un bloc englobant.
fn collect_presets(entries: &[CfgEntry], out: &mut BTreeMap<String, Preset>) {
    for entry in entries {
        if entry.name == "PROP_INFO_BGN"
            && let Some(Value::String(name)) = entry.variables.first()
        {
            let parent = match entry.variables.get(1) {
                Some(Value::String(p)) if !p.is_empty() && p != name => Some(p.clone()),
                _ => None,
            };
            let mut params = BTreeMap::new();
            collect_params(entry, &mut params);
            out.insert(
                name.clone(),
                Preset {
                    name: name.clone(),
                    parent,
                    params,
                },
            );
        }
        if !entry.children.is_empty() {
            collect_presets(&entry.children, out);
        }
    }
}

impl PropertySet {
    /// Décode un `camera_ctrl_property_info*.cfg.bin` (format T2B).
    ///
    /// # Errors
    /// [`CameraError::Format`] si le conteneur T2B est illisible, [`CameraError::Malformed`]
    /// s'il ne contient aucun preset.
    pub fn parse(data: &[u8]) -> Result<PropertySet> {
        let file = cfgbin::cfgbin_parse(data)?;
        PropertySet::from_entries(&file.entries)
    }

    /// Construit l'ensemble depuis des entrées T2B déjà décodées.
    ///
    /// Les blocs `PROP_INFO_BGN` sont cherchés **récursivement** : selon le fichier, ils sont au
    /// niveau racine ou imbriqués dans un bloc englobant.
    ///
    /// # Errors
    /// [`CameraError::Malformed`] si aucun `PROP_INFO_BGN` n'est trouvé.
    pub fn from_entries(entries: &[CfgEntry]) -> Result<PropertySet> {
        let mut presets = BTreeMap::new();
        collect_presets(entries, &mut presets);
        if presets.is_empty() {
            return Err(CameraError::Malformed(
                "aucun PROP_INFO_BGN : ce n'est pas un camera_ctrl_property_info".to_string(),
            ));
        }
        Ok(PropertySet { presets })
    }

    /// Paramètres **effectifs** d'un preset : les siens, complétés par ceux de ses ancêtres.
    ///
    /// Un paramètre déclaré localement l'emporte toujours sur celui du parent. La chaîne
    /// d'héritage est suivie jusqu'à la racine ; un cycle est interrompu sans erreur (chaque
    /// preset n'est visité qu'une fois).
    #[must_use]
    pub fn resolve(&self, name: &str) -> BTreeMap<String, ParamValue> {
        let mut out = BTreeMap::new();
        let mut seen: Vec<&str> = Vec::new();
        let mut cur = self.presets.get(name);
        while let Some(p) = cur {
            if seen.contains(&p.name.as_str()) {
                break;
            }
            seen.push(&p.name);
            for (k, v) in &p.params {
                out.entry(k.clone()).or_insert_with(|| v.clone());
            }
            cur = p.parent.as_deref().and_then(|par| self.presets.get(par));
        }
        out
    }

    /// Valeur `f32` effective d'un paramètre, héritage compris.
    #[must_use]
    pub fn f32_of(&self, preset: &str, param: &str) -> Option<f32> {
        self.resolve(preset).get(param).and_then(ParamValue::as_f32)
    }

    /// Noms des presets, triés.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.presets.keys().map(String::as_str)
    }
}

/// Un fichier `*_property.cfg.bin` à plat (`SoccerCameraProperty`, `SoccerCameraInterpProperty`).
///
/// Même conteneur T2B, mais un seul bloc `PROP_INFO_BGN` suivi de `PROP_PARAM` au **niveau
/// racine** (pas d'imbrication) : `m_ChaseRate`, `m_ChaseLength`, `m_InterpTime`, `m_FadeType`…
#[derive(Debug, Clone, Default)]
pub struct FlatProperty {
    /// Nom déclaré par le `PROP_INFO_BGN` (ex. `SoccerCameraProperty`).
    pub name: String,
    /// Paramètres.
    pub params: BTreeMap<String, ParamValue>,
}

impl FlatProperty {
    /// Décode un `*_property.cfg.bin` à plat.
    ///
    /// # Errors
    /// [`CameraError::Format`] si le T2B est illisible, [`CameraError::Malformed`] s'il n'y a
    /// pas de bloc `PROP_INFO_BGN`.
    pub fn parse(data: &[u8]) -> Result<FlatProperty> {
        let file = cfgbin::cfgbin_parse(data)?;
        FlatProperty::from_entries(&file.entries)
    }

    /// Construit la propriété depuis des entrées T2B déjà décodées.
    ///
    /// # Errors
    /// [`CameraError::Malformed`] s'il n'y a pas de bloc `PROP_INFO_BGN`.
    pub fn from_entries(entries: &[CfgEntry]) -> Result<FlatProperty> {
        let mut out = FlatProperty::default();
        let mut seen_header = false;
        for entry in entries {
            match entry.name.as_str() {
                "PROP_INFO_BGN" => {
                    if let Some(Value::String(n)) = entry.variables.first() {
                        out.name.clone_from(n);
                    }
                    seen_header = true;
                    collect_params(entry, &mut out.params);
                }
                "PROP_PARAM" | "INFO_PARAM" => {
                    if let Some((k, v)) = to_param(&entry.variables) {
                        out.params.insert(k, v);
                    }
                }
                _ => {}
            }
        }
        if !seen_header {
            return Err(CameraError::Malformed(
                "aucun PROP_INFO_BGN dans ce fichier de propriétés".to_string(),
            ));
        }
        Ok(out)
    }

    /// Valeur `f32` d'un paramètre.
    #[must_use]
    pub fn f32_of(&self, param: &str) -> Option<f32> {
        self.params.get(param).and_then(ParamValue::as_f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_formats::cfgbin::{CfgEntry, Value};

    fn param(name: &str, vals: Vec<Value>) -> CfgEntry {
        let mut variables = vec![Value::String(name.to_string())];
        variables.extend(vals);
        CfgEntry {
            name: "PROP_PARAM".to_string(),
            variables,
            children: Vec::new(),
        }
    }

    fn fixture() -> Vec<CfgEntry> {
        let base = CfgEntry {
            name: "PROP_INFO_BGN".to_string(),
            variables: vec![Value::String("CCameraCtrlChase".to_string())],
            children: vec![
                param("m_fCamLength", vec![Value::Float(7.0)]),
                param("m_fInterpRate", vec![Value::Float(0.5)]),
                param("m_priority", vec![Value::Int(10)]),
                param(
                    "m_vCameraRefOffset",
                    vec![Value::Float(0.0), Value::Float(1.0), Value::Float(0.0)],
                ),
            ],
        };
        let soccer = CfgEntry {
            name: "PROP_INFO_BGN".to_string(),
            variables: vec![
                Value::String("CCameraCtrlChase_Soccer".to_string()),
                Value::String("CCameraCtrlChase".to_string()),
            ],
            children: vec![
                param("m_fCamLength", vec![Value::Float(16.0)]),
                param("m_fInterpRate", vec![Value::Float(0.2)]),
            ],
        };
        vec![base, soccer]
    }

    #[test]
    fn heritage_resolu() {
        let set = PropertySet::from_entries(&fixture()).expect("parse");
        assert_eq!(
            set.names().collect::<Vec<_>>(),
            ["CCameraCtrlChase", "CCameraCtrlChase_Soccer"]
        );

        // Local prioritaire sur l'hérité.
        assert_eq!(
            set.f32_of("CCameraCtrlChase_Soccer", "m_fCamLength"),
            Some(16.0)
        );
        assert_eq!(
            set.f32_of("CCameraCtrlChase_Soccer", "m_fInterpRate"),
            Some(0.2)
        );
        // Hérité du parent.
        assert_eq!(
            set.f32_of("CCameraCtrlChase_Soccer", "m_priority"),
            Some(10.0)
        );
        assert_eq!(
            set.resolve("CCameraCtrlChase_Soccer")
                .get("m_vCameraRefOffset"),
            Some(&ParamValue::Vec3([0.0, 1.0, 0.0]))
        );
        // Inconnu.
        assert_eq!(set.f32_of("CCameraCtrlChase_Soccer", "m_inexistant"), None);
        assert!(set.resolve("PresetInconnu").is_empty());
    }

    #[test]
    fn cycle_heritage_ne_boucle_pas() {
        let a = CfgEntry {
            name: "PROP_INFO_BGN".to_string(),
            variables: vec![
                Value::String("A".to_string()),
                Value::String("B".to_string()),
            ],
            children: vec![param("x", vec![Value::Float(1.0)])],
        };
        let b = CfgEntry {
            name: "PROP_INFO_BGN".to_string(),
            variables: vec![
                Value::String("B".to_string()),
                Value::String("A".to_string()),
            ],
            children: vec![param("y", vec![Value::Float(2.0)])],
        };
        let set = PropertySet::from_entries(&[a, b]).expect("parse");
        let r = set.resolve("A");
        assert_eq!(
            r.len(),
            2,
            "les deux presets du cycle sont fusionnés une seule fois"
        );
    }

    #[test]
    fn refuse_un_fichier_sans_preset() {
        let vide = [CfgEntry {
            name: "AUTRE_CHOSE".to_string(),
            variables: vec![Value::Int(1)],
            children: Vec::new(),
        }];
        assert!(matches!(
            PropertySet::from_entries(&vide),
            Err(CameraError::Malformed(_))
        ));
    }

    #[test]
    fn propriete_a_plat() {
        let e = CfgEntry {
            name: "PROP_INFO_BGN".to_string(),
            variables: vec![Value::String("SoccerCameraProperty".to_string())],
            children: Vec::new(),
        };
        let p1 = param("m_ChaseRate", vec![Value::Float(1.0)]);
        let p2 = param("m_ChaseLength", vec![Value::Float(20.0)]);
        let fp = FlatProperty::from_entries(&[e, p1, p2]).expect("parse");
        assert_eq!(fp.name, "SoccerCameraProperty");
        assert_eq!(fp.f32_of("m_ChaseLength"), Some(20.0));
    }
}
