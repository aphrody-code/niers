//! Réconciliation de la base de connaissance avec le **découpage byte-exact**.
//!
//! La base RE (`var/niers.sqlite`) et la forge décrivent le même objet par deux
//! chemins indépendants : la première par le reverse (`.pdata`, RTTI, vtables,
//! chaînes), la seconde par un recouvrement total du fichier qui se réassemble
//! à l'octet près. Faire porter au même endroit ce que chacune sait, c'est
//! obtenir un tri de toutes les fonctions — offset, taille, nature, et
//! **état de production réel** — au lieu de deux inventaires qui s'ignorent.
//!
//! Ce que le croisement dit déjà, mesuré : les 115 326 unités de fonction de la
//! forge sont **toutes** connues de la base (aucune inventée), et la base porte
//! 2 168 adresses que le découpage validé refuse — bornes tombant au milieu
//! d'une instruction, ou fonctions de taille nulle. C'est précisément le genre
//! d'écart qu'un inventaire isolé ne peut pas voir.
//!
//! Rien n'est écrit dans `function` : la table `forge_unit` est ajoutée à côté,
//! et la vue `v_forge_function` fait la jointure. Le reverse garde donc la main
//! sur ses propres colonnes, et la forge sur les siennes.

use crate::lift::{blocking_reason, lift_body};
use anyhow::Context;
use nie_pe::{Cover, UnitKind};
use std::path::Path;

/// Schéma de la table de classification et de sa vue (idempotent).
///
/// Déclaré aussi dans `nie-index/src/schema.sql`, pour qu'une base créée de
/// zéro le porte sans avoir à passer par la forge.
pub const SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS forge_unit (
    binary_id INTEGER NOT NULL,
    file_off  INTEGER NOT NULL,
    vaddr     INTEGER,
    size      INTEGER NOT NULL,
    kind      TEXT NOT NULL,
    statut    TEXT NOT NULL,
    cause     TEXT,
    PRIMARY KEY (binary_id, file_off)
);
CREATE INDEX IF NOT EXISTS idx_forge_unit_vaddr ON forge_unit(binary_id, vaddr);
CREATE INDEX IF NOT EXISTS idx_forge_unit_statut ON forge_unit(binary_id, statut);
CREATE TABLE IF NOT EXISTS forge_classe (
    binary_id    INTEGER NOT NULL,
    classe       TEXT NOT NULL,
    vtable_vaddr INTEGER,
    methodes     INTEGER NOT NULL,
    resolues     INTEGER NOT NULL,
    produites    INTEGER NOT NULL,
    bloquees     INTEGER NOT NULL,
    octets       INTEGER NOT NULL,
    PRIMARY KEY (binary_id, classe)
);
CREATE VIEW IF NOT EXISTS v_forge_function AS
SELECT f.binary_id,
       f.vaddr,
       printf('0x%x', f.vaddr) AS va_hex,
       f.name,
       f.name_source,
       f.subsystem,
       f.role,
       f.size        AS taille_kb,
       u.size        AS taille_forge,
       u.file_off,
       u.kind,
       COALESCE(u.statut, 'hors_decoupage') AS statut,
       u.cause
FROM function f
LEFT JOIN forge_unit u
       ON u.binary_id = f.binary_id AND u.vaddr = f.vaddr;
";

/// État de production d'une unité, du point de vue de la forge.
mod statut {
    /// Corps relevé et ré-encodé à l'octet près : produit par le dépôt.
    pub const PRODUIT: &str = "produit";
    /// Code que le relevé refuse encore, avec sa cause.
    pub const BLOQUE: &str = "bloque";
    /// Octets régénérés par une règle du linker (bourrage `int3`, en-têtes).
    pub const REGLE: &str = "regle";
    /// Données déposées au milieu du code (tables de sauts, constantes).
    pub const DONNEES: &str = "donnees_inline";
    /// Recopié de la référence : rien n'est prétendu.
    pub const VERBATIM: &str = "verbatim";
}

