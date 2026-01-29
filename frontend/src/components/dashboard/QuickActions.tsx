// frontend/src/components/dashboard/QuickActions.tsx
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Stack,
  Button,
  Textarea,
  Group,
  Alert,
  Text,
  Collapse,
} from "@mantine/core";
import {
  IconWorld,
  IconPlus,
  IconRefresh,
  IconCheck,
  IconX,
} from "@tabler/icons-react";
import { postCookie, getBrowserCookie, getCookieStatus } from "../../api";

interface QuickActionsProps {
  onCookieSubmitted?: () => void;
  onRefresh?: () => void;
}

export function QuickActions({ onCookieSubmitted, onRefresh }: QuickActionsProps) {
  const { t } = useTranslation();
  const [isImporting, setIsImporting] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [showCookieInput, setShowCookieInput] = useState(false);
  const [cookieValue, setCookieValue] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [message, setMessage] = useState<{
    type: "success" | "error";
    text: string;
  } | null>(null);

  const handleImportFromBrowser = async () => {
    setIsImporting(true);
    setMessage(null);

    try {
      const response = await getBrowserCookie();

      if (response.found && response.cookie) {
        await postCookie(response.cookie);
        setMessage({
          type: "success",
          text: t("cookieSubmit.browserImport.success", {
            browser: response.browser,
            profile: response.profile,
          }),
        });
        onCookieSubmitted?.();
      } else {
        setMessage({
          type: "error",
          text: response.message || t("cookieSubmit.browserImport.notFound"),
        });
      }
    } catch (e) {
      setMessage({
        type: "error",
        text: t("cookieSubmit.browserImport.error", {
          message: e instanceof Error ? e.message : "Unknown error",
        }),
      });
    } finally {
      setIsImporting(false);
    }
  };

  const handleRefresh = async () => {
    setIsRefreshing(true);
    try {
      await getCookieStatus(true);
      onRefresh?.();
    } finally {
      setIsRefreshing(false);
    }
  };

  const handleSubmitCookie = async () => {
    if (!cookieValue.trim()) return;

    setIsSubmitting(true);
    setMessage(null);

    try {
      await postCookie(cookieValue.trim());
      setMessage({ type: "success", text: t("cookieSubmit.success") });
      setCookieValue("");
      setShowCookieInput(false);
      onCookieSubmitted?.();
    } catch (e) {
      setMessage({
        type: "error",
        text: e instanceof Error ? e.message : "Failed to submit cookie",
      });
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Stack gap="sm">
      <Text size="xs" c="dimmed" tt="uppercase" fw={500}>
        {t("dashboard.quickActions")}
      </Text>

      <Stack gap="xs">
        <Button
          onClick={handleImportFromBrowser}
          loading={isImporting}
          variant="light"
          leftSection={<IconWorld size={16} />}
          justify="flex-start"
          fullWidth
        >
          {isImporting
            ? t("cookieSubmit.browserImport.importing")
            : t("cookieSubmit.browserImport.button")}
        </Button>

        {!showCookieInput ? (
          <Button
            onClick={() => setShowCookieInput(true)}
            variant="light"
            leftSection={<IconPlus size={16} />}
            justify="flex-start"
            fullWidth
          >
            {t("dashboard.submitCookie")}
          </Button>
        ) : (
          <Collapse in={showCookieInput}>
            <Stack gap="xs" p="sm" style={{ background: "var(--mantine-color-dark-6)", borderRadius: "var(--mantine-radius-md)" }}>
              <Textarea
                value={cookieValue}
                onChange={(e) => setCookieValue(e.target.value)}
                placeholder={t("cookieSubmit.placeholder")}
                minRows={3}
                autosize
              />
              <Group gap="xs">
                <Button
                  onClick={handleSubmitCookie}
                  loading={isSubmitting}
                  disabled={!cookieValue.trim()}
                  size="sm"
                  flex={1}
                >
                  {t("cookieSubmit.submitButton")}
                </Button>
                <Button
                  onClick={() => {
                    setShowCookieInput(false);
                    setCookieValue("");
                  }}
                  variant="subtle"
                  color="gray"
                  size="sm"
                >
                  {t("auth.clear")}
                </Button>
              </Group>
            </Stack>
          </Collapse>
        )}

        <Button
          onClick={handleRefresh}
          loading={isRefreshing}
          variant="light"
          leftSection={<IconRefresh size={16} />}
          justify="flex-start"
          fullWidth
        >
          {isRefreshing
            ? t("cookieStatus.refreshing")
            : t("cookieStatus.refresh")}
        </Button>
      </Stack>

      {message && (
        <Alert
          color={message.type === "success" ? "green" : "red"}
          icon={message.type === "success" ? <IconCheck size={16} /> : <IconX size={16} />}
          variant="light"
        >
          {message.text}
        </Alert>
      )}
    </Stack>
  );
}

export default QuickActions;
