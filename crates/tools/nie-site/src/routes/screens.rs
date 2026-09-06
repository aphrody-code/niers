//! `/api/v1/icons` et `/api/v1/modes` — l'**index des icônes** du jeu et le **contenu d'un mode**.
//!
//! # Pourquoi ces deux routes existent
//!
//! Deux capacités écrites, testées et servies par aucune URL : `niers icons` (l'index des
//! icônes) et `niers mode` (l'agrégation d'un mode de jeu). Toutes deux vivent dans
//! `crates/tools/nie-cli`, qui n'a **pas de cible `[lib]`** — ses modules sont `mod x;` privés
//! et rien n'y est importable. La logique est donc **réécrite ici**, à partir de deux sources
//! lues et non recopiées :
//!
//! - `crates/tools/nie-cli/src/icons_cmd.rs:101` (`indexer`) → [`build_index`] ;
//! - `crates/tools/nie-cli/src/mode_index.rs:68` (`MODES`) et `:302` (`collect`) → [`MODES`]
//!   et [`collect`].
//!
//! Aucune variante qui **écrit sur le disque** n'est portée (`icons extract`, `icons dict`,
//! `mode index`, `mode export`) : un service web en lecture seule n'écrit pas, et `extract`
//! exigerait en plus la feature `textures`, éteinte ici par décision de crate.
//!
//! # Ce que ces routes ne promettent pas
//!
//! - **Elles ne décodent aucune image.** L'index dit *où* est une icône (atlas, rectangle,
//!   taille) et *sous quelle URL* la demander ; le PNG vient de `nie-model-serve` par le proxy
//!   `/assets/tex/…`. Les deux formes d'URL publiées ont été vérifiées en HTTP réel contre
//!   l'amont le 2026-09-06 (cf. [`region_url`] et [`atlas_url`]).
//! - **Elles ne lisent pas `nie.exe`.** `mode_index::contenu_json` accepte un chemin
//!   d'exécutable pour retrouver les clés de message d'un mode dans les chaînes du binaire ;
//!   le site passe `None` et le **dit** ([`ModeMessages`]) plutôt que de rendre un objet vide
//!   qui passerait pour « rien à signaler ».
//! - **Elles ne nomment pas les commandes funcLua.** `nie_lua::menu_host::command_name` est
//!   derrière la feature `vm` de `nie-lua`, volontairement éteinte dans cette crate (aucun
//!   interpréteur Lua n'est lié dans ce service, cf. `routes::lua`). Un `cmdId` sort donc avec
//!   son handler, jamais avec un nom deviné.
//!
//! # Le piège de la table funcLua, et comment il est traité ici
//!
//! `mode_index::charger_handlers_funclua` (`crates/tools/nie-cli/src/mode_index.rs:423`)
//! remonte l'arborescence depuis `std::env::current_dir()` pour trouver
//! `data/re/funclua-cmdid-handlers.json`. Dans une unité systemd, le répertoire courant n'est
//! pas le dépôt : la remontée échoue, la fonction rend une table vide, et l'analyse Lua rend
//! **zéro commande** en annonçant un succès. Le fichier est de surcroît gitignoré (dump de
//! reverse, © LEVEL-5) : il est légitimement absent d'un clone neuf.
//!
//! Ici : **une seule** résolution, déterministe — `resolve_game_dir()`, la même que tout le
//! dépôt — et le résultat est **publié** ([`FuncluaTable`]) avec le chemin tenté, le nombre
//! d'entrées et la raison de l'absence. Une réponse dont `funclua.available` vaut `false`
//! annonce ses `commands` vides ; elle ne les fait pas passer pour une mesure.
//!
//! # Nommage
//!
//! Règle du 2026-09-06 : identifiants, URLs et clés JSON en **anglais**, prose en français.
//! Cf. `CLAUDE.md` § *Langue*.
//!
//! # Les routes
//!
//! | Route | Ce qu'elle rend |
//! |---|---|
//! | `GET /api/v1/icons` | l'index nom → atlas, rectangle, taille, URLs — paginé, `?q=`, `?prefix=` |
//! | `GET /api/v1/icons/{name}` | une icône par son nom |
//! | `GET /api/v1/modes` | le catalogue des modes — slug, libellé, préfixes, officiel |
//! | `GET /api/v1/modes/{slug}` | le contenu mesuré d'un mode, avec ses comptes |

use std::collections::BTreeMap;
use std::sync::OnceLock;

use axum::Json;
use axum::extract::{Path, Query, State};
use nie_formats::cfgbin::{self, CfgEntry, Value};
use nie_formats::objbin;
use nie_formats::vfs::Vfs;
use nie_lua::bytecode;
use serde::{Deserialize, Serialize};

use crate::error::ErreurSite;
use crate::routes::{DemandePage, Page};
use crate::state::EtatSite;
use crate::vfs_index::{IndexVfs, Requete};

// ═══ Icônes ══════════════════════════════════════════════════════════════════

/// Fragment de chemin VFS sous lequel vivent les atlas d'icônes.
///
/// Comparé par `contains` et non par `starts_with` : les atlas sont sous `data/dx11/menu/…`
/// sur un montage packs comme sur un dump, et préfixer en dur `data/dx11/` ferait dépendre
/// l'index du montage. Mesuré le 2026-09-06 (`niers vfs find 'menu/' -n 300000`) : 41 191
/// `.g4tx` sous `menu/`, dont **19 534** sous `menu/200_icon/`.
pub const ICONS_ROOT: &str = "menu/200_icon/";

/// Suffixe des conteneurs indexés.
pub const ICONS_SUFFIX: &str = ".g4tx";

/// Les familles d'atlas volontairement **hors** de l'index, avec leur raison.
///
/// Ce n'est pas une commodité : mesuré le 2026-09-06, ces deux familles pèsent **19 322** des
/// 19 534 atlas de [`ICONS_ROOT`]. Sans elles l'index se construit sur **212** conteneurs
/// (1,9 s mesurées, 3 770 icônes) ; avec elles, le service décoderait 19 000 fichiers au
/// premier appel pour un contenu déjà adressable atlas par atlas sous `/assets/tex/`.
///
/// Le nombre d'atlas réellement écartés est **compté** à la construction et publié
/// ([`IconCatalog::skipped_families`]) — il n'est pas écrit ici.
pub const SKIPPED_FAMILIES: [(&str, &str); 2] = [
    (
        "10_icon_chr",
        "un atlas par personnage : le portrait se demande directement sous /assets/tex/",
    ),
    (
        "01_icon_emblem",
        "un atlas par emblème d'équipe : même adressage direct",
    ),
];

/// Rectangle d'une région dans son atlas, en pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Rect {
    /// Abscisse du coin haut-gauche.
    pub x: i16,
    /// Ordonnée du coin haut-gauche.
    pub y: i16,
    /// Largeur.
    pub w: i16,
    /// Hauteur.
    pub h: i16,
}

/// Une icône localisée dans un atlas.
///
/// Portage de `struct Icone` (`crates/tools/nie-cli/src/icons_cmd.rs:88`), avec deux
/// différences assumées : les champs sont en anglais, et l'URL du CLI (`/tex/…`, qui vise
/// `nie-model-serve` en direct) devient les **deux** URLs du proxy du site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Icon {
    /// Nom de l'icône — c'est la clé, et c'est aussi le nom de région que l'amont sait rogner.
    pub name: String,
    /// Chemin VFS de l'atlas qui la porte.
    pub atlas: String,
    /// Nom de la texture porteuse. Égal à `name` quand l'icône est une texture entière.
    pub texture: String,
    /// Rectangle de la région, `None` quand l'icône **est** la texture entière.
    pub rect: Option<Rect>,
    /// Largeur rendue, en pixels.
    pub width: i32,
    /// Hauteur rendue, en pixels.
    pub height: i32,
    /// URL de l'**icône** : l'amont rogne la région désignée par son nom.
    pub url: String,
    /// URL de l'**atlas entier** — utile pour inspecter, jamais pour afficher une icône.
    pub atlas_url: String,
}

/// URL de l'icône elle-même : `/assets/tex/<atlas sans `data/`>/<nom>.png`.
///
/// **Le `.g4tx` reste dans le chemin** : c'est lui qui fait basculer l'amont sur sa route de
/// région. Mesuré en HTTP réel contre `nie-model-serve` le 2026-09-06 —
/// `…/icon_deco.g4tx/ds01001.png` rend **200, 21 604 octets** quand
/// `…/icon_deco.png` (l'atlas) en rend **1 746 102**. Servir l'atlas à la place de l'icône ne
/// casserait rien de visible : ce serait juste 80× le poids, et la mauvaise image.
#[must_use]
pub fn region_url(atlas: &str, name: &str) -> String {
    format!("/assets/tex/{}/{name}.png", strip_data(atlas))
}

/// URL de l'atlas entier : `/assets/tex/<atlas sans `data/` et **sans** `.g4tx`>.png`.
///
/// Le retrait du suffixe n'est pas cosmétique : mesuré le 2026-09-06, `…/icon_item01.g4tx.png`
/// rend **404** là où `…/icon_item01.png` rend **200**.
#[must_use]
pub fn atlas_url(atlas: &str) -> String {
    let sans = strip_data(atlas);
    format!(
        "/assets/tex/{}.png",
        sans.strip_suffix(ICONS_SUFFIX).unwrap_or(sans)
    )
}

/// Retire le préfixe `data/` d'un chemin VFS : l'amont l'ajoute lui-même.
fn strip_data(path: &str) -> &str {
    path.strip_prefix("data/").unwrap_or(path)
}

/// L'index d'icônes tel qu'il est construit, une fois, puis gardé pour la vie du processus.
#[derive(Debug, Default)]
pub struct IconIndex {
    /// Nom → icône, trié.
    icons: BTreeMap<String, Icon>,
    /// Nombre d'atlas réellement lus et parsés.
    atlases: usize,
    /// Nombre d'atlas écartés par famille (clé = famille de [`SKIPPED_FAMILIES`]).
    skipped: BTreeMap<&'static str, usize>,
    /// Nombre d'atlas retenus mais illisibles ou non parsables.
    unreadable: usize,
    /// Durée de la construction, en millisecondes — mesurée, pas affirmée.
    elapsed_ms: u128,
}

impl IconIndex {
    /// Nombre d'icônes indexées.
    #[must_use]
    pub fn len(&self) -> usize {
        self.icons.len()
    }

    /// `true` si l'index est vide — ce qui, sur un jeu installé, est un symptôme.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }

    /// Une icône par son nom.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Icon> {
        self.icons.get(name)
    }
}

/// L'index, calculé à la première demande.
static ICONS: OnceLock<IconIndex> = OnceLock::new();

/// Vrai si ce chemin est un atlas que l'index retient ; sinon la famille qui l'écarte.
///
/// Rend `Ok(())` pour un candidat retenu, `Err(Some(famille))` pour un écart délibéré, et
/// `Err(None)` pour un chemin qui n'est simplement pas un atlas d'icône.
fn candidate(path: &str) -> Result<(), Option<&'static str>> {
    if !path.ends_with(ICONS_SUFFIX) || !path.contains(ICONS_ROOT) {
        return Err(None);
    }
    for (family, _) in SKIPPED_FAMILIES {
        if path.contains(family) {
            return Err(Some(family));
        }
    }
    Ok(())
}

/// Une texture ou une région est-elle une vraie icône ?
///
/// Les `dmy` sont les bouche-trous du jeu, et une image de 4×4 ou moins n'est pas une icône.
/// Même règle que `icons_cmd::indexer` — reproduite, pas devinée.
fn is_real_icon(name: &str, width: i32, height: i32) -> bool {
    !name.contains("dmy") && !(width <= 4 && height <= 4)
}

/// Construit l'index en lisant et parsant réellement chaque atlas retenu.
///
/// Portage de `icons_cmd::indexer` (`crates/tools/nie-cli/src/icons_cmd.rs:101`). Deux écarts :
/// l'énumération passe par [`IndexVfs`] (déjà construit au montage) au lieu de `Vfs::iter`, et
/// les écarts sont **comptés** au lieu d'être perdus.
fn build_index(index: &IndexVfs, vfs: &Vfs) -> IconIndex {
    let start = std::time::Instant::now();
    let (files, _) = index.page_filtree(None, &Requete::default());
    let mut out = IconIndex::default();

    let mut paths: Vec<String> = Vec::new();
    for f in files {
        match candidate(&f.chemin) {
            Ok(()) => paths.push(f.chemin),
            Err(Some(family)) => *out.skipped.entry(family).or_default() += 1,
            Err(None) => {}
        }
    }

    for path in paths {
        let Ok(raw) = vfs.read(&path) else {
            out.unreadable += 1;
            continue;
        };
        let Ok(tx) = nie_formats::g4tx::parse(&raw) else {
            out.unreadable += 1;
            continue;
        };
        out.atlases += 1;
        for tex in &tx.textures {
            if !is_real_icon(&tex.name, tex.width, tex.height) {
                continue;
            }
            // `or_insert` : le premier atlas qui porte un nom le garde. C'est la règle du CLI,
            // et la changer réordonnerait l'index à chaque mise à jour du jeu.
            out.icons.entry(tex.name.clone()).or_insert_with(|| Icon {
                name: tex.name.clone(),
                atlas: path.clone(),
                texture: tex.name.clone(),
                rect: None,
                width: tex.width,
                height: tex.height,
                url: region_url(&path, &tex.name),
                atlas_url: atlas_url(&path),
            });
            for sub in &tex.sub_textures {
                if !is_real_icon(&sub.name, i32::from(sub.width), i32::from(sub.height)) {
                    continue;
                }
                out.icons.entry(sub.name.clone()).or_insert_with(|| Icon {
                    name: sub.name.clone(),
                    atlas: path.clone(),
                    texture: tex.name.clone(),
                    rect: Some(Rect {
                        x: sub.x,
                        y: sub.y,
                        w: sub.width,
                        h: sub.height,
                    }),
                    width: i32::from(sub.width),
                    height: i32::from(sub.height),
                    url: region_url(&path, &sub.name),
                    atlas_url: atlas_url(&path),
                });
            }
        }
    }

    out.elapsed_ms = start.elapsed().as_millis();
    tracing::info!(
        icons = out.icons.len(),
        atlases = out.atlases,
        unreadable = out.unreadable,
        ms = out.elapsed_ms,
        "index d'icones construit"
    );
    out
}

