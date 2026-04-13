//! 1st-party event logging to /api/event_logging/batch
//!
//! This emulates Claude Code's primary telemetry pipeline, sending
//! ClaudeCodeInternalEvent protobuf-compatible JSON to Anthropic's
//! event logging endpoint.
//!
//! Events are sent **unauthenticated** — the real Claude Code falls back
//! to unauthenticated POSTs when tokens are unavailable, and the endpoint
//! accepts them. Sending without auth avoids forwarding live OAuth tokens
//! to a telemetry endpoint.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::Utc;
use serde::Serialize;
use serde_json::{Value, json};
use tracing::debug;
use uuid::Uuid;

use crate::config::{CLAUDE_CODE_USER_AGENT, CLAUDE_CODE_VERSION};

/// 1P event logging endpoint
const EVENT_LOGGING_ENDPOINT: &str = "https://api.anthropic.com/api/event_logging/batch";
const EVENT_LOGGING_TIMEOUT_MS: u64 = 10_000;

fn is_env_truthy(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"))
}

/// Environment metadata matching the ClaudeCodeInternalEvent proto
#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentMetadata {
    pub platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_raw: Option<String>,
    pub arch: String,
    pub node_version: String,
    pub terminal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_managers: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtimes: Option<String>,
    pub is_running_with_bun: bool,
    pub is_ci: bool,
    pub is_claubbit: bool,
    pub is_claude_code_remote: bool,
    pub is_local_agent_mode: bool,
    pub is_conductor: bool,
    pub is_github_action: bool,
    pub is_claude_code_action: bool,
    pub is_claude_ai_auth: bool,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_base: Option<String>,
    pub build_time: String,
    pub deployment_environment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vcs: Option<String>,
}

impl EnvironmentMetadata {
    /// Build environment metadata from resource attributes
    pub fn from_resource(resource: &super::resource::ResourceAttributes) -> Self {
        Self {
            platform: resource.process_platform.clone(),
            platform_raw: Some(resource.process_platform.clone()),
            arch: resource.host_arch.clone(),
            node_version: "v22.13.1".to_string(),
            terminal: std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "unknown".to_string()),
            package_managers: None,
            runtimes: Some("node".to_string()),
            is_running_with_bun: false,
            is_ci: is_env_truthy("CI"),
            is_claubbit: false,
            is_claude_code_remote: is_env_truthy("CLAUDE_CODE_REMOTE"),
            is_local_agent_mode: false,
            is_conductor: false,
            is_github_action: is_env_truthy("GITHUB_ACTIONS"),
            is_claude_code_action: is_env_truthy("CLAUDE_CODE_ACTION"),
            is_claude_ai_auth: true,
            version: CLAUDE_CODE_VERSION.to_string(),
            version_base: Some(version_base()),
            build_time: build_time_from_version(),
            deployment_environment: "production".to_string(),
            vcs: Some("git".to_string()),
        }
    }
}

/// A single event in the batch payload
#[derive(Debug, Serialize)]
struct FirstPartyEvent {
    event_type: &'static str,
    event_data: Value,
}

/// The batch payload sent to /api/event_logging/batch
#[derive(Debug, Serialize)]
struct EventBatchPayload {
    events: Vec<FirstPartyEvent>,
}

/// Event-specific metadata that varies per event type
#[derive(Debug, Clone, Default)]
pub struct EventData {
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error_type: Option<String>,
    pub status_code: Option<u16>,
    pub tool_name: Option<String>,
    pub streaming: Option<bool>,
    pub total_cost: Option<f64>,
    pub attempt: Option<u32>,
}

/// Build a ClaudeCodeInternalEvent JSON structure
fn build_event(
    event_name: &str,
    session_id: &str,
    device_id: Option<&str>,
    email: Option<&str>,
    auth: Option<(&str, Option<&str>)>,
    env_metadata: &EnvironmentMetadata,
    model: &str,
    event_metadata: &serde_json::Map<String, Value>,
) -> Value {
    let now = Utc::now();
    let mut event = json!({
        "event_id": Uuid::new_v4().to_string(),
        "event_name": event_name,
        "client_timestamp": now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "session_id": session_id,
        "model": model,
        "user_type": "",
        "is_interactive": true,
        "client_type": "cli",
        "env": env_metadata,
    });

    if let Some(did) = device_id {
        event["device_id"] = json!(did);
    }
    if let Some(e) = email {
        event["email"] = json!(e);
    }
    if let Some((account_uuid, org_uuid)) = auth {
        let mut auth_obj = json!({ "account_uuid": account_uuid });
        if let Some(org) = org_uuid {
            auth_obj["organization_uuid"] = json!(org);
        }
        event["auth"] = auth_obj;
    }

    // Encode event-specific metadata into additional_metadata (base64 JSON)
    if !event_metadata.is_empty() {
        if let Ok(json_str) = serde_json::to_string(&event_metadata) {
            event["additional_metadata"] = json!(BASE64.encode(json_str.as_bytes()));
        }
    }

    event
}

