# Codex Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Codex (OpenAI's `chatgpt.com/backend-api/codex/responses`) as a new backend provider in ClewdR, exposed as `POST /codex/v1/chat/completions` (OpenAI-compatible), with multi-credential pool, OAuth refresh, and an admin UI for credential CRUD.

**Architecture:** Mirror the existing `claude_code_state` + `cookie_actor` + `providers/claude` shape. Separate `CodexAuth` type (not shared with `CookieStatus`), separate `CodexAuthActor`, new `codex_state` module, new `providers/codex` provider implementing existing `LLMProvider` trait. Translation layer converts OpenAI ChatCompletions ↔ Codex Responses-API both directions including SSE streams.

**Tech Stack:** Rust 2024, axum 0.8, wreq 6 (HTTP w/ Chrome fingerprint emulation), ractor 0.15 (actor model), tokio, serde, eventsource-stream. Frontend: existing React + Vite stack in `frontend/`.

**Reference patterns when boilerplate is omitted:**
- `src/services/cookie_actor.rs` for actor pattern
- `src/claude_code_state/` for state-machine + retry pattern
- `src/providers/claude/mod.rs` for provider impl
- `src/api/cookie.rs` (or equivalent) for admin handlers
- `frontend/src/components/cookies/` (or equivalent) for tab structure

**Design doc:** `docs/plans/2026-04-27-codex-support-design.md` — read before starting.

**Skills to use during execution:**
- `superpowers:test-driven-development` — every task is red-green-commit
- `superpowers:systematic-debugging` — when something breaks
- `superpowers:verification-before-completion` — before marking any task done

---

## Phase 0: Setup & guardrails

### Task 0.1: Verify branch + clean tree

**Step 1:** Run `git status` and `git branch --show-current`. Expect: `feat/codex-support`, clean working tree (design doc already committed).

**Step 2:** Run `cargo check` to confirm baseline compiles. Expected: success, no errors. If fails, STOP and report — pre-existing breakage.

**Step 3:** Run `cargo test --no-run` to confirm tests build. If fails, STOP.

### Task 0.2: Add `wiremock` dev-dependency

**Files:**
- Modify: `Cargo.toml` (`[dev-dependencies]` section, append/create)

