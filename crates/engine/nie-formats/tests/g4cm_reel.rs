//! Le codec G4CM confronté aux **1 215 `.g4cm` réels** du jeu.
//!
//! Le contrat est le ré-encodage **byte-exact** : `encode(decode(x)) == x`. C'est la seule
//! preuve qu'une structure est comprise entièrement — un décodeur qui « lit » un fichier mais
//! ne sait pas le réécrire a laissé des octets qu'il n'explique pas.
//!
//! Le test vérifie en plus que le décodage remonte *quelque chose* (objets, canaux, temps) :
//! un round-trip réussi sur une structure vide passerait sans rien prouver.
//!
//! Il **annonce son saut** quand ni l'installation ni le dump ne sont disponibles.

#![cfg(feature = "std")]

use nie_formats::g4cm;
use nie_formats::vfs::{self, Vfs};

fn corpus() -> Option<Vfs> {
    match vfs::open_game() {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("skip : ni installation ni dump ({e:?})");
            None
        }
    }
}

#[test]
fn g4cm_reels_round_trip_byte_exact() {
    let Some(vfs) = corpus() else { return };
    let mut chemins: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.ends_with(".g4cm"))
        .collect();
    chemins.sort_unstable();
    if chemins.is_empty() {
        eprintln!("skip : aucun .g4cm dans le corpus monté");
        return;
    }

    let (mut exact, mut objets, mut canaux, mut echantillons, mut decodes) =
        (0, 0, 0, 0usize, 0usize);
    let mut echecs: Vec<String> = Vec::new();
    for p in &chemins {
        let Ok(octets) = vfs.read(p) else {
            echecs.push(format!("{p} : lecture impossible"));
            continue;
        };
        let anim = match g4cm::decode(&octets) {
            Ok(a) => a,
            Err(e) => {
                echecs.push(format!("{p} : {e}"));
                continue;
            }
        };
        match g4cm::encode(&anim) {
            Ok(re) if re == octets => exact += 1,
            Ok(re) => {
                let ou = re
                    .iter()
                    .zip(octets.iter())
                    .position(|(a, b)| a != b)
                    .map_or_else(|| "taille".to_string(), |i| format!("0x{i:X}"));
                echecs.push(format!(
                    "{p} : écart à {ou} ({} vs {} o)",
                    re.len(),
                    octets.len()
                ));
            }
            Err(e) => echecs.push(format!("{p} : encode {e}")),
        }
        objets += anim.objects.len();
        canaux += anim.channels.len();
        for c in &anim.channels {
            echantillons += c.track.len();
            if c.track.values().is_some() {
                decodes += c.track.len();
            }
        }
    }

    eprintln!(
        "g4cm : {exact}/{} round-trip byte-exact — {objets} objets, {canaux} canaux, \
         {echantillons} échantillons dont {decodes} en f32",
        chemins.len()
    );
    for e in echecs.iter().take(10) {
        eprintln!("  ÉCHEC {e}");
    }
    assert!(echecs.is_empty(), "{} fichier(s) en échec", echecs.len());
    assert!(
        objets > 0 && canaux > 0 && echantillons > 0,
        "décodage vide : rien n'est prouvé"
    );
}

/// Le dispatch `decode` doit rendre du JSON qui **contient réellement** les données de caméra.
///
/// Sans ce test, un `nie_decode_json` qui ne remonterait que l'en-tête passerait pour un succès :
/// il rend bien du JSON, il est bien non vide.
#[cfg(feature = "serde")]
#[test]
fn decode_json_porte_les_canaux() {
    let Some(vfs) = corpus() else { return };
    let Some(p) = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .find(|p| p.ends_with(".g4cm"))
    else {
        eprintln!("skip : aucun .g4cm dans le corpus monté");
        return;
    };
    let octets = vfs.read(&p).expect("lecture");
    let d = nie_formats::decode::decode(&octets).expect("le .g4cm doit se décoder");
    assert_eq!(d.format, "g4cm");
    let json = String::from_utf8(d.json).expect("json utf-8");
    for cle in [
        "\"channels\"",
        "\"times\"",
        "\"clips\"",
        "\"names\"",
        "\"objects\"",
    ] {
        assert!(
            json.contains(cle),
            "le JSON ne porte pas {cle} — décodage incomplet"
        );
    }
    eprintln!("g4cm → JSON : {} octets pour {p}", json.len());
}
