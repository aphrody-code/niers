//! `nie-lua` — la VM Lua **réelle** du jeu (mlua, PUC-Rio Lua 5.2.4 vendored).
//!
//! ## Modules
//!
//! - Ce fichier (`lib.rs`) — VM, chargement bytecode, découverte API hôte.
//! - [`menu_host`] — modèle [`menu_host::MenuState`], [`menu_host::install_menu_host`],
//!   [`menu_host::run_menu`].
//! - [`static_analysis`] — analyse statique des **sources** Lua (tree-sitter) : structure
//!   d'un script décompilé sans l'exécuter.
//!
//! Le moteur Level-5 « Lives » pilote ses menus, scènes et événements par des scripts
//! Lua 5.2 compilés en bytecode (`.lua.bin`, ~616 fichiers sous `data/common/script/lua/`).
//! Reproduire le jeu À L'IDENTIQUE impose d'exécuter CES scripts dans LEUR VM exacte —
//! pas une réinterprétation. mlua avec `lua52` + `vendored` embarque PUC-Rio Lua 5.2.4,
//! la même implémentation que le jeu, et charge le **bytecode** directement.
//!
//! (iecode, faute de VM 5.2 native en C#, décompile via `unluac` puis réinterprète sous
//! MoonSharp — chemin lossy ; ici on exécute le bytecode d'origine.)
//!
//! ## `unsafe`
//!
//! Charger du bytecode Lua arbitraire exige `Lua::unsafe_new` (un bytecode malformé peut
//! corrompre la VM) : cette crate est donc volontairement hors `forbid(unsafe_code)`.

/// Mode de chargement d'un chunk, réexporté de `mlua` : les consommateurs (nie-explorer) n'ont
/// pas à déclarer `mlua` en dépendance directe juste pour nommer `Binary`/`Text`.
#[cfg(feature = "vm")]
pub use mlua::ChunkMode;

pub mod bytecode;
#[cfg(feature = "vm")]
pub mod host;
#[cfg(feature = "vm")]
pub mod runtime;
#[cfg(feature = "vm")]
pub mod session;
#[cfg(feature = "vm")]
pub use session::RuntimeContext;
/// Analyse statique des **sources** Lua (tree-sitter) — feature `analysis`, active par défaut.
///
/// Elle est optionnelle parce qu'elle tire `tree-sitter` (code C) : `nie-formats`, qui n'a
/// besoin que du décodeur [`bytecode`] pour brancher les `.lua.bin` sur `decode`, la coupe et
/// reste ainsi en Rust pur.
#[cfg(feature = "analysis")]
pub mod static_analysis;
#[cfg(feature = "analysis")]
pub use static_analysis::{
    FunctionKind, LuaAnalysis, LuaAssignment, LuaCall, LuaFunction, LuaSyntaxError, LuaTable,
    LuaTableField, StaticAnalysisError, SyntaxErrorKind, ValueKind, analyze, analyze_dir,
    analyze_file, collect_lua_files,
};
#[cfg(feature = "vm")]
pub mod menu_host;
#[cfg(feature = "vm")]
pub use menu_host::{
    DriveReport, HeaderTab, MenuLayerState, MenuListItem, MenuObjectState, MenuState, drive_menu,
    drive_menu_for_frames, enumerate_header_tabs, install_menu_host, run_menu,
};

#[cfg(feature = "vm")]
use thiserror::Error;

/// Signature d'un chunk de bytecode Lua 5.2 PUC-Rio : `1B 4C 75 61` (`\x1bLua`) + `0x52`.
pub const LUA52_BYTECODE_SIGNATURE: [u8; 5] = [0x1B, 0x4C, 0x75, 0x61, 0x52];

