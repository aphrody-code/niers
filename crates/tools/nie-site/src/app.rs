//! Le routeur et les couches communes.
//!
//! La `Content-Security-Policy` est posée **ici**, par la crate, et nulle part ailleurs : deux
//! CSP s'additionnent et la plus stricte gagne, donc le bloc nginx d'`aphrody.com` n'en pose
//! aucune (cf. `docs/stack/web-platform.md`). Un en-tête qui vient de deux endroits est un
//! en-tête que personne ne contrôle.

use std::time::Duration;

use axum::Router;
use axum::http::{HeaderValue, StatusCode, header};
use axum::routing::{get, post};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::state::EtatSite;

/// La politique de sécurité du contenu servie par Aphrody.
///
/// `img-src`/`media-src` acceptent `blob:` et `data:` parce que le site décode des textures et
/// de l'audio côté client à partir d'octets bruts ; `connect-src 'self'` suffit puisque `/f`,
/// `/api/v1` et `/assets` sont sur la même origine — aucune origine tierce n'est nécessaire.
pub const CSP: &str = "default-src 'self'; \
     script-src 'self' 'wasm-unsafe-eval'; \
     style-src 'self' 'unsafe-inline'; \
     img-src 'self' data: blob:; \
     media-src 'self' blob:; \
     font-src 'self' data:; \
     connect-src 'self'; \
     worker-src 'self' blob:; \
     object-src 'none'; \
     base-uri 'none'; \
     form-action 'none'; \
     frame-ancestors 'none'";

/// Nombre d'en-têtes de sécurité posés — compté par les tests.
pub const NB_ENTETES_SECURITE: usize = 5;

/// Les en-têtes de sécurité posés sur **toutes** les réponses, y compris les erreurs.
#[must_use]
pub fn entetes_securite_liste() -> [(header::HeaderName, &'static str); NB_ENTETES_SECURITE] {
    [
        (header::CONTENT_SECURITY_POLICY, CSP),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::X_FRAME_OPTIONS, "DENY"),
        (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
        (
            header::HeaderName::from_static("permissions-policy"),
            "geolocation=(), camera=(), microphone=(), payment=()",
        ),
    ]
}

/// Délai maximal d'une requête entrante, toutes routes confondues. Il est plus large que le
/// délai d'amont (10 s) pour que le `504` de l'amont arrive au client avant que la requête ne
/// soit coupée ici.
pub const DELAI_REQUETE: Duration = Duration::from_secs(15);

/// Déclare les routes **une seule fois**, et en tire deux sorties : le montage du routeur et la
/// liste de leurs chemins.
///
/// Ce qui l'a rendue nécessaire, mesuré : `ROUTES` était une constante tenue à la main que
/// `tests/routes.rs` figeait à 19 entrées, alors que le routeur en montait 37. Les sept routes
/// d'`/pet`, les cinq de la 3D puis les six de Lua et des formats y avaient été ajoutées sans
/// entrer dans la liste — chaque lot ayant respecté son périmètre, et la liste n'appartenant à
/// aucun. **Un inventaire qui ne suit pas ce qu'il inventorie n'est pas une garde, c'est un
/// faux document** : il annonçait 19 routes servies sur un site qui en sert 37.
///
/// La macro supprime la classe entière de défaut : une route ajoutée ici est montée **et**
/// listée, une route retirée disparaît des deux. Aucun ordre de déclaration à maintenir, aucune
/// discipline à demander au prochain lot.
macro_rules! declarer_routes {
    ($($chemin:literal => $handler:path),+ $(,)?) => {
        /// Les chemins réellement montés, dans l'ordre de déclaration.
        ///
        /// Cette liste **est** celle du routeur : elles descendent de la même déclaration, et
        /// aucune ne peut être modifiée sans l'autre.
        #[must_use]
        pub fn chemins() -> Vec<&'static str> {
            vec![$($chemin),+]
        }

        /// Monte les routes déclarées sur un routeur nu, toutes en `GET`.
        fn monter(routeur: Router<EtatSite>) -> Router<EtatSite> {
            routeur $(.route($chemin, get($handler)))+
        }
    };
}

