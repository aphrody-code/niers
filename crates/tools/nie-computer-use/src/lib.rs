//! Native Computer Use boundary for the Windows `nie.exe` and Ghidra surfaces.
//!
//! This crate deliberately exposes probes and intent, not arbitrary shell execution. UI
//! mutation remains delegated to the approved WinClean/Computer Use host.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Complete static reverse-engineering surface, available through Computer Use.
pub mod re {
    pub use nie_re::*;
}

/// Complete live-process tracing surface, available through Computer Use.
pub mod trace {
    pub use nie_trace::*;
}

pub const NIE_PROCESS_NAME: &str = "nie.exe";

/// Safe facade over the read-only `nie-re` + `nie-trace` integration.
pub struct NiersComputerUse;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProcessSnapshot {
    pub process_name: String,
    pub pid: Option<i32>,
    pub module: Option<String>,
    pub module_base: Option<u64>,
    pub module_range: Option<(u64, u64)>,
    pub region_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LiveHit {
    pub address: u64,
    pub rva: Option<u64>,
    pub permissions: String,
}

impl NiersComputerUse {
    #[must_use]
    pub fn find_nie_pid() -> Option<i32> { nie_trace::find_pid_by_name(NIE_PROCESS_NAME) }

    #[must_use]
    pub fn module_range(pid: i32, module: &str) -> Option<(u64, u64)> {
        nie_trace::module_range(pid, module)
    }

    #[must_use]
    pub fn module_regions(pid: i32, module: &str, all: bool) -> Vec<nie_trace::MapEntry> {
        nie_trace::module_regions(pid, module, all)
    }

    pub fn read_memory(pid: i32, address: u64, length: usize) -> Result<Vec<u8>, nie_trace::MemError> {
        nie_trace::read_exact(pid, address, length)
    }

    #[must_use]
    pub fn snapshot(module: Option<&str>) -> ProcessSnapshot {
        let pid = Self::find_nie_pid();
        let (module_base, module_range, region_count) = match (pid, module) {
            (Some(pid), Some(module)) => (nie_trace::find_module_base(pid, module), nie_trace::module_range(pid, module), nie_trace::module_regions(pid, module, false).len()),
            _ => (None, None, 0),
        };
        ProcessSnapshot { process_name: NIE_PROCESS_NAME.into(), pid, module: module.map(str::to_owned), module_base, module_range, region_count }
    }

    pub fn scan_aob(pid: i32, module: &str, pattern: &str, limit: usize) -> Result<Vec<LiveHit>> {
        let parsed = nie_trace::Pattern::parse(pattern).map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let base = nie_trace::find_module_base(pid, module);
        Ok(nie_trace::scan_regions_masked(pid, &nie_trace::module_regions(pid, module, false), base, &parsed, limit)
            .into_iter().map(|hit| LiveHit { address: hit.addr, rva: hit.rva, permissions: hit.perms }).collect())
    }

    #[must_use]
    pub fn catalog_entry(id: &str) -> Option<&'static nie_trace::catalog::Entry> { nie_trace::catalog::find(id) }
}

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

    #[test]
    fn facade_keeps_nie_target_explicit() {
        assert_eq!(NIE_PROCESS_NAME, "nie.exe");
    }

    #[test]
    fn snapshot_is_bounded_to_nie() {
        assert_eq!(NiersComputerUse::snapshot(None).process_name, "nie.exe");
    }
}
