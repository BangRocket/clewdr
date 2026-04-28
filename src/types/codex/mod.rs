//! Minimal types for Codex `/backend-api/codex/responses` request and SSE events.
//! Only the subset needed for translation is modeled.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct CodexRequest {
    pub model: String,
    pub input: Vec<CodexInputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<CodexText>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CodexInputItem {
    Message { role: String, content: Vec<CodexContent> },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CodexContent {
    InputText { text: String },
    OutputText { text: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexText {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<serde_json::Value>,
}

// SSE event payload — partial, only fields we use.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum CodexSseEvent {
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta { delta: String },
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded {
        item: serde_json::Value,
        #[serde(default)]
        output_index: u32,
    },
    #[serde(rename = "response.function_call_arguments.delta")]
    FunctionCallArgumentsDelta {
        delta: String,
        #[serde(default)]
        item_id: String,
        #[serde(default)]
        output_index: u32,
    },
    #[serde(rename = "response.function_call_arguments.done")]
    FunctionCallArgumentsDone {
        arguments: String,
        #[serde(default)]
        item_id: String,
        #[serde(default)]
        output_index: u32,
    },
    #[serde(rename = "response.completed")]
    Completed { response: CodexFinalResponse },
    #[serde(rename = "response.error")]
    Error { message: String, code: Option<String> },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexFinalResponse {
    #[serde(default)]
    pub usage: Option<CodexUsage>,
    #[serde(default)]
    pub output: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: Option<u64>,
}