/// Les chemins qui acceptent autre chose que `GET`, montés par [`routeur`] par-dessus la macro.
///
/// Elle est **écrite**, et c'est voulu : la lecture seule du site est une garantie, et une
/// garantie qu'on ne peut pas énumérer n'en est pas une. Un test fige cette liste — une entrée
/// y arrive par une décision visible, jamais par inadvertance.
///
/// **Aucune des trois n'écrit quoi que ce soit.** Le `POST` dit ici la **taille de l'entrée**,
/// jamais un effet de bord : deux personnages entiers, onze joueurs avec leurs statistiques, ou
/// des milliers d'identifiants d'effectif ne tiennent pas dans une query string. Chacune a un
/// `GET` de même chemin qui publie son contrat, plutôt que de rendre un `405` muet au premier
/// client qui explore.
pub const CHEMINS_HORS_GET: &[&str] = &[
    "/api/v1/regles/comparaison",
    "/api/v1/team/synergy",
    "/api/v1/save/roster",
    "/api/v1/inspect/compare",
    "/api/v1/inspect/plate",
];

// Le site ne prend **aucune écriture** : ni base, ni disque, ni état. C'est la garantie que la
// macro rend structurelle, et le défaut de la déclaration est `GET`.
//
// Cinq routes sortent du `GET`, et aucune n'écrit : elles CALCULENT sur un corps que la query
// string ne peut pas porter (deux personnages entiers, un effectif de onze joueurs, une liste
// d'identifiants de sauvegarde, deux images RGBA). Le verbe dit la taille de l'entrée, pas un
// effet de bord — et chacune a son pendant `GET`, qui publie le contrat plutôt que de rendre un
// `405` muet au premier client qui explore. Le test `seules_les_routes_declarees_sortent_du_get`
// fige la liste : une sixième y entrera par une décision visible, jamais par inadvertance.
declarer_routes! {
    "/healthz" => crate::routes::health::healthz,
    "/robots.txt" => crate::routes::well_known::robots,
    "/llms.txt" => crate::routes::well_known::llms,
    "/llms-full.txt" => crate::routes::well_known::llms_complet,
    // Une route par langue, declarees une par une. Un parametre `/{langue}/manifest…`
    // capturerait n'importe quel segment et servirait le manifeste francais sous autant
    // d'URL qu'on peut en inventer.
    "/manifest.webmanifest" => crate::routes::well_known::manifeste,
    "/en/manifest.webmanifest" => crate::routes::well_known::manifeste,
    "/ja/manifest.webmanifest" => crate::routes::well_known::manifeste,
    "/.well-known/security.txt" => crate::routes::well_known::security,
    "/sitemap.xml" => crate::routes::well_known::sitemap,
    "/feed.atom" => crate::routes::feed::atom,
    "/api/v1/health" => crate::routes::api_v1::health,
    "/api/v1/chara" => crate::routes::api_v1::chara,
    "/api/v1/{vue}" => crate::routes::api_v1::vue,
    "/f/{*chemin}" => crate::routes::vfs::fichier,
    "/b" => crate::routes::vfs::parcours_racine,
    "/b/{*prefixe}" => crate::routes::vfs::parcours,
    "/api/v1/episodes" => crate::routes::episodes::episodes,
    "/assets/{*chemin}" => crate::routes::assets::assets,
    // Aphrody, le personnage du site. Sept routes explicites plutot qu'un
    // `/pet/{*fichier}` : le package n'est pas un dossier de fichiers, et un joker
    // inviterait a en deriver un espace qui n'existe pas. Cf. `routes::aphrody`.
    "/pet/aphrody.json" => crate::routes::aphrody::manifeste,
    "/pet/atlas.webp" => crate::routes::aphrody::atlas,
    "/pet/aphrody.svg" => crate::routes::aphrody::svg,
    "/pet/frame/{animation}/{fichier}" => crate::routes::aphrody::frame,
    "/api/v1/aphrody" => crate::routes::aphrody::dossier,
    "/api/v1/aphrody/diagnostic" => crate::routes::aphrody::diagnostic,
    "/api/v1/aphrody/palette" => crate::routes::aphrody::palette,
    // La couche 3D. Cinq routes, deux espaces : `/api/v1/3d` DECRIT (capacites, catalogue,
    // fiche, geometrie mesuree), `/model` SERT (le GLB assemble, l'apercu rendu). Un
    // catalogue qui rendrait aussi les octets melangerait deux durees de cache et deux
    // politiques d'erreur — un catalogue absent est un 503, un modele absent un 404.
    //
    // `/api/v1/3d` est declare AVANT `/api/v1/{vue}` : matchit prefere de toute facon le
    // segment litteral au parametre, mais l'ordre de lecture doit dire la meme chose que
    // l'ordre de resolution.
    "/api/v1/3d" => crate::routes::modeles3d::capacites,
    "/api/v1/3d/modeles" => crate::routes::modeles3d::catalogue,
    "/api/v1/3d/modeles/{famille}/{code}" => crate::routes::modeles3d::fiche,
    "/api/v1/3d/modeles/{famille}/{code}/analyse" => crate::routes::modeles3d::analyse,
    "/model/{famille}/{fichier}" => crate::routes::modeles3d::modele,
    // La couche Lua et la couche formats. Elles sont declarees AVANT `/api/v1/{vue}`
    // pour la meme raison que `/api/v1/3d` : matchit prefere de toute facon le segment
    // litteral au parametre, mais l'ordre de lecture doit dire ce que fait le routeur.
    //
    // Le desassemblage a son PROPRE prefixe au lieu d'etre un suffixe de `/scripts` : un
    // joker (`{*chemin}`) est terminal chez axum, et `/scripts/{*chemin}/desassemblage`
    // ne se declare pas. Cf. `routes::lua`.
    "/api/v1/lua" => crate::routes::lua::capacites,
    "/api/v1/lua/scripts" => crate::routes::lua::scripts,
    "/api/v1/lua/scripts/{*chemin}" => crate::routes::lua::script,
    "/api/v1/lua/desassemblage/{*chemin}" => crate::routes::lua::desassemblage,
    // L'arbre de navigation est construit par `nie-model-serve` depuis les vrais
    // `_setting.cfg.bin`. Le site le relaie sous son API, par le même proxy borné que les
    // assets, sans recopier ni réimplémenter le catalogue.
    "/api/v1/menu/screens" => crate::routes::menu::screens,
    "/api/v1/menu/screens/{stem}" => crate::routes::menu::screen,
    "/api/v1/formats" => crate::routes::formats::capacites,
    "/api/v1/formats/decode/{*chemin}" => crate::routes::formats::decode,
    // Les modules de `nie-formats` que le decodage generique n'atteint pas : ils prennent une
    // structure DEJA lue (un atlas, un canvas RGBA, deux images a comparer) et n'ont donc rien
    // a voir avec `decode/{chemin}`, qui part des octets d'un fichier. Sept inspecteurs, tous
    // sous `std` seul — `images`/`textures` restent ETEINTES, et `routes::formats` continue de
    // le dire. Cf. `routes::inspect`.
    "/api/v1/inspect" => crate::routes::inspect::catalog,
    "/api/v1/inspect/spritesheet/{*path}" => crate::routes::inspect::spritesheet,
    "/api/v1/inspect/font/{*path}" => crate::routes::inspect::font_metrics,
    "/api/v1/inspect/menu/{*path}" => crate::routes::inspect::menu,
    "/api/v1/inspect/texture-chunk/{*path}" => crate::routes::inspect::texture_chunk,
    "/api/v1/inspect/color" => crate::routes::inspect::color,
    "/api/v1/inspect/compare" => crate::routes::inspect::compare_contract,
    "/api/v1/inspect/plate" => crate::routes::inspect::plate_contract,
    // Chercher un fichier dans TOUT le VFS. `/b` ne filtre qu'un niveau — verifie :
    // `/b/data?q=chara_base` rend 0. Cf. `routes::recherche`.
    "/api/v1/recherche" => crate::routes::recherche::recherche,
    // Les donnees du jeu, en structures NOMMEES. Distincte de `/formats/decode`, qui rend la
    // structure generique du conteneur : un consommateur typé qui lit le générique y trouve
    // zero element en annoncant un succes. Cf. `routes::donnees`.
    "/api/v1/donnees" => crate::routes::donnees::capacites,
    // Declarees AVANT le joker : `{*chemin}` est terminal chez axum, et un chemin VFS reel
    // commence toujours par `data/` — aucune ambiguite, mais l'ordre de lecture doit dire ce
    // que fait le routeur.
    "/api/v1/donnees/familles" => crate::routes::donnees::familles,
    "/api/v1/donnees/famille/{cle}" => crate::routes::donnees::famille,
    "/api/v1/donnees/{*chemin}" => crate::routes::donnees::donnees,
    // Trois vues que la facade `decode_by_key(cle, root)` ne peut PAS exprimer, et qui
    // restaient donc `manquant` dans la matrice alors que leur parseur etait ecrit et teste :
    //
    // - `passives` joint CINQ fichiers (trois de donnees, deux de texte x trois langues) —
    //   `parse_player_passives` prend trois tables de texte en plus du conteneur ;
    // - `playstyle` lit le MEME fichier que `chara_param`, dont la cle est deja prise : une
    //   facade ne peut pas rendre deux structures pour une cle ;
    // - `cond`/`unlock_condition` prennent une CHAINE base64, pas un conteneur.
    //
    // Aucune des trois n'etait un manque de code. C'etait un manque d'adresse.
    "/api/v1/passives" => crate::routes::passives::catalog,
    "/api/v1/passives/{kind}" => crate::routes::passives::kind,
    "/api/v1/playstyles" => crate::routes::playstyles::catalog,
    "/api/v1/playstyles/{id}" => crate::routes::playstyles::playstyle,
    "/api/v1/conditions" => crate::routes::conditions::capabilities,
    "/api/v1/conditions/{blob}" => crate::routes::conditions::condition,
    // Deux capacites de la CLI que le web n'exposait pas. `nie-cli` n'a PAS de cible `[lib]`
    // (que `[[bin]] name = "niers"`) : rien n'y est importable, et la logique est donc reecrite
    // ici contre `nie-formats`/`nie-lua`, sans une feature de plus. Cf. `routes::screens`.
    "/api/v1/icons" => crate::routes::screens::icons,
    "/api/v1/icons/{name}" => crate::routes::screens::icon,
    "/api/v1/modes" => crate::routes::screens::modes,
    "/api/v1/modes/{slug}" => crate::routes::screens::mode,
    // La couverture des ECRANS — condition 4 du § 8 du cap. Le total est mesure sur le VFS,
    // jamais cite : le plan a deja ecrit 440 la ou la mesure en rend 475. Un ecran n'est
    // `served` que si TOUS ses calques resolvent vers un `.objbin` present, definition choisie
    // pour pouvoir echouer. Cf. `routes::screens`.
    "/api/v1/screens" => crate::routes::screens::screens,
    // Declaree AVANT `{screen}` pour la lisibilite ; matchit fait gagner le segment statique
    // de toute facon. Elle enumere les calques que le jeu DECLARE et ne LIVRE PAS — c'est ce
    // qui empeche « 36 % » de passer pour un reste-a-faire.
    "/api/v1/screens/missing" => crate::routes::screens::missing_layers,
    "/api/v1/screens/{screen}" => crate::routes::screens::screen,
    // Le texte localise du jeu, adresse par LANGUE et par FAMILLE. `/api/v1/donnees/{chemin}`
    // le sert deja, mais seulement a qui connait le chemin VFS exact AVEC son numero de version
    // (`menu_text_1.03.98.00.cfg.bin`) : inutilisable pour un consommateur. Neuf langues,
    // 980 fichiers, 247 familles, 643 168 lignes. Cf. `routes::text`.
    //
    // URLs en ANGLAIS (regle du 2026-09-06, cf. CLAUDE.md § Language) : ce sont elles qu'un
    // consommateur etranger lit. `/search` a UN segment, `{language}/{family}` en a DEUX :
    // aucune ambiguite pour matchit.
    "/api/v1/text" => crate::routes::text::catalog,
    "/api/v1/text/search" => crate::routes::text::search,
    "/api/v1/text/{language}/{family}" => crate::routes::text::family,
    "/api/v1/text/{language}/{family}/{hash}" => crate::routes::text::line,
    // La traduction s'appuie sur ce qui aligne REELLEMENT deux langues dans les fichiers du
    // jeu : le hash. Elle ne devine rien, la ou l'outil equivalent d'Azalee interroge sept
    // tables avec un score flou. Un segment, comme `/search` : aucune ambiguite avec
    // `{language}/{family}`, qui en a deux.
    "/api/v1/text/translate" => crate::routes::text::translate,
    // Les 219 tables du miroir, en lecture generique — 165 249 lignes. Ce qu'elle remplace est
    // mesure (`docs/inagle/05-service-et-types.md`) : 71 acces directs `from("inagle_…")` ecrits
    // a la main dans les pages du wiki, et 28 methodes de facade que plus rien n'appelle.
    //
    // Aucun nom venu du client n'entre dans une requete : il sert a RETROUVER une entree du
    // catalogue relu sur `sqlite_master` a chaque appel, et c'est le nom de la BASE qui est
    // ecrit. Cf. `routes::entites`.
    "/api/v1/entites" => crate::routes::entites::catalogue,
    "/api/v1/entites/{table}" => crate::routes::entites::lignes,
    "/api/v1/entites/{table}/{id}" => crate::routes::entites::ligne,
    // Les regles de JEU, calculees par le moteur — pas lues dans une base. C'est la logique
    // portee d'inagle vers `nie-core` : croissance, comparaison, rarete, builds.
    //
    // `/comparaison` est la seule route du site a accepter autre chose qu'un `GET`, et elle
    // n'ecrit rien : deux personnages entiers ne tiennent pas dans une query string. Son `GET`
    // publie le contrat au lieu de rendre un `405` muet.
    "/api/v1/regles" => crate::routes::regles::capacites,
    "/api/v1/regles/stats" => crate::routes::regles::stats,
    "/api/v1/regles/comparaison" => crate::routes::regles::contrat_comparaison,
    "/api/v1/regles/rarete" => crate::routes::regles::rarete,
    "/api/v1/regles/builds" => crate::routes::regles::builds,
    // L'autre moitie de `nie_core::optimisation`, celle que `routes::regles` disait « pas
    // routee ici » : elle prend un effectif complet, donc un corps de requete. Espace de noms
    // NEUF et entierement anglais — `/api/v1/regles/*` est deja servi en francais et ne se
    // renomme pas au fil de l'eau, mais un nouveau nom est anglais sans exception
    // (CLAUDE.md § Language, 2026-09-06). Cf. `routes::team`.
    "/api/v1/team/synergy" => crate::routes::team::contract,
    // Resoudre en LOT les identifiants d'un effectif. Ce qui traverse le reseau n'est jamais
    // une sauvegarde — `nie-save` tourne en wasm chez le joueur — mais une liste de codes du
    // jeu. Cf. `routes::save`.
    "/api/v1/save/roster" => crate::routes::save::contract,
    // La matrice de couverture du plan (§ 4). Elle est LUE, jamais mesuree ici : mesurer,
    // c'est lancer `niers --help`, lire quatre arbres de sources et parcourir 255 308 lignes
    // d'inventaire. Cf. `routes::couverture`.
    "/couverture" => crate::routes::couverture::page,
    "/api/v1/couverture" => crate::routes::couverture::json,
    "/" => crate::routes::pages::coquille,
}

