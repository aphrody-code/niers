//! Liste les clips squelettiques d'un paquet pour choisir une pose sur les données réelles.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: list_motion_clips <fichier.g4pk>")?;
    let bytes = std::fs::read(path)?;
    let pack = nie_formats::g4pk::parse(&bytes)?;
    for file in pack.files.iter().filter(|f| f.name.ends_with(".g4mt")) {
        let data = bytes
            .get(file.offset..file.offset + file.size)
            .ok_or("entrée hors limites")?;
        let motion = nie_formats::g4mt::Motion::parse(data).ok_or("motion illisible")?;
        for clip in &motion.clips {
            println!(
                "{}\t{}\t{} frames\tadditif={}",
                file.name,
                clip.name,
                clip.frame_count(),
                clip.is_additive()
            );
        }
    }
    Ok(())
}
