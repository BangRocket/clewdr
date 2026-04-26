# Cookie Cost Tracking, Expiration History, and Responsive UI — Design

**Date:** 2026-04-26
**Status:** Approved
**Author:** Joshua Heidorn (with Claude)

## Goal

Add ccusage-style cost tracking per cookie, preserve usage history across reset windows and terminal expirations, and bring the admin UI to mobile/desktop parity.

## Scope

In scope:
- USD cost calculation per request, summed into existing per-period buckets
- Append-only JSONL event log per cookie under `history/<sha256_16>.jsonl`
- Snapshot capture on every reset-window boundary and on terminal cookie death
- New `/api/usage/*` admin endpoints
- New "Usage" tab with summary cards, time-series charts (Recharts), per-cookie detail drawer, and a graveyard view for dead cookies
- Inline cost display on existing cookie cards
- Full responsive audit and redesign pass at six viewport widths

Out of scope:
- CSV/Excel export
- Webhook/email cost alerts
- Multi-tenant views
- Forecasting

## Decisions

| # | Question | Decision |
|---|----------|----------|
| 1 | What does "expire" mean? | Both: rate-limit window passing AND terminal `UselessCookie` invalidation |
| 2 | Pricing source | Fetch LiteLLM pricing JSON at startup; bundled hardcoded fallback |
| 3 | History granularity | Per-request rolling log + snapshot on rollover |
| 4 | Storage | Append-only JSONL per cookie under `history/`; snapshots back into `clewdr.toml` |
| 5 | UI shape | Inline cost on cookie cards + dedicated Usage tab |
| 6 | Mobile parity | Full audit + redesign pass at 375/414/768/1024/1280/1920 |
| 7 | Charts | Recharts |
| 8 | History filename | First 16 hex chars of SHA-256(cookie) |
| 9 | Retention | Configurable in `clewdr.toml`; default = keep everything |
| 10 | Web vs Code | Single combined stream per cookie, `source` field on each event |

## Architecture

Approach: **dedicated `UsageActor`** alongside the existing `cookie_actor`.

```
request path
   │
   ▼
add_and_bucket_usage()  ──► pricing::cost()  ──► CookieStatus.{period}_cost_usd += cost
                                                       │
                                                       ▼
                              USAGE_ACTOR.try_record(history_id, UsageEvent)
                                                       │  (mpsc::try_send, drop on full)
                                                       ▼
                                       UsageActor task (separate tokio task)
                                                       │
                          ┌────────────────────────────┼───────────────────────────┐
                          ▼                            ▼                           ▼
            BufWriter<File> per cookie         daily prune tick           snapshot/tombstone
            history/<sha>.jsonl                                           returns UsageSnapshot
            flush every 2s
```

Boundary crossings (reset windows, terminal death) call `UsageActor::rollover` / `tombstone`,
which return a `UsageSnapshot` that the caller persists into `CookieStatus.snapshots` or
`UselessCookie.final_snapshot`.

## Data Model

### New types — `src/config/usage.rs`

```rust
pub enum UsageSource { Web, Code }

pub struct UsageEvent {
    pub ts: i64,
    pub source: UsageSource,
    pub model: String,
    pub family: ModelFamily,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
}

pub enum SnapshotTrigger {
    SessionReset, WeeklyReset, WeeklySonnetReset, WeeklyOpusReset, Death(Reason),
}

pub struct UsageSnapshot {
    pub closed_at: i64,
    pub trigger: SnapshotTrigger,
    pub period_start: i64,
    pub usage: UsageBreakdown,
    pub cost_usd: f64,
    pub event_count: u64,
}
```

### Changes to existing types

`CookieStatus` (in `src/config/cookie.rs`):
- `session_cost_usd: f64`, `weekly_cost_usd: f64`, `weekly_sonnet_cost_usd: f64`,
  `weekly_opus_cost_usd: f64`, `lifetime_cost_usd: f64` (running totals matching existing buckets)
- `snapshots: Vec<UsageSnapshot>` (preserved across rollovers)
- `history_id()` method computing `sha256(cookie)[..16]` lazily; not serialized

`UselessCookie`:
- `final_snapshot: Option<UsageSnapshot>`
- `died_at: i64`

