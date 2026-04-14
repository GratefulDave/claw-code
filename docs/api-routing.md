# API Routing and Model Providers

Claw Code supports multiple LLM providers through a unified routing system. The provider is selected automatically based on the model name and available credentials.

## Provider matrix

| Provider | Protocol | Auth env var(s) | Base URL env var | Default base URL |
|----------|----------|-----------------|------------------|------------------|
| **Anthropic** (direct) | Anthropic Messages API | `ANTHROPIC_API_KEY` or `ANTHROPIC_AUTH_TOKEN` or OAuth | `ANTHROPIC_BASE_URL` | `https://api.anthropic.com` |
| **xAI** | OpenAI-compatible | `XAI_API_KEY` | `XAI_BASE_URL` | `https://api.x.ai/v1` |
| **OpenAI-compatible** | OpenAI Chat Completions | `OPENAI_API_KEY` | `OPENAI_BASE_URL` | `https://api.openai.com/v1` |
| **DashScope** (Alibaba) | OpenAI-compatible | `DASHSCOPE_API_KEY` | `DASHSCOPE_BASE_URL` | `https://dashscope.aliyuncs.com/compatible-mode/v1` |

The OpenAI-compatible backend also serves as the gateway for **OpenRouter**, **Ollama**, and any other service that speaks the OpenAI `/v1/chat/completions` wire format.

## How provider detection works

Provider selection follows this priority:

1. **Model-name prefix routing** (always wins):
   - `claude` → Anthropic
   - `grok` → xAI
   - `openai/` → OpenAI-compatible
   - `gpt-` → OpenAI-compatible
   - `qwen/` or `qwen-` → DashScope (OpenAI-compatible with DashScope-specific auth)

2. **Env-var presence** (fallback when no prefix matches):
   - `ANTHROPIC_API_KEY` or `ANTHROPIC_AUTH_TOKEN` → Anthropic
   - `OPENAI_API_KEY` + `OPENAI_BASE_URL` → OpenAI-compatible
   - `OPENAI_BASE_URL` alone (no key, e.g. Ollama) → OpenAI-compatible (last resort)
   - `XAI_API_KEY` → xAI

3. **Default** → Anthropic

## Using Anthropic (default)

```bash
export ANTHROPIC_API_KEY="sk-ant-..."

claw prompt "hello"
claw --model opus "explain this code"
claw --model sonnet "review the diff"
```

### Anthropic-compatible proxy

```bash
export ANTHROPIC_BASE_URL="http://127.0.0.1:8080"
export ANTHROPIC_AUTH_TOKEN="local-dev-token"

claw --model "claude-sonnet-4-6" prompt "reply with the word ready"
```

## Using OpenAI / OpenRouter / Ollama

### Direct OpenAI

```bash
export OPENAI_API_KEY="sk-..."

claw --model "openai/gpt-4" prompt "hello"
```

### OpenRouter

```bash
export OPENAI_BASE_URL="https://openrouter.ai/api/v1"
export OPENAI_API_KEY="sk-or-v1-..."

claw --model "openai/gpt-4.1-mini" prompt "summarize this repo"
```

### Ollama (local models)

```bash
export OPENAI_BASE_URL="http://127.0.0.1:11434/v1"
unset OPENAI_API_KEY

claw --model "llama3.2" prompt "hello"
```

Any model name not matching a known prefix is passed through verbatim. Use this for Ollama tags, OpenRouter slugs, or custom backend IDs.

## Using xAI (Grok)

```bash
export XAI_API_KEY="..."

claw --model "grok-3" prompt "explain Rust ownership"
claw --model "grok-mini" "summarize this"
```

## Using DashScope (Alibaba Qwen)

```bash
export DASHSCOPE_API_KEY="sk-..."

claw --model "qwen-plus" prompt "hello"
claw --model "qwen/qwen-max" prompt "write a poem"
```

Model names starting with `qwen/` or `qwen-` are automatically routed to the DashScope endpoint. You do **not** need to set `OPENAI_BASE_URL` or unset `ANTHROPIC_API_KEY` — the prefix router wins.

Reasoning variants (`qwen-qwq-*`, `qwq-*`, `*-thinking`) automatically strip `temperature`/`top_p`/`frequency_penalty`/`presence_penalty` before the request hits the wire.

## Model aliases

| Alias | Resolved model name | Provider | Max output tokens | Context window |
|-------|-------------------|----------|-------------------|----------------|
| `opus` | `claude-opus-4-6` | Anthropic | 32,000 | 200,000 |
| `sonnet` | `claude-sonnet-4-6` | Anthropic | 64,000 | 200,000 |
| `haiku` | `claude-haiku-4-5-20251213` | Anthropic | 64,000 | 200,000 |
| `grok` / `grok-3` | `grok-3` | xAI | 64,000 | 131,072 |
| `grok-mini` / `grok-3-mini` | `grok-3-mini` | xAI | 64,000 | 131,072 |
| `grok-2` | `grok-2` | xAI | — | — |

## User-defined aliases

Add custom aliases in any settings file:

```json
{
  "aliases": {
    "fast": "claude-haiku-4-5-20251213",
    "smart": "claude-opus-4-6",
    "cheap": "grok-3-mini"
  }
}
```

Aliases resolve transitively — `"fast": "haiku"` works because `haiku` is a built-in alias. Local project settings override user-level settings.

## Auth credential mapping

Getting the right credential in the right env var is the most common source of 401 errors:

| Credential shape | Env var | HTTP header | Typical source |
|-----------------|---------|-------------|----------------|
| `sk-ant-*` API key | `ANTHROPIC_API_KEY` | `x-api-key: sk-ant-...` | [console.anthropic.com](https://console.anthropic.com) |
| OAuth access token (opaque) | `ANTHROPIC_AUTH_TOKEN` | `Authorization: Bearer ...` | `claw login` or proxy |
| OpenRouter key (`sk-or-v1-*`) | `OPENAI_API_KEY` + `OPENAI_BASE_URL` | `Authorization: Bearer ...` | [openrouter.ai/keys](https://openrouter.ai/keys) |

### Common mistake: `sk-ant-*` in the wrong env var

If you put an `sk-ant-*` key in `ANTHROPIC_AUTH_TOKEN`, Anthropic's API returns `401 Invalid bearer token`. The fix is to move it to `ANTHROPIC_API_KEY`.

Recent `claw` builds detect this exact pattern and append a hint to the error message.

## HTTP proxy support

`claw` honors the standard proxy env vars:

```bash
export HTTPS_PROXY="http://proxy.corp.example:3128"
export HTTP_PROXY="http://proxy.corp.example:3128"
export NO_PROXY="localhost,127.0.0.1,.corp.example"
```

Both upper-case and lower-case spellings are accepted. Empty values are treated as unset.

## See also

- [CLI Reference](./cli-reference.md) — `--model` flag and model aliases
- [Configuration](./configuration.md) — `aliases`, `mcpServers`, and auth config
- [Getting Started](./getting-started.md) — quick setup for each provider
