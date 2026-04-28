use std::sync::Arc;
use std::time::Instant;

use axum::response::Response;
use colored::Colorize;
use tracing::info;

use super::LLMProvider;
use crate::{
    codex_state::CodexState,
    error::ClewdrError,
    services::codex_auth_actor::CodexAuthActorHandle,
    types::oai::CreateMessageParams,
    utils::{enabled, print_out_json},
};

#[derive(Clone)]
pub struct CodexInvocation {
    pub params: CreateMessageParams,
}

#[derive(Clone)]
pub struct CodexProvider {
    auth_actor: CodexAuthActorHandle,
}

impl CodexProvider {
    pub fn new(auth_actor: CodexAuthActorHandle) -> Self {
        Self { auth_actor }
    }
}

#[async_trait::async_trait]
impl LLMProvider for CodexProvider {
    type Request = CodexInvocation;
    type Output = Response;

    async fn invoke(&self, request: Self::Request) -> Result<Self::Output, ClewdrError> {
        let mut state = CodexState::new(self.auth_actor.clone());
        let stream = request.params.stream.unwrap_or(false);
        let model = request.params.model.clone();
        let msgs = request.params.messages.len();
        info!(
            "[REQ] codex stream: {}, msgs: {}, model: {}",
            enabled(stream),
            msgs.to_string().green(),
            model.green()
        );
        print_out_json(&request.params, "codex_client_req.json");
        let stopwatch = Instant::now();
        let response = state.try_chat(request.params).await?;
        let elapsed = stopwatch.elapsed();
        info!(
            "[FIN] codex elapsed: {}s",
            format!("{}", elapsed.as_secs_f32()).green()
        );
        Ok(response)
    }
}

pub fn build_codex_provider(auth_actor: CodexAuthActorHandle) -> Arc<CodexProvider> {
    Arc::new(CodexProvider::new(auth_actor))
}
