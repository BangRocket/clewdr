// frontend/src/hooks/useUsageSummary.ts
//
// Polling hook for GET /api/usage/summary. Refetches on a fixed interval
// and only when the document is visible (avoids hammering the backend
// when the tab is in the background).
import { useCallback, useEffect, useState } from "react";

import { getUsageSummary } from "../api";
import type { UsageSummary } from "../types/usage.types";

export function useUsageSummary(intervalMs = 5000) {
  const [data, setData] = useState<UsageSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchOnce = useCallback(async () => {
    try {
      const s = await getUsageSummary();
      setData(s);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchOnce();
    const tick = () => {
      if (document.visibilityState === "visible") fetchOnce();
    };
    const id = setInterval(tick, intervalMs);
    document.addEventListener("visibilitychange", tick);
    return () => {
      clearInterval(id);
      document.removeEventListener("visibilitychange", tick);
    };
  }, [fetchOnce, intervalMs]);

  return { data, error, loading, refetch: fetchOnce };
}
