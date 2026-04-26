// frontend/src/components/usage/UsageOverview.tsx
//
// Stub for the Overview sub-tab. Phase 11 will replace this with charts.
import React from "react";
import { useTranslation } from "react-i18next";

import type { UsageSummary } from "../../types/usage.types";

interface Props {
  summary: UsageSummary;
}

const UsageOverview: React.FC<Props> = ({ summary }) => {
  const { t } = useTranslation();
  return (
    <div className="text-sm text-gray-300">
      <p>{t("usage.overview.placeholder")}</p>
      <p className="mt-2 text-xs text-gray-500">
        {summary.per_cookie.length} {t("usage.overview.cookieCount")}
      </p>
    </div>
  );
};

export default UsageOverview;
