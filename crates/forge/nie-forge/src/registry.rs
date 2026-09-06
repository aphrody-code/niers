//! Registre de correspondance : quelle unité du binaire est fournie par du Rust.
//!
//! Le registre est **commité** (`forge/registry.json`) — c'est l'état de conquête
//! du binaire, la seule source de vérité sur « ce que le workspace produit
//! réellement ». Il ne contient aucun octet du jeu, seulement des adresses, des
//! chemins de symboles Rust et des preuves.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Degré de correspondance d'une unité.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    /// Portage identifié, pas encore validé.
    Wip,
    /// Comportement validé byte-exact par l'oracle (uemu), codegen non conforme.
    Semantic,
    /// Codegen rustc identique aux octets originaux (hors champs relogés).
    Bytes,
}

impl MatchStatus {
    /// Libellé court.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Wip => "wip",
            Self::Semantic => "semantic",
            Self::Bytes => "bytes",
        }
    }
}

/// Preuve attachée à une entrée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proof {
    /// Nature de la preuve (`uemu`, `golden`, `manual`…).
    pub kind: String,
    /// Référence vérifiable (script, test, document).
    pub reference: String,
}

/// Une fonction du binaire et son implémentation Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Adresse virtuelle du début de la fonction dans `nie.exe`.
    pub va: String,
    /// Chemin Rust de l'implémentation, quand elle est identifiée.
    ///
    /// `None` est un état légitime et fréquent : la fonction est validée
    /// sémantiquement par l'oracle, le symbole Rust reste à relier. Mieux vaut
    /// un trou explicite qu'un chemin inventé.
    #[serde(default)]
    pub rust: Option<String>,
    /// Statut de correspondance.
    pub status: MatchStatus,
    /// Preuve associée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<Proof>,
    /// Objet COFF fournissant le codegen à comparer (relatif à la racine du repo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// Symbole à extraire de cet objet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Note libre (limites connues, dépendances runtime…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl RegistryEntry {
    /// Adresse virtuelle décodée.
    ///
    /// # Erreurs
    /// Retourne une erreur si `va` n'est pas un entier hexadécimal `0x…`.
    pub fn va_value(&self) -> anyhow::Result<u64> {
        let s = self
            .va
            .trim()
            .trim_start_matches("0x")
            .trim_start_matches("0X");
        u64::from_str_radix(s, 16)
            .map_err(|e| anyhow::anyhow!("adresse invalide `{}` : {e}", self.va))
    }

    /// Identifiant d'unité correspondant dans le recouvrement.
    ///
    /// # Erreurs
    /// Retourne une erreur si l'adresse est invalide.
    pub fn unit_id(&self) -> anyhow::Result<String> {
        Ok(format!("fn.{:x}", self.va_value()?))
    }
}

/// Registre complet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    /// Version du schéma.
    pub version: u32,
    /// Binaire cible (sha256), pour détecter un changement de build du jeu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_sha256: Option<String>,
    /// Entrées, ordonnées par adresse.
    pub entries: Vec<RegistryEntry>,
}

impl Registry {
    /// Charge un registre depuis un fichier JSON.
    ///
    /// # Erreurs
    /// Retourne une erreur si le fichier est illisible ou mal formé.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("lecture de {} : {e}", path.display()))?;
        let r: Self = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("registre {} invalide : {e}", path.display()))?;
        Ok(r)
    }

    /// Écrit le registre (JSON indenté, ordre stable).
    ///
    /// # Erreurs
    /// Retourne une erreur d'écriture disque ou de sérialisation.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut sorted = self.clone();
        sorted
            .entries
            .sort_by_key(|e| e.va_value().unwrap_or(u64::MAX));
        std::fs::write(path, serde_json::to_string_pretty(&sorted)? + "\n")?;
        Ok(())
    }

    /// Index `adresse virtuelle → entrée`.
    ///
    /// Indexer par ADRESSE et non par identifiant d'unité : une fonction portée
    /// peut tomber sur une unité `fn.…` (bornée par `.pdata`) comme sur un
    /// résidu `res.…` (feuille sans information d'unwind). Synthétiser un
    /// `fn.<va>` ferait silencieusement disparaître tout le second cas.
    ///
    /// # Erreurs
    /// Retourne une erreur si une adresse du registre est invalide.
    pub fn by_va(&self) -> anyhow::Result<BTreeMap<u64, &RegistryEntry>> {
        let mut m = BTreeMap::new();
        for e in &self.entries {
            m.insert(e.va_value()?, e);
        }
        Ok(m)
    }

    /// Compte les entrées d'un statut donné.
    #[must_use]
    pub fn count(&self, status: MatchStatus) -> usize {
        self.entries.iter().filter(|e| e.status == status).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresse_et_identifiant_d_unite() {
        let e = RegistryEntry {
            va: "0x141334600".into(),
            rust: Some("nie_core::ball::ParabolaMove::step".into()),
            status: MatchStatus::Semantic,
            proof: None,
            object: None,
            symbol: None,
            note: None,
        };
        assert_eq!(e.va_value().unwrap(), 0x1_4133_4600);
        assert_eq!(e.unit_id().unwrap(), "fn.141334600");
    }

    #[test]
    fn adresse_invalide_est_rejetee() {
        let e = RegistryEntry {
            va: "pas-une-adresse".into(),
            rust: Some("x".into()),
            status: MatchStatus::Wip,
            proof: None,
            object: None,
            symbol: None,
            note: None,
        };
        assert!(e.va_value().is_err());
    }

    #[test]
    fn aller_retour_json() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("registry.json");
        let r = Registry {
            version: 1,
            target_sha256: Some("abc".into()),
            entries: vec![
                RegistryEntry {
                    va: "0x141339ba0".into(),
                    rust: Some("b".into()),
                    status: MatchStatus::Bytes,
                    proof: Some(Proof {
                        kind: "uemu".into(),
                        reference: "scripts/validate_lerp.py".into(),
                    }),
                    object: None,
                    symbol: None,
                    note: None,
                },
                RegistryEntry {
                    va: "0x141334600".into(),
                    rust: Some("a".into()),
                    status: MatchStatus::Semantic,
                    proof: None,
                    object: None,
                    symbol: None,
                    note: None,
                },
            ],
        };
        r.save(&p).unwrap();
        let back = Registry::load(&p).unwrap();
        assert_eq!(back.entries.len(), 2);
        assert_eq!(back.entries[0].va, "0x141334600", "tri par adresse");
        assert_eq!(back.count(MatchStatus::Bytes), 1);
        assert_eq!(back.by_va().unwrap().len(), 2);
    }
}
