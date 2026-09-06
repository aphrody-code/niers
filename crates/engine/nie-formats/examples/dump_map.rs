//! Diagnostic RE du maillage des **maps** : dumpe la table des submeshes du g4md (extrait du
//! g4pkm) et statistique le décodage de la géométrie (longueurs d'arêtes) pour localiser le
//! scramble. Usage : `cargo run -p nie-formats --example dump_map -- <g4pkm> <g4mg>`.

use nie_formats::{g4md, g4mg, g4pk};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pkm = std::fs::read(&args[1]).expect("lire g4pkm");
    let g4mg_bytes = std::fs::read(&args[2]).expect("lire g4mg");

    // Extrait le premier g4md du g4pkm.
    let pk = g4pk::parse(&pkm).expect("parse g4pkm");
    println!("g4pkm: {} sous-fichiers", pk.files.len());
    for f in &pk.files {
        println!("  - {} (offset {}, {} o)", f.name, f.offset, f.size);
    }
    let mdf = pk
        .files
        .iter()
        .find(|f| {
            f.name.ends_with(".g4md") || g4md::is_g4md(&pkm[f.offset..f.offset + f.size.min(4)])
        })
        .expect("g4md introuvable dans le g4pkm");
    let g4md_bytes = &pkm[mdf.offset..mdf.offset + mdf.size];
    let md = g4md::parse(g4md_bytes).expect("parse g4md");

    println!("\n=== G4MD ===");
    println!(
        "submesh_count={} material_count={} vlayout_count={} face_data_base=0x{:X}",
        md.header.submesh_count,
        md.header.material_count,
        md.header.vlayout_count,
        md.header.face_data_base
    );
    println!("attributs vertex: {:?}", md.attributes);
    let mat_names = g4md::extract_map_material_names(g4md_bytes, md.header.material_count as usize);
    println!("noms de texture PAR MATÉRIAU ({}) :", mat_names.len());
    for (i, n) in mat_names.iter().enumerate() {
        println!("  mat[{i}] = {n:?}");
    }
    println!(
        "noms de textures (material_base_names): {:?}",
        md.material_base_names
    );
    println!("g4mg: {} octets", g4mg_bytes.len());

    println!("\n=== SUBMESHES ===");
    for (i, sm) in md.submeshes.iter().enumerate() {
        println!(
            "[{i}] v_off={} v_cnt={} stride={} idx_off={} idx_cnt={} layout={} mat={}",
            sm.vertex_offset,
            sm.vertex_count,
            sm.stride,
            sm.index_offset,
            sm.index_count,
            sm.layout_num,
            sm.material_index
        );
    }

    println!("\n=== GÉOMÉTRIE DÉCODÉE (extract_geometry) ===");
    let geos = g4mg::extract_geometry(&g4mg_bytes, &md);
    for (i, g) in geos.iter().enumerate() {
        let np = g.positions.len();
        let ni = g.indices.len();
        // stats d'arêtes des triangles : une géométrie saine a des arêtes courtes/cohérentes.
        let mut edges = Vec::new();
        for t in g.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a.max(b).max(c) >= np {
                continue;
            }
            let p = |k: usize| g.positions[k];
            let d = |u: g4mg::Vec3, v: g4mg::Vec3| {
                ((u.x - v.x).powi(2) + (u.y - v.y).powi(2) + (u.z - v.z).powi(2)).sqrt()
            };
            edges.push(d(p(a), p(b)));
            edges.push(d(p(b), p(c)));
        }
        edges.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = if edges.is_empty() {
            0.0
        } else {
            edges[edges.len() / 2]
        };
        let mx = edges.last().copied().unwrap_or(0.0);
        let bad = edges.iter().filter(|&&e| e > 1e4).count();
        // bbox des positions
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &g.positions {
            for (k, v) in [p.x, p.y, p.z].iter().enumerate() {
                lo[k] = lo[k].min(*v);
                hi[k] = hi[k].max(*v);
            }
        }
        println!(
            "[{i}] np={np} ni={ni} arête_médiane={med:.2} arête_max={mx:.2e} arêtes>1e4={bad}  bbox=({:.1},{:.1},{:.1})..({:.1},{:.1},{:.1})",
            lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
        );
        if i == 0 {
            println!(
                "    pos[0..3]={:?}",
                g.positions
                    .iter()
                    .take(3)
                    .map(|p| (p.x, p.y, p.z))
                    .collect::<Vec<_>>()
            );
            println!(
                "    idx[0..9]={:?}",
                g.indices.iter().take(9).collect::<Vec<_>>()
            );
        }
    }
}
