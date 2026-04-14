# Plugin System

Claw Code has a full plugin system that supports hooks, custom tools, and lifecycle management. Plugins can intercept tool execution, add new tools, and run initialization/shutdown scripts.

## Plugin kinds

| Kind | Description | Source |
|------|-------------|--------|
| **Builtin** | Compiled into the binary | Hardcoded in `plugins::builtin_plugins()` |
| **Bundled** | Ship with the repo | `rust/crates/plugins/bundled/` |
| **External** | User-installed | Local path or git URL via `/plugin install` |

## Plugin manifest

Every plugin has a manifest file — either `plugin.json` at the root or `.claude-plugin/plugin.json` inside the plugin directory.

### Minimal manifest

```json
{
  "name": "my-plugin",
  "version": "1.0.0",
  "description": "A minimal plugin"
}
```

### Full manifest

```json
{
  "name": "my-plugin",
  "version": "1.0.0",
  "description": "A plugin with hooks, tools, and lifecycle scripts",
  "defaultEnabled": true,
  "permissions": ["read", "write", "execute"],
  "hooks": {
    "PreToolUse": ["./hooks/pre.sh"],
    "PostToolUse": ["./hooks/post.sh"],
    "PostToolUseFailure": ["./hooks/failure.sh"]
  },
  "lifecycle": {
    "Init": ["./scripts/init.sh"],
    "Shutdown": ["./scripts/shutdown.sh"]
  },
  "tools": [
    {
      "name": "my_tool",
      "description": "Does something useful",
      "inputSchema": {
        "type": "object",
        "properties": {
          "message": { "type": "string" }
        },
        "required": ["message"],
        "additionalProperties": false
      },
      "command": "./tools/my-tool.sh",
      "requiredPermission": "workspace-write"
    }
  ],
  "commands": [
    {
      "name": "sync",
      "description": "Sync data",
      "command": "./commands/sync.sh"
    }
  ]
}
```

### Manifest fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | ✅ | Plugin name (non-empty) |
| `version` | ✅ | Semantic version (non-empty) |
| `description` | ✅ | Human-readable description (non-empty) |
| `defaultEnabled` | No | Whether the plugin is enabled by default (default: `false`) |
| `permissions` | No | Array of `"read"`, `"write"`, `"execute"` (no duplicates) |
| `hooks` | No | Hook scripts (see below) |
| `lifecycle` | No | Init/Shutdown scripts |
| `tools` | No | Custom tool definitions |
| `commands` | No | Custom command definitions |

### Claude Code contract differences

Claw Code does **not** support the full Claude Code plugin contract. The following manifest fields are rejected with guidance:

- `skills` — Claw discovers skills from local roots (`.claw/skills`, `.omc/skills`, etc.)
- `mcpServers` — MCP servers are configured separately
- `agents` — Agent catalogs are not loaded from plugin manifests
- `commands` with string entries — Only object entries with `name`/`description`/`command` are supported
- Hooks other than `PreToolUse`, `PostToolUse`, `PostToolUseFailure` — e.g. `SessionStart` is rejected

## Hooks

Hooks are shell scripts that run before or after tool execution. They receive a JSON payload via stdin and environment variables.

### Hook events

| Event | When it runs | Can deny? |
|-------|-------------|-----------|
| `PreToolUse` | Before a tool executes | ✅ (exit 2) |
| `PostToolUse` | After a tool succeeds | ❌ |
| `PostToolUseFailure` | After a tool fails | ❌ |

### Hook protocol

Each hook receives:

**Environment variables:**
- `HOOK_EVENT` — Event name (`PreToolUse`, `PostToolUse`, `PostToolUseFailure`)
- `HOOK_TOOL_NAME` — Name of the tool being called
- `HOOK_TOOL_INPUT` — Tool input as JSON string
- `HOOK_TOOL_OUTPUT` — Tool output (PostToolUse/PostToolUseFailure only)
- `HOOK_TOOL_IS_ERROR` — `"1"` if the tool result is an error, `"0"` otherwise

**Stdin:** A JSON payload with the same fields.

**Exit codes:**
- `0` — Allow (continue to next hook or proceed with execution)
- `2` — Deny (block the tool call; stdout message is shown to the user)
- Any other code — Hook failure (warning is shown; tool execution is blocked)

### Hook script example

```bash
#!/bin/sh
# PreToolUse hook that blocks writes to production config files
INPUT=$(cat)
TOOL=$(echo "$INPUT" | jq -r '.tool_name')
PATH_VAL=$(echo "$INPUT" | jq -r '.tool_input.path // empty')

if [ "$TOOL" = "WriteFile" ] && echo "$PATH_VAL" | grep -q "production"; then
  printf 'Blocked: cannot write to production config files'
  exit 2
fi

# Allow
exit 0
```

## Plugin tools

Plugins can register external commands as callable tools. The model can invoke these tools just like built-in ones.

### Tool definition

Each tool in the manifest specifies:

