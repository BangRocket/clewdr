use std::time::Duration;

use chrono::{DateTime, Utc};
use oauth2::{EmptyExtraTokenFields, StandardTokenResponse, TokenResponse, basic::BasicTokenType};
use serde::{Deserialize, Serialize};
use serde_with::{DurationSeconds, TimestampSecondsWithFrac, serde_as};
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]

pub struct Organization {
    pub uuid: String,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenInfo {
    pub access_token: String,
    #[serde_as(as = "DurationSeconds")]
    pub expires_in: Duration,
    pub organization: Organization,
    pub refresh_token: String,
    #[serde_as(as = "TimestampSecondsWithFrac")]
    pub expires_at: DateTime<Utc>,
}

/// Default token expiry: 1 hour (Claude Code OAuth tokens typically last 1 hour)
const DEFAULT_TOKEN_EXPIRY_SECS: u64 = 3600;

impl TokenInfo {
    pub fn new(
        raw: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
        organization_uuid: String,
    ) -> Self {
        // Use provided expires_in, or default to 1 hour if not provided
        let expires_in_duration = raw
            .expires_in()
            .filter(|d| d.as_secs() > 0)
            .unwrap_or(Duration::from_secs(DEFAULT_TOKEN_EXPIRY_SECS));
        let expires_at = Utc::now() + expires_in_duration;
        tracing::info!(
            "TokenInfo created - expires_in: {:?}, expires_at: {}",
            expires_in_duration,
            expires_at.to_rfc3339()
        );
        Self {
            access_token: raw.access_token().secret().to_string(),
            expires_in: expires_in_duration,
            organization: Organization {
                uuid: organization_uuid,
            },
            refresh_token: raw
                .refresh_token()
                .map_or_else(Default::default, |rt| rt.secret().to_string()),
            expires_at,
        }
    }

    pub fn is_expired(&self) -> bool {
        debug!("Expires at: {}", self.expires_at.to_rfc3339());
        Utc::now() >= self.expires_at - Duration::from_secs(60 * 5) // 5 minutes
    }
}
