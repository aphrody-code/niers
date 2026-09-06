//! Câblage de la propagation de labels vers la base sqlite.
//!
//! Ce module fait le pont entre le graphe pur [`PropagationGraph`] et la
//! table `function` de la base de connaissance : il charge les fonctions +
//! arêtes d'appel, construit le graphe, exécute la propagation, puis réécrit
//! les champs `subsystem`, `confidence`, `subsys_src` pour les nœuds
//! nouvellement étiquetés et insère une `hypothesis` par changement.
//!
//! Ordre des étapes :
//! 1. Ancrage par chaînes   ([`crate::anchors::anchor_by_strings`]).
//! 2. Ancrage RTTI          ([`crate::anchors::anchor_by_rtti`]).
//! 3. Ancrage const-magic   ([`crate::anchors::anchor_by_const`]).
//! 4. Chargement du graphe et construction des arêtes.
//! 5. Propagation (`N` rounds).
//! 6. Écriture en base + hypothèses.

use anyhow::{Context, Result};
use hashbrown::HashMap;
use nie_index::{Db, ingest, rusqlite};
use tracing::{debug, info};

use crate::propagate::{Node, PropagationGraph};

/// Ligne de fonction chargée pour la propagation : `(id, vaddr, subsystem, name, confidence)`.
type FuncRow = (i64, i64, Option<String>, Option<String>, f64);

/// Statistiques de la propagation complète (ancrage + diffusion).
#[derive(Debug, Clone, Copy)]
pub struct Stats {
    /// Fonctions nouvellement étiquetées par la diffusion de labels.
    pub labeled: usize,
    /// Nombre de rounds exécutés.
    pub rounds: usize,
    /// Couverture avant tout ancrage (%).
    pub coverage_before: f64,
    /// Couverture après propagation (%).
    pub coverage_after: f64,
    /// Fonctions classifiées avant tout ancrage.
    pub classified_before: i64,
    /// Fonctions classifiées après propagation.
    pub classified_after: i64,
    /// Total de fonctions.
    pub total: i64,
    /// Ancres posées par chaînes.
    pub anchored_str: usize,
    /// Ancres posées par RTTI.
    pub anchored_rtti: usize,
    /// Ancres posées par constante magique.
    pub anchored_const: usize,
}

