//! **Bac à sable d'exécution Lua** — interpréteur, console et éditeur de valeurs.
//!
//! [`crate::new_vm`] fournit la VM ; ce module fournit ce qu'il faut autour pour s'en servir comme
//! d'un outil : exécuter un script (source **ou** bytecode du jeu) en capturant ce qu'il imprime,
//! inspecter l'état résultant, et modifier une valeur pour relancer.
//!
//! ## Ce qui est capturé, et pourquoi
//!
//! `print` est remplacé par une fonction hôte qui écrit dans un tampon partagé au lieu de la sortie
//! standard. Sans ça, la sortie d'un script lancé depuis une interface graphique part dans un
//! terminal que personne ne regarde — le script « ne fait rien » du point de vue de l'utilisatrice.
//!
//! ## Limite d'exécution
//!
//! Les scripts du jeu attendent un moteur : ils bouclent volontiers en espérant qu'un hôte
//! réponde. Un compteur d'instructions ([`ExecOptions::instruction_limit`]) interrompt l'exécution
//! au lieu de figer l'application appelante. C'est un garde-fou, pas un bac à sable de sécurité :
//! le bytecode reste exécuté par la vraie VM, avec ses bibliothèques.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use mlua::{Lua, MultiValue, Value, Variadic};

use crate::{LuaError, is_lua52_bytecode, validate_bytecode};

type IncludeResolver = Box<dyn Fn(&str) -> Option<Vec<u8>>>;

/// Valeurs natives primitives injectées dans la VM avant un chunk et ses includes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeContext {
    numbers: BTreeMap<String, f64>,
    booleans: BTreeMap<String, bool>,
    strings: BTreeMap<String, String>,
}

impl RuntimeContext {
    /// Pose un global numérique (indice, coordonnée ou enum natif).
    pub fn set_number(&mut self, name: impl Into<String>, value: f64) {
        self.numbers.insert(name.into(), value);
    }

    /// Pose un global booléen fourni par le moteur.
    pub fn set_boolean(&mut self, name: impl Into<String>, value: bool) {
        self.booleans.insert(name.into(), value);
    }

    /// Pose un global texte fourni par le moteur.
    pub fn set_string(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.strings.insert(name.into(), value.into());
    }

    pub(crate) fn apply(&self, lua: &Lua) -> mlua::Result<()> {
        let globals = lua.globals();
        for (name, value) in &self.numbers {
            globals.set(name.as_str(), *value)?;
        }
        for (name, value) in &self.booleans {
            globals.set(name.as_str(), *value)?;
        }
        for (name, value) in &self.strings {
            globals.set(name.as_str(), value.as_str())?;
        }
        Ok(())
    }
}

/// Options d'exécution.
#[derive(Debug, Clone)]
pub struct ExecOptions {
    /// Nom affiché du chunk dans les messages d'erreur (`@nom:ligne`).
    pub chunk_name: String,
    /// Nombre maximal d'instructions VM avant interruption. `None` = sans limite (à réserver aux
    /// scripts dont on maîtrise la terminaison).
    pub instruction_limit: Option<u32>,
    /// Installe l'hôte de menu du moteur ([`crate::install_menu_host`]) avant l'exécution — permet
    /// aux vrais scripts de menu d'aller au-delà du premier appel hôte.
    pub with_menu_host: bool,
    /// Globals primitifs fournis par le contexte save/scène du moteur.
    pub context: RuntimeContext,
}

impl Default for ExecOptions {
    fn default() -> Self {
        Self {
            chunk_name: "chunk".to_string(),
            // 20 millions : large pour un script de menu réel (le plus gros du jeu tient en
            // ~10⁵ instructions), assez bas pour couper une boucle infinie en une seconde.
            instruction_limit: Some(20_000_000),
            with_menu_host: false,
            context: RuntimeContext::default(),
        }
    }
}

