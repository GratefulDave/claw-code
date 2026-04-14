# CLI Reference

Complete reference for the `claw` command-line interface.

## Synopsis

```text
claw [OPTIONS] [COMMAND]
claw [OPTIONS] [PROMPT_TEXT]
```

## Global flags

| Flag | Description |
|------|-------------|
| `--model MODEL` | Model name or alias (e.g. `opus`, `sonnet`, `grok-3`, `openai/gpt-4`) |
| `--output-format FORMAT` | Output format: `text` (default) or `json` |
| `--permission-mode MODE` | Permission mode: `read-only`, `workspace-write`, or `danger-full-access` |
| `--dangerously-skip-permissions` | Skip all permission checks (equivalent to `danger-full-access`) |
| `--allowedTools TOOLS` | Comma-separated list of allowed tool names (e.g. `read,glob,grep`) |
| `--resume [TARGET]` | Resume a session. Target can be a `.jsonl` path, session ID, or `latest` |
| `--version`, `-V` | Print version and exit |
| `--help`, `-h` | Print help and exit |

## Model aliases

Short names that resolve to the latest model versions:

| Alias | Resolves to | Provider |
|-------|-------------|----------|
| `opus` | `claude-opus-4-6` | Anthropic |
| `sonnet` | `claude-sonnet-4-6` | Anthropic |
| `haiku` | `claude-haiku-4-5-20251213` | Anthropic |
| `grok` / `grok-3` | `grok-3` | xAI |
| `grok-mini` / `grok-3-mini` | `grok-3-mini` | xAI |
| `grok-2` | `grok-2` | xAI |

Any model name not matching an alias is passed through verbatim. This is how you use OpenRouter slugs, Ollama tags, or full model IDs.

## Top-level commands

| Command | Description |
|---------|-------------|
| `prompt <text>` | One-shot prompt (non-interactive) |
| `help` | Print help |
| `version` | Print version |
| `status` | Show current session status, model, and permissions |
| `sandbox` | Show sandbox/container detection status |
| `doctor` | Run built-in health and preflight diagnostics |
| `agents` | List available agents |
| `mcp` | List MCP servers and their status |
| `skills` | List available skills |
| `init` | Initialize `.claw/` directory in the current workspace |
| `login` | Authenticate via OAuth |
| `logout` | Clear stored OAuth credentials |
| `system-prompt` | Dump the assembled system prompt (for debugging) |
| `dump-manifests` | Extract tool/prompt manifests |
| `bootstrap-plan` | Generate a bootstrap plan |

All commands support `--output-format json` for structured machine-readable output.

## Shorthand prompt

If no subcommand is given and the arguments look like a prompt, `claw` runs it as a one-shot:

```bash
claw "explain this codebase"
claw --model sonnet "review the diff"
```

If stdin is a pipe (not a terminal), the piped content is read and used as the prompt.

## Permission modes

| Mode | Description |
|------|-------------|
| `read-only` | Only read tools allowed. No file writes, no mutating bash commands. |
| `workspace-write` | Read + write tools allowed within the current working directory. |
| `danger-full-access` | All tools allowed, unrestricted. **Default.** |

## Slash commands (REPL)

Inside the interactive REPL, use `/` to invoke commands. Tab completion is available.

### Session and visibility

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/status` | Show session status, model, cost |
| `/cost` | Show cumulative cost for the session |
| `/session` | Session management (list, etc.) |
| `/version` | Show version |
| `/usage` / `/stats` / `/tokens` / `/cache` | Usage and token statistics |
| `/compact` | Compact conversation history |
| `/clear` | Clear conversation history |
| `/export` | Export conversation to file |

### Workspace and git

| Command | Description |
|---------|-------------|
| `/config` | Show/edit runtime config |
| `/memory` | Show/edit CLAUDE.md project memory |
| `/init` | Initialize `.claw/` directory |
| `/diff` | Show git diff |
| `/commit` | Commit changes |
| `/pr` | Create a pull request |
| `/issue` | Create a GitHub issue |
| `/files` | List relevant files |
| `/branch` | Branch operations |
| `/release-notes` | Generate release notes |
| `/add-dir` | Add an additional directory to the workspace scope |

### Discovery and debugging

| Command | Description |
|---------|-------------|
| `/mcp [list\|show <server>\|help]` | MCP server management |
| `/agents [list\|help]` | Agent discovery |
| `/skills [list\|install <path>\|help]` | Skill management |
| `/doctor` | Health check diagnostics |
| `/tasks` | List background tasks |
| `/context` | Show context window usage |
| `/hooks` | Show registered hooks |
| `/providers` | Show available providers |

### Automation and analysis

| Command | Description |
|---------|-------------|
| `/review` | Code review |
| `/advisor` | Advisory analysis |
| `/insights` | Generate insights |
| `/security-review` | Security audit |
| `/subagent [list\|steer <target> <msg>\|kill <id>]` | Sub-agent management |
| `/team` | Team operations |
| `/telemetry` | Telemetry controls |
| `/cron` | Scheduled task management |

### Plugin management

| Command | Description |
|---------|-------------|
| `/plugin [list\|install <path>\|enable <name>\|disable <name>\|uninstall <id>\|update <id>]` | Plugin lifecycle |
| `/plugins` | Alias for `/plugin list` |
| `/marketplace` | Alias for `/plugin list` |

## Session resume

Resume a previous session from its saved state:

```bash
# Resume the most recent session
claw --resume latest

# Resume a specific session
claw --resume path/to/session.jsonl

# Resume and run a slash command
claw --resume latest /status
claw --resume latest /diff

# Resume with JSON output
claw --output-format json --resume latest /status
```

## JSON output

All CLI surfaces support structured JSON output:

```bash
claw --output-format json status
claw --output-format json sandbox
claw --output-format json mcp
claw --output-format json skills
claw --output-format json version
claw --output-format json doctor
claw --output-format json --resume latest /status
```

Error output is also JSON when `--output-format json` is set:

```json
{"type":"error","error":"missing Anthropic credentials"}
```

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | General error (API failure, config error, etc.) |
| `2` | Slash command error (in resume mode) |

## Environment variables

### Authentication

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Anthropic API key (`sk-ant-*`). Sent as `x-api-key` header. |
| `ANTHROPIC_AUTH_TOKEN` | OAuth/Bearer token. Sent as `Authorization: Bearer` header. |
| `OPENAI_API_KEY` | OpenAI/OpenRouter API key |
| `XAI_API_KEY` | xAI API key |
| `DASHSCOPE_API_KEY` | Alibaba DashScope API key |

### Provider endpoints

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_BASE_URL` | Override Anthropic API base URL |
| `OPENAI_BASE_URL` | OpenAI-compatible base URL (Ollama, OpenRouter, etc.) |
| `XAI_BASE_URL` | Override xAI API base URL |
| `DASHSCOPE_BASE_URL` | Override DashScope base URL |

### Proxy

| Variable | Description |
|----------|-------------|
| `HTTP_PROXY` / `http_proxy` | HTTP proxy |
| `HTTPS_PROXY` / `https_proxy` | HTTPS proxy |
| `NO_PROXY` / `no_proxy` | Proxy bypass list |

### Other

| Variable | Description |
|----------|-------------|
| `CLAW_CONFIG_HOME` | Override config home directory (default: `~/.claw`) |
| `CODEX_HOME` | Custom root for user-level skill and command lookups |

## See also

- [Getting Started](./getting-started.md)
- [Configuration](./configuration.md)
- [API Routing](./api-routing.md)
