use clewdr::config::{ClewdrConfig, OaiBackend};

#[test]
fn default_config_uses_claude_backend() {
    let cfg = ClewdrConfig::default();
    assert_eq!(cfg.default_oai_backend, OaiBackend::Claude);
}

#[test]
fn default_oai_backend_roundtrips_through_toml() {
    let mut cfg = ClewdrConfig::default();
    cfg.default_oai_backend = OaiBackend::Codex;

    let s = toml::to_string(&cfg).expect("serialize");
    assert!(
        s.contains("default_oai_backend = \"codex\""),
        "expected snake_case codex serialization, got: {s}"
    );

    let parsed: ClewdrConfig = toml::from_str(&s).expect("deserialize");
    assert_eq!(parsed.default_oai_backend, OaiBackend::Codex);
}

#[test]
fn default_oai_backend_roundtrips_through_json() {
    // Client posts JSON containing `"codex"` as a lowercase string.
    let json = serde_json::json!({"default_oai_backend": "codex"});
    let v: OaiBackend = serde_json::from_value(json["default_oai_backend"].clone())
        .expect("deserialize codex");
    assert_eq!(v, OaiBackend::Codex);

    let json = serde_json::json!({"default_oai_backend": "claude"});
    let v: OaiBackend = serde_json::from_value(json["default_oai_backend"].clone())
        .expect("deserialize claude");
    assert_eq!(v, OaiBackend::Claude);

    // Round-trip back to a JSON string.
    let s = serde_json::to_string(&OaiBackend::Codex).expect("serialize");
    assert_eq!(s, "\"codex\"");
}

#[test]
fn legacy_config_without_field_defaults_to_claude() {
    // A config TOML written before this field existed should still parse,
    // and the new field should default to Claude.
    let toml = r#"
        password = "x"
        admin_password = "y"
    "#;
    let cfg: ClewdrConfig = toml::from_str(toml).expect("parse legacy config");
    assert_eq!(cfg.default_oai_backend, OaiBackend::Claude);
}
