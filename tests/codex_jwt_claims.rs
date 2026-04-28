use clewdr::config::CodexAuth;
use clewdr::config::decode_codex_id_token_claims;

// Sample id_token payload: { "sub": "acct_abc", "https://api.openai.com/auth": { "chatgpt_account_id": "abc-123", "chatgpt_plan_type": "plus" } }
const SAMPLE_ID_TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJhY2N0X2FiYyIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhYmMtMTIzIiwiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIn19.sig";

#[test]
fn extracts_account_id_and_plan() {
    let claims = decode_codex_id_token_claims(SAMPLE_ID_TOKEN).expect("decodes");
    assert_eq!(claims.account_id, "abc-123");
    assert_eq!(claims.plan.as_deref(), Some("plus"));
}

#[test]
fn rejects_malformed_jwt() {
    assert!(decode_codex_id_token_claims("not.a.jwt.toomanyparts").is_err());
    assert!(decode_codex_id_token_claims("onlyone").is_err());
    assert!(decode_codex_id_token_claims("a.b.c").is_err()); // not base64
}

const AUTH_JSON: &str = r#"{
  "OPENAI_API_KEY": null,
  "tokens": {
    "id_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJhY2N0X2FiYyIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhYmMtMTIzIiwiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIn19.sig",
    "access_token": "at-xxx",
    "refresh_token": "rt-yyy",
    "account_id": "abc-123"
  },
  "last_refresh": "2026-04-27T00:00:00Z"
}"#;

#[test]
fn parses_auth_json_blob_into_codex_auth() {
    let auth = CodexAuth::from_auth_json(AUTH_JSON, Some("personal".to_string())).expect("parses");
    assert_eq!(auth.account_id, "abc-123");
    assert_eq!(auth.plan.as_deref(), Some("plus"));
    assert_eq!(auth.access_token, "at-xxx");
    assert_eq!(auth.refresh_token, "rt-yyy");
    assert_eq!(auth.label.as_deref(), Some("personal"));
    assert!(!auth.id.is_empty());
    assert!(auth.access_expires_at >= 0);
}
