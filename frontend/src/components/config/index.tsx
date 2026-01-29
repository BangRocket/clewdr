// frontend/src/components/config/index.tsx
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  Paper,
  Title,
  Button,
  Alert,
  Stack,
  Group,
  Loader,
  Center,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconCheck, IconX, IconRefresh, IconDeviceFloppy } from "@tabler/icons-react";
import { getConfig, saveConfig } from "../../api";
import { ConfigData } from "../../types/config.types";
import ConfigForm from "./ConfigForm";

export function ConfigTab() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<ConfigData | null>(null);
  const [originalPassword, setOriginalPassword] = useState<string>("");
  const [originalAdminPassword, setOriginalAdminPassword] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    fetchConfig();
  }, []);

  const fetchConfig = async () => {
    setLoading(true);
    setError("");
    try {
      const data = await getConfig();
      setConfig(data);
      setOriginalPassword(data.password || "");
      setOriginalAdminPassword(data.admin_password || "");
    } catch (err) {
      setError(
        t("common.error", {
          message: err instanceof Error ? err.message : String(err),
        })
      );
      console.error("Config fetch error:", err);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!config) return;

    setSaving(true);
    setError("");
    try {
      await saveConfig(config);
      notifications.show({
        title: t("config.title"),
        message: t("config.success"),
        color: "green",
        icon: <IconCheck size={16} />,
      });

      const adminPasswordChanged =
        config.admin_password !== originalAdminPassword;
      const regularPasswordChanged = config.password !== originalPassword;

      if (regularPasswordChanged) {
        notifications.show({
          title: t("config.title"),
          message: t("config.passwordChanged"),
          color: "blue",
        });
      }

      if (adminPasswordChanged) {
        notifications.show({
          title: t("config.title"),
          message: t("config.adminPasswordChanged"),
          color: "yellow",
        });

        setTimeout(() => {
          localStorage.removeItem("authToken");
          window.location.href = "/?passwordChanged=true";
        }, 3000);
      }
    } catch (err) {
      setError(
        t("common.error", {
          message: err instanceof Error ? err.message : String(err),
        })
      );
      console.error("Config save error:", err);
      notifications.show({
        title: t("config.title"),
        message: t("config.error"),
        color: "red",
        icon: <IconX size={16} />,
      });
    } finally {
      setSaving(false);
    }
  };

  const handleChange = (
    e: React.ChangeEvent<
      HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement
    >
  ) => {
    if (!config) return;

    const { name, value, type } = e.target;

    if (type === "checkbox") {
      const checked = (e.target as HTMLInputElement).checked;
      setConfig({ ...config, [name]: checked });
      return;
    }

    if (type === "number") {
      setConfig({
        ...config,
        [name]: value === "" ? 0 : Number(value),
      });
      return;
    }

    if (
      ["proxy", "rproxy", "custom_h", "custom_a"].includes(name) &&
      value === ""
    ) {
      setConfig({ ...config, [name]: null });
      return;
    }

    setConfig({ ...config, [name]: value });
  };

  if (loading) {
    return (
      <Center py="xl">
        <Loader size="md" />
      </Center>
    );
  }

  if (error) {
    return (
      <Alert
        color="red"
        title={t("common.error")}
        icon={<IconX size={16} />}
        withCloseButton
        onClose={() => setError("")}
      >
        {error}
        <Button
          onClick={fetchConfig}
          variant="light"
          color="red"
          size="xs"
          mt="sm"
          leftSection={<IconRefresh size={14} />}
        >
          {t("config.retry")}
        </Button>
      </Alert>
    );
  }

  if (!config) {
    return (
      <Alert color="yellow" title={t("config.noData")}>
        {t("config.noData")}
      </Alert>
    );
  }

  return (
    <Stack gap="md">
      <Group justify="space-between">
        <Title order={3}>{t("config.title")}</Title>
        <Button
          onClick={handleSave}
          loading={saving}
          leftSection={<IconDeviceFloppy size={16} />}
          variant="gradient"
          gradient={{ from: "cyan", to: "violet", deg: 90 }}
        >
          {saving ? t("config.saving") : t("config.saveButton")}
        </Button>
      </Group>

      <Paper p="md" radius="md" withBorder>
        <ConfigForm config={config} onChange={handleChange} />
      </Paper>
    </Stack>
  );
}

export default ConfigTab;
