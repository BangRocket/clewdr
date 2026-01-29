// frontend/src/components/claude/CookieSubmitForm.tsx
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Textarea,
  Button,
  Alert,
  Stack,
  Text,
  Group,
  Paper,
  ScrollArea,
  ThemeIcon,
} from "@mantine/core";
import {
  IconWorld,
  IconCheck,
  IconX,
  IconAlertCircle,
  IconInfoCircle,
} from "@tabler/icons-react";
import { postCookie, getBrowserCookie } from "../../api";

interface CookieResult {
  cookie: string;
  status: "success" | "error";
  message: string;
}

interface CookieSubmitFormProps {
  onSuccess?: () => void;
}

export function CookieSubmitForm({ onSuccess }: CookieSubmitFormProps) {
  const { t } = useTranslation();
  const [cookies, setCookies] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [results, setResults] = useState<CookieResult[]>([]);
  const [overallStatus, setOverallStatus] = useState<{
    type: "info" | "success" | "error" | "warning";
    message: string;
  }>({ type: "info", message: "" });

  const handleImportFromBrowser = async () => {
    setIsImporting(true);
    setOverallStatus({ type: "info", message: "" });

    try {
      const response = await getBrowserCookie();

      if (response.found && response.cookie) {
        setCookies(response.cookie);
        setOverallStatus({
          type: "success",
          message: t("cookieSubmit.browserImport.success", {
            browser: response.browser,
            profile: response.profile,
          }),
        });
      } else {
        const errorMsgs =
          response.errors?.length > 0
            ? response.errors.join("; ")
            : response.message || t("cookieSubmit.browserImport.notFound");
        setOverallStatus({ type: "warning", message: errorMsgs });
      }
    } catch (e) {
      const errorMessage = e instanceof Error ? e.message : "Unknown error";
      setOverallStatus({
        type: "error",
        message: t("cookieSubmit.browserImport.error", { message: errorMessage }),
      });
    } finally {
      setIsImporting(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();

    const cookieLines = cookies
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0);

    if (cookieLines.length === 0) {
      setOverallStatus({ type: "error", message: t("cookieSubmit.error.empty") });
      return;
    }

    setIsSubmitting(true);
    setOverallStatus({ type: "info", message: "" });
    setResults([]);

    const newResults: CookieResult[] = [];
    let successCount = 0;
    let errorCount = 0;

    for (const cookieStr of cookieLines) {
      try {
        await postCookie(cookieStr);
        newResults.push({
          cookie: cookieStr,
          status: "success",
          message: t("cookieSubmit.success"),
        });
        successCount++;
      } catch (e) {
        const errorMessage = e instanceof Error ? e.message : "Unknown error";
        let translatedError = errorMessage;

        if (errorMessage.includes("Invalid cookie format")) {
          translatedError = t("cookieSubmit.error.format");
        } else if (errorMessage.includes("Authentication failed")) {
          translatedError = t("cookieSubmit.error.auth");
        } else if (errorMessage.includes("Server error")) {
          translatedError = t("cookieSubmit.error.server");
        }

        newResults.push({ cookie: cookieStr, status: "error", message: translatedError });
        errorCount++;
      }
    }

    setResults(newResults);

    if (errorCount === 0) {
      setOverallStatus({
        type: "success",
        message: t("cookieSubmit.allSuccess", { count: successCount }),
      });
      setCookies("");
      // Call onSuccess after a brief delay to show the success message
      if (onSuccess) {
        setTimeout(onSuccess, 1500);
      }
    } else if (successCount === 0) {
      setOverallStatus({
        type: "error",
        message: t("cookieSubmit.allFailed", { count: errorCount }),
      });
    } else {
      setOverallStatus({
        type: "warning",
        message: t("cookieSubmit.partialSuccess", {
          successCount,
          errorCount,
          total: successCount + errorCount,
        }),
      });
    }

    setIsSubmitting(false);
  };

  const getAlertColor = (type: string) => {
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

  const getAlertIcon = (type: string) => {
    switch (type) {
      case "success":
        return <IconCheck size={16} />;
      case "error":
        return <IconX size={16} />;
      case "warning":
        return <IconAlertCircle size={16} />;
      default:
        return <IconInfoCircle size={16} />;
    }
  };

  return (
    <form onSubmit={handleSubmit}>
      <Stack gap="md">
        <Textarea
          label={t("cookieSubmit.value")}
          placeholder={t("cookieSubmit.placeholderMulti")}
          value={cookies}
          onChange={(e) => setCookies(e.target.value)}
          disabled={isSubmitting}
          minRows={5}
          autosize
        />

        <Group justify="space-between" align="center">
          <Text size="xs" c="dimmed">
            {t("cookieSubmit.descriptionMulti")}
          </Text>
          <Button
            variant="light"
            size="xs"
            onClick={handleImportFromBrowser}
            loading={isImporting}
            disabled={isSubmitting}
            leftSection={<IconWorld size={14} />}
          >
            {isImporting
              ? t("cookieSubmit.browserImport.importing")
              : t("cookieSubmit.browserImport.button")}
          </Button>
        </Group>

        {overallStatus.message && (
          <Alert
            color={getAlertColor(overallStatus.type)}
            icon={getAlertIcon(overallStatus.type)}
            variant="light"
          >
            {overallStatus.message}
          </Alert>
        )}

        {results.length > 0 && (
          <Paper p="sm" radius="md" withBorder>
            <Text size="sm" fw={500} mb="sm">
              {t("cookieSubmit.resultDetails")}:
            </Text>
            <ScrollArea.Autosize mah={200}>
              <Stack gap="xs">
                {results.map((result, index) => (
                  <Paper
                    key={index}
                    p="xs"
                    radius="sm"
                    withBorder
                    style={{
                      borderColor:
                        result.status === "success"
                          ? "var(--mantine-color-green-7)"
                          : "var(--mantine-color-red-7)",
                      background:
                        result.status === "success"
                          ? "var(--mantine-color-green-9)"
                          : "var(--mantine-color-red-9)",
                    }}
                  >
                    <Group gap="xs" wrap="nowrap">
                      <ThemeIcon
                        size="sm"
                        radius="xl"
                        variant="filled"
                        color={result.status === "success" ? "green" : "red"}
                      >
                        {result.status === "success" ? (
                          <IconCheck size={12} />
                        ) : (
                          <IconX size={12} />
                        )}
                      </ThemeIcon>
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <Text size="xs" ff="monospace" truncate>
                          {result.cookie.substring(0, 30)}
                          {result.cookie.length > 30 ? "..." : ""}
                        </Text>
                        <Text
                          size="xs"
                          c={result.status === "success" ? "green" : "red"}
                        >
                          {result.message}
                        </Text>
                      </div>
                    </Group>
                  </Paper>
                ))}
              </Stack>
            </ScrollArea.Autosize>
          </Paper>
        )}

        <Button type="submit" loading={isSubmitting} fullWidth>
          {isSubmitting ? t("cookieSubmit.submitting") : t("cookieSubmit.submitButton")}
        </Button>
      </Stack>
    </form>
  );
}

export default CookieSubmitForm;
