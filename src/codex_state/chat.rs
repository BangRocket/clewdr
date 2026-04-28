use axum::{
    body::Body,
    response::{IntoResponse, Response},
};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use http::StatusCode;
use snafu::{GenerateImplicitData, Location, ResultExt};
use tracing::{info, warn};
use uuid::Uuid;
use wreq::Method;

use std::sync::{Arc, Mutex};

use crate::{
    codex_state::{
        CodexState,
        transform::{
            CodexStreamState, aggregate_codex_events, translate_oai_request_to_codex,
        },
    },
    config::{CLEWDR_CONFIG, CodexAuthStatus},
    error::{ClewdrError, WreqSnafu},
    types::codex::CodexSseEvent,
};

const CODEX_ORIGINATOR: &str = "codex_cli_rs";
const CODEX_OPENAI_BETA: &str = "responses=experimental";
// Match a recent Codex CLI release. Codex backend gates new models on this
// header (e.g. gpt-5.5 requires >= 0.30 at time of writing). Bump when
// shipping support for newer models. Latest release: github.com/openai/codex.
const CODEX_CLI_VERSION: &str = "0.125.0";

impl CodexState {
    pub async fn try_chat(
        &mut self,
        request: serde_json::Value,
    ) -> Result<Response, ClewdrError> {
        // Translate OAI -> Codex Responses request. The handler hands us raw
        // JSON because Claude's `Message` enum rejects valid OAI shapes
        // (assistant.tool_calls without content; role: "tool").
        let codex_body = translate_oai_request_to_codex(&request).map_err(|e| {
            ClewdrError::CodexError {
                loc: Location::generate(),
                msg: format!("codex translate: {e}"),
            }
        })?;

        // Convert to JSON Value so we can apply Codex-specific sanitization.
        let mut value =
            serde_json::to_value(&codex_body).map_err(|e| ClewdrError::CodexError {
                loc: Location::generate(),
                msg: format!("serialize codex request: {e}"),
            })?;
        sanitize_codex_body(&mut value);

        let model = codex_body.model.clone();
        let client_wants_stream = request
            .get("stream")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        // Track whether the client wants streaming output; upstream is always streamed.
        self.stream = client_wants_stream;

        // Single source UUIDs per request — reused across retries.
        let session_id = Uuid::new_v4().to_string();
        let conversation_id = Uuid::new_v4().to_string();

        let mut last_err: Option<ClewdrError> = None;
        for attempt in 0..CLEWDR_CONFIG.load().max_retries + 1 {
            let auth = self.request_auth().await?;
            info!(
                "[REQ] codex stream={} model={} cred={} attempt={}",
                client_wants_stream,
                model,
                auth.id_prefix(),
                attempt + 1,
            );

            // Refresh if the access token is near expiry.
            if let Err(e) = self.ensure_fresh_access_token().await {
                warn!("codex refresh failed (attempt {}): {}", attempt + 1, e);
                self.return_auth().await;
                last_err = Some(e);
                continue;
            }

            let url = format!("{}/responses", self.api_base.trim_end_matches('/'));
            let resp = self
                .build_request(Method::POST, &url)?
                .header("session_id", session_id.as_str())
                .header("conversation_id", conversation_id.as_str())
                .header("originator", CODEX_ORIGINATOR)
                .header("openai-beta", CODEX_OPENAI_BETA)
                .header("version", CODEX_CLI_VERSION)
                .json(&value)
                .send()
                .await
                .context(WreqSnafu {
                    msg: "codex POST /responses",
                });

            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    warn!("codex network error (attempt {}): {}", attempt + 1, e);
                    self.return_auth().await;
                    last_err = Some(e);
                    continue;
                }
            };

            let status = resp.status();
            if status.is_success() {
                let response = if client_wants_stream {
                    self.stream_to_oai(resp, model.clone()).await?
                } else {
                    self.aggregate_to_oai(resp, model.clone()).await?
                };
                self.return_auth().await;
                return Ok(response);
            }

            // Non-success — classify, peek at body for diagnostics, decide
            // whether to retry or fail-fast.
            self.classify_and_mark(status, &resp);
            let body_snippet = read_body_snippet(resp).await;
            warn!(
                "codex upstream {} (attempt {}): {}",
                status.as_u16(),
                attempt + 1,
                body_snippet
            );
            self.return_auth().await;
            last_err = Some(ClewdrError::CodexError {
                loc: Location::generate(),
                msg: format!("codex upstream {}: {body_snippet}", status.as_u16()),
            });

