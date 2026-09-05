//! Lecture mémoire **live** du process `nie.exe`/`nie_eacpatched.exe` — façade IPC au-dessus de
//! `nie-trace` (RE en direct, cf. `crates/forge/nie-trace/src/lib.rs`). Décision utilisatrice tranchée
//! (câblage explicitement demandé, cf. `apps/nie-explorer/ROADMAP.md` §4.3/§5) : RE single-player
//! offline d'un jeu possédé, cadrée par l'accord `RG-L5-VR-2026-001` (cf. `CLAUDE.md`).
//!
//! Surface **lecture et écriture** : [`read_exact`](nie_trace::read_exact) /
//! [`write_exact`](nie_trace::write_exact) / [`find_pid_by_name`](nie_trace::find_pid_by_name) /
//! [`module_regions`](nie_trace::module_regions) / [`dump_regions`](nie_trace::dump_regions).
//!
//! L'écriture a été ouverte sur demande explicite de l'utilisateur (« rend nie-trace read and
//! write ») ; elle était auparavant bridée en lecture seule par prudence. [`nie_trace::patch_eac`]
//! reste hors de cette façade : il opère sur une COPIE fichier hors ligne, pas sur un process.
//! Toute écriture est **relue** et c'est la relecture qui est rendue — voir
//! [`re_trace_write_bytes_b64`].
//!
//! Ce module porte aussi le volet **hors ligne** : `re_dump_*` scanne un minidump `.dmp` DÉJÀ
//! capturé (via [`nie_dump`]) — un simple fichier lu en lecture seule, sans la moindre attache à
//! un process vivant, donc hors du champ d'EAC.

use base64::Engine as _;
use serde::Serialize;

/// Noms de process essayés dans l'ordre — le binaire patché EAC (lancé directement, cf.
/// `nie_trace::patch_eac`) d'abord, repli sur le nom d'origine (lancé via `EACLauncher.exe`, lecture
/// alors susceptible d'échouer — driver EAC kernel actif, cf. `win_memory.rs:48`).
const CANDIDATE_PROCESS_NAMES: [&str; 2] = ["nie_eacpatched.exe", "nie.exe"];

#[derive(Serialize, specta::Type)]
pub struct ReTraceProcessDto {
    pid: i32,
    process_name: String,
    /// Base du module principal en hexadécimal (`0x…`), `None` si non résolue (process trouvé
    /// mais `find_module_base` échoue — permissions insuffisantes p. ex.).
    module_base: Option<String>,
}

/// Cherche le process `nie.exe`/`nie_eacpatched.exe` en cours d'exécution. `None` si le jeu n'est
/// pas lancé — jamais d'attache silencieuse ni de retry en boucle.
#[tauri::command]
#[specta::specta]
pub fn re_trace_find_process() -> Option<ReTraceProcessDto> {
    for name in CANDIDATE_PROCESS_NAMES {
        if let Some(pid) = nie_trace::find_pid_by_name(name) {
            let module_base = nie_trace::find_module_base(pid, "nie").map(|b| format!("0x{b:x}"));
            return Some(ReTraceProcessDto { pid, process_name: name.to_string(), module_base });
        }
    }
    None
}

#[derive(Serialize, specta::Type)]
pub struct ReTraceRegionDto {
    start: String,
    end: String,
    /// Taille de la plage en octets. `f64` et **pas** `u64` : `specta` refuse d'exporter les types
    /// « BigInt » (`u64`/`usize`/`i64`/…) vers TypeScript pour éviter une perte de précision
    /// silencieuse — et le refus est FATAL (l'export panique au démarrage en debug, ce qui
    /// empêchait l'app de se lancer du tout). Une plage d'un module Windows x64 est bornée par
    /// l'espace d'adressage utilisateur (2⁴⁷), très en dessous des 2⁵³ entiers exactement
    /// représentables en `f64` : la conversion est donc SANS perte ici, contrairement à un
    /// `as u32` qui tronquerait pour de vrai.
    size: f64,
    perms: String,
    path: String,
}

fn to_region_dto(m: &nie_trace::MapEntry) -> ReTraceRegionDto {
    ReTraceRegionDto {
        start: format!("0x{:x}", m.start),
        end: format!("0x{:x}", m.end),
        size: m.size() as f64,
        perms: m.perms.clone(),
        path: m.path.clone(),
    }
}

/// Liste les plages mémoire du **module principal** (`nie`/`nie_eacpatched`) du process `pid` —
/// jamais tout l'espace d'adressage (autres DLL, tas non pertinent) : `module_regions(.., false)`
/// filtre déjà sur le module.
#[tauri::command]
#[specta::specta]
pub fn re_trace_module_regions(pid: i32) -> Result<Vec<ReTraceRegionDto>, String> {
    let regions = nie_trace::module_regions(pid, "nie", false);
    if regions.is_empty() {
        return Err("aucune plage lisible trouvée pour ce pid — process introuvable, permissions insuffisantes, ou EAC actif (relancer nie_eacpatched.exe directement)".into());
    }
    Ok(regions.iter().map(to_region_dto).collect())
}

