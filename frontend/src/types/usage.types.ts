// frontend/src/types/usage.types.ts
//
// TypeScript types for the /api/usage/* endpoints exposed by src/api/usage.rs.
//
// NOTE: `UsageBreakdown` is already exported from `./cookie.types` (with all
// fields optional, as returned by /api/cookies). The snapshot/timeseries
// version returned by the usage endpoints has *required* fields and a
// different semantic role, so it is exported here under a distinct name
// (`SnapshotUsageBreakdown`) to avoid an import-site collision.

export type UsageSource = "web" | "code";

export type ModelFamily = "sonnet" | "opus" | "other";

export interface SnapshotUsageBreakdown {
  total_input_tokens: number;
  total_output_tokens: number;
  sonnet_input_tokens: number;
  sonnet_output_tokens: number;
  opus_input_tokens: number;
  opus_output_tokens: number;
}

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
  usage: SnapshotUsageBreakdown;
  cost_usd: number;
  event_count: number;
}

export interface FamilyTotals {
  sonnet_input_tokens: number;
  sonnet_output_tokens: number;
  opus_input_tokens: number;
  opus_output_tokens: number;
}

export interface UsageTotals {
  lifetime_cost_usd: number;
  lifetime_input_tokens: number;
  lifetime_output_tokens: number;
  by_family: FamilyTotals;
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

export interface UsageSummary {
  totals: UsageTotals;
  per_cookie: PerCookieSummary[];
}

export interface TimeBucket {
  ts: number;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
  sonnet_input: number;
  sonnet_output: number;
  opus_input: number;
  opus_output: number;
}

export interface DeadCookieInfo {
  history_id: string;
  cookie_ellipse: string;
  reason: string;
  died_at: number;
  final_snapshot: UsageSnapshot | null;
}

export interface PricingMeta {
  source: "litellm" | "fallback";
  fetched_at: number;
  model_count: number;
}

export interface PruneStats {
  events_removed: number;
  files_compacted: number;
  bytes_reclaimed: number;
}
