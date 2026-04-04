//! Claude Code telemetry emulation
//!
//! This module emulates Claude Code's 1st-party event logging pipeline
//! to make clewdr traffic patterns match an authentic Claude Code client.
//!
//! Based on analysis of Claude Code source (services/analytics/).
//!
//! Primary channel: POST /api/event_logging/batch
//!   Format: ClaudeCodeInternalEvent protobuf-compatible JSON
//!   Headers: User-Agent, x-service-name (no Authorization — sent unauthenticated)

pub mod events;
pub mod resource;

pub use events::{EventData, EventLoggingClient, EnvironmentMetadata};
pub use resource::ResourceAttributes;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

use crate::config::CLAUDE_CODE_VERSION;

/// Global telemetry state for Claude Code emulation
static CC_TELEMETRY: std::sync::OnceLock<Arc<ClaudeCodeTelemetry>> = std::sync::OnceLock::new();

/// Claude Code telemetry manager
///
/// Manages session state and provides fire-and-forget event tracking
/// that mirrors authentic Claude Code telemetry behavior.
pub struct ClaudeCodeTelemetry {
    pub event_client: EventLoggingClient,
    pub resource: ResourceAttributes,
    pub env_metadata: EnvironmentMetadata,
    pub session_id: String,
    pub anonymous_id: String,
    enabled: AtomicBool,
}

impl ClaudeCodeTelemetry {
    /// Create new telemetry instance
    pub fn new(enabled: bool) -> Self {
        let resource = ResourceAttributes::collect(CLAUDE_CODE_VERSION);
        let env_metadata = EnvironmentMetadata::from_resource(&resource);
        Self {
            event_client: EventLoggingClient::new(),
            resource,
            env_metadata,
            session_id: Uuid::new_v4().to_string(),
            anonymous_id: Uuid::new_v4().to_string(),
            enabled: AtomicBool::new(enabled),
        }
    }

    /// Check if telemetry is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Set telemetry enabled state
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Build the composite user_id used in telemetry
    /// Format: user_{anonymousId}_account_{accountUuid}_session_{sessionId}
    pub fn build_user_id(&self, account_uuid: Option<&str>) -> String {
        match account_uuid {
            Some(uuid) => format!(
                "user_{}_account_{}_session_{}",
                self.anonymous_id, uuid, self.session_id
            ),
            None => format!("user_{}_session_{}", self.anonymous_id, self.session_id),
        }
    }

    /// Track a telemetry event (fire-and-forget, unauthenticated)
    ///
    /// Sends to the 1P event logging endpoint (/api/event_logging/batch).
    /// Failures are logged at debug level and silently dropped.
    pub async fn track_event(
        &self,
        event_name: &str,
        model: &str,
        event_data: EventData,
        account_uuid: Option<&str>,
        organization_uuid: Option<&str>,
        email: Option<&str>,
    ) {
        if !self.is_enabled() {
            return;
        }

        let device_id = self.build_user_id(account_uuid);
        let auth = account_uuid.map(|a| (a, organization_uuid));

        if let Err(e) = self
            .event_client
            .send_event(
                event_name,
                &self.session_id,
                Some(&device_id),
                email,
                auth,
                &self.env_metadata,
                model,
                &event_data,
            )
            .await
        {
            tracing::debug!("Failed to send telemetry event '{}': {}", event_name, e);
        }
    }
}

/// Initialize the global Claude Code telemetry
pub fn init_telemetry(enabled: bool) -> Arc<ClaudeCodeTelemetry> {
    let telemetry = Arc::new(ClaudeCodeTelemetry::new(enabled));
    let _ = CC_TELEMETRY.set(telemetry.clone());

    if enabled {
        tracing::info!("Claude Code telemetry emulation enabled");
    }

    telemetry
}

/// Get the global Claude Code telemetry instance
pub fn get_telemetry() -> Option<Arc<ClaudeCodeTelemetry>> {
    CC_TELEMETRY.get().cloned()
}

/// Track an event using the global telemetry instance (fire-and-forget)
///
/// Spawns a background task so the caller is never blocked.
/// Events are sent unauthenticated — no OAuth tokens are forwarded.
pub fn track(
    event_name: &'static str,
    model: String,
    event_data: EventData,
    account_uuid: Option<String>,
    organization_uuid: Option<String>,
    email: Option<String>,
) {
    if let Some(telemetry) = get_telemetry() {
        tokio::spawn(async move {
            telemetry
                .track_event(
                    event_name,
                    &model,
                    event_data,
                    account_uuid.as_deref(),
                    organization_uuid.as_deref(),
                    email.as_deref(),
                )
                .await;
        });
    }
}
