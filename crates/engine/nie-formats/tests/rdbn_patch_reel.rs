//! Le patch RDBN en place doit tenir sur les **vrais fichiers du jeu**, pas sur une fixture.
//!
//! Trois propriétés font tout l'intérêt du module face au réencodage : la taille ne bouge pas,
//! seuls les octets du champ visé changent, et la relecture par le parseur rend la nouvelle
//! valeur. Ce test les vérifie sur `system/level_limit_config`, un fichier court dont on connaît
//! la forme (deux listes, un champ `level` Int, un champ `rarity` Int).
//!
//! Le test **annonce son saut** quand le jeu n'est pas monté — un test muet est un faux vert.

use nie_formats::cfgbin::{self, RdbnValue};
use nie_formats::rdbn_patch::{Modif, PatchError, Val, localiser, patch_verifie};
use nie_formats::vfs;

/// Lit un `cfg.bin` du jeu, ou explique pourquoi le test ne peut pas tourner.
fn lire(chemin_partiel: &str) -> Option<Vec<u8>> {
    let vfs = match vfs::open_game() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("skip : VFS indisponible ({e})");
            return None;
        }
    };
    let chemin = vfs
        .iter()
        .map(|(c, _)| c.to_string())
        .find(|c| c.contains(chemin_partiel))?;
    match vfs.read(&chemin) {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("skip : {chemin} illisible ({e})");
            None
        }
    }
}

/// Valeur d'un champ, relue par le parseur.
fn valeur(data: &[u8], liste: &str, ligne: usize, champ: &str) -> Option<RdbnValue> {
    let rdbn = cfgbin::parse(data).ok()?;
    cfgbin::read_values(&rdbn, data)
        .into_iter()
        .find(|l| l.name == liste)
        .and_then(|l| l.rows.into_iter().nth(ligne))
        .and_then(|r| r.fields.into_iter().find(|(k, _)| k == champ))
        .map(|(_, v)| v)
}

#[test]
fn patch_en_place_preserve_tout_sauf_le_champ_vise() {
    let Some(vanilla) = lire("system/level_limit_config") else {
        eprintln!("skip : level_limit_config absent");
        return;
    };

    let avant = valeur(&vanilla, "m_LevelLimitInfoList", 0, "level")
        .expect("le champ level existe dans le fichier réel");
    let RdbnValue::Int(niveau_origine) = avant else {
        panic!("level devrait être un Int, lu : {avant:?}");
    };

    // Viser une valeur **différente** de celle en place : sur une installation moddée, le fichier
    // servi par le VFS peut déjà porter la cible, et un patch qui ne change rien ne prouve rien.
    let cible = if niveau_origine == 99 { 42 } else { 99 };

    let mut patche = vanilla.clone();
    let modifs = [Modif {
        liste: String::from("m_LevelLimitInfoList"),
        ligne: 0,
        champ: String::from("level"),
        valeur: Val::I32(cible),
    }];
    let verif = patch_verifie(&mut patche, &modifs).expect("le patch s'applique");

    // 1. La taille ne bouge pas.
    assert!(verif.taille_preservee(), "taille modifiée : {verif:?}");
    assert_eq!(patche.len(), vanilla.len());

    // 2. Seuls les octets du champ visé diffèrent (4 au plus pour un Int).
    let diffs: Vec<usize> = vanilla
        .iter()
        .zip(patche.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    let rdbn = cfgbin::parse(&vanilla).expect("RDBN");
    let loc = localiser(&rdbn, "m_LevelLimitInfoList", 0, "level").expect("champ localisé");
    assert!(
        diffs
            .iter()
            .all(|o| (loc.offset..loc.offset + loc.size).contains(o)),
        "octets modifiés hors du champ : {diffs:?} (champ à 0x{:X}..0x{:X})",
        loc.offset,
        loc.offset + loc.size
    );
    assert!(
        diffs.len() <= 4,
        "un Int tient sur 4 octets, {} modifiés",
        diffs.len()
    );
    assert!(
        !diffs.is_empty(),
        "le patch aurait dû changer au moins un octet"
    );

    // 3. La relecture rend la nouvelle valeur.
    assert_eq!(
        valeur(&patche, "m_LevelLimitInfoList", 0, "level"),
        Some(RdbnValue::Int(cible))
    );

    // 4. Le vanilla est intact — on n'a pas patché la source par effet de bord.
    assert_eq!(
        valeur(&vanilla, "m_LevelLimitInfoList", 0, "level"),
        Some(RdbnValue::Int(niveau_origine))
    );
}

#[test]
fn ecrire_le_mauvais_type_est_refuse() {
    let Some(vanilla) = lire("system/level_limit_config") else {
        eprintln!("skip : level_limit_config absent");
        return;
    };
    let mut data = vanilla.clone();

    // `level` est un Int : y écrire un octet doit échouer, et ne rien modifier.
    let modifs = [Modif {
        liste: String::from("m_LevelLimitInfoList"),
        ligne: 0,
        champ: String::from("level"),
        valeur: Val::U8(99),
    }];
    let err = patch_verifie(&mut data, &modifs).expect_err("un type incompatible doit échouer");
    assert!(
        matches!(err, PatchError::TypeIncompatible { .. }),
        "erreur inattendue : {err:?}"
    );
    assert_eq!(data, vanilla, "un patch refusé ne doit rien écrire");
}

#[test]
fn une_coordonnee_inexistante_est_refusee() {
    let Some(vanilla) = lire("system/level_limit_config") else {
        eprintln!("skip : level_limit_config absent");
        return;
    };
    let rdbn = cfgbin::parse(&vanilla).expect("RDBN");

    assert!(matches!(
        localiser(&rdbn, "m_ListeQuiNExistePas", 0, "level"),
        Err(PatchError::ListeInconnue(_))
    ));
    assert!(matches!(
        localiser(&rdbn, "m_LevelLimitInfoList", 9_999, "level"),
        Err(PatchError::LigneHorsPlage { .. })
    ));
    assert!(matches!(
        localiser(&rdbn, "m_LevelLimitInfoList", 0, "champInexistant"),
        Err(PatchError::ChampInconnu(_))
    ));
}
