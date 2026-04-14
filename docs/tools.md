# Built-in Tools Reference

Claw Code exposes 40+ built-in tools that the model can invoke. Each tool has a permission level that gates execution.

## File operations

### Bash

Execute shell commands.

- **Permission:** `danger-full-access`
- **Input:** `{ "command": "string", "timeout_ms": number?, "run_in_background": bool? }`
- **Features:** Timeout support, background execution, sandbox detection, read-only mode enforcement

### ReadFile

Read a file from the workspace.

- **Permission:** `read-only`
- **Input:** `{ "path": "string", "limit": number?, "offset": number? }`
- **Features:** Binary detection (NUL bytes), size limits, workspace boundary enforcement, symlink escape prevention

### WriteFile

Create or overwrite a file in the workspace.

- **Permission:** `workspace-write`
- **Input:** `{ "path": "string", "content": "string" }`
- **Features:** Workspace boundary validation, size limits

### EditFile

Replace text in a workspace file.

- **Permission:** `workspace-write`
- **Input:** `{ "path": "string", "old_string": "string", "new_string": "string", "replace_all": bool? }`
- **Features:** Supports single and replace-all modes

### GlobSearch

Find files by glob pattern.

- **Permission:** `read-only`
- **Input:** `{ "pattern": "string", "path": "string? }`
- **Features:** Brace expansion (`*.{rs,toml}`), nested brace support, deduplication

### GrepSearch

Search file contents with a regex pattern.

- **Permission:** `read-only`
- **Input:** `{ "pattern": "string", "path": "string?", "glob": "string?", "-i": bool?, "-n": bool?, ... }`
- **Features:** Regex patterns, case-insensitive mode, line numbers, file filtering, context lines

## Web tools

### WebSearch

Search the web for current information.

- **Permission:** `danger-full-access`
- **Input:** `{ "query": "string", "allowed_domains": string[]?, "blocked_domains": string[]? }`

### WebFetch

Fetch a URL and convert it to readable text.

- **Permission:** `danger-full-access`
- **Input:** `{ "url": "string", "prompt": "string" }`

## Agent and coordination

### Agent

Launch a specialized sub-agent task.

- **Permission:** `danger-full-access`
- **Input:** `{ "prompt": "string", "description": "string?", "model": "string?", "subagent_type": "string?" }`

### Skill

Load and execute a local skill definition.

- **Permission:** `danger-full-access`
- **Input:** `{ "skill": "string", "args": "string?" }`

### ToolSearch

Search for available tools by name or keywords.

- **Permission:** `read-only`
- **Input:** `{ "query": "string", "max_results": number? }`

## Task management

### TaskCreate

Create a background task that runs in a separate subprocess.

- **Permission:** `danger-full-access`
- **Input:** `{ "prompt": "string", "description": "string?" }`

### TaskGet

Get the status and details of a background task.

- **Permission:** `read-only`
- **Input:** `{ "task_id": "string" }`

### TaskList

List all background tasks and their current status.

- **Permission:** `read-only`
- **Input:** `{}`

### TaskStop

Stop a running background task by ID.

- **Permission:** `danger-full-access`
- **Input:** `{ "task_id": "string" }`

### TaskUpdate

Send a message or update to a running background task.

- **Permission:** `danger-full-access`
- **Input:** `{ "task_id": "string", "message": "string" }`

### TaskOutput

Retrieve the output produced by a background task.

- **Permission:** `read-only`
- **Input:** `{ "task_id": "string" }`

## Teams and scheduling

### TeamCreate

Create a team of sub-agents for parallel task execution.

- **Permission:** `danger-full-access`
- **Input:** `{ "name": "string", "tasks": object[] }`

### TeamDelete

Delete a team and stop all its running tasks.

- **Permission:** `danger-full-access`
- **Input:** `{ "team_id": "string" }`

### CronCreate

Create a scheduled recurring task.

- **Permission:** `danger-full-access`
- **Input:** `{ "schedule": "string", "prompt": "string", "description": "string?" }`

### CronDelete

Delete a scheduled recurring task by ID.

- **Permission:** `danger-full-access`
- **Input:** `{ "cron_id": "string" }`

### CronList

List all scheduled recurring tasks.

- **Permission:** `read-only`
- **Input:** `{}`

## MCP (Model Context Protocol)

### ListMcpResources

List available resources from connected MCP servers.

- **Permission:** `read-only`
- **Input:** `{ "server": "string?" }`

### ReadMcpResource

Read a specific resource from an MCP server by URI.

- **Permission:** `read-only`
- **Input:** `{ "server": "string", "uri": "string" }`

### MCP

Execute a tool provided by a connected MCP server.

- **Permission:** `danger-full-access`
- **Input:** `{ "server": "string", "tool": "string", "arguments": object }`

### McpAuth

Authenticate with an MCP server that requires OAuth or credentials.

