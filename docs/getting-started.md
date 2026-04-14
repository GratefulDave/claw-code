# Getting Started

This guide walks you through building, configuring, and running Claw Code for the first time.

## Prerequisites

- **Rust toolchain** — Install via [rustup](https://rustup.rs/). Verify with `cargo --version`.
- **API key** — One of:
  - `ANTHROPIC_API_KEY` for direct Anthropic API access
  - `OPENAI_API_KEY` for OpenAI, OpenRouter, or Ollama
  - `XAI_API_KEY` for xAI (Grok models)
  - `DASHSCOPE_API_KEY` for Alibaba DashScope (Qwen models)
- **Git** — Required for cloning and for `claw`'s git integration features.

> **Important:** Claw requires an **API key** — not a Claude subscription login. Get your key from [console.anthropic.com](https://console.anthropic.com).

## Build from source

> **Warning:** Do NOT use `cargo install claw-code`. The `claw-code` crate on crates.io is a deprecated stub. This repo is **build-from-source only**.

```bash
# 1. Clone
git clone https://github.com/ultraworkers/claw-code
cd claw-code/rust

# 2. Build
cargo build --workspace

# 3. Verify
./target/debug/claw --version
```

The binary is at `rust/target/debug/claw` after a debug build. For a release build, use `cargo build --release --workspace` (binary at `target/release/claw`).

## Set your API key

```bash
# Anthropic (default)
export ANTHROPIC_API_KEY="sk-ant-..."

# Or for other providers, see docs/api-routing.md
```

## First run: doctor check

Always run the built-in health check first:

```bash
cd rust
./target/debug/claw
```

Then inside the REPL:

```
/doctor
```

`/doctor` checks API connectivity, config files, permissions, and plugin health. Fix any issues it reports before proceeding.

## Quick start examples

### Interactive REPL

```bash
./target/debug/claw
```

This opens an interactive prompt where you can chat with the model, use slash commands, and let it execute tools.

### One-shot prompt

```bash
./target/debug/claw prompt "summarize this repository"
```

### Shorthand prompt (no subcommand)

```bash
./target/debug/claw "explain the runtime crate"
```

### JSON output for scripting

```bash
./target/debug/claw --output-format json prompt "status"
```

### Choose a model

```bash
./target/debug/claw --model sonnet prompt "review this diff"
./target/debug/claw --model opus "write a test for the plugin system"
```

### Restrict permissions

```bash
# Read-only: no file writes or mutations
./target/debug/claw --permission-mode read-only "list all TODO comments"

# Workspace-write: allow writes inside CWD
./target/debug/claw --permission-mode workspace-write "fix the typo in README.md"
```

## Windows setup

1. **Install Rust** from <https://rustup.rs/>. Close and reopen your terminal.
2. **Verify:** `cargo --version`
3. **Build:**

   ```powershell
   git clone https://github.com/ultraworkers/claw-code
   cd claw-code\rust
   cargo build --workspace
   ```

4. **Run:**

   ```powershell
   $env:ANTHROPIC_API_KEY = "sk-ant-..."
   .\target\debug\claw.exe prompt "say hello"
   ```

> **Note:** On Windows, the binary is `claw.exe`. PowerShell is a supported path. Git Bash and WSL are optional alternatives.

## Using alternative providers

See [api-routing.md](./api-routing.md) for the full provider matrix. Quick examples:

### Ollama (local models)

```bash
export OPENAI_BASE_URL="http://127.0.0.1:11434/v1"
unset OPENAI_API_KEY

./target/debug/claw --model "llama3.2" prompt "hello"
```

### OpenRouter

```bash
export OPENAI_BASE_URL="https://openrouter.ai/api/v1"
export OPENAI_API_KEY="sk-or-v1-..."

./target/debug/claw --model "openai/gpt-4.1-mini" prompt "summarize this repo"
```

### xAI (Grok)

```bash
export XAI_API_KEY="..."

./target/debug/claw --model "grok-3" prompt "explain Rust ownership"
```

## Verify everything works

```bash
cd rust

# Run the full test suite
cargo test --workspace

# Check formatting
cargo fmt --all --check

# Lint
cargo clippy --workspace --all-targets -- -D warnings
```

## Next steps

- [CLI Reference](./cli-reference.md) — all flags, commands, and slash commands
- [Configuration](./configuration.md) — config file hierarchy and settings
- [Plugins](./plugins.md) — install, create, and manage plugins
- [Sessions](./sessions.md) — session persistence and resume
- [Tools](./tools.md) — built-in tools reference
- [Container workflow](./container.md) — Docker/Podman usage
