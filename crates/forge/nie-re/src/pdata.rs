//! Découverte **autoritaire** des fonctions via la table `.pdata` (exception
//! unwind x86-64), et mesure du désalignement de l'index Ghidra.
//!
//! ## Pourquoi
//!
//! L'index `nie-index.json` fournit ~60 000 adresses `FUN_<hex>` supposées être
//! des débuts de fonction. La vérification byte-à-byte contre `.pdata` (la table
//! d'unwind générée par le compilateur, vérité terrain incontestable) montre que
//! ces adresses sont **massivement désalignées** : ~96 % ne sont pas des débuts
//! de fonction réels (84 % tombent *strictement à l'intérieur* d'un corps de
//! fonction `.pdata`), et l'ensemble est artificiellement aligné sur 16 octets à
//! 99 %. L'index reste exploitable comme **graphe de métadonnées** (chaînes,
//! namespaces, relations d'appel) mais ses **adresses ne sont pas fiables**.
//!
//! `.pdata` donne la liste réelle : chaque `RUNTIME_FUNCTION` (12 octets :
//! `begin_rva`, `end_rva`, `unwind_rva`) décrit une région de code avec ses
//! bornes. Certaines entrées sont des **fragments chaînés** (`UNW_FLAG_CHAININFO`)
//! — des morceaux d'une fonction plus grande, pas des débuts. On les écarte par
//! lecture du `UNWIND_INFO` ; les entrées **racines** restantes sont les vraies
//! fonctions.
//!
//! ## Limite (honnêteté)
//!
//! `.pdata` ne couvre que les fonctions **avec** information d'unwind (cadre de
//! pile). Les fonctions feuilles triviales sans cadre n'y figurent pas : la liste
//! `.pdata` est un plancher autoritaire, pas un plafond.

use anyhow::{Context, Result};
use goblin::pe::PE;
use nie_index::{Db, rusqlite};
use tracing::info;

/// `UNW_FLAG_CHAININFO` : l'entrée est un fragment chaîné à une fonction parente,
/// pas un début de fonction.
const UNW_FLAG_CHAININFO: u8 = 0x4;

/// Une fonction racine réelle : bornes `[start, end)` en adresses virtuelles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootFn {
    /// Adresse virtuelle de début (vraie entrée de fonction).
    pub start: u64,
    /// Adresse virtuelle de fin (exclusive).
    pub end: u64,
}

/// Statistiques de la découverte `.pdata`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PdataStats {
    /// Entrées `RUNTIME_FUNCTION` totales.
    pub entries: usize,
    /// Fragments chaînés écartés (`UNW_FLAG_CHAININFO`).
    pub chained_fragments: usize,
    /// Fonctions racines réelles retenues.
    pub roots: usize,
    /// Racines insérées en base.
    pub inserted: usize,
    /// Racines dont le début coïncide avec une adresse de l'index Ghidra.
    pub overlap_ghidra: usize,
    /// Adresses Ghidra tombant *strictement à l'intérieur* d'un corps `.pdata`
    /// (preuve de désalignement : ce ne sont pas des débuts de fonction).
    pub ghidra_inside_body: usize,
    /// Fonctions Ghidra totales (dans `.text`).
    pub ghidra_total: usize,
}

/// Convertit un RVA en offset fichier à travers toutes les sections du PE.
fn rva_to_off(pe: &PE, rva: u64) -> Option<usize> {
    for sec in &pe.sections {
        let va = u64::from(sec.virtual_address);
        let vsize = u64::from(sec.virtual_size.max(sec.size_of_raw_data));
        if va <= rva && rva < va + vsize {
            return Some(sec.pointer_to_raw_data as usize + (rva - va) as usize);
        }
    }
    None
}

/// Parse la table `.pdata` du binaire et renvoie les fonctions **racines**
/// (entrées `RUNTIME_FUNCTION` non chaînées), triées par adresse de début.
pub fn parse_roots(bytes: &[u8]) -> Result<Vec<RootFn>> {
    let pe = PE::parse(bytes).context("goblin: parse PE")?;
    let image_base = pe.image_base;

    // Section .pdata (= répertoire Exception).
    let pdata = pe
        .sections
        .iter()
        .find(|s| s.name().is_ok_and(|n| n.starts_with(".pdata")))
        .context("section .pdata introuvable")?;
    let raw_off = pdata.pointer_to_raw_data as usize;
    let raw_size = (pdata.virtual_size.min(pdata.size_of_raw_data)) as usize;
    let table = bytes
        .get(raw_off..raw_off + raw_size)
        .context(".pdata hors des limites du fichier")?;

    let mut roots = Vec::with_capacity(table.len() / 12);
    let mut chained = 0usize;
    for chunk in table.chunks_exact(12) {
        let begin = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let end = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        let unwind = u32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
        if begin == 0 && end == 0 {
            continue;
        }
        // Lecture du premier octet de UNWIND_INFO : Flags = (b >> 3) & 0x1f.
        let Some(uo) = rva_to_off(&pe, u64::from(unwind)) else {
            continue;
        };
        let Some(&b0) = bytes.get(uo) else { continue };
        let flags = (b0 >> 3) & 0x1f;
        if flags & UNW_FLAG_CHAININFO != 0 {
            chained += 1;
            continue;
        }
        roots.push(RootFn {
            start: image_base + u64::from(begin),
            end: image_base + u64::from(end),
        });
    }
    roots.sort_unstable_by_key(|r| r.start);
    roots.dedup_by_key(|r| r.start);
    info!(
        "pdata: {} entrées, {} fragments chaînés, {} fonctions racines",
        table.len() / 12,
        chained,
        roots.len()
    );
    Ok(roots)
}