/// Charge les fonctions + xrefs du binaire, construit le graphe, propage les
/// labels, et réécrit `function.subsystem` + `confidence` + `subsys_src='ml'`
/// sous une transaction unique. Insère une `hypothesis` par changement.
pub fn propagate_db(db: &mut Db, binary_id: i64, rounds: usize) -> Result<Stats> {
    // --- Étape 0 : couverture initiale ------------------------------------------
    let cov_before = nie_index::query::coverage(db.conn(), binary_id)?;
    info!(
        "propagate_db: couverture initiale {}/{} ({:.2}%)",
        cov_before.classified, cov_before.total, cov_before.pct
    );

    // --- Étape 1 : ancrage par chaînes ------------------------------------------
    let anchored_str = {
        let n =
            crate::anchors::anchor_by_strings(db.conn(), binary_id).context("ancrage strings")?;
        info!("ancrage strings: {} fonctions ancrées", n);
        n
    };

    // --- Étape 2 : ancrage par RTTI ---------------------------------------------
    let anchored_rtti = {
        let n = crate::anchors::anchor_by_rtti(db.conn(), binary_id).context("ancrage RTTI")?;
        info!("ancrage RTTI: {} fonctions ancrées", n);
        n
    };

    // --- Étape 3 : ancrage par constante magique --------------------------------
    let anchored_const = {
        let n =
            crate::anchors::anchor_by_const(db.conn(), binary_id).context("ancrage const-magic")?;
        info!("ancrage const-magic: {} fonctions ancrées", n);
        n
    };

    // --- Étape 4 : chargement des fonctions ------------------------------------
    // (id, vaddr, subsystem, name, confidence)
    let funcs: Vec<FuncRow> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, vaddr, subsystem, name, confidence FROM function WHERE binary_id=?1",
        )?;
        stmt.query_map([binary_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, f64>(4)?,
            ))
        })?
        .collect::<std::result::Result<_, _>>()?
    };

    let total = funcs.len();
    info!("propagate_db: {} fonctions chargées", total);

    // Bimap label_str → u32 (label numérique pour le graphe).
    let mut label_to_u32: HashMap<String, u32> = HashMap::new();
    let mut u32_to_label: Vec<String> = Vec::new();

    let mut get_or_insert_label = |s: &str| -> u32 {
        if let Some(&v) = label_to_u32.get(s) {
            return v;
        }
        let v = u32_to_label.len() as u32;
        label_to_u32.insert(s.to_owned(), v);
        u32_to_label.push(s.to_owned());
        v
    };

    // Bimap vaddr → index nœud dans le graphe.
    let mut vaddr_to_node: HashMap<i64, u32> = HashMap::with_capacity(total);
    // Bimap index nœud → function_id (pour les UPDATE).
    let mut node_to_fid: Vec<i64> = Vec::with_capacity(total);

    let mut graph = PropagationGraph::with_capacity(total);

    for (fid, vaddr, subsystem, name, confidence) in &funcs {
        let is_named = name.as_deref().is_some_and(|n| !n.starts_with("FUN_"));
        let node = match subsystem {
            Some(sub) if sub != "standalone" => {
                // Nœud ancre : label connu, verrouillé.
                let lbl = get_or_insert_label(sub);
                Node {
                    label: Some(lbl),
                    confidence: *confidence as f32,
                    locked: true,
                }
            }
            _ if is_named => {
                // Fonction nommée sans sous-système : laissée libre pour
                // recevoir un label par propagation depuis ses voisins.
                Node::unknown()
            }
            _ => Node::unknown(),
        };
        let idx = graph.add_node(node);
        vaddr_to_node.insert(*vaddr, idx);
        node_to_fid.push(*fid);
    }

    debug!(
        "graphe: {} nœuds, {} labels distincts",
        graph.nodes.len(),
        u32_to_label.len()
    );

    // --- Étape 5 : arêtes (xref kind='call') ------------------------------------
    // Arêtes typées : appel direct (réel) = 1.0, cohésion de vtable = 0.5,
    // référence LEA `lea reg,[rip+fn]` = 0.4 (indice indirect plus faible —
    // évite de sur-diffuser un label via une table de pointeurs / un callback).
    let xrefs: Vec<(i64, i64, f32)> = {
        let mut stmt = db.conn().prepare(
            "SELECT from_addr, to_addr, kind FROM xref WHERE binary_id=?1 AND kind IN ('call','vtable','lea')",
        )?;
        stmt.query_map([binary_id], |r| {
            let kind: String = r.get(2)?;
            let w = match kind.as_str() {
                "vtable" => 0.5_f32,
                "lea" => 0.4_f32,
                _ => 1.0_f32,
            };
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, w))
        })?
        .collect::<std::result::Result<_, _>>()?
    };

    let mut edges_added = 0usize;
    for (from, to, w) in &xrefs {
        if let (Some(&a), Some(&b)) = (vaddr_to_node.get(from), vaddr_to_node.get(to)) {
            graph.add_edge(a, b, *w);
            edges_added += 1;
        }
    }
    info!("propagate_db: {} arêtes call ajoutées", edges_added);

    // --- Étape 6 : propagation --------------------------------------------------
    let labeled_count_before = graph.labeled_count();
    let newly_labeled = graph.run(rounds);
    let labeled_count_after = graph.labeled_count();
    info!(
        "propagate_db: {} rounds, {} nouveaux labels ({} → {})",
        rounds, newly_labeled, labeled_count_before, labeled_count_after
    );

    // --- Étape 7 : écriture DB sous transaction ---------------------------------
    let tx = db.conn_mut().transaction()?;
    let mut updated = 0usize;

    // On réécrit uniquement les nœuds non verrouillés qui ont reçu un label.
    for (node_idx, node) in graph.nodes.iter().enumerate() {
        if node.locked || node.label.is_none() {
            continue;
        }
        let Some(lbl_u32) = node.label else { continue };
        let label_str = u32_to_label
            .get(lbl_u32 as usize)
            .map(|s| s.as_str())
            .unwrap_or("?");

        if label_str == "standalone" || label_str == "?" {
            continue;
        }

        let fid = node_to_fid[node_idx];
        let confidence = node.confidence as f64;

        tx.execute(
            "UPDATE function SET subsystem=?1, subsys_src='ml', confidence=?2 WHERE id=?3",
            rusqlite::params![label_str, confidence, fid],
        )?;

        // Hypothèse ML : claim "subsystem=X" pour cette fonction.
        ingest::hypothesis(
            &tx,
            binary_id,
            &format!("func:{fid}"),
            &format!("subsystem={label_str}"),
            confidence,
            "propagate_db",
        )?;

        updated += 1;
    }

    tx.commit().context("commit propagation")?;
    info!("propagate_db: {} fonctions mises à jour en DB", updated);

    // --- Étape 8 : couverture finale -------------------------------------------
    let cov_after = nie_index::query::coverage(db.conn(), binary_id)?;
    info!(
        "propagate_db: couverture finale {}/{} ({:.2}%)",
        cov_after.classified, cov_after.total, cov_after.pct
    );

    // Snapshot de couverture.
    db.snapshot_coverage(binary_id)?;

    Ok(Stats {
        labeled: updated,
        rounds,
        coverage_before: cov_before.pct,
        coverage_after: cov_after.pct,
        classified_before: cov_before.classified,
        classified_after: cov_after.classified,
        total: cov_before.total,
        anchored_str,
        anchored_rtti,
        anchored_const,
    })
}