/// Client for sending 1P events to the event logging endpoint
///
/// Events are always sent **unauthenticated** to avoid forwarding
/// live OAuth tokens to the telemetry endpoint.
pub struct EventLoggingClient {
    client: wreq::Client,
}

impl EventLoggingClient {
    pub fn new() -> Self {
        let client = wreq::Client::builder()
            .timeout(std::time::Duration::from_millis(EVENT_LOGGING_TIMEOUT_MS))
            .build()
            .expect("Failed to create event logging client");
        Self { client }
    }

    /// Send a single event to the 1P event logging endpoint (unauthenticated)
    pub async fn send_event(
        &self,
        event_name: &str,
        session_id: &str,
        device_id: Option<&str>,
        email: Option<&str>,
        auth: Option<(&str, Option<&str>)>,
        env_metadata: &EnvironmentMetadata,
        model: &str,
        event_data: &EventData,
    ) -> Result<(), EventLoggingError> {
        let mut metadata = serde_json::Map::new();
        if let Some(input) = event_data.input_tokens {
            metadata.insert("inputTokens".to_string(), json!(input));
        }
        if let Some(output) = event_data.output_tokens {
            metadata.insert("outputTokens".to_string(), json!(output));
        }
        if let Some(dur) = event_data.duration_ms {
            metadata.insert("durationMs".to_string(), json!(dur));
        }
        if let Some(ref err) = event_data.error_type {
            metadata.insert("errorType".to_string(), json!(err));
        }
        if let Some(code) = event_data.status_code {
            metadata.insert("statusCode".to_string(), json!(code));
        }
        if let Some(ref tool) = event_data.tool_name {
            metadata.insert("toolName".to_string(), json!(tool));
        }
        if let Some(streaming) = event_data.streaming {
            metadata.insert("streaming".to_string(), json!(streaming));
        }
        if let Some(cost) = event_data.total_cost {
            metadata.insert("totalCost".to_string(), json!(cost));
        }
        if let Some(attempt) = event_data.attempt {
            metadata.insert("attempt".to_string(), json!(attempt));
        }

        let event_json =
            build_event(event_name, session_id, device_id, email, auth, env_metadata, model, &metadata);

        let payload = EventBatchPayload {
            events: vec![FirstPartyEvent {
                event_type: "ClaudeCodeInternalEvent",
                event_data: event_json,
            }],
        };

        let response = self
            .client
            .post(EVENT_LOGGING_ENDPOINT)
            .header("Content-Type", "application/json")
            .header("User-Agent", CLAUDE_CODE_USER_AGENT)
            .header("x-service-name", "claude-code")
            .json(&payload)
            .send()
            .await
            .map_err(|e| EventLoggingError::Network(e.to_string()))?;

        if !response.status().is_success() {
            debug!(
                "1P event logging returned status {}: {}",
                response.status(),
                event_name,
            );
        }

        Ok(())
    }
}

impl Default for EventLoggingClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract base version: "2.1.76" -> "2.1.76", "2.0.36-dev.xxx" -> "2.0.36-dev"
fn version_base() -> String {
    let version = CLAUDE_CODE_VERSION;
    if let Some(idx) = version.find('-') {
        let after_dash = &version[idx + 1..];
        if let Some(dot_idx) = after_dash.find('.') {
            return version[..idx + 1 + dot_idx].to_string();
        }
    }
    version.to_string()
}

/// Derive a plausible build_time from the version constant.
/// For release versions like "2.1.76" we generate a date a few days before
/// the current date to avoid an obvious static timestamp.
fn build_time_from_version() -> String {
    // Use a deterministic offset from the version to produce a stable timestamp
    let hash: u32 = CLAUDE_CODE_VERSION
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let days_back = (hash % 7) as i64 + 1; // 1–7 days before "now" at init time
    let dt = Utc::now() - chrono::Duration::days(days_back);
    dt.format("%Y-%m-%dT00:00:00.000Z").to_string()
}

/// Event logging errors
#[derive(Debug, thiserror::Error)]
pub enum EventLoggingError {
    #[error("Network error: {0}")]
    Network(String),
}
