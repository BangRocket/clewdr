import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  Stack,
  Group,
  Text,
  Button,
  Alert,
  Loader,
  Center,
  Badge,
  Box,
  Title,
  Tooltip,
} from "@mantine/core";
import { IconRefresh, IconX } from "@tabler/icons-react";
import { getCookieStatus, deleteCookie } from "../../api";
import { formatTimestamp, formatIsoTimestamp } from "../../utils/formatters";
import { CookieStatusInfo, CookieItem } from "../../types/cookie.types";
import CookieSection from "./CookieSection";
import CookieValue from "./CookieValue";
import DeleteButton from "./DeleteButton";

// Default empty state
const emptyCookieStatus: CookieStatusInfo = {
  valid: [],
  exhausted: [],
  invalid: [],
};

const CookieVisualization: React.FC = () => {
  const { t } = useTranslation();
  const [cookieStatus, setCookieStatus] =
    useState<CookieStatusInfo>(emptyCookieStatus);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshCounter, setRefreshCounter] = useState(0);
  const [deletingCookie, setDeletingCookie] = useState<string | null>(null);
  const [isForceRefreshing, setIsForceRefreshing] = useState(false);

  // Fetch cookie data
  const fetchCookieStatus = useCallback(async (forceRefresh = false) => {
    setLoading(true);
    setError(null);
    if (forceRefresh) {
      setIsForceRefreshing(true);
    }

    try {
      const response = await getCookieStatus(forceRefresh);
      const safeData: CookieStatusInfo = {
        valid: Array.isArray(response.data?.valid) ? response.data.valid : [],
        exhausted: Array.isArray(response.data?.exhausted)
          ? response.data.exhausted
          : [],
        invalid: Array.isArray(response.data?.invalid)
          ? response.data.invalid
          : [],
      };
      setCookieStatus(safeData);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      setCookieStatus(emptyCookieStatus);
    } finally {
      setLoading(false);
      setIsForceRefreshing(false);
    }
  }, []);

  useEffect(() => {
    fetchCookieStatus();
  }, [fetchCookieStatus, refreshCounter]);

  const handleRefresh = (event?: React.MouseEvent<HTMLButtonElement>) => {
    const forceRefresh = event ? event.ctrlKey || event.metaKey : false;
    if (forceRefresh) {
      fetchCookieStatus(true);
    } else {
      setRefreshCounter((prev) => prev + 1);
    }
  };

  const getCooldownDisplay = (status: CookieItem) => {
    if (status.reset_time) {
      return {
        label: t("cookieStatus.status.cooldownFull") as string,
        time: formatTimestamp(status.reset_time),
      };
    }

    if (status.seven_day_opus_resets_at) {
      return {
        label: t("cookieStatus.status.cooldownOpus") as string,
        time: formatIsoTimestamp(status.seven_day_opus_resets_at),
      };
    }

    if (status.seven_day_sonnet_resets_at) {
      return {
        label: t("cookieStatus.status.cooldownSonnet") as string,
        time: formatIsoTimestamp(status.seven_day_sonnet_resets_at),
      };
    }

    if (status.seven_day_resets_at) {
      return {
        label: t("cookieStatus.status.cooldownFull") as string,
        time: formatIsoTimestamp(status.seven_day_resets_at),
      };
    }

    return null;
  };

  const renderUsageStats = (status: CookieItem) => {
    const s = status.session_usage || {};
    const w = status.weekly_usage || {};
    const ws = status.weekly_sonnet_usage || {};
    const wo = status.weekly_opus_usage || {};
    const lt = status.lifetime_usage || {};

    const groups: Array<{
      title: string;
      b: {
        total_input_tokens: number;
        total_output_tokens: number;
        sonnet_input_tokens: number;
        sonnet_output_tokens: number;
        opus_input_tokens: number;
        opus_output_tokens: number;
      };
      showSonnet: boolean;
      showOpus: boolean;
    }> = [];

    const toReq = (x: typeof s) => ({
      total_input_tokens: x.total_input_tokens ?? 0,
      total_output_tokens: x.total_output_tokens ?? 0,
      sonnet_input_tokens: x.sonnet_input_tokens ?? 0,
      sonnet_output_tokens: x.sonnet_output_tokens ?? 0,
      opus_input_tokens: x.opus_input_tokens ?? 0,
      opus_output_tokens: x.opus_output_tokens ?? 0,
    });

    const sReq = toReq(s);
    const wReq = toReq(w);
    const wsReq = toReq(ws);
    const woReq = toReq(wo);
    const ltReq = toReq(lt);

    const anyNonZero = (req: typeof sReq) =>
      req.total_input_tokens > 0 ||
      req.total_output_tokens > 0 ||
      req.sonnet_input_tokens > 0 ||
      req.sonnet_output_tokens > 0 ||
      req.opus_input_tokens > 0 ||
      req.opus_output_tokens > 0;

    if (anyNonZero(sReq)) {
      groups.push({
        title: t("cookieStatus.quota.session") as string,
        b: sReq,
        showSonnet:
          sReq.sonnet_input_tokens > 0 || sReq.sonnet_output_tokens > 0,
        showOpus: sReq.opus_input_tokens > 0 || sReq.opus_output_tokens > 0,
      });
    }
    if (anyNonZero(wReq)) {
      groups.push({
        title: t("cookieStatus.quota.sevenDay") as string,
        b: wReq,
        showSonnet:
          wReq.sonnet_input_tokens > 0 || wReq.sonnet_output_tokens > 0,
        showOpus: wReq.opus_input_tokens > 0 || wReq.opus_output_tokens > 0,
      });
    }
    if (anyNonZero(wsReq)) {
      groups.push({
        title: t("cookieStatus.quota.sevenDaySonnet") as string,
        b: wsReq,
        showSonnet: true,
        showOpus: wsReq.opus_input_tokens > 0 || wsReq.opus_output_tokens > 0,
      });
    }
    if (anyNonZero(woReq)) {
      groups.push({
        title: t("cookieStatus.quota.sevenDayOpus") as string,
        b: woReq,
        showSonnet:
          woReq.sonnet_input_tokens > 0 || woReq.sonnet_output_tokens > 0,
        showOpus: woReq.opus_input_tokens > 0 || woReq.opus_output_tokens > 0,
      });
    }
    if (anyNonZero(ltReq)) {
      groups.push({
        title: t("cookieStatus.quota.total") as string,
        b: ltReq,
        showSonnet:
          ltReq.sonnet_input_tokens > 0 || ltReq.sonnet_output_tokens > 0,
        showOpus: ltReq.opus_input_tokens > 0 || ltReq.opus_output_tokens > 0,
      });
    }

    if (groups.length === 0) return null;

    return (
      <Stack gap="xs" mt="xs">
        {groups.map(({ title, b, showSonnet, showOpus }, idx) => (
          <Box key={idx}>
            <Group gap="md" wrap="wrap">
              <Text size="xs" c="dimmed">
                {title} · {t("cookieStatus.usage.totalInput")}: {b.total_input_tokens}
              </Text>
              <Text size="xs" c="dimmed">
                {t("cookieStatus.usage.totalOutput")}: {b.total_output_tokens}
              </Text>
            </Group>
            {showSonnet && (
              <Group gap="md" wrap="wrap" ml="sm">
                <Text size="xs" c="dimmed">
                  {t("cookieStatus.usage.sonnetInput")}: {b.sonnet_input_tokens}
                </Text>
                <Text size="xs" c="dimmed">
                  {t("cookieStatus.usage.sonnetOutput")}: {b.sonnet_output_tokens}
                </Text>
              </Group>
            )}
            {showOpus && (
              <Group gap="md" wrap="wrap" ml="sm">
                <Text size="xs" c="dimmed">
                  {t("cookieStatus.usage.opusInput")}: {b.opus_input_tokens}
                </Text>
                <Text size="xs" c="dimmed">
                  {t("cookieStatus.usage.opusOutput")}: {b.opus_output_tokens}
                </Text>
              </Group>
            )}
          </Box>
        ))}
      </Stack>
    );
  };

  const renderQuotaStats = (status: CookieItem) => {
    const sess = status.session_utilization;
    const seven = status.seven_day_utilization;
    const sevenSonnet = status.seven_day_sonnet_utilization;
    const opus = status.seven_day_opus_utilization;
    const hasAny =
      typeof sess === "number" ||
      typeof seven === "number" ||
      typeof opus === "number" ||
      typeof sevenSonnet === "number";
    if (!hasAny) return null;

    return (
      <Stack gap={4} mt="xs">
        {typeof sess === "number" && (
          <Text size="xs" c="dimmed">
            {t("cookieStatus.quota.session")}: {sess}%
            {status.session_resets_at && (
              <Text span c="dimmed" ml="xs">
                {t("cookieStatus.quota.resetsAt", {
                  time: formatIsoTimestamp(status.session_resets_at),
                })}
              </Text>
            )}
          </Text>
        )}
        {typeof seven === "number" && (
          <Text size="xs" c="dimmed">
            {t("cookieStatus.quota.sevenDay")}: {seven}%
            {status.seven_day_resets_at && (
              <Text span c="dimmed" ml="xs">
                {t("cookieStatus.quota.resetsAt", {
                  time: formatIsoTimestamp(status.seven_day_resets_at),
                })}
              </Text>
            )}
          </Text>
        )}
        {typeof sevenSonnet === "number" && (
          <Text size="xs" c="dimmed">
            {t("cookieStatus.quota.sevenDaySonnet")}: {sevenSonnet}%
            {status.seven_day_sonnet_resets_at && (
              <Text span c="dimmed" ml="xs">
                {t("cookieStatus.quota.resetsAt", {
                  time: formatIsoTimestamp(status.seven_day_sonnet_resets_at),
                })}
              </Text>
            )}
          </Text>
        )}
        {typeof opus === "number" && (
          <Text size="xs" c="dimmed">
            {t("cookieStatus.quota.sevenDayOpus")}: {opus}%
            {status.seven_day_opus_resets_at && (
              <Text span c="dimmed" ml="xs">
                {t("cookieStatus.quota.resetsAt", {
                  time: formatIsoTimestamp(status.seven_day_opus_resets_at),
                })}
              </Text>
            )}
          </Text>
        )}
      </Stack>
    );
  };

  const handleDeleteCookie = async (cookie: string) => {
    if (!window.confirm(t("cookieStatus.deleteConfirm"))) return;

    setDeletingCookie(cookie);
    setError(null);

    try {
      const response = await deleteCookie(cookie);

      if (response.ok) {
        handleRefresh();
      } else {
        const errorMessage =
          response.status === 401
            ? t("cookieSubmit.error.auth")
            : await response
                .json()
                .then(
                  (data) =>
                    data.error ||
                    t("common.error", { message: response.status })
                );
        setError(errorMessage);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setDeletingCookie(null);
    }
  };

  // Helper for getting reason text from cookie reason object
  const getReasonText = (reason: unknown): string => {
    if (!reason) return t("cookieStatus.status.reasons.unknown");
    if (typeof reason === "string") return reason;

    try {
      if (typeof reason === "object" && reason !== null) {
        const r = reason as Record<string, unknown>;
        if ("Free" in r) return t("cookieStatus.status.reasons.freAccount");
        if ("Disabled" in r) return t("cookieStatus.status.reasons.disabled");
        if ("Banned" in r) return t("cookieStatus.status.reasons.banned");
        if ("Null" in r) return t("cookieStatus.status.reasons.invalid");
        if ("Restricted" in r && typeof r["Restricted"] === "number")
          return t("cookieStatus.status.reasons.restricted", {
            time: formatTimestamp(r["Restricted"] as number),
          });
        if ("TooManyRequest" in r && typeof r["TooManyRequest"] === "number")
          return t("cookieStatus.status.reasons.rateLimited", {
            time: formatTimestamp(r["TooManyRequest"] as number),
          });
      }
    } catch (e) {
      console.error("Error parsing reason:", e, reason);
    }
    return t("cookieStatus.status.reasons.unknown");
  };

  // Calculate total cookie count
  const totalCookies =
    cookieStatus.valid.length +
    cookieStatus.exhausted.length +
    cookieStatus.invalid.length;

  const renderContextBadge = (flag: boolean | null | undefined) => {
    if (flag === undefined) {
      return null;
    }

    if (flag === true) {
      return (
        <Badge color="green" variant="light" size="xs">
          {t("cookieStatus.context.enabled")}
        </Badge>
      );
    }
    if (flag === false) {
      return (
        <Badge color="red" variant="light" size="xs">
          {t("cookieStatus.context.disabled")}
        </Badge>
      );
    }
    return (
      <Badge color="gray" variant="light" size="xs">
        {t("cookieStatus.context.unknown")}
      </Badge>
    );
  };

  return (
    <Stack gap="md">
      {/* Header */}
      <Group justify="space-between" align="flex-start">
        <Box>
          <Title order={4}>{t("cookieStatus.title")}</Title>
          <Text size="xs" c="dimmed" mt={4}>
            {t("cookieStatus.total", { count: totalCookies })}
          </Text>
        </Box>
        <Tooltip label={t("cookieStatus.refreshTooltip")}>
          <Button
            onClick={handleRefresh}
            variant={isForceRefreshing ? "filled" : "light"}
            color={isForceRefreshing ? "orange" : "gray"}
            size="sm"
            leftSection={
              loading ? <Loader size={14} /> : <IconRefresh size={16} />
            }
            disabled={loading}
          >
            {loading
              ? isForceRefreshing
                ? t("cookieStatus.forceRefreshing")
                : t("cookieStatus.refreshing")
              : t("cookieStatus.refresh")}
          </Button>
        </Tooltip>
      </Group>

      {/* Error Display */}
      {error && (
        <Alert color="red" icon={<IconX size={16} />} withCloseButton onClose={() => setError(null)}>
          {error}
        </Alert>
      )}

      {/* Loading State */}
      {loading && totalCookies === 0 && (
        <Center py="xl">
          <Loader size="lg" />
        </Center>
      )}

      {/* Cookie Sections */}
      <Stack gap="md">
        {/* Valid Cookies */}
        <CookieSection
          title={t("cookieStatus.sections.valid")}
          cookies={cookieStatus.valid}
          color="green"
          renderStatus={(status, index) => {
            const contextBadge = renderContextBadge(status.supports_claude_1m);
            const usageStats = renderUsageStats(status);
            const quotaStats = renderQuotaStats(status);
            const hasMeta = contextBadge || usageStats || quotaStats;
            return (
              <Box key={index} py="xs">
                <Group justify="space-between" align="flex-start" wrap="nowrap">
                  <Box style={{ flex: 1, minWidth: 0 }}>
                    <Text c="green.4" component="div">
                      <CookieValue cookie={status.cookie} />
                    </Text>
                    {hasMeta && (
                      <details style={{ marginTop: 4 }}>
                        <summary style={{ cursor: "pointer", fontSize: "var(--mantine-font-size-xs)", color: "var(--mantine-color-dimmed)" }}>
                          {t("cookieStatus.meta.summary")}
                        </summary>
                        <Stack gap="xs" mt="xs">
                          {contextBadge && <Box>{contextBadge}</Box>}
                          {usageStats}
                          {quotaStats}
                        </Stack>
                      </details>
                    )}
                  </Box>
                  <Group gap="xs" wrap="nowrap">
                    <Text size="sm" c="dimmed">
                      {t("cookieStatus.status.available")}
                    </Text>
                    <DeleteButton
                      cookie={status.cookie}
                      onDelete={handleDeleteCookie}
                      isDeleting={deletingCookie === status.cookie}
                    />
                  </Group>
                </Group>
              </Box>
            );
          }}
        />

        {/* Exhausted Cookies */}
        <CookieSection
          title={t("cookieStatus.sections.exhausted")}
          cookies={cookieStatus.exhausted}
          color="yellow"
          renderStatus={(status, index) => {
            const contextBadge = renderContextBadge(status.supports_claude_1m);
            const usageStats = renderUsageStats(status);
            const quotaStats = renderQuotaStats(status);
            const hasMeta = contextBadge || usageStats || quotaStats;
            return (
              <Box key={index} py="xs">
                <Group justify="space-between" align="flex-start" wrap="nowrap">
                  <Box style={{ flex: 1, minWidth: 0 }}>
                    <Text c="yellow.4" component="div">
                      <CookieValue cookie={status.cookie} />
                    </Text>
                    {hasMeta && (
                      <details style={{ marginTop: 4 }}>
                        <summary style={{ cursor: "pointer", fontSize: "var(--mantine-font-size-xs)", color: "var(--mantine-color-dimmed)" }}>
                          {t("cookieStatus.meta.summary")}
                        </summary>
                        <Stack gap="xs" mt="xs">
                          {contextBadge && <Box>{contextBadge}</Box>}
                          {usageStats}
                          {quotaStats}
                        </Stack>
                      </details>
                    )}
                  </Box>
                  <Group gap="xs" wrap="nowrap">
                    <Text size="sm" c="dimmed">
                      {(() => {
                        const cooldown = getCooldownDisplay(status);
                        if (!cooldown)
                          return t("cookieStatus.status.unknownReset");
                        return `${cooldown.label}: ${cooldown.time}`;
                      })()}
                    </Text>
                    <DeleteButton
                      cookie={status.cookie}
                      onDelete={handleDeleteCookie}
                      isDeleting={deletingCookie === status.cookie}
                    />
                  </Group>
                </Group>
              </Box>
            );
          }}
        />

        {/* Invalid Cookies */}
        <CookieSection
          title={t("cookieStatus.sections.invalid")}
          cookies={cookieStatus.invalid}
          color="red"
          renderStatus={(status, index) => {
            return (
              <Box key={index} py="xs">
                <Group justify="space-between" align="flex-start" wrap="nowrap">
                  <Box style={{ flex: 1, minWidth: 0 }}>
                    <Text c="red.4" component="div">
                      <CookieValue cookie={status.cookie} />
                    </Text>
                    {renderUsageStats(status)}
                  </Box>
                  <Group gap="xs" wrap="nowrap">
                    <Text size="sm" c="dimmed">
                      {getReasonText(status.reason)}
                    </Text>
                    <DeleteButton
                      cookie={status.cookie}
                      onDelete={handleDeleteCookie}
                      isDeleting={deletingCookie === status.cookie}
                    />
                  </Group>
                </Group>
              </Box>
            );
          }}
        />
      </Stack>
    </Stack>
  );
};

export default CookieVisualization;
