# Architecture Overview

This document describes the internal architecture of Claw Code — the Rust implementation of the `claw` CLI agent harness.

## High-level architecture

```
┌──────────────────────────────────────────────────────────┐
│                     User (terminal / CI)                  │
└────────────────────┬─────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────┐
│             rusty-claude-cli (CLI binary `claw`)          │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ Arg parsing  │  │ REPL loop    │  │ One-shot prompt│  │
│  │ (main.rs)    │  │ (rustyline)  │  │ dispatch       │  │
│  └──────┬───────┘  └──────┬───────┘  └───────┬────────┘  │
│         │                 │                   │           │
│         └────────┬────────┴───────────────────┘           │
│                  ▼                                        │
│  ┌───────────────────────────────────────────────────┐    │
│  │              runtime (ConversationRuntime)         │    │
│  │  ┌──────────┐ ┌────────┐ ┌───────┐ ┌───────────┐  │    │
│  │  │ Session  │ │ Config │ │ Perms │ │ MCP client│  │    │
│  │  │ persist  │ │ loader │ │ policy│ │ lifecycle │  │    │
│  │  └──────────┘ └────────┘ └───────┘ └───────────┘  │    │
│  └────────────────────┬──────────────────────────────┘    │
│                       │                                    │
│         ┌─────────────┼──────────────┐                    │
│         ▼             ▼              ▼                    │
│  ┌────────────┐ ┌──────────┐ ┌──────────────┐            │
│  │  api       │ │  tools   │ │  plugins     │            │
│  │ (providers)│ │ (exec)   │ │ (hooks/tools)│            │
│  └────────────┘ └──────────┘ └──────────────┘            │
└──────────────────────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────┐
│  Anthropic / xAI /       │
│  OpenAI-compat / Ollama  │
│  DashScope (Qwen)        │
└──────────────────────────┘
```

## Workspace layout

```
claw-code/
├── rust/                          # Rust workspace root
│   ├── Cargo.toml                 # Workspace manifest (resolver = "2")
│   └── crates/
│       ├── api/                   # Provider clients + streaming
│       ├── commands/              # Slash-command registry + help rendering
│       ├── compat-harness/        # TS manifest extraction harness
│       ├── mock-anthropic-service/# Deterministic local Anthropic mock
│       ├── plugins/               # Plugin metadata, install, hooks
│       ├── runtime/               # Session, config, permissions, MCP, auth
│       ├── rusty-claude-cli/      # CLI binary (`claw`)
│       ├── telemetry/             # Session tracing & usage telemetry types
│       └── tools/                 # Built-in tools, skill resolution, agents
├── docs/                          # Documentation (this directory)
├── USAGE.md                       # Task-oriented usage guide
├── PARITY.md                      # Rust-port parity status
├── ROADMAP.md                     # Active roadmap and backlog
├── PHILOSOPHY.md                  # Project intent and design framing
└── README.md                      # Repository overview and quick start
```

## Crate responsibilities

### `rusty-claude-cli` — CLI binary

The main entry point. Contains:

- **CLI argument parsing** — `parse_args()` handles flags, subcommands, model aliases, permission modes, and session resume paths.
- **Interactive REPL** — Powered by `rustyline`. Provides tab completion for slash commands, model aliases, permission modes, and session IDs.
- **One-shot prompt** — `claw prompt "..."` or `claw "..."` for non-interactive execution.
- **Slash command dispatch** — Both in the REPL and in resume mode (`--resume <session> /command`).
- **Streaming display** — Renders SSE streaming responses, tool call results, and markdown in ANSI terminal output.
- **Session resume** — Restores conversation state from `.claw/sessions/` JSONL files.

### `runtime` — Core engine

The heart of the system. Manages:

- **`ConversationRuntime`** — The main runtime struct, generic over `ApiClient` and `ToolExecutor`. Drives the conversation loop: send messages → receive streaming response → execute tool calls → feed results back.
- **Session persistence** — JSONL-based session storage under `.claw/sessions/`. Supports fork, resume, and workspace root tracking.
- **Config loading** — `ConfigLoader` discovers and merges config from `~/.claw.json`, `~/.config/claw/settings.json`, `<repo>/.claw.json`, `<repo>/.claw/settings.json`, and `<repo>/.claw/settings.local.json` (in that order).
- **Permission enforcement** — `PermissionEnforcer` gates tool execution based on the active permission mode (read-only, workspace-write, danger-full-access).
- **MCP client lifecycle** — Manages MCP server processes (stdio, HTTP, WebSocket), tool/resource discovery, and degraded-startup reporting.
- **Worker boot protocol** — `WorkerRegistry` with explicit state machine: `Spawning → TrustRequired → ReadyForPrompt → Running → Finished/Failed`.
- **System prompt assembly** — Builds the system prompt from CLAUDE.md files, project config, and runtime context.
- **Auth** — API key (`ANTHROPIC_API_KEY`), bearer token (`ANTHROPIC_AUTH_TOKEN`), and OAuth (`claw login`) paths.

### `api` — Provider clients

Handles all communication with LLM providers:

