//! Robustesse CORPUS des parseurs de format (pilier A — « 100 % des fichiers lisibles »).
//!
//! Parse l'INTÉGRALITÉ des `.objbin` (12090) et `.g4pkm` (6927) du VFS IEVR réel et vérifie :
//! - **0 panic** (le harnais de test fait échouer tout panic) ;
//! - un **taux de succès élevé** (plancher de non-régression) — capte les bugs de parseur sur les
//!   fichiers à cas-limite que les tests-échantillons ratent.
//!
//! Gated sur la présence du vrai jeu (`NIE_GAME_DIR` ou chemin Steam par défaut) ; skip sinon.
//! Complète la validation `menu_setting_corpus_*` (menu_render_gate.rs) côté formats binaires.

use std::path::PathBuf;

fn game_dir() -> Option<PathBuf> {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let p = PathBuf::from(dir);
    nie_formats::vfs::donnees_disponibles(p.join("data")).then_some(p)
}

/// Parse tous les fichiers du VFS dont le basename finit par `ext`, via `parse`, et renvoie
/// `(ok, total)`. Déterministe (chemins triés). Tout panic du parseur fait échouer le test.
fn parse_all<T>(
    vfs: &nie_formats::vfs::Vfs,
    ext: &str,
    parse: impl Fn(&[u8]) -> Option<T>,
) -> (usize, usize) {
    let mut paths: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.ends_with(ext))
        .collect();
    paths.sort_unstable();
    let total = paths.len();
    let mut ok = 0usize;
    for p in &paths {
        if let Ok(bytes) = vfs.read(p)
            && parse(&bytes).is_some()
        {
            ok += 1;
        }
    }
    (ok, total)
}

#[test]
fn objbin_and_g4pkm_parsers_robust_on_full_corpus() {
    use nie_formats::{g4pkm, objbin, vfs::Vfs};
    let Some(game) = game_dir() else {
        eprintln!("skip format_corpus : jeu absent");
        return;
    };
    let mut vfs = Vfs::new();
    vfs.init(game.join("data").as_path()).expect("vfs init");

    let (obj_ok, obj_total) = parse_all(&vfs, ".objbin", |b| objbin::parse(b).ok());
    let (pkm_ok, pkm_total) = parse_all(&vfs, ".g4pkm", |b| g4pkm::parse(b).ok());

    let obj_rate = obj_ok as f64 / obj_total.max(1) as f64;
    let pkm_rate = pkm_ok as f64 / pkm_total.max(1) as f64;
    eprintln!(
        "\n=== corpus formats : objbin {obj_ok}/{obj_total} ({:.2}%), g4pkm {pkm_ok}/{pkm_total} ({:.2}%) ===\n",
        obj_rate * 100.0,
        pkm_rate * 100.0
    );

    // Le corpus doit être substantiel (sinon le VFS n'est pas monté correctement).
    assert!(
        obj_total >= 10_000,
        "objbin corpus trop petit ({obj_total})"
    );
    assert!(pkm_total >= 5_000, "g4pkm corpus trop petit ({pkm_total})");
    // Plancher de non-régression : la quasi-totalité des fichiers réels doit parser. À RELEVER si
    // le taux observé est plus haut (mesuré 2026-06-16).
    assert!(
        obj_rate >= 0.99,
        "taux de parse objbin {:.4} < 0.99",
        obj_rate
    );
    assert!(
        pkm_rate >= 0.99,
        "taux de parse g4pkm {:.4} < 0.99",
        pkm_rate
    );
}

