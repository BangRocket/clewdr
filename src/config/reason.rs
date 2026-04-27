use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::CookieStatus;
use crate::config::ClewdrCookie;

/// Reason why a cookie is considered useless
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Error)]
pub enum Reason {
    NormalPro,
    Free,
    Disabled,
    Banned,
    Null,
    Restricted(i64),
    TooManyRequest(i64),
}

impl Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let format_time = |secs: i64| {
            chrono::DateTime::from_timestamp(secs, 0)
                .map(|t| t.format("UTC %Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or("Invalid date".to_string())
        };
        match self {
            Reason::NormalPro => write!(f, "Normal Pro account"),
            Reason::Disabled => write!(f, "Organization Disabled"),
            Reason::Free => write!(f, "Free account"),
            Reason::Banned => write!(f, "Banned"),
            Reason::Null => write!(f, "Null"),
            Reason::Restricted(i) => {
                write!(f, "Restricted/Warning: until {}", format_time(*i))
            }
            Reason::TooManyRequest(i) => {
                write!(f, "429 Too many request: until {}", format_time(*i))
            }
        }
    }
}

/// A struct representing a cookie that can't be used
/// Contains the cookie and the reason why it's considered unusable
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UselessCookie {
    pub cookie: ClewdrCookie,
    pub reason: Reason,
    #[serde(default)]
    pub final_snapshot: Option<crate::config::UsageSnapshot>,
    #[serde(default)]
    pub died_at: i64,
}

impl PartialEq<CookieStatus> for UselessCookie {
    fn eq(&self, other: &CookieStatus) -> bool {
        self.cookie == other.cookie
    }
}

impl PartialEq for UselessCookie {
    fn eq(&self, other: &Self) -> bool {
        self.cookie == other.cookie
    }
}

impl Eq for UselessCookie {}

impl Hash for UselessCookie {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.cookie.hash(state);
    }
}

impl UselessCookie {
    /// Creates a new UselessCookie instance
    ///
    /// # Arguments
    /// * `cookie` - The cookie that is unusable
    /// * `reason` - The reason why the cookie is unusable
    ///
    /// # Returns
    /// A new UselessCookie instance
    pub fn new(cookie: ClewdrCookie, reason: Reason) -> Self {
        Self {
            cookie,
            reason,
            final_snapshot: None,
            died_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Attaches a final usage snapshot to this UselessCookie.
    ///
    /// Builder-style helper used when retiring a cookie so that its
    /// last-known usage state is preserved alongside the death record.
    pub fn with_final_snapshot(mut self, snapshot: crate::config::UsageSnapshot) -> Self {
        self.final_snapshot = Some(snapshot);
        self
    }

    /// One-time backfill: if `final_snapshot.cost_usd == 0` but its usage has tokens,
    /// estimate from current pricing. Intended for upgrade-time migration.
    pub fn backfill_costs(&mut self) -> bool {
        if let Some(snap) = self.final_snapshot.as_mut() {
            if snap.cost_usd == 0.0 && snap.usage.total_input_tokens > 0 {
                snap.cost_usd = snap.usage.estimate_cost();
                return true;
            }
        }
        false
    }

    /// SHA-256 first 16 hex chars of the cookie value. Stable, non-reversible.
    /// Mirrors `CookieStatus::history_id` so dead-cookie records can be
    /// correlated with their pre-death history files.
    pub fn history_id(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.cookie.as_bytes());
        let hash = hasher.finalize();
        hex::encode(&hash[..8])
    }
}
