// frontend/src/components/dashboard/LogsPanel.tsx
import { useState, useEffect, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  Paper,
  Group,
  Text,
  TextInput,
  Button,
  Badge,
  Checkbox,
  SegmentedControl,
  Stack,
  ScrollArea,
  Loader,
  Box,
} from "@mantine/core";
import {
  IconSearch,
  IconArrowUp,
  IconArrowDown,
  IconRefresh,
  IconTerminal2,
} from "@tabler/icons-react";
import { getLogs } from "../../api";
import { useWebSocket } from "../../hooks/useWebSocket";

type LogLevel = "all" | "error" | "warn" | "info" | "debug";
type ConnectionMode = "websocket" | "polling";

const MAX_LOG_LINES = 2000;

export function LogsPanel() {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [autoScroll, setAutoScroll] = useState(true);
  const [filter, setFilter] = useState<LogLevel>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [connectionMode, setConnectionMode] = useState<ConnectionMode>("websocket");
  const viewportRef = useRef<HTMLDivElement>(null);

  const token = localStorage.getItem("authToken") || "";
  const wsProtocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const wsUrl = `${wsProtocol}//${window.location.host}/api/ws/logs?token=${encodeURIComponent(token)}&initial_lines=500`;

  const handleMessage = useCallback(
    (message: { type: string; line?: string }) => {
      if (message.type === "log" && message.line) {
        setLogs((prev) => {
          const newLogs = [...prev, message.line!];
          if (newLogs.length > MAX_LOG_LINES) {
            return newLogs.slice(-MAX_LOG_LINES);
          }
          return newLogs;
        });
      } else if (message.type === "init_complete") {
        setLoading(false);
      }
    },
    []
  );

  const { status, connect, disconnect, isConnected } = useWebSocket({
    url: wsUrl,
    onMessage: handleMessage,
    onOpen: () => {
      setLogs([]);
      setLoading(true);
    },
    onClose: () => {
      if (connectionMode === "websocket" && logs.length === 0) {
        setConnectionMode("polling");
      }
    },
  });

  useEffect(() => {
    if (connectionMode === "websocket" && token) {
      connect();
      return () => disconnect();
    }
  }, [connectionMode, token, connect, disconnect]);

  const fetchLogs = useCallback(async () => {
    try {
      const data = await getLogs(2000);
      setLogs(data.logs);
    } catch (error) {
      console.error("Failed to fetch logs:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (connectionMode === "polling") {
      fetchLogs();
      const interval = setInterval(fetchLogs, 5000);
      return () => clearInterval(interval);
    }
  }, [connectionMode, fetchLogs]);

  useEffect(() => {
    if (autoScroll && viewportRef.current) {
      viewportRef.current.scrollTo({
        top: viewportRef.current.scrollHeight,
        behavior: "smooth",
      });
    }
  }, [logs, autoScroll]);

  const getLogLevel = (line: string): LogLevel => {
    if (line.includes(" ERROR ") || line.includes("[ERROR]")) return "error";
    if (line.includes(" WARN ") || line.includes("[WARN]")) return "warn";
    if (line.includes(" INFO ") || line.includes("[INFO]")) return "info";
    if (line.includes(" DEBUG ") || line.includes("[DEBUG]")) return "debug";
    return "info";
  };

  const getLogColor = (level: LogLevel): string => {
    switch (level) {
      case "error":
        return "red";
      case "warn":
        return "yellow";
      case "info":
        return "blue";
      case "debug":
        return "gray";
      default:
        return "dimmed";
    }
  };

  const filteredLogs = logs.filter((line) => {
    const level = getLogLevel(line);
    const matchesFilter = filter === "all" || level === filter;
    const matchesSearch =
      !searchQuery || line.toLowerCase().includes(searchQuery.toLowerCase());
    return matchesFilter && matchesSearch;
  });

  const levelCounts = {
    error: logs.filter((l) => getLogLevel(l) === "error").length,
    warn: logs.filter((l) => getLogLevel(l) === "warn").length,
    info: logs.filter((l) => getLogLevel(l) === "info").length,
    debug: logs.filter((l) => getLogLevel(l) === "debug").length,
  };

  const getStatusBadge = () => {
    if (connectionMode === "polling") {
      return (
        <Badge variant="light" color="yellow" size="sm">
          Polling
        </Badge>
      );
    }

    switch (status) {
      case "connected":
        return (
          <Badge variant="light" color="green" size="sm" leftSection={<span style={{ animation: "pulse 2s infinite" }}>●</span>}>
            Live
          </Badge>
        );
      case "connecting":
        return (
          <Badge variant="light" color="blue" size="sm">
            Connecting...
          </Badge>
        );
      default:
        return (
          <Badge variant="light" color="red" size="sm">
            Disconnected
          </Badge>
        );
    }
  };

  return (
    <Paper h="100%" radius="md" withBorder style={{ display: "flex", flexDirection: "column" }}>
      {/* Header */}
      <Box p="md" style={{ borderBottom: "1px solid var(--mantine-color-default-border)" }}>
        <Group justify="space-between" mb="sm">
          <Group gap="sm">
            <IconTerminal2 size={20} style={{ color: "var(--mantine-color-cyan-5)" }} />
            <Text fw={500}>{t("logs.title")}</Text>
            {getStatusBadge()}
          </Group>
          <Group gap="xs">
            <Text size="xs" c="dimmed">
              {t("logs.totalLines", { count: filteredLogs.length })}
            </Text>
            <Button
              size="xs"
              variant="subtle"
              leftSection={<IconArrowUp size={14} />}
              onClick={() => {
                if (viewportRef.current) {
                  viewportRef.current.scrollTo({ top: 0, behavior: "smooth" });
                  setAutoScroll(false);
                }
              }}
            >
              {t("logs.scrollToTop")}
            </Button>
            <Button
              size="xs"
              variant="subtle"
              leftSection={<IconArrowDown size={14} />}
              onClick={() => {
                if (viewportRef.current) {
                  viewportRef.current.scrollTo({
                    top: viewportRef.current.scrollHeight,
                    behavior: "smooth",
                  });
                  setAutoScroll(true);
                }
              }}
            >
              {t("logs.scrollToBottom")}
            </Button>
            {connectionMode === "websocket" && !isConnected && (
              <Button
                size="xs"
                variant="subtle"
                leftSection={<IconRefresh size={14} />}
                onClick={connect}
              >
                Reconnect
              </Button>
            )}
          </Group>
        </Group>

        {/* Filters */}
        <Group gap="md">
          <TextInput
            placeholder={t("dashboard.searchLogs")}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            leftSection={<IconSearch size={16} />}
            size="xs"
            style={{ flex: 1, minWidth: 200 }}
          />

          <SegmentedControl
            size="xs"
            value={filter}
            onChange={(value) => setFilter(value as LogLevel)}
            data={[
              { label: `All (${logs.length})`, value: "all" },
              { label: `Error (${levelCounts.error})`, value: "error" },
              { label: `Warn (${levelCounts.warn})`, value: "warn" },
              { label: `Info (${levelCounts.info})`, value: "info" },
              { label: `Debug (${levelCounts.debug})`, value: "debug" },
            ]}
          />

          <Checkbox
            label={t("logs.autoScroll")}
            checked={autoScroll}
            onChange={(e) => setAutoScroll(e.currentTarget.checked)}
            size="xs"
          />
        </Group>
      </Box>

      {/* Logs content */}
      <ScrollArea
        flex={1}
        p="md"
        viewportRef={viewportRef}
        onScrollPositionChange={(pos) => {
          if (viewportRef.current) {
            const isAtBottom =
              viewportRef.current.scrollHeight - pos.y <=
              viewportRef.current.clientHeight + 50;
            setAutoScroll(isAtBottom);
          }
        }}
      >
        {loading ? (
          <Stack align="center" justify="center" py="xl">
            <Loader size="sm" />
            <Text size="sm" c="dimmed">
              {t("common.loading")}
            </Text>
          </Stack>
        ) : filteredLogs.length === 0 ? (
          <Text size="sm" c="dimmed" ta="center" py="xl">
            {searchQuery || filter !== "all"
              ? t("dashboard.noMatchingLogs")
              : t("logs.noLogs")}
          </Text>
        ) : (
          <Stack gap={2}>
            {filteredLogs.map((line, index) => {
              const level = getLogLevel(line);
              const color = getLogColor(level);

              return (
                <Box
                  key={index}
                  p={4}
                  style={{
                    fontFamily: "monospace",
                    fontSize: "12px",
                    whiteSpace: "pre-wrap",
                    wordBreak: "break-all",
                    borderRadius: "var(--mantine-radius-xs)",
                    borderLeft: `3px solid var(--mantine-color-${color}-5)`,
                    background: `var(--mantine-color-${color}-9)`,
                    color: `var(--mantine-color-${color}-4)`,
                  }}
                >
                  {line}
                </Box>
              );
            })}
          </Stack>
        )}
      </ScrollArea>
    </Paper>
  );
}

export default LogsPanel;
