use std::{
    fmt::{Debug, Display},
    hash::Hash,
    ops::Deref,
    str::FromStr,
    sync::LazyLock,
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use snafu::{GenerateImplicitData, Location};
use tracing::info;

use crate::{
    config::{PLACEHOLDER_COOKIE, TokenInfo},
    error::ClewdrError,
};

/// Model family for usage bucketing
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelFamily {
    Sonnet,
    Opus,
    Other,
}

/// Per-model 1M context probing channel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claude1mChannel {
    Sonnet,
    Opus,
}

/// Per-period usage breakdown by family
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UsageBreakdown {
    #[serde(default)]
    pub total_input_tokens: u64,
    #[serde(default)]
    pub total_output_tokens: u64,

    #[serde(default)]
    pub sonnet_input_tokens: u64,
    #[serde(default)]
    pub sonnet_output_tokens: u64,

    #[serde(default)]
    pub opus_input_tokens: u64,
    #[serde(default)]
    pub opus_output_tokens: u64,
}

impl UsageBreakdown {
    /// Estimate USD cost from token counts using current pricing rates.
    /// Sonnet tokens use sonnet rates; opus tokens use opus rates;
    /// remaining "other" tokens fall back to sonnet rates as a conservative default.
    pub fn estimate_cost(&self) -> f64 {
        const SONNET_MODEL: &str = "claude-sonnet-4-5-20250929";
        const OPUS_MODEL: &str = "claude-opus-4-1-20250805";

        let other_input = self
            .total_input_tokens
            .saturating_sub(self.sonnet_input_tokens)
            .saturating_sub(self.opus_input_tokens);
        let other_output = self
            .total_output_tokens
            .saturating_sub(self.sonnet_output_tokens)
            .saturating_sub(self.opus_output_tokens);

        let sonnet_cost = crate::services::pricing::cost(
            SONNET_MODEL,
            self.sonnet_input_tokens,
            self.sonnet_output_tokens,
            0,
            0,
        );
        let opus_cost = crate::services::pricing::cost(
            OPUS_MODEL,
            self.opus_input_tokens,
            self.opus_output_tokens,
            0,
            0,
        );
        let other_cost = crate::services::pricing::cost(SONNET_MODEL, other_input, other_output, 0, 0);

        sonnet_cost + opus_cost + other_cost
    }
}

/// Window length constants matching the periods tracked on `CookieStatus`.
pub const SESSION_WINDOW_SECS: i64 = 5 * 60 * 60; // 5h
pub const WEEKLY_WINDOW_SECS: i64 = 7 * 24 * 60 * 60; // 7d

/// A rollover that the caller (in async context) should drive to UsageActor.
/// Returned alongside a reset CookieStatus so the actor call can be made
/// and the resulting UsageSnapshot pushed back into `cookie.snapshots`.
#[derive(Debug, Clone)]
pub struct PendingRollover {
    pub trigger: crate::config::SnapshotTrigger,
    pub usage: UsageBreakdown,
    pub cost_usd: f64,
    pub period_start: i64,
}

/// A struct representing a cookie
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClewdrCookie {
    inner: String,
}

impl Serialize for ClewdrCookie {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.inner)
    }
}

