//! Valide le patch d'octets sur le **vrai** `cpk_list.cfg.bin`, sans rien écrire dans le jeu.
//!
//! Vérifie, pour les chemins donnés : taille inchangée, nombre d'octets modifiés exactement égal
//! à 4 par chemin, relecture T2B possible, `cpk` désormais vide, et rechiffrement de même taille.
//!
//! Usage : `cargo run -p nie-viola --example cpk_list_patch_check -- <cpk_list.cfg.bin> <chemin…>`

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: cpk_list_patch_check <cpk_list> <chemin…>");
    let chemins: Vec<String> = args.collect();
    assert!(!chemins.is_empty(), "donner au moins un chemin VFS");

    let brut = std::fs::read(&path).expect("lecture");
    let clair = nie_formats::cpk::decrypt_cpk_list(&brut).expect("déchiffrement");
    println!("chiffré   {} octets", brut.len());
    println!("clair     {} octets", clair.len());

    let mut patche = clair.clone();
    let r = nie_viola::patch::patcher_clair(&mut patche, &chemins).expect("patch");
    println!("\nrendus loose  {:?}", r.rendus_loose);
    println!("déjà loose    {:?}", r.deja_loose);
    println!("introuvables  {:?}", r.introuvables);
    println!("octets modifiés {}", r.octets_modifies);

    assert!(r.introuvables.is_empty(), "chemin(s) absent(s) du cpk_list");
    assert_eq!(patche.len(), clair.len(), "la taille du clair a changé");
    let diff = patche
        .iter()
        .zip(clair.iter())
        .filter(|(a, b)| a != b)
        .count();
    println!("diff réel     {diff} octets");
    assert_eq!(
        diff,
        4 * r.rendus_loose.len(),
        "plus d'octets modifiés que prévu"
    );

    // Relecture : le fichier doit rester un T2B valide, et les chemins visés être loose.
    let cfg = nie_formats::cfgbin::cfgbin_parse(&patche).expect("le patché doit se reparser");
    let racine = cfg.entries.first().expect("entrée racine");
    let mut verifies = 0usize;
    for e in &racine.children {
        if e.variables.len() < 5 {
            continue;
        }
        if let (
            nie_formats::cfgbin::Value::String(d),
            nie_formats::cfgbin::Value::String(n),
            nie_formats::cfgbin::Value::String(c),
        ) = (&e.variables[0], &e.variables[1], &e.variables[3])
        {
            let plein = format!("{d}{n}");
            if chemins.iter().any(|x| x.replace('\\', "/") == plein) {
                assert!(c.is_empty(), "« {plein} » n'est pas loose après patch");
                verifies += 1;
            }
        }
    }
    println!("vérifiés      {verifies} chemin(s) désormais loose");

    let rechiffre = nie_formats::cpk::encrypt_cpk_list(&patche);
    assert_eq!(rechiffre.len(), brut.len(), "la taille chiffrée a changé");
    println!(
        "rechiffré     {} octets (taille d'origine conservée)",
        rechiffre.len()
    );
    println!("\nverdict   patch VALIDE — prêt à écrire");
}
