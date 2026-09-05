//! Tests du routeur — ils **comptent**.
//!
//! Chaque test affirme un nombre : nombre de routes, code HTTP, nombre d'éléments d'une page,
//! nombre d'en-têtes de sécurité, taille de corps. Une suite qui se contente de constater que
//! « ça compile » ne prouve rien ; une suite qui rend `0 passed` est un échec.
//!
//! Aucun test ne dépend du jeu, du miroir de production ni de `nie-model-serve` : l'index VFS
//! est injecté, le miroir est un SQLite temporaire créé par le test, et l'amont est une adresse
//! close. C'est ce qui les rend rejouables sur une machine nue.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt as _;
use nie_site::app::{NB_ENTETES_SECURITE, ROUTES, entetes_securite_liste};
use nie_site::config::{Config, PER_PAGE_MAX};
use nie_site::state::EtatSite;
use nie_site::vfs_index::IndexVfs;
use tower::ServiceExt as _;

/// Les chemins de l'index injecté : 4 textures, 2 modèles, 2 sons, 1 vidéo, 1 sans extension.
const CHEMINS: [(&str, u32); 10] = [
    ("data/dx11/menu/title/a.g4tx", 100),
    ("data/dx11/menu/title/b.g4tx", 200),
    ("data/dx11/menu/sub/c.g4tx", 300),
    ("data/dx11/menu/sub/d.g4tx", 400),
    ("data/common/chr/c01000010/c01000010.g4md", 500),
    ("data/common/chr/c01000010/c01000010.g4mg", 600),
    ("data/common/sound/bgm.acb", 700),
    ("data/common/sound/bgm.awb", 800),
    ("data/common/movie/op.usm", 900),
    ("data/common/misc/LISEZMOI", 10),
];

fn index() -> IndexVfs {
    IndexVfs::depuis(CHEMINS.iter().map(|(c, t)| ((*c).to_owned(), *t)).collect())
}

/// État de test : index injecté, aucun contenu VFS, miroir absent, amont clos.
fn etat() -> EtatSite {
    etat_avec(|_| {})
}

fn config_nue() -> Config {
    Config {
        db: "/nonexistent/mirror.sqlite".into(),
        statique: "/nonexistent/dist".into(),
        // Port 1 : jamais en écoute, la connexion est refusée immédiatement.
        amont: "http://127.0.0.1:1".to_owned(),
        ..Config::default()
    }
}

fn etat_avec(regle: impl FnOnce(&mut Config)) -> EtatSite {
    let mut config = config_nue();
    regle(&mut config);
    EtatSite::pour_tests(config, index())
}

async fn reponse(etat: &EtatSite, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    reponse_avec(etat, Request::builder().uri(uri)).await
}

async fn reponse_avec(
    etat: &EtatSite,
    requete: axum::http::request::Builder,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let app = nie_site::routeur(etat.clone());
    let r = app.oneshot(requete.body(Body::empty()).unwrap()).await.unwrap();
    let statut = r.status();
    let entetes = r.headers().clone();
    let corps = r.into_body().collect().await.unwrap().to_bytes().to_vec();
    (statut, entetes, corps)
}

fn json(corps: &[u8]) -> serde_json::Value {
    serde_json::from_slice(corps).expect("corps JSON")
}

#[tokio::test]
async fn les_quinze_routes_repondent() {
    let etat = etat();
    // Une instance concrète par route déclarée, dans le même ordre que `ROUTES`.
    let instances: [(&str, &[u16]); 15] = [
        ("/healthz", &[200]),
        ("/robots.txt", &[200]),
        ("/.well-known/security.txt", &[200]),
        ("/sitemap.xml", &[200]),
        ("/api/v1/health", &[200]),
        ("/api/v1/chara", &[503]), // miroir absent : capacité dégradée, pas une panne
        ("/llms.txt", &[200]),
        ("/llms-full.txt", &[200]),
        ("/api/v1/textures", &[200]),
        ("/f/data/dx11/menu/title/a.g4tx", &[503]), // index sans contenu
        ("/b", &[200]),
        ("/b/data/dx11/menu", &[200]),
        ("/assets/data/x.g4tx", &[502]), // amont clos
        // Catalogue des episodes : la base n'est pas la dans l'etat de test, et le serveur le
        // DIT plutot que de rendre une liste vide qu'un client prendrait pour un catalogue a
        // jour. C'est la porte de mise a jour des Inacord installes.
        ("/api/v1/episodes", &[503]),
        ("/", &[200]),
    ];
    assert_eq!(ROUTES.len(), 15);
    assert_eq!(instances.len(), ROUTES.len(), "une instance par route declaree");
    let mut vus = 0;
    for (uri, attendus) in instances {
        let (statut, _, _) = reponse(&etat, uri).await;
        assert!(
            attendus.contains(&statut.as_u16()),
            "{uri} a repondu {} au lieu de {attendus:?}",
            statut.as_u16()
        );
        vus += 1;
    }
    assert_eq!(vus, 15);
}

