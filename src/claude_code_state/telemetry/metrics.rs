//! Claude Code metrics client for sending telemetry to Anthropic

use serde::Serialize;
use std::time::Duration;
use tracing::debug;

use super::resource::ResourceAttributes;

const METRICS_ENDPOINT: &str = "https://api.anthropic.com/api/claude_code/metrics";
const METRICS_TIMEOUT_MS: u64 = 5000;

/// Claude Code tengu events
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TenguEvent {
    /// CLI initialization
    #[serde(rename = "tengu_init")]
    Init {
        entrypoint: String,
        #[serde(rename = "hasInitialPrompt")]
        has_initial_prompt: bool,
        #[serde(rename = "hasStdin")]
        has_stdin: bool,
        verbose: bool,
        debug: bool,
        #[serde(rename = "numAllowedTools")]
        num_allowed_tools: u32,
        #[serde(rename = "mcpClientCount")]
        mcp_client_count: u32,
    },

    /// Startup telemetry with system config
    #[serde(rename = "tengu_startup_telemetry")]
    StartupTelemetry {
        is_git: bool,
        worktree_count: u32,
        sandbox_enabled: bool,
    },

    /// API query made
    #[serde(rename = "tengu_api_query")]
    ApiQuery {
        model: String,
        #[serde(rename = "inputTokens")]
        input_tokens: u64,
        streaming: bool,
    },

    /// API success
    #[serde(rename = "tengu_api_success")]
    ApiSuccess {
        model: String,
        #[serde(rename = "inputTokens")]
        input_tokens: u64,
        #[serde(rename = "outputTokens")]
        output_tokens: u64,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// API error
    #[serde(rename = "tengu_api_error")]
    ApiError {
        model: String,
        #[serde(rename = "errorType")]
        error_type: String,
        #[serde(rename = "statusCode")]
        status_code: Option<u16>,
    },

    /// API retry
    #[serde(rename = "tengu_api_retry")]
    ApiRetry {
        model: String,
        attempt: u32,
        #[serde(rename = "errorType")]
        error_type: String,
    },

    /// Tool use success
    #[serde(rename = "tengu_tool_use_success")]
    ToolUseSuccess {
        #[serde(rename = "toolName")]
        tool_name: String,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// Tool use error
    #[serde(rename = "tengu_tool_use_error")]
    ToolUseError {
        #[serde(rename = "toolName")]
        tool_name: String,
        #[serde(rename = "errorType")]
        error_type: String,
    },

    /// Session exit
    #[serde(rename = "tengu_exit")]
    Exit {
        #[serde(rename = "totalInputTokens")]
        total_input_tokens: u64,
        #[serde(rename = "totalOutputTokens")]
        total_output_tokens: u64,
        #[serde(rename = "totalCost")]
        total_cost: f64,
        #[serde(rename = "durationSecs")]
        duration_secs: u64,
    },

    /// OAuth flow started
    #[serde(rename = "tengu_oauth_flow_started")]
    OAuthFlowStarted {
        provider: String,
    },

    /// OAuth flow completed
    #[serde(rename = "tengu_oauth_flow_completed")]
    OAuthFlowCompleted {
        provider: String,
        success: bool,
    },

    /// Input prompt submitted
    #[serde(rename = "tengu_input_prompt")]
    InputPrompt {
        #[serde(rename = "inputLength")]
        input_length: usize,
        #[serde(rename = "hasFiles")]
        has_files: bool,
        #[serde(rename = "hasTools")]
        has_tools: bool,
    },
}

/// Metrics payload sent to Anthropic
#[derive(Debug, Serialize)]
struct MetricsPayload<'a> {
    events: Vec<EventPayload<'a>>,
    resource: &'a ResourceAttributes,
}

/// Individual event in the payload
#[derive(Debug, Serialize)]
struct EventPayload<'a> {
    #[serde(flatten)]
    event: &'a TenguEvent,
    user_id: &'a str,
    session_id: &'a str,
    timestamp: i64,
}

/// Metrics client for sending telemetry
pub struct MetricsClient {
    client: wreq::Client,
}

impl MetricsClient {
    /// Create a new metrics client
    pub fn new() -> Self {
        let client = wreq::Client::builder()
            .timeout(Duration::from_millis(METRICS_TIMEOUT_MS))
            .build()
            .expect("Failed to create metrics client");

        Self { client }
    }

    /// Send an event to the metrics endpoint
    pub async fn send_event(
        &self,
        event: TenguEvent,
        user_id: &str,
        resource: &ResourceAttributes,
        access_token: Option<&str>,
    ) -> Result<(), MetricsError> {
        let session_id = user_id
            .split("_session_")
            .nth(1)
            .unwrap_or("unknown");

        let payload = MetricsPayload {
            events: vec![EventPayload {
                event: &event,
                user_id,
                session_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
            }],
            resource,
        };

        let mut request = self.client.post(METRICS_ENDPOINT).json(&payload);

        if let Some(token) = access_token {
            request = request.bearer_auth(token);
        }

        let response = request.send().await.map_err(|e| MetricsError::Network(e.to_string()))?;

        if !response.status().is_success() {
            debug!(
                "Metrics endpoint returned status {}: {:?}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
            // Don't treat non-2xx as fatal - telemetry should not block functionality
        }

        Ok(())
    }

    /// Send multiple events in a batch
    pub async fn send_batch(
        &self,
        events: Vec<(TenguEvent, String)>,
        resource: &ResourceAttributes,
        access_token: Option<&str>,
    ) -> Result<(), MetricsError> {
        if events.is_empty() {
            return Ok(());
        }

        let timestamp = chrono::Utc::now().timestamp_millis();
        let event_payloads: Vec<EventPayload> = events
            .iter()
            .map(|(event, user_id)| {
                let session_id = user_id
                    .split("_session_")
                    .nth(1)
                    .unwrap_or("unknown");
                EventPayload {
                    event,
                    user_id,
                    session_id,
                    timestamp,
                }
            })
            .collect();

        let payload = MetricsPayload {
            events: event_payloads,
            resource,
        };

        let mut request = self.client.post(METRICS_ENDPOINT).json(&payload);

        if let Some(token) = access_token {
            request = request.bearer_auth(token);
        }

        let response = request.send().await.map_err(|e| MetricsError::Network(e.to_string()))?;

        if !response.status().is_success() {
            debug!(
                "Metrics batch endpoint returned status {}",
                response.status()
            );
        }

        Ok(())
    }
}

impl Default for MetricsClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics errors
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
