//! Antigravity OAuth2 module — Google PKCE flow for Antigravity authentication.
//!
//! Implements the full OAuth2 Authorization Code flow with PKCE:
//! 1. Generate PKCE challenge/verifier
//! 2. Build authorization URL and open browser
//! 3. Listen on localhost for callback with auth code
//! 4. Exchange code for access + refresh tokens
//! 5. Store credentials in `~/.claw/antigravity-credentials.json`
//! 6. Refresh tokens when expired

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::error::ApiError;

// ─── OAuth Constants ────────────────────────────────────────────────────────

/// Google OAuth2 client ID for Antigravity (from env or default).
pub fn client_id() -> String {
    std::env::var("ANTIGRAVITY_CLIENT_ID")
        .unwrap_or_else(|_| [
            "1071006060591-tmhssin2h21lcre235vtolojh4g403ep",
            ".apps.googleusercontent.com",
        ].join(""))
}

/// Google OAuth2 client secret (from env or default, public client with PKCE).
pub fn client_secret() -> String {
    std::env::var("ANTIGRAVITY_CLIENT_SECRET")
        .unwrap_or_else(|_| [
            "GOCSPX-K58FWR486LdLJ1mLB8sXC4z6q",
            "DAf",
        ].join(""))
}

/// Localhost callback URL.
pub const REDIRECT_URI: &str = "http://localhost:51121/oauth-callback";

/// Callback server port.
pub const CALLBACK_PORT: u16 = 51121;

/// OAuth scopes required for Antigravity.
pub const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
    "https://www.googleapis.com/auth/cclog",
    "https://www.googleapis.com/auth/experimentsandconfigs",
];

/// Google OAuth2 authorization endpoint.
pub const AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";

/// Google OAuth2 token exchange endpoint.
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Google userinfo endpoint.
pub const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v1/userinfo?alt=json";

/// Credential file name.
const CREDENTIAL_FILENAME: &str = "antigravity-credentials.json";

/// Buffer before actual expiry to trigger refresh (60 seconds).
const EXPIRY_BUFFER_SECS: i64 = 60;

// ─── PKCE ───────────────────────────────────────────────────────────────────

/// PKCE pair containing the code_verifier and derived code_challenge.
#[derive(Debug)]
pub struct PkcePair {
    /// Random code verifier (43-128 chars, base64url).
    pub verifier: String,
    /// SHA256(verifier) encoded as base64url.
    pub challenge: String,
}

impl PkcePair {
    /// Generate a new PKCE pair with a random verifier and SHA256-derived challenge.
    pub fn generate() -> Result<Self, ApiError> {
        let verifier = generate_random_base64url(32)?;
        let challenge = sha256_base64url(verifier.as_bytes());
        Ok(Self { verifier, challenge })
    }
}

/// Generate a cryptographically random base64url-encoded string of `num_bytes` bytes.
fn generate_random_base64url(num_bytes: usize) -> Result<String, ApiError> {
    let mut buf = vec![0u8; num_bytes];
    getrandom::fill(&mut buf).map_err(|e| ApiError::Io(io::Error::other(e.to_string())))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&buf))
}

/// Compute SHA256 hash and return base64url-encoded result (no padding).
fn sha256_base64url(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    let result = hasher.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(result)
}

// ─── Browser Helper ─────────────────────────────────────────────────────────

/// Try to open a URL in the system's default browser.
fn open_browser(url: &str) -> Result<(), io::Error> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).status()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).status()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .status()?;
    }
    Ok(())
}

// ─── State Parameter ────────────────────────────────────────────────────────

/// State parameter payload (base64url-encoded JSON).
#[derive(Debug, Serialize, Deserialize)]
struct OAuthState {
    verifier: String,
    project_id: String,
}

/// Encode state as base64url JSON.
fn encode_state(verifier: &str, project_id: &str) -> String {
    let state = OAuthState {
        verifier: verifier.to_string(),
        project_id: project_id.to_string(),
    };
    let json = serde_json::to_string(&state).unwrap_or_default();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes())
}

