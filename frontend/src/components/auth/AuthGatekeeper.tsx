// frontend/src/components/auth/AuthGatekeeper.tsx
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  TextInput,
  Button,
  Stack,
  Alert,
  Text,
  Group,
  ActionIcon,
} from "@mantine/core";
import {
  IconEye,
  IconEyeOff,
  IconAlertCircle,
  IconCheck,
  IconInfoCircle,
  IconX,
} from "@tabler/icons-react";
import { useAuth } from "../../hooks/useAuth";

interface AuthGatekeeperProps {
  onAuthenticated?: (status: boolean) => void;
}

type MessageType = "success" | "error" | "warning" | "info";

interface StatusMessage {
  type: MessageType;
  message: string;
}

const getAlertColor = (type: MessageType) => {
  switch (type) {
    case "success":
      return "green";
    case "error":
      return "red";
    case "warning":
      return "yellow";
    default:
      return "blue";
  }
};

const getAlertIcon = (type: MessageType) => {
  switch (type) {
    case "success":
      return <IconCheck size={16} />;
    case "error":
      return <IconAlertCircle size={16} />;
    case "warning":
      return <IconAlertCircle size={16} />;
    default:
      return <IconInfoCircle size={16} />;
  }
};

export function AuthGatekeeper({ onAuthenticated }: AuthGatekeeperProps) {
  const { t } = useTranslation();
  const {
    authToken,
    setAuthToken,
    isLoading,
    error,
    savedToken,
    login,
    logout,
  } = useAuth(onAuthenticated);

  const [showPassword, setShowPassword] = useState(false);
  const [statusMessage, setStatusMessage] = useState<StatusMessage>({
    type: "info",
    message: "",
  });

  useEffect(() => {
    if (error) {
      setStatusMessage({
        type: "error",
        message: error,
      });
    }
  }, [error]);

  const handleSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setStatusMessage({ type: "info", message: "" });

    if (!authToken.trim()) {
      setStatusMessage({
        type: "warning",
        message: t("auth.enterToken"),
      });
      return;
    }

    try {
      await login(authToken);
      setStatusMessage({
        type: "success",
        message: t("auth.success"),
      });
    } catch {
      // Error handled in useAuth hook
    }
  };

  const handleClearToken = () => {
    logout();
    setStatusMessage({
      type: "info",
      message: t("auth.tokenCleared"),
    });
  };

  return (
    <form onSubmit={handleSubmit}>
      <Stack gap="md">
        <TextInput
          label={t("auth.token")}
          placeholder={t("auth.tokenPlaceholder")}
          type={showPassword ? "text" : "password"}
          value={authToken}
          onChange={(e) => setAuthToken(e.target.value)}
          disabled={isLoading}
          rightSection={
            <ActionIcon
              variant="subtle"
              onClick={() => setShowPassword(!showPassword)}
              tabIndex={-1}
            >
              {showPassword ? <IconEyeOff size={18} /> : <IconEye size={18} />}
            </ActionIcon>
          }
        />

        {savedToken && (
          <Group justify="space-between">
            <Text size="xs" c="dimmed">
              {t("auth.previousToken")}{" "}
              <Text component="span" ff="monospace" inherit>
                {savedToken}
              </Text>
            </Text>
            <Button
              variant="subtle"
              color="red"
              size="compact-xs"
              onClick={handleClearToken}
              disabled={isLoading}
              leftSection={<IconX size={14} />}
            >
              {t("auth.clear")}
            </Button>
          </Group>
        )}

        {statusMessage.message && (
          <Alert
            color={getAlertColor(statusMessage.type)}
            icon={getAlertIcon(statusMessage.type)}
            variant="light"
          >
            {statusMessage.message}
          </Alert>
        )}

        <Button type="submit" loading={isLoading} fullWidth>
          {isLoading ? t("auth.verifying") : t("auth.submitButton")}
        </Button>
      </Stack>
    </form>
  );
}

export default AuthGatekeeper;
