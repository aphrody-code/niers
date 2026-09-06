//! Résolution du corpus de dumps `gamedata` partagée par les tests golden.
//!
//! Les goldens de `nie-data` s'adossent aux dumps JSON des `cfg.bin` du jeu
//! (`common/gamedata/**/*.cfg.bin.json`). Ce corpus n'est pas versionné — c'est de la donnée
//! Level-5 — et il ne vit pas au même endroit selon la machine : sur un serveur d'indexation
//! c'est un dossier d'export, sur un poste de jeu c'est un sous-dossier de l'installation.
//!
//! Racine cherchée, dans l'ordre :
//! 1. `NIE_GAMEDATA_JSON` — chemin explicite du dossier `gamedata` de dumps ;
//! 2. `<NIE_GAME_DIR>/dump/gamedata` puis `<NIE_GAME_DIR>/data/common/gamedata` ;
//! 3. le répertoire courant et ses ancêtres, mêmes deux sous-chemins.
//!
//! **Aucun chemin de poste n'est codé ici.** Quand rien n'est trouvé, [`load`] rend `None` en
//! l'annonçant sur `stderr` : un golden qui ne s'exécute pas doit se voir, sinon il passe au vert
//! sans rien prouver — exactement le faux signal que la discipline du dépôt interdit.

// Chaque test d'intégration compile ce module dans son propre binaire et n'en utilise qu'une
// partie : sans cette permission, les fonctions non appelées par tel ou tel golden remontent en
// « never used » dans les 77 binaires de test.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Sous-chemins candidats d'une racine de dumps, relatifs à une racine de jeu ou de travail.
const SOUS_CHEMINS: [&str; 2] = ["dump/gamedata", "data/common/gamedata"];

fn depuis(base: &Path) -> Option<PathBuf> {
    SOUS_CHEMINS
        .iter()
        .map(|s| base.join(s))
        .find(|p| p.is_dir())
}

/// Racine du corpus de dumps, ou `None` si absent de cette machine.
#[must_use]
pub fn racine() -> Option<&'static Path> {
    static RACINE: OnceLock<Option<PathBuf>> = OnceLock::new();
    RACINE
        .get_or_init(|| {
            if let Ok(v) = std::env::var("NIE_GAMEDATA_JSON") {
                let p = PathBuf::from(v);
                return p.is_dir().then_some(p);
            }
            if let Ok(v) = std::env::var("NIE_GAME_DIR")
                && let Some(p) = depuis(Path::new(&v))
            {
                return Some(p);
            }
            let mut courant = std::env::current_dir().ok();
            while let Some(d) = courant {
                if let Some(p) = depuis(&d) {
                    return Some(p);
                }
                courant = d.parent().map(PathBuf::from);
            }
            None
        })
        .as_deref()
}

/// Charge un dump par son chemin **relatif à la racine `gamedata`**
/// (ex. `character/academic_year_config.cfg.bin.json`).
///
/// Rend `None` — en le signalant sur `stderr` — quand le corpus est absent, pour que le test
/// puisse se terminer sans échouer sur une machine qui n'a pas les données du jeu.
///
/// # Panics
/// Si le fichier existe mais n'est pas du JSON valide : c'est un corpus corrompu, pas une
/// absence, et le test doit le dire franchement.
#[must_use]
pub fn load(rel: &str) -> Option<serde_json::Value> {
    let Some(racine) = racine() else {
        eprintln!("skip : corpus gamedata introuvable — poser NIE_GAMEDATA_JSON ou NIE_GAME_DIR");
        return None;
    };
    let chemin = racine.join(rel);
    if !chemin.is_file() {
        eprintln!("skip : {} absent du corpus", chemin.display());
        return None;
    }
    let texte = std::fs::read_to_string(&chemin)
        .unwrap_or_else(|e| panic!("lecture de {} : {e}", chemin.display()));
    Some(
        serde_json::from_str(&texte)
            .unwrap_or_else(|e| panic!("JSON invalide dans {} : {e}", chemin.display())),
    )
}

/// Chemin absolu d'un dump, que le fichier existe ou non — pour les tests qui veulent ouvrir
/// eux-mêmes le fichier.
///
/// Rend `None` quand le corpus est absent de la machine, en l'annonçant **une fois par binaire
/// de test** : sans ce message, un golden qui ne s'exécute pas ne se distingue pas d'un golden
/// qui passe.
#[must_use]
pub fn chemin(rel: &str) -> Option<PathBuf> {
    let Some(racine) = racine() else {
        static DIT: OnceLock<()> = OnceLock::new();
        DIT.get_or_init(|| {
            eprintln!(
                "skip : corpus gamedata introuvable — poser NIE_GAMEDATA_JSON (dossier des dumps \
                 `*.cfg.bin.json`) ou NIE_GAME_DIR. Les goldens adossés au corpus ne s'exécutent pas."
            );
        });
        return None;
    };
    Some(racine.join(rel))
}