impl<'de> Deserialize<'de> for ClewdrCookie {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ClewdrCookie::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// A struct representing a cookie with its information
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CookieStatus {
    pub cookie: ClewdrCookie,
    #[serde(default)]
    pub token: Option<TokenInfo>,
    #[serde(default)]
    pub reset_time: Option<i64>,
    #[serde(default)]
    pub supports_claude_1m_sonnet: Option<bool>,
    #[serde(default)]
    pub supports_claude_1m_opus: Option<bool>,
    #[serde(default)]
    pub count_tokens_allowed: Option<bool>,

    // New: Per-period usage breakdown
    #[serde(default)]
    pub session_usage: UsageBreakdown,
    #[serde(default)]
    pub weekly_usage: UsageBreakdown,
    #[serde(default)]
    pub weekly_sonnet_usage: UsageBreakdown,
    #[serde(default)]
    pub weekly_opus_usage: UsageBreakdown,
    #[serde(default)]
    pub lifetime_usage: UsageBreakdown,

    // Reset boundaries for each period (epoch seconds, UTC)
    #[serde(default)]
    pub session_resets_at: Option<i64>,
    #[serde(default)]
    pub weekly_resets_at: Option<i64>,
    #[serde(default)]
    pub weekly_sonnet_resets_at: Option<i64>,
    #[serde(default)]
    pub weekly_opus_resets_at: Option<i64>,

    /// Last time we probed Anthropic console for resets_at
    #[serde(default)]
    pub resets_last_checked_at: Option<i64>,

    /// Whether the subscription exposes a reset boundary for each window
    /// None = unknown (not probed yet), Some(true) = track this window, Some(false) = no limit, never probe again
    #[serde(default)]
    pub session_has_reset: Option<bool>,
    #[serde(default)]
    pub weekly_has_reset: Option<bool>,
    #[serde(default)]
    pub weekly_sonnet_has_reset: Option<bool>,
    #[serde(default)]
    pub weekly_opus_has_reset: Option<bool>,

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
}

impl PartialEq for CookieStatus {
    fn eq(&self, other: &Self) -> bool {
        self.cookie == other.cookie
    }
}

impl Eq for CookieStatus {}

impl Hash for CookieStatus {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.cookie.hash(state);
    }
}

impl Ord for CookieStatus {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cookie.cmp(&other.cookie)
    }
}

impl PartialOrd for CookieStatus {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl CookieStatus {
    /// Creates a new CookieStatus instance
    ///
    /// # Arguments
    /// * `cookie` - Cookie string
    /// * `reset_time` - Optional timestamp when the cookie can be reused
    ///
    /// # Returns
    /// A new CookieStatus instance
    pub fn new(cookie: &str, reset_time: Option<i64>) -> Result<Self, ClewdrError> {
        let cookie = ClewdrCookie::from_str(cookie)?;
        Ok(Self {
            cookie,
            token: None,
            reset_time,
            supports_claude_1m_sonnet: Some(true),
            supports_claude_1m_opus: Some(true),
            count_tokens_allowed: None,

            session_usage: UsageBreakdown::default(),
            weekly_usage: UsageBreakdown::default(),
            weekly_sonnet_usage: UsageBreakdown::default(),
            weekly_opus_usage: UsageBreakdown::default(),
            lifetime_usage: UsageBreakdown::default(),
            session_resets_at: None,
            weekly_resets_at: None,
            weekly_sonnet_resets_at: None,
            weekly_opus_resets_at: None,
            resets_last_checked_at: None,
            session_has_reset: None,
            weekly_has_reset: None,
            weekly_sonnet_has_reset: None,
            weekly_opus_has_reset: None,

            session_cost_usd: 0.0,
            weekly_cost_usd: 0.0,
            weekly_sonnet_cost_usd: 0.0,
            weekly_opus_cost_usd: 0.0,
            lifetime_cost_usd: 0.0,
            snapshots: Vec::new(),
        })
    }

    /// Checks if the cookie's reset time has expired
    /// If the reset time has passed, sets it to None so the cookie becomes valid again
    ///
    /// # Returns
    /// The same CookieStatus with potentially updated reset_time
    pub fn reset(self) -> Self {
        if let Some(t) = self.reset_time
            && t < chrono::Utc::now().timestamp()
        {
            info!("Cookie reset time expired");
            // lifetime_cost_usd and snapshots intentionally preserved across resets
            return Self {
                reset_time: None,
                session_usage: UsageBreakdown::default(),
                weekly_usage: UsageBreakdown::default(),
                weekly_sonnet_usage: UsageBreakdown::default(),
                weekly_opus_usage: UsageBreakdown::default(),
                session_cost_usd: 0.0,
                weekly_cost_usd: 0.0,
                weekly_sonnet_cost_usd: 0.0,
                weekly_opus_cost_usd: 0.0,
                ..self
            };
        }
        self
    }

