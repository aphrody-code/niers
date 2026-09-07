//! Native Computer Use boundary for the Windows `nie.exe` and Ghidra surfaces.
//!
//! This crate deliberately exposes probes and intent, not arbitrary shell execution. UI
//! mutation remains delegated to the approved WinClean/Computer Use host.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::OpenFlags;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
pub const MAX_READ_BYTES: usize = 1024 * 1024;
pub const MAX_SCAN_HITS: usize = 4096;

/// Immutable identity of the executable attached to a static RE session.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReTarget {
    pub executable: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
    pub binary_id: i64,
    pub image_base: u64,
    pub arch: String,
    pub bits: u16,
}

/// Minimal provenance attached to a session result.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReProvenance {
    pub executable: PathBuf,
    pub sha256: String,
    pub binary_id: i64,
    pub image_base: u64,
    pub backend: String,
    pub operation: String,
    pub artifact: Option<PathBuf>,
}

/// Read-only static session. The executable and SQLite row must have the same hash and size.
pub struct ReSession {
    target: ReTarget,
    validated_bytes: Vec<u8>,
}

impl ReSession {
    /// Open a session after validating the executable against the RE database.
    pub fn open(
        executable: impl Into<PathBuf>,
        database: impl AsRef<Path>,
        expected_binary_id: Option<i64>,
    ) -> Result<Self> {
        let executable = executable.into();
        let bytes = std::fs::read(&executable)
            .with_context(|| format!("read executable {}", executable.display()))?;
        let sha256 = hex::encode(Sha256::digest(&bytes));
        let conn =
            rusqlite::Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .with_context(|| "open RE database read-only")?;
        let row = conn
            .query_row(
                "SELECT id, base_addr, arch, bits, size FROM binary WHERE sha256=?1",
                [&sha256],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                    ))
                },
            )
            .with_context(|| format!("binary hash {sha256} absent from RE database"))?;
        anyhow::ensure!(
            expected_binary_id.is_none_or(|id| id == row.0),
            "binary_id mismatch: expected {:?}, database has {}",
            expected_binary_id,
            row.0
        );
        anyhow::ensure!(
            row.4 >= 0 && row.4 as u64 == bytes.len() as u64,
            "binary size mismatch: database {}, file {}",
            row.4,
            bytes.len()
        );
        anyhow::ensure!(
            row.3 > 0 && row.3 <= u16::MAX as i64 && row.1 >= 0,
            "invalid binary metadata"
        );
        Ok(Self {
            target: ReTarget {
                executable,
                sha256,
                size_bytes: bytes.len() as u64,
                binary_id: row.0,
                image_base: row.1 as u64,
                arch: row.2,
                bits: row.3 as u16,
            },
            validated_bytes: bytes,
        })
    }

    #[must_use]
    pub fn target(&self) -> &ReTarget {
        &self.target
    }

    #[must_use]
    pub fn provenance(
        &self,
        backend: impl Into<String>,
        operation: impl Into<String>,
        artifact: Option<PathBuf>,
    ) -> ReProvenance {
        ReProvenance {
            executable: self.target.executable.clone(),
            sha256: self.target.sha256.clone(),
            binary_id: self.target.binary_id,
            image_base: self.target.image_base,
            backend: backend.into(),
            operation: operation.into(),
            artifact,
        }
    }

    pub fn rva_to_va(&self, rva: u64) -> Result<u64> {
        anyhow::ensure!(
            rva < self.target.size_bytes,
            "RVA 0x{rva:x} outside indexed image"
        );
        self.target
            .image_base
            .checked_add(rva)
            .context("RVA to VA overflow")
    }

    pub fn va_to_rva(&self, va: u64) -> Result<u64> {
        anyhow::ensure!(va >= self.target.image_base, "VA 0x{va:x} below image base");
        let rva = va - self.target.image_base;
        anyhow::ensure!(
            rva < self.target.size_bytes,
            "VA 0x{va:x} outside indexed image"
        );
        Ok(rva)
    }

    /// Read a bounded slice from the reference executable.
    pub fn read_file(&self, offset: u64, length: usize) -> Result<Vec<u8>> {
        anyhow::ensure!(
            length <= MAX_READ_BYTES,
            "read length {length} exceeds {MAX_READ_BYTES}"
        );
        anyhow::ensure!(
            offset
                .checked_add(length as u64)
                .is_some_and(|end| end <= self.target.size_bytes),
            "read outside indexed image"
        );
        Ok(self.validated_bytes[offset as usize..offset as usize + length].to_vec())
    }
}

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
    pub fn find_nie_pid() -> Option<i32> {
        nie_trace::find_pid_by_name(NIE_PROCESS_NAME)
    }

    #[must_use]
    pub fn module_range(pid: i32, module: &str) -> Option<(u64, u64)> {
        nie_trace::module_range(pid, module)
    }

    #[must_use]
    pub fn module_regions(pid: i32, module: &str, all: bool) -> Vec<nie_trace::MapEntry> {
        nie_trace::module_regions(pid, module, all)
    }

    pub fn read_memory(
        pid: i32,
        address: u64,
        length: usize,
    ) -> Result<Vec<u8>, nie_trace::MemError> {
        if length > MAX_READ_BYTES {
            return Err(nie_trace::MemError::InvalidLength {
                length,
                max: MAX_READ_BYTES,
            });
        }
        nie_trace::read_exact(pid, address, length)
    }

    #[must_use]
    pub fn snapshot(module: Option<&str>) -> ProcessSnapshot {
        let pid = Self::find_nie_pid();
        let (module_base, module_range, region_count) = match (pid, module) {
            (Some(pid), Some(module)) => (
                nie_trace::find_module_base(pid, module),
                nie_trace::module_range(pid, module),
                nie_trace::module_regions(pid, module, false).len(),
            ),
            _ => (None, None, 0),
        };
        ProcessSnapshot {
            process_name: NIE_PROCESS_NAME.into(),
            pid,
            module: module.map(str::to_owned),
            module_base,
            module_range,
            region_count,
        }
    }

    pub fn scan_aob(pid: i32, module: &str, pattern: &str, limit: usize) -> Result<Vec<LiveHit>> {
        anyhow::ensure!(
            limit <= MAX_SCAN_HITS,
            "scan limit {limit} exceeds {MAX_SCAN_HITS}"
        );
        let parsed = nie_trace::Pattern::parse(pattern)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let base = nie_trace::find_module_base(pid, module);
        Ok(nie_trace::scan_regions_masked(
            pid,
            &nie_trace::module_regions(pid, module, false),
            base,
            &parsed,
            limit,
        )
        .into_iter()
        .map(|hit| LiveHit {
            address: hit.addr,
            rva: hit.rva,
            permissions: hit.perms,
        })
        .collect())
    }

    #[must_use]
    pub fn catalog_entry(id: &str) -> Option<&'static nie_trace::catalog::Entry> {
        nie_trace::catalog::find(id)
    }
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
            other => {
                anyhow::bail!("unknown Computer Use surface `{other}`; expected nie-exe or ghidra")
            }
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

