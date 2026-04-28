use clewdr::config::{CodexAuth, CodexAuthStatus};

#[test]
fn codex_auth_roundtrips_through_toml() {
    let original = CodexAuth {
        id: "abc123".to_string(),
        label: Some("personal".to_string()),
        id_token: "eyJhbGciOi.PAYLOAD.SIG".to_string(),
        access_token: "at-token".to_string(),
        refresh_token: "rt-token".to_string(),
        access_expires_at: 1_700_000_000,
        account_id: "acct_123".to_string(),
        plan: Some("plus".to_string()),
        status: CodexAuthStatus::Valid,
        last_used_at: None,
    };

    let toml_str = toml::to_string(&original).expect("serialize");
    let parsed: CodexAuth = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(parsed.id, original.id);
    assert_eq!(parsed.access_token, original.access_token);
    assert_eq!(parsed.account_id, original.account_id);
    assert!(matches!(parsed.status, CodexAuthStatus::Valid));
}

#[test]
fn codex_auth_status_rate_limited_roundtrips() {
    let original = CodexAuthStatus::RateLimited { until: 1_800_000_000 };
    let s = serde_json::to_string(&original).unwrap();
    let parsed: CodexAuthStatus = serde_json::from_str(&s).unwrap();
    match parsed {
        CodexAuthStatus::RateLimited { until } => assert_eq!(until, 1_800_000_000),
        other => panic!("wrong variant: {other:?}"),
    }
}
