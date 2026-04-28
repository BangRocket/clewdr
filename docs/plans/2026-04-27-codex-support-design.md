# Codex Support — Design

**Date:** 2026-04-27
**Branch:** `feat/codex-support`
**Status:** Approved (design only; implementation plan to follow)

## 1. Scope & non-goals

**In scope:**
- Add Codex (OpenAI's `chatgpt.com/backend-api/codex/responses`) as a new backend provider.
- New OpenAI-compatible endpoint: `POST /codex/v1/chat/completions` (streaming + non-streaming).
- New `GET /codex/v1/models` endpoint listing supported Codex models.
- Multi-credential pool with rotation on failure (mirrors existing `CookieActor` pattern).
- OAuth refresh-token handling (auto-refresh on near-expiry).
- Per-credential status: `Valid` / `Expired` / `Invalid` / `Banned` / `RateLimited{until}`.
- Admin endpoints (admin-auth-gated) to add/list/delete Codex credentials.
- Admin-UI tab for credential management (CRUD + status badges, no charts).

**Out of scope (deferred to follow-up branches):**
- ChatGPT web cookie support — intentionally dropped (PoW/anti-bot maintenance burden).
- Native Responses-API passthrough endpoint (`/codex/v1/responses`).
- Usage event logging, cost tracking, Codex models in pricing table.
- Token-counting endpoint.
- Self-hosted OAuth flow (PKCE in browser).
- Frontend usage charts, drill-down, graveyard view for Codex.

## 2. Credential storage & provisioning

### Type — `CodexAuth` (in `src/config/codex_auth.rs`)

```rust
pub struct CodexAuth {
    pub id: String,                   // stable hash of refresh_token; history/log key
    pub label: Option<String>,        // user-friendly name
    pub id_token: String,             // JWT, encodes account_id + plan tier
    pub access_token: String,         // bearer for /backend-api/codex/responses
    pub refresh_token: String,        // long-lived
    pub access_expires_at: i64,       // unix seconds
    pub account_id: String,           // parsed from id_token claims
    pub plan: Option<String>,         // info only ("plus", "pro", "business")
    pub status: CodexAuthStatus,
    pub last_used_at: Option<i64>,
}

pub enum CodexAuthStatus {
    Valid,
    RateLimited { until: i64 },
    Expired,
    Invalid,
    Banned,
}
```

Persisted in `clewdr.toml` under a new `codex_auth = [...]` array. Hot-reload via existing config path.

### Provisioning paths

1. **Paste `auth.json` blob (preferred):** user runs `codex login` locally, copies `~/.codex/auth.json`, pastes into admin endpoint or admin UI. Server parses, extracts tokens, decodes id_token claims for `account_id` / `plan`.
2. **Direct config edit:** `[[codex_auth]]` blocks in `clewdr.toml`. Reload picks them up.

ClewdR does **not** host an OAuth redirect. No PKCE flow in-app. Avoids embedding OAuth client secrets and shrinks attack surface.

### Refresh

On every request, if `access_expires_at - 60s < now`:
- `CodexAuthActor` posts to `https://auth.openai.com/oauth/token` with the refresh_token.
- Updates `access_token` + `access_expires_at`, persists, then proceeds.
- On `400 invalid_grant` → status `Invalid` (re-login needed); rotate cred.
- On 5xx → status `Expired` (transient); rotate cred.

## 3. Module layout & routing

### New tree (mirrors Claude side)

```
src/
├── codex_state/
│   ├── mod.rs              # CodexState struct + auth refresh + request builder
│   ├── chat.rs             # try_chat — POST + stream pump
│   └── transform.rs        # OpenAI ChatCompletions ↔ Responses API translation
├── providers/codex/
│   └── mod.rs              # CodexProvider, CodexInvocation, build_providers
├── services/
│   └── codex_auth_actor.rs # mirrors cookie_actor: pool, request/return, refresh, status
├── config/
│   └── codex_auth.rs       # CodexAuth + status enum + serde
├── api/
│   └── codex.rs            # admin handlers
└── types/codex/
    └── mod.rs              # Responses-API request/response types
```

### Touched files

- `src/router.rs` — `route_codex_oai_endpoints()` + admin route group; wire `CodexProvider`.
- `src/api/mod.rs` — re-export new admin handlers.
- `src/config/mod.rs`, `clewdr_config.rs` — add `codex_auth: Vec<CodexAuth>`, retention/concurrency knobs.
- `src/middleware/mod.rs` — reuse existing `RequireBearerAuth` / `RequireAdminAuth`.
- `frontend/` — new `Codex` tab.

### Routes

```
POST   /codex/v1/chat/completions   (RequireBearerAuth + Compression + to_oai)
GET    /codex/v1/models             (RequireBearerAuth)
GET    /api/codex/auth              (RequireAdminAuth — list)
POST   /api/codex/auth              (RequireAdminAuth — add via auth.json paste)
DELETE /api/codex/auth/{id}         (RequireAdminAuth)
```

`CodexProvider` implements existing `LLMProvider` trait. No changes to the trait.

### Per-request state machine (`CodexState::try_chat`)

1. `auth = codex_auth_actor.request().await?` — get next-available `Valid` cred.
2. If `access_expires_at - 60 < now`: refresh; on fail → return cred with status, retry.
3. Translate OpenAI ChatCompletions request → Codex Responses-API body.
4. POST `https://chatgpt.com/backend-api/codex/responses` with `Authorization: Bearer {access_token}` + `ChatGPT-Account-Id` header.
5. Map status: 401 → `Expired`/`Invalid`, 403 → `Banned`, 429 → `RateLimited{until}`, 5xx → transient.
6. Pump SSE stream → translate Responses events to OpenAI ChatCompletion chunks → forward.
7. Return cred to pool with updated status/last_used_at.

Bounded retry budget: 3 hops across creds per request.

## 4. Translation, models, error handling

### Request translation (OpenAI ChatCompletions → Codex Responses)

| OpenAI ChatCompletions field | Codex Responses field | Notes |
|---|---|---|
| `model` | `model` | whitelist |
| `messages[]` | `input[]` | `{ type: "message", role, content: [{type:"input_text"\|"output_text", text}] }` |
| system role messages | `instructions` | concat if multiple |
| `stream: true` | header + `stream: true` | |
| `temperature`, `top_p`, `max_tokens`→`max_output_tokens` | passthrough | |
| `tools[]` | `tools[]` | function-tool shape compatible |
| `tool_choice` | `tool_choice` | passthrough |
| `response_format` | `text.format` | json_schema mapping |
| `n`, `logprobs`, `presence_penalty`, `frequency_penalty` | dropped + warn | unsupported |

### Stream translation (Codex SSE → OpenAI chunks)

- `response.output_text.delta` → `chat.completion.chunk` with `delta.content`.
- `response.tool_call.delta` → `delta.tool_calls[...]`.
- `response.completed` → final chunk + `[DONE]` marker, `finish_reason: "stop"`.
- `response.error` → 4xx/5xx mapped to OpenAI error envelope.
- `response.usage` (final) → `usage` field on last chunk.

Non-streaming: collect deltas, return single `chat.completion`.

### Models

Hardcoded whitelist for MVP:
```
gpt-5-codex, gpt-5, gpt-4.1, o3, o4-mini
```
Stored as `const CODEX_MODELS: &[&str]`. Unknown model → 400 with valid list. Dynamic discovery deferred.

### Error mapping

| Upstream | Action | Client sees |
|---|---|---|
| 401 (after refresh) | mark `Invalid`, rotate, retry once | OpenAI 401 if exhausted |
| 401 (refresh rejected) | mark `Invalid`, rotate, retry | same |
| 403 | mark `Banned`, rotate, retry | OpenAI 403 if exhausted |
| 429 + Retry-After | mark `RateLimited{until}`, rotate, retry | OpenAI 429 if exhausted |
| 5xx | rotate, retry (max 2) | OpenAI 502 if exhausted |
| network/timeout | retry once same cred, then rotate | OpenAI 502 |
| translation error | no retry | OpenAI 400 with detail |

### Logging

- `[REQ] codex stream=… model=… msgs=… cred={id_prefix}` per attempt.
- `[FIN] elapsed=…s tokens=in/out` on completion.
- Status transitions at WARN.

## 5. Frontend, testing, rollout

### Frontend (`frontend/src/`)

New `Codex` tab. Components:
- `CodexAuthList` — table: label, account_id (truncated), plan, status badge, last_used_at, delete button.
- `AddCodexAuthForm` — textarea for `~/.codex/auth.json`, optional label, submit. Inline parse-error display.
- API client functions mirroring existing `cookies.ts` shape.
- Sidebar tab entry next to `Claude` / `Usage` / `Settings`.

No charts, no drill-down, no graveyard for MVP.

### Testing

| Layer | Approach |
|---|---|
| Translation | Unit tests `tests/codex_transform.rs` — golden fixtures both directions |
| Auth refresh | Unit test with mocked token endpoint (success / `invalid_grant` / 5xx) |
| State machine retries | Integration test with `wiremock` (401→200, 429→200, 5xx exhaustion) |
| End-to-end | `tests/codex_e2e.rs` gated behind `CODEX_TEST_AUTH_JSON` env var; CI skips |
| Frontend | Vitest unit tests for table/form, no live backend |

Use TDD skill during execution.

### Rollout

- No feature flag. Additive endpoint; off at runtime when no creds configured (returns 503).
- No new Cargo features.
- New crate dep expected: `jsonwebtoken` for id_token claims decode. Verify before commit.

### Follow-up branches (not in this PR)

- ChatGPT web cookie support.
- Native `/codex/v1/responses` passthrough.
- Usage events + cost + Codex pricing.
- Token-counting endpoint.
- Self-hosted OAuth flow.
