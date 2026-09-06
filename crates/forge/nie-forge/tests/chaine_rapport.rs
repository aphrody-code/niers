//! La chaîne que consomment **les deux façades** de la mesure : la CLI
//! `nie-forge report` et l'onglet « Forge » de `nie-explorer`.
//!
//! L'explorateur ne shelle pas vers le binaire et ne lit pas un rapport figé :
//! il rappelle `ForgeStore::load` → `Registry::load` → `AsmSource::load_dir` →
//! `Report::build`, exactement comme la commande. Ce test l'exécute sur les
//! artefacts réels du dépôt et vérifie que le résultat est cohérent, pour que
//! les deux affichages ne puissent pas diverger sans qu'on le voie.
//!
//! Le test **s'annonce et se saute** quand `var/forge/` n'a pas encore été
//! produit : un test muet qui ne s'exécute pas est un faux vert, et la forge
//! n'est pas recouverte sur toutes les machines (`nie-forge split` est long).

use std::path::{Path, PathBuf};

/// Remonte jusqu'à la racine du dépôt (celle qui porte `forge/registry.json`).
fn repo_root() -> Option<PathBuf> {
    let mut cur: Option<&Path> = Some(Path::new(env!("CARGO_MANIFEST_DIR")));
    while let Some(dir) = cur {
        if dir.join("forge").join("registry.json").is_file() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

#[test]
fn le_rapport_se_reconstruit_depuis_les_artefacts() {
    let Some(root) = repo_root() else {
        println!("SAUTÉ : racine du dépôt introuvable (pas de forge/registry.json)");
        return;
    };
    let forge = root.join("var").join("forge");
    if !forge.join("cover.json").is_file() {
        println!(
            "SAUTÉ : {} absent — lancer `nie-forge split` pour produire le recouvrement",
            forge.join("cover.json").display()
        );
        return;
    }

    let store = nie_forge::ForgeStore::load(&forge).expect("recouvrement");
    let registry =
        nie_forge::Registry::load(&root.join("forge").join("registry.json")).expect("registre");
    let asm = nie_forge::AsmSource::load_dir(&root.join("forge").join("asm")).expect("source asm");
    let mut r = nie_forge::Report::build(&store.cover, &registry, &asm).expect("rapport");
    // Les deux façades ajoutent les sections-tables ré-émises : sans cela la
    // mesure sous-déclare de 4,2 points sur `nie.exe`.
    let exe = root.join("nie.exe");
    if let Ok(bytes) = std::fs::read(&exe)
        && let Ok(img) = nie_pe::PeImage::parse(bytes)
    {
        r.add_emitted_tables(&store.cover, &img);
    }

    // Invariants de forme — ce que les deux façades affichent doit être cohérent
    // entre soi, quel que soit l'avancement de la forge.
    assert!(r.total_bytes > 0, "le binaire cible a une taille");
    assert!(r.code_bytes > 0 && r.code_bytes <= r.total_bytes);
    assert!(
        r.produced_bytes() <= r.total_bytes,
        "on ne produit pas plus d'octets que le fichier n'en compte"
    );
    let pct = r.produced_pct();
    assert!(
        (0.0..=100.0).contains(&pct),
        "part produite hors bornes : {pct}"
    );
    assert!(
        (0.0..=100.0).contains(&r.code_pct()),
        "part du .text hors bornes : {}",
        r.code_pct()
    );

    // La part produite se recompose exactement de ses seaux — et `semantic`
    // n'en fait **pas** partie : un portage validé sémantiquement n'a pas
    // produit d'octet.
    let somme = r.emitted.bytes + r.assembled.bytes + r.matched_bytes.bytes;
    assert_eq!(
        somme,
        r.produced_bytes(),
        "les seaux comptés doivent sommer à la part produite"
    );

    println!(
        "forge : {:.4} % du fichier, {:.4} % du .text ({} / {} octets)",
        pct,
        r.code_pct(),
        r.produced_bytes(),
        r.total_bytes
    );
}

/// L'onglet « Forge » et `nie-forge lift` affichent la **même** liste de
/// blocages : c'est la même fonction qui l'agrège.
///
/// Une boucle recopiée de part et d'autre avait déjà fait diverger la *mesure*
/// de 4,2 points ; ce test couvre l'autre moitié de la façade.
#[test]
fn les_blocages_sont_agreges_par_une_seule_fonction() {
    let Some(root) = repo_root() else {
        println!("SAUTÉ : racine du dépôt introuvable");
        return;
    };
    let forge = root.join("var").join("forge");
    let exe = root.join("nie.exe");
    if !forge.join("cover.json").is_file() || !exe.is_file() {
        println!("SAUTÉ : recouvrement ou binaire absent — lancer `nie-forge split`");
        return;
    }

    let store = nie_forge::ForgeStore::load(&forge).expect("recouvrement");
    let bytes = std::fs::read(&exe).expect("binaire");
    let all = nie_forge::lift::blockers(&store.cover, &bytes, 0);

    // Trié par octets décroissants : la première ligne est la prochaine cible,
    // c'est ce que promettent l'onglet et la CLI.
    for pair in all.windows(2) {
        assert!(
            pair[0].bytes >= pair[1].bytes,
            "liste non triée : {} ({} o) avant {} ({} o)",
            pair[0].cause,
            pair[0].bytes,
            pair[1].cause,
            pair[1].bytes
        );
    }
    // Chaque cause porte au moins une unité et un exemple exploitable : une
    // ligne sans instruction désassemblée ne dirait pas quoi implémenter.
    for b in &all {
        assert!(b.units > 0, "cause `{}` sans unité", b.cause);
        assert!(!b.cause.is_empty());
        assert!(!b.sample.is_empty(), "cause `{}` sans exemple", b.cause);
    }
    if let Some(first) = all.first() {
        println!(
            "prochaine cible : {} — {} unités, {} octets ({})",
            first.cause, first.units, first.bytes, first.sample
        );
    }
}