            // 4xx other than auth/rate-limit means the request itself is bad
            // (unsupported model, bad params, etc). Retrying with another cred
            // won't help — fail fast with the body the user can act on.
            let code = status.as_u16();
            let retryable_4xx = matches!(code, 401 | 403 | 429);
            if (400..500).contains(&code) && !retryable_4xx {
                break;
            }
        }

        Err(last_err.unwrap_or(ClewdrError::CodexError {
            loc: Location::generate(),
            msg: "codex retry budget exhausted with no error".to_string(),
        }))
    }

    fn classify_and_mark(&mut self, status: StatusCode, resp: &wreq::Response) {
        let new_status = match status.as_u16() {
            401 => CodexAuthStatus::Invalid,
            403 => CodexAuthStatus::Banned,
            429 => {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(60);
                CodexAuthStatus::RateLimited {
                    until: chrono::Utc::now().timestamp() + retry_after,
                }
            }
            _ => return, // 5xx and others: just rotate without status change
        };
        if let Some(auth) = self.auth.as_mut() {
            auth.status = new_status;
        }
    }

    async fn stream_to_oai(
        &self,
        resp: wreq::Response,
        model: String,
    ) -> Result<Response, ClewdrError> {
        let id = format!("chatcmpl-{}", Uuid::new_v4());
        let created = chrono::Utc::now().timestamp();
        let stream = resp.bytes_stream().eventsource();
        let id_clone = id.clone();
        // Stateful translator: tracks output_index -> tool_call index across
        // events. Wrapped in Arc<Mutex<_>> so the per-event closure can mutate
        // it without breaking Send bounds on the resulting stream.
        let state = Arc::new(Mutex::new(CodexStreamState::new()));
        let mapped = stream.flat_map(move |evt| {
            let id = id_clone.clone();
            let model = model.clone();
            let state = Arc::clone(&state);
            let chunks: Vec<Result<String, std::io::Error>> = (|| {
                let evt = match evt {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::debug!(target: "codex::sse_raw", "stream error: {e}");
                        return Vec::new();
                    }
                };
                tracing::debug!(
                    target: "codex::sse_raw",
                    "event={} data={}",
                    if evt.event.is_empty() { "<unset>" } else { &evt.event },
                    truncate_for_log(&evt.data, 256)
                );
                let parsed: CodexSseEvent = match serde_json::from_str(&evt.data) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::debug!(
                            target: "codex::sse_raw",
                            "failed to parse codex event: {e}; data={}",
                            truncate_for_log(&evt.data, 256)
                        );
                        return Vec::new();
                    }
                };
                let mut guard = state.lock().expect("codex stream state mutex poisoned");
                let oai_chunks = guard.handle_event(&parsed, &id, &model, created);
                drop(guard);
                oai_chunks
                    .into_iter()
                    .filter_map(|c| serde_json::to_string(&c).ok())
                    .map(|json| Ok::<_, std::io::Error>(format!("data: {json}\n\n")))
                    .collect()
            })();
            futures::stream::iter(chunks)
        });

        // Append SSE [DONE] terminator.
        let done = futures::stream::once(async {
            Ok::<_, std::io::Error>("data: [DONE]\n\n".to_string())
        });
        let combined = mapped.chain(done);

        let body = Body::from_stream(combined);
        let mut response = Response::new(body);
        response
            .headers_mut()
            .insert("content-type", "text/event-stream".parse().unwrap());
        Ok(response)
    }

    async fn aggregate_to_oai(
        &self,
        resp: wreq::Response,
        model: String,
    ) -> Result<Response, ClewdrError> {
        let bytes = resp.bytes().await.context(WreqSnafu {
            msg: "codex body",
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|_| ClewdrError::CodexError {
            loc: Location::generate(),
            msg: "non-utf8 codex body".to_string(),
        })?;
        let mut events: Vec<CodexSseEvent> = Vec::new();
        for chunk in text.split("\n\n") {
            let mut event_name: Option<&str> = None;
            for line in chunk.lines() {
                if let Some(name) = line.strip_prefix("event: ") {
                    event_name = Some(name.trim());
                    continue;
                }
                if let Some(data) = line.strip_prefix("data: ") {
                    let trimmed = data.trim();
                    if trimmed == "[DONE]" {
                        continue;
                    }
                    tracing::debug!(
                        target: "codex::sse_raw",
                        "event={} data={}",
                        event_name.unwrap_or("<unset>"),
                        truncate_for_log(trimmed, 256)
                    );
                    match serde_json::from_str::<CodexSseEvent>(trimmed) {
                        Ok(ev) => events.push(ev),
                        Err(e) => tracing::debug!(
                            target: "codex::sse_raw",
                            "failed to parse codex event: {e}; data={}",
                            truncate_for_log(trimmed, 256)
                        ),
                    }
                }
            }
        }
        let id = format!("chatcmpl-{}", Uuid::new_v4());
        let oai = aggregate_codex_events(&events, &id, &model).map_err(|e| {
            ClewdrError::CodexError {
                loc: Location::generate(),
                msg: format!("aggregate: {e}"),
            }
        })?;
        let body = serde_json::to_vec(&oai).unwrap();
        Ok((
            StatusCode::OK,
            [(http::header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response())
    }
}

fn sanitize_codex_body(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    obj.insert("stream".to_string(), serde_json::Value::Bool(true));
    obj.insert("store".to_string(), serde_json::Value::Bool(false));
    for k in [
        "max_output_tokens",
        "max_completion_tokens",
        "max_tokens",
        "temperature",
        "metadata",
    ] {
        obj.remove(k);
    }
    if let Some(input) = obj.get_mut("input").and_then(|v| v.as_array_mut()) {
        input.retain(|item| {
            item.as_object()
                .and_then(|o| o.get("type"))
                .and_then(|t| t.as_str())
                .map(|t| t != "item_reference")
                .unwrap_or(true)
        });
    }
}

/// Truncate a string for log output, replacing newlines with spaces and
/// appending a single ellipsis when truncated.
fn truncate_for_log(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect::<String>().replace(['\n', '\r'], " ");
    if s.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// Drain a non-success response body and return a short, human-readable
/// snippet for logs and error messages. Caps at 512 bytes; replaces newlines
/// with spaces so it fits one line. Falls back to a placeholder on read error.
async fn read_body_snippet(resp: wreq::Response) -> String {
    match resp.bytes().await {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let trimmed = text.trim();
            let mut snippet: String = trimmed.chars().take(512).collect();
            if trimmed.chars().count() > 512 {
                snippet.push('…');
            }
            snippet.replace(['\n', '\r'], " ")
        }
        Err(e) => format!("<failed to read body: {e}>"),
    }
}
