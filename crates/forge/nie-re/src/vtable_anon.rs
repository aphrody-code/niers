//! Tables de pointeurs de fonctions **sans RTTI** : cohésion de classe pour le
//! résidu que `nie-re::vtable` ne voit pas.
//!
//! `vtable.rs` part des vtables **localisées par RTTI** (le slot `-8` porte le
//! `_RTTICompleteObjectLocator`) : 1 745 tables, 7 549 méthodes. Or `.rdata` et
//! `.data` de `nie.exe` contiennent **3 071** suites d'au moins trois pointeurs
//! `.text` consécutifs, totalisant 74 877 slots. Les **1 521 tables restantes
//! (37 334 slots)** n'ont pas de COL : classes compilées sans RTTI, tables de
//! rappels, tables de méthodes d'interface. Leurs méthodes appartiennent
//! néanmoins à une même unité de code — c'est exactement le signal dont la
//! propagation de labels a besoin.
//!
//! ## Méthode
//!
//! 1. Balayer les sections de données par pas de 8 octets ; une suite d'au
//!    moins `MIN_SLOTS` pointeurs tombant dans `.text` est une table candidate.
//! 2. Écarter celles déjà couvertes par une classe RTTI (`vtable.rs` les
//!    traite, avec un vrai nom de classe en prime).
//! 3. Ingérer les cibles absentes comme fonctions, relier les méthodes d'une
//!    même table par des arêtes de cohésion `kind='vtable'` (étoile depuis le
//!    premier slot), et nommer structurellement `vtbl_<va>::slot_N`.
//!
//! ## Honnêteté
//!
//! - Une table anonyme n'a **pas** de séparateur : deux vtables adjacentes de
//!   classes différentes se lisent comme une seule suite. Le groupe de cohésion
//!   peut donc réunir deux classes voisines. C'est du bruit borné — la
//!   propagation vote à la majorité — mais ce n'est pas une certitude, et le
//!   nom écrit ne prétend jamais nommer une classe : il désigne l'adresse de la
//!   table et le rang du slot.
//! - Une suite de pointeurs `.text` n'est pas forcément une vtable (table de
//!   rappels, table de sauts absolue). Le seuil `MIN_SLOTS` limite les
//!   coïncidences sans les exclure ; le compte rapporté est celui des tables
//!   réellement retenues, pas une estimation.

use anyhow::{Context, Result};
use goblin::pe::PE;
use hashbrown::HashSet;
use nie_index::{Db, rusqlite};
use tracing::info;

/// Nombre minimal de pointeurs `.text` consécutifs pour retenir une table.
///
/// À deux slots, les coïncidences (deux pointeurs de fonction voisins dans une
/// structure de données) dominent ; à trois, la suite est déjà un objet
/// structuré.
const MIN_SLOTS: usize = 3;

/// Nombre maximal de slots lus d'une seule table (garde-fou contre une
/// immense table de rappels qui noierait la cohésion).
const MAX_SLOTS: usize = 512;

/// Statistiques de la passe « vtables anonymes ».
#[derive(Debug, Clone, Copy, Default)]
pub struct AnonVtableStats {
    /// Suites de pointeurs `.text` trouvées dans les sections de données.
    pub tables_seen: usize,
    /// Tables retenues (hors vtables déjà couvertes par le RTTI).
    pub tables: usize,
    /// Slots des tables retenues.
    pub slots: usize,
    /// Méthodes distinctes visées par ces tables.
    pub methods: usize,
    /// Fonctions ajoutées (cible de slot inconnue jusqu'ici).
    pub new_funcs: usize,
    /// Arêtes de cohésion insérées.
    pub cohesion_edges: usize,
    /// Noms structurels `vtbl_<va>::slot_N` écrits.
    pub named: usize,
}

