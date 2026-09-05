//! Backend Tauri de `nie-explorer` — explorateur/éditeur du VFS (CPK) d'Inazuma Eleven:
//! Victory Road. Toute la logique de décodage vient de `nie-formats`/`nie-explore` (même
//! moteur que `niers vfs cat`, cf. `CLAUDE.md` anti-doublon) ; ce module n'est qu'une façade
//! IPC (JSON) au-dessus de ces crates + une recherche chara/waza via le miroir `nie-wiki`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use base64::Engine as _;
use nie_formats::cpk::{CpkEntry, CpkReader};
use nie_formats::vfs::Vfs;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri_plugin_sql::{Migration, MigrationKind};

mod aphrody;
mod camera_nav;
mod export;
mod forge;
pub mod game_data;
mod live_mod;
mod lua_session;
mod lua_tools;
mod mcp;
mod re_trace;
mod steam;
mod video;
mod viola;

use re_trace::{
    re_dump_open, re_dump_scan, re_trace_dump_module, re_trace_find_process,
    re_trace_module_regions, re_trace_read_bytes_b64, re_trace_write_bytes_b64,
};

/// Migrations SQLite du workspace de mods (`tauri-plugin-sql`, base `mods.db` dans
/// `BaseDirectory::AppData` — jamais dans le dossier du jeu). Un mod = un ensemble de fichiers
/// VFS remplacés par une copie éditée par l'utilisatrice ; `nie-formats` n'a pas d'encodeur CPK,
/// donc ce registre ne modifie RIEN en place — il organise des copies destinées à l'export.
fn mods_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "mods + mod_files + recent_paths",
            kind: MigrationKind::Up,
            sql: r#"
                CREATE TABLE mods (
                    id          TEXT PRIMARY KEY,
                    name        TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    enabled     INTEGER NOT NULL DEFAULT 0,
                    priority    INTEGER NOT NULL DEFAULT 0,
                    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE TABLE mod_files (
                    id            INTEGER PRIMARY KEY AUTOINCREMENT,
                    mod_id        TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
                    vfs_path      TEXT NOT NULL,
                    staged_file   TEXT NOT NULL,
                    original_file TEXT,
                    staged_size   INTEGER,
                    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
                    UNIQUE(mod_id, vfs_path)
                );
                CREATE INDEX idx_mod_files_mod ON mod_files(mod_id);
                CREATE TABLE recent_paths (
                    path      TEXT PRIMARY KEY,
                    kind      TEXT NOT NULL,
                    opened_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
            "#,
        },
        Migration {
            // Index complet du VFS (~255 800 entrées, cf. `vfs_all_entries` + `src/lib/
            // vfsIndexDb.ts`) : matérialisé sur demande (« Réindexer » dans Paramètres), PAS au
            // démarrage. Objectif = PRÉCISION — `code` (basename sans extension, ex.
            // `c01000100`, `c01000100_5000`, `whs00010`) indexé pour une résolution EXACTE
            // (`code = ? OR code LIKE ?||'\_%'`) au lieu du `.contains()` substring en mémoire
            // de `vfs_related`, qui peut matcher un code apparaissant par hasard ailleurs dans
            // un chemin sans rapport (faux positif).
            version: 2,
            description: "vfs_files (index complet du VFS pour résolution précise par code)",
            kind: MigrationKind::Up,
            sql: r#"
                CREATE TABLE vfs_files (
                    path TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    ext  TEXT NOT NULL,
                    code TEXT NOT NULL,
                    cpk  TEXT NOT NULL,
                    size INTEGER NOT NULL
                );
                CREATE INDEX idx_vfs_files_code ON vfs_files(code);
                CREATE INDEX idx_vfs_files_ext ON vfs_files(ext);
                CREATE TABLE vfs_index_meta (
                    id          INTEGER PRIMARY KEY CHECK (id = 1),
                    total       INTEGER NOT NULL,
                    reindexed_at TEXT NOT NULL
                );
            "#,
        },
        Migration {
            // Journal DURABLE des jobs (§8 ROADMAP — « prochaine étape concrète »). Jusqu'ici la
            // progression d'une opération longue (réindexation du VFS, export `.cpk`) ne vivait
            // que dans un `useState` : fermer l'app ou changer d'onglet la perdait sans laisser
            // la moindre trace, et un job interrompu en cours de route était indiscernable d'un
            // job jamais lancé. Cette table est la trace ; le moteur d'exécution reste
            // `nie-tasks` côté Rust (cf. `vfs_index_scan_start`), elle ne le remplace pas.
            //
            // `status` ∈ running | done | error | canceled | interrupted — « interrupted » est
            // posé AU DÉMARRAGE sur tout job resté « running » d'une session précédente (cf.
            // `jobsDb.reconcileOnStartup`) : sans process pour le poursuivre, il ne peut pas
            // rester « en cours ».
            version: 3,
            description: "jobs (journal durable des opérations longues)",
            kind: MigrationKind::Up,
            sql: r#"
                CREATE TABLE jobs (
                    id         TEXT PRIMARY KEY,
                    kind       TEXT NOT NULL,
                    label      TEXT NOT NULL DEFAULT '',
                    status     TEXT NOT NULL DEFAULT 'running',
                    progress   INTEGER NOT NULL DEFAULT 0,
                    total      INTEGER NOT NULL DEFAULT 0,
                    error      TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX idx_jobs_status ON jobs(status);
                CREATE INDEX idx_jobs_created ON jobs(created_at DESC);
            "#,
        },
        Migration {
            // Compositions d'équipe du constructeur (vue Outils, portée depuis `/tools/my-team`
            // du wiki). Le site les enregistre côté serveur derrière `getServerSession` : sans
            // compte connecté, il n'en garde qu'UN brouillon en `localStorage`. Une application
            // de bureau n'a pas de session, mais elle a un disque — d'où cette table, qui rend
            // plusieurs compositions NOMMÉES persistantes sans réseau ni authentification.
            //
            // `members` est le JSON `Record<créneau, TeamMember>` de
            // `@rosegriffon/azalee/game/team-types` — la MÊME forme que le wiki persiste, pour
            // que le code de partage reste interchangeable entre les deux surfaces.
            version: 4,
            description: "teams (compositions d'équipe locales, sans session)",
            kind: MigrationKind::Up,
            sql: r"
                CREATE TABLE teams (
                    id           TEXT PRIMARY KEY,
                    name         TEXT NOT NULL,
                    formation_id TEXT NOT NULL,
                    members      TEXT NOT NULL DEFAULT '{}',
                    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX idx_teams_updated ON teams(updated_at DESC);
            ",
        },
    ]
}

/// Chemin passé en argument au lancement (« Ouvrir avec nie-explorer » depuis l'explorateur
/// Windows) — posé au cold-start, consommé une fois par le frontend via [`take_pending_open`].
struct PendingOpen(Mutex<Option<String>>);

/// Premier argument CLI qui ressemble à un chemin de fichier existant (ignore argv[0] et les
/// flags `-*` que Tauri/webview peuvent ajouter).
fn first_path_arg<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    args.into_iter()
        .skip(1)
        .find(|a| !a.starts_with('-') && std::path::Path::new(a).is_file())
}

/// Résout la racine du jeu à utiliser : `game_dir` explicite (réglage utilisatrice) sinon
/// [`resolve_game_dir_native`]. Pure — ne construit pas de VFS (cf. [`with_vfs`]).
fn resolve_root(game_dir: Option<&str>) -> PathBuf {
    match game_dir.filter(|s| !s.trim().is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => resolve_game_dir_native(),
    }
}

/// Résolution du dossier de jeu par défaut, dans l'ordre :
/// 1. `NIE_GAME_DIR` (env, dev/CI).
/// 2. Répertoire courant, s'il porte déjà des données montables — l'installation
///    (`data/cpk_list.cfg.bin` + `data/packs/`) OU un dump extrait (`data/common/`,
///    `data/dx11/`), cf. [`nie_formats::vfs::donnees_disponibles`].
/// 3. VRAIE détection Steam ([`steam::detect_game_dir`] — registre + `libraryfolders.vdf` +
///    `appmanifest_2799860.acf`), pas un chemin deviné.
/// 4. Un dump désigné par `NIE_DUMP_DIR` ou trouvé au-dessus du répertoire courant : sur une
///    machine sans installation, c'est la seule source de données, et l'explorateur sait
///    l'ouvrir depuis que le VFS sert les mêmes chemins logiques dans les deux montages.
/// 5. Repli : répertoire courant tel quel (même invalide) — plus honnête qu'un faux chemin
///    plausible : l'UI (`check_game_dir`) affichera clairement « introuvable » plutôt que de
///    pointer silencieusement vers un dossier qui n'existe sur aucune machine utilisatrice.
fn resolve_game_dir_native() -> PathBuf {
    if let Ok(dir) = std::env::var("NIE_GAME_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    if nie_formats::vfs::donnees_disponibles(cwd.join("data")) {
        return cwd;
    }
    if let Some(dir) = steam::detect_game_dir() {
        return dir;
    }
    // Le VFS prend `<racine>/data` : on remonte donc du `data/` du dump à sa racine.
    if let Some(parent) =
        nie_formats::vfs::resolve_dump_dir().and_then(|d| d.parent().map(PathBuf::from))
    {
        return parent;
    }
    cwd
}

/// VFS mis en cache dans l'état géré Tauri — construit UNE SEULE FOIS par racine résolue, puis
/// réutilisé par toutes les commandes. Avant ce cache, `open_vfs()` reconstruisait un `Vfs`
/// (déchiffrement + réindexation des ~255 800 entrées) à CHAQUE appel IPC, y compris un simple
/// clic de navigation dans l'Explorateur — cause réelle de la latence signalée. Précédé d'un
/// appel explicite [`preload_vfs`] au démarrage de l'appli pour amortir le premier coût avant
/// toute interaction utilisatrice.
/// Le VFS monté, partagé.
///
/// **`RwLock` et non `Mutex`**, et **`Arc<Vfs>` et non `Vfs`** — les deux comptent :
///
/// - Un `Mutex` sérialisait les **66** commandes qui passent par [`with_vfs`], pour toute la
///   durée de leur travail. Décoder une texture de plusieurs mégaoctets gelait donc le listage
///   d'un dossier, alors que les deux ne font que LIRE. `Vfs::read` prend `&self` et son cache
///   interne est déjà verrouillé de son côté : rien ne justifiait l'exclusivité.
/// - L'`Arc` permet de relâcher le verrou **avant** le travail lourd, et de confier celui-ci à
///   un thread bloquant. Sans lui, une commande longue garde le verrou pendant tout son calcul,
///   ce qui annule le bénéfice du `RwLock` dès qu'une écriture attend.
struct VfsState(RwLock<Option<(PathBuf, Arc<Vfs>)>>);

/// Budget par défaut du cache CPK **pour cette application**, en octets (1 Gio).
///
/// `nie-formats` monte à 16 Gio, et c'est cohérent pour un outil de traitement par lots qui a
/// la machine pour lui : garder les paquets chauds évite de relire 57 Go de CPK. Un
/// explorateur, lui, tourne à côté du jeu, d'un navigateur et d'un IDE ; le cache retient les
/// octets **bruts** de chaque paquet ouvert, si bien que quelques lectures dans des paquets
/// différents suffisent à retenir plusieurs centaines de mégaoctets — sans qu'aucune interface
/// ne le dise, et sans que le symptôme (la machine qui rame) n'accuse jamais le cache.
const BUDGET_CACHE_GUI: usize = 1024 * 1024 * 1024;

/// Abaisse le budget du cache CPK, **sauf** si l'utilisateur l'a fixé explicitement.
///
/// `NIE_CPK_CACHE_BUDGET_GIB` posée est un choix délibéré : on ne l'écrase pas. On ne corrige
/// que le défaut, qui n'a pas été pensé pour une application de bureau.
fn appliquer_budget_cache(vfs: &Vfs) {
    if std::env::var_os("NIE_CPK_CACHE_BUDGET_GIB").is_some() {
        return;
    }
    vfs.regler_budget_cache(BUDGET_CACHE_GUI);
}

/// Exécute `f` sur le VFS mis en cache pour `game_dir` (le (re)construit d'abord si la racine
/// résolue diffère de celle en cache, ou si aucun VFS n'a encore été chargé).
fn with_vfs<T>(
    game_dir: Option<String>,
    state: &VfsState,
    f: impl FnOnce(&Vfs) -> Result<T, String>,
) -> Result<T, String> {
    let vfs = vfs_partage(game_dir, state)?;
    f(&vfs)
}

/// Rend le VFS monté pour `game_dir`, **en relâchant le verrou avant de rendre la main**.
///
/// C'est ce qui rend la navigation non bloquante : l'appelant garde un `Arc` vivant et peut
/// travailler aussi longtemps qu'il veut — décoder une texture, extraire un fichier — sans
/// qu'aucune autre commande n'attende derrière lui.
///
/// Le montage lui-même reste exclusif, mais il est rare (une fois par racine).
fn vfs_partage(game_dir: Option<String>, state: &VfsState) -> Result<Arc<Vfs>, String> {
    let root = resolve_root(game_dir.as_deref());

    // Cas normal : déjà monté sur cette racine → verrou PARTAGÉ, plusieurs commandes à la fois.
    {
        let guard = state
            .0
            .read()
            .map_err(|_| "verrou VFS empoisonné".to_string())?;
        if let Some((cached_root, vfs)) = guard.as_ref() {
            if cached_root == &root {
                return Ok(Arc::clone(vfs));
            }
        }
    }

    // Montage : exclusif. Un autre thread a pu monter la même racine pendant qu'on attendait le
    // verrou — d'où la re-vérification, sans laquelle on remonterait le VFS pour rien.
    let mut guard = state
        .0
        .write()
        .map_err(|_| "verrou VFS empoisonné".to_string())?;
    if let Some((cached_root, vfs)) = guard.as_ref() {
        if cached_root == &root {
            return Ok(Arc::clone(vfs));
        }
    }

    let data_dir = root.join("data");
    let mut vfs = Vfs::new();
    // `init` monte l'installation, et bascule seule sur un dump extrait si `cpk_list.cfg.bin`
    // manque mais que l'arborescence est là. Le message d'échec doit donc nommer les DEUX
    // possibilités : « cpk_list introuvable » enverrait chercher un fichier là où c'est
    // l'ensemble du répertoire qui est vide.
    vfs.init(&data_dir).map_err(|e| {
        format!(
            "init VFS depuis {} : {e} — ce dossier ne porte ni installation \
             (cpk_list.cfg.bin + packs/) ni dump extrait (common/, dx11/)",
            data_dir.display()
        )
    })?;
    appliquer_budget_cache(&vfs);

    let partage = Arc::new(vfs);
    *guard = Some((root, Arc::clone(&partage)));
    Ok(partage)
}

/// Exécute un travail lourd sur le VFS **hors du thread principal**.
///
/// En Tauri v2, une commande synchrone s'exécute sur le thread principal : tant qu'elle
/// calcule, l'interface ne répond plus. Une commande déclarée `async` qui délègue ici rend la
/// main immédiatement, et son travail part sur un thread bloquant.
///
/// Le `Arc<Vfs>` est résolu **avant** le `spawn_blocking` : la résolution touche au verrou, le
/// travail non.
async fn sur_vfs_bloquant<T, F>(
    game_dir: Option<String>,
    state: &VfsState,
    f: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&Vfs) -> Result<T, String> + Send + 'static,
{
    let vfs = vfs_partage(game_dir, state)?;
    tauri::async_runtime::spawn_blocking(move || f(&vfs))
        .await
        .map_err(|e| format!("tâche VFS interrompue : {e}"))?
}

// `specta::Type` (en plus de `Serialize`) sur tous les DTOs qui traversent l'IPC : c'est ce qui
// permet à `tauri-specta` de régénérer `src/lib/bindings.ts` à partir des VRAIES signatures Rust
// (cf. `run()` → `specta_builder()`), au lieu du miroir manuel que `src/lib/api.ts` maintenait
// jusqu'ici (désynchronisable en silence à chaque commande ajoutée/modifiée).
#[derive(Serialize, Deserialize, specta::Type)]
struct EntryDto {
    path: String,
    name: String,
    size: u32,
    cpk: String,
}

/// Un sous-dossier direct et le nombre de fichiers qu'il porte, tous niveaux confondus.
///
/// Le compte vient du même balayage que le listing : il ne coûte rien de plus, et il évite à
/// l'interface de rappeler `vfs_ls` par dossier pour savoir lesquels sont vides.
#[derive(Serialize, specta::Type)]
struct DirDto {
    name: String,
    count: u32,
}

#[derive(Serialize, specta::Type)]
struct LsDto {
    dirs: Vec<DirDto>,
    files: Vec<EntryDto>,
    /// Nombre TOTAL de fichiers directs, avant pagination — le dénominateur d'une vue paginée.
    file_total: u32,
    /// Décalage effectivement appliqué aux `files`.
    file_offset: u32,
    /// Rôle du dossier courant (cf. `nie_explore::folder_roles`), `None` si non catalogué —
    /// jamais un rôle deviné : la table ne couvre que ce qui est sourcé/vérifié.
    role: Option<FolderRoleDto>,
}

/// Une page de résultats de recherche, avec le nombre total de correspondances.
///
/// `total` est le compte AVANT pagination : sans lui, une page de 2 000 résultats est
/// indiscernable d'un VFS qui n'en contient que 2 000.
#[derive(Serialize, specta::Type)]
struct FindPageDto {
    files: Vec<EntryDto>,
    total: u32,
    offset: u32,
}

impl From<nie_explore::listing::FileEntry> for EntryDto {
    fn from(f: nie_explore::listing::FileEntry) -> Self {
        EntryDto {
            path: f.path,
            name: f.name,
            size: f.size,
            cpk: f.cpk,
        }
    }
}

#[derive(Serialize, specta::Type)]
struct FolderRoleDto {
    role: String,
    status: String,
}

// `u32` (pas `usize`) : `specta-typescript` refuse d'exporter les entiers 64 bits (`usize`
// compris) par défaut — risque réel de perte de précision côté JS (`Number.MAX_SAFE_INTEGER`
// < 2⁶⁴). `u32` (≤ 4 294 967 295) couvre très largement des compteurs de fichiers (~255 800
// entrées VFS au total) sans avoir besoin de désactiver ce garde-fou.
#[derive(Serialize, specta::Type, Clone)]
struct StatsDto {
    /// Provenance des données : `"packs"` (installation : `cpk_list.cfg.bin` + `packs/*.cpk`) ou
    /// `"dump"` (arborescence déjà extraite). Les deux servent les mêmes chemins logiques, donc
    /// rien d'autre dans l'interface ne change — mais « 255 316 fichiers, 0 CPK » est
    /// incompréhensible tant qu'on ignore lequel des deux est monté.
    montage: String,
    total: u32,
    cpk_count: u32,
    extra_count: u32,
    loose_count: u32,
    top_ext: Vec<(String, u32)>,
}

/// Racine de jeu par défaut — VRAIE détection (registre Steam + bibliothèques +
/// `appmanifest_2799860.acf`, cf. [`resolve_game_dir_native`]), pas un chemin deviné.
#[tauri::command]
#[specta::specta]
fn default_game_dir() -> String {
    resolve_game_dir_native().display().to_string()
}

/// Vérifie qu'un répertoire de jeu est exploitable : `data/` y porte l'installation
/// (`cpk_list.cfg.bin`) **ou** un dump extrait (`common/`, `dx11/`). Les deux se montent et
/// servent les mêmes chemins — refuser le second afficherait « introuvable » sur une machine
/// qui a pourtant tout ce qu'il faut.
#[tauri::command]
#[specta::specta]
fn check_game_dir(game_dir: String) -> bool {
    nie_formats::vfs::donnees_disponibles(PathBuf::from(game_dir).join("data"))
}

/// Force le (re)chargement du VFS en cache — appelé une fois au démarrage du frontend pour
/// amortir le coût d'indexation AVANT la première navigation (cf. demande utilisatrice
/// « précharge le VFS au chargement pour éviter la latence ensuite »). Renvoie les mêmes
/// statistiques que [`vfs_stats`] pour un toast de confirmation côté UI.
#[tauri::command]
#[specta::specta]
async fn preload_vfs(
    game_dir: Option<String>,
    state: tauri::State<'_, VfsState>,
    cache: tauri::State<'_, StatsCache>,
) -> Result<StatsDto, String> {
    vfs_stats(game_dir, state, cache).await
}

