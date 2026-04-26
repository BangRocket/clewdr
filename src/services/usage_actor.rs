use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ractor::{Actor, ActorProcessingErr, ActorRef, RactorErr, RpcReplyPort, rpc::CallResult};
use serde::Serialize;
use snafu::{GenerateImplicitData, Location};
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tracing::error;

use crate::config::{SnapshotTrigger, UsageBreakdown, UsageEvent, UsageSnapshot};
use crate::error::ClewdrError;

/// Snapshot-marker line prefix used to distinguish JSONL marker lines
/// from raw `UsageEvent` lines when streaming back from disk.
const SNAPSHOT_MARKER_PREFIX: &str = r#"{"snapshot":"#;

/// Messages handled by the [`UsageActor`].
#[derive(Debug)]
pub enum UsageActorMessage {
    /// Append a single usage event to the per-cookie JSONL file.
    Record {
        history_id: String,
        event: UsageEvent,
    },
    /// Flush all currently-open writers to disk.
    Flush,
    /// Read events back from disk, optionally filtered by a `[from, to]` range
    /// (inclusive). Snapshot marker lines are skipped.
    QueryEvents {
        history_id: String,
        from: Option<i64>,
        to: Option<i64>,
        reply: RpcReplyPort<Vec<UsageEvent>>,
    },
    /// Build a [`UsageSnapshot`] for the given period and append a marker line.
    Rollover {
        history_id: String,
        trigger: SnapshotTrigger,
        usage: UsageBreakdown,
        cost_usd: f64,
        period_start: i64,
        reply: RpcReplyPort<UsageSnapshot>,
    },
    /// Append a final snapshot marker for a dying cookie and drop its writer.
    Tombstone {
        history_id: String,
        snapshot: UsageSnapshot,
    },
    /// Drop the writer (if open) and remove the on-disk JSONL file entirely.
    DeleteHistory {
        history_id: String,
        reply: RpcReplyPort<Result<(), ClewdrError>>,
    },
    /// Walk the history directory and prune events older than the configured
    /// retention window. Snapshot markers are always preserved.
    PruneNow {
        reply: RpcReplyPort<PruneStats>,
    },
}

/// Aggregate statistics returned by [`UsageActorMessage::PruneNow`].
#[derive(Debug, Default, Clone, Serialize)]
pub struct PruneStats {
    pub events_removed: u64,
    pub files_compacted: u64,
    pub bytes_reclaimed: u64,
}

/// Actor managing per-cookie usage event JSONL files.
pub struct UsageActor;

