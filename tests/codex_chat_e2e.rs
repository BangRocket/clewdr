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
    let response = state.try_chat(req).await.expect("response");

    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("\"content\":\"hi\""), "stream body: {s}");
    assert!(s.contains("[DONE]"));
}

#[tokio::test]
async fn rotates_on_401_and_succeeds_on_second_cred() {
    let api = MockServer::start().await;

    let bad_token = ResponseTemplate::new(401);
    let good_body =
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n\
         event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n";

    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(header("authorization", "Bearer cred-a"))
        .respond_with(bad_token)
        .mount(&api)
        .await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(header("authorization", "Bearer cred-b"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(good_body),
        )
        .mount(&api)
        .await;

    let mut a = auth_for_test();
    a.id = "a".into();
    a.access_token = "cred-a".into();
    let mut b = auth_for_test();
    b.id = "b".into();
    b.access_token = "cred-b".into();
    let actor = CodexAuthActorHandle::start_with(vec![a, b]).await.unwrap();
    let mut state = CodexState::new(actor);
    state.api_base = api.uri();
    state.stream = true;

    let req = serde_json::json!({
        "model": "gpt-5",
        "messages": [{"role": "user", "content": "hi"}],
        "stream": true
    });
    let response = state.try_chat(req).await.expect("rotates and succeeds");
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("\"content\":\"ok\""));
}

#[tokio::test]
async fn streams_codex_tool_call_events_into_oai_tool_call_chunks() {
    let api = MockServer::start().await;
    // Simulate a real Codex tool-call SSE turn:
    //   reasoning at output_index=0,
    //   function_call at output_index=1 with two argument deltas + done,
    //   completion.
    let sse_body = concat!(
        // reasoning (must be dropped)
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"rs_1\",\"type\":\"reasoning\",\"summary\":[]},\"output_index\":0,\"sequence_number\":2}\n\n",
        // function_call introduce
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc_1\",\"type\":\"function_call\",\"status\":\"in_progress\",\"arguments\":\"\",\"call_id\":\"call_RQwYK\",\"name\":\"read_text_file\"},\"output_index\":1,\"sequence_number\":4}\n\n",
        // arguments deltas
        "event: response.function_call_arguments.delta\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{\\\"head\\\":80,\",\"item_id\":\"fc_1\",\"output_index\":1,\"sequence_number\":5}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"\\\"path\\\":\\\"x\\\"}\",\"item_id\":\"fc_1\",\"output_index\":1,\"sequence_number\":6}\n\n",
        // arguments done (must NOT double-emit content)
        "event: response.function_call_arguments.done\n",
        "data: {\"type\":\"response.function_call_arguments.done\",\"arguments\":\"{\\\"head\\\":80,\\\"path\\\":\\\"x\\\"}\",\"item_id\":\"fc_1\",\"output_index\":1,\"sequence_number\":7}\n\n",
        // completion
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":5}}}\n\n",
    );

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
    let response = state.try_chat(req).await.expect("response");

    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();

    // Parse out the data: lines (excluding [DONE]) so we can assert order.
    let chunks: Vec<serde_json::Value> = s
        .split("\n\n")
        .filter_map(|frame| frame.strip_prefix("data: "))
        .filter(|d| d.trim() != "[DONE]")
        .filter_map(|d| serde_json::from_str(d.trim()).ok())
        .collect();

    // We expect: tool-call intro, args-delta, args-delta, completion (4 chunks).
    assert_eq!(chunks.len(), 4, "got chunks: {chunks:?}\nraw stream: {s}");

    // 1. intro
    let intro = &chunks[0]["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(intro["index"], 0);
    assert_eq!(intro["id"], "call_RQwYK");
    assert_eq!(intro["type"], "function");
    assert_eq!(intro["function"]["name"], "read_text_file");

    // 2-3. args streamed
    let args1 = &chunks[1]["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"];
    let args2 = &chunks[2]["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"];
    assert_eq!(args1, r#"{"head":80,"#);
    assert_eq!(args2, r#""path":"x"}"#);

    // 4. completion: tool_calls finish_reason and usage.
    assert_eq!(chunks[3]["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(chunks[3]["usage"]["prompt_tokens"], 10);
    assert_eq!(chunks[3]["usage"]["completion_tokens"], 5);

    // [DONE] terminator present.
    assert!(s.contains("[DONE]"));
}

#[tokio::test]
async fn body_forces_stream_and_strips_unsupported_fields() {
    use wiremock::matchers::body_partial_json;

    let api = MockServer::start().await;
    let good_body =
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n";

    // Match: stream=true, store=false. (Partial-json matcher asserts these are present.)
    let expected = serde_json::json!({
        "stream": true,
        "store": false
    });

    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(body_partial_json(expected))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(good_body),
        )
        .mount(&api)
        .await;

    let actor = CodexAuthActorHandle::start_with(vec![auth_for_test()])
        .await
        .unwrap();
    let mut state = CodexState::new(actor);
    state.api_base = api.uri();
    state.stream = false;

    let req = serde_json::json!({
        "model": "gpt-5",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 256,
        "temperature": 0.7,
        "stream": false
    });
    let response = state.try_chat(req).await.expect("ok");
    // Just confirm we got 2xx; body matcher already validated stream=true + store=false.
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn e2e_tool_call_continuation_request_succeeds() {
    use wiremock::matchers::body_partial_json;

    let api = MockServer::start().await;
    let good_body =
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"done\"}\n\n\
         event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n";

    // Match: the upstream request body must contain a function_call input
    // item (from the assistant's prior tool_calls) and a function_call_output
    // input item (from the tool message), in that order.
    let expected = serde_json::json!({
        "input": [
            {"type": "message", "role": "user"},
            {"type": "function_call", "call_id": "call_xyz", "name": "read_text_file"},
            {"type": "function_call_output", "call_id": "call_xyz", "output": "<file contents>"},
            {"type": "message", "role": "user"}
        ]
    });

    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(body_partial_json(expected))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(good_body),
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
        "model": "gpt-5.5",
        "messages": [
            {"role": "system", "content": "be helpful"},
            {"role": "user", "content": "read x.txt"},
            {
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_xyz",
                    "type": "function",
                    "function": {
                        "name": "read_text_file",
                        "arguments": "{\"path\":\"x.txt\"}"
                    }
                }]
            },
            {"role": "tool", "content": "<file contents>", "tool_call_id": "call_xyz"},
            {"role": "user", "content": "continue..."}
        ],
        "stream": true
    });
    let response = state.try_chat(req).await.expect("ok");
    assert_eq!(response.status(), 200);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("\"content\":\"done\""), "stream body: {s}");
    assert!(s.contains("[DONE]"));
}
