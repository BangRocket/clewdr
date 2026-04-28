use clewdr::config::{ClewdrConfig, CodexAuth, CodexAuthStatus};

#[test]
fn config_with_codex_auth_roundtrips_through_toml() {
    let mut cfg = ClewdrConfig::default();
    cfg.codex_auth.push(CodexAuth {
        id: "abc12345".into(),
        label: Some("test".into()),
        id_token: "eyJ.x.y".into(),
        access_token: "at".into(),
        refresh_token: "rt".into(),
        access_expires_at: 1_700_000_000,
        account_id: "acct".into(),
        plan: None,
        status: CodexAuthStatus::Valid,
        last_used_at: None,
    });

    let s = toml::to_string(&cfg).expect("serialize");
    let parsed: ClewdrConfig = toml::from_str(&s).expect("deserialize");
    assert_eq!(parsed.codex_auth.len(), 1);
    assert_eq!(parsed.codex_auth[0].id, "abc12345");
}
