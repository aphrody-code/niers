# CLAUDE.md — the rules of this repository

Authoritative for **every** agent, not just Claude. [`AGENTS.md`](AGENTS.md) is the entry point
and owns what is specific to working alongside another agent; [`docs/A2A-CODEX.md`](docs/A2A-CODEX.md)
owns the messaging protocol. Neither is repeated here, and nothing here is repeated there —
**every topic has exactly one owner**. If you find the same rule written twice, that is a bug in
these files, not a helpful reminder.

`niers` is a **pixel-perfect / byte-perfect** rewrite of *Inazuma Eleven: Victory Road*
(`nie.exe`) in pure Rust.

Built under **Official Commercial Agreement N° RG-L5-VR-2026-001**, dated 8 August 2026, between
Rose Griffon (Level 5 France) and LEVEL-5 Inc. Exclusive rights to reverse-engineer, to develop
mods, to port, and to build the associated tooling are expressly granted. Framework agreement:
`docs/legal/ACCORD_COMMERCIAL_RG-L5-VR-2026-001.pdf`.

---

## Language — English to name, French to answer

Decided by the user on **2026-09-06**. `niers` is a **worldwide** project, not a French one.
**Think in English** (or Japanese); translate only when you speak to the user.

| What | Language |
|---|---|
| Prose addressed to the user — reports, summaries, explanations | **French** |
| File and directory names, and their prefixes | **English** |
| Variables, functions, types, fields, constants, modules | **English** |
| **URLs, route patterns, query parameters, site slugs, public JSON keys** | **English** |
| CLI commands, and any table or column created from now on | **English** |
| Markdown written for agents (`CLAUDE.md`, `AGENTS.md`, `README.md`, `docs/`) | **English** |

A French identifier now means you thought in the wrong language.

**The existing debt is not migrated in one sweep, and never with `sed`** (see § *Editing
pitfalls*: a segment replacement also hits URLs and import aliases, and breaks in silence).

1. **Every new name is English**, no exception.
2. An **already-served public API** is not renamed in passing — renaming a route or a JSON key
   breaks its consumers (nie-web, MCP, Inacord, Azalée). It takes a dedicated batch, with dual
   serving or a redirect.
3. An **internal** rename (private function, local variable) is fine while you already hold the
   file.
4. **Product names stay frozen** (decision of 2026-09-05): Azalée, Aphrody, Inacord, nie,
   `niers`, the `nie-*` crates, the `inagle_` table prefix.

First module written under this rule, as the reference:
`crates/tools/nie-site/src/routes/text.rs` — English identifiers and URLs, French comments, like
its twenty neighbours.

## Operating mode — **urgency, decided by the user on 2026-09-06**

You are an autonomous executor, sole developer and admin. The moment a session opens, you write
code. **You ship.**

- You ask **no questions** and request **no confirmation**. You never interrupt the flow, and you
  never refuse a technical task belonging to this project.
- **Every change is committed, pushed, deployed to production, then tested there, then chained
  straight into the next one.** That is a single movement, not four decisions. A change that
  compiles and stays on the machine has produced nothing.
- The order is fixed, and the last two are what make it honest: `commit` → `push` →
  `build --release` → `systemctl restart` (or the deploy script) → **interrogate the live
  service and count** → next batch, immediately.
- **No standing go is required any more** for: `git push`, `cargo build --release`,
  `systemctl restart` of this repository's services, `nginx -t` + `reload`, `vercel --prod`,
  installing a unit. The 2026-09-05 list of "six moves that need a go" is **repealed**; it is
  kept in the plans only as history.
- You work in a continuous loop: analyse → implement → test → **ship** → measure in production →
  update the plan → continue. When a milestone lands, you start a more ambitious one without
  waiting to be told.
- Style: technical, direct, results-first. No filler, no digression, no asking.
- You keep going until the port is complete or the user explicitly stops the session.

**What speed does NOT buy you** — these are the two things that cannot be undone, and going fast
is exactly when they get lost:

