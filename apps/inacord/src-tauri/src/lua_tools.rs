//! Atelier Lua — façade IPC au-dessus de [`nie_lua`].
//!
//! Le moteur Level-5 « Lives » pilote ses menus, scènes et événements par des scripts Lua 5.2
//! livrés **uniquement compilés** (~1 100 `.lua.bin`). Jusqu'ici l'app savait seulement dire
//! « c'est du bytecode Lua » : ce module ouvre la chaîne complète — décoder, désassembler,
//! exécuter, inspecter, modifier.
//!
//! Tout s'appuie sur `nie-lua`, qui embarque la **VM exacte du jeu** (mlua, PUC-Rio 5.2.4
//! vendored) : le bytecode est exécuté par la même implémentation que `nie.exe`, pas
//! réinterprété.

use serde::Serialize;

/// En-tête + statistiques d'un chunk décodé.
#[derive(Serialize, specta::Type)]
pub struct LuaChunkInfoDto {
    /// Version Lua encodée dans l'en-tête (`82` = `0x52` = Lua 5.2).
    pub version: u32,
    /// `true` si petit-boutiste.
    pub little_endian: bool,
    /// Taille d'un `size_t` C (4 sur une cible 32 bits, 8 sur 64) — décale tout le fichier.
    pub size_size_t: u32,
    /// Nombre de paramètres de la fonction principale.
    pub num_params: u32,
    /// Instructions de la fonction principale.
    pub instructions: u32,
    /// Instructions au total, prototypes imbriqués compris.
    pub total_instructions: u32,
    /// Nombre de prototypes imbriqués (récursif).
    pub total_protos: u32,
    /// Constantes de la fonction principale.
    pub constants: u32,
    /// Upvalues de la fonction principale.
    pub upvalues: u32,
    /// Nom de source du bloc de débogage — vide si le chunk a été dépouillé.
    pub source: String,
    /// `true` si les tables de débogage sont présentes (lignes/locales) : c'est ce qui rend le
    /// désassemblage lisible.
    pub has_debug_info: bool,
    /// Chaînes du pool de constantes de tout l'arbre — ce que le script manipule réellement
    /// (noms de menus, clés de texte, appels moteur).
    pub strings: Vec<String>,
}

/// Décode un `.lua.bin` et renvoie son en-tête + ses statistiques.
///
/// # Errors
/// Message lisible si le tampon n'est pas du bytecode Lua 5.2 ou s'il est tronqué.
pub fn chunk_info(data: &[u8]) -> Result<LuaChunkInfoDto, String> {
    let chunk = nie_lua::bytecode::parse(data).map_err(|e| e.to_string())?;
    let main = &chunk.main;

    let mut strings = Vec::new();
    collect_strings(main, &mut strings);
    strings.sort_unstable();
    strings.dedup();

    Ok(LuaChunkInfoDto {
        version: u32::from(chunk.header.version),
        little_endian: chunk.header.little_endian,
        size_size_t: u32::from(chunk.header.size_size_t),
        num_params: u32::from(main.num_params),
        instructions: main.code.len() as u32,
        total_instructions: main.total_instructions() as u32,
        total_protos: main.total_protos() as u32,
        constants: main.constants.len() as u32,
        upvalues: main.upvalues.len() as u32,
        source: main.source.clone(),
        has_debug_info: !main.line_info.is_empty() || !main.loc_vars.is_empty(),
        strings,
    })
}

/// Collecte récursivement les constantes chaîne de tout l'arbre de prototypes.
fn collect_strings(p: &nie_lua::bytecode::Prototype, out: &mut Vec<String>) {
    for k in &p.constants {
        if let nie_lua::bytecode::Constant::String(bytes) = k {
            // `from_utf8_lossy` : certains libellés du jeu sont dans un encodage japonais hérité,
            // les rejeter ferait disparaître des chaînes réelles de la liste.
            let s = String::from_utf8_lossy(bytes).trim().to_string();
            if !s.is_empty() {
                out.push(s);
            }
        }
    }
    for sub in &p.protos {
        collect_strings(sub, out);
    }
}

