//! Indexeur de binaire via `aphrody-re` : triage PE → sections + symboles
//! ingérés dans la base de connaissance (`section` + `symbol`).
//!
//! Ce module fournit [`triage_into`] qui :
//! 1. Lit le binaire avec `memmap2` (lecture seule, zero-copy).
//! 2. Appelle `aphrody_re::triage(bytes)` pour extraire le rapport PE.
//! 3. Insère les sections dans `nie_index::ingest::section` (si la table
//!    existe — elle est dans le schéma v1).
//! 4. Insère imports/exports dans `nie_index::ingest::symbol`.
//! 5. Renvoie un [`IndexStats`] avec les comptes.

use anyhow::{Context, Result};
use nie_index::{Db, rusqlite};
use std::path::Path;
use tracing::info;

/// Résultat de l'indexation d'un binaire.
#[derive(Debug, Clone, Copy, Default)]
pub struct IndexStats {
    /// Format détecté (affichage humain).
    pub format: &'static str,
    /// Sections insérées.
    pub sections: usize,
    /// Symboles importés insérés.
    pub imports: usize,
    /// Symboles exportés insérés.
    pub exports: usize,
}

/// Trie le binaire à `exe_path` via `aphrody_re::triage` et ingère les
/// sections + symboles dans la base. Idempotent (INSERT OR IGNORE).
///
/// Si `aphrody_re::triage` renvoie `Format::Unknown`, les sections/imports
/// sont vides mais la fonction réussit quand même (pas d'erreur).
pub fn triage_into(db: &mut Db, binary_id: i64, exe_path: &Path) -> Result<IndexStats> {
    // Lecture en mémoire (safe, pas d'unsafe memmap2).
    let bytes =
        std::fs::read(exe_path).with_context(|| format!("lecture {}", exe_path.display()))?;

    let report =
        aphrody_re::triage(&bytes).with_context(|| format!("triage {}", exe_path.display()))?;

    let format_str: &'static str = match report.format {
        aphrody_re::Format::Pe32 => "PE32",
        aphrody_re::Format::Pe64 => "PE32+",
        aphrody_re::Format::Elf32 => "ELF32",
        aphrody_re::Format::Elf64 => "ELF64",
        aphrody_re::Format::Unknown => "Unknown",
    };

    info!(
        "triage {}: {} ({} sections, {} imports, {} exports)",
        exe_path.display(),
        format_str,
        report.sections.len(),
        report.imports.len(),
        report.exports.len()
    );

    let tx = db.conn_mut().transaction()?;

    let mut sections = 0usize;
    for sec in &report.sections {
        // Tentative d'insertion dans la table `section` (peut échouer si la
        // section est déjà présente — ON CONFLICT IGNORE via le UNIQUE).
        let r = tx.execute(
            "INSERT OR IGNORE INTO section(binary_id, name, vaddr, vsize, paddr, psize, perm)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![
                binary_id,
                &sec.name,
                sec.vaddr as i64,
                sec.size as i64,  // vsize = raw size ici (triage ne distingue pas)
                sec.vaddr as i64, // paddr (conservatif : même valeur)
                sec.size as i64,  // psize
                "r"               // perm (inconnu depuis triage)
            ],
        );
        if r.is_ok() {
            sections += 1;
        }
    }

    let mut imports = 0usize;
    for imp in &report.imports {
        let r = tx.execute(
            "INSERT OR IGNORE INTO symbol(binary_id, name, kind) VALUES(?1,?2,'import')",
            rusqlite::params![binary_id, imp],
        );
        if r.is_ok() {
            imports += 1;
        }
    }

    let mut exports = 0usize;
    for exp in &report.exports {
        let r = tx.execute(
            "INSERT OR IGNORE INTO symbol(binary_id, name, kind) VALUES(?1,?2,'export')",
            rusqlite::params![binary_id, exp],
        );
        if r.is_ok() {
            exports += 1;
        }
    }

    tx.commit().context("commit triage")?;

    Ok(IndexStats {
        format: format_str,
        sections,
        imports,
        exports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_index::Db;
    use std::io::Write as _;

    #[test]
    fn triage_into_unknown_format_does_not_error() -> anyhow::Result<()> {
        // Crée un fichier temporaire avec du contenu aléatoire non-PE.
        let dir = std::env::temp_dir();
        let tmp_path = dir.join("nie_re_indexer_test.bin");
        {
            let mut f = std::fs::File::create(&tmp_path)?;
            f.write_all(b"not a real binary, just some bytes")?;
        }
        // Nettoyage garanti même en cas d'erreur.
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(tmp_path.clone());

        let mut db = Db::open_in_memory()?;
        let bin = db.upsert_binary("test.bin", "sha_test", "x86_64", 64, 0, 0, None, None)?;

        // Ne doit pas paniquer ni échouer même pour un format inconnu.
        let stats = triage_into(&mut db, bin, &tmp_path)?;
        assert_eq!(stats.format, "Unknown");
        assert_eq!(stats.sections, 0);
        assert_eq!(stats.imports, 0);
        assert_eq!(stats.exports, 0);
        Ok(())
    }
}
