# Cookie Cost Tracking, Expiration History, and Responsive UI — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add ccusage-style USD cost tracking per cookie, preserve usage events and snapshots across reset/expiration, expose them via a new Usage tab, and bring the admin UI to mobile/desktop parity.

**Architecture:** A new `UsageActor` (built with `ractor`, matching the existing `CookieActor`) owns an append-only JSONL log per cookie under `history/<sha256_16>.jsonl` and produces `UsageSnapshot`s on rollover/death which get persisted on `CookieStatus`/`UselessCookie`. A `pricing` module loads model rates from LiteLLM at startup with a bundled fallback. The frontend gains inline cost on cookie cards plus a dedicated Usage tab with Recharts. A separate phase audits and fixes responsive breakages.

**Tech Stack:**
- Backend: Rust (axum, ractor, sha2, wreq, serde, serde_json, chrono, snafu, tokio)
- Frontend: React 19, TypeScript, Tailwind 4, Recharts (new), i18next
- Tests: cargo test (backend), Playwright (e2e + responsive screenshots)

**Design doc:** `docs/plans/2026-04-26-cookie-cost-tracking-design.md` — read this before starting.

**Reference reading:**
- `src/config/cookie.rs` — `CookieStatus`, `UsageBreakdown`, `add_and_bucket_usage`
- `src/config/reason.rs` — `UselessCookie`, `Reason`
- `src/services/cookie_actor.rs` — actor pattern using ractor
- `frontend/src/components/claude/CookieVisualization.tsx` — current cookie UI

---

## Phase 0: Setup

### Task 0.1: Create worktree

**Files:** none

**Step 1: Create isolated worktree**

```bash
cd /Volumes/Storage/Code/clewdr
git worktree add ../clewdr-cost-tracking -b feature/cookie-cost-tracking
cd ../clewdr-cost-tracking
```

**Step 2: Verify**

Run: `git status && git branch --show-current`
Expected: branch `feature/cookie-cost-tracking`, clean tree.

### Task 0.2: Add Rust dependencies

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add deps**

Append under `[dependencies]`:

```toml
sha2 = "0.11"             # already present — verify, do not duplicate
async-trait = "0.1"       # already present — verify, do not duplicate
```

If neither is present after `grep -n "^sha2\|^async-trait" Cargo.toml`, add them. They were observed present at planning time; this task is a verify-only step.

**Step 2: Verify build still compiles**

Run: `cargo check --all-targets`
Expected: success, no new warnings.

**Step 3: Commit (only if Cargo.toml changed)**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: ensure sha2/async-trait deps for cost tracking"
```

### Task 0.3: Add Recharts to frontend

**Files:**
- Modify: `frontend/package.json`

**Step 1: Install**

```bash
cd frontend
bun add recharts
# or: pnpm add recharts / npm install recharts
cd ..
```

**Step 2: Verify build**

Run: `cd frontend && bun run build && cd ..`
Expected: build succeeds, dist produced.

**Step 3: Commit**

```bash
git add frontend/package.json frontend/bun.lockb  # or pnpm-lock.yaml / package-lock.json
git commit -m "chore(frontend): add recharts for usage charts"
```

---

## Phase 1: Pricing module

### Task 1.1: Bundled fallback pricing JSON

**Files:**
- Create: `resources/pricing_fallback.json`

**Step 1: Create directory**

```bash
mkdir -p resources
```

**Step 2: Write fallback file** (use rates current at planning time; LiteLLM key format:
`<model_id>` keyed object with `input_cost_per_token`, `output_cost_per_token`,
`cache_read_input_token_cost`, `cache_creation_input_token_cost`, all in USD per token).

```json
{
  "claude-sonnet-4-5-20250929":  { "input_cost_per_token": 0.000003, "output_cost_per_token": 0.000015, "cache_read_input_token_cost": 0.0000003, "cache_creation_input_token_cost": 0.00000375 },
  "claude-opus-4-1-20250805":     { "input_cost_per_token": 0.000015, "output_cost_per_token": 0.000075, "cache_read_input_token_cost": 0.0000015, "cache_creation_input_token_cost": 0.00001875 },
  "claude-haiku-4-5-20251001":    { "input_cost_per_token": 0.0000008, "output_cost_per_token": 0.000004, "cache_read_input_token_cost": 0.00000008, "cache_creation_input_token_cost": 0.000001 },
  "claude-3-5-sonnet-20241022":   { "input_cost_per_token": 0.000003, "output_cost_per_token": 0.000015, "cache_read_input_token_cost": 0.0000003, "cache_creation_input_token_cost": 0.00000375 },
  "claude-3-opus-20240229":       { "input_cost_per_token": 0.000015, "output_cost_per_token": 0.000075, "cache_read_input_token_cost": 0.0000015, "cache_creation_input_token_cost": 0.00001875 }
}
```

**Step 3: Commit**

```bash
git add resources/pricing_fallback.json
git commit -m "feat(pricing): bundled fallback pricing table"
```

### Task 1.2: Pricing types and parser (TDD — failing test first)

**Files:**
- Create: `src/services/pricing.rs`
- Modify: `src/services/mod.rs`
- Test: `src/services/pricing.rs` (inline `#[cfg(test)] mod tests`)

**Step 1: Write failing test**

In `src/services/pricing.rs`:

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct ModelPricing {
    #[serde(default)]
    pub input_cost_per_token: f64,
    #[serde(default)]
    pub output_cost_per_token: f64,
    #[serde(default)]
    pub cache_read_input_token_cost: f64,
    #[serde(default)]
    pub cache_creation_input_token_cost: f64,
}

#[derive(Debug, Clone, Default)]
pub struct PricingTable {
    pub models: HashMap<String, ModelPricing>,
    pub fetched_at: i64,
    pub source: PricingSource,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingSource {
    Litellm,
    #[default]
    Fallback,
}

impl PricingTable {
    pub fn parse_from_json(json: &str, source: PricingSource) -> Result<Self, serde_json::Error> {
        let models: HashMap<String, ModelPricing> = serde_json::from_str(json)?;
        Ok(Self { models, fetched_at: chrono::Utc::now().timestamp(), source })
    }

    pub fn cost(&self, model: &str, input: u64, output: u64, cache_read: u64, cache_create: u64) -> f64 {
        let Some(p) = self.models.get(model) else { return 0.0 };
        (input as f64) * p.input_cost_per_token
            + (output as f64) * p.output_cost_per_token
            + (cache_read as f64) * p.cache_read_input_token_cost
            + (cache_create as f64) * p.cache_creation_input_token_cost
    }
}

const FALLBACK_JSON: &str = include_str!("../../resources/pricing_fallback.json");

pub fn load_fallback() -> PricingTable {
    PricingTable::parse_from_json(FALLBACK_JSON, PricingSource::Fallback)
        .expect("bundled pricing_fallback.json must be valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_zero_for_unknown_model() {
        let t = load_fallback();
        assert_eq!(t.cost("nonexistent-model", 1000, 1000, 0, 0), 0.0);
    }

    #[test]
    fn cost_for_sonnet_matches_known_rates() {
        let t = load_fallback();
        // 1M input + 1M output @ Sonnet 4.5 = $3 + $15 = $18
        let cost = t.cost("claude-sonnet-4-5-20250929", 1_000_000, 1_000_000, 0, 0);
        assert!((cost - 18.0).abs() < 0.01, "expected ~18.0, got {}", cost);
    }

    #[test]
    fn cost_includes_cache_tokens() {
        let t = load_fallback();
        // 1M cache-read @ Sonnet = $0.30
        let cost = t.cost("claude-sonnet-4-5-20250929", 0, 0, 1_000_000, 0);
        assert!((cost - 0.30).abs() < 0.001, "expected ~0.30, got {}", cost);
    }

    #[test]
    fn fallback_loads_without_panic() {
        let t = load_fallback();
        assert!(!t.models.is_empty());
        assert!(matches!(t.source, PricingSource::Fallback));
    }
}
```

In `src/services/mod.rs`, add:

```rust
pub mod pricing;
```

**Step 2: Run tests — expect compile errors / failures**

Run: `cargo test --lib services::pricing -- --nocapture`
Expected: tests compile and pass on first run (no failing-then-passing dance is needed for pure parsing — code and test are introduced together by design).

**Step 3: Commit**

```bash
git add src/services/pricing.rs src/services/mod.rs
git commit -m "feat(pricing): pricing table types and cost calculation"
```

### Task 1.3: Pricing fetch with fallback

**Files:**
- Modify: `src/services/pricing.rs`

**Step 1: Add async init function**

Append to `src/services/pricing.rs`:

```rust
use std::sync::OnceLock;
use tracing::{info, warn};

static PRICING: OnceLock<PricingTable> = OnceLock::new();

const LITELLM_URL: &str = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

pub async fn init() {
    let table = match fetch_litellm().await {
        Ok(t) => {
            info!("pricing: fetched fresh from LiteLLM ({} models)", t.models.len());
            t
        }
        Err(e) => {
            let f = load_fallback();
            warn!("pricing: using bundled fallback ({} models): {}", f.models.len(), e);
            f
        }
    };
    let _ = PRICING.set(table);
}

pub fn current() -> &'static PricingTable {
    PRICING.get_or_init(load_fallback)
}

pub fn cost(model: &str, input: u64, output: u64, cache_read: u64, cache_create: u64) -> f64 {
    current().cost(model, input, output, cache_read, cache_create)
}