| Field | Description |
|-------|-------------|
| `name` | Tool name (must not conflict with built-in tools) |
| `description` | Human-readable description |
| `inputSchema` | JSON Schema for the tool input |
| `command` | Path to the executable (relative to plugin root or absolute) |
| `args` | Optional extra arguments |
| `requiredPermission` | `"read-only"`, `"workspace-write"`, or `"danger-full-access"` |

### Tool execution

When the model calls a plugin tool:

1. The tool input JSON is written to the command's stdin
2. Environment variables are set: `CLAWD_PLUGIN_ID`, `CLAWD_PLUGIN_NAME`, `CLAWD_TOOL_NAME`, `CLAWD_TOOL_INPUT`
3. The command runs in the plugin's root directory
4. `CLAWD_PLUGIN_ROOT` is set to the plugin's filesystem path
5. stdout is captured as the tool result
6. Non-zero exit codes produce an error

### Tool script example

```bash
#!/bin/sh
# Echo the tool input back with plugin metadata
INPUT=$(cat)
printf '{"plugin":"%s","tool":"%s","input":%s}\n' "$CLAWD_PLUGIN_ID" "$CLAWD_TOOL_NAME" "$INPUT"
```

## Plugin lifecycle

### Init scripts

Run when the CLI starts and the plugin is enabled:

```json
{
  "lifecycle": {
    "Init": ["./scripts/setup.sh"]
  }
}
```

### Shutdown scripts

Run when the CLI exits cleanly:

```json
{
  "lifecycle": {
    "Shutdown": ["./scripts/cleanup.sh"]
  }
}
```

Scripts run in the plugin's root directory. Non-zero exit codes produce a warning.

## Managing plugins

### List plugins

```bash
# In the REPL
/plugin list

# Or via aliases
/plugins
/marketplace
```

### Install a plugin

From a local path:
```bash
/plugin install /path/to/my-plugin
```

From a git URL:
```bash
/plugin install https://github.com/example/claw-plugin.git
```

### Enable/disable

```bash
/plugin enable my-plugin@external
/plugin disable my-plugin@external
```

### Update

```bash
/plugin update my-plugin@external
```

### Uninstall

```bash
/plugin uninstall my-plugin@external
```

Bundled plugins cannot be uninstalled — only disabled.

## Plugin discovery

Plugins are discovered from three sources:

1. **Builtin** — Hardcoded in the binary (e.g. `example-builtin`)
2. **Bundled** — Auto-synced from `rust/crates/plugins/bundled/` into `~/.claw/plugins/installed/`
3. **External** — Scanned from:
   - `~/.claw/plugins/installed/` (via `/plugin install`)
   - Additional directories configured in settings (`externalDirectories`)

Broken plugins (missing hook scripts, invalid manifests) are reported as `PluginLoadFailure` entries without preventing the CLI from starting.

## Plugin ID format

Plugin IDs follow the pattern `{name}@{marketplace}`:

- `example-builtin@builtin`
- `sample-hooks@bundled`
- `my-plugin@external`

## Settings storage

Plugin enable/disable state is persisted in `settings.json`:

```json
{
  "enabledPlugins": {
    "sample-hooks@bundled": true,
    "my-plugin@external": false
  }
}
```

The installed plugin registry is stored in `~/.claw/plugins/installed.json`.

## Bundled plugins

The repository ships with two bundled plugins:

### example-bundled

A minimal plugin scaffold demonstrating hooks.

```json
{
  "name": "example-bundled",
  "version": "0.1.0",
  "description": "Example bundled plugin scaffold for the Rust plugin system",
  "defaultEnabled": false,
  "hooks": {
    "PreToolUse": ["./hooks/pre.sh"],
    "PostToolUse": ["./hooks/post.sh"]
  }
}
```

### sample-hooks

A plugin scaffold for hook integration tests.

```json
{
  "name": "sample-hooks",
  "version": "0.1.0",
  "description": "Bundled sample plugin scaffold for hook integration tests.",
  "defaultEnabled": false,
  "hooks": {
    "PreToolUse": ["./hooks/pre.sh"],
    "PostToolUse": ["./hooks/post.sh"]
  }
}
```

## Creating a plugin

1. Create a directory for your plugin:

   ```bash
   mkdir -p my-plugin/.claude-plugin
   ```

2. Write the manifest:

   ```bash
   cat > my-plugin/.claude-plugin/plugin.json << 'EOF'
   {
     "name": "my-plugin",
     "version": "1.0.0",
     "description": "My custom plugin",
     "defaultEnabled": true,
     "hooks": {
       "PreToolUse": ["./hooks/pre.sh"]
     }
   }
   EOF
   ```

3. Write hook scripts:

   ```bash
   mkdir -p my-plugin/hooks
   cat > my-plugin/hooks/pre.sh << 'EOF'
   #!/bin/sh
   printf 'My plugin says hello before every tool call'
   exit 0
   EOF
   chmod +x my-plugin/hooks/pre.sh
   ```

4. Install:

   ```bash
   /plugin install /path/to/my-plugin
   ```

## See also

- [CLI Reference](./cli-reference.md) — `/plugin` command details
- [Configuration](./configuration.md) — Settings file format
- [Architecture](./architecture.md) — Plugin system internals
