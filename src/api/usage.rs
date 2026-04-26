use axum::{
    Json,
    extract::{Path, Query},
    http::StatusCode,
};
use serde::Serialize;

use crate::config::CLEWDR_CONFIG;

/// Per-family token totals across the deployment.
#[derive(Serialize)]
pub struct FamilyTotals {
    #[serde(default)]
    pub sonnet_input_tokens: u64,
    #[serde(default)]
    pub sonnet_output_tokens: u64,
    #[serde(default)]
    pub opus_input_tokens: u64,
    #[serde(default)]
    pub opus_output_tokens: u64,
}

/// Aggregate lifetime totals across all cookies (live + dead).
#[derive(Serialize)]
pub struct UsageTotals {
    pub lifetime_cost_usd: f64,
    pub lifetime_input_tokens: u64,
    pub lifetime_output_tokens: u64,
    pub by_family: FamilyTotals,
}

/// Compact per-cookie summary suitable for list views.
#[derive(Serialize)]
pub struct PerCookieSummary {
    pub history_id: String,
    pub cookie_ellipse: String,
    /// "valid" | "exhausted" | "invalid"
    pub state: String,
    pub session_cost_usd: f64,
    pub weekly_cost_usd: f64,
    pub lifetime_cost_usd: f64,
    pub snapshot_count: usize,
    pub died_at: Option<i64>,
}

/// Top-level shape returned by `GET /api/usage/summary`.
#[derive(Serialize)]
pub struct UsageSummary {
    pub totals: UsageTotals,
    pub per_cookie: Vec<PerCookieSummary>,
}

/// `GET /api/usage/summary` — aggregate cost/token totals plus a per-cookie
/// breakdown across live (valid + exhausted) and retired (invalid) cookies.
pub async fn summary() -> Json<UsageSummary> {
    let cfg = CLEWDR_CONFIG.load();
    let mut totals = UsageTotals {
        lifetime_cost_usd: 0.0,
        lifetime_input_tokens: 0,
        lifetime_output_tokens: 0,
        by_family: FamilyTotals {
            sonnet_input_tokens: 0,
            sonnet_output_tokens: 0,
            opus_input_tokens: 0,
            opus_output_tokens: 0,
        },
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
            state: if c.reset_time.is_some() {
                "exhausted".into()
            } else {
                "valid".into()
            },
            session_cost_usd: c.session_cost_usd,
            weekly_cost_usd: c.weekly_cost_usd,
            lifetime_cost_usd: c.lifetime_cost_usd,
            snapshot_count: c.snapshots.len(),
            died_at: None,
        });
    }

    for u in cfg.wasted_cookie.iter() {
        let lifetime_cost = u.final_snapshot.as_ref().map(|s| s.cost_usd).unwrap_or(0.0);
        let (li, lo, si, so, oi, oo) = u
            .final_snapshot
            .as_ref()
            .map(|s| {
                (
                    s.usage.total_input_tokens,
                    s.usage.total_output_tokens,
                    s.usage.sonnet_input_tokens,
                    s.usage.sonnet_output_tokens,
                    s.usage.opus_input_tokens,
                    s.usage.opus_output_tokens,
                )
            })
            .unwrap_or((0, 0, 0, 0, 0, 0));

        totals.lifetime_cost_usd += lifetime_cost;
        totals.lifetime_input_tokens += li;
        totals.lifetime_output_tokens += lo;
        totals.by_family.sonnet_input_tokens += si;
        totals.by_family.sonnet_output_tokens += so;
        totals.by_family.opus_input_tokens += oi;
        totals.by_family.opus_output_tokens += oo;

        per_cookie.push(PerCookieSummary {
            history_id: u.history_id(),
            cookie_ellipse: u.cookie.ellipse(),
            state: "invalid".into(),
            session_cost_usd: 0.0,
            weekly_cost_usd: 0.0,
            lifetime_cost_usd: lifetime_cost,
            snapshot_count: u.final_snapshot.is_some() as usize,
            died_at: Some(u.died_at),
        });
    }

    Json(UsageSummary { totals, per_cookie })
}

// -----------------------------------------------------------------------------
// Phase 7.2 — GET /api/usage/cookie/{history_id}/events
// -----------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct EventsQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    /// "web" | "code" | "all" (default: "all")
    pub source: Option<String>,
}

/// `GET /api/usage/cookie/{history_id}/events` — raw per-request usage events
/// for a cookie, optionally bounded by `from`/`to` epoch seconds and filtered
/// by `source` ("web"/"code"/"all").
pub async fn events(
    Path(history_id): Path<String>,
    Query(q): Query<EventsQuery>,
) -> Result<Json<Vec<crate::config::UsageEvent>>, StatusCode> {
    let actor = crate::services::usage_actor::USAGE_ACTOR
        .get()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let events = actor
        .query_events(history_id, q.from, q.to)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let filtered: Vec<crate::config::UsageEvent> = match q.source.as_deref() {
        Some("web") => events
            .into_iter()
            .filter(|e| matches!(e.source, crate::config::UsageSource::Web))
            .collect(),
        Some("code") => events
            .into_iter()
            .filter(|e| matches!(e.source, crate::config::UsageSource::Code))
            .collect(),
        _ => events,
    };
    Ok(Json(filtered))
}