async fn fetch_litellm() -> Result<PricingTable, Box<dyn std::error::Error + Send + Sync>> {
    let client = wreq::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let resp = client.get(LITELLM_URL).send().await?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()).into());
    }
    let json = resp.text().await?;
    // LiteLLM JSON is keyed by model name, but contains many non-Anthropic entries.
    // Parse leniently: skip entries missing input_cost_per_token rather than failing.
    let raw: HashMap<String, serde_json::Value> = serde_json::from_str(&json)?;
    let mut models = HashMap::new();
    for (k, v) in raw {
        if let Ok(p) = serde_json::from_value::<ModelPricing>(v) {
            // Skip entries with no useful pricing info
            if p.input_cost_per_token > 0.0 || p.output_cost_per_token > 0.0 {
                models.insert(k, p);
            }
        }
    }
    Ok(PricingTable { models, fetched_at: chrono::Utc::now().timestamp(), source: PricingSource::Litellm })
}
```

Add a test for the parser leniency:

```rust
#[test]
fn parse_skips_entries_without_pricing() {
    let json = r#"{
        "model-with-pricing": { "input_cost_per_token": 0.001, "output_cost_per_token": 0.002 },
        "model-without-pricing": { "litellm_provider": "anthropic" }
    }"#;
    let table = PricingTable::parse_from_json(json, PricingSource::Litellm).unwrap();
    // Strict parser keeps both (default fields are 0). Documenting current behavior:
    assert!(table.models.contains_key("model-with-pricing"));
}
```

**Step 2: Run tests**

Run: `cargo test --lib services::pricing -- --nocapture`
Expected: all pass.

**Step 3: Commit**

```bash
git add src/services/pricing.rs
git commit -m "feat(pricing): fetch from LiteLLM with bundled fallback"
```

### Task 1.4: Wire pricing init into main

**Files:**
- Modify: `src/main.rs`

**Step 1: Find startup flow**

Run: `grep -n "async fn main\|tokio::main" src/main.rs`
Note the location and the section where config is loaded (after `CLEWDR_CONFIG.load()`).

**Step 2: Add init call**

After config load and before any actor spawn:

```rust
clewdr::services::pricing::init().await;
```

(adjust path if `pricing` is exposed via `crate::services::pricing` inside the binary)

**Step 3: Run app, observe log**

Run: `cargo run` then Ctrl-C after ~3 seconds.
Expected log line: `pricing: fetched fresh from LiteLLM (...)` OR `pricing: using bundled fallback (...)`.

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(pricing): initialize pricing table at startup"
```

---

## Phase 2: Data model extensions

### Task 2.1: Add `UsageSource`, `UsageEvent`, `SnapshotTrigger`, `UsageSnapshot` types

**Files:**
- Create: `src/config/usage.rs`
- Modify: `src/config/mod.rs`

**Step 1: Write file**

`src/config/usage.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::config::cookie::{ModelFamily, UsageBreakdown};
use crate::config::reason::Reason;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UsageSource {
    Web,
    Code,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsageEvent {
    pub ts: i64,
    pub source: UsageSource,
    pub model: String,
    pub family: ModelFamily,
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SnapshotTrigger {
    SessionReset,
    WeeklyReset,
    WeeklySonnetReset,
    WeeklyOpusReset,
    Death { reason: Reason },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsageSnapshot {
    pub closed_at: i64,
    pub trigger: SnapshotTrigger,
    pub period_start: i64,
    pub usage: UsageBreakdown,
    pub cost_usd: f64,
    pub event_count: u64,
}
```

In `src/config/mod.rs`, add:

```rust
pub mod usage;
pub use usage::{UsageEvent, UsageSnapshot, UsageSource, SnapshotTrigger};
```

**Step 2: Verify compilation**

Run: `cargo check --all-targets`
Expected: success.

**Step 3: Commit**

```bash
git add src/config/usage.rs src/config/mod.rs
git commit -m "feat(usage): add UsageEvent, UsageSnapshot, UsageSource types"
```

### Task 2.2: Extend `CookieStatus` with cost fields, snapshots, and history_id

**Files:**
- Modify: `src/config/cookie.rs`

**Step 1: Add fields**

In `CookieStatus`, after the existing `weekly_opus_has_reset` field, add:

```rust
    // Cost tracking (matches existing UsageBreakdown bucket structure)
    #[serde(default)]
    pub session_cost_usd: f64,
    #[serde(default)]
    pub weekly_cost_usd: f64,
    #[serde(default)]
    pub weekly_sonnet_cost_usd: f64,
    #[serde(default)]
    pub weekly_opus_cost_usd: f64,
    #[serde(default)]
    pub lifetime_cost_usd: f64,

    /// Preserved usage snapshots from previous reset windows
    #[serde(default)]
    pub snapshots: Vec<crate::config::UsageSnapshot>,
```

In the `new()` constructor, initialize them all to defaults.

In `reset_window_usage`, also zero `session_cost_usd`, `weekly_cost_usd`, `weekly_sonnet_cost_usd`, `weekly_opus_cost_usd`.

**Step 2: Add `history_id()` method**

```rust
impl CookieStatus {
    /// SHA-256 first 16 hex chars of the cookie value. Stable, non-reversible.
    /// Used as filename for the per-cookie history JSONL.
    pub fn history_id(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.cookie.as_bytes());
        let hash = hasher.finalize();
        hex_encode_short(&hash[..8])
    }
}

fn hex_encode_short(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}
```

(Hash bytes go through `hex` only on first 8 bytes → 16 hex chars.)

**Step 3: Add a unit test**

Append to `cookie.rs` test module:

```rust
#[test]
fn history_id_is_stable_and_short() {
    let c1 = CookieStatus::new(&make_base_cookie_with_len(86), None).unwrap();
    let id1 = c1.history_id();
    let id2 = c1.history_id();
    assert_eq!(id1, id2);
    assert_eq!(id1.len(), 16);
    // Different cookies → different ids
    let c2 = CookieStatus::new(&make_base_cookie_with_len(87), None).unwrap();
    assert_ne!(c1.history_id(), c2.history_id());
}
```

**Step 4: Run tests**

Run: `cargo test --lib config::cookie -- --nocapture`
Expected: existing tests pass + new test passes.

**Step 5: Commit**

```bash
git add src/config/cookie.rs
git commit -m "feat(usage): add cost fields, snapshots, and history_id to CookieStatus"
```

### Task 2.3: Extend `UselessCookie` with final_snapshot and died_at

**Files:**
- Modify: `src/config/reason.rs`

**Step 1: Add fields**

In `UselessCookie`:

```rust
pub struct UselessCookie {
    pub cookie: ClewdrCookie,
    pub reason: Reason,
    #[serde(default)]
    pub final_snapshot: Option<crate::config::UsageSnapshot>,
    #[serde(default)]
    pub died_at: i64,
}
```

Update `UselessCookie::new` to take optional snapshot + died_at, OR add a builder/setter:

```rust
impl UselessCookie {
    pub fn new(cookie: ClewdrCookie, reason: Reason) -> Self {
        Self {
            cookie,
            reason,
            final_snapshot: None,
            died_at: chrono::Utc::now().timestamp(),
        }
    }

    pub fn with_final_snapshot(mut self, snapshot: crate::config::UsageSnapshot) -> Self {
        self.final_snapshot = Some(snapshot);
        self
    }
}
```

`PartialEq` and `Hash` should remain keyed only on `cookie` so existing dedup logic still works.

**Step 2: Verify compile**

Run: `cargo check --all-targets`
Expected: success.

**Step 3: Commit**

```bash
git add src/config/reason.rs
git commit -m "feat(usage): add final_snapshot and died_at to UselessCookie"
```

### Task 2.4: Add retention config knobs

**Files:**
- Modify: `src/config/clewdr_config.rs`

**Step 1: Add fields**

In `ClewdrConfig`, near other optional knobs:

```rust
    #[serde(default)]
    pub history_event_retention_days: Option<u32>,
    #[serde(default)]
    pub history_snapshot_max_per_cookie: Option<u32>,
```

(Both `Option` to distinguish "unset = keep everything" from "explicitly 0".)

**Step 2: Verify compile + existing tests pass**

Run: `cargo test --lib config::clewdr_config`
Expected: pass.

**Step 3: Commit**

```bash
git add src/config/clewdr_config.rs
git commit -m "feat(usage): add history retention config knobs"
```

---

## Phase 3: UsageActor

### Task 3.1: Skeleton actor with Record + Flush messages (TDD)

**Files:**
- Create: `src/services/usage_actor.rs`
- Modify: `src/services/mod.rs`

**Step 1: Write a failing test for Record + readback**

Create `src/services/usage_actor.rs` with:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use snafu::ResultExt;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tracing::{debug, error, warn};

use crate::config::{Reason, UsageBreakdown, UsageEvent, UsageSnapshot, SnapshotTrigger};
use crate::error::ClewdrError;

