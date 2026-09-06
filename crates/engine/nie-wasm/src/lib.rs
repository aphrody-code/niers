//! Bindings WebAssembly pour `nie-formats`.
//!
//! Expose au navigateur les parsers portables de `nie-formats` via `wasm-bindgen`. La cible
//! `wasm32-unknown-unknown` s'installe par `rustup target add wasm32-unknown-unknown` et n'a rien
//! de spécifique à une plateforme.
//!
//! ## Génération des bindings JS
//!
//! La compilation vers wasm32 seule ne suffit pas à produire les glues JS.
//!
//! **Le CLI `wasm-bindgen` doit avoir EXACTEMENT la version épinglée par le workspace**
//! (`wasm-bindgen = { version = "=…" }` dans le `Cargo.toml` racine) : un écart, même de patch,
//! fait rejeter les bindings générés. Ne pas recopier un numéro ici — il dériverait. Lire le pin,
//! puis installer le CLI correspondant :
//!
//! ```sh
//! # Option A — wasm-bindgen-cli, à la version du workspace
//! cargo install wasm-bindgen-cli --version "$(grep -oE 'wasm-bindgen = \{ version = "=[0-9.]+"' Cargo.toml | grep -oE '[0-9.]+')"
//! cargo build -p nie-wasm --target wasm32-unknown-unknown --release
//! wasm-bindgen target/wasm32-unknown-unknown/release/nie_wasm.wasm \
//!     --out-dir pkg/ --target bundler
//!
//! # Option B — wasm-pack (enchaîne les deux étapes)
//! cargo install wasm-pack
//! wasm-pack build crates/engine/nie-wasm --target bundler
//! ```
//!
//! `scripts/build-wasm.sh` fait le contrôle d'alignement avant de construire.
//!
//! ## Surface exposée
//!
//! En plus des parsers de formats (`nie-formats`), ce crate expose au navigateur le
//! savoir VÉRIFIÉ déjà porté dans les autres crates niers :
//!
//! - **`nie-core`** — calcul de statistiques (courbe de croissance lv1→99, ancrée sur
//!   `inagle/stat-calculator.ts`) et machine à états du match (FSM 11 états + score).
//! - **`nie-data`** — lookup skill (résolution hissatsu name/element/power), aura
//!   (sous-type + résolution du hissatsu lié) et item (catégorie + stats d'équipement),
//!   parsés depuis les dumps `*.cfg.bin.json` d'IEVR.
//!
//! Toutes les fonctions retournant une structure le font en **JSON sérialisé** (`String`),
//! que le JS désérialise via `JSON.parse`.
//!
//! ## Pattern d'import JS (ESM bundler)
//!
//! ```text
//! import init, {
//!   init_panic_hook, detect_format, crilayla_decompress, utf_table_json,
//!   calculate_stats, single_stat, rarity_to_growth_rank,
//!   match_tick, final_score,
//!   skill_lookup, aura_lookup, item_lookup,
//! } from "./pkg/nie_wasm.js";
//!
//! await init(); // charge le .wasm
//! init_panic_hook(); // redirige les panics Rust vers console.error
//!
//! const bytes = new Uint8Array(await file.arrayBuffer());
//! const format = detect_format(bytes);          // "CPK" | "CRILAYLA" | "@UTF" | …
//!
//! if (format === "CRILAYLA") {
//!   const decompressed = crilayla_decompress(bytes); // Uint8Array | throws Error
//! }
//! if (format === "@UTF") {
//!   const json = utf_table_json(bytes);              // string JSON | throws Error
//!   const table = JSON.parse(json);
//! }
//!
//! // Stats : FW rang UR (mainPosition 4, rank 5) au niveau 99.
//! const stats = JSON.parse(calculate_stats(4, 0, 0, 5, 0, 99));
//! console.log(stats.stats); // { kc, cr, tc, pr, ps, ag, it }
//!
//! // Match : transition de la FSM + score final.
//! const t = JSON.parse(match_tick("WaitTimer", false, 0)); // { next, immediate }
//! const score = final_score(2, 30); // 20030
//!
//! // Lookup data depuis un dump cfg.bin.json (string).
//! const skills = JSON.parse(skill_lookup(skillConfigJson, skillTextJson));
//! ```
//!
//! ## Sécurité
//!
//! `#![forbid(unsafe_code)]` est actif. `wasm-bindgen` génère du code unsafe dans
//! ses macros, mais ce code ne figure pas dans ce crate source — il est émis par le
//! compilateur à partir des attributs `#[wasm_bindgen]`, hors du scope de `forbid`.

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

#[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
pub mod web_viewer;

use nie_formats::{FileFormat, cfgbin, cpk, crilayla, detect};

// wasm-bindgen n'est importé qu'en cible wasm32.
// En cible native (rlib), le crate compile sans wasm-bindgen-sys → pas de linker wasm requis.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Initialiseur de hook de panique
// ---------------------------------------------------------------------------

/// Installe le hook de panique `console_error_panic_hook`.
///
/// Appeler cette fonction UNE FOIS au démarrage (après `await init()`) pour que
/// toute panique Rust apparaisse dans la console du navigateur avec un message
/// lisible au lieu d'une erreur Wasm opaque. Conservée pour compat ; le hook est
/// désormais aussi installé automatiquement par [`__wasm_start`] (best practice).
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn init_panic_hook() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
}

/// Point d'entrée **auto-exécuté à l'instanciation** du module (attribut `start`,
/// best practice wasm-bindgen) : installe le hook de panique sans dépendre d'un
/// appel JS explicite — toute panique reste lisible même si l'hôte oublie l'init.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn __wasm_start() {
    console_error_panic_hook::set_once();
}

// ---------------------------------------------------------------------------
// detect_format
// ---------------------------------------------------------------------------

/// Détecte le format d'un tampon d'octets et retourne son nom court.
///
/// Retourne l'une des chaînes suivantes :
/// `"CPK"`, `"@UTF"`, `"CRILAYLA"`, `"HCA"`, `"ACB"`, `"AWB"`, `"USM"`,
/// `"cfg.bin"`, `"G4MG"`, `"G4MD"`, `"G4TX"`, `"G4SK"`, `"G4PK"`, `"G4NV"`, `"?"`.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn detect_format(bytes: &[u8]) -> String {
    // Étend la détection avec RDBN (cfg.bin) non encore couvert par nie_formats::detect.
    let fmt = detect(bytes);
    if fmt == FileFormat::Unknown && cfgbin::is_rdbn(bytes) {
        return FileFormat::CfgBin.name().to_owned();
    }
    fmt.name().to_owned()
}

// ---------------------------------------------------------------------------
// crilayla_decompress
// ---------------------------------------------------------------------------

