import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { getCookieTimeSeries } from "../../api";
import type {
  PerCookieSummary,
  TimeBucket,
  UsageSummary,
} from "../../types/usage.types";
import CostLineChart from "./charts/CostLineChart";
import CookieUsageDetail from "./CookieUsageDetail";

interface Props {
  summary: UsageSummary;
}

const UsagePerCookie: React.FC<Props> = ({ summary }) => {
  const { t } = useTranslation();
  const [sparklines, setSparklines] = useState<Record<string, TimeBucket[]>>({});
  const [open, setOpen] = useState<PerCookieSummary | null>(null);

  useEffect(() => {
    let cancelled = false;
    const since = Math.floor(Date.now() / 1000) - 7 * 24 * 60 * 60;
    Promise.all(
      summary.per_cookie.map((c) =>
        getCookieTimeSeries(c.history_id, "day", { from: since })
          .catch(() => [] as TimeBucket[])
          .then((d) => [c.history_id, d] as const),
      ),
    ).then((entries) => {
      if (cancelled) return;
      const map: Record<string, TimeBucket[]> = {};
      for (const [id, data] of entries) map[id] = data;
      setSparklines(map);
    });
    return () => {
      cancelled = true;
    };
  }, [summary]);

  if (summary.per_cookie.length === 0) {
    return <p className="text-sm text-gray-500">{t("usage.byCookie.empty")}</p>;
  }

  return (
    <>
      <ul className="space-y-2">
        {summary.per_cookie.map((c) => {
          const data = sparklines[c.history_id] ?? [];
          return (
            <li key={c.history_id}>
              <button
                type="button"
                onClick={() => setOpen(c)}
                className="
                  group flex w-full items-center gap-3 rounded-md
                  border border-gray-700 bg-gray-800/50 p-3 text-left
                  hover:border-gray-600 hover:bg-gray-800
                  min-h-[64px]
                "
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-xs text-gray-300 truncate">
                      {c.cookie_ellipse}
                    </span>
                    <span
                      className={`rounded px-1.5 py-0.5 text-[10px] uppercase tracking-wide ${
                        c.state === "valid"
                          ? "bg-green-500/20 text-green-300"
                          : c.state === "exhausted"
                          ? "bg-amber-500/20 text-amber-300"
                          : "bg-gray-500/20 text-gray-300"
                      }`}
                    >
                      {c.state}
                    </span>
                  </div>
                  <div className="mt-1 text-xs text-gray-400">
                    ${c.lifetime_cost_usd.toFixed(2)} {t("usage.byCookie.lifetime")}
                    {c.snapshot_count > 0 && (
                      <>
                        {" • "}
                        {c.snapshot_count} {t("usage.byCookie.snapshots")}
                      </>
                    )}
                  </div>
                </div>
                <div className="hidden h-12 w-32 sm:block">
                  <CostLineChart data={data} height={48} minimal />
                </div>
              </button>
            </li>
          );
        })}
      </ul>

      <CookieUsageDetail
        historyId={open?.history_id ?? ""}
        cookieEllipse={open?.cookie_ellipse ?? ""}
        open={open !== null}
        onClose={() => setOpen(null)}
      />
    </>
  );
};

export default UsagePerCookie;