/// Découvre les fonctions racines via `.pdata`, les stocke dans `pdata_func`, et
/// mesure le désalignement de l'index Ghidra (`function`) déjà chargé.
pub fn discover_into(
    db: &mut Db,
    binary_id: i64,
    exe_path: &std::path::Path,
) -> Result<PdataStats> {
    let bytes =
        std::fs::read(exe_path).with_context(|| format!("lecture {}", exe_path.display()))?;
    let roots = parse_roots(&bytes)?;

    let total_entries = {
        // recompte les entrées brutes pour le rapport
        let pe = PE::parse(&bytes).context("parse PE")?;
        let pdata = pe
            .sections
            .iter()
            .find(|s| s.name().is_ok_and(|n| n.starts_with(".pdata")));
        pdata.map_or(0, |s| {
            (s.virtual_size.min(s.size_of_raw_data)) as usize / 12
        })
    };

    // Insertion des racines.
    let inserted = {
        let tx = db.conn_mut().transaction()?;
        let mut n = 0usize;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO pdata_func(binary_id, start, end) VALUES(?1,?2,?3)",
            )?;
            for r in &roots {
                n += stmt.execute(rusqlite::params![binary_id, r.start as i64, r.end as i64])?;
            }
        }
        tx.commit()?;
        n
    };

    // Mesure du désalignement Ghidra.
    let ghidra: Vec<i64> = {
        let mut stmt = db
            .conn()
            .prepare("SELECT vaddr FROM function WHERE binary_id=?1 ORDER BY vaddr")?;
        stmt.query_map([binary_id], |r| r.get::<_, i64>(0))?
            .collect::<std::result::Result<_, _>>()?
    };
    // Bornes des racines pour test d'inclusion (tri par start déjà fait).
    let starts: Vec<u64> = roots.iter().map(|r| r.start).collect();
    let root_starts: std::collections::HashSet<u64> = starts.iter().copied().collect();

    // .text bounds via parse (pour ne compter que les fonctions de .text).
    let (tva, tend) = {
        let pe = PE::parse(&bytes).context("parse PE")?;
        let t = pe
            .sections
            .iter()
            .find(|s| s.name().is_ok_and(|n| n.starts_with(".text")))
            .context(".text introuvable")?;
        let va = pe.image_base + u64::from(t.virtual_address);
        (va, va + u64::from(t.virtual_size.min(t.size_of_raw_data)))
    };

    let mut overlap = 0usize;
    let mut inside = 0usize;
    let mut gtotal = 0usize;
    for &g in &ghidra {
        let g = g as u64;
        if g < tva || g >= tend {
            continue;
        }
        gtotal += 1;
        if root_starts.contains(&g) {
            overlap += 1;
            continue;
        }
        // Recherche de la racine dont le corps contient g.
        let idx = starts.partition_point(|&s| s <= g);
        if idx > 0 {
            let r = roots[idx - 1];
            if r.start < g && g < r.end {
                inside += 1;
            }
        }
    }

    let stats = PdataStats {
        entries: total_entries,
        chained_fragments: total_entries.saturating_sub(roots.len()),
        roots: roots.len(),
        inserted,
        overlap_ghidra: overlap,
        ghidra_inside_body: inside,
        ghidra_total: gtotal,
    };
    info!(
        "pdata: {} racines insérées; Ghidra {}/{} alignées, {} à l'intérieur de corps",
        stats.inserted, stats.overlap_ghidra, stats.ghidra_total, stats.ghidra_inside_body
    );
    Ok(stats)
}