#[tokio::test]
async fn healthz_annonce_le_service_et_sa_version() {
    let (statut, entetes, corps) = reponse(&etat(), "/healthz").await;
    assert_eq!(statut, StatusCode::OK);
    assert_eq!(entetes[header::CONTENT_TYPE], "application/json");
    let v = json(&corps);
    assert_eq!(v["service"], "nie-site");
    assert_eq!(v["version"], nie_site::VERSION);
    assert_eq!(v["etat"], "ok");
    assert_eq!(v["capacites"]["vfs"], "pret");
    assert_eq!(v["capacites"]["vfs_entrees"], 10);
    assert_eq!(v["capacites"]["vfs_contenu"], false);
    assert_eq!(v["capacites"]["gisement"], false);
}

#[tokio::test]
async fn les_cinq_entetes_de_securite_sont_sur_toutes_les_reponses() {
    let etat = etat();
    for uri in ["/healthz", "/", "/api/v1/inconnue", "/robots.txt"] {
        let (_, entetes, _) = reponse(&etat, uri).await;
        for (nom, valeur) in entetes_securite_liste() {
            assert_eq!(entetes.get(&nom).map(|v| v.to_str().unwrap()), Some(valeur), "{uri}: {nom}");
        }
        assert_eq!(entetes_securite_liste().len(), NB_ENTETES_SECURITE);
    }
    let (_, entetes, _) = reponse(&etat, "/healthz").await;
    let csp = entetes[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert_eq!(csp.split(';').count(), 12, "douze directives CSP");
    assert!(csp.contains("frame-ancestors 'none'"));
}

#[tokio::test]
async fn documents_well_known() {
    let etat = etat_avec(|c| c.origine = "https://exemple.test".to_owned());

    let (statut, entetes, corps) = reponse(&etat, "/robots.txt").await;
    assert_eq!(statut, StatusCode::OK);
    assert_eq!(entetes[header::CONTENT_TYPE], "text/plain; charset=utf-8");
    let texte = String::from_utf8(corps).unwrap();
    assert!(texte.contains("Sitemap: https://exemple.test/sitemap.xml"));
    // 9 : 5 pour le regime general, 4 repetes pour celui des agents.
    assert_eq!(texte.matches("Disallow:").count(), 9);
    assert!(texte.contains("User-agent: GPTBot"), "les agents ont leur bloc");
    assert!(texte.contains("Allow: /api/v1/"), "l'API leur est ouverte");

    // Les deux documents destines aux agents repondent, en texte brut, et se citent l'origine
    // configuree — pas `aphrody.com` en dur, sinon une preview oriente vers la production.
    for (uri, debut) in [("/llms.txt", "# Aphrody"), ("/llms-full.txt", "# Aphrody")] {
        let (statut, entetes, corps) = reponse(&etat, uri).await;
        assert_eq!(statut, StatusCode::OK, "{uri}");
        assert_eq!(entetes[header::CONTENT_TYPE], "text/plain; charset=utf-8", "{uri}");
        let doc = String::from_utf8(corps).unwrap();
        assert!(doc.starts_with(debut), "{uri} ne commence pas par son titre");
        assert!(doc.contains("https://exemple.test"), "{uri} : origine");
        assert!(!doc.contains("aphrody.com"), "{uri} : origine codee en dur");
    }

    let (statut, _, corps) = reponse(&etat, "/.well-known/security.txt").await;
    assert_eq!(statut, StatusCode::OK);
    let texte = String::from_utf8(corps).unwrap();
    assert!(texte.contains("Contact:"), "RFC 9116 exige Contact");
    assert!(texte.contains("Expires:"), "RFC 9116 exige Expires");
    assert!(texte.contains("https://exemple.test/.well-known/security.txt"));

    let (statut, entetes, corps) = reponse(&etat, "/sitemap.xml").await;
    assert_eq!(statut, StatusCode::OK);
    assert_eq!(entetes[header::CONTENT_TYPE], "application/xml; charset=utf-8");
    let texte = String::from_utf8(corps).unwrap();
    assert!(texte.starts_with("<?xml"));
    // 5 routes x 3 langues, et chaque entree porte le groupe complet de ses traductions.
    assert_eq!(texte.matches("<url>").count(), 15);
    assert_eq!(texte.matches("<loc>").count(), 15);
    assert_eq!(texte.matches("xhtml:link").count(), 60, "4 alternates par entree");
    assert_eq!(texte.matches(r#"hreflang="x-default""#).count(), 15);
    // Sans la declaration de l'espace de noms, les `xhtml:link` ne sont que du bruit.
    assert!(texte.contains(r#"xmlns:xhtml="http://www.w3.org/1999/xhtml""#));
    for attendu in [
        "<loc>https://exemple.test/textures</loc>",
        "<loc>https://exemple.test/en/textures</loc>",
        "<loc>https://exemple.test/ja/textures</loc>",
    ] {
        assert!(texte.contains(attendu), "{attendu} absent du plan");
    }
}

#[tokio::test]
async fn les_quatre_filtres_enregistres_comptent_leurs_chemins() {
    let etat = etat();
    let attendus = [("textures", 4), ("modeles", 2), ("sons", 2), ("videos", 1)];
    assert_eq!(attendus.len(), 4);
    for (nom, total) in attendus {
        let (statut, _, corps) = reponse(&etat, &format!("/api/v1/{nom}")).await;
        assert_eq!(statut, StatusCode::OK, "{nom}");
        let v = json(&corps);
        assert_eq!(v["total"], total, "{nom}");
        assert_eq!(v["elements"].as_array().unwrap().len(), total, "{nom}");
        // Chaque élément porte son chemin VFS verbatim : c'est son URL sous /f/.
        let premier = &v["elements"][0]["chemin"].as_str().unwrap().to_owned();
        assert!(CHEMINS.iter().any(|(c, _)| c == premier), "{premier} n'est pas un chemin du VFS");
    }

    let (statut, _, corps) = reponse(&etat, "/api/v1/inexistante").await;
    assert_eq!(statut, StatusCode::NOT_FOUND);
    assert_eq!(json(&corps)["genre"], "introuvable");
}

#[tokio::test]
async fn la_pagination_est_bornee() {
    let etat = etat();
    let (_, _, corps) = reponse(&etat, "/api/v1/textures?page=0&per_page=100000").await;
    let v = json(&corps);
    assert_eq!(v["per_page"], PER_PAGE_MAX);
    assert_eq!(v["page"], 1);
    assert_eq!(v["pages"], 1);

    let (_, _, corps) = reponse(&etat, "/api/v1/textures?page=2&per_page=3").await;
    let v = json(&corps);
    assert_eq!(v["elements"].as_array().unwrap().len(), 1, "4 textures, 3 par page");
    assert_eq!(v["pages"], 2);

    let (_, _, corps) = reponse(&etat, "/api/v1/textures?page=99&per_page=3").await;
    assert_eq!(json(&corps)["elements"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn parcours_du_vfs() {
    let etat = etat();
    let (statut, _, corps) = reponse(&etat, "/b").await;
    assert_eq!(statut, StatusCode::OK);
    let v = json(&corps);
    assert_eq!(v["dossiers"].as_array().unwrap().len(), 1, "un seul dossier racine: data");
    assert_eq!(v["dossiers"][0], "data");
    assert_eq!(v["total_fichiers"], 0);

    let (_, _, corps) = reponse(&etat, "/b/data/dx11/menu").await;
    let v = json(&corps);
    assert_eq!(v["prefixe"], "data/dx11/menu");
    assert_eq!(v["dossiers"].as_array().unwrap().len(), 2, "title et sub");
    assert_eq!(v["total_fichiers"], 0);

    let (_, _, corps) = reponse(&etat, "/b/data/dx11/menu/title").await;
    let v = json(&corps);
    assert_eq!(v["total_fichiers"], 2);
    assert_eq!(v["fichiers"][0]["nom"], "a.g4tx");
    assert_eq!(v["fichiers"][0]["taille"], 100);
    assert_eq!(v["fichiers"][0]["chemin"], "data/dx11/menu/title/a.g4tx");

    let (statut, _, _) = reponse(&etat, "/b/data/inexistant").await;
    assert_eq!(statut, StatusCode::OK, "un dossier vide est vide, pas absent");
}

#[tokio::test]
async fn f_conserve_le_chemin_et_refuse_les_sorties() {
    let etat = etat();

    // Chemin indexé, contenu non monté : 503 explicite, jamais un 404 trompeur.
    let (statut, _, corps) = reponse(&etat, "/f/data/dx11/menu/title/a.g4tx").await;
    assert_eq!(statut, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json(&corps)["genre"], "indisponible");

    // Chemin absent de l'index : 404.
    let (statut, _, corps) = reponse(&etat, "/f/data/absent.g4tx").await;
    assert_eq!(statut, StatusCode::NOT_FOUND);
    assert_eq!(json(&corps)["genre"], "introuvable");

    // Sortie d'espace : 400, avant tout accès.
    let (statut, _, corps) = reponse(&etat, "/f/data/../../etc/passwd").await;
    assert_eq!(statut, StatusCode::BAD_REQUEST);
    assert_eq!(json(&corps)["genre"], "demande_invalide");
}

#[tokio::test]
async fn f_sert_les_octets_et_gere_le_304() {
    // Un VFS « dump » minimal : deux fichiers sur disque, montés en direct.
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir_all(data.join("common/misc")).unwrap();
    std::fs::create_dir_all(data.join("dx11")).unwrap();
    let contenu = b"OCTETS DU JEU".to_vec();
    std::fs::write(data.join("common/misc/note.txt"), &contenu).unwrap();
    std::fs::write(data.join("dx11/vide.g4tx"), b"").unwrap();

    let mut vfs = nie_formats::vfs::Vfs::new();
    vfs.init_loose(&data).expect("montage dump");
    let entrees: Vec<(String, u32)> =
        vfs.iter().map(|(c, e)| (c.to_owned(), e.file_size)).collect();
    assert_eq!(entrees.len(), 2, "deux fichiers montes");
    let index = IndexVfs::depuis(entrees);
    // Un dump sert les chemins LOGIQUES du jeu : `data/<relatif>`, comme un montage par packs.
    let note = "data/common/misc/note.txt";
    assert!(index.contient(note), "chemin logique attendu");
    assert_eq!(index.compte_vue(nie_site::vfs_index::Vue::Textures), 1);

    let etat = EtatSite::nouveau(config_nue());
    etat.poser_vfs(Some(std::sync::Arc::new(vfs)), std::sync::Arc::new(index), true);
    let (statut, entetes, corps) = reponse(&etat, &format!("/f/{note}")).await;
    assert_eq!(statut, StatusCode::OK, "chemin servi: {note}");
    assert_eq!(corps, contenu);
    assert_eq!(corps.len(), 13);
    assert_eq!(entetes[header::CONTENT_TYPE], "text/plain; charset=utf-8");
    assert!(entetes[header::CONTENT_DISPOSITION].to_str().unwrap().contains("note.txt"));
    let etag = entetes[header::ETAG].to_str().unwrap().to_owned();
    assert_eq!(etag.len(), 66, "blake3 hexa entre guillemets");

    let (statut, _, corps) = reponse_avec(
        &etat,
        Request::builder().uri(format!("/f/{note}")).header(header::IF_NONE_MATCH, &etag),
    )
    .await;
    assert_eq!(statut, StatusCode::NOT_MODIFIED);
    assert_eq!(corps.len(), 0);
}

#[tokio::test]
async fn chara_lit_le_miroir_et_pagine() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("mirror.sqlite");
    {
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE TABLE inagle_characters (internal_code TEXT, chara_id TEXT, base_slug TEXT, \
             name_fr TEXT, name_en TEXT, name_ja TEXT, element TEXT, position TEXT, rarity TEXT, \
             series TEXT, model_id TEXT, zukan_order INTEGER, colonne_ignoree TEXT);",
        )
        .unwrap();
        for i in 0..7 {
            c.execute(
                "INSERT INTO inagle_characters (internal_code, base_slug, name_fr, zukan_order, colonne_ignoree) \
                 VALUES (?1, ?2, ?3, ?4, 'jamais rendue')",
                rusqlite::params![format!("c0100{i:04}"), "unknown", format!("Perso {i}"), i],
            )
            .unwrap();
        }
    }
    let etat = etat_avec(|c| c.db = db.clone());

    let (statut, _, corps) = reponse(&etat, "/api/v1/chara?per_page=3").await;
    assert_eq!(statut, StatusCode::OK);
    let v = json(&corps);
    assert_eq!(v["total"], 7);
    assert_eq!(v["pages"], 3);
    assert_eq!(v["elements"].as_array().unwrap().len(), 3);
    assert_eq!(v["elements"][0]["internal_code"], "c01000000");
    assert_eq!(v["elements"][0]["base_slug"], "unknown");
    assert!(v["elements"][0].get("colonne_ignoree").is_none(), "jamais SELECT *");

    let (_, _, corps) = reponse(&etat, "/api/v1/chara?page=3&per_page=3").await;
    let v = json(&corps);
    assert_eq!(v["elements"].as_array().unwrap().len(), 1);
    assert_eq!(v["elements"][0]["internal_code"], "c01000006");

    // Le miroir disparaît : 503 avec un message, jamais un 500.
    std::fs::remove_file(&db).unwrap();
    let etat = etat_avec(|c| c.db = db.clone());
    let (statut, _, corps) = reponse(&etat, "/api/v1/chara").await;
    assert_eq!(statut, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json(&corps)["genre"], "indisponible");
}

#[tokio::test]
async fn bundle_statique_precompresse_et_empreinte() {
    let dir = tempfile::tempdir().unwrap();
    let assets = dir.path().join("assets");
    std::fs::create_dir_all(&assets).unwrap();
    let brut = b"const x = 1; // 40 octets de JavaScript ici".to_vec();
    std::fs::write(assets.join("app-1a2b3c4d.js"), &brut).unwrap();
    std::fs::write(assets.join("app-1a2b3c4d.js.br"), b"BROTLI").unwrap();
    std::fs::write(assets.join("app-1a2b3c4d.js.zst"), b"ZSTD!").unwrap();
    std::fs::write(dir.path().join("manifest.webmanifest"), b"{}").unwrap();
    let etat = etat_avec(|c| c.statique = dir.path().to_path_buf());

    // Sans négociation : le fichier tel quel, immuable parce qu'empreinté.
    let (statut, entetes, corps) = reponse(&etat, "/assets/app-1a2b3c4d.js").await;
    assert_eq!(statut, StatusCode::OK);
    assert_eq!(corps, brut);
    assert_eq!(corps.len(), 43);
    assert!(entetes.get(header::CONTENT_ENCODING).is_none());
    assert_eq!(entetes[header::CACHE_CONTROL], "public, max-age=31536000, immutable");
    assert_eq!(entetes[header::VARY], "Accept-Encoding");

    // Brotli annoncé et présent : la variante est servie telle quelle.
    let (statut, entetes, corps) = reponse_avec(
        &etat,
        Request::builder()
            .uri("/assets/app-1a2b3c4d.js")
            .header(header::ACCEPT_ENCODING, "br, zstd, gzip"),
    )
    .await;
    assert_eq!(statut, StatusCode::OK);
    assert_eq!(corps, b"BROTLI");
    assert_eq!(entetes[header::CONTENT_ENCODING], "br");
    assert_eq!(entetes[header::CONTENT_TYPE], "text/javascript; charset=utf-8");

    // zstd seul.
    let (_, entetes, corps) = reponse_avec(
        &etat,
        Request::builder()
            .uri("/assets/app-1a2b3c4d.js")
            .header(header::ACCEPT_ENCODING, "zstd"),
    )
    .await;
    assert_eq!(corps, b"ZSTD!");
    assert_eq!(entetes[header::CONTENT_ENCODING], "zstd");

    // Fichier sans empreinte : revalidation obligatoire.
    let (statut, entetes, _) = reponse(&etat, "/manifest.webmanifest").await;
    assert_eq!(statut, StatusCode::OK);
    assert_eq!(entetes[header::CACHE_CONTROL], "no-cache");
    assert_eq!(entetes[header::CONTENT_TYPE], "application/manifest+json");
}

#[tokio::test]
async fn la_coquille_porte_les_balises_og_de_la_route() {
    let etat = etat();
    let (statut, entetes, corps) = reponse(&etat, "/").await;
    assert_eq!(statut, StatusCode::OK);
    assert_eq!(entetes[header::CONTENT_TYPE], "text/html; charset=utf-8");
    let html = String::from_utf8(corps).unwrap();
    // 8 sans vignette : type, site_name, locale, 2 locale:alternate, title, description, url.
    assert_eq!(html.matches("<meta property=\"og:").count(), 8, "og: sans vignette");
    assert_eq!(html.matches("og:locale:alternate").count(), 2, "les deux autres langues");
    assert!(html.contains("data-route=\"/\""));
    assert!(html.contains("data-langue=\"fr\""));

    let (statut, _, corps) = reponse(&etat, "/textures").await;
    assert_eq!(statut, StatusCode::OK, "route du bundle : la coquille repond");
    let html = String::from_utf8(corps).unwrap();
    assert!(html.contains("<title>Textures — Aphrody</title>"));
    assert!(html.contains("og:url\" content=\"https://aphrody.com/textures\""));
    assert!(html.contains("data-route=\"/textures\""));

    // La meme route dans les deux autres langues : titre traduit, canonical prefixe, et le
    // MEME groupe hreflang des trois cotes — c'est la reciprocite qui rend le groupe valide.
    for (chemin, titre, lang) in [
        ("/en/textures", "Textures — Aphrody", "en"),
        ("/ja/textures", "\u{30c6}\u{30af}\u{30b9}\u{30c1}\u{30e3} — Aphrody", "ja"),
    ] {
        let (statut, _, corps) = reponse(&etat, chemin).await;
        assert_eq!(statut, StatusCode::OK, "{chemin}");
        let html = String::from_utf8(corps).unwrap();
        assert!(html.contains(&format!("<html lang=\"{lang}\">")), "{chemin} : lang");
        assert!(html.contains(&format!("<title>{titre}</title>")), "{chemin} : titre");
        assert!(
            html.contains(&format!("rel=\"canonical\" href=\"https://aphrody.com{chemin}\"")),
            "{chemin} : canonical"
        );
        assert_eq!(html.matches("rel=\"alternate\"").count(), 4, "{chemin} : hreflang");
        assert!(html.contains("hreflang=\"x-default\""), "{chemin} : x-default");
    }

    // `/fr/...` est compris mais renvoye vers la forme canonique, sans prefixe.
    let (statut, entetes, _) = reponse(&etat, "/fr/textures").await;
    // 308 et non 301 : la redirection preserve la methode, et un 301 se grave dans le cache
    // du navigateur de facon quasi irreversible (meme choix qu'`apps/azalee/next.config.ts`).
    assert_eq!(statut, StatusCode::PERMANENT_REDIRECT, "/fr/ n'est pas canonique");
    assert_eq!(entetes[header::LOCATION], "/textures");
}

#[tokio::test]
async fn le_proxy_borne_un_amont_injoignable() {
    let etat = etat();
    let (statut, _, corps) = reponse(&etat, "/assets/data/dx11/menu/title/a.g4tx?format=png").await;
    assert_eq!(statut, StatusCode::BAD_GATEWAY);
    let v = json(&corps);
    assert_eq!(v["genre"], "amont");
    assert!(
        !v["message"].as_str().unwrap().contains("127.0.0.1"),
        "aucune adresse interne ne fuit vers le client"
    );

    // Chemin invalide : refusé avant même de contacter l'amont.
    let (statut, _, corps) = reponse(&etat, "/assets/../secret").await;
    assert_eq!(statut, StatusCode::BAD_REQUEST);
    assert_eq!(json(&corps)["genre"], "demande_invalide");
}

#[tokio::test]
async fn routes_inconnues_repondent_selon_leur_espace() {
    let etat = etat();
    let cas = [
        ("/api/v1/health/trop/loin", 404, true),
        ("/api/inconnue", 404, true),
        ("/f/", 404, true),
        ("/une/route/du/bundle", 200, false),
    ];
    assert_eq!(cas.len(), 4);
    for (uri, code, en_json) in cas {
        let (statut, entetes, _) = reponse(&etat, uri).await;
        assert_eq!(statut.as_u16(), code, "{uri}");
        let type_contenu = entetes[header::CONTENT_TYPE].to_str().unwrap();
        if en_json {
            assert!(type_contenu.starts_with("application/json"), "{uri}: {type_contenu}");
        } else {
            assert!(type_contenu.starts_with("text/html"), "{uri}: {type_contenu}");
        }
    }
}

#[tokio::test]
async fn methodes_non_get_refusees() {
    let app = nie_site::routeur(etat());
    let r = app
        .oneshot(Request::builder().method("POST").uri("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::METHOD_NOT_ALLOWED, "le serveur est en lecture seule");
}

#[tokio::test]
async fn head_repond_comme_get_sans_corps() {
    let app = nie_site::routeur(etat());
    let r = app
        .oneshot(Request::builder().method("HEAD").uri("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let corps = r.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(corps.len(), 0);
}

/// Le mode d'échec le plus cher de cette crate : la coquille est valide, elle répond 200, et
/// elle ne charge pas le bundle. Rien ne le signale — ni le build, ni un test de route, ni un
/// contrôle de taille. C'est arrivé le 2026-09-05 : `apps/nie-web` écrit ses fichiers empreintés
/// dans `dist/static/` (parce que `/assets/` est déjà pris par le proxy vers `nie-model-serve`)
/// et la recherche du point d'entrée ne regardait que `dist/assets/`. Aphrody servait alors une
/// page d'accueil sans une ligne de JavaScript.
///
/// Ce test compte les `<script>` de la coquille pour les trois cas possibles.
#[tokio::test]
async fn coquille_charge_le_bundle() {
    for dossier in nie_site::routes::static_files::DOSSIERS_BUNDLE {
        let dist = tempfile::tempdir().unwrap();
        std::fs::create_dir(dist.path().join(dossier)).unwrap();
        // Le nom est celui que Vite produit REELLEMENT pour `apps/nie-web` (base64url, casses
        // melangees) : un test ecrit sur la forme *attendue* laisse passer le vrai bundle.
        std::fs::write(dist.path().join(dossier).join("index-RXLrxaJS.js"), "export {}").unwrap();
        std::fs::write(dist.path().join(dossier).join("index-RXLrxaJS.css"), ":root{}").unwrap();

        let etat = etat_avec(|c| c.statique = dist.path().to_path_buf());
        let (statut, _, corps) = reponse(&etat, "/").await;
        assert_eq!(statut, StatusCode::OK);
        let html = String::from_utf8(corps).unwrap();

        let script = format!(r#"<script type="module" src="/{dossier}/index-RXLrxaJS.js">"#);
        let feuille = format!(r#"<link rel="stylesheet" href="/{dossier}/index-RXLrxaJS.css">"#);
        assert!(html.contains(&script), "bundle dans {dossier}/ non charge : {html}");
        assert!(html.contains(&feuille), "feuille de {dossier}/ non chargee");
        // Deux `<script>` : les donnees structurees, puis le point d'entree du bundle.
        assert_eq!(html.matches("<script").count(), 2, "json-ld + un seul point d'entree");
        assert_eq!(html.matches("<script type=\"module\"").count(), 1);

        // Et les fichiers annoncés doivent réellement être servis, empreintés donc immuables.
        let (statut, entetes, _) = reponse(&etat, &format!("/{dossier}/index-RXLrxaJS.js")).await;
        assert_eq!(statut, StatusCode::OK, "{dossier}/index-RXLrxaJS.js");
        assert_eq!(
            entetes[header::CACHE_CONTROL].to_str().unwrap(),
            nie_site::routes::static_files::IMMUABLE,
        );
    }

    // Sans bundle, la coquille reste servie — mais sans `<script>`, et elle le dit.
    let etat = etat_avec(|c| c.statique = "/nonexistent/dist".into());
    let (statut, _, corps) = reponse(&etat, "/").await;
    assert_eq!(statut, StatusCode::OK);
    let html = String::from_utf8(corps).unwrap();
    // Seul subsiste le bloc de donnees structurees : aucun module a charger.
    assert_eq!(html.matches("<script type=\"module\"").count(), 0);
    assert_eq!(html.matches("<script").count(), 1, "json-ld seul");
    // Le `<noscript>` a cede la place a un contenu REEL : sans bundle comme sans JavaScript,
    // la page porte son titre, sa description et sa navigation.
    assert!(html.contains("<main>"));
    assert!(html.contains("<h1>"));
    for segment in ["textures", "modeles", "sons", "videos"] {
        assert!(html.contains(&format!("href=\"/{segment}\"")), "lien /{segment} absent");
    }
}
