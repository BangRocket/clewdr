# ClewdR

<p align="center">
  <img src="./assets/clewdr-logo.svg" alt="ClewdR" height="60">
</p>

ClewdR is a Rust proxy for Claude (Claude.ai, Claude Code).  
It keeps resource usage low, serves OpenAI-style endpoints, and ships with a small React admin UI for managing cookies and settings.

---

## Highlights

- Works with Claude web and Claude Code.
- Single static binary for Linux, macOS, Windows, and Android; Docker image available.
- Web dashboard shows live status and supports hot config reloads.
- Drops into existing OpenAI-compatible clients while keeping native Claude formats.
- Typical production footprint: `<10 MB` RAM, `<1 s` startup, `~15 MB` binary.

## Supported Endpoints

| Service | Endpoint |
|---------|----------|
| Claude.ai | `http://127.0.0.1:8484/v1/messages` |
| Claude.ai OpenAI compatible | `http://127.0.0.1:8484/v1/chat/completions` |
| Claude Code | `http://127.0.0.1:8484/code/v1/messages` |
| Claude Code OpenAI compatible | `http://127.0.0.1:8484/code/v1/chat/completions` |
| Codex (OpenAI-compatible) | `http://127.0.0.1:8484/codex/v1/chat/completions` |

Streaming responses work on every endpoint.

`/v1/chat/completions` (the bare OpenAI-compat path) routes to **Claude** by default but can be flipped to **Codex** via the `Default OAI Backend` toggle in the Config tab (or `default_oai_backend = "codex"` in `clewdr.toml`). Path-specific routes (`/code/v1/*`, `/codex/v1/*`) are unaffected and always hit their dedicated backend.

## Quick Start

1. Download the latest release for your platform from GitHub.  
   Linux/macOS example:
   ```bash
   curl -L -o clewdr.tar.gz https://github.com/BangRocket/clewdr/releases/latest/download/clewdr-linux-x64.tar.gz
   tar -xzf clewdr.tar.gz && cd clewdr-linux-x64
   chmod +x clewdr
   ```
2. Run the binary:
   ```bash
   ./clewdr
   ```
3. Open `http://127.0.0.1:8484` and enter the admin password shown in the console (or container logs if using Docker).

## Using the Web Admin

- `Dashboard` shows health, connected clients, and rate-limit status.
- `Claude` tab stores browser cookies; paste `cookie: value` pairs and save.
- `Usage` tab shows per-cookie token consumption and estimated USD cost (see below).
- `Settings` lets you rotate the admin password, set upstream proxies, and reload config without restarting.

If you forget the password, delete `clewdr.toml` and start the binary again. Docker users can mount a persistent folder for that file.

## Usage & Cost Tracking

ClewdR records every Claude request as a usage event with token counts and an estimated USD cost. The admin UI exposes a `Usage` tab with summary cards, time-series charts (cost + tokens), per-cookie sparklines + drill-down detail, and a Graveyard view of dead cookies with their final snapshot.

- **Where data lives:** events stream to `history/<cookie-history-id>.jsonl` (one file per cookie, append-only). Rollover and death snapshots are persisted in `clewdr.toml` on the matching `CookieStatus` / `UselessCookie` entries, so the recent-window context survives a restart.
- **How costs are computed:** at startup ClewdR fetches the current LiteLLM model price table; if the network is unavailable it falls back to a small bundled snapshot. Each event multiplies the model's input/output/cache rates by the request's token counts.
- **Retention:** the new `history_event_retention_days` config knob (in `clewdr.toml`) bounds the on-disk JSONL log; snapshot marker lines and unparseable lines are always preserved. `history_snapshot_max_per_cookie` caps the in-memory rollover history per cookie.

Sample event line in `history/<sha>.jsonl`:

```json
{"ts":1745625600,"source":"web","model":"claude-3-5-sonnet-20241022","family":"sonnet","input_tokens":1234,"output_tokens":567,"cache_read_tokens":0,"cache_creation_tokens":0,"cost_usd":0.012}
```

## Configure Upstreams

### Claude

1. Export your Claude.ai cookies (e.g., via browser devtools).  
2. Paste them into the Claude tab; ClewdR tracks their status automatically.  
3. Optionally set an outbound proxy or fingerprint overrides if Claude blocks your region.

### Codex

ClewdR can route requests to OpenAI's Codex backend using OAuth tokens minted by the official `codex` CLI.

1. On a machine with a browser, install the [Codex CLI](https://github.com/openai/codex) and run `codex login`.
2. Open `~/.codex/auth.json` and copy its full contents.
3. In the ClewdR admin UI, open the **Codex** tab, paste the JSON into the "Add" form, give it an optional label, and submit.

ClewdR auto-refreshes access tokens on each request when within 60s of expiry; no further action needed unless the refresh token is revoked (re-run `codex login` and re-paste).

Endpoint: `POST http://127.0.0.1:8484/codex/v1/chat/completions` (OpenAI-compatible). Or, with the toggle set to `codex`, the bare `POST http://127.0.0.1:8484/v1/chat/completions` will also route here.

Supported models: `gpt-5-codex`, `gpt-5`, `gpt-4.1`, `o3`, `o4-mini`. (Codex's upstream may map these to internal model names like `gpt-5.3-codex`; pass-through behavior.)

Example client (curl):

```bash
curl http://127.0.0.1:8484/codex/v1/chat/completions \
  -H "Authorization: Bearer password-from-console" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-5",
    "messages": [{"role": "user", "content": "hello"}],
    "stream": true
  }'
```

Status badges in the admin UI:
- **valid**: ready for dispatch
- **rate-limited**: backed off until upstream Retry-After expires
- **expired**: refresh failed transiently (will be retried)
- **invalid**: refresh token rejected (re-run `codex login`)
- **banned**: account-level ban (cannot be recovered without OpenAI intervention)

## Client Examples

SillyTavern:

```json
{
  "api_url": "http://127.0.0.1:8484/v1/chat/completions",
  "api_key": "password-from-console",
  "model": "claude-3-sonnet-20240229"
}
```

Continue (VS Code):

```json
{
  "models": [
    {
      "title": "Claude via ClewdR",
      "provider": "openai",
      "model": "claude-3-sonnet-20240229",
      "apiBase": "http://127.0.0.1:8484/v1/",
      "apiKey": "password-from-console"
    }
  ]
}
```

Cursor:

```json
{
  "openaiApiBase": "http://127.0.0.1:8484/v1/",
  "openaiApiKey": "password-from-console"
}
```

## Resources

- Fork: <https://github.com/BangRocket/clewdr>  
- Upstream wiki: <https://github.com/Xerxes-2/clewdr/wiki>  

## Thanks

- Originally created by [Xerxes-2](https://github.com/Xerxes-2/clewdr); this is a fork.  
- [wreq](https://github.com/0x676e67/wreq) for the fingerprinting library.  
- [Clewd](https://github.com/teralomaniac/clewd) for many upstream ideas.  
- [Clove](https://github.com/mirrorange/clove) for Claude Code helpers.