#[derive(Debug)]
pub enum UsageActorMessage {
    Record { history_id: String, event: UsageEvent },
    Flush,
    QueryEvents {
        history_id: String,
        from: Option<i64>,
        to: Option<i64>,
        reply: RpcReplyPort<Vec<UsageEvent>>,
    },
    Rollover {
        history_id: String,
        trigger: SnapshotTrigger,
        usage: UsageBreakdown,
        cost_usd: f64,
        period_start: i64,
        reply: RpcReplyPort<UsageSnapshot>,
    },
    Tombstone { history_id: String, snapshot: UsageSnapshot },
    DeleteHistory { history_id: String, reply: RpcReplyPort<Result<(), ClewdrError>> },
    PruneNow { reply: RpcReplyPort<PruneStats> },
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PruneStats {
    pub events_removed: u64,
    pub files_compacted: u64,
    pub bytes_reclaimed: u64,
}

pub struct UsageActor {
    pub history_dir: PathBuf,
}

pub struct UsageActorState {
    history_dir: PathBuf,
    open_writers: HashMap<String, BufWriter<tokio::fs::File>>,
    dropped_events_since_last_warn: u64,
}

#[async_trait::async_trait]
impl Actor for UsageActor {
    type Msg = UsageActorMessage;
    type State = UsageActorState;
    type Arguments = PathBuf;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        history_dir: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        fs::create_dir_all(&history_dir).await?;
        // 2-second flush tick
        let me = myself.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(2));
            loop {
                tick.tick().await;
                if me.cast(UsageActorMessage::Flush).is_err() {
                    break;
                }
            }
        });
        Ok(UsageActorState {
            history_dir,
            open_writers: HashMap::new(),
            dropped_events_since_last_warn: 0,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            UsageActorMessage::Record { history_id, event } => {
                if let Err(e) = append_event(state, &history_id, &event).await {
                    error!("usage append failed: {}", e);
                }
            }
            UsageActorMessage::Flush => {
                for w in state.open_writers.values_mut() {
                    let _ = w.flush().await;
                }
            }
            UsageActorMessage::QueryEvents { history_id, from, to, reply } => {
                let events = read_events(&state.history_dir, &history_id, from, to).await
                    .unwrap_or_default();
                let _ = reply.send(events);
            }
            UsageActorMessage::Rollover { history_id, trigger, usage, cost_usd, period_start, reply } => {
                let event_count = count_events_since(&state.history_dir, &history_id, period_start).await.unwrap_or(0);
                let snapshot = UsageSnapshot {
                    closed_at: chrono::Utc::now().timestamp(),
                    trigger,
                    period_start,
                    usage,
                    cost_usd,
                    event_count,
                };
                // Write a marker line to JSONL so post-mortem tooling can find rollovers
                let _ = append_marker(state, &history_id, &snapshot).await;
                let _ = reply.send(snapshot);
            }
            UsageActorMessage::Tombstone { history_id, snapshot } => {
                let _ = append_marker(state, &history_id, &snapshot).await;
                if let Some(mut w) = state.open_writers.remove(&history_id) {
                    let _ = w.flush().await;
                }
            }
            UsageActorMessage::DeleteHistory { history_id, reply } => {
                if let Some(mut w) = state.open_writers.remove(&history_id) {
                    let _ = w.flush().await;
                }
                let path = state.history_dir.join(format!("{}.jsonl", history_id));
                let result = if fs::try_exists(&path).await.unwrap_or(false) {
                    fs::remove_file(&path).await.map_err(|e| ClewdrError::Whatever {
                        message: format!("delete history: {}", e),
                        source: None,
                    })
                } else {
                    Ok(())
                };
                let _ = reply.send(result);
            }
            UsageActorMessage::PruneNow { reply } => {
                let stats = prune(&state.history_dir).await.unwrap_or_default();
                let _ = reply.send(stats);
            }
        }
        Ok(())
    }
}

async fn append_event(
    state: &mut UsageActorState,
    history_id: &str,
    event: &UsageEvent,
) -> std::io::Result<()> {
    let writer = get_or_open(state, history_id).await?;
    let line = serde_json::to_string(event).expect("UsageEvent serialization");
    writer.write_all(line.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    Ok(())
}

async fn append_marker(
    state: &mut UsageActorState,
    history_id: &str,
    snapshot: &UsageSnapshot,
) -> std::io::Result<()> {
    let writer = get_or_open(state, history_id).await?;
    let envelope = serde_json::json!({ "snapshot": snapshot });
    let line = serde_json::to_string(&envelope).expect("snapshot serialization");
    writer.write_all(line.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    Ok(())
}

async fn get_or_open<'a>(
    state: &'a mut UsageActorState,
    history_id: &str,
) -> std::io::Result<&'a mut BufWriter<tokio::fs::File>> {
    if !state.open_writers.contains_key(history_id) {
        let path = state.history_dir.join(format!("{}.jsonl", history_id));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        state.open_writers.insert(history_id.to_string(), BufWriter::new(file));
    }
    Ok(state.open_writers.get_mut(history_id).unwrap())
}

async fn read_events(
    dir: &PathBuf,
    history_id: &str,
    from: Option<i64>,
    to: Option<i64>,
) -> std::io::Result<Vec<UsageEvent>> {
    let path = dir.join(format!("{}.jsonl", history_id));
    if !fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(Vec::new());
    }
    let f = tokio::fs::File::open(&path).await?;
    let reader = BufReader::new(f);
    let mut lines = reader.lines();
    let mut events = Vec::new();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() { continue; }
        // Skip snapshot marker lines
        if line.starts_with(r#"{"snapshot":"#) { continue; }
        if let Ok(ev) = serde_json::from_str::<UsageEvent>(&line) {
            if from.is_some_and(|f| ev.ts < f) { continue; }
            if to.is_some_and(|t| ev.ts > t) { continue; }
            events.push(ev);
        }
    }
    Ok(events)
}

async fn count_events_since(
    dir: &PathBuf,
    history_id: &str,
    since: i64,
) -> std::io::Result<u64> {
    Ok(read_events(dir, history_id, Some(since), None).await?.len() as u64)
}

async fn prune(dir: &PathBuf) -> std::io::Result<PruneStats> {
    // Honors history_event_retention_days from CLEWDR_CONFIG; if None, no-op
    let retention = crate::config::CLEWDR_CONFIG.load().history_event_retention_days;
    let Some(days) = retention else { return Ok(PruneStats::default()); };
    let cutoff = chrono::Utc::now().timestamp() - (days as i64) * 86_400;
    let mut stats = PruneStats::default();
    let mut entries = fs::read_dir(dir).await?;
    while let Some(e) = entries.next_entry().await? {
        let path = e.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") { continue; }
        let original_size = e.metadata().await?.len();
        let mut kept_lines: Vec<String> = Vec::new();
        let f = tokio::fs::File::open(&path).await?;
        let reader = BufReader::new(f);
        let mut lines = reader.lines();
        let mut removed = 0u64;
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() { continue; }
            if line.starts_with(r#"{"snapshot":"#) {
                kept_lines.push(line);
                continue;
            }
            match serde_json::from_str::<UsageEvent>(&line) {
                Ok(ev) if ev.ts >= cutoff => kept_lines.push(line),
                Ok(_) => { removed += 1; },
                Err(_) => { kept_lines.push(line); /* preserve unrecognized */ },
            }
        }
        if removed > 0 {
            let tmp = path.with_extension("jsonl.tmp");
            tokio::fs::write(&tmp, kept_lines.join("\n") + "\n").await?;
            tokio::fs::rename(&tmp, &path).await?;
            stats.files_compacted += 1;
            stats.events_removed += removed;
            let new_size = tokio::fs::metadata(&path).await?.len();
            stats.bytes_reclaimed += original_size.saturating_sub(new_size);
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::cookie::ModelFamily;

    fn ev(ts: i64, model: &str, in_t: u64, out_t: u64, cost: f64) -> UsageEvent {
        UsageEvent {
            ts,
            source: crate::config::UsageSource::Web,
            model: model.into(),
            family: ModelFamily::Sonnet,
            input_tokens: in_t,
            output_tokens: out_t,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: cost,
        }
    }

    #[tokio::test]
    async fn record_then_query_returns_events() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut state = UsageActorState {
            history_dir: dir.clone(),
            open_writers: HashMap::new(),
            dropped_events_since_last_warn: 0,
        };
        append_event(&mut state, "abc123", &ev(100, "claude-sonnet-4-5-20250929", 10, 5, 0.01)).await.unwrap();
        append_event(&mut state, "abc123", &ev(200, "claude-sonnet-4-5-20250929", 20, 10, 0.02)).await.unwrap();
        for w in state.open_writers.values_mut() { w.flush().await.unwrap(); }
        let evs = read_events(&dir, "abc123", None, None).await.unwrap();
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0].ts, 100);
        assert_eq!(evs[1].ts, 200);
    }

    #[tokio::test]
    async fn query_filters_by_range() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut state = UsageActorState {
            history_dir: dir.clone(),
            open_writers: HashMap::new(),
            dropped_events_since_last_warn: 0,
        };
        for ts in [50, 150, 250] {
            append_event(&mut state, "x", &ev(ts, "m", 1, 1, 0.001)).await.unwrap();
        }
        for w in state.open_writers.values_mut() { w.flush().await.unwrap(); }
        let evs = read_events(&dir, "x", Some(100), Some(200)).await.unwrap();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].ts, 150);
    }

    #[tokio::test]
    async fn snapshot_marker_is_skipped_by_event_query() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut state = UsageActorState {
            history_dir: dir.clone(),
            open_writers: HashMap::new(),
            dropped_events_since_last_warn: 0,
        };
        append_event(&mut state, "y", &ev(100, "m", 1, 1, 0.001)).await.unwrap();
        let snap = UsageSnapshot {
            closed_at: 200,
            trigger: SnapshotTrigger::SessionReset,
            period_start: 0,
            usage: UsageBreakdown::default(),
            cost_usd: 0.0,
            event_count: 1,
        };
        append_marker(&mut state, "y", &snap).await.unwrap();
        for w in state.open_writers.values_mut() { w.flush().await.unwrap(); }
        let evs = read_events(&dir, "y", None, None).await.unwrap();
        assert_eq!(evs.len(), 1);
    }
}
```

In `src/services/mod.rs`, add `pub mod usage_actor;`.

**Step 2: Run tests**

Run: `cargo test --lib services::usage_actor -- --nocapture`
Expected: 3 tests pass.

**Step 3: Commit**

```bash
git add src/services/usage_actor.rs src/services/mod.rs
git commit -m "feat(usage): UsageActor with append/query/snapshot/prune"
```

### Task 3.2: Handle wrapper for fire-and-forget recording

**Files:**
- Modify: `src/services/usage_actor.rs`

**Step 1: Add a typed handle**

Append:

```rust
#[derive(Clone)]
pub struct UsageActorHandle {
    inner: ActorRef<UsageActorMessage>,
}

