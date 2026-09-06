//! Porte des champs numériques d'un RDBN à une valeur cible, **en patchant les octets en place**.
//!
//! Les valeurs RDBN sont à taille fixe (`Flag`/`Int` = 4 octets, `Short` = 2, `Byte` = 1) : porter
//! `261` à `999` ne déplace donc rien. On évite ainsi le réencodage, dont l'aller-retour n'est pas
//! fidèle sur ces formats (mesuré : −141 octets sur `chara_param`).
//!
//! Repérage des lignes : plutôt que de recalculer la disposition binaire des champs, on décode la
//! liste, on reconstruit la **signature d'octets** des valeurs consécutives à modifier, et on la
//! remplace. Une signature de 14 entiers consécutifs est unique en pratique ; le programme refuse
//! d'écrire si elle apparaît plusieurs fois.
//!
//! Usage :
//!   `rdbn_max <in> <out> --list <liste> --where <champ>=<val>[,<champ>=<val>] \`
//!   `          --set <champ1,champ2,…> --to <valeur>`

use std::collections::HashMap;

use nie_formats::cfgbin::{RdbnList, RdbnValue};

fn opt(args: &mut Vec<String>, nom: &str) -> Option<String> {
    args.iter().position(|a| a == nom).map(|i| {
        let v = args.get(i + 1).cloned().unwrap_or_default();
        args.drain(i..=i + 1);
        v
    })
}

/// Valeur entière d'un champ, quel que soit son type numérique.
fn num(v: &RdbnValue) -> Option<i64> {
    match v {
        RdbnValue::Byte(b) => Some(i64::from(*b)),
        RdbnValue::Short(s) | RdbnValue::ActType(s) => Some(i64::from(*s)),
        RdbnValue::Int(i) | RdbnValue::Flag(i) => Some(i64::from(*i)),
        RdbnValue::Bool(b) => Some(i64::from(*b)),
        _ => None,
    }
}

/// Encodage little-endian d'une valeur, dans la largeur de son type d'origine.
fn octets(v: &RdbnValue, n: i64) -> Option<Vec<u8>> {
    match v {
        RdbnValue::Byte(_) | RdbnValue::Bool(_) => u8::try_from(n).ok().map(|x| vec![x]),
        RdbnValue::Short(_) | RdbnValue::ActType(_) => {
            i16::try_from(n).ok().map(|x| x.to_le_bytes().to_vec())
        }
        RdbnValue::Int(_) | RdbnValue::Flag(_) => {
            i32::try_from(n).ok().map(|x| x.to_le_bytes().to_vec())
        }
        _ => None,
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let liste = opt(&mut args, "--list").expect("--list <nom>");
    let filtre = opt(&mut args, "--where").unwrap_or_default();
    let set = opt(&mut args, "--set").expect("--set <champs séparés par des virgules>");
    let to: i64 = opt(&mut args, "--to")
        .expect("--to <valeur>")
        .parse()
        .expect("valeur entière");
    let src = args.first().expect("usage: rdbn_max <in> <out> …").clone();
    let dst = args.get(1).expect("usage: rdbn_max <in> <out> …").clone();

    let conds: HashMap<String, i64> = filtre
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|kv| {
            let (k, v) = kv.split_once('=').expect("filtre : champ=valeur");
            (k.to_string(), v.parse().expect("valeur de filtre entière"))
        })
        .collect();
    let champs: Vec<&str> = set.split(',').filter(|s| !s.is_empty()).collect();

    let data = std::fs::read(&src).expect("lecture");
    let rdbn = nie_formats::cfgbin::parse(&data).expect("pas un RDBN");
    let lists = nie_formats::cfgbin::read_values(&rdbn, &data);
    let l: &RdbnList = lists
        .iter()
        .find(|l| l.name == liste)
        .expect("liste introuvable");
    println!("{src}\n  liste « {} » — {} ligne(s)", l.name, l.rows.len());

    let mut out = data.clone();
    let (mut touchees, mut ecrits) = (0usize, 0usize);
    for (i, row) in l.rows.iter().enumerate() {
        let idx: HashMap<&str, &RdbnValue> =
            row.fields.iter().map(|(k, v)| (k.as_str(), v)).collect();
        // La ligne doit satisfaire tout le filtre.
        if !conds
            .iter()
            .all(|(k, want)| idx.get(k.as_str()).and_then(|v| num(v)) == Some(*want))
        {
            continue;
        }
        // Signature : les champs visés, consécutifs, dans leur encodage d'origine.
        let mut sig = Vec::new();
        let mut rempl = Vec::new();
        let mut ok = true;
        for c in &champs {
            let Some(v) = idx.get(c) else {
                ok = false;
                break;
            };
            let (Some(cur), Some(neuf)) = (num(v).and_then(|n| octets(v, n)), octets(v, to)) else {
                ok = false;
                break;
            };
            sig.extend_from_slice(&cur);
            rempl.extend_from_slice(&neuf);
        }
        if !ok || sig.is_empty() {
            println!("  row{i} : champ(s) absent(s) ou non numérique(s) — ignorée");
            continue;
        }
        let occurrences = out
            .windows(sig.len())
            .filter(|w| *w == sig.as_slice())
            .count();
        if occurrences != 1 {
            println!("  row{i} : signature vue {occurrences}× — ambiguë, non écrite");
            continue;
        }
        let pos = out
            .windows(sig.len())
            .position(|w| w == sig.as_slice())
            .expect("déjà compté");
        out[pos..pos + rempl.len()].copy_from_slice(&rempl);
        touchees += 1;
        ecrits += rempl.len();
        println!("  row{i} : maxée à 0x{pos:X} ({} octets)", rempl.len());
    }

    assert_eq!(out.len(), data.len(), "la taille a changé — refus d'écrire");
    // Relecture de contrôle : le fichier doit rester un RDBN lisible.
    let r2 = nie_formats::cfgbin::parse(&out).expect("le fichier produit doit se reparser");
    let l2 = nie_formats::cfgbin::read_values(&r2, &out);
    assert_eq!(l2.len(), lists.len(), "nombre de listes modifié");
    std::fs::write(&dst, &out).expect("écriture");
    println!(
        "\n{touchees} ligne(s) maxée(s), {ecrits} octets écrits, taille inchangée ({} o)",
        out.len()
    );
    println!("écrit {dst}");
}
