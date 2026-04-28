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

#[test]
fn dispatchable_logic() {
    let mut a = CodexAuth {
        id: "x".into(), label: None, id_token: "".into(),
        access_token: "".into(), refresh_token: "".into(),
        access_expires_at: 0, account_id: "".into(), plan: None,
        status: CodexAuthStatus::Valid, last_used_at: None,
    };
    assert!(a.is_dispatchable(100));
    a.status = CodexAuthStatus::Banned;
    assert!(!a.is_dispatchable(100));
    a.status = CodexAuthStatus::Invalid;
    assert!(!a.is_dispatchable(100));
    a.status = CodexAuthStatus::Expired;
    assert!(a.is_dispatchable(100));
    a.status = CodexAuthStatus::RateLimited { until: 200 };
    assert!(!a.is_dispatchable(100));
    assert!(a.is_dispatchable(300));
}

#[test]
fn id_prefix_truncates_to_eight_chars_safely() {
    let mut a = CodexAuth {
        id: "abcdefghij".into(), label: None, id_token: "".into(),
        access_token: "".into(), refresh_token: "".into(),
        access_expires_at: 0, account_id: "".into(), plan: None,
        status: CodexAuthStatus::Valid, last_used_at: None,
    };
    assert_eq!(a.id_prefix(), "abcdefgh");

    a.id = "short".into();
    assert_eq!(a.id_prefix(), "short");

    // Multi-byte UTF-8 must not panic on byte-boundary slicing.
    a.id = "αβγδεζηθικ".into(); // 10 chars, 20 bytes
    assert_eq!(a.id_prefix(), "αβγδεζηθ");
}