All new fields use `#[serde(default)]`. Existing `clewdr.toml` files load unchanged.

### Pricing — `src/services/pricing.rs`

- `pricing::init().await` runs once at startup from `main.rs`
- Tries to fetch `https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json`
- On failure, falls back to bundled `resources/pricing_fallback.json` (also `include_str!` into a const for ultimate fallback)
- Stored in `OnceLock<PricingTable>` for synchronous access from the hot path
- `pricing::cost(model, input, output, cache_read, cache_create) -> f64`

### Config additions — `src/config/clewdr_config.rs`

```toml
# Defaults: keep everything (both unset)
history_event_retention_days = 90        # optional
history_snapshot_max_per_cookie = 0      # 0 = unlimited; optional
```

## Backend Wiring

### `src/services/usage_actor.rs`

```rust
enum UsageMsg {
    Record(String, UsageEvent),
    Rollover(String, SnapshotTrigger, UsageBreakdown, f64, oneshot::Sender<UsageSnapshot>),
    Tombstone(String, UsageSnapshot, Reason),
    QueryEvents(String, QueryRange, oneshot::Sender<Vec<UsageEvent>>),
    QuerySnapshots(String, oneshot::Sender<Vec<UsageSnapshot>>),
    QueryTimeSeries(String, Bucket, QueryRange, oneshot::Sender<Vec<TimeBucket>>),
    PruneNow(oneshot::Sender<PruneStats>),
}
```

- `HashMap<String, BufWriter<File>>` per active cookie's JSONL, lazy-opened
- `tokio::select!` on incoming messages and a 2-second flush tick
- Daily tick walks `history/`, runs prune (if configured) by streaming-rewriting each JSONL atomically (write to `<id>.jsonl.tmp`, fsync, rename)
- Bounded mpsc channel (1024 deep); senders use `try_send`. Overflow logs a warn throttled to once/min and increments `usage_events_dropped`

### Hot-path integration

`add_and_bucket_usage` signature extended:

```rust
pub fn add_and_bucket_usage(
    &mut self,
    input: u64, output: u64,
    cache_read: u64, cache_create: u64,
    family: ModelFamily,
    model: &str,
    source: UsageSource,
)
```

Existing callers updated to pass model + source (already in scope at call sites in
`src/claude_code_state/chat.rs`, `src/claude_web_state/mod.rs`, `src/router.rs`,
`src/types/claude_web/response.rs`).

After existing token bucket math:
- compute `cost = pricing::cost(...)`
- accumulate into `session_cost_usd` / `weekly_cost_usd` / `weekly_sonnet_cost_usd` /
  `weekly_opus_cost_usd` / `lifetime_cost_usd` matching the existing token bucket logic
- `USAGE_ACTOR.try_record(self.history_id(), UsageEvent {...})`

### Rollover hooks

`CookieStatus::reset()` — wrap the existing `reset_time` boundary crossing:
```rust
let snap = USAGE_ACTOR.rollover_blocking(
    self.history_id(),
    SnapshotTrigger::SessionReset,
    self.session_usage.clone(),
    self.session_cost_usd,
);
self.snapshots.push(snap);
// existing reset of session_usage / weekly_usage / weekly_sonnet_usage / weekly_opus_usage
// plus reset of session_cost_usd / weekly_cost_usd / etc.
```

The same pattern applied to weekly/weekly-sonnet/weekly-opus boundary crossings (these
checkpoints are currently scattered; implementation will trace them via grep on
`*_resets_at` and apply consistently).

### Death hook

In `cookie_actor::collect()`, when transitioning to `state.invalid` (Free / Restricted /
manual delete with reason), build a final `UsageSnapshot` from `lifetime_usage`, call
`UsageActor::tombstone`, attach to `UselessCookie.final_snapshot`, and set `died_at`.

### Pricing init

`main.rs` calls `pricing::init().await` after config load and before serving routes. Logs
"pricing: fetched fresh from LiteLLM (N models)" or "pricing: using bundled fallback
(reason: <reason>, N models)".

## API

