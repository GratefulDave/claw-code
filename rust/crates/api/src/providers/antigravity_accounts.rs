//! Antigravity multi-account rotation module.
//!
//! Manages multiple Antigravity OAuth accounts with round-robin rotation
//! and per-account rate-limit detection. Account pool is persisted to
//! `~/.claw/antigravity-accounts.json`.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::ApiError;

// ─── Constants ───────────────────────────────────────────────────────────────

/// Account pool file name.
const POOL_FILENAME: &str = "antigravity-accounts.json";

/// Buffer before actual expiry to trigger refresh (60 seconds).
const EXPIRY_BUFFER_SECS: i64 = 60;

// ─── Account Struct ─────────────────────────────────────────────────────────

/// A single Antigravity OAuth account with rotation metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityAccount {
    /// Account email (used as identifier).
    pub email: Option<String>,
    /// OAuth2 access token.
    pub access_token: String,
    /// OAuth2 refresh token (packed as `refresh|project_id`).
    pub refresh_token: String,
    /// GCP project ID.
    pub project_id: String,
    /// Whether this account is eligible for rotation.
    pub enabled: bool,
    /// Unix timestamp when the account was added.
    pub added_at: u64,
    /// Unix timestamp when rate limit expires, if rate-limited.
    pub rate_limited_until: Option<i64>,
    /// Unix timestamp when the access token expires.
    pub expires_at: i64,
}

// ─── Account Pool ────────────────────────────────────────────────────────────

/// Pool of Antigravity accounts with round-robin rotation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountPool {
    /// All accounts in the pool.
    pub accounts: Vec<AntigravityAccount>,
    /// Index of the most recently used account.
    pub active_index: usize,
    /// Per-model-family rotation tracking (model_family → next index).
    pub last_rotation: HashMap<String, usize>,
}

impl Default for AccountPool {
    fn default() -> Self {
        Self {
            accounts: Vec::new(),
            active_index: 0,
            last_rotation: HashMap::new(),
        }
    }
}

// ─── File Storage Helpers ────────────────────────────────────────────────────

/// Return the home directory, respecting `$HOME` on Linux.
fn home_dir() -> Result<PathBuf, ApiError> {
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home));
    }
    dirs::home_dir().ok_or_else(|| {
        ApiError::Io(std::io::Error::other("cannot determine home directory"))
    })
}

/// Return the path to the account pool file: `~/.claw/antigravity-accounts.json`.
pub fn pool_path() -> Result<PathBuf, ApiError> {
    let home = home_dir()?;
    Ok(home.join(".claw").join(POOL_FILENAME))
}

/// Load the account pool from disk.
///
/// Returns an empty pool if the file does not exist.
pub fn load_pool() -> Result<AccountPool, ApiError> {
    let path = pool_path()?;
    if !path.exists() {
        return Ok(AccountPool::default());
    }
    let data = std::fs::read_to_string(&path)
        .map_err(|e| ApiError::Io(std::io::Error::other(format!("reading account pool: {e}"))))?;
    let pool: AccountPool = serde_json::from_str(&data)
        .map_err(|e| ApiError::Io(std::io::Error::other(format!("parsing account pool: {e}"))))?;
    Ok(pool)
}

// ─── AccountPool Methods ─────────────────────────────────────────────────────

