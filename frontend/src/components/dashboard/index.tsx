// frontend/src/components/dashboard/index.tsx
import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Grid, Paper, Text, Stack, Box } from "@mantine/core";
import { getCookieStatus } from "../../api";
import UsageSummary from "./UsageSummary";
import QuotaGauge from "./QuotaGauge";
import QuickActions from "./QuickActions";
import LogsPanel from "./LogsPanel";
import OAuthStatus from "./OAuthStatus";
import TokenCostCalculator from "./TokenCostCalculator";

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
  input_tokens?: number;
  output_tokens?: number;
}

interface CookieData {
  valid: CookieItem[];
  exhausted: CookieItem[];
  invalid: CookieItem[];
}

export function Dashboard() {
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
      inUse: 0,
      exhausted,
      invalid,
      total: valid + exhausted + invalid,
    };
  };

  const getAllCookies = (): CookieItem[] => {
    if (!cookieData) return [];
    return [...(cookieData.valid || []), ...(cookieData.exhausted || [])];
  };

  const getAverageQuotas = () => {
    const allCookies = getAllCookies();
    if (allCookies.length === 0) return [];

    const cookiesWithSession = allCookies.filter(
      (c) => c.session_utilization !== undefined
    );
    const avgSession =
      cookiesWithSession.length > 0
        ? cookiesWithSession.reduce(
            (sum, c) => sum + (c.session_utilization || 0),
            0
          ) / cookiesWithSession.length
        : 0;

    const cookiesWithSonnet = allCookies.filter(
      (c) => c.seven_day_sonnet_utilization !== undefined
    );
    const avgSonnet =
      cookiesWithSonnet.length > 0
        ? cookiesWithSonnet.reduce(
            (sum, c) => sum + (c.seven_day_sonnet_utilization || 0),
            0
          ) / cookiesWithSonnet.length
        : null;

    const cookiesWithOpus = allCookies.filter(
      (c) => c.seven_day_opus_utilization !== undefined
    );
    const avgOpus =
      cookiesWithOpus.length > 0
        ? cookiesWithOpus.reduce(
            (sum, c) => sum + (c.seven_day_opus_utilization || 0),
            0
          ) / cookiesWithOpus.length
        : null;

    type QuotaColor = "cyan" | "green" | "yellow" | "red" | "violet";
    const quotas: Array<{
      label: string;
      percentage: number;
      color?: QuotaColor;
    }> = [];

    if (cookiesWithSession.length > 0) {
      quotas.push({
        label: t("cookieStatus.quota.session"),
        percentage: avgSession,
      });
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

  // Calculate token usage from cookies
  const getTokenUsage = () => {
    const allCookies = getAllCookies();
    let totalInput = 0;
    let totalOutput = 0;

    allCookies.forEach((cookie) => {
      if (cookie.input_tokens) totalInput += cookie.input_tokens;
      if (cookie.output_tokens) totalOutput += cookie.output_tokens;
    });

    return { totalInput, totalOutput };
  };

  const { totalInput, totalOutput } = getTokenUsage();

  return (
    <Grid gutter="md">
      {/* Left column - Stats & Actions */}
      <Grid.Col span={{ base: 12, lg: 4 }}>
        <Stack gap="md">
          {/* Usage Summary */}
          <Paper p="md" radius="md" withBorder>
            <Text size="xs" c="dimmed" tt="uppercase" fw={500} mb="md">
              {t("dashboard.usageSummary")}
            </Text>
            <UsageSummary counts={getCounts()} isLoading={isLoading} />
          </Paper>

          {/* Quota Gauges */}
          <Paper p="md" radius="md" withBorder>
            <Text size="xs" c="dimmed" tt="uppercase" fw={500} mb="md">
              {t("dashboard.quotaUsage")}
            </Text>
            <QuotaGauge quotas={getAverageQuotas()} isLoading={isLoading} />
          </Paper>

          {/* Token Cost Calculator */}
          <Paper p="md" radius="md" withBorder>
            <Text size="xs" c="dimmed" tt="uppercase" fw={500} mb="md">
              {t("dashboard.tokenCost.title")}
            </Text>
            <TokenCostCalculator
              totalInputTokens={totalInput}
              totalOutputTokens={totalOutput}
              isLoading={isLoading}
            />
          </Paper>

          {/* OAuth Status */}
          <Paper p="md" radius="md" withBorder>
            <Text size="xs" c="dimmed" tt="uppercase" fw={500} mb="md">
              {t("dashboard.oauth.title")}
            </Text>
            <OAuthStatus cookies={getAllCookies()} isLoading={isLoading} />
          </Paper>

          {/* Quick Actions */}
          <Paper p="md" radius="md" withBorder>
            <QuickActions
              onCookieSubmitted={fetchCookieStatus}
              onRefresh={fetchCookieStatus}
            />
          </Paper>
        </Stack>
      </Grid.Col>

      {/* Right column - Console Logs */}
      <Grid.Col span={{ base: 12, lg: 8 }}>
        <Box h={{ base: 500, lg: "calc(100vh - 180px)" }}>
          <LogsPanel />
        </Box>
      </Grid.Col>
    </Grid>
  );
}

export default Dashboard;