/// Contenu direct d'un dossier du VFS, fichiers paginés et sous-dossiers comptés.
///
/// `limit`/`offset` sont facultatifs : `None` = tout le dossier (comportement historique).
/// `limit = 0` renvoie la structure et `file_total` SANS aucun fichier — ce que veut un arbre.
///
/// Le calcul lui-même vit dans [`nie_explore::listing::ls_paged`], partagé avec `niers vfs ls` et
/// le service HTTP `nie-model-serve` : le VFS étant un index plat, cette vue « dossier » est
/// calculée, et elle divergeait auparavant entre les trois façades.
#[tauri::command]
#[specta::specta]
fn vfs_ls(
    prefix: String,
    limit: Option<u32>,
    offset: Option<u32>,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<LsDto, String> {
    with_vfs(game_dir, &state, |vfs| {
        let l = limit.map_or(usize::MAX, |v| v as usize);
        let listing = nie_explore::listing::ls_paged(vfs, &prefix, l, offset.unwrap_or(0) as usize);
        Ok(LsDto {
            dirs: listing
                .dirs
                .into_iter()
                .map(|d| DirDto {
                    name: d.name,
                    count: d.count as u32,
                })
                .collect(),
            files: listing.files.into_iter().map(EntryDto::from).collect(),
            file_total: listing.file_total as u32,
            file_offset: listing.file_offset as u32,
            role: listing.role.map(|r| FolderRoleDto {
                role: r.role,
                status: r.status,
            }),
        })
    })
}

#[tauri::command]
#[specta::specta]
fn vfs_find(
    query: String,
    ext: Option<String>,
    limit: u32,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<EntryDto>, String> {
    with_vfs(game_dir, &state, |vfs| {
        let ext = ext.filter(|e| !e.is_empty());
        let hits = nie_explore::listing::find(vfs, &query, ext.as_deref(), limit.max(1) as usize);
        Ok(hits.into_iter().map(EntryDto::from).collect())
    })
}

/// Recherche paginée : la tranche demandée **et** le nombre total de correspondances.
///
/// [`vfs_find`] tronque à `limit` sans jamais dire combien il a laissé derrière lui — une
/// interface ne peut alors ni paginer ni annoncer « 200 sur 12 480 ».
#[tauri::command]
#[specta::specta]
fn vfs_find_paged(
    query: String,
    ext: Option<String>,
    limit: u32,
    offset: u32,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<FindPageDto, String> {
    with_vfs(game_dir, &state, |vfs| {
        let ext = ext.filter(|e| !e.is_empty());
        let r = nie_explore::listing::find_paged(
            vfs,
            &query,
            ext.as_deref(),
            limit as usize,
            offset as usize,
        );
        Ok(FindPageDto {
            files: r.files.into_iter().map(EntryDto::from).collect(),
            total: r.total as u32,
            offset: r.offset as u32,
        })
    })
}

/// Statistiques du VFS mémoïsées par racine.
///
/// Les calculer demande de parcourir l'index **entier** — 255 308 entrées — et de compter les
/// extensions une par une. Sur un montage « dump », ce parcours DÉCLENCHE en plus la
/// construction paresseuse de l'index, qui prend des minutes sur NTFS. Ce coût est justifié
/// une fois ; le payer à chaque appel (barre d'état, ouverture d'un onglet, retour sur
/// Paramètres) ne l'est pas.
///
/// L'invalidation suit la racine : un VFS remonté sur un autre dossier recalcule.
struct StatsCache(Mutex<Option<(PathBuf, StatsDto)>>);

#[tauri::command]
#[specta::specta]
async fn vfs_stats(
    game_dir: Option<String>,
    state: tauri::State<'_, VfsState>,
    cache: tauri::State<'_, StatsCache>,
) -> Result<StatsDto, String> {
    // `if let` imbriqués et non chaînés : ce crate est en édition 2021, où les « let chains »
    // ne compilent pas (contrairement à `nie-formats`, en 2024).
    let root = resolve_root(game_dir.as_deref());
    if let Ok(guard) = cache.0.lock() {
        if let Some((racine, stats)) = guard.as_ref() {
            if racine == &root {
                return Ok(stats.clone());
            }
        }
    }

    let stats = vfs_stats_calcul(game_dir, &state).await?;
    if let Ok(mut guard) = cache.0.lock() {
        *guard = Some((root, stats.clone()));
    }
    Ok(stats)
}

/// Le calcul réel, sans cache — appelé une fois par racine.
async fn vfs_stats_calcul(game_dir: Option<String>, state: &VfsState) -> Result<StatsDto, String> {
    sur_vfs_bloquant(game_dir, state, |vfs| {
        let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for (path, _) in vfs.iter() {
            let base = path.rsplit('/').next().unwrap_or(path);
            let ext = base
                .rsplit_once('.')
                .map(|(_, e)| e.to_lowercase())
                .unwrap_or_else(|| "<none>".to_string());
            *counts.entry(ext).or_default() += 1;
        }
        let mut top_ext: Vec<(String, u32)> = counts.into_iter().collect();
        top_ext.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
        top_ext.truncate(30);
        Ok(StatsDto {
            montage: if vfs.is_dump() { "dump" } else { "packs" }.to_string(),
            total: vfs.asset_count() as u32,
            cpk_count: vfs.cpk_count() as u32,
            extra_count: vfs.extra_count() as u32,
            loose_count: vfs.loose_count() as u32,
            top_ext,
        })
    })
    .await
}

/// Métadonnées d'une seule entrée VFS (`None` si le chemin n'existe pas) — sert notamment à
/// savoir si un fichier est "loose" (`cpk` vide) donc éditable EN PLACE via [`vfs_write_b64`],
/// sans devoir refaire transiter tout l'index (`vfs_ls`/`vfs_all_entries`) pour un seul chemin.
#[tauri::command]
#[specta::specta]
fn vfs_entry_meta(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Option<EntryDto>, String> {
    with_vfs(game_dir, &state, |vfs| {
        Ok(vfs.find(&path).map(|e| EntryDto {
            path: path.clone(),
            name: path.rsplit('/').next().unwrap_or(&path).to_string(),
            size: e.file_size,
            cpk: e.cpk_filename.clone(),
        }))
    })
}

/// Aperçu structuré d'une entrée : résumé par format ([`nie_explore::describe_content`]) +
/// les 64 premiers octets en hex (pour un magic visible même sans décodeur dédié).
#[tauri::command]
#[specta::specta]
fn vfs_describe(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<String>, String> {
    with_vfs(game_dir, &state, |vfs| {
        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        let mut lines = nie_explore::describe_content(&path, &data).unwrap_or_default();
        if lines.is_empty() {
            lines.push("format      brut / non reconnu".to_string());
        }
        lines.push(format!(
            "magic       {}",
            nie_explore::hex_prefix(&data, 16)
        ));
        lines.push(format!("taille      {} octets", data.len()));
        Ok(lines)
    })
}

/// Contenu brut, borné à `max_bytes` (défaut 2 MiB) pour rester raisonnable sur l'IPC JSON —
/// utiliser `vfs_extract_to` pour les gros fichiers (écriture disque directe côté Rust).
#[tauri::command]
#[specta::specta]
async fn vfs_read_b64(
    path: String,
    game_dir: Option<String>,
    max_bytes: Option<u32>,
    state: tauri::State<'_, VfsState>,
) -> Result<String, String> {
    sur_vfs_bloquant(game_dir, &state, move |vfs| {
        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        let cap = max_bytes.map(|b| b as usize).unwrap_or(2 * 1024 * 1024);
        if data.len() > cap {
            return Err(format!(
                "fichier trop volumineux pour l'aperçu ({} octets > {cap})",
                data.len()
            ));
        }
        Ok(base64::engine::general_purpose::STANDARD.encode(&data))
    })
    .await
}

/// Décode la meilleure texture d'un `.g4tx` en PNG (base64), pour un `<img>` côté UI.
///
/// **Pleine résolution** : réservé à l'APERÇU d'un fichier ouvert (un seul à l'écran). Pour une
/// grille de vignettes, utiliser [`vfs_texture_thumb_png_b64`] — cf. la note qui l'accompagne.
#[tauri::command]
#[specta::specta]
async fn vfs_texture_png_b64(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<'_, VfsState>,
) -> Result<String, String> {
    sur_vfs_bloquant(game_dir, &state, move |vfs| {
        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        let base = nie_formats::g4tx_decode::basename_of(&path).to_string();
        let png = nie_formats::g4tx_decode::decode_best_to_png(&data, &base)
            .ok_or("décodage PNG impossible (texture non reconnue)")?;
        Ok(base64::engine::general_purpose::STANDARD.encode(&png))
    })
    .await
}

/// Plus grand côté par défaut d'une vignette, en pixels. 128 : les grilles affichent au plus
/// ~90 px de large, et le double couvre les écrans à 2 dpr sans jamais servir une image
/// d'affiche pour un timbre-poste.
const VIGNETTE_COTE_DEFAUT: u32 = 128;

/// Borne haute acceptée pour `max_cote` — au-delà, l'appelant veut en réalité l'aperçu plein
/// format ([`vfs_texture_png_b64`]), et une « vignette » de 4096 px ramènerait exactement le
/// problème que cette commande existe pour supprimer.
const VIGNETTE_COTE_MAX: u32 = 512;

/// Décode un `.g4tx` en **vignette** PNG (base64), plus grand côté borné à `max_cote`
/// (défaut 128, plafond 512).
///
/// Distincte de [`vfs_texture_png_b64`] parce que l'usage est distinct : une grille de dossier
/// affiche des centaines d'images de moins de 90 px, et le VFS contient des dossiers de plus de
/// 12 000 textures (`data/dx11/menu/200_icon/10_icon_chr/uniform`). Servir la pleine résolution
/// à cette grille fait décoder 2048×2048 RGBA par entrée : mesuré sur cette machine, une seule
/// page de vignettes fait passer le processus de rendu WebView2 de 453 à 704 Mio, et le défilement
/// du dossier entier le tue. La réduction est faite ICI, avant l'IPC — la traverser en pleine
/// résolution pour réduire côté client ne réglerait rien.
#[tauri::command]
#[specta::specta]
async fn vfs_texture_thumb_png_b64(
    path: String,
    max_cote: Option<u32>,
    game_dir: Option<String>,
    state: tauri::State<'_, VfsState>,
) -> Result<String, String> {
    let cote = max_cote
        .unwrap_or(VIGNETTE_COTE_DEFAUT)
        .clamp(8, VIGNETTE_COTE_MAX);
    sur_vfs_bloquant(game_dir, &state, move |vfs| {
        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        // Le sélecteur de sous-texture d'un conteneur g4tx s'adresse par le basename du fichier
        // (cf. `g4tx_decode::decode_best_to_rgba`) : sans lui, un conteneur multi-textures rendrait
        // une région arbitraire au lieu de celle que le chemin demande.
        let base = nie_formats::g4tx_decode::basename_of(&path).to_string();
        // Isolé : un décodeur de texture qui déborde la pile ou panique sur un fichier atypique
        // ne doit pas emporter la fenêtre entière — une grille en parcourt des milliers.
        let png = isoler("décodage de vignette", move || {
            nie_formats::image_out::g4tx_vignette(
                &data,
                &base,
                cote,
                nie_formats::image_out::ImageOut::Png,
            )
        })??;
        Ok(base64::engine::general_purpose::STANDARD.encode(&png))
    })
    .await
}

/// Une texture nommée à l'intérieur d'un conteneur `.g4tx`.
#[derive(Serialize, specta::Type)]
struct TextureDto {
    /// Identifiant interne de la texture dans le conteneur.
    id: u32,
    /// Nom porté par le conteneur, ex. `eq_ac0100101` — c'est la clé d'adressage.
    name: String,
    width: u32,
    height: u32,
    /// Vrai si la texture porte son propre payload DDS (texture principale autonome).
    dds: bool,
    /// Taille du payload en octets.
    size: u32,
    /// Nombre de régions d'atlas définies SUR cette texture (0 = pas un atlas spatial).
    regions: u32,
}

/// Catalogue les textures d'un conteneur `.g4tx` — **sans en décoder aucune**.
///
/// Un conteneur IEVR n'est pas mono-texture : `icon_item05.g4tx` porte 80 payloads DDS 256×256
/// nommés (`eq_ac0100101`…), et les atlas spatiaux portent des régions nommées. Jusqu'ici
/// l'explorateur ne pouvait afficher qu'UNE image par fichier — celle que le basename désigne —
/// et les 79 autres étaient invisibles depuis l'application. Ce catalogue est ce qui permet à
/// l'interface de proposer un sélecteur, et il ne coûte qu'un parse d'en-tête.
#[tauri::command]
#[specta::specta]
fn vfs_texture_list(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<TextureDto>, String> {
    with_vfs(game_dir, &state, |vfs| {
        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        let g4tx = nie_formats::g4tx::parse(&data).map_err(|e| format!("G4TX illisible : {e}"))?;
        Ok(g4tx
            .textures
            .iter()
            .map(|t| TextureDto {
                id: u32::from(t.id),
                name: t.name.clone(),
                width: t.width.max(0) as u32,
                height: t.height.max(0) as u32,
                dds: t.is_dds,
                size: t.data_size as u32,
                regions: t.sub_textures.len() as u32,
            })
            .collect())
    })
}

/// Décode la texture **nommée** `nom` d'un conteneur `.g4tx` en PNG (base64), pleine résolution.
///
/// Forme nommée de [`vfs_texture_png_b64`], seule façon d'adresser une texture précise d'un
/// conteneur multi-textures ou une région d'atlas (cf. [`vfs_texture_list`]).
#[tauri::command]
#[specta::specta]
fn vfs_texture_named_png_b64(
    path: String,
    nom: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<String, String> {
    with_vfs(game_dir, &state, |vfs| {
        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        let png = isoler("décodage de texture nommée", move || {
            nie_formats::g4tx_decode::decode_named_to_png(&data, &nom)
        })?
        .ok_or("texture absente du conteneur, ou payload non décodable")?;
        Ok(base64::engine::general_purpose::STANDARD.encode(&png))
    })
}

/// Vignette d'une texture nommée — ce qu'une GRILLE de sous-textures doit appeler.
///
/// Même raison d'être que [`vfs_texture_thumb_png_b64`] : un conteneur d'icônes en porte 80, les
/// décoder en pleine résolution pour les afficher à 90 px sature le processus de rendu.
#[tauri::command]
#[specta::specta]
fn vfs_texture_named_thumb_png_b64(
    path: String,
    nom: String,
    max_cote: Option<u32>,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<String, String> {
    let cote = max_cote
        .unwrap_or(VIGNETTE_COTE_DEFAUT)
        .clamp(8, VIGNETTE_COTE_MAX);
    with_vfs(game_dir, &state, |vfs| {
        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        let png = isoler("décodage de vignette nommée", move || {
            nie_formats::image_out::g4tx_vignette_nommee(
                &data,
                &nom,
                cote,
                nie_formats::image_out::ImageOut::Png,
            )
        })??;
        Ok(base64::engine::general_purpose::STANDARD.encode(&png))
    })
}

/// Exécute `f` sur un thread dédié à pile large, et convertit une panique en `Err` au lieu de la
/// laisser remonter.
///
/// Deux dangers distincts, tous deux constatés dans ce dépôt :
/// * un **débordement de pile** natif (`STATUS_STACK_OVERFLOW`) tue le processus entier — il n'est
///   pas rattrapable, seule une pile suffisante l'évite (cf. `audio_wav_from_bytes`, où
///   `cridecoder` débordait la pile par défaut de Windows sur un `.awb` réel) ;
/// * une **panique** dans une commande laisse la promesse côté JS pendante pour toujours :
///   l'interface reste sur « chargement… » sans jamais rien dire.
///
/// Le coût est un `CreateThread` par appel (~100 µs), négligeable devant un décodage BC7.
pub(crate) fn isoler<T: Send + 'static>(
    quoi: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(f)
        .map_err(|e| format!("échec de lancement du thread ({quoi}) : {e}"))?
        .join()
        .map_err(|_| format!("{quoi} : le traitement a paniqué (thread dédié)"))
}

/// Extrait un fichier VFS directement vers `dest` (écriture Rust→disque, pas de round-trip JS).
///
/// Sur un montage **dump**, le fichier est déjà sur disque : on le copie au lieu de charger ses
/// octets en mémoire pour les réécrire. La différence se voit sur les gros assets — un `.usm`
/// dépasse les centaines de mégaoctets, et `read` + `write` en tenait deux exemplaires en RAM.
#[tauri::command]
#[specta::specta]
// `u32` (pas `u64`) pour toutes les tailles en octets retournées ci-dessous : même contrainte
// `specta-typescript` que `StatsDto` (pas d'entier 64 bits exporté), et même convention déjà en
// place pour `EntryDto.size`/`VfsEntry.file_size` — aucun asset individuel du jeu n'approche 4 Gio.
fn vfs_extract_to(
    path: String,
    dest: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<u32, String> {
    with_vfs(game_dir, &state, |vfs| {
        if let Some(parent) = std::path::Path::new(&dest).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if vfs.is_dump() {
            if let Some(source) = vfs.resolve_loose_path(&path) {
                let n = std::fs::copy(&source, &dest).map_err(|e| e.to_string())?;
                return Ok(n as u32);
            }
        }
        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        std::fs::write(&dest, &data).map_err(|e| e.to_string())?;
        Ok(data.len() as u32)
    })
}

// ─── Export au format voulu (cf. `export.rs`) ──────────────────────────────────────────

/// Formats d'export disponibles pour `path` — le brut en tête, puis les conversions réellement
/// possibles pour cette famille de fichiers. Dérivé du nom seul : aucun accès au CPK, donc
/// appelable à chaque changement de sélection.
#[tauri::command]
#[specta::specta]
fn vfs_export_formats(path: String) -> Vec<export::ExportFormatDto> {
    export::formats_pour(&path)
}

/// Nom de fichier proposé pour `path` exporté en `format` (`c01000010.g4tx` + `png` →
/// `c01000010.png`) — l'interface le passe au sélecteur de fichier en nom par défaut.
#[tauri::command]
#[specta::specta]
fn vfs_export_default_name(path: String, format: String) -> String {
    export::nom_propose(&path, &format)
}

/// Convertit une entrée du VFS vers `format` et l'écrit dans `dest`. Rend la taille écrite.
///
/// `format` vient de [`vfs_export_formats`] ; `"raw"` écrit les octets du jeu inchangés, ce que
/// faisait déjà `vfs_extract_to` (qui reste, appelé partout où aucun choix n'est offert).
#[tauri::command]
#[specta::specta]
fn vfs_export_as(
    path: String,
    dest: String,
    format: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<u32, String> {
    let root = resolve_root(game_dir.as_deref());
    let bytes = with_vfs(Some(root.display().to_string()), &state, |vfs| {
        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        export::produire(vfs, &path, data, &format)
    })?;
    if let Some(parent) = std::path::Path::new(&dest).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    Ok(bytes.len() as u32)
}

/// Résultat d'un export en lot : ce qui a été écrit, et ce qui a échoué **avec sa raison**.
///
/// Un lot ne s'arrête pas au premier échec : sur une sélection de 300 fichiers, une texture
/// factice non décodable ne doit pas priver l'utilisatrice des 299 autres.
#[derive(Serialize, specta::Type)]
struct ExportBatchDto {
    /// Nombre de fichiers écrits.
    ecrits: u32,
    /// Total des octets écrits.
    octets: u32,
    /// `(chemin, raison)` pour chaque fichier non exporté.
    echecs: Vec<(String, String)>,
}

/// Exporte plusieurs entrées du VFS vers le dossier `dest_dir`, chacune nommée d'après
/// [`vfs_export_default_name`]. Les fichiers dont la conversion échoue sont RAPPORTÉS, pas
/// silencieusement omis.
#[tauri::command]
#[specta::specta]
fn vfs_export_many(
    paths: Vec<String>,
    dest_dir: String,
    format: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<ExportBatchDto, String> {
    let root = resolve_root(game_dir.as_deref());
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    with_vfs(Some(root.display().to_string()), &state, |vfs| {
        let mut out = ExportBatchDto {
            ecrits: 0,
            octets: 0,
            echecs: Vec::new(),
        };
        for path in &paths {
            // Un format demandé pour un lot hétérogène ne vaut pas pour tout le monde (un `png`
            // sur un `.awb`) : on retombe sur le brut plutôt que d'échouer sur chaque entrée.
            let effectif = if export::formats_pour(path).iter().any(|f| f.id == format) {
                format.as_str()
            } else {
                "raw"
            };
            let r = vfs
                .read(path)
                .map_err(|e| e.to_string())
                .and_then(|data| export::produire(vfs, path, data, effectif))
                .and_then(|bytes| {
                    let dest =
                        std::path::Path::new(&dest_dir).join(export::nom_propose(path, effectif));
                    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
                    Ok(bytes.len() as u32)
                });
            match r {
                Ok(n) => {
                    out.ecrits += 1;
                    out.octets = out.octets.saturating_add(n);
                }
                Err(e) => out.echecs.push((path.clone(), e)),
            }
        }
        Ok(out)
    })
}

/// Écrit `data_b64` EN PLACE sur un fichier VFS "loose" (physiquement présent sur disque sous
/// `<jeu>/<chemin>`, PAS empaqueté dans un CPK — `entry.cpk` vide côté `EntryDto`/`VfsEntry`,
/// cf. `Vfs::read` § « CPK vide → fichier loose ») — contrairement à [`vfs_extract_to`]/
/// [`save_bytes_b64`] qui exportent toujours vers une destination choisie par l'utilisatrice.
/// Refuse explicitement les entrées empaquetées dans un CPK : `nie-formats` n'a pas d'encodeur
/// CPK, y écrire corromprait l'archive — même contrainte que partout ailleurs dans ce fichier,
/// vérifiée ICI plutôt que suppposée (le VFS sait exactement quelles entrées sont loose).
///
/// Sur un montage **dump**, aucune entrée n'est empaquetée : l'écriture en place vaut pour tout
/// le contenu, et modifie le dump lui-même — c'est une arborescence de travail, pas une archive.
#[tauri::command]
#[specta::specta]
fn vfs_write_b64(
    path: String,
    data_b64: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<u32, String> {
    let root = resolve_root(game_dir.as_deref());
    with_vfs(Some(root.display().to_string()), &state, |vfs| {
        let entry = vfs
            .find(&path)
            .ok_or_else(|| format!("fichier VFS introuvable : {path}"))?;
        if !entry.cpk_filename.is_empty() {
            return Err(format!(
                "« {path} » est empaqueté dans {} — nie-formats n'a pas d'encodeur CPK, écriture \
                 en place impossible. Utilisez « Enregistrer sous… » pour exporter une copie externe.",
                entry.cpk_filename
            ));
        }
        let data = base64::engine::general_purpose::STANDARD
            .decode(data_b64.as_bytes())
            .map_err(|e| e.to_string())?;
        // Chemin interne VFS (`data/common/...`) déjà relatif à la racine du jeu (pas au dossier
        // `data/` lui-même) — même formule que `Vfs::read` pour une entrée loose enregistrée dans
        // `cpk_list.cfg.bin` (`game_data_dir.join(strip_prefix("data/"))`, avec
        // `game_data_dir = <racine>/data`, ce qui revient exactement à `<racine>.join(path)`).
        write_loose_bytes(&root, &path, &data)
    })
}

/// Écrit `data_b64` comme fichier "loose" AU MÊME CHEMIN qu'une entrée normalement empaquetée
/// dans un CPK — contournement de l'absence d'encodeur CPK, PAS une écriture confirmée : le
/// comportement réel de `nie.exe` face à un fichier loose à la place d'un CPK-packed n'est **pas
/// confirmé par rétro-ingénierie** (même incertitude déjà documentée pour l'export de mod
/// « overlay loose-file » dans `modWorkspace.ts`/`exportMod`). Le jeu peut tout simplement
/// ignorer ce fichier et continuer à lire le CPK. Confirmation explicite déjà exigée côté UI
/// avant l'appel (EAC présent sur cette installation).
#[tauri::command]
#[specta::specta]
fn vfs_write_loose_override_b64(
    path: String,
    data_b64: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<u32, String> {
    let root = resolve_root(game_dir.as_deref());
    with_vfs(Some(root.display().to_string()), &state, |vfs| {
        vfs.find(&path)
            .ok_or_else(|| format!("fichier VFS introuvable : {path}"))?;
        let data = base64::engine::general_purpose::STANDARD
            .decode(data_b64.as_bytes())
            .map_err(|e| e.to_string())?;
        write_loose_bytes(&root, &path, &data)
    })
}

/// Écrit `data` à `<root>/<path>` (même formule de chemin que [`vfs_write_b64`]) — factorisé
/// entre l'écriture "loose" normale et l'override loose d'une entrée normalement CPK-packed.
fn write_loose_bytes(root: &std::path::Path, path: &str, data: &[u8]) -> Result<u32, String> {
    let disk_path = root.join(path);
    if let Some(parent) = disk_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&disk_path, data).map_err(|e| e.to_string())?;
    Ok(data.len() as u32)
}

/// Écrit des octets édités (base64, depuis l'éditeur hex de l'UI) vers `dest` — n'écrit
/// JAMAIS dans un CPK : `nie-formats` n'a pas d'encodeur CPK, donc « éditer » un asset du
/// jeu produit toujours une copie externe, jamais une modification en place des packs.
#[tauri::command]
#[specta::specta]
fn save_bytes_b64(dest: String, data_b64: String) -> Result<u32, String> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(data_b64.as_bytes())
        .map_err(|e| e.to_string())?;
    if let Some(parent) = std::path::Path::new(&dest).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&dest, &data).map_err(|e| e.to_string())?;
    Ok(data.len() as u32)
}

// ─── Sauvegardes (Lives, `nie-save`) ────────────────────────────────────────────────

/// Sauvegarde actuellement ouverte (déchiffrée en mémoire, jamais persistée telle quelle) —
/// `Some((conteneur, chemin d'origine))` après [`save_open`].
struct SaveState(Mutex<Option<(nie_save::LivesContainer, PathBuf)>>);

#[derive(Serialize, specta::Type)]
struct SaveBlobDto {
    filename: String,
    subtype: String,
    size: u32,
}

/// Déchiffre + parse un fichier de sauvegarde Lives (ex. `002AB8F4-USERDATALIVE`) et renvoie
/// son résumé (`nie_save::SaveSummary`, sérialisé tel quel — joueur, niveau, temps de jeu,
/// roster…). Le conteneur déchiffré reste en mémoire pour [`save_list_blobs`]/[`save_export`].
/// Auto-détecte LA meilleure sauvegarde Steam Cloud (`userdata/<steamid>/2799860/remote/*-
/// USERDATALIVE`, cf. `steam::pick_best_save`) — `None` si Steam/le jeu/toute sauvegarde valide
/// est absent de ce poste (jamais un chemin deviné). Le frontend (`SaveView`) l'appelle au
/// montage et n'ouvre le sélecteur manuel qu'en repli, au lieu d'un `open()` systématique.
#[tauri::command]
#[specta::specta]
fn default_save_path() -> Option<String> {
    steam::pick_best_save(|p| nie_save::io::read_save(p).is_ok()).map(|p| p.display().to_string())
}

#[tauri::command]
#[specta::specta]
fn save_open(path: String, state: tauri::State<SaveState>) -> Result<RawJson, String> {
    let container =
        nie_save::io::read_save(std::path::Path::new(&path)).map_err(|e| e.to_string())?;
    let summary = nie_save::summarize(&container);
    let json = serde_json::to_value(&summary).map_err(|e| e.to_string())?;
    *state.0.lock().unwrap() = Some((container, PathBuf::from(path)));
    Ok(RawJson(json))
}

#[tauri::command]
#[specta::specta]
fn save_list_blobs(state: tauri::State<SaveState>) -> Result<Vec<SaveBlobDto>, String> {
    let guard = state.0.lock().unwrap();
    let (container, _) = guard.as_ref().ok_or("aucune sauvegarde ouverte")?;
    Ok(container
        .entries
        .iter()
        .zip(&container.blobs)
        .map(|(e, b)| SaveBlobDto {
            filename: e.filename.clone(),
            subtype: format!("{:?}", b.header.subtype),
            size: b.body.len() as u32,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
fn save_blob_hex_b64(index: u32, state: tauri::State<SaveState>) -> Result<String, String> {
    let guard = state.0.lock().unwrap();
    let (container, _) = guard.as_ref().ok_or("aucune sauvegarde ouverte")?;
    let blob = container
        .blobs
        .get(index as usize)
        .ok_or("index de blob invalide")?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&blob.body))
}

/// Écrit un texte (CSV/JSON/Markdown) à un chemin choisi par l'utilisatrice via la boîte de
/// dialogue système. Le plugin `fs` du front est cantonné aux dossiers de l'application
/// (`fs:allow-app-write*`, cf. `capabilities/`) : sans cette commande, aucun export de l'app ne
/// peut atterrir ailleurs que dans `%APPDATA%`, ce qui rend un « Exporter… » inutilisable.
/// Retourne le nombre d'octets écrits (`f64` : `specta` refuse les BigInt vers TypeScript).
#[tauri::command]
#[specta::specta]
fn write_text_file(dest: String, contents: String) -> Result<f64, String> {
    std::fs::write(&dest, contents.as_bytes()).map_err(|e| format!("écriture {dest} : {e}"))?;
    Ok(contents.len() as f64)
}

/// Ré-encode le conteneur actuellement ouvert (round-trip octet-identique si rien n'a été
/// modifié) et l'écrit à `dest` — jamais l'original en place (choisi par l'utilisatrice).
#[tauri::command]
#[specta::specta]
fn save_export(dest: String, state: tauri::State<SaveState>) -> Result<u32, String> {
    let guard = state.0.lock().unwrap();
    let (container, _) = guard.as_ref().ok_or("aucune sauvegarde ouverte")?;
    let bytes = container.encrypt().map_err(|e| e.to_string())?;
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    Ok(bytes.len() as u32)
}

// La recherche chara/waza (miroir wiki) est faite CÔTÉ FRONTEND via tauri-plugin-sql
// (`src/lib/wikiDb.ts`, mêmes requêtes SQL que `nie-wiki::query::search_characters`/
// `search_skills`) — pas de commande Rust ici : `nie-wiki` (rusqlite) est volontairement HORS
// de ce binaire (conflit de lien natif `sqlite3` avec `sqlx-sqlite`, cf. Cargo.toml).

/// Racines où chercher les artefacts du **dépôt** (miroir wiki, base RE), dans l'ordre : la
/// racine du jeu (une installation peut porter son propre `var/`), puis le répertoire courant et
/// chacun de ses ancêtres, puis le répertoire de l'exécutable et ses ancêtres.
///
/// Les deux dernières familles sont ce qui fait fonctionner l'application **installée** : son
/// `.exe` vit dans `Program Files`, `NIE_GAME_DIR` pointe vers l'install Steam, et aucun des deux
/// ne porte le `var/` du dépôt. Lancée depuis `target/release`, la remontée d'ancêtres retrouve
/// en revanche `<dépôt>/var` — c'est le cas du poste de développement.
fn racines_candidates(game_dir: Option<&str>) -> Vec<PathBuf> {
    let mut racines = vec![resolve_root(game_dir)];
    let mut ajouter_ancetres = |depart: Option<PathBuf>| {
        let mut cur = depart;
        while let Some(dir) = cur {
            if !racines.contains(&dir) {
                racines.push(dir.clone());
            }
            cur = dir.parent().map(std::path::Path::to_path_buf);
        }
    };
    ajouter_ancetres(std::env::current_dir().ok());
    ajouter_ancetres(
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf)),
    );
    racines
}

/// Miroir wiki sous une racine donnée, par ordre de préférence :
/// 1. `var/mirror.sqlite` — le nom canonique de `@niers/catalog` (un lien vers l'instantané
///    courant, rebasculé atomiquement par `scripts/donnees/miroir-inagle.sh`).
/// 2. `var/miroir/inagle-*.sqlite` — l'instantané daté le plus récent, si le lien manque.
/// 3. `var/wiki-mirror/supabase-*.sqlite`, puis `data/backups/supabase-*.sqlite` — les deux
///    emplacements historiques, conservés pour les postes qui les portent encore.
fn miroir_wiki_sous(racine: &std::path::Path) -> Option<PathBuf> {
    let var = racine.join("var");
    let lien = var.join("mirror.sqlite");
    if lien.is_file() {
        return Some(lien);
    }
    dernier_sqlite(&var.join("miroir"), "inagle-")
        .or_else(|| dernier_sqlite(&var.join("wiki-mirror"), "supabase-"))
        .or_else(|| dernier_sqlite(&racine.join("data").join("backups"), "supabase-"))
}

/// Résout le miroir SQLite du wiki (tables `inagle_*`) par défaut. Renvoie `None` si rien n'est
/// trouvé — jamais un chemin deviné : le champ « Base SQLite » des Paramètres reste alors vide,
/// à renseigner manuellement.
///
/// Ordre : `NIE_WIKI_DB`/`SQLITE_DB_PATH`, puis les bases **livrées avec l'application**
/// ([`bases_embarquees`] — c'est ce qui donne une expérience complète à une utilisatrice qui n'a
/// ni le dépôt ni le jeu), puis les emplacements du dépôt ([`miroir_wiki_sous`]).
#[tauri::command]
#[specta::specta]
fn default_wiki_db(app: tauri::AppHandle, game_dir: Option<String>) -> Option<String> {
    for var in ["NIE_WIKI_DB", "SQLITE_DB_PATH"] {
        if let Ok(v) = std::env::var(var) {
            if PathBuf::from(&v).is_file() {
                return Some(v);
            }
        }
    }
    for base in bases_embarquees(&app, "mirror.sqlite") {
        if base.is_file() {
            return Some(base.display().to_string());
        }
    }
    racines_candidates(game_dir.as_deref())
        .iter()
        .find_map(|r| miroir_wiki_sous(r))
        .map(|p| p.display().to_string())
}

/// Résout `var/niers.sqlite` (base RE — fonctions/classes RTTI/xrefs labellisées par `nie-re`,
/// cf. `src/lib/reDb.ts`). Commande Rust plutôt qu'un `exists()` JS (`@tauri-apps/plugin-fs`) :
/// la portée `fs:scope` de l'app ne couvre que `$APPDATA`, un `std::fs` Rust n'a pas cette
/// restriction — même raison que [`default_wiki_db`] au-dessus.
///
/// Même ordre que le miroir wiki : `NIE_RE_DB`, bases livrées avec l'application, puis le dépôt.
#[tauri::command]
#[specta::specta]
fn default_re_db(app: tauri::AppHandle, game_dir: Option<String>) -> Option<String> {
    if let Ok(v) = std::env::var("NIE_RE_DB") {
        if PathBuf::from(&v).is_file() {
            return Some(v);
        }
    }
    for base in bases_embarquees(&app, "niers.sqlite") {
        if base.is_file() {
            return Some(base.display().to_string());
        }
    }
    racines_candidates(game_dir.as_deref())
        .iter()
        .map(|r| r.join("var").join("niers.sqlite"))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
}

/// Résout `data/anime/episodes.db` — le catalogue des épisodes de la série (10 saisons, 355
/// épisodes avec vignettes), alimenté par `packages/ietv` et sa tâche `ietv-cache`.
///
/// C'est le quatrième gisement de `docs/FUSION.md` (`anime`), et la vue Cinéma le présente à côté
/// des cinématiques du jeu. Même ordre de résolution que les deux autres bases : `NIE_ANIME_DB`,
/// bases livrées avec l'application, puis le dépôt.
#[tauri::command]
#[specta::specta]
fn default_anime_db(app: tauri::AppHandle, game_dir: Option<String>) -> Option<String> {
    if let Ok(v) = std::env::var("NIE_ANIME_DB") {
        if PathBuf::from(&v).is_file() {
            return Some(v);
        }
    }
    for base in bases_embarquees(&app, "episodes.db") {
        if base.is_file() {
            return Some(base.display().to_string());
        }
    }
    racines_candidates(game_dir.as_deref())
        .iter()
        .map(|r| r.join("data").join("anime").join("episodes.db"))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
}

/// Emplacements d'une base **livrée avec l'application**, dans l'ordre de préférence :
/// le cache de données de l'app (`$APPDATA/db/<nom>` — où [`installer_bases_embarquees`] a
/// décompressé la base au premier lancement), puis les ressources empaquetées telles quelles
/// (`<resources>/db/<nom>`, cas d'une base livrée non compressée).
///
/// Le cache passe en premier : une base décompressée est ouvrable en écriture (WAL), là où les
/// ressources d'un MSI vivent sous `Program Files` en lecture seule.
fn bases_embarquees(app: &tauri::AppHandle, nom: &str) -> Vec<PathBuf> {
    use tauri::Manager as _;
    let mut chemins = Vec::new();
    if let Ok(dir) = app.path().app_data_dir() {
        chemins.push(dir.join("db").join(nom));
    }
    for source in dossiers_ressources(app) {
        chemins.push(source.join(nom));
    }
    chemins
}

/// Où le bundler dépose `resources/db/*.gz`.
///
/// **Tauri conserve le chemin relatif déclaré dans `bundle.resources`** : `"resources/db/*.gz"`
/// atterrit donc en `<resource_dir>/resources/db/`, et non en `<resource_dir>/db/`. La première
/// version de ce code visait le second — les archives étaient bien dans le paquet, le dossier
/// `$APPDATA/db/` ne s'est jamais créé, et rien ne le disait : la résolution retombait simplement
/// sur le dépôt, qui existe sur cette machine mais sur aucune machine utilisatrice.
///
/// Les deux formes sont essayées, la déclarée d'abord : `resource_dir()` lui-même varie (dossier
/// de l'exécutable hors bundle, `<install>/resources` pour un MSI).
fn dossiers_ressources(app: &tauri::AppHandle) -> Vec<PathBuf> {
    use tauri::Manager as _;
    let Ok(dir) = app.path().resource_dir() else {
        return Vec::new();
    };
    vec![dir.join("resources").join("db"), dir.join("db")]
}

/// Décompresse les bases livrées avec l'application (`<resources>/db/*.sqlite.gz`) vers
/// `$APPDATA/db/`, une fois par version de ressource. Renvoie les bases effectivement installées.
///
/// C'est ce qui rend l'application autonome : une utilisatrice qui n'a ni le dépôt, ni le jeu, ni
/// le VPS ouvre le wiki (6 166 personnages) et la base RE dès le premier lancement. Les bases
/// voyagent compressées (66 Mo → 7,9 Mo pour le miroir, 74 Mo → 22,4 Mo pour la base RE) et sont
/// décompressées **hors** de `Program Files` : SQLite doit pouvoir écrire son `-wal` à côté du
/// fichier, ce que les ressources d'un MSI, en lecture seule, interdisent.
///
/// Le témoin `<nom>.source` porte la taille de l'archive d'origine : une release qui embarque une
/// base plus récente le change, donc la décompression est refaite ; un lancement ordinaire ne
/// recopie rien.
fn installer_bases_embarquees(app: &tauri::AppHandle) -> Vec<PathBuf> {
    use tauri::Manager as _;
    let Ok(data_dir) = app.path().app_data_dir() else {
        return Vec::new();
    };
    let cible = data_dir.join("db");
    // Le premier dossier de ressources qui existe réellement — cf. [`dossiers_ressources`] pour
    // la raison d'en essayer deux.
    let Some(entrees) = dossiers_ressources(app)
        .into_iter()
        .find_map(|d| std::fs::read_dir(d).ok())
    else {
        log::error!("bases embarquées : aucun dossier de ressources lisible");
        return Vec::new();
    };
    let mut installees = Vec::new();
    for archive in entrees.flatten().map(|e| e.path()) {
        if archive.extension().is_none_or(|e| e != "gz") {
            continue;
        }
        let Some(nom) = archive
            .file_stem()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        let Ok(taille) = archive.metadata().map(|m| m.len()) else {
            continue;
        };
        let base = cible.join(&nom);
        let temoin = cible.join(format!("{nom}.source"));
        let deja = std::fs::read_to_string(&temoin)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok());
        if base.is_file() && deja == Some(taille) {
            installees.push(base);
            continue;
        }
        if let Err(e) = decompresser_gz(&archive, &base, &cible) {
            log::error!("base embarquée {nom} : {e}");
            continue;
        }
        let _ = std::fs::write(&temoin, taille.to_string());
        log::info!("base embarquée installée : {}", base.display());
        installees.push(base);
    }
    installees
}

/// Décompresse `archive` (gzip) vers `dest`, en passant par un fichier temporaire du même
/// répertoire : une décompression interrompue (coupure, disque plein) ne laisse jamais une base
/// tronquée que le lancement suivant prendrait pour valide, puisque le renommage final est
/// atomique et que le témoin n'est écrit qu'après.
fn decompresser_gz(
    archive: &std::path::Path,
    dest: &std::path::Path,
    dossier: &std::path::Path,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dossier)?;
    let tmp = dest.with_extension("part");
    {
        let entree = std::fs::File::open(archive)?;
        let mut lecteur = flate2::read::GzDecoder::new(std::io::BufReader::new(entree));
        let mut sortie = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        std::io::copy(&mut lecteur, &mut sortie)?;
        std::io::Write::flush(&mut sortie)?;
    }
    // Un `rename` sur un fichier existant échoue sous Windows : retirer la cible d'abord.
    let _ = std::fs::remove_file(dest);
    std::fs::rename(&tmp, dest)
}

/// Fichier `<prefixe>*.sqlite` non-vide le plus récent (tri lexicographique DESC — les noms
/// portent un horodatage ISO 8601, donc l'ordre lexicographique = l'ordre chronologique) —
/// même algorithme que `nie_wiki::mirror::latest_sqlite_in`.
fn dernier_sqlite(dir: &std::path::Path, prefixe: &str) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|ext| ext == "sqlite")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(prefixe))
                && p.metadata().is_ok_and(|m| m.len() > 0)
        })
        .collect();
    entries.sort();
    entries.into_iter().next_back()
}

/// Scanne la totalité du VFS (~255 800 entrées) — utilisé par `vfsIndexDb.reindex` (frontend)
/// pour matérialiser un index SQL persistant (`vfs_files`, table gérée par `tauri-plugin-sql`)
/// permettant une résolution EXACTE par code interne (segment de chemin), plus précise que le
/// `.contains()` substring en mémoire de [`vfs_related`] (qui peut matcher un code interne
/// apparaissant par hasard ailleurs dans un chemin non lié).
#[tauri::command]
#[specta::specta]
async fn vfs_all_entries(
    game_dir: Option<String>,
    state: tauri::State<'_, VfsState>,
) -> Result<Vec<EntryDto>, String> {
    sur_vfs_bloquant(game_dir, &state, |vfs| {
        Ok(vfs
            .iter()
            .map(|(path, entry)| EntryDto {
                path: path.to_string(),
                name: path.rsplit('/').next().unwrap_or(path).to_string(),
                size: entry.file_size,
                cpk: entry.cpk_filename.clone(),
            })
            .collect())
    })
    .await
}

// ─── Scan VFS annulable/pausable avec progression (nie-tasks) ─────────────────────────
//
// Remplace, pour l'usage `vfsIndexDb.reindex` (bouton « Réindexer » de Paramètres), l'appel
// opaque [`vfs_all_entries`] par un job [`nie_tasks::Task`] chunké : la collecte en mémoire
// elle-même reste synchrone (déjà rapide, cf. commentaire de [`vfs_all_entries`]) mais son
// ÉMISSION est désormais incrémentale et annulable — ce que ne permettait aucun `#[tauri::command]`
// à réponse unique. `vfs_all_entries` reste en place (compat, autres appelants potentiels).

/// État géré du système de jobs — un seul `TaskSystem` pour toute l'appli (pas seulement le scan
/// VFS : tout futur job long de nie-explorer peut s'y greffer). `results` retient la sortie JSON
/// des jobs terminés jusqu'à ce que le frontend la récupère via [`vfs_index_scan_take`].
struct VfsScanState {
    system: nie_tasks::TaskSystem<String>,
    results: Arc<Mutex<std::collections::HashMap<nie_tasks::TaskId, serde_json::Value>>>,
}

/// Job : ré-émet en lots de [`Self::CHUNK`] entrées déjà collectées (voir [`vfs_index_scan_start`]),
/// avec un point de contrôle annulation/pause à chaque lot — `ctx.progress.report` renseigne la
/// barre de progression du frontend (`vfs-index-progress`, relayé par le lecteur de `run()`).
struct VfsScanTask {
    id: nie_tasks::TaskId,
    entries: Vec<EntryDto>,
}

impl VfsScanTask {
    const CHUNK: usize = 8_000;
}

#[async_trait::async_trait]
impl nie_tasks::Task<String> for VfsScanTask {
    fn id(&self) -> nie_tasks::TaskId {
        self.id
    }

    async fn run(&mut self, ctx: &nie_tasks::TaskContext) -> Result<nie_tasks::ExecStatus, String> {
        let total = self.entries.len() as u64;
        let mut done = 0u64;
        for chunk in self.entries.chunks(Self::CHUNK) {
            ctx.interrupter
                .check()
                .await
                .map_err(|_| "scan VFS annulé".to_string())?;
            done += chunk.len() as u64;
            ctx.progress.report(done, total, None);
            // Cède la main au runtime tokio entre deux lots — sans quoi la boucle (rapide,
            // purement mémoire) tournerait d'une traite et la progression n'aurait aucun sens
            // observable côté frontend (tous les événements arriveraient d'un coup à la fin).
            tokio::task::yield_now().await;
        }
        let value = serde_json::to_value(&self.entries).map_err(|e| e.to_string())?;
        Ok(nie_tasks::ExecStatus::Done(value))
    }
}

/// Avancement relayé au frontend (événement `vfs-index-progress`) — miroir JSON de
/// [`nie_tasks::TaskProgress`], en `u32` (pas `usize`/`u64`, cf. convention `specta-typescript`
/// documentée sur [`EntryDto`]) et avec l'identifiant de job en `String` (UUID).
#[derive(Serialize, specta::Type, Clone)]
struct VfsIndexProgressDto {
    task_id: String,
    done: u32,
    total: u32,
}

fn parse_task_id(task_id: &str) -> Result<nie_tasks::TaskId, String> {
    uuid::Uuid::parse_str(task_id)
        .map(nie_tasks::TaskId)
        .map_err(|e| format!("task_id invalide : {e}"))
}

/// Démarre le scan complet du VFS en tâche de fond et renvoie immédiatement son `TaskId` (UUID) —
/// la collecte des ~255 800 entrées reste synchrone (même coût que [`vfs_all_entries`]) mais leur
/// émission par lots est annulable ([`vfs_index_scan_cancel`]) et suivie en direct par
/// l'événement `vfs-index-progress`. Le résultat final se récupère par [`vfs_index_scan_take`]
/// une fois l'événement `vfs-index-done` reçu.
#[tauri::command]
#[specta::specta]
async fn vfs_index_scan_start(
    game_dir: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, VfsState>,
    scan: tauri::State<'_, VfsScanState>,
) -> Result<String, String> {
    // `async` n'est pas ici une optimisation, c'est une CORRECTION DE PLANTAGE.
    //
    // `TaskSystem::dispatch` appelle `tokio::spawn`, qui exige un runtime Tokio. En Tauri v2,
    // une commande synchrone s'exécute sur le thread principal, hors de ce runtime : l'appel
    // paniquait donc en « there is no reactor running », et comme ce panic traverse une
    // frontière qui ne peut pas se dérouler, il devenait un `STATUS_STACK_BUFFER_OVERRUN` —
    // l'application entière s'abattait, sans message exploitable pour l'utilisateur.
    //
    // Déclarée `async`, la commande s'exécute dans le runtime, et `dispatch` y trouve son
    // réacteur. Le parcours de l'index (255 308 entrées) passe au passage hors du thread
    // principal, ce qui était de toute façon nécessaire.
    let entries = sur_vfs_bloquant(game_dir, &state, |vfs| {
        Ok(vfs
            .iter()
            .map(|(path, entry)| EntryDto {
                path: path.to_string(),
                name: path.rsplit('/').next().unwrap_or(path).to_string(),
                size: entry.file_size,
                cpk: entry.cpk_filename.clone(),
            })
            .collect::<Vec<_>>())
    })
    .await?;

    let id = nie_tasks::TaskId::new();
    let handle = scan.system.dispatch(VfsScanTask { id, entries });
    let results = Arc::clone(&scan.results);

    tauri::async_runtime::spawn(async move {
        match handle.wait().await {
            nie_tasks::TaskStatus::Done(value) => {
                results
                    .lock()
                    .expect("verrou résultats de scan empoisonné")
                    .insert(id, value);
                let _ = app.emit("vfs-index-done", id.to_string());
            }
            nie_tasks::TaskStatus::Canceled => {
                let _ = app.emit("vfs-index-canceled", id.to_string());
            }
            nie_tasks::TaskStatus::Error(e) => {
                let _ = app.emit("vfs-index-error", format!("{id}: {e}"));
            }
        }
    });

    Ok(id.to_string())
}

/// Annule un scan en cours ([`vfs_index_scan_start`]) — no-op silencieux s'il est déjà terminé.
#[tauri::command]
#[specta::specta]
fn vfs_index_scan_cancel(task_id: String, scan: tauri::State<VfsScanState>) -> Result<(), String> {
    scan.system.cancel(parse_task_id(&task_id)?);
    Ok(())
}

/// Récupère et consomme (retire du registre) le résultat d'un scan terminé (`vfs-index-done`
/// reçu) — erreur explicite si appelé trop tôt ou avec un `task_id` déjà consommé/inconnu.
#[tauri::command]
#[specta::specta]
fn vfs_index_scan_take(
    task_id: String,
    scan: tauri::State<VfsScanState>,
) -> Result<Vec<EntryDto>, String> {
    let id = parse_task_id(&task_id)?;
    let value = scan
        .results
        .lock()
        .expect("verrou résultats de scan empoisonné")
        .remove(&id)
        .ok_or_else(|| {
            "résultat de scan introuvable (pas encore prêt, ou déjà consommé)".to_string()
        })?;
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Chemins VFS dont le nom (sans extension) est CONTENU dans `needle`, insensible à la casse —
/// substring en mémoire, fallback historique tant que l'index SQL ([`vfs_all_entries`] +
/// `vfsIndexDb`) n'a pas été construit côté frontend.
#[tauri::command]
#[specta::specta]
fn vfs_related(
    needle: String,
    limit: u32,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<EntryDto>, String> {
    with_vfs(game_dir, &state, |vfs| {
        let mut hits: Vec<EntryDto> = vfs
            .iter()
            .filter(|(p, _)| p.contains(&needle))
            .map(|(path, entry)| EntryDto {
                path: path.to_string(),
                name: path.rsplit('/').next().unwrap_or(path).to_string(),
                size: entry.file_size,
                cpk: entry.cpk_filename.clone(),
            })
            .collect();
        hits.sort_by(|a, b| a.path.cmp(&b.path));
        hits.truncate(limit.max(1) as usize);
        Ok(hits)
    })
}

// ─── Données de jeu statiques (nie-data — techniques, extensible) ─────────────────────

/// Liste toutes les techniques du jeu (`nie_data::skill`, cf. `game_data.rs`) — première
/// donnée de jeu STATIQUE câblée depuis `nie-data` (dépendance déclarée mais jamais utilisée
/// avant), via le pont déjà existant `nie_explore::bridge` (même moteur que `niers vfs cat`).
#[tauri::command]
#[specta::specta]
fn game_data_skills(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::SkillDto>, String> {
    with_vfs(game_dir, &state, game_data::list_skills)
}

/// Objets (armes/consommables/costumes/…, `nie_data::item`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_items(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::ItemDto>, String> {
    with_vfs(game_dir, &state, game_data::list_items)
}

/// Avatar/Keshin (`nie_data::aura`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_auras(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::AuraDto>, String> {
    with_vfs(game_dir, &state, game_data::list_auras)
}

/// Succès (`nie_data::trophy`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_trophies(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::TrophyDto>, String> {
    with_vfs(game_dir, &state, game_data::list_trophies)
}

/// Quêtes (`nie_data::quest`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_quests(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::QuestDto>, String> {
    with_vfs(game_dir, &state, game_data::list_quests)
}

/// Boutiques (`nie_data::shop`) — même patron que [`game_data_skills`] (§4.1 roadmap).
#[tauri::command]
#[specta::specta]
fn game_data_shops(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::ShopDto>, String> {
    with_vfs(game_dir, &state, game_data::list_shops)
}

/// Stades (`nie_data::stadium`) — même patron que [`game_data_skills`] (§4.1 roadmap).
#[tauri::command]
#[specta::specta]
fn game_data_stadiums(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::StadiumDto>, String> {
    with_vfs(game_dir, &state, game_data::list_stadiums)
}

/// Capacités passives (`nie_data::passive`) — même patron que [`game_data_skills`] (§4.1 roadmap).
#[tauri::command]
#[specta::specta]
fn game_data_passives(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::PassiveDto>, String> {
    with_vfs(game_dir, &state, game_data::list_passives)
}

/// Tactiques spéciales (`nie_data::special_tactics`) — même patron que [`game_data_skills`]
/// (§4.1 roadmap).
#[tauri::command]
#[specta::specta]
fn game_data_special_tactics(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::SpecialTacticsDto>, String> {
    with_vfs(game_dir, &state, game_data::list_special_tactics)
}

/// Écussons d'équipe (`nie_data::emblems`) — même patron que [`game_data_skills`], côté RDBN
/// (`game_data::load_rdbn`, §4.1 roadmap).
#[tauri::command]
#[specta::specta]
fn game_data_emblems(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::EmblemDto>, String> {
    with_vfs(game_dir, &state, game_data::list_emblems)
}

/// Illustrations de la galerie (`nie_data::gallery`) — même patron que [`game_data_emblems`].
#[tauri::command]
#[specta::specta]
fn game_data_gallery(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::GalleryDto>, String> {
    with_vfs(game_dir, &state, game_data::list_gallery)
}

/// Feintes/dribbles (`nie_data::trick`) — même patron que [`game_data_emblems`].
#[tauri::command]
#[specta::specta]
fn game_data_tricks(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::TrickDto>, String> {
    with_vfs(game_dir, &state, game_data::list_tricks)
}

/// Arbre des activités/sous-tâches (`nie_data::activity`) — même patron que [`game_data_emblems`],
/// mais côté T2B.
#[tauri::command]
#[specta::specta]
fn game_data_activities(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::ActivityDto>, String> {
    with_vfs(game_dir, &state, game_data::list_activities)
}

/// Équipes d'appartenance (`nie_data::belong_team`, noms joints depuis `team_text`) — même patron
/// que [`game_data_emblems`].
#[tauri::command]
#[specta::specta]
fn game_data_belong_teams(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::BelongTeamDto>, String> {
    with_vfs(game_dir, &state, game_data::list_belong_teams)
}

/// Formations de terrain (`nie_data::formation`) — même patron que [`game_data_emblems`].
/// Identifiants bruts : `formation_text.cfg.bin` n'existe pas dans cette version du jeu.
#[tauri::command]
#[specta::specta]
fn game_data_formations(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::FormationDto>, String> {
    with_vfs(game_dir, &state, game_data::list_formations)
}

/// Uniformes (`nie_data::uniform`, tranches de modèles résolues) — même patron que
/// [`game_data_emblems`].
#[tauri::command]
#[specta::specta]
fn game_data_uniforms(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::UniformDto>, String> {
    with_vfs(game_dir, &state, game_data::list_uniforms)
}

/// Personnages complets (`game_data::list_charas` : identité, série, équipe, techniques apprises)
/// — la fiche entière, à distinguer du sélecteur réduit [`game_data_chara_picker`].
#[tauri::command]
#[specta::specta]
fn game_data_charas(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::CharaDto>, String> {
    with_vfs(game_dir, &state, game_data::list_charas)
}

/// Équipes adverses (`nie_data::opponent_team`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_opponent_teams(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::OpponentTeamDto>, String> {
    with_vfs(game_dir, &state, game_data::list_opponent_teams)
}

/// Vidéos (`nie_data::movie`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_movies(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::MovieDto>, String> {
    with_vfs(game_dir, &state, game_data::list_movies)
}

/// Bande-son (`nie_data::music_app`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_musics(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::MusicDto>, String> {
    with_vfs(game_dir, &state, game_data::list_musics)
}

/// Dictionnaire in-game (`nie_data::dictionary`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_dictionary(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::DictionaryDto>, String> {
    with_vfs(game_dir, &state, game_data::list_dictionary)
}

/// Courbe d'expérience (`nie_data::exp`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_exp_table(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::ExpLevelDto>, String> {
    with_vfs(game_dir, &state, game_data::list_exp_table)
}

/// Butin (`nie_data::soccer_drop`, table des esprits) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_drops(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::DropDto>, String> {
    with_vfs(game_dir, &state, game_data::list_drops)
}

/// Taux de tirage des capsules (`nie_data::capsule`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_capsule_rates(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::CapsuleRateDto>, String> {
    with_vfs(game_dir, &state, game_data::list_capsule_rates)
}

/// Index multilingue des noms (personnages, techniques, objets) lu DIRECTEMENT du jeu — les neuf
/// langues de `data/common/text/`. C'est la source du traducteur quand aucun miroir wiki n'est
/// configuré, cf. `game_data::list_noms`.
#[tauri::command]
#[specta::specta]
fn game_data_noms(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::NomsDto>, String> {
    with_vfs(game_dir, &state, game_data::list_noms)
}

/// Personnages sélectionnables pour le calculateur de stats (`nie_data::chara_param` joint à
/// `chara_base`/`chara_text`) — même patron que [`game_data_skills`].
#[tauri::command]
#[specta::specta]
fn game_data_chara_picker(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<game_data::CharaPickerDto>, String> {
    with_vfs(game_dir, &state, game_data::list_chara_picker)
}

/// Calcule les stats d'un personnage (§4.2 roadmap) — `nie_core::growth::calculate_stats` sur
/// les tables de croissance IEVR embarquées, cf. `game_data::calculate_character_stats`.
/// `rarity_code` : 0=N, 2=R, 3=SR, 4=SSR, 5=UR, 6=LR, 7=Legend, 20=BASARA.
#[tauri::command]
#[specta::specta]
fn game_data_calculate_stats(
    chara_param_id: String,
    level: u8,
    rarity_code: u8,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<game_data::StatBlockDto, String> {
    with_vfs(game_dir, &state, |vfs| {
        game_data::calculate_character_stats(vfs, &chara_param_id, level, rarity_code)
    })
}

/// Décode N'IMPORTE QUEL `.cfg.bin` du VFS (RDBN *ou* T2B, détecté automatiquement via
/// [`nie_formats::cfgbin::is_rdbn`]) vers la forme JSON "inagle" — couvre TOUS les fichiers de
/// configuration du jeu (personnages, objets, techniques, auras, boutiques, quêtes, trophées,
/// tactiques, capsules, costumes… plusieurs centaines de fichiers dans `data/common/gamedata/`
/// et `data/common/text/`), pas seulement les quelques modules `nie-data` câblés individuellement
/// avec un DTO typé (`game_data.rs`) — cf. demande utilisatrice « niers doit couvrir tout
/// nie.exe ». Générique : aucun parseur par format à écrire, juste le pont déjà
/// vérifié [`nie_explore::bridge`] (même moteur que `niers vfs cat`).
#[tauri::command]
#[specta::specta]
fn vfs_decode_cfgbin(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<RawJson, String> {
    with_vfs(game_dir, &state, |vfs| {
        game_data::decode_cfgbin(vfs, &path).map(RawJson)
    })
}

/// Occupation du cache CPK, en mégaoctets.
///
/// Les tailles sont en Mo et non en octets pour rester des entiers simples côté interface :
/// Specta traduit un flottant en `number | null` (un flottant non fini n'est pas
/// représentable en JSON), ce qui obligerait chaque affichage à traiter un cas impossible.
#[derive(Serialize, specta::Type)]
struct CacheCpkDto {
    /// Octets bruts retenus, en Mo.
    octets_mo: u32,
    /// Nombre de paquets CPK en cache.
    entrees: u32,
    /// Budget au-delà duquel l'éviction LRU se déclenche, en Mo.
    budget_mo: u32,
}

/// Convertit des octets en mégaoctets, arrondis au plus proche.
fn en_mo(octets: usize) -> u32 {
    u32::try_from(octets.div_ceil(1024 * 1024)).unwrap_or(u32::MAX)
}

/// Occupation actuelle du cache CPK — ce que l'explorateur retient en RAM.
///
/// Rend la consommation observable depuis l'interface : sans cette mesure, un cache qui monte
/// à plusieurs gigaoctets ne se voit nulle part, et le symptôme (la machine qui rame) n'accuse
/// jamais le cache.
#[tauri::command]
#[specta::specta]
fn vfs_cache_stats(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<CacheCpkDto, String> {
    with_vfs(game_dir, &state, |vfs| {
        let s = vfs.cache_stats();
        Ok(CacheCpkDto {
            octets_mo: en_mo(s.octets),
            entrees: u32::try_from(s.entrees).unwrap_or(u32::MAX),
            budget_mo: en_mo(s.budget),
        })
    })
}

/// Vide le cache CPK et rend les mégaoctets libérés.
///
/// Sans danger pour les lectures en cours : chacune détient un `Arc` sur sa donnée, qui reste
/// vivante jusqu'à la fin de l'extraction. Les lectures suivantes relisent le paquet depuis le
/// disque — c'est le prix, assumé, de rendre la RAM.
#[tauri::command]
#[specta::specta]
fn vfs_cache_vider(game_dir: Option<String>, state: tauri::State<VfsState>) -> Result<u32, String> {
    with_vfs(game_dir, &state, |vfs| Ok(en_mo(vfs.vider_cache())))
}

/// Aperçu traçable d'une caméra de cinématique (`.g4cm`) — 1 215 fichiers dans le jeu.
///
/// Rend des **pistes** `(objet, canal, temps → valeur)` plutôt que la structure complète du
/// décodeur : celle-ci descend jusqu'aux octets de rembourrage, ce qu'il faut pour réencoder à
/// l'octet près mais qui noierait une vue. Les canaux portent la position de la caméra
/// (`PosX/Y/Z`), son point visé (`RefX/Y/Z`) et son champ de vision (`Fov`).
///
/// Un canal dont le flux n'est pas `f32` sort avec `resolu = false` et sans valeurs :
/// l'encodage 2 octets n'est pas élucidé, et inventer des nombres donnerait une trajectoire
/// plausible et fausse.
#[tauri::command]
#[specta::specta]
async fn vfs_apercu_camera(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<'_, VfsState>,
) -> Result<camera_nav::ApercuCameraDto, String> {
    sur_vfs_bloquant(game_dir, &state, move |vfs| {
        camera_nav::apercu_camera(vfs, &path)
    })
    .await
}

/// Aperçu projetable d'un maillage de navigation (`.g4nv`) — 160 fichiers, 153 cartes.
///
/// Rend les sommets en coordonnées monde, les **triangles** (trois coins par polygone) et les
/// arêtes du graphe avec leur coût. `bord` marque les arêtes qui ne relient qu'un polygone :
/// c'est le contour de la zone marchable. `tronque` dit qu'un plafond a mordu — l'affichage
/// doit le signaler plutôt que de laisser croire à un maillage complet.
#[tauri::command]
#[specta::specta]
async fn vfs_apercu_navmesh(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<'_, VfsState>,
) -> Result<camera_nav::ApercuNavmDto, String> {
    sur_vfs_bloquant(game_dir, &state, move |vfs| {
        camera_nav::apercu_navm(vfs, &path)
    })
    .await
}

/// Décode un `.cfg.bin` **et** le passe au parseur typé de sa famille, si elle en a un.
///
/// `vfs_decode_cfgbin` rend la forme générique du conteneur (`lists`/`entries`) : des colonnes
/// numérotées, sans nom ni sens. Ici, la clé de famille est dérivée du nom de fichier
/// (`nie_data::typed::family_key`) puis dispatchée vers l'un des **112 parseurs** de `nie-data`,
/// qui rendent des structures nommées — c'est la différence entre « var3 = 1852 » et
/// « `consume_tp` = 1852 ».
///
/// `famille` est `None` quand aucun parseur ne correspond : l'appelant retombe alors sur la vue
/// générique plutôt que de ne rien afficher. C'est le cas de la majorité des `.cfg.bin` du jeu
/// (map, event, effect…), qui n'ont pas de sémantique portée.
#[tauri::command]
#[specta::specta]
fn vfs_decode_cfgbin_typed(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<CfgbinTyped, String> {
    let root = with_vfs(game_dir, &state, |vfs| game_data::decode_cfgbin(vfs, &path))?;
    let brut = serde_json::to_string(&root).map_err(|e| e.to_string())?;
    let cle = nie_data::typed::family_key(&path);
    match nie_data::typed::decode_by_key(&cle, &root) {
        Some((label, valeur)) => Ok(CfgbinTyped {
            cle,
            famille: Some(label.to_string()),
            json: serde_json::to_string(&valeur).map_err(|e| e.to_string())?,
            brut,
        }),
        None => Ok(CfgbinTyped {
            cle,
            famille: None,
            json: String::new(),
            brut,
        }),
    }
}

/// Résultat d'un décodage typé : la forme générique est toujours rendue, la forme nommée
/// seulement quand la famille est couverte.
#[derive(serde::Serialize, specta::Type)]
struct CfgbinTyped {
    /// Clé de famille dérivée du nom de fichier (`skill_config`, `formation_config`…).
    cle: String,
    /// Étiquette du parseur qui a répondu (`skill`, `formation`…), `None` si aucun.
    famille: Option<String>,
    /// Données typées sérialisées, vide si `famille` est `None`.
    json: String,
    /// Forme générique du conteneur — toujours présente.
    brut: String,
}

/// Ré-encode du JSON édité (forme "inagle" `{"entries":[...]}` T2B **ou** `{"lists":[...]}`
/// RDBN, dispatch automatique symétrique à [`vfs_decode_cfgbin`]) vers un `.cfg.bin` binaire
/// VALIDE.
///
/// - T2B : `nie_formats::cfgbin::encode_t2b`, reconstruction libre à partir du JSON seul.
/// - RDBN : `nie_formats::cfgbin::encode_rdbn` + `nie_explore::bridge::json_to_rdbn_lists`, qui a
///   besoin de l'ORIGINAL déjà décodé comme gabarit — c'est un *patch* de valeurs, pas une
///   reconstruction libre : le JSON seul perd l'information de type par colonne (ex. Short/
///   ActType ou Rates/Position sont indiscernables une fois sérialisés). D'où `path` en plus de
///   `json` ici : on relit et reparse le fichier original depuis le VFS pour fournir ce gabarit.
///
/// Les deux encodeurs sont vérifiés par round-trip réel sur des centaines/milliers de vrais
/// fichiers du jeu (`cfgbin.rs` : `encode_t2b_round_trip_sur_le_vrai_jeu`,
/// `encode_rdbn_round_trip_sur_le_vrai_jeu` ; `bridge.rs` : `json_bridge_round_trip_sur_le_vrai_jeu`,
/// `json_bridge_rdbn_round_trip_sur_le_vrai_jeu`), pas devinés.
///
/// Renvoie les octets en base64 : compose avec [`vfs_write_b64`]/
/// [`vfs_write_loose_override_b64`]/[`save_bytes_b64`] côté frontend pour l'écriture réelle —
/// pas de nouvelle commande d'écriture, réutilisation de celles qui existent déjà.
#[tauri::command]
#[specta::specta]
fn encode_cfgbin_config(
    path: String,
    json: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("JSON invalide : {e}"))?;
    let bytes = if value.get("lists").is_some() {
        with_vfs(game_dir, &state, |vfs| {
            let raw = vfs.read(&path).map_err(|e| e.to_string())?;
            let rdbn =
                nie_formats::cfgbin::parse(&raw).map_err(|e| format!("parse RDBN {path} : {e}"))?;
            let original = nie_formats::cfgbin::read_values(&rdbn, &raw);
            let lists = nie_explore::bridge::json_to_rdbn_lists(&original, &value)?;
            nie_formats::cfgbin::encode_rdbn(&lists)
        })?
    } else {
        let entries = nie_explore::bridge::json_to_t2b_entries(&value)?;
        nie_formats::cfgbin::encode_t2b(&entries)
    };
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

// ─── CPK brut hors VFS (« ouvrir un .cpk physiquement présent sur disque ») ────────────
//
// Le VFS (`Vfs`/`VfsState` ci-dessus) ne connaît QUE les CPK référencés par `cpk_list.cfg.bin`
// du jeu monté. Cette section permet d'ouvrir N'IMPORTE QUEL fichier `.cpk` du disque (mod
// téléchargé, sauvegarde d'un pack, DLC séparé…) directement, sans passer par l'index du jeu —
// même lecteur `nie_formats::cpk::CpkReader` que `Vfs`, juste sans l'indirection cpk_list/VFS.

/// CPK brut actuellement ouvert (octets bruts + lecteur d'entrées) — `Some` après
/// [`open_raw_cpk`], consommé par [`raw_cpk_extract_to`]/[`raw_cpk_read_b64`]/
/// [`raw_cpk_describe`] via l'INDEX de l'entrée (pas son chemin : `nie-formats` n'exclut pas les
/// doublons de nom entre dossiers différents d'un même CPK, l'index est donc la seule clé fiable).
struct RawCpkState(Mutex<Option<(PathBuf, Vec<u8>, CpkReader)>>);

#[derive(Serialize, specta::Type)]
struct RawCpkEntryDto {
    /// Index dans `CpkReader::entries` — clé stable pour les commandes suivantes (PAS le chemin :
    /// deux entrées de dossiers différents peuvent partager un nom de fichier).
    index: u32,
    path: String,
    size: u32,
    is_compressed: bool,
}

fn raw_cpk_entry_dto(index: usize, e: &CpkEntry) -> RawCpkEntryDto {
    let path = if e.directory.is_empty() {
        e.filename.clone()
    } else {
        format!("{}/{}", e.directory, e.filename)
    };
    RawCpkEntryDto {
        index: index as u32,
        path,
        size: e.extract_size as u32,
        is_compressed: e.is_compressed,
    }
}

#[derive(Serialize, specta::Type)]
struct PackFileDto {
    /// Chemin absolu réel sur disque (PAS un chemin interne VFS) — passé tel quel à
    /// [`open_raw_cpk`] pour l'ouvrir.
    path: String,
    name: String,
    size: u32,
}

/// Liste les VRAIS fichiers `.cpk` physiquement présents sous `<racine>/data/packs/` — le VFS
/// n'expose JAMAIS ces conteneurs comme des entrées navigables (`vfs_ls`/`vfs_all_entries` ne
/// listent que les chemins internes du jeu, ex. `data/common/...`, jamais `data/packs/*.cpk`
/// eux-mêmes), donc naviguer vers `data/packs` dans l'Explorateur y paraissait vide/« non
/// préchargé » alors que les fichiers sont bien là — cette commande comble ce trou en lisant le
/// vrai dossier, pour un pont direct vers [`open_raw_cpk`]/l'onglet CPK brut.
#[tauri::command]
#[specta::specta]
fn list_packs_dir(game_dir: Option<String>) -> Result<Vec<PackFileDto>, String> {
    let root = resolve_root(game_dir.as_deref());
    let packs = root.join("data").join("packs");
    let dir_iter =
        std::fs::read_dir(&packs).map_err(|e| format!("lecture de {} : {e}", packs.display()))?;
    let mut out = Vec::new();
    for entry in dir_iter.flatten() {
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("cpk"))
            .unwrap_or(false)
        {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0) as u32;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            out.push(PackFileDto {
                path: path.display().to_string(),
                name,
                size,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Ouvre un fichier `.cpk` quelconque du disque (chemin absolu choisi par l'utilisatrice) et
/// renvoie la liste de ses entrées — met à jour [`RawCpkState`] pour les commandes suivantes.
#[tauri::command]
#[specta::specta]
fn open_raw_cpk(
    path: String,
    state: tauri::State<RawCpkState>,
) -> Result<Vec<RawCpkEntryDto>, String> {
    let data = std::fs::read(&path).map_err(|e| format!("lecture de {path} : {e}"))?;
    let filename = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path)
        .to_string();
    let reader =
        CpkReader::new(&data, &filename).map_err(|e| format!("parsing CPK {path} : {e}"))?;
    let dtos: Vec<RawCpkEntryDto> = reader
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| raw_cpk_entry_dto(i, e))
        .collect();
    *state.0.lock().unwrap() = Some((PathBuf::from(path), data, reader));
    Ok(dtos)
}

/// Aperçu structuré d'une entrée du CPK brut ouvert (même moteur que [`vfs_describe`]) — extrait
/// et décompresse d'abord via [`CpkReader::extract`].
#[tauri::command]
#[specta::specta]
fn raw_cpk_describe(index: u32, state: tauri::State<RawCpkState>) -> Result<Vec<String>, String> {
    let guard = state.0.lock().unwrap();
    let (_, data, reader) = guard.as_ref().ok_or("aucun CPK ouvert")?;
    let entry = reader
        .entries
        .get(index as usize)
        .ok_or("index d'entrée invalide")?;
    let extracted = reader.extract(data, entry).map_err(|e| e.to_string())?;
    let path = if entry.directory.is_empty() {
        entry.filename.clone()
    } else {
        format!("{}/{}", entry.directory, entry.filename)
    };
    let mut lines = nie_explore::describe_content(&path, &extracted).unwrap_or_default();
    if lines.is_empty() {
        lines.push("format      brut / non reconnu".to_string());
    }
    lines.push(format!(
        "magic       {}",
        nie_explore::hex_prefix(&extracted, 16)
    ));
    lines.push(format!("taille      {} octets", extracted.len()));
    Ok(lines)
}

/// Contenu brut d'une entrée du CPK ouvert, borné (même plafond par défaut que [`vfs_read_b64`]).
#[tauri::command]
#[specta::specta]
fn raw_cpk_read_b64(
    index: u32,
    max_bytes: Option<u32>,
    state: tauri::State<RawCpkState>,
) -> Result<String, String> {
    let guard = state.0.lock().unwrap();
    let (_, data, reader) = guard.as_ref().ok_or("aucun CPK ouvert")?;
    let entry = reader
        .entries
        .get(index as usize)
        .ok_or("index d'entrée invalide")?;
    let extracted = reader.extract(data, entry).map_err(|e| e.to_string())?;
    let cap = max_bytes.map(|b| b as usize).unwrap_or(2 * 1024 * 1024);
    if extracted.len() > cap {
        return Err(format!(
            "fichier trop volumineux pour l'aperçu ({} octets > {cap})",
            extracted.len()
        ));
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(&extracted))
}

/// Extrait une entrée du CPK ouvert vers `dest` (écriture Rust→disque directe).
#[tauri::command]
#[specta::specta]
fn raw_cpk_extract_to(
    index: u32,
    dest: String,
    state: tauri::State<RawCpkState>,
) -> Result<u32, String> {
    let guard = state.0.lock().unwrap();
    let (_, data, reader) = guard.as_ref().ok_or("aucun CPK ouvert")?;
    let entry = reader
        .entries
        .get(index as usize)
        .ok_or("index d'entrée invalide")?;
    let extracted = reader.extract(data, entry).map_err(|e| e.to_string())?;
    if let Some(parent) = std::path::Path::new(&dest).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&dest, &extracted).map_err(|e| e.to_string())?;
    Ok(extracted.len() as u32)
}

/// Extrait TOUTES les entrées du CPK ouvert vers `dest_dir`, en préservant l'arborescence
/// `directory/filename` d'origine (mécanique identique à [`raw_cpk_extract_to`], en boucle sur
/// `RawCpkState.entries`) — évite d'extraire un CPK entier une entrée à la fois depuis l'UI.
/// Renvoie `(n_ok, n_err)` : les échecs individuels (entrée corrompue/compression non supportée)
/// n'interrompent pas le reste de l'extraction, pour ne pas perdre tout le travail sur 1 entrée.
#[tauri::command]
#[specta::specta]
fn raw_cpk_extract_all(
    dest_dir: String,
    state: tauri::State<RawCpkState>,
) -> Result<(u32, u32), String> {
    let guard = state.0.lock().unwrap();
    let (_, data, reader) = guard.as_ref().ok_or("aucun CPK ouvert")?;
    let (mut n_ok, mut n_err) = (0u32, 0u32);
    for entry in &reader.entries {
        let rel = if entry.directory.is_empty() {
            entry.filename.clone()
        } else {
            format!("{}/{}", entry.directory, entry.filename)
        };
        let dest = std::path::Path::new(&dest_dir).join(&rel);
        let ok = (|| -> Result<(), String> {
            let extracted = reader.extract(data, entry).map_err(|e| e.to_string())?;
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&dest, &extracted).map_err(|e| e.to_string())
        })();
        match ok {
            Ok(()) => n_ok += 1,
            Err(_) => n_err += 1, // entrée corrompue/compression non supportée : on continue le reste
        }
    }
    Ok((n_ok, n_err))
}

// ─── Fichier ouvert depuis l'explorateur Windows (« Ouvrir avec ») ─────────────────

/// Rend une fois le chemin passé en argument au lancement (`argv[1]`), puis se vide — le
/// frontend l'appelle une seule fois au démarrage pour savoir s'il doit ouvrir un fichier
/// « externe » (hors VFS, ex. un `.g4tx` déjà extrait sur disque) plutôt que la racine du VFS.
#[tauri::command]
#[specta::specta]
fn take_pending_open(state: tauri::State<PendingOpen>) -> Option<String> {
    state.0.lock().unwrap().take()
}

/// Aperçu structuré d'un fichier QUELCONQUE du disque (pas du VFS) — utilisé par « Ouvrir
/// avec nie-explorer » sur un fichier déjà extrait/exporté.
#[tauri::command]
#[specta::specta]
fn describe_disk_file(path: String) -> Result<Vec<String>, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mut lines = nie_explore::describe_content(&path, &data).unwrap_or_default();
    if lines.is_empty() {
        lines.push("format      brut / non reconnu".to_string());
    }
    lines.push(format!(
        "magic       {}",
        nie_explore::hex_prefix(&data, 16)
    ));
    lines.push(format!("taille      {} octets", data.len()));
    Ok(lines)
}

#[tauri::command]
#[specta::specta]
fn read_disk_file_b64(path: String, max_bytes: Option<u32>) -> Result<String, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let cap = max_bytes.map(|b| b as usize).unwrap_or(2 * 1024 * 1024);
    if data.len() > cap {
        return Err(format!(
            "fichier trop volumineux pour l'aperçu ({} octets > {cap})",
            data.len()
        ));
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(&data))
}

/// Resynchronise le chrome natif Windows 11 (Mica, barre de titre/légende) sur le thème
/// clair/sombre choisi côté frontend (`next-themes`, `resolvedTheme`) — corrige le fait que le
/// chrome natif restait figé en sombre (`Some(true)` posé une seule fois au lancement, cf. `run()`)
/// même si l'utilisatrice bascule en clair dans Paramètres. No-op silencieux hors Windows 11
/// (même best-effort que l'appel initial dans `run()`).
#[tauri::command]
#[specta::specta]
/// Force l'arrondi des coins Windows 11 (`DWMWA_WINDOW_CORNER_PREFERENCE` = `DWMWCP_ROUND`) sur
/// une fenêtre SANS bordure (`decorations: false`) — DWM n'arrondit par défaut que les fenêtres à
/// légende native (`WS_CAPTION`) ; une `WS_POPUP` custom resterait à coins vifs sans cet appel.
/// Même mécanique que `apply_dark_titlebar` de spacedrive (`windows.rs`, `DwmSetWindowAttribute`
/// brut — pas de crate tierce, l'attribut est trop récent pour `window_vibrancy`). Best-effort :
/// silencieux hors Windows 11 (build serveur/VM ancienne), la fenêtre reste alors à coins vifs.
#[cfg(target_os = "windows")]
fn apply_rounded_corners(window: &tauri::WebviewWindow) {
    #[allow(non_snake_case)]
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    #[allow(non_snake_case)]
    const DWMWCP_ROUND: i32 = 2;

    dwm_set_i32_attribute(window, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND);
}

/// Pose un attribut DWM de type `i32`/`BOOL` sur la fenêtre. `DwmSetWindowAttribute` est déclaré
/// à la main (pas de crate `windows` dans les dépendances) — même approche que `windows.rs` de
/// spacedrive. Best-effort : un attribut non reconnu (build Windows plus ancienne) renvoie une
/// erreur qu'on ignore, la fenêtre garde simplement son apparence par défaut.
#[cfg(target_os = "windows")]
fn dwm_set_i32_attribute(window: &tauri::WebviewWindow, attr: u32, value: i32) {
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: isize,
            attr: u32,
            value: *const std::ffi::c_void,
            size: u32,
        ) -> i32;
    }

    let Ok(hwnd) = window.hwnd() else { return };
    // SAFETY : `hwnd` désigne la fenêtre vivante détenue par Tauri, et le couple pointeur/taille
    // décrit exactement le `i32` attendu par ces attributs DWM.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd.0 as isize,
            attr,
            std::ptr::addr_of!(value).cast(),
            std::mem::size_of::<i32>() as u32,
        );
    }
}

/// Aligne le thème du chrome de fenêtre sur le clair/sombre de l'appli.
///
/// La fenêtre est SANS décorations (`decorations: false`) : il n'y a plus de barre de titre native
/// à teinter, et l'ancienne implémentation (`window_vibrancy::apply_mica`) faisait bien pire que
/// rien — Mica étend la frame DWM dans la zone client, ce qui redonne à Windows une frame à
/// dessiner (bordure + légende + boutons système par-dessus le chrome custom). Ce qui reste utile
/// et sans effet de bord, c'est `DWMWA_USE_IMMERSIVE_DARK_MODE` : il pilote la couleur de l'ombre
/// portée et des menus système associés à la fenêtre.
#[tauri::command]
#[specta::specta]
fn set_titlebar_theme(dark: bool, window: tauri::WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        #[allow(non_snake_case)]
        const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
        dwm_set_i32_attribute(&window, DWMWA_USE_IMMERSIVE_DARK_MODE, i32::from(dark));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (dark, window);
    }
    Ok(())
}

// ─── Presse-papiers FICHIERS natif (CF_HDROP) — inspiré de cosmic-files `clipboard.rs` ────────
//
// Recherche 2026-08-08 (« inspire-toi de cosmic-files pour... les interactions OS/filesystem ») :
// cosmic-files pose SIMULTANÉMENT `text/plain` (chemins), `text/uri-list` (URIs `file://`) et
// `x-special/gnome-copied-files` (le MIME copier/coller-fichiers de GNOME/Nautilus, préfixé
// `copy\n`/`cut\n`) sur le presse-papiers X11/Wayland — trois représentations du MÊME contenu,
// pour qu'un Ctrl+V dans N'IMPORTE QUELLE appli (pas seulement une autre instance de cosmic-files)
// comprenne « ce sont des fichiers ». L'équivalent Windows EXACT de ce triplet est un SEUL format
// natif : CF_HDROP (la structure `DROPFILES`, ce que l'Explorateur Windows lit/écrit pour
// Ctrl+C/Ctrl+V et le drag&drop) — `clipboard-win` l'expose via `formats::FileList`.

/// Pose une VRAIE liste de fichiers sur le presse-papiers Windows (CF_HDROP) — ce que
/// l'Explorateur Windows (ou n'importe quelle appli) sait coller comme de VRAIS fichiers, pas du
/// texte. Remplace l'ancien Ctrl+C de `ExplorerView` (`writeText` du plugin Tauri, chemins en
/// texte brut séparés par `\n` — lisible par notre propre `Ctrl+V` interne par accident, mais
/// PAS par l'Explorateur). `paths` doivent exister sur disque (CF_HDROP silencieux sinon, pas
/// d'erreur Windows explicite) — vérifié côté appelant.
#[tauri::command]
#[specta::specta]
fn clipboard_write_file_list(paths: Vec<String>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use clipboard_win::{formats, Clipboard, Setter};
        let _clip = Clipboard::new_attempts(10)
            .map_err(|e| format!("ouverture du presse-papiers Windows : {e}"))?;
        formats::FileList
            .write_clipboard(&paths)
            .map_err(|e| format!("presse-papiers Windows (CF_HDROP) : {e}"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = paths;
        Err("presse-papiers fichiers natif non implémenté hors Windows".to_string())
    }
}

/// Lit une VRAIE liste de fichiers depuis le presse-papiers Windows (CF_HDROP) — `None` si le
/// presse-papiers ne contient pas ce format (ex. juste du texte, ou vide). Permet un VRAI Ctrl+V
/// depuis l'Explorateur Windows (copier un fichier dans l'Explorateur, Ctrl+V ici) SANS dépendre
/// du fait que l'Explorateur pose accessoirement `CF_UNICODETEXT` (il ne le fait pas toujours,
/// contrairement à ce qu'un simple `readText()` supposait implicitement).
#[tauri::command]
#[specta::specta]
fn clipboard_read_file_list() -> Option<Vec<String>> {
    #[cfg(target_os = "windows")]
    {
        use clipboard_win::{formats, Clipboard, Getter};
        let _clip = Clipboard::new_attempts(10).ok()?;
        let mut out = Vec::new();
        formats::FileList.read_clipboard(&mut out).ok()?;
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Vérifie le VRAI presse-papiers Windows (pas de mock/simulation) : écrit une liste de fichiers
/// réels via CF_HDROP, la relit, compare. `#[ignore]` par défaut — un test qui écrase le vrai
/// presse-papiers de la session (utilisatrice ou CI) à chaque `cargo test` serait hostile ;
/// lancer explicitement via `cargo test -p nie-explorer --lib -- --ignored
/// clipboard_file_list_roundtrip_reel`. Gagné en confiance réelle (2026-08-08, recherche
/// « inspire-toi de cosmic-files pour les interactions OS/filesystem ») : validé une fois contre
/// le presse-papiers Windows réel de ce poste de dev avant d'être marqué `#[ignore]`.
#[cfg(all(test, target_os = "windows"))]
#[test]
#[ignore = "écrase le presse-papiers Windows réel — lancer explicitement avec --ignored"]
fn clipboard_file_list_roundtrip_reel() {
    let dir = std::env::temp_dir().join("nie-explorer-clipboard-test");
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.txt");
    let b = dir.join("b.txt");
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();
    let paths = vec![a.display().to_string(), b.display().to_string()];

    clipboard_write_file_list(paths.clone()).expect("écriture CF_HDROP");
    let read_back = clipboard_read_file_list()
        .expect("le presse-papiers doit contenir les fichiers qu'on vient d'y poser");
    assert_eq!(read_back, paths);
}

/// `true` si `path` désigne un FICHIER (pas un dossier) existant sur disque — hors de toute
/// portée `fs:scope` JS (même famille que [`describe_disk_file`], `std::fs` direct). Utilisé pour
/// valider un chemin venu du presse-papiers (Ctrl+V, cf. [`copy_disk_file_to_appdata`]) : le
/// plugin `fs` JS `exists()` est scopé à `$APPDATA/**` et renverrait faux/erreur sur un chemin
/// disque quelconque, alors que c'est justement le cas normal ici.
#[tauri::command]
#[specta::specta]
fn disk_file_exists(path: String) -> bool {
    std::fs::metadata(&path).is_ok_and(|m| m.is_file())
}

/// Copie un fichier disque ARBITRAIRE (hors de toute portée `fs:scope` JS — même famille que
/// [`read_disk_file_b64`]/[`describe_disk_file`], `std::fs` direct) vers un chemin relatif sous
/// `AppData` (espace de travail des mods, `mods/<modId>/…`, `crates`/… JS `modWorkspace.ts`).
/// Utilisé par le VRAI Ctrl+V (`editBus.paste()` → `stageReplacementFromPath`) : la source vient
/// du presse-papiers, pas d'un sélecteur natif — elle n'a donc PAS la portée temporaire que
/// Tauri accorde aux chemins choisis via `@tauri-apps/plugin-dialog`, et le plugin `fs` JS
/// (portée = `$APPDATA/**` seulement, cf. `capabilities/default.json`) refuserait de la lire.
/// `dest_appdata_rel` DOIT rester sous `AppData` (jamais le dossier du jeu) — construit côté
/// frontend depuis `modDir(modId)`, jamais depuis une entrée utilisatrice libre.
#[tauri::command]
#[specta::specta]
/// Renvoie le nombre d'octets copiés en `f64` et **pas** `u64` : `specta` refuse d'exporter les
/// types « BigInt » vers TypeScript, et le refus est FATAL (panique de l'export au démarrage en
/// debug — l'app ne se lançait plus du tout). Une taille de fichier reste très en dessous des 2⁵³
/// entiers exactement représentables en `f64`, la conversion est donc sans perte.
fn copy_disk_file_to_appdata(
    app: tauri::AppHandle,
    src: String,
    dest_appdata_rel: String,
) -> Result<f64, String> {
    use tauri::Manager;
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dest = base.join(&dest_appdata_rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let n = std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    Ok(n as f64)
}

/// Envoie un ou plusieurs fichiers de l'espace de travail des mods (`AppData/mods/<modId>/…`) à
/// la VRAIE Corbeille Windows (`trash` crate, `IFileOperation`/`SHFileOperationW`) — au lieu d'un
/// `std::fs::remove_file` permanent. Recherche 2026-08-08 (« lis vraiment le code de cosmic » —
/// `trash.rs` de cosmic-files enveloppe le même crate) : `removeStagedFile`/`deleteModWorkspace`
/// utilisaient `remove()` du plugin `fs` JS (suppression permanente) sur du VRAI travail
/// utilisatrice (fichiers de mod édités, parfois de vraies heures de remplacement de texture/
/// modèle) — un clic accidentel sur « Retirer » était irrattrapable. Chemins relatifs à
/// `AppData` (même convention que [`copy_disk_file_to_appdata`]) ; un chemin absent est ignoré
/// (pas une erreur — `deleteModWorkspace` appelle ceci pour des paires staged/original dont
/// l'une des deux peut légitimement ne pas exister, ex. fichier trop gros pour avoir une
/// sauvegarde `.original`, cf. `stageReplacementFromPath`).
#[tauri::command]
#[specta::specta]
fn trash_appdata_files(
    app: tauri::AppHandle,
    appdata_rel_paths: Vec<String>,
) -> Result<(), String> {
    use tauri::Manager;
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let existing: Vec<PathBuf> = appdata_rel_paths
        .iter()
        .map(|rel| base.join(rel))
        .filter(|p| p.exists())
        .collect();
    if existing.is_empty() {
        return Ok(());
    }
    trash::delete_all(&existing).map_err(|e| format!("envoi à la Corbeille : {e}"))
}

/// Vérifie la VRAIE Corbeille Windows (pas un mock) : envoie un fichier réel via `trash::
/// delete_all` (le même appel qu'utilise [`trash_appdata_files`] — testé directement plutôt que
/// via la commande Tauri, qui a besoin d'un `AppHandle` réel non constructible hors app).
/// `#[ignore]` : mute la VRAIE Corbeille Windows de la session — lancer explicitement avec
/// `--ignored`. **Validé 2026-08-08** avant d'être marqué `#[ignore]`, par DEUX vérifications
/// indépendantes : (1) ce test (le fichier disparaît de son emplacement d'origine) ; (2) `Shell.
/// Application` COM (`$shell.Namespace(10).Items()`, l'API que l'Explorateur Windows lui-même
/// utilise pour afficher la Corbeille) confirme le fichier réellement présent sous
/// `$Recycle.Bin\<SID>\$R*.txt` — `trash::os_limited::list()` s'est avéré peu fiable en
/// relecture IMMÉDIATE dans le même process (le fichier existe bel et bien dans la Corbeille,
/// vérifié par (2), mais `list()` ne le voyait pas systématiquement juste après l'écriture) donc
/// PAS gardé comme assertion automatisée — un faux négatif aurait fait perdre confiance dans une
/// fonctionnalité qui marche réellement.
#[cfg(test)]
#[test]
#[ignore = "envoie un vrai fichier dans la Corbeille Windows — lancer explicitement avec --ignored"]
fn trash_delete_reel() {
    let dir = std::env::temp_dir().join("nie-explorer-trash-test");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join(format!("trash-test-{}.txt", std::process::id()));
    std::fs::write(&f, b"jetable").unwrap();
    assert!(f.exists());

    trash::delete_all([&f]).expect("envoi à la Corbeille");
    assert!(
        !f.exists(),
        "le fichier doit avoir disparu de son emplacement d'origine"
    );
}

/// Régénère `src/lib/bindings.ts` sans lancer toute l'app (pas de fenêtre) — même export que
/// `run()`, exécuté ici en `#[ignore]` pour un rafraîchissement ponctuel (ex. après ajout de
/// commandes) sans dépendre de `bun run tauri dev`.
#[cfg(test)]
#[test]
#[ignore = "écrit sur disque (../src/lib/bindings.ts) — lancer explicitement avec --ignored"]
fn regen_bindings_ts() {
    specta_builder()
        .export(
            specta_typescript::Typescript::default(),
            "../src/lib/bindings.ts",
        )
        .expect("échec de l'export des bindings TypeScript (tauri-specta)");
}

/// Remplace la texture d'un `.g4tx` **mono-texture, sans région d'atlas** (§2.2 roadmap,
/// « Éditeur d'image (textures) ») par un PNG choisi — lit `vfs_path`, valide qu'il s'agit bien
/// du cas simple pris en charge (cf. doc `nie_formats::g4tx_encode`, rejette explicitement les
/// atlas multi-région comme `gaiji_game.g4tx` où « remplacer » n'aurait pas de sens univoque),
/// décode le PNG source (chemin disque arbitraire, hors portée `fs:scope` JS — même famille que
/// [`copy_disk_file_to_appdata`]) et écrit le `.g4tx` réencodé directement dans l'espace de
/// travail du mod (`AppData/mods/<modId>/…`, jamais le dossier du jeu). Conserve `name`/`id` de
/// la texture d'origine (dimensions reprises du PNG, peuvent différer de l'original).
#[tauri::command]
#[specta::specta]
fn stage_texture_replacement(
    app: tauri::AppHandle,
    vfs_path: String,
    png_src_path: String,
    dest_appdata_rel: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<f64, String> {
    // `f64` et pas `u64` : cf. [`copy_disk_file_to_appdata`] (contrainte `specta`/TypeScript).
    use tauri::Manager;

    let g4tx_bytes = with_vfs(game_dir, &state, |vfs| {
        let data = vfs.read(&vfs_path).map_err(|e| e.to_string())?;
        let parsed = nie_formats::g4tx::parse(&data).map_err(|e| e.to_string())?;
        if parsed.header.texture_count != 1 || parsed.header.sub_texture_count != 0 {
            return Err(
                "remplacement pris en charge uniquement pour les .g4tx mono-texture sans région d'atlas \
                 (les atlas multi-région comme gaiji_game.g4tx partagent une texture entre plusieurs \
                 régions — « remplacer » n'aurait pas de sens univoque)."
                    .to_string(),
            );
        }
        let tex = &parsed.textures[0];
        let png_bytes = std::fs::read(&png_src_path)
            .map_err(|e| format!("lecture PNG '{png_src_path}' : {e}"))?;
        let (w, h, rgba) = nie_formats::g4tx_encode::decode_png_to_rgba8(&png_bytes)?;
        let dds = nie_formats::g4tx_encode::encode_dds_bgra8(w, h, &rgba)?;
        Ok(nie_formats::g4tx_encode::encode_g4tx_single_texture(
            &tex.name, tex.id, w as i16, h as i16, &dds,
        ))
    })?;

    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dest = base.join(&dest_appdata_rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&dest, &g4tx_bytes).map_err(|e| e.to_string())?;
    Ok(g4tx_bytes.len() as f64)
}

/// Une entrée à empaqueter dans un `.cpk` exporté (§1.2 roadmap) — `vfs_path` sert à dériver
/// `directory`/`filename` (même convention que [`nie_formats::cpk::CpkEntry`] en lecture),
/// `staged_appdata_rel` est le chemin RELATIF sous `AppData` du fichier de remplacement déjà
/// mis en scène dans le mod (`ModFileRow.staged_file` côté frontend).
#[derive(Deserialize, specta::Type)]
struct CpkExportFileDto {
    vfs_path: String,
    staged_appdata_rel: String,
}

/// Exporte les fichiers d'un mod en un `.cpk` **autonome, non chiffré, non compressé** (§1.2
/// roadmap) — cf. `nie_formats::cpk_encode` pour la portée exacte et ses limites documentées
/// (vérifié par round-trip contre `CpkReader` déjà validé sur le vrai jeu, PAS par chargement
/// réel dans `nie.exe`). Lit chaque fichier mis en scène depuis `AppData` (`std::fs` direct, hors
/// portée `fs:scope` JS — même famille que [`copy_disk_file_to_appdata`]), jamais depuis le
/// dossier du jeu.
#[tauri::command]
#[specta::specta]
fn export_mod_as_cpk(
    app: tauri::AppHandle,
    files: Vec<CpkExportFileDto>,
    dest: String,
) -> Result<f64, String> {
    // `f64` et pas `u64` : cf. [`copy_disk_file_to_appdata`] (contrainte `specta`/TypeScript).
    use tauri::Manager;

    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut entries = Vec::with_capacity(files.len());
    for f in &files {
        let staged_path = base.join(&f.staged_appdata_rel);
        let data = std::fs::read(&staged_path)
            .map_err(|e| format!("lecture '{}' : {e}", staged_path.display()))?;
        let base_name = f.vfs_path.rsplit('/').next().unwrap_or(&f.vfs_path);
        let directory = f
            .vfs_path
            .strip_suffix(base_name)
            .unwrap_or("")
            .trim_end_matches('/')
            .to_string();
        entries.push(nie_formats::cpk_encode::CpkWriteEntry {
            filename: base_name.to_string(),
            directory,
            data,
        });
    }

    let bytes = nie_formats::cpk_encode::encode_cpk(&entries)?;
    if let Some(parent) = std::path::Path::new(&dest).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    Ok(bytes.len() as f64)
}

// ─── Pont Blender (plugins/niers-blender) ─────────────────────────────────────────────────────

/// Candidats d'installation Blender à essayer si aucun chemin explicite n'est fourni.
const BLENDER_CANDIDATES: &[&str] = &[
    r"C:\Program Files\Blender Foundation\Blender 5.2\blender.exe",
    r"C:\Program Files\Blender Foundation\Blender 4.2\blender.exe",
    r"C:\Program Files\Blender Foundation\Blender 4.1\blender.exe",
    r"C:\Program Files\Blender Foundation\Blender 4.0\blender.exe",
];

fn resolve_blender_exe(blender_exe: Option<String>) -> Result<PathBuf, String> {
    if let Some(p) = blender_exe.filter(|s| !s.trim().is_empty()) {
        let p = PathBuf::from(p);
        return if p.is_file() {
            Ok(p)
        } else {
            Err(format!("blender.exe introuvable : {}", p.display()))
        };
    }
    BLENDER_CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .ok_or_else(|| "blender.exe introuvable (candidats standards absents) — renseignez le chemin dans Paramètres".to_string())
}

/// Dépôt source amont de l'addon Blender (Level-5 G4 Blender Tools, licence de republication
/// confirmée auprès de l'auteur — cf. `plugins/niers-blender/README.md` en-tête).
/// `plugins/niers-blender` est **vendorisé** dans niers (fichiers réguliers versionnés, PAS un
/// submodule Git) : une utilisatrice qui clone `niers` l'a directement, sans étape
/// `git submodule update --init`. Cette constante ne sert donc qu'au filet de sécurité ci-dessous.
const NIERS_BLENDER_ADDON_GIT_URL: &str = "https://github.com/The-RealBobi/G4_Blender.git";

/// Nom du **module Python** de l'addon, indépendant du nom du dossier source. Blender l'active par
/// ce nom (`addon_enable(module=…)`, `import niers`), et un identifiant Python ne peut pas porter
/// de tiret : le dossier `plugins/niers-blender` est donc chargé par chemin explicite, et zippé
/// sous cette racine-là.
const NIERS_BLENDER_MODULE: &str = "niers";

/// Garantit que `<root>/plugins/niers-blender/__init__.py` existe et renvoie le dossier de l'addon
/// lui-même. Dans niers c'est TOUJOURS vrai (vendorisé, cf. constante ci-dessus) ; ce filet de
/// sécurité clone l'addon à la volée pour le cas où `root` (le dossier du JEU, résolu par
/// [`resolve_root`]) n'est PAS un checkout de ce dépôt — un build distribué de `nie-explorer`
/// pointé sur une simple install Steam n'a que le jeu.
fn ensure_niers_blender_addon(root: &std::path::Path) -> Result<PathBuf, String> {
    let plugins_dir = root.join("plugins");
    let addon_dir = plugins_dir.join("niers-blender");
    if addon_dir.join("__init__.py").is_file() {
        return Ok(addon_dir);
    }
    std::fs::create_dir_all(&plugins_dir)
        .map_err(|e| format!("création de {} : {e}", plugins_dir.display()))?;
    // Dossier présent mais incomplet (clone précédent interrompu) : repart de zéro plutôt que de
    // laisser `git clone` échouer sur un dossier non-vide non-git.
    if addon_dir.is_dir() {
        std::fs::remove_dir_all(&addon_dir)
            .map_err(|e| format!("nettoyage de {} : {e}", addon_dir.display()))?;
    }
    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", NIERS_BLENDER_ADDON_GIT_URL])
        .arg(&addon_dir)
        .stdin(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("échec de lancement de git (introuvable sur le PATH ?) : {e}"))?;
    if !status.success() {
        return Err(format!("échec du clonage de l'extension Blender niers ({status}) — {NIERS_BLENDER_ADDON_GIT_URL}"));
    }
    if !addon_dir.join("__init__.py").is_file() {
        return Err(format!(
            "extension clonée mais `__init__.py` introuvable sous {}",
            addon_dir.display()
        ));
    }
    Ok(addon_dir)
}

/// Extrait `path` (+ ses fichiers frères de même basename dans le même dossier VFS : g4mg/g4sk/
/// g4tx/g4mt) vers un dossier temporaire, lance Blender avec un script d'amorçage qui active
/// l'addon `plugins/niers-blender` (`bpy.utils` via `sys.path`, sans dépendre du dossier d'addons
/// utilisateur Blender — cloné à la volée via [`ensure_niers_blender_addon`] si absent) puis
/// importe RÉELLEMENT le modèle via l'opérateur `import_scene.level5_g4` (« File > Import >
/// Level-5 G4 Model »). Pose `NIE_GAME_DIR` dans l'environnement du process Blender : le panneau
/// de recherche niers→Blender (`niers_bridge.py`) l'utilise pour retrouver `niers.exe` et le VFS
/// sans deviner.
///
/// **Bug corrigé (2026-08-08, « Blender ouvre un fichier vide »)** : le script d'amorçage
/// appelait `level5_g4_port.load_original_model` — ce n'est PAS un import de scène, c'est
/// l'opérateur « choisir le template original » du **wizard d'export/portage** (`g4_port_addon.
/// py`, panneau « 1. Original model template » : il peuple les *réglages* internes de l'addon
/// pour un futur export, ne crée AUCUN objet maillage). Confirmé par lecture du code source de
/// l'addon (`plugins/niers-blender/g4_port_addon.py` `LEVEL5_G4PORT_OT_load_original_model.execute` appelle
/// `apply_original_model_to_settings`, pas un import). Le VRAI importeur (« File > Import >
/// Level-5 G4 Model », README de l'addon) est `import_scene.level5_g4` — **validé par un test
/// réel `blender --background --python`** sur le vrai `c01000010.g4md` : 3 objets créés
/// (`c01000010_20`/`eye_10`/`mouth_10`), contre 0 avant. `skip_character_setup=True` +
/// `import_character_parts=False` : évite le wizard interactif de pièces de personnage
/// (`INVOKE_DEFAULT` modal) pour un import direct et prévisible du seul fichier cliqué.
#[tauri::command]
#[specta::specta]
fn open_in_blender(
    path: String,
    blender_exe: Option<String>,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<String, String> {
    let blender = resolve_blender_exe(blender_exe)?;
    let root = resolve_root(game_dir.as_deref());
    let addon_dir = ensure_niers_blender_addon(&root)?;

    let built = with_vfs(Some(root.display().to_string()), &state, |vfs| {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let export_dir = std::env::temp_dir()
            .join("nie-explorer")
            .join("blender")
            .join(stamp.to_string());
        std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;

        // Montage dump : le modèle est DÉJÀ dans une arborescence `data/common`/`data/dx11`
        // complète, la seule chose que l'addon exige. On l'importe en place — et il y voit
        // alors tout l'arbre, pas seulement les cinq fichiers frères qu'on sait nommer :
        // un squelette partagé ou une texture au nom différent se résout, là où l'extraction
        // sélective ci-dessous le laissait manquant. `export_dir` ne sert plus qu'au script
        // de démarrage et au journal d'erreur.
        if vfs.is_dump() {
            if let Some(direct) = vfs.resolve_loose_path(&path) {
                return Ok((export_dir, direct, 0usize));
            }
        }

        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        let base = path.rsplit('/').next().unwrap_or(&path);
        let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
        let dir_prefix = path.strip_suffix(base).unwrap_or("");

        // Fichiers frères de même basename (g4mg/g4sk/g4tx/g4mt) : nécessaires au rendu complet
        // (le G4MD seul n'a ni géométrie ni squelette).
        //
        // IMPORTANT (trouvé par test réel headless `blender --background`, pas deviné) :
        // `apply_original_model_to_settings` (appelé par `load_original_model`) EXIGE que le
        // chemin du modèle soit sous un `data/common`/`data/dx11` — c'est de là qu'il déduit
        // code personnage/série/textures. Une extraction à plat (`export_dir/<stem>.<ext>`) échoue
        // avec « must be inside a data/common or data/dx11 filesystem tree ». On préserve donc le
        // chemin VFS relatif complet (`candidate`) sous `export_dir`, pas juste le basename.
        let sibling_exts = ["g4md", "g4mg", "g4sk", "g4mt", "g4tx"];
        let mut extracted_main: Option<PathBuf> = None;
        for ext in sibling_exts {
            let candidate = format!("{dir_prefix}{stem}.{ext}");
            let bytes = if candidate == path {
                Some(data.clone())
            } else {
                vfs.read(&candidate).ok()
            };
            if let Some(bytes) = bytes {
                let dest = export_dir.join(&candidate);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
                if candidate == path || ext == "g4md" {
                    extracted_main = Some(dest);
                }
            }
        }
        let main_path = extracted_main.unwrap_or(export_dir.join(&path));
        Ok((export_dir, main_path, sibling_exts.len()))
    })?;
    let (export_dir, main_path, sibling_count) = built;

    let error_log = export_dir.join("_nie_explorer_import_error.log");
    let script_path = export_dir.join("_bootstrap.py");
    let script = format!(
        r#"import importlib.util, sys, traceback
# Le dossier source porte un tiret (`niers-blender`) : il n'est pas importable par son nom.
# On charge son `__init__.py` par chemin explicite, sous le nom de module attendu par Blender.
try:
    _spec = importlib.util.spec_from_file_location({module_name:?}, {addon_init:?})
    g4b = importlib.util.module_from_spec(_spec)
    sys.modules[{module_name:?}] = g4b
    _spec.loader.exec_module(g4b)
    g4b.register()
    print("[nie-explorer] addon niers activé")
except Exception:
    traceback.print_exc()

import bpy

ERROR_LOG = {error_log:?}

def _nie_explorer_import():
    try:
        bpy.ops.import_scene.level5_g4(
            'EXEC_DEFAULT',
            filepath={main_path:?},
            skip_character_setup=True,
            import_character_parts=False,
            create_report_text=False,
        )
        print("[nie-explorer] modèle importé :", {main_path:?})
    except Exception:
        tb = traceback.format_exc()
        print(tb)
        try:
            with open(ERROR_LOG, "w", encoding="utf-8") as f:
                f.write(tb)
        except Exception:
            pass
        try:
            bpy.context.workspace.status_text_set("[nie-explorer] ECHEC import (voir " + ERROR_LOG + ")")
        except Exception:
            pass

# Differe via bpy.app.timers (meme mecanisme que l'addon lui-meme pour ses propres operateurs
# differes, cf. g4_animation_addon.defer_blender_call) : au tout premier instant ou --python
# s'execute au demarrage GUI, la fenetre/le contexte 3D ne sont pas garantis prets pour un
# operateur qui touche context.window_manager/context.workspace (import_scene.level5_g4 en a
# besoin pour sa barre de progression) -- un appel synchrone immediat peut echouer en silence.
bpy.app.timers.register(_nie_explorer_import, first_interval=0.3)
"#,
        module_name = NIERS_BLENDER_MODULE,
        addon_init = addon_dir.join("__init__.py").display().to_string(),
        error_log = error_log.display().to_string(),
        main_path = main_path.display().to_string(),
    );
    std::fs::write(&script_path, script).map_err(|e| e.to_string())?;

    std::process::Command::new(&blender)
        .arg("--python")
        .arg(&script_path)
        .env("NIE_GAME_DIR", root.display().to_string())
        // Lu par `inferred_raw_data_root`/`candidate_data_roots` de l'addon SI le chemin importé
        // n'est déjà sous un dossier `data/common/...` (ce qui EST le cas ici, cf. préservation du
        // chemin VFS ci-dessus — la résolution par chemin suffit pour ce fichier précis) ; posé
        // quand même en filet pour toute résolution qui remonterait plus haut (skelette partagé
        // hors de l'arborescence exportée, cf. `LEVEL5_G4_RAW_ROOT` dans `plugins/niers-blender/__init__.py`).
        .env(
            "LEVEL5_G4_RAW_ROOT",
            root.join("data").display().to_string(),
        )
        .spawn()
        .map_err(|e| {
            format!(
                "échec du lancement de Blender ({}) : {e}",
                blender.display()
            )
        })?;

    // Le compte d'exportés vaut 0 sur un dump : rien n'a été copié, le modèle est lu en place.
    // Dire « 0 exporté(s) » sans l'expliquer ferait croire à un échec silencieux.
    let source = if sibling_count == 0 {
        format!("modèle lu en place dans le dump ({})", main_path.display())
    } else {
        format!(
            "{sibling_count} fichier(s) exporté(s) vers {}",
            export_dir.display()
        )
    };
    Ok(format!(
        "Blender lancé — {source} (import différé, log d'erreur : {})",
        error_log.display()
    ))
}

// ─── Installation PERSISTANTE de l'extension Blender niers (« lier au max Blender et niers ») ─
//
// [`open_in_blender`] ci-dessus est un lien TRANSITOIRE : addon activé via `sys.path` pour la
// durée d'un seul process Blender lancé PAR nie-explorer, jamais installé dans le vrai dossier
// d'addons utilisateur. Cette section installe l'extension **pour de vrai** (comme Preferences >
// Add-ons > Install from Disk le ferait) ET configure sa préférence `raw_data_root` sur le VRAI
// dossier `data/` du jeu — un Blender lancé ensuite INDÉPENDAMMENT de nie-explorer (double-clic
// sur l'icône, pas de bootstrap) a alors l'addon actif ET connaît déjà le dépôt de données niers,
// sans que l'utilisatrice n'ouvre jamais Préférences > Add-ons.

/// Zippe `addon_dir` (`plugins/niers-blender`) sous la racine [`NIERS_BLENDER_MODULE`]
/// (`niers/__init__.py`, pas `__init__.py` à plat) — requis par `bpy.ops.preferences.addon_install`
/// pour une extension multi-fichiers (cf. README de l'addon : « package the directory as ZIP
/// while keeping its folder name and `__init__.py` at the add-on root »).
///
/// La racine est le **nom de module**, pas le nom du dossier source : Blender dérive le nom du
/// module Python de l'entrée racine de l'archive, et `niers-blender` n'est pas un identifiant
/// Python valide. Exclut `.git`.
fn zip_addon_dir(addon_dir: &std::path::Path) -> Result<PathBuf, String> {
    let addon_name = NIERS_BLENDER_MODULE.to_string();
    let dest_dir = std::env::temp_dir()
        .join("nie-explorer")
        .join("blender-addon");
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let zip_path = dest_dir.join("niers-addon.zip");
    let file = std::fs::File::create(&zip_path)
        .map_err(|e| format!("création de {} : {e}", zip_path.display()))?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    fn walk(
        dir: &std::path::Path,
        base: &std::path::Path,
        addon_name: &str,
        writer: &mut zip::ZipWriter<std::fs::File>,
        options: zip::write::SimpleFileOptions,
    ) -> Result<(), String> {
        let entries =
            std::fs::read_dir(dir).map_err(|e| format!("lecture de {} : {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if entry.file_name() == ".git" {
                continue;
            }
            if path.is_dir() {
                walk(&path, base, addon_name, writer, options)?;
            } else {
                let rel = path
                    .strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                writer
                    .start_file(format!("{addon_name}/{rel}"), options)
                    .map_err(|e| e.to_string())?;
                let bytes = std::fs::read(&path)
                    .map_err(|e| format!("lecture de {} : {e}", path.display()))?;
                std::io::Write::write_all(writer, &bytes).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    walk(addon_dir, addon_dir, &addon_name, &mut writer, options)?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(zip_path)
}

/// Installe/met à jour l'extension Blender **niers** dans le vrai dossier d'addons de
/// l'utilisatrice (`bpy.ops.preferences.addon_install` + `addon_enable`, PAS le bootstrap
/// `sys.path` transitoire de [`open_in_blender`]) et configure sa préférence `raw_data_root` sur
/// le vrai `<jeu>/data` (résolu par `inferred_raw_data_root`/`candidate_data_roots` de l'addon
/// pour la recherche de squelette partagé/pièces de personnage — cf. `plugins/niers-blender/g4_animation_
/// addon.py`) — persisté via `bpy.ops.wm.save_userpref()`, donc actif au prochain lancement de
/// Blender INDÉPENDAMMENT de nie-explorer. Bloquant (`--background`, `.output()` synchrone) : pas
/// de fenêtre à garder ouverte contrairement à [`open_in_blender`], donc pas de fuite de process.
#[tauri::command]
#[specta::specta]
fn install_niers_blender_addon(
    blender_exe: Option<String>,
    game_dir: Option<String>,
) -> Result<String, String> {
    let root = resolve_root(game_dir.as_deref());
    let addon_dir = ensure_niers_blender_addon(&root)?;
    let blender = resolve_blender_exe(blender_exe)?;
    let zip_path = zip_addon_dir(&addon_dir)?;

    let data_root = root.join("data");
    let script_path = zip_path.with_file_name("_install.py");
    let script = format!(
        r#"import traceback
import bpy

OK_MARKER = "NIE_EXPLORER_ADDON_INSTALL_OK"

try:
    bpy.ops.preferences.addon_install(filepath={zip_path:?}, overwrite=True)
    bpy.ops.preferences.addon_enable(module="niers")
    prefs = bpy.context.preferences.addons["niers"].preferences
    prefs.raw_data_root = {data_root:?}
    bpy.ops.wm.save_userpref()
    print(OK_MARKER)
except Exception:
    traceback.print_exc()
    print("NIE_EXPLORER_ADDON_INSTALL_FAILED")
"#,
        zip_path = zip_path.display().to_string(),
        data_root = data_root.display().to_string(),
    );
    std::fs::write(&script_path, &script).map_err(|e| e.to_string())?;

    let output = std::process::Command::new(&blender)
        .args(["--background", "--python"])
        .arg(&script_path)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| {
            format!(
                "échec de lancement de Blender ({}) : {e}",
                blender.display()
            )
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stdout.contains("NIE_EXPLORER_ADDON_INSTALL_OK") {
        Ok(format!(
            "Extension niers installée + activée (Préférences Blender persistées). Dossier de données lié : {}",
            data_root.display()
        ))
    } else {
        // Sortie complète (stdout+stderr) tronquée : le traceback Python utile est dedans, jamais
        // avalé — contrairement au `Blender a échoué (status)` générique qu'on aurait sinon.
        let mut detail = format!("{stdout}\n{stderr}");
        const CAP: usize = 4000;
        if detail.len() > CAP {
            detail.truncate(CAP);
            detail.push_str("\n… (tronqué)");
        }
        Err(format!(
            "échec de l'installation de l'extension Blender niers :\n{detail}"
        ))
    }
}

// ─── Pont Blender ↔ niers : importer un .blend existant, construire une scène technique ──────
//
// Demande utilisatrice (2026-08-08) : « tu dois faire un pont entre blender et niers, pouvoir
// importer ce type de fichier dans niers et pouvoir construire une scène blender via niers,
// par exemple fait moi une scène avec byron love qui fait savoir suprême ». Deux commandes :
// [`blender_preview_png_b64`] (importer/prévisualiser N'IMPORTE QUEL `.blend` local dans
// nie-explorer) et [`blender_build_skill_scene`] (construire un `.blend` réel : modèle de
// personnage + modèle de cut-in de technique, tous deux de VRAIS assets VFS, jamais fabriqués).
//
// **Recette validée par un test réel headless AVANT d'écrire ce code** (même méthodologie que
// [`open_in_blender`] ci-dessus, `blender --background --python`, sur les VRAIS fichiers extraits
// du VFS pour Byron Love Aphrody `c01001900` + la technique `whs00340`/« Savoir suprême »/
// `ev60_00340`) :
// - Le chemin VFS du dossier série (`chr/_face/<sub>/<code>/`) n'est PAS toujours en minuscules
//   comme le renvoie `nie_formats::assemble::series_dir_from_code` (`"01_ie1"`, utilisé pour les
//   URLs CDN qui normalisent la casse) — le VFS réel stocke `01_IE1` (majuscules) pour ce
//   personnage, et `vfs.read()`/`vfs.iter()` sont sensibles à la casse. **Ne JAMAIS reconstruire
//   un chemin depuis `series_dir_from_code()` pour une lecture VFS directe** : toujours découvrir
//   le chemin réel par sous-chaîne sur `vfs.iter()` (même patron que [`vfs_related`]), qui donne
//   la casse exacte telle qu'indexée.
// - Le cut-in de `whs00340` (`ev60_00340`) n'a PAS de `.g4md` dans le VFS (seulement `.g4mg` +
//   `.g4pkm` + `.objbin`) — `MODEL_EXTENSIONS = {".g4md", ".g4pkm"}` côté addon (`g4_port_addon.
//   py`) : `import_scene.level5_g4` accepte aussi `.g4pkm` comme point d'entrée. Toujours essayer
//   `.g4md` d'abord (fidélité totale), replier sur `.g4pkm` si absent — jamais l'inverse deviné.
// - Résultat réel (2 objets importés) : `skeleton_root` (ARMATURE) + `wing_10` (MESH, texture
//   `wing_10M` manquante côté matériau — non bloquant, géométrie présente) — cohérent avec
//   l'élément Vent de la technique (effet d'ailes). Personnage : 3 objets (`c01001900_20`/
//   `eye_10`/`mouth_10`), 3/3 matériaux, 8/8 hashes — import fidèle confirmé.

/// Résultat de [`blender_build_skill_scene`] : chemin du `.blend` produit + aperçu rendu +
/// avertissements NON bloquants (ex. personnage introuvable dans le VFS local → scène cut-in
/// seul, jamais un échec silencieux ni un personnage substitué en douce).
#[derive(Serialize, specta::Type)]
struct BlenderSceneResultDto {
    blend_path: String,
    preview_png_b64: Option<String>,
    skill_name: String,
    event_id_name: String,
    warnings: Vec<String>,
}

/// Sous-chaîne insensible à la casse ? (les codes internes/`event_id_name` sont ASCII, une
/// comparaison octet suffit — pas de dépendance `unicode-case` pour ce besoin ponctuel).
fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Tous les chemins VFS dont le nom de fichier (pas le chemin entier, pour éviter les faux
/// positifs de dossier parent) contient `needle` — casse EXACTE telle qu'indexée (cf. note de
/// section ci-dessus : jamais reconstruite depuis un template).
fn vfs_find_by_basename(vfs: &Vfs, needle: &str) -> Vec<String> {
    let mut hits: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| contains_ci(p.rsplit('/').next().unwrap_or(p), needle))
        .collect();
    hits.sort();
    hits
}

/// Copie `src_path` (chemin VFS réel) vers `export_dir/<même chemin>`, en créant les dossiers
/// parents — préserve la structure `data/common/...`/`data/dx11/...` exigée par l'addon (même
/// contrainte documentée sur [`open_in_blender`] : `apply_original_model_to_settings` déduit le
/// code personnage/série du chemin, une extraction à plat casse cette résolution).
fn stage_vfs_file(
    vfs: &Vfs,
    src_path: &str,
    export_dir: &std::path::Path,
) -> Result<PathBuf, String> {
    let bytes = vfs
        .read(src_path)
        .map_err(|e| format!("lecture {src_path} : {e}"))?;
    let dest = export_dir.join(src_path);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// Script Python partagé : supprime le `Cube` par défaut, cadre une caméra sur tous les meshes
/// restants (bounds → position/orientation, même recette que le test réel de validation), rend un
/// still EEVEE à `{output_png}`. Utilisé par [`blender_preview_png_b64`] ET
/// [`blender_build_skill_scene`] — une seule recette de cadrage/rendu, jamais dupliquée.
fn camera_frame_and_render_py(output_png: &std::path::Path) -> String {
    format!(
        r#"
import bpy, mathutils

if "Cube" in bpy.data.objects:
    bpy.data.objects.remove(bpy.data.objects["Cube"], do_unlink=True)

meshes = [o for o in bpy.data.objects if o.type in ("MESH", "ARMATURE")]
if meshes:
    coords = []
    for o in meshes:
        bbox = getattr(o, "bound_box", None)
        if bbox:
            coords.extend(o.matrix_world @ mathutils.Vector(c) for c in bbox)
        else:
            coords.append(o.matrix_world.translation)
    if coords:
        xs = [c.x for c in coords]; ys = [c.y for c in coords]; zs = [c.z for c in coords]
        center = mathutils.Vector(((min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2, (min(zs) + max(zs)) / 2))
        radius = max(max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs), 0.5) * 1.8
        cam = bpy.data.objects.get("Camera")
        if cam is None:
            cam = bpy.data.objects.new("Camera", bpy.data.cameras.new("Camera"))
            bpy.context.scene.collection.objects.link(cam)
        cam.location = center + mathutils.Vector((radius, -radius, radius * 0.6))
        cam.rotation_euler = (center - cam.location).to_track_quat("-Z", "Y").to_euler()
        bpy.context.scene.camera = cam
        sun = bpy.data.objects.get("Light")
        if sun:
            sun.location = center + mathutils.Vector((radius, -radius, radius * 1.5))

scene = bpy.context.scene
scene.render.engine = "BLENDER_EEVEE"
scene.render.resolution_x = 960
scene.render.resolution_y = 720
scene.render.filepath = {output_png:?}
try:
    bpy.ops.render.render(write_still=True)
    print("NIE_EXPLORER_RENDER_OK")
except Exception:
    import traceback; traceback.print_exc()
    print("NIE_EXPLORER_RENDER_FAILED")
"#,
        output_png = output_png.display().to_string(),
    )
}

/// Lit `png_path` et le rend en base64 si présent (n'échoue jamais la commande appelante pour un
/// rendu manqué — l'aperçu est un bonus, pas la valeur produite).
fn read_png_b64_if_exists(png_path: &std::path::Path) -> Option<String> {
    std::fs::read(png_path)
        .ok()
        .map(|b| base64::engine::general_purpose::STANDARD.encode(&b))
}

/// Ouvre N'IMPORTE QUEL `.blend` local (pas forcément un asset VFS niers — le fichier que
/// l'utilisatrice pointe, ex. une scène déjà construite) en headless, cadre une caméra sur son
/// contenu et rend un aperçu PNG base64 — c'est le côté « importer ce type de fichier dans
/// niers » du pont : nie-explorer peut prévisualiser un `.blend` sans lancer l'UI Blender.
#[tauri::command]
#[specta::specta]
fn blender_preview_png_b64(path: String, blender_exe: Option<String>) -> Result<String, String> {
    let blender = resolve_blender_exe(blender_exe)?;
    let blend_path = PathBuf::from(&path);
    if !blend_path.is_file() {
        return Err(format!(".blend introuvable : {}", blend_path.display()));
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let work_dir = std::env::temp_dir()
        .join("nie-explorer")
        .join("blender-preview")
        .join(stamp.to_string());
    std::fs::create_dir_all(&work_dir).map_err(|e| e.to_string())?;
    let png_path = work_dir.join("preview.png");
    let script_path = work_dir.join("_preview.py");
    std::fs::write(&script_path, camera_frame_and_render_py(&png_path))
        .map_err(|e| e.to_string())?;

    let output = std::process::Command::new(&blender)
        .arg("--background")
        .arg(&blend_path)
        .args(["--python"])
        .arg(&script_path)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| {
            format!(
                "échec de lancement de Blender ({}) : {e}",
                blender.display()
            )
        })?;

    read_png_b64_if_exists(&png_path).ok_or_else(|| {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!("rendu de l'aperçu échoué :\n{stdout}\n{stderr}")
    })
}

/// Ouvre un `.blend` dans le VRAI Blender GUI (process séparé, non bloquant) — bouton « Ouvrir
/// dans Blender » après [`blender_preview_png_b64`]/[`blender_build_skill_scene`].
#[tauri::command]
#[specta::specta]
fn blender_open_scene(path: String, blender_exe: Option<String>) -> Result<(), String> {
    let blender = resolve_blender_exe(blender_exe)?;
    let blend_path = PathBuf::from(&path);
    if !blend_path.is_file() {
        return Err(format!(".blend introuvable : {}", blend_path.display()));
    }
    std::process::Command::new(&blender)
        .arg(&blend_path)
        .spawn()
        .map_err(|e| {
            format!(
                "échec de lancement de Blender ({}) : {e}",
                blender.display()
            )
        })?;
    Ok(())
}

/// Construit une VRAIE scène Blender : modèle du personnage (`internal_code`, résolu par
/// sous-chaîne sur le VFS réel — jamais un chemin template) + modèle de cut-in de la technique
/// (résolue par [`game_data::find_skill`] sur `skill_query`, chemins via `SkillInfo::
/// cutin_assets()`). Sauvegarde un `.blend` réel + rend un aperçu PNG. Aucun octet fabriqué : si
/// le personnage ou la technique n'a pas d'assets 3D dans le VFS local, la commande le dit
/// (`warnings`) plutôt que de construire une scène vide en silence ou d'échouer sans explication.
#[tauri::command]
#[specta::specta]
fn blender_build_skill_scene(
    internal_code: String,
    skill_query: String,
    blender_exe: Option<String>,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<BlenderSceneResultDto, String> {
    let blender = resolve_blender_exe(blender_exe)?;
    let root = resolve_root(game_dir.as_deref());
    let addon_dir = ensure_niers_blender_addon(&root)?;

    let staged = with_vfs(Some(root.display().to_string()), &state, |vfs| {
        let skill = game_data::find_skill(vfs, &skill_query)?
            .ok_or_else(|| format!("aucune technique ne correspond à « {skill_query} »"))?;
        let cutin = skill.cutin_assets().ok_or_else(|| {
            format!(
                "« {skill_query} » ({}) n'a pas de cut-in 3D (pas d'event lié)",
                skill.skill_id_str
            )
        })?;

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let export_dir = std::env::temp_dir()
            .join("nie-explorer")
            .join("blender-scenes")
            .join(stamp.to_string());
        std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;

        let mut warnings = Vec::new();

        // Personnage : découverte par sous-chaîne (casse réelle, cf. note de section) sous
        // chr/_face — g4md + g4mg (siblings), texture g4tx en bonus (matériau, pas bloquant).
        let chara_hits = vfs_find_by_basename(vfs, &internal_code)
            .into_iter()
            .filter(|p| p.contains("chr/_face") || p.contains("chr\\_face"))
            .collect::<Vec<_>>();
        let chara_md = chara_hits.iter().find(|p| p.ends_with(".g4md")).cloned();
        let mut chara_entry: Option<PathBuf> = None;
        if let Some(md) = &chara_md {
            for hit in &chara_hits {
                stage_vfs_file(vfs, hit, &export_dir)?;
            }
            chara_entry = Some(export_dir.join(md));
        } else if chara_hits.is_empty() {
            warnings.push(format!(
                "personnage « {internal_code} » : aucun asset 3D trouvé dans le VFS local (scène = cut-in seul)"
            ));
        } else {
            warnings.push(format!(
                "personnage « {internal_code} » : g4md absent (seulement {} fichier(s) trouvés)",
                chara_hits.len()
            ));
        }

        // Cut-in technique : g4md si présent, sinon repli .g4pkm (cf. note de section — vérifié
        // réel sur whs00340, pas deviné). g4mg/objbin/g4tx copiés en frères systématiquement.
        let ev = &cutin.event_id_name;
        let cutin_hits = vfs_find_by_basename(vfs, ev);
        let cutin_entry_name = cutin_hits
            .iter()
            .find(|p| p.ends_with(".g4md"))
            .or_else(|| cutin_hits.iter().find(|p| p.ends_with(".g4pkm")))
            .cloned();
        let mut cutin_entry: Option<PathBuf> = None;
        if let Some(entry) = &cutin_entry_name {
            for hit in &cutin_hits {
                stage_vfs_file(vfs, hit, &export_dir)?;
            }
            cutin_entry = Some(export_dir.join(entry));
        } else {
            warnings.push(format!("technique « {ev} » : aucun modèle de cut-in (.g4md/.g4pkm) trouvé dans le VFS local"));
        }

        if chara_entry.is_none() && cutin_entry.is_none() {
            return Err(format!(
                "aucun asset 3D trouvé ni pour le personnage « {internal_code} » ni pour la technique « {skill_query} » — scène impossible à construire"
            ));
        }

        Ok((
            skill.skill_id_str.clone(),
            cutin.event_id_name.clone(),
            export_dir,
            chara_entry,
            cutin_entry,
            warnings,
        ))
    })?;
    let (skill_name, event_id_name, export_dir, chara_entry, cutin_entry, warnings) = staged;

    let blend_dir = std::env::temp_dir()
        .join("nie-explorer")
        .join("blender-scenes-out");
    std::fs::create_dir_all(&blend_dir).map_err(|e| e.to_string())?;
    let out_blend = blend_dir.join(format!("{internal_code}_{skill_name}.blend"));
    let out_png = export_dir.join("preview.png");
    let error_log = export_dir.join("_nie_explorer_scene_error.log");

    let script = format!(
        r#"import importlib.util, sys, traceback
# Cf. `open_in_blender` : dossier à tiret → chargement par chemin, pas par nom de module.
try:
    _spec = importlib.util.spec_from_file_location({module_name:?}, {addon_init:?})
    g4b = importlib.util.module_from_spec(_spec)
    sys.modules[{module_name:?}] = g4b
    _spec.loader.exec_module(g4b)
    g4b.register()
except Exception:
    traceback.print_exc()

import bpy

ERROR_LOG = {error_log:?}
errors = []

def try_import(filepath):
    if filepath is None:
        return
    try:
        bpy.ops.import_scene.level5_g4(
            'EXEC_DEFAULT',
            filepath=filepath,
            skip_character_setup=True,
            import_character_parts=False,
            create_report_text=False,
        )
        print("NIE_EXPLORER_IMPORT_OK", filepath)
    except Exception:
        tb = traceback.format_exc()
        print(tb)
        errors.append(tb)

try_import({chara_entry:?})
try_import({cutin_entry:?})

if errors:
    try:
        with open(ERROR_LOG, "w", encoding="utf-8") as f:
            f.write("\n---\n".join(errors))
    except Exception:
        pass

{render_script}

bpy.ops.wm.save_as_mainfile(filepath={out_blend:?})
print("NIE_EXPLORER_SCENE_SAVED", {out_blend:?})
"#,
        module_name = NIERS_BLENDER_MODULE,
        addon_init = addon_dir.join("__init__.py").display().to_string(),
        error_log = error_log.display().to_string(),
        chara_entry = chara_entry.as_ref().map(|p| p.display().to_string()),
        cutin_entry = cutin_entry.as_ref().map(|p| p.display().to_string()),
        render_script = camera_frame_and_render_py(&out_png),
        out_blend = out_blend.display().to_string(),
    );
    let script_path = export_dir.join("_build_scene.py");
    std::fs::write(&script_path, &script).map_err(|e| e.to_string())?;

    let output = std::process::Command::new(&blender)
        .args(["--background", "--python"])
        .arg(&script_path)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| {
            format!(
                "échec de lancement de Blender ({}) : {e}",
                blender.display()
            )
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stdout.contains("NIE_EXPLORER_SCENE_SAVED") {
        let mut detail = format!("{stdout}\n{stderr}");
        const CAP: usize = 4000;
        if detail.len() > CAP {
            detail.truncate(CAP);
            detail.push_str("\n… (tronqué)");
        }
        return Err(format!(
            "échec de construction de la scène Blender :\n{detail}"
        ));
    }

    let mut warnings = warnings;
    if let Ok(log) = std::fs::read_to_string(&error_log) {
        if !log.trim().is_empty() {
            warnings.push(format!("import partiel — détail :\n{log}"));
        }
    }

    Ok(BlenderSceneResultDto {
        blend_path: out_blend.display().to_string(),
        preview_png_b64: read_png_b64_if_exists(&out_png),
        skill_name,
        event_id_name,
        warnings,
    })
}

// ─── Aperçu 3D (G4MD+G4MG → GLB embarqué → viewport WebGL commun) ────────────────────────────
//
// PAS de `bpy` (Blender-comme-module-Python, `pip install bpy`, existe officiellement —
// developer.blender.org/docs/handbook/building_blender/python_module — et PyPI publie bien un
// build 5.2.0/Python 3.13 correspondant à la version installée) : vérifié après coup que ce
// module embarquable n'a PAS de window manager (`bpy.context.window`/`context.workspace`
// absents, opérateurs GUI en échec) — or `plugins/niers-blender` s'appuie dessus (barre de progression
// `context.window_manager.progress_update`, panneau N, préférences d'addon), donc PAS un bon
// candidat à l'embarquement headless sans réécrire l'addon. L'intégration Blender de ce fichier
// ([`open_in_blender`]) lance donc le vrai Blender GUI en process séparé — choix délibéré, pas
// une lacune d'API. Pour l'aperçu instantané, l'application assemble le GLB avec
// `nie_formats::assemble::assemble_generic_model` puis le charge dans son viewport WebGL commun :
// pas de rasterisation intermédiaire, de frame PNG ou de processus supplémentaire.

/// Découpe un chemin VFS en `(préfixe de dossier, nom de fichier, radical)` — le radical est le
/// nom sans sa dernière extension, c'est lui qui nomme toute la famille d'un asset
/// (`c000101.g4mg`, `c000101.g4sk`, `c000101_p010.g4pk`…).
fn split_vfs_path(path: &str) -> (&str, &str, &str) {
    let base = path.rsplit('/').next().unwrap_or(path);
    let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    let dir_prefix = path.strip_suffix(base).unwrap_or("");
    (dir_prefix, base, stem)
}

/// Noms de fichiers présents **directement** sous `dir_prefix`, triés. Sert à rendre un « frère
/// introuvable » diagnosticable : un dossier de personnage ne contient pas toujours le `.g4md`
/// attendu (`data/common/chr/c000101` n'a que `.g4mg`/`.g4sk`/`.g4pk`), et le dire vaut mieux que
/// de renvoyer un échec nu.
fn vfs_dir_filenames(vfs: &Vfs, dir_prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = vfs
        .iter()
        .filter_map(|(p, _)| p.strip_prefix(dir_prefix))
        .filter(|rest| !rest.is_empty() && !rest.contains('/'))
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Rend `names` lisible dans un message d'erreur, tronqué : un dossier de personnage dépasse la
/// cinquantaine d'entrées, les recracher toutes noierait le diagnostic.
fn summarize_names(names: &[String]) -> String {
    const SHOWN: usize = 12;
    if names.is_empty() {
        return "aucun fichier indexé".to_string();
    }
    let head = names
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > SHOWN {
        format!("{head}, … (+{} autres)", names.len() - SHOWN)
    } else {
        head
    }
}

/// Assemble le GLB (G4MD+G4MG+G4TX frère, cf. commentaire de section) pour `path`.
///
/// C'est l'unique préparation VFS du viewport temps réel commun : aucune image ou vidéo
/// concurrente n'est produite côté backend.
pub(crate) fn assemble_glb_for_preview(
    vfs: &nie_formats::vfs::Vfs,
    path: &str,
) -> Result<(String, Vec<u8>), String> {
    use nie_formats::assemble::{assemble_generic_model, GenericModelInput, MeshComponent};

    let data = vfs.read(path).map_err(|e| e.to_string())?;

    let (dir_prefix, _base, stem) = split_vfs_path(path);
    let sibling = |ext: &str| -> Option<Vec<u8>> {
        let candidate = format!("{dir_prefix}{stem}.{ext}");
        if candidate == path {
            Some(data.clone())
        } else {
            vfs.read(&candidate).ok()
        }
    };

    // Le frère manquant est le cas RÉEL le plus fréquent (cf. `data/common/chr/c000101`, qui n'a
    // ni `.g4md` ni `.g4mg` : sa géométrie vit ailleurs) : on nomme le chemin cherché ET le
    // voisinage réel, sinon l'utilisateur ne peut ni corriger sa sélection ni conclure.
    let missing = |ext: &str| -> String {
        format!(
            "{} introuvable : cherché « {dir_prefix}{stem}.{ext} » ; présent dans {} : {}",
            ext.to_uppercase(),
            if dir_prefix.is_empty() {
                "la racine du VFS"
            } else {
                dir_prefix
            },
            summarize_names(&vfs_dir_filenames(vfs, dir_prefix)),
        )
    };
    let g4md = sibling("g4md").ok_or_else(|| missing("g4md"))?;
    let g4mg = sibling("g4mg").ok_or_else(|| missing("g4mg"))?;

    let mut model = assemble_generic_model(GenericModelInput {
        code: stem.to_string(),
        g4md,
        g4mg,
        component: MeshComponent::Generic,
    })
    .map_err(|e| format!("assemblage GLB : {e}"))?;

    model.strict_materials = true;
    if let Some(g4tx) = sibling("g4tx").or_else(|| {
        vfs.read(&format!("{dir_prefix}{stem}.g4tx").replace("/common/", "/dx11/"))
            .ok()
    }) {
        bind_preview_textures(&mut model, &g4tx, stem);
    }

    Ok((stem.to_string(), model.to_glb_embedded()))
}

/// Les cheveux, yeux et bouche partagent un conteneur, pas nécessairement une image.
/// Une correspondance manquante reste neutre grâce au mode strict.
fn bind_preview_textures(
    model: &mut nie_formats::assemble::AssembledModel,
    g4tx: &[u8],
    stem: &str,
) {
    use nie_formats::assemble::{avatar_texture_name, EmbeddedTexture};
    let mut seen = std::collections::HashSet::new();
    model.strict_materials = true;
    for primitive in &model.primitives {
        let material = &primitive.material_name;
        if !seen.insert(material.clone()) {
            continue;
        }
        let base = avatar_texture_name(material);
        let facial_atlas = format!("{stem}_10");
        let texture = if matches!(base, "eye_10" | "mouth_10") {
            facial_atlas.as_str()
        } else {
            base
        };
        if let Some(png_bytes) = nie_formats::g4tx_decode::decode_named_to_png(g4tx, texture) {
            model.embedded_textures.push(EmbeddedTexture {
                component: primitive.component,
                name: material.clone(),
                png_bytes,
            });
        }
    }
}

/// Même logique que [`assemble_glb_for_preview`] (résolution de frères g4mg/g4tx + assemblage
/// GLB), mais scopée aux entrées d'un CPK brut ouvert ([`RawCpkState`]) plutôt qu'au VFS complet
/// — ferme le gap documenté `apps/nie-explorer/ROADMAP.md` §6 (« parité RawCpkView/DetailPane »,
/// aperçu 3D listé « hors de portée pour un CPK ouvert hors VFS » faute d'un « résolveur de
/// frères scopé au seul CPK courant »). Correspondance par (dossier, basename) au lieu d'un
/// chemin VFS complet : un CPK brut ouvert hors VFS n'a pas de préfixe `data/...` fiable, mais
/// `CpkEntry` porte déjà `directory`/`filename` séparément — pas besoin de reconstruire un chemin.
fn assemble_glb_from_cpk_entries(
    data: &[u8],
    reader: &CpkReader,
    entry: &CpkEntry,
) -> Result<(String, Vec<u8>), String> {
    use nie_formats::assemble::{assemble_generic_model, GenericModelInput, MeshComponent};

    let stem = entry
        .filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(&entry.filename)
        .to_string();
    let sibling = |ext: &str| -> Option<Vec<u8>> {
        let target = format!("{stem}.{ext}");
        reader
            .entries
            .iter()
            .find(|e| e.directory == entry.directory && e.filename.eq_ignore_ascii_case(&target))
            .and_then(|e| reader.extract(data, e).ok())
    };

    let g4md =
        sibling("g4md").ok_or("G4MD introuvable dans ce CPK (même dossier, même nom de base)")?;
    let g4mg =
        sibling("g4mg").ok_or("G4MG introuvable dans ce CPK (frère requis pour la géométrie)")?;

    let mut model = assemble_generic_model(GenericModelInput {
        code: stem.clone(),
        g4md,
        g4mg,
        component: MeshComponent::Generic,
    })
    .map_err(|e| format!("assemblage GLB : {e}"))?;

    model.strict_materials = true;
    if let Some(g4tx) = sibling("g4tx") {
        bind_preview_textures(&mut model, &g4tx, &stem);
    }

    Ok((stem, model.to_glb_embedded()))
}

/// Ouvre l'asset dans **nie-editor**, l'éditeur de scène 3D natif (éditeur Fyrox embarqué, rendu
/// OpenGL — cf. `crates/tools/nie-editor`).
///
/// Process séparé et non bloquant : l'éditeur a sa propre boucle d'événements winit et sa propre
/// fenêtre GPU, deux choses qui ne peuvent pas cohabiter avec la boucle Tauri de cette
/// application. Le binaire est cherché à côté de l'exécutable courant (build distribué), puis dans
/// les cibles de développement du workspace.
#[tauri::command]
#[specta::specta]
fn open_in_scene_editor(path: Option<String>, game_dir: Option<String>) -> Result<String, String> {
    let root = resolve_root(game_dir.as_deref());
    let exe_name = if cfg!(windows) {
        "nie-editor.exe"
    } else {
        "nie-editor"
    };

    let mut candidates: Vec<PathBuf> = Vec::new();
    // Pas de `let`-chain ici : ce crate est en édition 2021 (contrairement au workspace), qui ne
    // les accepte pas.
    if let Ok(current) = std::env::current_exe() {
        if let Some(dir) = current.parent() {
            candidates.push(dir.join(exe_name));
        }
    }
    for profile in ["release", "debug"] {
        candidates.push(root.join("target").join(profile).join(exe_name));
    }

    let editor =
        candidates
            .iter()
            .find(|p| p.is_file())
            .ok_or_else(|| {
                format!(
                "nie-editor introuvable. Compilez-le avec « cargo build -p nie-editor --release » \
                 (emplacements cherchés : {})",
                candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
            )
            })?
            .clone();

    let mut cmd = std::process::Command::new(&editor);
    cmd.arg("--game-dir").arg(&root);
    if let Some(asset) = path.as_deref().filter(|p| !p.trim().is_empty()) {
        cmd.arg("--asset").arg(asset);
    }
    cmd.spawn()
        .map_err(|e| format!("lancement de {} : {e}", editor.display()))?;

    Ok(match path {
        Some(p) => format!("Éditeur de scène ouvert sur {p}"),
        None => "Éditeur de scène ouvert".to_string(),
    })
}

// ─── Atelier Lua (cf. `lua_tools.rs`) ────────────────────────────────────────────────────────
//
// Les scripts du jeu vivent dans le VFS (`.lua.bin`) ou sur disque. Chaque commande accepte donc
// SOIT un chemin VFS, SOIT une source éditée dans l'interface : c'est ce qui permet d'ouvrir un
// script du jeu, le modifier et le relancer sans jamais écrire de fichier temporaire.

/// Lit un script : source fournie telle quelle, ou chemin VFS résolu contre le jeu monté.
fn lua_source_bytes(
    source: Option<String>,
    path: Option<String>,
    game_dir: Option<String>,
    state: &tauri::State<VfsState>,
) -> Result<Vec<u8>, String> {
    if let Some(src) = source {
        return Ok(src.into_bytes());
    }
    let path = path.ok_or("ni source ni chemin fourni")?;
    with_vfs(game_dir, state, |vfs| {
        vfs.read(&path).map_err(|e| e.to_string())
    })
}

/// En-tête + statistiques d'un chunk Lua (`.lua.bin`).
#[tauri::command]
#[specta::specta]
fn lua_chunk_info(
    path: Option<String>,
    source: Option<String>,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<lua_tools::LuaChunkInfoDto, String> {
    let data = lua_source_bytes(source, path, game_dir, &state)?;
    lua_tools::chunk_info(&data)
}

/// Désassemble un chunk Lua en listing lisible.
#[tauri::command]
#[specta::specta]
fn lua_disassemble(
    path: Option<String>,
    source: Option<String>,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<String, String> {
    let data = lua_source_bytes(source, path, game_dir, &state)?;
    lua_tools::disassemble(&data)
}

/// Exécute un script dans la VRAIE VM Lua 5.2 du jeu et renvoie sortie, erreur et appels moteur
/// manquants.
#[tauri::command]
#[specta::specta]
fn lua_execute(
    path: Option<String>,
    source: Option<String>,
    with_menu_host: bool,
    instruction_limit: Option<u32>,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<lua_tools::LuaExecResultDto, String> {
    let name = path.clone().unwrap_or_else(|| "éditeur".to_string());
    let data = lua_source_bytes(source, path, game_dir, &state)?;
    lua_tools::execute(&data, &name, with_menu_host, instruction_limit)
}

/// Exécute un script puis renvoie ses globals — l'éditeur de valeurs. `overrides` pose des valeurs
/// AVANT l'exécution (rejouer « comme si » telle variable moteur valait autre chose).
#[tauri::command]
#[specta::specta]
fn lua_globals(
    path: Option<String>,
    source: Option<String>,
    with_menu_host: bool,
    overrides: Vec<(String, String)>,
    include_stdlib: bool,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<lua_tools::LuaGlobalDto>, String> {
    let name = path.clone().unwrap_or_else(|| "éditeur".to_string());
    let data = lua_source_bytes(source, path, game_dir, &state)?;
    lua_tools::globals_after_run(&data, &name, with_menu_host, &overrides, include_stdlib)
}

/// Évalue une expression dans l'état laissé par le script — la console.
#[tauri::command]
#[specta::specta]
fn lua_eval(
    path: Option<String>,
    source: Option<String>,
    expression: String,
    with_menu_host: bool,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<String, String> {
    let name = path.clone().unwrap_or_else(|| "éditeur".to_string());
    let data = lua_source_bytes(source, path, game_dir, &state)?;
    lua_tools::eval(&data, &name, &expression, with_menu_host)
}

// ── Session Lua persistante (cf. `lua_session.rs`) ───────────────────────────────────────────
//
// Distincte des commandes `lua_execute`/`lua_eval` ci-dessus, qui repartent d'une VM neuve à
// chaque appel : celles-là servent à ANALYSER un script (deux analyses ne se contaminent pas),
// celles-ci à TRAVAILLER avec (l'état survit, la console est un vrai REPL).

/// Exécute un chunk dans la session vivante.
#[tauri::command]
#[specta::specta]
fn lua_session_exec(
    path: Option<String>,
    source: Option<String>,
    game_dir: Option<String>,
    vfs: tauri::State<VfsState>,
    session: tauri::State<lua_session::LuaSessionHandle>,
) -> Result<Vec<String>, String> {
    let name = path.clone().unwrap_or_else(|| "éditeur".to_string());
    let data = lua_source_bytes(source, path, game_dir, &vfs)?;
    session.exec(name, data)
}

/// Attache un script comme comportement (il doit renvoyer une table) et renvoie ses callbacks.
#[tauri::command]
#[specta::specta]
fn lua_session_attach(
    path: Option<String>,
    source: Option<String>,
    game_dir: Option<String>,
    vfs: tauri::State<VfsState>,
    session: tauri::State<lua_session::LuaSessionHandle>,
) -> Result<Vec<String>, String> {
    let name = path.clone().unwrap_or_else(|| "éditeur".to_string());
    let data = lua_source_bytes(source, path, game_dir, &vfs)?;
    session.attach(name, data)
}

/// Diffuse un callback de cycle de vie à tous les comportements attachés.
#[tauri::command]
#[specta::specta]
fn lua_session_broadcast(
    callback: String,
    session: tauri::State<lua_session::LuaSessionHandle>,
) -> Result<u32, String> {
    session.broadcast(callback)
}

/// Évalue une expression dans l'état COURANT de la session.
#[tauri::command]
#[specta::specta]
fn lua_session_eval(
    expression: String,
    session: tauri::State<lua_session::LuaSessionHandle>,
) -> Result<String, String> {
    session.eval(expression)
}

/// Pose une valeur globale dans la session vivante.
#[tauri::command]
#[specta::specta]
fn lua_session_set_global(
    name: String,
    expression: String,
    session: tauri::State<lua_session::LuaSessionHandle>,
) -> Result<(), String> {
    session.set_global(name, expression)
}

/// Globals de la session.
#[tauri::command]
#[specta::specta]
fn lua_session_globals(
    include_stdlib: bool,
    session: tauri::State<lua_session::LuaSessionHandle>,
) -> Result<Vec<lua_session::LuaSessionGlobalDto>, String> {
    session.globals(include_stdlib)
}

/// Recrée la VM et ré-attache les comportements — le `RefreshAll` d'Overload.
#[tauri::command]
#[specta::specta]
fn lua_session_reload(session: tauri::State<lua_session::LuaSessionHandle>) -> Result<(), String> {
    session.reload()
}

/// Récupère et vide la sortie accumulée (print + `Debug.*`).
#[tauri::command]
#[specta::specta]
fn lua_session_drain(
    session: tauri::State<lua_session::LuaSessionHandle>,
) -> Result<lua_session::LuaDrainDto, String> {
    session.drain()
}

/// Confronte l'API réclamée par les scripts à celle que l'hôte fournit.
#[tauri::command]
#[specta::specta]
fn lua_session_api_report(
    session: tauri::State<lua_session::LuaSessionHandle>,
) -> Result<lua_session::LuaApiReportDto, String> {
    session.api_report()
}

/// Liste les scripts Lua du VFS (`.lua.bin`/`.lua`), triés — le catalogue de l'atelier.
#[tauri::command]
#[specta::specta]
fn lua_list_scripts(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<Vec<EntryDto>, String> {
    with_vfs(game_dir, &state, |vfs| {
        let mut out: Vec<EntryDto> = vfs
            .iter()
            .filter(|(p, _)| {
                let lower = p.to_ascii_lowercase();
                lower.ends_with(".lua.bin") || lower.ends_with(".lua")
            })
            .map(|(p, e)| EntryDto {
                path: p.to_string(),
                name: p.rsplit('/').next().unwrap_or(p).to_string(),
                size: e.file_size,
                cpk: e.cpk_filename.clone(),
            })
            .collect();
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    })
}

/// Renvoie le **GLB assemblé lui-même** (base64), pas un rendu de celui-ci.
///
/// Le frontend le charge dans le moteur temps réel WebGL commun : caméra libre, éclairage,
/// sélection de maillage — le viewport d'un éditeur, pas une planche-contact.
///
/// Le GLB est auto-suffisant : `to_glb_embedded` embarque géométrie ET textures (le `.g4tx` frère
/// décodé en PNG, cf. [`assemble_glb_for_preview`]), donc aucun aller-retour supplémentaire pour
/// les ressources.
#[tauri::command]
#[specta::specta]
async fn vfs_glb_bytes_b64(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<'_, VfsState>,
) -> Result<String, String> {
    let vfs = vfs_partage(game_dir, &state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (_stem, glb) = assemble_glb_for_preview(&vfs, &path)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(&glb))
    })
    .await
    .map_err(|error| format!("Assemblage 3D interrompu : {error}"))?
}

// ─── Clips d'animation (G4MT dans les G4PK frères) — LISTE SEULE ────────────────────────────
//
// Le GLB renvoyé par [`vfs_glb_bytes_b64`] ne porte AUCUNE animation : `nie_formats::assemble`
// n'émet ni `skins`, ni `animations`, ni attribut `JOINTS_0`. Les rejouer dans le viewport
// supposerait d'écrire l'export skinné (skin glTF + échantillonnage des canaux G4MT), chantier
// Rust à part entière. Ce que le dépôt sait déjà faire, en revanche, c'est LIRE la table de
// clips : `g4mt::Motion::parse` donne nom, bornes de frames, fps, bit additif et cibles.
//
// PIÈGE : le `.g4mt` n'est presque jamais un frère direct du modèle. Il vit DANS une archive
// `.g4pk` (`data/common/chr/c000101` : 27 archives `c000101_p0XX.g4pk`, une par pose/animation,
// chacune contenant un unique `c000101_p0XX.g4mt`). La résolution passe donc par
// `g4pk::parse` puis par l'entrée dont le nom finit en `.g4mt`, exactement comme
// `nie-formats/examples/anim_mesh.rs`.

/// Un clip déclaré par un conteneur G4MT.
///
/// Tous les entiers sont des `f64` : `specta` refuse les entiers 64 bits (`u64`/`i64` panique à
/// la génération de bindings), et les champs concernés (u32 au plus) y tiennent exactement.
#[derive(Serialize, specta::Type)]
struct MotionClipDto {
    /// Archive `.g4pk` d'où provient le clip (chemin VFS complet).
    archive: String,
    /// Nom du sous-fichier `.g4mt` dans cette archive.
    motion_file: String,
    name: String,
    /// CRC32 du nom de clip — l'identifiant par lequel le jeu le référence.
    crc32: f64,
    start_frame: f64,
    end_frame: f64,
    /// Bornes incluses (`g4mt::Clip::frame_count`).
    frame_count: f64,
    fps: f64,
    /// Clip additif (superposé à une pose de base), `g4mt::Clip::is_additive`.
    additive: bool,
    /// Nombre d'os/cibles animés par le clip (`Motion::target_indices`, dédupliqué).
    target_count: f64,
}

/// Réponse de [`vfs_motion_clips`].
#[derive(Serialize, specta::Type)]
struct MotionClipsDto {
    /// Archives `.g4pk` réellement ouvertes, dans l'ordre d'inspection.
    archives: Vec<String>,
    clips: Vec<MotionClipDto>,
    /// Pourquoi la liste est vide ou incomplète. Une absence d'animation n'est pas une erreur :
    /// beaucoup d'assets n'en ont pas, et échouer ferait passer un fait pour une panne.
    notice: Option<String>,
}

/// Liste les clips d'animation d'un asset : archives `.g4pk` de même radical → sous-fichier
/// `.g4mt` → table de clips. **Lecture seule** — rien ici ne rejoue l'animation (cf. commentaire
/// de section : le GLB d'aperçu n'a pas de skin).
///
/// `path` est n'importe quel membre de la famille (`.g4md`, `.g4mg`, `.g4sk`, ou une `.g4pk`
/// précise) : seul son radical compte. Coût réel : une archive de personnage pèse quelques Mo et
/// il y en a des dizaines, la commande lit donc ~100 Mo — d'où l'appel asynchrone côté frontend.
#[tauri::command]
#[specta::specta]
fn vfs_motion_clips(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<MotionClipsDto, String> {
    let root = resolve_root(game_dir.as_deref());
    with_vfs(Some(root.display().to_string()), &state, |vfs| {
        let (dir_prefix, base, stem) = split_vfs_path(&path);

        // Une `.g4pk` désignée nommément ne se fait pas remplacer par ses sœurs : l'utilisateur a
        // demandé CETTE animation.
        let archives: Vec<String> = if base.to_ascii_lowercase().ends_with(".g4pk") {
            vec![path.clone()]
        } else {
            let mut hits: Vec<String> = vfs
                .iter()
                .filter_map(|(p, _)| p.strip_prefix(dir_prefix).map(|rest| (p, rest)))
                .filter(|(_, rest)| !rest.contains('/'))
                .filter(|(_, rest)| {
                    let lower = rest.to_ascii_lowercase();
                    let Some(name) = lower.strip_suffix(".g4pk") else {
                        return false;
                    };
                    let stem_lower = stem.to_ascii_lowercase();
                    // `_` obligatoire après le radical : sans lui, `c0001` happerait `c000101`.
                    name == stem_lower
                        || name
                            .strip_prefix(&stem_lower)
                            .is_some_and(|s| s.starts_with('_'))
                })
                .map(|(p, _)| p.to_string())
                .collect();
            hits.sort();
            hits
        };

        if archives.is_empty() {
            return Ok(MotionClipsDto {
                archives,
                clips: Vec::new(),
                notice: Some(format!(
                    "aucune archive « {stem}*.g4pk » dans {} — présent : {}",
                    if dir_prefix.is_empty() {
                        "la racine du VFS"
                    } else {
                        dir_prefix
                    },
                    summarize_names(&vfs_dir_filenames(vfs, dir_prefix)),
                )),
            });
        }

        let mut clips = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        for archive in &archives {
            let data = match vfs.read(archive) {
                Ok(d) => d,
                Err(e) => {
                    skipped.push(format!("{archive} (lecture : {e})"));
                    continue;
                }
            };
            let pk = match nie_formats::g4pk::parse(&data) {
                Ok(pk) => pk,
                Err(e) => {
                    skipped.push(format!("{archive} (G4PK : {e})"));
                    continue;
                }
            };
            let mut found_motion = false;
            for file in pk
                .files
                .iter()
                .filter(|f| f.name.to_ascii_lowercase().ends_with(".g4mt"))
            {
                found_motion = true;
                let Some(bytes) = data.get(file.offset..file.offset + file.size) else {
                    skipped.push(format!("{archive}/{} (bornes hors archive)", file.name));
                    continue;
                };
                let Some(motion) = nie_formats::g4mt::Motion::parse(bytes) else {
                    skipped.push(format!("{archive}/{} (G4MT illisible)", file.name));
                    continue;
                };
                for clip in &motion.clips {
                    clips.push(MotionClipDto {
                        archive: archive.clone(),
                        motion_file: file.name.clone(),
                        name: clip.name.clone(),
                        crc32: f64::from(clip.crc32),
                        start_frame: f64::from(clip.start_frame),
                        end_frame: f64::from(clip.end_frame),
                        frame_count: f64::from(clip.frame_count()),
                        fps: f64::from(clip.fps),
                        additive: clip.is_additive(),
                        target_count: motion.target_indices(clip).len() as f64,
                    });
                }
            }
            if !found_motion {
                skipped.push(format!("{archive} (aucun sous-fichier .g4mt)"));
            }
        }

        let notice = if skipped.is_empty() {
            None
        } else if clips.is_empty() {
            Some(format!("aucun clip lisible — {}", skipped.join(" ; ")))
        } else {
            Some(format!(
                "{} archive(s) écartée(s) : {}",
                skipped.len(),
                skipped.join(" ; ")
            ))
        };
        Ok(MotionClipsDto {
            archives,
            clips,
            notice,
        })
    })
}

/// Même chose que [`vfs_glb_bytes_b64`] pour une entrée d'un `.cpk` ouvert hors VFS — résolution
/// de frères scopée au CPK courant, cf. [`assemble_glb_from_cpk_entries`].
#[tauri::command]
#[specta::specta]
fn raw_cpk_glb_bytes_b64(index: u32, state: tauri::State<RawCpkState>) -> Result<String, String> {
    let guard = state.0.lock().unwrap();
    let (_, data, reader) = guard.as_ref().ok_or("aucun CPK ouvert")?;
    let entry = reader
        .entries
        .get(index as usize)
        .ok_or("index d'entrée invalide")?;
    let (_stem, glb) = assemble_glb_from_cpk_entries(data, reader, entry)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&glb))
}

// ─── Aperçu audio (ADX/HCA/AWB/ACB → WAV, natif Rust — clé IEVR déjà reversée) ─────────

/// Décode n'importe quel format audio Criware du VFS (`.acb`/`.awb`/`.hca`/`.adx`, dispatch par
/// magic) en WAV PCM16, base64 — `nie_formats::cri_audio::decode_to_wav` (feature `audio-decode`,
/// `cridecoder` + `IEVR_HCA_KEY` reversé de `nie.exe`, vérifié byte-exact sur `c00001001.awb`
/// (48 kHz mono, non silencieux) — cf. `docs/PLAN.md` § C1).
///
/// Décodage lancé sur un THREAD DÉDIÉ à pile de 16 Mio : trouvé par test réel (pas supposé) —
/// `cridecoder` fait un vrai `STATUS_STACK_OVERFLOW` sur la pile debug par défaut (~1 Mio
/// Windows) sur `c01000010.awb` réel (fonctionne en `--release`, casse en `cargo build`/
/// `tauri dev`, le mode utilisé pendant tout le développement de cette app). Un
/// `STATUS_STACK_OVERFLOW` tue le PROCESS entier (fault SEH, pas rattrapable par
/// `catch_unwind`/`thread::join`) : la pile élargie doit réellement suffire, ce n'est pas un
/// filet de sécurité — reconfirmé : le même fichier décode sans erreur sur un thread à 16 Mio,
/// y compris en debug non optimisé.
#[tauri::command]
#[specta::specta]
fn vfs_audio_preview_b64(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<String, String> {
    let data = with_vfs(game_dir, &state, |vfs| {
        vfs.read(&path).map_err(|e| e.to_string())
    })?;
    audio_wav_b64_from_bytes(data)
}

/// Une piste d'une banque audio, telle que l'interface la liste.
#[derive(Serialize, specta::Type)]
struct CueDto {
    /// Nom donné par la banque (`ev74_00840_me`), vide si elle ne nomme pas la piste.
    name: String,
    /// Cue-id AFS2 — l'identifiant à repasser à [`vfs_audio_cue_wav_b64`]. `null` = non jouable.
    awb_id: Option<u16>,
    /// Durée annoncée, en millisecondes (`0` si inconnue).
    length_ms: u32,
    /// Codec en clair (`HCA`, `ADX`…), vide si non résolu.
    codec: String,
    sample_rate: Option<u32>,
    channels: Option<u8>,
    /// Taille des octets de la piste, `null` si la banque d'octets n'a pas été ouverte.
    size: Option<u32>,
    /// Nom de fichier proposé au téléchargement — celui du CUE, jamais celui de la banque.
    filename: String,
}

/// Catalogue des pistes d'une banque audio du VFS, avec la provenance de ses octets.
#[derive(Serialize, specta::Type)]
struct AudioBankDto {
    /// `self` (le fichier EST l'AWB), `embedded`, ou le chemin VFS de l'AWB frère.
    source: String,
    /// Vrai si les octets sont réellement atteignables — sinon les pistes sont listées mais muettes.
    playable: bool,
    cues: Vec<CueDto>,
}

/// Au-delà de cette taille, l'AWB n'est PAS ouvert pour renseigner les tailles de piste.
///
/// Le catalogue vient de l'ACB, qui pèse deux ordres de grandeur de moins : lire un AWB de
/// 1,25 Gio pour afficher une colonne « taille » ferait payer un gigaoctet de disque à chaque
/// sélection dans l'explorateur. La lecture d'UNE piste, elle, reste possible : elle passe par
/// [`vfs_audio_cue_wav_b64`], à la demande.
const AWB_TAILLES_MAX: u32 = 64 * 1024 * 1024;

/// Liste les pistes jouables d'un `.acb`/`.awb` du VFS — **sans en décoder aucune**.
///
/// C'est ce qui manquait à l'explorateur : [`vfs_audio_preview_b64`] rend UNE piste par fichier
/// (la plus volumineuse), alors qu'une banque en décrit jusqu'à 1 512. Le catalogue vient de
/// l'ACB quand il y en a un — noms, durées, codec, fréquence — sans ouvrir l'AWB.
#[tauri::command]
#[specta::specta]
fn vfs_audio_cues(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<AudioBankDto, String> {
    with_vfs(game_dir, &state, |vfs| {
        let data = vfs.read(&path).map_err(|e| e.to_string())?;

        // LOCALISER, pas résoudre : un AWB externe n'est pas lu pour être listé. Le catalogue
        // vient de l'ACB, qui se suffit ; seule la colonne « taille » a besoin des octets, et
        // elle ne vaut pas le gigaoctet que coûterait `waza_stream.awb` à chaque sélection.
        let localise = nie_explore::audio::localiser_awb(vfs, &path, &data);
        let (source, playable, deja_en_main, taille) = match localise {
            None => ("aucune".to_string(), false, None, 0),
            Some((nie_explore::audio::SourceAwb::Autonome, _, t)) => {
                ("self".to_string(), true, None, t)
            }
            Some((nie_explore::audio::SourceAwb::Embarquee, bytes, t)) => {
                ("embedded".to_string(), true, bytes, t)
            }
            Some((nie_explore::audio::SourceAwb::Externe(p), _, t)) => (p.clone(), true, None, t),
        };

        // Les octets à passer au catalogue, dans l'ordre de coût croissant :
        //   1. le fichier lui-même s'il EST la banque (déjà lu) ;
        //   2. l'AWB embarqué, extrait par le parse ACB (déjà en mémoire) ;
        //   3. un externe, seulement s'il est assez petit — sinon aucune taille de piste.
        let externe = (playable
            && deja_en_main.is_none()
            && !data.starts_with(b"AFS2")
            && taille <= u64::from(AWB_TAILLES_MAX))
        .then(|| vfs.read(&source).ok())
        .flatten();
        let octets: Option<&[u8]> = if data.starts_with(b"AFS2") {
            Some(&data)
        } else {
            deja_en_main.as_deref().or(externe.as_deref())
        };

        let cues = nie_explore::audio::cues(&data, octets);
        Ok(AudioBankDto {
            source,
            playable,
            cues: cues.iter().map(|c| cue_dto(&path, c)).collect(),
        })
    })
}

/// Miroir IPC d'un [`nie_explore::audio::Cue`], nom de fichier proposé compris.
fn cue_dto(path: &str, c: &nie_explore::audio::Cue) -> CueDto {
    CueDto {
        name: c.name.clone(),
        awb_id: c.awb_id,
        length_ms: c.length_ms,
        codec: c.codec.clone(),
        sample_rate: c.sample_rate,
        channels: c.channels,
        size: c.size,
        filename: nie_explore::audio::nom_de_fichier(path, c),
    }
}

/// Décode UNE piste d'une banque, désignée par son **cue-id AFS2** (cf. [`vfs_audio_cues`]), en
/// WAV PCM16 base64.
///
/// Le cue-id n'est pas le rang de l'entrée dans l'AWB : ils coïncident souvent, jamais toujours,
/// et les confondre fait jouer une autre piste sans lever d'erreur. Même thread à pile large que
/// [`vfs_audio_preview_b64`], pour la même raison (`cridecoder` déborde la pile Windows par défaut).
#[tauri::command]
#[specta::specta]
fn vfs_audio_cue_wav_b64(
    path: String,
    awb_id: u16,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<String, String> {
    let awb = with_vfs(game_dir, &state, |vfs| {
        let data = vfs.read(&path).map_err(|e| e.to_string())?;
        nie_explore::audio::resoudre_awb(vfs, &path, &data)
            .map(|(bytes, _)| bytes)
            .ok_or_else(|| format!("{path} : aucune banque AWB atteignable"))
    })?;

    let wav = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || nie_explore::audio::decoder_cue(&awb, awb_id))
        .map_err(|e| format!("échec de lancement du thread de décodage : {e}"))?
        .join()
        .map_err(|_| "le décodage audio a paniqué (thread dédié)".to_string())??;

    const CAP: usize = 40 * 1024 * 1024;
    if wav.len() > CAP {
        return Err(format!(
            "WAV décodé trop volumineux pour l'aperçu ({} octets > {CAP})",
            wav.len()
        ));
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(&wav))
}

/// Cœur du décodage audio CRI (HCA/ADX) → WAV b64, indépendant de la SOURCE des octets (VFS monté
/// OU entrée d'un CPK brut hors VFS, cf. [`raw_cpk_audio_preview_b64`]) — factorisé pour la parité
/// d'outils `RawCpkView`/`DetailPane` (roadmap §6, « pas de Blender/aperçu 3D/audio/vidéo pour les
/// entrées d'un CPK ouvert hors VFS »).
fn audio_wav_b64_from_bytes(data: Vec<u8>) -> Result<String, String> {
    let wav = audio_wav_from_bytes(data)?;

    const CAP: usize = 40 * 1024 * 1024;
    if wav.len() > CAP {
        return Err(format!(
            "WAV décodé trop volumineux pour l'aperçu ({} octets > {CAP})",
            wav.len()
        ));
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(&wav))
}

/// Décodage audio CRI → WAV, **sans** plafond de taille ni base64 : c'est la forme utile à
/// l'EXPORT (écriture disque directe, cf. `export::produire`), là où [`audio_wav_b64_from_bytes`]
/// sert l'aperçu (qui doit, lui, refuser ce qui ne tient pas raisonnablement dans une page).
pub(crate) fn audio_wav_from_bytes(data: Vec<u8>) -> Result<Vec<u8>, String> {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || nie_formats::cri_audio::decode_to_wav(&data))
        .map_err(|e| format!("échec de lancement du thread de décodage : {e}"))?
        .join()
        .map_err(|_| "le décodage audio a paniqué (thread dédié)".to_string())?
}

/// Même décodage audio que [`vfs_audio_preview_b64`], mais depuis une entrée du CPK brut ouvert
/// (hors VFS) — un seul fichier autonome (HCA/ADX ne référence jamais de fichier frère), donc pas
/// de dépendance à l'indexation VFS. (L'aperçu 3D, qui a besoin des frères g4md/g4mg, a sa PROPRE
/// résolution scopée au CPK courant plutôt que le VFS — cf.
/// [`assemble_glb_from_cpk_entries`].)
#[tauri::command]
#[specta::specta]
fn raw_cpk_audio_preview_b64(
    index: u32,
    state: tauri::State<RawCpkState>,
) -> Result<String, String> {
    let data = {
        let guard = state.0.lock().unwrap();
        let (_, data, reader) = guard.as_ref().ok_or("aucun CPK ouvert")?;
        let entry = reader
            .entries
            .get(index as usize)
            .ok_or("index d'entrée invalide")?;
        reader.extract(data, entry).map_err(|e| e.to_string())?
    };
    audio_wav_b64_from_bytes(data)
}

// ─── Aperçu vidéo (USM/Sofdec2 → MP4, remux pur Rust) ─────────────────────────────────
//
// Ni `libvlc` ni `ffmpeg` : le remux vit dans le dépôt (`nie_formats::mp4`), donc l'aperçu
// marche sur une machine nue. Le sous-processus `ffmpeg` qui occupait cette place échouait ici
// (binaire absent du PATH) et coûtait deux écritures disque par requête pour une opération qui
// ne fait que recopier des octets — un remux ne réencode rien.
//
// Ce chemin base64 reste réservé aux APERÇUS courts (plafond 40 Mo). Les films entiers passent
// par le protocole `nievideo://` (cf. `video.rs`), qui gère les requêtes `Range` : c'est lui qui
// rend le déplacement dans la timeline instantané sur une cinématique de 300 Mo.

/// Remuxe le flux vidéo H.264 d'un `.usm` en MP4 lisible par un `<video>` HTML (base64, borné).
/// VP9 brut n'est pas remuxable simplement (pas de conteneur) : renvoie une erreur claire.
#[tauri::command]
#[specta::specta]
fn vfs_video_preview_b64(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<String, String> {
    let data = with_vfs(game_dir, &state, |vfs| {
        vfs.read(&path).map_err(|e| e.to_string())
    })?;
    // Le nom de fichier est la clé de l'enveloppe CRI des conteneurs *loose* : le transmettre
    // est ce qui rend `IE_15th.usm` et `L5logo.usm` lisibles ici aussi.
    let nom = path.rsplit('/').next().unwrap_or(&path).to_string();
    let mp4 = video::mp4_depuis_usm(&data, &nom)?;
    borner_et_encoder(&mp4)
}

/// Encode un MP4 en base64 pour l'IPC, avec le plafond d'aperçu.
fn borner_et_encoder(mp4: &[u8]) -> Result<String, String> {
    const CAP: usize = 40 * 1024 * 1024;
    if mp4.len() > CAP {
        return Err(format!(
            "MP4 remuxé trop volumineux pour l'aperçu ({} octets > {CAP}) — ouvrez-le dans le \
             Cinéma, qui diffuse par `nievideo://` sans plafond",
            mp4.len()
        ));
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(mp4))
}

/// Cœur du remuxage vidéo USM→MP4, indépendant de la SOURCE des octets (VFS monté OU entrée d'un
/// CPK brut hors VFS, cf. [`raw_cpk_video_preview_b64`]) — même factorisation que
/// [`audio_wav_b64_from_bytes`], même raison (parité d'outils `RawCpkView`, roadmap §6).
fn video_mp4_b64_from_bytes(data: Vec<u8>) -> Result<String, String> {
    let mp4 = video_mp4_from_bytes(data)?;
    borner_et_encoder(&mp4)
}

/// Remuxage USM→MP4 **sans** plafond ni base64 — forme utile à l'export, même partage que
/// [`audio_wav_from_bytes`].
pub(crate) fn video_mp4_from_bytes(data: Vec<u8>) -> Result<Vec<u8>, String> {
    // Le nom sert de clé au déchiffrement de l'enveloppe CRI des deux conteneurs *loose*. Ici,
    // la source des octets n'est pas forcément un chemin (entrée de CPK brut) : on passe une
    // chaîne vide, et un fichier chiffré remontera une erreur explicite plutôt qu'un faux MP4.
    video::mp4_depuis_usm(&data, "")
}

/// Répond à une requête `nievideo://`.
///
/// Le corps est produit une fois puis gardé dans [`video::CacheVideo`] : un `<video>` émet une
/// requête `Range` par saut dans la timeline, et sans ce cache chaque saut redémultiplexerait
/// tout le conteneur.
fn servir_video(
    app: &tauri::AppHandle,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{Response, StatusCode};
    use tauri::Manager;

    let echec = |code: StatusCode, message: String| -> Response<Vec<u8>> {
        Response::builder()
            .status(code)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(message.into_bytes())
            .unwrap_or_else(|_| Response::new(Vec::new()))
    };

    // `nievideo://localhost/data/common/movie/x.usm?track=audio` — le chemin VFS est le chemin
    // de l'URI, la piste demandée sa query.
    let uri = request.uri();
    let chemin = percent_decode_uri(uri.path().trim_start_matches('/'));
    if chemin.is_empty() || chemin.contains("..") {
        return echec(StatusCode::BAD_REQUEST, "chemin invalide".to_string());
    }
    let audio = uri.query().is_some_and(|q| q.contains("track=audio"));
    let cle = format!("{chemin}{}", if audio { "?audio" } else { "" });

    // `Range: bytes=<debut>-[fin]` — la seule forme qu'émettent les webviews. La borne haute
    // n'est bornée par la taille qu'APRÈS coup, une fois le total connu.
    let plage_demandee = request
        .headers()
        .get("range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes="))
        .map(|v| {
            let (d, f) = v.split_once('-').unwrap_or((v, ""));
            let debut = d.parse::<u64>().unwrap_or(0);
            let fin = f.parse::<u64>().unwrap_or(u64::MAX);
            (debut, fin.max(debut))
        });

    // La plage est résolue AVANT d'aller chercher les octets : le cache ne rend alors que la
    // tranche demandée, au lieu de recopier tout le film à chaque requête.
    let cache = app.state::<video::CacheVideo>();
    let (type_mime, corps, total) = match cache.tranche(&cle, plage_demandee) {
        Some(trouve) => trouve,
        None => {
            let vfs_state = app.state::<VfsState>();
            let brut = match with_vfs(None, &vfs_state, |vfs| {
                vfs.read(&chemin).map_err(|e| e.to_string())
            }) {
                Ok(b) => b,
                Err(e) => return echec(StatusCode::NOT_FOUND, e),
            };
            let nom = chemin.rsplit('/').next().unwrap_or(&chemin).to_string();
            // Le type MIME vient du CODEC, pas de l'extension demandée : H.264 sort en MP4,
            // VP9 en WebM. Annoncer `video/mp4` sur un WebM ferait échouer le décodage.
            let produit = if audio {
                // La bande-son demande le VFS : 95 films sur 97 n'ont pas la leur dans leur
                // conteneur, elle vit dans `anime_stream` — et son archive de 654 Mo se
                // matérialise dans le cache de l'application.
                let cache_dir = app
                    .path()
                    .app_cache_dir()
                    .unwrap_or_else(|_| std::env::temp_dir().join("nie-explorer"));
                let vfs_state = app.state::<VfsState>();
                with_vfs(None, &vfs_state, |vfs| {
                    video::wav_bande_son(vfs, &cache_dir, &chemin, &brut)
                })
                .map(|o| ("audio/wav", o))
            } else {
                video::flux_web_depuis_usm(&brut, &nom)
            };
            match produit {
                Err(e) => return echec(StatusCode::UNPROCESSABLE_ENTITY, e),
                Ok((mime, octets)) => {
                    let total = octets.len() as u64;
                    let morceau = video::decouper(&octets, plage_demandee);
                    cache.ranger(cle, mime, octets);
                    (mime, morceau, total)
                }
            }
        }
    };

    if total == 0 {
        return echec(StatusCode::UNPROCESSABLE_ENTITY, "flux vide".to_string());
    }
    // Bornes effectivement servies, une fois le total connu.
    let plage = plage_demandee.map(|(debut, _)| {
        let debut = debut.min(total - 1);
        (debut, debut + corps.len() as u64 - 1)
    });

    match plage {
        None => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", type_mime)
            .header("Accept-Ranges", "bytes")
            // Sous Windows, ce protocole est servi depuis `http://nievideo.localhost`, une
            // origine distincte de celle de l'application : sans cet en-tête, un
            // `<video crossorigin>` échoue et un `canvas` qui le dessine devient « teinté »,
            // donc `toDataURL` jette. C'est ce qui rend possibles les vignettes du Cinéma.
            .header("Access-Control-Allow-Origin", "*")
            .header("Content-Length", total.to_string())
            .body(corps)
            .unwrap_or_else(|_| Response::new(Vec::new())),
        Some((debut, fin)) => Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header("Content-Type", type_mime)
            .header("Accept-Ranges", "bytes")
            .header("Access-Control-Allow-Origin", "*")
            .header("Content-Range", format!("bytes {debut}-{fin}/{total}"))
            .header("Content-Length", corps.len().to_string())
            .body(corps)
            .unwrap_or_else(|_| Response::new(Vec::new())),
    }
}

/// Décode le percent-encoding d'un chemin d'URI (`%2F` → `/`, `%C3%A9` → `é`).
fn percent_decode_uri(s: &str) -> String {
    let src = s.as_bytes();
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'%' && i + 2 < src.len() {
            // Un `%XX` mal formé est laissé tel quel plutôt que perdu : mieux vaut un chemin qui
            // ne correspond à rien qu'un chemin silencieusement mutilé.
            if let Some(b) = std::str::from_utf8(&src[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(src[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Catalogue des cinématiques : instantané, sans lire un octet des conteneurs.
///
/// Les champs issus du démultiplexage (durée, définition, codec) restent vides ; le frontend les
/// remplit carte par carte avec [`video_info`], au fil du défilement. Démultiplexer les 97 films
/// d'un coup coûterait plusieurs minutes et bloquerait l'ouverture de la page.
#[tauri::command]
#[specta::specta]
fn video_catalog(
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<video::CatalogueVideoDto, String> {
    with_vfs(game_dir, &state, |vfs| Ok(video::catalogue(vfs)))
}

/// Prépare un film pour la lecture : démuxe, remuxe, et garde le résultat en cache.
///
/// Sans ça, cliquer sur une carte fait attendre le temps du remux — de quelques dixièmes de
/// seconde pour un logo à plusieurs secondes pour une cinématique de 300 Mo. Précharger pendant
/// que le curseur survole la carte rend la lecture instantanée au clic.
///
/// Rend la taille du flux prêt, en octets. Appeler deux fois est sans coût : la seconde fois,
/// l'entrée est déjà dans le cache.
#[tauri::command]
#[specta::specta]
fn video_precharger(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
    cache: tauri::State<video::CacheVideo>,
) -> Result<u32, String> {
    if let Some((_, octets, total)) = cache.tranche(&path, Some((0, 0))) {
        let _ = octets;
        return Ok(total as u32);
    }
    let brut = with_vfs(game_dir, &state, |vfs| {
        vfs.read(&path).map_err(|e| e.to_string())
    })?;
    let nom = path.rsplit('/').next().unwrap_or(&path).to_string();
    let (mime, octets) = video::flux_web_depuis_usm(&brut, &nom)?;
    let total = octets.len() as u32;
    cache.ranger(path, mime, octets);
    Ok(total)
}

/// Métadonnées complètes d'un film : codec, définition, cadence, durée, pistes sonores.
#[tauri::command]
#[specta::specta]
fn video_info(
    path: String,
    game_dir: Option<String>,
    state: tauri::State<VfsState>,
) -> Result<video::FilmDto, String> {
    with_vfs(game_dir, &state, |vfs| video::info_film(vfs, &path))
}

/// Même remuxage vidéo que [`vfs_video_preview_b64`], mais depuis une entrée du CPK brut ouvert
/// (hors VFS) — un `.usm` est autonome (pas de fichier frère référencé), donc pas de dépendance à
/// l'indexation VFS.
#[tauri::command]
#[specta::specta]
fn raw_cpk_video_preview_b64(
    index: u32,
    state: tauri::State<RawCpkState>,
) -> Result<String, String> {
    let data = {
        let guard = state.0.lock().unwrap();
        let (_, data, reader) = guard.as_ref().ok_or("aucun CPK ouvert")?;
        let entry = reader
            .entries
            .get(index as usize)
            .ok_or("index d'entrée invalide")?;
        reader.extract(data, entry).map_err(|e| e.to_string())?
    };
    video_mp4_b64_from_bytes(data)
}

/// JSON libre renvoyé tel quel sur l'IPC (réponses azalee : GraphQL/REST, forme non fixe côté
/// serveur — le frontend les type déjà en `any`/interfaces locales, cf. `src/lib/api.ts`).
///
/// `serde_json::Value` EST récursif (`Object`/`Array` se contiennent eux-mêmes, cf.
/// `impl Type for SerdeValue` dans `specta`) : l'exporter TS dessus fait un vrai
/// `STATUS_STACK_OVERFLOW` — vérifié en réel, y compris sur un thread à pile 64 Mio dédiée (cf.
/// `run()`), donc PAS un simple manque de pile, une récursion qui ne se referme jamais côté
/// réflexion de types. Ce wrapper s'exporte comme `unknown` côté TS (`specta_typescript::define`,
/// un type opaque non récursif) sans changer un seul octet envoyé sur l'IPC : `Serialize`
/// délègue tel quel à `serde_json::Value`.
struct RawJson(serde_json::Value);

impl serde::Serialize for RawJson {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl specta::Type for RawJson {
    fn definition(_: &mut specta::Types) -> specta::datatype::DataType {
        specta::datatype::DataType::Reference(specta_typescript::define("unknown"))
    }
}

// ─── Résolveur distant azalee (GraphQL + REST) ──────────────────────────────────────
//
// Contrat RÉEL confirmé depuis les sources du service (VPS OVH, `~/rg/apps/azalee`, session
// 2026-08-07) — pas une convention devinée :
//   - GraphQL POST `{base}/api/graphql` (graphql-yoga, sans auth) : `app/api/graphql/route.ts`.
//     Requêtes : `characters(q,limit)`/`character(id)`, `skills(q,limit)`/`skill(id)`,
//     `items(q,limit)`/`item(id)`, `auras(q,element,typeSlug!)`.
//   - REST `GET {base}/api/cpk?q=<sous-chaîne>` (index CPK complet, 250 800 fichiers) et
//     `GET {base}/api/cpk?path=<...>&meta=1` (métadonnées + URL CDN) : `app/api/cpk/route.ts`.
//   - REST `POST {base}/api/save/resolve-roster` `{ids: string[]}` → noms résolus du roster
//     d'une save (miroir serveur, aucun ID inventé) : `app/api/save/resolve-roster/route.ts`.
// Testé en direct (`curl`) le 2026-08-07 : les deux endpoints répondent en production.

const AZALEE_DEFAULT_URL: &str = "https://azalee.rosegriffon.fr";

fn azalee_base(base_url: &str) -> &str {
    let b = base_url.trim();
    if b.is_empty() {
        AZALEE_DEFAULT_URL
    } else {
        b.trim_end_matches('/')
    }
}

fn graphql_query(
    base_url: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let url = format!("{}/api/graphql", azalee_base(base_url));
    let body = serde_json::json!({ "query": query, "variables": variables });
    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("requête GraphQL échouée ({url}) : {e}"))?;
    let json: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("réponse non-JSON : {e}"))?;
    if let Some(errors) = json.get("errors") {
        return Err(format!("erreurs GraphQL : {errors}"));
    }
    json.get("data")
        .cloned()
        .ok_or_else(|| "réponse GraphQL sans champ « data »".to_string())
}

/// Recherche de personnages via le GraphQL azalee (`characters(q, limit)`), en bonus du miroir
/// local `nie-wiki` — utile quand aucun `supabase-*.sqlite` local n'est configuré.
#[tauri::command]
#[specta::specta]
fn remote_search_chara(base_url: String, query: String) -> Result<RawJson, String> {
    graphql_query(
        &base_url,
        "query($q: String) { characters(q: $q, limit: 20) { id internalCode name { fr en ja } \
         variants { charaParamId position element rarity image } } }",
        serde_json::json!({ "q": query }),
    )
    .map(RawJson)
}

/// Recherche de techniques via le GraphQL azalee (`skills(q, limit)`).
#[tauri::command]
#[specta::specta]
fn remote_search_waza(base_url: String, query: String) -> Result<RawJson, String> {
    graphql_query(
        &base_url,
        "query($q: String) { skills(q: $q, limit: 20) { id name { fr en ja } category element power tension image } }",
        serde_json::json!({ "q": query }),
    )
    .map(RawJson)
}

/// Recherche plein-texte dans l'index CPK distant (250 800 fichiers, azalee) — utile en
/// complément du VFS local (comparaison, ou navigation sans avoir le jeu monté).
#[tauri::command]
#[specta::specta]
fn remote_cpk_search(base_url: String, query: String) -> Result<RawJson, String> {
    let url = format!("{}/api/cpk?q={}", azalee_base(&base_url), urlencode(&query));
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| format!("requête distante échouée ({url}) : {e}"))?;
    resp.into_json::<serde_json::Value>()
        .map(RawJson)
        .map_err(|e| format!("réponse non-JSON : {e}"))
}

/// Résout les IDs de roster d'une sauvegarde (hash `0x........`) en noms réels via le miroir
/// serveur azalee — AUCUN octet de save ne transite, seulement les IDs déjà extraits en local
/// par `nie-save`. Anti-hallucination côté serveur : un ID absent revient `name: null`.
#[tauri::command]
#[specta::specta]
fn remote_resolve_roster(base_url: String, ids: Vec<String>) -> Result<RawJson, String> {
    let url = format!("{}/api/save/resolve-roster", azalee_base(&base_url));
    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({ "ids": ids }))
        .map_err(|e| format!("requête distante échouée ({url}) : {e}"))?;
    resp.into_json::<serde_json::Value>()
        .map(RawJson)
        .map_err(|e| format!("réponse non-JSON : {e}"))
}

// ─── Pipeline 3D distante (avatar + menus) ─────────────────────────────────────────
//
// Le service de modèles possède l'assemblage des couches de visage, les recettes de corps et le
// renderer de menus. Le desktop transporte ses artefacts finis vers le viewport WebGL, sans en
// créer une seconde implémentation. Les requêtes sortent du thread UI.
const MODEL_SERVICE_DEFAULT_URL: &str = "https://cdn.rosegriffon.fr";

fn model_service_base(base_url: &str) -> Result<&str, String> {
    let base = if base_url.trim().is_empty() {
        MODEL_SERVICE_DEFAULT_URL
    } else {
        base_url.trim()
    };
    if !(base.starts_with("https://") || base.starts_with("http://"))
        || base.contains('?')
        || base.contains('#')
    {
        return Err(
            "l'URL du service de modèles doit être une origine http(s), sans chemin ni paramètre"
                .to_string(),
        );
    }
    Ok(base.trim_end_matches('/'))
}

fn model_service_get(base_url: &str, path: &str) -> Result<Vec<u8>, String> {
    let base = model_service_base(base_url)?;
    let url = format!("{base}/{path}");
    let response = ureq::get(&url)
        .call()
        .map_err(|e| format!("service de modèles injoignable ({url}) : {e}"))?;
    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut bytes)
        .map_err(|e| format!("lecture de {url} : {e}"))?;
    Ok(bytes)
}

/// Charge le catalogue réellement exporté par `niers avatar export` depuis le service de modèles.
#[tauri::command]
#[specta::specta]
async fn model_service_avatar_catalog(base_url: String) -> Result<RawJson, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = model_service_get(&base_url, "avatar/catalog.json")?;
        serde_json::from_slice(&bytes)
            .map(RawJson)
            .map_err(|e| format!("catalogue avatar invalide : {e}"))
    })
    .await
    .map_err(|e| format!("tâche catalogue interrompue : {e}"))?
}

/// Récupère un avatar GLB assemblé par le serveur. La route reste bornée à `/model-avatar/` : le
/// réglage de service ne devient pas un proxy HTTP généraliste.
#[tauri::command]
#[specta::specta]
async fn model_service_avatar_glb_b64(
    base_url: String,
    model_path: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = model_path.trim_start_matches('/');
        if !path.starts_with("model-avatar/")
            || path.len() > 8_192
            || path.contains('#')
            || path.contains("..")
        {
            return Err("route d'avatar invalide".to_string());
        }
        let bytes = model_service_get(&base_url, path)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
    })
    .await
    .map_err(|e| format!("tâche avatar interrompue : {e}"))?
}

/// Rend un écran de menu depuis son layout réel (sprites + positions du jeu), en PNG.
#[tauri::command]
#[specta::specta]
async fn model_service_menu_png_b64(base_url: String, screen: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let screen = screen.trim();
        if screen.is_empty()
            || screen.len() > 64
            || !screen
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err("nom d'écran invalide".to_string());
        }
        let bytes = model_service_get(&base_url, &format!("menu-render/{screen}.png"))?;
        Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
    })
    .await
    .map_err(|e| format!("tâche de rendu de menu interrompue : {e}"))?
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}


// --- Aphrody : le pet, et la chaine pixel-perfect -------------------------------------------
//
// Toutes ces commandes sont `async` : elles lisent le disque, et une commande Tauri SYNCHRONE
// tourne sur le THREAD PRINCIPAL — une lecture lente y fige la fenetre (cf. `CLAUDE.md`).

/// Etat du pet Aphrody embarque : atlas, grille, animations, diagnostic d'integrite.
#[tauri::command]
#[specta::specta]
async fn aphrody_pet_etat() -> Result<aphrody::PetEtatDto, String> {
    aphrody::pet_etat()
}

/// Une frame d'animation du pet, extraite sans reechantillonnage, en PNG base64.
#[tauri::command]
#[specta::specta]
async fn aphrody_pet_frame_png_b64(animation: String, index: u32) -> Result<String, String> {
    aphrody::pet_frame_png_b64(&animation, index)
}

/// Mesure une image du disque : boite, ratio, remplissage, palette, epaisseur de trait, pente
/// des bords. `angles_exploitables` dit si les angles rendus veulent dire quelque chose.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
async fn aphrody_pixel_mesurer(
    chemin: String,
    k: Option<u32>,
    boite: Option<Vec<u32>>,
    mode: String,
    seuil: Option<u32>,
    teinte_min: Option<f64>,
    teinte_max: Option<f64>,
    saturation: Option<f64>,
) -> Result<aphrody::MesureDto, String> {
    aphrody::mesurer_fichier(&chemin, k, boite, &mode, seuil, teinte_min, teinte_max, saturation)
}

/// La palette d'une image en proprietes personnalisees CSS `oklch()`, HEX mesure en commentaire.
#[tauri::command]
#[specta::specta]
async fn aphrody_pixel_tokens_css(
    chemin: String,
    prefixe: String,
    k: Option<u32>,
) -> Result<String, String> {
    aphrody::tokens_css_fichier(&chemin, &prefixe, k)
}

/// Compare deux images : SSIM et part des pixels dans la tolerance. Ce ne sont pas le meme
/// critere — le premier juge une reproduction, le second un rendu qui doit etre identique.
#[tauri::command]
#[specta::specta]
async fn aphrody_pixel_comparer(
    a: String,
    b: String,
    tolerance: Option<u32>,
) -> Result<aphrody::ComparaisonDto, String> {
    aphrody::comparer_fichiers(&a, &b, tolerance)
}

/// Vectorise une image en SVG. C'est un DECALQUE : bon pour un logo plat, jamais pour pretendre
/// produire un dessin concu comme vectoriel.
#[tauri::command]
#[specta::specta]
async fn aphrody_pixel_vectoriser(
    chemin: String,
    k: Option<u32>,
    tolerance: Option<f64>,
    mode: String,
    seuil: Option<u32>,
) -> Result<String, String> {
    aphrody::vectoriser_fichier(&chemin, k, tolerance, &mode, seuil)
}

/// Assemble des images en planche de sprites et rend PNG + CSS + SVG + JSON — le meme rendu que
/// pour un atlas du jeu, via `nie_formats::sprite_sheet`.
#[tauri::command]
#[specta::specta]
async fn aphrody_pixel_planche(
    chemins: Vec<String>,
    colonnes: Option<u32>,
    nom: String,
) -> Result<aphrody::PlancheDto, String> {
    aphrody::planche_fichiers(&chemins, colonnes, &nom)
}

/// Collecte toutes les commandes IPC pour `tauri-specta` — une SEULE liste, source de vérité à
/// la fois pour l'enregistrement runtime (`invoke_handler`) et pour l'export des bindings
/// TypeScript (`src/lib/bindings.ts`), là où il fallait avant maintenir `tauri::generate_handler!`
/// ICI et le miroir `invoke<T>("cmd", {...})` de `api.ts` À LA MAIN, sans qu'un oubli ne soit
/// jamais signalé par le compilateur.
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        aphrody_pet_etat,
        aphrody_pet_frame_png_b64,
        aphrody_pixel_mesurer,
        aphrody_pixel_tokens_css,
        aphrody_pixel_comparer,
        aphrody_pixel_vectoriser,
        aphrody_pixel_planche,
        default_game_dir,
        check_game_dir,
        default_wiki_db,
        default_re_db,
        default_anime_db,
        preload_vfs,
        vfs_ls,
        vfs_find,
        vfs_find_paged,
        vfs_stats,
        vfs_entry_meta,
        vfs_describe,
        vfs_read_b64,
        vfs_texture_png_b64,
        vfs_texture_thumb_png_b64,
        vfs_texture_list,
        vfs_texture_named_png_b64,
        vfs_texture_named_thumb_png_b64,
        vfs_extract_to,
        vfs_export_formats,
        vfs_export_default_name,
        vfs_export_as,
        vfs_export_many,
        vfs_write_b64,
        vfs_write_loose_override_b64,
        save_bytes_b64,
        vfs_related,
        vfs_all_entries,
        vfs_index_scan_start,
        vfs_index_scan_cancel,
        vfs_index_scan_take,
        game_data_skills,
        game_data_items,
        game_data_auras,
        game_data_trophies,
        game_data_quests,
        game_data_shops,
        game_data_stadiums,
        game_data_passives,
        game_data_special_tactics,
        game_data_emblems,
        game_data_gallery,
        game_data_tricks,
        game_data_activities,
        game_data_belong_teams,
        game_data_formations,
        game_data_uniforms,
        game_data_chara_picker,
        game_data_charas,
        game_data_opponent_teams,
        game_data_movies,
        game_data_musics,
        game_data_dictionary,
        game_data_exp_table,
        game_data_drops,
        game_data_capsule_rates,
        game_data_noms,
        game_data_calculate_stats,
        vfs_decode_cfgbin,
        vfs_decode_cfgbin_typed,
        vfs_apercu_camera,
        vfs_apercu_navmesh,
        vfs_cache_stats,
        vfs_cache_vider,
        encode_cfgbin_config,
        list_packs_dir,
        open_raw_cpk,
        raw_cpk_describe,
        raw_cpk_read_b64,
        raw_cpk_extract_to,
        raw_cpk_extract_all,
        raw_cpk_audio_preview_b64,
        raw_cpk_video_preview_b64,
        copy_disk_file_to_appdata,
        disk_file_exists,
        stage_texture_replacement,
        export_mod_as_cpk,
        set_titlebar_theme,
        take_pending_open,
        describe_disk_file,
        read_disk_file_b64,
        open_in_blender,
        install_niers_blender_addon,
        blender_preview_png_b64,
        blender_open_scene,
        blender_build_skill_scene,
        remote_search_chara,
        remote_search_waza,
        remote_cpk_search,
        remote_resolve_roster,
        model_service_avatar_catalog,
        model_service_avatar_glb_b64,
        model_service_menu_png_b64,
        default_save_path,
        save_open,
        save_list_blobs,
        save_blob_hex_b64,
        save_export,
        write_text_file,
        vfs_video_preview_b64,
        video_catalog,
        video_info,
        video_precharger,
        open_in_scene_editor,
        lua_chunk_info,
        lua_disassemble,
        lua_execute,
        lua_globals,
        lua_eval,
        lua_list_scripts,
        lua_session_exec,
        lua_session_attach,
        lua_session_broadcast,
        lua_session_eval,
        lua_session_set_global,
        lua_session_globals,
        lua_session_reload,
        lua_session_drain,
        lua_session_api_report,
        vfs_glb_bytes_b64,
        vfs_motion_clips,
        raw_cpk_glb_bytes_b64,
        vfs_audio_preview_b64,
        vfs_audio_cues,
        vfs_audio_cue_wav_b64,
        clipboard_write_file_list,
        clipboard_read_file_list,
        trash_appdata_files,
        forge::forge_report,
        forge::forge_blockers,
        re_trace_find_process,
        re_trace_module_regions,
        re_trace_read_bytes_b64,
        re_trace_write_bytes_b64,
        re_trace_dump_module,
        re_dump_open,
        re_dump_scan,
        mcp::mcp_status,
        mcp::mcp_install,
        viola::viola_dump_start,
        viola::viola_cancel,
        viola::viola_pack,
        viola::viola_merge,
        viola::viola_crypto,
        live_mod::live_status,
        live_mod::live_find_team,
        live_mod::live_read_team,
        live_mod::live_write_member,
        live_mod::live_scan_u32,
        live_mod::live_write_u32,
        live_mod::launch_save_editor,
    ])
}

/// Réécrit `src/lib/bindings.ts` depuis les signatures Rust, sans lancer l'application.
///
/// Même export que celui fait au démarrage en dev, mais utilisable seul (`cargo run --bin
/// export-bindings`) : quand on ajoute une commande, le frontend doit pouvoir la typer sans
/// avoir à ouvrir une fenêtre Tauri, ni recopier sa signature à la main dans `api.ts`.
///
/// Le thread à pile large (64 Mio) est indispensable ici pour la même raison qu'en dev : la
/// réflexion de types de `specta` déborde la pile par défaut sur ce jeu de commandes.
#[cfg(debug_assertions)]
pub fn export_bindings() -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            specta_builder()
                .export(
                    specta_typescript::Typescript::default(),
                    "../src/lib/bindings.ts",
                )
                .map_err(|e| e.to_string())
        })
        .map_err(|e| format!("échec de lancement du thread d'export specta : {e}"))?
        .join()
        .map_err(|_| "le thread d'export specta a paniqué".to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Régénère `src/lib/bindings.ts` à CHAQUE lancement en dev — jamais en release (pas de
    // dépendance à `specta-typescript`/écriture disque dans le binaire distribué). Le frontend
    // importe ce fichier généré directement (cf. `src/lib/api.ts`), donc toute commande
    // ajoutée/modifiée ici se reflète côté TS au prochain `cargo tauri dev`, sans étape manuelle.
    //
    // Lancé sur un THREAD DÉDIÉ à pile large (64 Mio) : trouvé par test réel (pas supposé) — la
    // réflexion de types de `specta` sur ~29 commandes (dont plusieurs `serde_json::Value`,
    // récursif : `Object`/`Array` se référencent eux-mêmes) fait un vrai `STATUS_STACK_OVERFLOW`
    // sur la pile principale par défaut (thread `main`, crash silencieux avant même la création
    // de la fenêtre). Même remède que [`vfs_audio_preview_b64`] pour `cridecoder` : une pile
    // dédiée plus large suffit largement, ce n'est pas une récursion infinie (le process ne
    // boucle pas indéfiniment, il complète normalement une fois la pile élargie).
    #[cfg(debug_assertions)]
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            specta_builder()
                .export(
                    specta_typescript::Typescript::default(),
                    "../src/lib/bindings.ts",
                )
                .expect("échec de l'export des bindings TypeScript (tauri-specta)");
        })
        .expect("échec de lancement du thread d'export specta")
        .join()
        .expect("le thread d'export specta a paniqué");

    let specta = specta_builder();

    // Système de jobs (nie-tasks) — un seul par appli, cf. `VfsScanState`. Le lecteur de
    // progression est branché plus bas dans `.setup()` (a besoin de `app.handle()` pour émettre).
    let (task_system, mut task_progress_rx) = nie_tasks::TaskSystem::<String>::new();

    tauri::Builder::default()
        // DOIT être le premier plugin enregistré (contrat tauri-plugin-single-instance) :
        // relance = focus la fenêtre existante + transmet argv (« Ouvrir avec » Explorer).
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            use tauri::Manager;
            if let Some(path) = first_path_arg(argv) {
                let _ = app.emit("open-path", &path);
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        // Journal PERSISTANT — enregistré juste après `single-instance` (qui doit rester premier)
        // et AVANT tous les autres, pour que leur initialisation soit déjà tracée. Deux cibles :
        // `LogDir` → `%APPDATA%\dev.niers.explorer\logs\nie-explorer.log` (le fichier à demander
        // à une utilisatrice qui signale une anomalie : en release Windows aucune console n'est
        // attachée, `eprintln!` n'écrivait donc nulle part), et `Stdout` pour `tauri dev`.
        // `Webview` va dans l'autre sens : il pousse les logs RUST vers la console du webview
        // (événement `log://log`), ce qui n'a d'effet que si le front appelle `attachConsole()`
        // de `@tauri-apps/plugin-log`. Le trajet inverse (les `console.*` du front vers CE
        // fichier) n'est PAS automatique : il exige que le front logue via les fonctions du
        // paquet JS (`info`/`warn`/`error`), qui passent par la commande `log:allow-log`.
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("nie-explorer".into()),
                    },
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Webview,
                ))
                // Un seul fichier qui bascule à 8 Mo : un journal non borné grossit sans fin sur
                // une machine utilisatrice, et un `KeepAll` accumule les rotations.
                .max_file_size(8 * 1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                // `Info` en release (les `debug!` d'un webview sont très bavards), `Debug` en dev.
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .build(),
        )
        // `niers://…` — ouvrir un titre depuis le wiki azalée. DOIT être enregistré APRÈS
        // `single-instance` sur Windows/Linux : le système relance l'exe avec l'URL en argv, et
        // c'est `single-instance` qui la fait remonter à l'instance vivante. L'inverse ouvre une
        // seconde fenêtre. L'enregistrement du schéma auprès de Windows se fait à
        // l'INSTALLATION (MSI/NSIS, cf. `plugins.deep-link` de `tauri.conf.json`) : en
        // `tauri dev` rien n'est associé tant que `register_all()` (dans `.setup()`) n'a pas
        // écrit la clé HKCU.
        .plugin(tauri_plugin_deep_link::init())
        // Version d'OS / architecture — diagnostic (panneau Paramètres, rapport d'anomalie).
        .plugin(tauri_plugin_os::init())
        // Updater : vérifie/télécharge/installe les nouvelles versions depuis les endpoints
        // `plugins.updater.endpoints` de `tauri.conf.json` (azalee + fallback GitHub releases).
        // `tauri-plugin-process` (relaunch après install) doit être présent côté capacités
        // (`process:allow-restart`) — câblé côté JS par `@tauri-apps/plugin-updater`/`-process`.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:mods.db", mods_migrations())
                .build(),
        )
        // `nievideo://localhost/<chemin VFS>` — la piste vidéo d'un `.usm`, remuxée en MP4 et
        // servie avec le support des requêtes `Range`. C'est ce qui permet à un `<video>` de
        // démarrer et de se déplacer instantanément dans une cinématique de 300 Mo, là où le
        // chemin base64 (`vfs_video_preview_b64`) exige de tout charger et plafonne à 40 Mo.
        // `?track=audio` rend la bande-son décodée en WAV : le HCA Criware n'entre dans aucun
        // conteneur MP4, le lecteur resynchronise les deux flux.
        .register_uri_scheme_protocol("nievideo", |ctx, request| {
            servir_video(ctx.app_handle(), request)
        })
        .manage(video::CacheVideo::default())
        .manage(PendingOpen(Mutex::new(first_path_arg(std::env::args()))))
        .manage(SaveState(Mutex::new(None)))
        .manage(VfsState(RwLock::new(None)))
        .manage(StatsCache(Mutex::new(None)))
        .manage(RawCpkState(Mutex::new(None)))
        // Session Lua PERSISTANTE (thread dédié, cf. `lua_session.rs`) — équivalent du
        // `ScriptInterpreter` d'Overload : la VM vit tant que l'app vit, l'état survit d'une
        // évaluation à l'autre, le rechargement est explicite.
        .manage(lua_session::LuaSessionHandle::start(true))
        .manage(viola::ViolaState::default())
        .manage(VfsScanState {
            system: task_system,
            results: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
        .setup(move |app| {
            // Relaie chaque `TaskProgress` (nie-tasks) en événement Tauri `vfs-index-progress` —
            // générique à TOUT job dispatché sur `task_system`, pas seulement le scan VFS
            // (cf. `VfsIndexProgressDto` : nom du champ neutre `task_id`, pas `vfs_scan_id`).
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    while let Some(p) = task_progress_rx.recv().await {
                        let _ = app_handle.emit(
                            "vfs-index-progress",
                            VfsIndexProgressDto {
                                task_id: p.id.to_string(),
                                done: p.done as u32,
                                total: p.total as u32,
                            },
                        );
                    }
                });
            }
            // `niers://…` — deux temps, tous deux nécessaires :
            //  1. `register_all()` écrit l'association du schéma dans HKCU pour l'exe COURANT.
            //     C'est indispensable en `tauri dev` (aucun installeur n'est passé) et inoffensif
            //     sur une install (elle réécrit la même valeur). Best-effort : un poste où la
            //     clé est verrouillée par une stratégie ne doit pas empêcher l'app de démarrer.
            //  2. `on_open_url` relaie chaque URL reçue en événement Tauri `deep-link` — même
            //     forme que l'événement `open-path` déjà utilisé pour « Ouvrir avec ». La charge
            //     utile est la liste des URLs telles quelles (`niers://titre/<id>`), c'est au
            //     front de les router.
            {
                use tauri_plugin_deep_link::DeepLinkExt as _;
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                if let Err(e) = app.deep_link().register_all() {
                    log::warn!("schéma niers:// non enregistré : {e}");
                }
                let app_handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    let urls: Vec<String> = event.urls().iter().map(|u| u.to_string()).collect();
                    log::info!("deep-link reçu : {urls:?}");
                    let _ = app_handle.emit("deep-link", urls);
                });
            }
            // Habillage natif Windows 11 (Mica) — cf. demande utilisateur « ui windows native ».
            // Best-effort : une build hors Win11/serveur peut échouer l'appel, sans bloquer le
            // lancement (fenêtre reste opaque « surface » standard dans ce cas).
            //
            // `Some(true)` FORCE le mode sombre du chrome natif (texte/boutons de légende de la
            // vraie barre de titre) — `None` (essayé d'abord, cf. capture d'écran réelle) suit le
            // thème CLAIR du système au lieu du thème sombre par défaut de l'appli
            // (`defaultTheme="dark"`, `main.tsx`), ce qui donnait une barre de titre native
            // blanche au-dessus d'un contenu sombre. Valeur initiale sombre (cohérente avec le
            // défaut de l'appli) ; resynchronisée en direct au changement clair/sombre
            // (Paramètres) par [`set_titlebar_theme`], appelée depuis `App.tsx` sur
            // `resolvedTheme` (next-themes).
            #[cfg(target_os = "windows")]
            {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    // `window_vibrancy::apply_mica` N'EST PLUS APPELÉ ICI. Mica s'obtient en
                    // étendant la frame DWM dans la zone client (`DwmExtendFrameIntoClientArea`) ;
                    // sur une fenêtre `decorations: false`, cela REDONNE à Windows une frame à
                    // dessiner — bordure, légende et boutons système réapparaissent par-dessus le
                    // chrome custom, ce qui annulait tout le frameless (symptôme rapporté :
                    // « il y a toujours la bordure Win32 et les boutons close »). Le fond est
                    // peint par l'app (`body { background: var(--color-app) }`, tokens spaceui).
                    //
                    // L'arrondi, lui, reste nécessaire : Windows 11 n'arrondit d'office que les
                    // fenêtres à légende native (`WS_CAPTION`) — une `WS_POPUP` custom resterait
                    // à coins vifs sans cet appel DWM explicite.
                    apply_rounded_corners(&window);
                }
            }

            // Installe les bases livrées avec l'application (miroir wiki + base RE) sur un thread
            // dédié : ~140 Mo décompressés au tout premier lancement, à ne pas faire attendre à la
            // fenêtre. `bases-pretes` prévient le frontend, qui relance alors sa résolution du
            // miroir (`api.defaultWikiDb`) — sans cet événement, la première session afficherait
            // des codes internes bruts (`c01000010`) là où le miroir donne « Mark Evans », et il
            // faudrait relancer l'appli pour que les noms apparaissent.
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let installees = installer_bases_embarquees(&handle);
                    if !installees.is_empty() {
                        let _ = handle.emit(
                            "bases-pretes",
                            installees
                                .iter()
                                .map(|p| p.display().to_string())
                                .collect::<Vec<_>>(),
                        );
                    }
                });
            }

            // Précharge le VFS sur un thread dédié pendant que la fenêtre s'affiche — le premier
            // clic de navigation du frontend (qui appelle aussi `preload_vfs` explicitement au
            // montage, cf. `App.tsx`) retrouve alors un cache déjà chaud dans la plupart des cas,
            // au lieu d'attendre l'indexation complète (~255 800 entrées) en plein milieu d'un
            // clic. Best-effort : une erreur ici (jeu non détecté) est silencieuse, l'appel
            // explicite du frontend au montage remontera la vraie erreur à l'UI.
            {
                use tauri::Manager;
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let state = handle.state::<VfsState>();
                    let _ = with_vfs(None, &state, |_vfs| Ok(()));
                });
            }
            Ok(())
        })
        .invoke_handler(specta.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Golden réel sur le vrai jeu (`data/`, 57 Go, gitignored) : vérifie que le GLB livré au
/// viewport WebGL est réellement assemblable depuis le VFS ou un CPK brut.
/// `cargo test -p nie-explorer --lib --features real-fixtures`.
#[cfg(all(test, feature = "real-fixtures"))]
mod real_fixtures_tests {
    use super::{
        assemble_glb_for_preview, assemble_glb_from_cpk_entries, ensure_niers_blender_addon,
        CpkReader, Vfs,
    };

    /// `plugins/niers-blender` est vendorisé dans niers : il doit être détecté PRÉSENT sans
    /// déclencher de `git clone` (rapide, déterministe — pas de dépendance réseau dans ce test).
    /// Le chemin réseau (`git clone` si absent, filet de sécurité pour un `game_dir` qui n'est pas
    /// un checkout de ce dépôt) se vérifie manuellement contre le vrai dépôt GitHub, pas ici
    /// (pas d'accès réseau garanti en CI).
    #[test]
    fn ensure_niers_blender_addon_detecte_le_vendoring_deja_present() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let addon_dir = ensure_niers_blender_addon(&root)
            .expect("plugins/niers-blender doit déjà être présent (vendorisé)");
        assert!(
            addon_dir.ends_with("niers-blender"),
            "chemin inattendu : {}",
            addon_dir.display()
        );
        assert!(addon_dir.join("__init__.py").is_file());
        // La VRAIE ligne attendue de l'addon (pas un fichier vide/corrompu) : le bl_info du plugin.
        let init = std::fs::read_to_string(addon_dir.join("__init__.py")).unwrap();
        assert!(
            init.contains("Level-5 G4 Blender Tools"),
            "contenu inattendu — mauvais addon ?"
        );
    }

    #[test]
    fn glb_du_viewport_sur_un_vrai_modele() {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data");
        assert!(
            data_dir.is_dir(),
            "data/ introuvable ({}) — test réservé au poste avec le vrai jeu",
            data_dir.display()
        );
        let mut vfs = Vfs::new();
        vfs.init(&data_dir).expect("init VFS depuis le vrai data/");

        // `c01000010` = visage IE1 d'Endou (même fixture que `nie_formats::assemble::tests`,
        // casse réelle du VFS vérifiée via `niers vfs find c01000010` : `01_IE1`, pas `01_ie1`).
        let path = "data/common/chr/_face/01_IE1/c01000010/c01000010.g4md";
        let (stem, glb) = assemble_glb_for_preview(&vfs, path).expect("assemblage GLB réel");
        assert_eq!(stem, "c01000010");
        assert!(
            glb.len() > 1000,
            "GLB assemblé suspicieusement petit ({} octets)",
            glb.len()
        );

        assert!(glb.starts_with(b"glTF"), "signature GLB absente");
    }

    /// Même vérification, mais pour le chemin **CPK brut hors VFS** : ouvre un vrai `.cpk` de
    /// `data/packs/`, résout les frères g4mg/g4tx par (dossier, basename), puis assemble le GLB
    /// du viewport commun.
    #[test]
    fn raw_cpk_glb_du_viewport_sur_un_vrai_pack() {
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data/packs/eaabb0359e96871a72ea9f86c5d3d10d.cpk");
        assert!(
            pack.is_file(),
            "pack introuvable ({}) — test réservé au poste avec le vrai jeu",
            pack.display()
        );
        let data = std::fs::read(&pack).expect("lecture du CPK réel");
        let reader = CpkReader::new(&data, "eaabb0359e96871a72ea9f86c5d3d10d.cpk")
            .expect("parsing du CPK réel");

        let entry = reader
            .entries
            .iter()
            .find(|e| e.filename.eq_ignore_ascii_case("c01000010.g4md"))
            .expect(
                "c01000010.g4md doit être dans ce pack (même fixture que le test VFS ci-dessus)",
            );

        let (stem, glb) = assemble_glb_from_cpk_entries(&data, &reader, entry)
            .expect("assemblage GLB depuis le CPK brut");
        assert_eq!(stem, "c01000010");

        assert!(glb.starts_with(b"glTF"), "signature GLB absente");
    }
}