/// Ingère les tables de pointeurs de fonctions sans RTTI de `exe_path` dans
/// `dst_bin`, en excluant celles que le RTTI de `rtti_bin` couvre déjà.
///
/// # Errors
///
/// Échoue si le PE est illisible, si `.text` manque, ou sur toute erreur
/// SQLite pendant l'ingestion.
pub fn anon_vtable_edges_into(
    db: &mut Db,
    rtti_bin: i64,
    dst_bin: i64,
    exe_path: &std::path::Path,
) -> Result<AnonVtableStats> {
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

    // Vtables déjà localisées par RTTI : on tolère un décalage d'un slot, le
    // RTTI enregistrant tantôt le COL tantôt le premier slot de méthode.
    let rtti: HashSet<u64> = {
        let mut q = db.conn().prepare(
            "SELECT vtable_vaddr FROM rtti_class WHERE binary_id=?1 AND vtable_vaddr IS NOT NULL",
        )?;
        q.query_map([rtti_bin], |r| r.get::<_, i64>(0).map(|v| v as u64))?
            .collect::<std::result::Result<_, _>>()?
    };

    let mut known: HashSet<u64> = {
        let mut q = db
            .conn()
            .prepare("SELECT vaddr FROM function WHERE binary_id=?1")?;
        q.query_map([dst_bin], |r| r.get::<_, i64>(0).map(|v| v as u64))?
            .collect::<std::result::Result<_, _>>()?
    };

    let mut stats = AnonVtableStats::default();
    let mut groups: Vec<(u64, Vec<u64>)> = Vec::new();
    let mut all_methods: HashSet<u64> = HashSet::new();

    for name in [".rdata", ".data", "_RDATA", ".fptable", ".rodata"] {
        let Some(sec) = pe
            .sections
            .iter()
            .find(|s| s.name().is_ok_and(|n| n.starts_with(name)))
        else {
            continue;
        };
        let base = image_base + u64::from(sec.virtual_address);
        let off = sec.pointer_to_raw_data as usize;
        let len = sec.virtual_size.min(sec.size_of_raw_data) as usize;
        let Some(raw) = bytes.get(off..off + len) else {
            continue;
        };

        let mut i = 0usize;
        while i + 8 <= raw.len() {
            let v = u64::from_le_bytes(raw[i..i + 8].try_into().unwrap());
            if !(text_va..text_end).contains(&v) {
                i += 8;
                continue;
            }
            // Longueur de la suite de pointeurs `.text`.
            let mut slots = Vec::with_capacity(8);
            let mut j = i;
            while j + 8 <= raw.len() && slots.len() < MAX_SLOTS {
                let w = u64::from_le_bytes(raw[j..j + 8].try_into().unwrap());
                if !(text_va..text_end).contains(&w) {
                    break;
                }
                slots.push(w);
                j += 8;
            }
            if slots.len() >= MIN_SLOTS {
                stats.tables_seen += 1;
                let va = base + i as u64;
                // Déjà traitée par le RTTI ? (COL en -8, ou table enregistrée
                // directement sur son premier slot).
                let covered = rtti.contains(&va)
                    || rtti.contains(&va.wrapping_sub(8))
                    || rtti.contains(&(va + 8));
                if !covered {
                    stats.tables += 1;
                    stats.slots += slots.len();
                    for &m in &slots {
                        all_methods.insert(m);
                    }
                    groups.push((va, slots));
                }
            }
            i = j.max(i + 8);
        }
    }
    stats.methods = all_methods.len();

    let tx = db.conn_mut().transaction()?;
    {
        let mut ins = tx.prepare_cached(
            "INSERT OR IGNORE INTO function(binary_id, vaddr, name_source, subsystem, subsys_src, confidence)
             VALUES(?1,?2,'vtable-anon','standalone','vtable-anon',0.0)",
        )?;
        for &m in &all_methods {
            if !known.contains(&m) {
                ins.execute(rusqlite::params![dst_bin, m as i64])?;
                known.insert(m);
                stats.new_funcs += 1;
            }
        }
    }
    {
        let mut ins = tx.prepare_cached(
            "INSERT OR IGNORE INTO xref(binary_id, from_addr, to_addr, kind) VALUES(?1,?2,?3,'vtable')",
        )?;
        for (_, slots) in &groups {
            let hub = slots[0] as i64;
            for &m in &slots[1..] {
                if m as i64 != hub {
                    stats.cohesion_edges +=
                        ins.execute(rusqlite::params![dst_bin, hub, m as i64])?;
                }
            }
        }
    }
    {
        // Nom structurel : l'adresse de la table et le rang du slot. Il ne
        // nomme pas une classe — aucune n'est connue ici — mais il identifie
        // la méthode sans ambiguïté et rend la table lisible.
        let mut upd = tx.prepare_cached(
            "UPDATE function SET name=?1, name_source='vtable-anon-struct'
             WHERE binary_id=?2 AND vaddr=?3 AND name IS NULL",
        )?;
        for (va, slots) in &groups {
            for (k, &m) in slots.iter().enumerate() {
                let nm = format!("vtbl_{va:x}::slot_{k}");
                stats.named += upd.execute(rusqlite::params![nm, dst_bin, m as i64])?;
            }
        }
    }
    tx.commit()?;

    info!(
        tables = stats.tables,
        slots = stats.slots,
        methods = stats.methods,
        edges = stats.cohesion_edges,
        "vtable-anon: tables sans RTTI ingérées"
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le seuil doit écarter une paire de pointeurs voisins mais retenir une
    /// vraie suite : c'est ce qui sépare une structure de données d'une table.
    #[test]
    fn seuil_minimal_de_slots() {
        // Une suite de 2 est une coïncidence courante (deux pointeurs de
        // fonction voisins dans une structure) ; 3 est déjà un objet structuré.
        let suites = [vec![1u64], vec![1, 2], vec![1, 2, 3], vec![1, 2, 3, 4]];
        let retenues: Vec<usize> = suites
            .iter()
            .filter(|s| s.len() >= MIN_SLOTS)
            .map(Vec::len)
            .collect();
        assert_eq!(retenues, vec![3, 4]);
    }

    /// Une table est considérée couverte par le RTTI si elle-même, son slot
    /// précédent (le COL) ou son slot suivant est une vtable connue.
    #[test]
    fn couverture_rtti_tolere_un_slot_de_decalage() {
        let rtti: HashSet<u64> = [0x1000u64].into_iter().collect();
        for va in [0x1000u64, 0x1008, 0xff8] {
            let covered = rtti.contains(&va)
                || rtti.contains(&va.wrapping_sub(8))
                || rtti.contains(&(va + 8));
            assert!(covered, "{va:#x} devrait être vue comme couverte");
        }
        let va = 0x2000u64;
        assert!(!(rtti.contains(&va) || rtti.contains(&(va - 8)) || rtti.contains(&(va + 8))));
    }
}
