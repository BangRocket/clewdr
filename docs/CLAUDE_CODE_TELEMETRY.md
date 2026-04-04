# Claude Code Telemetry Analysis

**Source Analyzed:** Claude Code internal source (`services/analytics/`, `utils/telemetry/`)
**Version:** 2.1.76+
**Last Updated:** 2026-04-03

## Overview

Claude Code (internal codename: "Tengu") uses a multi-channel telemetry system for internal analytics. This document details the telemetry architecture, endpoints, data formats, and what clewdr emulates.

---

## Telemetry Architecture

Claude Code sends telemetry through these channels:

| Channel | Endpoint | Purpose | Emulated by clewdr? |
|---------|----------|---------|---------------------|
| **1st-Party (1P) Event Logging** | `/api/event_logging/batch` | Primary behavioral telemetry | **Yes** |
| **BigQuery Metrics** | `/api/claude_code/metrics` | Quantitative metrics for org analytics | No |
| **Datadog** | `https://http-intake.logs.us5.datadoghq.com/api/v2/logs` | Subset of allowed events | No |
| **GrowthBook** | `https://cdn.growthbook.io` | Feature flags / A/B test assignment | No |

**Removed channels** (no longer in source):
- ~~Statsig~~ — replaced by 1P event logging
- ~~Segment~~ — removed

---

## 1. Primary Channel: 1P Event Logging

### Endpoint
```
POST https://api.anthropic.com/api/event_logging/batch
```

### Headers
```
Content-Type: application/json
User-Agent: claude-code/<version> <platform>/<arch> node/v22.x
x-service-name: claude-code
Authorization: Bearer <access_token>  (optional — falls back to unauthenticated)
```

### Payload Format (ClaudeCodeInternalEvent proto)

```json
{
  "events": [
    {
      "event_type": "ClaudeCodeInternalEvent",
      "event_data": {
        "event_id": "<uuid>",
        "event_name": "tengu_api_query",
        "client_timestamp": "2026-04-03T12:00:00.000Z",
        "device_id": "user_<anonymousId>_account_<accountUuid>_session_<sessionId>",
        "session_id": "<sessionId>",
        "email": "<user email or omitted>",
        "auth": {
          "account_uuid": "<uuid>",
          "organization_uuid": "<uuid>"
        },
        "model": "claude-sonnet-4-5-20250514",
        "user_type": "",
        "is_interactive": true,
        "client_type": "cli",
        "env": {
          "platform": "darwin",
          "platform_raw": "darwin",
          "arch": "arm64",
          "node_version": "v22.13.1",
          "terminal": "iTerm.app",
          "runtimes": "node",
          "is_running_with_bun": false,
          "is_ci": false,
          "is_claubbit": false,
          "is_claude_code_remote": false,
          "is_local_agent_mode": false,
          "is_conductor": false,
          "is_github_action": false,
          "is_claude_code_action": false,
          "is_claude_ai_auth": true,
          "version": "2.1.76",
          "build_time": "2026-03-28T00:00:00.000Z",
          "deployment_environment": "production",
          "vcs": "git"
        },
        "additional_metadata": "<base64-encoded JSON of event-specific data>"
      }
    }
  ]
}
```

### Auth Handling

1. Try with OAuth bearer token first
2. On 401, retry without auth headers
3. Skip auth entirely if:
   - Trust dialog not accepted
   - OAuth token expired
   - Lacks `user:profile` scope (service key sessions)
   - Running on Bedrock/Vertex/Foundry

### Batch Processing

| Setting | Default | GrowthBook Config Key |
|---------|---------|----------------------|
| Export interval | 10 seconds | `tengu_1p_event_batch_config.scheduledDelayMillis` |
| Max batch size | 200 events | `tengu_1p_event_batch_config.maxExportBatchSize` |
| Max queue size | 8192 events | `tengu_1p_event_batch_config.maxQueueSize` |
| Max retry attempts | 8 | `tengu_1p_event_batch_config.maxAttempts` |
| Base backoff | 500ms | Quadratic: `base × attempts²` |
| Max backoff | 30 seconds | — |