/// Décompresse un tampon CRILAYLA.
///
/// Retourne les octets décompressés, ou lève une `Error` JS si le format est invalide.
///
/// En JS :
/// ```text
/// try {
///   const raw = crilayla_decompress(bytes); // Uint8Array
/// } catch (e) {
///   console.error("Décompression échouée :", e);
/// }
/// ```
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn crilayla_decompress(bytes: &[u8]) -> Result<Vec<u8>, JsValue> {
    crilayla::decompress(bytes).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Décompresse un tampon CRILAYLA (version native, sans JsValue).
///
/// En natif, retourne `Err(String)` au lieu de `Err(JsValue)`.
/// Utiliser [`crilayla_decompress`] en cible wasm32.
#[cfg(not(target_arch = "wasm32"))]
pub fn crilayla_decompress(bytes: &[u8]) -> Result<Vec<u8>, String> {
    crilayla::decompress(bytes).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// utf_table_json
// ---------------------------------------------------------------------------

/// Parse une table `@UTF` et retourne son contenu sérialisé en JSON.
///
/// Le JSON a la structure suivante :
///
/// ```text
/// {
///   "nom": "NomDeLaTable",
///   "colonnes": [{ "nom": "ColA", "type": "U32" }, ...],
///   "lignes": [[42, "hello"], ...]
/// }
/// ```
///
/// En JS :
/// ```text
/// const json = utf_table_json(bytes);
/// const table = JSON.parse(json);
/// console.log(table.nom, table.lignes.length);
/// ```
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn utf_table_json(bytes: &[u8]) -> Result<String, JsValue> {
    serialiser_utf(bytes).map_err(|e| JsValue::from_str(&e))
}

/// Sérialise une table @UTF en JSON (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn utf_table_json(bytes: &[u8]) -> Result<String, String> {
    serialiser_utf(bytes)
}

// ---------------------------------------------------------------------------
// Logique partagée wasm32 / natif
// ---------------------------------------------------------------------------

/// Sérialise une table @UTF en JSON (logique commune aux deux targets).
fn serialiser_utf(bytes: &[u8]) -> Result<String, String> {
    let table = cpk::parse_utf(bytes).map_err(|e| e.to_string())?;

    let colonnes: Vec<serde_json::Value> = table
        .columns
        .iter()
        .map(|c| {
            serde_json::json!({
                "nom":  c.name,
                "type": format!("{:?}", c.col_type),
            })
        })
        .collect();

    let lignes: Vec<Vec<serde_json::Value>> = table
        .rows
        .iter()
        .map(|row| row.iter().map(utf_value_to_json).collect())
        .collect();

    let obj = serde_json::json!({
        "nom":     table.name,
        "colonnes": colonnes,
        "lignes":  lignes,
    });

    serde_json::to_string(&obj).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Helper : UtfValue → serde_json::Value
// ---------------------------------------------------------------------------

fn utf_value_to_json(v: &cpk::UtfValue) -> serde_json::Value {
    use cpk::UtfValue;
    match v {
        UtfValue::U8(x) => serde_json::json!(x),
        UtfValue::I8(x) => serde_json::json!(x),
        UtfValue::U16(x) => serde_json::json!(x),
        UtfValue::I16(x) => serde_json::json!(x),
        UtfValue::U32(x) => serde_json::json!(x),
        UtfValue::I32(x) => serde_json::json!(x),
        UtfValue::U64(x) => serde_json::json!(x),
        UtfValue::I64(x) => serde_json::json!(x),
        UtfValue::F32(x) => serde_json::json!(x),
        UtfValue::F64(x) => serde_json::json!(x),
        UtfValue::String(s) => serde_json::json!(s),
        UtfValue::Bytes(b) => {
            // Les blobs sont encodés en tableau d'entiers pour rester JSON-pur.
            serde_json::json!(b)
        }
    }
}

// ===========================================================================
// nie-core — calcul de statistiques (growth)
// ===========================================================================

/// Calcule le bloc de 7 statistiques d'un personnage à un niveau donné.
///
/// Combine les tables de croissance réelles IEVR embarquées (`nie-core`,
/// ancrées sur `inagle/stat-calculator.ts`) avec la résolution par fallback en
/// cascade (lv1/lv30/main) puis l'interpolation 3-segments.
///
/// Paramètres :
/// - `main_position` : 1=GK, 2=DF, 3=MF, 4=FW.
/// - `sub_position` : sous-position (0 = aucune).
/// - `growth_pattern` : pattern de croissance (0, 1, 2+).
/// - `chara_rank` : code de rareté brut (0=N, 2=R, 3=SR, 4=SSR, 5=UR, 6=LR, 7=Legend, 20=BASARA).
/// - `play_style` : style de jeu (0 par défaut).
/// - `level` : niveau 1..=99.
///
/// Retourne un JSON :
/// ```text
/// {
///   "stats": { "kc": 207, "cr": 216, "tc": 218, "pr": 235, "ps": 242, "ag": 210, "it": 261 },
///   "total": 1589
/// }
/// ```
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[must_use]
pub fn calculate_stats(
    main_position: u8,
    sub_position: u8,
    growth_pattern: u8,
    chara_rank: u8,
    play_style: u8,
    level: u8,
) -> String {
    use nie_core::growth::{GrowthParams, GrowthTables, calculate_stats as core_calc};

    let tables = GrowthTables::load_embedded();
    let params = GrowthParams {
        main_position,
        sub_position,
        growth_pattern,
        chara_rank,
        play_style,
    };
    let block = core_calc(&tables, &params, level);
    serde_json::json!({
        "stats": block,
        "total": block.total(),
    })
    .to_string()
}

/// Calcule une statistique unique par interpolation 3-segments (lv1/30/50/99).
///
/// Expose directement `nie_core::stats::calculate_single_stat`. Les niveaux hors
/// plage sont clampés (lv≤1 → `stat_lv1`, lv≥99 → `stat_lv99`).
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[must_use]
pub fn single_stat(
    level: u8,
    stat_lv1: u16,
    stat_lv30: u16,
    stat_lv50: u16,
    stat_lv99: u16,
) -> u16 {
    nie_core::stats::calculate_single_stat(level, stat_lv1, stat_lv30, stat_lv50, stat_lv99)
}

/// Convertit un code de rareté brut en rang de table de croissance.
///
/// Expose `nie_core::stats::rarity_to_growth_rank` (0→0, 2→2, …, 5/6/7/20→5).
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[must_use]
pub fn rarity_to_growth_rank(rarity_code: u8) -> u8 {
    nie_core::stats::rarity_to_growth_rank(rarity_code)
}

// ===========================================================================
// nie-core — machine à états du match (FSM)
// ===========================================================================

/// Convertit un nom d'état (insensible à la casse) vers le `MatchState` typé.
///
/// Accepte les libellés canoniques (`"Init"`, `"WaitTimer"`, `"ResultUi"`,
/// `"CheckTelop"`, `"WaitAnim"`, `"Transition"`, `"Fade"`, `"Cleanup"`,
/// `"PostMatch"`, `"FadeOut"`, `"LoadNext"`) et les index numériques `"0".."10"`.
fn parse_match_state(state: &str) -> Option<nie_core::match_fsm::MatchState> {
    use nie_core::match_fsm::MatchState as S;
    // Index numérique direct (0-10) — réutilise le TryFrom<u8> porté.
    if let Ok(n) = state.trim().parse::<u8>() {
        return S::try_from(n).ok();
    }
    let key = state.trim().to_ascii_lowercase();
    Some(match key.as_str() {
        "init" => S::Init,
        "waittimer" => S::WaitTimer,
        "resultui" => S::ResultUi,
        "checktelop" => S::CheckTelop,
        "waitanim" => S::WaitAnim,
        "transition" => S::Transition,
        "fade" => S::Fade,
        "cleanup" => S::Cleanup,
        "postmatch" => S::PostMatch,
        "fadeout" => S::FadeOut,
        "loadnext" => S::LoadNext,
        _ => return None,
    })
}

/// Logique commune `match_tick` (wasm32 / natif) : résout l'état suivant de la FSM.
fn match_tick_impl(state: &str, is_training: bool, end_counter: i32) -> Result<String, String> {
    use nie_core::match_fsm::{MatchContext, tick};

    let s = parse_match_state(state).ok_or_else(|| alloc_format_unknown_state(state))?;
    let ctx = MatchContext {
        is_training,
        end_counter,
    };
    let t = tick(s, ctx);
    serde_json::json!({
        "next": t.next,
        "immediate": t.immediate,
    })
    .to_string()
    .pipe_ok()
}

/// Message d'erreur pour un état de match inconnu.
fn alloc_format_unknown_state(state: &str) -> String {
    format!("état de match inconnu : {state:?}")
}

/// Avance la machine à états du match d'un tick (transition nominale).
///
/// Porte la FSM 11 états de `CSceneSoccer` (`nie-core::match_fsm::tick`).
/// - `state` : nom de l'état courant (`"Init"`, `"WaitTimer"`, … ou index `"0".."10"`).
/// - `is_training` : flag entraînement (`false` = match normal).
/// - `end_counter` : compteur de fin (case 5 : 0/1 = restart, 2 = complétion).
///
/// Retourne un JSON `{ "next": "WaitTimer", "immediate": false }`, ou lève une
/// `Error` JS si l'état est inconnu.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn match_tick(state: &str, is_training: bool, end_counter: i32) -> Result<String, JsValue> {
    match_tick_impl(state, is_training, end_counter).map_err(|e| JsValue::from_str(&e))
}

/// Avance la FSM du match d'un tick (version native, sans `JsValue`).
#[cfg(not(target_arch = "wasm32"))]
pub fn match_tick(state: &str, is_training: bool, end_counter: i32) -> Result<String, String> {
    match_tick_impl(state, is_training, end_counter)
}

/// Encode le score final du match : `minutes * 10000 + secondes`.
///
/// Expose `nie_core::match_fsm::final_score` (case 7 de `FUN_1412aa4a0`).
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[must_use]
pub fn final_score(minutes: u16, seconds: u16) -> u32 {
    nie_core::match_fsm::final_score(minutes, seconds)
}

// ===========================================================================
// nie-data — lookup skill / aura / item (résolution hissatsu)
// ===========================================================================

/// Logique commune `skill_lookup` : parse `skill_config` (+ `skill_text` optionnel)
/// et émet la liste des techniques avec nom/élément/catégorie/puissance résolus.
fn skill_lookup_impl(skill_config_json: &str, skill_text_json: &str) -> Result<String, String> {
    use nie_data::skill::{SkillTextMaps, join_skill_text, parse_skill_config, parse_skill_text};

    let config_root: serde_json::Value =
        serde_json::from_str(skill_config_json).map_err(|e| e.to_string())?;
    let skills = parse_skill_config(&config_root);

    // skill_text est optionnel : chaîne vide → pas de jointure nom/description.
    let maps = if skill_text_json.trim().is_empty() {
        SkillTextMaps::default()
    } else {
        let text_root: serde_json::Value =
            serde_json::from_str(skill_text_json).map_err(|e| e.to_string())?;
        parse_skill_text(&text_root)
    };

    let out: Vec<serde_json::Value> = skills
        .iter()
        .map(|s| {
            let text = join_skill_text(s, &maps);
            serde_json::json!({
                "skillId": s.skill_id.to_hex(),
                "skillIdStr": s.skill_id_str,
                "name": text.name,
                "description": text.description,
                "element": s.element(),
                "category": s.category(),
                "partnerType": s.partner_type(),
                "powerMin": s.power_min,
                "powerMax": s.power_max,
                "consumeTp": s.consume_tp,
                "recastTime": s.recast_time,
            })
        })
        .collect();

    serde_json::json!({ "count": out.len(), "skills": out })
        .to_string()
        .pipe_ok()
}

/// Parse un `skill_config.cfg.bin.json` (et un `skill_text.cfg.bin.json` optionnel)
/// et retourne les techniques résolues (nom/élément/catégorie/puissance).
///
/// - `skill_config_json` : contenu JSON du dump `skill_config_*.cfg.bin.json`.
/// - `skill_text_json` : contenu JSON du `skill_text_*.cfg.bin.json` (chaîne vide
///   pour ignorer la jointure nom/description).
///
/// Retourne un JSON `{ "count": N, "skills": [ { skillId, skillIdStr, name, element,
/// category, powerMin, powerMax, … }, … ] }`, ou lève une `Error` JS si le JSON est invalide.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn skill_lookup(skill_config_json: &str, skill_text_json: &str) -> Result<String, JsValue> {
    skill_lookup_impl(skill_config_json, skill_text_json).map_err(|e| JsValue::from_str(&e))
}

