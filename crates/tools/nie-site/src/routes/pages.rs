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
//! navigateur c'est suffisant — le bundle remplit la page.
//!
//! Pour Googlebot, ce n'est plus un obstacle d'indexation : toutes les pages HTML sont rendues.
//! C'est en revanche un coût de latence à queue longue — la mesure indépendante la plus sérieuse
//! (MERJ × Vercel, 2024, 37 000 paires) donne une médiane de 10 s avant rendu, mais un p90 autour
//! de 3 h et un p99 vers 18 h. Sur un catalogue qui bouge, le rendu serveur ne débloque pas
//! l'indexation : il raccourcit la fraîcheur.
//!
//! Pour Bing et pour les aperçus sociaux, en revanche, l'obstacle est entier. La documentation de
//! Meta écrit noir sur blanc que le crawler de WhatsApp n'exécute aucun JavaScript, n'attend aucun
//! chargement et ne défile pas ; Discord, Slack, X et LinkedIn se comportent de même. Aucun des
//! sept ne lit le JSON-LD pour composer son aperçu. Un lien partagé vers une coquille vide est un
//! lien sans titre et sans image.
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

/// La vignette de partage, servie par le bundle.
///
/// Elle n'etait **jamais** emise : `image` valait `None` en dur, et le seul appelant ne la
/// posait pas. Consequence invisible en developpement et visible partout ailleurs — tout lien
/// vers Aphrody partage sur Discord, Slack, X ou WhatsApp s'affichait sans vignette, et rien
/// dans les tests ne le disait, puisqu'ils verifiaient la balise sur une coquille de test ou
/// l'image etait injectee a la main.
///
/// 1200x630, la taille que toutes les plateformes acceptent, pour 23 Ko — largement sous la
/// rupture de WhatsApp (~300 Ko observes) et sous les 5 Mo de LinkedIn.
pub const VIGNETTE: &str = "/static/og.png";

/// Largeur de [`VIGNETTE`], declaree pour eviter un fetch asynchrone de la plateforme.
pub const VIGNETTE_L: u32 = 1200;

/// Hauteur de [`VIGNETTE`].
pub const VIGNETTE_H: u32 = 630;

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

/// Nombre d'entrées rendues par page de catalogue.
///
/// 60, la même valeur qu'`apps/azalee` a retenue après avoir mesuré une page de 2 355 397
/// octets à 200 entrées. Au-delà, le poids de la page cesse d'être proportionnel à ce qu'un
/// visiteur lit réellement.
pub const PAR_PAGE: usize = 60;

/// Une entrée de catalogue, rendue côté serveur.
pub struct Element {
    /// URL de la ressource — son chemin VFS verbatim, sous `/f/`.
    pub href: String,
    /// Nom de la feuille, extension du jeu conservée.
    pub nom: String,
    /// Chemin complet, affiché : c'est l'identifiant, et il vaut d'être lu.
    pub chemin: String,
    /// Taille, en unités lisibles.
    pub taille: String,
}

/// Une page de catalogue rendue côté serveur.
pub struct Catalogue {
    /// Les entrées de cette page.
    pub elements: Vec<Element>,
    /// Nombre total d'entrées du catalogue.
    pub total: usize,
    /// Numéro de la page courante, à partir de 1.
    pub page: usize,
    /// Nombre de pages.
    pub pages: usize,
    /// URL de la page précédente, quand il y en a une.
    pub precedent: Option<String>,
    /// URL de la page suivante, quand il y en a une.
    pub suivant: Option<String>,
}

