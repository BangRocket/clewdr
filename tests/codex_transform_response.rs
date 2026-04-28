use clewdr::codex_state::transform::codex_event_to_oai_chunk;
use clewdr::types::codex::CodexSseEvent;

use clewdr::codex_state::transform::aggregate_codex_events;

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