/// Parse `skill_config` (+ `skill_text` optionnel) (version native, sans `JsValue`).
#[cfg(not(target_arch = "wasm32"))]
pub fn skill_lookup(skill_config_json: &str, skill_text_json: &str) -> Result<String, String> {
    skill_lookup_impl(skill_config_json, skill_text_json)
}

/// Logique commune `aura_lookup` : parse `aura_skill_config` (+ `skill_config`
/// optionnel pour la résolution du hissatsu lié) et émet les auras.
fn aura_lookup_impl(aura_config_json: &str, skill_config_json: &str) -> Result<String, String> {
    use nie_data::aura::{build_skill_map, parse_all_aura_cmds, resolve_aura_hissatsu};
    use nie_data::skill::parse_skill_config;

    let aura_root: serde_json::Value =
        serde_json::from_str(aura_config_json).map_err(|e| e.to_string())?;
    let auras = parse_all_aura_cmds(&aura_root);

    // skill_config optionnel : permet la résolution native config.skillId1 → SkillInfo.
    let skill_map = if skill_config_json.trim().is_empty() {
        Default::default()
    } else {
        let skill_root: serde_json::Value =
            serde_json::from_str(skill_config_json).map_err(|e| e.to_string())?;
        build_skill_map(parse_skill_config(&skill_root))
    };

    let out: Vec<serde_json::Value> = auras
        .iter()
        .map(|a| {
            let hissatsu = resolve_aura_hissatsu(&a.config, &skill_map);
            serde_json::json!({
                "auraId": a.aura_id.to_hex(),
                "assetCode": a.asset_code,
                "subType": a.sub_type,
                "subTypeLabel": a.sub_type.label_fr(),
                "element": a.element(),
                "config": a.config,
                "hissatsu": hissatsu,
            })
        })
        .collect();

    serde_json::json!({ "count": out.len(), "auras": out })
        .to_string()
        .pipe_ok()
}

/// Parse un `aura_skill_config.cfg.bin.json` (et un `skill_config.cfg.bin.json`
/// optionnel pour résoudre le hissatsu lié) et retourne les auras.
///
/// - `aura_config_json` : contenu du dump `aura_skill_config_*.cfg.bin.json`.
/// - `skill_config_json` : contenu du `skill_config_*.cfg.bin.json` (chaîne vide
///   pour ignorer la résolution `config.skillId1 → SkillInfo`).
///
/// Retourne un JSON `{ "count": N, "auras": [ { auraId, assetCode, subType, element,
/// config, hissatsu }, … ] }`, ou lève une `Error` JS si le JSON est invalide.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn aura_lookup(aura_config_json: &str, skill_config_json: &str) -> Result<String, JsValue> {
    aura_lookup_impl(aura_config_json, skill_config_json).map_err(|e| JsValue::from_str(&e))
}

/// Parse `aura_skill_config` (+ `skill_config` optionnel) (version native, sans `JsValue`).
#[cfg(not(target_arch = "wasm32"))]
pub fn aura_lookup(aura_config_json: &str, skill_config_json: &str) -> Result<String, String> {
    aura_lookup_impl(aura_config_json, skill_config_json)
}

/// Logique commune `item_lookup` : parse `item_config` et émet les objets.
fn item_lookup_impl(item_config_json: &str) -> Result<String, String> {
    use nie_data::item::parse_all_items;

    let root: serde_json::Value =
        serde_json::from_str(item_config_json).map_err(|e| e.to_string())?;
    let items = parse_all_items(&root);

    let out: Vec<serde_json::Value> = items
        .iter()
        .map(|it| {
            serde_json::json!({
                "itemId": it.item_id.to_hex(),
                "category": it.category.as_str(),
                "nameId": it.name_id.to_hex(),
                "descId": it.desc_id.to_hex(),
                "price": it.price,
                "stats": it.stats,
                "internalCode": it.internal_code,
                "uniformId": it.uniform_id.map(|h| h.to_hex()),
            })
        })
        .collect();

    serde_json::json!({ "count": out.len(), "items": out })
        .to_string()
        .pipe_ok()
}

/// Parse un `item_config.cfg.bin.json` et retourne les objets (catégorie + stats).
///
/// - `item_config_json` : contenu du dump `item_config_*.cfg.bin.json`.
///
/// Retourne un JSON `{ "count": N, "items": [ { itemId, category, nameId, price,
/// stats, internalCode, … }, … ] }`, ou lève une `Error` JS si le JSON est invalide.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn item_lookup(item_config_json: &str) -> Result<String, JsValue> {
    item_lookup_impl(item_config_json).map_err(|e| JsValue::from_str(&e))
}

/// Parse `item_config` (version native, sans `JsValue`).
#[cfg(not(target_arch = "wasm32"))]
pub fn item_lookup(item_config_json: &str) -> Result<String, String> {
    item_lookup_impl(item_config_json)
}

// ---------------------------------------------------------------------------
// Petit utilitaire : `String` → `Result<String, String>` (lisibilité).
// ---------------------------------------------------------------------------

/// Trait d'extension minimal pour envelopper une valeur en `Ok(_)` de façon fluide.
trait PipeOk: Sized {
    fn pipe_ok<E>(self) -> Result<Self, E>;
}

impl PipeOk for String {
    fn pipe_ok<E>(self) -> Result<Self, E> {
        Ok(self)
    }
}

// ---------------------------------------------------------------------------
// G4MD & G4MG WebGPU 3D support
// ---------------------------------------------------------------------------

/// Parse un fichier G4MD et retourne son JSON descriptif.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn g4md_parse_json(bytes: &[u8]) -> Result<String, JsValue> {
    let parsed = nie_formats::g4md::parse(bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&parsed).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Parse un fichier G4MD (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn g4md_parse_json(bytes: &[u8]) -> Result<String, String> {
    let parsed = nie_formats::g4md::parse(bytes).map_err(|e| e.to_string())?;
    serde_json::to_string(&parsed).map_err(|e| e.to_string())
}

/// Parse un fichier cfg.bin (T2B) et retourne son JSON structurel.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn cfgbin_parse_json(bytes: &[u8]) -> Result<String, JsValue> {
    let parsed =
        nie_formats::cfgbin::cfgbin_parse(bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&parsed).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Parse un fichier cfg.bin (T2B) (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn cfgbin_parse_json(bytes: &[u8]) -> Result<String, String> {
    let parsed = nie_formats::cfgbin::cfgbin_parse(bytes).map_err(|e| e.to_string())?;
    serde_json::to_string(&parsed).map_err(|e| e.to_string())
}

// ── cfg.bin -> structures de jeu TYPÉES (décodage natif in-browser) ─────────────
// Reshape vers la forme iecode (`lists`/`entries`) puis dispatch `nie_data::typed`
// (37 familles). Remplace côté navigateur la route serveur `/typed`.

/// Octets bruts en hex MAJUSCULE (identique au dump iecode `defensePos`).
fn nw_hex_upper(bytes: &[u8]) -> String {
    use core::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02X}");
    }
    s
}

/// [`nie_formats::cfgbin::RdbnValue`] -> JSON, encodage identique iecode (hash `0x..`, blob hex MAJ).
fn nw_rdbn_value_to_json(v: &nie_formats::cfgbin::RdbnValue) -> serde_json::Value {
    use nie_formats::cfgbin::RdbnValue as R;
    use serde_json::{Value, json};
    match v {
        R::Bool(b) => json!(b),
        R::Byte(n) => json!(n),
        R::Short(n) | R::ActType(n) => json!(n),
        R::Int(n) | R::Flag(n) => json!(n),
        R::Float(f) => json!(f),
        R::Hash(h) => json!(format!("0x{h:08X}")),
        R::Rates(a) | R::Position(a) => json!(a),
        R::Condition(s) => json!(s),
        R::ShortTuple(t) => json!(t),
        R::Blob(b) => json!(nw_hex_upper(b)),
        _ => Value::Null,
    }
}

/// Liste de frères T2B -> forme iecode, avec réplication du suffixe d'index `<base>_<i>`
/// d'iecode (exigé par `walk_named`) et variables `{type, value:"<string>"}`.
fn nw_t2b_siblings(siblings: &[nie_formats::cfgbin::CfgEntry]) -> Vec<serde_json::Value> {
    use nie_formats::cfgbin::Value as CfgValue;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    siblings
        .iter()
        .map(|e| {
            let idx = counts.entry(e.name.as_str()).or_insert(0);
            let name = format!("{}_{}", e.name, *idx);
            *idx += 1;
            let variables: Vec<Value> = e
                .variables
                .iter()
                .map(|v| match v {
                    CfgValue::String(s) => json!({ "type": "String", "value": s }),
                    CfgValue::Int(n) => json!({ "type": "Int", "value": n.to_string() }),
                    CfgValue::Float(f) => json!({ "type": "Float", "value": f.to_string() }),
                })
                .collect();
            json!({ "name": name, "variables": variables, "children": nw_t2b_siblings(&e.children) })
        })
        .collect()
}

/// Décode un `cfg.bin` vers la forme iecode adaptée (RDBN `lists` ou T2B `entries`).
fn nw_cfgbin_to_iecode(data: &[u8]) -> Option<serde_json::Value> {
    use serde_json::{Map, Value, json};
    if nie_formats::cfgbin::is_rdbn(data) {
        let rdbn = nie_formats::cfgbin::parse(data).ok()?;
        let lists = nie_formats::cfgbin::read_values(&rdbn, data);
        let lists_json: Vec<Value> = lists
            .iter()
            .map(|l| {
                let values: Vec<Value> = l
                    .rows
                    .iter()
                    .map(|row| {
                        let mut m = Map::new();
                        for (name, val) in &row.fields {
                            m.insert(name.clone(), nw_rdbn_value_to_json(val));
                        }
                        Value::Object(m)
                    })
                    .collect();
                json!({ "name": l.name, "typeName": l.type_name, "values": values })
            })
            .collect();
        Some(json!({ "lists": lists_json }))
    } else {
        let cfg = nie_formats::cfgbin::cfgbin_parse(data).ok()?;
        Some(json!({ "entries": nw_t2b_siblings(&cfg.entries) }))
    }
}