**Step 1:** Add to `[dev-dependencies]`:
```toml
[dev-dependencies]
wiremock = "0.6"
```
(Add the section if it doesn't exist.)

**Step 2:** Run `cargo build --tests`. Expect: wiremock pulled and compiled.

**Step 3:** Commit:
```bash
git add Cargo.toml Cargo.lock
git commit -m "chore(deps): add wiremock for codex integration tests"
```

---

## Phase 1: `CodexAuth` type + status + serde

### Task 1.1: Failing serde round-trip test

**Files:**
- Create: `src/config/codex_auth.rs`
- Test: `tests/codex_auth_serde.rs`

**Step 1:** Create `tests/codex_auth_serde.rs` with:
```rust
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
```

**Step 2:** Run `cargo test --test codex_auth_serde`. Expected: FAIL (`CodexAuth` does not exist).

### Task 1.2: Implement `CodexAuth` + `CodexAuthStatus`

**Step 1:** Create `src/config/codex_auth.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CodexAuth {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: i64,
    pub account_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(default)]
    pub status: CodexAuthStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CodexAuthStatus {
    Valid,
    RateLimited { until: i64 },
    Expired,
    Invalid,
    Banned,
}

impl Default for CodexAuthStatus {
    fn default() -> Self {
        CodexAuthStatus::Valid
    }
}

impl CodexAuth {
    pub fn id_prefix(&self) -> &str {
        &self.id[..self.id.len().min(8)]
    }

    pub fn is_dispatchable(&self, now: i64) -> bool {
        match &self.status {
            CodexAuthStatus::Valid | CodexAuthStatus::Expired => true,
            CodexAuthStatus::RateLimited { until } => *until <= now,
            CodexAuthStatus::Invalid | CodexAuthStatus::Banned => false,
        }
    }
}
```

**Step 2:** Add to `src/config/mod.rs`:
```rust
mod codex_auth;
pub use codex_auth::*;
```

**Step 3:** Run `cargo test --test codex_auth_serde`. Expected: PASS both tests.

**Step 4:** Commit:
```bash
git add src/config/codex_auth.rs src/config/mod.rs tests/codex_auth_serde.rs
git commit -m "feat(codex): CodexAuth type with status enum and serde round-trip"
```

### Task 1.3: Test for `is_dispatchable`

**Step 1:** Add to `tests/codex_auth_serde.rs`:
```rust
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
    a.status = CodexAuthStatus::Expired; // refreshable
    assert!(a.is_dispatchable(100));
    a.status = CodexAuthStatus::RateLimited { until: 200 };
    assert!(!a.is_dispatchable(100));
    assert!(a.is_dispatchable(300));
}
```

**Step 2:** Run `cargo test --test codex_auth_serde dispatchable_logic`. Expected: PASS.

**Step 3:** Commit:
```bash
git add tests/codex_auth_serde.rs
git commit -m "test(codex): is_dispatchable covers all status variants"
```

---

## Phase 2: JWT claim parsing (no new deps)

### Task 2.1: Failing test for JWT claim extraction

**Files:**
- Create: `src/config/codex_auth.rs` (extend)
- Test: `tests/codex_jwt_claims.rs`

**Step 1:** Create `tests/codex_jwt_claims.rs`:
```rust
use clewdr::config::decode_codex_id_token_claims;

// Sample id_token payload: { "sub": "acct_abc", "https://api.openai.com/auth": { "chatgpt_account_id": "abc-123", "chatgpt_plan_type": "plus" } }
// header.payload.signature -- payload base64url-encoded
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
```

**Step 2:** Run `cargo test --test codex_jwt_claims`. Expected: FAIL (function doesn't exist).

### Task 2.2: Implement `decode_codex_id_token_claims`

**Step 1:** Append to `src/config/codex_auth.rs`:
```rust
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

#[derive(Debug, Clone)]
pub struct CodexIdTokenClaims {
    pub account_id: String,
    pub plan: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CodexJwtError {
    #[error("expected 3 segments, got {0}")]
    Segments(usize),
    #[error("base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("json parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing chatgpt_account_id claim")]
    MissingAccountId,
}

pub fn decode_codex_id_token_claims(token: &str) -> Result<CodexIdTokenClaims, CodexJwtError> {
    let segments: Vec<&str> = token.split('.').collect();
    if segments.len() != 3 {
        return Err(CodexJwtError::Segments(segments.len()));
    }
    let payload_bytes = URL_SAFE_NO_PAD.decode(segments[1])?;
    let v: serde_json::Value = serde_json::from_slice(&payload_bytes)?;

    let auth_obj = v.get("https://api.openai.com/auth").and_then(|x| x.as_object());
    let account_id = auth_obj
        .and_then(|m| m.get("chatgpt_account_id"))
        .and_then(|x| x.as_str())
        .ok_or(CodexJwtError::MissingAccountId)?
        .to_string();
    let plan = auth_obj
        .and_then(|m| m.get("chatgpt_plan_type"))
        .and_then(|x| x.as_str())
        .map(str::to_string);

    Ok(CodexIdTokenClaims { account_id, plan })
}
```

**Step 2:** Confirm `thiserror` and `base64` already in Cargo.toml (they are).

**Step 3:** Run `cargo test --test codex_jwt_claims`. Expected: PASS both tests.

**Step 4:** Commit:
```bash
git add src/config/codex_auth.rs tests/codex_jwt_claims.rs
git commit -m "feat(codex): decode_codex_id_token_claims extracts account_id and plan"
```

### Task 2.3: Helper to build `CodexAuth` from auth.json blob

**Step 1:** Add failing test to `tests/codex_jwt_claims.rs`:
```rust
use clewdr::config::CodexAuth;

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
    assert!(auth.access_expires_at >= 0); // sanely set
}
```

**Step 2:** Run `cargo test --test codex_jwt_claims parses_auth_json_blob_into_codex_auth`. Expected: FAIL.

**Step 3:** Append to `src/config/codex_auth.rs`:
```rust
use serde::Deserialize as _;
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum CodexAuthParseError {
    #[error("auth.json malformed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing tokens.{field}")]
    MissingField { field: &'static str },
    #[error("id_token claims invalid: {0}")]
    Claims(#[from] CodexJwtError),
}

impl CodexAuth {
    pub fn from_auth_json(body: &str, label: Option<String>) -> Result<Self, CodexAuthParseError> {
        let v: serde_json::Value = serde_json::from_str(body)?;
        let tokens = v.get("tokens").ok_or(CodexAuthParseError::MissingField { field: "tokens" })?;

        let get = |k: &'static str| -> Result<String, CodexAuthParseError> {
            tokens.get(k).and_then(|x| x.as_str()).map(str::to_string)
                .ok_or(CodexAuthParseError::MissingField { field: k })
        };
        let id_token = get("id_token")?;
        let access_token = get("access_token")?;
        let refresh_token = get("refresh_token")?;

        let claims = decode_codex_id_token_claims(&id_token)?;
        let account_id = tokens
            .get("account_id")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or(claims.account_id);

        // Stable id = first 16 hex chars of sha256(refresh_token)
        let mut hasher = Sha256::new();
        hasher.update(refresh_token.as_bytes());
        let id = hex_first_n(&hasher.finalize(), 16);

        // Conservative: assume access_token already nearly expired so first request triggers refresh.
        let access_expires_at = 0;

        Ok(CodexAuth {
            id,
            label,
            id_token,
            access_token,
            refresh_token,
            access_expires_at,
            account_id,
            plan: claims.plan,
            status: CodexAuthStatus::Valid,
            last_used_at: None,
        })
    }
}

fn hex_first_n(bytes: &[u8], n: usize) -> String {
    let mut s = String::with_capacity(n);
    for b in bytes {
        if s.len() >= n { break; }
        s.push_str(&format!("{:02x}", b));
    }
    s.truncate(n);
    s
}
```

**Step 4:** Add `sha2 = "0.10"` to `[dependencies]` in `Cargo.toml` (verify it's not already there with `cargo tree | grep sha2`; if present transitively, add it explicitly anyway since we need it as a direct dep).

**Step 5:** Run `cargo test --test codex_jwt_claims parses_auth_json_blob_into_codex_auth`. Expected: PASS.

**Step 6:** Commit:
```bash
git add Cargo.toml Cargo.lock src/config/codex_auth.rs tests/codex_jwt_claims.rs
git commit -m "feat(codex): CodexAuth::from_auth_json parses ~/.codex/auth.json blob"
```

---

## Phase 3: Config integration

### Task 3.1: Add `codex_auth` field to `ClewdrConfig`

**Files:**
- Modify: `src/config/clewdr_config.rs`

**Step 1:** Read `src/config/clewdr_config.rs` to find where `cookie_array: Vec<CookieStatus>` is declared.

**Step 2:** Add field next to `cookie_array`:
```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub codex_auth: Vec<CodexAuth>,
```

**Step 3:** Add `use` for `CodexAuth` at top of file (look for existing `use crate::config::...` pattern).

**Step 4:** Run `cargo check`. Expected: PASS. If fails, the `Default` impl for `ClewdrConfig` may need a `codex_auth: Vec::new()` line.

**Step 5:** Run existing config tests if any: `cargo test config`. Expected: PASS.

**Step 6:** Commit:
```bash
git add src/config/clewdr_config.rs
git commit -m "feat(codex): add codex_auth array to ClewdrConfig"
```

### Task 3.2: Test config persistence round-trip

**Files:**
- Create: `tests/codex_config_roundtrip.rs`

**Step 1:** Write test:
```rust
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
```

**Step 2:** Run `cargo test --test codex_config_roundtrip`. Expected: PASS.

**Step 3:** Commit:
```bash
git add tests/codex_config_roundtrip.rs
git commit -m "test(codex): config with codex_auth survives toml round-trip"
```

---

## Phase 4: Codex Responses-API types

### Task 4.1: Define request body types

**Files:**
- Create: `src/types/codex/mod.rs`
- Modify: `src/types/mod.rs` (add `pub mod codex;`)

**Step 1:** Create `src/types/codex/mod.rs`:
```rust
//! Minimal types for Codex `/backend-api/codex/responses` request and SSE events.
//! Only the subset needed for translation is modeled.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct CodexRequest {
    pub model: String,
    pub input: Vec<CodexInputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<CodexText>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CodexInputItem {
    Message { role: String, content: Vec<CodexContent> },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CodexContent {
    InputText { text: String },
    OutputText { text: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexText {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<serde_json::Value>,
}

// SSE event payload — partial, only fields we use.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum CodexSseEvent {
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta { delta: String },
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded { item: serde_json::Value },
    #[serde(rename = "response.completed")]
    Completed { response: CodexFinalResponse },
    #[serde(rename = "response.error")]
    Error { message: String, code: Option<String> },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexFinalResponse {
    #[serde(default)]
    pub usage: Option<CodexUsage>,
    #[serde(default)]
    pub output: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: Option<u64>,
}
```

**Step 2:** Edit `src/types/mod.rs`:
```rust
pub mod codex;
```

**Step 3:** Run `cargo check`. Expected: PASS.

**Step 4:** Commit:
```bash
git add src/types/codex/mod.rs src/types/mod.rs
git commit -m "feat(codex): minimal Responses-API request/SSE event types"
```

---

## Phase 5: Request translation

### Task 5.1: Failing test — basic ChatCompletions → Codex request

**Files:**
- Create: `src/codex_state/transform.rs` (will be created in 5.2)
- Test: `tests/codex_transform_request.rs`

**Step 1:** Write `tests/codex_transform_request.rs`:
```rust
use clewdr::codex_state::transform::translate_chat_completions_to_codex;
use clewdr::types::oai::ChatCompletionsRequest; // existing OAI request type
// (If naming differs, check src/types/oai.rs and use the actual symbol.)

#[test]
fn translates_simple_user_message() {
    let oai = serde_json::json!({
        "model": "gpt-5",
        "messages": [
            {"role": "user", "content": "hello world"}
        ],
        "stream": false
    });
    let oai: ChatCompletionsRequest = serde_json::from_value(oai).unwrap();
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
    let oai: ChatCompletionsRequest = serde_json::from_value(oai).unwrap();
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
    let oai: ChatCompletionsRequest = serde_json::from_value(oai).unwrap();
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
    let oai: ChatCompletionsRequest = serde_json::from_value(oai).unwrap();
    let codex = translate_chat_completions_to_codex(&oai).expect("translates");
    assert_eq!(codex.max_output_tokens, Some(256));
}
```

**Step 2:** Run `cargo test --test codex_transform_request`. Expected: FAIL (`codex_state::transform` doesn't exist; verify symbol name for ChatCompletionsRequest first by reading `src/types/oai.rs`).

**Note for executor:** If `ChatCompletionsRequest` is named differently (likely something like `OaiRequest` or similar), update the use statement and JSON shape accordingly. Read `src/types/oai.rs` first.

### Task 5.2: Implement `translate_chat_completions_to_codex`

**Files:**
- Create: `src/codex_state/mod.rs` (just `pub mod transform;` for now)
- Create: `src/codex_state/transform.rs`
- Modify: `src/lib.rs` (add `pub mod codex_state;` and `pub mod types::codex;` exposure if needed)

**Step 1:** Create `src/codex_state/mod.rs`:
```rust
pub mod transform;
```

**Step 2:** Create `src/codex_state/transform.rs`:
```rust
use crate::types::codex::{CodexContent, CodexInputItem, CodexRequest, CodexText};
use crate::types::oai::ChatCompletionsRequest; // adjust if symbol differs
use snafu::Snafu;

pub const CODEX_MODELS: &[&str] = &[
    "gpt-5-codex",
    "gpt-5",
    "gpt-4.1",
    "o3",
    "o4-mini",
];

#[derive(Debug, Snafu)]
pub enum TranslateError {
    #[snafu(display("model `{model}` is not supported by Codex; valid: {valid}"))]
    UnknownModel { model: String, valid: String },
    #[snafu(display("messages array is empty"))]
    NoMessages,
}

pub fn translate_chat_completions_to_codex(
    req: &ChatCompletionsRequest,
) -> Result<CodexRequest, TranslateError> {
    if !CODEX_MODELS.iter().any(|m| *m == req.model) {
        return Err(TranslateError::UnknownModel {
            model: req.model.clone(),
            valid: CODEX_MODELS.join(", "),
        });
    }
    if req.messages.is_empty() {
        return Err(TranslateError::NoMessages);
    }

    // Concatenate system messages into instructions; pull non-system into input.
    let mut system_parts: Vec<String> = Vec::new();
    let mut input: Vec<CodexInputItem> = Vec::new();
    for m in &req.messages {
        // m.content may be string-or-array per OAI spec; assume helper exists or
        // call into existing utility. Adjust this loop to match the actual shape
        // of ChatCompletionsRequest::messages.
        let text = m.content_as_string(); // adjust per real API
        if m.role == "system" {
            system_parts.push(text);
        } else {
            let kind = if m.role == "assistant" {
                CodexContent::OutputText { text }
            } else {
                CodexContent::InputText { text }
            };
            input.push(CodexInputItem::Message {
                role: m.role.clone(),
                content: vec![kind],
            });
        }
    }

    let instructions = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };

    let text = req.response_format.as_ref().map(|fmt| CodexText {
        format: Some(fmt.clone()),
    });

    Ok(CodexRequest {
        model: req.model.clone(),
        input,
        instructions,
        temperature: req.temperature,
        top_p: req.top_p,
        max_output_tokens: req.max_tokens,
        tools: req.tools.clone().unwrap_or_default(),
        tool_choice: req.tool_choice.clone(),
        text,
        stream: req.stream.unwrap_or(false),
    })
}
```

**Step 3:** Modify `src/lib.rs` to expose `codex_state`:
```rust
pub mod codex_state;
```
(Find the section with other `pub mod ...;` lines.)

**Step 4:** Run `cargo check`. Likely fails because `m.content_as_string()` and field names need to match the real `ChatCompletionsRequest` shape. **Read `src/types/oai.rs` and adjust** — replace placeholder method/field names with the actual ones. Common shapes:
- If content is `OaiMessageContent` enum (string or parts), implement helper inline:
  ```rust
  let text = match &m.content {
      OaiMessageContent::Text(s) => s.clone(),
      OaiMessageContent::Parts(parts) => parts.iter()
          .filter_map(|p| p.text.as_deref())
          .collect::<Vec<_>>()
          .join("\n"),
  };
  ```

**Step 5:** Run `cargo test --test codex_transform_request`. Expected: PASS all 4 tests.

**Step 6:** Commit:
```bash
git add src/codex_state/mod.rs src/codex_state/transform.rs src/lib.rs tests/codex_transform_request.rs
git commit -m "feat(codex): translate OpenAI ChatCompletions request to Codex Responses-API"
```

### Task 5.3: Tool/tool_choice passthrough test

**Step 1:** Add to `tests/codex_transform_request.rs`:
```rust
#[test]
fn tools_pass_through_unchanged() {
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
    let oai: ChatCompletionsRequest = serde_json::from_value(oai).unwrap();
    let codex = translate_chat_completions_to_codex(&oai).expect("translates");
    assert_eq!(codex.tools, tools.as_array().unwrap().clone());
    assert_eq!(codex.tool_choice, Some(serde_json::json!("auto")));
}
```

**Step 2:** Run test. Expected: PASS (existing translation already handles passthrough).

**Step 3:** Commit:
```bash
git add tests/codex_transform_request.rs
git commit -m "test(codex): tools and tool_choice pass through translation"
```

---

## Phase 6: Response/SSE translation

### Task 6.1: Failing test — single delta event

**Files:**
- Test: `tests/codex_transform_response.rs`

**Step 1:** Write test:
```rust
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
```

**Step 2:** Run `cargo test --test codex_transform_response`. Expected: FAIL.

### Task 6.2: Implement `codex_event_to_oai_chunk`

**Step 1:** Append to `src/codex_state/transform.rs`:
```rust
use crate::types::codex::CodexSseEvent;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OaiChunk {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OaiChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OaiUsage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiChoice {
    pub index: u32,
    pub delta: OaiDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct OaiDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

pub fn codex_event_to_oai_chunk(
    event: &CodexSseEvent,
    id: &str,
    model: &str,
) -> Option<OaiChunk> {
    match event {
        CodexSseEvent::OutputTextDelta { delta } => Some(OaiChunk {
            id: id.to_string(),
            object: "chat.completion.chunk",
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![OaiChoice {
                index: 0,
                delta: OaiDelta {
                    role: None,
                    content: Some(delta.clone()),
                },
                finish_reason: None,
            }],
            usage: None,
        }),
        CodexSseEvent::Completed { response } => Some(OaiChunk {
            id: id.to_string(),
            object: "chat.completion.chunk",
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![OaiChoice {
                index: 0,
                delta: OaiDelta::default(),
                finish_reason: Some("stop".to_string()),
            }],
            usage: response.usage.as_ref().map(|u| OaiUsage {
                prompt_tokens: u.input_tokens,
                completion_tokens: u.output_tokens,
                total_tokens: u.total_tokens.unwrap_or(u.input_tokens + u.output_tokens),
            }),
        }),
        _ => None,
    }
}
```

**Step 2:** Run `cargo test --test codex_transform_response`. Expected: PASS.

**Step 3:** Commit:
```bash
git add src/codex_state/transform.rs tests/codex_transform_response.rs
git commit -m "feat(codex): translate Responses SSE events to OAI chat.completion.chunks"
```

### Task 6.3: Test — completed event with usage

**Step 1:** Add test:
```rust
#[test]
fn completed_event_emits_finish_reason_and_usage() {
    use clewdr::types::codex::{CodexFinalResponse, CodexUsage};
    let event = CodexSseEvent::Completed {
        response: CodexFinalResponse {
            usage: Some(CodexUsage { input_tokens: 100, output_tokens: 50, total_tokens: None }),
            output: vec![],
        },
    };
    let chunk = codex_event_to_oai_chunk(&event, "id-1", "gpt-5").expect("chunk");
    assert_eq!(chunk.choices[0].finish_reason.as_deref(), Some("stop"));
    let usage = chunk.usage.expect("usage");
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);
}
```

**Step 2:** Run test. Expected: PASS.

**Step 3:** Commit:
```bash
git add tests/codex_transform_response.rs
git commit -m "test(codex): completed event surfaces usage with computed total"
```

### Task 6.4: Non-streaming aggregation helper

**Step 1:** Failing test:
```rust
use clewdr::codex_state::transform::aggregate_codex_events;

#[test]
fn aggregates_deltas_into_full_completion() {
    let events = vec![
        CodexSseEvent::OutputTextDelta { delta: "Hello, ".to_string() },
        CodexSseEvent::OutputTextDelta { delta: "world!".to_string() },
        CodexSseEvent::Completed {
            response: clewdr::types::codex::CodexFinalResponse {
                usage: Some(clewdr::types::codex::CodexUsage {
                    input_tokens: 5, output_tokens: 3, total_tokens: Some(8),
                }),
                output: vec![],
            },
        },
    ];
    let resp = aggregate_codex_events(&events, "id", "gpt-5").expect("aggregates");
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["object"], "chat.completion");
    assert_eq!(v["choices"][0]["message"]["content"], "Hello, world!");
    assert_eq!(v["usage"]["total_tokens"], 8);
}
```

**Step 2:** Run. Expected: FAIL.

**Step 3:** Implement in `src/codex_state/transform.rs`:
```rust
#[derive(Debug, Clone, Serialize)]
pub struct OaiCompletion {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OaiCompletionChoice>,
    pub usage: OaiUsage,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiCompletionChoice {
    pub index: u32,
    pub message: OaiCompletionMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OaiCompletionMessage {
    pub role: String,
    pub content: String,
}

pub fn aggregate_codex_events(
    events: &[CodexSseEvent],
    id: &str,
    model: &str,
) -> Result<OaiCompletion, TranslateError> {
    let mut content = String::new();
    let mut usage = OaiUsage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };
    for ev in events {
        match ev {
            CodexSseEvent::OutputTextDelta { delta } => content.push_str(delta),
            CodexSseEvent::Completed { response } => {
                if let Some(u) = &response.usage {
                    usage = OaiUsage {
                        prompt_tokens: u.input_tokens,
                        completion_tokens: u.output_tokens,
                        total_tokens: u.total_tokens.unwrap_or(u.input_tokens + u.output_tokens),
                    };
                }
            }
            _ => {}
        }
    }
    Ok(OaiCompletion {
        id: id.to_string(),
        object: "chat.completion",
        created: chrono::Utc::now().timestamp(),
        model: model.to_string(),
        choices: vec![OaiCompletionChoice {
            index: 0,
            message: OaiCompletionMessage {
                role: "assistant".to_string(),
                content,
            },
            finish_reason: "stop".to_string(),
        }],
        usage,
    })
}
```

**Step 4:** Run test. Expected: PASS.

**Step 5:** Commit:
```bash
git add src/codex_state/transform.rs tests/codex_transform_response.rs
git commit -m "feat(codex): aggregate_codex_events for non-streaming responses"
```

---

## Phase 7: `CodexAuthActor`

### Task 7.1: Define `CodexAuthActorHandle` skeleton + start/dispatch tests

**Files:**
- Create: `src/services/codex_auth_actor.rs`
- Modify: `src/services/mod.rs`
- Test: `tests/codex_auth_actor.rs`

**Step 1:** Write a minimal failing test:
```rust
use clewdr::config::{CodexAuth, CodexAuthStatus};
use clewdr::services::codex_auth_actor::CodexAuthActorHandle;

fn dummy_auth(id: &str) -> CodexAuth {
    CodexAuth {
        id: id.into(), label: None, id_token: "x.y.z".into(),
        access_token: "at".into(), refresh_token: "rt".into(),
        access_expires_at: i64::MAX, // never expires for test
        account_id: "acct".into(), plan: None,
        status: CodexAuthStatus::Valid, last_used_at: None,
    }
}

#[tokio::test]
async fn dispatch_returns_valid_credential() {
    let handle = CodexAuthActorHandle::start_with(vec![dummy_auth("a"), dummy_auth("b")])
        .await
        .expect("start");
    let auth = handle.request().await.expect("dispatch");
    assert!(auth.id == "a" || auth.id == "b");
}

#[tokio::test]
async fn dispatch_skips_banned() {
    let mut a = dummy_auth("a");
    a.status = CodexAuthStatus::Banned;
    let b = dummy_auth("b");
    let handle = CodexAuthActorHandle::start_with(vec![a, b]).await.unwrap();
    let auth = handle.request().await.expect("dispatch");
    assert_eq!(auth.id, "b");
}

#[tokio::test]
async fn dispatch_fails_when_pool_empty() {
    let handle = CodexAuthActorHandle::start_with(vec![]).await.unwrap();
    assert!(handle.request().await.is_err());
}
```

**Step 2:** Run `cargo test --test codex_auth_actor`. Expected: FAIL.

### Task 7.2: Implement actor (mirror `cookie_actor` shape)

**Step 1:** Create `src/services/codex_auth_actor.rs`. Use `cookie_actor.rs` as template — copy and adapt:
```rust
use std::collections::VecDeque;

use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use snafu::{GenerateImplicitData, Location};
use tracing::{info, warn};

use crate::config::{CodexAuth, CodexAuthStatus};
use crate::error::ClewdrError;

#[derive(Debug)]
enum Msg {
    Request(RpcReplyPort<Result<CodexAuth, ClewdrError>>),
    Return(CodexAuth),
    Submit(CodexAuth, RpcReplyPort<Result<(), ClewdrError>>),
    Delete(String, RpcReplyPort<Result<(), ClewdrError>>),
    List(RpcReplyPort<Vec<CodexAuth>>),
}

#[derive(Debug)]
struct State {
    pool: VecDeque<CodexAuth>,
}

struct CodexAuthActor;

impl CodexAuthActor {
    fn dispatch(state: &mut State) -> Result<CodexAuth, ClewdrError> {
        let now = chrono::Utc::now().timestamp();
        let n = state.pool.len();
        for _ in 0..n {
            let Some(mut c) = state.pool.pop_front() else { break; };
            if c.is_dispatchable(now) {
                c.last_used_at = Some(now);
                let returning = c.clone();
                state.pool.push_back(c);
                return Ok(returning);
            }
            // not dispatchable — keep in pool but rotate
            state.pool.push_back(c);
        }
        Err(ClewdrError::NoCookieAvailable {
            loc: Location::generate(),
        })
        // ^ Reuse existing error variant if present; if not, add a new variant
        // ClewdrError::NoCodexAuthAvailable in src/error.rs.
    }

    fn collect(state: &mut State, returned: CodexAuth) {
        // replace by id
        if let Some(idx) = state.pool.iter().position(|x| x.id == returned.id) {
            state.pool[idx] = returned;
        } else {
            state.pool.push_back(returned);
        }
        Self::persist(state);
    }

    fn persist(state: &State) {
        use crate::config::{CLEWDR_CONFIG, ClewdrConfig};
        let snapshot: Vec<CodexAuth> = state.pool.iter().cloned().collect();
        CLEWDR_CONFIG.rcu(|cfg| {
            let mut cfg = ClewdrConfig::clone(cfg);
            cfg.codex_auth = snapshot.clone();
            cfg
        });
        tokio::spawn(async move {
            if let Err(e) = crate::config::CLEWDR_CONFIG.load().save().await {
                warn!("codex_auth save failed: {e}");
            }
        });
    }
}

impl Actor for CodexAuthActor {
    type Msg = Msg;
    type State = State;
    type Arguments = Vec<CodexAuth>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        seed: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        info!("CodexAuthActor starting with {} creds", seed.len());
        Ok(State { pool: VecDeque::from(seed) })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::Request(reply) => { let r = Self::dispatch(state); reply.send(r)?; }
            Msg::Return(c) => { Self::collect(state, c); }
            Msg::Submit(c, reply) => {
                if state.pool.iter().any(|x| x.id == c.id) {
                    reply.send(Err(ClewdrError::DuplicateCookie { loc: Location::generate() }))?;
                } else {
                    state.pool.push_back(c);
                    Self::persist(state);
                    reply.send(Ok(()))?;
                }
            }
            Msg::Delete(id, reply) => {
                let before = state.pool.len();
                state.pool.retain(|x| x.id != id);
                if state.pool.len() < before {
                    Self::persist(state);
                    reply.send(Ok(()))?;
                } else {
                    reply.send(Err(ClewdrError::CookieNotFound { loc: Location::generate() }))?;
                }
            }
            Msg::List(reply) => {
                reply.send(state.pool.iter().cloned().collect())?;
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct CodexAuthActorHandle {
    actor_ref: ActorRef<Msg>,
}

impl CodexAuthActorHandle {
    pub async fn start() -> Result<Self, ractor::SpawnErr> {
        let seed = crate::config::CLEWDR_CONFIG.load().codex_auth.clone();
        Self::start_with(seed).await
    }

    pub async fn start_with(seed: Vec<CodexAuth>) -> Result<Self, ractor::SpawnErr> {
        let (actor_ref, _) = Actor::spawn(None, CodexAuthActor, seed).await?;
        Ok(Self { actor_ref })
    }

    pub async fn request(&self) -> Result<CodexAuth, ClewdrError> {
        ractor::call!(self.actor_ref, Msg::Request).map_err(|e| ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("codex_auth request: {e}"),
        })?
    }

    pub async fn return_auth(&self, auth: CodexAuth) -> Result<(), ClewdrError> {
        ractor::cast!(self.actor_ref, Msg::Return(auth)).map_err(|e| ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("codex_auth return: {e}"),
        })
    }

    pub async fn submit(&self, auth: CodexAuth) -> Result<(), ClewdrError> {
        ractor::call!(self.actor_ref, Msg::Submit, auth).map_err(|e| ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("codex_auth submit: {e}"),
        })?
    }

    pub async fn delete(&self, id: String) -> Result<(), ClewdrError> {
        ractor::call!(self.actor_ref, Msg::Delete, id).map_err(|e| ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("codex_auth delete: {e}"),
        })?
    }

    pub async fn list(&self) -> Result<Vec<CodexAuth>, ClewdrError> {
        ractor::call!(self.actor_ref, Msg::List).map_err(|e| ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("codex_auth list: {e}"),
        })
    }
}
```

**Step 2:** Add `pub mod codex_auth_actor;` to `src/services/mod.rs`.

**Step 3:** Check `src/error.rs` — likely you'll need to either reuse `NoCookieAvailable` / `DuplicateCookie` / `CookieNotFound` (acceptable, slightly mis-named) or add new variants `NoCodexAuthAvailable` / `DuplicateCodexAuth` / `CodexAuthNotFound`. **Recommended:** add new variants to keep error messages unambiguous.

**Step 4:** Run `cargo test --test codex_auth_actor`. Expected: PASS all 3 tests.

**Step 5:** Commit:
```bash
git add src/services/codex_auth_actor.rs src/services/mod.rs src/error.rs tests/codex_auth_actor.rs
git commit -m "feat(codex): CodexAuthActor with pool, dispatch, submit, delete"
```

### Task 7.3: Test — submit duplicate is rejected

**Step 1:** Add test:
```rust
#[tokio::test]
async fn submit_duplicate_id_is_rejected() {
    let handle = CodexAuthActorHandle::start_with(vec![dummy_auth("a")]).await.unwrap();
    assert!(handle.submit(dummy_auth("a")).await.is_err());
    assert!(handle.submit(dummy_auth("b")).await.is_ok());
}

#[tokio::test]
async fn delete_removes_from_pool() {
    let handle = CodexAuthActorHandle::start_with(vec![dummy_auth("a"), dummy_auth("b")]).await.unwrap();
    handle.delete("a".into()).await.expect("delete");
    let list = handle.list().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "b");
}
```

**Step 2:** Run. Expected: PASS.

**Step 3:** Commit:
```bash
git add tests/codex_auth_actor.rs
git commit -m "test(codex): submit dedup and delete coverage"
```

---

## Phase 8: OAuth refresh

### Task 8.1: Refresh function — failing wiremock test

**Files:**
- Test: `tests/codex_refresh.rs`

**Step 1:** Write failing test:
```rust
use clewdr::codex_state::refresh::{refresh_codex_token, RefreshOutcome};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn refresh_success_returns_new_tokens() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id_token": "eyJ.new.id",
            "access_token": "new-at",
            "refresh_token": "new-rt",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;

    let result = refresh_codex_token(&server.uri(), "old-rt", None).await;
    let RefreshOutcome::Refreshed { id_token, access_token, refresh_token, expires_at }
        = result.expect("ok") else { panic!("expected Refreshed") };
    assert_eq!(access_token, "new-at");
    assert_eq!(refresh_token, "new-rt");
    assert_eq!(id_token, "eyJ.new.id");
    let now = chrono::Utc::now().timestamp();
    assert!(expires_at > now + 3500 && expires_at < now + 3700);
}

#[tokio::test]
async fn refresh_invalid_grant_returns_invalid() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant"
        })))
        .mount(&server)
        .await;

    let outcome = refresh_codex_token(&server.uri(), "bad-rt", None).await.expect("ok");
    assert!(matches!(outcome, RefreshOutcome::Invalid));
}

#[tokio::test]
async fn refresh_5xx_returns_transient() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let outcome = refresh_codex_token(&server.uri(), "rt", None).await.expect("ok");
    assert!(matches!(outcome, RefreshOutcome::Transient));
}
```

**Step 2:** Run. Expected: FAIL (module doesn't exist).

### Task 8.2: Implement refresh

**Files:**
- Create: `src/codex_state/refresh.rs`
- Modify: `src/codex_state/mod.rs` (add `pub mod refresh;`)

**Step 1:** Create `src/codex_state/refresh.rs`:
```rust
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

const CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann"; // codex CLI public client_id

#[derive(Debug, Snafu)]
pub enum RefreshError {
    #[snafu(display("http request failed: {source}"))]
    Http { source: wreq::Error },
    #[snafu(display("response body decode failed: {source}"))]
    Decode { source: wreq::Error },
}

#[derive(Debug, Clone)]
pub enum RefreshOutcome {
    Refreshed {
        id_token: String,
        access_token: String,
        refresh_token: String,
        expires_at: i64,
    },
    Invalid,    // refresh_token rejected; user must re-login
    Transient,  // 5xx/network; safe to retry later
}

#[derive(Debug, Serialize)]
struct RefreshRequest<'a> {
    client_id: &'a str,
    grant_type: &'a str,
    refresh_token: &'a str,
    scope: &'a str,
}

#[derive(Debug, Deserialize)]
struct RefreshResponseOk {
    id_token: String,
    access_token: String,
    refresh_token: Option<String>, // OpenAI rotates, sometimes returns new
    expires_in: i64,
}

pub async fn refresh_codex_token(
    base_url: &str,
    refresh_token: &str,
    proxy: Option<&wreq::Proxy>,
) -> Result<RefreshOutcome, RefreshError> {
    let mut builder = wreq::Client::builder();
    if let Some(p) = proxy {
        builder = builder.proxy(p.clone());
    }
    let client = builder.build().context(HttpSnafu)?;

    let body = RefreshRequest {
        client_id: CODEX_OAUTH_CLIENT_ID,
        grant_type: "refresh_token",
        refresh_token,
        scope: "openid profile email offline_access",
    };

    let url = format!("{}/oauth/token", base_url.trim_end_matches('/'));
    let resp = client.post(url).json(&body).send().await.context(HttpSnafu)?;

    let status = resp.status();
    if status.is_success() {
        let parsed: RefreshResponseOk = resp.json().await.context(DecodeSnafu)?;
        Ok(RefreshOutcome::Refreshed {
            id_token: parsed.id_token,
            access_token: parsed.access_token,
            refresh_token: parsed.refresh_token.unwrap_or_else(|| refresh_token.to_string()),
            expires_at: chrono::Utc::now().timestamp() + parsed.expires_in,
        })
    } else if status.as_u16() == 400 || status.as_u16() == 401 {
        Ok(RefreshOutcome::Invalid)
    } else {
        Ok(RefreshOutcome::Transient)
    }
}
```

**Step 2:** Add `pub mod refresh;` to `src/codex_state/mod.rs`.

**Step 3:** Run `cargo test --test codex_refresh`. Expected: PASS all 3 tests.

**Step 4:** Commit:
```bash
git add src/codex_state/refresh.rs src/codex_state/mod.rs tests/codex_refresh.rs
git commit -m "feat(codex): OAuth refresh with classify outcomes (refreshed/invalid/transient)"
```

### Task 8.3: Pull `client_id` from a constant

**Step 1:** The literal `app_EMoamEEZ73f0CkXaXp7hrann` is the public OAuth client_id used by the official `codex` CLI (verifiable by reading codex source). Move to `src/config/constants.rs`:
```rust
pub const CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const CODEX_OAUTH_BASE_URL: &str = "https://auth.openai.com";
pub const CODEX_API_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
```

**Step 2:** Update `src/codex_state/refresh.rs` to import from constants. Tests still pass (use `&server.uri()` for base_url).

**Step 3:** Run `cargo test --test codex_refresh`. Expected: PASS.

**Step 4:** Commit:
```bash
git add src/codex_state/refresh.rs src/config/constants.rs
git commit -m "refactor(codex): centralize Codex OAuth and API URLs in constants"
```

---

## Phase 9: `CodexState` (HTTP layer)

### Task 9.1: Skeleton struct

**Files:**
- Modify: `src/codex_state/mod.rs`

**Step 1:** Replace `src/codex_state/mod.rs`:
```rust
pub mod transform;
pub mod refresh;
pub mod chat;

use std::sync::LazyLock;

use http::{HeaderValue, header::AUTHORIZATION};
use snafu::ResultExt;
use tracing::warn;
use wreq::{Client, Method, Proxy, RequestBuilder};
use wreq_util::Emulation;

use crate::{
    config::{
        CLEWDR_CONFIG, CODEX_API_BASE_URL, CODEX_OAUTH_BASE_URL, CodexAuth, CodexAuthStatus,
    },
    error::{ClewdrError, WreqSnafu},
    services::codex_auth_actor::CodexAuthActorHandle,
};

pub static CODEX_CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

#[derive(Clone)]
pub struct CodexState {
    pub auth_actor: CodexAuthActorHandle,
    pub auth: Option<CodexAuth>,
    pub proxy: Option<Proxy>,
    pub client: Client,
    pub stream: bool,
    pub api_base: String,
    pub oauth_base: String,
}

impl CodexState {
    pub fn new(auth_actor: CodexAuthActorHandle) -> Self {
        Self {
            auth_actor,
            auth: None,
            proxy: CLEWDR_CONFIG.load().wreq_proxy.to_owned(),
            client: CODEX_CLIENT.to_owned(),
            stream: false,
            api_base: CODEX_API_BASE_URL.to_string(),
            oauth_base: CODEX_OAUTH_BASE_URL.to_string(),
        }
    }

    pub async fn request_auth(&mut self) -> Result<CodexAuth, ClewdrError> {
        let auth = self.auth_actor.request().await?;
        self.auth = Some(auth.clone());
        // refresh client with current proxy
        let mut builder = Client::builder().emulation(Emulation::Chrome136);
        if let Some(ref p) = self.proxy {
            builder = builder.proxy(p.clone());
        }
        self.client = builder.build().context(WreqSnafu {
            msg: "build codex client",
        })?;
        Ok(auth)
    }

    pub async fn return_auth(&mut self) {
        if let Some(auth) = self.auth.take() {
            if let Err(e) = self.auth_actor.return_auth(auth).await {
                warn!("codex return_auth failed: {e}");
            }
        }
    }

    pub fn build_request(&self, method: Method, url: impl ToString) -> RequestBuilder {
        let mut req = self.client.request(method, url.to_string());
        if let Some(auth) = self.auth.as_ref() {
            let bearer = format!("Bearer {}", auth.access_token);
            if let Ok(v) = HeaderValue::from_str(&bearer) {
                req = req.header(AUTHORIZATION, v);
            }
            if let Ok(v) = HeaderValue::from_str(&auth.account_id) {
                req = req.header("ChatGPT-Account-Id", v);
            }
        }
        req
    }

    /// Refresh access_token if within 60s of expiry. Returns Err and marks auth
    /// status if the refresh definitively fails.
    pub async fn ensure_fresh_access_token(&mut self) -> Result<(), ClewdrError> {
        let now = chrono::Utc::now().timestamp();
        let needs_refresh = self
            .auth
            .as_ref()
            .map(|a| a.access_expires_at.saturating_sub(now) < 60)
            .unwrap_or(false);
        if !needs_refresh {
            return Ok(());
        }
        let Some(auth) = self.auth.as_mut() else {
            return Ok(());
        };
        let outcome = crate::codex_state::refresh::refresh_codex_token(
            &self.oauth_base,
            &auth.refresh_token,
            self.proxy.as_ref(),
        )
        .await
        .map_err(|e| ClewdrError::Other {
            loc: snafu::location!(),
            msg: format!("codex refresh: {e}"),
        })?;
        match outcome {
            crate::codex_state::refresh::RefreshOutcome::Refreshed {
                id_token,
                access_token,
                refresh_token,
                expires_at,
            } => {
                auth.id_token = id_token;
                auth.access_token = access_token;
                auth.refresh_token = refresh_token;
                auth.access_expires_at = expires_at;
                auth.status = CodexAuthStatus::Valid;
                Ok(())
            }
            crate::codex_state::refresh::RefreshOutcome::Invalid => {
                auth.status = CodexAuthStatus::Invalid;
                Err(ClewdrError::Other {
                    loc: snafu::location!(),
                    msg: "codex refresh_token rejected; re-login required".to_string(),
                })
            }
            crate::codex_state::refresh::RefreshOutcome::Transient => {
                auth.status = CodexAuthStatus::Expired;
                Err(ClewdrError::Other {
                    loc: snafu::location!(),
                    msg: "codex token endpoint transient failure".to_string(),
                })
            }
        }
    }
}
```

**Step 2:** Verify `ClewdrError::Other` variant exists; if not, use whatever generic error variant is in `src/error.rs` (or add one). Adjust accordingly.

**Step 3:** Run `cargo check`. Expected: PASS.

**Step 4:** Commit:
```bash
git add src/codex_state/mod.rs
git commit -m "feat(codex): CodexState with auth-actor wiring, request builder, refresh gate"
```

---

## Phase 10: `try_chat`

### Task 10.1: Failing wiremock integration test — happy path streaming

**Files:**
- Create: `src/codex_state/chat.rs` (in 10.2)
- Test: `tests/codex_chat_e2e.rs`

**Step 1:** Write test:
```rust
use clewdr::codex_state::CodexState;
use clewdr::config::{CodexAuth, CodexAuthStatus};
use clewdr::services::codex_auth_actor::CodexAuthActorHandle;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn auth_for_test() -> CodexAuth {
    CodexAuth {
        id: "test-id".into(), label: None,
        id_token: "x.y.z".into(),
        access_token: "valid-at".into(),
        refresh_token: "rt".into(),
        access_expires_at: i64::MAX,
        account_id: "acct".into(), plan: None,
        status: CodexAuthStatus::Valid, last_used_at: None,
    }
}

#[tokio::test]
async fn streams_codex_sse_into_oai_chunks() {
    let api = MockServer::start().await;
    let sse_body =
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"hi\"}\n\n\
         event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n";
    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(header("authorization", "Bearer valid-at"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body),
        )
        .mount(&api)
        .await;

    let actor = CodexAuthActorHandle::start_with(vec![auth_for_test()]).await.unwrap();
    let mut state = CodexState::new(actor);
    state.api_base = api.uri();
    state.stream = true;

    let req = serde_json::json!({
        "model": "gpt-5",
        "messages": [{"role": "user", "content": "hi"}],
        "stream": true
    });
    let parsed = serde_json::from_value(req).unwrap();
    let response = state.try_chat(parsed).await.expect("response");

    // Drain body and check for `data: {"...content":"hi"...}` line
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("\"content\":\"hi\""), "stream body: {s}");
    assert!(s.contains("[DONE]"));
}
```

**Step 2:** Run. Expected: FAIL.

### Task 10.2: Implement `try_chat`

**Files:**
- Create: `src/codex_state/chat.rs`

**Step 1:** Create `src/codex_state/chat.rs`:
```rust
use axum::{
    body::Body,
    response::{IntoResponse, Response},
};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use http::StatusCode;
use serde_json::Value;
use snafu::ResultExt;
use tracing::{info, warn};
use wreq::Method;

use crate::{
    codex_state::{
        CodexState,
        transform::{
            aggregate_codex_events, codex_event_to_oai_chunk,
            translate_chat_completions_to_codex,
        },
    },
    config::CodexAuthStatus,
    error::{ClewdrError, WreqSnafu},
    types::{codex::CodexSseEvent, oai::ChatCompletionsRequest},
};

const RETRY_BUDGET: usize = 3;

impl CodexState {
    pub async fn try_chat(
        &mut self,
        request: ChatCompletionsRequest,
    ) -> Result<Response, ClewdrError> {
        let codex_body = translate_chat_completions_to_codex(&request)
            .map_err(|e| ClewdrError::BadRequest { msg: leak(format!("codex translate: {e}")) })?;
        let codex_body_value = serde_json::to_value(&codex_body)
            .map_err(|e| ClewdrError::Other { loc: snafu::location!(), msg: format!("serialize: {e}") })?;
        let model = request.model.clone();
        let stream = request.stream.unwrap_or(false);
        self.stream = stream;

        let mut last_err: Option<ClewdrError> = None;
        for attempt in 0..RETRY_BUDGET {
            let auth = self.request_auth().await?;
            info!(
                "[REQ] codex stream={} model={} cred={}",
                stream, model, auth.id_prefix()
            );

            // Refresh if needed
            if let Err(e) = self.ensure_fresh_access_token().await {
                warn!("refresh failed (attempt {attempt}): {e}");
                self.return_auth().await;
                last_err = Some(e);
                continue;
            }

            let url = format!("{}/responses", self.api_base.trim_end_matches('/'));
            let resp = self
                .build_request(Method::POST, &url)
                .json(&codex_body_value)
                .send()
                .await
                .context(WreqSnafu { msg: "codex POST /responses" });

            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    warn!("codex network error (attempt {attempt}): {e}");
                    self.return_auth().await;
                    last_err = Some(e);
                    continue;
                }
            };

            let status = resp.status();
            if status.is_success() {
                let response = if stream {
                    self.stream_to_oai(resp, model.clone()).await?
                } else {
                    self.aggregate_to_oai(resp, model.clone()).await?
                };
                self.return_auth().await;
                return Ok(response);
            }

            // Non-success — classify & rotate
            self.classify_and_mark(status, &resp).await;
            self.return_auth().await;
            last_err = Some(ClewdrError::UpstreamHttp { status: status.as_u16() });
            // ^ add UpstreamHttp variant to error.rs if it doesn't exist
        }

        Err(last_err.unwrap_or(ClewdrError::Other {
            loc: snafu::location!(),
            msg: "codex retry budget exhausted with no error".to_string(),
        }))
    }

    async fn classify_and_mark(&mut self, status: StatusCode, resp: &wreq::Response) {
        let new_status = match status.as_u16() {
            401 => CodexAuthStatus::Invalid,
            403 => CodexAuthStatus::Banned,
            429 => {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(60);
                CodexAuthStatus::RateLimited {
                    until: chrono::Utc::now().timestamp() + retry_after,
                }
            }
            _ => return, // 5xx: leave as-is, just rotate
        };
        if let Some(auth) = self.auth.as_mut() {
            auth.status = new_status;
        }
    }

    async fn stream_to_oai(
        &self,
        resp: wreq::Response,
        model: String,
    ) -> Result<Response, ClewdrError> {
        let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        let stream = resp.bytes_stream().eventsource();
        let id_clone = id.clone();
        let mapped = stream.filter_map(move |evt| {
            let id = id_clone.clone();
            let model = model.clone();
            async move {
                let evt = match evt {
                    Ok(e) => e,
                    Err(_) => return None,
                };
                let parsed: Result<CodexSseEvent, _> = serde_json::from_str(&evt.data);
                let codex_evt = match parsed {
                    Ok(p) => p,
                    Err(_) => return None,
                };
                let chunk = codex_event_to_oai_chunk(&codex_evt, &id, &model)?;
                let json = serde_json::to_string(&chunk).ok()?;
                Some(Ok::<_, std::io::Error>(format!("data: {json}\n\n")))
            }
        });

        // Append [DONE]
        let mapped = futures::stream::StreamExt::chain(
            mapped,
            futures::stream::once(async { Ok::<_, std::io::Error>("data: [DONE]\n\n".to_string()) }),
        );

        let body = Body::from_stream(mapped);
        let mut response = Response::new(body);
        response
            .headers_mut()
            .insert("content-type", "text/event-stream".parse().unwrap());
        Ok(response)
    }

    async fn aggregate_to_oai(
        &self,
        resp: wreq::Response,
        model: String,
    ) -> Result<Response, ClewdrError> {
        let bytes = resp.bytes().await.context(WreqSnafu { msg: "codex body" })?;
        // Re-parse SSE-style or JSON depending on accept header. Codex always streams,
        // so even non-streaming requests come back as SSE — drain and aggregate.
        let text = std::str::from_utf8(&bytes).map_err(|_| ClewdrError::Other {
            loc: snafu::location!(),
            msg: "non-utf8 codex body".to_string(),
        })?;
        let mut events: Vec<CodexSseEvent> = Vec::new();
        for chunk in text.split("\n\n") {
            for line in chunk.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data.trim() == "[DONE]" { continue; }
                    if let Ok(ev) = serde_json::from_str::<CodexSseEvent>(data) {
                        events.push(ev);
                    }
                }
            }
        }
        let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        let oai = aggregate_codex_events(&events, &id, &model)
            .map_err(|e| ClewdrError::Other { loc: snafu::location!(), msg: format!("aggregate: {e}") })?;
        let body = serde_json::to_vec(&oai).unwrap();
        Ok((StatusCode::OK, [(http::header::CONTENT_TYPE, "application/json")], body).into_response())
    }
}

// helper for static-string error msgs
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}
```

**Step 2:** Add `UpstreamHttp { status: u16 }` variant to `ClewdrError` in `src/error.rs`. Check if a similar variant already exists; reuse if so.

**Step 3:** Run `cargo check`. Fix any signature mismatches with `ChatCompletionsRequest` (per Phase 5 note).

**Step 4:** Run `cargo test --test codex_chat_e2e`. Expected: PASS.

**Step 5:** Commit:
```bash
git add src/codex_state/chat.rs src/error.rs tests/codex_chat_e2e.rs
git commit -m "feat(codex): try_chat with stream/aggregate paths and error classification"
```

### Task 10.3: Test — 401 rotates and retries on next cred

**Step 1:** Add to `tests/codex_chat_e2e.rs`:
```rust
#[tokio::test]
async fn rotates_on_401_and_succeeds_on_second_cred() {
    let api = MockServer::start().await;

    let bad_token = ResponseTemplate::new(401);
    let good_body =
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n\
         event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n";

    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(header("authorization", "Bearer cred-a"))
        .respond_with(bad_token)
        .mount(&api)
        .await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(header("authorization", "Bearer cred-b"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(good_body),
        )
        .mount(&api)
        .await;

    let mut a = auth_for_test(); a.id = "a".into(); a.access_token = "cred-a".into();
    let mut b = auth_for_test(); b.id = "b".into(); b.access_token = "cred-b".into();
    let actor = CodexAuthActorHandle::start_with(vec![a, b]).await.unwrap();
    let mut state = CodexState::new(actor);
    state.api_base = api.uri();
    state.stream = true;

    let req = serde_json::from_value(serde_json::json!({
        "model": "gpt-5",
        "messages": [{"role": "user", "content": "hi"}],
        "stream": true
    })).unwrap();
    let response = state.try_chat(req).await.expect("rotates and succeeds");
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("\"content\":\"ok\""));
}
```

**Step 2:** Run test. Expected: PASS.

**Step 3:** Commit:
```bash
git add tests/codex_chat_e2e.rs
git commit -m "test(codex): rotation on 401 succeeds with next valid cred"
```

---

## Phase 11: `CodexProvider`

### Task 11.1: Provider impl

**Files:**
- Create: `src/providers/codex/mod.rs`
- Modify: `src/providers/mod.rs`

**Step 1:** Modify `src/providers/mod.rs`:
```rust
pub mod codex;
```

**Step 2:** Create `src/providers/codex/mod.rs`:
```rust
use std::sync::Arc;
use std::time::Instant;

use axum::response::Response;
use colored::Colorize;
use tracing::info;

use super::LLMProvider;
use crate::{
    codex_state::CodexState,
    error::ClewdrError,
    services::codex_auth_actor::CodexAuthActorHandle,
    types::oai::ChatCompletionsRequest,
};

#[derive(Clone)]
pub struct CodexInvocation {
    pub params: ChatCompletionsRequest,
}

#[derive(Clone)]
pub struct CodexProvider {
    auth_actor: CodexAuthActorHandle,
}

impl CodexProvider {
    pub fn new(auth_actor: CodexAuthActorHandle) -> Self {
        Self { auth_actor }
    }
}

#[async_trait::async_trait]
impl LLMProvider for CodexProvider {
    type Request = CodexInvocation;
    type Output = Response;

    async fn invoke(&self, request: Self::Request) -> Result<Self::Output, ClewdrError> {
        let mut state = CodexState::new(self.auth_actor.clone());
        let stream = request.params.stream.unwrap_or(false);
        info!(
            "[REQ] codex stream={} model={} msgs={}",
            stream,
            request.params.model.green(),
            request.params.messages.len().to_string().green()
        );
        let stopwatch = Instant::now();
        let response = state.try_chat(request.params).await?;
        let elapsed = stopwatch.elapsed();
        info!("[FIN] codex elapsed={}s", format!("{}", elapsed.as_secs_f32()).green());
        Ok(response)
    }
}

pub fn build_codex_provider(auth_actor: CodexAuthActorHandle) -> Arc<CodexProvider> {
    Arc::new(CodexProvider::new(auth_actor))
}
```

**Step 3:** Run `cargo check`. Expected: PASS.

**Step 4:** Commit:
```bash
git add src/providers/codex/mod.rs src/providers/mod.rs
git commit -m "feat(codex): CodexProvider implementing LLMProvider trait"
```

---

## Phase 12: Routing

### Task 12.1: Wire `CodexProvider` into `RouterBuilder`

**Files:**
- Modify: `src/router.rs`

**Step 1:** Modify `RouterBuilder::new()` to start the `CodexAuthActor`:
```rust
pub async fn new() -> Self {
    let cookie_handle = CookieActorHandle::start()
        .await
        .expect("Failed to start CookieActor");
    let claude_providers = crate::providers::claude::build_providers(cookie_handle.clone());
    let codex_auth_handle = crate::services::codex_auth_actor::CodexAuthActorHandle::start()
        .await
        .expect("Failed to start CodexAuthActor");
    let codex_provider = crate::providers::codex::build_codex_provider(codex_auth_handle.clone());
    RouterBuilder {
        claude_providers,
        cookie_actor_handle: cookie_handle,
        codex_auth_handle,
        codex_provider,
        inner: Router::new(),
    }
}
```

**Step 2:** Add fields to the struct:
```rust
pub struct RouterBuilder {
    claude_providers: ClaudeProviders,
    cookie_actor_handle: CookieActorHandle,
    codex_auth_handle: crate::services::codex_auth_actor::CodexAuthActorHandle,
    codex_provider: std::sync::Arc<crate::providers::codex::CodexProvider>,
    inner: Router,
}
```

**Step 3:** Add new route methods:
```rust
fn route_codex_oai_endpoints(mut self) -> Self {
    let router = Router::new()
        .route("/codex/v1/chat/completions", post(api_codex_chat))
        .route("/codex/v1/models", get(api_codex_models))
        .layer(
            ServiceBuilder::new()
                .layer(from_extractor::<RequireBearerAuth>())
                .layer(CompressionLayer::new()),
        )
        .with_state(self.codex_provider.clone());
    self.inner = self.inner.merge(router);
    self
}

fn route_codex_admin_endpoints(mut self) -> Self {
    let router = Router::new()
        .route("/api/codex/auth", get(api_codex_list).post(api_codex_add))
        .route("/api/codex/auth/{id}", delete(api_codex_delete))
        .layer(from_extractor::<RequireAdminAuth>())
        .with_state(self.codex_auth_handle.clone());
    self.inner = self.inner.merge(router);
    self
}
```

**Step 4:** Hook into `with_default_setup`:
```rust
pub fn with_default_setup(self) -> Self {
    self.route_claude_code_endpoints()
        .route_claude_web_endpoints()
        .route_admin_endpoints()
        .route_claude_web_oai_endpoints()
        .route_claude_code_oai_endpoints()
        .route_codex_oai_endpoints()       // NEW
        .route_codex_admin_endpoints()     // NEW
        .setup_static_serving()
        .with_tower_trace()
        .with_cors()
}
```

**Step 5:** Run `cargo check`. Expect: errors about missing `api_codex_*` handlers — Phase 13 fixes.

**Step 6:** No commit yet — leaves tree in a broken state. Phase 13 will commit together.

---

## Phase 13: Admin API handlers

### Task 13.1: Handler module + handlers

**Files:**
- Create: `src/api/codex.rs`
- Modify: `src/api/mod.rs`

**Step 1:** Create `src/api/codex.rs`:
```rust
use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::{
    codex_state::transform::CODEX_MODELS,
    config::{CodexAuth, CodexAuthStatus},
    error::ClewdrError,
    providers::codex::{CodexInvocation, CodexProvider},
    services::codex_auth_actor::CodexAuthActorHandle,
    types::oai::ChatCompletionsRequest,
};

// ========== OAI surface ==========

pub async fn api_codex_chat(
    State(provider): State<Arc<CodexProvider>>,
    Json(params): Json<ChatCompletionsRequest>,
) -> Result<axum::response::Response, ClewdrError> {
    use crate::providers::LLMProvider;
    provider.invoke(CodexInvocation { params }).await
}

#[derive(Serialize)]
struct ModelEntry {
    id: &'static str,
    object: &'static str,
    owned_by: &'static str,
}

#[derive(Serialize)]
struct ModelsList {
    object: &'static str,
    data: Vec<ModelEntry>,
}

pub async fn api_codex_models() -> impl IntoResponse {
    let data: Vec<ModelEntry> = CODEX_MODELS
        .iter()
        .map(|m| ModelEntry { id: m, object: "model", owned_by: "openai" })
        .collect();
    Json(ModelsList { object: "list", data })
}

// ========== Admin surface ==========

#[derive(Serialize)]
pub struct CodexAuthSummary {
    id: String,
    label: Option<String>,
    account_id_prefix: String,
    plan: Option<String>,
    status: CodexAuthStatus,
    last_used_at: Option<i64>,
}

impl From<CodexAuth> for CodexAuthSummary {
    fn from(a: CodexAuth) -> Self {
        let p = a.account_id.chars().take(8).collect::<String>();
        Self {
            id: a.id, label: a.label, account_id_prefix: p,
            plan: a.plan, status: a.status, last_used_at: a.last_used_at,
        }
    }
}

pub async fn api_codex_list(
    State(handle): State<CodexAuthActorHandle>,
) -> Result<Json<Vec<CodexAuthSummary>>, ClewdrError> {
    let list = handle.list().await?;
    Ok(Json(list.into_iter().map(Into::into).collect()))
}

#[derive(Deserialize)]
pub struct AddCodexAuthBody {
    pub auth_json: String,
    pub label: Option<String>,
}

pub async fn api_codex_add(
    State(handle): State<CodexAuthActorHandle>,
    Json(body): Json<AddCodexAuthBody>,
) -> Result<(StatusCode, Json<CodexAuthSummary>), ClewdrError> {
    let auth = CodexAuth::from_auth_json(&body.auth_json, body.label)
        .map_err(|e| ClewdrError::BadRequest { msg: Box::leak(format!("auth.json: {e}").into_boxed_str()) })?;
    let summary: CodexAuthSummary = auth.clone().into();
    handle.submit(auth).await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

pub async fn api_codex_delete(
    State(handle): State<CodexAuthActorHandle>,
    Path(id): Path<String>,
) -> Result<StatusCode, ClewdrError> {
    handle.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

**Step 2:** Modify `src/api/mod.rs` to expose handlers:
```rust
pub mod codex;
pub use codex::{api_codex_add, api_codex_chat, api_codex_delete, api_codex_list, api_codex_models};
```

**Step 3:** Run `cargo check`. Resolve any error type mismatches.

**Step 4:** Run `cargo build`. Expected: PASS (full binary builds).

**Step 5:** Commit (Phase 12 + 13 together since router was incomplete):
```bash
git add src/router.rs src/api/codex.rs src/api/mod.rs
git commit -m "feat(codex): wire OAI endpoints, admin CRUD, and models list into router"
```

### Task 13.2: Smoke test — admin CRUD via running router

**Files:**
- Test: `tests/codex_admin_smoke.rs`

**Step 1:** Skeletal test invoking handlers directly with mocked actor (use the `CodexAuthActorHandle::start_with(vec![])` pattern):
```rust
use axum::extract::{Path, State};
use axum::http::StatusCode;
use clewdr::api::codex::{
    AddCodexAuthBody, api_codex_add, api_codex_delete, api_codex_list,
};
use clewdr::services::codex_auth_actor::CodexAuthActorHandle;

const AUTH_JSON: &str = r#"{
  "tokens": {
    "id_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJhY2N0X2FiYyIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhYmMtMTIzIiwiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIn19.sig",
    "access_token": "at",
    "refresh_token": "rt-unique-x9"
  }
}"#;

#[tokio::test]
async fn admin_add_then_list_then_delete() {
    let handle = CodexAuthActorHandle::start_with(vec![]).await.unwrap();

    // POST /api/codex/auth
    let (status, body) = api_codex_add(
        State(handle.clone()),
        axum::Json(AddCodexAuthBody {
            auth_json: AUTH_JSON.to_string(),
            label: Some("personal".into()),
        }),
    ).await.expect("add");
    assert_eq!(status, StatusCode::CREATED);
    let id = body.0.id.clone();
    assert!(!id.is_empty());

    // GET /api/codex/auth
    let list = api_codex_list(State(handle.clone())).await.expect("list");
    assert_eq!(list.0.len(), 1);
    assert_eq!(list.0[0].label.as_deref(), Some("personal"));

    // DELETE /api/codex/auth/{id}
    let status = api_codex_delete(State(handle.clone()), Path(id))
        .await
        .expect("delete");
    assert_eq!(status, StatusCode::NO_CONTENT);

    let list = api_codex_list(State(handle)).await.expect("list");
    assert_eq!(list.0.len(), 0);
}
```

**Step 2:** May require exposing `pub mod api;` and `pub mod codex;` in `lib.rs` and `api/mod.rs` so test can import — adjust visibility as needed (use `pub use` re-exports).

**Step 3:** Run test. Expected: PASS.

**Step 4:** Commit:
```bash
git add tests/codex_admin_smoke.rs src/api/mod.rs src/lib.rs
git commit -m "test(codex): admin CRUD smoke test exercises add/list/delete"
```

---

## Phase 14: Frontend `Codex` tab

### Task 14.1: Inspect existing tab structure

**Step 1:** Read `frontend/package.json`, `frontend/src/App.tsx` (or equivalent), and the existing `Cookies` tab files to learn conventions.

**Step 2:** Note: Vite + React + TypeScript. State management library, styling (Tailwind?), API base path. Find the tab-routing system (likely React Router or a custom switch).

**Step 3:** Note exact file locations to mirror. No code changes yet — this is reconnaissance.

### Task 14.2: API client

**Files:**
- Create: `frontend/src/api/codex.ts`

**Step 1:** Mirror `frontend/src/api/cookies.ts` (or whatever the existing cookie API client is named):
```typescript
import { apiBase, authHeaders } from './common'; // adjust to actual helper names

export interface CodexAuthSummary {
  id: string;
  label?: string;
  account_id_prefix: string;
  plan?: string;
  status: { kind: 'valid' } | { kind: 'rate_limited'; until: number }
        | { kind: 'expired' } | { kind: 'invalid' } | { kind: 'banned' };
  last_used_at?: number;
}

export async function listCodexAuth(): Promise<CodexAuthSummary[]> {
  const res = await fetch(`${apiBase}/api/codex/auth`, { headers: authHeaders() });
  if (!res.ok) throw new Error(`list failed: ${res.status}`);
  return res.json();
}

export async function addCodexAuth(authJson: string, label?: string): Promise<CodexAuthSummary> {
  const res = await fetch(`${apiBase}/api/codex/auth`, {
    method: 'POST',
    headers: { ...authHeaders(), 'content-type': 'application/json' },
    body: JSON.stringify({ auth_json: authJson, label }),
  });
  if (!res.ok) throw new Error(`add failed: ${res.status} ${await res.text()}`);
  return res.json();
}

export async function deleteCodexAuth(id: string): Promise<void> {
  const res = await fetch(`${apiBase}/api/codex/auth/${id}`, {
    method: 'DELETE',
    headers: authHeaders(),
  });
  if (!res.ok) throw new Error(`delete failed: ${res.status}`);
}
```

**Step 2:** Adjust imports/helpers to match the actual `frontend/src/api/common.ts` (or wherever `apiBase`/`authHeaders` live).

**Step 3:** Commit:
```bash
git add frontend/src/api/codex.ts
git commit -m "feat(frontend/codex): API client for codex auth CRUD"
```

### Task 14.3: List component + status badges

**Files:**
- Create: `frontend/src/components/codex/CodexAuthList.tsx`

**Step 1:** Build a table component matching styling/layout patterns of existing cookie list. Key features:
- Loads via `listCodexAuth()` on mount.
- Renders rows: label (or short id), account prefix, plan, status badge (color-coded), last used (relative time).
- Per-row delete button with confirmation.
- Refreshes list on delete success.
- Loading + error states.

**Step 2:** Vitest test for the component (mock the API calls). Include cases: renders rows, shows empty state, calls delete handler.

**Step 3:** Commit:
```bash
git add frontend/src/components/codex/CodexAuthList.tsx \
        frontend/src/components/codex/__tests__/CodexAuthList.test.tsx
git commit -m "feat(frontend/codex): CodexAuthList with status badges and delete"
```

### Task 14.4: Add form

**Files:**
- Create: `frontend/src/components/codex/AddCodexAuthForm.tsx`

**Step 1:** Form with: label input (optional), large textarea labeled "Paste contents of `~/.codex/auth.json`", submit. On submit calls `addCodexAuth`. Shows inline parse error from server response. On success, clears form and emits `onAdded` callback.

**Step 2:** Vitest test: submitting valid blob calls API, error response surfaces, empty submit blocked.

**Step 3:** Commit:
```bash
git add frontend/src/components/codex/AddCodexAuthForm.tsx \
        frontend/src/components/codex/__tests__/AddCodexAuthForm.test.tsx
git commit -m "feat(frontend/codex): AddCodexAuthForm with auth.json paste textarea"
```

### Task 14.5: Codex tab page + sidebar entry

**Files:**
- Create: `frontend/src/pages/CodexTab.tsx` (or actual pages dir)
- Modify: tab/sidebar config (e.g., `frontend/src/App.tsx`)

**Step 1:** `CodexTab.tsx` composes `<AddCodexAuthForm onAdded={...} />` and `<CodexAuthList key={refreshKey} />`. Bumps a `refreshKey` to force list re-fetch on add.

**Step 2:** Add the tab to the existing tab/sidebar config alongside `Claude`/`Usage`/`Settings`.

**Step 3:** Run frontend dev server: `cd frontend && pnpm dev` (or `npm run dev`). Navigate to the new Codex tab manually:
- Verify empty state renders.
- Run the backend (`cargo run`) in another terminal.
- Add a credential via the form (use a real `~/.codex/auth.json` if available, or a hand-crafted blob with the JWT structure used in tests).
- Verify it appears in the list.
- Delete it. Verify it disappears.

**Step 4:** If anything looks off, fix and re-test before commit.

**Step 5:** Commit:
```bash
git add frontend/src/pages/CodexTab.tsx frontend/src/App.tsx # + any tab config files
git commit -m "feat(frontend/codex): Codex tab integrated into admin sidebar"
```

---

## Phase 15: Documentation

### Task 15.1: Update README

**Files:**
- Modify: `README.md`

**Step 1:** Add a new section after "Claude" under "Configure Upstreams":
```markdown
### Codex

ClewdR can route requests to OpenAI's Codex backend using OAuth tokens minted by the official `codex` CLI.

1. On a machine with a browser, install the [Codex CLI](https://github.com/openai/codex) and run `codex login`.
2. Open `~/.codex/auth.json` and copy its contents.
3. In the ClewdR admin UI, open the **Codex** tab, paste the JSON into the form, and submit.

Endpoint: `http://127.0.0.1:8484/codex/v1/chat/completions` (OpenAI-compatible).
Supported models: `gpt-5-codex`, `gpt-5`, `gpt-4.1`, `o3`, `o4-mini`.

ClewdR auto-refreshes access tokens; no further action needed unless the refresh token is revoked (re-run `codex login` and re-paste).
```

**Step 2:** Add Codex to the "Supported Endpoints" table at the top.

**Step 3:** Run `cargo build && cargo test` one last time to confirm everything compiles and passes.

**Step 4:** Commit:
```bash
git add README.md
git commit -m "docs(codex): README section for Codex setup and supported models"
```

### Task 15.2: Final verification

**Step 1:** Full test suite: `cargo test`. Expected: all tests pass.

**Step 2:** Frontend tests: `cd frontend && pnpm test` (or equivalent). Expected: pass.

**Step 3:** Build release binary: `cargo build --release`. Expected: success.

**Step 4:** Run `git log --oneline feat/codex-support ^master | wc -l` — should show all phase commits.

**Step 5:** Push branch:
```bash
git push -u origin feat/codex-support
```

**Step 6:** Open PR with title "feat: add Codex backend support" and link to design doc.

---

## Skills to invoke during execution

- **`superpowers:test-driven-development`** before every task — red, green, commit.
- **`superpowers:systematic-debugging`** when any test fails unexpectedly or wiremock behaves oddly.
- **`superpowers:verification-before-completion`** before marking any phase complete — run the relevant `cargo test` slice and confirm output.
- **`superpowers:requesting-code-review`** before opening the final PR.