/// Taille en octets, rendue lisible.
///
/// Les puissances de 1024 et leurs symboles usuels ; une décimale au-delà du kilo-octet, aucune
/// en dessous — « 3,4 Mio » se lit, « 3565158 o » se compte.
#[must_use]
pub fn taille_lisible(octets: u32) -> String {
    const SEUIL: f64 = 1024.0;
    let o = f64::from(octets);
    if o < SEUIL {
        return format!("{octets} o");
    }
    let unites = ["kio", "Mio", "Gio"];
    let mut valeur = o / SEUIL;
    let mut rang = 0;
    while valeur >= SEUIL && rang + 1 < unites.len() {
        valeur /= SEUIL;
        rang += 1;
    }
    format!("{valeur:.1} {}", unites[rang])
}

/// Le numéro de page demandé par la requête, borné à 1 au minimum.
///
/// Une valeur absente, vide, nulle ou illisible vaut 1. Refuser la requête n'apporterait rien :
/// `?page=abc` est une URL fabriquée, pas une erreur de l'utilisateur, et la première page est
/// une réponse correcte à une question mal posée.
#[must_use]
pub fn numero_de_page(query: Option<&str>) -> usize {
    query
        .and_then(|q| {
            q.split('&')
                .find_map(|p| p.strip_prefix("page="))
                .and_then(|v| v.parse::<usize>().ok())
        })
        .filter(|n| *n >= 1)
        .unwrap_or(1)
}

/// Largeur d'affichage d'un texte, en demi-cadratins.
///
/// ## Pourquoi compter autre chose que des caractères
///
/// Un moteur ne tronque pas un titre à un nombre de caractères mais à une largeur — et un
/// idéogramme occupe deux fois la place d'une lettre latine (Unicode UAX #11 les classe
/// *Wide* ou *Fullwidth*). Valider `titre.chars().count()` laisserait donc passer un titre
/// japonais deux fois trop long, et rejetterait un titre français correct.
///
/// Un seul compteur pour les trois langues : 2 unités pour un caractère large, 1 sinon.
#[must_use]
pub fn largeur_affichage(texte: &str) -> usize {
    texte.chars().map(|c| if est_large(c) { 2 } else { 1 }).sum()
}

/// Dit si un caractère occupe deux demi-cadratins (UAX #11, classes `W` et `F`).
///
/// Les plages retenues sont celles qui apparaissent réellement dans du texte japonais, coréen
/// ou chinois — pas la table complète d'UAX #11, qui décrirait des écritures que ce site
/// n'affiche pas.
const fn est_large(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F      // jamos hangul de tête
        | 0x2E80..=0x303E    // radicaux CJK, Kangxi, ponctuation CJK
        | 0x3041..=0x33FF    // hiragana, katakana, bopomofo, carrés de compatibilité
        | 0x3400..=0x4DBF    // idéogrammes, extension A
        | 0x4E00..=0x9FFF    // idéogrammes unifiés
        | 0xA000..=0xA4CF    // yi
        | 0xAC00..=0xD7A3    // syllabes hangul
        | 0xF900..=0xFAFF    // idéogrammes de compatibilité
        | 0xFE30..=0xFE6F    // formes de ponctuation verticale
        | 0xFF00..=0xFF60    // formes pleine chasse
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x2FFFD  // extensions B et suivantes
        | 0x30000..=0x3FFFD
    )
}

/// Largeur au-delà de laquelle un `<title>` est tronqué à l'affichage.
///
/// Google ne fixe aucune limite de longueur ; c'est la largeur du bandeau de résultats qui
/// coupe. 60 demi-cadratins couvrent les deux usages mesurés : ~55-60 caractères latins, ~30
/// caractères japonais.
pub const LARGEUR_TITRE: usize = 60;