### Resilience

- **Disk queue:** Failed events written to `~/.claude/telemetry/1p_failed_events.<sessionId>.<batchUuid>.json`
- **JSONL format:** One event per line, append-only
- **Retry:** Quadratic backoff, dropped after 8 attempts
- **Immediate drain:** On export success, immediately retries queued events

---

## 2. BigQuery Metrics

### Endpoint
```
POST https://api.anthropic.com/api/claude_code/metrics
```

### Purpose
Quantitative metrics for organization-level analytics. Controlled by org-level opt-out via:
```
GET https://api.anthropic.com/api/claude_code/organizations/metrics_enabled
```

### Timeout
5000ms

### Disabled for:
- Non-subscriber API key users
- Service key sessions
- Organization opt-out (cached 24h at `~/.claude/telemetry/org_metrics_settings.json`)

---

## 3. Datadog

### Endpoint
```
POST https://http-intake.logs.us5.datadoghq.com/api/v2/logs
```

### API Key
```
DD-API-KEY: pubbbf48e6d78dae54bceaa4acf463299bf
```

### Allowed Events (27 total)

**Chrome Bridge (7):**
- `chrome_bridge_connection_succeeded`, `chrome_bridge_connection_failed`, `chrome_bridge_disconnected`
- `chrome_bridge_tool_call_completed`, `chrome_bridge_tool_call_error`, `chrome_bridge_tool_call_started`, `chrome_bridge_tool_call_timeout`

**Tengu Core (20):**
- `tengu_api_error`, `tengu_api_success`
- `tengu_brief_mode_enabled`, `tengu_brief_mode_toggled`, `tengu_brief_send`
- `tengu_cancel`, `tengu_compact_failed`, `tengu_exit`, `tengu_flicker`, `tengu_init`
- `tengu_model_fallback_triggered`
- `tengu_oauth_error`, `tengu_oauth_success`
- `tengu_oauth_token_refresh_failure`, `tengu_oauth_token_refresh_success`
- `tengu_oauth_token_refresh_lock_acquiring`, `tengu_oauth_token_refresh_lock_acquired`
- `tengu_oauth_token_refresh_starting`, `tengu_oauth_token_refresh_completed`
- `tengu_oauth_token_refresh_lock_releasing`, `tengu_oauth_token_refresh_lock_released`
- `tengu_query_error`, `tengu_session_file_read`, `tengu_started`
- `tengu_tool_use_error`, `tengu_tool_use_success`
- `tengu_tool_use_granted_in_prompt_permanent`, `tengu_tool_use_granted_in_prompt_temporary`, `tengu_tool_use_rejected_in_prompt`
- `tengu_uncaught_exception`, `tengu_unhandled_rejection`
- `tengu_voice_recording_started`, `tengu_voice_toggled`
- `tengu_team_mem_sync_pull`, `tengu_team_mem_sync_push`, `tengu_team_mem_sync_started`, `tengu_team_mem_entries_capped`

### Tag Fields
Events include these as Datadog tags for filtering:
`arch`, `clientType`, `errorType`, `http_status_range`, `http_status`, `kairosActive`, `model`, `platform`, `provider`, `skillMode`, `subscriptionType`, `toolName`, `userBucket`, `userType`, `version`, `versionBase`

### Feature Gate
Controlled by GrowthBook gate: `tengu_log_datadog_events`

---

## 4. GrowthBook Feature Flags

### Endpoint
```
https://cdn.growthbook.io
```

### Key Configurations
| Config Name | Purpose |
|-------------|---------|
| `tengu_event_sampling_config` | Per-event sample rates (0-1) |
| `tengu_1p_event_batch_config` | Batch processor settings |
| `tengu_frond_boric` | Per-sink killswitch (disable datadog/1P) |
| `tengu_log_datadog_events` | Feature gate for Datadog |

