import React from "react";
import { useTranslation } from "react-i18next";
import { Paper, Group, Text, Badge, Stack, Divider, Box } from "@mantine/core";
import type { CookieItem } from "../../types/cookie.types";

interface CookieSectionProps {
  title: string;
  cookies: CookieItem[];
  color: "green" | "yellow" | "red" | "cyan" | "violet";
  renderStatus: (item: CookieItem, index: number) => React.ReactNode;
}

const CookieSection: React.FC<CookieSectionProps> = ({
  title,
  cookies,
  color,
  renderStatus,
}) => {
  const { t } = useTranslation();
  // sort cookie base on reset_time
  const sortedCookies = [...cookies].sort((a, b) => {
    const aTime = a.reset_time ? new Date(a.reset_time).getTime() : 0;
    const bTime = b.reset_time ? new Date(b.reset_time).getTime() : 0;
    return aTime - bTime;
  });

  return (
    <Paper radius="md" className="glass-card" style={{ overflow: "hidden" }}>
      <Box
        p="sm"
        style={{
          borderBottom: `1px solid var(--mantine-color-${color}-9)`,
          background: `var(--mantine-color-${color}-9)`,
        }}
      >
        <Group justify="space-between">
          <Text fw={500} c={`${color}.2`}>
            {title}
          </Text>
          <Badge color={color} variant="filled" size="sm">
            {cookies.length}
          </Badge>
        </Group>
      </Box>
      {sortedCookies.length > 0 ? (
        <Stack gap={0} p="md">
          {sortedCookies.map((item, index) => (
            <React.Fragment key={index}>
              {index > 0 && <Divider my="xs" color="dark.5" />}
              {renderStatus(item, index)}
            </React.Fragment>
          ))}
        </Stack>
      ) : (
        <Box p="md">
          <Text size="sm" c="dimmed" fs="italic">
            {t("cookieStatus.noCookies", { type: title.toLowerCase() })}
          </Text>
        </Box>
      )}
    </Paper>
  );
};

export default CookieSection;
