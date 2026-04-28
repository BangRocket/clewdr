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