/// Impl partagée : `cfg.bin` brut + nom de fichier -> `{family, data}` (famille typée) ou
/// `{family:null, key, generic}` (RDBN/T2B brut iecode).
fn cfgbin_typed_json_impl(bytes: &[u8], filename: &str) -> Result<String, String> {
    let root = nw_cfgbin_to_iecode(bytes).ok_or("cfg.bin non décodable (ni RDBN ni T2B)")?;
    let key = nie_data::typed::family_key(filename);
    let out = match nie_data::typed::decode_by_key(&key, &root) {
        Some((family, data)) => serde_json::json!({ "family": family, "data": data }),
        None => {
            serde_json::json!({ "family": serde_json::Value::Null, "key": key, "generic": root })
        }
    };
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

/// Décode un `*_menu_setting.cfg.bin` vers sa structure sémantique sans enveloppe de famille.
fn cfgbin_menu_setting_json_impl(bytes: &[u8]) -> Result<String, String> {
    let root = nw_cfgbin_to_iecode(bytes).ok_or("cfg.bin non décodable (ni RDBN ni T2B)")?;
    let setting = nie_data::menu_setting::parse(&root);
    serde_json::to_string(&setting).map_err(|e| e.to_string())
}

/// Décode un `cfg.bin` (octets bruts) en structure de jeu typée selon le nom de fichier.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn cfgbin_typed_json(bytes: &[u8], filename: &str) -> Result<String, JsValue> {
    cfgbin_typed_json_impl(bytes, filename).map_err(|e| JsValue::from_str(&e))
}

/// Décode un `cfg.bin` typé (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn cfgbin_typed_json(bytes: &[u8], filename: &str) -> Result<String, String> {
    cfgbin_typed_json_impl(bytes, filename)
}

/// Décode un `*_menu_setting.cfg.bin` en structure de menu directement consommable.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn cfgbin_menu_setting_json(bytes: &[u8]) -> Result<String, JsValue> {
    cfgbin_menu_setting_json_impl(bytes).map_err(|e| JsValue::from_str(&e))
}

/// Décode un `*_menu_setting.cfg.bin` en structure de menu directement consommable (natif).
#[cfg(not(target_arch = "wasm32"))]
pub fn cfgbin_menu_setting_json(bytes: &[u8]) -> Result<String, String> {
    cfgbin_menu_setting_json_impl(bytes)
}

// ── Bytecode Lua 5.2 (scripts du jeu) ──────────────────────────────────────────
// Le decodeur est celui du depot (`nie_lua::bytecode`), compile sans la VM : un `.lua.bin`
// se lit donc dans le navigateur, sans passer par un service.

fn lua_bytecode_json_impl(bytes: &[u8]) -> Result<String, String> {
    let chunk = nie_lua::bytecode::parse(bytes).map_err(|e| e.to_string())?;
    let proto = &chunk.main;
    let constantes: Vec<serde_json::Value> = proto
        .constants
        .iter()
        .map(|c| serde_json::Value::String(c.display()))
        .collect();
    let out = serde_json::json!({
        "instructions": proto.total_instructions(),
        "prototypes": proto.total_protos(),
        "params": proto.num_params,
        "constantes": constantes,
        "source": proto.source,
    });
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

/// Décode un `.lua.bin` du jeu (bytecode Lua 5.2) en résumé JSON.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn lua_bytecode_json(bytes: &[u8]) -> Result<String, JsValue> {
    lua_bytecode_json_impl(bytes).map_err(|e| JsValue::from_str(&e))
}

/// Décode un `.lua.bin` du jeu (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn lua_bytecode_json(bytes: &[u8]) -> Result<String, String> {
    lua_bytecode_json_impl(bytes)
}

/// Vrai si les octets commencent par la signature d'un bytecode Lua 5.2.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[must_use]
pub fn is_lua_bytecode(bytes: &[u8]) -> bool {
    nie_lua::bytecode::parse(bytes).is_ok()
}

// ── G4TX -> PNG (textures décodées NATIVEMENT in-browser) ──────────────────────
// Le décodage DDS/BCn → RGBA8 → PNG est centralisé dans `nie_formats::g4tx_decode`
// (feature `textures`, source unique du workspace — Phase 1b dédup). Côté navigateur,
// `image_dds` (default-features=false) reste un décodeur pur Rust compatible wasm32.
// Bonus vs l'ancienne copie locale (DX10 seul) : support FourCC legacy + non compressé
// + sélecteur anti-dummy → corrige les textures invisibles en wasm.

/// Décode la texture principale d'un `.g4tx` en PNG (via le décodeur partagé anti-dummy).
fn g4tx_to_png_impl(bytes: &[u8]) -> Result<Vec<u8>, String> {
    // Basename vide ASSUMÉ : l'ABI wasm ne reçoit que des octets, jamais le nom du fichier
    // source. La sélection retombe donc sur « la plus grande texture non-dummy ». Pour viser
    // une texture précise d'un conteneur multi-textures, passer par `g4tx_named_to_png`.
    nie_formats::g4tx_decode::decode_best_to_png(bytes, "")
        .ok_or_else(|| "décodage G4TX → PNG échoué".to_string())
}

/// Décode une texture **nommée** d'un `.g4tx` en PNG (conteneur multi-textures ou atlas).
fn g4tx_named_to_png_impl(bytes: &[u8], nom: &str) -> Result<Vec<u8>, String> {
    nie_formats::g4tx_decode::decode_named_to_png(bytes, nom)
        .ok_or_else(|| format!("texture « {nom} » absente ou non décodable"))
}

/// Décode un `.g4tx` (octets bruts) en PNG (octets), in-browser.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn g4tx_to_png(bytes: &[u8]) -> Result<Vec<u8>, JsValue> {
    g4tx_to_png_impl(bytes).map_err(|e| JsValue::from_str(&e))
}

/// Décode un `.g4tx` en PNG (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn g4tx_to_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    g4tx_to_png_impl(bytes)
}

/// Décode la texture nommée `nom` d'un `.g4tx` en PNG, in-browser.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn g4tx_named_to_png(bytes: &[u8], nom: &str) -> Result<Vec<u8>, JsValue> {
    g4tx_named_to_png_impl(bytes, nom).map_err(|e| JsValue::from_str(&e))
}

/// Décode la texture nommée `nom` d'un `.g4tx` en PNG (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn g4tx_named_to_png(bytes: &[u8], nom: &str) -> Result<Vec<u8>, String> {
    g4tx_named_to_png_impl(bytes, nom)
}

/// Métadonnées d'un `.g4tx` (textures : nom, dimensions, DDS) en JSON, in-browser.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn g4tx_info_json(bytes: &[u8]) -> Result<String, JsValue> {
    let g4tx = nie_formats::g4tx::parse(bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&g4tx).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Métadonnées d'un `.g4tx` (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn g4tx_info_json(bytes: &[u8]) -> Result<String, String> {
    let g4tx = nie_formats::g4tx::parse(bytes).map_err(|e| e.to_string())?;
    serde_json::to_string(&g4tx).map_err(|e| e.to_string())
}

/// Feuille de sprites d'un atlas `.g4tx` : régions nommées avec leur rectangle, en JSON.
///
/// `g4tx_info_json` rend la structure brute du conteneur ; celle-ci rend ce qu'une interface
/// attend — un manifeste `{nom, largeur, hauteur, sprites[{nom, classe, x, y, largeur, hauteur}]}`
/// directement consommable pour positionner une icône, avec ou sans CSS.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn g4tx_sprite_sheet_json(bytes: &[u8]) -> Result<String, JsValue> {
    sprite_sheet_json(bytes).map_err(|e| JsValue::from_str(&e))
}

/// Feuille de sprites d'un atlas `.g4tx` (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn g4tx_sprite_sheet_json(bytes: &[u8]) -> Result<String, String> {
    sprite_sheet_json(bytes)
}

/// Corps commun aux deux cibles : parse l'atlas, en extrait les régions, sérialise.
fn sprite_sheet_json(bytes: &[u8]) -> Result<String, String> {
    let g4tx = nie_formats::g4tx::parse(bytes).map_err(|e| e.to_string())?;
    let feuille = nie_formats::sprite_sheet::depuis_g4tx(&g4tx, 0)
        .ok_or_else(|| "aucune texture dans ce G4TX".to_string())?;
    Ok(feuille.vers_json())
}

