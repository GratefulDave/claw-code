//! Antigravity provider — Google's unified API gateway for Gemini and Claude models.
//!
//! Translates between Anthropic-format messages (used internally by claw-code)
//! and the Gemini-style format used by the Antigravity API.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::types::{
    ContentBlockDelta, ContentBlockDeltaEvent, ContentBlockStartEvent, ContentBlockStopEvent,
    InputContentBlock, InputMessage, MessageDelta, MessageDeltaEvent, MessageRequest,
    MessageResponse, MessageStartEvent, MessageStopEvent, OutputContentBlock, StreamEvent,
    ToolChoice, ToolResultContentBlock, Usage,
};

use super::{Provider, ProviderFuture};

// ─── Constants ───────────────────────────────────────────────────────────────

/// Default Antigravity endpoint (daily sandbox — same as CLIProxy/Vibeproxy).
pub const DEFAULT_BASE_URL: &str = "https://daily-cloudcode-pa.sandbox.googleapis.com";

/// Default GCP project ID when none is configured.
pub const DEFAULT_PROJECT_ID: &str = "rising-fact-p41fc";

const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(200);
const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(2);
const DEFAULT_MAX_RETRIES: u32 = 2;

const ANTIGRAVITY_ENV_VARS: &[&str] = &["ANTIGRAVITY_ACCESS_TOKEN"];

// ─── Model Resolution ────────────────────────────────────────────────────────

/// Thinking budget token budgets for each level.
pub const THINKING_BUDGET_MINIMAL: u32 = 1_024;
pub const THINKING_BUDGET_LOW: u32 = 8_092;
pub const THINKING_BUDGET_MEDIUM: u32 = 16_184;
pub const THINKING_BUDGET_HIGH: u32 = 32_368;

/// Parse a model name that may contain a thinking-variant suffix.
///
/// Suffix format: `:minimal`, `:low`, `:medium`, `:high`
///
/// # Examples
/// - `antigravity-claude-opus-4-6-thinking:high` → (`antigravity-claude-opus-4-6-thinking`, `Some("high")`)
/// - `antigravity-claude-opus-4-6-thinking`       → (`antigravity-claude-opus-4-6-thinking`, `None`)
#[must_use]
pub fn parse_model_and_thinking(model: &str) -> (String, Option<String>) {
    if let Some((base, variant)) = model.rsplit_once(':') {
        match variant {
            "minimal" | "low" | "medium" | "high" => return (base.to_string(), Some(variant.to_string())),
            _ => {}
        }
    }
    (model.to_string(), None)
}

/// Resolve the Antigravity API model name from a claw-code model identifier.
///
/// Strips the `antigravity-` prefix, removes any `:thinking-variant` suffix,
/// and applies known model-family mappings:
/// - `antigravity-gemini-3.1-pro` → `gemini-3.1-pro-preview`
/// - `antigravity-gemini-3-pro` → `gemini-3-pro-preview`
/// - `antigravity-gemini-3-flash` → `gemini-3-flash-preview`
/// - `antigravity-claude-opus-4-6-thinking` → `claude-opus-4-6-thinking`
/// - Other `antigravity-X` → `X` (passthrough)
#[must_use]
pub fn resolve_antigravity_model(model: &str) -> String {
    // Strip any thinking-variant suffix first
    let (base_model, _variant) = parse_model_and_thinking(model);
    let stripped = base_model.strip_prefix("antigravity-").unwrap_or(&base_model);
    match stripped {
        "gemini-3.1-pro" => "gemini-3.1-pro-preview".to_string(),
        "gemini-3-pro" => "gemini-3-pro-preview".to_string(),
        "gemini-3-flash" => "gemini-3-flash-preview".to_string(),
        other => other.to_string(),
    }
}

/// Check whether an Antigravity access token is available from any source.
///
/// Checks environment variable, credential file, and account pool.
#[must_use]
pub fn has_access_token() -> bool {
    std::env::var("ANTIGRAVITY_ACCESS_TOKEN").map_or(false, |v| !v.is_empty())
        || crate::oauth::load_credentials().map_or(false, |c| c.is_some())
        || crate::providers::antigravity_accounts::load_pool().map_or(false, |p| !p.accounts.is_empty())
}

