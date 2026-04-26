# Cookie Cost Tracking — Implementation Summary

Branch: `feature/cookie-cost-tracking` (vs `master`)
Date: 2026-04-26

## Acceptance status

- Backend `cargo test --lib --features portable`: **23 passed; 0 failed; 0 ignored**
- `cargo check --all-targets`: clean (no warnings, no errors)
- Frontend `pnpm run build`: success
  - `static/assets/index-*.js`: 695.99 kB raw / 207.96 kB gzip
  - `static/assets/index-*.css`: 34.72 kB raw / 6.92 kB gzip
  - Build emits the standard "chunks larger than 500 kB" notice (Recharts is the
    main contributor); not blocking, can be revisited via dynamic import in a
    follow-up.
- Frontend tests: no `test` script in `frontend/package.json`; nothing to run.

## Commits on the branch (32 total, oldest → newest)

```
d5320d5 feat(pricing): bundled fallback pricing table
b45630e feat(pricing): pricing table types and cost calculation
c50da2a fix(pricing): warn on unknown model and document arg order
7d4c436 feat(pricing): fetch from LiteLLM with bundled fallback
77eab45 fix(pricing): warn on lazy fallback and rename misleading test
3fad68c feat(pricing): initialize pricing table at startup
ce0ffba feat(usage): add UsageEvent, UsageSnapshot, UsageSource types
31f7ea8 feat(usage): add cost fields, snapshots, and history_id to CookieStatus
23541fd fix(usage): use hex::encode and document reset preservation
b1a28b6 feat(usage): add final_snapshot and died_at to UselessCookie
2303e3f feat(usage): add history retention config knobs
7097f02 feat(usage): UsageActor with append/query/snapshot/prune
0f67972 fix(usage): prune writer orphan, rollover stale read, tombstone sync
aad44f7 feat(usage): UsageActorHandle for typed access from request path
b507fac feat(usage): spawn UsageActor at startup
9455e6e feat(usage): record usage events with cost on hot path
faa1b7b feat(usage): capture session-reset snapshot before wiping buckets
928020e feat(usage): rollover snapshots on weekly/sonnet/opus boundaries
3336abc feat(usage): preserve final_snapshot when cookie becomes invalid
f23b17e feat(api): GET /api/usage/summary
da67555 feat(api): events, timeseries, snapshots, dead, pricing, prune endpoints
b102df6 feat(frontend): inline cost on cookie cards and final_snapshot on invalid
5ddacad feat(frontend): TypeScript types and API client for /api/usage/*
1c4cf46 feat(frontend): Usage tab with summary cards and sub-tab scaffold
a0cd068 feat(frontend): Recharts cost+token charts on Usage Overview
363c661 feat(frontend): per-cookie list with sparklines and detail drawer
735dd8e feat(frontend): wire Graveyard cards to detail drawer
f4746c7 docs: responsive audit at 6 viewports
84c2913 fix(responsive): enlarge touch targets to 44px minimum
022e575 fix(responsive): correct grid/flex layouts at small viewports
a9ae0af fix(responsive): shrink empty-state chart placeholders
d25c344 docs(responsive): record Phase 15 fixes applied
```

(Phase 16 wrap-up commit is added on top of these.)

## Files added (new)

### Rust

- `resources/pricing_fallback.json` — bundled per-model rate snapshot used
  when LiteLLM fetch fails at startup.
- `src/services/pricing.rs` — pricing table types, LiteLLM fetch, fallback
  loader, cost calculation.
- `src/services/usage_actor.rs` — Ractor-based actor that appends events to
  per-cookie JSONL, answers query/timeseries/snapshot reads, and prunes old
  events on retention.
- `src/config/usage.rs` — `UsageEvent`, `UsageSnapshot`, `UsageSource`,
  `ModelFamily` types serialised to JSONL.
- `src/api/usage.rs` — REST handlers for `/api/usage/{summary,cookie/:id/events,
  cookie/:id/timeseries,snapshots,dead,pricing,prune}`.

### Frontend

- `frontend/src/components/usage/UsageDashboard.tsx` — top-level Usage tab with
  summary cards + sub-tab nav.
- `frontend/src/components/usage/UsageOverview.tsx` — overview pane with
  cost line chart and token bar chart.
- `frontend/src/components/usage/UsagePerCookie.tsx` — per-cookie list with
  sparklines, opens detail drawer on click.
- `frontend/src/components/usage/CookieUsageDetail.tsx` — drawer with full
  cookie history (events, snapshots, totals).
- `frontend/src/components/usage/Graveyard.tsx` — dead-cookie view; cards
  open the same detail drawer.
- `frontend/src/components/usage/charts/CostLineChart.tsx` — Recharts wrapper
  for USD-over-time series.
- `frontend/src/components/usage/charts/TokenBarChart.tsx` — Recharts wrapper
  for stacked token-by-family series.
- `frontend/src/hooks/useUsageSummary.ts` — SWR hook for `/api/usage/summary`.
- `frontend/src/types/usage.types.ts` — TS shapes mirroring backend payloads.
- `frontend/src/api/index.ts` — usage endpoint client (additions to existing
  re-exported modules).
- `frontend/scripts/capture-screenshots.ts` — Playwright helper used during
  the responsive audit (not shipped at runtime).

### Docs

- `docs/plans/2026-04-26-cookie-cost-tracking-design.md` (pre-existing in
  branch — design + decision log).
- `docs/plans/2026-04-26-cookie-cost-tracking-plan.md` (pre-existing in
  branch — phase plan).
- `docs/plans/responsive-audit.md` — 6-viewport audit results + Phase 15 fix
  log.
- `docs/plans/2026-04-26-cookie-cost-tracking-summary.md` — this file.

## Files modified

### Rust