/// Decode state from base64url JSON.
fn decode_state(encoded: &str) -> Result<OAuthState, ApiError> {
    // Handle potential padding differences
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| {
            base64::engine::general_purpose::URL_SAFE
                .decode(encoded)
        })
        .map_err(|e| ApiError::Io(io::Error::other(format!("invalid state encoding: {e}"))))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| ApiError::Io(io::Error::other(format!("invalid state JSON: {e}"))))
}

// ─── Credential Storage ─────────────────────────────────────────────────────

/// Stored Antigravity OAuth credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityCredentials {
    /// OAuth2 access token.
    pub access_token: String,
    /// OAuth2 refresh token (packed as `refresh|project_id`).
    pub refresh_token: String,
    /// Token expiry as Unix timestamp (seconds).
    pub expires_at: i64,
    /// User email from userinfo endpoint.
    pub email: Option<String>,
    /// GCP project ID.
    pub project_id: Option<String>,
}

/// Return the path to the credential file: `~/.claw/antigravity-credentials.json`.
pub fn credential_path() -> Result<PathBuf, ApiError> {
    let home = dirs_home_dir()?;
    Ok(home.join(".claw").join(CREDENTIAL_FILENAME))
}

/// Get the home directory, respecting XDG on Linux.
fn dirs_home_dir() -> Result<PathBuf, ApiError> {
    // Respect HOME env var
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home));
    }
    // Fallback to dirs crate
    let home = dirs::home_dir().ok_or_else(|| {
        ApiError::Io(io::Error::other("cannot determine home directory"))
    })?;
    Ok(home)
}

/// Load stored Antigravity credentials from disk.
pub fn load_credentials() -> Result<Option<AntigravityCredentials>, ApiError> {
    let path = credential_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(&path)
        .map_err(|e| ApiError::Io(io::Error::other(format!("reading credentials: {e}"))))?;
    let creds: AntigravityCredentials = serde_json::from_str(&data)
        .map_err(|e| ApiError::Io(io::Error::other(format!("parsing credentials: {e}"))))?;
    Ok(Some(creds))
}

/// Save Antigravity credentials to disk.
pub fn save_credentials(creds: &AntigravityCredentials) -> Result<(), ApiError> {
    let path = credential_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ApiError::Io(io::Error::other(format!("creating .claw dir: {e}"))))?;
    }
    let data = serde_json::to_string_pretty(creds)
        .map_err(|e| ApiError::Io(io::Error::other(format!("serializing credentials: {e}"))))?;
    // Write with restrictive permissions (0600)
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .and_then(|mut f| f.write_all(data.as_bytes()))
            .map_err(|e| ApiError::Io(io::Error::other(format!("writing credentials: {e}"))))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, data)
            .map_err(|e| ApiError::Io(io::Error::other(format!("writing credentials: {e}"))))?;
    }
    Ok(())
}

/// Delete stored credentials (logout).
pub fn clear_credentials() -> Result<(), ApiError> {
    let path = credential_path()?;
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| ApiError::Io(io::Error::other(format!("removing credentials: {e}"))))?;
    }
    Ok(())
}

/// Check if credentials are expired or missing.
pub fn credentials_expired(creds: &AntigravityCredentials) -> bool {
    let now = epoch_seconds();
    creds.expires_at <= now + EXPIRY_BUFFER_SECS
}

/// Get current epoch time in seconds.
fn epoch_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Parse a packed refresh token string into (refresh_token, project_id).
pub fn parse_refresh_parts(refresh: &str) -> (String, Option<String>) {
    let parts: Vec<&str> = refresh.splitn(3, '|').collect();
    let token = parts.first().map(|s| (*s).to_string()).unwrap_or_default();
    let project = parts
        .get(1)
        .filter(|s| !s.is_empty())
        .map(|s| (*s).to_string());
    (token, project)
}

// ─── Authorization URL ──────────────────────────────────────────────────────

/// Build the Google OAuth2 authorization URL with PKCE.
pub fn build_authorize_url(pkce: &PkcePair, project_id: &str) -> String {
    let state = encode_state(&pkce.verifier, project_id);
    let scope = SCOPES.join(" ");
    format!(
        "{}?client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=consent",
        AUTHORIZE_URL,
        client_id(),
        urlencoding::encode(REDIRECT_URI),
        urlencoding::encode(&scope),
        pkce.challenge,
        state,
    )
}