/// Ce que la synchronisation a écrit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Bilan {
    /// Identifiant du binaire visé dans la base.
    pub binary_id: i64,
    /// Unités écrites.
    pub unites: usize,
    /// Unités de code produites.
    pub produites: usize,
    /// Unités de code bloquées.
    pub bloquees: usize,
    /// Fonctions de la base sans unité correspondante dans le découpage.
    pub hors_decoupage: usize,
    /// Fonctions de la base dont la taille contredit celle du découpage.
    pub tailles_divergentes: usize,
    /// Classes RTTI dotées d'une adresse de vtable exploitable.
    pub classes: usize,
    /// Entrées de vtable lues.
    pub methodes: usize,
    /// Entrées tombant exactement sur une unité de fonction du découpage.
    pub methodes_resolues: usize,
}

/// Nombre maximal d'entrées lues dans une vtable.
///
/// Garde-fou : une adresse fausse ferait courir la lecture jusqu'au bout de la
/// section. Les hiérarchies les plus profondes du binaire restent loin en deçà.
const MAX_METHODES: usize = 4096;

/// Décalage entre l'adresse enregistrée et la première méthode.
///
/// `rtti_class.vtable_vaddr` désigne le **`complete object locator`**, pas la
/// première entrée : MSVC place ce pointeur juste avant la table. Mesuré sur
/// les 1 745 classes du binaire — **aucune** ne pointe sur du code, **toutes**
/// en ont à `+8`. Accessoirement, cette régularité valide ces adresses, que
/// l'avertissement du dépôt sur l'index Ghidra (« désaligné, figé ») donnait
/// pour douteuses : un index décalé ne tomberait pas juste 1 745 fois.
const COL_AVANT_VTABLE: usize = 8;

/// Énumère les entrées d'une vtable : des pointeurs vers du code, consécutifs.
///
/// S'arrête au premier mot qui ne pointe pas dans une section exécutable —
/// c'est la borne naturelle, la vtable suivante étant précédée de son propre
/// `complete object locator`, qui pointe dans `.rdata`.
fn methodes_de(img: &nie_pe::PeImage, vtable: u64) -> Vec<u64> {
    let mut out = Vec::new();
    let Some(mut off) = img.va_to_offset(vtable).map(|o| o + COL_AVANT_VTABLE) else {
        return out;
    };
    while out.len() < MAX_METHODES {
        let Some(mot) = img.bytes.get(off..off + 8) else {
            break;
        };
        let va = u64::from_le_bytes(mot.try_into().expect("8 octets"));
        let Some(rva) = va.checked_sub(img.opt.image_base) else {
            break;
        };
        let Ok(rva) = u32::try_from(rva) else { break };
        let exec = img
            .sections
            .iter()
            .any(|s| s.contains_rva(rva) && s.characteristics & 0x2000_0020 != 0);
        if !exec {
            break;
        }
        out.push(va);
        off += 8;
    }
    out
}

