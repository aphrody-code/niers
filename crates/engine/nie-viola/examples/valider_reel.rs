//! Validation bout-en-bout des opérations Viola **sur le vrai jeu**.
//!
//! Le dépôt n'accepte un FAIT que validé sur le réel : un test à fixtures synthétiques n'a jamais
//! prouvé qu'un pack se recharge ni qu'un dump extrait les bons octets. Cet exemple s'exécute
//! (contrairement aux tests de `apps/inacord/src-tauri`, qui ne démarrent pas sur ce poste).
//!
//! ```text
//! cargo run -p nie-viola --example valider_reel --release
//! ```
//!
//! Il n'écrit que dans un dossier temporaire, jamais dans l'installation du jeu.

use std::sync::atomic::AtomicBool;

use nie_formats::vfs::Vfs;
use nie_viola::{
    CriwareKey, DumpOptions, MergeStrategy, Platform, crypt_file, decode_cpk_list, dump_all,
    encode_cpk_list, merge_dirs, pack_mod,
};

fn main() -> Result<(), String> {
    let racine = nie_formats::vfs::resolve_game_dir();
    let data = racine.join("data");
    if !nie_formats::vfs::donnees_disponibles(&data) {
        return Err(format!("jeu introuvable sous {} — rien à valider", racine.display()));
    }
    println!("jeu : {}", racine.display());

    let mut vfs = Vfs::new();
    vfs.init(&data).map_err(|e| e.to_string())?;
    println!("VFS : {} fichiers, {} packs\n", vfs.asset_count(), vfs.cpk_count());

    let tmp = std::env::temp_dir().join("nie-viola-validation");
    std::fs::remove_dir_all(&tmp).ok();

    let mut echecs = 0;

    // ── 1. cpk_list : aller-retour sur le VRAI fichier ───────────────────────────────────────
    let brut = std::fs::read(data.join("cpk_list.cfg.bin")).map_err(|e| e.to_string())?;
    let (cfg, enveloppe) = decode_cpk_list(&brut)?;
    let entrees = cfg.entries.first().map_or(0, |r| r.children.len());
    println!("[1] cpk_list réel : {entrees} entrées, enveloppe {enveloppe:?}");

    let reencode = encode_cpk_list(&cfg.entries, enveloppe);
    let (cfg2, env2) = decode_cpk_list(&reencode)?;
    let entrees2 = cfg2.entries.first().map_or(0, |r| r.children.len());
    if entrees2 == entrees && env2 == enveloppe {
        println!("    OK — réencodé puis relu : {entrees2} entrées, même enveloppe");
    } else {
        println!("    ECHEC — {entrees} entrées → {entrees2}, {enveloppe:?} → {env2:?}");
        echecs += 1;
    }

    // ── 2. Dump filtré, comparé octet à octet à la lecture VFS ───────────────────────────────
    // Un filtre étroit : on valide la correction, pas le débit (le dump complet fait ~57 Gio).
    let sortie = tmp.join("dump");
    let options = DumpOptions {
        filtre: Some("data/common/gamedata/*".to_string()),
        reprise: true,
        sauter_identiques: true,
        threads: None,
        ..DumpOptions::default()
    };
    let annuler = AtomicBool::new(false);
    let debut = std::time::Instant::now();
    let rapport = dump_all(&vfs, &sortie, &options, &annuler, &|_| {})?;
    println!(
        "\n[2] dump filtré : {} extraits, {} sautés, {} échecs, {} octets en {:?}",
        rapport.extraits, rapport.sautes, rapport.echecs, rapport.octets, debut.elapsed()
    );

    // Chaque fichier extrait doit être EXACTEMENT ce que rend le VFS.
    let mut compares = 0;
    let mut divergents = 0;
    for (chemin, _) in vfs.iter() {
        if !nie_viola::glob_match(options.filtre.as_deref().unwrap_or("*"), chemin) {
            continue;
        }
        let attendu = match vfs.read(chemin) {
            Ok(o) => o,
            Err(_) => continue,
        };
        let obtenu = std::fs::read(sortie.join(chemin)).unwrap_or_default();
        compares += 1;
        if obtenu != attendu {
            divergents += 1;
            if divergents <= 3 {
                println!("    divergent : {chemin} ({} vs {} octets)", obtenu.len(), attendu.len());
            }
        }
    }
    if divergents == 0 && compares > 0 {
        println!("    OK — {compares} fichiers identiques à la lecture VFS, octet pour octet");
    } else {
        println!("    ECHEC — {divergents}/{compares} divergents");
        echecs += 1;
    }

    // Reprise : un second dump ne doit RIEN réécrire.
    let r2 = dump_all(&vfs, &sortie, &options, &annuler, &|_| {})?;
    if r2.extraits == 0 && r2.packs_repris > 0 {
        println!("    OK — reprise : 2ᵉ passage, {} packs sautés, 0 réécriture", r2.packs_repris);
    } else {
        println!("    ECHEC — reprise inopérante ({} réécrits)", r2.extraits);
        echecs += 1;
    }

    // ── 3. Pack d'un mod minuscule contre le VRAI cpk_list ───────────────────────────────────
    // On prend un fichier réel du jeu, on le modifie, et on vérifie que son entrée bascule bien
    // en « loose » avec la bonne taille — c'est ce qui fait charger le mod par le jeu.
    let cible = vfs
        .iter()
        .find(|(p, e)| p.ends_with(".cfg.bin") && !e.cpk_filename.is_empty())
        .map(|(p, _)| p.to_string())
        .ok_or("aucun fichier de config empaqueté trouvé")?;
    let contenu = vfs.read(&cible).map_err(|e| e.to_string())?;

    let mod_dir = tmp.join("mod");
    let chemin_mod = mod_dir.join(&cible);
    std::fs::create_dir_all(chemin_mod.parent().ok_or("pas de parent")?).map_err(|e| e.to_string())?;
    let mut modifie = contenu.clone();
    modifie.extend_from_slice(b"\0\0\0\0"); // taille volontairement différente
    std::fs::write(&chemin_mod, &modifie).map_err(|e| e.to_string())?;

    let sortie_pack = tmp.join("pack");
    let pr = pack_mod(&data.join("cpk_list.cfg.bin"), &mod_dir, &sortie_pack, Platform::Pc)?;
    println!(
        "\n[3] pack : {} mis à jour, {} ajoutés, {} copiés, {} entrées, {} déjà loose",
        pr.mis_a_jour, pr.ajoutes, pr.copies, pr.total, pr.loose_avant
    );

    // Relire le cpk_list produit et vérifier l'entrée de la cible.
    let produit = std::fs::read(sortie_pack.join(Platform::Pc.cpk_list_rel())).map_err(|e| e.to_string())?;
    let (cfg_pack, _) = decode_cpk_list(&produit)?;
    let mut trouve = false;
    if let Some(r) = cfg_pack.entries.first() {
        for enfant in &r.children {
            if enfant.variables.len() < 5 {
                continue;
            }
            let (
                nie_formats::cfgbin::Value::String(dir),
                nie_formats::cfgbin::Value::String(nom),
            ) = (&enfant.variables[0], &enfant.variables[1])
            else {
                continue;
            };
            if format!("{dir}{nom}") != cible {
                continue;
            }
            trouve = true;
            let pack_vide = matches!(&enfant.variables[3], nie_formats::cfgbin::Value::String(s) if s.is_empty());
            let taille_ok = matches!(&enfant.variables[4], nie_formats::cfgbin::Value::Int(n) if *n as usize == modifie.len());
            if pack_vide && taille_ok {
                println!("    OK — {cible} : pack vidé, taille {} inscrite", modifie.len());
            } else {
                println!("    ECHEC — pack_vide={pack_vide}, taille_ok={taille_ok}");
                echecs += 1;
            }
        }
    }
    if !trouve {
        println!("    ECHEC — entrée {cible} absente du cpk_list produit");
        echecs += 1;
    }

    // ── 4. Merge sémantique sur un VRAI .cfg.bin ─────────────────────────────────────────────
    // Deux mods qui changent des champs DIFFÉRENTS du même fichier : au fichier (Viola), l'un des
    // deux serait perdu ; au champ, les deux doivent survivre.
    // On cherche un T2B réel offrant au moins deux entiers éditables ET dont l'encodeur fait un
    // aller-retour exact : sans cette seconde condition, un écart mesuré viendrait de l'encodeur,
    // pas de la fusion.
    let mut chemin_t2b = String::new();
    let mut vanilla = Vec::new();
    let mut variables: Vec<(usize, usize)> = Vec::new();
    for (p, _) in vfs.iter() {
        if !p.ends_with(".cfg.bin") || nie_formats::cfgbin::is_rdbn(&vfs.read(p).unwrap_or_default())
        {
            continue;
        }
        let Ok(octets) = vfs.read(p) else { continue };
        let Ok(cfg) = nie_formats::cfgbin::cfgbin_parse(&octets) else { continue };
        // Témoin valable : l'encodeur doit rendre la MÊME STRUCTURE (l'égalité binaire n'est pas
        // garantie par `encode_t2b`, seule l'égalité structurelle l'est). Sans cette condition,
        // un écart mesuré viendrait de l'encodeur et non de la fusion.
        let Ok(retour) = nie_formats::cfgbin::cfgbin_parse(&nie_formats::cfgbin::encode_t2b(&cfg.entries))
        else {
            continue;
        };
        if retour.entries != cfg.entries {
            continue;
        }
        let mut vars = Vec::new();
        for (i, e) in cfg.entries.iter().enumerate() {
            for (k, v) in e.variables.iter().enumerate() {
                if matches!(v, nie_formats::cfgbin::Value::Int(_)) {
                    vars.push((i, k));
                }
            }
        }
        if vars.len() >= 2 {
            chemin_t2b = p.to_string();
            vanilla = octets;
            variables = vars;
            break;
        }
    }

    if variables.len() < 2 {
        println!("\n[4] merge sémantique : aucun T2B témoin trouvé — ECHEC");
        echecs += 1;
    } else {
        let base = nie_formats::cfgbin::cfgbin_parse(&vanilla).map_err(|e| e.to_string())?;
        let ecrire_variante = |dossier: &str, (i, k): (usize, usize), valeur: i32| -> Result<(), String> {
            let mut c = base.clone();
            c.entries[i].variables[k] = nie_formats::cfgbin::Value::Int(valeur);
            let octets = nie_formats::cfgbin::encode_t2b(&c.entries);
            let p = tmp.join(dossier).join(&chemin_t2b);
            std::fs::create_dir_all(p.parent().ok_or("pas de parent")?).map_err(|e| e.to_string())?;
            std::fs::write(p, octets).map_err(|e| e.to_string())
        };
        ecrire_variante("modA", variables[0], 1234)?;
        ecrire_variante("modB", variables[1], 5678)?;

        let sortie_merge = tmp.join("merge");
        let resolveur = |chemin: &str| vfs.read(chemin).ok();
        let mr = merge_dirs(
            &[tmp.join("modA"), tmp.join("modB")],
            &sortie_merge,
            &MergeStrategy::Semantique(&resolveur),
        )?;
        println!(
            "\n[4] merge sémantique sur {chemin_t2b} : {} fusionnés, {} copiés, {} chemins disputés",
            mr.fusionnes, mr.copies, mr.conflits.len()
        );
        if let Some(c) = mr.conflits.first() {
            println!(
                "    {} champs fusionnés, {} en désaccord, repli : {:?}",
                c.champs_fusionnes, c.champs_en_desaccord, c.repli
            );
        }

        let fusionne = std::fs::read(sortie_merge.join(&chemin_t2b)).map_err(|e| e.to_string())?;
        let relu = nie_formats::cfgbin::cfgbin_parse(&fusionne).map_err(|e| e.to_string())?;
        let a_ok = relu.entries[variables[0].0].variables[variables[0].1]
            == nie_formats::cfgbin::Value::Int(1234);
        let b_ok = relu.entries[variables[1].0].variables[variables[1].1]
            == nie_formats::cfgbin::Value::Int(5678);
        if a_ok && b_ok {
            println!("    OK — les DEUX modifications survivent (au fichier, l'une serait perdue)");
        } else {
            println!("    ECHEC — modA conservé={a_ok}, modB conservé={b_ok}");
            echecs += 1;
        }
    }

    // ── 5. Crypto Criware : aller-retour fichier sur un VRAI pack ────────────────────────────
    let pack = std::fs::read_dir(data.join("packs"))
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "cpk"))
        .ok_or("aucun .cpk trouvé")?;
    let nom = pack.file_name().unwrap_or_default().to_string_lossy().to_string();
    let cle = CriwareKey::DuNom(nom.clone());
    let a = tmp.join("crypto/a.bin");
    let b = tmp.join("crypto/b.bin");
    let n = crypt_file(&pack, &a, &cle)?;
    crypt_file(&a, &b, &cle)?;
    let original = std::fs::read(&pack).map_err(|e| e.to_string())?;
    let retour = std::fs::read(&b).map_err(|e| e.to_string())?;
    println!("\n[5] crypto Criware sur {nom} ({n} octets, par tranches)");
    if original == retour {
        println!("    OK — aller-retour identique sur un pack réel de {n} octets");
    } else {
        println!("    ECHEC — l'aller-retour ne rend pas l'original");
        echecs += 1;
    }

    std::fs::remove_dir_all(&tmp).ok();
    println!("\n═══ {} ═══", if echecs == 0 { "TOUT VERT".to_string() } else { format!("{echecs} ECHEC(S)") });
    if echecs > 0 { Err(format!("{echecs} validation(s) en échec")) } else { Ok(()) }
}