/// Mutable per-actor state.
pub struct UsageActorState {
    history_dir: PathBuf,
    open_writers: HashMap<String, BufWriter<tokio::fs::File>>,
}

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

        // 2-second flush tick: nudges the actor to drain its BufWriters so
        // crashes lose at most ~2s of data.
        let me = myself.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(2));
            // First tick fires immediately; skip it so the very first flush is
            // delayed by the full interval.
            tick.tick().await;
            loop {
                tick.tick().await;
                if ractor::cast!(me, UsageActorMessage::Flush).is_err() {
                    break;
                }
            }
        });

        Ok(UsageActorState {
            history_dir,
            open_writers: HashMap::new(),
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
                    error!("usage append failed for {}: {}", history_id, e);
                }
            }
            UsageActorMessage::Flush => {
                for (id, w) in state.open_writers.iter_mut() {
                    if let Err(e) = w.flush().await {
                        error!("usage flush failed for {}: {}", id, e);
                    }
                }
            }
            UsageActorMessage::QueryEvents {
                history_id,
                from,
                to,
                reply,
            } => {
                let events = read_events(&state.history_dir, &history_id, from, to)
                    .await
                    .unwrap_or_else(|e| {
                        error!("usage query failed for {}: {}", history_id, e);
                        Vec::new()
                    });
                reply.send(events)?;
            }
            UsageActorMessage::Rollover {
                history_id,
                trigger,
                usage,
                cost_usd,
                period_start,
                reply,
            } => {
                // Flush before counting so the on-disk read sees all buffered events.
                if let Some(w) = state.open_writers.get_mut(&history_id) {
                    if let Err(e) = w.flush().await {
                        error!("rollover pre-flush for {}: {}", history_id, e);
                    }
                }
                let event_count =
                    count_events_since(&state.history_dir, &history_id, period_start)
                        .await
                        .unwrap_or(0);
                let snapshot = UsageSnapshot {
                    closed_at: chrono::Utc::now().timestamp(),
                    trigger,
                    period_start,
                    usage,
                    cost_usd,
                    event_count,
                };
                if let Err(e) = append_marker(state, &history_id, &snapshot).await {
                    error!("rollover marker append for {}: {}", history_id, e);
                }
                reply.send(snapshot)?;
            }
            UsageActorMessage::Tombstone {
                history_id,
                snapshot,
            } => {
                if let Err(e) = append_marker(state, &history_id, &snapshot).await {
                    error!("tombstone marker append for {}: {}", history_id, e);
                }
                if let Some(mut w) = state.open_writers.remove(&history_id) {
                    if let Err(e) = w.flush().await {
                        error!("tombstone flush for {}: {}", history_id, e);
                    }
                    match w.into_inner().sync_all().await {
                        Ok(()) => {}
                        Err(e) => error!("tombstone sync_all for {}: {}", history_id, e),
                    }
                }
            }
            UsageActorMessage::DeleteHistory { history_id, reply } => {
                if let Some(mut w) = state.open_writers.remove(&history_id) {
                    let _ = w.flush().await;
                }
                let path = state.history_dir.join(format!("{}.jsonl", history_id));
                let result = if fs::try_exists(&path).await.unwrap_or(false) {
                    fs::remove_file(&path).await.map_err(|e| ClewdrError::Whatever {
                        message: format!("delete history {}: {}", history_id, e),
                        source: None,
                    })
                } else {
                    Ok(())
                };
                reply.send(result)?;
            }
            UsageActorMessage::PruneNow { reply } => {
                // Flush + drop all writers BEFORE prune renames files — otherwise
                // open FDs would orphan to the pre-rename inode and silently lose
                // subsequent writes.
                for w in state.open_writers.values_mut() {
                    if let Err(e) = w.flush().await {
                        error!("pre-prune flush failed: {}", e);
                    }
                }
                state.open_writers.clear();
                let stats = match prune(&state.history_dir).await {
                    Ok(s) => s,
                    Err(e) => {
                        error!("prune failed: {}", e);
                        PruneStats::default()
                    }
                };
                reply.send(stats)?;
            }
        }
        Ok(())
    }
}

/// Append a single `UsageEvent` to the cookie's JSONL file.
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

/// Append a `{"snapshot": <UsageSnapshot>}` marker line.
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

/// Borrow the open `BufWriter` for `history_id`, creating + opening the file
/// (in append mode) on first use.
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
        state
            .open_writers
            .insert(history_id.to_string(), BufWriter::new(file));
    }
    Ok(state.open_writers.get_mut(history_id).expect("just inserted"))
}

/// Stream-read events from `<dir>/<history_id>.jsonl`, skipping snapshot markers
/// and applying the optional `[from, to]` (inclusive) timestamp filter.
async fn read_events(
    dir: &Path,
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
        if line.trim().is_empty() {
            continue;
        }
        // Skip snapshot marker lines
        if line.starts_with(SNAPSHOT_MARKER_PREFIX) {
            continue;
        }
        match serde_json::from_str::<UsageEvent>(&line) {
            Ok(ev) => {
                if from.is_some_and(|f| ev.ts < f) {
                    continue;
                }
                if to.is_some_and(|t| ev.ts > t) {
                    continue;
                }
                events.push(ev);
            }
            Err(e) => {
                error!("usage read: malformed JSONL line in {:?}: {}", path, e);
            }
        }
    }
    Ok(events)
}

/// Count events whose `ts >= since` for the given history.
async fn count_events_since(
    dir: &Path,
    history_id: &str,
    since: i64,
) -> std::io::Result<u64> {
    Ok(read_events(dir, history_id, Some(since), None).await?.len() as u64)
}