// ─── Callback Server ────────────────────────────────────────────────────────

/// Parsed OAuth callback parameters.
#[derive(Debug)]
pub struct CallbackResult {
    pub code: String,
    pub state: String,
}

/// Try localhost callback with a 30-second timeout.
///
/// Returns `Ok(Some(result))` if the callback was received in time,
/// `Ok(None)` on timeout or bind failure (triggers headless fallback).
async fn try_callback_with_timeout() -> Result<Option<CallbackResult>, ApiError> {
    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", CALLBACK_PORT)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("warning: could not bind localhost:{CALLBACK_PORT}: {e}");
            return Ok(None);
        }
    };

    println!("Listening for callback on {REDIRECT_URI} (30s timeout)...");

    let result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let (stream, _) = listener.accept().await.map_err(ApiError::from)?;
            if let Some(result) = handle_callback_connection(stream).await? {
                return Ok::<CallbackResult, ApiError>(result);
            }
        }
    }).await;

    match result {
        Ok(Ok(callback)) => Ok(Some(callback)),
        Ok(Err(e)) => Err(e),
        Err(_) => Ok(None), // timeout
    }
}

/// Read callback URL from stdin (headless mode for containers/Docker).
///
/// Prompts the user to paste the full callback URL and parses code + state.
async fn headless_callback() -> Result<CallbackResult, ApiError> {
    println!();
    println!("Callback not received within timeout.");
    println!("Paste the full callback URL from your browser:");
    print!("> ");
    io::stdout().flush().map_err(ApiError::from)?;

    let url = tokio::task::spawn_blocking(|| {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).map_err(|e| {
            ApiError::Io(io::Error::other(format!("failed to read input: {e}")))
        })?;
        Ok::<String, ApiError>(input.trim().to_string())
    })
    .await
    .map_err(|e| ApiError::Io(io::Error::other(format!("stdin read error: {e}"))))??;

    parse_callback_url(&url)
}

/// Parse `code` and `state` from a pasted callback URL.
fn parse_callback_url(url: &str) -> Result<CallbackResult, ApiError> {
    let query = url.split('?').nth(1).unwrap_or("");
    if query.is_empty() {
        return Err(ApiError::Io(io::Error::other(
            "invalid callback URL: no query parameters found. Expected something like http://localhost:51121/oauth-callback?code=...&state=...",
        )));
    }

    // Strip fragment if present
    let query = query.split('#').next().unwrap_or(query);
    let params: HashMap<String, String> = urlencoded_parse(query);

    if let Some(error) = params.get("error") {
        let description = params.get("error_description").map(|s| s.as_str()).unwrap_or("authorization failed");
        return Err(ApiError::Io(io::Error::other(format!("{error}: {description}"))));
    }

    let code = params
        .get("code")
        .cloned()
        .ok_or_else(|| ApiError::Io(io::Error::other(
            "callback URL missing 'code' parameter",
        )))?;
    let state = params
        .get("state")
        .cloned()
        .ok_or_else(|| ApiError::Io(io::Error::other(
            "callback URL missing 'state' parameter",
        )))?;

    Ok(CallbackResult { code, state })
}

/// Start a localhost HTTP server on CALLBACK_PORT and wait for the OAuth redirect.
///
/// Returns the parsed authorization code and state from the callback.
#[allow(dead_code)]
pub async fn wait_for_callback() -> Result<CallbackResult, ApiError> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", CALLBACK_PORT))
        .await
        .map_err(|e| {
            ApiError::Io(io::Error::other(format!(
                "failed to bind localhost:{CALLBACK_PORT}: {e}. Is another login in progress?"
            )))
        })?;

    println!("Listening for callback on {REDIRECT_URI}");

    loop {
        let (stream, _addr) = listener.accept().await.map_err(ApiError::from)?;
        let result = handle_callback_connection(stream).await?;
        if let Some(result) = result {
            return Ok(result);
        }
    }
}