fn default_ghidra_url() -> String {
    "http://127.0.0.1:8080/mcp".into()
}

/// Probe a target without launching, clicking, or writing anything.
pub fn probe(request: &ProbeRequest) -> Result<ProbeResult> {
    match request.surface {
        Surface::NieExe => {
            let target = request.executable.as_deref().unwrap_or("nie.exe");
            let available = Path::new(target).exists();
            Ok(ProbeResult {
                surface: Surface::NieExe,
                available,
                target: target.into(),
                detail: if available {
                    "executable exists"
                } else {
                    "executable not found"
                }
                .into(),
            })
        }
        Surface::Ghidra => {
            let url = &request.ghidra_url;
            let response = std::process::Command::new("curl")
                .args([
                    "--silent",
                    "--show-error",
                    "--max-time",
                    "3",
                    "-o",
                    "NUL",
                    "-w",
                    "%{http_code}",
                    url,
                ])
                .output()
                .with_context(|| "curl is required for the non-destructive Ghidra probe")?;
            let code = String::from_utf8_lossy(&response.stdout).trim().to_owned();
            let available = matches!(code.as_str(), "200" | "400" | "401" | "404" | "405");
            Ok(ProbeResult {
                surface: Surface::Ghidra,
                available,
                target: url.clone(),
                detail: format!("HTTP status {code}"),
            })
        }
    }
}

pub fn probe_json(request: &ProbeRequest) -> Result<String> {
    serde_json::to_string_pretty(&probe(request)?).context("serialize Computer Use probe")
}

/// Parse the stable CLI spelling and return the JSON probe response.
pub fn probe_cli(surface: &str, executable: Option<String>, ghidra_url: String) -> Result<String> {
    let request = ProbeRequest {
        surface: surface.parse()?,
        executable,
        ghidra_url,
    };
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
        let result = probe(&ProbeRequest {
            surface: Surface::NieExe,
            executable: Some("does-not-exist-nie.exe".into()),
            ghidra_url: default_ghidra_url(),
        })
        .unwrap();
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

    #[test]
    fn session_binds_hash_binary_id_and_addresses() {
        let dir = std::env::temp_dir().join(format!("nie-cu-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("nie.exe");
        let db_path = dir.join("re.sqlite");
        let bytes = b"reproducible-fixture";
        std::fs::write(&exe, bytes).unwrap();
        let sha = hex::encode(Sha256::digest(bytes));
        let db = nie_index::Db::open(&db_path).unwrap();
        let id = db
            .upsert_binary(
                "nie.exe",
                &sha,
                "x86_64",
                64,
                0x140000000,
                bytes.len() as i64,
                None,
                None,
            )
            .unwrap();
        drop(db);
        let session = ReSession::open(&exe, &db_path, Some(id)).unwrap();
        assert_eq!(session.target().binary_id, id);
        assert_eq!(session.rva_to_va(3).unwrap(), 0x140000003);
        assert_eq!(session.va_to_rva(0x140000003).unwrap(), 3);
        assert_eq!(session.read_file(0, 4).unwrap(), b"repr");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn session_rejects_wrong_build() {
        let dir = std::env::temp_dir().join(format!("nie-cu-session-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("nie.exe");
        let db_path = dir.join("re.sqlite");
        std::fs::write(&exe, b"actual").unwrap();
        let db = nie_index::Db::open(&db_path).unwrap();
        db.upsert_binary("nie.exe", "wrong", "x86_64", 64, 0x140000000, 6, None, None)
            .unwrap();
        drop(db);
        assert!(ReSession::open(&exe, &db_path, None).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn live_reads_reject_unbounded_lengths_before_backend() {
        let error = NiersComputerUse::read_memory(0, 0, MAX_READ_BYTES + 1).unwrap_err();
        assert!(matches!(error, nie_trace::MemError::InvalidLength { .. }));
    }

    #[test]
    fn live_scans_reject_unbounded_hit_limits() {
        assert!(NiersComputerUse::scan_aob(0, "nie.exe", "90", MAX_SCAN_HITS + 1).is_err());
    }
}