/// Résultat d'une exécution.
#[derive(Debug, Clone, Default)]
pub struct ExecOutput {
    /// Lignes imprimées par le script (`print`, une entrée par appel).
    pub stdout: Vec<String>,
    /// Message d'erreur si l'exécution a échoué (script planté, limite atteinte).
    pub error: Option<String>,
    /// Valeurs retournées par le chunk, rendues en texte.
    pub returned: Vec<String>,
    /// Globals hôtes touchés mais non définis — la surface d'API que ce script attend du moteur.
    pub missing_host_calls: Vec<String>,
    /// Globals hôtes indéfinis seulement lus (sans appel), souvent des paramètres injectés par le
    /// moteur dans le contexte du chunk plutôt que des fonctions d'API.
    pub missing_host_reads: Vec<String>,
    /// Chemins hôtes effectivement invoqués (`__call__`), sous-ensemble actionnable des lectures.
    pub missing_host_invocations: Vec<String>,
    /// Chemins d'API hôte imbriqués touchés par les stubs (`LISTVIEW.Set...`, par exemple).
    pub missing_host_paths: Vec<String>,
    /// Modules demandés par `INCLUDE` mais absents du résolveur VFS.
    pub missing_includes: Vec<String>,
    /// Modules effectivement résolus et exécutés par `INCLUDE`, dans l'ordre de chargement.
    ///
    /// Cette trace est volontairement conservée séparément des includes manquants : elle permet
    /// de vérifier le chemin VFS brut → chunk → VM sans déduire un succès de la seule absence
    /// d'erreur.
    pub loaded_includes: Vec<String>,
    /// Durée d'exécution en millisecondes.
    pub duration_ms: u64,
}

/// Rendu texte court d'une valeur Lua, pour affichage.
#[must_use]
pub fn value_to_string(v: &Value) -> String {
    match v {
        Value::Nil => "nil".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.to_string_lossy().to_string(),
        Value::Table(_) => "<table>".to_string(),
        Value::Function(_) => "<function>".to_string(),
        Value::Thread(_) => "<thread>".to_string(),
        Value::UserData(_) => "<userdata>".to_string(),
        Value::LightUserData(_) => "<lightuserdata>".to_string(),
        Value::Error(e) => format!("<error: {e}>"),
        _ => "<valeur>".to_string(),
    }
}

/// Nom de type Lua, tel que `type()` le renverrait.
#[must_use]
pub fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Nil => "nil",
        Value::Boolean(_) => "boolean",
        Value::Integer(_) | Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Table(_) => "table",
        Value::Function(_) => "function",
        Value::Thread(_) => "thread",
        Value::UserData(_) | Value::LightUserData(_) => "userdata",
        _ => "unknown",
    }
}

/// Installe un `print` qui écrit dans `sink` au lieu de la sortie standard.
///
/// # Errors
/// [`mlua::Error`] si l'enregistrement échoue.
pub fn install_print_capture(lua: &Lua, sink: Rc<RefCell<Vec<String>>>) -> mlua::Result<()> {
    let f = lua.create_function(move |_, args: Variadic<Value>| {
        // `print` sépare ses arguments par une tabulation — on reproduit, pour que la sortie
        // corresponde à ce qu'un terminal aurait montré.
        let line = args
            .iter()
            .map(value_to_string)
            .collect::<Vec<_>>()
            .join("\t");
        sink.borrow_mut().push(line);
        Ok(())
    })?;
    lua.globals().set("print", f)?;
    Ok(())
}

/// Installe une métatable sur `_G` qui note tout accès à un global **indéfini** et renvoie un stub
/// appelable, pour que le script continue au lieu de planter au premier appel moteur.
///
/// Même technique que [`crate::discover_host_calls`], mais réutilisable pendant une exécution
/// normale : c'est ce qui permet de faire tourner un script de menu réel sans avoir implémenté
/// tout le moteur.
///
/// # Errors
/// [`mlua::Error`] si l'installation échoue.
pub fn install_host_stubs(lua: &Lua) -> mlua::Result<()> {
    lua.load(
        r#"
        _HOST_MISSING = {}
        _HOST_MISSING_READS = {}
        _HOST_MISSING_CALLS = {}
        _HOST_MISSING_PATHS = {}
        local function note_read(path)
            _HOST_MISSING_READS[path] = true
            _HOST_MISSING_PATHS[path] = true
        end
        local function note_call(path)
            _HOST_MISSING_CALLS[path] = true
            _HOST_MISSING_PATHS[path] = true
        end
        local function stub(path)
            return setmetatable({}, {
                __call = function() note_call(path .. "()"); return stub(path .. "()") end,
                __index = function(_, k)
                    local child = path .. "." .. tostring(k)
                    note_read(child)
                    return stub(child)
                end,
            })
        end
        setmetatable(_G, {
            __index = function(_, k)
                _HOST_MISSING[k] = true
                note_read(k)
                return stub(k)
            end,
        })
        "#,
    )
    .exec()
}