impl AccountPool {
    /// Persist the pool to disk.
    ///
    /// Creates parent directories as needed. On Unix, sets file mode 0600.
    pub fn save_pool(&self) -> Result<(), ApiError> {
        let path = pool_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ApiError::Io(std::io::Error::other(format!("creating .claw dir: {e}")))
            })?;
        }
        let data = serde_json::to_string_pretty(self).map_err(|e| {
            ApiError::Io(std::io::Error::other(format!("serializing account pool: {e}")))
        })?;

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
                .map_err(|e| {
                    ApiError::Io(std::io::Error::other(format!("writing account pool: {e}")))
                })?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&path, &data).map_err(|e| {
                ApiError::Io(std::io::Error::other(format!("writing account pool: {e}")))
            })?;
        }

        Ok(())
    }

    /// Add an account to the end of the pool.
    pub fn add_account(&mut self, account: AntigravityAccount) {
        self.accounts.push(account);
    }

    /// Remove an account by email. Returns `true` if an account was removed.
    pub fn remove_account(&mut self, email: &str) -> bool {
        let before = self.accounts.len();
        self.accounts.retain(|a| a.email.as_deref() != Some(email));
        self.accounts.len() < before
    }

    /// Get the next available account using round-robin rotation.
    ///
    /// - Skips disabled accounts.
    /// - Skips rate-limited accounts.
    /// - Tracks per-model-family rotation index.
    /// - If all accounts are rate-limited, returns error with earliest retry time.
    /// - If no enabled accounts exist, returns error.
    pub fn next_available(
        &mut self,
        model_family: &str,
    ) -> Result<&AntigravityAccount, ApiError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let start = self.last_rotation.get(model_family).copied().unwrap_or(0);
        let len = self.accounts.len();

        if len == 0 {
            return Err(ApiError::Auth("no accounts in pool".to_string()));
        }

        let mut earliest_rate_limit: Option<i64> = None;

        for i in 0..len {
            let idx = (start + i) % len;
            let account = &self.accounts[idx];

            if !account.enabled {
                continue;
            }

            if let Some(until) = account.rate_limited_until {
                if until > now {
                    earliest_rate_limit =
                        Some(earliest_rate_limit.map_or(until, |e| e.min(until)));
                    continue;
                }
            }

            // Found an available account — update rotation state and return.
            let next_idx = (idx + 1) % len;
            self.last_rotation
                .insert(model_family.to_string(), next_idx);
            self.active_index = idx;
            return Ok(&self.accounts[idx]);
        }

        // All accounts are rate-limited or disabled.
        if let Some(earliest) = earliest_rate_limit {
            let retry_after = earliest - now;
            Err(ApiError::Auth(format!(
                "all accounts rate-limited; retry after {retry_after}s"
            )))
        } else {
            Err(ApiError::Auth("no enabled accounts in pool".to_string()))
        }
    }

    /// Mark an account as rate-limited for the given duration.
    pub fn mark_rate_limited(&mut self, email: &str, retry_after_secs: i64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let until = now + retry_after_secs;
        for account in &mut self.accounts {
            if account.email.as_deref() == Some(email) {
                account.rate_limited_until = Some(until);
                break;
            }
        }
    }

    /// Update an account's access token and expiry after a refresh.
    pub fn update_account(&mut self, email: &str, access_token: String, expires_at: i64) {
        for account in &mut self.accounts {
            if account.email.as_deref() == Some(email) {
                account.access_token = access_token;
                account.expires_at = expires_at;
                break;
            }
        }
    }

    /// List all accounts in the pool.
    pub fn list_accounts(&self) -> &[AntigravityAccount] {
        &self.accounts
    }
}

// ─── Auto-refresh ────────────────────────────────────────────────────────────

/// Refresh an account's access token if it has expired.
///
/// Checks the `expires_at` field against the current time (with a 60-second
/// buffer). If expired, calls `oauth::refresh_token()` and persists the
/// updated pool to disk.
pub async fn refresh_account_if_needed(
    pool: &mut AccountPool,
    email: &str,
) -> Result<(), ApiError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let account_idx = pool
        .accounts
        .iter()
        .position(|a| a.email.as_deref() == Some(email))
        .ok_or_else(|| ApiError::Auth(format!("account not found: {email}")))?;

    // Not expired yet — nothing to do.
    if pool.accounts[account_idx].expires_at > now + EXPIRY_BUFFER_SECS {
        return Ok(());
    }

    // Build credentials for the refresh call.
    let account = &pool.accounts[account_idx];
    let creds = crate::oauth::AntigravityCredentials {
        access_token: account.access_token.clone(),
        refresh_token: account.refresh_token.clone(),
        expires_at: account.expires_at,
        email: account.email.clone(),
        project_id: Some(account.project_id.clone()),
    };

    let refreshed = crate::oauth::refresh_token(&creds).await?;

    // Update the in-memory account.
    pool.accounts[account_idx].access_token = refreshed.access_token;
    pool.accounts[account_idx].refresh_token = refreshed.refresh_token;
    pool.accounts[account_idx].expires_at = refreshed.expires_at;

    pool.save_pool()?;
    Ok(())
}

// ─── Import Helper ───────────────────────────────────────────────────────────

