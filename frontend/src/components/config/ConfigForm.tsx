// frontend/src/components/config/ConfigForm.tsx
import { useTranslation } from "react-i18next";
import {
  TextInput,
  PasswordInput,
  NumberInput,
  Checkbox,
  Textarea,
  Stack,
  SimpleGrid,
  Paper,
  Title,
  Text,
  Group,
} from "@mantine/core";
import { ConfigData } from "../../types/config.types";

interface ConfigFormProps {
  config: ConfigData;
  onChange: (
    e: React.ChangeEvent<
      HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement
    >
  ) => void;
}

interface ConfigSectionProps {
  title: string;
  description?: string;
  children: React.ReactNode;
}

function ConfigSection({ title, description, children }: ConfigSectionProps) {
  return (
    <Paper p="md" radius="md" withBorder>
      <Title order={5} c="cyan" mb="xs">
        {title}
      </Title>
      {description && (
        <Text size="xs" c="dimmed" mb="md">
          {description}
        </Text>
      )}
      <Stack gap="md">{children}</Stack>
    </Paper>
  );
}

export function ConfigForm({ config, onChange }: ConfigFormProps) {
  const { t } = useTranslation();

  const handleNumberChange = (name: string) => (value: string | number) => {
    const event = {
      target: {
        name,
        value: String(value),
        type: "number",
      },
    } as React.ChangeEvent<HTMLInputElement>;
    onChange(event);
  };

  return (
    <Stack gap="md">
      {/* Server Settings */}
      <ConfigSection
        title={t("config.sections.server.title")}
        description={t("config.sections.server.description")}
      >
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <TextInput
            label={t("config.sections.server.ip")}
            name="ip"
            value={config.ip}
            onChange={onChange}
          />
          <NumberInput
            label={t("config.sections.server.port")}
            name="port"
            value={config.port}
            onChange={handleNumberChange("port")}
            min={1}
            max={65535}
          />
        </SimpleGrid>
      </ConfigSection>

      {/* App Settings */}
      <ConfigSection title={t("config.sections.app.title")}>
        <Group gap="xl">
          <Checkbox
            label={t("config.sections.app.checkUpdate")}
            name="check_update"
            checked={config.check_update}
            onChange={onChange}
          />
          <Checkbox
            label={t("config.sections.app.autoUpdate")}
            name="auto_update"
            checked={config.auto_update}
            onChange={onChange}
          />
        </Group>
      </ConfigSection>

      {/* Network Settings */}
      <ConfigSection title={t("config.sections.network.title")}>
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <PasswordInput
            label={t("config.sections.network.password")}
            name="password"
            value={config.password}
            onChange={onChange}
          />
          <PasswordInput
            label={t("config.sections.network.adminPassword")}
            name="admin_password"
            value={config.admin_password}
            onChange={onChange}
          />
        </SimpleGrid>
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <TextInput
            label={t("config.sections.network.proxy")}
            name="proxy"
            value={config.proxy || ""}
            onChange={onChange}
            placeholder={t("config.sections.network.proxyPlaceholder")}
          />
          <TextInput
            label={t("config.sections.network.rproxy")}
            name="rproxy"
            value={config.rproxy || ""}
            onChange={onChange}
            placeholder={t("config.sections.network.rproxyPlaceholder")}
          />
        </SimpleGrid>
      </ConfigSection>

      {/* API Settings */}
      <ConfigSection title={t("config.sections.api.title")}>
        <NumberInput
          label={t("config.sections.api.maxRetries")}
          name="max_retries"
          value={config.max_retries}
          onChange={handleNumberChange("max_retries")}
          min={0}
          max={10}
          w={{ base: "100%", sm: "50%" }}
        />
        <Group gap="xl">
          <Checkbox
            label={t("config.sections.api.preserveChats")}
            name="preserve_chats"
            checked={config.preserve_chats}
            onChange={onChange}
          />
          <Checkbox
            label={t("config.sections.api.webSearch")}
            name="web_search"
            checked={config.web_search}
            onChange={onChange}
          />
          <Checkbox
            label={t("config.sections.api.webCountTokens")}
            name="enable_web_count_tokens"
            checked={!!config.enable_web_count_tokens}
            onChange={onChange}
          />
        </Group>
      </ConfigSection>

      {/* Cookie Settings */}
      <ConfigSection title={t("config.sections.cookie.title")}>
        <SimpleGrid cols={{ base: 1, sm: 2, md: 3 }}>
          <Checkbox
            label={t("config.sections.cookie.skipFree")}
            name="skip_non_pro"
            checked={config.skip_non_pro}
            onChange={onChange}
          />
          <Checkbox
            label={t("config.sections.cookie.skipRestricted")}
            name="skip_restricted"
            checked={config.skip_restricted}
            onChange={onChange}
          />
          <Checkbox
            label={t("config.sections.cookie.skipSecondWarning")}
            name="skip_second_warning"
            checked={config.skip_second_warning}
            onChange={onChange}
          />
          <Checkbox
            label={t("config.sections.cookie.skipFirstWarning")}
            name="skip_first_warning"
            checked={config.skip_first_warning}
            onChange={onChange}
          />
          <Checkbox
            label={t("config.sections.cookie.skipNormalPro")}
            name="skip_normal_pro"
            checked={config.skip_normal_pro}
            onChange={onChange}
          />
          <Checkbox
            label={t("config.sections.cookie.skipRateLimit")}
            name="skip_rate_limit"
            checked={config.skip_rate_limit}
            onChange={onChange}
          />
        </SimpleGrid>
      </ConfigSection>

      {/* Prompt Configurations */}
      <ConfigSection title={t("config.sections.prompt.title")}>
        <Checkbox
          label={t("config.sections.prompt.realRoles")}
          name="use_real_roles"
          checked={config.use_real_roles}
          onChange={onChange}
        />
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <TextInput
            label={t("config.sections.prompt.customH")}
            name="custom_h"
            value={config.custom_h || ""}
            onChange={onChange}
          />
          <TextInput
            label={t("config.sections.prompt.customA")}
            name="custom_a"
            value={config.custom_a || ""}
            onChange={onChange}
          />
        </SimpleGrid>
        <Textarea
          label={t("config.sections.prompt.customPrompt")}
          name="custom_prompt"
          value={config.custom_prompt}
          onChange={onChange}
          minRows={3}
          autosize
        />
      </ConfigSection>
    </Stack>
  );
}

export default ConfigForm;