/// Exécute une source Lua **ou** un bytecode `.lua.bin` du jeu et renvoie tout ce qui s'est passé.
///
/// Le mode (texte/binaire) est déduit de la signature : un `.lua.bin` commence par `\x1bLua`.
///
/// # Errors
/// [`LuaError`] si la VM ne peut pas être préparée. Une erreur *du script*, elle, n'est pas une
/// erreur de cette fonction : elle est rendue dans [`ExecOutput::error`], parce que voir le
/// message d'erreur EST le résultat attendu quand on met au point un script.
pub fn execute(data: &[u8], options: &ExecOptions) -> Result<ExecOutput, LuaError> {
    execute_inner(data, options, None)
}

/// Exécute un chunk avec un résolveur `INCLUDE` branché sur le VFS appelant.
///
/// Les modules sont chargés et exécutés dans la même VM que le chunk principal, comme le
/// moteur du jeu : leurs globals, fonctions et retours restent donc visibles pendant toute
/// l'exécution. Le résolveur peut renvoyer du bytecode Lua 5.2 ou de la source Lua.
pub fn execute_with_include<F>(
    data: &[u8],
    options: &ExecOptions,
    resolver: F,
) -> Result<ExecOutput, LuaError>
where
    F: Fn(&str) -> Option<Vec<u8>> + 'static,
{
    execute_inner(data, options, Some(Box::new(resolver)))
}

/// Exécute un chunk avec un index VFS de scripts versionnés.
///
/// Le caller ne fournit que les chemins physiques et le reader brut ; la résolution des noms
/// logiques `LUA_*`, des basenames et des versions numériques est identique à celle de
/// [`crate::session::LuaSession::with_script_paths`]. Le chunk principal et chaque include
/// restent exécutés dans la même VM instrumentée.
pub fn execute_with_script_paths<I, F>(
    data: &[u8],
    options: &ExecOptions,
    paths: I,
    reader: F,
) -> Result<ExecOutput, LuaError>
where
    I: IntoIterator<Item = String>,
    F: Fn(&str) -> Option<Vec<u8>> + 'static,
{
    let paths = paths.into_iter().collect::<Vec<_>>();
    let (by_name, by_logical) = crate::index_script_paths(paths.iter().map(String::as_str));
    let by_name = Rc::new(by_name);
    let by_logical = Rc::new(by_logical);
    let reader = Rc::new(reader);
    execute_with_include(data, options, move |name| {
        let path = crate::resolve_script_path(name, &by_name, &by_logical)?;
        reader(path)
    })
}