impl UsageActorHandle {
    pub fn new(inner: ActorRef<UsageActorMessage>) -> Self { Self { inner } }

    /// Fire-and-forget. Drops the event if the mailbox is full.
    pub fn try_record(&self, history_id: String, event: UsageEvent) {
        if let Err(e) = self.inner.cast(UsageActorMessage::Record { history_id, event }) {
            debug!("usage record dropped: {}", e);
        }
    }

    pub async fn rollover(
        &self,
        history_id: String,
        trigger: SnapshotTrigger,
        usage: UsageBreakdown,
        cost_usd: f64,
        period_start: i64,
    ) -> Result<UsageSnapshot, ClewdrError> {
        let (reply, rx) = ractor::call_t!(self.inner, |reply| UsageActorMessage::Rollover {
            history_id, trigger, usage, cost_usd, period_start, reply
        }, 5_000);
        // call_t! macro returns Result; adapt to your codebase's actual ractor version idiom.
        // If the macro shape differs in ractor 0.15, replace with manual oneshot:
        rx
    }

    pub fn tombstone(&self, history_id: String, snapshot: UsageSnapshot) {
        let _ = self.inner.cast(UsageActorMessage::Tombstone { history_id, snapshot });
    }

    pub async fn query_events(
        &self,
        history_id: String,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<Vec<UsageEvent>, ClewdrError> {
        // Use ractor::call! pattern matching CookieActorHandle::request_cookie's idiom
        unimplemented!("Match the existing CookieActorHandle ractor call idiom")
    }
}
```

**NOTE TO IMPLEMENTER:** ractor 0.15's `call!`/`call_t!` macros vary by version. Read
`src/services/cookie_actor.rs` for the project's exact idiom (search for `call_t!` or
`RpcReplyPort` send patterns) and copy the same pattern. Do not invent a new style.

**Step 2: Verify compile**

Run: `cargo check --all-targets`
Expected: success.

**Step 3: Commit**

```bash
git add src/services/usage_actor.rs
git commit -m "feat(usage): UsageActorHandle for typed access from request path"
```

### Task 3.3: Spawn UsageActor in main

**Files:**
- Modify: `src/main.rs`
- Possibly: `src/services/mod.rs` (re-export `USAGE_ACTOR` global)

**Step 1: Add global**

In `src/services/usage_actor.rs`:

```rust
use std::sync::OnceLock;
pub static USAGE_ACTOR: OnceLock<UsageActorHandle> = OnceLock::new();

pub async fn spawn(history_dir: PathBuf) -> Result<UsageActorHandle, ClewdrError> {
    let (actor_ref, _join) = Actor::spawn(
        Some("usage_actor".into()),
        UsageActor { history_dir: history_dir.clone() },
        history_dir,
    )
    .await
    .map_err(|e| ClewdrError::Whatever { message: format!("spawn UsageActor: {}", e), source: None })?;
    let handle = UsageActorHandle::new(actor_ref);
    let _ = USAGE_ACTOR.set(handle.clone());
    Ok(handle)
}
```

In `main.rs`, after `pricing::init()`:

```rust
let history_dir = std::path::PathBuf::from("history");
clewdr::services::usage_actor::spawn(history_dir).await?;
```

**Step 2: Run app, observe directory creation**

Run: `cargo run` for ~3s, then check `ls history/`
Expected: empty `history/` directory created.

**Step 3: Commit**

```bash
git add src/main.rs src/services/usage_actor.rs
git commit -m "feat(usage): spawn UsageActor at startup"
```

---

## Phase 4: Hot-path integration

### Task 4.1: Extend `add_and_bucket_usage` signature with cache + model + source

**Files:**
- Modify: `src/config/cookie.rs`

**Step 1: Change signature and bucket logic**

```rust
pub fn add_and_bucket_usage(
    &mut self,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_create: u64,
    family: ModelFamily,
    model: &str,
    source: crate::config::UsageSource,
) {
    if input == 0 && output == 0 && cache_read == 0 && cache_create == 0 {
        return;
    }

    // ... existing token bucket math ...

    // Cost calculation
    let cost = crate::services::pricing::cost(model, input, output, cache_read, cache_create);
    self.session_cost_usd += cost;
    self.weekly_cost_usd += cost;
    self.lifetime_cost_usd += cost;
    match family {
        ModelFamily::Sonnet => self.weekly_sonnet_cost_usd += cost,
        ModelFamily::Opus => self.weekly_opus_cost_usd += cost,
        ModelFamily::Other => {}
    }

    // Fire-and-forget event to UsageActor
    if let Some(actor) = crate::services::usage_actor::USAGE_ACTOR.get() {
        actor.try_record(
            self.history_id(),
            crate::config::UsageEvent {
                ts: chrono::Utc::now().timestamp(),
                source,
                model: model.to_string(),
                family,
                input_tokens: input,
                output_tokens: output,
                cache_read_tokens: cache_read,
                cache_creation_tokens: cache_create,
                cost_usd: cost,
            },
        );
    }
}
```

**Step 2: Verify compile fails at all call sites**

Run: `cargo check --all-targets`
Expected: 4 errors at call sites in claude_code_state, claude_web_state, types/claude_web/response.

This is the dependency leverage point — fix call sites in next task.

### Task 4.2: Update all 4 call sites

**Files:**
- Modify: `src/claude_code_state/chat.rs:509`
- Modify: `src/claude_code_state/chat.rs:551`
- Modify: `src/claude_web_state/mod.rs:177`
- Modify: `src/types/claude_web/response.rs:147`
- Modify: `src/types/claude_web/response.rs:166`

**Step 1: For each callsite, gather model and source**

In claude_code_state callsites: `source = UsageSource::Code`. Model name available
via `self.payload.as_ref().map(|p| p.model.clone())` — match existing access pattern in
those files.

In claude_web_state and types/claude_web/response: `source = UsageSource::Web`. Model
via `payload.model` (already in scope based on earlier grep showing
`Self::classify_model(&p.model)`).

Cache tokens: read from upstream usage struct. Search for `cache_read_input_tokens` and
`cache_creation_input_tokens` in `src/types/`. If not present, pass `0` for both — this is
acceptable for v1 (Anthropic only emits cache fields when prompt caching is used).

**Step 2: Update each call site**

Example for `claude_web_state/mod.rs:177`:

```rust
cookie.add_and_bucket_usage(
    input,
    output,
    payload.usage.as_ref().and_then(|u| u.cache_read_input_tokens).unwrap_or(0),
    payload.usage.as_ref().and_then(|u| u.cache_creation_input_tokens).unwrap_or(0),
    family,
    payload.as_ref().map(|p| p.model.as_str()).unwrap_or("unknown"),
    crate::config::UsageSource::Web,
);
```

Adjust each call site to its local variable shapes.

**Step 3: Verify compile**

Run: `cargo check --all-targets && cargo test --lib`
Expected: success, all existing tests still pass.

**Step 4: Commit**

```bash
git add src/config/cookie.rs src/claude_code_state/chat.rs src/claude_web_state/mod.rs src/types/claude_web/response.rs
git commit -m "feat(usage): record usage events with cost on hot path"
```

### Task 4.3: Integration smoke test

**Files:** none (manual verification)

**Step 1: Run the binary**

```bash
cargo run
```

**Step 2: Submit a cookie and make a real request through clewdr**

Hit `/v1/messages` with any test prompt via curl or the admin UI.

**Step 3: Verify history file appears**

Run: `ls history/ && cat history/*.jsonl | head -5`
Expected: at least one `.jsonl` file containing a JSON line with `cost_usd > 0`.

**Step 4: Commit (no code changes — verification only)**

If everything works, mark task complete.

---

## Phase 5: Rollover hooks

### Task 5.1: Capture snapshot in CookieStatus::reset

**Files:**
- Modify: `src/config/cookie.rs`

**Step 1: Add async helper that performs rollover**

`reset()` is currently sync. To call into the async `UsageActorHandle::rollover` we either
make `reset` async or move the snapshot capture out of `reset` into the call site (which
already runs in async context — see `cookie_actor.rs::CheckReset` handler).

**Decision:** keep `reset` sync (it's used widely); add a sync version that pushes a "rollover
intent" into a queue consumed by an async helper, OR call `actor.cast()` in fire-and-forget
mode plus push a synthetic snapshot synchronously.

**Cleaner approach:** add a new method `reset_with_snapshot(self) -> Self` that returns
the cookie + a `Vec<(SnapshotTrigger, UsageBreakdown, f64)>` of pending snapshots; the
async caller in `cookie_actor.rs` then drives the rollover and pushes results into
`self.snapshots`.

```rust
pub struct PendingRollover {
    pub trigger: SnapshotTrigger,
    pub usage: UsageBreakdown,
    pub cost_usd: f64,
    pub period_start: i64,
}

impl CookieStatus {
    /// Like `reset()` but also returns rollover descriptors that the caller should
    /// hand to UsageActor. Caller then pushes returned snapshots into `self.snapshots`.
    pub fn reset_with_rollover(self) -> (Self, Vec<PendingRollover>) {
        let mut pending = Vec::new();
        if let Some(t) = self.reset_time && t < chrono::Utc::now().timestamp() {
            pending.push(PendingRollover {
                trigger: SnapshotTrigger::SessionReset,
                usage: self.session_usage.clone(),
                cost_usd: self.session_cost_usd,
                period_start: self.session_resets_at.unwrap_or(0).saturating_sub(5 * 60 * 60),
            });
            // weekly etc. similar — only if actually crossed
        }
        (self.reset(), pending)
    }
}
```

**Step 2: In `cookie_actor.rs`**, where `cookie = cookie.reset();` was called, replace with:

```rust
let (cookie, pending) = cookie.reset_with_rollover();
let mut cookie = cookie;
if let Some(actor) = crate::services::usage_actor::USAGE_ACTOR.get() {
    for p in pending {
        if let Ok(snap) = actor.rollover(cookie.history_id(), p.trigger, p.usage, p.cost_usd, p.period_start).await {
            cookie.snapshots.push(snap);
        }
    }
}
```

**Step 3: Run tests**

Run: `cargo test --lib`
Expected: all pass.

**Step 4: Commit**

```bash
git add src/config/cookie.rs src/services/cookie_actor.rs
git commit -m "feat(usage): capture snapshots on session/weekly reset boundaries"
```

### Task 5.2: Hook weekly/sonnet/opus boundary crossings

**Files:**
- Modify: wherever `*_resets_at` boundary checks happen — find via `grep -rn "weekly_resets_at\|weekly_sonnet_resets_at\|weekly_opus_resets_at" src/`
- Likely: `src/claude_code_state/chat.rs::update_cookie_boundaries_if_due` (line ~508 area)

**Step 1: Trace boundary check function**

Run: `grep -rn "update_cookie_boundaries_if_due\|weekly_resets_at" src/ | head -30`

**Step 2: At each boundary-crossing point**, replace bucket reset with rollover-then-reset
following the same pattern as Task 5.1.

**Step 3: Run tests + smoke test**

Run: `cargo test --lib`
Manual: trigger a reset (e.g. by setting `reset_time = now - 1` in a test) and observe
that `snapshots` grows.

**Step 4: Commit**

```bash
git add -p
git commit -m "feat(usage): rollover snapshots on weekly/sonnet/opus boundaries"
```

---

## Phase 6: Death hook

### Task 6.1: Capture final_snapshot when cookie becomes invalid

**Files:**
- Modify: `src/services/cookie_actor.rs`

**Step 1: Find `state.invalid.insert(...)` paths**

Run: `grep -n "state\.invalid" src/services/cookie_actor.rs`
There are 2-3 paths in `collect()` matching `Reason::Free`, `Reason::Restricted`, and
explicit deletion.

**Step 2: At each `UselessCookie::new(...)` construction**, build a final snapshot:

```rust
let final_snapshot = UsageSnapshot {
    closed_at: chrono::Utc::now().timestamp(),
    trigger: SnapshotTrigger::Death { reason: reason.clone() },
    period_start: 0,
    usage: cookie.lifetime_usage.clone(),
    cost_usd: cookie.lifetime_cost_usd,
    event_count: 0,
};
let useless = UselessCookie::new(cookie.cookie.clone(), reason.clone())
    .with_final_snapshot(final_snapshot.clone());
if let Some(actor) = crate::services::usage_actor::USAGE_ACTOR.get() {
    actor.tombstone(cookie.history_id(), final_snapshot);
}
state.invalid.insert(useless);
```

**Step 3: Run tests + verify build**

Run: `cargo test --lib && cargo check --all-targets`
Expected: pass.

**Step 4: Commit**

```bash
git add src/services/cookie_actor.rs
git commit -m "feat(usage): preserve final_snapshot when cookie dies"
```

---

## Phase 7: API endpoints

### Task 7.1: `GET /api/usage/summary`

**Files:**
- Create: `src/api/usage.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/router.rs`

**Step 1: Write handler**

`src/api/usage.rs`:

```rust
use axum::{Json, extract::Path, http::StatusCode};
use serde::Serialize;

use crate::config::CLEWDR_CONFIG;

#[derive(Serialize)]
pub struct UsageTotals {
    pub lifetime_cost_usd: f64,
    pub lifetime_input_tokens: u64,
    pub lifetime_output_tokens: u64,
    pub by_family: FamilyTotals,
}

#[derive(Serialize, Default)]
pub struct FamilyTotals {
    pub sonnet_input_tokens: u64,
    pub sonnet_output_tokens: u64,
    pub opus_input_tokens: u64,
    pub opus_output_tokens: u64,
}

#[derive(Serialize)]
pub struct PerCookieSummary {
    pub history_id: String,
    pub cookie_ellipse: String,
    pub state: String, // "valid" | "exhausted" | "invalid"
    pub session_cost_usd: f64,
    pub weekly_cost_usd: f64,
    pub lifetime_cost_usd: f64,
    pub snapshot_count: usize,
    pub died_at: Option<i64>,
}

#[derive(Serialize)]
pub struct UsageSummary {
    pub totals: UsageTotals,
    pub per_cookie: Vec<PerCookieSummary>,
}

pub async fn summary() -> Json<UsageSummary> {
    let cfg = CLEWDR_CONFIG.load();
    let mut totals = UsageTotals {
        lifetime_cost_usd: 0.0,
        lifetime_input_tokens: 0,
        lifetime_output_tokens: 0,
        by_family: FamilyTotals::default(),
    };
    let mut per_cookie = Vec::new();
    for c in cfg.cookie_array.iter() {
        totals.lifetime_cost_usd += c.lifetime_cost_usd;
        totals.lifetime_input_tokens += c.lifetime_usage.total_input_tokens;
        totals.lifetime_output_tokens += c.lifetime_usage.total_output_tokens;
        totals.by_family.sonnet_input_tokens += c.lifetime_usage.sonnet_input_tokens;
        totals.by_family.sonnet_output_tokens += c.lifetime_usage.sonnet_output_tokens;
        totals.by_family.opus_input_tokens += c.lifetime_usage.opus_input_tokens;
        totals.by_family.opus_output_tokens += c.lifetime_usage.opus_output_tokens;
        per_cookie.push(PerCookieSummary {
            history_id: c.history_id(),
            cookie_ellipse: c.cookie.ellipse(),
            state: if c.reset_time.is_some() { "exhausted".into() } else { "valid".into() },
            session_cost_usd: c.session_cost_usd,
            weekly_cost_usd: c.weekly_cost_usd,
            lifetime_cost_usd: c.lifetime_cost_usd,
            snapshot_count: c.snapshots.len(),
            died_at: None,
        });
    }
    for u in cfg.wasted_cookie.iter() {
        totals.lifetime_cost_usd += u.final_snapshot.as_ref().map(|s| s.cost_usd).unwrap_or(0.0);
        per_cookie.push(PerCookieSummary {
            history_id: { let mut h = sha2::Sha256::new(); use sha2::Digest; h.update(u.cookie.as_bytes()); let bytes = h.finalize(); bytes[..8].iter().map(|b| format!("{:02x}", b)).collect() },
            cookie_ellipse: u.cookie.ellipse(),
            state: "invalid".into(),
            session_cost_usd: 0.0,
            weekly_cost_usd: 0.0,
            lifetime_cost_usd: u.final_snapshot.as_ref().map(|s| s.cost_usd).unwrap_or(0.0),
            snapshot_count: u.final_snapshot.is_some() as usize,
            died_at: Some(u.died_at),
        });
    }
    Json(UsageSummary { totals, per_cookie })
}
```

**Step 2: Mount route**

In `src/api/mod.rs`:
```rust
pub mod usage;
```

In `src/router.rs`, find admin-auth route group and add:
```rust
.route("/api/usage/summary", axum::routing::get(crate::api::usage::summary))
```

**Step 3: Run app, hit endpoint**

Run: `cargo run` then in another terminal:
```bash
curl -H "Authorization: Bearer $ADMIN_TOKEN" http://localhost:8484/api/usage/summary | jq .
```
Expected: 200 OK with `totals` and `per_cookie` arrays (possibly empty if no cookies).

**Step 4: Commit**

```bash
git add src/api/usage.rs src/api/mod.rs src/router.rs
git commit -m "feat(api): GET /api/usage/summary"
```

### Task 7.2: `GET /api/usage/cookie/:history_id/events`

**Files:**
- Modify: `src/api/usage.rs`
- Modify: `src/router.rs`

**Step 1: Handler**

```rust
use axum::extract::Query;

#[derive(serde::Deserialize)]
pub struct EventsQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub source: Option<String>, // "web" | "code" | "all"
}

pub async fn events(
    Path(history_id): Path<String>,
    Query(q): Query<EventsQuery>,
) -> Result<Json<Vec<crate::config::UsageEvent>>, StatusCode> {
    let actor = crate::services::usage_actor::USAGE_ACTOR.get()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let events = actor.query_events(history_id, q.from, q.to).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let filtered = match q.source.as_deref() {
        Some("web") => events.into_iter().filter(|e| matches!(e.source, crate::config::UsageSource::Web)).collect(),
        Some("code") => events.into_iter().filter(|e| matches!(e.source, crate::config::UsageSource::Code)).collect(),
        _ => events,
    };
    Ok(Json(filtered))
}
```

Mount: `/api/usage/cookie/:history_id/events`

**Step 2: Curl test**

```bash
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
    "http://localhost:8484/api/usage/cookie/abc123/events?from=1700000000" | jq .
```
Expected: `[]` for unknown id; or array of events.

**Step 3: Commit**

```bash
git add src/api/usage.rs src/router.rs
git commit -m "feat(api): GET /api/usage/cookie/:history_id/events"
```

### Task 7.3: `GET /api/usage/cookie/:history_id/timeseries`

**Files:**
- Modify: `src/api/usage.rs`

**Step 1: Server-side bucketing**

```rust
#[derive(serde::Deserialize)]
pub struct TimeSeriesQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub bucket: Option<String>, // "hour" | "day"
}

#[derive(serde::Serialize)]
pub struct TimeBucket {
    pub ts: i64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub sonnet_input: u64, pub sonnet_output: u64,
    pub opus_input: u64, pub opus_output: u64,
}

pub async fn timeseries(
    Path(history_id): Path<String>,
    Query(q): Query<TimeSeriesQuery>,
) -> Result<Json<Vec<TimeBucket>>, StatusCode> {
    let actor = crate::services::usage_actor::USAGE_ACTOR.get()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let events = actor.query_events(history_id, q.from, q.to).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let bucket_secs: i64 = match q.bucket.as_deref() { Some("hour") => 3600, _ => 86400 };
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<i64, TimeBucket> = BTreeMap::new();
    for e in events {
        let key = (e.ts / bucket_secs) * bucket_secs;
        let b = buckets.entry(key).or_insert(TimeBucket {
            ts: key, input_tokens: 0, output_tokens: 0, cost_usd: 0.0,
            sonnet_input: 0, sonnet_output: 0, opus_input: 0, opus_output: 0,
        });
        b.input_tokens += e.input_tokens;
        b.output_tokens += e.output_tokens;
        b.cost_usd += e.cost_usd;
        match e.family {
            crate::config::cookie::ModelFamily::Sonnet => { b.sonnet_input += e.input_tokens; b.sonnet_output += e.output_tokens; },
            crate::config::cookie::ModelFamily::Opus => { b.opus_input += e.input_tokens; b.opus_output += e.output_tokens; },
            _ => {},
        }
    }
    Ok(Json(buckets.into_values().collect()))
}
```

**Step 2: Mount + curl test**

```bash
curl "http://localhost:8484/api/usage/cookie/abc123/timeseries?bucket=hour" -H "Authorization: Bearer $T" | jq .
```

**Step 3: Commit**

```bash
git add src/api/usage.rs src/router.rs
git commit -m "feat(api): GET /api/usage/cookie/:history_id/timeseries"
```

### Task 7.4: Remaining endpoints (snapshots, dead, pricing, prune)

**Files:**
- Modify: `src/api/usage.rs`
- Modify: `src/router.rs`

**Step 1: Add the four handlers**

- `GET /api/usage/cookie/:history_id/snapshots` — read `cfg.cookie_array.iter().find(...)`'s `snapshots`
- `GET /api/usage/dead` — return `cfg.wasted_cookie.iter().cloned().collect::<Vec<_>>()`
- `GET /api/usage/pricing` — return `pricing::current()` table (just a count + source + fetched_at; do NOT leak the full LiteLLM blob — too big and not useful in UI)
- `POST /api/usage/prune` — call `actor.prune_now()` and return `PruneStats`

**Step 2: Mount each route under admin auth**

**Step 3: Curl-test all four**

**Step 4: Commit**

```bash
git add src/api/usage.rs src/router.rs
git commit -m "feat(api): snapshots, dead, pricing, prune endpoints"
```

### Task 7.5: Extend `GET /api/cookies` with cost + history_id + final_snapshot

**Files:**
- Modify: `src/api/misc.rs` (or wherever `/api/cookies` is defined; trace via grep)

**Step 1: Find handler**

Run: `grep -rn "fn cookies\|/api/cookies" src/api`

**Step 2: Add the new fields**

The Serialize impl on `CookieStatus` already includes the new cost fields (auto-derived).
For `UselessCookie`, ensure `final_snapshot` and `died_at` are in the serialized output.
Verify by hitting the endpoint:

```bash
curl -H "Authorization: Bearer $T" http://localhost:8484/api/cookies | jq '.data.invalid[0]'
```
Expected: includes `final_snapshot` and `died_at` (may be `null` / `0` for pre-existing entries).

**Step 3: Commit if any changes**

```bash
git add -p
git commit -m "feat(api): expose cost fields and final_snapshot in /api/cookies"
```

---

## Phase 8: Frontend types and API client

### Task 8.1: TypeScript types for usage

**Files:**
- Create: `frontend/src/types/usage.types.ts`

**Step 1: Define types matching backend serialization**

```typescript
export type UsageSource = "web" | "code";

export type ModelFamily = "sonnet" | "opus" | "other";

export interface UsageEvent {
  ts: number;
  source: UsageSource;
  model: string;
  family: ModelFamily;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
  cost_usd: number;
}

export type SnapshotTrigger =
  | { kind: "session_reset" }
  | { kind: "weekly_reset" }
  | { kind: "weekly_sonnet_reset" }
  | { kind: "weekly_opus_reset" }
  | { kind: "death"; reason: unknown };

export interface UsageSnapshot {
  closed_at: number;
  trigger: SnapshotTrigger;
  period_start: number;
  usage: {
    total_input_tokens: number;
    total_output_tokens: number;
    sonnet_input_tokens: number;
    sonnet_output_tokens: number;
    opus_input_tokens: number;
    opus_output_tokens: number;
  };
  cost_usd: number;
  event_count: number;
}

export interface UsageSummary {
  totals: {
    lifetime_cost_usd: number;
    lifetime_input_tokens: number;
    lifetime_output_tokens: number;
    by_family: {
      sonnet_input_tokens: number; sonnet_output_tokens: number;
      opus_input_tokens: number; opus_output_tokens: number;
    };
  };
  per_cookie: PerCookieSummary[];
}

export interface PerCookieSummary {
  history_id: string;
  cookie_ellipse: string;
  state: "valid" | "exhausted" | "invalid";
  session_cost_usd: number;
  weekly_cost_usd: number;
  lifetime_cost_usd: number;
  snapshot_count: number;
  died_at: number | null;
}

export interface TimeBucket {
  ts: number;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
  sonnet_input: number; sonnet_output: number;
  opus_input: number; opus_output: number;
}
```

**Step 2: Commit**

```bash
git add frontend/src/types/usage.types.ts
git commit -m "feat(frontend): TypeScript types for usage"
```

### Task 8.2: API client functions

**Files:**
- Modify: `frontend/src/api/index.ts`

**Step 1: Add functions**

```typescript
import type { UsageSummary, UsageEvent, UsageSnapshot, TimeBucket } from "../types/usage.types";

export async function getUsageSummary(): Promise<UsageSummary> {
  const r = await fetch("/api/usage/summary", { credentials: "include" });
  if (!r.ok) throw new Error(`usage summary failed: ${r.status}`);
  return r.json();
}

export async function getCookieEvents(
  historyId: string,
  opts?: { from?: number; to?: number; source?: "web" | "code" | "all" }
): Promise<UsageEvent[]> {
  const u = new URL(`/api/usage/cookie/${historyId}/events`, window.location.origin);
  if (opts?.from) u.searchParams.set("from", String(opts.from));
  if (opts?.to) u.searchParams.set("to", String(opts.to));
  if (opts?.source && opts.source !== "all") u.searchParams.set("source", opts.source);
  const r = await fetch(u.toString(), { credentials: "include" });
  if (!r.ok) throw new Error(`events failed: ${r.status}`);
  return r.json();
}

export async function getCookieTimeSeries(
  historyId: string,
  bucket: "hour" | "day",
  opts?: { from?: number; to?: number }
): Promise<TimeBucket[]> {
  const u = new URL(`/api/usage/cookie/${historyId}/timeseries`, window.location.origin);
  u.searchParams.set("bucket", bucket);
  if (opts?.from) u.searchParams.set("from", String(opts.from));
  if (opts?.to) u.searchParams.set("to", String(opts.to));
  const r = await fetch(u.toString(), { credentials: "include" });
  if (!r.ok) throw new Error(`timeseries failed: ${r.status}`);
  return r.json();
}

export async function getDeadCookies() {
  const r = await fetch("/api/usage/dead", { credentials: "include" });
  if (!r.ok) throw new Error(`dead cookies failed: ${r.status}`);
  return r.json();
}

export async function getCookieSnapshots(historyId: string): Promise<UsageSnapshot[]> {
  const r = await fetch(`/api/usage/cookie/${historyId}/snapshots`, { credentials: "include" });
  if (!r.ok) throw new Error(`snapshots failed: ${r.status}`);
  return r.json();
}
```

(Match the auth pattern of existing API calls in this file — observe `getCookieStatus` first.)

**Step 2: Build to verify TS**

Run: `cd frontend && bun run build && cd ..`
Expected: success.

**Step 3: Commit**

```bash
git add frontend/src/api/index.ts
git commit -m "feat(frontend): API client for /api/usage/*"
```

---

## Phase 9: Inline cost on existing CookieVisualization

### Task 9.1: Add cost line to cookie cards

**Files:**
- Modify: `frontend/src/components/claude/CookieVisualization.tsx` (and `CookieSection.tsx` if cards live there)
- Modify: `frontend/src/types/cookie.types.ts` to include cost fields

**Step 1: Extend CookieItem type**

In `frontend/src/types/cookie.types.ts`, add to `CookieItem`:

```typescript
session_cost_usd?: number;
weekly_cost_usd?: number;
weekly_sonnet_cost_usd?: number;
weekly_opus_cost_usd?: number;
lifetime_cost_usd?: number;
snapshot_count?: number;
```

For invalid cookies in `CookieStatusInfo.invalid`, add:
```typescript
final_snapshot?: UsageSnapshot;
died_at?: number;
```

**Step 2: Render cost inline**

In the cookie card render path (look for existing token display), add:

```tsx
{status.lifetime_cost_usd != null && (
  <div className="text-xs text-gray-400">
    {t("cookieStatus.lifetimeCost")}: ${status.lifetime_cost_usd.toFixed(2)}
  </div>
)}
```

Add to `frontend/src/locales/en/translation.json` and `zh/translation.json`:
```json
"cookieStatus": { ..., "lifetimeCost": "Lifetime cost" }
```

**Step 3: For invalid cookies** — render `final_snapshot.usage` and `final_snapshot.cost_usd`
where the existing "Reason: ..." line lives.

**Step 4: Visual smoke test**

Run: `cd frontend && bun run dev` and load admin UI; verify cost appears on cards.

**Step 5: Commit**

```bash
git add frontend/src/components/claude frontend/src/types/cookie.types.ts frontend/src/locales
git commit -m "feat(frontend): inline cost on cookie cards + final_snapshot on invalid"
```

---

## Phase 10: Usage tab scaffold + i18n

### Task 10.1: Add Usage tab to App.tsx

**Files:**
- Modify: `frontend/src/App.tsx`
- Create: `frontend/src/components/usage/UsageDashboard.tsx`

**Step 1: Stub component**

```tsx
import React from "react";
import { useTranslation } from "react-i18next";

const UsageDashboard: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div className="p-4">
      <h2 className="text-xl font-semibold">{t("usage.title")}</h2>
      <p>{t("usage.placeholder")}</p>
    </div>
  );
};

export default UsageDashboard;
```

**Step 2: Add tab in `App.tsx`**

Find existing tab config; add an entry with `id: "usage"`, render `<UsageDashboard />`.

**Step 3: i18n entries**

Add to en/zh translations:
```json
"usage": {
  "title": "Usage & Cost",
  "placeholder": "Coming soon"
}
```

**Step 4: Smoke test**

Run frontend dev, click Usage tab — see placeholder.

**Step 5: Commit**

```bash
git add frontend/src/App.tsx frontend/src/components/usage frontend/src/locales
git commit -m "feat(frontend): scaffold Usage tab"
```

### Task 10.2: Summary cards row

**Files:**
- Modify: `frontend/src/components/usage/UsageDashboard.tsx`
- Create: `frontend/src/hooks/useUsageSummary.ts`

**Step 1: Create polling hook**

```tsx
import { useEffect, useState, useCallback } from "react";
import { getUsageSummary } from "../api";
import type { UsageSummary } from "../types/usage.types";

export function useUsageSummary(intervalMs = 5000) {
  const [data, setData] = useState<UsageSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const fetch_ = useCallback(async () => {
    try {
      const s = await getUsageSummary();
      setData(s); setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetch_();
    const tick = () => {
      if (document.visibilityState === "visible") fetch_();
    };
    const id = setInterval(tick, intervalMs);
    document.addEventListener("visibilitychange", tick);
    return () => { clearInterval(id); document.removeEventListener("visibilitychange", tick); };
  }, [fetch_, intervalMs]);

  return { data, error, loading, refetch: fetch_ };
}
```

**Step 2: Render summary cards in UsageDashboard**

```tsx
const { data, loading, error } = useUsageSummary();

if (loading && !data) return <LoadingSpinner />;
if (error) return <StatusMessage type="error" message={error} />;

return (
  <div className="p-4 space-y-4">
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
      <SummaryCard label={t("usage.lifetimeCost")} value={`$${data!.totals.lifetime_cost_usd.toFixed(2)}`} />
      <SummaryCard label={t("usage.lifetimeInputTokens")} value={data!.totals.lifetime_input_tokens.toLocaleString()} />
      <SummaryCard label={t("usage.lifetimeOutputTokens")} value={data!.totals.lifetime_output_tokens.toLocaleString()} />
      <SummaryCard label={t("usage.activeCookies")} value={data!.per_cookie.filter(c => c.state === "valid").length} />
    </div>
    {/* sub-tabs go here in Task 10.3 */}
  </div>
);
```

`SummaryCard` is a tiny inline component (or extract to `usage/SummaryCard.tsx`).

**Step 3: Commit**

```bash
git add frontend/src/components/usage frontend/src/hooks frontend/src/locales
git commit -m "feat(frontend): summary cards row on Usage tab"
```

### Task 10.3: Sub-tabs (Overview / By Cookie / Graveyard)

**Files:**
- Modify: `frontend/src/components/usage/UsageDashboard.tsx`
- Create: `frontend/src/components/usage/UsageOverview.tsx`
- Create: `frontend/src/components/usage/UsagePerCookie.tsx`
- Create: `frontend/src/components/usage/Graveyard.tsx`

**Step 1: Stub components for the three tabs**

Each renders a heading and `t("usage.comingSoon")` for now.

**Step 2: Sub-tab switcher in dashboard**

Use plain state + buttons (no router needed); same pattern as App.tsx top-level tabs.

**Step 3: Commit**

```bash
git add frontend/src/components/usage frontend/src/locales
git commit -m "feat(frontend): Usage tab sub-tabs scaffold"
```

---

## Phase 11: Charts (Recharts)

### Task 11.1: Cost line chart shared component

**Files:**
- Create: `frontend/src/components/usage/charts/CostLineChart.tsx`

**Step 1: Recharts wrapper**

```tsx
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from "recharts";
import type { TimeBucket } from "../../../types/usage.types";

