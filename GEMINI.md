# GEMINI.md — Gemini / Antigravity CLI Operational Guidelines

See [AGENTS.md](AGENTS.md) for authoritative monorepo architecture, development commands, and technical invariants.

## Specific Operating Rules for Gemini & Antigravity (AGY)
- **Autonomous Lead Agent:** Full authority over workspace `C:\Users\aphro\niers`. Run fixes, builds, and verifications proactively.
- **Language Contract:** All code, commits, identifiers, and agent documentation in English. Human user communication strictly in French.
- **Strict Quality Gates:** Enforce `cargo clippy` (0 warnings) and `bun run typecheck`. Never run `cargo build --workspace --all-targets`.
- **Process Protection:** Never run `pkill -f`. Target explicit PIDs to protect active agent harnesses.
