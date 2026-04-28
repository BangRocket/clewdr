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
fn passes_through_arbitrary_model_names() {
    // Translator no longer enforces a whitelist — upstream Codex decides
    // what's supported. Models like `gpt-5.5` that didn't exist when this
    // proxy was first written should pass through unchanged.
    for model in ["gpt-5.5", "gpt-5.3-codex", "fake-model-xyz"] {
        let oai = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "hi"}],
            "stream": false
        });
        let oai: CreateMessageParams = serde_json::from_value(oai).unwrap();
        let codex = translate_chat_completions_to_codex(&oai)
            .unwrap_or_else(|e| panic!("model `{model}` should pass through: {e}"));
        assert_eq!(codex.model, model);
    }
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

#[test]
fn openai_function_tools_are_normalized_for_codex_responses() {
    let tools = serde_json::json!([
        {"type": "function", "function": {"name": "search", "description": "x", "parameters": {}}}
    ]);
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": tools.clone(),
        "tool_choice": "auto",
        "stream": false
    });
    let oai: CreateMessageParams = serde_json::from_value(oai).unwrap();
    let codex = translate_chat_completions_to_codex(&oai).expect("translates");
    assert_eq!(
        codex.tools,
        vec![serde_json::json!({
            "type": "function",
            "name": "search",
            "description": "x",
            "parameters": {}
        })]
    );
    // tool_choice "auto" round-trips
    assert!(codex.tool_choice.is_some());
}

#[test]
fn rejects_empty_messages() {
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [],
        "stream": false
    });
    let oai: CreateMessageParams = serde_json::from_value(oai).unwrap();
    assert!(matches!(
        translate_chat_completions_to_codex(&oai),
        Err(clewdr::codex_state::transform::TranslateError::NoMessages)
    ));
}

#[test]
fn system_messages_join_with_double_newline() {
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [
            {"role": "system", "content": "A"},
            {"role": "system", "content": "B"},
            {"role": "user", "content": "hi"}
        ],
        "stream": false
    });
    let oai: CreateMessageParams = serde_json::from_value(oai).unwrap();
    let codex = translate_chat_completions_to_codex(&oai).expect("translates");
    assert_eq!(codex.instructions.as_deref(), Some("A\n\nB"));
}