/// Compte les fonctions classifiées dans une connexion de test.
#[cfg(test)]
fn count_classified(conn: &rusqlite::Connection, binary_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM function WHERE binary_id=?1 AND subsystem IS NOT NULL AND subsystem <> 'standalone'",
        [binary_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_index::{Db, ingest};

    /// Construit une mini-base avec 3 fonctions : 2 ancres + 1 inconnue reliée
    /// par call, et vérifie que la propagation classifie l'inconnue.
    #[test]
    fn propagate_db_classifie_voisin_inconnu() -> anyhow::Result<()> {
        let mut db = Db::open_in_memory()?;
        let bin = db.upsert_binary(
            "test.exe",
            "abc123",
            "x86_64",
            64,
            0x1_4000_0000,
            0,
            None,
            None,
        )?;

        {
            let tx = db.conn_mut().transaction()?;
            // Fonction 1 : ancre "audio" à vaddr 0x1000.
            ingest::function(
                &tx,
                bin,
                0x1000,
                Some("CriAtom_Init"),
                Some("re"),
                "audio",
                "init",
                0.9,
            )?;
            // Fonction 2 : inconnue à vaddr 0x2000.
            ingest::function(
                &tx,
                bin,
                0x2000,
                None,
                Some("re"),
                "standalone",
                "leaf",
                0.0,
            )?;
            // Fonction 3 : ancre "gameplay" à vaddr 0x3000.
            ingest::function(
                &tx,
                bin,
                0x3000,
                Some("SoccerManager_Init"),
                Some("re"),
                "gameplay",
                "init",
                0.9,
            )?;
            // Appel : 0x1000 → 0x2000 (audio vers inconnu).
            ingest::xref(&tx, bin, 0x1000, 0x2000, "call")?;
            // Str-ref sur la fonction inconnue pour tester l'ancrage.
            let fid2 = tx.query_row("SELECT id FROM function WHERE vaddr=0x2000", [], |r| {
                r.get(0)
            })?;
            ingest::str_ref(&tx, bin, fid2, "criAtom_OpenWithId")?;
            tx.commit()?;
        }

        let stats = propagate_db(&mut db, bin, 8)?;

        // Après propagation, la fonction inconnue doit être classifiée.
        assert!(
            stats.classified_after > stats.classified_before,
            "la propagation doit augmenter la couverture : avant={}, après={}",
            stats.classified_before,
            stats.classified_after
        );

        // La couverture finale doit être supérieure à l'initiale.
        assert!(
            stats.coverage_after > stats.coverage_before,
            "pct avant={:.2} après={:.2}",
            stats.coverage_before,
            stats.coverage_after
        );

        Ok(())
    }

    /// Vérifie que l'ancrage RTTI enrichit les statistiques.
    #[test]
    fn propagate_db_ancres_rtti_comptees() -> anyhow::Result<()> {
        let mut db = Db::open_in_memory()?;
        let bin = db.upsert_binary(
            "test.exe",
            "abc456",
            "x86_64",
            64,
            0x1_4000_0000,
            0,
            None,
            None,
        )?;

        {
            let tx = db.conn_mut().transaction()?;
            // Fonction standalone qui référence un nom de classe RTTI game::
            ingest::function(
                &tx,
                bin,
                0x5000,
                None,
                Some("re"),
                "standalone",
                "leaf",
                0.0,
            )?;
            let fid = tx.query_row("SELECT id FROM function WHERE vaddr=0x5000", [], |r| {
                r.get(0)
            })?;
            ingest::str_ref(&tx, bin, fid, "BallMoveBezier")?;
            // Classe RTTI correspondante dans game:: namespace
            ingest::rtti_class(
                &tx,
                bin,
                "BallMoveBezier",
                Some("game"),
                Some(".?AVBallMoveBezier@game@@"),
            )?;
            tx.commit()?;
        }

        let stats = propagate_db(&mut db, bin, 4)?;

        // La fonction doit être ancrée par RTTI ou str
        assert!(
            stats.anchored_str > 0 || stats.anchored_rtti > 0,
            "au moins une ancre doit être posée (str={}, rtti={})",
            stats.anchored_str,
            stats.anchored_rtti
        );
        assert!(
            stats.classified_after > stats.classified_before,
            "la couverture doit augmenter"
        );

        Ok(())
    }

    #[test]
    fn count_classified_helper() {
        let db = Db::open_in_memory().unwrap();
        let bin = db
            .upsert_binary("t.exe", "sha1", "x86_64", 64, 0x140000000, 0, None, None)
            .unwrap();
        assert_eq!(count_classified(db.conn(), bin), 0);
    }
}