/// Parse une archive `.g4pk` (en-tête + sous-fichiers) en JSON, in-browser.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn g4pk_parse_json(bytes: &[u8]) -> Result<String, JsValue> {
    let g4pk = nie_formats::g4pk::parse(bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&g4pk).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Parse une archive `.g4pk` (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn g4pk_parse_json(bytes: &[u8]) -> Result<String, String> {
    let g4pk = nie_formats::g4pk::parse(bytes).map_err(|e| e.to_string())?;
    serde_json::to_string(&g4pk).map_err(|e| e.to_string())
}

/// Décode une piste de lip-sync `.p3lip` (visèmes datés) en JSON, in-browser.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn lip_to_json(bytes: &[u8]) -> Result<String, JsValue> {
    let lip = nie_formats::lip::parse(bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&lip).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Décode une piste de lip-sync `.p3lip` (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn lip_to_json(bytes: &[u8]) -> Result<String, String> {
    let lip = nie_formats::lip::parse(bytes).map_err(|e| e.to_string())?;
    serde_json::to_string(&lip).map_err(|e| e.to_string())
}

// ── Modèle 3D g4md+g4mg -> GLB (assemblé NATIVEMENT in-browser) ────────────────

/// Assemble un modèle générique (paire G4MD + G4MG) en **GLB** (géométrie, glTF binaire).
fn model_to_glb_impl(g4md: &[u8], g4mg: &[u8]) -> Result<Vec<u8>, String> {
    use nie_formats::assemble::{GenericModelInput, MeshComponent, assemble_generic_model};
    let model = assemble_generic_model(GenericModelInput {
        code: String::new(),
        g4md: g4md.to_vec(),
        g4mg: g4mg.to_vec(),
        component: MeshComponent::Generic,
    })
    .map_err(|e| e.to_string())?;
    Ok(model.to_glb_embedded())
}

/// Assemble une paire G4MD+G4MG (octets bruts) en GLB, in-browser.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn model_to_glb(g4md: &[u8], g4mg: &[u8]) -> Result<Vec<u8>, JsValue> {
    model_to_glb_impl(g4md, g4mg).map_err(|e| JsValue::from_str(&e))
}

/// Assemble un modèle G4MD+G4MG en GLB (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn model_to_glb(g4md: &[u8], g4mg: &[u8]) -> Result<Vec<u8>, String> {
    model_to_glb_impl(g4md, g4mg)
}

// ── Audio CRI → WAV (in-browser) ──────────────────────────────────────────────
// Délègue à la SOURCE UNIQUE `nie_formats::cri_audio::decode_to_wav` (feature `audio-decode`) :
// le décode HCA chiffré IEVR + le dispatch ADX/AWB/ACB y vivent (dédup Phase 1d). Plus de copie ici.
fn audio_to_wav_impl(raw: &[u8]) -> Result<Vec<u8>, String> {
    nie_formats::cri_audio::decode_to_wav(raw)
}

/// Décode un audio CRI (HCA/ADX/AWB/ACB, octets bruts) en **WAV PCM16**, in-browser.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn audio_to_wav(bytes: &[u8]) -> Result<Vec<u8>, JsValue> {
    audio_to_wav_impl(bytes).map_err(|e| JsValue::from_str(&e))
}

/// Décode un audio CRI en WAV (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn audio_to_wav(bytes: &[u8]) -> Result<Vec<u8>, String> {
    audio_to_wav_impl(bytes)
}