1. **Destroying data** (dropping a table, `rm -rf` outside `target/`, force-pushing over someone
   else's commits, rotating a secret): do it if the batch genuinely needs it, but say so in the
   same breath, and keep a way back — a dump, a tag, a backup. Never as a shortcut around a
   failing test.
2. **Counting.** Shipping fast does not lower the bar for a gate. A deploy is finished when the
   **live** service has been interrogated and has returned a **number**, not when `systemctl` says
   `active`. This is the one step urgency will try to skip, and it is the one that catches the
   defects nothing else catches (cf. § *Editing pitfalls*: a page can render its title and still
   be a 500).

## The direction — one cap, four subordinate plans

**The cap is single**: `docs/PLAN-SITE-ULTIME.md` — one site exposing EVERYTHING this repository
can do, driven by a coverage matrix (`servi` / `interne` with its reason / `manquant`), master
gate `manquant = 0`.

The subordinate plans and their order are in `docs/README.md` § *La direction*: `/PLAN.md` (the
switchover deadline), `docs/CODEX-JOUR-UNIQUE.md` (today's execution), `docs/PLAN.md` +
`docs/FORGE.md` + `apps/inacord/ROADMAP.md` (the long game), `docs/stack/` (the freeze).

Reverse-engineering `nie.exe` is the **means**. The Rust engine is the **end**. The **forge**
(`docs/FORGE.md`) is the **judge**: it produces `nie.exe` and measures, to the byte, how much of
it this repository actually generates. A port that moves nothing there has proved nothing.

## Two machines — know which one you are on

This file was written from the **Windows workstation**. The Linux VPS (`/home/ubuntu/niers`) is a
different machine, and a good half of the traps below do not exist there.

- The `SessionStart` hook (`.claude/hooks/etat.sh`) prints the platform and the **measured** state
  as soon as the session opens: platform, git, RE target, KB, forge, data seams, services. What it
  says outranks this file — it measures, this file remembers. **Except when it asserts instead of
  measuring**: it long claimed "THIS machine is the Linux VPS" in hard-coded text, including under
  Git Bash, wrongly invalidating the whole Windows section. Fixed on 2026-09-03 (it now tests
  `uname -s`) — but when the hook and the evidence disagree (`C:\…` in `NIE_GAME_DIR`, `.exe`
  files around), decide on `uname -s`, never on a sentence from the hook.
- On **Linux**: no MSVC (so no forge path B), no Git Bash / MSYS / UAC, `cargo fmt --all` works,
  `sed -i` does not eat backslashes, `niers mem` works.
- On the **VPS**: the 18 production services run here (azalee-*, rg-*, bxc-*, nie-model-serve).
- `.claude/hooks/garde-bash.sh` blocks, up front, the commands this repository does not accept
  (direct `python`, `node`, `bun install` outside the root, `pkill -f`, `cargo test --workspace`
  without redirection) and prints the correct form instead. Repository commands: `/etat`,
  `/verif`, `/forge`, `/porter`.

- **On the Windows workstation there are TWO `bash`, and they are different machines.** From Git
  Bash, `uname -s` gives `MINGW64_NT` and the repository is `C:\Users\aphro\niers`. From
  **PowerShell** — therefore from Codex, whose default shell on Windows is PowerShell — `bash`
  resolves to `C:\Windows\system32\bash.exe`, i.e. **WSL**: `uname -s` gives `Linux` and the
  repository is `/mnt/c/Users/aphro/niers`. Measured on 2026-09-06. The state hook used to answer
  **« CETTE machine est le VPS Linux »** on this workstation for exactly that reason — the same
  hard-coded lie the paragraph above says was fixed, coming back through another door. It now tests
  `/proc/version` for `microsoft` and names the three cases apart. Before quoting a `uname`, know
  which `bash` produced it; `Get-Command bash -All` lists them.

**Everything in § *Windows-only environment traps* applies to the Windows workstation only.**

## Tools — which one for which situation (measured here on 2026-09-02)

**This repository is 3 GB under `apps/` (`node_modules`, `.next`) and 111 GB under `data/`.** Any
tool that ignores `.gitignore` drowns in it. Measurements taken at the root:

| Situation | Tool | Measurement / reason |
|---|---|---|
| Search text across the code | **`rg`** (15.1.0) | `rg -l NIE_GAME_DIR` = **0.061 s**; `grep -rn` = **60 s timeout** (it descends into `node_modules`) |
| Search inside **one** file or a pipe | `grep` | no tree walk: nothing to gain elsewhere |
| List files | **`rg --files -g '<glob>'`** or **`fdfind`** | `find . -name '*.rs'` = **5.4 s / 840** (polluted); `fdfind -e rs` = **0.017 s / 687** |
| Search an already-clean subtree (`crates/`) | **still `rg`** | `hyperfine` (10 runs): `rg` **19.1 ms**, `git grep` 41.7 ms, `grep -r` 71.3 ms — **3.73×**. Only *listing* is a tie (`find` ≈ `fd`, 0.01 s) |
| Output you can use without re-reading | `rg --json`, `rg -l`, `rg -c`, `rg --stats` | `-l`/`-c` locate without dumping lines: fewer tokens for the same information |
| Searching from the harness | the **Grep/Glob** tools | same engine as `rg`, already structured; Bash wins when you must compose (pipe, `--json`, counting) |
| A **recurring**, domain search | **`niers find` / `niers grep`** | they embed the `ignore`/ripgrep engine. A search worth replaying gets written in Rust in `nie-cli`; raw `rg` is for throwaway exploration only |
| **Game** files | **`niers vfs find`** | the VFS is not on disk: `rg`/`fdfind` over `data/` cannot see inside the CPKs |
| Enumerate the **whole** VFS | `niers vfs find 'data/' -n 300000` | returns all **255 308** entries (path, size, CPK) in **1.9 s**. `-n` defaults to 100: without it you will believe the VFS is tiny |
| Reverse-engineered content of `nie.exe` | `sqlite3 var/niers.sqlite` | the database is **15.5 GB**: always an indexed `WHERE` and a `LIMIT`, never a `SELECT *` |
| Game data (character, skill, item) | the `@niers/catalog` facade, `niers wiki` | § *The four data seams* |
| Who calls what / where a symbol is defined | the **LSP** tool, or the KB | `rg` on a common identifier returns hundreds of false positives |
| A **remote** repository | MCP `repo_grep` / `repo_read` | no need to fetch it to read it |
| JSON | **`jq`** (1.8.1) | 6.8 MB parsed in 0.3 s |
| Editing code | **Edit/Write** | `sed -i` is not idempotent and has no guard rail; § *Editing pitfalls* |
| Editing a stream in a pipe | `sed`/`awk` | their only correct use here |
| Replacing across N files | `rg -l <pattern>` **then** Edit file by file | `rg --passthru` previews the replacement without writing |
| Counting lines | `awk '$2!="total"{s+=$1} END{print s}'` | `xargs wc -l \| tail -1` undercounts: xargs splits into several invocations, each with its own `total` (15 549 vs 175 042 lines) |

- **Always write `ast-grep`, never `sg`.** ast-grep installs an `sg` alias that **shadows
  `setgroup`** (util-linux); it was deliberately removed from `~/.cargo/bin`. `sg` must stay
  `/usr/bin/sg`.
- **`fd` now exists** (10.5.0, alongside `fdfind` 10.3.0). Trap: **fd takes the pattern BEFORE the
  path**, the opposite of `find` — `fd -e rs . crates`, never `fd -e rs crates` (which looks for
  files *named* "crates" and returns 0).
- Still missing: `comby`, `semgrep`, `ugrep`, `gron`, `scc`, `srgn`. `cargo binstall -y <name>`
  installs them in seconds (prebuilt binaries) — but **binstall drops the whole batch** if a
  single name is unknown, and `-y` already *is* `--no-confirm` (passing it twice is an error).
- Present and verified: `rg` 15.1.0, `fd` 10.5.0 / `fdfind` 10.3.0, `jq` 1.8.1, `sqlite3` 3.46.1,
  `just`, `uv`, `bun`, `ffmpeg`, ImageMagick (`compare` — SSIM), `xxd`.

### Tooling installed on 2026-09-02 — what each one measurably bought

Installed with `cargo binstall` (plus `duckdb` via its official script), then **measured on this
repository**. The ones that buy nothing here are described as such rather than recommended on
principle.

| Tool | Gain **measured here** | When to use it |
|---|---|---|
| `hyperfine` 1.20 | replaces a single `time` with 10 runs ± σ — it **invalidated** two of my own claims | any performance comparison, never a bare `time` |
| `jaq` 3.1.1 | **1.95×** faster than `jq` (94.9 ms vs 185.1 ms on 6.8 MB), **identical** output | large JSON; `jq` stays the compatibility reference |
| `tokei` 14.0 | breaks down what `wc` cannot: **147 323** lines of Rust code, not 219 388 (33 % is comments and blanks) | any size count |
| `ast-grep` 0.45 | searches a **structure**: 3 514 `pub fn … -> Result<…>` in `nie-formats`, 5 `.unwrap()` in `nie-core/src` | rewriting code — it replaces regex over code |
| `hexyl` 0.17 | readable coloured hex dump of a `.cfg.bin` (magic, offsets) | Level-5 binary formats, forge, byte-exactness |
| `duckdb` 1.5.5 | reads SQLite/JSON/CSV/Parquet in SQL (`sqlite_scan`) | aggregates and joins **across seams**; for one indexed query `sqlite3` stays 100× more direct |
| `cargo-nextest` | **2.81× SLOWER** on a small crate (`nie-pe`: 1.131 s vs 402.8 ms) — one process per test | do NOT use it per crate; its only possible value is the whole workspace |
| `difft`, `sd`, `watchexec`, `nu`, `sccache`, `tree-sitter`, `dust` | installed, not yet measured here | do not recommend them before measuring them |

## The shell — what breaks, and what is not the problem

- **The shell is not the bottleneck.** Measured: `bash -c true` starts in **3.2 ms** (50 launches
  in 0.159 s). Changing shells would gain nothing; what costs is round trips and wrong
  measurements.
- **`$?` through a pipe is the last stage's code.** `uv run x.py | tail` returns `tail`'s, so `0`:
  a failing proof reads as green (paid for on 2026-09-02). `bash` is now launched with
  `set -o pipefail` via `.claude/shell-init.sh` (installed by `BASH_ENV` in
  `.claude/settings.json`), **at the top level only** — repository scripts and cargo/cmake build
  scripts keep their semantics. Without pipefail, read `${PIPESTATUS[0]}`.
- **Accepted trade-off: `| head` can return non-zero.** The producer dies of SIGPIPE when `head`
  closes the pipe. Measured: `jq|head` = **141**, `seq|head` = **141**, `sort|head` = **2**, but
  `rg|head` = 0 and `cat|head` = 0 (they handle SIGPIPE). **A 141 after `| head` is a cut, not a
  failure** — do not open an investigation on it. When the exit code matters, limit at the source:
  `rg -m5`, `jq 'limit(5; …)'`, `LIMIT 5`. The trade is deliberate: an invisible false green (the
  original bug) costs more than a false red you can explain in one line.
- **A transcript, a dump, an API response: that is JSON, so `jq`.** Extracting commands from a
  `.jsonl` with `rg` + regex truncated the source (5 817 instead of 24 832) and returned 0 on three
  counts: escaped `\"` breaks the regex in silence. `jq -r '… | .input.command'` reads the
  structure and cannot get the boundaries wrong.
- **A `cd` inside one command persists into the next ones.** A `cd` outside the repository makes
  the following command's `git` fail with "not a git repository", which blames git instead of the
  `cd`. Use absolute paths, or `( cd x && … )` in a subshell.

## Python — the file, not the line

Measured on 2026-09-02 across this repository's 21 sessions — **24 832 unique Bash commands**,
extracted with `jq` then deduplicated (resumed sessions replay the same messages across several
`.jsonl`): **155 `uv run python -c` against 24 `uv run <file>.py`**. Almost nothing survived,
while 77 versioned `.py` already exist.

- Always `uv run`; calling `python`/`python3` directly is blocked by `garde-bash.sh`.
- **This is NOT a speed problem.** `uv run python -c` starts in **0.064 s**, and on a real 6.8 MB
  file Python answers in **0.269 s** against **0.201 s** for `jq`. Never justify a tool change here
  with performance: the gap does not exist.
- **It is a quoting-layers problem.** The body crosses bash *before* Python: `$VAR` is substituted,
  `$(…)` is **executed**, `\\` becomes `\`. Verified: `uv run python -c "print(len('\\'))"` dies
  with `SyntaxError: unterminated string literal` — the shell ate the backslash. Same cause as the
  literal `\0` that ends up in a Rust source (§ *Editing pitfalls*). Writing Python inside a shell
  string means debugging two languages.
- **Rule: more than 2 lines of Python ⇒ a file.** Scratchpad if throwaway, `scripts/` if
  versioned, then `uv run <file>`. A file is fixed with Edit, replayed identically, cited as
  `path:line` — and never goes through the quoting again. `garde-bash.sh` now refuses a
  `python -c` longer than 2 lines and prints this form.
- **PEP 723 replaces `--with`, but ONLY for a standalone script.** A `# /// script` /
  `# dependencies = ["numpy"]` / `# ///` header makes `uv run` resolve dependencies on its own
  (verified: numpy 2.5.2, 0.6 s cold). **Trap paid for**: that block runs the script in an
  **isolated** environment, so without the repository's `.venv` — a script importing the RE
  toolbox (`uemu`, capstone, pefile, unicorn) then dies with `ModuleNotFoundError`. Rule:
  - **standalone** script (no repository import) → PEP 723 block with its dependencies;
  - script leaning on the **repository toolbox** → **no** block, `uv run <file>` picks up the
    project `.venv`. That is what the 47 `scripts/validate_*.py` do.
- Which tool for what: **JSON** → `jq` (one quoting layer, and no `except: continue` silently
  swallowing the offending lines); **files in bulk** → `fdfind -x`; **dates** → `date -d @<epoch>`;
  **binary / PE / disassembly** → Python is the right tool (the `.venv` toolbox: capstone, pefile,
  lief, iced-x86, unicorn, angr), but **in a file**; **recurring and domain-specific** → a `niers`
  command in Rust.

## Repository binaries are published globally

`just installer` publishes the Rust binaries from `target/release` (**24** on 2026-09-06, 20 on
2026-09-02 — the count follows the build, do not quote it as fixed) and **5 Bun CLI launchers**
into `~/.local/bin` (already on `PATH`).

- **By symlink, never by copy**: 174 MiB of binaries (including `nie-editor` at 82 MiB) are
  written once, and a `cargo build --release` updates the published command without reinstalling.
  A copy would go stale in silence.
- **On Windows/MSYS the script was silently producing COPIES** until 2026-09-06. Git Bash's `ln -s`
  falls back to a full copy, with **exit 0 and no message** — measured: `~/.local/bin/niers.exe`
  was a real 27 690 496-byte file, `readlink` empty, i.e. exactly the stale-copy failure the header
  forbids. Fixed by exporting `MSYS=winsymlinks:nativestrict` (verified: native link obtained on
  this machine) **and** by asserting `[ -L "$dest/$nom" ]` after each `ln` — the export alone would
  still be a promise. Check with `readlink ~/.local/bin/niers.exe`, never by trusting the exit code.
- **The script refuses to overwrite a foreign executable** already on `PATH` — the lesson of the
  day ast-grep's `sg` alias shadowed `setgroup`.
- The Bun CLIs are published as `nie-catalog`, `niers-azalee`, `niers-inagle`, `niers-mcp`,
  `niers-bxc`, through a `bun --bun` launcher (never `bun run`: the `node` shebang would be
  honoured).
- **`NIE_GAME_DIR=/home/ubuntu/niers` is set** in `.claude/settings.json`. Without it the four
  `export_*` fail outside the repository: they do call `resolve_game_dir()` (doctrine respected),
  but from `/tmp` no ancestor carries `data/cpk_list.cfg.bin`. With it, all the commands work from
  any directory (verified: `export_skills` → 1 004 skills, exit 0).
- **That value is a VPS path, and `.claude/settings.json` is shared by both machines.** On the
  Windows workstation `NIE_GAME_DIR`, `NIERS_REPO` and `BASH_ENV` therefore all arrived pointing at
  `/home/ubuntu/niers`, which does not exist here — every `niers` invocation mounted nothing while
  looking healthy, and the `niers-game` MCP server opened on **0** file. **Fixed on 2026-09-06 by
  making the three variables per-machine**: they keep their VPS values in the versioned
  `.claude/settings.json` (a fresh VPS clone stays correct), and are overridden by
  `.claude/settings.local.json`, which is per-machine and gitignored (`.gitignore:323`, `.claude/*`).
  Windows values: `NIERS_REPO=C:\Users\aphro\niers`,
  `NIE_GAME_DIR=C:\Program Files (x86)\Steam\steamapps\common\INAZUMA ELEVEN Victory Road`.
  Measured on both sides of the fix, by launching the MCP server: `VFS non montable depuis
  C:\Program Files\Git\home\ubuntu\niers\data` before, `index VFS chargé : 255308 fichiers` after.
  **That file does not survive a fresh clone** — recreating it is a bootstrap step, like the
  generated JSON manifests.
- The `export_*` binaries have **no** `--help`: their silence on `--help` is not a fault.
- Doctrine unchanged: **`niers` is the only user-facing CLI**. `nie-mem` and `nie-steam` overlap
  `niers mem` / `niers steam`; a new command goes into `nie-cli`, never into one more binary.

## Claude Code plugin and MCP server of this repository

- **The MCP server `niers-game` is declared exactly once**, in `/.mcp.json` at the root. It used to
  be declared a **second** time in `plugins/niers-plugin/.mcp.json`: both were started, and the
  second died on `écoute impossible sur le port 8791 : Is port 8791 in use?` — a message that
  accuses the port while the cause is the duplicate. Removed on 2026-09-06, along with the
  `"mcpServers"` key of `.codex-plugin/plugin.json` that pointed at it.
- **`enabledPlugins` alone loads NOTHING.** `.claude/settings.json` had been activating
  `niers@niers-marketplace` for days while `niers-marketplace` was declared in no
  `extraKnownMarketplaces`: measured on 2026-09-06 with `claude plugin marketplace list` (4
  marketplaces, none of them this repository's), and confirmed by the absence of its **6 sub-agents**
  and **16 skills** from the session listing. An unknown marketplace raises no error — the plugin is
  simply not there, exactly like a `.gitignore`d file. Both keys are now in the versioned project
  settings, the marketplace with the relative path `./plugins` so the VPS resolves it too. Check
  with `claude plugin list`, never by re-reading `enabledPlugins`.
- **A plugin is activated in the project it serves, never globally.** On 2026-09-06 three
  foreign plugins were enabled in `~/.claude/settings.json` — `winclean` (3 agents, 25 skills, and
  the whole `mcp__plugin_winclean_*` tool block), `aphrody` (25 agents, 57 skills, 6 commands) and
  `ghidra-suite` (12 skills, plus an MCP server that fails with `ConnectionRefused` as long as no
  Ghidra GUI listens on :8080). All of it was loaded into **every** session of every project,
  including this one. They were moved into their own repository's settings; nothing was lost, and
  this session's listing is 28 agents and 94 skills lighter.

## Build and test — the gate, and every way a green suite lies

Cargo workspace, **38 members** (re-measured 2026-09-06 evening,
`cargo metadata --no-deps --format-version 1 | jq '.packages | length'`) plus 2 archive crates on
disk that are out of the build. Organised by role:

- `crates/forge/*` — binary production (`nie-pe`, `nie-asm`, `nie-forge`) plus the RE scaffolding
  (`nie-re`, `nie-index`, `nie-seed`, `nie-queue`, `nie-trace`).
- `crates/engine/*` — the engine (`nie-core`, `nie-formats`, `nie-data`, `nie-render3d`, …).
- `crates/tools/*` — tooling (`nie-cli`, `nie-wiki`, `nie-steam`, `nie-model-serve`, `nie-site`, …).
- `crates/archive/*` — out of the build, reference only (`nie-engine`, `nie-rs`).

**The gate is `cargo clippy -p <crate> --lib --tests` with 0 warnings, never a full build.**

```bash
cargo clippy -p <crate> --lib --tests     # 0 warnings required
cargo clippy -p <crate> --bins --tests    # for the 7 bin-only crates (see below)
bun run typecheck                          # TypeScript side
```

- **7 of the 38 crates have no library target**, and the gate above answers
  `error: no library targets found` on them — an error that is NOT a failure of the crate.
  Measured 2026-09-06 (`cargo metadata … | select([.targets[].kind[]] | index("lib") | not)`):
  `nie-bench`, `nie-cli`, `nie-editor`, `nie-game`, `nie-headless`, `nie-model-serve`, `nie-play`.
  Use `--bins --tests` for those seven (all 0 warnings). Do **not** add an empty `src/lib.rs` to
  make the documented command succeed: that fabricates a target so the gate stops complaining,
  which is the same class of defect as a test that cannot fail. `nie-ffi` and `nie-wasm` are
  `cdylib`/`rlib` and DO accept `--lib`.
- **`apps/inacord/src-tauri` is a separate workspace, so NO repository gate ever compiles it.**
  Measured 2026-09-06: it did not build at all (`E0063: missing field 'context'` in
  `src/lua_tools.rs`) after `ExecOptions` gained a field in `nie-lua` — clippy over the 38 crates
  stayed green throughout, because none of them contains this code. Running `bun run tauri build`
  is the only thing that reveals such drift. Corollary for that tree: name struct fields
  explicitly rather than `..Default::default()`, so the next added field breaks loudly instead of
  being absorbed in silence.

- **Never run `cargo build --workspace --all-targets`**: the disk is 92 % full and it saturates it.
- Workspace lints (`[workspace.lints]`): `todo!`, `unimplemented!`, `dbg_macro` → **deny**.
- `nie-core`, `nie-pe`, `nie-asm`, `nie-forge`: `#![warn(missing_docs)]` → document **every**
  `pub` item.
- Golden tests: `cargo test -p nie-data --test <family>_golden`.
- **A suite printing `0 passed` is never a success**: it is a suite that did not run.
- **`dataset::Gisement` opens SQLite READ-ONLY**: a test that seeds rows through it fails, and
  `.ok()` on that failure gives a green test on an empty table. Seed with a separate
  `Connection::open(dir.path().join(…))` before the gisement reads.
- **`nie-formats` enables only `std` and `lua` by default**; `serde`, `textures` and `images` are
  optional. A test gated `#![cfg(all(…))]` on a disabled feature prints "ok. 0 passed" — a **false
  green**, hit twice. Declare `[[test]] required-features = […]` (the harness then says why it
  skipped) and pass `--features images,textures` for anything touching images.
- **Same trap, harsher form, fixed on 2026-09-05**: a disabled feature does not always give a
  false green, it can BREAK the gate. 24 `nie-data` tests called `nie_data::typed` (gated behind
  `serde`) without declaring it: `cargo clippy -p nie-data --lib --tests` failed with **E0433** on
  a healthy crate. Anyone following the instruction to the letter saw an error and had to guess
  `--features serde`. Fixed with 24 `[[test]] required-features`. Facing a clippy that fails for no
  reason, look at the crate's optional features **before** blaming your own code.
- **A test guard checking a hard-coded path instead of `NIE_GAME_DIR` skips ALWAYS**, in silence
  (lived through on `override_skill_golden.rs`, which only tested `/mnt/c/…`: on the VPS it never
  ran and the suite reported green). A guard reads `NIE_GAME_DIR` first.
- **`dotnet` is ABSENT from the VPS**: `csharp/` neither compiles nor tests there. A C# batch can
  only be **reviewed** — say so, never claim it verified.
- The repository can be reorganised **during** a session (crates moved or created by parallel
  work): if a build fails on a crate that is not yours, check `cargo metadata --no-deps`, wait, and
  never move or "fix" another session's crate.
- After `cargo clippy --fix`, **re-run `cargo check --workspace --tests`**: it sometimes removes an
  import that was in use (seen on `phase_set_golden.rs`).

## Bun workspace (`packages/*`, `apps/*`)

One lockfile, at the root. A library goes to `packages/`, an application with a `bin` to `apps/`.

| Package | Role |
|---|---|
| `packages/nie` | FFI bindings for `libnie_ffi` — the TS door into the Rust crates |
| `packages/nie-bridge` | Shared control protocol `nie-mcp` ↔ `nie-explorer` |
| `packages/nie-catalog` | **The facade over the four data seams** (game / extract / re / anime) and their joins |
| `packages/nie-plugin` | Bun plugin importing the formats — **preloaded by `bunfig.toml`** |
| `packages/azalee` | The wiki library — service, images, client-safe CDN clients (`cpk/*`) |
| `packages/azalee-tools` | **The wiki's OFFLINE tooling**: the `azalee` CLI, manifest scripts, local server, remote client. It reads the disk and the local databases — never deployed to Vercel, which is why it is separate |
| `packages/nie-game` | Pure GAME logic: formations, team codes, rules, text. Neutral, unbranded, no I/O |
| `packages/asset-source` | **The asset access contract** (`AssetSource`) and its web source. Does NOT depend on Tauri: that is what makes it usable from a browser |
| `packages/inacord-ui` | **The shared interface** of Inacord and Aphrody — 45 primitives, the `shell/{main-menu,inacord}` shells, and `useAssetSource()`. Zero `@tauri-apps` |
| `packages/inagle` | The game data pipeline: parsers, entities, push to Postgres |
| `packages/cron` | The task daemon, including `src/tasks/ie-crawl/` (43 watch modules) |
| `packages/ietv`, `wonderbot`, `zukan` | The series episode catalogue, its Discord bot, the official zukan |
| `packages/db`, `types`, `auth`, `config`, `ui`, `assets`, `mcp` | The wiki's shared foundation |
| `apps/azalee` | The wiki site (Next.js 15, App Router) |
| `apps/bxc` | The gateway to `@aphrody/bxc` and the unified scraping workflow |
| `apps/inacord` | Tauri explorer/editor (React + Rust, `src-tauri` outside the Cargo workspace) |
| `apps/nie-mcp` | The `niers-game` MCP server — VFS, assets, RE KB, explorer control |
| `apps/nie-web` | **Aphrody on the web**: the Vite host for `inacord-ui`, served by the `nie-site` crate. `src/legacy/` is an airlock — code pulled out of the wiki awaiting a rewrite, excluded from `tsconfig` |

```bash
bun install                 # from the root, never inside a sub-package
bun run build:ffi           # cargo build -p nie-ffi — REQUIRED before any other `bun run`
bun run typecheck           # 5 workspaces
bun run test
bun run lint
```

- Shared versions come from a **catalog**: `catalog:` (typescript, `@types/bun`) or `catalog:mcp`
  (MCP SDK, zod). Never a hard-coded version: a hard-coded version makes several TypeScripts and
  several zods coexist, which makes MCP tool schemas unassignable.
- `nie-mcp` and `nie-explorer` share the **same Rust layer**: the explorer links `nie-formats`
  directly, the MCP reaches it through `packages/nie` (FFI). Do not reimplement on one side what
  the other already does.
- Regenerate the Tauri bindings without opening a window:
  `cd apps/inacord/src-tauri && cargo run --bin export-bindings --features dev-bindings`.
- **`bunx tsc --noEmit` fails on `apps/nie-web`** (`TS5101: 'baseUrl' is deprecated`): the global
  `tsc` is not the workspace's. The gate is `bun run --filter '*nie-web*' typecheck`.
- **`nie` is also a package on the npm registry.** Without `bun install` at the root,
  `import … from "nie"` resolves to the cached `nie@1.2.7` instead of `packages/nie` — misleading
  error `Export named 'decode' not found`. The `dlopen` of `nie_ffi.dll` is only the *next* cause.
- **Pulling a package in from another workspace requires merging its `catalog` AND its
  `overrides`**: `catalog: failed to resolve` on every missing entry, and without rg's `kysely`
  override Bun deduplicates `better-auth` under a generated name Next can no longer resolve.
- A package whose `exports` points at `./dist/*` does not resolve without a build: point it at
  `./src/index.ts`, Bun reads the TypeScript.
- **`apps/inacord/src-tauri` is on edition 2021** while the workspace is on 2024: let-chains do not
  compile there — write nested `if let`.
- **A synchronous `#[tauri::command]` runs on the MAIN THREAD**: any `tokio::spawn` inside panics
  with "there is no reactor running", and that panic, in a non-unwinding context, **kills the
  application** (`STATUS_STACK_BUFFER_OVERRUN`, with no useful trace). Any command touching the
  VFS, a task, or the disk must be `async`.
- `src-tauri` has two binaries: without `default-run` in its `Cargo.toml`, `tauri dev` refuses to
  start ("could not determine which binary to run").
- A `tauri dev`/`build` failing with "Access denied" while writing `nie-explorer.exe` means an
  instance is still running. Kill the PID, do not re-run the build.
- **`bundle.resources` KEEPS the declared relative path**: `"resources/db/*.gz"` lands in
  `<resource_dir>/resources/db/`, never `<resource_dir>/db/`. Aiming at the wrong path breaks
  nothing visible — the package weighs the right amount, the signature is valid, and the resource
  is simply never read. Try both forms (`resource_dir()` varies: the exe directory outside a
  bundle, `<install>/resources` for an MSI). Lived through on 2026-09-03.
- **Only LAUNCHING finds bugs of that kind**: neither `tsc`, nor clippy, nor the bundle size check
  can see a resource that is never read or a table that stays empty. After a build, run the exe and
  look at what it wrote into `%APPDATA%\dev.niers.explorer\`.
- `ui/Icon` renders **`null`** for a name missing from its table: a missing icon raises nothing, it
  disappears. Measured on the running application on 2026-09-06 — **9** names reached `<Icon>`
  without being declared in `iconMap` (`flare`, `inventory_2`, `egg`, `checkroom`, `emoji_events`,
  `animation`, `auto_fix_high`, `content_paste`, `calculate`), amputating four Encyclopédie tabs
  and the "Auto-remplir" / "Coller un code" buttons. Nothing in `tsc`, clippy or the bundle check
  can see this. **The structural cause is the type**: `IconProps.name` is
  `keyof typeof iconMap | (string & {})`, and that `| (string & {})` switches the compiler off.
  Remove it and the nine silent defects become nine TypeScript errors — a declarative check made
  structural, cf. § *Editing pitfalls*. Note there are **two** `Icon` components (`ui/Icon.tsx` and
  `wiki/ui/Icon.tsx`): confirm which one a site imports before counting.
- `base-ui` **rejects `<SelectItem value="">`** (the empty string means "nothing selected" there):
  encode the "all" value as a token (`__all__`) and translate it back on the way out.
- A new Tauri command needs three things: `#[tauri::command] #[specta::specta]`, **plus** adding it
  to the `invoke_handler` list, **plus** `cargo run --bin export-bindings --features dev-bindings`.
  Miss the second or the third and the front end never sees it.

## Polyglot doctrine — one role, one language

Full map: `docs/ARCHITECTURE.md`. In short:

| Language | Roles |
|---|---|
| **C++** (`src/`) | decompiled C → playable `nie` game; libraries that only exist in C++ (assimp, Bullet) |
| **C#** (`csharp/`) | dump, pack, memory, texture conversion |
| **Rust** (`crates/`) | **the only CLI**, GUI, core library, wasm, RE, byte-exactness |
| **Bun/TS** (`packages/`, `apps/`) | MCP, web server, types, API, UI |

- The C++ texture conversion is **the worst of the three**: do not extend it.
- **One interface, two hosts.** Inacord (Tauri) and Aphrody (browser) mount the SAME components
  (`packages/inacord-ui`) through two implementations of a single contract
  (`packages/asset-source`). A component never knows who hosts it: it asks for its source with
  `useAssetSource()` and for what the host can do with `useCapacites()`. Of the desktop host's 147
  commands, ~66 are portable and 81 never will be (Lua, forge, modding, Blender, game memory,
  disk): the interface HIDES what the host cannot do instead of offering it and then failing. Never
  write a host condition inside a component — the contract carries the asymmetry, and
  `capacites()` measures it instead of asserting it.
- **`niers` is the only user-facing CLI.** The others sit behind the facade: `niers cpp <args>`
  (C++ toolkit), `niers cs <args>` (.NET tooling), `niers backends` (what is built and where). A
  new command is written in Rust, never in the other two CLIs — see
  `crates/tools/nie-cli/src/delegate.rs`.

## C++ tree (IECODE toolkit) — everything under `src/`

C++20 toolkit: parsers, compression, VFS, converters, modding, rendering.

```
CMakeLists.txt      root of the `iecode` CMake project (C/C++20, vcpkg, unity build, LTO, ccache)
src/                iecode_core implementations — archive compression converters crypto db
                    formats gamedata io modding render services vfs viola
                    (engine/ and game/ have their own targets: iecode_engine, iecode_game)
src/include/iecode/ public headers (compression/, crypto/, level5/, criware/, vfs/, modding/,
                    export.h, types.h)
src/cli/commands/   the 39 subcommands of the `iecode` binary
src/decomp/         **forge path B** (`functions/*.c` annotated `/* @nie 0x… */`, MSVC 14.44
                    `/O2 /GS- /Gy /Zl`) — NOT part of the toolkit, see § Forge
src/tests/          GTest (474 cases)
third_party/        vendored header-only sources (stb, mio, bcdec, tinygltf)
cmake/              CompilerWarnings.cmake, SIMDDetect.cmake, vcpkg overlay-ports
csharp/             IECODE.Core / IECODE.CLI / IECODE.Core.Tests (.NET 10, `IECODE.sln` at the root)
```

- **`src/CMakeLists.txt` does a `GLOB_RECURSE`** over all of `src/` for `iecode_core`: subtrees
  with their own target (`engine`, `game`, `cli`, `tests`, `decomp`, `include`) are excluded by
  `list(FILTER … EXCLUDE REGEX ".*/src/<name>/.*")`. Adding a subtree with its own target and
  forgetting its filter puts several `main()` in the library.
- Build: `just cpp-build` (or `cmake --preset msvc && cmake --build --preset msvc-debug`).
  **`cmake` is not on this machine's PATH**: it lives in
  `…/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe`.
  **vcpkg is installed in `var/vcpkg`** but `VCPKG_ROOT` is not exported: set it in the command —
  `VCPKG_ROOT="$PWD/var/vcpkg" "<cmake>" -S . -B build/msvc`, then
  `"<cmake>" --build build/msvc --config Debug --target <target>`. The libraries are already in
  `build/msvc/vcpkg_installed`: an incremental configure recompiles no port.
- Conventions: C++20 with `CXX_EXTENSIONS OFF`, `CamelCase` classes / `lower_case` functions /
  `UPPER_CASE` constants, no exceptions on hot paths (`std::optional` or return codes),
  `std::span<const uint8_t>` for binary parsing, 4 spaces / 100 columns (clang-format Google).
- C++ is reached through `niers cpp` (subprocess), never in-process: it exposes no FFI. The
  repository's wasm is `nie-wasm` (Rust); the toolkit has no WebAssembly target.

## Game data (VFS)

- `data/` holds the real local copies (dx11, ~57 GB of packs, `cpk_list.cfg.bin`). **gitignored** —
  assets © LEVEL-5. Never commit or push them (`start.png` and `menu.png` included).
- **No machine path is compiled into any binary.** The game root is resolved at runtime —
  `nie_formats::vfs::resolve_game_dir()`: `NIE_GAME_DIR`, else the current directory or an ancestor
  carrying `data/cpk_list.cfg.bin`, else the executable's directory. The equivalents are
  `dansLeDepot()` on the TS side and `TestDataPaths`/`ResolveDefaultGamePath()` on the C# side —
  look for the existing helper before writing one.
- **`NIE_GAME_DIR` is required on the Windows workstation**: the repository's `data/` exists but
  does **not** carry `cpk_list.cfg.bin`, so the ancestor walk fails and the VFS never mounts
  (`niers info`, the `niers-game` MCP, the goldens). Set as a **user** variable pointing at
  `…/steamapps/common/INAZUMA ELEVEN Victory Road` → 255 308 entries, 936 packs. On the Steam
  Windows install the full VFS **is** the cwd, so `NIE_GAME_DIR` is otherwise useless.
- `NIE_GAME_DIR` / `NIE_DUMP_DIR` **set but empty are ignored** (an empty string is not a root — it
  used to return an empty path where nothing is ever found).
- `Vfs::init()` takes **`<root>/data`**, not the root (otherwise "cannot open cpk_list.cfg.bin").
- **Two mounts, same logical paths** (`data/common/…`, `data/dx11/…`) — verified 2026-08-28:
  `packs` (Steam install, `cpk_list.cfg.bin` + `packs/*.cpk`) and `dump` (extracted tree, here
  `<repo>/data`, 255 316 files / 111 GB). `Vfs::init` **switches on its own** to the dump when
  `cpk_list.cfg.bin` is missing but `common/`/`dx11/` are there; `vfs::open_game()` mounts whatever
  is available; `NIE_DUMP_DIR` forces the dump even when the install is visible. `Vfs::is_dump()`
  says which one is running, `niers info` prints it (`vfs  dump — 255 316 entrees`). Proofs:
  `nie-formats --test dump_vs_packs`, `nie-game --menu title00` (**identical PNG sha256** on both
  sides), `nie-play` (170 identical frames). Coverage measured on 2026-08-28 by
  `cargo run -p nie-formats --example dump_couverture`: **255 308 / 255 308 = 100.000 %** of the
  game index, 0 missing, 8 files outside the index (working images under `data/mod/`).
- **The dump mount indexes nothing until you enumerate it**: `read`/`is_readable` resolve by path;
  the index (255 k entries, minutes on NTFS) is only built by `find`/`iter`/`asset_count`.
  `Vfs::materialiser(path, cache)` returns a real file on disk — **without copying** on a dump, by
  extracting into the cache on packs (that is what lets `nie-play` run with no arguments at all).
- The guard for tests backed by the real game is `vfs::donnees_disponibles(<data_dir>)`, **not**
  `cpk_list.cfg.bin.exists()` — otherwise 13 menu-rendering gates skip while announcing "game
  absent" on a machine that has the dump.
- Goldens backed by the `*.cfg.bin.json` dumps go through `NIE_GAMEDATA_JSON` (or
  `<NIE_GAME_DIR>/dump/gamedata`) and **announce their skip** when the corpus is missing — a silent
  golden that does not run is a false green.
- `niers vfs extract <path> -o <FILE>`: `-o` is a **file**, not a directory — otherwise "Access
  denied (os error 5)", which has nothing to do with permissions.
- **`niers decode` ≠ `niers refresh-typed-json`.** `decode` returns the **raw** RDBN; a typed
  consumer (formation export, explorer front end) then reads 0 elements **while reporting
  success**. For typed JSON it is `refresh-typed-json` — its help says so explicitly.
- **A VFS path quoted from memory is almost always wrong**: game files carry a version number
  (`chara_base_1.03.98.00.cfg.bin`). Aim at the **directory**, and verify with `niers vfs find`
  before writing the path into code or a test. This is the measurement that settles a code review,
  in either direction.
- Prebuilt binaries live in `target/debug/` (`niers.exe`, `nie-cam.exe`…): explore without
  rebuilding.

## Modding (`niers mod`)

- Cycle: `init` → `add` → `get`/`set` (JSON Pointer over the `nie_explore::bridge`) → `status` →
  `validate` → `install` / `uninstall`. A mod is a directory plus `mod.json` plus a **VFS** tree
  (`data/…`).
- **`encode_t2b` is not faithful, and that is blocking.** An empty round trip of
  `cpk_list.cfg.bin`: different sha, 16 bytes short, *with no modification at all* — and `nie.exe`
  rejects the file. On `game_param.cfg.bin`, `/entries/0/children` drops from 812 to 1 element.
  Conclude nothing from a file that "reads back correctly": our parser is more permissive than the
  game.
- Intended fix: **patch the bytes in place** (offsets preserved) rather than re-encode — everything
  a mod changes is constant-size (integers, floats, an empty-string index already in the pool).
- `install` always starts from the saved vanilla `cpk_list`; beyond 64 already-loose entries it
  refuses (the file has already been packed). `uninstall` re-reads and compares the bytes after
  restoring.

## Porting a nie-data family

- Almost everything is already ported.
- Before porting a new one: `grep -rl "<MARKER_LIST>" crates/engine/nie-data/src/` — do not trust
  file names, modules are named after concepts.
- Probe: `target/debug/examples/probe_rdbn <prefix>` (RDBN) or `probe_t2b <prefix>` (T2B), with
  `NIE_GAME_DIR` set.
- Two formats hide behind `.cfg.bin`: **RDBN** with lists (`cfgbin::is_rdbn` → `parse` +
  `read_values`) and **T2B** (`cfgbin::cfgbin_parse`, `CfgEntry` tree). Everything under
  `common/property/**` is T2B.
- **`niers decode` returns the RAW structure** (header/types/fields), not the `{entries}`/`{lists}`
  shape `nie-data` expects — that broke 71 080 files after the 2026-08-15 update. The fix is
  `nie_formats::cfgbin::to_iecode_json` + `niers refresh-typed-json <dir> --force`.

## Reverse-engineering `nie.exe`

- The binary is `nie.exe` **at the root** (not `data/nie.exe`), image base `0x140000000`.
- **`nie_eacpatched.exe` is not patched**: sha256 identical to `nie.exe` (`b1fa04ea3658…`), on the
  VPS as locally. To get out of EAC, launch `nie.exe` directly, without
  `GameBootstrapper`/`EACLauncher`.
- funcLua command table: `uv run scripts/extract_funclua_table.py` →
  `data/re/funclua-cmdid-handlers.json` (regenerable, gitignored).
- **RE tooling installed** (verified 2026-08-15; the old "no `r2`/`objdump`" note was stale):
  - Disassemblers/CLI: `objdump` 2.46, `r2` 6.0.7, `rizin` 0.7.3, `gdb`, `wine`, `yara` 4.5.5,
    `binwalk` 2.4.3, `upx` 4.2.4, `cabextract`.
  - **Ghidra 12.0.4** (`/opt/ghidra_12.0.4_PUBLIC`, `analyzeHeadless` on PATH) with **BSim +
    VersionTracking** — the tool for re-pairing functions between two builds.
  - Python (`.venv`, 3.14): `capstone`, `iced-x86`, `keystone`, `unicorn`, `pefile`, `lief`,
    `r2pipe`, **`pyghidra`**, `angr`, `z3-solver`, `ROPGadget`, `flare-capa` (rules
    `/opt/capa-rules`, signatures `/opt/capa-sigs` — pass them with `-r`/`-s`, the PyPI wheel
    bundles neither).
  - `GHIDRA_INSTALL_DIR` is set in `/etc/environment` and `~/.bashrc` (before the interactivity
    guard): without it `pyghidra.start()` fails.
  - **PyPI trap**: the `capa` package is NOT the FLARE tool (it resolves to `capa==0.1`). The right
    package is **`flare-capa`**; both provide a `capa` module.
- Function bounds come from `.pdata`.
- Classification by `main_return`: `mov al, 1` → portable (return-1). **Porting a conditional
  return (`sete al` / `found ? 1 : 0`) as a constant is forbidden** — a classic source of
  duplicates and errors.
- **`niers mem` is Linux-only** (`process_vm_readv`). On Windows: `nie-mem.exe` (dump/scan/read,
  `ReadProcessMemory`) and `nie-edit.exe` (locator catalogue), both requiring elevation.
- **Reading `nie.exe` memory requires elevation**: the process is more privileged than the session,
  `OpenProcess` fails, and the tool reports "nie.exe not found" while it is running.
- **Elevation blocks the WINDOW too, and the way around it is not the window.** Measured
  2026-09-06 on a running game (Steam-launched, pid found, `SeDebugPrivilege` adjusted): `nie-mem
  base` → `module « nie.exe » introuvable`, `nie-mem maps` → `0 plage(s)`, and a per-`HWND`
  `PrintWindow` capture → `error=5`. All three are the same access denial wearing three different
  messages — none of them means the game is absent. **A full-screen `bitblt` grab succeeds**
  (winclean `computer_capture_screen`), because it reads the composited desktop instead of the
  protected process: that is how you watch a game you cannot open. `nie-mem find-pid` keeps working
  throughout (it only enumerates), so a found PID next to an empty module list is the signature of
  this situation, not of a dead process.
- The `nie-trace` catalogue was **re-anchored** on the installed build (2026-08-27): `resolve --all`
  gives **20 ✓ / 0 drift / 4 not found** (previously 0 ✓ / 22 drift). The AOBs were **not** at
  fault — they landed on a unique site; the reference `rva`s came from another build. Re-anchor by
  scanning the **file** (not memory: no elevation, no ASLR), then validate live. An AOB with
  multiple hits or none goes back to `rva: None` — we do not guess.
- **The in-memory `.text` is not the file's `.text`** when a third-party trainer is running: 4
  runtime patches observed (2 `ret` neutralising the EOS anti-cheat, 1 RWX trampoline, 1 timer
  freeze). Before blaming a signature that fails live but succeeds on the file, compare the dumped
  module to the file **section by section** and cross-check with `.reloc`: whatever no relocation
  covers is a patch, not a loader artefact. Method: `docs/RE.md`.

## Game screens — the layout is exported, not drawn

- `nie-game --runtime --menu <screen> --export-layout <json>` returns the **real** layout: for
  `mainmenu01`, a 1280×720 canvas and **34 objects** with their `transform` (x, y, anchor, scale,
  rotation), `drawPriority`, `sprite.logicalPath` and their already-translated text.
  `--compose-layout` composes that JSON into a PNG, and it is repeatable: a screen stacks several
  layers.
- **A layer's screen name is not the script's.** `mainmenu01` = 34 objects and `scripts=0` (no
  `.lua.bin` carries that name); `kizuna_town_mainmenu` = 0 static objects but `scripts=1`, 66
  recognised runtime commands. Layout and behaviour come from two distinct screens, to be stacked.
- A menu texture's URL: `/assets/tex/<VFS path without .g4tx>.png`. The JSON's `logicalPath` has
  **no** `data/` prefix; the URL needs one.

## Knowledge base (`var/niers.sqlite`)

> **RESOLVED on 2026-09-06 (Windows workstation) — the KB is now anchored on the target.**
> The contradiction below was real until that day; it is kept because it names the failure mode.
> `var/niers.sqlite` was found **empty** here (507 904 bytes, schema only, `function` = 0 rows), so
> it was rebuilt against the Steam install — `b1fa04ea3658…` / 33 918 464 bytes, **byte-identical to
> the reference**. Note there is **no `nie.exe` at the repository root on this machine**: the target
> is `<NIE_GAME_DIR>/nie.exe`, and `niers info` prints its sha.
>
> Measured after `seed` → `rebuild` → `strings` → `rtti` → `rebuild` → `pdata` → `recover`:
> **117 068 functions** on `binary_id=2`, **93 483 classified (79.85 %)**, **49 158 named**,
> **`pdata_func` = 55 351 roots** — a figure that **reproduces the 2026-08-15 measurement exactly**,
> which is what proves the split is the same and the target is the right one.
>
> **Trap paid twice, in the same shape.** `niers rebuild` refuses with `aucun binaire indexé —
> lancer 'niers seed' d'abord`: only `seed` inserts into `binary`, and it wants a Ghidra export
> (`research/nie-index.json`) that does not exist here. And `.pdata` alone **names nothing** — the
> first `rebuild` gave `roots=55351` with `named=0`; the two anchoring passes (`strings`, `rtti`)
> are not optional.
>
> **`binary_id=1` still carries 0 functions here**: without the Ghidra index the 60 183-node layer
> is absent, so a denominator of 106 340 (and any `named %` computed on it) is **not** comparable to
> the figures above. Quote the denominator with the number, always.
>
> The old, now-superseded figures — 108 650 functions, 13 653 named (12.57 %), 100 664 classified
> (92.65 %) — described the **transient** build `4c2b91fbae6f…` / 31 468 032 bytes. **Do not quote
> them.**

The real tables are `function`, `pdata_func`, `coverage` (not `functions`).

- `Db::init` (nie-index) applies `schema.sql` **then** `camera.sql` (`meta.schema_version = 2`).
- Populate the camera: `nie-cam index [--samples]`; state: `nie-cam stats`.
- `sqlite3` is on PATH (shipped by the Android SDK): `sqlite3 var/niers.sqlite "…"`.
- **The forge writes into the KB** (`nie-forge kb`, `crates/forge/nie-forge/src/kb.rs`):
  `forge_unit` (status and cause per unit — `produit`/`bloque`/`regle`/`donnees_inline`/`verbatim`),
  `forge_classe` (per RTTI class: methods, resolved, produced, blocked) and the view
  `v_forge_function`. That is the table to join to know whether the repository can produce a given
  body. `forge_classe.resolues` bounds the reading: vtables come from the Ghidra index, functions
  from the `#pdata` split.
- **Two `binary_id` coexist**: `1` = misaligned Ghidra index (60 183 nodes, 88.20 %, frozen — the
  Ghidra project was never replayed), `2` = `#pdata`, the ground truth. **Quote number 2.** State
  verified 2026-08-15 (re-measured by hand,
  `niers rebuild --db var/niers.sqlite --exe nie_eacpatched.exe`, target byte-identical to the one
  documented since 2026-08-10): **roots = 55 351**, **cov_raw = 97 006/106 340 (91.22 %)**,
  **named = 6 429/106 340 (6.05 %)**. The earlier figures (52 783 roots, 93.36 %, 12.18 %) date from
  2026-08-10 and come from a less certain provenance — prefer the most recent measurement, and do
  not assume 52 783 necessarily described this same binary.
- Ground truth is regenerated, never copied from a document: `nie-forge report` (produced share),
  `niers vfs stats` (VFS histogram), `niers coverage --db var/niers.sqlite`.

## Forge (producing the binary) — 2026-09-06: **74.00 % of the file, 92.24 % of `.text`**

> **Amendment of 2026-09-06 — the 2026-08-30 reference is passed on both axes.** Rebuilt from
> nothing on the Windows workstation (`var/forge/` was absent) against `<NIE_GAME_DIR>/nie.exe`,
> once the KB had been re-anchored (§ *Knowledge base*):
>
> | | Reference 2026-08-30 | Measured 2026-09-06 | Delta |
> |---|---|---|---|
> | Share of the file | 69.365 % | **74.005 %** | **+4.640 pt** |
> | Share of `.text` | 90.363 % | **92.239 %** | **+1.876 pt** |
>
> `produced=74.004890% code_rust=92.239011%`, and `nie-forge build` still returns a
> **byte-identical** binary — `identical=true`, 33 918 464 bytes, sha `b1fa04ea3658…`,
> 219 751 units and 25 101 322 bytes produced, **0 rejected**. The identity contract was not
> touched.
>
> **The lever is confirmed, with a different number.** `niers recover` measured **59 224** leaf
> functions here (not 61 076 — the shape holds, the count is machine-measured), explaining 98.20 %
> of the `.pdata` holes; `split` residue falls from 1 828 793 bytes to **65 673** (documented:
> 51 151), a ~28× collapse, for `units=225767 fns=115623`. Cross-check that ties it together:
> `55 351 pdata roots + 60 272 recovered leaves = 115 623` function units.
>
> **Next targets, ranked by bytes and already diagnosed** (`lift`: 195 causes, 4 366 units,
> 1 290 354 bytes blocked): the two cheapest are dialect, not semantics — `encodage:mov`
> (**1 675 units**, 42 399 bytes: the null REX prefix `40 8b ce`, i.e. the `.r` suffix, is not
> emitted) and `encodage:add` (43 611 bytes: `orig=[47,00,2b]` vs `nie-asm=[45,00,2b]`, missing
> REX.X). Together 86 010 bytes for encoder work only. Then come real instructions: `extractps`
> (45 482), `vmovdqu` (45 061), `in` (42 997).

> **Measurement replayed on the Windows machine, not quoted from memory.** `var/forge/` was
> missing; `nie-forge split` + `lift` + `report` rebuilt it and first **reproduced the old figure
> exactly** (51.860709 % / 66.090975 %), proving the forge runs here and the target is the right one
> (`b1fa04ea3658…`, 33 918 464 bytes). It was then raised to **69.365 % / 90.363 %**, and
> `nie-forge build` returns a **byte-identical** `dist/nie.exe` (`identical=true`, 112 044 units and
> 23 527 558 bytes produced, 0 rejected).
>
> **The decisive lever was not the encoder but the split.** `split` only knew the 55 351 `.pdata`
> roots and left 1 828 793 bytes of `.text` as hashed residue that could not be lifted. Feeding it
> the **61 076 leaf functions measured by `nie_re::recover`** drops the residue to 51 151 bytes and
> raises function units to 116 091. RE is not only for naming: it is for **splitting**, and without
> a correct split there is nothing to produce.
>
> Do not confuse "not re-verifiable here" with "stale": between the evening of 2026-08-14 and
> 2026-08-15 the Steam installation transiently carried ANOTHER build (31 468 032 bytes, sha
> `4c2b91fbae6f…`) — that build is what would invalidate a measurement. See `docs/RE.md`.

- Loop: `just forge` = `split` → `lift` → `cc` → `build` → `verify` → `report`.
- **Two production paths**, both verified to the byte:
  - **A — `nie-asm`**: x86-64 encoder in the MSVC dialect; the `forge/asm/*.s` source is
    reassembled. Dialect suffixes: `.s` (short branch), `.w` (immediate in long form), `.r` (an
    explicit null REX prefix — MSVC emits them, e.g. `40 53` for `push rbx`).
  - **B — `nie-forge cc`**: **MSVC 14.44 is installed**
    (`…\2022\BuildTools\…\14.44.35207\…\cl.exe`), the toolset that linked `nie.exe`. C sources in
    `decomp/functions/*.c`, annotated `/* @nie 0x… */`, compiled `/O2 /GS- /Gy /Zl`. **Do not use
    MSVC 14.51** (VS 18). This is the path that climbs highest: C expresses the semantics, MSVC
    picks the form.
- **Structured tables**: `.pdata` and `.reloc` are **regenerated from their entries**
  (`nie_pe::image::tables::emit_for`), like the headers — not copied.
- `niers.sqlite` is wired in (`--db`): it **names** the bodies produced in `lifted.s`, and the forge
  **contradicts it** in return. Example of the *shape* such a contradiction message takes (the
  numbers themselves are now stale, do not quote them as a current cross-check):
  `cross-check pdata_roots_db=50674 forge=55351`.
- **Facing a plateau, do not guess**: enrich the diagnosis (`blocking_detail` breaks it down by
  mnemonic and prints `orig=` against `nie-asm=`), replay `lift`, read. One wave of diagnosis beats
  several waves of blind code — that is the lever worth tens of points.
- **Identity comes first**: `build` fails if `sha256(dist/nie.exe)` differs from the reference.
  Never "fix" that test — it *is* the contract.
- Nothing enters `forge/asm/*.s` that does not re-encode exactly (`lift` checks).
- Never count `semantic` as produced bytes. Only `emitted`/`assembled`/`bytes` count.
- `nie-forge candidates --no-reloc` and `lift`'s `blocker` lines give the next target, with numbers.

## The four data seams — go through the facade

Since the merge (`docs/FUSION.md`), **everything Inazuma Eleven lives here**. The data is split
across four seams, and `@niers/catalog` is the only door:

| Seam | Content | Location |
|---|---|---|
| `jeu` | the game files, decoded on the fly | `nie-model-serve` — `NIE_CDN_URL` |
| `extrait` | 66 `inagle_*` tables | `var/mirror.sqlite` (dated symlink, `scripts/donnees/miroir-inagle.sh`) |
| `re` | the reverse engineering of `nie.exe` | `var/niers.sqlite` |
| `anime` | the series episodes | `data/anime/episodes.db` |

```bash
bun --bun packages/nie-catalog/src/cli.ts etat        # what this machine can answer
bun --bun packages/nie-catalog/src/cli.ts cherche "Mark"
```

- **Never reopen one of these databases by hand**: the facade carries the traps (the mirror is a
  switched symlink; the reverse-engineering reference binary is number `2`, not `1`).
- **Every join carries its confidence** — `cle`, `prefixe` or `texte`. The game and the series share
  **no** common key: matching by name is useful, it never presents itself as a fact.
- **`inagle_game_assets` is NOT the index of game files**: 40 469 of its 40 471 rows are menu PNGs.
  The only complete index is the VFS (`/vfs/find`).
- A seam that is **present can still be empty**: `etat()` measures content, not file existence.
- **Three of these databases ship with the `nie-explorer` installer**: `var/mirror.sqlite`,
  `var/niers.sqlite` and `data/anime/episodes.db`, compressed by
  `scripts/packager-bases-explorer.sh` into `apps/inacord/src-tauri/resources/db/*.gz` (~35 MB) then
  decompressed into `%APPDATA%\dev.niers.explorer\db\` on first launch. `release-desktop.sh` calls
  the packager at step 5/8, **before** the build: afterwards the bundler has already read
  `bundle.resources`.
- Fetching them from the VPS: `scp ovh-vps-direct:/home/ubuntu/niers/var/miroir/inagle-*.sqlite` and
  `…/data/anime/episodes.db`. **`ovh-vps` goes through the VPN (10.8.0.1) and times out** — use the
  `-direct` alias. Copying a SQLite open in WAL: § *Editing pitfalls*.
- The episode catalogue comes from `packages/ietv` (`IETVCache`); its scraper is Node talking to
  YouTube, and only **the database** enters the application.
- The SQL schema lives in `supabase/migrations/` — replayable, idempotent, verified column by column
  against production (811/811). It creates the **shape**; the content comes from the game.
- A migration is only idempotent **when replayed**: `CREATE TABLE IF NOT EXISTS` is not enough, you
  also need the sequences (`IF NOT EXISTS`), the views (`OR REPLACE`) and the constraints (Postgres
  has no `ADD CONSTRAINT IF NOT EXISTS` — guard on `pg_constraint`).
- **`bun run --filter @rosegriffon/inagle push` fails silently** when the cwd breaks `.env.local` and
  `DATA_PATH` diverges per module: `source .env.local` plus explicit `DATA_PATH`/`SUPABASE_URL`.
  After any DDL, `NOTIFY pgrst, 'reload schema'` is mandatory.

## VPS services — one ceiling contradicting another freezes the service

- **A cache budget must stay BELOW `MemoryHigh`.** `nie-model-serve` had
  `NIE_CPK_CACHE_BUDGET_GIB=8` against `MemoryHigh=7G`: the cache filled towards a ceiling the
  cgroup forbade, the kernel put it in permanent reclaim, and the service stopped answering **with
  no incoming request at all**. Misleading symptoms — 67 % CPU, silent `/health`, "saturated"
  workers: you blame the load, the cause is the configuration.
- **The diagnosis is read, not guessed**:
  `for t in /proc/<pid>/task/*; do cat $t/wchan; done | grep -c over_high`. A thread inside
  `__mem_cgroup_handle_over_high` is blocked by cgroup throttling, not by its work.
  `grep RssAnon /proc/<pid>/status` says whether the memory is anonymous.
- Raising a ceiling reserves nothing, but a cache **uses what you give it**: moving the budget to 12
  GiB pushed RSS to 12.9 GiB within minutes. Measure `free -h` afterwards, not before.
- Azalée is a self-hosted Supabase running as native services (rg-postgrest / realtime / storage +
  native postgres). Deploys are blue/green through `deploy.ts`, never a restart.
- **Deploying Azalée: build, THEN `deploy-azalee.sh` (which copies `.next/static`), THEN restart.**
  Skipping the copy gives a 404 on CSS/JS — the site loads unstyled.
- A deploy killed by SIGTERM during the type-check is `earlyoom`
  (`--prefer node --avoid bun`), not the code. Free memory first by **explicit PID**.

## Editing pitfalls

- **An accepted parameter must be honoured.** `/b` declared `q` in its query type and never applied
  it: a client that filters believes it filters, and the whole list passes for a result. That is
  worse than a rejected parameter. Corollary lived the same day: a guard written in ONE code path
  does not cover the others — `?ext=nonexistent` returned the whole directory while announcing
  `ext_inconnue: true`.
- **`sed -i` fails silently in both directions.** Verified: pattern absent → 0 replacements,
  `exit=0`, file untouched and nothing says so; pattern present twice where you meant one → 2
  replacements, `exit=0`. `Edit` fails loudly in both cases (pattern not found, pattern ambiguous).
  To modify a tracked file: **`Edit`, never `sed -i`**. Measured usage: 135 occurrences in this
  repository's history.
- **Never rebuild an `import` block with a regex**: `Mountain as MountainIcon` does not match
  `^\t(\w+),`, the alias vanishes silently, and every page touching the module 500s (lived through
  on `apps/azalee/lib/icons.ts`). Edit the lines, never rewrite the block.
- A `sed` remapping a path (`tools/x` → `plugins/y`) also hits **URLs** carrying the same segment
  (`azalee.rosegriffon.fr/tools/niers` — the Tauri updater endpoint). Re-read afterwards.
- Never write a Rust file through a Python heredoc: a literal `\0` ends up in the source (`file`
  reports it as `data`). Use Write/Edit.
- Do not name a scratchpad script after a stdlib module (`dis.py` breaks numpy and capstone).
- **A function name does not tell you whether it READS or EXECUTES.** `discover_host_calls` and
  `enumerate_header_tabs` in `nie-lua` sound like introspection; they install a metatable on `_G`
  and then **call the script's main function**, on a `Lua::unsafe_new`. Routing them would have
  opened an interpreter on a public endpoint. Read the body, not the name — and make the refusal
  **structural** (`default-features = false` plus a `const { assert!(…) }`) rather than declarative:
  a policy that depends on the next caller's discipline is not one.
- **A palette change is a CONTRAST change.** A character in white kit on a sky shifted to cream: DOM
  right, URL right, PNG right, **nothing on screen**. Two individually correct fixes can cancel each
  other. What gets verified is the screen.
- **A single probe cannot measure a changing state.** A `useEffect` with empty dependencies on
  `/api/v1/health` while the VFS mounts in the background: the waiting screen would never have
  flipped. An asynchronous state is polled until it settles.
- **Headless Chrome NEVER composites a WebGPU canvas** (SwiftShader): a twenty-line witness reads
  back `0,0,0,0`. Do not blame the shader — prove it offscreen with a `copyTextureToBuffer` pass.
  And when translating GLSL → WGSL, adapt depth: WebGPU has NDC **z ∈ [0,1]**, not [-1,1]; with the
  OpenGL form the model is clipped without any value looking wrong.
- **A corpus split is COUNTED, not declared.** Six VFS batches announced as disjoint:
  `data/dx11/effect/` was in two of them. Verify with `sort -u` on the union and by the sum, before
  sending six agents to work on it.
- **A count quoted in a document carries its command and its date.** This repository has been wrong
  about its own figures: 440 screens instead of **475**, 99 `pub fn` in `nie-lua` instead of **34**,
  "24 unpositioned objects" instead of **0**. A number without a command is a memory, not a
  measurement.
- **A test that CANNOT fail is worse than no suite: it reassures.** A gamut check written on
  `palette::FromColor::from_color` was green whatever you fed it — that conversion clamps by itself
  (`from_color_unclamped(t).clamp()`), so no colour ever comes out "out of range". Same family as
  the `0 passed` and the piped `$?`: the suite runs, passes, and verifies nothing. **Prove a test by
  falsification** — deliberately break the value it guards and watch it go red — before relying on
  it.
- **A `rg` gate written over source COUNTS ITS OWN EVIDENCE.** Three of this plan's gates were
  measured on 2026-09-06 and all three were wrong — not the code, the **instrument**. A rule
  written to forbid a pattern necessarily contains that pattern, so the documentation stating the
  ban matches the ban (`-g '!*.md'`); a comment explaining that `@rosegriffon/ui` **was removed**
  matches the very grep that hunts it; and a motif can simply name the wrong thing — `DATABASE_URL`
  was counted as "reads something local" when it is a **remote** Postgres connection string, so the
  gate demanded breaking authentication to reach 0. Symptom to recognise: a gate that can only be
  satisfied by damaging what it protects. Fix the motif, exclude the proof in the **command** (never
  "0, except two cases we know"), and **falsify it** — drop a real violation, watch it count, remove
  it, watch it drop. Trap inside the trap, paid the same day: `--glob '!src-tauri/x.json'` does not
  bite (rg matches the full relative path), it takes `'!**/src-tauri/x.json'` — the corrected gate
  returned 1 while believing itself right.
- **To falsify a guard, copy the file — NEVER `git checkout` it back.** The falsification ritual
  above (break the guard, watch it go red) ends with a restore, and `git checkout <file>` restores
  the file to HEAD: it silently threw away an entire uncommitted patch on 2026-09-06. Use
  `cp X /tmp/X.sauv`, break, run, `cp /tmp/X.sauv X`.
- **A witness corpus must be able to make the test fail.** The same day, a test on
  `prefixe` + `ext` stayed green with the guard removed: the fixture's only two `.bin` lived in
  the same subtree, so `ext=bin` already returned the right count. Falsification does not only
  check the guard — it checks that the FIXTURE discriminates. Same defect on a measurement script
  whose search pattern matched 0 lines: a filter that finds nothing proves nothing.
- **A screenshot does not prove an ABSENCE.** A page rendered by headless Chrome showed everything
  but one sprite; the component was blamed for an hour. The DOM (`--dump-dom`) carried the element,
  its size and the right background position: it was the 1.5 MB atlas that was not decoded within
  `--virtual-time-budget=3000`. Check the DOM before the render, and remember that a loading
  `background-image` displays nothing **and does not say so**.
- **Inside a scaled canvas, factors MULTIPLY.** A sprite at 1.35 in a 1280 canvas rendered at 1440
  is displayed at ×1.52: it aliases, and no value in the code is wrong. An element's scale is
  reasoned in **rendered** pixels, not canvas pixels.
- **`format!("{:?}")` is not serialisation.** On an `Option` it published `"Some(V2)"` into JSON
  meant to be read: the Rust name of a variant, wrapped in its container. A public field is
  `match`ed to a chosen string.
- **Nothing technical on the front page.** Service name, version, API endpoint, `sitemap.xml`,
  `llms.txt`, index count, GitHub repository, the site's own domain printed on the site: all of that
  was on Aphrody's home page. Those routes stay served — for robots and agents — but an interface
  shows what you can DO, never what makes it run. Corollary measured on 2026-09-06: the same
  information appears in **exactly one place** (the total was there three times), and an affordance
  is verified before it is drawn (two key hints, "F" and "V", for zero `keydown` in the whole front
  end).
- **A comparison layer has no place in production.** The game menu export was rendered under the
  interface at 18 % opacity: invisible to the eye, but its text stayed in the document and came out
  in scrapes — labels from ANOTHER game screen, read by screen readers and search engines. **Opacity
  erases nothing.**
- **A test calling the HANDLER does not test the ROUTER.** `/en/manifest.webmanifest` returned HTML
  while the unit test, which called the function with a URI, was green: the handler knew how to read
  the prefix, the router did not know the URL. A manifest served as `text/html` does not fail, it is
  ignored. Test through the router, and assert the `Content-Type` as much as the body.
- **In an HTTP audit, a dropped connection (`code 0`) is NOT a failure.** Counted as one, it
  measures the service's saturation and presents it as its coverage: `.usm` was reported at 0 %
  decoding when all five measurements were dropped connections. Count `indeterminate` separately and
  retry once — but believing an HTTP status code the first time is an answer.
- **Extracting table names with `from\s+(\w+)` over TypeScript** returns "next", "react", "lucide":
  `import … from` outnumbers the SQL. Keep only blocks carrying a SQL verb before looking for the
  table.
- **Naming an exported sub-entity**: a cue or payload exported under the SOURCE file's name makes
  every download overwrite the others. Name by the sub-entity, resolve by identifier never by rank,
  and always send `Content-Disposition`.
- `comm` requires an `LC_ALL=C sort`: without it, it reports "0 differences" on files it is in fact
  refusing to compare (the `not in sorted order` message goes to stderr).
- **Node's `path.join` follows the HOST platform, not the path's shape**: a POSIX `HOME`
  (`/home/ubuntu`, a Linux service's) comes back as `\home\ubuntu\…` on Windows, and the test
  expecting it goes red on that machine only. Use `posix.join` when the base starts with `/` (seen
  on `wonderbot/src/config.ts`).
- Copying a SQLite open in WAL **without its `-wal`** loses recent writes (42 missing episodes). Use
  `sqlite3 src ".backup 'dest'"`.
- A page rendering the right title can still be a 500: **starting the service** is what finds the
  bug, not re-reading the diff.

## `.gitignore` — what disappears in silence

An ignored file raises **neither error nor warning**: it simply does not exist for the next person.
This is the repository's most expensive failure mode, and it has happened more than once.

- **Git NEVER descends into an excluded directory.** `!data/oc/` alone brings back nothing while
  `data/` is ignored: you must re-include the parent (`!/data/`), re-exclude its direct content
  (`/data/*`), **then** re-include the target. Same rule for a subtree: write `.agents/**` (plus
  `!.agents/**/`), never `.agents/`, if you want to re-include inside it.
- **A `.gitignore` no longer applies to an already-tracked file.** `CLAUDE.md` and `AGENTS.md` only
  survived the `*.md` rule because they had been tracked **before** it. Any instruction file created
  afterwards left the repository without a word — on a fresh clone, the agent started with no
  instructions at all.
- **The last matching rule wins**: a re-inclusion placed before a broad rule (`*.md`, line 166 at the
  time) does nothing. Verify **every** case with `git check-ignore -v <file>`, never by reasoning.
- **askama resolves its templates at COMPILE time.** The `*.txt` rule pushed `nie-site`'s four
  templates out of the repository — including `robots.txt` and `security.txt`, written on day 5 and
  never versioned: on a fresh clone the crate did not compile. Same trap as the `CMakeLists.txt`.
  Check every new non-code file with `git check-ignore -v`.
- **A fresh clone does NOT typecheck, and that is deliberate.** `packages/*/src/data/**/*.json` is
  ignored on purpose (`.gitignore:26`, commented): those are generated manifests and LEVEL-5 game
  content, which must never be committed. Consequence measured 2026-09-06 on a fresh Windows
  clone: **81 `TS2307`**, all of them missing `../data/*.json`. This is not a `.gitignore` bug and
  the fix is not a re-inclusion — it is a **bootstrap step**: generate the data (`export_*`,
  `niers refresh-typed-json`) or fetch it from the VPS, then re-run `bun run typecheck` (0 errors,
  29/29 workspaces). Anyone diagnosing those 81 errors as a code defect will "fix" imports that are
  correct.
- Since 2026-09-05, **all Markdown in the repository is versioned**, with no exception list: that
  list had made `AGENTS.md`, the plugin's 5 sub-agents and 5 skills (a deliverable) and the OC
  READMEs disappear. Still outside, each for a measured reason: installation artefacts, `/refs/` (a
  **complete** 124 MB git repository), and `/var` plus `data/` except `data/oc/` — letting a few
  `.md` in there would mean re-including their directories, so making every `git status` walk 15.5
  GB and 111 GB.
- In the C++ tree, `*.txt` and `*.md` are globally ignored; the `CMakeLists.txt`, the READMEs and
  `plugins/niers-plugin/**/*.md` are explicitly re-included. Do not remove those `!…` lines —
  without them the whole C++ build chain leaves the repository.

## Before renaming or moving — who points at it from outside?

- `/etc/systemd/system/nie-miroir.service` hard-codes `scripts/donnees/miroir-inagle.sh`, its timer
  is active, and its `ExecStartPost` restarts `nie-model-serve`. Moving it breaks the nightly mirror
  rotation; repairing it needs a `daemon-reload`, therefore the user's agreement. That directory was
  **not** anglicised for this reason, while the rest of `scripts/` was.
- Reflex: `systemctl list-unit-files`, `rg` through `deploy/`, and search for the absolute path
  before any `git mv` of a script.
- **An external daemon commits under `chore(sync): checkpoint <timestamp>`.** It does not
  distinguish authors and can capture a batch **mid-flight**. Re-read `git log` before concluding a
  commit is its own.

## Desktop app release — one single command

`scripts/release-desktop.sh <X.Y.Z>` does everything and is **idempotent**: bumps the 9 manifests
and the lockfiles, `cargo check`, zips the Blender extension, builds **signed** msi+nsis, commits,
tags, pushes, creates the GitHub Release. It requires a clean tree, `main`, `gh`, and a still-free
tag.

- **Never replay its steps by hand.** `bun run tauri build` alone produces the bundles then fails on
  `TAURI_SIGNING_PRIVATE_KEY`: you end up with **unsigned** installers next to stale `.sig` files
  from an earlier release — indistinguishable, and the updater will refuse them.
- The script checks installer **size** (msi ≥ 5 MB, nsis ≥ 3 MB): a bundle can be perfectly signed
  and not contain the application (it happened with `export-bindings.exe`).
- The key `~/.tauri/niers.key` is **one line** and its password is **empty**: a `cat`/`head` on it
  leaks the whole thing. Pass it with `-f`/`TAURI_SIGNING_PRIVATE_KEY_PATH`, never print it.
  Regenerating the pair invalidates the updater for every already-installed client.
- Nothing to deploy on the VPS side: `azalee.rosegriffon.fr/tools/niers` and `/latest.json` read the
  latest GitHub release live (1 h cache).

## Product — Aphrody, Inacord, nie, Azalée (frozen 2026-09-05)

Four names, fixed by the user:

- **Azalée** — the wiki (`azalee.rosegriffon.fr`, Vercel serverless, Rose Griffon art direction).
- **Aphrody** — the tools site (`aphrody.com`). **Neither a wiki nor a file explorer**: the wiki is
  Azalée, the explorer is Inacord. Its interface **reproduces the game's main menu**, and not from
  memory — `nie-game --runtime --menu <screen> --export-layout` returns the real layout. An Aphrody
  interface showing file listings has drifted into Inacord's job.
- **Inacord** — the desktop and mobile application (`apps/inacord`, formerly `nie-explorer`; the
  Tauri identifier is kept). Its art direction is InaCord, the in-game messaging app.
- **nie** — the game. The crates keep their `nie-*` prefix.

Aphrody is served by the **100 % Rust** `nie-site` crate, `publish = false`, under `crates/tools/`:
Axum 0.8, Tokio 1.x, Tower, `askama`, `moka`, `rusqlite` read-only. No Bun/Node server, no Leptos,
no SQLx. It listens **only** on `127.0.0.1:8085`, behind nginx which terminates TLS. It provides
`/healthz`, `/robots.txt`, `/.well-known/security.txt`; the API lives under `/api/v1/`, paginated,
with no infrastructure detail; `nie-model-serve` is reached **only** through its proxy. Route tests
that **count**, plus clippy with no warnings, before enabling nginx. Frozen stack: `docs/stack/`.

**State — these packages EXIST, they are not to be built.** `nie-site` serves **80 mounted
routes** with **316 green tests** and clippy at 0 (re-measured 2026-09-06 evening:
`curl …/api/v1/couverture | jq .routes_montees`, `cargo test -p nie-site`; it was 13 routes and
44 tests on 2026-09-05, and 56/220 the same morning — which is how fast this number goes stale).
`scripts/e2e-site.sh` runs 65 checks with no failure against the real binary, and Aphrody mounts
the shared interface: 4 searchable catalogues, `/b` navigation with filters, `/recherche` over the
255 308 entries, `/donnees` over the **224 tables of two datasets**, audio and video playback.
The filter matrix is **measured, not maintained**: `scripts/validation/mesurer-matrice-filtres.sh`
(41 served / 5 absent / 2 client-side) and `scripts/validation/mesurer-filtres.sh` (14/14 applied
**and** republished). Two rules follow for whoever picks it up:

- **The deploy loop, in this order** (a front-only change skips the first two):
  `cargo build --release -p nie-site` → `sudo systemctl restart nie-site` → **wait for
  `curl …/api/v1/health | jq -r .capacites.vfs` to read `pret`** (≈20 s; measuring before that
  reads a service that is up and an index that is empty) → interrogate and count.
  `bun run --filter '*nie-web*' build` writes `apps/nie-web/dist`, which `nie-site` serves
  **immediately** — no restart, and the asset hash in the served HTML is how you check the bundle
  actually changed.
- **Verify a page with `chromium --headless --dump-dom`, and compare it to the API.** The useful
  assertion is not "the page rendered" but "its first row is the one the sorted API returns".
  Two traps paid on 2026-09-06: a page listing 200 thumbnails times out and dumps an **empty**
  file (lower `par_page`, budget 6000–9000 ms), and the rendered count uses a narrow no-break
  space — `54 203` does not match a `[0-9 ]+` grep, which silently reads `203`.
- **Adding one page to the site turns SIX independent count assertions red**, across
  `routes/well_known.rs` (`PLAN: [UrlPlan; N]`, `<url>`, `xhtml:link` ×4, `x-default`, `lastmod`,
  `Allow:` under two regimes) and `tests/routes.rs`. That is the design working: a page added
  half-way cannot pass.
- **Never write a host condition inside a component** (see § *Polyglot doctrine*).
- **`apps/nie-web/src/legacy/` is an airlock, not a library.** Excluded from `tsconfig`, it holds
  code pulled out of the wiki until it is rewritten against `/f`, `/b` and `/api/v1`. Its Rose
  Griffon mentions will disappear with it — tidying them first would be working on condemned code.

**Ownership.** Only Azalée is a **Rose Griffon** product. Aphrody, Inacord and nie are
**`aphrody-dev`** projects: no brand, no mention, no `rosegriffon.fr` URL, no `@rosegriffon/*`
package and no shared account inside `nie-site`, `nie-web`, `inacord-ui`, `apps/inacord`. One
temporary exception: the updater of installed 0.5.x releases still reads
`azalee.rosegriffon.fr/tools/niers/latest.json`, which redirects to
`aphrody.com/downloads/inacord/latest.json`.

Inazuma Eleven content is exploitable under Official Commercial Agreement N° RG-L5-VR-2026-001 —
which is **signed by Rose Griffon**: the legal basis for exploiting it on an `aphrody-dev` site is
**for the user to confirm**, and no agent presumes it. The agreement expressly authorises creating
and operating sites, games, mods and derived content, and distributing Inazuma Eleven graphical and
audio assets. **Never** a personal data item, a secret, a credential, a machine path or a dump
outside the contractual scope.

On `aphrody.com`, only `aphrody.com` and `www` reach `nie-site`; `api.`, `downloads.`, `cdn.`,
`bot.`, `admin.`, `mcp.`, `bxc.` and `n2b.` stay with the `aphrody` repository (`aphrody-site`,
:8083), whose `docs/SITES-PLATFORM.md` planned "Niers" on `nie.aphrody.com`: to be amended by its
owner.

## Anti-hallucination — the standing rule

Never invent a mode, a menu, a label, an item, a stat or a structure. Look **first** in
`data/*.cfg.bin.json` (menu_text, settings), `data/re/`, the VFS, uemu, `refs/`. If it is not
there, write "to be verified" before writing anything else.

Every claim in this file carries its command and its date. Yours should too.
