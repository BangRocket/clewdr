use clewdr::codex_state::transform::translate_chat_completions_to_codex;
use clewdr::types::oai::CreateMessageParams;

#[test]
fn translates_simple_user_message() {
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [
            {"role": "user", "content": "hello world"}
        ],
        "stream": false
    });
    let oai: CreateMessageParams = serde_json::from_value(oai).unwrap();
    let codex = translate_chat_completions_to_codex(&oai).expect("translates");
    assert_eq!(codex.model, "gpt-5");
    assert!(codex.instructions.is_none());
    assert_eq!(codex.input.len(), 1);
}

#[test]
fn extracts_system_messages_into_instructions() {
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [
            {"role": "system", "content": "be terse"},
            {"role": "system", "content": "no emojis"},
            {"role": "user", "content": "hi"}
        ],
        "stream": false
    });
    let oai: CreateMessageParams = serde_json::from_value(oai).unwrap();
    let codex = translate_chat_completions_to_codex(&oai).expect("translates");
    let inst = codex.instructions.expect("has instructions");
    assert!(inst.contains("be terse"));
    assert!(inst.contains("no emojis"));
    assert_eq!(codex.input.len(), 1, "system messages stripped from input");
}

#[test]
fn rejects_unknown_model() {
    let oai = serde_json::json!({
        "model": "fake-model-xyz",
        "messages": [{"role": "user", "content": "hi"}],
        "stream": false
    });
    let oai: CreateMessageParams = serde_json::from_value(oai).unwrap();
    assert!(translate_chat_completions_to_codex(&oai).is_err());
}

#[test]
fn maps_max_tokens_to_max_output_tokens() {
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 256,
        "stream": false
    });
    let oai: CreateMessageParams = serde_json::from_value(oai).unwrap();
    let codex = translate_chat_completions_to_codex(&oai).expect("translates");
    assert_eq!(codex.max_output_tokens, Some(256));
}
