use clewdr::codex_state::transform::codex_event_to_oai_chunk;
use clewdr::types::codex::CodexSseEvent;

use clewdr::codex_state::transform::{CodexStreamState, aggregate_codex_events};

#[test]
fn output_text_delta_becomes_oai_content_chunk() {
    let event = CodexSseEvent::OutputTextDelta { delta: "hello".to_string() };
    let chunk = codex_event_to_oai_chunk(&event, "msg-123", "gpt-5", 1_700_000_000)
        .expect("emits chunk");
    let json = serde_json::to_value(&chunk).unwrap();
    assert_eq!(json["object"], "chat.completion.chunk");
    assert_eq!(json["choices"][0]["delta"]["content"], "hello");
    assert_eq!(json["model"], "gpt-5");
    assert_eq!(json["created"], 1_700_000_000);
}

#[test]
fn unknown_event_emits_no_chunk() {
    let event = CodexSseEvent::Unknown;
    assert!(codex_event_to_oai_chunk(&event, "msg-1", "gpt-5", 1_700_000_000).is_none());
}

#[test]
fn completed_event_emits_finish_reason_and_usage() {
    use clewdr::types::codex::{CodexFinalResponse, CodexUsage};
    let event = CodexSseEvent::Completed {
        response: CodexFinalResponse {
            usage: Some(CodexUsage { input_tokens: 100, output_tokens: 50, total_tokens: None }),
            output: vec![],
        },
    };
    let chunk = codex_event_to_oai_chunk(&event, "id-1", "gpt-5", 1_700_000_000).expect("chunk");
    assert_eq!(chunk.choices[0].finish_reason.as_deref(), Some("stop"));
    let usage = chunk.usage.expect("usage");
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);
}

#[test]
fn aggregates_deltas_into_full_completion() {
    let events = vec![
        CodexSseEvent::OutputTextDelta { delta: "Hello, ".to_string() },
        CodexSseEvent::OutputTextDelta { delta: "world!".to_string() },
        CodexSseEvent::Completed {
            response: clewdr::types::codex::CodexFinalResponse {
                usage: Some(clewdr::types::codex::CodexUsage {
                    input_tokens: 5, output_tokens: 3, total_tokens: Some(8),
                }),
                output: vec![],
            },
        },
    ];
    let resp = aggregate_codex_events(&events, "id", "gpt-5").expect("aggregates");
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["object"], "chat.completion");
    assert_eq!(v["choices"][0]["message"]["content"], "Hello, world!");
    assert_eq!(v["usage"]["total_tokens"], 8);
}

#[test]
fn aggregator_surfaces_error_event_as_translate_error() {
    let events = vec![
        CodexSseEvent::OutputTextDelta { delta: "partial".into() },
        CodexSseEvent::Error { message: "boom".into(), code: Some("rate_limit".into()) },
    ];
    let err = aggregate_codex_events(&events, "id", "gpt-5").expect_err("must error");
    let s = err.to_string();
    assert!(s.contains("boom"), "{s}");
    assert!(s.contains("rate_limit"), "{s}");
}

#[test]
fn aggregator_errors_when_completed_missing() {
    let events = vec![CodexSseEvent::OutputTextDelta { delta: "abc".into() }];
    let err = aggregate_codex_events(&events, "id", "gpt-5").expect_err("must error");
    assert!(
        err.to_string().to_lowercase().contains("truncated")
            || err.to_string().to_lowercase().contains("completed"),
        "{err}"
    );
}

