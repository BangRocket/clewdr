use crate::types::claude::{ContentBlock, Message, MessageContent, Role, Tool};
use crate::types::codex::{CodexContent, CodexInputItem, CodexRequest};
use crate::types::oai::CreateMessageParams;
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
