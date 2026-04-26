// frontend/src/components/usage/UsageOverview.tsx
//
// Overview sub-tab: aggregates per-cookie time-series across all cookies and
// renders cost + token charts.
import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { getCookieTimeSeries } from "../../api";
import type { TimeBucket, UsageSummary } from "../../types/usage.types";
import CostLineChart from "./charts/CostLineChart";
import TokenBarChart from "./charts/TokenBarChart";

interface Props {
  summary: UsageSummary;
}

/** Merge per-cookie buckets keyed by ts. */
function mergeBuckets(rows: TimeBucket[][]): TimeBucket[] {
  const map = new Map<number, TimeBucket>();
  for (const row of rows) {
    for (const b of row) {
      const cur = map.get(b.ts);
      if (cur) {
        cur.input_tokens += b.input_tokens;
        cur.output_tokens += b.output_tokens;
        cur.cost_usd += b.cost_usd;
        cur.sonnet_input += b.sonnet_input;
        cur.sonnet_output += b.sonnet_output;
        cur.opus_input += b.opus_input;
        cur.opus_output += b.opus_output;
      } else {
        map.set(b.ts, { ...b });
      }
    }
  }
  return [...map.values()].sort((a, b) => a.ts - b.ts);
}

const UsageOverview: React.FC<Props> = ({ summary }) => {
  const { t } = useTranslation();
  const ids = useMemo(() => summary.per_cookie.map((c) => c.history_id), [summary]);
  const [series, setSeries] = useState<TimeBucket[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (ids.length === 0) {
      setSeries([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    Promise.all(ids.map((id) => getCookieTimeSeries(id, "day").catch(() => [])))
      .then((rows) => {
        if (cancelled) return;
        setSeries(mergeBuckets(rows));
      })
      .catch((e) => {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (cancelled) return;
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [ids]);

  return (
    <div className="space-y-4">
      <section>
        <h3 className="mb-2 text-sm font-semibold text-gray-200">
          {t("usage.overview.costOverTime")}
        </h3>
        {loading && <div className="text-sm text-gray-500">{t("usage.loading")}</div>}
        {error && <div className="text-sm text-red-400">{error}</div>}
        {!loading && !error && <CostLineChart data={series} height={240} />}
      </section>
      <section>
        <h3 className="mb-2 text-sm font-semibold text-gray-200">
          {t("usage.overview.tokensOverTime")}
        </h3>
        {!loading && !error && <TokenBarChart data={series} height={220} />}
      </section>
    </div>
  );
};

export default UsageOverview;