/// Écrit la classification de chaque unité dans la base et rend le bilan.
///
/// L'écriture est idempotente : la table est vidée pour ce binaire puis
/// réécrite, si bien qu'un découpage plus fin ne laisse jamais d'unité fantôme.
///
/// # Erreurs
/// Retourne une erreur si la base est absente, illisible, ou si aucun binaire
/// n'y porte de fonctions.
pub fn synchroniser(
    db: &Path,
    cover: &Cover,
    bytes: &[u8],
    img: Option<&nie_pe::PeImage>,
) -> anyhow::Result<Bilan> {
    let mut conn =
        rusqlite::Connection::open(db).with_context(|| format!("ouverture de {}", db.display()))?;
    conn.execute_batch(SCHEMA).context("schéma forge_unit")?;

    // Le binaire visé est celui qui porte les fonctions : la table `binary` en
    // compte plusieurs (index Ghidra, `#pdata`), et seul le second fait foi.
    let binary_id: i64 = conn
        .query_row(
            "SELECT binary_id FROM function GROUP BY binary_id ORDER BY count(*) DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .context("aucun binaire ne porte de fonctions dans la base")?;

    let mut b = Bilan {
        binary_id,
        ..Bilan::default()
    };

    let tx = conn.transaction()?;
    tx.execute("DELETE FROM forge_unit WHERE binary_id = ?1", [binary_id])?;
    {
        let mut ins = tx.prepare(
            "INSERT INTO forge_unit (binary_id, file_off, vaddr, size, kind, statut, cause)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for u in &cover.units {
            let (statut, cause) = classer(u, bytes);
            match statut {
                statut::PRODUIT => b.produites += 1,
                statut::BLOQUE => b.bloquees += 1,
                _ => {}
            }
            ins.execute(rusqlite::params![
                binary_id,
                i64::try_from(u.file_off).unwrap_or(-1),
                u.va.map(|v| v as i64),
                i64::try_from(u.len).unwrap_or(-1),
                u.kind.tag(),
                statut,
                cause,
            ])?;
            b.unites += 1;
        }
    }
    tx.commit()?;

    b.hors_decoupage = conn.query_row(
        "SELECT count(*) FROM function f
          WHERE f.binary_id = ?1
            AND NOT EXISTS (SELECT 1 FROM forge_unit u
                             WHERE u.binary_id = f.binary_id AND u.vaddr = f.vaddr)",
        [binary_id],
        |r| r.get::<_, i64>(0),
    )? as usize;
    b.tailles_divergentes = conn.query_row(
        "SELECT count(*) FROM function f
           JOIN forge_unit u ON u.binary_id = f.binary_id AND u.vaddr = f.vaddr
          WHERE f.binary_id = ?1 AND f.size > 0 AND f.size <> u.size",
        [binary_id],
        |r| r.get::<_, i64>(0),
    )? as usize;

    if let Some(img) = img {
        classes(&mut conn, binary_id, img, &mut b)?;
    }
    Ok(b)
}

/// Croise les vtables RTTI avec l'état de production de leurs méthodes.
///
/// Les adresses de vtable et les fonctions **ne vivent pas sous le même
/// `binary_id`** : `rtti_class` ne les porte que sur l'entrée de l'index
/// Ghidra, quand les fonctions sont sur celle de `#pdata`. Les deux décrivent
/// le même fichier, mais l'avertissement du dépôt sur l'index Ghidra
/// (« désaligné, figé ») interdit de le supposer — d'où le compteur
/// `methodes_resolues` : c'est lui qui dit si ces adresses tombent vraiment sur
/// des fonctions du découpage, ou si elles décrivent un autre agencement.
fn classes(
    conn: &mut rusqlite::Connection,
    binary_id: i64,
    img: &nie_pe::PeImage,
    b: &mut Bilan,
) -> anyhow::Result<()> {
    let sources: Vec<(String, i64)> = {
        let mut q = conn.prepare(
            "SELECT name, vtable_vaddr FROM rtti_class
              WHERE vtable_vaddr IS NOT NULL ORDER BY name",
        )?;
        let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        rows.flatten().collect()
    };

    // Statut de chaque unité de fonction, par adresse.
    let par_va: std::collections::HashMap<i64, (String, i64)> = {
        let mut q = conn.prepare(
            "SELECT vaddr, statut, size FROM forge_unit
              WHERE binary_id = ?1 AND vaddr IS NOT NULL AND kind = 'fn'",
        )?;
        let rows = q.query_map([binary_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, i64>(2)?),
            ))
        })?;
        rows.flatten().collect()
    };

    let tx = conn.transaction()?;
    tx.execute("DELETE FROM forge_classe WHERE binary_id = ?1", [binary_id])?;
    {
        let mut ins = tx.prepare(
            "INSERT OR REPLACE INTO forge_classe
                (binary_id, classe, vtable_vaddr, methodes, resolues, produites, bloquees, octets)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for (nom, vtable) in sources {
            let m = methodes_de(img, vtable as u64);
            let (mut resolues, mut produites, mut bloquees, mut octets) = (0i64, 0i64, 0i64, 0i64);
            for va in &m {
                if let Some((statut, taille)) = par_va.get(&(*va as i64)) {
                    resolues += 1;
                    octets += taille;
                    match statut.as_str() {
                        statut::PRODUIT => produites += 1,
                        statut::BLOQUE => bloquees += 1,
                        _ => {}
                    }
                }
            }
            b.classes += 1;
            b.methodes += m.len();
            b.methodes_resolues += usize::try_from(resolues).unwrap_or(0);
            ins.execute(rusqlite::params![
                binary_id,
                nom,
                vtable,
                i64::try_from(m.len()).unwrap_or(-1),
                resolues,
                produites,
                bloquees,
                octets,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Statut et cause d'une unité.
fn classer(u: &nie_pe::Unit, bytes: &[u8]) -> (&'static str, Option<String>) {
    if u.kind == UnitKind::InlineData {
        return (statut::DONNEES, None);
    }
    if u.kind == UnitKind::PeHeaders || u.emit_rule().is_some() {
        return (statut::REGLE, None);
    }
    if !u.kind.is_code() {
        return (statut::VERBATIM, None);
    }
    let (Some(va), Some(corps)) = (u.va, bytes.get(u.range())) else {
        return (statut::VERBATIM, None);
    };
    if lift_body(corps, va).is_some() {
        (statut::PRODUIT, None)
    } else {
        (statut::BLOQUE, blocking_reason(corps, va))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_pe::Unit;

    fn unit(kind: UnitKind, off: usize, len: usize, va: Option<u64>) -> Unit {
        Unit {
            id: format!("{}.{off:x}", kind.tag()),
            kind,
            section: Some(".text".into()),
            file_off: off,
            len,
            va,
            sha256: String::new(),
        }
    }

    #[test]
    fn classe_chaque_nature_d_unite() {
        // `xor eax,eax ; ret` se releve ; `ff ff ff` non.
        let bytes = vec![0x33, 0xC0, 0xC3, 0xFF, 0xFF, 0xFF, 0xCC, 0xCC];
        let va = 0x1_4000_0000;
        assert_eq!(
            classer(&unit(UnitKind::Function, 0, 3, Some(va)), &bytes).0,
            statut::PRODUIT
        );
        let (s, cause) = classer(&unit(UnitKind::Function, 3, 3, Some(va + 3)), &bytes);
        assert_eq!(s, statut::BLOQUE);
        assert!(cause.is_some(), "un blocage doit porter sa cause");
        assert_eq!(
            classer(&unit(UnitKind::Padding, 6, 2, Some(va + 6)), &bytes).0,
            statut::REGLE
        );
        assert_eq!(
            classer(&unit(UnitKind::InlineData, 3, 3, Some(va + 3)), &bytes).0,
            statut::DONNEES
        );
        assert_eq!(
            classer(&unit(UnitKind::SectionData, 0, 8, None), &bytes).0,
            statut::VERBATIM
        );
    }

    #[test]
    fn la_synchronisation_est_idempotente() {
        let dir = tempfile::tempdir().expect("tmp");
        let db = dir.path().join("kb.sqlite");
        let conn = rusqlite::Connection::open(&db).expect("ouverture");
        conn.execute_batch(
            "CREATE TABLE function (binary_id INTEGER, vaddr INTEGER, size INTEGER,
                                    name TEXT, name_source TEXT, subsystem TEXT, role TEXT);
             INSERT INTO function VALUES (2, 5368709120, 3, 'essai', 'pdata', 'menu', 'leaf');",
        )
        .expect("base d'essai");
        drop(conn);

        let cover = Cover {
            total_len: 3,
            sha256: String::new(),
            units: vec![unit(UnitKind::Function, 0, 3, Some(0x1_4000_0000))],
        };
        let bytes = vec![0x33, 0xC0, 0xC3];

        let a = synchroniser(&db, &cover, &bytes, None).expect("sync");
        assert_eq!(a.binary_id, 2);
        assert_eq!(a.unites, 1);
        assert_eq!(a.produites, 1);
        assert_eq!(a.hors_decoupage, 0);

        // Rejouee, elle ne duplique rien.
        let b = synchroniser(&db, &cover, &bytes, None).expect("sync 2");
        assert_eq!(a, b);
        let conn = rusqlite::Connection::open(&db).expect("relecture");
        let n: i64 = conn
            .query_row("SELECT count(*) FROM forge_unit", [], |r| r.get(0))
            .expect("compte");
        assert_eq!(n, 1, "la table est reecrite, pas empilee");
        let statut: String = conn
            .query_row("SELECT statut FROM v_forge_function", [], |r| r.get(0))
            .expect("vue");
        assert_eq!(statut, statut::PRODUIT, "la vue joint les deux inventaires");
    }
}
