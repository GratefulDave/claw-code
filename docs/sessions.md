# Session Management

Claw Code persists conversation state to disk so you can resume sessions across invocations.

## Session storage

Sessions are stored as JSONL (JSON Lines) files under `.claw/sessions/` in the current workspace:

```
.claw/sessions/
├── session-1775504246746-0.jsonl
├── session-1775504448974-0.jsonl
└── session-1775504608138-0.jsonl
```

Each line in a session file is a JSON record representing a conversation event:

- **`session_meta`** — Session metadata (ID, model, workspace root, creation time)
- **`user`** — User message
- **`assistant`** — Assistant response (text, tool calls)
- **`tool_result`** — Tool execution result

## Session metadata

Sessions track:

| Field | Description |
|-------|-------------|
| `session_id` | Unique session identifier |
| `model` | Model used for the session |
| `workspace_root` | Canonical git worktree path where the session was created |
| `created_at` | Session creation timestamp |

The `workspace_root` field enables workspace mismatch detection — sessions refuse to execute tools from a different CWD than where they were created.

## Starting a session

### Interactive (new session)

```bash
claw
```

This creates a new session and opens the REPL. Each turn is appended to the session file.

### One-shot (new session)

```bash
claw prompt "explain the runtime crate"
```

Creates a new session, runs the prompt, prints the response, and exits.

## Resuming sessions

### Resume the most recent session

```bash
claw --resume latest
```

Opens the REPL with the full conversation history restored.

### Resume a specific session

```bash
claw --resume path/to/session.jsonl
```

### Resume with a slash command

```bash
# Check status
claw --resume latest /status

# View the diff
claw --resume latest /diff

# Get JSON status
claw --output-format json --resume latest /status
```

### Resume and continue

```bash
claw --resume latest "now add tests for that function"
```

## Session lifecycle

```
1. New session created
   → .claw/sessions/session-{timestamp}-{nonce}.jsonl
   → session_meta record written

2. Conversation turns
   → user/assistant/tool_result records appended
   → Session state persisted after each turn

3. Session ends (exit, completion, or interrupt)
   → No explicit close record needed
   → Session file remains on disk

4. Resume
   → Session file read
   → Conversation history replayed into runtime
   → New turns appended to the same file
```

## Session fork

When a session is resumed, it can optionally fork — creating a new session file with the history from the original session. The fork inherits the `workspace_root` from the parent.

## Workspace root tracking

Sessions record the canonical workspace root (resolved git worktree path) at creation time. When resumed:

- If the current CWD matches the session's workspace root, the session proceeds normally
- If the CWD differs, the session still loads but workspace-aware tools validate against the session's recorded root

This prevents cross-worktree "phantom completions" where parallel lanes write to the wrong directory.

## Slash commands for sessions

| Command | Description |
|---------|-------------|
| `/session` | Show current session info |
| `/session list` | List all sessions in the workspace |
| `/status` | Show session status, model, message count |
| `/compact` | Compact conversation history to reduce context usage |
| `/clear` | Clear conversation history (keeps session alive) |
| `/export` | Export the conversation to a file |

## JSON output

Session info is available as structured JSON:

```bash
# Session status
claw --output-format json --resume latest /status

# Session list
claw --output-format json --resume latest /session list

# Resume info (no slash command)
claw --output-format json --resume latest
```

Example `/status` output:

```json
{
  "kind": "status",
  "session_id": "session-1775504246746-0",
  "model": "claude-opus-4-6",
  "message_count": 12,
  "permission_mode": "danger-full-access",
  "cwd": "/path/to/workspace"
}
```

## Session directory structure

```
.claw/
├── sessions/
│   ├── session-1775504246746-0.jsonl
│   ├── session-1775504448974-0.jsonl
│   └── ...
├── settings.json           # Project config
├── settings.local.json     # Local overrides (gitignored)
└── worker-state.json       # Worker boot state
```

## Tips

- **Use `--resume latest`** to continue your most recent session without remembering the file path
- **Use `/compact`** when context usage gets high — it summarizes earlier conversation to free space
- **Use `/export`** to save a copy of the conversation for review
- **Session files are append-only** — they grow over time but never shrink (unless compacted)
- **Session files are safe to delete** — removing a `.jsonl` file simply removes that session from history

## See also

- [CLI Reference](./cli-reference.md) — `--resume` flag details
- [Architecture](./architecture.md) — Session persistence internals
- [Configuration](./configuration.md) — `.claw/` directory structure
