# Claude Code Plugins — Complete Reference

> Sources:
> - https://code.claude.com/docs/en/plugins
> - https://code.claude.com/docs/en/plugins-reference
> - https://code.claude.com/docs/en/plugin-marketplaces

## Plugin vs Standalone vs Skill vs MCP

| Approach | Names | Scope | Best for |
|----------|-------|-------|----------|
| **Standalone** (`.claude/`) | `/hello` | Single project | Personal workflows, quick experiments |
| **Plugin** (`.claude-plugin/`) | `/plugin:hello` | Shareable | Team distribution, versioned releases, reusable across projects |
| **Skill** (SKILL.md only) | Model-invoked | Single project or plugin | Teaching Claude domain knowledge, auto-triggered by context |
| **MCP server** | Tool calls | Global or project | External tool/API integration, real-time data access |

### When to use what

- **Standalone**: personal, single project, short names, quick iteration
- **Plugin**: sharing with team/community, version control, marketplace distribution, namespaced
- **Skill inside plugin**: Claude auto-invokes based on context, domain expertise
- **Command inside plugin**: user explicitly invokes via `/plugin:command`
- **MCP server inside plugin**: connect Claude to external services/APIs
- **Hook inside plugin**: auto-run code on events (file save, tool use, session start)
- **Agent inside plugin**: specialized subagent for specific tasks
- **LSP server inside plugin**: code intelligence (go-to-definition, diagnostics)

## Plugin Directory Structure

```
my-plugin/
├── .claude-plugin/           # ONLY plugin.json goes here
│   └── plugin.json           # Manifest (name is only required field)
├── skills/                   # Skills with SKILL.md structure
│   └── skill-name/
│       └── SKILL.md
├── commands/                 # User-invoked commands (markdown files)
├── agents/                   # Subagent definitions (markdown files)
├── hooks/                    # Event handlers
│   └── hooks.json
├── bin/                      # Executables added to PATH
├── output-styles/            # Output style definitions
├── settings.json             # Default settings (only "agent" key supported)
├── .mcp.json                 # MCP server definitions
└── .lsp.json                 # LSP server configurations
```

**Critical:** commands/, agents/, skills/, hooks/ must be at plugin root, NOT inside .claude-plugin/.

## Plugin Manifest — plugin.json

```json
{
  "name": "plugin-name",           // Required — kebab-case, becomes namespace
  "version": "1.0.0",             // Semantic versioning
  "description": "Brief description",
  "author": {
    "name": "Author Name",
    "email": "author@example.com",
    "url": "https://github.com/author"
  },
  "homepage": "https://docs.example.com",
  "repository": "https://github.com/author/plugin",
  "license": "MIT",
  "keywords": ["keyword1", "keyword2"],
  "commands": ["./custom/commands/"],
  "agents": "./custom/agents/",
  "skills": "./custom/skills/",
  "hooks": "./config/hooks.json",
  "mcpServers": "./mcp-config.json",
  "outputStyles": "./styles/",
  "lspServers": "./.lsp.json"
}
```

### User Configuration (prompted at enable time)

```json
{
  "userConfig": {
    "api_endpoint": { "description": "Your team's API endpoint", "sensitive": false },
    "api_token": { "description": "API authentication token", "sensitive": true }
  }
}
```

Available as `${user_config.KEY}` in configs and `CLAUDE_PLUGIN_OPTION_<KEY>` env vars. Sensitive values go to system keychain.

## Installation Scopes

| Scope | Settings file | Use case |
|-------|--------------|----------|
| `user` | `~/.claude/settings.json` | Personal, all projects (default) |
| `project` | `.claude/settings.json` | Team, shared via version control |
| `local` | `.claude/settings.local.json` | Project-specific, gitignored |
| `managed` | Managed settings | Admin-controlled, read-only |

## Skills in Plugins

`skills/<name>/SKILL.md`:

```yaml
---
name: code-review
description: Reviews code for best practices. Use when reviewing code or checking PRs.
---

Instructions here.
```

Can include references/, scripts/, assets/ alongside SKILL.md.

## Commands in Plugins

`commands/<name>.md`:

```yaml
---
description: Deploy the current branch
disable-model-invocation: true
---

Deploy instructions here. User input available as $ARGUMENTS.
```

## Agents in Plugins

`agents/<name>.md`:

```yaml
---
name: agent-name
description: What this agent does
model: sonnet
effort: medium
maxTurns: 20
disallowedTools: Write, Edit
---

System prompt here.
```

Supported: name, description, model, effort, maxTurns, tools, disallowedTools, skills, memory, background, isolation ("worktree").
NOT supported in plugins: hooks, mcpServers, permissionMode.

## Hooks in Plugins