fn execute_inner(
    data: &[u8],
    options: &ExecOptions,
    resolver: Option<IncludeResolver>,
) -> Result<ExecOutput, LuaError> {
    // Le chunk principal suit le même chemin de décodage que `load_bytecode` et les includes.
    validate_bytecode(data)?;
    let lua = crate::new_vm();
    let started = std::time::Instant::now();

    let stdout = Rc::new(RefCell::new(Vec::new()));
    install_print_capture(&lua, Rc::clone(&stdout))?;

    // Global moteur requis par les includes et disponible même sans le host de menu.
    let crc32 =
        lua.create_function(|_, value: String| Ok(f64::from(crate::crc32(value.as_bytes()))))?;
    lua.globals().set("CRC32", crc32)?;

    let missing_includes = Rc::new(RefCell::new(Vec::<String>::new()));
    let loaded_includes = Rc::new(RefCell::new(Vec::<String>::new()));
    if let Some(resolver) = resolver {
        let missing = Rc::clone(&missing_includes);
        let loaded = Rc::clone(&loaded_includes);
        crate::install_include(&lua, move |name| match resolver(name) {
            Some(bytes) => {
                loaded.borrow_mut().push(name.to_string());
                Some(bytes)
            }
            None => {
                missing.borrow_mut().push(name.to_string());
                None
            }
        })?;
    }

    if options.with_menu_host {
        crate::install_menu_host(&lua)?;
    }
    install_host_stubs(&lua)?;
    options.context.apply(&lua)?;

    if let Some(limit) = options.instruction_limit {
        // Le hook VM est le seul moyen d'interrompre un chunk qui boucle : `mlua` ne préempte pas.
        // Le compteur vit dans une `Cell` : `set_hook` exige un `Fn` (appelable plusieurs fois
        // depuis la VM), pas un `FnMut`, donc la mutation doit passer par la mutabilité interne.
        let executed = std::cell::Cell::new(0u32);
        // `?` plutôt que d'ignorer : un hook refusé signifierait que la limite ne s'applique pas,
        // donc qu'un script bouclant figerait l'appelant — un échec silencieux inacceptable ici.
        lua.set_hook(mlua::HookTriggers::new().every_nth_instruction(10_000), move |_lua, _dbg| {
            executed.set(executed.get().saturating_add(10_000));
            if executed.get() >= limit {
                return Err(mlua::Error::RuntimeError(format!(
                    "limite d'exécution atteinte ({limit} instructions) — script probablement en attente du moteur"
                )));
            }
            Ok(mlua::VmState::Continue)
        })?;
    }

    let mode = if is_lua52_bytecode(data) {
        mlua::ChunkMode::Binary
    } else {
        mlua::ChunkMode::Text
    };
    let result = lua
        .load(data)
        .set_name(options.chunk_name.clone())
        .set_mode(mode)
        .call::<MultiValue>(());

    let mut out = ExecOutput {
        duration_ms: started.elapsed().as_millis() as u64,
        stdout: stdout.borrow().clone(),
        ..Default::default()
    };

    match result {
        Ok(values) => out.returned = values.iter().map(value_to_string).collect(),
        Err(e) => out.error = Some(e.to_string()),
    }

    // Globals manquants relevés par les stubs : la liste d'API hôte que ce script réclame.
    if let Ok(missing) = lua.globals().get::<mlua::Table>("_HOST_MISSING") {
        let mut names: Vec<String> = missing
            .pairs::<String, Value>()
            .filter_map(Result::ok)
            .map(|(k, _)| k)
            .collect();
        names.sort_unstable();
        names.dedup();
        out.missing_host_calls = names;
    }
    if let Ok(missing) = lua.globals().get::<mlua::Table>("_HOST_MISSING_READS") {
        let mut names: Vec<String> = missing
            .pairs::<String, Value>()
            .filter_map(Result::ok)
            .map(|(k, _)| k)
            .collect();
        names.sort_unstable();
        names.dedup();
        out.missing_host_reads = names;
    }
    if let Ok(missing) = lua.globals().get::<mlua::Table>("_HOST_MISSING_CALLS") {
        let mut names: Vec<String> = missing
            .pairs::<String, Value>()
            .filter_map(Result::ok)
            .map(|(k, _)| k)
            .collect();
        names.sort_unstable();
        names.dedup();
        out.missing_host_invocations = names;
    }
    if let Ok(missing) = lua.globals().get::<mlua::Table>("_HOST_MISSING_PATHS") {
        let mut paths: Vec<String> = missing
            .pairs::<String, Value>()
            .filter_map(Result::ok)
            .map(|(path, _)| path)
            .collect();
        paths.sort_unstable();
        paths.dedup();
        out.missing_host_paths = paths;
    }
    out.missing_includes = missing_includes.borrow().clone();
    out.missing_includes.sort_unstable();
    out.missing_includes.dedup();
    out.loaded_includes = loaded_includes.borrow().clone();

    Ok(out)
}

/// Une entrée de l'inspecteur de valeurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalEntry {
    /// Nom du global.
    pub name: String,
    /// Type Lua (`string`, `number`, `table`, `function`…).
    pub type_name: String,
    /// Rendu texte de la valeur (`<table>` pour les agrégats).
    pub value: String,
    /// Nombre d'entrées si c'est une table — sinon `None`.
    pub len: Option<u32>,
}

