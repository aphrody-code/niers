//! Validation croisée du décodage structurel G4MT (`g4mt::Motion`) contre un vrai fichier —
//! optionnellement contre l'implémentation Python indépendante `plugins/niers-blender` (valeurs
//! attendues passées en argument, cf. `docs game-data` pour la méthode).
//! Usage : `cargo run -p nie-formats --example validate_g4mt -- <fichier.g4mt> <squelette.g4sk>`

use nie_formats::{g4mt, g4sk};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let g4mt_path = args
        .get(1)
        .expect("usage: validate_g4mt <fichier.g4mt> <squelette.g4sk>");
    let sk_path = args
        .get(2)
        .expect("usage: validate_g4mt <fichier.g4mt> <squelette.g4sk>");
    let data = std::fs::read(g4mt_path).expect("lire g4mt");
    let sk_bytes = std::fs::read(sk_path).expect("lire g4sk");

    let motion = g4mt::Motion::parse(&data).expect("Motion::parse");
    println!(
        "clips={} cibles={}",
        motion.clips.len(),
        motion.target_hashes.len()
    );

    let header = g4sk::parse_header(&sk_bytes).expect("g4sk header");
    let bones = g4sk::parse_hierarchy(&sk_bytes, &header);
    let bone_names: Vec<&str> = bones.bones.iter().map(|b| b.name.as_str()).collect();
    let resolved = g4mt::resolve_targets(&motion.target_hashes, &bone_names);
    let n_resolved = resolved.iter().filter(|r| r.is_some()).count();
    println!(
        "cibles résolues contre le G4SK : {n_resolved}/{}",
        motion.target_hashes.len()
    );

    let mut total_samples = 0usize;
    let mut worst_non_unit = 0.0f32;
    for (ci, clip) in motion.clips.iter().enumerate() {
        let targets = motion.target_indices(clip);
        let mut ok = 0usize;
        for &t in &targets {
            for frame in [
                0.0,
                (clip.frame_count() as f32 - 1.0) * 0.5,
                (clip.frame_count() as f32 - 1.0),
            ] {
                if let Some(q) = motion.sample_rotation(&data, clip, t, frame) {
                    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                    worst_non_unit = worst_non_unit.max((n - 1.0).abs());
                    ok += 1;
                }
            }
        }
        total_samples += ok;
        if ci < 3 || clip.frame_count() > 300 {
            println!(
                "clip[{ci:3}] \"{}\" frames={:4} cibles={:3} échantillons_rotation_ok={ok}",
                clip.name,
                clip.frame_count(),
                targets.len()
            );
        }
    }
    println!(
        "total échantillons rotation décodés = {total_samples}, pire écart |q|-1 = {worst_non_unit:.6}"
    );

    let ok =
        n_resolved > motion.target_hashes.len() / 2 && total_samples > 0 && worst_non_unit < 0.01;
    println!("{}", if ok { "VALIDÉ ✓" } else { "ÉCHEC ✗" });
    if !ok {
        std::process::exit(1);
    }
}
