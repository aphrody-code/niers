//! Le décodeur G4NV confronté aux **160 `.g4nv` réels** du jeu.
//!
//! Ce test ne vérifie pas qu'un parse « aboutit » — un parseur permissif aboutit toujours. Il
//! rejoue les invariants géométriques qui ont servi à *dériver* la structure : le centroïde
//! annoncé par chaque polygone doit être la moyenne exacte de ses trois sommets, et son rayon²
//! le carré de la distance au sommet le plus lointain. Un découpage faux ne peut pas satisfaire
//! ces deux égalités sur 47 000 polygones.
//!
//! Il **annonce son saut** quand ni l'installation ni le dump ne sont disponibles.

#![cfg(feature = "std")]

use nie_formats::navm;
use nie_formats::vfs::{self, Vfs};

fn corpus() -> Option<Vfs> {
    match vfs::open_game() {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("skip : ni installation ni dump ({e:?})");
            None
        }
    }
}

fn chemins(vfs: &Vfs) -> Vec<String> {
    let mut v: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.ends_with(".g4nv"))
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn navmesh_reels_geometriquement_coherents() {
    let Some(vfs) = corpus() else { return };
    let chemins = chemins(&vfs);
    if chemins.is_empty() {
        eprintln!("skip : aucun .g4nv dans le corpus monté");
        return;
    }

    let (mut lus, mut polys, mut aretes, mut sommets) = (0usize, 0usize, 0usize, 0usize);
    let (mut externes, mut fichiers_externes) = (0usize, 0usize);
    let mut echecs: Vec<String> = Vec::new();
    for p in &chemins {
        let Ok(octets) = vfs.read(p) else {
            echecs.push(format!("{p} : lecture impossible"));
            continue;
        };
        let n = match navm::parse(&octets) {
            Ok(n) => n,
            Err(e) => {
                echecs.push(format!("{p} : {e}"));
                continue;
            }
        };
        if let Err(e) = navm::check(&n) {
            echecs.push(format!("{p} : {e}"));
            continue;
        }
        if !n.is_size_consistent() {
            echecs.push(format!("{p} : header_size + data_size != file_size"));
            continue;
        }
        // counts[0] == 3 * counts[3]
        if n.section_counts[0] != 3 * n.section_counts[3] {
            echecs.push(format!("{p} : coins != 3 × polygones"));
            continue;
        }
        if n.vertices.iter().any(|v| (v.w - 1.0).abs() > 1e-6) {
            echecs.push(format!("{p} : un sommet a w != 1"));
            continue;
        }
        let mut somme_refs = 0usize;
        for (i, poly) in n.polygons.iter().enumerate() {
            somme_refs += poly.edge_ref_count as usize;
            if poly.corner_count != 3 {
                echecs.push(format!("{p} : polygone {i} a {} coins", poly.corner_count));
                break;
            }
            let Some(t) = n.triangle(i) else {
                echecs.push(format!("{p} : polygone {i} sans triangle"));
                break;
            };
            let moy = |k: usize| (t[0][k] + t[1][k] + t[2][k]) / 3.0;
            let ecart = (0..3)
                .map(|k| (moy(k) - poly.center[k]).abs())
                .fold(0.0f32, f32::max);
            if ecart > 0.05 {
                echecs.push(format!(
                    "{p} : centroïde du polygone {i} faux (écart {ecart})"
                ));
                break;
            }
            let r2 = t
                .iter()
                .map(|v| (0..3).map(|k| (v[k] - poly.center[k]).powi(2)).sum::<f32>())
                .fold(0.0f32, f32::max);
            if (r2 - poly.radius_sq).abs() > (r2 * 1e-3).max(0.05) {
                echecs.push(format!(
                    "{p} : rayon² du polygone {i} faux ({r2} vs {})",
                    poly.radius_sq
                ));
                break;
            }
        }
        if somme_refs != n.edge_refs.len() {
            echecs.push(format!(
                "{p} : Σ edge_ref_count = {somme_refs} != {}",
                n.edge_refs.len()
            ));
            continue;
        }
        // le coût d'une arête est la distance entre les centroïdes de ses deux polygones
        for (i, e) in n.edges.iter().enumerate() {
            let (a, b) = (n.polygons[e.poly_a as usize], n.polygons[e.poly_b as usize]);
            let d = (0..3)
                .map(|k| (a.center[k] - b.center[k]).powi(2))
                .sum::<f32>()
                .sqrt();
            if (d - e.cost).abs() > (d * 1e-3).max(0.05) {
                echecs.push(format!("{p} : coût de l'arête {i} = {} != {d}", e.cost));
                break;
            }
        }
        let ext = n.external_ref_count();
        if ext > 0 {
            externes += ext;
            fichiers_externes += 1;
        }
        lus += 1;
        polys += n.polygons.len();
        aretes += n.edges.len();
        sommets += n.vertices.len();
    }

    eprintln!(
        "g4nv : {lus}/{} fichiers décodés — {sommets} sommets, {polys} polygones, {aretes} arêtes \
         ({externes} références externes sur {fichiers_externes} fichiers)",
        chemins.len()
    );
    for e in echecs.iter().take(10) {
        eprintln!("  ÉCHEC {e}");
    }
    assert!(echecs.is_empty(), "{} fichier(s) en échec", echecs.len());
    assert_eq!(lus, chemins.len(), "tous les .g4nv doivent se décoder");
}
