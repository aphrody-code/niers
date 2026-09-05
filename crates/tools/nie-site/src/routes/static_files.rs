//! Le bundle d'`apps/nie-web` : fichiers pré-compressés, empreintes et cache.
//!
//! Trois règles, toutes vérifiables depuis l'extérieur :
//!
//! 1. **Pré-compression servie telle quelle.** Si `x.js.br` ou `x.js.zst` existe à côté de
//!    `x.js`, il est servi avec `Content-Encoding` et `Vary: Accept-Encoding` — on ne
//!    recompresse jamais à la volée ce qui a déjà été compressé au build (et on ne cumule
//!    jamais `precompressed_*` avec une couche de compression, cf. `docs/stack/pieges-api.md`).
//! 2. **Immuable si empreinté.** Un nom qui porte une empreinte (`app-1a2b3c4d.js`) est
//!    `public, max-age=31536000, immutable` ; tout le reste est `no-cache` — un `index.html`
//!    figé un an dans un cache navigateur est un site qu'on ne peut plus déployer.
//! 3. **Aucune sortie de la racine.** Les composants `..`, absolus ou non normaux sont
//!    refusés avant tout accès disque.

use std::path::{Component, Path, PathBuf};

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;

use crate::state::{EtatSite, ReponseCachee};

/// Taille au-delà de laquelle un fichier statique n'est plus gardé en mémoire (il est relu
/// depuis le disque à chaque requête, ce que le cache de pages du noyau absorbe très bien).
pub const TAILLE_MAX_CACHE: u64 = 4 * 1024 * 1024;

/// `Cache-Control` d'un fichier empreinté.
pub const IMMUABLE: &str = "public, max-age=31536000, immutable";
/// `Cache-Control` d'un fichier sans empreinte.
pub const REVALIDER: &str = "no-cache";

/// Dit si un nom de fichier porte une empreinte de contenu.
///
/// Deux formes existent en pratique, et la première seule ne suffit pas :
///
/// - **hexadécimale** (`index.9f8e7d6c5b4a.css`) — Rollup et esbuild, et Vite quand on le
///   configure ainsi ;
/// - **base64url** (`index-RXLrxaJS.js`) — la forme que Vite produit PAR DÉFAUT, majuscules
///   comprises. C'est celle du bundle d'`apps/nie-web`.
///
/// Le piège est qu'un mot ordinaire est lui aussi une suite de lettres : `app-composant.js` ne
/// doit pas être pris pour un fichier empreinté, sans quoi on le figerait un an dans les caches
/// et on ne pourrait plus le déployer. On exige donc, pour la forme base64url, un mélange de
/// casse ou un chiffre — ce qu'un mot n'a pas et qu'un condensé a presque toujours.
///
/// Cette heuristique n'est que le second critère : la règle principale est [`immuable`], qui
/// regarde le DOSSIER et ne devine rien.
#[must_use]
pub fn empreinte(nom: &str) -> bool {
    nom.split(['-', '.']).skip(1).any(|s| {
        if s.len() < 8 || s.len() > 40 {
            return false;
        }
        if s.chars().all(|c| c.is_ascii_hexdigit()) {
            return true;
        }
        if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
        let chiffre = s.chars().any(|c| c.is_ascii_digit());
        let melange = s.chars().any(char::is_uppercase) && s.chars().any(char::is_lowercase);
        chiffre || melange
    })
}

