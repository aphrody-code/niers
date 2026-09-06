//! Source assembleur du dépôt (`forge/asm/*.s`).
//!
//! Format volontairement trivial — une ligne par corps de fonction :
//!
//! ```text
//! # commentaire
//! 0x14004d750: mov al, 0x1 ; ret
//! ```
//!
//! C'est du **code source** : lisible, diffable, modifiable à la main, et
//! suffisant à reconstruire les octets correspondants sans jamais consulter
//! `nie.exe`. Les lignes sont triées par adresse pour que les diffs restent
//! lisibles au fil des vagues de portage.

use anyhow::{Context, bail};
use nie_asm::Insn;
use std::collections::BTreeMap;
use std::path::Path;

/// Corps de fonctions régénérables, indexés par adresse virtuelle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AsmSource {
    /// Adresse virtuelle → suite d'instructions.
    pub bodies: BTreeMap<u64, Vec<Insn>>,
}

impl AsmSource {
    /// Analyse le contenu d'un fichier source.
    ///
    /// # Erreurs
    /// Retourne une erreur en indiquant la ligne fautive.
    pub fn parse(text: &str, origin: &str) -> anyhow::Result<Self> {
        let mut bodies = BTreeMap::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (addr, body) = line
                .split_once(':')
                .with_context(|| format!("{origin}:{} — `<adresse>: <insns>` attendu", n + 1))?;
            let a = addr.trim().trim_start_matches("0x");
            let va = u64::from_str_radix(a, 16)
                .with_context(|| format!("{origin}:{} — adresse invalide `{addr}`", n + 1))?;
            let insns = nie_asm::parse_line(body)
                .map_err(|e| anyhow::anyhow!("{origin}:{} — {e}", n + 1))?;
            if insns.is_empty() {
                bail!("{origin}:{} — corps vide", n + 1);
            }
            if bodies.insert(va, insns).is_some() {
                bail!("{origin}:{} — adresse {va:#x} déjà définie", n + 1);
            }
        }
        Ok(Self { bodies })
    }

    /// Charge tous les fichiers `.s` d'un répertoire (absent = source vide).
    ///
    /// # Erreurs
    /// Retourne une erreur de lecture ou d'analyse.
    pub fn load_dir(dir: &Path) -> anyhow::Result<Self> {
        let mut all = Self::default();
        if !dir.is_dir() {
            return Ok(all);
        }
        let mut files: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "s"))
            .collect();
        files.sort();
        for f in files {
            let text = std::fs::read_to_string(&f)
                .with_context(|| format!("lecture de {}", f.display()))?;
            let part = Self::parse(&text, &f.display().to_string())?;
            for (va, insns) in part.bodies {
                if all.bodies.insert(va, insns).is_some() {
                    bail!(
                        "adresse {va:#x} définie dans deux fichiers de {}",
                        dir.display()
                    );
                }
            }
        }
        Ok(all)
    }

    /// Écrit la source dans un fichier, avec un en-tête de commentaire.
    ///
    /// # Erreurs
    /// Retourne une erreur d'écriture disque.
    pub fn save(&self, path: &Path, header: &str) -> anyhow::Result<()> {
        self.save_annotated(path, header, &std::collections::BTreeMap::new())
    }

    /// Écrit la source en préfixant chaque corps du **nom** de la fonction qu'il
    /// reproduit, quand la base de connaissance RE le connaît.
    ///
    /// # Erreurs
    /// Retourne une erreur d'écriture disque.
    pub fn save_annotated(
        &self,
        path: &Path,
        header: &str,
        names: &BTreeMap<u64, String>,
    ) -> anyhow::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut out = String::new();
        for l in header.lines() {
            out.push_str("# ");
            out.push_str(l);
            out.push('\n');
        }
        out.push('\n');
        for (va, insns) in &self.bodies {
            // Nom issu de l'échafaudage RE : la source devient navigable.
            if let Some(n) = names.get(va) {
                out.push_str(&format!("# {n}\n"));
            }
            out.push_str(&format!("{va:#x}: {}\n", nie_asm::to_line(insns)));
        }
        std::fs::write(path, out).with_context(|| format!("écriture de {}", path.display()))?;
        Ok(())
    }

    /// Octets régénérés pour une adresse donnée.
    #[must_use]
    pub fn emit(&self, va: u64) -> Option<Vec<u8>> {
        // Encodage conscient de l'adresse : branchements et opérandes `[rip …]`
        // sont résolus depuis la position réelle du corps.
        self.bodies.get(&va).map(|i| nie_asm::encode_at(i, va))
    }

    /// Nombre de corps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// Vrai si la source est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour_fichier() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("asm").join("thunks.s");
        let src = AsmSource::parse(
            "# essai\n0x14004d750: mov al, 0x1 ; ret\n\n0x14004d770: xor eax, eax ; ret\n",
            "essai",
        )
        .unwrap();
        assert_eq!(src.len(), 2);
        assert_eq!(src.emit(0x1_4004_d750).unwrap(), vec![0xB0, 0x01, 0xC3]);

        src.save(&p, "généré par un test").unwrap();
        let back = AsmSource::load_dir(p.parent().unwrap()).unwrap();
        assert_eq!(back, src);
    }

    #[test]
    fn erreurs_localisees() {
        // `vfmadd231ps` tenait ce rôle jusqu'à l'arrivée du VEX ; `aesenc`
        // reste hors dialecte.
        let e = AsmSource::parse("0x140: aesenc xmm0, xmm1\n", "f.s").unwrap_err();
        assert!(e.to_string().contains("f.s:1"), "{e}");
        let e = AsmSource::parse("0x140: ret\n0x140: ret\n", "f.s").unwrap_err();
        assert!(e.to_string().contains("déjà définie"), "{e}");
        let e = AsmSource::parse("pas de deux-points\n", "f.s").unwrap_err();
        assert!(e.to_string().contains("attendu"), "{e}");
    }

    #[test]
    fn repertoire_absent_donne_une_source_vide() {
        let src = AsmSource::load_dir(Path::new("chemin/inexistant/pour/le/test")).unwrap();
        assert!(src.is_empty());
    }
}
