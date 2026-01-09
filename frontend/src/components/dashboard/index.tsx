import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { getCookieStatus } from "../../api";
import UsageSummary from "./UsageSummary";
import QuotaGauge from "./QuotaGauge";
import QuickActions from "./QuickActions";
import LogsPanel from "./LogsPanel";
import OAuthStatus from "./OAuthStatus";

interface TokenInfo {
  expires_at: string;
  organization: {
    uuid: string;
  };
}

interface CookieItem {
  cookie: string;
  token?: TokenInfo;
  session_utilization?: number;
  seven_day_utilization?: number;
  seven_day_sonnet_utilization?: number;
  seven_day_opus_utilization?: number;
}

interface CookieData {
  valid: CookieItem[];
  exhausted: CookieItem[];
  invalid: CookieItem[];
}

const Dashboard: React.FC = () => {
  const { t } = useTranslation();
  const [cookieData, setCookieData] = useState<CookieData | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const fetchCookieStatus = useCallback(async () => {
    try {
      const { data } = await getCookieStatus();
      setCookieData(data);
    } catch (error) {
      console.error("Failed to fetch cookie status:", error);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchCookieStatus();
    // Refresh every 30 seconds
    const interval = setInterval(fetchCookieStatus, 30000);
    return () => clearInterval(interval);
  }, [fetchCookieStatus]);

  const getCounts = () => {
    if (!cookieData) {
      return { valid: 0, inUse: 0, exhausted: 0, invalid: 0, total: 0 };
    }
    const valid = cookieData.valid?.length || 0;
    const exhausted = cookieData.exhausted?.length || 0;
    const invalid = cookieData.invalid?.length || 0;
    return {
      valid,
      inUse: 0, // API doesn't separate in-use
      exhausted,
      invalid,
      total: valid + exhausted + invalid,
    };
  };

  const getAllCookies = (): CookieItem[] => {
    if (!cookieData) return [];
    return [
      ...(cookieData.valid || []),
      ...(cookieData.exhausted || []),
    ];
  };

  const getAverageQuotas = () => {
    const allCookies = getAllCookies();
    if (allCookies.length === 0) return [];

    const cookiesWithSession = allCookies.filter(
      (c) => c.session_utilization !== undefined
    );
    const avgSession =
      cookiesWithSession.length > 0
        ? cookiesWithSession.reduce((sum, c) => sum + (c.session_utilization || 0), 0) /
          cookiesWithSession.length
        : 0;

    const cookiesWithSonnet = allCookies.filter(
      (c) => c.seven_day_sonnet_utilization !== undefined
    );
    const avgSonnet =
      cookiesWithSonnet.length > 0
        ? cookiesWithSonnet.reduce((sum, c) => sum + (c.seven_day_sonnet_utilization || 0), 0) /
          cookiesWithSonnet.length
        : null;

    const cookiesWithOpus = allCookies.filter(
      (c) => c.seven_day_opus_utilization !== undefined
    );
    const avgOpus =
      cookiesWithOpus.length > 0
        ? cookiesWithOpus.reduce((sum, c) => sum + (c.seven_day_opus_utilization || 0), 0) /
          cookiesWithOpus.length
        : null;

    type QuotaColor = "cyan" | "green" | "amber" | "red" | "violet";
    const quotas: Array<{ label: string; percentage: number; color?: QuotaColor }> = [];

    if (cookiesWithSession.length > 0) {
      quotas.push({ label: t("cookieStatus.quota.session"), percentage: avgSession });
    }

    if (avgSonnet !== null) {
      quotas.push({
        label: t("cookieStatus.quota.sevenDaySonnet"),
        percentage: avgSonnet,
        color: "green",
      });
    }

    if (avgOpus !== null) {
      quotas.push({
        label: t("cookieStatus.quota.sevenDayOpus"),
        percentage: avgOpus,
        color: "violet",
      });
    }

    return quotas;
  };

  return (
    <div className="h-full flex flex-col lg:flex-row gap-6">
      {/* Left panel - Stats & Actions */}
      <div className="w-full lg:w-80 flex-shrink-0 space-y-4">
        {/* Usage Summary Cards */}
        <div className="bg-gray-800/50 rounded-xl p-4 border border-gray-700">
          <h2 className="text-sm font-medium text-gray-400 uppercase tracking-wider mb-4">
            {t("dashboard.usageSummary")}
          </h2>
          <UsageSummary counts={getCounts()} isLoading={isLoading} />
        </div>

        {/* Quota Gauges */}
        <div className="bg-gray-800/50 rounded-xl p-4 border border-gray-700">
          <h2 className="text-sm font-medium text-gray-400 uppercase tracking-wider mb-4">
            {t("dashboard.quotaUsage")}
          </h2>
          <QuotaGauge quotas={getAverageQuotas()} isLoading={isLoading} />
        </div>

        {/* OAuth Status */}
        <div className="bg-gray-800/50 rounded-xl p-4 border border-gray-700">
          <h2 className="text-sm font-medium text-gray-400 uppercase tracking-wider mb-4">
            {t("dashboard.oauth.title")}
          </h2>
          <OAuthStatus cookies={getAllCookies()} isLoading={isLoading} />
        </div>

        {/* Quick Actions */}
        <div className="bg-gray-800/50 rounded-xl p-4 border border-gray-700">
          <QuickActions
            onCookieSubmitted={fetchCookieStatus}
            onRefresh={fetchCookieStatus}
          />
        </div>
      </div>

      {/* Right panel - Console Logs */}
      <div className="flex-1 min-w-0">
        <LogsPanel />
      </div>
    </div>
  );
};

export default Dashboard;