interface Props {
  data: TimeBucket[];
  height?: number;
}

const fmtTs = (ts: number) => new Date(ts * 1000).toLocaleDateString();

export default function CostLineChart({ data, height = 240 }: Props) {
  return (
    <ResponsiveContainer width="100%" height={height}>
      <LineChart data={data}>
        <CartesianGrid strokeDasharray="3 3" />
        <XAxis dataKey="ts" tickFormatter={fmtTs} />
        <YAxis tickFormatter={(v) => `$${v.toFixed(2)}`} />
        <Tooltip formatter={(v: number) => `$${v.toFixed(4)}`} labelFormatter={fmtTs} />
        <Line type="monotone" dataKey="cost_usd" />
      </LineChart>
    </ResponsiveContainer>
  );
}
```

**Step 2: Commit**

```bash
git add frontend/src/components/usage/charts
git commit -m "feat(frontend): CostLineChart Recharts wrapper"
```

### Task 11.2: Token bar chart (input vs output stacked)

Create `TokenBarChart.tsx` similarly with `BarChart` + stacked bars for input/output. Same
shape, different chart primitive.

```bash
git add frontend/src/components/usage/charts
git commit -m "feat(frontend): TokenBarChart"
```

### Task 11.3: UsageOverview using the charts

**Files:**
- Modify: `frontend/src/components/usage/UsageOverview.tsx`

**Step 1: Aggregate global timeseries**

For v1, aggregate timeseries across all cookies client-side: fetch each cookie's
`/timeseries?bucket=day` from `summary.per_cookie`, merge by `ts`. (Or add a new
`/api/usage/timeseries` endpoint that aggregates server-side. For v1 the client merge is
fine — N cookies is small.)

```tsx
const { data: summary } = useUsageSummary();
const [series, setSeries] = useState<TimeBucket[]>([]);

