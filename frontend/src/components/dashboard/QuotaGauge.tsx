import React from "react";
import { useTranslation } from "react-i18next";

interface QuotaData {
  label: string;
  percentage: number;
  resetsAt?: string;
  color?: "cyan" | "green" | "amber" | "red" | "violet";
}

interface QuotaGaugeProps {
  quotas: QuotaData[];
  isLoading?: boolean;
}

const QuotaGauge: React.FC<QuotaGaugeProps> = ({ quotas, isLoading }) => {
  const { t } = useTranslation();

  const getColorClasses = (percentage: number, color?: string) => {
    if (color) {
      const colorMap: Record<string, { bar: string; text: string }> = {
        cyan: { bar: "bg-cyan-500", text: "text-cyan-400" },
        green: { bar: "bg-green-500", text: "text-green-400" },
        amber: { bar: "bg-amber-500", text: "text-amber-400" },
        red: { bar: "bg-red-500", text: "text-red-400" },
        violet: { bar: "bg-violet-500", text: "text-violet-400" },
      };
      return colorMap[color] || colorMap.cyan;
    }

    // Auto-color based on percentage
    if (percentage >= 90) {
      return { bar: "bg-red-500", text: "text-red-400" };
    }
    if (percentage >= 70) {
      return { bar: "bg-amber-500", text: "text-amber-400" };
    }
    return { bar: "bg-cyan-500", text: "text-cyan-400" };
  };

  if (isLoading) {
    return (
      <div className="space-y-4">
        {[...Array(2)].map((_, i) => (
          <div key={i} className="animate-pulse">
            <div className="h-4 w-32 bg-gray-600 rounded mb-2" />
            <div className="h-3 w-full bg-gray-700 rounded" />
          </div>
        ))}
      </div>
    );
  }

  if (quotas.length === 0) {
    return (
      <div className="text-sm text-gray-500 text-center py-4">
        {t("dashboard.noQuotaData")}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {quotas.map((quota, index) => {
        const colors = getColorClasses(quota.percentage, quota.color);
        const clampedPercentage = Math.min(100, Math.max(0, quota.percentage));

        return (
          <div key={index}>
            <div className="flex items-center justify-between mb-1">
              <span className="text-sm text-gray-300">{quota.label}</span>
              <span className={`text-sm font-medium ${colors.text}`}>
                {quota.percentage.toFixed(1)}%
              </span>
            </div>
            <div className="h-2.5 w-full bg-gray-700 rounded-full overflow-hidden">
              <div
                className={`h-full ${colors.bar} rounded-full transition-all duration-500`}
                style={{ width: `${clampedPercentage}%` }}
              />
            </div>
            {quota.resetsAt && (
              <div className="text-xs text-gray-500 mt-1">
                {t("cookieStatus.quota.resetsAt", { time: quota.resetsAt })}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
};

export default QuotaGauge;
