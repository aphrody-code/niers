//! Couverture d'un **dump** face à l'index du jeu : ce que le dump sert, ce qu'il manque, ce
//! qu'il porte en plus.
//!
//! Le rendu d'un écran ou d'un playthrough ne touche qu'une fraction des assets — il prouve que
//! le branchement marche, pas que le dump est complet. Cet outil compare les deux ensembles de
//! chemins logiques en entier, et ventile les manques par famille pour dire *ce qui* manque.
//!
//! ```text
//! NIE_DUMP_DIR=<dump> cargo run -p nie-formats --example dump_couverture
//! ```

use std::collections::{BTreeMap, HashSet};

use nie_formats::vfs::{self, Vfs};

fn main() -> Result<(), String> {
    let install = vfs::resolve_game_dir().join("data");
    if !install.join("cpk_list.cfg.bin").is_file() {
        return Err(format!(
            "aucune installation sous {} — rien à quoi comparer le dump",
            install.display()
        ));
    }
    let dump_dir = vfs::resolve_dump_dir()
        .ok_or_else(|| "aucun dump trouvé (poser NIE_DUMP_DIR)".to_string())?;
    if dump_dir == install {
        return Err("le dump et l'installation sont le même répertoire".into());
    }

    let mut packs = Vfs::new();
    packs
        .init(&install)
        .map_err(|e| format!("montage packs : {e:?}"))?;
    let mut dump = Vfs::new();
    dump.init_loose(&dump_dir)
        .map_err(|e| format!("montage dump : {e:?}"))?;
    println!("packs  {}", install.display());
    println!("dump   {}", dump_dir.display());

    // L'index du jeu est la référence : c'est ce que `nie.exe` sait charger. Les CPK hors
    // `cpk_list` (films, sound_asset) comptent aussi — un dump qui les perdrait serait muet.
    let attendus: HashSet<String> = packs
        .iter()
        .map(|(p, _)| p.to_string())
        .chain(packs.iter_extra().map(|(p, _)| p.to_string()))
        .collect();
    let servis: HashSet<String> = dump.iter().map(|(p, _)| p.to_string()).collect();

    let mut manquants: BTreeMap<String, usize> = BTreeMap::new();
    let mut exemples: Vec<&str> = Vec::new();
    for chemin in &attendus {
        if servis.contains(chemin) {
            continue;
        }
        // Famille = les trois premiers segments (`data/common/gamedata`) : assez fin pour
        // désigner un sous-système, assez large pour ne pas rendre une liste illisible.
        let famille: Vec<&str> = chemin.split('/').take(3).collect();
        *manquants.entry(famille.join("/")).or_default() += 1;
        if exemples.len() < 10 {
            exemples.push(chemin);
        }
    }
    // Ce que le dump porte SANS que l'index le déclare : fichiers loose de l'installation,
    // résidus d'extraction. Les nommer évite de les prendre pour du contenu de jeu.
    let mut hors_index: Vec<&String> = servis.difference(&attendus).collect();
    hors_index.sort();
    let en_trop = hors_index.len();
    let couverts = attendus.len() - manquants.values().sum::<usize>();

    println!(
        "\nattendus {} | servis par le dump {} ({:.3} %) | manquants {} | en plus {}",
        attendus.len(),
        couverts,
        couverts as f64 * 100.0 / attendus.len() as f64,
        attendus.len() - couverts,
        en_trop,
    );
    if en_trop > 0 {
        println!("\nhors index ({en_trop}) :");
        for chemin in hors_index.iter().take(20) {
            println!("  {chemin}");
        }
    }
    if manquants.is_empty() {
        println!("\nle dump sert l'intégralité de l'index du jeu");
        return Ok(());
    }
    println!("\nmanques par famille :");
    let mut par_taille: Vec<(&String, &usize)> = manquants.iter().collect();
    par_taille.sort_by(|a, b| b.1.cmp(a.1));
    for (famille, n) in par_taille.iter().take(20) {
        println!("  {n:>8}  {famille}");
    }
    println!("\nexemples :");
    for chemin in &exemples {
        println!("  {chemin}");
    }
    Ok(())
}