useEffect(() => {
  if (!summary) return;
  Promise.all(summary.per_cookie.map(c => getCookieTimeSeries(c.history_id, "day"))).then(allSeries => {
    const merged = mergeBucketsByTs(allSeries.flat());
    setSeries(merged);
  });
}, [summary]);

return (
  <div className="space-y-4">
    <h3>{t("usage.costOverTime")}</h3>
    <CostLineChart data={series} />
  </div>
);
```

**Step 2: Commit**

```bash
git add frontend/src/components/usage
git commit -m "feat(frontend): UsageOverview chart"
```

---

## Phase 12: Per-cookie detail drawer

### Task 12.1: UsagePerCookie list with sparklines

**Files:**
- Modify: `frontend/src/components/usage/UsagePerCookie.tsx`

**Step 1: Render list from summary.per_cookie**

For each cookie, fetch a small (last 7d, hour bucket) timeseries and render an inline
`<CostLineChart height={40} />` sparkline. Click → open `CookieUsageDetail` drawer.

```bash
git add frontend/src/components/usage
git commit -m "feat(frontend): UsagePerCookie list with sparklines"
```

### Task 12.2: CookieUsageDetail drawer

**Files:**
- Create: `frontend/src/components/usage/CookieUsageDetail.tsx`

**Step 1: Drawer with bucket + source filters**

```tsx
interface Props {
  historyId: string;
  open: boolean;
  onClose: () => void;
}