/// Dit si un fichier du bundle peut être servi `immutable`.
///
/// La règle principale ne devine pas : tout ce qu'un bundler dépose dans son dossier d'assets
/// (`static/`, `assets/` — cf. [`DOSSIERS_BUNDLE`]) est empreinté par construction. Ce qui vit
/// à la racine du bundle (`index.html`, un favicon, un fichier copié de `public/`) ne l'est pas,
/// et doit rester revalidable. Un nom empreinté hors de ces dossiers est accepté en second
/// recours via [`empreinte`].
#[must_use]
pub fn immuable(relatif: &Path) -> bool {
    let dans_dossier_bundle = relatif
        .components()
        .next()
        .and_then(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .is_some_and(|d| DOSSIERS_BUNDLE.contains(&d));
    let a_plusieurs_composants = relatif.components().count() > 1;
    (dans_dossier_bundle && a_plusieurs_composants)
        || relatif.file_name().and_then(|n| n.to_str()).is_some_and(empreinte)
}

/// Normalise un chemin relatif reçu d'un client : rend `None` dès qu'il sort de la racine.
#[must_use]
pub fn chemin_sur(relatif: &str) -> Option<PathBuf> {
    let relatif = relatif.trim_start_matches('/');
    if relatif.is_empty() {
        return Some(PathBuf::new());
    }
    let mut sortie = PathBuf::new();
    for c in Path::new(relatif).components() {
        match c {
            Component::Normal(s) => {
                let s = s.to_str()?;
                if s.contains('\0') {
                    return None;
                }
                sortie.push(s);
            }
            // `..`, `/`, `C:` : un client n'a aucune raison légitime d'en envoyer.
            _ => return None,
        }
    }
    Some(sortie)
}

/// Type de contenu déduit de l'extension. Table volontairement courte : ce sont les seuls
/// types qu'un bundle produit, et les formats du jeu ont la leur dans [`super::vfs`].
#[must_use]
pub fn type_contenu(chemin: &Path) -> &'static str {
    let ext = chemin
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

/// Encodage retenu pour une réponse pré-compressée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encodage {
    /// Le fichier tel quel.
    Identite,
    /// Variante `.br`.
    Brotli,
    /// Variante `.zst`.
    Zstd,
}

impl Encodage {
    /// Suffixe de fichier de la variante.
    #[must_use]
    pub fn suffixe(self) -> &'static str {
        match self {
            Self::Identite => "",
            Self::Brotli => ".br",
            Self::Zstd => ".zst",
        }
    }

    /// Valeur d'en-tête `Content-Encoding`, ou `None` pour l'identité.
    #[must_use]
    pub fn entete(self) -> Option<&'static str> {
        match self {
            Self::Identite => None,
            Self::Brotli => Some("br"),
            Self::Zstd => Some("zstd"),
        }
    }
}

/// Choisit la meilleure variante disponible pour un fichier, selon `Accept-Encoding`.
///
/// Brotli d'abord (meilleur ratio sur du texte), zstd ensuite, identité en dernier. Un client
/// qui n'annonce rien reçoit l'identité — jamais un corps qu'il ne sait pas décoder.
#[must_use]
pub fn negocier(fichier: &Path, accept: Option<&str>) -> Encodage {
    let accept = accept.unwrap_or("").to_ascii_lowercase();
    let accepte = |jeton: &str| {
        accept.split(',').any(|p| {
            let p = p.trim();
            let nom = p.split(';').next().unwrap_or(p).trim();
            nom == jeton && !p.contains("q=0,") && !p.ends_with("q=0") && !p.contains("q=0.0")
        })
    };
    if accepte("br") && variante(fichier, Encodage::Brotli).is_file() {
        return Encodage::Brotli;
    }
    if accepte("zstd") && variante(fichier, Encodage::Zstd).is_file() {
        return Encodage::Zstd;
    }
    Encodage::Identite
}

fn variante(fichier: &Path, enc: Encodage) -> PathBuf {
    let mut s = fichier.as_os_str().to_os_string();
    s.push(enc.suffixe());
    PathBuf::from(s)
}

/// Dossiers où un bundle peut déposer ses fichiers empreintés, dans l'ordre d'essai.
///
/// `apps/nie-web` écrit dans `static/` et non dans le `assets/` par défaut de Vite : le proxy
/// vers `nie-model-serve` occupe déjà le préfixe `/assets/`, et laisser les deux se recouvrir
/// rendait le bundle dépendant de l'ordre des routes. `assets` reste accepté en repli pour un
/// bundle produit avec la configuration par défaut.
pub const DOSSIERS_BUNDLE: [&str; 2] = ["static", "assets"];