#[test]
fn tool_call_added_emits_intro_chunk() {
    let mut state = CodexStreamState::new();
    let event = CodexSseEvent::OutputItemAdded {
        item: serde_json::json!({
            "type": "function_call",
            "id": "fc_abc",
            "status": "in_progress",
            "arguments": "",
            "call_id": "call_RQwYK",
            "name": "read_text_file"
        }),
        output_index: 1,
    };
    let chunks = state.handle_event(&event, "id-1", "gpt-5", 1_700_000_000);
    assert_eq!(chunks.len(), 1);
    let v = serde_json::to_value(&chunks[0]).unwrap();
    let tc = &v["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(tc["index"], 0);
    assert_eq!(tc["id"], "call_RQwYK");
    assert_eq!(tc["type"], "function");
    assert_eq!(tc["function"]["name"], "read_text_file");
    assert_eq!(tc["function"]["arguments"], "");
    // Should NOT include a finish_reason mid-stream.
    assert!(v["choices"][0]["finish_reason"].is_null());
}

#[test]
fn tool_call_args_delta_emits_chunk_with_arguments() {
    let mut state = CodexStreamState::new();
    // Establish the function_call at output_index=1 first.
    let _ = state.handle_event(
        &CodexSseEvent::OutputItemAdded {
            item: serde_json::json!({
                "type": "function_call",
                "call_id": "call_x",
                "name": "read_text_file"
            }),
            output_index: 1,
        },
        "id",
        "gpt-5",
        0,
    );
    let chunks = state.handle_event(
        &CodexSseEvent::FunctionCallArgumentsDelta {
            delta: r#"{"head":80,"#.to_string(),
            item_id: "fc_abc".to_string(),
            output_index: 1,
        },
        "id",
        "gpt-5",
        0,
    );
    assert_eq!(chunks.len(), 1);
    let v = serde_json::to_value(&chunks[0]).unwrap();
    let tc = &v["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(tc["index"], 0);
    assert_eq!(tc["function"]["arguments"], r#"{"head":80,"#);
    // No id/type/name should be set on subsequent chunks.
    assert!(tc["id"].is_null());
    assert!(tc["type"].is_null());
    assert!(tc["function"]["name"].is_null());
}

#[test]
fn multiple_tool_calls_get_sequential_indices() {
    let mut state = CodexStreamState::new();
    // Reasoning at output_index=0 — should NOT consume a tool_call slot.
    let reasoning = state.handle_event(
        &CodexSseEvent::OutputItemAdded {
            item: serde_json::json!({"type": "reasoning", "id": "rs_1", "summary": []}),
            output_index: 0,
        },
        "id",
        "gpt-5",
        0,
    );
    assert!(reasoning.is_empty(), "reasoning emits no chunk");

    let first = state.handle_event(
        &CodexSseEvent::OutputItemAdded {
            item: serde_json::json!({"type": "function_call", "call_id": "call_a", "name": "f1"}),
            output_index: 1,
        },
        "id",
        "gpt-5",
        0,
    );
    let second = state.handle_event(
        &CodexSseEvent::OutputItemAdded {
            item: serde_json::json!({"type": "function_call", "call_id": "call_b", "name": "f2"}),
            output_index: 2,
        },
        "id",
        "gpt-5",
        0,
    );
    let v1 = serde_json::to_value(&first[0]).unwrap();
    let v2 = serde_json::to_value(&second[0]).unwrap();
    // Sequential indices among function_calls only — NOT 1 and 2.
    assert_eq!(v1["choices"][0]["delta"]["tool_calls"][0]["index"], 0);
    assert_eq!(v2["choices"][0]["delta"]["tool_calls"][0]["index"], 1);
}

#[test]
fn aggregate_with_tool_calls_finishes_with_tool_calls_reason() {
    use clewdr::types::codex::{CodexFinalResponse, CodexUsage};
    let events = vec![
        CodexSseEvent::OutputItemAdded {
            item: serde_json::json!({
                "type": "function_call",
                "call_id": "call_xyz",
                "name": "read_text_file"
            }),
            output_index: 1,
        },
        CodexSseEvent::FunctionCallArgumentsDelta {
            delta: r#"{"head":80,"#.to_string(),
            item_id: "fc_abc".to_string(),
            output_index: 1,
        },
        CodexSseEvent::FunctionCallArgumentsDelta {
            delta: r#""path":"x"}"#.to_string(),
            item_id: "fc_abc".to_string(),
            output_index: 1,
        },
        CodexSseEvent::FunctionCallArgumentsDone {
            arguments: r#"{"head":80,"path":"x"}"#.to_string(),
            item_id: "fc_abc".to_string(),
            output_index: 1,
        },
        CodexSseEvent::Completed {
            response: CodexFinalResponse {
                usage: Some(CodexUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: None,
                }),
                output: vec![],
            },
        },
    ];
    let resp = aggregate_codex_events(&events, "id", "gpt-5").expect("aggregates");
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["choices"][0]["finish_reason"], "tool_calls");
    let tc = &v["choices"][0]["message"]["tool_calls"][0];
    assert_eq!(tc["id"], "call_xyz");
    assert_eq!(tc["type"], "function");
    assert_eq!(tc["function"]["name"], "read_text_file");
    assert_eq!(tc["function"]["arguments"], r#"{"head":80,"path":"x"}"#);
    // No text content — content should be omitted (None serialized as missing).
    assert!(
        v["choices"][0]["message"]["content"].is_null(),
        "content should be null/missing when only tool_calls present, got: {}",
        v["choices"][0]["message"]
    );
}

#[test]
fn reasoning_output_item_added_does_not_emit_tool_call() {
    let mut state = CodexStreamState::new();
    let event = CodexSseEvent::OutputItemAdded {
        item: serde_json::json!({"type": "reasoning", "id": "rs_1", "summary": []}),
        output_index: 0,
    };
    let chunks = state.handle_event(&event, "id", "gpt-5", 0);
    assert!(chunks.is_empty(), "reasoning items must be dropped");
}

#[test]
fn aggregate_text_only_stays_with_stop_finish_reason() {
    // Regression check: no tool_calls -> finish_reason still "stop", content present.
    use clewdr::types::codex::{CodexFinalResponse, CodexUsage};
    let events = vec![
        CodexSseEvent::OutputTextDelta { delta: "abc".into() },
        CodexSseEvent::Completed {
            response: CodexFinalResponse {
                usage: Some(CodexUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                    total_tokens: Some(2),
                }),
                output: vec![],
            },
        },
    ];
    let resp = aggregate_codex_events(&events, "id", "gpt-5").expect("ok");
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["choices"][0]["finish_reason"], "stop");
    assert_eq!(v["choices"][0]["message"]["content"], "abc");
}
