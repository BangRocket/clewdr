// frontend/src/components/dashboard/UsageSummary.tsx
import { useTranslation } from "react-i18next";
import { SimpleGrid, Paper, Text, Skeleton, Group, ThemeIcon } from "@mantine/core";
import {
  IconCookie,
  IconCheck,
  IconPlayerPlay,
  IconAlertTriangle,
  IconX,
} from "@tabler/icons-react";

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

interface StatCardProps {
  label: string;
  value: number;
  color: string;
  icon: React.ReactNode;
}

function StatCard({ label, value, color, icon }: StatCardProps) {
  return (
    <Paper p="md" radius="md" withBorder>
      <Group gap="xs" mb="xs">
        <ThemeIcon size="sm" variant="light" color={color} radius="xl">
          {icon}
        </ThemeIcon>
        <Text size="xs" c="dimmed" tt="uppercase" fw={500}>
          {label}
        </Text>
      </Group>
      <Text size="xl" fw={700} c={color}>
        {value}
      </Text>
    </Paper>
  );
}

export function UsageSummary({ counts, isLoading }: UsageSummaryProps) {
  const { t } = useTranslation();

  if (isLoading) {
    return (
      <SimpleGrid cols={{ base: 2, sm: 3, lg: 5 }} spacing="sm">
        {[...Array(5)].map((_, i) => (
          <Paper key={i} p="md" radius="md" withBorder>
            <Skeleton height={12} width="60%" mb="xs" />
            <Skeleton height={28} width="40%" />
          </Paper>
        ))}
      </SimpleGrid>
    );
  }

  const cards = [
    {
      label: t("dashboard.totalCookies"),
      value: counts.total,
      color: "gray",
      icon: <IconCookie size={14} />,
    },
    {
      label: t("dashboard.validCookies"),
      value: counts.valid,
      color: "green",
      icon: <IconCheck size={14} />,
    },
    {
      label: t("dashboard.inUseCookies"),
      value: counts.inUse,
      color: "cyan",
      icon: <IconPlayerPlay size={14} />,
    },
    {
      label: t("dashboard.exhaustedCookies"),
      value: counts.exhausted,
      color: "yellow",
      icon: <IconAlertTriangle size={14} />,
    },
    {
      label: t("dashboard.invalidCookies"),
      value: counts.invalid,
      color: "red",
      icon: <IconX size={14} />,
    },
  ];

  return (
    <SimpleGrid cols={{ base: 2, sm: 3, lg: 5 }} spacing="sm">
      {cards.map((card) => (
        <StatCard key={card.label} {...card} />
      ))}
    </SimpleGrid>
  );
}

export default UsageSummary;
