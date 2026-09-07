# UNIFIED-PLAN.md — Master Execution Plan & Operational Roadmap

> **Consolidated on 2026-09-07.**
> Synthesizes:
> 1. The Supreme Objective: **PLAN-SITE-ULTIME** (Master coverage towards `manquant = 0`)
> 2. The Current Sprint: **CODEX-JOUR-UNIQUE** (7 priority blocks executed in 1 day)
> 3. The Switchover Horizon: **PLAN.md** (Azalée Vercel / Aphrody `aphrody.com` / Inacord unification)
> 4. The Core Engine & Binary Production: **PLAN-MOTEUR-FORGE** (Byte-exactness & RE)

---

## 1. Hierarchy & Guiding Principle

```
                    [ PLAN-SITE-ULTIME.md ]
                     (Le Cap: Couverture 100%, manquant = 0)
                               │
            ┌──────────────────┴──────────────────┐
            ▼                                     ▼
     [ PLAN-SEMAINE (PLAN.md) ]          [ FORGE & MOTEUR (docs/PLAN.md) ]
  (Bascule Vercel / Aphrody.com)         (nie.exe byte-exact, 92.24% .text)
            │
            ▼
 [ CODEX-JOUR-UNIQUE (Sprint Actif) ]
  (7 Blocs ordonnés, mesurés et sans confirmation)
```

**Core Law:** Any task that does not advance the coverage matrix or increase byte-exactness does not advance the project.

---

## 2. Immediate Execution Roadmap (The 7 Sprints from CODEX-JOUR-UNIQUE)

| Block | Target Scope | Master Gate Metric | Status |
| :--- | :--- | :--- | :--- |
| **Bloc 1** | **Typecheck & Monorepo Bun** | `bun run typecheck` = **0 err** on 5 workspaces | Fix `@rosegriffon/mcp` & `cron` exports |
| **Bloc 2** | **Wiki Serverless Isolation** | `rg -l 'bun:sqlite\|node:fs' apps/azalee packages/azalee` = **0** | Clean, preview Gate 1 (/chara >= 50 links) |
| **Bloc 3** | **Payload & ISR Optimization** | `/chara` < 250 Ko in `br`, 0 img without `srcset` | Next.js ISR cache + CDN webp |
| **Bloc 4** | **Brand Separation (aphrody-dev)**| Zero mention of Rose Griffon in Inacord/nie-web | Compliant (verified in `skeleton.tsx` / `tauri.conf`) |
| **Bloc 5** | **Production Rebuild `nie-site`** | Rebuild binary to resolve 500 error on WAL mode | Deploy release binary to VPS (:8085) |
| **Bloc 6** | **Hardening & Performance** | Moka cache TTL, Criterion baseline, docs freeze | Benchmarks and locked configs |
| **Bloc 7** | **Couverture Ultime (583 caps)** | `manquant = 0` and `partiel = 0` on API matrix | Bridge remaining 27 endpoints in `nie-site` |

---

## 3. Production Architecture & Deployment Topology

- **Azalée (`azalee.rosegriffon.fr`)**: Next.js 15 Serverless deployed on Vercel, querying Supabase Cloud directly.
- **Aphrody (`aphrody.com`)**: Native Axum server in Rust (`nie-site`), rendering game UI DA in < 50ms TTFB.
- **Inacord (`packages/inacord-ui`)**: Unified frontend mounted by both Tauri (desktop) and Vite/nie-web (browser) via `packages/asset-source`.
- **The Forge (`nie-forge`)**: Verified at **74.00% file coverage** and **92.24% of `.text`** byte-identical to `nie.exe` (`b1fa04ea3658...`).

---

## 4. Verification Checklist & Gate Ledger

1. `bun run typecheck` (TypeScript verification)
2. `cargo clippy -p <crate> --lib --tests` (0 warnings)
3. `cargo check --workspace --tests` (Workspace consistency)
4. `scripts/e2e-site.sh` (Live API coverage validation)
5. Live service returns verified count and response payload (not just 200/active).

## 5. RE / Computer Use parity gate

The RE work follows one canonical chain:

```text
local executable + hash
  -> Ghidra / CodeBrowser evidence
  -> nie-re + nie-index (static analysis and SQLite)
  -> nie-trace (bounded live reads/scans)
  -> nie-computer-use (typed read-only orchestration)
```

The existing Rust crates are kept; no duplicate implementation is to be removed. The migration
target is the boundary: a session must carry executable hash, image base, explicit SQLite
`binary_id`, RVA/VA, backend, operation and evidence artifact. Writes, EAC patches, recipes and
process launch stay outside the default agent surface.

The gate is not complete until the parity tests cover PE + SQLite fixtures, Ghidra CSV/schema and
MCP handshake, Windows live memory, size/permission limits, and rejection of a mismatched build.
The detailed inventory is [`docs/re/PARITY-AUDIT-2026-09-07.md`](re/PARITY-AUDIT-2026-09-07.md).