/// Cherche la feuille de style et le point d'entrée JavaScript du bundle.
///
/// Sert à la coquille `askama` : le nom des fichiers porte une empreinte qui change à chaque
/// build, on ne peut donc pas les coder en dur dans le template.
///
/// Rend `(None, None)` quand aucun bundle n'est présent — la coquille est alors servie sans
/// `<script>`, ce qui est le comportement voulu tant qu'`apps/nie-web` n'a pas été construit.
/// Ce mode d'échec est SILENCIEUX par nature (page valide, 200, aucun log) : c'est
/// `coquille_charge_le_bundle` qui le surveille, pas la lecture du code.
pub async fn points_d_entree(racine: &Path) -> (Option<String>, Option<String>) {
    for dossier in DOSSIERS_BUNDLE {
        let (css, js) = points_d_entree_dans(racine, dossier).await;
        if css.is_some() || js.is_some() {
            return (css, js);
        }
    }
    (None, None)
}

/// Balaie un dossier précis du bundle. Les fichiers sont triés pour que le point d'entrée
/// retenu ne dépende pas de l'ordre de lecture du système de fichiers.
async fn points_d_entree_dans(racine: &Path, dossier: &str) -> (Option<String>, Option<String>) {
    let chemin = racine.join(dossier);
    let Ok(mut lecture) = tokio::fs::read_dir(&chemin).await else {
        return (None, None);
    };
    let mut noms = Vec::new();
    while let Ok(Some(e)) = lecture.next_entry().await {
        noms.push(e.file_name().to_string_lossy().into_owned());
    }
    noms.sort_unstable();
    let trouve = |suffixes: &[&str]| -> Option<String> {
        noms.iter()
            .find(|n| suffixes.iter().any(|s| n.ends_with(s)))
            .map(|n| format!("/{dossier}/{n}"))
    };
    (trouve(&[".css"]), trouve(&[".js", ".mjs"]))
}

/// Sert un fichier du bundle. Rend `None` quand il n'existe pas : c'est l'appelant qui décide
/// du repli (la coquille pour une route de navigation, une erreur pour une ressource).
pub async fn servir(
    etat: &EtatSite,
    relatif: &str,
    entetes: &HeaderMap,
) -> Option<Response> {
    let sur = chemin_sur(relatif)?;
    if sur.as_os_str().is_empty() {
        return None;
    }
    let fichier = etat.config.statique.join(&sur);
    let meta = tokio::fs::metadata(&fichier).await.ok()?;
    if !meta.is_file() {
        return None;
    }
    let accept = entetes.get(header::ACCEPT_ENCODING).and_then(|v| v.to_str().ok());
    let enc = negocier(&fichier, accept);
    let chemin_servi = variante(&fichier, enc);
    let cle = format!("statique:{}:{}", chemin_servi.display(), enc.suffixe());

    let cachee = match etat.cache.get(&cle).await {
        Some(c) => Some(c),
        None => {
            let octets = tokio::fs::read(&chemin_servi).await.ok()?;
            let taille = octets.len() as u64;
            let corps = Bytes::from(octets);
            let c = ReponseCachee {
                etag: etiquette(&corps),
                type_contenu: type_contenu(&fichier).to_owned(),
                corps,
            };
            if taille <= TAILLE_MAX_CACHE {
                etat.cache.insert(cle, c.clone()).await;
            }
            Some(c)
        }
    }?;

    let controle = if immuable(&sur) { IMMUABLE } else { REVALIDER };
    Some(reponse_octets(&cachee, controle, enc, entetes))
}

/// Construit la réponse d'un corps déjà connu, avec ETag, `304` et `Vary`.
#[must_use]
pub fn reponse_octets(
    cachee: &ReponseCachee,
    controle: &str,
    enc: Encodage,
    entetes: &HeaderMap,
) -> Response {
    let deja = entetes
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|e| e.trim() == cachee.etag));

    let mut reponse = if deja {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        (StatusCode::OK, Body::from(cachee.corps.clone())).into_response()
    };
    let h = reponse.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&cachee.type_contenu) {
        h.insert(header::CONTENT_TYPE, v);
    }
    if let Ok(v) = HeaderValue::from_str(&cachee.etag) {
        h.insert(header::ETAG, v);
    }
    if let Ok(v) = HeaderValue::from_str(controle) {
        h.insert(header::CACHE_CONTROL, v);
    }
    h.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    if let Some(e) = enc.entete() {
        h.insert(header::CONTENT_ENCODING, HeaderValue::from_static(e));
    }
    reponse
}