/// Walk `dir` and atomically rewrite each `*.jsonl` file, dropping events older
/// than the retention cutoff configured in
/// [`crate::config::CLEWDR_CONFIG::history_event_retention_days`]. Snapshot
/// marker lines and unparseable lines are always preserved.
async fn prune(dir: &Path) -> std::io::Result<PruneStats> {
    let retention = crate::config::CLEWDR_CONFIG
        .load()
        .history_event_retention_days;
    let Some(days) = retention else {
        return Ok(PruneStats::default());
    };
    let cutoff = chrono::Utc::now().timestamp() - (days as i64) * 86_400;
    let mut stats = PruneStats::default();

    if !fs::try_exists(dir).await.unwrap_or(false) {
        return Ok(stats);
    }

    let mut entries = fs::read_dir(dir).await?;
    while let Some(e) = entries.next_entry().await? {
        let path = e.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        let original_size = e.metadata().await?.len();
        let mut kept_lines: Vec<String> = Vec::new();
        let f = tokio::fs::File::open(&path).await?;
        let reader = BufReader::new(f);
        let mut lines = reader.lines();
        let mut removed = 0u64;
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            if line.starts_with(SNAPSHOT_MARKER_PREFIX) {
                kept_lines.push(line);
                continue;
            }
            match serde_json::from_str::<UsageEvent>(&line) {
                Ok(ev) if ev.ts >= cutoff => kept_lines.push(line),
                Ok(_) => {
                    removed += 1;
                }
                Err(_) => {
                    // Preserve unrecognized lines so we never destroy data we
                    // can't classify.
                    kept_lines.push(line);
                }
            }
        }
        if removed > 0 {
            let tmp = path.with_extension("jsonl.tmp");
            let mut buf = kept_lines.join("\n");
            if !buf.is_empty() {
                buf.push('\n');
            }
            // Stream-write atomically: write to tmp, fsync, rename.
            {
                let mut tmp_file = tokio::fs::File::create(&tmp).await?;
                tmp_file.write_all(buf.as_bytes()).await?;
                tmp_file.flush().await?;
                tmp_file.sync_all().await?;
            }
            tokio::fs::rename(&tmp, &path).await?;
            stats.files_compacted += 1;
            stats.events_removed += removed;
            let new_size = tokio::fs::metadata(&path).await?.len();
            stats.bytes_reclaimed += original_size.saturating_sub(new_size);
        }
    }
    Ok(stats)
}

/// Typed handle wrapping an [`ActorRef<UsageActorMessage>`] so request-path
/// callers don't have to construct messages by hand.
#[derive(Clone)]
pub struct UsageActorHandle {
    actor_ref: ActorRef<UsageActorMessage>,
}

impl UsageActorHandle {
    /// Wrap an existing [`ActorRef`].
    pub fn new(actor_ref: ActorRef<UsageActorMessage>) -> Self {
        Self { actor_ref }
    }

    /// Fire-and-forget record. Drops the event if the mailbox send fails
    /// (which under normal operation should not happen — a closed mailbox
    /// means the actor is dead).
    pub fn try_record(&self, history_id: String, event: UsageEvent) {
        if let Err(e) = self
            .actor_ref
            .cast(UsageActorMessage::Record { history_id, event })
        {
            tracing::debug!("usage record dropped: {}", e);
        }
    }

    /// Fire-and-forget tombstone marker for a dying cookie.
    pub fn tombstone(&self, history_id: String, snapshot: UsageSnapshot) {
        if let Err(e) = self.actor_ref.cast(UsageActorMessage::Tombstone {
            history_id,
            snapshot,
        }) {
            tracing::debug!("usage tombstone dropped: {}", e);
        }
    }

    /// Close the current period and obtain the resulting snapshot.
    ///
    /// `ractor::call!` only supports tuple-style enum variants; the usage
    /// actor uses struct-style variants, so we drop down to
    /// [`ActorRef::call`] directly and replicate the macro's
    /// `CallResult` -> `Result` flattening.
    pub async fn rollover(
        &self,
        history_id: String,
        trigger: SnapshotTrigger,
        usage: UsageBreakdown,
        cost_usd: f64,
        period_start: i64,
    ) -> Result<UsageSnapshot, ClewdrError> {
        let result = self
            .actor_ref
            .call(
                |reply| UsageActorMessage::Rollover {
                    history_id,
                    trigger,
                    usage,
                    cost_usd,
                    period_start,
                    reply,
                },
                None,
            )
            .await
            .map_err(RactorErr::from);
        unwrap_call_result(result, "rollover")
    }