// ─── Client ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AntigravityClient {
    http: reqwest::Client,
    access_token: String,
    project_id: String,
    base_url: String,
    max_retries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl AntigravityClient {
    /// Create a new client with explicit credentials.
    #[must_use]
    pub fn new(access_token: impl Into<String>, project_id: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            access_token: access_token.into(),
            project_id: project_id.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
        }
    }

    /// Create a client from environment variables, credential file, or account pool.
    ///
    /// Resolution order:
    /// 1. `ANTIGRAVITY_ACCESS_TOKEN` env var
    /// 2. Account pool (`~/.claw/antigravity-accounts.json`) with round-robin
    /// 3. Single credential file (`~/.claw/antigravity-credentials.json`)
    ///
    /// Optional: `ANTIGRAVITY_PROJECT_ID` (default: `rising-fact-p41fc`)
    /// Optional: `ANTIGRAVITY_ENDPOINT` (default: daily sandbox)
    pub fn from_env() -> Result<Self, ApiError> {
        // 1. Check ANTIGRAVITY_ACCESS_TOKEN env var
        if let Some(access_token) = read_env_non_empty("ANTIGRAVITY_ACCESS_TOKEN")? {
            let project_id = read_env_non_empty("ANTIGRAVITY_PROJECT_ID")?
                .unwrap_or_else(|| DEFAULT_PROJECT_ID.to_string());
            let base_url = read_env_non_empty("ANTIGRAVITY_ENDPOINT")?
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
            return Ok(Self::new(access_token, project_id).with_base_url(&base_url));
        }

        // 2. Try account pool
        if let Ok(mut pool) = crate::providers::antigravity_accounts::load_pool() {
            // Auto-import from single credential if pool is empty
            if pool.accounts.is_empty() {
                let _ = crate::providers::antigravity_accounts::import_from_single_credential();
                pool = crate::providers::antigravity_accounts::load_pool().unwrap_or_default();
            }
            if let Ok(account) = pool.next_available("default") {
                let base_url = read_env_non_empty("ANTIGRAVITY_ENDPOINT")?
                    .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
                return Ok(Self::new(&account.access_token, &account.project_id).with_base_url(&base_url));
            }
        }

        // 3. Try single credential file
        if let Ok(Some(creds)) = crate::oauth::load_credentials() {
            let project_id = creds.project_id.unwrap_or_else(|| DEFAULT_PROJECT_ID.to_string());
            let base_url = read_env_non_empty("ANTIGRAVITY_ENDPOINT")?
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
            return Ok(Self::new(creds.access_token, project_id).with_base_url(&base_url));
        }

        Err(ApiError::missing_credentials("Antigravity", ANTIGRAVITY_ENV_VARS))
    }

    #[must_use]
    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }

    #[must_use]
    pub fn with_retry_policy(
        mut self,
        max_retries: u32,
        initial_backoff: Duration,
        max_backoff: Duration,
    ) -> Self {
        self.max_retries = max_retries;
        self.initial_backoff = initial_backoff;
        self.max_backoff = max_backoff;
        self
    }

    // ── Non-streaming ────────────────────────────────────────────────────────

    pub async fn send_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        let request = MessageRequest {
            stream: false,
            ..request.clone()
        };
        let response = self.send_with_retry(&request).await?;
        let payload: GeminiEnvelope = response.json().await?;
        let mut msg = normalize_response(&payload, &request.model)?;
        if msg.request_id.is_none() {
            msg.request_id = payload.trace_id.clone();
        }
        Ok(msg)
    }

    // ── Streaming ────────────────────────────────────────────────────────────

    pub async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageStream, ApiError> {
        let response = self
            .send_with_retry(&request.clone().with_streaming())
            .await?;
        Ok(MessageStream {
            response,
            parser: AntigravitySseParser::new(),
            pending: VecDeque::new(),
            done: false,
            state: StreamState::new(request.model.clone()),
        })
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    async fn send_with_retry(
        &self,
        request: &MessageRequest,
    ) -> Result<reqwest::Response, ApiError> {
        let mut attempts = 0;

        let last_error = loop {
            attempts += 1;
            let retryable_error = match self.send_raw_request(request).await {
                Ok(response) => match expect_success(response).await {
                    Ok(response) => return Ok(response),
                    Err(error) if error.is_retryable() && attempts <= self.max_retries + 1 => error,
                    Err(error) => return Err(error),
                },
                Err(error) if error.is_retryable() && attempts <= self.max_retries + 1 => error,
                Err(error) => return Err(error),
            };

            if attempts > self.max_retries {
                break retryable_error;
            }

            tokio::time::sleep(self.backoff_for_attempt(attempts)?).await;
        };

        Err(ApiError::RetriesExhausted {
            attempts,
            last_error: Box::new(last_error),
        })
    }

    async fn send_raw_request(
        &self,
        request: &MessageRequest,
    ) -> Result<reqwest::Response, ApiError> {
        let api_model = resolve_antigravity_model(&request.model);
        let thinking_level = parse_model_and_thinking(&request.model).1;
        let is_streaming = request.stream;
        let endpoint = if is_streaming {
            format!(
                "{}/v1internal:streamGenerateContent?alt=sse",
                self.base_url.trim_end_matches('/')
            )
        } else {
            format!(
                "{}/v1internal:generateContent",
                self.base_url.trim_end_matches('/')
            )
        };

        let gemini_request = build_gemini_request(request, &api_model, thinking_level.as_deref());

        self.http
            .post(&endpoint)
            .header("content-type", "application/json")
            .bearer_auth(&self.access_token)
            .header("x-goog-user-project", &self.project_id)
            .json(&gemini_request)
            .send()
            .await
            .map_err(ApiError::from)
    }

    fn backoff_for_attempt(&self, attempt: u32) -> Result<Duration, ApiError> {
        let Some(multiplier) = 1_u32.checked_shl(attempt.saturating_sub(1)) else {
            return Err(ApiError::BackoffOverflow {
                attempt,
                base_delay: self.initial_backoff,
            });
        };
        Ok(self
            .initial_backoff
            .checked_mul(multiplier)
            .map_or(self.max_backoff, |delay| delay.min(self.max_backoff)))
    }
}

impl Provider for AntigravityClient {
    type Stream = MessageStream;

    fn send_message<'a>(
        &'a self,
        request: &'a MessageRequest,
    ) -> ProviderFuture<'a, MessageResponse> {
        Box::pin(async move { self.send_message(request).await })
    }

    fn stream_message<'a>(
        &'a self,
        request: &'a MessageRequest,
    ) -> ProviderFuture<'a, Self::Stream> {
        Box::pin(async move { self.stream_message(request).await })
    }
}

