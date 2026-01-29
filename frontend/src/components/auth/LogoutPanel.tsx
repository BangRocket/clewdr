// frontend/src/components/auth/LogoutPanel.tsx
import { useTranslation } from "react-i18next";
import { Paper, Group, Title, Text, Button, Stack } from "@mantine/core";
import { IconLogout } from "@tabler/icons-react";

interface LogoutPanelProps {
  onLogout: () => void;
}

export function LogoutPanel({ onLogout }: LogoutPanelProps) {
  const { t } = useTranslation();

  return (
    <Paper p="lg" radius="md">
      <Stack gap="md">
        <Group justify="space-between">
          <Title order={4}>{t("auth.authTitle")}</Title>
          <Button
            onClick={onLogout}
            color="red"
            variant="light"
            leftSection={<IconLogout size={16} />}
          >
            {t("auth.logout")}
          </Button>
        </Group>
        <Text c="dimmed" size="sm">
          {t("auth.loggedInMessage")}
        </Text>
      </Stack>
    </Paper>
  );
}

export default LogoutPanel;
