//! Couverture du reverse des commandes `funcLua*` : ce que le dispatch Rust sait nommer,
//! rapporté à ce que le binaire déclare.
//!
//! La vérité terrain est `data/re/funclua-cmdid-handlers.json` — 3 471 paires `cmdId → adresse du
//! handler`, extraites des tables de dispatch de `nie.exe`. Le dispatch de [`nie_lua::menu_host`]
//! n'en nomme qu'une partie ; cet exemple chiffre l'écart et désigne les cibles suivantes :
//! **les cmdId les plus appelés par les scripts du jeu, parmi ceux qui restent anonymes**.
//!
//! ```text
//! cargo run -p nie-lua --example couverture_funclua --release
//! ```
//!
//! Le fichier vit sous `data/` (gitignoré, © LEVEL-5) : l'exemple annonce son saut s'il manque.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Cherche `data/re/<nom>` depuis le répertoire courant puis ses ancêtres.
fn fichier_re(nom: &str) -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let mut courant: &Path = &cwd;
    loop {
        let candidat = courant.join("data/re").join(nom);
        if candidat.is_file() {
            return Some(candidat);
        }
        courant = courant.parent()?;
    }
}

fn main() {
    let Some(chemin) = fichier_re("funclua-cmdid-handlers.json") else {
        eprintln!(
            "skip : data/re/funclua-cmdid-handlers.json introuvable — \
             le rapatrier depuis le dump de reverse."
        );
        return;
    };

    let texte = std::fs::read_to_string(&chemin).expect("lecture du dump de handlers");
    // Le dump est un objet plat `"0xCMDID": "0xVA"`, une paire par ligne : on l'extrait à la main
    // plutôt que d'ajouter `serde_json` à `nie-lua` pour un seul exemple.
    let handlers: BTreeMap<String, String> = texte
        .lines()
        .filter_map(|l| {
            let mut parts = l.split('"').filter(|s| s.starts_with("0x"));
            Some((parts.next()?.to_string(), parts.next()?.to_string()))
        })
        .collect();
    assert!(!handlers.is_empty(), "dump de handlers vide ou illisible");

    let (mut nommes, mut anonymes) = (0usize, 0usize);
    let mut restants: Vec<u32> = Vec::new();
    for cle in handlers.keys() {
        let Some(hex) = cle.strip_prefix("0x") else {
            continue;
        };
        let Ok(id) = u32::from_str_radix(hex, 16) else {
            continue;
        };
        if nie_lua::menu_host::command_name(id).is_some() {
            nommes += 1;
        } else {
            anonymes += 1;
            restants.push(id);
        }
    }

    let total = nommes + anonymes;
    #[allow(clippy::cast_precision_loss)]
    let taux = if total == 0 {
        0.0
    } else {
        nommes as f64 * 100.0 / total as f64
    };

    println!("dump        {}", chemin.display());
    println!("cmdId       {total} declares par le binaire");
    println!("nommes      {nommes}");
    println!("anonymes    {anonymes}");
    println!("couverture  {taux:.2} %");

    if !restants.is_empty() {
        println!("\nprochaines cibles (10 premiers cmdId anonymes) :");
        for id in restants.iter().take(10) {
            let va = handlers
                .get(&format!("0x{id:08X}"))
                .map_or("?", String::as_str);
            println!("  0x{id:08X}  handler {va}");
        }
    }
}
