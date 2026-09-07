# CLAUDE.md — Claude Code Operational Guidelines

See [AGENTS.md](AGENTS.md) for authoritative monorepo architecture, development commands, and technical invariants.

## Specific Operating Rules for Claude Code
- **Full Urgency & Autonomous Execution:** Execute tasks without interrupting for confirmations. Follow the single action chain: code -> test -> commit -> push -> deploy -> verify live.
- **Language Contract:** Think and write code, types, URLs, routes, and JSON keys in English. Only prose directly addressed to the user is in French.
- **Strict Quality Gates:** Run `cargo clippy -p <crate> --lib --tests` (or `--bins --tests` for bin-only crates) before completing any task.
- **Process Protection:** Never invoke `pkill -f` or terminate processes by wildcards. Always target explicit PIDs.