/// Dit si un motif de route d'axum reconnaît une URI concrète.
///
/// Elle vit **ici** et non dans les tests parce que la matrice de couverture en dépend : une
/// règle de classement cite la route qui sert une capacité, et [`crate::couverture::construire`]
/// rétrograde en `manquant` toute capacité dont la route n'est montée nulle part. Sans cette
/// fonction, la matrice se croirait sur parole — et une matrice qu'on ne peut pas contredire
/// n'est pas un instrument de mesure.
///
/// Elle reproduit la règle de `matchit` telle qu'axum 0.8 l'emploie : un segment `{param}`
/// consomme exactement un segment non vide, un `{*joker}` consomme tout le reste (au moins un
/// segment), et tout autre segment doit être égal.
#[must_use]
pub fn correspond(motif: &str, uri: &str) -> bool {
    let mut segments_uri = uri.trim_start_matches('/').split('/');
    let segments_motif: Vec<&str> = motif.trim_start_matches('/').split('/').collect();
    for m in &segments_motif {
        if m.starts_with("{*") {
            return segments_uri.next().is_some();
        }
        match segments_uri.next() {
            None => return false,
            Some(s) => {
                if m.starts_with('{') {
                    if s.is_empty() {
                        return false;
                    }
                } else if *m != s {
                    return false;
                }
            }
        }
    }
    segments_uri.next().is_none()
}

