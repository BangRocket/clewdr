// frontend/src/components/usage/Graveyard.tsx
//
// Stub for the Graveyard sub-tab. Loads dead cookies from
// GET /api/usage/dead and renders a card per dead cookie. Phase 13
// will polish layout and add filtering.
import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { getDeadCookies } from "../../api";
import type { DeadCookieInfo } from "../../types/usage.types";

const Graveyard: React.FC = () => {
  const { t } = useTranslation();
  const [dead, setDead] = useState<DeadCookieInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getDeadCookies()
      .then((d) => setDead(d))
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, []);

  if (error) return <div className="text-sm text-red-400">{error}</div>;
  if (dead === null)
    return <div className="text-sm text-gray-400">{t("usage.loading")}</div>;
  if (dead.length === 0)
    return (
      <div className="text-sm text-gray-500">{t("usage.graveyard.empty")}</div>
    );

  return (
    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {dead.map((d) => (
        <div
          key={d.history_id}
          className="rounded-lg border border-gray-700 bg-gray-800/50 p-3"
        >
          <div className="font-mono text-xs text-gray-400">{d.cookie_ellipse}</div>
          <div className="mt-1 text-sm text-gray-300">{d.reason}</div>
          {d.final_snapshot && (
            <div className="mt-2 text-xs text-gray-400">
              ${d.final_snapshot.cost_usd.toFixed(2)} •{" "}
              {(
                d.final_snapshot.usage.total_input_tokens +
                d.final_snapshot.usage.total_output_tokens
              ).toLocaleString()}{" "}
              {t("usage.graveyard.tokens")}
            </div>
          )}
          {d.died_at > 0 && (
            <div className="mt-1 text-xs text-gray-500">
              {t("usage.graveyard.died")}:{" "}
              {new Date(d.died_at * 1000).toLocaleString()}
            </div>
          )}
        </div>
      ))}
    </div>
  );
};

export default Graveyard;