- **Permission:** `danger-full-access`
- **Input:** `{ "server": "string" }`

## Code intelligence

### LSP

Query Language Server Protocol for code intelligence.

- **Permission:** `read-only`
- **Input:** `{ "action": "symbols|references|diagnostics|definition|hover", "path": "string?", "line": number?, "character": number?, "query": "string?" }`
- **Actions:** `symbols`, `references`, `diagnostics`, `definition`, `hover`

## Productivity

### TodoWrite

Update the structured task list for the current session.

- **Permission:** `read-only`
- **Input:** `{ "todos": object[] }`

### NotebookEdit

Replace, insert, or delete a cell in a Jupyter notebook.

- **Permission:** `workspace-write`
- **Input:** `{ "notebook_path": "string", "cell_id": "string?", "cell_type": "code|markdown", "edit_mode": "replace|insert|delete", "new_source": "string?" }`

### Sleep

Wait for a specified duration without holding a shell process.

- **Permission:** `read-only`
- **Input:** `{ "duration_ms": number }`

### SendUserMessage

Send a message to the user.

- **Permission:** `read-only`
- **Input:** `{ "message": "string", "status": "normal|proactive", "attachments": string[]? }`

### AskUserQuestion

Ask the user a question and wait for their response.

- **Permission:** `read-only`
- **Input:** `{ "question": "string", "options": string[]? }`

## Workflow

### Config

Get or set claw settings.

- **Permission:** `read-only`
- **Input:** `{ "setting": "string", "value": "string|bool|number?" }`

### EnterPlanMode

Enable a worktree-local planning mode override.

- **Permission:** `read-only`
- **Input:** `{}`

### ExitPlanMode

Restore or clear the worktree-local planning mode override.

- **Permission:** `read-only`
- **Input:** `{}`

### StructuredOutput

Return structured output in the requested format.

- **Permission:** `read-only`
- **Input:** arbitrary properties

### REPL

Execute code in a REPL-like subprocess.

- **Permission:** `danger-full-access`
- **Input:** `{ "code": "string", "language": "string", "timeout_ms": number? }`

### PowerShell

Execute a PowerShell command with optional timeout.

- **Permission:** `danger-full-access`
- **Input:** `{ "command": "string", "description": "string?", "run_in_background": bool?, "timeout": number? }`

## Worker management

### WorkerCreate

Create a coding worker boot session with trust-gate and prompt-delivery guards.

- **Permission:** `danger-full-access`
- **Input:** `{ "cwd": "string", "trusted_roots": string[]?, "auto_recover_prompt_misdelivery": bool? }`

### WorkerGet

Fetch the current worker boot state, last error, and event history.

- **Permission:** `read-only`
- **Input:** `{ "worker_id": "string" }`

### WorkerObserve

Feed a terminal snapshot into worker boot detection.

- **Permission:** `danger-full-access`
- **Input:** `{ "worker_id": "string", "screen_text": "string" }`

### WorkerResolveTrust

Resolve a detected trust prompt so worker boot can continue.

- **Permission:** `danger-full-access`
- **Input:** `{ "worker_id": "string" }`

### WorkerAwaitReady

Return the current ready-handshake verdict for a coding worker.

- **Permission:** `read-only`
- **Input:** `{ "worker_id": "string" }`

### WorkerSendPrompt

Send a task prompt only after the worker reaches `ready_for_prompt`.

- **Permission:** `danger-full-access`
- **Input:** `{ "worker_id": "string", "prompt": "string" }`

### WorkerRestart

Restart worker boot state after a failed or stale startup.

- **Permission:** `danger-full-access`
- **Input:** `{ "worker_id": "string" }`

### WorkerTerminate

Terminate a worker and mark the lane finished.

- **Permission:** `danger-full-access`
- **Input:** `{ "worker_id": "string" }`

### WorkerObserveCompletion

Report session completion to the worker.

- **Permission:** `danger-full-access`
- **Input:** `{ "worker_id": "string", "finish_reason": "string", "tokens_output": number }`

## Background tasks

### RunTaskPacket

Create a background task from a structured task packet.

- **Permission:** `danger-full-access`
- **Input:** `{ "objective": "string", "scope": "string", "repo": "string", "branch_policy": "string", "acceptance_tests": string[], "commit_policy": "string", "reporting_contract": "string", "escalation_policy": "string" }`

## Remote triggers

### RemoteTrigger

Trigger a remote action or webhook endpoint.

- **Permission:** `danger-full-access`
- **Input:** `{ "url": "string", "method": "GET|POST|PUT|DELETE", "headers": object?, "body": "string?" }`

## See also

- [Architecture](./architecture.md) — Tool execution pipeline
- [Plugins](./plugins.md) — Plugin-defined tools
- [CLI Reference](./cli-reference.md) — `--allowedTools` flag
