use clewdr::codex_state::transform::{
    translate_chat_completions_to_codex, translate_oai_request_to_codex,
};
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
    // Codex backend rejects requests without `instructions`; we set a default
    // when the client doesn't supply a system message.
    assert!(codex.instructions.is_some());
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

// ---- Permissive translator: tool-call conversation history ----

#[test]
fn assistant_message_with_tool_calls_becomes_function_call_input_item() {
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [
            {"role": "user", "content": "read the file"},
            {
                "role": "assistant",
                "tool_calls": [
                    {
                        "id": "call_xxx",
                        "type": "function",
                        "function": {
                            "name": "read_text_file",
                            "arguments": "{\"path\":\"/tmp/x\"}"
                        }
                    }
                ]
            }
        ],
        "stream": false
    });
    let codex = translate_oai_request_to_codex(&oai).expect("translates");
    let val = serde_json::to_value(&codex).unwrap();
    let input = val["input"].as_array().expect("input array");
    // Should be: user message, function_call. The assistant has no `content`,
    // so we must NOT emit an empty assistant message before the function_call.
    assert_eq!(input.len(), 2, "unexpected input: {input:#?}");
    assert_eq!(input[0]["type"], "message");
    assert_eq!(input[0]["role"], "user");
    assert_eq!(input[1]["type"], "function_call");
    assert_eq!(input[1]["call_id"], "call_xxx");
    assert_eq!(input[1]["name"], "read_text_file");
    assert_eq!(input[1]["arguments"], "{\"path\":\"/tmp/x\"}");
}

#[test]
fn tool_role_message_becomes_function_call_output() {
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [
            {"role": "user", "content": "go"},
            {"role": "tool", "content": "<file body>", "tool_call_id": "call_xxx"}
        ],
        "stream": false
    });
    let codex = translate_oai_request_to_codex(&oai).expect("translates");
    let val = serde_json::to_value(&codex).unwrap();
    let input = val["input"].as_array().expect("input array");
    assert_eq!(input.len(), 2);
    assert_eq!(input[1]["type"], "function_call_output");
    assert_eq!(input[1]["call_id"], "call_xxx");
    assert_eq!(input[1]["output"], "<file body>");
}

#[test]
fn multi_turn_conversation_with_tool_history_translates_correctly() {
    let oai = serde_json::json!({
        "model": "gpt-5.5",
        "messages": [
            {"role": "system", "content": "be helpful"},
            {"role": "user", "content": "read x.txt"},
            {
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "read_text_file", "arguments": "{\"path\":\"x.txt\"}"}
                }]
            },
            {"role": "tool", "content": "hello", "tool_call_id": "call_1"},
            {"role": "user", "content": "continue"}
        ],
        "stream": false
    });
    let codex = translate_oai_request_to_codex(&oai).expect("translates");
    let val = serde_json::to_value(&codex).unwrap();
    let input = val["input"].as_array().expect("input array");
    assert_eq!(input.len(), 4, "system goes to instructions, 4 input items");
    assert_eq!(input[0]["type"], "message");
    assert_eq!(input[0]["role"], "user");
    assert_eq!(input[1]["type"], "function_call");
    assert_eq!(input[1]["call_id"], "call_1");
    assert_eq!(input[2]["type"], "function_call_output");
    assert_eq!(input[2]["call_id"], "call_1");
    assert_eq!(input[2]["output"], "hello");
    assert_eq!(input[3]["type"], "message");
    assert_eq!(input[3]["role"], "user");
    assert_eq!(codex.instructions.as_deref(), Some("be helpful"));
}

#[test]
fn tool_message_content_array_form_is_joined() {
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [
            {"role": "user", "content": "go"},
            {
                "role": "tool",
                "tool_call_id": "x",
                "content": [
                    {"type": "text", "text": "a"},
                    {"type": "text", "text": "b"}
                ]
            }
        ],
        "stream": false
    });
    let codex = translate_oai_request_to_codex(&oai).expect("translates");
    let val = serde_json::to_value(&codex).unwrap();
    let input = val["input"].as_array().expect("input array");
    assert_eq!(input[1]["type"], "function_call_output");
    assert_eq!(input[1]["output"], "a\nb");
}

#[test]
fn assistant_with_both_text_and_tool_calls_emits_message_then_function_calls() {
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [
            {"role": "user", "content": "hi"},
            {
                "role": "assistant",
                "content": "thinking aloud",
                "tool_calls": [{
                    "id": "call_a",
                    "type": "function",
                    "function": {"name": "f", "arguments": "{}"}
                }]
            }
        ],
        "stream": false
    });
    let codex = translate_oai_request_to_codex(&oai).expect("translates");
    let val = serde_json::to_value(&codex).unwrap();
    let input = val["input"].as_array().expect("input array");
    assert_eq!(input.len(), 3);
    // order: user, assistant message, function_call
    assert_eq!(input[1]["type"], "message");
    assert_eq!(input[1]["role"], "assistant");
    assert_eq!(input[1]["content"][0]["text"], "thinking aloud");
    assert_eq!(input[2]["type"], "function_call");
    assert_eq!(input[2]["name"], "f");
}

#[test]
fn unknown_role_is_dropped_with_warn_and_function_role_translates() {
    // role: "system_extra" (made up) is dropped; legacy role: "function" is
    // mapped like "tool" using `name` as the call_id.
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [
            {"role": "user", "content": "go"},
            {"role": "system_extra", "content": "ignore me"},
            {"role": "function", "name": "call_legacy", "content": "result-text"}
        ],
        "stream": false
    });
    let codex = translate_oai_request_to_codex(&oai).expect("translates");
    let val = serde_json::to_value(&codex).unwrap();
    let input = val["input"].as_array().expect("input array");
    // user message + legacy function-call output. Unknown role dropped.
    assert_eq!(input.len(), 2);
    assert_eq!(input[1]["type"], "function_call_output");
    assert_eq!(input[1]["call_id"], "call_legacy");
    assert_eq!(input[1]["output"], "result-text");
}

#[test]
fn function_call_and_function_call_output_use_snake_case_type_discriminators() {
    // Belt-and-braces test that the serde discriminator strings match what
    // the Codex Responses API expects. Whole-input shape was checked above;
    // here we just confirm the variant-tag rename_all behavior.
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [
            {"role": "user", "content": "go"},
            {
                "role": "assistant",
                "tool_calls": [{
                    "id": "c1",
                    "type": "function",
                    "function": {"name": "n", "arguments": "{}"}
                }]
            },
            {"role": "tool", "tool_call_id": "c1", "content": "out"}
        ],
        "stream": false
    });
    let codex = translate_oai_request_to_codex(&oai).expect("translates");
    let val = serde_json::to_value(&codex).unwrap();
    let types: Vec<&str> = val["input"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["type"].as_str().unwrap())
        .collect();
    assert_eq!(types, vec!["message", "function_call", "function_call_output"]);
}