/// Robustesse du parseur **cfg.bin** (cœur du format de données) : parse TOUS les `gamedata/*.cfg.bin`
/// via `parse_t2b` sous `catch_unwind` et compte les **panics** (le harnais nie-game enveloppe déjà
/// `parse_t2b` dans `catch_unwind` au montage VFS — ce test QUANTIFIE le besoin). Un parseur robuste
/// = **0 panic** : les fichiers RDBN ou chiffrés doivent renvoyer `Err` proprement, jamais paniquer.
#[test]
fn cfgbin_parser_never_panics_on_gamedata_corpus() {
    use nie_formats::{cfgbin, vfs::Vfs};
    let Some(game) = game_dir() else {
        eprintln!("skip cfgbin_corpus : jeu absent");
        return;
    };
    let mut vfs = Vfs::new();
    vfs.init(game.join("data").as_path()).expect("vfs init");

    let mut paths: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.contains("/gamedata/") && p.ends_with(".cfg.bin"))
        .collect();
    paths.sort_unstable();
    let total = paths.len();
    assert!(
        total >= 1000,
        "corpus gamedata cfg.bin trop petit ({total})"
    );

    // Silence le hook de panic pendant la mesure (on COMPTE les panics, on ne veut pas le bruit).
    // Route par magic : RDBN → `cfgbin::parse`, sinon → `parse_t2b`. Les DEUX chemins du format
    // cœur sont validés sans panic, avec un taux de succès élevé par format.
    let prev = std::panic::take_hook();
    std::panic::set_hook(std::boxed::Box::new(|_| {}));
    let (mut t2b_ok, mut t2b_n) = (0usize, 0usize);
    let (mut rdbn_ok, mut rdbn_n) = (0usize, 0usize);
    let mut panics = 0usize;
    let mut panic_paths: Vec<String> = Vec::new();
    for p in &paths {
        let Ok(bytes) = vfs.read(p) else { continue };
        let is_rdbn = cfgbin::is_rdbn(&bytes);
        let res = if is_rdbn {
            std::panic::catch_unwind(|| cfgbin::parse(&bytes).is_ok())
        } else {
            std::panic::catch_unwind(|| cfgbin::parse_t2b(&bytes).is_ok())
        };
        match (is_rdbn, res) {
            (false, Ok(ok)) => {
                t2b_n += 1;
                t2b_ok += usize::from(ok);
            }
            (true, Ok(ok)) => {
                rdbn_n += 1;
                rdbn_ok += usize::from(ok);
            }
            (_, Err(_)) => {
                panics += 1;
                if panic_paths.len() < 10 {
                    panic_paths.push(p.clone());
                }
            }
        }
    }
    std::panic::set_hook(prev);

    let t2b_rate = t2b_ok as f64 / t2b_n.max(1) as f64;
    let rdbn_rate = rdbn_ok as f64 / rdbn_n.max(1) as f64;
    eprintln!(
        "\n=== cfgbin corpus : {total} gamedata → T2B {t2b_ok}/{t2b_n} ({:.2}%), RDBN {rdbn_ok}/{rdbn_n} ({:.2}%), {panics} PANICS ===",
        t2b_rate * 100.0,
        rdbn_rate * 100.0
    );
    if panics > 0 {
        eprintln!("    1ers fichiers qui paniquent : {panic_paths:?}");
    }
    eprintln!();
    // Robustesse : AUCUN panic sur l'un OU l'autre chemin du format cœur.
    assert_eq!(
        panics, 0,
        "cfgbin PANIQUE sur {panics} fichiers réels (robustesse insuffisante)"
    );
    // Les deux formats parsent proprement (RDBN = format distinct, doit réussir aussi).
    assert!(rdbn_n >= 100, "trop peu de fichiers RDBN ({rdbn_n})");
    assert!(t2b_rate >= 0.99, "taux T2B {t2b_rate:.4} < 0.99");
    assert!(rdbn_rate >= 0.99, "taux RDBN {rdbn_rate:.4} < 0.99");
}

// NB (g4tx corpus TENTÉ + ÉCARTÉ 2026-06-16) : une validation corpus du parseur `g4tx` (53668
// fichiers) est IMPRATICABLE — ce sont des TEXTURES (binaire lourd, Mo/fichier) : lire même 1/6 du
// corpus = des Go d'I/O VFS (> 100 s, time-out). Contrairement aux formats à parsing complexe
// (cfgbin/objbin/g4pkm, validés ci-dessus), `g4tx` est un conteneur simple à faible risque, déjà
// exercé en continu par le pipeline de rendu (`decode_texture_rgba`, menu_render_gate). La robustesse
// g4tx est donc couverte par le rendu, pas par un test corpus à I/O prohibitif.
