//! La table de dispatch `decode` doit router **tout parseur autonome du crate**.
//!
//! Un parseur qui existe mais que `decode` ignore est invisible : ni la FFI, ni `niers decode`,
//! ni l'explorateur, ni le MCP ne l'atteignent. C'est ce qui est arrivé à `g4sk`, `navm`, `g4mt`,
//! `g4cm`, `g4la`, `g4ma`, `g4vs` et `col` — huit modules écrits, testés, et non branchés.
//!
//! Le test tourne sur le vrai corpus (installation ou dump, cf. `vfs::open_game`) et **annonce
//! son saut** si aucune donnée n'est disponible.

#![cfg(all(feature = "std", feature = "serde"))]

use std::collections::BTreeMap;

use nie_formats::vfs::{self, Vfs};

/// Monte le VFS, ou dit pourquoi il ne peut pas.
fn corpus() -> Option<Vfs> {
    match vfs::open_game() {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("skip : ni installation ni dump ({e:?})");
            None
        }
    }
}

/// Échantillon réparti de chemins portant `ext`, pour ne pas lire un dossier entier.
fn echantillon(vfs: &Vfs, ext: &str, max: usize) -> Vec<String> {
    let suffixe = format!(".{ext}");
    let mut chemins: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.ends_with(&suffixe))
        .collect();
    chemins.sort_unstable();
    if chemins.len() > max {
        let pas = chemins.len().div_ceil(max);
        chemins = chemins.into_iter().step_by(pas).collect();
    }
    chemins
}

/// Chaque extension a bien un décodeur qui l'accepte sur de VRAIS fichiers.
///
/// L'extension attendue n'est pas le nom du parseur (`.g4nv` est décodé par `navm`, `.col` par
/// `col (PXCL)`) : on vérifie qu'un décodage aboutit, et on rapporte lequel — c'est la
/// correspondance extension → parseur que le dépôt prétend tenir.
#[test]
fn decode_route_les_familles_level5_annexes() {
    let Some(vfs) = corpus() else { return };

    // Extensions dont un parseur AUTONOME existe dans le crate. `.g4mg` en est volontairement
    // absent : sa géométrie n'a de sens qu'avec le `.g4md` frère, il n'y a rien à décoder seul.
    let familles = [
        "g4mt", "g4cm", "g4la", "g4ma", "g4vs", "col", "g4nv", "g4sk", "vfxo", "pfxo", "gfxo",
        "cfxo",
    ];

    // (décodés, essayés, nom du parseur) : afficher le dénominateur évite de lire « 4 décodés »
    // comme un échec partiel quand la famille ne compte que 4 fichiers dans tout le jeu.
    let mut vus: BTreeMap<&str, (usize, usize, String)> = BTreeMap::new();
    let mut absents: Vec<&str> = Vec::new();
    for ext in familles {
        let chemins = echantillon(&vfs, ext, 5);
        if chemins.is_empty() {
            absents.push(ext);
            continue;
        }
        let mut ok = 0usize;
        let mut nom = String::new();
        for chemin in &chemins {
            let Ok(octets) = vfs.read(chemin) else {
                continue;
            };
            if let Some(d) = nie_formats::decode::decode(&octets) {
                ok += 1;
                nom = d.format.to_string();
            }
        }
        assert!(
            ok > 0,
            "aucun `.{ext}` décodé sur {} essais — la table de dispatch ne le route pas",
            chemins.len(),
        );
        vus.insert(ext, (ok, chemins.len(), nom));
    }

    for (ext, (ok, essayes, nom)) in &vus {
        eprintln!("  .{ext} → {nom} ({ok}/{essayes} fichiers décodés)");
    }
    if !absents.is_empty() {
        eprintln!("  extensions absentes de ce corpus : {absents:?}");
    }
    assert!(
        vus.len() >= 4,
        "corpus trop pauvre pour conclure : {} familles vues",
        vus.len()
    );
}

/// Un `.g4mg` n'est PAS décodable seul, et le dépôt ne doit pas prétendre le contraire.
///
/// Ces fichiers n'ont aucun magic — ce sont des tampons de sommets bruts dont la structure vit
/// dans le `.g4md` frère. Un `decode` qui rendrait quelque chose ici signalerait qu'un parseur
/// accepte n'importe quoi, ce qui volerait des fichiers aux vrais parseurs.
#[test]
fn un_g4mg_ne_pretend_pas_se_decoder_seul() {
    let Some(vfs) = corpus() else { return };
    let chemins = echantillon(&vfs, "g4mg", 8);
    if chemins.is_empty() {
        eprintln!("skip : aucun .g4mg dans ce corpus");
        return;
    }
    let mut testes = 0usize;
    for chemin in &chemins {
        let Ok(octets) = vfs.read(chemin) else {
            continue;
        };
        testes += 1;
        assert!(
            nie_formats::decode::decode(&octets).is_none(),
            "{chemin} a été « décodé » alors qu'un g4mg n'a pas de structure autonome",
        );
        assert_eq!(
            nie_formats::detect(&octets),
            nie_formats::FileFormat::Unknown,
            "{chemin} : un g4mg réel ne porte pas de magic",
        );
    }
    assert!(testes > 0, "aucun .g4mg lisible");
    eprintln!("{testes} fichiers .g4mg : ni magic ni décodage autonome, comme attendu");
}