// -----------------------------------------------------------------------------
// Phase 7.3 — GET /api/usage/cookie/{history_id}/timeseries
// -----------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct TimeSeriesQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    /// "hour" | "day" (default: "day")
    pub bucket: Option<String>,
}

#[derive(serde::Serialize)]
pub struct TimeBucket {
    pub ts: i64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub sonnet_input: u64,
    pub sonnet_output: u64,
    pub opus_input: u64,
    pub opus_output: u64,
}

/// `GET /api/usage/cookie/{history_id}/timeseries` — events bucketed into
/// fixed-width windows (hour or day) for plotting.
pub async fn timeseries(
    Path(history_id): Path<String>,
    Query(q): Query<TimeSeriesQuery>,
) -> Result<Json<Vec<TimeBucket>>, StatusCode> {
    let actor = crate::services::usage_actor::USAGE_ACTOR
        .get()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let events = actor
        .query_events(history_id, q.from, q.to)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let bucket_secs: i64 = match q.bucket.as_deref() {
        Some("hour") => 3600,
        _ => 86_400,
    };

    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<i64, TimeBucket> = BTreeMap::new();
    for e in events {
        let key = (e.ts / bucket_secs) * bucket_secs;
        let b = buckets.entry(key).or_insert(TimeBucket {
            ts: key,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: 0.0,
            sonnet_input: 0,
            sonnet_output: 0,
            opus_input: 0,
            opus_output: 0,
        });
        b.input_tokens += e.input_tokens;
        b.output_tokens += e.output_tokens;
        b.cost_usd += e.cost_usd;
        match e.family {
            crate::config::ModelFamily::Sonnet => {
                b.sonnet_input += e.input_tokens;
                b.sonnet_output += e.output_tokens;
            }
            crate::config::ModelFamily::Opus => {
                b.opus_input += e.input_tokens;
                b.opus_output += e.output_tokens;
            }
            crate::config::ModelFamily::Other => {}
        }
    }
    Ok(Json(buckets.into_values().collect()))
}

// -----------------------------------------------------------------------------
// Phase 7.4 — snapshots, dead, pricing meta, prune
// -----------------------------------------------------------------------------

/// `GET /api/usage/cookie/{history_id}/snapshots` — preserved usage snapshots
/// captured at session/weekly/family-window boundaries (live cookies) or the
/// single final_snapshot of a retired cookie.
pub async fn snapshots(
    Path(history_id): Path<String>,
) -> Result<Json<Vec<crate::config::UsageSnapshot>>, StatusCode> {
    let cfg = CLEWDR_CONFIG.load();
    // Search live cookies first.
    if let Some(c) = cfg.cookie_array.iter().find(|c| c.history_id() == history_id) {
        return Ok(Json(c.snapshots.clone()));
    }
    // Then dead cookies — they have at most one final_snapshot.
    if let Some(u) = cfg.wasted_cookie.iter().find(|u| u.history_id() == history_id) {
        let snaps: Vec<_> = u.final_snapshot.iter().cloned().collect();
        return Ok(Json(snaps));
    }
    Err(StatusCode::NOT_FOUND)
}

#[derive(serde::Serialize)]
pub struct DeadCookieInfo {
    pub history_id: String,
    pub cookie_ellipse: String,
    pub reason: String,
    pub died_at: i64,
    pub final_snapshot: Option<crate::config::UsageSnapshot>,
}

/// `GET /api/usage/dead` — list of retired cookies with their final snapshot
/// and cause of death. Used by the Graveyard view.
pub async fn dead() -> Json<Vec<DeadCookieInfo>> {
    let cfg = CLEWDR_CONFIG.load();
    let dead: Vec<DeadCookieInfo> = cfg
        .wasted_cookie
        .iter()
        .map(|u| DeadCookieInfo {
            history_id: u.history_id(),
            cookie_ellipse: u.cookie.ellipse(),
            reason: format!("{}", u.reason),
            died_at: u.died_at,
            final_snapshot: u.final_snapshot.clone(),
        })
        .collect();
    Json(dead)
}

#[derive(serde::Serialize)]
pub struct PricingMeta {
    pub source: crate::services::pricing::PricingSource,
    pub fetched_at: i64,
    pub model_count: usize,
}

/// `GET /api/usage/pricing` — metadata about the current pricing table
/// (source, age, count). The full models map is intentionally NOT exposed
/// here — it's ~200KB+ of LiteLLM data and not useful in the UI.
pub async fn pricing_meta() -> Json<PricingMeta> {
    let table = crate::services::pricing::current();
    Json(PricingMeta {
        source: table.source.clone(),
        fetched_at: table.fetched_at,
        model_count: table.models.len(),
    })
}

/// `POST /api/usage/prune` — force-run the JSONL retention prune now and
/// return aggregate stats. Mutating, so POST.
pub async fn prune_now() -> Result<Json<crate::services::usage_actor::PruneStats>, StatusCode> {
    let actor = crate::services::usage_actor::USAGE_ACTOR
        .get()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let stats = actor
        .prune_now()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(stats))
}