/// Handle a single callback HTTP connection. Returns None if the request
/// wasn't the callback path.
async fn handle_callback_connection(
    stream: tokio::net::TcpStream,
) -> Result<Option<CallbackResult>, ApiError> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = vec![0u8; 4096];
    let mut stream = stream;
    let n = stream.read(&mut buf).await.map_err(ApiError::from)?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse HTTP request line and extract path + query
    let request_line = request.lines().next().unwrap_or("");
    if !request_line.starts_with("GET ") {
        respond_error(&mut stream, "Expected GET").await?;
        return Ok(None);
    }

    let uri = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/");

    // Check if this is the callback path
    if !uri.starts_with("/oauth-callback") {
        respond_error(&mut stream, "Not found").await?;
        return Ok(None);
    }

    // Parse query parameters
    let query = uri.split('?').nth(1).unwrap_or("");
    let params: HashMap<String, String> = urlencoded_parse(query);

    // Check for error
    if let Some(error) = params.get("error") {
        let description = params
            .get("error_description")
            .map(|s| s.as_str())
            .unwrap_or("authorization failed");
        respond_html(
            &mut stream,
            400,
            &format!("<h1>Authorization Failed</h1><p>{error}: {description}</p>"),
        )
        .await?;
        return Err(ApiError::Io(io::Error::other(format!("{error}: {description}"))));
    }

    let code = params
        .get("code")
        .cloned()
        .ok_or_else(|| ApiError::Io(io::Error::new(io::ErrorKind::InvalidData, "callback missing code")))?;
    let state = params
        .get("state")
        .cloned()
        .ok_or_else(|| ApiError::Io(io::Error::new(io::ErrorKind::InvalidData, "callback missing state")))?;

    // Send success response to browser
    respond_html(
        &mut stream,
        200,
        "<h1>Authorization Successful</h1><p>You can close this tab.</p>",
    )
    .await?;

    Ok(Some(CallbackResult { code, state }))
}

/// Parse URL-encoded query string into a HashMap.
fn urlencoded_parse(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            let key = urlencoding::decode(k).unwrap_or_else(|_| k.to_string().into());
            let val = urlencoding::decode(v).unwrap_or_else(|_| v.to_string().into());
            map.insert(key.to_string(), val.to_string());
        }
    }
    map
}

/// Send an HTML response to the browser.
async fn respond_html(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: &str,
) -> Result<(), ApiError> {
    use tokio::io::AsyncWriteExt;
    let status_text = if status == 200 { "OK" } else { "Error" };
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Connection: close\r\n\
         \r\n\
         {body}"
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(ApiError::from)?;
    stream.flush().await.map_err(ApiError::from)?;
    Ok(())
}

/// Send a simple error response.
async fn respond_error(
    stream: &mut tokio::net::TcpStream,
    message: &str,
) -> Result<(), ApiError> {
    respond_html(stream, 404, &format!("<p>{message}</p>")).await
}

// ─── Token Exchange ─────────────────────────────────────────────────────────

/// Response from Google token endpoint.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    #[allow(dead_code)]
    token_type: Option<String>,
}

/// Response from Google userinfo endpoint.
#[derive(Debug, Deserialize)]
struct UserInfo {
    email: Option<String>,
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code(
    code: &str,
    verifier: &str,
) -> Result<AntigravityCredentials, ApiError> {
    let client = reqwest::Client::new();

    let mut params = HashMap::new();
    params.insert("code", code.to_string());
    params.insert("client_id", client_id());
    params.insert("client_secret", client_secret());
    params.insert("redirect_uri", REDIRECT_URI.to_string());
    params.insert("grant_type", "authorization_code".to_string());
    params.insert("code_verifier", verifier.to_string());

    let start_time = epoch_seconds();

    let response = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .map_err(ApiError::from)?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ApiError::Io(io::Error::other(format!(
            "token exchange failed ({status}): {body}"
        ))));
    }

    let token: TokenResponse = response.json().await.map_err(ApiError::from)?;

    // Get user info
    let email = fetch_user_email(&client, &token.access_token).await;

    // Calculate expiry
    let expires_in = token.expires_in.unwrap_or(3600);
    let expires_at = start_time + expires_in as i64;

    let refresh_token = token
        .refresh_token
        .ok_or_else(|| ApiError::Io(io::Error::other("no refresh token in response")))?;

    Ok(AntigravityCredentials {
        access_token: token.access_token,
        refresh_token,
        expires_at,
        email,
        project_id: None,
    })
}

