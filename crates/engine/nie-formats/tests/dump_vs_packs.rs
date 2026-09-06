//! Le dump et les packs doivent être **interchangeables** : mêmes chemins logiques, mêmes
//! octets. C'est la condition pour faire tourner le moteur sur une machine qui n'a que le dump.
//!
//! Les deux tests s'exécutent seulement quand les deux montages sont visibles en même temps —
//! sinon il n'y a rien à comparer — et **annoncent leur saut** : un test muet qui ne lit rien
//! est un faux vert.

use nie_formats::vfs::{self, Vfs};

/// Monte les deux côtés, ou explique pourquoi il n'y en a qu'un.
///
/// L'installation est celle de `resolve_game_dir`, le dump celui de `resolve_dump_dir` — donc
/// `NIE_DUMP_DIR`, sinon un `data/` du dépôt portant `common/`. Les deux doivent désigner des
/// répertoires **distincts** : comparer un montage à lui-même ne prouve rien.
fn deux_montages() -> Option<(Vfs, Vfs)> {
    let install = vfs::resolve_game_dir().join("data");
    if !install.join("cpk_list.cfg.bin").is_file() {
        eprintln!("skip : aucune installation du jeu (pas de cpk_list.cfg.bin)");
        return None;
    }
    let dump = vfs::resolve_dump_dir()?;
    if dump == install {
        eprintln!("skip : le dump et l'installation sont le même répertoire");
        return None;
    }

    let mut packs = Vfs::new();
    packs.init(&install).expect("montage packs");
    let mut extrait = Vfs::new();
    extrait.init_loose(&dump).expect("montage dump");
    assert!(!packs.is_dump(), "l'installation doit se monter par packs");
    assert!(extrait.is_dump(), "le dump doit se monter en mode dump");
    Some((packs, extrait))
}

/// Le dump sert les chemins **logiques** du jeu, pas ses chemins disque relatifs.
///
/// C'est tout l'enjeu du branchement : `data/common/…` doit désigner la même chose des deux
/// côtés, sinon chaque consommateur devrait savoir sur quoi il tourne.
#[test]
fn le_dump_expose_les_chemins_logiques_du_jeu() {
    let Some((packs, dump)) = deux_montages() else {
        return;
    };

    let mut communs = 0usize;
    let mut absents = Vec::new();
    for (chemin, _) in packs.iter().take(20_000) {
        if !chemin.starts_with("data/") {
            continue;
        }
        if dump.is_readable(chemin) {
            communs += 1;
        } else {
            absents.push(chemin.to_string());
        }
    }
    assert!(communs > 1_000, "trop peu de chemins communs : {communs}");
    // Un dump peut être partiel (extraction interrompue) ; il ne doit pas être *décalé*.
    let taux = communs as f64 / (communs + absents.len()) as f64;
    assert!(
        taux > 0.90,
        "seulement {:.1} % des chemins de l'index sont servis par le dump — premiers manquants : {:?}",
        taux * 100.0,
        &absents[..absents.len().min(5)],
    );
    eprintln!(
        "chemins logiques communs : {communs} ({:.1} %)",
        taux * 100.0
    );
}

/// Même chemin, mêmes octets. Un dump qui rend un contenu différent sous un nom juste est pire
/// qu'un dump absent : il ferait diverger le moteur en silence.
#[test]
fn le_dump_rend_les_memes_octets_que_les_packs() {
    let Some((packs, dump)) = deux_montages() else {
        return;
    };

    // Échantillon déterministe et bon marché : les plus petits fichiers d'un dossier de données
    // stable. Extraire un CPK coûte cher — inutile d'en lire des milliers pour prouver l'égalité.
    let mut candidats: Vec<(&str, u32)> = packs
        .iter()
        .filter(|(p, e)| p.starts_with("data/common/gamedata/") && e.file_size > 0)
        .map(|(p, e)| (p, e.file_size))
        .collect();
    candidats.sort_by_key(|(p, taille)| (*taille, *p));
    candidats.truncate(12);
    assert!(
        !candidats.is_empty(),
        "aucun gamedata dans l'index des packs"
    );

    let mut compares = 0usize;
    for (chemin, _) in &candidats {
        let Ok(depuis_packs) = packs.read(chemin) else {
            continue;
        };
        let Ok(depuis_dump) = dump.read(chemin) else {
            eprintln!("absent du dump : {chemin}");
            continue;
        };
        assert_eq!(
            depuis_packs,
            depuis_dump,
            "octets divergents pour {chemin} ({} vs {} octets)",
            depuis_packs.len(),
            depuis_dump.len(),
        );
        compares += 1;
    }
    assert!(
        compares >= 5,
        "seulement {compares} fichiers comparés — preuve trop faible"
    );
    eprintln!("{compares} fichiers identiques octet pour octet entre packs et dump");
}