/// Largeur au-delà de laquelle une `meta description` est tronquée.
pub const LARGEUR_DESCRIPTION: usize = 240;

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
    /// Largeur de la vignette, déclarée pour éviter un fetch asynchrone de la plateforme.
    pub vignette_l: u32,
    /// Hauteur de la vignette.
    pub vignette_h: u32,
    /// Texte alternatif de la vignette.
    pub vignette_alt: String,
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
    /// Le contenu du catalogue, quand la route en désigne un et que le VFS est prêt.
    pub catalogue: Option<Catalogue>,
    /// Libellé du compte d'entrées, dans la langue de la page.
    pub libelle_total: String,
    /// Libellé de la page précédente.
    pub libelle_precedent: &'static str,
    /// Libellé de la page suivante.
    pub libelle_suivant: &'static str,
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
fn donnees_structurees(
    origine: &str,
    route: &str,
    langue: Langue,
    titre: &str,
    description: &str,
    catalogue: Option<&Catalogue>,
) -> String {
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
        // `ItemList` ne decrit que ce qui est REELLEMENT dans la page. En annoncer plus que le
        // document n'en porte est la facon la plus simple d'invalider tout le bloc.
        if let Some(c) = catalogue {
            let items: Vec<_> = c
                .elements
                .iter()
                .take(10)
                .enumerate()
                .map(|(i, e)| {
                    serde_json::json!({
                        "@type": "ListItem",
                        "position": i + 1,
                        "name": e.nom,
                        "url": format!("{origine}{}", e.href),
                    })
                })
                .collect();
            graphe.push(serde_json::json!({
                "@type": "ItemList",
                "name": titre,
                "numberOfItems": c.total,
                "itemListElement": items,
            }));
        }
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
    catalogue: Option<Catalogue>,
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
    let (libelle_precedent, libelle_suivant) = match langue {
        Langue::Fr => ("Page précédente", "Page suivante"),
        Langue::En => ("Previous page", "Next page"),
        Langue::Ja => ("前のページ", "次のページ"),
    };
    let libelle_total = catalogue.as_ref().map_or_else(String::new, |c| match langue {
        Langue::Fr => format!("{} fichiers · page {} sur {}", c.total, c.page, c.pages),
        Langue::En => format!("{} files · page {} of {}", c.total, c.page, c.pages),
        Langue::Ja => format!("{} 件 · {} / {} ページ", c.total, c.page, c.pages),
    });
    // La page courante fait partie de l'identite de l'URL : sans `?page=` au canonique, les
    // pages 2 et suivantes se declarent toutes copies de la premiere et disparaissent.
    let url = match catalogue.as_ref().map(|c| c.page) {
        Some(n) if n > 1 => format!("{}?page={n}", langue.url(origine, route)),
        _ => langue.url(origine, route),
    };
    Coquille {
        lang: langue.code(),
        jsonld: donnees_structurees(origine, route, langue, &titre, &description, catalogue.as_ref()),
        url,
        og_locale: langue.og_locale(),
        og_locales_alternes: Langue::TOUTES
            .iter()
            .filter(|l| **l != langue)
            .map(|l| l.og_locale())
            .collect(),
        alternatives: alternatives(origine, route),
        // Absolue : une plateforme sociale ne resout pas les URL relatives.
        image: Some(format!("{origine}{VIGNETTE}")),
        vignette_l: VIGNETTE_L,
        vignette_h: VIGNETTE_H,
        vignette_alt: titre.clone(),
        titre,
        description,
        type_og,
        route: route.to_owned(),
        feuille,
        script,
        couleur_theme: COULEUR_THEME,
        catalogues,
        langues,
        libelle_catalogues,
        libelle_langues,
        catalogue,
        libelle_total,
        libelle_precedent,
        libelle_suivant,
    }
}

