// frontend/src/components/usage/UsageDashboard.tsx
//
// Top-level Usage tab content: summary cards row + sub-tab navigation
// (Overview / By Cookie / Graveyard). Polls /api/usage/summary via
// useUsageSummary so totals stay live while the tab is in the foreground.
import React, { useState } from "react";
import { useTranslation } from "react-i18next";

import { useUsageSummary } from "../../hooks/useUsageSummary";
import LoadingSpinner from "../common/LoadingSpinner";
import StatusMessage from "../common/StatusMessage";
import TabNavigation from "../common/TabNavigation";
import Graveyard from "./Graveyard";
import UsageOverview from "./UsageOverview";
import UsagePerCookie from "./UsagePerCookie";

type SubTab = "overview" | "by_cookie" | "graveyard";

interface SummaryCardProps {
  label: string;
  value: string | number;
}

const SummaryCard: React.FC<SummaryCardProps> = ({ label, value }) => (
  <div className="rounded-lg border border-gray-700 bg-gray-800/50 p-3">
    <div className="text-xs text-gray-400">{label}</div>
    <div className="mt-1 text-lg font-semibold text-gray-100">{value}</div>
  </div>
);

const UsageDashboard: React.FC = () => {
  const { t } = useTranslation();
  const { data, loading, error } = useUsageSummary();
  const [subTab, setSubTab] = useState<SubTab>("overview");

  if (loading && !data) return <LoadingSpinner />;
  if (error) return <StatusMessage type="error" message={error} />;
  if (!data) return null;

  const activeCount = data.per_cookie.filter((c) => c.state === "valid").length;

  const subTabs = [
    { id: "overview", label: t("usage.subtabs.overview"), color: "cyan" },
    { id: "by_cookie", label: t("usage.subtabs.by_cookie"), color: "blue" },
    { id: "graveyard", label: t("usage.subtabs.graveyard"), color: "amber" },
  ];

  return (
    <div className="w-full space-y-4">
      {/* Summary cards row */}
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <SummaryCard
          label={t("usage.lifetimeCost")}
          value={`$${data.totals.lifetime_cost_usd.toFixed(2)}`}
        />
        <SummaryCard
          label={t("usage.lifetimeInputTokens")}
          value={data.totals.lifetime_input_tokens.toLocaleString()}
        />
        <SummaryCard
          label={t("usage.lifetimeOutputTokens")}
          value={data.totals.lifetime_output_tokens.toLocaleString()}
        />
        <SummaryCard label={t("usage.activeCookies")} value={activeCount} />
      </div>

      {/* Sub-tab navigation (matches the project's ClaudeTabs pattern) */}
      <TabNavigation
        tabs={subTabs}
        activeTab={subTab}
        onTabChange={(id) => setSubTab(id as SubTab)}
        className="mb-2"
      />

      {/* Sub-tab content */}
      <div>
        {subTab === "overview" && <UsageOverview summary={data} />}
        {subTab === "by_cookie" && <UsagePerCookie summary={data} />}
        {subTab === "graveyard" && <Graveyard />}
      </div>
    </div>
  );
};

export default UsageDashboard;