/// Désassemble un `.lua.bin` en listing lisible.
///
/// # Errors
/// Message lisible si le décodage échoue.
pub fn disassemble(data: &[u8]) -> Result<String, String> {
    let chunk = nie_lua::bytecode::parse(data).map_err(|e| e.to_string())?;
    Ok(nie_lua::bytecode::disassemble(&chunk))
}

/// Résultat d'exécution renvoyé au frontend.
#[derive(Serialize, specta::Type)]
pub struct LuaExecResultDto {
    /// Lignes imprimées par `print`.
    pub stdout: Vec<String>,
    /// Message d'erreur du script, s'il a échoué (ce n'est PAS une erreur de commande : voir le
    /// message est le résultat attendu quand on mène un script au point).
    pub error: Option<String>,
    /// Valeurs retournées par le chunk.
    pub returned: Vec<String>,
    /// Globals hôtes appelés mais non définis — la surface d'API moteur que ce script réclame.
    pub missing_host_calls: Vec<String>,
    /// Durée d'exécution, en millisecondes.
    pub duration_ms: u32,
}

/// Exécute une source Lua ou un bytecode du jeu.
///
/// `with_menu_host` installe l'hôte de menu reversé (`nie_lua::install_menu_host`), ce qui permet
/// aux vrais scripts de menu d'aller bien au-delà du premier appel moteur.
///
/// # Errors
/// Message lisible si la VM ne peut pas être préparée.
pub fn execute(data: &[u8], chunk_name: &str, with_menu_host: bool, instruction_limit: Option<u32>) -> Result<LuaExecResultDto, String> {
    let options = nie_lua::runtime::ExecOptions {
        chunk_name: chunk_name.to_string(),
        instruction_limit,
        with_menu_host,
    };
    let out = nie_lua::runtime::execute(data, &options).map_err(|e| e.to_string())?;
    Ok(LuaExecResultDto {
        stdout: out.stdout,
        error: out.error,
        returned: out.returned,
        missing_host_calls: out.missing_host_calls,
        duration_ms: out.duration_ms as u32,
    })
}

/// Une valeur globale exposée à l'éditeur de valeurs.
#[derive(Serialize, specta::Type)]
pub struct LuaGlobalDto {
    /// Nom du global.
    pub name: String,
    /// Type Lua.
    pub type_name: String,
    /// Rendu texte de la valeur.
    pub value: String,
    /// Nombre d'entrées si c'est une table.
    pub len: Option<u32>,
}

/// Exécute un script puis renvoie l'état de ses globals — le pas « inspecter après exécution ».
///
/// Chaque appel repart d'une VM neuve : deux inspections successives ne doivent pas se contaminer,
/// et un script qui a corrompu son état ne doit pas empoisonner le suivant.
///
/// # Errors
/// Message lisible si la VM ne peut pas être préparée.
pub fn globals_after_run(
    data: &[u8],
    chunk_name: &str,
    with_menu_host: bool,
    overrides: &[(String, String)],
    include_stdlib: bool,
) -> Result<Vec<LuaGlobalDto>, String> {
    let lua = nie_lua::new_vm();
    let sink = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    nie_lua::runtime::install_print_capture(&lua, sink).map_err(|e| e.to_string())?;
    if with_menu_host {
        nie_lua::install_menu_host(&lua).map_err(|e| e.to_string())?;
    }

    // Les valeurs forcées sont posées AVANT l'exécution : c'est ce qui permet de rejouer un script
    // « comme si » une variable moteur valait autre chose, au lieu de constater après coup.
    for (name, expr) in overrides {
        let assignment = format!("{name} = {expr}");
        lua.load(&assignment)
            .set_name("=override")
            .exec()
            .map_err(|e| format!("valeur forcée « {name} = {expr} » : {e}"))?;
    }

    nie_lua::runtime::install_host_stubs(&lua).map_err(|e| e.to_string())?;

    let mode = if nie_lua::is_lua52_bytecode(data) {
        mlua_chunk_mode_binary()
    } else {
        mlua_chunk_mode_text()
    };
    // L'échec du script ne doit pas empêcher d'inspecter ce qu'il a posé avant de planter.
    let _ = lua.load(data).set_name(chunk_name.to_string()).set_mode(mode).exec();

    Ok(nie_lua::runtime::list_globals(&lua, include_stdlib)
        .into_iter()
        .map(|g| LuaGlobalDto {
            name: g.name,
            type_name: g.type_name,
            value: g.value,
            len: g.len,
        })
        .collect())
}

