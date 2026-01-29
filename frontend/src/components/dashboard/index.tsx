// frontend/src/components/dashboard/index.tsx
import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  Grid,
  Paper,
  Text,
  Stack,
  Box,
  Group,
  SimpleGrid,
  ThemeIcon,
  RingProgress,
  Center,
} from "@mantine/core";
import {
  IconCookie,
  IconCheck,
  IconAlertTriangle,
  IconX,
  IconCoins,
  IconArrowUp,
  IconArrowDown,
} from "@tabler/icons-react";
import { getCookieStatus } from "../../api";
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
  sonnet_input_tokens?: number;
  sonnet_output_tokens?: number;
  opus_input_tokens?: number;
  opus_output_tokens?: number;
}

interface CookieData {
  valid: CookieItem[];
  exhausted: CookieItem[];
  invalid: CookieItem[];
}

// Stat Card Component for the top row
interface StatCardProps {
  label: string;
  value: string | number;
  icon: React.ReactNode;
  color: string;
  subLabel?: string;
  trend?: "up" | "down";
  trendValue?: string;
}

function StatCard({ label, value, icon, color, subLabel, trend, trendValue }: StatCardProps) {
  return (
    <Box className={`glass-card stat-card-${color}`} p="lg" style={{ borderRadius: 16 }}>
      <Group justify="space-between" align="flex-start">
        <Box>
          <Text size="xs" c="dimmed" tt="uppercase" fw={500} mb={4}>
            {label}
          </Text>
          <Text size="xl" fw={700} c="white">
            {value}
          </Text>
          {subLabel && (
            <Text size="xs" c="dimmed" mt={4}>
              {subLabel}
            </Text>
          )}
          {trend && trendValue && (
            <Group gap={4} mt={4}>
              {trend === "up" ? (
                <IconArrowUp size={14} color="var(--accent-cyan)" />
              ) : (
                <IconArrowDown size={14} color="var(--accent-magenta)" />
              )}
              <Text size="xs" c={trend === "up" ? "cyan" : "magenta"}>
                {trendValue}
              </Text>
            </Group>
          )}
        </Box>
        <ThemeIcon
          size={48}
          radius="xl"
          variant="light"
          color={color}
          style={{ opacity: 0.8 }}
        >
          {icon}
        </ThemeIcon>
      </Group>
    </Box>
  );
}

// Mini gauge for quota display
interface MiniQuotaProps {
  label: string;
  percentage: number;
  color: string;
}

