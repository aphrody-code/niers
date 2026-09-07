import { readFileSync, writeFileSync } from "fs";

// ─── 1. AGENTS-UNIFIED.md (Fusion AGENTS.md + CLAUDE.md + Protocole A2A) ───
const agentsMd = readFileSync("AGENTS.md", "utf-8");
const claudeMd = readFileSync("CLAUDE.md", "utf-8");
const a2aMd = readFileSync("docs/A2A-CODEX.md", "utf-8");

const unifiedAgentsContent = `# UNIFIED-AGENTS.md — Single Authority & Operational Contract for All Agents

> **Harmonized on 2026-09-07.**
> This document unites and synthesizes:
> - The multi-agent collaboration contract and arbitration rules (formerly \`AGENTS.md\`)
> - The authoritative technical repository rules, traps, and build gates (formerly \`CLAUDE.md\`)
> - The A2A autonomous wire protocol & messaging system (formerly \`docs/A2A-CODEX.md\`)
>
> Applicable identically to **Claude Code**, **Codex**, and **Gemini / Antigravity CLI (Astra)**.

---

## Part 1. High-Level Multi-Agent Collaboration & Operating Mode

### 1.1 Urgency Mode (YOLO & Autonomous Execution)
- **Ship.** Every change is committed, pushed, deployed to production, tested there, and chained straight into the next one. One continuous motion, not four disjointed decisions.
- **Auto-Allow & Zero Confirmation:** No agent pauses to ask permission before \`git push\`, \`cargo build --release\`, \`systemctl restart\`, \`nginx -t + reload\`, or \`vercel --prod\`. The legacy 2026-09-05 list of "six moves needing a go" is formally repealed.
- **Two Invariant Constraints:**
  1. **A deploy is finished only when the LIVE service has returned a number**, not when \`systemctl\` says \`active\`.
  2. **Destroying data stays deliberate:** say so immediately, and maintain a rollback path. Never use data deletion as a shortcut around a failing test.

### 1.2 Language Convention
- **English** for all machine-readable and technical identifiers: file & directory names, variables, functions, types, fields, constants, modules, **URLs, route patterns, query parameters, site slugs, public JSON keys**, CLI commands, DB tables/columns, and documentation written for agents.
- **French** strictly reserved for prose addressed to the user (reports, summaries, explanations).
- **Product Names Frozen:** Azalée, Aphrody, Inacord, nie, \`niers\`, the \`nie-*\` crates, and the \`inagle_\` table prefix.

### 1.3 Never Overwrite Another Agent
1. **Announce Scope Before Writing:** Post a \`claim: <paths>\` before touching files outside your current batch.
2. **Signature Stability:** Do not break callers in parallel worktrees. Extend shared signatures with \`#[derive(Default)]\` option structs and preserve delegations.
3. **One Author Per Batch:** Each batch is one commit containing its measured gate numbers.
4. **Targeted PIDs:** \`pkill -f\` is strictly forbidden (it terminates active agent sessions). Target explicit PIDs.

---

## Part 2. The Wire Protocol (A2A Codex/Claude/Astra)

### 2.1 Wire Mechanics
- CLI Command: \`aphrody a2a tick --iteration <n> --side <source> --peer <target> --kind <fact|ping> --subject "<type>: <topic>" --body "<data>"\`
- Subject Prefixes:
  - \`goal:\` Order or direction
  - \`claim:\` Scope reservation (mandatory before editing)
  - \`fact:\` Concrete measurement or verification (must carry numbers)
  - \`block:\` Arbitration requirement
  - \`done:\` Finished batch with proof & gate metrics
- Inbox paths: \`.coord/inbox-from-<agent>.jsonl\`

---

## Part 3. Authoritative Repository Rules & Build Gates

### 3.1 Build & Test Gates
- **Master Gate:** \`cargo clippy -p <crate> --lib --tests\` with **0 warnings**.
- **Bin-Only Crates (7 crates):** For \`nie-bench\`, \`nie-cli\`, \`nie-editor\`, \`nie-game\`, \`nie-headless\`, \`nie-model-serve\`, \`nie-play\`, use:
  \`\`\`bash
  cargo clippy -p <crate> --bins --tests
  \`\`\`
- **Independent Workspace Warning:** \`apps/inacord/src-tauri\` is a **separate** Cargo workspace. \`cargo check --workspace\` at the root will **never** validate it. It must be built via \`bun run tauri build\` or directly within its folder.
- **Never run \`cargo build --workspace --all-targets\`**: The disk is saturated (>92%) and full targets exhaust disk space.
- **TypeScript Gate:** \`bun run typecheck\` (validates the 5 workspaces).

### 3.2 Polyglot Architecture
- **Rust (\`crates/\`):** The primary CLI (\`niers\`), GUI core, WebAssembly, formats, rendering, and RE.
- **Bun/TypeScript (\`packages/\`, \`apps/\`):** \`packages/inacord-ui\` shared UI, MCP server (\`apps/nie-mcp\`), web serving (\`apps/nie-web\`), and pipelines.
- **C++ (\`src/\`):** Decompiled \`nie.exe\` game rebuild, native Assimp/Bullet dependencies, accessed via \`niers cpp\`.
- **C# (\`csharp/\`):** Legacy format dumps, memory inspection, texture converters, accessed via \`niers cs\`.

### 3.3 Critical Traps & Anti-Patterns
1. **Bun Preload Trap:** \`bunfig.toml\` preloads \`register.ts\` which binds \`libnie_ffi.dll\`. If \`nie-ffi\` is missing or locked, all Bun commands fail. Fix: \`cargo build -p nie-ffi\`.
2. **False Greens:** Never accept "0 passed" or a bare exit code as success. Count lines, items, or HTTP responses.
3. **Windows sed Trap:** Never use \`sed -i\` on code (it corrupts backslashes and fails silently). Use structured edits.
4. **VFS Initialization:** Always pass \`<root>/data\` to \`Vfs::init()\`. Use the explicit \`NIE_GAME_DIR\` pointing to the Steam installation.
`;

writeFileSync("docs/UNIFIED-AGENTS.md", unifiedAgentsContent, "utf-8");
console.log("Wrote docs/UNIFIED-AGENTS.md");