/// ETag fort : `blake3` du corps servi, en hexadécimal, entre guillemets.
#[must_use]
pub fn etiquette(corps: &[u8]) -> String {
    format!("\"{}\"", blake3::hash(corps).to_hex())
}

/// Route de repli du bundle : sert le fichier s'il existe, la coquille sinon.
pub async fn statique(
    State(etat): State<EtatSite>,
    entetes: HeaderMap,
    uri: axum::http::Uri,
) -> Response {
    if let Some(r) = servir(&etat, uri.path(), &entetes).await {
        return r;
    }
    super::pages::repli(State(etat), uri).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empreintes_reconnues() {
        assert!(empreinte("app-1a2b3c4d.js"), "hexadecimal");
        assert!(empreinte("index.9f8e7d6c5b4a.css"), "hexadecimal");
        // La forme que Vite produit par defaut — c'est elle qui manquait, et le bundle
        // d'`apps/nie-web` etait servi `no-cache` a chaque requete.
        assert!(empreinte("index-RXLrxaJS.js"), "base64url melangeant les casses");
        assert!(empreinte("app-B7xk29Za.css"), "base64url avec chiffres");
        assert!(!empreinte("index.html"));
        assert!(!empreinte("app-1a2b.js"), "moins de huit caracteres");
        assert!(!empreinte("app-composant.js"), "un mot n'est pas une empreinte");
        assert!(!empreinte("mon-fichier-normal.js"), "un mot n'est pas une empreinte");
    }

    #[test]
    fn immuable_suit_le_dossier_avant_le_nom() {
        for d in DOSSIERS_BUNDLE {
            assert!(immuable(&PathBuf::from(d).join("index-RXLrxaJS.js")));
            // Meme un nom sans empreinte est immuable dans le dossier d'assets : c'est le
            // bundler qui garantit l'unicite, pas la forme du nom.
            assert!(immuable(&PathBuf::from(d).join("worker.js")));
        }
        // La racine du bundle n'est jamais figee : un index.html immuable est un site qu'on
        // ne peut plus deployer.
        assert!(!immuable(Path::new("index.html")));
        assert!(!immuable(Path::new("favicon.ico")));
        assert!(!immuable(Path::new("static")), "le dossier lui-meme n'est pas un fichier");
        // Hors dossier connu, on retombe sur le nom.
        assert!(immuable(Path::new("vendor/app-1a2b3c4d.js")));
    }

    #[test]
    fn chemins_hors_racine_refuses() {
        assert!(chemin_sur("../etc/passwd").is_none());
        assert!(chemin_sur("/a/../../b").is_none());
        assert!(chemin_sur("/assets/app.js").is_some());
        assert_eq!(chemin_sur("/assets/app.js").unwrap(), Path::new("assets/app.js"));
    }

    #[test]
    fn negociation_prefere_brotli() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("app.js");
        std::fs::write(&f, b"x").unwrap();
        assert_eq!(negocier(&f, Some("br, gzip")), Encodage::Identite, "aucune variante presente");
        std::fs::write(dir.path().join("app.js.zst"), b"z").unwrap();
        assert_eq!(negocier(&f, Some("zstd")), Encodage::Zstd);
        std::fs::write(dir.path().join("app.js.br"), b"b").unwrap();
        assert_eq!(negocier(&f, Some("br, zstd")), Encodage::Brotli);
        assert_eq!(negocier(&f, Some("gzip")), Encodage::Identite);
        assert_eq!(negocier(&f, None), Encodage::Identite);
    }

    #[test]
    fn types_de_contenu() {
        assert_eq!(type_contenu(Path::new("a.js")), "text/javascript; charset=utf-8");
        assert_eq!(type_contenu(Path::new("a.wasm")), "application/wasm");
        assert_eq!(type_contenu(Path::new("a.inconnu")), "application/octet-stream");
    }
}
