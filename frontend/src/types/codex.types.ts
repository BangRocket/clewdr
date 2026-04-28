// Codex authentication types — mirror backend `CodexAuthSummary` shape
// from `src/api/codex.rs`.

export type CodexAuthStatus =
  | { kind: "valid" }
  | { kind: "rate_limited"; until: number }
  | { kind: "expired" }
  | { kind: "invalid" }
  | { kind: "banned" };

export interface CodexAuthSummary {
  id: string;
  label: string | null;
  account_id_prefix: string;
  plan: string | null;
  status: CodexAuthStatus;
  last_used_at: number | null;
}
