//! Mesure honnête de la conquête du binaire.
//!
//! Une seule question compte : **quelle part de `nie.exe` le workspace Rust
//! produit-il réellement ?** Le rapport la décompose sans arrondi favorable :
//!
//! - `emitted` : octets **calculés** par du code Rust (en-têtes PE ré-émis).
//! - `bytes` : fonctions dont le codegen rustc coïncide avec l'original.
//! - `semantic` : fonctions dont le comportement est validé byte-exact par
//!   l'oracle, mais dont le codegen ne coïncide pas — **elles ne comptent pas**
//!   dans la part produite, elles sont suivies à part.
//! - `verbatim` : tout le reste, recopié de la référence.

use crate::asmsrc::AsmSource;
use crate::registry::{MatchStatus, Registry};
use nie_pe::{Cover, UnitKind};
use serde::Serialize;

/// Ligne de statistiques d'une catégorie d'unités.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Bucket {
    /// Nombre d'unités.
    pub units: usize,
    /// Masse d'octets.
    pub bytes: usize,
}

impl Bucket {
    fn add(&mut self, len: usize) {
        self.units += 1;
        self.bytes += len;
    }
}

/// Rapport de couverture de la forge.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Taille du binaire cible.
    pub total_bytes: usize,
    /// Nombre total d'unités.
    pub total_units: usize,
    /// Octets de code (`.text` et assimilés).
    pub code_bytes: usize,
    /// Fonctions délimitées par `.pdata`.
    pub functions: usize,
    /// Octets couverts par des fonctions `.pdata`.
    pub function_bytes: usize,
    /// Unités calculées structurellement par du code Rust (en-têtes PE).
    pub emitted: Bucket,
    /// Unités régénérées par `nie-asm` depuis la source assembleur du dépôt.
    pub assembled: Bucket,
    /// Fonctions dont le codegen coïncide.
    pub matched_bytes: Bucket,
    /// Fonctions validées sémantiquement (comptées à part).
    pub matched_semantic: Bucket,
    /// Portages en cours, non validés.
    pub wip: Bucket,
    /// Entrées du registre sans unité correspondante (adresse morte).
    pub orphan_entries: usize,
}

impl Report {
    /// Ajoute au seau `emitted` les **sections-tables** que `nie-pe` sait
    /// ré-émettre depuis leurs entrées (`.pdata`, `.reloc`).
    ///
    /// Ces sections ne sont pas recopiées : elles sont régénérées, au même
    /// titre que les en-têtes PE, et `nie-forge build` les émet ainsi. Sans cet
    /// ajout le rapport **sous-déclare** et diverge de la construction — de
    /// 1 427 968 octets sur `nie.exe`, soit 4,2 points.
    ///
    /// Cette étape vit ici, et non dans l'appelant, précisément pour que les
    /// deux façades de la mesure — la CLI `nie-forge report` et l'onglet
    /// « Forge » de `nie-explorer` — ne puissent pas diverger.
    pub fn add_emitted_tables(&mut self, cover: &Cover, img: &nie_pe::PeImage) {
        for u in &cover.units {
            if u.kind == UnitKind::SectionData
                && let Some(sec) = u.section.as_deref()
                && nie_pe::image::tables::emit_for(img, sec).is_some_and(|b| b.len() == u.len)
            {
                self.emitted.units += 1;
                self.emitted.bytes += u.len;
            }
        }
    }

