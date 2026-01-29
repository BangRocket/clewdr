// frontend/src/components/claude/index.tsx
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Tabs } from "@mantine/core";
import { IconUpload, IconCookie } from "@tabler/icons-react";
import CookieSubmitForm from "./CookieSubmitForm";
import CookieVisualization from "./CookieVisualization";

export function ClaudeTabs() {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<string | null>("submit");

  return (
    <Tabs value={activeTab} onChange={setActiveTab}>
      <Tabs.List>
        <Tabs.Tab value="submit" leftSection={<IconUpload size={16} />}>
          {t("claudeTab.submit")}
        </Tabs.Tab>
        <Tabs.Tab value="status" leftSection={<IconCookie size={16} />}>
          {t("claudeTab.status")}
        </Tabs.Tab>
      </Tabs.List>

      <Tabs.Panel value="submit" pt="md">
        <CookieSubmitForm />
      </Tabs.Panel>

      <Tabs.Panel value="status" pt="md">
        <CookieVisualization />
      </Tabs.Panel>
    </Tabs>
  );
}

export default ClaudeTabs;