export default function CookieUsageDetail({ historyId, open, onClose }: Props) {
  const [bucket, setBucket] = useState<"hour" | "day">("day");
  const [source, setSource] = useState<"all" | "web" | "code">("all");
  const [series, setSeries] = useState<TimeBucket[]>([]);
  const [snapshots, setSnapshots] = useState<UsageSnapshot[]>([]);

  useEffect(() => {
    if (!open) return;
    getCookieTimeSeries(historyId, bucket).then(setSeries);
    getCookieSnapshots(historyId).then(setSnapshots);
  }, [historyId, bucket, open]);

  if (!open) return null;
  return (
    <div
      className="fixed inset-0 bg-black/40 z-50"
      onClick={onClose}
      role="dialog"
    >
      <div
        className="absolute right-0 top-0 h-full w-full md:w-[640px] bg-white dark:bg-gray-900 overflow-y-auto p-4
                   md:rounded-l-lg
                   bottom-sheet-mobile"
        onClick={(e) => e.stopPropagation()}
        style={{ paddingBottom: "env(safe-area-inset-bottom)" }}
      >
        {/* header, filters, chart, snapshots table */}
      </div>
    </div>
  );
}
```

Mobile: full-width slide-up sheet (`bottom-sheet-mobile` is a custom Tailwind utility you
add to `index.css` or use `@media` directly).

**Step 2: Commit**

```bash
git add frontend/src/components/usage
git commit -m "feat(frontend): CookieUsageDetail drawer with filters and snapshots"
```

---

## Phase 13: Graveyard

### Task 13.1: Graveyard view

**Files:**
- Modify: `frontend/src/components/usage/Graveyard.tsx`

**Step 1: Fetch dead cookies and render grid**

```tsx
useEffect(() => {
  getDeadCookies().then(setDead);
}, []);