/// Liste les globals d'une VM, triés par nom.
///
/// Les entrées standard (`string`, `table`, `math`, …) sont exclues par défaut : elles noient les
/// quelques dizaines de valeurs réellement posées par le script sous la bibliothèque standard.
#[must_use]
pub fn list_globals(lua: &Lua, include_stdlib: bool) -> Vec<GlobalEntry> {
    const STDLIB: [&str; 17] = [
        "_G",
        "_VERSION",
        "assert",
        "collectgarbage",
        "coroutine",
        "debug",
        "error",
        "io",
        "math",
        "os",
        "package",
        "pcall",
        "print",
        "string",
        "table",
        "type",
        "xpcall",
    ];

    let Ok(globals) = lua
        .globals()
        .pairs::<String, Value>()
        .collect::<mlua::Result<Vec<_>>>()
    else {
        return Vec::new();
    };

    let mut out: Vec<GlobalEntry> = globals
        .into_iter()
        .filter(|(name, _)| include_stdlib || !STDLIB.contains(&name.as_str()))
        .map(|(name, value)| {
            let len = match &value {
                Value::Table(t) => Some(t.clone().pairs::<Value, Value>().count() as u32),
                _ => None,
            };
            GlobalEntry {
                name,
                type_name: value_type_name(&value).to_string(),
                value: value_to_string(&value),
                len,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Évalue une expression dans une VM existante et renvoie son rendu texte.
///
/// C'est la brique d'une console : `pcall`-équivalent côté Rust, l'erreur est une valeur de
/// retour, pas une panique.
///
/// # Errors
/// Jamais — l'échec est renvoyé dans la variante texte. La signature reste `Result` pour rester
/// alignée sur le reste de l'API.
pub fn eval_expression(lua: &Lua, expr: &str) -> Result<String, LuaError> {
    // On tente `return <expr>` d'abord : c'est ce qui rend une console utile (taper `x` affiche la
    // valeur de `x`). Si ça ne compile pas, on retombe sur une instruction complète.
    let as_expr = format!("return {expr}");
    let result = lua
        .load(&as_expr)
        .set_name("=console")
        .eval::<MultiValue>()
        .or_else(|_| lua.load(expr).set_name("=console").eval::<MultiValue>());

    Ok(match result {
        Ok(values) if values.is_empty() => String::new(),
        Ok(values) => values
            .iter()
            .map(value_to_string)
            .collect::<Vec<_>>()
            .join("\t"),
        Err(e) => format!("erreur : {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_la_sortie_et_les_retours() {
        let out = execute(
            b"print('bonjour', 42) return 7, 'x'",
            &ExecOptions::default(),
        )
        .expect("exécution");
        assert_eq!(out.stdout, vec!["bonjour\t42".to_string()]);
        assert_eq!(out.returned, vec!["7".to_string(), "x".to_string()]);
        assert!(out.error.is_none(), "erreur inattendue : {:?}", out.error);
    }

    #[test]
    fn remonte_lerreur_du_script_sans_paniquer() {
        let out = execute(b"error('boum')", &ExecOptions::default()).expect("exécution");
        let msg = out.error.expect("une erreur était attendue");
        assert!(msg.contains("boum"), "message : {msg}");
    }

    #[test]
    fn execute_applique_le_contexte_type_avant_le_chunk() {
        let mut context = RuntimeContext::default();
        context.set_number("x", 12.5);
        context.set_boolean("isGrayout", false);
        context.set_string("MENU_LINIT_NONE", "none");
        let options = ExecOptions {
            context,
            ..Default::default()
        };
        let out = execute(
            br#"assert(x == 12.5); assert(not isGrayout); assert(MENU_LINIT_NONE == "none")"#,
            &options,
        )
        .expect("exécution avec contexte");
        assert!(out.error.is_none(), "erreur inattendue : {:?}", out.error);
        assert!(out.missing_host_reads.is_empty());
    }

    #[test]
    fn execute_valide_le_chunk_binaire_principal_avant_la_vm() {
        let malformed = [0x1B, b'L', b'u', b'a', 0x52, 0x00];
        let error = execute(&malformed, &ExecOptions::default())
            .expect_err("un chunk binaire invalide doit échouer avant la VM");
        assert!(matches!(error, LuaError::Decode(_)));
    }

    #[test]
    fn execute_avec_chemins_vfs_choisit_la_derniere_version_logique() {
        let paths = vec![
            "data/common/script/lua/menu/module_9.lua.bin".to_string(),
            "data/common/script/lua/menu/module_10.lua.bin".to_string(),
        ];
        let out = execute_with_script_paths(
            br#"INCLUDE("LUA_MODULE"); assert(vfs_version == 10)"#,
            &ExecOptions::default(),
            paths,
            |path| (path.ends_with("module_10.lua.bin")).then(|| b"vfs_version = 10".to_vec()),
        )
        .expect("exécution VFS");
        assert!(out.error.is_none(), "erreur inattendue : {:?}", out.error);
        assert_eq!(out.loaded_includes, vec!["LUA_MODULE"]);
    }

    /// Une boucle infinie doit être coupée par la limite d'instructions, pas figer l'appelant.
    #[test]
    fn interrompt_une_boucle_infinie() {
        let options = ExecOptions {
            instruction_limit: Some(100_000),
            ..Default::default()
        };
        let out = execute(b"while true do end", &options).expect("exécution");
        let msg = out.error.expect("la limite devait interrompre le script");
        assert!(msg.contains("limite d'exécution"), "message : {msg}");
    }

    /// Un appel à une fonction moteur inexistante ne doit pas arrêter le script, mais être relevé.
    #[test]
    fn releve_les_appels_hote_manquants() {
        let out = execute(
            b"MENU_OPEN('titre') SOME_ENGINE_CALL()",
            &ExecOptions::default(),
        )
        .expect("exécution");
        assert!(out.error.is_none(), "erreur inattendue : {:?}", out.error);
        assert!(out.missing_host_calls.contains(&"MENU_OPEN".to_string()));
        assert!(
            out.missing_host_calls
                .contains(&"SOME_ENGINE_CALL".to_string())
        );
        assert!(out.missing_host_paths.contains(&"MENU_OPEN()".to_string()));
        assert!(
            out.missing_host_paths
                .contains(&"SOME_ENGINE_CALL()".to_string())
        );
    }

    #[test]
    fn crc32_est_fourni_au_runtime_generique() {
        let out =
            execute(b"return CRC32('general_win')", &ExecOptions::default()).expect("exécution");
        assert_eq!(out.returned, vec!["292844459"]);
        assert!(!out.missing_host_calls.iter().any(|name| name == "CRC32"));
    }

    #[test]
    fn execute_with_include_partage_la_vm_avec_le_module() {
        let out = execute_with_include(
            b"INCLUDE('COMMON'); return included_value, CRC32('general_win')",
            &ExecOptions::default(),
            |name| (name == "COMMON").then(|| b"included_value = 41".to_vec()),
        )
        .expect("exécution");
        assert_eq!(out.returned, vec!["41", "292844459"]);
        assert!(!out.missing_host_calls.iter().any(|name| name == "INCLUDE"));
        assert!(out.missing_includes.is_empty());
        assert_eq!(out.loaded_includes, vec!["COMMON"]);
    }

    #[test]
    fn execute_with_include_signale_un_module_absent() {
        let out = execute_with_include(
            b"INCLUDE('ABSENT'); return 7",
            &ExecOptions::default(),
            |_name| None,
        )
        .expect("exécution");
        assert_eq!(out.returned, vec!["7"]);
        assert_eq!(out.missing_includes, vec!["ABSENT"]);
    }

    #[test]
    fn liste_et_evalue_les_globals() {
        let lua = crate::new_vm();
        lua.load("mavaleur = 3 matable = {a=1, b=2}")
            .exec()
            .expect("préparation");

        let globals = list_globals(&lua, false);
        let names: Vec<&str> = globals.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"mavaleur"), "globals : {names:?}");

        let table = globals
            .iter()
            .find(|g| g.name == "matable")
            .expect("matable");
        assert_eq!(table.type_name, "table");
        assert_eq!(table.len, Some(2));

        assert_eq!(eval_expression(&lua, "mavaleur").unwrap(), "3");
        assert_eq!(eval_expression(&lua, "mavaleur * 2").unwrap(), "6");
        // Une expression fausse renvoie un message, pas une panique.
        assert!(eval_expression(&lua, "@@@").unwrap().contains("erreur"));
    }

    /// L'édition d'une valeur suivie d'une réévaluation — le geste de l'éditeur de valeurs.
    #[test]
    fn modifie_une_valeur_puis_reevalue() {
        let lua = crate::new_vm();
        lua.load("hp = 100").exec().expect("préparation");
        assert_eq!(eval_expression(&lua, "hp").unwrap(), "100");

        lua.globals().set("hp", 250).expect("écriture");
        assert_eq!(eval_expression(&lua, "hp").unwrap(), "250");
    }
}
