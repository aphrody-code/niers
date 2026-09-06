//! Fidélité de l'aller-retour `décoder → réencoder` d'un `.cfg.bin`, à l'octet près.
//!
//! Deux usages, selon qu'on tient un fichier ou qu'on cherche une cause.
//!
//! **Un fichier** (`t2b_roundtrip <fichier.cfg.bin>… [--out <f>]`) — sert à distinguer, dans un
//! mod, ce qui vient d'une édition voulue de ce qui vient de l'encodeur : si le réencodage à
//! vide perd déjà des octets, un fichier de mod plus court que son vanilla n'a pas forcément été
//! « allégé » à dessein, il a simplement traversé l'encodeur. Avec `--out`, écrit le réencodé :
//! le comparer au fichier du mod isole l'édition réelle du bruit de l'encodeur.
//!
//! **Tout le corpus** (`t2b_roundtrip` sans argument, ou `--vfs <motif>`) — balaie les `.cfg.bin`
//! du VFS, T2B **et** RDBN, et ventile par famille. Sert quand la question n'est plus « ce
//! fichier est-il fidèle ? » mais « qu'est-ce qui, dans l'encodeur, ne l'est pas ? ».
//!
//! Ce que le mode corpus a déjà montré : sur les 152 `.cfg.bin` dont le nom porte `chara_`,
//! **0 se réencode à l'octet près**, et le réencodage **rogne** — 65 398 octets perdus sur 142
//! fichiers. Le plus petit cas divergent, `chara_cloth_change_1.00.29.cfg.bin`, fait 48 octets,
//! en perd exactement 16, et diverge à l'offset 32 : ses 32 premiers octets sont déjà rendus à
//! l'identique, seuls les 16 derniers manquent — le pied de page relevé par
//! `cargo run -p nie-formats --example cfgbin_pied`.
//!
//! Attention au faux vert : les tests d'aller-retour de `cfgbin.rs` passent (498/498 T2B, 16/16
//! RDBN) parce qu'ils comparent l'arbre relu par NOTRE décodeur, lequel n'ouvre jamais ce pied.
//! Un arbre qui survit ne dit rien de ce que le jeu accepte.
//!
//! ```text
//! cargo run -p nie-formats --example t2b_roundtrip -- fichier.cfg.bin --out re.bin
//! cargo run -p nie-formats --example t2b_roundtrip --release
//! cargo run -p nie-formats --example t2b_roundtrip --release -- --vfs chara_
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use nie_formats::cfgbin::{
    cfgbin_parse, encode_rdbn, encode_t2b, is_rdbn, parse, parse_t2b, read_values,
};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    if let Some(i) = args.iter().position(|a| a == "--vfs") {
        let motif = args.get(i + 1).cloned();
        corpus(motif.as_deref());
        return;
    }
    if args.is_empty() {
        corpus(None);
        return;
    }

    let mut out_path = None;
    if let Some(i) = args.iter().position(|a| a == "--out") {
        out_path = args.get(i + 1).cloned();
        args.drain(i..=i + 1);
    }
    assert!(
        !args.is_empty(),
        "usage: t2b_roundtrip <fichier.cfg.bin>… [--out <f>] | [--vfs <motif>]"
    );
    for path in &args {
        let brut = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                println!("{path} : illisible ({e})");
                continue;
            }
        };
        match cfgbin_parse(&brut) {
            Err(e) => println!("{path} : parse impossible ({e:?})"),
            Ok(cfg) => {
                let re = encode_t2b(&cfg.entries);
                let delta = re.len() as i64 - brut.len() as i64;
                let diff = re.iter().zip(brut.iter()).filter(|(a, b)| a != b).count();
                println!(
                    "{}\n  {} o → {} o  (Δ {delta:+})  {}  {diff} octets différents",
                    path,
                    brut.len(),
                    re.len(),
                    if re == brut { "FIDÈLE" } else { "INFIDÈLE" }
                );
                if re != brut {
                    println!(
                        "  premier écart à l'offset {0} (0x{0:X})",
                        premier_ecart(&brut, &re)
                    );
                }
                if let Some(o) = &out_path {
                    std::fs::write(o, &re).expect("écriture du réencodé");
                    println!("  réencodé écrit dans {o}");
                }
            }
        }
    }
}

/// Verdict d'un fichier du corpus.
enum Verdict {
    /// Octet pour octet, le même fichier.
    Identique,
    /// Même longueur, contenu différent — un agencement interne diffère.
    MemeTailleContenuDifferent { premier_offset: usize },
    /// Longueur différente : `delta` = réencodé − original.
    TailleDifferente { delta: isize, premier_offset: usize },
    /// Le réencodeur a refusé.
    EchecEncodage,
    /// Le décodeur a refusé — hors périmètre, le fichier n'est pas lu au départ.
    EchecDecodage,
}

#[derive(Default)]
struct Compte {
    identique: usize,
    meme_taille: usize,
    taille_differente: usize,
    echec_encodage: usize,
    echec_decodage: usize,
    /// Somme des écarts de taille, pour dire si le réencodage rogne ou gonfle.
    delta_total: isize,
    /// Le plus petit fichier divergent : le cas le moins coûteux à ouvrir à la main.
    plus_petit_divergent: Option<(String, usize, isize, usize)>,
}

impl Compte {
    fn total(&self) -> usize {
        self.identique
            + self.meme_taille
            + self.taille_differente
            + self.echec_encodage
            + self.echec_decodage
    }
}

