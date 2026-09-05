//! Pages HTML : la coquille d'`apps/nie-web` et les pages d'erreur, rendues par `askama`.
//!
//! `index.html` n'est pas servi tel quel : il passe par un template pour recevoir le titre, la
//! description et les balises `og:` **de la route demandée** — une texture ou un modèle partagé
//! sur un réseau social doit avoir sa vignette, et un SPA servi brut n'en a jamais.
//!
//! `askama_axum` est mort (`0.5.0+deprecated`) et il n'existe pas de feature `with-axum` en
//! askama 0.16 : on rend en `String` et on construit la réponse à la main.
//!
//! ## Ce que voit un robot
//!
//! La coquille portait des métadonnées correctes et un corps vide (`<div id="racine">`). Pour un
//! navigateur c'est suffisant — le bundle remplit la page. Pour Googlebot c'est un pari sur son
//! deuxième passage, pour Bing un pari perdu, et pour un aperçu Discord ou Slack — qui ne
//! lancent aucun JavaScript — une page sans contenu.
//!
//! Le template rend donc un `<main>` **réel** : titre, description, navigation vers les
//! catalogues, sélecteur de langue. Il vit à l'intérieur de `#racine`, l'élément que React
//! remplace au montage : le robot lit du contenu, le visiteur voit l'application, et il n'y a
//! qu'une seule vérité à maintenir.

use askama::Template;
use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Redirect, Response};

use crate::i18n::{Alternative, Langue, alternatives};
use crate::state::EtatSite;

/// Couleur de cadrage de la DA du jeu (`#295B9F`), mesurée sur le menu principal.
pub const COULEUR_THEME: &str = "#295B9F";

/// Le nom du jeu, **non traduit**.
///
/// C'est le titre commercial international, et c'est celui que portent les pages du jeu. Le
/// traduire de mémoire dans le titre japonais serait une invention : rien dans le gisement ne
/// donne le titre japonais officiel, et un titre inventé se propage dans les `<title>`, les
/// aperçus sociaux et le plan du site avant que personne ne le relise.
pub const JEU: &str = "Inazuma Eleven: Victory Road";