/// Lit `len` octets à `addr` (hex `0x…` ou décimal) dans `pid`, encodés base64 — jamais plus de
/// 1 Mio par appel (évite un `Vec` géant sur une fausse manip côté UI).
#[tauri::command]
#[specta::specta]
pub fn re_trace_read_bytes_b64(pid: i32, addr: String, len: u32) -> Result<String, String> {
    const MAX_LEN: u32 = 1024 * 1024;
    if len == 0 || len > MAX_LEN {
        return Err(format!("longueur invalide (1..={MAX_LEN})"));
    }
    let addr = parse_addr(&addr)?;
    let bytes = nie_trace::read_exact(pid, addr, len as usize).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Écrit des octets (base64) à `addr` dans `pid`, puis **relit** la zone et la rend, elle aussi
/// en base64.
///
/// Complète [`re_trace_read_bytes_b64`] : `nie-trace` porte `write_exact` depuis toujours, seule
/// cette façade ne l'exposait pas. Ce qui est rendu est ce que la mémoire contient **après**
/// l'écriture, jamais ce que l'appelant croyait y mettre — une page protégée en lecture seule ou
/// une écriture partielle se voit alors immédiatement côté UI.
///
/// Même plafond que la lecture : 1 Mio par appel.
#[tauri::command]
#[specta::specta]
pub fn re_trace_write_bytes_b64(pid: i32, addr: String, data_b64: String) -> Result<String, String> {
    const MAX_LEN: usize = 1024 * 1024;
    let octets = base64::engine::general_purpose::STANDARD
        .decode(data_b64.as_bytes())
        .map_err(|e| format!("base64 invalide : {e}"))?;
    if octets.is_empty() || octets.len() > MAX_LEN {
        return Err(format!("longueur invalide (1..={MAX_LEN})"));
    }
    let addr = parse_addr(&addr)?;
    nie_trace::write_exact(pid, addr, &octets).map_err(|e| e.to_string())?;
    let relu = nie_trace::read_exact(pid, addr, octets.len()).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(relu))
}

fn parse_addr(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let (digits, radix) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).map_or((s, 10), |d| (d, 16));
    u64::from_str_radix(digits, radix).map_err(|e| format!("adresse invalide {s:?} : {e}"))
}

#[derive(Serialize, specta::Type)]
pub struct ReTraceDumpStatsDto {
    regions: u32,
    /// Octets écrits. `f64` pour la même raison que [`ReTraceRegionDto::size`] — total borné par
    /// la taille du module dumpé, donc exactement représentable.
    bytes: f64,
    /// Dossier de sortie réel (sous `BaseDirectory::AppData`, jamais dans le dossier du jeu —
    /// même convention que le workspace de mods, cf. `lib.rs`).
    out_dir: String,
}

/// Dumpe les plages lisibles du module principal vers `AppData/re-dumps/<pid>-<horodatage>/` —
/// jamais dans le dossier du jeu. Réutilise [`nie_trace::dump_regions`] tel quel (lecture seule,
/// une plage volatile/refusée est simplement sautée).
#[tauri::command]
#[specta::specta]
pub fn re_trace_dump_module(pid: i32, app: tauri::AppHandle) -> Result<ReTraceDumpStatsDto, String> {
    use tauri::Manager;
    let regions = nie_trace::module_regions(pid, "nie", false);
    if regions.is_empty() {
        return Err("aucune plage lisible trouvée pour ce pid".into());
    }
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let out_dir = base.join("re-dumps").join(format!("{pid}-{stamp}"));
    let stats = nie_trace::dump_regions(pid, &regions, &out_dir).map_err(|e| e.to_string())?;
    Ok(ReTraceDumpStatsDto {
        regions: stats.regions as u32,
        bytes: stats.bytes as f64,
        out_dir: out_dir.display().to_string(),
    })
}

// ── Scan AOB hors ligne sur minidump `.dmp` (nie-dump) ─────────────────────────────────────────
//
// `nie_dump::Minidump` détient un `std::fs::File` et `scan` prend `&mut self` : plutôt que de
// garder un handle dans un `State` (verrou partagé, fichier bloqué tant que l'app tourne), chaque
// commande rouvre le dump. Le coût du `open` est le parsing des en-têtes (quelques Ko), négligeable
// devant le scan lui-même.

/// Nombre de coups renvoyés quand l'appelant ne borne pas (`limite = 0`).
const DUMP_SCAN_LIMITE_DEFAUT: u32 = 200;
/// Plafond dur : au-delà, la liste ne se lit plus et la sérialisation IPC coûte plus que le scan.
const DUMP_SCAN_LIMITE_MAX: u32 = 5000;

#[derive(Serialize, specta::Type)]
pub struct ReDumpModuleDto {
    name: String,
    /// Base virtuelle du module au moment de la capture (ASLR), en hexadécimal `0x…`.
    base: String,
    /// Taille de l'image en mémoire, en octets.
    size: f64,
}

