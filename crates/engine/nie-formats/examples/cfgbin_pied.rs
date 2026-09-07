//! Le pied de page des `.cfg.bin` T2B : ce que `parse_t2b` ignore et que `encode_t2b` réémet
//! avec une variante conservatrice.
//!
//! Mesure actuelle (`t2b_roundtrip`) : 4 fichiers sur 152 se réencodent à l'octet près ; le
//! réencodage **rogne encore**. Le cas de 48 octets qui a permis d'isoler le pied perdait 16
//! octets à l'offset 32 avant que `encode_t2b` ne l'émette. Ces 16 octets portent la chaîne `t2b`.
//!
//! Les tests d'aller-retour passent quand même (498/498 T2B) parce qu'ils comparent l'arbre
//! relu par notre propre décodeur, lequel n'ouvre jamais ce pied. C'est un vert qui ne prouve
//! rien sur ce que le jeu accepte.
//!
//! Cet exemple ne devine pas la sémantique du pied à partir d'un seul échantillon : il relève
//! les seize derniers octets de tous les T2B du jeu et ventile par motif, en marquant quels
//! octets sont **constants** sur tout le corpus et lesquels **varient**. Ce qui varie est un
//! champ ; ce qui ne varie pas est une signature.
//!
//! ```text
//! cargo run -p nie-formats --example cfgbin_pied --release
//! ```

use std::collections::BTreeMap;
use std::path::Path;

/// Taille du pied observé sur le plus petit cas.
const PIED: usize = 16;

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let data_dir = Path::new(&dir).join("data");

    let mut vfs = nie_formats::vfs::Vfs::new();
    if vfs.init(&data_dir).is_err() {
        eprintln!("skip : jeu absent à {}", data_dir.display());
        return;
    }

    let chemins: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.ends_with(".cfg.bin"))
        .collect();

    // Un octet est « constant » tant qu'on ne lui a vu qu'une seule valeur.
    let mut valeurs_vues: Vec<BTreeMap<u8, usize>> = vec![BTreeMap::new(); PIED];
    let mut motifs: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
    let mut n_t2b = 0usize;
    let mut n_avec_signature = 0usize;
    let mut n_trop_court = 0usize;
    let mut exemple_sans_signature: Option<String> = None;

    for chemin in &chemins {
        let Ok(octets) = vfs.read(chemin) else {
            continue;
        };
        if nie_formats::cfgbin::is_rdbn(&octets) {
            continue;
        }
        if nie_formats::cfgbin::parse_t2b(&octets).is_err() {
            continue;
        }
        n_t2b += 1;
        if octets.len() < PIED {
            n_trop_court += 1;
            continue;
        }
        let pied = &octets[octets.len() - PIED..];
        for (i, o) in pied.iter().enumerate() {
            *valeurs_vues[i].entry(*o).or_insert(0) += 1;
        }
        *motifs.entry(pied.to_vec()).or_insert(0) += 1;
        if pied.windows(3).any(|f| f == b"t2b") {
            n_avec_signature += 1;
        } else if exemple_sans_signature.is_none() {
            exemple_sans_signature = Some(chemin.clone());
        }
    }

    println!("{n_t2b} fichiers T2B lus ({n_trop_court} trop courts pour porter un pied)");
    println!("{n_avec_signature} portent la chaîne « t2b » dans leurs {PIED} derniers octets\n");

    println!("Position par position, sur les {PIED} derniers octets :");
    println!("{:>4}  {:<9} {:>8}  {}", "off", "état", "valeurs", "détail");
    for (i, vues) in valeurs_vues.iter().enumerate() {
        let etat = if vues.len() == 1 {
            "CONSTANT"
        } else {
            "variable"
        };
        let mut detail: Vec<String> = vues.iter().map(|(v, n)| format!("0x{v:02X}×{n}")).collect();
        detail.truncate(6);
        let suite = if vues.len() > 6 {
            format!(" … +{}", vues.len() - 6)
        } else {
            String::new()
        };
        println!(
            "{:>4}  {:<9} {:>8}  {}{}",
            i,
            etat,
            vues.len(),
            detail.join(" "),
            suite
        );
    }

    println!(
        "\n{} motif(s) de pied distinct(s). Les plus fréquents :",
        motifs.len()
    );
    let mut classes: Vec<(&Vec<u8>, &usize)> = motifs.iter().collect();
    classes.sort_by(|a, b| b.1.cmp(a.1));
    for (motif, n) in classes.iter().take(8) {
        let hexa: Vec<String> = motif.iter().map(|o| format!("{o:02X}")).collect();
        println!("  ×{n:<6} {}", hexa.join(" "));
    }

    if let Some(chemin) = &exemple_sans_signature {
        println!("\nPremier T2B SANS « t2b » dans son pied — à examiner avant de conclure :");
        println!("  {chemin}");
    } else if n_t2b > 0 {
        println!(
            "\nTous les T2B lus portent la signature : le pied est bien une partie du format."
        );
    }

    correler_octet_variable(&vfs, &chemins);
}