/// Rend l'index, en le construisant à la première demande.
///
/// # Errors
///
/// `Indisponible` (503) tant que le VFS n'est pas monté, ou quand le montage n'a pas de
/// contenu : c'est la capacité qui manque, pas la route.
async fn icon_index(state: &EtatSite) -> Result<&'static IconIndex, ErreurSite> {
    if let Some(i) = ICONS.get() {
        return Ok(i);
    }
    let index = state.index()?;
    let vfs = state.vfs()?;
    let built =
        tokio::task::spawn_blocking(move || ICONS.get_or_init(|| build_index(&index, &vfs))).await?;
    Ok(built)
}

/// Ce que `/api/v1/icons` accepte.
///
/// **Champs à plat, jamais `#[serde(flatten)]`** : avec lui, la désérialisation d'une query
/// passe par un tampon où toute valeur est une chaîne et `?per_page=2` échoue en
/// « invalid type: string "2", expected u32 ». Piège déjà payé sur `routes::recherche` et
/// `routes::text`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct IconQuery {
    /// Numéro de page, à partir de 1.
    pub page: Option<u32>,
    /// Nombre d'éléments par page, plafonné à [`crate::config::PER_PAGE_MAX`].
    pub per_page: Option<u32>,
    /// Motif cherché **dans le nom de l'icône**, sans casse.
    pub q: Option<String>,
    /// Fragment de chemin VFS qui borne le sous-arbre (`20_icon_deco`, `200_icon/02_`).
    pub prefix: Option<String>,
}

/// Une famille d'atlas écartée, avec son compte mesuré.
#[derive(Debug, Clone, Serialize)]
pub struct SkippedFamily {
    /// Nom du sous-dossier écarté.
    pub family: &'static str,
    /// Pourquoi il l'est.
    pub reason: &'static str,
    /// Nombre d'atlas réellement écartés — **compté** à la construction.
    pub atlases: usize,
}

/// L'index d'icônes tel qu'il est servi.
#[derive(Debug, Clone, Serialize)]
pub struct IconCatalog {
    /// Le fragment de chemin appliqué, s'il y en avait un.
    pub prefix: Option<String>,
    /// Le motif appliqué, normalisé en minuscules, s'il y en avait un.
    pub q: Option<String>,
    /// Nombre d'icônes indexées, **avant** filtrage.
    pub total_indexed: usize,
    /// Nombre d'atlas lus et parsés.
    pub atlases: usize,
    /// Nombre d'atlas retenus mais illisibles.
    pub unreadable_atlases: usize,
    /// Les familles écartées et leur compte.
    pub skipped_families: Vec<SkippedFamily>,
    /// Durée de la construction initiale de l'index, en millisecondes.
    pub index_ms: u128,
    /// Le fragment de chemin sous lequel l'index est construit.
    pub root: &'static str,
    /// La route qui rend une icône seule.
    pub icon_route: &'static str,
    /// La page.
    pub results: Page<Icon>,
}

/// Une icône est-elle retenue par le couple (`prefix`, `q`) ?
///
/// Séparée du handler pour être **falsifiable sans VFS ni HTTP** : c'est cette fonction que le
/// test `q_reduit_reellement_la_liste` casse volontairement. Un paramètre accepté puis ignoré
/// est le défaut n° 1 que ce dépôt traque, et il ne se prouve qu'en montrant la réduction.
fn retained(icon: &Icon, prefix: Option<&str>, pattern: Option<&str>) -> bool {
    prefix.is_none_or(|p| icon.atlas.contains(p))
        && pattern.is_none_or(|q| icon.name.to_lowercase().contains(q))
}

/// Normalise un motif de query : vide ou blanc ⇒ aucun filtre.
fn clean(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
}

/// Valide un fragment de chemin.
///
/// # Errors
///
/// `Demande` (400) sur un fragment qui remonte (`..`) ou qui commence par `/` : le premier
/// n'aurait aucun sens sur un chemin VFS, le second ne pourrait jamais correspondre. Les
/// refuser vaut mieux que rendre une liste vide qui se lirait « il n'y a rien ici ».
fn check_prefix(raw: Option<&str>) -> Result<Option<String>, ErreurSite> {
    let Some(p) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if p.contains("..") || p.starts_with('/') {
        return Err(ErreurSite::Demande(format!(
            "prefixe invalide `{p}` : attendu un fragment de chemin VFS \
             (`20_icon_deco`, `200_icon/02_`), sans `..` ni slash initial"
        )));
    }
    Ok(Some(p.to_owned()))
}

/// `GET /api/v1/icons` — l'index des icônes, paginé, filtré par `?q=` et borné par `?prefix=`.
///
/// # Errors
///
/// `400` sur un `prefix` invalide, `503` tant que le VFS n'est pas monté.
pub async fn icons(
    State(state): State<EtatSite>,
    Query(query): Query<IconQuery>,
) -> Result<Json<IconCatalog>, ErreurSite> {
    // L'ordre compte : une demande fautive se refuse AVANT de payer la construction de
    // l'index, sinon le client corrige la mauvaise chose.
    let prefix = check_prefix(query.prefix.as_deref())?;
    let pattern = clean(query.q.as_deref());
    let index = icon_index(&state).await?;

    let kept: Vec<&Icon> = index
        .icons
        .values()
        .filter(|i| retained(i, prefix.as_deref(), pattern.as_deref()))
        .collect();

    let bounds = DemandePage {
        page: query.page,
        per_page: query.per_page,
        q: None,
    }
    .bornee();
    let total = kept.len();
    let page: Vec<Icon> = kept
        .into_iter()
        .skip(bounds.offset())
        .take(bounds.per_page as usize)
        .cloned()
        .collect();

    Ok(Json(IconCatalog {
        prefix,
        q: pattern,
        total_indexed: index.len(),
        atlases: index.atlases,
        unreadable_atlases: index.unreadable,
        skipped_families: SKIPPED_FAMILIES
            .into_iter()
            .map(|(family, reason)| SkippedFamily {
                family,
                reason,
                atlases: index.skipped.get(family).copied().unwrap_or_default(),
            })
            .collect(),
        index_ms: index.elapsed_ms,
        root: ICONS_ROOT,
        icon_route: "/api/v1/icons/{name}",
        results: Page::nouvelle(page, bounds, total),
    }))
}

/// `GET /api/v1/icons/{name}` — une icône par son nom.
///
/// # Errors
///
/// `404` si le nom n'est pas indexé, `503` tant que le VFS n'est pas monté.
pub async fn icon(
    State(state): State<EtatSite>,
    Path(name): Path<String>,
) -> Result<Json<Icon>, ErreurSite> {
    let index = icon_index(&state).await?;
    index.get(&name).cloned().map(Json).ok_or_else(|| {
        ErreurSite::Introuvable(format!(
            "aucune icone nommee `{name}` parmi les {} indexees sous `{ICONS_ROOT}` ; \
             /api/v1/icons?q= cherche par sous-chaine",
            index.len()
        ))
    })
}

// ═══ Modes de jeu ════════════════════════════════════════════════════════════

/// Définition éditoriale d'un mode.
///
/// Portage de `mode_index::ModeDef` (`crates/tools/nie-cli/src/mode_index.rs:31`), sans les
/// champs que ce service ne peut pas honorer.
#[derive(Debug, Clone, Copy)]
pub struct ModeDef {
    /// Identifiant stable, utilisable en URL (`victory-road`).
    pub slug: &'static str,
    /// Nom de repli, si le jeu ne fournit pas de libellé pour ce mode.
    pub label: &'static str,
    /// Préfixes de noms d'écran et de script qui appartiennent à ce mode.
    pub prefixes: &'static [&'static str],
    /// Région de l'atlas `mode_base01_atl` qui porte l'icône, quand elle est identifiée.
    pub icon_region: Option<&'static str>,
    /// Hash `menu_text` du libellé officiel — le nom que le JEU affiche.
    pub label_hash: Option<u32>,
    /// `true` quand le jeu énumère lui-même ce mode dans ses réglages audio.
    pub official: bool,
    /// Ce que les fichiers permettent d'affirmer sur l'état du mode.
    pub note: &'static str,
    /// Sous-chaîne qui identifie, dans les chaînes de `nie.exe`, les clés de message du mode.
    /// Publiée, jamais résolue ici : le site ne lit pas le binaire (cf. [`ModeMessages`]).
    pub key_pattern: Option<&'static str>,
}

/// Les modes, chacun adossé à des écrans réels du VFS.
///
/// Portage de `mode_index::MODES` (`crates/tools/nie-cli/src/mode_index.rs:68`). Les cinq
/// modes marqués `official` ne sont **pas** un choix éditorial : le jeu les énumère lui-même
/// dans `menu_text`, via trois familles de réglages concordantes (volume BGM, volume des voix,
/// affichage de la liste de puissance). Les autres entrées sont des écrans utilitaires du menu
/// principal, utiles à cataloguer, mais que le jeu ne compte pas parmi ses modes.
///
/// `icon_region` n'est renseignée que pour les tuiles identifiées **visuellement** sur une
/// capture du menu ; les autres restent `None` plutôt que devinées.
pub const MODES: &[ModeDef] = &[
    ModeDef {
        slug: "victory-road",
        label: "Victory Road",
        prefixes: &[
            "victory_road",
            "victory_load",
            "victory_lode",
            "fake_vroad",
            "vroad_",
            "fade_menu_encount_victory_road",
        ],
        icon_region: Some("mode_base04"),
        label_hash: Some(0x80cd_176b),
        official: true,
        note: "Tournoi en ligne en trois phases (inscription, qualifications, classement \
               final). Ses assets vivent sous `menu/75_vroad/` et ses 28 ecrans couvrent \
               entree, tournoi final, classement, recompenses, region, photo et \
               notifications. Les ecrans `fake_vroad_*` sont des MAQUETTES posees sous \
               soccer99_*. `VictoryRoad` est l'orthographe canonique cote code ; \
               `victory_load`, `victory_lode` et `vroad` ne sont que des variantes cote \
               assets — aucune regle de prefixe ne les relierait, d'ou cette liste curatee.",
        key_pattern: Some("vroad"),
    },
    ModeDef {
        slug: "competition",
        label: "Mode Competition",
        prefixes: &[],
        icon_region: None,
        label_hash: Some(0x6e14_cca7),
        official: true,
        note: "Nomme par `menu_text`, mais AUCUN ecran ne porte ce nom dans le VFS et le \
               binaire n'a pas de cle de reglage a son nom. Comme les modes en ligne \
               (`lobby`, `ranked`, `bot_match`, tous absents), son contenu n'est pas dans les \
               fichiers installes : cette entree rend donc des comptes nuls, et c'est la \
               mesure, pas un defaut.",
        key_pattern: None,
    },
    ModeDef {
        slug: "story",
        label: "Histoire",
        prefixes: &["story_mode"],
        icon_region: None,
        label_hash: Some(0x76db_0fff),
        official: true,
        note: "Ecran story_mode_top_menu.",
        key_pattern: Some("story_mode"),
    },
    ModeDef {
        slug: "chronicle",
        label: "Mode Chronique",
        prefixes: &["chronicle_mode"],
        icon_region: Some("mode_base07"),
        label_hash: Some(0xce37_875a),
        official: true,
        note: "Ecrans chronicle_mode_top_menu et chronicle_mode_soccer_vs_menu ; images \
               dediees sous 220_img/ev_chronicle_img.",
        key_pattern: Some("chronicle"),
    },
    ModeDef {
        slug: "kizuna-station",
        label: "Station Kizuna",
        prefixes: &["kizuna_town"],
        icon_region: None,
        label_hash: Some(0x126c_915e),
        official: true,
        note: "Le MODE s'appelle « Station Kizuna » ; le LIEU qu'il ouvre est « Ville \
               Kizuna » (EN Bond Town), un libelle distinct. Ses ecrans portent le prefixe \
               kizuna_town.",
        key_pattern: Some("kizuna"),
    },
    ModeDef {
        slug: "chara-edit",
        label: "Editeur d'avatar",
        prefixes: &["chara_edit"],
        icon_region: None,
        label_hash: None,
        official: false,
        note: "Editeur de personnage joueur. 42 ecrans `chara_edit_*_setting` et 51 scripts \
               `chara_edit_*.lua` ; interface sous `menu/161_avatar/`, modeles et textures de \
               parts sous `chr/_face/20_EDIT/`. Aucun libelle de mode ne lui est attribue \
               dans `menu_text` : ce n'est pas une tuile du menu principal mais un editeur \
               ouvert depuis un autre mode, d'ou `official: false`.",
        key_pattern: Some("chara_edit"),
    },
    ModeDef {
        slug: "soccer",
        label: "Match",
        prefixes: &["soccer_top_menu", "soccer_game_mode"],
        icon_region: Some("mode_base03"),
        label_hash: Some(0x848d_75db),
        official: false,
        note: "Entree des matchs (crampons + ballon sur la tuile). Le jeu ne le compte pas \
               parmi les modes de ses reglages audio.",
        key_pattern: None,
    },
    ModeDef {
        slug: "bb-stadium",
        label: "BB Stadium",
        prefixes: &["bb_stadium"],
        icon_region: Some("mode_base10"),
        label_hash: None,
        official: false,
        note: "Tuile au logo `BB`.",
        key_pattern: Some("bb_stadium"),
    },
    ModeDef {
        slug: "play-guide",
        label: "Guide de jeu",
        prefixes: &["play_guide"],
        icon_region: Some("mode_base05"),
        label_hash: None,
        official: false,
        note: "Tuile au livre marque d'un point d'exclamation.",
        key_pattern: Some("play_guide"),
    },
    ModeDef {
        slug: "setting",
        label: "Parametres",
        prefixes: &["setting_top_menu"],
        icon_region: Some("mode_base06"),
        label_hash: Some(0x82c9_a2b3),
        official: false,
        note: "Tuile a l'engrenage.",
        key_pattern: None,
    },
    ModeDef {
        slug: "information",
        label: "Informations",
        prefixes: &["information_top_menu", "information_"],
        icon_region: Some("mode_base09"),
        label_hash: Some(0x1796_88e8),
        official: false,
        note: "Tuile au `i`.",
        key_pattern: None,
    },
    ModeDef {
        slug: "team-dock",
        label: "Equipe",
        prefixes: &["team_dock"],
        icon_region: None,
        label_hash: Some(0x7aae_281e),
        official: false,
        note: "Ecran commun de gestion d'equipe.",
        key_pattern: Some("team_dock"),
    },
];

