# AGENTS.md — Master Technical Specification & Operational Contract for All Agents

> Standardized instructions and operational ground truth for all AI coding agents working on `niers`
> (OpenAI Codex, Anthropic Claude Code, Google Gemini / Antigravity CLI).

---

## 1. Project Identity, Scope & Commercial Mandate
- **Repository:** `aphrody-code/nie` (working title: `niers`).
- **Objective:** Pure Rust native implementation and byte-exact reverse engineering of *Inazuma Eleven: Victory Road* (`nie.exe`).
- **Commercial Agreement:** Official Commercial Agreement N° RG-L5-VR-2026-001 between Rose Griffon (Level 5 France) and LEVEL-5 Inc.
- **Language Policy:**
  - **English:** All code, filenames, struct/fn names, DB schemas, route patterns, public API keys, and agent-facing documentation.
  - **French:** Exclusively reserved for human user prose (reports, summaries, explanations).
  - **Frozen Product Names:** Azalée, Aphrody, Inacord, nie, `niers`, `nie-*` crates, `inagle_` table prefixes.

---

## 2. Operating Doctrine: Full Autonomy & Urgency
- **Autonomous Lead Mode:** Ship continuously end-to-end. Do not wait for human confirmation.
- **Single Action Chain:** `commit` -> `push` -> `build --release` -> `deploy/restart` -> **interrogate live service and count** -> next task immediately.
- **Repealed Permissions:** No prior consent needed for `git push`, `cargo build --release`, `systemctl restart`, `nginx -t + reload`, or `vercel --prod`.
- **Two Invariant Constraints:**
  1. **A deployment is complete only when the LIVE service returns a valid measurement** (e.g. non-zero count, verified response payload), never when `systemctl` merely says `active`.
  2. **Destroying data remains deliberate:** announce it immediately and preserve a rollback path. Never drop tables or delete files outside `target/` as a shortcut around a failing test.

---

## 3. Multi-Agent Coordination & Safety Protocol
When multiple agents work concurrently in parallel worktrees:
1. **Scope Reservation (`claim:`):** Announce files before editing (`claim: <paths>`).
2. **Signature Stability:** Never alter shared public signatures without backward compatibility. Extend options structs with `#[derive(Default)]` and preserve delegations.
3. **Targeted Process Management:** `pkill -f` is strictly forbidden (it drops active agent harnesses). Always terminate specific PIDs.
4. **No Destructive Global Commands:** Do not run `git reset --hard` or `git checkout --` on paths outside your claimed batch.
5. **Completion Report (`done:`):** Complete every batch with the verified test gate, live metrics, and commit message containing concrete numbers.

---

## 4. Architecture & Polyglot Monorepo Structure

### 4.1 Rust Codebase (Primary: 781 files, ~307,600 LoC, 38 Crates)
- **`crates/engine/*` (523 files, ~183k LoC):** Core runtime, binary formats (`nie-formats`: CPK, G4TX, RDBN, T2B), data models (`nie-data`), 3D renderer (`nie-render3d`), and Lua 5.2 host (`nie-lua`).
- **`crates/forge/*` (62 files, ~28k LoC):** Binary production and exact reassembly against `nie.exe` (`nie-forge`, `nie-pe`, `nie-asm`, `nie-re`, `nie-trace`). File coverage: **74.00%**, `.text` coverage: **92.24%** (`b1fa04ea3658...`).
- **`crates/tools/*` (104 files, ~67k LoC):** Axum web server (`nie-site` serving `aphrody.com`), unified CLI dispatcher (`nie-cli`), and model server (`nie-model-serve`).
- **`crates/archive/*`:** Reference-only archives out of the build (`nie-engine`, `nie-rs`).

### 4.2 Web & Desktop Applications (Bun / TypeScript Monorepo)
- **`packages/inacord-ui`:** Shared UI component library mounting 45 primitives and `shell/{main-menu,inacord}`. Zero `@tauri-apps` dependencies.
- **`packages/asset-source`:** Asset access abstraction contract (`AssetSource`) consumed identically by web and desktop hosts.
- **`apps/inacord`:** Desktop explorer/editor (Tauri host). **Note:** `apps/inacord/src-tauri` is an independent Cargo workspace.
- **`apps/nie-web`:** Browser host (Vite) for `inacord-ui`, served in production by `nie-site`.
- **`apps/azalee`:** Next.js 15 serverless wiki deployed on Vercel via Supabase Cloud.

### 4.3 Auxiliary Stacks
- **C++ (`src/`):** Game runtime recreation (`iecode_core`, `src/decomp/` MSVC 14.44 decompilation), accessed via `niers cpp`.
- **C# (`csharp/`):** Format dumping and memory tools, accessed via `niers cs`.

---

## 5. Development Gates & Verification Commands

```bash
# Core Rust Library Gate (0 warnings required)
cargo clippy -p <crate> --lib --tests

# Bin-Only Crates Gate (nie-bench, nie-cli, nie-editor, nie-game, nie-headless, nie-model-serve, nie-play)
cargo clippy -p <crate> --bins --tests

# Independent Tauri Desktop Workspace Gate
cd apps/inacord/src-tauri && cargo clippy --bins --tests

# Full Workspace Check (DO NOT use cargo build --workspace --all-targets)
cargo check --workspace --tests

# Monorepo TypeScript Gate (all 5 workspaces)
bun run typecheck
```

- **NEVER run `cargo build --workspace --all-targets`:** Disk space is restricted (>92% full); building all targets will exhaust storage.
- **Formatting Policy:** Do not run `cargo fmt --all` or `cargo fmt -p <crate>` across unedited files. Format only modified files to avoid polluting git diffs.

---

## 6. Critical Technical Traps & Invariants

1. **Bun FFI Preload Trap:** `bunfig.toml` preloads `./packages/nie-plugin/src/register.ts`, which attempts to `dlopen` `libnie_ffi.dll`. If unbuilt or locked by a lingering Bun process, all `bun` and `bunx` commands fail immediately with `ERR_DLOPEN_FAILED`. Fix: build FFI via `cargo build -p nie-ffi`.
2. **Game VFS Resolution:** Requires explicit `NIE_GAME_DIR` pointing to the Steam installation directory (`INAZUMA ELEVEN Victory Road`). `Vfs::init()` requires `<root>/data`, not the root folder itself.
3. **No `sed -i` on Source Code:** Under Windows/MSYS, `sed -i` eats backslashes and fails silently on pattern mismatches. Use structured file modifications.
4. **False Greens:** Never accept "0 passed" or a bare exit code 0 as verification. A test without assertions or a page returning HTTP 200 with an empty body is an undetected failure. Count lines, returned records, or links.
