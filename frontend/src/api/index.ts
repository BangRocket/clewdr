// frontend/src/api/index.ts
/**
 * Fetches the current application version
 */
export async function getVersion() {
  const response = await fetch("/api/version");
  return await response.text();
}

/**
 * Validates authentication token
 * @param token The auth token to validate
 */
export async function validateAuthToken(token: string) {
  const response = await fetch("/api/auth", {
    method: "GET",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
  });

  return response.ok;
}

/**
 * Sends a cookie to the server.
 * @param cookie The cookie string to send
 * @returns The fetch response object
 *
 * Possible Status Codes:
 * - 200: Success
 * - 400: Invalid cookie
 * - 401: Invalid bearer token
 * - 500: Server error
 */
export async function postCookie(cookie: string) {
  const token = localStorage.getItem("authToken") || "";
  const response = await fetch("/api/cookie", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ cookie }),
  });

  if (response.status === 400) {
    throw new Error("Invalid cookie format");
  } else if (response.status === 401) {
    throw new Error("Authentication failed. Please set a valid auth token.");
  } else if (response.status === 500) {
    throw new Error("Server error.");
  }

  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }

  return response;
}

/**
 * Gets cookie status information from the server.
 * @param forceRefresh If true, bypasses cache and fetches fresh data
 * @returns The cookie status data with cache metadata
 *
 * Possible Status Codes:
 * - 200: Success with cookie status data
 * - 401: Invalid bearer token
 * - 500: Server error
 */
export async function getCookieStatus(forceRefresh = false) {
  const token = localStorage.getItem("authToken") || "";
  const url = forceRefresh ? "/api/cookies?refresh=true" : "/api/cookies";

  const response = await fetch(url, {
    method: "GET",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }

  const data = await response.json();
  const cacheStatus = response.headers.get("X-Cache-Status");
  const cacheTimestamp = response.headers.get("X-Cache-Timestamp");

  return {
    data,
    cacheInfo: {
      isFromCache: cacheStatus === "HIT",
      timestamp: cacheTimestamp ? parseInt(cacheTimestamp, 10) : null,
    },
  };
}

/**
 * Deletes a cookie from the server.
 * @param cookie The cookie string to delete
 * @returns The fetch response object
 *
 * Possible Status Codes:
 * - 204: Success (No Content)
 * - 401: Invalid bearer token
 * - 500: Server error
 */
export async function deleteCookie(cookie: string) {
  const token = localStorage.getItem("authToken") || "";
  const response = await fetch(`/api/cookie`, {
    method: "DELETE",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ cookie }),
  });

  return response;
}

/**
 * Updates per-cookie 1M support flags.
 */
export async function updateCookie1mSupport(
  cookie: string,
  supportsSonnet: boolean,
  supportsOpus: boolean
) {
  const token = localStorage.getItem("authToken") || "";
  const response = await fetch(`/api/cookie`, {
    method: "PUT",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      cookie,
      supports_claude_1m_sonnet: supportsSonnet,
      supports_claude_1m_opus: supportsOpus,
    }),
  });
  return response;
}

/**
 * Fetches the config data from the server
 */
