import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { getCookieSnapshots, getCookieTimeSeries } from "../../api";
import type {
  TimeBucket,
  UsageSnapshot,
} from "../../types/usage.types";
import CostLineChart from "./charts/CostLineChart";
import TokenBarChart from "./charts/TokenBarChart";

type Bucket = "hour" | "day";
type SourceFilter = "all" | "web" | "code";

interface Props {
  historyId: string;
  cookieEllipse: string;
  open: boolean;
  onClose: () => void;
}

const CookieUsageDetail: React.FC<Props> = ({
  historyId,
  cookieEllipse,
  open,
  onClose,
}) => {
  const { t } = useTranslation();
  const [bucket, setBucket] = useState<Bucket>("day");
  const [source, setSource] = useState<SourceFilter>("all");
  const [series, setSeries] = useState<TimeBucket[]>([]);
  const [snapshots, setSnapshots] = useState<UsageSnapshot[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !historyId) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    Promise.all([
      getCookieTimeSeries(historyId, bucket).catch(() => [] as TimeBucket[]),
      getCookieSnapshots(historyId).catch(() => [] as UsageSnapshot[]),
    ])
      .then(([ts, snaps]) => {
        if (cancelled) return;
        setSeries(ts);
        setSnapshots(snaps);
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
  }, [historyId, bucket, open]);

  // NOTE: source filter applies on the client-side display layer.
  // For v1 the timeseries endpoint already aggregates server-side; we don't refetch
  // when source changes. A future enhancement could pass `source` through if/when
  // the backend timeseries endpoint accepts it.

  if (!open) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-stretch justify-end bg-black/50"
      onClick={onClose}
    >
      <div
        className="
          w-full max-w-full md:max-w-[640px]
          h-full overflow-y-auto
          bg-gray-900 border-l border-gray-700
          p-4
        "
        style={{ paddingBottom: "max(1rem, env(safe-area-inset-bottom))" }}
        onClick={(e) => e.stopPropagation()}
      >
        <header className="mb-4 flex items-start justify-between gap-2">
          <div>
            <h2 className="text-lg font-semibold text-gray-100">
              {t("usage.detail.title")}
            </h2>
            <p className="font-mono text-xs text-gray-400 break-all">
              {cookieEllipse}
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t("usage.detail.close")}
            className="min-h-[44px] min-w-[44px] rounded-md border border-gray-700 bg-gray-800 px-3 text-sm text-gray-200 hover:bg-gray-700"
          >
            ✕
          </button>
        </header>

        {/* Filters */}
        <div className="mb-4 flex flex-wrap items-center gap-2 text-xs">
          <span className="text-gray-400">{t("usage.detail.bucket")}:</span>
          {(["hour", "day"] as const).map((b) => (
            <button
              key={b}
              type="button"
              onClick={() => setBucket(b)}
              className={`min-h-[36px] rounded-md border px-3 ${
                bucket === b
                  ? "border-blue-500 bg-blue-500/20 text-blue-300"
                  : "border-gray-700 bg-gray-800 text-gray-300 hover:bg-gray-700"
              }`}
            >
              {t(`usage.detail.bucketOption.${b}`)}
            </button>
          ))}

          <span className="ml-3 text-gray-400">{t("usage.detail.source")}:</span>
          {(["all", "web", "code"] as const).map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => setSource(s)}
              className={`min-h-[36px] rounded-md border px-3 ${
                source === s
                  ? "border-blue-500 bg-blue-500/20 text-blue-300"
                  : "border-gray-700 bg-gray-800 text-gray-300 hover:bg-gray-700"
              }`}
            >
              {t(`usage.detail.sourceOption.${s}`)}
            </button>
          ))}
        </div>

        {loading && <div className="text-sm text-gray-500">{t("usage.loading")}</div>}
        {error && <div className="text-sm text-red-400">{error}</div>}

        {!loading && !error && (
          <>
            <section className="mb-6">
              <h3 className="mb-2 text-sm font-semibold text-gray-200">
                {t("usage.detail.cost")}
              </h3>
              <CostLineChart data={series} height={200} />
            </section>
            <section className="mb-6">
              <h3 className="mb-2 text-sm font-semibold text-gray-200">
                {t("usage.detail.tokens")}
              </h3>
              <TokenBarChart data={series} height={200} />
            </section>
            <section>
              <h3 className="mb-2 text-sm font-semibold text-gray-200">
                {t("usage.detail.snapshots")}
              </h3>
              {snapshots.length === 0 ? (
                <p className="text-xs text-gray-500">{t("usage.detail.noSnapshots")}</p>
              ) : (
                <ul className="space-y-2">
                  {snapshots.map((s, i) => {
                    const triggerKey = s.trigger.kind;
                    return (
                      <li
                        key={`${s.closed_at}-${i}`}
                        className="rounded-md border border-gray-700 bg-gray-800/50 p-2 text-xs"
                      >
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-gray-300">
                            {t(`usage.detail.trigger.${triggerKey}`)}
                          </span>
                          <span className="text-gray-500">
                            {new Date(s.closed_at * 1000).toLocaleString()}
                          </span>
                        </div>
                        <div className="mt-1 text-gray-400">
                          ${s.cost_usd.toFixed(4)} •{" "}
                          {(
                            s.usage.total_input_tokens + s.usage.total_output_tokens
                          ).toLocaleString()}{" "}
                          {t("usage.graveyard.tokens")} • {s.event_count} events
                        </div>
                      </li>
                    );
                  })}
                </ul>
              )}
            </section>
          </>
        )}
      </div>
    </div>
  );
};

export default CookieUsageDetail;