    /// Like [`reset`] but, if the session reset_time has elapsed, returns a
    /// `PendingRollover` describing the data that was about to be wiped.
    /// The caller is responsible for driving the rollover via
    /// `UsageActorHandle::rollover` and pushing the returned snapshot into
    /// `self.snapshots`.
    pub fn reset_with_rollover(self) -> (Self, Vec<PendingRollover>) {
        let mut pending = Vec::new();
        if let Some(t) = self.reset_time
            && t < chrono::Utc::now().timestamp()
        {
            let period_start = self
                .session_resets_at
                .map(|x| x.saturating_sub(SESSION_WINDOW_SECS))
                .unwrap_or_else(|| t.saturating_sub(SESSION_WINDOW_SECS));
            pending.push(PendingRollover {
                trigger: crate::config::SnapshotTrigger::SessionReset,
                usage: self.session_usage.clone(),
                cost_usd: self.session_cost_usd,
                period_start,
            });
        }
        (self.reset(), pending)
    }

    /// Walk all four reset windows. For each elapsed (`now >= *_resets_at`)
    /// and tracked (`*_has_reset == Some(true)`, plus `*_resets_at.is_some()`
    /// for the session window) bucket, emit a [`PendingRollover`] and zero
    /// the in-memory usage + cost so subsequent requests start fresh.
    ///
    /// Boundaries (`*_resets_at`) are intentionally left untouched here —
    /// callers are responsible for rolling them forward (either from a
    /// server probe or by adding the window length).
    pub fn clear_due_period_buckets_with_rollover(&mut self, now: i64) -> Vec<PendingRollover> {
        let mut pending = Vec::new();

        // Session
        if self.session_has_reset == Some(true)
            && let Some(ts) = self.session_resets_at
            && now >= ts
        {
            pending.push(PendingRollover {
                trigger: crate::config::SnapshotTrigger::SessionReset,
                usage: self.session_usage.clone(),
                cost_usd: self.session_cost_usd,
                period_start: ts.saturating_sub(SESSION_WINDOW_SECS),
            });
            self.session_usage = UsageBreakdown::default();
            self.session_cost_usd = 0.0;
        }

        // Weekly (combined)
        if self.weekly_has_reset == Some(true)
            && let Some(ts) = self.weekly_resets_at
            && now >= ts
        {
            pending.push(PendingRollover {
                trigger: crate::config::SnapshotTrigger::WeeklyReset,
                usage: self.weekly_usage.clone(),
                cost_usd: self.weekly_cost_usd,
                period_start: ts.saturating_sub(WEEKLY_WINDOW_SECS),
            });
            self.weekly_usage = UsageBreakdown::default();
            self.weekly_cost_usd = 0.0;
        }

        // Weekly Sonnet
        if self.weekly_sonnet_has_reset == Some(true)
            && let Some(ts) = self.weekly_sonnet_resets_at
            && now >= ts
        {
            pending.push(PendingRollover {
                trigger: crate::config::SnapshotTrigger::WeeklySonnetReset,
                usage: self.weekly_sonnet_usage.clone(),
                cost_usd: self.weekly_sonnet_cost_usd,
                period_start: ts.saturating_sub(WEEKLY_WINDOW_SECS),
            });
            self.weekly_sonnet_usage = UsageBreakdown::default();
            self.weekly_sonnet_cost_usd = 0.0;
        }

        // Weekly Opus
        if self.weekly_opus_has_reset == Some(true)
            && let Some(ts) = self.weekly_opus_resets_at
            && now >= ts
        {
            pending.push(PendingRollover {
                trigger: crate::config::SnapshotTrigger::WeeklyOpusReset,
                usage: self.weekly_opus_usage.clone(),
                cost_usd: self.weekly_opus_cost_usd,
                period_start: ts.saturating_sub(WEEKLY_WINDOW_SECS),
            });
            self.weekly_opus_usage = UsageBreakdown::default();
            self.weekly_opus_cost_usd = 0.0;
        }

        pending
    }