export async function getConfig() {
  const token = localStorage.getItem("authToken") || "";
  const response = await fetch("/api/config", {
    method: "GET",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to fetch config: ${response.status}`);
  }

  return await response.json();
}

/**
 * Saves config data to the server
 * @param configData The config data to save
 */
import type { ConfigData } from "../types/config.types";

export async function saveConfig(configData: ConfigData) {
  const token = localStorage.getItem("authToken") || "";
  const response = await fetch("/api/config", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify(configData),
  });

  if (!response.ok) {
    // Try to include server error message when available for easier debugging
    try {
      const data = await response.json();
      const serverMsg = typeof data?.error === "string" ? ` - ${data.error}` : "";
      throw new Error(`Failed to save config: ${response.status}${serverMsg}`);
    } catch (_) {
      throw new Error(`Failed to save config: ${response.status}`);
    }
  }

  return response;
}

// Add this new function to frontend/src/api/index.ts

/**
 * Sends multiple cookies to the server as a batch.
 * @param cookies Array of cookie strings to send
 * @returns An array of results with status for each cookie
 */
export async function postMultipleCookies(cookies: string[]) {
  const token = localStorage.getItem("authToken") || "";
  const results = [];

  for (const cookie of cookies) {
    try {
      const response = await fetch("/api/cookie", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ cookie }),
      });

      if (response.status === 400) {
        results.push({
          cookie,
          success: false,
          message: "Invalid cookie format",
        });
      } else if (response.status === 401) {
        results.push({
          cookie,
          success: false,
          message: "Authentication failed. Please set a valid auth token.",
        });
      } else if (response.status === 500) {
        results.push({
          cookie,
          success: false,
          message: "Server error.",
        });
      } else if (!response.ok) {
        results.push({
          cookie,
          success: false,
          message: `Error ${response.status}: ${response.statusText}`,
        });
      } else {
        results.push({
          cookie,
          success: true,
          message: "Cookie submitted successfully",
        });
      }
    } catch (error) {
      results.push({
        cookie,
        success: false,
        message: error instanceof Error ? error.message : "Unknown error",
      });
    }
  }

  return results;
}

// === Usage tracking endpoints (/api/usage/*) ===

import type {
  UsageSummary,
  UsageEvent,
  UsageSnapshot,
  TimeBucket,
  DeadCookieInfo,
  PricingMeta,
  PruneStats,
} from "../types/usage.types";

/**
 * Builds the standard auth headers used by every /api/* endpoint that
 * requires the bearer token (mirrors the pattern in the existing functions
 * above: pull `authToken` from localStorage, send as `Bearer`, include
 * `Content-Type: application/json`).
 */
function authHeaders(): HeadersInit {
  const token = localStorage.getItem("authToken") || "";
  return {
    "Content-Type": "application/json",
    Authorization: `Bearer ${token}`,
  };
}

/**
 * GET /api/usage/summary — lifetime totals + per-cookie summary.
 */
export async function getUsageSummary(): Promise<UsageSummary> {
  const response = await fetch("/api/usage/summary", {
    method: "GET",
    headers: authHeaders(),
  });
  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * GET /api/usage/cookie/:id/events — raw events for a single cookie.
 *
 * @param historyId The cookie's history_id (stable opaque identifier)
 * @param opts Optional `from`/`to` (unix seconds) and `source` filter
 */
export async function getCookieEvents(
  historyId: string,
  opts?: { from?: number; to?: number; source?: "web" | "code" | "all" }
): Promise<UsageEvent[]> {
  const url = new URL(
    `/api/usage/cookie/${encodeURIComponent(historyId)}/events`,
    window.location.origin
  );
  if (opts?.from !== undefined) url.searchParams.set("from", String(opts.from));
  if (opts?.to !== undefined) url.searchParams.set("to", String(opts.to));
  if (opts?.source && opts.source !== "all") {
    url.searchParams.set("source", opts.source);
  }
  const response = await fetch(url.toString(), {
    method: "GET",
    headers: authHeaders(),
  });
  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * GET /api/usage/cookie/:id/timeseries — bucketed time-series for charting.
 *
 * @param historyId The cookie's history_id
 * @param bucket "hour" or "day"
 * @param opts Optional `from`/`to` (unix seconds)
 */
export async function getCookieTimeSeries(
  historyId: string,
  bucket: "hour" | "day",
  opts?: { from?: number; to?: number }
): Promise<TimeBucket[]> {
  const url = new URL(
    `/api/usage/cookie/${encodeURIComponent(historyId)}/timeseries`,
    window.location.origin
  );
  url.searchParams.set("bucket", bucket);
  if (opts?.from !== undefined) url.searchParams.set("from", String(opts.from));
  if (opts?.to !== undefined) url.searchParams.set("to", String(opts.to));
  const response = await fetch(url.toString(), {
    method: "GET",
    headers: authHeaders(),
  });
  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * GET /api/usage/cookie/:id/snapshots — closed-period snapshots.
 */
export async function getCookieSnapshots(
  historyId: string
): Promise<UsageSnapshot[]> {
  const response = await fetch(
    `/api/usage/cookie/${encodeURIComponent(historyId)}/snapshots`,
    {
      method: "GET",
      headers: authHeaders(),
    }
  );
  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * GET /api/usage/dead — graveyard of dead cookies + final snapshots.
 */
export async function getDeadCookies(): Promise<DeadCookieInfo[]> {
  const response = await fetch("/api/usage/dead", {
    method: "GET",
    headers: authHeaders(),
  });
  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * GET /api/usage/pricing — pricing source metadata (litellm vs fallback).
 */
export async function getPricingMeta(): Promise<PricingMeta> {
  const response = await fetch("/api/usage/pricing", {
    method: "GET",
    headers: authHeaders(),
  });
  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * POST /api/usage/prune — manually trigger retention pruning.
 */
export async function pruneUsage(): Promise<PruneStats> {
  const response = await fetch("/api/usage/prune", {
    method: "POST",
    headers: authHeaders(),
  });
  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }
  return response.json();
}

// === Codex auth endpoints (/api/codex/auth) ===

import type { CodexAuthSummary } from "../types/codex.types";

/**
 * GET /api/codex/auth — list configured Codex credentials.
 */
export async function listCodexAuth(): Promise<CodexAuthSummary[]> {
  const response = await fetch("/api/codex/auth", {
    method: "GET",
    headers: authHeaders(),
  });
  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * POST /api/codex/auth — register a new Codex credential.
 *
 * @param authJson Raw contents of `~/.codex/auth.json`
 * @param label    Optional human-friendly label
 *
 * On 400 (parse failure) the server returns an error message in the body;
 * we surface that text in the thrown Error so the form can render it inline.
 */
export async function addCodexAuth(
  authJson: string,
  label?: string,
): Promise<CodexAuthSummary> {
  const response = await fetch("/api/codex/auth", {
    method: "POST",
    headers: authHeaders(),
    body: JSON.stringify({ auth_json: authJson, label: label ?? null }),
  });
  if (response.status === 400) {
    const body = await response.text();
    throw new Error(body || "Invalid auth.json");
  }
  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }
  return response.json();
}

/**
 * DELETE /api/codex/auth/{id} — remove a credential by id.
 */
export async function deleteCodexAuth(id: string): Promise<void> {
  const response = await fetch(
    `/api/codex/auth/${encodeURIComponent(id)}`,
    {
      method: "DELETE",
      headers: authHeaders(),
    },
  );
  if (response.status === 404) {
    throw new Error("Credential not found");
  }
  if (!response.ok) {
    throw new Error(`Error ${response.status}: ${response.statusText}`);
  }
}
