use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use crate::{
    codex_state::transform::CODEX_MODELS,
    config::{CodexAuth, CodexAuthStatus},
    error::ClewdrError,
    providers::{
        LLMProvider,
        codex::{CodexInvocation, CodexProvider},
    },
    services::codex_auth_actor::CodexAuthActorHandle,
    types::oai::CreateMessageParams,
};

// ========== OAI surface ==========

pub async fn api_codex_chat(
    State(provider): State<Arc<CodexProvider>>,
    Json(params): Json<CreateMessageParams>,
) -> Result<axum::response::Response, ClewdrError> {
    provider.invoke(CodexInvocation { params }).await
}

#[derive(Serialize)]
struct ModelEntry {
    id: &'static str,
    object: &'static str,
    owned_by: &'static str,
}

#[derive(Serialize)]
struct ModelsList {
    object: &'static str,
    data: Vec<ModelEntry>,
}

pub async fn api_codex_models() -> impl IntoResponse {
    let data: Vec<ModelEntry> = CODEX_MODELS
        .iter()
        .map(|m| ModelEntry {
            id: m,
            object: "model",
            owned_by: "openai",
        })
        .collect();
    Json(ModelsList {
        object: "list",
        data,
    })
}

// ========== Admin surface ==========

#[derive(Serialize)]
pub struct CodexAuthSummary {
    pub id: String,
    pub label: Option<String>,
    pub account_id_prefix: String,
    pub plan: Option<String>,
    pub status: CodexAuthStatus,
    pub last_used_at: Option<i64>,
}

impl From<CodexAuth> for CodexAuthSummary {
    fn from(a: CodexAuth) -> Self {
        let p: String = a.account_id.chars().take(8).collect();
        Self {
            id: a.id,
            label: a.label,
            account_id_prefix: p,
            plan: a.plan,
            status: a.status,
            last_used_at: a.last_used_at,
        }
    }
}

pub async fn api_codex_list(
    State(handle): State<CodexAuthActorHandle>,
) -> Result<Json<Vec<CodexAuthSummary>>, ClewdrError> {
    let list = handle.list().await?;
    Ok(Json(list.into_iter().map(Into::into).collect()))
}

#[derive(Deserialize)]
pub struct AddCodexAuthBody {
    pub auth_json: String,
    pub label: Option<String>,
}

pub async fn api_codex_add(
    State(handle): State<CodexAuthActorHandle>,
    Json(body): Json<AddCodexAuthBody>,
) -> Result<(StatusCode, Json<CodexAuthSummary>), ClewdrError> {
    let auth = CodexAuth::from_auth_json(&body.auth_json, body.label).map_err(|e| {
        ClewdrError::BadInput {
            msg: format!("auth.json parse: {e}"),
        }
    })?;
    let summary: CodexAuthSummary = auth.clone().into();
    handle.submit(auth).await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

pub async fn api_codex_delete(
    State(handle): State<CodexAuthActorHandle>,
    Path(id): Path<String>,
) -> Result<StatusCode, ClewdrError> {
    handle.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
