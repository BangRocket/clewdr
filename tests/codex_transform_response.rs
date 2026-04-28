use clewdr::codex_state::transform::{
    codex_event_to_oai_chunk, OaiChunk,
};
use clewdr::types::codex::CodexSseEvent;

#[test]
fn output_text_delta_becomes_oai_content_chunk() {
    let event = CodexSseEvent::OutputTextDelta { delta: "hello".to_string() };
    let chunk = codex_event_to_oai_chunk(&event, "msg-123", "gpt-5").expect("emits chunk");
    let json = serde_json::to_value(&chunk).unwrap();
    assert_eq!(json["object"], "chat.completion.chunk");
    assert_eq!(json["choices"][0]["delta"]["content"], "hello");
    assert_eq!(json["model"], "gpt-5");
}

#[test]
fn unknown_event_emits_no_chunk() {
    let event = CodexSseEvent::Unknown;
    assert!(codex_event_to_oai_chunk(&event, "msg-1", "gpt-5").is_none());
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
    let chunk = codex_event_to_oai_chunk(&event, "id-1", "gpt-5").expect("chunk");
    assert_eq!(chunk.choices[0].finish_reason.as_deref(), Some("stop"));
    let usage = chunk.usage.expect("usage");
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);
}