    pub fn add_token(&mut self, token: TokenInfo) {
        self.token = Some(token);
    }

    pub fn claude_1m_support(&self, channel: Claude1mChannel) -> Option<bool> {
        match channel {
            Claude1mChannel::Sonnet => self.supports_claude_1m_sonnet,
            Claude1mChannel::Opus => self.supports_claude_1m_opus,
        }
    }

    pub fn set_claude_1m_support(&mut self, channel: Claude1mChannel, value: Option<bool>) {
        match channel {
            Claude1mChannel::Sonnet => self.supports_claude_1m_sonnet = value,
            Claude1mChannel::Opus => self.supports_claude_1m_opus = value,
        }
    }

    pub fn set_count_tokens_allowed(&mut self, value: Option<bool>) {
        self.count_tokens_allowed = value;
    }

    pub fn reset_window_usage(&mut self) {
        // Legacy window counters removed; reset session buckets conservatively
        self.session_usage = UsageBreakdown::default();
        self.weekly_usage = UsageBreakdown::default();
        self.weekly_sonnet_usage = UsageBreakdown::default();
        self.weekly_opus_usage = UsageBreakdown::default();
        self.session_cost_usd = 0.0;
        self.weekly_cost_usd = 0.0;
        self.weekly_sonnet_cost_usd = 0.0;
        self.weekly_opus_cost_usd = 0.0;
    }

    // ------------------------
    // New usage aggregation
    // ------------------------

    pub fn set_session_resets_at(&mut self, ts: Option<i64>) {
        self.session_resets_at = ts;
    }

    pub fn set_weekly_resets_at(&mut self, ts: Option<i64>) {
        self.weekly_resets_at = ts;
    }

    pub fn set_weekly_sonnet_resets_at(&mut self, ts: Option<i64>) {
        self.weekly_sonnet_resets_at = ts;
    }

    pub fn set_weekly_opus_resets_at(&mut self, ts: Option<i64>) {
        self.weekly_opus_resets_at = ts;
    }

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
        // Legacy totals/windows removed; only bucketed aggregation remains

        // session bucket (total + per family)
        self.session_usage.total_input_tokens =
            self.session_usage.total_input_tokens.saturating_add(input);
        self.session_usage.total_output_tokens = self
            .session_usage
            .total_output_tokens
            .saturating_add(output);
        match family {
            ModelFamily::Sonnet => {
                self.session_usage.sonnet_input_tokens =
                    self.session_usage.sonnet_input_tokens.saturating_add(input);
                self.session_usage.sonnet_output_tokens = self
                    .session_usage
                    .sonnet_output_tokens
                    .saturating_add(output);
            }
            ModelFamily::Opus => {
                self.session_usage.opus_input_tokens =
                    self.session_usage.opus_input_tokens.saturating_add(input);
                self.session_usage.opus_output_tokens =
                    self.session_usage.opus_output_tokens.saturating_add(output);
            }
            ModelFamily::Other => {}
        }

