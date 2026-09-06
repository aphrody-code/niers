//! Rend une image du match en 3D, avec les vrais modèles du jeu.
fn main() -> Result<(), String> {
    let vfs = nie_formats::vfs::open_game().map_err(|e| format!("{e:?}"))?;
    let t0 = std::time::Instant::now();
    let modele = nie_app::match3d::charger_modele_joueur(&vfs, 40).map_err(|e| format!("{e:#}"))?;
    let tris: usize = modele.primitives.iter().map(|p| p.indices.len() / 3).sum();
    println!(
        "modele charge en {:?} : {} primitives, {tris} triangles",
        t0.elapsed(),
        modele.primitives.len()
    );

    let mut w = nie_runtime::World::kickoff();
    for _ in 0..600 {
        w.step(1.0 / 60.0);
    }
    let t1 = std::time::Instant::now();
    let px = nie_app::match3d::rendre(&w, &modele);
    println!("image rendue en {:?}", t1.elapsed());

    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(
            std::io::Cursor::new(&mut out),
            nie_app::W as u32,
            nie_app::H as u32,
        );
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(&px)
            .map_err(|e| e.to_string())?;
    }
    std::fs::write("var/match3d.png", &out).map_err(|e| e.to_string())?;
    println!("var/match3d.png ({} octets)", out.len());
    Ok(())
}
