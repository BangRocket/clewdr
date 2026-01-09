// frontend/src/App.tsx
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import "./App.css";
import DashboardLayout from "./components/layout/DashboardLayout";
import Card from "./components/common/Card";
import AuthGatekeeper from "./components/auth/AuthGatekeeper";
import LogoutPanel from "./components/auth/LogoutPanel";
import ClaudeTabs from "./components/claude";
import ConfigTab from "./components/config";
import LogsTab from "./components/console";
import Dashboard from "./components/dashboard";
import StatusMessage from "./components/common/StatusMessage";
import ErrorBoundary from "./components/common/ErrorBoundary";
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
    // Check if redirected due to password change
    const params = new URLSearchParams(window.location.search);
    if (params.get("passwordChanged") === "true") {
      setPasswordChanged(true);
      // Clean up the URL
      window.history.replaceState({}, document.title, window.location.pathname);
    }
  }, []);

  // Function to handle successful authentication
  const handleAuthenticated = (status: boolean) => {
    setIsAuthenticated(status);
  };

  // Function to handle logout
  const handleLogout = () => {
    localStorage.removeItem("authToken");
    setIsAuthenticated(false);
  };

  // Render section based on active section
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

  // If not authenticated, show login screen
  if (!isAuthenticated) {
    return (
      <ErrorBoundary>
        <div className="min-h-screen bg-gradient-to-b from-gray-900 to-gray-800 text-white flex items-center justify-center p-4">
          <Card className="w-full max-w-md">
            <h2 className="text-xl font-semibold text-center mb-6">
              {t("auth.title")}
            </h2>

            {passwordChanged && (
              <StatusMessage type="info" message={t("auth.passwordChanged")} />
            )}

            <p className="text-gray-400 text-sm mb-6 text-center">
              {t("auth.description")}
            </p>

            <ErrorBoundary>
              <AuthGatekeeper onAuthenticated={handleAuthenticated} />
            </ErrorBoundary>
          </Card>
        </div>
      </ErrorBoundary>
    );
  }

  // Authenticated view with dashboard layout
  return (
    <ErrorBoundary>
      <DashboardLayout
        version={version}
        activeSection={activeTab}
        onSectionChange={setActiveTab}
      >
        <ErrorBoundary>{renderSection()}</ErrorBoundary>
      </DashboardLayout>
    </ErrorBoundary>
  );
}

export default App;