        // weekly bucket (total + per family)
        self.weekly_usage.total_input_tokens =
            self.weekly_usage.total_input_tokens.saturating_add(input);
        self.weekly_usage.total_output_tokens =
            self.weekly_usage.total_output_tokens.saturating_add(output);
        match family {
            ModelFamily::Sonnet => {
                self.weekly_usage.sonnet_input_tokens =
                    self.weekly_usage.sonnet_input_tokens.saturating_add(input);
                self.weekly_usage.sonnet_output_tokens = self
                    .weekly_usage
                    .sonnet_output_tokens
                    .saturating_add(output);

                // weekly_sonnet bucket (only sonnet contributes)
                self.weekly_sonnet_usage.total_input_tokens = self
                    .weekly_sonnet_usage
                    .total_input_tokens
                    .saturating_add(input);
                self.weekly_sonnet_usage.total_output_tokens = self
                    .weekly_sonnet_usage
                    .total_output_tokens
                    .saturating_add(output);
                self.weekly_sonnet_usage.sonnet_input_tokens = self
                    .weekly_sonnet_usage
                    .sonnet_input_tokens
                    .saturating_add(input);
                self.weekly_sonnet_usage.sonnet_output_tokens = self
                    .weekly_sonnet_usage
                    .sonnet_output_tokens
                    .saturating_add(output);
            }
            ModelFamily::Opus => {
                self.weekly_usage.opus_input_tokens =
                    self.weekly_usage.opus_input_tokens.saturating_add(input);
                self.weekly_usage.opus_output_tokens =
                    self.weekly_usage.opus_output_tokens.saturating_add(output);
            }
            ModelFamily::Other => {}
        }

        // weekly_opus bucket (only opus contributes)
        if matches!(family, ModelFamily::Opus) {
            self.weekly_opus_usage.total_input_tokens = self
                .weekly_opus_usage
                .total_input_tokens
                .saturating_add(input);
            self.weekly_opus_usage.total_output_tokens = self
                .weekly_opus_usage
                .total_output_tokens
                .saturating_add(output);
            self.weekly_opus_usage.opus_input_tokens = self
                .weekly_opus_usage
                .opus_input_tokens
                .saturating_add(input);
            self.weekly_opus_usage.opus_output_tokens = self
                .weekly_opus_usage
                .opus_output_tokens
                .saturating_add(output);
        }

        // lifetime bucket (total + per family)
        self.lifetime_usage.total_input_tokens =
            self.lifetime_usage.total_input_tokens.saturating_add(input);
        self.lifetime_usage.total_output_tokens = self
            .lifetime_usage
            .total_output_tokens
            .saturating_add(output);
        match family {
            ModelFamily::Sonnet => {
                self.lifetime_usage.sonnet_input_tokens = self
                    .lifetime_usage
                    .sonnet_input_tokens
                    .saturating_add(input);
                self.lifetime_usage.sonnet_output_tokens = self
                    .lifetime_usage
                    .sonnet_output_tokens
                    .saturating_add(output);
            }
            ModelFamily::Opus => {
                self.lifetime_usage.opus_input_tokens =
                    self.lifetime_usage.opus_input_tokens.saturating_add(input);
                self.lifetime_usage.opus_output_tokens = self
                    .lifetime_usage
                    .opus_output_tokens
                    .saturating_add(output);
            }
            ModelFamily::Other => {}
        }

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

    /// One-time backfill: if any period's `*_cost_usd` is 0 but its `*_usage` has tokens,
    /// estimate the cost from current pricing. Intended for upgrade-time migration of
    /// configs that pre-date cost tracking.
    pub fn backfill_costs(&mut self) -> bool {
        let mut changed = false;
        if self.lifetime_cost_usd == 0.0 && self.lifetime_usage.total_input_tokens > 0 {
            self.lifetime_cost_usd = self.lifetime_usage.estimate_cost();
            changed = true;
        }
        if self.weekly_cost_usd == 0.0 && self.weekly_usage.total_input_tokens > 0 {
            self.weekly_cost_usd = self.weekly_usage.estimate_cost();
            changed = true;
        }
        if self.weekly_sonnet_cost_usd == 0.0 && self.weekly_sonnet_usage.total_input_tokens > 0 {
            self.weekly_sonnet_cost_usd = self.weekly_sonnet_usage.estimate_cost();
            changed = true;
        }
        if self.weekly_opus_cost_usd == 0.0 && self.weekly_opus_usage.total_input_tokens > 0 {
            self.weekly_opus_cost_usd = self.weekly_opus_usage.estimate_cost();
            changed = true;
        }
        if self.session_cost_usd == 0.0 && self.session_usage.total_input_tokens > 0 {
            self.session_cost_usd = self.session_usage.estimate_cost();
            changed = true;
        }
        changed
    }

