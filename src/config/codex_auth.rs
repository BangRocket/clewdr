use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

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