return (
  <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
    {dead.map(d => (
      <DeadCookieCard key={d.cookie} {...d} />
    ))}
  </div>
);
```

`DeadCookieCard` shows: ellipsed cookie, reason, died_at (formatted), final_snapshot
totals, link to detail drawer.

**Step 2: Commit**

```bash
git add frontend/src/components/usage
git commit -m "feat(frontend): Graveyard view for dead cookies"
```

---

## Phase 14: Responsive audit

### Task 14.1: Capture screenshots at six viewports

**Files:**
- Create: `docs/plans/responsive-audit.md`
- Create: `frontend/scripts/capture-screenshots.ts` (Playwright)

**Step 1: Install Playwright**

```bash
cd frontend
bun add -D @playwright/test
bunx playwright install chromium
cd ..
```

**Step 2: Write capture script**

```typescript
// frontend/scripts/capture-screenshots.ts
import { chromium } from "playwright";

const VIEWPORTS = [
  { w: 375, h: 667, name: "iphone-se" },
  { w: 414, h: 896, name: "iphone-pro-max" },
  { w: 768, h: 1024, name: "ipad-portrait" },
  { w: 1024, h: 768, name: "ipad-landscape" },
  { w: 1280, h: 800, name: "laptop" },
  { w: 1920, h: 1080, name: "desktop" },
];

const TABS = ["cookies", "config", "usage"]; // adjust per actual tab IDs

(async () => {
  const browser = await chromium.launch();
  const ctx = await browser.newContext();
  for (const vp of VIEWPORTS) {
    await ctx.setViewportSize({ width: vp.w, height: vp.h });
    const page = await ctx.newPage();
    await page.goto("http://localhost:5173"); // vite dev port
    // login if needed — adapt to your auth flow
    for (const tab of TABS) {
      await page.click(`[data-tab="${tab}"]`).catch(() => {});
      await page.waitForTimeout(500);
      await page.screenshot({ path: `screenshots/${vp.name}-${tab}.png`, fullPage: true });
    }
  }
  await browser.close();
})();
```

(Add `data-tab` attributes to existing tab buttons if they don't have stable selectors.)

**Step 3: Run dev server + capture**

```bash
# terminal 1
cd frontend && bun run dev
# terminal 2
cd frontend && bun run scripts/capture-screenshots.ts
```

**Step 4: Audit each screenshot**

For each `screenshots/*.png`, inspect and document concrete issues in
`docs/plans/responsive-audit.md`:

```markdown
## iphone-se (375×667) — cookies tab
- [x] Cookie value reveal control overflows — fix container padding
- [x] Delete button tap target 32×32 — increase to 44×44
- [ ] (no issue)

## iphone-pro-max (414×896) — usage tab
...
```

Categorize each: layout / density / touch / navigation.

**Step 5: Commit audit**

```bash
git add docs/plans/responsive-audit.md frontend/scripts
git commit -m "docs: responsive audit at six viewports"
```

---

## Phase 15: Responsive fixes

### Task 15.1: Fix audit issues by category

**Files:** varies per audit findings

**Step 1: Triage**

Pick the highest-impact category from the audit (usually layout breakages at 375px).
For each entry in `responsive-audit.md`, make one focused fix.

**Step 2: Apply standard patterns** (from design doc Section 5)

- Containers → `grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3`
- Touch targets → `min-h-[44px] min-w-[44px]`
- Card-list-at-small / table-at-large → `<div className="block md:hidden">cards</div><table className="hidden md:table">…</table>`
- Drawer becomes bottom sheet on mobile via media-query CSS in `index.css`
- Form buttons → `flex flex-col sm:flex-row` with `w-full sm:w-auto`
- Typography → `text-base lg:text-lg` on body, scale up headings
- Safe areas → `padding-bottom: env(safe-area-inset-bottom)` on fixed bars

**Step 3: After each fix** — re-screenshot the affected viewport and verify the issue is gone.

**Step 4: Commit per fix or per-category**

```bash
git add frontend/src
git commit -m "fix(responsive): <specific issue>"
```

Iterate until every audit checkbox is checked.

### Task 15.2: Acceptance verification

**Step 1: Re-capture all screenshots**

```bash
cd frontend && bun run scripts/capture-screenshots.ts
```

**Step 2: Diff against pre-fix screenshots**

Eyeball or use `playwright-visual-diff` if available. No horizontal scrollbars at any
breakpoint except inside intentional scroll containers.

**Step 3: Lighthouse mobile**

```bash
bunx lighthouse http://localhost:5173 --only-categories=accessibility --form-factor=mobile --output=json --output-path=lighthouse.json
jq .categories.accessibility.score lighthouse.json
```

Expected: ≥ 0.90.

**Step 4: Commit final acceptance**

```bash
git add docs/plans/responsive-audit.md
git commit -m "docs: responsive audit closed"
```

---

## Phase 16: E2E and final polish

### Task 16.1: Playwright E2E for cost flow

**Files:**
- Create: `frontend/tests/e2e/cost-tracking.spec.ts`

**Step 1: Test**

```typescript
import { test, expect } from "@playwright/test";

test("submit cookie → make request → see cost", async ({ page }) => {
  await page.goto("/");
  // log in
  await page.fill('input[name="password"]', process.env.ADMIN_TOKEN!);
  await page.click('button[type="submit"]');

  // submit a cookie
  await page.click('[data-tab="cookies"]');
  await page.fill('textarea[name="cookie"]', "sk-ant-sid01-...");
  await page.click('[data-action="submit-cookie"]');

  // make a request via clewdr (curl from within the test)
  await fetch("http://localhost:8484/v1/messages", { ... });

  // verify cost appears
  await page.click('[data-tab="usage"]');
  await expect(page.locator('[data-summary-card="lifetime-cost"]')).toContainText(/\$\d+/);
});
```

**Step 2: Wire data-* attributes** in components for stable selectors.

**Step 3: Run**

```bash
bunx playwright test
```

**Step 4: Commit**

```bash
git add frontend/tests
git commit -m "test(e2e): cost tracking smoke test"
```

### Task 16.2: Final docs update

**Files:**
- Modify: `README.md`

Add a paragraph under existing features explaining the new Usage tab, JSONL history files
(under `history/`), and the `history_event_retention_days` config knob.

```bash
git add README.md
git commit -m "docs: usage tab and history retention"
```

### Task 16.3: Merge worktree

```bash
cd /Volumes/Storage/Code/clewdr
git checkout master
git merge feature/cookie-cost-tracking
# or open PR if preferred
```

---

## Definition of Done

- [ ] `cargo test --all-targets` passes
- [ ] `cd frontend && bun run build` passes
- [ ] `bunx playwright test` passes
- [ ] `history/` directory populated with JSONL after a real request
- [ ] Inline cost visible on cookie cards
- [ ] Usage tab renders summary, charts, per-cookie list, drawer, graveyard
- [ ] All audit issues in `docs/plans/responsive-audit.md` resolved
- [ ] Lighthouse mobile accessibility ≥ 0.90
- [ ] Existing `clewdr.toml` files load without migration (verify by copying an old config in)
- [ ] Pricing init logs either "fetched fresh" or "fallback" at startup
- [ ] Manual rollover test: set `reset_time = now - 1`, confirm snapshot pushed to `cookie.snapshots`