/// Construit le routeur complet : les routes déclarées ci-dessus, le repli statique et les
/// couches.
///
/// Syntaxe de route d'axum 0.8 : `{param}` et `{*wildcard}`. L'ancienne forme (`:id`, `*path`)
/// **panique** au `route()` — elle ne dégrade pas.
pub fn routeur(etat: EtatSite) -> Router {
    monter(Router::new())
        // Les cinq routes du site qui acceptent autre chose qu'un `GET` — et aucune n'écrit.
        // Deux personnages entiers, un effectif de onze joueurs, une liste d'identifiants de
        // sauvegarde, deux images : le verbe dit ici la taille de l'entrée, pas un effet de bord. Le `GET`
        // de chaque chemin, déclaré dans la macro, publie le contrat au lieu de rendre un
        // `405` muet au premier client qui explore ; axum fusionne les deux méthodes sur un
        // chemin déjà monté, si bien que `chemins()` continue de les compter une fois.
        .route(
            CHEMINS_HORS_GET[0],
            post(crate::routes::regles::comparaison),
        )
        .route(CHEMINS_HORS_GET[1], post(crate::routes::team::synergy))
        .route(CHEMINS_HORS_GET[2], post(crate::routes::save::roster))
        // Les deux inspecteurs qui prennent des PIXELS en entrée : `imgmetric::comparer` reçoit
        // deux images RGBA, `planche::mesurer` en reçoit une. Aucune query string ne les porte,
        // et ni l'une ni l'autre n'écrit quoi que ce soit.
        .route(CHEMINS_HORS_GET[3], post(crate::routes::inspect::compare))
        .route(CHEMINS_HORS_GET[4], post(crate::routes::inspect::plate))
        .fallback(crate::routes::static_files::statique)
        // Les couches s'empilent de la plus INTERNE à la plus externe, et l'ordre est ici un
        // choix, pas une habitude :
        //
        // - l'ETag est au plus près des routes, seul endroit d'où l'on voie le corps final ;
        // - la borne de débit est AU-DESSUS, pour qu'un client refusé ne fasse ni requête SQL
        //   ni condensé — un limiteur qui laisse d'abord travailler ne limite que la bande
        //   passante ;
        // - les en-têtes de sécurité l'enveloppent, pour qu'un `429` les porte aussi ;
        // - le délai maximal et la trace restent les plus externes, faute de quoi ils ne
        //   verraient ni les réponses des couches ci-dessus ni leur latence.
        .layer(axum::middleware::from_fn(crate::etag::conditionnel))
        .layer(axum::middleware::from_fn_with_state(
            etat.clone(),
            crate::debit::limiter,
        ))
        .layer(axum::middleware::from_fn(entetes_securite))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            DELAI_REQUETE,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(etat)
}

