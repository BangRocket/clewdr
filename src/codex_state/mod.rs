pub mod chat;
pub mod refresh;
pub mod transform;

use std::sync::LazyLock;

use http::{HeaderValue, header::AUTHORIZATION};
use snafu::{GenerateImplicitData, Location, ResultExt};
use tracing::warn;
use wreq::{Client, Method, Proxy, RequestBuilder};
use wreq_util::Emulation;

use crate::{
    config::{
        CLEWDR_CONFIG, CODEX_API_BASE_URL, CODEX_OAUTH_BASE_URL, CodexAuth, CodexAuthStatus,
    },
    error::{ClewdrError, WreqSnafu},
    services::codex_auth_actor::CodexAuthActorHandle,
};

pub static CODEX_CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

#[derive(Clone)]
pub struct CodexState {
    pub auth_actor: CodexAuthActorHandle,
    pub auth: Option<CodexAuth>,
    pub proxy: Option<Proxy>,
    pub client: Client,
    pub stream: bool,
    pub api_base: String,
    pub oauth_base: String,
}

impl CodexState {
    pub fn new(auth_actor: CodexAuthActorHandle) -> Self {
        Self {
            auth_actor,
            auth: None,
            proxy: CLEWDR_CONFIG.load().wreq_proxy.to_owned(),
            client: CODEX_CLIENT.to_owned(),
            stream: false,
            api_base: CODEX_API_BASE_URL.to_string(),
            oauth_base: CODEX_OAUTH_BASE_URL.to_string(),
        }
    }

    pub async fn request_auth(&mut self) -> Result<CodexAuth, ClewdrError> {
        let auth = self.auth_actor.request().await?;
        self.auth = Some(auth.clone());
        // refresh client with current proxy
        let mut builder = Client::builder().emulation(Emulation::Chrome136);
        if let Some(ref p) = self.proxy {
            builder = builder.proxy(p.clone());
        }
        self.client = builder.build().context(WreqSnafu {
            msg: "build codex client",
        })?;
        Ok(auth)
    }

    pub async fn return_auth(&mut self) {
        if let Some(auth) = self.auth.take()
            && let Err(e) = self.auth_actor.return_auth(auth).await
        {
            warn!("codex return_auth failed: {e}");
        }
    }

    pub fn build_request(
        &self,
        method: Method,
        url: impl ToString,
    ) -> Result<RequestBuilder, ClewdrError> {
        let mut req = self.client.request(method, url.to_string());
        if let Some(auth) = self.auth.as_ref() {
            let bearer = format!("Bearer {}", auth.access_token);
            let bearer_value = HeaderValue::from_str(&bearer)?;
            req = req.header(AUTHORIZATION, bearer_value);
            let account_value = HeaderValue::from_str(&auth.account_id)?;
            req = req.header("chatgpt-account-id", account_value);
        }
        Ok(req)
    }

    /// Refresh access_token if within 60s of expiry. Returns Err and marks auth
    /// status if the refresh definitively fails.
    pub async fn ensure_fresh_access_token(&mut self) -> Result<(), ClewdrError> {
        let now = chrono::Utc::now().timestamp();
        let needs_refresh = self
            .auth
            .as_ref()
            .map(|a| a.access_expires_at.saturating_sub(now) < 60)
            .unwrap_or(false);
        if !needs_refresh {
            return Ok(());
        }
        let Some(auth) = self.auth.as_mut() else {
            return Ok(());
        };
        let outcome = crate::codex_state::refresh::refresh_codex_token(
            &self.oauth_base,
            &auth.refresh_token,
            self.proxy.as_ref(),
        )
        .await
        .map_err(|e| ClewdrError::CodexError {
            loc: Location::generate(),
            msg: format!("codex refresh: {e}"),
        })?;
        match outcome {
            crate::codex_state::refresh::RefreshOutcome::Refreshed {
                id_token,
                access_token,
                refresh_token,
                expires_at,
            } => {
                auth.id_token = id_token;
                auth.access_token = access_token;
                auth.refresh_token = refresh_token;
                auth.access_expires_at = expires_at;
                auth.status = CodexAuthStatus::Valid;
                Ok(())
            }
            crate::codex_state::refresh::RefreshOutcome::Invalid => {
                auth.status = CodexAuthStatus::Invalid;
                Err(ClewdrError::CodexError {
                    loc: Location::generate(),
                    msg: "codex refresh_token rejected; re-login required".to_string(),
                })
            }
            crate::codex_state::refresh::RefreshOutcome::Transient => {
                auth.status = CodexAuthStatus::Expired;
                Err(ClewdrError::CodexError {
                    loc: Location::generate(),
                    msg: "codex token endpoint transient failure".to_string(),
                })
            }
        }
    }
}
