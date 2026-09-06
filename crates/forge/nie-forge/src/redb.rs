//! Lecture de la base de connaissance RE (`var/niers.sqlite`).
//!
//! L'échafaudage de reverse a nommé **6 429 fonctions** structurellement
//! (RTTI + vtables) et borné **50 674 racines** via `.pdata`. La forge s'en sert
//! pour deux choses :
//!
//! 1. **Nommer la source produite.** `forge/asm/lifted.s` passe d'une suite
//!    d'adresses anonymes à un fichier navigable : chaque corps porte le nom de
//!    la fonction qu'il reproduit. C'est ce qui relie le RE (le moyen) à la
//!    production du binaire (la fin).
//! 2. **Se faire contredire.** Le découpage de la forge et la table `pdata_func`
//!    décrivent le même objet ; tout écart entre les deux est un signal, pas un
//!    détail — l'un des deux se trompe.

use anyhow::Context;
use std::collections::BTreeMap;
use std::path::Path;

/// Ce que la base apporte à la forge.
#[derive(Debug, Default)]
pub struct ReNames {
    /// Adresse virtuelle → nom de fonction.
    pub names: BTreeMap<u64, String>,
    /// Racines `.pdata` enregistrées par l'échafaudage RE.
    pub pdata_roots: usize,
    /// Fonctions **mesurées** de l'échafaudage : `(adresse virtuelle, taille)`.
    ///
    /// Sert à découper le résidu de `.text` que `.pdata` ne décrit pas — les
    /// fonctions feuilles, sans données de déroulement.
    pub sized: Vec<(u64, u32)>,
}

impl ReNames {
    /// Charge les noms depuis la base ; base absente ⇒ ensemble vide, pas d'erreur.
    ///
    /// # Erreurs
    /// Retourne une erreur si la base existe mais est illisible.
    pub fn load(db: &Path) -> anyhow::Result<Self> {
        if !db.is_file() {
            return Ok(Self::default());
        }
        let conn =
            rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .with_context(|| format!("ouverture de {}", db.display()))?;

        let mut names = BTreeMap::new();
        let mut stmt =
            conn.prepare("SELECT vaddr, name FROM function WHERE name IS NOT NULL AND name <> ''")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?))
        })?;
        for row in rows.flatten() {
            names.insert(row.0, row.1);
        }
        let pdata_roots: i64 = conn
            .query_row("SELECT count(*) FROM pdata_func", [], |r| r.get(0))
            .unwrap_or(0);

        let mut sized = Vec::new();
        if let Ok(mut q) =
            conn.prepare("SELECT vaddr, size FROM function WHERE size > 0 ORDER BY vaddr")
            && let Ok(rows) = q.query_map([], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u32))
            })
        {
            sized.extend(rows.flatten());
        }

        Ok(Self {
            names,
            pdata_roots: usize::try_from(pdata_roots).unwrap_or(0),
            sized,
        })
    }

    /// Nom d'une adresse, s'il est connu.
    #[must_use]
    pub fn get(&self, va: u64) -> Option<&str> {
        self.names.get(&va).map(String::as_str)
    }

    /// Nombre de noms chargés.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Vrai si aucune information n'a été chargée.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}