// `mlua` n'est pas une dépendance directe de ce crate : ces deux helpers évitent de l'ajouter au
// `Cargo.toml` juste pour nommer deux variantes d'énumération.
fn mlua_chunk_mode_binary() -> nie_lua::ChunkMode {
    nie_lua::ChunkMode::Binary
}
fn mlua_chunk_mode_text() -> nie_lua::ChunkMode {
    nie_lua::ChunkMode::Text
}

/// Évalue une expression dans une VM neuve où `data` a d'abord été exécuté — la console.
///
/// # Errors
/// Message lisible si la VM ne peut pas être préparée.
pub fn eval(data: &[u8], chunk_name: &str, expression: &str, with_menu_host: bool) -> Result<String, String> {
    let lua = nie_lua::new_vm();
    let sink = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    nie_lua::runtime::install_print_capture(&lua, sink).map_err(|e| e.to_string())?;
    if with_menu_host {
        nie_lua::install_menu_host(&lua).map_err(|e| e.to_string())?;
    }
    nie_lua::runtime::install_host_stubs(&lua).map_err(|e| e.to_string())?;

    if !data.is_empty() {
        let mode = if nie_lua::is_lua52_bytecode(data) {
            mlua_chunk_mode_binary()
        } else {
            mlua_chunk_mode_text()
        };
        let _ = lua.load(data).set_name(chunk_name.to_string()).set_mode(mode).exec();
    }

    nie_lua::runtime::eval_expression(&lua, expression).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_et_desassemblage_dun_chunk_compile() {
        let lua = nie_lua::new_vm();
        let dumped = lua
            .load("local x = 1 print('salut') return x")
            .into_function()
            .expect("compilation")
            .dump(false);

        let info = chunk_info(&dumped).expect("info");
        assert_eq!(info.version, 0x52);
        assert!(info.instructions > 0);
        assert!(info.strings.iter().any(|s| s == "salut"), "chaînes : {:?}", info.strings);

        let listing = disassemble(&dumped).expect("désassemblage");
        assert!(listing.contains("function main"), "listing :\n{listing}");
    }

    #[test]
    fn execute_capture_la_sortie() {
        let out = execute(b"print('coucou') return 5", "essai", false, Some(1_000_000)).expect("exec");
        assert_eq!(out.stdout, vec!["coucou".to_string()]);
        assert_eq!(out.returned, vec!["5".to_string()]);
        assert!(out.error.is_none());
    }

    #[test]
    fn globals_avec_valeur_forcee() {
        // Sans valeur forcée, `hp` vaut ce que le script pose.
        let globals = globals_after_run(b"hp = 10", "essai", false, &[], false).expect("globals");
        let hp = globals.iter().find(|g| g.name == "hp").expect("hp");
        assert_eq!(hp.value, "10");

        // Le script écrase la valeur forcée — c'est le comportement attendu, et ça se voit.
        let forced = globals_after_run(
            b"if hp == nil then hp = 10 end",
            "essai",
            false,
            &[("hp".to_string(), "999".to_string())],
            false,
        )
        .expect("globals");
        let hp = forced.iter().find(|g| g.name == "hp").expect("hp");
        assert_eq!(hp.value, "999", "la valeur forcée devait survivre au garde `if nil`");
    }

    #[test]
    fn console_evalue_dans_letat_du_script() {
        let value = eval(b"total = 6 * 7", "essai", "total", false).expect("eval");
        assert_eq!(value, "42");
    }
}