---

## Data Collected

### User Identifiers

| Field | Source | Description |
|-------|--------|-------------|
| `device_id` / `user_id` | Generated | `user_{anonymousId}_account_{accountUuid}_session_{sessionId}` |
| `accountUuid` | OAuth profile | Anthropic account UUID |
| `organizationUuid` | OAuth profile | Organization UUID (team accounts) |
| `email` | OAuth profile | User's email (OAuth only) |
| `sessionId` | `crypto.randomUUID()` | Per-session unique ID |
| `parentSessionId` | State/env | Parent session (forked sessions) |

### System Information (OpenTelemetry Resource)

| Field | Source | Description |
|-------|--------|-------------|
| `host.id` | `/etc/machine-id`, IOPlatformUUID, or Windows registry | Persistent machine ID |
| `host.name` | `os.hostname()` | System hostname |
| `host.arch` | `process.arch` | CPU architecture (x64, arm64) |
| `os.type` | `os.type()` | Darwin, Linux, Windows_NT |
| `os.version` | OS-specific | OS version string |
| `service.name` | Constant | `claude-code` |
| `service.version` | Build macro | e.g., `2.1.76` |
| `process.platform` | `process.platform` | darwin, linux, win32 |

### Environment Context (per event)

| Field | Description |
|-------|-------------|
| `platform` | `darwin`, `linux`, `win32`, `wsl` |
| `arch` | `arm64`, `x64` |
| `node_version` | Node.js version |
| `terminal` | Terminal program (iTerm.app, vscode, etc.) |
| `version` | Claude Code version |
| `build_time` | Build timestamp |
| `deployment_environment` | `production` |
| `is_ci` | Running in CI? |
| `is_claude_ai_auth` | OAuth subscriber? |
| `is_claude_code_remote` | Remote session? |
| `vcs` | Detected VCS (git, etc.) |

### PII Protection

- MCP tool names: `mcp__<server>__<tool>` → sanitized to `mcp_tool` (unless tool details logging enabled)
- Skill/plugin names: Only in privileged `_PROTO_*` proto fields
- File paths: Stripped from general metadata
- User prompts: Redacted unless `OTEL_LOG_USER_PROMPTS=1`
- Tool inputs: Truncated (512→128 chars, depth limit 2, max 20 items)

---

## Key Events (non-exhaustive)

### Initialization & Session
- `tengu_init` — CLI startup with flags/configuration
- `tengu_started` — After initialization complete
- `tengu_startup_telemetry` — System configuration at launch
- `tengu_exit` — Session termination with totals

### API & Authentication
- `tengu_api_query` — API request initiated
- `tengu_api_success` — Successful API response
- `tengu_api_error` — API error (includes status code, error type)
- `tengu_api_retry` — Retry attempt
- `tengu_oauth_*` — OAuth flow events (20+ variants)

### Tool Usage
- `tengu_tool_use_success` — Tool completed successfully
- `tengu_tool_use_error` — Tool error
- `tengu_tool_use_cancelled` — User cancelled tool
- `tengu_tool_use_granted_in_config` / `_in_prompt_*` — Permission grants
- `tengu_tool_use_rejected_in_prompt` — User rejected tool

### User Interactions
- `tengu_input_prompt` — User prompt submitted
- `tengu_cancel` — User cancellation
- `tengu_compact` — Context compaction

---

## Environment Variables for Telemetry Control

| Variable | Effect |
|----------|--------|
| `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` | Disables telemetry, error reporting, feedback |
| `DISABLE_ERROR_REPORTING` | Disables Sentry error reporting |
| `DISABLE_TELEMETRY` | Disables telemetry collection |
| `CLAUDE_CODE_ENABLE_TELEMETRY` | Explicit telemetry enable (managed deployments) |
| `OTEL_LOG_USER_PROMPTS` | Enable user prompt logging |
| `OTEL_LOG_TOOL_DETAILS` | Enable detailed MCP tool/skill name logging |
| `CLAUDE_CODE_TELEMETRY_PRIVACY_LEVEL` | `essential-traffic` or `no-telemetry` |