    /// Read events back from disk for the given history.
    pub async fn query_events(
        &self,
        history_id: String,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<Vec<UsageEvent>, ClewdrError> {
        let result = self
            .actor_ref
            .call(
                |reply| UsageActorMessage::QueryEvents {
                    history_id,
                    from,
                    to,
                    reply,
                },
                None,
            )
            .await
            .map_err(RactorErr::from);
        unwrap_call_result(result, "query_events")
    }

    /// Drop the writer and remove the on-disk JSONL file.
    pub async fn delete_history(&self, history_id: String) -> Result<(), ClewdrError> {
        let result = self
            .actor_ref
            .call(
                |reply| UsageActorMessage::DeleteHistory { history_id, reply },
                None,
            )
            .await
            .map_err(RactorErr::from);
        // Inner reply is already a `Result<(), ClewdrError>`; flatten.
        unwrap_call_result(result, "delete_history")?
    }

    /// Walk the history dir and prune events past the retention cutoff.
    pub async fn prune_now(&self) -> Result<PruneStats, ClewdrError> {
        let result = self
            .actor_ref
            .call(
                |reply| UsageActorMessage::PruneNow { reply },
                None,
            )
            .await
            .map_err(RactorErr::from);
        unwrap_call_result(result, "prune_now")
    }
}

/// Collapse `Result<CallResult<T>, RactorErr<_>>` into `Result<T, ClewdrError>`,
/// matching the error-mapping idiom used by `cookie_actor.rs`.
fn unwrap_call_result<T: std::fmt::Debug, M>(
    result: Result<CallResult<T>, RactorErr<M>>,
    op: &'static str,
) -> Result<T, ClewdrError> {
    match result {
        Ok(CallResult::Success(value)) => Ok(value),
        Ok(other) => Err(ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("UsageActor {op} call failed: {other:?}"),
        }),
        Err(e) => Err(ClewdrError::RactorError {
            loc: Location::generate(),
            msg: format!("Failed to communicate with UsageActor for {op} operation: {e}"),
        }),
    }
}

#[cfg(all(test, feature = "portable"))]
mod tests {
    use super::*;
    use crate::config::{ModelFamily, UsageSource};

    fn ev(ts: i64, model: &str, in_t: u64, out_t: u64, cost: f64) -> UsageEvent {
        UsageEvent {
            ts,
            source: UsageSource::Web,
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
        };
        append_event(
            &mut state,
            "abc123",
            &ev(100, "claude-sonnet-4-5-20250929", 10, 5, 0.01),
        )
        .await
        .unwrap();
        append_event(
            &mut state,
            "abc123",
            &ev(200, "claude-sonnet-4-5-20250929", 20, 10, 0.02),
        )
        .await
        .unwrap();
        for w in state.open_writers.values_mut() {
            w.flush().await.unwrap();
        }
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
        };
        for ts in [50i64, 150, 250] {
            append_event(&mut state, "x", &ev(ts, "m", 1, 1, 0.001))
                .await
                .unwrap();
        }
        for w in state.open_writers.values_mut() {
            w.flush().await.unwrap();
        }
        let evs = read_events(&dir, "x", Some(100), Some(200)).await.unwrap();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].ts, 150);
    }

    #[tokio::test]
    async fn handle_try_record_appends_via_actor() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let (actor_ref, _join) = ractor::Actor::spawn(None, UsageActor, dir.clone())
            .await
            .expect("spawn actor");
        let handle = UsageActorHandle::new(actor_ref.clone());
        let event = UsageEvent {
            ts: 1000,
            source: crate::config::UsageSource::Web,
            model: "claude-sonnet-4-5-20250929".into(),
            family: crate::config::ModelFamily::Sonnet,
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: 0.000003 * 100.0 + 0.000015 * 50.0,
        };
        handle.try_record("test_handle_id".into(), event.clone());
        // `prune_now` flushes + drops all writers as a side effect (per the
        // post-3.1 fix), giving us a deterministic way to force the buffer
        // to disk before reading it back via `query_events`.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let _ = handle.prune_now().await.expect("prune_now");
        let events = handle
            .query_events("test_handle_id".into(), None, None)
            .await
            .expect("query");
        assert_eq!(events.len(), 1, "expected 1 event, got {:?}", events);
        assert_eq!(events[0].ts, 1000);
        actor_ref.stop(None);
    }

    #[tokio::test]
    async fn snapshot_marker_is_skipped_by_event_query() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut state = UsageActorState {
            history_dir: dir.clone(),
            open_writers: HashMap::new(),
        };
        append_event(&mut state, "y", &ev(100, "m", 1, 1, 0.001))
            .await
            .unwrap();
        let snap = UsageSnapshot {
            closed_at: 200,
            trigger: SnapshotTrigger::SessionReset,
            period_start: 0,
            usage: UsageBreakdown::default(),
            cost_usd: 0.0,
            event_count: 1,
        };
        append_marker(&mut state, "y", &snap).await.unwrap();
        for w in state.open_writers.values_mut() {
            w.flush().await.unwrap();
        }
        let evs = read_events(&dir, "y", None, None).await.unwrap();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].ts, 100);
    }
}
