// frontend/src/App.tsx
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  Center,
  Paper,
  Title,
  Text,
  Stack,
  Alert,
  Container,
} from "@mantine/core";
import { IconInfoCircle } from "@tabler/icons-react";
import AppShellLayout from "./components/layout/AppShellLayout";
import AuthGatekeeper from "./components/auth/AuthGatekeeper";
import LogoutPanel from "./components/auth/LogoutPanel";
import ClaudeTabs from "./components/claude";
import ConfigTab from "./components/config";
import LogsTab from "./components/console";
import Dashboard from "./components/dashboard";
import { useAppContext } from "./context/AppContext";

function App() {
  const { t } = useTranslation();
  const {
    version,
    isAuthenticated,
    setIsAuthenticated,
    activeTab,
    setActiveTab,
  } = useAppContext();

  const [passwordChanged, setPasswordChanged] = useState(false);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get("passwordChanged") === "true") {
      setPasswordChanged(true);
      window.history.replaceState({}, document.title, window.location.pathname);
    }
  }, []);

  const handleAuthenticated = (status: boolean) => {
    setIsAuthenticated(status);
  };

  const handleLogout = () => {
    localStorage.removeItem("authToken");
    setIsAuthenticated(false);
  };

  const renderSection = () => {
    switch (activeTab) {
      case "dashboard":
        return <Dashboard />;
      case "cookies":
        return <ClaudeTabs />;
      case "config":
        return <ConfigTab />;
      case "logs":
        return <LogsTab />;
      case "auth":
        return <LogoutPanel onLogout={handleLogout} />;
      default:
        return <Dashboard />;
    }
  };

  // Login screen
  if (!isAuthenticated) {
    return (
      <Center mih="100vh" p="md">
        <Container size="xs" w="100%">
          <Paper p="xl" radius="md" withBorder>
            <Stack gap="lg">
              <Title order={2} ta="center">
                {t("auth.title")}
              </Title>

              {passwordChanged && (
                <Alert
                  icon={<IconInfoCircle size={16} />}
                  color="blue"
                  variant="light"
                >
                  {t("auth.passwordChanged")}
                </Alert>
              )}

              <Text c="dimmed" size="sm" ta="center">
                {t("auth.description")}
              </Text>

              <AuthGatekeeper onAuthenticated={handleAuthenticated} />
            </Stack>
          </Paper>
        </Container>
      </Center>
    );
  }

  // Authenticated view with dashboard layout
  return (
    <AppShellLayout
      version={version}
      activeSection={activeTab}
      onSectionChange={setActiveTab}
    >
      {renderSection()}
    </AppShellLayout>
  );
}

export default App;
