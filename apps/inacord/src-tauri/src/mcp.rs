//! Installation du serveur MCP `niers-game` dans la configuration d'un client MCP.
//!
//! L'explorateur et le serveur MCP forment une paire : le serveur pilote l'explorateur par le
//! pont `@niers/bridge`, et c'est l'explorateur qui déclare le serveur à Claude Code / Claude
//! Desktop depuis ses Paramètres — l'utilisatrice n'a pas à éditer un JSON à la main.
//!
//! L'écriture est une **fusion** : les autres serveurs MCP déjà déclarés sont conservés, seule
//! l'entrée `niers-game` est ajoutée ou remplacée. Le fichier existant est sauvegardé en `.bak`
//! avant réécriture.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Nom de l'entrée dans `mcpServers`.
const SERVER_NAME: &str = "niers-game";

/// Point d'entrée du serveur, relatif à la racine du repo.
const ENTRYPOINT: &str = "apps/nie-mcp/src/index.ts";

/// Client MCP visé par l'installation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum McpTarget {
    /// `<repo>/.mcp.json` — serveur de projet, chemin relatif, versionné avec le dépôt.
    ClaudeCode,
    /// `%APPDATA%/Claude/claude_desktop_config.json` — chemins absolus obligatoires.
    ClaudeDesktop,
}

/// État de l'installation pour un client donné.
#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct McpStatusDto {
    /// Chemin du fichier de configuration visé.
    pub config_path: String,
    /// Vrai si ce fichier existe déjà.
    pub config_exists: bool,
    /// Vrai si `niers-game` y est déjà déclaré.
    pub installed: bool,
    /// Commande actuellement déclarée, si elle l'est.
    pub current_command: Option<String>,
    /// Vrai si le point d'entrée du serveur existe sur le disque.
    pub entrypoint_exists: bool,
    /// Chemin absolu attendu du point d'entrée.
    pub entrypoint: String,
}

/// Résultat d'une écriture de configuration.
#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct McpInstallDto {
    /// Fichier écrit.
    pub config_path: String,
    /// Vrai si une entrée `niers-game` préexistante a été remplacée.
    pub replaced: bool,
    /// Chemin de la sauvegarde `.bak`, si le fichier existait.
    pub backup_path: Option<String>,
}

/// Racine du repo niers, déduite de l'exécutable puis du répertoire courant.
///
/// En dev (`cargo tauri dev`) le binaire est dans `apps/nie-explorer/src-tauri/target/debug` ;
/// en release il est installé ailleurs, et c'est alors le dossier du jeu (= racine du repo sur
/// l'installation Steam) qui fait référence.
fn repo_root() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            if ancestor.join(ENTRYPOINT).is_file() {
                return ancestor.to_path_buf();
            }
        }
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    for ancestor in cwd.ancestors() {
        if ancestor.join(ENTRYPOINT).is_file() {
            return ancestor.to_path_buf();
        }
    }
    cwd
}

/// Chemin du fichier de configuration pour un client donné.
fn config_path(target: McpTarget) -> Result<PathBuf, String> {
    match target {
        McpTarget::ClaudeCode => Ok(repo_root().join(".mcp.json")),
        McpTarget::ClaudeDesktop => {
            let base = if cfg!(windows) {
                std::env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .ok_or_else(|| "APPDATA introuvable".to_string())?
            } else if cfg!(target_os = "macos") {
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .ok_or_else(|| "HOME introuvable".to_string())?;
                home.join("Library").join("Application Support")
            } else {
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .ok_or_else(|| "HOME introuvable".to_string())?;
                home.join(".config")
            };
            Ok(base.join("Claude").join("claude_desktop_config.json"))
        }
    }
}

/// Entrée `mcpServers["niers-game"]` pour un client donné.
///
/// Claude Code lance les serveurs de projet depuis la racine du dépôt : un chemin relatif y
/// reste valable d'une machine à l'autre. Claude Desktop, lui, part d'un répertoire courant
/// arbitraire — il lui faut des chemins absolus.
fn server_entry(target: McpTarget, game_dir: Option<&str>) -> serde_json::Value {
    let root = repo_root();
    let (entry, mut env) = match target {
        McpTarget::ClaudeCode => (ENTRYPOINT.to_string(), serde_json::Map::new()),
        McpTarget::ClaudeDesktop => {
            let mut env = serde_json::Map::new();
            env.insert(
                "NIERS_REPO".to_string(),
                serde_json::Value::String(root.display().to_string()),
            );
            (root.join(ENTRYPOINT).display().to_string(), env)
        }
    };
    if let Some(dir) = game_dir.map(str::trim).filter(|d| !d.is_empty()) {
        env.insert("NIE_GAME_DIR".to_string(), serde_json::Value::String(dir.to_string()));
    }
    serde_json::json!({
        "type": "stdio",
        "command": "bun",
        "args": ["run", entry],
        "env": serde_json::Value::Object(env),
    })
}