// ─── Request Transformation (Anthropic → Gemini) ────────────────────────────

/// Map a thinking level string to a token budget.
fn thinking_budget_for_level(level: &str) -> u32 {
    match level {
        "minimal" => THINKING_BUDGET_MINIMAL,
        "low" => THINKING_BUDGET_LOW,
        "high" => THINKING_BUDGET_HIGH,
        // Default to medium for "medium" or any unrecognized level
        _ => THINKING_BUDGET_MEDIUM,
    }
}

/// Build a Gemini-format request body from an Anthropic-format `MessageRequest`.
///
/// `thinking_level` is `Some("low" | "medium" | "high")` when the model name
/// included a thinking-variant suffix, or `None` otherwise.
fn build_gemini_request(
    request: &MessageRequest,
    api_model: &str,
    thinking_level: Option<&str>,
) -> Value {
    let tool_name_map = build_tool_name_map(&request.messages);

    let mut payload = json!({});

    // System instruction
    if let Some(system) = &request.system {
        payload["systemInstruction"] = json!({
            "role": "user",
            "parts": [{"text": system}]
        });
    }

    // Contents (messages → Gemini contents)
    payload["contents"] = translate_contents(&request.messages, &tool_name_map);

    // Generation config
    let mut gen_config = json!({
        "maxOutputTokens": request.max_tokens
    });

    // Enable thinking for models that support it
    if api_model.contains("thinking")
        || api_model.contains("gemini-3-pro")
        || api_model.contains("gemini-3.1-pro")
        || api_model.contains("gemini-3-flash")
    {
        let level = thinking_level.unwrap_or("medium");
        let budget = thinking_budget_for_level(level);
        gen_config["thinkingConfig"] = json!({
            "thinkingBudget": budget,
            "thinkingLevel": level.to_ascii_uppercase(),
        });
    }

    payload["generationConfig"] = gen_config;

    // Tools
    if let Some(tools) = &request.tools {
        if !tools.is_empty() {
            let declarations: Vec<Value> = tools
                .iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description.clone().unwrap_or_default(),
                        "parameters": tool.input_schema
                    })
                })
                .collect();
            payload["tools"] = json!([{ "functionDeclarations": declarations }]);
        }
    }

    // Tool choice
    if let Some(tool_choice) = &request.tool_choice {
        payload["toolConfig"] = translate_tool_choice(tool_choice);
    }

    payload
}

/// Build a mapping from `tool_use_id` → function name by scanning all messages.
///
/// Gemini's `functionResponse` uses the function name (not an ID) to match calls
/// with results, so we need this mapping when translating Anthropic `ToolResult`
/// blocks.
fn build_tool_name_map(messages: &[InputMessage]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for msg in messages {
        for block in &msg.content {
            if let InputContentBlock::ToolUse { id, name, .. } = block {
                map.insert(id.clone(), name.clone());
            }
        }
    }
    map
}

/// Translate Anthropic `messages` array to Gemini `contents` array.
fn translate_contents(
    messages: &[InputMessage],
    tool_name_map: &HashMap<String, String>,
) -> Value {
    let contents: Vec<Value> = messages
        .iter()
        .map(|msg| {
            let role = match msg.role.as_str() {
                "assistant" => "model",
                _ => "user",
            };
            let parts: Vec<Value> = msg
                .content
                .iter()
                .filter_map(|block| translate_content_block(block, tool_name_map))
                .collect();
            json!({ "role": role, "parts": parts })
        })
        .filter(|c| {
            c["parts"]
                .as_array()
                .is_some_and(|parts| !parts.is_empty())
        })
        .collect();
    Value::Array(contents)
}