/// Préfixe VFS des écrans de menu (`*_setting.cfg.bin`).
pub const SCREENS_ROOT: &str = "data/common/gamedata/menu/cfg/";

/// Suffixe des écrans de menu.
pub const SCREEN_SUFFIX: &str = "_setting.cfg.bin";

/// Préfixe VFS des objets de menu (`.objbin`).
pub const OBJECTS_ROOT: &str = "data/common/gamedata/menu/obj/";

/// Suffixe des objets de menu.
pub const OBJECT_SUFFIX: &str = ".objbin";

/// Fragment de chemin des scripts Lua du jeu.
pub const SCRIPTS_ROOT: &str = "/script/lua/";

/// Suffixe des scripts Lua compilés.
pub const SCRIPT_SUFFIX: &str = ".lua.bin";

/// Chemin, relatif à la racine du jeu, de la table `cmdId → handler` du dump de reverse.
pub const FUNCLUA_HANDLERS: &str = "data/re/funclua-cmdid-handlers.json";

/// Les chemins de menu, extraits **une fois** de l'index VFS.
///
/// `mode_index::collect` re-balaie les 255 308 entrées du VFS à chaque appel ; sur une route
/// HTTP, ce serait un balayage complet par requête. Les trois listes sont donc dérivées une
/// seule fois de [`IndexVfs`], qui est déjà construit au montage.
#[derive(Debug, Default)]
struct MenuPaths {
    /// Chemins des écrans `*_setting.cfg.bin`.
    screens: Vec<String>,
    /// Stem d'objet de menu → chemin VFS.
    objects: BTreeMap<String, String>,
    /// Chemins des scripts `.lua.bin`, avec leur stem et sa racine sans version.
    scripts: Vec<(String, String, String)>,
    /// Durée de l'extraction, en millisecondes.
    elapsed_ms: u128,
}

/// Les chemins de menu, calculés une fois.
static MENU_PATHS: OnceLock<MenuPaths> = OnceLock::new();

/// Feuille d'un chemin VFS, privée d'un suffixe. `None` si le suffixe n'y est pas.
fn stem(path: &str, suffix: &str) -> Option<String> {
    path.rsplit('/')
        .next()
        .and_then(|f| f.strip_suffix(suffix))
        .map(str::to_owned)
}

/// Racine d'un nom de script, privée de son suffixe de version (`_1.02.92.00`).
///
/// Sans elle, `main_menu_1.02.92.00` échapperait à tous les préfixes — même règle que
/// `mode_index::collect`.
fn versionless(s: &str) -> String {
    s.split_once(char::is_numeric)
        .map_or(s, |(a, _)| a.trim_end_matches('_'))
        .to_owned()
}

/// Extrait les trois listes de chemins de l'index VFS.
fn build_menu_paths(index: &IndexVfs) -> MenuPaths {
    let start = std::time::Instant::now();
    let (files, _) = index.page_filtree(None, &Requete::default());
    let mut out = MenuPaths::default();
    for f in files {
        let p = f.chemin;
        if p.starts_with(SCREENS_ROOT) && p.ends_with(SCREEN_SUFFIX) {
            out.screens.push(p);
        } else if p.starts_with(OBJECTS_ROOT) && p.ends_with(OBJECT_SUFFIX) {
            if let Some(s) = stem(&p, OBJECT_SUFFIX) {
                out.objects.insert(s, p);
            }
        } else if p.contains(SCRIPTS_ROOT)
            && p.ends_with(SCRIPT_SUFFIX)
            && let Some(s) = stem(&p, SCRIPT_SUFFIX)
        {
            let base = versionless(&s);
            out.scripts.push((p, s, base));
        }
    }
    out.elapsed_ms = start.elapsed().as_millis();
    tracing::info!(
        screens = out.screens.len(),
        objects = out.objects.len(),
        scripts = out.scripts.len(),
        ms = out.elapsed_ms,
        "chemins de menu extraits"
    );
    out
}

/// Rend les chemins de menu, en les extrayant à la première demande.
///
/// # Errors
///
/// `Indisponible` (503) tant que l'index VFS n'est pas prêt.
async fn menu_paths(state: &EtatSite) -> Result<&'static MenuPaths, ErreurSite> {
    if let Some(m) = MENU_PATHS.get() {
        return Ok(m);
    }
    let index = state.index()?;
    let built =
        tokio::task::spawn_blocking(move || MENU_PATHS.get_or_init(|| build_menu_paths(&index)))
            .await?;
    Ok(built)
}

// ── La table funcLua, et son absence assumée ─────────────────────────────────

/// La table `cmdId → handler`, avec **la raison** de son éventuelle absence.
///
/// C'est la correction du piège décrit en tête de module : le CLI dégrade en silence, cette
/// route publie son diagnostic.
#[derive(Debug, Clone, Serialize)]
pub struct FuncluaTable {
    /// `true` quand la table a été lue.
    pub available: bool,
    /// Le chemin **effectivement** tenté — une seule résolution, déterministe.
    pub path: String,
    /// Nombre d'entrées lues.
    pub entries: usize,
    /// Pourquoi la table manque, quand elle manque.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Ce que l'absence retire à la réponse.
    pub effect: &'static str,
    /// Pourquoi les commandes ne portent pas de nom, même table présente.
    pub naming: &'static str,
}

/// La table chargée, plus son diagnostic.
#[derive(Debug)]
struct Funclua {
    /// `cmdId` → adresse virtuelle du handler.
    table: BTreeMap<u32, u64>,
    /// Le chemin tenté.
    path: String,
    /// La raison de l'absence, ou `None` quand la table est là.
    reason: Option<String>,
}

/// La table, lue une fois.
static FUNCLUA: OnceLock<Funclua> = OnceLock::new();

/// Charge la table `cmdId → handler`, **sans remontée d'arborescence**.
///
/// Une seule résolution : `resolve_game_dir()` — la même que tout le dépôt, qui honore
/// `NIE_GAME_DIR` — suivie de [`FUNCLUA_HANDLERS`]. La remontée depuis `current_dir()` du CLI
/// est délibérément **non** portée : dans une unité systemd elle échoue et rend une table vide
/// que rien ne distingue d'un jeu sans commandes.
fn load_funclua() -> Funclua {
    let path = nie_formats::vfs::resolve_game_dir().join(FUNCLUA_HANDLERS);
    let shown = path.display().to_string();
    let texte = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            return Funclua {
                table: BTreeMap::new(),
                path: shown,
                reason: Some(format!(
                    "table absente ou illisible ({}) ; elle se regenere par \
                     `uv run scripts/extract_funclua_table.py` et reste gitignoree (dump de \
                     reverse)",
                    e.kind()
                )),
            };
        }
    };
    let brut: BTreeMap<String, String> = match serde_json::from_str(&texte) {
        Ok(b) => b,
        Err(e) => {
            return Funclua {
                table: BTreeMap::new(),
                path: shown,
                reason: Some(format!(
                    "table presente mais illisible : attendu un objet {{\"0x…\": \"0x…\"}} \
                     ({e})"
                )),
            };
        }
    };
    let mut table = BTreeMap::new();
    for (k, v) in brut {
        let (Some(id), Some(va)) = (parse_hex32(&k), parse_hex64(&v)) else {
            continue;
        };
        table.insert(id, va);
    }
    let reason = table
        .is_empty()
        .then(|| "table lue mais vide : aucune paire `0x… → 0x…` exploitable".to_owned());
    Funclua {
        table,
        path: shown,
        reason,
    }
}

/// Analyse un entier 32 bits préfixé `0x`.
fn parse_hex32(s: &str) -> Option<u32> {
    s.strip_prefix("0x")
        .and_then(|h| u32::from_str_radix(h, 16).ok())
}

/// Analyse une adresse virtuelle 64 bits préfixée `0x`.
fn parse_hex64(s: &str) -> Option<u64> {
    s.strip_prefix("0x")
        .and_then(|h| u64::from_str_radix(h, 16).ok())
}

/// Le diagnostic de la table, tel qu'il est publié.
fn funclua_report(f: &Funclua) -> FuncluaTable {
    FuncluaTable {
        available: !f.table.is_empty(),
        path: f.path.clone(),
        entries: f.table.len(),
        reason: f.reason.clone(),
        effect: "sans elle, `commands` est vide pour tous les scripts : un entier du pool de \
                 constantes ne devient un cmdId que s'il figure dans cette table",
        naming: "les commandes sortent sans nom : `nie_lua::menu_host::command_name` est \
                 derriere la feature `vm`, eteinte dans ce service pour qu'aucun interpreteur \
                 Lua n'y soit lie",
    }
}

// ── Agrégation d'un mode ─────────────────────────────────────────────────────

/// Vrai si `stem` relève d'un des préfixes du mode.
fn matches(def: &ModeDef, stem: &str) -> bool {
    def.prefixes.iter().any(|p| stem.starts_with(p))
}

/// Première chaîne non vide des variables d'une entrée T2B.
fn first_string(e: &CfgEntry) -> Option<&str> {
    e.variables.iter().find_map(|v| match v {
        Value::String(s) if !s.is_empty() => Some(s.as_str()),
        _ => None,
    })
}

/// Parcourt un arbre d'entrées T2B en profondeur.
fn walk<'a>(entries: &'a [CfgEntry], f: &mut impl FnMut(&'a CfgEntry)) {
    for e in entries {
        f(e);
        walk(&e.children, f);
    }
}

/// Nom RTTI d'un composant de menu.
fn component_type_name(c: &objbin::MenuComponent) -> &str {
    use objbin::MenuComponent as M;
    match c {
        M::Render(x) => &x.type_name,
        M::Animation(x) => &x.type_name,
        M::Text(x) => &x.type_name,
        M::Primitive(x) => &x.type_name,
        M::AttachLocator(x) => &x.type_name,
        M::Collision(x) => &x.type_name,
        M::SoundCmd(x) => &x.type_name,
        M::MeshVisible(x) => &x.type_name,
        M::Unknown(x) => &x.type_name,
    }
}

/// Un écran du mode, avec ses calques dans **l'ordre du fichier**.
#[derive(Debug, Clone, Serialize)]
pub struct Screen {
    /// Nom de l'écran (le stem, sans `_setting.cfg.bin`).
    pub screen: String,
    /// Chemin VFS de son `.cfg.bin`.
    pub cfg: String,
    /// Taille du conteneur, en octets.
    pub bytes: usize,
    /// Ses calques, dans l'ordre où le fichier les déclare — ce qu'un ensemble trié perdrait.
    pub layers: Vec<String>,
    /// Nombre d'éléments focusables déclarés.
    pub focus: usize,
}