/// En-tête T2B, tel que `parse_t2b` le lit : quatre entiers de 32 bits.
struct Entete {
    entrees: i32,
    chaines_offset: i32,
    chaines_longueur: i32,
    chaines_nombre: i32,
}

fn entete(octets: &[u8]) -> Option<Entete> {
    if octets.len() < 16 {
        return None;
    }
    // La longueur est déjà garantie ≥ 16 : les quatre lectures ne peuvent pas échouer.
    let lire = |i: usize| i32::from_le_bytes(octets[i..i + 4].try_into().unwrap());
    Some(Entete {
        entrees: lire(0),
        chaines_offset: lire(4),
        chaines_longueur: lire(8),
        chaines_nombre: lire(12),
    })
}

/// Nombre d'entrées de la table de clés, lu là où `encode_t2b` l'écrit : après la table de
/// chaînes, aligné sur 16, second entier du sous-en-tête. `None` si le fichier s'arrête avant.
fn nombre_de_cles(octets: &[u8], e: &Entete) -> Option<i32> {
    if e.chaines_offset < 0 || e.chaines_longueur < 0 {
        return None;
    }
    let fin = (e.chaines_offset as usize).checked_add(e.chaines_longueur as usize)?;
    let debut = fin.div_ceil(16) * 16;
    let champ = debut.checked_add(4)?;
    if champ + 4 > octets.len() {
        return None;
    }
    Some(i32::from_le_bytes(
        octets[champ..champ + 4].try_into().ok()?,
    ))
}

