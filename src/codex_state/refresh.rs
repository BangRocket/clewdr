use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::config::{CODEX_OAUTH_CLIENT_ID, CODEX_USER_AGENT};

#[derive(Debug, Snafu)]
pub enum RefreshError {
    #[snafu(display("http request failed: {source}"))]
    Http { source: wreq::Error },
    #[snafu(display("response body decode failed: {source}"))]
    Decode { source: wreq::Error },
}

#[derive(Debug, Clone)]
pub enum RefreshOutcome {
    /// 200 OK — new access/refresh/id tokens issued.
    Refreshed {
        id_token: String,
        access_token: String,
        refresh_token: String,
        expires_at: i64,
    },
    /// 400/401 — refresh_token rejected; user must re-login.
    Invalid,
    /// 5xx / network — safe to retry later.
    Transient,
}

#[derive(Debug, Serialize)]
struct RefreshRequest<'a> {
    client_id: &'a str,
    grant_type: &'a str,
    refresh_token: &'a str,
    scope: &'a str,
}

#[derive(Debug, Deserialize)]
struct RefreshResponseOk {
    id_token: String,
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
}

/// Exchange a Codex `refresh_token` for a new `access_token` at
/// `{base_url}/oauth/token`.
///
/// Returns `Ok(RefreshOutcome)` for any HTTP outcome; only network/decode
/// errors surface as `Err`.
pub async fn refresh_codex_token(
    base_url: &str,
    refresh_token: &str,
    proxy: Option<&wreq::Proxy>,
) -> Result<RefreshOutcome, RefreshError> {
    let mut builder = wreq::Client::builder();
    if let Some(p) = proxy {
        builder = builder.proxy(p.clone());
    }
    let client = builder.build().context(HttpSnafu)?;

    let body = RefreshRequest {
        client_id: CODEX_OAUTH_CLIENT_ID,
        grant_type: "refresh_token",
        refresh_token,
        scope: "openid profile email offline_access",
    };

    let url = format!("{}/oauth/token", base_url.trim_end_matches('/'));
    // OpenAI's token endpoint requires form-encoded bodies, not JSON.
    let resp = client
        .post(url)
        .header("user-agent", CODEX_USER_AGENT)
        .form(&body)
        .send()
        .await
        .context(HttpSnafu)?;

    let status = resp.status();
    if status.is_success() {
        let parsed: RefreshResponseOk = resp.json().await.context(DecodeSnafu)?;
        Ok(RefreshOutcome::Refreshed {
            id_token: parsed.id_token,
            access_token: parsed.access_token,
            refresh_token: parsed
                .refresh_token
                .unwrap_or_else(|| refresh_token.to_string()),
            expires_at: chrono::Utc::now().timestamp() + parsed.expires_in,
        })
    } else if status.as_u16() == 400 || status.as_u16() == 401 {
        Ok(RefreshOutcome::Invalid)
    } else {
        Ok(RefreshOutcome::Transient)
    }
}