All routes admin-auth gated, mounted at `/api/usage`:

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/usage/summary` | Dashboard aggregate (totals + per-cookie summary) |
| GET | `/api/usage/cookie/:history_id/events?from&to&source` | Raw event list, range-filtered |
| GET | `/api/usage/cookie/:history_id/snapshots` | Preserved snapshots for a cookie |
| GET | `/api/usage/cookie/:history_id/timeseries?bucket&from&to` | Server-bucketed time series |
| GET | `/api/usage/dead` | List of UselessCookie with `final_snapshot` |
| GET | `/api/usage/pricing` | Current pricing table + last-updated timestamp |
| POST | `/api/usage/prune` | Manual retention sweep |

Existing `GET /api/cookies` extended with `cost_usd` fields, `history_id`, and
`final_snapshot` on invalid entries.

No new mutating endpoints beyond manual prune.

## Frontend

### Tab

New "Usage" tab in `App.tsx` next to existing tabs.

### Components — `frontend/src/components/usage/`

- `UsageDashboard.tsx` — top-level. Summary cards row + sub-tabs (Overview / By Cookie / Graveyard)
- `UsageOverview.tsx` — global cost-over-time chart (stacked by family), model breakdown donut, top-N expensive cookies
- `UsagePerCookie.tsx` — list with sparklines; click → `CookieUsageDetail` drawer
- `CookieUsageDetail.tsx` — line chart with bucket selector (hour/day) and source filter (web/code/all), snapshots table
- `Graveyard.tsx` — dead cookies grid with reason, died_at, final_snapshot
- `charts/CostLineChart.tsx`, `charts/TokenBarChart.tsx`, `charts/FamilyDonut.tsx` — Recharts wrappers

### Inline cost on existing CookieVisualization

- "Cost: $X.XX" line under existing token counts on each card
- Invalid cookies show `final_snapshot` summary + "View history" link

### State

- `useUsageSummary()` — polls 5s when tab focused (visibility-aware), paused otherwise
- `useCookieEvents(id, range, bucket)` — caches by tuple key

### i18n

Add `usage.*` namespace to `frontend/src/locales/en/translation.json` and
`frontend/src/locales/zh/translation.json`. All copy goes through `t()`.

### States

Every component handles loading (skeleton), empty (helper text), error (retry button) —
matches existing `CookieVisualization` pattern.

## Mobile / Desktop Parity

### Audit (precedes any redesign)

1. `bun run dev` (or `pnpm dev`) in `frontend/`
2. Screenshot every tab and interactive flow at 375 / 414 / 768 / 1024 / 1280 / 1920
3. Document concrete issues in `docs/plans/responsive-audit.md` — each entry: viewport,
   screenshot, problem, category (layout / density / touch / navigation)

### Likely fixes (validated against audit)

- **Containers:** CSS Grid `repeat(auto-fit, minmax(280px, 1fr))` for cookie lists; 1-col
  ≤640px, 2-col 640–1024px, 3-col ≥1024px
- **Touch targets:** 44×44 px minimum on all buttons; cookie value reveal/copy gets larger
  tap zones with `active:` feedback
- **Navigation:** horizontal-scroll-snap tab bar on narrow widths if overflow; flex-wrap on
  desktop
- **Density:** mobile collapses secondary card info into a `<details>` disclosure; primary
  shows status + cost + reset countdown
- **Charts:** Recharts `<ResponsiveContainer>` with min-height 240px; thin axis ticks at
  narrow widths; legend below chart on mobile
- **Tables:** "card list at small / table at large" — `hidden md:block` on `<table>`,
  `block md:hidden` on the card-list rendering
- **Drawer vs sheet:** `CookieUsageDetail` is a right drawer ≥768px, full-screen
  bottom-sheet on mobile
- **Forms:** textarea grows on mobile, buttons stack full-width below 480px
- **Typography:** fluid scale via `text-base` defaults + `sm:` `lg:` modifiers on headings
- **Safe areas:** `env(safe-area-inset-*)` padding on fixed elements

### Acceptance

- Every audit issue closed (linked to fix)
- Re-screenshot at all six breakpoints; visual diff
- No horizontal scrollbars at any breakpoint except inside intentionally-scrollable
  elements (cookie value reveal, code blocks)
- Lighthouse mobile accessibility score ≥90

## Error Handling

| Failure | Behavior |
|---------|----------|
| Pricing fetch fails at startup | Log warn, use bundled fallback, never block startup |
| Unknown model in pricing table | cost = 0, log debug once per model, event still recorded |
| JSONL append fails (disk/perm) | Log error, drop event, increment `usage_events_dropped` |
| Channel full | `try_send` drops, throttled warn (once/min) |
| Snapshot rollover race | Actor serializes per-`history_id`; snapshots append-only; worst case is duplicate snapshot |
| Crash mid-batch | Up to 2s of unflushed events lost — acceptable for telemetry |
| Cookie deleted via API | JSONL file deleted; if cookie returns later, history starts fresh |
| Pricing JSON schema drift | Fallback file used; warn logged with first failed model name |

## Edge Cases

- First observation of a cookie has no JSONL → `OpenOptions::append(true).create(true)`
  handles it
- Cache tokens absent in upstream response → treated as 0
- Concurrent requests on same cookie → independent events, append-order is wall-clock
- Clock skew → use existing `chrono::Utc::now().timestamp()` consistently with
  existing `reset_time` checks
- `history/` dir missing → created on first append
- Migration: all new fields `#[serde(default)]`; existing config files load unchanged

