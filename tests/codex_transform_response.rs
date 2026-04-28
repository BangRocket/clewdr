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