- `src/config/clewdr_config.rs` — `history_event_retention_days`,
  `history_snapshot_max_per_cookie` knobs (hot-reload-safe).
- `src/config/cookie.rs` — added cost fields, rollover snapshot list,
  stable `history_id` derivation, `reset_with_rollover`.
- `src/config/reason.rs` — `UselessCookie` gains `final_snapshot` +
  `died_at`; `history_id()` helper aligned with `CookieStatus`.
- `src/config/mod.rs` — wires new modules.
- `src/main.rs` — initialises pricing table and spawns the UsageActor at
  startup.
- `src/api/mod.rs` — registers usage routes.
- `src/router.rs` — mounts `/api/usage/*`.
- `src/services/cookie_actor.rs` — death hook records `final_snapshot`,
  rollover hooks invoke `reset_with_rollover`, hot path calls
  `try_record` with cost.
- `src/services/mod.rs` — wires pricing + usage_actor.
- `src/claude_code_state/chat.rs`, `src/claude_web_state/mod.rs` — pass
  model + token counts + cost into `add_and_bucket_usage`.
- `src/types/claude_web/response.rs` — surface cache-token counts so they
  can flow into `UsageEvent`.

### Frontend

- `frontend/package.json`, `frontend/pnpm-lock.yaml` — adds `recharts`.
- `frontend/src/App.tsx`, `frontend/src/components/layout/Header.tsx` —
  Usage tab wiring.
- `frontend/src/components/claude/CookieVisualization.tsx` — inline cost
  display on existing cookie cards + final-snapshot rendering for invalid
  cookies.
- `frontend/src/components/config/{ConfigCheckbox,ConfigForm,index}.tsx` —
  responsive touch-target fixes from Phase 15.
- `frontend/src/locales/{en,zh}/translation.json` — Usage tab strings
  (zh strings authored alongside en; flagged for native-speaker re-audit
  below).
- `frontend/src/types/cookie.types.ts` — extra cost / `history_id` fields
  in the `Cookie` shape.

### Repo metadata

- `.gitignore` — ignores the runtime `history/` directory + screenshot
  scratch outputs.
- `README.md` — adds **Usage & Cost Tracking** section + Usage tab bullet
  in **Using the Web Admin** (this commit).

## Test counts

- Rust: 23 lib tests (covers pricing, usage_actor, cookie history_id,
  rollover, cookie parsing, claude middleware billing, claude web/code
  type deserialisation).
- Frontend: no automated test suite present.

## Bundle delta

Recharts is the main delta vs pre-feature. Final bundle (gzip):

- JS: 207.96 kB
- CSS:   6.92 kB

## Operational notes / smoke-test guide

- **History directory**: `./history/` relative to ClewdR's cwd. Each
  cookie writes to `history/<sha-of-cookie>.jsonl`. Snapshot marker lines
  are JSONL with `kind: "snapshot"` and survive pruning.
- **Retention**: `history_event_retention_days` (in `clewdr.toml`) is
  optional. When unset, no retention pruning runs. Set e.g. `30` to
  prune events older than 30 days. Snapshot markers + unparseable lines
  are always preserved.
- **Pricing**: at startup, ClewdR fetches the LiteLLM rate table over
  HTTPS. On any error the bundled `resources/pricing_fallback.json` is
  used; the fallback path emits a `WARN` log so it shows up in console.
  Unknown model IDs log a warning and are billed at $0 (event still
  recorded).
- **Smoke test (manual, by Joshua)**:
  1. Start ClewdR with at least one valid cookie.
  2. Send a few requests through `/v1/messages` and `/v1/chat/completions`.
  3. Confirm `history/<sha>.jsonl` grows, with one event per request.
  4. Open `Usage` tab; summary cards should populate, Overview charts
     should render once events exist, Per-Cookie list should show
     sparklines, Graveyard should show any invalidated cookies with
     their `final_snapshot`.
  5. (Optional) Trigger a rollover by adjusting clock or by waiting
     across a weekly/sonnet/opus boundary; confirm a snapshot marker
     appears in the JSONL and the rollover history shows up in the
     detail drawer.

## Outstanding items deferred to follow-ups

These were identified during the responsive audit (Phases 14–15) and
explicitly deferred from Phase 16 since they need either real seeded
data or a native zh-speaker review:

- **[empty-state] populated-state re-audit**: the Phase 14 audit ran
  against an empty cookie list. Re-audit Usage Overview / Per-Cookie /
  Graveyard at the same 6 viewports once seeded with valid + dead
  cookies, looking for layout overflow, table truncation, and chart
  legibility regressions.
- **[i18n] zh locale re-audit**: the zh strings added for the Usage
  tab were authored alongside the en strings as best-effort. A native
  zh speaker should review `frontend/src/locales/zh/translation.json`
  for the `usage.*`, `graveyard.*`, and `cookieDetail.*` keys.
- **[touch] CookieVisualization toggle/retry buttons**: the
  context-toggle (`px-3 py-1.5 text-xs`) and retry button on
  config-error states (`py-1 px-3`) on the Claude tab are below the
  44 px minimum target. They were left as-is in Phase 15 because they
  only appear in populated states that the static audit could not
  reach. Bump them to `min-h-[44px]` (or equivalent) when re-auditing
  populated states.
- **[layout] non-usage tabs constrained to `md:max-w-xl`**: the
  Claude / Config / Token tabs still render in a ~576 px column on
  `lg`+. This is intentional design today, but worth revisiting if any
  of those forms gains side-by-side content.
- **[bundle] Recharts code-split**: vite build emits the standard "
  chunks larger than 500 kB" advisory. Splitting the Usage tab into a
  lazy chunk (`React.lazy` + `Suspense`) would drop main-bundle gzip
  noticeably; unblocked by anything in this branch.