function MiniQuota({ label, percentage, color }: MiniQuotaProps) {
  return (
    <Box ta="center">
      <RingProgress
        size={80}
        thickness={6}
        roundCaps
        sections={[{ value: percentage, color }]}
        label={
          <Center>
            <Text size="xs" fw={600}>
              {percentage.toFixed(0)}%
            </Text>
          </Center>
        }
      />
      <Text size="xs" c="dimmed" mt={4}>
        {label}
      </Text>
    </Box>
  );
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

  // Calculate token usage from cookies - aggregated by model type
  const getTokenUsage = () => {
    const allCookies = getAllCookies();
    let totalInput = 0;
    let totalOutput = 0;
    let sonnetInput = 0;
    let sonnetOutput = 0;
    let opusInput = 0;
    let opusOutput = 0;

    allCookies.forEach((cookie) => {
      // Total tokens
      if (cookie.input_tokens) totalInput += cookie.input_tokens;
      if (cookie.output_tokens) totalOutput += cookie.output_tokens;

      // Model-specific tokens (if available from cookie data)
      if (cookie.sonnet_input_tokens) sonnetInput += cookie.sonnet_input_tokens;
      if (cookie.sonnet_output_tokens) sonnetOutput += cookie.sonnet_output_tokens;
      if (cookie.opus_input_tokens) opusInput += cookie.opus_input_tokens;
      if (cookie.opus_output_tokens) opusOutput += cookie.opus_output_tokens;
    });

    return {
      totalInput,
      totalOutput,
      sonnetInput,
      sonnetOutput,
      opusInput,
      opusOutput
    };
  };

  const counts = getCounts();
  const { totalInput, totalOutput, sonnetInput, sonnetOutput, opusInput, opusOutput } = getTokenUsage();
  const quotas = getAverageQuotas();

  // Format large numbers
  const formatNumber = (num: number) => {
    if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M`;
    if (num >= 1_000) return `${(num / 1_000).toFixed(1)}K`;
    return num.toString();
  };

  return (
    <Stack gap="md">
      {/* Top Row - Stat Cards */}
      <SimpleGrid cols={{ base: 2, sm: 4 }} spacing="md">
        <StatCard
          label={t("dashboard.totalCookies")}
          value={counts.total}
          icon={<IconCookie size={24} />}
          color="cyan"
          subLabel={`${counts.valid} ${t("dashboard.validCookies").toLowerCase()}`}
        />
        <StatCard
          label={t("dashboard.validCookies")}
          value={counts.valid}
          icon={<IconCheck size={24} />}
          color="green"
        />
        <StatCard
          label={t("dashboard.exhaustedCookies")}
          value={counts.exhausted}
          icon={<IconAlertTriangle size={24} />}
          color="violet"
        />
        <StatCard
          label={t("dashboard.invalidCookies")}
          value={counts.invalid}
          icon={<IconX size={24} />}
          color="magenta"
        />
      </SimpleGrid>

      {/* Second Row - Token Stats */}
      <SimpleGrid cols={{ base: 2, sm: 4 }} spacing="md">
        <StatCard
          label="Input Tokens"
          value={formatNumber(totalInput)}
          icon={<IconArrowDown size={24} />}
          color="cyan"
          subLabel="Total across all cookies"
        />
        <StatCard
          label="Output Tokens"
          value={formatNumber(totalOutput)}
          icon={<IconArrowUp size={24} />}
          color="violet"
          subLabel="Total across all cookies"
        />
        <StatCard
          label={t("dashboard.tokenCost.estimatedCost")}
          value={`$${((totalInput * 3 + totalOutput * 15) / 1_000_000).toFixed(2)}`}
          icon={<IconCoins size={24} />}
          color="green"
          subLabel="Sonnet 4.5 pricing"
        />
        <Box className="glass-card" p="lg" style={{ borderRadius: 16 }}>
          <Text size="xs" c="dimmed" tt="uppercase" fw={500} mb="md">
            {t("dashboard.quotaUsage")}
          </Text>
          <Group justify="center" gap="md">
            {quotas.length > 0 ? (
              quotas.slice(0, 2).map((q, idx) => (
                <MiniQuota
                  key={idx}
                  label={q.label.split(" ").pop() || q.label}
                  percentage={q.percentage}
                  color={q.color || "cyan"}
                />
              ))
            ) : (
              <Text size="sm" c="dimmed">
                {t("dashboard.noQuotaData")}
              </Text>
            )}
          </Group>
        </Box>
      </SimpleGrid>

      {/* Main Content Grid */}
      <Grid gutter="md">
        {/* Left Column - Details */}
        <Grid.Col span={{ base: 12, lg: 4 }}>
          <Stack gap="md">
            {/* Quota Gauges */}
            <Paper p="md" radius="md" className="glass-card">
              <Text size="xs" c="dimmed" tt="uppercase" fw={500} mb="md">
                {t("dashboard.quotaUsage")}
              </Text>
              <QuotaGauge quotas={quotas} isLoading={isLoading} />
            </Paper>

            {/* Token Cost Calculator */}
            <Paper p="md" radius="md" className="glass-card">
              <Text size="xs" c="dimmed" tt="uppercase" fw={500} mb="md">
                {t("dashboard.tokenCost.title")}
              </Text>
              <TokenCostCalculator
                totalInputTokens={totalInput}
                totalOutputTokens={totalOutput}
                sonnetInputTokens={sonnetInput}
                sonnetOutputTokens={sonnetOutput}
                opusInputTokens={opusInput}
                opusOutputTokens={opusOutput}
                isLoading={isLoading}
              />
            </Paper>

            {/* OAuth Status */}
            <Paper p="md" radius="md" className="glass-card">
              <Text size="xs" c="dimmed" tt="uppercase" fw={500} mb="md">
                {t("dashboard.oauth.title")}
              </Text>
              <OAuthStatus cookies={getAllCookies()} isLoading={isLoading} />
            </Paper>

            {/* Quick Actions */}
            <Paper p="md" radius="md" className="glass-card">
              <QuickActions
                onCookieSubmitted={fetchCookieStatus}
                onRefresh={fetchCookieStatus}
              />
            </Paper>
          </Stack>
        </Grid.Col>

        {/* Right Column - Console Logs */}
        <Grid.Col span={{ base: 12, lg: 8 }}>
          <Box h={{ base: 500, lg: "calc(100vh - 380px)" }}>
            <LogsPanel />
          </Box>
        </Grid.Col>
      </Grid>
    </Stack>
  );
}

export default Dashboard;