### Auto-Disabled When:
- `NODE_ENV=test`
- `CLAUDE_CODE_USE_BEDROCK=1` / `VERTEX` / `FOUNDRY`
- Non-subscriber API key users
- Service key sessions (lacks profile scope)
- Trust dialog not accepted (interactive mode)

---

## Data Sources: User-Supplied vs Cookie/Token-Derived

### Authentication Flow

Claude Code uses OAuth 2.0 with two identity providers:
1. **claude.ai** — Consumer accounts (`https://claude.ai/oauth/authorize`)
2. **platform.claude.com** — API/Console accounts (`https://platform.claude.com/oauth/authorize`)

### Data Derived from OAuth Tokens

| Field | Source |
|-------|--------|
| `account.uuid` | OAuth profile response |
| `account.email` | OAuth profile response |
| `account.display_name` | OAuth profile response |
| `organization.uuid` | OAuth profile response |
| `organization.organization_type` | OAuth profile response (`claude_max`, `claude_max_20x`, etc.) |
| `subscriptionType` | Token exchange |
| `rateLimitTier` | Token exchange |

### Data Generated Locally

| Field | Source |
|-------|--------|
| `sessionId` | `crypto.randomUUID()` per session |
| `anonymousId` | Persisted random UUID in `~/.claude.json` |
| `host.id` | `/etc/machine-id`, IOPlatformUUID, or Windows registry |
| `host.name` | `os.hostname()` |
| OS info | Standard OS APIs |

### Composite User ID
```
user_{anonymousId}_account_{accountUuid}_session_{sessionId}
```

---

## What clewdr Emulates

### Request-Level Masking (always active)
- **User-Agent:** `claude-code/2.1.76`
- **Billing header:** `x-anthropic-billing-header: cc_version=2.1.76.<hash>; cc_entrypoint=cli; cch=00000;`
- **Browser emulation:** Chrome 136 TLS fingerprint
- **OAuth headers:** `anthropic-beta: oauth-2025-04-20[,context-1m-2025-08-07]`
- **API version:** `anthropic-version: 2023-06-01`

### Telemetry Emulation (opt-in via `claude_code_telemetry = true`)
- **1P event logging** to `/api/event_logging/batch`
- Events: `tengu_api_query`, `tengu_api_success`, `tengu_api_error`
- Proper ClaudeCodeInternalEvent format with environment metadata
- System resource attributes (host.id, os.type, etc.)
- Session ID and anonymous ID generation
- Bearer token authentication (when available)
- Fire-and-forget: never blocks API calls

### Not Emulated
- Datadog logging (feature-gated, not required)
- GrowthBook experiment tracking
- BigQuery metrics
- Sentry error reporting

---

## Key Source Files (Claude Code)

| File | Purpose |
|------|---------|
| `services/analytics/firstPartyEventLogger.ts` | 1P event logger setup, batch config |
| `services/analytics/firstPartyEventLoggingExporter.ts` | Event export, retry, disk queue, auth fallback |
| `services/analytics/sink.ts` | Event routing (Datadog + 1P), sampling |
| `services/analytics/index.ts` | Public API: `logEvent()`, `logEventAsync()` |
| `services/analytics/datadog.ts` | Datadog integration, allowed events |
| `services/analytics/metadata.ts` | Core metadata enrichment, PII sanitization |
| `utils/telemetry/instrumentation.ts` | OTEL setup, exporters |
| `utils/telemetry/bigqueryExporter.ts` | BigQuery metrics POST |
| `utils/telemetryAttributes.ts` | Telemetry attribute collection |
