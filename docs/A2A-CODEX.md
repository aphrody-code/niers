# A2A wire protocol — how the agents on this repository talk

This file owns **one thing**: the mechanics of agent-to-agent messaging on `/home/ubuntu/niers`.
The rules about *not overwriting each other* live in [`AGENTS.md`](../AGENTS.md) § 2, and every
repository rule lives in [`CLAUDE.md`](../CLAUDE.md). Neither is repeated here.

The machine-readable version of this document is [`ai.json`](../ai.json) (A2A v1.0).

## Who is who

| Agent | A2A identity | Role | Launch |
|---|---|---|---|
| Claude Code | `claude@aphrody-code/niers` | orchestrator — splits, arbitrates | interactive session |
| Codex | `codex@aphrody-code/niers` | executor — works its scope | `codex exec --cd /home/ubuntu/niers` |

## Three channels, not interchangeable

### 1. `aphrody a2a` — coordination, asynchronous and traced

The default channel for anything that must stay written down: taking a scope, reporting a
blocker, handing back a result.

```bash
# Send (from the repository root, which carries ai.json)
aphrody a2a tick --iteration <n> --side codex --peer claude \
  --kind fact --subject "<subject>" --body "<one measured line>"

# Read what the other one wrote
tail -5 .coord/inbox-from-claude.jsonl | jq -c '{ts,topic,body}'   # Codex side
tail -5 .coord/inbox-from-codex.jsonl  | jq -c '{ts,topic,body}'   # Claude side
```

`--side` is the sender, `--peer` the recipient. The envelope is appended to
`.coord/inbox-from-<side>.jsonl` and the sender's heartbeat is dated.

**`--kind` accepts two values, and it rejects the others in silence.** Measured: `fact` and
`ping` pass; `claim`, `done`, `block` and `status` all fall back to `ping` with no error. A
`--kind done` therefore does not produce a `done` envelope — it produces a `ping`, and the
intent is lost. So the message type is encoded in the **subject**, as a prefix:

| Intent | Command |
|---|---|
| I am taking this scope | `--kind fact --subject "claim: <paths>"` |
| Here is a measured result | `--kind fact --subject "fact: <subject>"` |
| I am blocked, arbitrate | `--kind fact --subject "block: <subject>"` |
| My batch is finished | `--kind fact --subject "done: <batch>"` |
| Work on this next | `--kind fact --subject "goal: <objective>"` |
| I am alive | `--kind ping` |

**A `fact` carries a measurement, not an intention.** "47 files moved, clippy 0 warnings" is a
fact; "I think it works" is not.

### 2. The JSON-RPC listener — synchronous

```bash
aphrody a2a serve --bind 127.0.0.1:8792     # already up if /ping answers
curl -s http://127.0.0.1:8792/ping
```

For exchanges that need an immediate answer. **`8792` is this repository's port; `8788` belongs
to the `aphrody` repository** — do not confuse them.

### 3. MCP — tools, never messages

Both agents share the same MCP servers, declared in `~/.config/aphrody/mcp.json` and
`~/.codex/config.toml`:

- `aphrody` — documentation search (`docs_auto_search`), reverse engineering (`re_triage`,
  `re_disasm`), and `aphrody_mcp_call` to bounce onto any other server;
- `niers-game` — game VFS, assets, RE knowledge base, explorer control.

```bash
aphrody mcp list
aphrody mcp call --server niers-game --tool vfs_search --args '{"query":"..."}'
```

MCP is for **acting**, never for coordinating: an MCP call leaves no trace the other agent can
read. Anything that must be known goes through `a2a tick`.

## Starting a two-agent session

```bash
# 1. The channel (Claude)
curl -s http://127.0.0.1:8792/ping || aphrody a2a serve --bind 127.0.0.1:8792 &
aphrody a2a tick --iteration 0 --side claude --peer codex --kind fact \
  --subject "claim: scope" --body "claude: <paths> | codex: <paths>"

# 2. The work (Codex), non-interactive, writes bounded to the repository
codex exec --cd /home/ubuntu/niers -s workspace-write "<instruction, scope included>"

# 3. Handing back (Codex), then review and commit
aphrody a2a tick --iteration 1 --side codex --peer claude --kind fact \
  --subject "done: <batch>" --body "<files touched> ; clippy 0 warnings"
```

## The autonomous loop — the agents set each other's objectives

`scripts/a2a-loop.sh` runs **one turn** for one side:

```bash
bash scripts/a2a-loop.sh codex     # Codex executes the goal Claude set for it
bash scripts/a2a-loop.sh claude    # Claude executes the goal Codex set for it
```

A turn chains three things: read the latest message whose subject starts with `goal:` in the
peer's inbox, execute it, then emit **two** ticks — the measured result (`done: …`) **and the
next objective for the peer** (`goal: …`).

That second tick is what keeps the loop running: each agent feeds the other. Once the first
objective is seeded, nothing comes from outside.

- Only a `goal:`-prefixed subject counts as an order to work. A `fact` or a `ping` triggers
  nothing — otherwise any courtesy message would start a turn.
- With no objective received, the agent picks one itself, bounded and disjoint from what the
  peer announced.
- The iteration counter lives in `.coord/iteration`, the journal in `.coord/loop-<side>.log`.

Seeding it:

```bash
aphrody a2a tick --iteration 0 --side claude --peer codex --kind fact \
  --subject "goal: <objective>" \
  --body "scope: <paths> | success criterion: <measurement>"
bash scripts/a2a-loop.sh codex
```

### What the loop does not do, and why

It does not commit on the agent's behalf — an automatic commit hides what actually changed. It
does not write outside the repository, and it touches neither `/etc` nor any service: 18
production services run on this machine, and an agent restarting one while the other is
measuring produces a false result with nothing to signal it.
