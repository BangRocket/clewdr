// frontend/src/components/dashboard/QuotaGauge.tsx
import { useTranslation } from "react-i18next";
import { Stack, Text, Progress, Skeleton, Group } from "@mantine/core";

interface QuotaData {
  label: string;
  percentage: number;
  resetsAt?: string;
  color?: "cyan" | "green" | "yellow" | "red" | "violet";
}

interface QuotaGaugeProps {
  quotas: QuotaData[];
  isLoading?: boolean;
}

function getProgressColor(percentage: number, explicitColor?: string): string {
  if (explicitColor) return explicitColor;
  if (percentage >= 90) return "red";
  if (percentage >= 70) return "yellow";
  return "cyan";
}

export function QuotaGauge({ quotas, isLoading }: QuotaGaugeProps) {
  const { t } = useTranslation();

  if (isLoading) {
    return (
      <Stack gap="md">
        {[...Array(2)].map((_, i) => (
          <div key={i}>
            <Skeleton height={16} width="40%" mb="xs" />
            <Skeleton height={8} width="100%" radius="xl" />
          </div>
        ))}
      </Stack>
    );
  }

  if (quotas.length === 0) {
    return (
      <Text size="sm" c="dimmed" ta="center" py="lg">
        {t("dashboard.noQuotaData")}
      </Text>
    );
  }

  return (
    <Stack gap="md">
      {quotas.map((quota, index) => {
        const clampedPercentage = Math.min(100, Math.max(0, quota.percentage));
        const color = getProgressColor(quota.percentage, quota.color);

        return (
          <div key={index}>
            <Group justify="space-between" mb={4}>
              <Text size="sm" c="dimmed">
                {quota.label}
              </Text>
              <Text size="sm" fw={500} c={color}>
                {quota.percentage.toFixed(1)}%
              </Text>
            </Group>
            <Progress
              value={clampedPercentage}
              color={color}
              size="md"
              radius="xl"
              animated={clampedPercentage > 0 && clampedPercentage < 100}
            />
            {quota.resetsAt && (
              <Text size="xs" c="dimmed" mt={4}>
                {t("cookieStatus.quota.resetsAt", { time: quota.resetsAt })}
              </Text>
            )}
          </div>
        );
      })}
    </Stack>
  );
}

export default QuotaGauge;
