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
