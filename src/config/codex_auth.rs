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
    pub fn id_prefix(&self) -> &str {
        &self.id[..self.id.len().min(8)]
    }

    pub fn is_dispatchable(&self, now: i64) -> bool {
        match &self.status {
            CodexAuthStatus::Valid | CodexAuthStatus::Expired => true,
            CodexAuthStatus::RateLimited { until } => *until <= now,
            CodexAuthStatus::Invalid | CodexAuthStatus::Banned => false,
        }
    }
}