/// Translate a single Anthropic content block to a Gemini part.
fn translate_content_block(
    block: &InputContentBlock,
    tool_name_map: &HashMap<String, String>,
) -> Option<Value> {
    match block {
        InputContentBlock::Text { text } => Some(json!({ "text": text })),

        InputContentBlock::ToolUse { name, input, .. } => {
            Some(json!({ "functionCall": { "name": name, "args": input } }))
        }

        InputContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            let name = tool_name_map
                .get(tool_use_id)
                .cloned()
                .unwrap_or_else(|| "unknown_function".to_string());

            let response_value = if content.len() == 1 {
                match &content[0] {
                    ToolResultContentBlock::Text { text } => json!({ "content": text }),
                    ToolResultContentBlock::Json { value } => value.clone(),
                }
            } else {
                let text = content
                    .iter()
                    .map(|c| match c {
                        ToolResultContentBlock::Text { text } => text.clone(),
                        ToolResultContentBlock::Json { value } => value.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                json!({ "content": text })
            };

            let response = if *is_error {
                let mut r = response_value;
                r["error"] = json!(true);
                r
            } else {
                response_value
            };

            Some(json!({ "functionResponse": { "name": name, "response": response } }))
        }
    }
}

/// Translate Anthropic `tool_choice` to Gemini `toolConfig`.
fn translate_tool_choice(tool_choice: &ToolChoice) -> Value {
    match tool_choice {
        ToolChoice::Auto => json!({ "function_calling_config": { "mode": "AUTO" } }),
        ToolChoice::Any => json!({ "function_calling_config": { "mode": "ANY" } }),
        ToolChoice::Tool { name } => json!({
            "function_calling_config": {
                "mode": "ANY",
                "allowed_function_names": [name]
            }
        }),
    }
}

// ─── Response Types (Gemini → Anthropic) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiEnvelope {
    #[serde(default)]
    response: GeminiResponse,
    #[serde(default)]
    trace_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    usage_metadata: Option<GeminiUsageMetadata>,
    #[serde(default)]
    model_version: Option<String>,
    #[serde(default)]
    response_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    #[serde(default)]
    content: GeminiContent,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiContent {
    #[serde(default)]
    role: String,
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiPart {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    function_call: Option<GeminiFunctionCall>,
    /// Gemini models use `thoughtText` for thinking output.
    #[serde(default)]
    thought_text: Option<String>,
    /// Claude models via Antigravity use `thought: true` + `text`.
    #[serde(default)]
    thought: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiFunctionCall {
    name: String,
    #[serde(default)]
    args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsageMetadata {
    #[serde(default)]
    prompt_token_count: u32,
    #[serde(default)]
    candidates_token_count: u32,
    #[serde(default)]
    total_token_count: u32,
    #[serde(default)]
    thoughts_token_count: Option<u32>,
}

// ─── Response Transformation (Gemini → Anthropic) ────────────────────────────

/// Convert a Gemini response envelope into an Anthropic-format `MessageResponse`.
fn normalize_response(envelope: &GeminiEnvelope, model: &str) -> Result<MessageResponse, ApiError> {
    let response = &envelope.response;
    let candidate = response.candidates.first().ok_or_else(|| {
        let body = serde_json::to_string(envelope).unwrap_or_default();
        ApiError::Api {
            status: reqwest::StatusCode::OK,
            error_type: Some("NO_CANDIDATES".to_string()),
            message: Some("Antigravity response contained no candidates".to_string()),
            body,
            retryable: false,
        }
    })?;

    let mut content_blocks: Vec<OutputContentBlock> = Vec::new();
    let mut tool_call_counter: u32 = 0;

    for part in &candidate.content.parts {
        // 1. Gemini-style thinking (thoughtText field)
        if let Some(thinking) = &part.thought_text {
            if !thinking.is_empty() {
                content_blocks.push(OutputContentBlock::Thinking {
                    thinking: thinking.clone(),
                    signature: None,
                });
            }
            continue;
        }

        // 2. Claude-style thinking via Antigravity (thought=true flag + text)
        if part.thought == Some(true) {
            if let Some(text) = &part.text {
                if !text.is_empty() {
                    content_blocks.push(OutputContentBlock::Thinking {
                        thinking: text.clone(),
                        signature: None,
                    });
                }
            }
            continue;
        }

        // 3. Regular text
        if let Some(text) = &part.text {
            content_blocks.push(OutputContentBlock::Text {
                text: text.clone(),
            });
            continue;
        }

        // 4. Function call → tool use
        if let Some(fc) = &part.function_call {
            let id = format!("toolu_{}", tool_call_counter);
            tool_call_counter += 1;
            content_blocks.push(OutputContentBlock::ToolUse {
                id,
                name: fc.name.clone(),
                input: fc.args.clone(),
            });
        }
    }

    // Determine stop reason: if there are tool calls, use "tool_use";
    // otherwise map the Gemini finish reason.
    let has_tool_use = content_blocks
        .iter()
        .any(|b| matches!(b, OutputContentBlock::ToolUse { .. }));

    let stop_reason = if has_tool_use {
        Some("tool_use".to_string())
    } else {
        candidate
            .finish_reason
            .as_deref()
            .map(normalize_finish_reason)
    };

    let usage = response
        .usage_metadata
        .as_ref()
        .map(|u| Usage {
            input_tokens: u.prompt_token_count,
            output_tokens: u.candidates_token_count,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        })
        .unwrap_or_else(|| Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });

    let id = envelope
        .trace_id
        .clone()
        .or_else(|| response.response_id.clone())
        .unwrap_or_default();

    Ok(MessageResponse {
        id,
        kind: "message".to_string(),
        role: "assistant".to_string(),
        content: content_blocks,
        model: response
            .model_version
            .clone()
            .unwrap_or_else(|| model.to_string()),
        stop_reason,
        stop_sequence: None,
        usage,
        request_id: None,
    })
}

/// Map Gemini finish reasons to Anthropic stop reasons.
fn normalize_finish_reason(reason: &str) -> String {
    match reason {
        "STOP" => "end_turn".to_string(),
        "MAX_TOKENS" => "max_tokens".to_string(),
        "SAFETY" | "RECITATION" | "OTHER" => "stop".to_string(),
        other => other.to_lowercase(),
    }
}

// ─── Error Handling ──────────────────────────────────────────────────────────

async fn expect_success(response: reqwest::Response) -> Result<reqwest::Response, ApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = response.text().await.unwrap_or_else(|_| String::new());
    let parsed_error = serde_json::from_str::<GeminiErrorEnvelope>(&body).ok();
    let retryable = is_retryable_status(status);

    Err(ApiError::Api {
        status,
        error_type: parsed_error.as_ref().and_then(|e| e.error.status.clone()),
        message: parsed_error.as_ref().and_then(|e| e.error.message.clone()),
        body,
        retryable,
    })
}

const fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 409 | 429 | 500 | 502 | 503 | 504)
}

