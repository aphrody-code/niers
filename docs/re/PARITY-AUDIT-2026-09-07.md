# Reverse-engineering parity audit

## Canonical source

The canonical repository is [aphrody-code/nie](https://github.com/aphrody-code/nie). The local
checkout is `C:\Users\aphro\niers`, branch `main`, remote `origin` points to that repository.
The audited `nie-re`, `nie-dump`, `nie-trace`, and `nie-computer-use` paths have no local diff
against `origin/main`.

## Consumer map

| Local path | Consumer | Rust equivalent | Status |
|---|---|---|---|
| `crates/forge/nie-re/src/` | `nie-cli re`, RE examples, SQLite KB | `nie_re::{pdata,rtti,vtable,disasm,recover,...}` | keep |
| `crates/forge/nie-dump/src/` | dump census and AOB scans | `nie_re::dump` re-export of `nie_dump` | keep |
| `crates/forge/nie-trace/src/` | `nie mem`, `nie-mem`, `nie-edit` | typed Windows/Wine memory, scans, maps | keep; add Windows parity |
| `crates/tools/nie-computer-use/src/` | `computer-use` probe and typed RE bridge | `NiersComputerUse` + `ReSession` | keep; identity and bounds verified |

## Gates

- `cargo test -p nie-re --lib --locked`: 72 passed, 0 failed, 1 ignored.
- `cargo test -p nie-trace --tests --locked`: 43 passed, 0 failed; `self_mem` is Linux-only,
  so this is not proof of a Windows live-memory success path.
- `cargo test -p nie-computer-use --tests --locked`: 9 passed, 0 failed.

## Final decision

Keep the Rust implementation and remove no existing module. The typed read-only session now carries
executable hash, size, image base, SQLite `binary_id`, RVA/VA, backend, operation and evidence artifact.
The bounded verifier is `scripts/verify-computer-use-re-trace.ps1`. Ghidra MCP identity and a live
Windows process remain environment-dependent evidence and are intentionally not claimed by offline
gates; writes, EAC patches, recipes and process launch remain separate explicit capabilities.