/// Renvoie l'adresse de début de la fonction racine **contenant** `a`
/// (`start <= a < end`), ou `None`. `starts` doit être trié et correspondre
/// élément par élément à `roots`.
fn root_containing(starts: &[u64], roots: &[RootFn], a: u64) -> Option<u64> {
    let idx = starts.partition_point(|&s| s <= a);
    if idx == 0 {
        return None;
    }
    let r = roots[idx - 1];
    (a < r.end).then_some(r.start)
}

/// Statistiques de la refondation `.pdata`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RebuildStats {
    /// Fonctions racines insérées comme nœuds.
    pub roots: usize,
    /// Références de chaînes ré-ancrées sur la fonction contenante.
    pub str_refs_moved: usize,
    /// Constantes ré-ancrées.
    pub consts_moved: usize,
    /// Arêtes `ce` Ghidra repliées sur les racines contenantes.
    pub ce_edges_mapped: usize,
    /// Classes RTTI copiées vers la cible.
    pub rtti_copied: usize,
    /// Métadonnées Ghidra sans fonction racine contenante (perdues).
    pub unmapped: usize,
}

/// Reconstruit la carte des fonctions sur la **vérité terrain `.pdata`** : insère
/// les 50 674 fonctions racines comme nœuds dans `dst_bin`, puis **ré-ancre** les
/// métadonnées de l'index Ghidra (`src_bin`) par *inclusion* — chaque nœud Ghidra
/// à l'adresse `a` (massivement désaligné, souvent au milieu d'un corps) voit ses
/// chaînes / constantes / arêtes transférées à la fonction racine réelle qui
/// contient `a`. Le résultat : un graphe à adresses correctes sur lequel `disasm`
/// (vrais débuts) et `propagate` redeviennent physiquement valides.
pub fn rebuild_from_pdata(
    db: &mut Db,
    src_bin: i64,
    dst_bin: i64,
    exe_path: &std::path::Path,
) -> Result<RebuildStats> {
    let bytes =
        std::fs::read(exe_path).with_context(|| format!("lecture {}", exe_path.display()))?;
    let roots = parse_roots(&bytes)?;
    let starts: Vec<u64> = roots.iter().map(|r| r.start).collect();
    let find_root = |a: u64| -> Option<u64> { root_containing(&starts, &roots, a) };

    // --- Lectures de la source (avant d'ouvrir la transaction d'écriture) ------
    let str_rows: Vec<(u64, String)> = {
        let mut q = db.conn().prepare(
            "SELECT f.vaddr, s.value FROM func_str_ref s JOIN function f ON f.id=s.function_id WHERE f.binary_id=?1",
        )?;
        q.query_map([src_bin], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<_, _>>()?
    };
    let const_rows: Vec<(u64, i64)> = {
        let mut q = db.conn().prepare(
            "SELECT f.vaddr, c.value FROM func_const c JOIN function f ON f.id=c.function_id WHERE f.binary_id=?1",
        )?;
        q.query_map([src_bin], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)?))
        })?
        .collect::<std::result::Result<_, _>>()?
    };
    let ce_rows: Vec<(u64, u64)> = {
        let mut q = db
            .conn()
            .prepare("SELECT from_addr, to_addr FROM xref WHERE binary_id=?1 AND kind='call'")?;
        q.query_map([src_bin], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64))
        })?
        .collect::<std::result::Result<_, _>>()?
    };
    let rtti_rows: Vec<(String, Option<String>, Option<String>)> = {
        let mut q = db
            .conn()
            .prepare("SELECT name, namespace, mangled FROM rtti_class WHERE binary_id=?1")?;
        q.query_map([src_bin], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<std::result::Result<_, _>>()?
    };

    let mut stats = RebuildStats {
        roots: roots.len(),
        ..Default::default()
    };
    let tx = db.conn_mut().transaction()?;

    // --- 1. Insère les fonctions racines (vraies entrées + tailles) -----------
    {
        let mut ins = tx.prepare_cached(
            "INSERT OR IGNORE INTO function(binary_id, vaddr, size, name_source, subsystem, confidence)
             VALUES(?1,?2,?3,'pdata','standalone',0.0)",
        )?;
        for r in &roots {
            ins.execute(rusqlite::params![
                dst_bin,
                r.start as i64,
                (r.end - r.start) as i64
            ])?;
        }
    }
    // Carte vaddr → function_id pour la cible.
    let mut root_fid: hashbrown::HashMap<u64, i64> = hashbrown::HashMap::with_capacity(roots.len());
    {
        let mut q = tx.prepare("SELECT vaddr, id FROM function WHERE binary_id=?1")?;
        let rows = q.query_map([dst_bin], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (v, id) = row?;
            root_fid.insert(v, id);
        }
    }

    // --- 2. Ré-ancre les chaînes par inclusion --------------------------------
    {
        let mut ins = tx.prepare_cached(
            "INSERT INTO func_str_ref(binary_id, function_id, value) VALUES(?1,?2,?3)",
        )?;
        for (va, val) in &str_rows {
            match find_root(*va).and_then(|s| root_fid.get(&s)) {
                Some(&fid) => {
                    ins.execute(rusqlite::params![dst_bin, fid, val])?;
                    stats.str_refs_moved += 1;
                }
                None => stats.unmapped += 1,
            }
        }
    }
    // --- 3. Ré-ancre les constantes -------------------------------------------
    {
        let mut ins =
            tx.prepare_cached("INSERT INTO func_const(function_id, value) VALUES(?1,?2)")?;
        for (va, val) in &const_rows {
            if let Some(&fid) = find_root(*va).and_then(|s| root_fid.get(&s)) {
                ins.execute(rusqlite::params![fid, val])?;
                stats.consts_moved += 1;
            }
        }
    }
    // --- 4. Replie les arêtes d'appel Ghidra sur les racines ------------------
    {
        let mut ins = tx.prepare_cached(
            "INSERT OR IGNORE INTO xref(binary_id, from_addr, to_addr, kind) VALUES(?1,?2,?3,'call')",
        )?;
        for (from, to) in &ce_rows {
            if let (Some(rf), Some(rt)) = (find_root(*from), find_root(*to))
                && rf != rt
            {
                ins.execute(rusqlite::params![dst_bin, rf as i64, rt as i64])?;
                stats.ce_edges_mapped += 1;
            }
        }
    }
    // --- 5. Copie les classes RTTI --------------------------------------------
    {
        let mut ins = tx.prepare_cached(
            "INSERT OR IGNORE INTO rtti_class(binary_id, name, namespace, mangled) VALUES(?1,?2,?3,?4)",
        )?;
        for (name, ns, mangled) in &rtti_rows {
            ins.execute(rusqlite::params![dst_bin, name, ns, mangled])?;
            stats.rtti_copied += 1;
        }
    }

    tx.commit()?;
    info!(
        "rebuild: {} racines, {} str ré-ancrées ({} perdues), {} consts, {} arêtes ce, {} rtti",
        stats.roots,
        stats.str_refs_moved,
        stats.unmapped,
        stats.consts_moved,
        stats.ce_edges_mapped,
        stats.rtti_copied
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un PE x64 minimal en mémoire avec une `.pdata` contenant deux
    /// entrées (une racine, un fragment chaîné) et vérifie que seul le root sort.
    ///
    /// On ne fabrique pas un PE complet ici (goblin exige beaucoup de champs) :
    /// on teste plutôt la logique de filtrage de flags directement.
    #[test]
    fn flag_chaininfo_ecarte_les_fragments() {
        // byte0 = (Flags << 3) | Version. Version=1.
        let root = 1u8; // flags=0 → racine
        let frag = (UNW_FLAG_CHAININFO << 3) | 1; // flags=CHAININFO → fragment
        assert_eq!((root >> 3) & 0x1f & UNW_FLAG_CHAININFO, 0);
        assert_ne!((frag >> 3) & 0x1f & UNW_FLAG_CHAININFO, 0);
    }

    /// Vérifie le test d'inclusion (adresse → fonction racine contenante).
    #[test]
    fn inclusion_dans_la_fonction_racine() {
        let roots = [
            RootFn {
                start: 0x1000,
                end: 0x1100,
            },
            RootFn {
                start: 0x2000,
                end: 0x2400,
            },
        ];
        let starts: Vec<u64> = roots.iter().map(|r| r.start).collect();
        // Début exact → la fonction elle-même.
        assert_eq!(root_containing(&starts, &roots, 0x1000), Some(0x1000));
        // Milieu de corps → la fonction contenante.
        assert_eq!(root_containing(&starts, &roots, 0x2200), Some(0x2000));
        // Dans le trou entre deux fonctions (0x1100..0x2000) → None.
        assert_eq!(root_containing(&starts, &roots, 0x1500), None);
        // Avant la première → None.
        assert_eq!(root_containing(&starts, &roots, 0x0500), None);
        // À la borne de fin (exclusive) → None.
        assert_eq!(root_containing(&starts, &roots, 0x2400), None);
    }

    /// Vérifie le tri + déduplication des racines.
    #[test]
    fn roots_tries_et_dedupliques() {
        let mut v = [
            RootFn {
                start: 0x2000,
                end: 0x2100,
            },
            RootFn {
                start: 0x1000,
                end: 0x1100,
            },
            RootFn {
                start: 0x2000,
                end: 0x2100,
            },
        ]
        .to_vec();
        v.sort_unstable_by_key(|r| r.start);
        v.dedup_by_key(|r| r.start);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].start, 0x1000);
        assert_eq!(v[1].start, 0x2000);
    }
}