/// Un type de composant et son nombre d'occurrences.
#[derive(Debug, Clone, Serialize)]
pub struct ComponentCount {
    /// Nom RTTI du composant (`CMenuRenderComponent`, `MenuTextSetting`…).
    pub type_name: String,
    /// Nombre d'occurrences, tous objets du mode confondus.
    pub count: usize,
}

/// Une commande `funcLuaMenuCommand` qu'un script est structurellement capable d'émettre.
#[derive(Debug, Clone, Serialize)]
pub struct Command {
    /// `cmdId`, en hexadécimal sur huit chiffres.
    pub cmd_id: String,
    /// Adresse virtuelle du handler dans `nie.exe`, en hexadécimal.
    pub handler: String,
}

/// Un script Lua du mode, analysé sans être exécuté.
#[derive(Debug, Clone, Serialize)]
pub struct Script {
    /// Chemin VFS du `.lua.bin`.
    pub path: String,
    /// Taille du conteneur, en octets.
    pub bytes: usize,
    /// Nombre d'instructions, tous prototypes confondus.
    pub instructions: usize,
    /// Nombre de fonctions (prototype principal compris).
    pub functions: usize,
    /// Modules `INCLUDE`d, reconnus par le préfixe `LUA_` du pool de constantes.
    pub includes: Vec<String>,
    /// Les commandes reconnues. **Vide quand la table funcLua manque** — cf. [`FuncluaTable`].
    pub commands: Vec<Command>,
    /// Pourquoi le script n'a pas pu être analysé, le cas échéant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Ce que le site ne peut pas dire des messages d'un mode, et pourquoi.
///
/// `mode_index::messages_du_mode` retrouve les clés de message d'un mode en balayant les
/// chaînes ASCII et UTF-16 de `nie.exe`, puis les résout dans les tables localisées. Le site
/// **ne lit pas le binaire** : il n'y a pas de `nie.exe` dans le périmètre d'un serveur web, et
/// aller le chercher ferait dépendre une réponse HTTP d'un fichier de 33 Mio hors VFS.
#[derive(Debug, Clone, Serialize)]
pub struct ModeMessages {
    /// Toujours `false` ici — publié plutôt que sous-entendu par un objet vide.
    pub available: bool,
    /// La sous-chaîne qui identifierait les clés du mode dans `nie.exe`, quand il y en a une.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_pattern: Option<&'static str>,
    /// Pourquoi la résolution n'a pas lieu.
    pub reason: &'static str,
    /// Où la même donnée se lit, elle.
    pub alternative: &'static str,
}

/// Le contenu mesuré d'un mode.
#[derive(Debug, Clone, Serialize)]
pub struct ModeContent {
    /// Identifiant du mode.
    pub slug: &'static str,
    /// Libellé de repli.
    pub label: &'static str,
    /// `true` quand le jeu énumère lui-même ce mode.
    pub official: bool,
    /// Les préfixes qui définissent son périmètre — la réponse dit ce qu'elle a cherché.
    pub prefixes: &'static [&'static str],
    /// Ses écrans, dans l'ordre des noms.
    pub screens: Vec<Screen>,
    /// Ses calques, dédoublonnés et triés.
    pub layers: Vec<String>,
    /// Les `.objbin` résolus depuis les calques.
    pub objbins: Vec<String>,
    /// Les `.g4pkm` référencés par ces objets.
    pub g4pkm: Vec<String>,
    /// Les `.g4tx` référencés (chemin `SETUP` ou paramètre de composant).
    pub g4tx: Vec<String>,
    /// Les types de composants rencontrés, avec leur compte.
    pub components: Vec<ComponentCount>,
    /// Ses scripts Lua.
    pub scripts: Vec<Script>,
    /// Les comptes, rassemblés.
    pub counts: ModeCounts,
    /// L'état de la table funcLua au moment de la réponse.
    pub funclua: FuncluaTable,
    /// Ce que le site ne dit pas des messages du mode.
    pub messages: ModeMessages,
    /// Durée de l'agrégation, en millisecondes — mesurée à chaque appel.
    pub elapsed_ms: u128,
}

/// Les comptes d'un mode, rassemblés en un objet plutôt qu'éparpillés.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct ModeCounts {
    /// Nombre d'écrans.
    pub screens: usize,
    /// Nombre de calques distincts.
    pub layers: usize,
    /// Nombre d'objets de menu.
    pub objbins: usize,
    /// Nombre de `.g4pkm` référencés.
    pub g4pkm: usize,
    /// Nombre de `.g4tx` référencés.
    pub g4tx: usize,
    /// Nombre de types de composants distincts.
    pub component_types: usize,
    /// Nombre total de composants.
    pub components: usize,
    /// Nombre de scripts Lua.
    pub scripts: usize,
    /// Somme des éléments focusables de tous les écrans.
    pub focus: usize,
    /// Nombre de slots de texte `(objet, slot, hash)` distincts.
    pub text_slots: usize,
    /// Nombre d'objets, textures ou scripts illisibles sur ce montage.
    pub unreadable: usize,
}

/// Ce que l'agrégation trouve pour un mode, avant mise en forme.
#[derive(Debug, Default)]
pub struct ModeFacts {
    /// Les écrans, dans l'ordre des noms.
    pub screens: Vec<Screen>,
    /// Les calques distincts.
    pub layers: std::collections::BTreeSet<String>,
    /// Les objets de menu résolus.
    pub objbins: std::collections::BTreeSet<String>,
    /// Les `.g4pkm` référencés.
    pub g4pkm: std::collections::BTreeSet<String>,
    /// Les `.g4tx` référencés.
    pub g4tx: std::collections::BTreeSet<String>,
    /// Type de composant → nombre d'occurrences.
    pub components: BTreeMap<String, usize>,
    /// Les scripts Lua analysés.
    pub scripts: Vec<Script>,
    /// Slots de texte `(objet, slot, hash)`.
    pub text_slots: std::collections::BTreeSet<(String, String, u32)>,
    /// Somme des focusables.
    pub focus: usize,
    /// Fichiers illisibles rencontrés.
    pub unreadable: usize,
}