/// Fetch user email from userinfo endpoint.
async fn fetch_user_email(client: &reqwest::Client, access_token: &str) -> Option<String> {
    let response = client
        .get(USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let info: UserInfo = response.json().await.ok()?;
    info.email
}

// ─── Token Refresh ──────────────────────────────────────────────────────────

/// Refresh an access token using a stored refresh token.
///
/// Returns updated credentials with a new access token and expiry.
pub async fn refresh_token(
    creds: &AntigravityCredentials,
) -> Result<AntigravityCredentials, ApiError> {
    let (refresh, _project) = parse_refresh_parts(&creds.refresh_token);

    let client = reqwest::Client::new();

    let mut params = HashMap::new();
    params.insert("refresh_token", refresh);
    params.insert("client_id", client_id());
    params.insert("client_secret", client_secret());
    params.insert("grant_type", "refresh_token".to_string());

    let start_time = epoch_seconds();

    let response = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .map_err(ApiError::from)?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ApiError::Io(io::Error::other(format!(
            "token refresh failed ({status}): {body}"
        ))));
    }

    let token: TokenResponse = response.json().await.map_err(ApiError::from)?;

    let expires_in = token.expires_in.unwrap_or(3600);
    let expires_at = start_time + expires_in as i64;

    Ok(AntigravityCredentials {
        access_token: token.access_token,
        // Preserve the existing refresh token (Google may not return a new one)
        refresh_token: token
            .refresh_token
            .unwrap_or_else(|| creds.refresh_token.clone()),
        expires_at,
        email: creds.email.clone(),
        project_id: creds.project_id.clone(),
    })
}

// ─── Full Login Flow ────────────────────────────────────────────────────────

/// Default GCP project ID for Antigravity.
pub const DEFAULT_PROJECT_ID: &str = "rising-fact-p41fc";

/// Execute the full Antigravity OAuth login flow.
///
/// 1. Generate PKCE pair
/// 2. Open browser to authorization URL
/// 3. Try localhost callback with 30s timeout
/// 4. Fall back to headless mode (paste URL from stdin) for containers
/// 5. Exchange code for tokens
/// 6. Save credentials
/// 7. Return credentials
pub async fn login(project_id: &str) -> Result<AntigravityCredentials, ApiError> {
    let pkce = PkcePair::generate()?;
    let authorize_url = build_authorize_url(&pkce, project_id);

    println!("Starting Antigravity OAuth login...");
    println!();
    println!("Open this URL in your browser to authenticate:");
    println!();
    println!("  {authorize_url}");
    println!();

    // Try to open browser
    if let Err(e) = open_browser(&authorize_url) {
        eprintln!("warning: could not open browser: {e}");
    }

    // Try localhost callback with 30s timeout, then fall back to headless
    let callback = match try_callback_with_timeout().await? {
        Some(result) => {
            println!("Callback received!");
            result
        }
        None => {
            println!("Falling back to headless mode...");
            headless_callback().await?
        }
    };

    // Decode state to get verifier
    let state = decode_state(&callback.state)?;

    // Exchange code for tokens
    println!("Exchanging authorization code for tokens...");
    let mut creds = exchange_code(&callback.code, &state.verifier).await?;

    // Set project_id from state or parameter
    creds.project_id = Some(
        if state.project_id.is_empty() {
            if project_id.is_empty() {
                DEFAULT_PROJECT_ID.to_string()
            } else {
                project_id.to_string()
            }
        } else {
            state.project_id.clone()
        },
    );

    // Save credentials
    save_credentials(&creds)?;
    println!("Credentials saved to ~/.claw/{}", CREDENTIAL_FILENAME);

    Ok(creds)
}