/// Une entrée de catalogue : son segment d'URL et ses libellés dans les trois langues.
///
/// Les tableaux sont indexés par [`Langue::TOUTES`] — `[fr, en, ja]`. Une table plutôt que trois
/// `match` : ajouter une langue devient une colonne, pas une réécriture.
struct Entree {
    segment: &'static str,
    titres: [&'static str; 3],
    descriptions: [&'static str; 3],
}

/// Les quatre catalogues d'Aphrody, dans l'ordre où ils sont présentés.
const ENTREES: [Entree; 4] = [
    Entree {
        segment: "textures",
        titres: ["Textures", "Textures", "テクスチャ"],
        descriptions: [
            "Toutes les textures du jeu, à leur chemin d'origine, converties à la demande.",
            "Every texture in the game, at its original path, converted on demand.",
            "ゲーム内のすべてのテクスチャを、元のパスのまま、必要に応じて変換して配信します。",
        ],
    },
    Entree {
        segment: "modeles",
        titres: ["Modèles", "Models", "モデル"],
        descriptions: [
            "Les modèles du jeu, assemblés et exportables, à leur chemin d'origine.",
            "The game's models, assembled and exportable, at their original path.",
            "ゲーム内のモデルを、組み立て済みかつ書き出し可能な形で、元のパスのまま提供します。",
        ],
    },
    Entree {
        segment: "sons",
        titres: ["Sons", "Sounds", "サウンド"],
        descriptions: [
            "Les banques audio du jeu (ACB, AWB, HCA), décodées à la demande.",
            "The game's audio banks (ACB, AWB, HCA), decoded on demand.",
            "ゲームのオーディオバンク（ACB・AWB・HCA）を、必要に応じてデコードします。",
        ],
    },
    Entree {
        segment: "videos",
        titres: ["Vidéos", "Videos", "ムービー"],
        descriptions: [
            "Les vidéos du jeu (USM), lisibles depuis leur chemin d'origine.",
            "The game's videos (USM), playable from their original path.",
            "ゲームのムービー（USM）を、元のパスから再生できます。",
        ],
    },
];

/// Index de la langue dans les tables de libellés.
const fn rang(langue: Langue) -> usize {
    match langue {
        Langue::Fr => 0,
        Langue::En => 1,
        Langue::Ja => 2,
    }
}

/// Titre et description de l'accueil.
fn accueil(langue: Langue) -> (String, String) {
    match langue {
        Langue::Fr => (
            format!("Aphrody — les fichiers d'{JEU}"),
            "Explorer, décoder et exporter les textures, modèles, sons et vidéos du jeu, depuis leur chemin d'origine.".to_owned(),
        ),
        Langue::En => (
            format!("Aphrody — the files of {JEU}"),
            "Browse, decode and export the game's textures, models, sounds and videos, straight from their original path.".to_owned(),
        ),
        Langue::Ja => (
            format!("Aphrody — {JEU} のファイル"),
            "ゲームのテクスチャ・モデル・サウンド・ムービーを、元のパスのまま閲覧・デコード・書き出しできます。".to_owned(),
        ),
    }
}

/// Un lien de navigation rendu côté serveur.
pub struct Lien {
    /// URL absolue ou relative à suivre.
    pub href: String,
    /// Libellé affiché.
    pub libelle: String,
    /// Langue de la ressource visée, quand elle diffère de celle de la page.
    ///
    /// Vide pour un lien interne de même langue : `hreflang=""` serait invalide, et y mettre le
    /// libellé — « Français », « 日本語 » — produirait un attribut qui ressemble à du balisage
    /// correct sans en être.
    pub hreflang: &'static str,
}

/// Coquille HTML servie pour toute route de navigation.
#[derive(Template)]
#[template(path = "index.html")]
pub struct Coquille {
    /// Code de langue de la page (`<html lang>`).
    pub lang: &'static str,
    /// Titre de la page (`<title>` et `og:title`).
    pub titre: String,
    /// Description (`meta description` et `og:description`).
    pub description: String,
    /// URL canonique absolue.
    pub url: String,
    /// Type Open Graph (`website`, `article`…).
    pub type_og: &'static str,
    /// Locale Open Graph de cette page (`fr_FR`, `en_US`, `ja_JP`).
    pub og_locale: &'static str,
    /// Les autres locales disponibles (`og:locale:alternate`).
    pub og_locales_alternes: Vec<&'static str>,
    /// Liens `hreflang` — les trois langues plus `x-default`.
    pub alternatives: Vec<Alternative>,
    /// Vignette absolue, quand la route en désigne une.
    pub image: Option<String>,
    /// Route demandée, transmise au bundle par `data-route`.
    pub route: String,
    /// Feuille de style du bundle, quand elle a été trouvée.
    pub feuille: Option<String>,
    /// Point d'entrée JavaScript du bundle, quand il a été trouvé.
    pub script: Option<String>,
    /// Couleur de thème (`theme-color`).
    pub couleur_theme: &'static str,
    /// Données structurées `schema.org`, déjà sérialisées et échappées.
    pub jsonld: String,
    /// Navigation vers les catalogues, rendue côté serveur.
    pub catalogues: Vec<Lien>,
    /// Sélecteur de langue, rendu côté serveur.
    pub langues: Vec<Lien>,
    /// Libellé de la section de navigation, dans la langue de la page.
    pub libelle_catalogues: &'static str,
    /// Libellé du sélecteur de langue, dans la langue de la page.
    pub libelle_langues: &'static str,
}

/// Page d'erreur HTML, pour les routes de navigation.
#[derive(Template)]
#[template(path = "erreur.html")]
pub struct PageErreur {
    /// Code HTTP.
    pub code: u16,
    /// Titre court.
    pub titre: &'static str,
    /// Message en français.
    pub message: String,
    /// Couleur de thème.
    pub couleur_theme: &'static str,
}

impl PageErreur {
    /// Construit une page d'erreur et la rend directement en réponse.
    #[must_use]
    pub fn reponse(code: StatusCode, titre: &'static str, message: impl Into<String>) -> Response {
        let page = Self {
            code: code.as_u16(),
            titre,
            message: message.into(),
            couleur_theme: COULEUR_THEME,
        };
        match page.render() {
            Ok(html) => (code, Html(html)).into_response(),
            Err(e) => {
                tracing::error!(erreur = %e, "rendu de la page d'erreur impossible");
                (code, "erreur").into_response()
            }
        }
    }
}

/// Métadonnées d'une route de navigation, dans une langue : ce qui distingue une page d'une
/// autre pour un robot ou un aperçu de réseau social.
///
/// `route` est la route **nue**, sans préfixe de langue (cf. [`Langue::separer`]).
#[must_use]
pub fn metadonnees(route: &str, langue: Langue) -> (String, String, &'static str) {
    let nu = route.trim_start_matches('/').trim_end_matches('/');
    let premier = nu.split('/').next().unwrap_or("");
    if premier.is_empty() {
        let (t, d) = accueil(langue);
        return (t, d, "website");
    }
    let i = rang(langue);
    if let Some(e) = ENTREES.iter().find(|e| e.segment == premier) {
        return (
            format!("{} — Aphrody", e.titres[i]),
            e.descriptions[i].to_owned(),
            "website",
        );
    }
    let generique = match langue {
        Langue::Fr => "Explorer les fichiers d'Inazuma Eleven: Victory Road.",
        Langue::En => "Browse the files of Inazuma Eleven: Victory Road.",
        Langue::Ja => "イナズマイレブン Victory Road のファイルを閲覧します。",
    };
    (
        format!("{premier} — Aphrody"),
        generique.to_owned(),
        "article",
    )
}

/// Échappe une valeur JSON pour une insertion sûre dans un `<script>`.
///
/// `serde_json` échappe ce qu'exige JSON, pas ce qu'exige HTML : une chaîne contenant
/// `</script>` refermerait le bloc et le reste du document deviendrait du texte. Les trois
/// séquences neutralisées ici sont celles que le parseur HTML reconnaît à l'intérieur d'un
/// `<script>`, et leur forme `\uXXXX` reste du JSON parfaitement valide.
fn json_sur_pour_script(json: &str) -> String {
    json.replace('<', r"\u003c")
        .replace('>', r"\u003e")
        .replace('&', r"\u0026")
}

/// Les données structurées de la page, sérialisées.
///
/// Ce que Google exploite réellement ici est le fil d'Ariane (affiché dans les résultats) et le
/// `SearchAction` de l'accueil ; le reste décrit le site pour les moteurs qui savent le lire.
/// Rien de tout cela n'invente de données : les libellés sont ceux de la page.
fn donnees_structurees(origine: &str, route: &str, langue: Langue, titre: &str, description: &str) -> String {
    let url = langue.url(origine, route);
    let racine = langue.url(origine, "/");
    let nu = route.trim_matches('/');

    let mut graphe = vec![serde_json::json!({
        "@type": "WebSite",
        "@id": format!("{racine}#site"),
        "name": "Aphrody",
        "url": racine,
        "inLanguage": langue.code(),
        "description": description,
        "potentialAction": {
            "@type": "SearchAction",
            "target": {
                "@type": "EntryPoint",
                "urlTemplate": format!("{racine}?q={{recherche}}"),
            },
            "query-input": "required name=recherche",
        },
    })];

    if nu.is_empty() {
        graphe.push(serde_json::json!({
            "@type": "WebPage",
            "@id": url,
            "url": url,
            "name": titre,
            "description": description,
            "inLanguage": langue.code(),
            "isPartOf": { "@id": format!("{racine}#site") },
        }));
    } else {
        graphe.push(serde_json::json!({
            "@type": "CollectionPage",
            "@id": url,
            "url": url,
            "name": titre,
            "description": description,
            "inLanguage": langue.code(),
            "isPartOf": { "@id": format!("{racine}#site") },
            "about": { "@type": "VideoGame", "name": JEU },
        }));
        graphe.push(serde_json::json!({
            "@type": "BreadcrumbList",
            "itemListElement": [
                { "@type": "ListItem", "position": 1, "name": "Aphrody", "item": racine },
                { "@type": "ListItem", "position": 2, "name": titre.trim_end_matches(" — Aphrody"), "item": url },
            ],
        }));
    }

    let doc = serde_json::json!({ "@context": "https://schema.org", "@graph": graphe });
    json_sur_pour_script(&doc.to_string())
}

/// Construit la coquille d'une route, dans une langue.
///
/// Séparé du handler pour être testable sans serveur ni état.
#[must_use]
pub fn construire(
    origine: &str,
    route: &str,
    langue: Langue,
    feuille: Option<String>,
    script: Option<String>,
) -> Coquille {
    let (titre, description, type_og) = metadonnees(route, langue);
    let i = rang(langue);
    let catalogues = ENTREES
        .iter()
        .map(|e| Lien {
            href: format!("{}/{}", langue.prefixe(), e.segment),
            libelle: e.titres[i].to_owned(),
            hreflang: "",
        })
        .collect();
    let langues = Langue::TOUTES
        .iter()
        .filter(|l| **l != langue)
        .map(|l| Lien {
            // Relatif, et non absolu comme le `hreflang` du `<head>`. Les deux ne servent pas
            // le meme public : `hreflang` s'adresse a un moteur, qui exige une URL absolue ;
            // ceci est un lien qu'un visiteur clique, et une URL absolue vers `aphrody.com`
            // ferait quitter une preview ou une machine de developpement pour la production.
            href: match l.url("", route).as_str() {
                "" => "/".to_owned(),
                relatif => relatif.to_owned(),
            },
            libelle: l.nom_natif().to_owned(),
            hreflang: l.code(),
        })
        .collect();
    let (libelle_catalogues, libelle_langues) = match langue {
        Langue::Fr => ("Catalogues", "Langues"),
        Langue::En => ("Catalogues", "Languages"),
        Langue::Ja => ("カタログ", "言語"),
    };
    Coquille {
        lang: langue.code(),
        jsonld: donnees_structurees(origine, route, langue, &titre, &description),
        url: langue.url(origine, route),
        og_locale: langue.og_locale(),
        og_locales_alternes: Langue::TOUTES
            .iter()
            .filter(|l| **l != langue)
            .map(|l| l.og_locale())
            .collect(),
        alternatives: alternatives(origine, route),
        titre,
        description,
        type_og,
        image: None,
        route: route.to_owned(),
        feuille,
        script,
        couleur_theme: COULEUR_THEME,
        catalogues,
        langues,
        libelle_catalogues,
        libelle_langues,
    }
}

/// Sert la coquille pour une route de navigation.
pub async fn coquille(State(etat): State<EtatSite>, uri: Uri) -> Response {
    let demande = Langue::separer(uri.path());
    if demande.rediriger {
        // `/fr/x` est compris mais n'est pas la forme canonique. Un 308 — et non un 301 —
        // parce qu'il préserve la méthode et se laisse revenir en arrière : un 301 se grave
        // dans le cache des navigateurs déjà passés, et ne s'en retire plus.
        let cible = demande.langue.url("", &demande.route);
        let cible = if cible.is_empty() { "/".to_owned() } else { cible };
        return Redirect::permanent(&cible).into_response();
    }
    let (feuille, script) =
        crate::routes::static_files::points_d_entree(&etat.config.statique).await;
    let page = construire(
        &etat.config.origine,
        &demande.route,
        demande.langue,
        feuille,
        script,
    );
    match page.render() {
        Ok(html) => (
            StatusCode::OK,
            [
                (
                    header::CACHE_CONTROL,
                    "public, max-age=60, stale-while-revalidate=600",
                ),
                // La page change avec la langue de l'URL, jamais avec un en-tête : le dire
                // évite qu'un cache intermédiaire serve une langue pour une autre.
                (header::VARY, "Accept-Encoding"),
            ],
            Html(html),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(erreur = %e, "rendu de la coquille impossible");
            PageErreur::reponse(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Page indisponible",
                "La coquille du site n'a pas pu être rendue.",
            )
        }
    }
}

/// Repli des routes inconnues : JSON sous `/api`, page d'erreur HTML ailleurs.
pub async fn repli(State(etat): State<EtatSite>, uri: Uri) -> Response {
    let chemin = uri.path();
    if chemin.starts_with("/api/") || chemin == "/api" {
        return crate::ErreurSite::Introuvable(format!("route d'API inconnue: {chemin}"))
            .into_response();
    }
    if chemin.starts_with("/assets/") || chemin.starts_with("/f/") || chemin.starts_with("/b/") {
        return crate::ErreurSite::Introuvable(format!("ressource inconnue: {chemin}"))
            .into_response();
    }
    // Toute autre route est une route du bundle : c'est le client qui décide si elle existe.
    coquille(State(etat), uri).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(route: &str, langue: Langue) -> String {
        construire("https://aphrody.com", route, langue, None, None)
            .render()
            .expect("rendu")
    }

    #[test]
    fn metadonnees_par_route() {
        assert!(metadonnees("/", Langue::Fr).0.starts_with("Aphrody"));
        assert_eq!(metadonnees("/textures", Langue::Fr).0, "Textures — Aphrody");
        assert_eq!(metadonnees("/modeles/x/y", Langue::Fr).2, "website");
        assert_eq!(metadonnees("/inconnue", Langue::Fr).2, "article");
    }

    #[test]
    fn les_trois_langues_ont_des_titres_distincts() {
        let fr = metadonnees("/modeles", Langue::Fr).0;
        let en = metadonnees("/modeles", Langue::En).0;
        let ja = metadonnees("/modeles", Langue::Ja).0;
        assert_eq!(fr, "Modèles — Aphrody");
        assert_eq!(en, "Models — Aphrody");
        assert_eq!(ja, "モデル — Aphrody");
        // Une traduction oubliée se voit ici, pas en production.
        assert_ne!(fr, en);
        assert_ne!(en, ja);
        for langue in Langue::TOUTES {
            let (t, d, _) = metadonnees("/sons", langue);
            assert!(!t.is_empty() && !d.is_empty(), "libellé vide en {langue}");
        }
    }

    #[test]
    fn coquille_porte_les_balises_og() {
        let mut c = construire("https://aphrody.com", "/", Langue::Fr, None, None);
        c.image = Some("https://aphrody.com/i.png".to_owned());
        let html = c.render().expect("rendu");
        for balise in [
            "og:title",
            "og:description",
            "og:url",
            "og:image",
            "og:type",
            "og:site_name",
            "og:locale",
            "twitter:card",
        ] {
            assert!(html.contains(balise), "balise {balise} absente");
        }
    }

    #[test]
    fn la_langue_de_la_page_est_celle_de_l_url() {
        assert!(page("/", Langue::Ja).contains(r#"<html lang="ja">"#));
        assert!(page("/", Langue::En).contains(r#"<html lang="en">"#));
        assert!(page("/", Langue::Fr).contains(r#"<html lang="fr">"#));
        assert!(page("/", Langue::Ja).contains(r#"content="ja_JP""#));
        // Les deux autres locales sont annoncées, jamais celle de la page.
        let ja = page("/", Langue::Ja);
        assert!(ja.contains("og:locale:alternate"));
        assert_eq!(ja.matches("og:locale:alternate").count(), 2);
    }

    #[test]
    fn hreflang_complet_et_reciproque() {
        for langue in Langue::TOUTES {
            let html = page("/textures", langue);
            for attendu in [
                r#"hreflang="fr" href="https://aphrody.com/textures""#,
                r#"hreflang="en" href="https://aphrody.com/en/textures""#,
                r#"hreflang="ja" href="https://aphrody.com/ja/textures""#,
                r#"hreflang="x-default" href="https://aphrody.com/textures""#,
            ] {
                assert!(html.contains(attendu), "en {langue}, absent : {attendu}");
            }
            assert_eq!(html.matches("rel=\"alternate\"").count(), 4);
        }
    }

    #[test]
    fn le_canonical_porte_le_prefixe_de_langue() {
        assert!(page("/textures", Langue::Ja)
            .contains(r#"<link rel="canonical" href="https://aphrody.com/ja/textures">"#));
        assert!(page("/", Langue::Fr)
            .contains(r#"<link rel="canonical" href="https://aphrody.com">"#));
    }

    #[test]
    fn le_corps_contient_du_contenu_lisible_sans_javascript() {
        let html = page("/", Langue::Fr);
        // Un robot qui n'exécute rien doit trouver un titre, une description et des liens.
        assert!(html.contains("<h1>"));
        assert!(html.contains("<main"));
        for segment in ["textures", "modeles", "sons", "videos"] {
            assert!(
                html.contains(&format!("href=\"/{segment}\"")),
                "lien /{segment} absent du rendu serveur"
            );
        }
        // Le sélecteur de langue mène aux deux autres versions, en RELATIF : une preview ne
        // doit pas renvoyer son visiteur vers la production.
        assert!(html.contains(r#"<a href="/en" hreflang="en""#));
        assert!(html.contains(r#"<a href="/ja" hreflang="ja""#));
    }

    #[test]
    fn en_japonais_les_liens_de_catalogue_restent_dans_la_langue() {
        let html = page("/", Langue::Ja);
        assert!(html.contains("href=\"/ja/textures\""));
        assert!(html.contains("テクスチャ"));
        // Le sélecteur renvoie vers les deux AUTRES langues, jamais vers soi-même.
        assert!(!html.contains(">日本語</a>"));
        // Depuis le japonais, le français est la racine — pas une chaîne vide.
        assert!(html.contains(r#"<a href="/" hreflang="fr""#));
    }

    #[test]
    fn les_donnees_structurees_sont_du_json_valide_et_inoffensif() {
        for langue in Langue::TOUTES {
            for route in ["/", "/textures"] {
                let c = construire("https://aphrody.com", route, langue, None, None);
                // Le `<` échappé doit se relire comme du JSON : sinon la page porte un bloc mort.
                let brut = c.jsonld.replace(r"\u003c", "<").replace(r"\u003e", ">").replace(r"\u0026", "&");
                let v: serde_json::Value = serde_json::from_str(&brut).expect("json-ld valide");
                assert_eq!(v["@context"], "https://schema.org");
                assert!(v["@graph"].as_array().is_some_and(|g| !g.is_empty()));
                // Aucun `<` ne subsiste : il refermerait le <script>.
                assert!(!c.jsonld.contains('<'), "jsonld non échappé en {langue}");
            }
        }
        let c = construire("https://aphrody.com", "/textures", Langue::Fr, None, None);
        let brut = c.jsonld.replace(r"\u003c", "<").replace(r"\u003e", ">").replace(r"\u0026", "&");
        let v: serde_json::Value = serde_json::from_str(&brut).expect("json-ld");
        let types: Vec<_> = v["@graph"]
            .as_array()
            .expect("graphe")
            .iter()
            .map(|n| n["@type"].as_str().unwrap_or_default().to_owned())
            .collect();
        assert!(types.contains(&"BreadcrumbList".to_owned()));
        assert!(types.contains(&"CollectionPage".to_owned()));
    }
}
