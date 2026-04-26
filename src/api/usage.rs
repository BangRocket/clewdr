use axum::Json;
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