/// Pose les en-têtes de sécurité sur la réponse, sans jamais écraser un en-tête déjà posé par
/// une route (une route peut avoir une raison de durcir davantage, jamais d'assouplir).
async fn entetes_securite(
    requete: axum::extract::Request,
    suite: axum::middleware::Next,
) -> axum::response::Response {
    let mut reponse = suite.run(requete).await;
    let entetes = reponse.headers_mut();
    for (nom, valeur) in entetes_securite_liste() {
        if !entetes.contains_key(&nom) {
            entetes.insert(nom, HeaderValue::from_static(valeur));
        }
    }
    reponse
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csp_complete_et_stricte() {
        let directives: Vec<&str> = CSP.split(';').map(str::trim).collect();
        assert_eq!(directives.len(), 12, "douze directives, comptees");
        for attendue in [
            "default-src 'self'",
            "object-src 'none'",
            "base-uri 'none'",
            "form-action 'none'",
            "frame-ancestors 'none'",
        ] {
            assert!(
                directives.contains(&attendue),
                "directive absente: {attendue}"
            );
        }
        assert!(
            !CSP.contains(" 'unsafe-eval'"),
            "pas d'eval JavaScript (wasm excepte)"
        );
        assert!(
            !CSP.contains("script-src 'self' 'unsafe-inline'"),
            "pas de script inline"
        );
        assert_eq!(entetes_securite_liste().len(), NB_ENTETES_SECURITE);
    }

    #[test]
    fn correspondance_de_motif() {
        assert!(correspond("/f/{*chemin}", "/f/data/common/x.g4tx"));
        assert!(!correspond("/f/{*chemin}", "/f"));
        assert!(correspond("/api/v1/{vue}", "/api/v1/textures"));
        assert!(!correspond("/api/v1/{vue}", "/api/v1/textures/1"));
        assert!(correspond("/healthz", "/healthz"));
        assert!(!correspond("/healthz", "/healthz/x"));
        // Un motif ne reconnait pas un segment vide : `/api/v1/` n'est pas `/api/v1/{vue}`.
        assert!(!correspond("/api/v1/{vue}", "/api/v1/"));
    }

    #[test]
    fn seules_les_routes_declarees_sortent_du_get() {
        // La lecture seule du site est une GARANTIE, pas une habitude : ce test l'énumère.
        // Une route qui accepterait un `POST` sans passer par `CHEMINS_HORS_GET` ne serait
        // visible nulle part — c'est exactement le défaut que `ROUTES` figé à 19 avait déjà
        // coûté à cette crate.
        assert_eq!(
            CHEMINS_HORS_GET,
            [
                "/api/v1/regles/comparaison",
                "/api/v1/team/synergy",
                "/api/v1/save/roster",
                "/api/v1/inspect/compare",
                "/api/v1/inspect/plate",
            ],
            "cinq routes sortent du GET, et les cinq CALCULENT : aucune n'écrit ni base ni disque"
        );
        // Et son chemin est bien déclaré par la macro : sans cela, `chemins()` ne le compterait
        // pas et la matrice de couverture rétrograderait la capacité qu'il sert.
        for chemin in CHEMINS_HORS_GET {
            assert!(
                chemins().contains(chemin),
                "{chemin} accepte un POST mais n'est pas declare en GET"
            );
        }
    }

    #[test]
    fn contrat_de_routes() {
        let routes = chemins();
        assert_eq!(routes.len(), 82, "82 routes montees");
        for r in &routes {
            assert!(r.starts_with('/'), "{r}");
            // Syntaxe axum 0.7 (`:id`, `*path`) : elle PANIQUE au `route()`, elle ne degrade
            // pas. Un test la refuse ici plutot qu'au demarrage du service.
            assert!(!r.contains(":{"), "syntaxe axum 0.7 interdite: {r}");
            assert!(!r.contains("/:"), "syntaxe axum 0.7 interdite: {r}");
        }
        // Aucun doublon : deux fois le meme chemin, et c'est la seconde declaration qui
        // gagnerait en silence.
        let mut tries = routes.clone();
        tries.sort_unstable();
        tries.dedup();
        assert_eq!(tries.len(), routes.len(), "chemin declare deux fois");
    }
}