- **Anthropic client** — Native Messages API with SSE streaming, `x-api-key` and Bearer auth, thinking/budget tokens.
- **OpenAI-compatible client** — Chat Completions endpoint for OpenAI, Ollama, OpenRouter, DashScope, and any `/v1/chat/completions` service.
- **xAI client** — OpenAI-compatible endpoint at `api.x.ai/v1`.
- **Provider routing** — `detect_provider_kind()` selects the provider based on model-name prefix (`claude`, `grok`, `openai/`, `gpt-`, `qwen/`) falling back to env-var presence.
- **Request preflight** — Context-window and token-count validation before requests leave the process.
- **Streaming** — SSE event parsing, content block assembly, tool call extraction.

### `tools` — Built-in tool execution

Implements the 40+ built-in tools exposed to the model:

- **File operations**: `Bash`, `ReadFile`, `WriteFile`, `EditFile`, `GlobSearch`, `GrepSearch`
- **Web**: `WebSearch`, `WebFetch`
- **Agent/coordination**: `Agent`, `Skill`, `ToolSearch`
- **Task management**: `TaskCreate`, `TaskGet`, `TaskList`, `TaskStop`, `TaskUpdate`, `TaskOutput`
- **Teams/scheduling**: `TeamCreate`, `TeamDelete`, `CronCreate`, `CronDelete`, `CronList`
- **MCP bridge**: `ListMcpResources`, `ReadMcpResource`, `MCP`, `McpAuth`
- **Code intelligence**: `LSP` (symbols, references, diagnostics, definition, hover)
- **Productivity**: `TodoWrite`, `NotebookEdit`, `Sleep`, `SendUserQuestion`
- **Workflow**: `EnterPlanMode`, `ExitPlanMode`, `Config`, `StructuredOutput`, `REPL`, `PowerShell`
- **Worker management**: `WorkerCreate`, `WorkerGet`, `WorkerObserve`, `WorkerResolveTrust`, `WorkerAwaitReady`, `WorkerSendPrompt`, `WorkerRestart`, `WorkerTerminate`, `WorkerObserveCompletion`
- **Background tasks**: `RunTaskPacket`
- **Plugin tools**: Dynamically registered from installed plugins.

Each tool has a `required_permission` level that is enforced before execution.

### `plugins` — Plugin system

Full plugin lifecycle management:

- **Plugin kinds**: `Builtin`, `Bundled`, `External`
- **Discovery**: Scans bundled plugins from `crates/plugins/bundled/`, installed plugins from `~/.claude/plugins/installed/`, and external directories.
- **Lifecycle**: Init/shutdown scripts, validation of hook paths and tool commands.
- **Hooks**: `PreToolUse`, `PostToolUse`, `PostToolUseFailure` — shell scripts that receive JSON payloads via stdin and can approve (exit 0), deny (exit 2), or fail (exit 1) tool execution.
- **Plugin tools**: External commands registered as callable tools with their own JSON schemas.
- **Install/enable/disable/uninstall/update**: Full CRUD for plugin packages from local paths or git URLs.

### `commands` — Slash commands

Shared slash-command definitions, parsing, and rendering for both text and JSON output.

### `telemetry` — Usage tracking

Session trace events and telemetry payloads for cost/usage reporting.

### `mock-anthropic-service` — Testing

A deterministic Anthropic-compatible mock server used for end-to-end parity testing. Provides scripted responses for specific scenarios (streaming text, file tools, bash, permissions, plugins).

### `compat-harness` — Compatibility

Extracts tool/prompt manifests from upstream TypeScript source for parity comparison.

## Data flow

### One-shot prompt

```
User → CLI args → parse_args()
  → build_runtime()
    → ConfigLoader.discover()
    → PluginManager.plugin_registry()
    → ConversationRuntime::new()
  → runtime.send_message()
    → ApiClient.send_streaming()
      → Provider (Anthropic/xAI/OpenAI-compat)
    ← SSE events
  → Tool calls extracted
    → PermissionEnforcer.check()
    → HookRunner.run_pre_tool_use()
    → ToolExecutor.execute_tool()
    → HookRunner.run_post_tool_use()
    → Results fed back to runtime
  → Final response rendered
```

### Interactive REPL

```
User → rustyline prompt
  → validate_slash_command_input() or prompt dispatch
  → same flow as one-shot, but in a loop
  → session persisted after each turn
```

### Session resume

```
User → --resume <session>
  → Load JSONL from .claw/sessions/
  → Parse conversation history
  → If slash command: run_resume_command()
  → Else: enter REPL with restored state
```

## Permission model

| Mode               | Read tools | Write tools | Bash        |
|--------------------|------------|-------------|-------------|
| `read-only`        | ✅         | ❌          | Read-only   |
| `workspace-write`  | ✅         | ✅ (in CWD) | ✅          |
| `danger-full-access` | ✅       | ✅          | ✅          |

Permission enforcement happens in `PermissionEnforcer` before tool execution. Hook scripts can additionally deny tool calls regardless of the permission mode.

## Key design decisions

1. **State machine first** — Workers have explicit lifecycle states (`Spawning → TrustRequired → ReadyForPrompt → Running → Finished/Failed`).
2. **Events over scraped prose** — Lane events are typed (`LaneEvent` enum), not inferred from terminal output.
3. **Provider routing by model prefix** — Model names like `openai/gpt-4` or `grok-3` determine the provider, not just env-var presence.
4. **Plugin isolation** — Broken plugins don't crash the CLI; they're reported as `PluginLoadFailure` in a `PluginRegistryReport`.
5. **JSON output contract** — Every CLI surface supports `--output-format json` for machine consumption.
6. **Session persistence** — JSONL-based append-only logs support resume, fork, and workspace-root tracking.