/// Charge la page de catalogue que la route désigne, si elle en désigne une.
///
/// Rend `None` — et non une erreur — quand la route n'est pas un catalogue, ou quand l'index du
/// VFS n'est pas encore monté. Le montage prend des minutes sur 255 000 entrées : une page qui
/// refuserait de se rendre pendant ce temps serait pire que la même page sans sa liste, qui
/// garde son titre, sa navigation et ses liens.
fn charger_catalogue(
    etat: &EtatSite,
    route: &str,
    langue: Langue,
    page: usize,
) -> Option<Catalogue> {
    let segment = route.trim_matches('/');
    let vue = crate::vfs_index::Vue::depuis_segment(segment)?;
    let index = etat.index().ok()?;
    let total = index.compte_vue(vue);
    let pages = total.div_ceil(PAR_PAGE).max(1);
    let page = page.min(pages);
    let elements = index
        .page_vue(vue, (page - 1) * PAR_PAGE, PAR_PAGE)
        .into_iter()
        .map(|f| Element {
            href: format!("/f/{}", f.chemin),
            nom: f.nom,
            chemin: f.chemin,
            taille: taille_lisible(f.taille),
        })
        .collect();
    let lien = |n: usize| {
        let base = langue.url("", route);
        let base = if base.is_empty() { "/" } else { &base };
        if n == 1 { base.to_owned() } else { format!("{base}?page={n}") }
    };
    Some(Catalogue {
        elements,
        total,
        page,
        pages,
        precedent: (page > 1).then(|| lien(page - 1)),
        suivant: (page < pages).then(|| lien(page + 1)),
    })
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
    let catalogue = charger_catalogue(
        &etat,
        &demande.route,
        demande.langue,
        numero_de_page(uri.query()),
    );
    let page = construire(
        &etat.config.origine,
        &demande.route,
        demande.langue,
        feuille,
        script,
        catalogue,
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
        construire("https://aphrody.com", route, langue, None, None, None)
            .render()
            .expect("rendu")
    }

    /// Un catalogue synthétique : `n` entrées sur un total annoncé, à la page `page`.
    fn catalogue(n: usize, total: usize, page: usize) -> Catalogue {
        let pages = total.div_ceil(PAR_PAGE).max(1);
        Catalogue {
            elements: (0..n)
                .map(|i| Element {
                    href: format!("/f/data/dx11/chr/x{i:03}.g4tx"),
                    nom: format!("x{i:03}.g4tx"),
                    chemin: format!("data/dx11/chr/x{i:03}.g4tx"),
                    taille: taille_lisible(1024 * (i as u32 + 1)),
                })
                .collect(),
            total,
            page,
            pages,
            precedent: (page > 1).then(|| format!("/textures?page={}", page - 1)),
            suivant: (page < pages).then(|| format!("/textures?page={}", page + 1)),
        }
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
        // L'image n'est plus injectee par le test : elle doit etre la SANS qu'on la pose,
        // sinon on verifie une balise que la production n'emet pas.
        let c = construire("https://aphrody.com", "/", Langue::Fr, None, None, None);
        assert_eq!(c.image.as_deref(), Some("https://aphrody.com/static/og.png"));
        let html = c.render().expect("rendu");
        for balise in [
            "og:title",
            "og:description",
            "og:url",
            "og:image",
            "og:type",
            "og:site_name",
            "og:locale",
            "og:image:width",
            "og:image:height",
            "og:image:alt",
            "twitter:card",
            "twitter:image",
        ] {
            assert!(html.contains(balise), "balise {balise} absente");
        }
        assert!(html.contains(r#"content="summary_large_image""#));
        assert!(html.contains(r#"content="1200""#));
        assert!(html.contains(r#"content="630""#));
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
    fn la_largeur_compte_les_ideogrammes_pour_deux() {
        assert_eq!(largeur_affichage("abc"), 3);
        assert_eq!(largeur_affichage("テクスチャ"), 10);
        assert_eq!(largeur_affichage("モデル — Aphrody"), 6 + 1 + 1 + 1 + 7);
        // Un caractère latin pleine chasse compte double, comme un idéogramme.
        assert_eq!(largeur_affichage("Ａ"), 2);
        assert_eq!(largeur_affichage(""), 0);
    }

    #[test]
    fn aucun_titre_ni_description_ne_sera_tronque() {
        // La gate porte sur la LARGEUR, pas sur le nombre de caractères : sans cela un titre
        // japonais deux fois trop long passerait, et un titre français correct serait rejeté.
        let routes = ["/", "/textures", "/modeles", "/sons", "/videos"];
        for langue in Langue::TOUTES {
            for route in routes {
                let (titre, description, _) = metadonnees(route, langue);
                let lt = largeur_affichage(&titre);
                let ld = largeur_affichage(&description);
                assert!(
                    lt <= LARGEUR_TITRE,
                    "titre trop large en {langue} sur {route} : {lt} > {LARGEUR_TITRE} — {titre}"
                );
                assert!(
                    ld <= LARGEUR_DESCRIPTION,
                    "description trop large en {langue} sur {route} : {ld} > {LARGEUR_DESCRIPTION}"
                );
            }
        }
    }

    #[test]
    fn aucun_titre_ni_description_n_est_duplique() {
        // Deux pages qui portent le meme titre se font concurrence a elles-memes. Le compte
        // se mesure, il ne se relit pas a l'oeil (gate `wiki-azalee.md` §5).
        let routes = ["/", "/textures", "/modeles", "/sons", "/videos"];
        for langue in Langue::TOUTES {
            let titres: std::collections::BTreeSet<_> =
                routes.iter().map(|r| metadonnees(r, langue).0).collect();
            assert_eq!(titres.len(), routes.len(), "titres dupliqués en {langue}");
            let descriptions: std::collections::BTreeSet<_> =
                routes.iter().map(|r| metadonnees(r, langue).1).collect();
            assert_eq!(descriptions.len(), routes.len(), "descriptions dupliquées en {langue}");
        }
    }

    #[test]
    fn taille_lisible_change_d_unite_sans_mentir() {
        assert_eq!(taille_lisible(0), "0 o");
        assert_eq!(taille_lisible(1023), "1023 o");
        assert_eq!(taille_lisible(1024), "1.0 kio");
        assert_eq!(taille_lisible(3_498_240), "3.3 Mio");
        // Le plus gros fichier du jeu reste sous le gibioctet : l'unite suivante n'existe pas.
        assert_eq!(taille_lisible(u32::MAX), "4.0 Gio");
    }

    #[test]
    fn le_numero_de_page_tolere_ce_qu_on_lui_donne() {
        assert_eq!(numero_de_page(None), 1);
        assert_eq!(numero_de_page(Some("")), 1);
        assert_eq!(numero_de_page(Some("page=3")), 3);
        assert_eq!(numero_de_page(Some("q=x&page=12")), 12);
        // Une URL fabriquee n'est pas une erreur de l'utilisateur : la premiere page repond.
        assert_eq!(numero_de_page(Some("page=abc")), 1);
        assert_eq!(numero_de_page(Some("page=0")), 1);
        assert_eq!(numero_de_page(Some("page=-4")), 1);
    }

    #[test]
    fn le_catalogue_est_rendu_cote_serveur() {
        let c = construire(
            "https://aphrody.com",
            "/textures",
            Langue::Fr,
            None,
            None,
            Some(catalogue(60, 54_203, 1)),
        );
        let html = c.render().expect("rendu");
        // 60 entrees reellement dans le document : c'est ce que voit un robot qui n'execute rien.
        assert_eq!(html.matches("<li><a href=\"/f/").count(), 60);
        assert!(html.contains("54203 fichiers · page 1 sur 904"));
        assert!(html.contains(r#"<a href="/f/data/dx11/chr/x000.g4tx">x000.g4tx</a>"#));
        // La taille est lisible, pas un nombre d'octets nu.
        assert!(html.contains("1.0 kio"));
        // Premiere page : un suivant, pas de precedent.
        assert!(html.contains(r#"rel="next""#));
        assert!(!html.contains(r#"rel="prev""#));
    }

    #[test]
    fn la_pagination_se_declare_dans_le_canonique_et_dans_le_head() {
        let c = construire(
            "https://aphrody.com",
            "/textures",
            Langue::Ja,
            None,
            None,
            Some(catalogue(60, 54_203, 7)),
        );
        // Sans `?page=` au canonique, les pages 2 et suivantes se declarent copies de la
        // premiere, et disparaissent de l'index.
        assert_eq!(c.url, "https://aphrody.com/ja/textures?page=7");
        let html = c.render().expect("rendu");
        assert!(html.contains(r#"<link rel="prev" href="/textures?page=6">"#));
        assert!(html.contains(r#"<link rel="next" href="/textures?page=8">"#));
        assert!(html.contains(r#"<link rel="canonical" href="https://aphrody.com/ja/textures?page=7">"#));
        // Les libelles suivent la langue.
        assert!(html.contains("前のページ"));
        assert!(html.contains("54203 件 · 7 / 904 ページ"));
        // Le groupe hreflang reste celui de la ROUTE, sans le numero de page : les trois
        // langues d'une meme page se pointent, pas la page 7 francaise depuis la page 1.
        assert!(html.contains(r#"hreflang="fr" href="https://aphrody.com/textures""#));
    }

    #[test]
    fn la_derniere_page_n_annonce_pas_de_suivante() {
        let c = construire(
            "https://aphrody.com",
            "/textures",
            Langue::En,
            None,
            None,
            Some(catalogue(23, 143, 3)),
        );
        let html = c.render().expect("rendu");
        assert_eq!(html.matches("<li><a href=\"/f/").count(), 23);
        assert!(html.contains(r#"rel="prev""#));
        assert!(!html.contains(r#"rel="next""#));
        assert!(html.contains("143 files · page 3 of 3"));
    }

    #[test]
    fn l_itemlist_ne_decrit_que_ce_que_la_page_porte() {
        let c = construire(
            "https://aphrody.com",
            "/textures",
            Langue::Fr,
            None,
            None,
            Some(catalogue(60, 54_203, 1)),
        );
        let brut = c
            .jsonld
            .replace(r"\u003c", "<")
            .replace(r"\u003e", ">")
            .replace(r"\u0026", "&");
        let v: serde_json::Value = serde_json::from_str(&brut).expect("json-ld");
        let liste = v["@graph"]
            .as_array()
            .expect("graphe")
            .iter()
            .find(|n| n["@type"] == "ItemList")
            .expect("ItemList absent");
        assert_eq!(liste["numberOfItems"], 54_203);
        // 10 elements decrits, jamais les 60 : annoncer plus que le document ne porte invalide
        // le bloc entier.
        assert_eq!(liste["itemListElement"].as_array().expect("items").len(), 10);
        assert_eq!(liste["itemListElement"][0]["position"], 1);
        assert_eq!(
            liste["itemListElement"][0]["url"],
            "https://aphrody.com/f/data/dx11/chr/x000.g4tx"
        );
        // Une page sans catalogue n'en fabrique pas.
        let sans = construire("https://aphrody.com", "/", Langue::Fr, None, None, None);
        assert!(!sans.jsonld.contains("ItemList"));
    }

    #[test]
    fn les_donnees_structurees_sont_du_json_valide_et_inoffensif() {
        for langue in Langue::TOUTES {
            for route in ["/", "/textures"] {
                let c = construire("https://aphrody.com", route, langue, None, None, None);
                // Le `<` échappé doit se relire comme du JSON : sinon la page porte un bloc mort.
                let brut = c.jsonld.replace(r"\u003c", "<").replace(r"\u003e", ">").replace(r"\u0026", "&");
                let v: serde_json::Value = serde_json::from_str(&brut).expect("json-ld valide");
                assert_eq!(v["@context"], "https://schema.org");
                assert!(v["@graph"].as_array().is_some_and(|g| !g.is_empty()));
                // Aucun `<` ne subsiste : il refermerait le <script>.
                assert!(!c.jsonld.contains('<'), "jsonld non échappé en {langue}");
            }
        }
        let c = construire("https://aphrody.com", "/textures", Langue::Fr, None, None, None);
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