#[cfg(feature = "vm")]
/// Erreurs de chargement/exécution d'un script du jeu.
#[derive(Debug, Error)]
pub enum LuaError {
    /// Le tampon ne commence pas par la signature bytecode Lua 5.2.
    #[error("pas un bytecode Lua 5.2 (signature {0:02x?} attendue)")]
    NotLua52Bytecode([u8; 5]),
    /// Erreur remontée par la VM mlua (chargement ou exécution).
    #[error("erreur VM Lua : {0}")]
    Vm(#[from] mlua::Error),
    /// Le décodeur Rust refuse le conteneur avant de le remettre à la VM.
    ///
    /// Le chemin live doit mesurer et charger le même bytecode : laisser mlua accepter un chunk
    /// que notre décodeur ne sait pas lire rendrait les métriques d'audit trompeuses.
    #[error("erreur de décodage Lua : {0}")]
    Decode(#[from] bytecode::BytecodeError),
}

/// Vrai si `data` commence par la signature d'un bytecode Lua 5.2 PUC-Rio.
#[must_use]
pub fn is_lua52_bytecode(data: &[u8]) -> bool {
    data.len() >= 5 && data[..5] == LUA52_BYTECODE_SIGNATURE
}

#[cfg(feature = "vm")]
/// CRC32 IEEE utilisé par les scripts Level-5 (`CRC32("nom")`).
///
/// Même polynôme réfléchi et même finalisation que `nie-formats::cfgbin::crc32`, gardé ici pour
/// que la VM Lua puisse fournir ce global sans dépendre du crate de formats.
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(feature = "vm")]
/// Crée une VM Lua 5.2 capable de charger du bytecode (bibliothèques non sandboxées).
///
/// Note : `Lua::unsafe_new` est requis pour `ChunkMode::Binary`.
#[must_use]
pub fn new_vm() -> mlua::Lua {
    // SAFETY: on exécute du bytecode du jeu (de confiance) ; unsafe_new active le chargement
    // de chunks binaires, indispensable pour les .lua.bin.
    unsafe { mlua::Lua::unsafe_new() }
}

#[cfg(feature = "vm")]
/// **Charge** (compile) un chunk de bytecode `.lua.bin` du jeu dans `lua`, sans l'exécuter.
///
/// Prouve que la VM accepte le bytecode du jeu (= même implémentation Lua). Retourne la
/// fonction Lua compilée prête à être appelée (une fois les fonctions hôtes injectées).
///
/// # Errors
/// [`LuaError::NotLua52Bytecode`] si la signature est absente ; [`LuaError::Vm`] si mlua
/// refuse le bytecode (version/format incompatibles).
pub fn load_bytecode(lua: &mlua::Lua, data: &[u8], name: &str) -> Result<mlua::Function, LuaError> {
    if !is_lua52_bytecode(data) {
        let mut sig = [0u8; 5];
        sig.copy_from_slice(&data[..5.min(data.len())]);
        return Err(LuaError::NotLua52Bytecode(sig));
    }
    // Valider avec le décodeur partagé avant le chargement VM : le runtime live et `lua-audit`
    // doivent parler du même chunk, y compris pour les includes imbriqués.
    bytecode::parse(data)?;
    let func = lua
        .load(data)
        .set_name(name)
        .set_mode(mlua::ChunkMode::Binary)
        .into_function()?;
    Ok(func)
}

#[cfg(feature = "vm")]
/// Installe la fonction hôte `INCLUDE(name)` — le **système de modules** du moteur : le
/// script appelle `INCLUDE("…")`, l'hôte résout le nom en bytecode `.lua.bin` (via `resolver`)
/// et l'exécute dans la MÊME VM. Renvoie les valeurs du module inclus (ou rien si introuvable,
/// comportement aligné sur iecode `DefaultLuaHost`).
///
/// `resolver` : nom logique d'include → bytecode du module (typiquement adossé au VFS).
///
/// # Errors
/// [`mlua::Error`] si l'enregistrement de la fonction échoue.
pub fn install_include<F>(lua: &mlua::Lua, resolver: F) -> mlua::Result<()>
where
    F: Fn(&str) -> Option<Vec<u8>> + 'static,
{
    let f = lua.create_function(move |lua, name: String| {
        let Some(bytes) = resolver(&name) else {
            return Ok(mlua::MultiValue::new()); // introuvable → vide (comme iecode)
        };
        // Un module peut être du bytecode (.lua.bin) ou de la source ; on tente le bytecode.
        let mode = if is_lua52_bytecode(&bytes) {
            // Même garde que pour le chunk principal : un include binaire est décodé avant son
            // exécution dans la VM persistante. L'erreur est remontée comme erreur Lua de
            // callback, avec le nom logique pour rendre le défaut actionnable.
            if let Err(error) = crate::bytecode::parse(&bytes) {
                return Err(mlua::Error::RuntimeError(format!(
                    "décodage de l'include {name} : {error}"
                )));
            }
            mlua::ChunkMode::Binary
        } else {
            mlua::ChunkMode::Text
        };
        let func = lua
            .load(&bytes)
            .set_name(format!("@{name}"))
            .set_mode(mode)
            .into_function()?;
        func.call::<mlua::MultiValue>(())
    })?;
    lua.globals().set("INCLUDE", f)?;
    Ok(())
}

/// Convertit un **nom logique d'INCLUDE moteur** en base de fichier `.lua.bin`.
///
/// Les scripts du jeu appellent `INCLUDE("LUA_MAIN_MENU_INC")` avec un nom logique en
/// MAJUSCULES préfixé `LUA_`. Le moteur le résout en un fichier dont le basename (sans
/// suffixe de version) est ce nom en minuscules sans le préfixe : `LUA_MAIN_MENU_INC` →
/// `main_menu_inc` (fichier `main_menu_inc_3.00.01.00.lua.bin`). Vérifié sur les trois
/// includes du `main_menu` : `LUA_PROG_BASE`→`prog_base`, `LUA_MAIN_MENU_INC`→`main_menu_inc`,
/// `LUA_SOCCER_TOP_MENU_INC`→`soccer_top_menu_inc`.
///
/// Si le nom n'a pas le préfixe `LUA_`, il est simplement mis en minuscules.
#[must_use]
pub fn include_logical_base(include_name: &str) -> String {
    let lower = include_name.to_ascii_lowercase();
    lower.strip_prefix("lua_").unwrap_or(&lower).to_string()
}

/// Réduit le basename d'un script (`"main_menu_inc_3.00.01.00.lua.bin"`) à sa **base
/// logique versionless** (`"main_menu_inc"`), utilisée pour la résolution d'INCLUDE.
///
/// Retire l'extension `.lua.bin` puis, de façon répétée, tout segment de version final
/// `_<chiffres et points>` (ex. `_3.00.01.00`, `_0.06.33`). Les fichiers sans version
/// (`equip_medalset_inc.lua.bin`) sont renvoyés tels quels (base = `equip_medalset_inc`).
#[must_use]
pub fn script_logical_base(basename: &str) -> String {
    let mut s = basename.to_ascii_lowercase();
    for ext in [".lua.bin", ".lua"] {
        if let Some(stripped) = s.strip_suffix(ext) {
            s = stripped.to_string();
            break;
        }
    }
    // Retire les segments de version finaux `_<[0-9.]+>` (potentiellement plusieurs).
    while let Some(idx) = s.rfind('_') {
        let tail = &s[idx + 1..];
        if tail.is_empty() || !tail.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            break;
        }
        s.truncate(idx);
    }
    s
}