/// Import an existing single-credential login into the multi-account pool.
///
/// Reads `~/.claw/antigravity-credentials.json` (the legacy single-account
/// file) and adds it to the pool if not already present.
///
/// Returns `Ok(true)` if an account was imported, `Ok(false)` if there was
/// nothing to import (no credentials file or already in pool).
pub fn import_from_single_credential() -> Result<bool, ApiError> {
    let creds = match crate::oauth::load_credentials()? {
        Some(c) => c,
        None => return Ok(false),
    };

    let mut pool = load_pool()?;

    // Skip if already imported.
    if pool
        .accounts
        .iter()
        .any(|a| a.email == creds.email)
    {
        return Ok(false);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let account = AntigravityAccount {
        email: creds.email,
        access_token: creds.access_token,
        refresh_token: creds.refresh_token,
        project_id: creds.project_id.unwrap_or_default(),
        enabled: true,
        added_at: now,
        rate_limited_until: None,
        expires_at: creds.expires_at,
    };

    pool.add_account(account);
    pool.save_pool()?;
    Ok(true)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_account(email: &str, enabled: bool) -> AntigravityAccount {
        AntigravityAccount {
            email: Some(email.to_string()),
            access_token: "test-token".to_string(),
            refresh_token: "test-refresh".to_string(),
            project_id: "test-project".to_string(),
            enabled,
            added_at: 1000,
            rate_limited_until: None,
            expires_at: 9_999_999_999,
        }
    }

    #[test]
    fn test_add_account() {
        let mut pool = AccountPool::default();
        pool.add_account(make_account("a@test.com", true));
        pool.add_account(make_account("b@test.com", true));
        assert_eq!(pool.accounts.len(), 2);
        assert_eq!(pool.accounts[0].email.as_deref(), Some("a@test.com"));
    }

    #[test]
    fn test_remove_account() {
        let mut pool = AccountPool::default();
        pool.add_account(make_account("a@test.com", true));
        pool.add_account(make_account("b@test.com", true));
        assert!(pool.remove_account("a@test.com"));
        assert_eq!(pool.accounts.len(), 1);
        assert_eq!(pool.accounts[0].email.as_deref(), Some("b@test.com"));
        assert!(!pool.remove_account("nonexistent@test.com"));
    }

    #[test]
    fn test_round_robin_rotation() {
        let mut pool = AccountPool::default();
        pool.add_account(make_account("a@test.com", true));
        pool.add_account(make_account("b@test.com", true));
        pool.add_account(make_account("c@test.com", true));

        let a = pool.next_available("gemini").unwrap();
        assert_eq!(a.email.as_deref(), Some("a@test.com"));

        let b = pool.next_available("gemini").unwrap();
        assert_eq!(b.email.as_deref(), Some("b@test.com"));

        let c = pool.next_available("gemini").unwrap();
        assert_eq!(c.email.as_deref(), Some("c@test.com"));

        // Wraps around.
        let a2 = pool.next_available("gemini").unwrap();
        assert_eq!(a2.email.as_deref(), Some("a@test.com"));
    }

    #[test]
    fn test_per_model_family_independent() {
        let mut pool = AccountPool::default();
        pool.add_account(make_account("a@test.com", true));
        pool.add_account(make_account("b@test.com", true));

        let a = pool.next_available("gemini").unwrap();
        assert_eq!(a.email.as_deref(), Some("a@test.com"));

        // Different model family starts from index 0.
        let a2 = pool.next_available("claude").unwrap();
        assert_eq!(a2.email.as_deref(), Some("a@test.com"));

        // Gemini continues from where it left off.
        let b = pool.next_available("gemini").unwrap();
        assert_eq!(b.email.as_deref(), Some("b@test.com"));
    }

    #[test]
    fn test_skip_disabled() {
        let mut pool = AccountPool::default();
        pool.add_account(make_account("a@test.com", false));
        pool.add_account(make_account("b@test.com", true));

        let b = pool.next_available("gemini").unwrap();
        assert_eq!(b.email.as_deref(), Some("b@test.com"));
    }

    #[test]
    fn test_all_rate_limited() {
        let mut pool = AccountPool::default();
        let mut acc = make_account("a@test.com", true);
        acc.rate_limited_until = Some(9_999_999_999);
        pool.add_account(acc);

        let result = pool.next_available("gemini");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("rate-limited"), "expected rate-limit message, got: {msg}");
    }

    #[test]
    fn test_skip_rate_limited() {
        let mut pool = AccountPool::default();
        let mut limited = make_account("a@test.com", true);
        limited.rate_limited_until = Some(9_999_999_999);
        pool.add_account(limited);
        pool.add_account(make_account("b@test.com", true));

        let b = pool.next_available("gemini").unwrap();
        assert_eq!(b.email.as_deref(), Some("b@test.com"));
    }

    #[test]
    fn test_mark_rate_limited() {
        let mut pool = AccountPool::default();
        pool.add_account(make_account("a@test.com", true));

        pool.mark_rate_limited("a@test.com", 60);
        assert!(
            pool.accounts[0].rate_limited_until.is_some(),
            "should have rate_limited_until set"
        );
    }

    #[test]
    fn test_update_account() {
        let mut pool = AccountPool::default();
        pool.add_account(make_account("a@test.com", true));

        pool.update_account("a@test.com", "new-token".to_string(), 12345);
        assert_eq!(pool.accounts[0].access_token, "new-token");
        assert_eq!(pool.accounts[0].expires_at, 12345);
    }

    #[test]
    fn test_empty_pool_error() {
        let mut pool = AccountPool::default();
        let result = pool.next_available("gemini");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("no accounts"), "expected 'no accounts', got: {msg}");
    }

    #[test]
    fn test_no_enabled_accounts() {
        let mut pool = AccountPool::default();
        pool.add_account(make_account("a@test.com", false));
        pool.add_account(make_account("b@test.com", false));

        let result = pool.next_available("gemini");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("no enabled"), "expected 'no enabled', got: {msg}");
    }

    #[test]
    fn test_list_accounts() {
        let mut pool = AccountPool::default();
        pool.add_account(make_account("a@test.com", true));
        pool.add_account(make_account("b@test.com", true));

        assert_eq!(pool.list_accounts().len(), 2);
    }

    #[test]
    fn test_default_pool() {
        let pool = AccountPool::default();
        assert!(pool.accounts.is_empty());
        assert_eq!(pool.active_index, 0);
        assert!(pool.last_rotation.is_empty());
    }
}
