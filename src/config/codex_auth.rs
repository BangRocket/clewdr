use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CodexAuth {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: i64,
    pub account_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(default)]
    pub status: CodexAuthStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CodexAuthStatus {
    Valid,
    RateLimited { until: i64 },
    Expired,
    Invalid,
    Banned,
}

impl Default for CodexAuthStatus {
    fn default() -> Self {
        CodexAuthStatus::Valid
    }
}

impl CodexAuth {
    pub fn id_prefix(&self) -> String {
        self.id.chars().take(8).collect()
    }

    pub fn is_dispatchable(&self, now: i64) -> bool {
        match &self.status {
            CodexAuthStatus::Valid | CodexAuthStatus::Expired => true,
            CodexAuthStatus::RateLimited { until } => *until <= now,
            CodexAuthStatus::Invalid | CodexAuthStatus::Banned => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodexIdTokenClaims {
    pub account_id: String,
    pub plan: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CodexJwtError {
    #[error("expected 3 segments, got {0}")]
    Segments(usize),
    #[error("base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("json parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing chatgpt_account_id claim")]
    MissingAccountId,
}

pub fn decode_codex_id_token_claims(token: &str) -> Result<CodexIdTokenClaims, CodexJwtError> {
    let segments: Vec<&str> = token.split('.').collect();
    if segments.len() != 3 {
        return Err(CodexJwtError::Segments(segments.len()));
    }
    let payload_bytes = URL_SAFE_NO_PAD.decode(segments[1])?;
    let v: serde_json::Value = serde_json::from_slice(&payload_bytes)?;

    let auth_obj = v
        .get("https://api.openai.com/auth")
        .and_then(|x| x.as_object());
    let account_id = auth_obj
        .and_then(|m| m.get("chatgpt_account_id"))
        .and_then(|x| x.as_str())
        .ok_or(CodexJwtError::MissingAccountId)?
        .to_string();
    let plan = auth_obj
        .and_then(|m| m.get("chatgpt_plan_type"))
        .and_then(|x| x.as_str())
        .map(str::to_string);

    Ok(CodexIdTokenClaims { account_id, plan })
}

#[derive(Debug, thiserror::Error)]
pub enum CodexAuthParseError {
    #[error("auth.json malformed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing tokens.{field}")]
    MissingField { field: &'static str },
    #[error("id_token claims invalid: {0}")]
    Claims(#[from] CodexJwtError),
}

impl CodexAuth {
    pub fn from_auth_json(body: &str, label: Option<String>) -> Result<Self, CodexAuthParseError> {
        let v: serde_json::Value = serde_json::from_str(body)?;
        let tokens = v
            .get("tokens")
            .ok_or(CodexAuthParseError::MissingField { field: "tokens" })?;

        let get = |k: &'static str| -> Result<String, CodexAuthParseError> {
            tokens
                .get(k)
                .and_then(|x| x.as_str())
                .map(str::to_string)
                .ok_or(CodexAuthParseError::MissingField { field: k })
        };
        let id_token = get("id_token")?;
        let access_token = get("access_token")?;
        let refresh_token = get("refresh_token")?;

        let claims = decode_codex_id_token_claims(&id_token)?;
        let account_id = tokens
            .get("account_id")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or(claims.account_id);

        // Stable id = first 16 hex chars of sha256(refresh_token)
        let mut hasher = Sha256::new();
        hasher.update(refresh_token.as_bytes());
        let id = hex_first_n(&hasher.finalize(), 16);

        // Conservative: assume access_token already nearly expired so first request triggers refresh.
        let access_expires_at = 0;

        Ok(CodexAuth {
            id,
            label,
            id_token,
            access_token,
            refresh_token,
            access_expires_at,
            account_id,
            plan: claims.plan,
            status: CodexAuthStatus::Valid,
            last_used_at: None,
        })
    }
}

fn hex_first_n(bytes: &[u8], n: usize) -> String {
    let mut s = String::with_capacity(n);
    for b in bytes {
        if s.len() >= n {
            break;
        }
        s.push_str(&format!("{:02x}", b));
    }
    s.truncate(n);
    s
}
