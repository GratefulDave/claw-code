# Claw Code Documentation

Welcome to the Claw Code documentation. Claw Code is the public Rust implementation of the `claw` CLI agent harness — a high-performance coding assistant that supports multiple LLM providers, plugin extensions, and autonomous workflows.

## Quick links

| Document | Description |
|----------|-------------|
| [Getting Started](./getting-started.md) | Build, configure, and run `claw` for the first time |
| [CLI Reference](./cli-reference.md) | All flags, commands, slash commands, and env vars |
| [Configuration](./configuration.md) | Config file hierarchy, settings format, and auth |
| [API Routing](./api-routing.md) | Provider matrix, model aliases, and routing rules |
| [Plugin System](./plugins.md) | Hooks, custom tools, and plugin lifecycle |
| [Built-in Tools](./tools.md) | Complete reference for all 40+ built-in tools |
| [Sessions](./sessions.md) | Session persistence, resume, and workspace tracking |
| [Architecture](./architecture.md) | Internal code structure, data flow, and design decisions |
| [Contributing](./contributing.md) | Development workflow, testing, and code conventions |
| [Container Workflow](./container.md) | Docker/Podman build and test workflows |

## Also in the repository

| File | Description |
|------|-------------|
| [USAGE.md](../USAGE.md) | Task-oriented usage guide with copy/paste examples |
| [PARITY.md](../PARITY.md) | Rust-port parity status and migration notes |
| [ROADMAP.md](../ROADMAP.md) | Active roadmap and open work |
| [PHILOSOPHY.md](../PHILOSOPHY.md) | Project intent and system-design framing |
| [rust/README.md](../rust/README.md) | Crate map, CLI surface, and features |

## Ecosystem

- [clawhip](https://github.com/Yeachan-Heo/clawhip) — Event and notification router
- [oh-my-openagent](https://github.com/code-yeongyu/oh-my-openagent) — Multi-agent coordination
- [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) — Workflow and plugin layer
- [oh-my-codex](https://github.com/Yeachan-Heo/oh-my-codex) — Planning and automation
- [UltraWorkers Discord](https://discord.gg/5TUQKqFWd) — Community

## Stats

- **~20K lines** of Rust
- **9 crates** in workspace
- **40+ built-in tools**
- **4 provider backends** (Anthropic, xAI, OpenAI-compat, DashScope)
- **Binary:** `claw`
- **Default model:** `claude-opus-4-6`