/// Premier offset où les deux tampons diffèrent, `min(len)` si l'un est un préfixe de l'autre.
fn premier_ecart(a: &[u8], b: &[u8]) -> usize {
    a.iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()))
}

/// Réencode par la voie qui convient au conteneur, puis compare aux octets d'origine.
fn juger(octets: &[u8]) -> Verdict {
    let reencode = if is_rdbn(octets) {
        let Ok(rdbn) = parse(octets) else {
            return Verdict::EchecDecodage;
        };
        let valeurs = read_values(&rdbn, octets);
        match encode_rdbn(&valeurs) {
            Ok(v) => v,
            Err(_) => return Verdict::EchecEncodage,
        }
    } else {
        let Ok(arbre) = parse_t2b(octets) else {
            return Verdict::EchecDecodage;
        };
        encode_t2b(&arbre.entries)
    };

    if reencode == octets {
        return Verdict::Identique;
    }
    let offset = premier_ecart(octets, &reencode);
    let delta = reencode.len() as isize - octets.len() as isize;
    if delta == 0 {
        Verdict::MemeTailleContenuDifferent {
            premier_offset: offset,
        }
    } else {
        Verdict::TailleDifferente {
            delta,
            premier_offset: offset,
        }
    }
}

/// Famille d'un chemin : le dossier qui suit `gamedata/`, sinon le dossier parent.
fn famille(chemin: &str) -> String {
    if let Some(reste) = chemin.split("/gamedata/").nth(1) {
        if let Some((dossier, _)) = reste.split_once('/') {
            return format!("gamedata/{dossier}");
        }
        return "gamedata".to_string();
    }
    chemin
        .rsplit_once('/')
        .map_or_else(|| "?".to_string(), |(d, _)| d.to_string())
}

/// Retient le plus petit fichier divergent — le moins cher à ouvrir dans un éditeur hexa.
fn noter_divergent(compte: &mut Compte, chemin: &str, taille: usize, delta: isize, offset: usize) {
    if compte
        .plus_petit_divergent
        .as_ref()
        .is_none_or(|(_, t, _, _)| taille < *t)
    {
        compte.plus_petit_divergent = Some((chemin.to_string(), taille, delta, offset));
    }
}

/// Balaie les `.cfg.bin` du VFS et ventile le verdict par famille.
fn corpus(motif: Option<&str>) {
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
        .filter(|p| motif.is_none_or(|m| p.contains(m)))
        .collect();

    match motif {
        Some(m) => println!("{} fichiers .cfg.bin contenant « {m} »\n", chemins.len()),
        None => println!("{} fichiers .cfg.bin\n", chemins.len()),
    }
    if chemins.is_empty() {
        return;
    }

    let mut par_famille: BTreeMap<String, Compte> = BTreeMap::new();
    let mut global = Compte::default();

    for chemin in &chemins {
        let Ok(octets) = vfs.read(chemin) else {
            continue;
        };
        let verdict = juger(&octets);
        let c = par_famille.entry(famille(chemin)).or_default();

        for compte in [&mut *c, &mut global] {
            match verdict {
                Verdict::Identique => compte.identique += 1,
                Verdict::MemeTailleContenuDifferent { premier_offset } => {
                    compte.meme_taille += 1;
                    noter_divergent(compte, chemin, octets.len(), 0, premier_offset);
                }
                Verdict::TailleDifferente {
                    delta,
                    premier_offset,
                } => {
                    compte.taille_differente += 1;
                    compte.delta_total += delta;
                    noter_divergent(compte, chemin, octets.len(), delta, premier_offset);
                }
                Verdict::EchecEncodage => compte.echec_encodage += 1,
                Verdict::EchecDecodage => compte.echec_decodage += 1,
            }
        }
    }

    println!(
        "{:<34} {:>7} {:>9} {:>10} {:>10} {:>8} {:>8}",
        "famille", "total", "identique", "=taille≠", "≠taille", "enc.KO", "déc.KO"
    );
    for (nom, c) in &par_famille {
        println!(
            "{:<34} {:>7} {:>9} {:>10} {:>10} {:>8} {:>8}",
            nom,
            c.total(),
            c.identique,
            c.meme_taille,
            c.taille_differente,
            c.echec_encodage,
            c.echec_decodage
        );
    }

    let total = global.total();
    let part = if total == 0 {
        0.0
    } else {
        global.identique as f64 * 100.0 / total as f64
    };
    println!(
        "\nTOTAL {total} — octet-identiques {} ({part:.3} %), même taille mais différents {}, \
         taille différente {}, encodage refusé {}, décodage refusé {}",
        global.identique,
        global.meme_taille,
        global.taille_differente,
        global.echec_encodage,
        global.echec_decodage
    );
    if global.taille_differente > 0 {
        println!(
            "Écart de taille cumulé : {:+} octet(s) sur {} fichier(s) — {}",
            global.delta_total,
            global.taille_differente,
            if global.delta_total < 0 {
                "le réencodage ROGNE"
            } else {
                "le réencodage GONFLE"
            }
        );
    }
    if let Some((chemin, taille, delta, offset)) = &global.plus_petit_divergent {
        println!(
            "\nPlus petit cas divergent — à ouvrir en premier :\n  {chemin}\n  \
             {taille} octets, delta {delta:+}, premier écart à l'offset {offset} (0x{offset:X})"
        );
    }
    if global.identique == total {
        println!("\nTous les fichiers se réencodent à l'octet près.");
    }
}