    /// SHA-256 first 16 hex chars of the cookie value. Stable, non-reversible.
    /// Used as filename for the per-cookie history JSONL.
    pub fn history_id(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.cookie.inner.as_bytes());
        let hash = hasher.finalize();
        hex::encode(&hash[..8])
    }
}

impl Deref for ClewdrCookie {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Default for ClewdrCookie {
    fn default() -> Self {
        Self {
            inner: PLACEHOLDER_COOKIE.to_string(),
        }
    }
}

impl ClewdrCookie {
    pub fn ellipse(&self) -> String {
        let len = self.inner.len();
        if len > 20 {
            format!("{}...", &self.inner[..20])
        } else {
            self.inner.to_owned()
        }
    }
}

impl FromStr for ClewdrCookie {
    type Err = ClewdrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        static RE_FULL: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"sk-ant-sid\d{2}-[0-9A-Za-z_-]{86,120}-[0-9A-Za-z_-]{6}AA").unwrap()
        });
        static RE_BASE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^[0-9A-Za-z_-]{86,120}-[0-9A-Za-z_-]{6}AA$").unwrap());

        let cleaned = s
            .trim()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect::<String>();

        if let Some(found) = RE_FULL.find(&cleaned) {
            return Ok(Self {
                inner: found.as_str().to_string(),
            });
        }

        if RE_BASE.is_match(&cleaned) {
            return Ok(Self { inner: cleaned });
        }

        Err(ClewdrError::ParseCookieError {
            loc: Location::generate(),
            msg: "Invalid cookie format",
        })
    }
}

impl Display for ClewdrCookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sessionKey={}", self.inner)
    }
}

