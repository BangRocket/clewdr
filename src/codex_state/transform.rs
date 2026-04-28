use crate::types::claude::{ContentBlock, Message, MessageContent, Role, Tool};
use crate::types::codex::{CodexContent, CodexInputItem, CodexRequest, CodexSseEvent};
use crate::types::oai::CreateMessageParams;
use serde::Serialize;
use snafu::Snafu;
use tracing::warn;

pub const CODEX_MODELS: &[&str] = &[
    "gpt-5-codex",
    "gpt-5",
    "gpt-4.1",
    "o3",
    "o4-mini",
];

#[derive(Debug, Snafu)]
pub enum TranslateError {
    #[snafu(display("model `{model}` is not supported by Codex; valid: {valid}"))]
    UnknownModel { model: String, valid: String },
    #[snafu(display("messages array is empty"))]
    NoMessages,
    #[snafu(display("upstream codex error: {message}{}", code.as_deref().map(|c| format!(" ({c})")).unwrap_or_default()))]
    UpstreamError { message: String, code: Option<String> },
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
    if !CODEX_MODELS.iter().any(|m| *m == req.model) {
        return Err(TranslateError::UnknownModel {
            model: req.model.clone(),
            valid: CODEX_MODELS.join(", "),
        });
    }
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

    let instructions = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };

    // Tools: pass through via serde_json. Drop Known (Anthropic-specific) tools.
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
                    Tool::Custom(_) | Tool::Raw(_) => serde_json::to_value(t).ok(),
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
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

pub fn codex_event_to_oai_chunk(
    event: &CodexSseEvent,
    id: &str,
    model: &str,
    created: i64,
) -> Option<OaiChunk> {
    match event {
        CodexSseEvent::OutputTextDelta { delta } => Some(OaiChunk {
            id: id.to_string(),
            object: "chat.completion.chunk",
            created,
            model: model.to_string(),
            choices: vec![OaiChoice {
                index: 0,
                delta: OaiDelta {
                    role: None,
                    content: Some(delta.clone()),
                },
                finish_reason: None,
            }],
            usage: None,
        }),
        CodexSseEvent::Completed { response } => Some(OaiChunk {
            id: id.to_string(),
            object: "chat.completion.chunk",
            created,
            model: model.to_string(),
            choices: vec![OaiChoice {
                index: 0,
                delta: OaiDelta::default(),
                finish_reason: Some("stop".to_string()),
            }],
            usage: response.usage.as_ref().map(|u| OaiUsage {
                prompt_tokens: u.input_tokens,
                completion_tokens: u.output_tokens,
                total_tokens: u.total_tokens.unwrap_or(u.input_tokens + u.output_tokens),
            }),
        }),
        _ => None,
    }
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
    pub content: String,
}

pub fn aggregate_codex_events(
    events: &[CodexSseEvent],
    id: &str,
    model: &str,
) -> Result<OaiCompletion, TranslateError> {
    let mut seen_completed = false;
    let mut content = String::new();
    let mut usage = OaiUsage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };
    for ev in events {
        match ev {
            CodexSseEvent::OutputTextDelta { delta } => content.push_str(delta),
            CodexSseEvent::Completed { response } => {
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
            _ => {}
        }
    }
    if !seen_completed {
        return Err(TranslateError::IncompleteStream);
    }
    Ok(OaiCompletion {
        id: id.to_string(),
        object: "chat.completion",
        created: chrono::Utc::now().timestamp(),
        model: model.to_string(),
        choices: vec![OaiCompletionChoice {
            index: 0,
            message: OaiCompletionMessage {
                role: "assistant".to_string(),
                content,
            },
            finish_reason: "stop".to_string(),
        }],
        usage,
    })
}
