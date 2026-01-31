//! Claude Code telemetry emulation
//!
//! This module sends telemetry to Anthropic's metrics endpoint to appear
//! as an authentic Claude Code client. Based on analysis of Claude Code v2.1.15.

mod metrics;
mod resource;

pub use metrics::{MetricsClient, TenguEvent};
pub use resource::ResourceAttributes;

use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Global telemetry state for Claude Code emulation
static CC_TELEMETRY: std::sync::OnceLock<Arc<ClaudeCodeTelemetry>> = std::sync::OnceLock::new();

/// Claude Code telemetry manager
pub struct ClaudeCodeTelemetry {
    pub metrics_client: MetricsClient,
    pub resource: ResourceAttributes,
    pub session_id: String,
    pub anonymous_id: String,
    enabled: RwLock<bool>,
}

impl ClaudeCodeTelemetry {
    /// Create new telemetry instance
    pub fn new(enabled: bool) -> Self {
        Self {
            metrics_client: MetricsClient::new(),
            resource: ResourceAttributes::collect(),
            session_id: Uuid::new_v4().to_string(),
            anonymous_id: Uuid::new_v4().to_string(),
            enabled: RwLock::new(enabled),
        }
    }

    /// Check if telemetry is enabled
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.read().await
    }

    /// Set telemetry enabled state
    pub async fn set_enabled(&self, enabled: bool) {
        *self.enabled.write().await = enabled;
    }

    /// Build the composite user_id used in telemetry
    pub fn build_user_id(&self, account_uuid: Option<&str>) -> String {
        match account_uuid {
            Some(uuid) => format!(
                "user_{}_account_{}_session_{}",
                self.anonymous_id, uuid, self.session_id
            ),
            None => format!("user_{}_session_{}", self.anonymous_id, self.session_id),
        }
    }

    /// Track a tengu event
    pub async fn track_event(&self, event: TenguEvent, account_uuid: Option<&str>, access_token: Option<&str>) {
        if !self.is_enabled().await {
            return;
        }

        let user_id = self.build_user_id(account_uuid);
        if let Err(e) = self
            .metrics_client
            .send_event(event, &user_id, &self.resource, access_token)
            .await
        {
            tracing::debug!("Failed to send Claude Code telemetry: {}", e);
        }
    }
}

/// Initialize the global Claude Code telemetry
pub fn init_cc_telemetry(enabled: bool) -> Arc<ClaudeCodeTelemetry> {
    let telemetry = Arc::new(ClaudeCodeTelemetry::new(enabled));
    let _ = CC_TELEMETRY.set(telemetry.clone());

    if enabled {
        tracing::info!("Claude Code telemetry emulation enabled");
    }

    telemetry
}

/// Get the global Claude Code telemetry instance
pub fn get_cc_telemetry() -> Option<Arc<ClaudeCodeTelemetry>> {
    CC_TELEMETRY.get().cloned()
}

/// Track an event using the global telemetry instance
pub async fn track(event: TenguEvent, account_uuid: Option<&str>, access_token: Option<&str>) {
    if let Some(telemetry) = get_cc_telemetry() {
        telemetry.track_event(event, account_uuid, access_token).await;
    }
}
