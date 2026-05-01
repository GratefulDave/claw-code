# DeepSeek Provider Setup

This repo includes a local patch that adds DeepSeek as a provider. This file explains how to configure and use it.

## Prerequisites

Get an API key from [platform.deepseek.com](https://platform.deepseek.com/api_keys).

## Setting your API key

### Option A — `.env` file (recommended for project use)

Create a `.env` file in the repo root (it is gitignored):

```
DEEPSEEK_API_KEY=sk-your-key-here
```

claw-code reads `.env` automatically — no `source` or export needed.

### Option B — shell export (current session only)

```bash
export DEEPSEEK_API_KEY=sk-your-key-here
```

### Option C — shell profile (permanent)

Add to `~/.zshrc` (or `~/.bashrc`):

```bash
export DEEPSEEK_API_KEY=sk-your-key-here
```

Then reload: `source ~/.zshrc`

## Available models

| Alias | Canonical model | Use case |
|-------|----------------|----------|
| `deepseek` | `deepseek-v4-pro` | Coding, reasoning (default) |
| `deepseek-v4-pro` | `deepseek-v4-pro` | Coding, reasoning |
| `deepseek-v4-flash` | `deepseek-v4-flash` | Fast, cost-efficient |

Context window: 1M tokens. Max output: 384K tokens.

## Usage

```bash
claw --model deepseek "refactor this function"
claw --model deepseek-v4-flash "summarise this file"
claw --model deepseek-v4-pro "write unit tests"
```

Set DeepSeek as your default model so you don't need `--model` every time:

```json
// .claw/settings.local.json (gitignored)
{
  "model": "deepseek-v4-pro"
}
```

Or using a short alias in `.claw.json`:

```json
{
  "aliases": {
    "ds": "deepseek-v4-pro",
    "dsf": "deepseek-v4-flash"
  }
}
```

Then: `claw --model ds "your prompt"`

## Optional: override the base URL

If you are running a DeepSeek-compatible proxy (e.g. a local mirror):

```bash
export DEEPSEEK_BASE_URL=http://localhost:8080/v1
```

## Staying up to date

DeepSeek support is a local patch on branch `local/deepseek-patches`. When upstream publishes new commits, run:

```bash
./update-claw.sh
```

This fetches upstream, rebases your DeepSeek commit on top, and rebuilds the binary. If a rebase conflict occurs, git will pause and show you which files to fix. After resolving:

```bash
git add <conflicting-file>
git rebase --continue
./update-claw.sh
```

## Troubleshooting

**`DEEPSEEK_API_KEY` not found error**
The key is missing or empty. Check with: `echo $DEEPSEEK_API_KEY`

**Using a model name that doesn't route to DeepSeek**
Model names must start with `deepseek` or `deepseek/`. Any other prefix selects a different provider.

**Rate limits / quota errors**
These come from the DeepSeek API itself. Check your plan at [platform.deepseek.com](https://platform.deepseek.com).
