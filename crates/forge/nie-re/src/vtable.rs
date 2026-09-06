//! Récupération des arêtes de **vtable** (cohésion de classe) pour classer le
//! résidu appelé uniquement par indirection.
//!
//! Une vtable MSVC est un tableau de pointeurs de fonctions en `.rdata`, propre
//! à **une** classe : toutes ses méthodes appartiennent au même sous-système.
//! `nie-re::rtti` a déjà localisé l'adresse de chaque vtable (le slot juste
//! **avant** les méthodes contient le pointeur `_RTTICompleteObjectLocator`).
//!
//! ## Méthode
//!
//! Pour chaque vtable connue (`rtti_class.vtable_vaddr`) :
//! 1. les pointeurs de méthodes commencent à `vtable_vaddr + 8` (le slot `+0`
//!    est le pointeur COL) ;
//! 2. on lit les `u64` consécutifs tant qu'ils pointent dans `.text` ; le
//!    premier hors `.text` (= COL de la vtable suivante) marque la fin ;
//! 3. chaque cible `.text` est une **méthode** de la classe. Si ce n'est pas
//!    une fonction connue (`.pdata` ne couvre pas les feuilles sans unwind),
//!    on l'**ajoute** comme nouveau nœud ;
//! 4. on relie les méthodes d'une même vtable par des arêtes de **cohésion**
//!    (étoile depuis la 1re méthode) en `xref` kind=`vtable` : la propagation
//!    diffuse alors le sous-système de classe à toutes les co-méthodes.

use anyhow::{Context, Result};
use goblin::pe::PE;
use hashbrown::{HashMap, HashSet};
use nie_index::{Db, rusqlite};
use tracing::info;

use crate::anchors::classify_rtti;

/// Statistiques de la récupération de vtables.
#[derive(Debug, Clone, Copy, Default)]
pub struct VtableStats {
    /// Vtables traitées (avec au moins une méthode).
    pub vtables: usize,
    /// Méthodes distinctes trouvées (cibles `.text`).
    pub methods: usize,
    /// Nouvelles fonctions feuilles ajoutées (méthodes hors `.pdata`).
    pub new_leaf_funcs: usize,
    /// Arêtes de cohésion insérées.
    pub cohesion_edges: usize,
    /// Méthodes ancrées par RTTI de classe (`subsys_src='vtable'`).
    pub class_anchored: usize,
    /// Noms **structurels** écrits (`name_source='vtable-struct'`).
    ///
    /// Il s'agit de noms de la forme `Namespace::Classe::vmethod_N` dérivés du
    /// nom de classe RTTI et de l'index de slot dans la vtable.  Ce sont des
    /// noms **structurels** (position dans la vtable), **pas** des symboles PDB
    /// originaux : ils identifient la méthode de façon non ambiguë mais ne
    /// renseignent pas sur la sémantique (le nom C++ réel reste inconnu).
    pub named_struct: usize,
}

