use clewdr::codex_state::refresh::{RefreshOutcome, refresh_codex_token};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn refresh_success_returns_new_tokens() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id_token": "eyJ.new.id",
            "access_token": "new-at",
            "refresh_token": "new-rt",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;

    let result = refresh_codex_token(&server.uri(), "old-rt", None).await;
    let RefreshOutcome::Refreshed {
        id_token,
        access_token,
        refresh_token,
        expires_at,
    } = result.expect("ok")
    else {
        panic!("expected Refreshed")
    };
    assert_eq!(access_token, "new-at");
    assert_eq!(refresh_token, "new-rt");
    assert_eq!(id_token, "eyJ.new.id");
    let now = chrono::Utc::now().timestamp();
    assert!(expires_at > now + 3500 && expires_at < now + 3700);
}

#[tokio::test]
async fn refresh_invalid_grant_returns_invalid() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant"
        })))
        .mount(&server)
        .await;

    let outcome = refresh_codex_token(&server.uri(), "bad-rt", None)
        .await
        .expect("ok");
    assert!(matches!(outcome, RefreshOutcome::Invalid));
}

#[tokio::test]
async fn refresh_5xx_returns_transient() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let outcome = refresh_codex_token(&server.uri(), "rt", None)
        .await
        .expect("ok");
    assert!(matches!(outcome, RefreshOutcome::Transient));
}

#[tokio::test]
async fn refresh_request_uses_form_encoding() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann"))
        .and(body_string_contains("refresh_token=rt-xyz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id_token": "x.y.z",
            "access_token": "at",
            "refresh_token": "rt",
            "expires_in": 60
        })))
        .mount(&server)
        .await;

    let outcome = refresh_codex_token(&server.uri(), "rt-xyz", None)
        .await
        .expect("ok");
    assert!(matches!(outcome, RefreshOutcome::Refreshed { .. }));
}