#[derive(Serialize, specta::Type)]
pub struct ReDumpInfoDto {
    modules: Vec<ReDumpModuleDto>,
    /// Nombre de plages mémoire capturées.
    ranges: u32,
    /// Total d'octets mémoire capturés — c'est ce volume que chaque scan relit depuis le disque.
    mapped_bytes: f64,
    /// Base virtuelle de `nie.exe` dans la capture, si le module y figure.
    nie_base: Option<String>,
}

/// Ouvre un minidump `.dmp` et en renvoie l'inventaire (modules, plages, volume capturé).
///
/// Aucun scan : sert à valider le fichier et à afficher le volume avant d'en lancer un.
#[tauri::command]
#[specta::specta]
pub fn re_dump_open(chemin_dmp: String) -> Result<ReDumpInfoDto, String> {
    let dump = nie_dump::Minidump::open(&chemin_dmp).map_err(|e| format!("{chemin_dmp} : {e}"))?;
    let nie_base = dump.module("nie.exe").map(|m| format!("0x{:x}", m.base));
    Ok(ReDumpInfoDto {
        modules: dump
            .modules
            .iter()
            .map(|m| ReDumpModuleDto {
                name: m.name.clone(),
                base: format!("0x{:x}", m.base),
                size: f64::from(m.size),
            })
            .collect(),
        ranges: dump.range_count() as u32,
        mapped_bytes: dump.mapped_bytes() as f64,
        nie_base,
    })
}

#[derive(Serialize, specta::Type)]
pub struct ReDumpHitDto {
    /// Adresse virtuelle live du coup, en hexadécimal `0x…`. **Chaîne et pas nombre** : specta
    /// refuse d'exporter un `u64` vers TypeScript (perte de précision au-delà de 2⁵³) et le refus
    /// est FATAL — l'export des bindings panique au démarrage de l'app. Contrairement aux tailles
    /// (cf. [`ReTraceRegionDto::size`]), une adresse ne se dégrade pas en `f64` sans risque : la
    /// base image statique `0x1_4000_0000` plus un RVA reste exact, mais une adresse de tas ASLR
    /// x64 va jusqu'à 2⁴⁷ et l'hexa est de toute façon la forme qu'on copie dans un débogueur.
    va: String,
    /// Module contenant l'adresse, le cas échéant.
    module: Option<String>,
    /// Offset dans le module (RVA), hexadécimal `0x…`.
    rva: Option<String>,
    /// Adresse **statique** correspondante (`0x140000000 + rva`) si le coup est dans `nie.exe` —
    /// c'est celle qui se cherche dans `var/niers.sqlite` et dans le désassemblage.
    statique: Option<String>,
}

#[derive(Serialize, specta::Type)]
pub struct ReDumpScanDto {
    hits: Vec<ReDumpHitDto>,
    /// `true` si le scan s'est arrêté sur la limite : d'autres coups existent au-delà.
    tronque: bool,
    /// Octets réellement parcourables dans ce dump (borne haute du travail du scan).
    mapped_bytes: f64,
}

/// Scanne un motif AOB façon Cheat Engine (`"44 8B ?? 10"`, `??`/`?`/`*` = joker) dans un
/// minidump déjà capturé.
///
/// Le scan relit les plages mémoire du dump depuis le disque — plusieurs centaines de Mo pour une
/// capture complète : ce n'est **jamais** instantané, comptez plusieurs secondes. `limite` borne le
/// nombre de coups renvoyés **et** le travail effectué (le scan s'arrête à la limite atteinte) ;
/// `0` applique le défaut, et la valeur est plafonnée.
///
/// Lecture seule d'un fichier : aucune attache au process du jeu, aucune écriture mémoire.
#[tauri::command]
#[specta::specta]
pub fn re_dump_scan(chemin_dmp: String, motif: String, limite: u32) -> Result<ReDumpScanDto, String> {
    let pattern = nie_dump::Pattern::parse(&motif).map_err(|e| e.to_string())?;
    let limite = match limite {
        0 => DUMP_SCAN_LIMITE_DEFAUT,
        n => n.min(DUMP_SCAN_LIMITE_MAX),
    } as usize;
    let mut dump = nie_dump::Minidump::open(&chemin_dmp).map_err(|e| format!("{chemin_dmp} : {e}"))?;
    let mapped_bytes = dump.mapped_bytes() as f64;
    let hits = dump.scan_limited(&pattern, limite).map_err(|e| e.to_string())?;
    let tronque = hits.len() >= limite;
    Ok(ReDumpScanDto {
        hits: hits
            .iter()
            .map(|h| ReDumpHitDto {
                va: format!("0x{:x}", h.va),
                module: h.module.clone(),
                rva: h.rva.map(|r| format!("0x{r:x}")),
                statique: h.nie_static().map(|a| format!("0x{a:x}")),
            })
            .collect(),
        tronque,
        mapped_bytes,
    })
}
