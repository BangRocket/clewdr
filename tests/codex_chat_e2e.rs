use clewdr::codex_state::CodexState;
use clewdr::config::{CodexAuth, CodexAuthStatus};
use clewdr::services::codex_auth_actor::CodexAuthActorHandle;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn auth_for_test() -> CodexAuth {
    CodexAuth {
        id: "test-id".into(),
        label: None,
        id_token: "x.y.z".into(),
        access_token: "valid-at".into(),
        refresh_token: "rt".into(),
        access_expires_at: i64::MAX,
        account_id: "acct".into(),
        plan: None,
        status: CodexAuthStatus::Valid,
        last_used_at: None,
    }
}

#[tokio::test]
async fn streams_codex_sse_into_oai_chunks() {
    let api = MockServer::start().await;
    let sse_body =
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"hi\"}\n\n\
         event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n";
    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(header("authorization", "Bearer valid-at"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body),
        )
        .mount(&api)
        .await;

    let actor = CodexAuthActorHandle::start_with(vec![auth_for_test()])
        .await
        .unwrap();
    let mut state = CodexState::new(actor);
    state.api_base = api.uri();
    state.stream = true;

    let req = serde_json::json!({
        "model": "gpt-5",
        "messages": [{"role": "user", "content": "hi"}],
        "stream": true
    });
    let parsed: clewdr::types::oai::CreateMessageParams = serde_json::from_value(req).unwrap();
    let response = state.try_chat(parsed).await.expect("response");

    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("\"content\":\"hi\""), "stream body: {s}");
    assert!(s.contains("[DONE]"));
}
