//! Sonde les deux vues multi-entrées ajoutées à l'explorateur, contre le VRAI VFS.
//!
//! ```text
//! cargo run -p nie-explore --example probe_banque -- <chemin.acb|awb|g4tx>
//! ```
//!
//! Sans argument, prend la première banque et le premier conteneur multi-textures qu'il trouve.
//! Ce n'est pas un test (il exige les 57 Gio d'assets, absents du clone public) : c'est la preuve
//! qu'on peut rejouer à la main que le catalogue voit bien N entrées là où l'ancien chemin n'en
//! rendait qu'une.

use nie_explore::audio;
use nie_formats::vfs::{Vfs, resolve_game_dir};

fn main() {
    let racine = resolve_game_dir();
    let mut vfs = Vfs::new();
    if let Err(e) = vfs.init(racine.join("data")) {
        eprintln!("VFS indisponible ({}) : {e}", racine.display());
        return;
    }
    println!("VFS : {} entrées", vfs.asset_count());

    let cible = std::env::args().nth(1);
    match cible {
        Some(p) if p.ends_with(".g4tx") => textures(&vfs, &p),
        Some(p) => banque(&vfs, &p),
        None => {
            // Premiers candidats rencontrés, pour une exécution sans argument.
            let mut acb = None;
            let mut g4tx = None;
            for (path, _) in vfs.iter() {
                if acb.is_none() && path.ends_with(".acb") {
                    acb = Some(path.to_string());
                }
                if g4tx.is_none() && path.contains("icon_item") && path.ends_with(".g4tx") {
                    g4tx = Some(path.to_string());
                }
                if acb.is_some() && g4tx.is_some() {
                    break;
                }
            }
            if let Some(p) = acb {
                banque(&vfs, &p);
            }
            if let Some(p) = g4tx {
                textures(&vfs, &p);
            }
        }
    }
}

fn banque(vfs: &Vfs, path: &str) {
    let Ok(data) = vfs.read(path) else {
        eprintln!("{path} : absent du VFS");
        return;
    };
    // Voie de LISTAGE : ne lit jamais un AWB externe, quelle que soit sa taille.
    let localise = audio::localiser_awb(vfs, path, &data);
    let (source, taille) = match &localise {
        None => ("aucune".to_string(), 0),
        Some((s, _, t)) => (format!("{s:?}"), *t),
    };
    let cues = audio::cues(&data, localise.as_ref().and_then(|(_, b, _)| b.as_deref()));
    println!(
        "\n== {path} ==\nsource AWB : {source}\nbanque     : {taille} octets (NON lue si externe)\npistes     : {}",
        cues.len()
    );
    for c in cues.iter().take(5) {
        println!(
            "  {:<24} awb_id={:?} {} {}Hz {}ms → {}",
            if c.name.is_empty() {
                "(sans nom)"
            } else {
                &c.name
            },
            c.awb_id,
            c.codec,
            c.sample_rate.unwrap_or(0),
            c.length_ms,
            audio::nom_de_fichier(path, c)
        );
    }
    // Décodage réel de la première piste adressable : le catalogue ne prouve rien sans lui.
    // C'est ICI, et seulement ici, que la banque est réellement chargée (`resoudre_awb`).
    //
    // Sur un thread à pile de 16 Mio, comme les commandes de l'explorateur : `cridecoder` fait un
    // vrai `STATUS_STACK_OVERFLOW` sur la pile Windows par défaut en build debug — vérifié ici
    // même, le process entier meurt (fault SEH, non rattrapable).
    let charge = audio::resoudre_awb(vfs, path, &data);
    if let (Some((bytes, _)), Some(cue)) = (&charge, cues.iter().find(|c| c.awb_id.is_some())) {
        let id = cue.awb_id.unwrap_or(0);
        let octets = bytes.to_vec();
        let rendu = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(move || audio::decoder_cue(&octets, id))
            .expect("thread de décodage")
            .join()
            .expect("le décodage a paniqué");
        match rendu {
            Ok(wav) => println!("  décodage cue-id {id} : {} octets WAV", wav.len()),
            Err(e) => println!("  décodage cue-id {id} : ÉCHEC — {e}"),
        }
    }
}

fn textures(vfs: &Vfs, path: &str) {
    let Ok(data) = vfs.read(path) else {
        eprintln!("{path} : absent du VFS");
        return;
    };
    let Ok(g4tx) = nie_formats::g4tx::parse(&data) else {
        eprintln!("{path} : G4TX illisible");
        return;
    };
    println!("\n== {path} ==\ntextures : {}", g4tx.textures.len());
    for t in g4tx.textures.iter().take(5) {
        let png = nie_formats::g4tx_decode::decode_named_to_png(&data, &t.name);
        println!(
            "  {:<24} {}×{} dds={} régions={} → PNG {}",
            t.name,
            t.width,
            t.height,
            t.is_dds,
            t.sub_textures.len(),
            png.map_or("ÉCHEC".to_string(), |p| format!("{} octets", p.len()))
        );
    }
}