/// Décrit l'état d'installation sans rien modifier.
#[tauri::command]
#[specta::specta]
pub fn mcp_status(target: McpTarget) -> Result<McpStatusDto, String> {
    let path = config_path(target)?;
    let entrypoint = repo_root().join(ENTRYPOINT);

    let (config_exists, installed, current_command) = match std::fs::read_to_string(&path) {
        Ok(text) => {
            let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
            let entry = parsed.get("mcpServers").and_then(|m| m.get(SERVER_NAME));
            let command = entry.map(|e| {
                let cmd = e.get("command").and_then(|c| c.as_str()).unwrap_or("");
                let args = e
                    .get("args")
                    .and_then(|a| a.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default();
                format!("{cmd} {args}").trim().to_string()
            });
            (true, entry.is_some(), command)
        }
        Err(_) => (false, false, None),
    };

    Ok(McpStatusDto {
        config_path: path.display().to_string(),
        config_exists,
        installed,
        current_command,
        entrypoint_exists: entrypoint.is_file(),
        entrypoint: entrypoint.display().to_string(),
    })
}

/// Déclare `niers-game` dans la configuration du client visé, en préservant le reste.
#[tauri::command]
#[specta::specta]
pub fn mcp_install(target: McpTarget, game_dir: Option<String>) -> Result<McpInstallDto, String> {
    let path = config_path(target)?;
    let entrypoint = repo_root().join(ENTRYPOINT);
    if !entrypoint.is_file() {
        return Err(format!(
            "point d'entrée du serveur introuvable : {} — le dépôt niers doit être présent à côté du jeu",
            entrypoint.display()
        ));
    }

    let existing = std::fs::read_to_string(&path).ok();
    let backup_path = match &existing {
        Some(text) => {
            let bak = path.with_extension("json.bak");
            std::fs::write(&bak, text).map_err(|e| format!("sauvegarde impossible ({}) : {e}", bak.display()))?;
            Some(bak.display().to_string())
        }
        None => None,
    };

    let mut root: serde_json::Value = match &existing {
        Some(text) if !text.trim().is_empty() => {
            serde_json::from_str(text).map_err(|e| format!("{} n'est pas un JSON valide : {e}", path.display()))?
        }
        _ => serde_json::json!({}),
    };
    if !root.is_object() {
        return Err(format!("{} ne contient pas un objet JSON", path.display()));
    }

    let servers = root
        .as_object_mut()
        .expect("vérifié juste au-dessus")
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    let servers = servers
        .as_object_mut()
        .ok_or_else(|| format!("`mcpServers` de {} n'est pas un objet", path.display()))?;

    let replaced = servers.contains_key(SERVER_NAME);
    servers.insert(
        SERVER_NAME.to_string(),
        server_entry(target, game_dir.as_deref()),
    );

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("création de {} impossible : {e}", parent.display()))?;
    }
    let rendered = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(&path, format!("{rendered}\n")).map_err(|e| format!("écriture de {} impossible : {e}", path.display()))?;

    Ok(McpInstallDto {
        config_path: path.display().to_string(),
        replaced,
        backup_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_entree_claude_code_reste_relative() {
        let entry = server_entry(McpTarget::ClaudeCode, None);
        let args = entry["args"].as_array().expect("args");
        assert_eq!(args[1].as_str(), Some(ENTRYPOINT));
        assert_eq!(entry["command"].as_str(), Some("bun"));
    }

    #[test]
    fn le_dossier_du_jeu_passe_par_l_environnement() {
        let entry = server_entry(McpTarget::ClaudeCode, Some("D:/Jeux/IEVR"));
        assert_eq!(entry["env"]["NIE_GAME_DIR"].as_str(), Some("D:/Jeux/IEVR"));
    }

    #[test]
    fn un_dossier_de_jeu_vide_n_est_pas_ecrit() {
        let entry = server_entry(McpTarget::ClaudeCode, Some("   "));
        assert!(entry["env"].get("NIE_GAME_DIR").is_none());
    }
}
