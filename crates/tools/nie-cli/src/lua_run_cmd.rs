//! `niers lua-run` — exécution d'un chunk Lua brut avec résolution VFS des includes.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Context, bail};
use nie_formats::vfs::{Vfs, resolve_game_dir};
use nie_lua::runtime::{ExecOptions, execute_with_script_paths};
use nie_lua::{index_script_paths, resolve_script_path};
use serde_json::json;

/// Exécute un fichier disque ou un chemin logique du VFS dans la VM Lua 5.2 réelle.
pub fn run(
    script: &str,
    game_dir: Option<&Path>,
    instruction_limit: u32,
    with_menu_host: bool,
    disassemble: bool,
) -> anyhow::Result<()> {
    let root = game_dir.map(PathBuf::from).unwrap_or_else(resolve_game_dir);
    let mut vfs = Vfs::new();
    vfs.init(root.join("data"))
        .with_context(|| format!("montage VFS de {}", root.display()))?;

    let script_path = Path::new(script);
    let (name, bytes) = if script_path.is_file() {
        (
            script.to_string(),
            std::fs::read(script_path).with_context(|| format!("lecture de {script}"))?,
        )
    } else {
        let paths: Vec<String> = vfs.iter().map(|(path, _)| path.to_string()).collect();
        let (by_base, by_logical) = index_script_paths(paths.iter().map(String::as_str));
        let resolved = if script.starts_with("data/") {
            Some(script.to_string())
        } else {
            resolve_script_path(script, &by_base, &by_logical).cloned()
        }
        .ok_or_else(|| anyhow::anyhow!("script Lua introuvable dans le VFS : {script}"))?;
        let bytes = vfs
            .read(&resolved)
            .with_context(|| format!("lecture VFS de {resolved}"))?;
        (resolved, bytes)
    };

    let paths: Vec<String> = vfs.iter().map(|(path, _)| path.to_string()).collect();
    let vfs = Rc::new(vfs);
    let options = ExecOptions {
        chunk_name: name.clone(),
        instruction_limit: (instruction_limit != 0).then_some(instruction_limit),
        with_menu_host,
        context: Default::default(),
    };
    let decoded = nie_lua::bytecode::parse(&bytes);
    let decoded_instructions = decoded
        .as_ref()
        .ok()
        .map(|chunk| chunk.main.total_instructions());
    let decode_error = decoded.as_ref().err().map(ToString::to_string);
    let output =
        execute_with_script_paths(&bytes, &options, paths, move |path| vfs.read(path).ok())
            .map_err(|error| anyhow::anyhow!("exécution Lua de {name} : {error}"))?;
    let disassembly = if disassemble {
        Some(nie_lua::bytecode::disassemble(
            &nie_lua::bytecode::parse(&bytes)
                .map_err(|error| anyhow::anyhow!("décodage de {name} : {error}"))?,
        ))
    } else {
        None
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "script": name,
            "ok": output.error.is_none(),
            "decoded": decoded_instructions.is_some(),
            "decodedInstructions": decoded_instructions,
            "liveDecodedInstructions": output.decoded_instructions,
            "liveDecodedIncludeInstructions": output.decoded_include_instructions,
            "liveDecodedInstructionsTotal": output.decoded_instructions_total,
            "decodeError": decode_error,
            "stdout": output.stdout,
            "returned": output.returned,
            "missingHostCalls": output.missing_host_calls,
            "missingHostReads": output.missing_host_reads,
            "missingHostInvocations": output.missing_host_invocations,
            "missingHostPaths": output.missing_host_paths,
            "missingIncludes": output.missing_includes,
            "loadedIncludes": output.loaded_includes,
            "durationMs": output.duration_ms,
            "error": output.error,
            "disassembly": disassembly,
        }))?
    );
    if output.error.is_some() {
        bail!("exécution Lua échouée");
    }
    Ok(())
}
