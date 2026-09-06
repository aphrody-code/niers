# AGENTS.md — the entry point for every agent working on `niers`

Several agents work this repository **at the same time** (Claude Code, Codex, and whatever
comes next). This is the first file every one of them reads, whatever its engine. It fits on a
screen on purpose: it owns **only** what is specific to working *alongside another agent*.
Everything else lives in one place and is linked, never repeated.

| What you need | Where it lives — the **only** place it is written |
|---|---|
| Every rule about this repository (tools, build, traps, data, forge, RE, services) | [`CLAUDE.md`](CLAUDE.md) — authoritative for **all** agents, not just Claude |
| The A2A wire protocol (channels, message kinds, autonomous loop) | [`docs/A2A-CODEX.md`](docs/A2A-CODEX.md) |
| The machine-readable agent card | [`ai.json`](ai.json) (A2A v1.0) |
| The single direction, and which plan commands which | [`docs/README.md`](docs/README.md) § *La direction* |
| The mission in progress, its gates, and the six moves that need a go | [`docs/CODEX-JOUR-UNIQUE.md`](docs/CODEX-JOUR-UNIQUE.md) — since 2026-09-06 Codex owns all of `/PLAN.md` and commits its own batches **inside `niers`**, which replaces the 2026-09-05 repository split |
| What ships and how it is built, for humans | [`README.md`](README.md) |

If this file and a habit disagree, this file wins. If this file and `CLAUDE.md` seem to
disagree, `CLAUDE.md` wins on repository rules — and the overlap is a bug in this file, because
it is not supposed to have any.

---

## 1. Language — English to name, French to answer

Decided by the user on **2026-09-06**, and it binds every agent: `niers` is a **worldwide**
project. **Think in English** (or Japanese); translate only when you speak to the user.

- **English** — everything a machine or a non-French reader parses: file and directory names,
  variables, functions, types, fields, constants, modules, **URLs, route patterns, query
  parameters, site slugs, public JSON keys**, CLI commands, new tables and columns, and the
  Markdown written for agents (this file, `CLAUDE.md`, `README.md`).
- **French** — one thing only: the prose you address to the user (reports, summaries,
  explanations).

A French identifier now means you thought in the wrong language.

The existing debt is large and is **not** migrated in one sweep, and never with `sed`. Every
**new** name is English. An **already-served public API** is renamed only by a dedicated batch,
with a redirect or dual serving — never in passing, because renaming a route or a JSON key
breaks its consumers. Internal names may be fixed while you already hold the file. Product
names stay frozen: Azalée, Aphrody, Inacord, nie, `niers`, the `nie-*` crates, the `inagle_`
table prefix.

## 2. Never overwrite another agent

Two agents in one worktree overwrite each other **in silence**. The only protection is a
**disjoint scope, announced before writing**.

1. **Announce before you write.** A `claim:` subject names the paths you take, in plain text.
   With no claim, you touch nothing outside your batch. There is no exception to this.
2. **Scope is not the same thing as compilation.** You can respect your scope to the letter and
   break four files that are not yours: changing the **signature** of a shared function
   (`IndexVfs::page_filtree` taking a seventh parameter) breaks every caller, including the ones
   another agent is writing right now. Extend a shared signature with an options struct
   (`#[derive(Default)]`) and keep the short form delegating to it — existing callers then
   compile untouched.
3. **Arbitration files belong to Claude alone**: `CLAUDE.md`, `AGENTS.md`, `.gitignore`,
   `justfile`, the root manifests, `/PLAN.md`, `docs/CODEX-JOUR-UNIQUE.md`. Need a change there?
   Send a `block:` describing the line you want. The rest of `docs/` is open during a mission.
4. **One author per batch** (amended 2026-09-06; it used to be "one commit author"). A batch is
   one commit, by whoever wrote it, carrying its measured gate result in the message. The
   original rule assumed a single writer; with two, it turns one agent's work into an anonymous
   commit by the other — which happened here, `188e409` captured three files mid-flight.
5. **Nothing destructive, nothing in production, without agreement**: no `rm -rf`, no
   `git reset --hard`, no `git checkout --` on a file you did not write, no service restart, no
   write outside the repository. **`pkill -f` is forbidden** — it kills agent sessions. Target a
   PID.

## 3. Before you call a batch done

A batch is `done` only when the check has **actually run**, and the `done:` carries its number.
The gate, what saturates the disk, and every way a green suite can be lying to you are in
[`CLAUDE.md`](CLAUDE.md) § *Build and test* — read it once, it is the section that costs the
most when skipped.

## 4. Product constraint

One hard rule you will otherwise break by accident: **Aphrody is neither a wiki nor a file
explorer** — the wiki is Azalée, the explorer is Inacord. An Aphrody interface showing file
listings has drifted into Inacord's job. The four frozen names, ownership, the served stack and
what may never appear on the site are in [`CLAUDE.md`](CLAUDE.md) § *Product*.