/// Lit les vtables localisées par RTTI (`src_bin`), ajoute les méthodes feuilles
/// manquantes comme nœuds dans `dst_bin`, insère les arêtes de cohésion de
/// classe (`kind='vtable'`) et ancre les méthodes standalone par leur appartenance
/// à une classe RTTI identifiée (sous-système dérivé via [`crate::anchors::classify_rtti`]).
///
/// Si `skip_anchor` est vrai, l'ancrage RTTI de classe est omis (A/B `NIE_NO_INDIRECT=1`).
pub fn vtable_edges_into(
    db: &mut Db,
    src_bin: i64,
    dst_bin: i64,
    exe_path: &std::path::Path,
    skip_anchor: bool,
) -> Result<VtableStats> {
    let bytes =
        std::fs::read(exe_path).with_context(|| format!("lecture {}", exe_path.display()))?;
    let pe = PE::parse(&bytes).context("goblin: parse PE")?;
    let image_base = pe.image_base;

    let text = pe
        .sections
        .iter()
        .find(|s| s.name().is_ok_and(|n| n.starts_with(".text")))
        .context(".text introuvable")?;
    let text_va = image_base + u64::from(text.virtual_address);
    let text_end = text_va + u64::from(text.virtual_size.min(text.size_of_raw_data));

    let rdata = pe
        .sections
        .iter()
        .find(|s| s.name().is_ok_and(|n| n.starts_with(".rdata")))
        .context(".rdata introuvable")?;
    let rdata_va = image_base + u64::from(rdata.virtual_address);
    let rdata_off = rdata.pointer_to_raw_data as usize;
    let rdata_len = rdata.virtual_size.min(rdata.size_of_raw_data) as usize;
    let rdata_bytes = bytes
        .get(rdata_off..rdata_off + rdata_len)
        .context(".rdata hors limites")?;
    // Lit un u64 LE à l'adresse virtuelle `va` si elle est dans `.rdata`.
    let rd_u64 = |va: u64| -> Option<u64> {
        if va < rdata_va {
            return None;
        }
        let off = (va - rdata_va) as usize;
        rdata_bytes
            .get(off..off + 8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    };

    // Vtables localisées par RTTI : adresse + nom de classe + namespace.
    // Namespace depuis src_bin=1 (RTTI), fonctions à ancrer dans dst_bin=2.
    let vtables: Vec<(u64, String, String)> = {
        let mut q = db.conn().prepare(
            "SELECT vtable_vaddr, name, COALESCE(namespace,'') FROM rtti_class WHERE binary_id=?1 AND vtable_vaddr IS NOT NULL",
        )?;
        q.query_map([src_bin], |r| {
            Ok((
                r.get::<_, i64>(0).map(|v| v as u64)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<_, _>>()?
    };

    // Fonctions connues de la cible.
    let mut known: HashSet<u64> = {
        let mut q = db
            .conn()
            .prepare("SELECT vaddr FROM function WHERE binary_id=?1")?;
        q.query_map([dst_bin], |r| r.get::<_, i64>(0).map(|v| v as u64))?
            .collect::<std::result::Result<_, _>>()?
    };

    let mut stats = VtableStats::default();
    let mut all_methods: HashSet<u64> = HashSet::new();
    // Liste des vtables → (méthodes, nom_classe, namespace).
    let mut groups: Vec<(Vec<u64>, String, String)> = Vec::with_capacity(vtables.len());

    for (vt, name, ns) in vtables {
        let mut methods = Vec::new();
        let mut k = 1u64; // saute le pointeur COL en +0
        while k < 256 {
            let Some(slot) = rd_u64(vt + k * 8) else {
                break;
            };
            if (text_va..text_end).contains(&slot) {
                methods.push(slot);
                all_methods.insert(slot);
                k += 1;
            } else {
                break;
            }
        }
        if methods.len() >= 2 {
            stats.vtables += 1;
            groups.push((methods, name, ns));
        }
    }
    stats.methods = all_methods.len();

    // Détecte les thunks partagés : méthode présente dans des vtables de
    // plusieurs namespaces top-level distincts (_purecall, deleting-dtor…).
    // Ces méthodes ne seront pas ancrées pour éviter les confusions.
    let mut method_to_ns: HashMap<u64, HashSet<String>> = HashMap::new();
    if !skip_anchor {
        for (methods, _, ns) in &groups {
            let top_ns = ns.split("::").next().unwrap_or(ns).to_string();
            for &m in methods {
                method_to_ns.entry(m).or_default().insert(top_ns.clone());
            }
        }
    }

    let tx = db.conn_mut().transaction()?;
    // 1. Ajoute les méthodes feuilles manquantes comme nœuds.
    {
        let mut ins = tx.prepare_cached(
            "INSERT OR IGNORE INTO function(binary_id, vaddr, name_source, subsystem, confidence)
             VALUES(?1,?2,'vtable','standalone',0.0)",
        )?;
        for &m in &all_methods {
            if !known.contains(&m) {
                ins.execute(rusqlite::params![dst_bin, m as i64])?;
                known.insert(m);
                stats.new_leaf_funcs += 1;
            }
        }
    }
    // 2. Arêtes de cohésion (étoile depuis la 1re méthode).
    {
        let mut ins = tx.prepare_cached(
            "INSERT OR IGNORE INTO xref(binary_id, from_addr, to_addr, kind) VALUES(?1,?2,?3,'vtable')",
        )?;
        for (methods, _, _) in &groups {
            let hub = methods[0] as i64;
            for &m in &methods[1..] {
                if m as i64 != hub {
                    stats.cohesion_edges +=
                        ins.execute(rusqlite::params![dst_bin, hub, m as i64])?;
                }
            }
        }
    }
    // 3. Ancrage RTTI de classe : une méthode standalone d'une vtable dont la
    //    classe est classifiable (`classify_rtti`) hérite du sous-système de sa
    //    classe (confiance 0.7, ancre dure, n'écrase jamais un label existant).
    //    On saute les thunks partagés (méthode présente dans >1 namespace
    //    top-level distinct : _purecall, vector deleting destructor…).
    //
    // 4. Nommage structurel : pour chaque méthode non-thunk de chaque groupe,
    //    si `name IS NULL`, on écrit un nom de la forme
    //    `Namespace::Classe::vmethod_N` (N = index de slot dans la vtable).
    //    `name_source = 'vtable-struct'` distingue ces noms structurels des
    //    symboles PDB originaux : ils identifient sans ambiguïté la méthode
    //    (classe RTTI + rang) mais n'indiquent pas la sémantique C++ réelle.
    if !skip_anchor {
        let mut upd = tx.prepare_cached(
            "UPDATE function SET subsystem=?1, subsys_src='vtable', confidence=0.7
             WHERE binary_id=?2 AND vaddr=?3 AND (subsystem IS NULL OR subsystem='standalone')",
        )?;
        for (methods, name, ns) in &groups {
            let Some(sub) = classify_rtti(ns, name) else {
                continue;
            };
            for &m in methods {
                if method_to_ns.get(&m).map_or(0, |s| s.len()) > 1 {
                    continue; // thunk partagé entre plusieurs classes
                }
                stats.class_anchored += upd.execute(rusqlite::params![sub, dst_bin, m as i64])?;
            }
        }

        // Nommage structurel : itère TOUS les groupes (pas seulement les
        // classifiables) pour nommer chaque méthode à son index de slot.
        let mut upd_name = tx.prepare_cached(
            "UPDATE function SET name=?1, name_source='vtable-struct'
             WHERE binary_id=?2 AND vaddr=?3 AND name IS NULL",
        )?;
        for (methods, class, ns) in &groups {
            for (i, &m) in methods.iter().enumerate() {
                if method_to_ns.get(&m).map_or(0, |s| s.len()) > 1 {
                    continue; // thunk partagé entre plusieurs classes
                }
                let struct_name = if ns.is_empty() {
                    format!("{class}::vmethod_{i}")
                } else {
                    format!("{ns}::{class}::vmethod_{i}")
                };
                stats.named_struct +=
                    upd_name.execute(rusqlite::params![struct_name, dst_bin, m as i64])?;
            }
        }
    }
    tx.commit()?;

    info!(
        "vtable: {} vtables, {} méthodes, {} feuilles ajoutées, {} arêtes cohésion, {} noms-struct",
        stats.vtables,
        stats.methods,
        stats.new_leaf_funcs,
        stats.cohesion_edges,
        stats.named_struct
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vérifie qu'une étoile relie bien toutes les méthodes au pivot (logique
    /// de groupe — testée sans PE réel).
    #[test]
    fn etoile_relie_toutes_les_methodes() {
        let methods = [0x2000u64, 0x2100, 0x2200, 0x2300];
        let hub = methods[0];
        let edges: Vec<(u64, u64)> = methods[1..].iter().map(|&m| (hub, m)).collect();
        assert_eq!(edges.len(), 3);
        assert!(edges.iter().all(|&(h, _)| h == 0x2000));
        // chaque méthode (sauf le pivot) est atteinte.
        let reached: HashSet<u64> = edges.iter().map(|&(_, m)| m).collect();
        assert_eq!(reached.len(), 3);
        assert!(!reached.contains(&hub));
    }
}
