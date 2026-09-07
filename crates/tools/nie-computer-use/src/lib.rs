//! Native Computer Use boundary for the Windows `nie.exe` and Ghidra surfaces.
//!
//! This crate deliberately exposes probes and intent, not arbitrary shell execution. UI
//! mutation remains delegated to the approved WinClean/Computer Use host.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    NieExe,
    Ghidra,
}

impl FromStr for Surface {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "nie-exe" | "nie_exe" => Ok(Self::NieExe),
            "ghidra" => Ok(Self::Ghidra),
            other => anyhow::bail!("unknown Computer Use surface `{other}`; expected nie-exe or ghidra"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProbeRequest {
    pub surface: Surface,
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default = "default_ghidra_url")]
    pub ghidra_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProbeResult {
    pub surface: Surface,
    pub available: bool,
    pub target: String,
    pub detail: String,
}

fn default_ghidra_url() -> String { "http://127.0.0.1:8080/mcp".into() }

/// Probe a target without launching, clicking, or writing anything.
pub fn probe(request: &ProbeRequest) -> Result<ProbeResult> {
    match request.surface {
        Surface::NieExe => {
            let target = request.executable.as_deref().unwrap_or("nie.exe");
            let available = Path::new(target).exists();
            Ok(ProbeResult { surface: Surface::NieExe, available, target: target.into(), detail: if available { "executable exists" } else { "executable not found" }.into() })
        }
        Surface::Ghidra => {
            let url = &request.ghidra_url;
            let response = std::process::Command::new("curl")
                .args(["--silent", "--show-error", "--max-time", "3", "-o", "NUL", "-w", "%{http_code}", url])
                .output()
                .with_context(|| "curl is required for the non-destructive Ghidra probe")?;
            let code = String::from_utf8_lossy(&response.stdout).trim().to_owned();
            let available = matches!(code.as_str(), "200" | "400" | "401" | "404" | "405");
            Ok(ProbeResult { surface: Surface::Ghidra, available, target: url.clone(), detail: format!("HTTP status {code}") })
        }
    }
}

pub fn probe_json(request: &ProbeRequest) -> Result<String> {
    serde_json::to_string_pretty(&probe(request)?).context("serialize Computer Use probe")
}

/// Parse the stable CLI spelling and return the JSON probe response.
pub fn probe_cli(surface: &str, executable: Option<String>, ghidra_url: String) -> Result<String> {
    let request = ProbeRequest { surface: surface.parse()?, executable, ghidra_url };
    probe_json(&request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_target_is_ghidra_mcp() {
        let request: ProbeRequest = serde_json::from_str(r#"{"surface":"ghidra"}"#).unwrap();
        assert_eq!(request.ghidra_url, "http://127.0.0.1:8080/mcp");
    }

    #[test]
    fn missing_executable_is_reported_without_spawn() {
        let result = probe(&ProbeRequest { surface: Surface::NieExe, executable: Some("does-not-exist-nie.exe".into()), ghidra_url: default_ghidra_url() }).unwrap();
        assert!(!result.available);
        assert_eq!(result.detail, "executable not found");
    }

    #[test]
    fn cli_surface_spellings_are_stable() {
        assert_eq!("nie-exe".parse::<Surface>().unwrap(), Surface::NieExe);
        assert_eq!("ghidra".parse::<Surface>().unwrap(), Surface::Ghidra);
    }
}
