# Configuration

Claw Code loads configuration from a hierarchy of JSON files, with later entries overriding earlier ones.

## Config file resolution order

Files are loaded in this order. Later files override earlier ones:

| Priority | Path | Scope |
|----------|------|-------|
| 1 (lowest) | `~/.claw.json` | User-global |
| 2 | `~/.config/claw/settings.json` | User-global (XDG) |
| 3 | `<repo>/.claw.json` | Project |
| 4 | `<repo>/.claw/settings.json` | Project |
| 5 (highest) | `<repo>/.claw/settings.local.json` | Project-local (gitignored) |

The config home directory can be overridden with the `CLAW_CONFIG_HOME` environment variable.

## Settings file format

All config files are JSON objects. Here is a comprehensive example:

```json
{
  "model": "claude-sonnet-4-6",
  "aliases": {
    "fast": "haiku",
    "smart": "opus",
    "cheap": "grok-3-mini"
  },
  "permissionMode": "workspace-write",
  "maxOutputTokens": 64000,
  "enabledPlugins": {
    "sample-hooks@bundled": true,
    "my-plugin@external": false
  },
  "hooks": {
    "preToolUse": ["/path/to/pre-hook.sh"],
    "postToolUse": ["/path/to/post-hook.sh"],
    "postToolUseFailure": ["/path/to/failure-hook.sh"]
  },
  "permissionRules": {
    "allow": ["Bash(git status)", "Bash(git diff)"],
    "deny": ["Bash(rm -rf /)"],
    "ask": ["Bash(git push)"]
  },
  "plugins": {
    "externalDirectories": ["/path/to/custom/plugins"],
    "installRoot": "/custom/plugin/install/path",
    "registryPath": "/custom/registry.json",
    "bundledRoot": "/custom/bundled/plugins"
  },
  "mcpServers": {
    "my-server": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
      "transport": "stdio"
    }
  },
  "oauth": {
    "clientId": "custom-client-id",
    "authorizeUrl": "https://example.com/authorize",
    "tokenUrl": "https://example.com/token"
  },
  "sandbox": {
    "enabled": true
  }
}
```

## Top-level settings

### `model`

Default model to use when `--model` is not specified.

```json
{
  "model": "claude-sonnet-4-6"
}
```

Accepts full model IDs, aliases, or custom aliases that resolve through the alias table.

### `aliases`

Custom model aliases. Local project settings override user-level settings.

```json
{
  "aliases": {
    "fast": "claude-haiku-4-5-20251213",
    "smart": "claude-opus-4-6",
    "cheap": "grok-3-mini",
    "local": "llama3.2"
  }
}
```

Aliases resolve transitively — `"fast": "haiku"` also works since `haiku` is a built-in alias.

### `permissionMode`

Default permission mode: `"read-only"`, `"workspace-write"`, or `"danger-full-access"`.

```json
{
  "permissionMode": "workspace-write"
}
```

Overridden by `--permission-mode` CLI flag.

### `maxOutputTokens`

Maximum output tokens for model responses.

```json
{
  "maxOutputTokens": 64000
}
```

### `enabledPlugins`

Plugin enable/disable state. Keys are plugin IDs (`name@marketplace`).

```json
{
  "enabledPlugins": {
    "sample-hooks@bundled": true,
    "my-plugin@external": false
  }
}
```

### `permissionRules`

Fine-grained permission rules with allow/deny/ask lists:

```json
{
  "permissionRules": {
    "allow": ["Bash(git status)", "Bash(git log*)"],
    "deny": ["Bash(rm -rf *)"],
    "ask": ["Bash(git push)"]
  }
}
```

### `hooks`

Runtime-level hooks (separate from plugin hooks). These run alongside plugin hooks.

```json
{
  "hooks": {
    "preToolUse": ["/path/to/global-pre.sh"],
    "postToolUse": ["/path/to/global-post.sh"],
    "postToolUseFailure": ["/path/to/global-failure.sh"]
  }
}
```

### `plugins`

Plugin discovery configuration:

```json
{
  "plugins": {
    "externalDirectories": ["/path/to/custom/plugins"],
    "installRoot": "~/.claw/plugins/installed",
    "registryPath": "~/.claw/plugins/installed.json",
    "bundledRoot": "/path/to/bundled/plugins"
  }
}
```

| Field | Description |
|-------|-------------|
| `externalDirectories` | Additional directories to scan for plugins |
| `installRoot` | Where external plugins are installed |
| `registryPath` | Path to the installed plugins registry |
| `bundledRoot` | Path to bundled plugins source |

### `mcpServers`

MCP server configurations. Each server has a name and a transport config:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
      "transport": "stdio"
    },
    "remote": {
      "url": "https://mcp.example.com/sse",
      "transport": "sse"
    }
  }
}
```

Supported transports: `stdio`, `sse` (Server-Sent Events), `streamableHttp`, `sdk`.

### `oauth`

Custom OAuth configuration (overrides default Anthropic OAuth):

```json
{
  "oauth": {
    "clientId": "your-client-id",
    "authorizeUrl": "https://auth.example.com/authorize",
    "tokenUrl": "https://auth.example.com/token"
  }
}
```

### `sandbox`

Sandbox/container settings:

```json
{
  "sandbox": {
    "enabled": true
  }
}
```

### `trusted_roots`

Default trusted roots for worker trust resolution:

```json
{
  "trusted_roots": ["/path/to/repo", "/tmp/workspace"]
}
```

Workers started in these directories will auto-resolve trust prompts.

## Auth credential env vars

The correct mapping of credential shapes to env vars is critical:

| Credential | Env var | HTTP header | Notes |
|------------|---------|-------------|-------|
| `sk-ant-*` API key | `ANTHROPIC_API_KEY` | `x-api-key` | From [console.anthropic.com](https://console.anthropic.com) |
| OAuth access token | `ANTHROPIC_AUTH_TOKEN` | `Authorization: Bearer` | From `claw login` |
| OpenRouter key (`sk-or-v1-*`) | `OPENAI_API_KEY` + `OPENAI_BASE_URL` | `Authorization: Bearer` | From [openrouter.ai/keys](https://openrouter.ai/keys) |

> **Common mistake:** Putting an `sk-ant-*` key in `ANTHROPIC_AUTH_TOKEN` causes 401 errors because Anthropic rejects API keys over the Bearer header. Use `ANTHROPIC_API_KEY` instead.

## CLAUDE.md files

In addition to JSON config, `claw` reads `CLAUDE.md` files as project memory / system prompt extensions:

- `CLAUDE.md` at the project root
- `CLAUDE.md` at the user level (`~/.claude/CLAUDE.md` or similar)

These files contain Markdown-formatted instructions that are injected into the system prompt. They are not config files — they are instruction files for the model.

## Config home directory

The default config home is `~/.claw`. Override with:

```bash
export CLAW_CONFIG_HOME="/custom/config/path"
```

On Windows, if `HOME` is not set, `USERPROFILE` is used as a fallback.

## See also

- [CLI Reference](./cli-reference.md) — `--permission-mode`, `--model`, and other flags
- [Plugins](./plugins.md) — Plugin-specific config
- [API Routing](./api-routing.md) — Provider configuration
- [Architecture](./architecture.md) — ConfigLoader internals