/// Croise la seule valeur variable du pied (offset 6) avec ce que porte le fichier.
///
/// Le pied ne compte que deux motifs sur 70 798 fichiers : quinze octets constants et un seul
/// qui bascule entre `0x00` et `0x01`. Reste à savoir ce qu'il dit. On le confronte donc aux
/// grandeurs que l'en-tête donne — nombre d'entrées, de chaînes, de clés — et on regarde
/// laquelle le sépare proprement. Une grandeur qui vaut toujours zéro d'un côté et jamais de
/// l'autre est la réponse ; une grandeur qui se mélange ne l'est pas.
fn correler_octet_variable(vfs: &nie_formats::vfs::Vfs, chemins: &[String]) {
    /// Ce qu'on retient d'un groupe : les extrêmes et le compte des cas nuls.
    #[derive(Default)]
    struct Bilan {
        n: usize,
        entrees_nulles: usize,
        chaines_nulles: usize,
        cles_nulles: usize,
        cles_absentes: usize,
        entrees_max: i32,
        chaines_max: i32,
        cles_max: i32,
        exemple: Option<String>,
    }

    let mut groupes: [Bilan; 2] = [Bilan::default(), Bilan::default()];

    for chemin in chemins {
        let Ok(octets) = vfs.read(chemin) else {
            continue;
        };
        if nie_formats::cfgbin::is_rdbn(&octets) || octets.len() < PIED {
            continue;
        }
        if nie_formats::cfgbin::parse_t2b(&octets).is_err() {
            continue;
        }
        let drapeau = octets[octets.len() - PIED + 6];
        let Some(index) = (match drapeau {
            0x00 => Some(0usize),
            0x01 => Some(1usize),
            _ => None,
        }) else {
            continue;
        };
        let Some(e) = entete(&octets) else { continue };
        let cles = nombre_de_cles(&octets, &e);

        let b = &mut groupes[index];
        b.n += 1;
        if e.entrees == 0 {
            b.entrees_nulles += 1;
        }
        if e.chaines_nombre == 0 {
            b.chaines_nulles += 1;
        }
        match cles {
            None => b.cles_absentes += 1,
            Some(0) => b.cles_nulles += 1,
            Some(c) => b.cles_max = b.cles_max.max(c),
        }
        b.entrees_max = b.entrees_max.max(e.entrees);
        b.chaines_max = b.chaines_max.max(e.chaines_nombre);
        if b.exemple.is_none() {
            b.exemple = Some(chemin.clone());
        }
    }

    println!("\nL'octet variable (offset 6 du pied), croisé avec l'en-tête :");
    println!(
        "{:>7} {:>8} {:>14} {:>14} {:>12} {:>12} {:>11} {:>10} {:>10}",
        "valeur",
        "n",
        "entrées=0",
        "chaînes=0",
        "clés=0",
        "clés absentes",
        "entrées max",
        "chaînes max",
        "clés max"
    );
    for (v, b) in groupes.iter().enumerate() {
        println!(
            "{:>7} {:>8} {:>14} {:>14} {:>12} {:>12} {:>11} {:>10} {:>10}",
            format!("0x{v:02X}"),
            b.n,
            b.entrees_nulles,
            b.chaines_nulles,
            b.cles_nulles,
            b.cles_absentes,
            b.entrees_max,
            b.chaines_max,
            b.cles_max
        );
    }

    // Une colonne sépare les deux groupes si elle est totale d'un côté et nulle de l'autre.
    let separe = |extrait: fn(&Bilan) -> usize, nom: &str| {
        let (a, b) = (&groupes[0], &groupes[1]);
        let ta = a.n > 0 && extrait(a) == a.n;
        let tb = b.n > 0 && extrait(b) == b.n;
        if (ta && extrait(b) == 0) || (tb && extrait(a) == 0) {
            println!("  → « {nom} » sépare exactement les deux groupes.");
        }
    };
    separe(|b| b.entrees_nulles, "aucune entrée");
    separe(|b| b.chaines_nulles, "aucune chaîne");
    separe(|b| b.cles_nulles, "aucune clé");
    separe(|b| b.cles_absentes, "table de clés absente");

    // Aucun compteur de l'en-tête ne sépare les deux groupes — mesuré : `entrées=0` vaut 36/12 974
    // d'un côté et 2/57 824 de l'autre, `chaînes=0` 721 contre 2 709, et tous les maxima se
    // chevauchent. Le drapeau ne décrit donc pas la forme du fichier. Restait la piste que les
    // deux exemples désignaient : un `EventMap` d'un côté, un fichier de `/text/` de l'autre —
    // c'est-à-dire l'emplacement, donc la nature du contenu. On ventile par premier dossier.
    let mut par_dossier: BTreeMap<String, [usize; 2]> = BTreeMap::new();
    for chemin in chemins {
        let Ok(octets) = vfs.read(chemin) else {
            continue;
        };
        if nie_formats::cfgbin::is_rdbn(&octets) || octets.len() < PIED {
            continue;
        }
        if nie_formats::cfgbin::parse_t2b(&octets).is_err() {
            continue;
        }
        let index = match octets[octets.len() - PIED + 6] {
            0x00 => 0usize,
            0x01 => 1usize,
            _ => continue,
        };
        // Deux premiers segments après `data/` : assez pour distinguer `common/text` de
        // `common/event`, sans éclater en un dossier par épisode.
        let racine = chemin
            .strip_prefix("data/")
            .unwrap_or(chemin)
            .split('/')
            .take(2)
            .collect::<Vec<_>>()
            .join("/");
        par_dossier.entry(racine).or_default()[index] += 1;
    }

    println!("\nLe même octet, ventilé par emplacement :");
    println!(
        "{:<32} {:>10} {:>10}  {}",
        "dossier", "0x00", "0x01", "verdict"
    );
    let mut lignes: Vec<(&String, &[usize; 2])> = par_dossier.iter().collect();
    lignes.sort_by_key(|(_, c)| core::cmp::Reverse(c[0] + c[1]));
    let mut homogenes = 0usize;
    for (dossier, c) in lignes.iter().take(20) {
        let verdict = match (c[0], c[1]) {
            (0, _) | (_, 0) => "homogène",
            _ => "mélangé",
        };
        if verdict == "homogène" {
            homogenes += 1;
        }
        println!("{:<32} {:>10} {:>10}  {}", dossier, c[0], c[1], verdict);
    }
    let total_dossiers = par_dossier.len();
    let tous_homogenes = par_dossier
        .values()
        .filter(|c| c[0] == 0 || c[1] == 0)
        .count();
    println!(
        "\n{tous_homogenes}/{total_dossiers} dossiers sont homogènes ({homogenes} parmi les 20 \
         affichés). Un dossier mélangé suffit à réfuter « l'octet suit l'emplacement »."
    );

    for (v, b) in groupes.iter().enumerate() {
        if let Some(chemin) = &b.exemple {
            println!("  exemple 0x{v:02X} : {chemin}");
        }
    }
}
