// frontend/src/components/usage/UsagePerCookie.tsx
//
// Stub for the "By cookie" sub-tab. Phase 12 will replace this with a
// proper per-cookie list, sparklines, and a detail drawer.
import React from "react";
import { useTranslation } from "react-i18next";

import type { UsageSummary } from "../../types/usage.types";

interface Props {
  summary: UsageSummary;
}

const UsagePerCookie: React.FC<Props> = ({ summary }) => {
  const { t } = useTranslation();
  return (
    <div className="space-y-2 text-sm text-gray-300">
      <p>{t("usage.byCookie.placeholder")}</p>
      <ul className="list-disc pl-5 text-xs text-gray-500">
        {summary.per_cookie.map((c) => (
          <li key={c.history_id}>
            {c.cookie_ellipse} — ${c.lifetime_cost_usd.toFixed(2)} ({c.state})
          </li>
        ))}
      </ul>
    </div>
  );
};

export default UsagePerCookie;
