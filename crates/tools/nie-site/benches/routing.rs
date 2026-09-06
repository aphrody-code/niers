//! Bench de routage : ce que coûte une requête servie **entièrement en mémoire**.
//!
//! On mesure le chemin qui doit rester sous les 50 ms de TTFB visés à J5 (`/api/v1/textures`
//! partait de 392 ms en production, servi par le wiki) : résolution du chemin, extraction,
//! traversée des couches, sérialisation. Aucun disque, aucun réseau, aucun amont — un bench qui
//! mesurerait le VFS mesurerait le disque de la machine, pas le serveur.
//!
//! ## Ce que ce bench a longtemps mesuré par erreur
//!
//! Jusqu'au 2026-09-05, `requete()` appelait `routeur(etat.clone())` **à l'intérieur** de la
//! boucle mesurée : chaque itération reconstruisait les 19 routes et toutes leurs couches. Les
//! chiffres publiés mélangeaient donc une opération de **démarrage** (une fois dans la vie du
//! processus) avec le coût d'une requête (des millions de fois). L'ajout de deux couches
//! — ETag et borne de débit — l'a rendu manifeste : le bench a annoncé « +340 % » sur
//! `/healthz`, alors que le coût par requête, lui, n'avait quasiment pas bougé.
//!
//! Le routeur est donc construit **une fois** et cloné par itération, et sa construction a son
//! propre bench (`construction_routeur`) — ce qui rend le coût des couches visible là où il est
//! réellement payé, au lieu de le diluer dans sept mesures qui prétendent parler d'autre chose.

use std::hint::black_box;

use axum::body::Body;
use axum::http::Request;
use criterion::{Criterion, criterion_group, criterion_main};
use nie_site::config::Config;
use nie_site::state::EtatSite;
use nie_site::vfs_index::IndexVfs;
use tower::ServiceExt as _;

/// Index synthétique de taille réaliste : 20 000 chemins, dont un quart de textures.
fn index(n: usize) -> IndexVfs {
    let mut entrees = Vec::with_capacity(n);
    for i in 0..n {
        let ext = match i % 4 {
            0 => "g4tx",
            1 => "g4md",
            2 => "acb",
            _ => "cfg.bin",
        };
        entrees.push((format!("data/dx11/lot{:03}/objet{i:06}.{ext}", i % 128), (i % 4096) as u32));
    }
    IndexVfs::depuis(entrees)
}

fn etat() -> EtatSite {
    let config = Config {
        db: "/nonexistent/mirror.sqlite".into(),
        statique: "/nonexistent/dist".into(),
        ..Config::default()
    };
    EtatSite::pour_tests(config, index(20_000))
}

/// Joue une requête sur un routeur DÉJÀ construit.
///
/// `oneshot` consomme le service : on lui passe un clone. Un `Router` cloné partage ses tables
/// derrière des `Arc` — c'est ce qui rend la mesure représentative de ce que fait le serveur,
/// qui garde un routeur unique pour toute sa vie.
fn requete(runtime: &tokio::runtime::Runtime, routeur: &axum::Router, chemin: &str) -> u16 {
    let app = routeur.clone();
    runtime.block_on(async move {
        app.oneshot(Request::builder().uri(chemin).body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
            .as_u16()
    })
}

fn bench_routage(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let etat = etat();
    let routeur = nie_site::routeur(etat.clone());

    let mut groupe = c.benchmark_group("routage");
    for (nom, chemin) in [
        ("healthz", "/healthz"),
        ("api_v1_health", "/api/v1/health"),
        ("api_v1_textures", "/api/v1/textures?page=1&per_page=50"),
        ("api_v1_textures_page_100", "/api/v1/textures?page=100&per_page=50"),
        ("parcours", "/b/data/dx11/lot001"),
        ("robots", "/robots.txt"),
        ("sitemap", "/sitemap.xml"),
    ] {
        groupe.bench_function(nom, |b| {
            b.iter(|| black_box(requete(&runtime, &routeur, chemin)));
        });
    }
    groupe.finish();

    // Payé UNE FOIS au démarrage, jamais par requête : c'est ici, et nulle part ailleurs, que
    // le coût d'une couche supplémentaire doit se lire.
    c.bench_function("construction_routeur", |b| {
        b.iter(|| black_box(nie_site::routeur(etat.clone())));
    });

    c.bench_function("construction_index_20k", |b| {
        b.iter(|| black_box(index(20_000).len()));
    });

    bench_couches(c, &runtime);
}

/// Ce que chaque couche ajoutée coûte **par requête**, isolé.
///
/// Mesuré sur un routeur d'une seule route JSON, précisément pour que le coût de la couche ne
/// se noie pas dans celui de la route. Comparer `sans` aux trois autres donne l'attribution
/// exacte — la seule façon honnête de répondre à « combien coûte l'ETag ? », qu'une mesure
/// globale ne peut pas donner.
fn bench_couches(c: &mut Criterion, runtime: &tokio::runtime::Runtime) {
    /// Débit assez haut pour que la borne ne se déclenche jamais : on mesure ici le chemin
    /// passant, pas le refus (qui, lui, n'arrive qu'à un client déjà hors des clous).
    fn etat_sans_borne(par_seconde: f64) -> EtatSite {
        let config = Config {
            db: "/nonexistent/mirror.sqlite".into(),
            statique: "/nonexistent/dist".into(),
            debit: nie_site::debit::Reglage { par_seconde, rafale: 1e9 },
            ..Config::default()
        };
        EtatSite::pour_tests(config, IndexVfs::depuis(Vec::new()))
    }

    let route = || {
        axum::Router::new().route(
            "/j",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"a": 1, "b": 2})) }),
        )
    };
    let avec_debit = |etat: EtatSite| {
        route()
            .layer(axum::middleware::from_fn_with_state(etat.clone(), nie_site::debit::limiter))
            .with_state(etat)
    };

    let variantes: [(&str, axum::Router); 4] = [
        ("sans", route().with_state(etat_sans_borne(0.0))),
        (
            "etag",
            route()
                .layer(axum::middleware::from_fn(nie_site::etag::conditionnel))
                .with_state(etat_sans_borne(0.0)),
        ),
        ("debit", avec_debit(etat_sans_borne(1e9))),
        (
            "etag_et_debit",
            {
                let etat = etat_sans_borne(1e9);
                route()
                    .layer(axum::middleware::from_fn(nie_site::etag::conditionnel))
                    .layer(axum::middleware::from_fn_with_state(
                        etat.clone(),
                        nie_site::debit::limiter,
                    ))
                    .with_state(etat)
            },
        ),
    ];

    let mut groupe = c.benchmark_group("couches");
    for (nom, app) in variantes {
        groupe.bench_function(nom, |b| {
            b.iter(|| {
                let app = app.clone();
                black_box(runtime.block_on(async move {
                    app.oneshot(
                        Request::builder()
                            .uri("/j")
                            // La borne se déclenche sur cet en-tête et sur lui seul : sans
                            // lui, la couche passerait sans consulter le moindre seau et la
                            // mesure ne dirait rien de son coût réel.
                            .header("x-real-ip", "198.51.100.4")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap()
                    .status()
                    .as_u16()
                }));
            });
        });
    }
    groupe.finish();
}

criterion_group!(benches, bench_routage);
criterion_main!(benches);
