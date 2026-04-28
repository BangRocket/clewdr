import React, { useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import TabNavigation from "../common/TabNavigation";
import AddCodexAuthForm from "./AddCodexAuthForm";
import CodexAuthList from "./CodexAuthList";

const CodexTabs: React.FC = () => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<"add" | "list">("add");
  const [refreshKey, setRefreshKey] = useState(0);

  const handleAdded = useCallback(() => {
    setRefreshKey((k) => k + 1);
    setActiveTab("list");
  }, []);

  const tabs = [
    { id: "add", label: t("codexTab.add"), color: "blue" },
    { id: "list", label: t("codexTab.list"), color: "amber" },
  ];

  return (
    <div className="w-full">
      <TabNavigation
        tabs={tabs}
        activeTab={activeTab}
        onTabChange={(tabId) => setActiveTab(tabId as "add" | "list")}
        className="mb-6"
      />
      {activeTab === "add" ? (
        <AddCodexAuthForm onAdded={handleAdded} />
      ) : (
        <CodexAuthList key={refreshKey} />
      )}
    </div>
  );
};

export default CodexTabs;
