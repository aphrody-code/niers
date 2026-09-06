//! `niers lua-audit` — mesure batch de l'exécution des chunks Lua bruts du VFS.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Context, bail};
use nie_formats::vfs::{Vfs, resolve_game_dir};
use nie_lua::runtime::{ExecOptions, execute_with_include};
use nie_lua::{index_script_paths, resolve_script_path};
use serde_json::json;

/// Exécute les scripts `.lua.bin` du VFS et rend une mesure exploitable en CI/RE.
pub fn run(
    game_dir: Option<&Path>,
    prefix: Option<&str>,
    instruction_limit: u32,
    menu_host: bool,
) -> anyhow::Result<()> {
    let root = game_dir.map(PathBuf::from).unwrap_or_else(resolve_game_dir);
    let mut vfs = Vfs::new();
    vfs.init(root.join("data"))
        .with_context(|| format!("montage VFS de {}", root.display()))?;

    let paths: Vec<String> = vfs
        .iter()
        .map(|(path, _)| path.to_string())
        .filter(|path| {
            path.ends_with(".lua.bin") && prefix.is_none_or(|value| path.starts_with(value))
        })
        .collect();
    if paths.is_empty() {
        bail!("aucun `.lua.bin` ne correspond au filtre demandé");
    }

    let all_paths: Vec<String> = vfs.iter().map(|(path, _)| path.to_string()).collect();
    let (by_base, by_logical) = index_script_paths(all_paths.iter().map(String::as_str));
    let by_base = Rc::new(by_base);
    let by_logical = Rc::new(by_logical);
    let vfs = Rc::new(vfs);
    let mut executed = 0usize;
    let mut decoded = 0usize;
    let mut decode_errors = 0usize;
    let mut decoded_instructions = 0usize;
    let mut ok = 0usize;
    let mut errors = 0usize;
    let mut missing_includes: BTreeMap<String, usize> = BTreeMap::new();
    let mut loaded_includes: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing_hosts: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing_host_reads: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing_host_invocations: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing_host_paths: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing_host_scripts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut missing_host_read_scripts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut missing_host_path_scripts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut samples = Vec::new();

    for path in &paths {
        let bytes = match vfs.read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                errors += 1;
                if samples.len() < 20 {
                    samples.push(
                        json!({ "script": path, "kind": "read", "error": error.to_string() }),
                    );
                }
                continue;
            }
        };
        executed += 1;
        match nie_lua::bytecode::parse(&bytes) {
            Ok(chunk) => {
                decoded += 1;
                decoded_instructions += chunk.main.total_instructions();
            }
            Err(error) => {
                decode_errors += 1;
                if samples.len() < 20 {
                    samples.push(json!({
                        "script": path,
                        "kind": "decode",
                        "error": error.to_string()
                    }));
                }
            }
        }
        let options = ExecOptions {
            chunk_name: path.clone(),
            instruction_limit: (instruction_limit != 0).then_some(instruction_limit),
            with_menu_host: menu_host,
        };
        let resolver_vfs = Rc::clone(&vfs);
        let resolver_base = Rc::clone(&by_base);
        let resolver_logical = Rc::clone(&by_logical);
        let result = execute_with_include(&bytes, &options, move |include| {
            let resolved = resolve_script_path(include, &resolver_base, &resolver_logical)?;
            resolver_vfs.read(resolved).ok()
        });
        match result {
            Ok(output) => {
                for include in output.missing_includes {
                    *missing_includes.entry(include).or_default() += 1;
                }
                for include in output.loaded_includes {
                    *loaded_includes.entry(include).or_default() += 1;
                }
                for host in output.missing_host_calls {
                    *missing_hosts.entry(host.clone()).or_default() += 1;
                    let scripts = missing_host_scripts.entry(host).or_default();
                    if !scripts.iter().any(|known| known == path) {
                        scripts.push(path.clone());
                    }
                }
                for host in output.missing_host_reads {
                    *missing_host_reads.entry(host.clone()).or_default() += 1;
                    let scripts = missing_host_read_scripts.entry(host).or_default();
                    if !scripts.iter().any(|known| known == path) {
                        scripts.push(path.clone());
                    }
                }
                for host in output.missing_host_invocations {
                    *missing_host_invocations.entry(host).or_default() += 1;
                }
                for missing_path in output.missing_host_paths {
                    *missing_host_paths.entry(missing_path.clone()).or_default() += 1;
                    let scripts = missing_host_path_scripts.entry(missing_path).or_default();
                    if !scripts.iter().any(|known| known == path) {
                        scripts.push(path.clone());
                    }
                }
                if let Some(error) = output.error {
                    errors += 1;
                    if samples.len() < 20 {
                        samples.push(json!({ "script": path, "kind": "runtime", "error": error }));
                    }
                } else {
                    ok += 1;
                }
            }
            Err(error) => {
                errors += 1;
                if samples.len() < 20 {
                    samples.push(
                        json!({ "script": path, "kind": "load", "error": error.to_string() }),
                    );
                }
            }
        }
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "gameDir": root,
            "prefix": prefix,
            "scripts": paths.len(),
            "executed": executed,
            "decoded": decoded,
            "decodeErrors": decode_errors,
            "decodedInstructions": decoded_instructions,
            "ok": ok,
            "errors": errors,
            "missingIncludes": missing_includes,
            "loadedIncludes": loaded_includes,
            "missingHostCalls": missing_hosts,
            "missingHostReads": missing_host_reads,
            "missingHostInvocations": missing_host_invocations,
            "missingHostPaths": missing_host_paths,
            "missingHostScripts": missing_host_scripts,
            "missingHostReadScripts": missing_host_read_scripts,
            "missingHostPathScripts": missing_host_path_scripts,
            "samples": samples,
        }))?
    );
    Ok(())
}