## Testing

### Backend (cargo test)

1. `pricing::cost()` — known rates produce known dollar amounts (snapshot test)
2. `pricing::init()` — fallback path triggers on bad URL (test override)
3. `UsageActor::record` → file contains expected JSONL line (tempdir)
4. `UsageActor::rollover` → snapshot returned matches summed events; subsequent events go
   to fresh window
5. `CookieStatus::reset()` → snapshot pushed, buckets cleared, cost cleared
6. `add_and_bucket_usage` → emits `Record` with correct fields
7. `cookie_actor::collect` Free/Restricted path → `final_snapshot` attached
8. Retention prune — old events dropped, new kept, file rewritten atomically
9. Channel-full path — `try_send` failure does not panic, counter increments

### Frontend (vitest + Testing Library)

1. `UsageDashboard` renders summary; loading/empty/error states
2. `useUsageSummary` polls only when tab focused (mock `document.visibilityState`)
3. `CookieUsageDetail` drawer respects bucket/source filters
4. `Graveyard` shows `final_snapshot` when present, "no data" when missing
5. Inline cost rendered on `CookieVisualization` cards

### E2E (Playwright)

1. Submit cookie → request → cost appears inline → event in Usage tab time series
2. Force rollover (test endpoint or injected clock) → snapshot in history
3. Delete cookie → JSONL removed → graveyard does not show it
4. Responsive: capture screenshots at all six viewports for both tabs; assert no
   horizontal overflow

## Files Touched (anticipated)

**Backend:**
- `src/config/cookie.rs` (extend CookieStatus, UselessCookie, signature change)
- `src/config/usage.rs` (new)
- `src/config/clewdr_config.rs` (retention config)
- `src/services/usage_actor.rs` (new)
- `src/services/pricing.rs` (new)
- `src/services/cookie_actor.rs` (death hook)
- `src/services/mod.rs`
- `src/api/usage.rs` (new) + `src/api/mod.rs` route registration
- `src/api/misc.rs` (extend cookies response with cost + history_id + final_snapshot)
- `src/main.rs` (pricing init, usage actor spawn)
- `src/router.rs` (route mount)
- `src/claude_code_state/chat.rs`, `src/claude_web_state/mod.rs`,
  `src/types/claude_web/response.rs` (signature update at call sites)
- `resources/pricing_fallback.json` (new)

**Frontend:**
- `frontend/src/App.tsx` (tab routing)
- `frontend/src/api/index.ts` (new endpoints)
- `frontend/src/types/usage.types.ts` (new)
- `frontend/src/components/usage/*` (new)
- `frontend/src/components/claude/CookieVisualization.tsx` (inline cost)
- `frontend/src/locales/en/translation.json`, `zh/translation.json` (usage namespace)
- `frontend/package.json` (Recharts dep)

**Docs:**
- `docs/plans/responsive-audit.md` (produced during audit)
