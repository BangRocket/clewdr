// frontend/src/components/dashboard/OAuthStatus.tsx
import { useTranslation } from "react-i18next";
import {
  Stack,
  Group,
  Text,
  Badge,
  Skeleton,
  ThemeIcon,
  Paper,
} from "@mantine/core";
import {
  IconShieldCheck,
  IconShieldOff,
  IconAlertTriangle,
  IconLock,
} from "@tabler/icons-react";

interface TokenInfo {
  expires_at: string;
  organization: {
    uuid: string;
  };
}

interface Cookie {
  cookie: string;
  token?: TokenInfo;
}

interface OAuthStatusProps {
  cookies: Cookie[];
  isLoading?: boolean;
}

export function OAuthStatus({ cookies, isLoading }: OAuthStatusProps) {
  const { t } = useTranslation();

  if (isLoading) {
    return (
      <Stack gap="sm">
        <Skeleton height={20} width="50%" />
        <Skeleton height={16} width="30%" />
      </Stack>
    );
  }

  const tokensCount = cookies.filter((c) => c.token).length;
  const expiredCount = cookies.filter((c) => {
    if (!c.token) return false;
    const expiresAt = new Date(c.token.expires_at);
    return expiresAt < new Date();
  }).length;
  const validCount = tokensCount - expiredCount;

  const getStatusInfo = () => {
    if (tokensCount === 0) {
      return {
        icon: <IconLock size={18} />,
        color: "gray",
        text: t("dashboard.oauth.noTokens"),
      };
    }
    if (expiredCount === tokensCount) {
      return {
        icon: <IconShieldOff size={18} />,
        color: "red",
        text: t("dashboard.oauth.allExpired"),
      };
    }
    if (expiredCount > 0) {
      return {
        icon: <IconAlertTriangle size={18} />,
        color: "yellow",
        text: t("dashboard.oauth.someExpired", { valid: validCount, expired: expiredCount }),
      };
    }
    return {
      icon: <IconShieldCheck size={18} />,
      color: "green",
      text: t("dashboard.oauth.allValid", { count: validCount }),
    };
  };

  const statusInfo = getStatusInfo();

  return (
    <Stack gap="sm">
      <Group gap="xs">
        <ThemeIcon
          size="md"
          variant="light"
          color={statusInfo.color}
          radius="xl"
        >
          {statusInfo.icon}
        </ThemeIcon>
        <Text size="sm" fw={500} c={statusInfo.color}>
          {statusInfo.text}
        </Text>
      </Group>

      {tokensCount > 0 && (
        <Text size="xs" c="dimmed">
          {t("dashboard.oauth.totalTokens", { count: tokensCount })}
        </Text>
      )}

      {/* Token details */}
      <Stack gap="xs">
        {cookies
          .filter((c) => c.token)
          .slice(0, 3)
          .map((cookie, index) => {
            if (!cookie.token) return null;
            const expiresAt = new Date(cookie.token.expires_at);
            const isExpired = expiresAt < new Date();
            const timeUntilExpiry = expiresAt.getTime() - Date.now();
            const hoursUntilExpiry = Math.floor(
              timeUntilExpiry / (1000 * 60 * 60)
            );
            const minutesUntilExpiry = Math.floor(
              (timeUntilExpiry % (1000 * 60 * 60)) / (1000 * 60)
            );

            return (
              <Paper
                key={index}
                p="xs"
                radius="sm"
                withBorder
                style={{
                  borderColor: isExpired
                    ? "var(--mantine-color-red-7)"
                    : "var(--mantine-color-green-7)",
                  background: isExpired
                    ? "var(--mantine-color-red-9)"
                    : "var(--mantine-color-green-9)",
                }}
              >
                <Group justify="space-between">
                  <Text size="xs" ff="monospace" c={isExpired ? "red" : "green"}>
                    {cookie.cookie.substring(0, 10)}...
                  </Text>
                  <Badge
                    size="xs"
                    variant="light"
                    color={isExpired ? "red" : "green"}
                  >
                    {isExpired
                      ? t("dashboard.oauth.expired")
                      : hoursUntilExpiry > 0
                      ? t("dashboard.oauth.expiresIn", {
                          hours: hoursUntilExpiry,
                          minutes: minutesUntilExpiry,
                        })
                      : t("dashboard.oauth.expiresMinutes", {
                          minutes: minutesUntilExpiry,
                        })}
                  </Badge>
                </Group>
              </Paper>
            );
          })}
      </Stack>

      {tokensCount > 3 && (
        <Text size="xs" c="dimmed" ta="center">
          {t("dashboard.oauth.andMore", { count: tokensCount - 3 })}
        </Text>
      )}
    </Stack>
  );
}

export default OAuthStatus;