impl Debug for ClewdrCookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_base_cookie_with_len(prefix_len: usize) -> String {
        format!("{}-{}AA", "a".repeat(prefix_len), "b".repeat(6))
    }

    #[test]
    fn test_sk_cookie_from_str() {
        let base = make_base_cookie_with_len(86);
        let full = format!("sk-ant-sid01-{base}");
        let cookie = ClewdrCookie::from_str(&full).unwrap();
        assert_eq!(cookie.inner, full);
    }

    #[test]
    fn test_cookie_from_str() {
        let base = make_base_cookie_with_len(86);
        let cookie = ClewdrCookie::from_str(&base).unwrap();
        assert_eq!(cookie.inner, base);
    }

    #[test]
    fn test_long_cookie_from_str() {
        let base = make_base_cookie_with_len(109);
        let full = format!("sk-ant-sid02-{base}");
        let cookie = ClewdrCookie::from_str(&full).unwrap();
        assert_eq!(cookie.inner, full);
    }

    #[test]
    fn test_invalid_cookie() {
        let result = ClewdrCookie::from_str("invalid-cookie");
        assert!(result.is_err());
    }

    #[test]
    fn history_id_is_stable_and_short() {
        let base86 = make_base_cookie_with_len(86);
        let c1 = CookieStatus::new(&base86, None).unwrap();
        let id1 = c1.history_id();
        let id2 = c1.history_id();
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);
        assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));
        // Different cookies → different ids
        let base87 = make_base_cookie_with_len(87);
        let c2 = CookieStatus::new(&base87, None).unwrap();
        assert_ne!(c1.history_id(), c2.history_id());
    }

    #[test]
    fn reset_with_rollover_emits_pending_when_reset_time_elapsed() {
        let mut c = CookieStatus::new(&make_base_cookie_with_len(86), Some(0)).unwrap();
        c.reset_time = Some(1);
        c.session_usage.total_input_tokens = 100;
        c.session_cost_usd = 0.005;
        let (c2, pending) = c.reset_with_rollover();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].usage.total_input_tokens, 100);
        assert!((pending[0].cost_usd - 0.005).abs() < 1e-9);
        assert_eq!(c2.session_usage.total_input_tokens, 0);
        assert_eq!(c2.session_cost_usd, 0.0);
        assert!(c2.reset_time.is_none());
    }

    #[test]
    fn reset_with_rollover_no_pending_when_reset_time_in_future() {
        let mut c = CookieStatus::new(&make_base_cookie_with_len(86), None).unwrap();
        c.reset_time = Some(chrono::Utc::now().timestamp() + 3600);
        c.session_cost_usd = 0.005;
        let (c2, pending) = c.reset_with_rollover();
        assert!(pending.is_empty());
        assert!((c2.session_cost_usd - 0.005).abs() < 1e-9);
        assert!(c2.reset_time.is_some());
    }

    #[test]
    fn clear_due_period_buckets_emits_pending_for_elapsed_weekly() {
        let mut c = CookieStatus::new(&make_base_cookie_with_len(86), None).unwrap();
        c.weekly_has_reset = Some(true);
        c.weekly_resets_at = Some(100);  // in the past
        c.weekly_usage.total_input_tokens = 500;
        c.weekly_cost_usd = 0.05;
        let pending = c.clear_due_period_buckets_with_rollover(1000);
        assert!(pending.iter().any(|p| matches!(p.trigger, crate::config::SnapshotTrigger::WeeklyReset)));
        let weekly_pending = pending.iter().find(|p| matches!(p.trigger, crate::config::SnapshotTrigger::WeeklyReset)).unwrap();
        assert_eq!(weekly_pending.usage.total_input_tokens, 500);
        assert!((weekly_pending.cost_usd - 0.05).abs() < 1e-9);
        // Bucket and cost should be zeroed
        assert_eq!(c.weekly_usage.total_input_tokens, 0);
        assert_eq!(c.weekly_cost_usd, 0.0);
        // boundaries NOT updated
        assert_eq!(c.weekly_resets_at, Some(100));
    }

    #[test]
    fn clear_due_period_buckets_no_pending_when_not_elapsed() {
        let mut c = CookieStatus::new(&make_base_cookie_with_len(86), None).unwrap();
        c.weekly_has_reset = Some(true);
        c.weekly_resets_at = Some(2000);  // in the future
        c.weekly_cost_usd = 0.05;
        let pending = c.clear_due_period_buckets_with_rollover(1000);
        // Pending may exist for unrelated windows but NOT WeeklyReset
        assert!(!pending.iter().any(|p| matches!(p.trigger, crate::config::SnapshotTrigger::WeeklyReset)));
        // Cost preserved
        assert!((c.weekly_cost_usd - 0.05).abs() < 1e-9);
    }

    #[test]
    fn backfill_costs_fills_zero_cost_when_tokens_present() {
        let mut c = CookieStatus::new(&make_base_cookie_with_len(86), None).unwrap();
        c.lifetime_usage.total_input_tokens = 1_000_000;
        c.lifetime_usage.total_output_tokens = 100_000;
        c.lifetime_usage.sonnet_input_tokens = 1_000_000;
        c.lifetime_usage.sonnet_output_tokens = 100_000;
        assert_eq!(c.lifetime_cost_usd, 0.0);
        let changed = c.backfill_costs();
        assert!(changed);
        // 1M input * $3/M + 100K output * $15/M = $3 + $1.5 = $4.50
        assert!(c.lifetime_cost_usd > 4.0 && c.lifetime_cost_usd < 5.0,
                "expected ~4.50, got {}", c.lifetime_cost_usd);
    }

    #[test]
    fn backfill_costs_does_not_overwrite_existing_cost() {
        let mut c = CookieStatus::new(&make_base_cookie_with_len(86), None).unwrap();
        c.lifetime_usage.total_input_tokens = 1_000_000;
        c.lifetime_usage.sonnet_input_tokens = 1_000_000;
        c.lifetime_cost_usd = 999.0;
        let changed = c.backfill_costs();
        assert!(!changed);
        assert_eq!(c.lifetime_cost_usd, 999.0);
    }
}