`hooks/hooks.json`:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [{ "type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/scripts/lint.sh" }]
      }
    ]
  }
}
```

Hook types: `command`, `http`, `prompt`, `agent`.

Events: SessionStart, UserPromptSubmit, PreToolUse, PermissionRequest, PermissionDenied, PostToolUse, PostToolUseFailure, Notification, SubagentStart, SubagentStop, TaskCreated, TaskCompleted, Stop, StopFailure, TeammateIdle, InstructionsLoaded, ConfigChange, CwdChanged, FileChanged, WorktreeCreate, WorktreeRemove, PreCompact, PostCompact, Elicitation, ElicitationResult, SessionEnd.

## MCP Servers in Plugins

`.mcp.json`:

```json
{
  "mcpServers": {
    "my-server": {
      "command": "${CLAUDE_PLUGIN_ROOT}/servers/server",
      "args": ["--config", "${CLAUDE_PLUGIN_ROOT}/config.json"],
      "env": { "DB_PATH": "${CLAUDE_PLUGIN_ROOT}/data" }
    }
  }
}
```

## LSP Servers in Plugins

`.lsp.json`:

```json
{
  "go": {
    "command": "gopls",
    "args": ["serve"],
    "extensionToLanguage": { ".go": "go" }
  }
}
```

Required: command, extensionToLanguage. Optional: args, transport, env, initializationOptions, settings, restartOnCrash, maxRestarts.

## Environment Variables

- `${CLAUDE_PLUGIN_ROOT}` — plugin install directory (changes on update)
- `${CLAUDE_PLUGIN_DATA}` — persistent data directory (~/.claude/plugins/data/{id}/)

## Marketplace — marketplace.json

`.claude-plugin/marketplace.json`:

```json
{
  "name": "marketplace-name",
  "owner": { "name": "Team Name", "email": "team@example.com" },
  "metadata": {
    "description": "Brief marketplace description",
    "version": "1.0.0",
    "pluginRoot": "./plugins"
  },
  "plugins": [
    {
      "name": "plugin-name",
      "source": "./plugins/plugin-name",
      "description": "What the plugin does",
      "version": "1.0.0"
    }
  ]
}
```

### Plugin Sources

| Source | Format | Example |
|--------|--------|---------|
| Relative path | string | `"./plugins/my-plugin"` |
| GitHub | object | `{ "source": "github", "repo": "owner/repo", "ref": "v1.0" }` |
| Git URL | object | `{ "source": "url", "url": "https://gitlab.com/team/plugin.git" }` |
| Git subdirectory | object | `{ "source": "git-subdir", "url": "https://github.com/org/mono.git", "path": "tools/plugin" }` |
| npm | object | `{ "source": "npm", "package": "@org/plugin", "version": "^2.0.0" }` |

All git sources support optional `ref` (branch/tag) and `sha` (exact commit).

### Strict Mode

- `true` (default): plugin.json is authority, marketplace supplements
- `false`: marketplace entry is entire definition, plugin must not have its own component definitions

## CLI Commands

```bash
# Install/uninstall
claude plugin install <plugin>[@marketplace] [--scope user|project|local]
claude plugin uninstall <plugin> [--keep-data]

# Enable/disable
claude plugin enable <plugin> [--scope ...]
claude plugin disable <plugin> [--scope ...]

# Update
claude plugin update <plugin> [--scope ...]

# Marketplace management
claude plugin marketplace add <source> [--scope ...] [--sparse <paths>]
claude plugin marketplace list [--json]
claude plugin marketplace remove <name>
claude plugin marketplace update [name]

# Validate
claude plugin validate .

# Test locally
claude --plugin-dir ./my-plugin
```

## Converting Standalone to Plugin

1. Create `my-plugin/.claude-plugin/plugin.json` with `name`
2. Copy `.claude/commands/` → `my-plugin/commands/`
3. Copy `.claude/agents/` → `my-plugin/agents/`
4. Copy `.claude/skills/` → `my-plugin/skills/`
5. Move hooks from settings.json → `my-plugin/hooks/hooks.json`
6. Test: `claude --plugin-dir ./my-plugin`

## Team Distribution via Project Settings

`.claude/settings.json`:

```json
{
  "extraKnownMarketplaces": {
    "company-tools": {
      "source": { "source": "github", "repo": "your-org/plugins" }
    }
  },
  "enabledPlugins": {
    "formatter@company-tools": true
  }
}
```

## Common Issues

| Issue | Cause | Fix |
|-------|-------|-----|
| Plugin not loading | Invalid plugin.json | `claude plugin validate .` |
| Commands missing | Wrong directory | Components at root, not in .claude-plugin/ |
| Hooks not firing | Not executable | `chmod +x script.sh` |
| MCP server fails | Absolute paths | Use `${CLAUDE_PLUGIN_ROOT}` |
| Files not found | Outside plugin dir | Use symlinks or restructure |
| Relative paths fail | URL-based marketplace | Use git/github/npm sources instead |
