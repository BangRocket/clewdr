use crate::types::claude::{ContentBlock, Message, MessageContent, Role, Tool};
use crate::types::codex::{CodexContent, CodexInputItem, CodexRequest, CodexSseEvent};
use crate::types::oai::CreateMessageParams;
use serde::Serialize;
use snafu::Snafu;
use tracing::warn;

/// Models advertised on `/codex/v1/models`. The translator no longer enforces
/// this list — clients may send any model name and upstream Codex will reject
/// unsupported ones. Keeping the list as advertising/documentation only.
pub const CODEX_MODELS: &[&str] = &[
    "gpt-5.5",
    "gpt-5-codex",
    "gpt-5.3-codex",
    "gpt-5.2-codex",
    "gpt-5",
    "gpt-4.1",
    "o3",
    "o4-mini",
];

#[derive(Debug, Snafu)]
pub enum TranslateError {
    #[snafu(display("messages array is empty"))]
    NoMessages,
    #[snafu(display("upstream codex error: {message}{}", code.as_deref().map(|c| format!(" ({c})")).unwrap_or_default()))]
    UpstreamError {
        message: String,
        code: Option<String>,
    },
    #[snafu(display("codex stream ended without Completed event (truncated response)"))]
    IncompleteStream,
}

fn extract_text(msg: &Message) -> String {
    match &msg.content {
        MessageContent::Text { content } => content.clone(),
        MessageContent::Blocks { content } => content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text, .. } => Some(text.clone()),
                _ => {
                    warn!("dropping non-text content block in codex translation");
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

pub fn translate_chat_completions_to_codex(
    req: &CreateMessageParams,
) -> Result<CodexRequest, TranslateError> {
    if req.messages.is_empty() {
        return Err(TranslateError::NoMessages);
    }

    let mut system_parts: Vec<String> = Vec::new();
    let mut input: Vec<CodexInputItem> = Vec::new();
    for m in &req.messages {
        let text = extract_text(m);
        match m.role {
            Role::System => system_parts.push(text),
            Role::User => input.push(CodexInputItem::Message {
                role: "user".to_string(),
                content: vec![CodexContent::InputText { text }],
            }),
            Role::Assistant => input.push(CodexInputItem::Message {
                role: "assistant".to_string(),
                content: vec![CodexContent::OutputText { text }],
            }),
        }
    }

    // Codex backend rejects requests without an `instructions` field
    // (responds with `{"detail":"Instructions are required"}`). Use a generic
    // default when the client doesn't supply a system message.
    let instructions = if system_parts.is_empty() {
        Some("You are a helpful assistant.".to_string())
    } else {
        Some(system_parts.join("\n\n"))
    };

    // Tools: normalize Chat Completions function tools to Responses-style tools.
    // Drop Known (Anthropic-specific) tools.
    let tools: Vec<serde_json::Value> = req
        .tools
        .as_ref()
        .map(|ts| {
            ts.iter()
                .filter_map(|t| match t {
                    Tool::Known(_) => {
                        warn!("dropping Anthropic-specific tool in codex translation");
                        None
                    }
                    Tool::Custom(_) | Tool::Raw(_) => {
                        serde_json::to_value(t).ok().and_then(normalize_codex_tool)
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let tool_choice = req
        .tool_choice
        .as_ref()
        .and_then(|tc| serde_json::to_value(tc).ok());

    let max_output_tokens = req.max_tokens.or(req.max_completion_tokens);

    Ok(CodexRequest {
        model: req.model.clone(),
        input,
        instructions,
        temperature: req.temperature,
        top_p: req.top_p,
        max_output_tokens,
        tools,
        tool_choice,
        text: None,
        stream: req.stream.unwrap_or(false),
    })
}

fn normalize_codex_tool(mut tool: serde_json::Value) -> Option<serde_json::Value> {
    let Some(obj) = tool.as_object_mut() else {
        warn!("dropping non-object tool in codex translation");
        return None;
    };

    if obj.get("name").and_then(|v| v.as_str()).is_some() {
        if let Some(input_schema) = obj.remove("input_schema") {
            obj.entry("parameters".to_string()).or_insert(input_schema);
        }
        return Some(tool);
    }

    let Some(function) = obj.remove("function") else {
        warn!("dropping tool without top-level name in codex translation");
        return None;
    };

    let Some(function_obj) = function.as_object() else {
        warn!("dropping function tool with non-object function payload in codex translation");
        return None;
    };

    let Some(name) = function_obj.get("name").cloned() else {
        warn!("dropping function tool without function.name in codex translation");
        return None;
    };

    obj.insert(
        "type".to_string(),
        serde_json::Value::String("function".to_string()),
    );
    obj.insert("name".to_string(), name);
    for key in ["description", "parameters", "strict"] {
        if let Some(value) = function_obj.get(key).cloned() {
            obj.insert(key.to_string(), value);
        }
    }
    obj.entry("parameters".to_string())
        .or_insert_with(|| serde_json::json!({}));

    Some(tool)
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiChunk {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OaiChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OaiUsage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiChoice {
    pub index: u32,
    pub delta: OaiDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct OaiDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OaiToolCallDelta>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiToolCallDelta {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<OaiToolCallFunctionDelta>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct OaiToolCallFunctionDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// Per-tool-call tracking for stream translation. We keep enough state per
/// Codex `output_index` to know whether we already surfaced the tool call's
/// name and arguments to the OAI client — required to handle the case where
/// `response.output_item.done` carries the entire tool call without prior
/// argument deltas (and conversely, to suppress duplicate emission when the
/// deltas already streamed it).
#[derive(Debug, Default, Clone)]
struct ToolCallTracking {
    /// Index assigned in OAI `tool_calls[i].index` order (function-calls only,
    /// skipping reasoning items that share the same `output_index` space).
    oai_index: u32,
    name_emitted: bool,
    args_emitted: bool,
}

/// State carried across Codex SSE events while translating a single response
/// stream into OAI chat-completion chunks. Tracks the mapping from Codex
/// `output_index` to OAI `tool_calls[i].index`, since reasoning items consume
/// `output_index` slots but are not function calls.
#[derive(Debug, Default)]
pub struct CodexStreamState {
    tool_calls: std::collections::HashMap<u32, ToolCallTracking>,
    next_tool_index: u32,
    saw_tool_call: bool,
}

impl CodexStreamState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Translate a single Codex SSE event into 0..N OAI chat-completion chunks.
    pub fn handle_event(
        &mut self,
        event: &CodexSseEvent,
        id: &str,
        model: &str,
        created: i64,
    ) -> Vec<OaiChunk> {
        match event {
            CodexSseEvent::OutputTextDelta { delta }
            | CodexSseEvent::ReasoningTextDelta { delta }
            | CodexSseEvent::ReasoningSummaryTextDelta { delta } => vec![OaiChunk {
                id: id.to_string(),
                object: "chat.completion.chunk",
                created,
                model: model.to_string(),
                choices: vec![OaiChoice {
                    index: 0,
                    delta: OaiDelta {
                        content: Some(delta.clone()),
                        ..Default::default()
                    },
                    finish_reason: None,
                }],
                usage: None,
            }],

            CodexSseEvent::OutputItemAdded { item, output_index } => {
                let Some(obj) = item.as_object() else {
                    return Vec::new();
                };
                let item_type = obj.get("type").and_then(|v| v.as_str());
                if item_type != Some("function_call") {
                    // Reasoning items and other non-function output items are dropped.
                    return Vec::new();
                }
                let call_id = obj
                    .get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let name = obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                let tc_index = self.allocate_tool_index(*output_index);
                self.saw_tool_call = true;
                if let Some(state) = self.tool_calls.get_mut(output_index) {
                    if !name.is_empty() {
                        state.name_emitted = true;
                    }
                }

                vec![OaiChunk {
                    id: id.to_string(),
                    object: "chat.completion.chunk",
                    created,
                    model: model.to_string(),
                    choices: vec![OaiChoice {
                        index: 0,
                        delta: OaiDelta {
                            tool_calls: Some(vec![OaiToolCallDelta {
                                index: tc_index,
                                id: Some(call_id),
                                type_: Some("function".to_string()),
                                function: Some(OaiToolCallFunctionDelta {
                                    name: Some(name),
                                    arguments: Some(String::new()),
                                }),
                            }]),
                            ..Default::default()
                        },
                        finish_reason: None,
                    }],
                    usage: None,
                }]
            }

            CodexSseEvent::FunctionCallArgumentsDelta {
                delta,
                output_index,
                ..
            } => {
                let Some(state) = self.tool_calls.get_mut(output_index) else {
                    // Argument delta arrived before/without an OutputItemAdded —
                    // can't correlate to an OAI tool_call index. Drop.
                    warn!(
                        "codex function_call_arguments.delta with no tracked output_index={}",
                        output_index
                    );
                    return Vec::new();
                };
                let tc_index = state.oai_index;
                if !delta.is_empty() {
                    state.args_emitted = true;
                }
                vec![OaiChunk {
                    id: id.to_string(),
                    object: "chat.completion.chunk",
                    created,
                    model: model.to_string(),
                    choices: vec![OaiChoice {
                        index: 0,
                        delta: OaiDelta {
                            tool_calls: Some(vec![OaiToolCallDelta {
                                index: tc_index,
                                id: None,
                                type_: None,
                                function: Some(OaiToolCallFunctionDelta {
                                    name: None,
                                    arguments: Some(delta.clone()),
                                }),
                            }]),
                            ..Default::default()
                        },
                        finish_reason: None,
                    }],
                    usage: None,
                }]
            }

            // The arguments are already streamed via deltas; the .done event
            // only confirms the final string. Don't double-emit.
            CodexSseEvent::FunctionCallArgumentsDone { .. } => Vec::new(),

            CodexSseEvent::OutputItemDone { item, output_index } => {
                let Some(obj) = item.as_object() else {
                    return Vec::new();
                };
                let item_type = obj.get("type").and_then(|v| v.as_str());
                if item_type != Some("function_call") {
                    // Reasoning items: text already streamed via reasoning
                    // deltas; nothing to surface here.
                    return Vec::new();
                }

                let call_id = obj
                    .get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let name = obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let arguments = obj
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                let tc_index = self.allocate_tool_index(*output_index);
                self.saw_tool_call = true;

                let state = self
                    .tool_calls
                    .get_mut(output_index)
                    .expect("just allocated");

                let needs_name = !name.is_empty() && !state.name_emitted;
                let needs_args = !arguments.is_empty() && !state.args_emitted;

                if !needs_name && !needs_args {
                    return Vec::new();
                }

                let mut function = OaiToolCallFunctionDelta::default();
                if needs_name {
                    function.name = Some(name.clone());
                    state.name_emitted = true;
                }
                if needs_args {
                    function.arguments = Some(arguments);
                    state.args_emitted = true;
                }

                vec![OaiChunk {
                    id: id.to_string(),
                    object: "chat.completion.chunk",
                    created,
                    model: model.to_string(),
                    choices: vec![OaiChoice {
                        index: 0,
                        delta: OaiDelta {
                            tool_calls: Some(vec![OaiToolCallDelta {
                                index: tc_index,
                                id: if call_id.is_empty() {
                                    None
                                } else {
                                    Some(call_id)
                                },
                                type_: Some("function".to_string()),
                                function: Some(function),
                            }]),
                            ..Default::default()
                        },
                        finish_reason: None,
                    }],
                    usage: None,
                }]
            }

            CodexSseEvent::Completed { response }
            | CodexSseEvent::Incomplete { response }
            | CodexSseEvent::Failed { response } => {
                if matches!(event, CodexSseEvent::Failed { .. }) {
                    warn!(
                        "codex stream failed: error={:?} incomplete_details={:?}",
                        response.error, response.incomplete_details
                    );
                } else if matches!(event, CodexSseEvent::Incomplete { .. }) {
                    warn!(
                        "codex stream incomplete: incomplete_details={:?}",
                        response.incomplete_details
                    );
                }
                let finish_reason = if self.saw_tool_call { "tool_calls" } else { "stop" };
                vec![OaiChunk {
                    id: id.to_string(),
                    object: "chat.completion.chunk",
                    created,
                    model: model.to_string(),
                    choices: vec![OaiChoice {
                        index: 0,
                        delta: OaiDelta::default(),
                        finish_reason: Some(finish_reason.to_string()),
                    }],
                    usage: response.usage.as_ref().map(|u| OaiUsage {
                        prompt_tokens: u.input_tokens,
                        completion_tokens: u.output_tokens,
                        total_tokens: u.total_tokens.unwrap_or(u.input_tokens + u.output_tokens),
                    }),
                }]
            }

            CodexSseEvent::Error { message, code } => {
                // Mid-stream errors have no clean OAI delta encoding; log and
                // emit nothing. Truncated stream + missing usage tells the
                // client something went wrong.
                warn!("codex stream error: {message} ({code:?})");
                Vec::new()
            }

            CodexSseEvent::Unknown => {
                tracing::debug!(
                    target: "codex::sse_raw",
                    "unhandled SSE event type"
                );
                Vec::new()
            }
        }
    }

    /// Look up or assign a sequential OAI tool_call index for the given Codex
    /// `output_index`. Reasoning items live in the same `output_index` space
    /// as function_calls, so we keep a separate sequential counter for tools
    /// only.
    fn allocate_tool_index(&mut self, output_index: u32) -> u32 {
        if let Some(state) = self.tool_calls.get(&output_index) {
            return state.oai_index;
        }
        let oai_index = self.next_tool_index;
        self.next_tool_index += 1;
        self.tool_calls.insert(
            output_index,
            ToolCallTracking {
                oai_index,
                name_emitted: false,
                args_emitted: false,
            },
        );
        oai_index
    }
}


/// Backward-compat helper: translates a single event statelessly. Useful for
/// events that don't depend on accumulated state (text deltas, completed).
/// For function-call events, prefer `CodexStreamState::handle_event`.
pub fn codex_event_to_oai_chunk(
    event: &CodexSseEvent,
    id: &str,
    model: &str,
    created: i64,
) -> Option<OaiChunk> {
    let mut state = CodexStreamState::new();
    state.handle_event(event, id, model, created).into_iter().next()
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiCompletion {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OaiCompletionChoice>,
    pub usage: OaiUsage,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiCompletionChoice {
    pub index: u32,
    pub message: OaiCompletionMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiCompletionMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<OaiToolCall>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: OaiToolCallFunction,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Default)]
struct ToolCallAccum {
    id: String,
    name: String,
    arguments: String,
    /// Sequential index assigned in the order the function_call appeared.
    seq: u32,
    /// Set to true once an `output_item.done` arrives carrying canonical
    /// arguments — that string takes precedence over any delta-accumulated
    /// buffer (and the canonical buffer should not be overwritten by stragglers).
    done_seen: bool,
}

pub fn aggregate_codex_events(
    events: &[CodexSseEvent],
    id: &str,
    model: &str,
) -> Result<OaiCompletion, TranslateError> {
    let mut seen_completed = false;
    let mut content = String::new();
    let mut usage = OaiUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
    };
    let mut tool_calls: std::collections::HashMap<u32, ToolCallAccum> =
        std::collections::HashMap::new();
    let mut next_seq: u32 = 0;
    for ev in events {
        match ev {
            CodexSseEvent::OutputTextDelta { delta }
            | CodexSseEvent::ReasoningTextDelta { delta }
            | CodexSseEvent::ReasoningSummaryTextDelta { delta } => content.push_str(delta),
            CodexSseEvent::OutputItemAdded { item, output_index } => {
                let Some(obj) = item.as_object() else {
                    continue;
                };
                if obj.get("type").and_then(|v| v.as_str()) != Some("function_call") {
                    continue;
                }
                let call_id = obj
                    .get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let name = obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let seq = next_seq;
                next_seq += 1;
                tool_calls.insert(
                    *output_index,
                    ToolCallAccum {
                        id: call_id,
                        name,
                        arguments: String::new(),
                        seq,
                        done_seen: false,
                    },
                );
            }
            CodexSseEvent::FunctionCallArgumentsDelta {
                delta,
                output_index,
                ..
            } => {
                if let Some(acc) = tool_calls.get_mut(output_index) {
                    if !acc.done_seen {
                        acc.arguments.push_str(delta);
                    }
                }
            }
            CodexSseEvent::FunctionCallArgumentsDone {
                arguments,
                output_index,
                ..
            } => {
                // The .done event carries the canonical full arguments string.
                // Trust it over the accumulated deltas.
                if let Some(acc) = tool_calls.get_mut(output_index) {
                    acc.arguments = arguments.clone();
                }
            }
            CodexSseEvent::OutputItemDone { item, output_index } => {
                let Some(obj) = item.as_object() else {
                    continue;
                };
                if obj.get("type").and_then(|v| v.as_str()) != Some("function_call") {
                    // Reasoning items: text already aggregated via reasoning
                    // deltas. Nothing to do.
                    continue;
                }
                let call_id = obj
                    .get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let name = obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let arguments = obj
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                let acc = tool_calls.entry(*output_index).or_insert_with(|| {
                    let seq = next_seq;
                    next_seq += 1;
                    ToolCallAccum {
                        id: String::new(),
                        name: String::new(),
                        arguments: String::new(),
                        seq,
                        done_seen: false,
                    }
                });
                if !call_id.is_empty() {
                    acc.id = call_id;
                }
                if !name.is_empty() {
                    acc.name = name;
                }
                // `output_item.done` is authoritative — overwrite any
                // delta-accumulated buffer with the canonical arguments.
                acc.arguments = arguments;
                acc.done_seen = true;
            }
            CodexSseEvent::Completed { response }
            | CodexSseEvent::Incomplete { response }
            | CodexSseEvent::Failed { response } => {
                if matches!(ev, CodexSseEvent::Failed { .. }) {
                    warn!(
                        "codex aggregate: response.failed error={:?} incomplete_details={:?}",
                        response.error, response.incomplete_details
                    );
                } else if matches!(ev, CodexSseEvent::Incomplete { .. }) {
                    warn!(
                        "codex aggregate: response.incomplete details={:?}",
                        response.incomplete_details
                    );
                }
                seen_completed = true;
                if let Some(u) = &response.usage {
                    usage = OaiUsage {
                        prompt_tokens: u.input_tokens,
                        completion_tokens: u.output_tokens,
                        total_tokens: u.total_tokens.unwrap_or(u.input_tokens + u.output_tokens),
                    };
                }
            }
            CodexSseEvent::Error { message, code } => {
                return Err(TranslateError::UpstreamError {
                    message: message.clone(),
                    code: code.clone(),
                });
            }
            CodexSseEvent::Unknown => {}
        }
    }
    if !seen_completed {
        return Err(TranslateError::IncompleteStream);
    }

    // Emit tool_calls in their assigned sequential order.
    let mut tool_calls_vec: Vec<ToolCallAccum> = tool_calls.into_values().collect();
    tool_calls_vec.sort_by_key(|t| t.seq);
    let oai_tool_calls: Vec<OaiToolCall> = tool_calls_vec
        .into_iter()
        .map(|t| OaiToolCall {
            id: t.id,
            type_: "function".to_string(),
            function: OaiToolCallFunction {
                name: t.name,
                arguments: t.arguments,
            },
        })
        .collect();

    let has_tool_calls = !oai_tool_calls.is_empty();
    let finish_reason = if has_tool_calls { "tool_calls" } else { "stop" };
    let message_content = if content.is_empty() && has_tool_calls {
        None
    } else {
        Some(content)
    };

    Ok(OaiCompletion {
        id: id.to_string(),
        object: "chat.completion",
        created: chrono::Utc::now().timestamp(),
        model: model.to_string(),
        choices: vec![OaiCompletionChoice {
            index: 0,
            message: OaiCompletionMessage {
                role: "assistant".to_string(),
                content: message_content,
                tool_calls: oai_tool_calls,
            },
            finish_reason: finish_reason.to_string(),
        }],
        usage,
    })
}