/// Extrait le suffixe de version numérique d'un chemin de script.
///
/// La résolution d'include doit comparer `10.00` après `9.00`, même si l'ordre ASCII inverse
/// les deux chaînes. Les composants sont comparés numériquement ; le chemin complet départage
/// uniquement deux fichiers portant exactement la même version.
fn script_version_key(path: &str) -> Vec<u64> {
    let basename = path.rsplit('/').next().unwrap_or(path);
    let without_ext = basename
        .strip_suffix(".lua.bin")
        .or_else(|| basename.strip_suffix(".lua"))
        .unwrap_or(basename);
    let Some((_, tail)) = without_ext.rsplit_once('_') else {
        return Vec::new();
    };
    if tail.is_empty()
        || !tail
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return Vec::new();
    }
    tail.split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

#[cfg(feature = "vm")]
/// Indexe les chemins de scripts du VFS par nom physique et par nom logique versionless.
///
/// Le jeu ne demande pas nécessairement le basename physique : ses scripts utilisent des noms
/// logiques (`LUA_MAIN_MENU_INC`) tandis que le VFS porte une version (`main_menu_inc_3.00.01.00.lua.bin`).
/// Le type VFS reste volontairement hors de cette crate ; l'appelant ne lui fournit que ses chemins.
#[cfg(feature = "vm")]
pub fn index_script_paths<'a, I>(
    paths: I,
) -> (
    std::collections::HashMap<String, String>,
    std::collections::HashMap<String, String>,
)
where
    I: IntoIterator<Item = &'a str>,
{
    let mut by_name = std::collections::HashMap::<String, String>::new();
    let mut by_logical = std::collections::HashMap::<String, String>::new();
    // `Vfs::iter()` n'impose pas d'ordre : trier avant de choisir évite qu'une exécution live
    // charge une version différente du même include selon l'état du HashMap sous-jacent.
    let mut paths: Vec<String> = paths.into_iter().map(str::to_string).collect();
    paths.sort_unstable();
    for path in paths {
        let Some(base) = path.rsplit('/').next().filter(|b| b.ends_with(".lua.bin")) else {
            continue;
        };
        by_name
            .entry(path.to_ascii_lowercase())
            .or_insert_with(|| path.clone());
        by_name
            .entry(base.to_ascii_lowercase())
            .or_insert_with(|| path.clone());
        by_logical
            .entry(script_logical_base(base))
            // Comparaison numérique : l'ordre ASCII (`10` < `9`) ne doit jamais choisir une
            // mauvaise version d'include en session live.
            .and_modify(|selected| {
                let candidate_key = script_version_key(&path);
                let selected_key = script_version_key(selected);
                if candidate_key > selected_key
                    || (candidate_key == selected_key && path > *selected)
                {
                    *selected = path.clone();
                }
            })
            .or_insert(path);
    }
    (by_name, by_logical)
}

/// Résout un nom `INCLUDE` dans les index produits par [`index_script_paths`].
#[cfg(feature = "vm")]
pub fn resolve_script_path<'a>(
    name: &str,
    by_name: &'a std::collections::HashMap<String, String>,
    by_logical: &'a std::collections::HashMap<String, String>,
) -> Option<&'a String> {
    let lower = name.to_ascii_lowercase();
    let basename = lower.rsplit('/').next().unwrap_or(&lower);
    let exact = format!("{basename}.lua.bin");
    by_name
        .get(&lower)
        .or_else(|| by_name.get(&exact))
        .or_else(|| by_name.get(basename))
        // Certains includes natifs portent un chemin relatif (`common/script/...`) : le
        // VFS ajoute `data/`, mais la sélection de version se fait bien sur le basename.
        .or_else(|| by_logical.get(&include_logical_base(basename)))
}