/// Extrait la géométrie d'un fichier G4MG à l'aide des métadonnées G4MD fournies au format JSON.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn g4mg_extract_json(g4mg_bytes: &[u8], g4md_json: &str) -> Result<String, JsValue> {
    let g4md: nie_formats::g4md::G4md =
        serde_json::from_str(g4md_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let geom = nie_formats::g4mg::extract_geometry(g4mg_bytes, &g4md);
    serde_json::to_string(&geom).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Extrait la géométrie d'un fichier G4MG (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn g4mg_extract_json(g4mg_bytes: &[u8], g4md_json: &str) -> Result<String, String> {
    let g4md: nie_formats::g4md::G4md =
        serde_json::from_str(g4md_json).map_err(|e| e.to_string())?;
    let geom = nie_formats::g4mg::extract_geometry(g4mg_bytes, &g4md);
    serde_json::to_string(&geom).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// CPK Decryption & Extraction
// ---------------------------------------------------------------------------

/// Parse un fichier CPK et retourne son TOC (Table of Contents) au format JSON.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn cpk_parse_entries(cpk_bytes: &[u8], cpk_filename: &str) -> Result<String, JsValue> {
    let reader = nie_formats::cpk::CpkReader::new(cpk_bytes, cpk_filename)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&reader.entries).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Parse un fichier CPK (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn cpk_parse_entries(cpk_bytes: &[u8], cpk_filename: &str) -> Result<String, String> {
    let reader =
        nie_formats::cpk::CpkReader::new(cpk_bytes, cpk_filename).map_err(|e| e.to_string())?;
    serde_json::to_string(&reader.entries).map_err(|e| e.to_string())
}

/// Extrait et décompresse un fichier d'un CPK.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn cpk_extract_file(
    cpk_bytes: &[u8],
    cpk_filename: &str,
    entry_json: &str,
) -> Result<Vec<u8>, JsValue> {
    let reader = nie_formats::cpk::CpkReader::new(cpk_bytes, cpk_filename)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let entry: nie_formats::cpk::CpkEntry =
        serde_json::from_str(entry_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    reader
        .extract(cpk_bytes, &entry)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Extrait et décompresse un fichier d'un CPK (version native).
#[cfg(not(target_arch = "wasm32"))]
pub fn cpk_extract_file(
    cpk_bytes: &[u8],
    cpk_filename: &str,
    entry_json: &str,
) -> Result<Vec<u8>, String> {
    let reader =
        nie_formats::cpk::CpkReader::new(cpk_bytes, cpk_filename).map_err(|e| e.to_string())?;
    let entry: nie_formats::cpk::CpkEntry =
        serde_json::from_str(entry_json).map_err(|e| e.to_string())?;
    reader.extract(cpk_bytes, &entry).map_err(|e| e.to_string())
}

// ===========================================================================
// nie-save — parsing de saves IEVR côté navigateur (privacy-first)
// ===========================================================================

/// Résumé du conteneur Lives (champs sérialisables, sans les corps bruts).
///
/// Structure JSON retournée par [`parse_save_json`] :
///
/// ```text
/// {
///   "slot_name": "002AB8F4-USERDATALIVE",
///   "key": 1320666147,
///   "blobs": [
///     {
///       "filename": "AUTOSAVE_data.bin",
///       "subtype": "Autosave",
///       "size": 12345678,
///       "crc32": 305419896
///     },
///     ...
///   ],
///   "headersave": { ... },   // présent si blob HEADERSAVE parsé
///   "autosave": { ... }      // présent si blob AUTOSAVE parsé
/// }
/// ```
///
/// Les corps bruts (> 12 Mo pour AUTOSAVE) ne sont jamais sérialisés.
fn parse_save_impl(bytes: &[u8], filename: &str) -> Result<String, String> {
    use nie_save::{
        BlobSubtype,
        body::{
            autosave::parse_autosave_layout, autosave_roster::parse_autosave_roster,
            headersave::parse_headersave,
        },
    };

    let container = nie_save::parse(bytes, filename).map_err(|e| e.to_string())?;

    // --- Résumé des blobs (sans les corps bruts) ---
    let blobs_summary: Vec<serde_json::Value> = container
        .entries
        .iter()
        .zip(container.blobs.iter())
        .map(|(entry, blob)| {
            let subtype = match blob.header.subtype {
                BlobSubtype::System => "System",
                BlobSubtype::Autosave => "Autosave",
                BlobSubtype::Headersave => "Headersave",
                BlobSubtype::Unknown(_) => "Unknown",
            };
            serde_json::json!({
                "filename": entry.filename,
                "subtype":  subtype,
                "size":     entry.size,
                "crc32":    entry.crc32,
                "field8":   blob.header.field8,
            })
        })
        .collect();

    // --- Parse HEADERSAVE si présent ---
    let headersave_json: Option<serde_json::Value> = container
        .blob_by_subtype(BlobSubtype::Headersave)
        .and_then(|blob| {
            parse_headersave(&blob.body).ok().map(|hs| {
                let ts = &hs.save_timestamp;
                let slots: Vec<serde_json::Value> = hs
                    .slots
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let dt = s.slot_datetime.as_ref().map(|d| {
                            serde_json::json!({
                                "year":   d.year,
                                "month":  d.month,
                                "day":    d.day,
                                "hour":   d.hour,
                                "minute": d.minute,
                                "second": d.second,
                            })
                        });
                        serde_json::json!({
                            "index":         i,
                            "is_active":     s.is_active,
                            "section_role":  s.section_role,
                            "slot_variant":  s.slot_variant,
                            "slot_datetime": dt,
                            "playtime_secs": s.playtime_secs,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "format_version": hs.format_version,
                    "max_slots":      hs.max_slots,
                    "used_slots":     hs.used_slots,
                    "player_name":    hs.player_name,
                    "level_str":      hs.level_str,
                    "unique_id":      hs.unique_id,
                    "save_timestamp": {
                        "year":   ts.year,
                        "month":  ts.month,
                        "day":    ts.day,
                        "hour":   ts.hour,
                        "minute": ts.minute,
                        "second": ts.second,
                    },
                    "slots": slots,
                })
            })
        });

    // --- Parse AUTOSAVE (layout + roster + scalaires) si présent ---
    let autosave_json: Option<serde_json::Value> = container
        .blob_by_subtype(BlobSubtype::Autosave)
        .and_then(|blob| {
            // Layout macroscopique (sections + table CharaParam)
            let layout = parse_autosave_layout(&blob.body).ok()?;

            // Roster + scalaires (section EEFF 0x0510)
            let roster = parse_autosave_roster(&blob.body).ok()?;

            let scalars_json = roster.scalars.as_ref().map(|s| {
                let (h, m, sec) = s.playtime_hms();
                serde_json::json!({
                    "save_year":    s.save_year,
                    "save_month":   s.save_month,
                    "save_day":     s.save_day,
                    "save_hour":    s.save_hour,
                    "save_minute":  s.save_minute,
                    "save_second":  s.save_second,
                    "playtime_secs": s.playtime_secs,
                    "playtime_hms": { "h": h, "m": m, "s": sec },
                })
            });

            // Le roster complet peut être ~18 Ko de JSON (4534 ids × 12 chars) —
            // acceptable dans un contexte navigateur (la save elle-même fait 12 Mo).
            let owned_ids: Vec<u32> = roster.owned.iter().map(|c| c.raw()).collect();

            Some(serde_json::json!({
                "version":            layout.version,
                "opaque_4":           layout.opaque_4,
                "scalar_record_count": layout.scalar_record_count,
                "chara_slot_count":   layout.chara_param_slots.len(),
                "section2_range":     layout.section2_range,
                "main_data_range":    layout.main_data_range,
                "scalars":            scalars_json,
                "roster_slots":       roster.roster_slots,
                "owned_count":        roster.owned.len(),
                "owned_ids":          owned_ids,
            }))
        });

    let result = serde_json::json!({
        "slot_name":  container.slot_name,
        "key":        container.key,
        "blobs":      blobs_summary,
        "headersave": headersave_json,
        "autosave":   autosave_json,
    });

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Déchiffre et parse un fichier de sauvegarde IEVR, retourne un JSON résumé.
///
/// La save ne quitte PAS le navigateur : tout le traitement est effectué
/// client-side dans le module WebAssembly.
///
/// - `bytes` : contenu brut du fichier de sauvegarde (ex. `002AB8F4-USERDATALIVE`).
/// - `filename` : nom de base du fichier (sert à dériver la clé CRC32).
///
/// Retourne un JSON avec :
/// - `slot_name`, `key` : métadonnées du conteneur.
/// - `blobs` : liste des entrées (filename, subtype, size, crc32, field8).
/// - `headersave` : champs HEADERSAVE parsés (joueur, niveau, horodatage, slots).
/// - `autosave` : layout macroscopique + scalaires + roster complet (owned_ids).
///
/// Lève une `Error` JS (wasm32) ou retourne `Err(String)` (natif) si le fichier
/// est invalide ou la clé ne correspond pas au nom.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn parse_save_json(bytes: &[u8], filename: &str) -> Result<String, JsValue> {
    parse_save_impl(bytes, filename).map_err(|e| JsValue::from_str(&e))
}

/// Déchiffre et parse une save IEVR (version native, sans `JsValue`).
#[cfg(not(target_arch = "wasm32"))]
pub fn parse_save_json(bytes: &[u8], filename: &str) -> Result<String, String> {
    parse_save_impl(bytes, filename)
}

// ---------------------------------------------------------------------------
// Machine à états d'écran en navigateur (nie-app : GameState + framebuffer CPU)
// ---------------------------------------------------------------------------

/// Machine à états d'écran interactive, rendue en WebAssembly.
///
/// Écran-titre → menu → match simulé (`nie-runtime` : physique, 22 joueurs, ballon, buts) → mode
/// histoire, pilotée au clavier, rendue dans un framebuffer RGBA8 `W*H*4` que JS peint.
///
/// ⚠ **Ce n'est pas le jeu.** Le rendu est un placeholder 2D : il ne ressemble pas à l'UI d'IEVR,
/// parce que le vrai menu n'est pas dans les fichiers — il est construit à l'exécution par le
/// menu-manager C++ qui pilote Lua via `funcLuaMenuCommand`, boucle non encore portée. Et le
/// modèle de but de `match_sim` reste nominal. Ne pas présenter cette surface comme un jeu
/// jouable : ce qu'elle prouve, c'est que la logique portée tourne en wasm.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WasmGame {
    font: nie_app::Font,
    screen: nie_app::flow::Screen,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WasmGame {
    /// Construit le jeu depuis les octets de la police (`font.cfg.bin` + `font.g4tx`, fetchés par JS).
    /// Démarre sur l'écran-titre.
    #[wasm_bindgen(constructor)]
    pub fn new(font_cfg: &[u8], font_g4tx: &[u8]) -> Result<WasmGame, JsValue> {
        let font = nie_app::Font::from_bytes(font_cfg, font_g4tx)
            .map_err(|e| JsValue::from_str(&format!("font: {e}")))?;
        Ok(WasmGame {
            font,
            screen: nie_app::flow::Screen::new(),
        })
    }

    /// Largeur du framebuffer (px).
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> usize {
        nie_app::W
    }

    /// Hauteur du framebuffer (px).
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> usize {
        nie_app::H
    }

    /// Commande de menu IEVR (CMD_FCS_*, CMD_ENTER, CMD_BACK…). Le mapping clavier/souris/manette
    /// → commande vit côté front ; la FSM (transitions) vit dans `nie_app::flow` (dédup Phase 5).
    pub fn input(&mut self, cmd: &str) {
        self.screen.input(cmd);
    }

    /// Avance le temps de `dt` s : la physique du match tourne quand un match est en cours.
    pub fn update(&mut self, dt: f32) {
        self.screen.update(dt);
    }

    /// Score du match en cours `[domicile, extérieur]` (zéros hors match).
    pub fn score(&self) -> Vec<u32> {
        self.screen.score()
    }

    /// `true` si un match est en cours (pour l'overlay de score côté UI).
    #[wasm_bindgen(getter)]
    pub fn in_match(&self) -> bool {
        self.screen.in_match()
    }

    /// Rend l'écran courant en framebuffer RGBA8 `W*H*4`.
    pub fn render(&self) -> Vec<u8> {
        self.screen.render(&self.font)
    }
}

// ---------------------------------------------------------------------------
// Tests natifs
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_format_utf() {
        let magic = &[0x40u8, 0x55, 0x54, 0x46, 0x00, 0x00, 0x00, 0x08];
        assert_eq!(detect_format(magic), "@UTF");
    }

    #[test]
    fn detect_format_cpk() {
        assert_eq!(detect_format(b"CPK \x00\x00\x00\x00"), "CPK");
    }

    #[test]
    fn detect_format_crilayla() {
        assert_eq!(
            detect_format(b"CRILAYLA\x00\x00\x00\x00\x00\x00\x00\x00"),
            "CRILAYLA"
        );
    }

    #[test]
    fn detect_format_rdbn() {
        let mut buf = vec![0u8; 0x50];
        buf[0..4].copy_from_slice(b"RDBN");
        buf[6..10].copy_from_slice(&100i32.to_le_bytes());
        buf[10..12].copy_from_slice(&0x14i16.to_le_bytes());
        assert_eq!(detect_format(&buf), "cfg.bin");
    }

    #[test]
    fn detect_format_inconnu() {
        assert_eq!(detect_format(b"GARBAGE"), "?");
    }

    #[test]
    fn init_panic_hook_ne_panique_pas() {
        // En natif, le hook est une no-op wasm ; la fonction ne doit pas paniquer.
        init_panic_hook();
    }

    #[test]
    fn crilayla_decompress_trop_court() {
        let result = crilayla_decompress(b"CRILAYLA\x00\x00");
        assert!(result.is_err());
    }

    #[test]
    fn utf_table_json_mauvais_magic() {
        let result = utf_table_json(b"NOTUTF\x00\x00\x00\x00");
        assert!(result.is_err());
    }

    #[test]
    fn utf_table_json_fixture() {
        // @UTF minimal 2 colonnes / 2 lignes.
        let string_pool: &[u8] = b"TestTable\0ColA\0ColB\0hello\0world\0";
        let schema: &[u8] = &[0x24, 0x00, 0x00, 0x00, 0x0A, 0x2A, 0x00, 0x00, 0x00, 0x0F];
        let row_data: &[u8] = &[
            0x00, 0x00, 0x00, 42, 0x00, 0x00, 0x00, 20, 0x00, 0x00, 0x00, 99, 0x00, 0x00, 0x00, 26,
        ];
        let mut body = Vec::new();
        body.extend_from_slice(&0x22u32.to_be_bytes());
        body.extend_from_slice(&0x32u32.to_be_bytes());
        body.extend_from_slice(&0x52u32.to_be_bytes());
        body.extend_from_slice(&0u32.to_be_bytes());
        body.extend_from_slice(&2u16.to_be_bytes());
        body.extend_from_slice(&8u16.to_be_bytes());
        body.extend_from_slice(&2u32.to_be_bytes());
        body.extend_from_slice(schema);
        body.extend_from_slice(row_data);
        body.extend_from_slice(string_pool);
        let mut data = Vec::new();
        data.extend_from_slice(&[0x40, 0x55, 0x54, 0x46]);
        data.extend_from_slice(&(body.len() as u32).to_be_bytes());
        data.extend_from_slice(&body);

        let json_str = utf_table_json(&data).expect("parse @UTF");
        let json: serde_json::Value = serde_json::from_str(&json_str).expect("JSON valide");
        assert_eq!(json["nom"], "TestTable");
        assert_eq!(json["colonnes"].as_array().unwrap().len(), 2);
        assert_eq!(json["lignes"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn cfgbin_menu_setting_json_t2b_fixture() {
        use nie_formats::cfgbin::{CfgEntry, Value, encode_t2b};

        let entries = vec![CfgEntry {
            name: "MENU_LAYER_INFO_LIST_BEG".into(),
            variables: vec![Value::Int(1)],
            children: vec![CfgEntry {
                name: "MENU_LAYER_INFO_0".into(),
                variables: vec![
                    Value::Int(367_379_312),
                    Value::String("mainmenu90_00_background".into()),
                    Value::String(
                        "common/gamedata/menu/obj/mainmenu90_00_background.objbin".into(),
                    ),
                    Value::Int(1),
                ],
                children: Vec::new(),
            }],
        }];
        let bytes = encode_t2b(&entries);
        let json: serde_json::Value =
            serde_json::from_str(&cfgbin_menu_setting_json(&bytes).expect("menu JSON"))
                .expect("JSON valide");
        assert_eq!(json["layers"].as_array().unwrap().len(), 1);
        assert_eq!(json["layers"][0]["layer_id"], 367_379_312u32);
        assert_eq!(json["layers"][0]["name"], "mainmenu90_00_background");
        assert_eq!(json["layers"][0]["params"][0], 1);
    }

    // -----------------------------------------------------------------------
    // nie-core — stats (growth)
    // -----------------------------------------------------------------------

    /// Golden : FW rang UR (mainPosition 4, rank 5) au niveau 99.
    /// Ancré sur `nie_core::growth` test `golden_fw_ur` (sorties RÉELLES d'inagle).
    #[test]
    fn calculate_stats_fw_ur_lv99() {
        let json: serde_json::Value =
            serde_json::from_str(&calculate_stats(4, 0, 0, 5, 0, 99)).expect("JSON valide");
        let s = &json["stats"];
        assert_eq!(s["kc"], 207);
        assert_eq!(s["cr"], 216);
        assert_eq!(s["tc"], 218);
        assert_eq!(s["pr"], 235);
        assert_eq!(s["ps"], 242);
        assert_eq!(s["ag"], 210);
        assert_eq!(s["it"], 261);
        // total = somme des 7 stats.
        assert_eq!(json["total"], 207 + 216 + 218 + 235 + 242 + 210 + 261);
    }

    /// Golden : GK rang N au niveau 1 (= valeurs de base lv1).
    #[test]
    fn calculate_stats_gk_n_lv1() {
        let json: serde_json::Value =
            serde_json::from_str(&calculate_stats(1, 0, 0, 0, 0, 1)).expect("JSON valide");
        let s = &json["stats"];
        // golden_gk_n lv1 = [12, 13, 12, 10, 11, 9, 11].
        assert_eq!(s["kc"], 12);
        assert_eq!(s["it"], 11);
    }

    /// Position inexistante → stats 0 (parité TS, pas d'invention).
    #[test]
    fn calculate_stats_position_inconnue_zero() {
        let json: serde_json::Value =
            serde_json::from_str(&calculate_stats(9, 0, 0, 0, 0, 50)).expect("JSON valide");
        assert_eq!(json["total"], 0);
    }

    #[test]
    fn single_stat_bornes() {
        // lv≤1 → stat_lv1 ; lv≥99 → stat_lv99 ; lv30 exact → stat_lv30.
        assert_eq!(single_stat(1, 10, 30, 50, 80), 10);
        assert_eq!(single_stat(99, 10, 30, 50, 80), 80);
        assert_eq!(single_stat(30, 10, 30, 50, 80), 30);
    }

    #[test]
    fn rarity_rank_mapping() {
        assert_eq!(rarity_to_growth_rank(0), 0);
        assert_eq!(rarity_to_growth_rank(5), 5);
        assert_eq!(rarity_to_growth_rank(20), 5); // BASARA → UR.
    }

    // -----------------------------------------------------------------------
    // nie-core — FSM de match
    // -----------------------------------------------------------------------

    #[test]
    fn match_tick_normal_waittimer_to_transition() {
        // Match normal : WaitTimer → Transition (état 5).
        let json: serde_json::Value =
            serde_json::from_str(&match_tick("WaitTimer", false, 0).expect("ok"))
                .expect("JSON valide");
        assert_eq!(json["next"], "Transition");
        assert_eq!(json["immediate"], false);
    }

    #[test]
    fn match_tick_training_waittimer_to_resultui() {
        // Entraînement : WaitTimer → ResultUi (état 2).
        let json: serde_json::Value =
            serde_json::from_str(&match_tick("WaitTimer", true, 0).expect("ok"))
                .expect("JSON valide");
        assert_eq!(json["next"], "ResultUi");
    }

    #[test]
    fn match_tick_training_completion_immediate() {
        // Entraînement + end_counter==2 (Transition) → LoadNext, transition immédiate.
        let json: serde_json::Value =
            serde_json::from_str(&match_tick("Transition", true, 2).expect("ok"))
                .expect("JSON valide");
        assert_eq!(json["next"], "LoadNext");
        assert_eq!(json["immediate"], true);
    }

    #[test]
    fn match_tick_accepte_index_numerique() {
        // "1" == WaitTimer.
        let json: serde_json::Value =
            serde_json::from_str(&match_tick("1", false, 0).expect("ok")).expect("JSON valide");
        assert_eq!(json["next"], "Transition");
    }

    #[test]
    fn match_tick_etat_inconnu_erreur() {
        assert!(match_tick("Pizza", false, 0).is_err());
    }

    #[test]
    fn final_score_golden() {
        // 2 min 30 s = 20030 (golden FSM).
        assert_eq!(final_score(2, 30), 20030);
        assert_eq!(final_score(0, 0), 0);
    }

    // -----------------------------------------------------------------------
    // nie-data — lookup skill / aura / item
    // -----------------------------------------------------------------------

    /// Construit un `skill_config.cfg.bin.json` minimal (`lists`) avec la 1re valeur
    /// RÉELLE vérifiée (whs00010, « Trampoline du tonnerre », skillID 0x63BDA8A4,
    /// element=1 Vent, category=1 Tir, power 70→440). Source : `nie_data::skill`.
    fn skill_config_fixture() -> String {
        serde_json::json!({
            "version": 4,
            "lists": [{
                "name": "m_skillInfoList",
                "typeName": "SkillInfo",
                "values": [{
                    "skillID": "0x63BDA8A4",
                    "skillIDStr": "whs00010",
                    "skillNameId": "0x11111111",
                    "skillDescId": "0x22222222",
                    "power_min": 70,
                    "power_max": 440,
                    "element": 1,
                    "category": 1,
                    "consumeTp": 70,
                    "recastTime": 90,
                    "partnerType": 2,
                    "partner1": "0xAB97A3D2"
                }]
            }]
        })
        .to_string()
    }

    /// `skill_text.cfg.bin.json` minimal joignant le nom via NOUN_INFO (var0=hash, var5=nom).
    fn skill_text_fixture() -> String {
        serde_json::json!({
            "entries": [{
                "name": "NOUN_INFO_BEGIN",
                "variables": [],
                "children": [{
                    "name": "NOUN_INFO_0",
                    // var0 = hash décimal (les variables CfgBin brutes sont des entiers
                    // signés, pas des chaînes hex). 286331153 == 0x11111111 == skillNameId.
                    "variables": [
                        {"type": "Int", "value": "286331153"},
                        {"type": "Int", "value": "0"},
                        {"type": "String", "value": "fallback"},
                        {"type": "Int", "value": "0"},
                        {"type": "Int", "value": "0"},
                        {"type": "String", "value": "Trampoline du tonnerre"}
                    ],
                    "children": []
                }]
            }]
        })
        .to_string()
    }

    #[test]
    fn skill_lookup_resout_nom_et_element() {
        let out = skill_lookup(&skill_config_fixture(), &skill_text_fixture()).expect("ok");
        let json: serde_json::Value = serde_json::from_str(&out).expect("JSON valide");
        assert_eq!(json["count"], 1);
        let s = &json["skills"][0];
        assert_eq!(s["skillId"], "0x63BDA8A4");
        assert_eq!(s["skillIdStr"], "whs00010");
        assert_eq!(s["name"], "Trampoline du tonnerre");
        // element=1 → Wind ; category=1 → Shoot (enums nie-data).
        assert_eq!(s["element"], "Wind");
        assert_eq!(s["category"], "Shoot");
        assert_eq!(s["powerMin"], 70);
        assert_eq!(s["powerMax"], 440);
    }

    #[test]
    fn skill_lookup_sans_text_pas_de_nom() {
        // skill_text vide → name == null (pas d'invention).
        let out = skill_lookup(&skill_config_fixture(), "").expect("ok");
        let json: serde_json::Value = serde_json::from_str(&out).expect("JSON valide");
        assert!(json["skills"][0]["name"].is_null());
    }

    #[test]
    fn skill_lookup_json_invalide_erreur() {
        assert!(skill_lookup("{pas du json", "").is_err());
    }

    /// `aura_skill_config.cfg.bin.json` minimal avec le noeud RÉEL `AURA_CMD_INFO_0`
    /// (assetCode wks00020, element var8=3 Feu, sub_type Keshin). Source : `nie_data::aura`.
    fn aura_config_fixture() -> String {
        // 19 variables, ordre du dump vérifié.
        let vars: Vec<serde_json::Value> = [
            "2037965306",
            "wks00020",
            "493403631",
            "-1653680409",
            "30",
            "60",
            "260858381",
            "-1368456794",
            "3",
            "8",
            "0",
            "1",
            "-1124324279",
            "0",
            "0",
            "0",
            "1",
            "0",
            "0",
        ]
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let ty = if i == 1 { "String" } else { "Int" };
            serde_json::json!({"type": ty, "value": v})
        })
        .collect();

        serde_json::json!({
            "entries": [{
                "name": "AURA_CMD_INFO_0",
                "variables": vars,
                "children": []
            }]
        })
        .to_string()
    }

    #[test]
    fn aura_lookup_subtype_et_element() {
        // Sans skill_config → hissatsu == null (le skillId1 ne résout vers rien, comme le TS).
        let out = aura_lookup(&aura_config_fixture(), "").expect("ok");
        let json: serde_json::Value = serde_json::from_str(&out).expect("JSON valide");
        assert_eq!(json["count"], 1);
        let a = &json["auras"][0];
        assert_eq!(a["auraId"], "0x7978E1FA");
        assert_eq!(a["assetCode"], "wks00020");
        // préfixe wks → Keshin ; element var8=3 → Fire.
        assert_eq!(a["subType"], "Keshin");
        assert_eq!(a["subTypeLabel"], "Esprit Guerrier");
        assert_eq!(a["element"], "Fire");
        assert!(a["hissatsu"].is_null());
    }

    #[test]
    fn aura_lookup_resout_hissatsu_via_skill_config() {
        // skill_config où skillID == config.skillId1 de l'aura (var6 = 260858381 = 0x0F8C620D).
        let skill_config = serde_json::json!({
            "version": 4,
            "lists": [{
                "name": "m_skillInfoList",
                "values": [{
                    "skillID": "260858381",
                    "skillIDStr": "wks00020_hit",
                    "power_min": 100,
                    "power_max": 640,
                    "element": 3,
                    "category": 1
                }]
            }]
        })
        .to_string();

        let out = aura_lookup(&aura_config_fixture(), &skill_config).expect("ok");
        let json: serde_json::Value = serde_json::from_str(&out).expect("JSON valide");
        let h = &json["auras"][0]["hissatsu"];
        assert!(
            !h.is_null(),
            "skillId1 doit résoudre vers le skill_config fourni"
        );
        // `AuraHissatsu` est sérialisé tel quel par serde → clés snake_case.
        assert_eq!(h["skill_id_str"], "wks00020_hit");
        assert_eq!(h["element"], "Fire");
        assert_eq!(h["power"][0], 100);
        assert_eq!(h["power"][1], 640);
    }

    /// `item_config.cfg.bin.json` minimal avec le noeud RÉEL `ITEM_SHOES_INFO_0`
    /// (itemId 0x6D5D11A0, price 1401, stats 30/31, internalCode eq_sh110001).
    fn item_config_fixture() -> String {
        let raw = [
            "1834815904",
            "0",
            "1853054332",
            "0",
            "1401",
            "30",
            "31",
            "999",
            "0",
            "0",
            "0",
            "eq_sh110001",
            "1",
            "0",
            "0",
            "224",
            "0",
            "0",
            "961180446",
        ];
        let vars: Vec<serde_json::Value> = raw
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let ty = if i == 11 { "String" } else { "Int" };
                serde_json::json!({"type": ty, "value": v})
            })
            .collect();

        serde_json::json!({
            "entries": [{
                "name": "ITEM_SHOES_INFO_0",
                "variables": vars,
                "children": []
            }]
        })
        .to_string()
    }

    #[test]
    fn item_lookup_shoes() {
        let out = item_lookup(&item_config_fixture()).expect("ok");
        let json: serde_json::Value = serde_json::from_str(&out).expect("JSON valide");
        assert_eq!(json["count"], 1);
        let it = &json["items"][0];
        assert_eq!(it["itemId"], "0x6D5D11A0");
        assert_eq!(it["category"], "shoes");
        assert_eq!(it["price"], 1401);
        assert_eq!(it["stats"]["stat1"], 30);
        assert_eq!(it["stats"]["stat2"], 31);
        assert_eq!(it["internalCode"], "eq_sh110001");
    }

    #[test]
    fn item_lookup_json_invalide_erreur() {
        assert!(item_lookup("nope").is_err());
    }

    // -----------------------------------------------------------------------
    // nie-save — parse_save_json
    // -----------------------------------------------------------------------

    /// Construit un conteneur Lives synthétique minimal (1 blob HEADERSAVE),
    /// chiffre avec la clé CRC32 du nom, puis vérifie que parse_save_json
    /// retourne le bon slot_name et les bonnes métadonnées de blob.
    #[test]
    fn parse_save_json_conteneur_minimal() {
        use nie_save::{
            BLOB_MAGIC, BLOB_SUBTYPE_HEADERSAVE, Blob, BlobHeader, BlobSubtype, DATA_START,
            DIR_OFFSET, LIVES_CONST2, LIVES_MAGIC, crc32_of_pub, decrypt_block, key_from_filename,
        };

        let slot = "DEADBEEF-USERDATALIVE";

        // Corps minimal du blob HEADERSAVE (1 octet pour passer les bornes)
        let body = vec![0u8; 4];
        let blob = Blob {
            header: BlobHeader {
                subtype: BlobSubtype::Headersave,
                payload_size: body.len() as u32,
                field8: 0xABCD_1234,
            },
            body,
        };
        let blob_bytes = {
            let mut v = Vec::new();
            v.extend_from_slice(&BLOB_MAGIC.to_be_bytes());
            v.extend_from_slice(&BLOB_SUBTYPE_HEADERSAVE.to_be_bytes());
            v.extend_from_slice(&(blob.body.len() as u32).to_le_bytes());
            v.extend_from_slice(&blob.header.field8.to_le_bytes());
            v.extend_from_slice(&blob.body);
            v
        };
        let blob_crc = crc32_of_pub(&blob_bytes);

        // Construire le header plaintext (0x800)
        let mut hdr = vec![0u8; DATA_START];
        hdr[8..12].copy_from_slice(&LIVES_CONST2.to_le_bytes());
        let sn = slot.as_bytes();
        hdr[0x10..0x10 + sn.len()].copy_from_slice(sn);
        hdr[DIR_OFFSET..DIR_OFFSET + 4].copy_from_slice(&blob_crc.to_le_bytes());
        hdr[DIR_OFFSET + 4..DIR_OFFSET + 8]
            .copy_from_slice(&(blob_bytes.len() as u32).to_le_bytes());
        hdr[DIR_OFFSET + 8..DIR_OFFSET + 12].copy_from_slice(&0u32.to_le_bytes());
        let fname = b"HEADERSAVE_data.bin";
        hdr[DIR_OFFSET + 12..DIR_OFFSET + 12 + fname.len()].copy_from_slice(fname);
        let hdr_crc = crc32_of_pub(&hdr[8..DATA_START]);
        hdr[4..8].copy_from_slice(&hdr_crc.to_le_bytes());
        hdr[0..4].copy_from_slice(&LIVES_MAGIC.to_le_bytes());

        let mut plain = hdr;
        plain.extend_from_slice(&blob_bytes);

        let key = key_from_filename(slot);
        let mut enc = plain.clone();
        decrypt_block(&mut enc, 0, key);

        let json_str = parse_save_json(&enc, slot).expect("parse_save_json ne doit pas échouer");
        let json: serde_json::Value = serde_json::from_str(&json_str).expect("JSON valide");

        assert_eq!(json["slot_name"], slot);
        assert_eq!(json["key"], key);

        let blobs = json["blobs"].as_array().expect("blobs est un tableau");
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0]["filename"], "HEADERSAVE_data.bin");
        assert_eq!(blobs[0]["subtype"], "Headersave");
        assert_eq!(blobs[0]["field8"], 0xABCD_1234u32);

        // Le blob HEADERSAVE body fait 4 octets (trop court pour parse_headersave) →
        // headersave doit être null (pas d'erreur fatale).
        assert!(
            json["headersave"].is_null(),
            "headersave null sur body trop court"
        );
        // Pas de blob AUTOSAVE → autosave null.
        assert!(json["autosave"].is_null());
    }

    /// Vérifie que parse_save_json retourne une erreur sur un fichier vide.
    #[test]
    fn parse_save_json_tampon_vide_erreur() {
        assert!(parse_save_json(&[], "TEST-LIVE").is_err());
    }

    /// Vérifie que parse_save_json retourne une erreur sur une mauvaise clé.
    #[test]
    fn parse_save_json_mauvaise_cle_erreur() {
        // On encode avec "DEADBEEF-USERDATALIVE" mais on parse avec un autre nom.
        use nie_save::{
            DATA_START, LIVES_CONST2, LIVES_MAGIC, crc32_of_pub, decrypt_block, key_from_filename,
        };
        let slot = "DEADBEEF-USERDATALIVE";
        let mut hdr = vec![0u8; DATA_START];
        hdr[8..12].copy_from_slice(&LIVES_CONST2.to_le_bytes());
        let hdr_crc = crc32_of_pub(&hdr[8..DATA_START]);
        hdr[4..8].copy_from_slice(&hdr_crc.to_le_bytes());
        hdr[0..4].copy_from_slice(&LIVES_MAGIC.to_le_bytes());
        let key = key_from_filename(slot);
        let mut enc = hdr;
        decrypt_block(&mut enc, 0, key);
        // Parser avec un nom différent → mauvaise clé → BadMagic.
        assert!(parse_save_json(&enc, "CAFEBABE-LIVE").is_err());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests_sprite_sheet {
    /// Golden VFS : la feuille de sprites d'un atlas réel du jeu doit sortir de la FFI wasm avec
    /// ses régions et leurs rectangles — c'est ce que l'explorateur et le web consomment.
    #[test]
    fn feuille_de_sprites_d_un_atlas_reel() {
        use std::path::Path;

        let dir = nie_formats::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = Path::new(&dir).join("data");
        let mut vfs = nie_formats::vfs::Vfs::new();
        if vfs.init(&data_dir).is_err() {
            eprintln!(
                "skip feuille_de_sprites : jeu absent à {}",
                data_dir.display()
            );
            return;
        }
        let Some(chemin) = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .find(|p| p.ends_with("font/gaiji_game.g4tx"))
        else {
            eprintln!("skip feuille_de_sprites : gaiji_game.g4tx absent du VFS");
            return;
        };
        let data = vfs.read(&chemin).expect("lecture de l'atlas");

        let json = super::g4tx_sprite_sheet_json(&data).expect("feuille de sprites");
        let v: serde_json::Value = serde_json::from_str(&json).expect("JSON valide");
        let sprites = v["sprites"].as_array().expect("tableau de sprites");
        eprintln!("{chemin} : {} régions", sprites.len());

        assert!(
            sprites.len() > 100,
            "atlas d'icônes attendu, {} régions",
            sprites.len()
        );
        assert!(v["largeur"].as_i64().unwrap_or(0) > 0);
        // Chaque région porte un rectangle exploitable : c'est ce que `g4tx_info_json` ne donne pas.
        for s in sprites {
            assert!(!s["nom"].as_str().unwrap_or("").is_empty());
            assert!(
                s["largeur"].as_i64().unwrap_or(0) > 0,
                "region sans largeur : {s}"
            );
        }
    }
}