#[derive(Debug, Deserialize)]
struct GeminiErrorEnvelope {
    error: GeminiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorDetail {
    #[serde(default)]
    code: Option<u16>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

// ─── SSE Streaming ───────────────────────────────────────────────────────────

/// Streaming message iterator that converts Gemini SSE chunks into
/// Anthropic-compatible `StreamEvent`s.
#[derive(Debug)]
pub struct MessageStream {
    response: reqwest::Response,
    parser: AntigravitySseParser,
    pending: VecDeque<StreamEvent>,
    done: bool,
    state: StreamState,
}

impl MessageStream {
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        None
    }

    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>, ApiError> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }

            if self.done {
                self.pending.extend(self.state.finish());
                if let Some(event) = self.pending.pop_front() {
                    return Ok(Some(event));
                }
                return Ok(None);
            }

            match self.response.chunk().await? {
                Some(chunk) => {
                    for parsed in self.parser.push(&chunk)? {
                        self.pending.extend(self.state.ingest_payload(parsed));
                    }
                }
                None => {
                    self.done = true;
                }
            }
        }
    }
}

/// SSE parser for Antigravity's `data: {...}\n\n` streaming format.
///
/// Unlike Anthropic SSE, Antigravity frames have no `event:` prefix — just
/// `data:` lines containing the full Gemini response envelope.
#[derive(Debug, Default)]
struct AntigravitySseParser {
    buffer: Vec<u8>,
}

impl AntigravitySseParser {
    fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, chunk: &[u8]) -> Result<Vec<GeminiEnvelope>, ApiError> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();

        while let Some(frame) = next_sse_frame(&mut self.buffer) {
            if let Some(envelope) = parse_sse_frame(&frame)? {
                events.push(envelope);
            }
        }

        Ok(events)
    }
}

/// Extract the next SSE frame (delimited by `\n\n`) from the buffer.
fn next_sse_frame(buffer: &mut Vec<u8>) -> Option<String> {
    let sep = b"\n\n";
    let position = buffer.windows(sep.len()).position(|w| w == sep)?;

    let separator_len = sep.len();
    let frame: Vec<u8> = buffer.drain(..position + separator_len).collect();
    let frame_len = frame.len().saturating_sub(separator_len);
    Some(String::from_utf8_lossy(&frame[..frame_len]).into_owned())
}

/// Parse a single SSE frame into a `GeminiEnvelope`.
fn parse_sse_frame(frame: &str) -> Result<Option<GeminiEnvelope>, ApiError> {
    let trimmed = frame.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let mut data_lines = Vec::new();
    for line in trimmed.lines() {
        if line.starts_with(':') {
            continue; // SSE comment
        }
        if let Some(data) = line.strip_prefix("data: ") {
            data_lines.push(data);
        } else if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.trim_start());
        }
    }

    if data_lines.is_empty() {
        return Ok(None);
    }

    let payload = data_lines.join("\n");
    if payload == "[DONE]" {
        return Ok(None);
    }

    serde_json::from_str::<GeminiEnvelope>(&payload)
        .map(Some)
        .map_err(ApiError::from)
}

/// Streaming state machine that converts Gemini SSE payloads into
/// Anthropic `StreamEvent`s.
#[derive(Debug)]
struct StreamState {
    model: String,
    message_started: bool,
    /// Index of the next content block to emit.
    current_index: u32,
    text_started: bool,
    thinking_started: bool,
    tool_calls: BTreeMap<u32, StreamToolCall>,
    usage: Option<Usage>,
    stop_reason: Option<String>,
    /// Whether the final `MessageDelta` + `MessageStop` have been emitted.
    finalized: bool,
}

#[derive(Debug)]
struct StreamToolCall {
    name: String,
    args: String,
}

impl StreamState {
    fn new(model: String) -> Self {
        Self {
            model,
            message_started: false,
            current_index: 0,
            text_started: false,
            thinking_started: false,
            tool_calls: BTreeMap::new(),
            usage: None,
            stop_reason: None,
            finalized: false,
        }
    }

    fn ingest_payload(&mut self, envelope: GeminiEnvelope) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        // Emit MessageStart on first payload
        if !self.message_started {
            self.message_started = true;
            events.push(StreamEvent::MessageStart(MessageStartEvent {
                message: MessageResponse {
                    id: envelope
                        .response
                        .response_id
                        .clone()
                        .or_else(|| envelope.trace_id.clone())
                        .unwrap_or_default(),
                    kind: "message".to_string(),
                    role: "assistant".to_string(),
                    content: Vec::new(),
                    model: envelope
                        .response
                        .model_version
                        .clone()
                        .unwrap_or_else(|| self.model.clone()),
                    stop_reason: None,
                    stop_sequence: None,
                    usage: Usage {
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                    request_id: None,
                },
            }));
        }

        // Update usage metadata
        if let Some(um) = &envelope.response.usage_metadata {
            self.usage = Some(Usage {
                input_tokens: um.prompt_token_count,
                output_tokens: um.candidates_token_count,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            });
        }

        // Process candidates
        for candidate in &envelope.response.candidates {
            if let Some(reason) = &candidate.finish_reason {
                self.stop_reason = Some(normalize_finish_reason(reason));
            }

            for part in &candidate.content.parts {
                self.ingest_part(part, &mut events);
            }
        }

        events
    }