    /// Construit le rapport en croisant recouvrement, source assembleur et registre.
    ///
    /// # Erreurs
    /// Retourne une erreur si une adresse du registre est invalide.
    pub fn build(cover: &Cover, registry: &Registry, asm: &AsmSource) -> anyhow::Result<Self> {
        let mut r = Self {
            total_bytes: cover.total_len,
            total_units: cover.units.len(),
            functions: cover.count_by_kind(UnitKind::Function),
            function_bytes: cover.bytes_by_kind(UnitKind::Function),
            ..Default::default()
        };
        r.code_bytes = cover
            .units
            .iter()
            .filter(|u| u.kind.is_code())
            .map(|u| u.len)
            .sum();

        let by_va = registry.by_va()?;
        for u in &cover.units {
            // Ordre d'attribution IDENTIQUE à celui de `build` : en-têtes, puis
            // source assembleur, puis codegen enregistré. Une unité fournie par
            // deux voies ne doit être comptée qu'une fois — sinon la part
            // « produite » se gonflerait toute seule.
            let entry = u.va.and_then(|va| by_va.get(&va));
            if u.kind == UnitKind::PeHeaders {
                r.emitted.add(u.len);
            } else if u.emit_rule().is_some_and(|b| b.len() == u.len) {
                // Règle du linker connue (bourrage `int3`) : produite, pas recopiée.
                r.emitted.add(u.len);
            } else if u.kind.is_code()
                && let Some(va) = u.va
                && asm.emit(va).is_some_and(|b| b.len() == u.len)
            {
                r.assembled.add(u.len);
            } else if entry.is_some_and(|e| e.status == MatchStatus::Bytes) {
                r.matched_bytes.add(u.len);
            }

            // Suivi séparé : ne produit aucun octet, ne se cumule pas au-dessus.
            match entry.map(|e| e.status) {
                Some(MatchStatus::Semantic) => r.matched_semantic.add(u.len),
                Some(MatchStatus::Wip) => r.wip.add(u.len),
                _ => {}
            }
        }
        // Une entrée est orpheline si aucune unité ne COMMENCE à son adresse :
        // c'est le signal que l'adresse vient d'un autre build ou tombe au
        // milieu d'une fonction réelle.
        r.orphan_entries = by_va
            .keys()
            .filter(|va| !cover.units.iter().any(|u| u.va == Some(**va)))
            .count();
        Ok(r)
    }

    /// Octets réellement produits par du code Rust.
    ///
    /// Trois sources cumulées, aucune n'étant une recopie : en-têtes calculés,
    /// corps réassemblés depuis la source du dépôt, fonctions dont le codegen
    /// rustc coïncide. Le `semantic` n'y figure **pas** — il n'produit pas
    /// d'octets.
    #[must_use]
    pub fn produced_bytes(&self) -> usize {
        self.emitted.bytes + self.assembled.bytes + self.matched_bytes.bytes
    }

    /// Part du fichier réellement produite par du code Rust, en pourcentage.
    #[must_use]
    pub fn produced_pct(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        self.produced_bytes() as f64 * 100.0 / self.total_bytes as f64
    }

    /// Part du code (`.text`) produite par du code Rust, en pourcentage.
    #[must_use]
    pub fn code_pct(&self) -> f64 {
        if self.code_bytes == 0 {
            return 0.0;
        }
        (self.assembled.bytes + self.matched_bytes.bytes) as f64 * 100.0 / self.code_bytes as f64
    }

