import React from "react";
import { useTranslation } from "react-i18next";

interface CookieCounts {
  valid: number;
  inUse: number;
  exhausted: number;
  invalid: number;
  total: number;
}

interface UsageSummaryProps {
  counts: CookieCounts;
  isLoading?: boolean;
}

const UsageSummary: React.FC<UsageSummaryProps> = ({ counts, isLoading }) => {
  const { t } = useTranslation();

  const cards = [
    {
      label: t("dashboard.totalCookies"),
      value: counts.total,
      color: "text-white",
      bgColor: "bg-gray-700/50",
      borderColor: "border-gray-600",
    },
    {
      label: t("dashboard.validCookies"),
      value: counts.valid,
      color: "text-green-400",
      bgColor: "bg-green-900/20",
      borderColor: "border-green-800",
    },
    {
      label: t("dashboard.inUseCookies"),
      value: counts.inUse,
      color: "text-cyan-400",
      bgColor: "bg-cyan-900/20",
      borderColor: "border-cyan-800",
    },
    {
      label: t("dashboard.exhaustedCookies"),
      value: counts.exhausted,
      color: "text-amber-400",
      bgColor: "bg-amber-900/20",
      borderColor: "border-amber-800",
    },
    {
      label: t("dashboard.invalidCookies"),
      value: counts.invalid,
      color: "text-red-400",
      bgColor: "bg-red-900/20",
      borderColor: "border-red-800",
    },
  ];

  if (isLoading) {
    return (
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-3">
        {[...Array(5)].map((_, i) => (
          <div
            key={i}
            className="p-4 rounded-lg bg-gray-700/30 border border-gray-700 animate-pulse"
          >
            <div className="h-4 w-20 bg-gray-600 rounded mb-2" />
            <div className="h-8 w-12 bg-gray-600 rounded" />
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-3">
      {cards.map((card) => (
        <div
          key={card.label}
          className={`p-4 rounded-lg ${card.bgColor} border ${card.borderColor} transition-all hover:scale-[1.02]`}
        >
          <div className="text-xs text-gray-400 mb-1">{card.label}</div>
          <div className={`text-2xl font-bold ${card.color}`}>{card.value}</div>
        </div>
      ))}
    </div>
  );
};

export default UsageSummary;