/// Récolte les faits d'un mode depuis le VFS.
///
/// Portage de `mode_index::collect` (`crates/tools/nie-cli/src/mode_index.rs:302`) fusionné
/// avec la partie « écrans » de `contenu_json` (`:548`) : le CLI relit chaque `.cfg.bin` une
/// seconde fois pour retrouver l'ordre des calques, ce qui double les lectures VFS. Ici les
/// deux passes n'en font qu'une.
///
/// Un fichier illisible **individuellement** n'est pas une erreur — il est compté
/// ([`ModeCounts::unreadable`]) et absent du résultat.
fn collect(vfs: &Vfs, paths: &MenuPaths, def: &ModeDef, funclua: &Funclua) -> ModeFacts {
    let mut facts = ModeFacts::default();

    for path in &paths.screens {
        let Some(name) = stem(path, SCREEN_SUFFIX) else {
            continue;
        };
        if !matches(def, &name) {
            continue;
        }
        let Ok(bytes) = vfs.read(path) else {
            facts.unreadable += 1;
            continue;
        };
        let Ok(file) = cfgbin::parse_t2b(&bytes) else {
            facts.unreadable += 1;
            continue;
        };
        let (mut layers, mut focus) = (Vec::new(), 0usize);
        walk(&file.entries, &mut |e: &CfgEntry| {
            if e.name.contains("LIST_BEG") || e.name.contains("LIST_END") {
                return;
            }
            if e.name.starts_with("MENU_LAYER_INFO") {
                if let Some(n) = first_string(e) {
                    layers.push(n.to_owned());
                }
            } else if e.name.starts_with("MENU_FOCUS_BASE_INFO") {
                focus += 1;
            }
        });
        facts.focus += focus;
        facts.layers.extend(layers.iter().cloned());
        facts.screens.push(Screen {
            screen: name,
            cfg: path.clone(),
            bytes: bytes.len(),
            layers,
            focus,
        });
    }
    facts.screens.sort_by(|a, b| a.screen.cmp(&b.screen));

    // Calque → objbin (même stem) → assets et composants.
    for layer in facts.layers.clone() {
        let Some(p) = paths.objects.get(&layer) else {
            continue;
        };
        facts.objbins.insert(p.clone());
        let Ok(bytes) = vfs.read(p) else {
            facts.unreadable += 1;
            continue;
        };
        let Ok(obj) = objbin::parse(&bytes) else {
            facts.unreadable += 1;
            continue;
        };
        if let Some(g) = &obj.g4pkm_path {
            facts.g4pkm.insert(g.clone());
        }
        if let Some(t) = &obj.g4tx_path {
            facts.g4tx.insert(t.clone());
        }
        for c in &obj.components {
            *facts
                .components
                .entry(component_type_name(c).to_owned())
                .or_default() += 1;
            match c {
                // Un composant non reconnu expose ses chaînes : c'est là que vivent les
                // chemins de texture (`m_texPath`).
                objbin::MenuComponent::Unknown(u) => {
                    for s in u.strings() {
                        if s.ends_with(ICONS_SUFFIX) {
                            facts.g4tx.insert(s.to_owned());
                        }
                    }
                }
                // Le pont UI → texte : chaque slot porte le CRC-32 de son libellé.
                objbin::MenuComponent::Text(t) => {
                    for e in &t.entries {
                        for h in &e.hashes {
                            if *h != 0 {
                                facts
                                    .text_slots
                                    .insert((obj.name.clone(), e.key.clone(), *h));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    for (path, name, base) in &paths.scripts {
        if !matches(def, name) && !matches(def, base) {
            continue;
        }
        let Ok(bytes) = vfs.read(path) else {
            facts.unreadable += 1;
            facts.scripts.push(Script {
                path: path.clone(),
                bytes: 0,
                instructions: 0,
                functions: 0,
                includes: Vec::new(),
                commands: Vec::new(),
                error: Some("script indexe mais illisible sur ce montage".to_owned()),
            });
            continue;
        };
        facts
            .scripts
            .push(analyse_script(path, &bytes, &funclua.table));
    }
    facts.scripts.sort_by(|a, b| a.path.cmp(&b.path));

    facts
}

/// Analyse **byte-exacte** d'un `.lua.bin`.
///
/// Désassemble le conteneur de bytecode Lua 5.2 réel ([`nie_lua::bytecode::parse`], pas un
/// décompilateur externe) et en extrait ce qui intéresse une fiche de mode : nombre
/// d'instructions et de fonctions, modules `INCLUDE`d, et les `cmdId` que le script est
/// **structurellement capable** d'émettre.
///
/// Portage de `mode_index::analyse_lua` (`crates/tools/nie-cli/src/mode_index.rs:466`), à deux
/// choses près : l'erreur sort dans un champ `error` typé plutôt que dans un objet `{erreur}`
/// ad hoc, et les commandes n'ont pas de nom (cf. [`FuncluaTable::naming`]).
fn analyse_script(path: &str, bytes: &[u8], handlers: &BTreeMap<u32, u64>) -> Script {
    let chunk = match bytecode::parse(bytes) {
        Ok(c) => c,
        Err(e) => {
            return Script {
                path: path.to_owned(),
                bytes: bytes.len(),
                instructions: 0,
                functions: 0,
                includes: Vec::new(),
                commands: Vec::new(),
                error: Some(format!("bytecode Lua 5.2 non reconnu : {e}")),
            };
        }
    };

    let mut includes = std::collections::BTreeSet::new();
    let mut cmd_ids = std::collections::BTreeSet::new();
    walk_prototype(&chunk.main, &mut includes, &mut cmd_ids, handlers);

    Script {
        path: path.to_owned(),
        bytes: bytes.len(),
        instructions: chunk.main.total_instructions(),
        functions: chunk.main.total_protos() + 1,
        includes: includes.into_iter().collect(),
        // `filter_map` et non `map` : un `cmdId` n'entre dans `cmd_ids` que s'il est déjà dans
        // la table, donc la branche vide est inatteignable — mais la coder ainsi interdit
        // structurellement de publier un `handler: ""`, qui se lirait comme une adresse.
        commands: cmd_ids
            .into_iter()
            .filter_map(|id| {
                handlers.get(&id).map(|va| Command {
                    cmd_id: format!("0x{id:08X}"),
                    handler: format!("0x{va:X}"),
                })
            })
            .collect(),
        error: None,
    }
}

/// Parcourt un prototype et **tous** ses prototypes imbriqués.
///
/// En Lua 5.2 chaque prototype a son propre pool de constantes : s'arrêter au principal
/// perdrait la quasi-totalité des `cmdId`.
fn walk_prototype(
    p: &bytecode::Prototype,
    includes: &mut std::collections::BTreeSet<String>,
    cmd_ids: &mut std::collections::BTreeSet<u32>,
    handlers: &BTreeMap<u32, u64>,
) {
    for c in &p.constants {
        match c {
            bytecode::Constant::String(s) => {
                // Les modules partagés du moteur portent tous ce préfixe (`LUA_MENU_DEF`,
                // `LUA_LISTVIEW_INC`…) : c'est la convention que lit `INCLUDE()` côté VM, pas
                // une supposition locale.
                if let Ok(txt) = core::str::from_utf8(s)
                    && txt.starts_with("LUA_")
                {
                    includes.insert(txt.to_owned());
                }
            }
            // Les cmdId arrivent en f64 côté Lua ; on ne retient que les entiers exacts de
            // l'espace u32 ET présents dans le dump de handlers — sinon un flottant de jeu
            // ordinaire (score, ratio…) pourrait coïncider.
            bytecode::Constant::Number(n)
                if *n >= 0.0 && n.fract() == 0.0 && *n <= f64::from(u32::MAX) =>
            {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "borne verifiee juste au-dessus : 0 <= n <= u32::MAX et n.fract() == 0"
                )]
                let id = *n as u32;
                if handlers.contains_key(&id) {
                    cmd_ids.insert(id);
                }
            }
            _ => {}
        }
    }
    for sub in &p.protos {
        walk_prototype(sub, includes, cmd_ids, handlers);
    }
}

// ── Les routes des modes ─────────────────────────────────────────────────────

/// Un mode, tel que le catalogue le résume.
#[derive(Debug, Clone, Serialize)]
pub struct ModeSummary {
    /// Identifiant stable, utilisable en URL.
    pub slug: &'static str,
    /// Libellé de repli.
    pub label: &'static str,
    /// Les préfixes VFS qui définissent son périmètre.
    pub prefixes: &'static [&'static str],
    /// `true` quand le jeu énumère lui-même ce mode dans ses réglages audio.
    pub official: bool,
    /// Région de l'atlas du menu principal qui porte sa tuile, quand elle est identifiée.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_region: Option<&'static str>,
    /// Hash `menu_text` de son libellé officiel, en hexadécimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_hash: Option<String>,
    /// Ce que les fichiers permettent d'affirmer sur ce mode.
    pub note: &'static str,
    /// La route qui rend son contenu.
    pub content_route: String,
}

impl From<&'static ModeDef> for ModeSummary {
    fn from(d: &'static ModeDef) -> Self {
        Self {
            slug: d.slug,
            label: d.label,
            prefixes: d.prefixes,
            official: d.official,
            icon_region: d.icon_region,
            label_hash: d.label_hash.map(|h| format!("0x{h:08x}")),
            note: d.note,
            content_route: format!("/api/v1/modes/{}", d.slug),
        }
    }
}

/// Le catalogue des modes.
#[derive(Debug, Clone, Serialize)]
pub struct ModeCatalog {
    /// Le motif appliqué, s'il y en avait un.
    pub q: Option<String>,
    /// Nombre de modes catalogués, **avant** filtrage.
    pub total_modes: usize,
    /// Nombre de modes que le jeu énumère lui-même.
    pub official_modes: usize,
    /// D'où vient cette liste, et pourquoi elle est curatée.
    pub provenance: &'static str,
    /// La page.
    pub results: Page<ModeSummary>,
}

/// Un mode est-il retenu par le motif ?
fn mode_retained(d: &ModeDef, pattern: Option<&str>) -> bool {
    pattern.is_none_or(|q| {
        d.slug.to_lowercase().contains(q) || d.label.to_lowercase().contains(q)
    })
}

/// `GET /api/v1/modes` — le catalogue des modes de jeu.
///
/// Cette route ne touche **pas** le VFS : la liste est une constante du code, et elle répond
/// donc même pendant le montage. Le contenu, lui, exige le VFS.
///
/// # Errors
///
/// Aucune en pratique — la signature reste faillible pour rester homogène avec les autres
/// routes de l'API et pouvoir évoluer sans casser ses appelants.
pub async fn modes(Query(query): Query<DemandePage>) -> Result<Json<ModeCatalog>, ErreurSite> {
    let pattern = clean(query.q.as_deref());
    let kept: Vec<&'static ModeDef> = MODES
        .iter()
        .filter(|d| mode_retained(d, pattern.as_deref()))
        .collect();
    let bounds = query.bornee();
    let total = kept.len();
    let page: Vec<ModeSummary> = kept
        .into_iter()
        .skip(bounds.offset())
        .take(bounds.per_page as usize)
        .map(ModeSummary::from)
        .collect();
    Ok(Json(ModeCatalog {
        q: pattern,
        total_modes: MODES.len(),
        official_modes: MODES.iter().filter(|d| d.official).count(),
        provenance: "liste curatee : le jeu designe ses onglets par un TAB_TYPE entier et ne \
                     stocke la liste nulle part en clair ; chaque entree est en revanche \
                     adossee a des ecrans *_setting.cfg.bin reels du VFS, et l'agregation, \
                     elle, est mecanique",
        results: Page::nouvelle(page, bounds, total),
    }))
}

/// Le mode d'un slug, ou le `404` qui cite les slugs existants.
///
/// # Errors
///
/// `Introuvable` (404) quand le slug n'est pas au catalogue.
fn resolve_mode(slug: &str) -> Result<&'static ModeDef, ErreurSite> {
    MODES.iter().find(|d| d.slug == slug).ok_or_else(|| {
        ErreurSite::Introuvable(format!(
            "mode inconnu `{slug}` ; les {} modes catalogues sont : {}",
            MODES.len(),
            MODES
                .iter()
                .map(|d| d.slug)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })
}

/// `GET /api/v1/modes/{slug}` — le contenu mesuré d'un mode.
///
/// Le coût est réel : sur `victory-road`, l'agrégation ouvre 28 écrans, 204 objets de menu et
/// 32 scripts. C'est pour cela que `elapsed_ms` est publié à chaque appel plutôt qu'affirmé une
/// fois en commentaire — et que l'énumération du VFS, elle, ne se paie qu'une seule fois
/// ([`MENU_PATHS`]).
///
/// # Errors
///
/// `404` sur un slug inconnu, `503` tant que le VFS n'est pas monté.
pub async fn mode(
    State(state): State<EtatSite>,
    Path(slug): Path<String>,
) -> Result<Json<ModeContent>, ErreurSite> {
    // L'ordre compte : un slug inconnu rend 404 même sur un service sans VFS. Sinon le client
    // croit qu'il lui manque une capacité alors qu'il s'est trompé d'URL.
    let def = resolve_mode(&slug)?;
    let paths = menu_paths(&state).await?;
    let vfs = state.vfs()?;

    let start = std::time::Instant::now();
    let facts = tokio::task::spawn_blocking(move || {
        let funclua = FUNCLUA.get_or_init(load_funclua);
        (collect(&vfs, paths, def, funclua), funclua)
    })
    .await?;
    let (facts, funclua) = facts;
    let elapsed_ms = start.elapsed().as_millis();

    let counts = ModeCounts {
        screens: facts.screens.len(),
        layers: facts.layers.len(),
        objbins: facts.objbins.len(),
        g4pkm: facts.g4pkm.len(),
        g4tx: facts.g4tx.len(),
        component_types: facts.components.len(),
        components: facts.components.values().sum(),
        scripts: facts.scripts.len(),
        focus: facts.focus,
        text_slots: facts.text_slots.len(),
        unreadable: facts.unreadable,
    };

    Ok(Json(ModeContent {
        slug: def.slug,
        label: def.label,
        official: def.official,
        prefixes: def.prefixes,
        screens: facts.screens,
        layers: facts.layers.into_iter().collect(),
        objbins: facts.objbins.into_iter().collect(),
        g4pkm: facts.g4pkm.into_iter().collect(),
        g4tx: facts.g4tx.into_iter().collect(),
        components: facts
            .components
            .into_iter()
            .map(|(type_name, count)| ComponentCount { type_name, count })
            .collect(),
        scripts: facts.scripts,
        counts,
        funclua: funclua_report(funclua),
        messages: ModeMessages {
            available: false,
            key_pattern: def.key_pattern,
            reason: "le site ne lit pas nie.exe : les cles de message d'un mode ne sont \
                     nommees que dans les chaines du binaire, hors du VFS et hors du \
                     perimetre d'un serveur web",
            alternative: "`niers mode contenu <slug>` les resout en local ; le texte \
                          localise, lui, est servi par /api/v1/text",
        },
        elapsed_ms,
    }))
}

// ═══ Couverture des écrans ═══════════════════════════════════════════════════

/// Le canvas de référence des menus d'IEVR.
const CANVAS: (u32, u32) = (1280, 720);

/// La locale par défaut pour résoudre les chemins de texture porteurs de `<LG>`.
const SCREEN_LOCALE: &str = "fr";

/// Un écran, avec ce que le site sait en produire.
///
/// Le champ qui décide de la couverture est [`Self::served`], et sa définition est **choisie
/// pour pouvoir échouer** : un écran n'est `served` que si **tous** ses calques déclarés
/// résolvent vers un `.objbin` réellement présent dans ce montage. Compter `served` tout écran
/// dont le `_setting.cfg.bin` se lit aurait rendu 479/479 par construction — exactement le
/// défaut que le § 9 bis du cap a payé sur le VFS.
#[derive(Debug, Clone, Serialize)]
pub struct ScreenEntry {
    /// Le nom de l'écran — le stem, sans `_setting.cfg.bin`.
    pub screen: String,
    /// Le chemin VFS de son conteneur.
    pub cfg: String,
    /// Sa taille, en octets.
    pub bytes: usize,
    /// Le nombre de calques déclarés.
    pub layers: usize,
    /// Combien de ces calques ont leur `.objbin` dans ce montage.
    pub layers_resolved: usize,
    /// Ceux qui manquent, **nommés** — un calque perdu en silence est un écran qu'on croit
    /// complet.
    pub layers_missing: Vec<String>,
    /// Le nombre d'éléments focusables déclarés.
    pub focus: usize,
    /// `true` quand la route sait produire les trois nombres de cet écran.
    pub served: bool,
}

/// Le catalogue des écrans, construit une fois.
#[derive(Debug, Default)]
struct ScreenIndex {
    /// Les écrans, par nom croissant.
    entries: Vec<ScreenEntry>,
    /// Combien de conteneurs n'ont pas pu être lus.
    unreadable: usize,
    /// Nombre total de calques déclarés, toutes déclarations confondues.
    layers_declared: usize,
    /// Combien de ces déclarations résolvent vers un `.objbin`.
    layers_resolved: usize,
    /// Les noms de calque déclarés qu'aucun `.objbin` ne porte, avec leur nombre d'écrans.
    ///
    /// C'est ce qui transforme « 36 % » d'un reste-à-faire en un **fait sur le jeu** :
    /// vérifié le 2026-09-06 sur `cmn01_10_new_icon_tab`, `team13_03_grid_item_root` et
    /// `act01_04_achieve_icon_bronze`, `niers vfs find <nom>` rend **0 résultat** — ces
    /// calques n'existent sous AUCUNE forme dans les 255 308 entrées. Le plafond n'est pas
    /// dans le câblage du site.
    missing_layers: BTreeMap<String, usize>,
    /// Durée de la construction, en millisecondes.
    elapsed_ms: u128,
}

/// Le catalogue, calculé à la première demande.
static SCREENS: OnceLock<ScreenIndex> = OnceLock::new();

/// Lit les calques et les focusables d'un `_setting.cfg.bin`.
///
/// Même lecture que [`collect`], isolée pour que la couverture ne dépende pas d'un mode.
fn read_screen(bytes: &[u8]) -> Option<(Vec<String>, usize)> {
    let file = cfgbin::parse_t2b(bytes).ok()?;
    let (mut layers, mut focus) = (Vec::new(), 0usize);
    walk(&file.entries, &mut |e: &CfgEntry| {
        if e.name.contains("LIST_BEG") || e.name.contains("LIST_END") {
            return;
        }
        if e.name.starts_with("MENU_LAYER_INFO") {
            if let Some(n) = first_string(e) {
                layers.push(n.to_owned());
            }
        } else if e.name.starts_with("MENU_FOCUS_BASE_INFO") {
            focus += 1;
        }
    });
    Some((layers, focus))
}

/// Décide de la couverture d'un écran : combien de calques résolvent, lesquels manquent.
///
/// **Fonction pure**, extraite de [`build_screens`] pour être prouvée sans VFS. Un écran sans
/// aucun calque n'est PAS `served` : il n'a rien à mesurer, et le compter servi ferait monter
/// le ratio sans qu'un objet de plus soit atteignable.
fn layer_status(
    layers: &[String],
    objects: &BTreeMap<String, String>,
) -> (usize, Vec<String>, bool) {
    let missing: Vec<String> = layers
        .iter()
        .filter(|l| !objects.contains_key(*l))
        .cloned()
        .collect();
    let served = missing.is_empty() && !layers.is_empty();
    (layers.len() - missing.len(), missing, served)
}

/// Dit si un objet de menu est **muet** : ni texture, ni slot de texte.
///
/// **Fonction pure.** Un objet qui porte l'un des deux affiche quelque chose qu'on sait
/// nommer ; un objet qui n'a ni l'un ni l'autre est un conteneur, une collision ou une ancre —
/// c'est ce que le § 3 du cap appelle un « objet muet ».
fn is_mute(obj: &objbin::MenuObject) -> bool {
    let a_du_texte = obj.components.iter().any(|c| match c {
        objbin::MenuComponent::Text(t) => !t.entries.is_empty(),
        _ => false,
    });
    obj.g4tx_path.is_none() && !a_du_texte
}

/// Le ratio de couverture, en pourcentage arrondi au centième. `0` sur un total nul.
#[allow(clippy::cast_precision_loss)]
fn coverage_pct(served: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    (served as f64 / total as f64 * 10_000.0).round() / 100.0
}

/// Construit le catalogue des écrans.
///
/// Ne lit **que** les `_setting.cfg.bin` (479 fichiers, quelques kilo-octets chacun) : la
/// résolution d'un calque est une consultation de [`MenuPaths::objects`], déjà en mémoire.
/// Les `.objbin` et leurs compagnons ne sont lus qu'à la demande d'un écran précis — un
/// balayage complet coûterait des milliers de lectures VFS pour une page de catalogue.
fn build_screens(vfs: &Vfs, paths: &MenuPaths) -> ScreenIndex {
    let start = std::time::Instant::now();
    let mut out = ScreenIndex::default();
    for path in &paths.screens {
        let Some(name) = stem(path, SCREEN_SUFFIX) else {
            continue;
        };
        let Ok(bytes) = vfs.read(path) else {
            out.unreadable += 1;
            continue;
        };
        let Some((layers, focus)) = read_screen(&bytes) else {
            out.unreadable += 1;
            continue;
        };
        let (resolved, missing, served) = layer_status(&layers, &paths.objects);
        out.layers_declared += layers.len();
        out.layers_resolved += resolved;
        for m in &missing {
            *out.missing_layers.entry(m.clone()).or_insert(0) += 1;
        }
        out.entries.push(ScreenEntry {
            screen: name,
            cfg: path.clone(),
            bytes: bytes.len(),
            layers: layers.len(),
            layers_resolved: resolved,
            served,
            layers_missing: missing,
            focus,
        });
    }
    out.entries.sort_by(|a, b| a.screen.cmp(&b.screen));
    out.elapsed_ms = start.elapsed().as_millis();
    tracing::info!(
        ecrans = out.entries.len(),
        servis = out.entries.iter().filter(|e| e.served).count(),
        calques = out.layers_declared,
        calques_resolus = out.layers_resolved,
        calques_absents = out.missing_layers.len(),
        ms = out.elapsed_ms,
        "catalogue des ecrans construit"
    );
    out
}

/// Rend le catalogue des écrans, en le construisant à la première demande.
async fn screen_index(state: &EtatSite) -> Result<&'static ScreenIndex, ErreurSite> {
    if let Some(s) = SCREENS.get() {
        return Ok(s);
    }
    let paths = menu_paths(state).await?;
    let vfs = state.vfs()?;
    let built = tokio::task::spawn_blocking(move || {
        SCREENS.get_or_init(|| build_screens(&vfs, paths))
    })
    .await?;
    Ok(built)
}

/// Ce que `GET /api/v1/screens` publie.
#[derive(Debug, Clone, Serialize)]
pub struct ScreenCoverage {
    /// Le nombre d'écrans trouvés dans le VFS.
    pub total: usize,
    /// Combien dont la route sait produire les trois nombres.
    pub served: usize,
    /// Combien à qui il manque au moins un calque.
    pub partial: usize,
    /// Le ratio, en pourcentage, arrondi au centième.
    pub coverage_pct: f64,
    /// Combien de conteneurs n'ont pas pu être lus sur ce montage.
    pub unreadable: usize,
    /// Nombre total de calques déclarés par les écrans.
    pub layers_declared: usize,
    /// Combien de ces déclarations résolvent vers un `.objbin` du VFS.
    pub layers_resolved: usize,
    /// Le taux de résolution des **calques**, qui n'est pas celui des écrans : un seul calque
    /// absent suffit à retirer `served` à un écran de soixante.
    pub layers_pct: f64,
    /// Combien de **noms de calque distincts** sont déclarés sans qu'aucun `.objbin` les porte.
    pub missing_layers: usize,
    /// La route qui les énumère.
    pub missing_route: &'static str,
    /// Où est réellement le plafond, dit plutôt que laissé à déduire.
    pub ceiling: &'static str,
    /// Les trois nombres que porte chaque écran **servi**, quand on le demande.
    pub per_screen: &'static [&'static str],
    /// La route qui rend ces trois nombres.
    pub screen_route: &'static str,
    /// Durée de la construction du catalogue, mesurée.
    pub build_ms: u128,
    /// La page d'écrans.
    pub results: Page<ScreenEntry>,
}

/// `GET /api/v1/screens` — la couverture des écrans du jeu.
///
/// C'est la condition 4 du § 8 de `docs/PLAN-SITE-ULTIME.md` : publier `écrans servis / total`,
/// et pour chaque écran couvert ses trois nombres — objets, objets positionnés, objets muets.
/// Le total est **mesuré** sur le VFS, jamais cité : le plan a déjà écrit 440 là où la mesure
/// en rend 475.
///
/// `?q=` filtre sur le nom, et il est **appliqué** — le total republié est celui des retenus.
///
/// # Errors
///
/// `503` tant que le VFS n'est pas monté.
pub async fn screens(
    State(state): State<EtatSite>,
    Query(demande): Query<DemandePage>,
) -> Result<Json<ScreenCoverage>, ErreurSite> {
    let idx = screen_index(&state).await?;
    let motif = clean(demande.q.as_deref()).map(|q| q.to_lowercase());
    let retenus: Vec<&ScreenEntry> = idx
        .entries
        .iter()
        .filter(|e| {
            motif
                .as_ref()
                .is_none_or(|m| e.screen.to_lowercase().contains(m))
        })
        .collect();
    let total = retenus.len();
    let served = retenus.iter().filter(|e| e.served).count();
    let bornes = demande.bornee();
    let elements: Vec<ScreenEntry> = retenus
        .iter()
        .skip(bornes.offset())
        .take(bornes.per_page as usize)
        .map(|e| (*e).clone())
        .collect();
    Ok(Json(ScreenCoverage {
        total,
        served,
        partial: total - served,
        coverage_pct: coverage_pct(served, total),
        unreadable: idx.unreadable,
        layers_declared: idx.layers_declared,
        layers_resolved: idx.layers_resolved,
        layers_pct: coverage_pct(idx.layers_resolved, idx.layers_declared),
        missing_layers: idx.missing_layers.len(),
        missing_route: "/api/v1/screens/missing",
        ceiling: "un ecran n'est `served` que si TOUS ses calques resolvent. Les calques qui \
                  manquent ne sont PAS un defaut de cablage : verifie le 2026-09-06, \
                  `niers vfs find <nom>` rend 0 resultat sur les 255 308 entrees pour \
                  `cmn01_10_new_icon_tab`, `team13_03_grid_item_root` et \
                  `act01_04_achieve_icon_bronze`. Le jeu declare des calques dont l'asset \
                  n'est pas livre dans ce build — contenu coupe, ou construit au runtime par \
                  script. Le plafond est dans la donnee, pas dans le site",
        per_screen: &["objects", "positioned", "mute"],
        screen_route: "/api/v1/screens/{screen}",
        build_ms: idx.elapsed_ms,
        results: Page::nouvelle(elements, bornes, total).filtree(motif),
    }))
}

/// Un calque déclaré qu'aucun `.objbin` du VFS ne porte.
#[derive(Debug, Clone, Serialize)]
pub struct MissingLayer {
    /// Le nom du calque, tel que le `_setting.cfg.bin` le déclare.
    pub layer: String,
    /// Combien d'écrans le déclarent.
    pub screens: usize,
}

/// `GET /api/v1/screens/missing` — les calques que le jeu déclare et ne livre pas.
///
/// C'est la moitié de la mesure qui empêche « 36 % » de passer pour un reste-à-faire. Ces noms
/// n'existent sous **aucune** forme dans les 255 308 entrées du VFS — ni `.objbin`, ni archive,
/// ni autre extension. Les énumérer, c'est dire où est le plafond au lieu de le laisser
/// deviner.
///
/// `?q=` filtre sur le nom, et il est appliqué.
///
/// # Errors
///
/// `503` tant que le VFS n'est pas monté.
pub async fn missing_layers(
    State(state): State<EtatSite>,
    Query(demande): Query<DemandePage>,
) -> Result<Json<Page<MissingLayer>>, ErreurSite> {
    let idx = screen_index(&state).await?;
    let motif = clean(demande.q.as_deref()).map(|q| q.to_lowercase());
    let mut retenus: Vec<MissingLayer> = idx
        .missing_layers
        .iter()
        .filter(|(l, _)| {
            motif
                .as_ref()
                .is_none_or(|m| l.to_lowercase().contains(m))
        })
        .map(|(layer, screens)| MissingLayer {
            layer: layer.clone(),
            screens: *screens,
        })
        .collect();
    // Les plus réclamés d'abord : un calque déclaré par vingt écrans coûte vingt fois plus
    // qu'un déclaré par un seul.
    retenus.sort_by(|a, b| b.screens.cmp(&a.screens).then_with(|| a.layer.cmp(&b.layer)));
    let total = retenus.len();
    let bornes = demande.bornee();
    let elements = retenus
        .into_iter()
        .skip(bornes.offset())
        .take(bornes.per_page as usize)
        .collect();
    Ok(Json(Page::nouvelle(elements, bornes, total).filtree(motif)))
}

/// Un objet de menu d'un écran, réduit à ce qui décide des trois nombres.
#[derive(Debug, Clone, Serialize)]
pub struct ScreenObject {
    /// Le nom du calque, tel que le `_setting.cfg.bin` le déclare.
    pub layer: String,
    /// Le chemin VFS de son `.objbin`, ou `null` s'il manque à ce montage.
    pub objbin: Option<String>,
    /// Le nom de l'objet, tel que `OBJ_BGN` le porte.
    pub name: Option<String>,
    /// Sa priorité de dessin.
    pub draw_priority: Option<i32>,
    /// Sa position sur le canvas 1280×720, `null` quand aucune pose ne la donne.
    pub position: Option<[f32; 2]>,
    /// `true` quand un `.g4pkm` a fourni une pose.
    pub positioned: bool,
    /// `true` quand l'objet ne porte **ni** texture **ni** slot de texte : il n'affiche rien
    /// qu'on sache nommer.
    pub mute: bool,
    /// Pourquoi l'objet n'est pas positionné, quand il ne l'est pas.
    pub reason: Option<&'static str>,
}

/// Les trois nombres d'un écran, et le détail qui les produit.
#[derive(Debug, Clone, Serialize)]
pub struct ScreenDetail {
    /// Le nom de l'écran.
    pub screen: String,
    /// Le chemin VFS de son conteneur.
    pub cfg: String,
    /// Le canvas de référence.
    pub canvas: [u32; 2],
    /// **Objets** — le premier des trois nombres.
    pub objects: usize,
    /// **Objets positionnés** — le deuxième.
    pub positioned: usize,
    /// **Objets muets** — le troisième.
    pub mute: usize,
    /// Les calques déclarés dont l'`.objbin` manque à ce montage.
    pub layers_missing: Vec<String>,
    /// Le nombre d'éléments focusables déclarés.
    pub focus: usize,
    /// Le détail, objet par objet, dans l'ordre du fichier.
    pub items: Vec<ScreenObject>,
    /// Ce que ces nombres ne disent pas.
    pub caveat: &'static str,
    /// Durée de l'agrégation, mesurée à chaque appel.
    pub elapsed_ms: u128,
}

/// `GET /api/v1/screens/{screen}` — les trois nombres d'un écran, et leur détail.
///
/// # Les trois nomenclatures d'écran
///
/// Le § 9.4 du cap l'écrit : le nom du **calque** (`mainmenu01`), celui du **script**
/// (`kizuna_town_mainmenu`) et le **stem du `_setting.cfg.bin`** sont trois choses. C'est le
/// troisième qui est attendu ici, et un nom inconnu rend un `404` qui le dit — pas un objet
/// vide qu'on prendrait pour un écran sans contenu.
///
/// # Errors
///
/// `404` sur un écran inconnu, `503` sans VFS.
pub async fn screen(
    State(state): State<EtatSite>,
    Path(name): Path<String>,
) -> Result<Json<ScreenDetail>, ErreurSite> {
    let start = std::time::Instant::now();
    let idx = screen_index(&state).await?;
    let entry = idx
        .entries
        .iter()
        .find(|e| e.screen == name)
        .ok_or_else(|| {
            ErreurSite::Introuvable(format!(
                "aucun ecran `{name}` dans ce jeu ; le catalogue est sur /api/v1/screens. \
                 Attention : c'est le stem du `_setting.cfg.bin` qui est attendu, pas le nom \
                 d'un calque ni celui d'un script"
            ))
        })?;

    let paths = menu_paths(&state).await?;
    let vfs = state.vfs()?;
    let cfg = entry.cfg.clone();
    let index = state.index()?;

    let items = tokio::task::spawn_blocking(move || {
        let Ok(bytes) = vfs.read(&cfg) else {
            return Vec::new();
        };
        let Some((layers, _)) = read_screen(&bytes) else {
            return Vec::new();
        };
        layers
            .into_iter()
            .map(|layer| inspect_layer(&vfs, &index, paths, &layer))
            .collect::<Vec<_>>()
    })
    .await?;

    let objects = items.iter().filter(|i| i.objbin.is_some()).count();
    let positioned = items.iter().filter(|i| i.positioned).count();
    let mute = items.iter().filter(|i| i.mute).count();
    Ok(Json(ScreenDetail {
        screen: entry.screen.clone(),
        cfg: entry.cfg.clone(),
        canvas: [CANVAS.0, CANVAS.1],
        objects,
        positioned,
        mute,
        layers_missing: entry.layers_missing.clone(),
        focus: entry.focus,
        items,
        caveat: "`positioned` compte les objets dont un `.g4pkm` fournit une pose ; un objet \
                 non positionne est un MANQUE DE L'EXPORT, pas un detail de rendu. `mute` \
                 compte ceux qui ne portent ni texture ni slot de texte : ils n'affichent rien \
                 qu'on sache nommer. Aucun de ces trois nombres n'est une SSIM — la \
                 conformite pixel se mesure ailleurs, et elle n'est pas mesuree ici",
        elapsed_ms: start.elapsed().as_millis(),
    }))
}

/// Examine un calque : son `.objbin`, sa pose, et s'il affiche quelque chose.
fn inspect_layer(
    vfs: &Vfs,
    index: &IndexVfs,
    paths: &MenuPaths,
    layer: &str,
) -> ScreenObject {
    let vide = |reason: &'static str| ScreenObject {
        layer: layer.to_owned(),
        objbin: None,
        name: None,
        draw_priority: None,
        position: None,
        positioned: false,
        mute: false,
        reason: Some(reason),
    };
    let Some(objbin_path) = paths.objects.get(layer) else {
        return vide("aucun .objbin de ce nom dans ce montage du VFS");
    };
    let Ok(bytes) = vfs.read(objbin_path) else {
        return vide("objbin indexe mais illisible sur ce montage");
    };
    let Ok(obj) = objbin::parse(&bytes) else {
        return vide("objbin present mais illisible par objbin::parse");
    };

    let draw_priority = obj.components.iter().find_map(|c| match c {
        objbin::MenuComponent::Render(r) => Some(r.draw_priority),
        _ => None,
    });
    let mute = is_mute(&obj);

    // La pose. Le sprite entre dans l'appariement d'os, mais son ABSENCE ne doit pas empêcher
    // de placer : on place alors avec un sprite de 0×0, comme `routes::inspect`.
    let (mut position, mut positioned, mut reason) = (None, false, None);
    match obj.g4pkm_path.as_deref() {
        None => reason = Some("l'objet ne declare aucun SkeletonAnime"),
        Some(logique) => {
            match super::inspect::resolve_companion(index, logique, SCREEN_LOCALE) {
                None => reason = Some("chemin de squelette declare mais absent de ce montage"),
                Some(p) => match vfs.read(&p).ok().and_then(|d| {
                    nie_formats::g4pkm::parse(&d).ok()
                }) {
                    None => reason = Some("squelette lu mais illisible par g4pkm::parse"),
                    Some(layout) => {
                        let t = nie_formats::menu::assemble_object(&obj, &layout, 0, 0).transform;
                        position = Some([t.x_px, t.y_px]);
                        positioned = true;
                    }
                },
            }
        }
    }

    ScreenObject {
        layer: layer.to_owned(),
        objbin: Some(objbin_path.clone()),
        name: Some(obj.name),
        draw_priority,
        position,
        positioned,
        mute,
        reason,
    }
}

#[cfg(test)]
mod tests_screens {
    use super::*;

    fn objets(noms: &[&str]) -> BTreeMap<String, String> {
        noms.iter()
            .map(|n| ((*n).to_owned(), format!("data/x/{n}.objbin")))
            .collect()
    }

    fn calques(noms: &[&str]) -> Vec<String> {
        noms.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn un_ecran_n_est_servi_que_si_tous_ses_calques_resolvent() {
        // La garde qui empeche la gate d'etre vraie par construction. La moitie positive
        // seule passerait sur un `served` cable a `true` — c'est exactement le defaut que le
        // § 9 bis du cap a paye sur le VFS.
        let dispo = objets(&["a", "b"]);
        let (n, manquants, servi) = layer_status(&calques(&["a", "b"]), &dispo);
        assert_eq!((n, servi), (2, true));
        assert!(manquants.is_empty());

        let (n, manquants, servi) = layer_status(&calques(&["a", "b", "c"]), &dispo);
        assert_eq!((n, servi), (2, false), "un calque absent suffit a retirer `served`");
        assert_eq!(manquants, vec!["c".to_owned()], "et il est NOMME");
    }

    #[test]
    fn un_ecran_sans_calque_n_est_pas_servi() {
        // Sinon le ratio monterait sans qu'un objet de plus soit atteignable.
        let (n, manquants, servi) = layer_status(&[], &objets(&["a"]));
        assert_eq!((n, servi), (0, false));
        assert!(manquants.is_empty());
    }

    #[test]
    fn le_ratio_est_arrondi_au_centieme_et_ne_divise_pas_par_zero() {
        assert!((coverage_pct(171, 475) - 36.0).abs() < f64::EPSILON);
        assert!((coverage_pct(1, 3) - 33.33).abs() < f64::EPSILON);
        assert!((coverage_pct(0, 0) - 0.0).abs() < f64::EPSILON);
        assert!((coverage_pct(475, 475) - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn un_objet_est_muet_quand_il_ne_porte_ni_texture_ni_texte() {
        let nu = objbin::MenuObject {
            name: "x".to_owned(),
            engine_type: "gmdMenuObj".to_owned(),
            g4pkm_path: None,
            g4tx_path: None,
            skeleton_path: None,
            anime_path: None,
            components: Vec::new(),
        };
        assert!(is_mute(&nu), "ni texture ni texte : muet");

        // Preuve par falsification : une texture suffit a le rendre non muet. Sans cette
        // moitie, un `is_mute` cable a `true` passerait le test precedent.
        let avec_texture = objbin::MenuObject {
            g4tx_path: Some("menu/x.g4tx".to_owned()),
            ..nu.clone()
        };
        assert!(!is_mute(&avec_texture));
    }

    #[test]
    fn le_stem_d_un_ecran_n_est_pas_le_nom_d_un_calque() {
        // Le piege du § 9.4 du cap : trois nomenclatures. `mainmenu01` est un CALQUE ; le
        // stem attendu par la route est celui du `_setting.cfg.bin`. Mesure du 2026-09-06 :
        // /api/v1/screens/mainmenu01 rend 404, et c'est correct.
        assert_eq!(stem("a/b/mainmenu01_setting.cfg.bin", SCREEN_SUFFIX).as_deref(), Some("mainmenu01"));
        assert_eq!(stem("a/b/mainmenu01_00_background.objbin", SCREEN_SUFFIX), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    // ── Icônes : URLs ────────────────────────────────────────────────────────

    #[test]
    fn l_url_d_une_icone_garde_le_g4tx_celle_de_l_atlas_le_retire() {
        // Les deux formes ont ete verifiees en HTTP reel contre nie-model-serve le 2026-09-06.
        let atlas = "data/dx11/menu/200_icon/20_icon_deco/icon_deco.g4tx";
        assert_eq!(
            region_url(atlas, "ds01001"),
            "/assets/tex/dx11/menu/200_icon/20_icon_deco/icon_deco.g4tx/ds01001.png"
        );
        assert_eq!(
            atlas_url(atlas),
            "/assets/tex/dx11/menu/200_icon/20_icon_deco/icon_deco.png"
        );
        // Falsification : la forme `.g4tx.png` rend 404 chez l'amont. Elle ne doit JAMAIS
        // sortir d'ici.
        assert!(!atlas_url(atlas).contains(".g4tx"));
        // Et l'inverse : retirer le `.g4tx` de l'URL d'icone ferait servir l'atlas entier
        // (1 746 102 octets mesures contre 21 604 pour la region).
        assert!(region_url(atlas, "ds01001").contains(".g4tx/"));
        // Le prefixe `data/` est celui du VFS, pas celui de l'amont.
        assert!(!region_url(atlas, "x").contains("/data/"));
        assert!(!atlas_url(atlas).contains("/data/"));
    }

    #[test]
    fn un_chemin_sans_prefixe_data_traverse_sans_dommage() {
        // Robustesse : le montage packs et le montage dump servent les memes chemins
        // logiques, mais rien ne garantit le prefixe. Ne pas le trouver ne doit rien casser.
        assert_eq!(strip_data("data/dx11/x.g4tx"), "dx11/x.g4tx");
        assert_eq!(strip_data("dx11/x.g4tx"), "dx11/x.g4tx");
        assert_eq!(atlas_url("dx11/a/b.g4tx"), "/assets/tex/dx11/a/b.png");
    }

    // ── Icônes : sélection des candidats ─────────────────────────────────────

    #[test]
    fn le_candidat_est_un_g4tx_de_l_arbre_d_icones_et_rien_d_autre() {
        assert_eq!(
            candidate("data/dx11/menu/200_icon/02_icon_item/icon_item01.g4tx"),
            Ok(())
        );
        // Falsification : sans ces refus, l'index avalerait 41 191 atlas au lieu de 212.
        assert_eq!(
            candidate("data/dx11/menu/220_img/x.g4tx"),
            Err(None),
            "hors de l'arbre d'icones"
        );
        assert_eq!(
            candidate("data/dx11/menu/200_icon/02_icon_item/icon_item01.g4pkm"),
            Err(None),
            "mauvaise extension"
        );
        // Les deux familles ecartees le sont NOMMEMENT, pas silencieusement.
        assert_eq!(
            candidate("data/dx11/menu/200_icon/10_icon_chr/c01000010.g4tx"),
            Err(Some("10_icon_chr"))
        );
        assert_eq!(
            candidate("data/dx11/menu/200_icon/01_icon_emblem/e001.g4tx"),
            Err(Some("01_icon_emblem"))
        );
    }

    #[test]
    fn un_bouche_trou_n_est_pas_une_icone() {
        assert!(is_real_icon("icon_item01", 256, 256));
        // Falsification des deux moities de la regle, separement.
        assert!(!is_real_icon("dmy_icon", 256, 256), "les `dmy` sont exclus");
        assert!(!is_real_icon("icon_item01", 4, 4), "4x4 n'est pas une icone");
        // Une image large mais fine reste une icone : la regle exige les DEUX dimensions.
        assert!(is_real_icon("bar", 4, 64));
        assert!(is_real_icon("bar", 64, 4));
    }

    // ── Icônes : les filtres agissent réellement ─────────────────────────────

    /// Trois icônes de deux atlas différents, sans VFS.
    fn icones_temoins() -> Vec<Icon> {
        let a = "data/dx11/menu/200_icon/02_icon_item/icon_item01.g4tx";
        let b = "data/dx11/menu/200_icon/20_icon_deco/icon_deco.g4tx";
        ["abl_000001", "abl_000002", "ds01001"]
            .into_iter()
            .zip([a, a, b])
            .map(|(name, atlas)| Icon {
                name: name.to_owned(),
                atlas: atlas.to_owned(),
                texture: name.to_owned(),
                rect: None,
                width: 256,
                height: 256,
                url: region_url(atlas, name),
                atlas_url: atlas_url(atlas),
            })
            .collect()
    }

    /// Compte ce que le couple (prefix, q) retient.
    fn retenues(prefix: Option<&str>, q: Option<&str>) -> usize {
        icones_temoins()
            .iter()
            .filter(|i| retained(i, prefix, q))
            .count()
    }

    #[test]
    fn q_reduit_reellement_la_liste() {
        // Le defaut n° 1 que ce depot traque : un parametre accepte puis ignore. Il ne se
        // prouve qu'en montrant la REDUCTION, jamais en montrant que la route repond 200.
        assert_eq!(retenues(None, None), 3, "sans filtre, tout passe");
        assert_eq!(retenues(None, Some("abl")), 2, "`abl` doit ecarter ds01001");
        assert_eq!(retenues(None, Some("abl_000001")), 1);
        assert_eq!(retenues(None, Some("introuvable")), 0, "0 est un resultat");
        // Le filtre est sans casse : `clean` minusculise le motif, le nom aussi.
        assert_eq!(retenues(None, Some("abl")), retenues(None, Some("abl")));
    }

    #[test]
    fn prefix_borne_reellement_le_sous_arbre() {
        assert_eq!(retenues(Some("02_icon_item"), None), 2);
        assert_eq!(retenues(Some("20_icon_deco"), None), 1);
        assert_eq!(retenues(Some("99_inexistant"), None), 0);
        // Les deux filtres se cumulent — et leur intersection peut etre vide sans que l'un
        // des deux soit ignore.
        assert_eq!(retenues(Some("20_icon_deco"), Some("abl")), 0);
        assert_eq!(retenues(Some("02_icon_item"), Some("abl")), 2);
    }

    #[test]
    fn un_motif_vide_ou_blanc_n_est_pas_un_filtre() {
        assert_eq!(clean(None), None);
        assert_eq!(clean(Some("")), None);
        assert_eq!(clean(Some("   ")), None);
        assert_eq!(clean(Some("  ABL  ")).as_deref(), Some("abl"));
    }

    #[test]
    fn un_prefixe_qui_remonte_est_un_400_pas_une_liste_vide() {
        assert_eq!(check_prefix(None).unwrap(), None);
        assert_eq!(check_prefix(Some("  ")).unwrap(), None);
        assert_eq!(
            check_prefix(Some(" 02_icon_item ")).unwrap().as_deref(),
            Some("02_icon_item")
        );
        for mauvais in ["..", "../etc", "/menu", "a/../b"] {
            let e = check_prefix(Some(mauvais)).unwrap_err();
            assert_eq!(e.statut().as_u16(), 400, "`{mauvais}` doit etre refuse");
            assert!(format!("{e}").contains("prefixe invalide"), "{e}");
        }
    }

    // ── Modes : le catalogue ─────────────────────────────────────────────────

    #[test]
    fn les_slugs_sont_uniques_et_utilisables_en_url() {
        let mut vus = std::collections::BTreeSet::new();
        for d in MODES {
            assert!(vus.insert(d.slug), "slug duplique : {}", d.slug);
            assert!(
                d.slug
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'-' || b.is_ascii_digit()),
                "`{}` n'est pas un slug d'URL",
                d.slug
            );
            assert!(!d.label.is_empty());
            assert!(!d.note.is_empty(), "`{}` doit dire ce qu'il est", d.slug);
        }
        assert_eq!(vus.len(), MODES.len());
    }

    #[test]
    fn cinq_modes_sont_officiels_et_ce_n_est_pas_un_avis() {
        // Le jeu les enumere lui-meme dans menu_text (volumes BGM/voix, affichage de la liste
        // de puissance). Si ce compte bouge, c'est une decision, pas une retouche.
        let officiels: Vec<&str> = MODES.iter().filter(|d| d.official).map(|d| d.slug).collect();
        assert_eq!(
            officiels,
            vec![
                "victory-road",
                "competition",
                "story",
                "chronicle",
                "kizuna-station"
            ]
        );
        // Falsification : tous ne le sont pas.
        assert!(MODES.iter().any(|d| !d.official));
    }

    #[tokio::test]
    async fn le_catalogue_des_modes_repond_sans_vfs() {
        // Il n'y a rien a monter pour rendre une liste constante : repondre 503 ici ferait
        // croire que le catalogue depend du jeu installe.
        let c = modes(Query(DemandePage::default())).await.unwrap().0;
        assert_eq!(c.total_modes, MODES.len());
        assert_eq!(c.official_modes, 5);
        assert_eq!(c.results.total, MODES.len());
        assert_eq!(c.results.elements.len(), MODES.len().min(50));
        assert!(
            c.results
                .elements
                .iter()
                .any(|m| m.content_route == "/api/v1/modes/victory-road")
        );
    }

    #[tokio::test]
    async fn q_reduit_reellement_le_catalogue_des_modes() {
        let sans = modes(Query(DemandePage::default())).await.unwrap().0;
        let avec = modes(Query(DemandePage {
            q: Some("victory".to_owned()),
            ..DemandePage::default()
        }))
        .await
        .unwrap()
        .0;
        assert!(
            avec.results.total < sans.results.total,
            "le motif doit REDUIRE : {} contre {}",
            avec.results.total,
            sans.results.total
        );
        assert_eq!(avec.results.total, 1);
        assert_eq!(avec.results.elements[0].slug, "victory-road");
        // Le motif applique est republie : un filtre invisible est un filtre qu'on accuse.
        assert_eq!(avec.q.as_deref(), Some("victory"));
        // Sur le libelle aussi, pas seulement sur le slug.
        let par_libelle = modes(Query(DemandePage {
            q: Some("avatar".to_owned()),
            ..DemandePage::default()
        }))
        .await
        .unwrap()
        .0;
        assert_eq!(par_libelle.results.total, 1);
        assert_eq!(par_libelle.results.elements[0].slug, "chara-edit");
        // Et un motif absent rend 0, pas tout.
        let vide = modes(Query(DemandePage {
            q: Some("zzz_inexistant".to_owned()),
            ..DemandePage::default()
        }))
        .await
        .unwrap()
        .0;
        assert_eq!(vide.results.total, 0);
    }

    #[test]
    fn un_slug_inconnu_cite_ceux_qui_existent() {
        assert_eq!(resolve_mode("victory-road").unwrap().slug, "victory-road");
        let e = resolve_mode("victory_road").unwrap_err();
        assert_eq!(e.statut().as_u16(), 404, "l'underscore n'est pas le slug");
        assert!(format!("{e}").contains("victory-road"), "{e}");
        assert_eq!(resolve_mode("").unwrap_err().statut().as_u16(), 404);
    }

    // ── Modes : la reconnaissance par préfixe ────────────────────────────────

    #[test]
    fn le_suffixe_de_version_d_un_script_ne_lui_fait_pas_rater_son_mode() {
        // Sans `versionless`, `victory_road_top_menu_inc_7.01.12.00` echappait au prefixe
        // `victory_road` — et le mode rendait zero script en annoncant un succes.
        assert_eq!(versionless("main_menu_1.02.92.00"), "main_menu");
        assert_eq!(
            versionless("victory_road_top_menu_inc_7.01.12.00"),
            "victory_road_top_menu_inc"
        );
        assert_eq!(versionless("chara_edit_top"), "chara_edit_top");
        let vr = resolve_mode("victory-road").unwrap();
        assert!(matches(vr, &versionless("victory_road_top_menu_inc_7.01.12.00")));
        // Falsification : un script d'un autre mode ne doit pas y entrer.
        assert!(!matches(vr, &versionless("chronicle_mode_top_menu_1.0.0.0")));
        // Un mode sans prefixe ne capte RIEN — c'est le cas mesure de `competition`.
        let comp = resolve_mode("competition").unwrap();
        assert!(comp.prefixes.is_empty());
        assert!(!matches(comp, "competition_top_menu"));
    }

    #[test]
    fn le_stem_se_prend_sur_la_feuille_pas_sur_le_chemin() {
        assert_eq!(
            stem("data/common/gamedata/menu/obj/victory_road_top.objbin", OBJECT_SUFFIX).as_deref(),
            Some("victory_road_top")
        );
        assert_eq!(
            stem("a/b/story_mode_top_menu_setting.cfg.bin", SCREEN_SUFFIX).as_deref(),
            Some("story_mode_top_menu")
        );
        // Falsification : un mauvais suffixe ne doit pas rendre un stem tronque au hasard.
        assert_eq!(stem("a/b/x.objbin", SCREEN_SUFFIX), None);
        assert_eq!(stem("a/b/x.cfg.bin", OBJECT_SUFFIX), None);
    }

    // ── funcLua : l'absence se dit ───────────────────────────────────────────

    #[test]
    fn une_table_funclua_absente_est_annoncee_pas_deguisee_en_zero_commande() {
        // Le piege porte par `mode_index::charger_handlers_funclua` : la remontee depuis
        // current_dir() echoue sous systemd, rend une table vide, et l'analyse annonce zero
        // commande comme si c'etait une mesure.
        let absente = Funclua {
            table: BTreeMap::new(),
            path: "/nulle/part/data/re/funclua-cmdid-handlers.json".to_owned(),
            reason: Some("table absente ou illisible (NotFound)".to_owned()),
        };
        let r = funclua_report(&absente);
        assert!(!r.available);
        assert_eq!(r.entries, 0);
        assert!(r.reason.is_some(), "l'absence doit porter sa raison");
        assert!(r.path.ends_with("funclua-cmdid-handlers.json"));
        assert!(r.effect.contains("vide"), "l'effet est dit, pas devine");

        // Falsification : la meme fonction sur une table presente ne doit pas crier au loup.
        let presente = Funclua {
            table: BTreeMap::from([(0x214d_a123, 0x1_40d0_4300)]),
            path: "/x/data/re/funclua-cmdid-handlers.json".to_owned(),
            reason: None,
        };
        let r = funclua_report(&presente);
        assert!(r.available);
        assert_eq!(r.entries, 1);
        assert!(r.reason.is_none());
    }

    #[test]
    fn la_table_funclua_se_lit_en_hexadecimal_prefixe_et_rien_d_autre() {
        assert_eq!(parse_hex32("0x214DA123"), Some(0x214d_a123));
        assert_eq!(parse_hex64("0x140D04300"), Some(0x1_40d0_4300));
        // Falsification : sans le prefixe, `140D04300` se lirait comme un decimal invalide et
        // une paire mal formee entrerait dans la table.
        assert_eq!(parse_hex32("214DA123"), None);
        assert_eq!(parse_hex64("140D04300"), None);
        assert_eq!(parse_hex32("0x"), None);
        assert_eq!(parse_hex32("0xZZ"), None);
        assert_eq!(parse_hex32("0x1FFFFFFFF"), None, "au-dela de 32 bits");
    }

    #[test]
    fn le_chemin_de_la_table_est_unique_et_publie() {
        // Une seule resolution, deterministe : pas de remontee d'arborescence. Ce que le test
        // garantit, c'est que le chemin PUBLIE est bien celui qui a ete tente.
        let f = load_funclua();
        assert!(
            f.path.ends_with(FUNCLUA_HANDLERS),
            "le chemin publie doit finir par le chemin canonique : {}",
            f.path
        );
        assert_eq!(
            f.table.is_empty(),
            f.reason.is_some(),
            "table vide <=> raison presente : les deux ne peuvent pas diverger"
        );
    }

    // ── Analyse Lua ──────────────────────────────────────────────────────────

    #[test]
    fn un_script_illisible_porte_son_erreur_au_lieu_de_passer_pour_vide() {
        let s = analyse_script("data/x.lua.bin", b"pas du bytecode", &BTreeMap::new());
        assert_eq!(s.instructions, 0);
        assert_eq!(s.functions, 0);
        assert!(s.commands.is_empty());
        let raison = s.error.expect("l'echec doit etre dit");
        assert!(raison.contains("Lua 5.2"), "{raison}");
        // Jamais un `{:?}` dans un champ public : la raison est une phrase choisie.
        assert!(!raison.contains("BytecodeError {"), "{raison}");
        assert_eq!(s.bytes, b"pas du bytecode".len());
    }

    // ── Sans VFS : 503 explicite, jamais un catalogue vide ───────────────────

    /// Un état sans VFS : le montage est `EnCours`, comme au démarrage du service.
    fn etat_sans_vfs() -> EtatSite {
        EtatSite::nouveau(Config::default())
    }

    #[tokio::test]
    async fn sans_vfs_les_routes_d_icones_repondent_503() {
        // Un index vide servi en 200 ferait croire que ce jeu n'a pas d'icones.
        let etat = etat_sans_vfs();
        let e = icons(State(etat.clone()), Query(IconQuery::default()))
            .await
            .expect_err("503");
        assert_eq!(e.statut().as_u16(), 503);

        let e = icon(State(etat), Path("abl_000001".to_owned()))
            .await
            .expect_err("503");
        assert_eq!(e.statut().as_u16(), 503);
    }

    #[tokio::test]
    async fn un_prefixe_invalide_est_refuse_avant_de_toucher_au_vfs() {
        // L'ordre compte : sur un service sans VFS, une demande fautive doit rendre 400 (la
        // demande est en cause) et non 503 (la capacite manque).
        let e = icons(
            State(etat_sans_vfs()),
            Query(IconQuery {
                prefix: Some("../etc".to_owned()),
                ..IconQuery::default()
            }),
        )
        .await
        .expect_err("400");
        assert_eq!(e.statut().as_u16(), 400);
    }

    #[tokio::test]
    async fn un_slug_inconnu_rend_404_meme_sans_vfs() {
        let e = mode(State(etat_sans_vfs()), Path("inexistant".to_owned()))
            .await
            .expect_err("404");
        assert_eq!(e.statut().as_u16(), 404);
        // Et un slug connu, lui, bute sur la capacite manquante : 503.
        let e = mode(State(etat_sans_vfs()), Path("victory-road".to_owned()))
            .await
            .expect_err("503");
        assert_eq!(e.statut().as_u16(), 503);
    }

    // ── Pagination et désérialisation de query ──────────────────────────────

    #[test]
    fn une_query_d_icones_avec_des_nombres_se_deserialise() {
        // Non-regression du piege `#[serde(flatten)]` : avec lui, `per_page=25` echouait en
        // « invalid type: string "25", expected u32 » sur une requete valide.
        let map = serde_json::json!({
            "q": "abl", "prefix": "02_icon_item", "page": 3, "per_page": 25
        });
        let d: IconQuery = serde_json::from_value(map).expect("demande valide");
        assert_eq!(d.q.as_deref(), Some("abl"));
        assert_eq!(d.prefix.as_deref(), Some("02_icon_item"));
        assert_eq!(d.page, Some(3));
        assert_eq!(d.per_page, Some(25));
    }

    #[test]
    fn la_pagination_des_icones_est_bornee_par_la_configuration() {
        let bounds = DemandePage {
            page: Some(1),
            per_page: Some(100_000),
            q: None,
        }
        .bornee();
        assert_eq!(bounds.per_page, crate::config::PER_PAGE_MAX);
    }

    #[test]
    fn l_index_vide_se_declare_vide() {
        let vide = IconIndex::default();
        assert!(vide.is_empty());
        assert_eq!(vide.len(), 0);
        assert!(vide.get("abl_000001").is_none());
    }
}