    /// Rendu terse `clé=valeur` (convention CLI du projet).
    #[must_use]
    pub fn terse(&self) -> String {
        format!(
            "total={} units={} code={} fns={} emitted={} asm_units={} asm_bytes={} matched_bytes={} semantic={} wip={} produced={:.6}% code_rust={:.6}%",
            self.total_bytes,
            self.total_units,
            self.code_bytes,
            self.functions,
            self.emitted.bytes,
            self.assembled.units,
            self.assembled.bytes,
            self.matched_bytes.bytes,
            self.matched_semantic.units,
            self.wip.units,
            self.produced_pct(),
            self.code_pct(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::RegistryEntry;
    use nie_pe::Unit;

    fn unit(id: &str, kind: UnitKind, off: usize, len: usize, va: Option<u64>) -> Unit {
        Unit {
            id: id.into(),
            kind,
            section: None,
            file_off: off,
            len,
            va,
            sha256: String::new(),
        }
    }

    fn cover() -> Cover {
        Cover {
            total_len: 1000,
            sha256: String::new(),
            units: vec![
                unit("hdr", UnitKind::PeHeaders, 0, 100, None),
                unit(
                    "fn.140001000",
                    UnitKind::Function,
                    100,
                    400,
                    Some(0x1_4000_1000),
                ),
                unit(
                    "fn.140002000",
                    UnitKind::Function,
                    500,
                    200,
                    Some(0x1_4000_2000),
                ),
                unit("res..text.2bc", UnitKind::CodeResidue, 700, 100, None),
                unit("data.rdata", UnitKind::SectionData, 800, 200, None),
            ],
        }
    }

    fn entry(va: &str, status: MatchStatus) -> RegistryEntry {
        RegistryEntry {
            va: va.into(),
            rust: Some("x".into()),
            status,
            proof: None,
            object: None,
            symbol: None,
            note: None,
        }
    }

    #[test]
    fn compte_separement_produit_et_semantique() {
        let reg = Registry {
            version: 1,
            target_sha256: None,
            entries: vec![
                entry("0x140001000", MatchStatus::Bytes),
                entry("0x140002000", MatchStatus::Semantic),
            ],
        };
        let r = Report::build(&cover(), &reg, &AsmSource::default()).unwrap();
        assert_eq!(r.functions, 2);
        assert_eq!(r.code_bytes, 700);
        assert_eq!(r.emitted.bytes, 100);
        assert_eq!(r.matched_bytes.bytes, 400);
        assert_eq!(r.matched_semantic.bytes, 200, "sémantique suivi à part");
        assert_eq!(r.produced_bytes(), 500);
        assert!((r.produced_pct() - 50.0).abs() < 1e-9);
        assert!(
            (r.code_pct() - 400.0 * 100.0 / 700.0).abs() < 1e-9,
            "le sémantique ne gonfle pas la part de code produite"
        );
        assert_eq!(r.orphan_entries, 0);
    }

    #[test]
    fn detecte_les_entrees_orphelines() {
        let reg = Registry {
            version: 1,
            target_sha256: None,
            entries: vec![entry("0x14dead00", MatchStatus::Bytes)],
        };
        let r = Report::build(&cover(), &reg, &AsmSource::default()).unwrap();
        assert_eq!(r.orphan_entries, 1);
        assert_eq!(r.matched_bytes.units, 0);
    }

    #[test]
    fn une_unite_fournie_deux_fois_n_est_comptee_qu_une_fois() {
        // Même unité couverte par la source assembleur ET par un codegen enregistré :
        // `build` n'en écrit qu'une, le rapport ne doit pas en compter deux.
        let asm = AsmSource::parse("0x140002000: mov al, 0x1 ; ret\n", "essai").unwrap();
        let reg = Registry {
            version: 1,
            target_sha256: None,
            entries: vec![entry("0x140002000", MatchStatus::Bytes)],
        };
        let mut c = cover();
        c.units[2].len = 3;
        let r = Report::build(&c, &reg, &asm).unwrap();
        assert_eq!(r.assembled.bytes, 3);
        assert_eq!(r.matched_bytes.bytes, 0, "pas de double comptage");
        assert_eq!(r.produced_bytes(), 103);
    }

    #[test]
    fn compte_les_corps_reassembles_a_la_bonne_taille() {
        // Corps de 3 octets (`mov al, 1 ; ret`) déclaré à l'adresse d'une unité
        // qui en fait 400 : la taille ne colle pas, il ne doit PAS être compté.
        let asm = AsmSource::parse(
            "0x140001000: mov al, 0x1 ; ret\n0x140002000: mov al, 0x1 ; ret\n",
            "essai",
        )
        .unwrap();
        let mut c = cover();
        c.units[2].len = 3; // fn.140002000 fait désormais exactement 3 octets
        let r = Report::build(&c, &Registry::default(), &asm).unwrap();
        assert_eq!(
            r.assembled.units, 1,
            "seule l'unité de taille compatible compte"
        );
        assert_eq!(r.assembled.bytes, 3);
        assert_eq!(
            r.produced_bytes(),
            103,
            "en-têtes (100) + corps assemblé (3)"
        );
    }
}
