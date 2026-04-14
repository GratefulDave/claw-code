# Contributing to Claw Code

Thank you for your interest in contributing! This guide covers the development workflow, verification steps, and conventions.

## Development prerequisites

- **Rust toolchain** — Latest stable, installed via [rustup](https://rustup.rs/)
- **Git** — For version control
- **An API key** — For live testing (optional; mock service available for most tests)

## Clone and build

```bash
git clone https://github.com/ultraworkers/claw-code
cd claw-code/rust
cargo build --workspace
```

## Verification checklist

Run these before submitting changes:

```bash
cd rust

# 1. Format check
cargo fmt --all --check

# 2. Lint
cargo clippy --workspace --all-targets -- -D warnings

# 3. Tests
cargo test --workspace
```

All three must pass cleanly. The workspace has `unsafe_code = "forbid"` and `clippy::all` + `clippy::pedantic` at warn level.

## Workspace structure

```
rust/
├── Cargo.toml              # Workspace root (resolver = "2", edition = "2021")
└── crates/
    ├── api/                # Provider clients + streaming + request preflight
    ├── commands/           # Shared slash-command registry + help rendering
    ├── compat-harness/     # TS manifest extraction harness
    ├── mock-anthropic-service/ # Deterministic local Anthropic mock
    ├── plugins/            # Plugin metadata, manager, install/enable/disable surfaces
    ├── runtime/            # Session, config, permissions, MCP, prompts, auth/runtime loop
    ├── rusty-claude-cli/   # Main CLI binary (`claw`)
    ├── telemetry/          # Session tracing and usage telemetry types
    └── tools/              # Built-in tools, skill resolution, tool search, agent runtime surfaces
```

## Code conventions

### Rust style

- **Edition 2021** — The workspace uses Rust edition 2021
- **No unsafe code** — `unsafe_code = "forbid"` is set at the workspace level
- **Clippy pedantic** — All pedantic warnings are enabled; allowed lints are documented in `Cargo.toml`
- **Serde for serialization** — All config, manifest, and session types use `serde`

### Testing

- **Unit tests** — Live in the same file as the code under `#[cfg(test)] mod tests`
- **Integration tests** — Live in `crates/rusty-claude-cli/tests/`
- **Mock parity harness** — Use the mock Anthropic service for deterministic end-to-end tests:
  ```bash
  ./scripts/run_mock_parity_harness.sh
  ```
- **Test isolation** — Tests that touch config, plugins, or env vars must use temp directories and env locks to avoid leaking state to sibling tests

### When behavior changes

- **Update both surfaces** — When behavior changes, update both source code and tests
- **Keep mocks in sync** — Update mock service scripts when API contracts change
- **Run the full workspace test suite** — Individual crate tests are not sufficient

## Making changes

### Typical workflow

1. Create a feature branch from `main`
2. Make changes (source + tests)
3. Run the verification checklist
4. Commit with a descriptive message
5. Push and open a PR

### Commit message conventions

Use conventional commit prefixes:

- `feat(scope):` — New feature
- `fix(scope):` — Bug fix
- `docs(scope):` — Documentation
- `test(scope):` — Tests
- `refactor(scope):` — Refactoring
- `chore(scope):` — Maintenance

Examples from the repo:
```
feat(runtime): add TaskRegistry — in-memory task lifecycle management
fix(api): OPENAI_BASE_URL wins over Anthropic fallback for unknown models
docs(roadmap): file ROADMAP #61 — OPENAI_BASE_URL routing fix (done)
test(cli): add integration test for model persistence in resumed /status
```

### Scope guidelines

- Keep changes **small and reviewable**
- Don't add speculative abstractions or compatibility shims
- Don't create files unless required
- If an approach fails, diagnose the failure before switching tactics

## Key integration test suites

### `cli_flags_and_config_defaults.rs`

Tests CLI flag parsing, config defaults, and JSON output across direct CLI invocations.

### `resume_slash_commands.rs`

Tests session resume with slash commands, JSON output in resumed mode, and model persistence.

### `mock_parity_harness.rs`

End-to-end parity tests using the deterministic mock Anthropic service. Covers streaming, file tools, bash, permissions, and plugin paths.

### `output_format_contract.rs`

Regression tests ensuring all CLI surfaces honor `--output-format json`.

## Mock Anthropic service

The `mock-anthropic-service` crate provides a deterministic Anthropic-compatible server for testing:

```bash
# Start manually
cargo run -p mock-anthropic-service -- --bind 127.0.0.1:0

# Run scripted harness
./scripts/run_mock_parity_harness.sh
```

Current harness scenarios:
- `streaming_text`
- `read_file_roundtrip`
- `grep_chunk_assembly`
- `write_file_allowed`
- `write_file_denied`
- `multi_tool_turn_roundtrip`
- `bash_stdout_roundtrip`
- `bash_permission_prompt_approved`
- `bash_permission_prompt_denied`
- `plugin_tool_roundtrip`

## Plugin development

See [plugins.md](./plugins.md) for the full plugin system documentation, including how to create, test, and install plugins.

## Documentation

- Keep documentation in `docs/` up to date when behavior changes
- Update `USAGE.md` for user-facing workflow changes
- Update `PARITY.md` for port status changes
- Update `ROADMAP.md` for new roadmap items or completed work

## Community

- [UltraWorkers Discord](https://discord.gg/5TUQKqFWd) — Community discussion and support
- [GitHub Issues](https://github.com/ultraworkers/claw-code/issues) — Bug reports and feature requests

## See also

- [Architecture](./architecture.md) — Internal code structure
- [Getting Started](./getting-started.md) — Build and run instructions
- [Container workflow](./container.md) — Docker/Podman development
