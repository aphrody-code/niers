//! Lecture seule du miroir SQLite — et la parade au lien symbolique qui bascule la nuit.
//!
//! `var/mirror.sqlite` est un **lien symbolique daté**, rebasculé par le timer `nie-miroir`.
//! `open(2)` résout le lien une seule fois : rebasculer le lien n'a aucun effet sur une
//! connexion déjà ouverte, qui continue de lire l'ancien inode **indéfiniment, sans la moindre
//! erreur** (cf. `docs/stack/pieges-api.md`). Base figée, zéro signal — le mode d'échec le plus
//! cher de cette stack.
//!
//! Parade retenue, la seule correcte : mémoriser `(st_dev, st_ino)` à l'ouverture, le comparer
//! à chaque emprunt, et **rouvrir** quand il change.

use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use rusqlite::{Connection, OpenFlags};

use crate::erreur::ErreurSite;

/// Poignée vers le miroir, rouverte automatiquement quand le lien bascule.
#[derive(Debug)]
pub struct Gisement {
    chemin: PathBuf,
    etat: Mutex<Option<Ouverture>>,
}

#[derive(Debug)]
struct Ouverture {
    conn: Connection,
    identite: (u64, u64),
}

impl Gisement {
    /// Déclare le gisement sans l'ouvrir. Un fichier absent n'est pas une erreur au démarrage :
    /// les routes concernées répondront `503`, le reste du service tourne.
    #[must_use]
    pub fn nouveau(chemin: impl Into<PathBuf>) -> Self {
        Self { chemin: chemin.into(), etat: Mutex::new(None) }
    }

    /// Chemin déclaré du miroir.
    #[must_use]
    pub fn chemin(&self) -> &Path {
        &self.chemin
    }

    /// Dit si le fichier est présent **maintenant**. Mesure, ne mémorise pas : un gisement
    /// présent peut être vide, et un gisement absent peut réapparaître au prochain miroir.
    #[must_use]
    pub fn present(&self) -> bool {
        self.chemin.is_file()
    }

    /// Exécute une lecture sur la connexion, en la (r)ouvrant si nécessaire.
    ///
    /// Bloquant : à n'appeler que depuis `spawn_blocking`.
    ///
    /// # Errors
    ///
    /// `Indisponible` quand le fichier est absent ou illisible ; l'erreur de la closure sinon.
    pub fn lire<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, ErreurSite>,
    ) -> Result<T, ErreurSite> {
        let mut etat = self.etat.lock();
        let identite = identite(&self.chemin).ok_or_else(|| {
            ErreurSite::Indisponible(format!(
                "gisement absent: {} (le miroir n'a pas encore tourne ?)",
                self.chemin.display()
            ))
        })?;
        let besoin_ouverture = match etat.as_ref() {
            None => true,
            Some(o) => o.identite != identite,
        };
        if besoin_ouverture {
            // Fermer d'abord : un `close(2)` ailleurs dans le processus annule silencieusement
            // les verrous POSIX de TOUS les descripteurs — on ne garde jamais deux poignées
            // sur deux inodes de la même base.
            *etat = None;
            let conn = Connection::open_with_flags(
                &self.chemin,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|e| {
                tracing::error!(erreur = %e, "ouverture du miroir impossible");
                ErreurSite::Indisponible("gisement illisible".to_owned())
            })?;
            tracing::info!(
                chemin = %self.chemin.display(),
                dev = identite.0,
                ino = identite.1,
                "miroir (r)ouvert"
            );
            *etat = Some(Ouverture { conn, identite });
        }
        let ouverture = etat.as_ref().expect("ouverture posee juste au-dessus");
        f(&ouverture.conn)
    }

    /// Nombre de lignes d'une table, ou `None` si la table n'existe pas.
    ///
    /// Le nom de table n'est **jamais** celui du client : il vient d'une constante de la crate.
    ///
    /// # Errors
    ///
    /// Rend `Indisponible` quand le gisement est absent.
    pub fn compte_table(&self, table: &str) -> Result<Option<i64>, ErreurSite> {
        self.lire(|c| {
            let existe: i64 = c.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )?;
            if existe == 0 {
                return Ok(None);
            }
            let n: i64 = c.query_row(&format!("SELECT count(*) FROM \"{table}\""), [], |r| r.get(0))?;
            Ok(Some(n))
        })
    }
}

/// `(st_dev, st_ino)` du fichier **résolu** (le lien est suivi : c'est bien la cible qu'on
/// surveille).
fn identite(chemin: &Path) -> Option<(u64, u64)> {
    let m = std::fs::metadata(chemin).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        Some((m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        // Hors Unix, il n'y a pas de lien daté rebasculé : la taille et la date suffisent à
        // détecter un remplacement de fichier.
        let taille = m.len();
        let date = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        Some((taille, date))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gisement_absent_est_indisponible() {
        let g = Gisement::nouveau("/nonexistent/mirror.sqlite");
        assert!(!g.present());
        let e = g.lire(|_| Ok(())).unwrap_err();
        assert_eq!(e.statut().as_u16(), 503);
    }

    #[test]
    #[cfg(unix)]
    fn reouvre_quand_l_inode_change() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.sqlite");
        let b = dir.path().join("b.sqlite");
        for (f, n) in [(&a, 3i64), (&b, 7i64)] {
            let c = Connection::open(f).unwrap();
            c.execute_batch("CREATE TABLE t(x INTEGER);").unwrap();
            for i in 0..n {
                c.execute("INSERT INTO t VALUES (?1)", [i]).unwrap();
            }
        }
        let lien = dir.path().join("mirror.sqlite");
        std::os::unix::fs::symlink(&a, &lien).unwrap();
        let g = Gisement::nouveau(&lien);
        assert_eq!(g.compte_table("t").unwrap(), Some(3));

        std::fs::remove_file(&lien).unwrap();
        std::os::unix::fs::symlink(&b, &lien).unwrap();
        assert_eq!(g.compte_table("t").unwrap(), Some(7), "le lien a bascule, la lecture suit");
        assert_eq!(g.compte_table("table_absente").unwrap(), None);
    }
}
