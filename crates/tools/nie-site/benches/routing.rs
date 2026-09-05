//! Bench de routage : ce que coûte une requête servie **entièrement en mémoire**.
//!
//! On mesure le chemin qui doit rester sous les 50 ms de TTFB visés à J5 (`/api/v1/textures`
//! partait de 392 ms en production, servi par le wiki) : construction du routeur, résolution du
//! chemin, extraction, sérialisation. Aucun disque, aucun réseau, aucun amont — un bench qui
//! mesurerait le VFS mesurerait le disque de la machine, pas le serveur.

use std::hint::black_box;

use axum::body::Body;
use axum::http::Request;
use criterion::{Criterion, criterion_group, criterion_main};
use nie_site::config::Config;
use nie_site::etat::EtatSite;
use nie_site::index_vfs::IndexVfs;
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

fn requete(runtime: &tokio::runtime::Runtime, etat: &EtatSite, chemin: &str) -> u16 {
    let app = nie_site::routeur(etat.clone());
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
            b.iter(|| black_box(requete(&runtime, &etat, chemin)));
        });
    }
    groupe.finish();

    c.bench_function("construction_index_20k", |b| {
        b.iter(|| black_box(index(20_000).len()));
    });
}

criterion_group!(benches, bench_routage);
criterion_main!(benches);