#[cfg(feature = "vm")]
/// Exécute un script `.lua.bin` du jeu dans une VM instrumentée et retourne la liste TRIÉE
/// des **globals hôtes** qu'il référence (fonctions/tables fournies par le moteur C++).
///
/// Technique : une métatable sur `_G` dont `__index` enregistre chaque accès à un global
/// indéfini et renvoie un stub appelable (qui renvoie lui-même un stub), afin que le script
/// s'exécute le plus loin possible sans planter. Donne la **surface d'API hôte** réelle à
/// implémenter pour faire tourner ce menu. Ne prétend pas exécuter la logique — c'est un
/// outil de bring-up moteur.
///
/// # Errors
/// [`LuaError`] si le bytecode est invalide ou si l'instrumentation échoue.
pub fn discover_host_calls(data: &[u8], name: &str) -> Result<Vec<String>, LuaError> {
    let lua = new_vm();
    // Instrumentation : enregistre tout global indéfini, renvoie un stub appelable récursif.
    lua.load(
        r#"
        _HOST_SEEN = {}
        local function stub() return setmetatable({}, { __call = function() return stub() end }) end
        setmetatable(_G, { __index = function(_, k)
            _HOST_SEEN[k] = (_HOST_SEEN[k] or 0) + 1
            return stub()
        end })
        "#,
    )
    .set_name("<host-recorder>")
    .exec()?;

    let func = load_bytecode(&lua, data, name)?;
    // pcall : on tolère une erreur d'exécution (stubs imparfaits) ; on veut juste la collecte.
    let _ = func.call::<()>(());

    let seen: mlua::Table = lua.globals().get("_HOST_SEEN")?;
    let mut names: Vec<String> = Vec::new();
    for pair in seen.pairs::<String, i64>() {
        let (k, _) = pair?;
        names.push(k);
    }
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La VM exécute du Lua 5.2 SOURCE (sanity de l'intégration mlua/PUC 5.2.4).
    #[test]
    fn vm_runs_source_lua52() {
        let lua = new_vm();
        let v: i64 = lua.load("local a=2 return a*21").eval().expect("eval");
        assert_eq!(v, 42);
        // bit32 = bibliothèque spécifique à Lua 5.2 (absente en 5.1/5.3) → confirme la version.
        let b: i64 = lua
            .load("return bit32.band(0xF0, 0x3C)")
            .eval()
            .expect("bit32");
        assert_eq!(b, 0x30);
    }

    /// `is_lua52_bytecode` reconnaît la signature.
    #[test]
    fn detects_bytecode_signature() {
        assert!(is_lua52_bytecode(&[0x1B, 0x4C, 0x75, 0x61, 0x52, 0x00]));
        assert!(!is_lua52_bytecode(b"-- source lua"));
        assert!(!is_lua52_bytecode(&[0x1B, 0x4C, 0x75, 0x61, 0x51])); // 5.1
    }

    /// Les noms logiques d'INCLUDE moteur se réduisent à la base de fichier minuscule
    /// (préfixe `LUA_` retiré) — vérité terrain des includes du `main_menu`.
    #[test]
    fn include_logical_base_strips_lua_prefix() {
        assert_eq!(include_logical_base("LUA_MAIN_MENU_INC"), "main_menu_inc");
        assert_eq!(include_logical_base("LUA_PROG_BASE"), "prog_base");
        assert_eq!(
            include_logical_base("LUA_SOCCER_TOP_MENU_INC"),
            "soccer_top_menu_inc"
        );
        // Sans préfixe : simple minuscule.
        assert_eq!(include_logical_base("menu_def"), "menu_def");
    }

    /// La base logique d'un basename retire l'extension `.lua.bin` et le suffixe de version.
    #[test]
    fn script_logical_base_strips_version_suffix() {
        assert_eq!(
            script_logical_base("main_menu_inc_3.00.01.00.lua.bin"),
            "main_menu_inc"
        );
        assert_eq!(
            script_logical_base("prog_base_0.00.00.00.lua.bin"),
            "prog_base"
        );
        assert_eq!(
            script_logical_base("soccer_top_menu_inc_1.04.19.01.lua.bin"),
            "soccer_top_menu_inc"
        );
        // Sans suffixe de version : inchangé.
        assert_eq!(
            script_logical_base("equip_medalset_inc.lua.bin"),
            "equip_medalset_inc"
        );
        // Boucle d'INCLUDE résout le logique du moteur vers le fichier réel.
        assert_eq!(
            script_logical_base("main_menu_inc_3.00.01.00.lua.bin"),
            include_logical_base("LUA_MAIN_MENU_INC")
        );
    }

    #[test]
    fn script_index_resolves_versioned_logical_include() {
        let paths = [
            "data/common/script/lua/menu/main_menu_inc_3.00.01.00.lua.bin",
            "data/common/script/lua/menu/prog_base_0.00.00.00.lua.bin",
            "data/common/script/lua/menu/readme.txt",
        ];
        let (by_name, by_logical) = index_script_paths(paths);
        let expected = paths[0].to_string();
        assert_eq!(
            resolve_script_path("LUA_MAIN_MENU_INC", &by_name, &by_logical),
            Some(&expected)
        );
        assert_eq!(
            resolve_script_path("main_menu_inc_3.00.01.00.lua.bin", &by_name, &by_logical),
            Some(&expected)
        );
        assert_eq!(
            resolve_script_path(
                "common/script/lua/menu/main_menu_inc_3.00.01.00.lua.bin",
                &by_name,
                &by_logical,
            ),
            Some(&expected)
        );
        assert!(resolve_script_path("LUA_MISSING", &by_name, &by_logical).is_none());
    }

    #[test]
    fn script_index_compare_les_versions_numeriquement() {
        let paths = [
            "data/lua/foo_9.00.00.lua.bin",
            "data/lua/foo_10.00.00.lua.bin",
            "data/lua/bar_1.9.lua.bin",
            "data/lua/bar_1.10.lua.bin",
        ];
        let (_, by_logical) = index_script_paths(paths);
        assert_eq!(
            by_logical.get("foo").map(String::as_str),
            Some("data/lua/foo_10.00.00.lua.bin")
        );
        assert_eq!(
            by_logical.get("bar").map(String::as_str),
            Some("data/lua/bar_1.10.lua.bin")
        );
    }

    #[test]
    fn le_chargement_live_valide_le_chunk_et_ses_includes() {
        let lua = new_vm();
        let malformed = vec![0x1B, b'L', b'u', b'a', 0x52, 0x00];
        assert!(matches!(
            load_bytecode(&lua, &malformed, "principal"),
            Err(LuaError::Decode(_))
        ));

        install_include(&lua, move |name| {
            (name == "BAD").then(|| malformed.clone())
        })
        .expect("install include");
        let error = lua
            .load(r#"INCLUDE("BAD")"#)
            .exec()
            .expect_err("un include binaire invalide doit être refusé");
        assert!(error.to_string().contains("décodage de l'include BAD"));
    }

    /// **Bout-en-bout sur le vrai jeu** : charge un `.lua.bin` réel dans la VM 5.2.
    /// Prouve que mlua (PUC 5.2.4) accepte le bytecode du moteur — la fondation pour
    /// exécuter la logique réelle du jeu. Gated sur l'install Steam.
    #[test]
    fn loads_real_game_lua_bytecode() {
        let dir = nie_formats::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip loads_real_game_lua_bytecode : jeu absent");
            return;
        }
        let mut vfs = nie_formats::vfs::Vfs::new();
        if vfs.init(&data_dir).is_err() {
            eprintln!("skip : vfs.init KO");
            return;
        }
        let Some(path) = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .find(|p| p.ends_with(".lua.bin"))
        else {
            eprintln!("skip : aucun .lua.bin");
            return;
        };
        let bytes = vfs.read(&path).expect("read .lua.bin");
        eprintln!(
            "script={path} taille={} signature={:02x?}",
            bytes.len(),
            &bytes[..5.min(bytes.len())]
        );
        assert!(
            is_lua52_bytecode(&bytes),
            "le .lua.bin du jeu doit être un bytecode Lua 5.2 ; en-tête {:02x?}",
            &bytes[..8.min(bytes.len())]
        );
        let lua = new_vm();
        match load_bytecode(&lua, &bytes, &path) {
            Ok(_func) => eprintln!("OK : bytecode du jeu chargé dans la VM Lua 5.2 réelle (mlua)"),
            Err(e) => panic!("mlua refuse le bytecode du jeu : {e}"),
        }
    }

    /// Bring-up moteur : exécute de vrais scripts de menu et révèle la **surface d'API hôte**
    /// (fonctions du moteur C++ que les scripts appellent) à implémenter pour les faire tourner.
    #[test]
    fn discover_host_api_of_real_menus() {
        let dir = nie_formats::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip discover_host_api_of_real_menus : jeu absent");
            return;
        }
        let mut vfs = nie_formats::vfs::Vfs::new();
        if vfs.init(&data_dir).is_err() {
            eprintln!("skip : vfs.init KO");
            return;
        }
        // Quelques scripts de menu réels.
        let scripts: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| p.starts_with("data/common/script/lua/menu/") && p.ends_with(".lua.bin"))
            .take(5)
            .collect();
        if scripts.is_empty() {
            eprintln!("skip : aucun script de menu");
            return;
        }
        let mut union: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for path in &scripts {
            let Ok(bytes) = vfs.read(path) else { continue };
            match discover_host_calls(&bytes, path) {
                Ok(names) => {
                    eprintln!("\n{path}\n  → {} globals hôtes : {:?}", names.len(), names);
                    union.extend(names);
                }
                Err(e) => eprintln!("{path} : {e}"),
            }
        }
        eprintln!(
            "\n=== UNION API hôte sur {} menus ({} fonctions) ===\n{:#?}",
            scripts.len(),
            union.len(),
            union
        );
        assert!(
            !union.is_empty(),
            "les scripts doivent référencer des fonctions hôtes"
        );
    }

    /// **Milestone moteur** : exécute la vraie logique Lua de menus avec le host complet
    /// (`funcLuaMenuCommand` + `INCLUDE` VFS) et inspecte le [`MenuState`] résultant.
    ///
    /// Pour chaque script de menu sous `data/common/script/lua/menu/` :
    /// - charge + exécute (définis les callbacks)
    /// - appelle `OnSetupLayer` puis `OnOpenLayer` si présents
    /// - rapporte layers/objets créés et fonctions hôtes appelées
    ///
    /// Résultat honête : si aucun script ne peuple via OnOpenLayer, on rapporte
    /// exactement ce qu'ils ont appelé pour diagnostiquer la convention réelle.
    #[test]
    fn run_menu_with_menu_host() {
        use crate::menu_host::{install_menu_host, run_menu};
        use std::rc::Rc;

        let dir = nie_formats::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip run_menu_with_menu_host : jeu absent");
            return;
        }
        let mut vfs = nie_formats::vfs::Vfs::new();
        if vfs.init(&data_dir).is_err() {
            eprintln!("skip : vfs.init KO");
            return;
        }

        let vfs = Rc::new(vfs);
        let script_paths: Vec<String> = vfs.iter().map(|(p, _)| p.to_string()).collect();
        let (by_name, by_logical) = index_script_paths(script_paths.iter().map(String::as_str));
        let by_name = Rc::new(by_name);
        let by_logical = Rc::new(by_logical);

        // Sélectionner les scripts de menu — triés pour un ordre déterministe.
        let mut scripts: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| p.starts_with("data/common/script/lua/menu/") && p.ends_with(".lua.bin"))
            .collect();
        scripts.sort();
        scripts.truncate(30);

        if scripts.is_empty() {
            eprintln!("skip : aucun script de menu");
            return;
        }

        eprintln!(
            "\n=== run_menu_with_menu_host : {} scripts ===\n",
            scripts.len()
        );

        let mut found_populated = false;
        let mut total_unknown_cmds: std::collections::BTreeMap<u32, usize> =
            std::collections::BTreeMap::new();

        for path in &scripts {
            let Ok(bytes) = vfs.read(path) else { continue };

            // Nouvelle VM par script (état propre).
            let lua = new_vm();

            // Installe INCLUDE adossé au VFS.
            {
                let vfs = Rc::clone(&vfs);
                let by_name = Rc::clone(&by_name);
                let by_logical = Rc::clone(&by_logical);
                install_include(&lua, move |name| {
                    let path = resolve_script_path(name, &by_name, &by_logical)?;
                    vfs.read(path).ok()
                })
                .expect("install_include");
            }

            // Installe le host de menu.
            let state = match install_menu_host(&lua) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{path}: install_menu_host KO: {e}");
                    continue;
                }
            };

            // Détermine un layerId plausible : on essaie 0 (iecode utilise general_win
            // = 292844459 = 0x117473AB pour qrcode_menu ; sans le dico on essaie 0 puis ce hash).
            // Les scripts qui définissent OnOpenLayer(layerId) utiliseront souvent le
            // layerId passé pour filtrer ; on tente 0 = « tous les layers ».
            let layer_id: u32 = 0;

            let script_name = path.rsplit('/').next().unwrap_or(path.as_str());
            let run_result = run_menu(&lua, &bytes, path, layer_id);

            let st = state.borrow();
            let has_on_open = matches!(&run_result, Ok(true));
            let n_layers = st.layers.len();
            let n_objects: usize = st.layers.values().map(|l| l.objects.len()).sum();
            let n_known_calls = st.known_cmd_log.len();
            let n_unknown = st.unknown_cmd_log.len();

            // Accumule les cmdIds inconnus pour le rapport global.
            for (cmd_id, _, _) in &st.unknown_cmd_log {
                *total_unknown_cmds.entry(*cmd_id).or_insert(0) += 1;
            }

            eprintln!(
                "{script_name}\n  OnOpenLayer={has_on_open}  run={run_result:?}\n  \
                 layers={n_layers}  objects={n_objects}  known_calls={n_known_calls}  \
                 unknown_cmds={n_unknown}"
            );

            if !st.known_cmd_log.is_empty() {
                eprintln!("  commandes connues : {:?}", st.known_cmd_log);
            }
            if n_layers > 0 || n_objects > 0 {
                found_populated = true;
                eprintln!(
                    "  *** PEUPLÉ *** layers: {:?}",
                    st.layers.keys().collect::<Vec<_>>()
                );
                for (lid, layer) in &st.layers {
                    eprintln!(
                        "    layer 0x{lid:08X}: visible={} enabled={} focus={:?} objects={}",
                        layer.visible,
                        layer.enabled,
                        layer.focus,
                        layer.objects.len()
                    );
                    for (oid, obj) in &layer.objects {
                        eprintln!(
                            "      obj 0x{oid:08X}: visible={} sprite={:?} text={:?}",
                            obj.visible, obj.sprite_texture_hash, obj.text
                        );
                    }
                }
            }
            if !st.unknown_cmd_log.is_empty() {
                // Affiche les 5 premiers appels inconnus pour la découverte.
                eprintln!("  cmdIds inconnus (premiers 5) :");
                for (cmd_id, layer_id, repr) in st.unknown_cmd_log.iter().take(5) {
                    eprintln!("    0x{cmd_id:08X} layer=0x{layer_id:08X} args=[{repr}]");
                }
            }
        }

        // Rapport global des cmdIds inconnus (utile pour étendre le dispatch).
        if !total_unknown_cmds.is_empty() {
            eprintln!(
                "\n=== cmdIds funcLuaMenuCommand non reversés ({} distincts) ===",
                total_unknown_cmds.len()
            );
            for (cmd_id, count) in &total_unknown_cmds {
                eprintln!("  0x{cmd_id:08X}  ×{count}");
            }
        }

        eprintln!(
            "\n=== Bilan enquête : {} scripts — {} avec MenuState peuplé ===",
            scripts.len(),
            if found_populated { "≥1" } else { "0" }
        );

        // ── Assertion ciblée : loading_menu_trial_1.03.64.lua.bin ────────────
        // Ce script est CONFIRMÉ peupler le MenuState lors de l'exécution
        // principale (SetObjectVisible sur layer 0x738D7BFD, objet 0x00000000).
        // On le teste directement pour une assertion robuste.
        let loading_trial_path = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .find(|p| p.ends_with("loading_menu_trial_1.03.64.lua.bin"));

        if let Some(ref path) = loading_trial_path {
            let bytes = vfs.read(path).expect("read loading_menu_trial");
            let lua_t = new_vm();
            {
                let vfs = Rc::clone(&vfs);
                let by_name = Rc::clone(&by_name);
                let by_logical = Rc::clone(&by_logical);
                install_include(&lua_t, move |name| {
                    resolve_script_path(name, &by_name, &by_logical).and_then(|p| vfs.read(p).ok())
                })
                .expect("install_include");
            }
            let state_t = install_menu_host(&lua_t).expect("install_menu_host");
            let _ = run_menu(&lua_t, &bytes, path, 0);
            let st = state_t.borrow();

            eprintln!(
                "\n=== Assertion loading_menu_trial : layers={} objects={} known_calls={} ===",
                st.layers.len(),
                st.layers.values().map(|l| l.objects.len()).sum::<usize>(),
                st.known_cmd_log.len()
            );
            for (lid, layer) in &st.layers {
                eprintln!("  layer 0x{lid:08X}: {} objets", layer.objects.len());
                for (oid, obj) in &layer.objects {
                    eprintln!(
                        "    obj 0x{oid:08X}: visible={} sprite={:?}",
                        obj.visible, obj.sprite_texture_hash
                    );
                }
            }

            // Assertion forte : le script a peuplé ≥1 layer avec ≥1 objet.
            assert!(
                !st.layers.is_empty(),
                "loading_menu_trial doit créer ≥1 layer dans le MenuState"
            );
            let total_objects: usize = st.layers.values().map(|l| l.objects.len()).sum();
            assert!(
                total_objects > 0,
                "loading_menu_trial doit créer ≥1 objet dans le MenuState"
            );
            assert!(
                !st.known_cmd_log.is_empty(),
                "loading_menu_trial doit avoir appelé ≥1 commande hôte connue"
            );
        } else {
            eprintln!("SKIP assertion ciblée : loading_menu_trial_1.03.64.lua.bin introuvable");
        }
    }

    /// Golden du scénario `savedata_management_menu_save_and_upload` — les layerIds et cmdIds
    /// que les tests C# (`LuaRuntimeTests.cs`) affirmaient, rejoués ici sur le **vrai** script du
    /// jeu au lieu d'un décompilé produit par `unluac.jar`.
    ///
    /// Le C# dépendait de `re/lua/raw` + `unluac.jar`, absents du dépôt : ses assertions ne
    /// s'exécutaient jamais ici. Celle-ci s'exécute.
    #[test]
    fn golden_scenario_savedata_emet_ses_cmd_ids() {
        use crate::menu_host::{install_menu_host, run_menu};
        use std::rc::Rc;

        /// layerId passé à `OnOpenLayer` (constante observée dans le décompilé).
        const LAYER_OUVERTURE: u32 = 536_044_352;
        /// layerId passé à `OnChangeLayerGroup` — c'est `CRC32` du nom du script.
        const LAYER_GROUPE: u32 = 1_654_568_798;
        /// `SetObjectVisible`, reversé.
        const CMD_SET_OBJECT_VISIBLE: u32 = 711_242_136;
        /// Émis par `OnChangeLayerGroup`, handler non reversé à ce jour.
        const CMD_NON_REVERSE: u32 = 532_421_851;

        // Le layerId de groupe est vérifiable sans dictionnaire de hashes.
        assert_eq!(
            nie_formats::cfgbin::crc32(b"savedata_management_menu_save_and_upload"),
            LAYER_GROUPE
        );

        let dir = nie_formats::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        let mut vfs = nie_formats::vfs::Vfs::new();
        if vfs.init(&data_dir).is_err() {
            eprintln!(
                "skip golden_scenario_savedata : jeu absent à {}",
                data_dir.display()
            );
            return;
        }

        let Some(path) = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .find(|p| p.ends_with("savedata_management_menu_save_and_upload.lua.bin"))
        else {
            eprintln!("skip golden_scenario_savedata : script absent du VFS");
            return;
        };
        let bytes = vfs.read(&path).expect("lecture du script");

        // Index basename → chemin, pour l'INCLUDE réel.
        let mut by_base: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for (p, _) in vfs.iter() {
            if let Some(b) = p.rsplit('/').next()
                && b.ends_with(".lua.bin")
            {
                by_base
                    .entry(b.to_ascii_lowercase())
                    .or_insert_with(|| p.to_string());
            }
        }
        let vfs = Rc::new(vfs);
        let by_base = Rc::new(by_base);

        let lua = new_vm();
        {
            let vfs = Rc::clone(&vfs);
            let by_base = Rc::clone(&by_base);
            install_include(&lua, move |name| {
                let c = format!("{}.lua.bin", name.to_ascii_lowercase());
                by_base.get(&c).and_then(|p| vfs.read(p).ok())
            })
            .expect("install_include");
        }
        let state = install_menu_host(&lua).expect("install_menu_host");

        // OnSetupLayer + OnOpenLayer, puis OnChangeLayerGroup — la séquence des tests C#.
        let _ = run_menu(&lua, &bytes, &path, LAYER_OUVERTURE);
        if let Ok(mlua::Value::Function(f)) = lua.globals().get::<mlua::Value>("OnChangeLayerGroup")
        {
            let _ = f.call::<mlua::MultiValue>(f64::from(LAYER_GROUPE));
        }

        let st = state.borrow();
        // Tous les cmdId émis, reversés (par nom) comme inconnus (par id).
        let emis: Vec<u32> = st.unknown_cmd_log.iter().map(|(c, _, _)| *c).collect();
        eprintln!(
            "{path}\n  connus={} inconnus={} layers={}",
            st.known_cmd_log.len(),
            emis.len(),
            st.layers.len()
        );
        for (nom, lid) in st.known_cmd_log.iter().take(10) {
            eprintln!("    connu   {nom} layer=0x{lid:08X}");
        }
        for (cid, lid, args) in st.unknown_cmd_log.iter().take(10) {
            eprintln!("    inconnu 0x{cid:08X} layer=0x{lid:08X} args=[{args}]");
        }

        // Les deux commandes que le C# attendait de ce scénario sont bien émises — l'une nommée
        // (reversée), l'autre par son id (pas encore reversée).
        assert!(
            st.known_cmd_log
                .iter()
                .any(|(n, _)| n == "SetObjectVisible"),
            "0x{CMD_SET_OBJECT_VISIBLE:08X} SetObjectVisible doit être émis par ce scénario"
        );
        assert!(
            !emis.contains(&CMD_SET_OBJECT_VISIBLE),
            "0x{CMD_SET_OBJECT_VISIBLE:08X} est reversé : il ne doit jamais tomber dans unknown_cmd_log"
        );
        // Le non-reversé, lui, tombe dans le log des inconnus : c'est la trace du travail restant.
        // Le jour où son handler sera reversé, cette assertion doit basculer vers `known_cmd_log`.
        assert!(
            emis.contains(&CMD_NON_REVERSE),
            "0x{CMD_NON_REVERSE:08X} doit être émis par OnChangeLayerGroup, émis : {emis:08X?}"
        );
    }

    /// Bring-up moteur, couche 2 : avec un VRAI `INCLUDE` adossé au VFS, un script de menu
    /// charge ses modules → révèle l'API hôte PLUS PROFONDE (ce que les modules appellent).
    #[test]
    fn run_menu_with_real_include() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let dir = nie_formats::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip run_menu_with_real_include : jeu absent");
            return;
        }
        let mut vfs = nie_formats::vfs::Vfs::new();
        if vfs.init(&data_dir).is_err() {
            eprintln!("skip : vfs.init KO");
            return;
        }
        let vfs = Rc::new(vfs);
        let script_paths: Vec<String> = vfs.iter().map(|(p, _)| p.to_string()).collect();
        let (by_name, by_logical) = index_script_paths(script_paths.iter().map(String::as_str));
        let requested: Rc<RefCell<Vec<(String, bool)>>> = Rc::new(RefCell::new(Vec::new()));

        // Choisir un script de menu réel.
        let Some(top) = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .find(|p| p.starts_with("data/common/script/lua/menu/") && p.ends_with(".lua.bin"))
        else {
            eprintln!("skip : aucun script menu");
            return;
        };
        let top_bytes = vfs.read(&top).expect("read top");

        let lua = new_vm();
        {
            let requested = Rc::clone(&requested);
            let vfs = Rc::clone(&vfs);
            install_include(&lua, move |name| {
                let found = resolve_script_path(name, &by_name, &by_logical)
                    .and_then(|path| vfs.read(path).ok());
                requested
                    .borrow_mut()
                    .push((name.to_string(), found.is_some()));
                found
            })
            .expect("install INCLUDE");
        }
        // Recorder pour les AUTRES globals hôtes (INCLUDE est déjà réel).
        lua.load(
            r#"_HOST_SEEN={}
               local function stub() return setmetatable({},{__call=function() return stub() end}) end
               setmetatable(_G,{__index=function(_,k) _HOST_SEEN[k]=(_HOST_SEEN[k] or 0)+1; return stub() end})"#,
        )
        .exec()
        .expect("recorder");

        let func = load_bytecode(&lua, &top_bytes, &top).expect("load top");
        let run = func.call::<()>(());
        eprintln!("\nscript={top}\n exécution : {run:?}");
        // Jalon : le bytecode RÉEL du jeu s'EXÉCUTE dans la VM (au-delà du simple chargement).
        assert!(
            run.is_ok(),
            "le script du jeu doit s'exécuter sans erreur VM : {run:?}"
        );
        eprintln!(" includes demandés :");
        for (n, ok) in requested.borrow().iter() {
            eprintln!("   - {n}  [{}]", if *ok { "résolu" } else { "INTROUVABLE" });
        }
        let seen: mlua::Table = lua.globals().get("_HOST_SEEN").unwrap();
        let mut deeper: Vec<String> = seen
            .pairs::<String, i64>()
            .filter_map(Result::ok)
            .map(|(k, _)| k)
            .collect();
        deeper.sort();
        eprintln!(" API hôte profonde ({}) : {:?}", deeper.len(), deeper);
    }
}