    fn ingest_part(&mut self, part: &GeminiPart, events: &mut Vec<StreamEvent>) {
        // 1. Gemini-style thinking (thoughtText field)
        if let Some(thinking) = &part.thought_text {
            if !thinking.is_empty() {
                if !self.thinking_started {
                    self.thinking_started = true;
                    events.push(StreamEvent::ContentBlockStart(ContentBlockStartEvent {
                        index: self.current_index,
                        content_block: OutputContentBlock::Thinking {
                            thinking: String::new(),
                            signature: None,
                        },
                    }));
                }
                events.push(StreamEvent::ContentBlockDelta(ContentBlockDeltaEvent {
                    index: self.current_index,
                    delta: ContentBlockDelta::ThinkingDelta {
                        thinking: thinking.clone(),
                    },
                }));
            }
            return;
        }

        // 2. Claude-style thinking via Antigravity (thought=true + text)
        if part.thought == Some(true) {
            if let Some(text) = &part.text {
                if !text.is_empty() {
                    if !self.thinking_started {
                        self.thinking_started = true;
                        events.push(StreamEvent::ContentBlockStart(ContentBlockStartEvent {
                            index: self.current_index,
                            content_block: OutputContentBlock::Thinking {
                                thinking: String::new(),
                                signature: None,
                            },
                        }));
                    }
                    events.push(StreamEvent::ContentBlockDelta(ContentBlockDeltaEvent {
                        index: self.current_index,
                        delta: ContentBlockDelta::ThinkingDelta {
                            thinking: text.clone(),
                        },
                    }));
                }
            }
            return;
        }

        // 3. Regular text
        if let Some(text) = &part.text {
            if !text.is_empty() {
                // Close thinking block if transitioning to text
                if self.thinking_started {
                    events.push(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                        index: self.current_index,
                    }));
                    self.current_index += 1;
                    self.thinking_started = false;
                }

                if !self.text_started {
                    self.text_started = true;
                    events.push(StreamEvent::ContentBlockStart(ContentBlockStartEvent {
                        index: self.current_index,
                        content_block: OutputContentBlock::Text {
                            text: String::new(),
                        },
                    }));
                }
                events.push(StreamEvent::ContentBlockDelta(ContentBlockDeltaEvent {
                    index: self.current_index,
                    delta: ContentBlockDelta::TextDelta {
                        text: text.clone(),
                    },
                }));
            }
            return;
        }

        // 4. Function call → tool use
        if let Some(fc) = &part.function_call {
            // Close any open blocks
            if self.text_started {
                events.push(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                    index: self.current_index,
                }));
                self.current_index += 1;
                self.text_started = false;
            }
            if self.thinking_started {
                events.push(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                    index: self.current_index,
                }));
                self.current_index += 1;
                self.thinking_started = false;
            }

            let tc_index = self.current_index;
            let tc_id = format!("toolu_{}", tc_index);

            events.push(StreamEvent::ContentBlockStart(ContentBlockStartEvent {
                index: tc_index,
                content_block: OutputContentBlock::ToolUse {
                    id: tc_id.clone(),
                    name: fc.name.clone(),
                    input: json!({}),
                },
            }));

            let args_str = fc.args.to_string();
            if !args_str.is_empty() && args_str != "null" {
                events.push(StreamEvent::ContentBlockDelta(ContentBlockDeltaEvent {
                    index: tc_index,
                    delta: ContentBlockDelta::InputJsonDelta {
                        partial_json: args_str,
                    },
                }));
            }

            events.push(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                index: tc_index,
            }));

            self.tool_calls.insert(
                tc_index,
                StreamToolCall {
                    name: fc.name.clone(),
                    args: fc.args.to_string(),
                },
            );
            self.current_index += 1;
        }
    }

    /// Emit final events when the stream ends.
    fn finish(&mut self) -> Vec<StreamEvent> {
        if self.finalized || !self.message_started {
            return Vec::new();
        }
        self.finalized = true;

        let mut events = Vec::new();

        // Close any open content blocks
        if self.text_started || self.thinking_started {
            events.push(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                index: self.current_index,
            }));
        }

        // Determine final stop reason
        let has_tool_use = !self.tool_calls.is_empty();
        let stop_reason = if has_tool_use {
            "tool_use".to_string()
        } else {
            self.stop_reason
                .take()
                .unwrap_or_else(|| "end_turn".to_string())
        };

        let usage = self.usage.clone().unwrap_or_else(|| Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });

        events.push(StreamEvent::MessageDelta(MessageDeltaEvent {
            delta: MessageDelta {
                stop_reason: Some(stop_reason),
                stop_sequence: None,
            },
            usage,
        }));
        events.push(StreamEvent::MessageStop(MessageStopEvent {}));

        events
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn read_env_non_empty(key: &str) -> Result<Option<String>, ApiError> {
    match std::env::var(key) {
        Ok(value) if !value.is_empty() => Ok(Some(value)),
        Ok(_) | Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(ApiError::from(error)),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::InputContentBlock;
    use serde_json::json;
    #[test]
    fn resolves_antigravity_model_names() {
        assert_eq!(
            resolve_antigravity_model("antigravity-gemini-3.1-pro"),
            "gemini-3.1-pro-preview"
        );
        assert_eq!(
            resolve_antigravity_model("antigravity-gemini-3-pro"),
            "gemini-3-pro-preview"
        );
        assert_eq!(
            resolve_antigravity_model("antigravity-gemini-3-flash"),
            "gemini-3-flash-preview"
        );
        assert_eq!(
            resolve_antigravity_model("antigravity-claude-sonnet-4-6"),
            "claude-sonnet-4-6"
        );
        assert_eq!(
            resolve_antigravity_model("antigravity-claude-opus-4-6-thinking"),
            "claude-opus-4-6-thinking"
        );
    }

    #[test]
    fn builds_gemini_request_with_system_prompt() {
        let request = MessageRequest {
            model: "antigravity-claude-sonnet-4-6".to_string(),
            max_tokens: 1024,
            messages: vec![InputMessage::user_text("Hello")],
            system: Some("You are helpful.".to_string()),
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let payload = build_gemini_request(&request, "claude-sonnet-4-6", None);

        assert_eq!(payload["systemInstruction"]["role"], "user");
        assert_eq!(
            payload["systemInstruction"]["parts"][0]["text"],
            "You are helpful."
        );
        assert_eq!(payload["contents"][0]["role"], "user");
        assert_eq!(payload["contents"][0]["parts"][0]["text"], "Hello");
        assert_eq!(payload["generationConfig"]["maxOutputTokens"], 1024);
    }

    #[test]
    fn translates_tool_use_and_result_blocks() {
        let request = MessageRequest {
            model: "antigravity-claude-sonnet-4-6".to_string(),
            max_tokens: 4096,
            messages: vec![
                InputMessage {
                    role: "assistant".to_string(),
                    content: vec![InputContentBlock::ToolUse {
                        id: "toolu_123".to_string(),
                        name: "get_weather".to_string(),
                        input: json!({"location": "Paris"}),
                    }],
                },
                InputMessage {
                    role: "user".to_string(),
                    content: vec![InputContentBlock::ToolResult {
                        tool_use_id: "toolu_123".to_string(),
                        content: vec![ToolResultContentBlock::Json {
                            value: json!({"temperature": "22\u{00b0}C"}),
                        }],
                        is_error: false,
                    }],
                },
            ],
            system: None,
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let payload = build_gemini_request(&request, "claude-sonnet-4-6", None);

        // Model turn with function call
        assert_eq!(payload["contents"][0]["role"], "model");
        assert_eq!(
            payload["contents"][0]["parts"][0]["functionCall"]["name"],
            "get_weather"
        );

        // User turn with function response
        assert_eq!(payload["contents"][1]["role"], "user");
        assert_eq!(
            payload["contents"][1]["parts"][0]["functionResponse"]["name"],
            "get_weather"
        );
    }

    #[test]
    fn normalizes_gemini_finish_reasons() {
        assert_eq!(normalize_finish_reason("STOP"), "end_turn");
        assert_eq!(normalize_finish_reason("MAX_TOKENS"), "max_tokens");
        assert_eq!(normalize_finish_reason("SAFETY"), "stop");
        assert_eq!(normalize_finish_reason("RECITATION"), "stop");
        assert_eq!(normalize_finish_reason("OTHER"), "stop");
    }

    #[test]
    fn normalizes_gemini_response_with_text() {
        let envelope = GeminiEnvelope {
            response: GeminiResponse {
                candidates: vec![GeminiCandidate {
                    content: GeminiContent {
                        role: "model".to_string(),
                        parts: vec![GeminiPart {
                            text: Some("Hello!".to_string()),
                            function_call: None,
                            thought_text: None,
                            thought: None,
                        }],
                    },
                    finish_reason: Some("STOP".to_string()),
                }],
                usage_metadata: Some(GeminiUsageMetadata {
                    prompt_token_count: 50,
                    candidates_token_count: 10,
                    total_token_count: 60,
                    thoughts_token_count: None,
                }),
                model_version: Some("claude-sonnet-4-6".to_string()),
                response_id: Some("resp_123".to_string()),
            },
            trace_id: Some("trace_456".to_string()),
        };

        let msg = normalize_response(&envelope, "antigravity-claude-sonnet-4-6").unwrap();

        assert_eq!(msg.id, "trace_456");
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.stop_reason, Some("end_turn".to_string()));
        assert_eq!(msg.usage.input_tokens, 50);
        assert_eq!(msg.usage.output_tokens, 10);
        assert_eq!(msg.content.len(), 1);
    }

    #[test]
    fn normalizes_gemini_response_with_function_call() {
        let envelope = GeminiEnvelope {
            response: GeminiResponse {
                candidates: vec![GeminiCandidate {
                    content: GeminiContent {
                        role: "model".to_string(),
                        parts: vec![GeminiPart {
                            text: None,
                            function_call: Some(GeminiFunctionCall {
                                name: "get_weather".to_string(),
                                args: json!({"location": "Paris"}),
                            }),
                            thought_text: None,
                            thought: None,
                        }],
                    },
                    finish_reason: Some("STOP".to_string()),
                }],
                usage_metadata: None,
                model_version: None,
                response_id: None,
            },
            trace_id: Some("trace_789".to_string()),
        };

        let msg = normalize_response(&envelope, "antigravity-claude-sonnet-4-6").unwrap();

        assert_eq!(msg.stop_reason, Some("tool_use".to_string()));
        assert_eq!(msg.content.len(), 1);
        match &msg.content[0] {
            OutputContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "toolu_0");
                assert_eq!(name, "get_weather");
                assert_eq!(input["location"], "Paris");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn parses_antigravity_sse_frames() {
        let mut parser = AntigravitySseParser::new();

        let payload = concat!(
            "data: {\"response\":{\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Hello\"}]}}]},\"traceId\":\"t1\"}\n\n",
            "data: {\"response\":{\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\" world\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":5,\"totalTokenCount\":15}},\"traceId\":\"t1\"}\n\n"
        );

        let envelopes = parser.push(payload.as_bytes()).unwrap();
        assert_eq!(envelopes.len(), 2);
        assert_eq!(
            envelopes[0].response.candidates[0].content.parts[0]
                .text
                .as_deref(),
            Some("Hello")
        );
        assert_eq!(
            envelopes[1].response.candidates[0]
                .finish_reason
                .as_deref(),
            Some("STOP")
        );
    }

    #[test]
    fn tool_choice_translation() {
        let auto = translate_tool_choice(&ToolChoice::Auto);
        assert_eq!(auto["function_calling_config"]["mode"], "AUTO");

        let any = translate_tool_choice(&ToolChoice::Any);
        assert_eq!(any["function_calling_config"]["mode"], "ANY");

        let named = translate_tool_choice(&ToolChoice::Tool {
            name: "my_func".to_string(),
        });
        assert_eq!(named["function_calling_config"]["mode"], "ANY");
        assert_eq!(
            named["function_calling_config"]["allowed_function_names"][0],
            "my_func"
        );
    }

    #[test]
    fn includes_thinking_config_for_thinking_models() {
        let request = MessageRequest {
            model: "antigravity-claude-opus-4-6-thinking".to_string(),
            max_tokens: 8192,
            messages: vec![InputMessage::user_text("think about this")],
            system: None,
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let payload = build_gemini_request(&request, "claude-opus-4-6-thinking", None);
        assert!(payload["generationConfig"]["thinkingConfig"].is_object());
        // Default (no variant) maps to medium
        assert_eq!(
            payload["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            16_184
        );
        assert_eq!(
            payload["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            "MEDIUM"
        );
    }

    #[test]
    fn thinking_variant_high_uses_high_budget() {
        let request = MessageRequest {
            model: "antigravity-claude-opus-4-6-thinking:high".to_string(),
            max_tokens: 8192,
            messages: vec![InputMessage::user_text("think hard")],
            system: None,
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let payload = build_gemini_request(&request, "claude-opus-4-6-thinking", Some("high"));
        assert_eq!(
            payload["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            32_368
        );
        assert_eq!(
            payload["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            "HIGH"
        );
    }

    #[test]
    fn thinking_variant_low_uses_low_budget() {
        let request = MessageRequest {
            model: "antigravity-claude-sonnet-4-6-thinking:low".to_string(),
            max_tokens: 8192,
            messages: vec![InputMessage::user_text("think briefly")],
            system: None,
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let payload = build_gemini_request(&request, "claude-sonnet-4-6-thinking", Some("low"));
        assert_eq!(
            payload["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            8_092
        );
        assert_eq!(
            payload["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            "LOW"
        );
    }

    #[test]
    fn skips_thinking_config_for_non_thinking_models() {
        let request = MessageRequest {
            model: "antigravity-claude-sonnet-4-6".to_string(),
            max_tokens: 4096,
            messages: vec![InputMessage::user_text("hello")],
            system: None,
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let payload = build_gemini_request(&request, "claude-sonnet-4-6", None);
        assert!(payload["generationConfig"]["thinkingConfig"].is_null());
    }

    #[test]
    fn backoff_doubles_until_maximum() {
        let client = AntigravityClient::new("test-token", "test-project")
            .with_retry_policy(3, Duration::from_millis(10), Duration::from_millis(25));
        assert_eq!(
            client.backoff_for_attempt(1).unwrap(),
            Duration::from_millis(10)
        );
        assert_eq!(
            client.backoff_for_attempt(2).unwrap(),
            Duration::from_millis(20)
        );
        assert_eq!(
            client.backoff_for_attempt(3).unwrap(),
            Duration::from_millis(25)
        );
    }

    #[test]
    fn parse_model_and_thinking_strips_variant() {
        let (base, variant) = parse_model_and_thinking("antigravity-claude-opus-4-6-thinking:high");
        assert_eq!(base, "antigravity-claude-opus-4-6-thinking");
        assert_eq!(variant.as_deref(), Some("high"));

        let (base, variant) = parse_model_and_thinking("antigravity-claude-sonnet-4-6-thinking:low");
        assert_eq!(base, "antigravity-claude-sonnet-4-6-thinking");
        assert_eq!(variant.as_deref(), Some("low"));

        let (base, variant) = parse_model_and_thinking("antigravity-claude-opus-4-6-thinking");
        assert_eq!(base, "antigravity-claude-opus-4-6-thinking");
        assert!(variant.is_none());
    }

    #[test]
    fn parse_model_and_thinking_ignores_non_thinking_suffixes() {
        // A model with a colon that isn't a thinking variant should pass through
        let (base, variant) = parse_model_and_thinking("antigravity-claude-opus-4-6:v2");
        assert_eq!(base, "antigravity-claude-opus-4-6:v2");
        assert!(variant.is_none());
    }

    #[test]
    fn resolve_antigravity_model_strips_thinking_variant() {
        assert_eq!(
            resolve_antigravity_model("antigravity-claude-opus-4-6-thinking:high"),
            "claude-opus-4-6-thinking"
        );
        assert_eq!(
            resolve_antigravity_model("antigravity-claude-sonnet-4-6-thinking:low"),
            "claude-sonnet-4-6-thinking"
        );
        assert_eq!(
            resolve_antigravity_model("antigravity-claude-opus-4-6-thinking"),
            "claude-opus-4-6-thinking"
        );
    }
}
